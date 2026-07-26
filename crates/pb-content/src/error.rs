use std::fmt;

/// Content loading error.
#[derive(Debug, Clone)]
pub struct ContentError {
    pub code: &'static str,
    pub message: String,
}

impl ContentError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ContentError {}

/// Mod loading error.
#[derive(Debug, Clone)]
pub struct ModError {
    pub code: &'static str,
    pub message: String,
}

impl ModError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for ModError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ModError {}
