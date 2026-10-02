use std::time::{Duration, Instant};

use crate::{Config, MouseClickMode};
use nanomiddleclick_input::{
    MouseAction, MouseEventKind, TouchDeviceKind, TouchSource,
};

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GestureOutcome {
    None,
    EmulateMiddleClick,
    BeginMiddleDrag,
    ContinueMiddleDrag { delta_x: f64, delta_y: f64 },
    EndMiddleDrag,
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

#[derive(Clone, Copy, PartialEq)]
enum TouchSequenceState {
    Tracking,
    Dragging,
    Consumed,
}

pub struct GestureEngine {
    config: Config,
    required_touch_down: bool,
    magic_mouse_center_touch_down: bool,
    click_rewrite_deadline: Option<Instant>,
    rewritten_mouse_down_active: bool,
    touch_sequence_state: TouchSequenceState,
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
            touch_sequence_state: TouchSequenceState::Tracking,
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

    pub fn cancel_current_touch_sequence(&mut self) -> GestureOutcome {
        let outcome = if self.touch_sequence_state == TouchSequenceState::Dragging {
            GestureOutcome::EndMiddleDrag
        } else {
            GestureOutcome::None
        };
        self.touch_sequence_state = TouchSequenceState::Tracking;
        self.required_touch_down = false;
        self.magic_mouse_center_touch_down = false;
        self.click_rewrite_deadline = None;
        self.touch_start_time = None;
        self.start_centroid = None;
        self.latest_centroid = None;
        outcome
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
        if self.touch_sequence_state == TouchSequenceState::Dragging
            && source_kind != TouchDeviceKind::Trackpad
        {
            return GestureOutcome::None;
        }

        let now = Instant::now();
        let trackpad_drag_allowed =
            self.config.drag && source_kind == TouchDeviceKind::Trackpad;
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

        if self.touch_sequence_state == TouchSequenceState::Dragging {
            if !self.required_touch_down || !self.config.drag {
                self.touch_sequence_state = if touch_count == 0 {
                    TouchSequenceState::Tracking
                } else {
                    TouchSequenceState::Consumed
                };
                self.click_rewrite_deadline = None;
                self.clear_tap_tracking();
                return GestureOutcome::EndMiddleDrag;
            }

            if let Some(centroid) = analysis.centroid {
                let previous =
                    self.latest_centroid.replace(centroid).unwrap_or(centroid);
                if previous == centroid {
                    return GestureOutcome::None;
                }
                return GestureOutcome::ContinueMiddleDrag {
                    delta_x: centroid.x - previous.x,
                    delta_y: centroid.y - previous.y,
                };
            }
        }

        if touch_count == 0 {
            let consumed = self.touch_sequence_state == TouchSequenceState::Consumed;
            self.touch_sequence_state = TouchSequenceState::Tracking;
            if consumed || !self.tap_gesture_allowed_for_source(source_kind) {
                self.clear_tap_tracking();
                return GestureOutcome::None;
            }

            let outcome = self.finish_touch_sequence();
            if outcome == GestureOutcome::EmulateMiddleClick {
                self.click_rewrite_deadline = None;
            }
            return outcome;
        }

        if self.touch_sequence_state == TouchSequenceState::Consumed
            || self.rewritten_mouse_down_active
            || (!self.tap_gesture_allowed_for_source(source_kind)
                && !trackpad_drag_allowed)
        {
            self.clear_tap_tracking();
            return GestureOutcome::None;
        }

        if !trackpad_drag_allowed && self.touch_start_time.is_none() {
            self.touch_start_time = Some(now);
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

        self.latest_centroid = Some(centroid);
        if trackpad_drag_allowed {
            self.touch_sequence_state = TouchSequenceState::Dragging;
            self.click_rewrite_deadline = None;
            return GestureOutcome::BeginMiddleDrag;
        }

        if self.start_centroid.is_none() {
            self.start_centroid = Some(centroid);
        }
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
                let already_down =
                    self.touch_sequence_state == TouchSequenceState::Dragging;
                self.touch_sequence_state = TouchSequenceState::Consumed;
                self.clear_tap_tracking();
                self.rewritten_mouse_down_active = true;
                self.required_touch_down = false;
                self.magic_mouse_center_touch_down = false;
                self.click_rewrite_deadline = None;
                self.natural_middle_click_last_time = Some(Instant::now());
                if already_down {
                    MouseAction::Suppress
                } else {
                    MouseAction::RewriteDown
                }
            }
            MouseEventKind::LeftUp | MouseEventKind::RightUp
                if self.rewritten_mouse_down_active =>
            {
                self.rewritten_mouse_down_active = false;
                MouseAction::RewriteUp
            }
            MouseEventKind::Moved
                if self.config.drag && self.rewritten_mouse_down_active =>
            {
                MouseAction::RewriteDrag
            }
            MouseEventKind::Moved
                if self.touch_sequence_state == TouchSequenceState::Dragging =>
            {
                MouseAction::Suppress
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
            if !x.is_finite() || !y.is_finite() {
                return TouchAnalysis::default();
            }
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
            drag: false,
            drag_sensitivity: 1000.0,
            mouse_click_mode: MouseClickMode::ThreeFinger,
            ignored_app_bundles: Vec::new().into_boxed_slice(),
        }
    }

    fn config_with_mouse_click_mode(mouse_click_mode: MouseClickMode) -> Config {
        let mut config = config();
        config.mouse_click_mode = mouse_click_mode;
        config
    }

    fn drag_config() -> Config {
        let mut settings = config();
        settings.drag = true;
        settings
    }

    fn touch(x: f32, y: f32) -> TouchContact {
        TouchContact { x, y, touching: true }
    }

    fn hover_touch(x: f32, y: f32) -> TouchContact {
        TouchContact { x, y, touching: false }
    }

    #[test]
    fn holds_middle_button_for_physical_drag_and_preserves_release_after_cancel() {
        for (down, drag, up) in [
            (
                MouseEventKind::LeftDown,
                MouseEventKind::Moved,
                MouseEventKind::LeftUp,
            ),
            (
                MouseEventKind::RightDown,
                MouseEventKind::Moved,
                MouseEventKind::RightUp,
            ),
        ] {
            let mut settings = drag_config();
            settings.tap_to_click = false;
            let mut engine = GestureEngine::new(settings);
            engine.handle_touch_frame(TouchDeviceKind::Mouse, [touch(0.1, 0.1); 3]);
            assert_eq!(engine.handle_mouse_event(down), MouseAction::RewriteDown);
            assert_eq!(engine.handle_mouse_event(drag), MouseAction::RewriteDrag);
            assert_eq!(engine.cancel_current_touch_sequence(), GestureOutcome::None);
            assert_eq!(engine.handle_mouse_event(drag), MouseAction::RewriteDrag);
            let mut updated = engine.config().clone();
            updated.drag = false;
            engine.update_config(updated);
            assert_eq!(engine.handle_mouse_event(drag), MouseAction::Pass);
            assert_eq!(engine.handle_mouse_event(up), MouseAction::RewriteUp);
            assert_eq!(engine.handle_mouse_event(drag), MouseAction::Pass);
        }
    }

    #[test]
    fn presses_middle_button_on_contact_and_moves_until_release_or_cancel() {
        for finish in 0..3 {
            let mut settings = drag_config();
            // Tap limits must not delay the press or prevent pointer movement.
            settings.tap_to_click = false;
            settings.max_distance_delta = 10.0;
            settings.max_time_delta = Duration::ZERO;
            let mut engine = GestureEngine::new(settings);
            assert_eq!(
                engine.handle_touch_frame(
                    TouchDeviceKind::Trackpad,
                    [touch(0.1, 0.1); 3]
                ),
                GestureOutcome::BeginMiddleDrag,
            );
            assert_eq!(
                engine.handle_touch_frame(
                    TouchDeviceKind::Trackpad,
                    [touch(0.1, 0.1); 3]
                ),
                GestureOutcome::None,
            );
            let GestureOutcome::ContinueMiddleDrag { delta_x, delta_y } = engine
                .handle_touch_frame(
                    TouchDeviceKind::Trackpad,
                    [touch(0.2, 0.15); 3],
                )
            else {
                panic!("movement must move the pointer with the button held");
            };
            assert!((delta_x - 0.1).abs() < 1e-6);
            assert!((delta_y - 0.05).abs() < 1e-6);
            assert_eq!(
                engine.handle_mouse_event(MouseEventKind::Moved),
                MouseAction::Suppress
            );
            // A Magic Mouse frame must not end an active trackpad drag.
            assert_eq!(
                engine.handle_touch_frame(
                    TouchDeviceKind::Mouse,
                    std::iter::empty::<TouchContact>()
                ),
                GestureOutcome::None
            );
            let GestureOutcome::ContinueMiddleDrag { delta_x, delta_y } = engine
                .handle_touch_frame(
                    TouchDeviceKind::Trackpad,
                    [touch(0.23, 0.14); 3],
                )
            else {
                panic!("movement must continue the drag");
            };
            assert!((delta_x - 0.03).abs() < 1e-6);
            assert!((delta_y + 0.01).abs() < 1e-6);
            match finish {
                0 => assert_eq!(
                    engine.handle_touch_frame(
                        TouchDeviceKind::Trackpad,
                        [touch(0.23, 0.14); 2]
                    ),
                    GestureOutcome::EndMiddleDrag,
                ),
                1 => {
                    let mut updated = engine.config().clone();
                    updated.drag = false;
                    engine.update_config(updated);
                    assert_eq!(
                        engine.cancel_current_touch_sequence(),
                        GestureOutcome::EndMiddleDrag
                    );
                }
                _ => {
                    assert_eq!(
                        engine.handle_mouse_event(MouseEventKind::LeftDown),
                        MouseAction::Suppress
                    );
                    assert_eq!(
                        engine.handle_mouse_event(MouseEventKind::Moved),
                        MouseAction::RewriteDrag
                    );
                    assert_eq!(
                        engine.handle_mouse_event(MouseEventKind::LeftUp),
                        MouseAction::RewriteUp
                    );
                }
            }
            assert_eq!(
                engine.handle_touch_frame(
                    TouchDeviceKind::Trackpad,
                    std::iter::empty::<TouchContact>()
                ),
                GestureOutcome::None
            );
            assert_eq!(
                engine.handle_mouse_event(MouseEventKind::Moved),
                MouseAction::Pass
            );
        }
    }

    #[test]
    fn touch_drag_respects_option_finger_count_and_device_kind() {
        for (drag, tap_to_click, source, count) in [
            (false, true, TouchDeviceKind::Trackpad, 3),
            (false, false, TouchDeviceKind::Trackpad, 3),
            (true, true, TouchDeviceKind::Mouse, 3),
            (true, true, TouchDeviceKind::Trackpad, 2),
            (true, true, TouchDeviceKind::Trackpad, 4),
        ] {
            let mut settings = config();
            settings.tap_to_click = tap_to_click;
            settings.drag = drag;
            let mut engine = GestureEngine::new(settings);
            for position in [0.1, 0.2] {
                assert_eq!(
                    engine.handle_touch_frame(
                        source,
                        std::iter::repeat_n(touch(position, position), count)
                    ),
                    GestureOutcome::None,
                );
            }
            assert_eq!(
                engine
                    .handle_touch_frame(source, std::iter::empty::<TouchContact>()),
                GestureOutcome::None
            );
        }
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
            engine.handle_mouse_event(MouseEventKind::Moved),
            MouseAction::Pass
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
    fn short_contact_presses_and_releases_while_magic_mouse_keeps_tap_detection() {
        for (drag, source, press, release) in [
            (
                true,
                TouchDeviceKind::Trackpad,
                GestureOutcome::BeginMiddleDrag,
                GestureOutcome::EndMiddleDrag,
            ),
            (
                true,
                TouchDeviceKind::Mouse,
                GestureOutcome::None,
                GestureOutcome::EmulateMiddleClick,
            ),
            (
                false,
                TouchDeviceKind::Trackpad,
                GestureOutcome::None,
                GestureOutcome::EmulateMiddleClick,
            ),
        ] {
            let mut settings = config();
            settings.drag = drag;
            let mut engine = GestureEngine::new(settings);
            assert_eq!(
                engine.handle_touch_frame(source, [touch(0.1, 0.1); 3]),
                press
            );
            assert_eq!(
                engine
                    .handle_touch_frame(source, std::iter::empty::<TouchContact>()),
                release
            );
        }
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
        let mut engine = GestureEngine::new(drag_config());
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
            GestureOutcome::EndMiddleDrag
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
