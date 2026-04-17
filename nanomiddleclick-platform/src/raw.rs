use std::ffi::{CStr, c_char};
use std::ptr;

use nanomiddleclick_core::Config;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RawPoint {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RawVector {
    pub position: RawPoint,
    pub velocity: RawPoint,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RawTouch {
    pub frame: i32,
    pub timestamp: f64,
    pub identifier: i32,
    pub stage: i32,
    pub finger_id: i32,
    pub hand_id: i32,
    pub normalized_vector: RawVector,
    pub total: f32,
    pub pressure: f32,
    pub angle: f32,
    pub major_axis: f32,
    pub minor_axis: f32,
    pub absolute_vector: RawVector,
    pub unknown14: i32,
    pub unknown15: i32,
    pub density: f32,
}

#[repr(C)]
struct RawConfigSnapshot {
    fingers: i64,
    allow_more_fingers: bool,
    max_distance_delta: f64,
    max_time_delta_ms: i64,
    tap_to_click: bool,
    mouse_click_mode: u32,
    ignored_app_bundles: *mut *mut c_char,
    ignored_app_bundles_len: usize,
}

pub type TouchFrameCallback = extern "C" fn(
    touches: *const RawTouch,
    touch_count: usize,
    timestamp: f64,
    frame: i32,
    source_kind: u32,
);
pub type MouseEventCallback = extern "C" fn(kind: u32) -> u32;
pub type SystemEventCallback = extern "C" fn(kind: u32);
pub type SignalEventCallback = extern "C" fn(kind: u32);
pub type FrontmostBundleCallback = extern "C" fn(bundle_id: *const c_char);

#[link(name = "nanomiddleclick_shim", kind = "static")]
unsafe extern "C" {
    fn nmc_is_accessibility_trusted(prompt: bool) -> bool;
    fn nmc_get_system_tap_to_click() -> bool;
    fn nmc_load_config(out_snapshot: *mut RawConfigSnapshot) -> bool;
    fn nmc_free_config(snapshot: *mut RawConfigSnapshot);
    fn nmc_start(
        touch_callback: TouchFrameCallback,
        mouse_callback: MouseEventCallback,
        system_callback: SystemEventCallback,
        signal_callback: SignalEventCallback,
        frontmost_bundle_callback: FrontmostBundleCallback,
    ) -> bool;
    fn nmc_restart_listeners() -> bool;
    fn nmc_stop();
    fn nmc_run_loop_run();
    fn nmc_post_middle_mouse_click();
}

pub(crate) fn is_accessibility_trusted(prompt: bool) -> bool {
    unsafe { nmc_is_accessibility_trusted(prompt) }
}

pub(crate) fn system_tap_to_click() -> bool {
    unsafe { nmc_get_system_tap_to_click() }
}

pub(crate) fn load_config() -> Result<Config, String> {
    let mut raw = RawConfigSnapshot {
        fingers: 0,
        allow_more_fingers: false,
        max_distance_delta: 0.0,
        max_time_delta_ms: 0,
        tap_to_click: false,
        mouse_click_mode: 0,
        ignored_app_bundles: ptr::null_mut(),
        ignored_app_bundles_len: 0,
    };
    let raw_ptr = ptr::addr_of_mut!(raw);

    if !unsafe { nmc_load_config(raw_ptr) } {
        return Err("shim returned no config snapshot".to_string());
    }

    let config = unsafe { config_from_raw(&raw) };
    unsafe { nmc_free_config(raw_ptr) };
    Ok(config)
}

unsafe fn config_from_raw(raw: &RawConfigSnapshot) -> Config {
    let mut ignored_app_bundles = Vec::with_capacity(raw.ignored_app_bundles_len);

    if !raw.ignored_app_bundles.is_null() && raw.ignored_app_bundles_len > 0 {
        let values = unsafe {
            std::slice::from_raw_parts(
                raw.ignored_app_bundles,
                raw.ignored_app_bundles_len,
            )
        };

        for value in values {
            if value.is_null() {
                continue;
            }

            ignored_app_bundles.push(
                unsafe { CStr::from_ptr(*value) }
                    .to_string_lossy()
                    .into_owned()
                    .into_boxed_str(),
            );
        }
    }

    Config::from_raw_parts(
        raw.fingers,
        raw.allow_more_fingers,
        raw.max_distance_delta,
        raw.max_time_delta_ms,
        raw.tap_to_click,
        raw.mouse_click_mode,
        ignored_app_bundles.into_boxed_slice(),
    )
}

pub(crate) fn start(
    touch_callback: TouchFrameCallback,
    mouse_callback: MouseEventCallback,
    system_callback: SystemEventCallback,
    signal_callback: SignalEventCallback,
    frontmost_bundle_callback: FrontmostBundleCallback,
) -> bool {
    unsafe {
        nmc_start(
            touch_callback,
            mouse_callback,
            system_callback,
            signal_callback,
            frontmost_bundle_callback,
        )
    }
}

pub(crate) fn restart_listeners() -> bool {
    unsafe { nmc_restart_listeners() }
}

pub(crate) fn stop() {
    unsafe { nmc_stop() }
}

pub(crate) fn run_loop_run() {
    unsafe { nmc_run_loop_run() }
}

pub(crate) fn post_middle_mouse_click() {
    unsafe { nmc_post_middle_mouse_click() }
}
