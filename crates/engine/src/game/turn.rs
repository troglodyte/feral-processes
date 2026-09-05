//! The turn loop: advancing the clock, moving, and the actions a player
//! spends a turn on.

use crate::game::pursuit::pursuit_field;
use crate::resources::SeenConditions;
use crate::telemetry::Record;
use crate::tuning::{
    NEST_AGGRO_LEASH_RADIUS, NEST_PATH_SEARCH_MARGIN, NEST_PURSUIT_STEPS_PER_TICK,
    RANDOM_ENCOUNTER_CHANCE, REST_AMBUSH_CHANCE,
};
use crate::world::NEIGHBOURS;
use crate::*;

/// Chebyshev distance between two map tiles — the metric every pursuit and
/// leash check in this file uses, matching the 8-directional movement
/// `pursuit_field` routes over.
fn chebyshev(a: Position, b: Position) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

impl Game {
    /// The player's own `Entity`. `pub` because `equip`/`unequip` name the
    /// wearer, and the inventory screen's wearer is the player — app-core
    /// has to be able to say so. An `Entity` is inert outside `Game`:
    /// `world` stays private, so handing one out grants no reach into the
    /// ECS, only the right to name a wearer.
    pub fn player_entity(&self) -> Entity {
        self.world.resource::<PlayerEntity>().0
    }

    pub(crate) fn log(&mut self, s: impl Into<String>) {
        self.world.resource_mut::<MessageLog>().push(s);
        self.snapshot_roster();
    }

    pub(crate) fn log_kind(&mut self, kind: MessageKind, s: impl Into<String>) {
        self.world.resource_mut::<MessageLog>().push_kind(kind, s);
        self.snapshot_roster();
    }

    /// Narrates a resolved swing, the way `log_kind` narrates everything
    /// else — but tagged with the band `outcome` landed on, which is what
    /// lets a frontend's reveal fire a per-swing sound cue instead of one
    /// blip for the whole round. The only `log_*` variant that takes an
    /// `AttackOutcome`; every call site already has one in hand from
    /// `resolve_and_apply_attack`.
    pub(crate) fn log_swing(
        &mut self,
        kind: MessageKind,
        outcome: battle::AttackOutcome,
        s: impl Into<String>,
    ) {
        self.world
            .resource_mut::<MessageLog>()
            .push_swing(kind, outcome.into(), s);
        self.snapshot_roster();
    }

    /// News from the base — production, construction, and the base coming
    /// under attack. `log` stays the default because most of the game is not
    /// the base; see `MessageSource`.
    pub(crate) fn log_base(&mut self, s: impl Into<String>) {
        self.world.resource_mut::<MessageLog>().push_base(s);
    }

    pub(crate) fn log_base_kind(&mut self, kind: MessageKind, s: impl Into<String>) {
        self.world
            .resource_mut::<MessageLog>()
            .push_base_kind(kind, s);
    }

    /// Records why an action the player asked for was refused. The one
    /// public door onto the log: `App::refuse` sets `App::status_line` and
    /// calls this, so the banner the player reads and the history they can
    /// scroll back to cannot say different things.
    ///
    /// **Silent while a battle is open, and that is the load-bearing half.**
    /// `MessageLog::since_round` slices the battle pane by *position* and
    /// `App::advance_reveal` paces it by counting *raw* lines, so a line
    /// pushed from a battle submenu would land inside the round's range —
    /// drawn mid-narration with no round header to explain it, and
    /// swallowing one keypress' worth of reveal on the way past. The refusal
    /// still reaches the player: it is on the popup they typed into.
    pub fn note_refusal(&mut self, s: impl Into<String>) {
        if self.has_active_battle() {
            return;
        }
        self.log_kind(MessageKind::Refusal, s);
    }

    pub fn message_log(&self, n: usize) -> Vec<LogLine> {
        self.world.resource::<MessageLog>().recent(n).to_vec()
    }

    /// The last `n` lines with repeats folded together — what the history
    /// screen shows, and what anything scrolling that screen has to count
    /// rows with. See `resources::condense` for the fold and why it is a view
    /// rather than a collapse in storage.
    pub fn message_history(&self, n: usize) -> Vec<LogEntry> {
        crate::resources::condense(self.world.resource::<MessageLog>().recent(n))
    }

    /// The current round's narration, oldest first — what the battle pane
    /// shows. One round at a time, so a resolved round replaces the last
    /// rather than piling on top of it, and a fresh fight opens on an empty
    /// pane rather than the tail of the previous one. Once the battle has
    /// ended this is the pruned result set, which is what a frontend still
    /// mid-reveal is scrolling in.
    pub fn battle_log(&self) -> Vec<LogLine> {
        self.world.resource::<MessageLog>().since_round().to_vec()
    }

    /// Changes every time the pane's contents reset — a new round, or a new
    /// battle. A frontend pacing the narration restarts when this moves.
    pub fn battle_log_generation(&self) -> u64 {
        self.world.resource::<MessageLog>().generation()
    }

    pub fn is_game_over(&self) -> Option<String> {
        self.world.resource::<GameOver>().reason.clone()
    }

    /// How many ticks (see `tick`) have elapsed this session. Exposed so a
    /// caller (e.g. an autosave timer) can pace itself against game time
    /// rather than wall-clock time or its own separate counter.
    pub fn current_tick(&self) -> u64 {
        self.world.resource::<GameClock>().tick
    }

    pub fn has_active_battle(&self) -> bool {
        self.world.get_resource::<BattleState>().is_some()
    }

    /// Advances the world clock with no player action behind it — the hook
    /// a frontend's real-time loop calls once a second so the world keeps
    /// moving while the player is idle. A no-op during battle (turns there
    /// are paced by battle actions, not the wall clock) or after game over.
    pub fn idle_tick(&mut self) {
        if self.has_active_battle() {
            return;
        }
        self.tick();
    }

    pub(crate) fn tick(&mut self) {
        self.tick_inner(true);
    }

    /// Shared implementation behind `tick`. `age_temporary` controls
    /// whether this tick counts toward any `Temporary` structure's
    /// remaining lifespan (see `age_temporary_structures`) — `rest`'s
    /// internal loop passes `false` so resting at a structure doesn't burn
    /// down its lifespan any faster than leaving it standing idle would.
    pub(crate) fn tick_inner(&mut self, age_temporary: bool) {
        if self.is_game_over().is_some() {
            return;
        }
        // Captured here, at the one place the clock actually advances,
        // rather than at each of the thirty-odd call sites that spend a
        // tick — a verb-by-verb list of "and also announce a boundary" is
        // exactly how `idle_tick` and the crafting/building/trading paths
        // went silent on this in the first place. A single tick can cross
        // at most one epoch boundary (the clock only ever moves by one), so
        // this fires at most once per call regardless of how many verbs
        // share this function.
        let epoch_before = self.static_epoch();
        // Before the ambient roll, not after: the roll's density gate reads
        // `local_hostile_count`, and asking it about ground that has not been
        // stocked yet would answer "empty" and spend a spawn filling in what
        // is about to arrive properly.
        self.ensure_local_population();
        self.ensure_local_settlements();
        self.maybe_spawn_wild_creature();
        // Before the schedule, not after, so a body posted this tick makes
        // progress this tick rather than standing at its machine for one.
        // Beside `maybe_spawn_wild_creature` for the same reason that one is
        // here: both are `&mut Game` work a bevy system cannot express.
        self.schedule_base_labour();
        // Immediately after the scheduler and before the schedule, for both
        // of that call's reasons: a digger posted this tick swings this
        // tick, and a cycle that ends in `strike_rock` or `floor_cell` is
        // `&mut Game` work no bevy system can express.
        self.run_dig_crew();
        // Beside the dig crew and for its two reasons: a builder posted this
        // tick works this tick, and a cycle that ends in
        // `Game::spawn_structure` — the one place a structure's component
        // list is written — is `&mut Game` work no bevy system can express.
        self.run_build_crew();
        // Beside the two crews and for the same reason as `run_dig_crew`'s
        // second: the recovery line has to name the program through
        // `creature_label` and the heal through `restore_hp`, both of which
        // are `&mut Game` doors no bevy system can reach. See
        // `Game::run_repair_bays`.
        self.run_repair_bays();
        // Beside the two crews and the Bays, and for `run_dig_crew`'s second
        // reason: an off-screen battle names programs through
        // `creature_label`, damages through `apply_damage` and logs, none of
        // which a bevy system can reach. It sits *after* them so a squad
        // dispatched this tick does not fight before the base has finished
        // its own beat.
        self.run_sorties();
        self.schedule.run(&mut self.world);
        // Immediately after the schedule, where `haul_step_system`'s commands
        // have just flushed and the clock has not yet moved: a stranding is an
        // *edge*, and one tick later there is nothing left to read it off.
        self.note_strandings();
        // Beside `note_strandings` and for the same reason — the base
        // systems' commands have just flushed and the clock has not moved on
        // — but on a period rather than on an edge: a stranding *has* one and
        // a posting does not. See `MEMORY_POSTING_PERIOD`.
        self.note_postings();
        // Beside `note_strandings` and for its reason: `needs_tick_system`
        // has just drained the reserve inside the schedule above, and
        // `Game::notify` is a `&mut Game` door no bevy system can reach.
        self.note_low_power();
        // Immediately after the schedule, which is where `contract_system`
        // raised the progress this reads. Paying is `&mut Game` work — an
        // inventory write and an XP grant — so it cannot live in the system
        // that counts.
        self.settle_contracts();
        // After settling, so finishing a mission hands out the next one in
        // the same tick and the player never sees an empty slot.
        self.ensure_tutorial_held();
        self.structure_regen();
        // After the schedule, where the base systems' commands have just
        // flushed, so the walk reads structure positions that are settled —
        // and **before** `raid_check`, so a trader cannot be caught standing
        // in a sweep that is resolved the same tick it arrives.
        self.caravan_tick();
        self.raid_check();
        self.nest_respawn_tick();
        // Immediately after respawn: a guardian that just replaced a fallen
        // one at a besieged nest is already `Pursuing` (`nest_respawn_tick`
        // via `nest_has_pursuers`) and should get its step the same tick it
        // appeared, not wait a full tick doing nothing.
        self.nest_aggro_tick();
        if age_temporary {
            self.age_temporary_structures();
        }
        // Deliberately outside the `age_temporary` guard, unlike the call
        // just above: a `Temporary` structure does not decay while the
        // player rests, but a field buff does — rest is time passing for a
        // buff, not base upkeep the player paused by staying home.
        self.tick_field_buffs();
        // Before the clock moves, so the window a snapshot is stamped with
        // is the one whose events follow it: `base_ledger::fold` opens a
        // bucket at `tick - tick % BUCKET_TICKS`, and a snapshot taken after
        // the increment would describe the base one tick into the window it
        // is meant to head.
        self.note_base_snapshot();
        self.world.resource_mut::<GameClock>().tick += 1;
        self.note_static_turnover(epoch_before);
    }

    /// Tells the player once, ever, that their Power reserve has gone under
    /// `tuning::LOW_POWER_ATTACK_THRESHOLD`.
    ///
    /// **A state read once a tick rather than a hook on a spend**, which is
    /// the opposite of how every other tutorial fires — and it has to be.
    /// Power leaves the player two ways: `Game::spend_power` charges a
    /// routine, and `needs_tick_system` drains a flat rate every tick
    /// whatever the player is doing. The second is how most runs cross the
    /// line, and it is a bevy system with no `Game` to notify from. One read
    /// after the schedule covers both, and `Repeat::OnceEver` is what keeps
    /// it from being a per-tick alarm — the reserve sits under the threshold
    /// for as long as the player leaves it there.
    fn note_low_power(&mut self) {
        let player = self.player_entity();
        let low = self
            .world
            .get::<PowerReserve>(player)
            .is_some_and(|r| r.get() < crate::tuning::LOW_POWER_ATTACK_THRESHOLD);
        if low {
            self.notify(crate::notifications::NotificationKind::LowPower);
        }
    }

    /// Moves every provoked nest guardian one step closer to the player and
    /// starts a battle for the first one that arrives — the per-tick half of
    /// nest aggression, called from `tick_inner` right after
    /// `nest_respawn_tick`. A no-op during battle, after game over, or while
    /// the party is underground: `Position` stays pinned to the surface
    /// entrance tile the whole time they're down there (see the
    /// "load-bearing seams" note in `CLAUDE.md`), and this function is a new
    /// reader of it — without this guard it would drag a surface fight onto
    /// a party that is four frames down, size it off Stack-depth scaling
    /// (`fight_depth`), and raise Trace for kills that aren't underground at
    /// all.
    ///
    /// Order below is load-bearing:
    ///
    /// 1. Leash first, before the field is ever built — a guardian too far
    ///    (Chebyshev) from its own nest, or whose nest no longer resolves
    ///    (`despawn_nest` should already have caught that; this is belt and
    ///    braces), gives up on the spot. A fully-leashed swarm then costs
    ///    one query and no search.
    /// 2. The field is built once, centred on the *player* rather than any
    ///    one nest — the spec says "around the nest", but one box around
    ///    the player is the same bound with a simpler centre, and it holds
    ///    however many nests are provoked at once.
    /// 3. Adjacency is checked *before* every step, not just after: the
    ///    player's own tile has cost 0 in the field and would win any
    ///    downhill comparison, so a pursuer already adjacent must engage
    ///    here rather than take one more step onto the player's tile.
    /// 4. A pursuer absent from the field (farther out than
    ///    `NEST_AGGRO_LEASH_RADIUS + NEST_PATH_SEARCH_MARGIN`, or enclosed
    ///    with no route to the player at all) gives up on the spot too, the
    ///    same as failing the leash check — a guardian that can never reach
    ///    the player is a guardian that will never stop being absent from
    ///    the field either, and leaving `Pursuing` set would freeze it solid
    ///    forever (`wander_ai_system` excludes anything `Pursuing`) while
    ///    paying for a full field build on its behalf every tick from then
    ///    on.
    ///
    /// Only the first pursuer to reach the player fights this tick —
    /// `gather_pack` pulls in anything else standing near it (including a
    /// packmate still mid-chase), so the swarm arrives together rather than
    /// one at a time.
    pub(crate) fn nest_aggro_tick(&mut self) {
        // Off the surface in *either* direction, not just underground: the
        // player's `Position` is pinned to the anchor tile in base space as
        // much as to the entrance tile in the Stack, so a guardian standing
        // beside that tile would otherwise open a battle on a party that is
        // out of phase and nowhere near it. `is_underground` stays
        // Stack-only by design (see `Game::is_underground`), so this is two
        // questions rather than one.
        if self.is_game_over().is_some()
            || self.has_active_battle()
            || self.is_underground()
            || self.in_base()
        {
            return;
        }

        let pursuing: Vec<(Entity, Entity, Position)> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &NestGuardian, &Position), With<Pursuing>>();
            query
                .iter(&self.world)
                .map(|(e, guardian, &pos)| (e, guardian.nest, pos))
                .collect()
        };
        let leashed: Vec<Entity> = pursuing
            .iter()
            .filter(|&&(_, nest, pos)| {
                self.world
                    .get::<Position>(nest)
                    .is_none_or(|&nest_pos| chebyshev(pos, nest_pos) > NEST_AGGRO_LEASH_RADIUS)
            })
            .map(|&(entity, ..)| entity)
            .collect();
        for entity in leashed {
            self.world.entity_mut(entity).remove::<Pursuing>();
        }

        let pursuers: Vec<Entity> = {
            let mut query = self.world.query_filtered::<Entity, With<Pursuing>>();
            query.iter(&self.world).collect()
        };
        if pursuers.is_empty() {
            return;
        }

        let player = self.player_entity();
        let player_pos = *self.world.get::<Position>(player).unwrap();
        let field = {
            let mut map = self.world.resource_mut::<WorldMap>();
            pursuit_field(
                &mut map,
                (player_pos.x, player_pos.y),
                NEST_AGGRO_LEASH_RADIUS + NEST_PATH_SEARCH_MARGIN,
            )
        };

        for pursuer in pursuers {
            let Some(mut pos) = self.world.get::<Position>(pursuer).copied() else {
                continue;
            };
            if chebyshev(pos, player_pos) <= 1 {
                let pack = self.gather_pack(pursuer);
                self.start_battle(pack);
                return;
            }
            // This is also how the base ends a chase, zone-wide, and
            // deliberately so: standing in the platform's interior makes
            // every one of the player's own neighbours Biome::Platform, so
            // `field` above never grows past {player: 0} — the same shape
            // `pursuit_field`'s enclosed-origin case produces. Every
            // pursuer reads as absent from it here, however far off its own
            // chase actually is, not just whichever guardian was closest.
            // See the "Implementation note" under "Pathing: one distance
            // field per tick" in
            // docs/superpowers/archive/specs/2026-08-03-nest-aggression-design.md.
            if !field.contains_key(&(pos.x, pos.y)) {
                self.world.entity_mut(pursuer).remove::<Pursuing>();
                continue;
            }
            for _ in 0..NEST_PURSUIT_STEPS_PER_TICK {
                // Indexing directly rather than `.get`: the `contains_key`
                // check above guarantees the starting tile is present, and
                // every `pos` after that came from a candidate this same
                // `field.get` already found present — the field can't be
                // missing a tile a pursuer is standing on partway through
                // this loop.
                let current_cost = field[&(pos.x, pos.y)];
                let next = NEIGHBOURS
                    .iter()
                    .map(|(dx, dy)| (pos.x + dx, pos.y + dy))
                    .filter_map(|n| field.get(&n).map(|&cost| (n, cost)))
                    .filter(|&(_, cost)| cost < current_cost)
                    .min_by_key(|&(_, cost)| cost);
                let Some((next, _)) = next else {
                    break;
                };
                pos.x = next.0;
                pos.y = next.1;
                if chebyshev(pos, player_pos) <= 1 {
                    break;
                }
            }
            *self.world.get_mut::<Position>(pursuer).unwrap() = pos;
            if chebyshev(pos, player_pos) <= 1 {
                let pack = self.gather_pack(pursuer);
                self.start_battle(pack);
                return;
            }
        }
    }

    /// Ages every deployed `Temporary` structure by one tick, collapsing
    /// (despawning) any that just ran out — dropping a dangling
    /// cronjob/guard `Task` pointed at it the same way `remove_structure`
    /// does, but with no material refund since this is decay, not a
    /// deliberate demolition.
    pub(crate) fn age_temporary_structures(&mut self) {
        let expired: Vec<Entity> = {
            let mut query = self.world.query::<(Entity, &mut Temporary)>();
            query
                .iter_mut(&mut self.world)
                .filter_map(|(entity, mut temp)| {
                    temp.ticks_remaining = temp.ticks_remaining.saturating_sub(1);
                    (temp.ticks_remaining == 0).then_some(entity)
                })
                .collect()
        };
        for entity in expired {
            if let Some(kind) = self.world.get::<Structure>(entity).map(|s| s.kind.clone()) {
                let name = self
                    .world
                    .resource::<StructureDb>()
                    .get(&kind)
                    .map(|d| d.name.clone())
                    .unwrap_or(kind);
                self.log(format!("The {name} burns out and collapses."));
            }
            let workers: Vec<Entity> = {
                let mut tasks = self.world.query::<(Entity, &Task)>();
                tasks
                    .iter(&self.world)
                    .filter(|(_, t)| t.target == entity)
                    .map(|(w, _)| w)
                    .collect()
            };
            for worker in workers {
                self.world.entity_mut(worker).remove::<Task>();
            }
            self.world.despawn(entity);
        }
    }

    /// Drops any job the player is working, narrating it once.
    ///
    /// Its own function because both walkable locales have to do it: a job
    /// is posted from base space (`Game::work_structure` is a base action,
    /// and promises in so many words that moving off breaks it), and the
    /// party can then walk in either base space or on the zone surface. Two
    /// copies would be two places for that narration to drift.
    pub(crate) fn break_off_job(&mut self) {
        let player = self.player_entity();
        if self.world.get::<Task>(player).is_some() {
            self.world.entity_mut(player).remove::<Task>();
            self.log("You break off what you were doing.");
        }
    }

    /// One step on the zone surface — or, in base space, one step through
    /// that, since the same four keys steer both.
    ///
    /// The Stack is the exception rather than a third branch: down there the
    /// party has a facing and moves through `step_forward`/`turn_left` and
    /// friends instead, so these keys are routed away from here entirely by
    /// app-core and this is only the backstop. Walking a pinned `Position`
    /// would drag the player across the zone map without their ever leaving
    /// the Stack.
    pub fn move_player(&mut self, dx: i32, dy: i32) {
        if self.is_game_over().is_some() || self.has_active_battle() || self.is_underground() {
            return;
        }
        // Base space is its own coordinate space with its own walkability,
        // and the player's `Position` stays pinned to the anchor tile
        // throughout — everything below this line is about the zone map and
        // means nothing in there.
        if self.in_base() {
            self.move_in_base(dx, dy);
            return;
        }
        let player = self.player_entity();
        // Any attempt to move ends a job you were working (see
        // `Game::work_structure`) — including one that turns into a fight or
        // bounces off a wall, since either way you stopped working to do it.
        self.break_off_job();
        let pos = *self.world.get::<Position>(player).unwrap();
        let (nx, ny) = (pos.x + dx, pos.y + dy);

        if let Some(target) = self.find_wild_creature_at(nx, ny) {
            let pack = self.gather_pack(target);
            self.start_battle(pack);
            self.tick();
            return;
        }
        if let Some(nest) = self.find_nest_at(nx, ny) {
            self.attack_nest(nest);
            self.tick();
            return;
        }
        if self.find_surface_link_at(nx, ny).is_some() {
            // The entrance survives, unlike a zone portal — it is a place
            // you come back to, not a one-way door.
            self.enter_stack(nx, ny);
            self.tick();
            return;
        }
        if let Some(key) = self.find_settlement_at(nx, ny) {
            // The fourth arm of the same ladder, and the one that admits
            // nobody: a settlement is a landmark to read from the outside,
            // not a door with a hallway behind it, so the bump queues the
            // visit for app-core to open and leaves the player standing
            // exactly where they were — this returns before the step below
            // ever runs.
            self.world
                .resource_mut::<crate::resources::PendingVisit>()
                .0 = Some(key);
            self.tick();
            return;
        }
        // **No structure is consulted here.** Every `Structure` stands in
        // base space — `Structure` is the space tag, and there is exactly
        // one spawn site — so its `Position` is in a different coordinate
        // space from the tile being stepped onto. Asking a base-space query
        // about a surface tile answers by numeric coincidence, and the
        // coincidence is the common case: `find_walkable_start` returns
        // `(0, 0)` whenever it can, so the anchor, the zone spawn point and
        // base space's origin all carry the same numbers, and the pocket
        // covers the player's own starting tile. Left in, the base's own
        // Home made the anchor unwalkable, its machines were invisible
        // walls, and a Portal deployed inside fired a breach from out here.
        //
        // Walking onto a Portal breaches from **inside** the base, where the
        // Portal actually stands — see `Game::move_in_base`.
        let from = self
            .world
            .resource_mut::<WorldMap>()
            .tile(pos.x, pos.y)
            .biome;
        let walkable = self.world.resource_mut::<WorldMap>().tile(nx, ny).walkable;
        let mut drag_ticks = 0;
        if walkable {
            let mut p = self.world.get_mut::<Position>(player).unwrap();
            p.x = nx;
            p.y = ny;
            // Only on a crossing, and only on a step that covered ground:
            // both biomes are in hand here, so nothing is stored and no
            // save field appears. Outside any zone gate on purpose — the
            // ground's *name* is not one of its effects.
            // The ground's bite is a property of *arriving*, and lands
            // ahead of the encounter roll — the same order `Game::arrive`
            // keeps underground. Through `apply_damage`, the one code path
            // that lowers a creature's HP, which is what makes mitigation
            // and every other incoming-damage rule apply for free.
            let terrain = self.terrain_at(nx, ny);
            let max_hp = self.world.get::<Stats>(player).map_or(0, |s| s.max_hp);
            self.apply_damage(player, terrain.effect.bite(max_hp));
            drag_ticks = terrain.effect.extra_ticks;
            // Fired here, where the effect actually lands, and not from
            // `note_static_turnover`'s epoch boundary — that fires for
            // every biome's turnover regardless of where the player is
            // standing, and "you were told" must not come apart from "it
            // happened to you". Unconditional at this site: the once-only
            // rule is `Repeat::OnceEver`, and it lives inside
            // `queue_notification`, not here.
            if terrain.event.is_some() {
                self.notify(crate::notifications::NotificationKind::FirstStatic);
            }
            if terrain.biome != from {
                // The condition's name joins the crossing line rather than
                // getting one of its own — unclaimed ground (`condition:
                // None`) must read exactly as it did before this feature,
                // which is what `for_biome`'s own neutral case already
                // guarantees.
                match terrain.condition {
                    Some(condition) => self.log(format!(
                        "You cross into {} — {}.",
                        terrain.biome.name(),
                        condition.def().name
                    )),
                    None => self.log(format!("You cross into {}.", terrain.biome.name())),
                }
                // First meeting, once per session: `description` otherwise
                // has no reader at all. `SeenConditions` is session-only —
                // see its doc comment — so a reload re-announces, which is
                // cheaper than a save field for flavour text.
                if let Some(condition) = terrain.condition {
                    let seen = &mut self.world.resource_mut::<SeenConditions>().0;
                    if !seen.contains(&condition) {
                        seen.push(condition);
                        self.log(condition.def().description);
                    }
                }
            }
            // Only a step that actually covered ground draws an ambush —
            // every branch above returned already, so walking into a
            // creature, a nest or a portal can't also be jumped, and
            // shoving at a wall isn't travel. Already guarded against a
            // player the ground just killed: `maybe_ambush` checks
            // `is_game_over` itself.
            self.maybe_ambush();
        }
        self.tick();
        // Slow ground is the one step that costs more than a turn. A tick
        // can start a fight — `nest_aggro_tick` is the precedent — so the
        // rest of them are dropped the moment one does, rather than
        // resolving a world the player is no longer standing in while a
        // battle waits on the screen. Each of these ticks — and the one
        // above — makes its own `note_static_turnover` call from inside
        // `tick_inner`, so a `Drag` step spending several ticks still
        // announces at most once: only the one tick that actually crosses a
        // boundary has anything to say.
        for _ in 0..drag_ticks {
            if self.is_game_over().is_some() || self.has_active_battle() {
                break;
            }
            self.tick();
        }
    }

    /// Announces weather arriving or clearing under the player, if the tick
    /// `tick_inner` just took crossed a weather epoch boundary. Called from
    /// `tick_inner` itself, once per tick, right after the clock advances —
    /// the one place the clock actually moves, rather than a second call
    /// site threaded through every verb that spends a tick. A single tick
    /// only ever moves the clock forward by one, so it can cross at most one
    /// boundary; this cannot fire more than once for it.
    ///
    /// This is what makes standing still (`Game::idle_tick`), crafting,
    /// building, trading and every other tick-spending verb announce a
    /// boundary exactly like a step does — the previous version of this
    /// hook was reachable only from `move_player`, so a boundary crossed
    /// while the player stood still, worked a bench, or traded was missed
    /// forever: nothing about the previous epoch is stored, so the next
    /// step's own comparison could not see back past it.
    ///
    /// Fires only for the biome the player is standing in *now* — the other
    /// biomes turning over silently is the point, or a boundary would cost
    /// five lines of spam. The comparison is `static_in_epoch(biome,
    /// epoch_before)` against `static_in_epoch(biome, static_epoch())`, both
    /// pure calls: nothing about the previous epoch is stored anywhere,
    /// which is what keeps a save/load mid-epoch from re-announcing.
    ///
    /// Gated through `Game::environment_biome_at` — zone 1 and the base's
    /// own `Platform` floor never carry weather, so announcing a turnover
    /// there would describe an effect that never actually bites. Reading
    /// that gate rather than carrying a second copy of its two checks is
    /// what keeps this in step with `terrain_at`'s own refusal.
    ///
    /// **Also refuses underground and in base space**, which
    /// `environment_biome_at` alone does not: `Position` stays pinned to the
    /// surface entrance (Stack) or the anchor tile (base) in both locales —
    /// the same pinning `terrain_row`'s own guard exists for — so without
    /// this a boundary turning over at the entrance tile would announce
    /// weather at a place the player is not standing while they are four
    /// frames down or safely inside the base pocket. `move_player` already
    /// refused both states before this was ever reachable from anywhere but
    /// itself; called from `tick_inner`, every tick-spending verb in both
    /// locales reaches this and needs the same refusal.
    ///
    /// **Also refuses while a battle is active.** `move_player` used to be
    /// the only caller, and it refuses outright on `has_active_battle()`, so
    /// this was structurally unreachable during a fight. Moving the call
    /// into `tick_inner` opened three battle-active paths to it — the
    /// ambush early return's own `tick()`, an ordinary combat round's
    /// `tick()`, and a failed jack-out's `tick()` — and `MessageSource::
    /// Field` is not one the battle pane filters out, so a boundary crossed
    /// mid-fight would interleave a weather line into the fight's own
    /// narration. A boundary genuinely crossed during a battle is simply
    /// never announced: nothing about the epoch is stored, so there is
    /// nothing to announce late, and the player reads the weather off the
    /// map pane's border the moment the fight ends.
    pub(crate) fn note_static_turnover(&mut self, epoch_before: u64) {
        let epoch_after = self.static_epoch();
        if epoch_after == epoch_before {
            return;
        }
        if self.is_underground() || self.in_base() || self.has_active_battle() {
            return;
        }
        let player = self.player_entity();
        let pos = *self.world.get::<Position>(player).unwrap();
        let Some(biome) = self.environment_biome_at(pos.x, pos.y) else {
            return;
        };
        let before = self.static_in_epoch(biome, epoch_before);
        let after = self.static_in_epoch(biome, epoch_after);
        match (before, after) {
            (None, Some(event)) => self.log(format!(
                "Static: {} settles over {}. {}",
                event.def().name,
                biome.name(),
                event.def().description
            )),
            (Some(event), None) => self.log(format!(
                "Static: {} clears from {}.",
                event.def().name,
                biome.name()
            )),
            _ => {}
        }
    }

    /// Rolls `RANDOM_ENCOUNTER_CHANCE` for an ambush: on a hit, a
    /// biome-appropriate pack spawns on a neighbouring tile and engages
    /// immediately. Unlike every other way a fight starts, the player never
    /// saw this one coming and cannot route around it — which is the point.
    /// Crossing open ground is meant to cost something.
    ///
    /// Three deliberate limits:
    ///
    /// - Never on the base platform. It is a manufactured floor rather than
    ///   terrain, nothing spawns there, and it stays the one safe ground.
    /// - Never a boss (`pick_habitat_species(.., false)`). A boss you find
    ///   on the map is a fight you chose; one that jumps you is a death
    ///   sentence you never opted into.
    /// - Never a nest, since `spawn_pack` is called directly rather than
    ///   through `try_spawn_habitat_creature`. A nest is a structure you
    ///   attack, not a fight that attacks you.
    ///
    /// Spends the roll and does nothing if the player is boxed in by
    /// unwalkable tiles or the biome offers no ordinary species. Hunting
    /// further afield for somewhere to put a fight nobody asked for would
    /// be worse than letting the roll lapse.
    ///
    /// Deliberately skips the `WILD_CREATURE_CAP` cull that
    /// `maybe_spawn_wild_creature` performs: that cap bounds the population
    /// of *idle* programs the player walked away from, and an ambush pack
    /// is about to be resolved rather than left to roam.
    ///
    /// One `terrain_at` call, not a second tile lookup: the tile is already
    /// in `WorldMap`'s chunk cache from the step that led here, and its
    /// `effect.ambush_mult` is what a live weather event scales this roll
    /// by — `EnvironmentEffect::fold`'s multiplicative term, reaching a roll
    /// rather than a bite.
    pub(crate) fn maybe_ambush(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let player = self.player_entity();
        let pos = *self.world.get::<Position>(player).unwrap();
        let terrain = self.terrain_at(pos.x, pos.y);
        if terrain.biome == Biome::Platform {
            return;
        }
        let ambushed = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(
                (RANDOM_ENCOUNTER_CHANCE * terrain.effect.ambush_mult as f64).clamp(0.0, 1.0),
            )
        };
        if !ambushed {
            return;
        }
        let pack = self.surface_ambush_pack();
        if pack.is_empty() {
            return;
        }
        self.log("Something drops out of the noise floor — you've been made!");
        self.start_battle(pack);
    }

    /// The pack a *surface* ambush fields: one biome-appropriate group placed
    /// on a walkable neighbour of the player's tile, then widened by
    /// `gather_pack` so programs already standing there are pulled in too —
    /// exactly as walking into them would.
    ///
    /// `stack_encounter_pack`'s counterpart, and the two are named as a pair
    /// on purpose. **A rest is the first roll site that cannot know its
    /// locale by construction**: every other spawn path is reached from one
    /// kind of movement, so the placement rules only ever had one home. A
    /// rest happens anywhere, so the pack has to be *chosen*, and the choice
    /// is only safe while each half states its own placement once.
    ///
    /// Returns empty rather than refusing when the player is boxed in by
    /// unwalkable tiles or the biome offers no ordinary species. Both
    /// callers read that as "the roll lapses" — hunting further afield for
    /// somewhere to put a fight nobody asked for would be worse.
    pub(crate) fn surface_ambush_pack(&mut self) -> Vec<Entity> {
        let player = self.player_entity();
        let pos = *self.world.get::<Position>(player).unwrap();
        let open: Vec<(i32, i32)> = NEIGHBOURS
            .iter()
            .map(|(dx, dy)| (pos.x + dx, pos.y + dy))
            .filter(|&(x, y)| self.world.resource_mut::<WorldMap>().tile(x, y).walkable)
            .collect();
        if open.is_empty() {
            return Vec::new();
        }
        let (tx, ty) = {
            let mut rng = self.world.resource_mut::<GameRng>();
            open[rng.0.random_range(0..open.len())]
        };
        let Some((species, _)) = self.pick_habitat_species(tx, ty, None, false) else {
            return Vec::new();
        };
        let esc = self.field_escalation(tx, ty);
        let pack = self.spawn_pack(&species, false, tx, ty, esc);
        let Some(&anchor) = pack.first() else {
            return Vec::new();
        };
        self.gather_pack(anchor)
    }

    /// Consume one unit of `id` out of battle, applying its `ConsumeDef`:
    /// restore Power/Integrity (each clamped) and/or arm a
    /// `FieldBuff` (see `use_item`'s `prebattle_buff`) that outlives
    /// whatever intrusion it's next used in. A non-consumable or an empty
    /// stack is a logged no-op.
    pub fn use_item(&mut self, id: &ItemId) {
        // Still refused mid-intrusion: in battle an item is a *planned
        // action* costing that slot its round (`BattleAction::UseItem`), not
        // something the inventory screen can spend for free.
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        if self.consume_item(self.player_entity(), id) {
            self.tick();
        }
    }

    /// Spends one `id` and applies its consume effect to `who`, *without*
    /// advancing the clock. Shared by the map's `use_item` and the battle's
    /// `BattleAction::UseItem`, which tick on their own schedules — a round
    /// already ticks once at the end of `battle_resolve_round`, so an item
    /// used mid-round must not tick a second time. Returns whether an item
    /// was actually consumed.
    ///
    /// **The pack is always the player's and the effect is always `who`'s.**
    /// `Inventory` lives on the player alone and is the party's one shared
    /// kit, so a companion spending its round on a Power Cell draws from the
    /// same stack the player would — but the charge lands on the companion's
    /// own `PowerReserve`, which is the reserve its Specials are priced
    /// against (`combat_round.rs`'s `spend_power(entity, ..)`).
    ///
    /// A missing `PowerReserve` or `Stats` is a no-op rather than a panic,
    /// matching `spend_power`'s asymmetry: nothing outside the roster reaches
    /// here, and a hostile that somehow did should not crash the run.
    pub(crate) fn consume_item(&mut self, who: Entity, id: &ItemId) -> bool {
        let Some(effect) = self
            .world
            .resource::<ItemDb>()
            .get(id.as_str())
            .and_then(|d| d.consume)
        else {
            self.log("You can't use that.");
            return false;
        };
        let player = self.player_entity();
        if self
            .world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(id.clone(), 1)
            == 0
        {
            self.log(format!("You have no {}.", self.item_name(id)));
            return false;
        }
        if let Some(mut needs) = self.world.get_mut::<PowerReserve>(who) {
            needs.restore(effect.power);
        }
        if effect.heal != 0
            && let Some(mut stats) = self.world.get_mut::<Stats>(who)
        {
            stats.hp = (stats.hp + effect.heal).min(stats.max_hp);
        }
        let name = self.item_name(id).to_string();
        if let Some(buff) = effect.prebattle_buff {
            self.arm_field_buff(
                who,
                ActiveFieldBuff {
                    kind: buff.kind,
                    name: name.clone(),
                    power: buff.power,
                    remaining: buff.ticks,
                    // A consumable's buff has no cadence of its own to
                    // author — `ItemEffect::prebattle_buff` carries no
                    // `interval` — so it ticks every turn, as every field
                    // buff did before routines gained one.
                    interval: crate::abilities::every_turn(),
                    source: BuffSource::Consumable,
                },
            );
        }
        if who == player {
            self.log(format!("You use a {name}."));
        } else {
            let label = self.creature_label(who);
            self.log(format!("{label} uses a {name}."));
        }
        true
    }

    /// The `e` shortcut: use the first inventory item that restores Power.
    pub fn use_power_source(&mut self) {
        let player = self.player_entity();
        // Scope the DB + Inventory borrows so both release before use_item's
        // &mut self: target is an owned Option<ItemId>.
        let target = {
            let db = self.world.resource::<ItemDb>();
            let inv = self.world.get::<Inventory>(player).unwrap();
            inv.items.iter().map(|(id, _)| id.clone()).find(|id| {
                db.get(id.as_str())
                    .and_then(|d| d.consume)
                    .is_some_and(|c| c.power > 0.0)
            })
        };
        match target {
            Some(id) => self.use_item(&id),
            None => self.log("You have nothing to recharge from."),
        }
    }

    /// Power down: Integrity and Power restored to full, for the player and
    /// for every program that is **not base staff**.
    ///
    /// Priced by where the party is standing and nothing else. **Inside
    /// base space it is free** — the walk home is the cost, and no
    /// structure has to be in reach. **Anywhere else** — the open grid or
    /// four frames down the Stack alike — it spends one unit of an item
    /// whose def sets `ItemDef::enables_rest`, the Power Outlet among the
    /// shipped items.
    ///
    /// **A rest repairs the programs standing with the player and nobody
    /// else**, and that is a **per-role, exhaustive** decision:
    /// `InParty` and `Wielded` are repaired, `Sortie` and `Staff` are not.
    /// The base's labour pool mends at a Repair Bay, which is what that
    /// building is for — a rest that healed staff made the Bay optional and
    /// a survived sweep free — and a squad in another sector is provisioned
    /// out of its own dispatch (`SORTIE_PROVISION_HEAL_FRACTION` between
    /// battles), not out of a rest taken somewhere it cannot hear.
    ///
    /// The split is read off `Game::program_role` and never off `Party`
    /// membership: `Staff` is what `party::role_of` leaves *over*, so a
    /// hand-written "is it in the party" test here would also exclude
    /// `Wielded`, which is carried in the player's own hands.
    ///
    /// **Written as an exhaustive match, `cell_mark`'s rule**, and it was
    /// briefly not. While `Staff` was the only exclusion this read as
    /// `!= Staff` on the argument that a fifth role should inherit the heal,
    /// since being left out is what strands a program. `Sortie` joining the
    /// exclusion is what retired that argument: the roles now split two and
    /// two on *whether the program is with you*, there is no majority to
    /// default to, and the one role that was defaulted in — `Sortie` —
    /// turned out to be the one that wanted defaulting out. A fifth role
    /// must fail to compile here and be answered deliberately.
    ///
    /// **The split is by role and not by locale.** A rest four frames down
    /// the Stack does not reach back and repair the base, which is what it
    /// did before — the over-reach was that the walk was over every `Tamed`
    /// program the player owned, wherever anybody was standing.
    ///
    /// **Power is deliberately still refilled for everyone**, excluded roles
    /// included. A Bay restores Integrity and nothing else, and nothing
    /// refills a program's reserve passively, so withholding it here would
    /// leave a staff program that spent Power defending a sweep with no
    /// route back to full — a second dead end, invented rather than asked
    /// for. The rule is about the repair.
    ///
    /// **No rest advances the clock**, and that is what makes the free half
    /// safe to give away: a base rest that ticked could be spammed to farm
    /// production, raid pressure and need decay. It also means nothing in
    /// the game fast-forwards any more — `Game::wait` is the only way time
    /// passes without an action, one tick at a time.
    ///
    /// **One thing can happen after the charge is taken, and there is still
    /// no refund path.** A charged rest rolls `REST_AMBUSH_CHANCE` for an
    /// interrupt below the payment and above the restore. On a hit the
    /// outlet is gone and nothing is restored — that is the mechanic rather
    /// than an oversight, since powering down in the open is what left the
    /// party exposed, and a refund makes the risk free and the number
    /// meaningless.
    ///
    /// Three properties fall out of that placement. It **rides the branch
    /// that takes the charge**, so a free base rest never reaches the roll
    /// and the slab stays safe without a locale check of its own. A jumped
    /// rest **clears nothing** — the heal, the roster walk and
    /// `drop_until_rest_buffs_on_party` all sit below it, which is the rule
    /// a *refused* rest already follows, so the two failure modes agree. And
    /// a roll that hits but **fields no pack lapses into an ordinary rest**,
    /// because a charge burnt for no fight at all is the one outcome a
    /// player cannot read as anything but a bug.
    ///
    /// The interrupt is an `Ok`, not an `Err`: the charge really was spent
    /// and a fight really did start, so there is nothing for `App::refuse`
    /// to put on the status banner. It is news, and it goes to the log.
    ///
    /// **Every exit that does not rest says why**, which is what the
    /// `Result` is for rather than the bare `return`s this used to take.
    /// Three of its four failures were silent and the fourth was a plain
    /// `log()` line — an `Info` the log filter can hide — so a rest that
    /// did nothing was indistinguishable from a key that was never bound.
    /// That is not a hypothetical: `r` genuinely *was* unbound in the Stack
    /// until 0.13.21, the report survived the fix, and nothing either side
    /// of the seam could tell the two apart. A refusal now goes back to the
    /// caller, which puts it on the map's status banner through
    /// `App::refuse` like every other refused verb on that screen.
    pub fn rest(&mut self) -> Result<(), String> {
        if self.is_game_over().is_some() {
            return Err("Your run is over.".to_string());
        }
        if self.has_active_battle() {
            return Err("No powering down in the middle of an intrusion.".to_string());
        }
        let player = self.player_entity();
        let mut spent = None;
        if !self.in_base() {
            let Some(charge) = self.rest_charge_in_pack() else {
                let name = match self.rest_charge_name() {
                    Some(name) => format!("no {name}"),
                    None => "nothing".to_string(),
                };
                return Err(format!("You have {name} to power down with out here."));
            };
            // `rest_charge_in_pack` has already refused an empty slot, so
            // this cannot come back short — it is spelled as a refusal
            // rather than an `unwrap` because the alternative, the silent
            // `return` it replaces, is the bug this whole path is about.
            if self
                .world
                .get_mut::<Inventory>(player)
                .unwrap()
                .take(charge.clone(), 1)
                == 0
            {
                let name = self.item_name(&charge).to_string();
                return Err(format!("You have no {name} to power down with out here."));
            }
            let name = self.item_name(&charge).to_string();
            // Below the payment and above every restore below, which is the
            // whole of the design: see this function's doc comment.
            if self.roll_rest_interrupt() {
                // The first roll site that cannot know its locale by
                // construction, so the pack is chosen rather than implied.
                let pack = if self.stack_pos().is_some() {
                    self.stack_encounter_pack()
                } else {
                    self.surface_ambush_pack()
                };
                if !pack.is_empty() {
                    // Pins the cell on the frame map, exactly as a Stack
                    // encounter walked into does. A no-op on the surface.
                    self.remember_fight();
                    self.log(format!(
                        "Your {name} burns out — something was already in here with you."
                    ));
                    self.start_battle(pack);
                    return Ok(());
                }
            }
            spent = Some(name);
        }
        {
            let mut needs = self.world.get_mut::<PowerReserve>(player).unwrap();
            needs.fill();
        }
        {
            let mut stats = self.world.get_mut::<Stats>(player).unwrap();
            stats.hp = stats.max_hp;
        }
        // Below the gates rather than beside them, so a refused rest clears
        // nothing. The walk is player then `Party` — the same set
        // `tick_field_buffs` ages.
        self.drop_until_rest_buffs_on_party();
        // The roles are resolved into the list rather than asked for inside
        // the loop, since `program_role` borrows the world immutably and the
        // restores below take it mutably.
        let owned: Vec<(Entity, Option<ProgramRole>)> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Tamed), With<Creature>>();
            let owned: Vec<Entity> = query
                .iter(&self.world)
                .filter(|(_, t)| t.owner == player)
                .map(|(e, _)| e)
                .collect();
            owned
                .into_iter()
                .map(|e| (e, self.program_role(e)))
                .collect()
        };
        for (creature, role) in owned {
            // Exhaustive rather than a `!=`, `cell_mark`'s rule: the question
            // is whether this program is standing *with* the player, the four
            // roles answer it two and two, and there is no safe side to
            // default a fifth one to. `None` is unreachable — `owned` is
            // already filtered to programs this player owns — and is spelled
            // out rather than folded into a catch-all so it cannot become the
            // arm a new role quietly lands in.
            let repaired = match role {
                Some(ProgramRole::InParty | ProgramRole::Wielded) => true,
                Some(ProgramRole::Sortie | ProgramRole::Staff) => false,
                None => false,
            };
            if repaired && let Some(mut stats) = self.world.get_mut::<Stats>(creature) {
                stats.hp = stats.max_hp;
            }
            // Rest is the *only* refill for a companion's reserve: nothing
            // restores one passively, and there is no way to hand a program a
            // Power Cell mid-fight. The excluded roles included, deliberately
            // — a Bay restores Integrity and nothing else, so withholding
            // this too would strand a program that spent Power defending a
            // sweep.
            if let Some(mut reserve) = self.world.get_mut::<PowerReserve>(creature) {
                reserve.fill();
            }
        }
        match spent {
            Some(name) => self.log(format!(
                "You burn a {name} and come back online, fully recharged and repaired."
            )),
            None => self.log(
                "You power down at the base and come back online, fully recharged and repaired.",
            ),
        }
        Ok(())
    }

    /// The first item in the player's pack whose def sets
    /// `ItemDef::enables_rest` **and which there is at least one of** — what
    /// a field rest is bought with. Mirrors `use_power_source`'s scan, which
    /// answers the same shape of question about Power.
    ///
    /// The quantity is part of the predicate because a charge is a unit you
    /// can spend, not an id that happens to be in the map.
    /// `Inventory::take` drops a slot the moment it empties, but
    /// `Inventory::add` *pushes* a `(item, 0)` slot when asked for none and
    /// no slot exists yet — so an empty stack is a reachable state, and one
    /// matched on the flag alone sailed past the "you have none" refusal
    /// into a `take` that came back with nothing.
    fn rest_charge_in_pack(&self) -> Option<ItemId> {
        let player = self.player_entity();
        let db = self.world.resource::<ItemDb>();
        let inv = self.world.get::<Inventory>(player).unwrap();
        inv.items
            .iter()
            .filter(|(_, qty)| *qty > 0)
            .map(|(id, _)| id.clone())
            .find(|id| db.get(id.as_str()).is_some_and(|d| d.enables_rest))
    }

    /// What to call a rest charge in a refusal, when the player is holding
    /// none to name. Read off the catalogue rather than hardcoded, so an
    /// install that renamed or replaced the Power Outlet still refuses in
    /// its own vocabulary — and one that ships no rest charge at all says
    /// so instead of naming an item that does not exist.
    fn rest_charge_name(&self) -> Option<String> {
        let db = self.world.resource::<ItemDb>();
        let mut names: Vec<&str> = db
            .all()
            .filter(|d| d.enables_rest)
            .map(|d| d.name.as_str())
            .collect();
        names.sort_unstable();
        names.first().map(|n| n.to_string())
    }

    /// One draw against `REST_AMBUSH_CHANCE`, spelled out rather than
    /// inlined so the borrow of `GameRng` ends before the pack is conjured
    /// — `stack_encounter_pack` and `surface_ambush_pack` both take the
    /// resource again themselves.
    ///
    /// Unconditional at its one call site, which is what makes "a free base
    /// rest never touches the stream" a property of *where this is called*
    /// rather than of a locale test inside it.
    fn roll_rest_interrupt(&mut self) -> bool {
        let mut rng = self.world.resource_mut::<GameRng>();
        rng.0.random_bool(REST_AMBUSH_CHANCE)
    }

    /// Stand in place for a single tick — lets the world (wander AI,
    /// cronjob production, needs decay) advance by one step without moving
    /// or taking any other action. Distinct from `rest`, which advances
    /// `REST_TICKS` at once and restores Power.
    pub fn wait(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        self.tick();
    }

    /// Awards unsolicited income — battle loot, a Stack cache, a demolished
    /// structure's refund — returning how many units landed. The Buffer is
    /// unbounded and Research Data has no cap of its own, so this always
    /// lands `qty` in full; callers still read the return, since it's the
    /// same value they'd otherwise have to pass through by hand.
    ///
    /// **`source` is the whole reason this is one door.** Eighteen callers
    /// pass through here, and B5 — what share of a sector's Core Fragments
    /// a Mining Node is actually worth — is unanswerable without knowing
    /// which of them a unit came through.
    ///
    /// Recorded, never folded: see `telemetry::Record::Acquire`. The record
    /// is built **before** the item is spent into the pack, so the disarmed
    /// path allocates nothing — `Game::record`'s discipline, which a
    /// `item.clone()` above the call would quietly undo.
    pub(crate) fn grant_loot(&mut self, item: ItemId, qty: u32, source: LootSource) -> u32 {
        self.record(|g| Record::Acquire {
            tick: g.current_tick(),
            zone: g.world.resource::<crate::resources::ZoneLevel>().0,
            item: item.0.clone(),
            qty,
            source: source.as_str().to_string(),
        });
        let player = self.player_entity();
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(item, qty);
        qty
    }
}
