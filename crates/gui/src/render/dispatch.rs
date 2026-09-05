//! The Relay hub (`Mode::Dispatch`) and its two pickers,
//! `Mode::SortieSquad` and `Mode::RouteCargo` —
//! `render/settlement_market.rs` and `render/settlement_board.rs`'s shape,
//! one desk over.

use feral_processes_app_core::{RouteCargoBasket, SortieSquadRow};
use feral_processes_engine::routes::RouteLeg;
use feral_processes_engine::{RouteDestination, RouteReport, SortieReport, SortieRow};

use super::popup::*;
use super::*;

/// The hub itself: sortie sites, then route destinations, numbered
/// continuously the way `app::dispatch::dispatch_row` resolves a keypress
/// against them, so a row drawn here and a row acted on there can never
/// disagree about which index means what. Every trip in flight is drawn
/// below as read-only status.
pub(super) fn draw_dispatch(
    sections: Option<(&[SortieRow], &[RouteDestination])>,
    sortie_reports: &[SortieReport],
    route_reports: &[RouteReport],
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some((sites, destinations)) = sections else {
        draw_popup(
            "Relay",
            PopupSize::Small,
            &[text_row("No Relay stands yet.")],
            refusal,
            painter,
            m,
        );
        return;
    };
    let rows = dispatch_hub_rows(sites, destinations, sortie_reports, route_reports, selected);
    draw_popup("Relay", PopupSize::Large, &rows, refusal, painter, m);
}

pub(super) fn dispatch_hub_rows(
    sites: &[SortieRow],
    destinations: &[RouteDestination],
    sortie_reports: &[SortieReport],
    route_reports: &[RouteReport],
    selected: usize,
) -> Vec<Row> {
    let mut rows = vec![Row::TextColored("Sortie sites".to_string(), TEXT)];
    let mut idx = 0;
    if sites.is_empty() {
        rows.push(text_row("    Nothing on the board."));
    }
    for site in sites {
        rows.push(item_row(
            format!(
                "[{}] {} — risk +{}, {} fights, {} ticks",
                menu_shortcut(idx),
                site.name,
                site.risk,
                site.battles,
                site.ticks
            ),
            idx == selected,
        ));
        idx += 1;
    }

    rows.push(text_row(""));
    rows.push(Row::TextColored("Known destinations".to_string(), TEXT));
    if destinations.is_empty() {
        rows.push(text_row("    Nothing known yet."));
    }
    for dest in destinations {
        rows.push(item_row(
            format!(
                "[{}] {} — {}, {} ticks",
                menu_shortcut(idx),
                dest.name,
                dest.band.label(),
                dest.ticks
            ),
            idx == selected,
        ));
        idx += 1;
    }

    rows.push(text_row(""));
    rows.push(Row::TextColored("In flight".to_string(), TEXT));
    if sortie_reports.is_empty() && route_reports.is_empty() {
        rows.push(text_row("    Nothing away."));
    }
    for report in sortie_reports {
        rows.push(text_row(format!(
            "    {} — {}/{} fights, {} ticks left{}",
            report.site,
            report.battles_done,
            report.battles_total,
            report.ticks_left,
            if report.aborted {
                " (falling back)"
            } else {
                ""
            }
        )));
    }
    for report in route_reports {
        let leg = match report.leg {
            RouteLeg::Outbound => "outbound",
            RouteLeg::Inbound => "inbound",
        };
        let status = if report.stalled {
            " (stalled)"
        } else if report.standing {
            " (standing)"
        } else {
            ""
        };
        rows.push(text_row(format!(
            "    {} — {leg} leg, {} ticks left{status}",
            report.destination_name, report.ticks_left
        )));
    }

    rows.push(text_row(""));
    rows.extend(dispatch_hub_footer().into_iter().map(text_row));
    rows
}

/// Split out for the census below, `board_footer`'s reason.
fn dispatch_hub_footer() -> [&'static str; 2] {
    [
        "[S] send a squad  ·  [C] send cargo  ·  [X] cut a standing route",
        "Esc to go back",
    ]
}

/// The squad picker, opened with `[S]` on the hub for one sortie site.
pub(super) fn draw_sortie_squad(
    site: Option<&SortieRow>,
    candidates: &[SortieSquadRow],
    cost: &[(String, u32)],
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(site) = site else {
        draw_popup(
            "Send a squad",
            PopupSize::Small,
            &[text_row("Nothing to report.")],
            refusal,
            painter,
            m,
        );
        return;
    };
    let title = format!("Send a squad — {}", site.name);
    let rows = sortie_squad_rows(site, candidates, cost, selected);
    draw_popup(&title, PopupSize::Large, &rows, refusal, painter, m);
}

pub(super) fn sortie_squad_rows(
    site: &SortieRow,
    candidates: &[SortieSquadRow],
    cost: &[(String, u32)],
    selected: usize,
) -> Vec<Row> {
    let mut rows = vec![
        text_row(format!(
            "{} fights, about {} ticks there and back.",
            site.battles, site.ticks
        )),
        text_row(""),
    ];
    if candidates.is_empty() {
        rows.push(text_row(
            "Nobody is free to send — every program is in the party, wielded, or already away.",
        ));
    }
    for (idx, candidate) in candidates.iter().enumerate() {
        let mark = if candidate.picked { 'x' } else { ' ' };
        rows.push(item_row(
            format!("[{}] [{mark}] {}", menu_shortcut(idx), candidate.name),
            idx == selected,
        ));
    }
    rows.push(text_row(""));
    if cost.is_empty() {
        rows.push(text_row("Provisioning: nothing, with no squad picked yet."));
    } else {
        let line = cost
            .iter()
            .map(|(name, qty)| format!("{qty} {name}"))
            .collect::<Vec<_>>()
            .join(", ");
        rows.push(text_row(format!("Provisioning: {line}")));
    }
    rows.push(text_row(""));
    rows.push(text_row(
        "[X] toggle the highlighted program  ·  Enter to dispatch  ·  Esc to go back",
    ));
    rows
}

/// The cargo picker, opened with `[C]` on the hub for one destination.
pub(super) fn draw_route_cargo(
    basket: Option<RouteCargoBasket>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(basket) = basket else {
        draw_popup(
            "Send cargo",
            PopupSize::Small,
            &[text_row("Nothing to report.")],
            refusal,
            painter,
            m,
        );
        return;
    };
    let title = format!("Send cargo — {}", basket.destination_name);
    let rows = route_cargo_rows(&basket, selected);
    draw_popup(&title, PopupSize::Large, &rows, refusal, painter, m);
}

pub(super) fn route_cargo_rows(basket: &RouteCargoBasket, selected: usize) -> Vec<Row> {
    let mut rows = vec![
        text_row(if basket.standing {
            "Standing: yes — reloads and departs again on arrival home."
        } else {
            "Standing: no — a one-off trip."
        }),
        text_row(format!(
            "This basket would sell for {} at the destination.",
            basket.quote
        )),
        text_row(""),
    ];
    if basket.stock.is_empty() {
        rows.push(text_row("(nothing on the shelves)"));
    }
    for (idx, row) in basket.stock.iter().enumerate() {
        let (amount, ceiling) = basket.cells.get(idx).copied().unwrap_or((0, 0));
        rows.push(item_row(
            format!(
                "[{}] {} ({amount} of {ceiling})",
                menu_shortcut(idx),
                row.name
            ),
            idx == selected,
        ));
    }
    rows.push(text_row(""));
    rows.push(text_row(
        "Left/Right set a row  ·  Shift jumps to the end  ·  Ctrl halves the gap",
    ));
    rows.push(text_row(
        "[T] toggle standing  ·  [N] clear  ·  Enter to dispatch  ·  Esc to step away",
    ));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;
    use feral_processes_engine::settlements::relations::Standing;
    use feral_processes_engine::settlements::{SettlementDb, SettlementKey};
    use feral_processes_engine::sorties::SortieDb;
    use feral_processes_engine::{DifficultyMode, Game};

    fn assets() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    fn widest_room(screen_w: f32, m: &Metrics) -> f32 {
        popup_body_width(screen_w, PopupSize::Large, m)
    }

    fn assert_rows_fit(rows: &[Row]) {
        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = widest_room(1440.0, &m);
            for row in rows {
                let line = row_label_text(row);
                let drawn = p.measure_ui_advance(&line, m.font_size);
                assert!(
                    drawn <= room,
                    "a dispatch row overflows the page by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
        });
    }

    /// The hub's worst case: the widest shipped sortie site and the widest
    /// shipped settlement name, each drawn on its own row.
    #[test]
    fn no_dispatch_hub_row_overflows_its_popup() {
        let (sorties, warnings) =
            SortieDb::load_dir(&assets().join("sorties")).expect("the catalogue loads");
        assert!(warnings.is_empty(), "{warnings:?}");
        let widest_site = sorties
            .iter()
            .max_by_key(|def| def.name.chars().count())
            .expect("the shipped catalogue defines a sortie site")
            .clone();

        let (settlements, warnings) =
            SettlementDb::load_dir(&assets().join("settlements")).expect("the catalogue loads");
        assert!(warnings.is_empty(), "{warnings:?}");
        let widest_town_name = settlements
            .iter()
            .max_by_key(|def| def.name.chars().count())
            .expect("the shipped catalogue defines a settlement")
            .name
            .clone();

        let sites = vec![SortieRow {
            id: widest_site.id.clone(),
            name: widest_site.name.clone(),
            description: widest_site.description.clone(),
            risk: 99,
            battles: 9,
            ticks: 9_999,
        }];
        let destinations = vec![RouteDestination {
            destination: SettlementKey { rx: 0, ry: 0 },
            name: widest_town_name,
            band: Standing::Allied,
            ticks: 9_999,
        }];
        let sortie_reports = vec![SortieReport {
            site: sites[0].name.clone(),
            members: vec!["A".to_string()],
            casualties: Vec::new(),
            kills: 9,
            xp: 9_999,
            battles_done: 9,
            battles_total: 9,
            ticks_left: 9_999,
            aborted: true,
        }];
        let route_reports = vec![RouteReport {
            destination: destinations[0].destination,
            destination_name: destinations[0].name.clone(),
            standing: true,
            stalled: true,
            leg: RouteLeg::Inbound,
            cargo: Vec::new(),
            ticks_left: 9_999,
            proceeds: 9_999,
        }];

        let rows = dispatch_hub_rows(&sites, &destinations, &sortie_reports, &route_reports, 0);
        assert_rows_fit(&rows);
    }

    /// The squad picker's worst case: a program name as long as
    /// `MAX_CUSTOM_NAME_LEN` allows plus its species-derived fallback, and
    /// every provisioning line named.
    #[test]
    fn no_sortie_squad_row_overflows_its_popup() {
        let game = Game::new(41, DifficultyMode::Forgiving, &assets()).expect("shipped assets");
        let widest_item = game
            .item_defs()
            .iter()
            .max_by_key(|d| d.name.chars().count())
            .expect("the item set is not empty")
            .name
            .clone();
        let site = SortieRow {
            id: feral_processes_engine::sorties::SortieId::from("test"),
            name: "A very long dispatch site name indeed".to_string(),
            description: String::new(),
            risk: 9,
            battles: 9,
            ticks: 9_999,
        };
        let candidates: Vec<SortieSquadRow> = (0..8)
            .map(|i| SortieSquadRow {
                entity: feral_processes_engine::Entity::PLACEHOLDER,
                name: format!("A Very Long Program Name Indeed {i}"),
                picked: i % 2 == 0,
            })
            .collect();
        let cost = vec![(widest_item, 9_999)];
        let rows = sortie_squad_rows(&site, &candidates, &cost, 0);
        assert_rows_fit(&rows);
    }

    /// The cargo picker's worst case: the widest shipped item name, held and
    /// quoted at the top of their ranges.
    #[test]
    fn no_route_cargo_row_overflows_its_popup() {
        let game = Game::new(42, DifficultyMode::Forgiving, &assets()).expect("shipped assets");
        let defs = game.item_defs();
        let widest_item = defs
            .iter()
            .max_by_key(|d| d.name.chars().count())
            .expect("the item set is not empty");
        let stock = vec![feral_processes_engine::StockRow {
            item: widest_item.id.clone(),
            tag: widest_item.tag(),
            name: widest_item.name.clone(),
            qty: 9_999,
        }];
        let basket = RouteCargoBasket {
            destination: SettlementKey { rx: 0, ry: 0 },
            destination_name: "A Very Long Settlement Name Indeed".to_string(),
            stock,
            cells: vec![(9_999, 9_999)],
            quote: 9_999_999,
            standing: true,
        };
        let rows = route_cargo_rows(&basket, 0);
        assert_rows_fit(&rows);
    }
}
