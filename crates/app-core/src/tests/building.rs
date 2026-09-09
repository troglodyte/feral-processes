//! Placing, demolishing and upgrading structures through the menus.

use super::support::*;
use crate::*;
use feral_processes_engine::species::AffinityClass;

#[test]
fn the_upgrade_picker_opens_from_the_base_menu_and_esc_backs_into_it() {
    // A Compiler, not a Home: the row is hidden unless something nearby
    // actually declares an upgrade path (see `App::upgradeable_structures`).
    let mut app = app_owning_a_program_and_a_compiler(230, &[]);
    // Upgrading is a base action, so the row is offered in one locale only.
    stand_in_base(&mut app);

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
///
/// `stand_in_base` is load-bearing here now, not decoration: the Compiler
/// is a `Structure`, and `Game::view_entities` refuses to answer for one
/// outside base space — `upgradeable_structures` would otherwise come back
/// empty on the surface `app_owning_a_program_and_a_compiler` leaves the
/// party on.
#[test]
fn a_structure_at_its_zone_ceiling_is_still_listed_with_the_ceiling_shown() {
    let mut app = app_owning_a_program_and_a_compiler(232, &[]);
    stand_in_base(&mut app);

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
///
/// Driven from the open grid, because founding the run's first Home is the
/// one deploy made from out there — and leaves the party inside afterwards,
/// which is where every other base key is pressed from.
fn deploy_home(app: &mut App) {
    open_via_menu(app, 'b', "Deploy a structure");
    app.handle_key(GameKey::Enter);
    app.handle_key(GameKey::Up);
    assert_eq!(structure_count(app), 1, "Home should now be deployed");
    // Founding fires the base tutorial through the real key path, and any
    // key dismisses it — so without this the fixture's caller loses its
    // first keypress to a screen it never asked for.
    dismiss_notifications(app);
    stand_in_base(app);
}

/// Home declares no upgrade path, so a base consisting only of one leaves
/// nothing to upgrade — and the base menu now says so by not offering the
/// row at all, rather than opening a picker with no entries in it.
#[test]
fn a_structure_with_no_upgrade_path_hides_the_upgrade_row() {
    let mut app = test_app(231);
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

/// Guards Finding 3: `handle_build_direction_key` must recognise Home by
/// looking up the picked structure's real `StructureDef::category()`, not
/// by comparing its id against a hand-copied `"home"` literal that happens
/// to match today. A fresh run owns zero tamed programs, so if Home were
/// ever misrouted into `Mode::BuildProgram` it would land on a picker with
/// nothing in it — no key would confirm anything, and a first base could
/// never be founded. `deploy_home` (used throughout this file) already
/// relies on Home bypassing the picker, but only checks the *outcome*
/// (`structure_count == 1`) after `dismiss_notifications` has run; this
/// asserts the *mode* directly, one key after the direction press and
/// before anything else can intervene.
#[test]
fn founding_a_home_bypasses_the_program_picker() {
    let mut app = test_app(879);
    open_via_menu(&mut app, 'b', "Deploy a structure");
    app.handle_key(GameKey::Enter); // Home sorts first — see `StructureDef::category`
    assert_eq!(app.mode, Mode::BuildDirection);

    app.handle_key(GameKey::Up);
    // Not `Mode::Playing` yet — founding fires the base tutorial, which
    // takes the screen (see `deploy_home`'s own comment on this). What
    // matters here is that it is anything *but* `Mode::BuildProgram`: that
    // would mean Home got routed into the picker instead of committing on
    // the spot.
    assert_ne!(
        app.mode,
        Mode::BuildProgram,
        "Home commits directly rather than detouring through the picker"
    );
    dismiss_notifications(&mut app);
    assert_eq!(app.mode, Mode::Playing, "back on the map once dismissed");
    assert_eq!(structure_count(&mut app), 1, "the Home actually got built");
}

/// How many structures are deployed, from the roster rather than from a
/// scan around the party: the roster is the whole base whichever locale it
/// is asked from, and a scan centred on the party would answer this
/// differently depending on where they happen to be standing.
fn structure_count(app: &mut App) -> usize {
    app.game.as_mut().unwrap().structure_report().len()
}

#[test]
fn build_menu_number_key_reaches_the_direction_picker_and_can_place_a_structure() {
    let mut app = test_app(101);
    assert!(app.game.is_some(), "test game should have loaded");
    assert!(app.mode == Mode::Playing);
    // From the open grid: the run has no base yet, and founding one is the
    // deploy that opens it.

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
            // Deploying the Home is one of the placements this walks, and
            // that fires the base tutorial.
            dismiss_notifications(&mut app);
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
    stand_in_base(&mut app);
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
    // One cell south of the Home, which stands on the exit cell the fixture
    // otherwise puts the party on: the direct key aims at a *neighbour*, and
    // the tile you are standing on is not one.
    stand_in_base_at(&mut app, 0, 1);

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

/// Every structure stands in base space, so a direction key pressed anywhere
/// else aims at four tiles with nothing of the player's on them. Refused at
/// the keypress, matching the `base_only` flag the menu's Demolish row
/// carries and `Game::remove_structure`'s own `require_base`.
///
/// The permitted half is `the_demolish_key_says_when_there_is_nothing_that_way`
/// above, which drives the same key from inside the base and reaches the
/// direction prompt.
#[test]
fn the_demolish_key_is_refused_outside_base_space() {
    for (standing, mut app) in [
        ("in the Stack", app_inside_a_small_base(243, true)),
        (
            "on the open grid",
            app_owning_a_program_and_a_compiler(2431, &[]),
        ),
    ] {
        assert!(
            app.game.as_ref().is_some_and(|g| !g.in_base()),
            "precondition: the fixture really is {standing}"
        );

        app.handle_key(GameKey::Char('d'));

        assert_eq!(
            app.mode,
            Mode::Playing,
            "the direction prompt must not even open {standing}"
        );
        assert!(
            app.status_line.is_some(),
            "a refused key says why rather than doing nothing, {standing}"
        );
    }
}

/// Enter on a roster row posts a worker, and both actions behind it —
/// `Game::assign_cronjob` and `Game::work_structure` — are `require_base`.
/// So the keypress is refused anywhere else rather than opening a picker
/// whose every row the engine would turn down.
///
/// The roster itself still reads from any locale, which is why it carries no
/// `base_only` flag: the same fixture opens the screen in both halves below
/// and only the Enter differs.
#[test]
fn enter_on_a_roster_row_is_refused_outside_base_space() {
    let mut outside = app_owning_a_program_and_a_compiler(2432, &[]);
    open_via_menu(&mut outside, 'b', "Structure roster");
    assert_eq!(
        outside.mode,
        Mode::Structures,
        "the roster reads fine from the open grid"
    );

    outside.handle_key(GameKey::Enter);

    assert_eq!(
        outside.mode,
        Mode::Structures,
        "the staffing picker must not open from the open grid"
    );
    assert!(
        outside.status_line.is_some(),
        "a refused key says why rather than doing nothing"
    );

    // The same row, the same key, from the one locale the engine accepts.
    let mut inside = app_owning_a_program_and_a_compiler(2432, &[]);
    stand_in_base(&mut inside);
    open_via_menu(&mut inside, 'b', "Structure roster");
    let workable = inside
        .game
        .as_mut()
        .unwrap()
        .structure_report()
        .iter()
        .position(|r| r.workable)
        .expect("the fixture's Compiler is workable");
    for _ in 0..workable {
        inside.handle_key(GameKey::Down);
    }
    inside.handle_key(GameKey::Enter);
    assert_eq!(
        inside.mode,
        Mode::StructureAssign,
        "the row is workable, so inside the base it must open"
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
    stand_in_base(&mut far);
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

/// The Base Staff screen lists every program you own and says which role it
/// is in. `doing` was `None` for anything off the staff and the renderer read
/// that single `None` as "party" — so a program you had just tamed claimed to
/// be fighting alongside you. There is no longer any such limbo: a program
/// you own and are not fighting with is base staff, waiting for a post.
#[test]
fn the_staff_screen_tells_a_party_member_from_an_unposted_staffer() {
    let mut app = app_owning_distant_programs(742, 2);

    let rows = app.base_staff_rows();
    assert_eq!(rows.len(), 2, "fixture hands the player two programs");
    assert!(
        rows.iter()
            .all(|r| r.role == Some(ProgramRole::Staff) && r.doing == "idle"),
        "a program nobody assigned is staff between postings: {:?}",
        rows.iter().map(|r| r.doing.clone()).collect::<Vec<_>>()
    );

    // Joining a party is a base verb now, so the fixture stands where the
    // player has to stand to do it. The two entities are read back **after**
    // the crossing, not before: it goes through a save round trip and
    // `Entity` identity is private to the `World` that allocated it.
    stand_inside_the_base(&mut app);
    let rows = app.base_staff_rows();
    let (member, other) = (rows[0].program.entity, rows[1].program.entity);
    app.game.as_mut().unwrap().add_companion(member).unwrap();

    let rows = app.base_staff_rows();
    let row = |e| rows.iter().find(|r| r.program.entity == e).unwrap();
    assert_eq!(row(member).role, Some(ProgramRole::InParty));
    assert!(
        row(member).doing.contains("party"),
        "and it says so, got {:?}",
        row(member).doing
    );
    assert_eq!(row(other).role, Some(ProgramRole::Staff));
    assert_eq!(row(other).doing, "idle");
}

/// The screen writes nothing. It used to toggle a stored marker with Enter;
/// the roles are derived now, so the only thing a key here moves is the
/// selection — and Esc still closes.
#[test]
fn the_staff_screen_does_not_change_a_role() {
    let mut app = app_owning_distant_programs(743, 2);
    app.mode = Mode::BaseStaff;
    app.menu_selected = 0;
    let before: Vec<Option<ProgramRole>> = app.base_staff_rows().iter().map(|r| r.role).collect();

    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Enter);

    assert_eq!(
        app.base_staff_rows()
            .iter()
            .map(|r| r.role)
            .collect::<Vec<_>>(),
        before,
        "no key on this screen may move a program between roles"
    );
    assert_eq!(app.menu_selected, 1, "but Down still moves the selection");
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

/// The one key slice 2 adds to the map screen, driven end to end: swing at
/// the pocket's edge until it opens, step into it, and lay a tile on it.
///
/// `v` for the tile's own name — `t` is trade, and `T` belongs to two
/// battle screens nothing may document (`crates/engine/EASTER_EGGS.md`).
/// The surface half is asserted in the same test because the base half alone
/// passes against a key that fires everywhere — and out there the party's
/// `Position` is a tile on the zone map, with no cell of base space under it
/// at all.
#[test]
fn v_lays_a_tile_in_base_space_and_does_nothing_on_the_surface() {
    use feral_processes_engine::tuning::STARTING_POCKET_RADIUS;
    use feral_processes_engine::world::Biome;

    let mut app =
        app_owning_a_program_and_a_compiler_with_cargo(750, &[], &[("blank_substrate", 1)]);
    let edge = (STARTING_POCKET_RADIUS, 0);
    let cut = (STARTING_POCKET_RADIUS + 1, 0);
    stand_in_base_at(&mut app, edge.0, edge.1);

    // The bump only cuts once the tool is armed — `MiningMode` starts off so
    // that navigating a base cannot demolish it. Without this the loop below
    // bounces off the wall twenty times and the failure reads as the dig
    // being broken.
    app.handle_key(GameKey::Char('n'));

    // However many swings the rock's durability implies, and then the step
    // onto what they opened. Bounded rather than counted: what this test is
    // about is the key, not the wall.
    for _ in 0..20 {
        app.handle_key(GameKey::Char('l'));
        if app.game.as_ref().unwrap().base_pos() == Some(cut) {
            break;
        }
    }
    assert_eq!(
        app.game.as_ref().unwrap().base_pos(),
        Some(cut),
        "the fixture must dig through and stand on the cut cell"
    );
    let under_the_party = |app: &mut App| {
        let tiles = app.game.as_mut().unwrap().view_tiles(1, 1);
        tiles[1][1].biome
    };
    assert_eq!(
        under_the_party(&mut app),
        Biome::Excavated,
        "the cut cell must start unfloored, or the tiling below proves nothing"
    );

    app.handle_key(GameKey::Char('v'));

    assert_eq!(
        under_the_party(&mut app),
        Biome::Platform,
        "v on carved rock lays a VectorStasis Tile over it"
    );

    let mut outside =
        app_owning_a_program_and_a_compiler_with_cargo(751, &[], &[("blank_substrate", 1)]);
    let tick = outside.game.as_ref().unwrap().current_tick();
    outside.handle_key(GameKey::Char('v'));
    assert_eq!(
        outside.game.as_ref().unwrap().current_tick(),
        tick,
        "v on the open grid must spend nothing — there is no cell under you"
    );
    assert_eq!(outside.mode, Mode::Playing, "and must open no screen");
}

/// Opens the work order screen from the base menu and walks it to the
/// quantity page with the first orderable item pending — the two-page shape
/// the compile flow uses, one screen over.
fn open_order_quantity_page(app: &mut App) {
    open_via_menu(app, 'b', "Work orders");
    assert_eq!(app.mode, Mode::WorkOrders);
    // The trailing row is the one that queues another order.
    let last = app.work_order_rows().len() - 1;
    app.handle_key(GameKey::Char(menu_shortcut(last)));
    assert_eq!(app.mode, Mode::WorkOrderPick);
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::WorkOrderQuantity);
}

/// "Make me three" and "always hold three" are different errands, and the
/// toggle is where the player says which one this is.
#[test]
fn the_standing_toggle_files_an_order_that_holds_a_level() {
    let mut app = app_inside_a_small_base(252, false);
    open_order_quantity_page(&mut app);
    assert!(!app.standing_order, "an order is a batch unless asked");

    app.handle_key(GameKey::Char('s'));
    assert!(app.standing_order, "[S] turns it on");
    app.handle_key(GameKey::Char('s'));
    assert!(!app.standing_order, "and off again");

    app.handle_key(GameKey::Char('s'));
    app.handle_key(GameKey::Char('3'));
    app.handle_key(GameKey::Enter);

    assert_eq!(app.mode, Mode::WorkOrders);
    let orders = app.game.as_ref().unwrap().work_orders();
    assert_eq!(orders.len(), 1, "the order is filed");
    assert_eq!(orders[0].qty, 3);
    assert!(
        orders[0].standing,
        "and carries the flag the page was showing"
    );
}

/// The flag is the page's and must not outlive its order. One that survived
/// would turn the next batch into a level the base holds forever, on a
/// screen that had gone back to saying nothing about it — the leak
/// `careful_craft` is guarded against one screen over.
#[test]
fn the_standing_flag_does_not_outlive_its_order() {
    let mut app = app_inside_a_small_base(253, false);
    open_order_quantity_page(&mut app);
    app.handle_key(GameKey::Char('s'));
    assert!(app.standing_order, "precondition: the flag is set");

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::WorkOrderPick, "Esc backs into the picker");
    app.handle_key(GameKey::Char('1'));

    assert_eq!(app.mode, Mode::WorkOrderQuantity);
    assert!(!app.standing_order, "and a fresh order opens as a batch");
}

/// `[P]` raises first, because raising is what the feature is for: before
/// bands the only control over the base's attention was cancel-and-refile,
/// which lands the order you care about at the bottom.
#[test]
fn the_priority_key_cycles_the_band_and_the_filed_order_carries_it() {
    let mut app = app_inside_a_small_base(254, false);
    open_order_quantity_page(&mut app);
    assert_eq!(
        app.order_priority,
        OrderPriority::Normal,
        "an order is ordinary unless asked"
    );

    app.handle_key(GameKey::Char('p'));
    assert_eq!(app.order_priority, OrderPriority::High, "[P] raises");
    app.handle_key(GameKey::Char('p'));
    assert_eq!(
        app.order_priority,
        OrderPriority::Low,
        "and wraps past the top"
    );
    app.handle_key(GameKey::Char('p'));
    assert_eq!(
        app.order_priority,
        OrderPriority::Normal,
        "and back round to where it started"
    );

    app.handle_key(GameKey::Char('p'));
    app.handle_key(GameKey::Char('3'));
    app.handle_key(GameKey::Enter);

    let orders = app.game.as_ref().unwrap().work_orders();
    assert_eq!(orders.len(), 1, "the order is filed");
    assert_eq!(
        orders[0].priority,
        OrderPriority::High,
        "and carries the band the page was showing"
    );
}

/// The band is the page's, like the standing flag beside it: a High left
/// set would jump the queue with an order nobody asked to prioritise.
#[test]
fn the_priority_band_does_not_outlive_its_order() {
    let mut app = app_inside_a_small_base(255, false);
    open_order_quantity_page(&mut app);
    app.handle_key(GameKey::Char('p'));
    assert_eq!(
        app.order_priority,
        OrderPriority::High,
        "precondition: the band is raised"
    );

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::WorkOrderPick);
    app.handle_key(GameKey::Char('1'));

    assert_eq!(app.mode, Mode::WorkOrderQuantity);
    assert_eq!(
        app.order_priority,
        OrderPriority::Normal,
        "and a fresh order opens ordinary"
    );
}

/// Row 0 of the build menu is always Home (`StructureCategory::Home` sorts
/// ahead of every other category, see `StructureDef::category`), and this
/// fixture has already founded one — picking it again would hit the
/// "already deployed" refusal on the old direct path rather than the
/// picker this task adds. Row 1, `mining_node`, is the first structure that
/// actually costs a program, and `selected_index` labels rows `1`-`9`
/// before it starts spending letters (`DIGIT_ROWS`), so row 1 is key `'2'`,
/// not `'b'`.
fn deploy_second_structure(app: &mut App) {
    app.mode = Mode::Build;
    app.handle_key(GameKey::Char('2'));
    app.handle_key(GameKey::Right);
}

/// Opens the build menu and picks the row for `id`, leaving the app on
/// `Mode::BuildDirection`.
///
/// Down-and-Enter rather than a row shortcut, because a row's shortcut *is*
/// its position and every structure dropped into `assets/structures/`
/// renumbers whatever sorts after it.
fn choose_build_row(app: &mut App, id: &str) {
    open_via_menu(app, 'b', "Deploy a structure");
    let idx = app
        .game
        .as_ref()
        .expect("a running game")
        .buildable_structure_defs()
        .iter()
        .position(|def| def.id == id)
        .unwrap_or_else(|| panic!("{id} is not offered in the build menu"));
    for _ in 0..idx {
        app.handle_key(GameKey::Down);
    }
    app.handle_key(GameKey::Enter);
    assert_eq!(
        app.mode,
        Mode::BuildDirection,
        "the row for {id} was picked"
    );
}

/// A Depot costs no program — `Game::structure_needs_program` exempts any
/// structure that declares `costs_no_program` — so picking a direction for
/// one files the order on the spot instead of opening a picker.
///
/// **Routed off the engine's own answer, not a second derivation here.**
/// `founding_a_home_bypasses_the_program_picker` guards the same seam from
/// the Home's side; what this adds is that the exemption is no longer a
/// category the frontend can recognise by itself.
#[test]
fn deploying_a_depot_bypasses_the_program_picker() {
    let mut app = app_in_base_with_programs(873, 2);
    let held = app.game.as_mut().unwrap().owned_pets().len();

    choose_build_row(&mut app, "depot");
    app.handle_key(GameKey::Right);

    assert_ne!(
        app.mode,
        Mode::BuildProgram,
        "a Depot commits directly rather than detouring through the picker"
    );
    assert!(
        app.game
            .as_mut()
            .unwrap()
            .adjacent_build_site(1, 0)
            .is_some(),
        "and the order is filed by the direction press alone"
    );
    assert_eq!(
        app.game.as_mut().unwrap().owned_pets().len(),
        held,
        "with nobody spent on it"
    );
}

#[test]
fn choosing_a_direction_asks_which_program_to_spend() {
    let mut app = app_in_base_with_programs(870, 2);
    deploy_second_structure(&mut app);
    assert_eq!(
        app.mode,
        Mode::BuildProgram,
        "the picker is the confirm step"
    );
    assert!(
        app.game
            .as_mut()
            .unwrap()
            .adjacent_build_site(1, 0)
            .is_none(),
        "nothing is filed until a program is picked"
    );
}

#[test]
fn picking_a_program_files_the_order() {
    let mut app = app_in_base_with_programs(871, 2);
    deploy_second_structure(&mut app);
    app.handle_key(GameKey::Char('1')); // first program
    assert_eq!(app.mode, Mode::Playing);
    assert!(
        app.game
            .as_mut()
            .unwrap()
            .adjacent_build_site(1, 0)
            .is_some(),
        "the order is filed once a program is picked"
    );
}

#[test]
fn escaping_the_picker_files_nothing_and_spends_nothing() {
    let mut app = app_in_base_with_programs(872, 2);
    deploy_second_structure(&mut app);
    assert_eq!(
        app.mode,
        Mode::BuildProgram,
        "precondition: the picker is actually up before Esc is asked to leave it"
    );
    app.handle_key(GameKey::Esc);
    assert!(app.pending_build.is_none());
    assert!(
        app.game
            .as_mut()
            .unwrap()
            .adjacent_build_site(1, 0)
            .is_none(),
        "escaping files nothing"
    );
    // R6: a one-program base can never file an order at all — Task 6's
    // refusal against leaving the base at zero — so the count that matters
    // here is 2, not 1: nothing was spent picking a direction, only picking
    // a program spends.
    assert_eq!(
        app.game.as_mut().unwrap().owned_pets().len(),
        2,
        "nothing is spent until the picker confirms"
    );
}

#[test]
fn the_picker_lists_only_programs_deep_enough() {
    let mut app = app_in_base(873); // no programs yet
    tame_program_at_zone(&mut app, 1);
    tame_program_at_zone(&mut app, 3);
    let game = app.game.as_mut().unwrap();
    assert_eq!(game.programs_for_build(3).len(), 1, "only the deep one");
}

/// Proves `App::report` is wired to the picker's confirm rather than the
/// outcome being dropped on the floor: the "Things to get right" bullet
/// that a refusal after confirm must surface, not fail silently.
///
/// **Re-pointed at the occupied-cell refusal.** This test used to reach the
/// last-program rule: one program on the roster, offered by
/// `programs_for_build(1)` because that derivation had nothing to say about
/// how many were left over, and refused only once the player confirmed. The
/// R27 fix folded the roster floor into `programs_for_build` precisely so
/// that no longer happens, which leaves a one-program base with an *empty*
/// picker — a row key selects nothing, the mode never resolves, and the
/// test would be asserting on a screen the player cannot get past rather
/// than on a refusal.
///
/// So it files a real order first and then tries to deploy onto the same
/// cell. That is still an engine `Err` raised at exactly the moment this
/// test is about — after the program is picked, inside `place_structure` —
/// and it is now the reachable one: with the floor and the depth both
/// filtered before the list is drawn, every remaining `commit_for_build`
/// refusal is defence in depth for callers that never drew a picker (the
/// engine's own `a_one_program_base_may_not_spend_its_only_body` and the
/// `committing_a_*_program_is_refused` tests hold those directly).
///
/// Three programs, because the first order spends one and a base that then
/// held only one would open the second picker empty.
#[test]
fn the_pickers_confirm_surfaces_the_engines_refusal() {
    let mut app = app_in_base_with_programs(874, 3);
    deploy_second_structure(&mut app);
    assert_eq!(app.mode, Mode::BuildProgram);
    app.handle_key(GameKey::Char('1')); // files the order, spends one
    assert_eq!(app.mode, Mode::Playing);
    assert!(
        app.game
            .as_mut()
            .unwrap()
            .adjacent_build_site(1, 0)
            .is_some(),
        "precondition: a request now stands on the cell the next deploy wants"
    );

    deploy_second_structure(&mut app);
    assert_eq!(
        app.mode,
        Mode::BuildProgram,
        "precondition: the picker is up, so the refusal below is one the confirm raised"
    );
    app.handle_key(GameKey::Char('1'));
    assert_eq!(
        app.mode,
        Mode::Playing,
        "the picker always resolves back to the map, refused or not"
    );
    let status = app.status_line.clone().unwrap_or_default();
    assert!(
        status.contains("already set to build something there"),
        "the engine's refusal must reach the player, got: {status:?}"
    );
    assert_eq!(
        app.game.as_mut().unwrap().owned_pets().len(),
        2,
        "and a refusal spends nothing — only the first order's program went"
    );
}

/// `Mode::Upgrade`'s counterpart to `deploy_second_structure` — picks the
/// one upgradeable structure the fixture offers, the Compiler, through the
/// base menu rather than by hand-setting `app.mode`, since that menu path
/// is also what proves `Mode::Upgrade` is still reachable at all.
fn open_upgrade_picker(app: &mut App) {
    open_via_menu(app, 'b', "Upgrade a structure");
    assert_eq!(app.mode, Mode::Upgrade);
    app.handle_key(GameKey::Char('1')); // the only row: the Compiler
}

#[test]
fn upgrading_a_structure_asks_which_program_to_spend() {
    let mut app = app_owning_a_program_and_a_compiler(875, &[]);
    stand_in_base(&mut app);
    // A second program: R6's zero-program refusal is `deploy`'s test to
    // make, not this one's — this test is about the mode transition and
    // `to_tier`, and a refusal on confirm would leave the mode this test
    // asserts on ambiguous between "reached the picker" and "the picker's
    // own pick did nothing".
    //
    // Caught at zone 2, not 1: `to_tier` is 2 (Mk1 upgrades to Mk2), and
    // `programs_for_build(2)`'s `>=` floor means a zone-1 program would
    // leave this picker with zero rows — a row key could then press
    // anything and confirm nothing, which is exactly the "unreachable
    // upgrade arm" the review caught (see `open_upgrade_picker`'s callers
    // below, which do press a row).
    tame_program_at_zone(&mut app, 2);

    open_upgrade_picker(&mut app);
    assert_eq!(
        app.mode,
        Mode::BuildProgram,
        "the picker is the confirm step"
    );
    match app.pending_build {
        Some(PendingBuild::Upgrade { to_tier, .. }) => {
            assert_eq!(to_tier, 2, "Mk1 upgrades to Mk2 — current tier + 1")
        }
        _ => panic!("expected an Upgrade order pending confirmation"),
    }
}

#[test]
fn escaping_the_upgrade_picker_files_nothing_and_spends_nothing() {
    let mut app = app_owning_a_program_and_a_compiler(876, &[]);
    stand_in_base(&mut app);
    // Zone 2, `upgrading_a_structure_asks_which_program_to_spend`'s reason:
    // a zone-1 program is not deep enough for `programs_for_build(2)`, and
    // this test needs the picker to actually be showing a row for its own
    // precondition assertion to mean anything.
    tame_program_at_zone(&mut app, 2);

    open_upgrade_picker(&mut app);
    assert_eq!(
        app.mode,
        Mode::BuildProgram,
        "precondition: the picker is actually up before Esc is asked to leave it"
    );
    app.handle_key(GameKey::Esc);
    assert!(app.pending_build.is_none());
    assert_eq!(
        app.game.as_mut().unwrap().owned_pets().len(),
        2,
        "nothing is spent until the picker confirms"
    );
}

/// The upgrade counterpart to `the_pickers_confirm_surfaces_the_engines_refusal`
/// — the deploy version already proved `App::report` is wired to a refusal
/// after confirm, but the review that found this task's gaps pointed out
/// that proof was one-sided: neither existing upgrade test presses a program
/// key at all (`upgrading_a_structure_asks_which_program_to_spend` stops at
/// the mode assertion, and the Esc test above never confirms), so the
/// `PendingBuild::Upgrade` arm of `App::handle_build_program_key` — the
/// branch that calls `Game::upgrade_structure` rather than
/// `place_structure` — was exercised by nothing.
///
/// Built on `app_owning_one_deep_program_and_a_compiler` rather than
/// `app_owning_a_program_and_a_compiler`: this needs the base breached to
/// zone 2 (`Game::warp_to_zone`, the same real breach
/// `tests::achievements::breach_and_tick` drives) so `upgrade_ceiling`
/// admits a Mk2 order instead of refusing it first — every fixture that
/// stays at zone 1 caps every structure at Mk1, which is what left the last
/// review unable to drive a real upgrade confirm at all (see
/// `a_structure_at_its_zone_ceiling_is_still_listed_with_the_ceiling_shown`).
/// **Re-pointed alongside the deploy version above**, and for that test's
/// reason: it used to hold exactly one program so committing it hit the
/// "last program" rule, and R27 has since folded that rule into
/// `programs_for_build`, so a one-program base now opens this picker with
/// no rows at all and a row key confirms nothing. The refusal it reaches
/// instead is `upgrade_structure`'s standing-request check — still an
/// engine `Err`, still raised only after the program is picked, and still
/// the one thing this test is about: that the whole `Upgrade` arm runs
/// (candidates listed, row picked, entity passed to `upgrade_structure`,
/// the `Err` routed through `self.report`) without needing a structure to
/// actually finish upgrading.
///
/// Three programs at zone 2, because the first order spends one and a base
/// left holding one would open the second picker empty for the very rule
/// this test can no longer use.
#[test]
fn the_upgrade_pickers_confirm_surfaces_the_engines_refusal() {
    let mut app = app_owning_one_deep_program_and_a_compiler(877, 2, 2);
    stand_in_base(&mut app);
    tame_program_at_zone(&mut app, 2);
    tame_program_at_zone(&mut app, 2);

    open_upgrade_picker(&mut app);
    app.handle_key(GameKey::Char('1')); // files the upgrade order, spends one
    // The filed order's own tick pops a notification on this fixture, which
    // would otherwise take the screen and leave every assertion below
    // reading `Mode::Notification` for a reason this test never named.
    dismiss_notifications(&mut app);
    assert_eq!(app.mode, Mode::Playing, "precondition: the order was filed");

    open_upgrade_picker(&mut app);
    assert_eq!(
        app.mode,
        Mode::BuildProgram,
        "precondition: the picker is up"
    );

    app.handle_key(GameKey::Char('1'));
    assert_eq!(
        app.mode,
        Mode::Playing,
        "the picker always resolves back to the map, refused or not"
    );
    let status = app.status_line.clone().unwrap_or_default();
    assert!(
        status.contains("already on order"),
        "the engine's refusal must reach the player, got: {status:?}"
    );
    let compiler_tier = app
        .upgradeable_structures()
        .iter()
        .find(|e| e.label.contains("Compiler"))
        .and_then(|e| e.tier);
    assert_eq!(
        compiler_tier,
        Some(1),
        "a refused upgrade leaves the structure's tier untouched"
    );
    assert_eq!(
        app.game.as_mut().unwrap().owned_pets().len(),
        2,
        "and a refusal spends nothing — only the first order's program went"
    );
}

/// Guards Finding 2: `handle_build_program_key` must hand `programs_for_build`
/// the engine's own `program_tier_required(goal)`, not a restated copy of
/// its `New => 1, Upgrade { to_tier } => to_tier` formula. A copy is a risk
/// with no other test to catch it — `Game::upgrade_structure` independently
/// re-derives the depth requirement from the structure's own current tier
/// at commit time, so whichever program the picker actually spends is
/// re-checked there regardless of what app-core offered. The only place a
/// drift shows up is the picker's own candidate *list*: a tier derived as 1
/// instead of 2 would offer a program too shallow for the Mk2 this order
/// is, and a tier derived as (say) 3 would hide one that would have worked
/// — either way silently, since nothing here surfaces "the picker showed
/// the wrong rows".
///
/// Two programs, one at each side of the `to_tier == 2` floor: if the
/// picker is built on `programs_for_build(2)`, only the zone-2 program is a
/// row and there is nothing for a second key press to select. A tier
/// miscomputed as 1 would list both instead — which program actually lands
/// on which row is `programs_for_build`'s own ordering and not this test's
/// business; what this test polices is the *count*, since a second row
/// existing at all is the bug, whichever entity it happens to name. See the
/// mutation proof in the task-8 fix report.
#[test]
fn upgrade_confirm_only_lists_programs_at_the_engines_own_depth() {
    let mut app = app_owning_one_deep_program_and_a_compiler(878, 2, 2);
    stand_in_base(&mut app);
    // The order `app_in_base_with_programs` uses: `stand_in_base` before
    // `tame_program_at_zone`, so the added creature's save round trip finds
    // the party already in base space rather than clobbering that locale.
    tame_program_at_zone(&mut app, 1); // too shallow for a Mk2

    open_upgrade_picker(&mut app);
    assert_eq!(
        app.mode,
        Mode::BuildProgram,
        "precondition: the picker is up"
    );

    app.handle_key(GameKey::Char('2')); // a row past the true candidate count
    assert_eq!(
        app.mode,
        Mode::BuildProgram,
        "only one program is deep enough for a Mk2 — a second row must not exist, \
         so a second key press has nothing to select and confirms nothing"
    );
}

/// `pending_build_kind` answers with the machine standing there, not with a
/// deploy's def id — the two orders name their structure from different
/// places, and the picker asks `build_candidates` about the answer.
#[test]
fn an_upgrade_picker_asks_about_the_structure_standing_there() {
    let mut app = app_owning_one_deep_program_and_a_compiler(879, 2, 2);
    stand_in_base(&mut app);
    tame_program_at_zone(&mut app, 2);

    open_upgrade_picker(&mut app);
    assert_eq!(
        app.mode,
        Mode::BuildProgram,
        "precondition: the picker is up"
    );

    assert_eq!(
        app.pending_build_kind().as_deref(),
        Some("compiler"),
        "an upgrade must name the machine it is standing in front of"
    );
}

/// The handler indexes the *sorted* list gui draws, so the shortcut spends
/// the program on the row the player actually read.
///
/// The roster is staged so `owned_pets`' order and `build_candidates`' order
/// differ: `build_candidates` sorts best-first by the roll a Compiler
/// upgrade reads (assembly, since a Compiler assembles), and the fixture
/// gives the *second* program the better roll.
#[test]
fn the_picker_spends_the_program_on_the_row_the_player_read() {
    // The fixture's own program builds badly and the added one builds well,
    // so `build_candidates`' order is the reverse of `owned_pets`'.
    let mut app = app_owning_one_deep_program_and_a_compiler(880, 2, 2);
    stand_in_base(&mut app);
    tame_program_at_zone_with_build_rolls(&mut app, 2, 1.18, 1.0);

    let (owned_first, top) = {
        let game = app.game.as_mut().unwrap();
        let pets = game.owned_pets();
        assert_eq!(pets.len(), 2, "the fixture stages exactly two programs");
        let kind = "compiler".to_string();
        (
            pets[0].entity,
            game.build_candidates(&kind, BuildGoal::Upgrade { to_tier: 2 })[0]
                .pet
                .entity,
        )
    };
    assert_ne!(
        top, owned_first,
        "precondition: the sorted list and owned_pets' order must actually differ, \
         or the test passes against a handler indexing either one"
    );

    open_upgrade_picker(&mut app);
    app.handle_key(GameKey::Char('1'));

    let survivors: Vec<_> = app
        .game
        .as_mut()
        .unwrap()
        .owned_pets()
        .into_iter()
        .map(|p| p.entity)
        .collect();
    assert!(
        !survivors.contains(&top),
        "row 1 named the better builder, so that is the program that must have been spent"
    );
}
