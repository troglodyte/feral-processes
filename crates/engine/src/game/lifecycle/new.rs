//! `Game::new`/`new_with`: the fresh-run door.

use super::{AssetDbs, load_asset_dbs};
use crate::components::Tools;
use crate::game::zone::find_walkable_start;
use crate::tools::ToolId;
use crate::tuning::{STACK_LINKS_PER_ZONE, STARTER_TOOL_ID};
use crate::*;

/// The player's `world.spawn` bundle, shared by every constructor — every
/// field here is a neutral default. `Game::new_with` layers a
/// `CharacterChoice` on top afterwards, via `Game::apply_character_choice`,
/// one field at a time: the glyph, the `Inventory` and the free routine
/// slot all spawn empty or hardcoded-neutral here and are only ever
/// overwritten, never read, before that call runs.
///
/// `Routines`' `DECOMPILE_ABILITY_ID` and `Tools`' `STARTER_TOOL_ID` are the
/// two exceptions: both are permanent grants that outlive
/// `apply_character_choice` untouched, so both are written here rather than
/// there — `tuning::STARTER_TOOL_ID`'s own doc comment states the same
/// creation-only rule `DECOMPILE_ABILITY_ID` already follows. This function
/// is the whole of `Game::new`'s side of it: `spawn_player` has exactly one
/// caller, `new_with`, so nothing here runs on a `Game::load`.
fn spawn_player(world: &mut World, start: (i32, i32)) -> Entity {
    world
        .spawn((
            Player,
            Position {
                x: start.0,
                y: start.1,
            },
            Glyph {
                ch: '@',
                color: GlyphColor::Cyan,
            },
            crate::tuning::PLAYER_BASE_STATS,
            PowerReserve::default(),
            Experience::default(),
            Decompiler::default(),
            Equipment::default(),
            Inventory::default(),
            GearCopies::default(),
            StatusEffects::default(),
            CombatBuff::default(),
            FieldBuff::default(),
            Perks::default(),
            // Nested because the bundle tuple above is already at bevy's
            // 15-element ceiling — the query-tuple limit CLAUDE.md already
            // names, reached here from the bundle side instead.
            (
                Routines(vec![abilities::DECOMPILE_ABILITY_ID.to_string()]),
                PlayerIdentity::default(),
                DownedPrograms::default(),
                Tools(vec![ToolId(STARTER_TOOL_ID.to_string())]),
                crate::components::Implants::default(),
                crate::components::PhaseKeys::default(),
            ),
        ))
        .id()
}

impl Game {
    pub fn new(seed: u32, difficulty: DifficultyMode, assets_dir: &Path) -> std::io::Result<Self> {
        Self::new_with(seed, difficulty, assets_dir, &CharacterChoice::default())
    }

    /// `Game::new`'s constructor, taking who the player is as an explicit
    /// argument. There are ~1,600 `Game::new` call sites, almost all tests,
    /// so `new` stays a one-line delegation here rather than every one of
    /// them growing a fourth argument.
    pub fn new_with(
        seed: u32,
        difficulty: DifficultyMode,
        assets_dir: &Path,
        choice: &CharacterChoice,
    ) -> std::io::Result<Self> {
        let AssetDbs {
            abilities: ability_db,
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
            items: item_db,
            perks: perk_db,
            talents: talent_db,
            affixes: affix_db,
            settlements: settlement_db,
            outposts: outpost_db,
            policy: enemy_policy,
            behaviours,
            warnings: load_warnings,
        } = load_asset_dbs(assets_dir)?;

        // Zone 1 is always neutral, but this still routes through the one
        let mut world_map = WorldMap::new(seed);
        let start = find_walkable_start(&mut world_map);

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
        world.insert_resource(crate::alerts::AlertBoard::default());
        world.insert_resource(sortie_db);
        world.insert_resource(caravan_db);
        world.insert_resource(rock_db);
        world.insert_resource(floor_db);
        world.insert_resource(crop_db);
        world.insert_resource(room_db);
        world.insert_resource(prop_db);
        world.insert_resource(nemesis_db);
        world.insert_resource(world_map);
        world.insert_resource(GameClock::default());
        world.insert_resource(crate::resources::NextProgramId::START);
        world.insert_resource(GameRng(StdRng::seed_from_u64(seed as u64)));
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
        world.insert_resource(difficulty);
        world.insert_resource(Party::default());
        world.insert_resource(WieldedProgram::default());
        world.insert_resource(Research::default());
        world.insert_resource(crate::resources::ActiveResearch::default());
        // Empty, not the starter tool: the starter is granted straight into
        // the slot below, and knowledge is not what put it there — see
        // `resources::KnownTools`'s doc.
        world.init_resource::<KnownTools>();
        world.init_resource::<crate::resources::CraftedRoutines>();
        world.init_resource::<crate::resources::GearDesigns>();
        world.init_resource::<crate::resources::KnownAffixes>();
        world.init_resource::<crate::resources::DiscoveredResearch>();
        world.insert_resource(BuybackLedger::default());
        world.insert_resource(crate::resources::CaravanMemory::default());
        world.insert_resource(ZoneLevel::default());
        world.insert_resource(crate::base_grid::BaseGrid::default());
        // Off when a run starts: the player's bump has to be armed before it
        // destroys terrain. See `resources::MiningMode`.
        world.init_resource::<crate::resources::MiningMode>();
        world.init_resource::<crate::resources::EnemyStrength>();
        // Empty at the start of a run: nothing has been built yet, so every
        // `first_free` structure is still owed. See `resources::FreeBuilds`.
        world.init_resource::<crate::resources::FreeBuilds>();
        world.init_resource::<crate::resources::RunTally>();
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
        // Empty on a new game: nothing has been learned yet. `Game::load`
        // refills it from the save instead (todo #100 Task 5).
        world.insert_resource(crate::resources::EmulationImages::default());
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
        world.insert_resource(crate::resources::ActiveContracts::default());
        world.insert_resource(crate::resources::WorkOrders::default());
        // `PowerGrid`'s reason, and the same shape: a per-tick cache with
        // nothing in the save to restore it from, inserted at both doors so a
        // screen opened on the frame a game starts or loads finds an empty
        // demand rather than a missing resource.
        world.insert_resource(crate::resources::LabourDemand::default());
        // Empty on purpose in *both* constructors. What has actually been
        // earned is installed afterwards by `install_profile`, which app-core
        // calls on either path; paying for it is a separate call it makes on
        // only one. See `grant_profile_rewards`.
        world.insert_resource(crate::achievements::Profile::default());
        world.insert_resource(crate::resources::PendingProfileWrites::default());
        world.insert_resource(crate::ftue::FtuePlayed::default());
        world.insert_resource(crate::ftue::PendingCrafted::default());
        // Both doors, deliberately: a loaded game must collect telemetry too,
        // and this is not saved (dev output, not run state) so `load` has
        // nothing to restore it from.
        world.insert_resource(crate::resources::BattleTelemetry::default());
        world.insert_resource(crate::base_ledger::BaseLedger::default());
        world.insert_resource(ZoneSpawnPoint {
            x: start.0,
            y: start.1,
        });

        let player = spawn_player(&mut world, start);
        world.insert_resource(PlayerEntity(player));

        // The anchor's permanent door into the field, standing where the
        // player does. Spawned here rather than through a `Game` method —
        // there is no `Game` yet to call one on, the same reason the player
        // above is a bare `world.spawn`.
        let anchor = world
            .spawn((
                BaseAnchor,
                Position {
                    x: start.0,
                    y: start.1,
                },
                // `#` / Gray: neither arrow character was free for an
                // entrance. `SurfaceLink` already claims `>` for "a way
                // down" on this same surface map, and `<` is reserved to
                // mean "a way up/out" — `stack::FrameMapCell::LinkUp` /
                // `views::StackCellView::LinkUp` render `('<', YELLOW)` — so
                // a door only ever entered from the surface would read
                // backwards under either. Blue and Cyan are reserved too:
                // Blue marks a Nemesis and Cyan is the player's own glyph
                // colour (`game::inspection::difficulty_color`), and a
                // permanent map fixture has no business being mistaken for
                // either.
                Glyph {
                    ch: '#',
                    color: GlyphColor::Gray,
                },
            ))
            .id();
        world.insert_resource(AnchorEntity(anchor));

        let schedule = Self::build_schedule();

        let mut game = Self { world, schedule };
        game.apply_character_choice(choice);
        for warning in load_warnings {
            game.log(warning);
        }
        game.ensure_local_population();
        game.ensure_local_settlements();
        game.spawn_surface_links(STACK_LINKS_PER_ZONE);
        game.log(
            "Connection established. You materialize at the edge of the Phase-Manifold Basin.",
        );
        // Before the first tick, so the very first contracts screen a run
        // opens already has the chain's first mission in hand.
        game.ensure_tutorial_held();
        game.assess_situations();
        game.mark_seen_tiles();
        Ok(game)
    }

    /// The system schedule every tick runs, shared by `new_with` and `load`
    /// so the two can't drift — the chained pair below is exactly the kind of
    /// constraint that gets added to one copy and forgotten in the other.
    pub(crate) fn build_schedule() -> Schedule {
        let mut schedule = Schedule::default();
        schedule.add_systems((
            (systems::power_regen_system, systems::needs_tick_system).chain(),
            // Unchained: the only writer of `Needs` in the schedule, and what
            // it reads — who is staff, and who holds a `Task` — nothing above
            // it has written this tick.
            systems::needs_drain_system,
            systems::wander_ai_system,
            // Chained: `task_progress_system` and `assembler_system` both
            // write `Task::progress` (for different targets, but bevy can
            // only see the conflict, not the disjointness), and an
            // arbitrary-but-fixed order is not the same as a stated one.
            // `haul_step_system` joins the same chain for the same reason,
            // one component along — it writes `Stock` too — and runs last
            // because a load is taken off a machine the tick *after* that
            // machine reports itself clogged.
            (
                // First, and that ordering is the design rather than a
                // convenience: it decides which machines are dark *before*
                // anything reads or writes a `MachineStatus`. A power system
                // running last would overwrite what the producers decided and
                // log a transition twice per tick for as long as the base was
                // short — see `systems::power_grid_system`.
                systems::power_grid_system,
                systems::idle_machine_system,
                // Between the two, and inserted rather than reordered: it
                // needs the grid `power_grid_system` just decided, and
                // `task_progress_system` folds what it writes.
                crate::situations::assess_situation_system,
                systems::task_progress_system,
                systems::player_gather_system,
                systems::assembler_system,
                systems::depot_intake_system,
                crate::game::base::hauling::haul_step_system,
            )
                .chain(),
            difficulty::death_handling_system,
            // Unchained: it shares no mutable state with anything above it,
            // and what it reads are counters every one of those has already
            // finished writing for this tick.
            crate::game::achievements::achievement_system,
            // Unchained for the same reason, and safe beside the system above
            // because the two drain different `RunFeats` fields: each field
            // has exactly one drainer, so neither can eat the other's events.
            crate::game::contracts::contract_system,
        ));
        schedule
    }

    /// Spawns `species_id` at the player's tile, already owned, and returns
    /// what to call it — or `None` if this install has no such species, which
    /// is the cross-db check `AchievementDb::load_dir` deliberately deferred
    /// to here.
    ///
    /// Follows `adopt_orphan`'s sequence, and stops where it does:
    /// deliberately **not** pushed into `Party`. `Party` is the deployed
    /// battle line, capped at `MAX_PARTY_SIZE` and entered through an
    /// explicit `add_to_party` — the program arrives owned and the player
    /// deploys it, like every other acquisition.
    pub(crate) fn grant_starting_program(&mut self, species_id: &str) -> Option<String> {
        let player = self.player_entity();
        let at = *self.world.get::<Position>(player)?;
        let program = self.spawn_wild_creature_scaled(species_id, at.x, at.y, 1.0, false)?;
        self.world
            .entity_mut(program)
            .remove::<(Hostile, WanderAi)>();
        let parts = self.roster_parts();
        self.world.entity_mut(program).insert(parts);
        self.install_innate_routines(program);
        self.seat_derived(program);
        Some(self.creature_label(program))
    }
}
