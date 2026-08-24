//! Driving an intrusion from key presses to a resolved round.

use super::support::*;
use crate::*;
use feral_processes_engine::{MESSAGE_LOG_CAP, MessageKind};

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

/// The engine already greys a routine it would refuse — no taming
/// catalyst, still recharging, roster full — and both the action menu
/// (`handle_battle_key`) and the field cast list refuse the press with
/// that reason. The ability picker didn't: it took the press and opened
/// the group picker, so the player aimed a routine that could only be
/// thrown away once committed.
#[test]
fn an_unavailable_routine_is_refused_instead_of_opening_the_target_picker() {
    let mut app = battling_app_with(|game| {
        // Decompile's catalyst. Gone, the row reads "no taming catalyst",
        // which is the cheapest of the three reasons to set up.
        let _ = game.erase_item(
            &gear(
                &ItemId::from(feral_processes_engine::items::ids::ICE_BREAKER),
                0,
            ),
            99,
        );
    });
    let options = app.game.as_ref().unwrap().battle_special_options(0);
    let idx = options
        .iter()
        .position(|o| o.unavailable.is_some())
        .expect("with no ICE Breakers the player's Decompile row should be greyed");

    app.mode = Mode::BattleSpecial;
    app.pending_battle_action = Some(ActionKind::Special);
    app.pending_special_ability = None;
    app.menu_selected = idx;
    app.handle_key(GameKey::Enter);

    assert_eq!(
        app.mode,
        Mode::BattleSpecial,
        "an unavailable routine should leave the player on the picker it was refused from"
    );
    assert_eq!(
        app.pending_special_ability, None,
        "nothing was chosen, so nothing should be pending"
    );
    assert!(
        app.status_line.is_some_and(|s| s.contains("catalyst")),
        "the refusal should say why, the way the action menu's does"
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
            .find(|r| r.copy.item == target)
            .map(|r| r.qty)
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

/// `T` in the item picker throws the highlighted row instead of falling
/// through to the row shortcuts, and commits no `UseItem` for the slot.
/// Named by nothing on screen — see `crates/engine/EASTER_EGGS.md`.
#[test]
fn shift_t_in_the_item_picker_throws_rather_than_picking_a_row() {
    let mut app = battling_app();
    let target = app.game.as_ref().unwrap().battle_usable_items()[0].clone();
    let held = |app: &App| -> u32 {
        app.game
            .as_ref()
            .unwrap()
            .player_status()
            .inventory
            .iter()
            .find(|r| r.copy.item == target)
            .map(|r| r.qty)
            .unwrap_or(0)
    };
    let before = held(&app);
    let slot = app.game.as_ref().unwrap().battle_active_slot();
    let round = app.game.as_ref().unwrap().battle_view().unwrap().round;

    app.handle_key(GameKey::Char('u'));
    assert_eq!(app.mode, Mode::BattleItem);
    app.handle_key(GameKey::Char('T'));

    assert_eq!(held(&app), before - 1, "the highlighted row was not thrown");
    let game = app.game.as_ref().unwrap();
    assert_eq!(
        game.battle_active_slot(),
        slot,
        "throwing committed the slot's action"
    );
    assert_eq!(
        game.battle_view().unwrap().round,
        round,
        "throwing resolved the round"
    );
    assert_eq!(
        app.mode,
        Mode::Battle,
        "the picker should close back onto the roster, as Esc does"
    );
    assert!(
        app.pending_battle_action.is_none(),
        "a pending UseItem left dangling would strand the next picker"
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

/// The reported bug: on the map, a key sometimes did nothing at all.
///
/// `MessageLog::round_start` is never closed once the run's first battle has
/// opened it, so `since_round` — and with it `is_revealing` — goes on growing
/// with ordinary map and base news for the rest of the run. `handle_key`'s
/// skip is unconditional, so every one of those lines bought the player a
/// swallowed keypress on a screen with no narration to skip.
#[test]
fn a_key_pressed_while_map_news_scrolls_in_still_acts() {
    let mut app = escaped_app();
    app.advance_reveal(1_000.0);
    assert!(!app.is_revealing(), "the fixture did not settle");

    // Any map action that logs will do; a refused rest is the one that
    // neither ticks the world nor depends on where the player is standing.
    app.handle_key(GameKey::Char('r'));
    assert_eq!(app.mode, Mode::Playing, "the fixture left the map");
    app.advance_reveal(0.0);

    app.handle_key(GameKey::Char('x'));

    assert_eq!(
        app.mode,
        Mode::InspectDirection,
        "the key was eaten by the map log's reveal"
    );
}

/// The other half of the same leak: the map's pane chopped that unrevealed
/// tail off, so news from a running base arrived at four lines a second
/// however much of it there was.
#[test]
fn map_news_reaches_the_pane_without_waiting_for_a_reveal() {
    let mut app = escaped_app();
    app.advance_reveal(1_000.0);
    let before = app.visible_log(40).len();

    app.handle_key(GameKey::Char('r'));
    app.advance_reveal(0.0);

    assert_eq!(app.hidden_log_lines(), 0, "the map pane held a line back");
    assert!(
        app.visible_log(40).len() > before,
        "the refusal never reached the pane"
    );
}

/// A game back on the map with a finished battle behind it, which is what
/// leaves `round_start` open.
fn escaped_app() -> App {
    let mut app = battling_app();
    app.advance_reveal(1_000.0);
    for _ in 0..200 {
        if app.mode == Mode::Playing {
            return app;
        }
        app.handle_key(GameKey::Char('j'));
        app.advance_reveal(1_000.0);
    }
    panic!("could not get back to the map");
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

/// The roster the screen draws steps with the narration rather than
/// snapping to the end of the round — `App::battle_view` is the seam that
/// hands the engine the revealed-line count, and a renderer reading
/// `Game::battle_view` directly would bypass it.
///
/// Defending is what makes the fixture work: the player deals no damage,
/// so however many rounds this takes the fight stays live, and the only
/// thing that can move a bar is the enemy's own swing.
#[test]
fn the_roster_the_screen_draws_steps_with_the_reveal() {
    let mut app = battling_app();

    for _ in 0..40 {
        let before = app.battle_view().expect("still in the fight").party[0].hp;
        app.handle_key(GameKey::Char('d'));
        let Some(live) = app.game.as_ref().and_then(|g| g.battle_view()) else {
            break;
        };
        if live.party[0].hp >= before {
            // The swing missed, was absorbed, or the player levelled and
            // healed — nothing to observe this round.
            app.finish_reveal();
            continue;
        }
        assert_eq!(
            app.battle_view().expect("still in the fight").party[0].hp,
            before,
            "the bar dropped before the line describing the hit was on screen"
        );
        app.finish_reveal();
        assert_eq!(
            app.battle_view().expect("still in the fight").party[0].hp,
            live.party[0].hp,
            "the bar never caught up to the engine once the reveal finished"
        );
        return;
    }
    panic!("forty defended rounds landed no enemy damage — encounter setup changed");
}

/// Attacks until the fight ends, and returns the app sitting on whatever
/// screen that produced. Attacking rather than jacking out because the roll
/// to jack out can fail, and a fight the player is winning is the ending
/// the results page is most about.
fn app_with_a_finished_fight() -> App {
    let mut app = battling_app();
    for _ in 0..60 {
        if !app.game.as_ref().is_some_and(|g| g.has_active_battle()) {
            return app;
        }
        app.handle_key(GameKey::Char('a'));
        app.finish_reveal();
    }
    panic!("sixty rounds did not finish a single-program fight — combat setup changed");
}

#[test]
fn a_finished_fight_holds_the_battle_screen_rather_than_dropping_to_the_map() {
    let app = app_with_a_finished_fight();

    assert_eq!(
        app.mode,
        Mode::BattleResult,
        "the fight ended straight onto the map instead of stopping to report"
    );
}

/// The whole point: the loot and XP the engine pruned the log down to
/// arrive in the battle screen's own pane, not sliding past on the map.
#[test]
fn the_results_arrive_while_the_battle_screen_is_still_up() {
    let mut app = app_with_a_finished_fight();
    app.finish_reveal();

    let lines = app.revealed_battle_log();
    assert!(
        !lines.is_empty(),
        "a won fight reported nothing — the prune left no results to show"
    );
}

#[test]
fn a_key_leaves_the_finished_battle_screen_for_the_map() {
    let mut app = app_with_a_finished_fight();
    app.finish_reveal();
    assert_eq!(app.mode, Mode::BattleResult);

    app.handle_key(GameKey::Char(' '));

    assert_eq!(app.mode, Mode::Playing, "the page did not dismiss");
}

/// The final round's blow-by-blow is what the results screen opens with —
/// the fight used to jump from the kill straight to the salvage, because the
/// prune ran inside the round that ended it.
#[test]
fn the_results_screen_opens_with_the_final_rounds_blows() {
    let mut app = app_with_a_finished_fight();
    app.finish_reveal();

    let kinds: Vec<MessageKind> = app.revealed_battle_log().iter().map(|r| r.kind).collect();
    assert!(
        kinds.contains(&MessageKind::PartyDamage),
        "no swing survived onto the results screen: {:#?}",
        app.revealed_battle_log()
            .iter()
            .map(|r| (r.text.clone(), r.kind))
            .collect::<Vec<_>>()
    );
}

/// ...and leaving the screen is what takes it back off, so the map's pane
/// still gets the results alone.
#[test]
fn leaving_the_results_screen_drops_the_blows_but_keeps_the_results() {
    let mut app = app_with_a_finished_fight();
    app.finish_reveal();

    app.handle_key(GameKey::Char(' '));
    assert_eq!(app.mode, Mode::Playing);

    let log = app
        .game
        .as_ref()
        .expect("the run is still live")
        .message_log(MESSAGE_LOG_CAP);
    assert!(
        !log.iter().any(|l| l.kind == MessageKind::PartyDamage),
        "the blow-by-blow followed the player onto the map: {:#?}",
        log.iter()
            .map(|l| (l.text.clone(), l.kind))
            .collect::<Vec<_>>()
    );
    assert!(
        log.iter().any(|l| l.kind == MessageKind::Outcome),
        "the results were pruned away with the narration: {:#?}",
        log.iter()
            .map(|l| (l.text.clone(), l.kind))
            .collect::<Vec<_>>()
    );
}

/// The first key is spent skipping, so loot cannot be dismissed unread.
#[test]
fn a_key_pressed_while_the_results_scroll_skips_rather_than_leaving() {
    let mut app = app_with_a_finished_fight();
    app.restart_reveal();
    app.advance_reveal(0.0);
    assert!(app.is_revealing(), "the fixture left nothing to scroll");

    app.handle_key(GameKey::Char(' '));

    assert_eq!(
        app.mode,
        Mode::BattleResult,
        "the page dismissed on the key that should have released the narration"
    );
    assert!(!app.is_revealing(), "the key did not finish the reveal");

    app.handle_key(GameKey::Char(' '));
    assert_eq!(app.mode, Mode::Playing, "the second key did not dismiss");
}

/// The battle screen stays up on a finished fight, so `App::battle_view`
/// has to keep answering after `end_battle` has removed `BattleState` and
/// `Game::battle_view` has gone `None`. Without the fallback the renderer
/// returns early and the player sees the map — which is the behaviour this
/// replaced.
#[test]
fn the_screen_still_has_a_roster_to_draw_once_the_fight_is_over() {
    let app = app_with_a_finished_fight();

    assert!(
        app.game.as_ref().unwrap().battle_view().is_none(),
        "test premise: the live view is gone once the fight ends"
    );
    let view = app
        .battle_view()
        .expect("the screen has nothing to draw, so it would fall back to the map");
    assert!(!view.party.is_empty(), "the party roster came back empty");
    assert!(
        view.options.is_empty(),
        "a finished fight should offer no actions to spend"
    );
}

/// The fallback is scoped to the results screen, so a stale roster cannot
/// surface once the player is back on the map.
#[test]
fn the_closing_roster_does_not_leak_onto_the_map() {
    let mut app = app_with_a_finished_fight();
    app.finish_reveal();
    app.handle_key(GameKey::Char(' '));
    assert_eq!(app.mode, Mode::Playing);

    assert!(
        app.battle_view().is_none(),
        "the finished fight's roster followed the player onto the map"
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

/// The reported bug: one `a` appeared to command the whole party. Every
/// other fixture in this file fights with an empty `Party`, where a single
/// slot really is the whole round — so nothing here had ever driven a
/// two-slot battle from key presses.
#[test]
fn attack_plans_only_the_active_slot_when_a_companion_is_in_the_party() {
    let mut app = None;
    for seed in 0..200u32 {
        let mut candidate = app_with_companions_in_the_party(seed, 1);
        let game = candidate.game.as_mut().unwrap();
        let player = game.player_status().position;
        let target = game
            .view_entities(12, 12)
            .into_iter()
            .filter(|e| e.is_hostile && !e.is_tamed && !e.is_structure)
            .find(|e| (e.pos.0 - player.0).abs() + (e.pos.1 - player.1).abs() == 1);
        let Some(target) = target else { continue };
        candidate.handle_key(match (target.pos.0 - player.0, target.pos.1 - player.1) {
            (1, 0) => GameKey::Right,
            (-1, 0) => GameKey::Left,
            (0, 1) => GameKey::Down,
            _ => GameKey::Up,
        });
        let single_group = candidate
            .game
            .as_ref()
            .and_then(|g| g.battle_view())
            .is_some_and(|v| v.groups.len() == 1);
        if candidate.mode == Mode::Battle && single_group {
            let _ = candidate.take_sounds();
            candidate.finish_reveal();
            app = Some(candidate);
            break;
        }
    }
    let mut app = app.expect("no seed under 200 opened a lone-group battle with a companion");

    assert_eq!(
        app.game.as_ref().unwrap().battle_active_slot(),
        Some(0),
        "the player picks first"
    );

    app.handle_key(GameKey::Char('a'));
    assert_eq!(
        app.mode,
        Mode::BattleTarget,
        "attack always asks which group, even when there is only one"
    );
    app.handle_key(GameKey::Char('a')); // group A

    assert_eq!(
        app.mode,
        Mode::Battle,
        "one slot's attack must not resolve the round"
    );
    assert_eq!(
        app.game.as_ref().unwrap().battle_active_slot(),
        Some(1),
        "the companion's slot is now the one awaiting an action"
    );
}

/// The reported shape: a full party of three, each slot asked in turn. One
/// companion could pass by luck — a three-slot round is what "everyone
/// attacks off one key" would actually have to break.
#[test]
fn a_full_party_is_asked_slot_by_slot_and_only_then_resolves() {
    let mut app = None;
    for seed in 0..200u32 {
        let mut candidate = app_with_companions_in_the_party(seed, 3);
        let game = candidate.game.as_mut().unwrap();
        let player = game.player_status().position;
        let target = game
            .view_entities(12, 12)
            .into_iter()
            .filter(|e| e.is_hostile && !e.is_tamed && !e.is_structure)
            .find(|e| (e.pos.0 - player.0).abs() + (e.pos.1 - player.1).abs() == 1);
        let Some(target) = target else { continue };
        candidate.handle_key(match (target.pos.0 - player.0, target.pos.1 - player.1) {
            (1, 0) => GameKey::Right,
            (-1, 0) => GameKey::Left,
            (0, 1) => GameKey::Down,
            _ => GameKey::Up,
        });
        if candidate.mode == Mode::Battle {
            let _ = candidate.take_sounds();
            candidate.finish_reveal();
            app = Some(candidate);
            break;
        }
    }
    let mut app = app.expect("no seed under 200 opened a battle with a full party");

    for slot in 0..4 {
        assert_eq!(
            app.game.as_ref().unwrap().battle_active_slot(),
            Some(slot),
            "slot {slot} should be the one being asked"
        );
        app.handle_key(GameKey::Char('a'));
        assert_eq!(app.mode, Mode::BattleTarget, "slot {slot} picks a group");
        app.handle_key(GameKey::Char('a'));
    }
}

/// `T` says something and nothing else: it must not commit an action for
/// the slot being asked, and must not resolve the round. Named by nothing
/// on screen — see `crates/engine/EASTER_EGGS.md`.
#[test]
fn shift_t_taunts_without_spending_the_slots_action() {
    let mut app = battling_app();
    let (slot, round, logged) = {
        let game = app.game.as_ref().unwrap();
        (
            game.battle_active_slot(),
            game.battle_view().unwrap().round,
            game.message_log(usize::MAX).len(),
        )
    };

    app.handle_key(GameKey::Char('T'));

    assert_eq!(app.mode, Mode::Battle, "'T' must not open a picker");
    let game = app.game.as_ref().unwrap();
    assert_eq!(
        game.battle_active_slot(),
        slot,
        "taunting committed the slot's action"
    );
    assert_eq!(
        game.battle_view().unwrap().round,
        round,
        "taunting resolved the round"
    );
    assert_eq!(
        game.message_log(usize::MAX).len(),
        logged + 1,
        "the taunt should log exactly one line"
    );
}

/// A battle whose pane holds several lines: the opening range is a header
/// and little else, and a window has to have something to walk back through.
/// Resolving one round is what fills it — which may also finish the fight,
/// so the caller must not assume `Mode::Battle`.
fn app_with_narration() -> App {
    let mut app = battling_app();
    // All-attack, not the per-slot `a`: with one group there is no target to
    // pick, so this resolves the round instead of opening the picker.
    app.handle_key(GameKey::Char('A'));
    app.advance_reveal(1_000.0);
    assert!(
        app.revealed_battle_log().len() >= 2,
        "one resolved round produced too little narration to scroll"
    );
    app
}

/// The battle pane is a *window* on the round's narration rather than the
/// whole of it. Measured with the arena (`dev-arenas/full-group.ron`, 50
/// reps): a round runs to 18 lines, against a pane that seats about 15 in a
/// four-group fight — and `Game::battle_log` is `since_round`, so the next
/// round replaces the range outright and `retain_outcomes_since_battle`
/// deletes the blow-by-blow when the fight ends. What scrolls off the top is
/// therefore gone from every screen in the game, which is why the window has
/// to be walkable rather than pinned to the newest line.
#[test]
fn scrolling_up_walks_the_battle_pane_back_through_the_round() {
    let mut app = app_with_narration();
    let capacity = app.revealed_battle_log().len() - 1;

    let pinned = app.battle_pane(capacity);
    assert_eq!(
        pinned.above, 1,
        "the pane should start pinned to the newest line"
    );

    app.handle_key(GameKey::Up);
    let scrolled = app.battle_pane(capacity);

    assert_eq!(
        scrolled.above, 0,
        "scrolling up left lines hidden above the window"
    );
    assert_eq!(
        scrolled.below, 1,
        "the newest line should have moved below the window"
    );
    assert_eq!(
        scrolled.rows.first().map(|l| l.text.clone()),
        app.revealed_battle_log().first().map(|l| l.text.clone()),
        "scrolling up did not reach the round's opening line"
    );
}

/// Down is the way back, and the pane pins to the newest line rather than
/// running past it — that is the position every other reader assumes.
#[test]
fn scrolling_down_returns_the_pane_to_the_newest_line() {
    let mut app = app_with_narration();
    let capacity = app.revealed_battle_log().len() - 1;

    app.handle_key(GameKey::Up);
    app.battle_pane(capacity);
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Down);

    let pane = app.battle_pane(capacity);
    assert_eq!(
        pane.below, 0,
        "the pane did not come back to the newest line"
    );
}

/// The window stops at the oldest revealed line instead of walking off the
/// top into a shrinking pane — scrolling past the start would otherwise draw
/// fewer and fewer rows against a pane that has not changed size.
#[test]
fn the_scroll_stops_at_the_oldest_revealed_line() {
    let mut app = app_with_narration();
    let lines = app.revealed_battle_log().len();
    let capacity = lines - 1;

    for _ in 0..50 {
        app.handle_key(GameKey::Up);
        app.battle_pane(capacity);
    }

    let pane = app.battle_pane(capacity);
    assert_eq!(pane.above, 0, "walked past the oldest line");
    assert_eq!(
        pane.rows.len(),
        capacity,
        "the window shrank instead of stopping"
    );
}

/// A pane holding everything it is given has nothing to scroll, and must not
/// let a stray arrow key move it off the newest line anyway.
#[test]
fn a_pane_with_room_to_spare_does_not_scroll() {
    let mut app = app_with_narration();
    let capacity = app.revealed_battle_log().len() + 10;

    app.handle_key(GameKey::Up);
    let pane = app.battle_pane(capacity);

    assert_eq!(pane.above, 0);
    assert_eq!(pane.below, 0);
    assert_eq!(pane.rows.len(), app.revealed_battle_log().len());
}

/// A resolved round replaces the pane's whole range, so a scroll position
/// held over from the last one would point into narration that no longer
/// exists. `BattleReveal` is reset wholesale on a generation change, which
/// is what makes this come free rather than needing its own clear.
#[test]
fn a_new_round_snaps_the_pane_back_to_the_newest_line() {
    let mut app = app_with_narration();
    let capacity = app.revealed_battle_log().len() - 1;

    app.handle_key(GameKey::Up);
    app.battle_pane(capacity);

    // Resolves the round, which opens a fresh range.
    app.handle_key(GameKey::Char('A'));
    app.advance_reveal(1_000.0);

    let pane = app.battle_pane(capacity);
    assert_eq!(
        pane.below, 0,
        "the new round opened part-way up its own narration"
    );
}

/// The results page is where the tally and the XP lines land, so it is the
/// screen most likely to overflow — but every key there dismissed it. The
/// two arrows now scroll instead; everything else still leaves.
#[test]
fn an_arrow_on_the_results_page_scrolls_instead_of_dismissing_it() {
    let mut app = battling_app();
    while app.mode == Mode::Battle {
        app.handle_key(GameKey::Char('A'));
        app.advance_reveal(1_000.0);
    }
    assert_eq!(
        app.mode,
        Mode::BattleResult,
        "the fixture never finished the fight"
    );

    app.handle_key(GameKey::Up);

    assert_eq!(
        app.mode,
        Mode::BattleResult,
        "an arrow dismissed the results"
    );
}

#[test]
fn any_other_key_still_dismisses_the_results_page() {
    let mut app = battling_app();
    while app.mode == Mode::Battle {
        app.handle_key(GameKey::Char('A'));
        app.advance_reveal(1_000.0);
    }

    app.handle_key(GameKey::Enter);

    assert_eq!(
        app.mode,
        Mode::Playing,
        "the results page stopped being dismissable"
    );
}

/// A refusal raised inside a fight reaches the player on the status line
/// and stops there. `MessageLog::since_round` slices the battle pane by
/// position and the reveal paces it by counting raw lines, so logging one
/// mid-fight would draw it as narration and swallow a keypress —
/// `Game::note_refusal` owns that rule and `App::refuse` inherits it.
#[test]
fn a_refusal_inside_a_fight_stays_off_the_log() {
    let mut app = battling_app();
    let before = app
        .game
        .as_ref()
        .unwrap()
        .message_log(crate::MESSAGE_LOG_CAP)
        .len();

    // Esc on the first slot has nothing to back up to, so it refuses.
    app.handle_key(GameKey::Esc);

    assert!(app.status_line.is_some(), "the refusal reached the player");
    assert_eq!(
        app.game
            .as_ref()
            .unwrap()
            .message_log(crate::MESSAGE_LOG_CAP)
            .len(),
        before,
        "a refusal must not enter the log mid-fight"
    );
}
