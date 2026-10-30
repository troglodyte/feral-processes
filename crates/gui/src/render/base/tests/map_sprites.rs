use super::*;

/// Advances the wild side one turn at a time until the tactical board
/// hands the turn to the player — `crates/app-core/src/tests/
/// tactical.rs`'s `wait_for_the_player`, copied rather than shared
/// across crates.
fn wait_for_the_player(game: &mut Game) {
    for _ in 0..64 {
        if game.tactical_awaits_input() {
            return;
        }
        assert!(
            game.tactical_ai_turn(),
            "nobody is acting and it is not the player"
        );
    }
    panic!("the turn never came round to the player");
}

/// A `Game` mid-tactical-fight, with the player already emulating a
/// known image — todo #100 Task 6's fixture for the form-drawing
/// tests.
///
/// `resources::EmulationImages` has no public writer short of a real
/// extraction, and nor does a player's slot for an authored routine. So
/// this writes the image and the Emulate routine straight into the save
/// and reloads —
/// `savetool`'s own dump-edit-pack shape, entirely through the
/// engine's public `save::` functions, never `Game::world`.
fn emulating_player() -> (Game, String) {
    use feral_processes_engine::battle::SpecialTargeting;
    use feral_processes_engine::save;

    for seed in 0..200u32 {
        let choice = CharacterChoice::default();
        let mut game = Game::new_with(seed, DifficultyMode::Forgiving, &test_assets(), &choice)
            .expect("the shipped assets must load");
        let species = game
            .species_defs()
            .into_iter()
            .find(|d| !d.is_hybrid())
            .expect("at least one species ships")
            .id;

        let tmp = std::env::temp_dir().join(format!(
            "fp_gui_emulating_player_{}_{seed}.bin",
            std::process::id()
        ));
        game.save(&tmp).expect("the fixture must save");
        let mut data = save::load_from_file(&tmp).expect("the fixture save must load back");
        data.emulation_images.push(species.clone());
        data.player.routines.truncate(1);
        data.player.routines.push("emulate".to_string());
        save::save_to_file(&tmp, &data).expect("the edited save must write back");
        let mut game = Game::load(&tmp, &test_assets()).expect("the edited save must load");
        let _ = std::fs::remove_file(&tmp);

        let mut profile = game.profile().clone();
        profile.tactical_battles = true;
        game.install_profile(profile);

        let at = game.player_status().position;
        let target = game
            .view_entities(12, 12)
            .into_iter()
            .filter(|e| e.is_hostile && !e.is_tamed && !e.is_structure)
            .find(|e| (e.pos.0 - at.0).abs() + (e.pos.1 - at.1).abs() == 1);
        let Some(target) = target else { continue };
        game.move_player(target.pos.0 - at.0, target.pos.1 - at.1);
        if !game.in_tactical_battle() {
            continue;
        }

        wait_for_the_player(&mut game);
        let index = game
            .tactical_routine_options()
            .into_iter()
            .find(|o| o.targeting == SpecialTargeting::Image)
            .expect("emulate must be offered")
            .index;
        assert!(game.tactical_emulate(index, &species));
        return (game, species);
    }
    panic!("no seed under 200 put a lone wild program next to the player");
}

/// Every outline the map pane stroked in `DESTINATION`, for a frame
/// drawn with `travel` as the pending click-travel.
fn destination_outlines(game: &mut Game, travel: Option<TravelGoal>) -> Vec<bevy_egui::egui::Rect> {
    let mut fx = Fx::new();
    let (tile_px, glyph_px) = crate::text::map_cell(1);
    let (_, shapes) = with_painter(|p| {
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
            travel,
            status.position,
            false,
        );
    });
    crate::paint::painted_rect_stroke_boxes(&shapes, hud::palette::DESTINATION)
}

/// A clicked tile is outlined on the map, one tile's width further
/// along for a goal one tile further along — and nothing is outlined
/// while no travel is pending.
#[test]
fn a_clicked_tile_is_outlined_as_the_destination() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets())
        .expect("the shipped assets must load");
    let at = game.player_status().position;
    let (tile_px, _) = crate::text::map_cell(1);

    assert!(
        destination_outlines(&mut game, None).is_empty(),
        "no travel pending, no destination drawn"
    );
    let near = destination_outlines(&mut game, Some(TravelGoal::Tile(at.0 + 3, at.1)));
    let far = destination_outlines(&mut game, Some(TravelGoal::Tile(at.0 + 4, at.1)));
    assert_eq!(near.len(), 1, "one outline for one destination");
    assert_eq!(far.len(), 1);
    assert_eq!(far[0].min.x - near[0].min.x, tile_px);
    assert_eq!(far[0].min.y, near[0].min.y);
}

/// A hostile chosen as the destination is outlined where it is drawn —
/// the same cell a click on its tile would have outlined — and a goal
/// naming a body the map did not draw outlines nothing rather than a
/// stale cell.
#[test]
fn a_targeted_hostile_is_outlined_where_it_is_drawn() {
    let game = Game::new(7, DifficultyMode::Forgiving, &test_assets())
        .expect("the shipped assets must load");
    let (mut game, hostile) = a_wild_creature(game);

    let chased = destination_outlines(&mut game, Some(TravelGoal::Creature(hostile.entity)));
    let clicked = destination_outlines(
        &mut game,
        Some(TravelGoal::Tile(hostile.pos.0, hostile.pos.1)),
    );
    assert_eq!(chased.len(), 1, "the chased hostile's cell is outlined");
    assert_eq!(chased, clicked, "outlined on the cell it is drawn on");

    // A hostile dissolved mid-chase: the goal still names it, and the
    // map drew nothing under that name this frame.
    let gone = Entity::PLACEHOLDER;
    assert!(
        destination_outlines(&mut game, Some(TravelGoal::Creature(gone))).is_empty(),
        "a body the map did not draw gets no outline"
    );
}

/// `drawn_map`, but the game starts from a character-creation `choice`
/// rather than `CharacterChoice::default()` — the sprite-name and
/// colour tests need a player whose `look` carries something other than
/// the empty string and zero every other test's default game has.
fn drawn_map_with_choice(sprites: SpriteTable, choice: &CharacterChoice) -> (usize, Vec<String>) {
    let (images, glyphs) = drawn_map_images(sprites, choice);
    (images.len(), glyphs)
}

/// `drawn_map_with_choice`, keeping the textured meshes themselves
/// rather than counting them — which texture was drawn, and in what
/// tint, is the whole question for the drawn icon's two rules.
#[allow(clippy::type_complexity)]
fn drawn_map_images(
    sprites: SpriteTable,
    choice: &CharacterChoice,
) -> (
    Vec<(
        bevy_egui::egui::TextureId,
        bevy_egui::egui::Rect,
        bevy_egui::egui::Color32,
    )>,
    Vec<String>,
) {
    let mut game = Game::new_with(7, DifficultyMode::Forgiving, &test_assets(), choice)
        .expect("the shipped assets must load");
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
    (painted_images(&shapes), painted_text(&shapes))
}

/// **The overdraw rule, for the rung most exposed to it.** A drawn icon
/// is transparent wherever the player left the canvas bare — every icon
/// is — so a renderer that painted the texture over an `@` that was
/// still there would show the glyph through its own avatar. Against the
/// opaque placeholder art that failure is invisible, which is why both
/// halves are asserted in one test: the mesh landed, **and** the `@`
/// did not.
#[test]
fn the_drawn_icon_stands_in_for_the_at_sign() {
    let choice = CharacterChoice {
        icon: Some(a_drawing()),
        ..CharacterChoice::default()
    };
    let mut table = SpriteTable::default();
    table.insert_still(
        crate::sprites::DRAWN_ICON_KEY,
        bevy_egui::egui::TextureId::User(9),
    );

    let (images, glyphs) = drawn_map_with_choice(table, &choice);

    assert_eq!(images, 1, "exactly one sprite, the player's drawing");
    assert!(
        !glyphs.iter().any(|g| g == "@"),
        "the '@' must give way to the drawing, not sit under it: {glyphs:?}"
    );
}

/// The drawing is the **top** rung: a player who drew one and also
/// carries a named sprite sees the drawing. Both keys are in the table,
/// so nothing here can pass by a lookup that merely missed.
#[test]
fn the_drawn_icon_outranks_the_named_sprite() {
    let choice = CharacterChoice {
        sprite: "hero".to_string(),
        icon: Some(a_drawing()),
        ..CharacterChoice::default()
    };
    let mut table = SpriteTable::default();
    table.insert_still(
        crate::sprites::DRAWN_ICON_KEY,
        bevy_egui::egui::TextureId::User(9),
    );
    table.insert_still("hero", bevy_egui::egui::TextureId::User(4));

    let (images, _) = drawn_map_images(table, &choice);

    assert_eq!(images.len(), 1, "one tile draws one sprite");
    assert_eq!(
        images[0].0,
        bevy_egui::egui::TextureId::User(9),
        "the drawing must win the tile from the named sprite"
    );
}

/// **The drawn icon is the one sprite in the game drawn untinted.**
/// Every other sprite is authored near-white and multiplied by the hue
/// its tile would have worn; a drawing carries its own colours,
/// and multiplying those by an indigo swatch turns most of them black.
/// The tint is therefore neutral — grey at the vignette's own value, so
/// depth shading is kept and the hue is dropped.
#[test]
fn the_drawn_icon_is_drawn_untinted() {
    let choice = CharacterChoice {
        colour: Some(3),
        icon: Some(a_drawing()),
        ..CharacterChoice::default()
    };
    let mut table = SpriteTable::default();
    table.insert_still(
        crate::sprites::DRAWN_ICON_KEY,
        bevy_egui::egui::TextureId::User(9),
    );

    let (images, _) = drawn_map_images(table, &choice);

    let tint = images
        .iter()
        .find(|(id, _, _)| *id == bevy_egui::egui::TextureId::User(9))
        .expect("the drawing must be painted")
        .2;
    assert_eq!(
        tint.r(),
        tint.g(),
        "a neutral tint has equal channels, not a hue: {tint:?}"
    );
    assert_eq!(
        tint.g(),
        tint.b(),
        "a neutral tint has equal channels, not a hue: {tint:?}"
    );
    assert!(
        tint.r() > 0,
        "the vignette must scale the tint, not erase it: {tint:?}"
    );
}

/// The rung under the drawing must not regress: a player who drew
/// nothing keeps their named sprite, **even when the table still holds
/// a drawing**. Which rung runs is read off the player's own look, not
/// off whatever the sprite table happens to be carrying.
#[test]
fn a_player_with_no_drawing_still_draws_the_named_sprite() {
    let choice = CharacterChoice {
        sprite: "hero".to_string(),
        icon: None,
        ..CharacterChoice::default()
    };
    let mut table = SpriteTable::default();
    table.insert_still(
        crate::sprites::DRAWN_ICON_KEY,
        bevy_egui::egui::TextureId::User(9),
    );
    table.insert_still("hero", bevy_egui::egui::TextureId::User(4));

    let (images, glyphs) = drawn_map_images(table, &choice);

    assert_eq!(images.len(), 1, "one tile draws one sprite");
    assert_eq!(
        images[0].0,
        bevy_egui::egui::TextureId::User(4),
        "no drawing means the named sprite still owns the tile"
    );
    assert!(
        !glyphs.iter().any(|g| g == "@"),
        "the named sprite still stands in for the glyph: {glyphs:?}"
    );
}

/// The bottom rung: neither a drawing nor a sprite the table knows
/// leaves the `@`. `assets/sprites/` deleted is still the glyph map.
#[test]
fn a_player_with_neither_still_draws_the_glyph() {
    let choice = CharacterChoice {
        sprite: "no-such-sprite".to_string(),
        icon: None,
        ..CharacterChoice::default()
    };

    let (images, glyphs) = drawn_map_with_choice(SpriteTable::default(), &choice);

    assert_eq!(images, 0, "nothing in the table must paint no texture");
    assert!(
        glyphs.iter().any(|g| g == "@"),
        "the glyph is what is left when neither rung above it draws: {glyphs:?}"
    );
}

/// The overdraw trap, `the_drawn_icon_stands_in_for_the_at_sign`'s
/// shape, for an emulating player's own tile (todo #100 Task 6): the
/// image's sprite must draw, and neither the `@` nor the image's own
/// glyph as text may still be underneath it. Also asserts the
/// player-colour outline the con read and the boss magenta both leave
/// the player findable through.
#[test]
fn the_emulating_players_tile_draws_its_form_and_outline() {
    let (mut game, species) = emulating_player();
    let def = game
        .species_defs()
        .into_iter()
        .find(|d| d.id == species)
        .expect("the fixture's own species must resolve");
    let mut table = SpriteTable::default();
    table.insert_still(def.sprite_name(), bevy_egui::egui::TextureId::User(9));

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
    let glyphs = painted_text(&shapes);

    assert_eq!(images.len(), 1, "exactly one sprite, the image's own");
    assert!(
        !glyphs.iter().any(|g| g == "@"),
        "the '@' must give way to the form, not sit under it: {glyphs:?}"
    );
    let form_glyph = def.glyph.to_string();
    assert!(
        !glyphs.iter().any(|g| g == &form_glyph),
        "the form's own glyph must not draw as text under its sprite \
         either, or the sprite has drawn beside it rather than in place \
         of it: {glyphs:?}"
    );

    // The player-colour outline: `player_look_color(None)` is the
    // `PLAYER` role colour, since the fixture's choice names no swatch.
    let outline = player_look_color(None);
    assert!(
        painted_rect_stroke_count(&shapes, outline) > 0,
        "the emulating player's own tile must keep an outline in their \
         colour, since the hue is spent on boss magenta"
    );
}

/// The player's sprite stands in for the player's glyph — it does not
/// draw beside it. Both halves are asserted in one test on purpose: the
/// sprite half alone passes against a renderer that paints the texture
/// over an '@' that is still there, which on white placeholder art looks
/// exactly right and is wrong the moment the sprite has any transparency.
///
/// Deliberately runs against a **bare `Game::new`** — no explicit
/// choice — because that is what every save written before the wizard
/// existed, every `dev-saves/` template, and any run that skips the
/// Look step loads as. Handing it a choice that names `"player"` is the
/// accommodation that let the default lose the shipped art in silence
/// once already; `the_player_sprite_comes_from_the_choice` is what
/// covers a wizard-authored name.
#[test]
fn the_player_sprite_stands_in_for_the_at_sign() {
    let mut table = SpriteTable::default();
    table.insert_still(
        feral_processes_engine::DEFAULT_PLAYER_SPRITE,
        bevy_egui::egui::TextureId::User(1),
    );

    let (images, glyphs) = drawn_map(table);

    assert_eq!(images, 1, "exactly one sprite, the player's");
    assert!(
        !glyphs.iter().any(|g| g == "@"),
        "the '@' must give way to the sprite, not sit under it: {glyphs:?}"
    );
}

/// The sprite name is not tied to the literal `"player"` — any name the
/// wizard wrote into `PlayerIdentity::sprite` must reach the map the
/// same way, or the wizard's other icon options would be dead choices.
#[test]
fn the_player_sprite_comes_from_the_choice() {
    let choice = CharacterChoice {
        sprite: "hero".to_string(),
        ..CharacterChoice::default()
    };
    let mut table = SpriteTable::default();
    table.insert_still("hero", bevy_egui::egui::TextureId::User(4));

    let (images, glyphs) = drawn_map_with_choice(table, &choice);

    assert_eq!(images, 1, "exactly one sprite, the chosen one");
    assert!(
        !glyphs.iter().any(|g| g == "@"),
        "the '@' must give way to the chosen sprite, not sit under it: {glyphs:?}"
    );
}

/// A name the sprite table has nothing under returns `false` and the
/// caller draws the glyph — the fallback that keeps `assets/sprites/`
/// optional, exercised here against a *chosen* name rather than the
/// empty one a default game carries, so this cannot pass by accident on
/// an empty-string lookup that was never attempted.
#[test]
fn a_missing_sprite_falls_back_to_the_chosen_glyph() {
    let choice = CharacterChoice {
        sprite: "no-such-sprite".to_string(),
        ..CharacterChoice::default()
    };

    let (images, glyphs) = drawn_map_with_choice(SpriteTable::default(), &choice);

    assert_eq!(
        images, 0,
        "a name the table has nothing under must paint no texture"
    );
    assert!(
        glyphs.iter().any(|g| g == "@"),
        "the glyph must still draw when the chosen sprite is missing: {glyphs:?}"
    );
}

/// The chosen colour comes off `PlayerLook`, **0-indexed** —
/// `PLAYER_CHOICES[colour]` — and is what the map actually draws, not
/// just state nothing reads: `the_players_glyph_wears_the_player_role`
/// already covers `None` staying `PLAYER`, so this is the other half —
/// an explicit pick has to move the drawn colour off it.
#[test]
fn the_players_glyph_wears_the_chosen_colour() {
    let choice = CharacterChoice {
        colour: Some(2),
        ..CharacterChoice::default()
    };
    let mut game = Game::new_with(7, DifficultyMode::Forgiving, &test_assets(), &choice)
        .expect("the shipped assets must load");
    let mut fx = Fx::new();
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
    let dist = |a: Color, b: Color| (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs();
    let at = crate::paint::painted_map_glyphs(&shapes);
    let (_, drawn) = at
        .iter()
        .find(|(g, _)| g == "@")
        .expect("the map draws the player");
    let want = hud::palette::PLAYER_CHOICES[2];
    assert!(
        dist(*drawn, want) < 0.06,
        "colour Some(2) painted {drawn:?}, which is {:.3} from PLAYER_CHOICES[2] {want:?}",
        dist(*drawn, want)
    );
    assert!(
        dist(*drawn, hud::palette::PLAYER) > 0.06,
        "a chosen colour must not still read as PLAYER"
    );
}

/// **The whole reason `colour` is an `Option` and not a reserved zero.**
/// The first swatch on the wizard's own screen is index `0`, and under a
/// one-indexed `u8` picking it stored the same value "no choice was
/// made" carries — so the player's first pick painted `PLAYER` and read
/// as the key doing nothing. Both halves are asserted: the drawn colour
/// is `PLAYER_CHOICES[0]`, *and* it is not `PLAYER`.
#[test]
fn the_first_swatch_is_not_the_player_role() {
    let choice = CharacterChoice {
        colour: Some(0),
        ..CharacterChoice::default()
    };
    let mut game = Game::new_with(7, DifficultyMode::Forgiving, &test_assets(), &choice)
        .expect("the shipped assets must load");
    let mut fx = Fx::new();
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
    let dist = |a: Color, b: Color| (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs();
    let at = crate::paint::painted_map_glyphs(&shapes);
    let (_, drawn) = at
        .iter()
        .find(|(g, _)| g == "@")
        .expect("the map draws the player");
    let want = hud::palette::PLAYER_CHOICES[0];
    assert!(
        dist(*drawn, want) < 0.06,
        "the first swatch painted {drawn:?}, which is {:.3} from PLAYER_CHOICES[0] {want:?}",
        dist(*drawn, want)
    );
    assert!(
        dist(*drawn, hud::palette::PLAYER) > 0.06,
        "picking the first swatch must not read as no choice at all"
    );
}

/// The anchor gets the same treatment, and it needs its own test: it is
/// the one map fixture that is neither a creature nor a `Structure`, so
/// nothing the player's case exercises picks it out.
///
/// The party has to step off it first. `Game::new` spawns the anchor
/// *under* the player, and the player wins a shared tile, so a fresh run
/// draws no anchor at all — asserted here rather than assumed, or the
/// test would pass against a renderer that never selected the sprite.
#[test]
fn the_anchor_sprite_stands_in_for_the_hash() {
    let mut table = SpriteTable::default();
    table.insert_still("anchor", bevy_egui::egui::TextureId::User(2));

    let (under, _) = drawn_map_with(table.clone(), 0);
    assert_eq!(
        under, 0,
        "the party stands on the anchor at spawn and hides it, so this \
         fixture proves nothing until it moves"
    );

    let (images, glyphs) = drawn_map_with(table, STEPS_OFF_THE_ANCHOR);

    assert_eq!(images, 1, "exactly one sprite, the anchor's");
    assert!(
        !glyphs.iter().any(|g| g == "#"),
        "the '#' must give way to the sprite, not sit under it: {glyphs:?}"
    );
}

/// ...and with nothing loaded the map is exactly what it was. This is
/// what makes `assets/sprites/` deletable and optional by construction.
#[test]
fn an_empty_sprite_table_leaves_the_glyph_map_alone() {
    let (images, glyphs) = drawn_map(SpriteTable::default());

    assert_eq!(images, 0, "nothing loaded must paint no texture at all");
    assert!(
        glyphs.iter().any(|g| g == "@"),
        "the player must still be drawn as a glyph: {glyphs:?}"
    );
}
