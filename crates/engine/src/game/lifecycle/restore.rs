//! The `restore_*`/`attach_*` helpers `Game::load` calls, one per saved
//! collection.

use crate::game::spawning;
use crate::tuning::NEST_DURABILITY;
use crate::*;

impl Game {
    pub(super) fn restore_nests(
        &mut self,
        nests: Vec<save::NestSave>,
    ) -> HashMap<(i32, i32), Entity> {
        let mut nest_positions: HashMap<(i32, i32), Entity> = HashMap::new();
        for n in nests {
            let Some(species) = self.world.resource::<SpeciesDb>().get(&n.species).cloned() else {
                continue;
            };
            let nest = self
                .world
                .spawn(spawning::nest_components(
                    &species,
                    n.position.0,
                    n.position.1,
                    // Clamped rather than trusted outright: NEST_DURABILITY
                    // is a tuning.rs constant, not part of the save format,
                    // so lowering it must not leave an existing save's nest
                    // loading with hp above the new max — the structure
                    // path a little further down clamps the same way.
                    n.durability.min(NEST_DURABILITY),
                    n.pending_respawns,
                ))
                .id();
            nest_positions.insert(n.position, nest);
        }
        nest_positions
    }

    /// Stands every saved trap back up.
    ///
    /// A trap whose item def has gone since the save was written is dropped
    /// rather than restored, `restore_nests`' handling of a missing species:
    /// nothing forward-references a trap and a trap references nothing but
    /// its own item id, so there is no `pending_cronjobs` deferral to owe and
    /// this can sit anywhere in load order.
    ///
    /// The glyph is re-derived from `caught` rather than read back, so the
    /// char and the state cannot disagree.
    pub(super) fn restore_traps(&mut self, traps: Vec<save::TrapSave>) {
        for t in traps {
            if self
                .world
                .resource::<ItemDb>()
                .get(t.item.as_str())
                .is_none()
            {
                continue;
            }
            let ch = match t.caught.is_some() {
                true => crate::components::TRAP_GLYPH_SPRUNG,
                false => crate::components::TRAP_GLYPH_ARMED,
            };
            self.world.spawn((
                crate::components::Trap {
                    item: t.item,
                    next_roll: t.next_roll,
                    caught: t.caught,
                },
                Position {
                    x: t.position.0,
                    y: t.position.1,
                },
                Glyph {
                    ch,
                    color: GlyphColor::Yellow,
                },
            ));
        }
    }

    pub(super) fn restore_caravans(
        &mut self,
        memory: save::CaravanMemorySave,
        caravans: Vec<save::CaravanSave>,
    ) {
        self.world.insert_resource(crate::resources::CaravanMemory {
            visit: memory.visit,
            bought: memory.bought.into_iter().collect(),
        });

        // A caravan whose `.ron` file has gone since the save was written is
        // dropped rather than spawned glyphless: the visit becomes a miss,
        // which is the same thing an install with no `assets/caravans/` at
        // all sees, and is what makes deleting the directory supported rather
        // than breaking a save.
        for c in caravans {
            let Some(def) = self.visit_at(c.visit).and_then(|v| {
                self.world
                    .resource::<crate::caravans::CaravanDb>()
                    .get(&v.def_id)
                    .cloned()
            }) else {
                continue;
            };
            self.world.spawn((
                Caravan {
                    stage: c.stage,
                    visit: c.visit,
                    arrival_tile: c.arrival_tile,
                    stage_ticks: c.stage_ticks,
                    // Never saved: a reload is exactly when the player should
                    // be told again. `DigSite::announced_stuck`'s rule.
                    announced_stuck: false,
                },
                Position {
                    x: c.position.0,
                    y: c.position.1,
                },
                Glyph {
                    ch: def.glyph,
                    color: def.color,
                },
            ));
        }
    }

    pub(super) fn restore_build_sites(&mut self, sites: Vec<save::BuildSiteSave>) {
        for b in sites {
            let goal = b.goal;
            let mut site = self.world.spawn((
                BuildSite {
                    structure: b.structure,
                    cost: b.cost,
                    delivered: b.delivered,
                    progress: b.progress,
                    // Never saved, `DigSite`'s two latches' rule: a crew's
                    // "I already said so" is true of a conversation and not
                    // of the world, so a reload says it again.
                    announced_dry: false,
                    announced_stuck: false,
                    goal,
                    program: b.program,
                    awaiting_program: b.awaiting_program,
                    recipe: b.recipe,
                },
                Position {
                    x: b.position.0,
                    y: b.position.1,
                },
            ));
            // An upgrade site is born without one, because the machine it is
            // about is still standing on that cell and still drawing itself.
            // Attached unconditionally, the tile would carry two views and
            // the map would paint a build slab over a working machine.
            if goal == crate::components::BuildGoal::New {
                site.insert(Glyph {
                    ch: BUILD_SITE_GLYPH,
                    color: GlyphColor::Orange,
                });
            }
        }
    }

    pub(super) fn restore_dig_sites(&mut self, sites: Vec<save::DigSiteSave>) {
        for d in sites {
            // `BaseGrid::prune_finishes`'s rule, reached a second way: an
            // `Apply` order naming an id the loaded `FloorDb` no longer
            // resolves must not become a live `DigSite` the crew can never
            // finish. Same message shape as `prune_finishes`'s own drops,
            // logged through the same sink.
            if let Some(FinishOrder::Apply(id)) = &d.finish
                && self
                    .world
                    .resource::<crate::floors::FloorDb>()
                    .get(id)
                    .is_none()
            {
                self.log(format!(
                    "dropped floor finish {id:?} at ({}, {}): no such finish is loaded",
                    d.position.0, d.position.1
                ));
                continue;
            }
            let wall = self.wall_at(d.position.0, d.position.1);
            self.world.spawn((
                DigSite {
                    marked: d.marked,
                    // Never saved: a crew's "I already said so" is true of a
                    // conversation, not of the world, and a reload is
                    // exactly when the player should be told again.
                    announced_stuck: false,
                    announced_dry: false,
                    finish: d.finish,
                },
                Durability {
                    // Clamped rather than trusted, the same way a nest's is
                    // just above: a kind's durability lives in
                    // `assets/rock/` and not in the save format, so
                    // softening a kind — or deleting its file — must not
                    // leave an existing save's wall loading with more rock
                    // in it than a fresh one has.
                    //
                    // **Re-derived from the site's own `Position`**, which
                    // is why a `DigSite` still saves nothing about what it
                    // is made of: the kind is a property of the place.
                    // Clamping against the flat `BASE_ROCK_DURABILITY`
                    // instead would silently cap every dense wall in the
                    // base at the fallback's ceiling on the first reload.
                    hp: d.durability.min(wall.durability),
                    max_hp: wall.durability,
                },
                Position {
                    x: d.position.0,
                    y: d.position.1,
                },
            ));
        }
    }

    pub(super) fn restore_sorties(
        &mut self,
        saved: Vec<save::SortieSave>,
        sortie_members: &[(u32, Entity)],
    ) {
        // A record whose members all failed to load — a species file deleted
        // between sessions — is dropped rather than restored empty: an empty
        // squad is a countdown nothing comes home from, and a base is better
        // off short a trip than waiting forever on one.
        let sorties: Vec<crate::resources::Sortie> = saved
            .into_iter()
            .enumerate()
            .filter_map(|(index, s)| {
                let members: Vec<Entity> = sortie_members
                    .iter()
                    .filter(|&&(i, _)| i as usize == index)
                    .map(|&(_, e)| e)
                    .collect();
                if members.is_empty() {
                    return None;
                }
                Some(crate::resources::Sortie {
                    site: s.site,
                    risk: s.risk,
                    members,
                    ticks_total: s.ticks_total,
                    ticks_elapsed: s.ticks_elapsed,
                    battles_total: s.battles_total,
                    battles_done: s.battles_done,
                    aborted: s.aborted,
                    loot: s.loot,
                    programs: s.programs,
                    xp: s.xp,
                    kills: s.kills,
                    casualties: s.casualties,
                })
            })
            .collect();
        self.world
            .insert_resource(crate::resources::Sorties(sorties));
    }

    /// Straight field-for-field, `routes::Route`'s own reason: cargo names
    /// no entity, so there is nothing here to reconcile against
    /// `sortie_members` above. Settlement routes only — `restore_outpost_routes`
    /// builds the other half, and the two are merged by `order` rather than
    /// inserted directly: `SaveData::routes`/`outpost_routes` is a
    /// settlement/outpost split of one live `Vec<Route>`, so simply
    /// appending one after the other would reorder any save with the two
    /// kinds interleaved — see `save::RouteSave::order`'s doc.
    pub(super) fn restore_routes(
        &self,
        saved: Vec<save::RouteSave>,
    ) -> Vec<(usize, crate::routes::Route)> {
        saved
            .into_iter()
            .map(|r| {
                (
                    r.order,
                    crate::routes::Route {
                        destination: crate::routes::RouteEnd::Settlement {
                            key: r.destination,
                            def: r.destination_def,
                            tile: r.destination_tile,
                        },
                        cargo: r.cargo,
                        standing: r.standing,
                        stalled: r.stalled,
                        leg: r.leg,
                        ticks_total: r.ticks_total,
                        ticks_elapsed: r.ticks_elapsed,
                        proceeds: r.proceeds,
                    },
                )
            })
            .collect()
    }

    /// `restore_routes`' twin for the outpost half of `resources::Routes` —
    /// see its doc comment for why the two are merged by `order` rather than
    /// one simply appended onto the other.
    pub(super) fn restore_outpost_routes(
        &self,
        saved: Vec<save::OutpostRouteSave>,
    ) -> Vec<(usize, crate::routes::Route)> {
        saved
            .into_iter()
            .map(|r| {
                (
                    r.order,
                    crate::routes::Route {
                        destination: crate::routes::RouteEnd::Outpost(r.tile),
                        cargo: r.cargo,
                        standing: r.standing,
                        stalled: r.stalled,
                        leg: r.leg,
                        ticks_total: r.ticks_total,
                        ticks_elapsed: r.ticks_elapsed,
                        proceeds: 0,
                    },
                )
            })
            .collect()
    }

    /// `restore_routes`' shape one type over: an outpost names no entity
    /// either, so this is a straight field-for-field rebuild keyed back onto
    /// its tile. `announced` is not part of `save::OutpostSave` and comes
    /// back `None` here — `Game::reseed_outpost_announcements`, called once
    /// `attach_outpost_crew` has run, is what fixes it back to the real
    /// trend before the load returns.
    pub(super) fn restore_outposts(&mut self, saved: Vec<save::OutpostSave>) {
        let map: std::collections::BTreeMap<(i32, i32), crate::outposts::Outpost> = saved
            .into_iter()
            .map(|o| {
                (
                    o.tile,
                    crate::outposts::Outpost {
                        biome: o.biome,
                        growth: o.growth,
                        integrity: o.integrity,
                        stock: o.stock.into_iter().collect(),
                        stale_ticks: o.stale_ticks,
                        cycle_progress: o.cycle_progress,
                        announced: None,
                    },
                )
            })
            .collect();
        self.world.insert_resource(crate::resources::Outposts(map));
    }

    pub(super) fn restore_structures(
        &mut self,
        structures: Vec<save::StructureSave>,
    ) -> (HashMap<(i32, i32), Entity>, Vec<String>) {
        let mut structure_positions: HashMap<(i32, i32), Entity> = HashMap::new();
        let mut warnings = Vec::new();
        for s in structures {
            let Some(def) = self.world.resource::<StructureDb>().get(&s.kind).cloned() else {
                continue;
            };
            let mut entity = self.world.spawn((
                Structure {
                    kind: def.id.clone(),
                },
                Position {
                    x: s.position.0,
                    y: s.position.1,
                },
                Glyph {
                    ch: def.glyph,
                    color: def.color,
                },
            ));
            // A save written before `raidable` existed still records a
            // durability for what is now a non-raidable structure; the def
            // wins, so that stored value is simply dropped.
            if def.raidable {
                entity.insert(Durability {
                    hp: s.durability.unwrap_or(def.durability).min(def.durability),
                    max_hp: def.durability,
                });
            }
            let structure_id = entity.id();
            structure_positions.insert(s.position, structure_id);
            entity.insert(Stock {
                input: s.stock_input.iter().cloned().collect(),
                output: s.stock_output.iter().cloned().collect(),
                capacity: def.capacity,
            });
            // The def decides whether a rig stands here, not the save: a
            // stored hopper on a structure whose `strips` a mod has since
            // taken away is simply dropped, `durability`'s own rule two
            // arms above.
            if def.strips.is_some() {
                entity.insert(crate::components::Hopper {
                    queue: s.hopper.clone(),
                    progress: s.hopper_progress,
                    standing_tool: s.standing_tool.clone(),
                });
            }
            // Same rule one rung along: a stored shelf on a structure whose
            // `racks` a mod has since taken away is dropped.
            if def.racks.is_some() {
                entity.insert(crate::components::Racked(s.racked.clone()));
            }
            // Not gated on the def, unlike the two above: a held program is
            // a kill, and a structure that has merely stopped studying would
            // otherwise eat it. (A structure whose def is gone altogether is
            // skipped at the top of this loop, with everything on it.)
            if s.study_yield.is_some() {
                entity.insert(crate::components::StudyYield(s.study_yield.clone()));
            }
            // And again for the bay. Never shrunk below what the save holds:
            // a retune that cut `incubation_slots` — to zero included — must
            // not eat a child.
            if def.incubation_slots > 0 || s.incubating.iter().any(Option::is_some) {
                let mut incubator = crate::components::Incubator::with_slots(def.incubation_slots);
                for (slot, saved) in incubator.slots.iter_mut().zip(&s.incubating) {
                    *slot = saved.clone();
                }
                incubator
                    .slots
                    .extend(s.incubating.iter().skip(incubator.slots.len()).cloned());
                entity.insert(incubator);
            }
            // Both halves mirror `Game::spawn_structure`'s list, which is the
            // hand-written copy this file has always been: a burning supplier
            // missing its `PowerFuel` reads as a base whose grid collapsed on
            // reload, and nothing here fails to compile when it drifts.
            if def.has_machine_status() {
                entity.insert(MachineStatus::default());
            }
            if def.power_upkeep.is_some() {
                entity.insert(crate::components::PowerFuel {
                    ticks_left: s.power_fuel,
                });
            }
            // Absent rather than defaulted when neither flag is set, so
            // "has a standing job" stays readable as the component's
            // presence — the same invariant `Equipment` keeps.
            if s.standing_work || s.standing_guard {
                entity.insert(StandingJob {
                    work: s.standing_work,
                    guard: s.standing_guard,
                });
            }
            // Absent rather than empty for `StandingJob`'s reason one arm
            // up: "this Depot takes anything" has exactly one
            // representation, and `Game::set_depot_filter` keeps it that
            // way on the live side.
            if !s.denied_items.is_empty() {
                entity.insert(crate::components::DepotFilter {
                    denied: s.denied_items.iter().cloned().collect(),
                });
            }
            // Rebuilt from the def rather than from the save: with the
            // deposit pool gone, a node carries nothing per-instance that a
            // `.ron` file doesn't already say. What the node *produced* is
            // in `Stock` above, which is where the state now lives.
            //
            // A machine's recipe is the one per-instance fact, and the def
            // decides whether it still stands: a mod that dropped it leaves
            // the machine unconfigured rather than running something the
            // def no longer lists.
            let recipe = if def.machine.is_some() {
                let kept = s
                    .recipe
                    .clone()
                    .filter(|item| def.machine_recipe(item).is_some());
                if let (Some(saved), None) = (&s.recipe, &kept) {
                    warnings.push(format!(
                        "a {} at {},{} was set to {saved}, which it no longer makes — it loads with nothing set",
                        def.id, s.position.0, s.position.1
                    ));
                }
                entity.insert(crate::components::MachineRecipe(kept.clone()));
                kept
            } else {
                None
            };
            let placed = crate::structures::Placed {
                def: &def,
                recipe: recipe.as_ref(),
            };
            if let Some(production) = placed.production().filter(|p| p.extracts) {
                entity.insert(ResourceNode {
                    resource: production.recipe.produces.clone(),
                    level: production.recipe.level,
                });
            }
            // Inserted unconditionally: a Home reloaded at 1.0 carries a
            // component it did not have before, which changes nothing
            // because 1.0 is the neutral the absent case already means.
            entity.insert(crate::components::BuildQuality(s.build_quality));
            // The def decides whether a pod stands here, `strips`/`racks`'
            // rule two arms up — a stored charge on a structure whose
            // `drop_pod` a mod has since taken away is simply dropped.
            // Absent reads as charged, `s.pod_charged`'s own default.
            if def.drop_pod.is_some() {
                entity.insert(crate::components::DropPod {
                    charged: s.pod_charged.unwrap_or(true),
                });
            }
            if def.upgrade.is_some() {
                let tier = s.tier.unwrap_or(1);
                entity.insert(StructureTier(tier));
                // RecipeDef::level only carries the tier-1 baseline, so a
                // restored node's reliability has to be re-derived from its
                // tier or a Mk3 would come back extracting like a Mk1.
                if let Some(mut node) = entity.get_mut::<ResourceNode>()
                    && node.level.is_some()
                {
                    node.level = Some(tier);
                }
            }
        }
        (structure_positions, warnings)
    }

    pub(super) fn attach_cronjobs(
        &mut self,
        pending: Vec<(Entity, save::CronjobSave)>,
        structure_positions: &HashMap<(i32, i32), Entity>,
    ) {
        // Reconnect each restored cronjob to its target structure now that
        // both sides exist. A structure is matched by position (entity ids
        // aren't stable across a save/load round trip) — if it's gone,
        // the assignment is silently dropped rather than crashing.
        for (worker, cronjob) in pending {
            if let Some(&target) = structure_positions.get(&cronjob.target_position) {
                // A machine that loaded unset (its saved recipe is no longer
                // listed) runs no job, so a program posted to it would idle.
                // A supplier runs no job either, but `fuel_wants` posts a
                // body to it to carry the supply in: dropping that posting
                // here keeps the restored `Carrying` and strands the body.
                if matches!(cronjob.kind, save::CronjobKind::GatherResource)
                    && !self.accepts_a_program(target)
                    && !self.takes_a_supply_run(target)
                {
                    continue;
                }
                self.world.entity_mut(worker).insert(Task {
                    kind: match cronjob.kind {
                        save::CronjobKind::GatherResource => TaskKind::GatherResource,
                        save::CronjobKind::Guard => TaskKind::Guard,
                    },
                    target,
                    progress: cronjob.progress,
                    required: cronjob.required,
                });
            }
        }
    }

    /// Reconnects a restored subject to its Research Station now that both
    /// sides exist — `attach_cronjobs`'s shape and leniency, one call after
    /// it for the same reason: a station is one of the structures that
    /// array just rebuilt.
    ///
    /// **A tile naming no structure, or one that no longer declares
    /// `studies`** (a modder deleted or replaced the Station between
    /// sessions), **drops the pin silently** rather than failing the load —
    /// `nest_position`'s rule: the program comes back as ordinary `Staff`,
    /// which is exactly what standing the Station down would have left it
    /// as. `Game::study_pen` is the one door that answers "does this
    /// structure study", so this asks it rather than re-reading the def.
    pub(super) fn attach_pinned_subjects(
        &mut self,
        pending: Vec<(Entity, (i32, i32))>,
        structure_positions: &HashMap<(i32, i32), Entity>,
    ) {
        for (program, tile) in pending {
            let Some(&station) = structure_positions.get(&tile) else {
                continue;
            };
            if self.study_pen(station).is_none() {
                continue;
            }
            self.world
                .entity_mut(program)
                .insert(crate::components::UnderStudy { station });
        }
    }

    /// Reattaches `components::Siphoned` now that structures are restored —
    /// `attach_pinned_subjects`' shape. **A tile naming no structure, one that
    /// no longer declares `siphons`, or a siphon an earlier claimant already
    /// holds drops the hold silently**: the program comes back as ordinary
    /// `Staff`, which is what releasing it would have left it as, minus the
    /// integrity cost a load has no business charging.
    pub(super) fn attach_siphoned(
        &mut self,
        pending: Vec<(Entity, (i32, i32))>,
        structure_positions: &HashMap<(i32, i32), Entity>,
    ) {
        for (program, tile) in pending {
            let Some(&siphon) = structure_positions.get(&tile) else {
                continue;
            };
            if !self.is_siphon(siphon) || self.siphon_holder(siphon).is_some() {
                continue;
            }
            self.world
                .entity_mut(program)
                .insert(crate::components::Siphoned { siphon });
        }
    }

    /// Reattaches `components::Jailed` now that structures are restored —
    /// `attach_siphoned`'s shape. **A tile naming no structure, one that no
    /// longer declares `holds_prisoner`, or a cell an earlier prisoner
    /// already fills releases the prisoner**: the body despawns and its record
    /// goes back to the player's downed programs (`release_prisoner`'s rule),
    /// rather than the program walking free as `Staff` having skipped the
    /// decompile.
    pub(super) fn attach_jailed(
        &mut self,
        pending: Vec<(Entity, save::JailedSave)>,
        structure_positions: &HashMap<(i32, i32), Entity>,
    ) {
        for (program, saved) in pending {
            let cell = structure_positions
                .get(&saved.cell_pos)
                .copied()
                .filter(|&c| self.prison_pen(c).is_some() && self.cell_prisoner(c).is_none());
            let Some(cell) = cell else {
                let record = saved.record.or_else(|| self.downed_program_for(program));
                if let Some(record) = record {
                    self.push_downed_program(record);
                }
                self.world.despawn(program);
                continue;
            };
            self.world
                .entity_mut(program)
                .insert(crate::components::Jailed {
                    cell,
                    attempts: saved.attempts,
                    progress: saved.progress,
                    record: saved.record,
                });
        }
    }

    /// Reattaches `components::PostedAt` now that `resources::Outposts` is
    /// restored — `attach_pinned_subjects`' shape, but with nothing to wait
    /// on: an outpost is a record keyed by tile, not a structure entity, so
    /// this can resolve as soon as `restore_outposts` has run rather than
    /// after the structures array further down.
    ///
    /// **A tile naming no outpost drops the membership** — `nest_position`'s
    /// leniency: the record was edited away or lost between sessions, so the
    /// program comes back as ordinary `Staff` instead of refusing the whole
    /// load. Unlike `attach_pinned_subjects` this logs the drop, since a
    /// player watching their staff count is the one who would otherwise
    /// wonder where a body went.
    pub(super) fn attach_outpost_crew(&mut self, pending: Vec<(Entity, (i32, i32))>) {
        for (program, tile) in pending {
            if !self
                .world
                .resource::<crate::resources::Outposts>()
                .0
                .contains_key(&tile)
            {
                let name = self.creature_label(program);
                self.log(format!(
                    "{name} was posted at an outpost that's no longer there, \
                     and rejoins the base staff."
                ));
                continue;
            }
            self.world
                .entity_mut(program)
                .insert(crate::components::PostedAt(tile));
        }
    }

    /// Reconnects a besieger's `components::StolenFrom` to its source
    /// structure now that both sides exist — `attach_cronjobs`'s shape and
    /// leniency: a tile naming no structure (it was destroyed before the
    /// save, or the save predates this field) drops the tether silently,
    /// which is exactly what `game::siege::raiders::drop_besieger_cargo`
    /// already does for a source that no longer stands.
    pub(super) fn attach_stolen_from(
        &mut self,
        pending: Vec<(Entity, (i32, i32))>,
        structure_positions: &HashMap<(i32, i32), Entity>,
    ) {
        for (body, tile) in pending {
            if let Some(&structure) = structure_positions.get(&tile) {
                self.world
                    .entity_mut(body)
                    .insert(crate::components::StolenFrom(structure));
            }
        }
    }
}
