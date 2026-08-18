//! Populating a zone with wild programs, nests, and habitat-born
//! creatures.

use crate::resources::PopulatedChunks;
use crate::tuning::{
    BOSS_SPAWN_CHANCE, MAX_ENEMY_GROUPS, MAX_GROUP_SIZE, NEST_DURABILITY, NEST_GUARDIAN_MAX,
    NEST_GUARDIAN_MIN, NEST_SPAWN_CHANCE, NEST_TETHER_RADIUS, OPENING_RING_TILES,
    PACK_GATHER_RADIUS, POPULATION_CHUNK_MARGIN, WILD_CREATURE_CAP, ZONE_GROUP_STEP,
    chunk_wild_population,
};
use crate::tuning::{
    GOLD_SPAWN_CHANCE, GROUP_SIZE_DISTANCE_GROWTH, GROUP_SIZE_STEP_FRAMES, GROUP_SIZE_STEP_ZONES,
    MAX_GROUP_SIZE_STEPS, PLATINUM_SPAWN_CHANCE, PRISMATIC_SPAWN_CHANCE, SILVER_SPAWN_CHANCE,
    WILD_LOCAL_DENSITY_TARGET, WILD_ROUTINE_CHANCE, WILD_SPAWN_CHANCE, WILD_SPAWN_RADIUS_TILES,
};
use crate::world::CHUNK_SIZE;
use crate::*;

/// How large a pack may roll once Trace has been folded in: `base` scaled by
/// the band's multiplier, then clamped back under the zone's own ceiling.
///
/// The clamp is the point. Trace makes the party reach their zone's ceiling
/// *faster* — regardless of how far out they are — but must never raise it.
/// `zone_group_cap` is a balance bound on how big any fight in a zone can
/// get, and a meter the player runs up themselves should not vault it.
///
/// That is also where the lever's zone-1 inertness comes from rather than
/// being a special case: `zone_group_cap(1)` is 1, so the clamp pins every
/// group to a single member whatever Trace says.
pub(crate) fn trace_group_ceiling(base: u32, group_mult: u32, cap: u32) -> u32 {
    base.saturating_mul(group_mult).clamp(1, cap.max(1))
}

/// Everything that makes a spawn harder than its own tile would suggest,
/// decided by the caller and handed in as one value.
///
/// The bundling is the rule, not a convenience. Each of these is a property
/// of *where the party is*, and `spawn_pack` must never read any of them off
/// the world: ambient surface spawns and nest respawns keep rolling on every
/// `tick` while the party is underground, so a factor read inside the spawn
/// scales those too — which is how oversized packs once ended up waiting at
/// the link mouth for the climb out. Passing them together gives that rule
/// one place to live instead of three, and `surface()` names the case where
/// there is no escalation at all.
#[derive(Clone, Copy)]
pub(crate) struct SpawnEscalation {
    /// Multiplier on each member's stats — `Game::stack_depth_multiplier`
    /// underground, 1.0 on the surface.
    pub(crate) stat_mult: f32,
    /// Multiplier on the group-size ceiling — `TRACE_GROUP_MULT`'s band
    /// value, clamped back under the zone cap by `trace_group_ceiling`.
    pub(crate) group_mult: u32,
    /// Frames descended, or `None` on the surface, where the zone decides
    /// the same step instead — see `Game::danger_steps`.
    pub(crate) depth: Option<u32>,
}

impl SpawnEscalation {
    /// An ordinary surface spawn: baseline stats, no Trace, no depth.
    /// `danger_steps` reads the zone for it.
    pub(crate) fn surface() -> Self {
        Self {
            stat_mult: 1.0,
            group_mult: 1,
            depth: None,
        }
    }
}

/// The zone's ceiling on one species group: zone 1 is solo, every level
/// after adds `ZONE_GROUP_STEP`, and `MAX_GROUP_SIZE` is the hard stop.
/// Saturating arithmetic because zones are unbounded — the clamp is the
/// intent, an overflow partway to it is not.
pub(crate) fn zone_group_cap(zone: u32) -> u32 {
    ZONE_GROUP_STEP
        .saturating_mul(zone.saturating_sub(1))
        .saturating_add(1)
        .clamp(1, MAX_GROUP_SIZE)
}

/// How far a group of `n` scatters when it spawns, and how far `gather_pack`
/// searches from the member the player bumped — the same formula, but not
/// the same input: spawning passes the size it actually rolled, gathering
/// passes the zone's `max_group_size` ceiling. A roll is usually smaller
/// than its ceiling, so a scattered cluster usually pulls into one fight but
/// not always — a fringe member can be left for the next bump, which is the
/// cheap direction to err in. `PACK_GATHER_RADIUS` stays the floor: nothing
/// gets tighter than it was.
pub(crate) fn swarm_radius(n: u32) -> i32 {
    PACK_GATHER_RADIUS.max(crate::battle::ceil_sqrt(n) as i32)
}

/// `WILD_SPAWN_CHANCE`, cut by `damp_pct` percentage points of a running
/// `FieldBuffKind::EncounterDamp` field buff. Floored at 0 rather than
/// allowed to go negative, so a large enough buff can suppress wandering
/// encounters entirely but never invert into a spawn bonus.
pub(crate) fn damped_wild_spawn_chance(damp_pct: i32) -> f64 {
    (WILD_SPAWN_CHANCE * (1.0 - damp_pct as f64 / 100.0)).max(0.0)
}

/// How often each rung comes up. Exhaustive **on purpose**: a rung added to
/// `Rarity` without a chance beside it is a compile error here rather than a
/// tier that ships and never rolls, which is the failure a `_ =>` arm would
/// hide. Same argument `render/stack.rs::cell_mark` makes about a new
/// `CellKind` drawing as bare floor.
fn rarity_spawn_chance(tier: Rarity) -> f64 {
    match tier {
        Rarity::Ordinary => 0.0,
        Rarity::Silver => SILVER_SPAWN_CHANCE,
        Rarity::Gold => GOLD_SPAWN_CHANCE,
        Rarity::Platinum => PLATINUM_SPAWN_CHANCE,
        Rarity::Prismatic => PRISMATIC_SPAWN_CHANCE,
    }
}

/// Which rung a single `0.0..1.0` roll lands on, walking `Rarity::ALL`
/// **rarest first** so the thresholds accumulate and cannot sum past 1.0 —
/// one draw decides the tier, and each rung is genuinely rarer than the one
/// below rather than a separate chance landing on top of it.
///
/// The one definition of the ladder. `Game::roll_rarity` is its caller
/// today; gear rolls the same tiers and will share this rather than getting
/// its own chances, because two copies would let a weapon and a program
/// disagree about how rare "Overclocked" is while sharing the word and the
/// colour that say they don't.
pub(crate) fn rarity_for_roll(roll: f64) -> Rarity {
    let mut ceiling = 0.0;
    for tier in Rarity::ALL.into_iter().rev() {
        ceiling += rarity_spawn_chance(tier);
        if roll < ceiling {
            return tier;
        }
    }
    Rarity::Ordinary
}

impl Game {
    /// Rolls whether a fresh wild program carries a routine, and which —
    /// the `Routines` payload for one creature, empty on the (usual) miss.
    ///
    /// Two rolls, deliberately separate: `WILD_ROUTINE_CHANCE` decides
    /// whether there is a carrier at all, and the per-ability `wild_weight`
    /// decides what it holds. Folding them into one would mean adding an
    /// ability to the pool changed how often carriers appear.
    ///
    /// Exactly one routine. A carrier is a prize, and a second would need a
    /// slot policy at capture time that nothing else in the design needs.
    pub(crate) fn roll_wild_routine(&mut self) -> Vec<crate::abilities::AbilityId> {
        let carries = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(WILD_ROUTINE_CHANCE)
        };
        if !carries {
            return Vec::new();
        }
        let pool: Vec<(crate::abilities::AbilityId, u32)> = self
            .world
            .resource::<AbilityDb>()
            .wild_pool()
            .into_iter()
            .map(|(def, weight)| (def.id.clone(), weight))
            .collect();
        let total: u32 = pool.iter().map(|(_, w)| w).sum();
        if total == 0 {
            return Vec::new();
        }
        let roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(0..total)
        };
        let weights: Vec<u32> = pool.iter().map(|(_, w)| *w).collect();
        crate::abilities::weighted_pick(&weights, roll)
            .map(|index| vec![pool[index].0.clone()])
            .unwrap_or_default()
    }

    /// Spawns a wild creature of `species_id` at `(x, y)`, returning its
    /// `Entity` — `None` only if `species_id` isn't in `SpeciesDb` (every
    /// real call site passes an id it already validated against
    /// `SpeciesDb`, so this is a defensive no-op path, not an expected
    /// outcome). `spawn_nest_guardian` uses the returned entity to attach
    /// `NestGuardian`.
    ///
    /// This is the surface spawn: depth costs it nothing. A spawn that
    /// *is* a Stack encounter goes through `spawn_pack` with a
    /// multiplier — see `depth_mult` there for why that is a parameter
    /// rather than something read back off the party's locale.
    pub(crate) fn spawn_wild_creature(
        &mut self,
        species_id: &str,
        x: i32,
        y: i32,
    ) -> Option<Entity> {
        self.spawn_wild_creature_scaled(species_id, x, y, 1.0, false)
    }

    /// `spawn_wild_creature` with `depth_mult` folded into every stat.
    ///
    /// `pub(crate)` for `Game::adopt_orphan`, which is a sibling module and
    /// spawns the same way a Stack encounter does — depth-scaled, and with
    /// its potential and wild routines rolled from `GameRng` at spawn time
    /// like every other creature. Only *which species* an orphan is was ever
    /// pinned to the frame seed; what it turns out to be worth is not.
    pub(crate) fn spawn_wild_creature_scaled(
        &mut self,
        species_id: &str,
        x: i32,
        y: i32,
        depth_mult: f32,
        boss: bool,
    ) -> Option<Entity> {
        let species = self
            .world
            .resource::<SpeciesDb>()
            .get(species_id)
            .cloned()?;
        let zone_level = self.world.resource::<ZoneLevel>();
        let mult = zone_level.stat_multiplier() as f32;
        let zone = zone_level.0;
        let potential = self.roll_potential();
        let routines = self.roll_wild_routine();
        let rarity = self.roll_rarity(&species, x, y, boss);
        // An apex species is authored tough; only a rolled boss takes the
        // multiplier. The component goes on both, so a query need not ask
        // which.
        let boss_mult = if boss && !species.is_boss {
            crate::tuning::BOSS_STAT_MULT
        } else {
            1.0
        };
        // Rarity multiplies here, and in exactly one other place:
        // `Game::promote_rarity`, which escalates a nemesis's tier by the
        // *ratio* between its old and new multiplier rather than reapplying
        // this one. It is baked into `Stats` the same way `Potential`'s
        // three stat rolls are, and the component that rides along is the
        // receipt — see `Rarity`'s doc for why nothing else may apply it.
        let rarity_mult = rarity.stat_mult();
        let scale = |base: i32, roll: f32| {
            ((base as f32) * mult * depth_mult * boss_mult * rarity_mult * roll).round() as i32
        };
        let entity = self
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Position { x, y },
                Glyph {
                    ch: species.glyph,
                    color: species.color,
                },
                Stats {
                    hp: scale(species.base_hp, potential.hp_roll),
                    max_hp: scale(species.base_hp, potential.hp_roll),
                    atk: scale(species.base_atk, potential.atk_roll),
                    def: scale(species.base_def, potential.def_roll),
                },
                potential,
                rarity,
                Hostile,
                WanderAi::default(),
                ZonePortal(zone),
                StatusEffects::default(),
                Routines(routines),
            ))
            .id();
        // Kept out of the spawn tuple: it is already near bevy's arity
        // limit, and a conditional insert is what the load path does for
        // `Nemesis`.
        if boss {
            self.world.entity_mut(entity).insert(Boss);
        }
        Some(entity)
    }

    /// Spawns a program of `species_id` at `(x, y)` already tamed and
    /// already under the player — the shape a program takes when it joins
    /// the roster without being beaten in a fight.
    ///
    /// Two callers, and they exist for opposite reasons: `adopt_orphan`
    /// (`game/stack_features.rs`) takes something abandoned in a Stack dead
    /// end, and `grant_nest_cache` (`game/zone.rs`) takes what is left
    /// running in the wreckage of a nest. They disagree only about where
    /// the program was, what it costs and whether it happens at all — every
    /// step of *becoming* a companion is here, so a third route in cannot
    /// quietly skip one. `install_innate_routines` in particular went
    /// missing from exactly this kind of duplicate once.
    ///
    /// What is deliberately absent is as load-bearing as what is here, and
    /// `adopt_orphan`'s doc comment is where the reasoning lives: no
    /// `StackSpawn` tag (a companion that never fought would be despawned by
    /// `end_battle`), no XP, and no `Party` push — the roster is the
    /// destination, and which programs are fielded is a later choice.
    ///
    /// Neither caller checks `pet_capacity` here; both decide for themselves
    /// what a full roster means, because one refuses the action outright and
    /// the other has already destroyed the thing that was paying.
    /// The components a program gains by joining the roster, and **the only
    /// barrier the four doors have.**
    ///
    /// A program becomes a companion at four sites: `grant_starting_program`,
    /// a successful capture in `combat_rewards`, `adopt_program` here, and
    /// `fuse_companions`, which despawns both parents and assembles its own
    /// component list rather than going through any of the others. Nothing
    /// about `world.spawn` or `.insert` fails to compile when a component is
    /// missing from one of four hand-written tuples, so a shared constructor
    /// is the only thing available — the same role `work_node_parts()` plays
    /// for test fixtures, and the same failure if it is skipped: a fused
    /// companion silently unable to cast reads as fusion producing a bad
    /// program, not as a missing component.
    ///
    /// Reserves are full on arrival at every door.
    pub(crate) fn roster_parts(&self) -> (Tamed, Experience, PowerReserve) {
        (
            Tamed {
                owner: self.player_entity(),
            },
            Experience::default(),
            PowerReserve::default(),
        )
    }

    pub(crate) fn adopt_program(
        &mut self,
        species_id: &str,
        x: i32,
        y: i32,
        stat_mult: f32,
    ) -> Option<Entity> {
        let program = self.spawn_wild_creature_scaled(species_id, x, y, stat_mult, false)?;
        self.world
            .entity_mut(program)
            .remove::<(Hostile, WanderAi)>();
        let parts = self.roster_parts();
        self.world.entity_mut(program).insert(parts);
        self.install_innate_routines(program);
        Some(program)
    }

    /// Spawns a `Nest` for `species_id` at `(x, y)`, plus an initial
    /// `NEST_GUARDIAN_MIN..=NEST_GUARDIAN_MAX` guardians clustered within
    /// `NEST_TETHER_RADIUS` of it. Returns the nest's `Entity`, which
    /// provocation and destruction (`Game::attack_nest`,
    /// `Game::despawn_nest`) both need to act on this specific nest rather
    /// than requerying the map for it.
    ///
    /// `Game::load` (`lifecycle.rs`) spawns a restored nest's entity
    /// directly rather than calling this — it must not roll fresh guardians
    /// or spend `GameRng` when it's reconstructing an exact recorded state
    /// — but both it and the `spawn_bare_nest` test fixture build the same
    /// bundle through `nest_components` below, so the three can't drift the
    /// way `spawn_bare_nest` once did (it hardcoded `GlyphColor::Red` while
    /// a real scrapper nest is `Yellow`). Widen `nest_components`, not any
    /// one of its three callers, if the bundle ever needs a new component.
    pub(crate) fn spawn_nest(&mut self, species_id: &str, x: i32, y: i32) -> Entity {
        let species = self
            .world
            .resource::<SpeciesDb>()
            .get(species_id)
            .cloned()
            // Every real caller has already resolved species_id through
            // SpeciesDb before deciding to nest it — the `can_nest` check
            // in `try_spawn_habitat_creature`, or a test naming a shipped
            // species — so unlike `spawn_wild_creature`'s `Option`, an
            // unknown id here is a caller bug, not a runtime condition to
            // absorb quietly.
            .unwrap_or_else(|| panic!("spawn_nest: unknown species {species_id}"));
        let nest = self
            .world
            .spawn(nest_components(&species, x, y, NEST_DURABILITY, Vec::new()))
            .id();
        let guardian_count = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(NEST_GUARDIAN_MIN..=NEST_GUARDIAN_MAX)
        };
        for _ in 0..guardian_count {
            self.spawn_nest_guardian(nest, species_id, x, y);
        }
        nest
    }

    /// Spawns one `species_id` wild creature tethered to `nest`, at a
    /// random offset within `NEST_TETHER_RADIUS` of `(nest_x, nest_y)` —
    /// used both for a nest's initial guardians (`spawn_nest`) and for
    /// respawns (`nest_respawn_tick`, which needs the returned entity to
    /// mark a guardian `Pursuing` when it respawns at a besieged nest).
    /// The offset goes through `scatter_open_tile`, so a guardian is never
    /// placed on ground its own tether would then trap it on. `None` only
    /// if `species_id` isn't in `SpeciesDb`, mirroring
    /// `spawn_wild_creature`'s own defensive `Option`.
    pub(crate) fn spawn_nest_guardian(
        &mut self,
        nest: Entity,
        species_id: &str,
        nest_x: i32,
        nest_y: i32,
    ) -> Option<Entity> {
        let (gx, gy) = self.scatter_open_tile(nest_x, nest_y, NEST_TETHER_RADIUS);
        let guardian = self.spawn_wild_creature(species_id, gx, gy)?;
        self.world
            .entity_mut(guardian)
            .insert(NestGuardian { nest });
        Some(guardian)
    }

    /// Stat multiplier for a wild spawn at `(x, y)`, from how far it is
    /// (Chebyshev distance — matching 8-directional movement, so it's
    /// Chebyshev distance from `(x, y)` to the edge of safe territory: the
    /// platform's edge once a Home exists, the bare `ZoneSpawnPoint` before
    /// then. Measured from there rather than straight from the spawn point,
    /// so the whole base counts as distance zero instead of sitting
    /// part-way out of the ring.
    ///
    /// `Game::in_opening_ring` is the only consumer. Distance decides
    /// nothing else: it used to scale stats and group size, and a program's
    /// strength is now a property of its zone and, underground, its depth.
    pub(crate) fn distance_from_danger_origin(&self, x: i32, y: i32) -> i32 {
        let spawn = self.world.resource::<ZoneSpawnPoint>();
        let dist = (x - spawn.x).abs().max((y - spawn.y).abs());
        let platform = self.world.resource::<Platform>();
        if platform.center.is_some() {
            (dist - platform.radius).max(0)
        } else {
            dist
        }
    }

    /// Rolls a fresh `Potential` for a newly created creature — see
    /// `spawn_wild_creature`/`fuse_companions`. Each of the four fields is
    /// independently uniform in `MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL`
    /// — the "same species, different stats" mechanic.
    pub(crate) fn roll_potential(&mut self) -> Potential {
        let mut rng = self.world.resource_mut::<GameRng>();
        Potential {
            hp_roll: rng
                .0
                .random_range(MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL),
            atk_roll: rng
                .0
                .random_range(MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL),
            def_roll: rng
                .0
                .random_range(MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL),
            growth_roll: rng
                .0
                .random_range(MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL),
        }
    }

    /// Rolls the rare-spawn tier for a creature about to be created at
    /// `(x, y)` — see `components::Rarity`, which is where the "record of a
    /// multiplier already spent" rule lives.
    ///
    /// Two spawns are ineligible and **return without touching `GameRng`**,
    /// which is the load-bearing half of this function. A boss is excluded
    /// because its stats are hand-authored per `.ron` (see
    /// `assets/species/README.md`) and a multiplier discards that authoring;
    /// the opening ring is excluded because
    /// `balance_sim::beatable_by_a_fresh_player` guarantees a fresh player
    /// can beat one program there, computed against `MAX_INDIVIDUAL_ROLL`,
    /// and a gold spawn falsifies it.
    ///
    /// Gating *before* the draw is deliberate and is the mirror image of the
    /// density gate in `maybe_spawn_wild_creature`, which rolls first so a
    /// miss leaves the stream untouched. The reasoning inverts here: every
    /// zone-1 opening-ring spawn and every boss keeps its exact current RNG
    /// sequence, so the seeded tests covering those paths do not move.
    /// Eligible spawns do consume one draw, which shifts the stream for
    /// everything after them — that was a one-time, expected re-baselining.
    ///
    /// **One** roll decides the tier rather than one draw per rung, so the
    /// chances cannot sum past 1.0 and each rung is genuinely rarer than the
    /// one below instead of landing on top of it. The thresholds accumulate
    /// rarest-first for that reason, and `rarity_ladder` is the single
    /// definition of them — shared with `Game::roll_gear_rarity`, so a
    /// program and a dropped weapon cannot come up rare at different rates.
    pub(crate) fn roll_rarity(
        &mut self,
        species: &SpeciesDef,
        x: i32,
        y: i32,
        boss: bool,
    ) -> Rarity {
        // A boss, rolled or apex, never rolls a tier. An apex species'
        // stats are hand-authored and a multiplier would discard the
        // authoring; a rolled boss's `BOSS_STAT_MULT` is the whole of what
        // it is worth, and a rare tier would be a second, invisible
        // multiplier on top of it.
        if boss || species.is_boss || self.in_opening_ring(x, y) {
            return Rarity::Ordinary;
        }
        let mut rng = self.world.resource_mut::<GameRng>();
        let roll: f64 = rng.0.random_range(0.0..1.0);
        rarity_for_roll(roll)
    }

    /// The rare tier a piece of *dropped gear* rolls — see
    /// `Game::grant_gear_drop`, its only caller.
    ///
    /// Shares `rarity_for_roll` with the program ladder rather than having
    /// chances of its own, so an Overclocked weapon is exactly as rare as an
    /// Overclocked program and the word they share means one thing.
    ///
    /// Unlike `roll_rarity` this takes no position and has no exclusions.
    /// Both of that one's carve-outs are about a rare *enemy*: a boss's
    /// stats are hand-authored and a multiplier would discard the authoring,
    /// and a rare spawn inside the opening ring falsifies
    /// `balance_sim::beatable_by_a_fresh_player`. A lucky weapon is the
    /// opposite problem in both cases — it can only make a fight easier, and
    /// `balance_sim` models the player's gear without it, so the curve it
    /// gates is the unlucky floor rather than the expected case.
    pub(crate) fn roll_gear_rarity(&mut self) -> Rarity {
        let mut rng = self.world.resource_mut::<GameRng>();
        let roll: f64 = rng.0.random_range(0.0..1.0);
        rarity_for_roll(roll)
    }

    /// How many escalation steps a fight sits at — the one input both group
    /// curves take, so the two halves of the pack ceiling cannot disagree
    /// about how dangerous a place is.
    ///
    /// On the surface that is the zone; in the Stack it is `depth`. Both are
    /// commitments the player made — funding a Portal, descending a link —
    /// which is the whole point: this used to be distance from the danger
    /// origin, so which direction you wandered decided how hard the game
    /// was, and a zone had no consistent difficulty of its own.
    ///
    /// Depth *replaces* the zone step underground rather than adding to it.
    /// The party's `Position` is pinned to the entrance tile they walked in
    /// through, so there is no underground tile to read, and a stack should
    /// escalate by how far down it goes rather than inheriting whatever its
    /// entrance sat at.
    ///
    /// `depth` is a parameter rather than a `stack_pos()` read for the
    /// reason `spawn_pack`'s doc records: ambient surface spawns and nest
    /// respawns keep rolling every tick while the party is underground, and
    /// anything read off the party's own locale in here would size those
    /// from the party's depth.
    fn danger_steps(&self, depth: Option<u32>) -> u32 {
        let steps = match depth {
            Some(depth) => depth.saturating_sub(1) / GROUP_SIZE_STEP_FRAMES,
            None => self
                .world
                .resource::<ZoneLevel>()
                .0
                .saturating_sub(1)
                .saturating_div(GROUP_SIZE_STEP_ZONES),
        };
        steps.min(MAX_GROUP_SIZE_STEPS)
    }

    /// Maximum size of one wild species group at `(x, y)`: capped by the
    /// zone (`zone_group_cap`), and reached by doubling every escalation
    /// step (see `danger_steps`) — solo at your base or on the first frame
    /// down, a swarm deep in the field or deep in a stack. Used to pick how
    /// many creatures a group spawn roll places together
    /// (`try_spawn_habitat_creature`), as the per-group ceiling on one fight
    /// (`gather_pack`/`group_pack`), and to size the room a spawn roll needs
    /// (`maybe_spawn_wild_creature`).
    pub(crate) fn max_group_size(&self, depth: Option<u32>) -> u32 {
        let cap = zone_group_cap(self.world.resource::<ZoneLevel>().0);
        GROUP_SIZE_DISTANCE_GROWTH
            .pow(self.danger_steps(depth))
            .min(cap)
    }

    /// How many distinct species groups one fight at `(x, y)` may hold: a
    /// single group at the danger origin, gaining one more per escalation
    /// step out to `MAX_ENEMY_GROUPS`. Rides the same curve as
    /// `max_group_size`, and for the same reason — what meets you at your
    /// own doorstep is one program, and it is pushing out that turns it
    /// into a swarm.
    ///
    /// Without this the two halves of the pack ceiling disagreed near the
    /// origin: group *size* started at one there while the *number* of
    /// groups jumped straight to four. A zone-1 opening, where the zone cap
    /// pins every group to a single member anyway, was therefore a
    /// four-on-one against a player who has no companions yet —
    /// `balance_sim::simulate_roster_fight` scores that as a loss against every
    /// shipped species, including the four that `beatable_by_a_fresh_player`
    /// clears one-on-one.
    pub(crate) fn max_enemy_groups(&self, depth: Option<u32>) -> usize {
        (self.danger_steps(depth) as usize + 1).min(MAX_ENEMY_GROUPS)
    }

    /// Whether `(x, y)` is in the pocket a brand-new run opens in: zone 1,
    /// within `OPENING_RING_TILES` of the danger origin.
    ///
    /// An explicit radius, and it has to be. This used to be spelled as
    /// "both curves say a fight here is a single program", which was exact
    /// while distance drove those curves. It no longer does — zone 1 caps
    /// every group at one member *everywhere* — so the old spelling would
    /// now be true across the whole zone and quietly turn all of it into a
    /// nursery.
    ///
    /// Zone 1 only, and deliberately: past it the player has a party, and
    /// "what a bare level-1 player beats solo" would be filtering the
    /// wrong fight — a deep zone's home ring is meant to be quiet, not
    /// toothless.
    fn in_opening_ring(&self, x: i32, y: i32) -> bool {
        self.world.resource::<ZoneLevel>().0 == 1
            && self.distance_from_danger_origin(x, y) <= OPENING_RING_TILES
    }

    /// Spawns `count` wild creatures near the player, retrying with a fresh
    /// random offset whenever a roll whiffs (an unwalkable tile, or a biome
    /// with no matching species) rather than giving up on that slot — a
    /// freshly generated zone's terrain noise can otherwise leave large
    /// unwalkable or habitat-sparse patches right around the player's
    /// start point (see `find_walkable_start`, which always searches out
    /// from world origin), and a blind one-attempt-per-slot approach would
    /// leave the zone nearly empty whenever that happens. Bounded to
    /// `count * 20` attempts so a pathologically bad pocket can't loop
    /// forever instead of just spawning fewer than `count`.
    /// Stocks every chunk within `POPULATION_CHUNK_MARGIN` of the player's
    /// own that has not been stocked before, and marks it.
    ///
    /// This is where the sector's population comes from. The map is
    /// unbounded — `world::WorldMap` generates a chunk of terrain whenever
    /// something asks about a tile in it — so there is no finite area a
    /// one-time seed could cover, and a per-tick roll near the player cannot
    /// fill space either: walking costs a tick a tile, so a player crossing
    /// one density box buys about a spare roll of
    /// `WILD_SPAWN_CHANCE` against a target of `WILD_LOCAL_DENSITY_TARGET`.
    /// Measured before the change: zero to two programs per box across 240
    /// tiles of walked ground (`docs/measurements/`).
    ///
    /// Deliberately *not* guarded by `Game::require_surface` or an
    /// `is_underground` check, unlike `power_regen_system` and
    /// `nest_aggro_tick`. Those refuse because they would otherwise reach the
    /// party through a `Position` pinned to the entrance tile. This one makes
    /// no claim about the party at all — it stocks ground — and the surface
    /// is meant to keep living while they are below, which is the same rule
    /// `Trace`'s group-size lever states about surface spawns continuing to
    /// roll underground.
    pub(crate) fn ensure_local_population(&mut self) {
        let pos = *self.world.get::<Position>(self.player_entity()).unwrap();
        let (px, py) = (pos.x.div_euclid(CHUNK_SIZE), pos.y.div_euclid(CHUNK_SIZE));
        for cy in (py - POPULATION_CHUNK_MARGIN)..=(py + POPULATION_CHUNK_MARGIN) {
            for cx in (px - POPULATION_CHUNK_MARGIN)..=(px + POPULATION_CHUNK_MARGIN) {
                if !self
                    .world
                    .resource_mut::<PopulatedChunks>()
                    .0
                    .insert((cx, cy))
                {
                    continue;
                }
                self.populate_chunk(cx, cy);
            }
        }
    }

    /// Places one chunk's worth of wild programs inside chunk `(cx, cy)`.
    ///
    /// The mark is written by the caller *before* this runs, so a chunk that
    /// turns out to hold nothing placeable — open water, a biome with no
    /// habitat species, ground already crowded by a neighbour — is still
    /// marked and is not retried every tick for the rest of the run.
    ///
    /// Draws from `resources::GameRng` rather than seeding a local `StdRng`
    /// off the chunk coordinates, which is the opposite of what
    /// `stack::generate` does and is deliberate. A frame must regenerate
    /// identically because the party has *seen* it; a chunk's population is
    /// explicitly not reproducible — `Game::cull_to_cap` evicts and forgets
    /// whole chunks, so walking back to one is meant to find different
    /// programs. Pinning it to the place would be a promise the eviction
    /// breaks on purpose, and it keeps `try_spawn_habitat_creature` — which
    /// owns species, rarity, the opening ring and boss substitution — free
    /// of a threaded seed.
    fn populate_chunk(&mut self, cx: i32, cy: i32) {
        let target = chunk_wild_population();
        self.cull_to_cap(target * self.max_group_size(None) as usize);
        let (ox, oy) = (cx * CHUNK_SIZE, cy * CHUNK_SIZE);
        let mut placed = 0;
        let mut attempts = 0;
        while placed < target && attempts < target * 20 {
            attempts += 1;
            let (dx, dy) = {
                let mut rng = self.world.resource_mut::<GameRng>();
                (
                    rng.0.random_range(0..CHUNK_SIZE),
                    rng.0.random_range(0..CHUNK_SIZE),
                )
            };
            let (x, y) = (ox + dx, oy + dy);
            // The same gate the ambient roll applies, so a chunk cannot be
            // stocked past the density the roll would then maintain — and so
            // a chunk placed beside an already-full one fills only the part
            // of itself that is actually empty.
            if self.local_hostile_count(x, y) < WILD_LOCAL_DENSITY_TARGET
                && self.try_spawn_habitat_creature(x, y)
            {
                placed += 1;
            }
        }
    }

    /// Makes room for `needed` more `Hostile`s under `WILD_CREATURE_CAP` by
    /// evicting whole chunks, farthest from the player first.
    ///
    /// Whole chunks rather than individual creatures because a chunk is the
    /// unit population is placed in: evicting one creature from a chunk would
    /// leave the chunk marked as stocked while thinning it, and clearing the
    /// mark for one creature would leave a chunk marked-unstocked while it
    /// still held most of its programs. Evicting the chunk and dropping its
    /// mark together keeps the mark meaning exactly what it says, and keeps
    /// `PopulatedChunks` bounded by the cap rather than by how far the player
    /// has ever walked.
    ///
    /// Candidates come from **where the hostiles actually stand**, not from
    /// `PopulatedChunks`. Culling the marked set instead reads better and is
    /// wrong: a program that wanders across a chunk boundary into ground
    /// nothing stocked would be immune to eviction forever, so the leak the
    /// cap exists to stop would reopen slowly through the wander AI. The mark
    /// is cleared as a *consequence* of evicting a chunk, which is why it may
    /// be absent.
    ///
    /// The player's own neighbourhood is never a candidate — evicting the
    /// ground under their feet is visible, and is the one place the cap must
    /// not reach. `NestGuardian`s are eligible like any other hostile; an
    /// eviction is a plain despawn, so it deliberately doesn't feed the
    /// nest's `pending_respawns` the way an actual defeat does. Guardian
    /// counts are best-effort once a nest is far behind the player.
    pub(crate) fn cull_to_cap(&mut self, needed: usize) {
        let pos = *self.world.get::<Position>(self.player_entity()).unwrap();
        let (px, py) = (pos.x.div_euclid(CHUNK_SIZE), pos.y.div_euclid(CHUNK_SIZE));
        loop {
            let hostiles: Vec<(Entity, (i32, i32))> = {
                let mut query = self
                    .world
                    .query_filtered::<(Entity, &Position), With<Hostile>>();
                query
                    .iter(&self.world)
                    .map(|(e, p)| (e, (p.x.div_euclid(CHUNK_SIZE), p.y.div_euclid(CHUNK_SIZE))))
                    .collect()
            };
            if hostiles.len() + needed <= WILD_CREATURE_CAP {
                return;
            }
            // `max_by_key` over a *total* order, not distance alone: bevy's
            // query iteration order is not stable, so two chunks equally far
            // out would evict differently between runs and a save would stop
            // being reproducible. Same reasoning as `assembler_system`'s sort
            // and `drawn_on_surface_map`'s tie-break.
            let victim = hostiles
                .iter()
                .map(|&(_, chunk)| chunk)
                .filter(|&(cx, cy)| (cx - px).abs().max((cy - py).abs()) > POPULATION_CHUNK_MARGIN)
                .max_by_key(|&(cx, cy)| ((cx - px).abs().max((cy - py).abs()), cx, cy));
            // Everything left is under the player's feet. Better an over-full
            // map than an infinite loop; the cap is a bound on simulation
            // cost, not a rule the game owes the player.
            let Some(victim) = victim else {
                return;
            };
            self.world
                .resource_mut::<PopulatedChunks>()
                .0
                .remove(&victim);
            for (entity, chunk) in hostiles {
                if chunk == victim {
                    self.world.despawn(entity);
                }
            }
        }
    }

    /// How many `Hostile`s stand within `WILD_SPAWN_RADIUS_TILES` (Chebyshev,
    /// matching 8-directional movement) of `(x, y)` — the one definition of
    /// "how crowded is it here", read by the ambient spawn roll and by zone
    /// seeding so the density a zone is born at is the density it keeps.
    ///
    /// Deliberately the same radius the roll places into: a target measured
    /// over a different box than the one being filled would steer toward a
    /// density that never appears on screen.
    ///
    /// Tamed programs are not counted, matching `WILD_CREATURE_CAP` — a full
    /// roster should not starve the map of things to fight. Nest guardians
    /// are `Hostile` and so do count, which is right: they are a real part of
    /// why the ground around a besieged nest is crowded.
    pub(crate) fn local_hostile_count(&mut self, x: i32, y: i32) -> usize {
        let mut query = self.world.query_filtered::<&Position, With<Hostile>>();
        query
            .iter(&self.world)
            .filter(|p| (p.x - x).abs().max((p.y - y).abs()) <= WILD_SPAWN_RADIUS_TILES)
            .count()
    }

    pub(crate) fn maybe_spawn_wild_creature(&mut self) {
        let damp_pct = self.field_buff_power(self.player_entity(), FieldBuffKind::EncounterDamp);
        // Roll first: culling is wasted work if nothing was going to spawn.
        let roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(damped_wild_spawn_chance(damp_pct))
        };
        if !roll {
            return;
        }
        // The density gate sits here rather than inside `spawn_wild_nearby`
        // because it paces an *ambient* spawn; it is not part of what a spawn
        // is. `dev_force_encounter` shares that body and must still place a
        // fight when the player is already standing in a crowd — which is
        // exactly when a tester reaches for it.
        //
        // After the roll, not before, for the reason the comment above gives:
        // the scan is wasted work on the 95% of ticks that spawn nothing, and
        // rolling first leaves the RNG sequence the seeded spawn tests depend
        // on untouched on a miss.
        let pos = *self.world.get::<Position>(self.player_entity()).unwrap();
        if self.local_hostile_count(pos.x, pos.y) >= WILD_LOCAL_DENSITY_TARGET {
            return;
        }
        self.spawn_wild_nearby();
    }

    /// Places a wild spawn near the player now, skipping the roll — the dev
    /// console's encounter trigger.
    ///
    /// Calls the same body the ambient spawn does, so the console cannot
    /// disagree with the game about habitat pools, the opening ring or the
    /// cull. Reachable only through the `FERAL_DEV_CONSOLE` gate.
    #[doc(hidden)]
    pub fn dev_force_encounter(&mut self) {
        self.spawn_wild_nearby();
    }

    /// Everything a wild spawn *is*, once it has been decided one happens.
    ///
    /// `player_pos` is read here rather than before the roll so the split
    /// costs no RNG draw: the roll does not use it, and the draw order after
    /// it is untouched.
    fn spawn_wild_nearby(&mut self) {
        let player_pos = *self.world.get::<Position>(self.player_entity()).unwrap();
        let (dx, dy) = {
            let mut rng = self.world.resource_mut::<GameRng>();
            (
                rng.0
                    .random_range(-WILD_SPAWN_RADIUS_TILES..=WILD_SPAWN_RADIUS_TILES),
                rng.0
                    .random_range(-WILD_SPAWN_RADIUS_TILES..=WILD_SPAWN_RADIUS_TILES),
            )
        };
        let (tx, ty) = (player_pos.x + dx, player_pos.y + dy);
        self.cull_to_cap(self.max_group_size(None) as usize);
        self.try_spawn_habitat_creature(tx, ty);
    }

    /// Picks which species a spawn at `(x, y)` should field: the tile
    /// biome's habitat matches, gentled by the opening ring, with a rare
    /// boss substituted when `allow_boss` and the biome defines one.
    /// Returns the species id and whether it is a boss, or `None` for an
    /// unwalkable tile or a biome with nothing eligible.
    ///
    /// Split out of `try_spawn_habitat_creature` so `Game::maybe_ambush`
    /// can reuse the biome and opening-ring rules without inheriting the
    /// boss and nest substitutions — copying them instead is exactly the
    /// duplicated-formula trap this repo keeps falling into.
    ///
    /// The habitat caller's RNG draw order is unchanged by the split:
    /// `allow_boss` short-circuits ahead of a boss roll that was already
    /// conditional on the biome having bosses at all, so only the new
    /// caller skips a draw.
    pub(crate) fn pick_habitat_species(
        &mut self,
        x: i32,
        y: i32,
        allow_boss: bool,
    ) -> Option<(String, bool)> {
        let (candidates, boss_candidates) = self.habitat_pools(x, y)?;
        // A boss takes the tile's one spawn slot instead of an ordinary
        // habitat creature, but only rarely, and only where one is defined
        // for this biome at all.
        let spawn_boss = allow_boss && !boss_candidates.is_empty() && {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(BOSS_SPAWN_CHANCE)
        };
        let pool = if spawn_boss || candidates.is_empty() {
            if !allow_boss {
                // Only reachable with an empty ordinary pool: a biome whose
                // sole residents are bosses has nothing an ambush may field.
                return None;
            }
            &boss_candidates
        } else {
            &candidates
        };
        let pick = {
            let mut rng = self.world.resource_mut::<GameRng>();
            let idx = rng.0.random_range(0..pool.len());
            pool[idx].clone()
        };
        Some((pick, spawn_boss))
    }

    /// The ordinary and boss candidate pools for a spawn at `(x, y)`, after
    /// the opening-ring gentling and before any draw. `None` for an
    /// unwalkable tile or a biome with nothing eligible.
    ///
    /// Split out so the draw itself belongs to the caller.
    /// `pick_habitat_species` spends `GameRng`; `orphan_species` spends a
    /// frame-seeded `StdRng`, because the party has to be able to see what
    /// an orphan is before paying for it and so the answer has to survive a
    /// save/load. Copying the biome and opening-ring rules into the second
    /// caller instead is exactly the duplicated-formula trap this repo keeps
    /// falling into.
    ///
    /// `allow_boss` is deliberately not a parameter: both places it is
    /// consulted sit *after* the pools are built, so it stays with the draw.
    /// That is also why this split changes `pick_habitat_species`'s RNG draw
    /// order not at all, which the seeded spawn tests depend on.
    pub(crate) fn habitat_pools(&mut self, x: i32, y: i32) -> Option<(Vec<String>, Vec<String>)> {
        let tile = self.world.resource_mut::<WorldMap>().tile(x, y);
        if !tile.walkable {
            return None;
        }
        let species_db = self.world.resource::<SpeciesDb>();
        let mut candidates: Vec<String> = species_db
            .habitat_matches(tile.biome)
            .into_iter()
            .map(|s| s.id.clone())
            .collect();
        let mut boss_candidates: Vec<String> = species_db
            .boss_habitat_matches(tile.biome)
            .into_iter()
            .map(|s| s.id.clone())
            .collect();
        if candidates.is_empty() && boss_candidates.is_empty() {
            return None;
        }
        // The opening ring fields only what a fresh player can actually
        // beat — bosses emphatically included in what it turns away.
        //
        // Not every biome has something that qualifies (no shipped
        // StaticField species does), and there the ring falls back to the
        // gentlest thing that biome has rather than to its whole roster:
        // still a hard opening, but never the worst one on offer. Ranked
        // by flat stat total, the same crude yardstick
        // `balance_sim::toughest_ordinary_species` sorts by, because the
        // projection itself can't rank fights the player loses — they all
        // score zero HP left.
        if self.in_opening_ring(x, y) {
            let db = self.world.resource::<SpeciesDb>();
            let gentle: Vec<String> = candidates
                .iter()
                .filter(|id| {
                    db.get(id)
                        .is_some_and(crate::balance_sim::beatable_by_a_fresh_player)
                })
                .cloned()
                .collect();
            candidates = if gentle.is_empty() {
                candidates
                    .iter()
                    .min_by_key(|id| {
                        db.get(id)
                            .map(|s| s.base_hp + s.base_atk + s.base_def)
                            .unwrap_or(i32::MAX)
                    })
                    .cloned()
                    .into_iter()
                    .collect()
            } else {
                gentle
            };
            boss_candidates.clear();
            // A biome that offers nothing but bosses spawns nothing here,
            // rather than drawing from a pool the ring just emptied.
            if candidates.is_empty() {
                return None;
            }
        }
        Some((candidates, boss_candidates))
    }

    /// Places a group of `species_id` around `(x, y)` and returns whatever
    /// actually spawned. Bosses always spawn alone — packs are an
    /// ordinary-encounter mechanic, not something to stack onto an already
    /// tough boss fight.
    ///
    /// `esc` carries everything about the fight that its own tile cannot
    /// say — see `SpawnEscalation`, which documents why all three of its
    /// fields are handed in rather than read off the world here. Surface
    /// callers pass `SpawnEscalation::surface()`.
    pub(crate) fn spawn_pack(
        &mut self,
        species_id: &str,
        is_boss: bool,
        x: i32,
        y: i32,
        esc: SpawnEscalation,
    ) -> Vec<Entity> {
        if !is_boss {
            let size = self.roll_group_size(esc);
            return self.spawn_group(species_id, size, x, y, esc, false);
        }
        let mut spawned = self.spawn_group(species_id, 1, x, y, esc, true);
        // A boss is one group; an escort needs a second one to stand in.
        // Zone 1 has no room for it, which is deliberate — the opening
        // zone's boss is the one fight where "a single very large program"
        // is still the whole encounter.
        if self.max_enemy_groups(esc.depth) >= 2
            && let Some(escort) = self.pick_escort_species(x, y)
        {
            let size = self.roll_group_size(esc);
            let escort_pack = self.spawn_group(&escort, size, x, y, esc, false);
            spawned.extend(escort_pack);
        }
        spawned
    }

    /// How many members one ordinary group rolls: uniform in `1..=ceiling`,
    /// so raising the ceiling widens the range of fights a zone produces
    /// rather than making every fight bigger.
    fn roll_group_size(&mut self, esc: SpawnEscalation) -> u32 {
        let cap = zone_group_cap(self.world.resource::<ZoneLevel>().0);
        let max_group = trace_group_ceiling(self.max_group_size(esc.depth), esc.group_mult, cap);
        let mut rng = self.world.resource_mut::<GameRng>();
        rng.0.random_range(1..=max_group)
    }

    /// An ordinary program from `(x, y)`'s own habitat, to stand beside a
    /// boss. `None` where the biome offers nothing but the boss itself.
    ///
    /// Drawn from `habitat_pools` rather than a pool of its own so the
    /// escort obeys every rule an ordinary spawn does — including the
    /// opening ring, though a boss cannot presently reach one. Both boss
    /// sites hand `spawn_pack` the tile whose biome chose the boss (the
    /// surface roll its own, `rouse_lair` the Stack entrance it read), so
    /// this is the right pool for either.
    fn pick_escort_species(&mut self, x: i32, y: i32) -> Option<String> {
        let (candidates, _) = self.habitat_pools(x, y)?;
        if candidates.is_empty() {
            return None;
        }
        let mut rng = self.world.resource_mut::<GameRng>();
        let idx = rng.0.random_range(0..candidates.len());
        Some(candidates[idx].clone())
    }

    /// A tile within `radius` of `(x, y)` that a hostile can both stand on
    /// and step off — the single scatter rule for every creature placed at
    /// an offset from a validated anchor.
    ///
    /// Placement and movement used to disagree. Only the *anchor* of a
    /// spawn roll was ever checked (`habitat_pools`, and `walkable` at
    /// that), while `wander_ai_system` and `pursuit_field` both step only
    /// onto `Tile::open_to_hostiles`. A pack member or nest guardian
    /// scattered across a biome boundary onto DataVoid, or onto the base
    /// slab, therefore had no legal move for the rest of the run — and a
    /// guardian's tether meant nothing could displace it off, either.
    ///
    /// It spends exactly one draw per axis whatever the outcome, rather
    /// than resampling until a tile passes: the shared `GameRng` stream is
    /// what every seeded spawn test is written against, and a variable
    /// number of draws would move all of them for a reason unrelated to
    /// what they assert. The anchor is the fallback because it is the one
    /// tile already known to be legal.
    fn scatter_open_tile(&mut self, x: i32, y: i32, radius: i32) -> (i32, i32) {
        let (ox, oy) = {
            let mut rng = self.world.resource_mut::<GameRng>();
            (
                x + rng.0.random_range(-radius..=radius),
                y + rng.0.random_range(-radius..=radius),
            )
        };
        if self
            .world
            .resource_mut::<WorldMap>()
            .tile(ox, oy)
            .open_to_hostiles()
        {
            (ox, oy)
        } else {
            (x, y)
        }
    }

    /// Places `size` members of one species around `(x, y)`: the first on
    /// the tile itself, the rest scattered within `swarm_radius` of it.
    fn spawn_group(
        &mut self,
        species_id: &str,
        size: u32,
        x: i32,
        y: i32,
        esc: SpawnEscalation,
        boss: bool,
    ) -> Vec<Entity> {
        // Hoisted above the loop deliberately: it takes no RNG, so the
        // seeded sequence every spawn test depends on is untouched.
        let radius = swarm_radius(size);
        let mut spawned = Vec::new();
        for i in 0..size {
            // The first member anchors the roll's own tile; the rest
            // cluster around it, on ground they can actually leave.
            let (gx, gy) = if i == 0 {
                (x, y)
            } else {
                self.scatter_open_tile(x, y, radius)
            };
            spawned.extend(self.spawn_wild_creature_scaled(
                species_id,
                gx,
                gy,
                esc.stat_mult,
                boss,
            ));
        }
        spawned
    }

    /// Attempts to spawn one habitat-appropriate wild creature (or, away
    /// from the zone's spawn point, a small pack of the same species — see
    /// `max_group_size`) at `(x, y)`, returning whether it actually spawned
    /// anything — `false` on an unwalkable tile or a biome with no
    /// matching species, so callers (see `spawn_initial_creatures`) can
    /// retry elsewhere instead of silently losing that spawn slot.
    pub(crate) fn try_spawn_habitat_creature(&mut self, x: i32, y: i32) -> bool {
        let Some((pick, spawn_boss)) = self.pick_habitat_species(x, y, true) else {
            return false;
        };

        // A nest takes the tile's spawn slot instead of an ordinary pack,
        // same "rare special outcome" shape as the boss roll above — but
        // only ever considered for the non-boss pick, and only for a
        // species that opted in via `SpeciesDef::can_nest`. The RNG draw
        // only happens when `can_nest` is true, so this never shifts the
        // RNG sequence for the (overwhelmingly common) non-nesting case.
        if !spawn_boss {
            let can_nest = self
                .world
                .resource::<SpeciesDb>()
                .get(&pick)
                .is_some_and(|s| s.can_nest);
            let spawn_nest_roll = can_nest && {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_bool(NEST_SPAWN_CHANCE)
            };
            if spawn_nest_roll {
                self.spawn_nest(&pick, x, y);
                return true;
            }
        }

        self.spawn_pack(&pick, spawn_boss, x, y, SpawnEscalation::surface());
        true
    }
}

/// The component bundle every nest entity carries — `Nest`, `Position`,
/// `Glyph`, `Durability` — assembled once so `spawn_nest`, `Game::load`
/// (`lifecycle.rs`) and the `spawn_bare_nest` test fixture
/// (`tests/support.rs`) can't drift the way they already had: `hp` and
/// `pending_respawns` come in as plain values rather than always-fresh
/// defaults because a restored nest is neither full-health nor
/// respawn-queue-empty, and `species` comes in as a resolved `SpeciesDef`
/// reference rather than an id so the load path doesn't need `GameRng` on
/// hand to build the same bundle `spawn_nest` does while actually rolling
/// fresh guardians.
pub(crate) fn nest_components(
    species: &SpeciesDef,
    x: i32,
    y: i32,
    hp: u32,
    pending_respawns: Vec<u32>,
) -> (Nest, Position, Glyph, Durability) {
    (
        Nest {
            species: species.id.clone(),
            pending_respawns,
        },
        Position { x, y },
        Glyph {
            ch: 'N',
            color: species.color,
        },
        Durability {
            hp,
            max_hp: NEST_DURABILITY,
        },
    )
}
