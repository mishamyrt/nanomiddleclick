mod app;
mod logging;

use std::sync::Arc;

use app::App;
use nanomiddleclick_core::Config;
use nanomiddleclick_platform as platform;

fn main() {
    let config = match platform::load_config() {
        Ok(config) => config,
        Err(error) => {
            log_error!("failed to load config: {error}");
            Config::fallback(platform::system_tap_to_click())
        }
    };

    log_info!("starting nanomiddleclickd with domain {}", platform::DEFAULTS_DOMAIN);
    log_info!("config: {config}");

    if !platform::is_accessibility_trusted(false) {
        log_warn!(
            "Accessibility permission is not granted; click rewriting may stay inactive until permission is granted and listeners are reloaded"
        );
    }

    assert!(
        platform::install_event_handler(Arc::new(App::new(config))).is_ok(),
        "event handler should only be initialized once"
    );

    let listeners_active = platform::start();
    if listeners_active {
        log_info!("listeners activated");
    } else {
        log_warn!("listeners started in degraded mode");
    }

    platform::run_loop_run();
    platform::stop();
    log_info!("nanomiddleclickd stopped");
}
