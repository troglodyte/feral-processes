//! `T` on the battle roster — your front companion says something at the
//! wild group. Pure flavour, no turn, no round.
//!
//! The key is deliberately named by nothing on screen; see
//! `crates/engine/EASTER_EGGS.md`.

use super::support::*;
use crate::resources::MessageKind;
use crate::*;

fn game() -> Game {
    Game::new(77, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// A species that ships taunt lines, and one that ships none. Both are
/// read off the loaded set rather than named, so a content edit moves this
/// fixture instead of breaking a test that has nothing to do with it.
fn a_species_with_taunts(game: &Game) -> SpeciesDef {
    game.species_defs()
        .into_iter()
        .find(|s| s.taunts.len() >= 2)
        .expect("some shipped species should carry at least two taunt lines")
}

fn a_species_without_taunts(game: &Game) -> SpeciesDef {
    game.species_defs()
        .into_iter()
        .find(|s| s.taunts.is_empty())
        .expect("most shipped species carry none, which is the point of the fallback")
}

/// Fields one living companion of `species` and opens a fight, which is
/// the state a taunt needs: a battle, and someone in front to say it.
fn battle_with_a_companion_of(game: &mut Game, species: &SpeciesId) {
    let player = game.player_entity();
    let companion = game
        .world
        .spawn((
            Creature {
                species: species.clone(),
            },
            Position { x: 3, y: 3 },
            Stats {
                hp: 20,
                max_hp: 20,
                atk: 3,
                mitigation: 1,
            },
            Tamed { owner: player },
            Experience::default(),
            PowerReserve::default(),
        ))
        .id();
    game.world.resource_mut::<Party>().0.push(companion);
    let wild = spawn_wild_on_player_tile(game);
    insert_battle(game, player, vec![wild]);
}

/// The line the taunt just logged.
fn taunt(game: &mut Game) -> String {
    let before = game.message_log(usize::MAX).len();
    game.taunt()
        .expect("taunting in a battle should be allowed");
    let logged = game.message_log(usize::MAX);
    let line = logged.get(before).expect("taunting logged nothing");
    assert_eq!(
        line.kind,
        MessageKind::Info,
        "a taunt is Info so it is pruned when the battle ends"
    );
    line.text.clone()
}

#[test]
fn a_species_with_lines_of_its_own_speaks_one_of_them() {
    let mut game = game();
    let species = a_species_with_taunts(&game);
    battle_with_a_companion_of(&mut game, &species.id);

    let line = taunt(&mut game);

    assert!(
        species.taunts.iter().any(|t| line.contains(t.as_str())),
        "{} said something that is not one of its lines: {line}",
        species.name
    );
    assert!(
        line.contains(&species.name),
        "the line should name who said it: {line}"
    );
}

/// The key must never silently do nothing, so a species that authors no
/// lines still speaks — which is every shipped species until someone
/// writes lines for it.
#[test]
fn a_species_with_no_lines_still_says_something() {
    let mut game = game();
    let species = a_species_without_taunts(&game);
    battle_with_a_companion_of(&mut game, &species.id);

    let line = taunt(&mut game);

    assert!(
        line.len() > species.name.len() + 4,
        "the fallback produced no line at all: {line}"
    );
}

#[test]
fn taunting_twice_cycles_through_the_speakers_lines() {
    let mut game = game();
    let species = a_species_with_taunts(&game);
    battle_with_a_companion_of(&mut game, &species.id);

    let first = taunt(&mut game);
    let second = taunt(&mut game);

    assert_ne!(
        first, second,
        "a second press repeated the first line instead of cycling"
    );
}

/// The one property that makes this safe to press twenty times in a fight.
/// A `GameRng` draw for a cosmetic string shifts every later roll in the
/// run — which is how a seeded combat test was silently rewritten from
/// three files away once already.
#[test]
fn taunting_does_not_advance_the_shared_rng_stream() {
    let fight = |taunts: bool| {
        let mut game = game();
        let species = a_species_with_taunts(&game);
        battle_with_a_companion_of(&mut game, &species.id);
        let mut hp = Vec::new();
        for _ in 0..4 {
            if !game.has_active_battle() {
                break;
            }
            if taunts {
                game.taunt().unwrap();
                game.taunt().unwrap();
            }
            player_attacks(&mut game);
            hp.push(
                game.world
                    .get::<Stats>(game.player_entity())
                    .map(|s| s.hp)
                    .unwrap_or(0),
            );
        }
        hp
    };

    assert_eq!(
        fight(true),
        fight(false),
        "the same seeded fight came out differently with taunting in the middle"
    );
}

#[test]
fn taunting_outside_a_battle_is_refused() {
    let mut game = game();
    assert!(
        game.taunt().is_err(),
        "there is nobody out there to say it to"
    );
}

/// The `#[serde(default)]` obligation: every shipped species file predates
/// the field, and a mod's will too. If this fails, the whole species set
/// failed to load and every other test in the suite is already red — which
/// is precisely why it is worth stating once, here, in plain terms.
#[test]
fn a_species_file_without_a_taunts_key_still_parses() {
    let game = game();
    let defs = game.species_defs();
    assert!(
        defs.iter().any(|s| s.taunts.is_empty()),
        "no shipped species is missing the key, so nothing here proves the default"
    );
    assert!(
        defs.iter().any(|s| !s.taunts.is_empty()),
        "no shipped species carries the key, so nothing here proves it is read"
    );
}
