//! The counter a visiting caravan sets out, and the quantity page for
//! selling into it.

use feral_processes_engine::{CaravanOfferKind, CaravanView};

use super::popup::*;
use super::*;

/// Everything on the wagon, in the order `app::caravan::caravan_row`
/// resolves a picked row against — offers, then what the trader will take.
///
/// Both lists come from one `Game::caravan_view` call, which is what stops
/// the drawn rows and the handler's rows disagreeing about which row number
/// is which. A renderer that rebuilt either list itself would be right until
/// the first purchase.
/// The wagon, and what the player has put in the basket in front of it.
///
/// Assembled in `render::draw` rather than here, because every figure on it
/// comes out of `App` — `App::caravan_ceiling` takes `&self`, so it cannot
/// run once the draw holds `&mut app.game`. A struct rather than the tuple it
/// started as: three unlabelled numbers at a call site is how a `purse` ends
/// up drawn where a ceiling belongs.
pub(super) struct CaravanBasket {
    pub(super) view: CaravanView,
    /// `(amount, ceiling)` per row, index-aligned with the drawn list exactly
    /// as `App::caravan_amounts` is.
    pub(super) cells: Vec<(u32, u32)>,
    /// What the purse holds once this basket commits.
    pub(super) purse: u32,
}

pub(super) fn draw_caravan(
    game: &mut Game,
    basket: Option<CaravanBasket>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(basket) = basket else {
        return;
    };
    let title = basket.view.trader.clone();
    let rows = caravan_page_rows(game, &basket.view, &basket.cells, basket.purse, selected);
    draw_popup(&title, PopupSize::Large, &rows, refusal, painter, m);
}

/// The page's rows, split from the draw for `memory_page_rows`' reason:
/// **this page has no scroll**, so the two censuses below have to be able to
/// build the tallest and widest page the game can produce without a trader
/// standing in front of them.
pub(super) fn caravan_page_rows(
    game: &mut Game,
    view: &CaravanView,
    cells: &[(u32, u32)],
    purse: u32,
    selected: usize,
) -> Vec<Row> {
    let money = &view.currency;
    // What a row is holding, and out of what — the figures the arrows are
    // clamped to, taken from app-core and never recomputed here. An offer is
    // all-or-nothing, so it says *whether* rather than how many.
    // A row holding nothing says nothing: an untouched wagon has to read
    // exactly as it did before baskets existed, and the stack a cargo row
    // holds is already in its quantity column.
    let cell = |idx: usize, offer: bool| -> Option<String> {
        let (amount, ceiling) = cells.get(idx).copied().unwrap_or((0, 0));
        match (offer, amount) {
            (_, 0) => None,
            (true, _) => Some("in the basket".to_string()),
            (false, n) => Some(format!("{n} of {ceiling}")),
        }
    };
    // Wrapped, not trimmed: a trader's own line is prose a modder writes, and
    // nothing clips a row horizontally — an over-wide line is drawn off the
    // panel in silence. `no_caravan_row_overflows_its_popup` caught the
    // shipped descriptions running 350px past the body.
    let mut rows: Vec<Row> = wrap_text(&view.description, DESCRIBE_WRAP_COLUMNS)
        .into_iter()
        .map(|line| Row::TextColored(line, TEXT_DIM))
        .collect();
    rows.push(text_row(format!(
        "You have: {} {money}  ·  rolling on in {} turns",
        view.credits, view.ticks_left
    )));
    // **The figure the screen now exists to show.** Without it a player sets
    // six rows blind and finds out what the visit cost only after Enter.
    // Omitted while the basket is empty, so an untouched wagon reads exactly
    // as it did.
    if purse != view.credits {
        rows.push(Row::TextColored(
            format!("This basket leaves you {purse} {money}"),
            TEXT,
        ));
    }
    rows.push(Row::TextColored("On the wagon:".to_string(), TEXT));

    let mut idx = 0;
    if view.offers.is_empty() {
        rows.push(text_row("(bought out)"));
    }
    // The heading of the run being drawn. Emitted on the change rather than
    // counted up front, so a run of one still gets its heading and the two
    // lists need no second walk. `Game::caravan_group` is the one rule for
    // both where a run starts and what it is called — and these are
    // `Row::TextColored`, so `idx` (which is what `App::caravan_row`
    // resolves a pick through) never sees them.
    let mut run: Option<u8> = None;
    for offer in &view.offers {
        let (rank, heading) = game.caravan_group(&offer.kind);
        if run != Some(rank) {
            rows.push(Row::TextColored(heading.to_string(), TEXT));
            run = Some(rank);
        }
        // Rows the player cannot afford stay listed and stay dim: a wagon
        // you have to come back to is a reason to go and earn, where a
        // hidden row is a wagon that looks emptier than it is.
        let cost = offer.unit_cost * offer.qty;
        let label = format!("{}  ({cost} {money})", offer.name);
        let lead = row_lead(menu_shortcut(idx), (offer.qty > 1).then_some(offer.qty));
        // The same column the sell list below already carries, which the
        // offer list did not: what *category* a row is, and for a copy of
        // gear what quality it rolled. Both are the questions the wagon is
        // read for, and without them a Prismatic affixed weapon drew
        // identically to a plain one.
        //
        // Exhaustive on the kind, `cell_mark`'s rule — and the two kinds
        // that are not items pass a blank rather than a token invented in
        // the renderer. `ItemCategory::short_label` stays the one place the
        // three letters are spelled.
        // The rating column keys off the same match, and the two kinds that
        // are not items draw a *blank* rather than an em dash: a dash would
        // claim the disk had been rated and found wanting.
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
        // Colour on this list means "can you afford it" and nothing else.
        // Rarity deliberately does *not* reach it: `fusion_color`'s rule is
        // that a second meaning on one colour axis makes both unreadable,
        // and the affordability dimming is the one the player is scanning
        // for. The tier still shows in the name and in the tag column.
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
        // The same wrap for the same reason — an item's authored description
        // is prose too, and the indent has to come out of the budget or the
        // wrap measures a line narrower than the one it draws.
        const DETAIL_INDENT: &str = "      ";
        for line in wrap_text(&offer.detail, DESCRIBE_WRAP_COLUMNS - DETAIL_INDENT.len()) {
            rows.push(Row::TextColored(format!("{DETAIL_INDENT}{line}"), TEXT_DIM));
        }
        idx += 1;
    }

    rows.push(text_row(""));
    rows.push(Row::TextColored(
        "They'll take (no buyback — they won't be back for it):".to_string(),
        TEXT,
    ));
    if view.sells.is_empty() {
        rows.push(text_row("(nothing they want)"));
    }
    let mut run = None;
    for row in &view.sells {
        // The same headings, from the same call — the goods a wagon will
        // take are cargo, so every one of them has an `ItemCategory` and the
        // `Material` arm is the whole of what this needs from the kind.
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

    /// A census and not a fixture — the worst case is a property of the
    /// assets *and* of how this page packs them, so a trader named longer, a
    /// def blurbed longer, or a second line added per row has to fail here
    /// rather than be caught by eye.
    ///
    /// Every offer takes the longest name and the longest description the
    /// item set can supply, and the shelf is as deep as the deepest shipped
    /// `CaravanDef::rows`. The sell section is the player's whole inventory,
    /// which on this page is bounded by nothing — so it is capped at what a
    /// pack can plausibly hold and the *offers* half is what this measures.
    fn tallest_caravan_page(game: &mut Game) -> Vec<Row> {
        let (caravans, warnings) =
            feral_processes_engine::caravans::CaravanDb::load_dir(&assets().join("caravans"))
                .expect("the catalogue loads");
        assert!(warnings.is_empty(), "{warnings:?}");
        let deepest = caravans
            .all()
            .map(|d| d.rows)
            .max()
            .expect("the census must walk a real catalogue");
        let widest_trader = caravans
            .all()
            .max_by_key(|d| d.name.chars().count() + d.description.chars().count())
            .expect("the census must walk a real catalogue");

        // The dearest, longest-named, longest-described item the set has —
        // measured together, since the row draws the name and the line under
        // it draws the description.
        let items = game.item_defs();
        let widest_item = items
            .iter()
            .max_by_key(|d| d.name.chars().count())
            .expect("the item set is not empty");
        let widest_detail = items
            .iter()
            .max_by_key(|d| d.description.chars().count())
            .expect("the item set is not empty");

        let offers: Vec<CaravanOffer> = (0..deepest as usize)
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

        let view = CaravanView {
            trader: widest_trader.name.clone(),
            description: widest_trader.description.clone(),
            offers,
            sells,
            credits: 9_999_999,
            currency: "Credits".to_string(),
            ticks_left: 9_999,
        };
        // Measured with a **filled** basket: every row's amount suffix and
        // the total header are real width and a real row, and a census that
        // built an empty basket would measure a page nobody sees once they
        // start shopping.
        let cells: Vec<(u32, u32)> = (0..view.offers.len() + view.sells.len())
            .map(|i| {
                if i < view.offers.len() {
                    (1, 1)
                } else {
                    (9_999, 9_999)
                }
            })
            .collect();
        caravan_page_rows(game, &view, &cells, 0, 0)
    }

    fn census_game() -> Game {
        Game::new(3, DifficultyMode::Forgiving, &assets()).expect("a game for the census")
    }

    /// Every figure the basket puts on the page comes from app-core, and the
    /// page says nothing at all while the basket is empty — an untouched
    /// wagon has to read exactly as it did before baskets existed.
    #[test]
    fn the_wagon_draws_its_basket_and_only_when_there_is_one() {
        let mut game = census_game();
        let material = ItemId::from(ids::BYTECODE_BLOCK);
        let view = CaravanView {
            trader: "T".to_string(),
            description: String::new(),
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
            ticks_left: 10,
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

        let empty = text(caravan_page_rows(
            &mut game,
            &view,
            &[(0, 1), (0, 40)],
            100,
            0,
        ));
        assert!(
            !empty.iter().any(|l| l.starts_with("This basket")),
            "an untouched wagon must not claim a basket: {empty:?}"
        );
        assert!(
            empty.iter().all(|l| !l.contains('|') || l.ends_with('|')),
            "an untouched row must carry no amount: {empty:?}"
        );

        // One of each: the offer taken whole, three of the cargo stack.
        let filled = text(caravan_page_rows(
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

    /// A heading per run, and — the thing that can silently break — the
    /// shortcut on the *n*th pickable row still reads `n`.
    ///
    /// Headings are `Row::TextColored` and must never touch the counter
    /// `App::caravan_row` resolves a pick through. Off by one, `[c]` buys
    /// what `[b]` names and nothing anywhere fails.
    #[test]
    fn the_wagon_heads_each_run_without_moving_a_shortcut() {
        let mut game = census_game();
        let weapon = ItemId::from(ids::MONOFILAMENT_WHIP);
        let material = ItemId::from(ids::BYTECODE_BLOCK);
        let offer = |index: usize, kind: CaravanOfferKind| CaravanOffer {
            index,
            kind,
            name: "x".to_string(),
            detail: String::new(),
            unit_cost: 1,
            qty: 1,
        };
        let view = CaravanView {
            trader: "T".to_string(),
            description: String::new(),
            // Two runs on the offer side, in the order `caravan_group` ranks
            // them, plus one row the player can take.
            offers: vec![
                offer(0, CaravanOfferKind::Gear(GearCopy::plain(weapon.clone()))),
                offer(1, CaravanOfferKind::Gear(GearCopy::plain(weapon))),
                offer(2, CaravanOfferKind::Material(material.clone())),
            ],
            sells: vec![CaravanSellRow {
                copy: GearCopy::plain(material),
                name: "y".to_string(),
                held: 1,
                unit_price: 1,
            }],
            credits: 10,
            currency: "Credits".to_string(),
            ticks_left: 10,
        };

        let cells = vec![(0, 0); view.offers.len() + view.sells.len()];
        let rows = caravan_page_rows(&mut game, &view, &cells, view.credits, 0);
        let headings: Vec<&str> = rows
            .iter()
            .filter_map(|r| match r {
                Row::TextColored(t, _) if t == "Weapons" || t == "Materials" => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            headings,
            vec!["Weapons", "Materials", "Materials"],
            "one heading per run, on both lists, and never one per row"
        );

        let shortcuts: Vec<char> = rows
            .iter()
            .filter_map(|r| match r {
                Row::Item { tag: Some(t), .. } => t.lead.chars().nth(1),
                _ => None,
            })
            .collect();
        assert_eq!(
            shortcuts,
            (0..4).map(menu_shortcut).collect::<Vec<_>>(),
            "the headings moved a shortcut off the row it belongs to"
        );
    }

    /// **This page does scroll**, unlike the gear inspect and memories
    /// pages — `popup_layout` takes the span from the first `Row::Item` to
    /// the last as the body and pages it, and both of this page's sections
    /// are item rows. So the tallest-page census those two carry would be
    /// the wrong test here; what can still go wrong is the chrome around
    /// the list.
    ///
    /// The header (the trader's wrapped line, the money, the section head)
    /// and the footer (the two key hints) are drawn whole at every window
    /// size, and `raw_capacity` is what is left after them. Let those grow
    /// and the list they frame shrinks to `.max(1)` — a wagon shown one row
    /// at a time, with nothing failing anywhere.
    ///
    /// Swept rather than measured at one window, for the gear page's reason:
    /// `ui_metrics` clamps the font at both ends, so below the clamp the box
    /// keeps shrinking while the line height stops and the tightest window is
    /// the smallest one.
    #[test]
    fn the_caravan_pages_chrome_leaves_room_for_a_list() {
        /// Enough rows that the list still reads as one at the smallest
        /// window a player can have — not a number the layout derives, a
        /// judgement about what is worth drawing.
        const USABLE_ROWS: usize = 6;

        let rows = tallest_caravan_page(&mut census_game());
        let first = rows
            .iter()
            .position(|r| matches!(r, Row::Item { .. }))
            .expect("the page is a list");
        let last = rows
            .iter()
            .rposition(|r| matches!(r, Row::Item { .. }))
            .expect("the page is a list");
        // The ends only, and deliberately: unlike the gear page this one
        // *scrolls* — `draw_popup` pages its `Row::Item` span — so a
        // category heading between the first and the last row travels with
        // the list rather than eating into it, exactly as the per-offer
        // detail lines already did. What the scroll can never page past is
        // what sits outside the span, and that is what this counts.
        let chrome = first + (rows.len() - 1 - last);

        for h in (600..=2160).step_by(60) {
            let m = ui_metrics(h as f32);
            let cap = popup_max_rows(h as f32, PopupSize::Large, &m);
            let left = cap.saturating_sub(chrome + REFUSAL_MAX_LINES);
            assert!(
                left >= USABLE_ROWS,
                "the page's {chrome} rows of chrome leave {left} of a \
                 {cap}-row popup for the wagon itself at {h}px"
            );
        }
    }

    /// The offer list carries the same `WEP`/`ARM`/`MOD` column the sell
    /// list below it already did. It did not, and the gap was invisible on
    /// this page precisely because the two lists sit one above the other:
    /// the wagon's own stock was the untagged half.
    ///
    /// Asserted on the `Row::Item`'s `tag`, not on its text — the token is a
    /// column laid out as its own `ui_runs` piece, and a test matching a
    /// substring would pass against a renderer that formatted it into the
    /// middle of the row and left no span to paint.
    #[test]
    fn an_offer_row_carries_its_category_and_quality() {
        let mut game = census_game();
        // Picked out of the item set rather than named, so the test measures
        // the column and not one shipped weapon's continued existence.
        let weapon = game
            .item_defs()
            .iter()
            .find(|d| {
                matches!(
                    d.equipment.map(|(slot, _)| slot),
                    Some(feral_processes_engine::items::EquipmentSlot::Weapon)
                )
            })
            .expect("the item set ships a weapon")
            .id
            .clone();
        let copy = GearCopy::with_affixes(
            weapon,
            feral_processes_engine::components::Rarity::Ordinary,
            0,
            Vec::new(),
            117,
        );
        let view = CaravanView {
            trader: "T".to_string(),
            description: "d".to_string(),
            offers: vec![CaravanOffer {
                index: 0,
                kind: CaravanOfferKind::Gear(copy),
                name: "Arc Lance".to_string(),
                detail: String::new(),
                unit_cost: 1,
                qty: 1,
            }],
            sells: Vec::new(),
            credits: 9_999,
            currency: "Credits".to_string(),
            ticks_left: 9,
        };

        let cells = vec![(0, 0); view.offers.len() + view.sells.len()];
        let tag = caravan_page_rows(&mut game, &view, &cells, view.credits, 0)
            .into_iter()
            .find_map(|r| match r {
                Row::Item { tag, .. } => tag,
                _ => None,
            })
            .expect("the offer drew no item row");

        assert_eq!(
            tag.text, "WEP",
            "a weapon on the wagon drew as {:?}",
            tag.text
        );
        let (color, bold) = quality_tag_style(Some(117));
        assert_eq!(
            (tag.color, tag.bold),
            (color, bold),
            "the tag ignored the copy's quality"
        );
    }

    /// The other axis, and the one nothing clamps at all: `draw_row` clips a
    /// row vertically and never horizontally, so a line past the right edge
    /// is simply lost. On this page the tail of a row is the price — the
    /// figure the row is read for.
    ///
    /// Measured with real text metrics through `with_painter`. The UI font is
    /// DejaVu Sans Mono, not the map's unscii, so a character count would be
    /// measuring the wrong face.
    #[test]
    fn no_caravan_row_overflows_its_popup() {
        let rows = tallest_caravan_page(&mut census_game());
        with_painter(|p| {
            let m = ui_metrics(900.0);
            // 0.88 is `PopupSize::Large`'s width fraction, against the
            // 1440x900 geometry `ui_metrics` is calibrated for.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for row in &rows {
                let line = match row {
                    Row::Text(t) | Row::TextColored(t, _) => t.clone(),
                    // An `Item` row draws as three pieces and `suffix_x` lays
                    // its suffix on the joined form, so it has to be measured
                    // joined — measuring the head alone budgets for a row
                    // narrower than the one that is drawn.
                    Row::Item {
                        text, tag, suffix, ..
                    } => {
                        let joined = item_text(text, tag.as_ref());
                        match suffix {
                            Some(s) => format!("{joined} {s}"),
                            None => joined,
                        }
                    }
                    _ => continue,
                };
                let drawn = p.measure_ui_advance(&line, m.font_size);
                assert!(
                    drawn <= room,
                    "a caravan row overflows the page by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
        });
    }

    /// The refusal census in `mod.rs` can only say a caravan screen draws
    /// *no* refusal, because a fresh run has no trader standing at the
    /// counter. This is the other half: with a page to draw, it says it once.
    #[test]
    fn a_caravan_page_says_a_refusal_exactly_once() {
        let mut game = census_game();
        let rows = tallest_caravan_page(&mut game);
        let (_, shapes) = with_painter(|p| {
            let m = ui_metrics(900.0);
            draw_popup(
                "A wagon",
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
}
