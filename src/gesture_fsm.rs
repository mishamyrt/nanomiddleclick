use std::time::{Duration, Instant};

use crate::config::Config;
use crate::ffi::{MouseAction, MouseEventKind, RawTouch};

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

    pub fn handle_touch_frame(&mut self, touches: &[RawTouch]) -> GestureOutcome {
        let now = Instant::now();
        let touch_count = self.active_touch_count(touches);
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
            self.touch_start_time = Some(Instant::now());
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

        let centroid = self.centroid_for_touch_subset(touches);
        if self.start_centroid.is_none() {
            self.start_centroid = Some(centroid);
        }
        self.latest_centroid = Some(centroid);
        GestureOutcome::None
    }

    pub fn handle_mouse_event(&mut self, kind: MouseEventKind) -> MouseAction {
        let click_eligible = self.required_touch_down || self.has_recent_click_touch();
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

    fn centroid_for_touch_subset(&self, touches: &[RawTouch]) -> Point {
        let sample_count = self
            .active_touch_count(touches)
            .min(self.config.fingers);
        let sample_count =
            u32::try_from(sample_count).expect("finger count should fit into u32");

        let mut sum = Point::default();
        for touch in touches
            .iter()
            .filter(|touch| is_touch_contact_stage(touch.stage))
            .take(self.config.fingers)
        {
            sum.x += f64::from(touch.normalized_vector.position.x);
            sum.y += f64::from(touch.normalized_vector.position.y);
        }

        Point {
            x: sum.x / f64::from(sample_count),
            y: sum.y / f64::from(sample_count),
        }
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

    fn active_touch_count(&self, touches: &[RawTouch]) -> usize {
        touches
            .iter()
            .filter(|touch| is_touch_contact_stage(touch.stage))
            .count()
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

fn is_touch_contact_stage(stage: i32) -> bool {
    (3..=5).contains(&stage)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOUCH_STAGE_MAKE_TOUCH: i32 = 3;
    const TOUCH_STAGE_HOVER_IN_RANGE: i32 = 2;

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

    fn touch(x: f32, y: f32) -> RawTouch {
        touch_with_stage(x, y, TOUCH_STAGE_MAKE_TOUCH)
    }

    fn hover_touch(x: f32, y: f32) -> RawTouch {
        touch_with_stage(x, y, TOUCH_STAGE_HOVER_IN_RANGE)
    }

    fn touch_with_stage(x: f32, y: f32, stage: i32) -> RawTouch {
        RawTouch {
            stage,
            normalized_vector: crate::ffi::RawVector {
                position: crate::ffi::RawPoint { x, y },
                velocity: crate::ffi::RawPoint { x: 0.0, y: 0.0 },
            },
            ..RawTouch::default()
        }
    }

    #[test]
    fn rewrites_mouse_down_and_up_when_required_fingers_are_down() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame(&[
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
        engine.handle_touch_frame(&[
            touch(0.1, 0.1),
            touch(0.2, 0.2),
            touch(0.3, 0.3),
        ]);
        engine.handle_touch_frame(&[touch(0.1, 0.1), touch(0.2, 0.2)]);

        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftDown),
            MouseAction::RewriteDown
        );
    }

    #[test]
    fn does_not_rewrite_mouse_down_after_touch_grace_expires() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame(&[
            touch(0.1, 0.1),
            touch(0.2, 0.2),
            touch(0.3, 0.3),
        ]);
        engine.handle_touch_frame(&[touch(0.1, 0.1), touch(0.2, 0.2)]);
        std::thread::sleep(Duration::from_millis(100));

        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftDown),
            MouseAction::Pass
        );
    }

    #[test]
    fn emits_middle_click_for_valid_tap() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame(&[
            touch(0.1, 0.1),
            touch(0.2, 0.2),
            touch(0.3, 0.3),
        ]);
        engine.handle_touch_frame(&[
            touch(0.11, 0.1),
            touch(0.21, 0.2),
            touch(0.31, 0.3),
        ]);

        assert_eq!(
            engine.handle_touch_frame(&[]),
            GestureOutcome::EmulateMiddleClick
        );
    }

    #[test]
    fn rejects_tap_when_touch_duration_times_out() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame(&[
            touch(0.1, 0.1),
            touch(0.2, 0.2),
            touch(0.3, 0.3),
        ]);
        std::thread::sleep(Duration::from_millis(350));

        assert_eq!(engine.handle_touch_frame(&[]), GestureOutcome::None);
    }

    #[test]
    fn rejects_tap_when_distance_is_too_large() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame(&[
            touch(0.1, 0.1),
            touch(0.2, 0.2),
            touch(0.3, 0.3),
        ]);
        engine.handle_touch_frame(&[
            touch(0.3, 0.3),
            touch(0.4, 0.4),
            touch(0.5, 0.5),
        ]);

        assert_eq!(engine.handle_touch_frame(&[]), GestureOutcome::None);
    }

    #[test]
    fn rejects_oversized_touch_set_when_allow_more_fingers_is_disabled() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame(&[
            touch(0.1, 0.1),
            touch(0.2, 0.2),
            touch(0.3, 0.3),
            touch(0.4, 0.4),
        ]);

        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftDown),
            MouseAction::Pass
        );
    }

    #[test]
    fn ignores_hovering_paths_when_counting_click_fingers() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame(&[
            touch(0.1, 0.1),
            touch(0.2, 0.2),
            touch(0.3, 0.3),
            hover_touch(0.4, 0.4),
        ]);

        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftDown),
            MouseAction::RewriteDown
        );
    }

    #[test]
    fn accepts_oversized_touch_set_when_allow_more_fingers_is_enabled() {
        let mut config = config();
        config.allow_more_fingers = true;
        let mut engine = GestureEngine::new(config);

        engine.handle_touch_frame(&[
            touch(0.10, 0.10),
            touch(0.20, 0.20),
            touch(0.30, 0.30),
            touch(0.90, 0.90),
        ]);
        engine.handle_touch_frame(&[
            touch(0.11, 0.10),
            touch(0.21, 0.20),
            touch(0.31, 0.30),
            touch(0.95, 0.95),
        ]);

        assert_eq!(
            engine.handle_touch_frame(&[]),
            GestureOutcome::EmulateMiddleClick
        );
    }

    #[test]
    fn suppresses_synthetic_tap_after_rewritten_natural_click() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame(&[
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

        engine.handle_touch_frame(&[
            touch(0.1, 0.1),
            touch(0.2, 0.2),
            touch(0.3, 0.3),
        ]);
        assert_eq!(engine.handle_touch_frame(&[]), GestureOutcome::None);
    }

    #[test]
    fn tap_path_stays_disabled_when_tap_to_click_is_off() {
        let mut config = config();
        config.tap_to_click = false;
        let mut engine = GestureEngine::new(config);

        engine.handle_touch_frame(&[
            touch(0.1, 0.1),
            touch(0.2, 0.2),
            touch(0.3, 0.3),
        ]);
        assert_eq!(engine.handle_touch_frame(&[]), GestureOutcome::None);
    }

    #[test]
    fn reset_for_ignored_app_clears_pending_rewrite_state() {
        let mut engine = GestureEngine::new(config());
        engine.handle_touch_frame(&[
            touch(0.1, 0.1),
            touch(0.2, 0.2),
            touch(0.3, 0.3),
        ]);
        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftDown),
            MouseAction::RewriteDown
        );

        engine.reset_for_ignored_app();

        assert_eq!(
            engine.handle_mouse_event(MouseEventKind::LeftUp),
            MouseAction::Pass
        );
    }
}
