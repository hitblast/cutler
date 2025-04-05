/// Color constants.
pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";

/// Log levels for printing messages.
#[derive(PartialEq)]
pub enum LogLevel {
    Success,
    Error,
    Warning,
    Info,
}

/// Central logging function.
pub fn print_log(level: LogLevel, message: &str) {
    match level {
        LogLevel::Success => println!("{}[SUCCESS]{} {}", GREEN, RESET, message),
        LogLevel::Error => eprintln!("{}[ERROR]{} {}", RED, RESET, message),
        LogLevel::Warning => eprintln!("{}[WARN]{} {}", YELLOW, RESET, message),
        LogLevel::Info => println!("{}[INFO]{} {}", BOLD, RESET, message),
    }
}
