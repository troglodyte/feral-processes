use crate::*;

/// Raises the player's Analysis until they see at least twelve tiles, so a
/// test that places something at the examine range is not hidden by the
/// fog. Goes through the real derivation; there is no sight bypass.
pub(crate) fn see_twelve_tiles(game: &mut Game) {
    let analysis = crate::attributes::AttributeId::from("analysis");
    let player = game.player_entity();
    let base = game
        .world
        .resource::<crate::attributes::AttributeDb>()
        .get(&analysis)
        .unwrap()
        .base;
    game.world
        .get_mut::<crate::components::Attributes>(player)
        .unwrap()
        .set(&analysis, base + 14);
    game.recompute_derived(player);
    game.mark_seen_tiles();
}

/// Gives every defence the base stands (a Shield, a Turret) a few Charge
/// Coils, so a fixture written when defences ran free still defends. A bare
/// `Structure` spawned without a `Stock` gets one.
pub(crate) fn charge_defences(game: &mut Game) {
    let coil = ItemId::from(ids::CHARGE_COIL);
    let armed: Vec<Entity> = {
        let mut query = game.world.query::<(Entity, &Structure)>();
        let all: Vec<(Entity, String)> = query
            .iter(&game.world)
            .map(|(e, s)| (e, s.kind.clone()))
            .collect();
        let db = game.world.resource::<crate::structures::StructureDb>();
        all.into_iter()
            .filter(|(_, kind)| {
                db.get(kind)
                    .is_some_and(|d| d.is_defence() && d.supply.is_some())
            })
            .map(|(e, _)| e)
            .collect()
    };
    for entity in armed {
        let mut entity = game.world.entity_mut(entity);
        if !entity.contains::<Stock>() {
            entity.insert(Stock::new(10));
        }
        entity
            .get_mut::<Stock>()
            .unwrap()
            .input
            .insert(coil.clone(), 50);
    }
}
/// Puts `n` Patch Routines on a shelf the base can spend from, standing in
/// for a Depot the fixture never built. A rest in base now costs them
/// (`Game::rest`), so a test about anything else that rests stocks them
/// rather than weakening what it asserts about the heal.
///
/// Placed far from any base-space tile a fixture uses so it cannot collide
/// with a deployed structure.
pub(crate) fn stock_patches(game: &mut Game, n: u32) {
    let mut stock = Stock::new(10_000);
    stock.output.insert(ItemId::from(ids::PATCH_ROUTINE), n);
    game.world.spawn((
        Structure {
            kind: "depot".to_string(),
        },
        Position { x: 9_000, y: 9_000 },
        stock,
    ));
}

/// Lays floor over the inclusive box `a..=b`.
pub(crate) fn lay_floor_box(game: &mut Game, a: (i32, i32), b: (i32, i32)) {
    let mut grid = game.world.resource_mut::<crate::base_grid::BaseGrid>();
    for x in a.0..=b.0 {
        for y in a.1..=b.1 {
            grid.lay_floor(x, y);
        }
    }
}

/// A load of `qty` plain `item` units lies on the floor, one of them on `at`.
#[track_caller]
pub(crate) fn assert_load_on_floor(game: &mut Game, at: Position, item: &str, qty: u32) {
    let thing = crate::items::FloorThing::Item(ItemId::from(item));
    let cells: Vec<_> = floor_things(game)
        .into_iter()
        .filter(|(_, t)| *t == thing)
        .map(|(c, _)| c)
        .collect();
    assert_eq!(cells.len(), qty as usize, "units of {item} on the floor");
    assert!(
        cells.contains(&(at.x, at.y)),
        "none on its own tile {at:?}: {cells:?}"
    );
}

/// Every floor item as `(cell, thing)`, sorted by cell.
pub(crate) fn floor_things(game: &mut Game) -> Vec<((i32, i32), crate::items::FloorThing)> {
    let mut rows: Vec<_> = game
        .world
        .query::<(&Position, &crate::components::FloorItem)>()
        .iter(&game.world)
        .map(|(p, f)| ((p.x, p.y), f.0.clone()))
        .collect();
    rows.sort_by_key(|(c, _)| *c);
    rows
}

/// The one structure beside the party, as a transfer scope — what a
/// player's hub is open on. Panics when there is none or several, so a
/// fixture that meant a particular structure says which.
pub(crate) fn adjacent_at(game: &Game) -> TransferAt {
    let found = game.adjacent_structures();
    assert_eq!(found.len(), 1, "expected exactly one structure beside");
    TransferAt::Structure(found[0].entity)
}
