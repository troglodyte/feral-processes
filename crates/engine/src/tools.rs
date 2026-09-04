//! The extraction tool catalogue, the player's tool slots, and the starter
//! grant — see
//! `docs/superpowers/specs/2026-09-04-program-extraction-design.md`, section
//! 2. Still nothing *consumes* a `ToolDef`: `Game::extract_program` is a
//! later phase, and so is acquisition past the starter tool
//! (`unlocks_tools`, `KnownTools`, `forge_tool`, `install_tool`) — this
//! phase only gets a tool into a slot the one way `Game::new` does it.

use std::collections::HashMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::Game;
use crate::components::Tools;
use crate::items::ItemId;

/// A tool's id, `items::ItemId`'s shape: `#[serde(transparent)]` so a `.ron`
/// file spells one as a bare quoted string rather than a `ToolId("...")`
/// tuple, and `Ord`/`Hash` for the same reason `ItemId` carries them — a
/// slot list or a save keys off this the way `components::Stock` keys off
/// an item.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ToolId(pub String);

impl ToolId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ToolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What part of a downed program a tool reaches — a fixed, closed
/// vocabulary rather than a free-text field, because it groups the tool
/// screen (a later phase) and is read exhaustively wherever the category
/// itself decides behaviour, `render/stack.rs`'s `cell_mark` rule: a
/// catch-all arm would ship a fifth category blank.
///
/// `Routines` is the one category with no yield pool — a tool in it takes
/// the routine branch (`extract_routine`'s two paths, a later phase)
/// instead of drawing from `ToolDef::yields`. No `Routines` tool ships in
/// phase 1; see `every_non_routines_tool_has_a_non_empty_yield_pool` for
/// what that does to its own census.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCategory {
    Materials,
    Parts,
    Cores,
    Routines,
}

/// A moddable extraction tool. `assets/tools/README.md` is the schema
/// reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolDef {
    pub id: ToolId,
    pub name: String,
    pub description: String,
    pub category: ToolCategory,
    /// `(item, weight)` — one unit drawn from this pool picks an item by
    /// weight, the same relative-weight idiom `AbilityDef::wild_weight`
    /// uses. Empty for a `Routines` tool, which reads no pool at all.
    /// `#[serde(default)]` so a `Routines` file need not spell out `[]`.
    #[serde(default)]
    pub yields: Vec<(ItemId, f32)>,
    /// Scales the unit count a use produces, alongside the structure a
    /// program is extracted at — see the spec's `extraction_yield` formula.
    /// Not itself a rate: two tiers of the same category do not mean one is
    /// simply faster, they mean one reaches deeper into the same pool.
    pub tier: u32,
    /// Game ticks `Game::extract_program` (a later phase) spends on a use —
    /// `self.tick()`'s argument, the same currency `AbilityDef::power_cost`
    /// is to a routine but paid in time rather than Power.
    pub ticks: u64,
}

impl ToolDef {
    /// Names the first field holding a NaN or infinity, if any — the same
    /// defence `AbilityDef::non_finite_field` and `ItemDef::non_finite_field`
    /// take: cheaper to refuse the file at load than to defend every read a
    /// weighted pick makes downstream.
    ///
    /// A weight of zero or below is refused alongside a non-finite one:
    /// unlike a `droppable` chance (which is meaningfully `0.0`, "never"), a
    /// `yields` entry exists to be drawn from, and a weight that can never
    /// win the pick is an authoring mistake rather than a supported state.
    fn invalid_yield_weight(&self) -> Option<&'static str> {
        self.yields
            .iter()
            .any(|(_, weight)| !weight.is_finite() || *weight <= 0.0)
            .then_some("yields weight")
    }
}

#[derive(Resource, Default)]
pub struct ToolDb {
    tools: HashMap<String, ToolDef>,
}

impl ToolDb {
    /// Loads every `*.ron` tool in `dir`. A malformed file — one that fails
    /// to parse, or one that parses but names a non-finite or non-positive
    /// `yields` weight — is skipped with a returned warning rather than
    /// aborting the load, the same defence `AbilityDb::load_dir` takes.
    ///
    /// A missing `dir` is not an error, `AffixDb::load_dir`'s rule rather
    /// than `AbilityDb::load_dir`'s (abilities are mandatory content;
    /// `FALLBACK_ABILITY_ID` and `DECOMPILE_ABILITY_ID` must resolve or the
    /// game refuses to start). Tools have no such floor yet — an install
    /// without `assets/tools/` is the pre-extraction game, same as
    /// `assets/affixes/` absent is the pre-affix one.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = ToolDb::default();
        let mut warnings = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((db, warnings)),
            Err(e) => return Err(e),
        };
        for entry in entries {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<ToolDef>(&text) {
                Ok(def) => {
                    if let Some(field) = def.invalid_yield_weight() {
                        warnings.push(format!(
                            "skipped invalid tool file {path:?}: {field} must be a finite, \
                             positive number"
                        ));
                        continue;
                    }
                    db.tools.insert(def.id.as_str().to_string(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid tool file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &str) -> Option<&ToolDef> {
        self.tools.get(id)
    }

    /// Every loaded tool, by id. `HashMap` iteration order is randomized per
    /// instance, so without this a tool screen's numbering (a later phase)
    /// would shuffle between sessions even though nothing about the files
    /// changed — `AbilityDb::all`'s reason, exactly.
    pub fn all(&self) -> impl Iterator<Item = &ToolDef> {
        let mut defs: Vec<&ToolDef> = self.tools.values().collect();
        defs.sort_by(|a, b| a.id.cmp(&b.id));
        defs.into_iter()
    }
}

/// How many tool slots the player at `level` can hold — see
/// `tuning::TOOL_SLOT_BASE` and friends. Calls `abilities::routine_slots`
/// rather than restating its clamp, passing `1` as the per-step grant where
/// a routine's own wrappers pass `tuning::ROUTINE_SLOTS_PER_STEP` (2): one
/// tool slot a step is the shape this phase's controller ruling chose, and
/// it is not itself a tuning knob — `tuning::TOOL_SLOT_BASE`,
/// `TOOL_SLOT_PER_LEVEL` and `TOOL_SLOT_CAP` are the three that are.
pub fn player_tool_slots(level: u32) -> usize {
    crate::abilities::routine_slots(
        level,
        crate::tuning::TOOL_SLOT_BASE,
        1,
        crate::tuning::TOOL_SLOT_PER_LEVEL,
        crate::tuning::TOOL_SLOT_CAP,
    )
}

impl Game {
    /// The player's installed tools, resolved to their full `ToolDef` and in
    /// slot order — position is what a later phase's extraction screen picks
    /// by, `Routines`' own reason for keeping its ids ordered.
    ///
    /// An id `ToolDb` cannot resolve (a mod's tool file removed since a save
    /// referenced it) is dropped rather than surfaced as a hole, the same
    /// tolerance `ability_display_name`'s callers take on the routine side —
    /// there is no per-tool fallback to show in its place.
    pub fn installed_tools(&self) -> Vec<ToolDef> {
        let player = self.player_entity();
        let db = self.world.resource::<ToolDb>();
        self.world
            .get::<Tools>(player)
            .map(|tools| {
                tools
                    .0
                    .iter()
                    .filter_map(|id| db.get(id.as_str()).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_tool_slot_a_level_step_clamped_to_a_modest_cap() {
        // Base fills exactly one slot — the starter tool and nothing more,
        // since choosing the tool is the decision this phase's spec leans
        // on (decision 3). Contrast `player_routine_slots(1) == 2`: a
        // routine slot starts with one free.
        assert_eq!(player_tool_slots(1), 1);
        assert_eq!(
            player_tool_slots(crate::tuning::TOOL_SLOT_PER_LEVEL - 1),
            1,
            "no second slot before the first per-level step"
        );
        assert_eq!(
            player_tool_slots(crate::tuning::TOOL_SLOT_PER_LEVEL),
            2,
            "one slot, not two, at the first step — tools grow slower than routines"
        );
        assert_eq!(
            player_tool_slots(crate::tuning::TOOL_SLOT_PER_LEVEL * 3),
            crate::tuning::TOOL_SLOT_CAP as usize,
            "cap reached at three steps past base"
        );
        assert_eq!(
            player_tool_slots(9_999),
            crate::tuning::TOOL_SLOT_CAP as usize,
            "never above the cap, however high level climbs"
        );
    }
}
