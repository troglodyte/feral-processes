//! The read-only views the renderer draws from, plus symlink targeting.

use super::support::*;
use crate::game::inspection::difficulty_color;
use crate::*;

#[test]
fn inspect_reports_species_detail_without_starting_a_battle() {
    let mut game = Game::new(3, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
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
                hp: species.base_hp,
                max_hp: species.base_hp,
                atk: species.base_atk,
                def: species.base_def,
            },
        ))
        .id();

    let view = game
        .inspect(wild)
        .expect("wild creature should be inspectable");
    assert_eq!(view.name, species.name);
    assert!(view.is_hostile);
    assert!(!view.is_tamed);
    assert_eq!(view.max_hp, species.base_hp);
    let chance = view
        .decompile_chance
        .expect("the starting kit includes a taming catalyst");
    assert!((0.0..=1.0).contains(&chance));
    assert!(
        !game.has_active_battle(),
        "inspecting must not trigger an intrusion"
    );
}

#[test]
fn inspect_returns_none_for_non_creature_entities() {
    let game = Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    assert!(game.inspect(player).is_none());
}

#[test]
fn use_symlink_teleports_the_player_to_the_structure_and_charges_the_cost() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.teleport_cost.is_some())
        .expect("a symlink-capable structure (Home) should exist");
    let cost = def.teleport_cost.clone().unwrap();

    let home = game
        .world
        .spawn((
            Structure {
                kind: def.id.clone(),
            },
            Position { x: 50, y: 50 },
            Glyph {
                ch: def.glyph,
                color: def.color,
            },
        ))
        .id();

    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        for (item, qty) in &cost {
            inv.add(item.clone(), *qty);
        }
    }
    let before: Vec<u32> = cost
        .iter()
        .map(|(item, _)| game.world.get::<Inventory>(player).unwrap().count(item))
        .collect();

    let targets = game.symlink_targets();
    assert!(
        targets.iter().any(|t| t.entity == home),
        "Home should be a symlink target"
    );

    game.use_symlink(home).unwrap();

    let pos = *game.world.get::<Position>(player).unwrap();
    assert_eq!(
        pos,
        Position { x: 50, y: 50 },
        "symlink should teleport the player onto the structure"
    );
    for ((item, qty), before) in cost.iter().zip(before) {
        let after = game.world.get::<Inventory>(player).unwrap().count(item);
        assert_eq!(
            after,
            before - qty,
            "the teleport cost should be fully consumed"
        );
    }
}

#[test]
fn use_symlink_fails_without_enough_of_the_cost() {
    let mut game = Game::new(8, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.teleport_cost.is_some())
        .expect("a symlink-capable structure (Home) should exist");

    let home = game
        .world
        .spawn((
            Structure {
                kind: def.id.clone(),
            },
            Position { x: 20, y: 20 },
            Glyph {
                ch: def.glyph,
                color: def.color,
            },
        ))
        .id();

    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.items.clear();
    }

    let before_pos = *game.world.get::<Position>(player).unwrap();
    assert!(game.use_symlink(home).is_err());
    let after_pos = *game.world.get::<Position>(player).unwrap();
    assert_eq!(
        before_pos, after_pos,
        "a failed symlink shouldn't move the player"
    );
}

#[test]
fn find_creature_in_direction_finds_the_nearest_match_along_the_line() {
    let mut game = Game::new(14, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let start = *game.world.get::<Position>(player).unwrap();
    let species = game.species_defs().into_iter().next().unwrap();
    clear_creatures_east_of_player(&mut game, start, 10);

    assert!(game.find_creature_in_direction(1, 0, 10).is_none());

    let far = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Position {
                x: start.x + 5,
                y: start.y,
            },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                def: 1,
            },
        ))
        .id();
    let near = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Position {
                x: start.x + 2,
                y: start.y,
            },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                def: 1,
            },
        ))
        .id();

    let found = game.find_creature_in_direction(1, 0, 10);
    assert_eq!(
        found,
        Some(near),
        "the nearer creature along the ray should win"
    );
    assert_ne!(found, Some(far));
}

#[test]
fn find_creature_in_direction_respects_max_range() {
    let mut game = Game::new(15, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let start = *game.world.get::<Position>(player).unwrap();
    let species = game.species_defs().into_iter().next().unwrap();
    clear_creatures_east_of_player(&mut game, start, 10);
    game.world.spawn((
        Creature {
            species: species.id.clone(),
        },
        Position {
            x: start.x + 10,
            y: start.y,
        },
        Stats {
            hp: 1,
            max_hp: 1,
            atk: 1,
            def: 1,
        },
    ));

    assert!(
        game.find_creature_in_direction(1, 0, 5).is_none(),
        "creature is out of range"
    );
    assert!(
        game.find_creature_in_direction(1, 0, 10).is_some(),
        "creature should be within range"
    );
}

#[test]
fn find_creature_in_direction_matches_a_90_degree_cone_not_just_the_exact_row() {
    let mut game = Game::new(17, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let start = *game.world.get::<Position>(player).unwrap();
    let species = game.species_defs().into_iter().next().unwrap();
    clear_creatures_east_of_player(&mut game, start, 10);

    // Leans east more than north/south (ddx=4 >= |ddy|=3) — inside the cone.
    let diagonal_ish = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Position {
                x: start.x + 4,
                y: start.y - 3,
            },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                def: 1,
            },
        ))
        .id();
    assert_eq!(
        game.find_creature_in_direction(1, 0, 10),
        Some(diagonal_ish)
    );

    // Leans north more than east (ddy=-8, ddx=2) — outside the eastward cone.
    game.world.spawn((
        Creature {
            species: species.id.clone(),
        },
        Position {
            x: start.x + 2,
            y: start.y - 8,
        },
        Stats {
            hp: 1,
            max_hp: 1,
            atk: 1,
            def: 1,
        },
    ));
    assert_eq!(
        game.find_creature_in_direction(1, 0, 10),
        Some(diagonal_ish),
        "a creature that leans mostly north shouldn't win the eastward search"
    );
}

#[test]
fn difficulty_color_buckets_relative_power_into_con_colors() {
    assert_eq!(
        difficulty_color(50, 100, false),
        GlyphColor::Green,
        "much weaker than the player"
    );
    assert_eq!(
        difficulty_color(100, 100, false),
        GlyphColor::Yellow,
        "an even match"
    );
    assert_eq!(
        difficulty_color(140, 100, false),
        GlyphColor::Orange,
        "notably tougher"
    );
    assert_eq!(
        difficulty_color(200, 100, false),
        GlyphColor::Red,
        "far stronger than the player"
    );
}

#[test]
fn difficulty_color_is_always_magenta_for_a_boss_regardless_of_power() {
    assert_eq!(difficulty_color(1, 1000, true), GlyphColor::Magenta);
    assert_eq!(difficulty_color(1000, 1, true), GlyphColor::Magenta);
}

#[test]
fn difficulty_color_never_divides_by_zero_player_power() {
    assert_eq!(difficulty_color(10, 0, false), GlyphColor::Red);
}

#[test]
fn boss_creatures_are_flagged_in_entity_and_inspect_views() {
    let mut game = Game::new(52, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let boss = game
        .species_defs()
        .into_iter()
        .find(|s| s.is_boss)
        .expect("at least one boss species should exist in assets/species for this test");
    let normal = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss)
        .expect("at least one non-boss species should exist");

    // Clear the world's own initial habitat population so the only
    // hostiles in view are the two this test spawns itself below —
    // otherwise a stray boss (or non-boss) from that initial spawn
    // roll could land within view range and make the assertions below
    // fragile to unrelated changes in spawn odds/roll counts.
    let initial_hostiles: Vec<Entity> = {
        let mut query = game.world.query_filtered::<Entity, With<Hostile>>();
        query.iter(&game.world).collect()
    };
    for e in initial_hostiles {
        game.world.despawn(e);
    }

    let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let boss_entity = game
        .world
        .spawn((
            Creature {
                species: boss.id.clone(),
            },
            Hostile,
            Position {
                x: player_pos.x + 1,
                y: player_pos.y,
            },
            Glyph {
                ch: boss.glyph,
                color: boss.color,
            },
            Stats {
                hp: boss.base_hp,
                max_hp: boss.base_hp,
                atk: boss.base_atk,
                def: boss.base_def,
            },
        ))
        .id();
    game.world.spawn((
        Creature {
            species: normal.id.clone(),
        },
        Hostile,
        Position {
            x: player_pos.x - 1,
            y: player_pos.y,
        },
        Glyph {
            ch: normal.glyph,
            color: normal.color,
        },
        Stats {
            hp: normal.base_hp,
            max_hp: normal.base_hp,
            atk: normal.base_atk,
            def: normal.base_def,
        },
    ));

    let views = game.view_entities(5, 5);
    let boss_view = views.iter().find(|v| v.entity == boss_entity).unwrap();
    assert!(
        boss_view.is_boss,
        "the boss creature's EntityView should be flagged is_boss"
    );
    let normal_views: Vec<_> = views
        .iter()
        .filter(|v| v.entity != boss_entity && v.is_hostile)
        .collect();
    assert!(
        normal_views.iter().all(|v| !v.is_boss),
        "non-boss creatures shouldn't be flagged is_boss"
    );

    assert!(
        game.inspect(boss_entity).unwrap().is_boss,
        "InspectView should also flag a boss creature"
    );
}

#[test]
fn view_entities_colors_hostiles_by_difficulty_and_leaves_others_alone() {
    let mut game = Game::new(53, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let player_pos = *game.world.get::<Position>(player).unwrap();
    game.world.get_mut::<Stats>(player).unwrap().atk = 0;
    game.world.get_mut::<Stats>(player).unwrap().def = 0;
    game.world.get_mut::<Stats>(player).unwrap().max_hp = 100;
    game.world.get_mut::<Stats>(player).unwrap().hp = 100;
    // Player power is now 100. An easy hostile is well under that; a
    // hard one is well over it.
    let easy = game
        .world
        .spawn((
            Creature {
                species: "does_not_matter".to_string(),
            },
            Hostile,
            Position {
                x: player_pos.x + 1,
                y: player_pos.y,
            },
            Glyph {
                ch: 'e',
                color: GlyphColor::Cyan,
            },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 0,
                def: 0,
            },
        ))
        .id();
    let hard = game
        .world
        .spawn((
            Creature {
                species: "does_not_matter".to_string(),
            },
            Hostile,
            Position {
                x: player_pos.x - 1,
                y: player_pos.y,
            },
            Glyph {
                ch: 'h',
                color: GlyphColor::Cyan,
            },
            Stats {
                hp: 300,
                max_hp: 300,
                atk: 0,
                def: 0,
            },
        ))
        .id();
    let tamed_worker = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(tamed_worker).insert(Position {
        x: player_pos.x,
        y: player_pos.y + 1,
    });
    game.world.entity_mut(tamed_worker).insert(Glyph {
        ch: 't',
        color: GlyphColor::Cyan,
    });

    let views = game.view_entities(5, 5);
    let easy_view = views.iter().find(|v| v.entity == easy).unwrap();
    let hard_view = views.iter().find(|v| v.entity == hard).unwrap();
    let tamed_view = views.iter().find(|v| v.entity == tamed_worker).unwrap();

    assert_eq!(
        easy_view.color,
        GlyphColor::Green,
        "a much weaker hostile should read Green"
    );
    assert_eq!(
        hard_view.color,
        GlyphColor::Red,
        "a much stronger hostile should read Red"
    );
    assert_eq!(
        tamed_view.color,
        GlyphColor::Cyan,
        "a non-hostile entity should keep its own glyph color, not be difficulty-colored"
    );
}
