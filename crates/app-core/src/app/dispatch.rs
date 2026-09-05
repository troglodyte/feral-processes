//! The Relay hub (`Mode::Dispatch`) and its two pickers — a squad
//! (`Mode::SortieSquad`) and a cargo basket (`Mode::RouteCargo`).
//!
//! `app::contracts`/`app::settlement_board`'s shape: rows are numbered
//! continuously across two sections, resolved through one function both the
//! handler and the renderer call, so a keypress and a drawn row can never
//! disagree about which site or destination it means. The picker screens
//! that follow are `app::settlement_market`'s basket shape, one door over.

use crate::app::basket::halve;
use crate::*;

/// A row on `Mode::Dispatch`'s numbered list — sortie sites first, then
/// route destinations, `contract_row`'s own two-section shape.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum DispatchRow {
    Site(usize),
    Destination(usize),
}

/// Resolves a row number against the hub's two stacked sections. `None` for
/// a row past the end.
pub(crate) fn dispatch_row(idx: usize, sites: usize, destinations: usize) -> Option<DispatchRow> {
    if idx < sites {
        return Some(DispatchRow::Site(idx));
    }
    let idx = idx - sites;
    (idx < destinations).then_some(DispatchRow::Destination(idx))
}

/// Why a `SortieRefusal` or `RouteRefusal` was refused, worded for the
/// status line and the log — `app::contracts::refusal_line`'s shape, one
/// door for each engine refusal type since neither screen otherwise touches
/// `Game::item_name`.
pub(crate) fn sortie_refusal_line(game: &Game, why: SortieRefusal) -> String {
    match why {
        SortieRefusal::NotAtRelay => {
            "You need to be standing in your base, at the Relay, to dispatch a squad.".to_string()
        }
        SortieRefusal::NotOffered => "That site isn't on the board anymore.".to_string(),
        SortieRefusal::NoSquad => "Pick at least one program to send.".to_string(),
        SortieRefusal::Duplicate => "You picked the same program twice.".to_string(),
        SortieRefusal::NotStaff(name) => {
            format!("{name} isn't base staff — pull it out of the party or off the wield first.")
        }
        SortieRefusal::Downed(name) => {
            format!("{name} is down and needs a Bay before it can go anywhere.")
        }
        SortieRefusal::Wounded(name) => format!("{name} is too hurt to send out."),
        SortieRefusal::WouldEmptyTheBase => "That would leave nobody at the base.".to_string(),
        SortieRefusal::Unprovisioned { item, need, held } => format!(
            "Not enough {} to provision the squad: need {need}, have {held}.",
            game.item_name(&item)
        ),
    }
}

/// `sortie_refusal_line`'s twin for a caravan route.
pub(crate) fn route_refusal_line(game: &Game, why: RouteRefusal) -> String {
    match why {
        RouteRefusal::NotAtRelay => {
            "You need to be standing in your base, at the Relay, to dispatch a caravan.".to_string()
        }
        RouteRefusal::UnknownDestination => "You haven't found that settlement yet.".to_string(),
        RouteRefusal::Refused => "They won't deal with you.".to_string(),
        RouteRefusal::NoStandingRoutes => {
            "They don't trust you enough yet for a standing arrangement.".to_string()
        }
        RouteRefusal::EmptyManifest => "Put something in the cargo first.".to_string(),
        RouteRefusal::Understocked { item, need, held } => format!(
            "Not enough {} on hand: need {need}, have {held}.",
            game.item_name(&item)
        ),
        RouteRefusal::Duplicate => "A caravan is already on its way there.".to_string(),
        RouteRefusal::TooMany => "Too many caravans are already out.".to_string(),
    }
}

/// One candidate on `Mode::SortieSquad`'s list.
pub struct SortieSquadRow {
    pub entity: Entity,
    pub name: String,
    /// Whether `[X]` has already put this program in the squad.
    pub picked: bool,
}

/// A cargo basket over `Game::base_stock`, and what it is worth at the
/// picked destination — `settlement_market::SettlementMarketBasket`'s shape.
pub struct RouteCargoBasket {
    pub destination: SettlementKey,
    pub destination_name: String,
    pub stock: Vec<StockRow>,
    /// `(amount, ceiling)` per `stock` row, index-aligned with
    /// `App::route_cargo_amounts` exactly.
    pub cells: Vec<(u32, u32)>,
    /// What the basket, as built, would sell for — `Game::route_manifest_quote`,
    /// so this can never quote a different figure from the sale it previews.
    pub quote: u32,
    pub standing: bool,
}

impl App {
    /// Both halves of the hub's numbered list, in the order they are
    /// numbered — `contract_sections`' shape. `None` with no Relay standing
    /// anywhere in the run, `sortie_board`/`route_destinations`' own gate.
    pub fn dispatch_hub_sections(&mut self) -> Option<(Vec<SortieRow>, Vec<RouteDestination>)> {
        let game = self.game.as_mut()?;
        let sites = game.sortie_board()?;
        let destinations = game.route_destinations().unwrap_or_default();
        Some((sites, destinations))
    }

    /// Every trip currently away, for the hub's read-only status list —
    /// `contract_sections`' shape again, but neither half is numbered: there
    /// is nothing on either row to pick.
    pub fn dispatch_trip_reports(&mut self) -> (Vec<SortieReport>, Vec<RouteReport>) {
        let Some(game) = &self.game else {
            return (Vec::new(), Vec::new());
        };
        (game.sortie_reports(), game.route_reports())
    }

    pub(crate) fn handle_dispatch_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let (sites, destinations) = self.dispatch_hub_sections().unwrap_or_default();
        self.menu_selected = self
            .menu_selected
            .min((sites.len() + destinations.len()).saturating_sub(1));

        match key {
            GameKey::Char('S') => {
                match dispatch_row(self.menu_selected, sites.len(), destinations.len()) {
                    Some(DispatchRow::Site(i)) => {
                        self.pending_dispatch_site = Some(sites[i].id.clone());
                        self.dispatch_squad.clear();
                        self.status_line = None;
                        self.menu_selected = 0;
                        self.mode = Mode::SortieSquad;
                    }
                    _ => self.refuse("Highlight a site to send a squad to."),
                }
            }
            GameKey::Char('C') => {
                match dispatch_row(self.menu_selected, sites.len(), destinations.len()) {
                    Some(DispatchRow::Destination(i)) => {
                        self.pending_dispatch_destination = Some(destinations[i].destination);
                        self.route_cargo_amounts.clear();
                        self.route_standing = false;
                        self.status_line = None;
                        self.menu_selected = 0;
                        self.mode = Mode::RouteCargo;
                    }
                    _ => self.refuse("Highlight a destination to send cargo to."),
                }
            }
            GameKey::Char('X') => {
                match dispatch_row(self.menu_selected, sites.len(), destinations.len()) {
                    Some(DispatchRow::Destination(i)) => {
                        let target = destinations[i].destination;
                        let Some(game) = &mut self.game else { return };
                        if game.sever_route(target) {
                            self.status_line = None;
                        } else {
                            self.refuse("There's no standing route there to cut.");
                        }
                    }
                    _ => self.refuse("Highlight a destination to cut a standing route from."),
                }
            }
            _ => {
                if let Some(idx) = self.selected_index(key, sites.len() + destinations.len()) {
                    self.menu_selected = idx;
                }
            }
        }
    }

    /// Every base-staff program, and whether `[X]` has already put it in
    /// `dispatch_squad`.
    pub fn sortie_squad_candidates(&mut self) -> Vec<SortieSquadRow> {
        let picked = self.dispatch_squad.clone();
        let Some(game) = &self.game else {
            return Vec::new();
        };
        game.base_staff()
            .into_iter()
            .map(|entity| SortieSquadRow {
                entity,
                name: game.creature_label(entity),
                picked: picked.contains(&entity),
            })
            .collect()
    }

    /// The board row `App::pending_dispatch_site` names, re-resolved off the
    /// live board rather than cached — the board is derived and can rotate
    /// under a page left open, `sortie_board`'s own three-state rule.
    pub fn sortie_squad_site(&mut self) -> Option<SortieRow> {
        let id = self.pending_dispatch_site.clone()?;
        let game = self.game.as_mut()?;
        game.sortie_board()?.into_iter().find(|r| r.id == id)
    }

    /// What provisioning the squad built so far would cost, named rather
    /// than an `ItemId` — `Game::sortie_provision_cost`, the same call
    /// `dispatch_sortie` prices its own charge through, so this page cannot
    /// quote a figure the dispatch disagrees with.
    pub fn sortie_squad_cost(&mut self) -> Vec<(String, u32)> {
        let Some(site) = self.sortie_squad_site() else {
            return Vec::new();
        };
        let squad = self.dispatch_squad.len();
        let Some(game) = &self.game else {
            return Vec::new();
        };
        game.sortie_provision_cost(site.battles, squad)
            .into_iter()
            .map(|(item, qty)| (game.item_name(&item).to_string(), qty))
            .collect()
    }

    pub(crate) fn handle_sortie_squad_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_dispatch_site = None;
            self.dispatch_squad.clear();
            self.status_line = None;
            self.mode = Mode::Dispatch;
            return;
        }
        if self.pending_dispatch_site.is_none() {
            self.mode = Mode::Dispatch;
            return;
        }
        let candidates = self.sortie_squad_candidates();
        match key {
            GameKey::Up | GameKey::Down => self.scroll(key, candidates.len()),
            GameKey::Char('X') => {
                if let Some(row) = candidates.get(self.menu_selected) {
                    match self.dispatch_squad.iter().position(|&e| e == row.entity) {
                        Some(pos) => {
                            self.dispatch_squad.remove(pos);
                        }
                        None => self.dispatch_squad.push(row.entity),
                    }
                }
            }
            GameKey::Enter => {
                let Some(site) = self.pending_dispatch_site.clone() else {
                    self.mode = Mode::Dispatch;
                    return;
                };
                let squad = self.dispatch_squad.clone();
                let Some(game) = &mut self.game else { return };
                match game.dispatch_sortie(&site, &squad) {
                    Ok(()) => {
                        self.pending_dispatch_site = None;
                        self.dispatch_squad.clear();
                        self.status_line = None;
                        self.mode = Mode::Dispatch;
                    }
                    Err(e) => {
                        let line = sortie_refusal_line(game, e);
                        self.refuse(line);
                    }
                }
            }
            _ => {
                if let Some(idx) = self.selected_index(key, candidates.len()) {
                    self.menu_selected = idx;
                }
            }
        }
    }

    /// The cargo basket over `Game::base_stock` for `pending_dispatch_destination`
    /// — `None` only when there is no active game or no pending destination
    /// at all. **Not** a graceful fallback for a destination that has since
    /// vanished: `game.settlement_report(destination)` below `.expect()`s a
    /// `resources::Settlements` record for the key, which is safe only
    /// because nothing removes a settlement from that resource today — a
    /// stale key here panics rather than drawing nothing. A future removal
    /// path needs its own refusal before this comment's old claim is true.
    pub fn route_cargo_basket(&mut self) -> Option<RouteCargoBasket> {
        let destination = self.pending_dispatch_destination?;
        let game = self.game.as_ref()?;
        let stock = game.base_stock();
        if self.route_cargo_amounts.len() != stock.len() {
            self.route_cargo_amounts = vec![0; stock.len()];
        }
        let cells: Vec<(u32, u32)> = stock
            .iter()
            .enumerate()
            .map(|(i, row)| {
                (
                    self.route_cargo_amounts.get(i).copied().unwrap_or(0),
                    row.qty,
                )
            })
            .collect();
        let cargo = route_cargo_manifest(&self.route_cargo_amounts, &stock);
        let game = self.game.as_ref()?;
        let quote = game.route_manifest_quote(destination, &cargo).unwrap_or(0);
        let destination_name = game.settlement_name(destination);
        Some(RouteCargoBasket {
            destination,
            destination_name,
            stock,
            cells,
            quote,
            standing: self.route_standing,
        })
    }

    fn edit_route_cargo_row(&mut self, stock: &[StockRow], f: impl FnOnce(u32, u32) -> u32) {
        let row = self.menu_selected;
        if row >= self.route_cargo_amounts.len() {
            return;
        }
        let ceiling = stock.get(row).map(|r| r.qty).unwrap_or(0);
        if let Some(n) = self.route_cargo_amounts.get_mut(row) {
            *n = f(*n, ceiling).min(ceiling);
        }
    }

    pub(crate) fn handle_route_cargo_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_dispatch_destination = None;
            self.route_cargo_amounts.clear();
            self.status_line = None;
            self.mode = Mode::Dispatch;
            return;
        }
        let Some(destination) = self.pending_dispatch_destination else {
            self.mode = Mode::Dispatch;
            return;
        };
        let Some(game) = &self.game else { return };
        let stock = game.base_stock();
        if self.route_cargo_amounts.len() != stock.len() {
            self.route_cargo_amounts = vec![0; stock.len()];
        }
        self.menu_selected = self.menu_selected.min(stock.len().saturating_sub(1));

        match key {
            GameKey::Up | GameKey::Down => self.scroll(key, stock.len()),
            GameKey::Char('T') => self.route_standing = !self.route_standing,
            GameKey::Char('N') => self.route_cargo_amounts.iter_mut().for_each(|n| *n = 0),
            GameKey::Left => self.edit_route_cargo_row(&stock, |n, _| n.saturating_sub(1)),
            GameKey::Right => self.edit_route_cargo_row(&stock, |n, _| n.saturating_add(1)),
            GameKey::ShiftLeft => self.edit_route_cargo_row(&stock, |_, _| 0),
            GameKey::ShiftRight => self.edit_route_cargo_row(&stock, |_, ceiling| ceiling),
            GameKey::CtrlLeft => self.edit_route_cargo_row(&stock, |n, _| halve(n, 0)),
            GameKey::CtrlRight => self.edit_route_cargo_row(&stock, halve),
            GameKey::Enter => {
                let cargo = route_cargo_manifest(&self.route_cargo_amounts, &stock);
                let standing = self.route_standing;
                let Some(game) = &mut self.game else { return };
                match game.dispatch_route(destination, cargo, standing) {
                    Ok(()) => {
                        self.pending_dispatch_destination = None;
                        self.route_cargo_amounts.clear();
                        self.route_standing = false;
                        self.status_line = None;
                        self.mode = Mode::Dispatch;
                    }
                    Err(e) => {
                        let line = route_refusal_line(game, e);
                        self.refuse(line);
                    }
                }
            }
            _ => {}
        }
    }
}

/// `amounts` and `stock`, zipped down into what `Game::dispatch_route`
/// wants — every zero row dropped, since an untouched row is not part of
/// the manifest.
fn route_cargo_manifest(amounts: &[u32], stock: &[StockRow]) -> Vec<(ItemId, u32)> {
    amounts
        .iter()
        .zip(stock.iter())
        .filter(|(n, _)| **n > 0)
        .map(|(&n, row)| (row.item.clone(), n))
        .collect()
}
