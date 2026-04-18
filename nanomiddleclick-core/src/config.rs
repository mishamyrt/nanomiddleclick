use std::fmt;
use std::time::Duration;

const DEFAULT_FINGERS: usize = 3;
const DEFAULT_ALLOW_MORE_FINGERS: bool = false;
const DEFAULT_MAX_DISTANCE_DELTA: f64 = 0.05;
const DEFAULT_MAX_TIME_DELTA_MS: u64 = 300;

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MouseClickMode {
    ThreeFinger = 0,
    #[default]
    Center = 1,
    Disabled = 2,
}

impl MouseClickMode {
    pub fn try_from_raw(raw: u32) -> Option<Self> {
        match raw {
            0 => Some(Self::ThreeFinger),
            1 => Some(Self::Center),
            2 => Some(Self::Disabled),
            _ => None,
        }
    }

    pub fn from_raw(raw: u32) -> Self {
        Self::try_from_raw(raw).unwrap_or_default()
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ThreeFinger => "threeFinger",
            Self::Center => "center",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub fingers: usize,
    pub allow_more_fingers: bool,
    pub max_distance_delta: f64,
    pub max_time_delta: Duration,
    pub tap_to_click: bool,
    pub mouse_click_mode: MouseClickMode,
    pub ignored_app_bundles: Box<[Box<str>]>,
}

impl Config {
    pub fn from_raw_parts(
        fingers: i64,
        allow_more_fingers: bool,
        max_distance_delta: f64,
        max_time_delta_ms: i64,
        tap_to_click: bool,
        mouse_click_mode: u32,
        ignored_app_bundles: Box<[Box<str>]>,
    ) -> Self {
        let fingers = match usize::try_from(fingers) {
            Ok(value) if value > 0 => value,
            _ => DEFAULT_FINGERS,
        };

        let max_distance_delta =
            if max_distance_delta.is_finite() && max_distance_delta >= 0.0 {
                max_distance_delta
            } else {
                DEFAULT_MAX_DISTANCE_DELTA
            };

        let max_time_delta_ms = match u64::try_from(max_time_delta_ms) {
            Ok(value) if value > 0 => value,
            _ => DEFAULT_MAX_TIME_DELTA_MS,
        };

        Self {
            fingers,
            allow_more_fingers,
            max_distance_delta,
            max_time_delta: Duration::from_millis(max_time_delta_ms),
            tap_to_click,
            mouse_click_mode: MouseClickMode::from_raw(mouse_click_mode),
            ignored_app_bundles,
        }
    }

    pub fn fallback(system_tap_to_click: bool) -> Self {
        Self {
            fingers: DEFAULT_FINGERS,
            allow_more_fingers: DEFAULT_ALLOW_MORE_FINGERS,
            max_distance_delta: DEFAULT_MAX_DISTANCE_DELTA,
            max_time_delta: Duration::from_millis(DEFAULT_MAX_TIME_DELTA_MS),
            tap_to_click: system_tap_to_click,
            mouse_click_mode: MouseClickMode::default(),
            ignored_app_bundles: Vec::new().into_boxed_slice(),
        }
    }

    pub fn describe(&self) -> String {
        format!(
            "fingers={}, allowMoreFingers={}, maxDistanceDelta={:.4}, maxTimeDeltaMs={}, tapToClick={}, mouseClickMode={}, ignoredAppBundles={}",
            self.fingers,
            self.allow_more_fingers,
            self.max_distance_delta,
            self.max_time_delta.as_millis(),
            self.tap_to_click,
            self.mouse_click_mode.as_str(),
            self.ignored_app_bundles.len(),
        )
    }

    pub fn is_bundle_ignored(&self, bundle_id: &str) -> bool {
        self.ignored_app_bundles
            .iter()
            .any(|ignored_bundle| ignored_bundle.as_ref() == bundle_id)
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.describe())
    }
}

#[cfg(test)]
mod tests {
    use super::MouseClickMode;

    #[test]
    fn invalid_mouse_click_mode_falls_back_to_documented_default() {
        assert_eq!(MouseClickMode::from_raw(u32::MAX), MouseClickMode::Center);
    }

    #[test]
    fn valid_mouse_click_modes_are_preserved() {
        assert_eq!(MouseClickMode::from_raw(0), MouseClickMode::ThreeFinger);
        assert_eq!(MouseClickMode::from_raw(1), MouseClickMode::Center);
        assert_eq!(MouseClickMode::from_raw(2), MouseClickMode::Disabled);
    }
}
