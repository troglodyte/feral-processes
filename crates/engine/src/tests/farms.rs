//! Farm plots: marking a box, the one recipe, and the view.

use super::support::*;
use crate::base_grid::BaseGrid;
use crate::components::{CropGrowth, FarmPlot, FarmTile, GlyphColor};
use crate::crops::CropId;
use crate::game::base::farm::growth_stage;
use crate::views::FarmTileState;
use crate::*;

fn farm_base(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 20);
    place_now(&mut game, "home", 1, 0).unwrap();
    stand_in_base_at(&mut game, crate::tuning::STARTING_POCKET_HALF_WIDTH, 0);
    game
}

fn plot_count(game: &mut Game) -> usize {
    game.world.query::<&FarmPlot>().iter(&game.world).count()
}

fn plot_of(game: &Game, pos: (i32, i32)) -> Entity {
    game.farm_tiles()
        .into_iter()
        .find(|t| t.pos == pos)
        .unwrap()
        .plot
}

fn set_growth(game: &mut Game, tile: Entity, growth: CropGrowth) {
    game.world.get_mut::<FarmTile>(tile).unwrap().growth = growth;
}

#[test]
fn a_box_marks_one_plot_with_no_recipe() {
    let mut game = farm_base(1);
    game.toggle_plot_box((-3, 2), (-1, 3));
    let tiles = game.farm_tiles();
    assert_eq!(tiles.len(), 6);
    assert!(
        tiles
            .iter()
            .all(|t| t.crop.is_none() && t.state == FarmTileState::Fallow)
    );
    let plots: std::collections::HashSet<_> = tiles.iter().map(|t| t.plot).collect();
    assert_eq!(plots.len(), 1);
}

#[test]
fn a_box_skips_rock_and_structure_cells() {
    let mut game = farm_base(2);
    let home = *game.structure_tiles().iter().min().unwrap();
    let rock = (0..)
        .map(|d| (crate::tuning::STARTING_POCKET_HALF_WIDTH + d, 0))
        .find(|c| game.world.resource::<BaseGrid>().is_solid(c.0, c.1))
        .unwrap();
    let (a, b) = (
        (home.0.min(rock.0), home.1.min(rock.1)),
        (home.0.max(rock.0), home.1.max(rock.1)),
    );
    game.toggle_plot_box(a, b);
    let cells: Vec<_> = game.farm_tiles().into_iter().map(|t| t.pos).collect();
    assert!(!cells.is_empty());
    assert!(!cells.contains(&home));
    assert!(!cells.contains(&rock));
}

#[test]
fn a_box_anchored_on_a_plot_tile_clears_it() {
    let mut game = farm_base(3);
    game.toggle_plot_box((-3, 2), (-1, 3));
    game.toggle_plot_box((-3, 2), (-1, 3));
    assert!(game.farm_tiles().is_empty());
    assert_eq!(plot_count(&mut game), 0);
}

#[test]
fn a_box_over_an_existing_plot_leaves_its_tiles_where_they_were() {
    let mut game = farm_base(4);
    game.toggle_plot_box((-3, 2), (-2, 2));
    let a = plot_of(&game, (-3, 2));
    game.set_plot_recipe(a, &"power_cell".into()).unwrap();
    let tile = game.plot_tiles_by_cell()[&(-3, 2)];
    set_growth(&mut game, tile, CropGrowth::Growing { elapsed: 5 });

    game.toggle_plot_box((-4, 2), (-1, 2));
    let tiles = game.farm_tiles();
    assert_eq!(tiles.len(), 4);
    for t in &tiles {
        if t.pos == (-3, 2) || t.pos == (-2, 2) {
            assert_eq!(t.plot, a);
        } else {
            assert_ne!(t.plot, a);
        }
    }
    let new: std::collections::HashSet<_> = tiles
        .iter()
        .filter(|t| t.plot != a)
        .map(|t| t.plot)
        .collect();
    assert_eq!(new.len(), 1);
    assert_eq!(
        game.world.get::<FarmTile>(tile).unwrap().growth,
        CropGrowth::Growing { elapsed: 5 }
    );
}

#[test]
fn setting_a_recipe_sows_the_whole_plot() {
    let mut game = farm_base(5);
    game.toggle_plot_box((-3, 2), (-1, 3));
    let plot = plot_of(&game, (-3, 2));
    game.set_plot_recipe(plot, &"power_cell".into()).unwrap();
    assert_eq!(game.plot_recipe(plot), Some(CropId::from("power_cell")));
    for t in game.farm_tiles() {
        assert_eq!(t.crop, Some(("+".to_string(), GlyphColor::Yellow)));
    }
}

#[test]
fn changing_the_recipe_discards_growth_and_the_same_recipe_keeps_it() {
    let scratch = scratch_assets_dir("farm_recipe");
    std::fs::create_dir_all(&scratch).unwrap();
    copy_shipped_assets(&scratch, &[]);
    std::fs::write(
        scratch.join("crops/moss.ron"),
        r#"(id: "moss", name: "Moss", description: "Green.", produces: "power_cell",
            grow_ticks: 100, stages: 2, glyph: "m", color: Green)"#,
    )
    .unwrap();
    let mut game = Game::new(6, DifficultyMode::Forgiving, &scratch).unwrap();
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 20);
    place_now(&mut game, "home", 1, 0).unwrap();
    stand_in_base_at(&mut game, crate::tuning::STARTING_POCKET_HALF_WIDTH, 0);

    game.toggle_plot_box((-3, 2), (-2, 2));
    let plot = plot_of(&game, (-3, 2));
    game.set_plot_recipe(plot, &"power_cell".into()).unwrap();
    for tile in game.plot_tiles_by_cell().into_values() {
        set_growth(&mut game, tile, CropGrowth::Growing { elapsed: 7 });
    }
    game.set_plot_recipe(plot, &"power_cell".into()).unwrap();
    assert!(
        game.farm_tiles()
            .iter()
            .all(|t| matches!(t.state, FarmTileState::Growing { .. }))
    );
    game.set_plot_recipe(plot, &"moss".into()).unwrap();
    assert!(
        game.farm_tiles()
            .iter()
            .all(|t| t.state == FarmTileState::Fallow
                && t.crop == Some(("m".to_string(), GlyphColor::Green)))
    );
}

#[test]
fn an_unknown_crop_is_refused() {
    let mut game = farm_base(7);
    game.toggle_plot_box((-3, 2), (-2, 2));
    let plot = plot_of(&game, (-3, 2));
    assert!(game.set_plot_recipe(plot, &"nonesuch".into()).is_err());
    assert_eq!(game.plot_recipe(plot), None);
}

#[test]
fn growth_stage_climbs_through_every_stage() {
    assert_eq!(growth_stage(0, 1200, 4), 0);
    assert_eq!(growth_stage(299, 1200, 4), 0);
    assert_eq!(growth_stage(300, 1200, 4), 1);
    assert_eq!(growth_stage(1199, 1200, 4), 3);
    assert_eq!(growth_stage(5000, 1200, 4), 3);
}

fn sown_plot(game: &mut Game, a: (i32, i32), b: (i32, i32)) -> Vec<Entity> {
    game.toggle_plot_box(a, b);
    let plot = plot_of(game, a);
    game.set_plot_recipe(plot, &"power_cell".into()).unwrap();
    tiles_of(game)
}

fn tiles_of(game: &mut Game) -> Vec<Entity> {
    game.world
        .query_filtered::<Entity, With<FarmTile>>()
        .iter(&game.world)
        .collect()
}

fn elapsed_of(game: &Game, tile: Entity) -> Option<u32> {
    match game.world.get::<FarmTile>(tile).unwrap().growth {
        CropGrowth::Growing { elapsed } => Some(elapsed),
        _ => None,
    }
}

fn power_cell_grow_ticks(game: &Game) -> u32 {
    game.crop_defs()
        .into_iter()
        .find(|d| d.id.as_str() == "power_cell")
        .unwrap()
        .grow_ticks
}

#[test]
fn a_dimpled_tile_ripens_after_its_grow_time() {
    let mut game = farm_base(10);
    let tile = sown_plot(&mut game, (-3, 2), (-3, 2))[0];
    set_growth(&mut game, tile, CropGrowth::Growing { elapsed: 0 });
    let grow = power_cell_grow_ticks(&game);
    for _ in 0..grow - 1 {
        game.grow_farms();
    }
    assert!(matches!(
        game.world.get::<FarmTile>(tile).unwrap().growth,
        CropGrowth::Growing { .. }
    ));
    game.grow_farms();
    assert_eq!(
        game.world.get::<FarmTile>(tile).unwrap().growth,
        CropGrowth::Ripe
    );
}

#[test]
fn growth_waits_out_a_battle() {
    let mut game = farm_base(11);
    let tile = sown_plot(&mut game, (-3, 2), (-3, 2))[0];
    set_growth(&mut game, tile, CropGrowth::Growing { elapsed: 5 });
    battle_with_a_pack_of(&mut game, 1, 10);
    assert!(game.has_active_battle());
    game.tick();
    assert_eq!(elapsed_of(&game, tile), Some(5));
}

#[test]
fn an_unsown_plot_never_grows() {
    let mut game = farm_base(12);
    game.toggle_plot_box((-3, 2), (-3, 2));
    let tile = tiles_of(&mut game)[0];
    set_growth(&mut game, tile, CropGrowth::Growing { elapsed: 0 });
    game.grow_farms();
    assert_eq!(elapsed_of(&game, tile), Some(0));
}

#[test]
fn a_build_request_on_a_plot_tile_removes_only_that_tile() {
    let mut game = farm_base(13);
    let w = crate::tuning::STARTING_POCKET_HALF_WIDTH;
    for tile in sown_plot(&mut game, (-3, 2), (-1, 2)) {
        set_growth(&mut game, tile, CropGrowth::Growing { elapsed: 0 });
    }
    assert_eq!(game.farm_tiles().len(), 3);
    file_build(&mut game, "depot", -2 - w, 2).unwrap();
    // Filing a request is itself a tick, so measure from after it.
    let first = tiles_of(&mut game)[0];
    let before = elapsed_of(&game, first).unwrap();
    game.tick();
    let tiles = tiles_of(&mut game);
    assert_eq!(tiles.len(), 2);
    assert!(
        tiles
            .iter()
            .all(|&t| elapsed_of(&game, t) == Some(before + 1))
    );
    assert_eq!(
        game.world
            .query::<&crate::components::FloorPile>()
            .iter(&game.world)
            .count(),
        0
    );
}

fn tile_at(game: &mut Game, cell: (i32, i32)) -> Entity {
    game.plot_tiles_by_cell()[&cell]
}

fn farm_save_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "feral_processes_farms_{tag}_{}.bin",
        std::process::id()
    ))
}

/// Per-plot position sets, so two views compare without comparing the plot
/// `Entity`, which a load re-spawns.
fn plots_by_cells(game: &Game) -> Vec<Vec<(i32, i32)>> {
    let mut by_plot: std::collections::HashMap<Entity, Vec<(i32, i32)>> = Default::default();
    for t in game.farm_tiles() {
        by_plot.entry(t.plot).or_default().push(t.pos);
    }
    let mut groups: Vec<_> = by_plot.into_values().collect();
    groups.sort();
    groups
}

#[test]
fn plots_recipes_and_growth_survive_a_save_and_load() {
    let mut game = farm_base(40);
    game.toggle_plot_box((-3, 2), (-1, 2));
    game.toggle_plot_box((-3, 3), (-1, 3));
    let sown = plot_of(&game, (-3, 2));
    game.set_plot_recipe(sown, &"power_cell".into()).unwrap();
    for (cell, growth) in [
        ((-3, 2), CropGrowth::Fallow),
        ((-2, 2), CropGrowth::Growing { elapsed: 37 }),
        ((-1, 2), CropGrowth::Ripe),
    ] {
        let tile = tile_at(&mut game, cell);
        set_growth(&mut game, tile, growth);
    }
    let before = game.farm_tiles();
    let groups = plots_by_cells(&game);
    assert_eq!(groups.len(), 2);

    let path = farm_save_path("roundtrip");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let after = loaded.farm_tiles();
    assert_eq!(after.len(), before.len());
    for (b, a) in before.iter().zip(&after) {
        assert_eq!((b.pos, &b.crop, b.state), (a.pos, &a.crop, a.state));
    }
    assert_eq!(plots_by_cells(&loaded), groups);
}

#[test]
fn a_plot_whose_crop_is_gone_loads_unsown_with_a_warning() {
    let mut game = farm_base(41);
    game.toggle_plot_box((-3, 2), (-1, 2));
    let plot = plot_of(&game, (-3, 2));
    game.set_plot_recipe(plot, &"power_cell".into()).unwrap();
    let path = farm_save_path("retired");
    game.save(&path).unwrap();
    let mut data = crate::save::load_from_file(&path).unwrap();
    data.farm_plots[0].recipe = Some(CropId::from("retired_crop"));
    for t in &mut data.farm_plots[0].tiles {
        t.growth = CropGrowth::Ripe;
    }
    crate::save::save_to_file(&path, &data).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let tiles = loaded.farm_tiles();
    assert_eq!(tiles.len(), 3);
    assert!(
        tiles
            .iter()
            .all(|t| t.crop.is_none() && t.state == FarmTileState::Fallow)
    );
    let warned = loaded
        .message_log(200)
        .iter()
        .filter(|l| l.text.contains("retired_crop"))
        .count();
    assert_eq!(warned, 1);
    assert!(
        loaded
            .set_plot_recipe(tiles[0].plot, &"power_cell".into())
            .is_ok()
    );
}

#[test]
fn a_save_written_before_farms_loads_with_none() {
    let mut game = farm_base(42);
    game.toggle_plot_box((-3, 2), (-1, 2));
    let path = farm_save_path("prefarms");
    game.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let start = text.find("farm_plots:").expect("the field is written");
    let end = start + text[start..].find("\n    ],\n").unwrap() + "\n    ],".len();
    let older = format!("{}{}", &text[..start], &text[end..]);
    assert!(!older.contains("farm_plots"));
    std::fs::write(&path, older).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(loaded.farm_tiles().is_empty());
}
