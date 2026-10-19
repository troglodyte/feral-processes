use super::*;

/// One status condition a combatant is carrying, and how long it has left.
/// What it does is the `StatusDef` its `id` names — see `statuses`.
#[derive(Clone, Debug)]
pub struct ActiveStatus {
    pub id: crate::statuses::StatusId,
    /// Battle rounds remaining, ticked down at the end of every round bar
    /// the one it landed in — see `landed_this_round`.
    pub remaining: u32,
    /// What a `DamagePerRound` status deals per round per stack; unused by
    /// the other behaviours.
    pub power: i32,
    /// How many applications this entry holds. Always 1 for a `Refresh`
    /// status.
    pub stacks: u32,
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
    /// is the only way to add to `StatusEffects::active`.
    pub landed_this_round: bool,
}

/// Every status condition a creature or the player is carrying, at most one
/// entry per `StatusId`. Scoped to a single intrusion: cleared whenever a battle ends, however it
/// ends (kill, tame, flee, or the player going down).
#[derive(Component, Default, Clone)]
pub struct StatusEffects {
    pub active: Vec<ActiveStatus>,
}

/// Which stat a companion's rally/shield temporarily boosts — see
/// `CombatBuff`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuffKind {
    Atk,
    /// Percentage points, the unit `Stats::mitigation` carries — not the
    /// subtractive absorption `Def` named before the combat model changed.
    Mitigation,
}

impl BuffKind {
    /// The stat this raises, as a screen names it — the word the combat log
    /// already uses.
    pub fn label(self) -> &'static str {
        match self {
            BuffKind::Atk => "Attack",
            BuffKind::Mitigation => "Mitigation",
        }
    }
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

/// Battle-scoped: a body no picker may name until it acts.
///
/// **Not a `CombatBuff`**, which holds exactly one wanted buff at a time —
/// bracing would clobber a cloak, and an ally's Rally would strip one — and
/// **not a `StatusEffects` entry**, which is reserved for conditions
/// inflicted on you and so would be stripped by an ally's `Cleanse`. It also
/// moves no stat, where a `BuffKind` is a stat axis folded into
/// `combatant_profile`.
///
/// It changes *who a picker may name* and nothing else: the filter is
/// applied at the five doors that name a body and deliberately at none of
/// the doors that resolve against one, which is why an area routine still
/// covers a cloaked cell.
///
/// Ticked down in `Game::tick_one_combatant` and cleared in
/// `game/combat_teardown.rs` beside `CombatBuff` and `AbilityCooldowns`, so
/// like both of those it appears nowhere in `save.rs` and cost no
/// `SAVE_FORMAT_VERSION` bump.
#[derive(Component, Clone, Copy, Debug)]
pub struct Cloaked {
    /// Battle rounds left. The cap is the ceiling on the situation; what
    /// usually ends a cloak first is `Game::break_cloak`.
    pub remaining: u32,
}

/// Battle-scoped: what a `Tamper` routine (`AbilityEffect::Tamper`) has done
/// to this body's decision-making. At most one entry per `TamperSlot` —
/// `TamperSlot`'s own doc is why Heat and Cold replace each other while
/// `Profiled` and `Injected` coexist.
///
/// **Ticked down on the tampered body's own turn**, in
/// `tactical/turn.rs::hand_on_turn`, and deliberately not in
/// `Game::tick_one_combatant` with `Cloaked` and the rest — see
/// `AbilityEffect::Tamper`'s doc for why a per-round age would silently
/// expire a short entry before its target ever took the turn it was aimed
/// at.
///
/// Battle-scoped for `Cloaked`'s reason: no serde derive, cleared in
/// `game/combat_teardown.rs`, and so this cost no `SAVE_FORMAT_VERSION`
/// bump.
///
/// **Absent means untampered.** Every mutator removes the component the
/// moment its map empties, so `get::<Tampered>().is_none()` is the one
/// question a reader needs to ask — `Cloaked`'s own convention, since that
/// component is removed rather than zeroed for the same reason.
#[derive(Component, Default, Debug, Clone)]
pub struct Tampered {
    entries: std::collections::BTreeMap<TamperSlot, TamperEntry>,
}

impl Tampered {
    /// Writes `kind` into its slot, replacing whatever was there — refresh,
    /// never stack, `TamperSlot`'s own rule.
    ///
    /// `fresh` marks an entry landed on the invoker's own body (a radius
    /// tamper catching its caster). Its first `age` consumes the mark
    /// instead of spending a turn the target never had — see `age`'s own
    /// doc and the design's Decision 5.
    pub fn apply(&mut self, kind: TamperKind, duration: u32, fresh: bool) {
        self.entries.insert(
            kind.slot(),
            TamperEntry {
                kind,
                remaining: duration,
                fresh,
            },
        );
    }

    /// The active `Temperature`, if any — `Game::decision_temperature`'s one
    /// read.
    pub fn temperature(&self) -> Option<f32> {
        match self.entries.get(&TamperSlot::Temperature)?.kind {
            TamperKind::Temperature(t) => Some(t),
            _ => None,
        }
    }

    pub fn has(&self, slot: TamperSlot) -> bool {
        self.entries.contains_key(&slot)
    }

    pub fn remove(&mut self, slot: TamperSlot) -> Option<TamperEntry> {
        self.entries.remove(&slot)
    }

    /// Ages every entry by one of the tampered body's own turns, returning
    /// the kinds that just expired.
    ///
    /// A `fresh` entry spends this call consuming the mark rather than its
    /// count: applied on the same turn it lands (the invoker's own radius
    /// catching itself), the first `age` it sees is the hand-on of the very
    /// turn that applied it, and a decrement there would be aging a turn
    /// the target never had.
    pub fn age(&mut self) -> Vec<TamperKind> {
        let mut expired = Vec::new();
        self.entries.retain(|_, entry| {
            if entry.fresh {
                entry.fresh = false;
                return true;
            }
            entry.remaining = entry.remaining.saturating_sub(1);
            if entry.remaining == 0 {
                expired.push(entry.kind);
                false
            } else {
                true
            }
        });
        expired
    }

    pub fn slots(&self) -> impl Iterator<Item = (TamperSlot, TamperKind)> + '_ {
        self.entries.iter().map(|(&slot, entry)| (slot, entry.kind))
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// One `Tampered` slot's live state — see `Tampered::apply`.
#[derive(Debug, Clone, Copy)]
pub struct TamperEntry {
    pub kind: TamperKind,
    /// The tampered body's own turns left before this expires — see
    /// `Tampered::age`.
    pub remaining: u32,
    fresh: bool,
}

/// Marks a body forked into a fight by a `Summon` routine — a wild spawn
/// fighting on your side for the length of one battle.
///
/// Battle-scoped for `Cloaked`'s reason: `Game::finish_fight` sweeps every
/// holder unconditionally, living or not, so a marked body can never reach
/// a save and this cost no `SAVE_FORMAT_VERSION` bump.
///
/// Containment is otherwise entirely by *omission* — a fork never passes
/// through `Game::roster_parts`, so it carries no `Tamed`, no `Experience`
/// and no `ProgramId`, and is invisible to the roster, the base, the save
/// and `award_companion_xp` with no exclusion written anywhere. This marker
/// is read only where a check is genuinely needed: the teardown sweep, the
/// `bench_or_dissolve` skip, and `Game::tactical_ai_actor`'s gate.
#[derive(Component, Clone, Copy, Debug)]
pub struct Summoned;

/// Marks a `Summoned` body raised by the `Reanimate` effect (Respawn), on
/// either side, rather than forked fresh by a `Summon` routine.
///
/// **What splits the two summon pools.** `Game::dissolve_summons` narrows
/// to `With<Summoned>, Without<Respawned>` so a fork's own recast never
/// touches a raised body and vice versa (`Game::dissolve_respawned`, the
/// per-side counterpart); `tactical::view::TacticalBody::respawned` reads it
/// for the renderer's own tint. Everything `Summoned` already covers for
/// containment — no `Tamed`, swept unconditionally by `finish_fight` —
/// applies here too, since this always rides alongside it.
#[derive(Component, Clone, Copy, Debug)]
pub struct Respawned;

/// A Drop Trooper called into a battle map by `Game::call_reinforcement` —
/// a real roster body riding in `Party` for this fight alone, above
/// `MAX_PARTY_SIZE` if need be. `Game::finish_fight` takes every holder
/// back out of `Party` and strips the marker, living or not, so it never
/// reaches a save (an ordinary tactical fight is not saved mid-fight, and
/// a siege refuses the call).
///
/// `reorienting` is its lost first turn: **a `Stun` does not cost a turn on
/// a battle map** (it only stops a walk), so `Game::hand_on_turn` passes the
/// holder's first turn itself and clears this.
#[derive(Component, Clone, Copy, Debug)]
pub struct Reinforcement {
    pub reorienting: bool,
}

/// Several wild programs of one species folded into a single enhanced body
/// on a battle map — `tactical::squads::plan` decides who folds, and
/// `Game::spawn_squad` is the one place this component (and the rest of a
/// squad's shape) is written, `spawn_structure`'s rule.
///
/// **Never saved and never seen outside a fight.** A squad carries no world
/// `Position`, so `creature_save_for` skips it exactly as it skips a
/// `Summoned` fork; the last squad standing disbands back into `members`
/// (`Game::disband_squad`) the moment the fight that formed it ends, one
/// way or another. `members` is drained by a successful capture
/// (`Game::decompile_squad`) and by death (`reap_tactical_dead`, which pays
/// each remaining name here its own kill before despawning the squad
/// itself) — an emptied list is a second way a squad dies, independent of
/// its own `Stats::hp`.
#[derive(Component, Clone, Debug)]
pub struct Squad {
    pub members: Vec<Entity>,
    pub formation: usize,
}

/// Battle-scoped: when a reach weapon's swing may next be a wide one.
///
/// Battle-scoped for `Cloaked`'s reason and with `Cloaked`'s consequence —
/// it is removed in `game/combat_teardown.rs` beside `CombatBuff` and
/// `AbilityCooldowns`, so it appears nowhere in `save.rs` and cost no
/// `SAVE_FORMAT_VERSION` bump. Left set, it would follow the wielder out of
/// one fight and hold their first swing of the next one narrow, which is
/// the half a player would read as the weapon being broken.
///
/// Inserted on demand by `Game::arm_reach_charge` rather than at any spawn
/// site: absent already reads as "ready", which is what a body that has not
/// swung wide yet is.
#[derive(Component, Clone, Copy, Debug)]
pub struct ReachCharge {
    /// The first round on the holding model's own counter whose swing may be
    /// wide again. A `recharge` of 0 arms it for the round it was armed in
    /// and so is inert rather than special-cased — nothing divides by it.
    pub ready_on: u32,
}

/// Battle-scoped: a charge routine mid-wind-up. Removed only by
/// `Game::end_charge`, and in `game/combat_teardown.rs` beside the other
/// fight-only state, so it appears nowhere in `save.rs`.
///
/// Not `ReachCharge`, which is an unrelated weapon state.
#[derive(Component, Clone, Debug)]
pub struct Charging {
    pub ability: AbilityId,
    /// Turns of charging that reach full power (`ChargeSpec::rounds`), N.
    pub rounds: u32,
    /// Turns spent charging so far, k: 1 on the turn it starts. At
    /// `progress == rounds` the routine fires on its own.
    pub progress: u32,
    pub aim: ChargeAim,
}

/// Where a charge was aimed when it began.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChargeAim {
    /// Group battles: the members of the target group at the start. A set and
    /// not one body, because the front member dying must not read as the
    /// group being gone; the group is whichever one still holds any of them.
    Group(Vec<Entity>),
    /// Battle map: absolute board cells, so a victim can step out of them.
    Cells(Vec<(i32, i32)>),
}

/// The player's current image — `game::kit::Kit::Emulated`'s state, todo
/// #100. Only the player ever carries this.
///
/// Battle-scoped for `Cloaked`'s reason: no serde derive, dropped in
/// `game/combat_teardown.rs` beside `CombatBuff` and `AbilityCooldowns`, so
/// it appears nowhere in `save.rs` and costs no `SAVE_FORMAT_VERSION`
/// bump — a fight in progress is never saved.
#[derive(Component, Clone, Debug)]
pub struct Emulation {
    /// Which species' kit `game::kit::Kit::Emulated` reads.
    pub species: SpeciesId,
    /// Ages by one in `Game::tick_one_combatant`, beside `tick_combat_buff`
    /// — at zero, `Game::drop_emulation` removes this and logs the lapse.
    pub rounds_left: u32,
}

/// Which routine or item armed a `FieldBuff` entry. Drives the two
/// collision rules `Game::arm_field_buff` enforces — it is not shown to the
/// player, `ActiveFieldBuff::name` is.
///
/// **The order of these variants is part of the save format**, the same
/// constraint `Perk` documents (`perks.rs`): saves are bincode, which
/// serializes an enum by *name*, so a variant may be reordered freely and
/// renaming one is what breaks a save. See `FieldBuffKind` below.
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
    /// Always lands on the player, regardless of who ran it.
    Run,
}

/// A buff a field routine or consumable can arm outside combat that keeps
/// running after the map turn it was run on — through any battle that
/// follows, unlike `CombatBuff` — and through a save.
///
/// A variant *name* is the save format; the order is not. That is the RON
/// encoding `SAVE_FORMAT_VERSION` 29 moved to — the claim these two enums
/// carried about positional bincode had been stale since. Renaming a variant
/// still breaks a save; reordering no longer does. `Def` was removed under
/// that rule at version 31, which is why it earned a bump.
///
/// There is deliberately no `Def` beside `Mitigation`. Once `Stats::def`
/// became percentage points there was no flat-defence axis left for a second
/// name to describe, and two names on one axis is what makes both unreadable
/// wherever they are summed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldBuffKind {
    Regen,
    Trickle,
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
            Regen | Atk | Mitigation => FieldScope::Creature,
            Trickle | CaptureBoost | XpBoost | EncounterDamp | DropBoost => FieldScope::Run,
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
            Regen | Trickle => Some(AffinityKind::Heal),
            Atk | Mitigation => Some(AffinityKind::Buff),
            CaptureBoost | XpBoost | EncounterDamp | DropBoost => None,
        }
    }

    /// Whether `Game::run_field_routine` should run this kind's authored
    /// `power` through `abilities::scaled_stat_power` (level and affinity) or
    /// deliver it unchanged. The two point-amount kinds scale; the rest do
    /// not, for the reason `AbilityEffect::Drain`'s `heal_fraction` is
    /// excluded from `scaled_hp_power` too: a value that already carries its
    /// own ceiling doesn't need a second one stacked on top. A percentage is
    /// a property of the routine, not of how strong the invoker is — scaling
    /// one the way a flat point value scales would let an authored 10% cut
    /// land anywhere up to 140% off a high-level, high-affinity invoker, the
    /// exact ceiling-defeating outcome the cap on `Mitigation` exists to
    /// prevent.
    ///
    /// **`Trickle` is excluded by that same rule, and it is the difference
    /// between it and `Regen` that decides it.** Both restore a pool, but
    /// `Regen`'s ceiling is `max_hp`, which grows with level — so a scaled
    /// heal stays the same fraction of the bar. Power's ceiling is
    /// `POWER_MAX`, a fixed 100 forever. Scaled, an authored `power: 1` is 7 a
    /// turn at the level cap, which pins a full reserve for the buff's whole
    /// duration and makes the authored number untunable: the level term
    /// swamps whatever the file says. Unscaled, `power` means what it says
    /// at every level.
    pub fn scales_with_invoker(self) -> bool {
        use FieldBuffKind::*;
        match self {
            Regen | Atk => true,
            Trickle | Mitigation | CaptureBoost | XpBoost | EncounterDamp | DropBoost => false,
        }
    }

    /// Whether a **routine**-armed buff of this kind runs until the party
    /// rests instead of counting turns down. The fourth per-kind rule on
    /// this enum, beside `scope`, `affinity_kind` and `scales_with_invoker`,
    /// and read only through `ActiveFieldBuff::runs_until_rest` — which is
    /// where the `BuffSource::Routine` half of the predicate lives, and why
    /// this one is not called anywhere directly except there and the load
    /// check that refuses a `duration` it would ignore.
    ///
    /// **The two over-time kinds are the exceptions, and both directions of
    /// that are load-bearing.** `Regen` and `Trickle` are the only kinds
    /// with a per-tick effect (see `Game::apply_field_buff_tick`); the rest
    /// are read on demand and do nothing as a turn passes. So an until-rest
    /// `Regen` is unbounded healing and an until-rest `Trickle` is unbounded
    /// Power, which is the whole of the Stack's scarcity. They are also the
    /// only two that use `interval`, whose cadence is phased off
    /// `ActiveFieldBuff::remaining` — a counter an until-rest buff does not
    /// have. Excluding them here is what lets `Game::tick_field_buffs` leave
    /// its cadence filter alone.
    pub fn runs_until_rest(self) -> bool {
        use FieldBuffKind::*;
        match self {
            Regen | Trickle => false,
            Atk | Mitigation | CaptureBoost | XpBoost | EncounterDamp | DropBoost => true,
        }
    }

    /// The short tag a buff list shows next to a running entry, e.g.
    /// `"DEF+2"` or `"XP+15%"`. `power` is points for the four flat kinds
    /// and percentage points for the rest. `HP` rather than `INT` for
    /// `Regen`, matching the abbreviation the battle roster header already
    /// uses for Integrity (`render/battle.rs`) — Power had no established
    /// short form there, so `PWR` is new.
    ///
    /// `interval` only reaches the two over-time tags, and only shows when
    /// it is not 1: `HP+2/4t` reads as "2 every four turns", where the
    /// `/t` the other keeps already reads as "per turn". The flat and rate
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
            FieldBuffKind::Trickle => format!("PWR+{power}/{every}t"),
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
    /// Display name of the ability or item that armed it, captured at invocation.
    /// Stored rather than derived from `kind`: two different routines can
    /// arm the same kind, and the buff list has to tell them apart.
    pub name: String,
    pub power: i32,
    /// Ticks remaining. Turns (`Game::tick_inner`), not battle rounds like
    /// `ActiveBuff::remaining` — a field buff outlives any one battle.
    ///
    /// **Not a lifetime when `runs_until_rest` below says so.** Nothing
    /// decrements it for those and nothing reads it but the cadence filter,
    /// which no until-rest kind reaches. An old save's in-flight routine
    /// buff of such a kind therefore loads with whatever count it had and
    /// simply stops ageing, which is the new behaviour and needs no
    /// migration.
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

impl ActiveFieldBuff {
    /// Whether this buff has no turn count at all and runs until the party
    /// rests. **The one definition**, read by `Game::tick_field_buffs` (which
    /// skips ageing it), `drop_until_rest_buffs` below (which ends it) and
    /// `duration_label` (which says so on the row).
    ///
    /// **It reads `source` as well as `kind`, and that half is the load-bearing
    /// one.** `ItemEffect::prebattle_buff` arms this same struct from a
    /// one-shot consumable with its own authored `ticks` — `patch_routine`
    /// arms Mitigation for 120 of them. Keyed on the kind alone, that item
    /// would have gone permanent too, and since `field_buff_power_of` sums a
    /// `Consumable` and a `Routine` entry of one kind rather than picking
    /// between them, its 10% would have stacked under the routine's for the
    /// rest of the expedition. A routine is repeatable and priced in a
    /// reserve that rest refills; an item is spent. Only the routine's half
    /// of that pair was ever meant to last the trip.
    pub fn runs_until_rest(&self) -> bool {
        self.source == BuffSource::Routine && self.kind.runs_until_rest()
    }

    /// The lifetime tag a buff row shows, e.g. `"90t"` or `"until rest"` —
    /// the sibling of `FieldBuffKind::magnitude_label` and built here for
    /// the same reason: a row transform belongs to the engine, so the map's
    /// buff list and the ally picker's "this is what you'd replace" line
    /// cannot describe one buff two ways.
    pub fn duration_label(&self) -> String {
        if self.runs_until_rest() {
            // One word, not "until rest": the map's status column has room
            // for a seven-character tag beside a full-width name and
            // magnitude, and `draw_status_buffs` measures nothing —
            // `draw_row` clips rows vertically and never horizontally, so a
            // longer tag runs off the panel in silence. Pinned by
            // `render::field::tests::the_widest_until_rest_buff_row_fits_the_status_column`.
            "rest".to_string()
        } else {
            format!("{}t", self.remaining)
        }
    }
}

/// Ends every until-rest buff on one holder, returning the names of what
/// went so the caller can log it.
///
/// A free function taking the component directly, for exactly the reason
/// `field_buff_power_of` below is one: the two callers are `Game::rest`, a
/// method, and `difficulty::death_handling_system`, a plain bevy system with
/// no `Game` to reach through. The same split
/// `CLAUDE.md` records for `game::stack::surfaced`.
pub fn drop_until_rest_buffs(buff: &mut FieldBuff) -> Vec<String> {
    let dropped: Vec<String> = buff
        .active
        .iter()
        .filter(|b| b.runs_until_rest())
        .map(|b| b.name.clone())
        .collect();
    buff.active.retain(|b| !b.runs_until_rest());
    dropped
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
