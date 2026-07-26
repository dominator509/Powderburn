use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// An item stack in an actor's inventory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemStack {
    pub item_id: String,
    pub count: u32,
}

/// The seven core attributes (1-10 each).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attributes {
    pub grit: u32,
    pub nerve: u32,
    pub wind: u32,
    pub hands: u32,
    pub eyes: u32,
    pub savvy: u32,
    pub luck: u32,
}

/// A dice roll expression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiceRoll {
    pub count: i32,
    pub sides: i32,
    pub bonus: i32,
}

/// An actor definition in content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorData {
    pub id: String,
    pub archetype_id: String,
    pub faction_id: String,
    pub attributes: Attributes,
    pub level: u32,
    pub hp: i32,
    pub hp_max: i32,
    pub sand: i32,
    pub sand_max: i32,
    pub ap: i16,
    pub ap_max: i16,
    pub sequence: i32,
    pub pos: TileXYData,
    pub facing: u8,
    pub stance: String,
    pub wounds: Vec<String>,
    pub inventory: Vec<ItemStack>,
    pub equipped_primary: Option<String>,
    pub equipped_sidearm: Option<String>,
    pub marks: Vec<String>,
    pub ways: Vec<String>,
    pub is_companion: bool,
    pub is_dead: bool,
}

/// A tile coordinate for serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileXYData {
    pub x: i16,
    pub y: i16,
}

/// A weapon definition in content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponData {
    pub id: String,
    pub display_name: String,
    pub damage_dice: DiceRoll,
    pub accuracy: i32,
    pub range_bands: [i32; 4],
    pub ap_override: BTreeMap<String, i16>,
    pub capacity: i32,
    pub reload_class: String,
    pub fouling_rate: i32,
    pub base_misfire: i32,
    pub smoke_output: i32,
    pub two_handed: bool,
    pub first_year_available: u16,
    pub historical_note: String,
    pub sources: Vec<String>,
}

/// A tile definition in content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileData {
    pub terrain: String,
    pub elevation: i32,
    pub cover_edges: [String; 8],
    pub smoke_density: u32,
    pub fire: bool,
    pub blood: bool,
    pub occupant: Option<String>,
    pub items: Vec<String>,
}

/// A map definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapData {
    pub width: u32,
    pub height: u32,
    pub tiles: BTreeMap<String, TileData>,
}

/// A mission objective.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectiveData {
    pub id: String,
    pub description: String,
    pub kind: String,
    pub actor_ids: Vec<String>,
}

/// A scenario definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioData {
    pub id: String,
    pub display_name: String,
    pub date: String,
    pub map: MapData,
    pub light: String,
    pub weather: String,
    pub wind_dir: String,
    pub deployment_zones: BTreeMap<String, Vec<TileXYData>>,
    pub actors: Vec<ActorData>,
    pub objectives: Vec<ObjectiveData>,
    pub victory_conditions: Vec<String>,
    pub defeat_conditions: Vec<String>,
    pub historical_tag: Option<String>,
    pub citations: Vec<String>,
}

/// A campaign node definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignNodeData {
    pub id: String,
    pub kind: String,
    pub date: String,
    pub requires: Vec<String>,
    pub grants: Vec<String>,
    pub unlocks: Vec<String>,
    pub historical_tag: Option<String>,
    pub citations: Vec<String>,
    pub companion_gates: Vec<String>,
    pub scenario_id: Option<String>,
}

/// A companion character record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanionData {
    pub id: String,
    pub display_name: String,
    pub nation: Option<String>,
    pub community: Option<String>,
    pub sources: Vec<String>,
    pub attributes: Attributes,
    pub starting_weapons: Vec<String>,
    pub arc_anchor: String,
    pub dialogue_file: String,
}

/// A Ledger entry for serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntryData {
    pub index: u32,
    pub prev_hash: String,
    pub name: String,
    pub role: String,
    pub place: String,
    pub date: String,
    pub chosen_line: String,
    pub written_by: String,
    pub hash: String,
}

/// Simulation snapshot for mid-combat saves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimSnapshotData {
    pub tick: u64,
    pub actors: Vec<ActorData>,
    pub smoke: BTreeMap<String, u32>,
}

/// Save file format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveFileData {
    pub format_version: u32,
    pub ruleset_hash: String,
    pub content_hash: String,
    pub campaign_seed: u64,
    pub ledger_head_hash: String,
    pub ledger_entries: Vec<LedgerEntryData>,
    pub campaign_flags: Vec<String>,
    pub company: Vec<ActorData>,
    pub sim_snapshot: Option<SimSnapshotData>,
    pub written_at_tick: u64,
}

/// Loaded content container.
#[derive(Debug, Clone, Default)]
pub struct Content {
    pub scenarios: BTreeMap<String, ScenarioData>,
    pub weapons: BTreeMap<String, WeaponData>,
    pub companions: BTreeMap<String, CompanionData>,
    pub campaign_nodes: BTreeMap<String, CampaignNodeData>,
}
