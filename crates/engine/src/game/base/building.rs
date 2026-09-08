//! Placing, upgrading, and demolishing structures, and assigning programs
//! to work them.

use crate::base_grid::BaseGrid;
use crate::game::base::hauling;
use crate::structures::UpgradeDef;
use crate::tuning::STRUCTURE_REMOVAL_REFUND_PERCENT;
use crate::*;

impl Game {
    /// Files a request to raise `structure_id` on the cell `(dx, dy)` from
    /// the party, spending `program` — a tamed program you own — to pay for
    /// it. The founding Home is the exception at both ends: it is stood up
    /// on the spot rather than filed, and it costs no program.
    ///
    /// **`program` is a trailing parameter rather than a second door.** A
    /// `place_structure_with_program` beside this one would be two ways into
    /// one rule, and the one nobody remembers to update is the one that
    /// raises a structure for free. Every caller states what it is spending,
    /// and `None` is a statement too — `structure_needs_program` is what
    /// decides whether it is a legal one.
    pub fn place_structure(
        &mut self,
        structure_id: &str,
        dx: i32,
        dy: i32,
        program: Option<Entity>,
    ) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't deploy right now.".into());
        }
        let def = self
            .world
            .resource::<StructureDb>()
            .get(structure_id)
            .cloned()
            .ok_or_else(|| "Unknown structure".to_string())?;
        if !self.structure_unlocked(structure_id) {
            return Err(format!("{} hasn't been researched yet.", def.name));
        }
        // Both before the locale guard below, deliberately. A player on the
        // open grid with no base yet who picks a machine out of the build
        // menu is told the thing they can act on — deploy a Home — rather
        // than being told they are in the wrong place for a build that would
        // be refused wherever they stood.
        if structure_id != HOME_STRUCTURE_ID && !self.has_structure(HOME_STRUCTURE_ID) {
            return Err("Deploy a Home first before building anything else.".into());
        }
        if structure_id == HOME_STRUCTURE_ID && self.has_structure(HOME_STRUCTURE_ID) {
            return Err("A Home is already deployed. Remove it before building another.".into());
        }

        // Where a deploy is made *from*, and the run's first Home is the one
        // exception in the game.
        //
        // Every other build is a base action: it stands a structure in base
        // space, beside the ones already there, measured from the cell the
        // party is standing in. The first Home cannot be, because there is
        // no base to stand in yet — base space is solid everywhere and the
        // anchor refuses entry for want of a Home, so a run that had to be
        // inside to deploy one could never start a base at all. Founding is
        // therefore an open-grid act.
        //
        // **The two halves of it move independently, and that is the whole
        // of the placement rule.** The Home stands on base space's own
        // origin whatever direction was pointed — the pocket is laid around
        // that origin, and a Home somewhere else in it would make
        // `BASE_EXIT_CELL` a door onto bare floor. The *anchor* is a zone
        // fixture and goes where the party is standing, so the base opens on
        // ground the player chose rather than at the sector's arrival point.
        let founding_door = match structure_id == HOME_STRUCTURE_ID {
            true => {
                self.require_surface()?;
                let standing = *self
                    .world
                    .get::<Position>(self.player_entity())
                    .ok_or_else(|| "You aren't anywhere you can deploy from.".to_string())?;
                // A link is walked *onto* to descend (`Game::move_player`),
                // so an anchor sharing a link's tile could never be stepped
                // on to be entered — the step that would reach it drops the
                // party into the Stack instead. Refused up here with
                // `require_surface` rather than beside the materials check:
                // it is a question about where the door goes, and the door
                // is decided in this block.
                if self.find_surface_link_at(standing.x, standing.y).is_some() {
                    return Err(
                        "You're standing on a Stack link — an anchor can't share its tile. \
                         Step off it first."
                            .into(),
                    );
                }
                Some((standing.x, standing.y))
            }
            false => None,
        };
        let founding = founding_door.is_some();
        let (x, y) = if founding {
            crate::game::base_space::BASE_EXIT_CELL
        } else {
            self.require_base()?;
            let (px, py) = self
                .base_pos()
                .expect("require_base passed, so the party is in base space");
            (px + dx, py + dy)
        };

        // Where a build may go, and it is one rule now rather than two: a
        // structure stands on laid floor. There used to be a second, opposite
        // one for a `claims_ground` build, which existed precisely to put
        // ground where the slab was not — the slab is gone, and laying floor
        // is slice 2's own action rather than a side effect of a deploy.
        //
        // The founding Home skips the check: the pocket it lays does not
        // exist to be measured against yet.
        if !founding && !self.world.resource::<BaseGrid>().is_floor(x, y) {
            return Err("There's no floor there — a structure has to stand on laid ground.".into());
        }

        if self.find_blocking_structure_at(x, y).is_some() {
            return Err("Something is already deployed there.".into());
        }
        // A cell already spoken for by a request nobody has raised yet. A
        // refusal of its own rather than folded into the one above, because
        // the two leave the player different errands: one cell needs
        // demolishing, the other needs the crew to catch up — or the request
        // calling off.
        if self.build_site_at(x, y).is_some() {
            return Err("Your crew is already set to build something there.".into());
        }
        // Before the materials check, with the other refusals: a structure
        // whose effect accumulates is bounded by a count rather than by
        // whatever downstream constant its effect happens to clamp against,
        // because that constant is not a limit a player ever meets.
        if def.max_deployed > 0 {
            // Requests count against the ceiling alongside the structures
            // already standing. Counting only what is built lets a player
            // queue ten of a max-one machine, and the tenth would be refused
            // by nothing: `spawn_structure` performs no checks, because by
            // the time a crew finishes a request those were answered when it
            // was filed. This is where they are answered.
            let standing = self.count_structures(&def.id) + self.count_build_requests(&def.id);
            if standing >= def.max_deployed {
                return Err(format!(
                    "You already have {standing} {}{} — that's as many as this grid will hold.",
                    def.name,
                    if standing == 1 { "" } else { "s" }
                ));
            }
        }

        // A structure that widened the slab used to claim a ring of ground
        // on the zone surface, burying whatever stood in it — which is why
        // there was a refusal here for the ring that would take the sector's
        // last Stack link. Nothing a build does touches the zone surface any
        // more, so there is no ring, nothing to bury, and no refusal to
        // make. The two structures that carried `build_radius_bonus` (the
        // Heaps) were deleted along with the field itself, and
        // `resources::Platform` — the field's one remaining reader — retires
        // in the same task.
        let build_cost = self.structure_build_cost(&def);

        // **The program is committed here, and here is deliberate at both
        // ends.**
        //
        // *Below* every refusal about the world — researched, floored,
        // unoccupied, under `max_deployed` — because a player standing in
        // the wrong place must be told about the place, not about a program
        // they were never going to spend.
        //
        // *Above* the founding branch rather than beside the site spawn
        // below it, so `structure_needs_program` is what exempts the Home
        // rather than an early `return` that happens to sit in front of the
        // check. The two positions are the same for every other structure —
        // nothing between here and the spawn runs for them — and this one
        // is the one where the exemption is a rule instead of an accident.
        //
        // Resolved in the engine and not at the picker: the picker is a
        // frontend, and a rule only a frontend enforces is one a second
        // frontend skips.
        //
        // The label is read **before** the call, because a successful commit
        // despawns the body and there is nothing left to ask for a name
        // afterwards.
        let offered_label = program.map(|p| self.creature_label(p));
        let committed = self.commit_for_build(program, &def, BuildGoal::New)?;
        // Only names a program that was actually spent: an exempt structure
        // may be handed one and still commit nothing.
        let committed_who = offered_label.filter(|_| committed.is_some());

        // **The Home is stood up by hand; everything else is a request.**
        // Founding is the one build with nobody to ask: base space does not
        // exist yet, so there is no roster standing in it and no shelf to
        // fetch from. It therefore keeps the original shape entirely — every
        // material out of the player's own pack, refused on the spot when
        // they are short, and the structure standing on the tile before the
        // call returns.
        if founding {
            let short: Vec<String> = {
                let player = self.player_entity();
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
                let player = self.player_entity();
                let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
                for (item, qty) in &build_cost {
                    inv.take(item.clone(), *qty);
                }
            }
            self.spawn_structure(&def, x, y);
            // The Home is what opens base space, and it opens it exactly
            // where it stands: the pocket is laid around the origin the Home
            // was just put on.
            self.lay_starting_pocket();
            let (ax, ay) = founding_door.expect("founding, so the door tile was resolved above");
            self.move_anchor_to(ax, ay);
            self.log_base(format!(
                "You deploy a {}. The anchor settles where you stand.",
                def.name
            ));
            // Fired unconditionally: `Repeat::OnceEver` is what makes it
            // once-only, and a second `if first_time` here would put the
            // policy in two places.
            self.notify(crate::notifications::NotificationKind::BaseFounding);
            self.tick();
            return Ok(());
        }

        // **Everything else is filed, not built.** The materials are not
        // checked and not spent here: a request the base cannot afford yet
        // is a legitimate thing to file — production catches up, and the
        // crew starts carrying the moment the last unit exists. What would
        // be lost by refusing is the whole point of a queue.
        //
        // The cost is resolved **now** and carried on the site, so a
        // zone-portal request filed in one zone is not silently repriced by
        // breaching into the next while the crew is already hauling to it.
        //
        // The committed program rides the site from the moment it is
        // spawned rather than being inserted a line later: a site that
        // exists for even one statement without the program it was paid for
        // is a site a panic or an early return could leave standing for
        // free.
        let mut site = BuildSite::new(def.id.clone(), build_cost);
        site.program = committed;
        self.world.spawn((
            site,
            Position { x, y },
            // A glyph, unlike a `DigSite` — which is what puts a build site
            // on the map and under the examine ray for free, through
            // `view_entities` and `find_target_in_direction` rather than a
            // second draw path. The renderer paints its own frame around
            // this; the character is what `x` reads and what a text-mode
            // fallback would draw.
            Glyph {
                ch: BUILD_SITE_GLYPH,
                color: GlyphColor::Orange,
            },
        ));
        // Two sentences rather than one with an optional clause: the only
        // order that commits nothing is the Home, which never reaches this
        // line, so the `None` arm exists for a modded exempt structure and
        // says the truth about it rather than naming a program that was
        // never spent.
        self.log_base(match committed_who {
            Some(who) => format!(
                "You mark out a {} here, and commit {who} to it. Your crew will raise it.",
                def.name
            ),
            None => format!("You mark out a {} here. Your crew will raise it.", def.name),
        });
        self.tick();
        Ok(())
    }

    /// The whole program cost of a build order, answered once for both
    /// filing doors: whether one is owed at all, whether the one offered is
    /// deep enough, whether the base can spare it, and the commit itself.
    ///
    /// **One copy, not one per door.** `place_structure` and
    /// `upgrade_structure` ask the same four questions in the same order and
    /// differ only in the tier they demand and the noun they put in the
    /// sentence. A second copy of the ladder beside the first is how a
    /// deploy and an upgrade come to disagree about what a program costs —
    /// `count_build_requests`' argument about a `&self` twin, applied to the
    /// cost instead of the count.
    ///
    /// **The order inside the ladder is the contract.** Missing first, so a
    /// player who passed nothing is told to pick one rather than being told
    /// a depth about a program they do not have; then depth, so a player who
    /// passed something too shallow is told how deep to go; then the
    /// last-program rule, which is about the base rather than about the
    /// program offered and would otherwise mask both. The commit is last
    /// because it is the only step that moves anything: every refusal above
    /// it leaves the roster exactly as it found it.
    ///
    /// **`Ok(None)` is not a refusal.** It means this structure owes no
    /// program at all — the Home, and whatever a mod exempts alongside it —
    /// and the caller files the order without one. A refusal is an `Err`.
    ///
    /// `>=` and never `==` on the depth, `programs_for_build`'s rule: a run
    /// whose roster has outgrown zone 1 must still be able to raise a Mk1.
    fn commit_for_build(
        &mut self,
        program: Option<Entity>,
        def: &StructureDef,
        goal: BuildGoal,
    ) -> Result<Option<save::CreatureSave>, String> {
        if !self.structure_needs_program(&def.id) {
            return Ok(None);
        }
        let tier = crate::game::catalog::program_tier_required(goal);
        // What the order will produce, in the player's own words, so all
        // four sentences below name the same thing. A deploy is a machine; an
        // upgrade is a mark of one.
        let subject = match goal {
            BuildGoal::New => def.name.clone(),
            BuildGoal::Upgrade { to_tier } => format!("Mk{to_tier} {}", def.name),
        };
        let Some(chosen) = program else {
            return Err(match goal {
                BuildGoal::New => format!(
                    "Deploying a {} costs a tamed program. Pick one first.",
                    def.name
                ),
                BuildGoal::Upgrade { to_tier } => format!(
                    "Upgrading the {} to Mk{to_tier} costs a tamed program. Pick one first.",
                    def.name
                ),
            });
        };
        let depth = self.zone_tier(chosen);
        if depth < tier {
            return Err(format!(
                "That program is from zone {depth}. A {subject} needs one from zone {tier} \
                 or deeper."
            ));
        }
        // **Defence in depth, not the gate.** `programs_for_build` folds
        // this same predicate in, so a frontend that draws its list can no
        // longer offer the last program at all and a player should never
        // reach this arm through the picker. It stays because this function
        // is the only thing standing between *any* caller — a test, a script,
        // a future screen that assembles a commit some other way — and a
        // base spending itself to zero, and because its sentence names the
        // structure the order was for, which a menu-level line cannot.
        //
        // See `Game::build_would_empty_the_roster` for why zero is the line
        // and why the rule reads the whole roster rather than the eligible
        // list.
        if self.build_would_empty_the_roster() {
            return Err(match goal {
                BuildGoal::New => format!(
                    "Committing your last program would leave nobody to build the {}.",
                    def.name
                ),
                BuildGoal::Upgrade { .. } => format!(
                    "Committing your last program would leave nobody to upgrade the {}.",
                    def.name
                ),
            });
        }
        // **`None` here is a refusal, not an absence.** `commit_program`
        // returns it for a program that is wild, away on a sortie, downed or
        // carrying a load, and nothing has moved when it does. Filing the
        // order anyway would stand a structure up for nothing, and no
        // compiler would say so — which is why this is a `?` and not an
        // `unwrap_or(None)`.
        let Some(snapshot) = self.commit_program(chosen) else {
            return Err(
                "That program can't be committed to a build — it's out on a sortie, downed, \
                 or carrying a load."
                    .into(),
            );
        };
        Ok(Some(snapshot))
    }

    /// Spawns the finished structure `def` on base-space `(x, y)` and
    /// returns it.
    ///
    /// **The one place a structure's component list is written**, and it has
    /// two callers with nothing else in common: the player founding a Home
    /// by hand, and the build crew finishing a request. Left inline in
    /// `place_structure` the list would have had to be copied into
    /// `run_build_crew`, and nothing would fail to compile when the two
    /// drifted — a crew-built machine quietly missing its `MachineStatus`
    /// reads as the base being broken, not as a missing line. This is
    /// `Game::roster_parts`' argument applied to the other roster.
    ///
    /// It performs no checks at all. Every refusal — researched, floored,
    /// unoccupied, under `max_deployed` — belongs to whoever decided to
    /// build, and by the time a crew finishes a request those were answered
    /// when it was filed.
    pub(crate) fn spawn_structure(&mut self, def: &StructureDef, x: i32, y: i32) -> Entity {
        // The freebie is spent **here**, where a structure actually stands,
        // rather than at the deploy that asked for one. Both callers claim
        // it identically for free that way, and a request the player files
        // and then cancels — or one wiped with the cell it stood on — costs
        // the run nothing, which is what "the first one you *build*" means.
        if def.first_free {
            self.world
                .resource_mut::<crate::resources::FreeBuilds>()
                .0
                .insert(def.id.clone());
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
        // A burning supplier runs no job and would carry no status, so the
        // `Starved` it reports when the base runs out of Power Cells would
        // have nowhere to live — see `structures::StructureDef::power_upkeep`.
        if def.runs_a_job() || def.power_upkeep.is_some() {
            entity.insert(MachineStatus::default());
        }
        if def.power_upkeep.is_some() {
            entity.insert(crate::components::PowerFuel {
                ticks_left: crate::tuning::POWER_UPKEEP_TICKS,
            });
        }
        if let Some(work) = &def.work {
            entity.insert(ResourceNode {
                resource: work.produces.clone(),
                level: work.level,
            });
        }
        if def.strips.is_some() {
            entity.insert(crate::components::Hopper::default());
        }
        if let Some(temp) = &def.temporary {
            entity.insert(Temporary {
                ticks_remaining: temp.max_ticks,
            });
        }
        if def.upgrade.is_some() {
            entity.insert(StructureTier(1));
        }
        entity.id()
    }

    /// Posts `worker` to raise `site`, standing it down from the party
    /// first if it was fighting beside you.
    ///
    /// `post_digger`'s shape, and like it writes **no `Position`** — a
    /// posted program sets off from its own tile, and the player's is read
    /// nowhere in the scheduler.
    ///
    /// `Task::required` is set but never read: `run_build_crew` prices a
    /// tick of construction off `BuildSite::required_ticks`, which is
    /// derived from the bill of materials the site carries, so the meter
    /// lives on the site and survives the body being reassigned. The field
    /// is filled with that same figure rather than left at zero so a
    /// `Task` read by anything generic reports something true.
    pub(crate) fn post_builder(&mut self, worker: Entity, site: Entity) {
        if self.world.resource::<Party>().0.contains(&worker) {
            self.world
                .resource_mut::<Party>()
                .0
                .retain(|&e| e != worker);
            self.log_base("It stands down as your companion to raise the new structure.");
        }
        self.displace_task_holder(site, TaskKind::Construct);
        let required = self
            .world
            .get::<BuildSite>(site)
            .map(|b| b.required_ticks())
            .unwrap_or(1);
        self.world.entity_mut(worker).insert(Task {
            kind: TaskKind::Construct,
            target: site,
            progress: 0,
            required,
        });
    }

    /// Cancels the build request at `site`, returning everything already
    /// carried there.
    ///
    /// **Nothing is destroyed by changing your mind.** The delivered units
    /// left their shelves when a builder picked them up and have been
    /// standing on the cell ever since, so a cancel is a refund of goods
    /// that still exist rather than a rebate on goods that were consumed —
    /// which is exactly why `run_build_crew` does not spend the materials
    /// until the structure is raised. They go back through the same
    /// `return_material` a stray load does: Depots first, the pack second.
    ///
    /// **The program the order was paid with comes back too**, on the same
    /// terms and through the same `return_build_holdings` the cell-wipe door
    /// uses. Without that a deploy the player thinks better of would destroy
    /// a program outright — a rob rather than a refund, and the reason
    /// `commit_program` takes a snapshot before it despawns anything.
    ///
    /// The posted builder's `Task` is left alone rather than cleared here.
    /// `run_build_crew` finds the site gone on the next tick, puts back
    /// whatever it was still carrying and gives the post up itself — one
    /// place that knows how to unwind a builder, not two.
    pub fn cancel_build_request(&mut self, site: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        self.require_base()?;
        let Some(build) = self.world.get::<BuildSite>(site).cloned() else {
            return Err("That build request is already gone.".into());
        };
        let name = self.structure_name(&build.structure);
        self.return_build_holdings(&build);
        self.world.despawn(site);
        self.log_base(format!("You call off the {name}."));
        self.tick();
        Ok(())
    }

    /// Lays a VectorStasis Tile over the carved cell the party is standing
    /// on, turning it into buildable floor for one Blank Substrate.
    ///
    /// The substrate is raw stock; the tile is what it becomes underfoot.
    /// `BaseCell::Floor` stays the code's name for the result — this is the
    /// player's word for it, the same way "GC Entropy Sweep" is the player's
    /// word for a raid — and no new item exists for it.
    ///
    /// Paid out of the player's own `Inventory`, because that is where
    /// `place_structure` pays every build cost from: one store, not two.
    /// **Refusals first, and each one distinct**: nothing to lay and nothing
    /// to lay it on leave the player different errands, so they may not read
    /// as the same refusal.
    pub fn lay_tile(&mut self) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Not right now.".into());
        }
        self.require_base()?;
        let (x, y) = self
            .base_pos()
            .expect("require_base passed, so the party is in base space");
        match self.world.resource::<BaseGrid>().cell(x, y) {
            Some(base_grid::BaseCell::Open { .. }) => {}
            Some(base_grid::BaseCell::Floor) => {
                return Err("This cell is already floored.".into());
            }
            // Unreachable in play — solid rock is struck rather than stood
            // in — and worded for the day a fifth way into base space makes
            // it reachable rather than left to the `Floor` refusal, which
            // would send the player looking for a tile they already laid.
            None => return Err("Cut the entropy out first — nothing stands on raw rock.".into()),
        }

        let substrate = ItemId::from(ids::BLANK_SUBSTRATE);
        let player = self.player_entity();
        let held = self
            .world
            .get::<Inventory>(player)
            .map(|inv| inv.count(&substrate))
            .unwrap_or(0);
        if held == 0 {
            // Reported through the `Err` alone, like the two refusals above
            // it: app-core raises every one of them as the status banner,
            // and logging this one as well put the same sentence in the
            // base log permanently — once per press, folded by
            // `resources::condense` into a row with a count rather than
            // suppressed. A refusal is news to the player standing there,
            // not to the base's own record of what it did.
            return Err(format!(
                "You have no {} to press into a tile.",
                self.item_name(&substrate)
            ));
        }
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(substrate, 1);
        self.floor_cell(x, y);
        self.log_base("You lay a VectorStasis Tile, and the cell reads as floor.");
        self.tick();
        Ok(())
    }

    /// The pending `BuildSite` standing on base-space `(x, y)`, if any.
    ///
    /// `find_blocking_structure_at`'s counterpart, and it carries the same
    /// `in_base` gate for the same reason: a build site's `Position` is in
    /// base space, so a surface-space query must never be answered by one
    /// whose coordinates happen to coincide.
    pub(crate) fn build_site_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        if !self.in_base() {
            return None;
        }
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<BuildSite>>();
        query
            .iter(&self.world)
            .find(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
    }

    /// Every `BuildSite`'s committed program, in query order.
    ///
    /// Exists for tests: a save-round-trip test that wants to know a
    /// program survived the trip has no `Entity` to look it up by, since a
    /// `BuildSite::program` is a value snapshot rather than a live
    /// creature.
    #[cfg(test)]
    pub(crate) fn build_site_programs(&mut self) -> Vec<Option<save::CreatureSave>> {
        let mut query = self.world.query::<&BuildSite>();
        query
            .iter(&self.world)
            .map(|site| site.program.clone())
            .collect()
    }

    /// How many of `kind` the base has on order but has not raised yet.
    ///
    /// **`BuildGoal::New` only.** An upgrade site names the machine's own
    /// kind too — that is what `BuildSite::structure` means — so counting
    /// every goal would let a pending upgrade eat one of that kind's
    /// `max_deployed` slots and refuse a legitimate deploy with a figure the
    /// player cannot account for. Nothing new stands up when an upgrade
    /// finishes.
    /// `iter_entities` rather than a query, for `game::base::collect`'s
    /// reason: `Game::structure_build_cost` is `&self` — a screen quotes a
    /// price — and a `World::query` would make this the one figure in that
    /// expression it could not ask for. A second, `&self` copy of the rule
    /// beside a `&mut self` one is how the two goals drift.
    pub(crate) fn count_build_requests(&self, kind: &StructureId) -> u32 {
        self.world
            .iter_entities()
            .filter_map(|e| e.get::<BuildSite>())
            .filter(|site| &site.structure == kind && site.goal == BuildGoal::New)
            .count() as u32
    }

    /// How many of `kind` are deployed right now.
    fn count_structures(&mut self, kind: &StructureId) -> u32 {
        let mut query = self.world.query::<&Structure>();
        query.iter(&self.world).filter(|s| &s.kind == kind).count() as u32
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

    /// The bill the next tier would be filed against — what the upgrade menu
    /// quotes, scaled by the tier being reached exactly as
    /// `upgrade_structure` prices it. `None` where there is no next tier.
    ///
    /// Priced here rather than in the renderer for `BuildOrderRow`'s reason:
    /// a menu that quoted its own arithmetic could disagree with the request
    /// the player is about to file.
    pub fn upgrade_cost(&self, structure: Entity) -> Option<Vec<(ItemId, u32)>> {
        let kind = &self.world.get::<Structure>(structure)?.kind;
        let upgrade = self
            .world
            .resource::<StructureDb>()
            .get(kind)?
            .upgrade
            .as_ref()?;
        let tier = self
            .world
            .get::<StructureTier>(structure)
            .map(|t| t.0)
            .unwrap_or(1);
        if tier >= upgrade.max_tier {
            return None;
        }
        let next = tier + 1;
        Some(
            upgrade
                .cost
                .iter()
                .map(|(item, qty)| (item.clone(), qty * next))
                .collect(),
        )
    }

    /// **Files a request** to advance `structure` one upgrade tier, against
    /// its `UpgradeDef` cost scaled by the tier being reached. The crew that
    /// raises a deploy fetches this bill by hand and works the site until the
    /// tier lands — see `Game::raise_one_tick`, the one step that branches on
    /// `BuildGoal`.
    ///
    /// **Nothing is charged here**, `place_structure`'s rule: an upgrade the
    /// base cannot afford yet is a legitimate thing to file, and the store
    /// the crew draws from is the base's own shelves rather than the pack.
    /// This was the last structure cost paid out of the player's `Inventory`.
    ///
    /// The tier, when it lands, both multiplies the structure's work payout
    /// (see `systems::task_progress_system`) and becomes its
    /// `ResourceNode::level`, so extraction gets more reliable as well as
    /// more productive — reusing the existing `mining_success_chance` curve
    /// rather than adding a second one.
    ///
    /// The machine **keeps running** while its request stands: standing it
    /// down would bring back the deadlock class build orders closed, on a
    /// base that files three upgrades at once.
    ///
    /// **`program` is a tamed program you own, and it is spent.** A trailing
    /// parameter rather than an `upgrade_structure_with_program` beside this
    /// one, `place_structure`'s reason: two doors into one rule leave one
    /// that raises a tier for free. The tier reached is the depth demanded —
    /// a Mk3 wants a zone-3 program — and `upgrade_ceiling` has already
    /// clamped that against `ZoneLevel`, so the demand is always one the
    /// player could have met.
    pub fn upgrade_structure(
        &mut self,
        structure: Entity,
        program: Option<Entity>,
    ) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        self.require_base()?;
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
        let Some(upgrade) = def.upgrade.as_ref() else {
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
        if tier >= self.upgrade_ceiling(upgrade) {
            return Err(format!(
                "{} can't go past Mk{tier} until you breach to zone {}.",
                def.name,
                tier + 1
            ));
        }
        let Some(pos) = self.world.get::<Position>(structure).copied() else {
            return Err("That structure is already gone.".into());
        };
        // Its own sentence rather than folded into any refusal above,
        // because it leaves the player a different errand: the others need a
        // breach or a different machine, this one needs the standing request
        // calling off. A deploy request can never be on an occupied cell, so
        // a site here is always this machine's own upgrade.
        if self.build_site_at(pos.x, pos.y).is_some() {
            return Err(format!(
                "Your crew is already on order to upgrade the {}. Call that request off first.",
                def.name
            ));
        }
        let next = tier + 1;
        // Last of the refusals and immediately before the site is spawned,
        // `place_structure`'s ladder: every question about the machine and
        // the cell it stands on is answered first, so a player whose crew is
        // already on order for this very upgrade is told about that order
        // rather than about a program they were never going to spend.
        //
        // The label is read before the commit, which despawns the body.
        let offered_label = program.map(|p| self.creature_label(p));
        let committed =
            self.commit_for_build(program, &def, BuildGoal::Upgrade { to_tier: next })?;
        // `place_structure`'s rule: an exempt structure may be handed a
        // program and still commit nothing, and the line must not name one.
        let committed_who = offered_label.filter(|_| committed.is_some());
        // Resolved now and carried on the site, `BuildSite::cost`'s reason:
        // a request filed against one price may not be silently repriced by
        // an edited def while the crew is already hauling to it.
        let cost: Vec<(ItemId, u32)> = upgrade
            .cost
            .iter()
            .map(|(item, qty)| (item.clone(), qty * next))
            .collect();

        let mut site = BuildSite::upgrade(kind.clone(), cost, next);
        site.program = committed;
        self.world.spawn((site, pos));
        self.log_base(match committed_who {
            Some(who) => format!(
                "You put the {} in for an upgrade to Mk{next} and commit {who} to it — \
                 your crew will fetch what it needs.",
                def.name
            ),
            None => format!(
                "You put the {} in for an upgrade to Mk{next} — your crew will fetch what it needs.",
                def.name
            ),
        });
        self.tick();
        Ok(())
    }

    /// Despawns the pending build request standing on `(x, y)`, if one is,
    /// and hands back whatever had already been carried to it.
    ///
    /// **Both destruction paths call this** — `damage_structure`'s destroyed
    /// branch and `remove_structure`, the Home cascade included. Wired into
    /// one alone, the other strands goods on a cell nothing occupies, and
    /// nothing fails to compile when only one is done. The refund goes
    /// through `return_build_holdings`, the same door `cancel_build_request`
    /// uses — which is where the program committed to the order comes back
    /// from too, and it is a roster slot rather than a pile of goods.
    pub(crate) fn clear_pending_build_at(&mut self, x: i32, y: i32) {
        let Some(site) = self.build_site_at(x, y) else {
            return;
        };
        let Some(build) = self.world.get::<BuildSite>(site).cloned() else {
            return;
        };
        self.return_build_holdings(&build);
        self.world.despawn(site);
    }

    /// Hands back whatever a dying build request was holding — the materials
    /// already carried to the cell and, if the order was paid with one, the
    /// program committed to it.
    ///
    /// **Both destruction paths call this.** `cancel_build_request` is the
    /// player calling the job off; `clear_pending_build_at` is the cell going
    /// out from under it, which is `remove_structure` and `damage_structure`'s
    /// destroyed branch. Wired into one alone, the other quietly eats a
    /// program, and nothing fails to compile — the same warning
    /// `clear_pending_build_at`'s own doc carries about the materials, now
    /// with a roster slot riding on it.
    ///
    /// **It reads the caller's clone, never the live component.** Both doors
    /// take a whole `BuildSite` by `.cloned()` and then despawn the site, and
    /// that clone now deep-copies a `CreatureSave`. Reaching back through
    /// `site` here instead would work right up until a caller reordered its
    /// despawn, at which point a committed program would be silently
    /// destroyed rather than fail to compile.
    ///
    /// **A finished build never reaches this.** `consume_site` despawns the
    /// site and the snapshot goes with it, which is exactly what "spent"
    /// means: the structure is standing, and the program paid for it.
    ///
    /// The refund is announced because it is a thing the player owns coming
    /// back — the counterpart to the deploy line that named what it took.
    /// The failure is announced for the same reason and more loudly: a
    /// snapshot naming a species whose `.ron` has been deleted between
    /// sessions cannot be respawned (see `refund_program`), and that is a
    /// supported thing to do to an install rather than a panic. Saying
    /// nothing would leave a program simply missing from the roster with no
    /// line anywhere accounting for it.
    fn return_build_holdings(&mut self, build: &BuildSite) {
        for (item, qty) in &build.delivered {
            self.return_material(item, *qty);
        }
        let Some(program) = &build.program else {
            return;
        };
        match self.refund_program(program) {
            Some(back) => {
                let name = self.creature_label(back);
                self.log_base(format!("{name} comes back off the job."));
            }
            None => {
                // Named off the snapshot, because there is no entity to
                // label: `creature_name`'s own fallback order minus the
                // species lookup, which is the thing that just failed.
                let name = program
                    .custom_name
                    .clone()
                    .unwrap_or_else(|| program.species.clone());
                self.log_base(format!(
                    "{name} does not come back off the job — this install no longer \
                     carries its kind, and there is nothing left to restore it from."
                ));
            }
        }
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
        self.require_base()?;
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
            if let Some(pos) = self.world.get::<Position>(target).copied() {
                self.clear_pending_build_at(pos.x, pos.y);
            }
            self.announce_lost_shelf(target);
            self.world.despawn(target);
        }
        // `clear_platform` used to run here, restoring the zone surface
        // under a demolished Home's slab. The base is out of phase now —
        // its floor is `BaseGrid`, the player's own dug ground, and
        // `Game::has_home`'s own doc already states the design this
        // implies: demolishing the Home takes every structure with it but
        // leaves the floor standing, so you can be in base space having
        // just demolished the Home out from under yourself.

        // Route through `grant_loot`, not a direct `add`: demolishing a Home
        // cascades every other structure's refund in one shot, easily
        // enough to blow past the buffer with no message.
        for (item, qty) in &refund {
            self.grant_loot(item.clone(), *qty, LootSource::Refund);
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

    /// How many ticks one work cycle against `structure` takes for a worker
    /// of `worker_speed`, from the structure's def rate scaled by
    /// `systems::work_ticks_at_speed`.
    ///
    /// Shared by `assign_cronjob` and `work_structure`, which is what makes
    /// the comparison legible: the player has no species and so works at
    /// the baseline, and a posted program is faster or slower than that by
    /// its own `base_speed`.
    fn work_ticks_for(&mut self, structure: Entity, worker_speed: i32) -> u32 {
        let kind = self.world.get::<Structure>(structure).unwrap().kind.clone();
        let db = self.world.resource::<StructureDb>();
        let base = match db.get(&kind) {
            None => 5,
            Some(def) => match (&def.work, &def.assembles) {
                (Some(work), _) => work.ticks_per_unit,
                (None, Some(assembles)) => assembles.ticks_per_unit,
                (None, None) => 5,
            },
        };
        crate::systems::work_ticks_at_speed(
            base,
            worker_speed,
            crate::classes::work_tick_scale(self.player_class()),
        )
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
    /// (`transfer_offer`), so a job run from anywhere else fills a buffer
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
        self.require_base()?;
        if self.world.get::<ResourceNode>(structure).is_none() {
            return Err("That structure can't be worked.".into());
        }
        // The party's *base* cell, not their `Position` — that is pinned to
        // the anchor tile on the zone surface the whole time they are in
        // here, and the machine being worked stands in base space.
        let (hx, hy) = self
            .base_pos()
            .expect("require_base passed, so the party is in base space");
        let here = Position { x: hx, y: hy };
        let structure_pos = *self
            .world
            .get::<Position>(structure)
            .ok_or_else(|| "That structure isn't anywhere you can reach.".to_string())?;
        if !hauling::at_station(here, structure_pos) {
            return Err(
                "You have to be standing next to it to work it — get beside it first.".into(),
            );
        }
        let player = self.player_entity();
        let speed = self.species_base_speed(player);
        let ticks = self.work_ticks_for(structure, speed);
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

    /// Everything posting a program to a machine *does*, with none of what
    /// decides whether it may — `assign_cronjob` above is those refusals
    /// plus a call to this, and `schedule_base_labour` calls it directly
    /// having answered the same questions its own way.
    ///
    /// Split so the scheduler drives the mechanism that already exists
    /// rather than growing a second one: the same `Task`, the same
    /// `CronjobSave`, the same hauling, the same `work_ticks_for` rate
    /// baked in at assignment. That is what makes an existing save's
    /// postings survive and keeps the walk-in, the depot errand and the
    /// `Stranded` marker working without being reasoned about again.
    ///
    /// **No `Position` write**, the same omission `post_guard` makes and
    /// for the same reason: a program sets off from the tile it is standing
    /// on. This used to overwrite it with the player's, because a tamed
    /// program's `Position` was the tile it was beaten on and was never
    /// written again — posting was the only moment it could be made true.
    /// `drift_idle_staff` writes it every tick now, so the value is already
    /// live and overwriting it teleported a wandering body onto the player.
    pub(crate) fn post_worker(&mut self, worker: Entity, structure: Entity) {
        let speed = self.species_base_speed(worker);
        let ticks = self.work_ticks_for(structure, speed);
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
    }

    /// `post_worker`'s counterpart for a dig site: the crew job, posted by
    /// `schedule_base_labour` and worked by `Game::run_dig_crew`.
    ///
    /// The rate is `tuning::BASE_DIG_TICKS_PER_SWING` rather than
    /// `work_ticks_for`, because a dig site is not a structure and has no
    /// def to read one off. What the *worker* brings is the bite instead:
    /// `Game::swing_damage` takes its own species band and `atk`, so a
    /// stronger program cuts a wall out in fewer swings rather than faster
    /// ones.
    pub(crate) fn post_digger(&mut self, worker: Entity, site: Entity) {
        if self.world.resource::<Party>().0.contains(&worker) {
            self.world
                .resource_mut::<Party>()
                .0
                .retain(|&e| e != worker);
            self.log_base("It stands down as your companion to work the excavation.");
        }
        self.displace_task_holder(site, TaskKind::Excavate);
        self.world.entity_mut(worker).insert(Task {
            kind: TaskKind::Excavate,
            target: site,
            progress: 0,
            required: crate::tuning::BASE_DIG_TICKS_PER_SWING,
        });
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
    ///
    /// `post_worker`'s counterpart for a guard post, split for the same
    /// reason: the scheduler fills a standing guard job and has answered
    /// the refusals above its own way.
    ///
    /// No `work_ticks_for` and no `Position` write — guarding produces
    /// nothing, so there is no cycle to rate and no station to walk to.
    pub(crate) fn post_guard(&mut self, worker: Entity, structure: Entity) {
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
    }
}
