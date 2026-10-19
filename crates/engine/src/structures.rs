use std::collections::HashMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::components::GlyphColor;
use crate::items::ItemId;

pub type StructureId = String;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecipeDef {
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
    /// Research that unlocks picking this recipe on a machine. `None`: always
    /// available. `#[serde(default)]` so existing and modded files parse.
    #[serde(default)]
    pub research: Option<crate::research::ResearchId>,
    /// Opts the recipe out of the upgrade-tier scaling of its yield and
    /// reliability, for a product (power) that a tier must not multiply.
    /// `#[serde(default)]`: false, the normal tier curve.
    #[serde(default)]
    pub tierless: bool,
}

/// What a configurable machine does with its recipes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MachineRole {
    /// Produces from nothing, like a `work` node.
    Extract,
    /// Consumes the item's own `craftable.cost`, consuming its inputs.
    Assemble,
}

/// A machine a player configures: it lists every recipe it may run and each
/// placed instance picks one (`components::MachineRecipe`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MachineDef {
    pub role: MachineRole,
    pub recipes: Vec<RecipeDef>,
}

/// One recipe a structure runs, and whether it is an extraction.
#[derive(Clone, Copy, Debug)]
pub struct Production<'a> {
    pub recipe: &'a RecipeDef,
    pub extracts: bool,
}

/// A structure as placed: its def and the recipe this instance was set to.
/// The one door to "what does this placed machine make", so callers never
/// branch on `work` versus `machine` themselves.
#[derive(Clone, Copy, Debug)]
pub struct Placed<'a> {
    pub def: &'a StructureDef,
    pub recipe: Option<&'a ItemId>,
}

impl<'a> Placed<'a> {
    /// A def with the `MachineRecipe` component its entity carries, if any.
    pub fn of(def: &'a StructureDef, recipe: Option<&'a crate::components::MachineRecipe>) -> Self {
        Placed {
            def,
            recipe: recipe.and_then(|r| r.0.as_ref()),
        }
    }

    /// A fixed `work` block wins and extracts; otherwise the machine recipe
    /// this instance is set to, if the def still lists it.
    pub fn production(self) -> Option<Production<'a>> {
        if let Some(work) = &self.def.work {
            return Some(Production {
                recipe: work,
                extracts: true,
            });
        }
        let machine = self.def.machine.as_ref()?;
        let recipe = machine_recipe_in(machine, self.recipe?)?;
        Some(Production {
            recipe,
            extracts: machine.role == MachineRole::Extract,
        })
    }

    /// "Factory (Bytecode Block)" for a machine that is set; the def's own
    /// name for an unset machine and for every non-machine.
    pub fn label(self, items: &crate::items_db::ItemDb) -> String {
        match (self.def.machine.as_ref(), self.production()) {
            (Some(_), Some(production)) => {
                let product = items
                    .get(production.recipe.produces.as_str())
                    .map(|d| d.name.as_str())
                    .unwrap_or(production.recipe.produces.as_str());
                format!("{} ({product})", self.def.name)
            }
            _ => self.def.name.clone(),
        }
    }

    /// A configurable machine with nothing to run yet.
    pub fn is_unconfigured(self) -> bool {
        self.def.machine.is_some() && self.production().is_none()
    }
}

fn machine_recipe_in<'a>(machine: &'a MachineDef, item: &ItemId) -> Option<&'a RecipeDef> {
    machine.recipes.iter().find(|r| &r.produces == item)
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
    /// `MachineRole::Assemble`.
    Assembler,
    Utility,
    /// Keeps programs running: refills a need, heals the downed, or adds
    /// roster slots.
    EntityHealth,
    Trade,
    Defence,
}

/// A structure's automated-teardown capability — see
/// `StructureDef::strips` and `Game::run_teardown_rigs`.
///
/// It carries no
/// tick figure of its own: what a program costs to strip is
/// `Game::extraction_ticks`, the same derivation the player's own
/// extraction is priced by, so a rig and a hand cannot disagree.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StripDef {
    /// How many downed programs the hopper holds before it refuses.
    ///
    /// Authored per machine rather than in `tuning.rs`, for
    /// `StructureDef::capacity`'s reason: it is how big *this* box is, and
    /// a modder's second rig should be able to differ.
    pub hopper: u32,
}

/// A structure's prison capability — see `StructureDef::holds_prisoner`.
///
/// One figure, authored per machine like `StripDef::hopper`: how long a
/// warden takes over each decompile attempt is a fact about *this* cell, and
/// a modder's second cell should be able to differ. What an attempt is
/// worth lives in `tuning.rs` and `Game::jail_odds`, not here.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrisonSpec {
    /// Warden beats between one decompile roll and the next.
    pub attempt_ticks: u32,
}

/// A structure's carrier-storage capability — see `StructureDef::racks`.
///
/// Mirrors `StripDef` rung for rung — one figure, authored per machine — and
/// deliberately carries nothing about tools: `Game::extraction_yield` reads a
/// tool for both halves of its answer, and that is a fact about the *rig*
/// (`Hopper::standing_tool`), not about the shelf.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RackDef {
    /// How many downed programs this rack holds **at tier 1**.
    ///
    /// Authored per machine rather than in `tuning.rs`, for
    /// `StripDef::hopper`'s reason: it is how big *this* box is, and a
    /// modder's second rack should be able to differ.
    ///
    /// The ceiling is `slots * tier`, derived per read by `Game::rack_slots`
    /// and never stored beside the component — `BuildSite::required_ticks`'
    /// rule. A stored ceiling goes stale the moment a tier lands.
    pub slots: u32,
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

/// A structure that holds one drop pod — see `StructureDef::drop_pod`.
///
/// The bill alone: how long a recharge takes is `BuildSite::required_ticks`
/// derived from it, the same as any build request, and never authored
/// beside it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DropPodDef {
    /// What the crew hauls to the terminal to charge a spent pod again.
    pub recharge_cost: Vec<(ItemId, u32)>,
}

/// What a structure does for a **downed** program — see
/// `StructureDef::recovery` and `Game::run_repair_bays`.
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
    /// Integrity restored per tick while the structure's `supply` is
    /// being drawn on: it holds a patch, or is working through one already
    /// bought. Absent, a supply buys no speed and the Bay heals at
    /// `per_tick` alone. `#[serde(default)]` so every existing structure
    /// file keeps parsing.
    #[serde(default)]
    pub per_tick_supplied: Option<i32>,
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

    /// `rate`'s floor, applied to the supplied rate: `None` when the def
    /// authors none.
    pub fn supplied_rate(&self) -> Option<i32> {
        self.per_tick_supplied.map(|r| r.max(0))
    }
}

/// Marks a structure as a siege turret — see `StructureDef::turret` and
/// `game::siege::turrets`. No `Stats`, no initiative slot and no body: a
/// turret is a property of the structure, resolved once a round by
/// `Game::fire_turrets` against the nearest hostile in range with line of
/// sight.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TurretDef {
    /// Damage dealt to the target it fires at.
    pub damage: u32,
    /// How far the turret can fire, in the tactical board's own distance —
    /// `reach::line_of_sight`'s unit, never a second one.
    pub range: u32,
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
    pub work: Option<RecipeDef>,
    /// A configurable machine: lists its recipes and each placed instance
    /// picks one. `#[serde(default)]` so existing and modded files parse.
    #[serde(default)]
    pub machine: Option<MachineDef>,
    /// Research that must be done before this kind counts as an extraction
    /// bench (`extracts_routines` / `extracts_programs`). `None` counts from
    /// the moment one stands. `#[serde(default)]`.
    #[serde(default)]
    pub extracts_research: Option<crate::research::ResearchId>,
    /// How many units this structure's output buffer holds before it clogs
    /// (see `components::Stock`). Top-level rather than inside `work`
    /// because an assembler machine declares no `work` block at all,
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
    /// If set, this structure takes downed programs the player hands it and
    /// strips them into its output buffer over ticks, once a program is
    /// assigned to it — see `Game::run_teardown_rigs` and spec §10.
    ///
    /// The instanced object never leaves this machine: what lands in
    /// `Stock::output` is plain items, which is what lets hauling, depots
    /// and `collect` carry the yield with no instance rule (decision 2).
    /// `#[serde(default)]` so every existing structure file, including any
    /// mod, keeps parsing as a machine that strips nothing.
    #[serde(default)]
    pub strips: Option<StripDef>,
    /// If set, this structure is a shelf for downed programs — filled and
    /// emptied from the `c` transfer picker, and fetched from by a body
    /// posted at an adjacent rig. Not a haul target: no *item* ever enters
    /// one, which is why `stores` stays false and this field carries the
    /// capability instead.
    /// `#[serde(default)]` so every existing structure file, including any
    /// mod, keeps parsing as a machine that racks nothing.
    #[serde(default)]
    pub racks: Option<RackDef>,
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
    /// `components::Downed` and `Game::run_repair_bays`. No assigned
    /// worker and no input, `power_regen`'s shape. A downed program walks
    /// here on its own and leaves when it is whole.
    ///
    /// **Distinct from `repair` below**, which is about the base's
    /// structures and not about programs at all. `#[serde(default)]` so
    /// every existing structure file, including any mod, keeps parsing as a
    /// building that recovers nobody — which is the pre-feature game.
    #[serde(default)]
    pub recovery: Option<RecoveryDef>,
    /// If set, this structure is a drop pod: spending its charge drops one
    /// Drop Trooper into a tactical fight, and the crew recharges it by
    /// hauling `recharge_cost` to it as a `BuildGoal::Recharge` request.
    ///
    /// `dispatches_sorties`' rule — a structure is a pod because it declares
    /// this, never because the engine names its id, so a mod's second pod
    /// structure is a file. `#[serde(default)]` so every existing structure
    /// file, including any mod, keeps parsing as a building that is not one.
    #[serde(default)]
    pub drop_pod: Option<DropPodDef>,
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
    /// An item this structure spends by its own action (a defence firing, a
    /// Repair Bay mending) and that haulers deliver to its input buffer. Unlike
    /// `power_upkeep` it burns on no clock; whatever acts draws from the input.
    /// `#[serde(default)]` so every existing structure file, mods included,
    /// keeps parsing as one that spends nothing.
    #[serde(default)]
    pub supply: Option<ItemId>,
    /// How much of `supply` the input buffer holds at most, where the default
    /// of `supply_batch() * INPUT_STOCK_BATCHES` is too small for the use (a
    /// bench kept stocked for many mends). A structure that sets it is
    /// topped up whenever a full hauler's load fits under the ceiling
    /// (`work_orders::supply_top_up_due`), besides the usual less-than-one-batch
    /// trigger; hand-loading from Transfer is open only to one that sets it.
    /// `#[serde(default)]` so every existing structure file keeps parsing.
    #[serde(default)]
    pub supply_capacity: Option<u32>,
    /// Whether this structure issues contracts — see
    /// `Game::contract_board`. A plain `bool` rather than a block of its own,
    /// because a Broker has no per-structure configuration: what it offers is
    /// derived from the world seed, the sector and the clock, not authored on
    /// the building. `#[serde(default)]` so every existing structure file,
    /// including any mod, keeps parsing.
    #[serde(default)]
    pub issues_contracts: bool,
    /// Whether a squad can be dispatched from this structure — see
    /// `Game::dispatch_reach`. `issues_contracts`' shape and for its reason: a
    /// Relay has no per-structure configuration either, since what it offers
    /// is derived from the world seed, the sector and the clock rather than
    /// authored on the building. Also the gate a caravan route dispatches
    /// through — the field name stays as-is even though it now names two
    /// features, per the settlements Phase 6 decision that a route uses the
    /// same structure and the same flag rather than a second one.
    ///
    /// **A flag rather than the engine naming `"relay"`.** Hardcoding the
    /// shipped id would put game content in Rust and make a mod's second
    /// dispatch structure impossible; this way the gate, the building and
    /// the board are all data. `#[serde(default)]` so every existing
    /// structure file, including any mod, keeps parsing.
    #[serde(default)]
    pub dispatches_sorties: bool,
    /// Whether standing beside this structure reaches every Depot in the
    /// base rather than only the ones beside the party — see
    /// `Game::shelves_of`. Machine buffers and racks stay walk-up.
    ///
    /// `dispatches_sorties`' shape and for its reason: a flag rather than
    /// the engine naming the shipped id, so a mod's second terminal is a
    /// file. `#[serde(default)]` so every existing structure file keeps
    /// parsing.
    #[serde(default)]
    pub indexes_storage: bool,
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
    /// If true, this is the Basin Exit: stepping onto it asks to leave the
    /// Phase-Manifold Basin (`Game::escape_basin`). *Whether* it can be built
    /// — all ten Phase Keys, zone 10 or deeper, not yet escaped — is code
    /// (`Game::basin_exit_gate`), not data; the flag only names which
    /// structure the rule applies to, so a mod can reskin it. A missing or
    /// malformed file is replaced by `FALLBACK_BASIN_EXIT`.
    /// `#[serde(default)]`.
    #[serde(default)]
    pub basin_exit: bool,
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
    /// Whether the GC Entropy Sweep may pick this structure. Narrower than
    /// `raidable`, which takes the `Durability` pool away altogether: an
    /// unswept structure still has one, so a besieger on a siege board can
    /// still break it. `false` is what keeps a run of cheap Walls from
    /// soaking up sweeps meant for machines. `#[serde(default =
    /// "default_raidable")]` — true, as every structure was before it.
    #[serde(default = "default_raidable")]
    pub swept: bool,
    /// A barrier refuses the player's step in base space, as solid rock
    /// does, and blocks line of sight on a siege board while it stands —
    /// `Game::move_in_base` and `siege::board::seat_structures` are its
    /// two readers. Its cells already refuse every other body's walk,
    /// because every structure's anchor does. `#[serde(default)]`.
    #[serde(default)]
    pub barrier: bool,
    /// A door: its anchor bounds a room like a wall does (`rooms::detect`),
    /// yet the crew and the player walk through it — `Game::blocked_tiles`
    /// skips a door's anchor. `#[serde(default)]`.
    #[serde(default)]
    pub door: bool,
    /// What this structure counts as when `rooms::detect` matches a room
    /// against `assets/rooms/*.ron` `requires`. `#[serde(default)]`.
    #[serde(default)]
    pub room_tags: Vec<String>,
    /// How much this structure reduces raid damage by, for *every* raid
    /// against *any* deployed structure — not just itself — while it's
    /// standing (see `Game::raid_check`). Stacks additively across every
    /// deployed structure with this set, on top of whatever an assigned
    /// worker/guard already mitigates. `#[serde(default)]` so existing
    /// structure files (including mods) without this field contribute
    /// nothing, same as before it existed.
    #[serde(default)]
    pub raid_defense: u32,
    /// If set, this structure is a siege turret — see
    /// `game::siege::turrets::turret_defense` and `Game::fire_turrets`. Only
    /// meaningful during a siege; outside one it is an ordinary structure
    /// and keeps contributing `raid_defense` to the sweep as normal, which
    /// is the only place the two events touch. `#[serde(default)]` so
    /// existing structure files (including mods) stay ordinary structures,
    /// exactly as before this field existed. Whether a turret needs power,
    /// staffing or ammunition is a content question for the def that
    /// declares it, not a rule in Rust.
    #[serde(default)]
    pub turret: Option<TurretDef>,
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
    /// If true, owning one of these anywhere makes an *extraction* better —
    /// `Game::extract_program`, not `Game::extract_routine`. Never a gate:
    /// spec decision 7 is that extraction works in the field, at the base
    /// and in the Stack alike, because the starting tool is useless
    /// otherwise. Ownership rather than proximity, `extracts_routines`'
    /// own rule. `#[serde(default)]` so existing structure files (including
    /// mods) improve nothing, exactly as before this field existed.
    #[serde(default)]
    pub extracts_programs: bool,
    /// The side of a square claim on placement, anchored **top-left** — the
    /// same meaning and anchoring `tactical::footprint_cells_at` already
    /// spells, so there is one definition of "a square footprint" in the
    /// repo rather than two that can drift. The anchor cell (the one
    /// `place_structure` measures `(dx, dy)` from) is the structure's
    /// impassable block, carrying its glyph; the other
    /// `footprint * footprint - 1` cells are its own walkable floor. Checked
    /// only at placement (`Game::place_structure`'s widened refusal ladder)
    /// and never stored — a structure already standing is never refused
    /// retroactively, which is what lets a structure widen its footprint in
    /// a later release with no save migration: the legacy anchor keeps
    /// blocking exactly what it always blocked.
    /// `#[serde(default = "default_footprint")]` — a bare `#[serde(default)]`
    /// gives `0`, a footprint that claims no cells. That default only covers
    /// an *absent* field: an authored `footprint: 0` parses clean, so
    /// `StructureDb::load_dir` rejects it after the fact, the same way it
    /// handles any other malformed file.
    #[serde(default = "default_footprint")]
    pub footprint: u8,
    /// Whether this structure can hold a tamed program *under study*, in the
    /// footprint cell diagonally opposite the anchor — see
    /// `Game::study_pen`. `#[serde(default)]` so every existing structure
    /// file, including any mod, keeps parsing as a machine with no pen.
    #[serde(default)]
    pub studies: bool,
    /// Whether this structure holds a tamed program pinned into the grid, its
    /// `power_supply` counted only while one is held — see
    /// `game::base::power::ledger`. Data decides which structure does this,
    /// not a kind name in Rust. `#[serde(default)]` so every existing
    /// structure file, including any mod, keeps parsing as an ordinary one.
    #[serde(default)]
    pub siphons: bool,
    /// If set, this structure holds one downed program revived as a live
    /// prisoner in the footprint cell diagonally opposite the anchor — see
    /// `Game::jail_program` and `components::Jailed`. The pen is the same
    /// corner a `studies` structure uses (`game::base::study::pen_corner`).
    /// `#[serde(default)]` so every existing structure file, including any
    /// mod, keeps parsing as a structure that holds nobody.
    #[serde(default)]
    pub holds_prisoner: Option<PrisonSpec>,
    /// How many children this structure incubates at once — see
    /// `Game::breed`. Above zero, a built one unlocks the Breed action. `0`,
    /// the default, is every structure but the Breeding Bay, so an existing
    /// file and any mod that never heard of the field is unaffected.
    #[serde(default)]
    pub incubation_slots: u32,
}

fn default_durability() -> u32 {
    crate::tuning::DEFAULT_STRUCTURE_DURABILITY
}

fn default_raidable() -> bool {
    true
}

fn default_footprint() -> u8 {
    1
}

/// The id the built-in Basin Exit is filed under.
pub const BASIN_EXIT_ID: &str = "basin_exit";

/// Used when no loaded file declares `basin_exit: true`, so the game stays
/// completable whatever a mod does to `assets/structures/`. A literal parsed
/// through the same `StructureDef` the files use, so serde defaults fill the
/// rest.
const FALLBACK_BASIN_EXIT: &str = "(id: \"basin_exit\", name: \"Basin Exit\", \
    description: \"A way out of the Phase-Manifold Basin.\", glyph: '>', color: Cyan, \
    build_cost: [], work: None, basin_exit: true, max_deployed: 1)";

#[derive(Resource, Default)]
pub struct StructureDb {
    structures: HashMap<StructureId, StructureDef>,
    /// `structure_levels` over `structures`, filled by `compute_levels`
    /// once the item catalogue (an assembler's inputs are its item's recipe)
    /// has loaded. Derived, never saved.
    levels: HashMap<StructureId, u32>,
}

/// Every item `def` consumes: its build cost and, for an assembler, its
/// item's recipe. `recipe` answers the second, so this stays free of the
/// item catalogue and a test can hand-build both halves. `recipe` is asked
/// per produced item, so a machine that lists several contributes every one.
fn consumed_items<'a>(
    def: &'a StructureDef,
    recipe: &impl Fn(&ItemId) -> Option<&'a [(ItemId, u32)]>,
) -> Vec<&'a ItemId> {
    let mut items: Vec<&ItemId> = def.build_cost.iter().map(|(i, _)| i).collect();
    let machine = def
        .productions()
        .filter(|p| !p.extracts)
        .map(|p| &p.recipe.produces);
    for item in machine {
        if let Some(inputs) = recipe(item) {
            items.extend(inputs.iter().map(|(i, _)| i));
        }
    }
    items
}

/// A structure's production level: how many machines deep its inputs run.
///
/// A structure with no producer among its inputs is level 0 (a `work` node
/// paid for in what it mines); any other is 1 + the highest level of the
/// producers of what it costs and consumes. An item nothing produces
/// contributes nothing, a structure's own output in its own cost is ignored,
/// and a producer cycle shares one level (strongly connected components, so
/// the fixed point cannot climb forever).
pub fn structure_levels<'a>(
    defs: &'a HashMap<StructureId, StructureDef>,
    recipe: impl Fn(&ItemId) -> Option<&'a [(ItemId, u32)]>,
) -> HashMap<StructureId, u32> {
    let mut list: Vec<&StructureDef> = defs.values().collect();
    list.sort_by(|a, b| a.id.cmp(&b.id));
    let n = list.len();
    let mut producers: HashMap<&ItemId, Vec<usize>> = HashMap::new();
    for (i, def) in list.iter().enumerate() {
        // Def-level on purpose: a build site has no recipe picked yet.
        for production in def.productions() {
            producers
                .entry(&production.recipe.produces)
                .or_default()
                .push(i);
        }
    }
    let deps: Vec<Vec<usize>> = list
        .iter()
        .enumerate()
        .map(|(i, def)| {
            let mut d: Vec<usize> = consumed_items(def, &recipe)
                .into_iter()
                .filter_map(|item| producers.get(item))
                .flatten()
                .copied()
                .filter(|&p| p != i)
                .collect();
            d.sort_unstable();
            d.dedup();
            d
        })
        .collect();
    let mut reach = vec![vec![false; n]; n];
    for (i, ds) in deps.iter().enumerate() {
        for &d in ds {
            reach[i][d] = true;
        }
    }
    for k in 0..n {
        for i in 0..n {
            if reach[i][k] {
                let via = reach[k].clone();
                for (cell, through) in reach[i].iter_mut().zip(via) {
                    *cell |= through;
                }
            }
        }
    }
    let same = |a: usize, b: usize| a == b || (reach[a][b] && reach[b][a]);
    let mut level = vec![0u32; n];
    for _ in 0..=n {
        let mut changed = false;
        for i in 0..n {
            let next = (0..n)
                .filter(|&m| same(i, m))
                .flat_map(|m| deps[m].iter().copied())
                .filter(|&d| !same(i, d))
                .map(|d| level[d] + 1)
                .max()
                .unwrap_or(0);
            if next != level[i] {
                level[i] = next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    list.iter()
        .zip(level)
        .map(|(def, l)| (def.id.clone(), l))
        .collect()
}

impl StructureDef {
    /// Whether this structure runs a job a program can be posted to — an
    /// extractor (`work`) or a configurable machine.
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
        self.work.is_some()
            || self.machine.is_some()
            || self.strips.is_some()
            || self.holds_prisoner.is_some()
    }

    /// Every recipe this kind could run: the fixed `work` block and each
    /// machine recipe. Def-level; `Placed::production` is the instance's.
    pub fn productions(&self) -> impl Iterator<Item = Production<'_>> {
        let fixed = self.work.iter().map(|recipe| Production {
            recipe,
            extracts: true,
        });
        let extracts = self
            .machine
            .as_ref()
            .is_some_and(|m| m.role == MachineRole::Extract);
        let listed = self
            .machine
            .iter()
            .flat_map(|m| m.recipes.iter())
            .map(move |recipe| Production { recipe, extracts });
        fixed.chain(listed)
    }

    /// The listed machine recipe producing `item`, if any.
    pub fn machine_recipe(&self, item: &ItemId) -> Option<&RecipeDef> {
        machine_recipe_in(self.machine.as_ref()?, item)
    }

    /// Whether this kind extracts from nothing: a `work` block or an
    /// `Extract` machine.
    pub fn extracts(&self) -> bool {
        self.work.is_some()
            || self
                .machine
                .as_ref()
                .is_some_and(|m| m.role == MachineRole::Extract)
    }

    /// Whether this structure carries a `MachineStatus`. The one definition,
    /// read by both places a structure's components are written
    /// (`Game::spawn_structure` and the save loader) so a hand-written copy
    /// cannot drift: a job runner reports a stall, and a supply spender
    /// reports that it ran dry.
    pub fn has_machine_status(&self) -> bool {
        self.runs_a_job() || self.spends_supply().is_some()
    }

    /// The item this structure spends to keep working and that haulers
    /// deliver to its input buffer: a supplier's `power_upkeep` fuel or a
    /// defence's `supply`. The one question every hauling special-case asks,
    /// so the two kinds share a single hauling path.
    pub fn spends_supply(&self) -> Option<&ItemId> {
        self.power_upkeep.as_ref().or(self.supply.as_ref())
    }

    /// Whether stock in an adjacent structure's output counts as within
    /// reach of this one's intake. True for a burner, which
    /// `burn_grid_upkeep` pulls from its neighbours, and for an assembler;
    /// false for a structure that only spends a `supply`, which reads its
    /// own input alone (`defence_is_armed`, `spend_supply`).
    pub fn draws_from_neighbours(&self) -> bool {
        self.power_upkeep.is_some() || self.supply.is_none()
    }

    /// Whether this structure takes part in base defence: it softens a sweep
    /// or siege (`raid_defense`) or fires in one (`turret`).
    pub fn is_defence(&self) -> bool {
        self.raid_defense > 0 || self.turret.is_some()
    }

    /// How many units of `spends_supply` make one delivery: the fuel window
    /// for a burner, `tuning::SUPPLY_BATCH` for a defence. `intake_recipe`
    /// reports it as the batch size and `Game::fuel_wants` asks whether one
    /// batch is within reach, so the two cannot disagree.
    pub fn supply_batch(&self) -> u32 {
        if self.power_upkeep.is_some() {
            crate::tuning::POWER_UPKEEP_CELLS_PER_WINDOW
        } else {
            crate::tuning::SUPPLY_BATCH
        }
    }

    /// How much of its `spends_supply` item this structure's input holds at
    /// most: the authored `supply_capacity`, else `INPUT_STOCK_BATCHES`
    /// batches. The one answer the hauler's `input_room` and the roster's
    /// `StructureReport::supply` both read.
    pub fn supply_input_capacity(&self) -> u32 {
        self.supply_capacity
            .unwrap_or_else(|| crate::game::base::hauling::input_capacity(self.supply_batch()))
    }

    /// Whether raising or upgrading this structure costs a tamed program.
    ///
    /// **A body is spent on a machine that will take a body** — the cost
    /// follows `runs_a_job` exactly, so an extractor, an assembler and a rig
    /// charge one and nothing else does. A Shield, a Patch Node, a Relay, a
    /// shelf and the Zone Portal are all free, and they are free for one
    /// reason rather than each for its own: there is nothing at them for a
    /// committed program to stand at. That replaced an authored
    /// `costs_no_program` flag, which made the exemption content — a seventh
    /// depot file that forgot the line silently cost a program, and a modded
    /// ornament had to remember to say it.
    ///
    /// The Home is exempt a second time, by `category()`, because an edited
    /// `home.ron` that declared `work:` would otherwise leave a fresh run —
    /// which owns zero programs — unable to found a base at all.
    ///
    /// On the def rather than only on `Game`, because the build menu holds
    /// defs and not ids and would otherwise restate the rule — the copy that
    /// drifts. `Game::structure_needs_program` is the id-shaped door onto
    /// this one, and calls it.
    pub fn needs_program(&self) -> bool {
        self.category() != StructureCategory::Home && self.runs_a_job()
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
        if let Some(machine) = &self.machine {
            return match machine.role {
                MachineRole::Extract => StructureCategory::Extractor,
                MachineRole::Assemble => StructureCategory::Assembler,
            };
        }
        // A rig consumes something and pays items out, given a program —
        // which is what `Assembler` means here. Deliberately not an eighth
        // `StructureCategory`: the variant is a build-menu grouping, and a
        // new one is a group every menu has to learn to draw.
        if self.strips.is_some() {
            return StructureCategory::Assembler;
        }
        if self.trade.is_some() {
            return StructureCategory::Trade;
        }
        // Ahead of Defence, which `recovery` used to file the Repair Bay under.
        if !self.services.is_empty() || self.recovery.is_some() || self.pet_slot_bonus > 0 {
            return StructureCategory::EntityHealth;
        }
        if self.raid_defense > 0 || self.repair.is_some() || self.turret.is_some() {
            return StructureCategory::Defence;
        }
        StructureCategory::Utility
    }

    /// The name a sprite loader looks this structure up by: the `sprite`
    /// override when authored, the id otherwise. The one place this
    /// fallback is written — see `sprite`'s doc comment.
    ///
    /// An override starting with `@` is treated as absent and falls back to
    /// the id instead. `@` keys the runtime-only drawn-icon slot
    /// (`sprites::DRAWN_ICON_KEY` in the renderer) that the player's own
    /// pixel editor registers under — no file ever lands there — so a
    /// structure allowed to name it would draw the player's own hand-drawn
    /// icon on its tile.
    pub fn sprite_name(&self) -> &str {
        self.sprite
            .as_deref()
            .filter(|s| !s.starts_with('@'))
            .unwrap_or(&self.id)
    }
}

impl StructureDb {
    /// Whether `kind` is a door (`StructureDef::door`): bounds a room but is
    /// walked through. The one place the walk-blocking sets ask it.
    pub fn is_door(&self, kind: &str) -> bool {
        self.get(kind).is_some_and(|d| d.door)
    }

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
                    // `#[serde(default = "default_footprint")]` only saves an
                    // *absent* field — an authored `footprint: 0` parses
                    // clean and `footprint_cells_at` then returns no cells at
                    // all, so every widened `place_structure` refusal passes
                    // vacuously and nothing can ever post to the structure.
                    if def.footprint == 0 {
                        warnings.push(format!(
                            "skipped invalid structure file {path:?}: footprint is 0, which \
                             claims no cells at all"
                        ));
                        continue;
                    }
                    // The pen is the footprint's far corner, which on a
                    // footprint of 1 is the blocking anchor itself, and a
                    // cell that never attempts anything holds its prisoner
                    // forever.
                    if let Some(prison) = &def.holds_prisoner
                        && (def.footprint < 2 || prison.attempt_ticks == 0)
                    {
                        warnings.push(format!(
                            "skipped invalid structure file {path:?}: holds_prisoner needs a \
                             footprint of 2 or more and a non-zero attempt_ticks"
                        ));
                        continue;
                    }
                    db.structures.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid structure file {path:?}: {e}")),
            }
        }
        // Silent when the file is merely absent (a malformed one has already
        // warned above), so a fixture directory of one structure stays quiet.
        if !db.structures.values().any(|def| def.basin_exit) {
            let fallback: StructureDef =
                ron::from_str(FALLBACK_BASIN_EXIT).expect("the built-in Basin Exit def parses");
            db.structures.insert(fallback.id.clone(), fallback);
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &str) -> Option<&StructureDef> {
        self.structures.get(id)
    }

    /// Fills `level` from the loaded defs. Called once the item catalogue
    /// is final, because an assembler's inputs live on its item.
    pub fn compute_levels(&mut self, items: &crate::items_db::ItemDb) {
        let levels = structure_levels(&self.structures, |item| {
            Some(
                items
                    .get(item.0.as_str())?
                    .craftable
                    .as_ref()?
                    .cost
                    .as_slice(),
            )
        });
        self.levels = levels;
    }

    /// Drops each machine recipe that cannot run, one warning apiece: an
    /// unknown item, an `Assemble` item with no `craftable`, a zero cycle, or
    /// a repeat of an item the machine already lists. Run once the item
    /// catalogue is final.
    pub fn validate_machines(&mut self, items: &crate::items_db::ItemDb) -> Vec<String> {
        let mut warnings = Vec::new();
        for def in self.structures.values_mut() {
            let id = def.id.clone();
            let Some(machine) = def.machine.as_mut() else {
                continue;
            };
            let role = machine.role;
            let mut seen: Vec<ItemId> = Vec::new();
            machine.recipes.retain(|r| {
                let problem = if r.ticks_per_unit == 0 {
                    Some("has a zero-length cycle")
                } else if seen.contains(&r.produces) {
                    Some("is listed twice")
                } else {
                    match items.get(r.produces.0.as_str()) {
                        None => Some("names an unknown item"),
                        Some(item) if role == MachineRole::Assemble && item.craftable.is_none() => {
                            Some("is not craftable")
                        }
                        Some(_) => None,
                    }
                };
                if let Some(problem) = problem {
                    warnings.push(format!(
                        "structure '{id}': machine recipe '{}' {problem}; skipped",
                        r.produces.0
                    ));
                    return false;
                }
                seen.push(r.produces.clone());
                true
            });
        }
        warnings
    }

    /// A warning for every recipe, and every item recipe, gated behind a
    /// research node that does not exist: such a recipe would stay
    /// unreachable with nothing said. The recipe is kept, as an unknown tool
    /// id is.
    pub fn unknown_research_references(
        &self,
        items: &crate::items_db::ItemDb,
        research: &crate::research::ResearchDb,
    ) -> Vec<String> {
        let mut warnings = Vec::new();
        for def in self.structures.values() {
            for p in def.productions() {
                if let Some(id) = p
                    .recipe
                    .research
                    .as_ref()
                    .filter(|id| research.get(id).is_none())
                {
                    warnings.push(format!(
                        "structure '{}': recipe '{}' is gated behind unknown research '{id}'",
                        def.id, p.recipe.produces.0
                    ));
                }
            }
        }
        for item in items.all() {
            if let Some(id) = item
                .craftable
                .as_ref()
                .and_then(|c| c.research.as_ref())
                .filter(|id| research.get(id).is_none())
            {
                warnings.push(format!(
                    "item '{}': craftable behind unknown research '{id}'",
                    item.id
                ));
            }
        }
        warnings
    }

    /// Production level (see `structure_levels`); 0 for an unknown id.
    pub fn level(&self, id: &str) -> u32 {
        self.levels.get(id).copied().unwrap_or(0)
    }

    /// Adds `def`, replacing any structure already under its id.
    ///
    /// Test-only, and deliberately so — `SpeciesDb::insert`'s reason,
    /// verbatim: the shipped catalogue is a directory of `.ron` files and a
    /// mod's is too, so nothing in the game builds a structure in Rust. It
    /// exists so a test can stand a *second* structure carrying a flag only
    /// one shipped file carries, which is the only way to tell "the one
    /// that answered" apart from "the first one in the catalogue".
    #[cfg(test)]
    pub(crate) fn insert(&mut self, def: StructureDef) {
        self.structures.insert(def.id.clone(), def);
    }

    /// Every loaded structure, grouped by `StructureCategory` and
    /// alphabetical by `name` inside each group.
    ///
    /// `HashMap` iteration order is randomized per-instance (a fresh seed
    /// each time a `StructureDb` is built, i.e. every new/loaded game), so
    /// without a sort here the build menu's `[1]`, `[2]`, ... numbering would
    /// shuffle unpredictably from one session to the next even though nothing
    /// about the mod files changed — the same digit could mean a
    /// 12-Core-Fragment Extractor in one session and a 16-Core-Fragment
    /// Printer in the next.
    ///
    /// This used to pin four structures first by id and sort the rest alphabetically. That put a modded
    /// structure wherever its id fell in the alphabet, which stopped being
    /// tolerable once the production chain landed: the patch-routine bench sorted
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

/// The cycle this structure ships, or `None` for one that runs no cycle at
/// all. Named once so the picker's preview and the real rate cannot disagree
/// about which structures even have a rate.
pub(crate) fn cycle_ticks(placed: Placed<'_>) -> Option<u32> {
    placed
        .production()
        .map(|production| production.recipe.ticks_per_unit)
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

    #[test]
    fn what_keeps_programs_running_files_under_entity_health() {
        let db = test_db();
        for id in ["defrag_bay", "sandbox", "data_cache", "repair_bay"] {
            let def = db.get(id).expect("shipped structure should load");
            assert_eq!(def.category(), StructureCategory::EntityHealth, "{id}");
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
        let node = db.get("extractor").expect("extractor.ron should load");
        assert!(node.services.is_empty());
        // Same rule, same file, one field along: what `#[serde(default)]` is
        // for needs an assertion rather than an assumption.
        assert!(node.recovery.is_none());
    }

    /// A structure is a drop pod because it declares the field, never
    /// because of its id — and every file that doesn't declare it is not one.
    #[test]
    fn a_drop_pod_is_declared_by_its_field() {
        let db = test_db();
        assert!(db.get("extractor").unwrap().drop_pod.is_none());
        let pod = db
            .get("drop_pod_terminal")
            .and_then(|def| def.drop_pod.as_ref())
            .expect("drop_pod_terminal.ron should declare a drop_pod");
        assert!(!pod.recharge_cost.is_empty());
    }

    /// `per_tick` is mod-supplied and a field named for repair must never
    /// damage. The `i32` is what makes this half a clamp instead of a whole
    /// one — there is no non-finite case to guard.
    #[test]
    fn a_negative_recovery_rate_floors_at_zero() {
        assert_eq!(
            RecoveryDef {
                per_tick: -4,
                per_tick_supplied: None,
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

    fn levels_of(defs: &[&str], recipes: &[(&str, &[(&str, u32)])]) -> HashMap<StructureId, u32> {
        let map: HashMap<StructureId, StructureDef> = defs
            .iter()
            .map(|d| {
                let def: StructureDef = ron::from_str(d).expect("fixture def parses");
                (def.id.clone(), def)
            })
            .collect();
        let recipes: HashMap<ItemId, Vec<(ItemId, u32)>> = recipes
            .iter()
            .map(|(item, cost)| {
                (
                    ItemId(item.to_string()),
                    cost.iter()
                        .map(|(i, q)| (ItemId(i.to_string()), *q))
                        .collect(),
                )
            })
            .collect();
        structure_levels(&map, |item| recipes.get(item).map(|v| v.as_slice()))
    }

    fn fixture(id: &str, cost: &str, rest: &str) -> String {
        format!(
            "(id: \"{id}\", name: \"{id}\", description: \"\", glyph: 'x', color: Cyan, \
             build_cost: [{cost}], {rest})"
        )
    }

    #[test]
    fn a_raw_producer_is_level_zero_and_its_own_output_in_its_cost_is_ignored() {
        let mine = fixture(
            "mine",
            "(\"ore\", 5)",
            "work: Some((produces: \"ore\", ticks_per_unit: 10))",
        );
        let levels = levels_of(&[&mine], &[]);
        assert_eq!(levels["mine"], 0);
    }

    #[test]
    fn a_chain_climbs_one_level_per_step() {
        let mine = fixture(
            "mine",
            "(\"ore\", 5)",
            "work: Some((produces: \"ore\", ticks_per_unit: 10))",
        );
        let bench = fixture(
            "bench",
            "(\"ore\", 5)",
            "machine: Some((role: Assemble, recipes: [(produces: \"plate\", ticks_per_unit: 10)]))",
        );
        let press = fixture(
            "press",
            "",
            "machine: Some((role: Assemble, recipes: [(produces: \"gear\", ticks_per_unit: 10)]))",
        );
        let levels = levels_of(
            &[&mine, &bench, &press],
            &[("plate", &[("ore", 1)]), ("gear", &[("plate", 1)])],
        );
        assert_eq!(levels["mine"], 0);
        assert_eq!(levels["bench"], 1);
        assert_eq!(levels["press"], 2);
    }

    #[test]
    fn a_producer_cycle_shares_one_level() {
        let mine = fixture(
            "mine",
            "",
            "work: Some((produces: \"ore\", ticks_per_unit: 10))",
        );
        let a = fixture(
            "a",
            "(\"b_out\", 1)",
            "machine: Some((role: Assemble, recipes: [(produces: \"a_out\", ticks_per_unit: 10)]))",
        );
        let b = fixture(
            "b",
            "(\"a_out\", 1)",
            "machine: Some((role: Assemble, recipes: [(produces: \"b_out\", ticks_per_unit: 10)]))",
        );
        let levels = levels_of(
            &[&mine, &a, &b],
            &[("a_out", &[("ore", 1)]), ("b_out", &[("a_out", 1)])],
        );
        assert_eq!(levels["a"], levels["b"]);
        assert_eq!(levels["a"], 1);
    }

    #[test]
    fn an_item_nothing_produces_contributes_nothing() {
        let wall = fixture("wall", "(\"scrap\", 3)", "");
        let mine = fixture(
            "mine",
            "",
            "work: Some((produces: \"ore\", ticks_per_unit: 10))",
        );
        let post = fixture("post", "(\"ore\", 3)", "");
        let levels = levels_of(&[&wall, &mine, &post], &[]);
        assert_eq!(levels["wall"], 0);
        assert_eq!(levels["post"], 1);
    }

    fn def_with(rest: &str) -> StructureDef {
        ron::from_str(&fixture("m", "", rest)).expect("fixture def parses")
    }

    fn id(s: &str) -> ItemId {
        ItemId(s.to_string())
    }

    const TWO_RECIPE_ASSEMBLER: &str = "machine: Some((role: Assemble, recipes: [\
        (produces: \"a\", ticks_per_unit: 5), (produces: \"b\", ticks_per_unit: 9)]))";

    #[test]
    fn a_machine_def_parses_with_role_and_recipes() {
        let def = def_with(
            "machine: Some((role: Extract, recipes: [(produces: \"ore\", ticks_per_unit: 10, level: Some(1))]))",
        );
        let machine = def.machine.expect("machine parses");
        assert_eq!(machine.role, MachineRole::Extract);
        let recipe = &machine.recipes[0];
        assert_eq!(recipe.produces, id("ore"));
        assert_eq!(recipe.level, Some(1));
        assert_eq!(recipe.research, None);
        assert!(!recipe.tierless);
    }

    #[test]
    fn placed_production_reads_the_instance_recipe() {
        let def = def_with(TWO_RECIPE_ASSEMBLER);
        let b = id("b");
        let production = Placed {
            def: &def,
            recipe: Some(&b),
        }
        .production()
        .unwrap();
        assert_eq!(production.recipe.ticks_per_unit, 9);
        assert!(!production.extracts);

        let none = Placed {
            def: &def,
            recipe: None,
        };
        assert!(none.production().is_none());
        assert!(none.is_unconfigured());

        let c = id("c");
        let unlisted = Placed {
            def: &def,
            recipe: Some(&c),
        };
        assert!(unlisted.production().is_none());
    }

    #[test]
    fn a_fixed_work_block_wins_over_any_recipe() {
        let def = def_with("work: Some((produces: \"ore\", ticks_per_unit: 7))");
        let other = id("whatever");
        for recipe in [None, Some(&other)] {
            let production = Placed { def: &def, recipe }.production().unwrap();
            assert!(production.extracts);
            assert_eq!(production.recipe.ticks_per_unit, 7);
        }
        assert!(
            !Placed {
                def: &def,
                recipe: None
            }
            .is_unconfigured()
        );
    }

    #[test]
    fn a_machine_runs_a_job_and_groups_by_role() {
        let extract = def_with(
            "machine: Some((role: Extract, recipes: [(produces: \"ore\", ticks_per_unit: 3)]))",
        );
        let assemble = def_with(TWO_RECIPE_ASSEMBLER);
        for def in [&extract, &assemble] {
            assert!(def.runs_a_job());
            assert!(def.needs_program());
        }
        assert_eq!(extract.category(), StructureCategory::Extractor);
        assert_eq!(assemble.category(), StructureCategory::Assembler);
        assert!(extract.extracts());
        assert!(!assemble.extracts());
    }

    #[test]
    fn validation_skips_a_bad_recipe_and_keeps_the_rest() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (items, _) = crate::items_db::ItemDb::load_dir(
            &dir.join("items"),
            &crate::abilities::AbilityDb::load_dir(&dir.join("abilities"))
                .unwrap()
                .0,
        )
        .unwrap();
        let craftable = items
            .all()
            .find(|i| i.craftable.is_some())
            .expect("a craftable item ships")
            .id
            .0
            .clone();
        let raw = items
            .all()
            .find(|i| i.craftable.is_none())
            .expect("a non-craftable item ships")
            .id
            .0
            .clone();
        let mut db = StructureDb::default();
        let assemble = fixture(
            "asm",
            "",
            &format!(
                "machine: Some((role: Assemble, recipes: [\
                 (produces: \"{craftable}\", ticks_per_unit: 5), \
                 (produces: \"no_such_item\", ticks_per_unit: 5), \
                 (produces: \"{raw}\", ticks_per_unit: 5)]))"
            ),
        );
        let zero = fixture(
            "zero",
            "",
            &format!(
                "machine: Some((role: Extract, recipes: [\
                 (produces: \"{raw}\", ticks_per_unit: 4), \
                 (produces: \"{craftable}\", ticks_per_unit: 0)]))"
            ),
        );
        for d in [assemble, zero] {
            db.insert(ron::from_str(&d).unwrap());
        }
        let warnings = db.validate_machines(&items);
        assert_eq!(warnings.len(), 3, "{warnings:?}");
        let kept = |sid: &str| -> Vec<String> {
            db.get(sid)
                .unwrap()
                .machine
                .as_ref()
                .unwrap()
                .recipes
                .iter()
                .map(|r| r.produces.0.clone())
                .collect()
        };
        assert_eq!(kept("asm"), vec![craftable]);
        assert_eq!(kept("zero"), vec![raw]);
    }

    #[test]
    fn structure_levels_count_every_recipe_a_machine_lists() {
        let extractor = fixture(
            "extractor",
            "",
            "machine: Some((role: Extract, recipes: [(produces: \"ore\", ticks_per_unit: 10)]))",
        );
        let factory = fixture(
            "factory",
            "",
            "machine: Some((role: Assemble, recipes: [(produces: \"plate\", ticks_per_unit: 10)]))",
        );
        let levels = levels_of(&[&extractor, &factory], &[("plate", &[("ore", 1)])]);
        assert_eq!(levels["extractor"], 0);
        assert_eq!(levels["factory"], 1);
    }

    #[test]
    fn an_extractor_sits_below_a_printer_in_the_shipped_assets() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (mut db, _) = StructureDb::load_dir(&dir.join("structures")).unwrap();
        let (items, _) = crate::items_db::ItemDb::load_dir(
            &dir.join("items"),
            &crate::abilities::AbilityDb::load_dir(&dir.join("abilities"))
                .unwrap()
                .0,
        )
        .unwrap();
        db.compute_levels(&items);
        assert!(db.level("extractor") < db.level("printer"));
    }
}
