use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use nanomiddleclick_core::{
    Config, GestureEngine, GestureOutcome, MouseAction, MouseEventKind,
};
use nanomiddleclick_platform::{
    self as platform, EventHandler, SignalKind, SystemEventKind, TouchFrame,
};

pub struct App {
    engine: Mutex<GestureEngine>,
    frontmost_bundle: Mutex<Option<Box<str>>>,
    frontmost_bundle_ignored: AtomicBool,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            engine: Mutex::new(GestureEngine::new(config)),
            frontmost_bundle: Mutex::new(None),
            frontmost_bundle_ignored: AtomicBool::new(false),
        }
    }

    fn reload_config(&self) {
        match platform::load_config() {
            Ok(config) => {
                crate::log_info!("reloaded config: {config}");

                let frontmost_bundle = lock_or_recover(&self.frontmost_bundle);
                let mut engine = lock_or_recover(&self.engine);
                engine.update_config(config);

                let frontmost_bundle_ignored =
                    frontmost_bundle.as_deref().is_some_and(|bundle_id| {
                        engine.config().is_bundle_ignored(bundle_id)
                    });

                self.frontmost_bundle_ignored
                    .store(frontmost_bundle_ignored, Ordering::Relaxed);
            }
            Err(error) => {
                crate::log_error!("failed to reload config: {error}");
            }
        }
    }

    fn update_frontmost_bundle(&self, bundle_id: Option<&str>) {
        let mut frontmost_bundle = lock_or_recover(&self.frontmost_bundle);
        if frontmost_bundle.as_deref() == bundle_id {
            return;
        }

        let engine = lock_or_recover(&self.engine);
        let bundle_ignored = bundle_id
            .is_some_and(|bundle_id| engine.config().is_bundle_ignored(bundle_id));
        *frontmost_bundle = bundle_id.map(str::to_owned).map(String::into_boxed_str);
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

impl EventHandler for App {
    fn handle_touch_frame(&self, touches: TouchFrame<'_>) {
        if self.is_frontmost_bundle_ignored() {
            self.reset_for_ignored_app();
            return;
        }

        let outcome = {
            let mut engine = lock_or_recover(&self.engine);
            engine.handle_touch_frame(touches.source_kind(), touches.iter())
        };

        if let GestureOutcome::EmulateMiddleClick = outcome {
            crate::log_info!("emulating middle click from touch sequence");
            platform::post_middle_mouse_click();
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

    fn handle_system_event(&self, kind: SystemEventKind) {
        match kind {
            SystemEventKind::DeviceAdded => {
                crate::log_info!(
                    "multitouch device list changed; restarting listeners"
                );
            }
            SystemEventKind::Wake => {
                crate::log_info!("system woke up; restarting listeners");
            }
            SystemEventKind::DisplayReconfigured => {
                crate::log_info!(
                    "display configuration changed; restarting listeners"
                );
            }
        }

        if platform::restart_listeners() {
            crate::log_info!("listeners restarted successfully");
        } else {
            crate::log_warn!("listener restart completed in degraded mode");
        }
    }

    fn handle_signal(&self, kind: SignalKind) {
        match kind {
            SignalKind::Reload => {
                crate::log_info!("received SIGHUP; reloading config and listeners");
                self.reload_config();

                if platform::restart_listeners() {
                    crate::log_info!("listeners reloaded successfully");
                } else {
                    crate::log_warn!("listener reload completed in degraded mode");
                }
            }
        }
    }

    fn handle_frontmost_bundle_change(&self, bundle_id: Option<&str>) {
        self.update_frontmost_bundle(bundle_id);
    }
}

fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
