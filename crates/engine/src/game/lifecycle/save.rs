//! `Game::save`: the live world written out as a `SaveData`.

use super::convert::{copy_to_save, worn_to_save};
use crate::components::{Needs, Tools};
use crate::*;

impl Game {
    /// One creature as the save format describes it, or `None` if `e` is not
    /// a creature.
    ///
    /// **The single answer to "what is a creature in a file".** The bulk dump
    /// in `save` below is the only caller today; the one this was lifted out
    /// for is the commit step that snapshots a committed program onto a build
    /// request, which needs exactly this shape for exactly one entity. A
    /// second builder is the thing that separation exists to prevent — the
    /// copy that drifts is the one nobody runs, and a build request's
    /// snapshot is written once and read back a week of play later.
    ///
    /// Reads component by component rather than through a query, which is
    /// what makes it addressable by entity at all. The four **role** fields
    /// are the only ones that change shape in the move: `party_slot`,
    /// `sortie_index` and `wielded` search the same three resources the loop
    /// used to gather up front, and `staff` asks `program_role` directly
    /// rather than scanning `base_staff()` and searching the result — the
    /// same predicate, since `base_staff` is that question over every
    /// `Tamed` entity, and O(1) instead of a roster scan per creature.
    pub(crate) fn creature_save_for(&mut self, e: Entity) -> Option<save::CreatureSave> {
        let species = self.world.get::<Creature>(e)?.species.clone();
        let pos = *self.world.get::<Position>(e)?;
        let stats = *self.world.get::<Stats>(e)?;
        let potential = self
            .world
            .get::<Potential>(e)
            .copied()
            .unwrap_or(Potential::NEUTRAL);
        // A dig job is deliberately not saved: `CronjobSave` resolves
        // its target by position against the *structures* restored
        // beside it, and a `DigSite` is not one. Nothing is lost by it —
        // the mark is what the save carries, and `schedule_base_labour`
        // posts a body back onto it on the first tick after the load.
        // The most a reload can cost is one part-finished swing.
        // `Construct` is filtered out beside `Excavate` and for the
        // same reason: neither is a cronjob. A `BuildSite` is saved in
        // its own right, carrying the bill of materials and everything
        // already delivered to it, and `schedule_base_labour` posts a
        // body back onto it on the first tick after the load. The most a
        // reload can cost is one part-finished tick of construction.
        //
        // Copied out of the component before the target is looked up
        // rather than held across it: `Task` is not `Clone`, and the
        // resolution below is a second read of the same world.
        let job = self
            .world
            .get::<Task>(e)
            .filter(|t| {
                !matches!(
                    t.kind,
                    TaskKind::Excavate | TaskKind::Construct | TaskKind::Tend(_)
                )
            })
            .map(|t| (t.kind, t.target, t.progress, t.required));
        let cronjob = job.and_then(|(kind, target, progress, required)| {
            self.world
                .get::<Position>(target)
                .map(|target_pos| save::CronjobSave {
                    target_position: (target_pos.x, target_pos.y),
                    progress,
                    required,
                    kind: match kind {
                        TaskKind::GatherResource => save::CronjobKind::GatherResource,
                        TaskKind::Guard => save::CronjobKind::Guard,
                        TaskKind::Excavate | TaskKind::Construct | TaskKind::Tend(_) => {
                            unreachable!("filtered out above")
                        }
                    },
                })
        });
        // Same by-position resolution `cronjob` above uses: a
        // `NestGuardian`'s target entity id isn't stable across the
        // round trip, but a nest's tile is.
        let nest_position = self
            .world
            .get::<NestGuardian>(e)
            .map(|g| g.nest)
            .and_then(|nest| self.world.get::<Position>(nest))
            .map(|nest_pos| (nest_pos.x, nest_pos.y));
        // The second tether, resolved the same way and for the same
        // reason. A town cannot be destroyed, so unlike a nest this can
        // only fail to resolve on a load — never on a save.
        let patrol_position = self
            .world
            .get::<TownPatrol>(e)
            .map(|p| p.town)
            .and_then(|town| self.world.get::<Position>(town))
            .map(|town_pos| (town_pos.x, town_pos.y));
        // The station's tile, `nest_position`/`patrol_position`'s reason:
        // entity ids aren't stable across a save/load round trip, so
        // `components::UnderStudy::station` is resolved to a `Position`
        // rather than written as-is.
        let study_station = self
            .world
            .get::<crate::components::UnderStudy>(e)
            .map(|u| u.station)
            .and_then(|station| self.world.get::<Position>(station))
            .map(|station_pos| (station_pos.x, station_pos.y));
        // The siphon's tile, `study_station`'s reason: entity ids aren't
        // stable across a save/load round trip.
        let siphon = self
            .world
            .get::<crate::components::Siphoned>(e)
            .map(|s| s.siphon)
            .and_then(|siphon| self.world.get::<Position>(siphon))
            .map(|pos| (pos.x, pos.y));
        // The cell's tile, `study_station`'s reason.
        let jailed = self
            .world
            .get::<crate::components::Jailed>(e)
            .and_then(|j| {
                let cell = self.world.get::<Position>(j.cell)?;
                Some(save::JailedSave {
                    cell_pos: (cell.x, cell.y),
                    attempts: j.attempts,
                    progress: j.progress,
                    record: j.record.clone(),
                })
            });
        // `(order, cell)` in an in-progress siege — `None` whenever there is
        // no open `TacticalBattle`, the one open is not a siege
        // (`siege_pack == 0`, that field's own "not a siege" answer), or `e`
        // was never seated in it. Computed once rather than at each of the
        // two fields below, since both read the same lookup.
        let siege_membership = self
            .world
            .get_resource::<crate::tactical::TacticalBattle>()
            .filter(|b| b.siege_pack > 0)
            .and_then(|b| crate::game::siege::persist::member_of(b, e));
        let base = self
            .world
            .get::<crate::components::ProgramBase>(e)
            .map(|b| b.0);
        let seated = base.is_some();
        Some(save::CreatureSave {
            species,
            position: (pos.x, pos.y),
            hp: stats.hp,
            max_hp: if seated { 0 } else { stats.max_hp },
            atk: if seated { 0 } else { stats.atk },
            mitigation: if seated { 0 } else { stats.mitigation },
            tamed: self.world.get::<Tamed>(e).is_some(),
            power: self
                .world
                .get::<PowerReserve>(e)
                .map(|r| r.get())
                .unwrap_or(POWER_MAX),
            level: self
                .world
                .get::<Experience>(e)
                .map(|x| x.level)
                .unwrap_or(1),
            xp: self.world.get::<Experience>(e).map(|x| x.xp).unwrap_or(0),
            xp_to_next: self
                .world
                .get::<Experience>(e)
                .map(|x| x.xp_to_next)
                .unwrap_or_else(|| crate::progression::xp_for_level(1)),
            cronjob,
            party_slot: self
                .world
                .resource::<Party>()
                .0
                .iter()
                .position(|&member| member == e)
                .map(|i| i as u32),
            sortie_index: self
                .world
                .resource::<crate::resources::Sorties>()
                .0
                .iter()
                .position(|sortie| sortie.members.contains(&e))
                .map(|i| i as u32),
            wielded: self.wielded_program() == Some(e),
            zone: self.world.get::<ZonePortal>(e).map(|z| z.0).unwrap_or(1),
            custom_name: self.world.get::<CustomName>(e).map(|c| c.0.clone()),
            hp_roll: potential.hp_roll,
            atk_roll: potential.atk_roll,
            def_roll: potential.def_roll,
            growth_roll: potential.growth_roll,
            assembly_roll: potential.assembly_roll,
            extraction_roll: potential.extraction_roll,
            fusions: self.world.get::<FusionCount>(e).map(|f| f.0).unwrap_or(0),
            generation: self
                .world
                .get::<crate::components::Generation>(e)
                .map_or(0, |g| g.0),
            breed_ready_at: self
                .world
                .get::<crate::components::BreedReadyAt>(e)
                .map_or(0, |r| r.0),
            refactors: self.world.get::<Refactors>(e).map(|r| r.0).unwrap_or(0),
            purchased_tiers: self
                .world
                .get::<PurchasedTiers>(e)
                .map(|t| t.0)
                .unwrap_or(0),
            ring: self.world.get::<KernelRing>(e).map(|r| r.0).unwrap_or(0),
            talents: self
                .world
                .get::<Talents>(e)
                .map(|t| t.0.iter().map(|id| id.to_string()).collect())
                .unwrap_or_default(),
            bought_stats: self
                .world
                .get::<BoughtStats>(e)
                .copied()
                .unwrap_or_default(),
            routines: self
                .world
                .get::<Routines>(e)
                .map(|r| r.0.clone())
                .unwrap_or_default(),
            field_buffs: self
                .world
                .get::<FieldBuff>(e)
                .map(|f| f.active.clone())
                .unwrap_or_default(),
            nest_position,
            patrol_position,
            study_station,
            outpost: self
                .world
                .get::<crate::components::PostedAt>(e)
                .map(|p| p.0),
            siphon,
            jailed,
            pursuing: self.world.get::<Pursuing>(e).is_some(),
            boss: self.world.get::<Boss>(e).is_some(),
            carrying: self
                .world
                .get::<Carrying>(e)
                .map(|c| (c.item.clone(), c.qty)),
            carrying_program: self
                .world
                .get::<crate::components::CarryingProgram>(e)
                .map(|c| c.0.clone()),
            tidying: self
                .world
                .get::<crate::components::Tidying>(e)
                .map(|t| t.0.clone()),
            clearing: self.world.get::<crate::components::Clearing>(e).is_some(),
            rarity: self.world.get::<Rarity>(e).copied().unwrap_or_default(),
            nemesis_grudges: self.world.get::<Nemesis>(e).map(|n| n.0).unwrap_or(0),
            nemesis_muster_ticks: self
                .world
                .get::<crate::components::NemesisMuster>(e)
                .map_or(0, |m| m.ticks),
            nemesis_home: self
                .world
                .get::<crate::components::NemesisHome>(e)
                .map(|h| (h.0.x, h.0.y)),
            nemesis_band: self.nemesis_band_number(e),
            program_id: self.world.get::<ProgramId>(e).map(|p| p.0).unwrap_or(0),
            disposition: self
                .world
                .get::<crate::disposition::Disposition>(e)
                .copied(),
            disgruntled: self
                .world
                .get::<crate::components::Disgruntled>(e)
                .map(|d| d.grievance),
            disgruntled_stranded: self
                .world
                .get::<crate::components::Disgruntled>(e)
                .is_some_and(|d| d.stranded),
            memories: self
                .world
                .get::<Memories>(e)
                .map(|m| {
                    m.0.iter()
                        .map(|m| save::MemorySave {
                            def: m.def.clone(),
                            subject: m.subject.clone(),
                            subject_name: m.subject_name.clone(),
                            reinforced: m.reinforced,
                            strikes: m.strikes,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            conversations: self
                .world
                .get::<crate::components::Conversations>(e)
                .map(|c| c.0.iter().cloned().collect())
                .unwrap_or_default(),
            needs: self
                .world
                .get::<Needs>(e)
                .map(|n| n.iter().map(|(id, v)| (id.clone(), v)).collect())
                .unwrap_or_default(),
            // Absent reads as empty, the shape `needs` above takes for a
            // wild body — and an empty map is what the load path mints
            // from, so a body that somehow has none is repaired rather
            // than stranded.
            attributes: self
                .world
                .get::<crate::components::Attributes>(e)
                .map(|a| a.iter().map(|(id, v)| (id.clone(), v)).collect())
                .unwrap_or_default(),
            off_shift: self
                .world
                .get::<crate::components::OffShift>(e)
                .map(|o| o.need.clone()),
            downed: self.world.get::<crate::components::Downed>(e).is_some(),
            equipment: self
                .world
                .get::<Equipment>(e)
                .map(|eq| {
                    EquipmentSlot::ALL
                        .into_iter()
                        .filter_map(|slot| Some((slot, worn_to_save(&eq.get(slot)?))))
                        .collect()
                })
                .unwrap_or_default(),
            staff: self.program_role(e) == Some(ProgramRole::Staff),
            siege_cell: siege_membership.map(|(_, cell)| cell),
            siege_order: siege_membership.map(|(order, _)| order),
            besieger: self.world.get::<crate::components::Besieger>(e).is_some(),
            stolen_from: self
                .world
                .get::<crate::components::StolenFrom>(e)
                .and_then(|s| self.world.get::<Position>(s.0))
                .map(|p| (p.x, p.y)),
            // Sorted, `descriptions.rs`'s reason every iteration-order-
            // sensitive write in this file follows: a `BTreeSet` already
            // iterates in order, but naming it keeps the byte-identical-save
            // guarantee (`savetool dump`/`pack`) from depending on that
            // being true forever.
            off_duties: self
                .world
                .get::<crate::components::Duties>(e)
                .map(|d| {
                    let mut names: Vec<String> =
                        d.off.iter().map(|duty| duty.name().to_string()).collect();
                    names.sort();
                    names
                })
                .unwrap_or_default(),
            staff_rank: self
                .world
                .get::<crate::components::StaffRank>(e)
                .map(|r| r.0),

            drop_trooper: self
                .world
                .get::<crate::components::DropTrooper>(e)
                .is_some(),
            base,
            stat_points: self
                .world
                .get::<crate::components::StatPoints>(e)
                .map_or(0, |p| p.0),
            hold_points: self
                .world
                .get::<crate::components::HoldPoints>(e)
                .is_some_and(|h| h.0),
        })
    }

    pub(super) fn structure_saves_for(&mut self) -> Vec<save::StructureSave> {
        let mut structures = Vec::new();
        let mut structure_query = self.world.query::<(
            &Structure,
            &Position,
            Option<&Durability>,
            Option<&StructureTier>,
            Option<&Stock>,
            Option<&StandingJob>,
            Option<&crate::components::PowerFuel>,
            Option<&crate::components::Hopper>,
            Option<&crate::components::BuildQuality>,
            Option<&crate::components::DepotFilter>,
            Option<&crate::components::Racked>,
            Option<&crate::components::DropPod>,
            Option<&crate::components::Incubator>,
            Option<&crate::components::MachineRecipe>,
            Option<&crate::components::StudyYield>,
        )>();
        // `Stock` is optional here only because test fixtures hand-spawn
        // bare `Structure`s; `place_structure` and `load` both give every
        // real one a buffer.
        for (
            structure,
            pos,
            durability,
            tier,
            stock,
            standing,
            fuel,
            hopper,
            quality,
            filter,
            racked,
            pod,
            incubator,
            recipe,
            study_yield,
        ) in structure_query.iter(&self.world)
        {
            let encode = |map: Option<&std::collections::BTreeMap<ItemId, u32>>| {
                map.map(|m| m.iter().map(|(i, n)| (i.clone(), *n)).collect())
                    .unwrap_or_default()
            };
            structures.push(save::StructureSave {
                kind: structure.kind.clone(),
                position: (pos.x, pos.y),
                recipe: recipe.and_then(|r| r.0.clone()),
                durability: durability.map(|d| d.hp),
                tier: tier.map(|t| t.0),
                stock_input: encode(stock.map(|s| &s.input)),
                stock_output: encode(stock.map(|s| &s.output)),
                hopper: hopper.map(|h| h.queue.clone()).unwrap_or_default(),
                hopper_progress: hopper.map(|h| h.progress).unwrap_or(0),
                standing_tool: hopper.and_then(|h| h.standing_tool.clone()),
                standing_work: standing.is_some_and(|j| j.work),
                standing_guard: standing.is_some_and(|j| j.guard),
                denied_items: filter
                    .map(|f| f.denied.iter().cloned().collect())
                    .unwrap_or_default(),
                power_fuel: fuel
                    .map(|f| f.ticks_left)
                    .unwrap_or(crate::tuning::POWER_UPKEEP_TICKS),
                build_quality: quality.map_or(1.0, |q| q.0),
                racked: racked.map(|r| r.0.clone()).unwrap_or_default(),
                study_yield: study_yield.and_then(|y| y.0.clone()),
                pod_charged: pod.map(|p| p.charged),
                incubating: incubator.map(|i| i.slots.clone()).unwrap_or_default(),
            });
        }
        structures
    }

    pub(super) fn nest_saves_for(&mut self) -> Vec<save::NestSave> {
        let mut nests = Vec::new();
        let mut nest_query = self.world.query::<(&Nest, &Position, &Durability)>();
        for (nest, pos, durability) in nest_query.iter(&self.world) {
            nests.push(save::NestSave {
                species: nest.species.clone(),
                position: (pos.x, pos.y),
                durability: durability.hp,
                pending_respawns: nest.pending_respawns.clone(),
            });
        }
        nests
    }

    pub(super) fn trap_saves_for(&mut self) -> Vec<save::TrapSave> {
        let mut traps = Vec::new();
        let mut query = self.world.query::<(&crate::components::Trap, &Position)>();
        for (trap, pos) in query.iter(&self.world) {
            traps.push(save::TrapSave {
                item: trap.item.clone(),
                position: (pos.x, pos.y),
                next_roll: trap.next_roll,
                caught: trap.caught.clone(),
            });
        }
        traps
    }

    pub(super) fn dig_site_saves_for(&mut self) -> Vec<save::DigSiteSave> {
        let mut dig_sites = Vec::new();
        let mut dig_query = self.world.query::<(&DigSite, &Position, &Durability)>();
        for (site, pos, durability) in dig_query.iter(&self.world) {
            dig_sites.push(save::DigSiteSave {
                // Field coordinates: a `DigSite` is the one
                // non-`Structure` entity besides a posted program that
                // stands in the base's own space.
                position: (pos.x, pos.y),
                durability: durability.hp,
                marked: site.marked,
                finish: site.finish.clone(),
            });
        }
        dig_sites
    }

    pub(super) fn build_site_saves_for(&mut self) -> Vec<save::BuildSiteSave> {
        let mut build_sites = Vec::new();
        let mut build_query = self.world.query::<(&BuildSite, &Position)>();
        for (site, pos) in build_query.iter(&self.world) {
            build_sites.push(save::BuildSiteSave {
                // Field coordinates, exactly as a `DigSite`'s are.
                position: (pos.x, pos.y),
                structure: site.structure.clone(),
                cost: site.cost.clone(),
                delivered: site.delivered.clone(),
                progress: site.progress,
                goal: site.goal,
                program: site.program.clone(),
                awaiting_program: site.awaiting_program,
                recipe: site.recipe.clone(),
            });
        }
        build_sites
    }

    pub(super) fn caravan_saves_for(&mut self) -> Vec<save::CaravanSave> {
        let mut caravans = Vec::new();
        let mut caravan_query = self.world.query::<(&Caravan, &Position)>();
        for (caravan, pos) in caravan_query.iter(&self.world) {
            caravans.push(save::CaravanSave {
                // The component's own `Position` is already right for the
                // stage (`CaravanStage::inside_the_base`), so this copies
                // rather than decides.
                position: (pos.x, pos.y),
                stage: caravan.stage,
                visit: caravan.visit,
                arrival_tile: caravan.arrival_tile,
                stage_ticks: caravan.stage_ticks,
            });
        }
        caravans
    }

    pub(super) fn player_save_for(&mut self) -> save::PlayerSave {
        let player = self.player_entity();
        let pos = *self.world.get::<Position>(player).unwrap();
        let stats = *self.world.get::<Stats>(player).unwrap();
        let needs = *self.world.get::<PowerReserve>(player).unwrap();
        let exp = *self.world.get::<Experience>(player).unwrap();
        let equipment = self.world.get::<Equipment>(player).unwrap().clone();
        let inventory = self.world.get::<Inventory>(player).unwrap().items.clone();
        let gear_copies = self
            .world
            .get::<GearCopies>(player)
            .map(|f| {
                f.copies
                    .iter()
                    .map(|(copy, qty)| (copy_to_save(copy), *qty))
                    .collect()
            })
            .unwrap_or_default();
        let downed_programs = self
            .world
            .get::<DownedPrograms>(player)
            .map(|d| d.0.clone())
            .unwrap_or_default();
        let tools = self
            .world
            .get::<Tools>(player)
            .map(|t| t.0.clone())
            .unwrap_or_default();
        let implants = self
            .world
            .get::<crate::components::Implants>(player)
            .map(|i| i.installed.clone())
            .unwrap_or_default();
        let phase_keys = self
            .world
            .get::<crate::components::PhaseKeys>(player)
            .copied()
            .unwrap_or_default();
        let perks = self.world.get::<Perks>(player).cloned().unwrap_or_default();
        let bought_stats = self
            .world
            .get::<BoughtStats>(player)
            .copied()
            .unwrap_or_default();
        let routines = self
            .world
            .get::<Routines>(player)
            .map(|r| r.0.clone())
            .unwrap_or_default();
        let field_buffs = self
            .world
            .get::<FieldBuff>(player)
            .map(|f| f.active.clone())
            .unwrap_or_default();
        let glyph = self.world.get::<Glyph>(player).unwrap().ch;
        let identity = self
            .world
            .get::<PlayerIdentity>(player)
            .cloned()
            .unwrap_or_default();
        let name = self
            .world
            .get::<CustomName>(player)
            .map(|n| n.0.clone())
            .unwrap_or_default();

        // Sortie membership is written per *creature*, not here: entity ids
        // are not stable across the round trip, so a member list on the
        // sortie side could not be read back. `SortieSave` carries none —
        // `creature_save_for` answers it from `sortie_index`.
        let sorties: Vec<save::SortieSave> = self
            .world
            .resource::<crate::resources::Sorties>()
            .0
            .iter()
            .map(|s| save::SortieSave {
                site: s.site.clone(),
                risk: s.risk,
                ticks_total: s.ticks_total,
                ticks_elapsed: s.ticks_elapsed,
                battles_total: s.battles_total,
                battles_done: s.battles_done,
                aborted: s.aborted,
                loot: s.loot.clone(),
                programs: s.programs.clone(),
                xp: s.xp,
                kills: s.kills,
                casualties: s.casualties.clone(),
            })
            .collect();
        let attrs: std::collections::BTreeMap<crate::attributes::AttributeId, i32> = self
            .world
            .get::<crate::components::Attributes>(player)
            .map(|a| a.iter().map(|(id, v)| (id.clone(), v)).collect())
            .unwrap_or_default();
        // No membership to gather, `routes::Route`'s own reason: a
        // field-for-field conversion is the whole of it. Settlement routes
        // only — an outpost route saves as `SaveData::outpost_routes`
        // instead, design correction 4's reason (`RouteSave` stays
        // untouched, so widening its `destination` field is not an option).
        let routes: Vec<save::RouteSave> = self
            .world
            .resource::<crate::resources::Routes>()
            .0
            .iter()
            .enumerate()
            .filter_map(|(order, r)| match &r.destination {
                crate::routes::RouteEnd::Settlement { key, def, tile } => Some(save::RouteSave {
                    destination: *key,
                    destination_def: def.clone(),
                    destination_tile: *tile,
                    cargo: r.cargo.clone(),
                    standing: r.standing,
                    stalled: r.stalled,
                    leg: r.leg,
                    ticks_total: r.ticks_total,
                    ticks_elapsed: r.ticks_elapsed,
                    proceeds: r.proceeds,
                    order,
                }),
                crate::routes::RouteEnd::Outpost(_) => None,
            })
            .collect();
        save::PlayerSave {
            position: (pos.x, pos.y),
            hp: stats.hp,
            power: needs.get(),
            stat_points: self
                .world
                .get::<crate::components::StatPoints>(player)
                .map_or(0, |p| p.0),
            inventory,
            level: exp.level,
            xp: exp.xp,
            xp_to_next: exp.xp_to_next,
            weapon: equipment.weapon.as_ref().map(|e| e.copy.item.clone()),
            weapon_level: equipment.weapon.as_ref().map(|e| e.level).unwrap_or(1),
            weapon_fusion_tier: equipment.weapon.as_ref().map(|e| e.copy.tier).unwrap_or(0),
            weapon_rarity: equipment
                .weapon
                .as_ref()
                .map(|e| e.copy.rarity)
                .unwrap_or_default(),
            weapon_affix: None,
            weapon_affixes: equipment
                .weapon
                .as_ref()
                .map(|e| e.copy.affixes.clone())
                .unwrap_or_default(),
            weapon_quality: equipment
                .weapon
                .as_ref()
                .map(|e| e.copy.quality)
                .unwrap_or(crate::tuning::QUALITY_DEFAULT),
            weapon_wear: equipment.weapon.as_ref().map(|e| e.copy.wear).unwrap_or(0),
            armor: equipment.armor.as_ref().map(|e| e.copy.item.clone()),
            armor_level: equipment.armor.as_ref().map(|e| e.level).unwrap_or(1),
            armor_fusion_tier: equipment.armor.as_ref().map(|e| e.copy.tier).unwrap_or(0),
            armor_rarity: equipment
                .armor
                .as_ref()
                .map(|e| e.copy.rarity)
                .unwrap_or_default(),
            armor_affix: None,
            armor_affixes: equipment
                .armor
                .as_ref()
                .map(|e| e.copy.affixes.clone())
                .unwrap_or_default(),
            armor_quality: equipment
                .armor
                .as_ref()
                .map(|e| e.copy.quality)
                .unwrap_or(crate::tuning::QUALITY_DEFAULT),
            armor_wear: equipment.armor.as_ref().map(|e| e.copy.wear).unwrap_or(0),
            module: equipment.module.as_ref().map(|e| e.copy.item.clone()),
            module_level: equipment.module.as_ref().map(|e| e.level).unwrap_or(1),
            module_fusion_tier: equipment.module.as_ref().map(|e| e.copy.tier).unwrap_or(0),
            module_rarity: equipment
                .module
                .as_ref()
                .map(|e| e.copy.rarity)
                .unwrap_or_default(),
            module_affix: None,
            module_affixes: equipment
                .module
                .as_ref()
                .map(|e| e.copy.affixes.clone())
                .unwrap_or_default(),
            module_quality: equipment
                .module
                .as_ref()
                .map(|e| e.copy.quality)
                .unwrap_or(crate::tuning::QUALITY_DEFAULT),
            module_wear: equipment.module.as_ref().map(|e| e.copy.wear).unwrap_or(0),
            // The legacy store is never written again — see its doc in
            // `save/mod.rs`. It is `skip_serializing_if` empty, so a save
            // from here on carries only `gear_copies`.
            fused_gear: Vec::new(),
            gear_copies,
            downed_programs,
            tools,
            implants,
            phase_keys,
            perk_points: perks.points,
            unlocked_perks: perks.unlocked,
            bought_stats,
            tutorial_seeded: true,
            routines,
            field_buffs,
            sorties,
            routes,
            name,
            class: identity.class,
            glyph,
            sprite: identity.sprite,
            colour: identity.colour,
            icon: identity.icon.as_ref().map(PlayerIcon::encode),
            attributes: attrs,
        }
    }

    pub fn save(&mut self, path: &Path) -> std::io::Result<()> {
        let player_save = self.player_save_for();
        // Ids first, then a pass: `creature_save_for` takes `&mut self`, and
        // an open query iteration holds the world borrowed for as long as it
        // runs.
        //
        // **Deliberately unsorted.** This is bevy's archetype order, which is
        // what the hand-rolled loop this replaced emitted — and it is not the
        // order the same roster comes back in after a `Game::load`, because a
        // program's archetype at spawn is not its archetype after a load.
        // Nothing is lost by that; `tests::save_roundtrip` pins the round
        // trip as a multiset of lines and says why. Stabilising the emission
        // order is a change to the save format, not to this loop.
        let creature_ids: Vec<Entity> = {
            let mut q = self.world.query_filtered::<Entity, With<Creature>>();
            q.iter(&self.world).collect()
        };
        let creatures: Vec<save::CreatureSave> = creature_ids
            .into_iter()
            .filter_map(|e| self.creature_save_for(e))
            .collect();

        let structures = self.structure_saves_for();

        let nests = self.nest_saves_for();

        let traps = self.trap_saves_for();

        let dig_sites = self.dig_site_saves_for();

        let farm_plots = self.farm_plot_saves();

        let build_sites = self.build_site_saves_for();

        let caravans = self.caravan_saves_for();

        let tile_overrides = self
            .world
            .resource::<WorldMap>()
            .overrides()
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect();

        let base_grid = self.world.resource::<crate::base_grid::BaseGrid>().clone();

        let data = save::SaveData {
            seed: self.world.resource::<WorldMap>().seed(),
            tick: self.world.resource::<GameClock>().tick,
            difficulty: *self.world.resource::<DifficultyMode>(),
            // Written on every save, not only the one the ending takes:
            // it is `None` for the whole of a live run, so an autosave
            // carries it as truthfully as the seal does.
            game_over: self.is_game_over(),
            player: player_save,
            creatures,
            structures,
            floor_items: self.floor_item_saves(),
            floor_piles: Vec::new(),
            nests,
            traps,
            dig_sites,
            farm_plots,
            build_sites,
            ruins: self
                .world
                .resource::<crate::resources::Ruins>()
                .0
                .iter()
                .map(|r| save::RuinSave {
                    kind: r.kind.clone(),
                    x: r.x,
                    y: r.y,
                    recipe: r.recipe.clone(),
                })
                .collect(),
            caravans,
            caravan_memory: {
                let memory = self.world.resource::<crate::resources::CaravanMemory>();
                save::CaravanMemorySave {
                    visit: memory.visit,
                    bought: memory.bought.iter().copied().collect(),
                }
            },
            tile_overrides,
            base_grid,
            mining: self.world.resource::<crate::resources::MiningMode>().0,
            enemy_strength: self.enemy_strength(),
            free_builds: self
                .world
                .resource::<crate::resources::FreeBuilds>()
                .clone(),
            run_tally: self.world.resource::<crate::resources::RunTally>().clone(),
            anchor: self.anchor_position(),
            zone: self.world.resource::<ZoneLevel>().0,
            spawn_point: {
                let p = self.world.resource::<ZoneSpawnPoint>();
                (p.x, p.y)
            },
            // Never written again — see the field docs in `save/mod.rs`.
            buyback: Vec::new(),
            buyback_shelves: self
                .world
                .resource::<BuybackLedger>()
                .0
                .iter()
                .map(|((kind, tile), shelf)| {
                    let rows = shelf
                        .iter()
                        .map(|(copy, qty)| (copy_to_save(copy), *qty))
                        .collect();
                    (kind.clone(), *tile, rows)
                })
                .collect(),
            researched: {
                let mut ids: Vec<ResearchId> = self
                    .world
                    .resource::<Research>()
                    .0
                    .iter()
                    .cloned()
                    .collect();
                ids.sort();
                ids
            },
            active_research: self
                .world
                .resource::<crate::resources::ActiveResearch>()
                .id
                .clone(),
            research_progress: {
                let mut rows: Vec<(ResearchId, u32)> = self
                    .world
                    .resource::<crate::resources::ActiveResearch>()
                    .progress
                    .iter()
                    .map(|(id, n)| (id.clone(), *n))
                    .collect();
                rows.sort();
                rows
            },
            discovered_research: self
                .world
                .resource::<crate::resources::DiscoveredResearch>()
                .0
                .iter()
                .cloned()
                .collect(),
            study_progress: self
                .world
                .resource::<crate::resources::ActiveResearch>()
                .study,
            known_tools: self
                .world
                .resource::<KnownTools>()
                .0
                .iter()
                .cloned()
                .collect(),
            emulation_images: self
                .world
                .resource::<crate::resources::EmulationImages>()
                .0
                .iter()
                .cloned()
                .collect(),
            crafted_routines: self
                .world
                .resource::<crate::resources::CraftedRoutines>()
                .built
                .clone(),
            unslotted_routines: self
                .world
                .resource::<crate::resources::CraftedRoutines>()
                .unslotted
                .clone(),
            next_crafted_id: self
                .world
                .resource::<crate::resources::CraftedRoutines>()
                .next_id,
            ftue_played: self
                .world
                .resource::<crate::ftue::FtuePlayed>()
                .0
                .iter()
                .cloned()
                .collect(),
            gear_designs: self
                .world
                .resource::<crate::resources::GearDesigns>()
                .designs
                .clone(),
            next_gear_design_id: self
                .world
                .resource::<crate::resources::GearDesigns>()
                .next_id,
            known_affixes: self
                .world
                .resource::<crate::resources::KnownAffixes>()
                .0
                .iter()
                .cloned()
                .collect(),
            link_sites: {
                let mut query = self.world.query_filtered::<&Position, With<SurfaceLink>>();
                query.iter(&self.world).map(|p| (p.x, p.y)).collect()
            },
            locale: self.locale(),
            stack_memory: self.world.resource::<StackMemory>().clone(),
            stack_memory_tiered: true,
            entered_tiers: self
                .world
                .resource::<crate::resources::EnteredTiers>()
                .clone(),
            settlements: self
                .world
                .resource::<crate::resources::Settlements>()
                .clone(),
            standings: self.world.resource::<crate::resources::Standings>().clone(),
            compass: self
                .world
                .resource::<crate::resources::CompassBearing>()
                .clone(),
            populated_chunks: self
                .world
                .resource::<crate::resources::PopulatedChunks>()
                .clone(),
            explored_chunks: self
                .world
                .resource::<crate::resources::ExploredChunks>()
                .clone(),
            seen_tiles: self.world.resource::<crate::resources::SeenTiles>().clone(),
            trace: self.trace(),
            raid_pressure: *self.world.resource::<crate::resources::RaidPressure>(),
            siege_pressure: *self.world.resource::<crate::resources::SiegePressure>(),
            siege: crate::game::siege::persist::assemble(self),
            contracts: self
                .world
                .resource::<crate::resources::ActiveContracts>()
                .active
                .clone(),
            contracts_done: self
                .world
                .resource::<crate::resources::ActiveContracts>()
                .done
                .clone(),
            work_orders: self.work_orders().to_vec(),
            alerts: self
                .world
                .resource::<crate::alerts::AlertBoard>()
                .alerts
                .iter()
                .cloned()
                .collect(),
            next_program_id: self.world.resource::<crate::resources::NextProgramId>().0,
            base_ledger: self
                .world
                .resource::<crate::base_ledger::BaseLedger>()
                .clone(),
            outposts: self
                .world
                .resource::<crate::resources::Outposts>()
                .0
                .iter()
                .map(|(&tile, o)| save::OutpostSave {
                    tile,
                    biome: o.biome,
                    growth: o.growth,
                    integrity: o.integrity,
                    stock: o.stock.iter().map(|(id, &n)| (id.clone(), n)).collect(),
                    stale_ticks: o.stale_ticks,
                    cycle_progress: o.cycle_progress,
                })
                .collect(),
            outpost_routes: self
                .world
                .resource::<crate::resources::Routes>()
                .0
                .iter()
                .enumerate()
                .filter_map(|(order, r)| match &r.destination {
                    crate::routes::RouteEnd::Outpost(tile) => Some(save::OutpostRouteSave {
                        tile: *tile,
                        cargo: r.cargo.clone(),
                        standing: r.standing,
                        stalled: r.stalled,
                        leg: r.leg,
                        ticks_total: r.ticks_total,
                        ticks_elapsed: r.ticks_elapsed,
                        order,
                    }),
                    crate::routes::RouteEnd::Settlement { .. } => None,
                })
                .collect(),
        };
        save::save_to_file(path, &data)
    }
}
