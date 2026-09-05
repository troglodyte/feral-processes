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

    /// Whether the offers on screen can be acted on from where the player is
    /// standing. The renderer says so in the board's header rather than
    /// leaving the player to press a key and read a refusal.
    ///
    /// Asked of the engine rather than derived from the two lists being
    /// non-empty, because "there is a board" and "you may take from it" are
    /// exactly the two things `Game::broker_reach` exists to keep from
    /// drifting apart.
    pub fn broker_reach(&mut self) -> BrokerReach {
        self.game
            .as_mut()
            .map(|game| game.broker_reach())
            .unwrap_or(BrokerReach::NoBroker)
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
                // The engine refuses this too — that is where the invariant
                // lives — but a bare `false` cannot reach the log, and a key
                // that silently does nothing reads as a broken key.
                if active[row].tutorial {
                    self.refuse(
                        "Onboarding missions cannot be given back — finish this one and \
                         the next arrives.",
                    );
                    return;
                }
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
                let outcome = game.accept_contract(&id, None).map_err(refusal_line);
                self.report(outcome);
            }
            // Picking one you already hold hands over a delivery. Nothing
            // else on this screen is an action a held contract needs: the
            // other four objectives advance on their own, wherever you are.
            Some(ContractScreenRow::Active(row)) => {
                let id = active[row].id.clone();
                let Some(game) = &mut self.game else { return };
                let outcome = game
                    .deliver_to_contract(&id, None)
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

/// A refusal in the player's words. The engine returns a typed
/// `ContractRefusal` precisely so this wording lives on the screen side,
/// where the vocabulary already is.
pub(crate) fn refusal_line(why: ContractRefusal) -> String {
    match why {
        ContractRefusal::TooMany => {
            format!("You are already holding {MAX_ACTIVE_CONTRACTS}. Finish or abandon one first.")
        }
        ContractRefusal::AlreadyActive => "You have that one already.".to_string(),
        ContractRefusal::AlreadyDone => "That one is finished.".to_string(),
        ContractRefusal::NotOffered => "Nobody here is offering that.".to_string(),
        ContractRefusal::NotAtBroker => {
            "You can read the board from here, but not sign it. Go back to your base.".to_string()
        }
        ContractRefusal::NothingToDeliver => {
            "You are not carrying anything it asks for.".to_string()
        }
    }
}
