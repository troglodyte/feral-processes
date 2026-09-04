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
/// cannot: every variant below is a hook into a different formula, from the
/// hunger-decay multiplier to `taming::capture_chance`'s HP penalty to a
/// direct `Stats` write, and there is no shared shape to express in data the way
/// `SpeciesDef` or `ItemDef` have one. So a modder can rename, re-describe
/// and re-price the seventeen perks, but a new perk is still a new variant here
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
    /// Adds `KEEN_SCAVENGER_BONUS_PER_LEVEL` per level to a mining node's
    /// per-cycle success roll (`systems::mining_success_chance`), on top of
    /// what the node's own upgrade tier is worth.
    ///
    /// It used to boost the scan action, which was deleted for being
    /// unbounded income. The variant survived that rather than being removed
    /// because its position here is save format (see below), and the mining
    /// roll is where the same flavour — reading the terrain better — now
    /// pays off.
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
    /// Multiplies the magnitude of the player's own `Damage` abilities by
    /// `AFFINITY_PERK_BONUS_PER_LEVEL` per level. Scoped to the player's
    /// own invocations: a companion's affinity is its species' business, and a
    /// party-wide perk would multiply against it.
    DamageAffinity,
    /// As `DamageAffinity`, for `Heal`.
    HealAffinity,
    /// As `DamageAffinity`, for `Buff` — including saps, which are
    /// negative-power buffs.
    BuffAffinity,
    /// As `DamageAffinity`, for `Debuff`.
    DebuffAffinity,
    /// As `DamageAffinity`, for `Drain`'s damage. Its `heal_fraction`
    /// rides the damage dealt and is not scaled again.
    DrainAffinity,
    /// Cuts what each source adds to Trace by
    /// `OBFUSCATION_REDUCTION_PER_LEVEL` per level, floored so a rise is
    /// never cancelled outright. Deliberately unlike `LowPowerMode`, which
    /// is allowed to stop hunger draining entirely: Power has a structural
    /// answer already (a Recharger Node deletes it), while Trace is the
    /// Stack's only escalation pressure, so a perk that zeroed it would
    /// turn depth into free ground.
    Obfuscation,
    /// +`PROCESS_POOL_SLOTS_PER_LEVEL` roster slots per level, through the
    /// same `Game::pet_capacity` a Data Cache's `pet_slot_bonus` feeds — so
    /// what the perk buys survives losing the structures.
    ProcessPool,
    /// Adds `TEARDOWN_SALVAGE_PER_LEVEL` per level, once a bonus a kill's
    /// own `work_resource` drop took directly. That drop is gone —
    /// `Game::leave_downed_program` replaced it — so this perk's term now
    /// lives in `Game::extraction_yield`'s formula (the tool that consumes
    /// a downed program): a flat addend to the unit count, never a second
    /// `GameRng` draw, the same discipline the retired roll followed.
    Teardown,
    /// Adds `FAILOVER_REPAIR_PER_LEVEL` per level to the base-wide repair
    /// rate (`Game::total_repair_rate`), which is what a Patch Node
    /// contributes to — so a base with no repairer at all still mends.
    Failover,
    /// Adds `QUALITY_PERK_PER_LEVEL` per level to the floor a compiled copy
    /// of gear rolls its quality off (`Game::craft_quality_floor`) — the
    /// player-agency half of the bench term, and the only input to that
    /// floor that is not a building.
    ///
    /// Read on demand at the compile rather than applied on purchase, unlike
    /// `Attacker` and its two siblings: what it is worth is a property of
    /// each copy compiled after it, so gear already carried keeps the
    /// quality it was compiled at.
    TightenTolerances,
    /// Adds `TARGET_LOCK_ACCURACY_PER_LEVEL` Accuracy per level to every
    /// attack the player makes, through `Game::accuracy_bonus`.
    ///
    /// The player's half of the accuracy axis, and the only one that is not
    /// a piece of gear. Hooked at `accuracy_bonus` rather than at the roll
    /// because that is where the flat Accuracy sources meet — the same
    /// reason `Obfuscation` is read inside `raise_trace` rather than at the
    /// six things that raise Trace.
    ///
    /// Uncapped like every perk, and safe to leave uncapped: `HIT_CHANCE_MAX`
    /// keeps the *benefit* short of a guaranteed landing however many levels
    /// are bought, so this asymptotes rather than terminating the way a
    /// four-level knob would.
    TargetLock,
}

impl Perk {
    /// Every perk the engine can apply, in the order the picker lists them.
    /// A perk with no `.ron` entry is dropped from that list by
    /// `PerkDb::catalogue` — this is what *can* be bought, not what is
    /// currently on offer.
    pub fn all() -> [Perk; 18] {
        [
            Perk::KeenScavenger,
            Perk::LowPowerMode,
            Perk::ExploitFocus,
            Perk::LeanCompiler,
            Perk::Attacker,
            Perk::Defender,
            Perk::Buffer,
            Perk::DamageAffinity,
            Perk::HealAffinity,
            Perk::BuffAffinity,
            Perk::DebuffAffinity,
            Perk::DrainAffinity,
            Perk::Obfuscation,
            Perk::ProcessPool,
            Perk::Teardown,
            Perk::Failover,
            Perk::TightenTolerances,
            Perk::TargetLock,
        ]
    }

    /// Which affinity category this perk multiplies, or `None` for the
    /// twelve perks that do something else entirely. The one hook all five
    /// affinity perks share — they have a common shape, unlike the perks
    /// above them, so they get a common mapping rather than five bespoke
    /// arms in `unlock_perk`.
    pub fn affinity_kind(self) -> Option<crate::abilities::AffinityKind> {
        use crate::abilities::AffinityKind;
        match self {
            Perk::DamageAffinity => Some(AffinityKind::Damage),
            Perk::HealAffinity => Some(AffinityKind::Heal),
            Perk::BuffAffinity => Some(AffinityKind::Buff),
            Perk::DebuffAffinity => Some(AffinityKind::Debuff),
            Perk::DrainAffinity => Some(AffinityKind::Drain),
            _ => None,
        }
    }
}

/// What buying a level of a stat perk grants, for the three perks whose
/// effect is applied **at purchase** rather than read at a formula.
///
/// The gain is returned rather than written so the magnitudes — including
/// `Buffer`'s percentage-of-current-max and its floor — live beside the
/// perk, while the `Stats` write stays at the one site that owns it
/// (`Game::unlock_perk`). Composition, not a second writer of `Stats`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatGain {
    Atk(i32),
    Mitigation(i32),
    /// Raised max Integrity. The buyer is healed to the new maximum, the
    /// same way a level-up does — that is the caller's half.
    MaxHp(i32),
}

/// How many levels of `perk` these perks are held at — 0 with no component
/// at all, which is what makes every query below safe to ask of an entity
/// that has never bought anything.
fn level(perks: Option<&crate::components::Perks>, perk: Perk) -> u32 {
    perks.map(|p| p.level(perk)).unwrap_or(0)
}

// ─── What each perk is worth ────────────────────────────────────────────
//
// One named query per perk, and **the whole list of them is the census**:
// this is the one place that answers "which formula does this perk touch",
// which the enum's doc comments above describe but cannot keep in step.
// The module owns the answer and the call site owns the application —
// `needs::strain` and `party::role_of`'s shape.
//
// Two signature families, and the difference is not arbitrary. Most take
// the player's `Perks` and name their own variant, so no call site says
// `Perk::` at all. Two take a bare **level** instead, because the level
// travels to a formula the player may not be the subject of: a program
// compiling at a bench (`CraftOrder`) and a program working a node
// (`CycleModifiers`) each carry the player's level to a place that has no
// player to read. Giving those two a `Perks` argument would quietly make a
// program's cycle read the player's perks.
//
// The three `StatGain` perks are here too, in the only shape they can
// take: they are applied once at purchase and never read again.

/// The multiplier on Power (hunger) drain — `Perk::LowPowerMode`.
///
/// **Floored at 0**, so enough levels stop hunger draining entirely. That
/// is deliberate and unlike `trace_after_obfuscation` below: Power already
/// has a structural answer in a Recharger Node, so a perk reaching the same
/// place buys nothing the game did not already sell.
pub fn power_drain_multiplier(perks: Option<&crate::components::Perks>) -> f32 {
    (1.0 - crate::tuning::LOW_POWER_MODE_REDUCTION_PER_LEVEL
        * level(perks, Perk::LowPowerMode) as f32)
        .max(0.0)
}

/// `amount` of Trace after `Perk::Obfuscation`'s reduction, **floored at 1
/// whenever there was anything to reduce**.
///
/// The floor is the design, and the reason this is a whole-value query
/// rather than a multiplier: Trace is the Stack's only escalation pressure,
/// so however many levels are stacked, descending still costs something. A
/// level count past the point the reduction reaches 1.0 saturates the
/// conversion to 0 and the clamp lifts it back to 1, which is why the
/// arithmetic needs no ceiling of its own.
pub fn trace_after_obfuscation(perks: Option<&crate::components::Perks>, amount: u32) -> u32 {
    let level = level(perks, Perk::Obfuscation);
    if level == 0 || amount == 0 {
        return amount;
    }
    let kept = 1.0 - crate::tuning::OBFUSCATION_REDUCTION_PER_LEVEL * level as f32;
    ((amount as f32 * kept).round() as u32).clamp(1, amount)
}

/// What one line of a recipe costs after `Perk::LeanCompiler`, **floored at
/// 1** — a recipe line the perk erased entirely would make the item free.
///
/// A whole-value query for `trace_after_obfuscation`'s reason: the floor is
/// per line, so a caller handed a bare discount has to remember to apply it.
pub fn discounted_craft_cost(perks: Option<&crate::components::Perks>, qty: u32) -> u32 {
    let discount =
        crate::tuning::LEAN_COMPILER_DISCOUNT_PER_LEVEL * level(perks, Perk::LeanCompiler);
    qty.saturating_sub(discount).max(1)
}

/// What `Perk::ExploitFocus` shaves off a decompile target's remaining-HP
/// penalty.
pub fn decompile_hp_penalty_reduction(perks: Option<&crate::components::Perks>) -> f32 {
    crate::tuning::EXPLOIT_FOCUS_HP_PENALTY_REDUCTION_PER_LEVEL
        * level(perks, Perk::ExploitFocus) as f32
}

/// Extra roster slots from `Perk::ProcessPool` — the one term in
/// `Game::pet_capacity` a raid cannot take back.
pub fn roster_slot_bonus(perks: Option<&crate::components::Perks>) -> usize {
    level(perks, Perk::ProcessPool) as usize * crate::tuning::PROCESS_POOL_SLOTS_PER_LEVEL
}

/// What `Perk::Teardown` is worth — added to `Game::extraction_yield`'s
/// unit count as a plain addend, never a second `GameRng` draw (see the
/// variant's own doc).
pub fn salvage_bonus(perks: Option<&crate::components::Perks>) -> u32 {
    crate::tuning::TEARDOWN_SALVAGE_PER_LEVEL * level(perks, Perk::Teardown)
}

/// What `Perk::Failover` adds to the base-wide repair rate, so a base with
/// no repairer standing still mends.
pub fn repair_rate_bonus(perks: Option<&crate::components::Perks>) -> u32 {
    crate::tuning::FAILOVER_REPAIR_PER_LEVEL * level(perks, Perk::Failover)
}

/// Flat Accuracy from `Perk::TargetLock`, added to every attack the player
/// makes. Uncapped, and safe to leave uncapped — `HIT_CHANCE_MAX` keeps the
/// benefit short of a guaranteed landing however many levels are bought.
pub fn accuracy_bonus(perks: Option<&crate::components::Perks>) -> i32 {
    crate::tuning::TARGET_LOCK_ACCURACY_PER_LEVEL * level(perks, Perk::TargetLock) as i32
}

/// What the player's affinity perk for `kind` is worth, raw — the caller
/// adds it to `AFFINITY_NEUTRAL` and applies `AFFINITY_MAX`, since the
/// neutral point and the ceiling belong to the affinity system rather than
/// to the perk.
///
/// The five affinity perks are the one family with a common shape, so they
/// get one query rather than five, mapping through `AffinityKind` in both
/// directions.
pub fn affinity_bonus(
    perks: Option<&crate::components::Perks>,
    kind: crate::abilities::AffinityKind,
) -> f32 {
    kind.perk_bonus_per_level() * level(perks, kind.perk()) as f32
}

/// What `level` levels of `Perk::KeenScavenger` add to a mining node's
/// per-cycle success roll.
///
/// Takes a level rather than the component: the roll belongs to whoever is
/// working the node, and the player's level reaches it through
/// `CycleModifiers`.
pub fn mining_roll_bonus(level: u32) -> f64 {
    level as f64 * crate::tuning::KEEN_SCAVENGER_BONUS_PER_LEVEL
}

/// What `level` levels of `Perk::TightenTolerances` add to the floor a
/// compiled copy rolls its quality off.
///
/// Takes a level for `mining_roll_bonus`' reason — a program compiling at a
/// bench has none of its own, and `CraftOrder` is what carries the player's.
pub fn quality_floor_bonus(level: u32) -> u32 {
    level * crate::tuning::QUALITY_PERK_PER_LEVEL as u32
}

/// What buying a level of `perk` writes straight into the buyer's `Stats`,
/// or `None` for the fifteen perks read at a formula instead.
///
/// `current_max_hp` is only read by `Buffer`, whose gain is a percentage of
/// what the buyer already has — **floored at
/// `BUFFER_MIN_BONUS_PER_LEVEL`**, so an early purchase is worth taking.
pub fn purchase_stat_gain(perk: Perk, current_max_hp: i32) -> Option<StatGain> {
    match perk {
        Perk::Attacker => Some(StatGain::Atk(crate::tuning::ATTACKER_BONUS_PER_LEVEL)),
        Perk::Defender => Some(StatGain::Mitigation(
            crate::tuning::DEFENDER_BONUS_PER_LEVEL,
        )),
        Perk::Buffer => Some(StatGain::MaxHp(
            ((current_max_hp as f32 * crate::tuning::BUFFER_BONUS_PERCENT_PER_LEVEL).round()
                as i32)
                .max(crate::tuning::BUFFER_MIN_BONUS_PER_LEVEL),
        )),
        _ => None,
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

/// One labelled section of the perk picker, and the whole of what the
/// screen's layout is authored from: the heading, which perks sit under it,
/// and — by the section's position in `assets/perks/groups.ron` — where it
/// sits in the list.
///
/// One statement rather than a `group:` string repeated across seventeen
/// files, because membership alone does not order anything: a per-perk label
/// would need a second rule for which heading comes first, and two authored
/// halves of one layout drift.
///
/// Deliberately *not* `Perk::all()`'s order. That array mirrors the enum's
/// declaration order, which is save format (bincode encodes a variant
/// positionally), so it cannot be reshuffled to read better — see
/// `Perk::all` and `the_original_seven_perks_keep_their_positions`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerkGroupDef {
    pub name: String,
    pub perks: Vec<Perk>,
}

/// The one file in `assets/perks/` that is not a perk. It shares the
/// directory and the extension with the catalogue entries, so `load_dir`
/// skips the name explicitly — exactly as `HelpDb::load_dir` skips
/// `assets/help/README.md`, and for the same reason: read by the ordinary
/// loop it would fail to parse and warn on every startup.
const GROUPS_FILE: &str = "groups.ron";

#[derive(Resource, Default)]
pub struct PerkDb {
    defs: HashMap<Perk, PerkDef>,
    groups: Vec<PerkGroupDef>,
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
            if path.file_name().and_then(|n| n.to_str()) == Some(GROUPS_FILE) {
                match ron::from_str::<Vec<PerkGroupDef>>(&text) {
                    Ok(groups) => db.groups = groups,
                    // A broken layout costs the headings and nothing else:
                    // `grouped` falls back to one unlabelled run, so every
                    // perk stays buyable.
                    Err(e) => {
                        warnings.push(format!("skipped invalid perk group file {path:?}: {e}"))
                    }
                }
                continue;
            }
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

    /// The picker's sections, in the order `groups.ron` lists them, each
    /// paired with the defs under it. Anything no group names trails the
    /// list in a section whose name is empty — a typo in the layout file
    /// costs a heading, never a row, since a row is what the player spends
    /// points at.
    ///
    /// With no group file at all that trailing section is the *only* one,
    /// which is the flat list the screen drew before headings existed:
    /// deleting `groups.ron` restores it exactly.
    pub fn grouped(&self) -> Vec<(&str, Vec<&PerkDef>)> {
        let mut sections = Vec::new();
        let mut placed = Vec::new();
        for group in &self.groups {
            let mut defs: Vec<&PerkDef> = Vec::new();
            for perk in &group.perks {
                if placed.contains(perk) {
                    continue;
                }
                if let Some(def) = self.defs.get(perk) {
                    placed.push(*perk);
                    defs.push(def);
                }
            }
            if !defs.is_empty() {
                sections.push((group.name.as_str(), defs));
            }
        }
        let rest: Vec<&PerkDef> = Perk::all()
            .into_iter()
            .filter(|p| !placed.contains(p))
            .filter_map(|p| self.defs.get(&p))
            .collect();
        if !rest.is_empty() {
            sections.push(("", rest));
        }
        sections
    }

    /// Every perk currently on offer, in picker order — `grouped` flattened,
    /// so the numbering a player types against cannot disagree with the
    /// order the sections drew. Not `HashMap` iteration order, which would
    /// move between sessions.
    pub fn catalogue(&self) -> impl Iterator<Item = &PerkDef> {
        self.grouped().into_iter().flat_map(|(_, defs)| defs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Perks;

    fn perk_assets_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/perks")
    }

    /// A fresh empty directory to build a fixture catalogue in. Named per
    /// test as well as per process, since several of these run at once.
    fn scratch_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("fp_perk_db_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_perk(dir: &Path, file: &str, id: Perk) {
        std::fs::write(
            dir.join(file),
            format!("(id: {id:?}, name: \"{id:?}\", description: \"d\", cost: 2)"),
        )
        .unwrap();
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

    /// The shipped layout has to account for every perk, or one is drawn in
    /// the trailing unlabelled bucket — which is the graceful *failure*, not
    /// the shipped shape. An eighteenth perk added to `Perk::all()` and
    /// forgotten in `groups.ron` fails here rather than appearing under no
    /// heading in the game.
    #[test]
    fn every_shipped_perk_sits_under_a_heading() {
        let (db, warnings) = PerkDb::load_dir(&perk_assets_dir()).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");

        let grouped = db.grouped();
        assert!(
            grouped.iter().all(|(name, _)| !name.is_empty()),
            "a section with no name is the orphan bucket, holding {:?}",
            grouped
                .iter()
                .find(|(name, _)| name.is_empty())
                .map(|(_, defs)| defs.iter().map(|d| d.id).collect::<Vec<_>>()),
        );
        assert_eq!(
            db.catalogue().count(),
            Perk::all().len(),
            "the sections between them list every perk exactly once"
        );
    }

    /// The picker's numbering is the layout file's order, and deliberately
    /// not `Perk::all()`'s — that array mirrors the save format and cannot be
    /// reshuffled to read better. Not `HashMap` iteration either, which would
    /// renumber the screen between sessions.
    #[test]
    fn catalogue_order_follows_the_group_file() {
        let (db, _) = PerkDb::load_dir(&perk_assets_dir()).unwrap();
        let ids: Vec<Perk> = db.catalogue().map(|d| d.id).collect();
        let from_sections: Vec<Perk> = db
            .grouped()
            .iter()
            .flat_map(|(_, defs)| defs.iter().map(|d| d.id))
            .collect();
        assert_eq!(ids, from_sections);
        assert_ne!(
            ids,
            Perk::all().to_vec(),
            "the shipped layout reorders the list; if it stops doing so this \
             test is no longer saying anything"
        );
    }

    /// The group file shares the directory and the extension with the
    /// seventeen catalogue entries, so `load_dir` has to know its name — the
    /// same explicit skip `HelpDb::load_dir` makes for `assets/help/README.md`.
    /// Left to the ordinary loop it would fail to parse as a `PerkDef` and
    /// warn on every startup.
    #[test]
    fn the_group_file_is_not_read_as_a_perk() {
        let dir = scratch_dir("groups_skip");
        write_perk(&dir, "attacker.ron", Perk::Attacker);
        std::fs::write(
            dir.join(GROUPS_FILE),
            "[(name: \"Combat\", perks: [Attacker])]",
        )
        .unwrap();

        let (db, warnings) = PerkDb::load_dir(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        assert!(
            warnings.is_empty(),
            "the group file is catalogue metadata, not a malformed perk: {warnings:?}"
        );
        assert_eq!(db.catalogue().count(), 1);
    }

    /// The whole point of the file: it is the one statement of a section's
    /// label, its membership and where it sits in the list, so the picker's
    /// order is the file's order rather than `Perk::all()`'s.
    #[test]
    fn the_group_file_decides_the_pickers_order() {
        let dir = scratch_dir("groups_order");
        write_perk(&dir, "attacker.ron", Perk::Attacker);
        write_perk(&dir, "keen.ron", Perk::KeenScavenger);
        std::fs::write(
            dir.join(GROUPS_FILE),
            "[(name: \"Combat\", perks: [Attacker]), \
             (name: \"Workshop\", perks: [KeenScavenger])]",
        )
        .unwrap();

        let (db, _) = PerkDb::load_dir(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        let labels: Vec<&str> = db.grouped().iter().map(|(name, _)| *name).collect();
        assert_eq!(labels, vec!["Combat", "Workshop"]);
        assert_eq!(
            db.catalogue().map(|d| d.id).collect::<Vec<_>>(),
            vec![Perk::Attacker, Perk::KeenScavenger],
            "Attacker sits at index 4 of `Perk::all()` and Keen Scavenger at 0 — \
             the flattened order has to come from the group file, not from there"
        );
    }

    /// A perk no group names is still on offer, in a trailing unlabelled
    /// bucket. A typo in the group file costs a heading, never a perk: the
    /// row is what the player spends points at, and dropping it silently is
    /// the failure worth engineering against.
    #[test]
    fn a_perk_no_group_names_trails_the_list_unlabelled() {
        let dir = scratch_dir("groups_orphan");
        write_perk(&dir, "attacker.ron", Perk::Attacker);
        write_perk(&dir, "keen.ron", Perk::KeenScavenger);
        std::fs::write(
            dir.join(GROUPS_FILE),
            "[(name: \"Combat\", perks: [Attacker])]",
        )
        .unwrap();

        let (db, _) = PerkDb::load_dir(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        let grouped = db.grouped();
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[1].0, "", "an orphan bucket has no heading to draw");
        assert_eq!(
            grouped[1].1.iter().map(|d| d.id).collect::<Vec<_>>(),
            vec![Perk::KeenScavenger]
        );
    }

    /// No group file at all is the pre-grouping screen exactly: one
    /// unlabelled run in `Perk::all()` order. Deleting `groups.ron` restores
    /// the flat list the same supported way deleting `assets/sectors/`
    /// restores undifferentiated zones.
    #[test]
    fn no_group_file_leaves_the_catalogue_flat() {
        let dir = scratch_dir("groups_absent");
        write_perk(&dir, "attacker.ron", Perk::Attacker);
        write_perk(&dir, "keen.ron", Perk::KeenScavenger);

        let (db, warnings) = PerkDb::load_dir(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        assert!(warnings.is_empty(), "{warnings:?}");
        let grouped = db.grouped();
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped[0].0, "");
        assert_eq!(
            grouped[0].1.iter().map(|d| d.id).collect::<Vec<_>>(),
            vec![Perk::KeenScavenger, Perk::Attacker],
            "`Perk::all()` order, which is where the flat list came from"
        );
    }

    /// A malformed group file must cost the headings and nothing else —
    /// every perk stays buyable. `SpeciesDb::load_dir`'s rule, applied to a
    /// file that is presentation rather than content.
    #[test]
    fn a_malformed_group_file_costs_the_headings_and_not_the_perks() {
        let dir = scratch_dir("groups_broken");
        write_perk(&dir, "attacker.ron", Perk::Attacker);
        std::fs::write(dir.join(GROUPS_FILE), "[(name: \"Combat\", perks: [Nope])]").unwrap();

        let (db, warnings) = PerkDb::load_dir(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(db.catalogue().count(), 1);
        assert_eq!(db.grouped()[0].0, "");
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

    /// Every perk, and the one query that answers what it is worth.
    ///
    /// **This match is the census.** It is exhaustive on purpose —
    /// `cell_mark`'s rule — so a nineteenth perk fails to compile here until
    /// it has a home in the module above, which is what keeps this file the
    /// answer to "which formula does this perk touch" rather than a list
    /// that was true once. The enum's own doc comments say the same thing in
    /// prose, and prose is exactly what cannot be kept in step.
    ///
    /// Each arm asserts the query *moves*, so a perk wired to the wrong
    /// constant — or to one that is zero — fails as loudly as a missing one.
    fn one_level_is_worth_something(perk: Perk) -> bool {
        let bought = Perks {
            points: 0,
            unlocked: vec![perk],
        };
        let one = Some(&bought);
        let none = None;
        match perk {
            Perk::LowPowerMode => power_drain_multiplier(one) < power_drain_multiplier(none),
            Perk::Obfuscation => {
                trace_after_obfuscation(one, 100) < trace_after_obfuscation(none, 100)
            }
            Perk::LeanCompiler => discounted_craft_cost(one, 5) < discounted_craft_cost(none, 5),
            Perk::ExploitFocus => {
                decompile_hp_penalty_reduction(one) > decompile_hp_penalty_reduction(none)
            }
            Perk::ProcessPool => roster_slot_bonus(one) > roster_slot_bonus(none),
            Perk::Teardown => salvage_bonus(one) > salvage_bonus(none),
            Perk::Failover => repair_rate_bonus(one) > repair_rate_bonus(none),
            Perk::TargetLock => accuracy_bonus(one) > accuracy_bonus(none),
            Perk::DamageAffinity
            | Perk::HealAffinity
            | Perk::BuffAffinity
            | Perk::DebuffAffinity
            | Perk::DrainAffinity => {
                let kind = perk
                    .affinity_kind()
                    .expect("an affinity perk names its kind");
                affinity_bonus(one, kind) > affinity_bonus(none, kind)
            }
            // The two that travel as a bare level, because the formula's
            // subject may be a program rather than the player.
            Perk::KeenScavenger => mining_roll_bonus(1) > mining_roll_bonus(0),
            Perk::TightenTolerances => quality_floor_bonus(1) > quality_floor_bonus(0),
            // The three applied once at purchase and never read again.
            Perk::Attacker | Perk::Defender | Perk::Buffer => {
                purchase_stat_gain(perk, 100).is_some()
            }
        }
    }

    #[test]
    fn every_perk_has_a_query_that_answers_what_it_is_worth() {
        for perk in Perk::all() {
            assert!(
                one_level_is_worth_something(perk),
                "{perk:?} buys nothing its query can see"
            );
        }
    }

    /// Power is the one meter a perk may switch off entirely: a Recharger
    /// Node already deletes it, so the perk reaching the same place sells
    /// nothing new. Contrast `obfuscation_never_cancels_a_trace_rise`.
    #[test]
    fn low_power_mode_can_stop_hunger_draining_altogether() {
        let maxed = Perks {
            points: 0,
            unlocked: vec![Perk::LowPowerMode; 500],
        };
        assert_eq!(
            power_drain_multiplier(Some(&maxed)),
            0.0,
            "the multiplier floors at 0 rather than going negative and refilling"
        );
    }

    /// Trace is the Stack's only escalation pressure, so descending always
    /// costs something however many levels are stacked — the floor is the
    /// design, not a rounding artefact.
    #[test]
    fn obfuscation_never_cancels_a_trace_rise() {
        let maxed = Perks {
            points: 0,
            unlocked: vec![Perk::Obfuscation; 500],
        };
        assert_eq!(trace_after_obfuscation(Some(&maxed), 40), 1);
        assert_eq!(
            trace_after_obfuscation(Some(&maxed), 0),
            0,
            "a rise of nothing stays nothing — the floor lifts a reduction, \
             not an absence"
        );
    }

    /// A recipe line the discount erased would make the item free, which is
    /// the infinite-Credit fault `caravan_unit_cost` guards from the other
    /// side.
    #[test]
    fn lean_compiler_never_takes_a_recipe_line_to_zero() {
        let maxed = Perks {
            points: 0,
            unlocked: vec![Perk::LeanCompiler; 500],
        };
        assert_eq!(discounted_craft_cost(Some(&maxed), 8), 1);
    }

    /// `Buffer`'s gain is a percentage of what the buyer already has, so an
    /// early purchase would be worth almost nothing without the floor.
    #[test]
    fn buffer_is_worth_taking_at_a_low_maximum() {
        assert_eq!(
            purchase_stat_gain(Perk::Buffer, 10),
            Some(StatGain::MaxHp(crate::tuning::BUFFER_MIN_BONUS_PER_LEVEL)),
            "the percentage is worth 0 here; the floor is what it pays"
        );
        let big = purchase_stat_gain(Perk::Buffer, 10_000);
        assert!(
            matches!(big, Some(StatGain::MaxHp(n)) if n > crate::tuning::BUFFER_MIN_BONUS_PER_LEVEL),
            "and a developed player is paid the percentage, not the floor: {big:?}"
        );
    }

    /// The queries answer for an entity that has never bought anything, so
    /// no call site needs a branch — the property every `Option` in the
    /// signatures is there for.
    #[test]
    fn no_perks_at_all_is_worth_exactly_nothing() {
        assert_eq!(power_drain_multiplier(None), 1.0);
        assert_eq!(trace_after_obfuscation(None, 12), 12);
        assert_eq!(discounted_craft_cost(None, 3), 3);
        assert_eq!(roster_slot_bonus(None), 0);
        assert_eq!(salvage_bonus(None), 0);
        assert_eq!(repair_rate_bonus(None), 0);
        assert_eq!(accuracy_bonus(None), 0);
        assert_eq!(decompile_hp_penalty_reduction(None), 0.0);
        assert_eq!(mining_roll_bonus(0), 0.0);
        assert_eq!(quality_floor_bonus(0), 0);
    }
}
