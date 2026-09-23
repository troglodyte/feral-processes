//! The alert board (`Mode::Alerts`), opened with `N` — a popup list over
//! `Game::alerts`, scrolled by `draw_popup`/`popup_layout` like every other
//! long list (correction 4 in the plan: the popup already scrolls, so this
//! file writes no scroll arithmetic of its own).

use feral_processes_engine::alerts::AlertKind;

use super::*;

pub(super) fn draw_alerts(
    game: &Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = alert_rows(&game.alerts(), selected);
    rows.push(text_row(""));
    rows.push(text_row(
        "[\u{2191}\u{2193}] select  [x/d] dismiss  [Esc] close",
    ));
    draw_popup("Alerts", PopupSize::Large, &rows, refusal, painter, m);
}

/// The stall palette for `MachineStalled`, `THREAT` for a sweep or a siege —
/// hostility and inbound harm, the same red the map reserves it for — and
/// the ordinary row colour for everything else (a downed program, a cut-off
/// site, a full Depot: blockers, not threats).
fn alert_color(kind: &AlertKind) -> Color {
    match kind {
        AlertKind::MachineStalled(status) => marks::machine_color(*status),
        AlertKind::SweepIncoming
        | AlertKind::SweepHit
        | AlertKind::SiegeIncoming
        | AlertKind::SiegeBegun => hud::palette::THREAT,
        AlertKind::ProgramDowned | AlertKind::SiteCutOff | AlertKind::DepotsFull => TEXT,
    }
}

/// One row per alert, newest first as `Game::alerts` already orders them.
/// An unread row carries a leading marker rather than a colour of its own —
/// `alert_color` already spends colour on *what kind* of alert this is, and
/// a second meaning on the same axis would collide with it the way
/// `fusion_color`'s doc comment warns against.
pub(super) fn alert_rows(
    alerts: &[feral_processes_engine::AlertView],
    selected: usize,
) -> Vec<Row> {
    if alerts.is_empty() {
        return vec![text_row("No alerts.")];
    }
    alerts
        .iter()
        .enumerate()
        .map(|(i, alert)| {
            let marker = if alert.unread { "* " } else { "  " };
            counted_item_row(
                format!("{marker}{}", alert.text),
                alert.count as usize,
                i == selected,
                alert_color(&alert.kind),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use feral_processes_engine::DifficultyMode;
    use feral_processes_engine::alerts::Alert;

    use super::*;
    use crate::paint::with_painter;
    use crate::render::popup::{popup_body_width, row_label_text};
    use crate::render::test_support::test_assets_dir;

    fn view(
        kind: AlertKind,
        text: &str,
        count: u32,
        unread: bool,
    ) -> feral_processes_engine::AlertView {
        feral_processes_engine::AlertView {
            kind,
            text: text.to_string(),
            count,
            unread,
        }
    }

    #[test]
    fn an_empty_board_reads_no_alerts() {
        let rows = alert_rows(&[], 0);
        assert_eq!(row_label_text(&rows[0]).trim(), "No alerts.");
    }

    #[test]
    fn an_unread_row_carries_a_marker_a_read_one_does_not() {
        let rows = alert_rows(
            &[
                view(AlertKind::DepotsFull, "Every Depot is full.", 1, true),
                view(
                    AlertKind::SweepHit,
                    "A GC Entropy Sweep hits the base.",
                    1,
                    false,
                ),
            ],
            0,
        );
        assert!(row_label_text(&rows[0]).contains("* Every Depot is full."));
        assert!(!row_label_text(&rows[1]).contains('*'));
    }

    /// `counted_item_row`'s `×N` rides `Row::Item::suffix`, drawn separately
    /// from the label (`draw_row`'s own `suffix_x`) — `row_label_text` never
    /// includes it, so this reads the field directly rather than the label.
    fn suffix_of(row: &Row) -> Option<String> {
        match row {
            Row::Item { suffix, .. } => suffix.clone(),
            _ => panic!("expected an Item row"),
        }
    }

    #[test]
    fn a_count_above_one_draws_the_multiplier() {
        let rows = alert_rows(
            &[view(AlertKind::DepotsFull, "Every Depot is full.", 3, true)],
            0,
        );
        assert_eq!(suffix_of(&rows[0]).as_deref(), Some("\u{d7}3"));
    }

    #[test]
    fn a_lone_alert_draws_no_multiplier() {
        let rows = alert_rows(
            &[view(AlertKind::DepotsFull, "Every Depot is full.", 1, true)],
            0,
        );
        assert_eq!(suffix_of(&rows[0]), None);
    }

    /// The stall palette, `THREAT`, and the plain row colour, one row of
    /// each — the three-way split `alert_color` makes.
    #[test]
    fn each_alert_family_takes_its_own_colour() {
        let rows = alert_rows(
            &[
                view(
                    AlertKind::MachineStalled(MachineStatus::Dry),
                    "The Lathe is out of fuel.",
                    1,
                    true,
                ),
                view(AlertKind::SiegeBegun, "Besiegers pour in!", 1, true),
                view(AlertKind::ProgramDowned, "Scout is down.", 1, true),
            ],
            0,
        );
        let color = |row: &Row| match row {
            Row::Item { color, .. } => *color,
            _ => panic!("expected an Item row"),
        };
        assert_eq!(color(&rows[0]), marks::machine_color(MachineStatus::Dry));
        assert_eq!(color(&rows[1]), hud::palette::THREAT);
        assert_eq!(color(&rows[2]), TEXT);
    }

    /// The longest sentence each source can post, plus the widest count
    /// suffix (`ALERT_BOARD_CAP` is 50, so `\u{d7}99` never really happens,
    /// but a two-digit multiplier is the worst case a real run can reach
    /// short of that cap and is what the row actually draws past 9). Built
    /// through a real `Game` for the pieces it can supply — the longest
    /// shipped structure name — and the sources' own fixed sentences
    /// (`systems.rs`, `game/base/upkeep.rs`, `game/base/work_orders.rs`,
    /// `game/base/hauling.rs`, `game/siege/*.rs`) copied verbatim, since
    /// those carry no per-run data at all.
    fn widest_alert_texts() -> Vec<String> {
        let game =
            Game::new(24601, DifficultyMode::Forgiving, &test_assets_dir()).expect("test game");
        let longest_structure = game
            .structure_defs()
            .into_iter()
            .max_by_key(|def| def.name.chars().count())
            .expect("at least one shipped structure")
            .name;
        vec![
            "A siege hits the base while you're away.".to_string(),
            // `work_orders.rs::announce_cut_off` — a build site cut off,
            // naming the structure on order and its cell.
            format!(
                "The {longest_structure} on order at {}, {} is cut off — no program can find a way to it.",
                -9999, -9999
            ),
            // `work_orders.rs::announce_dig_cut_off` — a dig site cut off,
            // naming only the cell: there is no structure to name.
            format!(
                "The marked cell at {}, {} is cut off — no program can find a way to it.",
                -9999, -9999
            ),
            // `systems.rs::set_machine_status`, `MachineStatus::Stranded` —
            // a built machine cut off from its own posted program, not a
            // site.
            format!("The {longest_structure} is cut off — its program can't find a way to it."),
            "Sweep telemetry thickens around the anchor. A GC Entropy Sweep is forming."
                .to_string(),
            "A hauled load has nowhere to go — no Depot will take it.".to_string(),
        ]
    }

    #[test]
    fn no_alert_row_overflows_the_popup_body_at_1280x720() {
        let texts = widest_alert_texts();
        let alerts: Vec<Alert> = texts
            .iter()
            .enumerate()
            .map(|(i, text)| Alert {
                kind: AlertKind::DepotsFull,
                subject: format!("subject-{i}"),
                text: text.clone(),
                count: 99,
                unread: true,
            })
            .collect();
        let views: Vec<feral_processes_engine::AlertView> = alerts
            .into_iter()
            .map(|a| feral_processes_engine::AlertView {
                kind: a.kind,
                text: a.text,
                count: a.count,
                unread: a.unread,
            })
            .collect();
        let rows = alert_rows(&views, 0);
        let m = crate::text::ui_metrics(720.0);
        let body = popup_body_width(1280.0, PopupSize::Large, &m);
        with_painter(|p| {
            for row in &rows {
                let label = row_label_text(row);
                // `suffix_x` places the `×99` one `m.inset` past the label's
                // own advance, drawn as a second run rather than folded into
                // `label` — so the row's full drawn width is the label plus
                // that gap plus the suffix, and a census reading `label`
                // alone would be short by exactly the count every row here
                // carries.
                let suffix_width = suffix_of(row)
                    .map(|s| m.inset + p.measure_ui_advance(&s, m.font_size))
                    .unwrap_or(0.0);
                let width = p.measure_ui_advance(&label, m.font_size) + suffix_width;
                assert!(
                    width <= body,
                    "an alert row draws {width}px into a {body}px body at 1280x720: {label:?}"
                );
            }
        });
    }
}
