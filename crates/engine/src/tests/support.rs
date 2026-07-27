//! Fixtures and helpers shared by the engine's unit tests.

use crate::tuning::{GROUP_SIZE_STEP_TILES, MAX_ENEMY_GROUPS};
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

/// The species `spawn_tamed` builds its companions from: deliberately one
/// that declares no abilities, so the generic-companion helper yields the
/// fallback rather than whatever kit the shipped roster happens to give
/// its first species. Tests that need to name that species must read it
/// through here rather than re-deriving it, or the two silently drift when
/// kits are reassigned.
pub(super) fn generic_species(game: &Game) -> SpeciesDef {
    game.species_defs()
        .into_iter()
        .find(|s| s.abilities.is_empty())
        .expect("at least one species with no declared abilities")
}

/// A tile far enough from the danger origin that a fight there may hold a
/// full `MAX_ENEMY_GROUPS` groups — `Game::max_enemy_groups` allows one at
/// the origin and gains another every `GROUP_SIZE_STEP_TILES`.
///
/// Any fixture asserting about more than one enemy group has to stand its
/// combatants out here: both halves of the pack ceiling are read from the
/// ground the members occupy (see `Game::group_pack`), so a multi-group
/// pack placed beside the spawn point partitions down to a single group.
/// Stats scale with distance too, which is why fixtures that pin damage
/// numbers set `Stats` explicitly rather than relying on species bases.
pub(super) fn multi_group_ground(game: &Game) -> (i32, i32) {
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    (
        spawn.x + GROUP_SIZE_STEP_TILES * (MAX_ENEMY_GROUPS as i32 - 1),
        spawn.y,
    )
}

/// Sets `entity`'s level directly, for tests that need a level-gated
/// ability unlocked without grinding XP into it. Installs whatever species
/// unlocks that jump reaches, exactly as a real level-up would — otherwise
/// a test that raises a level would see a kit the game never leaves behind.
pub(super) fn set_level(game: &mut Game, entity: Entity, level: u32) {
    let before = game.world.get::<Experience>(entity).unwrap().level;
    game.world.get_mut::<Experience>(entity).unwrap().level = level;
    if level > before {
        game.install_unlocked_routines(entity, before, level);
    }
}

pub(super) fn test_assets_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
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

pub(super) fn player_decompiles(game: &mut Game) {
    resolve_round_with(game, BattleAction::Decompile { group: 0 });
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
        groups,
        round: 1,
        planned: vec![None; slots],
        finished: false,
        player_won: false,
    });
}

/// Copies the shipped `species`/`structures`/`research`/`items` asset
/// dirs into a fresh scratch dir, skipping the item files named in
/// `omit_items` and writing the `extra_*` (filename, RON body) pairs on
/// top — a stand-in for a modded install. The caller removes the
/// directory once its `Game` is done with it.
pub(super) fn modded_assets_dir(
    tag: &str,
    omit_items: &[&str],
    extra_items: &[(&str, &str)],
    extra_species: &[(&str, &str)],
    extra_research: &[(&str, &str)],
) -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "feral_processes_{tag}_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let shipped = test_assets_dir();
    for sub in ["species", "structures", "research", "items", "abilities"] {
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
    for (name, body) in extra_items {
        std::fs::write(dir.join("items").join(name), body).unwrap();
    }
    for (name, body) in extra_species {
        std::fs::write(dir.join("species").join(name), body).unwrap();
    }
    for (name, body) in extra_research {
        std::fs::write(dir.join("research").join(name), body).unwrap();
    }
    dir
}

/// A modded install missing `core_fragment.ron` — the item that holds
/// the Currency economy role — so `Game::new`'s missing-role startup
/// abort (see `ItemDb::missing_roles`) can be exercised against an
/// otherwise-valid item set.
pub(super) fn assets_dir_missing_currency_item() -> std::path::PathBuf {
    modded_assets_dir("missing_currency", &["core_fragment.ron"], &[], &[], &[])
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

/// Unlocks `id` and every prerequisite it needs, funding the whole
/// chain — so a test that just needs a research-gated structure on the
/// map doesn't have to model the tree itself.
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
    for node in chain {
        if !game.is_researched(&node) {
            game.unlock_research(&node).unwrap();
        }
    }
}

pub(super) fn research_data_held(game: &Game) -> u32 {
    game.player_status()
        .inventory
        .iter()
        .find(|(item, _)| *item == ItemId::from(ids::RESEARCH_DATA))
        .map(|(_, n)| *n)
        .unwrap_or(0)
}

/// Tames a program and puts it to work on a node producing `resource`,
/// so a cronjob is guaranteed to be running — the assertions below are
/// vacuous if nothing is assigned.
pub(super) fn assign_worker_producing(game: &mut Game, resource: ItemId) {
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
                amount: 20,
                capacity: 20,
                level: None,
            },
        ))
        .id();
    game.assign_cronjob(worker, structure).unwrap();
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
    run_one_full_gather_cycle_at_tier(game, resource, None)
}

/// Runs exactly one completed gather cycle against a hand-built node
/// producing `resource` at `tier`, and returns how many units landed in
/// the player's inventory.
///
/// `level: None` on the node means it always yields (see
/// `systems::mining_success_chance`), which is what keeps the payout
/// assertions off the RNG entirely.
pub(super) fn run_one_full_gather_cycle_at_tier(
    game: &mut Game,
    resource: &str,
    tier: Option<u32>,
) -> u32 {
    let worker = spawn_tamed(game, 10, 3);
    let mut structure = game.world.spawn((
        Structure {
            kind: "mining_node".to_string(),
        },
        Position { x: 3, y: 4 },
        ResourceNode {
            resource: ItemId::from(resource),
            amount: 5,
            capacity: 5,
            level: None,
        },
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

    let before = count_item(game, resource);
    game.tick();
    count_item(game, resource) - before
}

pub(super) fn find_structure_by_kind(game: &mut Game, kind: &str) -> Option<Entity> {
    let mut query = game.world.query::<(Entity, &Structure)>();
    query
        .iter(&game.world)
        .find(|(_, s)| s.kind == kind)
        .map(|(e, _)| e)
}

/// The initial world spawns 14 wild creatures scattered around the
/// player, so directional-inspect tests clear whatever landed along
/// their search ray first — otherwise they'd be at the mercy of the
/// seed's RNG instead of testing the method itself.
pub(super) fn clear_creatures_east_of_player(game: &mut Game, start: Position, range: i32) {
    // Matches the same 90° eastward cone `find_creature_in_direction`
    // itself uses, not just the exact row — otherwise a wild creature
    // that merely leans east (without being exactly on the player's
    // row) would survive the cleanup and make the test flaky.
    let stale: Vec<Entity> = {
        let mut query = game.world.query::<(Entity, &Position, &Creature)>();
        query
            .iter(&game.world)
            .filter(|(_, pos, _)| {
                let (ddx, ddy) = (pos.x - start.x, pos.y - start.y);
                ddx > 0 && ddx >= ddy.abs() && ddx <= range
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

/// Spawns a wild program on the player's tile and opens an intrusion on
/// it — the state `battle_decompile` needs.
pub(super) fn start_battle_with_a_wild_program(game: &mut Game) -> Entity {
    let wild = spawn_wild_on_player_tile(game);
    game.start_battle(vec![wild]);
    wild
}

pub(super) fn spawn_tamed(game: &mut Game, hp: i32, atk: i32) -> Entity {
    let player = game.player_entity();
    let species = generic_species(game);
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
/// and returns how much the player's fatigue dropped from commanding
/// it. Shared by the two fatigue-cost tests below.
pub(super) fn fatigue_spent_commanding_companion(seed: u32, stunned: bool) -> f32 {
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

    let fatigue_before = game.world.get::<Needs>(player).unwrap().fatigue;
    companion_uses_special(
        &mut game,
        companion,
        0,
        battle::SpecialTarget::Ally { slot: 0 },
    );
    let fatigue_after = game.world.get::<Needs>(player).unwrap().fatigue;
    fatigue_before - fatigue_after
}

/// A species declaring two abilities, so the multi-ability paths can be
/// exercised without depending on shipped kit assignments. The second is
/// gated above a fresh companion's level 1, which is what pins down
/// `Game::companion_abilities`' level filtering.
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

/// A trading structure, spawned without paying for it.
pub(super) fn spawn_market(game: &mut Game) -> Entity {
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
        .spawn((Structure { kind }, Position { x: 5, y: 5 }))
        .id()
}

pub(super) fn fragments(game: &Game) -> u32 {
    game.world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .count(&ItemId::from(ids::CORE_FRAGMENT))
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
pub(super) fn deploy_upgradeable_node(game: &mut Game) -> Entity {
    place_home(game, 0, 1);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 12);
    game.place_structure("mining_node", 1, 1).unwrap();
    find_structure_by_kind(game, "mining_node").unwrap()
}
