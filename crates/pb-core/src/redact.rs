//! Crash report redaction utilities.
//!
//! Redacts sensitive information from crash reports before they are logged
//! or persisted. Specifically:
//!
//! - `redact_path` replaces the user's home directory with `<PB_HOME>` and any
//!   absolute path outside the install root with `<PATH>`.
//! - `redact_user` replaces the current system user name with `<USER>`.
//!
//! See SECURITY.md section 8 for the full redaction policy.

use std::path::Path;

/// Redact a filesystem path for crash reports.
///
/// If `path` starts with the user's home directory, that prefix is replaced
/// with `<PB_HOME>`. If `path` is an absolute path that does not start with
/// the home directory, the entire path is replaced with `<PATH>`.
/// Relative paths and empty strings are returned unchanged.
pub fn redact_path(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }

    // Get the home directory from the HOME env var
    let home = std::env::var("HOME").ok();

    let canonical = Path::new(path);

    if let Some(ref home_dir) = home {
        let home_path = Path::new(home_dir);
        if canonical.starts_with(home_path) {
            // Replace the home dir prefix with <PB_HOME>
            if let Ok(relative) = canonical.strip_prefix(home_path) {
                let relative_str = relative.to_string_lossy();
                if relative_str.is_empty() {
                    return "<PB_HOME>".to_string();
                }
                return format!("<PB_HOME>/{}", relative_str);
            }
        }
    }

    // If it's an absolute path and wasn't under home, redact fully
    if canonical.has_root() {
        return "<PATH>".to_string();
    }

    // Relative path — return as-is
    path.to_string()
}

/// Redact the current system user name from text.
///
/// Replaces all occurrences of the current user's name (as reported by
/// `std::env::var("USER")`) with `<USER>`.
pub fn redact_user(text: &str) -> String {
    let user = match std::env::var("USER").or_else(|_| std::env::var("USERNAME")) {
        Ok(u) => u,
        Err(_) => return text.to_string(),
    };

    if user.is_empty() {
        return text.to_string();
    }

    text.replace(&user, "<USER>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_redact_path_empty() {
        assert_eq!(redact_path(""), "");
    }

    #[test]
    fn test_redact_path_relative() {
        assert_eq!(redact_path("relative/path.ron"), "relative/path.ron");
        assert_eq!(redact_path("foo/bar"), "foo/bar");
    }

    #[test]
    fn test_redact_path_home() {
        let home = env::var("HOME").expect("HOME should be set in test");
        let test_path = format!("{}/content/rules/weapons.ron", home);
        let result = redact_path(&test_path);
        assert_eq!(result, "<PB_HOME>/content/rules/weapons.ron");
    }

    #[test]
    fn test_redact_path_home_exact() {
        let home = env::var("HOME").expect("HOME should be set in test");
        let result = redact_path(&home);
        assert_eq!(result, "<PB_HOME>");
    }

    #[test]
    fn test_redact_path_absolute_outside_home() {
        let result = redact_path("/etc/powderburn/config.ron");
        assert_eq!(result, "<PATH>");
    }

    #[test]
    fn test_redact_path_install_root() {
        let home = env::var("HOME").expect("HOME should be set in test");
        let install_path = format!("{}/powderburn/bin/powderburn", home);
        let result = redact_path(&install_path);
        assert_eq!(result, "<PB_HOME>/powderburn/bin/powderburn");
    }

    #[test]
    fn test_redact_user_basic() {
        env::set_var("USER", "testuser");
        let text = "Error: could not open config for user testuser";
        let result = redact_user(text);
        assert_eq!(result, "Error: could not open config for user <USER>");
    }

    #[test]
    fn test_redact_user_no_match() {
        env::set_var("USER", "testuser");
        let text = "Error: something went wrong without a username";
        let result = redact_user(text);
        assert_eq!(result, "Error: something went wrong without a username");
    }

    #[test]
    fn test_redact_user_multiple_occurrences() {
        env::set_var("USER", "alice");
        let text = "alice's home is /home/alice and alice's config is there";
        let result = redact_user(text);
        assert_eq!(
            result,
            "<USER>'s home is /home/<USER> and <USER>'s config is there"
        );
    }

    #[test]
    fn test_redact_user_no_env() {
        // Remove USER env to test fallback
        let original_user = env::var("USER").ok();
        env::remove_var("USER");
        env::remove_var("USERNAME");

        let text = "some text with user bob";
        let result = redact_user(text);
        // Without an env var, the text should be unchanged
        assert_eq!(result, "some text with user bob");

        // Restore
        if let Some(u) = original_user {
            env::set_var("USER", u);
        }
    }
}
