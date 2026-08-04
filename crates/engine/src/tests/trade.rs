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
    let node = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 4, y: 4 },
            ResourceNode {
                resource: ItemId::from(ids::CORE_FRAGMENT),
                level: None,
            },
        ))
        .id();

    game.assign_cronjob(worker, node).unwrap();
    let label = game.program_activity(worker);
    assert_eq!(label, game.entity_label(node));
    assert!(
        !label.starts_with("guarding "),
        "a worker must not read as a guard"
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
    // power = max_hp + atk + def = 60 + 8 + 2 = 70, so 70/10 = 7.
    let pet = spawn_tamed(&mut game, 60, 8);
    game.world.get_mut::<Stats>(pet).unwrap().def = 2;

    let before = credits(&game);
    game.sell_companion(market, pet).unwrap();

    assert_eq!(credits(&game), before + 7, "a tenth of 70 power");
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
            kind: FieldBuffKind::Def,
            name: "Shield Protocol".to_string(),
            power: 3,
            remaining: 4,
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
    game.world.get_mut::<Stats>(pet).unwrap().def = 0;

    let before = credits(&game);
    game.sell_companion(market, pet).unwrap();
    assert_eq!(credits(&game), before + 1, "3 power still pays 1, not 0");
}

/// `sell_companion` checks room for the payout before despawning, the
/// same ordering `sell_item` documents. That guard cannot currently fire:
/// `check_room` only refuses a bank-limited item, and the only shipped
/// item with a `bank_limit` is Research Data, not the trade currency.
///
/// It stays anyway, because which item is currency and whether it is
/// banked are both `assets/items/` data — a mod can make this reachable
/// without touching Rust. This test pins the assumption that makes the
/// guard currently inert, so that if a future change banks the currency
/// it fails here and points at the ordering rather than surfacing as
/// programs vanishing for no payment.
#[test]
fn the_trade_currency_is_unbanked_so_a_payout_can_always_land() {
    let game = Game::new(122, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let currency = game.currency();
    assert_eq!(
        game.world
            .resource::<ItemDb>()
            .get(currency.as_str())
            .and_then(|d| d.bank_limit),
        None,
        "if the currency gains a bank_limit, re-check sell_companion's \
         check_room-before-despawn ordering — a refusal after the despawn \
         would destroy the program for nothing"
    );
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
    game.world.get_mut::<Stats>(pet).unwrap().def = 2;

    let options = game.program_sale_options(market);
    assert_eq!(options.len(), 1);
    assert_eq!(options[0].entity, pet);
    assert_eq!(options[0].power, 70);
    assert_eq!(options[0].payout, 7);

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

    game.sell_item(market, ItemId::from(ids::FIREWALL_PLATING), 2)
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
    game.sell_item(market, plating.clone(), 1).unwrap();
    let plating_paid = credits(&game) - before;

    let before = credits(&game);
    give(&mut game, &fragment, 1);
    game.sell_item(market, fragment.clone(), 1).unwrap();
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
    game.sell_item(market, plating, 2).unwrap();

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

    game.sell_item(market, ItemId::from(ids::CORE_FRAGMENT), 4)
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

    game.sell_item(market, plating.clone(), 3).unwrap();

    let paid = game.sell_price(market, &plating).unwrap();
    let shelf = game.buyback_options(market);
    assert_eq!(shelf.len(), 1);
    assert_eq!(shelf[0].item, plating);
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

    game.sell_item(market, plating.clone(), 99).unwrap();

    assert_eq!(game.buyback_options(market)[0].qty, 2);
}

#[test]
fn buying_back_returns_the_item_charges_double_and_empties_the_row() {
    let mut game = Game::new(142, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &plating, 2);
    game.sell_item(market, plating.clone(), 2).unwrap();

    let paid = game.sell_price(market, &plating).unwrap();
    give(&mut game, &ItemId::from(ids::CREDITS), paid * 4);
    let credits_before = credits(&game);

    game.buy_back(market, plating.clone(), 2).unwrap();

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
    game.sell_item(market, plating.clone(), 5).unwrap();
    give(&mut game, &ItemId::from(ids::CREDITS), 100);

    game.buy_back(market, plating.clone(), 2).unwrap();

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
    game.sell_item(market, plating.clone(), 1).unwrap();
    give(&mut game, &ItemId::from(ids::CREDITS), 100);

    assert!(game.buy_back(market, plating.clone(), 2).is_err());
    assert_eq!(
        game.buyback_options(market)[0].qty,
        1,
        "a refused buyback leaves the shelf alone"
    );
    assert!(
        game.buy_back(market, amplifier, 1).is_err(),
        "an item never sold here isn't on the shelf"
    );
}

#[test]
fn buying_back_without_the_credits_is_refused_and_costs_nothing() {
    let mut game = Game::new(145, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market(&mut game);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &plating, 1);
    game.sell_item(market, plating.clone(), 1).unwrap();
    // Buying back costs double what the sale paid, so the proceeds of a sale
    // are never enough to undo it whatever the item is worth.
    let credits_before = credits(&game);

    assert!(game.buy_back(market, plating.clone(), 1).is_err());
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
    game.sell_item(market, plating.clone(), 1).unwrap();
    give(&mut game, &ItemId::from(ids::CREDITS), 100);

    let wild = spawn_wild_on_player_tile(&mut game);
    let player = game.player_entity();
    insert_battle(&mut game, player, vec![wild]);

    assert!(game.buy_back(market, plating, 1).is_err());
}

/// The shelf is the stockroom on a site, not a property of the building, so
/// a raid that levels the trader must not take the player's sales with it.
#[test]
fn a_shelf_outlives_the_trader_standing_on_it() {
    let mut game = Game::new(148, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let market = spawn_market_at(&mut game, 5, 5);
    let plating = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &plating, 3);
    game.sell_item(market, plating.clone(), 3).unwrap();

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
    game.sell_item(market, plating.clone(), 3).unwrap();
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

    game.sell_item(near, plating.clone(), 2).unwrap();

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
    game.sell_item(market, plating.clone(), 2).unwrap();
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
    game.sell_item(market, plating.clone(), 3).unwrap();
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
    game.sell_item(standing, plating.clone(), 3).unwrap();
    // A shelf on a tile whose building is gone has to persist too, or the
    // rebuild-the-same-footprint rule silently stops working across a save.
    game.sell_item(doomed, amplifier.clone(), 1).unwrap();
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
    assert_eq!(shelf[0].item, plating);
    assert_eq!(shelf[0].qty, 3);

    let rebuilt = spawn_market_at(&mut loaded, 9, 9);
    assert_eq!(
        loaded.buyback_options(rebuilt)[0].item,
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
    game.sell_item(market, plating.clone(), 2).unwrap();
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
    game.sell_item(market, plating.clone(), 2).unwrap();

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
        game.sell_item(market, ItemId::from(ids::CREDITS), 1)
            .is_err()
    );
    assert!(
        game.sell_item(market, ItemId::from(ids::NEURAL_AMPLIFIER), 1)
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
