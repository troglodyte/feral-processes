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
#[ignore = "phase 2.3/2.4"]
fn the_b_rail_has_its_five_sections_in_order() {
    let app = app_at_a_contract_broker(46003, false);
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
        tabs_of("Alder"),
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
#[ignore = "phase 2.3"]
fn contracts_is_on_the_rail_only_with_a_broker_or_a_held_contract() {
    let app = test_app(46005);
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
#[ignore = "phase 2.3"]
fn the_contracts_launcher_verb_is_a_tab_of_b() {
    let mut app = app_at_a_contract_broker(46011, false);
    assert_eq!(reach_in_b(&mut app, "Contracts", "Board"), Mode::Contracts);
}

#[test]
#[ignore = "phase 2.4"]
fn the_caravan_and_dispatch_launcher_verbs_are_in_traffic() {
    let mut app = test_app(46012);
    assert_eq!(reach_in_b(&mut app, "Caravans", "Trader"), Mode::Caravan);
    let mut app = test_app(46013);
    assert_eq!(reach_in_b(&mut app, "Sorties", "Sorties"), Mode::Dispatch);
}

/// Work, Upgrade and Demolish are keys on the structure's Info tab in `i`
/// (decision 5: `W`, `U`, `X` if the key-hint census leaves them free), and
/// Hold is a Holding Cell's Hold tab. Phase 3 may move a target screen; it
/// may not drop a verb.
#[test]
#[ignore = "phase 3"]
fn the_structure_launcher_verbs_are_keys_on_the_structure_in_i() {
    for (key, target) in [
        ('W', Mode::WorkStructure),
        ('U', Mode::UpgradeDirection),
        ('X', Mode::RemoveConfirm),
    ] {
        let mut app = app_between_a_depot_and_a_machine(46014);
        app.handle_key(GameKey::Char('i'));
        let pages = Hub::Inventory.kind().pages(&app);
        let beside = pages
            .iter()
            .position(|p| p.section == Some("BESIDE YOU"))
            .expect("a BESIDE YOU entry");
        while app.hub_at().map(|(_, p, _)| p) != Some(beside) {
            app.handle_key(GameKey::Char(']'));
        }
        while app.mode != Mode::StructureManifest {
            app.handle_key(GameKey::Tab);
        }
        app.handle_key(GameKey::Right);
        app.handle_key(GameKey::Char(key));
        assert_eq!(app.mode, target, "{key} on Info");
    }

    let mut app = app_beside_a_holding_cell(46015, Vec::new(), 0);
    app.handle_key(GameKey::Char('i'));
    for _ in 0..64 {
        if app.mode == Mode::PinPrisoner {
            break;
        }
        app.handle_key(GameKey::Tab);
    }
    assert_eq!(app.mode, Mode::PinPrisoner, "the cell's Hold tab");
}
