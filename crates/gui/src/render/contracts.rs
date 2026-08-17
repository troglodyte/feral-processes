//! The contracts screen: what the run holds, then what a Broker is offering.

use super::popup::*;
use super::*;

/// The board's own header. Off the base the offers are still listed — they
/// are the sector's, not the tile's — so this is where the screen says they
/// cannot be signed from here, rather than leaving the player to press a key
/// and read a refusal.
fn offered_header(reach: BrokerReach) -> String {
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
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = Vec::new();
    let mut idx = 0;

    rows.push(Row::TextColored("Held".to_string(), CYAN));
    if active.is_empty() {
        rows.push(text_row("    Nothing in hand."));
    }
    for contract in active {
        rows.push(contract_line(contract, idx, selected, true));
        rows.push(text_row(format!("    {}", contract.description)));
        idx += 1;
    }

    rows.push(text_row(""));
    rows.push(Row::TextColored(offered_header(reach), CYAN));
    if offers.is_empty() {
        rows.push(text_row("    Nothing on the board."));
    }
    for contract in offers {
        rows.push(contract_line(contract, idx, selected, false));
        rows.push(text_row(format!("    {}", contract.description)));
        idx += 1;
    }

    rows.push(text_row(""));
    rows.push(text_row(
        "Pick an offer's key to take it, a held contract's to hand over its \
         cargo. [A] gives back the highlighted one. Esc to close",
    ));
    draw_popup("Contracts", PopupSize::Large, &rows, painter, m);
}

/// One contract's headline row. A held one shows its progress; an offer has
/// none to show, which is why the bar is not simply always drawn.
fn contract_line(contract: &ContractRow, idx: usize, selected: usize, held: bool) -> Row {
    let progress = if held {
        format!(" [{}/{}]", contract.progress, contract.target)
    } else {
        String::new()
    };
    item_row(
        format!(
            "[{}] {} - {}{progress} - pays {}",
            menu_shortcut(idx),
            contract.name,
            contract.objective_line,
            contract.reward_line
        ),
        idx == selected,
    )
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
        assert_eq!(offered_header(BrokerReach::AtBroker), "Offered");
        assert_eq!(
            offered_header(BrokerReach::NoBroker),
            "Offered",
            "with no Broker the section is empty anyway, and `Nothing on the \
             board` is the line that speaks"
        );
        let away = offered_header(BrokerReach::OffBase);
        assert!(
            away.starts_with("Offered"),
            "the section still has to be recognisable as the board: {away:?}"
        );
        assert!(
            away.contains("base"),
            "the header names the errand: {away:?}"
        );
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
}
