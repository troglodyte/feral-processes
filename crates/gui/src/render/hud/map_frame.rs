//! The map pane's frame, and the two strips mounted on it.
//!
//! The pane's title and threat readout ride its **top** border. Its bottom
//! border carries nothing: the player's vitals used to ride it, and moved to
//! the log pane's top border — `hud::log_frame` holds them now, and
//! `docs/seams.md`'s "The expanded log pane is an overlay" entry holds why.

use super::palette;
use super::strip::{Mount, Piece, draw_pieces, sep};
use crate::paint::{Painter, Rect};
use crate::text::Metrics;

/// What the threat strip reads.
///
/// There is deliberately **no countdown**. A GC Entropy Sweep is a per-tick
/// roll (`Game::raid_check`), not a schedule, so the handoff's "sweep in 3
/// ticks" is not derivable from anything the simulation holds. Adding a
/// field here for it would be inventing a mechanic to fill a strip.
#[derive(Copy, Clone, Debug)]
pub(in crate::render) struct Threat {
    pub hostiles: usize,
    pub shielded: bool,
}

fn threat_pieces(t: Threat) -> Vec<Piece> {
    let hot = t.hostiles > 0;
    let mut out = vec![(
        "THREAT ".to_string(),
        if hot { palette::THREAT } else { palette::LABEL },
        true,
    )];
    out.push(if hot {
        (format!(" {} hostile", t.hostiles), palette::THREAT, false)
    } else {
        (" clear".to_string(), palette::LABEL, false)
    });
    out.push(sep());
    out.push(if t.shielded {
        ("shields holding".to_string(), palette::HEALTHY, false)
    } else {
        ("no defence".to_string(), palette::ATTENTION, false)
    });
    out
}

/// Frame, title and threat readout.
///
/// **Call this after the pane's contents.** `border_strip` paints its own
/// background so the border reads as broken by a label; drawn first, the
/// map's own fill paints straight over the labels and cuts them in half.
pub(in crate::render) fn draw_map_frame(
    pane: Rect,
    threat: Threat,
    painter: &Painter,
    m: &Metrics,
) {
    painter.rect_lines(pane.x, pane.y, pane.w, pane.h, 2.0, palette::PANE_BORDER);

    draw_pieces(
        pane,
        Mount::TopLeft,
        &[("SECTOR MAP".to_string(), palette::PANE_TITLE, true)],
        painter,
        m,
    );
    draw_pieces(pane, Mount::TopRight, &threat_pieces(threat), painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{painted_text, with_painter};
    use crate::text::ui_metrics;

    /// Four cases across both axes, plus the assertion that stops the
    /// handoff's countdown being reintroduced by someone reading it rather
    /// than the spec: raids are a per-tick roll, so there is no number of
    /// ticks to show.
    #[test]
    fn the_threat_strip_reads_the_hostiles_and_the_shields() {
        let m = ui_metrics(900.0);
        for (hostiles, shielded, want) in [
            (0usize, true, "shields holding"),
            (0, false, "no defence"),
            (2, true, "2 hostile"),
            (2, false, "no defence"),
        ] {
            let (_, shapes) = with_painter(|p| {
                draw_pieces(
                    Rect::new(0.0, 0.0, 1200.0, 600.0),
                    Mount::TopRight,
                    &threat_pieces(Threat { hostiles, shielded }),
                    p,
                    &m,
                );
            });
            let text = painted_text(&shapes).join("");
            assert!(
                text.contains(want),
                "{hostiles} hostiles, shielded={shielded} drew {text:?}, wanted {want:?}"
            );
            assert!(
                !text.contains("tick"),
                "the threat strip invented a countdown: {text:?} — raids are a \
                 per-tick roll and there is no schedule to read"
            );
        }
        // The calm case says so rather than leaving a gap.
        let (_, shapes) = with_painter(|p| {
            draw_pieces(
                Rect::new(0.0, 0.0, 1200.0, 600.0),
                Mount::TopRight,
                &threat_pieces(Threat {
                    hostiles: 0,
                    shielded: true,
                }),
                p,
                &m,
            );
        });
        assert!(painted_text(&shapes).join("").contains("clear"));
    }
}
