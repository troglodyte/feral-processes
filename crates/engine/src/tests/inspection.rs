//! The read-only views the renderer draws from.

use super::support::*;
use crate::abilities::AffinityKind;
use crate::game::inspection::difficulty_color;
use crate::species::AffinityClass;
use crate::tuning::MAX_FUSIONS;
use crate::*;

#[test]
fn a_manifest_carries_the_class_whose_base_job_it_names() {
    let mut game = Game::new(3200, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 20, 5);
    game.world.get_mut::<Creature>(program).unwrap().species = "sentinel".to_string();

    assert_eq!(
        program_manifest(&game, program).base_job,
        Some(AffinityClass::Bastion),
        "the screen that says what a program is like to post has to say \
         which of the three base jobs it does"
    );
}

/// A boss is outside the class system, and the manifest is the one screen a
/// player meets one on — `Game::manifest` answers for a hostile too.
#[test]
fn a_boss_manifest_names_no_base_job() {
    let mut game = Game::new(3201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 20, 5);
    game.world.get_mut::<Creature>(program).unwrap().species = "overseer".to_string();

    assert_eq!(program_manifest(&game, program).base_job, None);
}

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
                mitigation: 1,
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
                mitigation: 2,
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
                mitigation: 1,
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
                mitigation: 1,
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
            mitigation: 1,
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

/// Spawns a bare creature at `(dx, dy)` from `start` — enough of one for the
/// inspector, which reads `Position` and `Creature` and nothing else.
fn spawn_marker_creature(game: &mut Game, start: Position, dx: i32, dy: i32) -> Entity {
    let species = game.species_defs().into_iter().next().unwrap();
    game.world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Position {
                x: start.x + dx,
                y: start.y + dy,
            },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                mitigation: 1,
            },
        ))
        .id()
}

fn spawn_marker_structure(game: &mut Game, start: Position, dx: i32, dy: i32) -> Entity {
    game.world
        .spawn((
            Structure {
                kind: "refinery".to_string(),
            },
            Position {
                x: start.x + dx,
                y: start.y + dy,
            },
        ))
        .id()
}

/// The same as `spawn_marker_creature`, but tamed and marked idle base
/// staff. `find_target_in_direction`'s `Structure` query only answers
/// inside base space now (`Structure` is the space tag), so any fixture
/// mixing a structure and a creature on one ray has to stand in base space
/// too — and an *untamed* creature is refused there (base space has no
/// wildlife). `Tamed` plus `BaseStaff` with no `Task` is what
/// `position_is_honest` reads as a legitimate, drawn position, which is
/// what lets this coexist with `spawn_marker_structure` on the same ray.
fn spawn_marker_staffer(game: &mut Game, start: Position, dx: i32, dy: i32) -> Entity {
    let creature = spawn_marker_creature(game, start, dx, dy);
    let owner = game.player_entity();
    game.world.entity_mut(creature).insert(Tamed { owner });
    creature
}

/// `start`, but in base space rather than on the surface: the fixed origin
/// `stand_in_base` puts the party at. Fixtures that mix a `Structure` with
/// a creature must be built around this instead of the player's own
/// `Position`, since a `Structure` only answers `find_target_in_direction`'s
/// ray inside base space.
fn stand_in_base_and_get_origin(game: &mut Game) -> Position {
    stand_in_base(game);
    let (x, y) = game.base_pos().expect("stand_in_base just set the locale");
    Position { x, y }
}

/// The inspector is a ray one tile wide, so a creature that merely *leans*
/// east is not east. This is the direct inversion of a deleted test that
/// asserted `(+4, -3)` — a pure-ish diagonal — was found by an eastward
/// scan: at the 40-tile reach the caller used to pass, that forgiveness
/// meant `x` could name something forty tiles off the row and well outside
/// the map pane.
///
/// The final leg is what makes the two `None`s evidence: it moves a creature
/// onto the row and finds it, so the emptiness above is the ray's doing
/// rather than an accident of a cleared world.
#[test]
fn find_target_in_direction_ignores_a_creature_one_tile_off_the_ray() {
    let mut game = Game::new(17, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let start = *game.world.get::<Position>(game.player_entity()).unwrap();
    clear_creatures_east_of_player(&mut game, start, 10);

    spawn_marker_creature(&mut game, start, 4, -3);
    assert_eq!(
        game.find_target_in_direction(1, 0, 10),
        None,
        "leaning east is not being east"
    );

    let just_off = spawn_marker_creature(&mut game, start, 4, -1);
    assert_eq!(
        game.find_target_in_direction(1, 0, 10),
        None,
        "one tile off the row is off the ray"
    );

    game.world.get_mut::<Position>(just_off).unwrap().y = start.y;
    assert_eq!(
        game.find_target_in_direction(1, 0, 10),
        Some(InspectTarget::Creature(just_off)),
        "and the same creature on the row is found, so the misses were the ray"
    );
}

/// The nearer thing on the ray hides everything behind it, whichever kinds
/// they are. Distinct from `the_inspector_returns_whichever_of_the_two_kinds_
/// is_nearer`, which asserts nearest-wins; this asserts the far one is
/// *never* the answer, which is what makes the ray a line of sight rather
/// than a search.
#[test]
fn a_nearer_thing_on_the_ray_shadows_a_farther_one() {
    let mut game = Game::new(1405, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let start = *game.world.get::<Position>(game.player_entity()).unwrap();
    clear_creatures_east_of_player(&mut game, start, 10);
    // A `Structure` only answers this ray inside base space, so both
    // fixtures below have to stand there — `spawn_marker_staffer`'s own doc
    // says why the creature is tamed staff rather than a wild marker.
    let base = stand_in_base_and_get_origin(&mut game);

    let near_creature = spawn_marker_staffer(&mut game, base, 2, 0);
    let far_structure = spawn_marker_structure(&mut game, base, 5, 0);
    let found = game.find_target_in_direction(1, 0, 10);
    assert_eq!(found, Some(InspectTarget::Creature(near_creature)));
    assert_ne!(
        found,
        Some(InspectTarget::Structure(far_structure)),
        "the structure is behind the creature and must stay hidden"
    );

    // Swap which kind is in front; the shadowing must not care.
    game.world.get_mut::<Position>(near_creature).unwrap().x = base.x + 7;
    let found = game.find_target_in_direction(1, 0, 10);
    assert_eq!(found, Some(InspectTarget::Structure(far_structure)));
    assert_ne!(found, Some(InspectTarget::Creature(near_creature)));
}

/// The bound is inclusive, and one tile past it finds nothing. Sharper than
/// `find_target_in_direction_respects_max_range`, which straddles the bound
/// at 5-vs-10 and so would pass against an off-by-one.
#[test]
fn find_target_in_direction_stops_exactly_at_the_range_bound() {
    let mut game = Game::new(1406, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let start = *game.world.get::<Position>(game.player_entity()).unwrap();
    clear_creatures_east_of_player(&mut game, start, 12);

    let creature = spawn_marker_creature(&mut game, start, 10, 0);
    assert_eq!(
        game.find_target_in_direction(1, 0, 10),
        Some(InspectTarget::Creature(creature)),
        "a creature exactly at the bound is within it"
    );
    assert_eq!(
        game.find_target_in_direction(1, 0, 9),
        None,
        "and one tile past the bound is out of reach"
    );
}

/// Bevy's query iteration order is not stable, so a scan that resolved a tie
/// by "whichever came back first" could name a different thing between runs
/// or after a reload — the trap `assembler_system`'s `(x, y)` sort exists to
/// prevent. `find_target_in_direction` orders candidates by
/// `(step, kind, entity)`, a total order with no first-of-equals for the
/// iteration order to leak through.
///
/// The two worlds spawn the structure and the creature in *opposite* orders
/// on purpose. Spawned the same way round, this would pass on iteration
/// order alone and prove nothing, which is the same reason the assembler's
/// test seeds its competitors backwards.
#[test]
fn two_things_on_one_tile_resolve_the_same_way_however_they_were_spawned() {
    let build = |structure_first: bool| {
        let mut game = Game::new(1407, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let start = *game.world.get::<Position>(game.player_entity()).unwrap();
        clear_creatures_east_of_player(&mut game, start, 10);
        // A `Structure` only answers this ray inside base space, so the
        // creature sharing its tile has to be tamed staff rather than a
        // wild marker — see `spawn_marker_staffer`'s doc for why.
        let base = stand_in_base_and_get_origin(&mut game);
        if structure_first {
            spawn_marker_structure(&mut game, base, 3, 0);
            spawn_marker_staffer(&mut game, base, 3, 0);
        } else {
            spawn_marker_staffer(&mut game, base, 3, 0);
            spawn_marker_structure(&mut game, base, 3, 0);
        }
        game.find_target_in_direction(1, 0, 10)
    };

    let structure_first = build(true);
    let creature_first = build(false);
    assert!(
        matches!(structure_first, Some(InspectTarget::Structure(_))),
        "a tile holding both names the structure, which is the thing the map \
         draws there"
    );
    assert!(
        matches!(creature_first, Some(InspectTarget::Structure(_))),
        "and it does so whichever order they were spawned in"
    );
}

/// The ray runs forward only. Nothing covered straight-behind before — the
/// deleted cone test only established that leaning the *wrong* way lost a
/// contest, never that a thing directly behind you is unreachable.
#[test]
fn find_target_in_direction_never_looks_behind_the_player() {
    let mut game = Game::new(1408, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let start = *game.world.get::<Position>(game.player_entity()).unwrap();
    // Both rays, because this test scans both ways: the westward assertion
    // below is meaningless if the sector put its own program on that row.
    clear_creatures_along_ray(&mut game, start, 1, 0, 10);
    clear_creatures_along_ray(&mut game, start, -1, 0, 10);

    let behind = spawn_marker_creature(&mut game, start, -3, 0);
    assert_eq!(
        game.find_target_in_direction(1, 0, 10),
        None,
        "a creature to the west is not found by an eastward scan"
    );
    assert_eq!(
        game.find_target_in_direction(-1, 0, 10),
        Some(InspectTarget::Creature(behind)),
        "and turning round finds it, so the miss was the direction"
    );
}

#[test]
fn difficulty_color_buckets_relative_power_into_con_colors() {
    assert_eq!(
        difficulty_color(50, 100, false, false),
        GlyphColor::Green,
        "much weaker than the player"
    );
    assert_eq!(
        difficulty_color(100, 100, false, false),
        GlyphColor::Yellow,
        "an even match"
    );
    assert_eq!(
        difficulty_color(140, 100, false, false),
        GlyphColor::Orange,
        "notably tougher"
    );
    assert_eq!(
        difficulty_color(200, 100, false, false),
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
    let expected = difficulty_color(power, game.player_status().strength, false, false);
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
    assert_eq!(difficulty_color(1, 1000, true, false), GlyphColor::Magenta);
    assert_eq!(difficulty_color(1000, 1, true, false), GlyphColor::Magenta);
}

#[test]
fn difficulty_color_never_divides_by_zero_player_power() {
    assert_eq!(difficulty_color(10, 0, false, false), GlyphColor::Red);
}

/// The reserved nemesis colour, requested regardless of power ratio — the
/// same override shape `is_boss` already has, applied to a second, more
/// specific reading.
#[test]
fn difficulty_color_is_always_blue_for_a_nemesis_regardless_of_power() {
    assert_eq!(difficulty_color(1, 1000, false, true), GlyphColor::Blue);
    assert_eq!(difficulty_color(1000, 1, false, true), GlyphColor::Blue);
}

/// **Nemesis wins.** Being a boss is a fact about how a creature spawned;
/// being a nemesis is a fact about what it did to you, which is both more
/// specific and the one a player can act on — so a creature that is both
/// draws as a nemesis, not magenta. Pinned so the two branches inside
/// `difficulty_color` cannot be reordered without a test failing.
#[test]
fn a_boss_that_is_also_a_nemesis_draws_the_nemesis_colour_not_magenta() {
    assert_eq!(difficulty_color(1, 1000, true, true), GlyphColor::Blue);
    assert_eq!(difficulty_color(1000, 1, true, true), GlyphColor::Blue);
}

/// The pure `difficulty_color` tests above prove the bucketing logic; this
/// proves `build_views` actually threads a real `Nemesis` component through
/// to it. Compared against the *computed* pre-mark colour (the same trick
/// `a_shiny_hostile_still_reports_its_difficulty_colour` uses above) rather
/// than a hard-coded bucket, so the test can't pass by accident if the
/// fixture's power ratio happens to change: a call site that silently
/// dropped `is_nemesis` (always passing `false`) would still pass every
/// pure `difficulty_color` test and only this one would catch it.
#[test]
fn a_marked_hostile_draws_the_nemesis_colour_on_the_map_not_just_in_the_pure_bucketing() {
    let mut game = Game::new(9031, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let wild = game
        .spawn_wild_creature("scrapper", pos.x + 1, pos.y)
        .expect("scrapper ships with the game");

    let power = game.world.get::<Stats>(wild).unwrap().power();
    let unmarked_expected = difficulty_color(power, game.player_status().strength, false, false);
    let before = game
        .view_entities(5, 5)
        .into_iter()
        .find(|v| v.entity == wild)
        .expect("the wild program should be in view");
    assert_eq!(
        before.color, unmarked_expected,
        "setup: an unmarked hostile should still read by power ratio"
    );

    game.world.entity_mut(wild).insert(Nemesis(1));
    let after = game
        .view_entities(5, 5)
        .into_iter()
        .find(|v| v.entity == wild)
        .expect("it should still be in view after gaining the component");
    assert_eq!(
        after.color,
        GlyphColor::Blue,
        "a marked nemesis must draw its reserved colour on the real map, \
         not just inside difficulty_color's own unit tests"
    );
    assert_ne!(
        after.color, before.color,
        "the mark has to actually change what's drawn, not merely agree \
         with an unmarked colour by coincidence"
    );
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
                mitigation: boss.base_mitigation,
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
            mitigation: normal.base_mitigation,
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
    game.world.get_mut::<Stats>(player).unwrap().mitigation = 0;
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
                mitigation: 0,
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
                mitigation: 0,
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
    // The tamed program is asked for from inside the base, because that is
    // the space its `Position` is a cell of and `view_entities` selects by
    // space (`Game::stands_in_base_space`). The rule under test — only a
    // hostile is difficulty-coloured — is the same on either side of the
    // anchor.
    stand_in_base_at(&mut game, player_pos.x, player_pos.y + 1);
    let base_views = game.view_entities(5, 5);
    let tamed_view = base_views
        .iter()
        .find(|v| v.entity == tamed_worker)
        .unwrap();

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
        (view.atk, view.mitigation, view.power),
        (status.atk, status.mitigation, status.strength),
        "the manifest must quote the same effective stats the sidebar does"
    );
    assert_eq!(view.level, Some(status.level));
    assert_eq!(view.xp, Some((status.xp, status.xp_to_next)));

    let ManifestSubject::Player(p) = view.subject else {
        panic!("the player is a Player subject");
    };
    assert_eq!(p.power, status.power);
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
    game.equip(game.player_entity(), &gear(&item, 0))
        .expect("equipping a held item works");

    let view = game.manifest(player).unwrap();
    let slot = view
        .equipment
        .iter()
        .find(|s| s.item_name == equippable.name)
        .expect("the item just equipped is listed");
    let (_, base) = game.equipment_of(&item).unwrap();
    let expected = base
        .scaled_for_level(slot.gear_level)
        .fused_for_tier(slot.fusion_tier);
    assert_eq!(
        (slot.atk, slot.mitigation, slot.decompiler),
        (expected.atk, expected.mitigation, expected.decompiler),
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

/// The examine screen (`x` on a program, or the manifest reached from the
/// roster) has to say a benched program is down — see
/// `Game::bench_or_dissolve` and `components::Downed`. It shares the
/// `activity` field `program_activity` derives, so this and the roster's
/// own test of that function must never disagree about the wording.
#[test]
fn manifest_reports_a_downed_program_as_recovering() {
    let mut game = Game::new(17, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 20, 5);
    game.world.entity_mut(pet).insert(crate::components::Downed);

    let view = game.manifest(pet).expect("a tamed program has a manifest");
    let ManifestSubject::Program(p) = view.subject else {
        panic!("a creature is a Program subject");
    };
    assert_eq!(
        p.activity.as_deref(),
        Some("recovering"),
        "a downed program must not read as idle"
    );
}

/// Any program the player owns has been able to wear gear since 0.8.0, and
/// the manifest is the page you open to find out what something *is* — so a
/// companion's loadout belongs on it for the same reason the player's does.
/// It was `PlayerManifest`-only until then, which made "a program has no
/// equipment" a type-level fact that had quietly stopped being true.
#[test]
fn manifest_lists_a_companions_worn_gear() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 20, 5);
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
    game.equip(pet, &gear(&item, 0))
        .expect("a program you own can wear gear");

    let view = game.manifest(pet).expect("a tamed program has a manifest");
    let slot = view
        .equipment
        .iter()
        .find(|s| s.item_name == equippable.name)
        .expect("the item the companion is wearing is listed");
    let (_, base) = game.equipment_of(&item).unwrap();
    let expected = base
        .scaled_for_level(slot.gear_level)
        .fused_for_tier(slot.fusion_tier);
    assert_eq!(
        (slot.atk, slot.mitigation, slot.decompiler),
        (expected.atk, expected.mitigation, expected.decompiler),
        "a companion's row is measured the same way the player's is"
    );
}

/// A wild program has no `Equipment` component at all, so the section is
/// absent rather than a row of empty slots — the same rule the player's page
/// has always followed for a slot it isn't using.
#[test]
fn manifest_of_a_wild_program_lists_no_equipment() {
    let mut game = Game::new(17, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);

    let view = game.manifest(wild).expect("a wild program has a manifest");
    assert!(
        view.equipment.is_empty(),
        "nothing has ever geared a wild program, so there is nothing to list"
    );
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
    base_mitigation: 2,
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
    stand_in_base(&mut game);
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 12);
    place_now(&mut game, "mining_node", 1, 0).unwrap();
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
    place_home(&mut game);
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
    stand_in_base(&mut game);
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 12);
    place_now(&mut game, "mining_node", 1, 0).unwrap();

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

/// `distance` is Chebyshev and `Game::work_structure` refuses anything the
/// player is not *orthogonally* beside, so the two answers genuinely differ
/// on a diagonal — which is the case a screen filtering its "work it
/// yourself" row on `distance <= 1` would get wrong.
///
/// Walked into place through `move_player`, which is the only thing that
/// ever changes `Game::base_pos` — not by mutating the player's `Position`
/// component, which stays pinned to the anchor tile for the whole visit and
/// has no say in base-space adjacency. A fixture that moved `Position`
/// instead would still pass today by accident: `structure_report` used to
/// measure `center` off that same pinned `Position`, which is exactly the
/// bug this test now guards against.
#[test]
fn structure_report_reads_a_diagonal_neighbour_as_not_player_adjacent() {
    let mut game = Game::new(704, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 12);
    place_now(&mut game, "mining_node", 1, 0).unwrap();

    let node_at = |game: &mut Game| {
        game.structure_report()
            .into_iter()
            .find(|s| s.kind == "mining_node")
            .expect("the node was just deployed")
    };

    game.move_player(2, 1);
    assert_eq!(
        game.base_pos(),
        Some((2, 1)),
        "walked there for real, not spawned there"
    );
    let node = node_at(&mut game);
    assert_eq!(
        node.distance, 1,
        "one diagonal step is Chebyshev distance 1"
    );
    assert!(
        !node.player_adjacent,
        "a diagonal is not a tile the node can be worked from"
    );

    game.move_player(0, -1);
    assert_eq!(game.base_pos(), Some((2, 0)));
    assert!(node_at(&mut game).player_adjacent, "one orthogonal step is");
}

/// The Home leads, so the roster opens on the thing the rest of the base is
/// measured from, and identical structures sit together rather than being
/// interleaved by distance.
#[test]
fn structure_report_puts_home_first_and_groups_by_kind() {
    let mut game = Game::new(703, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    unlock_research_chain(&mut game, "armor_bench");
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 60);
    place_home(&mut game);
    place_now(&mut game, "mining_node", 2, 0).unwrap();
    place_now(&mut game, "armory", 1, 0).unwrap();
    place_now(&mut game, "mining_node", 3, 0).unwrap();

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
        .map(|r| game.item_category(&r.copy.item))
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
        .position(|r| r.copy.item.as_str() == ids::MONOFILAMENT_WHIP)
        .unwrap();
    let cell = inventory
        .iter()
        .position(|r| r.copy.item.as_str() == ids::POWER_CELL)
        .unwrap();
    let frag = inventory
        .iter()
        .position(|r| r.copy.item.as_str() == ids::CORE_FRAGMENT)
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
        Some(MachineStatus::Idle),
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

/// A structure on the ray is a legitimate target, so pointing at your
/// Refinery with nothing alive between you and it finds the Refinery.
#[test]
fn the_inspector_finds_a_structure_when_no_creature_is_on_the_ray() {
    let mut game = Game::new(1400, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let start = *game.world.get::<Position>(game.player_entity()).unwrap();
    clear_creatures_east_of_player(&mut game, start, 10);
    // A `Structure` only answers this ray inside base space.
    let base = stand_in_base_and_get_origin(&mut game);

    let refinery = game
        .world
        .spawn((
            Structure {
                kind: "refinery".to_string(),
            },
            Position {
                x: base.x + 3,
                y: base.y,
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
/// creature and a structure at different distances along the ray, the nearer
/// one answers whichever kind it is. Kind decides nothing here — it is the
/// tiebreak for two things on *one* tile, which
/// `two_things_on_one_tile_resolve_the_same_way_however_they_were_spawned`
/// covers.
#[test]
fn the_inspector_returns_whichever_of_the_two_kinds_is_nearer() {
    let mut game = Game::new(1401, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let start = *game.world.get::<Position>(game.player_entity()).unwrap();
    let species = game.species_defs().into_iter().next().unwrap();
    clear_creatures_east_of_player(&mut game, start, 10);
    // A `Structure` only answers this ray inside base space, so the
    // creature fixture below is tamed staff rather than a wild marker —
    // see `spawn_marker_staffer`'s doc for why the pairing has to be that.
    let base = stand_in_base_and_get_origin(&mut game);
    let owner = game.player_entity();

    let spawn_creature = |game: &mut Game, dx: i32| {
        game.world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Position {
                    x: base.x + dx,
                    y: base.y,
                },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    mitigation: 1,
                },
                Tamed { owner },
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
                    x: base.x + dx,
                    y: base.y,
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
/// both on the eastward ray, with any wild leftovers on that row cleared
/// first — the shared fixture for every test asserting what the eastward
/// scan finds and does not find. The creature is nearer so a surface scan
/// resolves to it despite the structure also being on the ray. Returns the
/// creature's entity, which is the only one either test needs to assert
/// against by identity.
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
                mitigation: 1,
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
/// creature this fixture put on the ray is proof the emptiness is the
/// guard's doing, not an accident of an empty row.
#[test]
fn the_inspector_offers_no_structure_while_the_party_is_underground() {
    let (mut game, _creature) = game_with_structure_and_creature_east_of_player(1402);
    let start = *game.world.get::<Position>(game.player_entity()).unwrap();

    game.enter_stack(start.x, start.y);
    assert!(game.is_underground(), "the fixture really went down");

    assert_eq!(
        game.find_target_in_direction(1, 0, 10),
        None,
        "structure and creature both sit on the ray, but the guard refuses \
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

/// The arrangement the ray was built for, and untested before it.
///
/// A posted `GatherResource` worker stands *orthogonally adjacent* to its
/// machine (`hauling::at_station`), so aiming at a machine from the side its
/// worker is parked on used to hit the worker first — and a tamed program at
/// its post is not drawn, so the scan resolved on a tile that looks empty
/// while the machine's glyph sat under the cursor. The inspector now skips
/// any program the map does not draw, so what it names is what you can see.
///
/// The final leg is what keeps this honest: walking the same program off its
/// post makes it drawable, and then it *is* found. Without that, a scan that
/// had simply stopped seeing creatures would pass.
#[test]
fn examining_toward_a_machine_finds_it_past_its_posted_worker() {
    let mut game = Game::new(1409, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 500);
    place_now(&mut game, "mining_node", -4, 0).unwrap();

    let start = *game.world.get::<Position>(game.player_entity()).unwrap();
    let node = {
        let (x, y) = (start.x - 4, start.y);
        let mut query = game.world.query::<(Entity, &Position, &Structure)>();
        query
            .iter(&game.world)
            .find(|(_, p, s)| p.x == x && p.y == y && s.kind == "mining_node")
            .map(|(e, ..)| e)
            .expect("the node was just deployed")
    };

    let worker = spawn_tamed(&mut game, 20, 5);
    game.assign_cronjob(worker, node).unwrap();
    // East of the node, so it stands between the player and the machine.
    park_at_post(&mut game, worker, node);

    assert_eq!(
        game.find_target_in_direction(-1, 0, 10),
        Some(InspectTarget::Structure(node)),
        "aiming west names the machine, not the program parked in front of it"
    );

    // Off on an errand it is drawn, and then it is a legitimate target.
    game.world.get_mut::<Position>(worker).unwrap().x = start.x - 1;
    assert_eq!(
        game.find_target_in_direction(-1, 0, 10),
        Some(InspectTarget::Creature(worker)),
        "a worker away from its post is drawn, so the inspector names it"
    );
}

/// The structure's sheet is the one screen that can report on a program at
/// its post, because a posted program is not drawn on the map and the
/// inspector names only what is drawn — so aiming at the machine is the only
/// way to reach it, and the row has to carry more than a name.
#[test]
fn a_structures_assignee_row_carries_its_workers_level_and_health() {
    let mut game = Game::new(1410, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 500);
    place_now(&mut game, "mining_node", 1, 0).unwrap();

    let start = *game.world.get::<Position>(game.player_entity()).unwrap();
    let node = {
        let (x, y) = (start.x + 1, start.y);
        let mut query = game.world.query::<(Entity, &Position, &Structure)>();
        query
            .iter(&game.world)
            .find(|(_, p, s)| p.x == x && p.y == y && s.kind == "mining_node")
            .map(|(e, ..)| e)
            .expect("the node was just deployed")
    };

    let worker = spawn_tamed(&mut game, 20, 5);
    // Under `TALENT_START_LEVEL`, which is 6: a level the fixture cannot
    // actually reach would be clamped and this would assert on the clamp.
    set_level(&mut game, worker, 5);
    game.assign_cronjob(worker, node).unwrap();
    game.world.get_mut::<Stats>(worker).unwrap().hp = 3;

    let row = game.structure_manifest(node).expect("the node has a row");
    let posted = row
        .assignees
        .iter()
        .find(|a| a.entity == worker)
        .expect("the worker is listed");
    assert_eq!(posted.level, Some(5));
    let (hp, max_hp) = posted.hp.expect("a program has stats");
    assert_eq!(hp, 3, "the row reports the health it actually has");
    assert!(max_hp >= hp);
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
    stand_in_base(&mut game);
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 12);
    place_now(&mut game, "mining_node", 1, 0).unwrap();
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
    // walk to make: `assign_cronjob` starts it from the player's cell.
    stand_in_base_at(&mut game, 0, -4);
    game.assign_cronjob(worker, node).unwrap();
    game.assign_guard(guard, node).unwrap();

    let away = |game: &mut Game, e: Entity| {
        game.view_entities(40, 40)
            .into_iter()
            .find(|v| v.entity == e)
            .map(|v| v.wears_job_mark)
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
    stand_in_base(&mut game);
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 24);
    place_now(&mut game, "mining_node", 1, 0).unwrap();
    place_now(&mut game, "mining_node", 3, 0).unwrap();
    let nodes: Vec<Entity> = game
        .structure_report()
        .into_iter()
        .filter(|s| s.kind == "mining_node")
        .map(|s| s.entity)
        .collect();
    let (worked, guarded) = (nodes[0], nodes[1]);

    let worker = spawn_tamed_on_map(&mut game, 6, 6);
    let guard = spawn_tamed_on_map(&mut game, 6, 7);
    stand_in_base_at(&mut game, 0, -4);
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
    stand_in_base(&mut game);
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 12);
    place_now(&mut game, "mining_node", 1, 0).unwrap();
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
            .wears_job_mark;
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
    stand_in_base(&mut game);
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 48);
    place_now(&mut game, "mining_node", 1, 0).unwrap();
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

    place_now(&mut game, "depot", 1, 2).unwrap();
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

/// The direct demolish key aims at one tile, not down a line. `x`'s ray
/// would let a single keypress take down something across the base.
#[test]
fn adjacent_structure_finds_only_the_neighbouring_tile() {
    let mut game = Game::new(60, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    stand_player_at(&mut game, 0, 0);
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 500);
    place_now(&mut game, "mining_node", 1, 0).unwrap();
    place_now(&mut game, "mining_node", 3, 0).unwrap();

    let east = game
        .adjacent_structure(1, 0)
        .expect("the structure one tile east is adjacent");
    assert_eq!(east.pos, (1, 0));
    assert!(east.is_structure);
    assert!(!east.is_home);

    assert!(
        game.adjacent_structure(0, 1).is_none(),
        "an empty neighbour is nothing to demolish"
    );
    assert!(
        game.adjacent_structure(-1, 0).is_none(),
        "the tile the player stands on is not a neighbour, Home or not"
    );
}

#[test]
fn adjacent_structure_reports_the_home_it_finds() {
    let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    // One cell west of base space's origin, where the Home now stands.
    stand_in_base_at(&mut game, -1, 0);

    let found = game
        .adjacent_structure(1, 0)
        .expect("Home is a structure like any other to this lookup");
    assert!(
        found.is_home,
        "the caller needs this to route Home into its confirmation screen"
    );
}

/// `Position` is pinned to the surface entrance tile while the party is in
/// the Stack, so a direction key down there would aim at the base overhead —
/// the same trap `find_target_in_direction` refuses for.
#[test]
fn adjacent_structure_finds_nothing_underground() {
    let mut game = Game::new(62, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    stand_in_base_at(&mut game, -1, 0);
    assert!(game.adjacent_structure(1, 0).is_some(), "precondition");
    game.world.insert_resource(Locale::Surface);

    let start = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.enter_stack(start.x, start.y);
    assert!(game.is_underground(), "the fixture really went down");

    assert!(
        game.adjacent_structure(1, 0).is_none(),
        "a structure on the surface must not be reachable from four frames down"
    );
}

/// The manifest is the page a player opens to find out what something is,
/// so a rare-spawn tier belongs on it. Before this it appeared only as a
/// two-pixel bar on the map tile and as a tag on the battle roster's front
/// row — nowhere a player could go and *check*.
#[test]
fn a_manifest_carries_the_programs_rare_tier() {
    let mut game = Game::new(63, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(program).insert(Rarity::Gold);

    let view = game.manifest(program).expect("a program has a manifest");
    let ManifestSubject::Program(p) = &view.subject else {
        panic!("a tamed program's manifest is a program manifest");
    };
    assert_eq!(p.rarity, Rarity::Gold);

    let plain = spawn_tamed(&mut game, 10, 3);
    let view = game.manifest(plain).unwrap();
    let ManifestSubject::Program(p) = &view.subject else {
        panic!("a tamed program's manifest is a program manifest");
    };
    assert_eq!(
        p.rarity,
        Rarity::Ordinary,
        "and an ordinary program says so rather than inheriting a tier"
    );
}

/// Every zone past the first is Cold Storage here, with hues nothing else
/// in the shipped set uses, so a fallback cannot pass for the real answer.
const ONLY_COLD: &str = r#"(
    id: "cold_storage",
    name: "Cold Storage",
    description: "Long-idle allocations, frost-locked and slow to answer.",
    shape: (deadlock_temperature: 1.15),
    palette: (ground_hue: 205.0, hazard_hue: 12.0),
)"#;

/// The engine hands the renderer two numbers and no colour: the table and
/// everything about it stays in `crates/gui`.
#[test]
fn zone_one_reports_the_neutral_hues() {
    let assets = assets_dir_with_sectors("hues_zone_one", &[("cold.ron", ONLY_COLD)]);
    let game = Game::new(4242, DifficultyMode::Forgiving, &assets).unwrap();
    assert_eq!(
        game.sector_hues(),
        (
            crate::sectors::NEUTRAL_GROUND_HUE,
            crate::sectors::NEUTRAL_HAZARD_HUE
        )
    );
}

#[test]
fn a_sectors_own_hues_are_reported_after_a_breach() {
    let assets = assets_dir_with_sectors("hues_breached", &[("cold.ron", ONLY_COLD)]);
    let mut game = Game::new(4242, DifficultyMode::Forgiving, &assets).unwrap();
    breach_through_a_portal(&mut game);
    assert_eq!(game.player_status().zone, 2);

    assert_eq!(game.sector_hues(), (205.0, 12.0));
}

/// Absence is supported here too: with no sectors installed the renderer is
/// handed the neutral pair at every zone, which is what reproduces the
/// shipped colour table exactly.
#[test]
fn with_no_sectors_installed_the_hues_stay_neutral() {
    let assets = assets_dir_with_sectors("hues_absent", &[]);
    let mut game = Game::new(4242, DifficultyMode::Forgiving, &assets).unwrap();
    breach_through_a_portal(&mut game);

    assert_eq!(
        game.sector_hues(),
        (
            crate::sectors::NEUTRAL_GROUND_HUE,
            crate::sectors::NEUTRAL_HAZARD_HUE
        )
    );
}

/// The three facts that decide what a program is worth at a post, in one
/// answer — `Game::work_profile`, which the Base Staff screen reads.
///
/// Rootkit is the fixture because its three answers are mutually
/// distinguishable: speed and analysis differ from each other *and* from
/// their species defaults, so a profile that returned the wrong field, or
/// fell through to a default, still fails. Against a species where any two
/// agree, this test would pass with the lookup wired to the wrong one.
#[test]
fn a_work_profile_carries_the_three_facts_that_decide_a_posting() {
    let mut game = Game::new(3210, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 20, 5);
    game.world.get_mut::<Creature>(program).unwrap().species = "rootkit".to_string();

    let def = game
        .species_defs()
        .into_iter()
        .find(|s| s.id == "rootkit")
        .expect("rootkit ships with the game");
    assert_ne!(
        def.base_speed, def.base_int,
        "the fixture stops distinguishing a swapped field the day these agree"
    );
    assert_ne!(def.base_speed, crate::tuning::DEFAULT_BASE_SPEED);
    assert_ne!(def.base_int, crate::tuning::DEFAULT_BASE_INT);

    let profile = game
        .work_profile(program)
        .expect("a program of a shipped species has a work profile");
    assert_eq!(profile.speed, def.base_speed);
    assert_eq!(profile.analysis, def.base_int);
    assert_eq!(
        profile.class,
        Some(AffinityClass::Leech),
        "drain is rootkit's one raised axis, so Leech is its base job"
    );
}

/// `None` rather than a defaulted profile: a species the db has never heard
/// of is a mod that failed to load, and quoting it the roster's baseline
/// numbers would be inventing them.
#[test]
fn a_work_profile_is_none_for_a_species_the_db_never_heard_of() {
    let mut game = Game::new(3211, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 20, 5);
    game.world.get_mut::<Creature>(program).unwrap().species = "no_such_species".to_string();

    assert!(game.work_profile(program).is_none());
}

/// "In base space they ray across the zone surface" — the bug carried into
/// Task 7 for this function. `stand_in_base` puts base space's own origin
/// at `(0, 0)`, so a wild creature planted on the *zone surface* at `(3, 0)`
/// lands squarely on the eastward ray once the party phases in, by the same
/// numeric coincidence `find_walkable_start` and the base's origin usually
/// share. Before the fix this named it; a tamed one at the same tile is the
/// control that proves the guard is about wildlife, not about base space
/// finding nothing at all.
#[test]
fn find_target_in_direction_refuses_a_wild_creature_seen_from_base_space() {
    let mut game = Game::new(3212, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game.species_defs().into_iter().next().unwrap().id;
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.clone(),
            },
            Position { x: 3, y: 0 },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                mitigation: 1,
            },
        ))
        .id();
    stand_in_base(&mut game);

    assert_eq!(
        game.find_target_in_direction(1, 0, 10),
        None,
        "a wild, untamed program on the zone surface must not be named from inside the base"
    );

    // `Tamed` with no `Task` is what `position_is_honest` reads as an idle
    // base staffer — an owned program outside the party is staff by
    // derivation, and that is the only way to make `drawn_on_surface_map`
    // say yes without also giving it a post to walk to or from.
    let owner = game.player_entity();
    game.world.entity_mut(wild).insert(Tamed { owner });
    assert_eq!(
        game.find_target_in_direction(1, 0, 10),
        Some(InspectTarget::Creature(wild)),
        "the same tile, now tamed, proves the refusal above was about wildlife specifically"
    );
}

/// The mirror bug on the surface side, closed generally rather than left
/// open: `Structure` is the space tag, and `find_target_in_direction`'s own
/// `Structure` query answered it with no locale check at all, unlike
/// `find_blocking_structure_at`, `view_entities` and `adjacent_structure`.
/// A player who breaches, builds a Home, and walks back near the spawn tile
/// could aim `x` at a base structure drawn nowhere on their screen — base
/// space's own origin and the zone spawn point are both commonly `(0, 0)`,
/// which this reproduces deliberately (the player is moved onto the exact
/// tile the fixture's Mining Node's base-space position numerically
/// carries) rather than relying on that coincidence to land by luck.
#[test]
fn find_target_in_direction_refuses_a_base_structure_seen_from_the_surface() {
    let mut game = Game::new(3213, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (_home, node) = build_a_base(&mut game);
    assert!(
        !game.in_base(),
        "build_a_base's fixtures return the party to the surface"
    );
    let node_pos = *game.world.get::<Position>(node).unwrap();

    // Standing one tile west of the node's own coordinates, on the
    // surface, so the eastward ray runs straight through them.
    let player = game.player_entity();
    let start = Position {
        x: node_pos.x - 1,
        y: node_pos.y,
    };
    {
        let mut pos = game.world.get_mut::<Position>(player).unwrap();
        pos.x = start.x;
        pos.y = start.y;
    }
    // A wild program spawned on this exact tile would find *it* first and
    // mask the assertion this test is actually making.
    clear_creatures_east_of_player(&mut game, start, 10);

    assert_eq!(
        game.find_target_in_direction(1, 0, 10),
        None,
        "a base structure's position must not answer a surface-space ray, \
         however close the numbers land"
    );
}

/// Unreachable before base space rendered at all: nothing ever called
/// `view_entities` near the anchor or a link, so `entity_label`'s
/// fall-through — "You", the same body the player's own entity gets — never
/// had a caller. `Game::view_tiles` makes it reachable, and a screen naming
/// the anchor "You" would be worse than not naming it.
#[test]
fn entity_label_names_the_anchor_and_a_surface_link_rather_than_falling_through_to_you() {
    let mut game = Game::new(3212, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let anchor = game.world.resource::<resources::AnchorEntity>().0;
    assert_eq!(game.entity_label(anchor), "The Anchor");

    let mut query = game.world.query_filtered::<Entity, With<SurfaceLink>>();
    let link = query
        .iter(&game.world)
        .next()
        .expect("a fresh zone always scatters at least one Stack entrance");
    assert_eq!(game.entity_label(link), "Stack Entrance");

    // The player's own entity is the one thing that must still say "You" —
    // this guards against a fix that widened the match past the two new
    // arms and swallowed the fall-through's real case.
    assert_eq!(game.entity_label(game.player_entity()), "You");
}

#[test]
fn a_manifest_carries_the_to_hit_pair_the_fight_actually_rolls() {
    let game = Game::new(4401, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let view = game.manifest(player).expect("the player has a manifest");

    // Not a second copy of the formula: the sheet has to agree with the roll,
    // so the assertion calls exactly what `battle::resolve_attack` consults.
    let gear = game.gear_bonus(player);
    let level = game.ability_user_level(player);
    let speed = game.combat_speed(player);
    assert_eq!(
        view.accuracy as f64,
        crate::battle::accuracy_of(speed, level, gear.accuracy),
        "the manifest's Accuracy must be the one the attack roll uses"
    );
    assert_eq!(
        view.evasion as f64,
        crate::battle::evasion_of(speed, level, gear.evasion),
        "the manifest's Evasion must be the one the attack roll uses"
    );
}

#[test]
fn a_program_manifest_carries_the_same_to_hit_pair() {
    let mut game = Game::new(4402, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 20, 5);
    let view = game.manifest(program).expect("a program has a manifest");

    assert_eq!(
        view.accuracy as f64,
        crate::battle::accuracy_of(
            game.combat_speed(program),
            game.ability_user_level(program),
            game.gear_bonus(program).accuracy,
        ),
        "a program's sheet is built from the same two calls the player's is, \
         so buying the program page room later is a layout change and not a \
         data change"
    );
}

#[test]
fn a_player_manifest_reports_what_the_run_holds() {
    let mut game = Game::new(4403, DifficultyMode::Permadeath, &test_assets_dir()).unwrap();
    let player = game.player_entity();

    let before = game.manifest(player).unwrap();
    let ManifestSubject::Player(p) = before.subject else {
        panic!("the player is a Player subject");
    };
    assert_eq!(p.difficulty, DifficultyMode::Permadeath);
    assert_eq!(p.credits, game.banked(&items::ids::CREDITS.into()));
    assert_eq!(
        p.portal_fragments,
        game.banked(&items::ids::PORTAL_FRAGMENT.into())
    );
    assert_eq!(p.cycle, game.current_tick());
    assert_eq!(p.active_contracts, game.active_contracts().len());

    // The figures track the run rather than being sampled once at spawn.
    let credits_before = p.credits;
    give(&mut game, &items::ids::CREDITS.into(), 7);
    game.tick();

    let after = game.manifest(player).unwrap();
    let ManifestSubject::Player(q) = after.subject else {
        panic!("the player is a Player subject");
    };
    assert_eq!(q.credits, credits_before + 7);
    assert!(
        q.cycle > p.cycle,
        "the cycle row has to move with the clock, or it is a spawn-time \
         snapshot dressed as a run fact"
    );
}

/// The anchor is the one thing on the surface map that is neither a
/// creature nor a `Structure`, so `is_structure` and `is_player` both say
/// nothing about it and a renderer picking art for it had nothing to read.
/// `is_anchor` is that read — and it must be *exclusive*, or the sprite
/// meant for the portal lands on every wild program on the map.
#[test]
fn the_anchor_is_the_one_entity_its_flag_names() {
    let mut game = Game::new(3200, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let views = game.view_entities(40, 40);
    let anchors: Vec<_> = views.iter().filter(|v| v.is_anchor).collect();

    assert_eq!(
        anchors.len(),
        1,
        "exactly one entity on the map is the anchor, and every other view \
         must say so: {:?}",
        views
            .iter()
            .map(|v| (&v.label, v.is_anchor))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        anchors[0].label, "The Anchor",
        "the flag has to be on the anchor itself, not on whatever happens \
         to share its tile"
    );
}
