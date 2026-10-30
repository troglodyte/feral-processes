use super::*;
use crate::stack::{Dir, Frame};
use crate::tuning::MAX_ENEMY_GROUPS;
use crate::*;
use std::path::Path;

/// Mirror of the shipped starter-recipe costs in
/// `assets/items/{ice_breaker,power_cell}.ron` — the recipes are
/// data-driven now (see `Game::craft_recipes`), so these live here only
/// to keep the compile/discount tests asserting against a known number.
pub(crate) const ICE_BREAKER_CORE_COST: u32 = 3;

pub(crate) const POWER_CELL_CORE_COST: u32 = 2;

/// A shipped ability by id, for tests that call `Game::use_ability`
/// directly rather than going through a companion's menu.
pub(crate) fn ability(game: &Game, id: &str) -> crate::abilities::AbilityDef {
    game.world
        .resource::<crate::abilities::AbilityDb>()
        .get(id)
        .cloned()
        .unwrap_or_else(|| panic!("{id} ships with the game"))
}

pub(crate) const GENERIC_SPECIES_ID: &str = "test_generic";

/// The species `spawn_tamed` builds its companions from: a hand-written
/// fixture rather than whichever shipped species happens to declare no
/// abilities.
///
/// It used to be "first species by id with no declared abilities", which
/// resolved to construct and made **every** `spawn_tamed` companion carry
/// that species' data — so giving the roster kits would have taken the
/// `expect` out from under 233 call sites at once, and giving it affinities
/// silently multiplied every fixture's ability invocations by construct's. Both
/// are properties of the shipped roster leaking into a fixture that only
/// ever wanted a blank program.
///
/// Blank is the whole specification: no abilities (so
/// `install_innate_routines` yields `FALLBACK_ABILITY_ID`), neutral
/// affinities, and — load-bearing — **no habitats**, which is what keeps it
/// out of `habitat_matches`. That pool is indexed into by the spawn roll, so
/// a fixture species with a habitat would shift which species a seeded
/// `Game::new` spawns.
pub(crate) fn generic_species() -> SpeciesDef {
    SpeciesDef {
        id: GENERIC_SPECIES_ID.to_string(),
        name: "Test Generic".to_string(),
        glyph: '?',
        sprite: None,
        color: GlyphColor::White,
        base_hp: 40,
        base_atk: 4,
        base_mitigation: 2,
        resist: Default::default(),
        taming_difficulty: 0.5,
        habitats: Vec::new(),
        base_speed: crate::tuning::DEFAULT_BASE_SPEED,
        movement: None,
        moves: vec![crate::species::MoveDef {
            damage_type: None,
            name: "Test Strike".to_string(),
            power: 5,
            spread: 2,
            ranged: false,
            effect: None,
        }],
        work_resource: None,
        rich_in: None,
        equipment_drop: None,
        // Nothing authored, so a fixture body mints each attribute around
        // its own def's `base` — `generic_species`'s rule, that a fixture
        // must not shift meaning when a content file is edited.
        attributes: std::collections::BTreeMap::new(),
        is_boss: false,
        abilities: Vec::new(),
        growth_multiplier: crate::tuning::BASELINE_GROWTH_MULTIPLIER,
        affinities: crate::species::Affinities::NEUTRAL,
        taunts: Vec::new(),
        behaviour: None,
        can_nest: false,
        parents: None,
    }
}

pub(crate) const GENERIC_SETTLEMENT_ID: &str = "test_settlement";

/// A settlement definition for tests that need one materialized without
/// depending on the shipped catalogue — `generic_species`'s reason, one
/// level out: a fixture town must not shift meaning if a content file is
/// edited or a new one is added.
pub(crate) fn generic_settlement_def() -> crate::settlements::SettlementDef {
    crate::settlements::SettlementDef {
        id: GENERIC_SETTLEMENT_ID.to_string(),
        name: "Test Town".to_string(),
        blurb: "A place raised for a test.".to_string(),
        kind: crate::settlements::SettlementKind::Server,
        specialty: crate::settlements::Specialty::Gear,
        temperament: crate::settlements::Temperament::Open,
    }
}

/// Materializes a settlement at `(x, y)` directly — the `resources::
/// Settlements` record and one centre entity, without walking the region
/// derivation or building out the rest of its footprint, both of which have
/// their own coverage elsewhere. Deliberately one cell: most of this
/// fixture's callers place a wild program, a trap or the player at an exact
/// offset from the town and would read differently if a 3x3-or-larger square
/// of `Settlement` cells stood in the way. `SettlementCentre` still goes on
/// it, or `Game::field_patrol`'s `settlement_entity` lookup (and the load
/// path's `towns_by_tile`) would find nothing here. Despawns anything
/// already standing on the tile first, `ground_step`'s reason: a bump test
/// that landed on a wild program the seed happened to place there would find
/// the wrong thing.
pub(crate) fn place_settlement(
    game: &mut Game,
    key: crate::settlements::SettlementKey,
    x: i32,
    y: i32,
) -> Entity {
    let squatters: Vec<Entity> = {
        let mut q = game.world.query::<(Entity, &Position)>();
        q.iter(&game.world)
            .filter(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
            .collect()
    };
    for e in squatters {
        game.world.despawn(e);
    }
    // A key names one town, so a fixture that re-places one must take the
    // old entity with it. `ensure_local_settlements` never spawns a second
    // entity for a key it has already resolved; a fixture that left one
    // behind would be building a world the game cannot produce, and a
    // lookup by key would answer with whichever of the two bevy happened to
    // iterate first.
    let stale: Vec<Entity> = {
        let mut q = game
            .world
            .query::<(Entity, &crate::components::Settlement)>();
        q.iter(&game.world)
            .filter(|(_, settlement)| settlement.key == key)
            .map(|(e, _)| e)
            .collect()
    };
    for e in stale {
        game.world.despawn(e);
    }
    let def = generic_settlement_def();
    game.world
        .resource_mut::<crate::resources::Settlements>()
        .0
        .insert(
            key,
            crate::resources::KnownSettlement {
                tile: (x, y),
                def: def.clone(),
                visited: false,
            },
        );
    game.world
        .spawn((
            crate::components::Settlement { key },
            crate::components::SettlementCentre,
            Position { x, y },
            Glyph {
                ch: def.kind.glyph(),
                color: GlyphColor::Orange,
            },
        ))
        .id()
}

/// A tile far enough from the danger origin that a fight there may hold a
/// full `MAX_ENEMY_GROUPS` groups — `Game::max_enemy_groups` allows one in
/// zone 1 and gains another every zone after.
///
/// Any fixture asserting about more than one enemy group has to raise the
/// zone: both halves of the pack ceiling come from the zone and the depth
/// (see `Game::group_pack`), so a multi-group pack assembled in zone 1
/// partitions down to a single group wherever it stands. Returns the spawn
/// tile, which is as good as any — where the members are decides nothing.
pub(crate) fn multi_group_ground(game: &mut Game) -> (i32, i32) {
    game.world.resource_mut::<ZoneLevel>().0 = MAX_ENEMY_GROUPS as u32;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    (spawn.x + 500, spawn.y)
}

/// Sets `entity`'s level directly, for tests that need a level-gated
/// ability unlocked without grinding XP into it. Installs whatever species
/// unlocks that jump reaches, exactly as a real level-up would — otherwise
/// a test that raises a level would see a kit the game never leaves behind.
pub(crate) use crate::arena::set_level;

/// Puts the run in zone `zone` without running a breach. Enough for the
/// handful of tests that only care about a zone-gated *ceiling* — structure
/// upgrade tiers, gear level — rather than about anything `enter_next_zone`
/// does to the world. Reach for a real breach when the base moving matters.
pub(crate) fn set_zone(game: &mut Game, zone: u32) {
    game.world.resource_mut::<ZoneLevel>().0 = zone;
}

/// The shipped `AffixDb`, for a test that loads a `ResearchDb` over the
/// shipped tree — the Affixes tree is synthesised from it.
pub(crate) fn shipped_affix_db() -> crate::affixes::AffixDb {
    let assets = test_assets_dir();
    let (abilities, _) = crate::abilities::AbilityDb::load_dir(&assets.join("abilities")).unwrap();
    let (items, _) = crate::items_db::ItemDb::load_dir(&assets.join("items"), &abilities).unwrap();
    crate::affixes::AffixDb::load_dir(&assets.join("affixes"), &items)
        .unwrap()
        .0
}

pub(crate) fn test_assets_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

/// Where every fixture founds its base: the field's origin, so a test's base
/// cells and field cells are the same numbers the old base-space fixtures
/// used (ADR 0032 put the base on the field).
pub(crate) const TEST_HOME_CELL: (i32, i32) = (0, 0);

/// The fixture pocket `lay_test_pocket` floors: half-width and half-height
/// around `TEST_HOME_CELL`, chamfered by `TEST_POCKET_CORNER_CUT`. The 19x9
/// pocket a run used to open with, kept as test ground so a fixture has room
/// to stand machines without growing the floor tile by tile.
pub(crate) const TEST_POCKET_HALF_WIDTH: i32 = 9;
pub(crate) const TEST_POCKET_HALF_HEIGHT: i32 = 4;
pub(crate) const TEST_POCKET_CORNER_CUT: i32 = 2;

/// Every cell of the fixture pocket, row by row.
pub(crate) fn test_pocket_cells() -> Vec<(i32, i32)> {
    let (w, h) = (TEST_POCKET_HALF_WIDTH, TEST_POCKET_HALF_HEIGHT);
    let mut cells = Vec::new();
    for dy in -h..=h {
        for dx in -w..=w {
            if dx.abs() + dy.abs() <= w + h - TEST_POCKET_CORNER_CUT {
                cells.push((TEST_HOME_CELL.0 + dx, TEST_HOME_CELL.1 + dy));
            }
        }
    }
    cells
}

impl Game {
    /// Floors the fixture pocket (`test_pocket_cells`) — free, through the
    /// one floor door, whatever the field under it is.
    pub(crate) fn lay_test_pocket(&mut self) {
        self.dev_lay_floor(test_pocket_cells());
    }
}

/// Stands the player on `TEST_HOME_CELL` on the field — where a fixture's
/// Home stands — out of the Stack if they were in it, and floors the fixture
/// pocket first if nothing is laid there: standing in the base means standing
/// on its floor (`Game::on_base_floor`).
pub(crate) fn stand_in_base(game: &mut Game) {
    let floored = game
        .world
        .resource::<crate::base_grid::BaseGrid>()
        .is_floor(TEST_HOME_CELL.0, TEST_HOME_CELL.1);
    if !floored {
        game.lay_test_pocket();
    }
    stand_in_base_at(game, TEST_HOME_CELL.0, TEST_HOME_CELL.1);
}

/// Overrides field `cell` to a blocker (`Biome::DataVoid`), so a test that
/// needs rock there does not depend on the seed's terrain.
pub(crate) fn set_rock(game: &mut Game, cell: (i32, i32)) {
    game.world.resource_mut::<WorldMap>().set_override(
        cell.0,
        cell.1,
        Tile {
            biome: Biome::DataVoid,
            walkable: false,
            rock_shade: None,
        },
    );
}

/// A field cell well clear of the fixture pocket's floor.
pub(crate) const OFF_THE_FLOOR: (i32, i32) = (TEST_POCKET_HALF_WIDTH + 6, 0);

/// Stands the player on open ground off the base's floor — on the field,
/// but not home (`Game::on_base_floor` is false).
pub(crate) fn step_off_the_floor(game: &mut Game) {
    game.world.resource_mut::<WorldMap>().set_override(
        OFF_THE_FLOOR.0,
        OFF_THE_FLOOR.1,
        Tile {
            biome: Biome::OpenGrid,
            walkable: true,
            rock_shade: None,
        },
    );
    stand_in_base_at(game, OFF_THE_FLOOR.0, OFF_THE_FLOOR.1);
}

/// Runs `act` with the party standing at `TEST_HOME_CELL` on the field, and
/// puts them back where they were — locale and tile — afterwards.
///
/// For a fixture that has to *build* a base before a test about something
/// else entirely — a breach, a raid, a dive — measured from the Home's tile.
pub(crate) fn from_inside_the_base<T>(game: &mut Game, act: impl FnOnce(&mut Game) -> T) -> T {
    let outside = game.locale();
    let player = game.player_entity();
    let tile = *game.world.get::<Position>(player).unwrap();
    stand_in_base(game);
    let done = act(game);
    game.world.insert_resource(outside);
    *game.world.get_mut::<Position>(player).unwrap() = tile;
    done
}

/// Drops the party into depth 1 through an entrance on the tile they are
/// standing on, which is what walking onto a link does.
/// Puts `creature` in the party from inside base space, and comes back out
/// to wherever the party was standing.
///
/// Party assignment refuses in the Stack (`Game::require_field`), so a test
/// about a dive enlists from the field — `from_inside_the_base`'s reason one
/// verb along, spelled as a fixture rather than inlined 130 times.
pub(crate) fn enlist(game: &mut Game, creature: Entity) {
    from_inside_the_base(game, |g| g.add_companion(creature))
        .unwrap_or_else(|e| panic!("enlisting a companion: {e}"));
}

pub(crate) fn descend(game: &mut Game) {
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.enter_stack(pos.x, pos.y);
}

pub(crate) fn frame(game: &Game) -> Frame {
    game.world.resource::<CurrentStack>().0.clone().unwrap()
}

pub(crate) fn every_cell(level: &Frame) -> impl Iterator<Item = (i32, i32)> + use<> {
    let (w, h) = (level.width, level.height);
    (0..h).flat_map(move |y| (0..w).map(move |x| (x, y)))
}

/// Puts the party on `cell` facing `facing` without walking there. Caches
/// and orphans sit in dead ends, so reaching one honestly would mean
/// solving the maze first — `tests/stack.rs` teleports for the same reason.
pub(crate) fn stand_at(game: &mut Game, cell: (i32, i32), facing: Dir) {
    let Locale::Stack {
        depth,
        frames,
        entrance,
        ..
    } = game.locale()
    else {
        unreachable!("not underground")
    };
    game.world.insert_resource(Locale::Stack {
        depth,
        frames,
        x: cell.0,
        y: cell.1,
        facing,
        entrance,
    });
}
