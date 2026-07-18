use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::items::ItemId;
use crate::perks::Perk;
use crate::resources::DifficultyMode;
use crate::species::SpeciesId;
use crate::world::Tile;

#[derive(Serialize, Deserialize)]
pub struct PlayerSave {
    pub position: (i32, i32),
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    pub hunger: f32,
    pub fatigue: f32,
    pub inventory: Vec<(ItemId, u32)>,
    pub level: u32,
    pub xp: u32,
    pub xp_to_next: u32,
    #[serde(default)]
    pub decompiler: i32,
    #[serde(default)]
    pub weapon: Option<ItemId>,
    #[serde(default)]
    pub armor: Option<ItemId>,
    #[serde(default)]
    pub module: Option<ItemId>,
    /// Unspent Perk Points (see `perks::Perk`). Defaults to 0 for saves
    /// written before perks existed.
    #[serde(default)]
    pub perk_points: u32,
    /// Which perks have been unlocked. Defaults to empty for saves written
    /// before perks existed.
    #[serde(default)]
    pub unlocked_perks: Vec<Perk>,
}

#[derive(Serialize, Deserialize)]
pub struct CreatureSave {
    pub species: SpeciesId,
    pub position: (i32, i32),
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    pub tamed: bool,
    /// Only meaningful when `tamed` is true; wild creatures don't level.
    pub level: u32,
    pub xp: u32,
    pub xp_to_next: u32,
    /// Only meaningful when `tamed` is true. The target structure is
    /// identified by position rather than entity id, since entity ids
    /// aren't stable across a save/load round trip.
    #[serde(default)]
    pub cronjob: Option<CronjobSave>,
    /// Only meaningful when `tamed` is true: whether this program is the
    /// player's active battle companion.
    #[serde(default)]
    pub is_companion: bool,
    /// Which zone sector this creature was originally spawned in (see
    /// `components::ZonePortal`). Defaults to 1 for saves written before
    /// zone portals existed.
    #[serde(default = "default_zone_level")]
    pub zone: u32,
}

/// An in-progress work assignment (a "cronjob") a tamed creature is running
/// against a structure, persisted so it survives save/load instead of
/// silently dropping the worker's progress.
#[derive(Serialize, Deserialize)]
pub struct CronjobSave {
    pub target_position: (i32, i32),
    pub progress: u32,
    pub required: u32,
}

#[derive(Serialize, Deserialize)]
pub struct StructureSave {
    pub kind: String,
    pub position: (i32, i32),
    pub resource_amount: Option<u32>,
    /// Current raid durability (see `components::Durability`). `None` for
    /// saves written before raids existed — treated as full health at load
    /// time, using whatever the structure's current `.ron` def says.
    #[serde(default)]
    pub durability: Option<u32>,
}

/// Only the world seed and the sparse tile overlay are persisted; unmodified
/// terrain regenerates deterministically from the seed on load.
#[derive(Serialize, Deserialize)]
pub struct SaveData {
    pub seed: u32,
    pub tick: u64,
    pub difficulty: DifficultyMode,
    pub player: PlayerSave,
    pub creatures: Vec<CreatureSave>,
    pub structures: Vec<StructureSave>,
    pub tile_overrides: Vec<((i32, i32), Tile)>,
    /// Which zone sector the player had breached into. Defaults to 1 (the
    /// starting sector) for saves written before zone portals existed.
    #[serde(default = "default_zone_level")]
    pub zone: u32,
}

fn default_zone_level() -> u32 {
    1
}

pub fn save_to_file(path: &Path, data: &SaveData) -> io::Result<()> {
    let bytes = bincode::serde::encode_to_vec(data, bincode::config::standard())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, bytes)
}

pub fn load_from_file(path: &Path) -> io::Result<SaveData> {
    let bytes = std::fs::read(path)?;
    let (data, _) = bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(data)
}

/// Minimal nod to Dwarf Fortress's legends: on a permadeath run ending, a
/// short structured summary is appended to a plain-text history log.
pub fn append_run_history(path: &Path, summary: &str) -> io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{summary}")
}
