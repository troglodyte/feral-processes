//! The cross-run achievement profile: what a run records, what the tick
//! decides has been earned, and what the next run gets paid for it.

use super::support::*;
use crate::resources::RunFeats;
use crate::*;

/// A hostile of `species_id` standing on the player's tile with stats set by
/// hand — a shipped boss is 200+ HP with a `growth_multiplier` of 2.0, and
/// these tests care about whether the kill was *recorded*, not about winning
/// a real boss fight.
fn spawn_boss_on_player_tile(game: &mut Game, species_id: &str, hp: i32) -> Entity {
    let at = *game.world.get::<Position>(game.player_entity()).unwrap();
    assert!(
        game.species_defs()
            .into_iter()
            .any(|s| s.id == species_id && s.is_boss),
        "{species_id} should be a shipped boss species"
    );
    game.world
        .spawn((
            Creature {
                species: species_id.to_string(),
            },
            Hostile,
            Position { x: at.x, y: at.y },
            Stats {
                hp,
                max_hp: hp,
                atk: 1,
                def: 1,
            },
        ))
        .id()
}

/// Attacks until the battle is over. Bounded rather than looping forever, so
/// a fixture that can't actually win fails loudly.
fn fight_to_the_end(game: &mut Game) {
    for _ in 0..200 {
        if !game.has_active_battle() {
            return;
        }
        player_attacks(game);
    }
    panic!("200 rounds and the fight is still going — the fixture cannot win it");
}

#[test]
fn killing_a_boss_records_its_species() {
    let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let boss = spawn_boss_on_player_tile(&mut game, "overseer", 1);
    game.start_battle(vec![boss]);
    fight_to_the_end(&mut game);

    assert_eq!(
        game.world.resource::<RunFeats>().bosses_defeated,
        vec!["overseer".to_string()]
    );
}

#[test]
fn killing_an_ordinary_program_records_nothing() {
    let mut game = Game::new(12, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);
    game.world.get_mut::<Stats>(wild).unwrap().hp = 1;
    game.start_battle(vec![wild]);
    fight_to_the_end(&mut game);

    assert!(
        game.world.resource::<RunFeats>().bosses_defeated.is_empty(),
        "only a boss is a feat"
    );
}

#[test]
fn fleeing_a_boss_records_nothing() {
    let mut game = Game::new(13, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let boss = spawn_boss_on_player_tile(&mut game, "overseer", 500);
    game.start_battle(vec![boss]);
    flee_until_clear(&mut game);

    assert!(
        game.world.resource::<RunFeats>().bosses_defeated.is_empty(),
        "the record sits where `mark_lair_cleared` does, at the one point that \
         knows the boss died rather than being fled from"
    );
}
