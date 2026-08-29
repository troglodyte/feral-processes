//! The map pane's frame, and the three strips mounted on it.
//!
//! The pane's title and threat readout ride its top border; the player's
//! vitals ride its bottom one. That is what the handoff means by vitals
//! costing zero body rows — the numbers live on the frame, and the whole
//! pane below is map.
//!
//! **The vitals strip does not fit at every window size and does not pretend
//! to.** It is one row on a border with no wrap and no clip, so its segments
//! carry a fixed priority and the ones that do not fit are dropped from the
//! end, measured. That is `stock::fits`' rule applied to segments instead of
//! piles.

use feral_processes_engine::PlayerStatus;

use super::bar::bar;
use super::palette;
use super::strip::{Mount, Piece, draw_pieces, fitting, label, sep, value};
use crate::paint::{Color, Painter, Rect};
use crate::text::Metrics;

const INTEG_CELLS: usize = 16;
const POWER_CELLS: usize = 10;
const XP_CELLS: usize = 14;
/// `POWER_MAX` — the reserve's ceiling is fixed forever and does not scale
/// with the player, unlike `max_hp`.
const POWER_MAX: f32 = 100.0;

/// What the vitals strip reads.
pub(in crate::render) struct Vitals<'a> {
    pub status: &'a PlayerStatus,
    pub mining: bool,
}

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

/// A meter as three pieces: label, filled cells, trough cells.
fn meter(name: &str, v: f32, max: f32, cells: usize, fill: Color) -> Vec<Piece> {
    let b = bar(v, max, cells);
    vec![
        label(name),
        (b.filled, fill, false),
        (b.empty, palette::BAR_TROUGH, false),
    ]
}

/// The vitals segments in priority order. The caller takes the longest
/// prefix that fits.
fn vitals_segments(v: &Vitals) -> Vec<Vec<Piece>> {
    let s = v.status;
    let mut out: Vec<Vec<Piece>> = Vec::new();

    let mut integ = meter(
        "INTEG ",
        s.hp as f32,
        s.max_hp.max(1) as f32,
        INTEG_CELLS,
        palette::HEALTHY,
    );
    integ.push(value(format!(" {}/{}", s.hp, s.max_hp.max(1))));
    out.push(integ);

    let mut pwr = meter("PWR ", s.power, POWER_MAX, POWER_CELLS, palette::ATTENTION);
    pwr.push(value(format!(" {:.0}", s.power)));
    out.push(pwr);

    let mut xp = meter(
        &format!("L{} ", s.level),
        s.xp as f32,
        s.xp_to_next.max(1) as f32,
        XP_CELLS,
        palette::CH_GAIN,
    );
    xp.push(value(format!(" {}/{}", s.xp, s.xp_to_next)));
    out.push(xp);

    // Omitted entirely at zero rather than reading "0 perk pts". It wears
    // ATTENTION, which is reserved for "the player must act", so drawing it
    // with nothing to spend would be that reservation lapsing on its first
    // use in the game.
    if s.perk_points > 0 {
        out.push(vec![(
            format!("\u{25B8} {} perk pts [k]", s.perk_points),
            palette::ATTENTION,
            true,
        )]);
    }

    out.push(vec![label("MIT "), value(format!("{}%", s.mitigation))]);
    out.push(vec![label("ATK "), value(s.atk.to_string())]);
    out.push(vec![label("STR "), value(s.strength.to_string())]);
    out.push(vec![label("DEC "), value(s.decompiler.to_string())]);
    out.push(vec![if v.mining {
        ("mining on".to_string(), palette::HEALTHY, false)
    } else {
        ("mining off".to_string(), palette::LABEL, false)
    }]);
    out
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

/// Frame, title, threat readout and vitals.
///
/// **Call this after the pane's contents.** `border_strip` paints its own
/// background so the border reads as broken by a label; drawn first, the
/// map's own fill paints straight over the labels and cuts them in half.
pub(in crate::render) fn draw_map_frame(
    pane: Rect,
    vitals: &Vitals,
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

    let avail = pane.w - m.inset * 2.0;
    let segments = vitals_segments(vitals);
    let taken = fitting(&segments, avail, painter, m);
    draw_pieces(pane, Mount::BottomLeft, &taken, painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{painted_text, with_painter};
    use crate::text::ui_metrics;
    use feral_processes_engine::{DifficultyMode, Game};

    /// Every figure at its widest plausible value, so the width census is not
    /// passing on a short fixture.
    fn wide_status() -> PlayerStatus {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(7, DifficultyMode::Forgiving, assets).expect("shipped assets load");
        let mut s = game.player_status();
        s.hp = 99_999;
        s.max_hp = 99_999;
        s.power = 100.0;
        s.level = 99;
        s.xp = 999_999;
        s.xp_to_next = 999_999;
        s.perk_points = 99;
        s.mitigation = 88;
        s.atk = 777;
        s.strength = 9_999;
        s.decompiler = 666;
        s
    }

    fn segments_text(taken: &[Piece]) -> String {
        taken.iter().map(|(t, _, _)| t.as_str()).collect()
    }

    fn pane_at(screen_w: f32, screen_h: f32, m: &Metrics) -> Rect {
        super::super::layout::regions(screen_w, screen_h, 9.0, m, false).map_pane
    }

    /// The one thing this strip must never do. It is a single row on a
    /// border with no wrap and no clip, so a line wider than the pane is not
    /// cut off — it is drawn past the edge in silence.
    #[test]
    fn the_vitals_strip_never_draws_wider_than_its_pane() {
        let m = ui_metrics(720.0);
        let status = wide_status();
        let v = Vitals {
            status: &status,
            mining: true,
        };
        with_painter(|p| {
            let pane = pane_at(1280.0, 720.0, &m);
            let segments = vitals_segments(&v);
            let taken = fitting(&segments, pane.w - m.inset * 2.0, p, &m);
            let drawn = p.measure_ui_advance(segments_text(&taken), m.small());
            assert!(
                drawn <= pane.w - m.inset * 2.0,
                "vitals draw {drawn} into {}",
                pane.w - m.inset * 2.0
            );
            // If the fixture fits whole, this test is not exercising the drop
            // rule and would pass against no drop rule at all.
            let whole: String = segments
                .iter()
                .flatten()
                .map(|(t, _, _)| t.as_str())
                .collect();
            assert!(
                p.measure_ui_advance(&whole, m.small()) > pane.w - m.inset * 2.0,
                "fixture fits at 1280x720 — it cannot exercise the drop rule"
            );
        });
    }

    /// Guards a drop rule that is too eager, which the census above alone
    /// would not catch: a strip that always drew one segment would pass it.
    #[test]
    fn a_wide_window_keeps_more_than_a_narrow_one() {
        let status = wide_status();
        let v = Vitals {
            status: &status,
            mining: true,
        };
        with_painter(|p| {
            let narrow_m = ui_metrics(720.0);
            let wide_m = ui_metrics(1080.0);
            let count = |screen: (f32, f32), m: &Metrics| {
                let pane = pane_at(screen.0, screen.1, m);
                fitting(&vitals_segments(&v), pane.w - m.inset * 2.0, p, m).len()
            };
            let narrow = count((1280.0, 720.0), &narrow_m);
            let wide = count((1920.0, 1080.0), &wide_m);
            assert!(narrow > 0, "nothing fits at 1280x720");
            assert!(
                wide > narrow,
                "a 1920-wide pane took {wide} pieces against {narrow} at 1280"
            );
        });
    }

    /// A perk-points segment reading zero is chrome, and it wears the colour
    /// reserved for "the player must act".
    #[test]
    fn no_perk_segment_when_none_are_unspent() {
        let mut status = wide_status();
        status.perk_points = 0;
        let text: String = vitals_segments(&Vitals {
            status: &status,
            mining: false,
        })
        .iter()
        .flatten()
        .map(|(t, _, _)| t.as_str())
        .collect();
        assert!(
            !text.contains("perk"),
            "zero perk points still drew: {text}"
        );

        status.perk_points = 1;
        let text: String = vitals_segments(&Vitals {
            status: &status,
            mining: false,
        })
        .iter()
        .flatten()
        .map(|(t, _, _)| t.as_str())
        .collect();
        assert!(text.contains("perk pts [k]"), "one perk point drew nothing");
    }

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
