//! Adversarial tests for journal file parsing security trust boundaries (EP-006 M2).
//!
//! Tests that malformed journal files produce the correct named errors rather
//! than panics:
//!
//! - Lines with too few fields → `E-JOURNAL-PARSE`
//! - Unknown command names → `E-JOURNAL-ILLEGAL`
//! - Invalid numeric fields → `E-JOURNAL-PARSE`
//! - Missing required arguments → `E-JOURNAL-PARSE`

#![allow(clippy::expect_used)]

use std::io::Write;
use std::path::Path;

use pb_cli::journal::{parse_journal, JournalError};

/// Create a temporary journal file with the given content.
/// Uses a unique name based on the test function name (via #[test] attribute).
fn create_temp_journal(content: &str, test_name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pb_journal_test_{}", test_name));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("journal.txt");
    let mut f = std::fs::File::create(&path).expect("failed to create journal file");
    f.write_all(content.as_bytes())
        .expect("failed to write journal file");
    f.flush().expect("flush failed");
    path
}

fn cleanup_temp_journal(path: &Path) {
    let _ = std::fs::remove_file(path);
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}

#[test]
fn journal_too_few_fields_returns_parse_error() {
    let content = "1 2\n";
    let path = create_temp_journal(content, "too_few_fields");
    let result = parse_journal(&path);
    cleanup_temp_journal(&path);
    match result {
        Err(JournalError::Parse(msg)) => {
            assert!(
                msg.contains("expected at least 3 fields"),
                "unexpected parse message: {}",
                msg
            );
        }
        Err(other) => panic!("expected E-JOURNAL-PARSE, got: {:?}", other),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn journal_illegal_command_returns_illegal_error() {
    let content = "1 2 nocommand\n";
    let path = create_temp_journal(content, "illegal_cmd");
    let result = parse_journal(&path);
    cleanup_temp_journal(&path);
    match result {
        Err(JournalError::Illegal(msg)) => {
            assert!(
                msg.contains("nocommand") || msg.contains("unknown command"),
                "unexpected illegal message: {}",
                msg
            );
        }
        Err(other) => panic!("expected E-JOURNAL-ILLEGAL, got: {:?}", other),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn journal_invalid_tick_returns_parse_error() {
    let content = "abc 2 hold\n";
    let path = create_temp_journal(content, "invalid_tick");
    let result = parse_journal(&path);
    cleanup_temp_journal(&path);
    match result {
        Err(JournalError::Parse(msg)) => {
            assert!(
                msg.contains("invalid tick"),
                "unexpected parse message: {}",
                msg
            );
        }
        Err(other) => panic!("expected E-JOURNAL-PARSE, got: {:?}", other),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn journal_invalid_actor_id_returns_parse_error() {
    let content = "1 abc hold\n";
    let path = create_temp_journal(content, "invalid_actor");
    let result = parse_journal(&path);
    cleanup_temp_journal(&path);
    match result {
        Err(JournalError::Parse(msg)) => {
            assert!(
                msg.contains("invalid actor"),
                "unexpected parse message: {}",
                msg
            );
        }
        Err(other) => panic!("expected E-JOURNAL-PARSE, got: {:?}", other),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn journal_move_missing_args_returns_parse_error() {
    let content = "1 2 move\n";
    let path = create_temp_journal(content, "move_no_args");
    let result = parse_journal(&path);
    cleanup_temp_journal(&path);
    match result {
        Err(JournalError::Parse(msg)) => {
            assert!(
                msg.contains("x, y target") || msg.contains("requires"),
                "unexpected parse message: {}",
                msg
            );
        }
        Err(other) => panic!("expected E-JOURNAL-PARSE, got: {:?}", other),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn journal_empty_file_returns_empty_ok() {
    let content = "";
    let path = create_temp_journal(content, "empty");
    let result = parse_journal(&path);
    cleanup_temp_journal(&path);
    match result {
        Ok(entries) => {
            assert!(
                entries.is_empty(),
                "expected empty entries, got {} entries",
                entries.len()
            );
        }
        Err(e) => panic!("expected Ok for empty file, got: {:?}", e),
    }
}

#[test]
fn journal_comments_only_returns_empty_ok() {
    let content = "# This is a comment\n  # Another comment\n\n";
    let path = create_temp_journal(content, "comments_only");
    let result = parse_journal(&path);
    cleanup_temp_journal(&path);
    match result {
        Ok(entries) => {
            assert!(
                entries.is_empty(),
                "expected empty entries for comment-only file"
            );
        }
        Err(e) => panic!("expected Ok for comment-only file, got: {:?}", e),
    }
}
