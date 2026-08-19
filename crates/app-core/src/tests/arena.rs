//! The dev arena: the gate, the session, and the screens that edit and
//! fight a scenario.

use feral_processes_engine::arena::{Encounter, OpponentSpec, PlayerSource, Scenario};
use feral_processes_engine::tuning::MAX_GROUP_SIZE;
use feral_processes_engine::world::Biome;

use super::support::{test_app, test_assets_dir};
use crate::*;

/// An app sitting on the main menu with the arena gate open.
///
/// The flag is set on the field rather than in the environment on purpose:
/// `std::env` is process-global and the suite runs its cases in parallel, so
/// a test that set `FERAL_DEV_ARENA` would turn the gate on for every other
/// test in flight. `App::new` stays the only reader of the variable.
fn app_with_arena(seed: u32) -> App {
    let mut app = test_app(seed);
    app.game = None;
    app.mode = Mode::MainMenu;
    app.arena_enabled = true;
    app
}

/// A scenario naming `opponents` against a `Fresh` player.
fn scenario(level: u32, zone: u32, opponents: &[(&str, u32)], seed: u64) -> Scenario {
    Scenario {
        player: PlayerSource::Fresh { level, zone },
        opponents: opponents
            .iter()
            .map(|(species, count)| OpponentSpec {
                species: (*species).into(),
                count: *count,
            })
            .collect(),
        seed,
        ..Scenario::default()
    }
}

/// An open arena session already staged into `Mode::Battle`.
fn app_fighting(seed: u32, scenario: Scenario) -> App {
    let mut app = app_with_arena(seed);
    app.handle_key(GameKey::Char('r'));
    let session = app.arena.as_mut().unwrap();
    session.seed = scenario.seed;
    session.scenario = scenario;
    app.handle_key(GameKey::Char('f'));
    app
}

/// A key press that is the test's own input rather than a skipped reveal —
/// narration scrolls in on a frontend's clock, and no frontend is running.
fn press(app: &mut App, key: GameKey) {
    app.finish_reveal();
    app.handle_key(key);
}

fn rounds_seen(app: &App) -> u32 {
    app.arena
        .as_ref()
        .and_then(|s| s.watch.as_ref())
        .map(|w| w.rounds())
        .expect("a staged fight has a watch")
}

/// Plays the fight out with the party's own All-Attack until it lands
/// somewhere that is no longer a battle screen. More than one opponent
/// group sends that command through the target picker first, so this
/// answers that too rather than leaving the fight parked on it.
fn fight_to_the_end(app: &mut App) {
    for _ in 0..500 {
        if !app.mode.is_battle() {
            return;
        }
        press(
            app,
            match app.mode {
                Mode::Battle => GameKey::Char('A'),
                _ => GameKey::Enter,
            },
        );
    }
    panic!("the fixture never resolved: mode {:?}", app.mode);
}

#[test]
fn without_the_dev_flag_the_arena_row_is_absent() {
    let mut app = app_with_arena(1);
    app.arena_enabled = false;

    app.handle_key(GameKey::Char('r'));

    assert_eq!(app.mode, Mode::MainMenu);
    assert!(app.arena.is_none());
}

#[test]
fn with_the_dev_flag_r_opens_the_builder() {
    let mut app = app_with_arena(2);

    app.handle_key(GameKey::Char('r'));

    assert_eq!(app.mode, Mode::ArenaBuilder);
    assert!(app.arena.is_some());
}

#[test]
fn a_fresh_session_starts_from_the_default_scenario_unsaved() {
    let mut app = app_with_arena(3);

    app.handle_key(GameKey::Char('r'));

    let session = app.arena.as_ref().unwrap();
    assert_eq!(session.scenario, Scenario::default());
    assert_eq!(session.seed, session.scenario.seed);
    assert!(session.warnings.is_empty());
    assert!(session.watch.is_none());
    assert!(session.outcome.is_none());
}

#[test]
fn esc_from_the_builder_drops_the_session() {
    let mut app = app_with_arena(4);
    app.handle_key(GameKey::Char('r'));

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::MainMenu);
    assert!(
        app.arena.is_none(),
        "a scenario outliving its screen would be fought by the next session"
    );
}

#[test]
fn the_arena_needs_no_running_game() {
    // The screen hangs off the main menu, where `App::game` is `None` — so
    // anything it reads has to come from somewhere other than a `Game`.
    let mut app = app_with_arena(5);
    assert!(app.game.is_none());

    app.handle_key(GameKey::Char('r'));

    assert_eq!(app.mode, Mode::ArenaBuilder);
}

#[test]
fn an_arena_fight_opens_the_battle_screen() {
    let app = app_fighting(10, scenario(6, 1, &[("glitch", 1)], 1));

    assert_eq!(app.mode, Mode::Battle, "{:?}", app.status_line);
    assert!(app.game.as_ref().unwrap().has_active_battle());
    assert_eq!(rounds_seen(&app), 0, "nobody has acted yet");
}

#[test]
fn winning_an_arena_fight_lands_on_the_result() {
    let mut app = app_fighting(11, scenario(20, 1, &[("sprite", 1)], 3));

    fight_to_the_end(&mut app);

    assert_eq!(app.mode, Mode::ArenaResult);
    let record = app.arena.as_ref().unwrap().outcome.as_ref().unwrap();
    assert!(record.won, "{record:?}");
    assert!(record.rounds > 0);
    assert!(
        !record.transcript.is_empty(),
        "the blow-by-blow is what the screen is for"
    );
}

#[test]
fn an_arena_fight_writes_no_save() {
    let mut app = app_fighting(12, scenario(20, 1, &[("sprite", 1)], 3));
    assert!(
        app.current_save_path.is_none(),
        "a session opened from the main menu has no slot to write to"
    );

    fight_to_the_end(&mut app);

    assert!(app.current_save_path.is_none());
    assert!(
        app.list_saves().is_empty(),
        "an arena fight left a save behind"
    );
}

#[test]
fn an_arena_fight_writes_no_profile() {
    // Asserted on the file rather than on `App::profile`, because the
    // omission being tested is the *write*: a rung earned in the arena that
    // reached `profile.ron` would be paid out to every future new game.
    let mut app = app_fighting(13, scenario(30, 1, &[("wintermute", 1)], 5));
    let profile_path = app.profile_path.clone();
    let _ = std::fs::remove_file(&profile_path);

    fight_to_the_end(&mut app);

    assert_eq!(app.mode, Mode::ArenaResult);
    assert!(
        app.arena.as_ref().unwrap().outcome.as_ref().unwrap().won,
        "the fixture is meant to kill the boss, or it asserts nothing"
    );
    assert!(
        !profile_path.exists(),
        "an arena kill was written to the cross-run profile"
    );
}

#[test]
fn an_arena_loss_writes_no_run_history() {
    // A `Save` player source can carry Permadeath in, so a lost arena fight
    // is a reachable `is_game_over` — and it belongs on the result screen.
    let assets_dir = test_assets_dir();
    let path = std::env::temp_dir().join("feral_processes_arena_permadeath.bin");
    Game::new(9, DifficultyMode::Permadeath, &assets_dir)
        .unwrap()
        .save(&path)
        .unwrap();

    let mut app = app_fighting(
        14,
        Scenario {
            player: PlayerSource::Save(path.clone()),
            ..scenario(1, 1, &[("wintermute", 3)], 7)
        },
    );
    let history_path = app.history_path.clone();
    let _ = std::fs::remove_file(&history_path);

    fight_to_the_end(&mut app);
    let _ = std::fs::remove_file(&path);

    assert_eq!(app.mode, Mode::ArenaResult, "not the real Game Over page");
    assert!(
        !history_path.exists(),
        "an arena loss was written to the run history"
    );
}

#[test]
fn a_failed_jack_out_still_counts_its_round() {
    // The regression the unconditional `settle_after_round` fixes: a refused
    // flee resolves a round the tail never saw, and the HP sample for it was
    // lost. `Watch::rounds` is what a missed `observe` loses.
    //
    // A hopeless fight bottoms the escape chance out at `JACK_OUT_CHANCE_MIN`
    // rather than at zero, so the seed that refuses is hunted rather than
    // asserted — deterministically, since the arena seeds `GameRng` itself.
    for scenario_seed in 0..10 {
        let mut app = app_fighting(15, scenario(1, 1, &[("wintermute", 3)], scenario_seed));
        let before = rounds_seen(&app);

        press(&mut app, GameKey::Char('j'));

        if app.mode == Mode::Battle {
            assert!(
                rounds_seen(&app) > before,
                "the round the refused jack-out cost went unobserved"
            );
            return;
        }
    }
    panic!("no seed under 10 refused a jack-out, so this asserts nothing");
}

#[test]
fn refighting_keeps_the_seed() {
    let mut app = app_fighting(20, scenario(20, 1, &[("sprite", 1)], 3));
    fight_to_the_end(&mut app);

    press(&mut app, GameKey::Char('r'));

    assert_eq!(app.mode, Mode::Battle);
    assert_eq!(app.arena.as_ref().unwrap().seed, 3);
}

#[test]
fn the_next_seed_key_advances_by_one() {
    // The manual version of a rep: `arena::run` runs rep *n* at
    // `scenario.seed + n`, so `[N]` has to be the same increment or a loss
    // seed found here would not replay in the headless run.
    let mut app = app_fighting(21, scenario(20, 1, &[("sprite", 1)], 3));
    fight_to_the_end(&mut app);

    press(&mut app, GameKey::Char('n'));

    assert_eq!(app.mode, Mode::Battle);
    assert_eq!(app.arena.as_ref().unwrap().seed, 4);
    assert_eq!(
        app.arena.as_ref().unwrap().scenario.seed,
        3,
        "the file the author is building must not be rewritten by a reseed"
    );
}

#[test]
fn esc_from_the_result_returns_to_the_builder_with_the_scenario_intact() {
    let built = scenario(20, 1, &[("sprite", 1)], 3);
    let mut app = app_fighting(22, built.clone());
    fight_to_the_end(&mut app);

    press(&mut app, GameKey::Esc);

    assert_eq!(app.mode, Mode::ArenaBuilder);
    assert_eq!(app.arena.as_ref().unwrap().scenario, built);
}

#[test]
fn a_refight_starts_from_a_whole_party() {
    // The regression `arena::run` already guards with a fresh `Game` per
    // rep. Asserted on behaviour — a transcript line naming the companion —
    // rather than on a count, since a lopsided fight can be won without it
    // ever swinging.
    let mut built = scenario(1, 4, &[("sub_process", 6)], 11);
    built.party = vec![feral_processes_engine::arena::CompanionSpec {
        species: "glitch".into(),
        level: 1,
        ..Default::default()
    }];
    let mut app = app_fighting(23, built);

    fight_to_the_end(&mut app);
    let first = app.arena.as_ref().unwrap().outcome.clone().unwrap();
    press(&mut app, GameKey::Char('r'));
    fight_to_the_end(&mut app);
    let second = app.arena.as_ref().unwrap().outcome.clone().unwrap();

    let swung = |r: &feral_processes_engine::arena::RepRecord| {
        r.transcript.iter().any(|l| l.contains("Glitch"))
    };
    assert!(swung(&first), "{:?}", first.transcript);
    assert!(
        swung(&second),
        "the refight fielded no companion — the `Game` was carried over: {:?}",
        second.transcript
    );
}

#[test]
fn jacking_out_records_a_loss() {
    // Matching the headless path, where a fled fight leaves the pack
    // standing and `Watch::finish` reads the opponents. An abandon that
    // counted as neither would be a third notion of an outcome.
    for scenario_seed in 0..10 {
        let mut app = app_fighting(24, scenario(20, 1, &[("sprite", 1)], scenario_seed));

        press(&mut app, GameKey::Char('j'));

        if app.mode == Mode::ArenaResult {
            let record = app.arena.as_ref().unwrap().outcome.as_ref().unwrap();
            assert!(!record.won, "a fled fight is not a win: {record:?}");
            return;
        }
    }
    panic!("no seed under 10 escaped, so this asserts nothing");
}

#[test]
fn the_staging_warnings_survive_the_fight() {
    // The result screen is where they are read, and nothing is ever capped
    // — so a warning cleared when the battle opened would leave the tool
    // silently answering a question nobody asked.
    let mut app = app_fighting(25, scenario(1, 1, &[("glitch", 9)], 1));
    assert!(!app.arena.as_ref().unwrap().warnings.is_empty());

    fight_to_the_end(&mut app);

    assert_eq!(app.mode, Mode::ArenaResult);
    assert!(!app.arena.as_ref().unwrap().warnings.is_empty());
}

/// An open builder holding `scenario`, with nothing fought yet.
fn app_building(seed: u32, scenario: Scenario) -> App {
    let mut app = app_with_arena(seed);
    app.handle_key(GameKey::Char('r'));
    let session = app.arena.as_mut().unwrap();
    session.seed = scenario.seed;
    session.scenario = scenario;
    app
}

fn row_kinds(app: &App) -> Vec<ArenaRowKind> {
    app.arena_builder_rows()
        .into_iter()
        .map(|r| r.kind)
        .collect()
}

/// Puts the highlight on the row of `kind`, the way Down would.
fn highlight(app: &mut App, kind: ArenaRowKind) {
    app.menu_selected = row_kinds(app)
        .iter()
        .position(|k| *k == kind)
        .unwrap_or_else(|| panic!("{kind:?} is not a row right now: {:?}", row_kinds(app)));
}

#[test]
fn a_save_player_hides_the_loadout_rows() {
    // The engine treats an authored loadout beside a save as an *error*
    // rather than ignoring it, so the rows have to be gone, not inert.
    let mut app = app_building(
        30,
        Scenario {
            player: PlayerSource::Save("saves/whatever.bin".into()),
            ..scenario(1, 1, &[("glitch", 1)], 0)
        },
    );

    let kinds = row_kinds(&app);

    for hidden in [
        ArenaRowKind::PlayerLevel,
        ArenaRowKind::PlayerZone,
        ArenaRowKind::AddEquip,
        ArenaRowKind::AddInventory,
        ArenaRowKind::AddParty,
    ] {
        assert!(!kinds.contains(&hidden), "{hidden:?} survived: {kinds:?}");
    }
    // And they come back when the source does.
    highlight(&mut app, ArenaRowKind::PlayerSource);
    app.handle_key(GameKey::Right);
    assert!(row_kinds(&app).contains(&ArenaRowKind::PlayerLevel));
}

#[test]
fn the_row_under_the_highlight_is_the_row_that_changes() {
    // The bug hidden rows cause: with a save source the third row is the
    // first opponent, not the player's level — five rows have gone.
    let mut app = app_building(
        31,
        Scenario {
            player: PlayerSource::Save("saves/whatever.bin".into()),
            ..scenario(1, 1, &[("glitch", 2)], 0)
        },
    );
    app.menu_selected = 2;
    assert_eq!(row_kinds(&app)[2], ArenaRowKind::Opponent(0));

    app.handle_key(GameKey::Right);

    let s = &app.arena.as_ref().unwrap().scenario;
    assert_eq!(
        s.opponents[0].count, 3,
        "the label's row is not the one that moved"
    );
}

#[test]
fn right_on_an_opponent_row_raises_its_count() {
    let mut app = app_building(32, scenario(1, 1, &[("glitch", 2)], 0));
    highlight(&mut app, ArenaRowKind::Opponent(0));

    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Left);

    assert_eq!(app.arena.as_ref().unwrap().scenario.opponents[0].count, 3);
}

#[test]
fn an_opponent_count_stops_at_max_group_size() {
    // Past it `build_opponents` hard-errors rather than warning, so a
    // builder that let you author it would produce an unfightable scenario.
    let mut app = app_building(33, scenario(1, 1, &[("glitch", MAX_GROUP_SIZE)], 0));
    highlight(&mut app, ArenaRowKind::Opponent(0));

    app.handle_key(GameKey::Right);

    assert_eq!(
        app.arena.as_ref().unwrap().scenario.opponents[0].count,
        MAX_GROUP_SIZE
    );
}

#[test]
fn a_count_never_drops_to_zero() {
    let mut app = app_building(34, scenario(1, 1, &[("glitch", 1)], 0));
    highlight(&mut app, ArenaRowKind::Opponent(0));

    app.handle_key(GameKey::Left);

    assert_eq!(
        app.arena.as_ref().unwrap().scenario.opponents[0].count,
        1,
        "`build_opponents` refuses a count of 0 outright"
    );
}

#[test]
fn backspace_removes_the_highlighted_party_row() {
    let mut built = scenario(1, 1, &[("glitch", 1)], 0);
    built.party = ["glitch", "sprite"]
        .into_iter()
        .map(|species| feral_processes_engine::arena::CompanionSpec {
            species: species.into(),
            level: 1,
            ..Default::default()
        })
        .collect();
    let mut app = app_building(35, built);
    highlight(&mut app, ArenaRowKind::Party(0));

    app.handle_key(GameKey::Backspace);

    let party = &app.arena.as_ref().unwrap().scenario.party;
    assert_eq!(party.len(), 1);
    assert_eq!(party[0].species, "sprite");
}

#[test]
fn a_composition_past_the_zone_ceiling_is_still_authorable() {
    // Warning rather than capping is the point of the tool: "what if zone 1
    // threw nine at me" is the question it exists to answer.
    let mut app = app_building(36, scenario(1, 1, &[("glitch", 8)], 0));
    highlight(&mut app, ArenaRowKind::Opponent(0));

    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Char('f'));

    assert_eq!(app.mode, Mode::Battle);
    assert_eq!(app.arena.as_ref().unwrap().scenario.opponents[0].count, 9);
    assert!(
        !app.arena.as_ref().unwrap().warnings.is_empty(),
        "nine at zone 1 must be built and warned about"
    );
}

#[test]
fn switching_off_a_fresh_player_clears_the_loadout_it_authored() {
    // A save or template brings its whole run across, and `Scenario`'s own
    // validation refuses a loadout beside one — so a hidden row left in the
    // struct would write a file that will not load back.
    let mut built = scenario(5, 1, &[("glitch", 1)], 0);
    built.party = vec![feral_processes_engine::arena::CompanionSpec {
        species: "glitch".into(),
        level: 1,
        ..Default::default()
    }];
    let mut app = app_building(37, built);
    app.install_dev_templates(DevTemplates {
        names: vec!["extraction".to_string()],
        resolve: |_| Err("not in this test".to_string()),
    });
    highlight(&mut app, ArenaRowKind::PlayerSource);

    app.handle_key(GameKey::Right);

    let s = &app.arena.as_ref().unwrap().scenario;
    assert_eq!(s.player, PlayerSource::Template("extraction".into()));
    assert!(s.party.is_empty(), "the authored party survived the switch");
}

/// Opens the picker by putting the highlight on `kind` and pressing Enter.
fn open_pick(app: &mut App, kind: ArenaRowKind) {
    highlight(app, kind);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::ArenaPick, "{kind:?} did not open a picker");
}

/// Picks the row whose label starts with `id`.
fn pick(app: &mut App, id: &str) {
    let rows = app.arena_pick_rows();
    let idx = rows
        .iter()
        .position(|r| r.starts_with(id))
        .unwrap_or_else(|| panic!("{id} is not offered: {rows:?}"));
    app.menu_selected = idx;
    app.handle_key(GameKey::Enter);
}

#[test]
fn the_picker_lists_species_without_a_running_game() {
    // The whole reason the catalogue exists: the screen hangs off the main
    // menu, where there is no `Game` to ask for `species_defs()`.
    let mut app = app_building(40, scenario(1, 1, &[("glitch", 1)], 0));
    assert!(app.game.is_none());

    open_pick(&mut app, ArenaRowKind::AddOpponent);

    assert!(!app.arena_pick_rows().is_empty());
}

#[test]
fn picking_a_species_appends_an_opponent_row() {
    let mut app = app_building(41, scenario(1, 1, &[("glitch", 1)], 0));
    open_pick(&mut app, ArenaRowKind::AddOpponent);

    pick(&mut app, "sprite");

    assert_eq!(app.mode, Mode::ArenaBuilder);
    let opponents = &app.arena.as_ref().unwrap().scenario.opponents;
    assert_eq!(opponents.len(), 2);
    assert_eq!(opponents[1].species, "sprite");
    assert_eq!(opponents[1].count, 1, "a new group starts at one");
}

#[test]
fn picking_into_an_existing_row_replaces_its_id_and_keeps_its_count() {
    // The count is the tuning dial; losing it on an id change is the bug.
    let mut app = app_building(42, scenario(1, 1, &[("glitch", 5)], 0));
    open_pick(&mut app, ArenaRowKind::Opponent(0));

    pick(&mut app, "sprite");

    let opponents = &app.arena.as_ref().unwrap().scenario.opponents;
    assert_eq!(opponents.len(), 1, "an edit is not an append");
    assert_eq!(opponents[0].species, "sprite");
    assert_eq!(opponents[0].count, 5);
}

#[test]
fn the_equip_picker_offers_only_equippable_items() {
    let mut app = app_building(43, scenario(1, 1, &[("glitch", 1)], 0));

    open_pick(&mut app, ArenaRowKind::AddEquip);
    let worn = app.arena_pick_rows();
    app.handle_key(GameKey::Esc);
    open_pick(&mut app, ArenaRowKind::AddInventory);
    let cargo = app.arena_pick_rows();

    assert!(!worn.is_empty());
    assert!(
        worn.len() < cargo.len(),
        "the equip picker offered every item in the catalogue"
    );
    assert!(
        worn.iter().all(|row| cargo.contains(row)),
        "the equip picker offered something cargo would not hold"
    );
}

#[test]
fn esc_from_the_picker_returns_to_the_builder_adding_nothing() {
    let mut app = app_building(44, scenario(1, 1, &[("glitch", 1)], 0));
    open_pick(&mut app, ArenaRowKind::AddParty);

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::ArenaBuilder);
    assert!(app.arena.as_ref().unwrap().scenario.party.is_empty());
}

#[test]
fn a_picked_companion_and_item_land_at_their_defaults() {
    let mut app = app_building(45, scenario(1, 1, &[("glitch", 1)], 0));

    open_pick(&mut app, ArenaRowKind::AddParty);
    pick(&mut app, "sprite");
    open_pick(&mut app, ArenaRowKind::AddInventory);
    let first = app.arena_pick_rows()[0].clone();
    pick(&mut app, &first);

    let s = &app.arena.as_ref().unwrap().scenario;
    assert_eq!(s.party[0].species, "sprite");
    assert_eq!(s.party[0].level, 1);
    assert_eq!(s.inventory[0].qty, 1);
}

/// An open builder whose `dev-arenas/` is a scratch copy of the shipped
/// one, so a save in a test cannot rewrite a checked-in fixture.
fn app_with_scratch_arenas(seed: u32) -> App {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("feral_processes_arenas_{seed}_{unique}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shipped = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dev-arenas");
    for entry in std::fs::read_dir(&shipped).unwrap().flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "ron") {
            std::fs::copy(&path, dir.join(path.file_name().unwrap())).unwrap();
        }
    }

    let mut app = App::new(
        test_assets_dir(),
        std::env::temp_dir().join(format!("feral_processes_arenas_saves_{seed}_{unique}")),
        std::env::temp_dir().join(format!("feral_processes_arenas_{seed}_{unique}.log")),
        std::env::temp_dir().join(format!(
            "feral_processes_arenas_{seed}_{unique}_profile.ron"
        )),
        dir,
        std::env::temp_dir().join(format!(
            "feral_processes_arenas_{seed}_{unique}_telemetry.jsonl"
        )),
    );
    app.arena_enabled = true;
    app.handle_key(GameKey::Char('r'));
    app
}

fn arenas_dir(app: &App) -> PathBuf {
    app.arenas_dir.clone()
}

/// Loads the scenario whose name is `name` through the real picker.
fn load_scenario(app: &mut App, name: &str) {
    app.handle_key(GameKey::Char('l'));
    assert_eq!(app.mode, Mode::ArenaLoad);
    let rows = app.arena_load_rows();
    let idx = rows
        .iter()
        .position(|r| r == name)
        .unwrap_or_else(|| panic!("{name} is not listed: {rows:?}"));
    app.menu_selected = idx;
    app.handle_key(GameKey::Enter);
}

/// Saves the open scenario under `name` through the real filename screen.
fn save_scenario(app: &mut App, name: &str) {
    app.handle_key(GameKey::Char('s'));
    assert_eq!(app.mode, Mode::ArenaSave);
    for c in name.chars() {
        app.handle_key(GameKey::Char(c));
    }
    app.handle_key(GameKey::Enter);
}

#[test]
fn a_shipped_scenario_round_trips_through_the_builder() {
    // What makes the two tools one library rather than two: the struct the
    // screen edits is the struct the `.ron` holds and the bin runs.
    let mut app = app_with_scratch_arenas(50);

    load_scenario(&mut app, "opening-fight");
    assert_eq!(app.mode, Mode::ArenaBuilder, "{:?}", app.status_line);
    let loaded = app.arena.as_ref().unwrap().scenario.clone();
    save_scenario(&mut app, "round-trip");

    assert_eq!(app.mode, Mode::ArenaBuilder, "{:?}", app.status_line);
    let written = Scenario::load(&arenas_dir(&app).join("round-trip.ron")).unwrap();
    assert_eq!(loaded, written);
}

#[test]
fn a_loaded_template_scenario_keeps_saying_template() {
    // The trap `start_arena_fight`'s clone avoids: resolving the template
    // into the session's own scenario would rewrite the author's file into
    // a path under `saves/`.
    let mut app = app_with_scratch_arenas(51);
    load_scenario(&mut app, "geared-vs-boss");
    assert!(matches!(
        app.arena.as_ref().unwrap().scenario.player,
        PlayerSource::Template(_)
    ));

    save_scenario(&mut app, "still-a-template");

    let written = Scenario::load(&arenas_dir(&app).join("still-a-template.ron")).unwrap();
    assert!(
        matches!(written.player, PlayerSource::Template(_)),
        "{:?}",
        written.player
    );
}

#[test]
fn a_malformed_scenario_stays_on_the_picker_with_the_reason() {
    let mut app = app_with_scratch_arenas(52);
    std::fs::write(arenas_dir(&app).join("broken.ron"), "( opponents: [ ").unwrap();

    app.handle_key(GameKey::Char('l'));
    let rows = app.arena_load_rows();
    app.menu_selected = rows.iter().position(|r| r == "broken").unwrap();
    app.handle_key(GameKey::Enter);

    assert_eq!(
        app.mode,
        Mode::ArenaLoad,
        "a bad file must not close the picker"
    );
    let status = app.status_line.clone().unwrap_or_default();
    assert!(status.contains("broken"), "{status}");
}

#[test]
fn saving_over_an_existing_name_overwrites_it() {
    let mut app = app_with_scratch_arenas(53);
    load_scenario(&mut app, "opening-fight");
    save_scenario(&mut app, "twice");

    highlight(&mut app, ArenaRowKind::Opponent(0));
    app.handle_key(GameKey::Right);
    let edited = app.arena.as_ref().unwrap().scenario.clone();
    save_scenario(&mut app, "twice");

    let written = Scenario::load(&arenas_dir(&app).join("twice.ron")).unwrap();
    assert_eq!(
        written, edited,
        "the first version survived the second save"
    );
}

#[test]
fn a_filename_that_is_a_path_is_refused() {
    let mut app = app_with_scratch_arenas(54);
    load_scenario(&mut app, "opening-fight");

    save_scenario(&mut app, "../escaped");

    assert_eq!(
        app.mode,
        Mode::ArenaSave,
        "the screen must hold the mistake"
    );
    assert!(app.status_line.is_some());
    assert!(!arenas_dir(&app).join("../escaped.ron").exists());
}

#[test]
fn the_whole_loop_walks_from_a_shipped_scenario_to_a_new_one() {
    // The tool's stated purpose end to end, through the real keys: load a
    // measured fight, play it, reseed, adjust the composition by feel, and
    // write the result back out for the headless bin to run fifty times.
    let mut app = app_with_scratch_arenas(60);

    load_scenario(&mut app, "opening-fight");
    press(&mut app, GameKey::Char('f'));
    fight_to_the_end(&mut app);
    assert_eq!(app.mode, Mode::ArenaResult);

    press(&mut app, GameKey::Char('n'));
    assert_eq!(app.arena.as_ref().unwrap().seed, 2);
    fight_to_the_end(&mut app);
    press(&mut app, GameKey::Esc);
    assert_eq!(app.mode, Mode::ArenaBuilder);

    open_pick(&mut app, ArenaRowKind::AddOpponent);
    pick(&mut app, "glitch");
    open_pick(&mut app, ArenaRowKind::AddParty);
    pick(&mut app, "sprite");
    open_pick(&mut app, ArenaRowKind::AddEquip);
    let weapon = app.arena_pick_rows()[0].clone();
    pick(&mut app, &weapon);

    press(&mut app, GameKey::Char('f'));
    assert_eq!(app.mode, Mode::Battle, "{:?}", app.status_line);
    fight_to_the_end(&mut app);
    press(&mut app, GameKey::Esc);
    save_scenario(&mut app, "built-by-feel");

    // The far side of the loop: what the screen wrote is what the bin runs.
    let written = Scenario::load(&arenas_dir(&app).join("built-by-feel.ron")).unwrap();
    assert_eq!(written.opponents.len(), 2);
    assert_eq!(written.party.len(), 1);
    assert_eq!(written.equip.len(), 1);
    let (report, _) = feral_processes_engine::arena::run(
        &written,
        &test_assets_dir(),
        feral_processes_engine::arena::RunOptions::default(),
    )
    .expect("the bin's path");
    assert_eq!(report.reps.len(), written.reps as usize);
}

#[test]
fn dev_templates_install_whether_or_not_the_gate_is_open() {
    // The launcher installs unconditionally: the gate decides visibility,
    // and installing only when gated would make one flag mean two things.
    let mut app = test_app(6);
    app.install_dev_templates(DevTemplates {
        names: vec!["extraction".to_string()],
        resolve: |_| Ok(test_assets_dir()),
    });
    assert!(app.dev_templates.is_some());
}

fn row_labels(app: &App) -> Vec<String> {
    app.arena_builder_rows()
        .into_iter()
        .map(|r| r.label)
        .collect()
}

/// How many states the `Encounter:` row cycles through: Authored, Field,
/// Stack, Lair. Named so a test that means "all the way round" says so.
const ENCOUNTER_CYCLE_STATES: usize = 4;

/// Steps the `Encounter:` row `n` times to the right.
fn cycle_encounter(app: &mut App, n: usize) {
    highlight(app, ArenaRowKind::Encounter);
    for _ in 0..n {
        app.handle_key(GameKey::Right);
    }
}

#[test]
fn cycling_to_a_rolled_encounter_hides_the_opponent_rows() {
    // `Scenario::validate` refuses a file holding both, so the rows have to
    // be gone rather than inert — the same rule the loadout rows follow.
    let mut app = app_building(60, scenario(1, 1, &[("glitch", 1)], 0));

    cycle_encounter(&mut app, 1);

    let kinds = row_kinds(&app);
    assert!(
        !kinds.iter().any(|k| matches!(k, ArenaRowKind::Opponent(_))),
        "{kinds:?}"
    );
    assert!(!kinds.contains(&ArenaRowKind::AddOpponent), "{kinds:?}");
    assert!(
        !row_labels(&app).iter().any(|l| l.starts_with("Against:")),
        "{:?}",
        row_labels(&app)
    );
    assert!(app.arena.as_ref().unwrap().scenario.opponents.is_empty());
}

#[test]
fn cycling_back_to_authored_restores_an_opponent_row() {
    // Every state the cycle can reach has to be one `save` will accept, so
    // this walks the *whole* cycle rather than a fixed number of steps — a
    // fourth state added without a step here would have read as the cycle
    // being broken rather than as the test being stale.
    let mut app = app_building(61, scenario(1, 1, &[("glitch", 1)], 0));

    cycle_encounter(&mut app, ENCOUNTER_CYCLE_STATES);

    let s = &app.arena.as_ref().unwrap().scenario;
    assert!(s.encounter.is_none());
    assert_eq!(s.opponents.len(), 1);
    assert!(row_kinds(&app).contains(&ArenaRowKind::AddOpponent));
}

#[test]
fn a_stack_encounter_shows_a_depth_row_and_a_field_one_does_not() {
    let mut app = app_building(62, scenario(1, 1, &[("glitch", 1)], 0));

    cycle_encounter(&mut app, 1);
    let field = row_kinds(&app);
    assert!(field.contains(&ArenaRowKind::EncounterBiome), "{field:?}");
    assert!(!field.contains(&ArenaRowKind::EncounterDepth), "{field:?}");

    cycle_encounter(&mut app, 1);
    let stack = row_kinds(&app);
    assert!(stack.contains(&ArenaRowKind::EncounterDepth), "{stack:?}");

    highlight(&mut app, ArenaRowKind::EncounterDepth);
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Left);
    app.handle_key(GameKey::Left);
    assert_eq!(
        app.arena.as_ref().unwrap().scenario.encounter,
        Some(Encounter::Stack {
            biome: Biome::Deadlock,
            depth: 1,
        }),
        "depth floors at 1, and the biome beside it is untouched"
    );
}

#[test]
fn a_lair_encounter_carries_its_biome_over_and_nudges_its_own_depth() {
    // The depth dial is an `if let` under a catch-all arm, so a Lair the
    // pattern forgot compiles clean and ships a row that does nothing. This
    // asserts the number actually moved, not that the row is drawn.
    let mut app = app_building(65, scenario(1, 1, &[("glitch", 1)], 0));

    cycle_encounter(&mut app, 3);
    let kinds = row_kinds(&app);
    assert!(kinds.contains(&ArenaRowKind::EncounterDepth), "{kinds:?}");

    highlight(&mut app, ArenaRowKind::EncounterDepth);
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Right);
    assert_eq!(
        app.arena.as_ref().unwrap().scenario.encounter,
        Some(Encounter::Lair {
            biome: Biome::Deadlock,
            depth: 3,
        }),
        "the biome survives the step onto Lair and the depth dial moves"
    );
}

#[test]
fn the_biome_picker_offers_only_biomes_something_lives_in() {
    // Read off the live species db rather than hardcoded, so the two
    // clauses `habitat_pools` early-returns on are the two clauses here.
    let mut app = app_building(63, scenario(1, 1, &[("glitch", 1)], 0));
    cycle_encounter(&mut app, 1);

    open_pick(&mut app, ArenaRowKind::EncounterBiome);

    let rows = app.arena_pick_rows();
    assert!(rows.contains(&"OpenGrid".to_string()), "{rows:?}");
    assert!(
        !rows.contains(&"Platform".to_string()),
        "no species lives on a base slab: {rows:?}"
    );
    for unwalkable in ["DataVoid", "BlackIce"] {
        assert!(
            !rows.contains(&unwalkable.to_string()),
            "{unwalkable} is a hole in the map: {rows:?}"
        );
    }
}

#[test]
fn picking_a_biome_replaces_the_encounters_biome_and_keeps_its_depth() {
    // The depth beside it is the tuning dial, the same rule the counts and
    // levels already follow.
    let mut app = app_building(64, scenario(1, 1, &[("glitch", 1)], 0));
    cycle_encounter(&mut app, 2);
    highlight(&mut app, ArenaRowKind::EncounterDepth);
    for _ in 0..4 {
        app.handle_key(GameKey::Right);
    }

    open_pick(&mut app, ArenaRowKind::EncounterBiome);
    pick(&mut app, "NullSector");

    assert_eq!(
        app.arena.as_ref().unwrap().scenario.encounter,
        Some(Encounter::Stack {
            biome: Biome::NullSector,
            depth: 5,
        })
    );
}
