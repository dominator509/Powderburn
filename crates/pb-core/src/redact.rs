//! Crash report redaction utilities.
//!
//! Redacts sensitive information from crash reports before they are logged
//! or persisted. Specifically:
//!
//! Callers supply the permitted root and user token so this kernel crate never
//! reads process environment or filesystem state.
//!
//! See SECURITY.md section 8 for the full redaction policy.

use std::path::Path;

/// Redact a filesystem path for crash reports.
///
/// If `path` starts with the user's home directory, that prefix is replaced
/// with `<PB_HOME>`. If `path` is an absolute path that does not start with
/// the home directory, the entire path is replaced with `<PATH>`.
/// Relative paths and empty strings are returned unchanged.
pub fn redact_path(path: &str, permitted_root: Option<&Path>) -> String {
    if path.is_empty() {
        return String::new();
    }

    let canonical = Path::new(path);

    if let Some(root) = permitted_root {
        if canonical.starts_with(root) {
            if let Ok(relative) = canonical.strip_prefix(root) {
                let relative_str = relative.to_string_lossy();
                if relative_str.is_empty() {
                    return "<permitted>".to_string();
                }
                return format!("<permitted>/{}", relative_str);
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
pub fn redact_user(text: &str, user: Option<&str>) -> String {
    let Some(user) = user.filter(|user| !user.is_empty()) else {
        return text.to_string();
    };
    text.replace(user, "<USER>")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    #[test]
    fn test_redact_path_empty() {
        assert_eq!(redact_path("", None), "");
    }

    #[test]
    fn test_redact_path_relative() {
        assert_eq!(redact_path("relative/path.ron", None), "relative/path.ron");
        assert_eq!(redact_path("foo/bar", None), "foo/bar");
    }

    #[test]
    fn test_redact_path_home() {
        let root = Path::new("/home/test");
        let result = redact_path("/home/test/content/rules/weapons.ron", Some(root));
        assert_eq!(result, "<permitted>/content/rules/weapons.ron");
    }

    #[test]
    fn test_redact_path_home_exact() {
        let root = Path::new("/home/test");
        let result = redact_path("/home/test", Some(root));
        assert_eq!(result, "<permitted>");
    }

    #[test]
    fn test_redact_path_absolute_outside_home() {
        let result = redact_path("/etc/powderburn/config.ron", Some(Path::new("/game")));
        assert_eq!(result, "<PATH>");
    }

    #[test]
    fn test_redact_path_install_root() {
        let result = redact_path("/game/bin/powderburn", Some(Path::new("/game")));
        assert_eq!(result, "<permitted>/bin/powderburn");
    }

    #[test]
    fn test_redact_user_basic() {
        let text = "Error: could not open config for user testuser";
        let result = redact_user(text, Some("testuser"));
        assert_eq!(result, "Error: could not open config for user <USER>");
    }

    #[test]
    fn test_redact_user_no_match() {
        let text = "Error: something went wrong without a username";
        let result = redact_user(text, Some("testuser"));
        assert_eq!(result, "Error: something went wrong without a username");
    }

    #[test]
    fn test_redact_user_multiple_occurrences() {
        let text = "alice's home is /home/alice and alice's config is there";
        let result = redact_user(text, Some("alice"));
        assert_eq!(
            result,
            "<USER>'s home is /home/<USER> and <USER>'s config is there"
        );
    }

    #[test]
    fn test_redact_user_no_env() {
        let text = "some text with user bob";
        let result = redact_user(text, None);
        assert_eq!(result, "some text with user bob");
    }
}
