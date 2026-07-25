//! Battle flow: initiative, grouping, targeting, planning, and resolving a round.

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

/// `wild_retaliate` rolls per-call whether a companion soaks the hit, so
/// this drives it across many seeds and checks both outcomes occur —
/// proof the roll is live, not that any single call behaves one way.
#[test]
fn wild_retaliation_can_land_on_either_the_player_or_the_companion() {
    let species_id = {
        let game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.species_defs()
            .into_iter()
            .next()
            .expect("at least one species")
            .id
            .clone()
    };

    let mut companion_hit = false;
    let mut player_hit = false;

    for seed in 0..60u32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let companion = spawn_tamed(&mut game, 1000, 1);
        game.add_companion(companion).unwrap();
        let player_hp_before = game.world.get::<Stats>(player).unwrap().hp;

        let wild = game
            .world
            .spawn((
                Creature {
                    species: species_id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 1000,
                    max_hp: 1000,
                    atk: 5,
                    def: 0,
                },
            ))
            .id();
        insert_battle(&mut game, player, vec![wild]);

        player_attacks(&mut game);

        let companion_hp = game.world.get::<Stats>(companion).unwrap().hp;
        let player_hp_after = game.world.get::<Stats>(player).unwrap().hp;
        if companion_hp < 1000 {
            companion_hit = true;
        }
        if player_hp_after < player_hp_before {
            player_hit = true;
        }
        if companion_hit && player_hit {
            break;
        }
    }

    assert!(
        companion_hit,
        "across 60 battles, the companion should have taken at least one hit"
    );
    assert!(
        player_hit,
        "across 60 battles, the player should have taken at least one hit"
    );
}

#[test]
fn effective_def_excludes_the_players_party_bonus_when_a_companion_is_the_target() {
    // `wild_retaliate` calls `effective_def` on whichever entity got
    // hit — the player, or (per the test above) a companion. The
    // player's passive party bonus (see `party_stat_bonus`) must only
    // ever land on the player, never get double-applied to a
    // companion's own defense just because it's a party member too.
    let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = spawn_tamed(&mut game, 10, 30);
    game.world.get_mut::<Stats>(a).unwrap().def = 20;
    game.add_companion(a).unwrap();
    // A second party member gives the *player's* bonus a nonzero,
    // easy-to-notice value if it ever leaked onto `a`.
    let b = spawn_tamed(&mut game, 10, 200);
    game.add_companion(b).unwrap();

    let raw_def = game.world.get::<Stats>(a).unwrap().def;
    assert_eq!(
        game.effective_def(a),
        raw_def,
        "a companion's effective DEF as a retaliation target must be its own raw Stats, \
         not inflated by the player's party bonus"
    );
}

#[test]
fn defeating_the_front_pack_member_continues_the_battle_against_the_next_one() {
    let species_id = {
        let game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.species_defs()
            .into_iter()
            .next()
            .expect("at least one species")
            .id
            .clone()
    };
    let mut game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.atk = 1000; // guarantees a one-shot kill on the front target below
    }
    let front = game
        .world
        .spawn((
            Creature {
                species: species_id.clone(),
            },
            Hostile,
            Position { x: 5, y: 5 },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                def: 0,
            },
        ))
        .id();
    let second = game
        .world
        .spawn((
            Creature {
                species: species_id.clone(),
            },
            Hostile,
            Position { x: 6, y: 5 },
            Stats {
                hp: 500,
                max_hp: 500,
                atk: 1,
                def: 0,
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![front, second]);

    player_attacks(&mut game);

    assert!(
        game.has_active_battle(),
        "a pack member is still alive, so the fight should continue rather than end"
    );
    let view = game
        .battle_view()
        .expect("battle should still be active with the second member up front");
    assert_eq!(
        view.groups.len(),
        1,
        "both members are the same species, so they share one group"
    );
    assert_eq!(
        view.groups[0].count, 1,
        "only the second (surviving) member should remain, now as the front"
    );
    assert_eq!(
        view.groups[0].front_hp, 500,
        "the new front should be the untouched second pack member"
    );
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
    let shield = ability(&game, "sandbox");
    game.use_ability(&shield, pet, "Test", &[pet]);
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

/// Defend has to actually reduce incoming damage, or it's a wasted turn
/// dressed up as a choice. Same seed both times: neither Defend nor the
/// player's flat strike draws from the RNG, so the two runs stay in
/// lockstep and the only difference is the DEF bonus.
#[test]
fn defending_reduces_the_damage_a_party_member_takes_this_round() {
    let damage_taken = |defend: bool| {
        let mut game = Game::new(89, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let wild = game.spawn_wild_creature("scrapper", 5, 5).unwrap();
        game.start_battle(vec![wild]);
        let player = game.player_entity();
        let before = game.world.get::<Stats>(player).unwrap().hp;
        game.battle_set_action(
            0,
            if defend {
                BattleAction::Defend
            } else {
                BattleAction::Attack { group: 0 }
            },
        )
        .unwrap();
        game.battle_resolve_round();
        before - game.world.get::<Stats>(player).unwrap().hp
    };
    assert!(
        damage_taken(true) < damage_taken(false),
        "a defended round must cost less HP than an undefended one"
    );
}

/// Defend is offered to companions, so a companion must be able to hold
/// the buff it grants. Only the player is spawned carrying a buff slot,
/// so without inserting one on demand a companion's Defend would log
/// its message and change nothing.
#[test]
fn a_companion_can_hold_the_buff_defend_grants() {
    let mut game = Game::new(90, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 30, 5);
    game.add_companion(pet).unwrap();
    let raw_def = game.world.get::<Stats>(pet).unwrap().def;

    game.begin_defend(pet);

    assert_eq!(
        game.effective_def(pet),
        raw_def + DEFEND_DEF_BONUS,
        "a bracing companion must actually gain the DEF, not silently no-op"
    );
}

/// Soft ranks, not hard ones: a back-slot member is hit *less*, never
/// *not at all*. Both halves matter — a version that made back slots
/// untouchable would pass a front-heavy assertion just as well, and
/// would quietly turn the roster into a wall of invulnerable reserves.
#[test]
fn back_slot_party_members_draw_less_fire_but_are_still_reachable() {
    let mut game = Game::new(92, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let mut slots = vec![player];
    for _ in 0..MAX_PARTY_SIZE {
        // Huge HP pools so nobody drops out of the pool mid-sample.
        let pet = spawn_tamed(&mut game, 100_000, 1);
        game.add_companion(pet).unwrap();
        slots.push(pet);
    }
    assert!(
        slots.len() > FRONT_SLOTS,
        "the sample needs at least one back slot to be meaningful"
    );

    let mut hits = vec![0u32; slots.len()];
    for _ in 0..4000 {
        let target = game.roll_enemy_target(player);
        let idx = slots.iter().position(|&e| e == target).unwrap();
        hits[idx] += 1;
    }

    let (front, back) = hits.split_at(FRONT_SLOTS);
    assert!(
        back.iter().all(|&h| h > 0),
        "every back slot must still be reachable, got {hits:?}"
    );
    let front_min = *front.iter().min().unwrap();
    let back_max = *back.iter().max().unwrap();
    assert!(
        front_min > back_max,
        "every front slot should outdraw every back slot, got {hits:?}"
    );
}

/// Bracing draws fire — that is what makes Defend a party-level play
/// rather than a selfish one.
#[test]
fn a_bracing_member_draws_more_fire_than_it_otherwise_would() {
    let sample = |brace: bool| {
        let mut game = Game::new(93, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let pet = spawn_tamed(&mut game, 100_000, 1);
        game.add_companion(pet).unwrap();
        if brace {
            game.begin_defend(pet);
        }
        (0..4000)
            .filter(|_| game.roll_enemy_target(player) == pet)
            .count()
    };
    assert!(
        sample(true) > sample(false),
        "a bracing companion must take more of the incoming fire"
    );
}

/// Party order is mechanically meaningful under soft ranks — front
/// slots draw more fire — so it has to survive a save/load round trip.
/// The roster order here deliberately differs from spawn order, which
/// is what the party used to be rebuilt from.
#[test]
fn party_order_survives_a_save_load_round_trip() {
    let dir = std::env::temp_dir().join("feral_party_order_roundtrip");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Distinct max HP is the identity that has to come back in order.
    let a = spawn_tamed(&mut game, 31, 3);
    let b = spawn_tamed(&mut game, 47, 3);
    let c = spawn_tamed(&mut game, 53, 3);
    for pet in [c, a, b] {
        game.add_companion(pet).unwrap();
    }
    let path = dir.join("slot.sav");
    game.save(&path).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let order: Vec<i32> = loaded
        .world
        .resource::<Party>()
        .0
        .iter()
        .filter_map(|&e| loaded.world.get::<Stats>(e).map(|s| s.max_hp))
        .collect();
    assert_eq!(
        order,
        vec![53, 31, 47],
        "party order must round-trip exactly, not fall back to spawn order"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The reach rule is the balance valve that makes a big multi-group
/// fight survivable. A back group with only melee moves can't connect
/// at all.
#[test]
fn a_back_group_with_only_melee_moves_cannot_reach_the_party() {
    let mut game = Game::new(86, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Scrapper, Sentinel and Construct are authored melee-only.
    let a = game.spawn_wild_creature("scrapper", 5, 5).unwrap();
    let b = game.spawn_wild_creature("sentinel", 5, 6).unwrap();
    let c = game.spawn_wild_creature("construct", 5, 7).unwrap();
    game.start_battle(vec![a, b, c]);
    let player = game.player_entity();
    let hp_before = game.world.get::<Stats>(player).unwrap().hp;

    // Group 2 (Construct) is behind the engaged pair and melee-only.
    let construct = game.front_of_group(2).unwrap();
    for _ in 0..20 {
        game.wild_retaliate(construct, 2, player);
    }

    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        hp_before,
        "a melee-only back group must deal no damage"
    );
}

/// ...but a back group holding a ranged move connects normally. Without
/// this half, the test above would pass just as well against a bug that
/// makes back groups unconditionally inert.
#[test]
fn a_back_group_with_a_ranged_move_still_connects() {
    let mut game = Game::new(87, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = game.spawn_wild_creature("scrapper", 5, 5).unwrap();
    let b = game.spawn_wild_creature("sentinel", 5, 6).unwrap();
    // Glitch's "Static Burst" is authored ranged.
    let c = game.spawn_wild_creature("glitch", 5, 7).unwrap();
    game.start_battle(vec![a, b, c]);
    let player = game.player_entity();
    let hp_before = game.world.get::<Stats>(player).unwrap().hp;

    let glitch = game.front_of_group(2).unwrap();
    game.wild_retaliate(glitch, 2, player);

    assert!(
        game.world.get::<Stats>(player).unwrap().hp < hp_before,
        "a ranged back group must be able to land a hit"
    );
}

/// An engaged group picks from its whole moveset, ranged or not —
/// the restriction is about distance, not about the moves themselves.
#[test]
fn an_engaged_group_still_uses_its_melee_moves() {
    let mut game = Game::new(88, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let construct = game.spawn_wild_creature("construct", 5, 5).unwrap();
    game.start_battle(vec![construct]);
    let player = game.player_entity();
    let hp_before = game.world.get::<Stats>(player).unwrap().hp;

    game.wild_retaliate(construct, 0, player);

    assert!(
        game.world.get::<Stats>(player).unwrap().hp < hp_before,
        "a melee-only species in the front rank must still hit"
    );
}

/// A planned target can die earlier in the same round than the member
/// who aimed at it, leaving a stale group index behind. Falling back to
/// the front group is the difference between a wasted turn and an
/// out-of-bounds panic.
#[test]
fn a_stale_target_group_index_falls_back_to_the_front_group() {
    let mut game = Game::new(84, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let glitch = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    let scrapper = game.spawn_wild_creature("scrapper", 5, 6).unwrap();
    game.start_battle(vec![glitch, scrapper]);

    assert_eq!(game.retarget(1), Some(1), "group 1 is standing");

    game.world.get_mut::<Stats>(scrapper).unwrap().hp = 0;
    game.finish_group_member(1, player);

    assert_eq!(
        game.retarget(1),
        Some(0),
        "a stale index must fall back to the lowest surviving group"
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

/// Initiative order must be reproducible under a fixed seed. Every roll
/// goes through the existing `GameRng`, so a seeded test can assert an
/// exact order without touching the wall clock.
#[test]
fn initiative_order_is_reproducible_under_a_fixed_seed() {
    let order_for = |seed: u32| {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        let b = game.spawn_wild_creature("construct", 5, 6).unwrap();
        game.start_battle(vec![a, b]);
        game.roll_initiative()
    };
    assert_eq!(order_for(1234), order_for(1234), "same seed, same order");
}

/// Speed has to actually bias the order, or the stat is decoration.
/// Sampled rather than asserted per-round: a d10 on top of an 8-point
/// gap still lets the Construct win occasionally, and a test that
/// forbade that would be asserting the die doesn't exist.
#[test]
fn a_faster_species_wins_initiative_far_more_often_than_a_slower_one() {
    let mut sprite_first = 0;
    for seed in 0..200u32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let sprite = game.spawn_wild_creature("sprite", 5, 5).unwrap();
        let construct = game.spawn_wild_creature("construct", 5, 6).unwrap();
        game.start_battle(vec![sprite, construct]);
        let order = game.roll_initiative();
        let pos = |e: Entity| {
            order
                .iter()
                .position(|a| game.actor_entity(*a) == Some(e))
                .unwrap()
        };
        if pos(sprite) < pos(construct) {
            sprite_first += 1;
        }
    }
    assert!(
        sprite_first > 150,
        "a Sprite (14) should beat a Construct (6) far more often than not, got {sprite_first}/200"
    );
}

/// A pack partitions into one group per species, in first-appearance
/// order. `gather_pack` walks an ECS query, so the deterministic order
/// has to come from the partition step itself — an incidental query
/// order is exactly the kind of thing that produced this repo's
/// unsorted-habitat-lookup flake.
#[test]
fn a_mixed_pack_partitions_into_one_group_per_species_in_first_appearance_order() {
    let mut game = Game::new(77, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    let b = game.spawn_wild_creature("scrapper", 5, 6).unwrap();
    let c = game.spawn_wild_creature("glitch", 5, 7).unwrap();
    let d = game.spawn_wild_creature("scrapper", 6, 5).unwrap();

    game.start_battle(vec![a, b, c, d]);

    let battle = game.world.resource::<BattleState>();
    assert_eq!(battle.groups.len(), 2, "two species means two groups");
    assert_eq!(battle.groups[0].species, "glitch", "glitch appeared first");
    assert_eq!(battle.groups[0].members, vec![a, c]);
    assert_eq!(battle.groups[1].species, "scrapper");
    assert_eq!(battle.groups[1].members, vec![b, d]);
}

/// Only `MAX_ENEMY_GROUPS` species can engage at once. The overflow stays
/// on the map as ordinary hostiles rather than being despawned — the
/// player meets them on the next bump.
#[test]
fn a_pack_of_more_than_four_species_engages_the_four_largest_and_leaves_the_rest() {
    let mut game = Game::new(78, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // glitch x3, scrapper x2, virus x2, worm x2, sprite x1 -> sprite is
    // the smallest group and the one left out.
    let mut spawned = Vec::new();
    for (species, count) in [
        ("glitch", 3),
        ("scrapper", 2),
        ("virus", 2),
        ("worm", 2),
        ("sprite", 1),
    ] {
        for i in 0..count {
            spawned.push(game.spawn_wild_creature(species, 5, 5 + i).unwrap());
        }
    }
    let sprite = *spawned.last().unwrap();

    game.start_battle(spawned.clone());

    let battle = game.world.resource::<BattleState>();
    assert_eq!(battle.groups.len(), MAX_ENEMY_GROUPS);
    assert!(
        battle.groups.iter().all(|g| g.species != "sprite"),
        "the smallest group should be the one left out"
    );
    assert!(
        game.world.get_entity(sprite).is_ok(),
        "an un-engaged hostile must stay on the map, never be despawned"
    );
}

/// Wiping the front group promotes whatever sat behind it — the central
/// tension of the reach rule: clearing front-to-back is not
/// automatically correct, because it walks the back rank into melee.
#[test]
fn wiping_the_front_group_promotes_the_group_behind_it() {
    let mut game = Game::new(79, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let glitch = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    let scrapper = game.spawn_wild_creature("scrapper", 5, 6).unwrap();
    game.start_battle(vec![glitch, scrapper]);
    let player = game.player_entity();

    assert_eq!(
        game.world.resource::<BattleState>().groups[0].species,
        "glitch"
    );

    game.world.get_mut::<Stats>(glitch).unwrap().hp = 0;
    let battle_over = game.finish_group_member(0, player);

    assert!(!battle_over, "the scrapper group is still standing");
    let battle = game.world.resource::<BattleState>();
    assert_eq!(battle.groups.len(), 1);
    assert_eq!(
        battle.groups[0].species, "scrapper",
        "the surviving group should have shifted into index 0"
    );
}

#[test]
fn gather_pack_pulls_in_nearby_hostiles_and_caps_at_max_pack_size() {
    let species_id = {
        let game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.species_defs()
            .into_iter()
            .next()
            .expect("at least one species")
            .id
            .clone()
    };
    let mut game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    // Far enough out that zone 1's pack cap (2) is fully unlocked.
    let (ax, ay) = (spawn.x + PACK_SIZE_STEP_TILES * 5, spawn.y);
    let spawn_hostile = |game: &mut Game, x: i32, y: i32| {
        game.world
            .spawn((
                Creature {
                    species: species_id.clone(),
                },
                Hostile,
                Position { x, y },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    def: 0,
                },
            ))
            .id()
    };
    let anchor = spawn_hostile(&mut game, ax, ay);
    for i in 1..=3 {
        spawn_hostile(&mut game, ax + i, ay);
    }

    let pack = game.gather_pack(anchor);

    assert_eq!(
        pack[0], anchor,
        "the creature actually bumped into should always be the pack's front"
    );
    assert_eq!(
        pack.len(),
        PACK_SIZE_PER_ZONE as usize,
        "zone 1's pack cap should bind with 3 other Hostiles in range"
    );
}

#[test]
fn a_creatures_display_label_is_tagged_with_its_spawn_zone() {
    let mut game = Game::new(50, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game.species_defs().into_iter().next().unwrap();

    let zone1 = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 3, y: 3 },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                def: 1,
            },
            ZonePortal(1),
        ))
        .id();
    let zone2 = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 4, y: 4 },
            Stats {
                hp: 2,
                max_hp: 2,
                atk: 2,
                def: 2,
            },
            ZonePortal(2),
        ))
        .id();

    assert_eq!(game.entity_label(zone1), format!("{} 1", species.name));
    assert_eq!(game.entity_label(zone2), format!("{} 2", species.name));
    assert_eq!(
        game.inspect(zone2).unwrap().name,
        format!("{} 2", species.name)
    );
}

#[test]
fn defeating_a_boss_guarantees_a_cache_of_portal_fragments() {
    let mut game = Game::new(51, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let boss = game
        .species_defs()
        .into_iter()
        .find(|s| s.is_boss)
        .expect("at least one boss species should exist in assets/species for this test");

    let wild = game
        .world
        .spawn((
            Creature {
                species: boss.id.clone(),
            },
            Position { x: 0, y: 0 },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                def: 1,
            },
        ))
        .id();

    game.award_loot(wild);

    let qty = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::PORTAL_FRAGMENT));
    assert!(
        BOSS_PORTAL_FRAGMENT_DROP.contains(&qty),
        "boss kill should guarantee a portal fragment cache in {BOSS_PORTAL_FRAGMENT_DROP:?}, got {qty}"
    );
}

#[test]
fn stunned_player_loses_their_turn_but_wild_still_retaliates_and_stun_clears() {
    let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // Deliberately effect-free, so the wild creature's own retaliation
    // can't re-apply (and thus reset the clock on) the status this test
    // is tracking.
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss && s.moves.iter().all(|m| m.effect.is_none()))
        .expect("at least one species with no status-effect moves should exist for this test");
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 5, y: 5 },
            Stats {
                hp: 50,
                max_hp: 50,
                atk: 3,
                def: 0,
            },
            StatusEffects::default(),
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);
    game.world.get_mut::<StatusEffects>(player).unwrap().active = Some(ActiveStatus {
        kind: StatusKind::Stun,
        remaining: 1,
        power: 0,
    });

    let wild_hp_before = game.world.get::<Stats>(wild).unwrap().hp;
    player_attacks(&mut game);
    let wild_hp_after = game.world.get::<Stats>(wild).unwrap().hp;

    assert_eq!(
        wild_hp_before, wild_hp_after,
        "a stunned player shouldn't deal any attack damage"
    );
    assert!(
        game.world
            .get::<StatusEffects>(player)
            .unwrap()
            .active
            .is_none(),
        "the stun should clear after its one round elapses"
    );
}

#[test]
fn bleed_status_deals_extra_damage_each_round_and_expires_after_its_duration() {
    let mut game = Game::new(62, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // Deliberately effect-free, so the wild creature's own retaliation
    // can't re-apply (and thus reset the clock on) the status this test
    // is tracking.
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss && s.moves.iter().all(|m| m.effect.is_none()))
        .expect("at least one species with no status-effect moves should exist for this test");
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 5, y: 5 },
            Stats {
                hp: 100,
                max_hp: 100,
                atk: 0,
                def: 0,
            },
            StatusEffects {
                active: Some(ActiveStatus {
                    kind: StatusKind::Bleed,
                    remaining: 2,
                    power: 5,
                }),
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);
    let player_atk = game.world.get::<Stats>(player).unwrap().atk;
    let expected_attack_dmg = battle::compute_damage(player_atk, 0, 5);

    let hp_before = game.world.get::<Stats>(wild).unwrap().hp;
    player_attacks(&mut game);
    let hp_after = game.world.get::<Stats>(wild).unwrap().hp;
    assert_eq!(
        hp_before - hp_after,
        expected_attack_dmg + 5,
        "wild should take its attack damage plus one round of bleed"
    );
    assert_eq!(
        game.world
            .get::<StatusEffects>(wild)
            .unwrap()
            .active
            .unwrap()
            .remaining,
        1
    );

    let hp_before2 = game.world.get::<Stats>(wild).unwrap().hp;
    player_attacks(&mut game);
    let hp_after2 = game.world.get::<Stats>(wild).unwrap().hp;
    assert_eq!(
        hp_before2 - hp_after2,
        expected_attack_dmg + 5,
        "the second bleed round should also tick"
    );
    assert!(
        game.world
            .get::<StatusEffects>(wild)
            .unwrap()
            .active
            .is_none(),
        "bleed should clear once its duration elapses"
    );
}

#[test]
fn status_effects_are_cleared_once_the_battle_ends() {
    let mut game = Game::new(63, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // Deliberately effect-free, so the wild creature's own retaliation
    // can't re-apply (and thus reset the clock on) the status this test
    // is tracking.
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss && s.moves.iter().all(|m| m.effect.is_none()))
        .expect("at least one species with no status-effect moves should exist for this test");
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 5, y: 5 },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                def: 0,
            },
            StatusEffects::default(),
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);
    game.world.get_mut::<StatusEffects>(player).unwrap().active = Some(ActiveStatus {
        kind: StatusKind::Bleed,
        remaining: 5,
        power: 1,
    });

    // 1 HP wild creature dies to the player's first attack, ending the battle.
    player_attacks(&mut game);

    assert!(
        !game.has_active_battle(),
        "the wild creature's death should end the battle"
    );
    assert!(
        game.world
            .get::<StatusEffects>(player)
            .unwrap()
            .active
            .is_none(),
        "leftover status effects should be cleared once the battle ends, however it ends"
    );
}
