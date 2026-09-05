//! The contracts screen: what the run holds, then what a Broker is offering.

use super::popup::*;
use super::*;

/// The board's own header. Off the base the offers are still listed — they
/// are the sector's, not the tile's — so this is where the screen says they
/// cannot be signed from here, rather than leaving the player to press a key
/// and read a refusal.
///
/// `onboarding` is the same errand one step earlier: the board really is
/// empty while the chain runs, and under a Broker the player has just built,
/// *Nothing on the board.* reads as the Broker being broken rather than as
/// the game waiting on them.
fn offered_header(reach: BrokerReach, onboarding: bool) -> String {
    if onboarding {
        return "Offered - finish your onboarding and the board opens up".to_string();
    }
    match reach {
        BrokerReach::AtBroker | BrokerReach::NoBroker => "Offered".to_string(),
        BrokerReach::OffBase => "Offered - return to your base to take one".to_string(),
    }
}

/// Two stacked sections, numbered continuously — active contracts first, then
/// the board's offers.
///
/// The two lists come in from app-core rather than being asked of the engine
/// here, because `App::handle_contracts_key` resolves a row number against
/// the same pair: a renderer that rebuilt them would drift out of index and
/// row 2 would act on a different contract from the one under the highlight.
pub(super) fn draw_contracts(
    active: &[ContractRow],
    offers: &[ContractRow],
    reach: BrokerReach,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let rows = contract_rows(active, offers, reach, selected);
    draw_popup("Contracts", PopupSize::Large, &rows, refusal, painter, m);
}

/// Every row the screen draws, built without touching a `Painter`.
///
/// Split out so the width censuses measure **what the screen emits** rather
/// than what a helper would have returned if it were called — a census that
/// calls `description_rows` itself stays green through a `draw_contracts`
/// that stopped calling it, which is the regression that put an unwrapped
/// paragraph on this screen in the first place.
fn contract_rows(
    active: &[ContractRow],
    offers: &[ContractRow],
    reach: BrokerReach,
    selected: usize,
) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut idx = 0;

    rows.push(Row::TextColored("Held".to_string(), CYAN));
    if active.is_empty() {
        rows.push(text_row("    Nothing in hand."));
    }
    for contract in active {
        rows.push(contract_line(contract, idx, selected, true));
        rows.extend(description_rows(&contract.description));
        idx += 1;
    }

    rows.push(text_row(""));
    // Derived from the rows the screen is already holding rather than asked
    // of the engine a second time, so the header and the list cannot
    // disagree — the same reason both lists come in from app-core.
    let onboarding = active.iter().any(|row| row.tutorial);
    rows.push(Row::TextColored(offered_header(reach, onboarding), CYAN));
    if offers.is_empty() {
        rows.push(text_row("    Nothing on the board."));
    }
    for contract in offers {
        rows.push(contract_line(contract, idx, selected, false));
        rows.extend(description_rows(&contract.description));
        idx += 1;
    }

    rows.push(text_row(""));
    rows.extend(contract_footer().into_iter().map(text_row));
    rows
}

/// The footer's two lines. Split out, like `rename_help`, so a test can
/// measure exactly the strings `draw_contracts` pushes.
///
/// Two lines rather than one: a single 122-character run-on overflowed
/// `PopupSize::Large`'s body, painting its last few characters over the map,
/// and buried the `[A]` abandon hint in its third clause behind two clauses
/// about an unrelated key. The abandon hint now leads its own line.
fn contract_footer() -> [&'static str; 2] {
    [
        "Pick a number to take an offer, or to hand over a held contract's cargo.",
        "[A] gives back the highlighted contract.        Esc to close.",
    ]
}

/// One contract's headline row. A held one shows its progress; an offer has
/// none to show, which is why the bar is not simply always drawn.
pub(super) fn contract_line(
    contract: &ContractRow,
    idx: usize,
    selected: usize,
    held: bool,
) -> Row {
    let progress = if held {
        format!(" [{}/{}]", contract.progress, contract.target)
    } else {
        String::new()
    };
    let mut row = item_row(
        format!(
            "[{}] {} - {}{progress} - pays {}",
            menu_shortcut(idx),
            contract.name,
            contract.objective_line,
            contract.reward_line
        ),
        idx == selected,
    );
    // Onboarding missions, and nothing else on this screen. `color` means
    // fusion tier on the gear screens and CRITICAL HP on the party screen;
    // the contracts screen has never used it, so green lands on a free axis
    // rather than becoming a second meaning on a loaded one.
    if contract.tutorial
        && let Row::Item { color, .. } = &mut row
    {
        *color = GREEN;
    }
    row
}

/// How many rows the contracts screen spends on chrome alone — headers, the
/// two empty-section lines, the blank separators and the footer.
///
/// Exists for `render/settlement_board.rs`'s parity census, and built by
/// calling `contract_rows` rather than counting the pushes by eye, which is
/// the only version of it that can catch a row added here later.
#[cfg(test)]
pub(super) fn contract_rows_for_census(
    active: &[ContractRow],
    offers: &[ContractRow],
    reach: BrokerReach,
) -> usize {
    contract_rows(active, offers, reach, usize::MAX).len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use feral_processes_engine::{DifficultyMode, Game};

    /// The board stays on screen off the base, so the header is the only
    /// thing that says the offers cannot be taken from here. A player who has
    /// to press a key to find that out has been shown a menu that lies.
    #[test]
    fn the_board_header_says_when_an_offer_cannot_be_taken_from_here() {
        assert_eq!(offered_header(BrokerReach::AtBroker, false), "Offered");
        assert_eq!(
            offered_header(BrokerReach::NoBroker, false),
            "Offered",
            "with no Broker the section is empty anyway, and `Nothing on the \
             board` is the line that speaks"
        );
        let away = offered_header(BrokerReach::OffBase, false);
        assert!(
            away.starts_with("Offered"),
            "the section still has to be recognisable as the board: {away:?}"
        );
        assert!(
            away.contains("base"),
            "the header names the errand: {away:?}"
        );
    }

    fn row(id: &str, tutorial: bool) -> ContractRow {
        ContractRow {
            issuer: None,
            issuer_name: None,
            id: id.into(),
            name: "A Contract".to_string(),
            description: "d".to_string(),
            objective_line: "Build a Home".to_string(),
            reward_line: "10 Credits".to_string(),
            progress: 0,
            target: 1,
            tutorial,
        }
    }

    fn row_color(contract: &ContractRow) -> Color {
        match contract_line(contract, 0, usize::MAX, true) {
            Row::Item { color, .. } => color,
            _ => panic!("contract_line builds an item row"),
        }
    }

    /// An onboarding mission draws green. It is the only row on this screen
    /// that is coloured at all, so the axis carries exactly one meaning here.
    #[test]
    fn an_onboarding_missions_row_is_green() {
        assert_eq!(row_color(&row("tutorial_first_light", true)), GREEN);
    }

    /// An ordinary contract is untouched.
    #[test]
    fn an_ordinary_contracts_row_is_not_green() {
        assert_ne!(row_color(&row("raw_stock", false)), GREEN);
    }

    /// A board that is empty *because onboarding owns it* has to say so.
    /// Under a Broker the player just built, *Nothing on the board.* reads as
    /// the Broker being broken.
    #[test]
    fn the_board_header_says_when_onboarding_owns_it() {
        let line = offered_header(BrokerReach::AtBroker, true);
        assert!(
            line.starts_with("Offered"),
            "still recognisable as the board: {line:?}"
        );
        assert!(
            line.len() > "Offered".len(),
            "and it names why there is nothing on it: {line:?}"
        );
    }

    /// A description longer than the popup is wide is wrapped, not run off
    /// the edge. The screen clips nothing horizontally, so an unwrapped
    /// paragraph simply leaves the popup and takes its own tail with it.
    #[test]
    fn a_long_description_wraps_rather_than_running_off_the_popup() {
        let long = "Mining by hand is a way to spend a run. Stand up a Mining Node \
                    and let it work while you do something else — post a program to \
                    it, then stay in the base a while and watch your crew haul what \
                    it cuts across to a buffer on their own.";
        let rows: Vec<Row> = description_rows(long).collect();
        assert!(rows.len() > 1, "a paragraph this long is more than one row");
        for row in &rows {
            let Row::Item { text, .. } = row else {
                panic!("`description_rows` builds item rows");
            };
            assert!(
                text.chars().count() <= DESCRIBE_WRAP_COLUMNS,
                "a wrapped line still overruns the wrap budget: {text:?}"
            );
        }
    }

    /// **No shipped contract's description overflows the popup either.**
    ///
    /// The row census below measures the `[n] Name - objective - pays` line
    /// and has never looked at the paragraph under it — which is how eleven
    /// onboarding missions, whose descriptions are several times longer than
    /// any other contract's, shipped against a green suite.
    #[test]
    fn no_shipped_contract_description_overflows_its_popup() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(43, DifficultyMode::Forgiving, assets).expect("shipped assets");

        // Through the screen's own row builder, so a `draw_contracts` that
        // stopped wrapping fails here.
        let catalogue = game.contract_catalogue();
        let widest = contract_rows(&catalogue, &[], BrokerReach::AtBroker, usize::MAX)
            .into_iter()
            .filter_map(|row| match row {
                Row::Item { text, .. } if text.starts_with(DESCRIPTION_INDENT) => Some(text),
                _ => None,
            })
            .max_by_key(|line| line.chars().count())
            .expect("the shipped assets define contracts");

        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            let drawn = p.measure_ui_advance(&widest, m.font_size);
            assert!(
                drawn > 0.0,
                "the census measured nothing — the shipped set has to reach here"
            );
            assert!(
                drawn <= room,
                "the widest description line overflows by {:.0}px \
                 ({drawn:.0} into {room:.0}):\n{widest}",
                drawn - room
            );
        });
    }

    /// **The widest contract row the shipped assets can build still fits.**
    ///
    /// The popup never wraps or clips horizontally — an overflowing row simply
    /// runs off the right edge, taking the payout with it, which is the same
    /// failure two open `TODO.md` bugs already record. Measured against the
    /// real font rather than counted in characters, because counting is what
    /// missed it there.
    ///
    /// Built from the real `assets/contracts/` rather than hand-written, which
    /// is the difference between a census and a fixture: the widest row is a
    /// property of the assets, so a long contract name or reward list added
    /// later has to fail here rather than be caught by eye.
    #[test]
    fn no_shipped_contract_row_overflows_its_popup() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(41, DifficultyMode::Forgiving, assets).expect("shipped assets");

        // Every shipped contract as though it were held, since a held row is
        // the wider of the two: it carries a progress figure the offer does
        // not. The counts are the widest the shipped set asks for.
        let widest = game
            .contract_catalogue()
            .into_iter()
            .map(|row| {
                let line = contract_line(&row, 35, usize::MAX, true);
                match line {
                    Row::Item { text, .. } => text,
                    _ => unreachable!("contract_line builds an item row"),
                }
            })
            .max_by_key(|row| row.chars().count())
            .expect("the shipped assets define contracts");

        with_painter(|p| {
            let m = ui_metrics(900.0);
            // `PopupSize::Large`'s body, matching `draw_popup`'s 0.88 width.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            let drawn = p.measure_ui_advance(format!("  {widest}"), m.font_size);
            assert!(
                drawn > 0.0,
                "the census measured nothing — the shipped set has to reach here"
            );
            assert!(
                drawn <= room,
                "the widest contract row overflows by {:.0}px \
                 ({drawn:.0} into {room:.0}):\n{widest}",
                drawn - room
            );
        });
    }

    /// The footer is `Row::Text`, never wrapped or clipped horizontally
    /// either, so it stays inside the same body the item census above
    /// measures against. Every line the screen draws, not just the first —
    /// a two-line footer that only checked line one would miss an overflow
    /// on line two.
    #[test]
    fn no_contract_footer_overflows_its_popup() {
        with_painter(|p| {
            let m = ui_metrics(900.0);
            // `PopupSize::Large`'s body, matching `draw_popup`'s 0.88 width.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            // The section headers ride along: they are drawn on this screen
            // and are the only other lines on it that are not built from a
            // contract row, which the census above already measures.
            let headers = [
                offered_header(BrokerReach::AtBroker, true),
                offered_header(BrokerReach::OffBase, false),
            ];
            for line in contract_footer()
                .into_iter()
                .map(String::from)
                .chain(headers)
            {
                let drawn = p.measure_ui_advance(&line, m.font_size);
                assert!(
                    drawn <= room,
                    "the contract footer overflows its popup by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
        });
    }
}
