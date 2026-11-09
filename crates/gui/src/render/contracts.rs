//! The Contracts page's two tabs: Held, what the run holds and who for,
//! and Board, what a Broker is offering. Drawn in the hub's frame.

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

/// The Held tab: every contract in hand, each under its issuer.
///
/// The list comes in from app-core rather than being asked of the engine
/// here, because `App::handle_contracts_key` resolves a row number against
/// the same list: a renderer that rebuilt it would drift out of index and
/// row 2 would act on a different contract from the one under the highlight.
pub(super) fn held_rows(active: &[ContractRow], selected: usize) -> Vec<Row> {
    let mut rows = Vec::new();
    if active.is_empty() {
        rows.push(text_row("Nothing in hand."));
    }
    for (idx, contract) in active.iter().enumerate() {
        rows.extend(page_entry(contract, idx, selected, true));
    }
    rows
}

/// Who a held contract is for: the run's own Broker, or the town that
/// posted it, which is also where it is handed in.
fn issuer_line(issuer: Option<&str>) -> String {
    match issuer {
        None => "Broker".to_string(),
        Some(town) => format!("for {town} · hand in there"),
    }
}

/// The Board tab: the Broker's offers under the header that says whether
/// they can be signed from here. `onboarding` is whether a held contract is
/// an onboarding mission, read off the Held list app-core already built so
/// the header and the list cannot disagree.
pub(super) fn board_rows(
    offers: &[ContractRow],
    onboarding: bool,
    reach: BrokerReach,
    selected: usize,
) -> Vec<Row> {
    let mut rows = vec![Row::TextColored(offered_header(reach, onboarding), CYAN)];
    if offers.is_empty() {
        rows.push(text_row("    Nothing on the board."));
    }
    for (idx, contract) in offers.iter().enumerate() {
        rows.extend(page_entry(contract, idx, selected, false));
    }
    rows
}

/// How and where the objective is satisfied, under the authored prose - see
/// `Game::objective_hint`, which is where the words are decided and whether
/// there are any.
///
/// Drawn through `description_rows`, so it wraps at the same column and wears
/// the same `is_description_row` shape the description census filters on: a hint too
/// long for the body fails that census rather than running off the right edge.
/// The arrow is the whole of what separates it from the flavour text above,
/// deliberately - `color` on this screen already means "onboarding mission",
/// and a second meaning on one axis makes both unreadable.
fn hint_rows(contract: &ContractRow) -> Vec<Row> {
    contract
        .hint
        .as_deref()
        .map(|hint| description_rows(&format!("\u{2192} {hint}")).collect())
        .unwrap_or_default()
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
    contract_head(
        contract,
        format!(
            "{} - {}{progress} - pays {}",
            contract.name, contract.objective_line, contract.reward_line
        ),
        idx == selected,
    )
}

/// A contract's selectable row reading `text`.
fn contract_head(contract: &ContractRow, text: String, selected: bool) -> Row {
    let mut row = item_row(text, selected);
    // Onboarding missions, and nothing else on these screens. `color` means
    // fusion tier on the gear screens and CRITICAL HP on the party screen;
    // the contracts screens have never used it, so green lands on a free
    // axis rather than becoming a second meaning on a loaded one.
    if contract.tutorial
        && let Row::Item { color, .. } = &mut row
    {
        *color = GREEN;
    }
    row
}

/// One contract on a Contracts tab: the name (and a held one's progress) as
/// the selectable row, then what it asks and pays on lines of their own. The
/// hub's content rect is narrower than the town board's popup, and the
/// shipped one-line form ran 137px past it.
fn page_entry(contract: &ContractRow, idx: usize, selected: usize, held: bool) -> Vec<Row> {
    let progress = if held {
        format!(" [{}/{}]", contract.progress, contract.target)
    } else {
        String::new()
    };
    let mut rows = vec![
        contract_head(
            contract,
            format!("{}{progress}", contract.name),
            idx == selected,
        ),
        Row::TextColored(format!("    {}", contract.objective_line), TEXT_DIM),
        Row::TextColored(format!("    pays {}", contract.reward_line), TEXT_DIM),
    ];
    if held {
        rows.push(Row::TextColored(
            format!("    {}", issuer_line(contract.issuer_name.as_deref())),
            TEXT_DIM,
        ));
    }
    rows.extend(description_rows(&contract.description));
    rows.extend(hint_rows(contract));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
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
            hint: None,
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
        let long = "Mining by hand is a way to spend a run. Stand up an Extractor \
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

    /// **The hint reaches the screen, and an absent one costs no row.**
    ///
    /// `held_rows` is measured rather than `hint_rows`, for the reason the
    /// builder is split out at all: a census that calls the helper itself
    /// stays green through a tab that stopped calling it, which is how an
    /// unwrapped paragraph shipped on this screen once already.
    #[test]
    fn a_hint_is_drawn_under_the_contract_it_belongs_to() {
        let with = ContractRow {
            hint: Some("Carry them to your Contract Broker.".to_string()),
            ..row("deliver", false)
        };
        let without = ContractRow {
            hint: None,
            ..row("hunt", false)
        };

        let drawn = |contract: &ContractRow| -> Vec<String> {
            held_rows(std::slice::from_ref(contract), usize::MAX)
                .into_iter()
                .filter_map(|r| match r {
                    Row::Item { text, .. } => Some(text),
                    _ => None,
                })
                .collect()
        };

        assert!(
            drawn(&with)
                .iter()
                .any(|line| line.contains("Carry them to your Contract Broker.")),
            "a hint the engine built has to reach the screen: {:?}",
            drawn(&with)
        );
        assert_eq!(
            drawn(&without).len(),
            drawn(&with).len() - 1,
            "and an objective that is its own instruction spends no row on one"
        );
    }

    /// A held row names who it is for, which the old popup never drew.
    #[test]
    fn a_held_row_names_its_issuer() {
        let town = ContractRow {
            issuer_name: Some("Ashford".to_string()),
            ..row("town", false)
        };
        let texts = |rows: Vec<Row>| -> Vec<String> { rows.iter().map(row_label_text).collect() };
        let broker = texts(held_rows(&[row("own", false)], 0));
        assert!(broker.iter().any(|t| t.trim() == "Broker"), "{broker:?}");
        let posted = texts(held_rows(&[town], 0));
        assert!(
            posted
                .iter()
                .any(|t| t.trim() == "for Ashford · hand in there"),
            "{posted:?}"
        );
    }

    /// The row ordinals are the handler's indices on each tab: `Row::Item`
    /// headlines are numbered from zero per tab, description lines aside.
    #[test]
    fn each_tab_highlights_its_own_list_from_zero() {
        let two = [row("a", false), row("b", false)];
        for rows in [
            held_rows(&two, 1),
            board_rows(&two, false, BrokerReach::AtBroker, 1),
        ] {
            let picked: Vec<_> = rows
                .iter()
                .filter_map(|r| match r {
                    Row::Item {
                        text,
                        selected: true,
                        ..
                    } => Some(text.clone()),
                    _ => None,
                })
                .collect();
            assert_eq!(picked.len(), 1, "{picked:?}");
            assert!(picked[0].starts_with("A Contract"), "{picked:?}");
        }
    }

    /// **No row a shipped contract builds runs past the hub's content rect**,
    /// on either tab: the headline (held carries a progress figure the offer
    /// does not), the issuer line, the wrapped description and the hint.
    /// Built from the real `assets/contracts/`, so a long contract added
    /// later fails here rather than being caught by eye.
    #[test]
    fn no_shipped_contract_row_overflows_the_page() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(41, DifficultyMode::Forgiving, assets).expect("shipped assets");
        let (towns, _) = feral_processes_engine::settlements::SettlementDb::load_dir(
            &assets.join("settlements"),
        )
        .expect("the catalogue loads");
        let widest_town = towns
            .iter()
            .map(|def| def.name.clone())
            .max_by_key(|name| name.chars().count())
            .expect("the shipped catalogue defines a settlement");
        let catalogue: Vec<ContractRow> = game
            .contract_catalogue()
            .into_iter()
            .map(|row| ContractRow {
                issuer_name: Some(widest_town.clone()),
                // A held row's widest progress figure is a full one.
                progress: row.target,
                ..row
            })
            .collect();
        assert!(!catalogue.is_empty(), "the census must walk real contracts");
        crate::render::base_report::assert_rows_fit(&held_rows(&catalogue, 0), "held contract");
        crate::render::base_report::assert_rows_fit(
            &board_rows(&catalogue, false, BrokerReach::AtBroker, 0),
            "board contract",
        );
        for (onboarding, reach) in [(true, BrokerReach::AtBroker), (false, BrokerReach::OffBase)] {
            crate::render::base_report::assert_rows_fit(
                &board_rows(&[], onboarding, reach, 0),
                "board header",
            );
        }
    }
}
