//! A town's job board — Phase 5. `render/contracts.rs`'s screen, one vendor
//! over, and it draws through that module's `contract_line` rather than a
//! second copy of it: a row that reads one way at the Broker's desk and
//! another at a town's counter is the failure to avoid, and the two screens
//! genuinely list the same thing.
//!
//! What differs is the header. A town's board says whose it is and how they
//! regard you, because the slot count *is* the standing — a Neutral town
//! posting two jobs beside an Allied one posting four is a difference the
//! player can only read if the page says which they are looking at.

use super::contracts::contract_line;
use super::popup::*;
use super::*;

/// `sections` is the pair app-core built, passed whole rather than as two
/// arguments — `contract_sections`' rule is that the handler and the
/// renderer read *one* list each, and splitting them here is one more place
/// they could be handed over swapped.
pub(super) fn draw_settlement_board(
    game: &mut Game,
    key: Option<SettlementKey>,
    sections: (&[ContractRow], &[ContractRow]),
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let (active, offers) = sections;
    // `draw_settlement`'s fallback, and for its reason: the subject is set
    // from either door the instant the screen opens, so `None` is "the
    // subject is gone" rather than a state a player reaches by playing.
    let Some(key) = key else {
        draw_popup(
            "Jobs",
            PopupSize::Small,
            &[text_row("Nothing to report.")],
            refusal,
            painter,
            m,
        );
        return;
    };
    let view = game.settlement_report(key);
    let rows = board_rows(&view, active, offers, selected);
    draw_popup("Jobs", PopupSize::Large, &rows, refusal, painter, m);
}

/// Every row the screen draws, out of a `SettlementView` and the two lists
/// alone — `settlement_page_rows`' split, for the censuses' sake.
pub(super) fn board_rows(
    view: &SettlementView,
    active: &[ContractRow],
    offers: &[ContractRow],
    selected: usize,
) -> Vec<Row> {
    let mut rows = vec![
        // `settlement_page_rows`' call to the same door the map glyph is
        // drawn through, not a second hand-copied orange.
        Row::TextColored(view.name.clone(), glyph_color(GlyphColor::Orange)),
        text_row(format!("They regard you as {}.", view.standing)),
        text_row(""),
    ];
    let mut idx = 0;

    rows.push(Row::TextColored("Held for them".to_string(), CYAN));
    if active.is_empty() {
        rows.push(text_row("    Nothing in hand."));
    }
    for contract in active {
        rows.push(contract_line(contract, idx, selected, true));
        rows.extend(description_rows(&contract.description));
        idx += 1;
    }

    rows.push(text_row(""));
    rows.push(Row::TextColored("Posted".to_string(), CYAN));
    if offers.is_empty() {
        // A closed board and an exhausted one are the same empty list, and
        // the standing line two rows above is what tells them apart —
        // `Standing::refuses_service` is the whole of the difference and it
        // is already on the page.
        rows.push(text_row("    Nothing posted."));
    }
    for contract in offers {
        rows.push(contract_line(contract, idx, selected, false));
        rows.extend(description_rows(&contract.description));
        idx += 1;
    }

    rows.push(text_row(""));
    rows.extend(board_footer().into_iter().map(text_row));
    rows
}

/// `contract_footer`'s two lines, worded for a counter you walked to.
/// Split out for its reason: a test measures exactly the strings the screen
/// pushes.
fn board_footer() -> [&'static str; 2] {
    [
        "Pick a number to take a job, or to hand over a held job's cargo.",
        "[A] gives back the highlighted job.        Esc to go back.",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;
    use feral_processes_engine::settlements::SettlementDb;
    use feral_processes_engine::tuning::{MAX_ACTIVE_CONTRACTS, SETTLEMENT_ALLIED_BOARD_SLOTS};
    use feral_processes_engine::{DifficultyMode, Game};

    fn a_view(name: &str) -> SettlementView {
        SettlementView {
            name: name.to_string(),
            kind: "Server",
            specialty: "Programs",
            temperament: "Open",
            blurb: String::new(),
            standing: "Neutral",
        }
    }

    /// The two things the header exists for: whose board this is, and how
    /// they regard you — the slot count *is* the standing, so a page that
    /// did not say which band it was drawing would leave the difference
    /// between two and four rows unreadable.
    #[test]
    fn the_header_names_the_town_and_how_it_regards_you() {
        let rows = board_rows(&a_view("Hollow Index"), &[], &[], usize::MAX);
        let joined = rows
            .iter()
            .filter_map(|r| match r {
                Row::Text(t) | Row::TextColored(t, _) => Some(t.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("Hollow Index"));
        assert!(joined.contains("Neutral"));
    }

    /// `settlement_page_rows`' own gate: the header's orange must be a call
    /// to the door the map glyph resolves through, not a second copy.
    #[test]
    fn the_header_wears_the_map_glyphs_own_orange() {
        let rows = board_rows(&a_view("Hollow Index"), &[], &[], usize::MAX);
        let Row::TextColored(text, color) = &rows[0] else {
            panic!("the header row must be first and must carry a colour");
        };
        assert_eq!(text, "Hollow Index");
        assert_eq!(*color, glyph_color(GlyphColor::Orange));
    }

    /// Both empty sections say so. A closed board and an exhausted one are
    /// the same empty list here — the standing line is what tells them
    /// apart, which is why it sits above both.
    #[test]
    fn an_empty_board_says_so_on_both_sections() {
        let rows = board_rows(&a_view("Hollow Index"), &[], &[], usize::MAX);
        let joined = rows
            .iter()
            .map(|r| match r {
                Row::Text(t) | Row::TextColored(t, _) | Row::Item { text: t, .. } => t.as_str(),
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("Nothing in hand."));
        assert!(joined.contains("Nothing posted."));
    }

    /// The page at its worst case, out of the real catalogues.
    ///
    /// **Taller than the Broker's**: an Allied town posts
    /// `SETTLEMENT_ALLIED_BOARD_SLOTS` jobs where a Broker posts three, and
    /// the page has no scroll — so the height this screen has to fit is
    /// genuinely a different number from the contracts screen's, and
    /// borrowing that screen's census would measure the wrong page.
    fn tallest_board_page() -> Vec<Row> {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(44, DifficultyMode::Forgiving, assets).expect("shipped assets");
        let catalogue = game.contract_catalogue();
        let mut longest = catalogue.clone();
        longest.sort_by_key(|row| {
            std::cmp::Reverse(row.description.chars().count() + row.name.chars().count())
        });
        assert!(!longest.is_empty(), "the shipped assets define contracts");

        let held: Vec<ContractRow> = longest
            .iter()
            .cloned()
            .cycle()
            .take(MAX_ACTIVE_CONTRACTS)
            .collect();
        let offers: Vec<ContractRow> = longest
            .iter()
            .cloned()
            .cycle()
            .take(SETTLEMENT_ALLIED_BOARD_SLOTS)
            .collect();

        let (db, warnings) =
            SettlementDb::load_dir(&std::path::Path::new(assets).join("settlements"))
                .expect("the catalogue loads");
        assert!(warnings.is_empty(), "{warnings:?}");
        let widest_name = db
            .iter()
            .max_by_key(|def| def.name.chars().count())
            .expect("the census must walk a real catalogue")
            .name
            .clone();

        board_rows(&a_view(&widest_name), &held, &offers, usize::MAX)
    }

    /// **The board's only extra chrome is its three-row header**, and that
    /// is the property worth pinning rather than an absolute fits-the-popup
    /// gate: `draw_contracts`, this page's twin, has never had one, because
    /// a numbered list whose rows carry authored prose genuinely can run
    /// past a 600px popup and the wrap is what bounds a *row*, not the page.
    ///
    /// What can be held is the delta. The page is already taller than the
    /// Broker's by four jobs (`SETTLEMENT_ALLIED_BOARD_SLOTS` against
    /// `CONTRACT_BOARD_SLOTS` and `MAX_ACTIVE_CONTRACTS`), so a fourth
    /// header row quietly added here — or a section label growing a line —
    /// has to fail somewhere, and this is that somewhere.
    #[test]
    fn the_boards_only_extra_chrome_is_its_three_row_header() {
        const HEADER_ROWS: usize = 3; // town name, standing, blank
        let board = board_rows(&a_view("T"), &[], &[], usize::MAX).len();
        let contracts =
            super::super::contracts::contract_rows_for_census(&[], &[], BrokerReach::AtBroker);
        assert_eq!(
            board,
            contracts + HEADER_ROWS,
            "the board spends {board} rows of chrome against the contracts screen's {contracts}"
        );
    }

    /// The other axis, and the one nothing clamps: `draw_row` clips a row
    /// vertically and never horizontally. Every row the page emits, item
    /// rows included — the header, the two section labels, the footer and
    /// the contract lines all measured through the screen's own builder.
    #[test]
    fn no_board_row_overflows_its_popup() {
        let rows = tallest_board_page();
        with_painter(|p| {
            let m = ui_metrics(900.0);
            // `PopupSize::Large`'s width fraction against the 1440x900
            // geometry `ui_metrics` is calibrated for.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            let mut measured = 0;
            for row in &rows {
                let line = match row {
                    Row::Text(t) | Row::TextColored(t, _) => t,
                    Row::Item { text, .. } => text,
                };
                measured += 1;
                let drawn = p.measure_ui_advance(line, m.font_size);
                assert!(
                    drawn <= room,
                    "a board row overflows the page by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
            assert!(measured > 0, "the census measured nothing");
        });
    }
}
