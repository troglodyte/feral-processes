//! A census of every hub as it stood before ADR 0043 made the rail runtime
//! data and the page held: what each rail lists, what its `!` marks mean,
//! how the nav walks it, and where a screen reached off the map sits. The
//! expected literals describe the shipped behaviour and do not change when
//! the framework underneath them does; only `rail` and `place` are rewritten.

use crate::tests::alerts::{alert, seeded};
use crate::tests::support::*;
use crate::*;
use feral_processes_engine::alerts::AlertKind;

type Place = (Mode, Option<(Hub, usize, usize)>, bool);

/// Title, screens and `!` per rail page, in order.
fn rail(app: &App, hub: Hub) -> Vec<(String, Vec<Mode>, bool)> {
    hub.kind()
        .pages(app)
        .into_iter()
        .map(|p| (p.title, p.screens.to_vec(), p.marked))
        .collect()
}

fn place(app: &App) -> Place {
    (app.mode, app.hub_at(), app.hub_on_rail)
}

fn open_from_main_menu(seed: u32) -> App {
    let mut app = test_app(seed);
    app.mode = Mode::MainMenu;
    app.handle_key(GameKey::Char('o'));
    app
}

/// Titles and first-screen lists, without the `!`.
fn plain(rail: Vec<(String, Vec<Mode>, bool)>) -> Vec<(String, Vec<Mode>)> {
    rail.into_iter().map(|(t, s, _)| (t, s)).collect()
}

fn expect(rows: &[(&str, &[Mode])]) -> Vec<(String, Vec<Mode>)> {
    rows.iter()
        .map(|(t, s)| (t.to_string(), s.to_vec()))
        .collect()
}

const BASE_RAIL: &[(&str, &[Mode])] = &[
    ("Overview", &[Mode::BaseOverview]),
    ("Build", &[Mode::BaseBuild]),
    ("Research", &[Mode::BaseResearch]),
    ("Trade", &[Mode::BaseTrade]),
    ("Compile", &[Mode::BaseCompile]),
    ("Recipes", &[Mode::Recipes]),
    (
        "Staff",
        &[
            Mode::StaffWellbeing,
            Mode::StaffCoverage,
            Mode::StaffDiscontent,
        ],
    ),
    (
        "Production",
        &[Mode::BaseOutput, Mode::BaseMachines, Mode::BasePower],
    ),
    ("Stock", &[Mode::BaseStock]),
    ("Work orders", &[Mode::WorkOrders]),
    ("Staffing", &[Mode::BaseStaff, Mode::Structures]),
    (
        "Build orders",
        &[Mode::BuildOrders, Mode::DigOrders, Mode::FloorOrders],
    ),
    ("Alerts", &[Mode::Alerts]),
];

const INVENTORY_RAIL: &[(&str, &[Mode])] = &[
    ("Items", &[Mode::Inventory]),
    ("Phase keys", &[Mode::PhaseKeys]),
    ("Downed programs", &[Mode::DownedPrograms]),
    ("Tools", &[Mode::Tools]),
    ("Companions", &[Mode::Companion]),
    ("Manifests", &[Mode::ManifestIntro]),
    ("Programs", &[Mode::PartyPrograms]),
    ("Routines", &[Mode::PartyRoutines]),
    ("Perks", &[Mode::Perks]),
];

const GAME_MENU_RAIL: &[(&str, &[Mode])] = &[
    ("Gameplay", &[Mode::GameplayOptions]),
    ("Audio", &[Mode::AudioOptions]),
    ("Graphics", &[Mode::GraphicsOptions]),
    ("Sprites", &[Mode::SpriteOptions]),
    ("Manual", &[Mode::ManualOptions]),
    ("Save & quit", &[Mode::QuitRunConfirm]),
];

#[test]
fn every_fixed_rail_reads_as_it_does_today() {
    // Base, no bay.
    let app = app_inside_a_small_base_with_programs(41001, false, 2);
    assert_eq!(plain(rail(&app, Hub::Base)), expect(BASE_RAIL));

    // Base with a bay: Breed last; with the structure hub open: none.
    let mut app = app_with_a_breeding_bay(41002, 1, false);
    let mut with_breed = expect(BASE_RAIL);
    with_breed.push(("Breed".to_string(), vec![Mode::Breed]));
    assert_eq!(plain(rail(&app, Hub::Base)), with_breed);
    app.handle_key(GameKey::Char('c'));
    assert!(app.structure_hub.is_some());
    assert_eq!(plain(rail(&app, Hub::Base)), expect(BASE_RAIL));

    // Inventory.
    let app = test_app(41003);
    assert_eq!(plain(rail(&app, Hub::Inventory)), expect(INVENTORY_RAIL));

    // Game menu: six in a run, five from the main menu.
    let mut in_run = test_app(41004);
    in_run.handle_key(GameKey::Esc);
    assert_eq!(plain(rail(&in_run, Hub::GameMenu)), expect(GAME_MENU_RAIL));
    let menu_only = open_from_main_menu(41005);
    assert_eq!(
        plain(rail(&menu_only, Hub::GameMenu)),
        expect(&GAME_MENU_RAIL[..5])
    );

    // Structure hub beside a Depot.
    let mut app = app_beside_depots(41006, 1, 0, &[]);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(
        plain(rail(&app, Hub::Structure)),
        expect(&[
            ("Inventory", &[Mode::Transfer]),
            ("Filter", &[Mode::DepotFilter]),
            ("Info", &[Mode::StructureManifest]),
        ])
    );
}

#[test]
fn base_report_marks_are_its_flags() {
    let apps = [
        app_inside_a_small_base_with_programs(41010, false, 2),
        seeded(
            41011,
            vec![alert(
                AlertKind::DepotsFull,
                "depots",
                "the depots are full",
            )],
        ),
        app_with_a_breeding_bay(41012, 1, false),
    ];
    let mut any_marked = false;
    for app in &apps {
        let flags = app.game.as_ref().unwrap().base_report_flags();
        let pages = rail(app, Hub::Base);
        for (i, (_, _, marked)) in pages.iter().enumerate() {
            if i < BASE_REPORT_PAGES.len() {
                assert_eq!(*marked, base_page_flag(&flags, i).is_some(), "page {i}");
                any_marked |= *marked;
            } else {
                assert!(!marked, "Breed is never marked");
            }
        }
    }
    assert!(any_marked, "the census needs at least one marked page");
}

fn walk(app: &mut App, script: &[(GameKey, Place)]) {
    for (i, (key, want)) in script.iter().enumerate() {
        app.handle_key(*key);
        assert_eq!(place(app), *want, "after key {i}: {key:?}");
    }
}

#[test]
fn the_nav_walks_every_hub_as_it_does_today() {
    use GameKey::*;
    let at = |m, h, p, s, r| (m, Some((h, p, s)), r);

    // The base report.
    let mut app = test_app(41020);
    walk(
        &mut app,
        &[
            (Char('b'), at(Mode::BaseOverview, Hub::Base, 0, 0, true)),
            (Down, at(Mode::BaseBuild, Hub::Base, 1, 0, true)),
            (Down, at(Mode::BaseResearch, Hub::Base, 2, 0, true)),
            (Right, at(Mode::BaseResearch, Hub::Base, 2, 0, false)),
            (Esc, at(Mode::BaseResearch, Hub::Base, 2, 0, true)),
            (Char(']'), at(Mode::BaseTrade, Hub::Base, 3, 0, true)),
            (Char('['), at(Mode::BaseResearch, Hub::Base, 2, 0, true)),
            (Esc, (Mode::Playing, None, true)),
        ],
    );

    // The Staff page's three screens.
    let mut app = test_app(41021);
    let mut keys = vec![Char('b')];
    keys.extend([Char(']'); 6]);
    for k in keys {
        app.handle_key(k);
    }
    walk(
        &mut app,
        &[
            (Right, at(Mode::StaffWellbeing, Hub::Base, 6, 0, false)),
            (Right, at(Mode::StaffCoverage, Hub::Base, 6, 1, false)),
            (Right, at(Mode::StaffDiscontent, Hub::Base, 6, 2, false)),
            (Left, at(Mode::StaffCoverage, Hub::Base, 6, 1, false)),
            (Left, at(Mode::StaffWellbeing, Hub::Base, 6, 0, false)),
            (Left, at(Mode::StaffWellbeing, Hub::Base, 6, 0, true)),
        ],
    );

    // The inventory.
    let mut app = test_app(41022);
    walk(
        &mut app,
        &[
            (Char('i'), at(Mode::Inventory, Hub::Inventory, 0, 0, true)),
            (Down, at(Mode::PhaseKeys, Hub::Inventory, 1, 0, true)),
            (Down, at(Mode::DownedPrograms, Hub::Inventory, 2, 0, true)),
            (Right, at(Mode::DownedPrograms, Hub::Inventory, 2, 0, false)),
            (Esc, at(Mode::DownedPrograms, Hub::Inventory, 2, 0, true)),
            (Char(']'), at(Mode::Tools, Hub::Inventory, 3, 0, true)),
            (
                Char('['),
                at(Mode::DownedPrograms, Hub::Inventory, 2, 0, true),
            ),
            (Esc, (Mode::Playing, None, true)),
        ],
    );

    // The game menu: Esc on the map, down to Manual, Enter opens the manual,
    // Esc comes back to the rail.
    let mut app = test_app(41023);
    walk(
        &mut app,
        &[
            (Esc, at(Mode::GameplayOptions, Hub::GameMenu, 0, 0, true)),
            (Down, at(Mode::AudioOptions, Hub::GameMenu, 1, 0, true)),
            (Down, at(Mode::GraphicsOptions, Hub::GameMenu, 2, 0, true)),
            (Down, at(Mode::SpriteOptions, Hub::GameMenu, 3, 0, true)),
            (Down, at(Mode::ManualOptions, Hub::GameMenu, 4, 0, true)),
        ],
    );
    app.handle_key(Enter);
    assert_ne!(app.mode, Mode::ManualOptions, "Enter opens the manual");
    assert_eq!(app.hub_at(), None, "the manual is outside the hub");
    app.handle_key(Esc);
    assert_eq!(
        place(&app),
        at(Mode::ManualOptions, Hub::GameMenu, 4, 0, true)
    );

    // The structure hub beside a Depot and a machine.
    let mut app = app_between_a_depot_and_a_machine(41024);
    walk(
        &mut app,
        &[
            (Char('c'), at(Mode::Transfer, Hub::Structure, 0, 0, true)),
            (Right, at(Mode::Transfer, Hub::Structure, 0, 0, false)),
        ],
    );
    let first = app.structure_hub.as_ref().unwrap().current().entity;
    app.handle_key(Tab);
    assert_ne!(app.structure_hub.as_ref().unwrap().current().entity, first);
    assert!(app.hub_at().is_some_and(|(h, ..)| h == Hub::Structure));
    app.handle_key(Esc);
    assert!(app.hub_on_rail);
    app.handle_key(Esc);
    assert_eq!(place(&app).0, Mode::Playing);
    assert!(app.structure_hub.is_none());
}

#[test]
fn a_hub_reopens_where_it_was_left() {
    use GameKey::*;
    let mut app = test_app(41030);
    app.handle_key(Char('b'));
    for _ in 0..6 {
        app.handle_key(Char(']'));
    }
    for _ in 0..3 {
        app.handle_key(Right);
    }
    assert_eq!(app.mode, Mode::StaffDiscontent);
    app.handle_key(Esc);
    app.handle_key(Esc);
    assert_eq!(app.mode, Mode::Playing);
    app.handle_key(Char('b'));
    assert_eq!(app.mode, Mode::StaffDiscontent);
    assert!(app.hub_on_rail);

    // The game menu does not remember.
    let mut app = test_app(41031);
    app.handle_key(Esc);
    app.handle_key(Down);
    app.handle_key(Esc);
    assert_eq!(app.mode, Mode::Playing);
    app.handle_key(Esc);
    assert_eq!(app.mode, Mode::GameplayOptions);

    // Nor does the structure hub.
    let mut app = app_between_a_depot_and_a_machine(41032);
    app.handle_key(Char('c'));
    app.handle_key(Down);
    assert_eq!(app.mode, Mode::MachineRecipe);
    app.handle_key(Esc);
    assert_eq!(app.mode, Mode::Playing);
    app.handle_key(Char('c'));
    assert_eq!(
        app.hub_at().map(|(h, p, _)| (h, p)),
        Some((Hub::Structure, 0))
    );
}

#[test]
fn a_screen_reached_off_the_map_is_in_its_hub() {
    let mut app = test_app(41040);
    app.handle_key(GameKey::Char('D'));
    assert_eq!(app.hub_at(), Some((Hub::Inventory, 2, 0)));
    assert!(!app.hub_on_rail);

    let mut app = test_app(41041);
    app.handle_key(GameKey::Char('N'));
    assert_eq!(app.hub_at(), Some((Hub::Base, 12, 0)));

    // Overview Enter on its "Stock" row.
    let mut app = test_app(41042);
    app.handle_key(GameKey::Char('b'));
    app.handle_key(GameKey::Right);
    let stock = BASE_REPORT_PAGES
        .iter()
        .position(|p| p.title == "Stock")
        .unwrap();
    for _ in 1..stock {
        app.handle_key(GameKey::Down);
    }
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::BaseStock);
    assert_eq!(app.hub_at().map(|(_, p, _)| p), Some(stock));
}

#[test]
fn a_popup_over_a_hub_screen_keeps_its_parent_frame() {
    let mut app = app_inside_a_small_base(41050, false);
    open_hub_screen(&mut app, Mode::WorkOrders);
    let last = app.work_order_rows().len() - 1;
    pick_row(&mut app, last);
    assert_eq!(app.mode, Mode::WorkOrderPick);
    assert_eq!(
        app.hub_for(base_popup_parent(app.mode).unwrap()),
        Some((Hub::Base, 9, 0))
    );
    assert_eq!(app.hub_at(), None);
}

#[test]
fn hub_for_answers_only_from_the_held_page() {
    let mut app = test_app(41060);
    app.handle_key(GameKey::Char('b'));
    assert_eq!(app.mode, Mode::BaseOverview);
    assert_eq!(app.hub_for(Mode::BaseStock), None);
    let stock = HubPageKey::Named("Stock");
    app.turn_hub_to(Hub::Base, &stock);
    assert_eq!(app.hub_at(), Some((Hub::Base, 8, 0)));
    assert_eq!(app.hub_page(), Some(&stock));
}

#[test]
fn a_held_key_that_left_the_rail_is_forgotten_on_reopen() {
    // The bay is gone: the remembered Breed page is not on the rail.
    let mut app = app_inside_a_small_base_with_programs(41061, false, 2);
    let breed = HubPageKey::Named("Breed");
    app.hub_last[Hub::Base.slot()] = Some((breed.clone(), Mode::Breed));
    app.handle_key(GameKey::Char('b'));
    assert_eq!(app.hub_at(), Some((Hub::Base, 0, 0)));

    // Held while it leaves: nothing panics, and the hub is simply not shown.
    app.hub_place = Some(app::hub::HubPlace {
        hub: Hub::Base,
        key: breed,
    });
    app.mode = Mode::Breed;
    assert_eq!(app.hub_at(), None);
    assert!(!app.handle_hub_nav(GameKey::Down));
}

#[test]
fn settle_adopts_only_an_unambiguous_page() {
    let mut app = test_app(41062);
    app.mode = Mode::DownedPrograms;
    app.settle_hub();
    assert_eq!(app.hub_at(), Some((Hub::Inventory, 2, 0)));

    let mut app = test_app(41063);
    app.mode = Mode::Transfer;
    app.settle_hub();
    assert_eq!(app.hub_at(), None);
}

#[test]
fn landing_on_the_map_clears_the_held_page() {
    let mut app = test_app(41064);
    app.handle_key(GameKey::Char('b'));
    assert!(app.hub_page().is_some());
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.hub_place.is_none());
    assert_eq!(app.hub_page(), None);
}

#[test]
fn enter_hub_at_holds_the_named_page() {
    let mut app = test_app(41065);
    let key = HubPageKey::Named("Staff");
    app.enter_hub_at(Hub::Base, key.clone(), Mode::StaffCoverage);
    assert_eq!(app.hub_page(), Some(&key));
    assert_eq!(app.hub_at(), Some((Hub::Base, 6, 1)));
    app.settle_hub();
    assert_eq!(app.hub_page(), Some(&key), "settle keeps the held page");
}

/// Every screen offered by more than one page, across every hub. A screen
/// enters this list only once everything that jumps onto it goes through
/// `App::enter_hub_at` or a turn.
const SHARED_SCREENS: &[Mode] = &[];

pub(super) fn shared_screens(app: &App) -> Vec<Mode> {
    let mut counts: Vec<(Mode, usize)> = Vec::new();
    for hub in Hub::ALL {
        for page in hub.kind().pages(app) {
            for &mode in page.screens {
                match counts.iter_mut().find(|(m, _)| *m == mode) {
                    Some((_, n)) => *n += 1,
                    None => counts.push((mode, 1)),
                }
            }
        }
    }
    counts
        .into_iter()
        .filter(|(_, n)| *n > 1)
        .map(|(m, _)| m)
        .collect()
}

#[test]
fn shared_screens_are_pinned() {
    let mut app = app_with_a_breeding_bay(41066, 1, false);
    let message = "a screen is now shared by several pages: enter it with App::enter_hub_at";
    assert_eq!(shared_screens(&app), SHARED_SCREENS, "{message}");
    app.handle_key(GameKey::Char('c'));
    assert!(app.structure_hub.is_some());
    assert_eq!(shared_screens(&app), SHARED_SCREENS, "{message}");
}

/// With two towns and two outposts on the rail, every tab of a site page is
/// shared by its twin. Caravans is one page, so its tabs never are.
const SHARED_WITH_SITES: &[Mode] = &[
    Mode::SiteStatus,
    Mode::SiteGoods,
    Mode::SiteRoutes,
    Mode::SiteJobs,
    Mode::SiteLog,
    Mode::OutpostStatus,
    Mode::OutpostCrew,
    Mode::OutpostStock,
];

#[test]
fn shared_screens_with_sites_are_pinned() {
    let app = crate::tests::sites_hub::app_with_far_sites(41067);
    let mut shared = shared_screens(&app);
    shared.sort_by_key(|m| format!("{m:?}"));
    let mut pinned = SHARED_WITH_SITES.to_vec();
    pinned.sort_by_key(|m| format!("{m:?}"));
    assert_eq!(shared, pinned);
}
