//! The character-creation wizard — `Mode::CreateCharacter` and its eight
//! steps.
//!
//! `the_summary_step_commits_the_choice` is the load-bearing one: it walks
//! the whole wizard by keypress and then reads the *saved* run back off
//! disk, so every choice has to survive the whole path from a keystroke to
//! `Game::new_with` to `PlayerSave`.

use super::support::*;
use crate::*;
use feral_processes_engine::achievements::{AchievementId, Earned, roll_main_stat};
use feral_processes_engine::save;
use feral_processes_engine::species::AffinityClass;
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

/// Opens the wizard and picks Forgiving, leaving it on the Class step.
fn opened(name: &str) -> App {
    let mut app = wizard_app(name);
    press(&mut app, ch('n'));
    press(&mut app, ch('f'));
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
    assert_eq!(app.creation_step(), CreationStep::Class);
    press(&mut app, ch('1'));
    assert_eq!(app.creation_step(), CreationStep::Kit);
    press(&mut app, GameKey::Enter);
    assert_eq!(app.creation_step(), CreationStep::Icon);
    press(&mut app, ch('n'));
    assert_eq!(app.creation_step(), CreationStep::Colour);
    press(&mut app, ch('n'));
    assert_eq!(app.creation_step(), CreationStep::Points);
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
        CreationStep::Points,
        CreationStep::Colour,
        CreationStep::Icon,
        CreationStep::Kit,
        CreationStep::Class,
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
    press(&mut app, GameKey::Enter); // past the Kit step, basket untouched

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
    press(&mut app, GameKey::Right);
    press(&mut app, GameKey::Right);
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
        PLAYER_BASE_STATS.max_hp + 2 * CREATION_GAIN_INTEGRITY as i32
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

/// `[R]` rolls the rest and jumps to the summary, and **spends exactly the
/// pool** — that is what makes it a reroll for shape rather than one for
/// size, and what stops it beating point-buy.
#[test]
fn roll_everything_spends_exactly_the_pool() {
    let mut app = opened("roll");
    press(&mut app, ch('R'));
    assert_eq!(app.creation_step(), CreationStep::Summary);
    assert_eq!(
        app.creation_points_left(),
        0,
        "a roll left points on the table: {:?}",
        app.creation_choice().stats
    );
    assert!(
        app.creation_choice().class.is_some(),
        "the roll picked a class"
    );
    assert!(
        app.creation_choice().routine.is_some(),
        "the roll picked a starter routine"
    );
    assert!(
        app.creation_choice().colour.is_some(),
        "the roll picked a colour"
    );
}

/// `[R]` on the Summary must not destroy the character the player just
/// walked seven steps to build. Every choice is made here, so there is
/// nothing left to roll and the key refuses instead — the alternative,
/// which shipped, replaced class, look, spread and routine with fresh
/// random values on one undocumented keystroke with no undo.
#[test]
fn the_roll_leaves_a_finished_character_alone() {
    let mut app = opened("roll_finished");
    press(&mut app, ch('2')); // a class
    // A kit taken by hand: an untouched basket is not a decision, so `[R]`
    // filling one is the roll doing its job rather than overwriting a
    // choice — which is the distinction this test exists to hold.
    press(&mut app, GameKey::Right);
    press(&mut app, GameKey::Enter);
    press(&mut app, ch(menu_shortcut(1))); // the second icon
    press(&mut app, ch(menu_shortcut(2))); // the third swatch
    // The Points step now opens on a rolled spread that already spends the
    // whole pool, so every axis sits at its own ceiling the instant the
    // step is entered — Right/ShiftRight on any of them refuses. Only a
    // leftward move has room, and since the roll spent the pool somewhere,
    // some axis is guaranteed to hold at least one unit to take from.
    let axis = app
        .creation_choice()
        .stats
        .iter()
        .position(|&units| units > 0)
        .expect("a roll that spends the pool must spend it on some axis");
    app.menu_selected = axis;
    press(&mut app, GameKey::Left); // a point taken off the highlighted axis
    press(&mut app, GameKey::Enter);
    press(&mut app, ch('1')); // a starter routine
    assert_eq!(app.creation_step(), CreationStep::Summary);

    let before = app.creation_choice().clone();
    press(&mut app, ch('R'));

    assert_eq!(
        *app.creation_choice(),
        before,
        "the roll overwrote choices the player had already made"
    );
    assert!(app.status_line.is_some(), "the refusal said why");
}

/// The half of the same rule that still has to work: what the player has
/// not settled is exactly what `[R]` fills in. A class picked by hand
/// survives; the look, the spread and the routine are rolled around it.
#[test]
fn the_roll_fills_only_what_is_undecided() {
    let mut app = opened("roll_partial");
    let classes = app.creation_catalogue.class_rows();
    let wanted = classes[1].class;
    press(&mut app, ch('2'));
    assert_eq!(app.creation_choice().class, Some(wanted));

    press(&mut app, ch('R'));

    assert_eq!(app.creation_step(), CreationStep::Summary);
    assert_eq!(
        app.creation_choice().class,
        Some(wanted),
        "the roll replaced a class the player had already picked"
    );
    assert_eq!(app.creation_points_left(), 0, "the spread was still rolled");
    assert!(
        app.creation_choice().routine.is_some(),
        "the routine was still rolled"
    );
    assert!(
        app.creation_choice().colour.is_some(),
        "the colour was still rolled"
    );
}

/// Difficulty is the one thing `[R]` will not roll — a commitment, not a
/// shape — so pressing it on the first step is refused rather than
/// silently handing someone permadeath.
#[test]
fn the_roll_never_picks_the_difficulty() {
    let mut app = wizard_app("roll_difficulty");
    press(&mut app, ch('n'));
    press(&mut app, ch('R'));
    assert_eq!(
        app.creation_step(),
        CreationStep::Difficulty,
        "the roll skipped past the difficulty"
    );
    assert!(app.creation_difficulty().is_none());
    assert!(app.status_line.is_some(), "the refusal said why");
}

/// Right on a row with nothing left is refused, through `App::refuse` —
/// the one door, which is what puts the same sentence on the banner and in
/// the log the player scrolls back through.
#[test]
fn points_cannot_be_overspent() {
    let mut app = opened("overspend");
    press(&mut app, ch('1'));
    press(&mut app, GameKey::Enter); // past the Kit step, basket untouched
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
    press(&mut app, GameKey::Enter); // past the Kit step, basket untouched
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
    // `[R]` lands on the summary; accepting it is what asks for the name.
    press(&mut app, ch('R'));
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
    press(&mut app, ch('R'));
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
    // purpose —
    // that omission *is* the "no Unaligned option" rule.
    for key in [ch('n'), GameKey::Left, GameKey::Right, GameKey::Backspace] {
        press(&mut app, key);
        assert_eq!(
            app.creation_step(),
            CreationStep::Class,
            "{key:?} left the class step with nothing picked"
        );
        assert_eq!(app.creation_choice().class, None);
    }

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
    press(&mut app, ch('R'));
    assert_ne!(app.creation_choice(), &CharacterChoice::default());

    press(&mut app, GameKey::Esc);
    while app.mode != Mode::MainMenu {
        press(&mut app, GameKey::Esc);
    }
    press(&mut app, ch('n'));
    assert_eq!(app.creation_step(), CreationStep::Difficulty);
    assert_eq!(app.creation_choice(), &CharacterChoice::default());
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
        .starter_rows(Some(AffinityClass::Striker));
    let medic = app
        .creation_catalogue
        .starter_rows(Some(AffinityClass::Medic));
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
    press(&mut app, GameKey::Enter); // past the Kit step, basket untouched
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
    press(&mut app, GameKey::Enter); // past the Kit step, basket untouched
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
    press(&mut app, GameKey::Enter); // past the Kit step, basket untouched
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

    press(&mut app, GameKey::Enter); // -> Routine
    assert_eq!(app.creation_step(), CreationStep::Routine);
    press(&mut app, ch('n')); // no routine -> Summary
    assert_eq!(app.creation_step(), CreationStep::Summary);
    press(&mut app, GameKey::Esc); // -> Routine
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
    press(&mut app, GameKey::Enter); // past the Kit step, basket untouched
    skip_the_look(&mut app);
    assert_eq!(app.creation_step(), CreationStep::Points);
    assert!(
        !app.creation_decided.stats_decided(),
        "the rolled spread must not read as a hand-made decision"
    );

    press(&mut app, ch('R'));
    assert_eq!(
        app.creation_step(),
        CreationStep::Summary,
        "R must still be live for the points once the step's own seed has run"
    );
    assert_eq!(
        app.creation_choice().cost(),
        Some(CREATION_STAT_POINTS),
        "the reroll still spends exactly the pool"
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
    press(&mut app, ch('R'));
    assert_eq!(app.creation_step(), CreationStep::Summary);

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

/// A hand-built basket is not rerolled — `Decided::kit`, the rule every
/// other step already follows.
#[test]
fn the_roll_leaves_a_hand_made_basket_alone() {
    let mut app = on_the_kit_step("kit_roll_decided");
    press(&mut app, GameKey::Right);
    let before = app.creation_choice().items.clone();
    press(&mut app, ch('R'));
    assert_eq!(app.creation_choice().items, before);
}

/// The whole path, by keypress: a basket taken on the Kit step reaches the
/// run's `Inventory` and the class kit does not arrive beside it.
#[test]
fn a_picked_kit_reaches_the_started_run() {
    let mut app = on_the_kit_step("kit_commits");
    let row = app.creation_catalogue.shelf_rows()[0].clone();
    press(&mut app, GameKey::Right);
    press(&mut app, GameKey::Right);
    press(&mut app, ch('R')); // rolls the rest, jumps to the Summary
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
