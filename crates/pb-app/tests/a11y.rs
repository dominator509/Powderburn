//! SPEC-004 section 4: the complete seven-check accessibility floor.
//!
//! This invokes the same real report implementation shipped through `pbcli
//! a11y-report`; any failed check returns an error and fails this integration
//! test.

#![allow(clippy::expect_used)]

#[test]
fn all_seven_accessibility_checks_pass() {
    pb_cli::cmd_a11y::run_a11y_report()
        .expect("color, text scale, keyboard, palette, flashing, subtitles, and slow-clock");
}
