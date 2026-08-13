//! Error type shared across backends.

use std::fmt;

/// Fallible result for [`crate::GitRepo`] methods.
pub type Result<T> = std::result::Result<T, Error>;

/// Backend-agnostic error.
#[derive(Debug)]
pub struct Error {
    message: String,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for Error {}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Self { message }
    }
}

impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::new(err.to_string())
    }
}

/// `gix::init` rarely fails after `create_dir_all`; exercised via `From` in unit tests.
#[cfg(not(target_arch = "wasm32"))]
impl From<gix::init::Error> for Error {
    fn from(err: gix::init::Error) -> Self {
        Self::new(err.to_string())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<gix::open::Error> for Error {
    fn from(err: gix::open::Error) -> Self {
        Self::new(err.to_string())
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
