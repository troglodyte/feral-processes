//! Driving an intrusion from key presses to a resolved round.

use super::support::*;
use crate::*;

/// Scans seeds until one puts a wild program next to the player, then
/// bumps it to open a battle. Returns the app sitting in `Mode::Battle`
/// with the entry sounds already drained, so a caller can attribute
/// anything it observes afterwards to the key it pressed.
fn battling_app() -> App {
    for seed in 0..200u32 {
        let mut app = test_app(seed);
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
        if app.mode == Mode::Battle {
            let _ = app.take_sounds();
            return app;
        }
    }
    panic!("no seed under 200 put a wild program next to the player — encounter setup changed");
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
