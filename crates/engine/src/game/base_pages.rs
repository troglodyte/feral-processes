//! The base report's machine, power, stock and build-order pages: the
//! reports each page draws and the gap flags built from them, so a page and
//! its rail `!` cannot disagree.

use crate::Game;
use crate::components::{Durability, MachineStatus, Position, Structure};
use crate::structures::StructureDb;
use crate::views::{MachineRow, OrderState, PowerLine, PowerReport, StockReportRow};
use std::collections::BTreeMap;

/// Worst first on the Machines screen: a dark machine outranks everything
/// the player could fix about it, as the status's own doc argues.
const MACHINE_ORDER: [MachineStatus; 9] = [
    MachineStatus::Unpowered,
    MachineStatus::Dry,
    MachineStatus::Uncharged,
    MachineStatus::Starved,
    MachineStatus::Unstaffed,
    MachineStatus::Stranded,
    MachineStatus::Clogged,
    MachineStatus::Idle,
    MachineStatus::Running,
];

/// The statuses that put the Production page's `!` up: a machine that wants
/// something done. `Clogged` and `Idle` are listed on the page but are
/// not by themselves a gap. Exhaustive, so a new status must be decided.
fn is_machine_gap(status: MachineStatus) -> bool {
    match status {
        MachineStatus::Unpowered
        | MachineStatus::Dry
        | MachineStatus::Uncharged
        | MachineStatus::Starved
        | MachineStatus::Unstaffed
        | MachineStatus::Stranded => true,
        MachineStatus::Clogged | MachineStatus::Idle | MachineStatus::Running => false,
    }
}

impl Game {
    /// Structures that are not running, grouped by status worst first, then
    /// any other structure below full durability.
    pub fn machine_report(&self) -> Vec<MachineRow> {
        let mut rows: Vec<MachineRow> = self
            .world
            .iter_entities()
            .filter(|e| e.contains::<Structure>())
            .filter_map(|e| {
                let status = e.get::<MachineStatus>().copied();
                let damage = e
                    .get::<Durability>()
                    .filter(|d| d.hp < d.max_hp)
                    .map(|d| (d.hp, d.max_hp));
                if status.is_none_or(|s| s == MachineStatus::Running) && damage.is_none() {
                    return None;
                }
                let pos = e.get::<Position>()?;
                Some(MachineRow {
                    entity: e.id(),
                    label: self.entity_label(e.id()),
                    pos: (pos.x, pos.y),
                    status: status.unwrap_or_default(),
                    damage,
                })
            })
            .collect();
        let rank = |s: MachineStatus| MACHINE_ORDER.iter().position(|&o| o == s);
        rows.sort_by(|a, b| {
            rank(a.status)
                .cmp(&rank(b.status))
                .then_with(|| a.label.cmp(&b.label))
                .then_with(|| a.pos.cmp(&b.pos))
        });
        rows
    }

    /// The grid: `base_power`'s totals and what each structure adds to them,
    /// biggest contributor first.
    pub fn power_report(&self) -> PowerReport {
        let db = self.world.resource::<StructureDb>();
        let items = self.world.resource::<crate::items_db::ItemDb>();
        let ledger = crate::game::base::power::ledger(&self.world, db, items);
        let mut lines: Vec<PowerLine> = ledger
            .entries
            .iter()
            .filter_map(|entry| {
                let pos = self.world.get::<Position>(entry.entity)?;
                Some(PowerLine {
                    label: self.entity_label(entry.entity),
                    pos: (pos.x, pos.y),
                    supply: entry.supply,
                    draw: entry.draw,
                    dark: ledger.dark.contains(&entry.entity),
                })
            })
            .collect();
        lines.sort_by(|a, b| {
            (b.supply.max(b.draw))
                .cmp(&a.supply.max(a.draw))
                .then_with(|| a.label.cmp(&b.label))
                .then_with(|| a.pos.cmp(&b.pos))
        });
        PowerReport {
            draw: ledger.draw,
            supply: ledger.supply,
            lines,
        }
    }

    /// What the base holds, joined to the work order that asks for more of
    /// it: rows short of a target first, then the rest in `base_stock`'s
    /// order. An ordered item the stock block does not list (not a material)
    /// still gets its row, at what the order reports the base holding.
    pub fn stock_report(&self) -> Vec<StockReportRow> {
        let mut rows: BTreeMap<crate::items::ItemId, StockReportRow> = self
            .base_stock()
            .into_iter()
            .map(|s| {
                (
                    s.item.clone(),
                    StockReportRow {
                        item: s.item,
                        tag: s.tag,
                        name: s.name,
                        qty: s.qty,
                        order: None,
                    },
                )
            })
            .collect();
        let items = self.world.resource::<crate::items_db::ItemDb>();
        for order in self.work_order_report() {
            let row = rows
                .entry(order.item.clone())
                .or_insert_with(|| StockReportRow {
                    tag: items
                        .get(order.item.as_str())
                        .map(|d| d.tag())
                        .unwrap_or_default(),
                    name: order.label.clone(),
                    qty: order.have,
                    item: order.item.clone(),
                    order: None,
                });
            // Two orders for one item: the stalled one is the one worth
            // showing, then the larger target.
            let better = match row.order {
                None => true,
                Some((target, state)) => {
                    let stalled = |s| s == OrderState::Stalled;
                    (stalled(order.state), order.target) > (stalled(state), target)
                }
            };
            // The order's own holding, so `is_short` agrees with the order
            // whatever the stock block counts besides.
            row.qty = order.have;
            if better {
                row.order = Some((order.target, order.state));
            }
        }
        let mut rows: Vec<StockReportRow> = rows.into_values().collect();
        rows.sort_by_key(|r| !r.is_short());
        rows
    }

    /// Production's reason: machines that want something done, and a grid
    /// that cannot carry its draw. `None` when neither is true.
    pub(super) fn production_flag(&self) -> Option<String> {
        let stalled = self
            .machine_report()
            .iter()
            .filter(|m| is_machine_gap(m.status))
            .count();
        let power = self.power_report();
        let mut reasons = Vec::new();
        if stalled > 0 {
            reasons.push(format!("{stalled} stalled"));
        }
        if power.draw > power.supply {
            reasons.push(format!("grid short {}/{}", power.draw, power.supply));
        }
        (!reasons.is_empty()).then(|| reasons.join(", "))
    }

    /// Stock's reason: targets the base is short of whose order is stalled.
    pub(super) fn stock_flag(&self) -> Option<String> {
        let short = self
            .stock_report()
            .iter()
            .filter(|r| r.is_stalled_short())
            .count();
        (short > 0).then(|| format!("{short} short, stalled"))
    }

    /// Build orders' reason: requests waiting on the player's program, and
    /// requests the crew has said it cannot carry on.
    pub(super) fn build_orders_flag(&self) -> Option<String> {
        let report = self.build_order_report();
        let waiting = report.iter().filter(|b| b.awaiting_program).count();
        let blocked = report.iter().filter(|b| b.blocked.is_some()).count();
        let mut reasons = Vec::new();
        if waiting > 0 {
            reasons.push(format!("{waiting} awaiting a program"));
        }
        if blocked > 0 {
            reasons.push(format!("{blocked} blocked"));
        }
        (!reasons.is_empty()).then(|| reasons.join(", "))
    }
}
