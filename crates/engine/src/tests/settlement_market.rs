//! A settlement's shelf, its prices, and its buyback — Phase 3's market.
//!
//! `place_settlement`/`generic_settlement_def` (`support.rs`) put the
//! player one tile from a Server, Gear, Open settlement without walking
//! the region derivation — Phase 1's own coverage handles that half.

use super::support::*;
use crate::items::ids;
use crate::resources::GameClock;
use crate::settlements::{SettlementKey, SettlementKind, Specialty, Temperament};
use crate::views::CaravanOfferKind;
use crate::*;

fn game() -> Game {
    Game::new(4242, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn set_tick(game: &mut Game, tick: u64) {
    game.world.resource_mut::<GameClock>().tick = tick;
}

/// Materializes a settlement one tile east of the player — `tests/settlements.rs`'s
/// own fixture, repeated here rather than imported: that file's helper is
/// `pub(super)` to `tests`, which this module already sits inside, but a
/// second name for the same offset would read as two different fixtures if
/// the caravan tests ever grew one too.
fn settlement_east_of_player(game: &mut Game) -> (SettlementKey, (i32, i32)) {
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let target = (pos.x + 1, pos.y);
    let key = SettlementKey { rx: 0, ry: 0 };
    place_settlement(game, key, target.0, target.1);
    (key, target)
}

/// Registers `def` at `key`/`tile` without a map entity or an adjacency
/// requirement — for a test about the shelf or its prices alone, which are
/// pure functions of `resources::Settlements` and need no reach at all.
fn register_settlement(
    game: &mut Game,
    key: SettlementKey,
    def: crate::settlements::SettlementDef,
    tile: (i32, i32),
) {
    game.world
        .resource_mut::<crate::resources::Settlements>()
        .0
        .insert(key, crate::resources::KnownSettlement { tile, def });
}

// ---------------------------------------------------------------------------
// Task 2: the shelf and the prices
// ---------------------------------------------------------------------------

#[test]
fn a_settlement_shelf_is_a_function_of_its_inputs() {
    let mut game = game();
    let key = SettlementKey { rx: 3, ry: -2 };
    register_settlement(&mut game, key, generic_settlement_def(), (0, 0));

    let first = game.settlement_shelf(key, 5);
    let second = game.settlement_shelf(key, 5);

    assert_eq!(first, second, "asking twice must answer the same shelf");
    assert!(!first.is_empty(), "test premise: the shelf drew something");
}

#[test]
fn a_different_epoch_rolls_a_different_shelf() {
    let mut game = game();
    let key = SettlementKey { rx: 1, ry: 1 };
    register_settlement(&mut game, key, generic_settlement_def(), (0, 0));

    let a = game.settlement_shelf(key, 1);
    let b = game.settlement_shelf(key, 2);

    assert_ne!(a, b, "the epoch must reach the shelf's own seed");
}

#[test]
fn a_mainframe_carries_more_rows_than_a_server() {
    let mut game = game();
    let server_key = SettlementKey { rx: 0, ry: 0 };
    let mainframe_key = SettlementKey { rx: 9, ry: -4 };
    let mut server_def = generic_settlement_def();
    server_def.kind = SettlementKind::Server;
    let mut mainframe_def = generic_settlement_def();
    mainframe_def.kind = SettlementKind::Mainframe;
    register_settlement(&mut game, server_key, server_def, (0, 0));
    register_settlement(&mut game, mainframe_key, mainframe_def, (0, 0));

    let server_rows = game.settlement_shelf(server_key, 1).len();
    let mainframe_rows = game.settlement_shelf(mainframe_key, 1).len();

    assert!(
        mainframe_rows > server_rows,
        "a Server drew {server_rows} rows, a Mainframe {mainframe_rows} — \
         a city must carry more than a stop"
    );
}

/// `settlement_shelf` draws through the same `draw_shelf`/`stock_pool`
/// `game::caravan`'s own shelf does — a call, not a copy — but the sharing
/// itself is what a test has to walk rather than take on faith. A tool
/// carrier must never be a settlement's own offer, `no_caravan_shelf_ever_
/// stocks_a_tool_carrier`'s reason: buying one would let Credits skip the
/// research→forge chain the feature exists to make the player earn.
#[test]
fn no_settlement_shelf_ever_stocks_a_tool_carrier() {
    let mut game = game();
    let key = SettlementKey { rx: 5, ry: -5 };
    register_settlement(&mut game, key, generic_settlement_def(), (0, 0));

    for epoch in 0..60 {
        for offer in game.settlement_shelf(key, epoch) {
            if let CaravanOfferKind::Material(item) = &offer.kind {
                assert!(
                    item.tool_id().is_none(),
                    "{} is a tool carrier and must never be stocked for sale",
                    offer.name
                );
            }
        }
    }
}

/// Summed over 20 epochs rather than read off one draw: the weights are a
/// lean, not a guarantee, and a single unlucky roll would make this test
/// flaky against a perfectly good shelf. The sum is still fully
/// deterministic — nothing here reads the wall clock or an unseeded RNG —
/// so a failure here is never a flake.
#[test]
fn each_specialty_actually_biases_its_own_bucket() {
    let mut game = game();
    let key = SettlementKey { rx: 5, ry: 5 };

    let count =
        |game: &mut Game, specialty: Specialty, is_kind: fn(&CaravanOfferKind) -> bool| -> usize {
            let mut def = generic_settlement_def();
            def.kind = SettlementKind::Mainframe;
            def.specialty = specialty;
            register_settlement(game, key, def, (0, 0));
            (0..20u64)
                .map(|epoch| {
                    game.settlement_shelf(key, epoch)
                        .into_iter()
                        .filter(|o| is_kind(&o.kind))
                        .count()
                })
                .sum()
        };
    let is_gear = |k: &CaravanOfferKind| matches!(k, CaravanOfferKind::Gear(_));
    let is_material = |k: &CaravanOfferKind| matches!(k, CaravanOfferKind::Material(_));

    let gear_leaning_gear_rows = count(&mut game, Specialty::Gear, is_gear);
    let materials_leaning_gear_rows = count(&mut game, Specialty::Materials, is_gear);
    assert!(
        gear_leaning_gear_rows > materials_leaning_gear_rows,
        "a Gear settlement drew {gear_leaning_gear_rows} gear rows over 20 epochs, \
         a Materials settlement {materials_leaning_gear_rows} — the specialty must lean \
         its own bucket"
    );

    let materials_leaning_material_rows = count(&mut game, Specialty::Materials, is_material);
    let gear_leaning_material_rows = count(&mut game, Specialty::Gear, is_material);
    assert!(
        materials_leaning_material_rows > gear_leaning_material_rows,
        "a Materials settlement drew {materials_leaning_material_rows} material rows over \
         20 epochs, a Gear settlement {gear_leaning_material_rows}"
    );
}

/// The craft floor is slack on the shipped item set
/// (`no_craftable_is_sold_under_its_parts`'s own note), so this mods in a
/// craftable deliberately priced under its ingredients — the shipped
/// pattern `an_underpriced_craftable_is_still_sold_above_its_parts` follows,
/// one vendor over.
///
/// **Asserted at Open, the cheapest temperament to buy at.** A settlement's
/// markup is scaled by `Temperament::buy_mult` *before* the floor is taken
/// (`Game::marked_unit_cost`), so a discount is exactly where the floor
/// would fail first if it were applied in the other order.
#[test]
fn no_price_at_any_temperament_falls_below_the_craft_floor() {
    const CHEAP: &str = r#"(
        id: "cheap_plate",
        name: "Cheap Plate",
        description: "Worth less than what goes into it.",
        value: Some(1),
        craftable: Some((cost: [("core_fragment", 40)])),
    )"#;
    let dir = modded_assets_dir(
        "settlement_underpriced",
        &[],
        &[("cheap_plate.ron", CHEAP)],
        &[],
        &[],
        &[],
    );
    let game = Game::new(19, DifficultyMode::Forgiving, &dir).unwrap();
    let item = crate::ItemId::from("cheap_plate");
    let parts = game.item_value(&crate::ItemId::from(ids::CORE_FRAGMENT)) * 40;

    for temperament in [
        Temperament::Open,
        Temperament::Guarded,
        Temperament::Mercantile,
    ] {
        let asked = game.settlement_unit_cost(&item, temperament);
        assert!(
            asked > parts,
            "{temperament:?} asked {asked} for something whose parts are worth {parts}, \
             so a player could buy it and sell the parts forever"
        );
    }
}

#[test]
fn an_empty_settlement_shelf_yields_no_market_rather_than_panicking() {
    let mut game = game();
    let key = SettlementKey { rx: 40, ry: 40 };
    // Deliberately not registered: `resources::Settlements` has nothing
    // under this key, the shelf-level reading of an empty catalogue.
    let shelf = game.settlement_shelf(key, 0);
    assert!(shelf.is_empty());
}

/// Open discounts what you pay and pays you more; Guarded is the mirror.
/// Not one of the plan's named gates, but cheap insurance that the six
/// tuning constants actually reach `Game::settlement_unit_cost`/
/// `settlement_sell_price` in the direction the table names.
#[test]
fn open_settlements_charge_less_and_pay_more_than_guarded_ones() {
    let game = game();
    let item = crate::ItemId::from(ids::FIREWALL_PLATING);

    let open_buy = game.settlement_unit_cost(&item, Temperament::Open);
    let guarded_buy = game.settlement_unit_cost(&item, Temperament::Guarded);
    assert!(
        open_buy < guarded_buy,
        "Open asked {open_buy}, Guarded asked {guarded_buy}"
    );

    let open_sell = game.settlement_sell_price(&item, Temperament::Open);
    let guarded_sell = game.settlement_sell_price(&item, Temperament::Guarded);
    assert!(
        open_sell > guarded_sell,
        "Open paid {open_sell}, Guarded paid {guarded_sell}"
    );
}

/// Mercantile is deliberately not the average of Open and Guarded — it
/// competes on the buy side and takes its margin on the sell side.
#[test]
fn mercantile_settlements_pay_the_least_of_the_three() {
    let game = game();
    let item = crate::ItemId::from(ids::FIREWALL_PLATING);

    let open_sell = game.settlement_sell_price(&item, Temperament::Open);
    let guarded_sell = game.settlement_sell_price(&item, Temperament::Guarded);
    let mercantile_sell = game.settlement_sell_price(&item, Temperament::Mercantile);

    assert!(
        mercantile_sell < guarded_sell && mercantile_sell < open_sell,
        "Mercantile paid {mercantile_sell}, against Open {open_sell} and Guarded {guarded_sell}"
    );
}

// ---------------------------------------------------------------------------
// The commit path: the settle_basket integration
// ---------------------------------------------------------------------------

/// `Game::settle_basket` owns the funding check and the apply order; it does
/// **not** charge the currency — each vendor's own `apply_buys` closure
/// does. Nothing in the compiler holds that the price it charges is the
/// price the shelf quoted, which is exactly what this asserts.
///
/// **Picks a row with `qty > 1` on purpose.** A gear, routine or program
/// row always quotes `qty: 1`, so a buy closure that charged
/// `unit_cost` alone — silently dropping the `* qty` — would still pass
/// against one of those. Only a `Material` stack, the one kind that can
/// draw more than one, can catch that particular bug; several epochs are
/// swept because not every shelf draws one.
#[test]
fn a_settlement_baskets_quoted_cost_is_exactly_what_gets_charged() {
    let mut game = game();
    let (key, _) = settlement_east_of_player(&mut game);
    // Set (not ticked) so the clock stays exactly where the found offer was
    // quoted from — `commit_settlement_basket` reads its own epoch off the
    // same clock, and the two must land on the same shelf.
    let offer = (0..20u64)
        .find_map(|epoch| {
            set_tick(
                &mut game,
                epoch * crate::tuning::SETTLEMENT_MARKET_ROTATION_TICKS,
            );
            game.settlement_shelf(key, game.settlement_epoch())
                .into_iter()
                .find(|o| matches!(o.kind, CaravanOfferKind::Material(_)) && o.qty > 1)
        })
        .expect("test premise: some epoch's shelf draws a material stack of more than one");
    let quoted = offer.unit_cost * offer.qty;
    give(&mut game, &crate::ItemId::from(ids::CREDITS), quoted * 10);
    let credits_before = credits(&game);

    game.commit_settlement_basket(key, vec![], vec![offer.index])
        .unwrap();

    assert_eq!(
        credits_before - credits(&game),
        quoted,
        "the shelf quoted {quoted} for {} — the buy closure must charge exactly that",
        offer.name
    );
}

#[test]
fn an_unaffordable_settlement_basket_spends_nothing() {
    let mut game = game();
    let (key, _) = settlement_east_of_player(&mut game);
    let epoch = game.settlement_epoch();
    let offer = game
        .settlement_shelf(key, epoch)
        .into_iter()
        .find(|o| !matches!(o.kind, CaravanOfferKind::Program(_)))
        .expect("test premise: the shelf has at least one non-Program row");
    let credits_before = credits(&game);
    assert!(
        credits_before < offer.unit_cost * offer.qty,
        "test premise: the player cannot afford it"
    );

    let outcome = game.commit_settlement_basket(key, vec![], vec![offer.index]);

    assert!(outcome.is_err());
    assert_eq!(credits(&game), credits_before);
}

// ---------------------------------------------------------------------------
// Task 3: buyback, keyed to a settlement
// ---------------------------------------------------------------------------

#[test]
fn selling_a_rare_copy_to_a_town_and_buying_it_back_returns_that_copy() {
    let mut game = game();
    let (key, _) = settlement_east_of_player(&mut game);
    let item = crate::ItemId::from(ids::FIREWALL_PLATING);
    let rare = GearCopy {
        rarity: Rarity::Gold,
        ..GearCopy::plain(item.clone())
    };
    game.add_copies(&rare, 1);

    game.commit_settlement_basket(key, vec![(rare.clone(), 1)], vec![])
        .unwrap();
    assert_eq!(
        game.count_copies(&rare),
        0,
        "test premise: it left the pack"
    );

    let options = game.settlement_buyback_options(key);
    assert_eq!(options.len(), 1, "one row on the settlement's own shelf");
    assert_eq!(
        options[0].copy, rare,
        "the shelf must remember the RARE copy, not fold it into an ordinary one"
    );

    give(
        &mut game,
        &crate::ItemId::from(ids::CREDITS),
        options[0].unit_cost * 10,
    );
    game.settlement_buy_back(key, rare.clone(), 1).unwrap();

    assert_eq!(
        game.count_copies(&rare),
        1,
        "buying it back must return the SAME rare copy, not an ordinary one"
    );
}

#[test]
fn a_towns_shelf_and_a_structures_shelf_at_the_same_tile_do_not_collide() {
    let mut game = game();
    let (key, tile) = settlement_east_of_player(&mut game);
    let market = spawn_market_at(&mut game, tile.0, tile.1);

    let plating = crate::ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &plating, 1);
    // The two sales are made from the two different spaces they are each
    // reachable from, and crossing back is not decoration: `settlement_reach`
    // refuses while the party is in base space, because a `Position` is only
    // a surface tile in `Locale::Open` and the base grid's cells would
    // otherwise be compared against a town's coordinates.
    from_inside_the_base(&mut game, |game| {
        game.sell_item(market, gear(&plating, 0), 1).unwrap();
    });

    let rare = GearCopy {
        rarity: Rarity::Gold,
        ..GearCopy::plain(plating.clone())
    };
    game.add_copies(&rare, 1);
    game.commit_settlement_basket(key, vec![(rare.clone(), 1)], vec![])
        .unwrap();

    let structure_shelf = game.buyback_options(market);
    let settlement_shelf = game.settlement_buyback_options(key);
    assert_eq!(structure_shelf.len(), 1, "the structure's own shelf");
    assert_eq!(settlement_shelf.len(), 1, "the settlement's own shelf");
    assert_ne!(
        structure_shelf[0].copy, settlement_shelf[0].copy,
        "test premise: two different copies, so a collided key would show one \
         shelf's item on the other's row"
    );
}

/// Every refusal `Game::commit_settlement_basket` can give leaves the purse
/// and the pack exactly as they were.
///
/// **Asserted per refusal, which is the whole point of the seam.** Nine
/// paths can refuse and most of them return before the function has done
/// anything at all, so a single test over one of them passes against the
/// eight that were never at risk — `commit_caravan_basket`'s own
/// `every_refusal_leaves_credits_and_cargo_exactly_as_they_were` states the
/// same rule one vendor over. The one that has to be checked hardest is the
/// funding refusal, since by then the sells have already been *planned*.
#[test]
fn every_settlement_refusal_leaves_the_purse_and_the_pack_alone() {
    let mut game = game();
    let (key, tile) = settlement_east_of_player(&mut game);
    let item = ItemId::from(ids::CORE_FRAGMENT);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(item.clone(), 3);

    let epoch = game.settlement_epoch();
    let dearest = game
        .settlement_shelf(key, epoch)
        .into_iter()
        .max_by_key(|o| o.unit_cost * o.qty)
        .expect("test premise: the shelf is not empty");

    let holdings = |g: &Game| -> (u32, u32) {
        let held = g
            .world
            .get::<Inventory>(g.player_entity())
            .map(|inv| inv.count(&ItemId::from(ids::CORE_FRAGMENT)))
            .unwrap_or(0);
        (credits(g), held)
    };
    let before = holdings(&game);

    // Each entry is one refusal path, checked on its own — the assertion
    // after every one is what stops a later path being covered only by an
    // earlier path's early return.
    let refusals: Vec<(&str, Box<dyn Fn(&mut Game) -> Result<String, String>>)> = vec![
        (
            "an empty basket",
            Box::new(move |g: &mut Game| g.commit_settlement_basket(key, vec![], vec![])),
        ),
        (
            "a row that is not on the shelf",
            Box::new(move |g: &mut Game| g.commit_settlement_basket(key, vec![], vec![9_999])),
        ),
        (
            "selling nothing",
            Box::new(move |g: &mut Game| {
                g.commit_settlement_basket(
                    key,
                    vec![(GearCopy::plain(ItemId::from(ids::CORE_FRAGMENT)), 0)],
                    vec![],
                )
            }),
        ),
        (
            "selling what you do not hold",
            Box::new(move |g: &mut Game| {
                g.commit_settlement_basket(
                    key,
                    vec![(GearCopy::plain(ItemId::from("nothing_at_all")), 1)],
                    vec![],
                )
            }),
        ),
        (
            "a basket the purse cannot fund",
            Box::new(move |g: &mut Game| {
                g.commit_settlement_basket(key, vec![], vec![dearest.index])
            }),
        ),
    ];
    assert!(
        before.0 < dearest.unit_cost * dearest.qty,
        "test premise: a fresh player cannot afford the dearest row"
    );

    for (what, refuse) in refusals {
        assert!(
            refuse(&mut game).is_err(),
            "{what} should have been refused"
        );
        assert_eq!(holdings(&game), before, "{what} spent something");
    }

    // And out of reach, which is the one refusal that is a property of where
    // the party is standing rather than of the basket.
    let away = Position {
        x: tile.0 + 9,
        y: tile.1 + 9,
    };
    *game.world.get_mut::<Position>(player).unwrap() = away;
    assert!(
        game.commit_settlement_basket(key, vec![], vec![dearest.index])
            .is_err(),
        "a town nine tiles away should refuse to trade"
    );
    assert_eq!(
        holdings(&game),
        before,
        "an out-of-reach basket spent something"
    );
}
