//! `Game::load`: a save file back into a live world.

use super::convert::copy_from_save;
use super::spawn_saved::{CreatureRestore, spawn_player_from_save};
use super::{AssetDbs, load_asset_dbs};
use crate::abilities::AbilityId;
use crate::routine_craft::{CraftedRoutine, validate_build};
use crate::tuning::{CRAFT_ID_PREFIX, CRAFT_PART_CAP_MAX};
use crate::*;

/// Splits a persisted routine list into what `db` still recognizes and what
/// it doesn't — `Game::load`'s only chance to catch an id a save carries
/// that the loaded `AbilityDb` no longer has (the ability file was removed,
/// or is now malformed and got skipped with its own warning). Left
/// unfiltered, a ghost id survives into `Routines` as an entry
/// `routine_view` can't resolve: the slot renders `(empty)`, so the install
/// picker opens over an occupied one instead of the ghost ever being
/// uninstallable.
pub(super) fn recognized_routines(
    ids: &[AbilityId],
    db: &AbilityDb,
) -> (Vec<AbilityId>, Vec<AbilityId>) {
    ids.iter().cloned().partition(|id| db.get(id).is_some())
}

/// The built routines a save may keep: a record that fails `validate_build`
/// (a Core-less one would panic `compile`) is dropped with a warning, and the
/// slots that named it empty themselves through `recognized_routines`.
///
/// The counter is raised past every id the save holds, so a save whose
/// counter was lost or defaulted cannot mint an id a routine still names.
fn sound_crafted_routines(
    saved: Vec<CraftedRoutine>,
    saved_unslotted: Vec<AbilityId>,
    saved_next_id: u32,
    warnings: &mut Vec<String>,
) -> (Vec<CraftedRoutine>, Vec<AbilityId>, u32) {
    let highest_used = saved
        .iter()
        .filter_map(|r| r.id.strip_prefix(CRAFT_ID_PREFIX)?.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    let (kept, dropped): (Vec<CraftedRoutine>, Vec<CraftedRoutine>) = saved
        .into_iter()
        .partition(|r| validate_build(&r.parts, CRAFT_PART_CAP_MAX).is_ok());
    for routine in &dropped {
        warnings.push(format!(
            "Built routine {} is malformed and was dropped, and its slot is now empty.",
            routine.name
        ));
    }
    let unslotted = saved_unslotted
        .into_iter()
        .filter(|id| kept.iter().any(|r| r.id == *id))
        .collect();
    (kept, unslotted, saved_next_id.max(highest_used))
}

impl Game {
    pub fn load(path: &Path, assets_dir: &Path) -> std::io::Result<Self> {
        let mut data = save::load_from_file(path)?;
        // Permadeath's one guarantee, and it is enforced here rather than in
        // `load_from_file` on purpose: the load list reads a dead slot to
        // label it, and `savetool` has to be able to dump one. The refusal
        // is the game's, not the format's.
        if let Some(reason) = &data.game_over {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("that run is over — it {reason}. Its signal is gone from the Basin."),
            ));
        }
        // Every door seats a tamed program and the writer always records its
        // `base`, so a tamed record without one cannot have come from this
        // build; loading it would read its stats as zero.
        let held = data.build_sites.iter().filter_map(|b| b.program.as_ref());
        if let Some(c) = data
            .creatures
            .iter()
            .chain(held)
            .find(|c| c.tamed && c.base.is_none())
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "save is malformed: owned program {:?} has no base to derive its stats from",
                    c.species
                ),
            ));
        }
        let AssetDbs {
            abilities: mut ability_db,
            tools: tool_db,
            classes: class_db,
            achievements: achievement_db,
            contracts: contract_db,
            descriptions: description_db,
            memories: memory_db,
            statuses: status_db,
            implants: implant_db,
            phase_keys: phase_key_db,
            thoughts: thought_db,
            interactions: interaction_db,
            needs: need_db,
            attributes: attribute_db,
            sorties: sortie_db,
            caravans: caravan_db,
            rock: rock_db,
            floors: floor_db,
            crops: crop_db,
            rooms: room_db,
            props: prop_db,
            nemesis: nemesis_db,
            species: species_db,
            structures: structure_db,
            research: research_db,
            items: mut item_db,
            perks: perk_db,
            talents: talent_db,
            affixes: affix_db,
            settlements: settlement_db,
            outposts: outpost_db,
            policy: enemy_policy,
            behaviours,
            warnings: mut load_warnings,
        } = load_asset_dbs(assets_dir)?;

        // Both inputs come off the save, which is the whole reason a sector
        // needs no field of its own. Rebuilding at a different shape would
        // regenerate every unwalked chunk differently and could strand the
        // party inside rock.
        let mut world_map = WorldMap::new(data.seed);
        let overrides: HashMap<(i32, i32), Tile> = data.tile_overrides.into_iter().collect();
        world_map.restore_overrides(overrides);

        // A routine slot is persisted as a raw ability id and never
        // validated, so a save made under one mod configuration can name an
        // ability a later load's asset set doesn't have (the file was
        // removed, or is now malformed and got skipped with a warning of
        // its own). Left in place, `routine_view` can't resolve the id and
        // renders the slot `(empty)` — which then makes `install_built`
        // refuse "no free slot" for a slot the panel just told the player
        // was free. Dropped here instead, once, at the only point both the
        // save data and the loaded `AbilityDb` are in hand.
        // Taken before the bundle moves `unlocked_perks` out of `data`.
        let player_perk_levels = data.player.unlocked_perks.len() as u32;
        // Before any slot is recognised: a `crafted/<n>` id resolves only
        // once the crafted tier holds it.
        let (crafted_routines, unslotted_routines, next_crafted_id) = sound_crafted_routines(
            data.crafted_routines,
            data.unslotted_routines,
            data.next_crafted_id,
            &mut load_warnings,
        );
        ability_db.set_crafted(&crafted_routines);
        let (player_routines, mut dropped_player_routines) =
            recognized_routines(&data.player.routines, &ability_db);
        // A dropped built routine was already reported by name above; its
        // raw id means nothing to the player.
        dropped_player_routines.retain(|id| !id.starts_with(CRAFT_ID_PREFIX));
        // Before inventory, equipment, companions and shelves restore: a
        // `crafted_gear/<n>` id resolves only once the crafted tier holds it.
        load_warnings.extend(item_db.set_crafted_gear(&data.gear_designs, &affix_db));
        let next_gear_design_id = data
            .gear_designs
            .iter()
            .map(|d| d.id)
            .fold(data.next_gear_design_id, u32::max);
        let mut world = World::new();
        world.insert_resource(ability_db);
        world.insert_resource(tool_db);
        world.insert_resource(class_db);
        world.insert_resource(species_db);
        world.insert_resource(structure_db);
        world.insert_resource(research_db);
        world.insert_resource(item_db);
        world.insert_resource(perk_db);
        world.insert_resource(talent_db);
        world.insert_resource(affix_db);
        world.insert_resource(settlement_db);
        world.insert_resource(outpost_db);
        world.insert_resource(enemy_policy);
        world.insert_resource(behaviours);
        world.insert_resource(description_db);
        world.insert_resource(memory_db);
        world.insert_resource(status_db);
        world.insert_resource(implant_db);
        world.insert_resource(phase_key_db);
        world.insert_resource(thought_db);
        world.insert_resource(interaction_db);
        world.insert_resource(need_db);
        world.insert_resource(attribute_db);
        world.insert_resource(crate::resources::Notifications::default());
        world.insert_resource(sortie_db);
        world.insert_resource(caravan_db);
        world.insert_resource(rock_db);
        world.insert_resource(nemesis_db);
        world.insert_resource(world_map);
        world.insert_resource(GameClock { tick: data.tick });
        world.insert_resource(GameRng(StdRng::seed_from_u64(data.seed as u64 ^ data.tick)));
        world.insert_resource(MessageLog::default());
        world.insert_resource(BattleTimeline::default());
        world.insert_resource(EffectQueue::default());
        // Not a save field: a brawl lasts four to eight ticks and damage is
        // applied as it goes, so a save mid-fight loses only the summary.
        world.init_resource::<crate::resources::Brawls>();
        world.init_resource::<crate::resources::PendingProgramLevels>();
        world.init_resource::<crate::resources::RefitStand>();
        world.insert_resource(TransitQueue::default());
        world.init_resource::<crate::resources::SpeechQueue>();
        world.insert_resource(BoltQueue::default());
        world.insert_resource(TacticalFxQueue::default());
        world.insert_resource(crate::resources::SwingCueQueue::default());
        world.insert_resource(GameOver::default());
        world.insert_resource(data.difficulty);
        world.insert_resource(Party::default());
        world.insert_resource(WieldedProgram::default());
        world.insert_resource(Research(data.researched.into_iter().collect()));
        world.insert_resource(crate::resources::ActiveResearch {
            id: data.active_research,
            progress: data.research_progress.into_iter().collect(),
            study: data.study_progress,
        });
        world.insert_resource(crate::resources::DiscoveredResearch(
            data.discovered_research.into_iter().collect(),
        ));
        world.insert_resource(KnownTools(data.known_tools.into_iter().collect()));
        world.insert_resource(crate::resources::GearDesigns {
            designs: data.gear_designs,
            next_id: next_gear_design_id,
        });
        world.insert_resource(crate::resources::KnownAffixes(
            data.known_affixes.into_iter().collect(),
        ));
        world.insert_resource(crate::resources::CraftedRoutines {
            built: crafted_routines,
            unslotted: unslotted_routines,
            next_id: next_crafted_id,
        });
        world.insert_resource(BuybackLedger({
            // The pre-0.8.9 shelves are drained into the current shape here
            // and read nowhere else, exactly as `fused_gear` is above.
            let legacy = data.buyback.into_iter().map(|(kind, tile, shelf)| {
                let rows = shelf
                    .into_iter()
                    .map(|(item, tier, qty)| {
                        (
                            // These copies predate both the affix and the
                            // quality fields by two years of releases.
                            GearCopy {
                                item,
                                rarity: Rarity::Ordinary,
                                tier,
                                affixes: Vec::new(),
                                quality: crate::tuning::QUALITY_DEFAULT,
                                wear: 0,
                            },
                            qty,
                        )
                    })
                    .collect();
                ((kind, tile), rows)
            });
            legacy
                .chain(data.buyback_shelves.into_iter().map(|(kind, tile, shelf)| {
                    let rows = shelf
                        .into_iter()
                        .map(|(copy, qty)| (copy_from_save(copy), qty))
                        .collect();
                    ((kind, tile), rows)
                }))
                .collect()
        }));
        world.insert_resource(ZoneLevel(data.zone));
        // Pruned against the loaded catalogue right before the grid becomes
        // the live resource: a mod that renamed or deleted a finish, or a
        // cell that stopped being floor, must not leave a dangling entry
        // behind — `BaseGrid::revert`'s own rule, applied once at load.
        load_warnings.extend(data.base_grid.prune_finishes(&floor_db));
        world.insert_resource(floor_db);
        world.insert_resource(crop_db);
        world.insert_resource(room_db);
        world.insert_resource(prop_db);
        world.insert_resource(data.base_grid);
        world.insert_resource(crate::resources::MiningMode(data.mining));
        world.insert_resource(data.enemy_strength);
        world.insert_resource(data.free_builds);
        world.insert_resource(data.run_tally);
        world.insert_resource(Locale::default());
        world.insert_resource(CurrentStack::default());
        world.insert_resource(StackMemory::default());
        world.init_resource::<crate::resources::EnteredTiers>();
        world.insert_resource(crate::resources::PopulatedChunks::default());
        world.insert_resource(crate::resources::ExploredChunks::default());
        world.insert_resource(crate::resources::SeenTiles::default());
        world.insert_resource(crate::resources::Settlements::default());
        world.insert_resource(crate::resources::Outposts::default());
        world.insert_resource(crate::resources::Standings::default());
        world.insert_resource(crate::resources::PendingVisit::default());
        world.insert_resource(crate::resources::CompassBearing::default());
        world.insert_resource(crate::resources::EmulationImages(
            data.emulation_images.into_iter().collect(),
        ));
        world.insert_resource(crate::resources::PendingEmulateImage::default());
        world.insert_resource(crate::resources::PendingTeleportSubject::default());
        // Empty at both doors. `Game::load` refills it from the save below,
        // once every creature has an entity to name — see `SortieSave`.
        world.insert_resource(crate::resources::Sorties::default());
        // Empty at both doors too, `Sorties`' reason, though a route needs
        // no creature to have spawned first — see `save::RouteSave`.
        world.insert_resource(crate::resources::Routes::default());
        world.insert_resource(crate::resources::Trace::default());
        world.init_resource::<crate::resources::RaidPressure>();
        world.init_resource::<crate::resources::SiegePressure>();
        world.init_resource::<crate::resources::Ruins>();
        world.insert_resource(crate::resources::RunFeats::default());
        world.insert_resource(crate::resources::SeenConditions::default());
        // `RunFeats`' precedent: not saved, so quitting between a level-up
        // and the summary page loses the page, never the level itself —
        // `Experience`/`Stats` already carry the real gain to disk. Both
        // doors anyway, `PowerGrid`'s reason below, so a reader that beats
        // the first `award_player_xp` call finds an empty resource rather
        // than a missing one.
        world.insert_resource(crate::resources::PendingLevelUp::default());
        // Both doors, like `RunFeats` beside it, and empty at both. Nothing
        // restores this from a save because nothing saves it: it is a
        // per-tick cache of `game::base::power::ledger`, and
        // `systems::power_grid_system` fills it in at the head of the first
        // tick either constructor's schedule runs. It is inserted here only
        // so a reader that beats that tick — the base pane's grid header, on
        // the frame a load finishes — finds an empty grid rather than a
        // missing resource.
        world.insert_resource(crate::resources::PowerGrid::default());
        world.insert_resource(achievement_db);
        world.insert_resource(contract_db);
        world.insert_resource(crate::resources::ActiveContracts {
            active: data.contracts,
            done: data.contracts_done,
        });
        world.insert_resource(crate::resources::WorkOrders(data.work_orders));
        world.insert_resource({
            // The latch starts false on every load, `AlertBoard`'s own
            // doc: at worst a reload re-posts one alert the latch would
            // otherwise have suppressed, collapsed into the existing entry.
            let mut board = crate::alerts::AlertBoard {
                alerts: data.alerts.into_iter().collect(),
                depots_full: false,
            };
            crate::alerts::cap(&mut board);
            board
        });
        // See `Game::new`'s copy: both doors, and nothing restores it.
        world.insert_resource(crate::resources::LabourDemand::default());
        // Empty on purpose in *both* constructors. What has actually been
        // earned is installed afterwards by `install_profile`, which app-core
        // calls on either path; paying for it is a separate call it makes on
        // only one. See `grant_profile_rewards`.
        world.insert_resource(crate::achievements::Profile::default());
        world.insert_resource(crate::resources::PendingProfileWrites::default());
        world.insert_resource(crate::ftue::FtuePlayed(
            data.ftue_played.iter().cloned().collect(),
        ));
        world.insert_resource(crate::ftue::PendingCrafted::default());
        // See `Game::new`'s copy: both doors, and nothing restores it.
        world.insert_resource(crate::resources::BattleTelemetry::default());
        world.insert_resource(data.base_ledger.clone());
        world.insert_resource(ZoneSpawnPoint {
            x: data.spawn_point.0,
            y: data.spawn_point.1,
        });

        // Taken off `data.player` before the spawn consumes it. Sortie
        // membership is reconciled after the creature loop, which cannot run
        // until the player exists; the other two are read at the tail.
        let player_name = data.player.name.clone();
        let saved_sorties = std::mem::take(&mut data.player.sorties);
        let saved_routes = std::mem::take(&mut data.player.routes);
        let tutorial_seeded = data.player.tutorial_seeded;
        let saved_attributes = std::mem::take(&mut data.player.attributes);
        let saved_stat_points = data.player.stat_points;
        let player_class = data.player.class;
        let player =
            spawn_player_from_save(&mut world, data.player, player_routines, player_perk_levels);
        world.insert_resource(PlayerEntity(player));
        // `spawn_player_from_save` is a free fn over `&mut World` with no
        // `ClassDb` and no `AttributeDb`, exactly like `spawn_player`, so
        // the player's half of the mint lands here — the same shape the
        // creature loop uses, one level out. An empty map is what a save
        // written before this feature carries, and minting on load rather
        // than on first read is what keeps `Game::dossier_report` a `&self`
        // derivation.
        let player_attributes = if saved_attributes.is_empty() {
            let authored = player_class
                .and_then(|class| {
                    world
                        .resource::<crate::classes::ClassDb>()
                        .get(class)
                        .map(|def| def.attributes.clone())
                })
                .unwrap_or_default();
            crate::attributes::authored_or_base(
                world.resource::<crate::attributes::AttributeDb>(),
                &authored,
            )
        } else {
            let mut restored = crate::components::Attributes::default();
            for (id, value) in &saved_attributes {
                restored.set(id, *value);
            }
            restored
        };
        world.entity_mut(player).insert((
            player_attributes,
            crate::components::Derived::default(),
            crate::components::StatPoints(saved_stat_points),
        ));

        if let Some(name) = CustomName::sanitize(Some(player_name)) {
            world.entity_mut(player).insert(CustomName(name));
        }

        // The anchor's saved position, or — only for a save written before
        // this field existed, since `Game::save` always writes `Some` from
        // here on — the zone spawn point it started at. **Not** a
        // derivation from `data.spawn_point` in the normal case: a load
        // path that quietly recomputed the anchor from the spawn point
        // instead of trusting `data.anchor` would agree with this fallback
        // whenever the two happen to coincide (which is most of the time,
        // and always at `(0, 0)`) and only disagree once something has
        // moved the anchor independently of where the party breaches in —
        // exactly the kind of bug this repo has been bitten by before for
        // looking cheaper than threading the persisted field through.
        let anchor_pos = data.anchor.unwrap_or(data.spawn_point);
        let anchor = world
            .spawn((
                BaseAnchor,
                Position {
                    x: anchor_pos.0,
                    y: anchor_pos.1,
                },
                // See `Game::new`'s copy of this spawn for why `#`/Gray
                // rather than an arrow character or Blue/Cyan.
                Glyph {
                    ch: '#',
                    color: GlyphColor::Gray,
                },
            ))
            .id();
        world.insert_resource(AnchorEntity(anchor));

        let schedule = Self::build_schedule();

        let mut game = Self { world, schedule };
        // Derived values are not trusted from the file: gear, `BoughtStats`
        // and the attributes are, and the stats are rebuilt from them.
        let player = game.player_entity();
        game.recompute_derived(player);
        for warning in load_warnings {
            game.log(warning);
        }
        if !dropped_player_routines.is_empty() {
            game.log(format!(
                "Your installed routines included {} — no longer available, and the slot is now empty.",
                dropped_player_routines.join(", ")
            ));
        }

        // Spawned before the creature loop below so a guardian's
        // `nest_position` has a live nest to resolve to — mirrors
        // `structure_positions` further down, built for the same reason.
        //
        // The bundle itself comes from `nest_components` (`game/spawning.rs`),
        // shared with `Game::spawn_nest` — see that function's doc comment,
        // which is the other half of this note.
        let nest_positions = game.restore_nests(data.nests);

        game.restore_traps(data.traps);

        game.restore_caravans(data.caravan_memory, data.caravans);

        game.restore_build_sites(data.build_sites);

        game.world.insert_resource(crate::resources::Ruins(
            data.ruins
                .into_iter()
                .map(|r| crate::resources::Ruin {
                    kind: r.kind,
                    x: r.x,
                    y: r.y,
                    recipe: r.recipe,
                })
                .collect(),
        ));

        game.restore_dig_sites(data.dig_sites);
        game.restore_farm_plots(data.farm_plots);

        // At most one creature may claim the weapon hand. Taken defensively
        // — the first wins and any others are ignored — rather than trusting
        // the file, the same way `party_slots` is truncated below.
        let mut wielded: Option<Entity> = None;
        // Computed before the loop below because the loop *spends* it:
        // `CreatureRestore` is seeded with this number and every
        // `spawn_creature_from_save` call takes the next id from that seed,
        // so a maximum taken afterwards would arrive too late to have named
        // anything. It is a scan of the whole array and not of one row for
        // the same reason — the seed has to clear every id in the file
        // before the first creature is spawned. The
        // saved counter alone is not enough: a hand-edited or
        // savetool-packed file can carry ids it never saw, and reissuing one
        // makes two programs answer to the same name. The `.max(1)` is not
        // redundant with the sentinel — an empty roster gives 1 on its own,
        // but a legacy file's `next_program_id` of 0 would otherwise win.
        let next_program_id = data
            .next_program_id
            .max(
                data.creatures
                    .iter()
                    .map(|c| c.program_id)
                    .max()
                    .unwrap_or(0)
                    + 1,
            )
            .max(1);
        // The per-creature state the loop below used to thread through its
        // own iterations, lifted into `CreatureRestore` so the body could
        // become a function. See that struct's doc for what is in it, and
        // for why the wield is not.
        let mut restore = CreatureRestore::new(player, next_program_id, nest_positions);
        for c in &data.creatures {
            // `None` is the `continue` this loop used to spell inline: a file
            // naming a species whose `.ron` has been deleted since it was
            // written loads without that creature rather than failing
            // outright.
            let Some(creature_id) = game.spawn_creature_from_save(c, &mut restore) else {
                continue;
            };
            // Resolved out here rather than inside the restore, because "at
            // most one may claim the weapon hand and the first in the file
            // wins" is a decision about the array and not about one row.
            // `c.tamed` is part of it: the lifted body only ever reached
            // this test inside its tamed branch, so a hand-edited file
            // marking a *wild* creature as wielded was ignored before and
            // has to stay ignored.
            if c.tamed && c.wielded && wielded.is_none() {
                wielded = Some(creature_id);
            }
        }
        let CreatureRestore {
            next_program_id,
            mut party_slots,
            sortie_members,
            pending_cronjobs,
            pending_patrols,
            pending_study,
            pending_outpost_crew,
            pending_siphon,
            pending_jailed,
            pending_siege_members,
            pending_nemesis_band,
            pending_stolen_from,
            pending_ranks,
            max_rank_seen,
            ..
        } = restore;
        // One past the highest rank the file named, assigned in the order
        // these programs were written — a save with no ranks at all (every
        // file before this feature) starts every one of them at 1, in file
        // order, since `max_rank_seen` stays 0 with nothing to raise it.
        for (rank, e) in (max_rank_seen + 1..).zip(pending_ranks) {
            game.world
                .entity_mut(e)
                .insert(crate::components::StaffRank(rank));
        }
        game.world
            .insert_resource(crate::resources::NextProgramId(next_program_id));
        party_slots.sort_by_key(|&(slot, _)| slot);
        let mut party: Vec<Entity> = party_slots.into_iter().map(|(_, e)| e).collect();
        party.truncate(MAX_PARTY_SIZE);
        game.world.insert_resource(Party(party));
        // `enter_next_zone` must not wipe this: the program travels with you
        // across a breach exactly as the party does.
        game.world.insert_resource(WieldedProgram(wielded));
        game.restore_sorties(saved_sorties, &sortie_members);
        // Merged by `order` and not simply concatenated — `restore_routes`'
        // doc comment on why a settlement/outpost split needs the seam.
        let mut ordered_routes = game.restore_routes(saved_routes);
        ordered_routes
            .extend(game.restore_outpost_routes(std::mem::take(&mut data.outpost_routes)));
        ordered_routes.sort_by_key(|(order, _)| *order);
        game.world.insert_resource(crate::resources::Routes(
            ordered_routes.into_iter().map(|(_, r)| r).collect(),
        ));

        let (structure_positions, structure_warnings) = game.restore_structures(data.structures);
        for warning in structure_warnings {
            game.log(warning);
        }
        game.restore_floor_items(
            std::mem::take(&mut data.floor_items),
            std::mem::take(&mut data.floor_piles),
        );

        game.attach_cronjobs(pending_cronjobs, &structure_positions);
        game.attach_pinned_subjects(pending_study, &structure_positions);
        game.attach_siphoned(pending_siphon, &structure_positions);
        game.attach_jailed(pending_jailed, &structure_positions);
        game.attach_stolen_from(pending_stolen_from, &structure_positions);
        game.link_nemesis_bands(pending_nemesis_band);
        // After structures load, `attach_stolen_from`'s own reason: a
        // siege re-seats every structure still standing as a body on the
        // reconstructed board, `Game::open_siege`'s own placement.
        if let Some(siege_save) = data.siege.take() {
            crate::game::siege::persist::restore(&mut game, siege_save, &pending_siege_members);
        }

        game.restore_surface_links(data.link_sites);
        // Before `restore_locale`, which records what the party can see from
        // where they are standing and would otherwise write into a map that
        // is about to be overwritten.
        // A `FrameMemory` written before `FrameSpec::rng_seed` folded in
        // `tier` describes frames this build no longer generates, so it is
        // dropped rather than restored — see `SaveData::stack_memory_tiered`.
        game.world.insert_resource(if data.stack_memory_tiered {
            data.stack_memory
        } else {
            crate::resources::StackMemory::default()
        });
        game.world.insert_resource(data.entered_tiers);
        // **Before `restore_settlements`, not after.** That call draws every
        // known town's glyph through `Game::settlement_kind`, which reads
        // `Relation::grown` out of this resource. Restored after, every
        // grown city would come back from the save drawing the `s` its
        // catalogue authored — correct all run, wrong on every load.
        game.world.insert_resource(data.standings);
        game.world.insert_resource(data.populated_chunks);
        game.world.insert_resource(data.explored_chunks);
        game.world.insert_resource(data.seen_tiles);
        game.restore_settlements(data.settlements);
        game.restore_outposts(data.outposts);
        game.attach_outpost_crew(pending_outpost_crew);
        game.reseed_outpost_announcements();
        // After the towns exist, and the one place a patrol's tether is
        // rebuilt. A tile naming no town — the settlement catalogue was
        // edited between sessions — drops the tether silently rather than
        // failing the load, `nest_position`'s rule: the program comes back
        // as ordinary wild, which is exactly what standing its town down
        // would have left it as. `Pursuing` goes with it and never alone.
        // Filtered to the centre even though an outer cell could never
        // collide with it by tile — `settlement_entity`'s reason, so the two
        // doors onto "which entity is this settlement" read as one rule.
        let towns_by_tile: HashMap<(i32, i32), Entity> = {
            let mut query = game.world.query_filtered::<(
                Entity,
                &crate::components::Settlement,
                &Position,
            ), With<crate::components::SettlementCentre>>();
            query
                .iter(&game.world)
                .map(|(entity, _, pos)| ((pos.x, pos.y), entity))
                .collect()
        };
        for (member, tile, pursuing) in pending_patrols {
            let Some(&town) = towns_by_tile.get(&tile) else {
                continue;
            };
            let mut entity = game.world.entity_mut(member);
            entity.insert(TownPatrol { town });
            if pursuing {
                entity.insert(Pursuing);
            }
        }
        game.world.insert_resource(data.compass);
        game.world
            .insert_resource(crate::resources::Trace(data.trace));
        game.world.insert_resource(data.raid_pressure);
        game.world.insert_resource(data.siege_pressure);
        // Last, and after the WorldMap is in place: restoring a Stack
        // locale regenerates its frame from that map's seed.
        let locale = data.locale;
        game.restore_locale(locale);

        // After the locale, and after every occupant a footprint's growth
        // could displace — the player, every wild creature, nest, trap,
        // caravan and the anchor are all restored above this line.
        // `restore_settlements` spawned only each town's centre; growing the
        // rest of the square here, once, is what keeps a Stack entrance's
        // deferral (`sync_settlement_footprint`'s doc) reading the party's
        // real `Locale` instead of the constructor's default `Surface`.
        let settlement_keys: Vec<crate::settlements::SettlementKey> = game
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .keys()
            .copied()
            .collect();
        for key in settlement_keys {
            game.sync_settlement_footprint(key);
        }

        game.log("Session restored. Reconnecting to the Basin.");
        // A save from before the chain existed: file every mission as
        // finished so an established run is left alone. New runs are seeded
        // by `Game::new`, which sets the flag, so this fires exactly once
        // and only for a save the previous build wrote.
        if !tutorial_seeded {
            let ids: Vec<crate::contracts::ContractId> = game
                .world
                .resource::<crate::contracts::ContractDb>()
                .tutorial_chain()
                .iter()
                .map(|d| d.id.clone())
                .collect();
            let mut held = game
                .world
                .resource_mut::<crate::resources::ActiveContracts>();
            for id in ids {
                if !held.done.contains(&id) {
                    held.done.push(id);
                }
            }
        }
        // A save taken mid-chain resumes with no seeding path of its own —
        // the position is derived from the `done` list the save carries.
        game.ensure_tutorial_held();
        // A run saved before research became a project carries a bank of the
        // research currency with nothing left to spend it on. The stock block
        // folds in every `ItemDef::banked` pool **by the flag**, so a leftover
        // pool would sit over the map for the rest of the run — the fold itself stays, because it names no item and a mod
        // may ship another banked one.
        let stranded = game.banked(&game.research_currency());
        if stranded > 0 {
            let currency = game.research_currency();
            let name = game.item_name(&currency).to_string();
            let player = game.player_entity();
            if let Some(mut inventory) = game.world.get_mut::<Inventory>(player) {
                inventory.take(currency, stranded);
            }
            game.log(format!(
                "Your {stranded} banked {name} is written off — research runs on projects now."
            ));
        }
        // A save's `active_research` can name a node the loaded tree no
        // longer has — task 3 deleted nine, and a mod can delete or rename
        // its own — and nothing ever re-selects on its own, so a stale
        // project would stall forever with a Research Node staffed and
        // earning nothing. Checked once, here, rather than left to
        // `select_research`, which only ever refuses a *new* selection.
        let active_stale = game
            .world
            .resource::<crate::resources::ActiveResearch>()
            .id
            .as_ref()
            .is_some_and(|id| game.world.resource::<ResearchDb>().get(id).is_none());
        if active_stale {
            game.world
                .resource_mut::<crate::resources::ActiveResearch>()
                .id = None;
            game.withdraw_research_orders();
        }
        // **A derived fallback, not a scheduler run.** `schedule_base_labour`
        // only runs from `tick`, so `resources::LabourDemand` is still its
        // `Default` — every figure zero — until the first tick, and the Base
        // staff screen reads it if the player opens it before spending one.
        // Calling `schedule_base_labour` here instead was considered and
        // rejected: `run_tantrums`' `open_brawls` draws `GameRng` for any
        // body already on the rung, and nothing else runs between a load and
        // the game's own first tick to change that body's state — so a save
        // with a disgruntled program would draw that roll twice for the same
        // instant, once here and once when the first real tick calls the
        // scheduler again, an `GameRng`-stream shift the load path must not
        // cause. `staff`/`on_shift` alone is cheap and pure to derive —
        // `is_on_shift` reads no RNG and writes nothing — so it is filled in
        // here and `wanted`/`unworked` are left at zero for the first tick to
        // fill in, same as they always have been.
        let staff = game.base_staff();
        let amenities = game.amenities();
        let on_shift = staff
            .iter()
            .filter(|&&w| game.is_on_shift(w, &amenities))
            .count();
        game.world
            .resource_mut::<crate::resources::LabourDemand>()
            .staff = on_shift;
        game.assess_situations();
        game.mark_seen_tiles();
        Ok(game)
    }
}
