//! Perception: the derived stat that sets how many tiles of the surface the
//! player sees. Analysis supplies three quarters of a spread's worth and
//! Entropy the rest.

use super::support::*;
use crate::attributes::AttributeId;
use crate::components::{
    Attributes, BaseAnchor, Creature, Glyph, GlyphColor, Hostile, Nest, Position, ProgramBase,
    Stats, SurfaceLink,
};
use crate::species::SpeciesId;
use crate::tuning::{PERCEPTION_BASE_RADIUS, PERCEPTION_MIN_RADIUS};
use crate::*;

fn game() -> Game {
    Game::new(7101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn set_attribute(game: &mut Game, id: &str, value: i32) {
    let player = game.player_entity();
    game.world
        .get_mut::<Attributes>(player)
        .unwrap()
        .set(&AttributeId::from(id), value);
    game.recompute_derived(player);
}

fn base_of(game: &Game, id: &str) -> i32 {
    game.world
        .resource::<crate::attributes::AttributeDb>()
        .get(&AttributeId::from(id))
        .unwrap()
        .base
}

#[test]
fn baseline_attributes_see_exactly_the_base_radius() {
    assert_eq!(game().perception_radius(), PERCEPTION_BASE_RADIUS);
}

#[test]
fn a_point_of_analysis_adds_half_a_tile() {
    let mut game = game();
    let base = base_of(&game, "analysis");
    set_attribute(&mut game, "analysis", base + 1);
    assert!((game.perception_radius() - (PERCEPTION_BASE_RADIUS + 0.5)).abs() < 1e-4);
    set_attribute(&mut game, "analysis", base + 14);
    assert!((game.perception_radius() - (PERCEPTION_BASE_RADIUS + 7.0)).abs() < 1e-4);
}

#[test]
fn a_point_of_entropy_adds_a_thirtieth_of_a_tile() {
    let mut game = game();
    let base = base_of(&game, "entropy");
    set_attribute(&mut game, "entropy", base + 1);
    assert!((game.perception_radius() - (PERCEPTION_BASE_RADIUS + 0.033)).abs() < 1e-4);
}

#[test]
fn attributes_far_below_base_clamp_at_the_floor() {
    let mut game = game();
    set_attribute(&mut game, "analysis", 0);
    set_attribute(&mut game, "entropy", 0);
    assert_eq!(game.perception_radius(), PERCEPTION_MIN_RADIUS);
}

/// `DerivedBase` is written into every seated program's `CreatureSave`, and
/// the `dev-saves/` templates spell it out, so a record from before the
/// field must read as the baseline rather than as zero.
#[test]
fn a_seated_programs_old_base_loads_with_the_baseline_perception() {
    let mut game = game();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id
        .to_string();
    let program = game
        .spawn_wild_creature_scaled(&species, 60, 60, 1.0, false)
        .unwrap();
    game.world
        .entity_mut(program)
        .remove::<(Hostile, WanderAi)>();
    let parts = game.roster_parts();
    game.world.entity_mut(program).insert(parts);
    game.seat_derived(program);

    let path = std::env::temp_dir().join("feral_perception_old_base.sav");
    game.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let stripped: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("perception:"))
        .map(|l| format!("{l}\n"))
        .collect();
    assert!(
        stripped.len() < text.len(),
        "the save must have carried the field for this test to mean anything"
    );
    std::fs::write(&path, stripped).unwrap();
    let loaded = Game::load(&path, &test_assets_dir());
    let _ = std::fs::remove_file(&path);
    let loaded = loaded.unwrap();

    let seated: Vec<f32> = loaded
        .world
        .iter_entities()
        .filter_map(|e| e.get::<ProgramBase>().map(|b| b.0.perception))
        .collect();
    assert!(!seated.is_empty(), "the program must come back seated");
    assert!(seated.iter().all(|p| *p == PERCEPTION_BASE_RADIUS));
}

fn player_at(game: &mut Game) -> (i32, i32) {
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    (pos.x, pos.y)
}

fn teleport(game: &mut Game, (x, y): (i32, i32)) {
    let player = game.player_entity();
    let mut pos = game.world.get_mut::<Position>(player).unwrap();
    pos.x = x;
    pos.y = y;
}

#[test]
fn a_tile_at_the_radius_is_in_sight_and_one_further_is_not() {
    let mut game = game();
    let (px, py) = player_at(&mut game);
    let r = PERCEPTION_BASE_RADIUS as i32;
    assert_eq!(game.sight_at((px + r, py)), Sight::InSight);
    assert_ne!(game.sight_at((px + r + 1, py)), Sight::InSight);
    // A plain circle: the corner of the box is outside it.
    assert_ne!(game.sight_at((px + r, py + r)), Sight::InSight);
}

#[test]
fn the_start_is_remembered_and_the_far_side_is_unseen() {
    let mut game = game();
    let (px, py) = player_at(&mut game);
    assert_eq!(game.sight_at((px + 20, py)), Sight::Unseen);
    // Seen at the start, before any tick has run.
    teleport(&mut game, (px + 30, py));
    assert_eq!(game.sight_at((px, py)), Sight::Remembered);
    assert_eq!(game.sight_at((px + 20, py)), Sight::Unseen);
}

#[test]
fn a_tick_marks_what_the_player_walked_past() {
    let mut game = game();
    let (px, py) = player_at(&mut game);
    teleport(&mut game, (px + 40, py));
    game.wait();
    teleport(&mut game, (px + 80, py));
    assert_eq!(game.sight_at((px + 40, py)), Sight::Remembered);
}

#[test]
fn off_the_surface_every_tile_is_in_sight() {
    let mut game = game();
    let (px, py) = player_at(&mut game);
    stand_in_base(&mut game);
    assert_eq!(game.sight_at((px + 500, py - 500)), Sight::InSight);
}

#[test]
fn marking_crosses_chunk_boundaries_including_negative_ones() {
    let mut game = game();
    teleport(&mut game, (0, 0));
    game.mark_seen_tiles();
    teleport(&mut game, (900, 900));
    let r = PERCEPTION_BASE_RADIUS as i32;
    for tile in [(-r, 0), (0, -r), (r, 0), (-1, -1), (-3, 3), (3, -3)] {
        assert_eq!(game.sight_at(tile), Sight::Remembered, "{tile:?}");
    }
    assert_eq!(game.sight_at((-r - 1, 0)), Sight::Unseen);
    // Chunks (-1, -1), (0, -1), (-1, 0) and (0, 0) all hold marks.
    let chunks: Vec<_> = game
        .world
        .resource::<crate::resources::SeenTiles>()
        .0
        .keys()
        .copied()
        .collect();
    assert_eq!(chunks, vec![(-1, -1), (-1, 0), (0, -1), (0, 0)]);
}

#[test]
fn sight_view_matches_sight_at_tile_for_tile() {
    let mut game = game();
    let (px, py) = player_at(&mut game);
    let view = game.sight_view_at((px + 3, py), 4, 2);
    assert_eq!(view.len(), 5);
    assert_eq!(view[0].len(), 9);
    for (row, ty) in view.iter().zip(-2..) {
        for (got, tx) in row.iter().zip(-4..) {
            assert_eq!(*got, game.sight_at((px + 3 + tx, py + ty)));
        }
    }
}

#[test]
fn seen_tiles_survive_a_real_save_and_load() {
    let mut game = game();
    let (px, py) = player_at(&mut game);
    teleport(&mut game, (px + 40, py));
    game.mark_seen_tiles();
    let path = std::env::temp_dir().join("feral_perception_seen.sav");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir());
    let _ = std::fs::remove_file(&path);
    let loaded = loaded.unwrap();
    assert_eq!(
        loaded.world.resource::<crate::resources::SeenTiles>(),
        game.world.resource::<crate::resources::SeenTiles>()
    );
    let (lx, ly) = {
        let mut l = loaded;
        let p = player_at(&mut l);
        assert_eq!(l.sight_at((px, py)), Sight::Remembered);
        p
    };
    assert_eq!((lx, ly), (px + 40, py));
}

#[test]
fn an_old_save_without_the_field_loads_with_only_the_start_remembered() {
    let mut game = game();
    let (px, py) = player_at(&mut game);
    teleport(&mut game, (px + 40, py));
    game.mark_seen_tiles();
    let path = std::env::temp_dir().join("feral_perception_old_save.sav");
    game.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let start = text
        .find("seen_tiles:")
        .expect("the save carries the field");
    // Cut the field out by balanced parens.
    let mut depth = 0;
    let mut stop = None;
    for (i, c) in text[start..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    stop = Some(start + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let mut stripped = text[..start].to_string();
    let mut rest = &text[stop.unwrap()..];
    rest = rest.strip_prefix(',').unwrap_or(rest);
    stripped.push_str(rest);
    std::fs::write(&path, stripped).unwrap();
    let loaded = Game::load(&path, &test_assets_dir());
    let _ = std::fs::remove_file(&path);
    let loaded = loaded.unwrap();
    // Load re-marks around the player; the old trail is gone.
    assert_eq!(loaded.sight_at((px, py)), Sight::Unseen);
    assert_eq!(loaded.sight_at((px + 40, py)), Sight::InSight);
}

fn view_of_player(game: &mut Game) -> EntityView {
    game.view_entities(2, 2)
        .into_iter()
        .find(|v| v.is_player)
        .unwrap()
}

fn plain(mut v: EntityView) -> EntityView {
    v.is_player = false;
    v.is_companion = false;
    v.is_landmark = false;
    v.is_structure = false;
    v.is_anchor = false;
    v
}

#[test]
fn shown_at_follows_the_table_row_by_row() {
    let mut game = game();
    let base = plain(view_of_player(&mut game));
    let table =
        |v: &EntityView| [Sight::InSight, Sight::Remembered, Sight::Unseen].map(|s| shown_at(s, v));
    // Wild creature, boss, nemesis, caravan, patrol: only in sight.
    assert_eq!(table(&base), [true, false, false]);
    let mut hostile = base.clone();
    hostile.is_hostile = true;
    hostile.is_boss = true;
    hostile.nemesis = true;
    assert_eq!(table(&hostile), [true, false, false]);
    // A settlement, the anchor, a nest or a Stack link: dim once seen.
    let mut landmark = base.clone();
    landmark.is_landmark = true;
    assert_eq!(table(&landmark), [true, true, false]);
    // A plain surface structure is not a landmark in itself.
    let mut structure = base.clone();
    structure.is_structure = true;
    assert_eq!(table(&structure), [true, false, false]);
    // The player and the party are always shown.
    let mut player = base.clone();
    player.is_player = true;
    assert_eq!(table(&player), [true, true, true]);
    let mut companion = base.clone();
    companion.is_companion = true;
    assert_eq!(table(&companion), [true, true, true]);
}

#[test]
fn built_views_flag_settlements_the_anchor_nests_and_links_as_landmarks() {
    let mut game = game();
    let (px, py) = player_at(&mut game);
    let key = crate::settlements::SettlementKey { rx: 7, ry: -3 };
    let town = place_settlement(&mut game, key, px + 1, py);
    let nest = game
        .world
        .spawn((
            Nest {
                species: SpeciesId::from("x"),
                pending_respawns: vec![],
            },
            Position { x: px + 2, y: py },
            Glyph {
                ch: 'n',
                color: GlyphColor::Red,
            },
        ))
        .id();
    let link = game
        .world
        .spawn((
            SurfaceLink,
            Position {
                x: px + 1,
                y: py + 1,
            },
            Glyph {
                ch: '>',
                color: GlyphColor::Red,
            },
        ))
        .id();
    let anchor = game
        .world
        .spawn((
            BaseAnchor,
            Position {
                x: px + 2,
                y: py + 1,
            },
            Glyph {
                ch: 'H',
                color: GlyphColor::Red,
            },
        ))
        .id();
    let creature = game
        .world
        .spawn((
            Creature {
                species: game.species_defs()[0].id.clone(),
            },
            Position { x: px, y: py + 2 },
            Glyph {
                ch: 'c',
                color: GlyphColor::Red,
            },
        ))
        .id();
    let views = game.view_entities(3, 3);
    let flag = |e: Entity| views.iter().find(|v| v.entity == e).unwrap().is_landmark;
    assert!(flag(town) && flag(nest) && flag(link) && flag(anchor));
    assert!(!flag(creature));
    assert!(!flag(game.player_entity()));
}

#[test]
fn examine_skips_a_hostile_at_eight_tiles_but_names_a_remembered_settlement() {
    let mut game = game();
    let player = game.player_entity();
    let start = *game.world.get::<Position>(player).unwrap();
    clear_creatures_east_of_player(&mut game, start, 12);
    let species = game.species_defs()[0].id.clone();
    let stats = Stats {
        hp: 1,
        max_hp: 1,
        atk: 1,
        mitigation: 1,
    };
    let hostile = game
        .world
        .spawn((
            Creature { species },
            Hostile,
            Position {
                x: start.x + 8,
                y: start.y,
            },
            stats,
        ))
        .id();
    assert_eq!(game.find_target_in_direction(1, 0, 12), None);

    // Raising Perception brings the same creature into sight and examine.
    let base = base_of(&game, "analysis");
    set_attribute(&mut game, "analysis", base + 8);
    assert_eq!(
        game.find_target_in_direction(1, 0, 12),
        Some(InspectTarget::Creature(hostile))
    );
    set_attribute(&mut game, "analysis", base);

    // A town seen earlier stays examinable from out of sight.
    let key = crate::settlements::SettlementKey { rx: 7, ry: -3 };
    let town = place_settlement(&mut game, key, start.x + 5, start.y);
    game.world.despawn(hostile);
    game.mark_seen_tiles();
    teleport(&mut game, (start.x - 3, start.y));
    assert_eq!(game.sight_at((start.x + 5, start.y)), Sight::Remembered);
    assert_eq!(
        game.find_target_in_direction(1, 0, 12),
        Some(InspectTarget::Settlement(town))
    );
}
