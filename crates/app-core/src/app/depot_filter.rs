//! The Depot filter screen: which items one shelf will take in.
//!
//! A page of the structure hub (`app/structure_hub.rs`) on a Depot. Every edit
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
}

impl App {
    /// Opens the filter on one Depot, the hub's Filter page.
    pub(crate) fn open_depot_filter_for(&mut self, depot: Entity) -> bool {
        let shown = self.show_depot_filter(depot);
        if shown {
            self.menu_selected = 0;
            self.mode = Mode::DepotFilter;
        }
        shown
    }

    /// Re-reads one Depot into the screen, reporting whether it is still
    /// there. `false` leaves the screen empty; the hub's refresh closes it.
    fn show_depot_filter(&mut self, depot: Entity) -> bool {
        let Some(view) = self.game.as_ref().and_then(|g| g.depot_filter_view(depot)) else {
            self.depot_filter = None;
            return false;
        };
        self.depot_filter = Some(DepotFilterScreen { depot, view });
        true
    }

    pub(crate) fn handle_depot_filter_key(&mut self, key: GameKey) {
        let Some(screen) = &self.depot_filter else {
            return;
        };
        let (depot, rows) = (screen.depot, screen.view.rows.len());
        match key {
            GameKey::Up | GameKey::Down | GameKey::PageUp | GameKey::PageDown => {
                self.scroll(key, rows)
            }
            // `+` lets the row in and `-` keeps it out — the transfer
            // page's keys beside it, since the hub spends the bare arrows.
            GameKey::Char('+' | '=') => self.set_selected_depot_filter(depot, true),
            GameKey::Char('-') => self.set_selected_depot_filter(depot, false),
            // Uppercase: whole-list actions, set apart from the single-row
            // `+`/`-` above.
            GameKey::Char('A') => self.set_whole_depot_filter(depot, true),
            GameKey::Char('D') => self.set_whole_depot_filter(depot, false),
            _ => {}
        }
    }

    fn set_selected_depot_filter(&mut self, depot: Entity, allowed: bool) {
        let items = self
            .depot_filter
            .as_ref()
            .and_then(|s| s.view.rows.get(self.menu_selected))
            .map(|row| row.items.clone());
        let (Some(items), Some(game)) = (items, &mut self.game) else {
            return;
        };
        game.set_depot_filter(depot, &items, allowed);
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
        let Some(depot) = self.depot_filter.as_ref().map(|s| s.depot) else {
            return;
        };
        self.show_depot_filter(depot);
    }
}
