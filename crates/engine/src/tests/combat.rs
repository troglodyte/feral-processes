//! One round of a battle: planning every slot, resolving it, and
//! ending the fight.

use super::support::*;
use crate::*;

#[test]
fn battle_flee_applies_the_same_mild_xp_setback_as_a_death() {
    let mut game = Game::new(33, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Experience>(player).unwrap().xp = 10;
    let species = game
        .species_defs()
        .into_iter()
        .next()
        .expect("at least one species");
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 3, y: 3 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 0,
                def: 1,
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);

    game.battle_flee();

    assert_eq!(
        game.world.get::<Experience>(player).unwrap().xp,
        8,
        "fleeing should dock the same 20% setback as a death"
    );
    assert!(!game.has_active_battle(), "fleeing should end the battle");
}

/// All-attack asks which group only when there is a choice to make. With
/// a single group left the prompt is pure friction, which is the whole
/// complaint this work started from.
#[test]
fn all_attack_needs_a_target_only_while_more_than_one_group_lives() {
    let mut game = Game::new(82, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let mut species = game.species_defs().into_iter().map(|s| s.id);
    let first = species.next().unwrap();
    let second = species.next().expect("assets ship at least two species");

    let solo = game.spawn_wild_creature(&first, 5, 5).unwrap();
    insert_battle(&mut game, player, vec![solo]);
    let needs = |game: &Game| {
        game.battle_party_commands()
            .into_iter()
            .find(|c| c.kind == PartyCommandKind::AllAttack)
            .expect("all-attack should always be offered")
            .needs_target
    };
    assert!(
        !needs(&game),
        "one group means no choice, so all-attack shouldn't open a picker"
    );

    let a = game.spawn_wild_creature(&first, 5, 5).unwrap();
    let b = game.spawn_wild_creature(&second, 6, 5).unwrap();
    insert_battle(&mut game, player, vec![a, b]);
    assert_eq!(
        game.battle_view().unwrap().groups.len(),
        2,
        "two different species should partition into two groups — test premise"
    );
    assert!(
        needs(&game),
        "two groups means a real focus-fire choice, so all-attack must ask"
    );
}

/// The renderers draw this list verbatim instead of hardcoding strings.
#[test]
fn battle_party_commands_offers_all_attack_all_defend_and_jack_out() {
    let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let species = game.species_defs().into_iter().next().unwrap().id;
    let wild = game.spawn_wild_creature(&species, 5, 5).unwrap();
    insert_battle(&mut game, player, vec![wild]);

    let commands = game.battle_party_commands();
    let keys: Vec<char> = commands.iter().map(|c| c.key).collect();
    assert_eq!(
        keys,
        vec!['A', 'D', 'j'],
        "uppercase for the party-wide pair, lowercase for jack out"
    );
    for command in &commands {
        assert!(
            command.label.contains(&format!("[{}]", command.key)),
            "{:?} advertises key {:?} but its label is {:?}",
            command.kind,
            command.key,
            command.label
        );
    }
}

/// `[A]`/`[D]` fill the party in one keypress, but must never overwrite a
/// choice the player already made deliberately — they pressed it partway
/// through planning, not before starting.
#[test]
fn battle_plan_remaining_fills_only_unplanned_slots() {
    let mut game = Game::new(79, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let species = game.species_defs().into_iter().next().unwrap().id;
    let companion = game.spawn_wild_creature(&species, 4, 5).unwrap();
    game.world.resource_mut::<Party>().0.push(companion);
    let wild = game.spawn_wild_creature(&species, 5, 5).unwrap();
    insert_battle(&mut game, player, vec![wild]);

    // Slot 0 (the player) picks for itself; slot 1 is left open.
    game.battle_set_action(0, BattleAction::Attack { group: 0 })
        .unwrap();
    game.battle_plan_remaining(BattleAction::Defend).unwrap();

    let planned = &game.world.resource::<BattleState>().planned;
    assert_eq!(
        planned[0],
        Some(BattleAction::Attack { group: 0 }),
        "the slot that was already planned must keep its own choice"
    );
    assert_eq!(
        planned[1],
        Some(BattleAction::Defend),
        "the open slot should have been filled"
    );
    assert!(
        game.battle_round_ready(),
        "every actionable slot is planned"
    );
}

/// A knocked-out companion's slot is skipped by `battle_active_slot` and
/// doesn't block `battle_round_ready`. Filling it would hand an action to
/// a member that can't take one.
#[test]
fn battle_plan_remaining_skips_a_slot_that_cannot_act() {
    let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let species = game.species_defs().into_iter().next().unwrap().id;
    let companion = game.spawn_wild_creature(&species, 4, 5).unwrap();
    game.world.resource_mut::<Party>().0.push(companion);
    let wild = game.spawn_wild_creature(&species, 5, 5).unwrap();
    insert_battle(&mut game, player, vec![wild]);

    // Drop the companion, so slot 1 can no longer act.
    game.world.get_mut::<Stats>(companion).unwrap().hp = 0;
    assert!(
        !game.slot_can_act(1),
        "a companion at 0 HP should not be able to act — test premise is wrong"
    );

    game.battle_plan_remaining(BattleAction::Defend).unwrap();

    let planned = &game.world.resource::<BattleState>().planned;
    assert_eq!(planned[0], Some(BattleAction::Defend));
    assert_eq!(
        planned[1], None,
        "a slot that can't act must stay unplanned, not be handed an action"
    );
}

/// Uppercase A and D became party-wide commands, which only works if the
/// per-slot keys underneath them are Attack and Defend. Decompile moved
/// off `d` to make room. Pinned here so a future re-key cannot silently
/// swap a brace for a capture attempt that spends a taming catalyst.
#[test]
fn battle_action_keys_are_lowercase_with_defend_on_d_and_decompile_on_c() {
    let mut game = Game::new(78, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    insert_battle(&mut game, player, vec![wild]);

    let options = game.battle_action_options(0);
    let key_for = |kind: ActionKind| {
        options
            .iter()
            .find(|o| o.kind == kind)
            .unwrap_or_else(|| panic!("the player's menu should offer {kind:?}"))
            .key
    };
    assert_eq!(key_for(ActionKind::Attack), 'a');
    assert_eq!(key_for(ActionKind::Defend), 'd');
    assert_eq!(key_for(ActionKind::Decompile), 'c');
    assert_eq!(key_for(ActionKind::UseItem), 'u');

    for option in &options {
        assert!(
            option.label.contains(&format!("[{}]", option.key)),
            "{:?} advertises key {:?} but its label is {:?} — the bracketed \
             letter must be the lowercase key the player actually presses",
            option.kind,
            option.key,
            option.label
        );
    }
}

/// The menu is data, not renderer strings. Decompile must report *why*
/// it is unavailable so the UI can grey it with a reason instead of
/// hiding it and leaving the player guessing.
#[test]
fn decompile_is_offered_with_a_reason_when_no_catalyst_is_held() {
    let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // `Inventory` exposes no `clear` — `items` is a public
    // `Vec<(ItemId, u32)>`, so empty it directly.
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .items
        .clear();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    game.start_battle(vec![wild]);

    let options = game.battle_action_options(0);
    let decompile = options
        .iter()
        .find(|o| o.kind == ActionKind::Decompile)
        .expect("Decompile must be listed even when unusable");
    assert!(
        decompile
            .unavailable
            .as_deref()
            .is_some_and(|r| r.contains("catalyst")),
        "expected a catalyst reason, got {:?}",
        decompile.unavailable
    );
}

/// The planning API is the whole extensibility story: the engine emits
/// the menu, renderers dispatch off it. A slot that does not exist must
/// be refused rather than silently ignored.
#[test]
fn battle_set_action_refuses_a_slot_that_is_not_in_the_party() {
    let mut game = Game::new(80, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    game.start_battle(vec![wild]);

    // Slot 0 is the player and always exists; slot 1 needs a companion.
    assert!(
        game.battle_set_action(0, BattleAction::Attack { group: 0 })
            .is_ok()
    );
    let err = game
        .battle_set_action(1, BattleAction::Attack { group: 0 })
        .unwrap_err();
    assert!(
        err.contains("party"),
        "expected a party-slot error, got {err:?}"
    );
}

/// The same argument, for the two indices an ally-targeted Special
/// carries beyond the acting slot. Unchecked, both resolve to `None`
/// mid-round and cost the member its turn in silence — while the player
/// is still charged the fatigue for commanding it.
#[test]
fn battle_set_action_refuses_an_out_of_range_ally_slot_or_ability() {
    let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 20, 5);
    game.add_companion(pet).unwrap();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    game.start_battle(vec![wild]);

    let abilities = game.battle_special_options(1).len();
    assert!(
        abilities >= 1,
        "every companion has at least the fallback rally"
    );

    let err = game
        .battle_set_action(
            1,
            BattleAction::Special {
                ability: 0,
                target: battle::SpecialTarget::Ally { slot: 42 },
            },
        )
        .unwrap_err();
    assert!(
        err.contains("party"),
        "expected a party-slot error, got {err:?}"
    );

    let err = game
        .battle_set_action(
            1,
            BattleAction::Special {
                ability: abilities,
                target: battle::SpecialTarget::Ally { slot: 0 },
            },
        )
        .unwrap_err();
    assert!(
        err.contains("ability"),
        "expected an ability error, got {err:?}"
    );

    assert!(
        game.battle_set_action(
            1,
            BattleAction::Special {
                ability: 0,
                target: battle::SpecialTarget::Ally { slot: 0 },
            },
        )
        .is_ok(),
        "a valid ability aimed at a real slot must still be accepted"
    );
}

/// Backing up a slot is how the player corrects a misclick — the cursor
/// has to walk back, not just blank the entry.
#[test]
fn battle_clear_action_walks_the_active_slot_back() {
    let mut game = Game::new(82, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    game.start_battle(vec![wild]);

    assert_eq!(game.battle_active_slot(), Some(0));
    game.battle_set_action(0, BattleAction::Attack { group: 0 })
        .unwrap();
    assert_eq!(
        game.battle_active_slot(),
        None,
        "solo party is fully planned"
    );
    game.battle_clear_action(0);
    assert_eq!(game.battle_active_slot(), Some(0));
}

/// A companion knocked offline mid-fight keeps its slot, at 0 HP. That
/// slot must stop counting toward the round, or it sits forever
/// awaiting an action that nothing can supply: the menu for a downed
/// slot is empty, so no keypress can fill it and the round can never
/// resolve. The player would be stuck with Jack Out as their only move.
#[test]
fn a_slot_whose_member_was_knocked_out_stops_holding_the_round_open() {
    let mut game = Game::new(94, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 30, 5);
    game.add_companion(pet).unwrap();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    game.start_battle(vec![wild]);
    assert_eq!(game.world.resource::<BattleState>().planned.len(), 2);

    // Exactly what `wild_retaliate` leaves behind when a companion hits
    // 0 HP: still in the party, holding its slot, unable to act.
    game.world.get_mut::<Stats>(pet).unwrap().hp = 0;

    game.battle_set_action(0, BattleAction::Attack { group: 0 })
        .unwrap();
    assert_eq!(
        game.battle_active_slot(),
        None,
        "the empty slot must not be waiting on an action nothing can give it"
    );
    assert!(
        game.battle_round_ready(),
        "with the only living member planned, the round has to be resolvable"
    );

    let hp_before = game.world.get::<Stats>(wild).unwrap().hp;
    game.battle_resolve_round();
    assert!(
        game.world.get::<Stats>(wild).unwrap().hp < hp_before,
        "the round should actually have resolved"
    );
}

/// `BattleState::planned` indexes `Party` positionally (see
/// `actor_entity`), so dropping a member the instant it falls shifts
/// every member behind it forward a slot: the survivor answers to the
/// fallen member's slot, inherits whatever was planned for it, and takes
/// over its roster row. Membership therefore has to hold still for the
/// whole battle, with `slot_can_act` — not removal — keeping a downed
/// slot from holding the round open.
#[test]
fn a_companion_knocked_offline_keeps_its_slot_for_the_rest_of_the_battle() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let first = spawn_tamed(&mut game, 12, 5);
    let second = spawn_tamed(&mut game, 12, 5);
    game.add_companion(first).unwrap();
    game.add_companion(second).unwrap();

    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    {
        let mut w = game.world.get_mut::<Stats>(wild).unwrap();
        w.hp = 10_000;
        w.max_hp = 10_000;
        w.atk = 400;
    }
    game.start_battle(vec![wild]);
    // The player has to outlast the companions, or the battle ends
    // before the invariant can be observed.
    {
        let mut p = game.world.get_mut::<Stats>(game.player_entity()).unwrap();
        p.hp = 100_000;
        p.max_hp = 100_000;
    }

    let slot_owner: Vec<Entity> = game
        .battle_view()
        .unwrap()
        .party
        .iter()
        .map(|p| p.entity)
        .collect();
    assert_eq!(slot_owner.len(), 3, "player plus two companions");

    // Resolve until something falls. Bounded, and every round is a
    // no-choice Defend, so nothing here depends on the RNG landing a
    // particular way — only on the pack eventually connecting.
    let mut downed = false;
    for _ in 0..30 {
        if !game.has_active_battle() {
            break;
        }
        game.battle_plan_remaining(BattleAction::Defend).unwrap();
        game.battle_resolve_round();
        downed = [first, second]
            .iter()
            .any(|&e| game.world.get::<Stats>(e).is_none_or(|s| s.hp <= 0));
        if downed {
            break;
        }
    }
    assert!(downed, "the setup should have knocked a companion offline");
    assert!(
        game.has_active_battle(),
        "the fight has to still be running"
    );

    for (slot, &expected) in slot_owner.iter().enumerate() {
        assert_eq!(
            game.actor_entity(battle::Actor::Party(slot)),
            Some(expected),
            "slot {slot} changed hands mid-battle"
        );
    }
}

#[test]
fn battle_resolve_round_is_a_no_op_until_every_slot_is_planned() {
    let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 30, 6);
    game.add_companion(pet).unwrap();
    let wild = game.spawn_wild_creature("construct", 5, 5).unwrap();
    game.start_battle(vec![wild]);

    let hp_before = game.world.get::<Stats>(wild).unwrap().hp;
    game.battle_set_action(0, BattleAction::Attack { group: 0 })
        .unwrap();
    assert!(
        !game.battle_round_ready(),
        "the companion has no action yet"
    );
    game.battle_resolve_round();
    assert_eq!(
        game.world.get::<Stats>(wild).unwrap().hp,
        hp_before,
        "resolving a half-planned round must do nothing at all"
    );

    game.battle_set_action(1, BattleAction::Attack { group: 0 })
        .unwrap();
    assert!(game.battle_round_ready());
    game.battle_resolve_round();
    assert!(game.world.get::<Stats>(wild).unwrap().hp < hp_before);
}

/// The resolve popup used to title itself with the round number. With the
/// popup gone the log is the only place that boundary exists, so the
/// separator has to be logged exactly once and numbered to match the
/// planning header — not the post-increment round.
#[test]
fn resolving_a_round_logs_one_round_separator_numbered_for_the_round_that_ran() {
    let mut game = Game::new(77, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    insert_battle(&mut game, player, vec![wild]);

    let round_before = game.battle_view().unwrap().round;
    game.battle_set_action(0, BattleAction::Defend).unwrap();
    game.battle_resolve_round();

    let separators: Vec<String> = game
        .message_log(200)
        .into_iter()
        .filter(|(kind, _)| *kind == MessageKind::Round)
        .map(|(_, text)| text)
        .collect();
    assert_eq!(
        separators.len(),
        1,
        "one resolved round should log exactly one separator, got {separators:?}"
    );
    assert!(
        separators[0].contains(&round_before.to_string()),
        "the separator should name the round that just ran ({round_before}), got {:?}",
        separators[0]
    );
}

/// The whole party plans against the same group, and the first hit
/// wipes it — so every later actor in the initiative order is holding a
/// plan against a group that no longer exists, in a battle that has
/// already ended. The round must unwind cleanly rather than panic.
#[test]
fn a_round_survives_its_target_dying_before_every_member_has_acted() {
    let mut game = Game::new(85, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 40, 8);
    game.add_companion(pet).unwrap();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    game.start_battle(vec![wild]);
    // One HP, so whoever wins initiative ends the fight outright.
    game.world.get_mut::<Stats>(wild).unwrap().hp = 1;

    game.battle_set_action(0, BattleAction::Attack { group: 0 })
        .unwrap();
    game.battle_set_action(1, BattleAction::Attack { group: 0 })
        .unwrap();
    game.battle_resolve_round();

    assert!(
        !game.has_active_battle(),
        "the fight should have ended the moment the only group was wiped"
    );
}

/// The loss path: a round that kills the player has to end the fight,
/// clearing `BattleState` so the game-over handling isn't left running
/// against a battle that's still notionally active.
/// Permadeath rather than Forgiving, because a Forgiving flatline
/// soft-reboots the player back to life within the same tick — which
/// would make "did the player die?" unobservable after the fact.
#[test]
fn a_round_that_kills_the_player_ends_the_battle() {
    let mut game = Game::new(96, DifficultyMode::Permadeath, &test_assets_dir()).unwrap();
    // A wild program that hits far harder than the player can survive,
    // and with enough HP that the player can't end it first.
    let wild = game.spawn_wild_creature("construct", 5, 5).unwrap();
    {
        let mut w = game.world.get_mut::<Stats>(wild).unwrap();
        w.hp = 100_000;
        w.max_hp = 100_000;
        w.atk = 100_000;
    }
    game.start_battle(vec![wild]);

    game.battle_set_action(0, BattleAction::Attack { group: 0 })
        .unwrap();
    game.battle_resolve_round();

    assert!(
        game.is_game_over().is_some(),
        "the setup should have flatlined the player outright"
    );
    assert!(
        !game.has_active_battle(),
        "a fight the player lost has to be over, not left active"
    );
}

/// A Rally or Shield aimed at a companion has to die with the battle.
/// `CombatBuff` only ticks down inside one, and `effective_atk` /
/// `effective_def` read it unconditionally, so a survivor carries a free
/// stat bonus onto the overworld and into every fight after it.
#[test]
fn a_buff_aimed_at_a_companion_does_not_outlive_the_battle() {
    let mut game = Game::new(23, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 30, 5);
    game.add_companion(pet).unwrap();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    game.start_battle(vec![wild]);

    let def_before = game.effective_def(pet);
    game.use_special_ability(
        &SpecialAbility::Shield {
            power: 4,
            duration: 3,
        },
        "Test",
        pet,
    );
    assert!(
        game.effective_def(pet) > def_before,
        "the shield should be up while the fight runs"
    );

    game.battle_flee();
    assert_eq!(
        game.effective_def(pet),
        def_before,
        "the buff must not outlive the battle"
    );
}
