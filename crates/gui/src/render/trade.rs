//! The trading post: stock, prices, and the program-sale confirmation.

use super::popup::*;
use super::*;

pub(super) fn draw_trade_menu(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let structures: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.can_trade)
        .collect();
    let mut rows = vec![text_row("Trade with which structure?")];
    if structures.is_empty() {
        rows.push(text_row("(no trading posts nearby)"));
    }
    for (i, s) in structures.iter().enumerate() {
        let durability = s
            .durability
            .map(|(hp, max)| format!(" [HP {hp}/{max}]"))
            .unwrap_or_default();
        rows.push(item_row(
            format!(
                "[{}] {} at ({}, {}){}",
                menu_shortcut(i),
                s.label,
                s.pos.0,
                s.pos.1,
                durability
            ),
            i == selected,
        ));
    }
    draw_popup("Trade", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_trade_action_menu(
    game: &mut Game,
    structure: Option<Entity>,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(structure) = structure else { return };
    let Some(trade) = game.trade_options(structure) else {
        return;
    };
    let status = game.player_status();
    let inventory = status.inventory.clone();
    // The trade currency, not the build salvage: salvage is ordinary goods
    // to a trader, and money is the one thing it won't take back.
    let currency = game.trade_currency();
    let money = game.item_name(&currency).to_string();

    let mut rows = vec![Row::TextColored("Sell (from inventory):".to_string(), TEXT)];
    let sellable: Vec<_> = inventory
        .iter()
        .filter(|(item, _)| *item != currency)
        .collect();
    if sellable.is_empty() {
        rows.push(text_row("(nothing to sell)"));
    }
    let mut idx = 0;
    for (item, qty) in &sellable {
        // Same tag the inventory shows, so what you're about to part with
        // reads identically on both screens — fusion tier included, since
        // that's exactly what you'd want to check before selling.
        let tag = equip_preview_tag(game, item, status.zone, game.item_fusion_tier(item));
        rows.push(item_row(
            format!(
                "[{}] Sell {} x{qty}{} ({} {money} each)",
                menu_shortcut(idx),
                game.item_name(item),
                tag,
                trade.sell_rate
            ),
            idx == selected,
        ));
        idx += 1;
    }
    rows.push(text_row(""));
    rows.push(Row::TextColored("Buy:".to_string(), TEXT));
    for (item, cost) in &trade.buy {
        // Fusion tier 0: stock is unfused, so the tag shows what you'd get
        // buying it, not what some copy in your buffer happens to be.
        let tag = equip_preview_tag(game, item, status.zone, 0);
        rows.push(item_row(
            format!(
                "[{}] Buy {}{} ({cost} {money} each)",
                menu_shortcut(idx),
                game.item_name(item),
                tag
            ),
            idx == selected,
        ));
        idx += 1;
    }
    // Only shown by a trader that buys programs — see
    // `TradeDef::program_sell_divisor`. Omitted entirely otherwise, rather
    // than shown empty, so an items-only trader's screen is unchanged.
    let programs = game.program_sale_options(structure);
    if !programs.is_empty() {
        rows.push(text_row(""));
        rows.push(Row::TextColored(
            "Sell programs (permanent):".to_string(),
            TEXT,
        ));
        for program in &programs {
            rows.push(item_row(
                format!(
                    "[{}] Sell {} Lv{} — power {} → {} {money}{}",
                    menu_shortcut(idx),
                    program.name,
                    program.level,
                    program.power,
                    program.payout,
                    activity_tag(&program.activity),
                ),
                idx == selected,
            ));
            idx += 1;
        }
    }
    rows.push(text_row(""));
    rows.push(text_row("Esc to cancel; Up/Down + Enter also work"));
    draw_popup("Trade", PopupSize::Large, &rows, painter, m);
}

/// Confirms a program sale. The only screen that says what else the sale
/// takes down, since selling detaches the program from its party slot,
/// cronjob or guard post without asking.
pub(super) fn draw_trade_program_confirm(
    option: Option<&ProgramSaleOption>,
    money: &str,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(option) = option else { return };
    let mut rows = vec![
        text_row(format!(
            "Sell {} (Lv {}, power {}) for {} {money}?",
            option.name, option.level, option.power, option.payout
        )),
        Row::TextColored(
            "This erases the program for good. It cannot be undone.".to_string(),
            RED,
        ),
    ];
    if !option.detaches.is_empty() {
        rows.push(text_row(""));
        for detached in &option.detaches {
            rows.push(Row::TextColored(format!("It {detached}."), ORANGE));
        }
    }
    rows.push(text_row(""));
    rows.push(text_row("[y] sell    [n] keep it    Esc to cancel"));
    draw_popup("Confirm sale", PopupSize::Small, &rows, painter, m);
}

pub(super) fn draw_trade_quantity_menu(
    game: &mut Game,
    structure: Option<Entity>,
    choice: Option<TradeChoice>,
    quantity_input: &str,
    painter: &Painter,
    m: &Metrics,
) {
    let (Some(structure), Some(choice)) = (structure, choice) else {
        return;
    };
    let Some(trade) = game.trade_options(structure) else {
        return;
    };
    let (verb, item, unit_price) = match choice {
        TradeChoice::Sell(item) => ("Sell", item, trade.sell_rate),
        TradeChoice::Buy(item) => {
            let price = trade
                .buy
                .iter()
                .find(|(i, _)| *i == item)
                .map(|(_, c)| *c)
                .unwrap_or(0);
            ("Buy", item, price)
        }
    };
    let shown = if quantity_input.is_empty() {
        "1"
    } else {
        quantity_input
    };
    let money = game.item_name(&game.trade_currency()).to_string();
    let rows = vec![
        text_row(format!("{verb} how many {}?", game.item_name(&item))),
        text_row(""),
        text_row(format!("Price: {unit_price} {money} each")),
        text_row(""),
        text_row(format!("Quantity: {shown}")),
        text_row(""),
        text_row(format!(
            "Type digits, Enter to {}, Esc to go back",
            verb.to_lowercase()
        )),
    ];
    draw_popup("Trade", PopupSize::Large, &rows, painter, m);
}
