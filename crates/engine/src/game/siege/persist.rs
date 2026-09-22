//! Assembling and restoring a siege in progress across a save/load round
//! trip — `save::SiegeSave`'s one writer and one reader.
//!
//! **Never serialises `TacticalBattle`.** `assemble` reads the board's own
//! cells and each body's own cell and index out of it and builds a
//! `SiegeSave` from scratch; `restore` rebuilds a fresh `TacticalBattle`
//! from that record rather than deserialising one — see that type's own
//! doc for why it does not gain `Serialize` at all. Membership rides
//! `save::CreatureSave::siege_cell`/`siege_order` the way a sortie's rides
//! `sortie_index`, `game/lifecycle.rs::restore_sorties`'s reason: entity
//! ids are not stable across a save/load round trip.

use bevy_ecs::prelude::Entity;

use crate::Game;
use crate::save::SiegeSave;
use crate::tactical::TacticalBattle;
use crate::tactical::map::Board;

use super::board::{self, SiegeBoard};

/// One body's own place in the fight `battle` is — its index into
/// `TacticalBattle::initiative` and its board cell. `None` for a body not
/// seated in `battle` at all (a structure, or an entity that never joined
/// it) — `save::CreatureSave::siege_cell`/`siege_order`'s one writer, and
/// `assemble`'s own reader for the currently-acting body.
pub(crate) fn member_of(battle: &TacticalBattle, entity: Entity) -> Option<(u32, (i32, i32))> {
    let order = battle.initiative().iter().position(|&e| e == entity)? as u32;
    let cell = battle.cell_of(entity)?;
    Some((order, cell))
}

/// Assembles a save of the siege in progress, if the open `TacticalBattle`
/// (if any) is one.
///
/// `TacticalBattle::siege_pack` being `0` is that field's own answer for
/// "not a siege" — `Game::open_siege`'s doc — so an ordinary tactical
/// fight, or no fight at all, assembles to `None` exactly as
/// `save::SaveData::siege` wants it to.
pub(crate) fn assemble(game: &Game) -> Option<SiegeSave> {
    let battle = game.world.get_resource::<TacticalBattle>()?;
    if battle.siege_pack == 0 {
        return None;
    }
    let cells = battle.board.cells().map(|(_, kind)| kind).collect();
    // The currently-acting body's own order value, not a raw index — see
    // `SiegeSave::turn`'s doc for why `restore` needs it that way. A fight
    // with nobody left to act (the reap just cleared the last body and the
    // close hasn't landed yet) has no actor to name; `0` is inert then,
    // since `restore` only ever indexes into the members that did reload.
    let turn = battle
        .actor()
        .and_then(|actor| member_of(battle, actor))
        .map(|(order, _)| order)
        .unwrap_or(0);
    // The player is never one of `SaveData::creatures` — see `save::
    // SiegeSave::player_order`'s doc for why its cell needs no field here
    // but its order does.
    let player_order = member_of(battle, game.player_entity())
        .map(|(order, _)| order)
        .unwrap_or(0);
    Some(SiegeSave {
        spec: battle.spec,
        side: battle.board.side,
        cells,
        origin: battle.siege_origin,
        door: battle.siege_door,
        round: battle.round,
        turn,
        actions_left: battle.actions_left(),
        siege_pack: battle.siege_pack,
        player_order,
    })
}

/// Rebuilds the `TacticalBattle` a save carried, once every creature has
/// loaded and every structure has too.
///
/// `members` is `(the body's own `CreatureSave::siege_order`, the loaded
/// entity, its `siege_cell`)` for every body that reloaded successfully —
/// `game/lifecycle.rs::restore_sorties`'s `sortie_members` shape. **A save
/// whose members all failed to load is dropped rather than restored as an
/// empty fight nobody can end**, `restore_sorties`'s own rule: a species
/// file deleted between sessions is rare, and a fight with nobody left to
/// call it closed is worse than no fight at all.
///
/// Re-seats every structure still standing exactly as `Game::open_siege`
/// does — `board::seat_structures`, this file's other caller — since
/// `SiegeSave` carries no structure list of its own: a structure's own
/// state is already the save's source of truth.
pub(crate) fn restore(game: &mut Game, saved: SiegeSave, members: &[(u32, Entity, (i32, i32))]) {
    if members.is_empty() {
        return;
    }

    let board = Board::from_cells(saved.side, saved.cells);
    let mut battle = TacticalBattle::open(saved.spec, board);
    battle.siege_pack = saved.siege_pack;
    battle.siege_origin = saved.origin;
    battle.siege_door = saved.door;

    let siege_board = SiegeBoard {
        board: battle.board.clone(),
        origin: saved.origin,
        door: saved.door,
    };
    let structures = game.structure_footprints();
    board::seat_structures(&mut battle, &siege_board, structures);

    let mut sorted: Vec<(u32, Entity, (i32, i32))> = members.to_vec();
    // The player rides `SiegeSave::player_order` rather than `members`,
    // that field's own reason: it is never one of `SaveData::creatures`.
    // Its cell is re-derived from `PlayerSave::position` exactly as
    // `Game::open_siege` derives it the first time — the player never
    // moves on the world map while a battle is open, so that position is
    // still the cell the fight opened on.
    let player = game.player_entity();
    if let Some(pos) = game.world.get::<crate::components::Position>(player)
        && let Some(cell) = siege_board.to_board((pos.x, pos.y))
    {
        sorted.push((saved.player_order, player, cell));
    }
    sorted.sort_by_key(|&(order, _, _)| order);
    for &(_, entity, cell) in &sorted {
        battle.place(entity, cell);
    }
    // A siege the player was never seated back into is not one worth
    // resuming — `members.is_empty()`'s own reason, applied to the one
    // member that check cannot see.
    if battle.cell_of(player).is_none() {
        return;
    }
    let initiative: Vec<Entity> = sorted.iter().map(|&(_, e, _)| e).collect();

    // The saved `turn` is an *order value*, not a position in this
    // (possibly shorter) reconstructed list — `SiegeSave::turn`'s doc. The
    // exact match always succeeds when every member reloaded, which is the
    // case this feature is built and tested against; a body whose own turn
    // it was failing to reload is rare enough to fall back to the front of
    // the order rather than earn its own recovery rule.
    let turn = sorted
        .iter()
        .position(|&(order, _, _)| order == saved.turn)
        .unwrap_or(0);
    battle.resume(initiative, saved.round, turn, saved.actions_left);

    game.world.insert_resource(battle);
    game.world
        .resource_mut::<crate::resources::MessageLog>()
        .open_battle();
}
