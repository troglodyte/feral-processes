//! The Depot filter screen: which items one shelf will take in.
//!
//! Opened with `[F]` from the transfer picker, which is the only screen
//! that already knows the player is standing beside a Depot. Every edit
//! writes straight through to the engine — there is no basket here,
//! because nothing on this screen is spent and a filter has no direction
//! to accumulate in.

use crate::*;

/// Which Depot the screen is editing and what it last read of it.
///
/// The view is refreshed after every edit rather than re-derived per
/// drawn frame: the rows are the item catalogue and cannot change under
/// the cursor, so a snapshot is safe, and rebuilding sixty item names
/// sixty times a second to answer a keypress that has not happened is
/// not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DepotFilterScreen {
    pub depot: Entity,
    pub view: DepotFilterView,
    /// How many Depots are beside the party, so the screen can say
    /// whether `Tab` will do anything.
    pub of: usize,
}

impl App {
    /// Opens the filter for the first adjacent Depot, or refuses.
    ///
    /// Refuses rather than silently doing nothing: `[F]` is offered in the
    /// transfer picker's hint whenever that screen is open, and the picker
    /// also opens beside a Mining Node, which has a shelf but no filter.
    pub(crate) fn open_depot_filter(&mut self) {
        let Some(game) = &self.game else {
            return;
        };
        let depots = game.adjacent_depot_entities();
        let Some(depot) = depots.first().copied() else {
            self.refuse("There is no Depot here to set up.");
            return;
        };
        if self.show_depot_filter(depot, depots.len()) {
            self.menu_selected = 0;
            self.mode = Mode::DepotFilter;
        }
    }

    /// Moves to the next adjacent Depot, wrapping. A no-op with one.
    ///
    /// The list is re-read rather than cached with the screen, so a Depot
    /// that has gone — demolished from the base menu, or lost to a sweep
    /// while this was open — is simply not in it.
    pub(crate) fn cycle_depot_filter(&mut self) {
        let Some((game, current)) = self
            .game
            .as_ref()
            .zip(self.depot_filter.as_ref().map(|s| s.depot))
        else {
            return;
        };
        let depots = game.adjacent_depot_entities();
        let next = depots
            .iter()
            .position(|e| *e == current)
            .map(|i| (i + 1) % depots.len().max(1))
            .and_then(|i| depots.get(i).copied());
        match next {
            Some(depot) => {
                if self.show_depot_filter(depot, depots.len()) {
                    self.menu_selected = 0;
                }
            }
            None => self.leave_depot_filter(),
        }
    }

    /// Re-reads one Depot into the screen, reporting whether it is still
    /// there. `false` closes the screen — the building has been
    /// demolished, or fallen to a GC Entropy Sweep, while it was open.
    fn show_depot_filter(&mut self, depot: Entity, of: usize) -> bool {
        let Some(view) = self.game.as_ref().and_then(|g| g.depot_filter_view(depot)) else {
            self.leave_depot_filter();
            return false;
        };
        self.depot_filter = Some(DepotFilterScreen { depot, view, of });
        true
    }

    /// Back to the transfer picker, on a **freshly taken** offer.
    ///
    /// Re-snapshotted rather than restored: a filter the player has just
    /// changed is exactly what `TransferRow::can_put` is derived from, so
    /// the basket they were holding was built against ceilings that no
    /// longer apply. Zeroing it is the honest answer and costs nothing
    /// but the amounts — no tick has passed and nothing has been spent.
    pub(crate) fn leave_depot_filter(&mut self) {
        self.depot_filter = None;
        let offer = self.game.as_ref().map(|g| {
            (
                g.transfer_offer(),
                g.rack_offer(),
                g.transfer_room(),
                g.total_rack_room(),
            )
        });
        match offer {
            Some((rows, carriers, room, rack_room)) => {
                self.open_transfer(rows, carriers, room, rack_room)
            }
            None => self.leave_basket(),
        }
    }

    pub(crate) fn handle_depot_filter_key(&mut self, key: GameKey) {
        let Some(screen) = &self.depot_filter else {
            self.mode = Mode::Playing;
            return;
        };
        let (depot, rows) = (screen.depot, screen.view.rows.len());
        match key {
            GameKey::Esc | GameKey::Enter => self.leave_depot_filter(),
            GameKey::Up | GameKey::Down => self.scroll(key, rows),
            GameKey::Tab => self.cycle_depot_filter(),
            // The table reads `item | denied | allowed`, so an arrow moves
            // the row toward the column it points at — the transfer
            // picker's own convention, which is the screen this one is
            // opened from.
            GameKey::Left => self.set_selected_depot_filter(depot, false),
            GameKey::Right => self.set_selected_depot_filter(depot, true),
            // Uppercase, because a lowercase letter picks a row everywhere
            // else in the game.
            GameKey::Char('A') => self.set_whole_depot_filter(depot, true),
            GameKey::Char('D') => self.set_whole_depot_filter(depot, false),
            _ => {}
        }
    }

    fn set_selected_depot_filter(&mut self, depot: Entity, allowed: bool) {
        let item = self
            .depot_filter
            .as_ref()
            .and_then(|s| s.view.rows.get(self.menu_selected))
            .map(|row| row.item.clone());
        let (Some(item), Some(game)) = (item, &mut self.game) else {
            return;
        };
        game.set_depot_filter(depot, &item, allowed);
        self.refresh_depot_filter();
    }

    fn set_whole_depot_filter(&mut self, depot: Entity, allowed: bool) {
        if let Some(game) = &mut self.game {
            game.set_all_depot_filters(depot, allowed);
        }
        self.refresh_depot_filter();
    }

    /// Re-reads the Depot after a write. The cursor is left where it is —
    /// the rows are the catalogue and a filter cannot reorder them.
    fn refresh_depot_filter(&mut self) {
        let Some((depot, of)) = self.depot_filter.as_ref().map(|s| (s.depot, s.of)) else {
            return;
        };
        self.show_depot_filter(depot, of);
    }
}
