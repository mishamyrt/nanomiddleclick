use nanomiddleclick_core::{Config, MouseClickMode};
use nanomiddleclick_preferences::Preferences;

pub(crate) const DEFAULTS_DOMAIN: &str = "co.myrt.nanomiddleclick";

const KEY_FINGERS: &str = "fingers";
const KEY_ALLOW_MORE_FINGERS: &str = "allowMoreFingers";
const KEY_MAX_DISTANCE_DELTA: &str = "maxDistanceDelta";
const KEY_MAX_TIME_DELTA: &str = "maxTimeDelta";
const KEY_TAP_TO_CLICK: &str = "tapToClick";
const KEY_MOUSE_CLICK_MODE: &str = "mouseClickMode";
const KEY_IGNORED_APP_BUNDLES: &str = "ignoredAppBundles";

pub(crate) fn load_config() -> Result<Config, String> {
    let preferences =
        Preferences::new(DEFAULTS_DOMAIN).map_err(|error| error.to_string())?;
    let system_tap_to_click = nanomiddleclick_preferences::system_tap_to_click();

    Ok(Config::from_raw_parts(
        preferences.get_i64(KEY_FINGERS, 3).map_err(|error| error.to_string())?,
        preferences
            .get_bool(KEY_ALLOW_MORE_FINGERS, false)
            .map_err(|error| error.to_string())?,
        preferences
            .get_f64(KEY_MAX_DISTANCE_DELTA, 0.05)
            .map_err(|error| error.to_string())?,
        preferences
            .get_i64(KEY_MAX_TIME_DELTA, 300)
            .map_err(|error| error.to_string())?,
        preferences
            .get_bool(KEY_TAP_TO_CLICK, system_tap_to_click)
            .map_err(|error| error.to_string())?,
        load_mouse_click_mode(&preferences)?,
        preferences
            .get_string_array(KEY_IGNORED_APP_BUNDLES)
            .map_err(|error| error.to_string())?,
    ))
}

fn load_mouse_click_mode(preferences: &Preferences) -> Result<u32, String> {
    if let Some(raw_value) = preferences
        .get_string(KEY_MOUSE_CLICK_MODE)
        .map_err(|error| error.to_string())?
    {
        if let Some(mode) = parse_mouse_click_mode(&raw_value) {
            return Ok(mode as u32);
        }
    }

    let raw_value = preferences
        .get_i64(KEY_MOUSE_CLICK_MODE, MouseClickMode::default() as i64)
        .map_err(|error| error.to_string())?;

    match u32::try_from(raw_value) {
        Ok(raw_value) if MouseClickMode::try_from_raw(raw_value).is_some() => {
            Ok(raw_value)
        }
        _ => Ok(MouseClickMode::default() as u32),
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
