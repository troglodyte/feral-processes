//! The party's pages: Companions, Programs, Routines and Perks sit on the
//! inventory rail after Tools, and every screen that hung off the old `p`
//! menu still has its door. Driven through `handle_key` throughout, so a lost door fails here.

use feral_processes_engine::save;

use super::support::*;
use crate::*;

const RAIL: [Mode; 4] = [
    Mode::Companion,
    Mode::PartyPrograms,
    Mode::PartyRoutines,
    Mode::Perks,
];

fn press(app: &mut App, keys: &[GameKey]) {
    for key in keys {
        app.handle_key(*key);
    }
}

/// `i`, then `]` along the rail to Companions, the cursor still on the rail.
fn open_companions(app: &mut App) {
    app.handle_key(GameKey::Char('i'));
    for _ in 0..INVENTORY_PAGES.len() {
        if app.mode == Mode::Companion {
            break;
        }
        app.handle_key(GameKey::Char(']'));
    }
    assert_eq!(app.mode, Mode::Companion);
    assert!(app.hub_on_rail);
}

fn assert_in_page(app: &App, mode: Mode) {
    assert_eq!(app.mode, mode);
    assert!(
        !app.hub_on_rail,
        "the cursor is in the page, not on the rail"
    );
}

/// A run holding `perk_points`, through the save, the only door app-core has
/// onto them.
fn app_with_perk_points(seed: u32, perk_points: u32) -> App {
    app_with_points(seed, 0, perk_points)
}

fn app_with_points(seed: u32, stat_points: u32, perk_points: u32) -> App {
    let mut app = test_app(seed);
    let path = scratch_path("party_hub_points", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.player.stat_points = stat_points;
    data.player.perk_points = perk_points;
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
    let _ = std::fs::remove_file(&path);
    app
}

#[test]
fn the_party_pages_follow_tools_on_the_inventory_rail() {
    for (page, mode) in RAIL.into_iter().enumerate() {
        assert_eq!(hub_of(mode), Some((Hub::Inventory, 4 + page, 0)));
    }
}

#[test]
fn p_opens_nothing_on_the_map() {
    let mut app = test_app(31001);
    app.handle_key(GameKey::Char('p'));
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn right_and_enter_go_into_the_page_and_esc_climbs_back_out() {
    for key in [GameKey::Right, GameKey::Enter] {
        let mut app = test_app(31003);
        open_companions(&mut app);
        app.handle_key(key);
        assert_in_page(&app, Mode::Companion);
        app.handle_key(GameKey::Esc);
        assert_eq!(app.mode, Mode::Companion);
        assert!(app.hub_on_rail, "Esc in a page goes to the rail");
        app.handle_key(GameKey::Esc);
        assert_eq!(app.mode, Mode::Playing, "Esc on the rail leaves the hub");
    }
}

#[test]
fn i_reopens_on_a_party_page_and_the_base_keeps_its_own_slot() {
    let mut app = test_app(31004);
    open_companions(&mut app);
    press(&mut app, &[GameKey::Down, GameKey::Down]);
    assert_eq!(app.mode, Mode::PartyRoutines);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);

    press(&mut app, &[GameKey::Char('b'), GameKey::Char(']')]);
    assert_eq!(app.mode, Mode::BaseBuild);
    press(&mut app, &[GameKey::Esc]);

    app.handle_key(GameKey::Char('i'));
    assert_eq!(app.mode, Mode::PartyRoutines, "i kept its own slot");
    assert!(app.hub_on_rail);
    press(&mut app, &[GameKey::Esc]);
    app.handle_key(GameKey::Char('b'));
    assert_eq!(app.mode, Mode::BaseBuild);
}

#[test]
fn a_command_page_with_every_row_hidden_still_turns_and_escapes() {
    let mut app = app_owning_a_program_and_an_extractor(31005, &[]);
    assert!(
        !party_rows(&mut app)
            .iter()
            .any(|r| r.label == "Fuse two programs"),
        "premise: one companion and no fusion research hides Fuse"
    );
    open_companions(&mut app);
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::PartyPrograms);
    // Rows are selected by key, not by walking onto a hidden one.
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Char('z'));
    assert_eq!(app.mode, Mode::PartyPrograms, "no row answers to z");
    press(&mut app, &[GameKey::Esc, GameKey::Char(']')]);
    assert_eq!(app.mode, Mode::PartyRoutines);
    press(&mut app, &[GameKey::Esc]);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn the_command_pages_draw_their_rows_from_command_rows_alone() {
    let mut app = app_owning_distant_programs(31006, 2);
    mark_researched(&mut app, "program_refactoring");
    let programs: Vec<_> = app
        .command_rows(Mode::PartyPrograms)
        .unwrap()
        .iter()
        .map(|r| r.label)
        .collect();
    assert_eq!(
        programs,
        [
            "Read a manifest",
            "Fuse two programs",
            "Refactor a program",
            "Develop a program"
        ]
        .into_iter()
        .filter(|l| programs.contains(l))
        .collect::<Vec<_>>(),
        "Programs holds only its own rows, in table order"
    );
    assert!(programs.contains(&"Read a manifest"));
    assert!(programs.contains(&"Fuse two programs"));
    let routines = app.command_rows(Mode::PartyRoutines).unwrap();
    assert!(routines.iter().all(|r| matches!(
        r.label,
        "Install a routine" | "Etch a routine disk" | "Extract a routine"
    )));
}

#[test]
fn selecting_a_row_parks_the_page_as_the_origin() {
    let mut app = app_owning_distant_programs(31007, 1);
    open_via_menu(&mut app, 'p', "Read a manifest");
    assert_eq!(app.menu_origin, Some(Mode::PartyPrograms));
    let mut app = app_owning_a_program_and_an_extractor(31008, &[]);
    open_via_menu(&mut app, 'p', "Install a routine");
    assert_eq!(app.menu_origin, Some(Mode::PartyRoutines));
}

// Reachability: one test per row of the spec's table.

#[test]
fn the_roster_is_the_first_page() {
    let mut app = app_owning_distant_programs(31010, 1);
    open_companions(&mut app);
}

#[test]
fn the_menu_manifest_esc_lands_on_programs_in_page() {
    let mut app = test_app(31011);
    open_via_menu(&mut app, 'p', "Read a manifest");
    assert_eq!(app.mode, Mode::Manifest);
    app.handle_key(GameKey::Esc);
    assert_in_page(&app, Mode::PartyPrograms);
}

#[test]
fn the_roster_manifest_esc_lands_on_the_same_row_in_page() {
    let mut app = app_owning_distant_programs(31012, 2);
    open_companions(&mut app);
    press(&mut app, &[GameKey::Right, GameKey::Down, GameKey::Enter]);
    assert_in_page(&app, Mode::Companion);
    assert_eq!(app.menu_selected, 1);
    app.handle_key(GameKey::Char('M'));
    assert_eq!(app.mode, Mode::Manifest);
    app.handle_key(GameKey::Esc);
    assert_in_page(&app, Mode::Companion);
    assert_eq!(app.menu_selected, 1);
}

#[test]
fn equip_memories_and_rename_return_to_the_roster_page() {
    for (key, screen) in [
        ('E', Mode::CompanionEquip),
        ('R', Mode::CompanionMemories),
        ('N', Mode::RenamePet),
    ] {
        let mut app = app_owning_distant_programs(31013, 2);
        open_companions(&mut app);
        press(&mut app, &[GameKey::Right, GameKey::Down, GameKey::Enter]);
        app.handle_key(GameKey::Char(key));
        assert_eq!(app.mode, screen, "{key}");
        app.handle_key(GameKey::Esc);
        assert_in_page(&app, Mode::Companion);
        // Only the memories page parks the row (`keeps_highlight`); equip and
        // rename have always reopened the roster at the top.
        if key == 'R' {
            assert_eq!(app.menu_selected, 1, "memories keeps the row");
        }
    }
}

#[test]
fn fuse_esc_lands_on_programs_in_page() {
    let mut app = app_owning_distant_programs(31014, 2);
    mark_researched(&mut app, "program_refactoring");
    open_via_menu(&mut app, 'p', "Fuse two programs");
    assert_eq!(app.mode, Mode::Fuse);
    app.handle_key(GameKey::Esc);
    assert_in_page(&app, Mode::PartyPrograms);
}

#[test]
fn refactor_and_develop_are_reached_from_programs() {
    let mut app = app_owning_a_program_and_an_extractor_with_cargo(
        31015,
        &[],
        &[
            ("privilege_ring", 3),
            ("scrap_plating", 1),
            ("bounds_check", 1),
            ("inline_cache", 1),
        ],
    );
    let rows = party_rows(&mut app);
    for (label, mode) in [
        ("Refactor a program", Mode::Refactor),
        ("Develop a program", Mode::Develop),
    ] {
        assert!(rows.iter().any(|r| r.label == label), "{label} is offered");
        open_via_menu(&mut app, 'p', label);
        assert_eq!(app.mode, mode);
        app.handle_key(GameKey::Esc);
        assert_in_page(&app, Mode::PartyPrograms);
        press(&mut app, &[GameKey::Esc, GameKey::Esc]);
        assert_eq!(app.mode, Mode::Playing);
    }
}

#[test]
fn routine_install_extract_and_etch_return_to_routines_in_page() {
    let mut app = app_owning_a_program_and_an_extractor(31016, &["sandbox"]);
    teach_player_routines(&mut app, &["sandbox"]);
    for (label, mode) in [
        ("Install a routine", Mode::RoutineTarget),
        ("Extract a routine", Mode::Extract),
        ("Etch a routine disk", Mode::RoutineEtch),
    ] {
        open_via_menu(&mut app, 'p', label);
        assert_eq!(app.mode, mode, "{label}");
        app.handle_key(GameKey::Esc);
        assert_in_page(&app, Mode::PartyRoutines);
        press(&mut app, &[GameKey::Esc, GameKey::Esc]);
        assert_eq!(app.mode, Mode::Playing);
    }
}

#[test]
fn the_etch_page_returns_to_routines_in_page() {
    let mut app = test_app(31017);
    teach_player_routines(&mut app, &["sandbox"]);
    open_via_menu(&mut app, 'p', "Etch a routine disk");
    assert_eq!(app.mode, Mode::RoutineEtch);
    app.handle_key(GameKey::Esc);
    assert_in_page(&app, Mode::PartyRoutines);
}

#[test]
fn level_up_p_lands_on_perks_in_page() {
    let mut app = app_with_perk_points(31020, 1);
    app.mode = Mode::LevelUp;
    app.handle_key(GameKey::Char('P'));
    assert_in_page(&app, Mode::Perks);
}

#[test]
fn allocate_stats_returns_to_perks_in_page() {
    let mut app = app_with_points(31021, 3, 1);
    open_via_menu(&mut app, 'p', "Perks");
    app.handle_key(GameKey::Char('S'));
    assert_eq!(app.mode, Mode::AllocateStats, "{:?}", app.status_line);
    app.handle_key(GameKey::Esc);
    assert_in_page(&app, Mode::Perks);
}

#[test]
fn perk_bought_and_respec_return_to_perks_in_page_with_the_row_kept() {
    let mut app = app_with_perk_points(31022, 20);
    open_via_menu(&mut app, 'p', "Perks");
    app.menu_selected = 2;
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::PerkBought, "{:?}", app.status_line);
    app.handle_key(GameKey::Enter);
    assert_in_page(&app, Mode::Perks);
    assert_eq!(app.menu_selected, 2);

    app.handle_key(GameKey::Char('X'));
    assert_eq!(app.mode, Mode::RespecPerksConfirm);
    app.handle_key(GameKey::Char('n'));
    assert_in_page(&app, Mode::Perks);
    app.handle_key(GameKey::Char('X'));
    app.handle_key(GameKey::Char('y'));
    assert_in_page(&app, Mode::Perks);
}

#[test]
fn perk_previews_are_fresh_after_a_rail_turn() {
    let mut app = app_with_perk_points(31023, 20);
    open_via_menu(&mut app, 'p', "Perks");
    let stale = app.perk_previews.clone();
    // Turn away, change the perks under the page, turn back.
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Char('['));
    assert_eq!(app.mode, Mode::PartyRoutines);
    let id = app.game.as_mut().unwrap().perk_defs()[0].id;
    app.game
        .as_mut()
        .unwrap()
        .buy_perk(id)
        .expect("a point to spend");
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::Perks);

    let game = app.game.as_mut().unwrap();
    let fresh: Vec<_> = game
        .perk_defs()
        .into_iter()
        .map(|def| game.preview_perk(def.id))
        .collect();
    assert_eq!(app.perk_previews, fresh, "the rail turn refreshed them");
    assert_ne!(app.perk_previews, stale, "and they moved with the purchase");
}

#[test]
fn i_reopening_on_perks_refreshes_the_previews() {
    let mut app = app_with_perk_points(31024, 20);
    open_via_menu(&mut app, 'p', "Perks");
    press(&mut app, &[GameKey::Esc, GameKey::Esc]);
    let id = app.game.as_mut().unwrap().perk_defs()[0].id;
    app.game
        .as_mut()
        .unwrap()
        .buy_perk(id)
        .expect("a point to spend");
    app.handle_key(GameKey::Char('i'));
    assert_eq!(app.mode, Mode::Perks);
    let game = app.game.as_mut().unwrap();
    let fresh: Vec<_> = game
        .perk_defs()
        .into_iter()
        .map(|def| game.preview_perk(def.id))
        .collect();
    assert_eq!(app.perk_previews, fresh);
}
