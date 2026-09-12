use nanomiddleclick_core::{Config, MouseClickMode};
use prefs::{Key, Preferences};

pub(crate) const DEFAULTS_DOMAIN: &str = "co.myrt.nanomiddleclick";

const KEY_FINGERS: Key<i64> = Key::new("fingers");
const KEY_ALLOW_MORE_FINGERS: Key<bool> = Key::new("allowMoreFingers");
const KEY_MAX_DISTANCE_DELTA: Key<f64> = Key::new("maxDistanceDelta");
const KEY_MAX_TIME_DELTA: Key<i64> = Key::new("maxTimeDelta");
const KEY_TAP_TO_CLICK: Key<bool> = Key::new("tapToClick");
const KEY_MOUSE_CLICK_MODE: Key<String> = Key::new("mouseClickMode");
const KEY_MOUSE_CLICK_MODE_NUMBER: Key<i64> = Key::new("mouseClickMode");
const KEY_IGNORED_APP_BUNDLES: Key<Vec<String>> = Key::new("ignoredAppBundles");

pub(crate) fn load_config() -> Config {
    let preferences = Preferences::new(DEFAULTS_DOMAIN)
        .expect("static preferences domain contains no null bytes");
    let trackpad =
        Preferences::new("com.apple.driver.AppleBluetoothMultitouch.trackpad")
            .expect("static trackpad domain contains no null bytes");
    let system_tap_to_click =
        trackpad.get_or(Key::new("Clicking"), false).unwrap_or(false);

    load_config_from(&preferences, system_tap_to_click)
}

fn load_config_from(preferences: &Preferences, system_tap_to_click: bool) -> Config {
    Config::from_raw_parts(
        preferences.get_or(KEY_FINGERS, 3).unwrap_or(3),
        preferences.get_or(KEY_ALLOW_MORE_FINGERS, false).unwrap_or(false),
        preferences.get_or(KEY_MAX_DISTANCE_DELTA, 0.05).unwrap_or(0.05),
        preferences.get_or(KEY_MAX_TIME_DELTA, 300).unwrap_or(300),
        preferences
            .get_or(KEY_TAP_TO_CLICK, system_tap_to_click)
            .unwrap_or(system_tap_to_click),
        load_mouse_click_mode(preferences),
        preferences
            .get_or(KEY_IGNORED_APP_BUNDLES, Vec::new())
            .unwrap_or_default()
            .into_iter()
            .map(String::into_boxed_str)
            .collect(),
    )
}

fn load_mouse_click_mode(preferences: &Preferences) -> u32 {
    if let Ok(Some(raw_value)) = preferences.get(KEY_MOUSE_CLICK_MODE) {
        return parse_mouse_click_mode(&raw_value).unwrap_or_default() as u32;
    }

    let raw_value = preferences
        .get_or(KEY_MOUSE_CLICK_MODE_NUMBER, MouseClickMode::default() as i64)
        .unwrap_or(MouseClickMode::default() as i64);

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
        value.trim_start().parse().ok().and_then(MouseClickMode::try_from_raw)
    }
}

#[cfg(test)]
mod tests {
    use super::{load_config_from, load_mouse_click_mode};
    use nanomiddleclick_core::{Config, MouseClickMode};
    use prefs::{Key, Preferences};
    use std::time::Duration;

    #[test]
    fn loads_existing_preferences_and_defaults() {
        let preferences = Preferences::new(&format!(
            "co.myrt.nanomiddleclick.tests.config.{}",
            std::process::id()
        ))
        .unwrap();
        let defaults = load_config_from(&preferences, false);
        assert!(load_config_from(&preferences, true).tap_to_click);

        preferences.set(Key::new("fingers"), &4_i64).unwrap();
        preferences.set(Key::new("allowMoreFingers"), &true).unwrap();
        preferences.set(Key::new("maxDistanceDelta"), &0.03_f64).unwrap();
        preferences.set(Key::new("maxTimeDelta"), &150_i64).unwrap();
        preferences.set(Key::new("tapToClick"), &false).unwrap();
        preferences
            .set(Key::new("mouseClickMode"), &String::from("threeFinger"))
            .unwrap();
        preferences
            .set(
                Key::new("ignoredAppBundles"),
                &vec![
                    String::from("com.apple.finder"),
                    String::from("com.apple.Terminal"),
                ],
            )
            .unwrap();
        let configured = load_config_from(&preferences, true);

        preferences.set(Key::new("fingers"), &String::from("4")).unwrap();
        preferences.set(Key::new("allowMoreFingers"), &String::from("YES")).unwrap();
        preferences
            .set(Key::new("maxDistanceDelta"), &String::from("0.03"))
            .unwrap();
        preferences.set(Key::new("maxTimeDelta"), &String::from("150")).unwrap();
        preferences.set(Key::new("tapToClick"), &String::from("no")).unwrap();
        let string_values = load_config_from(&preferences, true);

        for name in [
            "fingers",
            "allowMoreFingers",
            "maxDistanceDelta",
            "maxTimeDelta",
            "tapToClick",
            "mouseClickMode",
            "ignoredAppBundles",
        ] {
            preferences.remove(Key::<String>::new(name)).unwrap();
        }

        assert_eq!(
            defaults,
            Config {
                fingers: 3,
                allow_more_fingers: false,
                max_distance_delta: 0.05,
                max_time_delta: Duration::from_millis(300),
                tap_to_click: false,
                mouse_click_mode: MouseClickMode::Center,
                ignored_app_bundles: Box::default(),
            }
        );
        assert_eq!(
            configured,
            Config {
                fingers: 4,
                allow_more_fingers: true,
                max_distance_delta: 0.03,
                max_time_delta: Duration::from_millis(150),
                tap_to_click: false,
                mouse_click_mode: MouseClickMode::ThreeFinger,
                ignored_app_bundles: vec![
                    "com.apple.finder".into(),
                    "com.apple.Terminal".into()
                ]
                .into(),
            }
        );
        assert_eq!(string_values, configured);
    }

    #[test]
    fn preserves_mouse_click_modes_and_invalid_value_fallback() {
        let preferences = Preferences::new(&format!(
            "co.myrt.nanomiddleclick.tests.mouse-mode.{}",
            std::process::id()
        ))
        .unwrap();

        for (value, expected) in [
            ("CENTER", MouseClickMode::Center),
            ("ThReEfInGeR", MouseClickMode::ThreeFinger),
            ("disabled", MouseClickMode::Disabled),
            ("0", MouseClickMode::ThreeFinger),
            ("1", MouseClickMode::Center),
            ("  +2", MouseClickMode::Disabled),
            ("invalid", MouseClickMode::Center),
            ("", MouseClickMode::Center),
            ("-1", MouseClickMode::Center),
            ("3", MouseClickMode::Center),
            ("4294967296", MouseClickMode::Center),
        ] {
            let key = Key::new("mouseClickMode");
            preferences.set(key, &value.to_owned()).unwrap();
            let actual = load_mouse_click_mode(&preferences);
            preferences.remove(key).unwrap();
            assert_eq!(actual, expected as u32, "{value:?}");
        }

        for (value, expected) in [
            (0_i64, MouseClickMode::ThreeFinger),
            (1, MouseClickMode::Center),
            (2, MouseClickMode::Disabled),
            (-1, MouseClickMode::Center),
            (3, MouseClickMode::Center),
            (i64::MAX, MouseClickMode::Center),
        ] {
            let key = Key::new("mouseClickMode");
            preferences.set(key, &value).unwrap();
            let actual = load_mouse_click_mode(&preferences);
            preferences.remove(key).unwrap();
            assert_eq!(actual, expected as u32, "{value}");
        }
    }
}
