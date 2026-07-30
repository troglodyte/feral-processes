//! Driving an intrusion from key presses to a resolved round.

use super::support::*;
use crate::*;

/// Scans seeds until one puts a wild program next to the player, then
/// bumps it to open a battle. Returns the app sitting in `Mode::Battle`
/// with the entry sounds already drained, so a caller can attribute
/// anything it observes afterwards to the key it pressed.
///
/// Seeds whose bump gathers more than one species are skipped: a pack may
/// now carry `MAX_ENEMY_GROUPS` groups even where each holds a single
/// member, and several tests below are about what the UI does with exactly
/// one group. Multi-group behaviour belongs to the engine's own tests,
/// which can build a pack directly instead of fishing for a seed.
fn battling_app() -> App {
    battling_app_with(|_| {})
}

/// Same seed search as `battling_app`, but `setup` runs on the fresh game
/// before anything walks toward battle — the only window some engine calls
/// (like `uninstall_routine`) allow, since they refuse once a battle is
/// active.
fn battling_app_with(setup: impl Fn(&mut Game)) -> App {
    for seed in 0..200u32 {
        let mut app = test_app(seed);
        setup(app.game.as_mut().unwrap());
        let game = app.game.as_mut().unwrap();
        let player = game.player_status().position;
        let target = game
            .view_entities(12, 12)
            .into_iter()
            .filter(|e| e.is_hostile && !e.is_tamed && !e.is_structure)
            .find(|e| (e.pos.0 - player.0).abs() + (e.pos.1 - player.1).abs() == 1);
        let Some(target) = target else { continue };
        app.handle_key(match (target.pos.0 - player.0, target.pos.1 - player.1) {
            (1, 0) => GameKey::Right,
            (-1, 0) => GameKey::Left,
            (0, 1) => GameKey::Down,
            _ => GameKey::Up,
        });
        let single_group = app
            .game
            .as_ref()
            .and_then(|g| g.battle_view())
            .is_some_and(|v| v.groups.len() == 1);
        if app.mode == Mode::Battle && single_group {
            let _ = app.take_sounds();
            // The opening narration is scrolling in, and a key pressed
            // during that skips rather than acting. Drain it so a caller's
            // first key is its own test's input, not a skip.
            app.finish_reveal();
            return app;
        }
    }
    panic!(
        "no seed under 200 put a lone wild program next to the player — encounter setup changed"
    );
}

/// The action set lives in the engine. If app-core or a renderer
/// hardcoded a key, the two frontends would drift the moment an action
/// was added — which is the exact failure this indirection exists to
/// prevent. So the keys under test are read from the engine rather
/// than written here.
///
/// Case handling is deliberately split. The per-slot prompts are
/// lowercase (`[a]ttack`, `[d]efend`), and uppercase `A`/`D` are the
/// party-wide commands — so those two must NOT fold. Every other battle
/// key still folds, since a shifted keypress there is a slip, and
/// swallowing it costs the player a round.
///
/// Asserts only that each key was routed at all — which action it
/// resolves to is the engine's business, and depends on the gear and
/// party the seed happens to hand out.
#[test]
fn battle_action_keys_come_from_the_engine_with_only_the_party_pair_case_sensitive() {
    let probe = battling_app();
    let game = probe.game.as_ref().unwrap();
    let per_slot: Vec<char> = game
        .battle_action_options(0)
        .iter()
        .map(|o| o.key)
        .collect();
    assert!(
        per_slot.contains(&'a') && per_slot.contains(&'d'),
        "the engine should always offer at least Attack and Defend, got {per_slot:?}"
    );
    let party: Vec<char> = game.battle_party_commands().iter().map(|c| c.key).collect();
    assert_eq!(party, vec!['A', 'D', 'j']);

    // Every key the engine advertises must route as pressed, and the
    // shifted form of each lowercase one must route too.
    let mut probes: Vec<char> = per_slot.clone();
    probes.extend(per_slot.iter().map(|k| k.to_ascii_uppercase()));
    probes.extend(party.iter().copied());
    probes.push('J');

    for key in probes {
        let mut app = battling_app();
        app.handle_key(GameKey::Char(key));
        let acted =
            !app.take_sounds().is_empty() || app.status_line.is_some() || app.mode != Mode::Battle;
        assert!(
            acted,
            "[{key}] is advertised by the engine, but the keypress was swallowed"
        );
    }
}

/// The complaint that started this work: with one group left there is no
/// focus-fire choice to make, so all-attack must resolve on the single
/// keypress instead of stopping to ask.
#[test]
fn all_attack_with_one_group_resolves_without_opening_the_target_picker() {
    let mut app = battling_app();
    assert_eq!(
        app.game
            .as_mut()
            .unwrap()
            .battle_view()
            .unwrap()
            .groups
            .len(),
        1,
        "a bump battle is a single group — test premise"
    );

    app.handle_key(GameKey::Char('A'));

    assert_ne!(
        app.mode,
        Mode::BattleTarget,
        "one group means no choice, so all-attack shouldn't open the picker"
    );
    assert!(
        app.pending_battle_action.is_none(),
        "nothing should be left pending once the round resolved"
    );
}

/// `D` is a party-wide command, not the per-slot Defend that `d` runs.
/// Both have to reach the engine.
#[test]
fn all_defend_resolves_the_round() {
    let mut app = battling_app();
    app.handle_key(GameKey::Char('D'));
    assert!(
        matches!(app.mode, Mode::Battle | Mode::Playing | Mode::GameOver),
        "all-defend plans every slot, so the round should have resolved; got {:?}",
        app.mode
    );
}

/// Picking an action that needs a target must not resolve the round on
/// the spot — it has to stop and ask which group.
#[test]
fn a_targeted_action_stops_to_ask_which_group() {
    let mut app = battling_app();
    app.handle_key(GameKey::Char('a'));
    assert_eq!(
        app.mode,
        Mode::BattleTarget,
        "Attack needs a target, so it should open the group picker"
    );

    // Esc backs out without spending the round.
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Battle);
    assert!(app.pending_battle_action.is_none());
}

/// Every action the engine offers must lead somewhere that can actually
/// complete. `[u]se item` originally routed to the map's inventory
/// screen, whose flow calls `Game::use_item` — which refuses outright
/// during an intrusion. The action appeared in the menu, swallowed the
/// player's pick, and did nothing.
#[test]
fn every_offered_action_reaches_a_state_that_can_complete_it() {
    let probe = battling_app();
    let options = probe.game.as_ref().unwrap().battle_action_options(0);
    assert!(
        options.iter().any(|o| o.kind == ActionKind::UseItem),
        "the starting kit holds consumables, so Use Item should be offered"
    );

    for option in options {
        if option.unavailable.is_some() {
            continue;
        }
        let mut app = battling_app();
        app.handle_key(GameKey::Char(option.key));
        assert_ne!(
            app.mode,
            Mode::Inventory,
            "[{}] dead-ends in the map inventory, which refuses to act mid-battle",
            option.key
        );
        // Either it resolved the round outright, or it opened a picker
        // that belongs to the battle flow.
        assert!(
            matches!(
                app.mode,
                Mode::Battle
                    | Mode::BattleTarget
                    | Mode::BattleItem
                    | Mode::BattleSpecial
                    | Mode::BattleAlly
                    | Mode::Playing
                    | Mode::GameOver
            ),
            "[{}] left the app in {:?}, which isn't part of the battle flow",
            option.key,
            app.mode
        );
    }
}

/// A Special needs both an ability and a group, and `action_from` is the
/// one place that pairing is enforced. Tested directly because the flow
/// that produces it needs a companion in the party, which every battle
/// reachable from `battling_app` lacks — the player is slot 0 and is
/// never offered Special.
#[test]
fn a_special_is_only_built_once_both_the_ability_and_a_target_are_known() {
    assert_eq!(
        action_from(
            ActionKind::Special,
            Collected {
                group: Some(2),
                ability: Some(1),
                ..Collected::default()
            }
        ),
        Some(BattleAction::Special {
            ability: 1,
            target: SpecialTarget::EnemyGroup { group: 2 }
        })
    );
    assert_eq!(
        action_from(
            ActionKind::Special,
            Collected {
                ally: Some(3),
                ability: Some(0),
                ..Collected::default()
            }
        ),
        Some(BattleAction::Special {
            ability: 0,
            target: SpecialTarget::Ally { slot: 3 }
        }),
        "a buff aimed at a party member targets that slot, with no group involved"
    );
    assert_eq!(
        action_from(
            ActionKind::Special,
            Collected {
                group: Some(2),
                ..Collected::default()
            }
        ),
        None,
        "a target without an ability must not fall back to some default special"
    );
    assert_eq!(
        action_from(
            ActionKind::Special,
            Collected {
                ability: Some(1),
                ..Collected::default()
            }
        ),
        None,
        "an ability with nobody to land on isn't an action yet"
    );
}

/// Backing out of the target picker for a Special returns to the ability
/// picker rather than discarding both choices — one Esc, one step.
#[test]
fn esc_from_the_target_picker_steps_back_to_the_ability_picker() {
    let mut app = battling_app();
    app.mode = Mode::BattleTarget;
    app.pending_battle_action = Some(ActionKind::Special);
    app.pending_special_ability = Some(1);

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::BattleSpecial);
    assert_eq!(
        app.pending_special_ability, None,
        "the ability is re-picked on the way back, not kept"
    );
    assert_eq!(
        app.pending_battle_action,
        Some(ActionKind::Special),
        "the action itself is still pending — only its ability was undone"
    );

    // A second Esc leaves the Special flow entirely.
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Battle);
    assert_eq!(app.pending_battle_action, None);
}

/// The ally picker is the other second step, and backs out the same way.
#[test]
fn esc_from_the_ally_picker_steps_back_to_the_ability_picker() {
    let mut app = battling_app();
    app.mode = Mode::BattleAlly;
    app.pending_battle_action = Some(ActionKind::Special);
    app.pending_special_ability = Some(0);

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::BattleSpecial);
    assert_eq!(app.pending_special_ability, None);
    assert_eq!(
        app.pending_battle_action,
        Some(ActionKind::Special),
        "only the ability was undone, not the whole action"
    );
}

/// Every picker layered over the battle roster has to count as being in
/// a battle, or the renderer discards battle-only state the moment one
/// opens. Pinned as a test as well as an exhaustive match, so the intent
/// survives someone "simplifying" the match into a wildcard.
#[test]
fn every_battle_screen_counts_as_being_in_a_battle() {
    for mode in [
        Mode::Battle,
        Mode::BattleTarget,
        Mode::BattleItem,
        Mode::BattleSpecial,
        Mode::BattleAlly,
    ] {
        assert!(mode.is_battle(), "{mode:?} is drawn over the battle roster");
    }
    for mode in [Mode::Playing, Mode::Inventory, Mode::Trade, Mode::GameOver] {
        assert!(!mode.is_battle(), "{mode:?} is not part of an intrusion");
    }
}

/// A picker that fails to commit must still close. Every picker clears
/// its pending action *before* calling `commit_battle_action`, so a
/// popup left up after a rejected action is inert — its rows all bail
/// on the now-missing pending state, leaving Esc as the only way out.
///
/// Called directly rather than driven through `handle_key`: the engine
/// never ends a battle on its own, so the only way to reach the error
/// branch from a keypress would be a rejection the pickers cannot
/// currently produce. The branch is the contract being pinned here.
#[test]
fn a_failed_commit_still_returns_to_the_roster() {
    let mut app = battling_app();
    app.mode = Mode::BattleAlly;
    // The state each picker leaves behind before it commits.
    app.pending_battle_action = None;
    app.pending_special_ability = None;
    app.game.as_mut().unwrap().battle_flee();

    app.commit_battle_action(0, BattleAction::Defend);

    assert_eq!(
        app.mode,
        Mode::Battle,
        "the ally picker must not be left on screen after a failed commit"
    );
    assert!(
        app.status_line.is_some(),
        "and the player has to be told why"
    );
}

/// Following the item picker through to a pick must actually spend the
/// item and resolve the round, not just look like it did.
#[test]
fn using_an_item_in_battle_spends_it_and_costs_the_round() {
    let mut app = battling_app();
    // The first row of the picker is what pressing `1` selects.
    let target = app.game.as_ref().unwrap().battle_usable_items()[0].clone();
    let held = |app: &App| -> u32 {
        app.game
            .as_ref()
            .unwrap()
            .player_status()
            .inventory
            .iter()
            .find(|(id, _)| *id == target)
            .map(|(_, n)| *n)
            .unwrap_or(0)
    };
    let before = held(&app);
    assert!(before > 0, "the starting kit should hold a consumable");

    app.handle_key(GameKey::Char('u'));
    assert_eq!(app.mode, Mode::BattleItem);
    app.handle_key(GameKey::Char('1'));

    assert!(
        app.pending_battle_action.is_none(),
        "the pending action should have been consumed, not left dangling"
    );
    assert_eq!(
        held(&app),
        before - 1,
        "exactly one {target:?} should have been spent"
    );
    assert!(
        matches!(app.mode, Mode::Battle | Mode::Playing | Mode::GameOver),
        "the only slot was planned, so the round should have resolved straight \
         back into planning; got {:?}",
        app.mode
    );
}

/// A solo player is a one-slot party, so choosing an untargeted action
/// completes the round immediately and drops straight back into planning.
/// No narration page in between: the battle screen's log pane already
/// shows what happened.
#[test]
fn completing_every_slot_resolves_the_round_without_a_narration_page() {
    let mut app = battling_app();
    let slots = app
        .game
        .as_ref()
        .unwrap()
        .battle_view()
        .unwrap()
        .party
        .len();
    assert_eq!(slots, 1, "the test seed's player starts with no companions");

    app.handle_key(GameKey::Char('d'));

    assert!(
        matches!(app.mode, Mode::Battle | Mode::Playing | Mode::GameOver),
        "the only slot was planned, so the round should have resolved straight \
         back into planning; got {:?}",
        app.mode
    );
}

/// The player's Special row is hidden entirely once nothing is installed —
/// a fresh game pre-installs decompile, so this pops it back out first. If
/// the row silently reappeared, pressing `s` would plan a Special against an
/// empty ability list and silently cost the player their round.
#[test]
fn pressing_special_with_nothing_installed_does_nothing() {
    let mut app = battling_app_with(|game| {
        let player = game
            .view_entities(12, 12)
            .into_iter()
            .find(|e| e.is_player)
            .expect("the player is always in view of itself")
            .entity;
        game.uninstall_routine(player, 0).unwrap();
    });
    assert!(
        app.game
            .as_ref()
            .unwrap()
            .battle_action_options(0)
            .into_iter()
            .all(|o| o.kind != ActionKind::Special),
        "a fresh game has nothing installed, so the row should be hidden"
    );

    app.handle_key(GameKey::Char('s'));

    assert_eq!(app.mode, Mode::Battle, "no picker should have opened");
    assert!(app.pending_battle_action.is_none());
}

/// The reveal must be driven by an injected delta, never a wall clock —
/// otherwise this test would need a sleep, which the suite forbids.
#[test]
fn lines_are_released_in_proportion_to_the_elapsed_time() {
    let mut app = battling_app();
    app.restart_reveal();

    app.advance_reveal(0.0);
    assert_eq!(
        app.revealed_battle_log().len(),
        0,
        "a zero delta released a line"
    );

    app.advance_reveal(1.0 / REVEAL_LINES_PER_SECOND);
    assert_eq!(
        app.revealed_battle_log().len(),
        1,
        "one line's worth of time released something other than one line"
    );
}

/// A frame covering less than a whole line must not lose the fraction: two
/// half-line frames make a line. Truncating instead would stall the reveal
/// completely on a fast enough frame rate.
#[test]
fn the_fractional_carry_does_not_lose_a_line() {
    let mut app = battling_app();
    app.restart_reveal();

    let half = 0.5 / REVEAL_LINES_PER_SECOND;
    app.advance_reveal(half);
    assert_eq!(app.revealed_battle_log().len(), 0);
    app.advance_reveal(half);
    assert_eq!(
        app.revealed_battle_log().len(),
        1,
        "the sub-line carry was dropped between frames"
    );
}

#[test]
fn the_reveal_stops_at_the_last_line_and_reports_done() {
    let mut app = battling_app();
    app.restart_reveal();
    let total = app.game.as_ref().unwrap().battle_log().len();
    assert!(total > 0, "the fixture produced no narration to reveal");

    app.advance_reveal(1_000.0);

    assert_eq!(app.revealed_battle_log().len(), total);
    assert!(
        !app.is_revealing(),
        "still reporting a reveal in progress with every line out"
    );
    assert_eq!(app.hidden_log_lines(), 0);
}

/// The pane shows this battle's lines and nothing older, so what the reveal
/// paces is scoped to the fight the player is actually in.
#[test]
fn the_revealed_log_never_runs_past_this_battle() {
    let mut app = battling_app();
    app.restart_reveal();
    app.advance_reveal(1_000.0);

    let revealed = app.revealed_battle_log();
    let battle = app.game.as_ref().unwrap().battle_log();
    assert_eq!(
        revealed.len(),
        battle.len(),
        "the pane and the engine disagree on what this battle logged"
    );
}

#[test]
fn a_key_pressed_mid_reveal_skips_instead_of_acting() {
    let mut app = battling_app();
    app.restart_reveal();
    app.advance_reveal(0.0);
    let mode_before = app.mode;
    assert!(app.is_revealing(), "the fixture left nothing to reveal");

    app.handle_key(GameKey::Esc);

    assert!(!app.is_revealing(), "the key did not finish the reveal");
    assert_eq!(
        app.mode, mode_before,
        "the skip key was acted on as well as skipping"
    );
}

#[test]
fn a_key_pressed_after_the_reveal_acts_normally() {
    let mut app = battling_app();
    app.advance_reveal(1_000.0);
    assert!(!app.is_revealing());

    app.handle_key(GameKey::Char('j'));

    assert!(
        !app.take_sounds().is_empty() || app.mode != Mode::Battle,
        "jacking out did nothing once the reveal was done"
    );
}

/// A won battle has had its log pruned to results by the engine. Those
/// results are what the map's log pane shows, and they scroll in there at
/// the same pace rather than appearing whole.
#[test]
fn ending_a_battle_restarts_the_reveal_for_the_results() {
    let mut app = battling_app();
    app.advance_reveal(1_000.0);
    assert!(!app.is_revealing());

    // Jack out — the one battle ending reachable from a single key press
    // regardless of how the fight is going.
    app.handle_key(GameKey::Char('j'));
    if app.game.as_ref().is_some_and(|g| g.has_active_battle()) {
        // The jack-out roll can fail; the battle is still on, so there is
        // no ended-battle handoff to assert about.
        return;
    }

    assert_eq!(
        app.revealed_battle_log().len(),
        0,
        "the results were shown whole instead of scrolling in"
    );
}

/// A refusal is drawn over the action bar, so leaving it up hides the menu
/// the player needs in order to press a different key.
#[test]
fn a_refusal_clears_itself_so_the_action_bar_comes_back() {
    let mut app = battling_app();
    app.status_line = Some("that ability isn't ready".to_string());

    app.advance_status(STATUS_LINE_SECONDS / 2.0);
    assert!(
        app.status_line.is_some(),
        "the message vanished before it could be read"
    );

    app.advance_status(STATUS_LINE_SECONDS / 2.0);
    assert!(
        app.status_line.is_none(),
        "the message outstayed its welcome and is still covering the menu"
    );
}

/// The window belongs to the newest message. Without the reset, a refusal
/// raised just as an older one aged out would flash and vanish.
///
/// Raised through a real key rather than by writing the field, because that
/// is now what tells a fresh message from a carried one — see
/// `a_standing_refusal_ages_out_while_the_player_keeps_planning`.
#[test]
fn a_new_refusal_gets_a_window_of_its_own() {
    let mut app = battling_app();
    app.status_line = Some("stale".to_string());
    app.advance_status(STATUS_LINE_SECONDS * 0.9);

    // Esc on the first slot has nothing to back up to, so it refuses.
    app.handle_key(GameKey::Esc);
    assert!(
        app.status_line.as_deref() != Some("stale"),
        "Esc on slot 0 should have raised a refusal of its own"
    );
    app.advance_status(STATUS_LINE_SECONDS * 0.5);

    assert!(
        app.status_line.is_some(),
        "the new message inherited the old one's remaining time"
    );
}

/// Nothing in a battle clears a refusal on the way *out* — unlike the map
/// menus, which clear it on every success — so the window is the only thing
/// that takes it down. Restarting that window on every key press meant a
/// player planning a round kept "Can't do that — 3 more rounds." on screen
/// for as long as they went on pressing keys, over the action bar it is
/// drawn on top of.
#[test]
fn a_standing_refusal_ages_out_while_the_player_keeps_planning() {
    let mut app = battling_app();
    app.status_line = Some("Can't do that — 3 more rounds.".to_string());
    app.advance_status(STATUS_LINE_SECONDS * 0.6);

    // An arrow is not an action in `Mode::Battle`: it raises nothing, so it
    // has nothing to say and no claim on the screen.
    app.handle_key(GameKey::Down);
    app.advance_status(STATUS_LINE_SECONDS * 0.6);

    assert_eq!(
        app.status_line, None,
        "the refusal was handed a fresh window by a key that said nothing, \
         and is still covering the action bar"
    );
}

/// Each round clears the pane, so the reveal has to restart with it.
/// Resetting only when the *battle* changes leaves `revealed` holding the
/// previous round's count — which already covers the new round's equally
/// short range, so `revealed >= total` short-circuits and the whole round
/// lands at once with no scrolling at all.
///
/// Resolves two rounds deliberately: the first round's carried count is
/// only the opening line, so the bug shows up in full from the second.
#[test]
fn every_round_restarts_the_reveal_instead_of_landing_whole() {
    let mut app = battling_app();
    // Adopt the current generation and drain the opening line, so what this
    // asserts afterwards is about the round and not about a stale reset.
    app.advance_reveal(1_000.0);

    for round in 0..2 {
        assert!(
            app.game.as_ref().is_some_and(|g| g.has_active_battle()),
            "the fight ended after {round} rounds — this needs one that continues"
        );
        // All-defend resolves the round without stopping to pick a target.
        app.handle_key(GameKey::Char('D'));
        app.advance_reveal(0.0);

        assert_eq!(
            app.revealed_battle_log().len(),
            0,
            "round {round} was already on screen before any time had passed"
        );
        assert!(
            app.is_revealing(),
            "round {round} has narration, so it should still be scrolling in"
        );
        app.advance_reveal(1_000.0);
    }
}

/// Every round has to actually take time on screen, not just the first.
///
/// Two separate bugs made rounds after the first land whole. The counter
/// was keyed on the battle rather than the round, so a carried-over count
/// already covered the next round's equally short range; and `is_revealing`
/// read the raw count, so between a round resolving and the next frame's
/// `advance_reveal` every reader still saw the finished round's total. This
/// walks real frames, which is what catches the second one — the reveal
/// looked fine to a test that reset it by hand first.
#[test]
fn every_round_takes_real_time_to_scroll_in() {
    let mut app = battling_app();
    app.advance_reveal(1_000.0);
    let frame = 1.0 / 60.0;

    for round in 0..3 {
        assert!(
            app.game.as_ref().is_some_and(|g| g.has_active_battle()),
            "the fight ended after {round} rounds — this needs one that continues"
        );
        // All-defend resolves the round without stopping to pick a target.
        app.handle_key(GameKey::Char('D'));

        let total = app.game.as_ref().unwrap().battle_log().len();
        assert!(total > 1, "round {round} narrated too little to pace");
        assert_eq!(
            app.revealed_battle_log().len(),
            0,
            "round {round} was on screen before a frame had passed"
        );

        let mut frames = 0;
        while app.is_revealing() && frames < 10_000 {
            app.advance_reveal(frame);
            frames += 1;
        }
        let seconds = frames as f32 * frame;
        let expected = total as f32 / REVEAL_LINES_PER_SECOND;
        assert!(
            (seconds - expected).abs() < 0.1,
            "round {round}: {total} lines took {seconds:.2}s, expected about {expected:.2}s"
        );
    }
}
