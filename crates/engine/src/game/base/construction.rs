//! Build requests: the structure the player asked for, the materials a
//! program carries to the spot, and the raising of it.
//!
//! **A deploy is a request now, not an act.** `Game::place_structure` files
//! a `components::BuildSite` on the cell and returns; a body posted by
//! `schedule_base_labour` fetches what the bill of materials calls for,
//! sets it down there, and raises the structure over
//! `tuning::BUILD_TICKS_PER_MATERIAL` ticks per unit. The Home is the one
//! exception and is still stood up by the player's own hand — see
//! `place_structure` for why founding cannot be base labour.
//!
//! **A `&mut Game` pass rather than a bevy system**, called from
//! `tick_inner` beside `Game::run_dig_crew` and for the same reason: a
//! completed build ends in `Game::spawn_structure`, the one place a
//! structure's component list is written, and a system could not reach it
//! without keeping a second copy of that list. `Task` with
//! `TaskKind::Construct` is the posting, and neither `haul_step_system`
//! nor `task_progress_system` can touch it — each resolves its target
//! through a query a build site cannot answer.

use bevy_ecs::prelude::{Entity, With};

use crate::base_grid::BaseGrid;
use crate::components::{
    BuildGoal, BuildSite, Carrying, Inventory, Position, ResourceNode, Stock, Structure,
    StructureTier, Task, TaskKind,
};
use crate::game::base::{hauling, stock};
use crate::items::ItemId;
use crate::structures::StructureDb;
use crate::{Game, tuning};

/// Where a unit of material can be picked up from, and the tile a body has
/// to reach to do it.
///
/// **Two variants because there are two stores, not because there are two
/// kinds of shelf.** Every deployed structure's output buffer is one store
/// — the same set `stock::spend_from_base` drains and the stock strip
/// counts, so a strip reading `BS 12` is a strip saying the base can build
/// — and the player's pack is the other. A Depot has no privilege here: a
/// Mining Node's own shelf is as fetchable as a Depot's, which is what
/// keeps "the base is holding it" one answer rather than two.
#[derive(Clone, Copy)]
enum Source {
    /// A deployed structure's output buffer.
    Shelf(Entity, Position),
    /// The party's own pack, reachable only while the party is standing in
    /// base space — a builder walks over and takes it out of your hands, so
    /// there has to be a pair of hands there to take it from. Four frames
    /// down the Stack the pack is simply not a source, and the site waits.
    Pack(Position),
}

impl Source {
    fn tile(self) -> Position {
        match self {
            Source::Shelf(_, pos) | Source::Pack(pos) => pos,
        }
    }
}

/// Which leg of the job a posted builder is on, derived fresh every tick
/// and never stored.
///
/// `hauling::Errand`'s rule: where a worker is headed and whether it has
/// arrived are both read off `Position` and the load off `Carrying`, so
/// there is no state field that can desync into a body standing on the site
/// insisting it is still walking. Every variant carries owned data for the
/// same reason that one's do — the borrow on the world ends before the walk
/// begins.
enum Errand {
    /// Carrying something the site still wants: walk it there and set it
    /// down.
    Deliver(Position),
    /// The site is short of this item and the body is empty-handed: walk to
    /// the source and take up to a carry's worth.
    Fetch(ItemId, u32, Source),
    /// The site is short of something no shelf and no pack holds.
    Dry,
    /// Everything is in: stand at the site and raise it.
    Raise(Position),
    /// Carrying something the site does not want — a load left over from a
    /// cancelled or already-satisfied line. Put it back rather than let the
    /// units evaporate with the `Task`.
    PutBack,
}

impl Game {
    /// One tick of the build crew: every program posted to a `BuildSite`
    /// takes one step of whichever leg it is on.
    ///
    /// Sorted by tile for `assembler_system`'s reason — bevy's iteration
    /// order is not stable, and two builders drawing from the same shelf in
    /// a different order between runs would make the same base save
    /// differently.
    ///
    /// Runs immediately after `schedule_base_labour`, so a body posted this
    /// tick works this tick, and beside `run_dig_crew` rather than inside
    /// it: the two crews share the walk (`hauling::step_to_post`) and
    /// nothing else.
    pub(crate) fn run_build_crew(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let mut builders: Vec<(i32, i32, Entity)> = {
            let mut query = self.world.query::<(Entity, &Task, &Position)>();
            query
                .iter(&self.world)
                .filter(|(_, task, _)| task.kind == TaskKind::Construct)
                .map(|(e, _, p)| (p.x, p.y, e))
                .collect()
        };
        builders.sort_unstable();
        let blocked = self.structure_tiles();
        let pocket_radius = self.world.resource::<BaseGrid>().radius();
        for (.., worker) in builders {
            self.step_one_builder(worker, &blocked, pocket_radius);
        }
    }

    /// Whether a body posted to `site` right now would have anything to do.
    ///
    /// True when the materials are all in — there is a structure to raise —
    /// or when at least one outstanding item can be picked up somewhere.
    ///
    /// **This is a stock count, and it is the one place a want is allowed to
    /// be one.** `dig_wants` and `feeders_for` are both deliberately
    /// structural, on the grounds that a want flickering as a shelf drains
    /// walks bodies on and off a post for the rest of the run. The reason
    /// this one is different is that the alternative is not flicker, it is a
    /// **deadlock**: build wants outrank production, so a base with one
    /// program posts it to the request, the request is dry, the body stands
    /// there, and the Mining Node that would make the very material the site
    /// is waiting for is never worked again. The crew says "nothing to raise
    /// it with" once and the base is finished for the rest of the run — from
    /// a player doing the supported thing, since filing a request the base
    /// cannot afford is the whole reason filing charges nothing.
    ///
    /// What the flicker actually looks like here is also the behaviour you
    /// want: a lone body mines until a unit exists, carries it to the site,
    /// and goes back to mining. Slow, visible, and progress. A build is a
    /// one-off job with a terminal state, not a post the base holds
    /// indefinitely, which is what makes that acceptable where it would not
    /// be for a machine.
    ///
    /// It does **not** ask whether the whole bill can be met — only whether
    /// there is a next unit to fetch. Waiting for the full bill would put
    /// the deadlock back for any structure costing more than the base can
    /// hold at one time.
    pub(crate) fn build_is_workable(&mut self, site: Entity) -> bool {
        let Some(outstanding) = self
            .world
            .get::<BuildSite>(site)
            .map(|build| build.outstanding())
        else {
            return false;
        };
        if outstanding.is_empty() {
            return true;
        }
        // **A load already in a builder's hands is work in progress**, and
        // the site is not dry while one is walking to it. Without this the
        // base announces "nothing to fetch" the moment a builder empties the
        // last shelf into its arms — naming the *whole* bill as outstanding,
        // because nothing has been set down yet — and then goes quiet, so
        // the one line the player gets is the wrong figure at the wrong
        // moment. The want is dropped in the same breath; the carrier
        // finishes anyway, since `schedule_base_labour` never frees a body
        // holding a `Carrying`, but the report was already wrong.
        let carrying_for_this = {
            let mut query = self.world.query::<(&Task, &Carrying)>();
            query
                .iter(&self.world)
                .any(|(task, _)| task.kind == TaskKind::Construct && task.target == site)
        };
        if carrying_for_this {
            return true;
        }
        outstanding.into_iter().any(|(item, _)| {
            if self.pack_source(&item).is_some() {
                return true;
            }
            let mut query = self.world.query_filtered::<&Stock, With<Structure>>();
            query
                .iter(&self.world)
                .any(|stock| stock.output.get(&item).copied().unwrap_or(0) > 0)
        })
    }

    /// One builder, one step. Split out of the loop above so the site
    /// lookup and the early returns read as the sequence they are rather
    /// than as five levels of `continue`.
    fn step_one_builder(
        &mut self,
        worker: Entity,
        blocked: &std::collections::HashSet<(i32, i32)>,
        pocket_radius: i32,
    ) {
        let Some(site) = self.world.get::<Task>(worker).map(|t| t.target) else {
            return;
        };
        // The site is gone — raised by somebody else, or cancelled out from
        // under this body. Whatever it is still carrying was fetched for a
        // job that no longer exists and has already left the shelf it came
        // off, so it goes back before the post does. Freeing the body with
        // the load still on it is what destroys the goods.
        let Some(target) = self.world.get::<Position>(site).copied() else {
            self.put_back_load(worker);
            self.world.entity_mut(worker).remove::<Task>();
            return;
        };
        if self.world.get::<BuildSite>(site).is_none() {
            self.put_back_load(worker);
            self.world.entity_mut(worker).remove::<Task>();
            return;
        }
        let from = self
            .world
            .get::<Position>(worker)
            .copied()
            .unwrap_or(Position { x: 0, y: 0 });
        match self.builder_errand(worker, site, target) {
            // **Silent here, deliberately.** The scheduler owns this
            // announcement, because it is the only thing that can see a site
            // nobody is posted to — and a dry site is dropped from the want
            // list precisely so it does not hold a body. What reaches this
            // arm is the narrow race where a source existed when the posting
            // was made and was emptied before this body arrived; the next
            // tick re-derives the want and reports it if it lasts.
            Errand::Dry => {}
            Errand::PutBack => {
                self.put_back_load(worker);
            }
            Errand::Deliver(dest)
            | Errand::Raise(dest)
            | Errand::Fetch(_, _, Source::Shelf(_, dest) | Source::Pack(dest))
                if !hauling::at_station(from, dest) =>
            {
                self.walk_builder(worker, from, dest, blocked, pocket_radius);
            }
            Errand::Deliver(_) => self.set_load_down(worker, site),
            // The dry latch is **not** cleared here. Both halves of it live
            // at the one site that decides whether to staff the job —
            // `build_wants` announces the drought and clears the latch the
            // tick a source reappears — because a latch cleared in two
            // places is a latch whose clearing neither place is responsible
            // for, and the redundant one hides a broken primary.
            Errand::Fetch(item, qty, source) => self.pick_up_for_site(worker, &item, qty, source),
            Errand::Raise(_) => self.raise_one_tick(worker, site, target),
        }
    }

    /// What this body should be doing about `site` right now.
    ///
    /// **The load is asked about before the shortfall**, so a body that
    /// fetched five against a shortfall of two finishes delivering what it
    /// is holding before it is sent anywhere else. Asked the other way it
    /// would set off for the next item still holding the last one, and
    /// `Carrying` is a single `(item, qty)` pair.
    fn builder_errand(&mut self, worker: Entity, site: Entity, target: Position) -> Errand {
        let held = self.world.get::<Carrying>(worker).cloned();
        let build = self
            .world
            .get::<BuildSite>(site)
            .expect("checked by the caller");
        let outstanding = build.outstanding();
        if let Some(load) = held {
            let wanted = outstanding.iter().any(|(item, _)| *item == load.item);
            return if wanted {
                Errand::Deliver(target)
            } else {
                Errand::PutBack
            };
        }
        let Some((item, short)) = outstanding.into_iter().next() else {
            return Errand::Raise(target);
        };
        let from = self
            .world
            .get::<Position>(worker)
            .copied()
            .unwrap_or(Position { x: 0, y: 0 });
        match self.nearest_source(&item, from) {
            Some(source) => Errand::Fetch(item, short, source),
            None => Errand::Dry,
        }
    }

    /// The closest place a unit of `item` can be picked up from, measured in
    /// Chebyshev tiles from `from`.
    ///
    /// **Chebyshev rather than `walk_field` path cost**, which is
    /// `hauling::nearest_depot`'s trade exactly: a second field per builder
    /// per tick buys a difference only a wall between two near-equidistant
    /// shelves can produce. Ties break by tile and then by store, so a base
    /// with two shelves holding the same item drains them in the same order
    /// every run — the property `stock::spend_from_base`'s sort exists for,
    /// and the one a reload would otherwise break.
    ///
    /// The pack sorts **last** among equals: a builder that can reach a
    /// shelf takes the shelf, and only walks over to you when the base
    /// itself has run out. Taking from the party's hands is the fallback,
    /// not the habit.
    fn nearest_source(&mut self, item: &ItemId, from: Position) -> Option<Source> {
        let mut candidates: Vec<(i32, i32, i32, u8, Source)> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Position, &Stock), With<Structure>>();
            query
                .iter(&self.world)
                .filter(|(_, _, stock)| stock.output.get(item).copied().unwrap_or(0) > 0)
                .map(|(e, p, _)| (chebyshev(from, *p), p.x, p.y, 0, Source::Shelf(e, *p)))
                .collect()
        };
        if let Some(pack) = self.pack_source(item) {
            let tile = pack.tile();
            candidates.push((chebyshev(from, tile), tile.x, tile.y, 1, pack));
        }
        candidates.sort_by_key(|(d, x, y, store, _)| (*d, *x, *y, *store));
        candidates.into_iter().next().map(|(.., source)| source)
    }

    /// The party's pack as a source, or `None` when it holds none of `item`
    /// or when the party is not standing in base space to be taken from.
    fn pack_source(&self, item: &ItemId) -> Option<Source> {
        let (x, y) = self.base_pos()?;
        let player = self.player_entity();
        let held = self
            .world
            .get::<Inventory>(player)
            .map(|inv| inv.count(item))
            .unwrap_or(0);
        (held > 0).then_some(Source::Pack(Position { x, y }))
    }

    /// One step toward `dest`, or the post given up if there is no route.
    ///
    /// **A builder that loses its route drops the job**, `run_dig_crew`'s
    /// rule: it is what keeps the stall announcement honest, since the
    /// scheduler only ever looks at sites nobody is posted to. What it does
    /// *not* do is drop the load — `put_back_load` runs first, because the
    /// units in a body's hands have already left the shelf.
    fn walk_builder(
        &mut self,
        worker: Entity,
        from: Position,
        dest: Position,
        blocked: &std::collections::HashSet<(i32, i32)>,
        pocket_radius: i32,
    ) {
        let step = {
            let grid = self.world.resource::<BaseGrid>();
            hauling::step_to_post(grid, from, dest, blocked, pocket_radius)
        };
        match step {
            Ok(Some(next)) => {
                if let Some(mut pos) = self.world.get_mut::<Position>(worker) {
                    *pos = next;
                }
            }
            // Nowhere better to stand: the field admits the tile the body is
            // already on and nothing closer. It waits.
            Ok(None) => {}
            Err(_) => {
                self.put_back_load(worker);
                self.world.entity_mut(worker).remove::<Task>();
            }
        }
    }

    /// Takes up to a carry's worth of `item` out of `source`.
    ///
    /// Capped by `tuning::HAUL_CARRY_CAPACITY` — the same cap a hauler's
    /// trip is bounded by, and the reason `Carrying` can be one
    /// `(item, qty)` pair rather than a map — and by what the site is
    /// actually short of, so nothing is carried that would have to be
    /// carried back.
    fn pick_up_for_site(&mut self, worker: Entity, item: &ItemId, short: u32, source: Source) {
        let want = short.min(tuning::HAUL_CARRY_CAPACITY);
        let taken = match source {
            Source::Shelf(shelf, _) => match self.world.get_mut::<Stock>(shelf) {
                Some(mut stock) => hauling::take_from(&mut stock, item, want),
                None => 0,
            },
            Source::Pack(_) => {
                let player = self.player_entity();
                match self.world.get_mut::<Inventory>(player) {
                    Some(mut inv) => inv.take(item.clone(), want),
                    None => 0,
                }
            }
        };
        if taken == 0 {
            // The shelf emptied between the scan and the arrival. Nothing to
            // say: the next tick re-derives the errand and finds the next
            // source, or reports the base dry.
            return;
        }
        self.world.entity_mut(worker).insert(Carrying {
            item: item.clone(),
            qty: taken,
        });
    }

    /// Sets what the body is carrying down on the site, and puts back
    /// whatever the bill of materials had no room for.
    fn set_load_down(&mut self, worker: Entity, site: Entity) {
        let Some(load) = self.world.get::<Carrying>(worker).cloned() else {
            return;
        };
        let landed = match self.world.get_mut::<BuildSite>(site) {
            Some(mut build) => build.deliver(&load.item, load.qty),
            None => 0,
        };
        self.world.entity_mut(worker).remove::<Carrying>();
        let spare = load.qty - landed;
        if spare > 0 {
            self.return_material(&load.item, spare);
        }
    }

    /// Spends what the site was holding and despawns it — the tick the
    /// materials actually leave the run.
    ///
    /// **The one place a build's materials are consumed**, which is
    /// `CLAUDE.md`'s "materials are not spent until the structure is raised"
    /// written as code rather than as a comment. Until this call they stand
    /// on the cell and a cancel gives them back, which is exactly why the
    /// early returns above leave a site alone rather than tidying it away —
    /// folding at the raise's *start* would charge the run for a build that
    /// a missing def or a lowered tier ceiling then declines to finish.
    fn consume_site(&mut self, site: Entity) {
        let delivered = self
            .world
            .get::<BuildSite>(site)
            .map(|b| b.delivered.clone())
            .unwrap_or_default();
        for (item, qty) in delivered {
            let id = item.0.clone();
            self.report_base(
                crate::base_ledger::Event::Consume {
                    item: item.clone(),
                    qty,
                },
                move |tick, zone, _| crate::telemetry::Record::Consume {
                    tick,
                    zone,
                    item: id,
                    qty,
                    source: crate::base_ledger::ConsumeSource::Build
                        .as_str()
                        .to_string(),
                },
            );
        }
        self.world.despawn(site);
    }

    /// One tick of construction, and the structure itself once the meter is
    /// full.
    ///
    /// The materials are **not** spent here: they left their shelves as they
    /// were picked up and have been standing on the cell ever since, which
    /// is what makes a cancelled build refundable and a half-supplied one
    /// visible. Raising the structure just consumes what is already there,
    /// by despawning the site that holds it.
    fn raise_one_tick(&mut self, worker: Entity, site: Entity, target: Position) {
        let done = {
            let Some(mut build) = self.world.get_mut::<BuildSite>(site) else {
                return;
            };
            build.progress += 1;
            build.progress >= build.required_ticks()
        };
        if !done {
            return;
        }
        let Some((kind, goal)) = self
            .world
            .get::<BuildSite>(site)
            .map(|b| (b.structure.clone(), b.goal))
        else {
            return;
        };
        let Some(def) = self.world.resource::<StructureDb>().get(&kind).cloned() else {
            // The structure's `.ron` file was deleted or broke while the
            // request stood. The materials are still on the cell, so the
            // site is left exactly as it is rather than despawned: a mod
            // fixed between two runs finishes the build, and nothing the
            // player paid for is destroyed by a file they can put back.
            return;
        };
        // The one step that differs between the two goals, which is the whole
        // of why `BuildGoal` is a field on one component rather than a second
        // component with its own crew pass.
        match goal {
            BuildGoal::New => {
                self.consume_site(site);
                self.world.entity_mut(worker).remove::<Task>();
                self.spawn_structure(&def, target.x, target.y);
                self.log_base(format!("Your crew finishes the {}.", def.name));
            }
            BuildGoal::Upgrade { to_tier } => {
                // Resolved by **tile**, which is why the site never held an
                // `Entity`: there is nothing to dangle when the machine is
                // destroyed underneath the request.
                let machine = {
                    let mut query = self
                        .world
                        .query_filtered::<(Entity, &Position), With<Structure>>();
                    query
                        .iter(&self.world)
                        .find(|(_, p)| p.x == target.x && p.y == target.y)
                        .map(|(e, _)| e)
                };
                // Left standing rather than despawned where it cannot
                // commit — the missing-def arm's precedent, extended to a
                // machine that is gone and to a tier the ceiling no longer
                // permits. The materials are still on the cell.
                let Some(machine) = machine else {
                    return;
                };
                let permitted = def
                    .upgrade
                    .as_ref()
                    .is_some_and(|upgrade| to_tier <= self.upgrade_ceiling(upgrade));
                if !permitted {
                    return;
                }
                self.world
                    .entity_mut(machine)
                    .insert(StructureTier(to_tier));
                // A node that opted into chance-based yield tracks its tier
                // as its level; one that always succeeds (level None) stays
                // that way.
                if let Some(mut node) = self.world.get_mut::<ResourceNode>(machine)
                    && node.level.is_some()
                {
                    node.level = Some(to_tier);
                }
                self.consume_site(site);
                self.world.entity_mut(worker).remove::<Task>();
                self.log_base(format!(
                    "Your crew finishes upgrading the {} to Mk{to_tier}.",
                    def.name
                ));
            }
        }
    }

    /// Puts whatever `worker` is carrying back into the base, and takes the
    /// `Carrying` off it either way.
    fn put_back_load(&mut self, worker: Entity) {
        let Some(load) = self.world.get::<Carrying>(worker).cloned() else {
            return;
        };
        self.world.entity_mut(worker).remove::<Carrying>();
        self.return_material(&load.item, load.qty);
    }

    /// Puts `qty` of `item` back where the base can reach it: a Depot
    /// first, the party's pack second.
    ///
    /// **Depots and not any shelf**, which is deliberately narrower than
    /// where the units were *taken* from: a unit pushed into a machine's
    /// output buffer reads as something that machine produced, and would be
    /// hauled off and counted as a cycle's yield. The asymmetry is the same
    /// one the dig crew's substrate draw already carries.
    ///
    /// Anything that fits nowhere is **logged rather than dropped in
    /// silence**. It is reachable — a base with no Depot, the party away and
    /// a build cancelled — and a player who is not told simply sees the
    /// stock strip fall.
    pub(crate) fn return_material(&mut self, item: &ItemId, qty: u32) {
        let landed = stock::return_to_depots(self, item, qty);
        let mut spare = qty - landed;
        if spare > 0 && self.base_pos().is_some() {
            let player = self.player_entity();
            if let Some(mut inv) = self.world.get_mut::<Inventory>(player) {
                inv.add(item.clone(), spare);
                spare = 0;
            }
        }
        if spare > 0 {
            let name = self.item_name(item);
            self.log_base(format!(
                "There is nowhere to put {spare} {name} down — your crew leaves it in the dust."
            ));
        }
    }
}

fn chebyshev(a: Position, b: Position) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}
