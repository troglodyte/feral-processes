//! The Sites stores: saved with the game, written at the doors, read back as
//! views. A RON round trip cannot see a skipped field, so persistence is
//! asserted through a real `Game::save` and `Game::load`.

use super::breeding::save_and_load;
use super::support::*;
use crate::RouteDestinationId;
use crate::settlements::SettlementKey;
use crate::sites::{SeenOffer, SeenShelf, SiteLogKind};
use crate::*;

fn game() -> Game {
    Game::new(4242, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn texts(game: &Game, site: RouteDestinationId) -> Vec<String> {
    game.sites
        .log
        .newest_first(site)
        .map(|e| e.text.clone())
        .collect()
}

#[test]
fn the_site_log_survives_save_and_load() {
    let mut game = game();
    let town = RouteDestinationId::Settlement(SettlementKey { rx: 2, ry: -1 });
    let outpost = RouteDestinationId::Outpost((7, 9));
    game.note_site(town, SiteLogKind::Visit, "Walked in.".into());
    game.note_site(town, SiteLogKind::Trade, "Sold wafers.".into());
    game.note_site(outpost, SiteLogKind::Founded, "Founded.".into());

    let loaded = save_and_load(&mut game, "site_log");

    assert_eq!(texts(&loaded, town), vec!["Sold wafers.", "Walked in."]);
    assert_eq!(texts(&loaded, outpost), vec!["Founded."]);
    assert_eq!(texts(&loaded, town), texts(&game, town));
}

#[test]
fn a_seen_shelf_survives_save_and_load() {
    let mut game = game();
    let key = SettlementKey { rx: 1, ry: 1 };
    let shelf = SeenShelf {
        tick: 77,
        offers: vec![SeenOffer {
            name: "Logic Wafer".into(),
            detail: "x".into(),
            unit_cost: 12,
            qty: 3,
        }],
    };
    game.sites.seen.record(key, shelf.clone());

    let loaded = save_and_load(&mut game, "seen_shelf");

    assert_eq!(loaded.sites.seen.get(key), Some(&shelf));
}
