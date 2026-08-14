//! The turn loop: advancing the clock, moving, and the actions a player
//! spends a turn on.

use crate::game::pursuit::pursuit_field;
use crate::game::spawning::SpawnEscalation;
use crate::tuning::{
    NEST_AGGRO_LEASH_RADIUS, NEST_PATH_SEARCH_MARGIN, NEST_PURSUIT_STEPS_PER_TICK,
    RANDOM_ENCOUNTER_CHANCE, REST_TICKS,
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
        self.maybe_spawn_wild_creature();
        // Before the schedule, not after, so a body posted this tick makes
        // progress this tick rather than standing at its machine for one.
        // Beside `maybe_spawn_wild_creature` for the same reason that one is
        // here: both are `&mut Game` work a bevy system cannot express.
        self.schedule_base_labour();
        self.schedule.run(&mut self.world);
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
        if self.is_game_over().is_some() || self.has_active_battle() || self.is_underground() {
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

    pub fn move_player(&mut self, dx: i32, dy: i32) {
        // Underground the party moves through `step_forward`/`turn_left` and
        // friends instead — `Position` is pinned to the entrance tile while
        // down there, so walking it would drag the player across the zone
        // map without their ever leaving the Stack.
        if self.is_game_over().is_some() || self.has_active_battle() || self.is_underground() {
            return;
        }
        let player = self.player_entity();
        // Any attempt to move ends a job you were working (see
        // `Game::work_structure`) — including one that turns into a fight or
        // bounces off a wall, since either way you stopped working to do it.
        if self.world.get::<Task>(player).is_some() {
            self.world.entity_mut(player).remove::<Task>();
            self.log("You break off what you were doing.");
        }
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
        if let Some(portal) = self.find_zone_portal_at(nx, ny) {
            // Consumed before enter_next_zone snapshots the base, so it
            // isn't carried forward. Load-bearing now that structures
            // survive a breach: a portal that travelled would make every
            // breach after the first free, bypassing its per-zone cost.
            self.world.despawn(portal);
            self.enter_next_zone();
            self.tick();
            return;
        }
        if self.find_blocking_structure_at(nx, ny).is_some() {
            return;
        }
        let walkable = self.world.resource_mut::<WorldMap>().tile(nx, ny).walkable;
        if walkable {
            let mut p = self.world.get_mut::<Position>(player).unwrap();
            p.x = nx;
            p.y = ny;
            // Only a step that actually covered ground draws an ambush —
            // every branch above returned already, so walking into a
            // creature, a nest or a portal can't also be jumped, and
            // shoving at a wall isn't travel.
            self.maybe_ambush();
        }
        self.tick();
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
        let Some((species, _)) = self.pick_habitat_species(tx, ty, false) else {
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
    /// restore Power/Fatigue/Integrity (each clamped) and/or arm a
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
            let mut needs = self.world.get_mut::<Needs>(player).unwrap();
            needs.hunger = (needs.hunger + effect.power).min(NEED_MAX);
            needs.fatigue = (needs.fatigue + effect.fatigue).min(NEED_MAX);
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

    /// Power down for the night: many ticks pass at once (power reserves
    /// drain accordingly, tamed programs keep processing, rogue programs
    /// keep roaming), then Fatigue and Integrity are both restored to full.
    /// Requires the player to be standing within the radius of a structure
    /// that sets `StructureDef::enables_rest` — Home, and only Home, among
    /// the shipped structures — and, since it grants a free heal otherwise
    /// unbounded by anything Power can limit, spending that structure's
    /// `RestDef::cost` (an empty cost is a free rest, unchanged from before
    /// the field existed). Both gates run before a single tick does; the
    /// price is resolved and taken here rather than as its own system
    /// because a refused rest must not spend anything, and a rest that
    /// *starts* has already bought its ticks — the mid-loop `is_game_over`
    /// bail below does not refund it. Beyond that, there's no separate
    /// "rest" system beyond replaying the normal tick loop plus a
    /// Fatigue/HP reset at the end (via `tick_inner(false)`, so these ticks
    /// don't age the rest structure itself — see
    /// `age_temporary_structures`). If Power runs out and you take lethal
    /// damage mid-rest, the loop bails out via the `is_game_over` check
    /// before either restore happens.
    pub fn rest(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        if let Err(reason) = self.require_surface() {
            self.log(reason);
            return;
        }
        let player_pos = *self.world.get::<Position>(self.player_entity()).unwrap();
        let Some(rest_structure) = self.nearby_rest_structure(player_pos) else {
            self.log("You need to be within your base, near Home, to power down and rest.");
            return;
        };
        let cost = self.rest_cost(rest_structure);
        let player = self.player_entity();
        let missing = {
            let inv = self.world.get::<Inventory>(player).unwrap();
            cost.iter()
                .find(|(item, qty)| inv.count(item) < *qty)
                .cloned()
        };
        if let Some((item, qty)) = missing {
            self.log(format!(
                "Resting needs {} {}, and you're short.",
                qty,
                self.item_name(&item)
            ));
            return;
        }
        let taken: Vec<(ItemId, u32)> = {
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            cost.iter()
                .map(|(item, qty)| (item.clone(), inv.take(item.clone(), *qty)))
                .collect()
        };
        // The affordability check above tests each `(item, qty)` pair
        // independently against the full stack, so a `cost` with a repeated
        // item id can pass it and still come up short here once an earlier
        // pair has already spent from the same stack. `take`'s return is
        // exactly how much came off, so a shortfall is caught rather than
        // silently treated as paid — refund what was taken, since a rest
        // that can't fully afford itself must spend nothing.
        if let Some((item, qty)) = cost
            .iter()
            .zip(&taken)
            .find(|((_, qty), (_, got))| got < qty)
            .map(|((item, qty), _)| (item.clone(), *qty))
        {
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            for (item, got) in taken {
                if got > 0 {
                    inv.add(item, got);
                }
            }
            self.log(format!(
                "Resting needs {} {}, and you're short.",
                qty,
                self.item_name(&item)
            ));
            return;
        }
        self.log("You drop into low-power standby to recharge.");
        for _ in 0..REST_TICKS {
            if self.is_game_over().is_some() {
                return;
            }
            self.tick_inner(false);
            // `nest_aggro_tick` (inside `tick_inner`) can start a battle on
            // any of these ticks now that a provoked swarm chases through
            // rest as it does everywhere else — a swarm catching you here
            // has to fight the party it actually caught, not the party the
            // heal below is about to produce. No refund: like the
            // `is_game_over` bail above, a rest that *started* has already
            // spent its cost.
            if self.has_active_battle() {
                return;
            }
        }
        let player = self.player_entity();
        {
            let mut needs = self.world.get_mut::<Needs>(player).unwrap();
            needs.fatigue = 100.0;
        }
        {
            let mut stats = self.world.get_mut::<Stats>(player).unwrap();
            stats.hp = stats.max_hp;
        }
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
        }
        self.log("You come back online, fully recharged and repaired.");
    }

    /// Stand in place for a single tick — lets the world (wander AI,
    /// cronjob production, needs decay) advance by one step without moving
    /// or taking any other action. Distinct from `rest`, which advances
    /// `REST_TICKS` at once and restores Fatigue.
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
