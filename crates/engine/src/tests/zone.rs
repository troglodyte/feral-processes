//! Zone depth scaling and what survives a breach into the next zone.

use super::support::*;
use crate::tuning::BOSS_STAT_MULT;
use crate::tuning::{
    MAX_BUILD_DISTANCE_FROM_HOME, MAX_ENEMY_GROUPS, MAX_GROUP_SIZE, NEST_AGGRO_LEASH_RADIUS,
    NEST_CACHE_CREDIT_ZONE_BONUS, NEST_CACHE_CREDITS, NEST_CACHE_WORK_RESOURCE_MULT,
    NEST_DURABILITY, NEST_PATH_SEARCH_MARGIN, NEST_PURSUIT_STEPS_PER_TICK, NEST_RESPAWN_TICKS,
    NEST_TETHER_RADIUS, WORK_RESOURCE_DROP,
};
use crate::world::SectorShape;
use crate::*;

#[test]
fn entering_a_zone_portal_increments_zone_and_doubles_wild_stats() {
    let mut game = Game::new(40, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(game.player_status().zone, 1);

    breach_through_a_portal(&mut game);

    assert_eq!(
        game.player_status().zone,
        2,
        "walking onto a zone portal should advance the zone level"
    );

    let species_db = game.species_defs();
    let mut query = game
        .world
        .query_filtered::<(&Creature, &Stats, &Position, Option<&Rarity>, Option<&Boss>), With<Hostile>>();
    let results: Vec<_> = query
        .iter(&game.world)
        .map(|(c, s, p, r, b)| {
            (
                c.species.clone(),
                s.max_hp,
                *p,
                r.copied().unwrap_or_default(),
                b.is_some(),
            )
        })
        .collect();
    assert!(
        !results.is_empty(),
        "zone 2 should have spawned wild creatures"
    );
    for (species_id, max_hp, _pos, rarity, boss) in results {
        let species = species_db.iter().find(|s| s.id == species_id).unwrap();
        // Zone 2 doubles base stats (`ZoneLevel::stat_multiplier`) and the
        // spawn's own `Potential::hp_roll` scales it within
        // `MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL`. That is the whole
        // range now — where on the map it spawned contributes nothing,
        // which is what makes this a tight bound rather than the
        // three-times-wider one distance scaling used to force.
        //
        // The rare-spawn tier is the one other factor, and folding it in per
        // creature rather than widening the bound to the gold ceiling is
        // deliberate: it keeps an ordinary spawn held to the tight range,
        // and it makes this assert that `Rarity::stat_mult` was applied
        // exactly *once* — a second application anywhere downstream lands
        // outside these bounds rather than passing quietly.
        let rare = rarity.stat_mult();
        // Folded in per creature for exactly the reason `rare` is: an
        // ordinary spawn stays held to the tight range, and this asserts
        // `BOSS_STAT_MULT` was applied exactly *once*. A rolled boss is an
        // ordinary species carrying `Boss` — reading `is_boss` off the
        // species would miss it.
        let boss_mult = if boss && !species.is_boss {
            BOSS_STAT_MULT
        } else {
            1.0
        };
        assert!(
            (max_hp as f32)
                >= (species.base_hp as f32) * 2.0 * rare * boss_mult * MIN_INDIVIDUAL_ROLL,
            "zone 2 wild creatures should have at least doubled stats, times the roll floor"
        );
        // Rounded, because `spawn_wild_creature_scaled` rounds: a 112-HP
        // species at the roll ceiling computes 268.8 and stores 269, which
        // a bare float bound reads as over-cap. The bound was only ever
        // right by luck — 14 seeded creatures rarely landed a near-ceiling
        // roll on a high-HP species, and seeding to a density target makes
        // that the common case.
        assert!(
            (max_hp as f32)
                <= ((species.base_hp as f32) * 2.0 * rare * boss_mult * MAX_INDIVIDUAL_ROLL)
                    .round(),
            "zone 2 wild creatures shouldn't exceed the zone doubling times the roll ceiling"
        );
    }
}

/// The cap is linear, not geometric, and that is the point of it.
///
/// Geometric growth from a base of 1 spends its whole early range in single
/// digits — zone 2 capped every group at 3, so a party of five met packs of
/// three however far out or however deep they pushed — and then runs away
/// past zone 4, where 27 and 81 are numbers no encounter design uses. A
/// straight line opens the early zones, which is where the game is actually
/// played, and keeps the tail somewhere a fight can still be read.
#[test]
fn zone_group_cap_is_linear_and_never_passes_max_group_size() {
    use crate::game::spawning::zone_group_cap;
    assert_eq!(
        zone_group_cap(1),
        2,
        "zone 1 takes the ZONE_ONE_GROUP_CAP floor, not the curve's base — the curve alone \
         puts it at 1, which is a group that teaches a new player nothing about groups"
    );
    assert_eq!(zone_group_cap(2), 10);
    assert_eq!(zone_group_cap(3), 19);
    assert_eq!(zone_group_cap(4), 28);
    assert_eq!(zone_group_cap(5), 37);
    assert_eq!(
        zone_group_cap(12),
        MAX_GROUP_SIZE,
        "1 + 9 * 11 is exactly 100, so zone 12 is where the clamp starts"
    );
    assert_eq!(
        zone_group_cap(99),
        MAX_GROUP_SIZE,
        "a deep zone must clamp rather than overflow the arithmetic"
    );
}

/// Group size doubles per escalation step, and on the surface a step is a
/// zone. The distance curve this replaced meant a zone had no consistent
/// difficulty of its own — how hard a fight was depended on which way you
/// had wandered from the spawn point.
#[test]
fn max_group_size_doubles_per_zone_and_caps_per_zone() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let at = |game: &mut Game, zone: u32| {
        game.world.resource_mut::<ZoneLevel>().0 = zone;
        game.max_group_size(None)
    };

    assert_eq!(
        at(&mut game, 1),
        1,
        "zone 1 is solo — the whole point of it"
    );
    assert_eq!(at(&mut game, 2), 2, "one zone in doubles");
    assert_eq!(
        at(&mut game, 4),
        8,
        "and keeps doubling while under the cap"
    );
    assert_eq!(
        at(&mut game, 5),
        16,
        "four steps is 2^4, still under zone 5's cap of 37"
    );
    assert_eq!(
        at(&mut game, 6),
        32,
        "five steps is 2^5, under zone 6's cap of 46"
    );
    assert_eq!(
        at(&mut game, 8),
        64,
        "seven steps would be 128, but the step count clamps at 7 first"
    );
    assert_eq!(
        at(&mut game, 12),
        MAX_GROUP_SIZE,
        "zone 12 is where the linear cap first reaches the hard ceiling"
    );
    assert_eq!(
        at(&mut game, 99),
        MAX_GROUP_SIZE,
        "no zone may push a group past MAX_GROUP_SIZE, or shift past the type"
    );
}

/// Where a fight happens inside a zone decides nothing about its size. This
/// is the property the distance curve cost us: a base built far from the
/// spawn point used to sit in permanently harder territory.
#[test]
fn group_size_is_the_same_everywhere_in_a_zone() {
    let mut game = Game::new(931, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 4;
    let expected = game.max_group_size(None);
    assert!(
        expected > 1,
        "zone 4 should field packs, or this asserts nothing"
    );

    place_home(&mut game);
    assert_eq!(
        game.max_group_size(None),
        expected,
        "placing a Home must not change the zone's pack size"
    );
}

/// The count of groups rides the same curve as their size, one step per
/// zone rather than a doubling. Zone 1 is the part that matters: a fight
/// there is one program, which is what makes the opening survivable for a
/// player who has no companions yet.
#[test]
fn max_enemy_groups_gains_one_group_per_zone_and_stops_at_the_ceiling() {
    let mut game = Game::new(43, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let at = |game: &mut Game, zone: u32| {
        game.world.resource_mut::<ZoneLevel>().0 = zone;
        game.max_enemy_groups(None)
    };

    assert_eq!(at(&mut game, 1), 1, "one group in zone 1");
    assert_eq!(at(&mut game, 2), 2);
    assert_eq!(at(&mut game, 3), 3);
    assert_eq!(at(&mut game, 4), MAX_ENEMY_GROUPS);
    assert_eq!(
        at(&mut game, 10_000),
        MAX_ENEMY_GROUPS,
        "no zone may push a fight past the group ceiling"
    );
}

#[test]
fn stepping_through_a_portal_consumes_it_so_it_never_travels() {
    let mut game = Game::new(950, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::PORTAL_FRAGMENT), 10);
    stand_in_base(&mut game);
    place_now(&mut game, "portal", 1, 0).unwrap();

    game.move_player(1, 0);

    assert_eq!(
        game.world.resource::<ZoneLevel>().0,
        2,
        "stepping onto the portal breaches"
    );
    assert!(
        find_structure_by_kind(&mut game, "portal").is_none(),
        "a portal is one-use — carrying it forward would make every later breach free"
    );
    assert!(
        find_structure_by_kind(&mut game, "home").is_some(),
        "consuming the portal must not take the rest of the base with it"
    );
}

/// `warp_to_zone` exists for the savetool, and its whole value is that it
/// is not a shortcut: writing `ZoneLevel` directly would relabel the party
/// as being in zone 4 while leaving zone 1's map and zone 1's wild programs
/// around them.
///
/// The wild population is what discriminates. The spawn point does *not* —
/// `find_walkable_start` spirals out from the origin, so for most seeds it
/// answers (0, 0) in every sector and is identical either way. An earlier
/// version of this test asserted on it and would have passed against a
/// `ZoneLevel` write.
#[test]
fn warping_forward_lands_exactly_where_stepping_through_the_breaches_lands() {
    let wild = |game: &mut Game| {
        let mut query = game
            .world
            .query_filtered::<(&Creature, &Stats), With<Hostile>>();
        let mut found: Vec<_> = query
            .iter(&game.world)
            .map(|(c, s)| (c.species.clone(), s.max_hp))
            .collect();
        found.sort();
        found
    };

    let mut warped = Game::new(940, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let before = wild(&mut warped);
    warped.warp_to_zone(4).unwrap();

    let mut stepped = Game::new(940, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for _ in 0..3 {
        stepped.enter_next_zone();
    }

    assert_eq!(warped.player_status().zone, 4);
    let after = wild(&mut warped);
    assert_ne!(
        after, before,
        "warping must generate new sectors, not just relabel the current one"
    );
    assert_eq!(
        after,
        wild(&mut stepped),
        "and must leave exactly what walking through three portals leaves"
    );
}

/// There is no portal back, so a breach only runs forward.
#[test]
fn warping_to_a_zone_that_is_not_ahead_is_refused() {
    let mut game = Game::new(940, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.warp_to_zone(3).unwrap();

    assert!(
        game.warp_to_zone(3).is_err(),
        "the current zone is not ahead"
    );
    assert!(game.warp_to_zone(1).is_err(), "and zone 1 is behind");
    assert_eq!(
        game.player_status().zone,
        3,
        "a refused warp must not have moved the party part of the way"
    );
}

/// Base space is a separate coordinate system from the zone surface — Home
/// sits at `BASE_EXIT_CELL` and a fresh zone's spawn tile is found by the
/// same `(0, 0)`-first scan, so their numbers coincide by construction and
/// comparing them proves nothing about which space either belongs to (see
/// `find_blocking_structure_at`'s doc comment for the same trap on a
/// different call site). Home and the node are hand-displaced off the
/// origin before the breach for exactly that reason: left at their real
/// founding coordinates, both would sit at `(0, 0)` before and after
/// regardless of whether the deleted reposition block were still here, so
/// the check would pass either way. What actually matters, and what the old
/// version of this test got backwards, is that the breach must leave Home
/// exactly where it already was rather than relocating it to the new spawn
/// tile — asserting equality to spawn was asserting the very relocation
/// this task deletes.
#[test]
fn breaching_carries_every_structure_and_its_offset_from_home() {
    let mut game = Game::new(940, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (home, node) = build_a_base(&mut game);
    {
        let mut pos = game.world.get_mut::<Position>(home).unwrap();
        pos.x = 17;
        pos.y = 4;
    }
    {
        let mut pos = game.world.get_mut::<Position>(node).unwrap();
        pos.x = 25;
        pos.y = 41;
    }
    let home_before = *game.world.get::<Position>(home).unwrap();
    let node_before = *game.world.get::<Position>(node).unwrap();

    game.enter_next_zone();

    assert!(
        game.world.get_entity(home).is_ok(),
        "the Home is not zone-local — it survives the breach"
    );
    assert!(
        game.world.get_entity(node).is_ok(),
        "so does everything built around it"
    );
    assert_eq!(
        *game.world.get::<Position>(home).unwrap(),
        home_before,
        "a breach must not move the Home at all, let alone onto the new spawn point"
    );
    assert_eq!(
        *game.world.get::<Position>(node).unwrap(),
        node_before,
        "and every structure keeps its own absolute base-space coordinate too"
    );
}

#[test]
fn breaching_with_a_base_still_populates_the_new_zone() {
    for seed in 0u32..12 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        build_a_base(&mut game);

        game.enter_next_zone();

        let hostiles = {
            let mut query = game.world.query_filtered::<Entity, With<Hostile>>();
            query.iter(&game.world).count()
        };
        assert!(
            hostiles > 0,
            "seed {seed}: a zone breached into with a base must still have wild programs \
             in it. The platform has no habitat species and is exactly as wide as the \
             initial spawn scatter, so a scatter that never reaches past its edge leaves \
             the whole zone empty."
        );
    }
}

/// `Structure` is the space tag: `find_blocking_structure_at` must refuse
/// to answer at all outside base space, rather than matching a base-space
/// `Structure`'s position against a surface coordinate that happens to
/// carry the same numbers. `Game::new`'s spawn point and base space's own
/// origin are both commonly `(0, 0)` (see the standing note on
/// `find_walkable_start`), which is exactly the collision this closes —
/// before it, `game/stack.rs::link_site_free` could refuse a valid Stack
/// entrance near a base for no reason a player could see.
#[test]
fn find_blocking_structure_at_refuses_outside_base_space() {
    let mut game = Game::new(944, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (home, _node) = build_a_base(&mut game);
    assert!(
        !game.in_base(),
        "build_a_base's fixtures return the party to the surface"
    );
    let home_pos = *game.world.get::<Position>(home).unwrap();

    assert!(
        game.find_blocking_structure_at(home_pos.x, home_pos.y)
            .is_none(),
        "a Structure's base-space position must not answer a surface-space query, \
         even when the numbers coincide"
    );

    stand_in_base_at(&mut game, home_pos.x, home_pos.y);
    assert_eq!(
        game.find_blocking_structure_at(home_pos.x, home_pos.y),
        Some(home),
        "the same coordinates answer correctly once the query is actually asked from base space"
    );
}

#[test]
fn breaching_preserves_structure_durability_and_node_stock() {
    let mut game = Game::new(941, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (_home, node) = build_a_base(&mut game);
    game.world.get_mut::<Durability>(node).unwrap().hp = 7;
    game.world
        .get_mut::<Stock>(node)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), 2);

    game.enter_next_zone();

    assert_eq!(
        game.world.get::<Durability>(node).unwrap().hp,
        7,
        "damage travels with the structure"
    );
    assert_eq!(
        node_output(&game, node, ids::CORE_FRAGMENT),
        2,
        "so does anything still sitting in its output buffer, uncollected"
    );
}

#[test]
fn breaching_leaves_a_cronjob_assignment_pointing_at_a_live_structure() {
    let mut game = Game::new(943, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (home, node) = build_a_base(&mut game);
    // Both Home and the node are moved off `(0, 0)` for the same reason
    // `breaching_leaves_every_structures_absolute_position_untouched` does:
    // Home always founds at `BASE_EXIT_CELL`, itself `(0, 0)`, and
    // `find_walkable_start` always resolves `(0, 0)` too — so the deleted
    // block's `spawn + (node - home)` write reproduces the same absolute
    // number whenever Home is left at its real founding position, even
    // with the node displaced on its own. Displacing Home too breaks that
    // coincidence and makes the check below load-bearing.
    {
        let mut pos = game.world.get_mut::<Position>(home).unwrap();
        pos.x = 12;
        pos.y = -7;
    }
    {
        let mut pos = game.world.get_mut::<Position>(node).unwrap();
        pos.x = 33;
        pos.y = -9;
    }
    let node_before = *game.world.get::<Position>(node).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: node,
        progress: 0,
        required: 10,
    });

    game.enter_next_zone();

    let task = game
        .world
        .get::<Task>(worker)
        .expect("the cronjob survives the breach");
    assert_eq!(
        task.target, node,
        "and still points at the structure that travelled with it"
    );
    assert!(
        game.world.get_entity(task.target).is_ok(),
        "which is still alive"
    );
    assert_eq!(
        *game.world.get::<Position>(task.target).unwrap(),
        node_before,
        "and still stands exactly where the cronjob left it, not wherever a breach's \
         reposition would have put it"
    );
}

#[test]
fn zone_transition_carries_tamed_companions_and_the_base_but_leaves_wild_creatures_behind() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();

    // Clear anything the world's own initial habitat spawn happened to
    // place on the tiles this test is about to use for its own fixtures
    // (portal, home, wild) — the exact initial layout isn't this test's
    // concern, and asserting it stays untouched would make the test
    // fragile to unrelated changes in spawn odds/roll counts.
    let stray: Vec<Entity> = {
        let mut query = game.world.query::<(Entity, &Position)>();
        query
            .iter(&game.world)
            .filter(|(e, p)| {
                *e != player
                    && ((p.x, p.y) == (ppos.x + 1, ppos.y)
                        || (p.x, p.y) == (ppos.x + 3, ppos.y)
                        || (p.x, p.y) == (ppos.x + 5, ppos.y))
            })
            .map(|(e, _)| e)
            .collect()
    };
    for e in stray {
        game.world.despawn(e);
    }

    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();

    let species = game.species_defs().into_iter().next().unwrap();
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position {
                x: ppos.x + 3,
                y: ppos.y,
            },
            Stats {
                hp: 5,
                max_hp: 5,
                atk: 1,
                mitigation: 1,
            },
        ))
        .id();

    let home = game
        .world
        .spawn((
            Structure {
                kind: "home".to_string(),
            },
            Position {
                x: ppos.x + 5,
                y: ppos.y,
            },
        ))
        .id();

    breach_through_a_portal(&mut game);

    assert_eq!(game.player_status().zone, 2);
    assert!(
        game.world.get::<Tamed>(companion).is_some(),
        "the companion should still be tamed after breaching"
    );
    assert!(
        game.world.get::<Creature>(wild).is_none(),
        "wild creatures should be left behind, not carried through the portal"
    );
    assert!(
        game.world.get::<Structure>(home).is_some(),
        "the base is not zone-local — it survives the breach"
    );
    assert_eq!(
        *game.world.get::<Position>(home).unwrap(),
        Position {
            x: ppos.x + 5,
            y: ppos.y,
        },
        "and stays exactly where it was, not repositioned onto the new zone's spawn point"
    );
    let companion_pos = *game.world.get::<Position>(companion).unwrap();
    let player_pos = *game.world.get::<Position>(player).unwrap();
    assert_eq!(
        companion_pos, player_pos,
        "the companion should travel with the player into the new zone"
    );
}

#[test]
fn breaching_wipes_the_currency_and_craft_currency_stacks() {
    let mut game = Game::new(945, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.add(ItemId::from(ids::PORTAL_FRAGMENT), 25);
        inv.add(ItemId::from(ids::CORE_FRAGMENT), 40);
        inv.add(ItemId::from(ids::CREDITS), 30);
    }

    game.enter_next_zone();

    assert_eq!(
        count_item(&game, ids::PORTAL_FRAGMENT),
        0,
        "the next zone's portal has to be funded in the zone you leave from"
    );
    assert_eq!(
        count_item(&game, ids::CORE_FRAGMENT),
        0,
        "and so does everything the base is bought with"
    );
    assert_eq!(
        count_item(&game, ids::CREDITS),
        30,
        "Credits are the one liquid thing that crosses a breach — selling \
         a doomed stockpile before you go is the point of a trader"
    );
}

/// Cache Grain is a `WorkResource`, not one of the two currency roles
/// `enter_next_zone` wipes by name — so unlike Portal Fragment and Core
/// Fragment in the zone-currency stacks test above, it must ride the breach
/// through untouched, the same as any other banked material.
#[test]
fn breaching_carries_cache_grain_through_while_wiping_both_currencies() {
    let mut game = Game::new(951, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.add(ItemId::from("cache_grain"), 17);
        inv.add(ItemId::from(ids::PORTAL_FRAGMENT), 25);
        inv.add(ItemId::from(ids::CORE_FRAGMENT), 40);
    }

    game.enter_next_zone();

    assert_eq!(
        count_item(&game, "cache_grain"),
        17,
        "Cache Grain is a work resource, not a currency — it must cross the breach"
    );
    assert_eq!(
        count_item(&game, ids::PORTAL_FRAGMENT),
        0,
        "while the build currency it's not is wiped, same as the other currency test"
    );
    assert_eq!(
        count_item(&game, ids::CORE_FRAGMENT),
        0,
        "and the craft currency alongside it"
    );
}

#[test]
fn breaching_keeps_everything_that_is_not_spendable_currency() {
    let mut game = Game::new(946, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.add(ItemId::from(ids::RESEARCH_DATA), 60);
        inv.add(ItemId::from(ids::POWER_CELL), 4);
    }
    game.world
        .get_mut::<GearCopies>(player)
        .unwrap()
        .add(gear(&ItemId::from(ids::ABLATIVE_PLATING), 1), 1);

    game.enter_next_zone();

    assert_eq!(
        count_item(&game, ids::RESEARCH_DATA),
        60,
        "banked research is progress, not pocket money"
    );
    assert_eq!(
        count_item(&game, ids::POWER_CELL),
        7,
        "3 from the starting kit plus the 4 added; supplies are carried, not confiscated"
    );
    assert_eq!(
        count_item(&game, ids::ICE_BREAKER),
        3,
        "the starting kit's catalysts make the trip too"
    );
    assert_eq!(
        game.world
            .get::<GearCopies>(player)
            .unwrap()
            .count(&gear(&ItemId::from(ids::ABLATIVE_PLATING), 1)),
        1,
        "a fused copy is gear, not currency"
    );
}

/// A `FieldBuff` is player state, not zone-local — the inverse of the
/// `BuybackLedger` trap, where anything zone-local has to be wiped by name
/// in `enter_next_zone`. Nothing should be wiping this one, so a breach
/// must leave it running untouched.
#[test]
fn breaching_keeps_a_running_field_buff() {
    let mut game = Game::new(948, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::XpBoost,
            name: "Overclock Protocol".to_string(),
            power: 20,
            remaining: 9,
            interval: 1,
            source: BuffSource::Consumable,
        },
    );

    game.enter_next_zone();

    let buff = game
        .world
        .get::<FieldBuff>(player)
        .unwrap()
        .active
        .first()
        .cloned()
        .expect("a field buff is player state, not zone-local — it must survive a breach");
    assert_eq!(buff.kind, FieldBuffKind::XpBoost);
    assert_eq!(
        buff.remaining, 9,
        "a breach must not itself age or reset a buff"
    );
}

#[test]
fn the_decohere_message_only_fires_when_there_was_something_to_lose() {
    let mut game = Game::new(947, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .take(ItemId::from(ids::CORE_FRAGMENT), u32::MAX);

    game.enter_next_zone();

    assert!(
        !game
            .message_log(200)
            .iter()
            .any(|e| e.text.contains("decohere")),
        "an empty wallet shouldn't be announced as a loss"
    );

    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::PORTAL_FRAGMENT), 3);
    game.enter_next_zone();

    // "{qty} {name}", the same unpluralized shape `describe_structure`
    // uses for a teleport cost — item names are modder-supplied data, not
    // English to inflect.
    assert!(
        game.message_log(200)
            .iter()
            .any(|e| e.text.contains("3 Portal Fragment")),
        "a real loss is named and counted: {:?}",
        game.message_log(200)
    );
}

#[test]
fn portal_cost_grows_by_half_the_base_rate_per_zone() {
    let mut game = Game::new(944, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let portal = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "portal")
        .expect("portal.ron should load");
    let fragments = |game: &Game, def: &StructureDef| {
        game.structure_build_cost(def)
            .into_iter()
            .find(|(item, _)| item.as_str() == ids::PORTAL_FRAGMENT)
            .map(|(_, qty)| qty)
            .expect("a portal is bought with portal fragments")
    };

    assert_eq!(fragments(&game, &portal), 10, "zone 1 pays the base rate");

    game.world.insert_resource(ZoneLevel(2));
    assert_eq!(
        fragments(&game, &portal),
        15,
        "each zone adds half the base rate, not another whole one"
    );

    game.world.insert_resource(ZoneLevel(5));
    assert_eq!(
        fragments(&game, &portal),
        30,
        "the ramp stays linear in the base rate all the way down"
    );

    let node = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "mining_node")
        .expect("mining_node.ron should load");
    assert_eq!(
        game.structure_build_cost(&node),
        node.build_cost,
        "only a zone-portal structure scales; everything else is flat at any depth"
    );
}

#[test]
fn portal_build_cost_ramps_with_current_zone_level() {
    let mut game = Game::new(42, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    place_home(&mut game);

    // Zone 1: base rate from portal.ron, 10 PortalFragment, unramped.
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::PORTAL_FRAGMENT), 10);
    stand_in_base(&mut game);
    place_now(&mut game, "portal", 1, 0).unwrap();
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::PORTAL_FRAGMENT)),
        0,
        "zone 1 portal should cost the base rate"
    );

    game.move_player(1, 0);
    assert_eq!(game.player_status().zone, 2);
    // The Home travelled through the breach with the rest of the base
    // (see `breaching_carries_every_structure_and_its_offset_from_home`),
    // so the new zone needs no fresh Home before building.

    // Zone 2: base rate plus half of it again (10 + 5 = 15), not double.
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::PORTAL_FRAGMENT), 14);
    // The ramp is read off the bill of materials the request is filed
    // against, not off a refusal: a deploy no longer refuses for want of
    // materials, so what says "14 is not enough" is the site itself
    // reporting one fragment still outstanding.
    game.place_structure("portal", 1, 0)
        .expect("a request is filed whatever is in the pack");
    let site = game.build_site_at(1, 0).expect("the request stands there");
    assert_eq!(
        game.world
            .get::<crate::components::BuildSite>(site)
            .expect("it is a build site")
            .cost,
        vec![(ItemId::from(ids::PORTAL_FRAGMENT), 15)],
        "zone 2 is the base rate plus half of it again, not double"
    );
    game.cancel_build_request(site).unwrap();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::PORTAL_FRAGMENT), 1);
    stand_in_base(&mut game);
    place_now(&mut game, "portal", 1, 0).unwrap();
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::PORTAL_FRAGMENT)),
        0,
        "zone 2 portal should cost the base rate plus half again"
    );
}

#[test]
fn zone_level_survives_save_and_load() {
    let assets = test_assets_dir();
    let mut game = Game::new(43, DifficultyMode::Forgiving, &assets).unwrap();
    breach_through_a_portal(&mut game);
    assert_eq!(game.player_status().zone, 2);

    let path = std::env::temp_dir().join(format!(
        "feral_processes_zone_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded.player_status().zone,
        2,
        "zone level should survive a save/load round trip"
    );
}

/// Regression test for a nearly-empty zone: `find_walkable_start`
/// always re-centers a freshly generated zone's spawn box near world
/// origin, and the terrain noise there has roughly the same period as
/// that box — so a blind, one-attempt-per-slot spawn (the previous
/// behavior of `spawn_initial_creatures`) could land almost all 14
/// rolls on an unwalkable or habitat-mismatched tile for an unlucky
/// seed, leaving the new zone feeling all but abandoned. Sweeps a
/// range of seeds (rather than trusting one lucky one) to confirm the
/// retry-until-`count` fix reliably delivers the full population.
#[test]
fn zone_transition_reliably_populates_the_new_zone_regardless_of_seed() {
    for seed in 0u32..20 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let ppos = *game.world.get::<Position>(player).unwrap();
        // The zone-1 starting spawn can, for some seeds, happen to
        // place a wild creature right on the tile the portal is about
        // to go on — clear it so the walk onto the portal deterministically
        // enters the portal rather than picking a fight instead.
        let blockers: Vec<Entity> = {
            let mut query = game
                .world
                .query_filtered::<(Entity, &Position), With<Hostile>>();
            query
                .iter(&game.world)
                .filter(|(_, p)| p.x == ppos.x + 1 && p.y == ppos.y)
                .map(|(e, _)| e)
                .collect()
        };
        for e in blockers {
            game.world.despawn(e);
        }
        breach_through_a_portal(&mut game);
        assert_eq!(
            game.player_status().zone,
            2,
            "seed {seed}: portal should advance the zone"
        );

        let mut query = game.world.query_filtered::<Entity, With<Hostile>>();
        let count = query.iter(&game.world).count();
        assert!(
            count >= 14,
            "seed {seed}: zone 2 should have spawned at least the 14 requested wild \
             creatures, found {count}"
        );
    }
}

/// Nest provocation: `Game::attack_nest` marking guardians `Pursuing`, and
/// every path that removes a guardian from the world (a destroyed nest, a
/// tamed capture) removing the marker with it. The tests below this point
/// are about setting and clearing that marker in isolation; `nest_aggro_tick`
/// — the part that actually moves a `Pursuing` guardian and starts a fight —
/// gets its own tests further down.
fn guardians_of(game: &mut Game, nest: Entity) -> Vec<Entity> {
    let mut query = game.world.query::<(Entity, &NestGuardian)>();
    query
        .iter(&game.world)
        .filter(|(_, g)| g.nest == nest)
        .map(|(e, _)| e)
        .collect()
}

#[test]
fn attacking_a_nest_provokes_only_its_own_guardians() {
    let mut game = Game::new(700, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Far enough apart (tether radius 5) that the two guardian clusters
    // cannot possibly overlap — otherwise "no guardian of the other nest"
    // could pass by accident rather than by the provocation actually being
    // scoped to one nest.
    let nest_a = game.spawn_nest("scrapper", 100, 100);
    let nest_b = game.spawn_nest("scrapper", 300, 300);
    let guardians_a = guardians_of(&mut game, nest_a);
    let guardians_b = guardians_of(&mut game, nest_b);
    assert!(!guardians_a.is_empty(), "nest_a should have guardians");
    assert!(!guardians_b.is_empty(), "nest_b should have guardians");

    game.attack_nest(nest_a);

    for &guardian in &guardians_a {
        assert!(
            game.world.get::<Pursuing>(guardian).is_some(),
            "every guardian of the attacked nest should be provoked"
        );
    }
    for &guardian in &guardians_b {
        assert!(
            game.world.get::<Pursuing>(guardian).is_none(),
            "a guardian of an untouched nest must not be provoked"
        );
    }
}

#[test]
fn destroying_a_nest_clears_pursuing() {
    let mut game = Game::new(701, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let nest = game.spawn_nest("scrapper", 110, 110);
    let guardians = guardians_of(&mut game, nest);
    assert!(!guardians.is_empty());

    game.provoke_nest(nest);
    for &guardian in &guardians {
        assert!(game.world.get::<Pursuing>(guardian).is_some());
    }

    game.world.get_mut::<Durability>(nest).unwrap().hp = 0;
    game.despawn_nest(nest);

    assert!(
        game.world.get::<Nest>(nest).is_none(),
        "the nest itself should be gone"
    );
    for &guardian in &guardians {
        assert!(
            game.world.get::<Pursuing>(guardian).is_none(),
            "a guardian of a destroyed nest must not still be marked pursuing"
        );
        assert!(
            game.world.get::<NestGuardian>(guardian).is_none(),
            "and must no longer be tethered to a dead nest"
        );
    }
}

#[test]
fn a_guardian_respawned_at_a_besieged_nest_is_already_pursuing() {
    let mut game = Game::new(702, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();

    // Open ground the whole way, and — unlike this test before Task 4's
    // review — the nest sits inside the player's pursuit field rather
    // than off at an arbitrary far corner of the map. A guardian
    // `nest_aggro_tick` can never reach gives up on the spot (see the
    // "absent from the field" rule), which would strip the survivor's
    // `Pursuing` within the first tick or two and defeat this test's
    // premise long before the respawn timer ever fires.
    // Wide enough to hold the nest's whole tether square, not just the lane
    // between the player and it: `spawn_nest_guardian` scatters a
    // replacement anywhere within `NEST_TETHER_RADIUS` of the nest on both
    // axes, and one that lands on un-overridden terrain is absent from the
    // pursuit field and has its `Pursuing` stripped the same tick it was
    // granted. Which offset the roll produces moves with `GameRng`, so a
    // strip sized to one seed's answer is a trap for the next.
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dx in -2..=(15 + NEST_TETHER_RADIUS) {
            for dy in -NEST_TETHER_RADIUS..=NEST_TETHER_RADIUS {
                map.set_override(
                    ppos.x + dx,
                    ppos.y + dy,
                    Tile {
                        biome: Biome::OpenGrid,
                        walkable: true,
                        rock_shade: None,
                    },
                );
            }
        }
    }

    let nest_pos = Position {
        x: ppos.x + 15,
        y: ppos.y,
    };
    let nest = game
        .world
        .spawn((
            Nest {
                species: "scrapper".to_string(),
                pending_respawns: vec![NEST_RESPAWN_TICKS],
            },
            nest_pos,
            Glyph {
                ch: 'N',
                color: GlyphColor::Red,
            },
            Durability {
                hp: NEST_DURABILITY,
                max_hp: NEST_DURABILITY,
            },
        ))
        .id();
    // A guardian still standing and already marked Pursuing, as an
    // attack_nest hit would leave it — this is what makes the nest
    // "besieged" for nest_has_pursuers, independent of the respawn queue.
    // Fifteen tiles out: comfortably inside the field's reach, but far
    // enough that ten ticks of closing (one tile each) never brings it to
    // adjacency and starts a fight, which would only complicate what this
    // test is actually checking.
    let survivor = spawn_pursuing_guardian(&mut game, nest, "scrapper", nest_pos.x, nest_pos.y);

    for _ in 0..NEST_RESPAWN_TICKS {
        game.tick();
    }

    let new_guardian = guardians_of(&mut game, nest)
        .into_iter()
        .find(|&e| e != survivor)
        .expect("a replacement guardian should have spawned");
    assert!(
        game.world.get::<Pursuing>(new_guardian).is_some(),
        "a guardian respawned at a besieged nest should already be pursuing"
    );
}

#[test]
fn a_guardian_respawned_at_a_calm_nest_is_not_pursuing() {
    let mut game = Game::new(703, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let nest = game
        .world
        .spawn((
            Nest {
                species: "scrapper".to_string(),
                pending_respawns: vec![NEST_RESPAWN_TICKS],
            },
            Position { x: 130, y: 130 },
            Glyph {
                ch: 'N',
                color: GlyphColor::Red,
            },
            Durability {
                hp: NEST_DURABILITY,
                max_hp: NEST_DURABILITY,
            },
        ))
        .id();

    for _ in 0..NEST_RESPAWN_TICKS {
        game.tick();
    }

    let new_guardian = guardians_of(&mut game, nest)
        .into_iter()
        .next()
        .expect("a replacement guardian should have spawned");
    assert!(
        game.world.get::<Pursuing>(new_guardian).is_none(),
        "a guardian respawned at a calm nest should not be pursuing"
    );
}

#[test]
fn a_pursuing_guardian_does_not_also_wander() {
    let mut game = Game::new(704, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let nest = spawn_bare_nest(&mut game, 140, 140);
    let guardian = spawn_pursuing_guardian(&mut game, nest, "scrapper", 141, 140);

    // Freeze `nest_aggro_tick` itself with an unrelated active battle, so
    // `Pursuing` survives genuinely across every tick below rather than by
    // a distance trick — `nest_aggro_tick` no longer leaves a far-off
    // guardian frozen-but-still-`Pursuing` indefinitely (see the "absent
    // from the field" rule this task's review added: such a guardian now
    // gives up immediately instead), so a real battle is the only thing
    // left that can hold this state open long enough to prove
    // `wander_ai_system`'s own `Without<Pursuing>` filter is what's
    // keeping the guardian still, not a side effect of `nest_aggro_tick`'s
    // own guard.
    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);

    let before = *game.world.get::<Position>(guardian).unwrap();
    for _ in 0..10 {
        game.tick();
    }
    let after = *game.world.get::<Position>(guardian).unwrap();

    assert_eq!(
        before, after,
        "wander_ai_system must exclude a Pursuing guardian even while nest_aggro_tick is \
         separately frozen by the battle — otherwise the two systems could double-move it \
         once nest_aggro_tick resumes"
    );
    assert!(
        game.world.get::<Pursuing>(guardian).is_some(),
        "the guardian should still be genuinely Pursuing throughout — that's what this test \
         is checking wander's exclusion against"
    );
}

#[test]
fn decompiling_a_pursuing_guardian_strips_the_marker() {
    let mut game = Game::new(705, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let nest = game
        .world
        .spawn((
            Nest {
                species: "scrapper".to_string(),
                pending_respawns: Vec::new(),
            },
            Position { x: 150, y: 150 },
            Glyph {
                ch: 'N',
                color: GlyphColor::Red,
            },
            Durability {
                hp: NEST_DURABILITY,
                max_hp: NEST_DURABILITY,
            },
        ))
        .id();
    let guardian = game
        .world
        .spawn((
            Creature {
                species: "scrapper".to_string(),
            },
            Hostile,
            WanderAi::default(),
            NestGuardian { nest },
            Pursuing,
            Position { x: 151, y: 150 },
            Stats {
                hp: 1,
                max_hp: 10,
                atk: 1,
                mitigation: 1,
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![guardian]);
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::ICE_BREAKER), 50);
    game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;

    for _ in 0..50 {
        if game.world.get::<Tamed>(guardian).is_some() {
            break;
        }
        player_decompiles(&mut game);
    }

    assert!(
        game.world.get::<Tamed>(guardian).is_some(),
        "the capture roll should land within 50 attempts at this skill/potency"
    );
    assert!(
        game.world.get::<Pursuing>(guardian).is_none(),
        "taming a pursuing guardian should strip the Pursuing marker along with NestGuardian"
    );
}

/// The per-tick pursuit step (`Game::nest_aggro_tick`) that wires
/// `Pursuing` (provocation) to `pursuit_field` (routing) together — this
/// is the half of nest aggression that actually moves a guardian and
/// starts a fight.
fn chebyshev(a: Position, b: Position) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

#[test]
fn a_pursuer_closes_on_the_player_each_tick() {
    let mut game = Game::new(710, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();

    // A generous hand-carved lane rather than hoping procedurally
    // generated terrain happens to be open between the two points.
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dx in -2..=12 {
            for dy in -2..=2 {
                map.set_override(
                    ppos.x + dx,
                    ppos.y + dy,
                    Tile {
                        biome: Biome::OpenGrid,
                        walkable: true,
                        rock_shade: None,
                    },
                );
            }
        }
    }
    assert!(
        game.world
            .resource_mut::<WorldMap>()
            .tile(ppos.x + 10, ppos.y)
            .walkable,
        "the lane precondition this test depends on"
    );

    let nest = spawn_bare_nest(&mut game, ppos.x + 10, ppos.y);
    let guardian = spawn_pursuing_guardian(&mut game, nest, "scrapper", ppos.x + 10, ppos.y);

    let before = chebyshev(ppos, *game.world.get::<Position>(guardian).unwrap());
    game.tick();
    let after = chebyshev(ppos, *game.world.get::<Position>(guardian).unwrap());

    assert_eq!(
        before - after,
        NEST_PURSUIT_STEPS_PER_TICK as i32,
        "a pursuer should close on the player by exactly NEST_PURSUIT_STEPS_PER_TICK each tick"
    );
}

#[test]
fn a_pursuer_that_reaches_the_player_starts_a_battle() {
    let mut game = Game::new(711, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dx in -2..=4 {
            for dy in -2..=2 {
                map.set_override(
                    ppos.x + dx,
                    ppos.y + dy,
                    Tile {
                        biome: Biome::OpenGrid,
                        walkable: true,
                        rock_shade: None,
                    },
                );
            }
        }
    }

    let nest = spawn_bare_nest(&mut game, ppos.x + 2, ppos.y);
    spawn_pursuing_guardian(&mut game, nest, "scrapper", ppos.x + 2, ppos.y);

    assert!(!game.has_active_battle());
    game.tick();
    assert!(
        game.has_active_battle(),
        "a pursuer that reaches the player should start a battle within the tick it arrives"
    );
}

#[test]
fn the_battle_a_pursuer_starts_includes_its_packmates() {
    let mut game = Game::new(712, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Both `max_group_size` and `max_enemy_groups` are 1 right at the
    // danger origin (by design — see `Game::max_group_size`'s doc), which
    // would truncate this fight back down to a single member regardless of
    // how the pack was gathered. `multi_group_ground` is ground far enough
    // out that a full `MAX_ENEMY_GROUPS` fight is allowed there.
    let (gx, gy) = multi_group_ground(&mut game);
    let player = game.player_entity();
    *game.world.get_mut::<Position>(player).unwrap() = Position { x: gx, y: gy };
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dx in -2..=2 {
            for dy in -2..=2 {
                map.set_override(
                    gx + dx,
                    gy + dy,
                    Tile {
                        biome: Biome::OpenGrid,
                        walkable: true,
                        rock_shade: None,
                    },
                );
            }
        }
    }

    let nest = spawn_bare_nest(&mut game, gx, gy);
    // Two different species so a single-species group cap can't be the
    // reason both end up in the fight — each gets its own group, and
    // `multi_group_ground` guarantees more than one group is allowed here.
    let scrapper = spawn_pursuing_guardian(&mut game, nest, "scrapper", gx + 1, gy);
    let crawler = spawn_pursuing_guardian(&mut game, nest, "crawler", gx + 1, gy + 1);

    game.tick();

    assert!(game.has_active_battle());
    let members: Vec<Entity> = game
        .world
        .resource::<BattleState>()
        .groups
        .iter()
        .flat_map(|g| g.members.iter().copied())
        .collect();
    assert!(
        members.contains(&scrapper) && members.contains(&crawler),
        "the battle a pursuer starts should pull in the packmate standing beside it \
         (gather_pack), not just the one that reached the player; found {members:?}"
    );
}

#[test]
fn a_pursuer_beyond_the_leash_gives_up() {
    let mut game = Game::new(713, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();

    // Both the nest and the guardian sit inside the player's search box
    // (radius NEST_AGGRO_LEASH_RADIUS + NEST_PATH_SEARCH_MARGIN = 20), on a
    // hand-carved lane — the point of this test is that the *leash* rule
    // (distance from the nest) fires, not the ordinary out-of-field rule
    // (distance from the player), so both must be within reach for the
    // leash to be the only thing that can be stripping `Pursuing`. Before
    // this rewrite the nest and guardian sat ~200 tiles from the player, so
    // *every* run of this test was passing on the field-absence rule
    // instead — a reviewer who replaced the leash filter's body with
    // `false` still saw all 902 engine tests go green.
    let search_box = NEST_AGGRO_LEASH_RADIUS + NEST_PATH_SEARCH_MARGIN;
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dx in -2..=search_box {
            for dy in -2..=2 {
                map.set_override(
                    ppos.x + dx,
                    ppos.y + dy,
                    Tile {
                        biome: Biome::OpenGrid,
                        walkable: true,
                        rock_shade: None,
                    },
                );
            }
        }
    }

    let nest_pos = Position {
        x: ppos.x + 2,
        y: ppos.y,
    };
    let nest = spawn_bare_nest(&mut game, nest_pos.x, nest_pos.y);
    // `search_box - 2` (18): past NEST_AGGRO_LEASH_RADIUS (15) from the nest
    // two tiles east, but still inside the player's own search box —
    // the leash must be what strips `Pursuing` here, not simply being out
    // of the field's reach.
    let start = Position {
        x: ppos.x + search_box - 2,
        y: ppos.y,
    };
    let guardian = spawn_pursuing_guardian(&mut game, nest, "scrapper", start.x, start.y);
    assert!(
        chebyshev(start, nest_pos) > NEST_AGGRO_LEASH_RADIUS,
        "test premise: past the leash radius from the nest"
    );
    assert!(
        chebyshev(start, ppos) <= search_box,
        "test premise: inside the player's own search box — so a pass here can't be the \
         ordinary out-of-field rule in disguise"
    );

    game.tick();

    assert!(
        game.world.get::<Pursuing>(guardian).is_none(),
        "a pursuer past NEST_AGGRO_LEASH_RADIUS from its own nest should give up, even though \
         it's still well within the player's own search box"
    );
    assert_eq!(
        *game.world.get::<Position>(guardian).unwrap(),
        start,
        "giving up on the leash must not also have taken a step toward the player — \
         the leash check runs before the field is even built"
    );
}

#[test]
fn pursuers_never_step_onto_the_base_platform() {
    let mut game = Game::new(714, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();
    let half = MAX_BUILD_DISTANCE_FROM_HOME;

    // Open ground generous enough that a detour around the platform's edge
    // has somewhere to go, sized off `half` rather than a guessed constant
    // so it stays correct if the build radius ever changes.
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dx in -(half + 4)..=(half * 2 + 10) {
            for dy in -(half + 10)..=(half + 10) {
                map.set_override(
                    ppos.x + dx,
                    ppos.y + dy,
                    Tile {
                        biome: Biome::OpenGrid,
                        walkable: true,
                        rock_shade: None,
                    },
                );
            }
        }
    }
    // Stamped after the walkable override (so Platform wins inside the
    // slab) but before the nest and pursuer are placed. Written directly
    // through `set_override` rather than `Game::stamp_platform`, which
    // retired with `resources::Platform` — what this test exercises is
    // `pursuit_field`'s `Biome::Platform` exclusion, not the (now gone)
    // production path that used to lay the tile, so any Platform region
    // does. Sits east of the player, not on top of it: `half - 3` would
    // have put the player's own tile inside the slab.
    let platform_center = (ppos.x + half + 1, ppos.y);
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dx in -half..=half {
            for dy in -half..=half {
                map.set_override(
                    platform_center.0 + dx,
                    platform_center.1 + dy,
                    Tile {
                        biome: Biome::Platform,
                        walkable: true,
                        rock_shade: None,
                    },
                );
            }
        }
    }

    // Wall off the southern detour entirely, so the only way around the
    // slab is north — where the nest sits — instead of leaving dijkstra a
    // coin-flip between two equally short routes. The first version of
    // this test left both open: the field routed the pursuer south half
    // the time, which pulled it more than `NEST_AGGRO_LEASH_RADIUS` from a
    // nest placed north of the slab, stripped `Pursuing`, and handed it to
    // `wander_ai_system` — which has no Platform exclusion — long before it
    // ever reached the player.
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dx in -(half + 4)..=(half * 2 + 10) {
            for dy in -(half + 10)..=-(half + 1) {
                map.set_override(
                    ppos.x + dx,
                    ppos.y + dy,
                    Tile {
                        biome: Biome::DataVoid,
                        walkable: false,
                        rock_shade: None,
                    },
                );
            }
        }
    }

    // The pursuer sits on the far side of the slab, directly in line with
    // the player, so the straight route is blocked and reaching adjacency
    // at all means detouring around the platform's edge — north, per the
    // wall above. The nest sits on that forced corridor rather than at the
    // pursuer's own tile: putting it there keeps both the start and the
    // final adjacent-to-player position within 8 of it (against a
    // 15-tile leash), so the leash can't be what stops the chase partway
    // through the one detour this test leaves available.
    let guardian_pos = (platform_center.0 + half + 1, ppos.y);
    let nest_pos = (platform_center.0, ppos.y + half + 1);
    let nest = spawn_bare_nest(&mut game, nest_pos.0, nest_pos.1);
    let guardian =
        spawn_pursuing_guardian(&mut game, nest, "scrapper", guardian_pos.0, guardian_pos.1);

    for _ in 0..80 {
        if game.has_active_battle() {
            break;
        }
        game.tick();
        let pos = *game.world.get::<Position>(guardian).unwrap();
        let biome = game
            .world
            .resource_mut::<WorldMap>()
            .tile(pos.x, pos.y)
            .biome;
        assert_ne!(
            biome,
            Biome::Platform,
            "a pursuer must never step onto the base platform, but stands at {pos:?}"
        );
    }

    assert!(
        game.has_active_battle(),
        "the pursuer should have detoured around the platform and reached the player within \
         80 ticks — if it never does, this test can't tell 'never stepped on Platform' apart \
         from 'never stepped at all'"
    );
}

#[test]
fn nest_aggro_tick_is_a_no_op_during_a_battle() {
    let mut game = Game::new(715, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();

    // A walkable lane, the same as tests 1/2/5 carve — otherwise a
    // guardian standing on unwalkable natural terrain would already be
    // absent from the field and wouldn't have moved even without the
    // battle guard, and this test would pass for the wrong reason.
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dx in -2..=4 {
            for dy in -2..=2 {
                map.set_override(
                    ppos.x + dx,
                    ppos.y + dy,
                    Tile {
                        biome: Biome::OpenGrid,
                        walkable: true,
                        rock_shade: None,
                    },
                );
            }
        }
    }

    // Close enough that, absent the battle guard, it would engage or at
    // least step this very tick — so this test would actually catch a
    // missing `has_active_battle` check rather than passing vacuously.
    let nest = spawn_bare_nest(&mut game, ppos.x + 3, ppos.y);
    let guardian = spawn_pursuing_guardian(&mut game, nest, "scrapper", ppos.x + 2, ppos.y);
    let before = *game.world.get::<Position>(guardian).unwrap();

    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);

    game.tick();

    assert_eq!(
        *game.world.get::<Position>(guardian).unwrap(),
        before,
        "nest_aggro_tick must not move a pursuer while a battle is already running"
    );
}

#[test]
fn nest_aggro_tick_is_a_no_op_while_underground() {
    let mut game = Game::new(716, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();

    // Adjacent to the surface entrance tile. `Position` stays pinned there
    // for as long as the party is underground (see CLAUDE.md's
    // load-bearing-seams note on `Locale::Stack`), so this pursuer would
    // engage this very tick if `nest_aggro_tick` didn't know to leave that
    // surface `Position` alone while the party is four frames down.
    let nest = spawn_bare_nest(&mut game, ppos.x + 1, ppos.y);
    let guardian = spawn_pursuing_guardian(&mut game, nest, "scrapper", ppos.x + 1, ppos.y);

    game.enter_stack(ppos.x, ppos.y);
    assert!(
        game.is_underground(),
        "test premise: the party must actually be underground"
    );

    game.tick();

    assert!(
        !game.has_active_battle(),
        "nest_aggro_tick must not fight the player's surface Position while the party is \
         underground"
    );
    assert_eq!(
        *game.world.get::<Position>(guardian).unwrap(),
        Position {
            x: ppos.x + 1,
            y: ppos.y
        },
        "a pursuer must not move while the party is underground either"
    );
}

/// Pins the deviation recorded in `nest_aggro_tick`'s "field-absence"
/// branch (and the matching "Implementation note" in
/// docs/superpowers/archive/specs/2026-08-03-nest-aggression-design.md): standing
/// inside the base slab empties the pursuit field outright, for *any*
/// pursuer, not just one already too far away to matter. Standing on the
/// platform's interior makes every one of the player's own neighbours
/// `Biome::Platform`, so `pursuit_field`'s search can't take a single step
/// out of the player's own tile — the same shape as
/// `pursuit.rs::an_enclosed_origin_yields_a_field_of_just_itself`. A
/// guardian well inside both the leash and the search box, on open ground
/// the whole way, reads as absent from that field anyway and loses
/// `Pursuing` — the platform is what did that, not distance, which is what
/// the previous version of this test (guardian 40 tiles out, past the
/// 20-tile search box on distance alone) failed to isolate. Verified: with
/// the Platform stamp below removed, this test fails; restored, it passes.
#[test]
fn standing_inside_the_base_slab_strips_pursuing_from_a_reachable_guardian() {
    let mut game = Game::new(717, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();

    // Open ground the whole way, so — absent the platform — this guardian
    // would be found in the field and close on the player like any other
    // pursuer (see `a_pursuer_closes_on_the_player_each_tick`). The
    // platform has to be what empties the field here, not natural terrain
    // blocking the route.
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dx in -2..=12 {
            for dy in -2..=2 {
                map.set_override(
                    ppos.x + dx,
                    ppos.y + dy,
                    Tile {
                        biome: Biome::OpenGrid,
                        walkable: true,
                        rock_shade: None,
                    },
                );
            }
        }
    }

    // Written directly through `set_override` rather than
    // `Game::stamp_platform`, which retired with `resources::Platform` —
    // same substitution as `pursuers_never_step_onto_the_base_platform`
    // above, at the same radius the old stamp laid.
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        let r = MAX_BUILD_DISTANCE_FROM_HOME;
        for dx in -r..=r {
            for dy in -r..=r {
                map.set_override(
                    ppos.x + dx,
                    ppos.y + dy,
                    Tile {
                        biome: Biome::Platform,
                        walkable: true,
                        rock_shade: None,
                    },
                );
            }
        }
    }
    assert_eq!(
        game.world
            .resource_mut::<WorldMap>()
            .tile(ppos.x, ppos.y)
            .biome,
        Biome::Platform,
        "test premise: the player must actually be standing inside the slab"
    );

    // 10 tiles out: well inside both NEST_AGGRO_LEASH_RADIUS (15) and the
    // player's own search box (20), and outside the platform's own
    // MAX_BUILD_DISTANCE_FROM_HOME (7) footprint, so the guardian's own
    // tile isn't Platform either — only the player's immediate
    // neighbourhood needs to be.
    let nest = spawn_bare_nest(&mut game, ppos.x + 10, ppos.y);
    let guardian = spawn_pursuing_guardian(&mut game, nest, "scrapper", ppos.x + 10, ppos.y);

    game.tick();

    assert!(
        game.world.get::<Pursuing>(guardian).is_none(),
        "standing inside the slab should empty the pursuit field even for a guardian that \
         would otherwise be well within reach"
    );
}

/// The nest cache: `Game::grant_nest_cache`, called from `attack_nest`'s
/// `destroyed` branch. Content comes entirely from the nest's `SpeciesDef` —
/// these tests lean on shipped species rather than hardcoded item ids
/// wherever the range rolls involved make that reliable, and mod in a single
/// throwaway species only where a shipped one's equipment chance is too low
/// to force deterministically.
fn one_shot_nest(game: &mut Game, nest: Entity) {
    let player = game.player_entity();
    // Comfortably above NEST_DURABILITY (60), so a single attack_nest call
    // is always lethal regardless of the player's own starting atk.
    game.world.get_mut::<Stats>(player).unwrap().atk = 1000;
    game.attack_nest(nest);
}

#[test]
fn destroying_a_nest_grants_its_species_work_resource() {
    let mut game = Game::new(720, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Scrapper is the `can_nest` species carrying a `work_resource` at all —
    // crawler and trojan have none, and using one of those would make this
    // test vacuous. *Which* resource is read off the species file rather
    // than named here: this test asserted `power_cell` until 2026-08-04 moved
    // the Scrapper to Core Fragments, and the claim it exists to make is
    // "the nest pays its species' resource", not which one that is.
    let resource = game
        .world
        .resource::<SpeciesDb>()
        .get("scrapper")
        .and_then(|s| s.work_resource.clone())
        .expect("the nesting species used here must carry a work_resource");
    let nest = game.spawn_nest("scrapper", 400, 400);
    let before = held(&game, &resource);

    one_shot_nest(&mut game, nest);

    let after = held(&game, &resource);
    let minimum = NEST_CACHE_WORK_RESOURCE_MULT * WORK_RESOURCE_DROP.start();
    assert!(
        after >= before + minimum,
        "destroying the nest should have granted at least {minimum} {}, went from \
         {before} to {after}",
        resource.as_str()
    );
}

#[test]
fn destroying_a_nest_grants_trade_currency() {
    let mut game = Game::new(721, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let currency = game.trade_currency();
    let nest = game.spawn_nest("scrapper", 410, 410);
    let before = held(&game, &currency);

    one_shot_nest(&mut game, nest);

    let after = held(&game, &currency);
    // Zone 1 (the default here), so NEST_CACHE_CREDIT_ZONE_BONUS
    // contributes nothing — the floor is the bare range roll.
    let minimum = *NEST_CACHE_CREDITS.start();
    assert!(
        after >= before + minimum,
        "destroying the nest should have granted at least {minimum} trade currency, went from \
         {before} to {after}"
    );
}

#[test]
fn destroying_a_nest_grants_no_craft_currency() {
    let mut game = Game::new(7211, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let currency = game.craft_currency();
    let nest = game.spawn_nest("scrapper", 411, 411);
    let before = held(&game, &currency);

    one_shot_nest(&mut game, nest);

    assert_eq!(
        held(&game, &currency),
        before,
        "the breaching currency is STACK_BOSS_PORTAL_FRAGMENT_DROP's alone — a nest cleared \
         on the surface must not advance the party toward the next zone"
    );
}

/// Which of `seeds` left an orphan behind, and what species each one was.
/// `NEST_ORPHAN_CHANCE` is a coin flip, so a single seed proves only its
/// own outcome — sweeping a fixed list keeps the assertions deterministic
/// while letting them speak about the roll rather than about one draw.
fn nest_orphans_across(seeds: std::ops::Range<u32>, fill_roster: bool) -> Vec<Option<String>> {
    seeds
        .map(|seed| {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            if fill_roster {
                while game.pet_count() < game.pet_capacity() {
                    spawn_tamed(&mut game, 10, 1);
                }
            }
            let roster = |game: &Game| -> Vec<(Entity, String)> {
                let player = game.player_entity();
                game.world
                    .iter_entities()
                    .filter(|e| e.get::<Tamed>().is_some_and(|t| t.owner == player))
                    .filter_map(|e| e.get::<Creature>().map(|c| (e.id(), c.species.clone())))
                    .collect()
            };
            let before = roster(&game);
            let nest = game.spawn_nest("scrapper", 440, 440);
            one_shot_nest(&mut game, nest);
            roster(&game)
                .into_iter()
                .find(|(e, _)| !before.iter().any(|(seen, _)| seen == e))
                .map(|(_, species)| species)
        })
        .collect()
}

#[test]
fn destroying_a_nest_sometimes_leaves_an_orphan_of_its_own_species() {
    let outcomes = nest_orphans_across(760..800, false);

    let adopted: Vec<&String> = outcomes.iter().flatten().collect();
    assert!(
        !adopted.is_empty(),
        "NEST_ORPHAN_CHANCE is what a nest is cleared for — 40 seeds paying none of them \
         means the roll never fires"
    );
    assert!(
        outcomes.iter().any(|o| o.is_none()),
        "the orphan is chanced, not guaranteed — 40 seeds all paying one means the roll \
         is inert and the constant is doing nothing"
    );
    assert!(
        adopted.iter().all(|s| s.as_str() == "scrapper"),
        "an orphan is of the nest's own species, so hunting the nest of the program you \
         want is a real choice; got {adopted:?}"
    );
}

#[test]
fn a_full_roster_loses_the_nest_orphan() {
    let outcomes = nest_orphans_across(760..800, true);

    assert!(
        outcomes.iter().all(|o| o.is_none()),
        "a roster already at pet_capacity has nowhere to put an orphan, and adopt_program \
         must not be reached at all: {outcomes:?}"
    );
}

#[test]
fn a_lost_nest_orphan_says_so() {
    // The one seed of the sweep above known to roll a hit, run twice: the
    // point is that the *same* roll reads differently to a player with room
    // and a player without, rather than silently paying nothing.
    let hit = (760..800)
        .zip(nest_orphans_across(760..800, false))
        .find_map(|(seed, outcome)| outcome.map(|_| seed))
        .expect("the sweep above already asserts at least one seed hits");

    let mut game = Game::new(hit, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    while game.pet_count() < game.pet_capacity() {
        spawn_tamed(&mut game, 10, 1);
    }
    let nest = game.spawn_nest("scrapper", 440, 440);
    one_shot_nest(&mut game, nest);

    assert!(
        game.message_log(200)
            .into_iter()
            .any(|e| e.text.contains("no room")),
        "a full roster must be told what it just lost, not left to notice nothing arrived: \
         {:?}",
        game.message_log(200)
    );
}

#[test]
fn a_non_lethal_hit_on_a_nest_grants_nothing() {
    let mut game = Game::new(722, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let nest = game.spawn_nest("scrapper", 420, 420);
    let before = game.world.get::<Inventory>(player).unwrap().items.clone();

    game.attack_nest(nest);

    assert!(
        game.world.get::<Durability>(nest).unwrap().hp > 0,
        "test premise: a fresh player's ordinary hit must not one-shot NEST_DURABILITY"
    );
    let after = game.world.get::<Inventory>(player).unwrap().items.clone();
    assert_eq!(
        before, after,
        "a non-lethal hit on a nest must not grant any part of the cache"
    );
}

#[test]
fn a_deeper_zone_pays_a_larger_nest_cache() {
    let seed = 723;
    // Same seed, same species, same tile for both runs, so the two games
    // consume GameRng identically right up to the currency roll itself —
    // ZoneLevel doesn't change how many draws spawn_nest/spawn_wild_creature
    // make (see spawn_wild_creature_scaled), only the resulting stats. That
    // makes this a comparison of the same underlying roll plus a different
    // zone bonus, not two independent unseeded rolls.
    let currency_gained_at = |zone: u32| {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world.resource_mut::<ZoneLevel>().0 = zone;
        let currency = game.trade_currency();
        let nest = game.spawn_nest("scrapper", 430, 430);
        let before = held(&game, &currency);
        one_shot_nest(&mut game, nest);
        held(&game, &currency) - before
    };

    let zone_1 = currency_gained_at(1);
    let zone_4 = currency_gained_at(4);
    let minimum_gap = 3 * NEST_CACHE_CREDIT_ZONE_BONUS;
    assert!(
        zone_4 >= zone_1 + minimum_gap,
        "a nest cleared at zone 4 should pay at least {minimum_gap} more trade currency than \
         the same nest at zone 1 (zone_1={zone_1}, zone_4={zone_4})"
    );
}

#[test]
fn destroying_a_nest_rolls_its_species_gear_table_repeatedly() {
    // No shipped can_nest species has an equipment chance high enough to
    // force a repeated-roll outcome without new RNG-forcing plumbing (see
    // the task brief's fallback clause), so this mods in one whose
    // equipment_drop chance is 1.0 — every one of NEST_CACHE_EQUIPMENT_ROLLS
    // passes then lands for certain, which is the only way to observe
    // ROLLS=3 differ from ROLLS=1 without touching the engine.
    let dir = modded_assets_dir(
        "nest_cache_gear",
        &[],
        &[],
        &[(
            "nest_cache_test.ron",
            r#"(
                id: "nest_cache_test",
                name: "Cache Test",
                glyph: 'X',
                color: Yellow,
                base_hp: 50,
                base_atk: 5,
                base_mitigation: 2,
                taming_difficulty: 0.5,
                habitats: [],
                moves: [],
                work_resource: None,
                equipment_drop: Some(("shiv_routine", 1.0)),
                can_nest: true,
            )"#,
        )],
        &[],
        &[],
    );
    let mut game = Game::new(724, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    let gear = ItemId::from("shiv_routine");
    let nest = game.spawn_nest("nest_cache_test", 440, 440);
    // `held_any`, because the question is how many rolls landed and not what
    // tier they came up: `grant_gear_drop` files a rare copy in `GearCopies`
    // rather than `Inventory`, so counting the plain store alone loses one.
    let before = held_any(&game, &gear);

    one_shot_nest(&mut game, nest);

    let after = held_any(&game, &gear);
    assert!(
        after >= before + 2,
        "a guaranteed gear roll repeated NEST_CACHE_EQUIPMENT_ROLLS times should yield more \
         than one copy of it — the only observable difference from a single pass — went from \
         {before} to {after}"
    );
}

#[test]
fn the_cache_lines_are_loot_kind() {
    let mut game = Game::new(725, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // scrapper's work_resource guarantees the resource-drop cache line lands
    // even on an unseeded roll (WORK_RESOURCE_DROP's minimum is > 0 and the
    // Buffer is uncapped), so this doesn't need to force any range roll —
    // unlike the gear-table test above, which does.
    let nest = game.spawn_nest("scrapper", 450, 450);

    one_shot_nest(&mut game, nest);

    let log = game.message_log(200);
    let cache_lines: Vec<_> = log
        .iter()
        .filter(|e| e.text.contains("wreckage") || e.text.contains("cache"))
        .collect();
    assert!(
        !cache_lines.is_empty(),
        "test premise: a nest cache line should have been logged, got: {log:?}"
    );
    for e in &cache_lines {
        assert_eq!(
            e.kind,
            MessageKind::Loot,
            "cache line {:?} must be MessageKind::Loot so it survives \
             retain_outcomes_since_battle and follows the player onto the map, not \
             MessageKind::Info which would be pruned when the swarm fight ends",
            e.text
        );
    }
}

// ---------------------------------------------------------------------------
// Sector traits reaching world generation
// ---------------------------------------------------------------------------
//
// Three sites build a `WorldMap` for real play — `Game::new`, `Game::load`
// and `enter_next_zone` — and all three must derive through
// `sectors::for_zone`. Which sector a zone gets is `tests::sectors`' subject;
// what is under test here is the wiring, so these drive scratch installs
// holding one sector or none rather than searching the shipped pool for a
// seed.

/// Every zone past the first is Cold Storage in this install: Deadlock
/// over most of the ground, holes exactly where a neutral sector puts them.
const ONLY_COLD: &str = r#"(
    id: "cold_storage",
    name: "Cold Storage",
    description: "Long-idle allocations, frost-locked and slow to answer.",
    shape: (deadlock_temperature: 1.15),
    palette: (ground_hue: 200.0, hazard_hue: 12.0),
)"#;

/// Walks the player onto a zone portal, which is what a breach is.
fn breach(game: &mut Game) {
    breach_through_a_portal(game);
}

/// How many tiles of `biome` a map has around its origin.
fn count_biome(map: &mut WorldMap, biome: Biome) -> usize {
    (-32..32)
        .flat_map(|y| (-32..32).map(move |x| (x, y)))
        .filter(|&(x, y)| map.tile(x, y).biome == biome)
        .count()
}

/// Zone 1 is neutral through the real constructor, not just through
/// `for_zone`. The opening ring's roster depends on it.
#[test]
fn a_new_game_generates_zone_one_at_the_neutral_shape() {
    let game = Game::new(4242, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(
        game.world.resource::<WorldMap>().shape(),
        SectorShape::NEUTRAL
    );
}

/// The wiring test: a breach into a Static-Field sector must put Static
/// Field on the ground. It is a crisp assertion because the latitude falloff
/// leaves a neutral sector with *no* Deadlock at all near the origin, so
/// this cannot pass on terrain that was already there.
#[test]
fn breaching_into_a_cold_sector_generates_its_biome() {
    let assets = assets_dir_with_sectors("zone_cold", &[("cold.ron", ONLY_COLD)]);
    let mut game = Game::new(4242, DifficultyMode::Forgiving, &assets).unwrap();
    assert_eq!(
        count_biome(
            game.world.resource_mut::<WorldMap>().as_mut(),
            Biome::Deadlock
        ),
        0,
        "zone 1 is neutral, and a neutral sector generates no Deadlock here"
    );

    breach(&mut game);
    assert_eq!(game.player_status().zone, 2);

    let cold = count_biome(
        game.world.resource_mut::<WorldMap>().as_mut(),
        Biome::Deadlock,
    );
    assert!(
        cold > 500,
        "zone 2 generated {cold} Deadlock tiles of 4096 — the sector's shape \
         is not reaching `enter_next_zone`"
    );
}

/// The roster moves with the biome mix, and it moves for free: there is no
/// species-pool knob, because `Game::habitat_pools` already filters by the
/// tile's biome. A second knob pointing at the same outcome could disagree
/// with this one.
///
/// Asserted against the *same seed under a neutral install* rather than
/// against an absolute count, since what is claimed is a shift and not a
/// number.
#[test]
fn a_cold_sectors_wild_population_leans_on_deadlock_species() {
    let count_cold_dwellers = |assets: &std::path::Path| {
        let mut game = Game::new(4242, DifficultyMode::Forgiving, assets).unwrap();
        breach(&mut game);
        let species: Vec<String> = {
            let mut query = game.world.query_filtered::<&Creature, With<Hostile>>();
            query.iter(&game.world).map(|c| c.species.clone()).collect()
        };
        let db = game.species_defs();
        species
            .iter()
            .filter(|s| {
                db.iter()
                    .find(|d| &d.id == *s)
                    .is_some_and(|d| d.habitats.contains(&Biome::Deadlock))
            })
            .count()
    };

    let cold = assets_dir_with_sectors("zone_roster_cold", &[("cold.ron", ONLY_COLD)]);
    let neutral = assets_dir_with_sectors("zone_roster_neutral", &[]);
    let in_cold = count_cold_dwellers(&cold);
    let in_neutral = count_cold_dwellers(&neutral);

    assert!(
        in_cold > in_neutral,
        "a Deadlock sector spawned {in_cold} Static-Field-dwelling programs \
         against a neutral sector's {in_neutral} — the biome mix is not reaching \
         `habitat_pools`"
    );
}

/// `Game::load` must rebuild the map at the same shape `enter_next_zone`
/// built it with. Reconstructing at a different one regenerates every
/// unwalked chunk differently, which can strand a party inside rock — the
/// same class of bug the Stack-frame RNG rule exists to prevent.
///
/// Through a real save file rather than a recomputation in the same process,
/// because what is being claimed is that the two saved numbers are enough.
#[test]
fn a_sectors_shape_survives_a_save_and_load_round_trip() {
    let assets = assets_dir_with_sectors("zone_roundtrip", &[("cold.ron", ONLY_COLD)]);
    let mut game = Game::new(4242, DifficultyMode::Forgiving, &assets).unwrap();
    breach(&mut game);

    let shape = game.world.resource::<WorldMap>().shape();
    let before: Vec<Biome> = {
        let mut map = game.world.resource_mut::<WorldMap>();
        // Deliberately far from anywhere the party has walked, so these
        // chunks are regenerated by the load rather than restored from the
        // override overlay.
        (0..64).map(|i| map.tile(400 + i, -350 - i).biome).collect()
    };

    let path = std::env::temp_dir().join(format!(
        "feral_processes_sector_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded.world.resource::<WorldMap>().shape(),
        shape,
        "the load rebuilt the map at a different shape"
    );
    let after: Vec<Biome> = {
        let mut map = loaded.world.resource_mut::<WorldMap>();
        (0..64).map(|i| map.tile(400 + i, -350 - i).biome).collect()
    };
    assert_eq!(before, after, "unwalked terrain regenerated differently");
}

/// Absence is supported, at every zone rather than only at zone 1. Deleting
/// `assets/sectors/` restores the pre-sector game exactly, the way deleting
/// `assets/affixes/` or the enemy policy does — and an omission is invisible
/// without a test saying so.
#[test]
fn with_no_sectors_installed_every_zone_generates_at_the_neutral_shape() {
    let assets = assets_dir_with_sectors("zone_no_sectors", &[]);
    let mut game = Game::new(4242, DifficultyMode::Forgiving, &assets).unwrap();

    for zone in 2..=5 {
        breach(&mut game);
        assert_eq!(game.player_status().zone, zone);

        let seed = game.world.resource::<WorldMap>().seed();
        assert_eq!(
            game.world.resource::<WorldMap>().shape(),
            SectorShape::NEUTRAL,
            "zone {zone} is not neutral"
        );
        // The seed is read back off the live map rather than recomputed, so
        // this does not carry a second copy of how a breach advances it.
        let mut reference = WorldMap::new(seed);
        let live: Vec<Biome> = {
            let mut map = game.world.resource_mut::<WorldMap>();
            (-40..40).map(|i| map.tile(i, i * 2).biome).collect()
        };
        let expected: Vec<Biome> = (-40..40).map(|i| reference.tile(i, i * 2).biome).collect();
        assert_eq!(live, expected, "zone {zone} did not generate as neutral");
    }
}

/// A breach says *where* you have landed, not only how hard it is. The
/// level line is untouched: a neutral sector must read exactly as it did
/// before sectors existed, which is the same absence-is-supported property
/// the generation side has.
#[test]
fn breaching_into_a_named_sector_announces_it() {
    let assets = assets_dir_with_sectors("zone_announce", &[("cold.ron", ONLY_COLD)]);
    let mut game = Game::new(4242, DifficultyMode::Forgiving, &assets).unwrap();
    breach(&mut game);

    let log = game
        .message_log(200)
        .into_iter()
        .map(|l| l.text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        log.contains("level 2 sector"),
        "the level line should be unchanged: {log}"
    );
    assert!(
        log.contains("Cold Storage"),
        "the sector's name should be announced: {log}"
    );
    assert!(
        log.contains("frost-locked"),
        "the sector's description should be announced: {log}"
    );
}

#[test]
fn breaching_into_a_neutral_sector_logs_only_the_level_line() {
    let assets = assets_dir_with_sectors("zone_announce_neutral", &[]);
    let mut game = Game::new(4242, DifficultyMode::Forgiving, &assets).unwrap();
    breach(&mut game);

    let breach_lines: Vec<String> = game
        .message_log(200)
        .into_iter()
        .map(|l| l.text)
        .filter(|m| m.contains("breach the portal"))
        .collect();
    assert_eq!(
        breach_lines.len(),
        1,
        "a neutral sector should add no second line: {breach_lines:?}"
    );
}

/// The base is out of phase, not on the zone surface — a breach must not
/// touch a `Structure`'s `Position` at all, in either direction.
///
/// `find_walkable_start` always resolves `(0, 0)` on every generated map
/// (checked directly: every seed 0..30 breaches to spawn `(0, 0)`), and the
/// Home is always founded at `BASE_EXIT_CELL`, itself `(0, 0)` — so under an
/// ordinary breach a structure's absolute position and its offset from Home
/// are the same number, and the old offset-rebuild block reproduced the
/// right answer by coincidence. Comparing to Home, or trusting an
/// unperturbed fixture, would silently pass with that block still in place.
/// This test defeats both: the fixture is hand-moved onto coordinates that
/// don't touch the origin, so the deleted block's `spawn + offset` write
/// would land somewhere else entirely, and every assertion below reads an
/// absolute coordinate, never a delta from Home.
#[test]
fn breaching_leaves_every_structures_absolute_position_untouched() {
    let mut game = Game::new(940, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (home, node) = build_a_base(&mut game);
    game.world.get_mut::<Position>(home).unwrap().x = 12;
    game.world.get_mut::<Position>(home).unwrap().y = -7;
    game.world.get_mut::<Position>(node).unwrap().x = 20;
    game.world.get_mut::<Position>(node).unwrap().y = 30;
    let home_before = *game.world.get::<Position>(home).unwrap();
    let node_before = *game.world.get::<Position>(node).unwrap();

    game.enter_next_zone();

    assert!(
        game.world.get_entity(home).is_ok(),
        "the Home is not zone-local — it must survive the breach"
    );
    assert!(
        game.world.get_entity(node).is_ok(),
        "so does everything built around it"
    );
    assert_eq!(
        *game.world.get::<Position>(home).unwrap(),
        home_before,
        "base space is untouched by a breach — the Home stays exactly where it was"
    );
    assert_eq!(
        *game.world.get::<Position>(node).unwrap(),
        node_before,
        "and so does every other structure, at its own absolute coordinate"
    );
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    assert_ne!(
        (home_before.x, home_before.y),
        (spawn.x, spawn.y),
        "the fixture must not coincide with the new spawn, or an unmoved Home \
         and a relocated one would be indistinguishable"
    );
}

/// `BaseGrid` is base-space's own ground and is not zone-local — the
/// surrounding wipe-by-name block in `enter_next_zone` (`StackMemory`,
/// `BuybackLedger`, `PopulatedChunks`) is the pattern every zone-local
/// resource follows, and `BaseGrid` failing to appear in that list is the
/// whole point of this task, not an oversight. Compared by value rather
/// than by cell count, so a breach that rewrote every cell to the same
/// count but different coordinates would still be caught.
#[test]
fn breaching_does_not_touch_the_base_grid() {
    let mut game = Game::new(949, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    build_a_base(&mut game);
    let before = game.world.resource::<crate::base_grid::BaseGrid>().clone();
    assert_ne!(
        before,
        crate::base_grid::BaseGrid::default(),
        "the fixture must have actually laid a pocket, or this proves nothing"
    );

    game.enter_next_zone();

    assert_eq!(
        game.world.resource::<crate::base_grid::BaseGrid>(),
        &before,
        "a breach must not touch base space at all"
    );
}

/// The anchor is on the zone surface, not in base space — unlike a
/// `Structure`, it really does have to move to the new zone's spawn point
/// on every breach (Task 4 wired the write; this confirms it fires). The
/// anchor is hand-displaced first because `find_walkable_start` always
/// resolves `(0, 0)`, so an untouched fixture starting at `(0, 0)` could
/// not tell "moved to the new spawn" apart from "never moved at all".
#[test]
fn breaching_moves_the_anchor_to_the_new_spawn_point() {
    let mut game = Game::new(950, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let anchor = game.world.resource::<AnchorEntity>().0;
    {
        let mut pos = game.world.get_mut::<Position>(anchor).unwrap();
        pos.x = 40;
        pos.y = -15;
    }

    game.enter_next_zone();

    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let after = *game.world.get::<Position>(anchor).unwrap();
    assert_eq!(
        (after.x, after.y),
        (spawn.x, spawn.y),
        "the anchor must land on the new zone's spawn point, not stay where it was displaced to"
    );
}
