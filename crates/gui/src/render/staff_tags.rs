//! The always-on staff name tags: what tells your staff from a wild program
//! of the same species now that both stand on the same field — the
//! RimWorld rule, a name over it means it is yours. `place` is the pure
//! crowding rule, in tile units and testable without a frame; `draw_staff_tags`
//! is the measuring and painting either side of it, `alt_overlay`'s split.

use super::alt_labels::TileRect;
use super::base::{MapClickLayout, takes_tile_from};
use super::*;

/// Point size of a staff tag: under the Alt room label's 14, since a tag
/// is always on and should not outshout the map.
const TAG_SIZE: u16 = 12;

/// Pixels of dark backing around a tag's text, each side.
const TAG_PAD: f32 = 2.0;

/// A name tag over every drawn program on the staff. `entities` is what the
/// tile loop drew from, so a program out of sight carries no tag; and only
/// the body a tile actually shows is named — the tile loop's own
/// `takes_tile_from` — so a staffer under the player's `@`, or a second
/// body on a shared tile, does not float a name over someone else's glyph.
pub(super) fn draw_staff_tags(painter: &Painter, view: &MapClickLayout, entities: &[EntityView]) {
    let t = view.tile_px;
    let mut shown: std::collections::HashMap<(i32, i32), &EntityView> = Default::default();
    for ev in entities
        .iter()
        .filter(|e| !e.is_structure && e.build.is_none())
    {
        if takes_tile_from(shown.get(&ev.pos).copied()) {
            shown.insert(ev.pos, ev);
        }
    }
    // `place` orders its own claims by tile, so the map's order is harmless.
    let measured: Vec<(&EntityView, f32)> = shown
        .into_values()
        .filter(|e| e.is_tamed && !e.is_player && !e.is_companion)
        .map(|e| (e, painter.measure_ui(&e.label, TAG_SIZE).width))
        .collect();
    let h = (painter.measure_ui("Ag", TAG_SIZE).height + 2.0 * TAG_PAD) / t;
    let requests: Vec<TagRequest> = measured
        .iter()
        .map(|(e, text_w)| TagRequest {
            at: e.pos,
            w: (text_w + 2.0 * TAG_PAD) / t,
            h,
        })
        .collect();
    let origin = (
        (view.center.0 - view.half.0) as f32 + view.off.0,
        (view.center.1 - view.half.1) as f32 + view.off.1,
    );
    let bounds = TileRect {
        x: origin.0,
        y: origin.1,
        w: view.pane.w / t,
        h: view.pane.h / t,
    };
    for (rect, (ev, text_w)) in place(&requests, bounds).iter().zip(&measured) {
        let Some(rect) = rect else { continue };
        let x = view.pane.x + (rect.x - origin.0) * t;
        let y = view.pane.y + (rect.y - origin.1) * t;
        let (w, h) = (rect.w * t, rect.h * t);
        painter.rect(x, y, w, h, hud::palette::STATUS_BG);
        painter.ui(
            &ev.label,
            x + (w - text_w) / 2.0,
            y + h / 2.0 + TAG_SIZE as f32 * 0.3,
            TAG_SIZE,
            TEXT,
        );
    }
}

/// One tag to place: the program's tile, and the tag's size in tiles
/// including its backing-box padding.
pub(crate) struct TagRequest {
    pub at: (i32, i32),
    pub w: f32,
    pub h: f32,
}

/// Place every tag directly above its program, or one row higher when that
/// spot is taken, or not at all — text never clips or wraps, and a tag piled
/// on another reads as neither. Tags higher on the screen claim first (then
/// leftmost), so the lower of two neighbours is the one that moves.
/// Index-aligned with `tags`.
pub(crate) fn place(tags: &[TagRequest], bounds: TileRect) -> Vec<Option<TileRect>> {
    let mut order: Vec<usize> = (0..tags.len()).collect();
    order.sort_by_key(|&i| (tags[i].at.1, tags[i].at.0));
    let mut placed: Vec<Option<TileRect>> = vec![None; tags.len()];
    let mut boxes: Vec<TileRect> = Vec::new();
    for i in order {
        let t = &tags[i];
        // The view hands over bodies a ring or two past the pane; a tag
        // inside the pane for a program outside it names nothing on screen.
        let tile = TileRect {
            x: t.at.0 as f32,
            y: t.at.1 as f32,
            w: 1.0,
            h: 1.0,
        };
        if !bounds.contains(&tile) {
            continue;
        }
        let pick = [0.0, 1.0]
            .into_iter()
            .map(|lift| TileRect {
                x: t.at.0 as f32 + 0.5 - t.w / 2.0,
                y: t.at.1 as f32 - t.h - lift,
                w: t.w,
                h: t.h,
            })
            .find(|r| bounds.contains(r) && !boxes.iter().any(|b| b.overlaps(r)));
        if let Some(r) = pick {
            boxes.push(r);
        }
        placed[i] = pick;
    }
    placed
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDS: TileRect = TileRect {
        x: -20.0,
        y: -20.0,
        w: 40.0,
        h: 40.0,
    };

    fn tag(x: i32, y: i32, w: f32) -> TagRequest {
        TagRequest {
            at: (x, y),
            w,
            h: 0.5,
        }
    }

    #[test]
    fn a_lone_tag_sits_centred_on_the_tile_above_its_program() {
        let got = place(&[tag(3, 4, 2.0)], BOUNDS);
        assert_eq!(
            got,
            vec![Some(TileRect {
                x: 2.5,
                y: 3.5,
                w: 2.0,
                h: 0.5
            })]
        );
    }

    #[test]
    fn of_two_crowded_neighbours_the_one_further_down_moves_up_a_row() {
        // A row apart, they never contest a spot.
        let got = place(&[tag(1, 1, 3.0), tag(0, 0, 3.0)], BOUNDS);
        assert_eq!(got[0].map(|r| r.y), Some(0.5));
        assert_eq!(got[1].map(|r| r.y), Some(-0.5));
        // Side by side they do; listed right-first, so input order is not
        // what decides.
        let got = place(&[tag(1, 0, 3.0), tag(0, 0, 3.0)], BOUNDS);
        assert_eq!(got[1].map(|r| r.y), Some(-0.5), "leftmost keeps its spot");
        assert_eq!(got[0].map(|r| r.y), Some(-1.5), "its neighbour lifts a row");
    }

    #[test]
    fn a_tag_with_no_free_spot_is_dropped_not_piled_on() {
        let got = place(&[tag(0, 0, 3.0), tag(1, 0, 3.0), tag(2, 0, 3.0)], BOUNDS);
        assert!(got[0].is_some() && got[1].is_some());
        assert_eq!(got[2], None);
    }

    #[test]
    fn the_higher_row_claims_first_even_when_it_is_further_right() {
        // Tall tags, so rows contest: (5, 0)'s lift-0 box reaches down into
        // row -1, where (0, 1)'s would sit. Row order, not column, decides.
        let tall = |x, y| TagRequest {
            at: (x, y),
            w: 12.0,
            h: 1.5,
        };
        let got = place(&[tall(0, 1), tall(5, 0)], BOUNDS);
        assert_eq!(
            got[1].map(|r| r.y),
            Some(-1.5),
            "the higher row keeps its spot"
        );
        // Both of its spots overlap that box, so it is the one dropped.
        assert_eq!(got[0], None);
    }

    #[test]
    fn a_program_past_the_bottom_of_the_map_gets_no_tag() {
        let bounds = TileRect {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
        };
        // Its tag would sit wholly inside the pane, over a tile that is not it.
        assert_eq!(place(&[tag(5, 10, 2.0)], bounds), vec![None]);
        assert!(place(&[tag(5, 9, 2.0)], bounds)[0].is_some());
    }

    #[test]
    fn a_tag_that_would_leave_the_map_is_dropped() {
        let bounds = TileRect {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
        };
        assert_eq!(place(&[tag(5, 0, 2.0)], bounds), vec![None]);
    }
}
