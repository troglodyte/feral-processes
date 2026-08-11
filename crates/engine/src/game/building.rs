//! Placing, upgrading, and demolishing structures, and assigning programs
//! to work them.

use crate::game::hauling;
use crate::structures::UpgradeDef;
use crate::tuning::{MAX_BUILD_DISTANCE_FROM_HOME, STRUCTURE_REMOVAL_REFUND_PERCENT};
use crate::*;

impl Game {
    pub fn place_structure(&mut self, structure_id: &str, dx: i32, dy: i32) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't deploy right now.".into());
        }
        self.require_surface()?;
        let def = self
            .world
            .resource::<StructureDb>()
            .get(structure_id)
            .cloned()
            .ok_or_else(|| "Unknown structure".to_string())?;
        if !self.structure_unlocked(structure_id) {
            return Err(format!("{} hasn't been researched yet.", def.name));
        }
        if structure_id != HOME_STRUCTURE_ID && !self.has_structure(HOME_STRUCTURE_ID) {
            return Err("Deploy a Home first before building anything else.".into());
        }
        if structure_id == HOME_STRUCTURE_ID && self.has_structure(HOME_STRUCTURE_ID) {
            return Err("A Home is already deployed. Remove it before building another.".into());
        }
        let player = self.player_entity();
        let ppos = *self.world.get::<Position>(player).unwrap();
        let (x, y) = (ppos.x + dx, ppos.y + dy);

        if structure_id != HOME_STRUCTURE_ID {
            let home = self.home_position().expect("checked above: a Home exists");
            if !Platform::covers(x - home.x, y - home.y) {
                return Err(format!(
                    "Too far from Home — structures must be built within {MAX_BUILD_DISTANCE_FROM_HOME} tiles of it."
                ));
            }
        }

        // A link and a structure on one tile makes walking onto it
        // ambiguous — `move_player` checks the link first, so the structure
        // would become unbumpable and you'd descend every time you tried to
        // reach it.
        if self.find_surface_link_at(x, y).is_some() {
            return Err("There's a link here — deploy clear of it.".into());
        }

        let walkable = self.world.resource_mut::<WorldMap>().tile(x, y).walkable;
        if !walkable {
            return Err("Can't deploy onto that terrain.".into());
        }
        if self.find_blocking_structure_at(x, y).is_some() {
            return Err("Something is already deployed there.".into());
        }
        let build_cost = self.structure_build_cost(&def);
        // Every shortfall at once, and each with its numbers: the build menu
        // is off screen by the time this fires, so one item per attempt would
        // make finding out what a structure needs a matter of walking back and
        // forth. Logged as well as returned because the status line carrying
        // the refusal ages out in `STATUS_LINE_SECONDS`, and this is the one
        // build refusal that leaves the player an errand.
        let short: Vec<String> = {
            let inv = self.world.get::<Inventory>(player).unwrap();
            build_cost
                .iter()
                .filter(|(item, qty)| inv.count(item) < *qty)
                .map(|(item, qty)| {
                    format!("{qty} {} (have {})", self.item_name(item), inv.count(item))
                })
                .collect()
        };
        if !short.is_empty() {
            let msg = format!(
                "Not enough materials to deploy the {} — needs {}.",
                def.name,
                short.join(", ")
            );
            self.log_base(msg.clone());
            return Err(msg);
        }
        {
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            for (item, qty) in &build_cost {
                inv.take(item.clone(), *qty);
            }
        }

        let mut entity = self.world.spawn((
            Structure {
                kind: def.id.clone(),
            },
            Position { x, y },
            Glyph {
                ch: def.glyph,
                color: def.color,
            },
        ));
        if def.raidable {
            entity.insert(Durability {
                hp: def.durability,
                max_hp: def.durability,
            });
        }
        entity.insert(Stock::new(def.capacity));
        if def.runs_a_job() {
            entity.insert(MachineStatus::default());
        }
        if let Some(work) = &def.work {
            entity.insert(ResourceNode {
                resource: work.produces.clone(),
                level: work.level,
            });
        }
        if let Some(temp) = &def.temporary {
            entity.insert(Temporary {
                ticks_remaining: temp.max_ticks,
            });
        }
        if def.upgrade.is_some() {
            entity.insert(StructureTier(1));
        }
        if def.id == HOME_STRUCTURE_ID {
            self.stamp_platform(x, y);
        }
        self.log_base(format!("You deploy a {}.", def.name));
        self.tick();
        Ok(())
    }

    /// The highest tier a structure with this `upgrade` path can currently
    /// reach. Two ceilings, and the lower wins: the def's own `max_tier`,
    /// which is permanent, and the zone the player has breached to, which is
    /// not — reaching zone *N* is what unlocks Mk*N*, so nothing upgrades at
    /// all before the first breach.
    ///
    /// That mirrors gear, where reaching zone *N* unlocks level *N* gear
    /// (`tuning::GEAR_LEVEL_GROWTH`, enforced in `Game::equip`), and the two
    /// ladders line up: every shipped upgradeable structure caps at 5, the
    /// same span `ZoneLevel::stat_multiplier`'s curve is pinned over.
    ///
    /// This is a function rather than a `min` inlined into
    /// `upgrade_structure` because `Game::view_entities` needs the same
    /// answer, to label an upgrade-menu row with the tier it cannot reach
    /// yet. A menu that computed its own ceiling would drift from the one
    /// that does the refusing.
    pub(crate) fn upgrade_ceiling(&self, upgrade: &UpgradeDef) -> u32 {
        upgrade.max_tier.min(self.world.resource::<ZoneLevel>().0)
    }

    /// `upgrade_ceiling` for a deployed structure, paired with the def's
    /// permanent `max_tier` and resolving the def on the way. `None` when
    /// `entity` is not a structure, or when its def declares no upgrade
    /// path — which pairs with `StructureTier` being absent, so
    /// `EntityView`'s `tier`, `ceiling` and `max_tier` are `Some` together.
    pub(crate) fn entity_upgrade_ceiling(&self, entity: Entity) -> Option<(u32, u32)> {
        let kind = &self.world.get::<Structure>(entity)?.kind;
        let def = self.world.resource::<StructureDb>().get(kind)?;
        let upgrade = def.upgrade.as_ref()?;
        Some((self.upgrade_ceiling(upgrade), upgrade.max_tier))
    }

    /// Advances `structure` one upgrade tier, charging its `UpgradeDef`
    /// cost scaled by the tier being reached. The new tier both multiplies
    /// the structure's work payout (see `systems::task_progress_system`)
    /// and becomes its `ResourceNode::level`, so extraction gets more
    /// reliable as well as more productive — reusing the existing
    /// `mining_success_chance` curve rather than adding a second one.
    pub fn upgrade_structure(&mut self, structure: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        self.require_surface()?;
        let Some(kind) = self
            .world
            .get::<Structure>(structure)
            .map(|s| s.kind.clone())
        else {
            return Err("That structure is already gone.".into());
        };
        let def = self
            .world
            .resource::<StructureDb>()
            .get(&kind)
            .cloned()
            .ok_or_else(|| "Unknown structure".to_string())?;
        let Some(upgrade) = def.upgrade else {
            return Err(format!("{} can't be upgraded.", def.name));
        };
        let tier = self
            .world
            .get::<StructureTier>(structure)
            .map(|t| t.0)
            .unwrap_or(1);
        if tier >= upgrade.max_tier {
            return Err(format!("{} is already fully upgraded.", def.name));
        }
        // Checked after the permanent ceiling, so a maxed-out structure in a
        // shallow zone reads as finished rather than as waiting on a breach
        // it would never benefit from.
        if tier >= self.upgrade_ceiling(&upgrade) {
            return Err(format!(
                "{} can't go past Mk{tier} until you breach to zone {}.",
                def.name,
                tier + 1
            ));
        }
        let next = tier + 1;
        let cost: Vec<(ItemId, u32)> = upgrade
            .cost
            .iter()
            .map(|(item, qty)| (item.clone(), qty * next))
            .collect();

        let player = self.player_entity();
        {
            let inv = self.world.get::<Inventory>(player).unwrap();
            for (item, qty) in &cost {
                if inv.count(item) < *qty {
                    return Err(format!("Not enough {}.", self.item_name(item)));
                }
            }
        }
        {
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            for (item, qty) in &cost {
                inv.take(item.clone(), *qty);
            }
        }

        self.world.entity_mut(structure).insert(StructureTier(next));
        // A node that opted into chance-based yield tracks its tier as its
        // level; one that always succeeds (level None) stays that way.
        if let Some(mut node) = self.world.get_mut::<ResourceNode>(structure)
            && node.level.is_some()
        {
            node.level = Some(next);
        }
        self.log_base(format!("You upgrade the {} to Mk{next}.", def.name));
        self.tick();
        Ok(())
    }

    /// Demolishes `structure`, refunding `STRUCTURE_REMOVAL_REFUND_PERCENT`
    /// of its current build cost. Removing the Home is a special case: it
    /// cascades to demolish every other structure along with it (each
    /// refunding its own share the same way), since nothing else can exist
    /// outside a Home's `MAX_BUILD_DISTANCE_FROM_HOME` radius anyway.
    /// Frontends are expected to warn the player about that cascade before
    /// calling this for a Home — this method itself performs the removal
    /// unconditionally, with no confirmation step of its own.
    pub fn remove_structure(&mut self, structure: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        self.require_surface()?;
        let kind = self
            .world
            .get::<Structure>(structure)
            .ok_or_else(|| "That structure is already gone.".to_string())?
            .kind
            .clone();
        let is_home = kind == HOME_STRUCTURE_ID;
        let removed_name = self
            .world
            .resource::<StructureDb>()
            .get(&kind)
            .map(|d| d.name.clone())
            .unwrap_or(kind.clone());

        let mut targets = vec![structure];
        if is_home {
            let mut query = self.world.query::<(Entity, &Structure)>();
            targets.extend(
                query
                    .iter(&self.world)
                    .filter(|(e, s)| *e != structure && s.kind != HOME_STRUCTURE_ID)
                    .map(|(e, _)| e),
            );
        }
        let removed_count = targets.len();

        let mut refund: Vec<(ItemId, u32)> = Vec::new();
        for &target in &targets {
            let Some(target_kind) = self.world.get::<Structure>(target).map(|s| s.kind.clone())
            else {
                continue;
            };
            let Some(def) = self
                .world
                .resource::<StructureDb>()
                .get(&target_kind)
                .cloned()
            else {
                continue;
            };
            for (item, qty) in self.structure_build_cost(&def) {
                let share = qty * STRUCTURE_REMOVAL_REFUND_PERCENT / 100;
                if share == 0 {
                    continue;
                }
                match refund.iter_mut().find(|(i, _)| *i == item) {
                    Some((_, total)) => *total += share,
                    None => refund.push((item, share)),
                }
            }
            let workers: Vec<Entity> = {
                let mut tasks = self.world.query::<(Entity, &Task)>();
                tasks
                    .iter(&self.world)
                    .filter(|(_, t)| t.target == target)
                    .map(|(w, _)| w)
                    .collect()
            };
            for worker in workers {
                // `Carrying` goes with the `Task`: a worker whose machine is
                // gone has nowhere to put its load down and would hold it for
                // the rest of the run. `damage_structure` carries the same
                // pair for the same reason.
                self.world.entity_mut(worker).remove::<(Task, Carrying)>();
            }
            self.announce_lost_shelf(target);
            self.world.despawn(target);
        }
        if is_home {
            self.clear_platform();
        }

        // Route through `grant_loot`, not a direct `add`: demolishing a Home
        // cascades every other structure's refund in one shot, easily
        // enough to blow past the buffer with no message.
        for (item, qty) in &refund {
            self.grant_loot(item.clone(), *qty);
        }
        let refund_note = if refund.is_empty() {
            String::new()
        } else {
            let parts: Vec<String> = refund
                .iter()
                .map(|(item, qty)| format!("{qty} {}", self.item_name(item)))
                .collect();
            format!(" You recover {}.", parts.join(", "))
        };
        if is_home && removed_count > 1 {
            self.log_base_kind(
                MessageKind::Loot,
                format!(
                    "You demolish the Home — without it, {} other base structure{} collapse{}.{refund_note}",
                    removed_count - 1,
                    if removed_count - 1 == 1 { "" } else { "s" },
                    if removed_count - 1 == 1 { "s" } else { "" },
                ),
            );
        } else {
            self.log_base_kind(
                MessageKind::Loot,
                format!("You demolish the {removed_name}.{refund_note}"),
            );
        }
        self.tick();
        Ok(())
    }

    /// How many ticks one gather cycle against `structure` takes, from its
    /// def's `work` recipe. Shared by `assign_cronjob` and `work_structure`
    /// so a program and the player grind at the same rate.
    fn work_ticks_for(&mut self, structure: Entity) -> u32 {
        let kind = self.world.get::<Structure>(structure).unwrap().kind.clone();
        let db = self.world.resource::<StructureDb>();
        let Some(def) = db.get(&kind) else {
            return 5;
        };
        match (&def.work, &def.assembles) {
            (Some(work), _) => work.ticks_per_unit,
            (None, Some(assembles)) => assembles.ticks_per_unit,
            (None, None) => 5,
        }
    }

    /// Whether a program can be posted to `structure` — an extractor
    /// (`ResourceNode`) or an assembler (`StructureDef::assembles`). Named
    /// once because the cronjob menu and the assignment itself have to agree:
    /// a structure the menu offers and the assignment refuses is a dead end,
    /// and one the menu hides but the assignment would take is unreachable.
    pub(crate) fn accepts_a_program(&self, structure: Entity) -> bool {
        // `ResourceNode` first so a hand-spawned test node with no def in the
        // db still counts; otherwise the def is the authority.
        if self.world.get::<ResourceNode>(structure).is_some() {
            return true;
        }
        self.world
            .get::<Structure>(structure)
            .and_then(|s| self.world.resource::<StructureDb>().get(&s.kind))
            .is_some_and(|d| d.runs_a_job())
    }

    /// Works `structure` yourself instead of posting a program to it — the
    /// same `Task` a cronjob worker carries, on the player, advanced by
    /// `systems::player_gather_system` and paying out through the same
    /// `resolve_gather_cycle`.
    ///
    /// There is no separate "working" mode: the job simply runs while the
    /// world ticks, which is what makes stepping away end it (`move_player`
    /// drops the `Task`). It is deliberately not persisted — `PlayerSave`
    /// carries no task, so loading a save puts you next to the node rather
    /// than mid-cycle at it, and that costs at most one cycle's progress
    /// without a save-format bump.
    ///
    /// You have to be standing on one of the node's four station tiles, by
    /// the same `hauling::at_station` a posted program has to walk to. The
    /// cycle pays into the node's *own* buffer (`systems::
    /// player_gather_system`) and `c` reaches only those four tiles
    /// (`collect_adjacent`), so a job run from anywhere else fills a buffer
    /// the player cannot open — silently, since the extraction lines read
    /// the same either way. `move_player` drops the `Task` on any step, so
    /// checking once at the start is what makes "a player still holding one
    /// is standing beside the node" true for the whole job rather than an
    /// assumption those two functions were making.
    ///
    /// A refusal rather than a filtered menu, for the reason `assign_cronjob`
    /// refuses an unwalkable post: the picker lists everything within
    /// `MENU_SCAN_RADIUS`, and a row that vanished by distance would take the
    /// whole screen — and the base menu row leading to it — with it.
    pub fn work_structure(&mut self, structure: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        self.require_surface()?;
        if self.world.get::<ResourceNode>(structure).is_none() {
            return Err("That structure can't be worked.".into());
        }
        let here = *self
            .world
            .get::<Position>(self.player_entity())
            .ok_or_else(|| "You aren't anywhere you can work from.".to_string())?;
        let structure_pos = *self
            .world
            .get::<Position>(structure)
            .ok_or_else(|| "That structure isn't anywhere you can reach.".to_string())?;
        if !hauling::at_station(here, structure_pos) {
            return Err(
                "You have to be standing next to it to work it — get beside it first.".into(),
            );
        }
        let ticks = self.work_ticks_for(structure);
        let player = self.player_entity();
        self.world.entity_mut(player).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 0,
            required: ticks,
        });
        self.log_base("You set to work. Moving off breaks your concentration.");
        self.tick();
        Ok(())
    }

    /// Stands down whoever currently holds `kind` on `structure`, so no
    /// structure ever has two of the same job running on it. Returns the
    /// displaced entity, if there was one.
    ///
    /// The two kinds are counted separately: a worked structure is still
    /// worth guarding, and a guard produces nothing, so they never compete.
    ///
    /// Occupancy is read off the `Task` components rather than cached on the
    /// structure. `Task` is already the only record of who works what, and
    /// eight sites remove one (raid damage, demolition, sale, breach, party
    /// recall, fusion, rest, zone change) — a cached field would have to be
    /// kept in step with every one of them, which is the shape of bug
    /// `announce_lost_shelf` already exists to work around.
    fn displace_task_holder(&mut self, structure: Entity, kind: TaskKind) -> Option<Entity> {
        let holder = {
            let mut tasks = self.world.query::<(Entity, &Task)>();
            tasks
                .iter(&self.world)
                .find(|(_, t)| t.target == structure && t.kind == kind)
                .map(|(e, _)| e)
        }?;
        self.world.entity_mut(holder).remove::<Task>();
        if holder == self.player_entity() {
            self.log_base("You break off what you were doing.");
        } else {
            let name = self.creature_label(holder);
            self.log_base(format!("{name} stands down."));
        }
        Some(holder)
    }

    pub fn assign_cronjob(&mut self, worker: Entity, structure: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        self.require_surface()?;
        let owner = self
            .world
            .get::<Tamed>(worker)
            .ok_or_else(|| "That program isn't compiled under your control.".to_string())?
            .owner;
        if owner != self.player_entity() {
            return Err("You don't control that program.".into());
        }
        if !self.accepts_a_program(structure) {
            return Err("That structure can't be worked.".into());
        }
        // The program sets off from wherever *you* are standing, because that
        // is where it has been all along. A tamed program's `Position` is the
        // tile it was beaten on and is never written again — `views.rs`
        // states it, and `render/base.rs` refuses to draw a companion for
        // exactly that reason. Posting is the moment the tile starts meaning
        // something (`haul_step_system` walks it from here on), so it is also
        // the moment to make it true. Measuring the walk from the stale tile
        // instead would strand a program tamed further out than
        // `HAUL_WALK_RADIUS`, which is the bug this replaced.
        let from = *self
            .world
            .get::<Position>(self.player_entity())
            .ok_or_else(|| "You aren't anywhere you can post a program from.".to_string())?;
        let structure_pos = *self
            .world
            .get::<Position>(structure)
            .ok_or_else(|| "That structure isn't anywhere you can post to.".to_string())?;
        // Checked before anything is spent, for the reason `install_routine`
        // checks knowledge before taking the disk: accepting a post the
        // program can never walk to would stand a companion down and displace
        // the structure's current worker to pay for a cronjob that produces
        // nothing.
        let mut map = self.world.resource_mut::<WorldMap>();
        if !hauling::can_reach_post(&mut map, from, structure_pos) {
            return Err(
                "That structure is too far away to post a program to — get closer \
                 to it first."
                    .into(),
            );
        }
        if let Some(mut pos) = self.world.get_mut::<Position>(worker) {
            *pos = from;
        }
        let ticks = self.work_ticks_for(structure);
        if self.world.resource::<Party>().0.contains(&worker) {
            self.world
                .resource_mut::<Party>()
                .0
                .retain(|&e| e != worker);
            self.log_base("It stands down as your companion to run this cronjob.");
        }
        self.displace_task_holder(structure, TaskKind::GatherResource);
        self.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 0,
            required: ticks,
        });
        self.log_base("Cronjob scheduled.");
        self.tick();
        Ok(())
    }

    /// Posts `worker` (a tamed program you own) to guard `structure`
    /// against raids (see `raid_check`), without assigning it a cronjob.
    /// Unlike `assign_cronjob`, this works on any structure — including
    /// ones with no `work` recipe at all, like a Terminal — since defending
    /// doesn't require producing anything. A structure that's already
    /// cronjob-worked is already defended by its worker; this is for posting
    /// a guard on structures that otherwise have no defender. A structure
    /// raids can't target at all (`StructureDef::raidable`, e.g. Home) is
    /// refused, since a guard there would wait forever for a raid that never
    /// comes.
    pub fn assign_guard(&mut self, worker: Entity, structure: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        self.require_surface()?;
        let owner = self
            .world
            .get::<Tamed>(worker)
            .ok_or_else(|| "That program isn't compiled under your control.".to_string())?
            .owner;
        if owner != self.player_entity() {
            return Err("You don't control that program.".into());
        }
        let kind = self
            .world
            .get::<Structure>(structure)
            .ok_or_else(|| "That's not a structure.".to_string())?
            .kind
            .clone();
        let unraidable = self
            .world
            .resource::<StructureDb>()
            .get(&kind)
            .filter(|def| !def.raidable)
            .map(|def| def.name.clone());
        if let Some(name) = unraidable {
            return Err(format!("{name} can't be raided — it doesn't need a guard."));
        }
        if self.world.resource::<Party>().0.contains(&worker) {
            self.world
                .resource_mut::<Party>()
                .0
                .retain(|&e| e != worker);
            self.log_base("It stands down as your companion to guard this structure.");
        }
        self.displace_task_holder(structure, TaskKind::Guard);
        self.world.entity_mut(worker).insert(Task {
            kind: TaskKind::Guard,
            target: structure,
            progress: 0,
            required: 0,
        });
        self.log_base("It takes up a defensive position.");
        self.tick();
        Ok(())
    }
}
