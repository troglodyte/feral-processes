//! The Sites hub's pages: row counts against app-core, widths at the smallest
//! window, and no tick figure on any tab.

use super::*;
use crate::render::base_report::assert_rows_fit;
use crate::render::base_report::sites::*;
use crate::render::popup::row_label_text;
use feral_processes_app_core::{App, GameKey, TOWN_TABS};
use feral_processes_engine::settlements::SettlementKey;
use feral_processes_engine::sites::TownGoods;
use feral_processes_engine::{
    CaravanSchedule, OutpostCrewRow, OutpostReport, RouteDestinationId, RouteLine, SeenOffer,
    SiteLogLine, TownStatus,
};

const LONG_NAME: &str =
    "Overclocked Mirror Exchange Of The Eastern Lattice And Its Many Annexes Ltd";

/// The census app with two visited towns, one with an 80-character name, and
/// two outposts, written straight into a save as app-core's fixtures do.
fn sites_app() -> App {
    use feral_processes_engine::save;
    let mut app = census_app();
    let (px, py) = app.game.as_ref().unwrap().field_pos().unwrap();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let nth = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("fp_gui_sites_{}_{nth}.bin", std::process::id()));
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.settlements.0.clear();
    data.outposts.clear();
    for (i, (name, tile)) in [("Alder", (px, py - 2)), (LONG_NAME, (510, 500))]
        .into_iter()
        .enumerate()
    {
        let near = |p: (i32, i32)| (p.0 - tile.0).abs().max((p.1 - tile.1).abs()) <= 2;
        data.creatures.retain(|c| !near(c.position));
        data.nests.retain(|n| !near(n.position));
        data.link_sites.retain(|&site| !near(site));
        let key = SettlementKey {
            rx: 7,
            ry: 1 + i as i32,
        };
        data.settlements.0.insert(
            key,
            feral_processes_engine::resources::KnownSettlement {
                tile,
                def: feral_processes_engine::settlements::SettlementDef {
                    id: "test_settlement".to_string(),
                    name: name.to_string(),
                    blurb: String::new(),
                    kind: feral_processes_engine::settlements::SettlementKind::Server,
                    specialty: feral_processes_engine::settlements::Specialty::Materials,
                    temperament: feral_processes_engine::settlements::Temperament::Open,
                },
                visited: true,
            },
        );
    }
    for tile in [(520, 500), (530, 500)] {
        data.outposts.push(save::OutpostSave {
            name: String::new(),
            tile,
            biome: feral_processes_engine::world::Biome::Deadlock,
            growth: 0,
            integrity: feral_processes_engine::tuning::OUTPOST_MAX_INTEGRITY,
            stock: Vec::new(),
            stale_ticks: 0,
            cycle_progress: 0,
        });
    }
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(feral_processes_engine::Game::load(&path, &root.join("assets")).unwrap());
    let _ = std::fs::remove_file(&path);
    app
}

fn selectable(rows: &[Row]) -> usize {
    rows.iter()
        .filter(|r| matches!(r, Row::Item { .. }))
        .count()
}

fn route(cargo: &str) -> RouteLine {
    RouteLine {
        site: RouteDestinationId::Outpost((1, 1)),
        site_name: LONG_NAME.to_string(),
        leg: "Outbound",
        cargo: cargo.to_string(),
        due: "held up for stock",
        standing: true,
        stalled: true,
    }
}

fn long_report() -> OutpostReport {
    let mut r = OutpostReport {
        tile: (0, 0),
        name: LONG_NAME.to_string(),
        biome: feral_processes_engine::world::Biome::Deadlock,
        tier: 2,
        tier_label: "Complex",
        growth_fill: 1.0,
        trend: feral_processes_engine::outposts::Trend::Stable,
        reason: "Stable: max tier reached, and nothing about it needs your hand right now"
            .to_string(),
        integrity: 100,
        max_integrity: 100,
        stock: 60,
        stock_cap: 60,
        route: None,
        crew: Vec::new(),
        yields: vec![feral_processes_engine::OutpostYieldRow {
            item: feral_processes_engine::items::ItemId("core_fragment".to_string()),
            name: LONG_NAME.to_string(),
            tier: 3,
        }],
        next_tier_requirement: Some(
            "Needs a crew of five and a bigger stockpile first".to_string(),
        ),
        dark: false,
    };
    r.crew = (0..5)
        .map(|_| OutpostCrewRow {
            name: LONG_NAME.to_string(),
            species: LONG_NAME.to_string(),
            level: 99,
        })
        .collect();
    r
}

#[test]
fn every_sites_tab_draws_as_many_selectable_rows_as_app_core_counts() {
    let mut app = sites_app();
    app.handle_key(GameKey::Char('w'));
    assert!(
        app.hub_at().is_some(),
        "w did not open the hub: {:?}",
        app.status_line
    );
    let mut seen = 0;
    for _ in 0..8 {
        if app.mode == Mode::Playing {
            break;
        }
        app.handle_key(GameKey::Right);
        let tabs = {
            let mut n = 0;
            for _ in 0..6 {
                let site = app.held_site();
                let expect = app.sites_row_count();
                let game = app.game.as_mut().unwrap();
                let rows = site_rows(game, app.mode, site, 0);
                assert_eq!(
                    selectable(&rows),
                    expect,
                    "{:?} at {site:?} draws a different number of rows than app-core counts",
                    app.mode
                );
                seen += 1;
                n += 1;
                app.handle_key(GameKey::Right);
            }
            n
        };
        assert!(tabs > 0);
        for _ in 0..8 {
            if app.hub_on_rail {
                break;
            }
            app.handle_key(GameKey::Left);
        }
        app.handle_key(GameKey::Down);
    }
    assert!(seen >= 10, "the walk reached only {seen} tabs");
}

#[test]
fn the_widest_sites_rows_fit_the_page_at_720() {
    let status = TownStatus {
        name: LONG_NAME.to_string(),
        kind: "Server",
        specialty: "Materials",
        temperament: "Mercantile",
        standing: "Hostile",
        hostile: true,
        meaning: vec![
            "They trade with you, but take only one-off routes.".to_string(),
            "They prey on caravans that pass near them.".to_string(),
            "Their patrols go after you and their raiders reach as far as your base.".to_string(),
        ],
        condition: Some("Vitality: Thriving, and growing faster than its neighbours".to_string()),
        gift: "Nobody spare for you yet; one frees up a long way off.".to_string(),
        travel: "They would hold a relay link open once you build a Relay.".to_string(),
    };
    assert_rows_fit(&town_status_rows(&status), "town status");
    let offers: Vec<SeenOffer> = (0..3)
        .map(|_| SeenOffer {
            name: LONG_NAME.to_string(),
            detail: String::new(),
            unit_cost: 99_999,
            qty: 9_999,
        })
        .collect();
    for goods in [
        TownGoods::Live {
            offers: offers.clone(),
        },
        TownGoods::Seen {
            offers,
            age: "a while ago",
        },
        TownGoods::NotSeen,
    ] {
        assert_rows_fit(&goods_rows(&goods, "Core Fragments", 0), "goods");
    }
    let five =
        route("12 Iron Ingot, 12 Copper Wire, 12 Salvage Coil, 12 Cache Grain, 12 Core Fragment");
    assert_rows_fit(&routes_rows(std::slice::from_ref(&five), 0), "routes");
    assert_rows_fit(
        &flight_rows(std::slice::from_ref(&five), 0),
        "routes in flight",
    );
    let log = SiteLogLine {
        age: "a while ago",
        text: format!("Sold {LONG_NAME} {LONG_NAME} and took payment in kind"),
    };
    assert_rows_fit(&log_rows(&[log], 0), "site log");
    let report = long_report();
    assert_rows_fit(&outpost_status_rows(&report), "outpost status");
    assert_rows_fit(&crew_rows(&report, 0), "outpost crew");
    let stock = vec![(LONG_NAME.to_string(), 60)];
    assert_rows_fit(&stock_rows(&report, &stock, &[five], 0), "outpost stock");
    let schedule = CaravanSchedule {
        docked: Some((LONG_NAME.to_string(), "a long way off")),
        next: Some((LONG_NAME.to_string(), "a long way off")),
    };
    assert_rows_fit(&caravan_rows(&schedule), "caravans");
}

#[test]
fn the_sites_rail_fits_with_long_names() {
    let mut app = sites_app();
    app.handle_key(GameKey::Char('w'));
    assert!(
        app.hub_at().is_some(),
        "w did not open the hub: {:?}",
        app.status_line
    );
    let mut fx = Fx::new();
    let (_, shapes) = crate::paint::with_painter(|p| draw(&mut app, &mut fx, p, false, false));
    let text = crate::paint::painted_text(&shapes);
    assert!(
        text.iter().any(|t| t.contains("Alder")),
        "the rail never drew a site"
    );
    assert!(
        text.iter().all(|t| !t.contains(LONG_NAME)),
        "an 80-character title must be elided on the rail"
    );
}

#[test]
fn no_sites_row_shows_a_tick() {
    let mut app = sites_app();
    app.handle_key(GameKey::Char('w'));
    assert!(
        app.hub_at().is_some(),
        "w did not open the hub: {:?}",
        app.status_line
    );
    let mut fx = Fx::new();
    let tick = regex_lite_ticks;
    for _ in 0..12 {
        let (_, shapes) = crate::paint::with_painter(|p| draw(&mut app, &mut fx, p, false, false));
        for line in crate::paint::painted_text(&shapes) {
            assert!(!tick(&line), "{:?} shows a tick figure: {line}", app.mode);
        }
        app.handle_key(GameKey::Right);
    }
}

/// `\d+ ticks?` without a regex dependency.
fn regex_lite_ticks(line: &str) -> bool {
    let words: Vec<&str> = line.split_whitespace().collect();
    words.windows(2).any(|w| {
        w[0].chars().all(|c| c.is_ascii_digit())
            && !w[0].is_empty()
            && w[1]
                .trim_end_matches(|c: char| !c.is_alphanumeric())
                .starts_with("tick")
    })
}

#[test]
fn an_outpost_status_never_draws_its_tick_route_line() {
    let mut report = long_report();
    report.route = Some("running toward Anchorpoint Station, 4800 ticks left".to_string());
    for row in outpost_status_rows(&report)
        .iter()
        .chain(&crew_rows(&report, 0))
        .chain(&stock_rows(&report, &[], &[], 0))
    {
        let text = row_label_text(row);
        assert!(
            !text.contains("ticks") && !text.contains("running toward"),
            "{text}"
        );
    }
}

#[test]
fn unavailable_actions_are_still_on_the_keybar() {
    let mut app = sites_app();
    app.handle_key(GameKey::Char('w'));
    assert!(
        app.hub_at().is_some(),
        "w did not open the hub: {:?}",
        app.status_line
    );
    // At base the Goods tab's trade is unavailable; the keybar says so only
    // when it is used, so the pair must be drawn regardless.
    let keys = feral_processes_app_core::sites_screen_keys(Mode::SiteGoods);
    assert!(keys.iter().any(|(_, verb)| *verb == "trade"));
    for _ in 0..2 {
        app.handle_key(GameKey::Right);
    }
    assert_eq!(app.mode, TOWN_TABS[1]);
    let mut fx = Fx::new();
    let (_, shapes) = crate::paint::with_painter(|p| draw(&mut app, &mut fx, p, false, false));
    let text = crate::paint::painted_text(&shapes).join("\n");
    assert!(
        text.contains("trade"),
        "the Goods keybar lost its trade key:\n{text}"
    );
}

#[test]
fn every_sites_tab_draws_a_refusal_once() {
    const REFUSAL: &str = "Requires Zone 3 first.";
    let mut app = sites_app();
    app.handle_key(GameKey::Char('w'));
    let mut fx = Fx::new();
    let mut modes = std::collections::BTreeSet::new();
    for _ in 0..8 {
        app.handle_key(GameKey::Right);
        for _ in 0..6 {
            app.status_line = Some(REFUSAL.to_string());
            let (_, shapes) =
                crate::paint::with_painter(|p| draw(&mut app, &mut fx, p, false, false));
            let drawn = crate::paint::painted_text(&shapes)
                .iter()
                .filter(|t| t.contains(REFUSAL))
                .count();
            assert_eq!(drawn, 1, "{:?} drew the refusal {drawn} times", app.mode);
            modes.insert(format!("{:?}", app.mode));
            app.handle_key(GameKey::Right);
        }
        for _ in 0..8 {
            if app.hub_on_rail {
                break;
            }
            app.handle_key(GameKey::Left);
        }
        app.handle_key(GameKey::Down);
    }
    for mode in [
        "SiteStatus",
        "SiteGoods",
        "SiteRoutes",
        "SiteJobs",
        "SiteLog",
        "OutpostStatus",
        "OutpostCrew",
        "OutpostStock",
    ] {
        assert!(modes.contains(mode), "never reached {mode}: {modes:?}");
    }
}

/// The longest name a player can type, with the glyph and the `!` the rail
/// adds, is elided to the rail rather than overflowing it.
#[test]
fn the_rail_elides_a_longest_outpost_name_and_no_coordinates_show() {
    let mut app = sites_app();
    let longest = "W".repeat(feral_processes_engine::tuning::OUTPOST_NAME_MAX);
    app.game
        .as_mut()
        .unwrap()
        .rename_outpost((520, 500), &longest)
        .unwrap();
    app.handle_key(GameKey::Char('w'));
    let mut fx = Fx::new();
    let (_, shapes) = crate::paint::with_painter(|p| draw(&mut app, &mut fx, p, false, false));
    let text = crate::paint::painted_text(&shapes);
    assert!(
        text.iter()
            .any(|t| t.starts_with("WWWW") && t.ends_with('…') && !t.contains("W…W")),
        "the longest name is cut at its end to the rail's width: {text:?}"
    );
    assert!(
        text.iter()
            .all(|t| !t.contains("(520, 500)") && !t.contains("(530, 500)")),
        "no outpost is named by its coordinates: {text:?}"
    );
}

#[test]
fn the_outpost_name_popup_fits() {
    let field = feral_processes_app_core::TextField::from(
        "W".repeat(feral_processes_engine::tuning::OUTPOST_NAME_MAX)
            .as_str(),
    );
    assert_rows_fit(
        &crate::render::outposts::outpost_name_rows(&field),
        "outpost name",
    );
}

/// The census tolerates a site page drawing nothing, because which pages
/// have a site depends on its map; with towns and outposts on the rail each
/// one must say the refusal exactly once, or a `None` handed to its popup
/// would pass unseen.
#[test]
fn every_site_page_draws_a_refusal_once() {
    let mut app = sites_app();
    let mut fx = Fx::new();
    app.handle_key(GameKey::Char('w'));
    let mut seen = std::collections::BTreeSet::new();
    // Several sites offer each tab, so `settle_hub` adopts none: walk the
    // rail with the keys instead, holding a real page.
    for _ in 0..8 {
        if app.mode == Mode::Playing {
            break;
        }
        app.handle_key(GameKey::Right);
        for _ in 0..6 {
            if !super::refusals::SITE_PAGES.contains(&app.mode) {
                break;
            }
            app.status_line = Some(super::refusals::CENSUS_REFUSAL.to_string());
            let (_, shapes) =
                crate::paint::with_painter(|p| draw(&mut app, &mut fx, p, false, false));
            let drawn = crate::paint::painted_text(&shapes)
                .iter()
                .filter(|t| t.contains(super::refusals::CENSUS_REFUSAL))
                .count();
            assert_eq!(drawn, 1, "{:?} painted the refusal {drawn} times", app.mode);
            seen.insert(format!("{:?}", app.mode));
            app.handle_key(GameKey::Right);
        }
        for _ in 0..8 {
            if app.hub_on_rail {
                break;
            }
            app.handle_key(GameKey::Left);
        }
        app.handle_key(GameKey::Down);
    }
    assert_eq!(
        seen.len(),
        super::refusals::SITE_PAGES.len(),
        "the walk reached only {seen:?}"
    );
}
