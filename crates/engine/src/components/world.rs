use super::*;

/// A wild creature that will fight rather than flee when engaged.
#[derive(Component)]
pub struct Hostile;

/// A wild program besieging the base — `game::siege::raiders`'s marker.
///
/// The third kind of body carrying a base-space `Position`, after a posted
/// program and a `DigSite`. `Game::stands_in_base_space` is the door that
/// sharpening matters at: without an arm there a besieger's board cell would
/// be read as a zone-surface tile the way an ordinary wild `Creature`'s is,
/// and the two coordinate spaces alias onto each other by design.
///
/// A unit marker rather than data — a besieger's behaviour lives in
/// `game::siege::raiders`, keyed off this component and off `components::
/// Carrying`, not off anything stored here.
#[derive(Component, Clone, Copy, Debug)]
pub struct Besieger;

/// Which structure a besieger's `Carrying` load came out of.
///
/// **Killing a carrier before the door drops the goods back into this
/// structure, if it still stands** — `game::siege::raiders`'s single most
/// load-bearing line, and what makes interception the shape of the fight
/// rather than a race. Written alongside `Carrying` at the moment of the
/// steal and read once, by `game::siege::raiders::drop_besieger_cargo` on
/// the reap that follows a kill.
#[derive(Component, Clone, Copy, Debug)]
pub struct StolenFrom(pub Entity);

/// Which leg of its journey a caravan is on.
///
/// **The stage decides which space the caravan stands in**, which is why
/// `Game::stands_in_base_space` reads a field rather than testing for the
/// component's presence: this is the first entity besides the player that
/// changes spaces, and asking "is it a caravan" would put it in one space
/// forever. See `in_base_space`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaravanStage {
    /// Walking across the zone surface toward the anchor.
    Approaching,
    /// Standing **on** the anchor tile, about to phase out.
    Docking,
    /// Inside base space, walking to the Market.
    Crossing,
    /// Standing beside the Market, open for trade.
    Docked,
    /// Back on the anchor tile, walking out the way it came.
    Leaving,
}

impl CaravanStage {
    /// Whether a caravan at this stage stands in base space.
    ///
    /// `Docking` and `Leaving` are the two transition ticks and both answer
    /// **false**, decided rather than defaulted: each is spent standing on
    /// the anchor tile *on the zone surface*, which is the tile the caravan
    /// walked to and the tile it walks away from. Answering true for either
    /// would draw it inside the base on a surface coordinate — base space's
    /// origin and the zone spawn point commonly alias, so the glyph would
    /// land somewhere plausible and stay wrong.
    pub fn in_base_space(self) -> bool {
        matches!(self, CaravanStage::Crossing | CaravanStage::Docked)
    }
}

/// A trader walking in, standing beside the Market, and walking out again.
///
/// **The journey is entity state; the schedule and the shelf are not.** When
/// this visit was due, who is walking in and what they carry are all derived
/// in `game/caravan.rs` from the base's own seed. What lives here is what
/// could not be derived: which leg it is on and where it is standing.
///
/// The third entity to carry a base-space `Position`, after a posted program
/// and a `DigSite` — and the first that carries one only *sometimes*. See
/// `CaravanStage::in_base_space`.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct Caravan {
    pub stage: CaravanStage,
    /// Which scheduled visit this caravan is, so it can be matched back to
    /// its own shelf after a reload.
    pub visit: u64,
    /// The surface tile it appeared on, and the one it walks back to. Kept
    /// rather than re-derived from the bearing, because a caravan that
    /// arrived under one sector's `ZoneLevel` must leave the way it came
    /// even if something about the derivation moves under it.
    pub arrival_tile: (i32, i32),
    /// How many ticks it has spent on the current stage. Read by the two
    /// stuck cases, which are the only things that care.
    pub stage_ticks: u32,
    /// Whether the base has already been told this trader cannot reach the
    /// counter. `DigSite::announced_stuck`'s field, rule and reason: the
    /// stall lasts the rest of the visit, and entering a state is news while
    /// staying in it is not.
    ///
    /// Not saved, for `DigSite::announced_stuck`'s reason too — it is a
    /// conversation's "I already said so", and a reload is exactly when the
    /// player should be told again.
    pub announced_stuck: bool,
}

/// A way down into the Stack, standing on the zone map. Carries `Position`
/// and `Glyph` alongside this, the way a `Nest` does, but no `Durability`:
/// an entrance is walked into, not attacked.
///
/// Walking onto one is checked in `Game::move_player` before the generic
/// blocking-structure test, the same way a nest and a zone portal are.
#[derive(Component, Clone, Copy, Debug)]
pub struct SurfaceLink;

/// The permanent door into base space — the player's pocket-dimension base,
/// stepped into by walking onto this tile (`Game::enter_base`, wired in a
/// later task). Modeled on `SurfaceLink`: it carries `Position` and `Glyph`
/// alongside this, and stands on the zone map like any other surface
/// entity.
///
/// Deliberately **not** a `Structure`. That is load-bearing rather than
/// cosmetic: "every `Structure` entity is in base space" is how this slice
/// tells a deployed building apart from anything standing on the surface,
/// with no marker component of its own — making the anchor a `Structure`
/// would silently break that rule.
///
/// There is exactly one, spawned once by `Game::new` (and restored once by
/// `Game::load`) and never destroyed: it carries no `Durability`, so
/// `run_raid`'s `(With<Durability>, With<Structure>)` query cannot select
/// it. Unlike a
/// `SurfaceLink`, it stands where it was founded for the life of the run:
/// a breach raises a tier and moves nothing, so the door back to the base
/// is where the player last left it. `Game::move_anchor_to` is the one
/// writer, and founding the Home is now its only caller.
#[derive(Component, Clone, Copy, Debug)]
pub struct BaseAnchor;

/// A settlement standing on the zone surface.
///
/// Carries only its key: everything about *who* it is lives in
/// `resources::Settlements`, which is what survives a save. The entity is
/// the drawable half and is rebuilt on load like any other map fixture.
#[derive(Component, Clone, Copy, Debug)]
pub struct Settlement {
    pub key: crate::settlements::SettlementKey,
}

/// The one cell of a settlement's footprint that a tether may name.
///
/// A footprint's outer cells are despawned and respawned as it grows and
/// shrinks (`Game::sync_settlement_footprint`), but `components::TownPatrol`
/// tethers to an *entity*, and `settlement_entity`/`towns_by_tile` have to
/// resolve to the same one every time. The centre is the cell a radius that
/// never drops below 1 can never despawn, so it carries this marker and
/// every other reader of "which entity is this settlement" filters on it.
#[derive(Component, Clone, Copy, Debug)]
pub struct SettlementCentre;

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

/// A creature that spawned as a boss.
///
/// Two things are bosses and only one of them is a species. An **apex**
/// species (`SpeciesDef::is_boss`) is always one and is hand-authored tough;
/// any other species can be **rolled** into one at `BOSS_SPAWN_CHANCE` and is
/// scaled by `tuning::BOSS_STAT_MULT` instead. This component is written at
/// both, so a query can ask without reaching for the db.
///
/// `Game::is_boss_creature` is still the one door, and it keeps the species
/// fallback: a fixture that hand-spawns an apex species outside `spawn_pack`
/// never gets a component, and must still be a boss.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Boss;

/// How many times this program has sent the party away from a fight —
/// escalated by `Game::mark_nemeses` on a jack-out or a Forgiving defeat,
/// never a win. Absent means it never has, and nothing in this feature ever
/// takes it back to absent: the cap (`tuning::MAX_NEMESES`) refuses a *new*
/// mark once full, but an existing one always still escalates, so demotion
/// is not a case any caller has to handle.
#[derive(Component, Clone, Copy, Debug)]
pub struct Nemesis(pub u32);

/// Ticks since a `Nemesis` last recruited, or since its band filled. Reset on
/// each recruit; `Game::nemesis_muster` inserts it on a nemesis that lacks one.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct NemesisMuster {
    pub ticks: u32,
}

/// A wild program that trails a `Nemesis`, pointing at its leader. Stops
/// wandering (`systems::wander_ai_system`), survives the wild-population
/// sweeps, and is dropped when the leader goes (`Game::nemesis_muster`).
#[derive(Component, Clone, Copy, Debug)]
pub struct NemesisFollower(pub Entity);

/// The zone cell a nemesis left to lead a siege from, so it can walk home
/// afterwards. Saved with the leader.
#[derive(Component, Clone, Copy, Debug)]
pub struct NemesisHome(pub Position);

/// Tags a wild creature as tethered to a `Nest` — see
/// `systems::wander_ai_system`'s radius check. Removed (not the
/// creature) when its nest is destroyed (`Game::attack_nest`) or when the
/// creature itself is killed/tamed, at which point it either despawns or
/// resumes ordinary untethered behavior.
#[derive(Component, Clone, Copy, Debug)]
pub struct NestGuardian {
    pub nest: Entity,
}

/// Tethers a wild program to the town that fielded it — Phase 7b's second
/// tether, `NestGuardian`'s sibling.
///
/// **A second component rather than a `Tethered { anchor, kind }` unifying
/// both.** Unifying buys one query and one save field; it costs a save
/// field whose *meaning* is rewritten rather than added, edits across ~19
/// files, and a `kind` match at every nest-specific site anyway —
/// `despawn_nest` strips tethers, `nest_has_pursuers` asks a nest question,
/// `combat_rewards` and `combat_round` look up the victim's nest. The
/// branching moves; it does not leave.
///
/// The two differ in every respect except the tick: what provokes them
/// (proximity here, an attack there), what frees them (the band being
/// repaired here, the nest dying there), what a save keys them by, and what
/// happens when the anchor dies — **a town cannot be destroyed**, so a
/// patrol's leash never resolves to nothing, which is a case
/// `NestGuardian`'s belt-and-braces check exists for and this does not need.
#[derive(Component, Clone, Copy, Debug)]
pub struct TownPatrol {
    pub town: Entity,
}

/// Marks a tethered program driven at the player instead of wandering —
/// a `NestGuardian` roused by an attack on its nest, or a `TownPatrol`
/// member that has noticed the player inside
/// `SETTLEMENT_PATROL_AGGRO_RADIUS`.
///
/// **Must only ever be inserted alongside one of the two tethers.** An
/// untethered `Pursuing` has no leash, so nothing in `pursuit_tick` can
/// ever clear it and the program chases across the whole zone forever.
///
/// `systems::wander_ai_system` excludes it; the drive itself is
/// `Game::pursuit_tick`. Deliberately no target field: the player is the
/// only thing anything in this game pursues. And deliberately no duration:
/// the chase ends spatially or with the tether, never on a timer — a second
/// pursuable target or a timed chase
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
