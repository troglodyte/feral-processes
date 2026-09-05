//! A town's job board — Phase 5.
//!
//! `app::contracts`' screen, one vendor over, and it deliberately reuses that
//! module's `contract_row` resolver rather than restating the offset
//! arithmetic: two stacked sections numbered continuously is exactly the
//! shape, and a second copy of it is where a screen with more than one
//! section goes wrong.
//!
//! The one difference is `issuer`. Every engine door here is called with
//! `Some(key)`, and the held section is filtered to that town's own jobs —
//! **a contract is delivered where it was signed**, so a row for the
//! Broker's delivery sitting on a town's board could only ever refuse.

use crate::app::contracts::{ContractScreenRow, contract_row, refusal_line};
use crate::*;

impl App {
    /// Both halves, in the order they are numbered — `contract_sections`'
    /// contract, and asked by the handler and the renderer alike so row 2
    /// cannot mean two different jobs.
    pub fn settlement_board_sections(&mut self) -> (Vec<ContractRow>, Vec<ContractRow>) {
        let Some(key) = self.pending_settlement else {
            return (Vec::new(), Vec::new());
        };
        let Some(game) = &mut self.game else {
            return (Vec::new(), Vec::new());
        };
        let offers = game.settlement_board(key).unwrap_or_default();
        let held = game
            .active_contracts()
            .into_iter()
            .filter(|row| row.issuer == Some(key))
            .collect();
        (held, offers)
    }

    pub(crate) fn handle_settlement_board_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.status_line = None;
            self.mode = Mode::Settlement;
            return;
        }
        let Some(town) = self.pending_settlement else {
            return;
        };
        let (active, offers) = self.settlement_board_sections();

        // Uppercase, `handle_contracts_key`'s reason: a shifted letter that
        // also picked a row would fire both on one keypress, and this one
        // gives a contract up.
        if key == GameKey::Char('A') {
            let idx = self.menu_selected;
            if let Some(ContractScreenRow::Active(row)) =
                contract_row(idx, active.len(), offers.len())
            {
                let id = active[row].id.clone();
                if let Some(game) = &mut self.game {
                    game.abandon_contract(&id);
                }
                self.status_line = None;
            } else {
                self.status_line =
                    Some("Highlight a job you are holding to give it back.".to_string());
            }
            return;
        }

        let Some(idx) = self.selected_index(key, active.len() + offers.len()) else {
            return;
        };
        match contract_row(idx, active.len(), offers.len()) {
            Some(ContractScreenRow::Offer(row)) => {
                let id = offers[row].id.clone();
                let Some(game) = &mut self.game else { return };
                let outcome = game.accept_contract(&id, Some(town)).map_err(refusal_line);
                self.report(outcome);
            }
            Some(ContractScreenRow::Active(row)) => {
                let id = active[row].id.clone();
                let Some(game) = &mut self.game else { return };
                let outcome = game
                    .deliver_to_contract(&id, Some(town))
                    .map(|_| ())
                    .map_err(|why| match why {
                        ContractRefusal::NotOffered => {
                            "That one finishes itself — just go and do it.".to_string()
                        }
                        why => refusal_line(why),
                    });
                self.report(outcome);
            }
            None => {}
        }
    }
}
