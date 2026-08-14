//! The contracts screen: what the run is holding, then what the Broker in
//! front of you is offering.

use crate::*;

/// Which section of the contracts screen a picked row number lands in, and
/// its index within that section.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum ContractScreenRow {
    Active(usize),
    Offer(usize),
}

/// Resolves a row number against the screen's two stacked sections — active
/// contracts, then the board's offers — given how many rows each contributed.
/// `None` for a row past the end.
///
/// Pulled out of `handle_contracts_key` and shared with the renderer that
/// numbers the same rows, for `trade_row`'s stated reason: the offset
/// arithmetic is where a screen with more than one section goes wrong, and it
/// is the one part of this flow testable without a Broker to stand in front
/// of.
pub(crate) fn contract_row(idx: usize, active: usize, offers: usize) -> Option<ContractScreenRow> {
    if idx < active {
        return Some(ContractScreenRow::Active(idx));
    }
    let idx = idx - active;
    (idx < offers).then_some(ContractScreenRow::Offer(idx))
}

impl App {
    /// Both halves of the screen, in the order they are numbered. The handler
    /// and the renderer both call this rather than each asking the engine
    /// separately — a renderer that rebuilt the list would drift out of index
    /// with the handler and row 2 would act on a different contract from the
    /// one under the highlight.
    pub fn contract_sections(&mut self) -> (Vec<ContractRow>, Vec<ContractRow>) {
        let Some(game) = &mut self.game else {
            return (Vec::new(), Vec::new());
        };
        let offers = game.contract_board().unwrap_or_default();
        (game.active_contracts(), offers)
    }

    pub(crate) fn handle_contracts_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let (active, offers) = self.contract_sections();

        // Uppercase is reserved for screen actions, as `[S]`/`[B]` are on the
        // trade screen — a shifted letter that also picked a row would fire
        // both on one keypress, and one of these gives a contract up.
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
                    Some("Highlight a contract you are holding to give it back.".to_string());
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
                match game.accept_contract(&id) {
                    Ok(()) => self.status_line = None,
                    Err(why) => self.status_line = Some(refusal_line(why)),
                }
            }
            // Picking one you already hold hands over a delivery. Nothing
            // else on this screen is an action a held contract needs: the
            // other four objectives advance on their own, wherever you are.
            Some(ContractScreenRow::Active(row)) => {
                let id = active[row].id.clone();
                let Some(game) = &mut self.game else { return };
                match game.deliver_to_contract(&id) {
                    Ok(_) => self.status_line = None,
                    Err(ContractRefusal::NotOffered) => {
                        self.status_line =
                            Some("That one finishes itself — just go and do it.".to_string())
                    }
                    Err(why) => self.status_line = Some(refusal_line(why)),
                }
            }
            None => {}
        }
    }
}

/// A refusal in the player's words. The engine returns a typed
/// `ContractRefusal` precisely so this wording lives on the screen side,
/// where the vocabulary already is.
fn refusal_line(why: ContractRefusal) -> String {
    match why {
        ContractRefusal::TooMany => {
            format!("You are already holding {MAX_ACTIVE_CONTRACTS}. Finish or abandon one first.")
        }
        ContractRefusal::AlreadyActive => "You have that one already.".to_string(),
        ContractRefusal::AlreadyDone => "That one is finished.".to_string(),
        ContractRefusal::NotOffered => "Nobody here is offering that.".to_string(),
        ContractRefusal::NothingToDeliver => {
            "You are not carrying anything it asks for.".to_string()
        }
    }
}
