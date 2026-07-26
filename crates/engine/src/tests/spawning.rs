//! Populating a zone: wild programs, the spawn cap, nests, and guardians.

use super::support::*;
use crate::*;

#[test]
fn entering_a_zone_portal_despawns_nests_left_behind_in_the_old_zone() {
    let mut game = Game::new(602, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let nest = game
        .world
        .spawn((
            Nest {
                species: "scrapper".to_string(),
                pending_respawns: Vec::new(),
            },
            Position { x: 10, y: 10 },
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

    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();
    game.world.spawn((
        Structure {
            kind: "portal".to_string(),
        },
        Position {
            x: ppos.x + 1,
            y: ppos.y,
        },
    ));

    game.move_player(1, 0);

    // Note: `enter_next_zone` spawns fresh initial creatures for the new
    // zone, which can legitimately include brand-new nests — so this
    // must check the specific entity spawned above, not just count all
    // `Nest` entities in the world.
    assert!(
        game.world.get_entity(nest).is_err(),
        "a Nest left behind in the old zone must be despawned on zone transition, not just its guardians"
    );
}

#[test]
fn spawn_nest_creates_a_tethered_guardian_cluster() {
    let mut game = Game::new(601, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    // `Game::new` runs its own initial habitat-spawn rolls, which can
    // themselves occasionally create a Nest (now that species like
    // scrapper have can_nest: true) before this test's own explicit
    // spawn_nest call ever runs. Capture whatever nests already exist
    // first, so the assertions below only ever look at the nest this
    // test itself created, not a world-wide count that a background
    // spawn could inflate.
    let pre_existing_nests: std::collections::HashSet<Entity> = {
        let mut query = game.world.query_filtered::<Entity, With<Nest>>();
        query.iter(&game.world).collect()
    };
    game.spawn_nest("scrapper", 30, 30);

    let nests: Vec<(Entity, Position)> = {
        let mut query = game.world.query::<(Entity, &Nest, &Position)>();
        query
            .iter(&game.world)
            .filter(|(e, _, _)| !pre_existing_nests.contains(e))
            .map(|(e, _, p)| (e, *p))
            .collect()
    };
    assert_eq!(
        nests.len(),
        1,
        "spawn_nest should create exactly one new Nest entity"
    );
    let (nest, nest_pos) = nests[0];
    assert_eq!(nest_pos, Position { x: 30, y: 30 });
    assert_eq!(
        game.world.get::<Durability>(nest).unwrap().hp,
        NEST_DURABILITY
    );

    let guardians: Vec<Position> = {
        let mut query = game.world.query::<(&NestGuardian, &Position)>();
        query
            .iter(&game.world)
            .filter(|(g, _)| g.nest == nest)
            .map(|(_, p)| *p)
            .collect()
    };
    assert!(
        guardians.len() >= NEST_GUARDIAN_MIN as usize
            && guardians.len() <= NEST_GUARDIAN_MAX as usize,
        "expected {}..={} guardians, got {}",
        NEST_GUARDIAN_MIN,
        NEST_GUARDIAN_MAX,
        guardians.len()
    );
    for pos in guardians {
        let dist = (pos.x - 30).abs().max((pos.y - 30).abs());
        assert!(
            dist <= NEST_TETHER_RADIUS,
            "guardian spawned {dist} tiles from its nest, past the {NEST_TETHER_RADIUS}-tile tether"
        );
    }
}

#[test]
fn guardian_never_wanders_beyond_the_nest_tether_radius() {
    let mut game = Game::new(602, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.spawn_nest("scrapper", 40, 40);

    let (nest, nest_pos) = {
        let mut query = game.world.query::<(Entity, &Nest, &Position)>();
        let (e, _, p) = query.iter(&game.world).next().expect("nest should exist");
        (e, *p)
    };
    let guardians: Vec<Entity> = {
        let mut query = game.world.query::<(Entity, &NestGuardian)>();
        query
            .iter(&game.world)
            .filter(|(_, g)| g.nest == nest)
            .map(|(e, _)| e)
            .collect()
    };
    assert!(!guardians.is_empty());

    for _ in 0..200 {
        game.tick();
        for &guardian in &guardians {
            let pos = *game.world.get::<Position>(guardian).unwrap();
            let dist = (pos.x - nest_pos.x).abs().max((pos.y - nest_pos.y).abs());
            assert!(
                dist <= NEST_TETHER_RADIUS,
                "guardian wandered {dist} tiles from its nest, past the {NEST_TETHER_RADIUS}-tile tether"
            );
        }
    }
}

/// End to end, through the real round loop rather than a projection: a
/// fresh run must be able to win the first fight it walks into. Everything
/// else about the opening ring is machinery in service of this.
///
/// Whole seeds rather than a built fixture, because the failure this
/// guards against was assembled out of parts that each looked reasonable —
/// zone 1 caps a group at one member, four groups may engage, the player
/// starts with no companions — and only bit when a generated world put
/// them together.
#[test]
fn a_fresh_run_can_win_the_first_fight_it_walks_into() {
    let mut fights = 0;
    for seed in 1..=8u32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let spawn = *game.world.resource::<ZoneSpawnPoint>();
        let nearest = {
            let mut query = game
                .world
                .query_filtered::<(Entity, &Position), With<Hostile>>();
            query
                .iter(&game.world)
                .map(|(e, p)| (e, (p.x - spawn.x).abs().max((p.y - spawn.y).abs())))
                .filter(|(_, dist)| *dist < GROUP_SIZE_STEP_TILES)
                .min_by_key(|&(_, dist)| dist)
                .map(|(e, _)| e)
        };
        let Some(anchor) = nearest else {
            continue;
        };
        let species = game.world.get::<Creature>(anchor).unwrap().species.clone();
        let pack = game.gather_pack(anchor);
        assert_eq!(
            pack.len(),
            1,
            "seed {seed}: bumping a program in the opening ring pulled in {} of them",
            pack.len()
        );

        game.start_battle(pack);
        // Bounded so a stalemate fails the assertion below instead of
        // hanging the suite. Well above the 8 rounds the slowest shipped
        // matchup takes.
        let mut rounds = 0;
        while game.has_active_battle() && rounds < 60 {
            player_attacks(&mut game);
            rounds += 1;
        }

        let hp = game.world.get::<Stats>(game.player_entity()).unwrap().hp;
        assert!(
            !game.has_active_battle() && hp > 0,
            "seed {seed}: a bare level-1 player lost their opening fight against a \
             {species} after {rounds} rounds, at {hp} HP"
        );
        assert!(
            rounds > 2,
            "seed {seed}: the opening {species} died in {rounds} rounds — the ring is \
             supposed to be winnable, not free"
        );
        fights += 1;
    }
    assert!(
        fights >= 6,
        "only {fights} of 8 seeds put anything in the opening ring to fight, so this \
         test is mostly asserting nothing"
    );
}

/// A fresh run opens with an empty `Party`, so the first fights are solo —
/// and eleven of the fifteen shipped ordinary species beat a bare level-1
/// player one-on-one. Inside the opening ring a spawn roll may only place
/// the ones that don't.
///
/// Asserted over the spawn *roll*, not over the standing population: wild
/// programs wander (`systems::wander_ai_system`), and a nest just outside
/// the ring tethers guardians up to `NEST_TETHER_RADIUS` inward, so
/// something tougher can walk in. The ring decides what is born there, not
/// what may ever stand there.
#[test]
fn the_zone_one_opening_ring_only_rolls_species_a_fresh_player_can_beat() {
    let mut game = Game::new(444, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    // Every tile of a full ring-width square, so this covers whatever
    // biomes this seed's terrain happens to lay down rather than the one
    // the spawn tile sits in.
    let ring = GROUP_SIZE_STEP_TILES - 1;
    for dx in -ring..=ring {
        for dy in -ring..=ring {
            game.try_spawn_habitat_creature(spawn.x + dx, spawn.y + dy);
        }
    }

    let placed: Vec<(String, Position, i32)> = {
        let mut query = game
            .world
            .query_filtered::<(&Creature, &Position), With<Hostile>>();
        query
            .iter(&game.world)
            .map(|(c, p)| {
                (
                    c.species.clone(),
                    *p,
                    game.distance_from_danger_origin(p.x, p.y),
                )
            })
            .collect()
    };
    assert!(
        placed
            .iter()
            .any(|(_, _, dist)| *dist < GROUP_SIZE_STEP_TILES),
        "the sweep has to actually populate the ring, or this asserts nothing"
    );
    let stat_total = |s: &SpeciesDef| s.base_hp + s.base_atk + s.base_def;
    for (species, pos, dist) in placed {
        if dist >= GROUP_SIZE_STEP_TILES {
            continue;
        }
        let biome = game
            .world
            .resource_mut::<WorldMap>()
            .tile(pos.x, pos.y)
            .biome;
        let db = game.world.resource::<SpeciesDb>();
        let def = db
            .get(&species)
            .expect("a spawned creature's species is in the db");
        // No shipped StaticField species is a fair solo fight, so the rule
        // the ring actually enforces is "beatable, or else the gentlest
        // this biome has" — asserted as an outcome rather than by
        // re-deriving the pool the spawn path built.
        let gentlest = db
            .habitat_matches(biome)
            .into_iter()
            .map(stat_total)
            .min()
            .expect("the creature spawned here, so this biome has species");
        assert!(
            crate::balance::beatable_by_a_fresh_player(def) || stat_total(def) == gentlest,
            "{species} spawned {dist} tiles into the opening ring: a bare level-1 \
             player is projected to lose to it, and it isn't the gentlest thing \
             its biome offers either"
        );
    }
}

/// The fallback half of the ring rule, on the biome that forces it: no
/// shipped StaticField species is a fair solo fight for a level-1 player,
/// so a StaticField tile in the ring fields the gentlest of them rather
/// than rolling freely across sentinels and ciphers. The biome is forced
/// with a tile override rather than hunted for in generated terrain, so
/// the case is covered whatever the seed lays down.
#[test]
fn a_ring_biome_with_nothing_gentle_fields_only_its_gentlest_species() {
    let mut game = Game::new(445, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let (x, y) = (spawn.x + 1, spawn.y + 1);
    game.world.resource_mut::<WorldMap>().set_override(
        x,
        y,
        Tile {
            biome: Biome::StaticField,
            walkable: true,
        },
    );

    let expected = {
        let db = game.world.resource::<SpeciesDb>();
        let pool = db.habitat_matches(Biome::StaticField);
        assert!(
            pool.iter()
                .all(|s| !crate::balance::beatable_by_a_fresh_player(s)),
            "this test's premise is that StaticField has nothing a fresh player \
             beats — if that changed, it is now testing the wrong branch"
        );
        pool.into_iter()
            .min_by_key(|s| s.base_hp + s.base_atk + s.base_def)
            .expect("StaticField ships species")
            .id
            .clone()
    };

    for _ in 0..40 {
        game.try_spawn_habitat_creature(x, y);
    }

    let spawned: Vec<String> = {
        let mut query = game
            .world
            .query_filtered::<(&Creature, &Position), With<Hostile>>();
        query
            .iter(&game.world)
            .filter(|(_, p)| p.x == x && p.y == y)
            .map(|(c, _)| c.species.clone())
            .collect()
    };
    assert!(!spawned.is_empty(), "40 rolls should place something");
    assert!(
        spawned.iter().all(|id| *id == expected),
        "a ring tile whose biome offers nothing gentle must field only its \
         gentlest species ({expected}), got {spawned:?}"
    );
}

/// The counterpart to the ring test above: one step out, the species it
/// turns away are all still there to be met. A buffer that quietly became
/// a zone-wide difficulty cut would be a worse bug than the one it fixes.
#[test]
fn past_the_opening_ring_the_full_habitat_roster_spawns_again() {
    let mut game = Game::new(444, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let out = spawn.x + GROUP_SIZE_STEP_TILES;
    for dy in -20..=20 {
        for dx in 0..40 {
            game.try_spawn_habitat_creature(out + dx, spawn.y + dy);
        }
    }

    let spawned: Vec<String> = {
        let mut query = game.world.query_filtered::<&Creature, With<Hostile>>();
        query.iter(&game.world).map(|c| c.species.clone()).collect()
    };
    let db = game.world.resource::<SpeciesDb>();
    let tough: Vec<&String> = spawned
        .iter()
        .filter(|id| {
            db.get(id)
                .is_some_and(|s| !crate::balance::beatable_by_a_fresh_player(s))
        })
        .collect();
    assert!(
        !tough.is_empty(),
        "past the ring, the species a fresh player loses to must spawn normally"
    );
}

#[test]
fn spawn_wild_creature_rolls_individual_stat_variance_within_a_species() {
    let mut game = Game::new(420, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species_id = game.species_defs().into_iter().next().unwrap().id;
    for _ in 0..15 {
        game.spawn_wild_creature(&species_id, 5, 5);
    }

    let mut query = game
        .world
        .query_filtered::<(&Position, &Stats), With<Hostile>>();
    let max_hps: Vec<i32> = query
        .iter(&game.world)
        .filter(|(p, _)| p.x == 5 && p.y == 5)
        .map(|(_, s)| s.max_hp)
        .collect();
    assert_eq!(max_hps.len(), 15);
    assert!(
        max_hps.iter().any(|&hp| hp != max_hps[0]),
        "spawning the same species repeatedly should roll different individual stats, got {max_hps:?}"
    );
}

#[test]
fn wild_spawn_cap_is_not_exhausted_by_tamed_creatures() {
    let mut game = Game::new(422, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let species_id = game.species_defs().into_iter().next().unwrap().id;
    for _ in 0..24 {
        game.world.spawn((
            Creature {
                species: species_id.clone(),
            },
            Position { x: 0, y: 0 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 1,
                def: 1,
            },
            Tamed { owner: player },
        ));
    }

    // `Game::new` already seeds 14 initial (hostile) wild creatures, so
    // the true wild population here is 14 — comfortably under any
    // reasonable cap — even though total `Creature` entities (wild +
    // tamed) is 38.
    let mut creature_query = game.world.query_filtered::<(), With<Creature>>();
    let before = creature_query.iter(&game.world).count();

    for _ in 0..500 {
        game.maybe_spawn_wild_creature();
    }

    let after = creature_query.iter(&game.world).count();
    assert!(
        after > before,
        "wild creatures should still be able to spawn even when the map already has \
         24 tamed (non-hostile) programs on it, but the population stayed at {before} \
         after 500 attempts"
    );
}

#[test]
fn a_full_wild_population_far_away_is_culled_so_spawns_near_the_player_still_happen() {
    let mut game = Game::new(423, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species_id = game.species_defs().into_iter().next().unwrap().id;
    let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();

    // Fill the cap with a wild population the player wandered away from,
    // far outside the (-12..=12) radius `maybe_spawn_wild_creature` ever
    // spawns into around the player's *current* position.
    let mut hostile_query = game.world.query_filtered::<(), With<Hostile>>();
    let already = hostile_query.iter(&game.world).count();
    let distant: Vec<Entity> = (0..WILD_CREATURE_CAP - already)
        .map(|_| {
            game.world
                .spawn((
                    Creature {
                        species: species_id.clone(),
                    },
                    Position {
                        x: player_pos.x + 500,
                        y: player_pos.y + 500,
                    },
                    Stats {
                        hp: 10,
                        max_hp: 10,
                        atk: 1,
                        def: 1,
                    },
                    Hostile,
                ))
                .id()
        })
        .collect();

    let mut nearby_query = game.world.query_filtered::<&Position, With<Hostile>>();
    let nearby_before = nearby_query
        .iter(&game.world)
        .filter(|p| (p.x - player_pos.x).abs() <= 20 && (p.y - player_pos.y).abs() <= 20)
        .count();

    for _ in 0..500 {
        game.maybe_spawn_wild_creature();
    }

    let nearby_after = nearby_query
        .iter(&game.world)
        .filter(|p| (p.x - player_pos.x).abs() <= 20 && (p.y - player_pos.y).abs() <= 20)
        .count();

    assert!(
        nearby_after > nearby_before,
        "a wild population the player left behind elsewhere on the map shouldn't be able \
         to block new spawns near the player's current position, but nothing spawned \
         nearby in 500 attempts (nearby count stayed at {nearby_before})"
    );

    let surviving_distant = distant
        .iter()
        .filter(|&&e| game.world.get_entity(e).is_ok())
        .count();
    assert!(
        surviving_distant < distant.len(),
        "the distant population should have been culled to make room, but all \
         {} of them survived",
        distant.len()
    );
}

#[test]
fn nest_guardians_are_eligible_to_be_culled_for_spawn_room() {
    let mut game = Game::new(424, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species_id = game.species_defs().into_iter().next().unwrap().id;
    let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();

    let nest = game
        .world
        .spawn((
            Nest {
                species: species_id.clone(),
                pending_respawns: Vec::new(),
            },
            Position {
                x: player_pos.x + 500,
                y: player_pos.y + 500,
            },
            Durability {
                hp: 100,
                max_hp: 100,
            },
        ))
        .id();

    // Fill the cap entirely with guardians of that far-away nest — the
    // farthest hostile from the player is always going to be one of them.
    let mut hostile_query = game.world.query_filtered::<(), With<Hostile>>();
    let already = hostile_query.iter(&game.world).count();
    for _ in 0..WILD_CREATURE_CAP - already {
        game.world.spawn((
            Creature {
                species: species_id.clone(),
            },
            Position {
                x: player_pos.x + 500,
                y: player_pos.y + 500,
            },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 1,
                def: 1,
            },
            Hostile,
            WanderAi::default(),
            NestGuardian { nest },
        ));
    }

    let nearby_before = {
        let mut query = game.world.query_filtered::<&Position, With<Hostile>>();
        query
            .iter(&game.world)
            .filter(|p| (p.x - player_pos.x).abs() <= 20 && (p.y - player_pos.y).abs() <= 20)
            .count()
    };

    for _ in 0..500 {
        game.maybe_spawn_wild_creature();
    }

    let mut hostile_query = game.world.query_filtered::<&Position, With<Hostile>>();
    let nearby_after = hostile_query
        .iter(&game.world)
        .filter(|p| (p.x - player_pos.x).abs() <= 20 && (p.y - player_pos.y).abs() <= 20)
        .count();
    assert!(
        nearby_after > nearby_before,
        "guardians of a nest the player left behind shouldn't block spawns near the \
         player, but nothing spawned nearby in 500 attempts"
    );

    let mut guardian_query = game.world.query_filtered::<(), With<NestGuardian>>();
    let guardians_left = guardian_query.iter(&game.world).count();
    assert!(
        guardians_left < WILD_CREATURE_CAP - already,
        "the farthest hostile should be culled even when it's a nest guardian, but \
         all {guardians_left} guardians survived"
    );
}

/// One roll can place a whole group, so the cull has to free room for the
/// group rather than for one creature — otherwise the population ratchets
/// up past the cap and never comes back down.
#[test]
fn a_spawn_roll_culls_enough_room_for_the_whole_group_it_places() {
    let mut game = Game::new(425, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 4;
    let species_id = game.species_defs().into_iter().next().unwrap().id;

    // Put the player deep in the field, where a roll places a real group
    // rather than a single creature.
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let player = game.player_entity();
    let far = Position {
        x: spawn.x + GROUP_SIZE_STEP_TILES * 7,
        y: spawn.y,
    };
    *game.world.get_mut::<Position>(player).unwrap() = far;
    assert!(
        game.max_group_size(far.x, far.y) > 1,
        "the fixture is pointless unless a roll here places more than one"
    );

    // Fill the cap with a population far from the player.
    let mut hostile_query = game.world.query_filtered::<(), With<Hostile>>();
    let already = hostile_query.iter(&game.world).count();
    for _ in 0..WILD_CREATURE_CAP - already {
        game.world.spawn((
            Creature {
                species: species_id.clone(),
            },
            Position {
                x: far.x + 500,
                y: far.y + 500,
            },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 1,
                def: 1,
            },
            Hostile,
        ));
    }

    for _ in 0..60 {
        game.maybe_spawn_wild_creature();
        // Bind the query before iterating: `query_filtered` takes `&mut
        // World`, so it can't be chained straight into an `iter(&world)`.
        let live = hostile_query.iter(&game.world).count();
        // `NEST_GUARDIAN_MAX` slack: a nest roll spawns its guardians
        // through `spawn_nest`, which isn't sized by `max_group_size`, so
        // it's the one path that can overspend the budget — by a bounded
        // amount that the next roll's cull reclaims.
        assert!(
            live <= WILD_CREATURE_CAP + NEST_GUARDIAN_MAX as usize,
            "the hostile population ran past the cap ({live} of {WILD_CREATURE_CAP}) — \
             the cull is freeing room for one creature, not for the group being placed"
        );
    }
}

#[test]
fn individual_growth_roll_scales_stat_gains_independently_of_species_growth_multiplier() {
    let mut game = Game::new(421, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let species_id = game.species_defs().into_iter().next().unwrap().id;

    let low_roll = game
        .world
        .spawn((
            Creature {
                species: species_id.clone(),
            },
            Position { x: 3, y: 3 },
            Stats {
                hp: 100,
                max_hp: 100,
                atk: 10,
                def: 10,
            },
            Potential {
                hp_roll: 1.0,
                atk_roll: 1.0,
                def_roll: 1.0,
                growth_roll: MIN_INDIVIDUAL_ROLL,
            },
            Tamed { owner: player },
            Experience {
                level: 1,
                xp: 0,
                xp_to_next: 1,
            },
        ))
        .id();
    let high_roll = game
        .world
        .spawn((
            Creature {
                species: species_id,
            },
            Position { x: 3, y: 3 },
            Stats {
                hp: 100,
                max_hp: 100,
                atk: 10,
                def: 10,
            },
            Potential {
                hp_roll: 1.0,
                atk_roll: 1.0,
                def_roll: 1.0,
                growth_roll: MAX_INDIVIDUAL_ROLL,
            },
            Tamed { owner: player },
            Experience {
                level: 1,
                xp: 0,
                xp_to_next: 1,
            },
        ))
        .id();
    game.add_companion(low_roll).unwrap();
    game.add_companion(high_roll).unwrap();

    // xp_to_next is rigged to 1 above, so any non-zero party XP levels
    // both companions up exactly once, at the same species (and so the
    // same growth_multiplier) — only their individual growth_roll differs.
    game.award_player_xp(player, 2);

    let low_hp = game.world.get::<Stats>(low_roll).unwrap().max_hp;
    let high_hp = game.world.get::<Stats>(high_roll).unwrap().max_hp;
    assert!(
        high_hp > low_hp,
        "a higher individual growth_roll should out-grow a lower one at the same species: {high_hp} vs {low_hp}"
    );
}

#[test]
fn bumping_a_nest_damages_it_and_destroying_it_frees_its_guardians() {
    let mut game = Game::new(603, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Position>(player).unwrap().x = 49;
    game.world.get_mut::<Position>(player).unwrap().y = 50;

    let nest = game
        .world
        .spawn((
            Nest {
                species: "scrapper".to_string(),
                pending_respawns: Vec::new(),
            },
            Position { x: 50, y: 50 },
            Glyph {
                ch: 'N',
                color: GlyphColor::Red,
            },
            Durability { hp: 5, max_hp: 5 },
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
            Position { x: 52, y: 52 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 1,
                def: 1,
            },
        ))
        .id();

    // Player's base ATK (6) vs. 0 defense, move_power 5 → well over 5
    // damage, so one bump is enough to destroy a 5-HP nest.
    game.move_player(1, 0);

    assert!(
        game.world.get::<Nest>(nest).is_none(),
        "nest should be destroyed by one bump"
    );
    assert!(
        game.world.get::<NestGuardian>(guardian).is_none(),
        "guardian should lose its NestGuardian tether once the nest is destroyed"
    );
}

#[test]
fn bumping_a_nest_with_high_hp_damages_it_without_destroying_it() {
    let mut game = Game::new(604, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Position>(player).unwrap().x = 49;
    game.world.get_mut::<Position>(player).unwrap().y = 50;

    let nest = game
        .world
        .spawn((
            Nest {
                species: "scrapper".to_string(),
                pending_respawns: Vec::new(),
            },
            Position { x: 50, y: 50 },
            Glyph {
                ch: 'N',
                color: GlyphColor::Red,
            },
            Durability { hp: 50, max_hp: 50 },
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
            Position { x: 52, y: 52 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 1,
                def: 1,
            },
        ))
        .id();

    // Player's base ATK (6) vs. 0 defense, move_power 5 → 11 damage,
    // well short of the nest's 50 HP, so one bump only dents it.
    game.move_player(1, 0);

    assert!(
        game.world.get::<Nest>(nest).is_some(),
        "nest should survive a single bump when it has 50 HP"
    );
    let hp = game.world.get::<Durability>(nest).unwrap().hp;
    assert!(
        hp < 50,
        "nest HP should have decreased from the bump, got {hp}"
    );
    assert!(hp > 0, "nest HP should still be positive, got {hp}");
    assert!(
        game.world.get::<NestGuardian>(guardian).is_some(),
        "guardian should keep its NestGuardian tether while the nest survives"
    );
}

#[test]
fn killing_a_guardian_respawns_a_replacement_after_exactly_the_respawn_delay() {
    let mut game = Game::new(604, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let nest = game
        .world
        .spawn((
            Nest {
                species: "scrapper".to_string(),
                pending_respawns: Vec::new(),
            },
            Position { x: 60, y: 60 },
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
            NestGuardian { nest },
            Position { x: 61, y: 61 },
            Stats {
                hp: 1,
                max_hp: 10,
                atk: 0,
                def: 0,
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![guardian]);

    player_attacks(&mut game);

    // the round loop's own kill-resolution path (finish_group_member
    // returning true, the pack now empty) already calls self.tick() once
    // internally before returning — that tick already ran
    // nest_respawn_tick and decremented the entry we just pushed. So the
    // value observed here is NEST_RESPAWN_TICKS - 1, not the full delay.
    assert_eq!(
        game.world.get::<Nest>(nest).unwrap().pending_respawns,
        vec![NEST_RESPAWN_TICKS - 1],
        "killing a guardian should queue one respawn"
    );

    let guardian_count = |game: &mut Game| -> usize {
        let mut query = game.world.query::<&NestGuardian>();
        query.iter(&game.world).filter(|g| g.nest == nest).count()
    };

    for _ in 0..(NEST_RESPAWN_TICKS - 2) {
        game.tick();
    }
    assert_eq!(
        guardian_count(&mut game),
        0,
        "no replacement should spawn before its delay elapses"
    );

    game.tick();
    assert_eq!(
        guardian_count(&mut game),
        1,
        "a replacement should spawn exactly when its delay elapses"
    );
}

#[test]
fn taming_a_guardian_also_queues_a_respawn() {
    let mut game = Game::new(605, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let nest = game
        .world
        .spawn((
            Nest {
                species: "scrapper".to_string(),
                pending_respawns: Vec::new(),
            },
            Position { x: 70, y: 70 },
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
            Position { x: 71, y: 71 },
            Stats {
                hp: 1,
                max_hp: 10,
                atk: 1,
                def: 1,
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

    assert!(game.world.get::<Tamed>(guardian).is_some());
    assert!(
        game.world.get::<NestGuardian>(guardian).is_none(),
        "a tamed creature should lose its nest tether"
    );
    // Same off-by-one as the kill test above: battle_decompile's
    // success path also calls self.tick() once internally before
    // returning, which already decremented the entry we just pushed.
    assert_eq!(
        game.world.get::<Nest>(nest).unwrap().pending_respawns,
        vec![NEST_RESPAWN_TICKS - 1],
        "taming a guardian should also queue one respawn"
    );
}

#[test]
fn killing_a_guardian_whose_nest_is_already_gone_queues_nothing() {
    let mut game = Game::new(606, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // A dangling nest Entity — never actually spawned, standing in
    // for "the nest was destroyed before this guardian died."
    let gone_nest = game.world.spawn_empty().id();
    let guardian = game
        .world
        .spawn((
            Creature {
                species: "scrapper".to_string(),
            },
            Hostile,
            NestGuardian { nest: gone_nest },
            Position { x: 80, y: 80 },
            Stats {
                hp: 1,
                max_hp: 10,
                atk: 0,
                def: 0,
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![guardian]);

    // Should not panic even though `gone_nest` has no Nest component.
    player_attacks(&mut game);

    for _ in 0..(NEST_RESPAWN_TICKS + 5) {
        game.tick();
    }
    // Nothing to assert beyond "didn't panic" — there's no Nest left
    // to have queued anything on, and no new guardian entity for a
    // nonexistent nest.
}

#[test]
fn nest_respawn_tick_spawns_one_guardian_per_ready_entry_not_one_per_nest() {
    let mut game = Game::new(607, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let nest = game
        .world
        .spawn((
            Nest {
                species: "scrapper".to_string(),
                // Two entries reach 0 on the same tick, and a third
                // untouched entry that should survive, decremented but
                // not fired — this proves nest_respawn_tick spawns once
                // per ready entry, not once per nest.
                pending_respawns: vec![1, 1, 5],
            },
            Position { x: 90, y: 90 },
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

    let guardian_count = |game: &mut Game| -> usize {
        let mut query = game.world.query::<&NestGuardian>();
        query.iter(&game.world).filter(|g| g.nest == nest).count()
    };
    assert_eq!(guardian_count(&mut game), 0, "no guardians before the tick");

    game.tick();

    assert_eq!(
        guardian_count(&mut game),
        2,
        "both entries reaching 0 on the same tick should each spawn a guardian"
    );
    assert_eq!(
        game.world.get::<Nest>(nest).unwrap().pending_respawns,
        vec![4],
        "the two fired entries should be removed and the untouched entry decremented once"
    );
}
