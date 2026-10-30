use super::*;
use crate::abilities::AbilityDb;
use crate::*;

/// Fuses `game`'s two freshest tamed programs together repeatedly to
/// build up a lineage `depth` fusions deep, returning that program.
pub(crate) fn fuse_to_depth(game: &mut Game, depth: u32) -> Entity {
    unlock_research_chain(game, "program_refactoring");
    let mut current = spawn_tamed(game, 10, 3);
    for _ in 0..depth {
        let partner = spawn_tamed(game, 10, 3);
        game.fuse_companions(current, partner, None).unwrap();
        current = game
            .owned_pets()
            .into_iter()
            .max_by_key(|p| p.fusions)
            .expect("the fusion result should be owned")
            .entity;
    }
    current
}

/// A Core Fragment extractor, spawned without paying for it — the plainest
/// structure a `GatherResource` cronjob can be posted to.
///
/// Bare: no `Stock` and no `MachineStatus`, so `task_progress_system` skips
/// it and it never produces. That is what the tests about *posting* want;
/// anything measuring a payout curve needs `work_node_parts()` as well, and
/// a node short of those reads as a rate that moved rather than as a fixture
/// missing a component.
pub(crate) fn spawn_extractor(game: &mut Game, x: i32, y: i32) -> Entity {
    game.lay_test_pocket();
    game.world
        .spawn((
            Structure {
                kind: "extractor".to_string(),
            },
            crate::components::MachineRecipe(Some(ItemId::from(ids::CORE_FRAGMENT))),
            Position { x, y },
            ResourceNode {
                resource: ItemId::from(ids::CORE_FRAGMENT),
                level: None,
            },
        ))
        .id()
}

/// A trading structure, spawned without paying for it.
pub(crate) fn spawn_market(game: &mut Game) -> Entity {
    spawn_market_at(game, 5, 5)
}

/// The same, at a chosen tile — for tests about the buyback shelf, which is
/// keyed by the tile its trader stands on.
pub(crate) fn spawn_market_at(game: &mut Game, x: i32, y: i32) -> Entity {
    game.lay_test_pocket();
    let kind = game
        .structure_defs()
        .into_iter()
        .find(|d| {
            d.trade
                .as_ref()
                .is_some_and(|t| t.program_sell_divisor.is_some())
        })
        .expect("a trader that buys programs should ship")
        .id
        .clone();
    game.world
        .spawn((Structure { kind }, Position { x, y }))
        .id()
}

/// Puts `qty` of `item` straight into the player's pack, bypassing whatever
/// would normally have to be mined, looted or bought to get it there.
pub(crate) fn give(game: &mut Game, item: &ItemId, qty: u32) {
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(item.clone(), qty);
}

/// Writes `ability` into `entity`'s next free slot, making room first. Most
/// tests want a routine sitting in a slot rather than the chain that got it
/// there; the player's real door is `Game::install_built`, which only takes
/// a routine built from parts.
pub(crate) fn install_routine_for_test(game: &mut Game, entity: Entity, ability: &str) {
    make_room_for_routine(game, entity);
    game.write_routine(entity, ability);
}

/// Mints a built routine out of `core` at version 1.0 and returns its
/// `crafted/<n>` id, without installing it — a fixture that needs a
/// particular authored-style effect on a player builds it the way the game
/// would. Ids are sequential from a new game's Patch (`crafted/1`), so a
/// fixture that mints in a fixed order may name the ids.
pub(crate) fn mint_core(game: &mut Game, core: crate::routine_craft::Core) -> String {
    game.mint_built(
        core.name(),
        &[crate::routine_craft::Subroutine::Core(
            core,
            crate::routine_craft::Version::V1,
        )],
    )
}

/// Mints a built routine out of exactly `parts`, for a fixture that needs a
/// routine of a particular shape (a Core plus an Area, say).
pub(crate) fn mint_parts(game: &mut Game, parts: &[crate::routine_craft::Subroutine]) -> String {
    game.mint_built("Test routine", parts)
}

/// The id of the built routine made from `core` alone, minting it the first
/// time it is asked for. The tests that once read a shipped routine by id
/// (Teleport, Symlink, Detach...) read the routine the game would compile.
pub(crate) fn built_core(game: &mut Game, core: crate::routine_craft::Core) -> String {
    let existing = game
        .world
        .resource::<crate::resources::CraftedRoutines>()
        .built
        .iter()
        .find(|r| {
            r.parts
                == [crate::routine_craft::Subroutine::Core(
                    core,
                    crate::routine_craft::Version::V1,
                )]
        })
        .map(|r| r.id.clone());
    existing.unwrap_or_else(|| mint_core(game, core))
}

/// Frees a slot on a full `entity` by destroying the first built routine it
/// holds: a new game starts with Decompile and Patch (a built Heal), so a
/// test that wants a second routine on the player has no free slot.
pub(crate) fn make_room_for_routine(game: &mut Game, entity: Entity) {
    let held = game.world.get::<Routines>(entity).unwrap().0.clone();
    if held.len() < game.routine_slots(entity) {
        return;
    }
    if let Some(built) = held.iter().find(|id| id.starts_with("crafted/")) {
        let kept: Vec<String> = held.iter().filter(|id| *id != built).cloned().collect();
        game.world.entity_mut(entity).insert(Routines(kept));
        game.destroy_built(std::slice::from_ref(built));
    }
}

/// Empties `entity`'s routine slots outright.
///
/// A direct write, not `Game::uninstall_routine`: `decompile` is welded into
/// the player's slot 0 and that verb refuses it, so an empty player kit is
/// reachable only as state — what a save written before that rule can carry,
/// and what `battle_action_options` hides its Special row for.
pub(crate) fn clear_routines(game: &mut Game, entity: Entity) {
    game.world.entity_mut(entity).insert(Routines(Vec::new()));
}

/// `count` routine ids a test can park in a kit to fill it — catalogue
/// order, skipping `avoid` and anything one-of-a-kind (`decompile` is
/// refused a second home by design).
pub(crate) fn spare_routine_ids(game: &Game, count: usize, avoid: &[&str]) -> Vec<String> {
    let mut ids: Vec<String> = game
        .world
        .resource::<AbilityDb>()
        .all()
        .filter(|def| !def.exclusive)
        .map(|def| def.id.clone())
        .filter(|id| !avoid.contains(&id.as_str()))
        .collect();
    ids.sort();
    ids.truncate(count);
    assert_eq!(
        ids.len(),
        count,
        "the catalogue is short of routines to fill a kit with"
    );
    ids
}

/// Installs routines into `entity` until only `spare` slots are free, so a
/// test about the full-kit branch says how much room it wants rather than
/// restating how many slots a level happens to buy.
pub(crate) fn fill_routine_slots(game: &mut Game, entity: Entity, spare: usize, avoid: &[&str]) {
    let kit = |game: &Game| {
        game.world
            .get::<Routines>(entity)
            .map(|r| r.0.clone())
            .unwrap_or_default()
    };
    while kit(game).len() + spare < game.routine_slots(entity) {
        let installed = kit(game);
        let mut taken: Vec<&str> = avoid.to_vec();
        taken.extend(installed.iter().map(|id| id.as_str()));
        let id = spare_routine_ids(game, 1, &taken).remove(0);
        install_routine_for_test(game, entity, &id);
    }
}

/// How many *ordinary* (unfused) copies of `item` the player is carrying.
pub(crate) fn held(game: &Game, item: &ItemId) -> u32 {
    held_at(game, item, 0)
}

pub(crate) fn fragments(game: &Game) -> u32 {
    game.world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .count(&ItemId::from(ids::CORE_FRAGMENT))
}

/// The trade currency, which is what every trader pays — distinct from
/// `fragments`, the build salvage no trader deals in.
pub(crate) fn credits(game: &Game) -> u32 {
    game.world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .count(&ItemId::from(ids::CREDITS))
}

/// Finds the deployed Home, if any. Home is the only structure of its
/// kind, so the first match is the only match.
pub(crate) fn find_home(game: &mut Game) -> Option<Entity> {
    let mut query = game.world.query::<(Entity, &Structure)>();
    query
        .iter(&game.world)
        .find(|(_, s)| s.kind == HOME_STRUCTURE_ID)
        .map(|(e, _)| e)
}

/// How many copies of `item` at fusion `tier` the player is carrying —
/// `Inventory` at tier 0, `GearCopies` above it, the same split
/// `Game::count_copies` makes.
///
/// Reads the stores rather than `PlayerStatus::inventory` on purpose: that
/// list omits banked items, so a view-based helper would report 0 for
/// Research Data and read as a payout bug.
pub(crate) fn held_at(game: &Game, item: &ItemId, tier: u32) -> u32 {
    let player = game.player_entity();
    let copy = gear(item, tier);
    if copy.is_plain() {
        game.world
            .get::<Inventory>(player)
            .map(|inv| inv.count(item))
            .unwrap_or(0)
    } else {
        game.world
            .get::<GearCopies>(player)
            .map(|f| f.count(&copy))
            .unwrap_or(0)
    }
}

/// A carried copy of `item` at fusion `tier`, ordinary rare tier — what a
/// gear test means when it says "a copy" unless it is specifically testing
/// rare tiers.
///
/// Exists so the ~90 call sites that used to pass `(&item, tier)` as two
/// loose arguments keep reading the same way now that `items::GearCopy` is
/// the unit. A test that *is* about rare tiers builds the struct directly,
/// which makes those cases stand out on the page rather than hiding behind
/// a defaulted parameter.
pub fn gear(item: &ItemId, tier: u32) -> GearCopy {
    GearCopy {
        rarity: Rarity::Ordinary,
        tier,
        affixes: Vec::new(),
        ..GearCopy::plain(item.clone())
    }
}

/// Whether `f` left the shared `GameRng` stream exactly where it found it.
///
/// A gate that refuses *before* drawing keeps every seeded test downstream of
/// it in place; one that draws and then discards the result moves them all.
/// Asserting only on the returned value would pass equally well either way,
/// which is the regression this exists to catch — see `Game::roll_rarity`
/// (the two ineligible spawns) and `Game::grant_gear_drop` (a material,
/// which has no tier to roll).
///
/// `StdRng` is not `Clone`, so the proof is two games built from one seed:
/// they share a stream position, and only one is asked to do the thing. If it
/// spent a draw, their next values diverge.
pub(crate) fn rng_unadvanced_by(seed: u32, f: impl FnOnce(&mut Game)) -> bool {
    let mut touched = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut untouched = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    f(&mut touched);
    let after: u64 = touched.world.resource_mut::<GameRng>().0.random();
    let baseline: u64 = untouched.world.resource_mut::<GameRng>().0.random();
    after == baseline
}

/// How many copies of `item` the player is carrying **at any tier** — both
/// stores summed.
///
/// `held` above counts only plain copies, which is what most tests mean.
/// This one is for tests asking "did the player gain one of these at all",
/// where the answer must not depend on what the copy rolled — a surface
/// boss's gear now always lands in `GearCopies`, so counting `Inventory`
/// alone reads as the boss having paid nothing.
pub(crate) fn held_any(game: &Game, item: &ItemId) -> u32 {
    let player = game.player_entity();
    let plain = game
        .world
        .get::<Inventory>(player)
        .map(|inv| inv.count(item))
        .unwrap_or(0);
    let special: u32 = game
        .world
        .get::<GearCopies>(player)
        .map(|g| {
            g.copies
                .iter()
                .filter(|(copy, _)| copy.item == *item)
                .map(|(_, qty)| *qty)
                .sum()
        })
        .unwrap_or(0);
    plain + special
}
