//! Watching a program work — `Game::watch_position`, the one door the
//! camera's follow reads.

use super::support::*;
use crate::components::{
    Creature, Glyph, GlyphColor, Position, Stats, Structure, Tamed, Task, TaskKind,
};
use crate::resources::{Sorties, Sortie, WieldedProgram};
use crate::*;

/// A base with a Home standing, the party inside it, and the tutorial out of
/// the way. `hauling.rs`' fixture, which every watchable body needs: staff
/// stand in base space and there is nothing to watch from outside it.
fn base(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    skip_tutorial(&mut game);
    place_home(&mut game);
    stand_in_base(&mut game);
    game
}

/// A tamed program the scheduler treats as base staff, standing at `offset`
/// from base space's origin.
fn staffer(game: &mut Game, dx: i32, dy: i32) -> Entity {
    let (ox, oy) = game.base_pos().expect("call from inside base space");
    let species = game.species_defs().into_iter().next().unwrap();
    let owner = game.player_entity();
    game.world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Position {
                x: ox + dx,
                y: oy + dy,
            },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                mitigation: 1,
            },
            Tamed { owner },
            // The map's entity query is `(Entity, &Position, &Glyph)`, so a
            // body with no glyph is in no window at all.
            Glyph {
                ch: 'p',
                color: GlyphColor::Green,
            },
        ))
        .id()
}

/// The plain case: an idle staff program is walked by the scheduler every
/// tick, so the camera may sit on it.
#[test]
fn an_idle_staff_program_can_be_watched_where_it_stands() {
    let mut game = base(1);
    let origin = game.base_pos().unwrap();
    let staff = staffer(&mut game, 2, -1);

    assert_eq!(
        game.watch_position(staff),
        Some((origin.0 + 2, origin.1 - 1)),
        "an idle staff program stands where the scheduler parked it"
    );
}

/// **The regression this feature turns on.** `position_is_honest` is
/// `wears_job_mark || idle staff`, and `mark_sits_on_the_post` answers true
/// for a worker standing at its machine — so the honest flag goes *false*
/// at exactly the moment the player wanted to watch it work. Its `Position`
/// is the post's tile and perfectly live; it is merely hidden under the
/// machine's own glyph. Gated on `position_is_honest`, this drops the
/// camera the instant the body arrives.
#[test]
fn a_worker_standing_at_its_post_can_still_be_watched() {
    let mut game = base(1);
    let (ox, oy) = game.base_pos().unwrap();
    let post = game
        .world
        .spawn((
            Structure {
                kind: "refinery".to_string(),
            },
            Position { x: ox, y: oy },
        ))
        .id();
    let staff = staffer(&mut game, 1, 0);
    park_at_post(&mut game, staff, post);
    game.world.entity_mut(staff).insert(Task {
        kind: TaskKind::GatherResource,
        target: post,
        progress: 0,
        required: 10,
    });

    assert!(
        !game.position_is_honest(staff),
        "fixture must put the body where the mark sits on the post, or this \
         test proves nothing about the difference between the two rules"
    );
    let at = *game.world.get::<Position>(staff).unwrap();
    assert_eq!(
        game.watch_position(staff),
        Some((at.x, at.y)),
        "a worker at its post is walked by the sim and stands where it says"
    );
}

/// The four whose `Position` is written once and never again. Watching one
/// parks the camera on the tile it was beaten on — out on the surface, or
/// four frames down the Stack.
#[test]
fn a_program_the_sim_never_walks_cannot_be_watched() {
    let mut game = base(1);

    let companion = staffer(&mut game, 1, 1);
    enlist(&mut game, companion);
    assert_eq!(
        game.watch_position(companion),
        None,
        "a party member stands beside you, not where it was caught"
    );

    let wielded = staffer(&mut game, 2, 2);
    game.world.resource_mut::<WieldedProgram>().0 = Some(wielded);
    assert_eq!(
        game.watch_position(wielded),
        None,
        "a wielded program is in your hand"
    );

    let away = staffer(&mut game, 3, 3);
    game.world
        .resource_mut::<Sorties>()
        .0
        .push(Sortie::test_stub(vec![away]));
    assert_eq!(
        game.watch_position(away),
        None,
        "a dispatched program is off the map entirely"
    );

    let guard = staffer(&mut game, 4, 4);
    let post = game
        .world
        .spawn((
            Structure {
                kind: "refinery".to_string(),
            },
            Position { x: 0, y: 0 },
        ))
        .id();
    game.world.entity_mut(guard).insert(Task {
        kind: TaskKind::Guard,
        target: post,
        progress: 0,
        required: 0,
    });
    assert_eq!(
        game.watch_position(guard),
        None,
        "nothing ever walks a guard to what it guards"
    );
}

/// Staff stand in base space and the map draws one space at a time, so
/// there is nothing to watch from the surface.
#[test]
fn nothing_can_be_watched_from_outside_base_space() {
    let mut game = base(1);
    let staff = staffer(&mut game, 1, 0);
    assert!(game.watch_position(staff).is_some());

    game.leave_base().expect("the party can always step out");
    assert_eq!(
        game.watch_position(staff),
        None,
        "a base-space cell drawn over the zone surface is the aliasing every \
         other map-facing view already refuses"
    );
}

/// A program that has been dissolved, or was never one, answers nothing
/// rather than panicking — this is the read the camera makes every frame.
#[test]
fn a_program_that_is_gone_cannot_be_watched() {
    let mut game = base(1);
    let staff = staffer(&mut game, 1, 0);
    assert!(game.watch_position(staff).is_some());

    game.world.despawn(staff);
    assert_eq!(game.watch_position(staff), None);

    assert_eq!(
        game.watch_position(game.player_entity()),
        None,
        "the player is what watching returns you to, never a subject of it"
    );
}

/// The whole camera change is one value: the tile the two view calls are
/// centred on. Watching re-points that and nothing else, so the tile window,
/// the entity window and the effect overlay all move together.
///
/// The uncentred pair keep `scan_center`, which is what the inspector and
/// `run_symlink` target from — a camera that moved *those* would let the
/// player examine and teleport from wherever they happened to be looking.
#[test]
fn the_centred_views_are_centred_where_they_are_told() {
    let mut game = base(1);
    let (ox, oy) = game.base_pos().unwrap();
    let staff = staffer(&mut game, 3, 2);
    let watched = game.watch_position(staff).unwrap();

    let seen: Vec<_> = game
        .view_entities_at(watched, 0, 0)
        .into_iter()
        .map(|v| v.entity)
        .collect();
    assert!(
        seen.contains(&staff),
        "a zero-radius window on the watched tile holds the watched program"
    );

    let seen: Vec<_> = game
        .view_entities(0, 0)
        .into_iter()
        .map(|v| v.entity)
        .collect();
    assert!(
        !seen.contains(&staff),
        "the uncentred call still looks at the party's own cell, three tiles \
         away — `scan_center` is what examine and symlink target from"
    );

    // Base space is derived per cell from `(seed, block)`, so two windows on
    // the same tile agree by construction and a shifted one need not: the
    // assertion that carries is that the *centre* moved, which the entity
    // window above already shows. This one pins that the tile call takes the
    // same coordinates rather than quietly ignoring them.
    let there = game.view_tiles_at(watched, 0, 0);
    let here = game.view_tiles_at((ox, oy), 0, 0);
    assert_eq!(there.len(), 1, "a zero-radius window is one row of one");
    assert_eq!(here.len(), 1);
    let shifted = game.view_tiles_at((ox + 3, oy + 2), 0, 0);
    assert_eq!(
        there[0][0].biome, shifted[0][0].biome,
        "the same coordinates must produce the same cell however they were \
         arrived at"
    );
}
