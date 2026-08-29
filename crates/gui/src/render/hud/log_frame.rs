//! The log pane: its frame, the channel gutter down its left edge, the filter
//! strip on its top border and the keybar on its bottom one.
//!
//! Two rows come off the pane's body and onto its frame here. The filter
//! header used to cost a body row and now rides the top border, which is one
//! more line of log at every window size; the four-line key block used to sit
//! at the foot of the status column and now rides the bottom border as one
//! row. Both are [`border_strip`] doing what it was written for.
//!
//! **The gutter is the honest half of the handoff's five channels.** That
//! reference names `FIELD` `GAIN` `BASE` `DEFEND` `IDLE`, which this game has
//! no model for: a line carries a [`MessageSource`] (two variants) and a
//! [`MessageKind`] (twelve), and nothing anywhere marks a line "idle". So the
//! tag is the source, with the two kinds that are genuinely their own news —
//! a pickup and a sweep — picked out of it. See `channel_tag`.

use feral_processes_app_core::LogFilter;
use feral_processes_engine::{LogEntry, MessageKind, MessageSource};

use super::palette;
use super::strip::{Mount, Piece, draw_pieces, fitting};
use crate::paint::{Color, Painter, Rect};
use crate::render::draw_message_line;
use crate::text::Metrics;

/// The widest tag plus the space that separates it from the message. The
/// gutter is measured off this rather than off a character count, because
/// `layout`'s `char_w` is the body face's advance and this column is drawn at
/// the body size — measuring the string that is actually drawn is what keeps
/// the two from drifting apart.
const GUTTER_SAMPLE: &str = "ALERT  ";

/// What the pane draws.
pub(in crate::render) struct LogPane<'a> {
    pub entries: &'a [LogEntry],
    pub filter: LogFilter,
    pub filtered_out: usize,
    /// The refusal the player's last keypress earned, if any. Stays the first
    /// line of this pane — `needs_status_banner` names the four screens that
    /// draw no popup and `Playing` is not one of them.
    pub refusal: Option<&'a str>,
    /// The pane's border, dimmed or lit by the frame effects layer.
    pub border: Color,
}

/// Which channel a line belongs to, as the gutter says it.
///
/// **Exhaustive on both axes on purpose**, `cell_mark`'s rule: as a `_ =>`
/// match, a thirteenth [`MessageKind`] would ship into the gutter under
/// whatever its source happened to be, which is a claim about the line rather
/// than a gap anyone would notice.
///
/// Only two kinds earn a tag of their own. A pickup (`Loot`) is the one
/// routine event a player scans the log *for*, and a sweep (`Raid`) is the
/// one they must not miss; everything else — narration, a level, a refusal,
/// the whole blow-by-blow of a fight — is news from where the party is
/// standing or news from the base, which is exactly what [`MessageSource`]
/// already says.
fn channel_tag(entry: &LogEntry) -> (&'static str, Color) {
    match entry.kind {
        MessageKind::Loot => ("GAIN", palette::CH_GAIN),
        MessageKind::Raid => ("ALERT", palette::CH_DEFEND),
        MessageKind::Info
        | MessageKind::LevelUp
        | MessageKind::Round
        | MessageKind::Outcome
        | MessageKind::PartyDamage
        | MessageKind::EnemyAttack
        | MessageKind::EnemySpecial
        | MessageKind::Heal
        | MessageKind::Complete
        | MessageKind::Refusal => match entry.source {
            MessageSource::Field => ("FIELD", palette::CH_FIELD),
            MessageSource::Base => ("BASE", palette::CH_BASE),
        },
    }
}

fn dim(text: impl Into<String>) -> Piece {
    (text.into(), palette::FIELD_LABEL, false)
}

/// The filter strip: every channel with the active one picked out, the key
/// that cycles them, the key that opens the history, and — when a channel is
/// being suppressed — how much of it is going unread.
///
/// All three filters are listed rather than only the active one. Named alone,
/// "Field" says nothing about what the other settings are or which way the
/// key steps, and a player reading a base line under a header they thought
/// said otherwise has no way to tell whether the filter or the tagging is
/// wrong. The order is `LogFilter::ALL`, which is the order the key walks.
fn filter_pieces(filter: LogFilter, filtered_out: usize) -> Vec<Piece> {
    let mut pieces = vec![dim("LOG  ")];
    for (i, option) in LogFilter::ALL.iter().enumerate() {
        if i > 0 {
            pieces.push(dim(" · "));
        }
        pieces.push(if *option == filter {
            (option.label().to_string(), palette::HEALTHY, true)
        } else {
            dim(option.label())
        });
    }
    // Lower case because that is what is bound: `App::handle_playing_key`
    // matches `'f'` and `'L'`, and nothing matches `'F'`.
    pieces.push(dim("   f cycle · L history"));
    if let Some(channel) = filter.hidden_channel()
        && filtered_out > 0
    {
        pieces.push(dim(format!("   {filtered_out} {channel} hidden")));
    }
    pieces
}

fn keycap(key: &str, verb: &str) -> Vec<Piece> {
    vec![
        (key.to_string(), palette::EMPHASIS, true),
        (format!(" {verb}"), palette::FIELD_LABEL, false),
    ]
}

fn divider() -> Vec<Piece> {
    vec![("│".to_string(), palette::KEYBAR_DIVIDER, false)]
}

/// The keybar's segments in priority order — the caller takes the longest
/// prefix that fits, so what is last here is what goes first.
///
/// This replaces a four-line block that named eighteen keys. What is not here
/// is in the manual (`?`), which is the discoverable home for it, and `f` and
/// `L` moved to the filter strip on the opposite border rather than being
/// dropped.
///
/// **The order is priority, not the handoff's reading order, and the census
/// is why.** `the_keybar_fits_the_log_pane` measured twelve segments fitting
/// at 1280x720 and thirteen at 1920x1080 — the bar is near enough
/// size-invariant, because `ui_metrics` ramps the face with the window. Under
/// the handoff's own order that put `? help` and `q menu` past the cut, which
/// strands every key this bar had to drop. So movement and the three screens
/// that hold the rest come first, and the answer to the spec's open question
/// is **no**: there is no slack for `t trade` or `s save` at any supported
/// size, and they stay cut exactly as the handoff had them.
///
/// `SPACE` (expand/collapse this pane, see `App::log_expanded`) is not a
/// segment here for the same reason: at 60.5px of slack left over at
/// 1280x720 it is already narrower than what the next-lowest-priority key
/// (`e drain`) needs, and this bar has no room to spend on advertising a key
/// nothing else on the census requires. `?` is still where it is discovered.
fn keybar_segments() -> Vec<Vec<Piece>> {
    vec![
        keycap("hjkl", "move"),
        keycap(".", "wait"),
        divider(),
        keycap("b", "base"),
        keycap("i", "pack"),
        keycap("?", "help"),
        keycap("q", "menu"),
        divider(),
        keycap("a", "routine"),
        keycap("c", "collect"),
        keycap("n", "mine"),
        keycap("x", "examine"),
        keycap("e", "drain"),
        keycap("r", "recharge"),
        keycap("p", "party"),
        keycap("t", "trade"),
        keycap("s", "save"),
    ]
}

/// Fill, refusal, gutter and lines, then the border and its two strips.
///
/// **One function because the order is the whole of it.** `border_strip`
/// paints a background quad so the border reads as broken by a label; drawn
/// before the pane's own fill, every label on both borders is painted over
/// and cut in half. Split across a `draw_body` and a `draw_frame` that is a
/// bug the caller can have; written here it is one this function's ordering
/// prevents.
pub(in crate::render) fn draw_log_pane(pane: Rect, log: &LogPane, painter: &Painter, m: &Metrics) {
    painter.rect(pane.x, pane.y, pane.w, pane.h, palette::STATUS_BG);

    let gutter = painter.measure_ui_advance(GUTTER_SAMPLE, m.font_size);
    let text_x = pane.x + m.inset + gutter;
    let mut y = pane.y + m.inset + m.font_size as f32 / 2.0;

    // The gutter stays blank for a refusal: it is the answer to a keypress,
    // not traffic on a channel, and inventing a tag for it would put it in a
    // column the filter key claims to control.
    if let Some(s) = log.refusal {
        painter.ui(s, text_x, y, m.font_size, palette::THREAT);
        y += m.line_height;
    }
    // The keybar is mounted *on* the bottom border and its glyphs stand half
    // a line above it, so the body's floor is that border rather than the
    // pane's inner edge.
    //
    // At every supported window size the caller's capacity figure already
    // stops short of this, so the guard does not fire in play. It is here
    // because that figure is computed in `draw_playing_base` off the same
    // geometry this function is handed, in a different file — nothing makes
    // the two move together, and a drift between them lands on the keys
    // rather than anywhere a reader would look.
    let floor = pane.y + pane.h - m.inset;
    for entry in log.entries {
        if y > floor {
            break;
        }
        let (tag, color) = channel_tag(entry);
        painter.ui(tag, pane.x + m.inset, y, m.font_size, color);
        draw_message_line(entry, text_x, y, painter, m);
        y += m.line_height;
    }

    painter.rect_lines(pane.x, pane.y, pane.w, pane.h, 2.0, log.border);
    draw_pieces(
        pane,
        Mount::TopLeft,
        &filter_pieces(log.filter, log.filtered_out),
        painter,
        m,
    );
    let avail = pane.w - m.inset * 2.0;
    let taken = fitting(&keybar_segments(), avail, painter, m);
    draw_pieces(pane, Mount::BottomLeft, &taken, painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{painted_text, with_painter};
    use crate::render::hud::layout;
    use crate::text::ui_metrics;

    /// The smallest window the design is stated against.
    const SMALLEST: (f32, f32) = (1280.0, 720.0);

    /// What the keybar may never drop, whatever the window. Movement,
    /// because nothing else on the screen says how to walk; the two screens
    /// that hold the rest of the verbs; and `?`, which is where every key
    /// this bar had to cut now lives. A bar that drops `?` strands the
    /// eighteen keys the four-line block used to name.
    const ESSENTIAL: [&str; 6] = ["hjkl", "move", "b base", "i pack", "? help", "q menu"];

    fn entry(kind: MessageKind, source: MessageSource) -> LogEntry {
        LogEntry {
            kind,
            source,
            text: "a line".to_string(),
            repeats: 1,
        }
    }

    /// Every kind, so a census over them cannot go stale when a thirteenth
    /// lands — `channel_tag`'s own match is exhaustive, and this is the list
    /// the width census walks.
    const KINDS: [MessageKind; 12] = [
        MessageKind::Info,
        MessageKind::Loot,
        MessageKind::LevelUp,
        MessageKind::Raid,
        MessageKind::Round,
        MessageKind::Outcome,
        MessageKind::PartyDamage,
        MessageKind::EnemyAttack,
        MessageKind::EnemySpecial,
        MessageKind::Heal,
        MessageKind::Complete,
        MessageKind::Refusal,
    ];

    /// The two kinds that are their own news whatever channel they came from:
    /// a pickup is what a player scans the log for and a sweep is what they
    /// must not miss. Reading either off the source alone buries it.
    #[test]
    fn a_pickup_and_a_sweep_outrank_the_channel_they_came_from() {
        for source in [MessageSource::Field, MessageSource::Base] {
            assert_eq!(channel_tag(&entry(MessageKind::Loot, source)).0, "GAIN");
            assert_eq!(channel_tag(&entry(MessageKind::Raid, source)).0, "ALERT");
        }
    }

    /// Everything else is news from where the party is standing or news from
    /// the base, which is what `MessageSource` already says — and what the
    /// filter key cycles, so the gutter and the filter agree by construction.
    #[test]
    fn every_other_kind_falls_through_to_its_source() {
        for kind in KINDS {
            if matches!(kind, MessageKind::Loot | MessageKind::Raid) {
                continue;
            }
            assert_eq!(
                channel_tag(&entry(kind, MessageSource::Field)).0,
                "FIELD",
                "{kind:?}"
            );
            assert_eq!(
                channel_tag(&entry(kind, MessageSource::Base)).0,
                "BASE",
                "{kind:?}"
            );
        }
    }

    /// The gutter is a fixed column and the message starts past it, so a tag
    /// wider than [`GUTTER_SAMPLE`] does not push the text over — it draws
    /// straight through it. Nothing else would catch that; the tag and the
    /// line are two separate draw calls and both would look fine alone.
    #[test]
    fn no_channel_tag_overflows_the_gutter() {
        let m = ui_metrics(SMALLEST.1);
        with_painter(|p| {
            let gutter = p.measure_ui_advance(GUTTER_SAMPLE, m.font_size);
            for kind in KINDS {
                for source in [MessageSource::Field, MessageSource::Base] {
                    let (tag, _) = channel_tag(&entry(kind, source));
                    let w = p.measure_ui_advance(tag, m.font_size);
                    assert!(
                        w < gutter,
                        "{tag} is {w}px against a {gutter}px gutter — it would draw \
                         under the message text"
                    );
                }
            }
        });
    }

    fn header_text(filter: LogFilter, filtered_out: usize) -> String {
        filter_pieces(filter, filtered_out)
            .iter()
            .map(|(t, _, _)| t.as_str())
            .collect()
    }

    /// The strip is the only place the two log keys are advertised, so it
    /// draws under `All` too — a filter you can only discover from the help
    /// popup is one nobody turns on. Lower case, because those are the keys
    /// that are bound; `F` reaches nothing.
    #[test]
    fn the_unfiltered_strip_still_names_its_keys_and_counts_nothing() {
        let header = header_text(LogFilter::All, 0);
        assert!(header.contains("All"), "{header}");
        assert!(header.contains("f cycle"), "{header}");
        assert!(header.contains("L history"), "{header}");
        assert!(!header.contains("hidden"), "nothing is hidden: {header}");
    }

    /// The whole set is listed whichever one is active, which is the point of
    /// the strip: "Field" alone says nothing about what else there is.
    #[test]
    fn the_strip_lists_every_filter_in_cycle_order() {
        for filter in LogFilter::ALL {
            let header = header_text(filter, 0);
            let labels: Vec<&str> = LogFilter::ALL.iter().map(|f| f.label()).collect();
            let mut cursor = 0;
            for label in &labels {
                let at = header[cursor..]
                    .find(label)
                    .unwrap_or_else(|| panic!("{label} missing from {header:?} under {filter:?}"));
                cursor += at + label.len();
            }
        }
    }

    /// Bold green is the only thing distinguishing the active filter from the
    /// two it sits between, so it has to land on exactly one piece.
    #[test]
    fn only_the_active_filter_is_picked_out() {
        for filter in LogFilter::ALL {
            let pieces = filter_pieces(filter, 0);
            let picked: Vec<&str> = pieces
                .iter()
                .filter(|(_, color, bold)| *bold && *color == palette::HEALTHY)
                .map(|(t, _, _)| t.as_str())
                .collect();
            assert_eq!(picked, [filter.label()], "under {filter:?}");
        }
    }

    /// The count is what stops a raid landing unseen while the pane is
    /// showing only field news.
    #[test]
    fn a_filtered_strip_counts_the_channel_it_is_hiding() {
        let header = header_text(LogFilter::Field, 3);
        assert!(header.contains("Field"), "{header}");
        assert!(header.contains("3 base hidden"), "{header}");
    }

    /// A channel with no traffic in it has nothing to report, so the strip
    /// stays quiet rather than saying "0 base".
    #[test]
    fn a_filtered_strip_with_an_empty_channel_says_nothing() {
        let header = header_text(LogFilter::Base, 0);
        assert!(!header.contains("hidden"), "{header}");
    }

    /// The census the spec asks for: the keybar is one row on a border with
    /// no wrap and no clip, so whether the two optional keys come back is a
    /// measured question and not a guess. Asserted at the *smallest*
    /// supported window, which is the only size where the answer is in doubt.
    #[test]
    fn the_keybar_fits_the_log_pane() {
        for (w, h) in [SMALLEST, (1920.0, 1080.0)] {
            let m = ui_metrics(h);
            with_painter(|p| {
                let char_w = p.measure_ui_advance("M", m.font_size);
                let pane = layout::regions(w, h, char_w, &m, false).log_pane;
                let avail = pane.w - m.inset * 2.0;
                let taken = fitting(&keybar_segments(), avail, p, &m);
                let drawn: String = taken.iter().map(|(t, _, _)| t.as_str()).collect();
                let slack = avail - p.measure_ui_advance(&drawn, m.small());

                println!("keybar at {w}x{h}: {drawn:?}  slack {slack:.1}px");
                for key in ESSENTIAL {
                    assert!(
                        drawn.contains(key),
                        "the keybar dropped {key:?} at {w}x{h} — slack {slack:.1}px. \
                         Reorder `keybar_segments`, do not widen the pane."
                    );
                }
                assert!(
                    slack >= 0.0,
                    "the keybar overhangs its pane by {slack:.1}px"
                );
            });
        }
    }

    /// The one bug both borders can have. `border_strip` paints a background
    /// quad so the border reads as broken by a label; painted before the
    /// pane's own fill, every label on both borders is covered and cut in
    /// half. `draw_log_pane` is one function precisely so the ordering is
    /// not a thing five call sites each have to get right.
    #[test]
    fn the_pane_fills_before_its_strips() {
        let m = ui_metrics(900.0);
        let pane = crate::paint::Rect::new(0.0, 600.0, 900.0, 120.0);
        let entries = [entry(MessageKind::Info, MessageSource::Field)];
        let (_, shapes) = with_painter(|p| {
            draw_log_pane(
                pane,
                &LogPane {
                    entries: &entries,
                    filter: LogFilter::All,
                    filtered_out: 0,
                    refusal: None,
                    border: palette::PANE_BORDER,
                },
                p,
                &m,
            );
        });

        let fill = shapes
            .iter()
            .position(|cs| match &cs.shape {
                // `fill.a() > 0` is load-bearing: `rect_lines` paints a rect
                // the exact size of the pane too, as a stroke with a
                // transparent fill. Matching on geometry alone finds the
                // border and passes whichever drew first.
                bevy_egui::egui::Shape::Rect(r) => {
                    r.fill.a() > 0
                        && (r.rect.min.x - pane.x).abs() < 0.5
                        && (r.rect.min.y - pane.y).abs() < 0.5
                        && (r.rect.width() - pane.w).abs() < 0.5
                }
                _ => false,
            })
            .expect("the log pane paints a background the size of the pane");
        let label = shapes
            .iter()
            .position(|cs| match &cs.shape {
                bevy_egui::egui::Shape::Text(t) => t.galley.text().contains("LOG"),
                _ => false,
            })
            .expect("the filter strip paints its label");
        assert!(
            fill < label,
            "the pane's fill painted at {fill}, after the strip label at {label} — \
             the label is cut in half"
        );
    }

    /// Nothing else would catch a row drawn through the keys: the line and
    /// the keybar are both painted, both look right in isolation, and the
    /// keybar draws last, so it sits on top of a half-covered line rather
    /// than under one.
    ///
    /// **The fixture over-asks on purpose.** Handed exactly the capacity
    /// `draw_playing_base` computes, the rows clear the border on the
    /// arithmetic alone and this test passes with the floor deleted — which
    /// is a test that reads as coverage and holds nothing. Handed more than
    /// fits, it is the floor that stops them, and removing the floor fails it
    /// at y=706 against a 694 ceiling. Over-asking is also the drift the
    /// floor exists for: that capacity figure lives in another file.
    #[test]
    fn no_log_line_reaches_the_keybar() {
        let m = ui_metrics(SMALLEST.1);
        with_painter(|p| {
            let char_w = p.measure_ui_advance("M", m.font_size);
            let pane = layout::regions(SMALLEST.0, SMALLEST.1, char_w, &m, false).log_pane;
            // `draw_playing_base`'s own figure. Kept in step by hand, so if
            // that expression moves this test is measuring the wrong pane.
            let capacity = ((pane.h - m.line_height) / m.line_height).max(1.0) as usize;
            let entries: Vec<LogEntry> = (0..capacity + 3)
                .map(|_| entry(MessageKind::Info, MessageSource::Field))
                .collect();
            let (_, shapes) = with_painter(|q| {
                draw_log_pane(
                    pane,
                    &LogPane {
                        entries: &entries,
                        filter: LogFilter::All,
                        filtered_out: 0,
                        refusal: Some("Nothing to collect."),
                        border: palette::PANE_BORDER,
                    },
                    q,
                    &m,
                );
            });

            // The keybar's glyphs are centred on the border and rise about
            // half their size above it; a body row must stay clear of that.
            let ceiling = pane.y + pane.h - m.small() as f32 / 2.0;
            for cs in &shapes {
                if let bevy_egui::egui::Shape::Text(t) = &cs.shape {
                    let text = t.galley.text();
                    if text.contains("a line") || text.contains("Nothing to collect.") {
                        assert!(
                            t.pos.y < ceiling,
                            "a body row drew at y={} against a {ceiling} floor — it \
                             runs through the keybar",
                            t.pos.y
                        );
                    }
                }
            }
        });
    }

    /// Both of the pane's borders carry a strip, and the gutter tags the one
    /// line the fixture holds. All three come off one call.
    #[test]
    fn the_pane_draws_its_gutter_and_both_strips() {
        let m = ui_metrics(900.0);
        let entries = [entry(MessageKind::Loot, MessageSource::Base)];
        let (_, shapes) = with_painter(|p| {
            draw_log_pane(
                crate::paint::Rect::new(0.0, 600.0, 900.0, 120.0),
                &LogPane {
                    entries: &entries,
                    filter: LogFilter::All,
                    filtered_out: 0,
                    refusal: Some("Nothing to collect."),
                    border: palette::PANE_BORDER,
                },
                p,
                &m,
            );
        });
        let text = painted_text(&shapes).join("\n");
        assert!(text.contains("GAIN"), "no gutter tag: {text}");
        assert!(text.contains("a line"), "no log line: {text}");
        assert!(text.contains("Nothing to collect."), "no refusal: {text}");
        assert!(text.contains("LOG"), "no filter strip: {text}");
        assert!(text.contains("move"), "no keybar: {text}");
    }
}
