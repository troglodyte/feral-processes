//! The three-page breeding flow, opened from a Breeding Bay's structure
//! sheet: first parent, second parent, then the confirm that spends a seed.

use crate::*;

impl App {
    /// Picks the first parent from the whole roster, as fusion does: nothing
    /// in `Game::breed` asks the parents to be near the bay. A program still
    /// resting is a row like any other and refuses with the engine's reason,
    /// so the picker and `Game::breed` cannot disagree about who is ready.
    pub(crate) fn handle_breed_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.leave_breeding();
            return;
        }
        if let Some(parent) = self.pick_parent(key, None) {
            self.pending_breed_first = Some(parent);
            self.mode = Mode::BreedSecond;
        }
    }

    /// Picks the second parent. Esc steps back to re-pick the first.
    pub(crate) fn handle_breed_second_key(&mut self, key: GameKey) {
        let Some(first) = self.pending_breed_first else {
            self.leave_breeding();
            return;
        };
        if key == GameKey::Esc {
            self.pending_breed_first = None;
            self.status_line = None;
            self.mode = Mode::Breed;
            return;
        }
        if let Some(parent) = self.pick_parent(key, Some(first)) {
            self.pending_breed_second = Some(parent);
            self.mode = Mode::BreedConfirm;
        }
    }

    /// Enter runs the breeding. Esc steps back to re-pick the second parent.
    pub(crate) fn handle_breed_confirm_key(&mut self, key: GameKey) {
        let (Some(bay), Some(first), Some(second)) = (
            self.pending_breed_bay,
            self.pending_breed_first,
            self.pending_breed_second,
        ) else {
            self.leave_breeding();
            return;
        };
        match key {
            GameKey::Esc => {
                self.pending_breed_second = None;
                self.status_line = None;
                self.mode = Mode::BreedSecond;
            }
            GameKey::Enter => {
                let Some(game) = &mut self.game else { return };
                match game.breed(first, second, bay) {
                    Ok(_) => {
                        self.status_line = None;
                        self.leave_breeding();
                    }
                    Err(refusal) => self.refuse(refusal.reason()),
                }
            }
            _ => {}
        }
    }

    /// The roster row `key` picks, minus `exclude`, or `None` — after
    /// saying why when that program cannot be a parent right now.
    fn pick_parent(&mut self, key: GameKey, exclude: Option<Entity>) -> Option<Entity> {
        let game = self.game.as_mut()?;
        let candidates: Vec<Entity> = game
            .owned_pets()
            .into_iter()
            .map(|p| p.entity)
            .filter(|e| Some(*e) != exclude)
            .collect();
        let idx = self.selected_index(key, candidates.len())?;
        let parent = candidates[idx];
        match self.game.as_ref()?.breed_refusal(parent) {
            Some(refusal) => {
                self.refuse(refusal.reason());
                None
            }
            None => {
                self.status_line = None;
                Some(parent)
            }
        }
    }

    fn leave_breeding(&mut self) {
        self.pending_breed_bay = None;
        self.pending_breed_first = None;
        self.pending_breed_second = None;
        self.close_screen();
    }
}
