//! The map pane's frame, and the two strips mounted on it.
//!
//! The pane's ground readout and threat readout ride its **top** border. Its
//! bottom border carries nothing: the player's vitals used to ride it, and
//! moved to the log pane's top border — `hud::log_frame` holds them now, and
//! `docs/seams.md`'s "The expanded log pane is an overlay" entry holds why.
//!
//! The pane used to open on a static `"SECTOR MAP"` title, which carried no
//! information. `Game::terrain_row` is what replaced it: the engine already
//! resolves a biome, its standing condition and any live weather to names,
//! so this module draws them and derives nothing of its own —
//! `ground_pieces`/`weather_pieces` only choose colour and which of the two
//! segments comes first.

use super::palette;
use super::strip::{Mount, Piece, draw_pieces, fitting, sep};
use crate::paint::{Painter, Rect};
use crate::text::Metrics;
use feral_processes_engine::TerrainRow;

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

/// The ground segment: the standing condition's name where one claims this
/// biome, the biome's own name otherwise. Never both — a condition already
/// pins the one biome it claims (`GroundCondition::for_biome`), and the
/// crossing line already named that biome the moment the party arrived.
pub(in crate::render) fn ground_pieces(row: &TerrainRow) -> Vec<Piece> {
    vec![(
        row.condition.unwrap_or(row.biome).to_string(),
        palette::LABEL,
        false,
    )]
}

/// The weather segment. Empty when nothing is live over this ground, which
/// is what lets [`fitting`] drop it without leaving a stray separator
/// standing in for it.
fn weather_pieces(row: &TerrainRow) -> Vec<Piece> {
    match row.event {
        Some(event) => vec![(event.to_string(), palette::ATTENTION, false)],
        None => Vec::new(),
    }
}

/// The measured advance of `pieces`' concatenated text, at the strip's own
/// size — the same measurement [`fitting`] and `border_strip` take
/// internally, exposed here so a caller can size a budget against another
/// mount's *real* width rather than an estimate of it.
fn advance_of(pieces: &[Piece], painter: &Painter, m: &Metrics) -> f32 {
    let text: String = pieces.iter().map(|(t, _, _)| t.as_str()).collect();
    painter.measure_ui_advance(&text, m.small())
}

/// Frame, ground readout and threat readout.
///
/// **Call this after the pane's contents.** `border_strip` paints its own
/// background so the border reads as broken by a label; drawn first, the
/// map's own fill paints straight over the labels and cuts them in half.
///
/// `ground` is `None` underground — a Stack frame has no biome, the same
/// reason `threat.hostiles` is always `0` down there.
pub(in crate::render) fn draw_map_frame(
    pane: Rect,
    ground: Option<TerrainRow>,
    threat: Threat,
    painter: &Painter,
    m: &Metrics,
) {
    painter.rect_lines(pane.x, pane.y, pane.w, pane.h, 2.0, palette::PANE_BORDER);

    let pieces = threat_pieces(threat);
    if let Some(row) = ground {
        // Measured, not estimated: the row has no wrap and no clip, so the
        // left mount's budget is sized against the right mount's *real*
        // width, not a character count of it.
        let avail = (pane.w - m.inset * 2.0 - advance_of(&pieces, painter, m)).max(0.0);
        // Weather first, ground second — `fitting` keeps the longest
        // prefix, so a narrow window drops the ground detail and keeps the
        // news.
        let segments = [weather_pieces(&row), ground_pieces(&row)];
        let shown = fitting(&segments, avail, painter, m);
        draw_pieces(pane, Mount::TopLeft, &shown, painter, m);
    }
    draw_pieces(pane, Mount::TopRight, &pieces, painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{painted_text, with_painter};
    use crate::text::ui_metrics;
    use feral_processes_engine::environment::{GroundCondition, StaticEvent};

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

    /// **Segment order is load-bearing.** `fitting` keeps the longest
    /// *prefix* of `[weather, ground]` that fits, so a budget that fits
    /// only one segment must keep the weather and drop the ground — the
    /// news over the detail.
    ///
    /// Drives `draw_map_frame` itself, at a pane sized precisely to fit the
    /// weather segment beside the threat readout and nothing more — a test
    /// against `fitting` alone, handed a manually built `[weather, ground]`
    /// array, would prove nothing about the order `draw_map_frame` actually
    /// passes; this is what catches that array quietly flipped to
    /// `[ground, weather]`.
    #[test]
    fn draw_map_frame_drops_the_ground_before_the_weather_in_a_narrow_pane() {
        let m = ui_metrics(900.0);
        // The shortest shipped event against the longest shipped condition,
        // so the gap between the two is as wide as the shipped data allows.
        let row = TerrainRow {
            biome: "Null Sector",
            condition: Some(GroundCondition::LockContention.def().name),
            event: Some(StaticEvent::PacketFlood.def().name),
        };
        let threat = Threat {
            hostiles: 0,
            shielded: false,
        };
        let (_, shapes) = with_painter(|p| {
            let weather_w = advance_of(&weather_pieces(&row), p, &m);
            let ground_w = advance_of(&ground_pieces(&row), p, &m);
            assert!(
                weather_w < ground_w,
                "fixture must pick a shorter weather segment than ground: \
                 {weather_w} vs {ground_w}"
            );
            let threat_w = advance_of(&threat_pieces(threat), p, &m);
            // `avail = pane.w - m.inset * 2.0 - threat_w`, `draw_map_frame`'s
            // own formula, solved for a pane wide enough for the weather
            // segment plus one pixel and no more.
            let pane_w = m.inset * 2.0 + threat_w + weather_w + 1.0;
            let pane = Rect::new(0.0, 0.0, pane_w, 200.0);
            draw_map_frame(pane, Some(row), threat, p, &m);
        });
        let text = painted_text(&shapes).join("");
        assert!(
            text.contains(weather_pieces(&row)[0].0.as_str()),
            "the weather segment must survive a budget this narrow: {text:?}"
        );
        assert!(
            !text.contains(ground_pieces(&row)[0].0.as_str()),
            "the ground segment must not fit beside it: {text:?}"
        );
        // The threat readout is unaffected by any of this.
        assert!(text.contains("THREAT"));
    }

    /// Underground, `Game::terrain_row` is `None` and the left mount draws
    /// nothing at all — not an empty background quad standing in for the
    /// title that used to sit there.
    #[test]
    fn no_ground_draws_nothing_on_the_left_mount() {
        let m = ui_metrics(900.0);
        let pane = Rect::new(0.0, 0.0, 1200.0, 600.0);
        let (_, shapes) = with_painter(|p| {
            draw_map_frame(
                pane,
                None,
                Threat {
                    hostiles: 0,
                    shielded: false,
                },
                p,
                &m,
            );
        });
        // The threat readout still draws — only the left mount is empty.
        let text = painted_text(&shapes).join("");
        assert!(
            text.contains("THREAT"),
            "the threat readout must still draw"
        );
        assert!(
            !text.contains("Null Sector")
                && !text.contains("Data Void")
                && !text.contains("Dangling Reads"),
            "nothing should be drawn on the left mount underground: {text:?}"
        );
    }

    /// **Width census.** The widest shipped weather-plus-ground pair must
    /// fit beside the widest `THREAT` readout at 1280x720 — the smallest
    /// window the design is stated against. Both halves are walked from the
    /// live catalogues rather than hardcoded, so a renamed or newly-shipped
    /// condition or event keeps this test honest instead of silently
    /// leaving it testing stale prose.
    ///
    /// Drives `draw_map_frame` itself, at the real `map_pane` rect
    /// `hud::layout::regions` derives for 1280x720 — measuring `fitting`
    /// against an avail recomputed by the test would pass even if
    /// `draw_map_frame`'s own budget (the pane width less the *measured*
    /// threat advance) were wrong, which is exactly the property this
    /// census exists to hold.
    #[test]
    fn the_widest_weather_and_ground_pair_fits_beside_the_widest_threat_at_1280x720() {
        let m = ui_metrics(720.0);
        // The closure returns which names it picked as widest, so the
        // assertions below check the row that was actually drawn rather
        // than re-deriving "widest" a second way — the two used to
        // disagree whenever the UI face stopped being monospace, since one
        // measured real advance and the other counted characters.
        let ((chosen_event, chosen_condition), shapes) = with_painter(|p| {
            let widest = |names: &[&'static str]| -> &'static str {
                names
                    .iter()
                    .max_by(|a, b| {
                        p.measure_ui_advance(*a, m.small())
                            .total_cmp(&p.measure_ui_advance(*b, m.small()))
                    })
                    .expect("at least one shipped value")
            };
            let event_names: Vec<&'static str> =
                StaticEvent::all().iter().map(|e| e.def().name).collect();
            let condition_names: Vec<&'static str> = GroundCondition::all()
                .iter()
                .map(|c| c.def().name)
                .collect();
            let chosen_event = widest(&event_names);
            let chosen_condition = widest(&condition_names);
            let row = TerrainRow {
                biome: "Null Sector",
                condition: Some(chosen_condition),
                event: Some(chosen_event),
            };

            // The widest `THREAT` readout: a two-digit hostile count against
            // whichever tail — "shields holding" or "no defence" — measures
            // wider, found rather than assumed.
            let threat = [true, false]
                .into_iter()
                .map(|shielded| Threat {
                    hostiles: 99,
                    shielded,
                })
                .max_by(|a, b| {
                    advance_of(&threat_pieces(*a), p, &m).total_cmp(&advance_of(
                        &threat_pieces(*b),
                        p,
                        &m,
                    ))
                })
                .unwrap();

            let char_w = p.measure_ui_advance("M", m.font_size);
            let regions = crate::render::hud::layout::regions(1280.0, 720.0, char_w, &m, false);
            draw_map_frame(regions.map_pane, Some(row), threat, p, &m);

            (chosen_event, chosen_condition)
        });

        let text = painted_text(&shapes).join("");
        assert!(
            text.contains(chosen_event),
            "the widest weather segment did not survive beside THREAT at \
             1280x720: {text:?}"
        );
        assert!(
            text.contains(chosen_condition),
            "the widest ground segment did not survive beside THREAT at \
             1280x720: {text:?}"
        );
    }
}
