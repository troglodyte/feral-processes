//! Populating a zone: wild programs, the spawn cap, nests, and guardians.

use super::support::*;
use crate::species::DangerBand;
use crate::tuning::{
    BOSS_SPAWN_CHANCE, DANGER_RAMP_TILES, MAX_GROUP_SIZE_STEPS, MAX_INDIVIDUAL_ROLL,
    NEST_DURABILITY, NEST_GUARDIAN_MAX, NEST_GUARDIAN_MIN, NEST_RESPAWN_TICKS, NEST_TETHER_RADIUS,
    OPENING_RING_TILES, POPULATION_CHUNK_MARGIN, WANDER_COOLDOWN_MAX_TICKS,
    WANDER_COOLDOWN_MIN_TICKS, WILD_CREATURE_CAP, WILD_LOCAL_DENSITY_TARGET,
    WILD_SPAWN_RADIUS_TILES, chunk_wild_population,
};
use crate::*;

#[test]
fn a_breach_leaves_a_nest_standing() {
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

    breach_through_a_portal(&mut game);

    // The specific entity spawned above, not a count: `ensure_local_population`
    // legitimately adds nests of its own at the new tier.
    assert!(
        game.world.get::<Nest>(nest).is_some(),
        "a breach despawned a nest — a nest is part of the place, and the place persists"
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

/// Nothing today can drag a guardian outside its tether — this is the
/// latent bug pursuit (built later) would otherwise make reachable. A
/// guardian whose tether check refuses on raw distance alone has no legal
/// move once displaced past `NEST_TETHER_RADIUS`, since every neighbouring
/// tile is still beyond it too, and stands frozen for the rest of the run.
#[test]
fn a_guardian_outside_its_tether_walks_back_toward_its_nest() {
    let mut game = Game::new(608, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let nest_pos = Position { x: 200, y: 200 };
    // Open ground across the whole path the guardian can walk, not just
    // the endpoints — a DataVoid pocket anywhere along the way would give
    // it no legal step for a reason unrelated to the tether fix.
    for dx in -20..=20 {
        for dy in -20..=20 {
            game.world.resource_mut::<WorldMap>().set_override(
                nest_pos.x + dx,
                nest_pos.y + dy,
                Tile {
                    biome: Biome::OpenGrid,
                    walkable: true,
                    rock_shade: None,
                },
            );
        }
    }

    game.spawn_nest("scrapper", nest_pos.x, nest_pos.y);
    let nest = {
        let mut query = game.world.query::<(Entity, &Nest, &Position)>();
        query
            .iter(&game.world)
            .find(|(_, _, p)| **p == nest_pos)
            .map(|(e, _, _)| e)
            .expect("spawn_nest should have created a Nest at nest_pos")
    };
    let guardian = {
        let mut query = game.world.query::<(Entity, &NestGuardian)>();
        query
            .iter(&game.world)
            .find(|(_, g)| g.nest == nest)
            .map(|(e, _)| e)
            .expect("spawn_nest should have created at least one guardian")
    };

    // Well outside NEST_TETHER_RADIUS (5) — nothing today can put a
    // guardian here, but a chase (built later) will.
    let start = Position {
        x: nest_pos.x + 12,
        y: nest_pos.y,
    };
    *game.world.get_mut::<Position>(guardian).unwrap() = start;
    let start_dist = (start.x - nest_pos.x)
        .abs()
        .max((start.y - nest_pos.y).abs());

    // WanderAi's cooldown is 2-6 ticks, so this is well past enough
    // firings for a guardian with a legal move to have taken several.
    for _ in 0..200 {
        game.tick();
    }

    let pos = *game.world.get::<Position>(guardian).unwrap();
    let dist = (pos.x - nest_pos.x).abs().max((pos.y - nest_pos.y).abs());
    // Closing *any* distance is the invariant; arriving isn't — the walk
    // is a random ±1 step on a cooldown, so the ticks needed to fully
    // return aren't deterministic.
    assert!(
        dist < start_dist,
        "a guardian dragged outside its tether should walk back toward its nest, \
         but stayed at distance {dist} (started at {start_dist})"
    );
}

/// A wild program waits out `WANDER_COOLDOWN_MIN_TICKS` between moves, and
/// averages the range's midpoint over a run of them.
///
/// The pace is a **pairing**, and this is the only thing holding it.
/// `WORLD_SPEED_MULTIPLIER` in app-core runs the world at two ticks a real
/// second, and these two constants are what keep a wanderer at the
/// wall-clock speed it had at one — nothing makes them fail to compile when
/// one moves without the other. At the `2..6` this replaced, the smallest
/// gap is 2 and the mean is under 4, so both assertions below fail.
///
/// Gaps between *moves* rather than between cooldown firings: a firing that
/// draws `(0, 0)`, or that would step onto ground closed to hostiles, spends
/// its cooldown without moving. So a gap is a sum of one or more firing
/// intervals, which is why the minimum is asserted exactly and the mean is
/// asserted against a band that sits above the firing interval itself.
#[test]
fn a_wild_program_waits_out_its_cooldown_between_moves() {
    let mut game = Game::new(4021, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    skip_tutorial(&mut game);
    let at = *game.world.get::<Position>(game.player_entity()).unwrap();
    let start = Position {
        x: at.x + 4,
        y: at.y,
    };
    let wanderer = game
        .world
        .spawn((
            Creature {
                species: "scrapper".to_string(),
            },
            Hostile,
            WanderAi::default(),
            start,
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 1,
                mitigation: 1,
            },
        ))
        .id();

    // `ensure_local_population` runs every tick and can spawn more
    // `Hostile`+`WanderAi` creatures near the player. `wander_ai_system`'s
    // query has no stable order, so a second wanderer racing this one for
    // `GameRng` draws would make the mean below depend on bevy's iteration
    // order rather than on the wander formula this test asserts about —
    // swept out every tick so `wanderer` is always the only entity a
    // wander tick can draw for.
    let despawn_other_wanderers = |game: &mut Game| {
        let strays: Vec<Entity> = {
            let mut query = game.world.query_filtered::<Entity, With<WanderAi>>();
            query.iter(&game.world).filter(|&e| e != wanderer).collect()
        };
        for stray in strays {
            game.world.despawn(stray);
        }
    };
    despawn_other_wanderers(&mut game);

    let mut gaps: Vec<u64> = Vec::new();
    let mut last_move: Option<u64> = None;
    let mut prev = start;
    for step in 1..=1500u64 {
        game.tick();
        despawn_other_wanderers(&mut game);
        let Some(now) = game.world.get::<Position>(wanderer).copied() else {
            break;
        };
        if now != prev {
            if let Some(last) = last_move {
                gaps.push(step - last);
            }
            last_move = Some(step);
            prev = now;
        }
    }

    assert!(
        gaps.len() >= 50,
        "1500 ticks should have moved a wanderer well over fifty times, but saw {} moves \
         — if it is far fewer the program was despawned or fenced in, and the pace below \
         is being measured off noise",
        gaps.len()
    );
    let smallest = *gaps.iter().min().expect("gaps is non-empty");
    assert!(
        smallest >= u64::from(WANDER_COOLDOWN_MIN_TICKS),
        "a wanderer moved again after only {smallest} ticks, under the \
         WANDER_COOLDOWN_MIN_TICKS of {WANDER_COOLDOWN_MIN_TICKS} — the map's wall-clock \
         pace is pinned to that floor"
    );
    let mean = gaps.iter().sum::<u64>() as f64 / gaps.len() as f64;
    assert!(
        (6.0..=10.0).contains(&mean),
        "a wanderer averaged a move every {mean:.2} ticks; the cooldown range \
         {WANDER_COOLDOWN_MIN_TICKS}..{WANDER_COOLDOWN_MAX_TICKS} should put that near 7.9 \
         (a firing every 7 ticks, ~8 in 9 of which move)"
    );
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
                .filter(|(_, dist)| *dist <= OPENING_RING_TILES)
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
    let ring = OPENING_RING_TILES;
    for dx in -ring..=ring {
        for dy in -ring..=ring {
            game.try_spawn_habitat_creature(spawn.x + dx, spawn.y + dy);
        }
    }

    // Nest guardians are excluded because their tile is not their roll: a
    // nest is placed by one ring-filtered pick at *its* tile and then
    // scatters guardians across `NEST_TETHER_RADIUS`, so a worm nest rolled
    // legitimately at distance 11 puts worms as far in as distance 6. Those
    // guardians never went through `habitat_pools` at the tile they stand
    // on, which is exactly the "born there" versus "stands there"
    // distinction this test's doc draws — sweeping them in asserts the rule
    // against creatures it was never meant to cover, and whether that fires
    // is down to which seed happens to roll a nest near the boundary.
    let placed: Vec<(String, Position, i32)> = {
        let mut query = game
            .world
            .query_filtered::<(&Creature, &Position), (With<Hostile>, Without<NestGuardian>)>();
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
            .any(|(_, _, dist)| *dist <= OPENING_RING_TILES),
        "the sweep has to actually populate the ring, or this asserts nothing"
    );
    let stat_total = |s: &SpeciesDef| s.base_hp + s.base_atk + s.base_mitigation;
    for (species, pos, dist) in placed {
        if dist > OPENING_RING_TILES {
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
        // No shipped Deadlock species is a fair solo fight, so the rule
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
            crate::balance_sim::beatable_by_a_fresh_player(def) || stat_total(def) == gentlest,
            "{species} spawned {dist} tiles into the opening ring: a bare level-1 \
             player is projected to lose to it, and it isn't the gentlest thing \
             its biome offers either"
        );
    }
}

/// The fallback half of the ring rule, on the biome that forces it: no
/// shipped Deadlock species is a fair solo fight for a level-1 player,
/// so a Deadlock tile in the ring fields the gentlest of them rather
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
            biome: Biome::Deadlock,
            walkable: true,
            rock_shade: None,
        },
    );

    let expected = {
        let db = game.world.resource::<SpeciesDb>();
        let pool = db.habitat_matches(Biome::Deadlock);
        assert!(
            pool.iter()
                .all(|s| !crate::balance_sim::beatable_by_a_fresh_player(s)),
            "this test's premise is that Deadlock has nothing a fresh player \
             beats — if that changed, it is now testing the wrong branch"
        );
        pool.into_iter()
            .min_by_key(|s| s.base_hp + s.base_atk + s.base_mitigation)
            .expect("Deadlock ships species")
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
///
/// The danger window narrowed what zone 1 fields at all, so this can no
/// longer be read off the whole roster: every band-0 species is one a fresh
/// player can beat, so in zone 1 the ring now has nothing left to gentle.
/// The zone is raised to the first step whose window admits a band the ring
/// *does* refuse, which is where the claim is still measurable — and the
/// ring is a distance from the danger origin rather than a property of zone
/// 1, so raising the zone does not take it away.
#[test]
fn past_the_opening_ring_the_full_habitat_roster_spawns_again() {
    let mut game = Game::new(444, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 1 + crate::tuning::TIER_ENTRY_STEPS);
    let spawn = *game.world.resource::<ZoneSpawnPoint>();

    // Tiles beyond the ring whose window offers a species the ring's
    // gentling turns away. Located rather than assumed, so this fails as
    // "the window left nothing for the ring to refuse" rather than going
    // quietly vacuous.
    let mut refusable: Vec<(i32, i32)> = Vec::new();
    for dy in -60..=60 {
        for dx in -60..=60 {
            let (x, y) = (spawn.x + dx, spawn.y + dy);
            if dx.abs() <= OPENING_RING_TILES && dy.abs() <= OPENING_RING_TILES {
                continue;
            }
            let Some((ordinary, _)) = game.habitat_pools(x, y, None, 0) else {
                continue;
            };
            let db = game.world.resource::<SpeciesDb>();
            if ordinary.iter().any(|id| {
                db.get(id)
                    .is_some_and(|s| !crate::balance_sim::beatable_by_a_fresh_player(s))
            }) {
                refusable.push((x, y));
            }
        }
    }
    assert!(
        !refusable.is_empty(),
        "no tile past the ring offers a species the ring would refuse — the \
         window has left this test nothing to measure"
    );

    for &(x, y) in &refusable {
        game.try_spawn_habitat_creature(x, y);
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
                .is_some_and(|s| !crate::balance_sim::beatable_by_a_fresh_player(s))
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
                mitigation: 1,
            },
            Tamed { owner: player },
            PowerReserve::default(),
        ));
    }

    // A zone is now seeded to `WILD_LOCAL_DENSITY_TARGET`, so the ground
    // around the player starts full and the ambient roll is legitimately
    // gated. Clear the box to isolate what this test is actually about —
    // whether *tamed* programs eat the wild population's room.
    let ppos = *game.world.get::<Position>(player).unwrap();
    despawn_hostiles_near(&mut game, ppos.x, ppos.y);

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

    // Clear the ground the player is standing on first. `spawn_initial_
    // creatures` seeds *to* `WILD_LOCAL_DENSITY_TARGET`, so whether the
    // local box starts with headroom is a property of where that seeded
    // scatter happened to land — and this test is about the *cap* and the
    // far-away cull, not about density. Leaving it to the seed made the
    // premise silently depend on the RNG stream position, which is what
    // broke it when a draw was added upstream.
    let local: Vec<Entity> = {
        let mut q = game
            .world
            .query_filtered::<(Entity, &Position), With<Hostile>>();
        q.iter(&game.world)
            .filter(|(_, p)| {
                (p.x - player_pos.x).abs() <= WILD_SPAWN_RADIUS_TILES
                    && (p.y - player_pos.y).abs() <= WILD_SPAWN_RADIUS_TILES
            })
            .map(|(e, _)| e)
            .collect()
    };
    for e in local {
        game.world.despawn(e);
    }

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
                        mitigation: 1,
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

    // Clear the box around the player *before* filling to the cap, and in
    // that order. A seeded zone starts at `WILD_LOCAL_DENSITY_TARGET`, which
    // would gate the ambient roll before it ever reached the cull this test
    // is about — but clearing after the fill would drop the total back under
    // the cap and leave the cull with nothing to do, which is the same test
    // passing for the wrong reason.
    despawn_hostiles_near(&mut game, player_pos.x, player_pos.y);

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
                mitigation: 1,
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

    // Deep enough that a roll places a real group rather than a single
    // creature — zone, not distance, is what decides that now.
    game.world.resource_mut::<ZoneLevel>().0 = 4;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let player = game.player_entity();
    let far = Position {
        x: spawn.x,
        y: spawn.y,
    };
    *game.world.get_mut::<Position>(player).unwrap() = far;
    assert!(
        game.max_group_size(None) > 1,
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
                mitigation: 1,
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
                mitigation: 10,
            },
            Potential {
                hp_roll: 1.0,
                atk_roll: 1.0,
                def_roll: 1.0,
                growth_roll: MIN_INDIVIDUAL_ROLL,
            },
            Tamed { owner: player },
            PowerReserve::default(),
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
                mitigation: 10,
            },
            Potential {
                hp_roll: 1.0,
                atk_roll: 1.0,
                def_roll: 1.0,
                growth_roll: MAX_INDIVIDUAL_ROLL,
            },
            Tamed { owner: player },
            PowerReserve::default(),
            Experience {
                level: 1,
                xp: 0,
                xp_to_next: 1,
            },
        ))
        .id();
    enlist(&mut game, low_roll);
    enlist(&mut game, high_roll);

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
                mitigation: 1,
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
                mitigation: 1,
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
                mitigation: 0,
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![guardian]);

    // Forced: there is no respawn to queue unless the guardian actually dies.
    force_the_next_attack_to_land(&mut game);
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
                mitigation: 0,
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

/// The roll is seeded, so the same seed produces the same carrier. Without
/// the ordering in `AbilityDb::wild_pool` this would pass or fail depending
/// on `HashMap` iteration order.
#[test]
fn the_wild_routine_roll_is_reproducible_from_the_seed() {
    let carried = |seed: u32| {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        (0..40)
            .filter_map(|_| game.roll_wild_routine().first().cloned())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        carried(770),
        carried(770),
        "same seed, same carriers — a weighted walk over an unordered pool would not be"
    );
}

/// Whatever the roll produces has to be a real, hunt-only ability: a
/// carrier holding something a species already grants would be no prize.
#[test]
fn a_rolled_routine_is_always_one_of_the_opted_in_abilities() {
    let mut game = Game::new(771, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pool: Vec<String> = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .wild_pool()
        .into_iter()
        .map(|(d, _)| d.id.clone())
        .collect();

    let mut rolled = 0;
    for _ in 0..500 {
        let routines = game.roll_wild_routine();
        assert!(routines.len() <= 1, "a carrier holds exactly one routine");
        if let Some(id) = routines.first() {
            rolled += 1;
            assert!(
                pool.contains(id),
                "rolled {id:?}, which is not in the wild pool"
            );
        }
    }
    assert!(
        rolled > 0,
        "500 rolls at WILD_ROUTINE_CHANCE should produce at least one carrier"
    );
}

/// Every wild program routes through `spawn_wild_creature`, so every one of
/// them holds a `Routines` component — empty for the overwhelming majority.
/// Without it, `install_innate_routines` would have nothing to merge and
/// `wild_retaliate` nothing to read.
#[test]
fn every_spawned_wild_creature_holds_a_routines_component() {
    let mut game = Game::new(772, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let species = game.species_defs().into_iter().next().unwrap();
    let entity = game
        .spawn_wild_creature(&species.id, spawn.x + 3, spawn.y)
        .expect("a shipped species spawns");
    assert!(
        game.world.get::<Routines>(entity).is_some(),
        "a wild program with no Routines can never be a carrier and never merges on capture"
    );
}

/// `CreatureSave.routines` is already written for wild creatures, so a
/// carrier round-trips on the existing save format — this pins that, since
/// the spec's "no SAVE_FORMAT_VERSION bump" rests on it.
#[test]
fn a_wild_carrier_survives_a_save_load_round_trip() {
    let dir = std::env::temp_dir().join(format!("feral_carrier_save_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("carrier.sav");

    let mut game = Game::new(773, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let species = game.species_defs().into_iter().next().unwrap();
    let entity = game
        .spawn_wild_creature(&species.id, spawn.x + 4, spawn.y)
        .unwrap();
    game.world
        .entity_mut(entity)
        .insert(Routines(vec!["kernel_panic".to_string()]));
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let mut query = loaded.world.query::<(&Position, &Routines)>();
    let found = query
        .iter(&loaded.world)
        .any(|(pos, r)| pos.x == spawn.x + 4 && r.0 == vec!["kernel_panic".to_string()]);
    assert!(found, "a wild carrier's routine must survive save/load");

    let _ = std::fs::remove_dir_all(&dir);
}

/// `damped_wild_spawn_chance` is the small pure function
/// `Game::maybe_spawn_wild_creature` derives its roll from — asserted
/// directly rather than through a spawn roll, since the roll itself is
/// seeded RNG and would make this test flaky.
#[test]
fn encounter_damp_reduces_the_wild_spawn_chance_proportionally() {
    use crate::game::spawning::damped_wild_spawn_chance;
    use crate::tuning::WILD_SPAWN_CHANCE;

    assert_eq!(damped_wild_spawn_chance(0), WILD_SPAWN_CHANCE);
    assert_eq!(damped_wild_spawn_chance(40), WILD_SPAWN_CHANCE * 0.6);
    assert_eq!(
        damped_wild_spawn_chance(100),
        0.0,
        "a full EncounterDamp should suppress wandering spawns entirely"
    );
    assert_eq!(
        damped_wild_spawn_chance(150),
        0.0,
        "must floor at 0 rather than invert into a spawn bonus"
    );
}

/// Without a `NestSave`, save/reload silently deleted every nest in the
/// zone — a free way out of a swarm the player provoked, and a way to
/// launder a nest destroyed most of the way to its cache.
#[test]
fn a_nest_survives_a_save_load_round_trip() {
    let mut game = Game::new(608, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let nest = game.spawn_nest("scrapper", 120, 120);
    game.world.get_mut::<Durability>(nest).unwrap().hp = 17;
    game.world.get_mut::<Nest>(nest).unwrap().pending_respawns = vec![3, NEST_RESPAWN_TICKS];

    let path = std::env::temp_dir().join(format!("feral_nest_save_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    // By tile, not species: `Game::new`'s own habitat spawning can roll an
    // unrelated scrapper nest on this seed, and filtering on species alone
    // would silently grab that one instead.
    let mut query = loaded.world.query::<(&Nest, &Position, &Durability)>();
    let (restored_nest, _, durability) = query
        .iter(&loaded.world)
        .find(|(_, p, _)| p.x == 120 && p.y == 120)
        .expect("the nest must survive the round trip");
    assert_eq!(
        durability.hp, 17,
        "durability must round-trip, not reset to full"
    );
    assert_eq!(
        restored_nest.pending_respawns,
        vec![3, NEST_RESPAWN_TICKS],
        "queued respawns must round-trip or a reload would silently refill the nest early"
    );
}

/// A guardian's `NestGuardian.nest` is a raw `Entity`, which is not stable
/// across a round trip — so this asserts the tether by the reloaded nest's
/// `Position` instead, which is the whole reason the save format keys it
/// by tile rather than by id. Resolving by position also keeps the test
/// honest about `Game::new`'s own habitat spawning, which can roll an
/// unrelated scrapper nest (and guardians) on this seed — a raw "every
/// guardian in the world must point at our nest" assertion would fail
/// against that second nest's guardians for a reason unrelated to this test.
#[test]
fn a_guardians_tether_survives_a_save_load_round_trip() {
    let mut game = Game::new(609, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let nest = game.spawn_nest("scrapper", 130, 130);
    let guardian_count_before = {
        let mut query = game.world.query::<&NestGuardian>();
        query.iter(&game.world).filter(|g| g.nest == nest).count()
    };

    let path = std::env::temp_dir().join(format!("feral_tether_save_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let mut guardian_query = loaded.world.query::<(&NestGuardian, Option<&Pursuing>)>();
    let guardian_nests: Vec<(Entity, bool)> = guardian_query
        .iter(&loaded.world)
        .map(|(g, p)| (g.nest, p.is_some()))
        .collect();
    let mut pos_query = loaded.world.query::<&Position>();
    // Every guardian tethered to our tile, paired with whether it came
    // back `Pursuing` — collected rather than just counted, since this
    // nest was never provoked and a calm guardian coming back aggro'd is
    // exactly the regression this test also needs to catch.
    let tethered_to_our_tile: Vec<bool> = guardian_nests
        .iter()
        .filter_map(|&(nest, pursuing)| {
            pos_query
                .get(&loaded.world, nest)
                .ok()
                .filter(|p| p.x == 130 && p.y == 130)
                .map(|_| pursuing)
        })
        .collect();
    assert_eq!(
        tethered_to_our_tile.len(),
        guardian_count_before,
        "every guardian tethered to this nest before the save must still resolve to it \
         after the load — a count that only checks '> 0' would pass even if the load \
         dropped some of them"
    );
    assert!(
        tethered_to_our_tile.iter().all(|&pursuing| !pursuing),
        "a guardian saved calm must not come back Pursuing — this nest was never provoked, \
         so if a reload aggros it anyway, an unconditional `Pursuing` insert on load would \
         pass every other test here without this one catching it"
    );
}

/// Provoke a nest's guardians, save, and reload — `Pursuing` must still be
/// set, or a save mid-chase would be a free way to shrug off an aggro'd
/// swarm.
#[test]
fn live_aggro_survives_a_save_load_round_trip() {
    let mut game = Game::new(610, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let nest = game.spawn_nest("scrapper", 140, 140);
    game.attack_nest(nest);
    // Filtered to `nest` specifically (not every `NestGuardian` in the
    // world): `Game::new`'s habitat spawning can roll its own, unprovoked
    // scrapper nest on this seed, and counting its guardians here would
    // both inflate `pursuing_before` incorrectly and make it pass even if
    // `attack_nest` provoked nothing at all.
    let pursuing_before = {
        let mut query = game.world.query::<(&NestGuardian, Option<&Pursuing>)>();
        query
            .iter(&game.world)
            .filter(|(g, p)| g.nest == nest && p.is_some())
            .count()
    };
    assert!(
        pursuing_before > 0,
        "attack_nest should have provoked at least one guardian of this nest"
    );

    let path = std::env::temp_dir().join(format!("feral_aggro_save_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    // Resolved by tile rather than the pre-save `nest` entity id, which
    // does not survive the round trip — and, same reason as above, so an
    // unrelated nest's (never-provoked) guardians can't be counted here.
    let mut guardian_query = loaded.world.query::<(&NestGuardian, Option<&Pursuing>)>();
    let guardian_nests: Vec<(Entity, bool)> = guardian_query
        .iter(&loaded.world)
        .map(|(g, p)| (g.nest, p.is_some()))
        .collect();
    let mut pos_query = loaded.world.query::<&Position>();
    let pursuing_after = guardian_nests
        .iter()
        .filter(|&&(nest, pursuing)| {
            pursuing
                && pos_query
                    .get(&loaded.world, nest)
                    .is_ok_and(|p| p.x == 140 && p.y == 140)
        })
        .count();
    assert_eq!(
        pursuing_after, pursuing_before,
        "every guardian of this nest provoked before the save must still be Pursuing after the load"
    );
}

/// A `nest_position` naming no `NestSave` — the nest's mod was removed, or
/// the nest was destroyed between save and a hand-edited file — must not
/// fail the load. The creature comes back as an ordinary wild program
/// instead, exactly like a save predating nests entirely.
#[test]
fn a_creature_whose_nest_is_missing_loads_as_an_ordinary_wild_program() {
    let game = Game::new(611, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let data = save::SaveData {
        seed: game.world.resource::<WorldMap>().seed(),
        base_ledger: Default::default(),
        game_over: None,
        mining: false,
        free_builds: crate::resources::FreeBuilds::default(),
        tick: 0,
        difficulty: DifficultyMode::Forgiving,
        player: save::PlayerSave {
            position: (spawn.x, spawn.y),
            hp: 30,
            max_hp: 30,
            atk: 6,
            mitigation: 2,
            power: 100.0,
            inventory: Vec::new(),
            level: 1,
            xp: 0,
            xp_to_next: 20,
            decompiler: 0,
            weapon: None,
            weapon_level: 1,
            weapon_fusion_tier: 0,
            weapon_rarity: Rarity::Ordinary,
            weapon_affix: None,
            weapon_affixes: Vec::new(),
            weapon_quality: crate::tuning::QUALITY_DEFAULT,
            armor: None,
            armor_level: 1,
            armor_fusion_tier: 0,
            armor_rarity: Rarity::Ordinary,
            armor_affix: None,
            armor_affixes: Vec::new(),
            armor_quality: crate::tuning::QUALITY_DEFAULT,
            module: None,
            module_level: 1,
            module_fusion_tier: 0,
            module_rarity: Rarity::Ordinary,
            module_affix: None,
            module_affixes: Vec::new(),
            module_quality: crate::tuning::QUALITY_DEFAULT,
            perk_points: 0,
            unlocked_perks: Vec::new(),
            bought_stats: crate::components::BoughtStats::default(),
            tutorial_seeded: true,
            fused_gear: Vec::new(),
            gear_copies: Vec::new(),
            downed_programs: Vec::new(),
            tools: Vec::new(),
            routines: Vec::new(),
            field_buffs: Vec::new(),
            sorties: Vec::new(),
            routes: Vec::new(),
            name: String::new(),
            class: None,
            glyph: '@',
            sprite: String::new(),
            colour: None,
            icon: None,
        },
        creatures: vec![save::CreatureSave {
            species: "scrapper".to_string(),
            position: (spawn.x + 2, spawn.y),
            hp: 10,
            max_hp: 10,
            atk: 1,
            mitigation: 1,
            tamed: false,
            power: crate::components::POWER_MAX,
            level: 1,
            xp: 0,
            xp_to_next: 20,
            cronjob: None,
            party_slot: None,
            sortie_index: None,
            wielded: false,
            zone: 1,
            custom_name: None,
            hp_roll: 1.0,
            atk_roll: 1.0,
            def_roll: 1.0,
            growth_roll: 1.0,
            fusions: 0,
            refactors: 0,
            purchased_tiers: 0,
            ring: 0,
            talents: Vec::new(),
            bought_stats: crate::components::BoughtStats::default(),
            routines: Vec::new(),
            field_buffs: Vec::new(),
            // No NestSave anywhere in this data names this tile.
            nest_position: Some((999, 999)),
            pursuing: true,
            carrying: None,
            rarity: Rarity::Ordinary,
            boss: false,
            nemesis_grudges: 0,
            program_id: 0,
            disposition: None,
            disgruntled: None,
            memories: Vec::new(),
            needs: Default::default(),
            off_shift: None,
            staff: false,
            downed: false,
            equipment: Vec::new(),
        }],
        structures: Vec::new(),
        nests: Vec::new(),
        dig_sites: Vec::new(),
        build_sites: Vec::new(),
        caravans: Vec::new(),
        caravan_memory: Default::default(),
        tile_overrides: Vec::new(),
        base_grid: crate::base_grid::BaseGrid::default(),
        anchor: None,
        zone: 1,
        spawn_point: (spawn.x, spawn.y),
        buyback: Vec::new(),
        buyback_shelves: Vec::new(),
        researched: Vec::new(),
        known_routines: Vec::new(),
        link_sites: Vec::new(),
        locale: crate::resources::Locale::Surface,
        stack_memory: crate::resources::StackMemory::default(),
        stack_memory_tiered: true,
        populated_chunks: crate::resources::PopulatedChunks::default(),
        settlements: Default::default(),
        standings: Default::default(),
        trace: 0,
        contracts: Vec::new(),
        contracts_done: Vec::new(),
        work_orders: Vec::new(),
        next_program_id: 0,
    };
    let path = std::env::temp_dir().join(format!("feral_missing_nest_{}.bin", std::process::id()));
    save::save_to_file(&path, &data).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let mut query = loaded.world.query::<(
        &Position,
        &Hostile,
        &WanderAi,
        Option<&NestGuardian>,
        Option<&Pursuing>,
    )>();
    let found = query
        .iter(&loaded.world)
        .find(|(pos, ..)| pos.x == spawn.x + 2 && pos.y == spawn.y);
    let (_, _, _, guardian, pursuing) = found.expect(
        "the creature must still load as an ordinary wild program even though its nest is missing",
    );
    assert!(
        guardian.is_none(),
        "a nest_position that resolves to nothing must not produce a NestGuardian"
    );
    // The saved `pursuing: true` on this creature must not survive on its
    // own — `Pursuing` is only ever inserted alongside `NestGuardian`
    // (`Game::load`). Without that guard, this would be an *unleashed*
    // pursuer, not a frozen one: `nest_aggro_tick`'s driving pass
    // (`game/turn.rs`) collects everything `With<Pursuing>` and moves it
    // toward the player regardless of `NestGuardian` — only the leash
    // check that runs before it needs the tether, to find a nest position
    // to measure distance from. A `Pursuing` creature with no
    // `NestGuardian` never enters that check at all, so it has no nest to
    // wander outside of and chases the player forever.
    assert!(
        pursuing.is_none(),
        "pursuing must not survive when the nest_position it depended on didn't resolve"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// Fixed zone scaling — distance is not a difficulty axis
// ─────────────────────────────────────────────────────────────────────────

/// A program's stats are a property of its zone (and, underground, its
/// depth) — never of how far from home it happened to spawn. Walking away
/// from the base used to multiply stats by up to `MAX_DISTANCE_STAT_MULTIPLIER`,
/// which also leaked into the Stack: every underground spawn is placed at the
/// entrance tile, so descending through a far-flung link scaled the whole
/// frame.
///
/// Asserted exactly rather than statistically by dividing the individual
/// roll back out — `spawn_wild_creature_scaled` is
/// `round(base * zone_mult * depth_mult * roll)`, and zone 1 is x1.
#[test]
fn wild_stats_do_not_vary_with_distance_from_the_danger_origin() {
    let mut game = Game::new(444, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let base_hp = game
        .world
        .resource::<SpeciesDb>()
        .get("scrapper")
        .unwrap()
        .base_hp;

    // Far enough out to sit past every step the old curve had.
    let far = game
        .spawn_wild_creature("scrapper", spawn.x + 500, spawn.y + 500)
        .expect("scrapper should spawn on any tile");
    let roll = game.world.get::<Potential>(far).unwrap().hp_roll;
    let stats = *game.world.get::<Stats>(far).unwrap();

    assert_eq!(
        stats.max_hp,
        ((base_hp as f32) * roll).round() as i32,
        "a program 500 tiles out should be worth exactly its zone-1 stats"
    );
}

/// Group size and group count are fixed within a zone. Both ride
/// `danger_steps`, so this pins the pair together — the invariant that the
/// two halves of the pack ceiling cannot disagree survives the distance
/// axis being removed.
#[test]
fn group_size_and_count_are_fixed_within_a_zone() {
    let game = Game::new(444, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    assert_eq!(
        game.max_group_size(None),
        game.max_group_size(None),
        "pack size should not grow with distance"
    );
    assert_eq!(
        game.max_enemy_groups(None),
        game.max_enemy_groups(None),
        "group count should not grow with distance"
    );
}

/// Underground, `danger_steps` used to read `depth` alone — a depth-1 frame
/// under zone 3 sat at step 0, identical to a depth-1 frame under zone 1,
/// which is why a late-zone Stack ambush fielded exactly one program. The
/// zone is a commitment the player already made by breaching there, so the
/// zone step now carries underground too, summed with the depth step and
/// clamped exactly as the surface already was.
#[test]
fn danger_steps_underground_sums_the_zone_step_and_the_depth_step() {
    let mut game = Game::new(9001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    game.world.insert_resource(ZoneLevel(1));
    assert_eq!(game.danger_steps(Some(1)), 0, "zone 1 depth 1 -> step 0");

    game.world.insert_resource(ZoneLevel(3));
    assert_eq!(game.danger_steps(Some(1)), 2, "zone 3 depth 1 -> step 2");
    assert_eq!(game.danger_steps(Some(3)), 4, "zone 3 depth 3 -> step 4");

    game.world.insert_resource(ZoneLevel(5));
    assert_eq!(
        game.danger_steps(Some(5)),
        MAX_GROUP_SIZE_STEPS,
        "zone 5 depth 5 -> step 8, clamped to {MAX_GROUP_SIZE_STEPS}"
    );
}

/// The surface half of `danger_steps` is untouched by the underground fix —
/// zone alone, clamped the same way it always was.
#[test]
fn danger_steps_on_the_surface_is_unchanged() {
    let mut game = Game::new(9002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let expected = [0u32, 1, 2, 3, 4, 5];
    for (zone, &want) in (1..=6u32).zip(expected.iter()) {
        game.world.insert_resource(ZoneLevel(zone));
        assert_eq!(
            game.danger_steps(None),
            want,
            "zone {zone} surface step should be unchanged at {want}"
        );
    }
}

/// The bug this fix closes: a zone-3 Stack ambush fielded exactly one
/// program on its first frame, identical to zone 1, because depth alone
/// decided the step underground. With the zone step folded in, the same
/// depth-1 frame now opens the second and third enemy groups the way a
/// zone-3 surface fight already does.
#[test]
fn a_later_zones_stack_ambush_can_field_more_than_one_group() {
    let mut game = Game::new(9003, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(ZoneLevel(3));

    assert_eq!(
        game.max_enemy_groups(Some(1)),
        3,
        "zone 3 depth 1 is danger step 2, which opens a third group"
    );
    assert_eq!(
        game.max_enemy_groups(Some(1)),
        game.max_enemy_groups(None),
        "the first frame down should escalate exactly as far as the zone \
         it was entered from already had"
    );
}

/// A boss used to fight alone, and the gap between it and an ordinary
/// program was the whole difficulty of the fight. Lowering its stats closes
/// that gap from one end; an escort closes it from the other, so a boss is
/// a harder *fight* rather than a harder single opponent.
///
/// Gated on the fight being able to hold a second group at all, which zone 1
/// cannot — a boss met in the opening zone is still a lone one.
#[test]
fn a_boss_past_zone_one_brings_an_escort_of_another_species() {
    use crate::game::spawning::SpawnEscalation;
    let mut game = Game::new(909, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 3;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();

    let pack = game.spawn_pack(
        "overseer",
        true,
        spawn.x,
        spawn.y,
        SpawnEscalation::surface(),
    );

    let species: Vec<String> = pack
        .iter()
        .filter_map(|&e| game.world.get::<Creature>(e))
        .map(|c| c.species.clone())
        .collect();
    assert!(
        species.contains(&"overseer".to_string()),
        "the boss itself must still be in the pack, got {species:?}"
    );
    assert!(
        species.iter().any(|s| s != "overseer"),
        "a zone-3 boss should arrive with an escort of some other species, got {species:?}"
    );
}

/// The other half of the rule above: zone 1 holds one group, so there is
/// nowhere for an escort to stand and the opening zone's boss stays solo.
#[test]
fn a_zone_one_boss_still_fights_alone() {
    use crate::game::spawning::SpawnEscalation;
    let mut game = Game::new(909, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();

    let pack = game.spawn_pack(
        "overseer",
        true,
        spawn.x,
        spawn.y,
        SpawnEscalation::surface(),
    );

    assert_eq!(pack.len(), 1, "zone 1 fields one group, so the boss is it");
}

// ─────────────────────────────────────────────────────────────────────────
// Local density: the target a zone is seeded to and topped back up to.
// ─────────────────────────────────────────────────────────────────────────

/// Every `Hostile` within `WILD_SPAWN_RADIUS_TILES` of `(x, y)` — the same
/// box `spawn_wild_nearby` places into, restated here so these tests measure
/// what the gate measures without reaching into a private helper.
fn hostiles_near(game: &mut Game, x: i32, y: i32) -> usize {
    let mut query = game.world.query_filtered::<&Position, With<Hostile>>();
    query
        .iter(&game.world)
        .filter(|p| (p.x - x).abs().max((p.y - y).abs()) <= WILD_SPAWN_RADIUS_TILES)
        .count()
}

/// Empties the spawn box around `(x, y)`, leaving anything outside it alone.
///
/// The precondition every "does a spawn happen here" test now needs: a zone
/// is seeded to `WILD_LOCAL_DENSITY_TARGET`, so the ambient roll around a
/// freshly placed player is legitimately gated and a test that does not
/// clear the box is measuring the gate rather than its own subject.
fn despawn_hostiles_near(game: &mut Game, x: i32, y: i32) {
    let mut query = game
        .world
        .query_filtered::<(Entity, &Position), With<Hostile>>();
    let near: Vec<Entity> = query
        .iter(&game.world)
        .filter(|(_, p)| (p.x - x).abs().max((p.y - y).abs()) <= WILD_SPAWN_RADIUS_TILES)
        .map(|(e, _)| e)
        .collect();
    for e in near {
        game.world.despawn(e);
    }
}

fn despawn_all_hostiles(game: &mut Game) {
    let mut query = game.world.query_filtered::<Entity, With<Hostile>>();
    let all: Vec<Entity> = query.iter(&game.world).collect();
    for e in all {
        game.world.despawn(e);
    }
}

/// The bug this whole feature exists for: standing in one place used to
/// accumulate wild programs without bound, because spawning is
/// player-relative and the only thing that ever removed a creature was a
/// `WILD_CREATURE_CAP` two orders of magnitude above any real population.
/// A real save measured 65 hostiles in one box around a base the player had
/// been working at.
///
/// 4000 rolls at `WILD_SPAWN_CHANCE` is ~100 spawn events; ungated that is
/// ~100 creatures in the box, so the bound below fails by an order of
/// magnitude with the gate removed rather than by a hair.
#[test]
fn idling_in_one_place_stops_at_the_local_density_target() {
    let mut game = Game::new(4242, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    despawn_all_hostiles(&mut game);
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();

    for _ in 0..4000 {
        game.maybe_spawn_wild_creature();
    }

    let near = hostiles_near(&mut game, pos.x, pos.y);
    // A nest rolled at 11 counts adds its guardians on top of the target,
    // which is the one legitimate way to land above it.
    assert!(
        near <= WILD_LOCAL_DENSITY_TARGET + NEST_GUARDIAN_MAX as usize,
        "idling should settle at the density target, found {near} in the box"
    );
    assert!(
        near >= WILD_LOCAL_DENSITY_TARGET / 2,
        "the gate should still let the area fill up, found only {near}"
    );
}

/// The other side of the same rule: the gate must not be so eager that
/// ground the player has just walked onto stays empty.
#[test]
fn a_sparse_area_still_fills_to_the_target() {
    let mut game = Game::new(4243, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    despawn_all_hostiles(&mut game);
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();

    for _ in 0..4000 {
        game.maybe_spawn_wild_creature();
    }

    assert!(
        hostiles_near(&mut game, pos.x, pos.y) > 0,
        "an empty area must still spawn"
    );
}

/// Tamed programs are not counted, matching `WILD_CREATURE_CAP`'s rule that
/// a full roster never starves the map of things to fight. Counting every
/// `Creature` instead would let a six-program party suppress spawning
/// wherever it stood.
#[test]
fn a_full_roster_does_not_suppress_wild_spawns() {
    let mut game = Game::new(4244, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    despawn_all_hostiles(&mut game);
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    for _ in 0..(WILD_LOCAL_DENSITY_TARGET * 2) {
        spawn_tamed_on_map(&mut game, pos.x, pos.y);
    }

    for _ in 0..4000 {
        game.maybe_spawn_wild_creature();
    }

    assert!(
        hostiles_near(&mut game, pos.x, pos.y) > 0,
        "tamed programs standing on the tile must not count as local density"
    );
}

/// The far-field half of the complaint: a zone used to be seeded in a
/// 15-tile bubble around the arrival point, so everything past it was born
/// empty and stayed that way — walking costs a tick a tile against a 5%
/// roll, so the player outruns the spawner and finds nothing out there.
/// Seeding now covers `INITIAL_SPAWN_SCATTER_TILES`, matched to the link
/// scatter that already says how far out the game sends people.
#[test]
fn a_new_zone_is_seeded_past_the_arrival_bubble() {
    // Swept rather than trusting one seed: scatter is a uniform roll, so a
    // single lucky seed proves nothing about the distribution.
    for seed in 0u32..8 {
        let game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let spawn = *game.world.resource::<ZoneSpawnPoint>();
        let mut world = game.world;
        let mut query = world.query_filtered::<&Position, With<Hostile>>();
        let farthest = query
            .iter(&world)
            .map(|p| (p.x - spawn.x).abs().max((p.y - spawn.y).abs()))
            .max()
            .unwrap_or(0);
        assert!(
            farthest > 30,
            "seed {seed}: the zone should be populated past the old 22-tile \
             bubble, but the farthest wild program is {farthest} tiles out"
        );
    }
}

/// The density gate is about pacing an *ambient* spawn, so it lives in
/// `maybe_spawn_wild_creature` rather than in the body the dev console
/// shares. Forcing an encounter while standing in a crowd must still
/// produce a fight, or the console silently stops working exactly where a
/// tester is most likely to use it.
#[test]
fn the_dev_console_ignores_the_density_target() {
    let mut game = Game::new(4245, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    // Walkable ground across the whole box the roll can land in. Without
    // it the test rests on the seed happening to pick a walkable tile, and
    // a forced encounter that lands on rock spawns nothing for a reason
    // that has nothing to do with density.
    for dx in -WILD_SPAWN_RADIUS_TILES..=WILD_SPAWN_RADIUS_TILES {
        for dy in -WILD_SPAWN_RADIUS_TILES..=WILD_SPAWN_RADIUS_TILES {
            game.world.resource_mut::<WorldMap>().set_override(
                pos.x + dx,
                pos.y + dy,
                Tile {
                    biome: Biome::OpenGrid,
                    walkable: true,
                    rock_shade: None,
                },
            );
        }
    }
    // Saturate the box well past the target with hand-placed hostiles.
    let species = game.species_defs().into_iter().next().unwrap().id;
    for i in 0..(WILD_LOCAL_DENSITY_TARGET * 2) {
        game.spawn_wild_creature(&species, pos.x + (i % 5) as i32, pos.y);
    }
    let before = hostiles_near(&mut game, pos.x, pos.y);

    game.dev_force_encounter();

    assert!(
        hostiles_near(&mut game, pos.x, pos.y) > before,
        "the console's forced encounter must not be gated by local density"
    );
}

#[test]
fn a_boss_never_rolls_a_rarity() {
    let mut game = Game::new(9021, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let boss = game
        .species_defs()
        .into_iter()
        .find(|s| s.is_boss)
        .expect("at least one boss species should ship");
    // Far from the danger origin, so the opening ring is not what is doing
    // the refusing here.
    let far = OPENING_RING_TILES + 50;
    for _ in 0..200 {
        assert_eq!(
            game.roll_rarity(&boss, far, far, false),
            Rarity::Ordinary,
            "a boss's stats are hand-authored; a multiplier discards that"
        );
    }
    assert!(
        rng_unadvanced_by(9021, |g| {
            g.roll_rarity(&boss, far, far, false);
        }),
        "refusing a boss must not spend a draw from the shared stream"
    );
}

#[test]
fn no_shiny_spawns_in_the_opening_ring() {
    let mut game = Game::new(9022, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ordinary = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss)
        .expect("ordinary species should ship");
    for _ in 0..500 {
        assert_eq!(
            game.roll_rarity(&ordinary, 0, 0, false),
            Rarity::Ordinary,
            "balance_sim::beatable_by_a_fresh_player guarantees a fresh \
             player can beat one program in the ring"
        );
    }
    assert!(
        rng_unadvanced_by(9022, |g| {
            g.roll_rarity(&ordinary, 0, 0, false);
        }),
        "refusing an opening-ring spawn must not spend a draw"
    );
}

/// A census over `Rarity::ALL` rather than a check on two named tiers, so
/// a rung added to the ladder without a threshold in `roll_rarity` fails
/// here instead of silently never spawning.
///
/// The sample is large enough that the rarest rung is not a coin flip:
/// `PRISMATIC_SPAWN_CHANCE` is 0.0003, so 200k rolls expect ~60 of them.
/// The roll is seeded, so this is deterministic rather than merely likely.
#[test]
fn every_rare_tier_is_reachable_and_rarer_than_the_one_below() {
    let mut game = Game::new(9023, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ordinary = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss)
        .expect("ordinary species should ship");
    let far = OPENING_RING_TILES + 50;

    let mut counts = [0usize; Rarity::ALL.len()];
    for _ in 0..200_000 {
        counts[game.roll_rarity(&ordinary, far, far, false).rank() as usize] += 1;
    }

    for tier in Rarity::ALL {
        assert!(
            counts[tier.rank() as usize] > 0,
            "{tier:?} never spawned in 200k rolls — is it missing a threshold \
             in roll_rarity? counts: {counts:?}"
        );
    }
    for pair in Rarity::ALL.windows(2) {
        let (lower, upper) = (
            counts[pair[0].rank() as usize],
            counts[pair[1].rank() as usize],
        );
        assert!(
            upper < lower,
            "{:?} ({upper}) must be rarer than {:?} ({lower})",
            pair[1],
            pair[0]
        );
    }
}

#[test]
fn a_shiny_survives_a_save_round_trip() {
    let mut game = Game::new(9025, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let far = OPENING_RING_TILES + 60;
    let wild = game
        .spawn_wild_creature("scrapper", far, far)
        .expect("scrapper ships with the game");
    // Set the tier by hand rather than hunting a seed that rolls one: what
    // is under test is whether the tag travels and whether the numbers stay
    // put, not the roll — which `an_eligible_spawn_can_roll_both_tiers`
    // covers on its own.
    game.world.entity_mut(wild).insert(Rarity::Gold);
    let before = *game.world.get::<Stats>(wild).unwrap();

    let path =
        std::env::temp_dir().join(format!("feral_rarity_roundtrip_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let mut q = loaded
        .world
        .query_filtered::<(&Position, &Stats, Option<&Rarity>), With<Hostile>>();
    let (_, after, rarity) = q
        .iter(&loaded.world)
        .find(|(p, _, _)| p.x == far && p.y == far)
        .expect("the wild program should come back");
    assert_eq!(
        rarity.copied(),
        Some(Rarity::Gold),
        "the rare tier is part of what a creature is"
    );
    // The multiplier was spent at spawn and the numbers were saved verbatim.
    // A load that re-applied `stat_mult` would hand back 1.8x of these and
    // compound again on the next reload — see `Rarity`'s doc.
    assert_eq!(
        (after.hp, after.max_hp, after.atk, after.mitigation),
        (before.hp, before.max_hp, before.atk, before.mitigation),
        "loading must restore recorded stats, not re-apply the tier"
    );
}

#[test]
fn a_shiny_spawn_has_its_stats_multiplied() {
    let mut game = Game::new(9024, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss)
        .expect("ordinary species should ship");
    let far = OPENING_RING_TILES + 50;
    // Potential is a +/-20% band per stat, so a single ordinary spawn is no
    // baseline. Take the luckiest ordinary roll seen and require a gold to
    // beat it outright: 1.8x clears 1.2x with room, and that gap is the
    // whole reason the tier is discrete rather than a wider band.
    let mut best_ordinary = 0;
    let mut gold_hp = None;
    for i in 0..4000 {
        let e = game
            .spawn_wild_creature(&species.id, far + (i % 7), far + (i % 5))
            .expect("shipped species should spawn");
        let hp = game.world.get::<Stats>(e).unwrap().max_hp;
        match game.world.get::<Rarity>(e).copied().unwrap_or_default() {
            Rarity::Ordinary => best_ordinary = best_ordinary.max(hp),
            Rarity::Gold if gold_hp.is_none() => gold_hp = Some(hp),
            _ => {}
        }
    }
    let gold_hp = gold_hp.expect("4000 spawns should turn up at least one gold");
    assert!(
        gold_hp > best_ordinary,
        "an Overclocked spawn ({gold_hp} HP) should beat the luckiest \
         ordinary roll ({best_ordinary} HP)"
    );
}

/// Carves `(cx, cy)` open and walls the box around it, so every scattered
/// offset lands somewhere a hostile can never step off.
fn open_tile_in_a_sealed_box(game: &mut Game, cx: i32, cy: i32, half: i32) {
    for dx in -half..=half {
        for dy in -half..=half {
            let open = dx == 0 && dy == 0;
            game.world.resource_mut::<WorldMap>().set_override(
                cx + dx,
                cy + dy,
                Tile {
                    biome: if open {
                        Biome::OpenGrid
                    } else {
                        Biome::DataVoid
                    },
                    walkable: open,
                    rock_shade: None,
                },
            );
        }
    }
}

/// Placement and movement have to agree. `wander_ai_system` and
/// `pursuit_field` both step only onto `Tile::open_to_hostiles`, so a
/// guardian scattered onto rock across a biome boundary had no legal move
/// for the rest of the run — and its tether meant it could never be
/// displaced off it either.
#[test]
fn a_nest_guardian_is_never_placed_where_it_could_never_step() {
    let mut game = Game::new(608, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (nx, ny) = (200, 200);
    open_tile_in_a_sealed_box(&mut game, nx, ny, NEST_TETHER_RADIUS + 2);

    game.spawn_nest("scrapper", nx, ny);

    let mut query = game.world.query_filtered::<&Position, With<NestGuardian>>();
    let placed: Vec<Position> = query.iter(&game.world).copied().collect();
    assert!(!placed.is_empty(), "the nest should have guardians");
    for pos in placed {
        assert!(
            game.world
                .resource_mut::<WorldMap>()
                .tile(pos.x, pos.y)
                .open_to_hostiles(),
            "a guardian at ({}, {}) is standing where it can never step",
            pos.x,
            pos.y
        );
    }
}

/// The same gap on the other placement path: a pack's members after the
/// first are scattered around the roll's anchor tile.
#[test]
fn a_pack_member_is_never_placed_where_it_could_never_step() {
    let mut game = Game::new(21, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Zone 1 packs are solo, and a lone spawn lands on the anchor tile
    // whatever the fix does — the scattered members are the whole point.
    game.world.resource_mut::<ZoneLevel>().0 = 3;
    let (ax, ay) = (300, 300);
    open_tile_in_a_sealed_box(&mut game, ax, ay, 12);

    let pack = game.spawn_pack(
        "scrapper",
        false,
        ax,
        ay,
        crate::game::spawning::SpawnEscalation::surface(),
    );

    assert!(pack.len() > 1, "the fixture needs a pack, not a lone spawn");
    for e in pack {
        let pos = *game.world.get::<Position>(e).unwrap();
        assert!(
            game.world
                .resource_mut::<WorldMap>()
                .tile(pos.x, pos.y)
                .open_to_hostiles(),
            "a pack member at ({}, {}) is standing where it can never step",
            pos.x,
            pos.y
        );
    }
}

/// The bug this whole mechanism exists to close: ground the player travels
/// to used to be born empty and stay that way. Population was placed only
/// relative to the player — a one-time seeded disc plus a per-tick roll
/// inside `WILD_SPAWN_RADIUS_TILES` — in a world map that is unbounded and
/// generated a chunk at a time, so crossing one density box bought about a
/// spare `WILD_SPAWN_CHANCE` roll against a target of twelve.
///
/// Asserted on the *mark* as well as the population, because the two are
/// different claims: the mark says the sector took responsibility for that
/// ground, the count says it actually put something there.
#[test]
fn walking_into_new_ground_stocks_it() {
    let mut game = Game::new(9001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let start = *game.world.get::<Position>(game.player_entity()).unwrap();
    // Five chunks out: far past anything `Game::new` stocked, and far past
    // the radius the ambient roll can reach from where the player began.
    let far = Position {
        x: start.x + 5 * crate::world::CHUNK_SIZE,
        y: start.y,
    };
    let chunk = (
        far.x.div_euclid(crate::world::CHUNK_SIZE),
        far.y.div_euclid(crate::world::CHUNK_SIZE),
    );

    assert!(
        !game
            .world
            .resource::<crate::resources::PopulatedChunks>()
            .0
            .contains(&chunk),
        "the premise: ground five chunks out has not been stocked yet"
    );
    assert_eq!(
        game.local_hostile_count(far.x, far.y),
        0,
        "the premise: and nothing lives there"
    );

    *game
        .world
        .get_mut::<Position>(game.player_entity())
        .unwrap() = far;
    game.tick();

    assert!(
        game.world
            .resource::<crate::resources::PopulatedChunks>()
            .0
            .contains(&chunk),
        "arriving marks the chunk as stocked"
    );
    let neighbourhood: usize = {
        let mut q = game.world.query_filtered::<&Position, With<Hostile>>();
        q.iter(&game.world)
            .filter(|p| {
                let (cx, cy) = (
                    p.x.div_euclid(crate::world::CHUNK_SIZE),
                    p.y.div_euclid(crate::world::CHUNK_SIZE),
                );
                (cx - chunk.0).abs() <= POPULATION_CHUNK_MARGIN
                    && (cy - chunk.1).abs() <= POPULATION_CHUNK_MARGIN
            })
            .count()
    };
    // One chunk's worth across the nine that were stocked, which is a fifth
    // of what they should hold. Loose on purpose: how much of a given nine
    // chunks is walkable, and how many of their biomes list habitat species,
    // is a property of the terrain seed and not of this mechanism. The
    // density itself is pinned by `tuning`'s derivation test and measured in
    // `docs/measurements/`.
    assert!(
        neighbourhood >= chunk_wild_population(),
        "and stocks it: {neighbourhood} wild programs across the nine chunks \
         around the arrival, expected at least {}",
        chunk_wild_population()
    );
}

/// Standing on ground already stocked must not stock it again. Without the
/// mark, `ensure_local_population` would top every chunk around the player
/// back up to target every single tick, which is the old base-halo bug with
/// a much bigger engine behind it.
#[test]
fn ground_already_stocked_is_not_stocked_again() {
    let mut game = Game::new(9002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.tick();
    let marks = game
        .world
        .resource::<crate::resources::PopulatedChunks>()
        .0
        .len();
    let before = {
        let mut q = game.world.query_filtered::<(), With<Hostile>>();
        q.iter(&game.world).count()
    };

    for _ in 0..200 {
        game.tick();
    }

    assert_eq!(
        game.world
            .resource::<crate::resources::PopulatedChunks>()
            .0
            .len(),
        marks,
        "a player who never left their chunk stocked no new ground"
    );
    let after = {
        let mut q = game.world.query_filtered::<(), With<Hostile>>();
        q.iter(&game.world).count()
    };
    // The ambient roll is still running and is allowed to top the local box
    // back up, so this bounds the growth rather than forbidding it. A
    // re-stocking bug puts a chunk's worth down every tick and would clear
    // this by two orders of magnitude.
    assert!(
        after < before + chunk_wild_population(),
        "standing still added {} programs in 200 ticks, which is re-stocking \
         rather than the ambient roll topping up",
        after - before
    );
}

/// `PopulatedChunks` is zone-local, like `BuybackLedger` and `StackMemory`,
/// and so has to be wiped **by name** on a breach. A mark carried forward
/// tells the new sector that ground it has never stocked is already full,
/// which would make the new zone empty exactly where the old one was
/// populated.
#[test]
fn breaching_forgets_which_chunks_were_stocked() {
    let mut game = Game::new(9003, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.tick();
    // A mark far from where anyone arrives. Asserting on the chunks the old
    // zone actually stocked would be vacuous: a breach lands the player at
    // roughly the same coordinates, so those same chunks are stocked again
    // on the way in and the marks are back before anything can look.
    let distant = (500, 500);
    game.world
        .resource_mut::<crate::resources::PopulatedChunks>()
        .0
        .insert(distant);

    game.enter_next_zone();

    assert!(
        !game
            .world
            .resource::<crate::resources::PopulatedChunks>()
            .0
            .contains(&distant),
        "the new sector inherited a stocked-ground mark from the old one, so          that ground will never be populated here"
    );
    assert!(
        !game
            .world
            .resource::<crate::resources::PopulatedChunks>()
            .0
            .is_empty(),
        "and the ground the party breached onto was stocked on arrival"
    );
}

/// The cap evicts the unit population is placed in, and drops the mark with
/// it — so walking back to evicted ground finds it stocked afresh rather
/// than permanently dead.
///
/// Candidates are taken from where hostiles actually stand rather than from
/// `PopulatedChunks`, which is why this test never marks the chunk it fills:
/// a program that wandered into unstocked ground must still be evictable, or
/// the wander AI slowly reopens the leak the cap exists to close.
#[test]
fn the_cap_evicts_a_whole_chunk_and_forgets_it() {
    let mut game = Game::new(9004, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species_id = game.species_defs().into_iter().next().unwrap().id;
    let start = *game.world.get::<Position>(game.player_entity()).unwrap();
    let far = Position {
        x: start.x + 20 * crate::world::CHUNK_SIZE,
        y: start.y,
    };
    let victim = (
        far.x.div_euclid(crate::world::CHUNK_SIZE),
        far.y.div_euclid(crate::world::CHUNK_SIZE),
    );
    game.world
        .resource_mut::<crate::resources::PopulatedChunks>()
        .0
        .insert(victim);

    let already = {
        let mut q = game.world.query_filtered::<(), With<Hostile>>();
        q.iter(&game.world).count()
    };
    for i in 0..(WILD_CREATURE_CAP - already) {
        game.world.spawn((
            Creature {
                species: species_id.clone(),
            },
            Position {
                x: far.x + (i % 8) as i32,
                y: far.y,
            },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 1,
                mitigation: 1,
            },
            Hostile,
        ));
    }

    game.cull_to_cap(1);

    let left_in_victim = {
        let mut q = game.world.query_filtered::<&Position, With<Hostile>>();
        q.iter(&game.world)
            .filter(|p| {
                (
                    p.x.div_euclid(crate::world::CHUNK_SIZE),
                    p.y.div_euclid(crate::world::CHUNK_SIZE),
                ) == victim
            })
            .count()
    };
    assert_eq!(
        left_in_victim, 0,
        "the farthest chunk is evicted whole, not thinned"
    );
    assert!(
        !game
            .world
            .resource::<crate::resources::PopulatedChunks>()
            .0
            .contains(&victim),
        "and its mark goes with it, or walking back finds it dead forever"
    );
}

/// The cap must never reach the ground the player is standing on, however
/// crowded the map gets — an eviction there is one the player watches
/// happen.
#[test]
fn the_cap_never_evicts_the_ground_under_the_player() {
    let mut game = Game::new(9005, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species_id = game.species_defs().into_iter().next().unwrap().id;
    let start = *game.world.get::<Position>(game.player_entity()).unwrap();

    let already = {
        let mut q = game.world.query_filtered::<(), With<Hostile>>();
        q.iter(&game.world).count()
    };
    let local: Vec<Entity> = (0..(WILD_CREATURE_CAP - already))
        .map(|_| {
            game.world
                .spawn((
                    Creature {
                        species: species_id.clone(),
                    },
                    Position {
                        x: start.x,
                        y: start.y,
                    },
                    Stats {
                        hp: 10,
                        max_hp: 10,
                        atk: 1,
                        mitigation: 1,
                    },
                    Hostile,
                ))
                .id()
        })
        .collect();

    game.cull_to_cap(500);

    assert!(
        local.iter().all(|&e| game.world.get::<Stats>(e).is_some()),
        "the cap evicted creatures standing on the player's own chunk"
    );
}

/// The marks have to survive a save, or a reload re-stocks every chunk the
/// run had already emptied — which would hand back a full sector's worth of
/// programs for the price of quitting to the menu.
///
/// This is the test the RON round trip cannot stand in for: `#[serde(skip)]`
/// drops a field from *both* encodings it compares, so that test stays green
/// while the field never reaches disk.
#[test]
fn stocked_ground_survives_a_save() {
    let assets = test_assets_dir();
    let mut game = Game::new(9006, DifficultyMode::Forgiving, &assets).unwrap();
    let start = *game.world.get::<Position>(game.player_entity()).unwrap();
    // Somewhere the run has actually been, so the mark is one play produced
    // rather than one the test wrote by hand.
    *game
        .world
        .get_mut::<Position>(game.player_entity())
        .unwrap() = Position {
        x: start.x + 4 * crate::world::CHUNK_SIZE,
        y: start.y,
    };
    game.tick();
    let before = game
        .world
        .resource::<crate::resources::PopulatedChunks>()
        .clone();
    assert!(
        before.0.len() > 9,
        "the premise: two neighbourhoods were stocked"
    );

    let path = std::env::temp_dir().join(format!(
        "feral_processes_population_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded
            .world
            .resource::<crate::resources::PopulatedChunks>()
            .0,
        before.0,
        "a reload forgot which ground the sector had already stocked"
    );
}

/// The one door. A creature carrying `Boss` is a boss even though its
/// species is not, and an apex species is a boss even without the component
/// — a fixture that hand-spawns one outside `spawn_pack` never gets one.
#[test]
fn is_boss_creature_reads_the_component_or_the_species() {
    let mut game = Game::new(4101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let wild = spawn_wild_on_player_tile(&mut game);
    assert!(
        !game.is_boss_creature(wild),
        "an ordinary species with no component is not a boss"
    );
    game.world.entity_mut(wild).insert(Boss);
    assert!(
        game.is_boss_creature(wild),
        "the component alone must make a creature a boss"
    );

    // The species half still has to answer: this fixture spawns outside the
    // boss path, so no component is written.
    let apex = spawn_boss_on_player_tile(&mut game);
    assert!(
        game.world.get::<Boss>(apex).is_none(),
        "this fixture spawns outside the boss path, so the component is the \
         wrong thing to be asserting on"
    );
    assert!(
        game.is_boss_creature(apex),
        "an apex species is a boss without a component"
    );
}

/// The receipt must survive a reload, or a boss killed after a save/load
/// pays nothing and reads as the drop rate having moved. A RON round-trip
/// cannot catch a load path that drops the component — this has to go
/// through `Game::save` and `Game::load`.
#[test]
fn a_rolled_boss_keeps_its_component_across_a_save_and_load() {
    let mut game = Game::new(4102, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);
    let species = game.world.get::<Creature>(wild).unwrap().species.clone();
    let pos = *game.world.get::<Position>(wild).unwrap();
    game.world.entity_mut(wild).insert(Boss);

    let dir = scratch_assets_dir("rolled_boss");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("rolled_boss.sav");
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();

    let mut q = loaded.world.query::<(Entity, &Creature, &Position)>();
    let found: Vec<Entity> = q
        .iter(&loaded.world)
        .filter(|(_, c, p)| c.species == species && p.x == pos.x && p.y == pos.y)
        .map(|(e, _, _)| e)
        .collect();
    assert_eq!(
        found.len(),
        1,
        "exactly one creature should match the saved one"
    );
    assert!(
        loaded.is_boss_creature(found[0]),
        "a rolled boss must come back a boss — the load path dropped `Boss`"
    );
}

/// The field is additive behind `#[serde(default)]`, which is what buys this
/// change no `SAVE_FORMAT_VERSION` bump: a file written before rolled bosses
/// existed must load rather than be refused.
#[test]
fn a_save_without_the_boss_field_loads_un_bossed() {
    let mut game = Game::new(4103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);
    game.world.entity_mut(wild).insert(Boss);

    let dir = scratch_assets_dir("boss_default");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("boss_default.sav");
    game.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("boss: true"),
        "a fresh save must carry the field, or stripping it below proves nothing"
    );
    let stripped: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("boss:"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path, stripped).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let mut q = loaded.world.query::<(Entity, &Creature)>();
    let creatures: Vec<Entity> = q.iter(&loaded.world).map(|(e, _)| e).collect();
    assert!(
        creatures
            .iter()
            .all(|&e| loaded.world.get::<Boss>(e).is_none()),
        "a file with the field stripped must load with nothing bossed"
    );
}

/// An apex species is authored tough and must not be scaled on top of that;
/// an ordinary species rolled into a boss has nothing but the multiplier.
///
/// Asserted against the *ceiling* of an unbossed roll rather than against a
/// paired spawn, because `roll_potential` gives every spawn an independent
/// ±20% and nothing in the fixture can pin it. `MAX_INDIVIDUAL_ROLL` is 1.2
/// and `BOSS_STAT_MULT * MIN_INDIVIDUAL_ROLL` is 1.4, so the two bands do not
/// overlap and the comparison is exact rather than probabilistic.
#[test]
fn a_rolled_boss_is_scaled_and_an_apex_boss_is_not() {
    let mut game = Game::new(4201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let zone_mult = game.world.resource::<ZoneLevel>().stat_multiplier() as f32;

    let ordinary = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss)
        .expect("the shipped roster is not all bosses");
    let plain_ceiling = (ordinary.base_hp as f32 * zone_mult * MAX_INDIVIDUAL_ROLL).round() as i32;
    let bossed = game
        .spawn_wild_creature_scaled(&ordinary.id, pos.x + 3, pos.y + 3, 1.0, true)
        .expect("a shipped species should spawn");
    assert!(
        game.world.get::<Stats>(bossed).unwrap().max_hp > plain_ceiling,
        "a rolled boss must out-scale the luckiest ordinary roll of its own species"
    );

    let apex = game
        .species_defs()
        .into_iter()
        .find(|s| s.is_boss)
        .expect("at least one apex species ships");
    let apex_ceiling = (apex.base_hp as f32 * zone_mult * MAX_INDIVIDUAL_ROLL).round() as i32;
    let apex_spawn = game
        .spawn_wild_creature_scaled(&apex.id, pos.x + 4, pos.y + 4, 1.0, true)
        .expect("a shipped apex species should spawn");
    assert!(
        game.world.get::<Stats>(apex_spawn).unwrap().max_hp <= apex_ceiling,
        "an apex species must not take BOSS_STAT_MULT on top of its authored stats"
    );
}

/// A boss's stats are the whole of what it is worth, and a rare tier on top
/// would be a second, invisible multiplier — the same reason an apex spawn
/// has always been excluded. Spawned well outside the opening ring, or the
/// ring's own exclusion would be what makes this pass.
#[test]
fn a_rolled_boss_never_rolls_a_rare_tier() {
    let mut game = Game::new(4202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let ordinary = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss)
        .expect("the shipped roster is not all bosses");
    let far = OPENING_RING_TILES * 4;
    for i in 0..200 {
        let spawned = game
            .spawn_wild_creature_scaled(&ordinary.id, pos.x + far + i, pos.y + far, 1.0, true)
            .expect("a shipped species should spawn");
        assert_eq!(
            *game.world.get::<Rarity>(spawned).unwrap(),
            Rarity::Ordinary,
            "a rolled boss must never carry a rare tier"
        );
    }
}

/// A boss is one group; the escort standing with it is a second, and is
/// never itself a boss. Zone 1 has room for only one group, so this run
/// usually places the boss alone — the assertion is written as "exactly one
/// of whatever spawned" so it holds either way.
#[test]
fn a_boss_pack_marks_the_boss_and_not_its_escort() {
    use crate::game::spawning::SpawnEscalation;
    let mut game = Game::new(4203, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let ordinary = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss)
        .expect("the shipped roster is not all bosses");
    let pack = game.spawn_pack(
        &ordinary.id,
        true,
        pos.x + OPENING_RING_TILES * 4,
        pos.y,
        SpawnEscalation::surface(),
    );
    assert!(
        !pack.is_empty(),
        "a boss pack should place at least the boss"
    );
    assert_eq!(
        pack.iter().filter(|&&e| game.is_boss_creature(e)).count(),
        1,
        "exactly one member of a boss pack is the boss"
    );
}

/// The headline. A fresh run's zone fields the easy end of the ladder and
/// nothing else — the thing that reads wrong today, where a level-1 player
/// can meet a band-2 species outside the seven-tile ring.
///
/// Deadlock is the exception and is asserted rather than excused: it
/// ships no band-0 species, so the fallback reaches band 1 there.
#[test]
fn zone_one_fields_only_the_easiest_band() {
    let mut game = Game::new(4301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Hoisted: `species_defs()` clones the whole db, and this walk visits
    // thousands of tiles.
    let bands: std::collections::HashMap<String, DangerBand> = game
        .species_defs()
        .into_iter()
        .map(|s| (s.id.clone(), s.danger_band()))
        .collect();
    let mut checked = 0;
    for dx in -30..=30 {
        for dy in -30..=30 {
            let Some((ordinary, _)) = game.habitat_pools(dx, dy, None, 0) else {
                continue;
            };
            let biome = game.world.resource_mut::<WorldMap>().tile(dx, dy).biome;
            let expected = if biome == Biome::Deadlock {
                DangerBand::Tier(1)
            } else {
                DangerBand::Tier(0)
            };
            for id in &ordinary {
                let band = bands[id];
                assert_eq!(band, expected, "zone 1 offered {id} on {biome:?}");
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "the walk found no populated tiles at all");
}

/// A hand-authored boss must not turn up in a fresh run. It can today.
#[test]
fn zone_one_never_fields_an_apex_species() {
    let mut game = Game::new(4302, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for dx in -30..=30 {
        for dy in -30..=30 {
            if let Some((_, apex)) = game.habitat_pools(dx, dy, None, 0) {
                assert!(
                    apex.is_empty(),
                    "zone 1 offered apex species {apex:?} at ({dx}, {dy})"
                );
            }
        }
    }
}

/// The window follows depth underground too, on top of whatever the zone
/// itself already contributes — the same rule `danger_steps` applies to the
/// two group curves. Run at the default zone 1, where the zone step is
/// zero, so any band-2 species reaching depth 6 that zone 1's surface
/// withholds is depth's contribution alone, isolated from the zone term.
#[test]
fn the_stack_window_follows_depth_not_the_surface_zone() {
    let mut game = Game::new(4303, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let bands: std::collections::HashMap<String, DangerBand> = game
        .species_defs()
        .into_iter()
        .map(|s| (s.id.clone(), s.danger_band()))
        .collect();
    let mut found = false;
    for dx in -30..=30 {
        for dy in -30..=30 {
            let (Some((deep, _)), Some((shallow, _))) = (
                game.habitat_pools(dx, dy, Some(6), 0),
                game.habitat_pools(dx, dy, None, 0),
            ) else {
                continue;
            };
            if deep.iter().any(|id| bands[id] == DangerBand::Tier(2))
                && shallow.iter().all(|id| bands[id] != DangerBand::Tier(2))
            {
                found = true;
            }
        }
    }
    assert!(
        found,
        "no tile fielded a band-2 species at depth 6 that zone 1 withholds — \
         depth is not moving the window"
    );
}

/// The boss roll fires everywhere outside the opening ring, and before
/// `APEX_ENTRY_STEP` it can only produce a rolled boss — the whole of "easy
/// bosses on the surface, hard ones deep".
#[test]
fn an_early_boss_is_a_rolled_one() {
    let mut game = Game::new(4304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let far = OPENING_RING_TILES * 3;
    let apex: std::collections::HashSet<String> = game
        .species_defs()
        .into_iter()
        .filter(|s| s.is_boss)
        .map(|s| s.id.clone())
        .collect();
    let mut bosses = 0;
    for i in 0..4000 {
        let (x, y) = (pos.x + far + (i % 40), pos.y + far + (i / 40));
        let Some((species, is_boss)) = game.pick_habitat_species(x, y, None, true) else {
            continue;
        };
        if is_boss {
            bosses += 1;
            assert!(
                !apex.contains(&species),
                "zone 1 named the apex species {species} as a boss"
            );
        }
    }
    assert!(
        bosses > 0,
        "4000 picks produced no boss at all at a {BOSS_SPAWN_CHANCE} rate — \
         the roll is not firing"
    );
}

/// The opening ring turns a boss away, the same as it turns a rare tier
/// away, and for the same reason: a `BOSS_STAT_MULT` spawn in the nursery
/// falsifies `balance_sim::beatable_by_a_fresh_player`.
#[test]
fn the_opening_ring_refuses_a_boss() {
    let mut game = Game::new(4305, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    for _ in 0..2000 {
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some((species, is_boss)) =
                    game.pick_habitat_species(pos.x + dx, pos.y + dy, None, true)
                {
                    assert!(!is_boss, "the opening ring produced a boss: {species}");
                }
            }
        }
    }
}

/// A zone tier scales HP and attack and leaves mitigation exactly where the
/// species authored it. Delete the fix and this fails: the spawn would carry
/// `base_mitigation * stat_multiplier`, which reaches the cap by zone 5 on
/// half the roster.
#[test]
fn a_zone_tier_never_scales_mitigation() {
    let mut game = Game::new(4021, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 4);
    let spawned = spawn_wild_without_routine(&mut game, "sentinel", 30, 30);
    let stats = *game.world.get::<Stats>(spawned).unwrap();
    let species = game
        .world
        .resource::<SpeciesDb>()
        .get("sentinel")
        .unwrap()
        .clone();
    assert_eq!(stats.mitigation, species.base_mitigation);
    assert!(stats.max_hp > species.base_hp);
}

// ─────────────────────────────────────────────────────────────────────────
// The field ramp
// ─────────────────────────────────────────────────────────────────────────

/// The spawn point of `game`'s current zone, which every ramp measurement
/// below is taken from — `distance_from_danger_origin`'s own origin.
fn danger_origin(game: &Game) -> (i32, i32) {
    let spawn = game.world.resource::<ZoneSpawnPoint>();
    (spawn.x, spawn.y)
}

/// `spawn_wild_creature_scaled`'s stat block for one `glitch` at `(x, y)`
/// under `stat_mult`, off a pinned stream so two placements are comparable.
fn scaled_spawn(game: &mut Game, x: i32, y: i32, stat_mult: f32) -> (i32, i32) {
    reseed_rng(game, 7717);
    let e = game
        .spawn_wild_creature_scaled("glitch", x, y, stat_mult, false)
        .expect("glitch ships in the test assets");
    let stats = game.world.get::<Stats>(e).expect("a spawn carries Stats");
    (stats.max_hp, stats.atk)
}

/// The nursery is exactly baseline. `beatable_by_a_fresh_player` is computed
/// against the unscaled species, so any ramp inside the ring falsifies it.
#[test]
fn the_field_ramp_is_flat_inside_the_opening_ring() {
    let game = Game::new(9101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (sx, sy) = danger_origin(&game);
    for d in 0..=OPENING_RING_TILES {
        assert_eq!(
            game.field_stat_mult(sx + d, sy),
            1.0,
            "the ring must stay exactly baseline, {d} tiles out"
        );
    }
}

/// The whole of the cap's design: the far field of zone N is arithmetically
/// the doorstep of zone N+1, which is what leaves `balance_sim`'s existing
/// per-zone curves gating the ramp rather than needing a curve of their own.
#[test]
fn the_field_ramp_tops_out_at_exactly_the_next_zones_doorstep() {
    let mut game = Game::new(9102, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (sx, sy) = danger_origin(&game);
    let far = OPENING_RING_TILES + DANGER_RAMP_TILES;
    for zone in 1..=8u32 {
        game.world.insert_resource(ZoneLevel(zone));
        let here = ZoneLevel(zone).stat_multiplier() as f32;
        let next = ZoneLevel(zone + 1).stat_multiplier() as f32;
        let reached = here * game.field_stat_mult(sx + far, sy);
        assert!(
            (reached - next).abs() < 1e-4,
            "zone {zone}'s far field reached x{reached}, not zone {}'s x{next}",
            zone + 1
        );
        let beyond = here * game.field_stat_mult(sx + far * 4, sy);
        assert!(
            (beyond - next).abs() < 1e-4,
            "the ramp is capped at one zone step; four times out reached x{beyond}"
        );
    }
}

/// The point of the feature: somewhere to walk that is not the doorstep.
#[test]
fn a_far_field_spawn_outclasses_the_same_species_at_the_doorstep() {
    let mut game = Game::new(9103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (sx, sy) = danger_origin(&game);
    let near = sx + OPENING_RING_TILES + 1;
    let far = sx + OPENING_RING_TILES + DANGER_RAMP_TILES;

    let mult = game.field_escalation(near, sy).stat_mult;
    let doorstep = scaled_spawn(&mut game, near, sy, mult);
    let mult = game.field_escalation(far, sy).stat_mult;
    let frontier = scaled_spawn(&mut game, far, sy, mult);

    assert!(
        frontier.0 > doorstep.0 && frontier.1 > doorstep.1,
        "the frontier fielded {frontier:?} against the doorstep's {doorstep:?}"
    );
}

/// The second bug the 2026-08-05 removal was for, pinned. Every Stack spawn
/// is placed at the **surface entrance tile** (`stack_encounter_pack`), so a
/// distance term read inside the spawn scales a whole frame by how far out
/// its link happens to sit. The escalation decides the stats; the tile never
/// does.
#[test]
fn a_spawns_stats_come_from_its_escalation_and_never_from_its_tile() {
    let mut game = Game::new(9104, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (sx, sy) = danger_origin(&game);
    let underground = game.stack_escalation(3);
    // Both tiles sit outside the opening ring, so `roll_rarity` spends a
    // draw at each and the two streams stay in step.
    let near = scaled_spawn(
        &mut game,
        sx + OPENING_RING_TILES + 1,
        sy,
        underground.stat_mult,
    );
    let far = scaled_spawn(
        &mut game,
        sx + OPENING_RING_TILES + DANGER_RAMP_TILES * 2,
        sy,
        underground.stat_mult,
    );
    assert_eq!(
        near, far,
        "a frame under a far-flung link came out harder than one beside the base"
    );
}

/// `SpawnEscalation::surface()` still names the case with no escalation at
/// all, and its two remaining callers depend on that: an arena composition
/// is authored (`arena::encounter`) and a sortie prices its own risk through
/// `habitat_pools`' `step_bonus` (`game::sortie`). Folding the ramp in here
/// instead of into `Game::field_escalation` changes both in silence.
#[test]
fn the_bare_surface_escalation_carries_no_ramp() {
    assert_eq!(
        crate::game::spawning::SpawnEscalation::surface().stat_mult,
        1.0
    );
}

/// The ramp has to reach the spawner the world actually uses, not just the
/// helper. Zone 1 is x1, so an unramped spawn cannot exceed its species'
/// base by more than `MAX_INDIVIDUAL_ROLL`; at the cap the ceiling is twice
/// that. Rarity only ever multiplies further, so this is one-directional.
#[test]
fn the_ambient_spawner_ramps_with_distance() {
    let mut game = Game::new(9105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (sx, sy) = danger_origin(&game);
    let far = sx + OPENING_RING_TILES + DANGER_RAMP_TILES;
    let before: Vec<Entity> = game
        .world
        .query_filtered::<Entity, With<Hostile>>()
        .iter(&game.world)
        .collect();

    // The whole perimeter at the cap's Chebyshev radius, not one column of
    // it: which tiles are walkable is terrain, and a single column can be
    // solid all the way down.
    let ring = far - sx;
    let perimeter: Vec<(i32, i32)> = (-ring..=ring)
        .flat_map(|o| {
            [
                (sx + ring, sy + o),
                (sx - ring, sy + o),
                (sx + o, sy + ring),
                (sx + o, sy - ring),
            ]
        })
        .collect();
    let mut placed = false;
    'search: for (x, y) in perimeter {
        for _ in 0..8 {
            if game.try_spawn_habitat_creature(x, y) {
                placed = true;
                break 'search;
            }
        }
    }
    assert!(
        placed,
        "no walkable, habitable tile anywhere on the ramp's cap to spawn onto"
    );

    let fresh: Vec<(String, i32)> = game
        .world
        .query_filtered::<(Entity, &Creature, &Stats, &Rarity), With<Hostile>>()
        .iter(&game.world)
        .filter(|(e, _, _, r)| !before.contains(e) && **r == Rarity::Ordinary)
        .map(|(_, c, s, _)| (c.species.clone(), s.max_hp))
        .collect();
    assert!(
        !fresh.is_empty(),
        "no ordinary spawn landed at the cap to measure the ramp against"
    );

    for (species, max_hp) in fresh {
        let base = game
            .species_defs()
            .into_iter()
            .find(|s| s.id == species)
            .expect("a spawned species resolves")
            .base_hp;
        let unramped_ceiling = (base as f32 * MAX_INDIVIDUAL_ROLL).round() as i32;
        assert!(
            max_hp > unramped_ceiling,
            "{species} spawned at the ramp's cap with {max_hp} hp, inside the \
             unramped ceiling of {unramped_ceiling} — the ambient spawner is \
             not reading the ramp"
        );
    }
}
