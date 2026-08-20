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
    /// Opts this node out of `systems::node_payout` entirely: every
    /// completed cycle yields exactly 1, whatever the upgrade tier or zone
    /// depth. That curve was written for bulk salvage, where a Mk5 in zone 5
    /// paying 9 a cycle is the reward for having built and travelled; a node
    /// producing something consumed *one at a time* — a taming catalyst, a
    /// key — instead outruns its own sink within a zone or two.
    ///
    /// Banked items (`ItemDef::banked`) already bypass the curve for
    /// their own reason, so the two conditions are ORed rather than merged:
    /// one is a property of the item, this is a property of how the node
    /// produces it. The same item mined by a bulk node still scales.
    ///
    /// `#[serde(default)]` — false, the scaling curve, so existing structure
    /// files and mods are unaffected.
    #[serde(default)]
    pub flat_payout: bool,
}

fn default_output_capacity() -> u32 {
    crate::tuning::DEFAULT_OUTPUT_CAPACITY
}

/// Which group a structure lists under in the build menu.
///
/// Derived from the fields a def declares rather than authored, exactly like
/// `ItemDef::category` — so a modded structure groups by what it *does*
/// instead of by where its id happens to fall in the alphabet. Variant order
/// **is** menu order: shelter, then the things that produce, then the chain
/// that consumes what they produce, then the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StructureCategory {
    Home,
    /// Produces from nothing, given a program — see `StructureDef::work`.
    Extractor,
    /// Consumes its neighbours' output, given a program — see
    /// `StructureDef::assembles`.
    Assembler,
    Utility,
    Trade,
    Defence,
}

/// A structure's automated-crafting capability — see
/// `StructureDef::assembles` and `systems::assembler_system`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssembleDef {
    /// What this machine builds. Deliberately an *item id* and not a recipe:
    /// the machine runs that item's own `ItemDef::craftable.cost`, so there
    /// is exactly one recipe format in the game and a modder who adds a
    /// craftable item gets an automatable one for free.
    pub item: ItemId,
    /// Ticks of progress a completed unit costs, once the machine has a full
    /// batch of ingredients staged, room in its output, and a program
    /// assigned.
    pub ticks_per_unit: u32,
}

/// A structure's power-regeneration capability — see
/// `StructureDef::power_regen` and `systems::power_regen_system`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PowerRegenDef {
    /// Power (`components::PowerReserve::hunger`) restored per tick while the
    /// player is in range. Stacks additively across every in-range
    /// structure that sets it.
    pub per_tick: f32,
    /// Chebyshev distance (in tiles) the player must be within for this to
    /// run, same box-radius style as `RestDef::radius`.
    pub radius: i32,
}

/// A structure's trading post capability: sell any item here, and buy
/// specific items back. Everything is priced in the
/// `EconomyRole::TradeCurrency` item — never in the salvage the build
/// economy runs on.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TradeDef {
    /// This trader's multiplier on what an item is worth — the trader's half
    /// of a sale, where `ItemDef::value` is the item's half. At 1 the trader
    /// pays an item's full value; at 2 it pays double. Resolved against the
    /// item by `Game::sell_price`, which is the only thing that should read
    /// this field.
    pub sell_rate: u32,
    /// Items purchasable here, each as `(item, cost in the trade currency)`.
    pub buy: Vec<(ItemId, u32)>,
    /// Divisor applied to a tamed program's `Stats::power()` to price it
    /// when sold here in the trade currency — 10 pays a tenth of its power,
    /// rounded down, with a floor of 1.
    ///
    /// `None` (the default) means this trader deals in items only. A
    /// structure field rather than an engine constant so no code names a
    /// structure id and a modded trader can pay differently or refuse
    /// programs; defaulting to `None` so an existing structure file does not
    /// start buying creatures just because the game learned how.
    /// `Some(0)` is treated as `None` rather than dividing by zero.
    #[serde(default)]
    pub program_sell_divisor: Option<u32>,
}

/// A structure's rest capability — see `StructureDef::enables_rest` and
/// `Game::rest`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RestDef {
    /// Chebyshev distance (in tiles) the player must be within to rest
    /// using this structure.
    pub radius: i32,
    /// Items spent per rest, each as `(item, quantity)`, checked and taken
    /// after every other gate passes and before the rest ticks run (see
    /// `Game::rest`). Priced on the structure that grants rest rather than
    /// as a global rate, so a modded alternate rest structure can charge
    /// differently — or nothing. `#[serde(default)]` so a `RestDef` written
    /// before this field existed (including a mod's) still parses, as a
    /// free rest, exactly as before this field existed.
    #[serde(default)]
    pub cost: Vec<(ItemId, u32)>,
}

/// Marks a structure as temporary — see `StructureDef::temporary`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TemporaryDef {
    /// How many ordinary game-clock ticks this structure survives after
    /// being deployed before it automatically collapses. Ticks spent
    /// inside a `Game::rest` cycle don't count toward this (see
    /// `Game::tick_inner`) — resting near it doesn't wear it down any
    /// faster than just leaving it standing idle would.
    pub max_ticks: u32,
}

/// How fast a structure patches the rest of the base back up — see
/// `Game::total_repair_rate`. Deliberately has no `radius`: like
/// `raid_defense`, a repairer works base-wide from wherever it stands,
/// which is the only sensible reading given every non-Home structure has
/// to sit within `MAX_BUILD_DISTANCE_FROM_HOME` of the Home anyway.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct RepairDef {
    /// `Durability` restored to every deployed structure per upgrade tier,
    /// every `STRUCTURE_REGEN_INTERVAL` ticks. A tier-3 repairer restores
    /// three times this; two repairers add together.
    pub per_tier: u32,
}

/// A structure's upgrade path — see `Game::upgrade_structure`. The cost to
/// reach tier N is each amount in `cost` multiplied by N, so upgrades get
/// steadily more expensive without needing a per-tier table.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpgradeDef {
    pub max_tier: u32,
    pub cost: Vec<(ItemId, u32)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StructureDef {
    pub id: StructureId,
    pub name: String,
    /// One line on what this structure is for, shown in the build menu.
    /// Authored here rather than derived from the other fields so a modder
    /// controls exactly how their structure reads. `#[serde(default)]` so an
    /// existing mod file without it still parses — as an empty line, which
    /// the shipped-assets test refuses for anything in this repo.
    #[serde(default)]
    pub description: String,
    pub glyph: char,
    pub color: GlyphColor,
    pub build_cost: Vec<(ItemId, u32)>,
    /// If set, a tamed creature can be assigned to work this structure,
    /// producing `produces` every `ticks_per_unit` ticks.
    pub work: Option<WorkDef>,
    /// How many units this structure's output buffer holds before it clogs
    /// (see `components::Stock`). Top-level rather than inside `work`
    /// because an assembler declares `assembles` and no `work` block at all,
    /// and a storage building declares neither — both still need an output
    /// size. `#[serde(default = "default_output_capacity")]` so existing
    /// structure files (including mods) get a usable buffer rather than 0,
    /// which would clog on the first unit produced.
    #[serde(default = "default_output_capacity")]
    pub capacity: u32,
    /// Whether a posted program may empty a clogged machine's buffer into
    /// this structure — see `game::base::hauling`. A flag rather than "has a
    /// `Stock` and runs no job", because *every* deployed structure has a
    /// `Stock`: that rule would make a Home, a Shield and a Data Cache all
    /// depots. `#[serde(default)]` so existing structure files (including
    /// mods) written before this field existed still parse, as something a
    /// hauler ignores.
    #[serde(default)]
    pub stores: bool,
    /// If set, this structure automatically builds the named item from
    /// ingredients pulled out of its orthogonal neighbours' output buffers,
    /// once a program is assigned to it. Unlike `work`, which produces from
    /// nothing, this consumes — which is what lets machines form a chain.
    /// `#[serde(default)]` so existing structure files (including mods)
    /// written before this field existed still parse.
    #[serde(default)]
    pub assembles: Option<AssembleDef>,
    /// If set, this structure restores the player's Power every tick while
    /// they stand within `radius` tiles — no assigned worker and no input
    /// item, unlike `work`. `#[serde(default)]` so
    /// existing structure files (including mods) written before this field
    /// existed still parse (defaulting to no regeneration).
    #[serde(default)]
    pub power_regen: Option<PowerRegenDef>,
    /// What this structure needs from the base's Grid to run. Summed against
    /// every deployed structure's `power_supply` every tick; see
    /// `game::base::power`. A machine whose draw doesn't fit the base's
    /// remaining supply reports `MachineStatus::Unpowered` and makes no
    /// progress. `#[serde(default)]` so existing structure files (including
    /// mods) written before this field existed still parse, as a machine
    /// that draws nothing.
    #[serde(default)]
    pub power_draw: u32,
    /// What this structure contributes to the base's Grid — see
    /// `game::base::power`. A separate field from `power_regen` rather than
    /// a third member of `PowerRegenDef`, because they answer different
    /// questions and one structure (the Recharger) is about to carry both
    /// with different values: `power_regen` restores a creature's
    /// `PowerReserve`, a different resource entirely, while this feeds
    /// every machine on the base. `#[serde(default)]` so existing structure
    /// files (including mods) written before this field existed still
    /// parse, as a building that supplies nothing.
    #[serde(default)]
    pub power_supply: u32,
    /// Whether this structure issues contracts — see
    /// `Game::contract_board`. A plain `bool` rather than a block of its own,
    /// because a Broker has no per-structure configuration: what it offers is
    /// derived from the world seed, the sector and the clock, not authored on
    /// the building. `#[serde(default)]` so every existing structure file,
    /// including any mod, keeps parsing.
    #[serde(default)]
    pub issues_contracts: bool,
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
    /// Whether raids can target this structure. A non-raidable structure is
    /// spawned with no `Durability` component at all, which is what keeps
    /// `Game::raid_check` — whose target query is `With<Durability>` — from
    /// ever selecting it, and leaves `durability` above inert.
    /// `#[serde(default = "default_raidable")]` so existing structure files
    /// (including mods) stay raidable, exactly as before this field existed.
    #[serde(default = "default_raidable")]
    pub raidable: bool,
    /// How much this structure reduces raid damage by, for *every* raid
    /// against *any* deployed structure — not just itself — while it's
    /// standing (see `Game::raid_check`). Stacks additively across every
    /// deployed structure with this set, on top of whatever an assigned
    /// worker/guard already mitigates. `#[serde(default)]` so existing
    /// structure files (including mods) without this field contribute
    /// nothing, same as before it existed.
    #[serde(default)]
    pub raid_defense: u32,
    /// How many extra tamed-program (pet) slots this structure grants while
    /// it's deployed (see `Game::pet_capacity`). Stacks additively across
    /// every deployed structure that sets it, so several Data Caches each add
    /// their slots on top of the base pet cap. `#[serde(default)]` so existing
    /// structure files (including mods) contribute nothing, same as before it
    /// existed.
    #[serde(default)]
    pub pet_slot_bonus: u32,
    /// How many tiles this structure widens the base platform by while it's
    /// deployed (see `Game::build_radius`). Stacks additively across every
    /// deployed structure that sets it, so each Heap Pillar creeps the edge
    /// How many of this structure may stand at once. `0` — the default, and
    /// what every shipped structure but the Line Driver leaves it at — means
    /// no limit, so an existing file and any mod that never heard of the
    /// field is unaffected.
    ///
    /// This is where a structure whose *effect accumulates* is bounded, and
    /// the Pillar is the case it exists for: bounding growth by
    /// `MAX_BUILD_RADIUS_TILES` alone puts the limit ninety-six purchases
    /// away, which is no limit a player will ever meet. A count is the knob
    /// worth tuning, and it lives here rather than in `tuning.rs` because it
    /// is a property of the structure — a mod adds a capped structure by
    /// setting a number, not by editing the engine.
    #[serde(default)]
    pub max_deployed: u32,
    /// If set, `Game::rest` is only allowed while the player stands within
    /// this structure's `radius` — resting has no other way to happen.
    /// `#[serde(default)]` so existing structure files (including mods)
    /// without this field don't grant rest capability.
    #[serde(default)]
    pub enables_rest: Option<RestDef>,
    /// If set, this structure is temporary: it automatically collapses
    /// once `max_ticks` ordinary game-clock ticks have passed since it was
    /// deployed. `#[serde(default)]` so existing (permanent) structure
    /// files keep parsing.
    #[serde(default)]
    pub temporary: Option<TemporaryDef>,
    /// If set, this structure repairs every deployed structure — itself
    /// included — by `per_tier` times its own upgrade tier, every
    /// `STRUCTURE_REGEN_INTERVAL` ticks (see `Game::structure_regen`).
    /// Stacks additively across every deployed structure that sets it, the
    /// same way `raid_defense` and `pet_slot_bonus` do, and pairs with
    /// `upgrade` — without one the tier is always 1. `#[serde(default)]` so
    /// existing structure files (including mods) repair nothing, exactly as
    /// before this field existed.
    #[serde(default)]
    pub repair: Option<RepairDef>,
    /// If set, this structure can be upgraded through tiers (see
    /// `Game::upgrade_structure`). Each tier multiplies the structure's work
    /// payout and becomes its `ResourceNode::level`, so extraction gets more
    /// reliable as well as more productive. `#[serde(default)]` so existing
    /// structure files (including mods) stay un-upgradeable, exactly as
    /// before this field existed.
    #[serde(default)]
    pub upgrade: Option<UpgradeDef>,
    /// If true, owning one of these anywhere lets you extract a routine out
    /// of a program you own (see `Game::extract_routine`). Deliberately
    /// ownership, not proximity: the check is `Game::has_structure`, the
    /// same "have you built one" test a researched recipe's bench uses.
    /// `#[serde(default)]` so existing structure files (including mods)
    /// grant no extraction, exactly as before this field existed.
    #[serde(default)]
    pub extracts_routines: bool,
}

fn default_durability() -> u32 {
    crate::tuning::DEFAULT_STRUCTURE_DURABILITY
}

fn default_raidable() -> bool {
    true
}

#[derive(Resource, Default)]
pub struct StructureDb {
    structures: HashMap<StructureId, StructureDef>,
}

impl StructureDef {
    /// Whether this structure runs a job a program can be posted to — an
    /// extractor (`work`) or an assembler (`assembles`).
    ///
    /// Named once because three things have to agree about it and one of
    /// them already drifted: deploy inserts `components::MachineStatus` on a
    /// machine, `Game::accepts_a_program` decides what the cronjob menu
    /// offers, and the map colours an outline by that status. Deploy tested
    /// `work.is_some()` alone, so every assembler stood without a status —
    /// which silently cost it its stall log lines, its roster state and its
    /// outline, because every consumer treats a missing status as "not a
    /// machine" rather than as an error.
    pub fn runs_a_job(&self) -> bool {
        self.work.is_some() || self.assembles.is_some()
    }

    /// Which group this structure lists under. Checked in this order because
    /// the first match wins and the overlaps have a right answer: a bench
    /// that also defends is still where you go to build things, and the
    /// Compiler produces *and* gates a recipe but is an extractor first.
    ///
    /// Total by construction — a structure declaring none of these is
    /// utility, which is what a Data Cache or a Recharger Node is.
    pub fn category(&self) -> StructureCategory {
        if self.id == crate::HOME_STRUCTURE_ID {
            return StructureCategory::Home;
        }
        if self.work.is_some() {
            return StructureCategory::Extractor;
        }
        if self.assembles.is_some() {
            return StructureCategory::Assembler;
        }
        if self.trade.is_some() {
            return StructureCategory::Trade;
        }
        if self.raid_defense > 0 || self.repair.is_some() {
            return StructureCategory::Defence;
        }
        StructureCategory::Utility
    }
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

    /// Every loaded structure, grouped by `StructureCategory` and
    /// alphabetical by `name` inside each group.
    ///
    /// `HashMap` iteration order is randomized per-instance (a fresh seed
    /// each time a `StructureDb` is built, i.e. every new/loaded game), so
    /// without a sort here the build menu's `[1]`, `[2]`, ... numbering would
    /// shuffle unpredictably from one session to the next even though nothing
    /// about the mod files changed — the same digit could mean a
    /// 2-Core-Fragment Mining Node in one session and an 8-Core-Fragment
    /// Fabricator in the next.
    ///
    /// This used to pin `home`, `mining_node`, `research_node` and `compiler`
    /// first by id and sort the rest alphabetically. That put a modded
    /// structure wherever its id fell in the alphabet, which stopped being
    /// tolerable once the production chain landed: `assembly_bay` sorted
    /// third overall, ahead of every machine that feeds it. Grouping by what
    /// a structure *does* puts a mod's assembler with the assemblers for
    /// free, and costs only that the four hand-pinned ids now sort by name
    /// within their own group.
    pub fn all(&self) -> impl Iterator<Item = &StructureDef> {
        let mut defs: Vec<&StructureDef> = self.structures.values().collect();
        defs.sort_by(|a, b| {
            a.category()
                .cmp(&b.category())
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| a.id.cmp(&b.id))
        });
        defs.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> StructureDb {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/structures");
        StructureDb::load_dir(&dir)
            .expect("assets/structures should load")
            .0
    }

    /// Buying programs is opt-in per trader. The shipped Market takes them
    /// at a tenth of their power; every other structure — and any modded one
    /// written before the field existed — deals in items only, rather than
    /// silently gaining the ability to buy creatures.
    #[test]
    fn only_the_market_buys_programs_and_it_pays_a_tenth_of_power() {
        let db = test_db();
        let market = db.get("market").expect("black_market.ron should load");
        assert_eq!(
            market
                .trade
                .as_ref()
                .expect("the market trades")
                .program_sell_divisor,
            Some(10)
        );
        for def in db.all() {
            if def.id != "market"
                && let Some(trade) = &def.trade
            {
                assert_eq!(
                    trade.program_sell_divisor, None,
                    "{} should not buy programs",
                    def.id
                );
            }
        }
    }

    /// A structure file written before this field existed keeps parsing.
    #[test]
    fn a_trade_block_without_the_field_defaults_to_not_buying_programs() {
        let def: StructureDef = ron::from_str(
            r#"(
                id: "old_trader", name: "Old Trader", glyph: '$', color: Yellow,
                build_cost: [],
                work: None,
                trade: Some((sell_rate: 1, buy: [])),
            )"#,
        )
        .expect("an older trade block must still parse");
        assert_eq!(def.trade.unwrap().program_sell_divisor, None);
    }

    /// A structure file written before `capacity` existed gets the default
    /// output size. Defaulting to 0 instead would clog every existing mod's
    /// machines on the first unit they produced.
    #[test]
    fn a_structure_def_without_capacity_gets_the_default_output_size() {
        let def: StructureDef = ron::from_str(
            r#"(
                id: "old_node", name: "Old Node", glyph: '$', color: Brown,
                build_cost: [],
                work: None,
            )"#,
        )
        .expect("a file written before `capacity` existed must still parse");
        assert_eq!(def.capacity, crate::tuning::DEFAULT_OUTPUT_CAPACITY);
    }

    #[test]
    fn a_structure_def_may_set_its_own_output_capacity() {
        let def: StructureDef = ron::from_str(
            r#"(
                id: "big_node", name: "Big Node", glyph: '$', color: Brown,
                build_cost: [],
                work: None,
                capacity: 40,
            )"#,
        )
        .expect("an authored capacity must parse");
        assert_eq!(def.capacity, 40);
    }

    /// The mod-compatibility guarantee: a `RestDef` written before `cost`
    /// existed (or a modder who never touched it) still parses, and rests
    /// for free — today's behaviour, preserved by defaulting to empty
    /// rather than requiring every rest structure to price itself.
    #[test]
    fn a_rest_def_without_a_cost_field_defaults_to_a_free_rest() {
        let def: RestDef = ron::from_str("(radius: 7)").expect("an older RestDef must still parse");
        assert!(def.cost.is_empty());
    }

    #[test]
    fn the_data_cache_is_the_only_structure_granting_pet_slots() {
        let db = test_db();
        let cache = db.get("data_cache").expect("data_cache.ron should load");
        assert_eq!(cache.pet_slot_bonus, 5);
        for def in db.all() {
            if def.id != "data_cache" {
                assert_eq!(
                    def.pet_slot_bonus, 0,
                    "{} should not grant pet slots",
                    def.id
                );
            }
        }
    }
}
