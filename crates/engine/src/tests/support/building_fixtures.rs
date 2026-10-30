use super::*;
use crate::*;

/// Gives the player `n` Research Data, bypassing the Research Node so
/// the test doesn't depend on tick timing or a tamed worker.
pub(crate) fn grant_research_data(game: &mut Game, n: u32) {
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::RESEARCH_DATA), n);
}

/// Files a build request through `Game::place_structure` and raises it on
/// the spot, charging the bill of materials to the player's pack.
///
/// **What a fixture almost always wants.** A deploy is a *request* now: the
/// structure stands only once a posted program has carried the materials to
/// the cell and worked there for `BuildSite::required_ticks`. A test about
/// what a standing machine *does* should not have to run a crew to get one,
/// and a test about the request itself — the refusals, the site, the crew —
/// calls `place_structure` directly and leaves the site alone.
///
/// The Home passes straight through: `place_structure` still stands one up
/// by hand, so there is no site left behind and `raise_pending_builds` finds
/// nothing to do.
pub(crate) fn place_now(game: &mut Game, kind: &str, dx: i32, dy: i32) -> Result<(), String> {
    file_build(game, kind, dx, dy)?;
    raise_pending_builds(game);
    Ok(())
}

/// Files a build request through the real `Game::place_structure`, taming a
/// program on the spot to pay for it, and leaves the site standing.
///
/// **What a fixture wants now that a deploy costs a program.** A test about
/// the crew, the cell or the bill should not have to stage a roster to get a
/// request filed — and the alternative, spelling the two lines out at every
/// one of the several dozen call sites, is several dozen chances to stage it
/// differently. A test about the *cost* calls `place_structure` directly and
/// says what it is spending.
///
/// The exempt structures — the Home — are passed `None`, which is what
/// `structure_needs_program` answers and what a frontend would do.
pub(crate) fn file_build(game: &mut Game, kind: &str, dx: i32, dy: i32) -> Result<(), String> {
    if kind == "portal" {
        // As `breach_through_a_portal`: a fixture builds the doorway, and
        // whether it may is `tests::phase_keys`' question.
        let zone = game.world.resource::<ZoneLevel>().0;
        game.grant_phase_key(zone);
    }
    // The Home founds where the party stands, so every fixture founds on
    // `TEST_HOME_CELL` and gets the fixture pocket around it — the ground
    // its coordinates were written against.
    if kind == "home" {
        from_inside_the_base(game, |g| g.place_structure(kind, dx, dy, None))?;
        game.lay_test_pocket();
        return Ok(());
    }
    let program = build_program(game, &kind.to_string(), 1);
    game.place_structure(kind, dx, dy, program)
}

/// Files an upgrade through the real `Game::upgrade_structure`, taming a
/// program deep enough to pay for it, and leaves the site standing.
/// `file_build`'s counterpart, for the same reason.
pub(crate) fn file_upgrade(game: &mut Game, structure: Entity) -> Result<(), String> {
    let kind = game
        .world
        .get::<Structure>(structure)
        .map(|s| s.kind.clone())
        .unwrap_or_default();
    let next = game
        .world
        .get::<StructureTier>(structure)
        .map(|t| t.0)
        .unwrap_or(1)
        + 1;
    let program = build_program(game, &kind, next);
    game.upgrade_structure(structure, program)
}

/// A tamed program deep enough to pay for a build of `tier`, or `None` if
/// `kind` is exempt.
///
/// It also parks a **spare** in the battle party when the roster would
/// otherwise be down to the one program being spent, because
/// `place_structure` refuses to take a base to zero programs. In the party
/// rather than on the staff deliberately: `base_staff` skips a party member,
/// so a fixture whose whole point is that there is nobody to build still has
/// nobody. The spare is added only when it is needed, so a fixture that
/// already staffed its base is left exactly as it staged itself.
pub(crate) fn build_program(game: &mut Game, kind: &StructureId, tier: u32) -> Option<Entity> {
    if !game.structure_needs_program(kind) {
        return None;
    }
    let spend = tame_at_zone(game, tier);
    if game.owned_pets().len() < 2 {
        let spare = tame_at_zone(game, tier);
        game.world.resource_mut::<Party>().0.push(spare);
    }
    Some(spend)
}

/// Overwrites just the two build rolls on `program`, leaving the four
/// combat rolls exactly as they were — the axis every build-quality fixture
/// varies, and the one `quality_percent` deliberately cannot see.
pub(crate) fn set_build_rolls(game: &mut Game, program: Entity, assembly: f32, extraction: f32) {
    let mut p = game
        .world
        .get::<crate::components::Potential>(program)
        .copied()
        .unwrap_or(crate::components::Potential::NEUTRAL);
    p.assembly_roll = assembly;
    p.extraction_roll = extraction;
    game.world.entity_mut(program).insert(p);
}

/// `file_build`, paid for with a program of known build rolls, returning the
/// program it spent. The fixture half of everything about build quality: the
/// figure is taken off the program at the moment the request is filed, so
/// the rolls have to be on it before `place_structure` is called.
pub(crate) fn file_build_with_rolls(
    game: &mut Game,
    kind: &str,
    dx: i32,
    dy: i32,
    assembly: f32,
    extraction: f32,
) -> Entity {
    let program =
        build_program(game, &kind.to_string(), 1).expect("this kind is paid for with a program");
    set_build_rolls(game, program, assembly, extraction);
    game.place_structure(kind, dx, dy, Some(program))
        .expect("the fixture files a legal build");
    program
}

/// The first structure of `kind` standing anywhere — for a test that has
/// just reloaded and cannot hold on to an `Entity` across the round trip.
pub(crate) fn first_structure(game: &mut Game, kind: &str) -> Entity {
    let mut q = game.world.query::<(Entity, &Structure)>();
    q.iter(&game.world)
        .find(|(_, s)| s.kind == kind)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("no {kind} stands in the base"))
}

/// A base with one Mining Node raised by a crew, built by a program of the
/// given build rolls — a real machine carrying a real `BuildQuality`, which
/// a hand-spawned one deliberately does not.
pub(crate) fn base_with_a_built_node(game_seed: u32, assembly: f32, extraction: f32) -> Game {
    let mut game = Game::new(game_seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 500);
    stand_in_base(&mut game);
    spawn_tamed(&mut game, 500, 3);
    file_build_with_rolls(&mut game, "extractor", 1, 0, assembly, extraction);
    for _ in 0..400 {
        if first_structure_opt(&mut game, "extractor").is_some() {
            break;
        }
        game.tick();
    }
    let node = first_structure(&mut game, "extractor");
    set_recipe_researched(&mut game, node, ids::CORE_FRAGMENT).unwrap();
    game
}

fn first_structure_opt(game: &mut Game, kind: &str) -> Option<Entity> {
    let mut q = game.world.query::<(Entity, &Structure)>();
    q.iter(&game.world)
        .find(|(_, s)| s.kind == kind)
        .map(|(e, _)| e)
}

/// Files an upgrade through the real `Game::upgrade_structure` and then
/// raises it, for a test that only wants a Mk2 machine rather than a crew.
///
/// `place_now`'s counterpart, and its reason: upgrading is a request now, so
/// a test about what a Mk2 node *does* would otherwise have to run a crew to
/// get one. A test about the request itself — the refusals, the site, the
/// crew — calls `Game::upgrade_structure` directly and leaves the site alone.
pub(crate) fn upgrade_now(game: &mut Game, structure: Entity) -> Result<(), String> {
    file_upgrade(game, structure)?;
    raise_pending_builds(game);
    Ok(())
}

/// Raises every outstanding build request immediately, charging each bill of
/// materials to the player's pack and falling back to the base's own
/// shelves — the two stores a real crew fetches from, in the order it
/// prefers reversed, because a fixture's materials are nearly always in the
/// pack.
///
/// A shortfall is **not** an error here: `place_structure` no longer refuses
/// a request the base cannot afford, so a fixture that never stocked
/// anything still gets its structure. A test that cares what a build costs
/// asserts on the pack afterwards.
pub(crate) fn raise_pending_builds(game: &mut Game) {
    loop {
        let pending: Option<(bevy_ecs::prelude::Entity, Position, BuildSite)> = {
            let mut query = game
                .world
                .query::<(bevy_ecs::prelude::Entity, &BuildSite, &Position)>();
            query
                .iter(&game.world)
                .next()
                .map(|(e, site, pos)| (e, *pos, site.clone()))
        };
        let Some((site, pos, build)) = pending else {
            return;
        };
        let (kind, cost, goal) = (build.structure, build.cost, build.goal);
        for (item, qty) in &cost {
            let player = game.player_entity();
            let paid = game
                .world
                .get_mut::<Inventory>(player)
                .map(|mut inv| inv.take(item.clone(), *qty))
                .unwrap_or(0);
            if paid < *qty {
                crate::game::base::stock::spend_from_base(
                    game,
                    item,
                    qty - paid,
                    crate::base_ledger::ConsumeSource::Base,
                );
            }
        }
        let def = game
            .world
            .resource::<crate::structures::StructureDb>()
            .get(&kind)
            .cloned()
            .expect("a fixture never files a request for a structure with no file");
        game.world.despawn(site);
        // **Branching on the goal is not optional.** Pointed at an upgrade
        // site, a fixture that spawns a structure for every request stands a
        // duplicate machine on an occupied cell, and every test that files an
        // upgrade and then places something goes strange in a way that reads
        // as the feature being broken.
        match goal {
            BuildGoal::New => {
                game.spawn_structure(&def, pos.x, pos.y, None);
            }
            BuildGoal::Upgrade { to_tier } => {
                let machine = {
                    let mut query = game
                        .world
                        .query::<(bevy_ecs::prelude::Entity, &Position, &Structure)>();
                    query
                        .iter(&game.world)
                        .find(|(_, p, _)| p.x == pos.x && p.y == pos.y)
                        .map(|(e, ..)| e)
                        .expect("an upgrade request stands on its own machine")
                };
                game.world
                    .entity_mut(machine)
                    .insert(StructureTier(to_tier));
                if let Some(mut node) = game.world.get_mut::<ResourceNode>(machine)
                    && node.level.is_some()
                {
                    node.level = Some(to_tier);
                }
            }
            BuildGoal::Recharge => {
                let terminal = {
                    let mut query = game
                        .world
                        .query::<(bevy_ecs::prelude::Entity, &Position, &Structure)>();
                    query
                        .iter(&game.world)
                        .find(|(_, p, _)| p.x == pos.x && p.y == pos.y)
                        .map(|(e, ..)| e)
                        .expect("a recharge request stands on its own terminal")
                };
                if let Some(mut pod) = game.world.get_mut::<crate::components::DropPod>(terminal) {
                    pod.charged = true;
                }
            }
        }
        // The base's systems, without advancing the clock — which is the
        // ordering `tick_inner` gives a real crew-raised structure:
        // `run_build_crew` stands it up and `schedule.run` follows in the
        // same tick, so `idle_machine_system` corrects its optimistic
        // default `MachineStatus` before anything can look. Skipped, a
        // fixture's machine reads `Running` with nobody posted to it, which
        // is a fact about the fixture and not about the base.
        game.schedule.run(&mut game.world);
    }
}

/// Deploys a Data Cache next to the player without going through/// Deploys a Data Cache next to the player without going through
/// `place_structure`, sidestepping its Home/cost/radius requirements —
/// those aren't what the capacity tests are about.
pub(crate) fn spawn_data_cache(game: &mut Game, offset: i32) {
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.world.spawn((
        Structure {
            kind: "data_cache".to_string(),
        },
        Position {
            x: pos.x + offset,
            y: pos.y,
        },
    ));
}

/// Deploys a structure of `kind` at an absolute position, bypassing
/// `place_structure`'s Home, cost and distance rules — for tests about what
/// a standing structure *enables*, not about the build rules.
/// Returns the entity it spawned, so a fixture that stands a structure can
/// then address it — `build_program_bench` inserting a `StructureTier` is
/// the case that needed it, and a helper that could not be addressed is why
/// `run_one_full_gather_cycle_at_tier` below spawns its own by hand.
pub(crate) fn spawn_structure_at(game: &mut Game, kind: &str, x: i32, y: i32) -> Entity {
    game.world
        .spawn((
            Structure {
                kind: kind.to_string(),
            },
            Position { x, y },
        ))
        .id()
}

/// A Workbench standing at `(x, y)` with Automation done: the pair that makes
/// `Game::can_extract_routines` true, by ownership rather than touch.
pub(crate) fn stand_an_extraction_bench(game: &mut Game, x: i32, y: i32) -> Entity {
    unlock_research_chain(game, "automation");
    spawn_structure_at(game, "workbench", x, y)
}

/// Stands the party in base space with a Workbench orthogonally beside it:
/// the touch a bench recipe is offered under. Returns the bench.
pub(crate) fn touch_a_workbench(game: &mut Game) -> Entity {
    stand_in_base(game);
    spawn_structure_at(game, "workbench", 1, 0)
}

/// Stands a machine of `kind` beside the party through the real build path,
/// then sets it to `recipe` the way the player does — researching the
/// recipe's gate first so the refusal is not the thing under test.
pub(crate) fn place_machine(
    game: &mut Game,
    kind: &str,
    recipe: &str,
    dx: i32,
    dy: i32,
) -> Result<Entity, String> {
    place_now(game, kind, dx, dy)?;
    let (bx, by) = game
        .field_pos()
        .expect("a test places machines from the field");
    let machine = game
        .world
        .query::<(Entity, &Structure, &Position)>()
        .iter(&game.world)
        .find(|(_, s, p)| s.kind == kind && (p.x, p.y) == (bx + dx, by + dy))
        .map(|(e, _, _)| e)
        .unwrap_or_else(|| panic!("the {kind} should stand at the tile"));
    set_recipe_researched(game, machine, recipe)?;
    Ok(machine)
}

/// `place_machine` without the build: a machine of `kind` at an absolute
/// tile, set to `recipe`. Named apart from `spawn_machine_at`, which stands a
/// machine with nothing set for the older single-recipe fixtures.
pub(crate) fn spawn_machine_with_recipe_at(
    game: &mut Game,
    kind: &str,
    recipe: &str,
    x: i32,
    y: i32,
) -> Entity {
    game.lay_test_pocket();
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == kind)
        .unwrap_or_else(|| panic!("{kind} should be a shipped structure"));
    let machine = game.spawn_structure(&def, x, y, None);
    set_recipe_researched(game, machine, recipe).unwrap();
    machine
}

pub(crate) fn set_recipe_researched(
    game: &mut Game,
    machine: Entity,
    recipe: &str,
) -> Result<(), String> {
    let kind = game.world.get::<Structure>(machine).unwrap().kind.clone();
    let gate = game
        .world
        .resource::<StructureDb>()
        .get(&kind)
        .and_then(|d| d.machine.as_ref())
        .and_then(|m| m.recipes.iter().find(|r| r.produces.as_str() == recipe))
        .and_then(|r| r.research.clone());
    if let Some(gate) = gate {
        unlock_research_chain(game, &gate);
    }
    let item = ItemId::from(recipe);
    // Setting is a base action; a fixture out on the surface crosses in and
    // back so the call is not what the test is about.
    if game.field_pos().is_some() {
        game.set_machine_recipe(machine, &item)
    } else {
        from_inside_the_base(game, |g| g.set_machine_recipe(machine, &item))
    }
}

/// A walled 4x4 room (interior x and y in 2..=5) with a door, `finished`
/// of its 16 cells in cobalt carpet. Scored by area and finish alone: 16
/// finished cells is Superb, 8 is Fine, none is Cramped-to-Plain. Anything
/// the caller adds to it is on top.
pub(crate) fn walled_room_with_finish(game: &mut Game, finished: usize) {
    for i in 1..=6 {
        for (x, y) in [(i, 1), (i, 6), (1, i), (6, i)] {
            let kind = if (x, y) == (3, 1) { "door" } else { "wall" };
            spawn_structure_at(game, kind, x, y);
        }
    }
    let mut grid = game.world.resource_mut::<crate::base_grid::BaseGrid>();
    for (n, (x, y)) in (2..=5)
        .flat_map(|x| (2..=5).map(move |y| (x, y)))
        .enumerate()
    {
        grid.lay_floor(x, y);
        if n < finished {
            assert!(grid.set_finish(x, y, crate::floors::FloorId::from("cobalt_carpet")));
        }
    }
}

/// Marks a `Game` that has already had `stand_ample_grid_supply` run against
/// it, so a helper a test calls more than once (`deployed`, `worked_node_at`,
/// ...) doesn't pile up a fresh batch of Rechargers on every call.
#[derive(bevy_ecs::prelude::Resource)]
struct GridSupplyStood;

/// Stands enough Recharger Nodes on `game`'s base that its grid supply is
/// never the reason a fixture machine goes dark.
///
/// A slew of fixtures across `tests::chains` and `tests::building` spawn a
/// machine directly via `world.spawn` (`spawn_structure_at` above included),
/// bypassing `Game::build_structure` and the Home a real base always has
/// standing — so once Task 4 authored a real `power_draw` onto a shipped
/// machine, every one of them went dark from the moment it existed. See
/// `docs/superpowers/archive/specs/2026-08-17-base-power-grid-design.md`.
///
/// Deliberately absurd — comfortably past 1,000 supply against draws in the
/// single digits — for the same reason `tests::power::GRID_SOURCE` picks an
/// absurd draw for the systems tests: so this stays true no matter how the
/// Recharger's own `power_supply` is retuned later, rather than being a
/// number some future change quietly falls under again.
///
/// Built out of `spawn_structure_at`, so — like the rest of this file's
/// fixtures — it carries no `Durability`, which keeps it out of the GC
/// Entropy Sweep's target pool (`Game::run_raid` filters on
/// `(With<Durability>, With<Structure>)`). A raidable fixture here would change how many
/// `GameRng` draws a multi-tick test consumes and shift every roll after it;
/// see the RNG-stream hazard this file's fixtures already avoid.
pub(crate) fn stand_ample_grid_supply(game: &mut Game) {
    if game.world.get_resource::<GridSupplyStood>().is_some() {
        return;
    }
    let per = game
        .world
        .resource::<crate::structures::StructureDb>()
        .get("recharger_node")
        .map(|d| d.power_supply.max(1))
        .unwrap_or(1);
    let count = 1000u32.div_ceil(per);
    for i in 0..count {
        // `spawn_structure_at`'s shape — a bare `Structure` and `Position`,
        // no `Durability` — plus an inexhaustible charge. A Recharger burns a
        // Power Cell every `POWER_UPKEEP_TICKS` and supplies nothing while it
        // cannot pay (`StructureDef::power_upkeep`), and these stand a million
        // tiles from anything that could feed them. Absurd for the same reason
        // the count is: the grid must never be the reason a fixture machine
        // goes dark, and a bank that quietly starved on tick 20 would read as
        // the multi-tick tests it props up having broken.
        game.world.spawn((
            Structure {
                kind: "recharger_node".to_string(),
            },
            Position {
                x: -1_000_000,
                y: -1_000_000 - i as i32,
            },
            crate::components::PowerFuel {
                ticks_left: u32::MAX,
            },
        ));
    }
    game.world.insert_resource(GridSupplyStood);
}
