//! The log pane: its frame, the channel gutter down its left edge, the
//! player's vitals on its top border, the filter header as its first body
//! row, and the keybar on its bottom border.
//!
//! **Only one strip may ride a border**, and that is what decides the two
//! halves above. [`border_strip`] centres its background quad *on* the line
//! it mounts to, reaching `size/2 + pad/2` past it on **both** sides — so
//! the filter strip, riding this pane's top border, painted over the lower
//! half of the vitals riding the map pane's bottom border, baseline
//! included. The vitals took the border because they are the readout that
//! must never be covered; the filter header went back to being a body row,
//! where it is top-aligned with the messages it heads and nothing above
//! `pane.y` can reach it. The keybar keeps the bottom border, which is
//! uncontested.
//!
//! The vitals ride *this* pane rather than the map's for a second reason:
//! the expanded pane (SPACE, `App::log_expanded`) is an overlay over the
//! bottom of the map, so a strip on the map's bottom border disappeared
//! entirely for as long as the log was open. Mounted here it travels with
//! the pane.
//!
//! **The vitals strip does not fit at every window size and does not pretend
//! to.** It is one row on a border with no wrap and no clip, so its segments
//! carry a fixed priority and the ones that do not fit are dropped from the
//! end, measured. That is `stock::fits`' rule applied to segments instead of
//! piles.
//!
//! **The gutter is the honest half of the handoff's five channels.** That
//! reference names `FIELD` `GAIN` `BASE` `DEFEND` `IDLE`, which this game has
//! no model for: a line carries a [`MessageSource`] (two variants) and a
//! [`MessageKind`] (twelve), and nothing anywhere marks a line "idle". So the
//! tag is the source, with the two kinds that are genuinely their own news —
//! a pickup and a sweep — picked out of it. See `channel_tag`.

use feral_processes_app_core::LogFilter;
use feral_processes_engine::text::wrap;
use feral_processes_engine::{LogEntry, MessageKind, MessageSource, PlayerStatus};

use super::bar::bar;
use super::palette;
use super::strip::{Mount, Piece, draw_pieces, fitting, label, value};
use crate::paint::{Color, Painter, Rect, TextRun};
use crate::render::{draw_message_text, message_text};
use crate::text::Metrics;

/// The widest tag plus the space that separates it from the message. The
/// gutter is measured off this rather than off a character count, because
/// `layout`'s `char_w` is the body face's advance and this column is drawn at
/// the body size — measuring the string that is actually drawn is what keeps
/// the two from drifting apart.
const GUTTER_SAMPLE: &str = "ALERT  ";

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

/// What the pane draws.
pub(in crate::render) struct LogPane<'a> {
    pub entries: &'a [LogEntry],
    pub filter: LogFilter,
    pub filtered_out: usize,
    /// Mounted on the pane's top border, not drawn among the rows — see the
    /// module comment for why it is this pane's border and not the map's.
    pub vitals: &'a Vitals<'a>,
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

fn dim(text: impl Into<String>) -> Piece {
    (text.into(), palette::FIELD_LABEL, false)
}

/// The filter row: every channel with the active one picked out, the key
/// that cycles them, the key that opens the history, and — when a channel is
/// being suppressed — how much of it is going unread.
///
/// Drawn at `m.small()`, not the body size. It is chrome heading the rows
/// rather than one of them, it is the size it was measured and tuned at
/// while it rode the border, and at the body size it would compete with the
/// news underneath it for the eye.
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

/// How many characters of message fit between the gutter and the pane's
/// right edge.
///
/// Measured rather than assumed. The UI face is DejaVu Sans Mono, whose
/// advance is nothing like the map font's cell, and `layout`'s `char_w` is
/// derived at the map size — a hardcoded column count would be right at one
/// window size and wrong at every other, since `ui_metrics` ramps the face
/// with the window.
fn message_columns(pane: Rect, text_x: f32, painter: &Painter, m: &Metrics) -> usize {
    let avail = pane.x + pane.w - m.inset - text_x;
    (avail / painter.measure_ui_advance("M", m.font_size)).max(1.0) as usize
}

/// One drawn row of the body: the text, the kind that styles it, and the
/// channel tag when this is the row its entry leads with.
struct BodyRow {
    tag: Option<(&'static str, Color)>,
    text: String,
    kind: MessageKind,
}

/// How many rows fit between a first baseline at `y` and the body's `floor`.
///
/// The floor counted forwards rather than checked per row: once an entry can
/// take more than one row, what does not fit has to be decided *before* the
/// drawing, so the rows that are dropped are the oldest ones and not the
/// newest.
fn rows_fitting(y: f32, floor: f32, line_height: f32) -> usize {
    if y > floor {
        return 0;
    }
    ((floor - y) / line_height) as usize + 1
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
    let columns = message_columns(pane, text_x, painter, m);
    // The keybar is mounted *on* the bottom border and its glyphs stand half
    // a line above it, so the body's floor is that border rather than the
    // pane's inner edge.
    //
    // At every supported window size the caller's capacity figure already
    // stops short of this, so nothing is dropped for it in play. It is here
    // because that figure is computed in `draw_playing_base` off the same
    // geometry this function is handed, in a different file — nothing makes
    // the two move together, and a drift between them lands on the keys
    // rather than anywhere a reader would look. Wrapping is the second way
    // the two can disagree, and this time by design: the caller counts
    // entries and a wrapped entry is several rows.
    let floor = pane.y + pane.h - m.inset;

    // The filter row is pinned first, above even a refusal: it names what
    // the rows below it are, and a header that moves the moment the player
    // mistypes a key is not a header. It starts at the pane's own inset —
    // in the gutter column, not past it — because it heads the whole body
    // and not one channel of it. `LOG_FILTER_ROWS` in `hud::layout` is the
    // row this consumes.
    let filter = filter_pieces(log.filter, log.filtered_out);
    let runs: Vec<TextRun> = filter
        .iter()
        .map(|(text, color, bold)| TextRun {
            text,
            bold: *bold,
            color: *color,
        })
        .collect();
    painter.ui_runs(&runs, pane.x + m.inset, y, m.small());
    y += m.line_height;

    // The gutter stays blank for a refusal: it is the answer to a keypress,
    // not traffic on a channel, and inventing a tag for it would put it in a
    // column the filter key claims to control.
    if let Some(s) = log.refusal {
        for line in wrap(s, columns) {
            if y > floor {
                break;
            }
            painter.ui(&line, text_x, y, m.font_size, palette::THREAT);
            y += m.line_height;
        }
    }

    // Rows, not entries. The tag rides the first row of its entry alone: it
    // says which channel a *line* came in on, and repeating it down a wrapped
    // one would read as that many lines of traffic.
    let mut rows: Vec<BodyRow> = Vec::new();
    for entry in log.entries {
        let (tag, color) = channel_tag(entry);
        for (i, line) in wrap(&message_text(entry), columns).into_iter().enumerate() {
            rows.push(BodyRow {
                tag: (i == 0).then_some((tag, color)),
                text: line,
                kind: entry.kind,
            });
        }
    }
    // The cut comes off the *oldest* end, which is the whole reason this is
    // a cut rather than a `break` at the floor. `pane_rows` hands the rows
    // over oldest first, so stopping at the floor would drop the newest news
    // — the half of the pane the player is actually reading — every time a
    // long line pushed the rest past the bottom. A single entry taller than
    // the pane keeps its own tail for the same reason, which is what a
    // terminal does with a line too long for its window.
    let room = rows_fitting(y, floor, m.line_height);
    if rows.len() > room {
        rows.drain(..rows.len() - room);
    }
    for row in rows {
        if let Some((tag, color)) = row.tag {
            painter.ui(tag, pane.x + m.inset, y, m.font_size, color);
        }
        draw_message_text(row.kind, &row.text, text_x, y, painter, m);
        y += m.line_height;
    }

    painter.rect_lines(pane.x, pane.y, pane.w, pane.h, 2.0, log.border);
    let avail = pane.w - m.inset * 2.0;
    let vitals = fitting(&vitals_segments(log.vitals), avail, painter, m);
    draw_pieces(pane, Mount::TopLeft, &vitals, painter, m);
    let taken = fitting(&keybar_segments(), avail, painter, m);
    draw_pieces(pane, Mount::BottomLeft, &taken, painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{painted_text, with_painter};
    use crate::render::hud::layout;
    use crate::text::ui_metrics;
    use feral_processes_engine::{DifficultyMode, Game};

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

    /// Every figure at its widest plausible value, so the width census is
    /// not passing on a short fixture.
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

    /// The pane's three constant fields, so a test that does not care about
    /// them says so by omission.
    fn log_pane<'a>(
        entries: &'a [LogEntry],
        vitals: &'a Vitals<'a>,
        refusal: Option<&'a str>,
    ) -> LogPane<'a> {
        LogPane {
            entries,
            filter: LogFilter::All,
            filtered_out: 0,
            vitals,
            refusal,
            border: palette::PANE_BORDER,
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
        let status = wide_status();
        let vitals = Vitals {
            status: &status,
            mining: true,
        };
        let entries = [entry(MessageKind::Info, MessageSource::Field)];
        let (_, shapes) = with_painter(|p| {
            draw_log_pane(pane, &log_pane(&entries, &vitals, None), p, &m);
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
            let capacity = ((pane.h - m.line_height * (1.0 + layout::LOG_FILTER_ROWS))
                / m.line_height)
                .max(1.0) as usize;
            let status = wide_status();
            let vitals = Vitals {
                status: &status,
                mining: true,
            };
            let entries: Vec<LogEntry> = (0..capacity + 3)
                .map(|_| entry(MessageKind::Info, MessageSource::Field))
                .collect();
            let (_, shapes) = with_painter(|q| {
                draw_log_pane(
                    pane,
                    &log_pane(&entries, &vitals, Some("Nothing to collect.")),
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

    /// A line long enough to reach the info column beside it. `Painter`
    /// clips nothing horizontally, so an unwrapped row does not stop at the
    /// pane's edge — it draws straight across the column to its right, and
    /// reads as a fault in *that* column rather than in this one.
    ///
    /// Every styling path is in the fixture, because each reaches the screen
    /// through a different `Painter` call: plain text, the bold a level
    /// takes, the run-per-number a blow takes, and the folded row's count,
    /// which is appended to the text after this function has handed it over.
    /// The refusal is here too — it is the pane's first line and is the one
    /// thing `draw_log_pane` draws itself.
    #[test]
    fn no_log_line_runs_past_the_pane_into_the_info_column() {
        let m = ui_metrics(SMALLEST.1);
        with_painter(|p| {
            let char_w = p.measure_ui_advance("M", m.font_size);
            let pane = layout::regions(SMALLEST.0, SMALLEST.1, char_w, &m, false).log_pane;
            let long: String = (0..40).map(|i| format!("overrun-{i:02} ")).collect();
            let refusal: String = (0..20).map(|i| format!("refused-{i:02} ")).collect();
            let status = wide_status();
            let vitals = Vitals {
                status: &status,
                mining: true,
            };
            let entries: Vec<LogEntry> = [
                (MessageKind::Info, 1),
                (MessageKind::LevelUp, 1),
                (MessageKind::PartyDamage, 1),
                (MessageKind::Loot, 4),
            ]
            .into_iter()
            .map(|(kind, repeats)| LogEntry {
                kind,
                source: MessageSource::Field,
                text: long.clone(),
                repeats,
            })
            .collect();
            let (_, shapes) = with_painter(|q| {
                draw_log_pane(pane, &log_pane(&entries, &vitals, Some(&refusal)), q, &m);
            });

            let edge = pane.x + pane.w;
            for cs in &shapes {
                if let bevy_egui::egui::Shape::Text(t) = &cs.shape {
                    let text = t.galley.text().to_string();
                    if !text.contains("overrun") && !text.contains("refused") {
                        continue;
                    }
                    let right = t.pos.x + t.galley.rect.width();
                    assert!(
                        right <= edge,
                        "a log row ran to x={right} against a {edge} pane edge — it \
                         draws over the info column: {text:?}"
                    );
                }
            }
        });
    }

    /// An entry led by `lead` and just long enough to take a second row.
    ///
    /// Sized off the measured column count rather than a fixed length. The
    /// pane is about five rows tall at the smallest supported window, so a
    /// fixture that wraps into more rows than the pane holds says nothing
    /// about which end the overflow comes off — the answer would be "both".
    fn two_row_entry(lead: &str, columns: usize) -> LogEntry {
        let mut text = lead.to_string();
        while text.chars().count() <= columns {
            text.push_str(" filler");
        }
        LogEntry {
            kind: MessageKind::Info,
            source: MessageSource::Field,
            text,
            repeats: 1,
        }
    }

    /// A tag belongs to the entry, not to each row it happens to take. Two
    /// tags down one wrapped sentence read as two lines of traffic on that
    /// channel, and the continuation rows have to line up under the first or
    /// the gutter stops being a column.
    #[test]
    fn a_wrapped_entry_wears_one_tag_and_indents_its_continuations() {
        let m = ui_metrics(SMALLEST.1);
        with_painter(|p| {
            let char_w = p.measure_ui_advance("M", m.font_size);
            let pane = layout::regions(SMALLEST.0, SMALLEST.1, char_w, &m, false).log_pane;
            let text_x = pane.x + m.inset + p.measure_ui_advance(GUTTER_SAMPLE, m.font_size);
            let status = wide_status();
            let vitals = Vitals {
                status: &status,
                mining: true,
            };
            let entries = [two_row_entry(
                "overrun",
                message_columns(pane, text_x, p, &m),
            )];
            let (_, shapes) = with_painter(|q| {
                draw_log_pane(pane, &log_pane(&entries, &vitals, None), q, &m);
            });

            let mut tags = 0;
            let mut rows = 0;
            for cs in &shapes {
                if let bevy_egui::egui::Shape::Text(t) = &cs.shape {
                    let text = t.galley.text();
                    if text == "FIELD" {
                        tags += 1;
                    }
                    if text.contains("filler") {
                        rows += 1;
                        assert!(
                            (t.pos.x - text_x).abs() < 0.5,
                            "a wrapped row started at x={} against a {text_x} text column",
                            t.pos.x
                        );
                    }
                }
            }
            assert!(rows > 1, "the fixture did not wrap: {rows} row(s)");
            assert_eq!(tags, 1, "{rows} rows wore {tags} tags");
        });
    }

    /// The cut has to come off the oldest end. `pane_rows` hands the pane its
    /// rows oldest first and `draw_playing_base` sizes the request in
    /// *entries*, so as soon as one entry wraps there are more rows than the
    /// pane has — and stopping at the floor instead would drop the newest
    /// news, which is worse than the overhang this wrap exists to fix.
    #[test]
    fn a_wrapped_screenful_drops_the_oldest_rows_and_not_the_newest() {
        let m = ui_metrics(SMALLEST.1);
        with_painter(|p| {
            let char_w = p.measure_ui_advance("M", m.font_size);
            let pane = layout::regions(SMALLEST.0, SMALLEST.1, char_w, &m, false).log_pane;
            let text_x = pane.x + m.inset + p.measure_ui_advance(GUTTER_SAMPLE, m.font_size);
            let columns = message_columns(pane, text_x, p, &m);
            let status = wide_status();
            let vitals = Vitals {
                status: &status,
                mining: true,
            };
            let entries: Vec<LogEntry> = (0..10)
                .map(|i| two_row_entry(&format!("mark{i:02}"), columns))
                .collect();
            let (_, shapes) = with_painter(|q| {
                draw_log_pane(pane, &log_pane(&entries, &vitals, None), q, &m);
            });

            let drawn = painted_text(&shapes).join("\n");
            assert!(
                drawn.contains("mark09"),
                "the newest line went undrawn: {drawn}"
            );
            assert!(
                !drawn.contains("mark00"),
                "the fixture fits the pane, so it proves nothing: {drawn}"
            );
        });
    }

    fn segments_text(taken: &[Piece]) -> String {
        taken.iter().map(|(t, _, _)| t.as_str()).collect()
    }

    fn pane_at(screen_w: f32, screen_h: f32, m: &Metrics) -> Rect {
        layout::regions(screen_w, screen_h, 9.0, m, false).log_pane
    }

    /// The one thing this strip must never do. It is a single row on a
    /// border with no wrap and no clip, so a line wider than the pane is not
    /// cut off — it is drawn past the edge in silence.
    #[test]
    fn the_vitals_strip_never_draws_wider_than_its_pane() {
        let m = ui_metrics(SMALLEST.1);
        let status = wide_status();
        let v = Vitals {
            status: &status,
            mining: true,
        };
        with_painter(|p| {
            let pane = pane_at(SMALLEST.0, SMALLEST.1, &m);
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
            let narrow_m = ui_metrics(SMALLEST.1);
            let wide_m = ui_metrics(1080.0);
            let count = |screen: (f32, f32), m: &Metrics| {
                let pane = pane_at(screen.0, screen.1, m);
                fitting(&vitals_segments(&v), pane.w - m.inset * 2.0, p, m).len()
            };
            let narrow = count(SMALLEST, &narrow_m);
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

    /// **The two halves of the swap, asserted together because either alone
    /// is satisfied by the arrangement this replaced.** The vitals ride the
    /// top border, where a strip's quad has the line to itself; the filter
    /// header is the *first row of the body*, which is what "the top of the
    /// toggle menu and the top of the log area are the same" means.
    ///
    /// Positions are galley **tops** (`Painter::ui_runs` converts the
    /// baseline it takes), and the header is drawn at `m.small()` against
    /// the messages' `m.font_size`, so the assertions are orderings and
    /// containments rather than a line-height arithmetic that would be
    /// comparing two different ascents. What a body row *costs* the pane is
    /// `layout::the_log_pane_carries_one_filter_row_in_both_states`.
    #[test]
    fn the_filter_heads_the_body_and_the_vitals_ride_the_border() {
        let m = ui_metrics(SMALLEST.1);
        with_painter(|p| {
            let char_w = p.measure_ui_advance("M", m.font_size);
            let pane = layout::regions(SMALLEST.0, SMALLEST.1, char_w, &m, false).log_pane;
            let status = wide_status();
            let vitals = Vitals {
                status: &status,
                mining: true,
            };
            let entries = [entry(MessageKind::Info, MessageSource::Field)];
            let (_, shapes) = with_painter(|q| {
                draw_log_pane(pane, &log_pane(&entries, &vitals, None), q, &m);
            });

            let y_of = |want: &str| {
                shapes
                    .iter()
                    .find_map(|cs| match &cs.shape {
                        bevy_egui::egui::Shape::Text(t) if t.galley.text().contains(want) => {
                            Some(t.pos.y)
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("{want:?} was never painted"))
            };
            // `INTEG`, not `MIT`: the strip drops segments from the end at
            // this window size, and MIT is past the cut here.
            let vitals_y = y_of("INTEG");
            let filter_y = y_of("LOG");
            let message_y = y_of("a line");

            assert!(
                vitals_y < pane.y,
                "the vitals drew at y={vitals_y}, below the pane's top edge at \
                 {} — they are not on the border",
                pane.y
            );
            assert!(
                filter_y >= pane.y,
                "the filter header drew at y={filter_y}, above the pane's top \
                 edge at {} — it is a border strip again",
                pane.y
            );
            assert!(
                filter_y > vitals_y,
                "the filter header at y={filter_y} and the vitals at y={vitals_y} \
                 share a line — two strips on one border cut each other in half"
            );
            assert!(
                message_y > filter_y,
                "the first message drew at y={message_y}, above the header at \
                 {filter_y} that names its channel"
            );
        });
    }

    /// Both of the pane's borders carry a strip, and the gutter tags the one
    /// line the fixture holds. All three come off one call.
    #[test]
    fn the_pane_draws_its_gutter_and_both_strips() {
        let m = ui_metrics(900.0);
        let status = wide_status();
        let vitals = Vitals {
            status: &status,
            mining: true,
        };
        let entries = [entry(MessageKind::Loot, MessageSource::Base)];
        let (_, shapes) = with_painter(|p| {
            draw_log_pane(
                crate::paint::Rect::new(0.0, 600.0, 900.0, 120.0),
                &log_pane(&entries, &vitals, Some("Nothing to collect.")),
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
