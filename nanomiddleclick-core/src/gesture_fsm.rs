use std::time::{Duration, Instant};

use crate::Config;

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
}

pub struct GestureEngine {
    config: Config,
    required_touch_down: bool,
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
        self.reset_for_ignored_app();
    }

    pub fn cancel_current_touch_sequence(&mut self) {
        self.required_touch_down = false;
        self.click_rewrite_deadline = None;
        self.touch_start_time = None;
        self.start_centroid = None;
        self.latest_centroid = None;
    }

    pub fn reset_for_ignored_app(&mut self) {
        self.cancel_current_touch_sequence();
        self.rewritten_mouse_down_active = false;
    }

    pub fn handle_touch_frame<I, T>(&mut self, touches: I) -> GestureOutcome
    where
        I: IntoIterator<Item = T>,
        T: TouchSource,
    {
        let now = Instant::now();
        let analysis = self.analyze_touches(touches);
        let touch_count = analysis.active_count;
        let matches_required_fingers =
            self.count_matches_required_fingers(touch_count);
        self.required_touch_down = matches_required_fingers;

        if matches_required_fingers {
            self.click_rewrite_deadline = Some(now + click_rewrite_grace());
        }

        if touch_count == 0 {
            let outcome = self.finish_touch_sequence();
            if outcome == GestureOutcome::EmulateMiddleClick {
                self.click_rewrite_deadline = None;
            }
            return outcome;
        }

        if !self.config.tap_to_click {
            self.touch_start_time = None;
            self.start_centroid = None;
            self.latest_centroid = None;
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
        let click_eligible =
            self.required_touch_down || self.has_recent_click_touch();
        match kind {
            MouseEventKind::LeftDown | MouseEventKind::RightDown
                if click_eligible =>
            {
                self.rewritten_mouse_down_active = true;
                self.required_touch_down = false;
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

    fn analyze_touches<I, T>(&self, touches: I) -> TouchAnalysis
    where
        I: IntoIterator<Item = T>,
        T: TouchSource,
    {
        let mut active_count = 0usize;
        let mut sample_count = 0usize;
        let mut sum = Point::default();
        for touch in touches {
            if !touch.is_touching() {
                continue;
            }

            active_count += 1;
            if sample_count >= self.config.fingers {
                continue;
            }

            let (x, y) = touch.normalized_position();
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

        TouchAnalysis { active_count, centroid }
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

    fn config() -> Config {
        Config {
            fingers: 3,
            allow_more_fingers: false,
            max_distance_delta: 0.05,
            max_time_delta: Duration::from_millis(300),
            tap_to_click: true,
            ignored_app_bundles: Vec::new().into_boxed_slice(),
        }
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
        engine.handle_touch_frame([
            touch(0.1, 0.1),
            touch(0.2, 0.2),
            touch(0.3, 0.3),
        ]);

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
        engine.handle_touch_frame([
            touch(0.1, 0.1),
            touch(0.2, 0.2),
            touch(0.3, 0.3),
        ]);
        engine.handle_touch_frame([touch(0.1, 0.1), touch(0.2, 0.2)]);

        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftDown),
            MouseAction::RewriteDown
        );
    }

    #[test]
    fn does_not_rewrite_mouse_down_after_touch_grace_expires() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame([
            touch(0.1, 0.1),
            touch(0.2, 0.2),
            touch(0.3, 0.3),
        ]);
        engine.handle_touch_frame([touch(0.1, 0.1), touch(0.2, 0.2)]);
        std::thread::sleep(Duration::from_millis(100));

        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftDown),
            MouseAction::Pass
        );
    }

    #[test]
    fn emits_middle_click_for_valid_tap() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame([
            touch(0.1, 0.1),
            touch(0.2, 0.2),
            touch(0.3, 0.3),
        ]);
        engine.handle_touch_frame([
            touch(0.11, 0.1),
            touch(0.21, 0.2),
            touch(0.31, 0.3),
        ]);

        assert_eq!(
            engine.handle_touch_frame(std::iter::empty::<TouchContact>()),
            GestureOutcome::EmulateMiddleClick
        );
    }

    #[test]
    fn ignores_tap_when_touch_count_never_reaches_required_fingers() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame([touch(0.1, 0.1), touch(0.2, 0.2)]);

        assert_eq!(
            engine.handle_touch_frame(std::iter::empty::<TouchContact>()),
            GestureOutcome::None
        );
    }

    #[test]
    fn ignores_hover_touches_for_centroid_and_counting() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame([
            touch(0.1, 0.1),
            touch(0.2, 0.2),
            touch(0.3, 0.3),
            hover_touch(0.9, 0.9),
        ]);

        assert_eq!(
            engine.handle_touch_frame(std::iter::empty::<TouchContact>()),
            GestureOutcome::EmulateMiddleClick
        );
    }
}
