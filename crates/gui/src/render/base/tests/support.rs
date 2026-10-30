use super::*;

pub(super) fn test_assets() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

/// One tile of the map, at the origin, big enough for the mark to have
/// somewhere to bounce.
pub(super) const CELL: f32 = 20.0;
pub(super) const CELL_GLYPH_PX: u16 = 16;

/// A staff program as the map sees one, mending or not.
pub(super) fn patient_view(recovering: bool) -> EntityView {
    EntityView {
        unconfigured: false,
        making: None,
        entity: Entity::PLACEHOLDER,
        pos: (0, 0),
        glyph: 'd',
        sprite: None,
        color: GlyphColor::Cyan,
        label: "Scrapper".into(),
        is_player: false,
        look: None,
        form: None,
        is_tamed: true,
        is_companion: false,
        is_hostile: false,
        is_structure: false,
        shielded: false,
        is_landmark: false,
        is_anchor: false,
        is_home: false,
        tier: None,
        ceiling: None,
        max_tier: None,
        pod_charged: None,
        is_boss: false,
        nemesis: false,
        patrol: None,
        difficulty: None,
        can_work: false,
        can_trade: false,
        issues_contracts: false,
        structure_worker: None,
        wears_job_mark: false,
        position_is_honest: true,
        structure_attended: false,
        recovering,
        refitting: false,
        serviced: false,
        mood: feral_processes_engine::MoodStep::Ok,
        build: None,
        job_progress: None,
        shelf_fill: None,
        output_stranded: false,
        holds_output: false,
        holds_spent_program: false,
        hp_fraction: None,
        level: None,
        durability: None,
        fusions: 0,
        rarity: Rarity::Ordinary,
        machine_status: None,
        linked_edges: Vec::new(),
        unseen_routine: false,
    }
}

/// A fresh `App` with a game already in progress, for a test that draws
/// `draw_playing_base` directly rather than one of its pieces. `game` is
/// assigned by hand instead of walking `App::handle_key` through the new
/// game flow — this only needs *a* game standing on the surface, not the
/// menu path that produces one.
pub(crate) fn playing_app() -> feral_processes_app_core::App {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let tmp = std::env::temp_dir().join(format!("fp_gui_map_frame_census_{}", std::process::id()));
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
    app.game = Some(
        Game::new(7, DifficultyMode::Forgiving, &test_assets())
            .expect("the shipped assets must load"),
    );
    app
}

/// What the map frame's left mount actually draws for `app`'s game right
/// now — `Game::terrain_row` through the same `ground_pieces` the
/// renderer calls, rather than a hardcoded string that would drift the
/// moment the ground readout's wording changed. Read before
/// `draw_playing_base` runs, since `terrain_row` needs `&mut Game`.
pub(super) fn ground_label(app: &mut feral_processes_app_core::App) -> String {
    let row = app
        .game
        .as_mut()
        .and_then(|g| g.terrain_row())
        .expect("a fresh surface game has ground for the border to read");
    hud::map_frame::ground_pieces(&row)[0].0.clone()
}

/// Draws one surface map and reports what landed: the textured meshes,
/// and the text of every glyph painted.
pub(super) fn drawn_map(sprites: SpriteTable) -> (usize, Vec<String>) {
    drawn_map_with(sprites, 0)
}

/// How far the party has to walk before the anchor it spawned on top of
/// is a cell of its own. One step is enough; the constant is named so
/// the assertion above can say what zero means.
pub(super) const STEPS_OFF_THE_ANCHOR: usize = 1;

/// `drawn_map`, with the party walked `steps` tiles east first.
pub(super) fn drawn_map_with(sprites: SpriteTable, steps: usize) -> (usize, Vec<String>) {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets())
        .expect("the shipped assets must load");
    for _ in 0..steps {
        game.move_player(1, 0);
    }
    let mut fx = Fx::new();
    let (tile_px, glyph_px) = crate::text::map_cell(1);
    let (_, shapes) = with_sprites(sprites, |p| {
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
            status.position,
            false,
        );
    });
    (painted_images(&shapes).len(), painted_text(&shapes))
}

/// A drawing with one lit pixel — non-blank, which is all the map cares
/// about, since the pixels themselves live in the texture the upload
/// built and never reach this code.
pub(super) fn a_drawing() -> feral_processes_engine::PlayerIcon {
    let mut canvas = feral_processes_engine::icon::Canvas::new(feral_processes_engine::ICON_SIZE);
    canvas.set(3, 4, 7);
    feral_processes_engine::PlayerIcon::new(
        vec![canvas],
        feral_processes_engine::DEFAULT_SPRITE_FRAME_MS,
    )
    .unwrap()
}

/// A wild creature the map can see (moved into the player's sight, see
/// `near_the_player`), found through the public
/// `Game::view_entities` rather than any of the spawn helpers a gui test
/// cannot reach (`spawn_wild_creature_scaled`, `adopt_program` and
/// friends are all `pub(crate)`). Seed 7's opening population always
/// lands at least one hostile within its stocked chunks — see
/// `Game::ensure_local_population` — and handing back its own `pos`
/// lets the caller centre the draw there directly rather than walking
/// the player to it one step at a time.
///
/// **Filtered on `is_hostile`, not on "not player/anchor/structure".**
/// The wider exclusion also admits a Stack-entrance link (`glyph: '>'`)
/// and a nest, neither a `Creature` nor a `Structure`, so both carry
/// `sprite: None` — and `Game::view_entities` is raw, unsorted bevy
/// query iteration (`Game::view_entities_at`'s own doc), so which
/// candidate `find` lands on is not stable across a query reorder. A
/// wild creature is always `Hostile` on the surface, which neither of
/// those two is, and being hostile implies a `Creature` component,
/// which is what guarantees the `sprite` below is never `None`.
pub(super) fn a_wild_creature(game: Game) -> (Game, EntityView) {
    let mut game = game;
    let far = game
        .view_entities(64, 64)
        .into_iter()
        .find(|ev| ev.is_hostile)
        .expect("seed 7's opening population must hold at least one wild creature");
    near_the_player(game, far)
}

/// The creature moved inside the player's sight and read back from the
/// reloaded game, because the fog hides anything beyond it and a test
/// has no business bypassing the fog.
pub(super) fn near_the_player(game: Game, far: EntityView) -> (Game, EntityView) {
    let (mut game, at) = super::test_support::with_creature_within_sight(game, far.pos);
    let near = game
        .view_entities(8, 8)
        .into_iter()
        .find(|ev| ev.pos == at && ev.is_hostile)
        .expect("the moved creature stands where it was put");
    (game, near)
}

/// `drawn_map`, centred on an arbitrary point instead of the player's
/// own position. The creature-sprite tests need this: a fresh seed's
/// population lands nowhere near spawn, and `draw_surface_map` already
/// takes its centre as an explicit argument rather than always reading
/// `status.position`, so there is no need to walk the player there.
pub(super) fn drawn_map_centered_on(
    game: &mut Game,
    sprites: SpriteTable,
    center: (i32, i32),
) -> (usize, Vec<String>) {
    drawn_map_centered_on_revealed(game, sprites, center, false)
}

/// `drawn_map_centered_on` with `reveal` as an explicit argument — the
/// Alt marker's own tests need both states, and the plain helper above
/// stays `false` for every caller that isn't about the marker.
pub(super) fn drawn_map_centered_on_revealed(
    game: &mut Game,
    sprites: SpriteTable,
    center: (i32, i32),
    reveal: bool,
) -> (usize, Vec<String>) {
    let mut fx = Fx::new();
    let (tile_px, glyph_px) = crate::text::map_cell(1);
    let (_, shapes) = with_sprites(sprites, |p| {
        let status = game.player_status();
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
            center,
            reveal,
        );
    });
    (painted_images(&shapes).len(), painted_text(&shapes))
}

/// A base with one request filed on the cell east of the party, drawn
/// once. Returns the shapes so a caller can ask what landed.
///
/// The party is stood **in base space**, which is the whole point: the
/// site's `Position` is a base-space cell, and `view_entities` selects
/// on `Game::stands_inside_the_base` — drawn from the surface, nothing
/// here would be on the pane at all.
pub(super) fn drawn_base(at: f64, request: bool) -> Vec<bevy_egui::egui::epaint::ClippedShape> {
    drawn_base_at(at, request, true)
}

/// `drawn_base` with the animation switch, so a test can ask where
/// something *rests* as well as where it is this frame.
pub(super) fn drawn_base_at(
    at: f64,
    request: bool,
    animated: bool,
) -> Vec<bevy_egui::egui::epaint::ClippedShape> {
    drawn_base_site(at, request, animated, false)
}

/// `drawn_base_at` with the request filed as a wreck still waiting for a
/// program.
pub(super) fn drawn_base_site(
    at: f64,
    request: bool,
    animated: bool,
    awaiting_program: bool,
) -> Vec<bevy_egui::egui::epaint::ClippedShape> {
    let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets())
        .expect("the shipped assets must load");
    game.place_structure("home", 0, 0, None)
        .expect("a Home founds it");
    if request {
        // **Staged through the save rather than filed through
        // `place_structure`.** Filing a request costs a tamed program
        // now, and a fresh run owns none — so a renderer fixture that
        // called the real door would have to stage a roster, a depth and
        // a spare body before it could draw one orange glyph. The save
        // is the crate's existing way past a private `World` (see
        // `a_settlement_draws_its_glyph_on_the_surface_map`), and it
        // leaves this fixture asking about the *drawing* rather than
        // about what a build costs.
        let (px, py) = game.field_pos().expect("the party stepped inside");
        // A per-call counter, not pid alone: unlike the neighbouring
        // `fp_gui_attention_census_`/`fp_gui_settlement_glyph_` fixtures,
        // this helper is reached from several `#[test]` functions
        // (`a_pending_build_site_draws_a_slab_an_edge_and_a_caret`,
        // `the_caret_over_a_build_site_bounces`,
        // `the_caret_bounces_around_the_middle_of_its_slab`), which cargo
        // runs concurrently in one process. Pid alone gave every thread
        // the same path, so one thread's `remove_file` could land
        // between another's write and its read. See
        // `app-core/src/tests/support/common.rs::scratch_path` for the same
        // trap and the same fix.
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fp_gui_base_site_{}_{unique}.sav",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let mut data = feral_processes_engine::save::load_from_file(&path).unwrap();
        data.build_sites
            .push(feral_processes_engine::save::BuildSiteSave {
                position: (px + 1, py),
                structure: "extractor".to_string(),
                cost: vec![(
                    feral_processes_engine::items::ItemId::from("core_fragment"),
                    5,
                )],
                delivered: Vec::new(),
                progress: 0,
                goal: feral_processes_engine::components::BuildGoal::New,
                program: None,
                recipe: None,
                awaiting_program,
            });
        feral_processes_engine::save::save_to_file(&path, &data).unwrap();
        game = Game::load(&path, &test_assets()).unwrap();
        let _ = std::fs::remove_file(&path);
    }

    let mut fx = Fx::new();
    fx.enabled = animated;
    fx.begin_frame(at, FrameCues::default(), false);
    let (tile_px, glyph_px) = crate::text::map_cell(1);
    let (_, shapes) = with_painter(|p| {
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
            status.position,
            false,
        );
    });
    shapes
}

/// Every rect fill and every rect stroke a frame painted.
pub(super) fn painted_fills_and_strokes(
    shapes: &[bevy_egui::egui::epaint::ClippedShape],
) -> (Vec<bevy_egui::egui::Color32>, Vec<bevy_egui::egui::Color32>) {
    let mut fills = Vec::new();
    let mut strokes = Vec::new();
    for cs in shapes {
        if let bevy_egui::egui::Shape::Rect(r) = &cs.shape {
            fills.push(r.fill);
            if r.stroke.width > 0.0 {
                strokes.push(r.stroke.color);
            }
        }
    }
    (fills, strokes)
}

/// A base with a founded Home and a standing Research Station — footprint
/// `2`, `studies: true` per `assets/structures/research_node.ron` — with
/// nobody pinned to it yet.
///
/// `Game::place_structure` would file a build *request* for anything but
/// Home rather than raising it (`Game::spawn_structure`'s doc), so the
/// Station is written straight into the save the way
/// `test_support::app_in_base_with_an_extractor` stands its Compiler up:
/// `place_structure` would spend a tamed program this fixture has no need
/// to own, and the engine exposes no cheaper way to stand a structure up
/// from outside its own crate.
pub(super) fn game_with_a_research_station(seed: u32, anchor: (i32, i32)) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets())
        .expect("the shipped assets must load");
    game.place_structure("home", 0, 0, None)
        .expect("a Home founds it");
    game.dev_lay_floor((-2..=8).flat_map(|x| (-2..=6).map(move |y| (x, y))));

    let path = crate::render::test_support::scratch_path("research_station", seed);
    let _cleanup = crate::render::test_support::RemoveOnDrop(&path);
    game.save(&path).unwrap();
    let mut data = feral_processes_engine::save::load_from_file(&path).unwrap();
    data.structures
        .push(feral_processes_engine::save::StructureSave {
            recipe: None,
            kind: "research_node".to_string(),
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
            pod_charged: None,
            incubating: Vec::new(),
            study_yield: None,
        });
    feral_processes_engine::save::save_to_file(&path, &data).unwrap();
    Game::load(&path, &test_assets()).unwrap()
}
