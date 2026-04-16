mod app;
mod logging;

use std::env;
use std::process::ExitCode;
use std::sync::Arc;

use app::App;
use nanomiddleclick_core::Config;
use nanomiddleclick_launchd as launchd;
use nanomiddleclick_platform as platform;

#[allow(clippy::print_stderr, clippy::print_stdout)]
fn main() -> ExitCode {
    match parse_args(env::args().skip(1)) {
        Ok(Command::Run { verbose }) => {
            logging::set_verbose(verbose);
            run_daemon();
            ExitCode::SUCCESS
        }
        Ok(Command::DaemonOn) => match env::current_exe() {
            Ok(executable_path) => match launchd::install(&executable_path) {
                Ok(plist_path) => {
                    println!("launchd agent installed at {}", plist_path.display());
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    log_error!("{error}");
                    ExitCode::FAILURE
                }
            },
            Err(error) => {
                log_error!("failed to resolve current executable path: {error}");
                ExitCode::FAILURE
            }
        },
        Ok(Command::DaemonOff) => match launchd::uninstall() {
            Ok(plist_path) => {
                println!("launchd agent removed from {}", plist_path.display());
                ExitCode::SUCCESS
            }
            Err(error) => {
                log_error!("{error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run_daemon() {
    let config = match platform::load_config() {
        Ok(config) => config,
        Err(error) => {
            log_error!("failed to load config: {error}");
            Config::fallback(platform::system_tap_to_click())
        }
    };

    log_info!("starting nanomiddleclick with domain {}", platform::DEFAULTS_DOMAIN);
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
    log_info!("nanomiddleclick stopped");
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Run { verbose: bool },
    DaemonOn,
    DaemonOff,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let args = args.into_iter().collect::<Vec<_>>();
    match args.as_slice() {
        [] => Ok(Command::Run { verbose: false }),
        [flag] if flag == "-v" => Ok(Command::Run { verbose: true }),
        [command, state] if command == "daemon" => {
            if state == "on" {
                Ok(Command::DaemonOn)
            } else if state == "off" {
                Ok(Command::DaemonOff)
            } else {
                Err(usage().to_owned())
            }
        }
        _ => Err(usage().to_owned()),
    }
}

fn usage() -> &'static str {
    "usage: nanomiddleclick [-v] | nanomiddleclick daemon <on|off>"
}

#[cfg(test)]
mod tests {
    use super::{Command, parse_args};

    #[test]
    fn parse_args_defaults_to_running_daemon() {
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
    fn parse_args_supports_daemon_on() {
        assert_eq!(
            parse_args(vec!["daemon".to_owned(), "on".to_owned()]),
            Ok(Command::DaemonOn)
        );
    }

    #[test]
    fn parse_args_supports_daemon_off() {
        assert_eq!(
            parse_args(vec!["daemon".to_owned(), "off".to_owned()]),
            Ok(Command::DaemonOff)
        );
    }

    #[test]
    fn parse_args_rejects_invalid_combinations() {
        assert!(parse_args(vec!["daemon".to_owned()]).is_err());
        assert!(
            parse_args(vec!["daemon".to_owned(), "on".to_owned(), "-v".to_owned()])
                .is_err()
        );
        assert!(
            parse_args(vec!["-v".to_owned(), "daemon".to_owned(), "on".to_owned()])
                .is_err()
        );
    }
}
