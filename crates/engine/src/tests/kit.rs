//! `Game::kit_of`: the one answer to where a body's kit comes from.
//!
//! The player-side tests pin what the four readers answered for the player
//! before they became matches on `Kit`, so converting them has a witness.

use super::support::*;
use super::tactical::{body, log_texts, tactical_fight, wait_for_turn, western_edge};
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

    /// Two distinct shipped species, id-ordered so the pair is stable
    /// across a run — for tests that need to tell "this image" from
    /// "the other one" rather than caring which is which.
    fn two_species(game: &Game) -> (crate::species::SpeciesDef, crate::species::SpeciesDef) {
        let mut all = game.species_defs();
        all.sort_by(|a, b| a.id.cmp(&b.id));
        assert!(
            all.len() >= 2,
            "the fixture needs at least two shipped species"
        );
        (all[0].clone(), all[1].clone())
    }

    /// The ordinary hunger drain one round's own `tick()` costs the player
    /// regardless of what action it spent — `systems::needs_tick_system`,
    /// unrelated to routines or Power charges. A "costs no Power" assertion
    /// has to subtract this or it is really asserting "costs no Power
    /// *and* nobody ever gets hungry", which is false of every action.
    fn ambient_hunger_drain(game: &Game, player: Entity) -> f32 {
        crate::systems::power_drain_per_tick(crate::perks::power_drain_multiplier(
            game.world.get::<Perks>(player),
        ))
    }

    /// The index of the (already-installed) Emulate ability in the
    /// player's own Special menu.
    fn emulate_index(game: &Game) -> usize {
        game.battle_special_options(0)
            .into_iter()
            .find(|o| o.name == "Emulate")
            .expect("Emulate must be offered")
            .index
    }

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

    /// Final review F5 (U2): a bought stat stays yours while you're wearing
    /// someone else's kit — `components::BoughtStats` is the receipt
    /// `Perk::Attacker`/`Perk::Defender` and a talent `Stat` node write, and
    /// `worn_gear_still_adds_atk_and_mitigation_while_emulating`'s sibling.
    #[test]
    fn bought_attacker_levels_still_add_atk_while_emulating() {
        let mut game = game();
        let player = game.player_entity();
        let def = drone(&game);
        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 3,
        });
        let atk_before = game.effective_atk(player);

        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        game.unlock_perk(Perk::Attacker).unwrap();

        assert_eq!(
            game.effective_atk(player),
            atk_before + crate::tuning::ATTACKER_BONUS_PER_LEVEL,
            "a bought Attacker level must land on top of the emulated base"
        );
    }

    /// U2's other axis, `Perk::Defender`'s own `BoughtStats::mitigation`.
    #[test]
    fn bought_defender_levels_still_add_mitigation_while_emulating() {
        let mut game = game();
        let player = game.player_entity();
        let def = drone(&game);
        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 3,
        });
        let mitigation_before = game.effective_mitigation(player);

        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        game.unlock_perk(Perk::Defender).unwrap();

        assert_eq!(
            game.effective_mitigation(player),
            mitigation_before + crate::tuning::DEFENDER_BONUS_PER_LEVEL,
            "a bought Defender level must land on top of the emulated base"
        );
    }

    /// The creation stat pool is deliberately excluded: `apply_creation_
    /// stats` bakes it straight into `Stats` at character creation and
    /// never writes `BoughtStats`, so there is no receipt for
    /// `emulated_base` to read — U2 says "if it is receipted there", and
    /// verifying that it is not is this test's whole job.
    #[test]
    fn the_creation_stat_pool_writes_no_bought_stats_receipt() {
        let choice = crate::CharacterChoice {
            stats: [1, 0, 0, 0],
            ..crate::CharacterChoice::default()
        };
        let game = Game::new_with(4, DifficultyMode::Forgiving, &test_assets_dir(), &choice)
            .expect("a 1-point Atk spend is affordable at CREATION_STAT_POINTS");
        let player = game.player_entity();
        assert!(
            game.world.get::<Stats>(player).unwrap().atk > crate::tuning::PLAYER_BASE_STATS.atk,
            "test premise: the creation spend must have actually raised Stats::atk"
        );
        assert_eq!(
            game.world
                .get::<BoughtStats>(player)
                .copied()
                .unwrap_or_default()
                .atk,
            0,
            "the creation stat pool must not write a BoughtStats receipt"
        );
    }

    /// The preview and the fight must read the identical figure once a
    /// bought stat and worn gear are both in play — F5's whole point.
    #[test]
    fn emulation_options_preview_equals_the_fight_figure_with_gear_and_bought_stats() {
        let mut game = game();
        let player = game.player_entity();
        let def = drone(&game);
        game.world
            .resource_mut::<crate::resources::EmulationImages>()
            .0
            .insert(def.id.clone());
        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        game.unlock_perk(Perk::Attacker).unwrap();
        game.unlock_perk(Perk::Defender).unwrap();
        equip_weapon(&mut game, player, "shim_blade");
        equip_armor(&mut game, player, "ablative_plating");

        let option = game
            .emulation_options()
            .into_iter()
            .find(|o| o.species == def.id)
            .expect("the learned image appears in the options");

        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 3,
        });

        assert_eq!(
            option.atk,
            game.effective_atk(player),
            "the preview's atk must equal what the fight actually uses"
        );
        assert_eq!(
            option.mitigation,
            game.effective_mitigation(player),
            "the preview's mitigation must equal what the fight actually uses"
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
        // F6 (U3): Decompile rides along beside the capped species list and
        // does not itself count against `slots` — a fresh `game()` player
        // always carries it in slot 0 (`spawn_player`).
        assert_eq!(game.actor_abilities(player).len(), slots + 1);
        assert!(
            game.actor_abilities(player)
                .iter()
                .any(|a| a.id == "decompile"),
            "Decompile must still be offered while emulating"
        );
    }

    /// Final review F6 (U3): the player's welded Decompile routine stays on
    /// the list while emulating, beside the species' own routines — it
    /// never left slot 0, so the species list (which replaces the *rest* of
    /// the kit) does not evict it.
    #[test]
    fn actor_abilities_lists_decompile_beside_the_species_list_while_emulating() {
        let mut game = game();
        let player = game.player_entity();
        assert!(
            game.world
                .get::<Routines>(player)
                .unwrap()
                .0
                .iter()
                .any(|id| id == "decompile"),
            "test premise: a fresh player always carries decompile"
        );
        let def = drone(&game);
        game.world.get_mut::<Experience>(player).unwrap().level = 10;
        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 3,
        });

        let ids: Vec<AbilityId> = game
            .actor_abilities(player)
            .iter()
            .map(|a| a.id.clone())
            .collect();

        assert!(
            ids.contains(&"decompile".to_string()),
            "decompile must still be offered while emulating: {ids:?}"
        );
        assert!(
            ids.contains(&"skim_group".to_string()),
            "the species list must still be offered alongside it: {ids:?}"
        );
    }

    /// A capture is legal in the group model while emulating — U3's "capture
    /// works while emulating in both models."
    #[test]
    fn a_capture_succeeds_in_the_group_model_while_emulating() {
        let mut game = game();
        let player = game.player_entity();
        let def = drone(&game);
        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 50,
        });
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
                WanderAi::default(),
                Position { x: 3, y: 3 },
                Stats {
                    hp: 1,
                    max_hp: 10,
                    atk: 1,
                    mitigation: 1,
                },
                StatusEffects::default(),
            ))
            .id();
        insert_battle(&mut game, player, vec![wild]);
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::ICE_BREAKER), 50);
        game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;

        for _ in 0..50 {
            if game.world.get::<Tamed>(wild).is_some() {
                break;
            }
            if game.world.get::<Emulation>(player).is_none() {
                // A lapsed emulation would make this a test of the ordinary
                // kit instead — re-arm it rather than let that happen
                // silently.
                game.world.entity_mut(player).insert(Emulation {
                    species: def.id.clone(),
                    rounds_left: 50,
                });
            }
            player_decompiles(&mut game);
        }

        assert!(
            game.world.get::<Tamed>(wild).is_some(),
            "the wild program should have been captured while the player was emulating"
        );
    }

    /// The tactical model's own half of U3's "capture works while
    /// emulating in both models" — `decompile_body`, reached through a cell
    /// rather than a group index, the same walk-into-reach loop
    /// `a_capture_on_a_battle_map_turns_the_program_it_was_aimed_at`
    /// (`tests::tactical`) uses.
    #[test]
    fn a_capture_succeeds_on_a_battle_map_while_emulating() {
        let mut game = game();
        let player = game.player_entity();
        let def = drone(&game);
        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 50,
        });
        let pack = tactical_fight(&mut game, 1, 40);
        game.world.get_mut::<Stats>(pack[0]).unwrap().hp = 1;
        game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;
        crate::tests::support::set_inventory(&mut game, &[(ids::ICE_BREAKER, 50)]);

        for _ in 0..50 {
            if game.world.get::<Tamed>(pack[0]).is_some() {
                break;
            }
            if game.world.get::<Emulation>(player).is_none() {
                // A lapsed emulation would make this a test of the
                // ordinary kit instead — re-arm it rather than let that
                // happen silently.
                game.world.entity_mut(player).insert(Emulation {
                    species: def.id.clone(),
                    rounds_left: 50,
                });
            }
            if !wait_for_turn(&mut game, player) {
                break;
            }
            let at = game
                .world
                .resource::<crate::tactical::TacticalBattle>()
                .cell_of(pack[0])
                .expect("the target left the board");
            while game
                .world
                .resource::<crate::tactical::TacticalBattle>()
                .cell_of(player)
                .is_some_and(|from| crate::tactical::reach::distance(from, at) > 1)
            {
                let from = game
                    .world
                    .resource::<crate::tactical::TacticalBattle>()
                    .cell_of(player)
                    .unwrap();
                let dir = ((at.0 - from.0).signum(), (at.1 - from.1).signum());
                if game.tactical_step(dir) != crate::tactical::turn::StepOutcome::Moved {
                    break;
                }
            }
            let index = game
                .actor_abilities(player)
                .iter()
                .position(|a| a.id == "decompile")
                .expect("decompile must still be offered while emulating");
            if !game.tactical_use_routine(index, at) {
                game.tactical_end_turn();
            }
        }

        assert!(
            game.world.get::<Tamed>(pack[0]).is_some(),
            "the wild program should have been captured on a battle map while emulating"
        );
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

    /// Spec §4 "Invoking": `emulation_options` reads `emulated_stats` as a
    /// call rather than a copy, so this row and what invoking it installs
    /// can never disagree.
    #[test]
    fn emulation_options_rows_carry_emulated_stats_figures() {
        let mut game = game();
        let player = game.player_entity();
        game.world.get_mut::<Experience>(player).unwrap().level = 8;
        let def = drone(&game);
        game.world
            .resource_mut::<crate::resources::EmulationImages>()
            .0
            .insert(def.id.clone());

        let options = game.emulation_options();
        let row = options
            .iter()
            .find(|o| o.species == def.id)
            .expect("the learned image appears in the options");

        let level = game.ability_user_level(player);
        let fidelity = emulation_fidelity_level(game.world.get::<Perks>(player));
        let expected = emulated_stats(&def, level, fidelity);
        assert_eq!(row.atk, expected.atk);
        assert_eq!(row.mitigation, expected.mitigation);
        assert_eq!(row.name, def.name);
        assert_eq!(row.glyph, def.glyph);
    }

    #[test]
    fn emulation_options_are_sorted_by_name() {
        let mut game = game();
        let (a, b) = two_species(&game);
        {
            let mut images = game
                .world
                .resource_mut::<crate::resources::EmulationImages>();
            images.0.insert(a.id.clone());
            images.0.insert(b.id.clone());
        }

        let names: Vec<String> = game
            .emulation_options()
            .into_iter()
            .map(|o| o.name)
            .collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "emulation_options must be sorted by name");
    }

    #[test]
    fn emulate_is_refused_with_no_images_known() {
        let mut game = game();
        let player = game.player_entity();
        install_routine_for_test(&mut game, player, "emulate");
        let ability = game
            .world
            .resource::<crate::abilities::AbilityDb>()
            .get("emulate")
            .unwrap()
            .clone();
        assert_eq!(
            game.ability_unavailable(player, &ability),
            Some("no images known".to_string())
        );
    }

    #[test]
    fn emulate_is_refused_while_already_emulating() {
        let mut game = game();
        let player = game.player_entity();
        install_routine_for_test(&mut game, player, "emulate");
        let def = drone(&game);
        game.world
            .resource_mut::<crate::resources::EmulationImages>()
            .0
            .insert(def.id.clone());
        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 3,
        });
        let ability = game
            .world
            .resource::<crate::abilities::AbilityDb>()
            .get("emulate")
            .unwrap()
            .clone();
        assert_eq!(
            game.ability_unavailable(player, &ability),
            Some("already emulating".to_string())
        );
    }

    #[test]
    fn invoking_emulate_in_the_group_model_inserts_the_component() {
        let mut game = game();
        let player = game.player_entity();
        install_routine_for_test(&mut game, player, "emulate");
        let def = drone(&game);
        game.world
            .resource_mut::<crate::resources::EmulationImages>()
            .0
            .insert(def.id.clone());
        let hostile = overwhelmed_hostile(&mut game, 100_000, 0);
        insert_battle(&mut game, player, vec![hostile]);

        let index = emulate_index(&game);
        resolve_round_with(
            &mut game,
            BattleAction::Special {
                ability: index,
                target: battle::SpecialTarget::WholeParty,
                image: Some(def.id.clone()),
            },
        );

        match game.world.get::<Emulation>(player) {
            Some(emulation) => assert_eq!(emulation.species, def.id),
            None => panic!("expected the player to be emulating"),
        }
        assert!(
            log_texts(&game)
                .iter()
                .any(|t| t == &format!("You emulate a {}.", def.name)),
            "invoking must log the image's name"
        );
    }

    /// Decision 8's own trap: a pending image must never survive past the
    /// invocation that set it. Two invocations of different images, with a
    /// Revert between them, must each install exactly their own — not the
    /// other's.
    #[test]
    fn each_invocation_installs_its_own_image_never_a_stale_one() {
        let mut game = game();
        let player = game.player_entity();
        install_routine_for_test(&mut game, player, "emulate");
        let (a, b) = two_species(&game);
        {
            let mut images = game
                .world
                .resource_mut::<crate::resources::EmulationImages>();
            images.0.insert(a.id.clone());
            images.0.insert(b.id.clone());
        }
        let hostile = overwhelmed_hostile(&mut game, 100_000, 0);
        insert_battle(&mut game, player, vec![hostile]);

        let index_a = emulate_index(&game);
        resolve_round_with(
            &mut game,
            BattleAction::Special {
                ability: index_a,
                target: battle::SpecialTarget::WholeParty,
                image: Some(a.id.clone()),
            },
        );
        assert_eq!(
            game.world
                .get::<Emulation>(player)
                .map(|e| e.species.clone()),
            Some(a.id.clone())
        );

        resolve_round_with(&mut game, BattleAction::Revert);
        assert!(game.world.get::<Emulation>(player).is_none());
        // Emulate's own cooldown (4 rounds) is a separate refusal from
        // decision 8's pending-image concern this test is isolating —
        // cleared here so the second invocation reaches `use_ability` at
        // all rather than being refused by `battle_set_action` first.
        game.world.entity_mut(player).remove::<AbilityCooldowns>();

        let index_b = emulate_index(&game);
        resolve_round_with(
            &mut game,
            BattleAction::Special {
                ability: index_b,
                target: battle::SpecialTarget::WholeParty,
                image: Some(b.id.clone()),
            },
        );
        assert_eq!(
            game.world
                .get::<Emulation>(player)
                .map(|e| e.species.clone()),
            Some(b.id.clone()),
            "the second invocation must install its own image, never a stale one \
             left over from the first"
        );
    }

    #[test]
    fn revert_is_offered_only_while_emulating_in_the_group_model() {
        let mut game = game();
        let player = game.player_entity();
        let hostile = overwhelmed_hostile(&mut game, 100_000, 0);
        insert_battle(&mut game, player, vec![hostile]);

        assert!(
            !game
                .battle_action_options(0)
                .iter()
                .any(|o| o.kind == battle::ActionKind::Revert),
            "Revert must not be offered while not emulating"
        );

        let def = drone(&game);
        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 3,
        });
        assert!(
            game.battle_action_options(0)
                .iter()
                .any(|o| o.kind == battle::ActionKind::Revert),
            "Revert must be offered while emulating"
        );
    }

    #[test]
    fn revert_removes_emulation_and_costs_no_power_in_the_group_model() {
        let mut game = game();
        let player = game.player_entity();
        let def = drone(&game);
        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 5,
        });
        let hostile = overwhelmed_hostile(&mut game, 100_000, 0);
        insert_battle(&mut game, player, vec![hostile]);
        let power_before = game.world.get::<PowerReserve>(player).unwrap().get();

        let drain = ambient_hunger_drain(&game, player);
        resolve_round_with(&mut game, BattleAction::Revert);

        assert!(game.world.get::<Emulation>(player).is_none());
        assert_eq!(
            game.world.get::<PowerReserve>(player).unwrap().get(),
            power_before - drain,
            "Revert must cost no Power beyond the round's ordinary hunger drain"
        );
        assert!(
            log_texts(&game)
                .iter()
                .any(|t| t == "You drop the emulation."),
            "Revert must log the line spec §4 gives it"
        );
    }

    #[test]
    fn tactical_use_routine_refuses_emulate_since_it_has_no_aim() {
        let mut game = game();
        let player = game.player_entity();
        install_routine_for_test(&mut game, player, "emulate");
        let def = drone(&game);
        game.world
            .resource_mut::<crate::resources::EmulationImages>()
            .0
            .insert(def.id.clone());
        tactical_fight(&mut game, 1, 100_000);
        assert!(wait_for_turn(&mut game, player));

        let index = game
            .actor_abilities(player)
            .iter()
            .position(|a| a.id == "emulate")
            .expect("emulate is installed");
        let cell = game
            .world
            .resource::<crate::tactical::TacticalBattle>()
            .cell_of(player)
            .unwrap();

        assert!(!game.tactical_use_routine(index, cell));
        assert!(game.world.get::<Emulation>(player).is_none());
    }

    #[test]
    fn tactical_emulate_inserts_the_component() {
        let mut game = game();
        let player = game.player_entity();
        install_routine_for_test(&mut game, player, "emulate");
        let def = drone(&game);
        game.world
            .resource_mut::<crate::resources::EmulationImages>()
            .0
            .insert(def.id.clone());
        tactical_fight(&mut game, 1, 100_000);
        assert!(wait_for_turn(&mut game, player));

        let index = game
            .actor_abilities(player)
            .iter()
            .position(|a| a.id == "emulate")
            .expect("emulate is installed");

        assert!(game.tactical_emulate(index, &def.id));
        match game.world.get::<Emulation>(player) {
            Some(emulation) => assert_eq!(emulation.species, def.id),
            None => panic!("expected the player to be emulating"),
        }
    }

    #[test]
    fn tactical_emulate_is_refused_with_an_unknown_image() {
        let mut game = game();
        let player = game.player_entity();
        install_routine_for_test(&mut game, player, "emulate");
        let def = drone(&game);
        // Deliberately not inserted into `EmulationImages`.
        tactical_fight(&mut game, 1, 100_000);
        assert!(wait_for_turn(&mut game, player));

        let index = game
            .actor_abilities(player)
            .iter()
            .position(|a| a.id == "emulate")
            .expect("emulate is installed");

        assert!(!game.tactical_emulate(index, &def.id));
        assert!(game.world.get::<Emulation>(player).is_none());
    }

    #[test]
    fn tactical_revert_is_refused_while_not_emulating() {
        let mut game = game();
        let player = game.player_entity();
        tactical_fight(&mut game, 1, 100_000);
        assert!(wait_for_turn(&mut game, player));

        assert!(!game.tactical_revert());
    }

    #[test]
    fn tactical_revert_removes_emulation_and_costs_no_power() {
        let mut game = game();
        let player = game.player_entity();
        let def = drone(&game);
        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 5,
        });
        tactical_fight(&mut game, 1, 100_000);
        assert!(wait_for_turn(&mut game, player));
        let power_before = game.world.get::<PowerReserve>(player).unwrap().get();
        let drain = ambient_hunger_drain(&game, player);

        assert!(game.tactical_revert());

        assert!(game.world.get::<Emulation>(player).is_none());
        assert_eq!(
            game.world.get::<PowerReserve>(player).unwrap().get(),
            power_before - drain,
            "tactical_revert must cost no Power beyond the round's ordinary hunger drain"
        );
        assert!(
            log_texts(&game)
                .iter()
                .any(|t| t == "You drop the emulation."),
            "tactical_revert must log the same line the group model does"
        );
    }

    /// Seats `hostile` on an orthogonal neighbour of `player`'s current
    /// cell — `deploy::plan` does not seat a freshly opened fight's two
    /// sides adjacent to each other, so a reaction test has to move one of
    /// them there itself (`tests::reactions::face_off`'s own reason).
    fn seat_adjacent(game: &mut crate::Game, player: Entity, hostile: Entity) {
        let neighbour = {
            let battle = game.world.resource::<crate::tactical::TacticalBattle>();
            let at = battle.cell_of(player).expect("the player stands somewhere");
            [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .into_iter()
                .map(|(dx, dy)| (at.0 + dx, at.1 + dy))
                .find(|&cell| {
                    battle.board.walkable(cell.0, cell.1) && battle.occupant(cell).is_none()
                })
                .expect("the player's fixture cell has no free orthogonal neighbour")
        };
        assert!(
            game.world
                .resource_mut::<crate::tactical::TacticalBattle>()
                .move_to(hostile, neighbour),
            "seating the hostile beside the player failed"
        );
    }

    /// The fix for review round 1's finding: `run_tactical_routine`'s
    /// reactor/provoke check is not exempted for Emulate (only `Decompile`
    /// is), so an adjacent hostile's reaction can fizzle the invocation
    /// before `use_ability` — and its own clear — is ever reached.
    /// `Game::tactical_emulate` must clear `PendingEmulateImage`
    /// unconditionally on its way out, not only rely on `use_ability`'s.
    #[test]
    fn a_reaction_fizzle_clears_the_pending_image() {
        let mut game = game();
        let player = game.player_entity();
        let pack = tactical_fight(&mut game, 1, 4000);
        let hostile = pack[0];
        seat_adjacent(&mut game, player, hostile);
        // Overwhelming, `tests::reactions::a_fizzled_routine_keeps_its_
        // price_and_lands_nothing`'s own number — the fixture the fizzle
        // path is proven reachable with, at the same seed.
        game.world.get_mut::<Stats>(hostile).unwrap().atk = 500;
        game.world
            .entity_mut(player)
            .insert(Routines(vec!["emulate".to_string()]));
        let def = drone(&game);
        game.world
            .resource_mut::<crate::resources::EmulationImages>()
            .0
            .insert(def.id.clone());
        assert!(wait_for_turn(&mut game, player));

        for _ in 0..24 {
            // A body already emulating from a prior, non-fizzled iteration
            // has nothing to invoke — start every attempt from the same
            // state the fixture needs.
            game.world.entity_mut(player).remove::<Emulation>();
            game.world.entity_mut(player).remove::<AbilityCooldowns>();
            game.world.get_mut::<Stats>(player).unwrap().hp = 1;
            game.world.get_mut::<PowerReserve>(player).unwrap().fill();

            let index = game
                .actor_abilities(player)
                .iter()
                .position(|a| a.id == "emulate")
                .expect("emulate is installed");
            let before = game.message_history(usize::MAX).len();

            assert!(game.tactical_emulate(index, &def.id));

            let cut_off = game
                .message_history(usize::MAX)
                .into_iter()
                .skip(before)
                .any(|line| line.text.contains("is cut off"));
            if cut_off {
                assert!(
                    game.world
                        .resource::<crate::resources::PendingEmulateImage>()
                        .0
                        .is_none(),
                    "a fizzled Emulate invocation must not leave a stale \
                     pending image behind"
                );
                assert!(
                    game.world.get::<Emulation>(player).is_none(),
                    "a fizzled invocation must not have installed the image either"
                );
                return;
            }
            assert!(
                game.world
                    .get_resource::<crate::tactical::TacticalBattle>()
                    .is_some(),
                "the fight closed without the invocation ever being cut off"
            );
            game.tactical_end_turn();
            assert!(wait_for_turn(&mut game, player));
        }
        panic!("no reaction cut Emulate off in 24 rounds against 500 Attack");
    }

    /// Final review F1 (Critical): a companion could emulate and it
    /// outlived the fight. `ability_unavailable` is the one door every
    /// chooser and invocation site shares — see `seam:only-the-player-
    /// emulates`.
    mod only_the_player_emulates {
        use super::*;

        fn emulate_def(game: &Game) -> crate::abilities::AbilityDef {
            game.world
                .resource::<crate::abilities::AbilityDb>()
                .get("emulate")
                .unwrap()
                .clone()
        }

        #[test]
        fn ability_unavailable_refuses_emulate_for_a_companion() {
            let mut game = game();
            let companion = spawn_tamed(&mut game, 30, 6);
            // A mod's talent tree or species kit is the only way a
            // companion ever carries this — `install_disk` refuses it, so
            // this is written directly rather than through that door.
            game.world
                .entity_mut(companion)
                .insert(Routines(vec!["emulate".to_string()]));
            let ability = emulate_def(&game);

            assert_eq!(
                game.ability_unavailable(companion, &ability),
                Some("only you can emulate".to_string())
            );
        }

        #[test]
        fn ability_unavailable_still_permits_emulate_for_the_player() {
            let mut game = game();
            let player = game.player_entity();
            install_routine_for_test(&mut game, player, "emulate");
            let def = drone(&game);
            game.world
                .resource_mut::<crate::resources::EmulationImages>()
                .0
                .insert(def.id.clone());
            let ability = emulate_def(&game);

            assert_eq!(game.ability_unavailable(player, &ability), None);
        }

        #[test]
        fn install_disk_refuses_the_emulate_disk_onto_a_companion() {
            let mut game = game();
            teach_routine(&mut game, "emulate");
            give_disks(&mut game, 1);
            game.etch_disk("emulate").unwrap();
            let companion = spawn_tamed(&mut game, 30, 6);

            let err = game
                .install_disk(companion, "emulate")
                .expect_err("a companion must never take the Emulate disk");
            assert_eq!(err, "Only you can emulate.");
            assert!(
                game.world
                    .get::<Routines>(companion)
                    .is_some_and(|r| !r.0.iter().any(|id| id == "emulate")),
                "the refused install must not have written the routine anyway"
            );
        }

        #[test]
        fn install_disk_still_permits_the_emulate_disk_onto_the_player() {
            let mut game = game();
            let player = game.player_entity();
            teach_routine(&mut game, "emulate");
            give_disks(&mut game, 1);
            game.etch_disk("emulate").unwrap();

            game.install_disk(player, "emulate")
                .expect("the player must still be able to install Emulate");
        }

        #[test]
        fn battle_set_action_refuses_emulate_for_a_companion_in_the_group_model() {
            let mut game = game();
            let player = game.player_entity();
            let companion = spawn_tamed(&mut game, 30, 6);
            game.world
                .entity_mut(companion)
                .insert(Routines(vec!["emulate".to_string()]));
            game.world.resource_mut::<Party>().0.push(companion);
            let def = drone(&game);
            game.world
                .resource_mut::<crate::resources::EmulationImages>()
                .0
                .insert(def.id.clone());
            let hostile = overwhelmed_hostile(&mut game, 100_000, 0);
            insert_battle(&mut game, player, vec![hostile]);

            let index = game
                .actor_abilities(companion)
                .iter()
                .position(|a| a.id == "emulate")
                .expect("the companion carries the emulate routine");
            let result = game.battle_set_action(
                1,
                BattleAction::Special {
                    ability: index,
                    target: battle::SpecialTarget::WholeParty,
                    image: Some(def.id.clone()),
                },
            );

            assert!(result.is_err(), "a companion must be refused Emulate");
            assert!(game.world.get::<Emulation>(companion).is_none());
        }

        #[test]
        fn tactical_emulate_is_refused_for_a_companion_actor() {
            let mut game = game();
            let companion = spawn_tamed(&mut game, 100_000, 6);
            game.world
                .entity_mut(companion)
                .insert(Routines(vec!["emulate".to_string()]));
            game.world.resource_mut::<Party>().0.push(companion);
            let def = drone(&game);
            game.world
                .resource_mut::<crate::resources::EmulationImages>()
                .0
                .insert(def.id.clone());
            tactical_fight(&mut game, 1, 100_000);
            assert!(wait_for_turn(&mut game, companion));

            let index = game
                .actor_abilities(companion)
                .iter()
                .position(|a| a.id == "emulate")
                .expect("the companion carries the emulate routine");

            assert!(!game.tactical_emulate(index, &def.id));
            assert!(game.world.get::<Emulation>(companion).is_none());
        }

        /// Defence in depth: even a companion that ended up carrying
        /// `Emulation` some other way (a mod, a future bug) loses it at
        /// teardown, the same as the player.
        #[test]
        fn finish_fight_clears_emulation_from_a_companion_too() {
            let mut game = game();
            let player = game.player_entity();
            let companion = spawn_tamed(&mut game, 30, 6);
            game.world.resource_mut::<Party>().0.push(companion);
            let def = drone(&game);
            game.world.entity_mut(companion).insert(Emulation {
                species: def.id.clone(),
                rounds_left: 5,
            });
            let hostile = overwhelmed_hostile(&mut game, 1, 0);
            insert_battle(&mut game, player, vec![hostile]);

            force_the_next_attack_to_land(&mut game);
            player_attacks(&mut game);

            assert!(!game.has_active_battle());
            assert!(
                game.world.get::<Emulation>(companion).is_none(),
                "a companion's stray Emulation must not survive the fight either"
            );
        }
    }

    /// F1's tactical teardown case, `finish_fight_removes_emulation_after_
    /// a_win`'s sibling on a battle map — Task 1's sweep covered a win, a
    /// loss and a jack-out in the group model, but never a battle map's own
    /// jack-out.
    #[test]
    fn finish_fight_removes_emulation_after_a_tactical_jack_out() {
        let mut game = game();
        let player = game.player_entity();
        let def = drone(&game);
        game.world.entity_mut(player).insert(Emulation {
            species: def.id.clone(),
            rounds_left: 5,
        });
        tactical_fight(&mut game, 1, 100_000);
        assert!(wait_for_turn(&mut game, player));
        let edge = western_edge(&game);
        assert!(
            game.world
                .resource_mut::<crate::tactical::TacticalBattle>()
                .move_to(player, edge)
        );

        assert_eq!(
            game.tactical_step((-1, 0)),
            crate::tactical::turn::StepOutcome::Departed
        );

        assert!(
            game.world
                .get_resource::<crate::tactical::TacticalBattle>()
                .is_none()
        );
        assert!(
            game.world.get::<Emulation>(player).is_none(),
            "a jack-out on a battle map must clear the emulation too"
        );
    }
}
