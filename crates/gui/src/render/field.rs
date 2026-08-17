//! Casting a field routine outside battle, and the always-visible list of
//! whatever is currently running — see `Game::field_routines` and
//! `Game::active_buffs`.

use super::battle::cell;
use super::popup::*;
use super::*;
use feral_processes_engine::{ActiveBuffView, FieldCastTargetView};

/// Width of the buff name column, in monospace cells. Long enough for every
/// shipped routine's name with room to spare; a name that overruns clips
/// rather than shoving the magnitude that follows it out of the panel — the
/// same discipline `battle::cell` holds the roster's own NAME column to.
const BUFF_NAME_W: usize = 20;

/// How many rows the battle panel ever shows at once, indicator row
/// included. The narration pane it sits in front of is already only a
/// handful of lines on most windows, and the panel is drawn over it — a
/// box that grew with every routine the party had running would eventually
/// swallow the whole pane. Four is enough to show a small loadout at a
/// glance without that; anything past it rolls into the "N more" row, and
/// the full list is always available on the map.
const BATTLE_BUFF_ROW_CAP: usize = 4;

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
                suffix: Some(b.remaining.clone()),
                icon: None,
            }
        })
        .collect()
}

/// Caps `rows` at `limit` total rows, trading the last slot for a "N more"
/// indicator when the list doesn't fit. Shared by both panels — a fixed
/// display cap for the battle box, however many lines are left in the
/// status column for the map panel — so a truncated list reads as one
/// feature in either place rather than two different-looking cutoffs. A
/// list that already fits within `limit` is returned untouched, indicator
/// and all, so nothing here fires when there's nothing to hide.
fn cap_rows(rows: Vec<Row>, limit: usize) -> Vec<Row> {
    if rows.len() <= limit {
        return rows;
    }
    let keep = limit.saturating_sub(1);
    let hidden = rows.len() - keep;
    let mut capped: Vec<Row> = rows.into_iter().take(keep).collect();
    capped.push(Row::TextColored(format!("+{hidden} more"), TEXT_DIM));
    capped
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
///
/// `max_y` is the same bound the inventory list right below this already
/// clips its own rows against (the status column's keybind footer) — a
/// well-buffed party with routines running on every slot of every holder
/// can outgrow the column same as a full inventory can, and this section
/// used to have no guard of its own where that one does. Requires room for
/// the heading plus at least one more line before drawing anything at all,
/// so there's never a heading left standing over nothing: the routines
/// section is dropped whole rather than left half-drawn.
pub(super) fn draw_status_buffs(
    buffs: &[ActiveBuffView],
    x: f32,
    y: f32,
    max_y: f32,
    painter: &Painter,
    m: &Metrics,
) -> f32 {
    let rows = buff_rows(buffs);
    if rows.is_empty() || y + m.line_height * 2.0 > max_y {
        return y;
    }
    painter.ui("Routines:", x, y, m.font_size, TEXT);
    let heading_bottom = y + m.line_height;
    let slots = ((max_y - heading_bottom) / m.line_height).floor() as usize;
    let cy = draw_buff_list(&cap_rows(rows, slots), x, heading_bottom, painter, m);
    cy + m.gap
}

/// Pixel width a boxed buff panel needs for `rows` against `title` — the
/// widest of the two, measured rather than guessed. `buff_rows` bounds the
/// name column, but a companion's holder tag is a player-chosen name with
/// no length limit of its own, so only measuring the actual content is
/// honest about how wide the box has to be. Covers the "N more" indicator
/// `cap_rows` can append (`Row::TextColored`) too, so a truncated list
/// never gets a box too narrow for its own last line.
fn buff_panel_width(rows: &[Row], title: &str, painter: &Painter, m: &Metrics) -> f32 {
    let mut width = painter.measure_ui(title, m.font_size).width;
    for row in rows {
        let w = match row {
            // By advance, matching how `draw_row` actually lays the row out —
            // the two-space prefix has no ink, so an ink measurement here
            // would size the box too narrow and push the suffix past its
            // right border. See `Painter::measure_ui_advance`.
            Row::Item { text, suffix, .. } => {
                let mut w = painter.measure_ui_advance(format!("  {text}"), m.font_size);
                if let Some(suffix) = suffix {
                    w += m.inset + painter.measure_ui_advance(suffix, m.font_size);
                }
                w
            }
            Row::Text(s) | Row::TextColored(s, _) => painter.measure_ui(s, m.font_size).width,
        };
        width = width.max(w);
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
///
/// Bounded to `BATTLE_BUFF_ROW_CAP` rows regardless of how many buffs are
/// actually running: the box overlaps narration from the moment any line
/// has revealed, not only once it's full, so letting it grow with the
/// party's whole routine loadout would make the overlap worse the more
/// buffs anyone kept up. In battle the player needs to know *that* buffs
/// are running and roughly which; the full list is one keystroke away on
/// the map.
pub(super) fn draw_battle_buffs(
    buffs: &[ActiveBuffView],
    top_right_x: f32,
    top_y: f32,
    painter: &Painter,
    m: &Metrics,
) {
    let rows = cap_rows(buff_rows(buffs), BATTLE_BUFF_ROW_CAP);
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
        // `cost` arrives already carrying its unit, which is the engine's to
        // decide, not this file's.
        let label = format!(
            "[{}] {} — {} — {} ({})",
            menu_shortcut(i),
            r.name,
            r.cost,
            r.description,
            r.holder_label,
        );
        // Greyed rather than hidden, and still selectable: the engine
        // refuses it again on commit with the reason in `App::status_line`,
        // and a row that vanished would leave the player wondering where a
        // routine they installed went — same call `draw_battle_special_menu`
        // makes for an unavailable Special.
        rows.push(match &r.unavailable {
            None => item_row(label, i == selected),
            Some(reason) => spent_item_row(format!("{label} — {reason}"), i == selected),
        });
    }
    draw_popup("Run a Routine", PopupSize::Large, &rows, painter, m);
}

/// Who a `OneAlly` field routine picked in `Mode::FieldCast` lands on: you,
/// then your active `Party`, via `Game::field_cast_targets` — narrower than
/// `draw_routine_target`'s "you, then every program you own"
/// (`Game::routine_holders`), since only the player and the party are ever
/// ticked. `App::field_ally_options` calls the same `Game::field_cast_targets`
/// to resolve what row was picked, so this list and the target the engine
/// actually casts on can't disagree.
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
    for (i, t) in game.field_cast_targets(pending).into_iter().enumerate() {
        let mut lines = field_cast_target_rows(menu_shortcut(i), &t).into_iter();
        let head = lines
            .next()
            .expect("field_cast_target_rows always emits the target's row");
        rows.push(with_icon(
            item_row(head, i == selected),
            t.glyph,
            glyph_color(t.color),
        ));
        for line in lines {
            rows.push(colored_item_row(line, false, TEXT_DIM));
        }
    }
    draw_popup("Run a Routine", PopupSize::Large, &rows, painter, m);
}

/// One ally-picker target's lines: the row its shortcut selects, priced with
/// the stats the buff is about to land on, then the buff this cast would
/// displace underneath.
///
/// The stats sit on the row itself in the format `fuse_candidate_label`
/// already uses, because both screens are the same kind of decision — every
/// `OneAlly` routine the game ships is about a stat, and the picker's job is
/// choosing whose. The displaced buff goes to a continuation line instead:
/// `arm_field_buff` drops a running `Routine` buff of the same kind, so the
/// tag is the one thing on this screen saying the cast is a replacement
/// rather than an addition — but it is also 30-odd cells of routine name and
/// magnitude on a row already carrying five numbers, and `draw_row` clamps a
/// row vertically and nothing clamps it horizontally. A target carrying
/// nothing of the kind gets no line at all.
///
/// Returns the lines rather than drawing them so their width is measurable
/// without a window — see `the_widest_ally_picker_row_fits_its_popup`.
fn field_cast_target_rows(num: char, t: &FieldCastTargetView) -> Vec<String> {
    let displaced = t.running.as_ref().map(|b| {
        format!(
            "replaces {} {} — {}t left",
            b.name, b.magnitude, b.remaining
        )
    });
    std::iter::once(format!(
        "[{num}] {} Lv{} - HP {}/{}  ATK {}  DEF {}  PWR {}",
        t.name, t.level, t.hp, t.max_hp, t.atk, t.def, t.power
    ))
    .chain(continuation_lines(displaced.as_deref().unwrap_or_default()))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;
    use feral_processes_engine::RunningBuffView;

    fn buff(name: &str, magnitude: &str, remaining: &str, holder: Option<&str>) -> ActiveBuffView {
        ActiveBuffView {
            name: name.to_string(),
            magnitude: magnitude.to_string(),
            remaining: remaining.to_string(),
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
            buff("Hardened Shell", "DEF+2", "5t", None),
            buff("Trace Analysis", "XP+15%", "3t", None),
        ];
        let rows = buff_rows(&buffs);
        assert_eq!(rows.len(), 2, "one row per buff");
        assert!(text_of(&rows[0]).contains("Hardened Shell"));
        assert!(text_of(&rows[1]).contains("Trace Analysis"));
    }

    #[test]
    fn only_a_companion_borne_buff_carries_a_holder_tag() {
        let buffs = vec![
            buff("Hardened Shell", "DEF+2", "5t", None),
            buff("Data Cache", "HP+1/t", "5t", Some("Sparkgrub")),
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
            buff("Shell", "DEF+2", "5t", None),
            buff(
                "A Very Long Field Routine Name That Overruns The Column",
                "DEF+2",
                "5t",
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
        let rows = buff_rows(&[buff("Shell", "DEF+2", "7t", None)]);
        assert_eq!(suffix_of(&rows[0]), Some("7t"));
    }

    /// The map's copy of this panel measures nothing — `draw_status_buffs`
    /// draws into the status column and `draw_row` clips rows vertically but
    /// never horizontally, so a row too wide runs off the panel silently.
    /// `"until rest"` is seven characters longer than the `"90t"` it replaced
    /// on eight shipped routines, which is exactly the kind of change that
    /// pays for itself off-screen.
    ///
    /// Bounds the **player's own** rows only. A companion-borne row carries a
    /// trailing holder tag and already overran this column by ~200px before
    /// any of this — measured, not assumed — so asserting on one here would
    /// be pinning a bug rather than a guarantee. `TODO.md` records it.
    #[test]
    fn the_widest_until_rest_buff_row_fits_the_status_column() {
        use feral_processes_engine::abilities::{AbilityDb, AbilityEffect};

        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/abilities");
        let (db, warnings) = AbilityDb::load_dir(&dir).expect("the abilities load");
        assert!(warnings.is_empty(), "{warnings:?}");

        let rows: Vec<ActiveBuffView> = db
            .all()
            .filter_map(|d| match d.effect {
                AbilityEffect::FieldBuff {
                    kind,
                    power,
                    duration,
                    interval,
                } => {
                    let lifetime = if kind.runs_until_rest() {
                        "rest".to_string()
                    } else {
                        format!("{duration}t")
                    };
                    Some(buff(
                        &d.name,
                        &kind.magnitude_label(power, interval),
                        &lifetime,
                        // No holder tag: a companion-borne row already
                        // overflows this column by ~200px with any suffix at
                        // all, which is a pre-existing bug of its own (see
                        // `TODO.md`) rather than something this tag caused.
                        None,
                    ))
                }
                _ => None,
            })
            .collect();
        assert!(!rows.is_empty(), "the shipped set has field buffs to draw");

        with_painter(|p| {
            let m = ui_metrics(900.0);
            // The status panel is the window's width less the map pane's
            // `PANE_W`, and `draw_status_buffs` is called one inset in — the
            // same 1440x900 geometry `ui_metrics` is calibrated for.
            let room = 1440.0 * (1.0 - super::super::base::PANE_W) - m.inset * 2.0;
            for row in buff_rows(&rows) {
                let Row::Item { text, suffix, .. } = &row else {
                    continue;
                };
                let mut drawn = p.measure_ui_advance(format!("  {text}"), m.font_size);
                if let Some(suffix) = suffix {
                    drawn += m.inset + p.measure_ui_advance(suffix, m.font_size);
                }
                assert!(
                    drawn <= room,
                    "a buff row overflows the status column by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{text} [{suffix:?}]",
                    drawn - room
                );
            }
        });
    }

    fn indicator_of(row: &Row) -> Option<&str> {
        match row {
            Row::TextColored(s, _) => Some(s),
            _ => None,
        }
    }

    fn named_buffs(names: &[&str]) -> Vec<ActiveBuffView> {
        names.iter().map(|n| buff(n, "DEF+2", "5t", None)).collect()
    }

    /// The battle panel's own bound: more buffs than `BATTLE_BUFF_ROW_CAP`
    /// gives up the last slot for a "+N more" line rather than growing the
    /// box past its cap. Checked by emission order, not just the count —
    /// row-count fixtures have hidden a real overflow in this repo before.
    #[test]
    fn a_list_longer_than_the_battle_cap_emits_exactly_the_cap_including_the_indicator() {
        let buffs = named_buffs(&["Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot"]);
        let rows = cap_rows(buff_rows(&buffs), BATTLE_BUFF_ROW_CAP);
        assert_eq!(
            rows.len(),
            BATTLE_BUFF_ROW_CAP,
            "the box never exceeds its cap"
        );
        // The first BATTLE_BUFF_ROW_CAP - 1 rows are real buffs, in the
        // order they were given, and the last one is the indicator — not
        // the other way around, and not some real row dropped from the
        // middle to make room.
        assert!(text_of(&rows[0]).contains("Alpha"));
        assert!(text_of(&rows[1]).contains("Bravo"));
        assert!(text_of(&rows[2]).contains("Charlie"));
        assert_eq!(
            indicator_of(&rows[3]),
            Some("+3 more"),
            "6 buffs, 3 shown, 3 left off"
        );
    }

    /// At or under the cap, nothing is hidden and no indicator is drawn —
    /// the box is exactly the buffs that are running, same as it always
    /// was before the cap existed.
    #[test]
    fn a_list_at_or_under_the_battle_cap_emits_no_indicator() {
        let at_cap = named_buffs(&["Alpha", "Bravo", "Charlie", "Delta"]);
        let rows = cap_rows(buff_rows(&at_cap), BATTLE_BUFF_ROW_CAP);
        assert_eq!(rows.len(), 4);
        assert!(rows.iter().all(|r| indicator_of(r).is_none()));
        assert!(text_of(&rows[0]).contains("Alpha"));
        assert!(text_of(&rows[3]).contains("Delta"));

        let under_cap = named_buffs(&["Alpha", "Bravo"]);
        let rows = cap_rows(buff_rows(&under_cap), BATTLE_BUFF_ROW_CAP);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| indicator_of(r).is_none()));
    }

    /// The map panel's bound is however many lines are left in the status
    /// column rather than a fixed constant, but it's the same `cap_rows`
    /// underneath — this pins the mechanism `draw_status_buffs` calls with
    /// a representative "3 lines left" budget, the same way the battle
    /// tests above pin it with `BATTLE_BUFF_ROW_CAP`.
    #[test]
    fn a_list_longer_than_the_available_map_slots_emits_exactly_that_many_including_the_indicator()
    {
        let buffs = named_buffs(&["Alpha", "Bravo", "Charlie", "Delta", "Echo"]);
        let slots = 3;
        let rows = cap_rows(buff_rows(&buffs), slots);
        assert_eq!(rows.len(), slots);
        assert!(text_of(&rows[0]).contains("Alpha"));
        assert!(text_of(&rows[1]).contains("Bravo"));
        assert_eq!(
            indicator_of(&rows[2]),
            Some("+3 more"),
            "5 buffs, 2 shown, 3 left off"
        );
    }

    #[test]
    fn a_list_that_fits_the_available_map_slots_emits_no_indicator() {
        let buffs = named_buffs(&["Alpha", "Bravo", "Charlie"]);
        let rows = cap_rows(buff_rows(&buffs), 3);
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|r| indicator_of(r).is_none()));

        let rows = cap_rows(buff_rows(&buffs), 10);
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|r| indicator_of(r).is_none()));
    }

    fn target(name: &str, running: Option<RunningBuffView>) -> FieldCastTargetView {
        FieldCastTargetView {
            entity: Entity::PLACEHOLDER,
            glyph: 'p',
            color: GlyphColor::White,
            name: name.to_string(),
            level: 6,
            hp: 22,
            max_hp: 28,
            atk: 8,
            def: 5,
            power: 19,
            running,
        }
    }

    /// Every `OneAlly` routine the game ships is about a stat — Regen about
    /// HP, Overclock about ATK, the other two about DEF — so the numbers are
    /// what the pick is actually made on. Without them the only way to
    /// choose is to remember them.
    #[test]
    fn an_ally_picker_row_prices_the_target_by_its_stats() {
        let lines = field_cast_target_rows('a', &target("Kestrel", None));

        assert_eq!(lines.len(), 1, "nothing running means no second line");
        assert!(lines[0].contains("[a] Kestrel Lv6"), "{lines:?}");
        assert!(lines[0].contains("HP 22/28"), "{lines:?}");
        assert!(lines[0].contains("ATK 8"), "{lines:?}");
        assert!(lines[0].contains("DEF 5"), "{lines:?}");
        assert!(lines[0].contains("PWR 19"), "{lines:?}");
    }

    /// The tag names the routine and how long it has left, because the cast
    /// *replaces* it rather than stacking with it (`Game::arm_field_buff`)
    /// — and two different routines can arm one kind, so "already running"
    /// alone would not say which is about to go.
    #[test]
    fn an_ally_picker_row_says_what_the_cast_would_replace() {
        let lines = field_cast_target_rows(
            'b',
            &target(
                "Kestrel",
                Some(RunningBuffView {
                    name: "Repair Loop Single".to_string(),
                    magnitude: "HP+7/4t".to_string(),
                    remaining: "62t".to_string(),
                }),
            ),
        );

        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[1].contains("Repair Loop Single"), "{lines:?}");
        assert!(lines[1].contains("HP+7/4t"), "{lines:?}");
        assert!(lines[1].contains("62t"), "{lines:?}");
        assert!(
            lines[1].starts_with(' '),
            "the tag is indented under the row it belongs to: {lines:?}"
        );
    }

    /// Measured against the real ability set rather than a literal, so an
    /// author naming a field routine longer fails this instead of shipping a
    /// line that runs off the box.
    ///
    /// It bounds the *shipped* half only. A target's name is
    /// `Game::creature_label`, which can carry a player-authored custom
    /// name — the same unbounded row `TODO.md`'s Bug 2 records against the
    /// Party popup, and no census over the assets can reach it.
    #[test]
    fn the_widest_ally_picker_row_fits_its_popup() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/abilities");
        let (db, warnings) = feral_processes_engine::abilities::AbilityDb::load_dir(&dir)
            .expect("the abilities load");
        assert!(warnings.is_empty(), "{warnings:?}");
        let widest = db
            .all()
            .filter(|d| {
                matches!(
                    d.effect,
                    feral_processes_engine::abilities::AbilityEffect::FieldBuff { .. }
                )
            })
            .max_by_key(|d| d.name.chars().count())
            .expect("the shipped set has field buffs to name")
            .name
            .clone();

        let lines = field_cast_target_rows(
            'a',
            &FieldCastTargetView {
                // Four digits apiece: past anything reachable, so the census
                // is not one level-up away from being wrong.
                hp: 9999,
                max_hp: 9999,
                atk: 9999,
                def: 9999,
                power: 9999,
                level: 99,
                running: Some(RunningBuffView {
                    name: widest,
                    magnitude: "HP+9999/99t".to_string(),
                    remaining: "9999t".to_string(),
                }),
                ..target("Overclocked Sentinel [z10]", None)
            },
        );

        with_painter(|p| {
            let m = ui_metrics(900.0);
            // 0.88 is `PopupSize::Large`'s width fraction, against the
            // 1440x900 geometry `ui_metrics` is calibrated for.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for line in &lines {
                let drawn = p.measure_ui_advance(line, m.font_size);
                assert!(
                    drawn <= room,
                    "an ally-picker line overflows the popup by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
        });
    }
}
