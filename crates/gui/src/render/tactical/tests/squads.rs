use super::*;

/// A game standing in a tactical fight around one freshly-folded squad
/// and nothing else — five of a kind is the shipped formation's own
/// threshold (`tuning::FORMATIONS[0].members`), so the whole pack folds
/// and leaves no leftover single of the same species to confuse a test
/// about which body's glyph or sprite it is reading.
///
/// Staged through `arena::stage` rather than the `fighting()` seed
/// search above: nothing about a squad depends on being found on the
/// zone surface, and a scenario asks for exactly the composition a test
/// needs instead of hoping a seed rolls one.
fn squad_fighting() -> Game {
    use feral_processes_engine::arena::{CombatModel, OpponentSpec, Scenario};

    let species = Game::new(0, DifficultyMode::Forgiving, &assets())
        .expect("the assets parse")
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .expect("at least one species ships")
        .id;
    let scenario = Scenario {
        opponents: vec![OpponentSpec { species, count: 5 }],
        model: CombatModel::Tactical,
        ..Scenario::default()
    };
    feral_processes_engine::arena::stage(&scenario, &assets(), 0, false, CombatModel::Tactical)
        .expect("the scenario stages")
        .game
}

/// `expand_to_footprint` turns one anchor into its whole block, and
/// leaves an ordinary body's single cell alone.
#[test]
fn expand_to_footprint_turns_an_anchor_into_its_whole_block() {
    let mut block = expand_to_footprint(&[(3, 4)], 2);
    block.sort();
    assert_eq!(block, vec![(3, 4), (3, 5), (4, 4), (4, 5)]);

    assert_eq!(
        expand_to_footprint(&[(3, 4), (9, 9)], 1),
        vec![(3, 4), (9, 9)],
        "footprint 1 must leave a plain list of anchors untouched"
    );
}

/// `draw_cell_field`'s own boundary logic generalises to a
/// footprint-shaped field with no change: a solid 2x2 block reads as
/// **one region**, an outer perimeter of eight unit edges and no seam
/// drawn between its own four cells.
#[test]
fn a_footprint_shaped_field_reads_as_one_region() {
    use crate::paint::{painted_line_count, painted_rect_fill_count};

    let field = expand_to_footprint(&[(0, 0)], 2);
    let tile_px = 32.0;
    let (_, shapes) = with_painter(|p| {
        for dy in 0..2 {
            for dx in 0..2 {
                let cell = (dx, dy);
                draw_cell_field(
                    p,
                    &field,
                    &field,
                    cell,
                    (100.0 + dx as f32 * tile_px, 200.0 + dy as f32 * tile_px),
                    tile_px,
                    palette::PLAN,
                );
            }
        }
    });
    let wash = Color::new(
        palette::PLAN.r,
        palette::PLAN.g,
        palette::PLAN.b,
        REACH_WASH_ALPHA,
    );
    assert_eq!(
        painted_rect_fill_count(&shapes, wash),
        4,
        "all four cells of the block must wash, one rect each"
    );
    assert_eq!(
        painted_line_count(&shapes),
        8,
        "a 2x2 block's own outer perimeter is eight unit edges, with none \
         drawn between its own four cells"
    );
}

/// A squad's own reach wash spans its whole footprint at a candidate
/// anchor, not the anchor cell alone — forced down to the squad's own
/// single reachable cell ("its own included") so the fill count is
/// exact rather than however many cells a live movement allowance
/// happens to reach.
#[test]
fn a_squads_reach_wash_covers_its_whole_footprint() {
    use crate::paint::painted_rect_fill_count;

    let mut game = squad_fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    let squad_index = view
        .bodies
        .iter()
        .position(|b| b.squad.is_some())
        .expect("5 of a kind must fold into a squad");
    let rung = view
        .order
        .iter()
        .position(|r| r.entity == view.bodies[squad_index].entity)
        .expect("the squad must hold a rung in the turn order");
    view.active = Some(rung);
    view.reachable = vec![view.bodies[squad_index].cell];
    view.covered = Vec::new();
    view.provoking = Vec::new();

    let mut fx = Fx::new();
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    let wash = Color::new(
        palette::PLAN.r,
        palette::PLAN.g,
        palette::PLAN.b,
        REACH_WASH_ALPHA,
    );
    assert_eq!(
        painted_rect_fill_count(&shapes, wash),
        4,
        "one reachable anchor on a 2x2 squad must wash all four of its cells"
    );
}

/// `squad_mark_rect`'s own geometry, pinned the way `marks.rs`'s other
/// four are: no `Painter` needed to hold that it sits in the
/// **bottom-right** corner of its footprint — not the top-right a stale
/// spec asked for, which the in-cover mark owns now — and lifts clear
/// above the HP bar the way the top-corner marks drop below the rarity
/// bar.
#[test]
fn squad_mark_rect_sits_bottom_right_and_lifts_above_the_hp_bar() {
    use crate::render::marks::{squad_mark_rect, tactical_hp_bar_rect};

    let (px, py, cell_px) = (100.0, 200.0, 64.0);
    let mark = squad_mark_rect(px, py, cell_px);
    let bar = tactical_hp_bar_rect(px, py, cell_px);

    assert!(
        mark.x + mark.w > px + cell_px / 2.0,
        "the mark must sit in the right half of the footprint: {mark:?}"
    );
    assert!(
        mark.y + mark.h > py + cell_px / 2.0,
        "the mark must sit in the bottom half of the footprint: {mark:?}"
    );
    assert!(
        mark.x + mark.w <= px + cell_px,
        "the mark must not overhang the footprint's own right edge: {mark:?}"
    );
    assert!(
        mark.y + mark.h <= bar.y,
        "the mark must lift clear above the HP bar, not sit on top of it: \
         mark bottom {} vs bar top {}",
        mark.y + mark.h,
        bar.y
    );
}

/// A squad's own body draws over its whole footprint, and the sprite
/// substitutes for the glyph there exactly as it does at one cell — the
/// overdraw trap unchanged at 2x2.
#[test]
fn a_squad_draws_its_sprite_over_the_whole_footprint_never_beside_its_glyph() {
    use crate::paint::{SpriteTable, painted_images, with_sprites};

    let mut game = squad_fighting();
    let view = game.tactical_view().expect("the fight is open");
    let squad = view
        .bodies
        .iter()
        .find(|b| b.squad.is_some())
        .expect("5 of a kind must fold into a squad");
    assert_eq!(squad.footprint, 2, "fixture: the shipped formation is 2x2");
    let name = squad
        .sprite
        .clone()
        .expect("a body with a Creature always resolves a sprite name");

    let mut table = SpriteTable::default();
    table.insert_still(name, bevy_egui::egui::TextureId::User(9));

    let mut fx = Fx::new();
    let (_, shapes) = with_sprites(table, |p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });

    let mesh = painted_images(&shapes)
        .into_iter()
        .find(|(id, _, _)| *id == bevy_egui::egui::TextureId::User(9))
        .expect("the squad's sprite must be painted");
    assert!(
        (mesh.1.width() - 48.0).abs() < 0.5,
        "a footprint-2 body at glyph_px 24 must draw a 48px sprite, not one cell's: {:?}",
        mesh.1
    );
    assert!(
        !painted_text(&shapes)
            .iter()
            .any(|t| t == &squad.glyph.to_string()),
        "the sprite must substitute for the glyph, not sit beside it"
    );
}

/// A squad also standing in cover wears both marks at once — the
/// bottom-right one this feature adds and the top-right one the battle
/// map already draws — because the two corners are independent and
/// nothing about folding five bodies into one should spend the other
/// mark's channel.
#[test]
fn a_squad_in_cover_draws_both_its_own_mark_and_the_cover_mark() {
    use crate::paint::painted_poly_points;

    let mut game = squad_fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    let squad_index = view
        .bodies
        .iter()
        .position(|b| b.squad.is_some())
        .expect("5 of a kind must fold into a squad");
    view.bodies[squad_index].in_cover = true;
    let mark = view.bodies[squad_index]
        .squad
        .as_ref()
        .unwrap()
        .mark
        .to_string();

    let mut fx = Fx::new();
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });

    let cover = painted_poly_points(&shapes, palette::HEALTHY);
    assert!(
        !cover.is_empty(),
        "the cover mark must still draw for a squad standing in cover"
    );
    // Told apart by *position* rather than by which draws at all — both
    // corners are free to carry a mark at once. The sun's lowest y is
    // the top of its dome, near the footprint's top edge
    // (`sunrise`'s own geometry); the squad's own mark must sit
    // strictly below that, in the corner nothing else claims.
    let cover_top = cover
        .iter()
        .flat_map(|pts| pts.iter().map(|&(_, y)| y))
        .fold(f32::MAX, f32::min);
    let boxes = crate::paint::painted_text_boxes(&shapes);
    let (_, _, mark_box) = boxes
        .iter()
        .find(|(_, text, _)| text == &mark)
        .unwrap_or_else(|| panic!("the squad's own mark {mark:?} was not drawn: {boxes:?}"));
    assert!(
        mark_box.y > cover_top,
        "the squad's mark (y={}) must sit below the cover mark's own corner (y={cover_top})",
        mark_box.y
    );
}

/// The turn arrow and the aim cursor both span a squad's whole
/// footprint rather than the one cell an ordinary body wears them on.
#[test]
fn the_turn_arrow_and_aim_cursor_span_a_squads_whole_footprint() {
    use crate::paint::{painted_poly_points, painted_rect_stroke_boxes};

    let mut game = squad_fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    let (squad_entity, squad_cell, squad_hostile) = {
        let squad = view
            .bodies
            .iter()
            .find(|b| b.squad.is_some())
            .expect("5 of a kind must fold into a squad");
        (squad.entity, squad.cell, squad.is_hostile)
    };
    let rung = view
        .order
        .iter()
        .position(|r| r.entity == squad_entity)
        .expect("the squad must hold a rung in the turn order");
    view.active = Some(rung);

    let mut fx = Fx::new();
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    let arrow_color = if squad_hostile {
        palette::THREAT
    } else {
        palette::PLAN
    };
    // `arrows()` is the arrow's own shape filter (base points level,
    // apex below and centred) — a bare 3-point poly is not enough,
    // since a squad's own con earmark can share the arrow's hue.
    let found = arrows(&painted_poly_points(&shapes, arrow_color));
    let (_, width, _) = found
        .first()
        .expect("the turn arrow must draw over the acting squad");
    let expected = 64.0 * TURN_ARROW_WIDTH;
    assert!(
        (*width - expected).abs() < 0.5,
        "a 2x2 footprint must draw a turn arrow {expected}px wide, not {width}"
    );

    let mut fx = Fx::new();
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(
            &view,
            &Overlay {
                cursor: Some(squad_cell),
                ..Default::default()
            },
            &mut fx,
            p,
            pane(),
            32.0,
            24,
        )
    });
    let cursor = painted_rect_stroke_boxes(&shapes, palette::EMPHASIS);
    let boxed = cursor
        .iter()
        .find(|r| r.width() > 32.0)
        .expect("the aim cursor must span the squad's whole footprint, not one cell");
    assert!(
        (boxed.width() - 63.0).abs() < 1.0,
        "a 2x2 footprint at tile_px 32 must draw a ~63px cursor: {boxed:?}"
    );
}
