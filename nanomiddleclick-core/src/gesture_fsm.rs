use std::time::{Duration, Instant};

use crate::{Config, MouseClickMode};

pub trait TouchSource {
    fn is_touching(&self) -> bool;
    fn normalized_position(&self) -> (f32, f32);
}

impl<T: TouchSource + ?Sized> TouchSource for &T {
    fn is_touching(&self) -> bool {
        (*self).is_touching()
    }

    fn normalized_position(&self) -> (f32, f32) {
        (*self).normalized_position()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TouchContact {
    pub x: f32,
    pub y: f32,
    pub touching: bool,
}

impl TouchSource for TouchContact {
    fn is_touching(&self) -> bool {
        self.touching
    }

    fn normalized_position(&self) -> (f32, f32) {
        (self.x, self.y)
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TouchDeviceKind {
    #[default]
    Unknown = 0,
    Mouse = 1,
    Trackpad = 2,
}

impl TouchDeviceKind {
    pub fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            0 => Some(Self::Unknown),
            1 => Some(Self::Mouse),
            2 => Some(Self::Trackpad),
            _ => None,
        }
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseEventKind {
    LeftDown = 1,
    LeftUp = 2,
    RightDown = 3,
    RightUp = 4,
}

impl MouseEventKind {
    pub fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            1 => Some(Self::LeftDown),
            2 => Some(Self::LeftUp),
            3 => Some(Self::RightDown),
            4 => Some(Self::RightUp),
            _ => None,
        }
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseAction {
    Pass = 0,
    RewriteDown = 1,
    RewriteUp = 2,
}

impl MouseAction {
    pub fn as_raw(self) -> u32 {
        self as u32
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum GestureOutcome {
    None,
    EmulateMiddleClick,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Point {
    x: f64,
    y: f64,
}

impl Point {
    fn delta_to(self, other: Self) -> f64 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct TouchAnalysis {
    active_count: usize,
    centroid: Option<Point>,
    single_active_position: Option<Point>,
}

pub struct GestureEngine {
    config: Config,
    required_touch_down: bool,
    magic_mouse_center_touch_down: bool,
    click_rewrite_deadline: Option<Instant>,
    rewritten_mouse_down_active: bool,
    natural_middle_click_last_time: Option<Instant>,
    touch_start_time: Option<Instant>,
    start_centroid: Option<Point>,
    latest_centroid: Option<Point>,
}

impl GestureEngine {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            required_touch_down: false,
            magic_mouse_center_touch_down: false,
            click_rewrite_deadline: None,
            rewritten_mouse_down_active: false,
            natural_middle_click_last_time: None,
            touch_start_time: None,
            start_centroid: None,
            latest_centroid: None,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn update_config(&mut self, config: Config) {
        self.config = config;
    }

    pub fn cancel_current_touch_sequence(&mut self) {
        self.required_touch_down = false;
        self.magic_mouse_center_touch_down = false;
        self.click_rewrite_deadline = None;
        self.touch_start_time = None;
        self.start_centroid = None;
        self.latest_centroid = None;
    }

    pub fn reset_for_ignored_app(&mut self) {
        self.cancel_current_touch_sequence();
        self.rewritten_mouse_down_active = false;
    }

    pub fn handle_touch_frame<I, T>(
        &mut self,
        source_kind: TouchDeviceKind,
        touches: I,
    ) -> GestureOutcome
    where
        I: IntoIterator<Item = T>,
        T: TouchSource,
    {
        let now = Instant::now();
        let analysis = self.analyze_touches(touches);
        let touch_count = analysis.active_count;
        let matches_required_fingers =
            self.count_matches_required_fingers(touch_count);
        self.required_touch_down = matches_required_fingers
            && self.finger_click_allowed_for_source(source_kind);
        self.magic_mouse_center_touch_down = self
            .center_click_allowed_for_source(source_kind)
            && Self::is_magic_mouse_center_touch(source_kind, analysis);

        if self.required_touch_down {
            self.click_rewrite_deadline = Some(now + click_rewrite_grace());
        }

        if touch_count == 0 {
            if !self.tap_gesture_allowed_for_source(source_kind) {
                self.clear_tap_tracking();
                return GestureOutcome::None;
            }

            let outcome = self.finish_touch_sequence();
            if outcome == GestureOutcome::EmulateMiddleClick {
                self.click_rewrite_deadline = None;
            }
            return outcome;
        }

        if !self.tap_gesture_allowed_for_source(source_kind) {
            self.clear_tap_tracking();
            return GestureOutcome::None;
        }

        if self.touch_start_time.is_none() {
            self.touch_start_time = Some(now);
        } else if let Some(start_time) = self.touch_start_time {
            if start_time.elapsed() > self.config.max_time_delta {
                self.start_centroid = None;
            }
        }

        if touch_count < self.config.fingers {
            return GestureOutcome::None;
        }

        if !matches_required_fingers {
            self.start_centroid = None;
            self.latest_centroid = None;
            return GestureOutcome::None;
        }

        let Some(centroid) = analysis.centroid else {
            return GestureOutcome::None;
        };

        if self.start_centroid.is_none() {
            self.start_centroid = Some(centroid);
        }
        self.latest_centroid = Some(centroid);
        GestureOutcome::None
    }

    pub fn handle_mouse_event(&mut self, kind: MouseEventKind) -> MouseAction {
        let click_eligible = self.required_touch_down
            || self.magic_mouse_center_touch_down
            || self.has_recent_click_touch();
        match kind {
            MouseEventKind::LeftDown | MouseEventKind::RightDown
                if click_eligible =>
            {
                self.rewritten_mouse_down_active = true;
                self.required_touch_down = false;
                self.magic_mouse_center_touch_down = false;
                self.click_rewrite_deadline = None;
                self.natural_middle_click_last_time = Some(Instant::now());
                MouseAction::RewriteDown
            }
            MouseEventKind::LeftUp | MouseEventKind::RightUp
                if self.rewritten_mouse_down_active =>
            {
                self.rewritten_mouse_down_active = false;
                MouseAction::RewriteUp
            }
            _ => MouseAction::Pass,
        }
    }

    fn finish_touch_sequence(&mut self) -> GestureOutcome {
        let Some(start_time) = self.touch_start_time.take() else {
            self.start_centroid = None;
            self.latest_centroid = None;
            return GestureOutcome::None;
        };

        let elapsed = start_time.elapsed();
        let start_centroid = self.start_centroid.take();
        let latest_centroid = self.latest_centroid.take();
        self.required_touch_down = false;
        self.magic_mouse_center_touch_down = false;

        let Some(start_centroid) = start_centroid else {
            return GestureOutcome::None;
        };
        let latest_centroid = latest_centroid.unwrap_or(start_centroid);

        if elapsed > self.config.max_time_delta {
            return GestureOutcome::None;
        }

        if start_centroid.delta_to(latest_centroid) >= self.config.max_distance_delta
        {
            return GestureOutcome::None;
        }

        if self.should_suppress_synthetic_click() {
            return GestureOutcome::None;
        }

        GestureOutcome::EmulateMiddleClick
    }

    fn count_matches_required_fingers(&self, touch_count: usize) -> bool {
        if self.config.allow_more_fingers {
            touch_count >= self.config.fingers
        } else {
            touch_count == self.config.fingers
        }
    }

    fn finger_click_allowed_for_source(&self, source_kind: TouchDeviceKind) -> bool {
        source_kind != TouchDeviceKind::Mouse
            || self.config.mouse_click_mode == MouseClickMode::ThreeFinger
    }

    fn center_click_allowed_for_source(&self, source_kind: TouchDeviceKind) -> bool {
        source_kind == TouchDeviceKind::Mouse
            && self.config.mouse_click_mode == MouseClickMode::Center
    }

    fn tap_gesture_allowed_for_source(&self, source_kind: TouchDeviceKind) -> bool {
        self.config.tap_to_click && self.finger_click_allowed_for_source(source_kind)
    }

    fn is_magic_mouse_center_touch(
        source_kind: TouchDeviceKind,
        analysis: TouchAnalysis,
    ) -> bool {
        const MAGIC_MOUSE_CENTER_MIN_X: f64 = 0.35;
        const MAGIC_MOUSE_CENTER_MAX_X: f64 = 0.65;

        source_kind == TouchDeviceKind::Mouse
            && analysis.active_count == 1
            && analysis.single_active_position.is_some_and(|position| {
                (MAGIC_MOUSE_CENTER_MIN_X..=MAGIC_MOUSE_CENTER_MAX_X)
                    .contains(&position.x)
            })
    }

    fn analyze_touches<I, T>(&self, touches: I) -> TouchAnalysis
    where
        I: IntoIterator<Item = T>,
        T: TouchSource,
    {
        let mut active_count = 0usize;
        let mut sample_count = 0usize;
        let mut sum = Point::default();
        let mut single_active_position = None;
        for touch in touches {
            if !touch.is_touching() {
                continue;
            }

            active_count += 1;
            let (x, y) = touch.normalized_position();
            if active_count == 1 {
                single_active_position =
                    Some(Point { x: f64::from(x), y: f64::from(y) });
            } else {
                single_active_position = None;
            }

            if sample_count >= self.config.fingers {
                continue;
            }

            sum.x += f64::from(x);
            sum.y += f64::from(y);
            sample_count += 1;
        }

        let centroid = if sample_count == 0 {
            None
        } else {
            let sample_count = u32::try_from(sample_count)
                .expect("finger count should fit into u32");
            Some(Point {
                x: sum.x / f64::from(sample_count),
                y: sum.y / f64::from(sample_count),
            })
        };

        TouchAnalysis { active_count, centroid, single_active_position }
    }

    fn clear_tap_tracking(&mut self) {
        self.touch_start_time = None;
        self.start_centroid = None;
        self.latest_centroid = None;
    }

    fn should_suppress_synthetic_click(&self) -> bool {
        let Some(last_natural_click) = self.natural_middle_click_last_time else {
            return false;
        };

        last_natural_click.elapsed()
            <= scale_duration(self.config.max_time_delta, 3, 4)
    }

    fn has_recent_click_touch(&mut self) -> bool {
        let Some(deadline) = self.click_rewrite_deadline else {
            return false;
        };

        if Instant::now() <= deadline {
            return true;
        }

        self.click_rewrite_deadline = None;
        false
    }
}

fn scale_duration(duration: Duration, numerator: u32, denominator: u32) -> Duration {
    Duration::from_secs_f64(
        duration.as_secs_f64() * f64::from(numerator) / f64::from(denominator),
    )
}

fn click_rewrite_grace() -> Duration {
    Duration::from_millis(75)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MouseClickMode;

    fn config() -> Config {
        Config {
            fingers: 3,
            allow_more_fingers: false,
            max_distance_delta: 0.05,
            max_time_delta: Duration::from_millis(300),
            tap_to_click: true,
            mouse_click_mode: MouseClickMode::ThreeFinger,
            ignored_app_bundles: Vec::new().into_boxed_slice(),
        }
    }

    fn config_with_mouse_click_mode(mouse_click_mode: MouseClickMode) -> Config {
        let mut config = config();
        config.mouse_click_mode = mouse_click_mode;
        config
    }

    fn touch(x: f32, y: f32) -> TouchContact {
        TouchContact { x, y, touching: true }
    }

    fn hover_touch(x: f32, y: f32) -> TouchContact {
        TouchContact { x, y, touching: false }
    }

    #[test]
    fn rewrites_mouse_down_and_up_when_required_fingers_are_down() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame(
            TouchDeviceKind::Trackpad,
            [touch(0.1, 0.1), touch(0.2, 0.2), touch(0.3, 0.3)],
        );

        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftDown),
            MouseAction::RewriteDown
        );
        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftUp),
            MouseAction::RewriteUp
        );
    }

    #[test]
    fn rewrites_mouse_down_after_brief_touch_drop_before_click() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame(
            TouchDeviceKind::Trackpad,
            [touch(0.1, 0.1), touch(0.2, 0.2), touch(0.3, 0.3)],
        );
        engine.handle_touch_frame(
            TouchDeviceKind::Trackpad,
            [touch(0.1, 0.1), touch(0.2, 0.2)],
        );

        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftDown),
            MouseAction::RewriteDown
        );
    }

    #[test]
    fn cancel_current_touch_sequence_preserves_pending_rewritten_mouse_up() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame(
            TouchDeviceKind::Trackpad,
            [touch(0.1, 0.1), touch(0.2, 0.2), touch(0.3, 0.3)],
        );

        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftDown),
            MouseAction::RewriteDown
        );

        engine.cancel_current_touch_sequence();

        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftUp),
            MouseAction::RewriteUp
        );
        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftDown),
            MouseAction::Pass
        );
    }

    #[test]
    fn updating_config_does_not_drop_pending_rewritten_mouse_up() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame(
            TouchDeviceKind::Trackpad,
            [touch(0.1, 0.1), touch(0.2, 0.2), touch(0.3, 0.3)],
        );

        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftDown),
            MouseAction::RewriteDown
        );

        let mut updated_config = config();
        updated_config.mouse_click_mode = MouseClickMode::Disabled;
        engine.update_config(updated_config);
        engine.cancel_current_touch_sequence();

        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftUp),
            MouseAction::RewriteUp
        );
        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftDown),
            MouseAction::Pass
        );
    }

    #[test]
    fn does_not_rewrite_mouse_down_after_touch_grace_expires() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame(
            TouchDeviceKind::Trackpad,
            [touch(0.1, 0.1), touch(0.2, 0.2), touch(0.3, 0.3)],
        );
        engine.handle_touch_frame(
            TouchDeviceKind::Trackpad,
            [touch(0.1, 0.1), touch(0.2, 0.2)],
        );
        std::thread::sleep(Duration::from_millis(100));

        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftDown),
            MouseAction::Pass
        );
    }

    #[test]
    fn emits_middle_click_for_valid_tap() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame(
            TouchDeviceKind::Trackpad,
            [touch(0.1, 0.1), touch(0.2, 0.2), touch(0.3, 0.3)],
        );
        engine.handle_touch_frame(
            TouchDeviceKind::Trackpad,
            [touch(0.11, 0.1), touch(0.21, 0.2), touch(0.31, 0.3)],
        );

        assert_eq!(
            engine.handle_touch_frame(
                TouchDeviceKind::Trackpad,
                std::iter::empty::<TouchContact>(),
            ),
            GestureOutcome::EmulateMiddleClick
        );
    }

    #[test]
    fn ignores_tap_when_touch_count_never_reaches_required_fingers() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame(
            TouchDeviceKind::Trackpad,
            [touch(0.1, 0.1), touch(0.2, 0.2)],
        );

        assert_eq!(
            engine.handle_touch_frame(
                TouchDeviceKind::Trackpad,
                std::iter::empty::<TouchContact>(),
            ),
            GestureOutcome::None
        );
    }

    #[test]
    fn ignores_hover_touches_for_centroid_and_counting() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame(
            TouchDeviceKind::Trackpad,
            [
                touch(0.1, 0.1),
                touch(0.2, 0.2),
                touch(0.3, 0.3),
                hover_touch(0.9, 0.9),
            ],
        );

        assert_eq!(
            engine.handle_touch_frame(
                TouchDeviceKind::Trackpad,
                std::iter::empty::<TouchContact>(),
            ),
            GestureOutcome::EmulateMiddleClick
        );
    }

    #[test]
    fn rewrites_click_for_magic_mouse_three_finger_touch_in_three_finger_mode() {
        let mut engine = GestureEngine::new(config_with_mouse_click_mode(
            MouseClickMode::ThreeFinger,
        ));
        engine.handle_touch_frame(
            TouchDeviceKind::Mouse,
            [touch(0.1, 0.1), touch(0.2, 0.2), touch(0.3, 0.3)],
        );

        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftDown),
            MouseAction::RewriteDown
        );
        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftUp),
            MouseAction::RewriteUp
        );
    }

    #[test]
    fn rewrites_click_for_magic_mouse_center_touch_in_center_mode() {
        let mut engine =
            GestureEngine::new(config_with_mouse_click_mode(MouseClickMode::Center));
        engine.handle_touch_frame(TouchDeviceKind::Mouse, [touch(0.5, 0.4)]);

        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftDown),
            MouseAction::RewriteDown
        );
    }

    #[test]
    fn does_not_rewrite_click_for_magic_mouse_off_center_touch_in_center_mode() {
        let mut engine =
            GestureEngine::new(config_with_mouse_click_mode(MouseClickMode::Center));
        engine.handle_touch_frame(TouchDeviceKind::Mouse, [touch(0.2, 0.4)]);

        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftDown),
            MouseAction::Pass
        );
    }

    #[test]
    fn does_not_rewrite_click_for_magic_mouse_center_touch_in_disabled_mode() {
        let mut engine = GestureEngine::new(config_with_mouse_click_mode(
            MouseClickMode::Disabled,
        ));
        engine.handle_touch_frame(TouchDeviceKind::Mouse, [touch(0.5, 0.4)]);

        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftDown),
            MouseAction::Pass
        );
    }

    #[test]
    fn does_not_emit_tap_for_magic_mouse_in_center_mode() {
        let mut engine =
            GestureEngine::new(config_with_mouse_click_mode(MouseClickMode::Center));
        engine.handle_touch_frame(
            TouchDeviceKind::Mouse,
            [touch(0.1, 0.1), touch(0.2, 0.2), touch(0.3, 0.3)],
        );

        assert_eq!(
            engine.handle_touch_frame(
                TouchDeviceKind::Mouse,
                std::iter::empty::<TouchContact>(),
            ),
            GestureOutcome::None
        );
    }

    #[test]
    fn does_not_rewrite_click_for_trackpad_center_touch() {
        let mut engine =
            GestureEngine::new(config_with_mouse_click_mode(MouseClickMode::Center));
        engine.handle_touch_frame(TouchDeviceKind::Trackpad, [touch(0.5, 0.4)]);

        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftDown),
            MouseAction::Pass
        );
    }
}
