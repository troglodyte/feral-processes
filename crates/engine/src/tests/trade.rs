//! Buying and selling at a trader, including selling programs off the roster.

use super::support::*;
use crate::*;

/// A guard used to read as being "on a cronjob" everywhere a program was
/// listed: `PetInfo::job_structure` was `Task.target`'s label with no
/// regard for `TaskKind`, and all three of its consumers wrapped it as
/// "on a cronjob". Party membership was shown nowhere at all.
#[test]
fn program_activity_tells_a_guard_apart_from_a_worker() {
    let mut game = Game::new(130, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let idle = spawn_tamed(&mut game, 30, 5);
    let fighter = spawn_tamed(&mut game, 30, 5);
    let guard = spawn_tamed(&mut game, 30, 5);
    let market = spawn_market(&mut game);

    assert_eq!(game.program_activity(idle), "idle");

    game.add_companion(fighter).unwrap();
    assert_eq!(game.program_activity(fighter), "in party");

    game.assign_guard(guard, market).unwrap();
    let label = game.program_activity(guard);
    assert!(
        label.starts_with("guarding "),
        "a guard must not read as a worker, got {label:?}"
    );
    assert!(
        label.contains(&game.entity_label(market)),
        "and it must name what it is guarding, got {label:?}"
    );
}

/// A cronjob worker reads as the structure it works, with no verb — the
/// bare name is what distinguishes it from a guard.
#[test]
fn program_activity_names_the_structure_a_worker_is_on() {
    let mut game = Game::new(131, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 30, 5);
    let node = spawn_mining_node(&mut game, 4, 4);

    game.assign_cronjob(worker, node).unwrap();
    let label = game.program_activity(worker);
    assert_eq!(label, game.entity_label(node));
    assert!(
        !label.starts_with("guarding "),
        "a worker must not read as a guard"
    );
}

/// `program_post` is the one read of a `Task` into a label, and
/// `program_activity` is built on top of it — so the manifest's row and the
/// terse status every dialog shows cannot disagree about which structure a
/// program is standing at, or about which of the two jobs it is doing.
///
/// Two games rather than one, on the two seeds the neighbouring activity
/// tests already prove: `assign_cronjob` refuses a machine with no walkable
/// unoccupied neighbour to stand at, so which tiles a post can be built on
/// is a property of the seed's terrain, and pairing an arbitrary seed with
/// two structures is how this reads as a broken feature instead of a fixture
/// that got unlucky.
#[test]
fn program_post_names_the_structure_and_which_job_it_is() {
    let mut game = Game::new(130, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let idle = spawn_tamed(&mut game, 30, 5);
    let fighter = spawn_tamed(&mut game, 30, 5);
    let guard = spawn_tamed(&mut game, 30, 5);
    let market = spawn_market(&mut game);

    game.assign_guard(guard, market).unwrap();
    game.add_companion(fighter).unwrap();

    assert_eq!(
        game.program_post(guard),
        Some((TaskKind::Guard, game.entity_label(market))),
        "a guard's post carries the kind, so the renderer picks the word"
    );
    assert_eq!(
        game.program_post(idle),
        None,
        "a program with no Task is posted nowhere"
    );
    assert_eq!(
        game.program_post(fighter),
        None,
        "and neither is one in the party — `add_companion` clears the task"
    );

    let mut game = Game::new(131, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 30, 5);
    let node = spawn_mining_node(&mut game, 4, 4);
    game.assign_cronjob(worker, node).unwrap();

    assert_eq!(
        game.program_post(worker),
        Some((TaskKind::GatherResource, game.entity_label(node)))
    );
}

/// The manifest's WORK box carries the post, which is what turns the bare
/// structure name in the header's run of tags into a stated assignment.
#[test]
fn a_manifest_names_the_structure_a_program_is_posted_to() {
    let mut game = Game::new(134, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 30, 5);
    let node = spawn_mining_node(&mut game, 4, 4);
    game.assign_cronjob(worker, node).unwrap();

    let view = game
        .manifest(worker)
        .expect("a tamed program has a manifest");
    let ManifestSubject::Program(p) = view.subject else {
        panic!("a creature is a Program subject");
    };
    assert_eq!(
        p.post,
        Some((TaskKind::GatherResource, game.entity_label(node)))
    );
}

/// The trader's rows carry it too, so the screen that permanently erases
/// a program says what that program is currently doing.
#[test]
fn a_sale_row_carries_the_programs_activity() {
    let mut game = Game::new(132, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let fighter = spawn_tamed(&mut game, 30, 5);
    game.add_companion(fighter).unwrap();

    let options = game.program_sale_options(market);
    let row = options
        .iter()
        .find(|o| o.entity == fighter)
        .expect("the party member is still sellable");
    assert_eq!(row.activity, "in party");
}

#[test]
fn selling_a_program_pays_a_tenth_of_its_power_and_despawns_it() {
    let mut game = Game::new(120, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    // `Stats::power` prices mitigation as the effective HP it buys rather
    // than summing a percentage into a total: 60 / (1 - 0.02) = 61, + 8 atk
    // = 69, so 69/10 = 6.
    let pet = spawn_tamed(&mut game, 60, 8);
    game.world.get_mut::<Stats>(pet).unwrap().mitigation = 2;

    let before = credits(&game);
    game.sell_companion(market, pet).unwrap();

    assert_eq!(credits(&game), before + 6, "a tenth of 69 power");
    assert!(
        game.world.get::<Stats>(pet).is_none(),
        "the sold program has to be gone, not merely stood down"
    );
}

/// `dissolve_tamed_program` (which `sell_companion` calls) despawns the
/// whole entity, so a `FieldBuff` riding on a sold program needs no
/// dedicated hook to avoid being orphaned — this pins that down rather than
/// trusting it stays true across future edits to the despawn path.
#[test]
fn selling_a_buffed_companion_leaves_no_orphaned_field_buff() {
    let mut game = Game::new(133, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let pet = spawn_tamed(&mut game, 30, 5);
    game.arm_field_buff(
        pet,
        ActiveFieldBuff {
            kind: FieldBuffKind::Mitigation,
            name: "Shield Protocol".to_string(),
            power: 3,
            remaining: 4,
            interval: 1,
            source: BuffSource::Routine,
        },
    );

    game.sell_companion(market, pet).unwrap();

    assert!(
        game.world.get::<Stats>(pet).is_none(),
        "the sold program has to be gone"
    );
    assert!(
        game.world.get::<FieldBuff>(pet).is_none(),
        "its FieldBuff must not survive as an orphaned component on a despawned entity"
    );
}

/// The floor exists so a sale can never destroy a program for nothing.
#[test]
fn a_program_too_weak_to_price_still_sells_for_one_credit() {
    let mut game = Game::new(121, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let pet = spawn_tamed(&mut game, 2, 1);
    game.world.get_mut::<Stats>(pet).unwrap().mitigation = 0;

    let before = credits(&game);
    game.sell_companion(market, pet).unwrap();
    assert_eq!(credits(&game), before + 1, "3 power still pays 1, not 0");
}

/// Whatever the reason, a refused sale must leave the program alive.
#[test]
fn a_refused_sale_never_destroys_the_program() {
    let mut game = Game::new(127, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let pet = spawn_tamed(&mut game, 30, 5);

    // Not the player's to sell.
    let stranger = game.world.spawn(()).id();
    game.world.get_mut::<Tamed>(pet).unwrap().owner = stranger;
    assert!(game.sell_companion(market, pet).is_err());
    assert!(game.world.get::<Stats>(pet).is_some());

    // Mid-battle.
    game.world.get_mut::<Tamed>(pet).unwrap().owner = game.player_entity();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    game.start_battle(vec![wild]);
    assert!(game.sell_companion(market, pet).is_err());
    assert!(
        game.world.get::<Stats>(pet).is_some(),
        "a program must survive a sale refused mid-intrusion"
    );
}

#[test]
fn a_trader_that_does_not_buy_programs_refuses() {
    let mut game = Game::new(123, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let kind = game
        .structure_defs()
        .into_iter()
        .find(|d| d.trade.is_none())
        .expect("plenty of structures don't trade")
        .id
        .clone();
    let not_a_trader = game
        .world
        .spawn((Structure { kind }, Position { x: 5, y: 5 }))
        .id();
    let pet = spawn_tamed(&mut game, 30, 5);

    assert!(game.sell_companion(not_a_trader, pet).is_err());
    assert!(game.world.get::<Stats>(pet).is_some());
}

#[test]
fn selling_detaches_the_program_from_its_party_slot_and_its_job() {
    let mut game = Game::new(124, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let worker = spawn_tamed(&mut game, 30, 5);
    let fighter = spawn_tamed(&mut game, 30, 5);
    game.add_companion(fighter).unwrap();
    game.assign_guard(worker, market).unwrap();
    assert!(game.world.get::<Task>(worker).is_some());

    game.sell_companion(market, worker).unwrap();
    game.sell_companion(market, fighter).unwrap();

    assert!(
        !game.world.resource::<Party>().0.contains(&fighter),
        "a sold party member must leave the party"
    );
    assert!(
        game.player_status().companions.is_empty(),
        "nothing sold should still be listed"
    );
}

/// The whole point of the feature: a full roster stops being a dead end.
#[test]
fn selling_a_program_frees_a_roster_slot() {
    let mut game = Game::new(125, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let capacity = game.pet_capacity();
    let pets: Vec<Entity> = (0..capacity)
        .map(|_| spawn_tamed(&mut game, 30, 5))
        .collect();
    assert_eq!(game.pet_count(), capacity, "roster should be full");

    game.sell_companion(market, pets[0]).unwrap();

    assert_eq!(
        game.pet_count(),
        capacity - 1,
        "selling has to free the slot, or the feature does nothing"
    );
}

#[test]
fn program_sale_options_price_each_program_and_are_empty_for_a_non_buyer() {
    let mut game = Game::new(126, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let pet = spawn_tamed(&mut game, 60, 8);
    game.world.get_mut::<Stats>(pet).unwrap().mitigation = 2;

    let options = game.program_sale_options(market);
    assert_eq!(options.len(), 1);
    assert_eq!(options[0].entity, pet);
    // See `selling_a_program_pays_a_tenth_of_its_power_and_despawns_it` for
    // where 69 comes from — `Stats::power` prices mitigation as soak.
    assert_eq!(options[0].power, 69);
    assert_eq!(options[0].payout, 6);

    let kind = game
        .structure_defs()
        .into_iter()
        .find(|d| d.trade.is_none())
        .unwrap()
        .id
        .clone();
    let plain = game
        .world
        .spawn((Structure { kind }, Position { x: 6, y: 6 }))
        .id();
    assert!(game.program_sale_options(plain).is_empty());
}

/// The sale row is where a program is erased for good, so it carries the
/// fusion depth the party and fuse screens show — a 3/3 is the one you
/// least want to part with by accident.
#[test]
fn program_sale_options_carry_the_programs_fusion_depth() {
    let mut game = Game::new(127, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let plain = spawn_tamed(&mut game, 60, 8);
    let fused = spawn_tamed(&mut game, 60, 8);
    game.world.entity_mut(fused).insert(FusionCount(2));

    let options = game.program_sale_options(market);

    assert_eq!(
        options
            .iter()
            .find(|o| o.entity == plain)
            .map(|o| o.fusions),
        Some(0),
        "a program that was never fused reads as 0, not as absent"
    );
    assert_eq!(
        options
            .iter()
            .find(|o| o.entity == fused)
            .map(|o| o.fusions),
        Some(2)
    );
}

/// `sell_rate` is the trader's multiplier now rather than the price itself,
/// so the payout is quoted through `sell_price` — see
/// `sell_item_pays_each_item_its_own_value` for the ladder that made it two
/// numbers instead of one.
#[test]
fn sell_item_pays_for_the_sold_quantity_and_mints_no_salvage() {
    let mut game = Game::new(90, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.trade.is_some())
        .expect("a trading structure (Black Market) should exist");
    let market = game
        .world
        .spawn((
            Structure {
                kind: def.id.clone(),
            },
            Position { x: 5, y: 5 },
        ))
        .id();

    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::FIREWALL_PLATING), 3);
    let credits_before = credits(&game);
    let fragments_before = fragments(&game);
    let unit_price = game
        .sell_price(market, &ItemId::from(ids::FIREWALL_PLATING))
        .unwrap();

    game.sell_item(market, gear(&ItemId::from(ids::FIREWALL_PLATING), 0), 2)
        .unwrap();

    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::FIREWALL_PLATING)),
        1,
        "only the sold quantity should leave the inventory"
    );
    assert_eq!(credits(&game), credits_before + unit_price * 2);
    assert_eq!(
        fragments(&game),
        fragments_before,
        "a trader deals in Credits and never mints build salvage"
    );
}

/// The whole point of the price ladder: what a thing is worth is a property
/// of the thing, not of the counter it is sold over. Researched plating and
/// raw salvage sold at the same trader must not fetch the same Credits.
#[test]
fn sell_item_pays_each_item_its_own_value() {
    let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    let fragment = ItemId::from(ids::CORE_FRAGMENT);
    let rate = game.trade_options(market).unwrap().sell_rate;

    assert!(
        game.item_value(&plating) > game.item_value(&fragment),
        "researched plating has to outprice raw salvage or there is no ladder"
    );

    give(&mut game, &plating, 1);
    let before = credits(&game);
    game.sell_item(market, gear(&plating.clone(), 0), 1)
        .unwrap();
    let plating_paid = credits(&game) - before;

    let before = credits(&game);
    give(&mut game, &fragment, 1);
    game.sell_item(market, gear(&fragment.clone(), 0), 1)
        .unwrap();
    let fragment_paid = credits(&game) - before;

    assert_eq!(plating_paid, game.item_value(&plating) * rate);
    assert_eq!(fragment_paid, game.item_value(&fragment) * rate);
    assert!(plating_paid > fragment_paid);
}

/// The screen quotes the price the sale then honours. Split out because the
/// renderer used to print `sell_rate` itself, which was right only while
/// every item cost the same.
#[test]
fn the_quoted_sell_price_is_what_selling_actually_pays() {
    let mut game = Game::new(92, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &plating, 2);

    let quoted = game.sell_price(market, &plating).unwrap();
    let before = credits(&game);
    game.sell_item(market, gear(&plating, 0), 2).unwrap();

    assert_eq!(credits(&game) - before, quoted * 2);
}

/// The on-ramp that makes a pre-breach sell-off possible: salvage is
/// ordinary goods to a trader now that it isn't the trade currency.
#[test]
fn sell_item_accepts_core_fragments_and_pays_credits_for_them() {
    let mut game = Game::new(94, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.trade.is_some())
        .unwrap();
    let market = game
        .world
        .spawn((
            Structure {
                kind: def.id.clone(),
            },
            Position { x: 5, y: 5 },
        ))
        .id();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.items.clear();
        inv.add(ItemId::from(ids::CORE_FRAGMENT), 4);
    }

    game.sell_item(market, gear(&ItemId::from(ids::CORE_FRAGMENT), 0), 4)
        .unwrap();

    let unit_price = game
        .sell_price(market, &ItemId::from(ids::CORE_FRAGMENT))
        .unwrap();
    let inv = game.world.get::<Inventory>(player).unwrap();
    assert_eq!(inv.count(&ItemId::from(ids::CORE_FRAGMENT)), 0);
    assert_eq!(inv.count(&ItemId::from(ids::CREDITS)), unit_price * 4);
}

/// A sale is no longer one-way: what the trader took goes on its shelf,
/// priced at double what it paid, so walking a sale back costs a fee rather
/// than being impossible.
#[test]
fn selling_stocks_the_shelf_at_double_what_the_trader_paid() {
    let mut game = Game::new(140, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &plating, 5);

    assert!(
        game.buyback_options(market).is_empty(),
        "a trader you have never sold to has nothing to offer back"
    );

    game.sell_item(market, gear(&plating.clone(), 0), 3)
        .unwrap();

    let paid = game.sell_price(market, &plating).unwrap();
    let shelf = game.buyback_options(market);
    assert_eq!(shelf.len(), 1);
    assert_eq!(shelf[0].copy.item, plating);
    assert_eq!(shelf[0].qty, 3, "only what the trader actually took");
    assert_eq!(shelf[0].unit_cost, paid * 2);
}

/// Selling more than you hold sells what you hold — the shelf follows the
/// clamp, not the request.
#[test]
fn the_shelf_records_what_was_taken_not_what_was_asked_for() {
    let mut game = Game::new(141, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &plating, 2);

    game.sell_item(market, gear(&plating.clone(), 0), 99)
        .unwrap();

    assert_eq!(game.buyback_options(market)[0].qty, 2);
}

#[test]
fn buying_back_returns_the_item_charges_double_and_empties_the_row() {
    let mut game = Game::new(142, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &plating, 2);
    game.sell_item(market, gear(&plating.clone(), 0), 2)
        .unwrap();

    let paid = game.sell_price(market, &plating).unwrap();
    give(&mut game, &ItemId::from(ids::CREDITS), paid * 4);
    let credits_before = credits(&game);

    game.buy_back(market, gear(&plating.clone(), 0), 2).unwrap();

    assert_eq!(held(&game, &plating), 2, "the goods come home");
    assert_eq!(credits(&game), credits_before - paid * 4);
    assert!(
        game.buyback_options(market).is_empty(),
        "a bought-out row leaves no empty shelf entry behind"
    );
}

#[test]
fn buying_back_takes_only_part_of_a_stack() {
    let mut game = Game::new(143, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &plating, 5);
    game.sell_item(market, gear(&plating.clone(), 0), 5)
        .unwrap();
    give(&mut game, &ItemId::from(ids::CREDITS), 100);

    game.buy_back(market, gear(&plating.clone(), 0), 2).unwrap();

    assert_eq!(held(&game, &plating), 2);
    assert_eq!(game.buyback_options(market)[0].qty, 3);
}

/// The shelf is finite — it is a record of your own sales, not a shop.
#[test]
fn you_cannot_buy_back_more_than_you_sold() {
    let mut game = Game::new(144, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    let amplifier = ItemId::from(ids::NEURAL_AMPLIFIER);
    give(&mut game, &plating, 1);
    game.sell_item(market, gear(&plating.clone(), 0), 1)
        .unwrap();
    give(&mut game, &ItemId::from(ids::CREDITS), 100);

    assert!(game.buy_back(market, gear(&plating.clone(), 0), 2).is_err());
    assert_eq!(
        game.buyback_options(market)[0].qty,
        1,
        "a refused buyback leaves the shelf alone"
    );
    assert!(
        game.buy_back(market, gear(&amplifier, 0), 1).is_err(),
        "an item never sold here isn't on the shelf"
    );
}

#[test]
fn buying_back_without_the_credits_is_refused_and_costs_nothing() {
    let mut game = Game::new(145, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &plating, 1);
    game.sell_item(market, gear(&plating.clone(), 0), 1)
        .unwrap();
    // Buying back costs double what the sale paid, so the proceeds of a sale
    // are never enough to undo it whatever the item is worth.
    let credits_before = credits(&game);

    assert!(game.buy_back(market, gear(&plating.clone(), 0), 1).is_err());
    assert_eq!(credits(&game), credits_before);
    assert_eq!(game.buyback_options(market)[0].qty, 1);
    assert_eq!(held(&game, &plating), 0);
}

/// Every other trade action is barred mid-battle and after a loss; this one
/// is no different.
#[test]
fn buying_back_is_barred_during_a_battle() {
    let mut game = Game::new(146, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &plating, 1);
    game.sell_item(market, gear(&plating.clone(), 0), 1)
        .unwrap();
    give(&mut game, &ItemId::from(ids::CREDITS), 100);

    let wild = spawn_wild_on_player_tile(&mut game);
    let player = game.player_entity();
    insert_battle(&mut game, player, vec![wild]);

    assert!(game.buy_back(market, gear(&plating, 0), 1).is_err());
}

/// The shelf is the stockroom on a site, not a property of the building, so
/// a raid that levels the trader must not take the player's sales with it.
#[test]
fn a_shelf_outlives_the_trader_standing_on_it() {
    let mut game = Game::new(148, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market_at(&mut game, 5, 5);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &plating, 3);
    game.sell_item(market, gear(&plating.clone(), 0), 3)
        .unwrap();

    game.world.despawn(market);
    let rebuilt = spawn_market_at(&mut game, 5, 5);

    let shelf = game.buyback_options(rebuilt);
    assert_eq!(shelf.len(), 1, "rebuilding the same site reopens the store");
    assert_eq!(shelf[0].qty, 3);
}

/// The cost of keying on the tile: rebuild elsewhere and you have a new
/// store. The stock is not destroyed, just out of reach until something
/// stands on the old footprint again — which is why losing a trader is
/// announced rather than silent.
#[test]
fn a_trader_rebuilt_on_a_different_tile_opens_empty() {
    let mut game = Game::new(149, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market_at(&mut game, 5, 5);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &plating, 3);
    game.sell_item(market, gear(&plating.clone(), 0), 3)
        .unwrap();
    game.world.despawn(market);

    let moved = spawn_market_at(&mut game, 9, 9);
    assert!(game.buyback_options(moved).is_empty());

    let back_home = spawn_market_at(&mut game, 5, 5);
    assert_eq!(game.buyback_options(back_home)[0].qty, 3);
}

#[test]
fn two_traders_in_one_zone_keep_separate_shelves() {
    let mut game = Game::new(150, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let near = spawn_market_at(&mut game, 5, 5);
    let far = spawn_market_at(&mut game, 9, 9);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &plating, 2);

    game.sell_item(near, gear(&plating.clone(), 0), 2).unwrap();

    assert_eq!(game.buyback_options(near)[0].qty, 2);
    assert!(
        game.buyback_options(far).is_empty(),
        "selling to one trader must not stock another"
    );
}

/// The trader kind is part of the key, so a different structure raised on a
/// dead trader's footprint inherits nothing.
#[test]
fn another_structure_on_the_tile_inherits_nothing() {
    let mut game = Game::new(151, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market_at(&mut game, 5, 5);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &plating, 2);
    game.sell_item(market, gear(&plating.clone(), 0), 2)
        .unwrap();
    game.world.despawn(market);

    spawn_structure_at(&mut game, "shield", 5, 5);
    let shield = game
        .world
        .query::<(Entity, &Structure)>()
        .iter(&game.world)
        .find(|(_, s)| s.kind == "shield")
        .map(|(e, _)| e)
        .expect("the shield should be standing");

    assert!(game.buyback_options(shield).is_empty());
}

/// Build salvage and breach keys are wiped at a breach so a stockpile can't
/// fund content it never engaged with. A shelf holding that same salvage
/// would be exactly the loophole, so it goes too.
#[test]
fn a_breach_clears_every_shelf() {
    let mut game = Game::new(152, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market_at(&mut game, 5, 5);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &plating, 3);
    game.sell_item(market, gear(&plating.clone(), 0), 3)
        .unwrap();
    assert_eq!(game.buyback_options(market)[0].qty, 3);

    game.enter_next_zone();

    // Asserting the ledger, not just `buyback_options`: the breach moves the
    // base to a new spawn point, so every trader's tile key changes and the
    // old entry stops matching whether or not anything cleared it. Left
    // behind it would still be saved, and would spring back the moment a
    // trader happened to be rebuilt on the matching tile.
    assert!(
        game.world
            .resource::<resources::BuybackLedger>()
            .0
            .is_empty(),
        "a shelf must not carry a doomed stockpile across a breach"
    );
    assert!(game.buyback_options(market).is_empty());
}

#[test]
fn a_shelf_survives_a_save_and_load_round_trip() {
    let mut game = Game::new(153, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let standing = spawn_market_at(&mut game, 5, 5);
    let doomed = spawn_market_at(&mut game, 9, 9);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    let amplifier = ItemId::from(ids::NEURAL_AMPLIFIER);
    give(&mut game, &plating, 3);
    give(&mut game, &amplifier, 1);
    game.sell_item(standing, gear(&plating.clone(), 0), 3)
        .unwrap();
    // A shelf on a tile whose building is gone has to persist too, or the
    // rebuild-the-same-footprint rule silently stops working across a save.
    game.sell_item(doomed, gear(&amplifier.clone(), 0), 1)
        .unwrap();
    game.world.despawn(doomed);

    let path = std::env::temp_dir().join(format!("feral_buyback_save_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let restored = find_structure_by_kind(
        &mut loaded,
        &game.world.get::<Structure>(standing).unwrap().kind.clone(),
    )
    .expect("the surviving trader should load");
    let shelf = loaded.buyback_options(restored);
    assert_eq!(shelf.len(), 1);
    assert_eq!(shelf[0].copy.item, plating);
    assert_eq!(shelf[0].qty, 3);

    let rebuilt = spawn_market_at(&mut loaded, 9, 9);
    assert_eq!(
        loaded.buyback_options(rebuilt)[0].copy.item,
        amplifier,
        "the orphaned shelf is still on its tile after a reload"
    );
}

/// The same-footprint rule is invisible unless the game says so, and the
/// moment it matters is the moment the trader comes down.
#[test]
fn losing_a_trader_that_holds_stock_says_so() {
    let mut game = Game::new(154, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market_at(&mut game, 5, 5);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &plating, 2);
    game.sell_item(market, gear(&plating.clone(), 0), 2)
        .unwrap();
    // `spawn_market_at` bypasses `place_structure`, so nothing raidable is
    // attached; without this `damage_structure` returns before it can act.
    game.world
        .entity_mut(market)
        .insert(Durability { hp: 1, max_hp: 1 });

    let before = game.world.resource::<MessageLog>().lines.len();
    game.damage_structure(market, u32::MAX, "iso Market");
    let said: Vec<String> = game.world.resource::<MessageLog>().lines[before..]
        .iter()
        .map(|e| e.text.clone())
        .collect();

    assert!(
        said.iter().any(|line| line.contains("Firewall Plating")),
        "the loss must name what was on the shelf, got {said:?}"
    );
    assert!(
        said.iter()
            .any(|line| line.to_lowercase().contains("footprint")),
        "and how to get it back, got {said:?}"
    );
}

#[test]
fn losing_an_empty_trader_is_silent_about_the_shelf() {
    let mut game = Game::new(155, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market_at(&mut game, 5, 5);
    game.world
        .entity_mut(market)
        .insert(Durability { hp: 1, max_hp: 1 });

    let before = game.world.resource::<MessageLog>().lines.len();
    game.damage_structure(market, u32::MAX, "iso Market");

    assert!(
        !game.world.resource::<MessageLog>().lines[before..]
            .iter()
            .any(|e| e.text.to_lowercase().contains("footprint")),
        "a trader with nothing on its shelf loses nothing"
    );
}

/// Demolition is the other way a trader comes down, and it went quiet the
/// first time this was wired up only into the raid path.
#[test]
fn demolishing_a_trader_that_holds_stock_says_so_too() {
    let mut game = Game::new(156, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 0);
    let market = spawn_market_at(&mut game, 5, 5);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &plating, 2);
    game.sell_item(market, gear(&plating.clone(), 0), 2)
        .unwrap();

    let before = game.world.resource::<MessageLog>().lines.len();
    game.remove_structure(market).unwrap();

    assert!(
        game.world.resource::<MessageLog>().lines[before..]
            .iter()
            .any(|e| e.text.to_lowercase().contains("footprint")),
        "demolishing a trader must not go quiet where a raid speaks up"
    );
}

/// A program is destroyed by its sale, not shelved — buying one back would
/// mean resurrecting a despawned entity.
#[test]
fn selling_a_program_stocks_nothing() {
    let mut game = Game::new(147, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let pet = spawn_tamed(&mut game, 60, 8);

    game.sell_companion(market, pet).unwrap();

    assert!(game.buyback_options(market).is_empty());
}

#[test]
fn sell_item_rejects_credits_and_items_you_dont_have() {
    let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.trade.is_some())
        .unwrap();
    let market = game
        .world
        .spawn((
            Structure {
                kind: def.id.clone(),
            },
            Position { x: 5, y: 5 },
        ))
        .id();

    assert!(
        game.sell_item(market, gear(&ItemId::from(ids::CREDITS), 0), 1)
            .is_err()
    );
    assert!(
        game.sell_item(market, gear(&ItemId::from(ids::NEURAL_AMPLIFIER), 0), 1)
            .is_err(),
        "can't sell what you don't have"
    );
}

#[test]
fn buy_item_charges_credits_and_grants_the_item() {
    let mut game = Game::new(92, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.trade.is_some())
        .unwrap();
    let (buy_item, unit_cost) = def.trade.as_ref().unwrap().buy[0].clone();
    let market = game
        .world
        .spawn((
            Structure {
                kind: def.id.clone(),
            },
            Position { x: 5, y: 5 },
        ))
        .id();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.items.clear();
        inv.add(ItemId::from(ids::CREDITS), unit_cost * 2);
    }

    game.buy_item(market, buy_item.clone(), 2).unwrap();

    let inv = game.world.get::<Inventory>(player).unwrap();
    assert_eq!(
        inv.count(&ItemId::from(ids::CREDITS)),
        0,
        "the full cost should be charged"
    );
    assert_eq!(inv.count(&buy_item), 2);
}

#[test]
fn buy_item_fails_without_enough_credits_or_for_an_unlisted_item() {
    let mut game = Game::new(93, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.trade.is_some())
        .unwrap();
    let (buy_item, _) = def.trade.as_ref().unwrap().buy[0].clone();
    let market = game
        .world
        .spawn((
            Structure {
                kind: def.id.clone(),
            },
            Position { x: 5, y: 5 },
        ))
        .id();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .items
        .clear();

    assert!(
        game.buy_item(market, buy_item, 1).is_err(),
        "no Credits should fail the purchase"
    );
    assert!(
        game.buy_item(market, ItemId::from(ids::CORE_FRAGMENT), 1)
            .is_err(),
        "an item not on the buy list shouldn't be purchasable"
    );
}

/// A bank is not a good. The sell menu can't reach this today — its rows
/// come from `PlayerStatus::inventory`, which omits banked items — but that
/// filter is a consequence of this rule rather than a substitute for it, and
/// `sell_item` is public API. Ungated, a Research Node is a slow Credit
/// press: it produces forever, on a timer, out of nothing.
#[test]
fn a_banked_item_cannot_be_sold() {
    let mut game = Game::new(128, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    grant_research_data(&mut game, 40);
    let before = credits(&game);

    let refusal = game
        .sell_item(market, gear(&ItemId::from(ids::RESEARCH_DATA), 0), 10)
        .expect_err("a banked item must not be sellable");

    assert!(
        refusal.contains("can't be traded"),
        "the refusal should say why: {refusal:?}"
    );
    assert_eq!(
        research_data_held(&game),
        40,
        "a refused sale must leave the bank untouched"
    );
    assert_eq!(credits(&game), before, "and must pay nothing");
}

/// A shelf keyed on the item alone would hand a mis-sold T2 back as an
/// ordinary copy, silently deleting the four base copies that made it —
/// the reason `BuybackLedger` rows carry a tier at all. The plain copy sold
/// alongside is what pins the two as separate rows rather than one merged
/// stack.
#[test]
fn selling_a_fused_copy_and_buying_it_back_returns_it_fused() {
    let mut game = Game::new(1441, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &plating, 5);
    game.fuse_item(&gear(&plating, 0)).unwrap();
    game.fuse_item(&gear(&plating, 0)).unwrap();
    game.fuse_item(&gear(&plating, 1)).unwrap();
    assert_eq!(held_at(&game, &plating, 2), 1);
    assert_eq!(held_at(&game, &plating, 0), 1);

    game.sell_item(market, gear(&plating.clone(), 2), 1)
        .unwrap();
    game.sell_item(market, gear(&plating.clone(), 0), 1)
        .unwrap();

    let shelf = game.buyback_options(market);
    assert_eq!(shelf.len(), 2, "the two tiers are separate shelf rows");
    assert_eq!(shelf.iter().filter(|o| o.copy.tier == 2).count(), 1);

    give(&mut game, &ItemId::from(ids::CREDITS), 100);
    game.buy_back(market, gear(&plating.clone(), 2), 1).unwrap();

    assert_eq!(held_at(&game, &plating, 2), 1, "it comes back fused");
    assert_eq!(
        held_at(&game, &plating, 0),
        0,
        "and not as an ordinary copy"
    );
}

/// A trader pays for what a program *is*, never for what the player spent on
/// it — and that is an economy bound, not a flavour preference.
///
/// `program_payout` is a tenth of `Stats::power()`, and a Recompile Kernel
/// doubles every one of those stats for 12 Core Fragments. Core Fragments
/// sell for 1 Credit each, so from zone 3 up the round trip prints money, and
/// it compounds: measured at zone 7 a zone-1 program bought up through six
/// tiers sold for 716 Credits against 72 fragments' worth of kernels. Wild
/// programs are free to tame and Credits are the one currency that survives a
/// breach, so it is a repeatable press rather than a one-off.
///
/// Neither existing bound can see it — `no_craftable_item_is_worth_more_than_
/// its_ingredients` prices the kernel (correctly, 8 against 12) and
/// `every_base_produced_item_sits_at_the_floor_price` prices what structures
/// print. The leak is the program-power channel.
#[test]
fn buying_a_programs_zone_tiers_does_not_raise_what_a_trader_pays() {
    let mut game = Game::new(133, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let pet = spawn_tamed(&mut game, 60, 8);
    game.world.get_mut::<Stats>(pet).unwrap().mitigation = 2;
    game.world.entity_mut(pet).insert(ZonePortal(1));
    let unbumped = game.program_payout(market, pet).unwrap();

    // Two kernels, applied the way a player would.
    game.world.resource_mut::<crate::resources::ZoneLevel>().0 = 3;
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from("recompile_kernel"), 2);
    for _ in 0..2 {
        game.refactor_companion(pet, &ItemId::from("recompile_kernel"))
            .unwrap();
    }

    assert_eq!(
        game.world.get::<Stats>(pet).unwrap().power(),
        208,
        "the program really is three times as strong — this is not a no-op. \
         Three rather than four because the zone curve is linear: tier 1 to \
         3 is x2 then x3/2, where a doubling curve gave x2 then x2. The \
         figure is 208 rather than a clean 69 x 3 because `Stats::power` now \
         prices mitigation as the effective HP it buys, and a kernel raises \
         mitigation by its own percentage while the tier step deliberately \
         does not touch it"
    );
    assert_eq!(
        game.program_payout(market, pet).unwrap(),
        unbumped,
        "but the market pays what it paid before, so the kernels bought no Credits"
    );
}

/// The other side of that rule, and the one it could destroy by accident: a
/// program tamed deep is *legitimately* worth more, because beating it is
/// what the game charges for the tier. Only bought tiers are divided out.
#[test]
fn a_program_tamed_in_a_deep_zone_still_sells_for_what_it_is() {
    let mut game = Game::new(134, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);

    let shallow = spawn_tamed(&mut game, 60, 8);
    game.world.get_mut::<Stats>(shallow).unwrap().mitigation = 2;
    game.world.entity_mut(shallow).insert(ZonePortal(1));

    // Same species tamed three zones down: the spawner scaled it, nobody
    // bought it, and no `PurchasedTiers` records otherwise.
    let deep = spawn_tamed(&mut game, 240, 32);
    game.world.get_mut::<Stats>(deep).unwrap().mitigation = 8;
    game.world.entity_mut(deep).insert(ZonePortal(3));

    // Not a clean x4 any more, and that is `Stats::power`'s redefinition
    // rather than a rule bending: mitigation is priced as the effective HP
    // it buys, so the deep program's 8% is worth more against its 240 HP
    // than the shallow one's 2% is against 60. A payout is a *tenth* of
    // power, so integer division widens the gap further.
    assert_eq!(game.program_payout(market, shallow).unwrap(), 6);
    assert_eq!(
        game.program_payout(market, deep).unwrap(),
        29,
        "earned tiers are still worth every Credit they were"
    );
}

/// `program_payout` prices a program off `Stats::power()` and runs *before*
/// the dissolve, so the strip has to be explicit here rather than inherited
/// from `dissolve_tamed_program` — otherwise the trader pays for gear the
/// player is about to get back.
#[test]
fn selling_a_geared_program_returns_the_gear_and_prices_the_program_alone() {
    let mut game = Game::new(134, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let weapon = ItemId::from(ids::OVERCLOCK_CORE);
    give(&mut game, &weapon, 1);

    let geared = spawn_tamed(&mut game, 60, 8);
    game.world.get_mut::<Stats>(geared).unwrap().mitigation = 2;
    let bare = spawn_tamed(&mut game, 60, 8);
    game.world.get_mut::<Stats>(bare).unwrap().mitigation = 2;
    game.equip(geared, &gear(&weapon, 0)).unwrap();

    let before = credits(&game);
    game.sell_companion(market, geared).unwrap();
    let geared_payout = credits(&game) - before;
    assert_eq!(
        held(&game, &weapon),
        1,
        "the gear is the player's and comes back off a sold program"
    );

    let before = credits(&game);
    game.sell_companion(market, bare).unwrap();
    let bare_payout = credits(&game) - before;

    assert_eq!(
        geared_payout, bare_payout,
        "a sale appraises the program, not the gear it happens to be holding"
    );
}

/// A shelf keeps *the copy that was sold to it*, rare tier included.
///
/// This is `BuybackLedger`'s existing fusion-tier argument reached by a new
/// route: the unit price is deliberately the same at every tier
/// (`Game::item_value` is untouched), so a player who sells a Bare-Metal
/// weapon by mistake gets nothing back for the difference — buying back the
/// *same copy* is the only thing that makes the mistake recoverable. Keyed
/// on the item alone, the shelf would hand over an ordinary copy and quietly
/// delete the tier.
#[test]
fn selling_a_rare_copy_buys_back_the_same_copy() {
    let mut game = Game::new(84, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let post = spawn_market_at(&mut game, 1, 0);
    let rare = GearCopy {
        item: ItemId::from(ids::ABLATIVE_PLATING),
        rarity: Rarity::Prismatic,
        tier: 0,
        affix: None,
    };
    let plain = GearCopy::plain(rare.item.clone());
    game.add_copies(&rare, 1);
    game.add_copies(&plain, 1);

    // Both copies of the same item, so a shelf keyed on the id alone would
    // merge them into one row and lose the tier — which is the failure this
    // exists to catch, and is invisible if only one copy is ever sold.
    game.sell_item(post, rare.clone(), 1).unwrap();
    game.sell_item(post, plain.clone(), 1).unwrap();

    let shelf = game.buyback_options(post);
    assert_eq!(
        shelf.len(),
        2,
        "two copies that differ by rare tier are two shelf rows, got {shelf:?}",
        shelf = shelf.iter().map(|r| r.copy.clone()).collect::<Vec<_>>()
    );
    let shelved_rare = shelf
        .iter()
        .find(|r| r.copy == rare)
        .expect("the Bare-Metal copy must be on the shelf as itself");

    give(&mut game, &ItemId::from(ids::CREDITS), 500);
    game.buy_back(post, shelved_rare.copy.clone(), 1).unwrap();

    assert_eq!(
        game.count_copies(&rare),
        1,
        "buying back must return the Bare-Metal copy, not an ordinary one"
    );
    assert_eq!(
        game.count_copies(&plain),
        0,
        "and must not have quietly handed back the ordinary one instead"
    );
}
