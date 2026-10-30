use super::*;

/// The zone map drawn at `at` seconds, with effects live. The party is
/// left standing where a new run puts it, on the surface — which is the
/// half of the cloud gate this fixture exists to see.
fn drawn_surface_at(at: f64) -> Vec<bevy_egui::egui::epaint::ClippedShape> {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets())
        .expect("the shipped assets must load");
    let mut fx = Fx::new();
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

/// Clouds drift over the field: the ground is shaded differently a minute
/// of wind later.
#[test]
fn clouds_drift_over_the_field() {
    let (surface_early, _) = painted_fills_and_strokes(&drawn_surface_at(0.0));
    let (surface_later, _) = painted_fills_and_strokes(&drawn_surface_at(60.0));
    assert_ne!(
        surface_early, surface_later,
        "the ground must be shaded differently a minute of wind later"
    );
}

/// A base with a founded Home and nothing else, the party stepped
/// `steps` tiles east off it first. `Game::place_structure` raises a
/// Home immediately — no request, no labour — so this is the one
/// non-Home structure test could ever use without a labour crew to
/// finish a build with, and it needs the step off the same way
/// `STEPS_OFF_THE_ANCHOR` does: the party stands on the Home's own tile
/// when it is founded, and an actor there would take the sprite lookup
/// over the structure standing under it.
fn drawn_base_with_sprites(sprites: SpriteTable, steps: usize) -> (usize, Vec<String>) {
    let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets())
        .expect("the shipped assets must load");
    game.place_structure("home", 0, 0, None)
        .expect("a Home founds it");
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

/// A structure whose def has art draws the sprite and gives up its
/// glyph. This is the case `draw_surface_map` never exercised before
/// this task: the sprite lookup used to read only the tile's `actor`,
/// so a bare structure with nobody standing on it could not draw a
/// sprite at all.
#[test]
fn a_structure_with_art_draws_its_sprite_and_not_its_glyph() {
    let mut table = SpriteTable::default();
    // Home's `sprite:` field names "anchor" — the same art the base
    // anchor draws on the zone map outside, see `assets/structures/
    // home.ron`.
    table.insert_still("anchor", bevy_egui::egui::TextureId::User(7));

    let (under, _) = drawn_base_with_sprites(table.clone(), 0);
    assert_eq!(
        under, 0,
        "the party stands on the Home at founding and hides it, so this \
         fixture proves nothing until it steps off"
    );

    let (images, glyphs) = drawn_base_with_sprites(table, 1);

    assert!(images >= 1, "the Home's art must paint a sprite");
    assert!(
        !glyphs.iter().any(|g| g == "#"),
        "the '#' must give way to the sprite, not sit under it: {glyphs:?}"
    );
}

/// The rung under it: a structure with nothing in the table still draws
/// its glyph, exactly as before this task.
#[test]
fn a_structure_with_no_art_draws_its_glyph() {
    let (images, glyphs) = drawn_base_with_sprites(SpriteTable::default(), 1);

    assert_eq!(images, 0, "an empty table must paint no texture");
    assert!(
        glyphs.iter().any(|g| g == "#"),
        "the glyph is what is left when no sprite is loaded: {glyphs:?}"
    );
}

/// One tile draws one sprite — the actor-over-structure precedence test
/// exercises the case where the actor's own lookup misses; this one is
/// the case where it hits, with a table that could satisfy either name.
/// A renderer that painted both a drawn icon *and* the structure under
/// it would still show the drawing (nothing draws over it), so the
/// discriminating assertion is that only one mesh landed at all.
#[test]
fn the_drawn_icon_outranks_a_structures_sprite() {
    let choice = CharacterChoice {
        icon: Some(a_drawing()),
        ..CharacterChoice::default()
    };
    let mut game = Game::new_with(9, DifficultyMode::Forgiving, &test_assets(), &choice)
        .expect("the shipped assets must load");
    game.place_structure("home", 0, 0, None)
        .expect("a Home founds it");

    let mut table = SpriteTable::default();
    table.insert_still(
        crate::sprites::DRAWN_ICON_KEY,
        bevy_egui::egui::TextureId::User(9),
    );
    table.insert_still("anchor", bevy_egui::egui::TextureId::User(7));

    let mut fx = Fx::new();
    let (tile_px, glyph_px) = crate::text::map_cell(1);
    let (_, shapes) = with_sprites(table, |p| {
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
    let images = painted_images(&shapes);

    assert_eq!(images.len(), 1, "one tile draws one sprite");
    assert_eq!(
        images[0].0,
        bevy_egui::egui::TextureId::User(9),
        "the drawing must win the tile from the structure's own sprite"
    );
}

/// **The actor-over-structure precedence itself**, not just "one of the
/// two wins" — this is the case `the_drawn_icon_outranks_a_structures_sprite`
/// cannot reach, because a drawn icon always resolves to `Some` when
/// present and so never needs to ask what stands under it.
///
/// An actor whose own lookup resolves to nothing — no drawn icon, no
/// named sprite — must draw its **glyph**, never fall through to a
/// structure's sprite standing on the same tile. The two are
/// independent rungs, not one ladder: `structure` is read only when
/// `actor` is entirely absent (see the `match` in `draw_surface_map`),
/// and this is the test a `.or_else(|| structure...)` rewrite of that
/// match would still pass every *other* test in this module while
/// failing.
#[test]
fn an_actor_with_no_sprite_of_its_own_draws_its_glyph_over_a_structures_sprite() {
    let choice = CharacterChoice {
        sprite: String::new(),
        icon: None,
        ..CharacterChoice::default()
    };
    let mut game = Game::new_with(9, DifficultyMode::Forgiving, &test_assets(), &choice)
        .expect("the shipped assets must load");
    game.place_structure("home", 0, 0, None)
        .expect("a Home founds it");

    let mut table = SpriteTable::default();
    table.insert_still("anchor", bevy_egui::egui::TextureId::User(7));

    let mut fx = Fx::new();
    let (tile_px, glyph_px) = crate::text::map_cell(1);
    let (_, shapes) = with_sprites(table, |p| {
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

    assert_eq!(
        painted_images(&shapes).len(),
        0,
        "the actor's own miss must not fall through to the structure's sprite"
    );
    assert!(
        painted_text(&shapes).iter().any(|g| g == "@"),
        "the player must not vanish under their own base: {:?}",
        painted_text(&shapes)
    );
}

/// A request draws as a slab, an edge and a caret — all three, because
/// each carries a different half of the message and any one alone reads
/// as something else.
///
/// The slab alone is an unlit tile; the caret alone is a glyph standing
/// on bare ground, which is what every *creature* on this map looks like.
///
/// **Differential against the same base with nothing on order**, and
/// that is load-bearing rather than tidy. Written as "is there a greyish
/// rect somewhere", this test passed with the slab draw deleted
/// outright — the map paints plenty of grey. What only a build site can
/// produce is a fill and a stroke the identical frame without one never
/// paints at all.
#[test]
fn a_pending_build_site_draws_a_slab_an_edge_and_a_caret() {
    let with = drawn_base(0.0, true);
    let without = drawn_base(0.0, false);
    let (with_fills, with_strokes) = painted_fills_and_strokes(&with);
    let (bare_fills, bare_strokes) = painted_fills_and_strokes(&without);

    assert!(
        with_fills.iter().any(|c| !bare_fills.contains(c)),
        "the slab is a fill the same frame without a request never paints"
    );
    assert!(
        with_strokes.iter().any(|c| !bare_strokes.contains(c)),
        "and its edge is a stroke that frame never paints either"
    );
    assert!(
        crate::paint::painted_text(&with).iter().any(|g| g == "^"),
        "the caret is what says work is happening here: {:?}",
        crate::paint::painted_text(&with)
    );
    assert!(
        !crate::paint::painted_text(&without)
            .iter()
            .any(|g| g == "^"),
        "and it is drawn only where a request stands"
    );
}

/// A wreck waiting for a program wears a still "!" in the caret's place:
/// the bouncing caret says a crew is on the job, which is the one thing
/// not true of it.
#[test]
fn a_site_awaiting_a_program_wears_a_still_bang_not_the_caret() {
    let text = |at| {
        let shapes = drawn_base_site(at, true, true, true);
        crate::paint::painted_text(&shapes)
    };
    let first = text(0.0);
    assert!(first.iter().any(|g| g == "!"), "{first:?}");
    assert!(!first.iter().any(|g| g == "^"), "{first:?}");

    let bang_y = |at| {
        let shapes = drawn_base_site(at, true, true, true);
        shapes
            .iter()
            .find_map(|cs| match &cs.shape {
                bevy_egui::egui::Shape::Text(t) if t.galley.text() == "!" => Some(t.pos.y),
                _ => None,
            })
            .expect("the marker is drawn")
    };
    assert_eq!(bang_y(0.0), bang_y(0.25), "it does not bounce");
}

/// ...and the caret bounces.
///
/// **The motion is the point**, not decoration: a build site is the one
/// cell on this map where "nothing is happening yet" is the wrong
/// reading, and a still caret says exactly that. Two frames at different
/// times, comparing where the glyph landed — a test that only checks the
/// caret exists passes against a renderer that pinned it.
/// Where the caret landed in a frame.
fn caret_y(shapes: &[bevy_egui::egui::epaint::ClippedShape]) -> f32 {
    shapes
        .iter()
        .find_map(|cs| match &cs.shape {
            bevy_egui::egui::Shape::Text(t) if t.galley.text() == "^" => Some(t.pos.y),
            _ => None,
        })
        .expect("the caret is drawn")
}

#[test]
fn the_caret_over_a_build_site_bounces() {
    // A quarter of the bob's period apart, so the raised cosine is at
    // genuinely different heights rather than at two points that happen
    // to share one.
    let (a, b) = (
        caret_y(&drawn_base(0.0, true)),
        caret_y(&drawn_base(0.25, true)),
    );
    assert_ne!(
        a, b,
        "the caret must sit at different heights across frames — a still one reads as a \
         glyph standing on the ground"
    );
}

/// ...and it bounces *around* the middle of its slab, not on top of it.
///
/// The caret rides the same raised cosine the staffed mark does, and
/// that one is upward-only by design — its rest position is an inset off
/// the tile's bottom edge that a down-swing would spend. The caret's
/// rest position is the centre of its own slab, so anchored the same way
/// it spent its whole cycle at or above centre and read as sitting high
/// in the tile.
///
/// **Eight frames spanning one full second-long cycle** (the bob runs at
/// 1 Hz), rather than the two frames half a period apart this test used
/// to take.
///
/// Two fixed samples tied the result to *this entity's* incidental
/// phase: `Fx::centred_bob` keys its phase off `Entity::to_bits()`,
/// which `place_structure` hands out from bevy's own allocator — this
/// test has no way to see or choose it, and never should (the engine's
/// `World` is private outside its own crate). For six of the sixty-four
/// phase buckets a raised cosine sampled exactly half a period apart
/// lands on the identical pixel both times: that offset is a genuine
/// zero of the curve, not a flaw in it, and any two-sample, fixed-gap
/// probe has some phase that defeats it.
///
/// Eight samples 45° apart covering a full turn do not have that
/// problem. Two identities hold for *any* rotation of eight equally
/// spaced points around a circle: the offsets always average to zero
/// (the real parts of the eighth roots of unity sum to zero), and no
/// point on the circle — the true peak or trough included — is ever
/// more than 22.5° from its nearest sample. `cos(22.5°) ≈ 0.92`, so
/// against the swing's 2px half-amplitude the worst-placed sample still
/// clears 1.8px on each side; asserting past 1.0px leaves real margin
/// while asserting nothing a rendering rounding error could trip.
#[test]
fn the_caret_bounces_around_the_middle_of_its_slab() {
    let rest = caret_y(&drawn_base_at(0.0, true, false));
    let offsets: Vec<f32> = (0..8)
        .map(|i| caret_y(&drawn_base_at(i as f64 / 8.0, true, true)) - rest)
        .collect();

    let mean = offsets.iter().sum::<f32>() / offsets.len() as f32;
    assert!(
        mean.abs() < 0.01,
        "eight frames spanning a full cycle must average out to the caret's rest position: \
         {offsets:?} (mean {mean}) about {rest}"
    );

    let min = offsets.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = offsets.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    assert!(
        min < -1.0 && max > 1.0,
        "the swing must clear the rest position by a real margin on both sides across a \
         full cycle, whatever phase the caret's own entity happens to carry: {offsets:?} \
         about {rest}"
    );
}
