use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
}

impl LogLevel {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "error" => Some(Self::Error),
            "warn" => Some(Self::Warn),
            "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            _ => None,
        }
    }
}

fn level_cell() -> &'static AtomicU8 {
    static CELL: OnceLock<AtomicU8> = OnceLock::new();
    CELL.get_or_init(|| AtomicU8::new(LogLevel::Info as u8))
}

pub fn set_level(level: LogLevel) {
    level_cell().store(level as u8, Ordering::Relaxed);
}

pub fn enabled(level: LogLevel) -> bool {
    (level as u8) <= level_cell().load(Ordering::Relaxed)
}

pub fn error(message: &str) {
    if enabled(LogLevel::Error) {
        eprintln!("ERROR: {}", message);
    }
}

pub fn warn(message: &str) {
    if enabled(LogLevel::Warn) {
        eprintln!("Warning: {}", message);
    }
}

pub fn info(message: &str) {
    if enabled(LogLevel::Info) {
        println!("{}", message);
    }
}

pub fn debug(message: &str) {
    if enabled(LogLevel::Debug) {
        eprintln!("DEBUG: {}", message);
    }
}
