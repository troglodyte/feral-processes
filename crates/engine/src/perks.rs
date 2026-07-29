use std::collections::HashMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

/// A permanent passive upgrade purchased with Perk Points (earned 1 per
/// player level-up — see `Game::award_player_xp`). Unlike a one-time
/// unlock, a perk can be bought repeatedly: each purchase adds another
/// level, and each level is worth exactly 1 point toward that perk's
/// bonus (see `components::Perks::level`) — a small, steady stack rather
/// than one big jump.
///
/// **This enum is the moddable seam's limit.** A perk's *catalogue* entry —
/// its name, its one-line description and what it costs — lives in
/// `assets/perks/*.ron` and is editable without touching Rust. Its *effect*
/// cannot: every variant below is a hook into a different formula, from
/// `forage_chance` to `taming::capture_chance`'s HP penalty to a direct
/// `Stats` write, and there is no shared shape to express in data the way
/// `SpeciesDef` or `ItemDef` have one. So a modder can rename, re-describe
/// and re-price the seven perks, but a new perk is still a new variant here
/// plus a hook wherever its effect belongs — see `CLAUDE.md`.
///
/// The catalogue keys off this enum rather than a string id so that a `.ron`
/// file naming a perk the build doesn't have simply fails to parse: no save
/// can ever hold a perk with nothing behind it.
///
/// **The order of these variants is part of the save format.** Saves are
/// bincode, which encodes an enum positionally (see `save.rs`), so
/// `PlayerSave::unlocked_perks` stores indices into this list — reordering
/// it would quietly turn one player's Attacker levels into Defender levels
/// on load. Append new variants at the end, or bump `SAVE_FORMAT_VERSION`.
/// The `.ron` files are unaffected either way: RON is self-describing and
/// names its variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Perk {
    /// Raises `Game::forage`'s success chance by
    /// `KEEN_SCAVENGER_BONUS_PER_LEVEL` per level.
    KeenScavenger,
    /// Slows Power (hunger) drain by `LOW_POWER_MODE_REDUCTION_PER_LEVEL`
    /// per level, down to a floor of 0 (hunger stops draining at all).
    LowPowerMode,
    /// Softens how much a decompile target's remaining Integrity counts
    /// against the attempt, by
    /// `EXPLOIT_FOCUS_HP_PENALTY_REDUCTION_PER_LEVEL` per level. Worth most
    /// against a program at full health and nothing at all against a drained
    /// one — deliberately a different axis from the `Decompiler` stat, which
    /// scales every attempt and already grows for free on level-up.
    ExploitFocus,
    /// Cuts `Game::craft`'s recipe costs by
    /// `LEAN_COMPILER_DISCOUNT_PER_LEVEL` of each item per level, down to a
    /// minimum of 1 each.
    LeanCompiler,
    /// +`ATTACKER_BONUS_PER_LEVEL` permanent ATK per level, applied directly
    /// to the player's `Stats` the moment it's bought (same as a level-up's
    /// stat bump).
    Attacker,
    /// +`DEFENDER_BONUS_PER_LEVEL` permanent DEF per level, applied directly
    /// to the player's `Stats` the moment it's bought.
    Defender,
    /// +`BUFFER_BONUS_PERCENT_PER_LEVEL` permanent max Integrity per level
    /// (floored at `BUFFER_MIN_BONUS_PER_LEVEL`), applied directly to the
    /// player's `Stats` the moment it's bought, fully healing them the same
    /// way a level-up does.
    Buffer,
}

impl Perk {
    /// Every perk the engine can apply, in the order the picker lists them.
    /// A perk with no `.ron` entry is dropped from that list by
    /// `PerkDb::catalogue` — this is what *can* be bought, not what is
    /// currently on offer.
    pub fn all() -> [Perk; 7] {
        [
            Perk::KeenScavenger,
            Perk::LowPowerMode,
            Perk::ExploitFocus,
            Perk::LeanCompiler,
            Perk::Attacker,
            Perk::Defender,
            Perk::Buffer,
        ]
    }
}

/// The authored half of a perk: what it's called, how it reads, and what it
/// costs. Everything a player sees in the picker, and nothing the engine
/// computes from — see `Perk` for why the effect stays in Rust.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerkDef {
    pub id: Perk,
    pub name: String,
    /// One line on what buying a level does, shown under the perk in the
    /// picker. Authored rather than derived from the tuning constant behind
    /// it, so a modder controls exactly how their perk reads.
    pub description: String,
    /// Perk Points spent per level — the same cost every time, however many
    /// levels you already have.
    pub cost: u32,
}

#[derive(Resource, Default)]
pub struct PerkDb {
    defs: HashMap<Perk, PerkDef>,
}

impl PerkDb {
    /// Loads every `*.ron` perk in `dir`. A malformed file is skipped with a
    /// returned warning rather than aborting the load, same as
    /// `AbilityDb::load_dir` — including a file naming a `Perk` this build
    /// doesn't have, which `ron` rejects as an unknown variant.
    ///
    /// A perk left without a def is not an error: it simply stops being
    /// offered. A save that already holds levels of it keeps them, and keeps
    /// the effect, since every effect reads the `Perks` component rather
    /// than this db.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = PerkDb::default();
        let mut warnings = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<PerkDef>(&text) {
                Ok(def) => {
                    if def.cost == 0 {
                        warnings.push(format!(
                            "skipped invalid perk file {path:?}: cost must be at least 1, or the \
                             perk could be bought without limit"
                        ));
                        continue;
                    }
                    db.defs.insert(def.id, def);
                }
                Err(e) => warnings.push(format!("skipped invalid perk file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, perk: Perk) -> Option<&PerkDef> {
        self.defs.get(&perk)
    }

    /// Every perk currently on offer, in `Perk::all()` order. Driven by that
    /// array rather than by `HashMap` iteration so the picker's numbering is
    /// the authored order and is stable between sessions.
    pub fn catalogue(&self) -> impl Iterator<Item = &PerkDef> {
        Perk::all().into_iter().filter_map(|p| self.defs.get(&p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perk_assets_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/perks")
    }

    #[test]
    fn every_shipped_perk_has_a_catalogue_entry() {
        let (db, warnings) = PerkDb::load_dir(&perk_assets_dir()).unwrap();
        assert!(
            warnings.is_empty(),
            "shipped perks should all parse: {warnings:?}"
        );
        for perk in Perk::all() {
            let def = db
                .get(perk)
                .unwrap_or_else(|| panic!("{perk:?} has no .ron file"));
            assert!(!def.name.is_empty(), "{perk:?} needs a name");
            assert!(!def.description.is_empty(), "{perk:?} needs a description");
        }
    }

    #[test]
    fn catalogue_order_follows_perk_all_not_hashmap_iteration() {
        let (db, _) = PerkDb::load_dir(&perk_assets_dir()).unwrap();
        let ids: Vec<Perk> = db.catalogue().map(|d| d.id).collect();
        assert_eq!(ids, Perk::all().to_vec());
    }

    #[test]
    fn a_malformed_perk_file_is_skipped_with_a_warning_not_a_panic() {
        let dir = std::env::temp_dir().join(format!("fp_perk_db_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("broken.ron"), "(id: NotAPerk, name: \"x\")").unwrap();
        std::fs::write(
            dir.join("free.ron"),
            "(id: Buffer, name: \"Free\", description: \"d\", cost: 0)",
        )
        .unwrap();
        std::fs::write(
            dir.join("ok.ron"),
            "(id: Attacker, name: \"Attacker\", description: \"d\", cost: 2)",
        )
        .unwrap();

        let (db, warnings) = PerkDb::load_dir(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(
            warnings.len(),
            2,
            "both bad files should warn: {warnings:?}"
        );
        assert!(
            db.get(Perk::Attacker).is_some(),
            "the good file should load"
        );
        assert!(
            db.get(Perk::Buffer).is_none(),
            "a free perk would be buyable without limit"
        );
        assert_eq!(db.catalogue().count(), 1);
    }
}
