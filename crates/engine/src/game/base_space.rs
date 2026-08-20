//! Walking in and out of base space through the anchor, and walking around
//! once inside.
//!
//! The counterpart to `game/stack.rs` for the other off-surface locale, and
//! it plays the same trick for the same reason: the party's base-space
//! coordinates live on `resources::Locale`, and the player's `Position`
//! component stays pinned to the anchor tile on the zone surface. See
//! `resources::Locale` for why that is the load-bearing decision — nothing
//! on the surface has to know base space exists.
//!
//! What that costs is a guard on anything that reaches the zone map through
//! `Position`, and `enter_base` is itself one of those things: it asks
//! whether the player is standing on the anchor. Underground the pinned
//! `Position` *is* the entrance tile, and a run that dived from its own
//! starting tile dived from the anchor — so that question answers yes four
//! frames down, and `Game::require_surface` has to be asked first.

use crate::base_grid::BaseGrid;
use crate::resources::Locale;
use crate::*;

/// The one cell of base space the anchor's door opens onto, and the only
/// cell you can leave from — base space's own origin, where the Home
/// stands.
///
/// Here rather than in `tuning.rs`: it is not a knob, it is the origin the
/// whole coordinate space is defined against, and a base space whose exit
/// was somewhere else would be a different space rather than a retuned one.
pub(crate) const BASE_EXIT_CELL: (i32, i32) = (0, 0);

impl Game {
    /// Steps through the anchor and out of phase, landing on
    /// `BASE_EXIT_CELL`.
    ///
    /// Refused unless the party is on the zone surface proper, standing on
    /// the anchor tile, with a Home deployed. That last one is not a
    /// formality: a new run has no base at all, because
    /// `Game::place_structure` refuses every structure until a Home exists
    /// and the Home is player-built. Until one is up, base space is solid
    /// everywhere and the anchor opens onto nothing — so it says so, in
    /// wording that shares nothing with the "you are not on the anchor"
    /// refusal, because those are two different things for a player to fix.
    ///
    /// The player's `Position` is not written here, unlike `enter_stack`'s
    /// pin to the entrance tile. It does not need to be: the guard above is
    /// that the player is *already* standing on the anchor, and nothing in
    /// base space moves `Position` — `move_player` dispatches to
    /// `move_in_base`, which only ever rewrites the locale. The pin is a
    /// property of the two of them together rather than an assignment that
    /// would be a no-op the day it was written.
    pub fn enter_base(&mut self) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Not right now.".into());
        }
        self.require_surface()?;
        let anchor = self
            .anchor_position()
            .ok_or_else(|| "There's no anchor in this sector.".to_string())?;
        let standing = *self
            .world
            .get::<Position>(self.player_entity())
            .ok_or_else(|| "You aren't anywhere you can phase out from.".to_string())?;
        if (standing.x, standing.y) != anchor {
            return Err("No anchor here — you phase out from the anchor tile.".into());
        }
        if !self.has_structure(HOME_STRUCTURE_ID) {
            return Err(
                "The anchor is dark. There's nothing on the other side until you deploy a Home."
                    .into(),
            );
        }

        let (x, y) = BASE_EXIT_CELL;
        self.world.insert_resource(Locale::Base { x, y });
        self.log("You step into the anchor and phase out of the sector.");
        self.tick();
        Ok(())
    }

    /// Steps back out through the anchor onto the tile it stands on.
    ///
    /// Refused anywhere but `BASE_EXIT_CELL` — base space has one door, not
    /// one per wall. Nothing has to be restored on the way out beyond the
    /// locale: unlike `Game::clear_stack`, which also drops the generated
    /// frame and the Trace built up inside it, base space keeps no
    /// per-visit state at all. `BaseGrid` is the player's own dug ground and
    /// outlives every visit, including a breach into the next zone.
    pub fn leave_base(&mut self) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Not right now.".into());
        }
        self.require_base()?;
        let standing = self
            .base_pos()
            .expect("require_base passed, so the party is in base space");
        if standing != BASE_EXIT_CELL {
            return Err("The way out is back at the Home — walk to it first.".into());
        }

        self.world.insert_resource(Locale::Surface);
        self.log("You phase back in, standing on the anchor.");
        self.tick();
        Ok(())
    }

    /// Whether the run has a base at all — a Home standing.
    ///
    /// The one thing a frontend has to be able to ask about the base without
    /// being in it, and it exists because of what the answer gates: the
    /// anchor refuses entry without a Home, and deploying one is the single
    /// build permitted from the open grid (`Game::place_structure`). A build
    /// menu that hid its Deploy row off the base would hide the only way
    /// onto it, so the row asks this.
    ///
    /// `Game::in_base` is where the party is; this is whether there is a
    /// base for them to be in. The two are independent — you can stand in
    /// base space having just demolished the Home out from under yourself.
    pub fn has_home(&self) -> bool {
        self.has_structure(HOME_STRUCTURE_ID)
    }

    /// Lays the pocket the run opens with: `BaseCell::Floor` over the
    /// chamfered box of `STARTING_POCKET_RADIUS` around base space's own
    /// origin.
    ///
    /// Called from the one site `Game::stamp_platform` used to be called
    /// from — deploying the first Home — and it is a one-for-one
    /// replacement for it, which is what keeps the opening playing as it
    /// did. The shape is the slab's, `PLATFORM_CORNER_CUT` and all, so the
    /// base still reads as rounded rather than as a stamped square and the
    /// buildable cell count is unchanged.
    ///
    /// **It writes no `WorldMap` tile.** That is the point of the whole
    /// relocation: `Biome::Platform` stops being stamped into the zone
    /// surface, and the base's footprint becomes `BaseGrid::is_floor` and
    /// nothing else.
    ///
    /// Idempotent, because `lay_floor` overwrites: a second call re-floors
    /// cells that are already floored and takes nothing away. Nothing calls
    /// it twice today — the first Home is the only caller and there is only
    /// ever one — but a pocket that could be *shrunk* by re-laying it would
    /// be a much worse thing to have to reason about later.
    pub(crate) fn lay_starting_pocket(&mut self) {
        let r = crate::tuning::STARTING_POCKET_RADIUS;
        let cut = crate::tuning::PLATFORM_CORNER_CUT;
        let mut grid = self.world.resource_mut::<BaseGrid>();
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() + dy.abs() <= 2 * r - cut {
                    grid.lay_floor(dx, dy);
                }
            }
        }
    }

    /// The `DigSite` standing on base-space `(x, y)`, if the player has
    /// started on that wall or marked it.
    ///
    /// **Its `Position` is in base space**, so this must never be reached
    /// for with a zone-surface coordinate — see `components::DigSite`.
    pub(crate) fn dig_site_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<DigSite>>();
        query
            .iter(&self.world)
            .find(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
    }

    /// One swing at the solid base-space cell `(x, y)`, spawning the
    /// `DigSite` that records the wall's progress if this is the first.
    ///
    /// The shape is `Game::attack_nest`'s, down to sharing its damage
    /// through `Game::swing_damage`: rock is a thing you wear down by
    /// bumping into it, it cannot dodge, and identical swings land identical
    /// damage. What it does *not* share is a `Durability` sized from an
    /// asset — `tuning::BASE_ROCK_DURABILITY` is the same for every cell in
    /// every zone at every depth, so the thing that changes as a run goes on
    /// is the player's swing.
    ///
    /// Opening the cell despawns the site **unless it is marked**: a mark
    /// outlives the cut, because marked solid means cut it and marked `Open`
    /// means floor it (slice 2, phase B).
    ///
    /// **`swinger` is whoever is hitting it**, and the same function serves
    /// the player's bump and a posted digger's cycle (`Game::run_dig_crew`)
    /// — one place where rock takes damage, one place where a cell opens,
    /// one fragment roll. What the swinger decides is the damage, through
    /// its own `Game::swing_damage`, and the voice: a crew's swings are
    /// silent and only the break is base news, because a line per swing per
    /// digger is the log for the rest of the run.
    pub(crate) fn strike_rock(&mut self, swinger: Entity, x: i32, y: i32) {
        let by_player = swinger == self.player_entity();
        let dmg = self.swing_damage(swinger);
        let site = self.dig_site_at(x, y).unwrap_or_else(|| {
            self.world
                .spawn((
                    DigSite::default(),
                    Durability {
                        hp: crate::tuning::BASE_ROCK_DURABILITY,
                        max_hp: crate::tuning::BASE_ROCK_DURABILITY,
                    },
                    Position { x, y },
                ))
                .id()
        });
        let Some(mut durability) = self.world.get_mut::<Durability>(site) else {
            return;
        };
        // A site standing on solid rock with nothing left to cut is a cell
        // entropy took back: it was opened, never floored, and the wall
        // re-knit under a mark that outlived it. The wall is whole again, so
        // it costs the swings a whole wall costs — without this the next
        // swing lands on a spent `Durability` and opens it for free, which
        // is the promise `BASE_ENTROPY_REFILL_TICKS` makes read backwards.
        if durability.hp == 0 {
            durability.hp = durability.max_hp;
        }
        durability.hp = durability.hp.saturating_sub(dmg);
        if durability.hp > 0 {
            if by_player {
                self.log(format!("You cut into the entropy for {dmg} damage."));
            }
            return;
        }

        // Read before the cell is opened rather than after: the swing lands
        // on this tick, and `base_entropy_system` measures its window from
        // it.
        let tick = self.world.resource::<GameClock>().tick;
        self.world.resource_mut::<BaseGrid>().open(x, y, tick);
        let marked = self.world.get::<DigSite>(site).is_some_and(|d| d.marked);
        if !marked {
            self.world.despawn(site);
        }
        if by_player {
            self.log("The entropy gives way, and the cell opens.");
        } else {
            self.log_base("Your crew cuts through, and the cell opens.");
        }

        // A live action, not world generation, so `GameRng` is the right
        // stream to draw from: nothing here has to be reproduced by a
        // reload.
        let paid = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0
                .random_bool(crate::tuning::BASE_MINE_FRAGMENT_CHANCE.clamp(0.0, 1.0) as f64)
        };
        if paid {
            let item = ItemId::from(crate::items::ids::CORE_FRAGMENT);
            let landed = self.grant_loot(item.clone(), 1);
            if landed > 0 {
                let line = format!("A {} shakes loose from the cut.", self.item_name(&item));
                if by_player {
                    self.log_kind(MessageKind::Loot, line);
                } else {
                    self.log_base_kind(MessageKind::Loot, line);
                }
            }
        }
    }

    /// Marks — or clears — every cell in the inclusive box spanned by `a`
    /// and `b`.
    ///
    /// **The anchor decides which of the two it does**: a box whose `a` cell
    /// is already marked clears, an unmarked one marks. That is the whole of
    /// why there is no second erase verb — settled decision 4 — and it is
    /// also why the anchor is read *before* anything in the box is written.
    ///
    /// The box is normalised rather than assumed ordered: a plan drawn
    /// up-left is the same plan drawn down-right, and a cursor the player
    /// dragged backwards is the ordinary case rather than the corner one.
    ///
    /// A `Floor` cell takes no mark. There is nothing left to do to it, and a
    /// site spawned over one would be a mark the crew could never clear.
    pub fn toggle_mark_box(&mut self, a: (i32, i32), b: (i32, i32)) {
        let marking = !self.is_marked(a.0, a.1);
        let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
        let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.set_mark(x, y, marking);
            }
        }
    }

    /// Every marked cell, in `(x, y)` order.
    ///
    /// Sorted rather than handed back in query order for the reason `Stock`
    /// keys by `BTreeMap`: bevy's iteration order is not stable, and the
    /// renderer draws these in the order it gets them.
    pub fn marked_cells(&mut self) -> Vec<(i32, i32)> {
        let mut query = self.world.query::<(&DigSite, &Position)>();
        let mut cells: Vec<(i32, i32)> = query
            .iter(&self.world)
            .filter(|(d, _)| d.marked)
            .map(|(_, p)| (p.x, p.y))
            .collect();
        cells.sort_unstable();
        cells
    }

    /// Whether base-space `(x, y)` is marked. A cell with no `DigSite` is
    /// not, which is what makes an untouched wall the unmarked case without
    /// storing anything for it.
    fn is_marked(&mut self, x: i32, y: i32) -> bool {
        self.dig_site_at(x, y)
            .and_then(|site| self.world.get::<DigSite>(site))
            .is_some_and(|d| d.marked)
    }

    /// Writes one cell's mark, spawning or retiring its `DigSite` as needed.
    ///
    /// The durability a *marked* site is born with is the cell's own state:
    /// a solid cell has the whole wall left to cut, an already-open one has
    /// none — its mark means floor it. Clearing retires the site unless it is
    /// still holding chip progress, so an unmarked wall the player had
    /// started on does not heal.
    fn set_mark(&mut self, x: i32, y: i32, marked: bool) {
        let grid = self.world.resource::<BaseGrid>();
        if grid.is_floor(x, y) {
            return;
        }
        let solid = grid.is_solid(x, y);
        match self.dig_site_at(x, y) {
            Some(site) => {
                if let Some(mut dig) = self.world.get_mut::<DigSite>(site) {
                    dig.marked = marked;
                }
                // An unmarked site earns its keep by holding chip progress,
                // and *both* ends of the meter hold none. A full meter is a
                // wall nobody has touched; a spent one on a cell that is
                // still solid is what entropy leaves behind when it reverts
                // a marked `Open` cell, and `strike_rock` refills it on the
                // next swing anyway. Keeping either leaves an entity drawn
                // nowhere, wanted by nobody, and written to every save from
                // then on.
                let holds_progress = self
                    .world
                    .get::<Durability>(site)
                    .is_some_and(|d| d.hp > 0 && d.hp < d.max_hp);
                let keep = marked || (solid && holds_progress);
                if !keep {
                    self.world.despawn(site);
                }
            }
            None if marked => {
                let max_hp = crate::tuning::BASE_ROCK_DURABILITY;
                self.world.spawn((
                    DigSite {
                        marked: true,
                        announced_stuck: false,
                    },
                    Durability {
                        hp: if solid { max_hp } else { 0 },
                        max_hp,
                    },
                    Position { x, y },
                ));
            }
            None => {}
        }
    }

    /// Lays floor over base-space `(x, y)` and retires the cell's `DigSite`
    /// with it.
    ///
    /// The one place a base-space cell becomes `Floor` in play — `lay_tile`
    /// today, the crew's flooring job next — because a finished cell has
    /// nothing left to record and a mark that outlived the tile would be a
    /// job no one could ever complete. `lay_starting_pocket` writes the grid
    /// directly instead: it runs before anything can have dug.
    pub(crate) fn floor_cell(&mut self, x: i32, y: i32) {
        self.world.resource_mut::<BaseGrid>().lay_floor(x, y);
        if let Some(site) = self.dig_site_at(x, y) {
            self.world.despawn(site);
        }
    }

    /// One tick of the dig crew: every program posted to a `DigSite` walks
    /// to it, cuts it, and floors what the cut opened.
    ///
    /// **A `&mut Game` method called from `tick_inner`, not a bevy system**,
    /// and for a stronger version of the reason `schedule_base_labour` is
    /// one: a cycle here ends in `Game::strike_rock` or `Game::floor_cell`,
    /// which are *the* place rock takes damage and *the* place a base cell
    /// becomes floor. A system could reach neither, and would have had to
    /// keep a second copy of the swing damage, the fragment roll, the
    /// substrate spend and the cell write — four formulas with one door
    /// each, which is exactly the drift `balance_sim` has already been
    /// bitten by four times. It runs immediately after
    /// `schedule_base_labour`, so a body posted this tick digs this tick.
    ///
    /// The walk is `hauling::step_to_post` — the same walk a posted worker
    /// takes to a machine, not a second one — because `haul_step_system`
    /// cannot move a digger: it resolves every destination through a
    /// `Structure` query, and a dig site is the one `Task` target that is
    /// not a structure.
    ///
    /// **A digger that loses its route drops the job** rather than standing
    /// in the dark forever. That is what keeps the stall announcement
    /// honest: `schedule_base_labour` is the only thing that announces one,
    /// and it only ever looks at sites nobody is posted to.
    pub(crate) fn run_dig_crew(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        // Sorted by tile for the reason `assembler_system` sorts its
        // machines: bevy's iteration order is not stable, and two diggers
        // whose swings land in a different order between runs would make the
        // same base save differently.
        let mut diggers: Vec<(i32, i32, Entity)> = {
            let mut query = self.world.query::<(Entity, &Task, &Position)>();
            query
                .iter(&self.world)
                .filter(|(_, task, _)| task.kind == TaskKind::Excavate)
                .map(|(e, _, p)| (p.x, p.y, e))
                .collect()
        };
        diggers.sort_unstable();
        let blocked = self.structure_tiles();
        let pocket_radius = self.world.resource::<BaseGrid>().radius();
        for (.., worker) in diggers {
            let Some(site) = self.world.get::<Task>(worker).map(|t| t.target) else {
                continue;
            };
            // The site is gone — cut and floored by somebody else — or its
            // mark was cleared while this body was working. Either way there
            // is nothing left to dig here, and the scheduler will find it
            // something else.
            //
            // The cleared mark is the half that has to be said separately:
            // `set_mark` deliberately *keeps* an unmarked site that still
            // holds chip progress, so a cancelled plan does not look like a
            // despawned entity. `dig_wants` stops listing it, but
            // `schedule_base_labour` never takes a body off a post it has
            // nowhere better to send — so nothing upstream frees this one.
            let marked = self.world.get::<DigSite>(site).is_some_and(|d| d.marked);
            let Some(target) = self.world.get::<Position>(site).copied().filter(|_| marked) else {
                self.world.entity_mut(worker).remove::<Task>();
                continue;
            };
            let from = self
                .world
                .get::<Position>(worker)
                .copied()
                .unwrap_or(Position { x: 0, y: 0 });
            if !crate::game::base::hauling::at_station(from, target) {
                let step = {
                    let grid = self.world.resource::<BaseGrid>();
                    crate::game::base::hauling::step_to_post(
                        grid,
                        from,
                        target,
                        &blocked,
                        pocket_radius,
                    )
                };
                match step {
                    Ok(Some(next)) => {
                        if let Some(mut pos) = self.world.get_mut::<Position>(worker) {
                            *pos = next;
                        }
                    }
                    // Nowhere better to stand: the field admits the tile the
                    // worker is already on and nothing closer. It waits.
                    Ok(None) => {}
                    Err(_) => {
                        self.world.entity_mut(worker).remove::<Task>();
                    }
                }
                continue;
            }
            let Some(mut task) = self.world.get_mut::<Task>(worker) else {
                continue;
            };
            task.progress += 1;
            if task.progress < task.required {
                continue;
            }
            task.progress = 0;
            // Which of the two halves of the one verb this is, decided by the
            // cell rather than by anything stored on the job: marked solid
            // means cut it, marked `Open` means floor it. A cut cell is still
            // this body's job on the next cycle, because the mark outlives
            // the cut.
            if self
                .world
                .resource::<BaseGrid>()
                .is_solid(target.x, target.y)
            {
                self.strike_rock(worker, target.x, target.y);
            } else {
                self.crew_lays_tile(target.x, target.y);
            }
        }
    }

    /// The crew's half of `Game::lay_tile`: one Blank Substrate out of the
    /// same store every build is paid from, and the cell is floor.
    ///
    /// **Silent when the store is empty.** The mark stays, the body stays
    /// posted, and the tile goes down the moment a substrate exists — the
    /// only thing missing is stock, which the player is already being told
    /// about by every other thing that wants it. The route stall is
    /// announced because nothing else in the base reports it.
    fn crew_lays_tile(&mut self, x: i32, y: i32) {
        let substrate = ItemId::from(crate::items::ids::BLANK_SUBSTRATE);
        let player = self.player_entity();
        let held = self
            .world
            .get::<Inventory>(player)
            .map(|inv| inv.count(&substrate))
            .unwrap_or(0);
        if held == 0 {
            return;
        }
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(substrate, 1);
        self.floor_cell(x, y);
        self.log_base("Your crew lays a VectorStasis Tile, and the cell reads as floor.");
    }

    /// One step through base space, reached from `Game::move_player` — the
    /// same four keys, dispatched on locale.
    ///
    /// Reads `BaseGrid::walkable` for whether the step lands, and then the
    /// one thing a step can do besides land: walk onto a Portal and breach.
    /// `WorldMap` is a different coordinate space entirely and has no say
    /// here, and solid rock is simply not walkable — there is no way to end
    /// up standing inside it, so base space needs no analogue of the Stack's
    /// `die_in_the_rock`.
    ///
    /// Nothing *blocks*. A structure standing on a cell is walked over, not
    /// bumped into, unlike the zone surface — and it has to be, because the
    /// Home stands on `BASE_EXIT_CELL` and a blocked exit cell is a base
    /// with no way out of it.
    ///
    /// **A step into solid rock is a swing**, and costs the turn the step
    /// would have — see `Game::strike_rock`. Slice 1 refused it for free, on
    /// the grounds that base space is walls until the player cuts them and
    /// brushing against your own corners should not be taxed; slice 2 makes
    /// cutting them the point, so the wall is a thing you attack instead.
    ///
    /// **It still breaks off a posted job**, and that matches the surface:
    /// `move_player` drops the job before it looks at what is in the way, on
    /// the grounds that either way you stopped working to do it, and
    /// `Game::work_structure` promises the player as much when it posts.
    /// The drop sits above every branch below rather than inside one, which
    /// is why turning the refusal into a swing did not quietly stop it
    /// happening.
    pub(crate) fn move_in_base(&mut self, dx: i32, dy: i32) {
        let Some((x, y)) = self.base_pos() else {
            return;
        };
        // Before the wall check, exactly as `move_player` does it.
        self.break_off_job();
        let (nx, ny) = (x + dx, y + dy);
        // Solid rock is hit, not bumped into — the branch sits exactly where
        // `move_player` holds its nest branch, and for the same reason: the
        // wall is a thing you attack, and a swing costs the turn a step
        // would have. There is no new key and no direction prompt.
        if self.world.resource::<BaseGrid>().is_solid(nx, ny) {
            let player = self.player_entity();
            self.strike_rock(player, nx, ny);
            self.tick();
            return;
        }
        // Still the one statement of what a step lands on. Every cell state
        // `BaseGrid` has today is walkable, so nothing reaches this return —
        // it is what a fifth `BaseCell` variant would meet, rather than a
        // branch play can take.
        if !self.world.resource::<BaseGrid>().walkable(nx, ny) {
            return;
        }
        // The one structure a step in here does anything with. A Portal is a
        // `Structure` and so stands in base space with the rest of them, and
        // walking onto it is how a run breaches — the surface branch of
        // `move_player` used to hold this check, back when the base was on
        // the surface, and asking it out there now answers about a tile in
        // another coordinate space entirely.
        //
        // Consumed before `enter_next_zone` runs, exactly as it was: a
        // portal that travelled would make every breach after the first
        // free, bypassing its per-zone cost.
        if let Some(portal) = self.find_zone_portal_at(nx, ny) {
            self.world.despawn(portal);
            self.enter_next_zone();
            self.tick();
            return;
        }
        self.world.insert_resource(Locale::Base { x: nx, y: ny });
        self.tick();
    }
}
