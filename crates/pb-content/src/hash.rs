//! Canonical ruleset and content hashing from SPEC-002 section 8.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Hash the sorted byte contents of every regular file below `content/rules`.
pub fn ruleset_hash(content_root: &Path) -> io::Result<[u8; 32]> {
    hash_files(&content_root.join("rules"), |_| true)
}

/// Hash the sorted byte contents of every regular file below `content`, except
/// `BIBLIOGRAPHY.md`, which is documentation rather than executable content.
pub fn content_hash(content_root: &Path) -> io::Result<[u8; 32]> {
    hash_files(content_root, |relative| {
        relative != Path::new("BIBLIOGRAPHY.md")
    })
}

/// Lower-case hexadecimal representation used by save containers.
pub fn hex(hash: &[u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in hash {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn hash_files(root: &Path, include: impl Fn(&Path) -> bool) -> io::Result<[u8; 32]> {
    let mut paths = Vec::new();
    collect_files(root, root, &include, &mut paths)?;
    paths.sort();

    let mut bytes = Vec::new();
    for path in paths {
        bytes.extend_from_slice(&fs::read(path)?);
    }
    Ok(pb_core::hash::hash_state(&bytes))
}

fn collect_files(
    root: &Path,
    directory: &Path,
    include: &impl Fn(&Path) -> bool,
    output: &mut Vec<PathBuf>,
) -> io::Result<()> {
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_dir() {
            collect_files(root, &path, include, output)?;
        } else if file_type.is_file() {
            let relative = path.strip_prefix(root).map_err(io::Error::other)?;
            if include(relative) {
                output.push(path);
            }
        }
    }
    Ok(())
}
