//! `Game::kit_of`: the one answer to where a body's kit comes from.
//!
//! The player-side tests pin what the four readers answered for the player
//! before they became matches on `Kit`, so converting them has a witness.

use super::support::*;
use super::tactical::body;
use crate::components::Perks;
use crate::game::kit::Kit;
use crate::perks::{Perk, emulation_fidelity_level};
use crate::progression::{emulated_stats, stats_after_levels};
use crate::tuning::{PLAYER_UNARMED_DAMAGE, TACTICAL_MELEE_RANGE};
use crate::{DifficultyMode, Game, components::Stats};

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
        Kit::Unarmed => None,
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
