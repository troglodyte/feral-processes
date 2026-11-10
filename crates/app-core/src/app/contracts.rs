//! The Contracts page (`b` → JOBS): Held, every contract the run is
//! holding, and Board, what the Broker in front of you is offering.

use crate::*;

/// Which section of a town's board a picked row number lands in, and its
/// index within that section.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum ContractScreenRow {
    Active(usize),
    Offer(usize),
}

/// Resolves a row number against a board's two stacked sections — held
/// contracts, then offers — given how many rows each contributed. `None`
/// for a row past the end. Shared with the renderer that numbers the same
/// rows: the offset arithmetic is where a screen with more than one section
/// goes wrong.
pub(crate) fn contract_row(idx: usize, active: usize, offers: usize) -> Option<ContractScreenRow> {
    if idx < active {
        return Some(ContractScreenRow::Active(idx));
    }
    let idx = idx - active;
    (idx < offers).then_some(ContractScreenRow::Offer(idx))
}

/// Why an onboarding mission cannot be given back, on the Contracts page's
/// Held tab and a town's Jobs tab alike.
pub(crate) const TUTORIAL_REFUSAL: &str =
    "Onboarding missions cannot be given back — finish this one and the next arrives.";

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

    /// Both tabs. Held: Enter hands a delivery over, `[A]` gives the
    /// highlighted contract back. Board: Enter takes the highlighted offer.
    /// Each tab's rows are its own list from `contract_sections`, so a row
    /// number is an index into that list alone.
    /// `[A]` on a held contract, from the Held tab, a town's Jobs tab or its
    /// board: gives it back. The engine refuses a tutorial too — that is
    /// where the invariant lives — but a bare `false` cannot reach the log,
    /// and a key that silently does nothing reads as a broken key.
    pub(crate) fn give_back(&mut self, contract: &ContractRow) {
        if contract.tutorial {
            self.refuse(TUTORIAL_REFUSAL);
            return;
        }
        if let Some(game) = &mut self.game {
            game.abandon_contract(&contract.id);
        }
        self.status_line = None;
    }

    pub(crate) fn handle_contracts_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let (active, offers) = self.contract_sections();
        let held = self.mode == Mode::ContractsHeld;

        // Uppercase is reserved for screen actions, as `[S]`/`[B]` are on the
        // trade screen — a shifted letter that also picked a row would fire
        // both on one keypress, and this one gives a contract up.
        if held && key == GameKey::Char('A') {
            match active.get(self.menu_selected) {
                Some(contract) => self.give_back(contract),
                None => self.refuse("Highlight a contract you are holding to give it back."),
            }
            return;
        }

        let rows = if held { active.len() } else { offers.len() };
        let Some(row) = self.selected_index(key, rows) else {
            return;
        };
        let Some(game) = &mut self.game else { return };
        let outcome = if held {
            // Picking one you hold hands over a delivery. Nothing else on
            // this tab is an action a held contract needs: the other
            // objectives advance on their own, wherever you are.
            game.deliver_to_contract(&active[row].id, None)
                .map(|_| ())
                .map_err(|why| match why {
                    ContractRefusal::NotOffered => {
                        "That one finishes itself — just go and do it.".to_string()
                    }
                    why => refusal_line(why),
                })
        } else {
            game.accept_contract(&offers[row].id, None)
                .map_err(refusal_line)
        };
        self.report(outcome);
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
