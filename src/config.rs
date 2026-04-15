use std::fmt;
use std::time::Duration;

use crate::ffi;

pub const DEFAULTS_DOMAIN: &str = "co.myrt.nanomiddleclick";
const DEFAULT_FINGERS: usize = 3;
const DEFAULT_ALLOW_MORE_FINGERS: bool = false;
const DEFAULT_MAX_DISTANCE_DELTA: f64 = 0.05;
const DEFAULT_MAX_TIME_DELTA_MS: u64 = 300;

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub fingers: usize,
    pub allow_more_fingers: bool,
    pub max_distance_delta: f64,
    pub max_time_delta: Duration,
    pub tap_to_click: bool,
    pub ignored_app_bundles: Box<[Box<str>]>,
}

impl Config {
    pub fn load() -> Result<Self, String> {
        let snapshot = ffi::load_config_snapshot()?;

        let fingers = match usize::try_from(snapshot.fingers) {
            Ok(value) if value > 0 => value,
            _ => DEFAULT_FINGERS,
        };

        let max_distance_delta = if snapshot.max_distance_delta.is_finite()
            && snapshot.max_distance_delta >= 0.0
        {
            snapshot.max_distance_delta
        } else {
            DEFAULT_MAX_DISTANCE_DELTA
        };

        let max_time_delta_ms = match u64::try_from(snapshot.max_time_delta_ms) {
            Ok(value) if value > 0 => value,
            _ => DEFAULT_MAX_TIME_DELTA_MS,
        };

        Ok(Self {
            fingers,
            allow_more_fingers: snapshot.allow_more_fingers,
            max_distance_delta,
            max_time_delta: Duration::from_millis(max_time_delta_ms),
            tap_to_click: snapshot.tap_to_click,
            ignored_app_bundles: snapshot
                .ignored_app_bundles
                .into_iter()
                .map(String::into_boxed_str)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        })
    }

    pub fn fallback() -> Self {
        Self {
            fingers: DEFAULT_FINGERS,
            allow_more_fingers: DEFAULT_ALLOW_MORE_FINGERS,
            max_distance_delta: DEFAULT_MAX_DISTANCE_DELTA,
            max_time_delta: Duration::from_millis(DEFAULT_MAX_TIME_DELTA_MS),
            tap_to_click: ffi::system_tap_to_click(),
            ignored_app_bundles: Vec::new().into_boxed_slice(),
        }
    }

    pub fn describe(&self) -> String {
        format!(
            "fingers={}, allowMoreFingers={}, maxDistanceDelta={:.4}, maxTimeDeltaMs={}, tapToClick={}, ignoredAppBundles={}",
            self.fingers,
            self.allow_more_fingers,
            self.max_distance_delta,
            self.max_time_delta.as_millis(),
            self.tap_to_click,
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
