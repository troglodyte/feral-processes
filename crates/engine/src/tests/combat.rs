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
                mitigation: 1,
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);

    flee_until_clear(&mut game);

    assert_eq!(
        game.world.get::<Experience>(player).unwrap().xp,
        8,
        "fleeing should dock the same 20% setback as a death"
    );
    assert!(!game.has_active_battle(), "fleeing should end the battle");
}

/// Builds a fight the player cannot realistically escape — one enemy whose
/// stat line pins `jack_out_chance` to its floor — starts the player on 10
/// XP, and returns the game plus that seed's first jack-out result.
/// Sweeping seeds rather than pinning one keeps this off a specific RNG
/// sequence, the same pattern the retaliation-targeting tests use.
fn first_jack_out_against_an_overwhelming_pack(seed: u32) -> (Game, bool) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
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
                hp: 100_000,
                max_hp: 100_000,
                atk: 1,
                mitigation: 1,
            },
            StatusEffects::default(),
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);
    let escaped = game.battle_flee();
    (game, escaped)
}

#[test]
fn a_failed_jack_out_leaves_the_battle_running_and_costs_no_xp() {
    for seed in 0..60 {
        let (game, escaped) = first_jack_out_against_an_overwhelming_pack(seed);
        if escaped {
            continue;
        }
        let player = game.player_entity();
        assert!(
            game.has_active_battle(),
            "a failed jack-out must leave you in the fight"
        );
        assert_eq!(
            game.world.get::<Experience>(player).unwrap().xp,
            10,
            "a failed attempt must not dock XP — you only pay for an escape you got"
        );
        return;
    }
    panic!("no seed in 0..60 produced a failed jack-out against a 100k-power enemy");
}

#[test]
fn a_failed_jack_out_draws_a_parting_volley() {
    // **Summed across every failed attempt rather than read off the first
    // one.** The volley rolls to hit now, so any single pinned attempt can
    // cost nothing; totalling the whole sweep is deterministic and still
    // fails outright if being pinned draws no volley at all — which is the
    // thing that would break. Picking whichever seed happened to land would
    // pass against no volley being thrown.
    let mut pinned = 0;
    let mut total_lost = 0;
    for seed in 0..60 {
        let (game, escaped) = first_jack_out_against_an_overwhelming_pack(seed);
        if escaped {
            continue;
        }
        pinned += 1;
        let stats = game.world.get::<Stats>(game.player_entity()).unwrap();
        total_lost += stats.max_hp - stats.hp;
    }
    assert!(
        pinned > 0,
        "no seed in 0..60 produced a failed jack-out against a 100k-power enemy"
    );
    assert!(
        total_lost > 0,
        "{pinned} pinned attempts and not one of them cost a point of Integrity"
    );
}

/// A successful jack-out now shakes the pack that caught you, rather than
/// trying (and mathematically failing — see the fix's own history) to
/// outrun it: `battle_flee`'s successful path clears `Pursuing` from every
/// entity that was actually in the battle, before the `tick` that follows
/// would otherwise let `nest_aggro_tick` re-engage the same, still-adjacent
/// pack inside the same call. `NestGuardian` survives, so the guardian
/// resumes ordinary tethered wandering exactly like a `despawn_nest`
/// survivor — the nest re-provokes it the next time `attack_nest` lands.
#[test]
fn a_successful_jack_out_shakes_the_pack_that_caught_it() {
    let mut game = Game::new(730, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let nest = spawn_bare_nest(&mut game, 500, 500);
    let guardian = spawn_pursuing_guardian(&mut game, nest, "scrapper", 501, 500);
    insert_battle(&mut game, player, vec![guardian]);

    for _ in 0..200 {
        if game.battle_flee() {
            assert!(
                !game.has_active_battle(),
                "a successful jack-out must not have been immediately re-engaged"
            );
            assert!(
                game.world.get::<Pursuing>(guardian).is_none(),
                "the guardian that was actually in the fight should have been shaken loose"
            );
            assert!(
                game.world.get::<NestGuardian>(guardian).is_some(),
                "shaking the chase must not also untether the guardian from its nest"
            );
            return;
        }
    }
    panic!("200 jack-out attempts all failed against a trivial pursuer");
}

/// The scoping half of the same fix: a jack-out only shakes the pack that
/// was actually in the fight. A second guardian, pursuing but never
/// gathered into this battle, must keep chasing — otherwise fleeing one
/// nest's swarm would quietly call off every other chase in the zone too.
#[test]
fn a_successful_jack_out_does_not_shake_a_pursuer_outside_the_battle() {
    let mut game = Game::new(732, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();
    // Open ground generous enough that neither guardian's tile can be
    // "absent from the field" for a reason unrelated to what this test is
    // checking — natural terrain blocking the route, say.
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dx in -2..=12 {
            for dy in -2..=2 {
                map.set_override(
                    ppos.x + dx,
                    ppos.y + dy,
                    Tile {
                        biome: Biome::OpenGrid,
                        walkable: true,
                        rock_shade: None,
                    },
                );
            }
        }
    }
    let nest = spawn_bare_nest(&mut game, ppos.x + 2, ppos.y);
    let in_battle = spawn_pursuing_guardian(&mut game, nest, "scrapper", ppos.x + 1, ppos.y);
    // 8 from the nest (inside the 15-tile leash) and 10 from the player
    // (inside the 20-tile search box, but not adjacent — it must not reach
    // the player and start a second battle within this same tick) — so if
    // this loses `Pursuing`, that can only be this fix's own scoping, not
    // the ordinary leash or out-of-field rules `nest_aggro_tick` already
    // applies to every pursuer regardless of this fix.
    let elsewhere = spawn_pursuing_guardian(&mut game, nest, "scrapper", ppos.x + 10, ppos.y);
    insert_battle(&mut game, player, vec![in_battle]);

    for _ in 0..200 {
        if game.battle_flee() {
            assert!(
                game.world.get::<Pursuing>(in_battle).is_none(),
                "the guardian that was in the fight should have been shaken loose"
            );
            assert!(
                game.world.get::<Pursuing>(elsewhere).is_some(),
                "a guardian that was never gathered into this battle must keep chasing"
            );
            return;
        }
    }
    panic!("200 jack-out attempts all failed against a trivial pursuer");
}

#[test]
fn battle_flee_reports_whether_the_escape_happened() {
    // Against a trivial enemy the chance pins to its ceiling, so a handful
    // of attempts is overwhelmingly likely to include a success; the
    // assertion is that a `true` return and an ended battle agree.
    let mut game = Game::new(33, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);

    for _ in 0..200 {
        let escaped = game.battle_flee();
        assert_eq!(
            escaped,
            !game.has_active_battle(),
            "the return value must agree with whether the fight actually ended"
        );
        if escaped {
            return;
        }
    }
    panic!("200 jack-out attempts all failed against a trivial enemy");
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

    let (x, y) = multi_group_ground(&mut game);
    let solo = game.spawn_wild_creature(&first, x, y).unwrap();
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

    let a = game.spawn_wild_creature(&first, x, y).unwrap();
    let b = game.spawn_wild_creature(&second, x + 1, y).unwrap();
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
/// per-slot keys underneath them are Attack and Defend. Pinned here so a
/// future re-key cannot silently swap a brace for something else.
#[test]
fn battle_action_keys_are_lowercase_with_defend_on_d() {
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
    assert_eq!(key_for(ActionKind::Special), 's');
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
/// is still charged for commanding it.
#[test]
fn battle_set_action_refuses_an_out_of_range_ally_slot_or_ability() {
    let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 20, 5);
    enlist(&mut game, pet);
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
    enlist(&mut game, pet);
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
    // The swing has to land for the HP drop to be evidence the round ran.
    force_the_next_attack_to_land(&mut game);
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
    enlist(&mut game, first);
    enlist(&mut game, second);

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
    enlist(&mut game, pet);
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
        .filter(|e| e.kind == MessageKind::Round)
        .map(|e| e.text)
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
    enlist(&mut game, pet);
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
/// `effective_mitigation` read it unconditionally, so a survivor carries a free
/// stat bonus onto the overworld and into every fight after it.
#[test]
fn a_buff_aimed_at_a_companion_does_not_outlive_the_battle() {
    let mut game = Game::new(23, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 30, 5);
    enlist(&mut game, pet);
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    game.start_battle(vec![wild]);

    let def_before = game.effective_mitigation(pet);
    let shield = ability(&game, "sandbox");
    game.use_ability(&shield, pet, "Test", &[pet]);
    assert!(
        game.effective_mitigation(pet) > def_before,
        "the shield should be up while the fight runs"
    );

    // Retried rather than called once: jacking out is a roll now, and a
    // failed attempt leaves the shield up and the fight running, which
    // would fail this assertion for a reason that has nothing to do with
    // buff teardown.
    flee_until_clear(&mut game);
    assert!(!game.has_active_battle(), "the fight should be over");
    assert_eq!(
        game.effective_mitigation(pet),
        def_before,
        "the buff must not outlive the battle"
    );
}

/// A hidden row teaches nobody the feature exists; a greyed one with a
/// reason points at the research tree.
#[test]
fn the_player_is_offered_no_special_before_installing_a_routine() {
    let mut game = Game::new(37, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // The player starts with decompile installed; pop it out so the fixture
    // actually has nothing installed, which is the state under test.
    game.uninstall_routine(player, 0).unwrap();
    let enemy = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![enemy]);

    assert!(
        game.battle_action_options(0)
            .iter()
            .all(|o| o.kind != ActionKind::Special),
        "nothing is installed, so the row is hidden rather than greyed"
    );
}

#[test]
fn installing_a_researched_routine_makes_the_players_special_available() {
    let mut game = Game::new(38, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "self_exec");
    let player = game.player_entity();
    // Only one slot at level 1, and decompile already occupies it — free it
    // before installing the routine under test.
    game.uninstall_routine(player, 0).unwrap();
    give_disks(&mut game, 1);
    fit_routine(&mut game, player, "priority_boost");
    let enemy = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![enemy]);

    let special = game
        .battle_action_options(0)
        .into_iter()
        .find(|o| o.kind == ActionKind::Special)
        .expect("the installed routine shows the Special row");
    assert_eq!(special.unavailable, None);
    assert_eq!(
        special.detail, "Hyperthread Single v1.0",
        "one ability reads as its own name"
    );
}

/// Every `MessageKind` a resolved round can log, in the order the round
/// produced them.
fn logged_kinds(game: &Game) -> Vec<MessageKind> {
    game.message_log(200).into_iter().map(|e| e.kind).collect()
}

/// The kinds one `species`'s retaliation logs, across a fixed span of seeds.
/// Swept rather than pinned to one seed because `WILD_ABILITY_CHANCE` decides
/// per turn whether the program reaches for its move's effect — a single seed
/// would only ever show one side of that.
fn retaliation_kinds_across_seeds(species: &str) -> Vec<MessageKind> {
    (1..60u32)
        .flat_map(|seed| {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let player = game.player_entity();
            // A carrier also logs `EnemySpecial`, for a routine rather than a
            // move's effect — a different mechanism this test isn't about.
            // `spawn_wild_creature` rolls one in at `WILD_ROUTINE_CHANCE`
            // regardless of species, and 59 seeds make that near-certain
            // over the run, so it has to be stripped rather than left to
            // chance.
            let wild = spawn_wild_without_routine(&mut game, species, 5, 5);
            insert_battle(&mut game, player, vec![wild]);
            resolve_round_with(&mut game, BattleAction::Defend);
            logged_kinds(&game)
        })
        .collect()
}

/// A hostile blow logs `EnemySpecial` only when the program actually reached
/// for its move's status effect this turn — `wild_retaliate` decides the kind
/// *after* the `WILD_ABILITY_CHANCE` gate has had its say and cleared
/// `mv.effect`. That ordering is the whole point: taken before the gate, a
/// Crawler would read as a special on every single swing while the condition
/// landed on barely one in ten, which is a colour that means nothing.
///
/// Both species have a homogeneous moveset — Glitch's two moves are plain,
/// Crawler's two both carry a condition — so which move the RNG rolls cannot
/// affect the kind, and the gate is the only thing that can.
#[test]
fn an_enemy_logs_a_special_only_on_a_turn_it_reached_for_its_moves_effect() {
    let glitch = retaliation_kinds_across_seeds("glitch");
    assert!(
        glitch.contains(&MessageKind::EnemyAttack),
        "a retaliating glitch has to log its blow at all"
    );
    assert!(
        !glitch.contains(&MessageKind::EnemySpecial),
        "neither Glitch move carries a condition, so no gate roll can make one special"
    );

    let crawler = retaliation_kinds_across_seeds("crawler");
    assert!(
        crawler.contains(&MessageKind::EnemySpecial),
        "a Crawler that reached for its condition has to read as a special"
    );
    assert!(
        crawler.contains(&MessageKind::EnemyAttack),
        "a Crawler that did *not* reach for it swings as a plain attack — if this \
         never happens the kind is being decided before the WILD_ABILITY_CHANCE gate"
    );
}

/// A party member's hit is the one log line styled unevenly — the frontend
/// picks the number out of it — so it has to be distinguishable from the
/// enemy's blow in the same round rather than sharing `Info` with it.
#[test]
fn a_party_members_hit_is_logged_as_party_damage() {
    let mut game = Game::new(52, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    // Enough HP that the round runs to the enemy's turn too, so this also
    // pins the two sides apart rather than only checking one of them.
    game.world.get_mut::<Stats>(wild).unwrap().hp = 500;
    game.world.get_mut::<Stats>(wild).unwrap().max_hp = 500;
    insert_battle(&mut game, player, vec![wild]);

    resolve_round_with(&mut game, BattleAction::Attack { group: 0 });

    let damage_lines: Vec<String> = game
        .message_log(200)
        .into_iter()
        .filter(|e| e.kind == MessageKind::PartyDamage)
        .map(|e| e.text)
        .collect();
    assert_eq!(
        damage_lines.len(),
        1,
        "one attacking member should log one PartyDamage line, got {damage_lines:?}"
    );
    assert!(
        damage_lines[0].contains("damage"),
        "the line the frontend emphasises a number inside has to carry one: {:?}",
        damage_lines[0]
    );
    assert!(
        logged_kinds(&game).contains(&MessageKind::EnemyAttack),
        "the enemy's own blow must stay its own kind: {:?}",
        game.message_log(200)
    );
}

/// Sets up the same one-sided battle `a_party_members_hit_is_logged_as_
/// party_damage` does — enough wild HP that the round runs to completion
/// either way — so the four tests below only have to force a band and read
/// the party's own line back.
fn battle_for_swing_outcomes(seed: u32) -> (Game, Entity, Entity) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    game.world.get_mut::<Stats>(wild).unwrap().hp = 500;
    game.world.get_mut::<Stats>(wild).unwrap().max_hp = 500;
    insert_battle(&mut game, player, vec![wild]);
    (game, player, wild)
}

fn partys_swing_outcome(game: &Game) -> Option<SwingOutcome> {
    game.message_log(50)
        .into_iter()
        .find(|l| l.kind == MessageKind::PartyDamage)
        .expect("the player's swing logs a PartyDamage line")
        .outcome
}

/// Which band a party member's swing landed on rides along on the log line
/// (`resources::SwingOutcome`), which is what lets a frontend fire a
/// per-swing sound cue as the reveal releases that exact line rather than
/// one blip for the whole round. Four bands, four tests: three forced
/// through `support`'s matchup-independent fixtures, and the plain miss
/// through the matchup-aware one — `force_the_next_attack_to_miss` actually
/// forces a *fumble* (see its doc comment), so it is not a substitute for
/// the fourth.
///
/// All four resolve the swing through `support::player_swings_at_group`
/// rather than a full `resolve_round_with` round: `Game::roll_initiative`
/// draws once per living actor ahead of anyone's attack roll, which would
/// spend the forced draw on an initiative die instead of on the swing these
/// tests are pinning down.
#[test]
fn a_partys_crit_carries_the_swing_outcome() {
    let (mut game, ..) = battle_for_swing_outcomes(52);
    force_the_next_attack_to_crit(&mut game);
    player_swings_at_group(&mut game, 0);
    assert_eq!(partys_swing_outcome(&game), Some(SwingOutcome::Crit));
}

#[test]
fn a_partys_hit_carries_the_swing_outcome() {
    let (mut game, ..) = battle_for_swing_outcomes(52);
    force_the_next_attack_to_land(&mut game);
    player_swings_at_group(&mut game, 0);
    assert_eq!(partys_swing_outcome(&game), Some(SwingOutcome::Hit));
}

#[test]
fn a_partys_fumble_carries_the_swing_outcome() {
    let (mut game, ..) = battle_for_swing_outcomes(52);
    force_the_next_attack_to_miss(&mut game);
    player_swings_at_group(&mut game, 0);
    assert_eq!(partys_swing_outcome(&game), Some(SwingOutcome::Fumble));
}

#[test]
fn a_partys_plain_miss_carries_the_swing_outcome() {
    let (mut game, player, wild) = battle_for_swing_outcomes(52);
    let range = game.attack_range(player, crate::tuning::PLAYER_UNARMED_DAMAGE);
    force_the_next_attack_to_miss_plainly(&mut game, player, wild, battle::Swing::plain(range));
    player_swings_at_group(&mut game, 0);
    assert_eq!(partys_swing_outcome(&game), Some(SwingOutcome::Miss));
}
