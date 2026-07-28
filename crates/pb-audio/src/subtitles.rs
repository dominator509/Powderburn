//! Subtitle messages emitted by the cue bus.

use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subtitle {
    pub speaker: &'static str,
    pub text: &'static str,
}

#[derive(Debug)]
pub(crate) struct ActiveSubtitle {
    pub subtitle: Subtitle,
    pub started: Instant,
}

impl core::fmt::Display for Subtitle {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "[{}] {}", self.speaker, self.text)
    }
}
