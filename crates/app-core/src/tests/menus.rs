//! Row shortcuts and menu navigation.

use super::support::*;
use crate::*;
use feral_processes_engine::ManifestSubject;
use feral_processes_engine::abilities::FALLBACK_ABILITY_ID;

/// An irreversible confirm opens on its No row, so a pick of row 0 has to
/// walk off the opening row rather than assume the highlight starts at 0.
#[test]
fn pick_row_picks_from_a_non_zero_opening_row() {
    let mut app = app_owning_a_program_and_an_extractor(9101, &[FALLBACK_ABILITY_ID]);
    open_via_menu(&mut app, 'p', "Extract a routine");
    pick_row(&mut app, 0);
    pick_row(&mut app, 0);
    assert_eq!(app.mode, Mode::ExtractConfirm);
    assert_eq!(
        app.menu_selected, 1,
        "test premise: the confirm opens on No"
    );
    let before = app.game.as_mut().unwrap().owned_pets().len();

    pick_row(&mut app, 0);

    assert_eq!(
        app.game.as_mut().unwrap().owned_pets().len(),
        before - 1,
        "row 0 is Yes, so the program is extracted"
    );
}

#[test]
fn pick_row_reaches_row_twelve() {
    let mut app = app_on_inventory_with_many_items(9102);
    open_hub_screen(&mut app, Mode::Inventory);
    assert!(
        app.game.as_ref().unwrap().player_status().inventory.len() >= 13,
        "test premise: a thirteen-row list"
    );
    highlight_row(&mut app, 12);
    assert_eq!(app.menu_selected, 12);
}

/// Lists are highlight + Enter. A digit or a lowercase letter names no row,
/// so nothing a player types as a mnemonic can also pick one: the keys are
/// free for the screens that bind them, and for a later type-to-filter.
///
/// The twelve-row menu is long enough to reach past the nine digits into the
/// letters, which is where the old scheme kept going.
#[test]
fn digits_and_lowercase_letters_pick_no_row() {
    let mut app = test_app(920);
    let len = 35;
    app.menu_selected = 3;
    for c in ('0'..='9').chain('a'..='z') {
        assert_eq!(
            app.selected_index(GameKey::Char(c), len),
            None,
            "[{c}] must not pick a row"
        );
        assert_eq!(app.menu_selected, 3, "[{c}] must not move the highlight");
    }
}

/// Through a real screen, not only the resolver: a long list, every digit
/// pressed, and neither the mode nor the highlight moves. Letters are not
/// pressed here because the inventory binds many of them as mnemonics.
#[test]
fn digits_leave_a_long_screen_where_it_was() {
    let mut app = app_on_inventory_with_many_items(9103);
    open_hub_screen(&mut app, Mode::Inventory);
    assert!(app.game.as_ref().unwrap().player_status().inventory.len() >= 12);
    highlight_row(&mut app, 2);
    for c in '0'..='9' {
        app.handle_key(GameKey::Char(c));
        assert_eq!(app.mode, Mode::Inventory, "[{c}] left the screen");
        assert_eq!(app.menu_selected, 2, "[{c}] moved the highlight");
    }
}

/// Word letters are not row selectors and keep working: the main menu's own
/// letters and the routine installer's `e`. (Alerts `x`/`d`, the sprite
/// picker's `t`, the kernel ring's `r` and the wizard's `n` are pinned where
/// those screens' own tests live.) A letter that is no screen's mnemonic does
/// nothing at all, where it used to pick a row.
#[test]
fn mnemonic_letters_still_act() {
    let mut app = test_app(923);
    app.game = None;
    for (key, mode) in [
        ('a', Mode::Achievements),
        ('o', Mode::GameplayOptions),
        ('q', Mode::QuitAppConfirm),
    ] {
        app.mode = Mode::MainMenu;
        app.menu_selected = 0;
        app.handle_key(GameKey::Char(key));
        assert_eq!(app.mode, mode, "main menu [{key}]");
    }
    app.mode = Mode::MainMenu;
    app.menu_selected = 0;
    app.handle_key(GameKey::Char('z'));
    assert_eq!(app.mode, Mode::MainMenu, "[z] is no main-menu key");
    assert_eq!(app.menu_selected, 0);

    let mut app = test_app(924);
    teach_player_routines(&mut app, &["sandbox"]);
    open_via_menu(&mut app, 'p', "Install a routine");
    pick_row(&mut app, 0);
    pick_row(&mut app, 1);
    assert_eq!(app.mode, Mode::RoutineInstall);
    let row = app.menu_selected;
    app.handle_key(GameKey::Char('z'));
    assert_eq!(app.mode, Mode::RoutineInstall, "[z] is no installer key");
    assert_eq!(app.menu_selected, row);
    app.handle_key(GameKey::Char('e'));
    assert_eq!(app.mode, Mode::RoutineEtch, "installer [e]");
}

#[test]
fn the_companions_page_opens_the_manifest_on_you_and_esc_backs_out() {
    let mut app = test_app(70);
    open_via_menu(&mut app, 'p', "Read a manifest");
    assert_eq!(app.mode, Mode::Manifest);
    assert_eq!(app.manifest_origin, ManifestOrigin::Menu);
    assert_eq!(
        app.pending_manifest,
        Some(app.game.as_ref().unwrap().player_entity()),
        "the menu row opens subject 0, which is you"
    );
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::PartyPrograms, "Esc walks back up one level");
    assert!(!app.hub_on_rail, "into the page it was opened from");
    app.handle_key(GameKey::Esc);
    assert!(app.hub_on_rail);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn the_manifest_menu_row_opens_on_you() {
    let mut app = test_app(71);
    let player = app.game.as_ref().unwrap().player_status().position;

    open_via_menu(&mut app, 'p', "Read a manifest");
    assert_eq!(app.mode, Mode::Manifest);
    let subject = app.pending_manifest.expect("the row opens a subject");
    let view = app.game.as_ref().unwrap().manifest(subject).unwrap();
    assert_eq!(view.name, "You", "you are always the first row");
    let ManifestSubject::Player(p) = view.subject else {
        panic!("row 1 is the player");
    };
    assert_eq!(p.position, player);
}

#[test]
fn paging_from_the_menu_row_reaches_every_owned_program_after_you() {
    let mut app = app_owning_distant_programs(72, 2);
    let subjects = app.manifest_subjects();
    assert_eq!(subjects.len(), 3, "you plus two programs");

    open_via_menu(&mut app, 'p', "Read a manifest");
    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::Manifest);
    assert_eq!(app.pending_manifest, Some(subjects[1]));
}

/// A modifier is inert outside the screens `handle_key`'s fold names.
///
/// The frontend always sends the modified form, so without the fold in
/// `handle_key` a player holding Shift would find the arrow keys dead on
/// every screen in the game — and dead quietly, since every other handler
/// ends in a `_ => {}`. The manifest pager stands in for all of them: it is
/// the nearest other Left/Right consumer that reports where it landed.
#[test]
fn a_modifier_is_stripped_outside_the_menus() {
    let mut app = app_owning_distant_programs(74, 2);
    let subjects = app.manifest_subjects();
    open_via_menu(&mut app, 'p', "Read a manifest");
    assert_eq!(app.pending_manifest, Some(subjects[0]));

    app.handle_key(GameKey::ShiftRight);
    assert_eq!(
        app.pending_manifest,
        Some(subjects[1]),
        "Shift+Right pages exactly as Right does"
    );
    app.handle_key(GameKey::CtrlLeft);
    assert_eq!(
        app.pending_manifest,
        Some(subjects[0]),
        "and Ctrl+Left exactly as Left"
    );
}

#[test]
fn left_and_right_cycle_the_owned_subjects_and_wrap_at_both_ends() {
    let mut app = app_owning_distant_programs(73, 2);
    let subjects = app.manifest_subjects();
    open_via_menu(&mut app, 'p', "Read a manifest");
    assert_eq!(app.pending_manifest, Some(subjects[0]));

    app.handle_key(GameKey::Right);
    assert_eq!(app.pending_manifest, Some(subjects[1]));
    app.handle_key(GameKey::Right);
    assert_eq!(app.pending_manifest, Some(subjects[2]));
    app.handle_key(GameKey::Right);
    assert_eq!(
        app.pending_manifest,
        Some(subjects[0]),
        "past the last subject wraps to the first"
    );
    app.handle_key(GameKey::Left);
    assert_eq!(
        app.pending_manifest,
        Some(subjects[2]),
        "before the first subject wraps to the last"
    );
    assert_eq!(app.mode, Mode::Manifest, "cycling never leaves the screen");
}

#[test]
fn up_and_down_page_the_subjects_as_left_and_right_do() {
    let mut app = app_owning_distant_programs(7501, 2);
    let subjects = app.manifest_subjects();
    open_via_menu(&mut app, 'p', "Read a manifest");
    app.handle_key(GameKey::Down);
    assert_eq!(app.pending_manifest, Some(subjects[1]));
    app.handle_key(GameKey::Up);
    app.handle_key(GameKey::Up);
    assert_eq!(app.pending_manifest, Some(subjects[2]), "Up wraps");
    assert_eq!(app.mode, Mode::Manifest);
}

#[test]
fn a_digit_or_letter_does_not_jump_the_manifest() {
    let mut app = app_owning_distant_programs(7502, 2);
    let subjects = app.manifest_subjects();
    open_via_menu(&mut app, 'p', "Read a manifest");
    assert_eq!(app.pending_manifest, Some(subjects[0]));
    for c in ('0'..='9').chain('a'..='z') {
        app.handle_key(GameKey::Char(c));
        assert_eq!(app.pending_manifest, Some(subjects[0]), "[{c}] jumped");
        assert_eq!(app.mode, Mode::Manifest, "[{c}] left the sheet");
    }
}

/// Paging keeps a program's Social face, and a page onto you, who have no
/// Social face, falls back to Stats.
#[test]
fn paging_keeps_the_face_and_falls_back_on_you() {
    let mut app = app_owning_distant_programs(7504, 2);
    let subjects = app.manifest_subjects();
    open_via_menu(&mut app, 'p', "Read a manifest");
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Tab);
    assert_eq!(app.manifest_tab, ManifestTab::Social);
    app.handle_key(GameKey::Right);
    assert_eq!(app.pending_manifest, Some(subjects[2]));
    assert_eq!(app.manifest_tab, ManifestTab::Social);
    app.handle_key(GameKey::Right);
    assert_eq!(app.pending_manifest, Some(subjects[0]));
    assert_eq!(app.manifest_tab, ManifestTab::Stats);
}

#[test]
fn a_wild_subject_ignores_digits_letters_and_up_down() {
    let mut app = app_owning_distant_programs(7503, 2);
    let (wild, direction) = a_wild_program_and_its_direction(&mut app);
    app.handle_key(GameKey::Char('x'));
    app.handle_key(direction);
    assert_eq!(app.pending_manifest, Some(wild));
    for key in [
        GameKey::Char('1'),
        GameKey::Char('2'),
        GameKey::Char('a'),
        GameKey::Up,
        GameKey::Down,
    ] {
        app.handle_key(key);
        assert_eq!(app.pending_manifest, Some(wild), "{key:?}");
        assert_eq!(app.mode, Mode::Manifest, "{key:?}");
    }
}

/// A wild program three tiles due east of the player, and the key that
/// looks at it.
///
/// It *places* one rather than finding whichever the seed scattered nearby
/// and deriving a dominant direction from it, which is what this did while
/// `x` was a 90° cone: a program mostly-east-and-somewhat-north answered to
/// `Right` back then and answers to nothing now. Placing it makes the
/// fixture state the arrangement instead of inheriting one, and the returned
/// key is a constant.
fn a_wild_program_and_its_direction(app: &mut App) -> (Entity, GameKey) {
    let wild = place_wild_program_east(app, 3);
    (wild, GameKey::Right)
}

#[test]
fn cycling_does_nothing_when_the_subject_is_a_program_you_do_not_own() {
    let mut app = test_app(74);
    // A wild program reached through `i` + direction is not in the owned
    // list, so there is nothing to cycle to.
    let (wild, _) = a_wild_program_and_its_direction(&mut app);

    app.pending_manifest = Some(wild);
    app.mode = Mode::Manifest;
    app.handle_key(GameKey::Right);
    assert_eq!(app.pending_manifest, Some(wild));
    assert_eq!(app.mode, Mode::Manifest);
}

#[test]
fn esc_returns_to_the_menu_when_the_manifest_was_opened_from_it() {
    let mut app = app_owning_distant_programs(75, 1);
    open_via_menu(&mut app, 'p', "Read a manifest");
    assert_eq!(app.mode, Mode::Manifest);

    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.mode,
        Mode::PartyPrograms,
        "Esc leaves for the page the manifest was opened from"
    );
    assert_eq!(app.pending_manifest, None);
}

/// Reached with `x` there is no list behind the sheet, so Esc goes straight
/// back to the map rather than dropping the player into a picker they never
/// opened — and which wouldn't list the wild program anyway.
#[test]
fn esc_leaves_for_the_map_when_the_manifest_was_opened_from_the_world() {
    let mut app = test_app(77);
    let (_, direction) = a_wild_program_and_its_direction(&mut app);
    app.handle_key(GameKey::Char('x'));
    app.handle_key(direction);
    assert_eq!(app.mode, Mode::Manifest);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.pending_manifest, None);
}

#[test]
fn inspecting_a_direction_still_lands_on_the_manifest() {
    let mut app = test_app(76);
    let (_, direction) = a_wild_program_and_its_direction(&mut app);

    app.handle_key(GameKey::Char('x'));
    assert_eq!(app.mode, Mode::InspectDirection);
    app.handle_key(direction);
    assert_eq!(app.mode, Mode::Manifest);
    assert!(
        app.pending_manifest.is_some(),
        "the inspected program is the manifest's subject"
    );
}

/// Reloads the game with Analysis raised by 14, which is +7 tiles of
/// Perception, so something at the examine range is inside the fog's
/// reach. Goes through the real derivation on load; no sight bypass.
fn see_twelve_tiles(app: &mut App) {
    let game = app.game.as_mut().unwrap();
    let analysis = game
        .attributes_of(game.player_entity())
        .get(&"analysis".into())
        .unwrap();
    let path = scratch_path("see_twelve_tiles", 79);
    game.save(&path).unwrap();
    let mut data = feral_processes_engine::save::load_from_file(&path).unwrap();
    data.player
        .attributes
        .insert("analysis".into(), analysis + 14);
    feral_processes_engine::save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
    let _ = std::fs::remove_file(&path);
}

/// The shipped reach, pinned at the seam that chooses it.
///
/// `find_target_in_direction` takes its range as a parameter so the engine's
/// own tests stay immune to a retune, which leaves *which* number the game
/// passes untested there. It used to pass `MENU_SCAN_RADIUS` — a menu
/// window, 40 tiles, more than twice the map pane in either axis — so `x`
/// named things the player could not see. This is what would fail if that
/// came back.
#[test]
fn inspecting_a_direction_reaches_no_further_than_the_examine_range() {
    use feral_processes_engine::tuning::EXAMINE_RANGE_TILES;

    let mut app = test_app(79);
    see_twelve_tiles(&mut app);
    place_wild_program_east(&mut app, EXAMINE_RANGE_TILES + 1);
    app.handle_key(GameKey::Char('x'));
    app.handle_key(GameKey::Right);
    assert_eq!(
        app.mode,
        Mode::Playing,
        "one tile past the reach is out of reach"
    );
    assert_eq!(
        app.status_line.as_deref(),
        Some("Nothing in that direction."),
        "and it says so rather than silently doing nothing"
    );

    let mut app = test_app(79);
    see_twelve_tiles(&mut app);
    place_wild_program_east(&mut app, EXAMINE_RANGE_TILES);
    app.handle_key(GameKey::Char('x'));
    app.handle_key(GameKey::Right);
    assert_eq!(
        app.mode,
        Mode::Manifest,
        "and a program exactly at the reach is found, so the miss was the bound"
    );
}

/// A structure in the view path opens its own sheet rather than the
/// creature manifest — the two screens share almost no fields, so they are
/// separate modes (see `Mode::StructureManifest`).
#[test]
fn inspecting_toward_a_structure_opens_the_structure_sheet() {
    let mut app = test_app(76);
    found_the_base(&mut app);
    // One cell west of the Home the founding deploy stood on base space's
    // origin, so it is the thing one step east.
    stand_in_base_at(&mut app, -1, 0);

    app.handle_key(GameKey::Char('x'));
    assert_eq!(app.mode, Mode::InspectDirection);
    app.handle_key(GameKey::Right);
    assert_eq!(
        app.mode,
        Mode::StructureManifest,
        "the Home is one tile east and nothing is nearer"
    );
    assert!(app.pending_structure_manifest.is_some());
    assert!(
        app.pending_manifest.is_none(),
        "and it did not land on the creature manifest"
    );
}

/// The sheet is read-only and reached only from the map, so it has no list
/// to page through — any key leaves, the way a plain popup does.
#[test]
fn any_key_leaves_the_structure_sheet_and_forgets_its_subject() {
    let mut app = test_app(77);
    found_the_base(&mut app);
    stand_in_base_at(&mut app, -1, 0);
    app.handle_key(GameKey::Char('x'));
    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::StructureManifest);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.pending_structure_manifest.is_none());
}

/// The manual's index is an ordinary menu and a page is a document — the
/// split the whole screen is built on, so these walk it end to end.
#[test]
fn the_manual_opens_on_an_index_of_the_shipped_pages() {
    let mut app = test_app(930);
    app.handle_key(GameKey::Char('?'));
    assert_eq!(app.mode, Mode::Help);
    let rows = app.help_index_rows();
    assert!(
        rows.len() >= 4,
        "the shipped manual is missing: {rows:?}",
        rows = rows.len()
    );
    assert!(app.help_stack.is_empty(), "nothing is being read yet");
}

#[test]
fn picking_a_topic_opens_that_page() {
    let mut app = test_app(931);
    app.handle_key(GameKey::Char('?'));
    let first = app.help_index_rows()[0].title.clone();
    pick_row(&mut app, 0);
    assert_eq!(app.mode, Mode::HelpPage);
    assert_eq!(app.help_stack.len(), 1);
    let view = app.help_page_view().expect("a page is open");
    assert_eq!(view.title, first);
    assert!(!view.prose.is_empty());
}

/// The reading trail. Following a link leaves the page you came from
/// underneath it, which is the whole of what makes "three links deep and
/// back" work.
#[test]
fn a_typed_link_pushes_the_trail_and_esc_pops_it_one_level_at_a_time() {
    let mut app = test_app(932);
    app.handle_key(GameKey::Char('?'));
    pick_row(&mut app, 0);
    let first = app.help_stack[0].clone();
    let link = app.help_page_view().unwrap().links;
    assert!(
        !link.is_empty(),
        "the fixture page carries no links, so this proves nothing"
    );

    app.handle_key(GameKey::Char(link[0].shortcut));
    assert_eq!(
        app.mode,
        Mode::HelpPage,
        "a link opens another page, not another screen"
    );
    assert_eq!(app.help_stack.len(), 2);
    assert_eq!(app.help_stack[0], first, "the page beneath is still there");
    assert_ne!(app.help_stack[1], first);

    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.mode,
        Mode::HelpPage,
        "one level back, not out of the manual"
    );
    assert_eq!(app.help_stack, vec![first]);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Help, "the last level lands on the index");
    assert!(app.help_stack.is_empty());
}

#[test]
fn esc_on_the_index_closes_the_manual_and_clears_the_trail() {
    let mut app = test_app(933);
    app.handle_key(GameKey::Char('?'));
    pick_row(&mut app, 0);
    app.mode = Mode::Help;
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.help_stack.is_empty());
}

/// A page scrolls; it does not select. Enter is inert on one, and Down
/// moves the scroll without changing which page is open.
#[test]
fn up_and_down_scroll_a_page_without_changing_which_page_is_open() {
    let mut app = test_app(934);
    app.handle_key(GameKey::Char('?'));
    // The controls page is the long one, so there is something to scroll.
    let idx = app
        .help_index_rows()
        .iter()
        .position(|r| r.title == "Controls")
        .expect("the controls page ships");
    pick_row(&mut app, idx);
    let open = app.help_stack.clone();
    assert!(app.help_page_view().unwrap().prose.len() > 1);

    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, 1);
    assert_eq!(app.help_stack, open, "scrolling is not navigation");

    app.handle_key(GameKey::Enter);
    assert_eq!(app.help_stack, open, "Enter does nothing on a document");
    assert_eq!(app.mode, Mode::HelpPage);
}

/// PageUp/PageDown jump `MENU_PAGE_ROWS` at a time and stop at the ends
/// rather than wrapping: a held key that wrapped would cycle the list
/// forever instead of parking on the row the player was paging toward.
#[test]
fn page_keys_jump_a_page_and_stop_at_the_ends() {
    let mut app = test_app(921);
    let len = MENU_PAGE_ROWS * 2 + 3;
    app.menu_selected = 0;

    assert_eq!(app.selected_index(GameKey::PageDown, len), None);
    assert_eq!(app.menu_selected, MENU_PAGE_ROWS);
    app.selected_index(GameKey::PageDown, len);
    app.selected_index(GameKey::PageDown, len);
    assert_eq!(app.menu_selected, len - 1, "PageDown parks on the last row");

    app.selected_index(GameKey::PageUp, len);
    assert_eq!(app.menu_selected, len - 1 - MENU_PAGE_ROWS);
    app.selected_index(GameKey::PageUp, len);
    app.selected_index(GameKey::PageUp, len);
    assert_eq!(app.menu_selected, 0, "PageUp parks on the first row");
}
