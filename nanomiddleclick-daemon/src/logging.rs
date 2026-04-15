use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

#[allow(clippy::print_stderr)]
pub fn log(level: &str, message: fmt::Arguments<'_>) {
    let timestamp =
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

    eprintln!("[{timestamp}] {level}: {message}");
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::logging::log("INFO", format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::logging::log("WARN", format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::logging::log("ERROR", format_args!($($arg)*))
    };
}
