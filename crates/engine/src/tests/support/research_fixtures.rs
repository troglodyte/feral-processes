use super::*;
use crate::*;

/// Unlocks `id` and every prerequisite it needs, funding the whole
/// chain — so a test that just needs a research-gated structure on the
/// map doesn't have to model the tree itself.
///
/// Since the tree gained zone bands (`ResearchDef::min_zone`), funding is no
/// longer enough on its own, so this also stands the party in the deepest
/// zone the chain asks for. That is a direct `ZoneLevel` write rather than a
/// real breach deliberately: `enter_next_zone` regenerates the map and
/// respawns the wild population, which is a great deal of world change to
/// impose on a fixture that only wanted a bench researched. Nothing is lost
/// by the shortcut, because the tests that are *about* the gate
/// (`breaching_makes_a_zone_gated_node_available`) breach for real, so a
/// gate regression cannot hide behind this.
///
/// **Now writes to the player's own stores, not only the scratch shelf.**
/// Since `ResearchDef::requires_subject` landed, every subject-gated node
/// in the chain spawns a fresh tamed program, pins it, and lets
/// `settle_research` spend it — the real completion door, this function's
/// own reason above — so completing one leaves a `DownedProgram` row in
/// `DownedPrograms` and bumps `resources::NextProgramId`, exactly as a real
/// study would. A test asserting an exact `DownedPrograms` count, or a
/// specific `ProgramId`, after calling this on a subject-gated chain
/// should account for that row rather than be surprised by it.
/// Discovers every shipped research node — `research::ResearchDef::
/// discoverable` hides twenty of them from `Game::research_nodes` and
/// `Game::research_graph` until a study finds them, and a census of the
/// tree's *shape*, or of what a named node reports, is about the catalogue
/// rather than about what this run happens to have uncovered.
///
/// Goes through `Game::discover_research`, the one door, so a fixture can
/// never write a state the game itself could not reach.
pub(crate) fn discover_all_research(game: &mut Game) {
    for def in game.research_defs() {
        game.discover_research(&def.id);
    }
}

pub(crate) fn unlock_research_chain(game: &mut Game, id: &str) {
    fn order(game: &Game, id: &str, out: &mut Vec<String>) {
        let Some(def) = game.world.resource::<ResearchDb>().get(id).cloned() else {
            return;
        };
        for req in &def.requires {
            order(game, req, out);
        }
        if !out.contains(&def.id) {
            out.push(def.id);
        }
    }
    let mut chain = Vec::new();
    order(game, id, &mut chain);
    let needed = chain
        .iter()
        .filter_map(|node| game.world.resource::<ResearchDb>().get(node))
        .map(|def| def.min_zone)
        .max()
        .unwrap_or(0);
    let zone = &mut game.world.resource_mut::<ZoneLevel>().0;
    *zone = (*zone).max(needed);
    // Driven through the real completion door, `Game::settle_research`, rather
    // than a direct write into `resources::Research`: the routines and tools a
    // node hands over are granted there, and a fixture that inserted the id by
    // hand would silently stop unlocking them.
    //
    // What is shortcut is the *earning*, both halves of it — the progress a
    // Research Node would have fed in, and the bill the base would have made.
    // A fixture that only wanted a bench researched should no more have to
    // mine for it than it has to post a program on a Research Node. The
    // scratch shelf is despawned afterwards so it cannot be counted by a
    // `base_holding` assertion in the test that called this.
    let shelf = game
        .world
        .spawn((
            Structure {
                kind: "test_research_shelf".to_string(),
            },
            Position { x: -64, y: -64 },
            Stock::new(1_000_000),
        ))
        .id();
    // `Game::settle_research` refuses a `requires_subject` node without a
    // body standing in a `studies` structure's pen (Task 8), so a chain
    // touching one of the 19 subject-gated nodes needs the same shortcut
    // this fixture already gives materials: a scratch station, far enough
    // into negative coordinates that `Game::pinned_subject`'s `(x, y)` sort
    // picks it over anything a calling test may have placed for real, and a
    // fresh subject per gated node — `settle_research` despawns it on
    // completion, so there is nothing left to clean up afterward.
    let station = game
        .world
        .spawn((
            Structure {
                kind: "research_node".to_string(),
            },
            Position {
                x: -1_000_000,
                y: -1_000_000,
            },
        ))
        .id();
    let pen = game
        .study_pen(station)
        .expect("research_node.ron declares studies");
    // And a scratch rack for the gate's room check; the station's slot is
    // emptied after each completion, standing in for the crew that would
    // carry the program there.
    let rack = game
        .world
        .spawn((
            Structure {
                kind: "quarantine_rack".to_string(),
            },
            Position {
                x: -1_000_000,
                y: -1_000_002,
            },
            crate::components::Racked::default(),
        ))
        .id();
    for node in chain {
        if game.is_researched(&node) {
            continue;
        }
        let def = game
            .world
            .resource::<ResearchDb>()
            .get(&node)
            .cloned()
            .expect("the chain was walked out of the db");
        {
            let mut stock = game.world.get_mut::<Stock>(shelf).unwrap();
            for (item, need) in &def.materials {
                *stock.output.entry(item.clone()).or_default() += need;
            }
        }
        if def.requires_subject {
            let subject = spawn_tamed(game, 10, 3);
            game.world.entity_mut(subject).insert((
                Position { x: pen.0, y: pen.1 },
                crate::components::UnderStudy { station },
            ));
        }
        {
            let mut research = game
                .world
                .resource_mut::<crate::resources::ActiveResearch>();
            research.id = Some(def.id.clone());
            research.progress.insert(def.id.clone(), def.cost);
        }
        game.settle_research();
        assert!(
            game.is_researched(&node),
            "the fixture must actually land {node}"
        );
        game.world
            .entity_mut(station)
            .remove::<crate::components::StudyYield>();
    }
    game.world.despawn(shelf);
    game.world.despawn(station);
    game.world.despawn(rack);
}

/// Stands the party in base space with a Research Node deployed, which is the
/// state a project reads `Blocked` without — a project needs somewhere to
/// come from, and the node is what makes the research currency.
///
/// Returns the node, since a test about progress has to be able to post a body
/// on it.
pub(crate) fn base_with_a_research_node(game: &mut Game) -> Entity {
    stand_in_base(game);
    // A Research Node draws power, and a dark machine makes no progress — the
    // grid must never be the reason a project sits still.
    stand_ample_grid_supply(game);
    let node = spawn_machine_at(game, "research_node", 2, 2);
    // A project is refused unless every line of its bill is a chain the base
    // could actually work — `Game::research_block` through
    // `work_orders::chain_break`. So this stands a producer for every item any
    // shipped bill names *and every ingredient behind it*, plus the Depot that
    // lets a bench be fed by a walk rather than only by a neighbour.
    //
    // Derived from the shipped assets rather than named here, so a retuned bill
    // or a new intermediate joins this the moment its `.ron` file does.
    let bills: Vec<WorkOrder> = game
        .world
        .resource::<ResearchDb>()
        .all()
        .flat_map(|def| def.materials.clone())
        .map(|(item, qty)| WorkOrder::batch(item, qty.max(1)))
        .collect();
    let mut needed: Vec<ItemId> =
        crate::game::base::work_orders::queue_needs(&bills, game.world.resource::<ItemDb>())
            .into_iter()
            .collect();
    // Sorted so the tiles these land on do not depend on set iteration order.
    needed.sort();
    // One machine per (kind, product): a configurable machine makes one thing,
    // so a bill of three items on the Extractor needs three Extractors.
    let mut machines: Vec<(String, Option<ItemId>)> = Vec::new();
    for item in needed {
        let Some(def) = crate::game::base::work_orders::makeable_by(game, &item) else {
            continue;
        };
        let recipe = def.machine.is_some().then(|| item.clone());
        let stand = (def.id, recipe);
        if !machines.contains(&stand) {
            machines.push(stand);
        }
    }
    // The shelf `feeders_for` falls back to when no producer is a neighbour.
    machines.push(("depot".to_string(), None));
    // **Laid floor, or none of this is reachable.** The starting pocket is four
    // cells across, which does not hold a Research Node plus a producer per
    // shipped bill; a machine off the floor is one `can_walk_to_post` refuses,
    // so the scheduler posts nobody and the base reads as having no work — the
    // failure looks like a scheduler bug and is a fixture that stood its
    // machines in the rock.
    for (lane, (kind, recipe)) in machines.into_iter().enumerate() {
        // Four to a row so the farthest machine stays within a walk of the
        // pocket; two tiles apart so none is another's neighbour.
        let (x, y) = (2 + (lane as i32 % 4) * 2, 4 + (lane as i32 / 4) * 3);
        for step in 0..=y {
            game.floor_cell(x, step);
            game.floor_cell(x - 1, step);
        }
        if let Some(item) = recipe {
            // `spawn_structure` is the real component list; the recipe is set
            // directly because a fixture must not research its way to one.
            let def = game
                .structure_defs()
                .into_iter()
                .find(|d| d.id == kind)
                .expect("a shipped machine");
            let machine = game.spawn_structure(&def, x, y, None);
            let node = def
                .machine_recipe(&item)
                .filter(|_| def.extracts())
                .map(|r| ResourceNode {
                    resource: item.clone(),
                    level: r.level,
                });
            let mut entity = game.world.entity_mut(machine);
            entity.insert(crate::components::MachineRecipe(Some(item)));
            if let Some(node) = node {
                entity.insert(node);
            }
        } else {
            spawn_machine_at(game, &kind, x, y);
        }
    }
    node
}

/// Puts the whole bill of the named research node onto a scratch base shelf, so
/// `Game::settle_research`'s second gate is satisfied without a production run.
///
/// A shelf rather than the player's pack, deliberately: a project's bill is
/// paid off the base's buffers through `stock::spend_bill_from_base`, and a
/// fixture that stocked the pack would leave every completion test green
/// against a bill nobody could pay.
pub(crate) fn shelve_research_bill(game: &mut Game, id: &str, x: i32, y: i32) -> Entity {
    let materials = game
        .world
        .resource::<ResearchDb>()
        .get(id)
        .map(|def| def.materials.clone())
        .unwrap_or_default();
    let mut stock = Stock::new(1_000_000);
    for (item, need) in &materials {
        *stock.output.entry(item.clone()).or_default() += need;
    }
    game.world
        .spawn((
            Structure {
                kind: "test_research_shelf".to_string(),
            },
            Position { x, y },
            stock,
        ))
        .id()
}

/// Credits the active project with the full progress a Research Node would have
/// fed it, so a completion test does not have to run one for hundreds of ticks.
pub(crate) fn fill_research_progress(game: &mut Game, id: &str) {
    let cost = game
        .world
        .resource::<ResearchDb>()
        .get(id)
        .map_or(0, |def| def.cost);
    game.world
        .resource_mut::<crate::resources::ActiveResearch>()
        .progress
        .insert(id.to_string(), cost);
}

/// How far the base has got on `id` — see `resources::ActiveResearch`.
pub(crate) fn research_progress(game: &Game, id: &str) -> u32 {
    game.world
        .resource::<crate::resources::ActiveResearch>()
        .progress
        .get(id)
        .copied()
        .unwrap_or(0)
}

/// Which project the base is working, if any.
pub(crate) fn active_research(game: &Game) -> Option<String> {
    game.world
        .resource::<crate::resources::ActiveResearch>()
        .id
        .clone()
}

/// Stocks every material a structure upgrade asks for *besides* the fragment
/// economy, so a fixture that only wanted to raise a tier does not have to
/// know which zone's material the ladder currently runs on.
///
/// Derived from the shipped `UpgradeDef`s rather than named here: a second
/// material tier joins this the moment its `.ron` file does, which is the
/// whole reason it exists — seven fixtures went red at once when the first
/// one landed, none of them about upgrades costing a new thing.
///
/// Deliberately leaves the currencies alone. Several callers assert on an
/// exact fragment delta across the upgrade, and topping those up here would
/// quietly rewrite what they measure.
pub(crate) fn stock_upgrade_materials(game: &mut Game, qty: u32) {
    let currencies = [game.currency(), game.craft_currency()];
    let wanted: std::collections::BTreeSet<ItemId> = game
        .structure_defs()
        .iter()
        .filter_map(|def| def.upgrade.as_ref())
        .flat_map(|upgrade| upgrade.cost.iter().map(|(id, _)| id.clone()))
        .filter(|id| !currencies.contains(id))
        .collect();
    for id in wanted {
        give(game, &id, qty);
    }
}

/// Reads the bank directly rather than through `PlayerStatus::inventory`,
/// which deliberately omits banked items — see `Game::banked`.
pub(crate) fn research_data_held(game: &Game) -> u32 {
    game.banked(&ItemId::from(ids::RESEARCH_DATA))
}

/// Marks `id` researched with a decompiler grant, overwriting whatever the
/// shipped node says, so the fold is tested against stated figures.
pub(crate) fn research_decompiler(
    game: &mut Game,
    id: &str,
    grant: crate::research::DecompilerUpgrade,
) {
    let mut db = game.world.resource_mut::<crate::research::ResearchDb>();
    let mut def = db.get("routine_fabrication").unwrap().clone();
    def.id = id.to_string();
    def.requires.clear();
    def.decompiler = Some(grant);
    db.insert_for_test(def);
    game.world
        .resource_mut::<crate::resources::Research>()
        .0
        .insert(id.to_string());
}
