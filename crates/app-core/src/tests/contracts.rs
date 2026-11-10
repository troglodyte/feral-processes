//! The Contracts page (`b` → JOBS): when it is on the rail, its two tabs,
//! and the three verbs.

use super::support::*;
use crate::*;

/// Whether the Contracts page is on `b`'s rail right now.
fn on_the_rail(app: &App) -> bool {
    Hub::Base
        .kind()
        .pages(app)
        .iter()
        .any(|p| p.key == HubPageKey::Named("Contracts"))
}

#[test]
fn the_page_is_off_the_rail_with_no_broker_and_nothing_in_hand() {
    let mut app = test_app(3101);
    // Past onboarding: a run still running the chain always has a mission in
    // hand, which is the case the test below covers.
    skip_tutorial(&mut app);
    assert!(
        !on_the_rail(&app),
        "a page that opens on nothing is what the rail's gate exists to stop"
    );
}

/// The other half, and new with the onboarding chain: a fresh run holds its
/// first mission before it has built anything, so the page is there from the
/// first keypress. Without it the chain is handed out somewhere the player
/// has no way to reach.
#[test]
fn the_page_is_on_the_rail_from_the_first_tick_by_the_onboarding_mission() {
    let app = test_app(3122);
    assert!(
        app.game
            .as_ref()
            .unwrap()
            .active_contracts()
            .iter()
            .any(|row| row.tutorial),
        "a new run holds the chain's first mission"
    );
    assert!(on_the_rail(&app));
}

#[test]
fn a_contract_in_hand_keeps_the_page_underground() {
    let mut app = app_at_a_contract_broker(3104, true);
    assert!(
        app.game.as_mut().unwrap().contract_board().is_some(),
        "the board is the sector's bulletin, and four frames down the party \
         is still in the sector"
    );
    assert!(
        on_the_rail(&app),
        "mission status is the question worth answering underground"
    );
}

/// The page is what makes the board reachable at all, so "you can check
/// your missions from anywhere" is really a claim about the rail.
#[test]
fn the_page_is_listed_across_the_map_from_the_base() {
    let mut app = app_at_a_contract_broker(3120, false);
    walk_far_from_the_base(&mut app);
    assert_eq!(
        app.broker_reach(),
        BrokerReach::OffBase,
        "the fixture has to actually leave the slab"
    );
    assert!(
        on_the_rail(&app),
        "the board is readable from anywhere, so the page that shows it is too"
    );
}

/// The refusal a player will actually hit: they read the board out in the
/// field and press Enter. The wording has to send them home rather than
/// claim the contract does not exist.
#[test]
fn taking_an_offer_away_from_the_base_says_where_to_go() {
    let mut app = app_at_a_contract_broker(3121, false);
    let offered = app.contract_sections().1;
    assert!(!offered.is_empty(), "the fixture needs a board");

    walk_far_from_the_base(&mut app);
    open_hub_screen(&mut app, Mode::Contracts);
    assert_eq!(
        app.contract_sections().1.len(),
        offered.len(),
        "the offers are still on screen out here"
    );

    pick_row(&mut app, 0);
    assert!(app.game.as_ref().unwrap().active_contracts().is_empty());
    let said = app.status_line.clone().expect("a refusal is reported");
    assert!(
        said.contains("base"),
        "the refusal has to name the errand, not just decline: {said:?}"
    );
    assert_eq!(app.mode, Mode::Contracts, "the tab stays open");
}

#[test]
fn esc_on_either_tab_goes_to_the_rail_then_closes_the_hub() {
    for tab in [Mode::ContractsHeld, Mode::Contracts] {
        let mut app = app_at_a_contract_broker(3105, false);
        open_hub_screen(&mut app, tab);
        app.handle_key(GameKey::Esc);
        assert!(app.hub_on_rail, "Esc on the page goes to the rail");
        assert_eq!(app.mode, tab);
        app.handle_key(GameKey::Esc);
        assert_eq!(app.mode, Mode::Playing);
    }
}

#[test]
fn picking_an_offer_accepts_it_and_the_tab_stays_open() {
    let mut app = app_at_a_contract_broker(3106, false);
    open_hub_screen(&mut app, Mode::Contracts);
    let offered = app.game.as_mut().unwrap().contract_board().unwrap();
    assert!(!offered.is_empty());

    pick_row(&mut app, 0);
    assert_eq!(
        app.mode,
        Mode::Contracts,
        "the tab stays open so several can be taken in one visit"
    );
    let held = app.game.as_ref().unwrap().active_contracts();
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].id, offered[0].id);
    assert!(
        !app.game
            .as_mut()
            .unwrap()
            .contract_board()
            .unwrap()
            .iter()
            .any(|row| row.id == held[0].id),
        "an accepted contract drops off the offers, so the row it vacated \
         cannot be picked twice"
    );
}

/// `[A]` on an onboarding mission refuses with a sentence, on both surfaces.
/// A silent no-op reads as the key being broken, and the engine's own
/// refusal is a bare `false` that cannot reach the log.
#[test]
fn giving_back_an_onboarding_mission_is_refused_with_a_sentence() {
    // Not `app_at_a_contract_broker`, which skips the chain: this is about a
    // run still running it, and it needs no Broker — the mission is in hand
    // from tick 0.
    let mut app = test_app(3123);
    open_hub_screen(&mut app, Mode::ContractsHeld);
    let held = app.game.as_ref().unwrap().active_contracts();
    assert_eq!(held.len(), 1, "one onboarding mission in hand");
    assert!(held[0].tutorial);

    app.handle_key(GameKey::Char('A'));

    let line = app.status_line.clone().expect("a refusal has a sentence");
    let lowered = line.to_lowercase();
    assert!(
        lowered.contains("onboarding") || lowered.contains("finish"),
        "the sentence says why: {line:?}"
    );
    // Both surfaces: `App::refuse` writes the log too, and asserting only on
    // `status_line` would pass against a bare `self.status_line = ...`.
    assert!(
        app.game
            .as_ref()
            .unwrap()
            .message_history(50)
            .iter()
            .any(|entry| entry.text.contains("cannot be given back")),
        "the refusal reaches the log the player scrolls back through — and on \
         a fragment the hand-out line does not share, or the ONBOARDING: \
         line would answer this on its own"
    );
    assert_eq!(
        app.game
            .as_ref()
            .unwrap()
            .active_contracts()
            .iter()
            .filter(|r| r.tutorial)
            .count(),
        1,
        "and it is still in hand"
    );
}

#[test]
fn abandoning_the_highlighted_held_contract_drops_it() {
    let mut app = app_at_a_contract_broker(3107, false);
    open_hub_screen(&mut app, Mode::Contracts);
    pick_row(&mut app, 0);
    assert_eq!(app.game.as_ref().unwrap().active_contracts().len(), 1);

    app.handle_key(GameKey::ShiftTab);
    assert_eq!(app.mode, Mode::ContractsHeld);
    app.handle_key(GameKey::Char('A'));
    assert!(
        app.game.as_ref().unwrap().active_contracts().is_empty(),
        "status: {:?}",
        app.status_line
    );
    assert_eq!(app.mode, Mode::ContractsHeld);
}

/// `[A]` is the Held tab's key: on the Board it names no contract you hold,
/// so it gives nothing back.
#[test]
fn a_on_the_board_gives_nothing_back() {
    let mut app = app_at_a_contract_broker(3109, false);
    open_hub_screen(&mut app, Mode::Contracts);
    pick_row(&mut app, 0);
    app.handle_key(GameKey::Char('A'));
    assert_eq!(app.game.as_ref().unwrap().active_contracts().len(), 1);
}

#[test]
fn a_refusal_reports_why_and_leaves_the_tab_open() {
    let mut app = app_at_a_contract_broker(3108, false);
    open_hub_screen(&mut app, Mode::Contracts);
    // Take everything the board offers, then try for one more.
    for _ in 0..MAX_ACTIVE_CONTRACTS + 1 {
        let offers = app.game.as_mut().unwrap().contract_board().unwrap().len();
        if offers == 0 {
            break;
        }
        pick_row(&mut app, 0);
    }
    assert_eq!(
        app.game.as_ref().unwrap().active_contracts().len(),
        MAX_ACTIVE_CONTRACTS.min(3),
        "status: {:?}",
        app.status_line
    );
    assert_eq!(app.mode, Mode::Contracts);
}

/// A town's job is one contract wherever it is read: on Contracts → Held
/// and on that town's Jobs tab, and giving it back from either drops it
/// from both.
#[test]
fn a_town_job_is_one_contract_on_held_and_on_the_towns_jobs_tab() {
    for from_jobs in [false, true] {
        let mut app = crate::tests::sites_hub::app_beside_birch(3110);
        let town = crate::tests::sites_hub::TOWN_B;
        let game = app.game.as_mut().unwrap();
        let rows = game.settlement_board(town).expect("beside the counter");
        let row = rows.iter().find(|r| !r.tutorial).expect("a giveable job");
        let id = row.id.clone();
        let accepted = game.accept_contract(&id, Some(town));
        assert!(accepted.is_ok(), "{accepted:?}");

        open_hub_screen(&mut app, Mode::ContractsHeld);
        let held = app.contract_sections().0;
        let on_held = held.iter().position(|r| r.id == id).expect("on Held");
        assert_eq!(held[on_held].issuer, Some(town));
        let jobs = app.game.as_ref().unwrap().town_contracts(town);
        assert_eq!(jobs.iter().map(|r| &r.id).collect::<Vec<_>>(), [&id]);

        if from_jobs {
            crate::tests::sites_hub::open_town_tab(&mut app, town, Mode::SiteJobs);
            highlight_row(&mut app, 0);
        } else {
            highlight_row(&mut app, on_held);
        }
        app.handle_key(GameKey::Char('A'));
        assert_eq!(app.status_line, None);
        let game = app.game.as_ref().unwrap();
        assert!(!game.active_contracts().iter().any(|r| r.id == id));
        assert!(game.town_contracts(town).is_empty());
    }
}

/// **A new run's first tick opens on its onboarding briefing.**
///
/// The full-screen notice is how a new player learns the chain exists at
/// all — the mission is in hand from tick 0, but nothing else points at it.
/// Asserted through `start_new_game` rather than `test_app`, which drains
/// the queue, so this is the screen a player actually sees.
#[test]
fn a_new_run_opens_on_the_first_missions_briefing() {
    let mut app = test_app(3124);
    app.start_new_game(DifficultyMode::Forgiving, &CharacterChoice::default());
    let mission = app
        .game
        .as_ref()
        .unwrap()
        .active_contracts()
        .into_iter()
        .find(|row| row.tutorial)
        .expect("a new run holds the chain's first mission");

    // The queue is drained by `after_tick`, so the briefing lands on the
    // first keypress rather than at the menu.
    app.handle_key(GameKey::Char('.'));

    assert_eq!(
        app.mode,
        Mode::Notification,
        "the briefing takes the screen"
    );
    let shown = app
        .pending_notification
        .clone()
        .expect("a notice is on screen");
    assert_eq!(shown.title, mission.name, "titled for the mission");
    assert!(
        shown.body.contains(&mission.description),
        "and carries the contract's own words rather than a second copy: {:?}",
        shown.body
    );
    assert!(
        shown.body.contains("[4]"),
        "it points at the key that opens the contracts pane: {:?}",
        shown.body
    );

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing, "and Esc returns to the map");
}
