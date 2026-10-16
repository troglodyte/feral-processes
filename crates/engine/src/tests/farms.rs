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
