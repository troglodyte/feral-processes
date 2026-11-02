//! A research project's material bill is carried to the Research Station a
//! load at a time and consumed into the project as it lands — never spent off
//! the shelves in one go at the end.

use super::*;

/// A node whose data takes far longer to earn than the bill takes to carry,
/// so a test can watch the bill go in while the data is still accruing.
const SLOW_NODE: &str = r#"(
    id: "slow_billed",
    name: "Slow Billed Node",
    description: "A node with a bill and a long data cost.",
    cost: 5000,
    materials: [("core_fragment", 12)],
)"#;

fn slow_assets(tag: &str) -> ScratchAssets {
    assets_dir_with_extra_research(tag, "slow_billed.ron", SLOW_NODE)
}

/// The Depot `base_with_a_research_node` stands on laid floor.
fn fixture_depot(game: &mut Game) -> Entity {
    let mut query = game.world.query::<(Entity, &Structure)>();
    query
        .iter(&game.world)
        .find(|(_, s)| s.kind == "depot")
        .map(|(e, _)| e)
        .expect("the research fixture stands a Depot")
}

fn set_shelf(game: &mut Game, shelf: Entity, item: &str, qty: u32) {
    let mut stock = game.world.get_mut::<Stock>(shelf).unwrap();
    if qty == 0 {
        stock.output.remove(&ItemId::from(item));
    } else {
        stock.output.insert(ItemId::from(item), qty);
    }
}

/// Everything but the depot's shelf emptied of `item`, so the depot is the
/// one place the base holds it.
fn clear_item_everywhere(game: &mut Game, item: &str) {
    let shelves: Vec<Entity> = {
        let mut query = game.world.query_filtered::<Entity, With<Stock>>();
        query.iter(&game.world).collect()
    };
    for shelf in shelves {
        set_shelf(game, shelf, item, 0);
    }
}

/// A Research Station with a program posted at it, a project selected, and
/// nothing on any shelf yet.
fn station_with_project(game: &mut Game, id: &str) -> (Entity, Entity) {
    let station = base_with_a_research_node(game);
    game.select_research(id).unwrap();
    let worker = spawn_tamed(game, 10, 3);
    park_at_post(game, worker, station);
    clear_item_everywhere(game, ids::CORE_FRAGMENT);
    (station, fixture_depot(game))
}

fn delivered(game: &Game, id: &str, item: &str) -> u32 {
    game.research_delivered(id, &ItemId::from(item))
}

/// The bug this file exists for. The bill used to be one batch order filed at
/// selection: the moment the base held all twelve the order completed and
/// vanished, and with the data still accruing nothing could spend them. Eaten
/// by anything else before the data finished, they were gone for good and the
/// project said "needs Core Fragment" forever with nothing in the queue.
///
/// Fed, the twelve are in the project before anything else can take them.
#[test]
fn a_bill_held_briefly_and_then_eaten_elsewhere_still_completes() {
    let assets = slow_assets("research_fed_reproducer");
    let mut game = Game::new(9701, DifficultyMode::Forgiving, &assets).unwrap();
    let (_, depot) = station_with_project(&mut game, "slow_billed");
    set_shelf(&mut game, depot, ids::CORE_FRAGMENT, 12);

    for _ in 0..600 {
        game.tick();
    }
    assert!(
        research_progress(&game, "slow_billed") < 5000,
        "the data is still accruing — the case the bug needed"
    );
    // Whatever is still on a shelf is eaten by something else.
    clear_item_everywhere(&mut game, ids::CORE_FRAGMENT);
    fill_research_progress(&mut game, "slow_billed");
    game.tick();

    assert!(
        game.is_researched("slow_billed"),
        "the bill went into the project while it was on the shelves"
    );
}

/// Ticks until `done` holds or `limit` ticks pass; reports whether it held.
fn tick_until(game: &mut Game, limit: u32, done: impl Fn(&Game) -> bool) -> bool {
    for _ in 0..limit {
        if done(game) {
            return true;
        }
        game.tick();
    }
    done(game)
}

fn fragments_held(game: &Game) -> u32 {
    crate::game::base::work_orders::base_holding(game, &ItemId::from(ids::CORE_FRAGMENT))
}

/// Units go into the project as they land, while the data is still accruing,
/// and the bill's remaining need falls by exactly what arrived.
#[test]
fn a_delivery_lowers_what_the_bill_still_needs() {
    let assets = slow_assets("research_fed_incremental");
    let mut game = Game::new(9702, DifficultyMode::Forgiving, &assets).unwrap();
    let (station, depot) = station_with_project(&mut game, "slow_billed");
    set_shelf(&mut game, depot, ids::CORE_FRAGMENT, 5);

    assert!(tick_until(&mut game, 600, |g| {
        delivered(g, "slow_billed", ids::CORE_FRAGMENT) == 5
    }));

    assert!(research_progress(&game, "slow_billed") < 5000);
    assert_eq!(node_output(&game, depot, ids::CORE_FRAGMENT), 0);
    let row = research_node(&game, "slow_billed");
    assert_eq!(
        (row.materials[0].delivered, row.materials[0].need),
        (5, 12),
        "the screen reads delivered against needed"
    );
    let def = game
        .world
        .resource::<ResearchDb>()
        .get("slow_billed")
        .cloned()
        .unwrap();
    assert_eq!(
        game.research_outstanding(&def),
        vec![(ItemId::from(ids::CORE_FRAGMENT), 7)]
    );
    game.tick();
    assert!(
        game.world.get::<Stock>(station).unwrap().input.is_empty(),
        "what lands in the hopper is consumed, not stored"
    );
}

/// The base never has to hold the whole bill: a shelf a standing order keeps
/// at four refills as the Station drains it, and the project completes.
#[test]
fn a_bill_completes_without_the_base_ever_holding_all_of_it() {
    let assets = slow_assets("research_fed_trickle");
    let mut game = Game::new(9703, DifficultyMode::Forgiving, &assets).unwrap();
    station_with_project(&mut game, "slow_billed");
    spawn_tamed(&mut game, 10, 3);
    game.queue_work_order(WorkOrder::level(ItemId::from(ids::CORE_FRAGMENT), 4))
        .unwrap();
    fill_research_progress(&mut game, "slow_billed");

    let mut most_held = 0;
    for _ in 0..6000 {
        if game.is_researched("slow_billed") {
            break;
        }
        most_held = most_held.max(fragments_held(&game));
        game.tick();
    }

    assert!(game.is_researched("slow_billed"));
    assert!(
        most_held < 12,
        "the base held {most_held} at most — never the whole bill"
    );
}

/// Delivered materials stay with the node through an abandon, never refunded,
/// and are still there when it is picked again.
#[test]
fn abandoning_keeps_what_was_delivered() {
    let assets = slow_assets("research_fed_abandon");
    let mut game = Game::new(9704, DifficultyMode::Forgiving, &assets).unwrap();
    let (_, depot) = station_with_project(&mut game, "slow_billed");
    set_shelf(&mut game, depot, ids::CORE_FRAGMENT, 5);
    assert!(tick_until(&mut game, 600, |g| {
        delivered(g, "slow_billed", ids::CORE_FRAGMENT) == 5
    }));

    game.abandon_research().unwrap();
    assert_eq!(delivered(&game, "slow_billed", ids::CORE_FRAGMENT), 5);
    assert_eq!(fragments_held(&game), 0, "nothing is refunded");

    game.select_research("slow_billed").unwrap();
    assert_eq!(delivered(&game, "slow_billed", ids::CORE_FRAGMENT), 5);
}

/// A hauler carries no more than the line still needs, so a deep shelf is
/// drawn down by exactly the bill and the rest stays where it was.
#[test]
fn a_station_never_takes_more_than_the_bill_needs() {
    let assets = slow_assets("research_fed_overfill");
    let mut game = Game::new(9705, DifficultyMode::Forgiving, &assets).unwrap();
    let (station, depot) = station_with_project(&mut game, "slow_billed");
    set_shelf(&mut game, depot, ids::CORE_FRAGMENT, 30);

    assert!(tick_until(&mut game, 1500, |g| {
        delivered(g, "slow_billed", ids::CORE_FRAGMENT) == 12
    }));
    for _ in 0..200 {
        game.tick();
    }

    assert_eq!(delivered(&game, "slow_billed", ids::CORE_FRAGMENT), 12);
    assert_eq!(
        fragments_held(&game),
        18,
        "the shelf keeps everything the bill did not ask for"
    );
    assert!(game.world.get::<Stock>(station).unwrap().input.is_empty());
}

/// The live save this fix was for: the data already in, the bill partly on a
/// Depot's shelf, and nothing in the queue. Loading it must just start the
/// carrying — and the project says it needs a work order for the rest.
#[test]
fn full_data_and_a_part_bill_on_a_depot_starts_the_carrying() {
    let assets = slow_assets("research_fed_live_save");
    let mut game = Game::new(9706, DifficultyMode::Forgiving, &assets).unwrap();
    let (_, depot) = station_with_project(&mut game, "slow_billed");
    fill_research_progress(&mut game, "slow_billed");
    set_shelf(&mut game, depot, ids::CORE_FRAGMENT, 10);

    assert!(tick_until(&mut game, 1000, |g| {
        delivered(g, "slow_billed", ids::CORE_FRAGMENT) == 10
    }));

    let want =
        crate::game::base::work_orders::no_order_for(&game, &ItemId::from(ids::CORE_FRAGMENT));
    assert_eq!(
        research_node(&game, "slow_billed").blocked_by.as_deref(),
        Some(want.as_str())
    );
}

/// The bill files no order of its own, so a running project whose material
/// nothing supplies has to say so — and stops saying it once an order or the
/// shelves cover the rest.
#[test]
fn an_unsupplied_material_asks_for_a_work_order() {
    let assets = slow_assets("research_fed_blocker");
    let mut game = Game::new(9707, DifficultyMode::Forgiving, &assets).unwrap();
    let (_, depot) = station_with_project(&mut game, "slow_billed");
    let fragment = ItemId::from(ids::CORE_FRAGMENT);
    let want = crate::game::base::work_orders::no_order_for(&game, &fragment);
    assert!(
        want.contains("work order") && want.contains("Core Fragment"),
        "{want}"
    );

    assert_eq!(
        research_node(&game, "slow_billed").blocked_by.as_deref(),
        Some(want.as_str())
    );
    assert_eq!(
        game.research_readout(),
        Some(ResearchReadout::Blocked {
            name: "Slow Billed Node".to_string(),
            reason: want.clone(),
        })
    );

    set_shelf(&mut game, depot, ids::CORE_FRAGMENT, 12);
    assert_eq!(
        research_node(&game, "slow_billed").blocked_by,
        None,
        "shelves covering the rest are no block"
    );

    set_shelf(&mut game, depot, ids::CORE_FRAGMENT, 0);
    game.queue_work_order(WorkOrder::batch(fragment, 3))
        .unwrap();
    assert_eq!(
        research_node(&game, "slow_billed").blocked_by,
        None,
        "an order making it is no block"
    );
}

/// A plain item on a stockpile cell is the base's, and the Station's program
/// lifts it from there too.
#[test]
fn a_stockpiled_unit_is_carried_to_the_station() {
    let assets = slow_assets("research_fed_stockpile");
    let mut game = Game::new(9708, DifficultyMode::Forgiving, &assets).unwrap();
    station_with_project(&mut game, "slow_billed");
    game.toggle_stockpile_box((1, 0), (1, 0));
    assert_eq!(game.stockpile_cells(), vec![(1, 0)]);
    crate::game::base::floor::spawn_floor_item(
        &mut game.world,
        Position { x: 1, y: 0 },
        crate::items::FloorThing::Item(ItemId::from(ids::CORE_FRAGMENT)),
    );

    assert!(tick_until(&mut game, 600, |g| {
        delivered(g, "slow_billed", ids::CORE_FRAGMENT) == 1
    }));
}

/// What a bill has had delivered survives a real save and load — a RON round
/// trip cannot see a field the writer forgot.
#[test]
fn delivered_materials_survive_a_save_and_load() {
    let assets = slow_assets("research_fed_save");
    let mut game = Game::new(9709, DifficultyMode::Forgiving, &assets).unwrap();
    station_with_project(&mut game, "slow_billed");
    game.world
        .resource_mut::<crate::resources::ActiveResearch>()
        .delivered
        .entry("slow_billed".to_string())
        .or_default()
        .insert(ItemId::from(ids::CORE_FRAGMENT), 7);

    let path = std::env::temp_dir().join(format!(
        "feral_research_delivered_{}.ron",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(delivered(&loaded, "slow_billed", ids::CORE_FRAGMENT), 7);
}

/// An old save's research-filed order carries a `for_research` key that no
/// longer exists. Nothing sets `deny_unknown_fields`, so it loads as the
/// ordinary order it always was.
#[test]
fn an_old_research_order_loads_as_an_ordinary_order() {
    let order: WorkOrder =
        ron::from_str(r#"(item: "core_fragment", qty: 30, for_research: true)"#).unwrap();
    assert_eq!(
        order,
        WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 30)
    );
}
