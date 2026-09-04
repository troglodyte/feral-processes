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
    /// run, a Chebyshev box rather than a circle.
    pub radius: i32,
}

/// What a structure does for a program's needs — see `StructureDef::services`
/// and `game::base::offshift`.
///
/// Deliberately `PowerRegenDef`'s shape: a rate and a Chebyshev radius. What
/// differs is who it is for — that one refills the *player's* Power while
/// they stand near it, this refills an owned program's reserve while the
/// program stands near it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceDef {
    /// Which `needs::NeedDef` this refills. An id no file defines is inert —
    /// the empty-catalogue rule, held at the reader rather than at load.
    pub need: crate::needs::NeedId,
    /// Reserve restored per tick while the program is in range.
    pub per_tick: f32,
    /// Chebyshev distance in tiles, `power_regen`'s form. `0` is "standing on
    /// it or beside it", `hauling::at_station`'s reach.
    pub radius: i32,
}

/// What a structure does for a **downed** program — see
/// `StructureDef::recovery` and `systems::recovery_system`.
///
/// The third member of the `PowerRegenDef` / `ServiceDef` family: a rate and
/// a Chebyshev radius. What differs is what it aims at — `power_regen` at
/// the player's Power, `services` at an owned program's need reserves, this
/// at a downed program's Integrity.
///
/// **Not `RepairDef`, which is already taken** and means something else
/// entirely: how fast a Patch Node puts the base's *structures* back
/// together. Two fields both called `repair` on one type would be read as
/// one axis and unified by the next person through here.
///
/// `i32` rather than `PowerRegenDef`'s `f32` because `Stats::hp` is one, and
/// that type choice is what deletes half the clamp: a negative rate is
/// floored at zero and there is no non-finite case to guard. Do not "fix"
/// it back to a float for symmetry with the other two.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct RecoveryDef {
    /// Integrity restored per tick to a downed program within `radius`.
    pub per_tick: i32,
    /// Chebyshev distance in tiles, `power_regen`'s form rather than a
    /// circle. `0` is "standing on it".
    pub radius: i32,
}

impl RecoveryDef {
    /// What this actually restores per tick. Mod-supplied, so floored rather
    /// than trusted — a field named for repair must never damage.
    pub fn rate(&self) -> i32 {
        self.per_tick.max(0)
    }
}

impl ServiceDef {
    /// What this actually refills per tick.
    ///
    /// `per_tick` is mod-supplied, so it is **clamped at both ends rather
    /// than trusted**, exactly as `power_regen_system` clamps: a field named
    /// for refilling must never drain, and NaN would pin a reserve at the
    /// ceiling forever — `f32::min` returns the non-NaN operand, so a bare
    /// `.min(NEED_MAX)` silently yields the maximum. A non-finite rate skips
    /// the service entirely; a negative one floors at zero.
    pub fn rate(&self) -> Option<f32> {
        self.per_tick.is_finite().then(|| self.per_tick.max(0.0))
    }
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
    /// Names a one-cell sprite under `assets/sprites/` in place of the
    /// convention (see `sprite_name`) — an escape hatch for when the id
    /// isn't the filename wanted, not the normal way a structure gets art.
    /// `#[serde(default)]` so existing structure files (including mods)
    /// without this field keep parsing, and keep drawing under the id as
    /// they always have. No shipped structure authors one.
    #[serde(default)]
    pub sprite: Option<String>,
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
    /// What this structure does for an owned program's needs — see
    /// `needs::NeedDef` and `game::base::offshift`. An amenity: a program
    /// whose reserve has run critical walks here and stands until it is
    /// content. `#[serde(default)]` so every existing structure file,
    /// including any mod, keeps parsing as a building that services nothing.
    #[serde(default)]
    pub services: Vec<ServiceDef>,
    /// If set, this structure restores a downed program's Integrity every
    /// tick while that program stands within `radius` tiles — see
    /// `components::Downed` and `systems::recovery_system`. No assigned
    /// worker and no input, `power_regen`'s shape. A downed program walks
    /// here on its own and leaves when it is whole.
    ///
    /// **Distinct from `repair` below**, which is about the base's
    /// structures and not about programs at all. `#[serde(default)]` so
    /// every existing structure file, including any mod, keeps parsing as a
    /// building that recovers nobody — which is the pre-feature game.
    #[serde(default)]
    pub recovery: Option<RecoveryDef>,
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
    /// The item this structure burns to keep supplying, `None` meaning it
    /// burns nothing — see `systems::power_grid_system`. A supplier that
    /// names one carries `components::PowerFuel`, spends one unit off an
    /// orthogonally adjacent output buffer every `tuning::POWER_UPKEEP_TICKS`,
    /// and contributes **zero** `power_supply` — and, per
    /// `game::base::power::is_fuelled`, restores no `power_regen` trickle
    /// either — for as long as it cannot pay.
    ///
    /// Data decides *which* suppliers burn and *what* they burn; `tuning.rs`
    /// decides how fast. The Home deliberately leaves this unset: its free
    /// supply is the bootstrap, and a base with no Power Cells could never
    /// make the first one if the thing powering the Conduit needed one
    /// already.
    ///
    /// `#[serde(default)]` so every existing structure file, mods included,
    /// keeps parsing as a supplier that burns nothing. The census
    /// `tests::assets::every_burning_supplier_supplies_something_and_the_home_burns_nothing`
    /// asserts every shipped fuel id resolves to a real item — a typo here
    /// ships a supplier that can never be fed and never says why.
    #[serde(default)]
    pub power_upkeep: Option<ItemId>,
    /// Whether this structure issues contracts — see
    /// `Game::contract_board`. A plain `bool` rather than a block of its own,
    /// because a Broker has no per-structure configuration: what it offers is
    /// derived from the world seed, the sector and the clock, not authored on
    /// the building. `#[serde(default)]` so every existing structure file,
    /// including any mod, keeps parsing.
    #[serde(default)]
    pub issues_contracts: bool,
    /// Whether a squad can be dispatched from this structure — see
    /// `Game::sortie_reach`. `issues_contracts`' shape and for its reason: a
    /// Relay has no per-structure configuration either, since what it offers
    /// is derived from the world seed, the sector and the clock rather than
    /// authored on the building.
    ///
    /// **A flag rather than the engine naming `"relay"`.** Hardcoding the
    /// shipped id would put game content in Rust and make a mod's second
    /// dispatch structure impossible; this way the gate, the building and
    /// the board are all data. `#[serde(default)]` so every existing
    /// structure file, including any mod, keeps parsing.
    #[serde(default)]
    pub dispatches_sorties: bool,
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
    /// Extra bill lines that only apply once the current zone reaches
    /// `min_zone`: `(min_zone, item, base_qty)`. Additive on top of
    /// `build_cost`, which is implicitly `min_zone: 1` — a separate field
    /// rather than widening `build_cost`'s own tuple, because that would
    /// touch every one of the 30 shipped structure files and every mod's.
    ///
    /// For a `zone_portal` structure, `Game::structure_build_cost` ramps
    /// **each** line — `build_cost` and `zone_build_cost` alike — from the
    /// zone it was introduced in rather than from zone 1, so a line authored
    /// for a later sector charges its authored base the first zone it can
    /// legally be demanded, not an already-inflated number. For any other
    /// structure the qualifying lines are appended unramped. `#[serde(default)]`
    /// so every existing structure file, mods included, keeps parsing as
    /// authoring no later-sector lines.
    #[serde(default)]
    pub zone_build_cost: Vec<(u32, ItemId, u32)>,
    /// Whether the run's *first* one of these costs nothing —
    /// `build_cost` is waived until one has actually been raised, and
    /// charged in full for every one after it.
    ///
    /// The other modifier on `build_cost` beside `zone_portal` above, and
    /// resolved in the same one place: `Game::structure_build_cost`, which
    /// every quote, every filed request's stored bill and the removal
    /// refund all read. Data rather than an id in Rust because *which*
    /// structure onboards a run is a content decision — the shipped Broker
    /// is behind no research precisely so contracts can carry a new run,
    /// and its price was the last thing in the way.
    ///
    /// `#[serde(default)]` so every existing structure file, mods included,
    /// keeps parsing as a structure that is never free.
    #[serde(default)]
    pub first_free: bool,
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
    /// `Game::raid_check` — whose target query is `(With<Durability>,
    /// With<Structure>)` — from ever selecting it, and leaves `durability`
    /// above inert.
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
    /// How many of this structure may stand at once. `0` — the default, and
    /// what every shipped structure but the Line Driver leaves it at — means
    /// no limit, so an existing file and any mod that never heard of the
    /// field is unaffected.
    ///
    /// This is where a structure whose *effect accumulates* is bounded, and
    /// the Line Driver is the case it exists for: its `power_supply` adds to
    /// the base's Grid capacity from wherever it stands, with no radius to
    /// bound it and no worker slot to make deploying a second one cost
    /// anything beyond the build itself — a count is the only knob that
    /// paces it. It lives here rather than in `tuning.rs` because it is a
    /// property of the structure — a mod adds a capped structure by setting
    /// a number, not by editing the engine.
    #[serde(default)]
    pub max_deployed: u32,
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

    /// The name a sprite loader looks this structure up by: the `sprite`
    /// override when authored, the id otherwise. The one place this
    /// fallback is written — see `sprite`'s doc comment.
    pub fn sprite_name(&self) -> &str {
        self.sprite.as_deref().unwrap_or(&self.id)
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

    /// An amenity declares what it refills, and the block round-trips.
    #[test]
    fn an_amenity_exposes_what_it_services() {
        let db = test_db();
        let bay = db.get("defrag_bay").expect("defrag_bay.ron should load");
        let service = bay
            .services
            .first()
            .expect("an amenity that services nothing is not an amenity");
        assert_eq!(service.need, crate::needs::NeedId::from("coherence"));
        assert!(service.rate().is_some_and(|r| r > 0.0));
    }

    /// Asserted against a **shipped** file rather than a fixture: the point is
    /// that a mod's untouched `.ron` still loads, and every existing file in
    /// this repo is exactly that.
    #[test]
    fn a_structure_that_services_nothing_still_parses() {
        let db = test_db();
        let node = db.get("mining_node").expect("mining_node.ron should load");
        assert!(node.services.is_empty());
        // Same rule, same file, one field along: what `#[serde(default)]` is
        // for needs an assertion rather than an assumption.
        assert!(node.recovery.is_none());
    }

    /// `per_tick` is mod-supplied and a field named for repair must never
    /// damage. The `i32` is what makes this half a clamp instead of a whole
    /// one — there is no non-finite case to guard.
    #[test]
    fn a_negative_recovery_rate_floors_at_zero() {
        assert_eq!(
            RecoveryDef {
                per_tick: -4,
                radius: 0,
            }
            .rate(),
            0
        );
    }

    /// `per_tick` is mod-supplied, so it is clamped at both ends rather than
    /// trusted — a field named for refilling must never drain, and NaN would
    /// pin a reserve at the ceiling forever.
    #[test]
    fn a_nonsense_rate_is_refused_and_a_negative_one_floors() {
        let nonsense = |per_tick| ServiceDef {
            need: crate::needs::NeedId::from("coherence"),
            per_tick,
            radius: 0,
        };
        assert_eq!(nonsense(f32::NAN).rate(), None);
        assert_eq!(nonsense(f32::INFINITY).rate(), None);
        assert_eq!(nonsense(-5.0).rate(), Some(0.0));
        assert_eq!(nonsense(0.6).rate(), Some(0.6));
    }
}
