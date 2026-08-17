//! Fixtures and helpers shared by the engine's unit tests.

use crate::game::spawning;
use crate::stack::{Dir, Frame};
use crate::tuning::{MAX_ENEMY_GROUPS, NEST_DURABILITY};
use crate::*;
use std::path::Path;

/// Mirror of the shipped starter-recipe costs in
/// `assets/items/{ice_breaker,power_cell}.ron` — the recipes are
/// data-driven now (see `Game::craft_recipes`), so these live here only
/// to keep the compile/discount tests asserting against a known number.
pub(super) const ICE_BREAKER_CORE_COST: u32 = 3;

pub(super) const POWER_CELL_CORE_COST: u32 = 2;

/// A shipped ability by id, for tests that call `Game::use_ability`
/// directly rather than going through a companion's menu.
pub(super) fn ability(game: &Game, id: &str) -> crate::abilities::AbilityDef {
    game.world
        .resource::<crate::abilities::AbilityDb>()
        .get(id)
        .cloned()
        .unwrap_or_else(|| panic!("{id} ships with the game"))
}

pub(super) const GENERIC_SPECIES_ID: &str = "test_generic";

/// The species `spawn_tamed` builds its companions from: a hand-written
/// fixture rather than whichever shipped species happens to declare no
/// abilities.
///
/// It used to be "first species by id with no declared abilities", which
/// resolved to construct and made **every** `spawn_tamed` companion carry
/// that species' data — so giving the roster kits would have taken the
/// `expect` out from under 233 call sites at once, and giving it affinities
/// silently multiplied every fixture's ability casts by construct's. Both
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
        color: GlyphColor::White,
        base_hp: 40,
        base_atk: 4,
        base_def: 2,
        taming_difficulty: 0.5,
        habitats: Vec::new(),
        base_speed: crate::tuning::DEFAULT_BASE_SPEED,
        base_int: crate::tuning::DEFAULT_BASE_INT,
        moves: vec![crate::species::MoveDef {
            name: "Test Strike".to_string(),
            power: 5,
            ranged: false,
            effect: None,
        }],
        work_resource: None,
        equipment_drop: None,
        is_boss: false,
        abilities: Vec::new(),
        growth_multiplier: crate::tuning::BASELINE_GROWTH_MULTIPLIER,
        affinities: crate::species::Affinities::NEUTRAL,
        taunts: Vec::new(),
        can_nest: false,
    }
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
pub(super) fn multi_group_ground(game: &mut Game) -> (i32, i32) {
    game.world.resource_mut::<ZoneLevel>().0 = MAX_ENEMY_GROUPS as u32;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    (spawn.x + 500, spawn.y)
}

/// Sets `entity`'s level directly, for tests that need a level-gated
/// ability unlocked without grinding XP into it. Installs whatever species
/// unlocks that jump reaches, exactly as a real level-up would — otherwise
/// a test that raises a level would see a kit the game never leaves behind.
pub(super) use crate::arena::set_level;

/// Puts the run in zone `zone` without running a breach. Enough for the
/// handful of tests that only care about a zone-gated *ceiling* — structure
/// upgrade tiers, gear level — rather than about anything `enter_next_zone`
/// does to the world. Reach for a real breach when the base moving matters.
pub(super) fn set_zone(game: &mut Game, zone: u32) {
    game.world.resource_mut::<ZoneLevel>().0 = zone;
}

pub(crate) fn test_assets_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

/// Drops the party into depth 1 through an entrance on the tile they are
/// standing on, which is what walking onto a link does.
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

/// Plans `action` for the player and a plain attack for every
/// companion, then resolves the round. The one-line stand-in for the
/// old single-action entry points, for tests that care about what a
/// round *does* rather than how it gets planned — the planning API has
/// its own tests.
pub(super) fn resolve_round_with(game: &mut Game, action: BattleAction) {
    let slots = game
        .world
        .get_resource::<BattleState>()
        .map(|b| b.planned.len())
        .unwrap_or(0);
    if game.battle_set_action(0, action).is_err() {
        return;
    }
    for slot in 1..slots {
        let _ = game.battle_set_action(slot, BattleAction::Attack { group: 0 });
    }
    game.battle_resolve_round();
}

pub(super) fn player_attacks(game: &mut Game) {
    resolve_round_with(game, BattleAction::Attack { group: 0 });
}

/// Plans the player's decompile as the Special it now is, and resolves the
/// round.
pub(super) fn player_decompiles(game: &mut Game) {
    let index = game
        .battle_special_options(0)
        .into_iter()
        .find(|o| o.name.to_lowercase().contains("decompile"))
        .expect("the player starts with decompile installed")
        .index;
    resolve_round_with(
        game,
        BattleAction::Special {
            ability: index,
            target: crate::battle::SpecialTarget::EnemyGroup { group: 0 },
        },
    );
}

/// Resolves a round in which `companion` uses its Special (the rally or
/// species ability that commanding it used to trigger) and everyone
/// else braces. Defend deals no damage, so anything that happens to the
/// enemy in such a round is attributable to the Special alone.
pub(super) fn companion_uses_special(
    game: &mut Game,
    companion: Entity,
    ability: usize,
    target: battle::SpecialTarget,
) {
    let slot = game
        .world
        .resource::<Party>()
        .0
        .iter()
        .position(|&e| e == companion)
        .map(|i| i + 1);
    let Some(slot) = slot else {
        return;
    };
    let slots = game
        .world
        .get_resource::<BattleState>()
        .map(|b| b.planned.len())
        .unwrap_or(0);
    for other in 0..slots {
        let action = if other == slot {
            BattleAction::Special { ability, target }
        } else {
            BattleAction::Defend
        };
        let _ = game.battle_set_action(other, action);
    }
    game.battle_resolve_round();
}

/// Starts a battle against `enemies`, partitioned into species groups
/// exactly as `start_battle` does. Tests that use this build their
/// combatants by hand to pin down the precise stats a case needs, which
/// `start_battle`'s own spawn path can't express — this keeps them
/// saying "these programs are in the fight" without restating the
/// partition, and without the opening log line.
pub(super) fn insert_battle(game: &mut Game, player: Entity, enemies: Vec<Entity>) {
    let groups = game.group_pack(enemies);
    let slots = game.world.resource::<Party>().0.len() + 1;
    game.world.insert_resource(BattleState {
        player,
        round_targets: groups.iter().map(|g| g.members.clone()).collect(),
        groups,
        round: 1,
        planned: vec![None; slots],
        finished: false,
        player_won: false,
        decompile_attempts: std::collections::HashMap::new(),
        rewards: BattleRewards::default(),
        lair: None,
    });
}

/// Spawns `count` hostile members of one species into a single group and
/// starts a battle against them, so back-rank indices actually exist.
/// Stats are set by hand rather than rolled, because these tests assert on
/// exact HP.
///
/// Placed deep and far on purpose: a group's size ceiling is the local
/// `max_group_size`, which at a zone-1 spawn point is one member — there
/// would be no back rank to test. The hand-set stats are what make the move
/// free, since nothing here reads the distance or zone scaling it implies.
pub(super) fn battle_with_a_pack_of(game: &mut Game, count: usize, hp: i32) -> Vec<Entity> {
    let player = game.player_entity();
    let species = game
        .species_defs()
        .into_iter()
        .next()
        .expect("at least one species");
    game.world.resource_mut::<ZoneLevel>().0 = 3;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let (x, y) = (spawn.x + 500, spawn.y);
    let members: Vec<Entity> = (0..count)
        .map(|i| {
            game.world
                .spawn((
                    Creature {
                        species: species.id.clone(),
                    },
                    Hostile,
                    Position { x: x + i as i32, y },
                    Stats {
                        hp,
                        max_hp: hp,
                        atk: 0,
                        def: 0,
                    },
                    StatusEffects::default(),
                ))
                .id()
        })
        .collect();
    insert_battle(game, player, members.clone());
    members
}

/// A scratch asset install that deletes itself when it goes out of scope.
///
/// Cleanup **must** be a guard rather than a `remove_dir_all` at the end of
/// each test. Every one of these holds a full copy of the shipped assets —
/// eight directories, ~190 files — so a test that panics on a failed assert
/// leaks the lot, and a helper that returns only its `Game` had no way to
/// clean up at all. Between them those two shapes put 5,437 stale installs
/// in `/tmp` and exhausted the filesystem's *inode* table (the tmpfs was
/// only 15% full by bytes), which fails builds and tests across the whole
/// machine with an error naming none of this.
///
/// Dropping it is safe as early as the `Game` is built: `Game` holds a
/// `World` and a `Schedule` and does not retain the assets path, so a
/// helper returning a bare `Game` can let its guard fall at the end of the
/// function. Keep the binding alive only where the *path* is used again,
/// which is what a second `Game::new` or a `Game::load` against the same
/// install needs.
pub(super) struct ScratchAssets(std::path::PathBuf);

impl std::ops::Deref for ScratchAssets {
    type Target = std::path::Path;

    fn deref(&self) -> &std::path::Path {
        &self.0
    }
}

impl AsRef<std::path::Path> for ScratchAssets {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for ScratchAssets {
    fn drop(&mut self) {
        // Best-effort: a test asserting on a failed install may have left
        // the directory in a state `remove_dir_all` dislikes, and turning
        // that into a second panic during unwinding would abort the process
        // and bury the assertion that actually failed.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A fresh, uniquely-named scratch directory under the OS temp dir, wiped
/// if a stale one from a crashed prior run is somehow still there. Shared
/// by `modded_assets_dir`, `assets_dir_with_extra_structure` and the policy
/// tests (which want a scratch *path* and no assets at all) so they all
/// draw from one counter — collisions were never really possible (`tag`
/// already disambiguates by caller) but there's no reason to run two.
///
/// The directory is deliberately not created: a caller that wants one calls
/// `copy_shipped_assets` or `create_dir_all` itself, and a caller testing an
/// absent file wants the path to stay absent.
pub(super) fn scratch_assets_dir(tag: &str) -> ScratchAssets {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "feral_processes_{tag}_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    ScratchAssets(dir)
}

/// Copies every shipped asset file into `dir` (which must already exist),
/// skipping the item files named in `omit_items`. The shared body of
/// `modded_assets_dir` and `assets_dir_with_extra_structure` — both need
/// the whole shipped set present so a scratch install still passes
/// `Game::new`'s missing-role startup check.
pub(super) fn copy_shipped_assets(dir: &std::path::Path, omit_items: &[&str]) {
    let shipped = test_assets_dir();
    for sub in [
        "species",
        "structures",
        "research",
        "items",
        "abilities",
        "perks",
        "achievements",
        "descriptions",
        // The trained enemy policy comes along too, or a modded install
        // would quietly fight under the uniform baseline while the shipped
        // one fought under the weights — a difference no test would name.
        "policies",
        // Same argument: without these every zone past the first would
        // generate at the neutral shape, so a modded install would be a
        // different world from the shipped one for a reason nothing said
        // out loud. `assets_dir_with_sectors` is how a test asks for a
        // *different* pool, including an empty one.
        "sectors",
    ] {
        let dst = dir.join(sub);
        std::fs::create_dir_all(&dst).unwrap();
        for entry in std::fs::read_dir(shipped.join(sub)).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            if sub == "items" && omit_items.contains(&name.to_str().unwrap_or_default()) {
                continue;
            }
            std::fs::copy(entry.path(), dst.join(name)).unwrap();
        }
    }
}

/// Copies the shipped `species`/`structures`/`research`/`items`/`abilities`
/// asset dirs into a fresh scratch dir, skipping the item files named in
/// `omit_items` and writing the `extra_*` (filename, RON body) pairs on
/// top — a stand-in for a modded install. The caller removes the
/// directory once its `Game` is done with it.
pub(super) fn modded_assets_dir(
    tag: &str,
    omit_items: &[&str],
    extra_items: &[(&str, &str)],
    extra_species: &[(&str, &str)],
    extra_research: &[(&str, &str)],
    extra_abilities: &[(&str, &str)],
) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    copy_shipped_assets(&dir, omit_items);
    for (name, body) in extra_items {
        std::fs::write(dir.join("items").join(name), body).unwrap();
    }
    for (name, body) in extra_species {
        std::fs::write(dir.join("species").join(name), body).unwrap();
    }
    for (name, body) in extra_research {
        std::fs::write(dir.join("research").join(name), body).unwrap();
    }
    for (name, body) in extra_abilities {
        std::fs::write(dir.join("abilities").join(name), body).unwrap();
    }
    dir
}

/// Like `modded_assets_dir`, but for the one existing test that needs a
/// modded *structure* — none of `modded_assets_dir`'s five callers-so-far
/// have needed one, and widening its signature for a single caller isn't
/// worth the churn across its other ~20 call sites. Cleanup is the
/// `ScratchAssets` guard's, not the caller's.
/// A scratch install whose `sectors/` directory holds exactly `files` and
/// nothing else.
///
/// Two things need this, and they are opposite ends of the same question.
/// `&[]` gives an install with **no** sectors, which is the pre-sector game
/// and the only way to assert that absence is still supported. A single file
/// gives an install where every zone past the first is *that* sector, which
/// takes the derivation out of the picture when what is under test is the
/// wiring — `tests::sectors` already covers which sector a zone gets.
pub(super) fn assets_dir_with_sectors(tag: &str, files: &[(&str, &str)]) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    copy_shipped_assets(&dir, &[]);
    let sectors = dir.join("sectors");
    std::fs::remove_dir_all(&sectors).unwrap();
    std::fs::create_dir_all(&sectors).unwrap();
    for (name, body) in files {
        std::fs::write(sectors.join(name), body).unwrap();
    }
    dir
}

pub(super) fn assets_dir_with_extra_structure(tag: &str, name: &str, body: &str) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    copy_shipped_assets(&dir, &[]);
    std::fs::write(dir.join("structures").join(name), body).unwrap();
    dir
}

/// A scratch install carrying one extra item *and* one extra structure —
/// the pair a modded machine needs, since a machine runs its product's own
/// `craftable.cost` and there is no recipe on the structure to write
/// instead.
pub(super) fn assets_dir_with_extra_machine(
    tag: &str,
    item: (&str, &str),
    structure: (&str, &str),
) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    copy_shipped_assets(&dir, &[]);
    std::fs::write(dir.join("items").join(item.0), item.1).unwrap();
    std::fs::write(dir.join("structures").join(structure.0), structure.1).unwrap();
    dir
}

/// A scratch install carrying one extra achievement on top of the shipped
/// ladder, for cases that need a rung the real assets deliberately don't have.
pub(super) fn scratch_assets_with_achievement(id: &str, body: &str) -> ScratchAssets {
    let dir = scratch_assets_dir(id);
    copy_shipped_assets(&dir, &[]);
    std::fs::write(dir.join("achievements").join(format!("{id}.ron")), body).unwrap();
    dir
}

/// A modded install missing `core_fragment.ron` — the item that holds
/// the Currency economy role — so `Game::new`'s missing-role startup
/// abort (see `ItemDb::missing_roles`) can be exercised against an
/// otherwise-valid item set.
pub(super) fn assets_dir_missing_currency_item() -> ScratchAssets {
    modded_assets_dir(
        "missing_currency",
        &["core_fragment.ron"],
        &[],
        &[],
        &[],
        &[],
    )
}

/// Gives the player `n` Research Data, bypassing the Research Node so
/// the test doesn't depend on tick timing or a tamed worker.
pub(super) fn grant_research_data(game: &mut Game, n: u32) {
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::RESEARCH_DATA), n);
}

/// Deploys a Data Cache next to the player without going through
/// `place_structure`, sidestepping its Home/cost/radius requirements —
/// those aren't what the capacity tests are about.
pub(super) fn spawn_data_cache(game: &mut Game, offset: i32) {
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.world.spawn((
        Structure {
            kind: "data_cache".to_string(),
        },
        Position {
            x: pos.x + offset,
            y: pos.y,
        },
    ));
}

/// Deploys a structure of `kind` at an absolute position, bypassing
/// `place_structure`'s Home, cost and distance rules — for tests about what
/// a standing structure *enables*, not about the build rules.
pub(super) fn spawn_structure_at(game: &mut Game, kind: &str, x: i32, y: i32) {
    game.world.spawn((
        Structure {
            kind: kind.to_string(),
        },
        Position { x, y },
    ));
}

/// Unlocks `id` and every prerequisite it needs, funding the whole
/// chain — so a test that just needs a research-gated structure on the
/// map doesn't have to model the tree itself.
///
/// Since the tree gained zone bands (`ResearchDef::min_zone`), funding is no
/// longer enough on its own, so this also stands the party in the deepest
/// zone the chain asks for. That is a direct `ZoneLevel` write rather than a
/// real breach deliberately: `enter_next_zone` regenerates the map and
/// respawns the wild population, which is a great deal of world change to
/// impose on a fixture that only wanted a bench researched. Nothing is lost
/// by the shortcut, because the tests that are *about* the gate
/// (`breaching_makes_a_zone_gated_node_available`) breach for real, so a
/// gate regression cannot hide behind this.
pub(super) fn unlock_research_chain(game: &mut Game, id: &str) {
    fn order(game: &Game, id: &str, out: &mut Vec<String>) {
        let Some(def) = game.world.resource::<ResearchDb>().get(id).cloned() else {
            return;
        };
        for req in &def.requires {
            order(game, req, out);
        }
        if !out.contains(&def.id) {
            out.push(def.id);
        }
    }
    grant_research_data(game, 1000);
    let mut chain = Vec::new();
    order(game, id, &mut chain);
    let needed = chain
        .iter()
        .filter_map(|node| game.world.resource::<ResearchDb>().get(node))
        .map(|def| def.min_zone)
        .max()
        .unwrap_or(0);
    let zone = &mut game.world.resource_mut::<ZoneLevel>().0;
    *zone = (*zone).max(needed);
    for node in chain {
        if !game.is_researched(&node) {
            game.unlock_research(&node).unwrap();
        }
    }
}

/// Reads the bank directly rather than through `PlayerStatus::inventory`,
/// which deliberately omits banked items — see `Game::banked`.
pub(super) fn research_data_held(game: &Game) -> u32 {
    game.banked(&ItemId::from(ids::RESEARCH_DATA))
}

/// Tames a program and puts it to work on a node producing `resource`,
/// so a cronjob is guaranteed to be running — the assertions below are
/// vacuous if nothing is assigned. Returns the node, since a cronjob's
/// output lands in its own buffer rather than the player's cargo.
///
/// The buffer is deliberately roomy: a test about whether a cronjob runs at
/// all should not be measuring how fast it clogs.
pub(super) fn assign_worker_producing(game: &mut Game, resource: ItemId) -> Entity {
    let worker = spawn_tamed(game, 10, 3);
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "test_node".to_string(),
            },
            Position { x: 3, y: 4 },
            ResourceNode {
                resource,
                level: None,
            },
            Stock::new(10_000),
            MachineStatus::default(),
        ))
        .id();
    stand_player_at_post(game, structure);
    game.assign_cronjob(worker, structure).unwrap();
    structure
}

/// The two components `task_progress_system` needs on a node beyond
/// `ResourceNode` itself. A node missing either is skipped by that system's
/// query and silently produces nothing — which reads as a payout curve that
/// moved rather than as a fixture that is short a component, so bundle them
/// rather than leaving each fixture to remember.
///
/// The buffer is deliberately far past any one cycle's payout: a test about
/// what a cycle is worth should not be measuring how fast the node clogs.
pub(super) fn work_node_parts() -> (Stock, MachineStatus) {
    (Stock::new(10_000), MachineStatus::default())
}

/// Deploys a working machine of `kind` at an absolute tile, with every
/// component the def implies — `Stock` sized from the def, a
/// `MachineStatus`, and a `ResourceNode` if it extracts.
///
/// The difference from `spawn_structure_at`, which spawns a bare
/// `Structure` and `Position`: a chain test needs machines that can
/// actually hold and pull stock, and a node short of `Stock` or
/// `MachineStatus` is skipped by `task_progress_system`'s query and
/// silently produces nothing.
pub(super) fn spawn_machine_at(game: &mut Game, kind: &str, x: i32, y: i32) -> Entity {
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == kind)
        .unwrap_or_else(|| panic!("{kind} should be a shipped structure"));
    let mut entity = game.world.spawn((
        Structure {
            kind: def.id.clone(),
        },
        Position { x, y },
        Stock::new(def.capacity),
        MachineStatus::default(),
    ));
    if let Some(work) = &def.work {
        entity.insert(ResourceNode {
            resource: work.produces.clone(),
            level: work.level,
        });
    }
    entity.id()
}

/// Stands the player on `(x, y)`.
///
/// `assign_cronjob` starts a posted program from the player's tile, so this
/// is how a fixture decides whether a new cronjob has a walk to make: stand
/// away from the machine and the worker walks in, stand at it and the worker
/// is already at its post. It is the player's position that carries that
/// distance now, not the worker's.
pub(super) fn stand_player_at(game: &mut Game, x: i32, y: i32) {
    let player = game.player_entity();
    let mut pos = game.world.get_mut::<Position>(player).unwrap();
    pos.x = x;
    pos.y = y;
}

/// Stands the player at the post east of `structure`, so a cronjob assigned
/// next starts its program already at the machine.
pub(super) fn stand_player_at_post(game: &mut Game, structure: Entity) {
    let target = *game.world.get::<Position>(structure).unwrap();
    stand_player_at(game, target.x + 1, target.y);
}

/// Stands `worker` on the tile east of `structure` — a post it can work
/// from.
///
/// Only meaningful *after* `assign_cronjob`, which starts a program from the
/// player's tile and would overwrite this — use `stand_player_at_post`
/// before an assignment and this one after.
///
/// A posted program produces nothing until it is orthogonally adjacent to
/// its machine (`task_progress_system`'s `Unstaffed` gate), and it gets
/// there by walking, which takes ticks a fixture usually doesn't want to
/// spend. Any test measuring what a cronjob *produces* rather than how a
/// program gets to work should start it here.
pub(super) fn park_at_post(game: &mut Game, worker: Entity, structure: Entity) {
    let target = *game.world.get::<Position>(structure).unwrap();
    let mut pos = game.world.get_mut::<Position>(worker).unwrap();
    pos.x = target.x + 1;
    pos.y = target.y;
}

/// How many of `item` are sitting in `structure`'s output buffer.
pub(super) fn node_output(game: &Game, structure: Entity, item: &str) -> u32 {
    game.world
        .get::<Stock>(structure)
        .and_then(|s| s.output.get(&ItemId::from(item)).copied())
        .unwrap_or(0)
}

/// Deploys a Home just off the player's current position (`dx`, `dy`
/// relative, so it doesn't collide with whatever the caller places
/// next) — `place_structure` refuses anything else until a Home
/// exists, so most structure-placement tests need this first.
pub(super) fn place_home(game: &mut Game, dx: i32, dy: i32) {
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 5);
    game.place_structure("home", dx, dy).unwrap();
}

/// How many of `id` the player is holding.
pub(super) fn count_item(game: &Game, id: &str) -> u32 {
    let player = game.player_entity();
    game.world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(id))
}

pub(super) fn run_one_full_gather_cycle(game: &mut Game, resource: &str) -> u32 {
    run_one_full_gather_cycle_at_tier(game, "mining_node", resource, None)
}

/// Runs exactly one completed gather cycle against a hand-built node of
/// `kind` producing `resource` at `tier`, and returns how many units landed
/// in the player's inventory.
///
/// `kind` is a real structure id rather than a placeholder because the
/// payout consults that structure's `WorkDef` for `flat_payout` — a node
/// whose kind isn't in the `StructureDb` would silently take the scaling
/// branch.
///
/// `level: None` on the node means it always yields (see
/// `systems::mining_success_chance`), which is what keeps the payout
/// assertions off the RNG entirely.
///
/// Measured across *both* places a cycle can pay into — the node's own
/// buffer for ordinary salvage, and the player's bank for a banked resource
/// (see `systems::deliver_payout`) — so this helper keeps answering the one
/// question it is for, "how much did a cycle pay", whichever kind of
/// resource the caller asked about. The buffer is sized far past any one
/// cycle's payout so a clog can never be mistaken for a payout curve that
/// moved.
pub(super) fn run_one_full_gather_cycle_at_tier(
    game: &mut Game,
    kind: &str,
    resource: &str,
    tier: Option<u32>,
) -> u32 {
    let worker = spawn_tamed(game, 10, 3);
    let mut structure = game.world.spawn((
        Structure {
            kind: kind.to_string(),
        },
        Position { x: 3, y: 4 },
        ResourceNode {
            resource: ItemId::from(resource),
            level: None,
        },
        Stock::new(10_000),
        MachineStatus::default(),
    ));
    if let Some(t) = tier {
        structure.insert(StructureTier(t));
    }
    let structure = structure.id();
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 0,
        required: 1,
    });

    let item = ItemId::from(resource);
    let paid = |game: &Game| node_output(game, structure, resource) + held(game, &item);
    let before = paid(game);
    game.tick();
    paid(game) - before
}

pub(super) fn find_structure_by_kind(game: &mut Game, kind: &str) -> Option<Entity> {
    let mut query = game.world.query::<(Entity, &Structure)>();
    query
        .iter(&game.world)
        .find(|(_, s)| s.kind == kind)
        .map(|(e, _)| e)
}

/// The initial world spawns wild creatures scattered around the player, so
/// directional-inspect tests clear whatever landed along their search ray
/// first — otherwise they'd be at the mercy of the seed's RNG instead of
/// testing the method itself.
pub(super) fn clear_creatures_east_of_player(game: &mut Game, start: Position, range: i32) {
    // Exactly the row `Game::find_target_in_direction` reads, and no wider.
    // It used to clear a 90° cone, matching the scan when the scan was one;
    // a cone-shaped cleanup for a ray-shaped read despawns creatures no test
    // could have seen and would quietly hide a widening of the ray.
    let stale: Vec<Entity> = {
        let mut query = game.world.query::<(Entity, &Position, &Creature)>();
        query
            .iter(&game.world)
            .filter(|(_, pos, _)| {
                let (ddx, ddy) = (pos.x - start.x, pos.y - start.y);
                ddy == 0 && ddx >= 1 && ddx <= range
            })
            .map(|(e, ..)| e)
            .collect()
    };
    for e in stale {
        game.world.despawn(e);
    }
}

/// Replaces the player's whole inventory with `stock`, so a taming test
/// states exactly which catalysts are on hand instead of inheriting
/// whatever `Game::new`'s starting kit holds.
pub(super) fn set_inventory(game: &mut Game, stock: &[(&str, u32)]) {
    let player = game.player_entity();
    let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
    inv.items.clear();
    for (id, qty) in stock {
        inv.add(ItemId::from(*id), *qty);
    }
}

/// Retries `battle_flee` until the escape roll lands, for the tests that
/// are about what a *successful* jack-out does rather than about the roll.
/// Jacking out stopped being guaranteed (see `battle::jack_out_chance`), so
/// a single call is no longer enough to end a battle.
///
/// Bounded rather than looping forever: the chance floors at
/// `JACK_OUT_CHANCE_MIN`, so 200 straight failures is a ~1e-9 event and
/// means something is actually broken. Bails out if the battle ends under
/// it — a failed attempt draws a full volley, which can flatline the party.
pub(super) fn flee_until_clear(game: &mut Game) {
    for _ in 0..200 {
        if game.battle_flee() {
            return;
        }
        if !game.has_active_battle() {
            return;
        }
    }
    panic!("200 jack-out attempts all failed — the escape roll is broken");
}

/// Dives the party from the surface to `depth` through the real descent
/// path — an entrance under the player's feet, then one `Game::descend` per
/// frame — teleporting only *within* a frame to stand on its way down, which
/// is the one thing a test can't be asked to walk a maze for.
///
/// `Locale` is never hand-written into a different depth: `enter_frame` is
/// the one way into a frame and a test that skipped it would be asserting
/// against a world no player can reach.
/// The entrance chosen is far enough from the zone's spawn point that its
/// stack actually runs `depth` frames deep — `stack::frames_for` grows the
/// count with distance, and the shallow stacks beside the spawn have no
/// bottom to reach.
pub(super) fn dive_to_depth(game: &mut Game, depth: u32) {
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let entrance = (0..500)
        .map(|step| (pos.x + step, pos.y))
        .find(|&(x, y)| {
            game.frames_at((x, y)) >= depth
                && game.world.resource_mut::<WorldMap>().tile(x, y).walkable
        })
        .expect("somewhere along this row there should be a stack deep enough");
    game.enter_stack(entrance.0, entrance.1);
    for _ in 1..depth {
        stand_on_link_down(game);
        game.descend();
    }
    let Locale::Stack { depth: reached, .. } = game.locale() else {
        panic!("the dive should have left the party underground");
    };
    assert_eq!(reached, depth, "the dive did not reach the asked-for depth");
}

/// Teleports the party onto the current frame's way down, so a test about
/// descending doesn't have to walk the maze to find it.
fn stand_on_link_down(game: &mut Game) {
    let down = game
        .world
        .resource::<CurrentStack>()
        .0
        .as_ref()
        .unwrap()
        .link_down
        .expect("every frame a test dives through should have a way down");
    let Locale::Stack {
        depth,
        frames,
        facing,
        entrance,
        ..
    } = game.locale()
    else {
        unreachable!("not underground")
    };
    game.world.insert_resource(Locale::Stack {
        depth,
        frames,
        x: down.0,
        y: down.1,
        facing,
        entrance,
    });
}

/// Spawns a wild program on the player's tile and opens an intrusion on
/// it — the state `battle_decompile` needs.
pub(super) fn start_battle_with_a_wild_program(game: &mut Game) -> Entity {
    let wild = spawn_wild_on_player_tile(game);
    game.start_battle(vec![wild]);
    wild
}

/// `Game::spawn_wild_creature` through `species`, with whatever routine the
/// `WILD_ROUTINE_CHANCE` roll gave it immediately cleared — for a fixture
/// that asserts on enemy *move* behaviour and can't tolerate the roll
/// occasionally handing it a carrier instead. A carrier's routine acting
/// differently from a move is loud, not silent (a kill or damage assertion
/// fails rather than passes), but loud is still a mystery failure in an
/// unrelated test the day the RNG stream shifts.
pub(super) fn spawn_wild_without_routine(game: &mut Game, species: &str, x: i32, y: i32) -> Entity {
    let entity = game
        .spawn_wild_creature(species, x, y)
        .unwrap_or_else(|| panic!("{species} ships with the game"));
    game.world.entity_mut(entity).insert(Routines::default());
    entity
}

pub(super) fn spawn_tamed(game: &mut Game, hp: i32, atk: i32) -> Entity {
    let player = game.player_entity();
    let species = generic_species();
    let entity = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Position { x: 3, y: 3 },
            Stats {
                hp,
                max_hp: hp,
                atk,
                def: 1,
            },
            Tamed { owner: player },
            Experience::default(),
        ))
        .id();
    game.install_innate_routines(entity);
    entity
}

/// `spawn_tamed` at a chosen tile and carrying a `Glyph`, so `view_entities`
/// can see it.
///
/// That query is `(Entity, &Position, &Glyph)`, and `spawn_tamed` grants no
/// glyph — a program spawned by it is invisible to every map-facing view,
/// which reads as the view filtering it out rather than as the fixture
/// being short a component.
pub(super) fn spawn_tamed_on_map(game: &mut Game, x: i32, y: i32) -> Entity {
    let entity = spawn_tamed(game, 10, 3);
    game.world.entity_mut(entity).insert(Glyph {
        ch: 'd',
        color: GlyphColor::Cyan,
    });
    let mut pos = game.world.get_mut::<Position>(entity).unwrap();
    pos.x = x;
    pos.y = y;
    entity
}

/// Spawns a minimal wild (untamed, `Hostile`) `Creature` on the
/// player's own tile, suitable to pass straight into `start_battle` —
/// mirrors `spawn_tamed`'s pattern but without `Tamed`/`Experience`,
/// since a wild pack member has neither.
pub(super) fn spawn_wild_on_player_tile(game: &mut Game) -> Entity {
    let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .next()
        .expect("at least one species");
    game.world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position {
                x: player_pos.x,
                y: player_pos.y,
            },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 0,
                def: 1,
            },
        ))
        .id()
}

/// The wild-boss counterpart of `spawn_wild_on_player_tile`, for the tests
/// that assert a boss can never reach the roster. Its stats come from the
/// real species rather than the token 10 HP above, because `atk: 0` is
/// exactly what stops those tests noticing if a refused round resolves
/// anyway.
pub(super) fn spawn_boss_on_player_tile(game: &mut Game) -> Entity {
    let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let boss = game
        .species_defs()
        .into_iter()
        .find(|s| s.is_boss)
        .expect("at least one boss species should ship in assets/species");
    game.spawn_wild_creature(&boss.id, player_pos.x, player_pos.y)
        .expect("a shipped boss species should spawn")
}

/// Opens a lair fight in the frame the party is standing in, against a
/// guardian the game will actually let the player decompile.
///
/// A real `rouse_lair` cannot produce one: every walkable biome fields a
/// boss (`every_biome_a_stack_link_can_open_in_fields_a_boss`) and a boss
/// is refused as a decompile target before the roll. What is left is
/// `pick_lair_species`'s fallback — the toughest *ordinary* program a
/// biome with no boss can field, which a mod can reach and which carries
/// no `is_boss` to refuse on. Installing that case by hand is cheaper than
/// a bossless install and names the same state `rouse_lair` writes: the
/// pack in the fight, `StackSpawn` on it, and `BattleState::lair` pointing
/// at the guardian.
///
/// Softened to 1 HP, so a test about what a capture *does* is not also a
/// test of whether the roll lands.
pub(super) fn rouse_a_tameable_guardian(game: &mut Game) -> Entity {
    let player = game.player_entity();
    let pos = game
        .stack_pos()
        .expect("a lair fight belongs to a frame — descend first");
    let tile = *game.world.get::<Position>(player).unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss)
        .expect("the shipped roster is not all bosses");
    let guardian = game
        .spawn_wild_creature(&species.id, tile.x, tile.y)
        .expect("a shipped species should spawn");
    game.world.entity_mut(guardian).insert(StackSpawn);
    game.world.get_mut::<Stats>(guardian).unwrap().hp = 1;
    insert_battle(game, player, vec![guardian]);
    game.world.resource_mut::<BattleState>().lair =
        Some(crate::resources::LairFight { pos, guardian });
    guardian
}

/// Deploys a Home directly on the player's current tile — `Game::rest`
/// requires a rest-enabling structure nearby, so tests exercising `rest`
/// need one in place first. Spawned directly rather than through
/// `place_structure` to sidestep its cost and one-Home-only
/// requirements, which aren't what these tests are about.
pub(super) fn spawn_rest_structure_at_player(game: &mut Game) {
    let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.world.spawn((
        Structure {
            kind: "home".to_string(),
        },
        Position {
            x: player_pos.x,
            y: player_pos.y,
        },
    ));
}

/// Sets up a single-round battle with one companion (stunned or not)
/// and returns how much the player's Power dropped from commanding
/// it. Shared by the two cost tests below.
pub(super) fn power_spent_commanding_companion(seed: u32, stunned: bool) -> f32 {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 20);
    game.add_companion(companion).unwrap();
    if stunned {
        game.world.entity_mut(companion).insert(StatusEffects {
            active: Some(ActiveStatus {
                kind: StatusKind::Stun,
                remaining: 1,
                power: 0,
                landed_this_round: false,
            }),
        });
    }

    let species = game
        .species_defs()
        .into_iter()
        .next()
        .expect("at least one species");
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 5, y: 5 },
            Stats {
                hp: 100,
                max_hp: 100,
                atk: 0,
                def: 0,
            },
            StatusEffects::default(),
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);

    // Start off the cap. Power drains per tick, and both arms
    // of the comparison are supposed to absorb one tick's worth identically
    // — which they only do if neither is clamped at either end.
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(50.0);
    let power_before = game.world.get::<PowerReserve>(player).unwrap().get();
    companion_uses_special(
        &mut game,
        companion,
        0,
        battle::SpecialTarget::Ally { slot: 0 },
    );
    let power_after = game.world.get::<PowerReserve>(player).unwrap().get();
    power_before - power_after
}

/// A `Creature`-scoped `FieldBuff` ability — field-only, so it must never
/// surface as something a battle picker or a wild carrier can spend a round
/// on. Shared between the picker-filtering and wild-retaliation tests
/// rather than defined twice.
pub(super) const FIELD_ONLY_ABILITY: &str = r#"(
    id: "test_field_regen",
    name: "Test Field Regen",
    description: "d",
    target: OneAlly,
    power_cost: 5.0,
    effect: FieldBuff(kind: Regen, power: 2, duration: 20),
)"#;

/// A `Run`-scoped `FieldBuff` — `field_buff_target_mismatch` requires
/// `target: WholeParty` on this scope even though `Game::cast_field_routine`
/// ignores it and always lands the buff on the player alone. For the test
/// asserting that a `Run`-scoped routine held by a companion still lands on
/// the player, not the companion.
pub(super) const FIELD_ONLY_RUN_ABILITY: &str = r#"(
    id: "test_field_trickle",
    name: "Test Field Trickle",
    description: "d",
    target: WholeParty,
    power_cost: 4.0,
    effect: FieldBuff(kind: Trickle, power: 3, duration: 15),
)"#;

/// A `Creature`-scoped, `WholeParty`-targeted `FieldBuff` — for the test
/// asserting a cast arms every living party member (and skips a dead one)
/// rather than just the caster.
pub(super) const FIELD_ONLY_PARTY_ABILITY: &str = r#"(
    id: "test_field_def",
    name: "Test Field Def",
    description: "d",
    target: WholeParty,
    power_cost: 3.0,
    effect: FieldBuff(kind: Def, power: 4, duration: 10),
)"#;

/// A `Creature`-scoped, percentage-magnitude `FieldBuff` — `Mitigation` is
/// the one percentage kind with a real `affinity_kind` (`Buff`, the same
/// category `FIELD_ONLY_PARTY_ABILITY`'s `Def` uses), so casting the two off
/// the same high-level, high-affinity holder is what proves
/// `FieldBuffKind::scales_with_caster` actually splits them: `Def` scales,
/// `Mitigation` lands at exactly its authored value either way.
pub(super) const FIELD_ONLY_MITIGATION_ABILITY: &str = r#"(
    id: "test_field_mitigation",
    name: "Test Field Mitigation",
    description: "d",
    target: WholeParty,
    power_cost: 5.0,
    effect: FieldBuff(kind: Mitigation, power: 10, duration: 20),
)"#;

/// A species declaring two abilities, so the multi-ability paths can be
/// exercised without depending on shipped kit assignments. The second is
/// gated above a fresh companion's level 1, which is what pins down
/// `Game::actor_abilities`' level filtering.
pub(super) const TWO_ABILITY_SPECIES: &str = r#"(
    id: "test_medic",
    name: "Test Medic",
    glyph: 'm',
    color: Cyan,
    base_hp: 10,
    base_atk: 4,
    base_def: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    base_speed: 10,
    moves: [(name: "Poke", power: 3)],
    abilities: [
        (id: "hot_patch"),
        (id: "sandbox", level: 5),
    ],
)"#;

/// Spawns a tamed member of `TWO_ABILITY_SPECIES` into the party of a
/// game built on a modded install that ships it.
pub(super) fn game_with_two_ability_companion() -> (Game, Entity) {
    let dir = modded_assets_dir(
        "two_ability_species",
        &[],
        &[],
        &[("test_medic.ron", TWO_ABILITY_SPECIES)],
        &[],
        &[],
    );
    let mut game = Game::new(94, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    let medic = game
        .world
        .spawn((
            Creature {
                species: "test_medic".to_string(),
            },
            Position { x: 3, y: 3 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 5,
                def: 1,
            },
            Tamed { owner: player },
            Experience::default(),
        ))
        .id();
    game.install_innate_routines(medic);
    game.add_companion(medic).unwrap();
    (game, medic)
}

/// A species granting *two* abilities at one level, which is the only shape
/// that can still run a companion out of routine slots.
///
/// `COMPANION_ROUTINE_SLOT_PER_LEVEL` is 1, so an ordinary level-up brings a
/// slot along with whatever it unlocks and the two never contend — no
/// shipped species can reach the eviction branch any more. A mod can, by
/// unlocking two routines on the same rung, and that branch is still live
/// code with a log line of its own.
pub(super) const CONTENDING_UNLOCK_SPECIES: &str = r#"(
    id: "test_crowded",
    name: "Test Crowded",
    glyph: 'c',
    color: Cyan,
    base_hp: 10,
    base_atk: 4,
    base_def: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    base_speed: 10,
    moves: [(name: "Poke", power: 3)],
    abilities: [
        (id: "hot_patch"),
        (id: "sandbox", level: 3),
        (id: "cascade_overflow", level: 3),
    ],
)"#;

/// Spawns a tamed member of `CONTENDING_UNLOCK_SPECIES` into the party of a
/// game built on a modded install that ships it.
pub(super) fn game_with_contending_unlocks_companion() -> (Game, Entity) {
    let dir = modded_assets_dir(
        "contending_unlock_species",
        &[],
        &[],
        &[("test_crowded.ron", CONTENDING_UNLOCK_SPECIES)],
        &[],
        &[],
    );
    let mut game = Game::new(94, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    let crowded = game
        .world
        .spawn((
            Creature {
                species: "test_crowded".to_string(),
            },
            Position { x: 3, y: 3 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 5,
                def: 1,
            },
            Tamed { owner: player },
            Experience::default(),
        ))
        .id();
    game.install_innate_routines(crowded);
    game.add_companion(crowded).unwrap();
    (game, crowded)
}

/// Deploys a Recharger Node `dx`/`dy` tiles from the player, bypassing
/// `place_structure`'s Home and cost requirements — this is about the
/// regen system, not the build rules.
pub(super) fn spawn_recharger_node(game: &mut Game, dx: i32, dy: i32) {
    let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.world.spawn((
        Structure {
            kind: "recharger_node".to_string(),
        },
        Position {
            x: player_pos.x + dx,
            y: player_pos.y + dy,
        },
    ));
}

/// Fuses `game`'s two freshest tamed programs together repeatedly to
/// build up a lineage `depth` fusions deep, returning that program.
pub(super) fn fuse_to_depth(game: &mut Game, depth: u32) -> Entity {
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
pub(super) fn spawn_mining_node(game: &mut Game, x: i32, y: i32) -> Entity {
    game.world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x, y },
            ResourceNode {
                resource: ItemId::from(ids::CORE_FRAGMENT),
                level: None,
            },
        ))
        .id()
}

/// A trading structure, spawned without paying for it.
pub(super) fn spawn_market(game: &mut Game) -> Entity {
    spawn_market_at(game, 5, 5)
}

/// The same, at a chosen tile — for tests about the buyback shelf, which is
/// keyed by the tile its trader stands on.
pub(super) fn spawn_market_at(game: &mut Game, x: i32, y: i32) -> Entity {
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
pub(super) fn give(game: &mut Game, item: &ItemId, qty: u32) {
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(item.clone(), qty);
}

/// Teaches the player `ability` without the research node that normally
/// would — see `resources::KnownRoutines`. Half of what an install needs;
/// `disks` is the other half.
pub(super) fn teach_routine(game: &mut Game, ability: &str) {
    game.world
        .resource_mut::<KnownRoutines>()
        .0
        .insert(ability.to_string());
}

/// Puts `qty` blank Routine Disks in the player's pack, skipping the
/// four-machine chain that manufactures them.
pub(super) fn give_disks(game: &mut Game, qty: u32) {
    give(game, &ItemId::from(ids::ROUTINE_DISK), qty);
}

/// Both halves of a real install — etch a blank, then spend the result —
/// for a test that has already arranged the knowledge and the blank disks
/// itself and is asserting on what they cost.
///
/// Distinct from `install_routine_for_test`, which arranges those too. Use
/// this one when the disk accounting is the point of the test.
pub(super) fn fit_routine(game: &mut Game, entity: Entity, ability: &str) {
    game.etch_disk(ability)
        .unwrap_or_else(|e| panic!("etching {ability}: {e}"));
    game.install_disk(entity, ability)
        .unwrap_or_else(|e| panic!("installing {ability}: {e}"));
}

/// Puts `qty` etched disks of `ability` in the player's pack, skipping both
/// the blank chain and whatever would have etched them.
pub(super) fn give_etched_disks(game: &mut Game, ability: &str, qty: u32) {
    give(game, &ItemId::etched(ability), qty);
}

/// Writes `ability` into `entity`'s next free slot the way the game does —
/// teach it, hand over the blank it burns, etch, install. Most tests want a
/// routine sitting in a slot rather than the chain that got it there.
///
/// Goes through both real verbs rather than `write_routine` directly, so a
/// test that depends on a routine being installed also depends on the
/// install path still working. An exclusive routine can't be taught, so this
/// hands its disk over directly instead — the same two steps the game does,
/// minus the boss.
pub(super) fn install_routine_for_test(game: &mut Game, entity: Entity, ability: &str) {
    if game.routine_is_exclusive(ability) {
        give_etched_disks(game, ability, 1);
    } else {
        teach_routine(game, ability);
        give_disks(game, 1);
        game.etch_disk(ability)
            .unwrap_or_else(|e| panic!("etching {ability}: {e}"));
    }
    game.install_disk(entity, ability)
        .unwrap_or_else(|e| panic!("installing {ability}: {e}"));
}

/// How many *ordinary* (unfused) copies of `item` the player is carrying.
pub(super) fn held(game: &Game, item: &ItemId) -> u32 {
    held_at(game, item, 0)
}

pub(super) fn fragments(game: &Game) -> u32 {
    game.world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .count(&ItemId::from(ids::CORE_FRAGMENT))
}

/// The trade currency, which is what every trader pays — distinct from
/// `fragments`, the build salvage no trader deals in.
pub(super) fn credits(game: &Game) -> u32 {
    game.world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .count(&ItemId::from(ids::CREDITS))
}

/// Finds the deployed Home, if any. Home is the only structure of its
/// kind, so the first match is the only match.
pub(super) fn find_home(game: &mut Game) -> Option<Entity> {
    let mut query = game.world.query::<(Entity, &Structure)>();
    query
        .iter(&game.world)
        .find(|(_, s)| s.kind == HOME_STRUCTURE_ID)
        .map(|(e, _)| e)
}

/// How many `raid_check` rolls each seed gets in the sweeps below.
/// `RAID_CHANCE_PER_TICK` is a per-call roll, so a single call per seed
/// leaves a ~2.7% chance of a 300-seed sweep never firing at all — which
/// unsorted habitat lookup can turn from a stable pass into a flake by
/// shifting RNG consumption between runs. Seven attempts takes that to
/// ~1e-11. Every sweep returns on the first fire, so no target ever takes
/// a second hit.
pub(super) const RAID_ATTEMPTS_PER_SEED: u32 = 7;

/// Deploys a Home plus a Mining Node beside it, returning both entities
/// so a caller can assert on what survives a breach.
pub(super) fn build_a_base(game: &mut Game) -> (Entity, Entity) {
    // On the player's own tile, which is walkable by definition — the
    // Home's slab then guarantees everything around it is too, so this
    // works for any seed.
    place_home(game, 0, 0);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 12);
    game.place_structure("mining_node", 1, 1).unwrap();
    (
        find_structure_by_kind(game, "home").unwrap(),
        find_structure_by_kind(game, "mining_node").unwrap(),
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
pub(super) fn deploy_upgradeable_node(game: &mut Game) -> Entity {
    place_home(game, 0, 1);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 12);
    game.place_structure("mining_node", 1, 0).unwrap();
    find_structure_by_kind(game, "mining_node").unwrap()
}

/// The `ProgramManifest` half of `entity`'s manifest. Most tests that reach
/// for a manifest only care about creature-side detail, and matching the
/// subject enum inline five times reads worse than naming the expectation.
pub(super) fn program_manifest(game: &Game, entity: Entity) -> ProgramManifest {
    match game
        .manifest(entity)
        .expect("entity should have a manifest")
        .subject
    {
        ManifestSubject::Program(p) => p,
        ManifestSubject::Player(_) => panic!("expected a program, got the player"),
    }
}

/// Spawns a bare `Nest` at `(x, y)` with no guardians — for
/// `nest_aggro_tick` tests that build their own hand-picked guardian set
/// rather than taking whatever `Game::spawn_nest`'s RNG-picked count and
/// placement roll. Built from the same `nest_components` (`game/spawning.rs`)
/// as `spawn_nest` and the save-load path, rather than a hand-copied
/// component list — this one used to hardcode `GlyphColor::Red` while a real
/// scrapper nest is `Yellow`, which is exactly the drift sharing the bundle
/// closes off.
pub(super) fn spawn_bare_nest(game: &mut Game, x: i32, y: i32) -> Entity {
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
pub(super) fn spawn_pursuing_guardian(
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
                def: 1,
            },
        ))
        .id()
}

/// How many copies of `item` at fusion `tier` the player is carrying —
/// `Inventory` at tier 0, `GearCopies` above it, the same split
/// `Game::count_copies` makes.
///
/// Reads the stores rather than `PlayerStatus::inventory` on purpose: that
/// list omits banked items, so a view-based helper would report 0 for
/// Research Data and read as a payout bug.
pub(super) fn held_at(game: &Game, item: &ItemId, tier: u32) -> u32 {
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
        item: item.clone(),
        rarity: Rarity::Ordinary,
        tier,
        affix: None,
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
pub(super) fn rng_unadvanced_by(seed: u32, f: impl FnOnce(&mut Game)) -> bool {
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
pub(super) fn held_any(game: &Game, item: &ItemId) -> u32 {
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

/// Posting a program to a machine by hand, **as a test fixture only**.
///
/// This was `Game::assign_cronjob`, a player action, until work orders
/// landed on 2026-08-14: the scheduler decides who stands where now, and
/// the menu row and the engine method both went. It survives here because
/// roughly fifty tests about hauling, upkeep, inspection and the manifest
/// need a program *on* a machine and have nothing to say about how it got
/// there — standing up a work order and a staff pool in each of them would
/// be fixture noise that hides what they are actually asserting.
///
/// **Composed from the shipping primitives, never a copy of the removed
/// body.** `require_surface`, `accepts_a_program`, `post_reach` and
/// `post_worker` are the same four the scheduler goes through, so a test
/// that passes here is a test about a posting the live game could make.
/// A hand-copied body would be the second copy `CLAUDE.md` records drifting
/// four times.
impl Game {
    pub(super) fn assign_cronjob(
        &mut self,
        worker: Entity,
        structure: Entity,
    ) -> Result<(), String> {
        self.require_surface()?;
        if !self.accepts_a_program(structure) {
            return Err("That structure can't be worked.".into());
        }
        let from = *self
            .world
            .get::<Position>(self.player_entity())
            .ok_or_else(|| "You aren't anywhere you can post a program from.".to_string())?;
        let target = *self
            .world
            .get::<Position>(structure)
            .ok_or_else(|| "That structure isn't anywhere you can post to.".to_string())?;
        let blocked = self.structure_tiles();
        let build_radius = self.build_radius();
        {
            let mut map = self.world.resource_mut::<WorldMap>();
            // The two errands stay distinct, as they were: a machine the
            // base has been built around needs digging out, one with no
            // route may just need you to walk over to it.
            crate::game::base::hauling::post_reach(&mut map, from, target, &blocked, build_radius)
                .map_err(|reason| match reason {
                    crate::game::base::hauling::NoPost::BoxedIn => {
                        "That structure is walled in — nothing can stand next to it.".to_string()
                    }
                    crate::game::base::hauling::NoPost::NoRoute => {
                        "No route to that structure from here.".to_string()
                    }
                })?;
        }
        self.post_worker(worker, structure, from);
        // The live scheduler runs inside `tick_inner` and cannot tick again;
        // the removed player action did, and the tests written against it
        // count ticks from that point. Keeping it is what makes this a
        // faithful stand-in rather than a subtly faster one.
        self.tick();
        Ok(())
    }

    pub(super) fn assign_guard(&mut self, worker: Entity, structure: Entity) -> Result<(), String> {
        self.require_surface()?;
        // The live route to a guard post is `Game::set_standing_job`, which
        // carries this same refusal — asked here so the fixture cannot
        // create a post the game would not.
        let kind = self
            .world
            .get::<Structure>(structure)
            .ok_or_else(|| "That's not a structure.".to_string())?
            .kind
            .clone();
        if let Some(name) = self
            .world
            .resource::<StructureDb>()
            .get(&kind)
            .filter(|def| !def.raidable)
            .map(|def| def.name.clone())
        {
            return Err(format!("{name} can't be raided — it doesn't need a guard."));
        }
        self.post_guard(worker, structure);
        self.tick();
        Ok(())
    }
}
