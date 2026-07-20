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
    pub decompiler: i32,
    pub weapon: Option<ItemId>,
    /// Gear level `weapon` was equipped at — see `components::EquippedItem`.
    pub weapon_level: u32,
    /// Fusion tier `weapon` was equipped at — see `components::EquippedItem`.
    pub weapon_fusion_tier: u32,
    pub armor: Option<ItemId>,
    pub armor_level: u32,
    pub armor_fusion_tier: u32,
    pub module: Option<ItemId>,
    pub module_level: u32,
    pub module_fusion_tier: u32,
    /// Unspent Perk Points — see `perks::Perk`.
    pub perk_points: u32,
    /// Which perks have been bought, and at what level (see
    /// `components::Perks::level`) — one entry per level bought.
    pub unlocked_perks: Vec<Perk>,
    /// How many times each item type has been fused — see
    /// `components::ItemFusions`.
    pub item_fusions: Vec<(ItemId, u32)>,
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
    pub cronjob: Option<CronjobSave>,
    /// Only meaningful when `tamed` is true: whether this program is the
    /// player's active battle companion.
    pub is_companion: bool,
    /// Which zone sector this creature was originally spawned in (see
    /// `components::ZonePortal`).
    pub zone: u32,
    /// The player's custom display name for this creature, if they set one
    /// (see `components::CustomName`) — currently only possible via
    /// `Game::fuse_companions`. This is a shape change to `CreatureSave`,
    /// so it required bumping `SAVE_FORMAT_VERSION` (bincode has no
    /// granular field-level compatibility here — see that constant's docs).
    pub custom_name: Option<String>,
}

/// Mirrors `components::TaskKind` for persistence — kept separate so the
/// engine-internal enum doesn't need to derive `Serialize`/`Deserialize`.
#[derive(Serialize, Deserialize, Default, Clone, Copy)]
pub enum CronjobKind {
    #[default]
    GatherResource,
    Guard,
}

/// An in-progress work assignment (a "cronjob") a tamed creature is running
/// against a structure, persisted so it survives save/load instead of
/// silently dropping the worker's progress.
#[derive(Serialize, Deserialize)]
pub struct CronjobSave {
    pub target_position: (i32, i32),
    pub progress: u32,
    pub required: u32,
    pub kind: CronjobKind,
}

#[derive(Serialize, Deserialize)]
pub struct StructureSave {
    pub kind: String,
    pub position: (i32, i32),
    pub resource_amount: Option<u32>,
    /// Current raid durability — see `components::Durability`.
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
    /// Which zone sector the player had breached into.
    pub zone: u32,
    /// Where the player materialized on breaching into that zone — see
    /// `resources::ZoneSpawnPoint`.
    pub spawn_point: (i32, i32),
}

/// Bumped whenever `SaveData` (or anything it contains, transitively)
/// changes shape in *any* way — a field added/removed/reordered, an enum
/// gaining a variant, all of it.
///
/// bincode encodes everything *positionally*: it has no field names or
/// self-describing structure on disk, so a struct is really just "decode
/// exactly `fields.len()` values in order," where `fields.len()` is
/// whatever the *current* type definition says. serde's `#[serde(default)]`
/// (which genuinely works for the RON-based species/structure asset files,
/// since RON *is* self-describing) does **not** give bincode saves any
/// backward compatibility: an old file missing a newly-added field doesn't
/// decode that field as its default, it desyncs every byte read after that
/// point and produces garbage — which usually doesn't fail until some much
/// later, unrelated field happens to decode into a nonsense enum
/// discriminant. That's a footgun this project hit directly: several
/// fields below used to carry `#[serde(default = ...)]` on the assumption
/// that it made old saves keep loading, and it silently didn't.
///
/// The fix is this version prefix (see `save_to_file`/`load_from_file`): a
/// save written by a different version is rejected up front with a clear
/// error, instead of decoded into corruption. There is no partial/granular
/// compatibility — any shape change at all means bumping this constant,
/// and every save written under the old version stops loading. That's an
/// intentional, simple tradeoff for a single-player game rather than
/// building real schema migration.
pub const SAVE_FORMAT_VERSION: u32 = 4;

pub fn save_to_file(path: &Path, data: &SaveData) -> io::Result<()> {
    let encoded = bincode::serde::encode_to_vec(data, bincode::config::standard())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut bytes = Vec::with_capacity(4 + encoded.len());
    bytes.extend_from_slice(&SAVE_FORMAT_VERSION.to_le_bytes());
    bytes.extend(encoded);
    std::fs::write(path, bytes)
}

pub fn load_from_file(path: &Path) -> io::Result<SaveData> {
    let bytes = std::fs::read(path)?;
    let Some((version_bytes, payload)) = bytes.split_first_chunk::<4>() else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "save file is too short to be valid"));
    };
    let version = u32::from_le_bytes(*version_bytes);
    if version != SAVE_FORMAT_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "incompatible save version (v{version}, this build reads v{SAVE_FORMAT_VERSION}) — \
                 delete it and start a new game"
            ),
        ));
    }
    let (data, _) = bincode::serde::decode_from_slice(payload, bincode::config::standard())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(data)
}

/// Minimal nod to Dwarf Fortress's legends: on a permadeath run ending, a
/// short structured summary is appended to a plain-text history log.
pub fn append_run_history(path: &Path, summary: &str) -> io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{summary}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_data() -> SaveData {
        SaveData {
            seed: 1,
            tick: 0,
            difficulty: DifficultyMode::Forgiving,
            player: PlayerSave {
                position: (0, 0),
                hp: 30,
                max_hp: 30,
                atk: 6,
                def: 2,
                hunger: 100.0,
                fatigue: 100.0,
                inventory: Vec::new(),
                level: 1,
                xp: 0,
                xp_to_next: 20,
                decompiler: 0,
                weapon: None,
                weapon_level: 1,
                weapon_fusion_tier: 0,
                armor: None,
                armor_level: 1,
                armor_fusion_tier: 0,
                module: None,
                module_level: 1,
                module_fusion_tier: 0,
                item_fusions: Vec::new(),
                perk_points: 0,
                unlocked_perks: Vec::new(),
            },
            creatures: Vec::new(),
            structures: Vec::new(),
            tile_overrides: Vec::new(),
            zone: 1,
            spawn_point: (0, 0),
        }
    }

    #[test]
    fn a_save_round_trips_through_the_current_version() {
        let path = std::env::temp_dir()
            .join(format!("feral_processes_save_roundtrip_{}.bin", std::process::id()));
        save_to_file(&path, &sample_data()).unwrap();
        let loaded = load_from_file(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(loaded.seed, 1);
    }

    #[test]
    fn a_save_written_with_a_different_version_is_rejected_cleanly_instead_of_corrupting() {
        let path = std::env::temp_dir()
            .join(format!("feral_processes_save_badversion_{}.bin", std::process::id()));
        let encoded = bincode::serde::encode_to_vec(sample_data(), bincode::config::standard()).unwrap();
        let mut bytes = 999u32.to_le_bytes().to_vec();
        bytes.extend(encoded);
        std::fs::write(&path, bytes).unwrap();

        let Err(err) = load_from_file(&path) else {
            panic!("loading a mismatched-version save should fail, not succeed");
        };
        let _ = std::fs::remove_file(&path);
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(
            err.to_string().contains("incompatible save version"),
            "error should clearly say the save is from an incompatible version, got: {err}"
        );
    }

    #[test]
    fn a_truncated_file_fails_cleanly_instead_of_panicking() {
        let path = std::env::temp_dir()
            .join(format!("feral_processes_save_truncated_{}.bin", std::process::id()));
        std::fs::write(&path, [1, 2]).unwrap();
        let Err(err) = load_from_file(&path) else {
            panic!("loading a truncated save should fail, not succeed");
        };
        let _ = std::fs::remove_file(&path);
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
