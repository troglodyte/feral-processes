//! Casting a field routine outside battle, and the always-visible list of
//! whatever is currently running — see `Game::field_routines` and
//! `Game::active_buffs`.

use super::battle::cell;
use super::popup::*;
use super::*;
use feral_processes_engine::ActiveBuffView;

/// Width of the buff name column, in monospace cells. Long enough for every
/// shipped routine's name with room to spare; a name that overruns clips
/// rather than shoving the magnitude that follows it out of the panel — the
/// same discipline `battle::cell` holds the roster's own NAME column to.
const BUFF_NAME_W: usize = 20;

/// One row per running buff, in the order the engine hands them back — see
/// `Game::active_buffs`. Sorting is out of scope; this draws whatever it is
/// given, in that order.
///
/// A companion-borne buff carries its holder as a trailing tag; the
/// player's carries none, since every other row on this panel not tagged is
/// already understood to be theirs. Ticks remaining ride in the row's
/// `suffix`, the same slot the history screen uses for a repeat count — an
/// annotation set apart from the row's own text rather than folded into it.
pub(super) fn buff_rows(buffs: &[ActiveBuffView]) -> Vec<Row> {
    buffs
        .iter()
        .map(|b| {
            let holder = b
                .holder_label
                .as_deref()
                .map(|h| format!(" ({h})"))
                .unwrap_or_default();
            Row::Item {
                text: format!("{} {}{holder}", cell(&b.name, BUFF_NAME_W), b.magnitude),
                selected: false,
                bold: false,
                color: TEXT,
                suffix: Some(format!("{}t", b.remaining)),
            }
        })
        .collect()
}

/// `buff_rows` drawn as a plain vertical list from `(x, y)`, no box or
/// title of its own — both panels below wrap it in whatever chrome their
/// screen already needs. Returns the y coordinate below the last row.
///
/// Rows are drawn via `draw_row` with `x` shifted left by `m.pad`: that
/// function's own `x + m.pad` then lands exactly on the `x` this was
/// called with, and since every row here has `selected: false`, the `w` it
/// takes for the (unused) highlight rect doesn't matter.
fn draw_buff_list(rows: &[Row], x: f32, mut y: f32, painter: &Painter, m: &Metrics) -> f32 {
    for row in rows {
        y = draw_row(row, x - m.pad, 0.0, y, f32::MAX, painter, m);
    }
    y
}

/// The map screen's copy of the panel — folded into the status column
/// between the party roster and the inventory list, rather than a floating
/// box of its own, since that column already is one. Draws nothing at all
/// when nothing is running, heading included, so the panel disappears
/// rather than leaving "Routines:" over an empty list. Returns the y
/// coordinate the caller's next section should start from.
pub(super) fn draw_status_buffs(
    buffs: &[ActiveBuffView],
    x: f32,
    y: f32,
    painter: &Painter,
    m: &Metrics,
) -> f32 {
    let rows = buff_rows(buffs);
    if rows.is_empty() {
        return y;
    }
    painter.ui("Routines:", x, y, m.font_size, TEXT);
    let cy = draw_buff_list(&rows, x, y + m.line_height, painter, m);
    cy + m.gap
}

/// Pixel width a boxed buff panel needs for `rows` against `title` — the
/// widest of the two, measured rather than guessed. `buff_rows` bounds the
/// name column, but a companion's holder tag is a player-chosen name with
/// no length limit of its own, so only measuring the actual content is
/// honest about how wide the box has to be.
fn buff_panel_width(rows: &[Row], title: &str, painter: &Painter, m: &Metrics) -> f32 {
    let mut width = painter.measure_ui(title, m.font_size).width;
    for row in rows {
        if let Row::Item { text, suffix, .. } = row {
            let mut w = painter.measure_ui(format!("  {text}"), m.font_size).width;
            if let Some(suffix) = suffix {
                w += m.inset + painter.measure_ui(suffix, m.font_size).width;
            }
            width = width.max(w);
        }
    }
    width
}

/// The battle screen's copy of the panel — a small bordered box tucked into
/// the top-right corner of the narration pane between the two rosters,
/// anchored by its own right edge (`top_right_x`) since the box's width
/// depends on its content. Drawn last, over whatever narration is showing
/// underneath: the pane is flavor text, not tactical data, so an occasional
/// overlap there costs far less than putting this over the HP/DECOMP
/// columns either roster owns. Draws nothing when nothing is running.
pub(super) fn draw_battle_buffs(
    buffs: &[ActiveBuffView],
    top_right_x: f32,
    top_y: f32,
    painter: &Painter,
    m: &Metrics,
) {
    let rows = buff_rows(buffs);
    if rows.is_empty() {
        return;
    }
    const TITLE: &str = "Routines";
    let content_w = buff_panel_width(&rows, TITLE, painter, m);
    let w = content_w + m.pad * 2.0;
    let h = m.line_height * (rows.len() as f32 + 1.0) + m.inset;
    let x = top_right_x - w;
    painter.rect(x, top_y, w, h, PANEL_BG);
    painter.rect_lines(x, top_y, w, h, 2.0, BORDER);
    let mut cy = top_y + m.font_size as f32;
    painter.ui(TITLE, x + m.pad, cy, m.font_size, CYAN);
    cy += m.line_height;
    draw_buff_list(&rows, x + m.pad, cy, painter, m);
}

/// Which installed field routine to run — a `FieldBuff` ability on you or a
/// program you own, cast outside battle. Reached with `a` from
/// `Mode::Playing`; rows come straight from `Game::field_routines`, same
/// contract the battle action bar holds to.
pub(super) fn draw_field_cast(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let routines = game.field_routines();
    let mut rows = vec![text_row("Run which routine?")];
    if routines.is_empty() {
        rows.push(text_row(
            "(no field routines installed — install one from the routines menu)",
        ));
    }
    for (i, r) in routines.iter().enumerate() {
        let label = format!(
            "[{}] {} — {:.0} PWR — {} ({})",
            menu_shortcut(i),
            r.name,
            r.power_cost,
            r.description,
            r.holder_label,
        );
        // Greyed rather than hidden, and still selectable: the engine
        // refuses it again on commit with the reason in `App::status_line`,
        // and a row that vanished would leave the player wondering where a
        // routine they installed went — same call `draw_battle_special_menu`
        // makes for an unaffordable Special.
        rows.push(if r.affordable {
            item_row(label, i == selected)
        } else {
            spent_item_row(format!("{label} — not enough Power"), i == selected)
        });
    }
    draw_popup("Run a Routine", PopupSize::Large, &rows, painter, m);
}

/// Who a `OneAlly` field routine picked in `Mode::FieldCast` lands on.
/// Offers the same "you, then every program you own" list
/// `draw_routine_target` does, via `Game::routine_holders` — the picker
/// itself carries no notion of who owns what, that's enforced by
/// `App::field_ally_options` before the cast ever reaches the engine.
pub(super) fn draw_field_cast_ally(
    game: &mut Game,
    pending: Option<usize>,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(pending) = pending else { return };
    let routines = game.field_routines();
    let Some(routine) = routines.get(pending) else {
        return;
    };
    let mut rows = vec![text_row(format!("Run {} on whom?", routine.name))];
    for (i, h) in game.routine_holders().into_iter().enumerate() {
        rows.push(item_row(
            format!("[{}] {} Lv{}", menu_shortcut(i), h.name, h.level),
            i == selected,
        ));
    }
    draw_popup("Run a Routine", PopupSize::Large, &rows, painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buff(name: &str, magnitude: &str, remaining: u32, holder: Option<&str>) -> ActiveBuffView {
        ActiveBuffView {
            name: name.to_string(),
            magnitude: magnitude.to_string(),
            remaining,
            holder_label: holder.map(str::to_string),
        }
    }

    fn text_of(row: &Row) -> &str {
        match row {
            Row::Item { text, .. } => text,
            _ => panic!("expected an item row, got a text row"),
        }
    }

    fn suffix_of(row: &Row) -> Option<&str> {
        match row {
            Row::Item { suffix, .. } => suffix.as_deref(),
            _ => None,
        }
    }

    /// Chars, not bytes — `cell` can pad the name with a multi-byte `…`,
    /// same reason `battle.rs`'s own column tests read this way.
    fn at(s: &str, col: usize) -> String {
        s.chars().skip(col).collect()
    }

    #[test]
    fn one_row_per_buff_in_the_order_given() {
        let buffs = vec![
            buff("Hardened Shell", "DEF+2", 5, None),
            buff("Trace Analysis", "XP+15%", 3, None),
        ];
        let rows = buff_rows(&buffs);
        assert_eq!(rows.len(), 2, "one row per buff");
        assert!(text_of(&rows[0]).contains("Hardened Shell"));
        assert!(text_of(&rows[1]).contains("Trace Analysis"));
    }

    #[test]
    fn only_a_companion_borne_buff_carries_a_holder_tag() {
        let buffs = vec![
            buff("Hardened Shell", "DEF+2", 5, None),
            buff("Data Cache", "HP+1/t", 5, Some("Sparkgrub")),
        ];
        let rows = buff_rows(&buffs);
        assert!(
            !text_of(&rows[0]).contains('('),
            "the player's own buff picked up a holder tag: {:?}",
            text_of(&rows[0])
        );
        assert!(
            text_of(&rows[1]).contains("(Sparkgrub)"),
            "a companion's buff needs its holder tag: {:?}",
            text_of(&rows[1])
        );
    }

    /// The panel hides entirely when nothing is running — not an empty box,
    /// not a "None" placeholder — so an empty list has to mean zero rows,
    /// not one saying so.
    #[test]
    fn no_active_buffs_produces_no_rows() {
        assert!(buff_rows(&[]).is_empty());
    }

    /// The magnitude sits at the same fixed column whether the name ahead
    /// of it is short or long enough to clip — the failure this exists to
    /// prevent is a long name shoving "DEF+2" off to the right by however
    /// many characters it overran by.
    #[test]
    fn a_long_buff_name_does_not_push_the_magnitude_out_of_the_panel() {
        let rows = buff_rows(&[
            buff("Shell", "DEF+2", 5, None),
            buff(
                "A Very Long Field Routine Name That Overruns The Column",
                "DEF+2",
                5,
                None,
            ),
        ]);
        let magnitude_col = BUFF_NAME_W + 1;
        assert!(at(text_of(&rows[0]), magnitude_col).starts_with("DEF+2"));
        assert!(
            at(text_of(&rows[1]), magnitude_col).starts_with("DEF+2"),
            "the long name pushed the magnitude out of column {magnitude_col}: {:?}",
            text_of(&rows[1])
        );
        assert!(
            text_of(&rows[1]).contains('…'),
            "the clipped name has to show it was clipped"
        );
    }

    #[test]
    fn remaining_ticks_ride_in_the_rows_suffix() {
        let rows = buff_rows(&[buff("Shell", "DEF+2", 7, None)]);
        assert_eq!(suffix_of(&rows[0]), Some("7t"));
    }
}
