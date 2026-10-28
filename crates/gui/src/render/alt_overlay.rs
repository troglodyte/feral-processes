//! Alt's labels drawn on the map: the pure placement is `alt_labels`, this
//! is the measuring and painting either side of it.

use super::alt_labels::{LabelKind, LabelRequest, LayoutInput, Placement, TileRect, layout};
use super::base::{MapClickLayout, RINGS};
use super::*;
use feral_processes_engine::views::{FarmTileView, RoomView};

/// Point size of a room's Alt label: small enough that a four-cell room
/// holds "Fine dormitory" at the default zoom.
const ROOM_LABEL_SIZE: u16 = 14;

/// Pixels of dark backing around a machine's Alt label, each side.
const ALT_LABEL_PAD: f32 = 2.0;

/// The line from a pushed-out machine label back to its machine.
const ALT_LEADER: Color = Color { a: 0.45, ..TEXT };

/// Alt's labels: room names, then what each machine and sown plot makes,
/// placed by `alt_labels::layout` so that none covers another.
pub(super) fn draw_alt_labels(
    painter: &Painter,
    view: &MapClickLayout,
    rooms: &[RoomView],
    entities: &[EntityView],
    farm_tiles: &[FarmTileView],
    structure_floor: &[Vec<bool>],
) {
    let (t, size) = (view.tile_px, ROOM_LABEL_SIZE);
    // One label per plot, on its first cell in (y, x).
    let mut cells: Vec<_> = farm_tiles
        .iter()
        .filter_map(|f| Some(((f.pos.1, f.pos.0), f.plot, f.crop_name.as_deref()?)))
        .collect();
    cells.sort_unstable_by_key(|c| c.0);
    let mut seen = std::collections::HashSet::new();
    let mut makers: Vec<((i32, i32), &str)> = entities
        .iter()
        .filter(|e| e.is_structure)
        .filter_map(|e| Some(((e.pos.1, e.pos.0), e.making.as_deref()?)))
        .chain(
            cells
                .into_iter()
                .filter(|c| seen.insert(c.1))
                .map(|c| (c.0, c.2)),
        )
        .collect();
    makers.sort_by_key(|m| m.0);
    let mut texts = Vec::new();
    let mut requests = Vec::new();
    for room in rooms {
        let text = format!("{} {}", room.band.label(), room.role);
        requests.push(LabelRequest {
            kind: LabelKind::Room {
                cells: &room.cells,
                centre: room.label_at,
            },
            w: painter.measure_ui(&text, size).width / t,
            h: 1.0,
        });
        texts.push(text);
    }
    for ((y, x), text) in makers {
        let dims = painter.measure_ui(text, size);
        requests.push(LabelRequest {
            kind: LabelKind::Machine { at: (x, y) },
            w: (dims.width + 2.0 * ALT_LABEL_PAD) / t,
            h: (dims.height + 2.0 * ALT_LABEL_PAD) / t,
        });
        texts.push(text.to_string());
    }
    let (hw, hh) = (view.half.0 + RINGS, view.half.1 + RINGS);
    let soft = entities
        .iter()
        .filter(|e| e.is_structure)
        .map(|e| e.pos)
        .chain(structure_floor.iter().enumerate().flat_map(|(ry, row)| {
            row.iter().enumerate().filter(|c| *c.1).map(move |(rx, _)| {
                (
                    rx as i32 - hw + view.center.0,
                    ry as i32 - hh + view.center.1,
                )
            })
        }))
        .collect();
    let origin = (
        (view.center.0 - view.half.0) as f32 + view.off.0,
        (view.center.1 - view.half.1) as f32 + view.off.1,
    );
    let placements = layout(&LayoutInput {
        labels: &requests,
        soft: &soft,
        bounds: TileRect {
            x: origin.0,
            y: origin.1,
            w: view.pane.w / t,
            h: view.pane.h / t,
        },
    });
    let px = |(x, y): (f32, f32)| {
        (
            view.pane.x + (x - origin.0) * t,
            view.pane.y + (y - origin.1) * t,
        )
    };
    let placed: Vec<_> = placements
        .iter()
        .zip(&requests)
        .zip(&texts)
        .filter_map(|((p, req), text)| match p {
            Placement::At { rect, leader } => Some((rect, leader, req, text)),
            Placement::Dropped => None,
        })
        .collect();
    for (_, leader, _, _) in &placed {
        if let Some((a, b)) = leader {
            let ((x1, y1), (x2, y2)) = (px(*a), px(*b));
            painter.line(x1, y1, x2, y2, 1.0, ALT_LEADER);
        }
    }
    for (rect, _, req, _) in &placed {
        if matches!(req.kind, LabelKind::Machine { .. }) {
            let (x, y) = px((rect.x, rect.y));
            painter.rect(x, y, rect.w * t, rect.h * t, hud::palette::STATUS_BG);
        }
    }
    for (rect, _, _, text) in &placed {
        let (x, y) = px((rect.x, rect.y));
        let w = painter.measure_ui(text, size).width;
        painter.ui(
            text,
            x + (rect.w * t - w) / 2.0,
            y + rect.h * t / 2.0 + size as f32 * 0.3,
            size,
            TEXT,
        );
    }
}
