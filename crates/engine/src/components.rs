use bevy_ecs::prelude::{Component, Entity};
use serde::{Deserialize, Serialize};

use crate::MAX_CUSTOM_NAME_LEN;
use crate::abilities::AbilityId;
use crate::items::{EquipmentSlot, ItemId};
use crate::items_db::ItemDb;
use crate::perks::Perk;
use crate::species::SpeciesId;
use crate::structures::StructureId;
use crate::tuning::{GOLD_STAT_MULT, MAX_INDIVIDUAL_ROLL, MIN_INDIVIDUAL_ROLL, SILVER_STAT_MULT};

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GlyphColor {
    White,
    Gray,
    Green,
    DarkGreen,
    Red,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Brown,
    Orange,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Glyph {
    pub ch: char,
    pub color: GlyphColor,
}

/// Marks the single player-controlled entity.
#[derive(Component)]
pub struct Player;

#[derive(Component, Clone)]
pub struct Creature {
    pub species: SpeciesId,
}

/// A player-chosen display name that overrides a creature's species name
/// wherever it's shown — set by `Game::fuse_companions` and
/// `Game::rename_companion`. The constructor enforces the length; the
/// tuple field does not, so build one through `sanitize` rather than
/// wrapping a raw string.
#[derive(Component, Clone, Debug)]
pub struct CustomName(pub String);

impl CustomName {
    /// What the player typed, reduced to what may actually be stored:
    /// trimmed, truncated to `MAX_CUSTOM_NAME_LEN`, and `None` if nothing
    /// is left.
    ///
    /// **`None` means "no override", and both callers want that** — it is
    /// why this returns an `Option` rather than refusing empty input.
    /// Fusion reads it as "insert no `CustomName`" and a rename reads it as
    /// "remove the one that's there", and those land on the same place: the
    /// species name. So blank is the way back to the default, in both.
    pub fn sanitize(input: Option<String>) -> Option<String> {
        let trimmed = input?
            .trim()
            .chars()
            .take(MAX_CUSTOM_NAME_LEN)
            .collect::<String>();
        (!trimmed.is_empty()).then_some(trimmed)
    }
}

/// Which zone portal's sector a creature was spawned in — set once at
/// spawn time and never changed afterward, even if the creature is later
/// tamed and carried through a portal into a deeper zone. Drives its stat
/// scale (see `ZoneLevel::stat_multiplier`) and is appended to its display
/// label (e.g. "Scrapper 2") so a deeper-zone catch reads differently from
/// a shallow one.
#[derive(Component, Clone, Copy, Debug)]
pub struct ZonePortal(pub u32);

#[derive(Component, Clone, Copy, Debug)]
pub struct Stats {
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
}

impl Stats {
    pub fn hp_fraction(&self) -> f32 {
        if self.max_hp <= 0 {
            0.0
        } else {
            (self.hp as f32 / self.max_hp as f32).clamp(0.0, 1.0)
        }
    }

    /// A rough "how strong is this" scalar — max HP plus Attack plus
    /// Defense, unweighted — used to gauge relative difficulty (see
    /// `difficulty_color`) without singling out any one stat.
    pub fn power(&self) -> i32 {
        self.max_hp + self.atk + self.def
    }
}

/// The satisfied end of a need's range. Lives here beside `Needs` rather
/// than in `balance_sim`, because it is the type's own documented invariant
/// rather than a tuning knob: anything writing `hunger` or `fatigue` has to
/// clamp to it.
pub const NEED_MAX: f32 = 100.0;
/// The critical end. Below it a need is meaningless, and `Needs` readers
/// (the status bars, `battle::power_attack_multiplier`) assume it holds.
pub const NEED_MIN: f32 = 0.0;

/// Hunger/fatigue both run `NEED_MIN..=NEED_MAX`; full is satisfied, 0 is
/// critical.
#[derive(Component, Clone, Copy, Debug)]
pub struct Needs {
    pub hunger: f32,
    pub fatigue: f32,
}

impl Default for Needs {
    fn default() -> Self {
        Self {
            hunger: NEED_MAX,
            fatigue: NEED_MAX,
        }
    }
}

/// A wild creature that will fight rather than flee when engaged.
#[derive(Component)]
pub struct Hostile;

/// Tracks level/XP for the player and any tamed creature. Wild (untamed)
/// creatures don't carry this — they don't level until compiled.
#[derive(Component, Clone, Copy, Debug)]
pub struct Experience {
    pub level: u32,
    pub xp: u32,
    pub xp_to_next: u32,
}

impl Default for Experience {
    fn default() -> Self {
        Self {
            level: 1,
            xp: 0,
            xp_to_next: 20,
        }
    }
}

#[derive(Component, Default)]
pub struct WanderAi {
    pub cooldown: u32,
}

/// Player-only skill at cracking a program's ICE — raises decompile odds
/// independent of the target's HP or species difficulty, and grows on
/// player level-up (see `award_player_xp`). Creatures never attempt a
/// decompile themselves, so this never appears on them.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Decompiler {
    pub skill: i32,
}

#[derive(Component)]
pub struct Tamed {
    pub owner: Entity,
}

/// An item sitting in an `Equipment` slot, and the gear level its stat
/// bonus was scaled for when it was equipped (see
/// `items::EquipmentStats::scaled_for_level`). The level is captured at
/// equip time — like a wild program's zone-doubled stats, it doesn't
/// retroactively change if the player breaches deeper afterward; re-equip
/// (or unequip/re-equip) to pick up a newly unlocked level.
///
/// `fusion_tier` is the tier of the individual copy that went on — see
/// `FusedGear` and `items::EquipmentStats::fused_for_tier`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquippedItem {
    pub item: ItemId,
    pub level: u32,
    pub fusion_tier: u32,
}

/// What's currently equipped in each slot, on the player or on any program
/// they own — `Game::check_wearer` is the rule. Each slot's level-scaled
/// stat bonus (see `EquippedItem`, `Game::equipment_of`) is added directly
/// onto `Stats`/`Decompiler` when equipped and subtracted back on unequip —
/// mirroring how leveling directly mutates `Stats` elsewhere, rather than
/// maintaining a separate "base stats" layer.
///
/// Inserted on demand by `Game::equip` rather than at every spawn site: its
/// absence already reads as an empty loadout everywhere it is consulted, so
/// a program grows one only when it first wears something. Because the bonus
/// lives in `Stats`, **no stats operation may run while it is there** — see
/// `Game::gear_bonus` and `Game::strip_gear`.
#[derive(Component, Default, Clone)]
pub struct Equipment {
    pub weapon: Option<EquippedItem>,
    pub armor: Option<EquippedItem>,
    pub module: Option<EquippedItem>,
}

impl Equipment {
    pub fn slot_mut(&mut self, slot: EquipmentSlot) -> &mut Option<EquippedItem> {
        match slot {
            EquipmentSlot::Weapon => &mut self.weapon,
            EquipmentSlot::Armor => &mut self.armor,
            EquipmentSlot::Module => &mut self.module,
        }
    }

    pub fn get(&self, slot: EquipmentSlot) -> Option<EquippedItem> {
        match slot {
            EquipmentSlot::Weapon => self.weapon.clone(),
            EquipmentSlot::Armor => self.armor.clone(),
            EquipmentSlot::Module => self.module.clone(),
        }
    }
}

#[derive(Component, Default, Clone)]
pub struct Inventory {
    pub items: Vec<(ItemId, u32)>,
}

/// The abilities installed on this entity, in menu order — the player's and
/// every companion's entire kit. Length is bounded by
/// `Game::routine_slots`; position is what `BattleAction::Special::ability`
/// indexes.
///
/// A companion's species kit is *pre-installed* here rather than read from
/// `SpeciesDef` at menu time, which is what lets an innate ability be popped
/// out and plugged into a different program.
#[derive(Component, Default, Clone)]
pub struct Routines(pub Vec<AbilityId>);

/// Player-only: the gear the player has fused, one row per `(item, tier)`
/// — see `Game::fuse_item`. A fusion produces one stronger *physical copy*
/// (consuming `crate::tuning::ITEM_FUSION_COST` copies at the tier below
/// it), so a fused copy lives here while its ordinary spares stay in
/// `Inventory`.
///
/// **`Inventory` is by definition the tier-0 store**, and that is the seam
/// that keeps this out of the production chain entirely: recipes, `Stock`,
/// `assembler_system`, hauling and banking read `Inventory` and therefore
/// cannot encounter a fused copy, so none of them needs a tier rule. Tier
/// here is always >= 1.
///
/// Keyed by value rather than by position. Two copies of the same item at
/// the same tier are genuinely interchangeable, so an index would identify
/// nothing the pair does not — and it would be the positional-index trap
/// `BattleState::planned` documents, for no gain.
///
/// `EquippedItem::fusion_tier` is the same fact for a copy that is worn
/// rather than carried; this ledger agrees with it rather than shadowing
/// it, which is what the per-`ItemId` predecessor got wrong.
#[derive(Component, Default, Clone)]
pub struct FusedGear {
    pub copies: Vec<(ItemId, u32, u32)>,
}

impl FusedGear {
    pub fn add(&mut self, item: ItemId, tier: u32, qty: u32) {
        // Saturating for the reason `Inventory::add` is.
        if let Some(row) = self
            .copies
            .iter_mut()
            .find(|(i, t, _)| *i == item && *t == tier)
        {
            row.2 = row.2.saturating_add(qty);
        } else {
            self.copies.push((item, tier, qty));
        }
    }

    pub fn count(&self, item: &ItemId, tier: u32) -> u32 {
        self.copies
            .iter()
            .find(|(i, t, _)| i == item && *t == tier)
            .map(|(_, _, q)| *q)
            .unwrap_or(0)
    }

    /// Removes up to `qty` copies of `item` at `tier`, returning how many
    /// were actually removed. Drops the row at zero for the reason
    /// `Inventory::take` does — every screen lists these rows.
    pub fn take(&mut self, item: &ItemId, tier: u32, qty: u32) -> u32 {
        let Some(pos) = self
            .copies
            .iter()
            .position(|(i, t, _)| i == item && *t == tier)
        else {
            return 0;
        };
        let taken = self.copies[pos].2.min(qty);
        self.copies[pos].2 -= taken;
        if self.copies[pos].2 == 0 {
            self.copies.remove(pos);
        }
        taken
    }

    /// Every fused copy held, for the cargo total. Fused gear is carried
    /// like anything else, so it counts against the Buffer figure the
    /// player reads.
    pub fn total(&self) -> u32 {
        self.copies.iter().map(|(_, _, qty)| *qty).sum()
    }
}

impl Inventory {
    pub fn add(&mut self, item: ItemId, qty: u32) {
        // Saturating so an unbounded Buffer can never wrap a stack's count.
        if let Some(slot) = self.items.iter_mut().find(|(i, _)| *i == item) {
            slot.1 = slot.1.saturating_add(qty);
        } else {
            self.items.push((item, qty));
        }
    }

    pub fn count(&self, item: &ItemId) -> u32 {
        self.items
            .iter()
            .find(|(i, _)| i == item)
            .map(|(_, q)| *q)
            .unwrap_or(0)
    }

    /// Total units of ordinary cargo held. Banked currencies (an item with
    /// `ItemDef::banked` set) are excluded — this is just how full the
    /// (unbounded) Buffer is, shown to the player.
    pub fn cargo_used(&self, db: &ItemDb) -> u32 {
        self.items
            .iter()
            .filter(|(item, _)| !db.get(item.as_str()).is_some_and(|d| d.banked))
            .map(|(_, qty)| *qty)
            .sum()
    }

    /// Removes up to `qty` of `item`, returning how many were actually
    /// removed. Drops the slot entirely once it hits zero, rather than
    /// leaving a `(item, 0)` behind — callers that list `items` (the status
    /// panel, the inventory screen) shouldn't have to filter zero-quantity
    /// stacks themselves.
    pub fn take(&mut self, item: ItemId, qty: u32) -> u32 {
        let Some(pos) = self.items.iter().position(|(i, _)| *i == item) else {
            return 0;
        };
        let taken = self.items[pos].1.min(qty);
        self.items[pos].1 -= taken;
        if self.items[pos].1 == 0 {
            self.items.remove(pos);
        }
        taken
    }
}

#[derive(Component)]
pub struct Structure {
    pub kind: StructureId,
}

/// A structure's current upgrade tier, starting at 1. Present only on
/// structures whose definition sets `StructureDef::upgrade`.
#[derive(Component, Clone, Copy, Debug)]
pub struct StructureTier(pub u32);

/// A structure that can be worked for `resource`. Carries no deposit pool:
/// a node is not a reserve that gets mined down, it is a tap. What paces it
/// is `Stock::capacity` — it produces until its output buffer is full and
/// then clogs until someone collects.
#[derive(Component)]
pub struct ResourceNode {
    pub resource: ItemId,
    /// Mirrors `WorkDef::level`. `None` means a completed gather cycle
    /// always yields, same as before this field existed. `Some(level)` gates
    /// each completion behind a level-based percentage chance instead (see
    /// `systems::task_progress_system`) — a harder, chancier variant that a
    /// structure opts into via its `.ron` file rather than something every
    /// worked node does by default.
    pub level: Option<u32>,
}

/// A structure's local buffers — the whole of a production chain's
/// directionality.
///
/// Neighbours (and the player, collecting) may take from `output`. Nothing
/// outside a machine ever touches its `input`. That asymmetry is why a chain
/// flows one way without belts existing: a machine can only pull from what
/// its upstream has already finished, never reach into what upstream is
/// still working on.
///
/// `BTreeMap`, not `HashMap`, in both directions. Iteration order feeds the
/// pull phase, so a `HashMap` would make two machines competing for one
/// scarce feeder resolve differently between runs — and would make the save
/// encoding differ run to run as well.
///
/// `capacity` bounds `output` *in total*, not per item: it is how much the
/// box holds, so a machine that produces two things cannot dodge clogging by
/// splitting its output across them. `input` has no field here — it is
/// derived per ingredient from the recipe, at `INPUT_STOCK_BATCHES` batches,
/// so a greedy machine cannot drain a shared feeder dry.
#[derive(Component, Clone, Debug, Default)]
pub struct Stock {
    pub input: std::collections::BTreeMap<ItemId, u32>,
    pub output: std::collections::BTreeMap<ItemId, u32>,
    pub capacity: u32,
}

impl Stock {
    pub fn new(capacity: u32) -> Self {
        Self {
            capacity,
            ..Default::default()
        }
    }

    /// How many units are in `output`, across every item in it.
    pub fn output_used(&self) -> u32 {
        self.output.values().sum()
    }

    /// Room left in `output` before it clogs.
    pub fn output_room(&self) -> u32 {
        self.capacity.saturating_sub(self.output_used())
    }
}

/// A load a posted program is physically carrying to a depot.
///
/// The *only* stored hauling state, and deliberately so. Where the worker is
/// headed (the nearest depot while carrying, its own machine otherwise) and
/// whether it has arrived (`game::hauling::at_station`) are both derived
/// from `Position`, so there is one source of truth and no state field that
/// can desync into a worker standing at its machine insisting it is still
/// walking.
///
/// One `(item, qty)` pair rather than a map because
/// `tuning::HAUL_CARRY_CAPACITY` bounds a trip: `Stock::output` is a
/// `BTreeMap` and may hold several item ids, so an uncapped drain would have
/// had to carry all of them, and the save with it.
#[derive(Component, Clone, Debug)]
pub struct Carrying {
    pub item: ItemId,
    pub qty: u32,
}

/// A posted program that found no route to where it is trying to get. Set and
/// cleared every tick by `game::hauling::haul_step_system`, the one system
/// that walks a field and so the only one that can know.
///
/// A cache of that tick's answer rather than stored state, and not saved for
/// the same reason `MachineStatus` isn't: the walk that produced it runs
/// again on the next tick. It exists at all because the status it drives is
/// written by `systems::task_progress_system` — giving `MachineStatus` two
/// writers would have them ping-pong `Unstaffed`↔`Stranded` every tick, and
/// `set_machine_status` logs on every transition.
///
/// `task_progress_system` runs *first* in the chain, so it reads a marker
/// written on the previous tick. That one-tick lag on a status label is the
/// price of leaving the chain order alone, which is load-bearing for the
/// clog/pickup handoff (see `Game::build_schedule`).
#[derive(Component, Clone, Copy, Debug)]
pub struct Stranded;

/// Why a machine is or isn't producing. Present only on structures that
/// actually run a job (`StructureDef::work` or `::assembles`) — absence
/// means "not a machine", which is why a Home never reports a status it
/// could not possibly leave.
///
/// Deliberately not saved. It initialises to `Running` and is corrected on
/// the first tick, so a base that loads starved announces it once — which is
/// information the player wants, and costs no save field.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MachineStatus {
    #[default]
    Running,
    /// Input short: upstream is too slow, or isn't adjacent.
    Starved,
    /// Output full: downstream is too slow, or nobody has collected.
    Clogged,
    /// A program is posted but is not standing at the machine — walking to
    /// its post, carrying a load to a depot, or unable to reach the machine
    /// at all. Distinct from `Idle`, which means nobody is posted.
    ///
    /// Wins over both `Clogged` and `Running` whenever the worker is off its
    /// tile: after shedding a load the machine is no longer full, and
    /// without the precedence the pane would claim it is running while
    /// nothing produces.
    Unstaffed,
    /// A program is posted and cannot get here at all — no route to the
    /// machine or to any depot, usually because the base has been built
    /// around it. Strictly more specific than `Unstaffed`, which it wins
    /// over: both mean nobody is at the machine, but this one will not
    /// resolve itself by waiting, so it is the difference between a worker
    /// walking and a worker that needs the player to clear a path.
    ///
    /// Driven by the `Stranded` marker `haul_step_system` maintains.
    Stranded,
    /// No program assigned.
    Idle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskKind {
    GatherResource,
    /// Posted to defend a structure against raids (see `Game::raid_check`)
    /// without also working it — see `Game::assign_guard`. Unlike
    /// `GatherResource`, `task_progress_system` ignores this kind entirely;
    /// a guard doesn't produce anything even if its target happens to have
    /// a `ResourceNode`.
    Guard,
}

/// A generic ongoing job: `worker` progresses `target` over multiple ticks.
/// This is deliberately generic so base-building work and any future
/// colonist-style job assignment share one mechanism.
#[derive(Component)]
pub struct Task {
    pub kind: TaskKind,
    pub target: Entity,
    pub progress: u32,
    pub required: u32,
}

/// A status condition a battle `MoveDef::effect` can inflict on a combatant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusKind {
    /// Deals `ActiveStatus::power` damage at the end of every round it's
    /// active.
    Bleed,
    /// Causes the afflicted side to lose their next action in battle.
    Stun,
}

/// One combatant's currently active status condition, and how long it has
/// left.
#[derive(Clone, Copy, Debug)]
pub struct ActiveStatus {
    pub kind: StatusKind,
    /// Battle rounds remaining, ticked down at the end of every round bar
    /// the one it landed in — see `landed_this_round`.
    pub remaining: u32,
    /// Bleed damage dealt per round; unused for `Stun`.
    pub power: i32,
    /// True from being armed until the first `Game::tick_status_effects`
    /// after it, which spends itself clearing this flag and does nothing
    /// else — the round a condition lands in is not one of the rounds it
    /// lasts.
    ///
    /// Without it, end-of-round upkeep charged a round to a condition
    /// applied moments earlier in that same round: a `duration: 1` stun
    /// (every stun the shipped roster carries) expired before its victim's
    /// next turn, costing them nothing unless the attacker also happened to
    /// out-roll them on initiative, and `memory_leak`'s advertised "3
    /// rounds" of bleed dealt its first tick instantly and showed two.
    ///
    /// `Game::arm_status` is the only thing that sets it, which is why that
    /// is the only way to write `StatusEffects::active`.
    pub landed_this_round: bool,
}

/// A creature or the player can carry at most one status condition at a
/// time — a fresh application overwrites whatever was active, mirroring a
/// classic single-status-condition model rather than a stacking one.
/// Scoped to a single intrusion: cleared whenever a battle ends, however it
/// ends (kill, tame, flee, or the player going down).
#[derive(Component, Default, Clone, Copy)]
pub struct StatusEffects {
    pub active: Option<ActiveStatus>,
}

/// Which stat a companion's rally/shield temporarily boosts — see
/// `CombatBuff`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuffKind {
    Atk,
    Def,
}

/// One active combat buff, and how long it has left.
#[derive(Clone, Copy, Debug)]
pub struct ActiveBuff {
    pub kind: BuffKind,
    /// Battle rounds remaining, ticked down at the end of every round.
    pub remaining: u32,
    pub power: i32,
}

/// A temporary combat buff on any one combatant — the player from a
/// companion's Special or a pre-battle consumable, a companion from bracing
/// (see `Game::begin_defend`). Kept separate from `StatusEffects` because
/// that component is reserved for conditions a hostile move can inflict
/// (always unwanted), which shouldn't be clobbered by — or clobber — a buff.
///
/// Holds at most one buff at a time, so a fresh one overwrites whatever was
/// still active: a companion that braces gives up a Rally it was carrying,
/// which is a real cost of the choice.
///
/// Only the player is spawned holding this component; `begin_defend`
/// inserts it on demand for anyone else. Scoped to a single intrusion,
/// cleared with everything else when a battle ends.
#[derive(Component, Default, Clone, Copy)]
pub struct CombatBuff {
    pub active: Option<ActiveBuff>,
}

/// Which routine or item armed a `FieldBuff` entry. Drives the two
/// collision rules `Game::arm_field_buff` enforces — it is not shown to the
/// player, `ActiveFieldBuff::name` is.
///
/// **The order of these variants is part of the save format**, the same
/// constraint `Perk` documents (`perks.rs`): saves are bincode, which
/// encodes an enum positionally, so `PlayerSave`/`CreatureSave`'s
/// `field_buffs` store these values directly. Append new variants at the
/// end; never reorder or remove one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuffSource {
    Consumable,
    Routine,
}

/// Where a field buff lands. See `FieldBuffKind::scope`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldScope {
    /// Lands on whatever `AbilityTarget` names, and travels with that
    /// creature into whatever battle it's in.
    Creature,
    /// Always lands on the player, regardless of who cast it.
    Run,
}

/// A buff a field routine or consumable can arm outside combat that keeps
/// running after the map turn it was cast on — through any battle that
/// follows, unlike `CombatBuff` — and through a save.
///
/// **The order of these variants is part of the save format**, the same
/// constraint as `BuffSource` above: append new kinds at the end, never
/// reorder or remove one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldBuffKind {
    Regen,
    Coolant,
    Trickle,
    Def,
    Atk,
    Mitigation,
    CaptureBoost,
    XpBoost,
    EncounterDamp,
    DropBoost,
}

impl FieldBuffKind {
    /// Which end of a battle party this kind lands on. `Creature`-scoped
    /// kinds land on whatever `AbilityTarget` names; `Run`-scoped kinds
    /// always land on the player, since they're pressure or economy
    /// knobs the whole run feels rather than a single combatant's stats.
    pub fn scope(self) -> FieldScope {
        use FieldBuffKind::*;
        match self {
            Regen | Def | Atk | Mitigation => FieldScope::Creature,
            Coolant | Trickle | CaptureBoost | XpBoost | EncounterDamp | DropBoost => {
                FieldScope::Run
            }
        }
    }

    /// The affinity category `abilities::scaled_stat_power` scales this kind's
    /// authored magnitude against, the same call as
    /// `AbilityEffect::affinity_kind`. `None` for the four percentage-rate
    /// kinds: a rate isn't a magnitude in any of the five affinity
    /// categories, the same reasoning that gives `Cleanse`/`Decompile`
    /// `None` there.
    pub fn affinity_kind(self) -> Option<crate::abilities::AffinityKind> {
        use crate::abilities::AffinityKind;
        use FieldBuffKind::*;
        match self {
            Regen | Coolant | Trickle => Some(AffinityKind::Heal),
            Def | Atk | Mitigation => Some(AffinityKind::Buff),
            CaptureBoost | XpBoost | EncounterDamp | DropBoost => None,
        }
    }

    /// Whether `Game::cast_field_routine` should run this kind's authored
    /// `power` through `abilities::scaled_stat_power` (level and affinity) or
    /// deliver it unchanged. The five point-amount kinds scale; the five
    /// percentage-point kinds do not, for the reason `AbilityEffect::Drain`'s
    /// `heal_fraction` is excluded from `scaled_hp_power` too: a value that
    /// already carries its own ceiling doesn't need a second one stacked on
    /// top. A percentage is a property of the routine, not of how strong the
    /// caster is — scaling one the way a flat point value scales would let
    /// an authored 10% cut land anywhere up to 140% off a high-level,
    /// high-affinity caster, the exact ceiling-defeating outcome the cap on
    /// `Mitigation` exists to prevent.
    pub fn scales_with_caster(self) -> bool {
        use FieldBuffKind::*;
        match self {
            Regen | Coolant | Trickle | Def | Atk => true,
            Mitigation | CaptureBoost | XpBoost | EncounterDamp | DropBoost => false,
        }
    }

    /// The short tag a buff list shows next to a running entry, e.g.
    /// `"DEF+2"` or `"XP+15%"`. `power` is points for the five flat kinds
    /// and percentage points for the rest. `HP` rather than `INT` for
    /// `Regen`, matching the abbreviation the battle roster header already
    /// uses for Integrity (`render/battle.rs`) — Power and Fatigue have no
    /// established short form there, so `PWR`/`FTG` are new.
    ///
    /// `interval` only reaches the three over-time tags, and only shows when
    /// it is not 1: `HP+2/4t` reads as "2 every four turns", where the
    /// `/t` the other two keep already reads as "per turn". The flat and rate
    /// kinds have no per-tick effect at all (see `apply_field_buff_tick`), so
    /// a cadence on one of them would be describing something that does not
    /// happen.
    pub fn magnitude_label(self, power: i32, interval: u32) -> String {
        let every = if interval > 1 {
            interval.to_string()
        } else {
            String::new()
        };
        match self {
            FieldBuffKind::Regen => format!("HP+{power}/{every}t"),
            FieldBuffKind::Coolant => format!("FTG+{power}/{every}t"),
            FieldBuffKind::Trickle => format!("PWR+{power}/{every}t"),
            FieldBuffKind::Def => format!("DEF+{power}"),
            FieldBuffKind::Atk => format!("ATK+{power}"),
            FieldBuffKind::Mitigation => format!("DMG-{power}%"),
            FieldBuffKind::CaptureBoost => format!("TAME+{power}%"),
            FieldBuffKind::XpBoost => format!("XP+{power}%"),
            FieldBuffKind::EncounterDamp => format!("ENC-{power}%"),
            FieldBuffKind::DropBoost => format!("DROP+{power}%"),
        }
    }
}

/// One running field buff and how long it has left.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActiveFieldBuff {
    pub kind: FieldBuffKind,
    /// Display name of the ability or item that armed it, captured at cast.
    /// Stored rather than derived from `kind`: two different routines can
    /// arm the same kind, and the buff list has to tell them apart.
    pub name: String,
    pub power: i32,
    /// Ticks remaining. Turns (`Game::tick_inner`), not battle rounds like
    /// `ActiveBuff::remaining` — a field buff outlives any one battle.
    pub remaining: u32,
    /// Turns between firings — see `AbilityEffect::FieldBuff::interval`, which
    /// is where the authored value comes from. Carried on the running buff
    /// rather than looked up from the def it came from, because `name` is a
    /// display string and there is nothing here to look a def up *by*.
    ///
    /// The cadence is phased off `remaining`: a buff fires whenever
    /// `remaining % interval == 0`. That costs no second counter and no
    /// second save field, at the price of the phase depending on the
    /// duration — a duration that is a multiple of its interval, as every
    /// shipped one is, fires on the first tick and every interval-th after.
    #[serde(default = "crate::abilities::every_turn")]
    pub interval: u32,
    pub source: BuffSource,
}

/// Every field buff currently running on this entity. A `Vec`, not a
/// single slot like `CombatBuff`: a `Consumable` entry and a `Routine`
/// entry coexist, and distinct `Routine` kinds coexist with each other —
/// only a second buff from the *same* source (and, for `Routine`, the same
/// kind) displaces the one already running. `Game::arm_field_buff` is the
/// only writer and enforces both rules; nothing else may push or remove an
/// entry directly.
///
/// Only the player is spawned holding one; `arm_field_buff` inserts it on
/// demand for a companion a `Creature`-scoped buff lands on, the same
/// pattern `arm_buff` uses for `CombatBuff`.
#[derive(Component, Default, Clone)]
pub struct FieldBuff {
    pub active: Vec<ActiveFieldBuff>,
}

/// `buff`'s running `kind` power, `0` when none is active. Sums every
/// matching entry rather than reading just one: a `Consumable` and a
/// `Routine` of the same kind are required to coexist (`arm_field_buff`'s
/// whole reason for two separate displacement rules), and a reader that
/// only saw one of them would make that coexistence pointless — the buff
/// whichever entry it skipped would silently apply nothing.
///
/// A free function taking the component directly, not a method needing a
/// `Game`/`World`, so a plain bevy system with its own `Query<&FieldBuff>`
/// can call it too — `Game::field_buff_power` is a thin wrapper around this
/// for callers that already have an `Entity` and a `World` to fetch from.
pub fn field_buff_power_of(buff: &FieldBuff, kind: FieldBuffKind) -> i32 {
    buff.active
        .iter()
        .filter(|b| b.kind == kind)
        .map(|b| b.power)
        .sum()
}

/// Rounds remaining before each ability this combatant has spent can be
/// used again. Battle-scoped exactly like `CombatBuff` and `StatusEffects`
/// — armed during a fight, ticked at end of round, cleared when the
/// intrusion ends — so nothing here is ever persisted.
#[derive(Component, Default)]
pub struct AbilityCooldowns(pub std::collections::HashMap<crate::abilities::AbilityId, u32>);

/// An individual creature's innate quality roll, set once when it's
/// created (see `Game::spawn_wild_creature` / `Game::fuse_companions`)
/// and carried for its lifetime. `hp_roll`/`atk_roll`/`def_roll` are baked
/// into its starting `Stats` at creation time — this component doesn't
/// reapply them later, it just remembers what was rolled so
/// `quality_percent`/`quality_label` can describe it. `growth_roll`
/// actively scales `progression::add_xp`'s growth on every level-up, on
/// top of `SpeciesDef::growth_multiplier`.
#[derive(Component, Clone, Copy, Debug)]
pub struct Potential {
    pub hp_roll: f32,
    pub atk_roll: f32,
    pub def_roll: f32,
    pub growth_roll: f32,
}

impl Potential {
    /// Fallback for an entity with no roll of its own (e.g. a legacy save,
    /// or a test helper that spawns a creature directly) — every roll at
    /// its neutral 1.0, contributing neither a bonus nor a penalty.
    pub const NEUTRAL: Potential = Potential {
        hp_roll: 1.0,
        atk_roll: 1.0,
        def_roll: 1.0,
        growth_roll: 1.0,
    };

    /// A single 0-100 "how good is this individual" percentile: averages
    /// all four rolls and maps `MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL`
    /// onto 0-100. Purely a display aggregate — each roll still applies
    /// independently to its own stat/growth.
    pub fn quality_percent(&self) -> u32 {
        let avg = (self.hp_roll + self.atk_roll + self.def_roll + self.growth_roll) / 4.0;
        let pct = (avg - MIN_INDIVIDUAL_ROLL) / (MAX_INDIVIDUAL_ROLL - MIN_INDIVIDUAL_ROLL) * 100.0;
        pct.round().clamp(0.0, 100.0) as u32
    }

    /// A coarse, human-readable tier for `quality_percent` — shown next to
    /// a creature in the pets and inspect screens.
    pub fn quality_label(&self) -> &'static str {
        match self.quality_percent() {
            0..=19 => "Poor",
            20..=39 => "Below Average",
            40..=59 => "Average",
            60..=79 => "Above Average",
            _ => "Excellent",
        }
    }

    /// Averages two parents' rolls into one — used when fusing two
    /// companions into one (`Game::fuse_companions`), so the result's
    /// quality reflects both parents rather than an independent fresh
    /// roll.
    pub fn averaged(a: Potential, b: Potential) -> Potential {
        Potential {
            hp_roll: (a.hp_roll + b.hp_roll) / 2.0,
            atk_roll: (a.atk_roll + b.atk_roll) / 2.0,
            def_roll: (a.def_roll + b.def_roll) / 2.0,
            growth_roll: (a.growth_roll + b.growth_roll) / 2.0,
        }
    }
}

/// The rare-spawn tier a creature rolled when it was created, if any — see
/// `Game::roll_rarity`. A creature that rolled ordinary has no such
/// component at all, which reads as `Ordinary`, so no test fixture or
/// hand-built bundle has to know this exists.
///
/// **This is a record of a multiplier already spent, not a live one.**
/// `stat_mult` is applied exactly once, inside
/// `Game::spawn_wild_creature_scaled`, and baked into `Stats` there the same
/// way `Potential`'s three stat rolls are. `Game::load` restores those
/// numbers verbatim and `Game::fuse_companions` derives them from its
/// parents, so neither may re-apply it — the shape is
/// `EquippedItem::fusion_tier`, whose doc makes the same argument. Applying
/// it a second time compounds the bonus on every reload, invisibly, because
/// a stat carries no record of where it came from. The regression to head
/// off is a later reader finding a multiplier that appears to go unused and
/// "finishing the job"; `a_shiny_survives_a_save_round_trip` asserts the
/// stats come back *unchanged* for exactly that reason.
///
/// Player-facing text says Optimized/Overclocked rather than silver/gold,
/// which is the `MessageKind::Raid` / "GC Entropy Sweep" split: the enum and
/// the save field name the colours, everything a player reads names the
/// thing. `label` is the one place that translation happens.
///
/// **Variant order is save format.** bincode encodes enums positionally and
/// `CreatureSave::rarity` holds one, so append new tiers, never reorder —
/// the same trap `Perk` carries.
#[derive(
    Component, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
pub enum Rarity {
    #[default]
    Ordinary,
    Silver,
    Gold,
}

impl Rarity {
    /// What this tier multiplies every one of a creature's four stats by,
    /// on top of zone, depth and the individual `Potential` roll.
    pub fn stat_mult(self) -> f32 {
        match self {
            Rarity::Ordinary => 1.0,
            Rarity::Silver => SILVER_STAT_MULT,
            Rarity::Gold => GOLD_STAT_MULT,
        }
    }

    /// How the tier reads to a player, or `None` for an ordinary creature —
    /// `Option` rather than an empty string so a caller has to decide what
    /// "no tier" looks like in its own context, the way `fusion_color`
    /// returns `Option` so a louder rule can compose with it.
    pub fn label(self) -> Option<&'static str> {
        match self {
            Rarity::Ordinary => None,
            Rarity::Silver => Some("Optimized"),
            Rarity::Gold => Some("Overclocked"),
        }
    }
}

/// How many fusions deep a creature's lineage is (see
/// `Game::fuse_companions`). A creature that was caught or spawned
/// normally has no such component at all, which reads as 0; a fusion's
/// result carries `max(parent_a, parent_b) + 1`, so the number is the
/// *depth* of the deepest chain behind it, not a count of ancestors.
///
/// Once it hits `MAX_FUSIONS` that creature is a finished product: it
/// can't be used as an input to another fusion, which stops a player from
/// laundering an endless supply of duplicates into one runaway program.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FusionCount(pub u32);

/// How many percentage upgrades have been spent on this program — see
/// `Game::refactor_companion`. Absent means zero, the same way `FusionCount`
/// is absent on a program that has never been fused.
///
/// It counts **only** the percentage buffs. A Recompile Kernel raises
/// `ZonePortal` instead and spends nothing here, because that track bounds
/// itself against the player's own zone and a player forced to burn permanent
/// slots just staying current has had the feature taken away at exactly the
/// point it was meant to help.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Refactors(pub u32);

/// How many of this program's zone tiers were **bought** with Recompile
/// Kernels rather than earned by being tamed that deep. Absent means none,
/// like `Refactors` above.
///
/// It exists for one reason: `Game::program_payout` pays a fraction of
/// `Stats::power()`, and a kernel multiplies every one of those stats for
/// twelve printable Core Fragments. Left unrecorded, buying tiers and selling
/// the program prints Credits — measured at zone 7, a zone-1 program bought
/// up six tiers sold for 716 against 72 fragments' worth of kernels, and
/// Credits are the one currency that survives a breach. So the sale divides
/// the bought tiers back out, and a trader pays for what a program *is*.
///
/// `ZonePortal` cannot answer this on its own: a tier-4 program is worth
/// tier-4 money when it was tamed in zone 4, and this is what tells those two
/// apart. The percentage buffs deliberately are *not* divided out — five
/// slots is at most a 1.28x on power, which never repays the annealed cores
/// it costs, so there is nothing to close.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PurchasedTiers(pub u32);

/// A structure's remaining health against raids (see `Game::raid_check`).
/// Every deployed structure gets one, sized from its
/// `StructureDef::durability`; reaching 0 destroys the structure.
#[derive(Component, Clone, Copy, Debug)]
pub struct Durability {
    pub hp: u32,
    pub max_hp: u32,
}

/// A stationary spawner for a wild species. Present on the nest entity
/// itself, which also carries `Position`, `Glyph`, and `Durability` (all
/// reused as-is — a nest is destroyed the same way a structure is, just
/// via a direct bump-attack instead of a raid).
#[derive(Component, Clone, Debug)]
pub struct Nest {
    pub species: SpeciesId,
    /// Ticks remaining until each queued replacement guardian spawns —
    /// one entry per guardian currently missing from the nest's original
    /// count (see `systems`-adjacent `Game::nest_respawn_tick`). Emptied
    /// naturally once every missing guardian is back.
    pub pending_respawns: Vec<u32>,
}

/// A way down into the Stack, standing on the zone map. Carries `Position`
/// and `Glyph` alongside this, the way a `Nest` does, but no `Durability`:
/// an entrance is walked into, not attacked.
///
/// Walking onto one is checked in `Game::move_player` before the generic
/// blocking-structure test, the same way a nest and a zone portal are.
#[derive(Component, Clone, Copy, Debug)]
pub struct SurfaceLink;

/// Tags a wild program that was conjured for a Stack encounter rather
/// than found on the zone map.
///
/// It exists at surface coordinates like any other creature — the player's
/// `Position` is pinned to the link while they're underground, so that is
/// where the pack lands — but it has no business surviving the fight. Left
/// alive after a jack-out it would be standing around the link mouth when
/// the party climbs back out, a pack from a place the player left. So
/// `Game::end_battle` despawns whatever still carries this.
#[derive(Component, Clone, Copy, Debug)]
pub struct StackSpawn;

/// Tags a wild creature as tethered to a `Nest` — see
/// `systems::wander_ai_system`'s radius check. Removed (not the
/// creature) when its nest is destroyed (`Game::attack_nest`) or when the
/// creature itself is killed/tamed, at which point it either despawns or
/// resumes ordinary untethered behavior.
#[derive(Component, Clone, Copy, Debug)]
pub struct NestGuardian {
    pub nest: Entity,
}

/// Marks a `NestGuardian` roused by an attack on its own nest — driven at
/// the player instead of wandering (`systems::wander_ai_system` excludes
/// it; the drive itself is Task 4's `nest_aggro_tick`). Deliberately no
/// target field: the player is the only thing anything in this game
/// pursues. And deliberately no duration: the chase ends spatially or with
/// the nest, never on a timer — a second pursuable target or a timed chase
/// is the signal to revisit this, not to bolt a field onto it.
#[derive(Component, Clone, Copy, Debug)]
pub struct Pursuing;

/// Present on a structure whose `StructureDef::temporary` is set —
/// counts down by one on every ordinary game tick (see
/// `Game::tick_inner`) until it hits 0, at which point the structure
/// collapses. Ticks spent inside a `Game::rest` cycle deliberately don't
/// decrement this.
#[derive(Component, Clone, Copy, Debug)]
pub struct Temporary {
    pub ticks_remaining: u32,
}

/// Player-only: accumulated Perk Points (earned 1 per level-up) and which
/// perks have been bought with them. See `perks::Perk` — a perk can be
/// bought more than once, so `unlocked` holds one entry per level bought
/// (duplicates allowed) rather than a unique set.
#[derive(Component, Default, Clone)]
pub struct Perks {
    pub points: u32,
    pub unlocked: Vec<Perk>,
}

impl Perks {
    /// How many levels of `perk` have been bought — 0 if none.
    pub fn level(&self, perk: Perk) -> u32 {
        self.unlocked.iter().filter(|&&p| p == perk).count() as u32
    }
}

#[cfg(test)]
mod rarity_tests {
    use super::Rarity;

    #[test]
    fn rarity_multiplies_every_stat() {
        assert_eq!(Rarity::Ordinary.stat_mult(), 1.0, "ordinary must be inert");
        assert!(Rarity::Silver.stat_mult() > Rarity::Ordinary.stat_mult());
        assert!(Rarity::Gold.stat_mult() > Rarity::Silver.stat_mult());
    }

    #[test]
    fn only_a_rare_tier_has_a_label() {
        assert_eq!(Rarity::Ordinary.label(), None);
        assert!(Rarity::Silver.label().is_some());
        assert!(Rarity::Gold.label().is_some());
    }

    /// `Game::fuse_companions` inherits `max(parent_a, parent_b)`, so the
    /// derived `Ord` has to agree with the tiers' worth. Reordering the
    /// variants would silently make a fusion of a gold and a silver
    /// produce a silver — and reordering is already forbidden for a save
    /// format reason, so this pins the second consequence too.
    #[test]
    fn the_tiers_order_by_how_good_they_are() {
        assert!(Rarity::Gold > Rarity::Silver);
        assert!(Rarity::Silver > Rarity::Ordinary);
        assert_eq!(Rarity::Silver.max(Rarity::Gold), Rarity::Gold);
        assert_eq!(Rarity::default(), Rarity::Ordinary);
    }
}

#[cfg(test)]
mod potential_tests {
    use super::Potential;

    #[test]
    fn quality_percent_maps_the_roll_range_onto_0_to_100() {
        let worst = Potential {
            hp_roll: 0.8,
            atk_roll: 0.8,
            def_roll: 0.8,
            growth_roll: 0.8,
        };
        let neutral = Potential::NEUTRAL;
        let best = Potential {
            hp_roll: 1.2,
            atk_roll: 1.2,
            def_roll: 1.2,
            growth_roll: 1.2,
        };
        assert_eq!(worst.quality_percent(), 0);
        assert_eq!(neutral.quality_percent(), 50);
        assert_eq!(best.quality_percent(), 100);
    }

    #[test]
    fn quality_label_buckets_the_percent_into_a_coarse_tier() {
        assert_eq!(
            Potential {
                hp_roll: 0.8,
                atk_roll: 0.8,
                def_roll: 0.8,
                growth_roll: 0.8,
            }
            .quality_label(),
            "Poor"
        );
        assert_eq!(Potential::NEUTRAL.quality_label(), "Average");
        assert_eq!(
            Potential {
                hp_roll: 1.2,
                atk_roll: 1.2,
                def_roll: 1.2,
                growth_roll: 1.2,
            }
            .quality_label(),
            "Excellent"
        );
    }

    #[test]
    fn averaged_splits_the_difference_between_two_parents() {
        let a = Potential {
            hp_roll: 0.8,
            atk_roll: 1.0,
            def_roll: 1.2,
            growth_roll: 0.9,
        };
        let b = Potential {
            hp_roll: 1.2,
            atk_roll: 1.0,
            def_roll: 0.8,
            growth_roll: 1.1,
        };
        let fused = Potential::averaged(a, b);
        assert_eq!(fused.hp_roll, 1.0);
        assert_eq!(fused.atk_roll, 1.0);
        assert_eq!(fused.def_roll, 1.0);
        assert_eq!(fused.growth_roll, 1.0);
    }
}

#[cfg(test)]
mod inventory_tests {
    use super::*;
    use crate::items::ids;
    use std::path::Path;

    fn item_db() -> ItemDb {
        ItemDb::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/items"))
            .unwrap()
            .0
    }

    #[test]
    fn cargo_used_ignores_banked_currency() {
        let db = item_db();
        let mut inv = Inventory::default();
        inv.add(ItemId::from(ids::CORE_FRAGMENT), 5);
        inv.add(ItemId::from(ids::POWER_CELL), 3);
        inv.add(ItemId::from(ids::RESEARCH_DATA), 90);
        assert_eq!(
            inv.cargo_used(&db),
            8,
            "Research Data is banked, not carried"
        );
    }
}
