//! `Game::kit_of`: the one answer to where a body's kit comes from.
//!
//! The player-side tests pin what the four readers answered for the player
//! before they became matches on `Kit`, so converting them has a witness.

use super::support::*;
use super::tactical::{body, log_texts, tactical_fight};
use crate::abilities::AbilityId;
use crate::components::Perks;
use crate::game::kit::Kit;
use crate::perks::{Perk, emulation_fidelity_level};
use crate::progression::{emulated_stats, stats_after_levels};
use crate::tuning::{PLAYER_UNARMED_DAMAGE, TACTICAL_MELEE_RANGE};
use crate::*;

fn game() -> Game {
    Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn drone(game: &Game) -> crate::species::SpeciesDef {
    game.world
        .resource::<crate::species::SpeciesDb>()
        .get("drone")
        .unwrap()
        .clone()
}

fn innate_species(game: &Game, entity: bevy_ecs::entity::Entity) -> Option<String> {
    match game.kit_of(entity) {
        Kit::Innate(def) => Some(def.id.clone()),
        Kit::Unarmed | Kit::Emulated { .. } => None,
    }
}

#[test]
fn the_player_is_unarmed() {
    let game = game();
    assert!(matches!(game.kit_of(game.player_entity()), Kit::Unarmed));
}

#[test]
fn a_wild_program_carries_its_species_kit() {
    let mut game = game();
    let drone = body(&mut game, "drone");
    assert_eq!(innate_species(&game, drone).as_deref(), Some("drone"));
}

#[test]
fn a_companion_carries_its_species_kit() {
    let mut game = game();
    let pet = spawn_tamed(&mut game, 30, 6);
    let species = game
        .world
        .get::<crate::Creature>(pet)
        .unwrap()
        .species
        .clone();
    assert_eq!(innate_species(&game, pet), Some(species));
}

/// A body whose species no longer resolves — a mod pulled out from under a
/// save — has no kit to borrow, and reads as unarmed rather than panicking.
#[test]
fn a_body_of_an_unknown_species_is_unarmed() {
    let mut game = game();
    let stray = body(&mut game, "no_such_species");
    assert!(matches!(game.kit_of(stray), Kit::Unarmed));
}

#[test]
fn the_unarmed_player_swings_a_data_strike() {
    let mut game = game();
    let player = game.player_entity();
    let (name, band) = game.swing_move_at(player, None);
    assert_eq!(name, "data strike");
    assert_eq!(band, PLAYER_UNARMED_DAMAGE);
}

#[test]
fn the_unarmed_players_natural_band_is_the_unarmed_band() {
    let game = game();
    let player = game.player_entity();
    assert_eq!(
        game.natural_range_of(player),
        game.attack_range(player, PLAYER_UNARMED_DAMAGE)
    );
}

#[test]
fn the_unarmed_player_swings_at_arms_length() {
    let game = game();
    assert_eq!(game.swing_range(game.player_entity()), TACTICAL_MELEE_RANGE);
}

/// The two readers that guard their `Unarmed` arm on the player: a body
/// with no species to borrow from is not handed the player's strike name
/// or the player's class affinity.
#[test]
fn a_stray_body_swings_a_raw_signal_burst() {
    let mut game = game();
    let stray = body(&mut game, "no_such_species");
    let (name, band) = game.swing_move_at(stray, None);
    assert_eq!(name, "a raw signal burst");
    assert_eq!(band, PLAYER_UNARMED_DAMAGE);
}

/// A Medic, so the player's `Heal` affinity sits above neutral and a stray
/// body handed the player's arm would read above neutral too.
#[test]
fn a_stray_body_has_no_class_affinity() {
    let choice = crate::CharacterChoice {
        class: Some(crate::classes::PlayerClass::Medic),
        ..crate::CharacterChoice::default()
    };
    let mut game =
        Game::new_with(4, DifficultyMode::Forgiving, &test_assets_dir(), &choice).unwrap();
    let stray = body(&mut game, "no_such_species");
    let heal = crate::abilities::AbilityEffect::Heal {
        power: 10,
        spread: 0,
    };
    let neutral = crate::tuning::AFFINITY_NEUTRAL;
    assert!(game.ability_affinity(game.player_entity(), &heal) > neutral);
    assert_eq!(game.ability_affinity(stray, &heal), neutral);
}

/// `progression::emulated_stats`: an emulation's strength, todo #100 Task 2.
mod emulated_stats_tests {
    use super::*;

    /// `EMULATION_EDGE` is above 1.0, so an emulation beats a wild program
    /// of the same species at the same level — see the constant's doc.
    #[test]
    fn an_emulations_atk_exceeds_the_wild_growth_it_is_built_on() {
        let game = game();
        let def = drone(&game);
        let base = Stats {
            hp: def.base_hp,
            max_hp: def.base_hp,
            atk: def.base_atk,
            mitigation: def.base_mitigation,
        };
        let grown = stats_after_levels(base, 9, def.growth_multiplier);

        let emulated = emulated_stats(&def, 10, 0);

        assert!(
            emulated.atk > grown.atk,
            "emulated atk {} should beat the wild growth it is scaled from ({})",
            emulated.atk,
            grown.atk
        );
    }

    /// Species base stats are level 1, so growing "to the player's level"
    /// is `levels_gained = player_level - 1`.
    #[test]
    fn growth_stops_one_level_short_of_the_player_level() {
        let game = game();
        let def = drone(&game);

        let level_one = emulated_stats(&def, 1, 0);
        let level_two = emulated_stats(&def, 2, 0);

        assert_eq!(
            level_one.atk,
            (def.base_atk as f32 * crate::tuning::EMULATION_EDGE).round() as i32,
            "at player level 1 the species has gained no levels yet"
        );
        assert!(
            level_two.atk > level_one.atk,
            "a level 2 player should grow the image past its level-1 figure"
        );
    }

    #[test]
    fn each_fidelity_level_raises_atk() {
        let game = game();
        let def = drone(&game);

        let unperked = emulated_stats(&def, 10, 0);
        let one_level = emulated_stats(&def, 10, 1);
        let two_levels = emulated_stats(&def, 10, 2);

        assert!(one_level.atk > unperked.atk);
        assert!(two_levels.atk > one_level.atk);
    }

    /// Mitigation is percentage points and never scaled by level (see
    /// `components::Stats::mitigation`), but it does take the same
    /// fidelity multiplier as attack, rounded — the cap stays
    /// `Game::effective_mitigation`'s job.
    #[test]
    fn mitigation_is_unscaled_by_level_but_takes_the_multiplier() {
        let game = game();
        let def = drone(&game);

        let low_level = emulated_stats(&def, 1, 0);
        let high_level = emulated_stats(&def, 40, 0);
        assert_eq!(
            low_level.mitigation, high_level.mitigation,
            "mitigation must not grow with the player's level"
        );
        assert_eq!(
            low_level.mitigation,
            (def.base_mitigation as f32 * crate::tuning::EMULATION_EDGE).round() as i32
        );

        let perked = emulated_stats(&def, 1, 3);
        assert!(
            perked.mitigation > low_level.mitigation,
            "a higher fidelity level should still raise mitigation via the multiplier"
        );
    }

    /// `emulation_fidelity_level` is the perk's named query —
    /// `every_perk_has_a_query_that_answers_what_it_is_worth` (perks.rs)
    /// fails to compile until `Perk::EmulationFidelity` has one.
    #[test]
    fn emulation_fidelity_level_reads_the_perk() {
        assert_eq!(emulation_fidelity_level(None), 0);

        let bought = Perks {
            points: 0,
            unlocked: vec![Perk::EmulationFidelity, Perk::EmulationFidelity],
        };
        assert_eq!(emulation_fidelity_level(Some(&bought)), 2);
    }
}

/// `Kit::Emulated` and every reader answering it, todo #100 Task 3.
mod emulation_tests {
    use super::*;

    /// A hostile with `hp` and `atk` set by hand, standing far enough away
    /// that nothing else in the world touches it.
    fn overwhelmed_hostile(game: &mut Game, hp: i32, atk: i32) -> Entity {
        let species = game
            .species_defs()
            .into_iter()
            .next()
            .expect("at least one species ships");
        game.world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 3, y: 3 },
                Stats {
                    hp,
                    max_hp: hp,
                    atk,
                    mitigation: 0,
                },
                StatusEffects::default(),
            ))
            .id()
    }

    #[test]
    fn kit_of_answers_emulated_for_an_emulating_player() {
        let mut game = game();
        let player = game.player_entity();
        let def = drone(&game);
        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 3,
        });

        match game.kit_of(player) {
            Kit::Emulated { def: got, .. } => assert_eq!(got.id, def.id),
            Kit::Unarmed | Kit::Innate(_) => panic!("expected Kit::Emulated"),
        }
    }

    /// `EMULATION_EDGE` is what makes this true — see the constant's doc and
    /// `progression::emulated_stats`'s own tests.
    #[test]
    fn an_emulating_player_outhits_a_same_level_wild_program_of_the_species() {
        let mut game = game();
        let player = game.player_entity();
        let def = drone(&game);
        game.world.get_mut::<Experience>(player).unwrap().level = 10;

        let wild = body(&mut game, &def.id);
        let base = Stats {
            hp: def.base_hp,
            max_hp: def.base_hp,
            atk: def.base_atk,
            mitigation: def.base_mitigation,
        };
        let grown = stats_after_levels(base, 9, def.growth_multiplier);
        *game.world.get_mut::<Stats>(wild).unwrap() = grown;

        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 3,
        });

        assert!(
            game.effective_atk(player) > game.effective_atk(wild),
            "an emulating player ({}) should outhit a same-level wild program \
             of the species it is wearing ({})",
            game.effective_atk(player),
            game.effective_atk(wild)
        );
    }

    /// `effective_atk`/`effective_mitigation`'s base is `stats.atk +
    /// gear_bonus(entity).atk` while emulating (constraints.md decision 4)
    /// — worn gear is not shadowed by the swap.
    #[test]
    fn worn_gear_still_adds_atk_and_mitigation_while_emulating() {
        let mut game = game();
        let player = game.player_entity();
        let def = drone(&game);
        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 3,
        });

        let atk_before = game.effective_atk(player);
        let mitigation_before = game.effective_mitigation(player);

        equip_weapon(&mut game, player, "shim_blade"); // atk: 2
        equip_armor(&mut game, player, "ablative_plating"); // mitigation: 12

        assert_eq!(
            game.effective_atk(player),
            atk_before + 2,
            "worn atk should still land on top of the emulated base"
        );
        assert_eq!(
            game.effective_mitigation(player),
            mitigation_before + 12,
            "worn mitigation should still land on top of the emulated base"
        );
    }

    /// The cap is applied inside `effective_mitigation` itself, so a body
    /// summing to more than `MAX_MITIGATION_PERCENT` while emulating is held
    /// down exactly as any other body's is.
    #[test]
    fn mitigation_caps_while_emulating() {
        let mut game = game();
        let player = game.player_entity();
        let def = drone(&game);
        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 3,
        });
        game.arm_field_buff(
            player,
            ActiveFieldBuff {
                kind: FieldBuffKind::Mitigation,
                name: "Test Field Buff".to_string(),
                power: 1_000,
                remaining: 5,
                interval: 1,
                source: BuffSource::Routine,
            },
        );

        assert_eq!(
            game.effective_mitigation(player),
            crate::tuning::MAX_MITIGATION_PERCENT
        );
    }

    /// Emulating, lapsing and teardown all leave `Stats` alone — spec §1's
    /// "`Stats` is never written by this feature." Exercised directly
    /// against `tick_combatant_upkeep` and `clear_battle_status_effects`
    /// rather than through a live exchange, so this cannot flake on the
    /// species' own move dealing damage independent of `atk`.
    #[test]
    fn hp_is_unchanged_by_emulating_lapsing_and_teardown() {
        let mut game = game();
        let player = game.player_entity();
        let def = drone(&game);
        let before_hp = game.world.get::<Stats>(player).unwrap().hp;

        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 1,
        });
        assert_eq!(
            game.world.get::<Stats>(player).unwrap().hp,
            before_hp,
            "emulating must not touch HP"
        );

        game.tick_combatant_upkeep(player);
        assert!(
            game.world.get::<Emulation>(player).is_none(),
            "one round of upkeep should have lapsed a one-round emulation"
        );
        assert_eq!(
            game.world.get::<Stats>(player).unwrap().hp,
            before_hp,
            "lapsing must not touch HP"
        );

        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 5,
        });
        game.clear_battle_status_effects(player, None);
        assert!(
            game.world.get::<Emulation>(player).is_none(),
            "teardown must clear the emulation too"
        );
        assert_eq!(
            game.world.get::<Stats>(player).unwrap().hp,
            before_hp,
            "teardown must not touch HP"
        );
    }

    #[test]
    fn actor_abilities_reads_the_species_list_while_emulating_and_the_players_own_after() {
        let mut game = game();
        let player = game.player_entity();
        let before: Vec<AbilityId> = game
            .actor_abilities(player)
            .iter()
            .map(|a| a.id.clone())
            .collect();

        let def = drone(&game);
        game.world.get_mut::<Experience>(player).unwrap().level = 10;
        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 3,
        });

        let while_emulating: Vec<AbilityId> = game
            .actor_abilities(player)
            .iter()
            .map(|a| a.id.clone())
            .collect();
        assert!(
            while_emulating.contains(&"skim_group".to_string()),
            "drone's level-2 ability should be offered at player level 10: {while_emulating:?}"
        );
        assert!(
            while_emulating.contains(&"skim_v1".to_string()),
            "drone's level-4 ability should be offered at player level 10: {while_emulating:?}"
        );

        game.world.entity_mut(player).remove::<Emulation>();
        let after: Vec<AbilityId> = game
            .actor_abilities(player)
            .iter()
            .map(|a| a.id.clone())
            .collect();
        assert_eq!(
            after, before,
            "the player's own routines should return once the emulation ends"
        );
    }

    #[test]
    fn actor_abilities_caps_the_emulated_list_at_routine_slots() {
        let mut game = game();
        let player = game.player_entity();
        let mut species = generic_species();
        species.abilities = vec![
            crate::species::SpeciesAbility {
                id: "priority_boost".to_string(),
                level: 1,
            },
            crate::species::SpeciesAbility {
                id: "skim_group".to_string(),
                level: 1,
            },
            crate::species::SpeciesAbility {
                id: "skim_v1".to_string(),
                level: 1,
            },
        ];
        let id = species.id.clone();
        game.world
            .resource_mut::<crate::species::SpeciesDb>()
            .insert(species);

        game.world.entity_mut(player).insert(Emulation {
            species: id,
            rounds_left: 3,
        });

        let slots = game.routine_slots(player);
        assert!(
            slots < 3,
            "the fixture needs fewer slots than abilities to prove the cap holds"
        );
        assert_eq!(game.actor_abilities(player).len(), slots);
    }

    /// The lapse: `tick_one_combatant` ages `rounds_left`, and at zero
    /// `Game::drop_emulation` removes the component and logs the line.
    #[test]
    fn an_emulation_lapses_after_its_rounds_in_the_group_model() {
        let mut game = game();
        let player = game.player_entity();
        let def = drone(&game);
        let hostile = overwhelmed_hostile(&mut game, 100_000, 0);
        insert_battle(&mut game, player, vec![hostile]);
        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 2,
        });

        resolve_round_with(&mut game, BattleAction::Defend);
        assert!(
            game.world.get::<Emulation>(player).is_some(),
            "a two-round emulation must still be active after one round"
        );

        resolve_round_with(&mut game, BattleAction::Defend);
        assert!(
            game.world.get::<Emulation>(player).is_none(),
            "a two-round emulation must have lapsed after two rounds"
        );
        assert!(
            log_texts(&game)
                .iter()
                .any(|t| t == "Your emulation lapses."),
            "the lapse must be logged"
        );
    }

    /// The tactical model's own upkeep, `tactical_round_upkeep`, calls the
    /// same `tick_combatant_upkeep` the group model does — this is that
    /// door's own test.
    #[test]
    fn an_emulation_lapses_after_its_rounds_on_a_battle_map() {
        let mut game = game();
        let player = game.player_entity();
        let def = drone(&game);
        tactical_fight(&mut game, 1, 100_000);
        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 2,
        });

        for _ in 0..64 {
            if game.world.get::<Emulation>(player).is_none() {
                break;
            }
            game.tactical_end_turn();
        }

        assert!(
            game.world.get::<Emulation>(player).is_none(),
            "the emulation should have lapsed on the battle map"
        );
        assert!(
            game.has_active_battle(),
            "the fight itself must still be open — only the emulation should have lapsed"
        );
        assert!(
            log_texts(&game)
                .iter()
                .any(|t| t == "Your emulation lapses."),
            "the lapse must be logged on a battle map too"
        );
    }

    #[test]
    fn finish_fight_removes_emulation_after_a_win() {
        let mut game = game();
        let player = game.player_entity();
        let def = drone(&game);
        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 5,
        });
        let hostile = overwhelmed_hostile(&mut game, 1, 0);
        insert_battle(&mut game, player, vec![hostile]);

        force_the_next_attack_to_land(&mut game);
        player_attacks(&mut game);

        assert!(
            !game.has_active_battle(),
            "the fixture should have finished the fight in one round"
        );
        assert!(
            game.world.get::<Emulation>(player).is_none(),
            "a won fight must clear the emulation"
        );
    }

    #[test]
    fn finish_fight_removes_emulation_after_a_loss() {
        let mut game = game();
        let player = game.player_entity();
        let def = drone(&game);
        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 5,
        });
        let hostile = overwhelmed_hostile(&mut game, 100_000, 100_000);
        insert_battle(&mut game, player, vec![hostile]);

        for _ in 0..64 {
            if !game.has_active_battle() {
                break;
            }
            game.battle_set_action(0, BattleAction::Attack { group: 0 })
                .unwrap();
            game.battle_resolve_round();
        }

        assert!(
            !game.has_active_battle(),
            "64 rounds against an overwhelming hostile and the fight is still open"
        );
        assert!(
            game.world.get::<Emulation>(player).is_none(),
            "a lost fight must clear the emulation"
        );
    }

    #[test]
    fn finish_fight_removes_emulation_after_a_jack_out() {
        let mut game = game();
        let player = game.player_entity();
        let def = drone(&game);
        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 5,
        });
        let hostile = overwhelmed_hostile(&mut game, 100_000, 1);
        insert_battle(&mut game, player, vec![hostile]);

        flee_until_clear(&mut game);

        assert!(!game.has_active_battle());
        assert!(
            game.world.get::<Emulation>(player).is_none(),
            "a jack-out must clear the emulation"
        );
    }
}
