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
        want.contains("standing work order") && want.contains("Core Fragment"),
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

/// A worker posted at `station`, standing on the face west of its anchor and
/// holding `qty` Core Fragments — mid-delivery, so a test can land two
/// loads on one tick.
fn carrier_at(game: &mut Game, station: Entity, qty: u32) -> Entity {
    let at = *game.world.get::<Position>(station).unwrap();
    let worker = spawn_tamed(game, 10, 3);
    game.post_worker(worker, station);
    game.world.entity_mut(worker).insert((
        Position {
            x: at.x - 1,
            y: at.y,
        },
        Carrying {
            item: ItemId::from(ids::CORE_FRAGMENT),
            qty,
        },
    ));
    worker
}

/// Two Stations loading the bill's last units on the same tick must not both
/// fill them: the second load finds no room and goes back to a shelf, rather
/// than sitting in a hopper nothing will ever empty.
#[test]
fn two_stations_loading_on_one_tick_do_not_overfill() {
    let assets = slow_assets("research_fed_two_stations");
    let mut game = Game::new(9710, DifficultyMode::Forgiving, &assets).unwrap();
    let (first, _) = station_with_project(&mut game, "slow_billed");
    for x in 9..=14 {
        for y in -1..=4 {
            game.floor_cell(x, y);
        }
    }
    let second = spawn_machine_at(&mut game, "research_node", 11, 1);
    game.world
        .resource_mut::<crate::resources::ActiveResearch>()
        .delivered
        .entry("slow_billed".to_string())
        .or_default()
        .insert(ItemId::from(ids::CORE_FRAGMENT), 7);
    carrier_at(&mut game, first, 5);
    carrier_at(&mut game, second, 5);

    // The tick both arrive: only one load may land. The spill would put a
    // surplus back on a shelf later; this is the room check itself.
    game.tick();
    let hoppers: u32 = [first, second]
        .iter()
        .map(|&s| {
            game.world
                .get::<Stock>(s)
                .unwrap()
                .input
                .values()
                .sum::<u32>()
        })
        .sum();
    assert_eq!(hoppers, 5, "the second Station had no room left to load");

    for _ in 0..200 {
        game.tick();
    }

    assert_eq!(delivered(&game, "slow_billed", ids::CORE_FRAGMENT), 12);
    for station in [first, second] {
        assert!(
            game.world.get::<Stock>(station).unwrap().input.is_empty(),
            "no Station hopper is left holding a surplus"
        );
    }
    assert_eq!(
        fragments_held(&game),
        5,
        "the second load went back to a shelf the base can count"
    );
}

/// A bill with two lines: one a test can strand on an unreachable shelf
/// while the other sits on the fixture's Depot.
const TWO_LINE_NODE: &str = r#"(
    id: "two_line",
    name: "Two Line Node",
    description: "A node whose first line can be cut off.",
    cost: 5000,
    materials: [("power_cell", 2), ("core_fragment", 12)],
)"#;

/// A Depot standing far off the laid floor, where no program can walk.
fn unreachable_depot(game: &mut Game) -> Entity {
    spawn_machine_at(game, "depot", 60, 60)
}

/// The first line's only unit is on a shelf nobody can reach; the second's is
/// on one beside the floor. The second is still carried in — an unreachable
/// line is skipped, not walked at.
#[test]
fn an_unreachable_first_line_does_not_starve_a_reachable_second() {
    let assets =
        assets_dir_with_extra_research("research_fed_two_line", "two_line.ron", TWO_LINE_NODE);
    let mut game = Game::new(9711, DifficultyMode::Forgiving, &assets).unwrap();
    let (_, depot) = station_with_project(&mut game, "two_line");
    clear_item_everywhere(&mut game, "power_cell");
    let far = unreachable_depot(&mut game);
    set_shelf(&mut game, far, "power_cell", 2);
    set_shelf(&mut game, depot, ids::CORE_FRAGMENT, 5);

    assert!(tick_until(&mut game, 600, |g| {
        delivered(g, "two_line", ids::CORE_FRAGMENT) == 5
    }));
    assert_eq!(delivered(&game, "two_line", "power_cell"), 0);
}

/// Once the data is in, a Station is staffed only while a shelf it can walk
/// to holds some of the bill — a unit nobody can reach is no reason to hold a
/// body there.
#[test]
fn a_station_with_only_an_unreachable_shelf_holds_no_body() {
    let assets = slow_assets("research_fed_unreachable_staff");
    let mut game = Game::new(9712, DifficultyMode::Forgiving, &assets).unwrap();
    let station = base_with_a_research_node(&mut game);
    game.select_research("slow_billed").unwrap();
    clear_item_everywhere(&mut game, ids::CORE_FRAGMENT);
    fill_research_progress(&mut game, "slow_billed");
    let far = unreachable_depot(&mut game);
    set_shelf(&mut game, far, ids::CORE_FRAGMENT, 12);
    let worker = spawn_tamed(&mut game, 10, 3);

    for _ in 0..20 {
        game.tick();
    }

    assert!(
        game.world
            .get::<Task>(worker)
            .is_none_or(|t| t.target != station),
        "nothing the Station could fetch, so nobody is posted there"
    );
    // And the reachable case, so the assertion above is not vacuous.
    let near = fixture_depot(&mut game);
    set_shelf(&mut game, near, ids::CORE_FRAGMENT, 3);
    game.tick();
    assert_eq!(
        game.world.get::<Task>(worker).map(|t| t.target),
        Some(station)
    );
}

/// The last units on their way — in a program's hands, or landed in a hopper
/// and not yet absorbed — are supply, so the project is not told it has none.
#[test]
fn the_last_load_on_its_way_is_not_read_as_none() {
    let assets = slow_assets("research_fed_in_flight");
    let mut game = Game::new(9713, DifficultyMode::Forgiving, &assets).unwrap();
    let (station, _) = station_with_project(&mut game, "slow_billed");
    fill_research_progress(&mut game, "slow_billed");
    game.world
        .resource_mut::<crate::resources::ActiveResearch>()
        .delivered
        .entry("slow_billed".to_string())
        .or_default()
        .insert(ItemId::from(ids::CORE_FRAGMENT), 7);
    let def = game
        .world
        .resource::<ResearchDb>()
        .get("slow_billed")
        .cloned()
        .unwrap();

    let carrier = carrier_at(&mut game, station, 5);
    assert_eq!(game.research_block(&def), None, "a load in hand");
    assert_eq!(game.research_material_shortfall(), None);

    game.world.entity_mut(carrier).remove::<Carrying>();
    game.world
        .get_mut::<Stock>(station)
        .unwrap()
        .input
        .insert(ItemId::from(ids::CORE_FRAGMENT), 5);
    assert_eq!(game.research_block(&def), None, "a load in the hopper");
    assert_eq!(game.research_material_shortfall(), None);
}

/// A load that has landed in the hopper but not yet been absorbed when the
/// project is abandoned is credited to that project, not handed to the next.
#[test]
fn abandoning_absorbs_a_load_still_in_the_hopper() {
    let assets = slow_assets("research_fed_abandon_hopper");
    let mut game = Game::new(9715, DifficultyMode::Forgiving, &assets).unwrap();
    let (station, _) = station_with_project(&mut game, "slow_billed");
    game.world
        .get_mut::<Stock>(station)
        .unwrap()
        .input
        .insert(ItemId::from(ids::CORE_FRAGMENT), 5);

    game.abandon_research().unwrap();

    assert_eq!(delivered(&game, "slow_billed", ids::CORE_FRAGMENT), 5);
    assert!(game.world.get::<Stock>(station).unwrap().input.is_empty());
}

/// Makes `id` the active project again. `spawn_machine_with_recipe_at`
/// researches its recipe's chain through the real completion door, which
/// clears whatever project was active.
fn reselect(game: &mut Game, id: &str) {
    game.world
        .resource_mut::<crate::resources::ActiveResearch>()
        .id = Some(id.to_string());
}

/// Lays a strip of floor from the fixture's pocket out to `x_end`, along
/// `y = 0` and one row above, so machines stood there are walkable to.
fn floor_strip(game: &mut Game, x_end: i32) {
    for x in 1..=x_end {
        game.floor_cell(x, 0);
        game.floor_cell(x, -1);
    }
}

/// The shipped stall the review found, in a smaller shape: a standing order
/// for the bill's material is satisfied by units sitting in a producer's
/// buffer beside a consumer the base still wants (its own standing order is
/// met, so nobody works it). The buffer is a feed buffer, so neither the
/// Tend nor idle clearing carries it to a shelf, and the order sleeps
/// because the base already holds the level. With the consumer idle, the
/// Station's program takes the units straight from the buffer.
#[test]
fn a_bill_held_in_an_idle_feed_buffer_is_still_carried_in() {
    let assets = slow_assets("research_fed_feed_buffer");
    let mut game = Game::new(9716, DifficultyMode::Forgiving, &assets).unwrap();
    let (_, depot) = station_with_project(&mut game, "slow_billed");
    floor_strip(&mut game, 24);
    let extractor = spawn_machine_with_recipe_at(&mut game, "extractor", ids::CORE_FRAGMENT, 22, 1);
    spawn_machine_with_recipe_at(&mut game, "factory", "blank_substrate", 23, 1);
    reselect(&mut game, "slow_billed");
    fill_research_progress(&mut game, "slow_billed");
    set_shelf(&mut game, depot, "blank_substrate", 1);
    game.queue_work_order(WorkOrder::level(ItemId::from("blank_substrate"), 1))
        .unwrap();
    game.queue_work_order(WorkOrder::level(ItemId::from(ids::CORE_FRAGMENT), 12))
        .unwrap();
    set_shelf(&mut game, extractor, ids::CORE_FRAGMENT, 12);
    spawn_tamed(&mut game, 10, 3);

    assert!(tick_until(&mut game, 2000, |g| g.is_researched("slow_billed")));
}

/// The other half: a buffer whose neighbour is being worked right now is the
/// line's, and is not counted as research supply.
#[test]
fn a_feed_buffer_an_active_line_is_pulling_is_not_research_supply() {
    let assets = slow_assets("research_fed_busy_buffer");
    let mut game = Game::new(9717, DifficultyMode::Forgiving, &assets).unwrap();
    let (_, _) = station_with_project(&mut game, "slow_billed");
    floor_strip(&mut game, 24);
    let extractor = spawn_machine_with_recipe_at(&mut game, "extractor", ids::CORE_FRAGMENT, 22, 1);
    let factory = spawn_machine_with_recipe_at(&mut game, "factory", "blank_substrate", 23, 1);
    set_shelf(&mut game, extractor, ids::CORE_FRAGMENT, 12);
    let fragment = ItemId::from(ids::CORE_FRAGMENT);
    assert_eq!(game.research_supply(&fragment), 12, "an idle neighbour");

    let worker = spawn_tamed(&mut game, 10, 3);
    game.post_worker(worker, factory);

    assert_eq!(game.research_supply(&fragment), 0, "a worked neighbour");
}

/// What a Station's hopper holds that the bill does not want goes back to a
/// Depot that will take it — and, with none, onto the floor beside the
/// Station — rather than sitting where nothing counts it.
#[test]
fn a_hopper_surplus_with_no_project_spills_past_a_refusing_depot_to_the_floor() {
    let assets = slow_assets("research_fed_spill");
    let mut game = Game::new(9718, DifficultyMode::Forgiving, &assets).unwrap();
    let (station, depot) = station_with_project(&mut game, "slow_billed");
    game.abandon_research().unwrap();
    let grain = ItemId::from("cache_grain");
    game.set_depot_filter(depot, std::slice::from_ref(&grain), false);
    game.world
        .get_mut::<Stock>(station)
        .unwrap()
        .input
        .insert(grain.clone(), 2);

    game.absorb_research_deliveries();

    assert!(game.world.get::<Stock>(station).unwrap().input.is_empty());
    assert_eq!(
        node_output(&game, depot, "cache_grain"),
        0,
        "a Depot that refuses the item is not a place to spill it"
    );
    let on_floor = game
        .world
        .iter_entities()
        .filter(|e| {
            e.get::<crate::components::FloorItem>()
                .is_some_and(|f| f.0 == crate::items::FloorThing::Item(grain.clone()))
        })
        .count();
    assert_eq!(on_floor, 2);
}

/// A bill's only units on a Depot nobody can walk to are no supply: the
/// running project says it cannot reach them, rather than reading as fine
/// while no body is ever posted to fetch them.
#[test]
fn units_on_an_unreachable_shelf_are_flagged_not_counted() {
    let assets = slow_assets("research_fed_unreachable_flag");
    let mut game = Game::new(9719, DifficultyMode::Forgiving, &assets).unwrap();
    station_with_project(&mut game, "slow_billed");
    fill_research_progress(&mut game, "slow_billed");
    let far = unreachable_depot(&mut game);
    set_shelf(&mut game, far, ids::CORE_FRAGMENT, 12);
    let fragment = ItemId::from(ids::CORE_FRAGMENT);

    assert_eq!(game.research_supply(&fragment), 0);
    let want = crate::game::base::work_orders::no_reach_for(&game, &fragment);
    assert_eq!(
        research_node(&game, "slow_billed").blocked_by.as_deref(),
        Some(want.as_str())
    );
    assert!(game.research_material_shortfall().is_some());
}

/// A Station whose first face (by tile order) is a dead end still reaches a
/// shelf from another face.
#[test]
fn a_station_reaches_a_shelf_from_any_of_its_faces() {
    let assets = slow_assets("research_fed_faces");
    let mut game = Game::new(9720, DifficultyMode::Forgiving, &assets).unwrap();
    stand_in_base(&mut game);
    // West face (39, 40) is an island; the east face (42, 40) runs on to a
    // Depot at (45, 40).
    game.floor_cell(39, 40);
    for x in 42..=46 {
        game.floor_cell(x, 40);
        game.floor_cell(x, 41);
    }
    let station = spawn_machine_at(&mut game, "research_node", 40, 40);
    let depot = spawn_machine_at(&mut game, "depot", 45, 41);
    set_shelf(&mut game, depot, ids::CORE_FRAGMENT, 3);

    assert!(game.research_bill_reachable(station, &[ItemId::from(ids::CORE_FRAGMENT)]));
}
