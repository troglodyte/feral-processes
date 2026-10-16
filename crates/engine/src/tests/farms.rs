//! Farm plots: marking a box, the one recipe, and the view.

use super::support::*;
use crate::base_grid::BaseGrid;
use crate::components::{CropGrowth, FarmPlot, FarmTile, GlyphColor, Task};
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

/// A base whose crop catalogue also holds a second crop, `moss`.
fn moss_base(seed: u32, tag: &str) -> Game {
    let scratch = scratch_assets_dir(tag);
    std::fs::create_dir_all(&scratch).unwrap();
    copy_shipped_assets(&scratch, &[]);
    std::fs::write(
        scratch.join("crops/moss.ron"),
        r#"(id: "moss", name: "Moss", description: "Green.", produces: "power_cell",
            grow_ticks: 100, stages: 2, glyph: "m", color: Green)"#,
    )
    .unwrap();
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &scratch).unwrap();
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 20);
    place_now(&mut game, "home", 1, 0).unwrap();
    stand_in_base_at(&mut game, crate::tuning::STARTING_POCKET_HALF_WIDTH, 0);
    game
}

#[test]
fn changing_the_recipe_discards_growth_and_the_same_recipe_keeps_it() {
    let mut game = moss_base(6, "farm_recipe");
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
            .query::<&crate::components::FloorItem>()
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
    for t in loaded.world.query::<&FarmTile>().iter(&loaded.world) {
        assert_eq!(
            t.growth,
            CropGrowth::Fallow,
            "a dropped crop forgets growth"
        );
    }
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

// ---------------------------------------------------------------------
// Labour: dimple and harvest
// ---------------------------------------------------------------------

use crate::components::{FarmJob, FloorItem};
use crate::items::FloorThing;
use crate::tuning::{FARM_DIMPLE_TICKS, FARM_HARVEST_TICKS};

/// Slack for the walk from wherever an idle body was drifting out to a tile,
#[test]
fn farm_wants_count_as_jobs_but_never_as_unworked() {
    let mut game = farm_base(54);
    sown_plot(&mut game, (-3, 2), (-1, 2));
    spawn_tamed(&mut game, 10, 3);

    pass(&mut game, 3);
    let demand = game.labour_demand();
    assert!(
        demand.wanted > demand.staff,
        "precondition: the plot outruns the spare staff ({} wanted, {} staff)",
        demand.wanted,
        demand.staff
    );
    assert_eq!(game.unworked_total(), 0);
}

/// `base_space.rs`'s allowance.
const WALK: usize = 20;

fn pass(game: &mut Game, ticks: usize) {
    for _ in 0..ticks {
        game.tick();
    }
}

fn power_cells_at(game: &mut Game, pos: (i32, i32)) -> u32 {
    power_cell_cells(game).iter().filter(|&&p| p == pos).count() as u32
}

/// Every cell a power cell lies on, one item per tile.
fn power_cell_cells(game: &mut Game) -> Vec<(i32, i32)> {
    let cell = FloorThing::Item(ItemId::from("power_cell"));
    let mut query = game.world.query::<(&Position, &FloorItem)>();
    query
        .iter(&game.world)
        .filter(|(_, item)| item.0 == cell)
        .map(|(p, _)| (p.x, p.y))
        .collect()
}

fn tending(game: &mut Game) -> Vec<(Entity, FarmJob)> {
    let mut query = game.world.query::<(Entity, &Task)>();
    query
        .iter(&game.world)
        .filter_map(|(e, t)| match t.kind {
            TaskKind::Tend(job) => Some((e, job)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_tender_dimples_then_harvests_a_power_cell_onto_the_tile() {
    let mut game = farm_base(50);
    let tile = sown_plot(&mut game, (-3, 2), (-3, 2))[0];
    spawn_tamed(&mut game, 10, 3);

    pass(&mut game, FARM_DIMPLE_TICKS as usize + WALK);
    assert!(
        elapsed_of(&game, tile).is_some(),
        "the tile should be dimpled and growing"
    );

    let grow = power_cell_grow_ticks(&game) as usize;
    pass(&mut game, grow + FARM_HARVEST_TICKS as usize + WALK);
    assert_eq!(power_cells_at(&mut game, (-3, 2)), 1);
    assert_ne!(
        game.world.get::<FarmTile>(tile).unwrap().growth,
        CropGrowth::Ripe
    );
}

/// A core fragment on `pos`, the item every occupied-tile test lays.
fn lay_fragment(game: &mut Game, pos: (i32, i32)) -> Entity {
    crate::game::base::floor::spawn_floor_item(
        &mut game.world,
        Position { x: pos.0, y: pos.1 },
        FloorThing::Item(ItemId::from(ids::CORE_FRAGMENT)),
    )
}

#[test]
fn a_floor_item_on_a_fallow_tile_never_blocks_the_dimple_or_the_growth() {
    let mut game = farm_base(51);
    let tile = sown_plot(&mut game, (-3, 2), (-3, 2))[0];
    lay_fragment(&mut game, (-3, 2));
    spawn_tamed(&mut game, 10, 3);
    pass(&mut game, FARM_DIMPLE_TICKS as usize + WALK);
    let dimpled = elapsed_of(&game, tile).expect("an item never blocks the dimple");
    pass(&mut game, 1);
    assert_eq!(
        elapsed_of(&game, tile),
        Some(dimpled + 1),
        "an item never blocks the growth"
    );
}

#[test]
fn an_occupied_ripe_tiles_harvest_lands_on_the_nearest_free_floor() {
    let mut game = farm_base(54);
    let tile = sown_plot(&mut game, (-3, 2), (-3, 2))[0];
    set_growth(&mut game, tile, CropGrowth::Ripe);
    let fragment = lay_fragment(&mut game, (-3, 2));
    spawn_tamed(&mut game, 10, 3);
    pass(&mut game, FARM_HARVEST_TICKS as usize + WALK);
    assert_ne!(
        game.world.get::<FarmTile>(tile).unwrap().growth,
        CropGrowth::Ripe,
        "the tile was harvested"
    );
    assert_eq!(
        game.world.get::<FloorItem>(fragment).map(|f| &f.0),
        Some(&FloorThing::Item(ItemId::from(ids::CORE_FRAGMENT))),
        "the item already on the tile is untouched"
    );
    assert_eq!(
        game.world.get::<Position>(fragment).map(|p| (p.x, p.y)),
        Some((-3, 2))
    );
    let cells = power_cell_cells(&mut game);
    assert_eq!(cells.len(), 1, "{cells:?}");
    let (x, y) = cells[0];
    assert_eq!(
        (x + 3).abs().max((y - 2).abs()),
        1,
        "the harvest lands beside the tile, not on it: {cells:?}"
    );
}

#[test]
fn an_unsown_plot_gets_no_tender() {
    let mut game = farm_base(52);
    game.toggle_plot_box((-3, 2), (-2, 2));
    spawn_tamed(&mut game, 10, 3);

    for _ in 0..(FARM_DIMPLE_TICKS as usize + WALK) {
        game.tick();
        assert!(tending(&mut game).is_empty());
    }
}

#[test]
fn farm_wants_go_unserved_when_production_and_digging_take_every_body() {
    let mut game = farm_base(53);
    spawn_machine_with_recipe_at(&mut game, "extractor", ids::CORE_FRAGMENT, -4, 0);
    game.queue_work_order(crate::WorkOrder::batch(ItemId::from("core_fragment"), 50))
        .unwrap();
    let wall = (crate::tuning::STARTING_POCKET_HALF_WIDTH + 1, 0);
    game.toggle_mark_box(wall, wall, None);
    sown_plot(&mut game, (-3, 2), (-3, 2));
    spawn_tamed(&mut game, 10, 3);
    spawn_tamed(&mut game, 10, 3);

    pass(&mut game, 3);
    assert!(
        tending(&mut game).is_empty(),
        "the farm is the last want and two bodies are spoken for"
    );

    spawn_tamed(&mut game, 10, 3);
    pass(&mut game, 3);
    assert_eq!(tending(&mut game).len(), 1);
}

fn tender_standing_at(game: &mut Game, tile: Entity, job: FarmJob, progress: u32) -> Entity {
    let worker = spawn_tamed(game, 10, 3);
    let at = *game.world.get::<Position>(tile).unwrap();
    game.world.entity_mut(worker).insert((
        Position {
            x: at.x + 1,
            y: at.y,
        },
        Task {
            kind: TaskKind::Tend(job),
            target: tile,
            progress,
            required: match job {
                FarmJob::Dimple => FARM_DIMPLE_TICKS,
                FarmJob::Harvest => FARM_HARVEST_TICKS,
            },
        },
    ));
    worker
}

#[test]
fn a_recipe_change_mid_harvest_drops_the_labour_and_spawns_nothing() {
    let mut game = moss_base(54, "farm_recipe_mid_harvest");
    game.toggle_plot_box((-3, 2), (-3, 2));
    let plot = plot_of(&game, (-3, 2));
    game.set_plot_recipe(plot, &"power_cell".into()).unwrap();
    let tile = tile_at(&mut game, (-3, 2));
    set_growth(&mut game, tile, CropGrowth::Ripe);
    let worker = tender_standing_at(&mut game, tile, FarmJob::Harvest, FARM_HARVEST_TICKS - 1);
    // A different recipe leaves every tile fallow: the job posted for no
    // longer matches the tile.
    game.set_plot_recipe(plot, &"moss".into()).unwrap();

    // The crew step alone: a full `tick()` would let the scheduler re-post
    // the body to the dimple the tile now wants and hide the landing check.
    game.run_farm_crew();

    assert_eq!(power_cells_at(&mut game, (-3, 2)), 0);
    assert_ne!(
        game.world.get::<Task>(worker).map(|t| t.kind),
        Some(TaskKind::Tend(FarmJob::Harvest))
    );
}

#[test]
fn clearing_the_plot_mid_harvest_drops_the_labour_and_spawns_nothing() {
    let mut game = farm_base(55);
    let tile = sown_plot(&mut game, (-3, 2), (-3, 2))[0];
    set_growth(&mut game, tile, CropGrowth::Ripe);
    let worker = tender_standing_at(&mut game, tile, FarmJob::Harvest, FARM_HARVEST_TICKS - 1);
    game.toggle_plot_box((-3, 2), (-3, 2));

    game.run_farm_crew();

    assert_eq!(power_cells_at(&mut game, (-3, 2)), 0);
    assert!(game.world.get::<Task>(worker).is_none());
}

#[test]
fn a_posted_tenders_manifest_row_names_the_plot_cell() {
    let mut game = farm_base(58);
    let tile = sown_plot(&mut game, (-3, 2), (-3, 2))[0];
    let worker = tender_standing_at(&mut game, tile, FarmJob::Dimple, 0);

    let (kind, target) = game.program_post(worker).unwrap();

    assert_eq!(kind, TaskKind::Tend(FarmJob::Dimple));
    assert_eq!(target, "Farm Plot (-3, 2)");
    assert_eq!(game.program_activity(worker), "tending Farm Plot (-3, 2)");
}

#[test]
fn a_harvest_log_line_carries_no_coordinates_so_repeats_condense() {
    let mut game = farm_base(59);
    let tile = sown_plot(&mut game, (-3, 2), (-3, 2))[0];
    set_growth(&mut game, tile, CropGrowth::Ripe);
    let before = game.message_log(500).len();
    tender_standing_at(&mut game, tile, FarmJob::Harvest, FARM_HARVEST_TICKS - 1);

    game.run_farm_crew();

    let lines: Vec<String> = game
        .message_log(500)
        .iter()
        .skip(before)
        .map(|l| l.text.clone())
        .filter(|t| t.contains("harvest"))
        .collect();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(!lines[0].contains('('), "{}", lines[0]);
}

#[test]
fn a_farm_alone_keeps_its_tender_moving_between_tiles() {
    let mut game = farm_base(56);
    sown_plot(&mut game, (-3, 2), (-2, 2));
    spawn_tamed(&mut game, 10, 3);

    pass(&mut game, 2 * (FARM_DIMPLE_TICKS as usize + WALK));

    let tiles = tiles_of(&mut game);
    assert_eq!(tiles.len(), 2);
    assert!(tiles.iter().all(|&t| elapsed_of(&game, t).is_some()));
}

#[test]
fn an_unreachable_plot_tile_is_dropped_silently() {
    let mut game = farm_base(57);
    let tile = sown_plot(&mut game, (-3, 2), (-3, 2))[0];
    // Seal the tile in: every orthogonal neighbour is a structure.
    for (x, y) in [(-4, 2), (-2, 2), (-3, 1), (-3, 3)] {
        spawn_machine_with_recipe_at(&mut game, "extractor", ids::CORE_FRAGMENT, x, y);
    }
    spawn_tamed(&mut game, 10, 3);
    let before = game.message_log(500).len();

    pass(&mut game, FARM_DIMPLE_TICKS as usize + WALK);

    assert!(tending(&mut game).is_empty());
    assert_eq!(elapsed_of(&game, tile), None);
    // The sealing structures say their own idle lines; the plot says nothing.
    let said = game.message_log(500).iter().skip(before).any(|l| {
        let t = l.text.to_lowercase();
        t.contains("plot") || t.contains("cut off") || t.contains("tend")
    });
    assert!(!said, "a sealed plot tile must be dropped without a word");
}

#[test]
fn standing_beside_a_plot_offers_it_on_c() {
    let mut game = farm_base(60);
    let hw = crate::tuning::STARTING_POCKET_HALF_WIDTH;
    game.toggle_plot_box((hw - 1, 0), (hw - 1, 0));
    let plot = plot_of(&game, (hw - 1, 0));
    let offered = game.adjacent_interactions();
    let found = offered
        .iter()
        .find(|i| i.kind == InteractionKind::Plot(plot))
        .expect("the plot beside the party is offered");
    assert_eq!(found.dir, (-1, 0));

    stand_in_base_at(&mut game, hw - 3, 0);
    assert!(
        game.adjacent_interactions()
            .iter()
            .all(|i| i.kind != InteractionKind::Plot(plot)),
        "one step away the plot is not offered"
    );
    stand_in_base_at(&mut game, hw - 1, 0);
    assert!(
        game.adjacent_interactions()
            .iter()
            .all(|i| i.kind != InteractionKind::Plot(plot)),
        "standing on a plot tile alone does not offer it"
    );
}

#[test]
fn a_plot_and_a_stockpile_zone_overlap_without_clearing_each_other() {
    let mut game = farm_base(55);
    game.toggle_plot_box((-3, 2), (-2, 2));
    game.toggle_stockpile_box((-3, 2), (-3, 3));
    game.toggle_plot_box((-3, 3), (-3, 3));
    let plotted: Vec<(i32, i32)> = game.farm_tiles().iter().map(|t| t.pos).collect();
    assert_eq!(plotted, vec![(-3, 2), (-3, 3), (-2, 2)]);
    assert_eq!(game.stockpile_cells(), vec![(-3, 2), (-3, 3)]);
}
