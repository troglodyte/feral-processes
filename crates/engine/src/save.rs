use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::components::ActiveFieldBuff;
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
    /// The abilities installed in the player's routine slots, in menu order
    /// — see `components::Routines`.
    pub routines: Vec<crate::abilities::AbilityId>,
    /// Every field buff currently running on the player — see
    /// `components::FieldBuff`. Player state, not zone-local, so
    /// `Game::enter_next_zone` must never clear it.
    pub field_buffs: Vec<ActiveFieldBuff>,
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
    /// This program's index in the player's active party, or `None` if it
    /// isn't a party member. Party order is mechanically meaningful under
    /// soft ranks — front slots draw more enemy fire — so it can't be
    /// rebuilt from creature-iteration order the way it was before.
    /// Supersedes the old `is_companion` flag, which `party_slot.is_some()`
    /// now says.
    pub party_slot: Option<u32>,
    /// Which zone sector this creature was originally spawned in (see
    /// `components::ZonePortal`).
    pub zone: u32,
    /// The player's custom display name for this creature, if they set one
    /// (see `components::CustomName`) — currently only possible via
    /// `Game::fuse_companions`. This is a shape change to `CreatureSave`,
    /// so it required bumping `SAVE_FORMAT_VERSION` (bincode has no
    /// granular field-level compatibility here — see that constant's docs).
    pub custom_name: Option<String>,
    /// This creature's individual quality roll — see
    /// `components::Potential`. Persisted so `growth_roll` keeps applying
    /// consistently across save/load rather than resetting; `hp_roll`/
    /// `atk_roll`/`def_roll` are along for the ride purely so
    /// `Potential::quality_percent`/`quality_label` stay accurate too.
    pub hp_roll: f32,
    pub atk_roll: f32,
    pub def_roll: f32,
    pub growth_roll: f32,
    /// How many fusions deep this creature's lineage is — see
    /// `components::FusionCount`. Persisted so the `MAX_FUSIONS` ceiling
    /// survives a save/load instead of resetting to 0 and handing the
    /// player unlimited fusions for free.
    pub fusions: u32,
    /// The abilities installed in this program's routine slots, in menu
    /// order — see `components::Routines`. Persisted rather than re-derived
    /// from its species, because an innate routine can be popped out and a
    /// foreign one plugged in.
    pub routines: Vec<crate::abilities::AbilityId>,
    /// Every field buff currently running on this creature — see
    /// `components::FieldBuff`. A companion sold, extracted, fused away or
    /// killed takes this with it: the entity simply despawns, and neither
    /// `Game::dissolve_tamed_program` nor `Game::fuse_companions` needs to
    /// know this field exists.
    pub field_buffs: Vec<ActiveFieldBuff>,
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
    /// Current upgrade tier — see `components::StructureTier`. `None` for a
    /// structure whose def declares no upgrade path.
    pub tier: Option<u32>,
}

/// One trading post's buyback shelf on disk: the trader kind and tile that
/// key it, then what is on it — see `SaveData::buyback`.
pub type BuybackShelfSave = (
    crate::structures::StructureId,
    (i32, i32),
    Vec<(ItemId, u32)>,
);

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
    /// Each trading post's buyback shelf — see `resources::BuybackLedger`,
    /// whose `BTreeMap` this is the flattened, key-ordered form of. Not part
    /// of `StructureSave` because a shelf outlives its building and can sit
    /// on a tile holding nothing at all.
    pub buyback: Vec<BuybackShelfSave>,
    /// Which research nodes have been unlocked — see `research::ResearchDb`.
    /// Sorted on write so the encoded bytes don't depend on `HashSet`
    /// iteration order.
    pub researched: Vec<crate::research::ResearchId>,
    /// Every Stack entrance standing on the zone map — see
    /// `components::SurfaceLink`. Only the tile: an entrance carries no
    /// state of its own, and which stack it opens onto is a pure function
    /// of the world seed and the depth walked to.
    pub link_sites: Vec<(i32, i32)>,
    /// Whether the player was on the surface or down the Stack, and where —
    /// see `resources::Locale`. The frame itself is *not* here: it
    /// regenerates from `seed` and the saved depth, exactly as terrain
    /// regenerates from `seed` alone.
    pub locale: crate::resources::Locale,
    /// What the party has learned about each Stack frame walked in this
    /// zone — see `resources::StackMemory`. The one piece of Stack state
    /// that is saved rather than regenerated: a frame is a pure function of
    /// its spec, but which parts of it the player has *seen* is history.
    pub stack_memory: crate::resources::StackMemory,
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
pub const SAVE_FORMAT_VERSION: u32 = 15;

/// Renders a save as editable RON, for the `savetool` binary.
///
/// This is the one place the save is legible: the on-disk form is bincode,
/// which is positional and carries no field names (see
/// `SAVE_FORMAT_VERSION`), so there is nothing to hand-edit without going
/// through here. Round-trip fidelity is what makes it safe to edit —
/// `a_save_survives_a_round_trip_through_ron_unchanged` pins that.
pub fn to_ron(data: &SaveData) -> io::Result<String> {
    ron::ser::to_string_pretty(data, ron::ser::PrettyConfig::default())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Parses the RON produced by `to_ron` back into a save.
pub fn from_ron(text: &str) -> io::Result<SaveData> {
    ron::from_str(text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

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
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "save file is too short to be valid",
        ));
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
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
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
                routines: Vec::new(),
                field_buffs: Vec::new(),
            },
            creatures: Vec::new(),
            structures: Vec::new(),
            tile_overrides: Vec::new(),
            zone: 1,
            spawn_point: (0, 0),
            buyback: Vec::new(),
            researched: Vec::new(),
            link_sites: Vec::new(),
            locale: crate::resources::Locale::Surface,
            stack_memory: crate::resources::StackMemory::default(),
        }
    }

    /// The savetool's whole premise: a save dumped to RON, then packed back,
    /// must be the same save. Byte identity of the bincode encoding is the
    /// strictest form of that and catches a field silently dropped by the
    /// text encoding, which a field-by-field assertion would miss.
    #[test]
    fn a_save_survives_a_round_trip_through_ron_unchanged() {
        let mut data = sample_data();
        data.player.inventory = vec![(ItemId::from("core_fragment"), 3)];
        data.player.hunger = 62.5;
        data.tile_overrides = vec![(
            (4, -7),
            Tile {
                biome: crate::world::Biome::Platform,
                walkable: true,
            },
        )];
        data.zone = 3;
        // `StackMemory` is a map keyed by a *tuple* (`LevelKey`), which is
        // exactly where a text encoding tends to give up, and `Locale` is a
        // struct-variant enum. Both are in the round trip deliberately.
        data.locale = crate::resources::Locale::Stack {
            depth: 2,
            frames: 4,
            x: 9,
            y: 11,
            facing: crate::stack::Dir::West,
            entrance: (4, -7),
        };
        data.stack_memory.0.insert(
            ((4, -7), 2),
            crate::resources::FrameMemory {
                seen: [(1, 1), (1, 2)].into_iter().collect(),
                looted: [(3, 3)].into_iter().collect(),
                opened: Default::default(),
                cleared: true,
                fights: [(5, 5)].into_iter().collect(),
            },
        );

        let before = bincode::serde::encode_to_vec(&data, bincode::config::standard()).unwrap();
        let text = to_ron(&data).unwrap();
        let parsed = from_ron(&text).unwrap();
        let after = bincode::serde::encode_to_vec(&parsed, bincode::config::standard()).unwrap();

        assert_eq!(
            before, after,
            "a RON round trip must not change a single byte of the save"
        );
    }

    #[test]
    fn a_save_round_trips_through_the_current_version() {
        let path = std::env::temp_dir().join(format!(
            "feral_processes_save_roundtrip_{}.bin",
            std::process::id()
        ));
        save_to_file(&path, &sample_data()).unwrap();
        let loaded = load_from_file(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(loaded.seed, 1);
    }

    #[test]
    fn a_save_written_with_a_different_version_is_rejected_cleanly_instead_of_corrupting() {
        let path = std::env::temp_dir().join(format!(
            "feral_processes_save_badversion_{}.bin",
            std::process::id()
        ));
        let encoded =
            bincode::serde::encode_to_vec(sample_data(), bincode::config::standard()).unwrap();
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

    /// `SAVE_FORMAT_VERSION` moved 14 -> 15 to add `field_buffs`. There is
    /// no migration path (see that constant's docs), so a save genuinely
    /// written under the prior version must be refused exactly like any
    /// other version mismatch, not silently decoded into garbage.
    #[test]
    fn a_save_written_at_v14_is_refused_now_that_v15_is_current() {
        let path = std::env::temp_dir().join(format!(
            "feral_processes_save_v14_{}.bin",
            std::process::id()
        ));
        let encoded =
            bincode::serde::encode_to_vec(sample_data(), bincode::config::standard()).unwrap();
        let mut bytes = 14u32.to_le_bytes().to_vec();
        bytes.extend(encoded);
        std::fs::write(&path, bytes).unwrap();

        let Err(err) = load_from_file(&path) else {
            panic!("a v14 save should not load under the v15 format");
        };
        let _ = std::fs::remove_file(&path);
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(
            err.to_string().contains("incompatible save version"),
            "error should clearly say the save is from an incompatible version, got: {err}"
        );
    }

    /// `dev-saves/extraction.ron` deserializes by field name — RON is
    /// self-describing, unlike the positional bincode save (see
    /// `SAVE_FORMAT_VERSION`'s docs) — so a `SaveData` field rename that
    /// forgets to update the template's keys breaks `--template extraction`
    /// at load. Nothing in the launcher's `dev_template` tests loads this
    /// file by name, so this is the guard.
    #[test]
    fn the_extraction_template_parses_into_save_data() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dev-saves/extraction.ron");
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        if let Err(e) = from_ron(&text) {
            panic!("dev-saves/extraction.ron should parse into SaveData: {e}");
        }
    }

    /// The claim `SAVE_FORMAT_VERSION` staying at 15 rests on: a save
    /// carrying a Stack position, written and read back through the real
    /// file round trip (not just the RON one above), still has that
    /// position, and the version prefix is unmoved.
    #[test]
    fn a_stack_position_survives_a_binary_save_round_trip() {
        assert_eq!(SAVE_FORMAT_VERSION, 15);
        let path = std::env::temp_dir().join(format!(
            "feral_processes_save_stack_roundtrip_{}.bin",
            std::process::id()
        ));
        let mut data = sample_data();
        data.locale = crate::resources::Locale::Stack {
            depth: 2,
            frames: 4,
            x: 9,
            y: 11,
            facing: crate::stack::Dir::West,
            entrance: (4, -7),
        };
        save_to_file(&path, &data).unwrap();
        let loaded = load_from_file(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            loaded.locale, data.locale,
            "the Stack position did not survive the round trip"
        );
    }

    #[test]
    fn a_truncated_file_fails_cleanly_instead_of_panicking() {
        let path = std::env::temp_dir().join(format!(
            "feral_processes_save_truncated_{}.bin",
            std::process::id()
        ));
        std::fs::write(&path, [1, 2]).unwrap();
        let Err(err) = load_from_file(&path) else {
            panic!("loading a truncated save should fail, not succeed");
        };
        let _ = std::fs::remove_file(&path);
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
