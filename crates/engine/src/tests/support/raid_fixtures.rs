use super::*;
use crate::game::spawning;
use crate::tuning::NEST_DURABILITY;
use crate::*;

/// How many `raid_check` rolls each seed gets in the sweeps below.
/// `RAID_CHANCE_PER_TICK` is a per-call roll, so a single call per seed
/// leaves a ~2.7% chance of a 300-seed sweep never firing at all — which
/// unsorted habitat lookup can turn from a stable pass into a flake by
/// shifting RNG consumption between runs. Seven attempts takes that to
/// ~1e-11. Every sweep returns on the first fire, so no target ever takes
/// a second hit.
/// How many forced sweeps a per-seed raid fixture takes.
///
/// It used to be how many *rolls* a fixture spent waiting for a 1.2% chance
/// to land. `Game::raid_check` is a clock now, so `sweep_now` forces each
/// one and the first attempt always lands — the count is kept because these
/// tests vary which structure and which defender a sweep picks, and several
/// of them assert across repeats rather than on one.
pub(crate) const RAID_ATTEMPTS_PER_SEED: u32 = 7;

/// Spawns just enough undowned base staff (`spawn_tamed`, left unposted so
/// they fall out as `ProgramRole::Staff` by construction) to clear
/// `RAID_MIN_BASE_STAFF` — the floor a fixture testing raid *mechanics*
/// must clear so `Game::raid_check`'s new staff gate never masks the roll
/// it exists to protect.
/// Winds the raid clock past any interval its jitter can draw and ticks it
/// once, so a test about what a sweep *does* gets exactly one without also
/// standing in for the clock's own tests.
///
/// Goes through `Game::raid_check` rather than around it, so every gate a
/// real sweep passes — the sector floor, the staff floor, having anything to
/// sweep at all — is still in force. A test that wants no sweep must still
/// not call this.
pub(crate) fn sweep_now(game: &mut Game) {
    game.world
        .resource_mut::<crate::resources::RaidPressure>()
        .level = crate::tuning::RAID_PRESSURE_THRESHOLD * 2;
    game.raid_check();
}

pub(crate) fn spawn_min_raid_staff(game: &mut Game) {
    for _ in 0..crate::tuning::RAID_MIN_BASE_STAFF {
        spawn_tamed(game, 10, 3);
    }
}

/// Deploys a Home plus a Mining Node beside it, returning both entities
/// so a caller can assert on what survives a breach.
pub(crate) fn build_a_base(game: &mut Game) -> (Entity, Entity) {
    // On the player's own tile, which is walkable by definition — the
    // Home's slab then guarantees everything around it is too, so this
    // works for any seed.
    place_home(game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 12);
    from_inside_the_base(game, |g| {
        place_machine(g, "extractor", ids::CORE_FRAGMENT, 1, 1)
    })
    .unwrap();
    (
        find_structure_by_kind(game, "home").unwrap(),
        find_structure_by_kind(game, "extractor").unwrap(),
    )
}

/// Deploys a Mining Node beside a Home with materials to spare, and
/// returns it.
///
/// Orthogonally east of the player rather than on a diagonal, which is
/// twice deliberate: the build menu offers only the four orthogonals
/// (`App::handle_build_direction_key`), so a diagonal node is a tile no
/// player can build on, and `Game::work_structure` refuses a node the
/// player is not standing at the station of — so a diagonal fixture left
/// every test that works this node by hand testing an unreachable state.
pub(crate) fn deploy_upgradeable_node(game: &mut Game) -> Entity {
    place_home(game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 12);
    from_inside_the_base(game, |g| {
        place_machine(g, "extractor", ids::CORE_FRAGMENT, 1, 0)
    })
    .unwrap();
    find_structure_by_kind(game, "extractor").unwrap()
}

/// The `ProgramManifest` half of `entity`'s manifest. Most tests that reach
/// for a manifest only care about creature-side detail, and matching the
/// subject enum inline five times reads worse than naming the expectation.
pub(crate) fn program_manifest(game: &Game, entity: Entity) -> ProgramManifest {
    match game
        .manifest(entity)
        .expect("entity should have a manifest")
        .subject
    {
        ManifestSubject::Program(p) => *p,
        ManifestSubject::Player(_) => panic!("expected a program, got the player"),
    }
}

/// Spawns a bare `Nest` at `(x, y)` with no guardians — for
/// `pursuit_tick` tests that build their own hand-picked guardian set
/// rather than taking whatever `Game::spawn_nest`'s RNG-picked count and
/// placement roll. Built from the same `nest_components` (`game/spawning.rs`)
/// as `spawn_nest` and the save-load path, rather than a hand-copied
/// component list — this one used to hardcode `GlyphColor::Red` while a real
/// scrapper nest is `Yellow`, which is exactly the drift sharing the bundle
/// closes off.
pub(crate) fn spawn_bare_nest(game: &mut Game, x: i32, y: i32) -> Entity {
    let species = game
        .world
        .resource::<SpeciesDb>()
        .get("scrapper")
        .cloned()
        .expect("scrapper is a shipped species");
    game.world
        .spawn(spawning::nest_components(
            &species,
            x,
            y,
            NEST_DURABILITY,
            Vec::new(),
        ))
        .id()
}

/// Spawns a `NestGuardian` of `nest` at `(x, y)`, already `Pursuing` — the
/// state `Game::provoke_nest` would have left it in, for a test that needs
/// a pursuer in place without walking through an actual `attack_nest` call.
pub(crate) fn spawn_pursuing_guardian(
    game: &mut Game,
    nest: Entity,
    species: &str,
    x: i32,
    y: i32,
) -> Entity {
    game.world
        .spawn((
            Creature {
                species: species.to_string(),
            },
            Hostile,
            WanderAi::default(),
            NestGuardian { nest },
            Pursuing,
            Position { x, y },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 1,
                mitigation: 1,
            },
        ))
        .id()
}

/// `spawn_pursuing_guardian`'s sibling for Phase 7b's second tether.
pub(crate) fn spawn_pursuing_patrol(
    game: &mut Game,
    town: Entity,
    species: &str,
    x: i32,
    y: i32,
) -> Entity {
    game.world
        .spawn((
            Creature {
                species: species.to_string(),
            },
            Hostile,
            WanderAi::default(),
            crate::components::TownPatrol { town },
            Pursuing,
            Position { x, y },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 1,
                mitigation: 1,
            },
        ))
        .id()
}
