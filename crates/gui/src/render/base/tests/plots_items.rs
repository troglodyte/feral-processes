use super::*;

/// A base with a Home, and `thing` laid on the floor one cell off the
/// party's own when `Some`. Written through the save, the engine's only
/// door for a floor item from outside its crate.
fn base_with_a_floor_item(
    seed: u32,
    thing: Option<feral_processes_engine::items::FloorThing>,
) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets())
        .expect("the shipped assets must load");
    game.place_structure("home", 0, 0, None)
        .expect("a Home founds it");
    let at = game.field_pos().expect("in base space");
    let path = crate::render::test_support::scratch_path("floor_pile", seed);
    let _cleanup = crate::render::test_support::RemoveOnDrop(&path);
    game.save(&path).unwrap();
    let mut data = feral_processes_engine::save::load_from_file(&path).unwrap();
    if let Some(thing) = thing {
        data.floor_items
            .push(feral_processes_engine::save::FloorItemSave {
                position: (at.0 + 1, at.1),
                thing,
            });
    }
    feral_processes_engine::save::save_to_file(&path, &data).unwrap();
    Game::load(&path, &test_assets()).unwrap()
}

/// A base with a Home and one farm plot a cell off the party's, with a
/// core fragment on the floor of the same cell when `item`.
fn base_with_a_plot(seed: u32, recipe: Option<&str>, item: bool) -> Game {
    base_with_a_plot_at(seed, recipe, item, 1)
}

/// `base_with_a_plot` with the plot `dx` cells east of the party, so 0
/// puts it under the party's own glyph.
fn base_with_a_plot_at(seed: u32, recipe: Option<&str>, item: bool, dx: i32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets())
        .expect("the shipped assets must load");
    game.place_structure("home", 0, 0, None)
        .expect("a Home founds it");
    let at = game.field_pos().expect("in base space");
    let path = crate::render::test_support::scratch_path("farm_plot", seed);
    let _cleanup = crate::render::test_support::RemoveOnDrop(&path);
    game.save(&path).unwrap();
    let mut data = feral_processes_engine::save::load_from_file(&path).unwrap();
    let cell = (at.0 + dx, at.1);
    data.farm_plots
        .push(feral_processes_engine::save::FarmPlotSave {
            recipe: recipe.map(Into::into),
            tiles: vec![feral_processes_engine::save::FarmTileSave {
                position: cell,
                growth: feral_processes_engine::components::CropGrowth::Growing { elapsed: 0 },
            }],
        });
    if item {
        data.floor_items
            .push(feral_processes_engine::save::FloorItemSave {
                position: cell,
                thing: feral_processes_engine::items::FloorThing::Item(
                    feral_processes_engine::items::ItemId::from("core_fragment"),
                ),
            });
    }
    feral_processes_engine::save::save_to_file(&path, &data).unwrap();
    Game::load(&path, &test_assets()).unwrap()
}

fn painted_on_a_plot_base(mut game: Game) -> Vec<String> {
    let mut fx = Fx::new();
    let (tile_px, glyph_px) = crate::text::map_cell(1);
    let (_, shapes) = with_painter(|p| {
        let status = game.player_status();
        let centre = game.field_pos().unwrap();
        draw_surface_map(
            &mut game,
            &mut fx,
            p,
            Rect::new(0.0, 0.0, 800.0, 600.0),
            tile_px,
            glyph_px,
            &status,
            None,
            None,
            centre,
            false,
        );
    });
    painted_text(&shapes)
}

#[test]
fn a_plot_dims_with_its_tile_light() {
    let tile = |state| FarmTileView {
        pos: (0, 0),
        plot: feral_processes_engine::Entity::PLACEHOLDER,
        crop: Some(("+".to_string(), GlyphColor::Yellow)),
        crop_name: Some("Power Cell".to_string()),
        state,
    };
    let pane = Rect::new(0.0, 0.0, 100.0, 100.0);
    let occupied = Default::default();
    let dims = std::collections::HashMap::from([((0, 0), 0.5_f32)]);
    let (_, shapes) = with_painter(|p| {
        draw_farm_plots(
            p,
            &[tile(FarmTileState::Ripe)],
            &occupied,
            &dims,
            |_| (10.0, 10.0),
            (32.0, 24),
            pane,
        );
    });
    assert_eq!(
        painted_rect_stroke_count(&shapes, at_level(PLOT_EDGE, 0.5)),
        1
    );
    assert_eq!(painted_rect_stroke_count(&shapes, PLOT_EDGE), 0);
    assert_eq!(
        painted_rect_stroke_count(&shapes, at_level(RIPE_EDGE, 0.5)),
        1
    );
    assert_eq!(painted_rect_stroke_count(&shapes, RIPE_EDGE), 0);
}

#[test]
fn crop_glyphs_grow_with_every_stage_and_ripe_is_full_size() {
    let full = 32;
    let sizes: Vec<u16> = (0..4)
        .map(|stage| {
            crop_glyph_px(full, FarmTileState::Growing { stage, stages: 4 })
                .expect("a growing cell draws")
        })
        .collect();
    assert!(sizes.windows(2).all(|w| w[0] < w[1]), "{sizes:?}");
    assert!(sizes.iter().all(|&s| s < full), "{sizes:?}");
    assert_eq!(crop_glyph_px(full, FarmTileState::Ripe), Some(full));
    assert_eq!(crop_glyph_px(full, FarmTileState::Fallow), None);
}

#[test]
fn a_growing_plot_tile_draws_its_crop_glyph_and_an_unsown_one_does_not() {
    let count = |recipe| {
        painted_on_a_plot_base(base_with_a_plot(560_300, recipe, false))
            .iter()
            .filter(|g| g.as_str() == "+")
            .count()
    };
    assert_eq!(count(Some("power_cell")), 1);
    assert_eq!(count(None), 0);
}

#[test]
fn a_crop_never_draws_over_a_glyph_already_on_the_cell() {
    let drawn = painted_on_a_plot_base(base_with_a_plot_at(560_302, Some("power_cell"), false, 0));
    assert_eq!(drawn.iter().filter(|g| g.as_str() == "+").count(), 0);
}

#[test]
fn a_plot_cell_holding_a_glyph_keeps_its_outline_and_ripe_edge_but_no_crop() {
    let tile = FarmTileView {
        pos: (0, 0),
        plot: feral_processes_engine::Entity::PLACEHOLDER,
        crop: Some(("+".to_string(), GlyphColor::Yellow)),
        crop_name: Some("Power Cell".to_string()),
        state: FarmTileState::Ripe,
    };
    let occupied = std::collections::HashSet::from([(0, 0)]);
    let (_, shapes) = with_painter(|p| {
        draw_farm_plots(
            p,
            &[tile],
            &occupied,
            &Default::default(),
            |_| (10.0, 10.0),
            (32.0, 24),
            Rect::new(0.0, 0.0, 100.0, 100.0),
        );
    });
    assert_eq!(painted_rect_stroke_count(&shapes, PLOT_EDGE), 1);
    assert_eq!(painted_rect_stroke_count(&shapes, RIPE_EDGE), 1);
    assert!(painted_text(&shapes).is_empty());
}

/// A floor item on a plot cell takes the cell's glyph: the item's mark
/// draws and the crop's does not, where the same plot bare shows its crop.
#[test]
fn a_floor_item_on_a_plot_draws_its_glyph_and_hides_the_crop() {
    let crops = |item| {
        painted_on_a_plot_base(base_with_a_plot(560_301, Some("power_cell"), item))
            .iter()
            .filter(|g| g.as_str() == "+")
            .count()
    };
    assert_eq!(crops(false), 1);
    assert_eq!(crops(true), 0);
    let fragments = |item| {
        let mut game = base_with_a_plot(560_301, Some("power_cell"), item);
        floor_glyphs_drawn(&mut game, ".", hud::palette::WARN)
    };
    assert_eq!(fragments(true), fragments(false) + 1);
}

/// How many glyphs `text` the base map paints within a rounding step of
/// `want`. A distance and not an equality: the map multiplies every glyph
/// by the vignette and a tile shade. The colour is what keeps the floor's
/// own marks from aliasing the glyph.
fn floor_glyphs_drawn(game: &mut Game, text: &str, want: Color) -> usize {
    let mut fx = Fx::new();
    let (tile_px, glyph_px) = crate::text::map_cell(1);
    let (_, shapes) = with_painter(|p| {
        let status = game.player_status();
        let centre = game.field_pos().unwrap();
        draw_surface_map(
            game,
            &mut fx,
            p,
            Rect::new(0.0, 0.0, 800.0, 600.0),
            tile_px,
            glyph_px,
            &status,
            None,
            None,
            centre,
            false,
        );
    });
    let gap = |a: Color, b: Color| {
        ((a.r - b.r).powi(2) + (a.g - b.g).powi(2) + (a.b - b.b).powi(2)).sqrt()
    };
    crate::paint::painted_map_glyphs(&shapes)
        .into_iter()
        .filter(|(t, c)| t == text && gap(*c, want) < 0.08)
        .count()
}

/// A floor item draws its own glyph, and only because it is there: the
/// identical base without one paints no such glyph in that colour.
#[test]
fn a_floor_item_draws_its_items_glyph_inside_the_base() {
    use feral_processes_engine::items::{FloorThing, ItemId};
    let drawn = |thing: Option<FloorThing>| {
        let mut game = base_with_a_floor_item(560_200, thing);
        floor_glyphs_drawn(&mut game, ".", hud::palette::WARN)
    };
    let with = drawn(Some(FloorThing::Item(ItemId::from("core_fragment"))));
    let without = drawn(None);
    assert_eq!(with, without + 1);
}

#[test]
fn a_downed_program_on_the_floor_draws_the_fallen_mark() {
    use feral_processes_engine::items::{DownedProgram, FloorThing};
    let program = DownedProgram {
        species: "scrapper".to_string(),
        level: 3,
        rarity: feral_processes_engine::components::Rarity::Ordinary,
        boss: false,
        condition: 70,
        carried: None,
    };
    let drawn = |thing: Option<FloorThing>| {
        let mut game = base_with_a_floor_item(560_201, thing);
        floor_glyphs_drawn(&mut game, "x", hud::palette::FALLEN)
    };
    let with = drawn(Some(FloorThing::Program(program)));
    let without = drawn(None);
    assert_eq!(with, without + 1);
}

/// Whether a painted fill is `STRUCTURE_FLOOR_FILL`, up to the vignette's
/// own scaling and `Color32`'s rounding — a hue-only match rather than an
/// exact one, since exactly which tile the camera happens to centre on
/// moves `vig` by a fraction of a percent. `BUILD_SITE_FILL`'s grey and
/// `Biome::Platform`'s dark navy both sit far enough from this hue that
/// the tolerance cannot mistake either for it.
fn is_structure_floor_fill(c: bevy_egui::egui::Color32) -> bool {
    let close = |ch: u8, target: f32| ((ch as f32) - target * 255.0).abs() < 12.0;
    c.a() == 255
        && close(c.r(), STRUCTURE_FLOOR_FILL.r)
        && close(c.g(), STRUCTURE_FLOOR_FILL.g)
        && close(c.b(), STRUCTURE_FLOOR_FILL.b)
}

/// The three non-anchor footprint cells of a standing Research Station
/// take `STRUCTURE_FLOOR_FILL`; the anchor — which draws the structure's
/// own tile and glyph — never does. A plain Home (footprint 1) draws no
/// floor cell at all, the ordinary case the
/// render path can reach; the genuinely legacy case (a bare-spawned 1x1
/// Research Node predating this task) has no door in from outside the
/// engine crate and is pinned at the engine level instead, in
/// `tests/building.rs::view_structure_floor_at_excludes_a_bare_spawned_
/// nodes_own_anchor`.
#[test]
fn a_research_stations_floor_takes_the_fill_and_its_anchor_does_not() {
    let anchor = (3, 0);
    let mut game = game_with_a_research_station(560_100, anchor);
    let mut fx = Fx::new();
    let (tile_px, glyph_px) = crate::text::map_cell(1);
    let (_, with_station) = with_painter(|p| {
        let status = game.player_status();
        draw_surface_map(
            &mut game,
            &mut fx,
            p,
            Rect::new(0.0, 0.0, 800.0, 600.0),
            tile_px,
            glyph_px,
            &status,
            None,
            None,
            anchor,
            false,
        );
    });
    let floor_fills = |shapes: &[bevy_egui::egui::epaint::ClippedShape]| {
        painted_fills_and_strokes(shapes)
            .0
            .into_iter()
            .filter(|c| is_structure_floor_fill(*c))
            .count()
    };
    assert_eq!(
        floor_fills(&with_station),
        3,
        "exactly the three non-anchor footprint cells take the floor fill"
    );

    let mut bare = Game::new(560_101, DifficultyMode::Forgiving, &test_assets())
        .expect("the shipped assets must load");
    bare.place_structure("home", 0, 0, None)
        .expect("a Home founds it");
    let mut fx2 = Fx::new();
    let (_, without_station) = with_painter(|p| {
        let status = bare.player_status();
        draw_surface_map(
            &mut bare,
            &mut fx2,
            p,
            Rect::new(0.0, 0.0, 800.0, 600.0),
            tile_px,
            glyph_px,
            &status,
            None,
            None,
            status.position,
            false,
        );
    });
    assert_eq!(
        floor_fills(&without_station),
        0,
        "a footprint-1 structure draws no floor cells at all"
    );
}

/// An `App` with a founded Home and a standing, footprint-2 Drop Pod
/// Terminal beside it — `game_with_a_research_station`'s technique:
/// `place_structure` would file a request rather than raise one, and
/// would spend a program this fixture has no need to own, so the
/// terminal is written straight into the save instead.
fn app_with_a_drop_pod_terminal(
    seed: u32,
    anchor: (i32, i32),
    charged: bool,
) -> feral_processes_app_core::App {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let tmp = std::env::temp_dir().join(format!(
        "fp_gui_drop_pod_census_{seed}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let mut app = feral_processes_app_core::App::new(
        root.join("assets"),
        tmp.join("saves"),
        tmp.join("history.log"),
        tmp.join("profile.ron"),
        root.join("dev-arenas"),
        tmp.join("telemetry.jsonl"),
    );
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets())
        .expect("the shipped assets must load");
    game.place_structure("home", 0, 0, None)
        .expect("a Home founds it");

    let path = crate::render::test_support::scratch_path("drop_pod_terminal", seed);
    let _cleanup = crate::render::test_support::RemoveOnDrop(&path);
    game.save(&path).unwrap();
    let mut data = feral_processes_engine::save::load_from_file(&path).unwrap();
    data.structures
        .push(feral_processes_engine::save::StructureSave {
            recipe: None,
            kind: "drop_pod_terminal".to_string(),
            position: anchor,
            durability: None,
            tier: None,
            stock_input: Vec::new(),
            stock_output: Vec::new(),
            standing_work: false,
            standing_guard: false,
            denied_items: Vec::new(),
            power_fuel: feral_processes_engine::tuning::POWER_UPKEEP_TICKS,
            build_quality: 1.0,
            racked: Vec::new(),
            hopper: Vec::new(),
            hopper_progress: 0,
            standing_tool: None,
            pod_charged: Some(charged),
            battery_charge: 0,
            incubating: Vec::new(),
            study_yield: None,
        });
    feral_processes_engine::save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &test_assets()).unwrap());
    app
}

/// A spent terminal's glyph dims toward `WARN` — the crew is already
/// walking the bill over, so waiting is what fixes it — while a charged
/// one keeps its authored cyan. The terminal carries no `MachineStatus`
/// at all (it runs no job), so this is the one case `machine_status`
/// alone cannot cover, and it must be checked ahead of the plain
/// `None` fallback rather than folded into `machine_color`.
#[test]
fn a_spent_drop_pod_terminal_dims_toward_warn() {
    let anchor = (3, 0);
    let mut spent = app_with_a_drop_pod_terminal(560_200, anchor, false);
    let mut charged = app_with_a_drop_pod_terminal(560_201, anchor, true);
    let mut fx = Fx::new();
    let m = ui_metrics(900.0);

    let (_, spent_shapes) =
        with_painter(|p| draw_playing_base(&mut spent, &mut fx, None, p, &m, false));
    let (_, charged_shapes) =
        with_painter(|p| draw_playing_base(&mut charged, &mut fx, None, p, &m, false));

    let dist = |a: Color, b: Color| (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs();
    let glyph_of = |shapes: &[bevy_egui::egui::epaint::ClippedShape]| {
        crate::paint::painted_map_glyphs(shapes)
            .into_iter()
            .find(|(g, _)| g == "V")
            .map(|(_, c)| c)
            .expect("the terminal's glyph is drawn")
    };
    let spent_color = glyph_of(&spent_shapes);
    let charged_color = glyph_of(&charged_shapes);

    assert!(
        dist(spent_color, hud::palette::WARN) < dist(spent_color, glyph_color(GlyphColor::Cyan)),
        "a spent terminal should read nearer WARN than its authored cyan: {spent_color:?}"
    );
    assert!(
        dist(charged_color, glyph_color(GlyphColor::Cyan))
            < dist(charged_color, hud::palette::WARN),
        "a charged terminal keeps its authored cyan: {charged_color:?}"
    );
}
