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
    pub grit: i32,
    pub nerve: i32,
    pub wind: i32,
    pub hands: i32,
    pub eyes: i32,
    pub savvy: i32,
    pub luck: i32,
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

/// An item definition (consumable, quest, or misc).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemData {
    pub id: String,
    pub display_name: String,
    pub item_type: String,
    pub weight_lbs: f32,
    pub description: String,
    pub effects: Option<ItemEffects>,
    pub quest_id: Option<String>,
}

/// Effects that a consumable item provides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemEffects {
    pub sand_restore: Option<i32>,
    pub hp_restore: Option<i32>,
    pub bleed_stop: Option<bool>,
    pub ap_restore: Option<i16>,
    pub suppress_clear: Option<bool>,
}

/// A mark (perk gained every 3rd level).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkData {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub effects: MarkEffects,
}

/// Effects provided by a mark.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkEffects {
    pub stat_mods: Option<Attributes>,
    pub ability_grant: Option<String>,
    pub passive: Option<String>,
    pub ap_bonus: Option<i16>,
    pub accuracy_bonus: Option<i32>,
    pub penalty_reduction: Option<i32>,
    pub cost_reduction: Option<i32>,
}

/// A way (starting trait chosen at character creation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WayData {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub stat_mods: Attributes,
    pub starting_items: Vec<String>,
}

/// A faction record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactionData {
    pub id: String,
    pub display_name: String,
    pub interests: Vec<String>,
    pub sand_multiplier: f32,
    pub default_hostility: String,
    pub description: String,
}

/// A hit location entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitLocationEntry {
    pub name: String,
    pub weight_pct: f32,
    pub damage_multiplier: f32,
    pub critical_threshold: i32,
}

/// A range band definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeBand {
    pub name: String,
    pub min_tiles: i32,
    pub max_tiles: i32,
    pub accuracy_mod: i32,
}

/// A stance modifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StanceMod {
    pub name: String,
    pub evasion_bonus: i32,
    pub accuracy_bonus: i32,
}

/// A cover modifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverMod {
    pub name: String,
    pub accuracy_penalty: i32,
}

/// A called shot penalty entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalledShotPenalty {
    pub hit_location: String,
    pub penalty: i32,
}

/// A critical effect entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalEffect {
    pub roll_range: (i32, i32),
    pub effect_name: String,
    pub description: String,
}

/// A wound healing time entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WoundHealingTime {
    pub wound_name: String,
    pub camp_days: i32,
}

/// Game balance tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameTables {
    pub hit_locations: Vec<HitLocationEntry>,
    pub range_bands: Vec<RangeBand>,
    pub stance_modifiers: Vec<StanceMod>,
    pub cover_modifiers: Vec<CoverMod>,
    pub called_shot_penalties: Vec<CalledShotPenalty>,
    pub critical_effects: Vec<CriticalEffect>,
    pub wound_healing_times: Vec<WoundHealingTime>,
}

/// Loaded content container.
#[derive(Debug, Clone, Default)]
pub struct Content {
    pub scenarios: BTreeMap<String, ScenarioData>,
    pub weapons: BTreeMap<String, WeaponData>,
    pub companions: BTreeMap<String, CompanionData>,
    pub campaign_nodes: BTreeMap<String, CampaignNodeData>,
    pub items: BTreeMap<String, ItemData>,
    pub marks: BTreeMap<String, MarkData>,
    pub ways: BTreeMap<String, WayData>,
    pub factions: BTreeMap<String, FactionData>,
    pub tables: Option<GameTables>,
}
