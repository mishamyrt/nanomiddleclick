use std::ffi::{CStr, CString, c_char, c_void};
use std::fmt;

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
pub struct RawTouch {
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
    ignored_app_bundles: *mut *mut c_char,
    ignored_app_bundles_len: usize,
}

#[derive(Debug, PartialEq)]
pub struct ConfigSnapshot {
    pub fingers: i64,
    pub allow_more_fingers: bool,
    pub max_distance_delta: f64,
    pub max_time_delta_ms: i64,
    pub tap_to_click: bool,
    pub ignored_app_bundles: Vec<String>,
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

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemEventKind {
    DeviceAdded = 1,
    Wake = 2,
    DisplayReconfigured = 3,
}

impl SystemEventKind {
    pub fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            1 => Some(Self::DeviceAdded),
            2 => Some(Self::Wake),
            3 => Some(Self::DisplayReconfigured),
            _ => None,
        }
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignalKind {
    Reload = 1,
}

impl SignalKind {
    pub fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            1 => Some(Self::Reload),
            _ => None,
        }
    }
}

pub type TouchFrameCallback = extern "C" fn(
    touches: *const RawTouch,
    touch_count: usize,
    timestamp: f64,
    frame: i32,
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

pub fn is_accessibility_trusted(prompt: bool) -> bool {
    unsafe { nmc_is_accessibility_trusted(prompt) }
}

pub fn system_tap_to_click() -> bool {
    unsafe { nmc_get_system_tap_to_click() }
}

pub fn load_config_snapshot() -> Result<ConfigSnapshot, String> {
    let mut raw = RawConfigSnapshot {
        fingers: 0,
        allow_more_fingers: false,
        max_distance_delta: 0.0,
        max_time_delta_ms: 0,
        tap_to_click: false,
        ignored_app_bundles: std::ptr::null_mut(),
        ignored_app_bundles_len: 0,
    };

    if !unsafe { nmc_load_config(&raw mut raw) } {
        return Err("shim returned no config snapshot".to_string());
    }

    let snapshot = unsafe { ConfigSnapshot::from_raw(&raw) };
    unsafe { nmc_free_config(&raw mut raw) };
    Ok(snapshot)
}

impl ConfigSnapshot {
    unsafe fn from_raw(raw: &RawConfigSnapshot) -> Self {
        let mut ignored_app_bundles =
            Vec::with_capacity(raw.ignored_app_bundles_len);

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
                    unsafe { CStr::from_ptr(*value) }.to_string_lossy().into_owned(),
                );
            }
        }

        Self {
            fingers: raw.fingers,
            allow_more_fingers: raw.allow_more_fingers,
            max_distance_delta: raw.max_distance_delta,
            max_time_delta_ms: raw.max_time_delta_ms,
            tap_to_click: raw.tap_to_click,
            ignored_app_bundles,
        }
    }
}

pub fn start(
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

pub fn restart_listeners() -> bool {
    unsafe { nmc_restart_listeners() }
}

pub fn stop() {
    unsafe { nmc_stop() }
}

pub fn run_loop_run() {
    unsafe { nmc_run_loop_run() }
}

pub fn post_middle_mouse_click() {
    unsafe { nmc_post_middle_mouse_click() }
}

impl fmt::Display for ConfigSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ConfigSnapshot {{ fingers: {}, allow_more_fingers: {}, max_distance_delta: {}, max_time_delta_ms: {}, tap_to_click: {}, ignored_app_bundles: {} }}",
            self.fingers,
            self.allow_more_fingers,
            self.max_distance_delta,
            self.max_time_delta_ms,
            self.tap_to_click,
            self.ignored_app_bundles.len(),
        )
    }
}

#[allow(dead_code)]
pub fn make_c_string(value: &str) -> CString {
    CString::new(value).expect("strings passed over FFI must not contain NUL bytes")
}

#[allow(dead_code)]
pub type OpaquePointer = *mut c_void;
