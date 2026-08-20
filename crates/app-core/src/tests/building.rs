//! Placing, demolishing and upgrading structures through the menus.

use super::support::*;
use crate::*;
use feral_processes_engine::species::AffinityClass;

#[test]
fn the_upgrade_picker_opens_from_the_base_menu_and_esc_backs_into_it() {
    // A Compiler, not a Home: the row is hidden unless something nearby
    // actually declares an upgrade path (see `App::upgradeable_structures`).
    let mut app = app_owning_a_program_and_a_compiler(230, &[]);

    open_via_menu(&mut app, 'b', "Upgrade a structure");
    assert_eq!(app.mode, Mode::Upgrade);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BaseMenu, "Esc walks back up one level");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing, "and out to the map from there");
}

/// A structure the current zone caps at Mk1 is still offered, not filtered
/// out. Hiding it would take the whole Upgrade row out of the base menu for
/// the entirety of zone 1 — see `app/group_menu.rs`, which drops a row whose
/// screen would be empty — and a player who has never breached would never
/// learn that upgrading exists. The row explains itself instead, which is
/// what `EntityView::ceiling` is carried for.
#[test]
fn a_structure_at_its_zone_ceiling_is_still_listed_with_the_ceiling_shown() {
    let mut app = app_owning_a_program_and_a_compiler(232, &[]);

    let listed = app.upgradeable_structures();
    let compiler = listed
        .iter()
        .find(|e| e.label.contains("Compiler"))
        .expect("a zone-capped structure must still be offered, not hidden");

    assert_eq!(compiler.tier, Some(1));
    assert_eq!(
        compiler.ceiling,
        Some(1),
        "zone 1 caps the Compiler at Mk1 even though its def allows Mk5"
    );
}

/// Home is the first entry in the build menu and declares no upgrade path,
/// which makes it the fixture for "deployed, but nothing to upgrade".
fn deploy_home(app: &mut App) {
    open_via_menu(app, 'b', "Deploy a structure");
    app.handle_key(GameKey::Enter);
    app.handle_key(GameKey::Up);
    assert_eq!(structure_count(app), 1, "Home should now be deployed");
}

/// Home declares no upgrade path, so a base consisting only of one leaves
/// nothing to upgrade — and the base menu now says so by not offering the
/// row at all, rather than opening a picker with no entries in it.
#[test]
fn a_structure_with_no_upgrade_path_hides_the_upgrade_row() {
    let mut app = test_app(231);
    stand_in_base(&mut app);
    deploy_home(&mut app);

    app.handle_key(GameKey::Char('b'));
    let rows: Vec<_> = app.base_menu_rows().iter().map(|r| r.label).collect();
    assert!(
        rows.contains(&"Demolish a structure"),
        "Home is deployed, so the rows that take any structure are live: {rows:?}"
    );
    assert!(
        !rows.contains(&"Upgrade a structure"),
        "Home has no upgrade path, so the row leads nowhere: {rows:?}"
    );
}

fn structure_count(app: &mut App) -> usize {
    app.game
        .as_mut()
        .unwrap()
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.is_structure)
        .count()
}

#[test]
fn build_menu_number_key_reaches_the_direction_picker_and_can_place_a_structure() {
    let mut app = test_app(101);
    assert!(app.game.is_some(), "test game should have loaded");
    assert!(app.mode == Mode::Playing);
    // Deploying happens from inside the base now.
    stand_in_base(&mut app);

    let structure_count_in_menu = app.game.as_mut().unwrap().buildable_structure_defs().len();
    let mut placed = false;
    // Navigate with Down + Enter rather than a digit key, both to
    // exercise the new arrow-navigation path and because a menu with
    // more than 9 rows can't be reached by a single digit at all.
    'outer: for n in 0..structure_count_in_menu {
        for dir in [GameKey::Up, GameKey::Down, GameKey::Left, GameKey::Right] {
            let before = structure_count(&mut app);

            open_via_menu(&mut app, 'b', "Deploy a structure");
            assert!(app.mode == Mode::Build, "the base menu should reach Deploy");

            for _ in 0..n {
                app.handle_key(GameKey::Down);
            }
            app.handle_key(GameKey::Enter);
            assert!(
                app.mode == Mode::BuildDirection,
                "picking structure {n} via Down+Enter should move to the direction picker"
            );

            app.handle_key(dir);
            assert!(
                app.mode == Mode::Playing,
                "the direction picker should return to Playing either way"
            );

            if structure_count(&mut app) > before {
                placed = true;
                break 'outer;
            }
        }
    }
    assert!(
        placed,
        "should have been able to place at least one of the {structure_count_in_menu} structures \
         in at least one of the four directions"
    );
}

/// Exercises the demolish flow end to end through `App::handle_key`:
/// picking Home moves to a confirmation step instead of demolishing
/// immediately (unlike any other structure — see `Game::remove_structure`
/// for why Home is special), `n` backs out leaving it standing, and `y`
/// actually demolishes it.
#[test]
fn remove_key_on_home_requires_confirmation_before_demolishing() {
    let mut app = test_app(203);
    stand_in_base(&mut app);

    deploy_home(&mut app);

    open_via_menu(&mut app, 'b', "Demolish a structure");
    assert_eq!(app.mode, Mode::Remove);
    app.handle_key(GameKey::Enter);
    assert!(
        app.mode == Mode::RemoveConfirm,
        "picking Home should require confirmation instead of demolishing immediately"
    );
    assert_eq!(
        structure_count(&mut app),
        1,
        "Home shouldn't be removed yet"
    );

    app.handle_key(GameKey::Char('n'));
    assert!(app.mode == Mode::Playing);
    assert_eq!(
        structure_count(&mut app),
        1,
        "declining the warning should leave Home in place"
    );

    open_via_menu(&mut app, 'b', "Demolish a structure");
    app.handle_key(GameKey::Enter);
    assert!(app.mode == Mode::RemoveConfirm);
    app.handle_key(GameKey::Char('y'));
    assert!(app.mode == Mode::Playing);
    assert_eq!(
        structure_count(&mut app),
        0,
        "confirming should demolish Home"
    );
}

/// A cronjob posts a program to a node; working it yourself is the same job,
/// so it offers the same `App::workable_structures` list rather than a second
/// kind of screen.
#[test]
fn working_a_structure_yourself_opens_the_same_structure_list() {
    let mut app = app_owning_a_program_and_a_compiler(960, &[]);
    open_via_menu(&mut app, 'b', "Work a structure yourself");
    assert_eq!(app.mode, Mode::WorkStructure);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BaseMenu, "Esc backs into the base menu");
}

/// A companion's `Position` is the tile it was captured on and is never
/// written again, so the posting picker must offer the whole roster rather
/// than a window onto the map. It used to scan `MENU_SCAN_RADIUS`, which hid
/// every program tamed further out than that — and because `base_menu_rows`
/// drops a row whose first screen would be empty, a player whose only
/// program was tamed that far away lost the Cronjob row with it.
#[test]
fn the_posting_picker_offers_programs_parked_far_from_the_player() {
    // The fixture parks both of these beyond `MENU_SCAN_RADIUS`.
    let mut app = app_owning_distant_programs(741, 2);
    let roster = app.game.as_mut().unwrap().owned_pets().len();
    assert!(
        roster >= 2,
        "fixture should hand the player at least the two distant programs"
    );

    let offered = app.nearby_programs();

    assert_eq!(
        offered.len(),
        roster,
        "the picker must offer every program the player owns, wherever it was tamed"
    );
    assert!(
        offered.iter().all(|v| v.is_tamed),
        "and still list only tamed programs"
    );
}

/// `d` + a direction demolishes what is on the neighbouring tile, without
/// going through the base menu's picker at all.
#[test]
fn the_demolish_key_removes_the_structure_next_to_you() {
    let mut app = app_inside_a_small_base(240, false);
    assert_eq!(
        structure_count(&mut app),
        2,
        "precondition: Home and a node"
    );

    app.handle_key(GameKey::Char('d'));
    assert_eq!(
        app.mode,
        Mode::RemoveDirection,
        "the key opens a direction prompt rather than a list"
    );

    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(
        structure_count(&mut app),
        1,
        "the node east of the player should be gone"
    );
}

/// Home cascades to the whole base, so the hotkey routes it into the same
/// warning the menu does rather than taking the base down on one keypress.
#[test]
fn the_demolish_key_still_asks_before_taking_down_home() {
    let mut app = app_inside_a_small_base(241, false);

    app.handle_key(GameKey::Char('d'));
    app.handle_key(GameKey::Up);
    assert_eq!(
        app.mode,
        Mode::RemoveConfirm,
        "Home must reach the confirmation screen, not be demolished outright"
    );
    assert_eq!(structure_count(&mut app), 2, "nothing is gone yet");

    app.handle_key(GameKey::Char('y'));
    assert_eq!(
        structure_count(&mut app),
        0,
        "confirming takes Home and the cascade with it"
    );
}

#[test]
fn the_demolish_key_says_when_there_is_nothing_that_way() {
    let mut app = app_inside_a_small_base(242, false);

    app.handle_key(GameKey::Char('d'));
    app.handle_key(GameKey::Down);

    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(structure_count(&mut app), 2, "nothing was demolished");
    assert!(
        app.status_line
            .as_deref()
            .is_some_and(|s| s.contains("Nothing to demolish")),
        "an empty neighbour has to say so: {:?}",
        app.status_line
    );
}

/// `Position` is pinned to the surface entrance tile underground, so a
/// direction key down there would aim at the base overhead. Refused at the
/// keypress, matching the `surface_only` flag the menu's Demolish row carries.
#[test]
fn the_demolish_key_is_refused_underground() {
    let mut app = app_inside_a_small_base(243, true);
    assert!(
        app.game.as_ref().is_some_and(|g| g.is_underground()),
        "precondition: the fixture really went down"
    );

    app.handle_key(GameKey::Char('d'));

    assert_eq!(
        app.mode,
        Mode::Playing,
        "the direction prompt must not even open down here"
    );
    assert!(
        app.status_line.is_some(),
        "a refused key says why rather than doing nothing"
    );
}

/// The roster is where you go to find out what is *idle*; making the player
/// back out to a program-first picker to act on what they just found is the
/// friction Enter removes.
#[test]
fn enter_on_a_workable_roster_row_opens_the_staffing_picker() {
    let mut app = app_inside_a_small_base_with_programs(244, false, 1);
    open_via_menu(&mut app, 'b', "Structure roster");
    app.handle_key(GameKey::Down);
    assert_eq!(
        node_row(&mut app),
        app.menu_selected,
        "precondition: the node is the highlighted row"
    );

    app.handle_key(GameKey::Enter);

    assert_eq!(app.mode, Mode::StructureAssign);
}

/// Setting a standing job leaves the screen up, so the player can set both
/// on one machine — a toggle is not a commitment the way posting was.
#[test]
fn a_standing_job_set_from_the_roster_sticks_and_stays_on_screen() {
    let mut app = app_inside_a_small_base_with_programs(245, false, 1);
    open_via_menu(&mut app, 'b', "Structure roster");
    app.handle_key(GameKey::Down);
    let row = app.menu_selected;
    app.handle_key(GameKey::Enter);

    let toggle = app
        .staffing()
        .expect("the toggles are open")
        .rows
        .iter()
        .position(|r| r.kind == StaffAction::StandingWork)
        .expect("a Mining Node can be kept running");
    app.handle_key(GameKey::Char(menu_shortcut(toggle)));

    assert_eq!(app.mode, Mode::StructureAssign, "still on the toggles");
    assert!(
        app.staffing().unwrap().rows[toggle].on == Some(true),
        "the instruction stuck: {:?}",
        app.status_line
    );
    let node = app.game.as_mut().unwrap().structure_report().remove(row);
    assert_eq!(node.kind, "mining_node", "sanity: the right row");
}

/// Esc is a way back into the roster, not out of it: the pick was a
/// side-trip from a screen the player was reading.
#[test]
fn esc_from_the_staffing_picker_returns_to_the_roster() {
    let mut app = app_inside_a_small_base_with_programs(246, false, 1);
    open_via_menu(&mut app, 'b', "Structure roster");
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::StructureAssign);

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Structures);
}

/// The roster lists everything standing, and most of it takes no worker.
/// Enter on a Home has to say so rather than be a dead key — the roster
/// cannot filter those rows out the way `App::upgradeable_structures` can,
/// because showing the whole base is what the screen is for.
#[test]
fn enter_on_a_row_that_takes_no_worker_says_so() {
    let mut app = app_inside_a_small_base_with_programs(247, false, 1);
    open_via_menu(&mut app, 'b', "Structure roster");
    assert!(
        app.game.as_mut().unwrap().structure_report()[app.menu_selected].is_home,
        "precondition: the roster opens on the Home"
    );

    app.handle_key(GameKey::Enter);

    assert_eq!(app.mode, Mode::Structures, "the picker must not open");
    assert!(
        app.status_line.is_some(),
        "a refused key says why rather than doing nothing"
    );
}

/// `Game::work_structure` refuses unless the player is orthogonally beside
/// the structure, and the roster is zone-wide — so the row that offers it
/// appears only where it would be accepted, rather than being offered and
/// then refused.
#[test]
fn working_it_yourself_is_offered_only_from_the_next_tile() {
    let mut app = app_inside_a_small_base_with_programs(248, false, 1);
    open_via_menu(&mut app, 'b', "Structure roster");
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Enter);
    assert!(
        app.staffing()
            .unwrap()
            .rows
            .iter()
            .any(|r| r.kind == StaffAction::WorkYourself),
        "the player is standing right beside this node"
    );

    // The Compiler in this fixture sits 30 tiles off, which is the same
    // question asked of a structure the player is nowhere near.
    let mut far = app_owning_a_program_and_a_compiler(249, &[]);
    open_via_menu(&mut far, 'b', "Structure roster");
    let compiler = far
        .game
        .as_mut()
        .unwrap()
        .structure_report()
        .iter()
        .position(|s| s.kind == "compiler")
        .expect("the fixture deploys one");
    far.menu_selected = compiler;
    far.handle_key(GameKey::Enter);
    assert_eq!(far.mode, Mode::StructureAssign, "a Compiler takes a worker");
    assert!(
        far.staffing()
            .unwrap()
            .rows
            .iter()
            .all(|r| r.kind != StaffAction::WorkYourself),
        "you cannot work something you are not standing next to"
    );
}

/// The roster reads the same underground, but `work_structure` is behind
/// `require_base` — `Position` is pinned
/// to the entrance tile down there, so posting would measure a walk from the
/// wrong end of the map. Refused at the keypress, like the demolish key.
#[test]
fn the_roster_does_not_staff_anything_underground() {
    let mut app = app_inside_a_small_base_with_programs(250, true, 1);
    assert!(
        app.game.as_ref().is_some_and(|g| g.is_underground()),
        "precondition: the fixture really went down"
    );
    open_via_menu(&mut app, 'b', "Structure roster");
    app.handle_key(GameKey::Down);

    app.handle_key(GameKey::Enter);

    assert_eq!(
        app.mode,
        Mode::Structures,
        "the picker must not even open down here"
    );
}

/// The Base Staff screen lists every program you own, and there are three
/// places one can be: on the staff, in your party, and neither. `doing` was
/// `None` for anything off the staff and the renderer read that single `None`
/// as "party" — so a program you had just tamed, or one you had just stood
/// down from the base, claimed to be fighting alongside you. Only
/// `add_companion` ever pushes into `Party`, so "neither" is the state every
/// program starts in and the screen was wrong about it by default.
#[test]
fn the_staff_screen_tells_a_party_member_from_an_idle_program() {
    let mut app = app_owning_distant_programs(742, 2);

    let rows = app.base_staff_rows();
    assert_eq!(rows.len(), 2, "fixture hands the player two programs");
    assert!(
        rows.iter().all(|r| !r.on_staff && r.doing == "idle"),
        "a program that has been given no job is idle, not in the party: {:?}",
        rows.iter().map(|r| r.doing.clone()).collect::<Vec<_>>()
    );

    let (member, other) = (rows[0].program.entity, rows[1].program.entity);
    app.game.as_mut().unwrap().add_companion(member).unwrap();

    let rows = app.base_staff_rows();
    let doing = |e| {
        rows.iter()
            .find(|r| r.program.entity == e)
            .map(|r| r.doing.clone())
            .unwrap()
    };
    assert_ne!(
        doing(member),
        doing(other),
        "the one that fell in beside you must not read the same as the one that didn't"
    );
    assert!(
        doing(member).contains("party"),
        "and it is the party it is in, got {:?}",
        doing(member)
    );
    assert_eq!(doing(other), "idle");
}

/// The third state: a program on the staff reads as what the base has it
/// doing, and the row still carries the flag Enter acts on.
#[test]
fn a_staffed_program_reads_as_staff_rather_than_as_idle() {
    let mut app = app_owning_distant_programs(743, 1);
    let program = app.base_staff_rows()[0].program.entity;

    app.game
        .as_mut()
        .unwrap()
        .assign_base_staff(program)
        .unwrap();

    let row = app.base_staff_rows().remove(0);
    assert!(row.on_staff, "Enter must now release rather than assign");
}

/// The roster sorts the Home first and the node after it, so one Down from
/// the opening row is the workable structure — asserted rather than assumed,
/// since every staffing test above rides on it.
fn node_row(app: &mut App) -> usize {
    app.game
        .as_mut()
        .unwrap()
        .structure_report()
        .iter()
        .position(|s| s.kind == "mining_node")
        .expect("the fixture deploys one")
}

/// The Base Staff row carries what the program is worth at a post, so the
/// player picks staff on the facts the sim actually reads rather than on the
/// name.
///
/// Two species, not two copies of one: rootkit and sprite disagree on all
/// three answers, which is what makes a screen reading row `i`'s facts off
/// program `j` fail here instead of passing on identical numbers.
#[test]
fn a_staff_row_carries_what_the_program_is_worth_at_a_post() {
    let mut app = app_owning_distant_programs_of(744, &["rootkit", "sprite"]);

    let rows = app.base_staff_rows();
    let profile = |name: &str| {
        rows.iter()
            .find(|r| r.program.label.contains(name))
            .unwrap_or_else(|| panic!("fixture spawns a {name}"))
            .work
            .expect("a shipped species has a work profile")
    };

    let rootkit = profile("Rootkit");
    let sprite = profile("Sprite");
    assert_eq!(rootkit.speed, 9);
    assert_eq!(rootkit.analysis, 13);
    assert_eq!(rootkit.class, Some(AffinityClass::Leech));
    assert_ne!(
        sprite.speed, rootkit.speed,
        "the two rows must not be reporting the same program"
    );
}
