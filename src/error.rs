use std::fmt::Display;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    Parse(String),
}

impl ConfigError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    pub fn io(error: std::io::Error, context: impl Display) -> Self {
        Self::Io(format!("{context}: {error}"))
    }

    pub fn parse(error: impl Display, context: impl Display) -> Self {
        Self::Parse(format!("{context}: {error}"))
    }
}

impl From<anyhow::Error> for ConfigError {
    fn from(value: anyhow::Error) -> Self {
        Self::Validation(value.to_string())
    }
}
