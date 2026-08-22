//! The turn loop: advancing the clock, moving, and the actions a player
//! spends a turn on.

use crate::environment::EnvironmentEffect;
use crate::game::pursuit::pursuit_field;
use crate::game::spawning::SpawnEscalation;
use crate::tuning::{
    NEST_AGGRO_LEASH_RADIUS, NEST_PATH_SEARCH_MARGIN, NEST_PURSUIT_STEPS_PER_TICK,
    RANDOM_ENCOUNTER_CHANCE,
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
        // Before the ambient roll, not after: the roll's density gate reads
        // `local_hostile_count`, and asking it about ground that has not been
        // stocked yet would answer "empty" and spend a spawn filling in what
        // is about to arrive properly.
        self.ensure_local_population();
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
        self.schedule.run(&mut self.world);
        // Immediately after the schedule, where `haul_step_system`'s commands
        // have just flushed and the clock has not yet moved: a stranding is an
        // *edge*, and one tick later there is nothing left to read it off.
        self.note_strandings();
        // Immediately after the schedule, which is where `contract_system`
        // raised the progress this reads. Paying is `&mut Game` work — an
        // inventory write and an XP grant — so it cannot live in the system
        // that counts.
        self.settle_contracts();
        self.structure_regen();
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
        self.world.resource_mut::<GameClock>().tick += 1;
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
        let to = self.world.resource_mut::<WorldMap>().tile(nx, ny);
        let walkable = to.walkable;
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
            let effect = self.ground_effect(nx, ny).map(|d| d.effect);
            match effect {
                Some(EnvironmentEffect::Attrition {
                    hp_percent,
                    min_damage,
                }) => {
                    let max_hp = self.world.get::<Stats>(player).map_or(0, |s| s.max_hp);
                    let bite = ((max_hp as f32 * hp_percent).round() as i32).max(min_damage);
                    self.apply_damage(player, bite);
                }
                Some(EnvironmentEffect::Drag { extra_ticks }) => drag_ticks = extra_ticks,
                None => {}
            }
            if to.biome != from {
                self.log(format!("You cross into {}.", to.biome.name()));
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
        // battle waits on the screen.
        for _ in 0..drag_ticks {
            if self.is_game_over().is_some() || self.has_active_battle() {
                break;
            }
            self.tick();
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
    pub(crate) fn maybe_ambush(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let player = self.player_entity();
        let pos = *self.world.get::<Position>(player).unwrap();
        if self
            .world
            .resource_mut::<WorldMap>()
            .tile(pos.x, pos.y)
            .biome
            == Biome::Platform
        {
            return;
        }
        let ambushed = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(RANDOM_ENCOUNTER_CHANCE)
        };
        if !ambushed {
            return;
        }
        let open: Vec<(i32, i32)> = NEIGHBOURS
            .iter()
            .map(|(dx, dy)| (pos.x + dx, pos.y + dy))
            .filter(|&(x, y)| self.world.resource_mut::<WorldMap>().tile(x, y).walkable)
            .collect();
        if open.is_empty() {
            return;
        }
        let (tx, ty) = {
            let mut rng = self.world.resource_mut::<GameRng>();
            open[rng.0.random_range(0..open.len())]
        };
        let Some((species, _)) = self.pick_habitat_species(tx, ty, None, false) else {
            return;
        };
        let pack = self.spawn_pack(&species, false, tx, ty, SpawnEscalation::surface());
        let Some(&anchor) = pack.first() else {
            return;
        };
        self.log("Something drops out of the noise floor — you've been made!");
        // Through `gather_pack` rather than engaging the spawned pack
        // directly, so an ambush sprung beside programs already standing
        // there pulls them in too, exactly as walking into one would.
        let pack = self.gather_pack(anchor);
        self.start_battle(pack);
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
        if self.consume_item(id) {
            self.tick();
        }
    }

    /// Spends one `id` and applies its consume effect to the player,
    /// *without* advancing the clock. Shared by the map's `use_item` and the
    /// battle's `BattleAction::UseItem`, which tick on their own schedules —
    /// a round already ticks once at the end of `battle_resolve_round`, so
    /// an item used mid-round must not tick a second time. Returns whether
    /// an item was actually consumed.
    pub(crate) fn consume_item(&mut self, id: &ItemId) -> bool {
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
        {
            let mut needs = self.world.get_mut::<PowerReserve>(player).unwrap();
            needs.restore(effect.power);
        }
        if effect.heal != 0 {
            let mut stats = self.world.get_mut::<Stats>(player).unwrap();
            stats.hp = (stats.hp + effect.heal).min(stats.max_hp);
        }
        let name = self.item_name(id).to_string();
        if let Some(buff) = effect.prebattle_buff {
            self.arm_field_buff(
                player,
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
        self.log(format!("You use a {name}."));
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

    /// Power down: Integrity and Power are restored to full, for the
    /// player, the party and every tamed program on the roster.
    ///
    /// Priced by where the party is standing and nothing else. **Inside
    /// base space it is free** — the walk home is the cost, and no
    /// structure has to be in reach. **Anywhere else** — the open grid or
    /// four frames down the Stack alike — it spends one unit of an item
    /// whose def sets `ItemDef::enables_rest`, the Power Outlet among the
    /// shipped items.
    ///
    /// **No rest advances the clock**, and that is what makes the free half
    /// safe to give away: a base rest that ticked could be spammed to farm
    /// production, raid pressure and need decay. It also means nothing in
    /// the game fast-forwards any more — `Game::wait` is the only way time
    /// passes without an action, one tick at a time.
    ///
    /// Nothing can fail after the charge is taken, so there is no refund
    /// path: the two gates and the payment run in that order and the
    /// restore is unconditional from there.
    pub fn rest(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let player = self.player_entity();
        let mut spent = None;
        if !self.in_base() {
            let Some(charge) = self.rest_charge_in_pack() else {
                let name = match self.rest_charge_name() {
                    Some(name) => format!("no {name}"),
                    None => "nothing".to_string(),
                };
                self.log(format!("You have {name} to power down with out here."));
                return;
            };
            if self
                .world
                .get_mut::<Inventory>(player)
                .unwrap()
                .take(charge.clone(), 1)
                == 0
            {
                return;
            }
            spent = Some(self.item_name(&charge).to_string());
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
        // Every tamed program you own gets fully healed too, not just your
        // active party — including any left behind defending a structure
        // from a raid while you were away.
        let owned: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Tamed), With<Creature>>();
            query
                .iter(&self.world)
                .filter(|(_, t)| t.owner == player)
                .map(|(e, _)| e)
                .collect()
        };
        for creature in owned {
            if let Some(mut stats) = self.world.get_mut::<Stats>(creature) {
                stats.hp = stats.max_hp;
            }
            // Rest is the *only* refill for a companion's reserve: nothing
            // restores one passively, and there is no way to hand a program a
            // Power Cell mid-fight.
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
    }

    /// The first item in the player's pack whose def sets
    /// `ItemDef::enables_rest` — what a field rest is bought with. Mirrors
    /// `use_power_source`'s scan, which answers the same shape of question
    /// about Power.
    fn rest_charge_in_pack(&self) -> Option<ItemId> {
        let player = self.player_entity();
        let db = self.world.resource::<ItemDb>();
        let inv = self.world.get::<Inventory>(player).unwrap();
        inv.items
            .iter()
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
    pub(crate) fn grant_loot(&mut self, item: ItemId, qty: u32) -> u32 {
        let player = self.player_entity();
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(item, qty);
        qty
    }
}
