use super::*;

/// Every `Biome` variant, listed the way `species.rs`'s own biome census
/// lists them. A new variant missing from here makes the tint census
/// below pass vacuously, which is the failure mode that census exists to
/// prevent — but `biome_tint`'s match is exhaustive, so a new biome
/// cannot reach a test run without someone having already been sent to
/// this file by the compiler.
const ALL_BIOMES: [Biome; 9] = [
    Biome::DataVoid,
    Biome::Deadlock,
    Biome::NullSector,
    Biome::Backplane,
    Biome::OpenGrid,
    Biome::BlackIce,
    Biome::Platform,
    Biome::Excavated,
    Biome::Entropy,
];

/// Whether a tint reads as hostile: red dominant over both other
/// channels. The map's whole colour rule is that hue answers "can I
/// cross this", so this is the question the census below asks.
fn reads_as_hostile(c: Color) -> bool {
    c.r > c.g && c.r > c.b
}

/// Kept for `no_two_walkable_biomes_share_a_hue`, which needs hue in
/// isolation to check the five walkable biomes stay distinguishable from
/// each other. Nothing in production reads hue back out of a `Color` any
/// more — `rotate_hue` was the one caller, and it is gone with the
/// sectors it rotated a palette for.
fn rgb_to_hsv(c: Color) -> (f32, f32, f32) {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == c.r {
        60.0 * (((c.g - c.b) / d).rem_euclid(6.0))
    } else if max == c.g {
        60.0 * ((c.b - c.r) / d + 2.0)
    } else {
        60.0 * ((c.r - c.g) / d + 4.0)
    };
    (h, if max == 0.0 { 0.0 } else { d / max }, max)
}

/// The palette's one load-bearing promise: hue tells the player whether
/// terrain can be walked on, and pattern tells them which biome it is.
/// A biome tinted into the wrong family is worse than a drawing bug —
/// it tells the player they can walk into the void.
///
/// This used to sweep every hue pair a sector was legally allowed to
/// author, because a mod's sector palette could otherwise author a
/// legal-but-unreadable one. Sectors are retired and there is one
/// palette for the whole run, so the sweep is gone; the property it
/// protected — hue answers passability — is still worth asserting
/// against the one palette that ships.
#[test]
fn every_biomes_tint_says_whether_it_can_be_walked_on() {
    for biome in ALL_BIOMES {
        assert_eq!(
            reads_as_hostile(biome_tint(biome)),
            !biome.walkable(),
            "{biome:?} is walkable={} but its tint {:?} reads the other way — hue is \
             the map's only signal for passability",
            biome.walkable(),
            biome_tint(biome),
        );
    }
}

/// What separates the five walkable biomes from each other is brightness,
/// not hue — hue is spoken for by the rule above. Platform in particular
/// has to stay much the darkest: it is the only biome the player lays,
/// it covers whole screens wherever a base stands, and that number was
/// taken down twice after being seen on screen behind a full base.
#[test]
fn platform_is_the_darkest_walkable_biome() {
    let value = |c: Color| c.r.max(c.g).max(c.b);
    let walkable: Vec<Biome> = ALL_BIOMES.into_iter().filter(|b| b.walkable()).collect();
    let mut sorted: Vec<Biome> = walkable.clone();
    sorted.sort_by(|a, b| {
        value(biome_tint(*a))
            .partial_cmp(&value(biome_tint(*b)))
            .unwrap()
    });
    assert_eq!(
        sorted.first(),
        Some(&Biome::Platform),
        "Platform must be the darkest walkable biome"
    );
}

/// The shipped table, locked in against an accidental edit to
/// `biome_reference_tint`. Asserted against the literals rather than a
/// recomputation, so a drift in either direction fails.
#[test]
fn biome_tint_matches_the_shipped_reference_table() {
    let table = [
        (Biome::DataVoid, Color::new(0.95, 0.60, 0.15, 1.0)),
        (Biome::BlackIce, Color::new(0.95, 0.32, 0.18, 1.0)),
        (Biome::Platform, Color::new(0.06, 0.11, 0.32, 1.0)),
        (Biome::Backplane, Color::new(0.25, 0.85, 0.85, 1.0)),
        (Biome::Deadlock, Color::new(0.70, 0.92, 0.95, 1.0)),
        (Biome::OpenGrid, Color::new(0.35, 0.85, 0.60, 1.0)),
        (Biome::NullSector, Color::new(0.20, 0.50, 0.52, 1.0)),
        (Biome::Excavated, Color::new(0.15, 0.22, 0.42, 1.0)),
        (Biome::Entropy, Color::new(0.10, 0.04, 0.04, 1.0)),
    ];
    for (biome, expected) in table {
        let got = biome_tint(biome);
        assert_eq!(
            (got.r, got.g, got.b, got.a),
            (expected.r, expected.g, expected.b, expected.a),
            "{biome:?} is not what the shipped table says it is"
        );
    }
}

/// The five walkable biomes must stay telling-apart-able, and brightness
/// alone does not do it: Backplane and OpenGrid have *identical* value
/// (0.85), so hue is the only thing separating them.
#[test]
fn no_two_walkable_biomes_share_a_hue() {
    let walkable: Vec<Biome> = ALL_BIOMES.into_iter().filter(|b| b.walkable()).collect();
    for (i, a) in walkable.iter().enumerate() {
        for b in &walkable[i + 1..] {
            let (ha, _, _) = rgb_to_hsv(biome_tint(*a));
            let (hb, _, _) = rgb_to_hsv(biome_tint(*b));
            assert!(
                (ha - hb).abs() > 1.0,
                "{a:?} and {b:?} are both at hue {ha}"
            );
        }
    }
}

// ---------------------------------------------------------------------
// Floor finishes: the separation census and draw_finish
// ---------------------------------------------------------------------

fn euclid_dist(a: Color, b: Color) -> f32 {
    ((a.r - b.r).powi(2) + (a.g - b.g).powi(2) + (a.b - b.b).powi(2)).sqrt()
}

/// Every reference a `FloorShade` must clear `FINISH_SHADE_MIN_SEPARATION`
/// from, as the spec's separation rule states it: an exposed rock face at
/// every brightness a shipped kind or a mod's headroom could reach,
/// `Excavated` and plain `Platform`, `THREAT`, the critical damage wash
/// (`Fx::structure_condition`'s red lift over a plain slab), and every
/// other shade. `exclude` is the shade under test, so it is not compared
/// against itself.
fn finish_shade_references(exclude: FloorShade) -> Vec<(String, Color)> {
    let mut refs = vec![
        ("Excavated".to_string(), biome_tint(Biome::Excavated)),
        ("Platform".to_string(), biome_tint(Biome::Platform)),
        ("THREAT".to_string(), hud::palette::THREAT),
        ("damage_wash".to_string(), {
            let bg = at_level(biome_tint(Biome::Platform), GROUND_LEVEL);
            Color::new((bg.r + GROUND_LEVEL).min(1.0), bg.g, bg.b, bg.a)
        }),
    ];
    let mut f = 1.0;
    while f <= 4.0 + 1e-6 {
        refs.push((
            format!("Entropy*{f:.1}"),
            brighten(biome_tint(Biome::Entropy), f),
        ));
        f += 0.1;
    }
    for shade in FloorShade::ALL {
        if shade != exclude {
            refs.push((format!("{shade:?}"), shade_color(shade)));
        }
    }
    refs
}

/// **The census.** Every shade must read as itself and not as a wall, a
/// hazard, or another shade — `biome_tint`'s own load-bearing promise,
/// carried over to a palette that shares the map with rock. Mutation-
/// tested by putting `Umber` back at its original 0.24, 0.16, 0.10 (see
/// `shade_color`'s own comment) and confirming this names it and
/// `Entropy*2.6`.
#[test]
fn every_finish_shade_clears_its_minimum_separation() {
    for shade in FloorShade::ALL {
        let c = shade_color(shade);
        let (name, d) = finish_shade_references(shade)
            .into_iter()
            .map(|(name, rc)| (name, euclid_dist(c, rc)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .expect("the reference list is never empty");
        assert!(
            d >= FINISH_SHADE_MIN_SEPARATION,
            "{shade:?} is only {d:.4} from {name}, short of the {FINISH_SHADE_MIN_SEPARATION} floor"
        );
    }
}

fn a_finish_view(sprite: &str) -> feral_processes_engine::views::FinishView {
    feral_processes_engine::views::FinishView {
        shade: FloorShade::Cobalt,
        sprite: sprite.to_string(),
        name: "Cobalt Carpet".to_string(),
    }
}

#[test]
fn draw_finish_draws_the_fill_the_edge_and_the_sprite() {
    let mut table = SpriteTable::default();
    table.insert_still("cobalt_carpet", bevy_egui::egui::TextureId::User(9));
    let finish = a_finish_view("cobalt_carpet");
    let r = Rect::new(0.0, 0.0, 32.0, 32.0);
    let (_, shapes) = with_sprites(table, |p| draw_finish(p, r, &finish, 1.0));

    let shade = shade_color(FloorShade::Cobalt);
    assert!(
        painted_rect_fill_count(&shapes, shade) > 0,
        "the fill layer must paint the shade itself at dim 1.0"
    );
    assert!(
        painted_rect_stroke_count(&shapes, at_level(shade, FINISH_EDGE_LEVEL)) > 0,
        "the edge ring must be the shade scaled by FINISH_EDGE_LEVEL"
    );
    assert_eq!(
        painted_images(&shapes).len(),
        1,
        "the sprite must draw one textured mesh"
    );
}

/// Shipped floor-finish sprites are opaque, so a sprite tinted at the
/// shade's full strength paints clean over the dimmed fill beneath it —
/// the finish would stop answering to the Power vignette and to cloud
/// dimming the way a plain floor tile does. The edge ring and the
/// sprite tint must both fall with `dim`, not just the fill.
#[test]
fn draw_finish_dims_the_edge_and_the_sprite_along_with_the_fill() {
    let mut table = SpriteTable::default();
    table.insert_still("cobalt_carpet", bevy_egui::egui::TextureId::User(9));
    let finish = a_finish_view("cobalt_carpet");
    let r = Rect::new(0.0, 0.0, 32.0, 32.0);

    let (_, bright) = with_sprites(table.clone(), |p| draw_finish(p, r, &finish, 1.0));
    let (_, dim) = with_sprites(table, |p| draw_finish(p, r, &finish, 0.5));

    let tint = |shapes: &[bevy_egui::egui::epaint::ClippedShape]| {
        painted_images(shapes)
            .first()
            .expect("the sprite must draw one textured mesh")
            .2
    };
    let bright_tint = tint(&bright);
    let dim_tint = tint(&dim);
    assert!(
        dim_tint.r() < bright_tint.r()
            && dim_tint.g() < bright_tint.g()
            && dim_tint.b() < bright_tint.b(),
        "a dimmer draw must tint the sprite darker: {dim_tint:?} vs {bright_tint:?}"
    );
}

/// The overdraw trap this feature shares with `sprite`: a fill and an
/// edge drawn under a sprite that never arrives must not vanish with it.
#[test]
fn draw_finish_with_no_sprite_draws_the_fill_and_edge_alone() {
    let finish = a_finish_view("no_such_sprite");
    let r = Rect::new(0.0, 0.0, 32.0, 32.0);
    let (_, shapes) = with_painter(|p| draw_finish(p, r, &finish, 1.0));

    let shade = shade_color(FloorShade::Cobalt);
    assert!(painted_rect_fill_count(&shapes, shade) > 0);
    assert!(painted_rect_stroke_count(&shapes, at_level(shade, FINISH_EDGE_LEVEL)) > 0);
    assert_eq!(
        painted_images(&shapes).len(),
        0,
        "no sprite is registered under this name, so nothing should be textured"
    );
}

/// A founded base with a real finish, staged the way `drawn_base_at`
/// stages a pending build site: through the save text itself, since
/// `BaseGrid::set_finish` is `pub(crate)` to the engine. `at` must be a
/// laid-floor cell that is not the Home's own tile, or the structure's
/// glyph would take the `occupied` gate before the finish ever could.
fn drawn_base_with_finish(
    sprites: SpriteTable,
    finish_id: &str,
    at: (i32, i32),
) -> Vec<bevy_egui::egui::epaint::ClippedShape> {
    let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets())
        .expect("the shipped assets must load");
    game.place_structure("home", 0, 0, None)
        .expect("a Home founds it");
    game.dev_lay_floor((-2..=8).flat_map(|x| (-2..=6).map(move |y| (x, y))));

    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "fp_gui_base_finish_{}_{unique}.sav",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let patched = text.replacen(
        "finishes: {},",
        &format!("finishes: {{({}, {}): \"{finish_id}\"}},", at.0, at.1),
        1,
    );
    assert_ne!(
        text, patched,
        "a fresh base's finishes map was not the empty `{{}}` this patch expects"
    );
    std::fs::write(&path, patched).unwrap();
    game = Game::load(&path, &test_assets()).unwrap();
    let _ = std::fs::remove_file(&path);

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
    shapes
}

/// The wiring `draw_finish`'s own unit tests cannot see: that a real,
/// finished `Platform` cell reached through `Game::view_finishes_at`
/// actually routes to `draw_finish` rather than `draw_biome`.
///
/// The edge's colour is checked by **channel ratio**, not against a
/// literal `at_level(shade, FINISH_EDGE_LEVEL)`: a real tile's `dim` is
/// `tile_shade`'s own per-tile jitter times the vignette, so it is
/// essentially never exactly `1.0` — `draw_finish_dims_the_edge_and_
/// the_sprite_along_with_the_fill` is what pins the edge and the
/// sprite to a *known* `dim`, and this test only needs to know the
/// ratio survived whatever `dim` this tile actually landed at.
#[test]
fn a_real_finished_cell_draws_through_the_full_pipeline() {
    let mut table = SpriteTable::default();
    table.insert_still("cobalt_carpet", bevy_egui::egui::TextureId::User(9));
    let shapes = drawn_base_with_finish(table, "cobalt_carpet", (3, 3));

    assert_eq!(
        painted_images(&shapes).len(),
        1,
        "the finish's sprite must draw exactly once"
    );

    let shade = shade_color(FloorShade::Cobalt);
    // Tolerance 2, not 1: the edge is `dim` scaled twice over (once for
    // the ambient tile shade, once for `FINISH_EDGE_LEVEL`), and each
    // `at_level` call rounds to a `u8` independently.
    let found = shapes.iter().any(|cs| match &cs.shape {
        bevy_egui::egui::Shape::Rect(r) if r.stroke.width > 0.0 && r.stroke.color.r() > 0 => {
            let k = r.stroke.color.r() as f32 / 255.0 / (shade.r * FINISH_EDGE_LEVEL);
            let expect = |channel: f32| (channel * FINISH_EDGE_LEVEL * k * 255.0).round() as i32;
            (r.stroke.color.g() as i32 - expect(shade.g)).abs() <= 2
                && (r.stroke.color.b() as i32 - expect(shade.b)).abs() <= 2
        }
        _ => false,
    });
    assert!(
        found,
        "the tile loop did not reach draw_finish for a real finished cell"
    );
}

/// **The regression check.** An ordinary, unfinished `Platform` cell —
/// the pocket a founded Home lays, untouched — must draw no finish edge
/// of any shade and no floor-finish sprite: wiring `draw_finish` in must
/// not touch the plain floor path `draw_biome` still owns.
#[test]
fn a_plain_platform_cell_still_draws_the_slab() {
    let shapes = drawn_base(0.0, false);
    for shade in FloorShade::ALL {
        assert_eq!(
            painted_rect_stroke_count(&shapes, at_level(shade_color(shade), FINISH_EDGE_LEVEL)),
            0,
            "a plain floor cell drew {shade:?}'s edge ring"
        );
    }
    assert_eq!(
        painted_images(&shapes).len(),
        0,
        "a plain floor cell must not draw a floor-finish sprite"
    );
}

/// The rim is the edge of the world: it is drawn exactly where
/// passability changes, so it cannot appear inside walkable ground or
/// inside the void. Symmetric because an edge is shared by two tiles and
/// must not depend on which one is asking.
#[test]
fn a_rim_marks_only_the_boundary_between_walkable_and_not() {
    for a in ALL_BIOMES {
        for b in ALL_BIOMES {
            assert_eq!(
                rim(a, b),
                a.walkable() != b.walkable(),
                "rim({a:?}, {b:?}) disagrees with their walkability"
            );
            assert_eq!(rim(a, b), rim(b, a), "rim is not symmetric for {a:?}/{b:?}");
        }
    }
}
