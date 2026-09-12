mod raw;

use std::ffi::CStr;
use std::sync::{Arc, OnceLock};

static HANDLER: OnceLock<Arc<dyn EventHandler>> = OnceLock::new();

pub trait EventHandler: Send + Sync {
    fn handle_wake(&self);
    fn handle_frontmost_bundle_change(&self, bundle_id: Option<&str>);
}

pub fn install_event_handler(
    handler: Arc<dyn EventHandler>,
) -> Result<(), &'static str> {
    HANDLER.set(handler).map_err(|_| "event handler already installed")
}

pub fn start(monitor_frontmost_bundle: bool) {
    raw::start(
        wake_callback,
        monitor_frontmost_bundle.then_some(frontmost_bundle_callback),
    );
}

pub fn set_frontmost_bundle_monitor_enabled(enabled: bool) {
    raw::set_frontmost_bundle_monitor_enabled(
        enabled.then_some(frontmost_bundle_callback),
    );
}

pub fn stop() {
    raw::stop();
}

extern "C" fn wake_callback() {
    if let Some(handler) = handler() {
        handler.handle_wake();
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
