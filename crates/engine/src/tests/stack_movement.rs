//! The two Stack movement routines — `AbilityEffect::Phase` (Buffer Overrun)
//! and `AbilityEffect::Jump` (Wild Jump). See `game/stack_movement.rs`.

use super::support::*;
use crate::components::{PowerReserve, Routines};
use crate::resources::{CurrentStack, Locale, StackMemory, Trace};
use crate::stack::{CellKind, Dir};
use crate::tuning::{TRACE_PER_JUMP, TRACE_PER_PHASE};
use crate::*;

const PHASE: &str = "buffer_overrun";
const JUMP: &str = "wild_jump";

/// A game whose player carries both movement routines, standing on depth 1
/// of the stack under their own tile.
fn underground() -> Game {
    underground_on(DifficultyMode::Forgiving)
}

fn underground_on(mode: DifficultyMode) -> Game {
    let mut game = surfaced_with_routines_on(mode);
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.enter_stack(pos.x, pos.y);
    game
}

/// The same, but still on open grid — for the Stack-only refusals.
fn surfaced_with_routines() -> Game {
    surfaced_with_routines_on(DifficultyMode::Forgiving)
}

fn surfaced_with_routines_on(mode: DifficultyMode) -> Game {
    let mut game = Game::new(16, mode, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .entity_mut(player)
        .insert(Routines(vec![PHASE.to_string(), JUMP.to_string()]));
    // Full Power, so an unrelated drain can never be what refuses a cast
    // these tests expect to run.
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(100.0);
    game
}

/// Index of `id` in the field-routine picker — the number
/// `cast_field_routine` actually takes.
fn row(game: &mut Game, id: &str) -> usize {
    game.field_routines()
        .iter()
        .position(|r| r.ability == id)
        .unwrap_or_else(|| panic!("{id} is not in the field-routine list"))
}

fn cast(game: &mut Game, id: &str, pick: FieldCastTarget) -> Result<(), String> {
    let index = row(game, id);
    game.cast_field_routine(index, pick)
}

fn frame(game: &Game) -> crate::stack::Frame {
    game.world
        .resource::<CurrentStack>()
        .0
        .clone()
        .expect("underground fixtures have a frame")
}

fn at(game: &Game) -> (i32, i32) {
    match game.locale() {
        Locale::Stack { x, y, .. } => (x, y),
        Locale::Surface => panic!("expected to be underground"),
    }
}

fn facing(game: &Game) -> Dir {
    match game.locale() {
        Locale::Stack { facing, .. } => facing,
        Locale::Surface => panic!("expected to be underground"),
    }
}

fn power(game: &Game) -> f32 {
    game.world
        .get::<PowerReserve>(game.player_entity())
        .unwrap()
        .get()
}

fn trace(game: &Game) -> u32 {
    game.world.resource::<Trace>().0
}

/// Stands the party on `cell` facing `dir`, without walking there. The one
/// thing every test below needs and the game itself has no way to do — a
/// fixture that walked to each spot would spend its length on the maze.
fn stand(game: &mut Game, cell: (i32, i32), dir: Dir) {
    let Locale::Stack {
        depth,
        frames,
        entrance,
        ..
    } = game.locale()
    else {
        panic!("stand() is for an underground fixture");
    };
    game.world.insert_resource(Locale::Stack {
        depth,
        frames,
        x: cell.0,
        y: cell.1,
        facing: dir,
        entrance,
    });
}

const DIRS: [Dir; 4] = [Dir::North, Dir::East, Dir::South, Dir::West];

/// Where to stand, which way to face, and where a phase from there lands —
/// what `wall_site` picks out of a generated frame.
type WallSite = ((i32, i32), Dir, (i32, i32));

/// Every cell of the frame, in a fixed order, so a search over it is
/// reproducible.
fn cells(level: &crate::stack::Frame) -> Vec<(i32, i32)> {
    (0..level.height)
        .flat_map(|y| (0..level.width).map(move |x| (x, y)))
        .collect()
}

/// The first `(stand-on, facing)` in the frame whose cell ahead is solid and
/// whose cell two ahead satisfies `beyond`.
///
/// Searched rather than hardcoded: which cells of a generated maze happen to
/// have a one-thick wall is a property of the seed, and a hardcoded pair
/// would silently stop testing what it names the moment anything shifted the
/// generator's stream.
fn wall_site(
    level: &crate::stack::Frame,
    beyond: impl Fn(&crate::stack::Frame, (i32, i32)) -> bool,
) -> Option<WallSite> {
    for cell in cells(level) {
        if !level.walkable(cell.0, cell.1) {
            continue;
        }
        for dir in DIRS {
            let (dx, dy) = dir.delta();
            let wall = (cell.0 + dx, cell.1 + dy);
            let landing = (cell.0 + dx * 2, cell.1 + dy * 2);
            if !level.walkable(wall.0, wall.1) && beyond(level, landing) {
                return Some((cell, dir, landing));
            }
        }
    }
    None
}

fn in_bounds(level: &crate::stack::Frame, (x, y): (i32, i32)) -> bool {
    x >= 0 && y >= 0 && x < level.width && y < level.height
}

#[test]
fn phasing_crosses_one_wall_and_lands_on_the_open_cell_beyond() {
    let mut game = underground();
    let level = frame(&game);
    let (from, dir, landing) =
        wall_site(&level, |l, c| l.walkable(c.0, c.1)).expect("a one-thick wall somewhere");
    stand(&mut game, from, dir);

    cast(&mut game, PHASE, FieldCastTarget::None).expect("a one-thick wall is what Phase crosses");

    assert_eq!(at(&game), landing, "the party did not land beyond the wall");
    assert_eq!(
        facing(&game),
        dir,
        "phasing must leave the party facing the way they went in"
    );
}

#[test]
fn phasing_is_refused_by_two_deep_rock() {
    let mut game = underground();
    let level = frame(&game);
    let (from, dir, landing) = wall_site(&level, |l, c| in_bounds(l, c) && !l.walkable(c.0, c.1))
        .expect("two-deep rock somewhere in a 21x21 maze");
    stand(&mut game, from, dir);

    let refused = cast(&mut game, PHASE, FieldCastTarget::None);

    assert!(
        refused.is_err(),
        "phasing into rock at {landing:?} must be refused, not survived"
    );
    assert_eq!(at(&game), from, "a refused phase moved the party anyway");
}

/// The frame is bordered in solid rock, so a cell one in from the edge
/// facing out has a wall ahead and the void beyond it. That reads back as
/// `CellKind::Rock` like any other out-of-bounds cell, which is why the
/// bounds check has to come first — "the rock runs deeper" is true of the
/// void and useless to a player standing at the edge of the world.
#[test]
fn phasing_off_the_edge_of_the_frame_is_refused() {
    let mut game = underground();
    let level = frame(&game);
    let from = cells(&level)
        .into_iter()
        .find(|&(x, y)| x == 1 && level.walkable(x, y))
        .expect("a walkable cell one in from the west border");
    stand(&mut game, from, Dir::West);

    let refused = cast(&mut game, PHASE, FieldCastTarget::None);

    assert!(refused.is_err(), "phasing out of the frame must be refused");
    assert_eq!(at(&game), from);
}

#[test]
fn phasing_with_nothing_solid_ahead_is_refused() {
    let mut game = underground();
    let level = frame(&game);
    let (from, dir) = cells(&level)
        .into_iter()
        .filter(|&(x, y)| level.walkable(x, y))
        .find_map(|cell| {
            DIRS.into_iter().find_map(|dir| {
                let (dx, dy) = dir.delta();
                level
                    .walkable(cell.0 + dx, cell.1 + dy)
                    .then_some((cell, dir))
            })
        })
        .expect("an open corridor somewhere");
    stand(&mut game, from, dir);

    let refused = cast(&mut game, PHASE, FieldCastTarget::None);

    assert!(
        refused.is_err(),
        "Phase crosses a wall; with open corridor ahead the player should just walk"
    );
}

/// Neither routine reaches zone-map state, so `require_surface` is not what
/// guards them — they read and write `Locale::Stack`'s own coordinates, and
/// the guard is that locale being absent.
#[test]
fn neither_routine_runs_on_the_surface() {
    let mut game = surfaced_with_routines();
    for id in [PHASE, JUMP] {
        let refused = cast(&mut game, id, FieldCastTarget::Cell(3, 3));
        assert!(refused.is_err(), "{id} ran on open grid");
    }
}

#[test]
fn neither_routine_runs_mid_battle_or_after_game_over() {
    let mut game = underground();
    let player = game.player_entity();
    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);
    for id in [PHASE, JUMP] {
        assert!(
            cast(&mut game, id, FieldCastTarget::Cell(3, 3)).is_err(),
            "{id} ran mid-battle"
        );
    }
    flee_until_clear(&mut game);

    game.world
        .resource_mut::<crate::resources::GameOver>()
        .reason = Some("test".into());
    for id in [PHASE, JUMP] {
        assert!(
            cast(&mut game, id, FieldCastTarget::Cell(3, 3)).is_err(),
            "{id} ran after game over"
        );
    }
}

#[test]
fn a_refused_cast_spends_no_power_and_raises_no_trace() {
    let mut game = underground();
    let level = frame(&game);
    let (from, dir, _) = wall_site(&level, |l, c| in_bounds(l, c) && !l.walkable(c.0, c.1))
        .expect("two-deep rock somewhere");
    stand(&mut game, from, dir);
    let (before_power, before_trace) = (power(&game), trace(&game));

    assert!(cast(&mut game, PHASE, FieldCastTarget::None).is_err());
    assert!(cast(&mut game, JUMP, FieldCastTarget::Cell(-4, -4)).is_err());

    assert_eq!(power(&game), before_power, "a refusal charged Power");
    assert_eq!(trace(&game), before_trace, "a refusal raised Trace");
}

#[test]
fn both_routines_charge_power_and_raise_trace_on_success() {
    let mut game = underground();
    let level = frame(&game);

    let (from, dir, _) = wall_site(&level, |l, c| l.walkable(c.0, c.1)).unwrap();
    stand(&mut game, from, dir);
    let before = power(&game);
    cast(&mut game, PHASE, FieldCastTarget::None).unwrap();
    assert!(power(&game) < before, "a phase cost no Power");
    assert_eq!(trace(&game), TRACE_PER_PHASE);

    // Onto the cell it just came from, which is known walkable and known
    // reachable — this test is about the meter, not the landing.
    let before = power(&game);
    cast(&mut game, JUMP, FieldCastTarget::Cell(from.0, from.1)).unwrap();
    assert!(power(&game) < before, "a jump cost no Power");
    assert_eq!(trace(&game), TRACE_PER_PHASE + TRACE_PER_JUMP);
}

#[test]
fn jumping_to_a_mapped_floor_cell_arrives_there() {
    let mut game = underground();
    let level = frame(&game);
    let entry = level.entry;
    let target = cells(&level)
        .into_iter()
        .filter(|&c| level.cell(c.0, c.1) == CellKind::Floor)
        .max_by_key(|c| (c.0 - entry.0).abs() + (c.1 - entry.1).abs())
        .expect("a floor cell");

    cast(&mut game, JUMP, FieldCastTarget::Cell(target.0, target.1))
        .expect("a plain floor cell is a legal landing");

    assert_eq!(at(&game), target);
}

/// The regression that matters is a *fourth* arrival path skipping the tail
/// `Game::step` used to hold inline, so this asserts behaviour — a cache
/// emptied by a jump — rather than that some function was called.
#[test]
fn a_jump_fires_the_arrival_tail() {
    let mut game = underground();
    let level = frame(&game);
    let cache = cells(&level)
        .into_iter()
        .find(|&c| level.cell(c.0, c.1) == CellKind::Cache)
        .expect("every frame lays caches");

    cast(&mut game, JUMP, FieldCastTarget::Cell(cache.0, cache.1)).unwrap();

    let pos = game.stack_pos().unwrap();
    assert!(
        !game.cache_unopened(pos, cache),
        "jumping onto a cache left it unopened — the arrival tail did not run"
    );
}

/// Both hazards and both prizes ride the same tail, so a fault is the other
/// half of the same property: jump onto one and you fall through it.
#[test]
fn jumping_onto_a_fault_drops_the_party_a_frame() {
    let mut game = underground();
    let level = frame(&game);
    let Some(fault) = cells(&level)
        .into_iter()
        .find(|&c| level.cell(c.0, c.1) == CellKind::Fault)
    else {
        // Faults are laid on every frame but the bottom one; a stack two
        // frames deep still has them on depth 1. If this seed ever produces
        // a frame without one, the property is untested rather than wrong.
        panic!("expected at least one fault on depth 1");
    };
    let depth = game.stack_pos().unwrap().depth;

    cast(&mut game, JUMP, FieldCastTarget::Cell(fault.0, fault.1)).unwrap();

    assert_eq!(
        game.stack_pos().unwrap().depth,
        depth + 1,
        "the fault did not fire on a jump the way it does on a step"
    );
}

/// Rock is the one `CellKind` that is both unwalkable and sight-blocking, so
/// a party standing inside one is the occluder trap doors sprang. Not
/// writing `Locale` is what makes that state unreachable rather than merely
/// unlikely.
#[test]
fn jumping_into_rock_ends_a_permadeath_run_and_never_writes_the_locale() {
    let mut game = underground_on(DifficultyMode::Permadeath);
    let level = frame(&game);
    let rock = solid_cell(&level);
    let before = at(&game);

    cast(&mut game, JUMP, FieldCastTarget::Cell(rock.0, rock.1))
        .expect("jumping into rock is allowed — that is the gamble");

    assert!(
        game.is_game_over().is_some(),
        "materialising inside rock has to be fatal"
    );
    assert_eq!(
        at(&game),
        before,
        "the party was moved into the rock rather than killed where they stood"
    );
}

/// The other half of the same property. On Forgiving the death is survived,
/// and what gets the party out is the reboot path that was already built —
/// `difficulty::death_handling_system` warping them out through
/// `stack::surfaced`. The jump itself still never writes `Locale`, which is
/// why the party surfaces rather than rebooting inside the wall.
#[test]
fn jumping_into_rock_reboots_a_forgiving_run_onto_open_grid() {
    let mut game = underground();
    let level = frame(&game);
    let rock = solid_cell(&level);

    cast(&mut game, JUMP, FieldCastTarget::Cell(rock.0, rock.1)).unwrap();

    assert!(game.is_game_over().is_none(), "Forgiving survives a death");
    assert_eq!(
        game.locale(),
        Locale::Surface,
        "the reboot left the party underground"
    );
}

/// A rock cell well inside the maze rather than the border, so the test is
/// about solid substrate rather than about the bounds check.
fn solid_cell(level: &crate::stack::Frame) -> (i32, i32) {
    cells(level)
        .into_iter()
        .find(|&(x, y)| {
            x > 1
                && y > 1
                && x < level.width - 2
                && y < level.height - 2
                && level.cell(x, y) == CellKind::Rock
        })
        .expect("a bordered maze is mostly rock")
}

/// Both routines refuse a landing the party has not earned their way to.
/// Two separate refusals, and the second is not redundant: a sealed door is
/// `walkable()`, and `Game::step` only consults `pass_seal` for the cell
/// being stepped *into* — so landing on the door is the bypass, not landing
/// past it.
#[test]
fn neither_routine_lands_behind_an_unopened_seal_or_on_one() {
    let mut game = underground();
    let (seal, lair) = bottom_frame_with_a_seal(&mut game);

    for cell in [seal, lair] {
        let refused = cast(&mut game, JUMP, FieldCastTarget::Cell(cell.0, cell.1));
        assert!(
            refused.is_err(),
            "a jump reached {cell:?} without forcing a seal"
        );
    }
}

#[test]
fn a_forced_seal_stops_excluding_the_wing_behind_it() {
    let mut game = underground();
    let (seal, lair) = bottom_frame_with_a_seal(&mut game);
    let pos = game.stack_pos().unwrap();
    game.frame_memory_mut(pos).opened.insert(seal);

    cast(&mut game, JUMP, FieldCastTarget::Cell(lair.0, lair.1))
        .expect("the wing is the party's once the seal has been forced");
    assert_eq!(at(&game), lair);
}

/// Puts the party on the bottom frame of the stack they are in and returns
/// `(a sealed door, the lair behind it)`. Seals only exist down there — they
/// are what walls the guardian off from the rest of the frame.
fn bottom_frame_with_a_seal(game: &mut Game) -> ((i32, i32), (i32, i32)) {
    let pos = game.stack_pos().expect("already underground");
    game.restore_locale(Locale::Stack {
        depth: pos.frames,
        frames: pos.frames,
        x: 0,
        y: 0,
        facing: Dir::North,
        entrance: pos.entrance,
    });
    let level = frame(game);
    // Stood on the frame's own entry, which is where a descent would have
    // put them and is by definition on the legitimate side of the seal.
    stand(game, level.entry, Dir::North);

    let seal = cells(&level)
        .into_iter()
        .find(|&c| level.cell(c.0, c.1) == CellKind::SealedDoor)
        .expect("the bottom frame seals its lair off");
    let lair = cells(&level)
        .into_iter()
        .find(|&c| level.cell(c.0, c.1) == CellKind::Lair)
        .expect("the bottom frame holds the lair");
    (seal, lair)
}

/// A jump is refused outright rather than resolved against a clamped
/// coordinate. The cursor bounds this in the UI; the engine does not take
/// the picker's word for it.
#[test]
fn a_jump_outside_the_frame_is_refused() {
    let mut game = underground();
    let before = at(&game);
    assert!(cast(&mut game, JUMP, FieldCastTarget::Cell(-1, 4)).is_err());
    assert!(cast(&mut game, JUMP, FieldCastTarget::Cell(4, 999)).is_err());
    assert_eq!(at(&game), before);
}

#[test]
fn the_picker_asks_for_a_cell_only_for_the_jump() {
    let mut game = underground();
    let rows = game.field_routines();
    let pick = |id: &str| {
        rows.iter()
            .find(|r| r.ability == id)
            .map(|r| r.second_pick)
            .unwrap()
    };
    assert_eq!(pick(JUMP), FieldCastPick::Cell);
    assert_eq!(pick(PHASE), FieldCastPick::None);
}

/// The rows are greyed with the permanent objection rather than the
/// temporary one: telling a player on open grid they are short of Power
/// would send them to rest for a routine that was never going to run there.
#[test]
fn the_picker_greys_both_routines_on_the_surface_and_prices_them_in_power() {
    let mut game = surfaced_with_routines();
    for r in game.field_routines() {
        assert!(r.cost.ends_with("PWR"), "{} costs {}", r.ability, r.cost);
        assert_eq!(r.unavailable.as_deref(), Some("only in the Stack"));
    }
}

#[test]
fn a_routine_the_player_cannot_pay_for_is_greyed_rather_than_hidden() {
    let mut game = underground();
    *game
        .world
        .get_mut::<PowerReserve>(game.player_entity())
        .unwrap() = PowerReserve::new(1.0);
    let rows = game.field_routines();
    assert_eq!(rows.len(), 2, "an unaffordable routine must not vanish");
    assert!(
        rows.iter()
            .all(|r| r.unavailable.as_deref() == Some("not enough PWR"))
    );
    assert!(cast(&mut game, PHASE, FieldCastTarget::None).is_err());
}

/// Installing either routine goes through the ordinary door — a research
/// node teaches it, a manufactured disk writes it — with no new machinery.
#[test]
fn both_routines_install_the_way_every_other_researched_routine_does() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    set_level(&mut game, player, 30);
    for id in [PHASE, JUMP] {
        install_routine_for_test(&mut game, player, id);
    }
    let installed = game.world.get::<Routines>(player).unwrap().0.clone();
    assert!(installed.contains(&PHASE.to_string()));
    assert!(installed.contains(&JUMP.to_string()));
}

/// A landing is unwalkable or it is not; `StackMemory` has no say. The
/// gamble is about what the *player* has seen, not about what the engine
/// has recorded — which is why the map drawing unknown cells as unknown is
/// the whole of the warning.
#[test]
fn an_unseen_floor_cell_is_a_legal_landing() {
    let mut game = underground();
    let level = frame(&game);
    let target = cells(&level)
        .into_iter()
        .find(|&c| level.cell(c.0, c.1) == CellKind::Floor && !seen(&game, c))
        .expect("a corridor the party has not walked");

    cast(&mut game, JUMP, FieldCastTarget::Cell(target.0, target.1))
        .expect("an unseen cell is exactly what a wild jump is for");
    assert_eq!(at(&game), target);
}

fn seen(game: &Game, cell: (i32, i32)) -> bool {
    let pos = game.stack_pos().unwrap();
    game.world
        .resource::<StackMemory>()
        .0
        .get(&(pos.entrance, pos.depth))
        .is_some_and(|m| m.seen.contains(&cell))
}
