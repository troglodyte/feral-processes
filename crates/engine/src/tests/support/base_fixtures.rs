use super::*;
use crate::*;

/// Tames a program and puts it to work on a node producing `resource`,
/// so a cronjob is guaranteed to be running — the assertions below are
/// vacuous if nothing is assigned. Returns the node, since a cronjob's
/// output lands in its own buffer rather than the player's cargo.
///
/// The buffer is deliberately roomy: a test about whether a cronjob runs at
/// all should not be measuring how fast it clogs.
pub(crate) fn assign_worker_producing(game: &mut Game, resource: ItemId) -> Entity {
    let worker = spawn_tamed(game, 10, 3);
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "test_node".to_string(),
            },
            Position { x: 3, y: 4 },
            ResourceNode {
                resource,
                level: None,
            },
            Stock::new(10_000),
            MachineStatus::default(),
        ))
        .id();
    stand_player_at_post(game, structure);
    game.assign_cronjob(worker, structure).unwrap();
    structure
}

/// The two components `task_progress_system` needs on a node beyond
/// `ResourceNode` itself. A node missing either is skipped by that system's
/// query and silently produces nothing — which reads as a payout curve that
/// moved rather than as a fixture that is short a component, so bundle them
/// rather than leaving each fixture to remember.
///
/// The buffer is deliberately far past any one cycle's payout: a test about
/// what a cycle is worth should not be measuring how fast the node clogs.
pub(crate) fn work_node_parts() -> (Stock, MachineStatus) {
    (Stock::new(10_000), MachineStatus::default())
}

/// Deploys a working machine of `kind` at an absolute tile, with every
/// component the def implies — `Stock` sized from the def, a
/// `MachineStatus`, and a `ResourceNode` if it extracts.
///
/// The difference from `spawn_structure_at`, which spawns a bare
/// `Structure` and `Position`: a chain test needs machines that can
/// actually hold and pull stock, and a node short of `Stock` or
/// `MachineStatus` is skipped by `task_progress_system`'s query and
/// silently produces nothing.
pub(crate) fn spawn_machine_at(game: &mut Game, kind: &str, x: i32, y: i32) -> Entity {
    game.lay_test_pocket();
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == kind)
        .unwrap_or_else(|| panic!("{kind} should be a shipped structure"));
    let mut entity = game.world.spawn((
        Structure {
            kind: def.id.clone(),
        },
        Position { x, y },
        Stock::new(def.capacity),
        MachineStatus::default(),
    ));
    if let Some(work) = &def.work {
        entity.insert(ResourceNode {
            resource: work.produces.clone(),
            level: work.level,
        });
    }
    // A configurable machine stands with nothing set, as `spawn_structure`
    // leaves it; short of the component `idle_machine_system` reads it as an
    // ordinary idle machine.
    if def.machine.is_some() {
        entity.insert(crate::components::MachineRecipe::default());
    }
    // `Game::spawn_structure`'s own list, and short of it a rig is a machine
    // that refuses every deposit and strips nothing — silently, since
    // `run_teardown_rigs` filters on the component's presence.
    if def.strips.is_some() {
        entity.insert(crate::components::Hopper::default());
    }
    if def.racks.is_some() {
        entity.insert(crate::components::Racked::default());
    }
    if def.incubation_slots > 0 {
        entity.insert(crate::components::Incubator::with_slots(
            def.incubation_slots,
        ));
    }
    entity.id()
}

/// Stands the player on `(x, y)`.
///
/// `assign_cronjob` starts a posted program from the player's tile, so this
/// is how a fixture decides whether a new cronjob has a walk to make: stand
/// away from the machine and the worker walks in, stand at it and the worker
/// is already at its post. It is the player's position that carries that
/// distance now, not the worker's.
pub(crate) fn stand_player_at(game: &mut Game, x: i32, y: i32) {
    let player = game.player_entity();
    let mut pos = game.world.get_mut::<Position>(player).unwrap();
    pos.x = x;
    pos.y = y;
}

/// Stands the party in base space at the post east of `structure`, so a
/// cronjob assigned next starts its program already at the machine.
///
/// The **base** cell and not the player's `Position`: every `Structure`
/// stands in base space, and both of the actions this sets up for —
/// `assign_cronjob` and `Game::work_structure` — measure from the cell the
/// party is standing in there. `Position` is pinned to the anchor tile on
/// the zone surface and has no say.
pub(crate) fn stand_player_at_post(game: &mut Game, structure: Entity) {
    let target = *game.world.get::<Position>(structure).unwrap();
    stand_in_base_at(game, target.x + 1, target.y);
}

/// Stands a Portal one cell east of base space's exit cell and walks the
/// party onto it, which is the whole of how a breach happens now.
///
/// A Portal is a `Structure`, so it stands in base space with the rest of
/// the base, and `Game::move_in_base` is what consumes it — the surface
/// branch of `move_player` reads no structure at all. A fixture that spawned
/// one at the player's surface tile and stepped onto it used to work only by
/// the coordinate collision at `(0, 0)`.
///
/// The pocket is laid because a step in base space is refused by solid rock,
/// and spawned rather than deployed because a Portal costs Portal Fragments
/// this is never trying to measure.
pub(crate) fn breach_through_a_portal(game: &mut Game) {
    // The gate is `tests::phase_keys`' to measure; a breach test is about
    // what the breach does.
    let zone = game.world.resource::<ZoneLevel>().0;
    game.grant_phase_key(zone);
    game.lay_test_pocket();
    // The first pocket cell east of a standing spot whose step nothing on
    // the field would answer first — the base shares the field with links,
    // programs and towns, and a fixture's portal must not land on one.
    let clear = |game: &mut Game, x: i32, y: i32| {
        game.find_wild_creature_at(x, y).is_none()
            && game.find_nest_at(x, y).is_none()
            && game.find_surface_link_at(x, y).is_none()
            && game.find_settlement_at(x, y).is_none()
            && game.find_trap_at(x, y).is_none()
    };
    let x = (0..TEST_POCKET_HALF_WIDTH)
        .chain(-TEST_POCKET_HALF_WIDTH..0)
        .find(|&x| clear(game, x, 0) && clear(game, x + 1, 0))
        .expect("a pocket row with two clear cells");
    game.world.spawn((
        Structure {
            kind: "portal".to_string(),
        },
        Position { x: x + 1, y: 0 },
    ));
    stand_in_base_at(game, x, 0);
    game.move_player(1, 0);
}

/// Stands the player on field `(x, y)` — `stand_in_base`, at a chosen cell
/// rather than at the Home — out of the Stack if they were in it.
pub(crate) fn stand_in_base_at(game: &mut Game, x: i32, y: i32) {
    game.world.insert_resource(Locale::Surface);
    stand_player_at(game, x, y);
}

/// Stands `worker` on the tile east of `structure` — a post it can work
/// from.
///
/// Only meaningful *after* `assign_cronjob`, which starts a program from the
/// player's tile and would overwrite this — use `stand_player_at_post`
/// before an assignment and this one after.
///
/// A posted program produces nothing until it is orthogonally adjacent to
/// its machine (`task_progress_system`'s `Unstaffed` gate), and it gets
/// there by walking, which takes ticks a fixture usually doesn't want to
/// spend. Any test measuring what a cronjob *produces* rather than how a
/// program gets to work should start it here.
pub(crate) fn park_at_post(game: &mut Game, worker: Entity, structure: Entity) {
    let target = *game.world.get::<Position>(structure).unwrap();
    let mut pos = game.world.get_mut::<Position>(worker).unwrap();
    pos.x = target.x + 1;
    pos.y = target.y;
}

/// Pins `program` in `station`'s pen through the real `Game::pin_subject`
/// door and then walks it there by hand, `pin_subject_is_refused_when_the
/// _pen_already_holds_a_body`'s pattern: pinning writes no `Position`
/// (`components::UnderStudy`'s own doc), so `Game::research_block`'s "a body
/// is standing in the pen" gate needs a body actually there, and a test
/// about the gate should not have to tick `drift_idle_staff` to get one.
pub(crate) fn pin_subject_at_pen(game: &mut Game, program: Entity, station: Entity) {
    let pen = game
        .study_pen(station)
        .expect("a station under test must declare studies");
    game.pin_subject(program, station)
        .expect("pin_subject should succeed for a staff program with a route");
    let mut pos = game.world.get_mut::<Position>(program).unwrap();
    pos.x = pen.0;
    pos.y = pen.1;
}

/// How many of `item` are sitting in `structure`'s output buffer.
pub(crate) fn node_output(game: &Game, structure: Entity, item: &str) -> u32 {
    game.world
        .get::<Stock>(structure)
        .and_then(|s| s.output.get(&ItemId::from(item)).copied())
        .unwrap_or(0)
}

/// How many times a stall into `status` reached the alert board, summed
/// across machines. The board is where a stall is news; the log never
/// carries one.
pub(crate) fn stall_alerts(game: &Game, status: MachineStatus) -> u32 {
    game.alerts()
        .iter()
        .filter(|a| a.kind == crate::alerts::AlertKind::MachineStalled(status))
        .map(|a| a.count)
        .sum()
}

/// Deploys the run's first Home on `TEST_HOME_CELL` and floors the fixture
/// pocket around it — `place_structure` refuses anything else until a Home
/// exists, so most structure-placement tests need this first. The player is
/// stood on the Home's tile to found it and put back where they were.
///
/// The founding fragments are given first and are exactly what founding
/// takes, so every downstream inventory count is as the test left it.
pub(crate) fn place_home(game: &mut Game) {
    give(
        game,
        &ItemId::from(ids::CORE_FRAGMENT),
        crate::tuning::HOME_FOUNDING_COST,
    );
    place_now(game, "home", 0, 0).unwrap();
}

/// A 2x2 (or `footprint`-wide) fixture registered under `id`, cloned off
/// `shell_assembler` so it needs a program and a real bill exactly like a shipped
/// structure does — `spawn_structure_at` bare-spawns a `Structure` with none
/// of that and is for what a standing structure *enables*, not for the
/// placement or reach machinery a footprint test is about.
///
/// Shared by `tests::building`, `tests::hauling` and `tests::work_orders` —
/// the squad seam's failure was that every fixture was hand-built at
/// footprint 1, so every anchor-measuring reader stayed green across 5,964
/// passing tests.
pub(crate) fn footprint_fixture(game: &mut Game, id: &str, footprint: u8) {
    let mut def = game
        .world
        .resource::<StructureDb>()
        .get("assembler")
        .cloned()
        .expect("assembler ships");
    def.id = id.to_string();
    def.name = id.to_string();
    def.footprint = footprint;
    game.world.resource_mut::<StructureDb>().insert(def);
}

/// A base with a Home standing, a footprint-2 fixture registered under
/// `id`, and enough Core Fragments to raise it — the shared setup every
/// footprint test builds on.
pub(crate) fn base_with_footprint_fixture(seed: u32, id: &str) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    footprint_fixture(&mut game, id, 2);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 200);
    game
}

/// How many of `id` the player is holding.
pub(crate) fn count_item(game: &Game, id: &str) -> u32 {
    let player = game.player_entity();
    game.world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(id))
}

pub(crate) fn run_one_full_gather_cycle(game: &mut Game, resource: &str) -> u32 {
    run_one_full_gather_cycle_at_tier(game, "extractor", resource, None)
}

/// Runs exactly one completed gather cycle against a hand-built node of
/// `kind` producing `resource` at `tier`, and returns how many units landed
/// in the player's inventory.
///
/// `kind` is a real structure id rather than a placeholder because the
/// payout consults that structure's `RecipeDef` for `flat_payout` — a node
/// whose kind isn't in the `StructureDb` would silently take the scaling
/// branch.
///
/// `level: None` on the node means it always yields (see
/// `systems::mining_success_chance`), which is what keeps the payout
/// assertions off the RNG entirely.
///
/// Measured across **all three** places a cycle can pay into — the node's own
/// buffer for ordinary salvage, the player's bank for a banked resource, and
/// the active research project's progress for the research currency (see
/// `systems::deliver_payout`) — so this helper keeps answering the one
/// question it is for, "how much did a cycle pay", whichever kind of resource
/// the caller asked about. The buffer is sized far past any one cycle's payout
/// so a clog can never be mistaken for a payout curve that moved.
///
/// For the research currency it also **arms a project**, on the priciest node
/// in the tree: with none selected a cycle lands nowhere at all by design, and
/// the priciest node is what keeps `ActiveResearch::credit`'s saturation from
/// clipping the figure under test. Set directly rather than through
/// `Game::select_research`, which would want a base, a Research Node and a
/// whole material chain standing for a question about one cycle's payout.
pub(crate) fn run_one_full_gather_cycle_at_tier(
    game: &mut Game,
    kind: &str,
    resource: &str,
    tier: Option<u32>,
) -> u32 {
    stand_ample_grid_supply(game);
    let worker = spawn_tamed(game, 10, 3);
    let mut structure = game.world.spawn((
        Structure {
            kind: kind.to_string(),
        },
        crate::components::MachineRecipe(Some(ItemId::from(resource))),
        Position { x: 3, y: 4 },
        ResourceNode {
            resource: ItemId::from(resource),
            level: None,
        },
        Stock::new(10_000),
        MachineStatus::default(),
    ));
    if let Some(t) = tier {
        structure.insert(StructureTier(t));
    }
    let structure = structure.id();
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 0,
        required: 1,
    });

    let item = ItemId::from(resource);
    let project = (game.world.resource::<ItemDb>().research_currency() == Some(&item)).then(|| {
        let id = game
            .world
            .resource::<ResearchDb>()
            .all()
            .max_by_key(|def| def.cost)
            .map(|def| def.id.clone())
            .expect("the shipped tree has nodes");
        game.world
            .resource_mut::<crate::resources::ActiveResearch>()
            .id = Some(id.clone());
        id
    });
    let paid = |game: &Game| {
        node_output(game, structure, resource)
            + held(game, &item)
            + project.as_ref().map_or(0, |id| {
                game.world
                    .resource::<crate::resources::ActiveResearch>()
                    .progress
                    .get(id)
                    .copied()
                    .unwrap_or(0)
            })
    };
    let before = paid(game);
    game.tick();
    paid(game) - before
}

pub(crate) fn find_structure_by_kind(game: &mut Game, kind: &str) -> Option<Entity> {
    let mut query = game.world.query::<(Entity, &Structure)>();
    query
        .iter(&game.world)
        .find(|(_, s)| s.kind == kind)
        .map(|(e, _)| e)
}

/// The initial world spawns wild creatures scattered around the player, so
/// directional-inspect tests clear whatever landed along their search ray
/// first — otherwise they'd be at the mercy of the seed's RNG instead of
/// testing the method itself.
pub(crate) fn clear_creatures_east_of_player(game: &mut Game, start: Position, range: i32) {
    clear_creatures_along_ray(game, start, 1, 0, range);
}

/// The same cleanup along an arbitrary ray, for a test that scans in more
/// than one direction. Clearing only eastward used to be enough by luck: the
/// ground west of the player happened to be empty on the seeds those tests
/// use. It stopped being once wild programs became a property of place
/// rather than of where the player had walked, since every chunk is now
/// stocked whether or not anyone has been there.
pub(crate) fn clear_creatures_along_ray(
    game: &mut Game,
    start: Position,
    dx: i32,
    dy: i32,
    range: i32,
) {
    // Exactly the row `Game::find_target_in_direction` reads, and no wider.
    // It used to clear a 90° cone, matching the scan when the scan was one;
    // a cone-shaped cleanup for a ray-shaped read despawns creatures no test
    // could have seen and would quietly hide a widening of the ray.
    let stale: Vec<Entity> = {
        let mut query = game.world.query::<(Entity, &Position, &Creature)>();
        query
            .iter(&game.world)
            .filter(|(_, pos, _)| {
                (1..=range).any(|step| pos.x - start.x == dx * step && pos.y - start.y == dy * step)
            })
            .map(|(e, ..)| e)
            .collect()
    };
    for e in stale {
        game.world.despawn(e);
    }
}

/// Posting a program to a machine by hand, **as a test fixture only**.
///
/// This was `Game::assign_cronjob`, a player action, until work orders
/// landed on 2026-08-14: the scheduler decides who stands where now, and
/// the menu row and the engine method both went. It survives here because
/// roughly fifty tests about hauling, upkeep, inspection and the manifest
/// need a program *on* a machine and have nothing to say about how it got
/// there — standing up a work order and a staff pool in each of them would
/// be fixture noise that hides what they are actually asserting.
///
/// **Composed from the shipping primitives, never a copy of the removed
/// body.** `require_field`, `accepts_a_program`, `post_reach` and
/// `post_worker` are the same four the scheduler goes through, so a test
/// that passes here is a test about a posting the live game could make.
/// A hand-copied body would be the second copy `CLAUDE.md` records drifting
/// four times.
///
/// `require_field`: `Game::queue_work_order`, the live queue this stands in
/// for, carries the same guard.
impl Game {
    pub(crate) fn assign_cronjob(
        &mut self,
        worker: Entity,
        structure: Entity,
    ) -> Result<(), String> {
        self.require_field()?;
        if !self.accepts_a_program(structure) {
            return Err("That structure can't be worked.".into());
        }
        let (fx, fy) = self
            .field_pos()
            .ok_or_else(|| "You aren't anywhere you can post a program from.".to_string())?;
        let from = Position { x: fx, y: fy };
        let target = *self
            .world
            .get::<Position>(structure)
            .ok_or_else(|| "That structure isn't anywhere you can post to.".to_string())?;
        let blocked = self.blocked_tiles();
        let pocket_radius = self.world.resource::<crate::base_grid::BaseGrid>().radius();
        let side = self.structure_footprint_of(structure);
        {
            let grid = self.world.resource::<crate::base_grid::BaseGrid>();
            // The two errands stay distinct, as they were: a machine the
            // base has been built around needs digging out, one with no
            // route may just need you to walk over to it.
            crate::game::base::hauling::post_reach(
                grid,
                from,
                target,
                side,
                &blocked,
                pocket_radius,
            )
            .map_err(|reason| match reason {
                crate::game::base::hauling::NoPost::BoxedIn => {
                    "That structure is walled in — nothing can stand next to it.".to_string()
                }
                crate::game::base::hauling::NoPost::NoRoute => {
                    "No route to that structure from here.".to_string()
                }
            })?;
        }
        // The removed player action started the program from the player's
        // tile; `post_worker` no longer writes a `Position` at all, so the
        // fixture does it here rather than changing what fifty tests
        // measure. `stand_player_at_post` before this call is still what
        // puts a program straight onto its machine.
        if let Some(mut pos) = self.world.get_mut::<Position>(worker) {
            *pos = from;
        }
        self.post_worker(worker, structure);
        // The live scheduler runs inside `tick_inner` and cannot tick again;
        // the removed player action did, and the tests written against it
        // count ticks from that point. Keeping it is what makes this a
        // faithful stand-in rather than a subtly faster one.
        self.tick();
        Ok(())
    }

    pub(crate) fn assign_guard(&mut self, worker: Entity, structure: Entity) -> Result<(), String> {
        // A guard post is about a structure, and every structure stands in
        // base space — asked here so the fixture cannot create a post from
        // a locale the base's own actions refuse. `Game::set_standing_job`,
        // the live route, carries no locale guard of its own; this fixture
        // is deliberately the stricter of the two.
        self.require_field()?;
        let kind = self
            .world
            .get::<Structure>(structure)
            .ok_or_else(|| "That's not a structure.".to_string())?
            .kind
            .clone();
        if let Some(name) = self
            .world
            .resource::<StructureDb>()
            .get(&kind)
            .filter(|def| !def.raidable)
            .map(|def| def.name.clone())
        {
            return Err(format!("{name} can't be raided — it doesn't need a guard."));
        }
        self.post_guard(worker, structure);
        self.tick();
        Ok(())
    }
}

// ------------------------------------------------- passives in battle
