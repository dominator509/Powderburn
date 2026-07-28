//! Mod loading with path confinement and executable detection.
//! See SPEC-005 section 3 for the sandbox model.
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::ModError;
use crate::schema::Content;

/// Maximum number of files allowed in a mod.
pub const MAX_MOD_FILES: usize = 2048;

/// Maximum nesting depth for mod directory structures.
pub const MAX_NEST_DEPTH: u32 = 64;

/// Data-only mod manifest stored as `mod.ron` at the mod root.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ModManifest {
    pub id: String,
    #[serde(default)]
    pub load_order: i32,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

/// Fully loaded, validated data-mod set.
#[derive(Debug, Clone)]
pub struct LoadedModSet {
    pub content: Content,
    /// Deterministic application order after dependency resolution.
    pub load_order: Vec<String>,
}

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
    let mut file_count = 0usize;
    walk_mod_files(&canonical_root, &canonical_root, 0, &mut file_count)?;

    // Try loading content from the mod directory
    let content = crate::load::load_all(root)
        .map_err(|e| ModError::new("E-MOD-PATH", format!("failed to load mod content: {}", e)))?;

    Ok(content)
}

/// Load a selected set of mods, resolve dependencies deterministically, merge
/// them over shipped content, and validate the complete candidate atomically.
///
/// No caller-visible content is changed on failure. A bad mod is therefore
/// disabled as a unit instead of leaving partially applied records behind.
pub fn load_mod_set(
    shipped: &Content,
    mods_root: &Path,
    enabled: &[String],
) -> Result<LoadedModSet, ModError> {
    if enabled.len() > MAX_MOD_FILES {
        return Err(ModError::new(
            "E-MOD-PATH",
            "enabled mod count exceeds safety limit",
        ));
    }

    let enabled_set: BTreeSet<&str> = enabled.iter().map(String::as_str).collect();
    if enabled_set.len() != enabled.len() {
        return Err(ModError::new(
            "E-MOD-PATH",
            "enabled mod identifiers must be unique",
        ));
    }

    let mut pending = BTreeMap::<String, (ModManifest, PathBuf, Content)>::new();
    for requested_id in enabled {
        validate_mod_id(requested_id)?;
        let root = mods_root.join(requested_id);
        let manifest = load_manifest(&root)?;
        if manifest.id != *requested_id {
            return Err(ModError::new(
                "E-MOD-PATH",
                format!(
                    "mod directory `{requested_id}` declares mismatched id `{}`",
                    manifest.id
                ),
            ));
        }
        if manifest
            .dependencies
            .iter()
            .any(|dependency| !enabled_set.contains(dependency.as_str()))
        {
            let missing = manifest
                .dependencies
                .iter()
                .filter(|dependency| !enabled_set.contains(dependency.as_str()))
                .cloned()
                .collect::<Vec<_>>()
                .join(",");
            return Err(ModError::new(
                "E-MOD-PATH",
                format!("mod `{requested_id}` has disabled or missing dependencies: {missing}"),
            ));
        }
        let content = load_mod(&root)?;
        pending.insert(requested_id.clone(), (manifest, root, content));
    }

    let fixed_nodes: BTreeSet<String> = shipped
        .campaign_nodes
        .values()
        .filter(|node| node.historical_tag.as_deref() == Some("HISTORICAL_FIXED"))
        .map(|node| node.id.clone())
        .collect();
    let fixed_scenarios: BTreeSet<String> = shipped
        .campaign_nodes
        .values()
        .filter(|node| node.historical_tag.as_deref() == Some("HISTORICAL_FIXED"))
        .filter_map(|node| node.scenario_id.clone())
        .collect();

    let mut candidate = shipped.clone();
    let mut applied = BTreeSet::<String>::new();
    let mut order = Vec::new();
    while !pending.is_empty() {
        let next = pending
            .iter()
            .filter(|(_, (manifest, _, _))| {
                manifest
                    .dependencies
                    .iter()
                    .all(|dependency| applied.contains(dependency))
            })
            .min_by_key(|(id, (manifest, _, _))| (manifest.load_order, (*id).clone()))
            .map(|(id, _)| id.clone());
        let Some(id) = next else {
            return Err(ModError::new(
                "E-MOD-PATH",
                "mod dependency graph contains a cycle",
            ));
        };
        let Some((manifest, _root, content)) = pending.remove(&id) else {
            return Err(ModError::new(
                "E-MOD-PATH",
                "mod dependency resolver lost a selected mod",
            ));
        };
        reject_historical_overrides(&content, &fixed_nodes, &fixed_scenarios)?;
        merge_content(&mut candidate, content);
        applied.insert(manifest.id.clone());
        order.push(manifest.id);
    }

    let diagnostics = crate::validate::validate(&candidate);
    if let Some(first) = diagnostics.first() {
        return Err(ModError::new(
            "E-CONTENT-002",
            format!(
                "combined mod content is invalid ({}): {}",
                first.code, first.message
            ),
        ));
    }

    Ok(LoadedModSet {
        content: candidate,
        load_order: order,
    })
}

fn validate_mod_id(id: &str) -> Result<(), ModError> {
    let valid = !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
    if valid {
        Ok(())
    } else {
        Err(ModError::new(
            "E-MOD-PATH",
            format!("invalid mod identifier `{id}`"),
        ))
    }
}

fn load_manifest(root: &Path) -> Result<ModManifest, ModError> {
    let path = root.join("mod.ron");
    let metadata = fs::metadata(&path).map_err(|error| {
        ModError::new(
            "E-MOD-PATH",
            format!("cannot read mod manifest {}: {error}", path.display()),
        )
    })?;
    if metadata.len() > crate::load::MAX_CONTENT_FILE_BYTES {
        return Err(ModError::new(
            "E-MOD-PATH",
            format!("mod manifest {} exceeds size limit", path.display()),
        ));
    }
    let text = fs::read_to_string(&path).map_err(|error| {
        ModError::new(
            "E-MOD-PATH",
            format!("cannot read mod manifest {}: {error}", path.display()),
        )
    })?;
    let mut manifest: ModManifest = ron::from_str(&text).map_err(|error| {
        ModError::new(
            "E-MOD-PATH",
            format!("invalid mod manifest {}: {error}", path.display()),
        )
    })?;
    validate_mod_id(&manifest.id)?;
    manifest.dependencies.sort();
    manifest.dependencies.dedup();
    if manifest.dependencies.iter().any(|id| id == &manifest.id) {
        return Err(ModError::new(
            "E-MOD-PATH",
            format!("mod `{}` depends on itself", manifest.id),
        ));
    }
    for dependency in &manifest.dependencies {
        validate_mod_id(dependency)?;
    }
    Ok(manifest)
}

fn reject_historical_overrides(
    content: &Content,
    fixed_nodes: &BTreeSet<String>,
    fixed_scenarios: &BTreeSet<String>,
) -> Result<(), ModError> {
    if let Some(id) = content
        .campaign_nodes
        .keys()
        .find(|id| fixed_nodes.contains(*id))
    {
        return Err(ModError::new(
            "E-HIST-001",
            format!("mod attempts to override historical-fixed campaign node `{id}`"),
        ));
    }
    if let Some(id) = content
        .scenarios
        .keys()
        .find(|id| fixed_scenarios.contains(*id))
    {
        return Err(ModError::new(
            "E-HIST-001",
            format!("mod attempts to override historical-fixed scenario `{id}`"),
        ));
    }
    Ok(())
}

fn merge_content(target: &mut Content, source: Content) {
    target.scenarios.extend(source.scenarios);
    target.weapons.extend(source.weapons);
    target.companions.extend(source.companions);
    target.campaign_nodes.extend(source.campaign_nodes);
    target.dialogue.extend(source.dialogue);
    target.ledger_line_sets.extend(source.ledger_line_sets);
    target.items.extend(source.items);
    target.marks.extend(source.marks);
    target.ways.extend(source.ways);
    target.factions.extend(source.factions);
    if source.tables.is_some() {
        target.tables = source.tables;
    }
}

/// Walk files in the mod tree, checking for path escapes and executables.
fn walk_mod_files(
    dir: &Path,
    root: &Path,
    depth: u32,
    file_count: &mut usize,
) -> Result<(), ModError> {
    if depth > MAX_NEST_DEPTH {
        return Err(ModError::new(
            "E-MOD-PATH",
            format!("mod directory nesting depth {depth} exceeds limit {MAX_NEST_DEPTH}"),
        ));
    }

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
            walk_mod_files(&path, root, depth + 1, file_count)?;
        } else if path.is_file() {
            *file_count = file_count.saturating_add(1);
            if *file_count > MAX_MOD_FILES {
                return Err(ModError::new(
                    "E-MOD-PATH",
                    format!("mod file count exceeds limit {MAX_MOD_FILES}"),
                ));
            }
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
