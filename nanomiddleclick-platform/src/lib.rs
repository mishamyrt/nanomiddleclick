mod raw;

use std::ffi::CStr;
use std::slice;
use std::sync::{Arc, OnceLock};

use nanomiddleclick_core::{
    Config, MouseAction, MouseEventKind, TouchDeviceKind, TouchSource,
};

pub const DEFAULTS_DOMAIN: &str = "co.myrt.nanomiddleclick";

static HANDLER: OnceLock<Arc<dyn EventHandler>> = OnceLock::new();

#[derive(Clone, Copy, Debug)]
pub struct TouchFrame<'a> {
    raw: &'a [raw::RawTouch],
    source_kind: TouchDeviceKind,
}

impl<'a> TouchFrame<'a> {
    fn new(raw: &'a [raw::RawTouch], source_kind: TouchDeviceKind) -> Self {
        Self { raw, source_kind }
    }

    pub fn iter(self) -> impl Iterator<Item = Touch<'a>> {
        self.raw.iter().map(Touch)
    }

    pub fn source_kind(self) -> TouchDeviceKind {
        self.source_kind
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Touch<'a>(&'a raw::RawTouch);

impl TouchSource for Touch<'_> {
    fn is_touching(&self) -> bool {
        (3..=5).contains(&self.0.stage)
    }

    fn normalized_position(&self) -> (f32, f32) {
        (self.0.normalized_vector.position.x, self.0.normalized_vector.position.y)
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
    fn from_raw(raw: u32) -> Option<Self> {
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
    fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            1 => Some(Self::Reload),
            _ => None,
        }
    }
}

pub trait EventHandler: Send + Sync {
    fn handle_touch_frame(&self, touches: TouchFrame<'_>);
    fn handle_mouse_event(&self, kind: MouseEventKind) -> MouseAction;
    fn handle_system_event(&self, kind: SystemEventKind);
    fn handle_signal(&self, kind: SignalKind);
    fn handle_frontmost_bundle_change(&self, bundle_id: Option<&str>);
}

pub fn install_event_handler(
    handler: Arc<dyn EventHandler>,
) -> Result<(), &'static str> {
    HANDLER.set(handler).map_err(|_| "event handler already installed")
}

pub fn load_config() -> Result<Config, String> {
    raw::load_config()
}

pub fn is_accessibility_trusted(prompt: bool) -> bool {
    raw::is_accessibility_trusted(prompt)
}

pub fn system_tap_to_click() -> bool {
    raw::system_tap_to_click()
}

pub fn start() -> bool {
    raw::start(
        touch_frame_callback,
        mouse_event_callback,
        system_event_callback,
        signal_callback,
        frontmost_bundle_callback,
    )
}

pub fn restart_listeners() -> bool {
    raw::restart_listeners()
}

pub fn stop() {
    raw::stop();
}

pub fn run_loop_run() {
    raw::run_loop_run();
}

pub fn post_middle_mouse_click() {
    raw::post_middle_mouse_click();
}

extern "C" fn touch_frame_callback(
    touches: *const raw::RawTouch,
    touch_count: usize,
    _timestamp: f64,
    _frame: i32,
    source_kind: u32,
) {
    let Some(handler) = handler() else {
        return;
    };

    let source_kind =
        TouchDeviceKind::from_raw(source_kind).unwrap_or(TouchDeviceKind::Unknown);
    let touches = if touches.is_null() || touch_count == 0 {
        TouchFrame::new(&[], source_kind)
    } else {
        TouchFrame::new(
            unsafe { slice::from_raw_parts(touches, touch_count) },
            source_kind,
        )
    };

    handler.handle_touch_frame(touches);
}

extern "C" fn mouse_event_callback(kind: u32) -> u32 {
    let Some(kind) = MouseEventKind::from_raw(kind) else {
        return MouseAction::Pass.as_raw();
    };

    handler()
        .map(|handler| handler.handle_mouse_event(kind).as_raw())
        .unwrap_or_else(|| MouseAction::Pass.as_raw())
}

extern "C" fn system_event_callback(kind: u32) {
    let Some(kind) = SystemEventKind::from_raw(kind) else {
        return;
    };

    if let Some(handler) = handler() {
        handler.handle_system_event(kind);
    }
}

extern "C" fn signal_callback(kind: u32) {
    let Some(kind) = SignalKind::from_raw(kind) else {
        return;
    };

    if let Some(handler) = handler() {
        handler.handle_signal(kind);
    }
}

extern "C" fn frontmost_bundle_callback(bundle_id: *const std::ffi::c_char) {
    let Some(handler) = handler() else {
        return;
    };

    if bundle_id.is_null() {
        handler.handle_frontmost_bundle_change(None);
        return;
    }

    let bundle_id = unsafe { CStr::from_ptr(bundle_id) }.to_string_lossy();
    handler.handle_frontmost_bundle_change(Some(bundle_id.as_ref()));
}

fn handler() -> Option<&'static Arc<dyn EventHandler>> {
    HANDLER.get()
}
