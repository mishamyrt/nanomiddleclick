use std::ffi::CStr;

use nanomiddleclick_core::{Config, MouseClickMode};
use nanomiddleclick_preferences::Preferences;

pub(crate) const DEFAULTS_DOMAIN: &CStr = c"co.myrt.nanomiddleclick";

const KEY_FINGERS: &CStr = c"fingers";
const KEY_ALLOW_MORE_FINGERS: &CStr = c"allowMoreFingers";
const KEY_MAX_DISTANCE_DELTA: &CStr = c"maxDistanceDelta";
const KEY_MAX_TIME_DELTA: &CStr = c"maxTimeDelta";
const KEY_TAP_TO_CLICK: &CStr = c"tapToClick";
const KEY_MOUSE_CLICK_MODE: &CStr = c"mouseClickMode";
const KEY_IGNORED_APP_BUNDLES: &CStr = c"ignoredAppBundles";

pub(crate) fn load_config() -> Config {
    let preferences = Preferences::new(DEFAULTS_DOMAIN);
    let system_tap_to_click = nanomiddleclick_preferences::system_tap_to_click();

    Config::from_raw_parts(
        preferences.get_i64(KEY_FINGERS, 3),
        preferences.get_bool(KEY_ALLOW_MORE_FINGERS, false),
        preferences.get_f64(KEY_MAX_DISTANCE_DELTA, 0.05),
        preferences.get_i64(KEY_MAX_TIME_DELTA, 300),
        preferences.get_bool(KEY_TAP_TO_CLICK, system_tap_to_click),
        load_mouse_click_mode(&preferences),
        preferences.get_string_array(KEY_IGNORED_APP_BUNDLES),
    )
}

fn load_mouse_click_mode(preferences: &Preferences<'_>) -> u32 {
    if let Some(raw_value) = preferences.get_string(KEY_MOUSE_CLICK_MODE) {
        if let Some(mode) = parse_mouse_click_mode(&raw_value) {
            return mode as u32;
        }
    }

    let raw_value =
        preferences.get_i64(KEY_MOUSE_CLICK_MODE, MouseClickMode::default() as i64);

    match u32::try_from(raw_value) {
        Ok(raw_value) if MouseClickMode::try_from_raw(raw_value).is_some() => {
            raw_value
        }
        _ => MouseClickMode::default() as u32,
    }
}

fn parse_mouse_click_mode(value: &str) -> Option<MouseClickMode> {
    if value.eq_ignore_ascii_case("center") {
        Some(MouseClickMode::Center)
    } else if value.eq_ignore_ascii_case("disabled") {
        Some(MouseClickMode::Disabled)
    } else if value.eq_ignore_ascii_case("threefinger") {
        Some(MouseClickMode::ThreeFinger)
    } else {
        None
    }
}
