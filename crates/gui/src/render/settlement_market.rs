//! A settlement's shelf, and the basket for buying off it and selling into
//! it — `caravan.rs`'s shape, one vendor over.

use feral_processes_engine::settlements::SettlementKey;
use feral_processes_engine::{CaravanOfferKind, SettlementMarketView};

use super::popup::*;
use super::*;

/// The settlement's shelf, and what the player has put in the basket in
/// front of it — `caravan::CaravanBasket`'s shape, minus the fields a
/// settlement has no use for (`trader`/`description`/`ticks_left`: a
/// settlement's identity is `Mode::Settlement`'s page, and it never leaves).
pub(super) struct SettlementMarketBasket {
    pub(super) view: SettlementMarketView,
    /// `(amount, ceiling)` per row, index-aligned with the drawn list exactly
    /// as `App::settlement_amounts` is.
    pub(super) cells: Vec<(u32, u32)>,
    /// What the purse holds once this basket commits.
    pub(super) purse: u32,
}

pub(super) fn draw_settlement_market(
    game: &mut Game,
    key: Option<SettlementKey>,
    basket: Option<SettlementMarketBasket>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let (Some(key), Some(basket)) = (key, basket) else {
        return;
    };
    // The settlement's own name, off the same door the hub page reads it
    // through — `Game::settlement_report` — rather than a second field on
    // `SettlementMarketView` for a title this page alone would want.
    let title = game.settlement_report(key).name;
    let rows =
        settlement_market_page_rows(game, &basket.view, &basket.cells, basket.purse, selected);
    draw_popup(&title, PopupSize::Large, &rows, refusal, painter, m);
}

/// The page's rows, split from the draw for `caravan_page_rows`' reason:
/// **this page scrolls**, the same as the wagon's own list and for the same
/// cause — the sell section is the player's whole cargo, which nothing here
/// bounds, so the two censuses below measure the chrome around a scrolling
/// list rather than claim a fixed page fits every inventory whole.
pub(super) fn settlement_market_page_rows(
    game: &mut Game,
    view: &SettlementMarketView,
    cells: &[(u32, u32)],
    purse: u32,
    selected: usize,
) -> Vec<Row> {
    // A closed counter is a page, not an empty list: every figure below is
    // about a basket, and there is no basket to be had here.
    if view.closed {
        return vec![
            Row::TextColored("The counter is shut to you.".to_string(), TEXT),
            text_row(""),
            text_row("They will not trade until you have made it right."),
            text_row(""),
            text_row("Esc to go back"),
        ];
    }
    let money = &view.currency;
    // What a row is holding, and out of what — `caravan_page_rows`' own
    // `cell` closure, unchanged: an offer says *whether*, cargo says how
    // many out of what.
    let cell = |idx: usize, offer: bool| -> Option<String> {
        let (amount, ceiling) = cells.get(idx).copied().unwrap_or((0, 0));
        match (offer, amount) {
            (_, 0) => None,
            (true, _) => Some("in the basket".to_string()),
            (false, n) => Some(format!("{n} of {ceiling}")),
        }
    };

    let mut rows: Vec<Row> = vec![text_row(format!("You have: {} {money}", view.credits))];
    // The figure the screen exists to show — omitted while the basket is
    // empty, `caravan_page_rows`' reason: an untouched shelf reads exactly
    // as it did before a basket was opened.
    if purse != view.credits {
        rows.push(Row::TextColored(
            format!("This basket leaves you {purse} {money}"),
            TEXT,
        ));
    }
    rows.push(Row::TextColored("On the shelf:".to_string(), TEXT));

    let mut idx = 0;
    if view.offers.is_empty() {
        rows.push(text_row("(nothing on the shelf)"));
    }
    // One heading per run, emitted on the change rather than counted up
    // front — `Game::caravan_group`'s own rule, shared verbatim with the
    // wagon: `idx`, which `App::settlement_ceiling` resolves a pick
    // through, never sees a heading row.
    let mut run: Option<u8> = None;
    for offer in &view.offers {
        let (rank, heading) = game.caravan_group(&offer.kind);
        if run != Some(rank) {
            rows.push(Row::TextColored(heading.to_string(), TEXT));
            run = Some(rank);
        }
        let cost = offer.unit_cost * offer.qty;
        let label = format!("{}  ({cost} {money})", offer.name);
        let lead = row_lead(menu_shortcut(idx), (offer.qty > 1).then_some(offer.qty));
        let (tag, quality, power) = match &offer.kind {
            CaravanOfferKind::Gear(copy) => (
                game.item_category(&copy.item).short_label(),
                Some(copy.quality),
                PowerCell::of_copy(game, copy),
            ),
            CaravanOfferKind::Material(item) => (
                game.item_category(item).short_label(),
                None,
                PowerCell::of_item(game, item),
            ),
            CaravanOfferKind::Routine(_) | CaravanOfferKind::Program(_) => {
                ("", None, PowerCell::Blank)
            }
        };
        let row = if view.credits >= cost {
            with_tag(item_row(label, idx == selected), lead, tag, quality, power)
        } else {
            with_tag(
                spent_item_row(label, idx == selected),
                lead,
                tag,
                quality,
                power,
            )
        };
        rows.push(match cell(idx, true) {
            Some(text) => with_suffix(row, text),
            None => row,
        });
        const DETAIL_INDENT: &str = "      ";
        for line in wrap_text(&offer.detail, DESCRIBE_WRAP_COLUMNS - DETAIL_INDENT.len()) {
            rows.push(Row::TextColored(format!("{DETAIL_INDENT}{line}"), TEXT_DIM));
        }
        idx += 1;
    }

    rows.push(text_row(""));
    rows.push(Row::TextColored("They'll buy:".to_string(), TEXT));
    if view.sells.is_empty() {
        rows.push(text_row("(nothing they want)"));
    }
    let mut run = None;
    for row in &view.sells {
        let (rank, heading) =
            game.caravan_group(&CaravanOfferKind::Material(row.copy.item.clone()));
        if run != Some(rank) {
            rows.push(Row::TextColored(heading.to_string(), TEXT));
            run = Some(rank);
        }
        let sell = with_tag(
            tier_row(
                format!("Sell {} ({} {money} each)", row.name, row.unit_price),
                idx == selected,
                row.copy.tier,
                row.copy.rarity,
            ),
            row_lead(menu_shortcut(idx), Some(row.held)),
            game.item_category(&row.copy.item).short_label(),
            Some(row.copy.quality),
            PowerCell::of_copy(game, &row.copy),
        );
        rows.push(match cell(idx, false) {
            Some(text) => with_suffix(sell, text),
            None => sell,
        });
        for line in effect_lines(game, &row.copy.item) {
            rows.push(tier_row(line, false, row.copy.tier, row.copy.rarity));
        }
        idx += 1;
    }
    rows.push(text_row(""));
    rows.push(text_row(
        "Left/Right set a row  ·  Shift jumps to the end  ·  Ctrl halves the gap",
    ));
    rows.push(text_row(
        "[A] all your cargo  ·  [N] clear  ·  Enter to trade  ·  Esc to step away",
    ));
    rows.push(text_row(
        "[I] inspect — full stats, and what a granted routine actually does",
    ));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;
    use feral_processes_engine::items::{GearCopy, ItemId, ids};
    use feral_processes_engine::{CaravanOffer, CaravanOfferKind, CaravanSellRow, DifficultyMode};

    fn assets() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    fn census_game() -> Game {
        Game::new(3, DifficultyMode::Forgiving, &assets()).expect("a game for the census")
    }

    /// A census and not a fixture, `tallest_caravan_page`'s reason one
    /// vendor over: the worst case is a property of the assets and how this
    /// page packs them, not a hand-picked example. The shelf is as deep as
    /// the deepest shipped `SETTLEMENT_MAINFRAME_ROWS`, and the sell section
    /// is bounded the same way the caravan's own census bounds it — nothing
    /// in `SettlementMarketView` caps it, so a plausible pack rather than
    /// the unbounded true worst case is what this measures.
    fn tallest_settlement_market_page(game: &mut Game) -> Vec<Row> {
        let items = game.item_defs();
        let widest_item = items
            .iter()
            .max_by_key(|d| d.name.chars().count())
            .expect("the item set is not empty");
        let widest_detail = items
            .iter()
            .max_by_key(|d| d.description.chars().count())
            .expect("the item set is not empty");

        let rows = feral_processes_engine::tuning::SETTLEMENT_MAINFRAME_ROWS as usize;
        let offers: Vec<CaravanOffer> = (0..rows)
            .map(|index| CaravanOffer {
                index,
                kind: CaravanOfferKind::Material(widest_item.id.clone()),
                name: widest_item.name.clone(),
                detail: widest_detail.description.clone(),
                unit_cost: 9_999,
                qty: feral_processes_engine::tuning::CARAVAN_MATERIAL_STACK,
            })
            .collect();
        let sells = vec![CaravanSellRow {
            copy: GearCopy::plain(ItemId::from(ids::CORE_FRAGMENT)),
            name: widest_item.name.clone(),
            held: 9_999,
            unit_price: 9_999,
        }];

        let view = SettlementMarketView {
            offers,
            sells,
            credits: 9_999_999,
            currency: "Credits".to_string(),
            closed: false,
        };
        let cells: Vec<(u32, u32)> = (0..view.offers.len() + view.sells.len())
            .map(|i| {
                if i < view.offers.len() {
                    (1, 1)
                } else {
                    (9_999, 9_999)
                }
            })
            .collect();
        settlement_market_page_rows(game, &view, &cells, 0, 0)
    }

    /// Every figure the basket puts on the page comes from app-core, and the
    /// page says nothing at all while the basket is empty —
    /// `the_wagon_draws_its_basket_and_only_when_there_is_one`'s own
    /// property, one vendor over.
    #[test]
    fn the_shelf_draws_its_basket_and_only_when_there_is_one() {
        let mut game = census_game();
        let material = ItemId::from(ids::BYTECODE_BLOCK);
        let view = SettlementMarketView {
            offers: vec![CaravanOffer {
                index: 0,
                kind: CaravanOfferKind::Material(material.clone()),
                name: "x".to_string(),
                detail: String::new(),
                unit_cost: 7,
                qty: 1,
            }],
            sells: vec![CaravanSellRow {
                copy: GearCopy::plain(material),
                name: "y".to_string(),
                held: 40,
                unit_price: 2,
            }],
            credits: 100,
            currency: "Credits".to_string(),
            closed: false,
        };
        let text = |rows: Vec<Row>| -> Vec<String> {
            rows.into_iter()
                .map(|row| match row {
                    Row::Text(t) | Row::TextColored(t, _) => t,
                    Row::Item { text, suffix, .. } => {
                        format!("{text}|{}", suffix.unwrap_or_default())
                    }
                })
                .collect()
        };

        let empty = text(settlement_market_page_rows(
            &mut game,
            &view,
            &[(0, 1), (0, 40)],
            100,
            0,
        ));
        assert!(
            !empty.iter().any(|l| l.starts_with("This basket")),
            "an untouched shelf must not claim a basket: {empty:?}"
        );
        assert!(
            empty.iter().all(|l| !l.contains('|') || l.ends_with('|')),
            "an untouched row must carry no amount: {empty:?}"
        );

        let filled = text(settlement_market_page_rows(
            &mut game,
            &view,
            &[(1, 1), (3, 40)],
            93,
            0,
        ));
        assert!(
            filled.contains(&"This basket leaves you 93 Credits".to_string()),
            "the total is the figure the screen now exists to show: {filled:?}"
        );
        assert!(
            filled.iter().any(|l| l.ends_with("|in the basket")),
            "an offer is all-or-nothing, so it says whether and not how many: {filled:?}"
        );
        assert!(
            filled.iter().any(|l| l.ends_with("|3 of 40")),
            "a cargo row says how many, out of what: {filled:?}"
        );
    }

    /// **This page does scroll**, `caravan_page_rows`' own note: both
    /// sections are item rows and nothing here bounds the sell side, so the
    /// chrome around the list — not a claim that the whole shelf and the
    /// whole pack always fit at once — is what a fit census can hold.
    #[test]
    fn the_settlement_markets_chrome_leaves_room_for_a_list() {
        const USABLE_ROWS: usize = 6;

        let rows = tallest_settlement_market_page(&mut census_game());
        let first = rows
            .iter()
            .position(|r| matches!(r, Row::Item { .. }))
            .expect("the page is a list");
        let last = rows
            .iter()
            .rposition(|r| matches!(r, Row::Item { .. }))
            .expect("the page is a list");
        let chrome = first + (rows.len() - 1 - last);

        for h in (600..=2160).step_by(60) {
            let m = ui_metrics(h as f32);
            let cap = popup_max_rows(h as f32, PopupSize::Large, &m);
            let left = cap.saturating_sub(chrome + REFUSAL_MAX_LINES);
            assert!(
                left >= USABLE_ROWS,
                "the page's {chrome} rows of chrome leave {left} of a \
                 {cap}-row popup for the shelf itself at {h}px"
            );
        }
    }

    /// The other axis, and the one nothing clamps at all: `draw_row` clips a
    /// row vertically and never horizontally, so a line past the right edge
    /// is simply lost. Measured with real text metrics — the UI font is
    /// DejaVu Sans Mono, not the map's unscii.
    #[test]
    fn no_settlement_market_row_overflows_its_popup() {
        let rows = tallest_settlement_market_page(&mut census_game());
        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for row in &rows {
                let line = match row {
                    Row::Text(t) | Row::TextColored(t, _) => t.clone(),
                    Row::Item {
                        text, tag, suffix, ..
                    } => {
                        let joined = item_text(text, tag.as_ref());
                        match suffix {
                            Some(s) => format!("{joined} {s}"),
                            None => joined,
                        }
                    }
                };
                let drawn = p.measure_ui_advance(&line, m.font_size);
                assert!(
                    drawn <= room,
                    "a settlement market row overflows the page by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
        });
    }

    /// `every_screen_draws_a_refusal_exactly_once` in `mod.rs` can only say
    /// this screen draws *no* refusal, since the census app never stands the
    /// player next to a settlement — `a_caravan_page_says_a_refusal_exactly_once`'s
    /// reason, one vendor over. This is the other half: with a page to draw,
    /// it says it once.
    #[test]
    fn a_settlement_market_page_says_a_refusal_exactly_once() {
        let mut game = census_game();
        let rows = tallest_settlement_market_page(&mut game);
        let (_, shapes) = with_painter(|p| {
            let m = ui_metrics(900.0);
            draw_popup(
                "A settlement",
                PopupSize::Large,
                &rows,
                Some("Requires Zone 3 first."),
                p,
                &m,
            );
        });
        let said = crate::paint::painted_text(&shapes)
            .iter()
            .filter(|t| t.contains("Requires Zone 3 first."))
            .count();
        assert_eq!(
            said, 1,
            "the page painted the refusal {said} times, not once"
        );
    }
    /// The closed counter is a page with something on it, not an empty
    /// list: every figure the ordinary page shows is about a basket, and a
    /// shut town has none to offer.
    #[test]
    fn a_shut_counter_says_so_and_offers_nothing() {
        let mut game = census_game();
        let view = SettlementMarketView {
            offers: Vec::new(),
            sells: Vec::new(),
            credits: 500,
            currency: "Credits".to_string(),
            closed: true,
        };
        let rows = settlement_market_page_rows(&mut game, &view, &[], 500, 0);
        let joined = rows
            .iter()
            .map(|row| match row {
                Row::Text(t) | Row::TextColored(t, _) => t.clone(),
                Row::Item { text, .. } => text.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("shut"), "{joined}");
        assert!(
            !rows.iter().any(|row| matches!(row, Row::Item { .. })),
            "a shut counter must offer no selectable row"
        );
    }
}
