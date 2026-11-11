//! The `b` rail as the hub-merge spec's "The two hubs" table has it, and a
//! census of every verb the old launcher pages (Build, Research, Trade,
//! Compile) opened, each reached from its new home. The launcher tables are
//! deleted, so the targets are spelled out here instead.

use crate::tests::support::*;
use crate::*;

type Tab = (&'static str, Mode);
type Entry = (Option<&'static str>, &'static str, &'static [Tab]);

/// Section, title and tabs per rail page, in order.
fn b_rail(app: &App) -> Vec<(Option<&'static str>, String, Vec<Tab>)> {
    Hub::Base
        .kind()
        .pages(app)
        .into_iter()
        .map(|p| {
            let tabs = p.tabs.iter().map(|t| (t.name, t.mode)).collect();
            (p.section, p.title, tabs)
        })
        .collect()
}

fn expect(entries: &[Entry]) -> Vec<(Option<&'static str>, String, Vec<Tab>)> {
    entries
        .iter()
        .map(|(s, t, tabs)| (*s, t.to_string(), tabs.to_vec()))
        .collect()
}

/// The BASE section up to Research; Breed (with a bay) and Alerts follow.
const BASE_SECTION: &[Entry] = &[
    (
        Some("BASE"),
        "Overview",
        &[("Overview", Mode::BaseOverview)],
    ),
    (
        None,
        "Build",
        &[
            ("Deploy", Mode::Build),
            ("Build orders", Mode::BuildOrders),
            ("Dig", Mode::DigOrders),
            ("Floor", Mode::FloorOrders),
        ],
    ),
    (
        None,
        "Production",
        &[
            ("Output", Mode::BaseOutput),
            ("Machines", Mode::BaseMachines),
            ("Power", Mode::BasePower),
            ("Recipes", Mode::Recipes),
            ("Compile", Mode::Craft),
        ],
    ),
    (
        None,
        "Stock",
        &[
            ("Stock", Mode::BaseStock),
            ("Work orders", Mode::WorkOrders),
        ],
    ),
    (
        None,
        "Staff",
        &[
            ("Wellbeing", Mode::StaffWellbeing),
            ("Coverage", Mode::StaffCoverage),
            ("Discontent", Mode::StaffDiscontent),
            ("Duties", Mode::BaseStaff),
            ("Staffing", Mode::Structures),
        ],
    ),
    (
        None,
        "Research",
        &[
            ("Projects", Mode::Research),
            ("Routines", Mode::RoutineResearch),
            ("Affixes", Mode::AffixResearch),
            ("Weapons", Mode::WeaponResearch),
            ("Study", Mode::PinSubject),
        ],
    ),
];
const BREED: Entry = (None, "Breed", &[("Breed", Mode::Breed)]);
const ALERTS: Entry = (None, "Alerts", &[("Alerts", Mode::Alerts)]);

#[test]
fn the_base_section_reads_as_the_spec() {
    let app = app_inside_a_small_base_with_programs(46001, false, 2);
    let mut want = BASE_SECTION.to_vec();
    want.push(ALERTS);
    let rail = b_rail(&app);
    assert_eq!(rail[..want.len()], expect(&want)[..]);

    // Breed sits between Research and Alerts, and only once a bay stands.
    let app = app_with_a_breeding_bay(46002, 1, false);
    let mut want = BASE_SECTION.to_vec();
    want.extend([BREED, ALERTS]);
    let rail = b_rail(&app);
    assert_eq!(rail[..want.len()], expect(&want)[..]);
}

#[test]
fn the_b_rail_has_its_five_sections_in_order() {
    let mut app = app_at_a_contract_broker(46003, false);
    crate::tests::sites_hub::set_sites(&mut app, &[], &[]);
    let sections: Vec<_> = b_rail(&app).into_iter().filter_map(|e| e.0).collect();
    assert_eq!(sections, ["BASE", "JOBS", "TRAFFIC"], "no site is known");

    let app = crate::tests::sites_hub::app_with_far_sites(46004);
    let rail = b_rail(&app);
    let sections: Vec<_> = rail.iter().filter_map(|e| e.0).collect();
    assert!(
        sections.ends_with(&["TOWNS", "OUTPOSTS", "TRAFFIC"]),
        "{sections:?}"
    );
    let tabs_of = |title: &str| {
        rail.iter()
            .find(|e| e.1 == title)
            .map(|e| e.2.iter().map(|t| t.0).collect::<Vec<_>>())
            .unwrap_or_else(|| panic!("{title} is not on the rail: {rail:?}"))
    };
    assert_eq!(
        tabs_of("Alder south-east"),
        ["Status", "Goods", "Routes", "Jobs", "Log"]
    );
    assert_eq!(tabs_of("Caravans"), ["Schedule", "In flight", "Trader"]);
    assert_eq!(tabs_of("Sorties"), ["Sorties"]);
    let outposts = rail
        .iter()
        .find(|e| e.0 == Some("OUTPOSTS"))
        .expect("an outpost entry");
    let names: Vec<_> = outposts.2.iter().map(|t| t.0).collect();
    assert_eq!(names, ["Status", "Crew", "Stock", "Log"]);
}

#[test]
fn contracts_is_on_the_rail_only_with_a_broker_or_a_held_contract() {
    let mut app = test_app(46005);
    skip_tutorial(&mut app);
    let game = app.game.as_ref().unwrap();
    assert!(game.active_contracts().is_empty(), "precondition");
    assert!(!b_rail(&app).iter().any(|e| e.1 == "Contracts"));

    let app = app_at_a_contract_broker(46006, false);
    let contracts = b_rail(&app)
        .into_iter()
        .find(|e| e.1 == "Contracts")
        .expect("a Broker puts Contracts on the rail");
    assert_eq!(contracts.0, Some("JOBS"));
    let names: Vec<_> = contracts.2.iter().map(|t| t.0).collect();
    assert_eq!(names, ["Held", "Board"]);
}

/// Opens `b`, walks `]` to the page titled `page` and `Tab` to its tab
/// `tab`, the way a player does, and answers the mode shown there.
fn reach_in_b(app: &mut App, page: &str, tab: &str) -> Mode {
    app.handle_key(GameKey::Char('b'));
    let pages = Hub::Base.kind().pages(app);
    for _ in 0..pages.len() {
        if app.hub_at().is_some_and(|(_, p, _)| pages[p].title == page) {
            break;
        }
        app.handle_key(GameKey::Char(']'));
    }
    let (_, p, _) = app.hub_at().expect("b holds a page");
    assert_eq!(pages[p].title, page, "{page} is on the rail");
    for _ in 0..pages[p].tabs.len() {
        let (_, _, t) = app.hub_at().expect("still in b");
        if pages[p].tabs[t].name == tab {
            break;
        }
        app.handle_key(GameKey::Tab);
    }
    let (hub, _, t) = app.hub_at().expect("the tab is in b");
    assert_eq!((hub, pages[p].tabs[t].name), (Hub::Base, tab));
    app.mode
}

/// What each deleted launcher row opened, and the page and tab of `b` that
/// shows it now.
const B_VERBS: &[(&str, Mode, &str, &str)] = &[
    ("Deploy a structure", Mode::Build, "Build", "Deploy"),
    ("Research", Mode::Research, "Research", "Projects"),
    (
        "Routine research",
        Mode::RoutineResearch,
        "Research",
        "Routines",
    ),
    ("Affix research", Mode::AffixResearch, "Research", "Affixes"),
    (
        "Weapon research",
        Mode::WeaponResearch,
        "Research",
        "Weapons",
    ),
    ("Study a program", Mode::PinSubject, "Research", "Study"),
    ("Compile an item", Mode::Craft, "Production", "Compile"),
];

#[test]
fn every_base_launcher_verb_is_a_tab_of_b() {
    for &(label, target, page, tab) in B_VERBS {
        let mut app = test_app(46010);
        stand_in_base(&mut app);
        assert_eq!(reach_in_b(&mut app, page, tab), target, "{label}");
    }
}

#[test]
fn the_contracts_launcher_verb_is_a_tab_of_b() {
    let mut app = app_at_a_contract_broker(46011, false);
    assert_eq!(reach_in_b(&mut app, "Contracts", "Board"), Mode::Contracts);
}

#[test]
fn the_caravan_and_dispatch_launcher_verbs_are_in_traffic() {
    let mut app = test_app(46012);
    assert_eq!(reach_in_b(&mut app, "Caravans", "Trader"), Mode::Caravan);
    let mut app = test_app(46013);
    assert_eq!(reach_in_b(&mut app, "Sorties", "Sorties"), Mode::Dispatch);
}

/// Work, Upgrade and Demolish are keys on the structure's Info tab in `i`
/// (decision 5: `W`, `U`, `X`, free on that tab and in the key-hint
/// census), each aimed at the entry's own structure through the door the old
/// launcher row used.
#[test]
fn the_structure_launcher_verbs_act_on_the_entry_they_are_pressed_on() {
    // The fixture's Extractor east of the party, Home west.
    let mut app = app_inside_a_small_base_with_programs(46014, false, 2);
    on_the_info_tab_of(&mut app, "Extractor");
    let keys = Hub::Inventory.kind().extra_keys(&app);
    for key in ["W", "U", "X"] {
        assert!(keys.iter().any(|(k, _)| *k == key), "{key} in {keys:?}");
    }
    app.handle_key(GameKey::Char('W'));
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.status_line, None, "the work order was taken");
    let log = app.game.as_ref().unwrap().message_log(32);
    assert!(
        log.iter().any(|l| l.text.contains("You set to work")),
        "W started work at the Extractor"
    );

    let mut app = app_inside_a_small_base_with_programs(46017, false, 2);
    let node = on_the_info_tab_of(&mut app, "Extractor");
    app.handle_key(GameKey::Char('U'));
    assert_eq!(app.mode, Mode::BuildProgram, "the program picker is up");
    match app.pending_build {
        Some(PendingBuild::Upgrade { structure, .. }) => assert_eq!(structure, node),
        _ => panic!("U leaves an upgrade of the Extractor pending"),
    }

    let mut app = app_inside_a_small_base_with_programs(46018, false, 2);
    let node = on_the_info_tab_of(&mut app, "Extractor");
    let before = app.game.as_ref().unwrap().adjacent_structures().len();
    app.handle_key(GameKey::Char('X'));
    assert!(!is_beside(&app, node), "X demolished the Extractor");
    let after = app.game.as_ref().unwrap().adjacent_structures().len();
    assert_eq!(after, before - 1, "and only it");
}

/// Info-only structures get a BESIDE YOU entry because W or U does
/// something to them (X comes along); the keys are pressed through the app, never the
/// `Game` door.
#[test]
fn the_verbs_reach_the_info_only_structures() {
    use crate::tests::reach_circles::{app_beside_a_patch_node, app_beside_a_shield};

    for (kind, mut app) in [
        ("Shield", app_beside_a_shield(46030)),
        ("Patch Node", app_beside_a_patch_node(46031)),
    ] {
        on_the_info_tab_of(&mut app, kind);
        app.handle_key(GameKey::Char('U'));
        assert_ne!(app.mode, Mode::BuildProgram, "{kind} needs no program");
        assert!(
            app.game
                .as_mut()
                .unwrap()
                .adjacent_build_site(1, 0)
                .is_some(),
            "U filed an upgrade of the {kind}"
        );
    }

    let mut app = app_beside_a_shield(46032);
    let shield = on_the_info_tab_of(&mut app, "Shield");
    app.handle_key(GameKey::Char('X'));
    assert!(!is_beside(&app, shield), "X demolished the Shield");

    let mut app = app_owning_a_program_and_a_research_station(46033);
    stand_beside_the_extractor(&mut app);
    on_the_info_tab_of(&mut app, "Research Station");
    app.handle_key(GameKey::Char('W'));
    assert_eq!(app.status_line, None, "the work order was taken");
    let log = app.game.as_ref().unwrap().message_log(32);
    assert!(
        log.iter().any(|l| l.text.contains("You set to work")),
        "W started work at the Research Station"
    );
}

/// Each door answers a refusal in its own words, aimed at the entry's own
/// structure: the Breeding Bay is no node, has no upgrade path, and will not
/// come down with a program incubating in it.
#[test]
fn the_structure_launcher_doors_refuse_in_their_own_sentences() {
    let bay_info = |seed| {
        let mut app = app_with_a_breeding_bay(seed, 1, false);
        let bay = on_the_info_tab_of(&mut app, "Breeding Bay");
        (app, bay)
    };
    let (mut app, _) = bay_info(46019);
    app.handle_key(GameKey::Char('W'));
    assert_eq!(
        app.status_line.as_deref(),
        Some("That structure can't be worked.")
    );

    let (mut app, _) = bay_info(46021);
    app.handle_key(GameKey::Char('U'));
    assert_eq!(
        app.status_line.as_deref(),
        Some("Breeding Bay can't be upgraded.")
    );

    let (mut app, _) = bay_info(46022);
    while app.mode != Mode::Breed {
        app.handle_key(GameKey::Tab);
    }
    pick_row(&mut app, 0);
    pick_row(&mut app, 0);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::StructureManifest, "back on Info");
    // The child is only in the bay once its parents' visit is over.
    let path = scratch_path("launcher_x_refusal", 46022);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = feral_processes_engine::save::load_from_file(&path).unwrap();
    for slot in data
        .structures
        .iter_mut()
        .filter(|s| s.kind == "breeding_bay")
        .flat_map(|s| s.incubating.iter_mut())
        .flatten()
    {
        slot.visit = None;
        slot.due = u64::MAX / 2;
    }
    feral_processes_engine::save::save_to_file(&path, &data).unwrap();
    app.game = Game::load(&path, &test_assets_dir()).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    let bay = on_the_info_tab_of(&mut app, "Breeding Bay");
    app.handle_key(GameKey::Char('X'));
    assert!(
        is_beside(&app, bay),
        "the bay stands: {:?}",
        app.status_line
    );
    assert_eq!(
        app.status_line.as_deref(),
        Some("A program is incubating — wait for it to hatch.")
    );
}

/// Hold is a Holding Cell's Hold tab.
#[test]
fn hold_is_a_holding_cells_tab() {
    let mut app = app_beside_a_holding_cell(46015, Vec::new(), 0);
    app.handle_key(GameKey::Char('c'));
    for _ in 0..64 {
        if app.mode == Mode::PinPrisoner {
            break;
        }
        app.handle_key(GameKey::Tab);
    }
    assert_eq!(app.mode, Mode::PinPrisoner, "the cell's Hold tab");
}

// ----- a held page that leaves the rail while it is open -----

fn held(app: &App) -> Option<HubPageKey> {
    app.hub_page().cloned()
}

/// Beside Birch with the onboarding chain done and one of Birch's jobs in
/// hand, and no Broker: that job alone keeps Contracts on the rail.
fn app_holding_one_town_job(seed: u32) -> (App, feral_processes_engine::contracts::ContractId) {
    let mut app = crate::tests::sites_hub::app_beside_birch(seed);
    skip_tutorial(&mut app);
    let town = crate::tests::sites_hub::TOWN_B;
    let game = app.game.as_mut().unwrap();
    let rows = game.settlement_board(town).expect("beside the counter");
    let id = rows.iter().find(|r| !r.tutorial).expect("a job").id.clone();
    assert!(game.accept_contract(&id, Some(town)).is_ok());
    assert_eq!(game.broker_reach(), BrokerReach::NoBroker, "precondition");
    assert_eq!(game.active_contracts().len(), 1, "precondition");
    (app, id)
}

/// The last contract leaves hand while Contracts → Held is open: JOBS has
/// no other entry, so the hold falls to the hub's first page, and the
/// cursor stays in the page.
#[test]
fn a_contract_leaving_hand_while_held_is_open_moves_to_the_first_page() {
    let (mut app, id) = app_holding_one_town_job(46020);
    assert_eq!(
        reach_in_b(&mut app, "Contracts", "Held"),
        Mode::ContractsHeld
    );
    app.handle_key(GameKey::Right);
    // Settled elsewhere: the engine drops it between two keys.
    app.game.as_mut().unwrap().abandon_contract(&id);
    app.handle_key(GameKey::Down);
    assert_eq!(held(&app), Some(HubPageKey::Named("Overview")));
    assert_eq!(app.mode, Mode::BaseOverview);
    assert!(!app.hub_on_rail, "focus is unchanged");
}

/// The same through the tab's own `[A]`.
#[test]
fn giving_back_the_last_contract_on_held_moves_to_the_first_page() {
    let (mut app, _) = app_holding_one_town_job(46021);
    reach_in_b(&mut app, "Contracts", "Held");
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Char('A'));
    assert!(app.game.as_ref().unwrap().active_contracts().is_empty());
    app.handle_key(GameKey::Down);
    assert_eq!(held(&app), Some(HubPageKey::Named("Overview")));
    assert!(!app.hub_on_rail);
}

/// An outpost lost while its Crew tab is open: the next outpost in the
/// section takes the hold, on the same tab, with the cursor where it was;
/// the last outpost lost falls back to the one before it. Three outposts,
/// so neither is the first page that happens to offer the tab.
#[test]
fn an_outpost_lost_while_open_moves_to_the_next_then_the_previous() {
    use crate::tests::sites_hub::{FAR_TILE, TOWN_B, set_sites};
    let tiles = [(520, 500), (530, 500), (540, 500)];
    let outpost = |tile| HubPageKey::Site(RouteDestinationId::Outpost(tile));
    let with = |app: &mut App, kept: &[(i32, i32)]| {
        let posts: Vec<_> = kept.iter().map(|&t| (t, Vec::new())).collect();
        set_sites(app, &[(TOWN_B, "Alder", FAR_TILE)], &posts);
    };
    let names = |app: &App| -> Vec<HubPageKey> {
        Hub::Base
            .kind()
            .pages(app)
            .into_iter()
            .map(|p| p.key)
            .filter(|k| matches!(k, HubPageKey::Site(RouteDestinationId::Outpost(_))))
            .collect()
    };

    let mut app = crate::tests::sites_hub::app_with_far_sites(46022);
    with(&mut app, &tiles);
    let order = names(&app);
    assert_eq!(order.len(), 3, "precondition");
    app.handle_key(GameKey::Char('b'));
    app.enter_hub_at(Hub::Base, order[1], Mode::OutpostCrew);
    app.hub_on_rail = false;
    app.handle_key(GameKey::Up);
    let kept: Vec<_> = tiles
        .iter()
        .copied()
        .filter(|&t| outpost(t) != order[1])
        .collect();
    with(&mut app, &kept);
    app.handle_key(GameKey::Down);
    assert_eq!(held(&app), Some(order[2]), "the next in OUTPOSTS");
    assert_eq!(app.mode, Mode::OutpostCrew, "the tab is kept");
    assert!(!app.hub_on_rail);

    let mut app = crate::tests::sites_hub::app_with_far_sites(46023);
    with(&mut app, &tiles);
    app.handle_key(GameKey::Char('b'));
    app.enter_hub_at(Hub::Base, order[2], Mode::OutpostStatus);
    app.handle_key(GameKey::Char('x'));
    assert!(app.hub_on_rail);
    let kept: Vec<_> = tiles
        .iter()
        .copied()
        .filter(|&t| outpost(t) != order[2])
        .collect();
    with(&mut app, &kept);
    app.handle_key(GameKey::Char('x'));
    assert_eq!(held(&app), Some(order[1]), "the previous in OUTPOSTS");
    assert_eq!(app.mode, Mode::OutpostStatus);
    assert!(app.hub_on_rail, "focus is unchanged");
}

/// A 2x2 structure's Info tab acts on the held entry's entity: the
/// direction lookup finds only the anchor tile, and the party stands at
/// `(3, -1)`, whose south neighbour is the station's `(3, 0)`, not its
/// anchor `(2, 0)`.
#[test]
fn u_and_x_on_a_wide_structures_info_tab_reach_it_from_a_non_anchor_side() {
    let station_info = |seed| {
        let mut app = app_owning_a_program_and_a_research_station(seed);
        stand_in_base_at(&mut app, 3, -1);
        let station = on_the_info_tab_of(&mut app, "Research Station");
        (app, station)
    };
    let (mut app, _) = station_info(46040);
    app.handle_key(GameKey::Char('U'));
    assert_eq!(app.mode, Mode::BuildProgram, "U found the station");

    let (mut app, station) = station_info(46041);
    app.handle_key(GameKey::Char('X'));
    assert!(!is_beside(&app, station), "X demolished the station");
}
