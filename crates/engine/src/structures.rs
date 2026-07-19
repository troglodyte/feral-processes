use std::collections::HashMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::components::GlyphColor;
use crate::items::ItemId;

pub type StructureId = String;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkDef {
    pub produces: ItemId,
    pub ticks_per_unit: u32,
    /// How many units a worked node stores before its assigned creature has
    /// to wait for it to refill. Once a node is mined down to 0 it
    /// immediately refills to `capacity` and the cronjob keeps running —
    /// nodes are an infinite (if bursty) resource, not a one-time deposit.
    /// `#[serde(default)]` so existing structure files (including mods)
    /// without this field get a sensible baseline.
    #[serde(default = "default_work_capacity")]
    pub capacity: u32,
    /// If set, a completed gather cycle isn't a guaranteed yield: it only
    /// pays out with a level-based percentage chance (see
    /// `systems::task_progress_system`), and a miss still resets the cycle.
    /// Higher `level` values yield more reliably. `None` (the default) keeps
    /// the old always-succeeds behavior — this is an opt-in per structure,
    /// not something every worked node gets automatically.
    /// `#[serde(default)]` so existing structure files (including mods)
    /// without this field keep parsing as guaranteed-yield nodes.
    #[serde(default)]
    pub level: Option<u32>,
}

fn default_work_capacity() -> u32 {
    5
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PassiveProcessDef {
    pub consumes: ItemId,
    pub produces: ItemId,
    pub ticks_per_unit: u32,
    /// Chebyshev distance (in tiles) the player must be within for this to run.
    pub radius: i32,
}

/// A structure's trading post capability: sell any item here for a flat
/// per-unit payout, and buy specific items back for Core Fragments.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TradeDef {
    /// Core Fragments granted per unit when selling any item to this
    /// structure — a uniform sell price, not a per-item table.
    pub sell_rate: u32,
    /// Items purchasable here, each as `(item, cost in Core Fragments)`.
    pub buy: Vec<(ItemId, u32)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StructureDef {
    pub id: StructureId,
    pub name: String,
    pub glyph: char,
    pub color: GlyphColor,
    pub build_cost: Vec<(ItemId, u32)>,
    /// If set, a tamed creature can be assigned to work this structure,
    /// producing `produces` every `ticks_per_unit` ticks.
    pub work: Option<WorkDef>,
    /// If set, this structure automatically converts `consumes` into
    /// `produces` whenever the player is standing within `radius` tiles —
    /// no assigned worker needed, unlike `work`. `#[serde(default)]` so
    /// existing structure files written before this field existed still
    /// parse (defaulting to no passive processing).
    #[serde(default)]
    pub passive_process: Option<PassiveProcessDef>,
    /// If set, this structure is a symlink target: `Game::use_symlink` can
    /// teleport the player to it for this item cost, from anywhere on the
    /// map. `#[serde(default)]` so existing structure files written before
    /// this field existed still parse (defaulting to no symlink).
    #[serde(default)]
    pub teleport_cost: Option<Vec<(ItemId, u32)>>,
    /// If true, walking onto this structure breaches into the next zone
    /// (see `Game::enter_next_zone`) instead of just blocking movement.
    /// `build_cost` is treated as a *per-zone-level* rate for this
    /// structure: the actual cost charged when deploying it is each amount
    /// multiplied by the current zone level, since a deeper breach costs
    /// more raw material. `#[serde(default)]` so existing structure files
    /// written before this field existed still parse (defaulting to a
    /// plain, non-portal structure).
    #[serde(default)]
    pub zone_portal: bool,
    /// If set, this structure is a trading post: `Game::sell_item` and
    /// `Game::buy_item` work against it. `#[serde(default)]` so existing
    /// structure files written before this field existed still parse
    /// (defaulting to no trading).
    #[serde(default)]
    pub trade: Option<TradeDef>,
    /// How much damage this structure can take from raids (see
    /// `components::Durability`) before being destroyed.
    /// `#[serde(default = "default_durability")]` so existing structure
    /// files (including mods) without this field get a sturdy baseline
    /// rather than 0, which would let the very next raid destroy them.
    #[serde(default = "default_durability")]
    pub durability: u32,
    /// How much this structure reduces raid damage by, for *every* raid
    /// against *any* deployed structure — not just itself — while it's
    /// standing (see `Game::raid_check`). Stacks additively across every
    /// deployed structure with this set, on top of whatever an assigned
    /// worker/guard already mitigates. `#[serde(default)]` so existing
    /// structure files (including mods) without this field contribute
    /// nothing, same as before it existed.
    #[serde(default)]
    pub raid_defense: u32,
}

fn default_durability() -> u32 {
    30
}

#[derive(Resource, Default)]
pub struct StructureDb {
    structures: HashMap<StructureId, StructureDef>,
}

impl StructureDb {
    /// Loads every `*.ron` structure definition in `dir`. Malformed files
    /// are skipped (with a returned warning) rather than aborting the whole
    /// load — a single bad custom/mod file shouldn't be able to crash
    /// startup for everything else.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = StructureDb::default();
        let mut warnings = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<StructureDef>(&text) {
                Ok(def) => {
                    db.structures.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid structure file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &str) -> Option<&StructureDef> {
        self.structures.get(id)
    }

    /// Every loaded structure, sorted by `id`. `HashMap` iteration order is
    /// randomized per-instance (a fresh seed each time a `StructureDb` is
    /// built, i.e. every new/loaded game), so without this sort, the
    /// build menu's `[1]`, `[2]`, ... numbering would shuffle unpredictably
    /// from one session to the next even though nothing about the mod files
    /// changed — the same digit could mean a 2-Core-Fragment Mining Node in
    /// one session and an 8-Core-Fragment Fabricator in the next.
    pub fn all(&self) -> impl Iterator<Item = &StructureDef> {
        let mut defs: Vec<&StructureDef> = self.structures.values().collect();
        defs.sort_by(|a, b| a.id.cmp(&b.id));
        defs.into_iter()
    }
}
