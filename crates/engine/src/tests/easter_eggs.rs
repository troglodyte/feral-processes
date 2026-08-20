//! The easter-egg census, engine half.
//!
//! Four keys in the game do something real and are named by nothing on
//! screen: `W`, `Z` and `T` twice — see `crates/engine/EASTER_EGGS.md`.
//! An omission is invisible, so these are the assertions that hold it.
//!
//! The other half is `tests::assets::no_shipped_help_page_names_a_hidden_key`,
//! over `assets/help/` — the manual is the one thing in the game that lists
//! key bindings.

use super::support::*;
use crate::*;

fn game() -> Game {
    Game::new(404, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// Every battle key a player is shown comes out of these two lists — both
/// renderers build the round prompt from them rather than hardcoding
/// letters, which is what stops the two views drifting apart. It is also
/// what would print a hidden key on screen the day a new action claimed
/// one.
///
/// Nothing claims `W`, `T` or `Z` today. This is not a red-green test; it
/// is the guard that fails if that changes.
#[test]
fn no_battle_action_or_party_command_claims_a_hidden_key() {
    let mut game = game();
    let companion = spawn_tamed(&mut game, 20, 3);
    game.world.resource_mut::<Party>().0.push(companion);
    let player = game.player_entity();
    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);

    let mut claimed: Vec<char> = game
        .battle_party_commands()
        .into_iter()
        .map(|c| c.key)
        .collect();
    for slot in 0..2 {
        claimed.extend(game.battle_action_options(slot).into_iter().map(|o| o.key));
    }
    assert!(
        !claimed.is_empty(),
        "the fixture found no battle keys at all, so it proves nothing"
    );

    for key in claimed {
        assert!(
            !matches!(key, 'W' | 'T' | 'Z'),
            "a battle key claimed {key:?}, which both renderers will now print \
             in the prompt — see crates/engine/EASTER_EGGS.md"
        );
    }
}
