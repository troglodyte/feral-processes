//! The player's chosen class.
//!
//! Phase 2A of the character-creation feature (`assets/classes/*.ron`,
//! `ClassDb`) is what makes `class` matter — an affinity spread and a
//! per-class starting kit. Until that lands, `apply_kit` is
//! `Game::apply_character_choice`'s "kit" step for every class, including
//! `None`, which is what makes `CharacterChoice::default()` still produce
//! today's hardcoded four-item kit.

use crate::Game;
use crate::components::Inventory;
use crate::items::ids;
use crate::species::AffinityClass;

/// Stocks the player's starting `Inventory`. The four items and quantities
/// are `Game::new`'s own hardcoded kit, moved here rather than duplicated —
/// see the module doc comment for why `class` goes unread.
pub fn apply_kit(game: &mut Game, _class: Option<AffinityClass>) {
    let player = game.player_entity();
    let mut inventory = game.world.get_mut::<Inventory>(player).unwrap();
    inventory.add(ids::ICE_BREAKER.into(), 3);
    inventory.add(ids::POWER_CELL.into(), 3);
    inventory.add(ids::CORE_FRAGMENT.into(), 5);
    inventory.add(ids::OUTLET.into(), 2);
}
