use std::ffi::c_char;

pub type EventCallback = extern "C" fn(kind: u32);
pub type FrontmostBundleCallback = extern "C" fn(bundle_id: *const c_char);

#[link(name = "nanomiddleclick_app_monitor_shim", kind = "static")]
unsafe extern "C" {
    fn NMCStartWorkspaceMonitor(
        event_callback: EventCallback,
        frontmost_bundle_callback: Option<FrontmostBundleCallback>,
    );
    fn NMCSetFrontmostBundleMonitorEnabled(
        frontmost_bundle_callback: Option<FrontmostBundleCallback>,
    );
    fn NMCStopWorkspaceMonitor();
}

pub(crate) fn start(
    event_callback: EventCallback,
    frontmost_bundle_callback: Option<FrontmostBundleCallback>,
) {
    unsafe { NMCStartWorkspaceMonitor(event_callback, frontmost_bundle_callback) }
}

pub(crate) fn set_frontmost_bundle_monitor_enabled(
    frontmost_bundle_callback: Option<FrontmostBundleCallback>,
) {
    unsafe { NMCSetFrontmostBundleMonitorEnabled(frontmost_bundle_callback) }
}

pub(crate) fn stop() {
    unsafe { NMCStopWorkspaceMonitor() }
}
