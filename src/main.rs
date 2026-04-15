mod config;
mod ffi;
mod gesture_fsm;
mod logging;

use std::ffi::CStr;
use std::slice;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use config::Config;
use ffi::{MouseAction, MouseEventKind, SignalKind, SystemEventKind};
use gesture_fsm::{GestureEngine, GestureOutcome};

static APP: OnceLock<App> = OnceLock::new();

fn main() {
    let config = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            log_error!("failed to load config: {error}");
            Config::fallback()
        }
    };

    log_info!("starting nanomiddleclickd with domain {}", config::DEFAULTS_DOMAIN);
    log_info!("config: {}", config.describe());

    if !ffi::is_accessibility_trusted(false) {
        log_warn!(
            "Accessibility permission is not granted; click rewriting may stay inactive until permission is granted and listeners are reloaded"
        );
    }

    assert!(
        APP.set(App::new(config)).is_ok(),
        "APP should only be initialized once"
    );

    let listeners_active = ffi::start(
        touch_frame_callback,
        mouse_event_callback,
        system_event_callback,
        signal_callback,
        frontmost_bundle_callback,
    );

    if listeners_active {
        log_info!("listeners activated");
    } else {
        log_warn!("listeners started in degraded mode");
    }

    ffi::run_loop_run();
    ffi::stop();
    log_info!("nanomiddleclickd stopped");
}

struct App {
    engine: Mutex<GestureEngine>,
    frontmost_bundle: Mutex<Option<Box<str>>>,
    frontmost_bundle_ignored: AtomicBool,
}

impl App {
    fn new(config: Config) -> Self {
        Self {
            engine: Mutex::new(GestureEngine::new(config)),
            frontmost_bundle: Mutex::new(None),
            frontmost_bundle_ignored: AtomicBool::new(false),
        }
    }

    fn handle_touch_frame(&self, touches: &[ffi::RawTouch]) {
        if self.is_frontmost_bundle_ignored() {
            self.reset_for_ignored_app();
            return;
        }

        let outcome = {
            let mut engine = lock_or_recover(&self.engine);
            engine.handle_touch_frame(touches)
        };

        if let GestureOutcome::EmulateMiddleClick = outcome {
            log_info!("emulating middle click from touch sequence");
            ffi::post_middle_mouse_click();
        }
    }

    fn handle_mouse_event(&self, kind: MouseEventKind) -> MouseAction {
        if self.is_frontmost_bundle_ignored() {
            self.reset_for_ignored_app();
            return MouseAction::Pass;
        }

        let mut engine = lock_or_recover(&self.engine);
        engine.handle_mouse_event(kind)
    }

    fn handle_system_event(kind: SystemEventKind) {
        match kind {
            SystemEventKind::DeviceAdded => {
                log_info!("multitouch device list changed; restarting listeners");
            }
            SystemEventKind::Wake => {
                log_info!("system woke up; restarting listeners");
            }
            SystemEventKind::DisplayReconfigured => {
                log_info!("display configuration changed; restarting listeners");
            }
        }

        if ffi::restart_listeners() {
            log_info!("listeners restarted successfully");
        } else {
            log_warn!("listener restart completed in degraded mode");
        }
    }

    fn reload_config(&self) {
        match Config::load() {
            Ok(config) => {
                let frontmost_bundle = {
                    let frontmost_bundle = lock_or_recover(&self.frontmost_bundle);
                    frontmost_bundle.clone()
                };
                let frontmost_bundle_ignored = frontmost_bundle
                    .as_deref()
                    .is_some_and(|bundle_id| config.is_bundle_ignored(bundle_id));

                log_info!("reloaded config: {}", config.describe());

                let mut engine = lock_or_recover(&self.engine);
                engine.update_config(config);

                self.frontmost_bundle_ignored
                    .store(frontmost_bundle_ignored, Ordering::Relaxed);
            }
            Err(error) => {
                log_error!("failed to reload config: {error}");
            }
        }
    }

    fn handle_frontmost_bundle_change(&self, bundle_id: Option<Box<str>>) {
        let bundle_ignored = {
            let engine = lock_or_recover(&self.engine);
            bundle_id.as_deref().is_some_and(|bundle_id| {
                engine.config().is_bundle_ignored(bundle_id)
            })
        };

        let mut frontmost_bundle = lock_or_recover(&self.frontmost_bundle);
        *frontmost_bundle = bundle_id;
        self.frontmost_bundle_ignored.store(bundle_ignored, Ordering::Relaxed);
    }

    fn is_frontmost_bundle_ignored(&self) -> bool {
        self.frontmost_bundle_ignored.load(Ordering::Relaxed)
    }

    fn reset_for_ignored_app(&self) {
        let mut engine = lock_or_recover(&self.engine);
        engine.reset_for_ignored_app();
    }
}

extern "C" fn touch_frame_callback(
    touches: *const ffi::RawTouch,
    touch_count: usize,
    _timestamp: f64,
    _frame: i32,
) {
    let Some(app) = app() else {
        return;
    };

    let touches = if touches.is_null() || touch_count == 0 {
        &[]
    } else {
        unsafe { slice::from_raw_parts(touches, touch_count) }
    };

    app.handle_touch_frame(touches);
}

extern "C" fn mouse_event_callback(kind: u32) -> u32 {
    let Some(kind) = MouseEventKind::from_raw(kind) else {
        return MouseAction::Pass.as_raw();
    };

    app()
        .map(|app| app.handle_mouse_event(kind).as_raw())
        .unwrap_or_else(|| MouseAction::Pass.as_raw())
}

extern "C" fn system_event_callback(kind: u32) {
    let Some(kind) = SystemEventKind::from_raw(kind) else {
        return;
    };

    App::handle_system_event(kind);
}

extern "C" fn signal_callback(kind: u32) {
    let Some(kind) = SignalKind::from_raw(kind) else {
        return;
    };

    match kind {
        SignalKind::Reload => {
            log_info!("received SIGHUP; reloading config and listeners");
            if let Some(app) = app() {
                app.reload_config();
            }

            if ffi::restart_listeners() {
                log_info!("listeners reloaded successfully");
            } else {
                log_warn!("listener reload completed in degraded mode");
            }
        }
    }
}

extern "C" fn frontmost_bundle_callback(bundle_id: *const std::ffi::c_char) {
    let Some(app) = app() else {
        return;
    };

    let bundle_id = if bundle_id.is_null() {
        None
    } else {
        Some(
            unsafe { CStr::from_ptr(bundle_id) }
                .to_string_lossy()
                .into_owned()
                .into_boxed_str(),
        )
    };

    app.handle_frontmost_bundle_change(bundle_id);
}

fn app() -> Option<&'static App> {
    APP.get()
}

fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
