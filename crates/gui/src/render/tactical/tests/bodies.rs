use super::*;

/// A game standing in a tactical fight, opened the way the router opens
/// one.
/// The free cell nearest the acting body. The camera is held on that
/// body, so the board's first free cell in reading order can sit off
/// the pane entirely and a mark placed there is never drawn.
fn free_cell_in_view(view: &TacticalView) -> Option<(i32, i32)> {
    let occupied: std::collections::HashSet<(i32, i32)> =
        view.bodies.iter().map(|b| b.cell).collect();
    let acting = view.order[view.active?].entity;
    let at = view.bodies.iter().find(|b| b.entity == acting)?.cell;
    view.board
        .cells()
        .filter(|(cell, kind)| *kind != BattleCell::Blocked && !occupied.contains(cell))
        .map(|(cell, _)| cell)
        .min_by_key(|c| ((c.0 - at.0).abs().max((c.1 - at.1).abs()), c.1, c.0))
}

/// Advances the wild side one turn at a time until the board hands the
/// turn to the player — `crates/app-core/src/tests/tactical.rs`'s
/// `wait_for_the_player`, copied rather than shared across crates.
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

/// `fighting()`, with the player already emulating a known image —
/// todo #100 Task 6. `resources::EmulationImages` has no public writer
/// short of a real extraction, unlike `KnownRoutines` (`CharacterChoice::
/// routine`, `abilities::install_starter`'s own door), so this writes
/// the image straight into `save::SaveData::emulation_images` and
/// reloads — `savetool`'s own dump-edit-pack shape, entirely through
/// the engine's public `save::` functions, never `Game::world`.
fn emulating_fight() -> Game {
    use feral_processes_engine::battle::SpecialTargeting;
    use feral_processes_engine::save;
    use feral_processes_engine::{CharacterChoice, DifficultyMode};

    for seed in 0..200u32 {
        let choice = CharacterChoice::default();
        let mut game = Game::new_with(seed, DifficultyMode::Forgiving, &assets(), &choice)
            .expect("the assets parse");
        let species = game
            .species_defs()
            .into_iter()
            .find(|d| !d.is_hybrid())
            .expect("at least one species ships")
            .id;

        let tmp = std::env::temp_dir().join(format!(
            "fp_gui_emulating_fight_{}_{seed}.bin",
            std::process::id()
        ));
        game.save(&tmp).expect("the fixture must save");
        let mut data = save::load_from_file(&tmp).expect("the fixture save must load back");
        data.emulation_images.push(species.clone());
        data.player.routines.truncate(1);
        data.player.routines.push("emulate".to_string());
        save::save_to_file(&tmp, &data).expect("the edited save must write back");
        let mut game = Game::load(&tmp, &assets()).expect("the edited save must load");
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
        return game;
    }
    panic!("no seed under 200 put a lone wild program next to the player");
}

/// The grid draws a mark for every body standing on it. Without this a
/// silently-empty battle map passes every census in the file.
#[test]
fn every_body_on_the_board_is_drawn() {
    let mut game = fighting();
    let view = game.tactical_view().expect("the fight is open");
    let mut fx = Fx::new();
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    let painted = painted_text(&shapes);
    for body in &view.bodies {
        assert!(
            painted.iter().any(|t| t == &body.glyph.to_string()),
            "{:?} was not drawn on the battle map",
            body.glyph
        );
    }
}

/// The overdraw trap, `every_body_on_the_board_is_drawn`'s companion,
/// for an emulating player's own body (todo #100 Task 6): the image's
/// sprite must draw, and neither the player's `@` nor the image's own
/// glyph as text may still be underneath it — plus the player-colour
/// outline the con read and the boss magenta both leave the player
/// findable through.
#[test]
fn the_emulating_players_body_draws_its_form_and_outline() {
    use crate::paint::{SpriteTable, painted_rect_stroke_count, with_sprites};

    let mut game = emulating_fight();
    let view = game.tactical_view().expect("the fight is open");
    let player = view
        .bodies
        .iter()
        .find(|b| b.is_player)
        .expect("the player is on the board");
    let form = player.form.clone().expect("the fixture must be emulating");
    let mut table = SpriteTable::default();
    table.insert_still(
        form.sprite
            .clone()
            .expect("the fixture's image has a sprite name"),
        bevy_egui::egui::TextureId::User(9),
    );

    let mut fx = Fx::new();
    let (_, shapes) = with_sprites(table, |p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    let images = crate::paint::painted_images(&shapes);
    let painted = painted_text(&shapes);

    assert_eq!(images.len(), 1, "exactly one sprite, the image's own");
    assert!(
        !painted.iter().any(|t| t == "@"),
        "the '@' must give way to the form, not sit under it: {painted:?}"
    );
    let form_glyph = form.glyph.to_string();
    assert!(
        !painted.iter().any(|t| t == &form_glyph),
        "the form's own glyph must not draw as text under its sprite \
         either, or the sprite has drawn beside it rather than in place \
         of it: {painted:?}"
    );

    let outline =
        super::super::super::player_look_color(player.look.as_ref().and_then(|l| l.colour));
    assert!(
        painted_rect_stroke_count(&shapes, outline) > 0,
        "the emulating player's own body must keep an outline in their \
         colour, since the hue is spent on boss magenta"
    );
}

/// The board's view of `fighting()` with the player wearing `look` —
/// written onto the view rather than the game, since a drawing has no
/// writer short of character creation and the renderer reads only the
/// view.
fn player_wearing(
    sprite: &str,
    icon: Option<feral_processes_engine::PlayerIcon>,
) -> feral_processes_engine::tactical::view::TacticalView {
    let mut view = fighting().tactical_view().expect("the fight is open");
    let player = view
        .bodies
        .iter_mut()
        .find(|b| b.is_player)
        .expect("the player is on the board");
    let look = player.look.as_mut().expect("the player carries a look");
    look.sprite = sprite.to_string();
    look.icon = icon;
    view
}

fn a_drawing() -> feral_processes_engine::PlayerIcon {
    let mut canvas = feral_processes_engine::icon::Canvas::new(feral_processes_engine::ICON_SIZE);
    canvas.set(3, 4, 7);
    feral_processes_engine::PlayerIcon::new(
        vec![canvas],
        feral_processes_engine::DEFAULT_SPRITE_FRAME_MS,
    )
    .unwrap()
}

/// The player's own drawing stands on their body on this board as it
/// does on the map — top rung over a named sprite, in place of the `@`
/// rather than over it, and untinted for `render/base.rs`'s reason.
#[test]
fn the_players_drawing_stands_in_for_the_at_sign() {
    use crate::paint::{SpriteTable, with_sprites};

    let view = player_wearing("hero", Some(a_drawing()));
    let mut table = SpriteTable::default();
    table.insert_still(
        crate::sprites::DRAWN_ICON_KEY,
        bevy_egui::egui::TextureId::User(9),
    );
    table.insert_still("hero", bevy_egui::egui::TextureId::User(4));

    let mut fx = Fx::new();
    let (_, shapes) = with_sprites(table, |p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    let images = crate::paint::painted_images(&shapes);
    let painted = painted_text(&shapes);

    let drawing: Vec<_> = images
        .iter()
        .filter(|(id, _, _)| *id == bevy_egui::egui::TextureId::User(9))
        .collect();
    assert_eq!(drawing.len(), 1, "the drawing must be painted once");
    assert!(
        !images
            .iter()
            .any(|(id, _, _)| *id == bevy_egui::egui::TextureId::User(4)),
        "the drawing must win the body from the named sprite"
    );
    let tint = drawing[0].2;
    assert!(
        tint.r() == tint.g() && tint.g() == tint.b(),
        "a drawing is drawn untinted, not in a hue: {tint:?}"
    );
    assert!(
        !painted.iter().any(|t| t == "@"),
        "the '@' must give way to the drawing, not sit under it: {painted:?}"
    );
}

/// With no drawing, the player's named sprite is the next rung, as on
/// the map.
#[test]
fn the_players_named_sprite_stands_in_without_a_drawing() {
    use crate::paint::{SpriteTable, with_sprites};

    let view = player_wearing("hero", None);
    let mut table = SpriteTable::default();
    table.insert_still("hero", bevy_egui::egui::TextureId::User(4));

    let mut fx = Fx::new();
    let (_, shapes) = with_sprites(table, |p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    let images = crate::paint::painted_images(&shapes);
    let painted = painted_text(&shapes);

    assert!(
        images
            .iter()
            .any(|(id, _, _)| *id == bevy_egui::egui::TextureId::User(4)),
        "the player's named sprite must draw on the board"
    );
    assert!(
        !painted.iter().any(|t| t == "@"),
        "the '@' must give way to the sprite: {painted:?}"
    );
}

/// A finished tactical fight ends in a popup over its own board, and
/// never on the group model's battle screen — the bug this screen fixed.
#[test]
fn a_finished_fight_draws_its_results_over_the_board() {
    let mut game = fighting();
    for _ in 0..4000 {
        if !game.has_active_battle() {
            break;
        }
        game.tactical_auto_beat();
    }
    assert!(!game.has_active_battle(), "the fight never resolved");
    assert!(
        game.tactical_result_view().is_some(),
        "the fight left no board"
    );
    let outcomes = game.battle_outcomes();
    let first = outcomes
        .first()
        .expect("the fight reported nothing")
        .text
        .clone();
    let board = game.tactical_result_view().expect("checked above").board;
    let lighting = Lighting::of(&board);
    let open_fills: Vec<Color> = board
        .cells()
        .filter(|&(_, kind)| kind == BattleCell::Open)
        .map(|(cell, kind)| lighting.at(cell).apply(cell_color(kind)))
        .collect();

    let mut app = crate::render::test_support::playing_app_around(game);
    app.mode = Mode::TacticalResult;
    let mut fx = Fx::new();
    let (_, shapes) = with_painter(|p| crate::render::draw(&mut app, &mut fx, p, false, false));
    let painted = painted_text(&shapes);

    assert!(
        painted.iter().any(|t| t == "Fight over"),
        "no results popup was drawn"
    );
    assert!(
        painted.iter().any(|t| t.contains(first.as_str())),
        "the popup does not list the fight's first result, {first:?}"
    );
    // The ground's own fill and not a body's glyph: the player's `@` is
    // drawn by the surface map too, so a glyph passes with no board.
    assert!(
        open_fills
            .iter()
            .any(|&fill| painted_rect_fill_count(&shapes, fill) > 0),
        "the board the fight ended on is not under the popup"
    );
    assert!(
        !painted.iter().any(|t| t.starts_with("Hostile programs")),
        "the fight ended on the group model's battle screen"
    );
}

/// A cloaked body fades, and an uncloaked one does not.
///
/// Read off the **alpha** rather than the colour: the map dims every
/// glyph it draws by a vignette and a per-tile shade, so rgb moves for
/// reasons that have nothing to do with a cloak — and alpha is the one
/// channel `ConRead::glyph_ink` carries through untouched, which is what
/// makes the same multiply fade a sprite's tint too.
#[test]
fn a_cloaked_body_draws_faded_and_an_uncloaked_one_does_not() {
    let mut game = fighting();
    let view = game.tactical_view().expect("the fight is open");
    let subject = view
        .bodies
        .iter()
        .find(|b| b.is_hostile)
        .cloned()
        .expect("a hostile stands on the board");

    let alpha_of = |cloaked: bool| {
        let mut view = view.clone();
        for body in &mut view.bodies {
            if body.entity == subject.entity {
                body.cloaked = cloaked;
            }
        }
        let mut fx = Fx::new();
        let (_, shapes) = with_painter(|p| {
            draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
        });
        crate::paint::painted_map_glyphs(&shapes)
            .into_iter()
            .find(|(text, _)| text == &subject.glyph.to_string())
            .map(|(_, c)| c.a)
            .expect("the body was not drawn at all")
    };

    let plain = alpha_of(false);
    let faded = alpha_of(true);
    assert!(plain > 0.9, "an uncloaked body drew faded already: {plain}");
    assert!(
        (faded - plain * FADED_ALPHA).abs() < 0.02,
        "a cloaked body drew at alpha {faded}, not {} — one multiply at the draw site",
        plain * FADED_ALPHA
    );
}

/// A Hallucination's fake draws the invoker's own glyph at `FADED_ALPHA`
/// — one constant with the cloak fade above, because a decoy means the
/// same thing a cloaked body does: not really there.
///
/// **(M)** Dropping the `* FADED_ALPHA` multiply at the decoy draw site
/// makes this fail the same way removing the cloak's own multiply would
/// fail the test above.
#[test]
fn a_decoy_draws_the_invokers_glyph_faded() {
    use feral_processes_engine::components::GlyphColor;
    use feral_processes_engine::tactical::view::DecoyView;

    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    let free = free_cell_in_view(&view).expect("the board has a free cell for a decoy");
    view.decoys = vec![DecoyView {
        cell: free,
        glyph: 'Q',
        color: GlyphColor::Red,
        of_player: false,
    }];
    let mut fx = Fx::new();
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    let (_, color) = crate::paint::painted_map_glyphs(&shapes)
        .into_iter()
        .find(|(text, _)| text == "Q")
        .expect("the decoy was not drawn at all");
    assert!(
        (color.a - FADED_ALPHA).abs() < 0.02,
        "a decoy drew at alpha {}, not {FADED_ALPHA}",
        color.a
    );
}

/// Where a body fell draws a grey `x`, whatever side it was on.
#[test]
fn a_fallen_body_leaves_a_grey_x() {
    use feral_processes_engine::tactical::Fallen;

    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    let free = free_cell_in_view(&view).expect("the board has a free cell for a mark");
    view.fallen = vec![Fallen {
        cell: free,
        footprint: 1,
        raise: None,
    }];
    let mut fx = Fx::new();
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    let (_, color) = crate::paint::painted_map_glyphs(&shapes)
        .into_iter()
        .find(|(text, _)| text == "x")
        .expect("the fallen mark was not drawn at all");
    assert_eq!(color, palette::FALLEN);
}

/// A raised body draws in `palette::RESPAWNED` instead of its authored
/// hue — `draw_body`'s replacement of `authored` rather than a colour
/// drawn beside it.
///
/// **A party-side body with no sprite**, deliberately: a hostile's own
/// difficulty rung is a *con read*, which `ConRead::of` puts on the
/// glyph itself whenever no sprite drew — unrelated to Respawn, and the
/// design doc says it stays "unchanged" under a hostile raised body. A
/// `None` difficulty and no sprite is what isolates this test to the
/// one path Respawn actually changes.
#[test]
fn a_raised_body_draws_in_the_respawned_tint() {
    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    let template = view
        .bodies
        .iter()
        .find(|b| !b.is_player)
        .expect("a hostile stands on the board")
        .clone();
    let raised = TacticalBody {
        respawned: true,
        is_hostile: false,
        difficulty: None,
        sprite: None,
        ..template
    };
    let glyph = raised.glyph.to_string();
    view.bodies = vec![raised];
    let mut fx = Fx::new();
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    let (_, color) = crate::paint::painted_map_glyphs(&shapes)
        .into_iter()
        .find(|(text, _)| text == &glyph)
        .expect("the raised body was not drawn at all");
    assert_eq!(color, palette::RESPAWNED);
}
