//! The turn loop: advancing the clock, moving, and the actions a player
//! spends a turn on.

use crate::*;

impl Game {
    pub(crate) fn player_entity(&self) -> Entity {
        self.world.resource::<PlayerEntity>().0
    }

    pub(crate) fn log(&mut self, s: impl Into<String>) {
        self.world.resource_mut::<MessageLog>().push(s);
    }

    pub(crate) fn log_kind(&mut self, kind: MessageKind, s: impl Into<String>) {
        self.world.resource_mut::<MessageLog>().push_kind(kind, s);
    }

    pub fn message_log(&self, n: usize) -> Vec<(MessageKind, String)> {
        self.world.resource::<MessageLog>().recent(n).to_vec()
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
        self.schedule.run(&mut self.world);
        self.structure_regen();
        self.raid_check();
        self.nest_respawn_tick();
        if age_temporary {
            self.age_temporary_structures();
        }
        self.world.resource_mut::<GameClock>().tick += 1;
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
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let player = self.player_entity();
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
        }
        self.tick();
    }

    /// Consume one unit of `id` out of battle, applying its `ConsumeDef`:
    /// restore Power/Fatigue/Integrity (each clamped) and/or arm a pre-battle
    /// combat buff (see `use_item`'s `prebattle_buff`, applied at the next
    /// intrusion). A non-consumable or an empty stack is a logged no-op.
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
        if let Some(buff) = effect.prebattle_buff {
            self.world.get_mut::<CombatBuff>(player).unwrap().active = Some(ActiveBuff {
                kind: buff.kind,
                remaining: buff.rounds,
                power: buff.power,
            });
        }
        let name = self.item_name(id).to_string();
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
    /// the shipped structures — and there's no other way to rest. Beyond
    /// that gate, there's no separate "rest" system beyond replaying the
    /// normal tick loop plus a Fatigue/HP reset at the end (via
    /// `tick_inner(false)`, so these ticks don't age the rest structure
    /// itself — see `age_temporary_structures`). If Power runs out and you
    /// take lethal damage mid-rest, the loop bails out via the
    /// `is_game_over` check before either restore happens.
    pub fn rest(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let player_pos = *self.world.get::<Position>(self.player_entity()).unwrap();
        if self.nearby_rest_structure(player_pos).is_none() {
            self.log("You need to be within your base, near Home, to power down and rest.");
            return;
        }
        self.log("You drop into low-power standby to recharge.");
        for _ in 0..REST_TICKS {
            if self.is_game_over().is_some() {
                return;
            }
            self.tick_inner(false);
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

    /// Awards unsolicited income — a scan find, battle loot, a boss cache —
    /// clamped to whatever room is left, returning how many units landed.
    /// Income clamps rather than refusing so a full buffer can never stall
    /// a battle from resolving or a cronjob worker from running; the loss
    /// is logged so it is never silent.
    pub(crate) fn grant_loot(&mut self, item: ItemId, qty: u32) -> u32 {
        let player = self.player_entity();
        let added = self
            .world
            .resource_scope(|world, db: bevy_ecs::prelude::Mut<ItemDb>| {
                world
                    .get_mut::<Inventory>(player)
                    .unwrap()
                    .add_capped(item.clone(), qty, &db)
            });
        if added < qty {
            let lost = qty - added;
            let label = if self.bank_limit_of(&item).is_some() {
                "Research bank"
            } else {
                "Buffer"
            };
            let name = self.item_name(&item).to_string();
            self.log(format!("{label} full — {lost} {name} lost."));
        }
        added
    }

    /// Scan the current sector for salvageable Core Fragments. Chance
    /// depends on the sector's biome; besides starting inventory and combat
    /// drops, this and structure cronjobs are the only ways to replenish
    /// Core Fragments — the raw material Power Cells and ICE Breakers are
    /// compiled from (see `craft_recipes`).
    pub fn forage(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let player = self.player_entity();
        let pos = *self.world.get::<Position>(player).unwrap();
        let biome = self
            .world
            .resource_mut::<WorldMap>()
            .tile(pos.x, pos.y)
            .biome;
        let chance = forage_chance(biome, self.player_perk_level(Perk::KeenScavenger));
        let found = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(chance)
        };
        if found {
            if self.grant_loot(self.currency(), 1) > 0 {
                self.log_kind(
                    MessageKind::Loot,
                    "You scan the sector and recover a core fragment.",
                );
            }
        } else {
            self.log("You scan the sector but find nothing salvageable.");
        }
        self.tick();
    }
}

/// `Game::forage`'s success chance for `biome`, boosted by
/// `KEEN_SCAVENGER_BONUS_PER_LEVEL` for every level of `keen_scavenger_level`
/// (capped at 1.0) — pulled out of the method so the formula is
/// unit-testable without an RNG.
pub(crate) fn forage_chance(biome: Biome, keen_scavenger_level: u32) -> f64 {
    let chance = match biome {
        Biome::Mainframe | Biome::OpenGrid => 0.6,
        Biome::NullSector => 0.3,
        Biome::StaticField => 0.15,
        // A base platform is a manufactured floor, not terrain — there's
        // nothing on it to scavenge. Keeping it at 0.0 also stops a base
        // from being a risk-free forage spot, which would undercut the
        // whole reason to leave the platform.
        Biome::DataVoid | Biome::BlackIce | Biome::Platform => 0.0,
    };
    if chance > 0.0 && keen_scavenger_level > 0 {
        (chance + KEEN_SCAVENGER_BONUS_PER_LEVEL * keen_scavenger_level as f64).min(1.0)
    } else {
        chance
    }
}
