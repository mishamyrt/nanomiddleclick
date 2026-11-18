mod app;
mod logging;
mod settings;

use std::env;
use std::process::ExitCode;
use std::sync::Arc;

use app::App;

#[allow(clippy::print_stderr, clippy::print_stdout)]
fn main() -> ExitCode {
    match parse_args(env::args().skip(1)) {
        Ok(Command::Run { verbose }) => {
            logging::set_verbose(verbose);
            run();
            ExitCode::SUCCESS
        }
        Ok(Command::Version) => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() {
    let config = settings::load_config();

    log_info!("starting nanomiddleclick with domain {}", settings::DEFAULTS_DOMAIN);
    log_debug!("config: {config}");
    let monitor_frontmost_bundle = !config.ignored_app_bundles.is_empty();

    if !nanomiddleclick_input::is_accessibility_trusted() {
        log_warn!(
            "Accessibility permission is not granted; click rewriting may stay inactive until permission is granted and listeners are reloaded"
        );
    }

    let app = Arc::new(App::new(config));
    assert!(
        nanomiddleclick_input::install_event_handler(app.clone()).is_ok(),
        "input event handler should only be initialized once"
    );
    assert!(
        nanomiddleclick_app_monitor::install_event_handler(app).is_ok(),
        "app monitor event handler should only be initialized once"
    );

    nanomiddleclick_app_monitor::start(monitor_frontmost_bundle);

    let listeners_active = nanomiddleclick_input::start();
    if listeners_active {
        log_info!("listeners activated");
    } else {
        log_warn!("listeners started in degraded mode");
    }

    nanomiddleclick_input::run_loop_run();
    nanomiddleclick_app_monitor::stop();
    nanomiddleclick_input::stop();
    log_info!("nanomiddleclick stopped");
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Run { verbose: bool },
    Version,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let args = args.into_iter().collect::<Vec<_>>();
    match args.as_slice() {
        [] => Ok(Command::Run { verbose: false }),
        [flag] if flag == "-v" => Ok(Command::Run { verbose: true }),
        [command] if command == "version" => Ok(Command::Version),
        _ => Err(usage().to_owned()),
    }
}

fn usage() -> &'static str {
    "usage: nanomiddleclick [-v] | nanomiddleclick version"
}

#[cfg(test)]
mod tests {
    use super::{Command, parse_args};

    #[test]
    fn parse_args_defaults_to_foreground() {
        assert_eq!(
            parse_args(Vec::<String>::new()),
            Ok(Command::Run { verbose: false })
        );
    }

    #[test]
    fn parse_args_supports_verbose_flag() {
        assert_eq!(
            parse_args(vec!["-v".to_owned()]),
            Ok(Command::Run { verbose: true })
        );
    }

    #[test]
    fn parse_args_supports_version() {
        assert_eq!(parse_args(vec!["version".to_owned()]), Ok(Command::Version));
    }

    #[test]
    fn parse_args_rejects_removed_daemon_commands() {
        for state in ["on", "off"] {
            assert!(
                parse_args(vec!["daemon".to_owned(), state.to_owned()]).is_err()
            );
        }
    }

    #[test]
    fn parse_args_rejects_invalid_combinations() {
        assert!(parse_args(vec!["version".to_owned(), "-v".to_owned()]).is_err());
    }
}
