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
                amount: 5,
                capacity: 5,
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

#[test]
fn sell_item_pays_out_credits_at_the_structures_sell_rate() {
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
    let credits_before = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::CREDITS));
    let fragments_before = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::CORE_FRAGMENT));

    game.sell_item(market, ItemId::from(ids::FIREWALL_PLATING), 2)
        .unwrap();

    let inv = game.world.get::<Inventory>(player).unwrap();
    assert_eq!(
        inv.count(&ItemId::from(ids::FIREWALL_PLATING)),
        1,
        "only the sold quantity should leave the inventory"
    );
    let sell_rate = def.trade.as_ref().unwrap().sell_rate;
    assert_eq!(
        inv.count(&ItemId::from(ids::CREDITS)),
        credits_before + sell_rate * 2
    );
    assert_eq!(
        inv.count(&ItemId::from(ids::CORE_FRAGMENT)),
        fragments_before,
        "a trader deals in Credits and never mints build salvage"
    );
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

    let sell_rate = def.trade.as_ref().unwrap().sell_rate;
    let inv = game.world.get::<Inventory>(player).unwrap();
    assert_eq!(inv.count(&ItemId::from(ids::CORE_FRAGMENT)), 0);
    assert_eq!(inv.count(&ItemId::from(ids::CREDITS)), sell_rate * 4);
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
