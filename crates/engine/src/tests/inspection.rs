//! The read-only views the renderer draws from, plus symlink targeting.

use super::support::*;
use crate::abilities::AffinityKind;
use crate::game::inspection::difficulty_color;
use crate::tuning::MAX_FUSIONS;
use crate::*;

#[test]
fn a_creatures_display_label_is_tagged_with_its_spawn_zone() {
    let mut game = Game::new(50, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game.species_defs().into_iter().next().unwrap();

    let zone1 = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 3, y: 3 },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                def: 1,
            },
            ZonePortal(1),
        ))
        .id();
    let zone2 = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 4, y: 4 },
            Stats {
                hp: 2,
                max_hp: 2,
                atk: 2,
                def: 2,
            },
            ZonePortal(2),
        ))
        .id();

    assert_eq!(game.entity_label(zone1), format!("{} 1", species.name));
    assert_eq!(game.entity_label(zone2), format!("{} 2", species.name));
    assert_eq!(
        game.manifest(zone2).unwrap().name,
        format!("{} 2", species.name)
    );
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
fn find_target_in_direction_finds_the_nearest_match_along_the_line() {
    let mut game = Game::new(14, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let start = *game.world.get::<Position>(player).unwrap();
    let species = game.species_defs().into_iter().next().unwrap();
    clear_creatures_east_of_player(&mut game, start, 10);

    assert!(game.find_target_in_direction(1, 0, 10).is_none());

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

    let found = game.find_target_in_direction(1, 0, 10);
    assert_eq!(
        found,
        Some(InspectTarget::Creature(near)),
        "the nearer creature along the ray should win"
    );
    assert_ne!(found, Some(InspectTarget::Creature(far)));
}

#[test]
fn find_target_in_direction_respects_max_range() {
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
        game.find_target_in_direction(1, 0, 5).is_none(),
        "creature is out of range"
    );
    assert!(
        game.find_target_in_direction(1, 0, 10).is_some(),
        "creature should be within range"
    );
}

#[test]
fn find_target_in_direction_matches_a_90_degree_cone_not_just_the_exact_row() {
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
        game.find_target_in_direction(1, 0, 10),
        Some(InspectTarget::Creature(diagonal_ish))
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
        game.find_target_in_direction(1, 0, 10),
        Some(InspectTarget::Creature(diagonal_ish)),
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

/// A rare tier is drawn as a bar along the top of the tile, *not* by
/// recolouring the glyph, because the glyph is already carrying
/// `difficulty_color` — how badly this thing would beat you, which is the
/// one reading a player cannot afford to lose. Two channels, and this is
/// what stops a later tidy-up collapsing them into one recolour.
#[test]
fn a_shiny_hostile_still_reports_its_difficulty_colour() {
    let mut game = Game::new(9030, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let wild = game
        .spawn_wild_creature("scrapper", pos.x + 1, pos.y)
        .expect("scrapper ships with the game");

    game.world.entity_mut(wild).insert(Rarity::Gold);

    // Compared against the *computed* difficulty colour rather than against
    // the same creature's pre-tier view: a before/after comparison passes
    // whenever a wrong override happens to land on the colour the fight was
    // going to be anyway, and an even matchup draws Yellow — which is
    // exactly what a first draft of this test collided with.
    let power = game.world.get::<Stats>(wild).unwrap().power();
    let expected = difficulty_color(power, game.player_status().power, false);
    let shiny = game
        .view_entities(5, 5)
        .into_iter()
        .find(|v| v.entity == wild)
        .expect("the wild program should be in view");

    assert_eq!(
        shiny.color, expected,
        "the tier must not touch the glyph colour — that channel is the \
         difficulty read"
    );
    assert_eq!(
        shiny.rarity,
        Rarity::Gold,
        "the tier rides its own field for the map bar to draw"
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

    let ManifestSubject::Program(boss) = game.manifest(boss_entity).unwrap().subject else {
        panic!("a creature is a Program subject");
    };
    assert!(
        boss.is_boss,
        "the manifest should also flag a boss creature"
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

#[test]
fn manifest_reports_the_player_with_equipment_folded_into_their_stats() {
    let game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let status = game.player_status();

    let view = game.manifest(player).expect("the player has a manifest");
    assert_eq!(view.name, "You");
    assert_eq!(view.hp, status.hp);
    assert_eq!(view.max_hp, status.max_hp);
    assert_eq!(
        (view.atk, view.def, view.power),
        (status.atk, status.def, status.power),
        "the manifest must quote the same effective stats the sidebar does"
    );
    assert_eq!(view.level, Some(status.level));
    assert_eq!(view.xp, Some((status.xp, status.xp_to_next)));

    let ManifestSubject::Player(p) = view.subject else {
        panic!("the player is a Player subject");
    };
    assert_eq!(p.hunger, status.hunger);
    assert_eq!(p.fatigue, status.fatigue);
    assert_eq!(p.decompiler, status.decompiler);
    assert_eq!(p.zone, status.zone);
    assert_eq!(p.position, status.position);
    assert_eq!(p.pet_capacity, status.pet_capacity);
}

#[test]
fn manifest_lists_every_equipped_item_with_the_bonus_it_is_actually_granting() {
    let mut game = Game::new(12, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let equippable = game
        .item_defs()
        .into_iter()
        .find(|d| d.equipment.is_some())
        .expect("the shipped item set has equippable gear");
    let item = equippable.id.clone();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(item.clone(), 1);
    game.equip(&item, 0).expect("equipping a held item works");

    let view = game.manifest(player).unwrap();
    let ManifestSubject::Player(p) = view.subject else {
        panic!("the player is a Player subject");
    };
    let slot = p
        .equipment
        .iter()
        .find(|s| s.item_name == equippable.name)
        .expect("the item just equipped is listed");
    let (_, base) = game.equipment_of(&item).unwrap();
    let expected = base
        .scaled_for_level(slot.gear_level)
        .fused_for_tier(slot.fusion_tier);
    assert_eq!(
        (slot.atk, slot.def, slot.decompiler),
        (expected.atk, expected.def, expected.decompiler),
        "the listed bonus must be the one captured at equip time, not a fresh preview"
    );
}

#[test]
fn manifest_reports_a_tamed_program_with_all_four_potential_rolls() {
    let mut game = Game::new(13, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 20, 5);
    game.world.entity_mut(pet).insert(Potential {
        hp_roll: 1.10,
        atk_roll: 1.05,
        def_roll: 0.95,
        growth_roll: 1.15,
    });

    let view = game.manifest(pet).expect("a tamed program has a manifest");
    assert_eq!(view.max_hp, 20);
    assert!(
        !view.routines.is_empty(),
        "spawn_tamed installs the species' innate routines"
    );

    let ManifestSubject::Program(p) = view.subject else {
        panic!("a creature is a Program subject");
    };
    assert!(p.is_tamed);
    assert!(!p.is_hostile);
    assert_eq!(p.max_fusions, MAX_FUSIONS);
    assert_eq!(
        p.activity.as_deref(),
        Some("idle"),
        "an owned program always reports what it is doing"
    );
    let rolls = p.potential.expect("the rolls were just inserted");
    assert_eq!(
        (
            rolls.hp_roll,
            rolls.atk_roll,
            rolls.def_roll,
            rolls.growth_roll
        ),
        (1.10, 1.05, 0.95, 1.15),
        "every roll is surfaced individually, not just the aggregate tier"
    );
    assert!(!rolls.label.is_empty());
}

#[test]
fn manifest_of_a_wild_program_has_no_experience_and_no_activity() {
    let mut game = Game::new(14, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);

    let view = game.manifest(wild).expect("a wild program has a manifest");
    assert_eq!(
        view.xp, None,
        "a wild program carries no Experience until it is compiled"
    );
    let ManifestSubject::Program(p) = view.subject else {
        panic!("a creature is a Program subject");
    };
    assert!(p.is_hostile);
    assert!(!p.is_tamed);
    assert_eq!(
        p.activity, None,
        "a program you don't own isn't doing a job"
    );
    assert!(
        p.decompile_chance.is_some(),
        "the starting kit includes a taming catalyst"
    );
    assert!(!game.has_active_battle(), "a manifest never starts a fight");
}

#[test]
fn manifest_survives_a_creature_that_predates_the_potential_component() {
    let mut game = Game::new(15, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 12, 3);
    game.world.entity_mut(pet).remove::<Potential>();

    let view = game
        .manifest(pet)
        .expect("still inspectable without a roll");
    let ManifestSubject::Program(p) = view.subject else {
        panic!("a creature is a Program subject");
    };
    assert_eq!(p.potential, None);
}

#[test]
fn manifest_returns_none_for_anything_that_is_neither_the_player_nor_a_creature() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structure = game.world.spawn(Position { x: 0, y: 0 }).id();
    assert!(game.manifest(structure).is_none());
}

#[test]
fn the_manifest_lists_only_non_neutral_affinities() {
    const LOPSIDED: &str = r#"(
    id: "test_lopsided",
    name: "Test Lopsided",
    glyph: 'l',
    color: Cyan,
    base_hp: 10,
    base_atk: 4,
    base_def: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    moves: [(name: "Poke", power: 3)],
    affinities: (heal: 1.4, damage: 0.8),
)"#;
    let dir = modded_assets_dir(
        "manifest_affinities",
        &[],
        &[],
        &[("test_lopsided.ron", LOPSIDED)],
        &[],
        &[],
    );
    let mut game = Game::new(94, DifficultyMode::Forgiving, &dir).unwrap();
    let entity = spawn_wild_without_routine(&mut game, "test_lopsided", 3, 3);
    // `Game::manifest` is the public entry; `program_manifest` behind it is
    // private to game::inspection and not reachable from here.
    let ManifestSubject::Program(p) = game.manifest(entity).unwrap().subject else {
        panic!("expected a program manifest");
    };
    assert_eq!(
        p.affinities,
        vec![(AffinityKind::Damage, 0.8), (AffinityKind::Heal, 1.4)],
        "listed in AffinityKind order, and the three neutral ones omitted"
    );
}

/// A structure can carry a cronjob worker *and* a guard at once, and the
/// roster's whole purpose is showing what is on what. `view_entities` cannot
/// answer this — its `structure_worker` comes from a map keyed by the task's
/// target, so two programs on one structure collapse into one.
#[test]
fn structure_report_lists_every_assignee_not_just_one() {
    let mut game = Game::new(700, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, -1, 0);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 12);
    game.place_structure("mining_node", 1, 0).unwrap();
    let node = game
        .structure_report()
        .into_iter()
        .find(|s| s.kind == "mining_node")
        .expect("the node was just deployed")
        .entity;

    let miner = spawn_tamed(&mut game, 10, 3);
    let guard = spawn_tamed(&mut game, 10, 3);
    game.assign_cronjob(miner, node).unwrap();
    game.assign_guard(guard, node).unwrap();

    let report = game.structure_report();
    let node = report
        .iter()
        .find(|s| s.entity == node)
        .expect("the node is still standing");
    assert_eq!(node.assignees.len(), 2, "both programs should be reported");
    let kinds: Vec<TaskKind> = node.assignees.iter().map(|a| a.kind).collect();
    assert!(kinds.contains(&TaskKind::GatherResource));
    assert!(kinds.contains(&TaskKind::Guard));
    assert!(
        node.assignees.iter().all(|a| !a.label.is_empty()),
        "each assignee is named, so the roster can say who is on what"
    );
}

/// Structures cluster within `MAX_BUILD_DISTANCE_FROM_HOME` of the Home, but
/// the player does not — walk far enough and a radius-limited scan would
/// report an empty base.
#[test]
fn structure_report_is_zone_wide_and_not_relative_to_the_player() {
    let mut game = Game::new(701, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, -1, 0);
    let before = game.structure_report().len();
    assert!(before > 0, "the Home should be reported");

    let player = game.player_entity();
    let mut pos = game.world.get_mut::<Position>(player).unwrap();
    pos.x += 500;
    pos.y += 500;

    let report = game.structure_report();
    assert_eq!(
        report.len(),
        before,
        "walking away must not shrink the roster"
    );
    assert!(
        report.iter().all(|s| s.distance > 400),
        "the distance is measured from wherever the player is standing"
    );
}

/// Tier and durability are `Some` only where the def declares them: the Home
/// has no upgrade path and raids can't touch it, a Mining Node has both.
#[test]
fn structure_report_carries_tier_durability_and_whether_the_structure_is_workable() {
    let mut game = Game::new(702, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, -1, 0);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 12);
    game.place_structure("mining_node", 1, 0).unwrap();

    let report = game.structure_report();
    let home = report.iter().find(|s| s.is_home).unwrap();
    assert_eq!(home.tier, None, "Home declares no upgrade path");
    assert_eq!(home.durability, None, "Home is not raidable");
    assert!(!home.workable, "Home has no work recipe");

    let node = report.iter().find(|s| s.kind == "mining_node").unwrap();
    assert_eq!(node.tier, Some(1), "a fresh node sits at tier 1");
    let (hp, max_hp) = node.durability.expect("a node is raidable");
    assert!(hp > 0 && hp == max_hp, "an unraided node is at full health");
    assert!(node.workable, "a node is the workable structure");
    assert!(
        node.assignees.is_empty(),
        "nobody has been assigned, which is what makes it read as idle"
    );
}

/// The Home leads, so the roster opens on the thing the rest of the base is
/// measured from, and identical structures sit together rather than being
/// interleaved by distance.
#[test]
fn structure_report_puts_home_first_and_groups_by_kind() {
    let mut game = Game::new(703, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "armor_bench");
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 60);
    place_home(&mut game, 0, 0);
    game.place_structure("mining_node", 2, 0).unwrap();
    game.place_structure("armory", 1, 0).unwrap();
    game.place_structure("mining_node", 3, 0).unwrap();

    let kinds: Vec<String> = game
        .structure_report()
        .into_iter()
        .map(|s| s.kind.to_string())
        .collect();
    assert_eq!(kinds[0], "home", "the Home leads the roster");
    let mining: Vec<usize> = kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| k.as_str() == "mining_node")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(mining.len(), 2);
    assert_eq!(
        mining[1],
        mining[0] + 1,
        "both nodes should be adjacent rows, not split by the armory"
    );
}

/// The inventory view groups by category so a player scanning for gear
/// isn't reading a list interleaved with salvage. Sorted here, in the view,
/// rather than in `Inventory` — that component's order is persisted through
/// `PlayerSave`, and pickup order is not the renderer's business to rewrite.
#[test]
fn the_inventory_view_comes_back_grouped_by_category() {
    let mut game = Game::new(50, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        // Added in deliberately scrambled order: material, weapon,
        // consumable, armor.
        inv.add(ItemId::from(ids::CORE_FRAGMENT), 5);
        inv.add(ItemId::from(ids::MONOFILAMENT_WHIP), 1);
        inv.add(ItemId::from(ids::POWER_CELL), 2);
        inv.add(ItemId::from(ids::FIREWALL_PLATING), 1);
    }

    let inventory = game.player_status().inventory;
    let categories: Vec<ItemCategory> = inventory
        .iter()
        .map(|r| game.item_category(&r.item))
        .collect();
    let mut sorted = categories.clone();
    sorted.sort();
    assert_eq!(
        categories, sorted,
        "rows must arrive grouped: {inventory:?}"
    );

    // And the grouping is the documented one, not merely *some* order.
    let whip = inventory
        .iter()
        .position(|r| r.item.as_str() == ids::MONOFILAMENT_WHIP)
        .unwrap();
    let cell = inventory
        .iter()
        .position(|r| r.item.as_str() == ids::POWER_CELL)
        .unwrap();
    let frag = inventory
        .iter()
        .position(|r| r.item.as_str() == ids::CORE_FRAGMENT)
        .unwrap();
    assert!(cell < whip, "consumables list before weapons");
    assert!(whip < frag, "weapons list before salvage");
}

/// The player cannot play a chain they cannot see. A report row carries what
/// is in both buffers and why the machine is or isn't producing — folded in
/// the engine, because per `CLAUDE.md` a read-only screen's row shaping is
/// app-core's to bound and gui's to draw, never gui's to invent.
#[test]
fn a_structure_report_row_carries_its_stock_and_status() {
    let mut game = Game::new(985, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = deploy_upgradeable_node(&mut game);
    {
        let mut stock = game.world.get_mut::<Stock>(node).unwrap();
        stock.input.insert(ItemId::from(ids::POWER_CELL), 2);
        stock.output.insert(ItemId::from(ids::CORE_FRAGMENT), 6);
    }

    let report = game.structure_report();
    let row = report
        .iter()
        .find(|r| r.entity == node)
        .expect("the deployed node is on the roster");

    assert_eq!(row.output, vec![("Core Fragment".to_string(), 6)]);
    assert_eq!(row.input, vec![("Power Cell".to_string(), 2)]);
    assert_eq!(row.output_capacity, crate::tuning::DEFAULT_OUTPUT_CAPACITY);
    assert_eq!(
        row.status,
        Some(MachineStatus::Running),
        "a work node has a state to be in"
    );
    assert!(row.workable, "and a program can be posted to it");

    let home = find_home(&mut game).unwrap();
    let home_row = game
        .structure_report()
        .into_iter()
        .find(|r| r.entity == home)
        .unwrap();
    assert_eq!(
        home_row.status, None,
        "a Home runs no job, so it has no state to report"
    );
    assert!(home_row.output.is_empty());
}

/// A structure in the cone is a legitimate target now, so pointing at your
/// Refinery with nothing alive between you and it finds the Refinery.
#[test]
fn the_inspector_finds_a_structure_when_no_creature_is_in_the_cone() {
    let mut game = Game::new(1400, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let start = *game.world.get::<Position>(game.player_entity()).unwrap();
    clear_creatures_east_of_player(&mut game, start, 10);

    let refinery = game
        .world
        .spawn((
            Structure {
                kind: "refinery".to_string(),
            },
            Position {
                x: start.x + 3,
                y: start.y,
            },
        ))
        .id();

    assert_eq!(
        game.find_target_in_direction(1, 0, 10),
        Some(InspectTarget::Structure(refinery))
    );
}

/// Nearest wins across *both* kinds, which is the whole reason one walk
/// gathers them rather than two functions the caller picks between: with a
/// creature and a structure both in the cone, neither kind gets priority.
#[test]
fn the_inspector_returns_whichever_of_the_two_kinds_is_nearer() {
    let mut game = Game::new(1401, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let start = *game.world.get::<Position>(game.player_entity()).unwrap();
    let species = game.species_defs().into_iter().next().unwrap();
    clear_creatures_east_of_player(&mut game, start, 10);

    let spawn_creature = |game: &mut Game, dx: i32| {
        game.world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Position {
                    x: start.x + dx,
                    y: start.y,
                },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    def: 1,
                },
            ))
            .id()
    };
    let spawn_structure = |game: &mut Game, dx: i32| {
        game.world
            .spawn((
                Structure {
                    kind: "refinery".to_string(),
                },
                Position {
                    x: start.x + dx,
                    y: start.y,
                },
            ))
            .id()
    };

    let near_structure = spawn_structure(&mut game, 2);
    spawn_creature(&mut game, 6);
    assert_eq!(
        game.find_target_in_direction(1, 0, 10),
        Some(InspectTarget::Structure(near_structure)),
        "the structure is nearer, so the structure wins"
    );

    let nearer_creature = spawn_creature(&mut game, 1);
    assert_eq!(
        game.find_target_in_direction(1, 0, 10),
        Some(InspectTarget::Creature(nearer_creature)),
        "and a creature closer than it takes the target back"
    );
}

/// A creature two tiles east of the player and a structure five tiles east,
/// both inside the default scan cone, with any wild leftovers in that cone
/// cleared first — the shared fixture for every test asserting what the
/// eastward scan finds and does not find. The creature is nearer so a
/// surface scan resolves to it despite the structure also being in the
/// cone. Returns the creature's entity, which is the only one either test
/// needs to assert against by identity.
fn game_with_structure_and_creature_east_of_player(seed: u32) -> (Game, Entity) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let start = *game.world.get::<Position>(game.player_entity()).unwrap();
    let species = game.species_defs().into_iter().next().unwrap();
    clear_creatures_east_of_player(&mut game, start, 10);

    let creature = game
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
    game.world.spawn((
        Structure {
            kind: "refinery".to_string(),
        },
        Position {
            x: start.x + 5,
            y: start.y,
        },
    ));
    (game, creature)
}

/// `Position` is pinned to the surface entrance while the party is in the
/// Stack, so an unguarded scan would report the base four frames overhead as
/// lying off to your east. The whole function refuses underground now, so
/// this no longer stops at "no structure" — nothing is found at all, and the
/// creature this fixture put in the cone is proof the emptiness is the
/// guard's doing, not an accident of an empty cone.
#[test]
fn the_inspector_offers_no_structure_while_the_party_is_underground() {
    let (mut game, _creature) = game_with_structure_and_creature_east_of_player(1402);
    let start = *game.world.get::<Position>(game.player_entity()).unwrap();

    game.enter_stack(start.x, start.y);
    assert!(game.is_underground(), "the fixture really went down");

    assert_eq!(
        game.find_target_in_direction(1, 0, 10),
        None,
        "structure and creature both sit in the cone, but the guard refuses \
         underground regardless of kind"
    );
}

/// The other half of the same defect. `Position` is pinned to the surface
/// entrance tile while the party is in the Stack, so an unguarded creature
/// scan opens a manifest for a program four frames overhead and reports it
/// as lying that way. The test for whether a `Position` reader needs the
/// guard is not "does it act" but "does it claim something about where the
/// party is", and this claims exactly that.
#[test]
fn the_inspector_scans_no_creature_while_the_party_is_underground() {
    let (mut game, creature) = game_with_structure_and_creature_east_of_player(1404);
    let start = *game.world.get::<Position>(game.player_entity()).unwrap();

    assert_eq!(
        game.find_target_in_direction(1, 0, 10),
        Some(InspectTarget::Creature(creature)),
        "the fixture must put the creature where the surface scan finds it"
    );

    game.enter_stack(start.x, start.y);
    assert!(game.is_underground(), "the fixture really went down");

    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        assert_eq!(
            game.find_target_in_direction(dx, dy, 10),
            None,
            "the inspector found something at ({dx}, {dy}) from four frames under it"
        );
    }
}

/// The detail screen and the `B` roster must never disagree about the same
/// machine, which is why one calls the other rather than building its own
/// row (see `Game::structure_manifest`).
#[test]
fn a_structure_manifest_is_the_same_row_the_roster_shows() {
    let mut game = Game::new(1403, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let start = *game.world.get::<Position>(game.player_entity()).unwrap();
    let refinery = game
        .world
        .spawn((
            Structure {
                kind: "refinery".to_string(),
            },
            Position {
                x: start.x + 2,
                y: start.y,
            },
        ))
        .id();

    let from_roster = game
        .structure_report()
        .into_iter()
        .find(|r| r.entity == refinery)
        .expect("the roster lists it");
    let from_manifest = game
        .structure_manifest(refinery)
        .expect("and so does the inspector");

    assert_eq!(from_manifest.kind, from_roster.kind);
    assert_eq!(from_manifest.label, from_roster.label);
    assert_eq!(from_manifest.pos, from_roster.pos);
    assert_eq!(from_manifest.tier, from_roster.tier);
    assert_eq!(from_manifest.workable, from_roster.workable);
    assert_eq!(from_manifest.output_capacity, from_roster.output_capacity);
}

/// An entity that is not a structure has no row, rather than an empty one.
#[test]
fn a_structure_manifest_for_something_that_is_not_a_structure_is_none() {
    let mut game = Game::new(1404, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    assert!(game.structure_manifest(player).is_none());
}

/// A posted worker is drawn only while it is *away* from its machine —
/// walking in to take the job, carrying a load to a depot, or coming back.
/// At its post it sits under the machine's own glyph, so the base reads as
/// buildings at rest and motion is the only thing that draws the eye.
///
/// Nothing else tamed is ever drawn. Nothing walks a guard to its post, and
/// an idle program and a party member are never moved at all, so each keeps
/// whatever tile it was standing on when it took the job.
#[test]
fn a_worker_is_only_away_from_its_post_while_it_is_actually_away() {
    let mut game = Game::new(1405, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, -1, 0);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 12);
    game.place_structure("mining_node", 1, 0).unwrap();
    let node = game
        .structure_report()
        .into_iter()
        .find(|s| s.kind == "mining_node")
        .expect("the node was just deployed")
        .entity;

    let worker = spawn_tamed_on_map(&mut game, 6, 6);
    let guard = spawn_tamed_on_map(&mut game, 6, 7);
    let idle = spawn_tamed_on_map(&mut game, 6, 8);
    // Posted from across the base, which is what leaves the worker with a
    // walk to make: `assign_cronjob` starts it from the player's tile.
    stand_player_at(&mut game, 6, 6);
    game.assign_cronjob(worker, node).unwrap();
    game.assign_guard(guard, node).unwrap();

    let away = |game: &mut Game, e: Entity| {
        game.view_entities(40, 40)
            .into_iter()
            .find(|v| v.entity == e)
            .map(|v| v.worker_away_from_post)
    };

    assert_eq!(
        away(&mut game, worker),
        Some(true),
        "spawned across the base and not yet walked in, so it is on its way"
    );
    park_at_post(&mut game, worker, node);
    assert_eq!(
        away(&mut game, worker),
        Some(false),
        "standing at its post, it hides under the machine"
    );

    assert_eq!(away(&mut game, guard), Some(false));
    assert_eq!(away(&mut game, idle), Some(false));
}

/// The mark is on the program when the program is drawn and on the structure
/// when it isn't — one sentence covering a guard, a worker at its post and a
/// worker mid-errand alike. So a machine wears the mark exactly while its
/// program is standing at it.
#[test]
fn a_structure_is_attended_only_while_its_program_is_standing_at_it() {
    let mut game = Game::new(1406, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, -1, 0);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 24);
    game.place_structure("mining_node", 1, 0).unwrap();
    game.place_structure("mining_node", 3, 0).unwrap();
    let nodes: Vec<Entity> = game
        .structure_report()
        .into_iter()
        .filter(|s| s.kind == "mining_node")
        .map(|s| s.entity)
        .collect();
    let (worked, guarded) = (nodes[0], nodes[1]);

    let worker = spawn_tamed_on_map(&mut game, 6, 6);
    let guard = spawn_tamed_on_map(&mut game, 6, 7);
    stand_player_at(&mut game, 6, 6);
    game.assign_cronjob(worker, worked).unwrap();
    game.assign_guard(guard, guarded).unwrap();

    let attended = |game: &mut Game, e: Entity| {
        game.view_entities(40, 40)
            .into_iter()
            .find(|v| v.entity == e)
            .map(|v| v.structure_attended)
    };

    assert_eq!(
        attended(&mut game, guarded),
        Some(true),
        "a guard is never drawn, so its structure carries the mark for it"
    );
    assert_eq!(
        attended(&mut game, worked),
        Some(false),
        "its worker is still walking in, and is wearing the mark itself"
    );

    park_at_post(&mut game, worker, worked);
    assert_eq!(attended(&mut game, worked), Some(true));
}

/// Exactly one mark per posted program at all times: it never doubles while
/// the worker stands on its machine, and never vanishes while it is away.
/// The two flags are the two halves of that, so they are asserted together.
#[test]
fn a_worked_machine_and_its_worker_never_both_wear_the_mark() {
    let mut game = Game::new(1407, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, -1, 0);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 12);
    game.place_structure("mining_node", 1, 0).unwrap();
    let node = game
        .structure_report()
        .into_iter()
        .find(|s| s.kind == "mining_node")
        .expect("the node was just deployed")
        .entity;
    let worker = spawn_tamed_on_map(&mut game, 6, 6);
    game.assign_cronjob(worker, node).unwrap();

    for step in 0..12 {
        let views = game.view_entities(40, 40);
        let machine = views
            .iter()
            .find(|v| v.entity == node)
            .expect("the node is standing")
            .structure_attended;
        let program = views
            .iter()
            .find(|v| v.entity == worker)
            .expect("the worker exists")
            .worker_away_from_post;
        assert!(
            machine != program,
            "step {step}: machine {machine}, worker away {program} — the mark \
             either doubled or went missing"
        );
        game.tick();
    }
}

/// A machine whose output is full while nothing in the base can take a load
/// is at a dead end: its worker will never leave, so the mark that would
/// have walked away stays put. The flag is what lets a frontend say so.
///
/// Deliberately not a sixth `MachineStatus`: that enum is one machine's own
/// state, and "there is nowhere left to put this" is a fact about every
/// depot at once.
#[test]
fn a_full_machine_with_nowhere_to_unload_reads_as_stranded() {
    let mut game = Game::new(1408, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, -1, 0);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 48);
    game.place_structure("mining_node", 1, 0).unwrap();
    let node = game
        .structure_report()
        .into_iter()
        .find(|s| s.kind == "mining_node")
        .expect("the node was just deployed")
        .entity;

    let stranded = |game: &mut Game, e: Entity| {
        game.view_entities(40, 40)
            .into_iter()
            .find(|v| v.entity == e)
            .map(|v| v.output_stranded)
    };

    assert_eq!(
        stranded(&mut game, node),
        Some(false),
        "an empty buffer is not stranded, whatever else is true"
    );

    let mut stock = game.world.get_mut::<Stock>(node).unwrap();
    let capacity = stock.capacity;
    stock
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), capacity);
    assert_eq!(
        stranded(&mut game, node),
        Some(true),
        "full, and no depot has been built at all"
    );

    game.place_structure("depot", 1, 2).unwrap();
    assert_eq!(
        stranded(&mut game, node),
        Some(false),
        "a depot with room is somewhere to put it, so the dead end is over"
    );

    // Fill the depot too: a depot with no room is no better than no depot,
    // which is the case a "has a depot been built" check would miss.
    let depot = game
        .structure_report()
        .into_iter()
        .find(|s| s.kind == "depot")
        .expect("the depot was just deployed")
        .entity;
    let mut stock = game.world.get_mut::<Stock>(depot).unwrap();
    let capacity = stock.capacity;
    stock
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), capacity);
    assert_eq!(stranded(&mut game, node), Some(true));
}

/// The manifest is where a player finds out their companion is behind the
/// zone they are standing in. The tag on its name says "1"; without the
/// player's own zone beside it that number means nothing, which is why the
/// view carries the pair rather than the tier alone.
#[test]
fn the_manifest_shows_a_programs_zone_tier_against_the_players_own() {
    let mut game = Game::new(640, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    game.world
        .entity_mut(pet)
        .insert((ZonePortal(1), Refactors(2)));
    game.world.resource_mut::<crate::resources::ZoneLevel>().0 = 4;

    let ManifestSubject::Program(view) = game.manifest(pet).unwrap().subject else {
        panic!("a creature is a Program subject");
    };

    assert_eq!(
        (view.zone_tier, view.player_zone),
        (1, 4),
        "three doublings behind the ground it is standing on, and the screen has to say so"
    );
    assert_eq!(
        (view.refactors, view.max_refactors),
        (2, crate::tuning::MAX_COMPANION_REFACTORS),
        "and how many upgrade slots are left"
    );
    assert_eq!(
        game.owned_pets()[0].refactors,
        2,
        "the party menu's own row carries the same count"
    );
}
