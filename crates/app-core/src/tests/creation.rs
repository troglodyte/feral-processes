//! The character-creation wizard — `Mode::CreateCharacter` and its eight
//! steps.
//!
//! `the_summary_step_commits_the_choice` is the load-bearing one: it walks
//! the whole wizard by keypress and then reads the *saved* run back off
//! disk, so every choice has to survive the whole path from a keystroke to
//! `Game::new_with` to `PlayerSave`.

use super::support::*;
use crate::*;
use feral_processes_engine::PlayerIcon;
use feral_processes_engine::achievements::{AchievementId, Earned, roll_main_stat};
use feral_processes_engine::classes::PlayerClass;
use feral_processes_engine::save;
use feral_processes_engine::tuning::CREATION_PERK_POINTS;
use feral_processes_engine::tuning::{
    CREATION_COST_DEF, CREATION_CREDITS, CREATION_GAIN_INTEGRITY, CREATION_STAT_POINTS,
    PLAYER_BASE_STATS,
};

/// An `App` sitting on the main menu with no run, its own scratch saves
/// directory and profile — the wizard writes a save the moment it commits,
/// and two tests sharing a directory would see each other's.
fn wizard_app(name: &str) -> App {
    let assets_dir = test_assets_dir();
    let root = std::env::temp_dir().join(format!(
        "feral_processes_creation_{name}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    // The saves directory has to exist before the wizard commits: the
    // first thing a new run does is claim a slot, and `Game::save` into a
    // missing directory fails silently into the status line. Without this
    // `esc_from_the_first_step_does_not_start_a_run`'s save assertion would
    // be vacuous.
    std::fs::create_dir_all(root.join("saves")).unwrap();
    App::new(
        assets_dir,
        root.join("saves"),
        root.join("history.log"),
        root.join("profile.ron"),
        arenas_dir(),
        root.join("telemetry.jsonl"),
    )
}

fn press(app: &mut App, key: GameKey) {
    app.handle_key(key);
}

fn ch(c: char) -> GameKey {
    GameKey::Char(c)
}

/// Dismisses whatever the freshly-started run put on screen — a new run
/// queues the onboarding chain's first briefing, which takes `Mode::
/// Notification` the moment the wizard commits. Every engine and app-core
/// fixture drains it the same way; a test about the briefing queues its
/// own.
fn settle(app: &mut App) {
    while app.mode == Mode::Notification {
        press(app, GameKey::Esc);
    }
}

/// Walks past both halves of the look without deciding either — `[n]`
/// twice, since the icons and the swatches are two steps.
///
/// Named rather than inlined because six tests only want to *reach* the
/// Points step: with the walk spelled out at each of them, splitting a step
/// in two is six edits and the one that gets missed fails somewhere that
/// has nothing to do with the look.
fn skip_the_look(app: &mut App) {
    press(app, ch('n'));
    press(app, ch('n'));
}

/// Spends the Kit step's whole allowance — which is now what leaving that
/// step costs. One `ShiftRight` pass over every row leaves each at its own
/// ceiling given the others, so nothing is affordable afterwards, which is
/// exactly the question `leave_refusal` asks.
fn spend_the_kit(app: &mut App) {
    for i in 0..app.creation_rows().len() {
        app.menu_selected = i;
        press(app, GameKey::ShiftRight);
    }
    app.menu_selected = 0;
}

/// `spend_the_kit` for the Perks step, whose allowance is spent here or
/// lost the same way.
fn spend_the_perks(app: &mut App) {
    for i in 0..app.creation_rows().len() {
        app.menu_selected = i;
        press(app, GameKey::ShiftRight);
    }
    app.menu_selected = 0;
}

/// `spend_the_kit` for the Points step's four axes.
fn spend_the_points(app: &mut App) {
    for i in 0..MainStat::all().len() {
        app.menu_selected = i;
        press(app, GameKey::ShiftRight);
    }
    app.menu_selected = 0;
}

/// Walks from the Kit step to the Summary, answering each step the
/// cheapest legal way: the allowance spent, the look skipped, the pool
/// spent, no starter routine. Named for the reason `skip_the_look` is —
/// six tests only want to *arrive* somewhere later.
fn walk_to_the_summary(app: &mut App) {
    spend_the_kit(app);
    press(app, GameKey::Enter);
    skip_the_look(app);
    spend_the_points(app);
    press(app, GameKey::Enter);
    press(app, GameKey::Enter); // the perks step, allowance kept
    press(app, ch('n'));
}

/// Opens the wizard and picks Forgiving, leaving it on the Class step.
///
/// The profile summary sits between the two and has nothing to decide, so
/// Enter walks it — spelled here rather than at each of the twenty-odd
/// callers that only want to *reach* a later step.
fn opened(name: &str) -> App {
    let mut app = wizard_app(name);
    press(&mut app, ch('n'));
    press(&mut app, ch('f'));
    press(&mut app, GameKey::Enter);
    app
}

/// The saved run the wizard just started, read back off a real file.
fn saved_run(app: &mut App, name: &str) -> save::SaveData {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_creation_read_{name}_{}.sav",
        std::process::id()
    ));
    app.game.as_mut().unwrap().save(&path).unwrap();
    let data = save::load_from_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    data
}

/// `CreationStep::ALL` is the exhaustive list every walk of the wizard
/// reads — `next`, `prev`, the row builder and the renderer's per-step
/// draw. A step left out of it is a step nothing can reach, so this is the
/// one place that can be checked.
#[test]
fn every_step_is_in_the_exhaustive_list() {
    let all = CreationStep::ALL;
    for (i, step) in all.iter().enumerate() {
        assert_eq!(step.index(), i, "{step:?} is not at its own position");
        assert_eq!(step.next(), all.get(i + 1).copied());
        assert_eq!(step.prev(), i.checked_sub(1).map(|j| all[j]));
        assert!(!step.title().is_empty(), "{step:?} has no heading");
    }
    assert_eq!(all.first().copied(), Some(CreationStep::Difficulty));
    // **The name is asked for last, after the summary is accepted** — the
    // summary is the last screen with a decision on it, and naming what you
    // have just agreed to is the run's first act rather than one more field
    // to fill in ahead of seeing it.
    assert_eq!(all.last().copied(), Some(CreationStep::Name));
}

/// Every step draws something. A step with no rows is a blank popup the
/// player cannot tell from a broken screen — and against the real
/// `assets/`, "no rows" would mean the class or routine catalogue silently
/// failed to load.
#[test]
fn every_step_has_rows_to_draw() {
    let mut app = opened("rows");
    for step in CreationStep::ALL {
        app.creation_step = step;
        assert!(
            !app.creation_rows().is_empty(),
            "{step:?} draws nothing at all"
        );
    }
}

#[test]
fn the_wizard_walks_forward_and_back() {
    let mut app = wizard_app("walk");
    press(&mut app, ch('n'));
    assert_eq!(app.mode, Mode::CreateCharacter);
    assert_eq!(app.creation_step(), CreationStep::Difficulty);

    press(&mut app, ch('f'));
    assert_eq!(app.creation_step(), CreationStep::Profile);
    press(&mut app, GameKey::Enter);
    assert_eq!(app.creation_step(), CreationStep::Class);
    press(&mut app, ch('1'));
    assert_eq!(app.creation_step(), CreationStep::Kit);
    press(&mut app, GameKey::Enter);
    assert_eq!(
        app.creation_step(),
        CreationStep::Kit,
        "a spendable allowance holds the step"
    );
    spend_the_kit(&mut app);
    press(&mut app, GameKey::Enter);
    assert_eq!(app.creation_step(), CreationStep::Icon);
    press(&mut app, ch('n'));
    assert_eq!(app.creation_step(), CreationStep::Colour);
    press(&mut app, ch('n'));
    assert_eq!(app.creation_step(), CreationStep::Points);
    spend_the_points(&mut app);
    press(&mut app, GameKey::Enter);
    assert_eq!(app.creation_step(), CreationStep::Perks);
    spend_the_perks(&mut app);
    press(&mut app, GameKey::Enter);
    assert_eq!(app.creation_step(), CreationStep::Routine);
    press(&mut app, ch('n'));
    assert_eq!(app.creation_step(), CreationStep::Summary);
    press(&mut app, GameKey::Enter);
    assert_eq!(app.creation_step(), CreationStep::Name);

    // Esc walks back one step at a time, all the way to the first.
    for expected in [
        CreationStep::Summary,
        CreationStep::Routine,
        CreationStep::Perks,
        CreationStep::Points,
        CreationStep::Colour,
        CreationStep::Icon,
        CreationStep::Kit,
        CreationStep::Class,
        CreationStep::Profile,
        CreationStep::Difficulty,
    ] {
        press(&mut app, GameKey::Esc);
        assert_eq!(app.creation_step(), expected);
    }
    assert_eq!(app.mode, Mode::CreateCharacter, "the wizard is still open");

    press(&mut app, GameKey::Esc);
    assert_eq!(app.mode, Mode::MainMenu, "Esc on the first step leaves");
}

/// Backing out of the wizard must not have started anything — no `Game`,
/// and no save file claimed for a run that was never begun.
#[test]
fn esc_from_the_first_step_does_not_start_a_run() {
    let mut app = wizard_app("abandon");
    press(&mut app, ch('n'));
    press(&mut app, GameKey::Esc);
    assert_eq!(app.mode, Mode::MainMenu);
    assert!(app.game.is_none(), "backing out started a run");
    assert!(app.list_saves().is_empty(), "backing out claimed a save");
}

/// Every choice, by keypress, read back off the saved run. The whole path:
/// a keystroke, `CharacterChoice`, `Game::new_with`, `PlayerSave`.
#[test]
fn the_name_step_commits_the_choice() {
    let mut app = opened("commit");
    let classes = app.creation_catalogue.class_rows();
    let wanted_class = classes[1].class;
    press(&mut app, ch('2')); // the second class
    spend_the_kit(&mut app); // the step will not be left with Credits in hand
    press(&mut app, GameKey::Enter);

    // Icon and colour are two steps now, and a pick advances off each.
    press(&mut app, ch(menu_shortcut(1))); // the second icon
    press(&mut app, ch(menu_shortcut(2))); // the third swatch

    // The step opens on a rolled spread that already spends the pool —
    // clear it so exactly two units of Integrity is this test's own spend,
    // not whatever the roll happened to leave there.
    for i in 0..MainStat::all().len() {
        app.menu_selected = i;
        press(&mut app, GameKey::ShiftLeft);
    }
    let integrity = MainStat::all()
        .iter()
        .position(|s| *s == MainStat::Integrity)
        .unwrap();
    app.menu_selected = integrity;
    press(&mut app, GameKey::ShiftRight);
    let bought = app.creation_choice().stats[integrity];
    press(&mut app, GameKey::Enter);

    assert_eq!(app.creation_step(), CreationStep::Perks);
    spend_the_perks(&mut app);
    press(&mut app, GameKey::Enter);

    let routines = app.creation_catalogue.starter_rows(Some(wanted_class));
    let wanted_routine = routines[0].id.clone();
    press(&mut app, ch('1'));

    assert_eq!(app.creation_step(), CreationStep::Summary);
    press(&mut app, GameKey::Enter);
    assert!(
        app.game.is_none(),
        "the summary is read back and accepted, not the last word"
    );
    assert_eq!(
        app.creation_step(),
        CreationStep::Name,
        "accepting the summary asks for a name"
    );

    for c in "Zephyr".chars() {
        press(&mut app, ch(c));
    }
    press(&mut app, GameKey::Enter);
    assert!(app.game.is_some(), "Enter on the name starts the run");
    settle(&mut app);
    assert_eq!(app.mode, Mode::Playing);

    let data = saved_run(&mut app, "commit");
    assert_eq!(data.player.name, "Zephyr");
    assert_eq!(data.player.class, Some(wanted_class));
    assert_eq!(data.player.glyph, CREATION_ICONS[1].0);
    assert_eq!(data.player.sprite, CREATION_ICONS[1].1);
    assert_eq!(data.player.colour, Some(2));
    assert_eq!(
        data.player.max_hp,
        PLAYER_BASE_STATS.max_hp + (bought * CREATION_GAIN_INTEGRITY) as i32
    );
    assert_eq!(
        data.player.hp, data.player.max_hp,
        "a run must not start damaged"
    );
    assert!(
        data.player.routines.contains(&wanted_routine),
        "the starter routine reached the player: {:?}",
        data.player.routines
    );
    assert_eq!(data.difficulty, DifficultyMode::Forgiving);
}

/// Right on a row with nothing left is refused, through `App::refuse` —
/// the one door, which is what puts the same sentence on the banner and in
/// the log the player scrolls back through.
#[test]
fn points_cannot_be_overspent() {
    let mut app = opened("overspend");
    press(&mut app, ch('1'));
    spend_the_kit(&mut app); // the step will not be left with Credits in hand
    press(&mut app, GameKey::Enter);
    skip_the_look(&mut app);
    assert_eq!(app.creation_step(), CreationStep::Points);

    // The step opens on a rolled spread that already spends the pool —
    // clear it so "fill the first axis, then ask for one more" is this
    // test's own doing rather than a coincidence of whatever was rolled.
    for i in 0..MainStat::all().len() {
        app.menu_selected = i;
        press(&mut app, GameKey::ShiftLeft);
    }
    app.menu_selected = 0;
    press(&mut app, GameKey::ShiftRight);
    assert_eq!(app.creation_points_left(), 0);
    let spent = app.creation_choice().stats;
    app.status_line = None;

    press(&mut app, GameKey::Right);
    assert_eq!(
        app.creation_choice().stats,
        spent,
        "an overspend must change nothing"
    );
    assert!(
        app.status_line.is_some(),
        "an overspend must say why, through App::refuse"
    );
    assert!(
        app.creation_choice().cost().is_some(),
        "the choice must never be allowed to become unaffordable"
    );
}

/// The Points step is `Mode::Transfer`'s key idiom, which means the four
/// modified arrows have to reach it unfolded — `App::handle_key`'s fold is
/// the list of screens allowed to see one, and a screen missing from it
/// gets bare `Left`/`Right` with nothing failing anywhere.
#[test]
fn the_points_step_sees_a_modifier() {
    let mut app = opened("modifier");
    press(&mut app, ch('1'));
    spend_the_kit(&mut app); // the step will not be left with Credits in hand
    press(&mut app, GameKey::Enter);
    skip_the_look(&mut app);
    assert_eq!(app.creation_step(), CreationStep::Points);
    let axis = |want: MainStat| MainStat::all().iter().position(|s| *s == want).unwrap();

    // The step now opens on a rolled spread that already spends the whole
    // pool, which is what this test's own "the whole pool fits on it"
    // assumption depends on being false-going-in — clear every axis first
    // so this test still starts from an empty pool regardless of the roll.
    for i in 0..MainStat::all().len() {
        app.menu_selected = i;
        press(&mut app, GameKey::ShiftLeft);
    }

    // Integrity costs one point, so the whole pool fits on it — which is
    // what makes a *target* observably different from a single step. Def
    // costs three, and at a five-point pool its ceiling is one unit, so a
    // test written on that axis passes with the fold missing entirely.
    let integrity = axis(MainStat::Integrity);
    app.menu_selected = integrity;
    press(&mut app, GameKey::ShiftRight);
    assert_eq!(
        app.creation_choice().stats[integrity],
        CREATION_STAT_POINTS,
        "Shift+Right is a target — the far end of the row, not one step"
    );

    press(&mut app, GameKey::CtrlLeft);
    let halved = app.creation_choice().stats[integrity];
    assert!(
        halved > 0 && halved < CREATION_STAT_POINTS,
        "Ctrl+Left is a step that halves the gap, and it landed on {halved}"
    );

    press(&mut app, GameKey::ShiftLeft);
    assert_eq!(
        app.creation_choice().stats[integrity],
        0,
        "Shift+Left empties the row"
    );

    // The axis that costs more than a point: its ceiling is a division, so
    // the pool's remainder is unspendable there by construction.
    let def = axis(MainStat::Def);
    app.menu_selected = def;
    press(&mut app, GameKey::ShiftRight);
    assert_eq!(
        app.creation_choice().stats[def],
        CREATION_STAT_POINTS / CREATION_COST_DEF
    );
}

/// The save picker shows the player's own name, which is most of why the
/// Name step is worth having. A run that never named itself still falls
/// back to the filename.
#[test]
fn the_save_list_shows_the_players_name() {
    let mut app = opened("save_name");
    press(&mut app, ch('1')); // a class
    walk_to_the_summary(&mut app);
    assert_eq!(app.creation_step(), CreationStep::Summary);
    press(&mut app, GameKey::Enter);
    assert_eq!(app.creation_step(), CreationStep::Name);
    for c in "Kestrel".chars() {
        press(&mut app, ch(c));
    }
    press(&mut app, GameKey::Enter);
    settle(&mut app);
    assert_eq!(app.mode, Mode::Playing);

    let saves = app.list_saves();
    assert_eq!(saves.len(), 1);
    assert_eq!(saves[0].name, "Kestrel");
}

/// Difficulty is folded into the wizard rather than kept as a screen of its
/// own — `Mode::DifficultyPick` is gone — and the mode it replaced is what
/// the run's `DifficultyMode` now comes from.
#[test]
fn difficulty_is_chosen_in_the_wizard() {
    let mut app = wizard_app("difficulty");
    press(&mut app, ch('n'));
    assert_eq!(
        app.mode,
        Mode::CreateCharacter,
        "[n] opens the wizard, not a difficulty screen"
    );
    press(&mut app, ch('p'));
    assert_eq!(app.creation_difficulty(), Some(DifficultyMode::Permadeath));
    press(&mut app, GameKey::Enter); // the profile summary
    press(&mut app, ch('1')); // a class
    walk_to_the_summary(&mut app);
    press(&mut app, GameKey::Enter); // the summary, accepted
    press(&mut app, GameKey::Enter); // the name, left blank
    settle(&mut app);

    let data = saved_run(&mut app, "difficulty");
    assert_eq!(data.difficulty, DifficultyMode::Permadeath);
}

/// **There is no Unaligned option**, so the class step advances only once a
/// class is picked. `CharacterChoice::default()` stays classless and
/// neutral all the same — that is what the engine's ~1,600 `Game::new` call
/// sites construct and what `balance_sim`'s modelled floor corresponds to.
/// **The screen and the default disagree on purpose**, so both halves are
/// asserted here; "fixing" either one breaks the other.
#[test]
fn the_class_step_cannot_be_left_without_a_class() {
    assert_eq!(
        CharacterChoice::default().class,
        None,
        "the engine's default must stay classless — 1,600 call sites depend on it"
    );

    let mut app = opened("class");
    assert_eq!(app.creation_step(), CreationStep::Class);
    // No key advances this step without picking a class. `[n]` is what
    // skips the Icon, Colour and Routine steps, and it is inert here on
    // purpose — that omission *is* the "no Unaligned option" rule. Right
    // is the page-forward key and is refused for the same reason, out
    // loud; Left is page-*back* and is tested below, since it is allowed.
    for key in [ch('n'), GameKey::Right, GameKey::Backspace] {
        press(&mut app, key);
        assert_eq!(
            app.creation_step(),
            CreationStep::Class,
            "{key:?} left the class step with nothing picked"
        );
        assert_eq!(app.creation_choice().class, None);
    }

    assert!(
        app.status_line.is_some(),
        "a refused page-forward must say why"
    );

    // Left is the way back, and going back is always allowed.
    press(&mut app, GameKey::Left);
    assert_eq!(app.creation_step(), CreationStep::Profile);
    press(&mut app, GameKey::Left);
    assert_eq!(app.creation_step(), CreationStep::Difficulty);
    press(&mut app, ch('f'));
    press(&mut app, GameKey::Enter);
    assert_eq!(app.creation_step(), CreationStep::Class);

    // Enter takes the highlighted row, which is a pick like any other —
    // there is no row on this screen that means "none".
    press(&mut app, GameKey::Enter);
    assert_eq!(app.creation_step(), CreationStep::Kit);
    assert!(app.creation_choice().class.is_some());
}

/// The wizard resets on every open, so an abandoned character cannot leak
/// into the next one.
#[test]
fn reopening_the_wizard_starts_clean() {
    let mut app = opened("reset");
    press(&mut app, ch('1'));
    spend_the_kit(&mut app);
    assert_ne!(app.creation_choice(), &CharacterChoice::at_creation());

    press(&mut app, GameKey::Esc);
    while app.mode != Mode::MainMenu {
        press(&mut app, GameKey::Esc);
    }
    press(&mut app, ch('n'));
    assert_eq!(app.creation_step(), CreationStep::Difficulty);
    // `at_creation`, not `default`: the wizard opens holding the Perk
    // Point allowance, which is the one thing about its starting choice
    // that is not the engine's default player.
    assert_eq!(app.creation_choice(), &CharacterChoice::at_creation());
    assert_eq!(app.creation_difficulty(), None);
}

/// A fresh app has no earned rungs at all — the empty-catalogue guarantee
/// every reader of `Profile` carries, checked here rather than assumed: an
/// empty preview must draw as no rows rather than a blank one.
#[test]
fn an_empty_profile_previews_nothing() {
    let app = opened("empty_preview");
    assert!(app.profile_preview_rows().is_empty());
}

/// **The wizard's second page is what earlier runs earned**, and it draws
/// rows either way — an empty profile gets the sentence rather than a
/// blank box, which is the state a first-ever run is in and the state the
/// page is most worth reading in.
///
/// It sits before the class step because it is the one thing on the board
/// the player did not just decide, and because the ladder's payout is
/// otherwise met as a couple of lines buried in the Summary — which is how
/// a run that opened on six Perk Points after a screen saying four read as
/// a defect.
#[test]
fn the_profile_page_summarises_what_carried_over() {
    let mut first_run = wizard_app("profile_page_empty");
    press(&mut first_run, ch('n'));
    press(&mut first_run, ch('f'));
    assert_eq!(first_run.creation_step(), CreationStep::Profile);
    let rows = first_run.creation_rows();
    assert_eq!(rows.len(), 1, "a blank page is a broken page: {rows:?}");
    let CreationRow::Earned(line) = &rows[0] else {
        panic!("the profile page drew something else: {rows:?}");
    };
    assert!(
        line.contains("Nothing yet"),
        "an unearned profile has to say so: {line:?}"
    );
    press(&mut first_run, GameKey::Enter);
    assert_eq!(first_run.creation_step(), CreationStep::Class);

    // A returning player's record: the page lists what it will pay, folded
    // — two Perk Point rungs read as one `+2` line, not as `+1` twice.
    let mut profile = profile_with_every_reward_kind();
    profile.record(Earned {
        id: "boss_wintermute".into(), // PerkPoints(1), a second one
        first_tick: 9,
        permadeath: false,
        rolled_stat: None,
    });
    let mut app = wizard_app_with_profile("profile_page", &profile);
    press(&mut app, ch('n'));
    press(&mut app, ch('f'));
    let lines: Vec<String> = app
        .creation_rows()
        .into_iter()
        .map(|row| match row {
            CreationRow::Earned(line) => line,
            other => panic!("the profile page drew something else: {other:?}"),
        })
        .collect();
    assert!(
        lines.iter().any(|l| l == "+2 Perk Points"),
        "two rungs paying a point each must read as one +2 line: {lines:?}"
    );
    assert!(
        !lines.iter().any(|l| l == "+1 Perk Point"),
        "the folded line must replace the receipt, not sit beside it: {lines:?}"
    );
}

/// One rung of each `Reward` kind, earned before the wizard ever opens —
/// what a returning player's record would look like. The `RandomMainStat`
/// rung's roll is deliberately forced to differ from what a *fresh*
/// `roll_main_stat` call on the same id would produce, so this only passes
/// if the preview reads `Earned::rolled_stat` — the recorded answer — and
/// not a re-roll.
fn profile_with_every_reward_kind() -> Profile {
    let stat_rung: AchievementId = "breach_zone_2".into();
    let fresh_roll = roll_main_stat(&stat_rung);
    let recorded_roll = MainStat::all()
        .into_iter()
        .find(|stat| *stat != fresh_roll)
        .expect("MainStat::all() has more than one variant");

    let mut profile = Profile::default();
    profile.record(Earned {
        id: stat_rung,
        first_tick: 1,
        permadeath: false,
        rolled_stat: Some(recorded_roll),
    });
    profile.record(Earned {
        id: "stack_depth_5".into(), // PerkPoints(1)
        first_tick: 2,
        permadeath: false,
        rolled_stat: None,
    });
    profile.record(Earned {
        id: "stack_depth_8".into(), // StartingProgram("scrapper")
        first_tick: 3,
        permadeath: false,
        rolled_stat: None,
    });
    profile
}

/// Same shape as `wizard_app`, but `profile.ron` is seeded with `profile`
/// before the app ever reads it — the state a previous run's earning would
/// have left on disk.
fn wizard_app_with_profile(name: &str, profile: &Profile) -> App {
    let assets_dir = test_assets_dir();
    let root = std::env::temp_dir().join(format!(
        "feral_processes_creation_{name}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("saves")).unwrap();
    let profile_path = root.join("profile.ron");
    profile.save(&profile_path).unwrap();
    App::new(
        assets_dir,
        root.join("saves"),
        root.join("history.log"),
        profile_path,
        arenas_dir(),
        root.join("telemetry.jsonl"),
    )
}

/// Which `PlayerStatus` field a `Reward::RandomMainStat` axis lands on —
/// mirrors the match in `Game::grant_profile_rewards`, by call rather than
/// copy: `stat_field` is test-only scaffolding to read the *result*, not a
/// second statement of what pays what.
fn stat_field(stat: MainStat, status: &feral_processes_engine::views::PlayerStatus) -> i32 {
    match stat {
        MainStat::Atk => status.atk,
        MainStat::Def => status.mitigation,
        MainStat::Integrity => status.max_hp,
        MainStat::Decompiler => status.decompiler,
    }
}

/// The load-bearing test for Task 8: the preview and the payout must agree,
/// because they are the same call. Builds a profile with all three reward
/// kinds, reads the preview, starts the run, and checks the actual `Stats`,
/// `perk_points` and roster against exactly what the preview claimed —
/// against a same-choice baseline run with an empty profile, so the
/// assertion needs no hardcoded knowledge of `PLAYER_BASE_STATS` beyond what
/// `tuning.rs` already states elsewhere.
#[test]
fn the_preview_matches_what_is_paid() {
    let profile = profile_with_every_reward_kind();
    let recorded_stat = profile.earned[0].rolled_stat.unwrap();

    let mut app = wizard_app_with_profile("matches_paid", &profile);
    let rows = app.profile_preview_rows();
    assert_eq!(
        rows,
        vec![
            format!("+1 {}", recorded_stat.label()),
            "+1 Perk Point".to_string(),
            "start with a scrapper".to_string(),
        ]
    );

    let mut baseline = wizard_app_with_profile("matches_paid_baseline", &Profile::default());
    baseline.start_new_game(DifficultyMode::Forgiving, &CharacterChoice::default());
    settle(&mut baseline);
    let baseline_status = baseline.game.as_ref().unwrap().player_status();

    app.start_new_game(DifficultyMode::Forgiving, &CharacterChoice::default());
    settle(&mut app);
    let status = app.game.as_ref().unwrap().player_status();

    assert_eq!(
        stat_field(recorded_stat, &status),
        stat_field(recorded_stat, &baseline_status) + 1,
        "the stat the preview named did not move by what it claimed"
    );
    assert_eq!(status.perk_points, baseline_status.perk_points + 1);

    let pets = app.game.as_mut().unwrap().owned_pets();
    assert_eq!(
        pets.len(),
        baseline.game.as_mut().unwrap().owned_pets().len() + 1
    );
    assert!(
        pets[0].name.contains("Scrapper"),
        "expected the granted program to be a Scrapper, got {:?}",
        pets[0].name
    );
}

/// The class picked on step two is what prices the routine rows on step
/// five — a Medic reading a damage routine at 0.8 is the whole point of not
/// filtering the pool by class.
#[test]
fn the_routine_rows_are_priced_through_the_chosen_class() {
    let app = opened("priced");
    let striker = app
        .creation_catalogue
        .starter_rows(Some(PlayerClass::Striker));
    let medic = app
        .creation_catalogue
        .starter_rows(Some(PlayerClass::Medic));
    assert!(
        !striker.is_empty(),
        "the shipped assets carry starter routines"
    );
    assert_ne!(
        striker, medic,
        "two classes priced the same pool identically — the class term is not reaching the rows"
    );
}

/// The Points step opens on a rolled spread rather than a blank form — the
/// spec's own words, and until this test the actual gap: `advance_creation`
/// moved the cursor and nothing seeded `stats`, so it opened at `[0; 0; 0;
/// 0]` and cost `Some(0)` instead.
#[test]
fn the_points_step_opens_on_a_full_spread() {
    let mut app = opened("full_spread");
    press(&mut app, ch('1')); // a class
    spend_the_kit(&mut app); // the step will not be left with Credits in hand
    press(&mut app, GameKey::Enter);
    skip_the_look(&mut app); // -> Points, no icon or swatch picked
    assert_eq!(app.creation_step(), CreationStep::Points);
    assert_eq!(
        app.creation_choice().cost(),
        Some(CREATION_STAT_POINTS),
        "the step opened on {:?}, not a rolled spread",
        app.creation_choice().stats
    );
}

/// The rolled spread is a starting point, not a fixed one — the player must
/// still be able to move points around, and moving them must not create or
/// destroy any: `cost()` reads the same pool figure before and after.
#[test]
fn a_rolled_spread_can_be_redistributed() {
    let mut app = opened("redistribute");
    press(&mut app, ch('1'));
    spend_the_kit(&mut app); // the step will not be left with Credits in hand
    press(&mut app, GameKey::Enter);
    skip_the_look(&mut app);
    assert_eq!(app.creation_step(), CreationStep::Points);
    assert_eq!(app.creation_choice().cost(), Some(CREATION_STAT_POINTS));

    // Atk, Integrity and Decompiler all cost one point a unit; Def costs
    // three, and two units of Def alone already outspends a five-point
    // pool, so a one-point axis is guaranteed to hold at least one point
    // for a roll that spent the pool at all — moving a point between two
    // of them is a like-for-like swap the total spend cannot see.
    let one_point_axes: Vec<usize> = MainStat::all()
        .iter()
        .enumerate()
        .filter(|(_, s)| **s != MainStat::Def)
        .map(|(i, _)| i)
        .collect();
    let from = *one_point_axes
        .iter()
        .find(|&&i| app.creation_choice().stats[i] > 0)
        .expect("a one-point axis always holds what Def alone could not");
    let to = *one_point_axes.iter().find(|&&i| i != from).unwrap();

    app.menu_selected = from;
    press(&mut app, GameKey::Left);
    app.menu_selected = to;
    press(&mut app, GameKey::Right);

    assert_eq!(
        app.creation_choice().cost(),
        Some(CREATION_STAT_POINTS),
        "moving a point between two one-point axes must not change the total spend"
    );
}

/// A spread the player redistributed by hand must survive walking away from
/// the step and back — the seed is a starting point, and re-entering must
/// not silently replace what was built on top of it. `Decided::stats` is
/// what the entry seed checks; `spend_on_row` is what sets it the moment an
/// arrow key actually changes a row.
#[test]
fn reentering_points_keeps_a_hand_made_spread() {
    let mut app = opened("reenter_points");
    press(&mut app, ch('1'));
    spend_the_kit(&mut app); // the step will not be left with Credits in hand
    press(&mut app, GameKey::Enter);
    skip_the_look(&mut app);
    assert_eq!(app.creation_step(), CreationStep::Points);

    // A spread deliberately unlike anything the roll would leave alone:
    // clear every axis, then put the whole pool on Decompiler alone.
    for i in 0..MainStat::all().len() {
        app.menu_selected = i;
        press(&mut app, GameKey::ShiftLeft);
    }
    let decompiler = MainStat::all()
        .iter()
        .position(|s| *s == MainStat::Decompiler)
        .unwrap();
    app.menu_selected = decompiler;
    press(&mut app, GameKey::ShiftRight);
    let made = app.creation_choice().stats;
    assert_eq!(app.creation_choice().cost(), Some(CREATION_STAT_POINTS));

    press(&mut app, GameKey::Enter); // -> Perks
    assert_eq!(app.creation_step(), CreationStep::Perks);
    spend_the_perks(&mut app);
    press(&mut app, GameKey::Enter); // -> Routine
    press(&mut app, ch('n')); // no routine -> Summary
    assert_eq!(app.creation_step(), CreationStep::Summary);
    press(&mut app, GameKey::Esc); // -> Routine
    press(&mut app, GameKey::Esc); // -> Perks
    press(&mut app, GameKey::Esc); // -> Points, re-entered

    assert_eq!(app.creation_step(), CreationStep::Points);
    assert_eq!(
        app.creation_choice().stats,
        made,
        "re-entering the step reseeded a spread the player had already made"
    );
}

/// `[R]` must still be free to reroll the points after the step's own seed
/// has already run — a seed is not the hand-made decision `Decided::stats`
/// records. Checked directly against the flag rather than by comparing two
/// random draws, which are not guaranteed to differ and so cannot
/// black-box-prove a reroll actually ran.
#[test]
fn the_seed_does_not_mark_the_points_step_decided() {
    let mut app = opened("seed_not_decided");
    press(&mut app, ch('1'));
    spend_the_kit(&mut app); // the step will not be left with Credits in hand
    press(&mut app, GameKey::Enter);
    skip_the_look(&mut app);
    assert_eq!(app.creation_step(), CreationStep::Points);
    assert!(
        !app.creation_decided.stats_decided(),
        "the rolled spread must not read as a hand-made decision"
    );
    assert_eq!(
        app.creation_choice().cost(),
        Some(CREATION_STAT_POINTS),
        "the seed spends exactly the pool"
    );

    // One deliberate move is what marks it, and from then on re-entering
    // the step must not reseed — `reentering_points_keeps_a_hand_made_
    // spread` is the other half of that.
    press(&mut app, GameKey::Left);
    assert!(
        app.creation_decided.stats_decided(),
        "a spend by hand is the decision this flag records"
    );
}

// ---------------------------------------------------------------------------
// The Kit step.
// ---------------------------------------------------------------------------

/// A wizard sitting on the Kit step with a class picked.
fn on_the_kit_step(name: &str) -> App {
    let mut app = opened(name);
    press(&mut app, ch('1'));
    assert_eq!(app.creation_step(), CreationStep::Kit);
    app
}

/// What the basket holds of the shelf row at `index`.
fn taken(app: &App, index: usize) -> u32 {
    let id = &app.creation_catalogue.shelf_rows()[index].id;
    app.creation_choice()
        .items
        .iter()
        .find(|(item, _)| item == id)
        .map_or(0, |(_, qty)| *qty)
}

/// The step opens on an empty basket with the whole allowance — unlike
/// Points, which is seeded with a roll on the way in. Nothing to seed here:
/// an empty basket already means something (keep the class kit), so a seeded
/// one would be a decision the player never made.
#[test]
fn the_kit_step_opens_empty_with_the_whole_allowance() {
    let app = on_the_kit_step("kit_opens");
    assert!(app.creation_choice().items.is_empty());
    assert_eq!(app.creation_credits_left(), CREATION_CREDITS);
    assert!(
        !app.creation_rows().is_empty(),
        "the shipped item set stocked no shelf"
    );
}

/// `Mode::Transfer`'s table, in Credits: Right takes, Left puts back,
/// ShiftRight fills the row to what the allowance permits and ShiftLeft
/// empties it.
#[test]
fn the_kit_step_takes_and_puts_back() {
    let mut app = on_the_kit_step("kit_arrows");
    let price = app.creation_catalogue.shelf_rows()[0].price;

    press(&mut app, GameKey::Right);
    assert_eq!(taken(&app, 0), 1);
    assert_eq!(app.creation_credits_left(), CREATION_CREDITS - price);

    press(&mut app, GameKey::Left);
    assert_eq!(taken(&app, 0), 0);
    assert_eq!(app.creation_credits_left(), CREATION_CREDITS);
    assert!(
        app.creation_choice().items.is_empty(),
        "a row lowered to zero must leave the basket, not sit in it at zero"
    );

    press(&mut app, GameKey::ShiftRight);
    assert_eq!(taken(&app, 0), CREATION_CREDITS / price);
    assert_eq!(app.creation_credits_left(), CREATION_CREDITS % price);
    press(&mut app, GameKey::ShiftLeft);
    assert_eq!(app.creation_credits_left(), CREATION_CREDITS);
}

/// `App::put_available`'s rule: a row's ceiling counts what the **other**
/// rows have spent, never its own units — otherwise a row filled to the
/// allowance could never be lowered and raised again.
#[test]
fn a_full_row_can_still_be_lowered_and_raised() {
    let mut app = on_the_kit_step("kit_full_row");
    press(&mut app, GameKey::ShiftRight);
    let full = taken(&app, 0);
    assert!(full > 1);
    assert_eq!(app.creation_credits_left(), 0, "row 0 is the cheapest row");

    press(&mut app, GameKey::Left);
    assert_eq!(taken(&app, 0), full - 1);
    press(&mut app, GameKey::Right);
    assert_eq!(taken(&app, 0), full, "the row could not be raised again");
}

/// The allowance is one budget across every row, so filling one row lowers
/// what the others may hold — and asking past it refuses out loud rather
/// than going quietly dead. `App::refuse` is the door, so it lands on both
/// the popup and the log.
#[test]
fn the_allowance_is_one_budget_and_refuses_out_loud() {
    let mut app = on_the_kit_step("kit_budget");
    press(&mut app, GameKey::ShiftRight);
    assert_eq!(app.creation_credits_left(), 0);

    app.menu_selected = 1;
    press(&mut app, GameKey::Right);
    assert_eq!(
        taken(&app, 1),
        0,
        "a second row must not spend a spent purse"
    );
    let refusal = app.status_line.clone().expect("the refusal is said");
    assert!(
        refusal.contains("Credits left"),
        "unexpected refusal: {refusal}"
    );
}

/// Esc walks back to the Class step and forward again with the basket
/// intact — `CharacterChoice::items` is the only store, so there is no
/// parallel amount list to fall out of step with the shelf.
#[test]
fn walking_away_from_the_kit_step_keeps_the_basket() {
    let mut app = on_the_kit_step("kit_reenter");
    press(&mut app, GameKey::Right);
    press(&mut app, GameKey::Right);
    assert_eq!(taken(&app, 0), 2);

    press(&mut app, GameKey::Esc);
    assert_eq!(app.creation_step(), CreationStep::Class);
    press(&mut app, ch('1'));
    assert_eq!(app.creation_step(), CreationStep::Kit);
    assert_eq!(
        taken(&app, 0),
        2,
        "the basket did not survive the walk back"
    );
}

/// `[R]` spends as much of the allowance as the shelf allows, by
/// construction rather than by a check — `roll_points_spread`'s guarantee on
/// the other pool. It can never hand out a basket the commit would refuse.
#[test]
fn the_roll_fills_the_basket_within_the_allowance() {
    let mut app = on_the_kit_step("kit_roll");
    press(&mut app, ch('r'));
    assert_eq!(
        app.creation_step(),
        CreationStep::Kit,
        "the reroll stays on the screen whose basket it rolled"
    );

    let shelf = app.creation_catalogue.shelf_rows();
    let cheapest = shelf.iter().map(|r| r.price).min().unwrap();
    let spend: u32 = app
        .creation_choice()
        .items
        .iter()
        .map(|(id, qty)| {
            let row = shelf
                .iter()
                .find(|r| &r.id == id)
                .expect("the roll drew a row off the shelf");
            row.price * qty
        })
        .sum();
    assert!(
        spend <= CREATION_CREDITS,
        "the roll overspent: {spend} of {CREATION_CREDITS}"
    );
    assert!(
        CREATION_CREDITS - spend < cheapest,
        "the roll left {} Credits with a {cheapest}-Credit row still affordable",
        CREATION_CREDITS - spend
    );
}

/// **The perks picked on the wizard's own screen reach the run**, end to
/// end by keypress — the step is the third budget and the only one whose
/// spend is applied by replaying a purchase (`Game::unlock_perk`), so this
/// is what says the basket the screen built is the basket the run bought.
#[test]
fn the_perk_step_reaches_the_started_run() {
    let mut app = opened("perks_commit");
    press(&mut app, ch('1'));
    spend_the_kit(&mut app);
    press(&mut app, GameKey::Enter);
    skip_the_look(&mut app);
    spend_the_points(&mut app);
    press(&mut app, GameKey::Enter);
    assert_eq!(app.creation_step(), CreationStep::Perks);

    // The cheapest row, bought to whatever the allowance covers.
    press(&mut app, GameKey::ShiftRight);
    let bought = app.creation_choice().perks.clone();
    assert!(!bought.is_empty(), "the first row bought nothing");
    press(&mut app, GameKey::Enter);
    press(&mut app, ch('n')); // no routine
    press(&mut app, GameKey::Enter); // the summary
    press(&mut app, GameKey::Enter); // no name, which starts the run
    settle(&mut app);

    let game = app.game.as_ref().expect("the run did not start");
    // The manifest is where a player reads their perks back, and it is
    // built from the same `Perks` component the purchase wrote.
    let view = game.manifest(game.player_entity()).expect("a player sheet");
    let feral_processes_engine::ManifestSubject::Player(player) = &view.subject else {
        panic!("the player's own entity produced a program sheet");
    };
    let spent: u32 = bought
        .iter()
        .map(|(perk, levels)| {
            levels
                * game
                    .perk_defs()
                    .into_iter()
                    .find(|def| def.id == *perk)
                    .expect("a shipped perk")
                    .cost
        })
        .sum();
    assert_eq!(
        player.perk_points,
        CREATION_PERK_POINTS - spent,
        "what the screen did not spend must arrive with the run"
    );
    let (perk, levels) = bought[0];
    let name = game
        .perk_defs()
        .into_iter()
        .find(|def| def.id == perk)
        .expect("the perk the screen offered is in the run's own catalogue")
        .name;
    assert!(
        player
            .perks
            .iter()
            .any(|(n, lv)| *n == name && *lv == levels),
        "the perk bought at creation is not on the player: {:?}",
        player.perks
    );
}

/// **On a step that spends, the arrows spend** — they are not the page
/// keys there, which is the whole reason the two budget steps were carved
/// out of the paging block.
///
/// The Perks step shipped outside that carve-out for one release: its own
/// footer said "Left/Right buys", and Left walked back to the stat pool
/// while Right refused as a page-forward. Only the Shift and Ctrl variants
/// reached the basket at all.
///
/// Asserted on all three, because the carve-out is one list and a fourth
/// budget added outside it would land exactly here again.
#[test]
fn a_spending_step_gives_its_arrows_to_the_basket() {
    let mut app = opened("arrows_spend");
    press(&mut app, ch('1'));

    // The probe direction differs because the Points step opens on a
    // rolled spread that already spends the pool — Right there is refused
    // for having nothing affordable, which is not the question.
    for (step, probe, undo) in [
        (CreationStep::Kit, GameKey::Right, GameKey::Left),
        (CreationStep::Points, GameKey::Left, GameKey::Right),
        (CreationStep::Perks, GameKey::Right, GameKey::Left),
    ] {
        assert_eq!(app.creation_step(), step, "the walk fell out of step");
        let before = app.creation_choice().clone();
        press(&mut app, probe);
        assert_eq!(
            app.creation_step(),
            step,
            "{step:?} paged on {probe:?} instead of spending"
        );
        assert_ne!(
            app.creation_choice(),
            &before,
            "{step:?} did nothing on {probe:?}"
        );
        press(&mut app, undo);
        assert_eq!(
            app.creation_step(),
            step,
            "{step:?} paged on {undo:?} instead of spending"
        );
        assert_eq!(
            app.creation_choice(),
            &before,
            "{step:?} did not undo what {probe:?} did"
        );

        match step {
            CreationStep::Kit => {
                spend_the_kit(&mut app);
                press(&mut app, GameKey::Enter);
                skip_the_look(&mut app);
            }
            CreationStep::Points => {
                spend_the_points(&mut app);
                press(&mut app, GameKey::Enter);
            }
            _ => {}
        }
    }
}

/// **The creation allowance and the achievement ladder's Perk Points
/// stack**, and the wizard says so before the run starts.
///
/// This is the arithmetic that reads as a bug: the Perks step hands out
/// `CREATION_PERK_POINTS` and shows exactly that, while `Game::new` pays
/// the profile *after* the character is applied — so a player with two
/// earned Perk Points walks off a screen saying "4 of 4" and lands on 6.
#[test]
fn the_profile_adds_its_perk_points_on_top_of_the_allowance() {
    let profile = profile_with_every_reward_kind();
    let granted = {
        let app = wizard_app_with_profile("perk_profile_probe", &profile);
        app.profile_perk_points()
    };
    assert!(
        granted > 0,
        "the fixture profile has to grant Perk Points for this to say anything"
    );

    let mut app = wizard_app_with_profile("perk_profile", &profile);
    press(&mut app, ch('n'));
    press(&mut app, ch('f'));
    press(&mut app, GameKey::Enter); // the profile summary
    press(&mut app, ch('1'));
    walk_to_the_summary(&mut app);
    press(&mut app, GameKey::Enter); // the summary
    press(&mut app, GameKey::Enter); // no name, which starts the run
    settle(&mut app);

    let game = app.game.as_ref().expect("the run did not start");
    let view = game.manifest(game.player_entity()).expect("a player sheet");
    let feral_processes_engine::ManifestSubject::Player(player) = &view.subject else {
        panic!("the player's own entity produced a program sheet");
    };
    assert_eq!(
        player.perk_points,
        CREATION_PERK_POINTS + granted,
        "the wizard's allowance and the ladder's points are added, not \
         one instead of the other"
    );
}

/// **An allowance you can still spend is not a decision you have made.**
/// Both budget steps refuse to be left while anything on them is
/// affordable, and both say why — walking past the Points screen with the
/// pool untouched was the whole reason the figure went on the footer, and
/// a figure is only advice.
///
/// Asserted on **both** steps, because the two refusals are written at
/// different call sites and one of them holding is not evidence about the
/// other.
/// **Walking past the perk screen keeps the points**, which is the whole
/// of why that step is not a gate like the other two: a Perk Point is the
/// same point after the run starts and buys the same perk at the same
/// price, so a player who wants to decide later loses nothing by it.
///
/// This is the case that shipped broken — the allowance was consumed at
/// the door and a new run opened on zero.
#[test]
fn an_unspent_perk_allowance_arrives_with_the_run() {
    let mut app = opened("perks_kept");
    press(&mut app, ch('1'));
    walk_to_the_summary(&mut app);
    press(&mut app, GameKey::Enter); // the summary
    press(&mut app, GameKey::Enter); // no name, which starts the run
    settle(&mut app);

    let game = app.game.as_ref().expect("the run did not start");
    let view = game.manifest(game.player_entity()).expect("a player sheet");
    let feral_processes_engine::ManifestSubject::Player(player) = &view.subject else {
        panic!("the player's own entity produced a program sheet");
    };
    assert_eq!(player.perk_points, CREATION_PERK_POINTS);
    assert!(
        player.perks.is_empty(),
        "nothing was bought, so nothing is unlocked: {:?}",
        player.perks
    );
}

#[test]
fn an_unspent_allowance_holds_its_step() {
    let mut app = opened("unspent");
    press(&mut app, ch('1'));
    assert_eq!(app.creation_step(), CreationStep::Kit);

    press(&mut app, GameKey::Enter);
    assert_eq!(app.creation_step(), CreationStep::Kit, "Enter was let past");
    let why = app.status_line.clone().expect("the refusal said nothing");
    assert!(
        why.contains("Credits"),
        "the refusal must name what is unspent: {why:?}"
    );

    spend_the_kit(&mut app);
    press(&mut app, GameKey::Enter);
    skip_the_look(&mut app);
    assert_eq!(app.creation_step(), CreationStep::Points);

    // The Points step opens on a spread that already spends the pool, so
    // taking a point back is what makes it leavable-in-error.
    press(&mut app, GameKey::Left);
    assert!(app.creation_points_left() > 0, "nothing was freed");
    press(&mut app, GameKey::Enter);
    assert_eq!(
        app.creation_step(),
        CreationStep::Points,
        "the pool was left part-spent"
    );
    let why = app.status_line.clone().expect("the refusal said nothing");
    assert!(
        why.contains("points"),
        "the refusal must name what is unspent: {why:?}"
    );

    spend_the_points(&mut app);
    press(&mut app, GameKey::Enter);
    assert_eq!(app.creation_step(), CreationStep::Perks);
    spend_the_perks(&mut app);
    press(&mut app, GameKey::Enter);
    assert_eq!(app.creation_step(), CreationStep::Routine);
}

/// **Left and Right page the wizard**, on every step that does not spend —
/// the two that do already mean "take one" and "put one back" by them,
/// `Mode::Transfer`'s rule, and are the two steps a player cannot leave
/// early anyway.
///
/// Right is not a *pick*: it moves on leaving the step's choice as it
/// stands, which is what `[n]` already did on three of them.
#[test]
fn left_and_right_page_the_wizard() {
    let mut app = opened("paging");
    press(&mut app, ch('1'));
    spend_the_kit(&mut app);
    press(&mut app, GameKey::Enter);
    assert_eq!(app.creation_step(), CreationStep::Icon);

    press(&mut app, GameKey::Right);
    assert_eq!(app.creation_step(), CreationStep::Colour);
    assert_eq!(
        app.creation_choice().glyph,
        CharacterChoice::default().glyph,
        "paging past a step must not decide it"
    );
    press(&mut app, GameKey::Left);
    assert_eq!(app.creation_step(), CreationStep::Icon);

    // Back across the two spending steps, where the arrows are taking and
    // putting back rather than paging — Esc is what crosses those, and it
    // always was.
    press(&mut app, GameKey::Left);
    assert_eq!(app.creation_step(), CreationStep::Kit);
    press(&mut app, GameKey::Left);
    assert_eq!(
        app.creation_step(),
        CreationStep::Kit,
        "Left is a basket key here, not a page key"
    );
    press(&mut app, GameKey::Esc);
    press(&mut app, GameKey::Left);
    assert_eq!(app.creation_step(), CreationStep::Profile);
    press(&mut app, GameKey::Left);
    assert_eq!(app.creation_step(), CreationStep::Difficulty);

    // Left on the first step is not a way out of the wizard — Esc is, and
    // an arrow key that dropped the player back to the main menu mid-walk
    // would be a lost character.
    press(&mut app, GameKey::Left);
    assert_eq!(app.mode, Mode::CreateCharacter, "Left left the wizard");
    assert_eq!(app.creation_step(), CreationStep::Difficulty);
}

/// **`[r]` replaces a hand-made basket, and that is the inversion of the
/// rule it used to follow.** While the key rolled the whole character it
/// had to leave alone anything the player had settled, or one keystroke
/// destroyed eight steps of work with no undo. Narrowed to the one screen
/// whose basket it rolls, asking for a reroll *is* the decision — and
/// pressing it again costs nothing.
///
/// The old key is asserted dead in the same test: a wizard that quietly
/// kept `[R]` working would pass every other test here.
#[test]
fn the_kit_reroll_replaces_a_hand_made_basket() {
    let mut app = on_the_kit_step("kit_roll_decided");
    press(&mut app, GameKey::ShiftRight);
    let by_hand = app.creation_choice().items.clone();
    assert!(!by_hand.is_empty(), "the hand-made basket took nothing");

    press(&mut app, ch('R'));
    assert_eq!(
        app.creation_choice().items,
        by_hand,
        "[R] is not a key any more — only [r] rerolls"
    );

    // A roll that happens to reproduce the hand-made basket would make
    // this vacuous, so the property asserted is that the basket is a
    // *rolled* one: spent down to where nothing is affordable, which
    // ShiftRight on one row is not.
    press(&mut app, ch('r'));
    let shelf = app.creation_catalogue.shelf_rows();
    let cheapest = shelf.iter().map(|r| r.price).min().unwrap();
    assert!(
        app.creation_credits_left() < cheapest,
        "the reroll left {} Credits with a {cheapest}-Credit row affordable",
        app.creation_credits_left()
    );
}

/// The whole path, by keypress: a basket taken on the Kit step reaches the
/// run's `Inventory` and the class kit does not arrive beside it.
#[test]
fn a_picked_kit_reaches_the_started_run() {
    let mut app = on_the_kit_step("kit_commits");
    let row = app.creation_catalogue.shelf_rows()[0].clone();
    press(&mut app, GameKey::Right);
    press(&mut app, GameKey::Right);
    walk_to_the_summary(&mut app); // spends the rest of the allowance
    press(&mut app, GameKey::Enter); // accepted -> the Name step
    press(&mut app, GameKey::Enter); // no name, which starts the run

    // Not `Mode::Playing`: a fresh run may open on a notification screen.
    let game = app.game.as_ref().expect("the run did not start");
    let carried = game
        .player_status()
        .inventory
        .iter()
        .find(|r| r.copy.item == row.id)
        .map(|r| r.qty)
        .unwrap_or(0);
    assert!(
        carried >= 2,
        "picked 2x {} and the run holds {carried}",
        row.name
    );
}

// ---------------------------------------------------------------------------
// The Icon step's drawn-icon row and editor.
// ---------------------------------------------------------------------------

/// A wizard sitting on the Icon step, class picked and the Kit spent —
/// every test below starts from here.
fn on_the_icon_step(name: &str) -> App {
    let mut app = opened(name);
    press(&mut app, ch('1'));
    spend_the_kit(&mut app);
    press(&mut app, GameKey::Enter);
    assert_eq!(app.creation_step(), CreationStep::Icon);
    app
}

/// Opens the editor from the Icon step's sixth row — the one action every
/// test below needs before it can drive the editor's own key table.
fn open_the_editor(app: &mut App) {
    app.menu_selected = CREATION_ICONS.len();
    press(app, GameKey::Enter);
}

/// Paints one pixel through the editor's real key table and keeps it —
/// `Tab` to the palette is not needed, since the editor opens with a
/// paintable colour already selected. Leaves the wizard on the Icon step,
/// which is where the spec's key table says both editor endings land.
fn draw_and_keep(app: &mut App) {
    press(app, GameKey::Char(' '));
    press(app, GameKey::Enter);
}

/// Pages off the Icon step onto Colour. `Right` rather than `Enter`,
/// because `Enter` on the drawn row reopens the editor.
fn leave_the_icon_step(app: &mut App) {
    press(app, GameKey::Right);
}

/// The Icon step offers the five presets plus the drawn row, and the drawn
/// row reads app-core's own `drawn` flag rather than something the
/// renderer has to re-derive.
#[test]
fn the_icon_step_offers_six_rows() {
    let app = on_the_icon_step("six_rows");
    let rows = app.creation_rows();
    assert_eq!(rows.len(), CREATION_ICONS.len() + 1);
    assert!(
        matches!(rows.last(), Some(CreationRow::DrawnIcon { drawn: false })),
        "a fresh wizard with nothing drawn must offer the sixth row undrawn: {rows:?}"
    );
}

/// Taking the drawn row opens the editor rather than deciding anything —
/// the wizard stays on the Icon step until the editor itself is left.
#[test]
fn taking_the_drawn_row_opens_the_editor() {
    let mut app = on_the_icon_step("opens_editor");
    assert!(app.icon_editor_view().is_none());

    open_the_editor(&mut app);

    assert!(
        app.icon_editor_view().is_some(),
        "taking the sixth row must open the editor"
    );
    assert_eq!(
        app.creation_step(),
        CreationStep::Icon,
        "opening the editor does not advance the wizard"
    );
}

/// `Enter` inside the editor lands the drawing on the choice and returns
/// to the Icon step with the drawn row still selected — the spec's key
/// table, which pairs `Enter` ("keep the drawing, return to the Icon
/// step") with `Esc` ("discard changes, return to the Icon step"). The
/// editor is the one place in the wizard where a decision is not also a
/// step forward: the player has just been shown their own art, and the
/// screen that shows it again is the one they came from.
#[test]
fn enter_in_the_editor_keeps_the_drawing_and_returns_to_the_icon_step() {
    let mut app = on_the_icon_step("editor_keep");
    open_the_editor(&mut app);
    assert!(app.icon_editor_view().is_some());

    draw_and_keep(&mut app);

    assert!(
        app.icon_editor_view().is_none(),
        "keeping the drawing must close the editor"
    );
    assert!(
        app.creation_choice().icon.is_some(),
        "Enter in the editor must land the drawing on the choice"
    );
    assert_eq!(
        app.creation_step(),
        CreationStep::Icon,
        "keeping a drawing returns to the Icon step rather than advancing"
    );
    assert!(
        matches!(
            app.creation_rows().get(app.menu_selected),
            Some(CreationRow::DrawnIcon { drawn: true })
        ),
        "the drawn row must still be the selected one: {:?}",
        app.creation_rows().get(app.menu_selected)
    );
}

/// The row is selected on the way back even when the editor was opened by
/// its number key — `selected_index` answers a digit without moving the
/// cursor, so "return to the Icon step with that row selected" has to be
/// arranged on the way in.
#[test]
fn opening_the_editor_by_its_number_key_selects_the_drawn_row() {
    let mut app = on_the_icon_step("editor_number_key");
    app.menu_selected = 0;

    press(&mut app, ch('6')); // the sixth row
    assert!(app.icon_editor_view().is_some(), "the editor must open");
    press(&mut app, GameKey::Esc);

    assert_eq!(
        app.menu_selected,
        CREATION_ICONS.len(),
        "leaving the editor must land on the drawn row"
    );
}

/// `Esc` inside the editor discards what was drawn and leaves the choice
/// exactly as it was — the editor itself restores what it opened with, so
/// the wizard only has to close it and change nothing.
#[test]
fn esc_in_the_editor_leaves_the_choice_as_it_was() {
    let mut app = on_the_icon_step("editor_discard");
    assert!(app.creation_choice().icon.is_none());
    open_the_editor(&mut app);
    assert!(app.icon_editor_view().is_some());

    press(&mut app, GameKey::Char(' ')); // paint something
    press(&mut app, GameKey::Esc); // then throw it away

    assert!(
        app.icon_editor_view().is_none(),
        "discarding must close the editor"
    );
    assert!(
        app.creation_choice().icon.is_none(),
        "Esc must not land the drawing on the choice"
    );
    assert_eq!(
        app.creation_step(),
        CreationStep::Icon,
        "discarding does not advance the wizard"
    );
}

/// **Taking a preset clears a drawn icon.** The two choices cannot both be
/// live and the drawn icon wins at the draw site, so a preset that left it
/// in place would look like the preset row doing nothing.
#[test]
fn taking_a_preset_row_clears_a_drawn_icon() {
    let mut app = on_the_icon_step("preset_clears");
    open_the_editor(&mut app);
    draw_and_keep(&mut app);
    assert!(app.creation_choice().icon.is_some());
    assert_eq!(app.creation_step(), CreationStep::Icon);

    press(&mut app, ch('1')); // the first preset

    assert!(
        app.creation_choice().icon.is_none(),
        "taking a preset must clear a drawn icon"
    );
    assert_eq!(app.creation_choice().glyph, CREATION_ICONS[0].0);
    assert_eq!(app.creation_step(), CreationStep::Colour);
}

/// **Regression.** `None` is also what taking a preset produces on
/// purpose, so a seed guarded on the *value* of `creation_choice.icon`
/// rather than a one-shot latch would fire again the moment the player
/// walked back to the Icon step, silently un-picking the preset by
/// re-seeding the profile's saved drawing over it.
#[test]
fn walking_back_to_the_icon_step_does_not_undo_a_preset() {
    let mut icon = PlayerIcon::default();
    icon.set(0, 0, 5);
    let profile = Profile {
        player_icon: Some(icon.encode()),
        ..Default::default()
    };

    let mut app = wizard_app_with_profile("preset_survives_reentry", &profile);
    press(&mut app, ch('n'));
    press(&mut app, ch('f'));
    press(&mut app, GameKey::Enter); // -> Class
    press(&mut app, ch('1'));
    spend_the_kit(&mut app);
    press(&mut app, GameKey::Enter); // -> Icon, seeded from the profile
    assert_eq!(
        app.creation_choice().icon,
        Some(icon),
        "the step must open seeded from the profile"
    );

    press(&mut app, ch('1')); // take the first preset -> Colour
    assert!(
        app.creation_choice().icon.is_none(),
        "the preset must clear it"
    );
    assert_eq!(app.creation_step(), CreationStep::Colour);

    press(&mut app, GameKey::Esc); // back to Icon
    assert_eq!(app.creation_step(), CreationStep::Icon);

    assert!(
        app.creation_choice().icon.is_none(),
        "walking back to the Icon step must not reseed over a preset the player just took"
    );
}

/// The step seeds the drawing from `Profile::player_icon` the moment it is
/// entered — once, the Points step's roll's own rule — so a player who
/// drew something last run sees it again without having to redraw it.
#[test]
fn entering_the_icon_step_seeds_from_a_profile_with_an_icon() {
    let mut icon = PlayerIcon::default();
    icon.set(0, 0, 3);
    let profile = Profile {
        player_icon: Some(icon.encode()),
        ..Default::default()
    };

    let mut app = wizard_app_with_profile("icon_seed", &profile);
    press(&mut app, ch('n'));
    press(&mut app, ch('f'));
    press(&mut app, GameKey::Enter); // -> Class
    press(&mut app, ch('1'));
    spend_the_kit(&mut app);
    press(&mut app, GameKey::Enter); // -> Icon

    assert_eq!(app.creation_step(), CreationStep::Icon);
    assert_eq!(
        app.creation_choice().icon,
        Some(icon),
        "the step must seed the drawing from the profile on arrival"
    );
    assert!(matches!(
        app.creation_rows().last(),
        Some(CreationRow::DrawnIcon { drawn: true })
    ));
}

/// **A profile written before the grid halved still seeds the wizard.**
/// `v1` carried 256 pixels; `PlayerIcon::decode` folds each 2x2 block onto
/// the 8x8 grid, so a player who drew an icon on the old editor opens the
/// new one on the same figure rather than on a blank canvas. Asserted
/// through the wizard rather than on `decode` alone, because the seed is
/// the only place a player would ever see the difference.
#[test]
fn entering_the_icon_step_seeds_from_a_v1_profile() {
    // A 2x2 block of colour 4 at the old grid's origin, folding to cell
    // (0, 0); everything else transparent.
    let mut v1 = String::from("v1:");
    for y in 0..16 {
        for x in 0..16 {
            v1.push(if x < 2 && y < 2 { '4' } else { '0' });
        }
    }
    let profile = Profile {
        player_icon: Some(v1),
        ..Default::default()
    };

    let mut app = wizard_app_with_profile("icon_seed_v1", &profile);
    press(&mut app, ch('n'));
    press(&mut app, ch('f'));
    press(&mut app, GameKey::Enter); // -> Class
    press(&mut app, ch('1'));
    spend_the_kit(&mut app);
    press(&mut app, GameKey::Enter); // -> Icon

    let mut folded = PlayerIcon::default();
    folded.set(0, 0, 4);
    assert_eq!(
        app.creation_choice().icon,
        Some(folded),
        "a v1 profile must fold onto the 8x8 grid rather than being dropped"
    );
    assert!(matches!(
        app.creation_rows().last(),
        Some(CreationRow::DrawnIcon { drawn: true })
    ));
}

/// A profile with nothing drawn opens the editor on a blank canvas rather
/// than a garbage or stale one.
#[test]
fn a_profile_with_no_icon_opens_the_editor_on_a_blank_canvas() {
    let mut app = on_the_icon_step("blank_canvas");
    assert!(app.creation_choice().icon.is_none());

    open_the_editor(&mut app);

    let view = app.icon_editor_view().expect("the editor must be open");
    assert!(
        view.canvas.cells.iter().all(|&p| p == 0),
        "a profile with nothing drawn must open the editor on a blank canvas"
    );
}

/// The whole path: a drawing kept on the Icon step reaches `profile.ron`
/// when creation finishes, through the same write path every other profile
/// change uses — never a hand-written file.
#[test]
fn a_drawn_icon_reaches_the_profile_when_creation_finishes() {
    let mut app = on_the_icon_step("icon_to_profile");
    open_the_editor(&mut app);
    draw_and_keep(&mut app);
    let drawn = app
        .creation_choice()
        .icon
        .clone()
        .expect("the drawing must have landed on the choice");
    assert_eq!(app.creation_step(), CreationStep::Icon);
    leave_the_icon_step(&mut app);

    press(&mut app, ch('n')); // skip the swatch
    spend_the_points(&mut app);
    press(&mut app, GameKey::Enter); // -> Perks
    spend_the_perks(&mut app);
    press(&mut app, GameKey::Enter); // -> Routine
    press(&mut app, ch('n')); // -> Summary
    press(&mut app, GameKey::Enter); // -> Name
    press(&mut app, GameKey::Enter); // starts the run
    assert!(app.game.is_some(), "the run did not start");
    settle(&mut app);

    let path = app.profile_path.clone();
    let (on_disk, warning) = Profile::load(&path);
    assert!(warning.is_none(), "{warning:?}");
    assert_eq!(
        on_disk.player_icon.as_deref().and_then(PlayerIcon::decode),
        Some(drawn),
        "the drawn icon must reach the profile on disk when creation finishes"
    );
}

/// The Colour step's own line of help text: a drawn icon takes over the map
/// tile, and the step says so rather than quietly deciding nothing — the
/// swatch still colours the glyph everywhere else, which is what the note
/// must name.
#[test]
fn the_colour_step_explains_a_drawn_icon_hides_the_swatch() {
    let mut app = on_the_icon_step("colour_note");
    assert!(
        app.creation_colour_note().is_none(),
        "nothing has been drawn yet"
    );

    open_the_editor(&mut app);
    draw_and_keep(&mut app);
    leave_the_icon_step(&mut app);
    assert_eq!(app.creation_step(), CreationStep::Colour);

    let note = app
        .creation_colour_note()
        .expect("a drawn icon must earn a note on the Colour step");
    assert!(
        note.to_lowercase().contains("glyph"),
        "the note must say what the swatch still governs: {note:?}"
    );
}

/// **Regression (Finding 1).** An all-transparent canvas is not a drawing.
/// `Enter` on one used to land `Some(blank)` on the choice, and only the
/// texture upload knew better: the row read "Your drawing", the Colour
/// step promised a map tile that `Sprites::sync_drawn_icon` then declined
/// to upload, and a payload of zeros persisted to both the save and the
/// profile. The decision belongs at the one place the drawing is kept.
#[test]
fn keeping_a_blank_canvas_is_not_a_drawing() {
    let mut app = on_the_icon_step("blank_keep");
    open_the_editor(&mut app);

    press(&mut app, GameKey::Enter); // keep, having drawn nothing at all

    assert!(
        app.creation_choice().icon.is_none(),
        "an all-transparent canvas must not be kept as a drawing"
    );
    assert!(
        matches!(
            app.creation_rows().last(),
            Some(CreationRow::DrawnIcon { drawn: false })
        ),
        "the row must not read as drawn: {:?}",
        app.creation_rows().last()
    );
    assert!(
        app.creation_colour_note().is_none(),
        "the Colour step must not promise a map tile nothing will draw"
    );
}

/// The same rule reached from the other side: a drawing cleared with `x`
/// and then kept is a blank canvas too.
#[test]
fn clearing_a_drawing_and_keeping_it_is_not_a_drawing() {
    let mut app = on_the_icon_step("cleared_keep");
    open_the_editor(&mut app);
    press(&mut app, GameKey::Char(' ')); // paint one pixel
    press(&mut app, ch('x')); // then wipe the canvas
    press(&mut app, GameKey::Enter); // and keep what is left

    assert!(
        app.creation_choice().icon.is_none(),
        "a canvas cleared back to transparent must not be kept as a drawing"
    );
}

/// **Regression (Finding 2).** The profile holds *the last thing drawn*,
/// which is what makes it survive across runs. Choosing a preset icon for
/// one character is not drawing something, so it must not overwrite — and
/// there is no undo and no second copy of the art anywhere the wizard
/// reads.
#[test]
fn taking_a_preset_leaves_the_profiles_drawing_alone() {
    let mut icon = PlayerIcon::default();
    icon.set(3, 4, 7);
    let profile = Profile {
        player_icon: Some(icon.encode()),
        ..Default::default()
    };

    let mut app = wizard_app_with_profile("preset_keeps_profile_icon", &profile);
    press(&mut app, ch('n'));
    press(&mut app, ch('f'));
    press(&mut app, GameKey::Enter); // -> Class
    press(&mut app, ch('1'));
    spend_the_kit(&mut app);
    press(&mut app, GameKey::Enter); // -> Icon, seeded from the profile
    assert_eq!(app.creation_choice().icon, Some(icon.clone()));

    press(&mut app, ch('1')); // a preset -> Colour, and the drawing is cleared
    assert!(app.creation_choice().icon.is_none());
    press(&mut app, ch('n')); // skip the swatch
    spend_the_points(&mut app);
    press(&mut app, GameKey::Enter); // -> Perks
    spend_the_perks(&mut app);
    press(&mut app, GameKey::Enter); // -> Routine
    press(&mut app, ch('n')); // -> Summary
    press(&mut app, GameKey::Enter); // -> Name
    press(&mut app, GameKey::Enter); // starts the run
    assert!(app.game.is_some(), "the run did not start");
    settle(&mut app);

    let (on_disk, warning) = Profile::load(&app.profile_path);
    assert!(warning.is_none(), "{warning:?}");
    assert_eq!(
        on_disk.player_icon.as_deref().and_then(PlayerIcon::decode),
        Some(icon),
        "wearing a preset must not erase the cross-run drawing from profile.ron"
    );
    assert!(
        saved_run(&mut app, "preset_keeps_profile_icon")
            .player
            .icon
            .is_none(),
        "a character who wears a preset has no icon of their own in the save"
    );
}
