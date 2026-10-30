use super::*;
use crate::paint::{painted_text, with_painter};
use crate::text::ui_metrics;

/// Points the app's compass at its first destination and draws the map.
/// Returns the shapes and the arrow the block should be showing.
fn shapes_with_compass(point: bool) -> (Vec<bevy_egui::egui::epaint::ClippedShape>, char) {
    let mut app = playing_app();
    let mut arrow = ' ';
    if point {
        let game = app.game.as_mut().expect("a game");
        let first = game.compass_targets().first().cloned();
        arrow = first.as_ref().map(|r| r.arrow).unwrap_or(' ');
        game.set_compass_bearing(first.map(|r| r.target));
    }
    let mut fx = Fx::new();
    let m = ui_metrics(900.0);
    let (_, shapes) = with_painter(|p| {
        draw_playing_base(&mut app, &mut fx, None, p, &m, false);
    });
    (shapes, arrow)
}

/// **The whole reason the compass stopped being a border strip.** As a
/// strip it reached into the pane, so the map had to buy a band it
/// could never draw tiles in — and buying that band only while a
/// destination was selected would have re-laid the entire tile grid on
/// the keypress that selected one. A block floating inside the pane
/// overlays tiles that are still drawn, so this compares the map's own
/// background fill between the two states and expects it not to move.
#[test]
fn the_map_is_the_same_size_whether_or_not_a_destination_is_picked() {
    let fill = |point: bool| {
        shapes_with_compass(point)
            .0
            .iter()
            .find_map(|cs| match &cs.shape {
                bevy_egui::egui::Shape::Rect(r) if r.fill.a() > 0 => Some(r.rect),
                _ => None,
            })
            .expect("the map pane paints a background")
    };
    assert_eq!(
        fill(true),
        fill(false),
        "picking a destination moved the map"
    );
}

#[test]
fn the_block_names_the_destination_and_points_home_with_none_picked() {
    let (shapes, arrow) = shapes_with_compass(true);
    let with = painted_text(&shapes).join("");
    assert!(
        with.contains(arrow),
        "the compass block never painted its {arrow:?}: {with:?}"
    );
    assert!(
        with.contains(" tiles"),
        "and the block carries the figure: {with:?}"
    );

    let home = {
        let mut app = playing_app();
        let game = app.game.as_mut().expect("a game");
        let rows = game.compass_targets();
        let home = rows
            .iter()
            .find(|r| r.target == feral_processes_engine::settlements::CompassTarget::Home);
        home.expect("a playing app has a home").arrow
    };
    let without = painted_text(&shapes_with_compass(false).0).join("");
    assert!(
        without.contains(home),
        "nothing is selected, so the block points home ({home:?}): {without:?}"
    );
}

/// **The block sits under the THREAT readout, not through it.** That
/// strip rides the pane's top border and its background quad hangs
/// `strip_clearance` down *into* the pane, so a block starting at
/// `pane.y + m.inset` would be painted straight over the lower half of
/// its glyphs — the same fault the vitals/filter-header collision was,
/// which shipped three times before anyone caught it.
#[test]
fn the_block_clears_the_threat_readout_it_sits_beneath() {
    let m = ui_metrics(900.0);
    let pane = Rect::new(0.0, 0.0, 1200.0, 600.0);
    let row = feral_processes_engine::CompassRow {
        target: feral_processes_engine::settlements::CompassTarget::Home,
        label: "home".to_string(),
        bearing: "south",
        arrow: '↓',
        distance: 219,
        visited: true,
    };
    let (block, _) = with_painter(|p| {
        hud::compass_block::draw_compass_block(pane, &row, p, &m).expect("it fits")
    });
    assert!(
        block.y >= pane.y + hud::layout::strip_inset(&m) - 0.001,
        "the block starts at {} and the THREAT strip's quad reaches {} \
         into the pane",
        block.y,
        pane.y + hud::layout::strip_inset(&m)
    );
    assert!(
        block.x + block.w <= pane.x + pane.w - m.inset + 0.001,
        "and it stays inside the right edge"
    );
}

/// A pane too narrow for the block draws nothing rather than a box
/// hanging off the edge — `strip::fitting`'s rule in the one form a
/// fixed-size box can take it.
#[test]
fn a_pane_with_no_room_for_the_block_draws_none_of_it() {
    let m = ui_metrics(900.0);
    let row = feral_processes_engine::CompassRow {
        target: feral_processes_engine::settlements::CompassTarget::Home,
        label: "a settlement with an implausibly long name".to_string(),
        bearing: "south",
        arrow: '↓',
        distance: 219,
        visited: false,
    };
    let (drawn, shapes) = with_painter(|p| {
        hud::compass_block::draw_compass_block(Rect::new(0.0, 0.0, 40.0, 600.0), &row, p, &m)
    });
    assert_eq!(drawn, None);
    assert!(
        painted_text(&shapes).join("").is_empty(),
        "a refusal must not paint half a block"
    );
}
