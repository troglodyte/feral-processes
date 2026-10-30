use super::*;

/// A creature whose species has art draws the sprite and gives up its
/// glyph — the same overdraw rule the player and the anchor already
/// hold, now exercised against a real wild program so the name drawn
/// from is `EntityView::sprite`, the one the engine actually resolved,
/// rather than a hand-built fixture.
#[test]
fn a_creature_with_art_draws_its_sprite_and_not_its_glyph() {
    let game = Game::new(7, DifficultyMode::Forgiving, &test_assets())
        .expect("the shipped assets must load");
    let (mut game, creature) = a_wild_creature(game);
    let name = creature
        .sprite
        .clone()
        .expect("a creature always resolves a sprite name");
    let mut table = SpriteTable::default();
    table.insert_still(name, bevy_egui::egui::TextureId::User(6));

    let (images, glyphs) = drawn_map_centered_on(&mut game, table, creature.pos);

    assert!(images >= 1, "the species' art must paint a sprite");
    assert!(
        !glyphs.iter().any(|g| *g == creature.glyph.to_string()),
        "the glyph must give way to the sprite, not sit under it: {glyphs:?}"
    );
}

/// The rung under it: a species with nothing in the table still draws
/// its glyph, exactly as before this task.
#[test]
fn a_creature_with_no_art_draws_its_glyph() {
    let game = Game::new(7, DifficultyMode::Forgiving, &test_assets())
        .expect("the shipped assets must load");
    let (mut game, creature) = a_wild_creature(game);

    let (images, glyphs) = drawn_map_centered_on(&mut game, SpriteTable::default(), creature.pos);

    assert_eq!(images, 0, "an empty table must paint no texture");
    assert!(
        glyphs.iter().any(|g| *g == creature.glyph.to_string()),
        "the glyph is what is left when no sprite is loaded: {glyphs:?}"
    );
}

/// A settlement carries none of `stands_inside_the_base`'s tags —
/// `Caravan`/`Structure`/`BuildSite`/`Tamed` — so it falls through
/// `Game::view_entities_at` to the surface map and lands in
/// `draw_surface_map`'s `actor` slot exactly like a nest. Nothing states
/// that anywhere; this is the assertion.
///
/// Reached through the save round trip rather than a fixture that spawns
/// a component directly — `Game`'s `world` is private from out here and
/// deliberately stays that way, `attention_drives_all_three_markers`'s
/// reason. The tile east of spawn is cleared of anything else the fresh
/// seed put there, or the actor slot could resolve to whichever entity
/// iteration happens to visit last rather than to the settlement this
/// test is about.
///
/// `SettlementKind::Mainframe` (glyph `'M'`), not `Server` (glyph
/// `'s'`) — `'s'` is also the Sprite species' glyph
/// (`assets/species/sprite.ron`), and this seed's draw box already
/// carries wild Sprites before the fixture adds anything, so a `Server`
/// settlement made this assertion pass with the settlement entity
/// absent entirely. No shipped species uses `'M'` (verified against
/// every file under `assets/species/`), so this glyph can only have
/// come from the settlement.
#[test]
fn a_settlement_draws_its_glyph_on_the_surface_map() {
    let (mut game, target) = game_with_settlement_at((1, 0), false);

    // Not asserted against `SpriteTable::default()`: an empty table
    // paints no texture no matter what the fixture spawns, which is
    // what made this line read as coverage while proving nothing —
    // `a_creature_with_no_art_draws_its_glyph` above already owns that
    // fact.
    let (_images, glyphs) = drawn_map_centered_on(&mut game, SpriteTable::default(), target);

    assert!(
        glyphs.iter().any(|g| *g == "M"),
        "a Mainframe settlement's glyph never reached the surface map: {glyphs:?}"
    );
}

/// The fixture of the test above, `offset` from the player. `seen` writes
/// the tile into the saved `SeenTiles`, the way walking past it would
/// have, so the settlement can be remembered without being in sight.
fn game_with_settlement_at(offset: (i32, i32), seen: bool) -> (Game, (i32, i32)) {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets())
        .expect("the shipped assets must load");
    let path = std::env::temp_dir().join(format!(
        "fp_gui_settlement_glyph_{}_{}_{}.sav",
        std::process::id(),
        offset.0,
        seen
    ));
    game.save(&path).unwrap();
    let mut data = feral_processes_engine::save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    let target = (px + offset.0, py + offset.1);
    data.creatures.retain(|c| c.position != target);
    data.nests.retain(|n| n.position != target);
    data.link_sites.retain(|&site| site != target);
    if seen {
        data.seen_tiles.mark(target);
    }
    let key = feral_processes_engine::settlements::SettlementKey { rx: 0, ry: 0 };
    data.settlements.0.insert(
        key,
        feral_processes_engine::resources::KnownSettlement {
            tile: target,
            def: feral_processes_engine::settlements::SettlementDef {
                id: "test_settlement".to_string(),
                name: "Test Settlement".to_string(),
                blurb: "A settlement placed for a test.".to_string(),
                kind: feral_processes_engine::settlements::SettlementKind::Mainframe,
                specialty: feral_processes_engine::settlements::Specialty::Materials,
                temperament: feral_processes_engine::settlements::Temperament::Open,
            },
            visited: false,
        },
    );
    feral_processes_engine::save::save_to_file(&path, &data).unwrap();
    let game = Game::load(&path, &test_assets()).unwrap();
    let _ = std::fs::remove_file(&path);
    (game, target)
}

/// Out of sight and never seen, a settlement is black ground.
#[test]
fn an_unseen_settlement_is_not_drawn() {
    let (mut game, target) = game_with_settlement_at((9, 0), false);

    let (_, glyphs) = drawn_map_centered_on(&mut game, SpriteTable::default(), target);

    assert!(!glyphs.iter().any(|g| *g == "M"), "{glyphs:?}");
}

/// Out of sight but seen once, a settlement stays on the map: a landmark.
#[test]
fn a_remembered_settlement_is_still_drawn() {
    let (mut game, target) = game_with_settlement_at((9, 0), true);

    let (_, glyphs) = drawn_map_centered_on(&mut game, SpriteTable::default(), target);

    assert!(glyphs.iter().any(|g| *g == "M"), "{glyphs:?}");
}

/// A hostile past the circle draws no glyph, and so is not handed back to
/// the Threat count and the chase lookup that read the drawn Vec.
#[test]
fn a_hostile_outside_sight_is_neither_drawn_nor_counted() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets())
        .expect("the shipped assets must load");
    let at = game.player_status().position;
    let far = game
        .view_entities(64, 64)
        .into_iter()
        .find(|e| e.is_hostile && (e.pos.0 - at.0).abs().max((e.pos.1 - at.1).abs()) > 8)
        .expect("seed 7 has a wild creature past the circle");
    let mut fx = Fx::new();
    let (tile_px, glyph_px) = crate::text::map_cell(1);
    let mut drawn = Vec::new();
    let (_, shapes) = with_sprites(SpriteTable::default(), |p| {
        let status = game.player_status();
        drawn = draw_surface_map(
            &mut game,
            &mut fx,
            p,
            Rect::new(0.0, 0.0, 800.0, 600.0),
            tile_px,
            glyph_px,
            &status,
            None,
            None,
            far.pos,
            false,
        );
    });

    assert!(drawn.iter().all(|e| e.entity != far.entity));
    assert!(
        !painted_text(&shapes)
            .iter()
            .any(|g| *g == far.glyph.to_string()),
        "the hidden creature's glyph was painted"
    );
}
