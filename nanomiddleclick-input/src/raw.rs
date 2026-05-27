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

#[link(name = "nanomiddleclick_input_shim", kind = "static")]
unsafe extern "C" {
    fn nmc_is_accessibility_trusted(prompt: bool) -> bool;
    fn nmc_start(
        touch_callback: TouchFrameCallback,
        mouse_callback: MouseEventCallback,
        system_callback: SystemEventCallback,
        signal_callback: SignalEventCallback,
    ) -> bool;
    fn nmc_restart_listeners() -> bool;
    fn nmc_stop();
    fn nmc_run_loop_run();
    fn nmc_post_middle_mouse_click();
}

pub(crate) fn is_accessibility_trusted(prompt: bool) -> bool {
    unsafe { nmc_is_accessibility_trusted(prompt) }
}

pub(crate) fn start(
    touch_callback: TouchFrameCallback,
    mouse_callback: MouseEventCallback,
    system_callback: SystemEventCallback,
    signal_callback: SignalEventCallback,
) -> bool {
    unsafe {
        nmc_start(touch_callback, mouse_callback, system_callback, signal_callback)
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
