//! The Sites hub's pages. Every sentence and figure is an engine view
//! (`Game::town_status`, `town_goods`, `site_routes`, `site_log`, ...); this
//! only lays them out. No age or wait shows a tick: the views carry the
//! banded words, and `OutpostReport::route` (which carries ticks) is never
//! read here.
//!
//! The selectable rows are `Row::Item`s and exactly as many as
//! `App::sites_row_count`, in the same order; everything else is a text row.

use super::staff::elide;
use super::*;
use crate::render::outposts::{GROWTH_BAR_WIDTH, text_growth_bar};
use feral_processes_engine::sites::TownGoods;
use feral_processes_engine::tuning::OUTPOST_CREW_CAP;
use feral_processes_engine::{
    CaravanSchedule, ContractRow, OutpostReport, RouteDestinationId, RouteLine, SiteLogLine,
    TownStatus,
};

/// The widest a site, item or program name grows in a row.
const NAME_CHARS: usize = 28;
/// A prose line wraps here: the hub's budget less a row's highlight and
/// continuation indent.
const PROSE_CHARS: usize = HUB_WRAP_COLUMNS - 6;
const INDENT: &str = "    ";

pub(in crate::render) fn site_rows(
    game: &mut Game,
    mode: Mode,
    site: Option<RouteDestinationId>,
    selected: usize,
) -> Vec<Row> {
    match (mode, site) {
        (Mode::SiteStatus, Some(RouteDestinationId::Settlement(key))) => game
            .town_status(key)
            .map_or_else(Vec::new, |s| town_status_rows(&s)),
        (Mode::SiteGoods, Some(RouteDestinationId::Settlement(key))) => {
            let currency = game.trade_currency();
            let money = game.item_name(&currency).to_string();
            goods_rows(&game.town_goods(key), &money, selected)
        }
        (Mode::SiteRoutes, Some(site)) => routes_rows(&game.site_routes(site), selected),
        (Mode::SiteJobs, Some(RouteDestinationId::Settlement(key))) => {
            jobs_rows(&game.town_contracts(key), selected)
        }
        (Mode::SiteLog, Some(site)) => log_rows(&game.site_log(site), selected),
        (Mode::OutpostStatus, Some(RouteDestinationId::Outpost(tile))) => game
            .outpost_report(tile)
            .map_or_else(Vec::new, |r| outpost_status_rows(&r)),
        (Mode::OutpostCrew, Some(RouteDestinationId::Outpost(tile))) => game
            .outpost_report(tile)
            .map_or_else(Vec::new, |r| crew_rows(&r, selected)),
        (Mode::OutpostStock, Some(RouteDestinationId::Outpost(tile))) => {
            let report = game.outpost_report(tile);
            let stock: Vec<(String, u32)> = game
                .outpost_transfer_offer(tile)
                .iter()
                .map(|row| (game.item_name(&row.item).to_string(), row.on_shelves))
                .collect();
            let routes = game.site_routes(RouteDestinationId::Outpost(tile));
            report.map_or_else(Vec::new, |r| stock_rows(&r, &stock, &routes, selected))
        }
        (Mode::CaravanSchedule, _) => caravan_rows(&game.caravan_schedule()),
        (Mode::RoutesInFlight, _) => flight_rows(&game.routes_in_flight(), selected),
        _ => Vec::new(),
    }
}

fn heading(text: &str) -> Row {
    Row::TextColored(text.to_string(), TEXT)
}

/// `text` wrapped to the page, each line after the first indented.
fn prose(text: &str, lead: &str) -> Vec<Row> {
    wrap_text(text, PROSE_CHARS - lead.len())
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            let pad = if i == 0 { lead } else { INDENT };
            text_row(format!("{pad}{line}"))
        })
        .collect()
}

/// An item row followed by its overflow lines as plain text.
fn item_with_overflow(first: String, rest: Vec<String>, selected: bool) -> Vec<Row> {
    let mut rows = vec![item_row(first, selected)];
    rows.extend(rest.into_iter().map(|l| text_row(format!("{INDENT}{l}"))));
    rows
}

pub(in crate::render) fn town_status_rows(status: &TownStatus) -> Vec<Row> {
    let mut rows = vec![Row::TextColored(elide(&status.name, PROSE_CHARS), CYAN)];
    rows.push(text_row(format!(
        "{}  ·  {}  ·  {}",
        status.kind, status.specialty, status.temperament
    )));
    let standing = format!("Standing: {}", status.standing);
    rows.push(if status.hostile {
        Row::TextColored(standing, hud::palette::ATTENTION)
    } else {
        text_row(standing)
    });
    for line in &status.meaning {
        rows.extend(prose(line, "  "));
    }
    if let Some(condition) = &status.condition {
        rows.push(text_row(""));
        rows.extend(prose(condition, ""));
    }
    rows.push(text_row(""));
    rows.extend(prose(&status.gift, "Gift: "));
    rows.extend(prose(&status.travel, "Travel: "));
    rows
}

pub(in crate::render) fn goods_rows(goods: &TownGoods, money: &str, selected: usize) -> Vec<Row> {
    let (header, offers) = match goods {
        TownGoods::NotSeen => {
            return vec![text_row("Not seen yet. Visit to see their shelf.")];
        }
        TownGoods::Seen { offers, age } => (
            format!("Seen {age}. Prices as seen then."),
            offers.as_slice(),
        ),
        TownGoods::Live { offers } => (
            "Live shelf from Price Feeds. Buying needs a visit.".to_string(),
            offers.as_slice(),
        ),
    };
    let mut rows = vec![text_row(header)];
    if offers.is_empty() {
        rows.push(text_row("The shelf was empty."));
    }
    for (i, offer) in offers.iter().enumerate() {
        rows.push(item_row(
            format!(
                "{}  x{}  {} {money}",
                elide(&offer.name, NAME_CHARS),
                offer.qty,
                offer.unit_cost
            ),
            i == selected,
        ));
    }
    rows
}

/// A route as its head line and the cargo wrapped beneath it.
fn route_item(line: &RouteLine, with_site: bool, selected: bool) -> Vec<Row> {
    let site = if with_site {
        format!("{}  ", elide(&line.site_name, NAME_CHARS))
    } else {
        String::new()
    };
    let repeat = if line.standing { "  (repeating)" } else { "" };
    let head = format!("{site}{}  {}{repeat}", line.leg, line.due);
    let cargo = wrap_text(&line.cargo, PROSE_CHARS - INDENT.len());
    let mut rows = item_with_overflow(head, cargo, selected);
    if line.stalled {
        let first = rows.remove(0);
        rows.insert(0, with_icon(first, '!', hud::palette::ATTENTION));
    }
    rows
}

pub(in crate::render) fn routes_rows(routes: &[RouteLine], selected: usize) -> Vec<Row> {
    let mut rows = Vec::new();
    if routes.is_empty() {
        rows.push(text_row("No routes running here."));
    }
    for (i, line) in routes.iter().enumerate() {
        rows.extend(route_item(line, false, i == selected));
    }
    rows.push(item_row("New route…", selected == routes.len()));
    rows
}

pub(in crate::render) fn jobs_rows(contracts: &[ContractRow], selected: usize) -> Vec<Row> {
    if contracts.is_empty() {
        return vec![text_row("You hold no jobs from this town.")];
    }
    contracts
        .iter()
        .enumerate()
        .flat_map(|(i, c)| {
            let rest = wrap_text(&c.objective_line, PROSE_CHARS - INDENT.len());
            item_with_overflow(elide(&c.name, PROSE_CHARS), rest, i == selected)
        })
        .collect()
}

pub(in crate::render) fn log_rows(log: &[SiteLogLine], selected: usize) -> Vec<Row> {
    if log.is_empty() {
        return vec![text_row("Nothing has happened here yet.")];
    }
    log.iter()
        .enumerate()
        .flat_map(|(i, line)| {
            let age = format!("{:<12}", line.age);
            let mut wrapped = wrap_text(&line.text, PROSE_CHARS - age.len());
            let first = if wrapped.is_empty() {
                String::new()
            } else {
                wrapped.remove(0)
            };
            item_with_overflow(format!("{age}{first}"), wrapped, i == selected)
        })
        .collect()
}

pub(in crate::render) fn outpost_status_rows(report: &OutpostReport) -> Vec<Row> {
    let mut rows = vec![
        Row::TextColored(
            format!(
                "{} · {:?} · Tier {} · {}",
                elide(&report.name, NAME_CHARS),
                report.biome,
                report.tier + 1,
                report.tier_label
            ),
            CYAN,
        ),
        text_row(format!(
            "Growth  {}",
            text_growth_bar(report.growth_fill, GROWTH_BAR_WIDTH)
        )),
    ];
    rows.extend(prose(&report.reason, ""));
    rows.push(text_row(format!(
        "Integrity {}/{}   Stock {}/{}",
        report.integrity, report.max_integrity, report.stock, report.stock_cap
    )));
    rows.push(text_row(""));
    rows.push(heading("Yields now"));
    if report.yields.is_empty() {
        rows.push(text_row("  Nothing yet."));
    }
    for y in &report.yields {
        rows.push(text_row(format!(
            "  {}   tier {}",
            elide(&y.name, NAME_CHARS),
            y.tier
        )));
    }
    if let Some(next) = &report.next_tier_requirement {
        rows.extend(prose(next, "  "));
    }
    rows
}

pub(in crate::render) fn crew_rows(report: &OutpostReport, selected: usize) -> Vec<Row> {
    let mut rows = vec![heading(&format!(
        "Crew ({}/{OUTPOST_CREW_CAP})",
        report.crew.len()
    ))];
    if report.crew.is_empty() {
        rows.push(text_row("  Nobody is posted here."));
    }
    for (i, member) in report.crew.iter().enumerate() {
        rows.push(item_row(
            format!(
                "{}  {}  lv {}",
                elide(&member.name, NAME_CHARS),
                elide(&member.species, NAME_CHARS),
                member.level
            ),
            i == selected,
        ));
    }
    rows.push(item_row("Post a program…", selected == report.crew.len()));
    rows
}

pub(in crate::render) fn stock_rows(
    report: &OutpostReport,
    stock: &[(String, u32)],
    routes: &[RouteLine],
    selected: usize,
) -> Vec<Row> {
    let mut rows = vec![heading(&format!(
        "Stock {}/{}",
        report.stock, report.stock_cap
    ))];
    if stock.is_empty() {
        rows.push(text_row("  Nothing to take."));
    }
    for (i, (name, qty)) in stock.iter().enumerate() {
        rows.push(item_row(
            format!("{}  x{qty}", elide(name, NAME_CHARS)),
            i == selected,
        ));
    }
    rows.push(text_row(""));
    rows.push(heading("Routes"));
    if routes.is_empty() {
        rows.push(text_row("  No routes running here."));
    }
    for (i, line) in routes.iter().enumerate() {
        rows.extend(route_item(line, false, stock.len() + i == selected));
    }
    rows.push(item_row(
        "New outpost route…",
        selected == stock.len() + routes.len(),
    ));
    rows
}

pub(in crate::render) fn caravan_rows(schedule: &CaravanSchedule) -> Vec<Row> {
    let docked = match &schedule.docked {
        Some((name, leaves)) => format!("Docked: {} (leaves {leaves})", elide(name, NAME_CHARS)),
        None => "Docked: nobody".to_string(),
    };
    let next = match &schedule.next {
        Some((name, arrives)) => format!("Next: {} (arrives {arrives})", elide(name, NAME_CHARS)),
        None => "Next: no trader is expected".to_string(),
    };
    vec![text_row(docked), text_row(next)]
}

pub(in crate::render) fn flight_rows(routes: &[RouteLine], selected: usize) -> Vec<Row> {
    if routes.is_empty() {
        return vec![text_row("No routes are running.")];
    }
    routes
        .iter()
        .enumerate()
        .flat_map(|(i, line)| route_item(line, true, i == selected))
        .collect()
}
