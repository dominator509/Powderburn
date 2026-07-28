use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::error::ContentError;
use crate::schema::*;
use crate::validate::Diagnostic;

/// Maximum content file size in bytes.
pub const MAX_CONTENT_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// Maximum records per content file.
pub const MAX_RECORDS_PER_FILE: usize = 8192;

/// Maximum nesting depth for content structures.
pub const MAX_NEST_DEPTH: u32 = 64;

/// Load all content from the content directory tree.
pub fn load_all(root: &Path) -> Result<Content, ContentError> {
    let mut content = Content::default();

    load_dialogue(root, &mut content)?;

    let ledger_lines_path = root.join("rules").join("ledger_lines.ron");
    if ledger_lines_path.exists() {
        let line_sets: Vec<LedgerLineSetData> = load_ron_file(&ledger_lines_path)?;
        for line_set in line_sets {
            content
                .ledger_line_sets
                .insert(line_set.id.clone(), line_set);
        }
    }

    // Load weapons from content/rules/weapons.ron
    let weapons_path = root.join("rules").join("weapons.ron");
    if weapons_path.exists() {
        let weapons_list: Vec<WeaponData> = load_ron_file(&weapons_path)?;
        for w in weapons_list {
            content.weapons.insert(w.id.clone(), w);
        }
    }

    // Load scenarios from content/scenarios/*.ron
    let scenarios_dir = root.join("scenarios");
    if scenarios_dir.is_dir() {
        for entry in fs::read_dir(&scenarios_dir).map_err(|e| {
            ContentError::new("E-CONTENT-001", format!("cannot read scenarios dir: {}", e))
        })? {
            let entry = entry.map_err(|e| {
                ContentError::new("E-CONTENT-001", format!("dir entry error: {}", e))
            })?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "ron") {
                let scenario: ScenarioData = load_single_ron(&path)?;
                content.scenarios.insert(scenario.id.clone(), scenario);
            }
        }
    }

    // Load companions from content/companions/*.ron
    let companions_dir = root.join("companions");
    if companions_dir.is_dir() {
        for entry in fs::read_dir(&companions_dir).map_err(|e| {
            ContentError::new(
                "E-CONTENT-001",
                format!("cannot read companions dir: {}", e),
            )
        })? {
            let entry = entry.map_err(|e| {
                ContentError::new("E-CONTENT-001", format!("dir entry error: {}", e))
            })?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "ron") {
                let companions: Vec<CompanionData> = load_ron_file(&path)?;
                for c in companions {
                    content.companions.insert(c.id.clone(), c);
                }
            }
        }
    }

    // Load campaign nodes from content/campaign/*.ron
    let campaign_dir = root.join("campaign");
    if campaign_dir.is_dir() {
        for entry in fs::read_dir(&campaign_dir).map_err(|e| {
            ContentError::new("E-CONTENT-001", format!("cannot read campaign dir: {}", e))
        })? {
            let entry = entry.map_err(|e| {
                ContentError::new("E-CONTENT-001", format!("dir entry error: {}", e))
            })?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "ron") {
                let nodes: Vec<CampaignNodeData> = load_ron_file(&path)?;
                for n in nodes {
                    content.campaign_nodes.insert(n.id.clone(), n);
                }
            }
        }
    }

    // Load items from content/rules/items.ron
    let items_path = root.join("rules").join("items.ron");
    if items_path.exists() {
        let items_list: Vec<ItemData> = load_ron_file(&items_path)?;
        for item in items_list {
            content.items.insert(item.id.clone(), item);
        }
    }

    // Load marks from content/rules/marks.ron
    let marks_path = root.join("rules").join("marks.ron");
    if marks_path.exists() {
        let marks_list: Vec<MarkData> = load_ron_file(&marks_path)?;
        for m in marks_list {
            content.marks.insert(m.id.clone(), m);
        }
    }

    // Load ways from content/rules/ways.ron
    let ways_path = root.join("rules").join("ways.ron");
    if ways_path.exists() {
        let ways_list: Vec<WayData> = load_ron_file(&ways_path)?;
        for w in ways_list {
            content.ways.insert(w.id.clone(), w);
        }
    }

    // Load factions from content/rules/factions.ron
    let factions_path = root.join("rules").join("factions.ron");
    if factions_path.exists() {
        let factions_list: Vec<FactionData> = load_ron_file(&factions_path)?;
        for f in factions_list {
            content.factions.insert(f.id.clone(), f);
        }
    }

    // Load tables from content/rules/tables.ron (single record)
    let tables_path = root.join("rules").join("tables.ron");
    if tables_path.exists() {
        content.tables = Some(load_single_ron(&tables_path)?);
    }

    Ok(content)
}

fn load_dialogue(root: &Path, content: &mut Content) -> Result<(), ContentError> {
    let dialogue_dir = root.join("dialogue");
    if !dialogue_dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(&dialogue_dir).map_err(|error| {
        ContentError::new(
            "E-CONTENT-001",
            format!("cannot read dialogue dir: {error}"),
        )
    })? {
        let entry = entry.map_err(|error| {
            ContentError::new(
                "E-CONTENT-001",
                format!("dialogue dir entry error: {error}"),
            )
        })?;
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "ron") {
            let scenes: Vec<DialogueSceneData> = load_ron_file(&path)?;
            for scene in scenes {
                content.dialogue.insert(scene.id.clone(), scene);
            }
        }
    }
    Ok(())
}

/// Load a RON file that contains a Vec of records.
fn load_ron_file<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Vec<T>, ContentError> {
    let data = read_file_with_limits(path)?;
    let parsed: Vec<T> = ron::from_str(&data).map_err(|e| {
        ContentError::new(
            "E-CONTENT-001",
            format!("parse error in {}: {}", path.display(), e),
        )
    })?;
    if parsed.len() > MAX_RECORDS_PER_FILE {
        return Err(ContentError::new(
            "E-CONTENT-001",
            format!(
                "{}: {} records exceeds limit of {}",
                path.display(),
                parsed.len(),
                MAX_RECORDS_PER_FILE
            ),
        ));
    }
    Ok(parsed)
}

/// Load a single-record RON file.
fn load_single_ron<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ContentError> {
    let data = read_file_with_limits(path)?;
    let parsed: T = ron::from_str(&data).map_err(|e| {
        ContentError::new(
            "E-CONTENT-001",
            format!("parse error in {}: {}", path.display(), e),
        )
    })?;
    Ok(parsed)
}

/// Read a file, checking size limits.
fn read_file_with_limits(path: &Path) -> Result<String, ContentError> {
    let metadata = fs::metadata(path).map_err(|e| {
        ContentError::new(
            "E-CONTENT-001",
            format!("cannot read {}: {}", path.display(), e),
        )
    })?;
    let file_size = metadata.len();
    if file_size > MAX_CONTENT_FILE_BYTES {
        return Err(ContentError::new(
            "E-CONTENT-001",
            format!(
                "{}: file size {} exceeds limit of {}",
                path.display(),
                file_size,
                MAX_CONTENT_FILE_BYTES
            ),
        ));
    }
    fs::read_to_string(path).map_err(|e| {
        ContentError::new(
            "E-CONTENT-001",
            format!("cannot read {}: {}", path.display(), e),
        )
    })
}

/// Validate that all scenario references (weapon ids, actor ids) are consistent.
pub fn validate_references(content: &Content) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut seen_ids: BTreeMap<&str, &str> = BTreeMap::new();

    // Check scenario actor ids for duplicates
    for (sid, scenario) in &content.scenarios {
        for actor in &scenario.actors {
            if let Some(&existing) = seen_ids.get(actor.id.as_str()) {
                diags.push(Diagnostic {
                    code: "E-CONTENT-003".into(),
                    message: format!(
                        "duplicate actor id {} in scenario {} (also in {})",
                        actor.id, sid, existing
                    ),
                    file: Some(format!("scenarios/{}.ron", sid)),
                    line: None,
                });
            } else {
                seen_ids.insert(&actor.id, sid);
            }
        }
    }

    diags
}
