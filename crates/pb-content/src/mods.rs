//! Mod loading with path confinement and executable detection.
//! See SPEC-005 section 3 for the sandbox model.
#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;

use crate::error::ModError;
use crate::schema::Content;

/// Maximum number of files allowed in a mod.
pub const MAX_MOD_FILES: usize = 2048;

/// Maximum nesting depth for mod directory structures.
pub const MAX_NEST_DEPTH: u32 = 64;

/// Load a mod from a directory. Validates path confinement and rejects executables.
pub fn load_mod(root: &Path) -> Result<Content, ModError> {
    // Canonicalize the mod root to check path confinement
    let canonical_root = root.canonicalize().map_err(|e| {
        ModError::new(
            "E-MOD-PATH",
            format!("cannot access mod directory {}: {}", root.display(), e),
        )
    })?;

    // Walk all files in the mod directory
    walk_mod_files(&canonical_root, &canonical_root)?;

    // Try loading content from the mod directory
    let content = crate::load::load_all(root)
        .map_err(|e| ModError::new("E-MOD-PATH", format!("failed to load mod content: {}", e)))?;

    Ok(content)
}

/// Walk files in the mod tree, checking for path escapes and executables.
fn walk_mod_files(dir: &Path, root: &Path) -> Result<(), ModError> {
    for entry in fs::read_dir(dir)
        .map_err(|e| ModError::new("E-MOD-PATH", format!("cannot read mod directory: {}", e)))?
    {
        let entry = entry
            .map_err(|e| ModError::new("E-MOD-PATH", format!("directory entry error: {}", e)))?;
        let path = entry.path();

        // Check path confinement: path must be under the mod root
        let canonical = path.canonicalize().map_err(|e| {
            ModError::new(
                "E-MOD-PATH",
                format!("cannot resolve path {}: {}", path.display(), e),
            )
        })?;
        if !canonical.starts_with(root) {
            return Err(ModError::new(
                "E-MOD-PATH",
                format!("mod file {} escapes the mod root", path.display()),
            ));
        }

        if path.is_dir() {
            walk_mod_files(&path, root)?;
        } else if path.is_file() {
            // Check for executable files
            check_not_executable(&path)?;
        }
    }

    Ok(())
}

/// Check that a file is not an executable.
fn check_not_executable(path: &Path) -> Result<(), ModError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(path).map_err(|e| {
            ModError::new(
                "E-MOD-EXEC",
                format!("cannot read metadata for {}: {}", path.display(), e),
            )
        })?;
        let mode = metadata.permissions().mode();
        if mode & 0o111 != 0 {
            return Err(ModError::new(
                "E-MOD-EXEC",
                format!("file {} has execute permission bits set", path.display()),
            ));
        }
    }

    // Check for ELF, PE, or shebang magic bytes
    let data = fs::read(path).map_err(|e| {
        ModError::new(
            "E-MOD-EXEC",
            format!("cannot read {}: {}", path.display(), e),
        )
    })?;

    if data.len() >= 4 {
        // ELF: \x7fELF
        if data[0] == 0x7f && data[1] == b'E' && data[2] == b'L' && data[3] == b'F' {
            return Err(ModError::new(
                "E-MOD-EXEC",
                format!("file {} is an ELF executable", path.display()),
            ));
        }
        // PE: MZ
        if data[0] == b'M' && data[1] == b'Z' {
            return Err(ModError::new(
                "E-MOD-EXEC",
                format!("file {} is a PE executable", path.display()),
            ));
        }
    }
    if data.len() >= 2 && data[0] == b'#' && data[1] == b'!' {
        return Err(ModError::new(
            "E-MOD-EXEC",
            format!("file {} has a shebang line", path.display()),
        ));
    }

    Ok(())
}
