//! Type-to-filter on item lists: the matcher, the filter stack on `App`, and
//! the key rule every filtering screen shares.
//!
//! A screen's handler and the gui both ask the same `*_visible` function which
//! source rows the filter leaves, so the highlight and the row an action
//! applies to cannot disagree. Later screens extend `is_filter_home` and
//! `filter_lives`; nothing else here is per-screen.

use crate::app::dev_item_picker::dev_item_visible;
use crate::app::inventory::{INVENTORY_SLOT_ROWS, inventory_visible};
use crate::{App, GameKey, Mode};

/// One screen's filter. `row` is the highlight to restore when the player
/// comes back from a sub-screen opened on a filtered row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListFilter {
    pub(crate) home: Mode,
    pub(crate) text: String,
    pub(crate) row: usize,
}

/// Source indices of the names containing `filter`, ignoring case. An empty
/// filter keeps every row.
pub fn filter_matches<'a>(names: impl IntoIterator<Item = &'a str>, filter: &str) -> Vec<usize> {
    let needle = filter.to_lowercase();
    names
        .into_iter()
        .enumerate()
        .filter(|(_, name)| name.to_lowercase().contains(&needle))
        .map(|(i, _)| i)
        .collect()
}

/// The screens that take typed filter text.
fn is_filter_home(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::DevItemPicker | Mode::Inventory | Mode::EquipSwap
    )
}

/// Whether a filter homed on `home` is still wanted once the mode is `now`:
/// on the home itself, or on a sub-screen the player returns from. Each phase
/// adds its screens' round trips here. Anything else forgets the filter.
pub(crate) fn filter_lives(home: Mode, now: Mode) -> bool {
    home == now
        || matches!(
            (home, now),
            // Every screen the pack opens on a row and returns from. `Trade`
            // is the picker `[S]` shows when several posts are in range, and
            // `TradeAction` the list a sale can land on.
            (
                Mode::Inventory,
                Mode::InventoryItemAction
                    | Mode::EraseQuantity
                    | Mode::TradeQuantity
                    | Mode::ModCopy
                    | Mode::RefitCopy
                    | Mode::TrapDirection
                    | Mode::ItemDescribe
                    | Mode::EquipSwap
                    | Mode::Trade
                    | Mode::TradeAction
                    | Mode::TradeProgramConfirm
            ) | (Mode::EquipSwap, Mode::RefitCopy | Mode::ItemDescribe)
        )
}

impl App {
    /// The filter typed on the current screen, or `""`.
    pub fn filter_text(&self) -> &str {
        self.list_filters
            .iter()
            .rev()
            .find(|f| f.home == self.mode)
            .map_or("", |f| f.text.as_str())
    }

    pub fn filter_applied(&self) -> bool {
        !self.filter_text().is_empty()
    }

    /// Feeds a filtering screen's key to its filter. True when consumed:
    /// lowercase letters, digits and space type, Backspace deletes, and Esc
    /// clears a filter that is applied (with none, Esc is the screen's own).
    /// Capitals stay screen actions.
    pub(crate) fn take_filter_key(&mut self, key: GameKey) -> bool {
        if !is_filter_home(self.mode) {
            return false;
        }
        match key {
            GameKey::Char(c) if c.is_ascii_lowercase() || c.is_ascii_digit() || c == ' ' => {
                self.edit_filter(|text| text.push(c));
            }
            GameKey::Backspace => self.edit_filter(|text| {
                text.pop();
            }),
            GameKey::Esc if self.filter_applied() => self.edit_filter(String::clear),
            _ => return false,
        }
        true
    }

    fn edit_filter(&mut self, edit: impl FnOnce(&mut String)) {
        let at = self.list_filters.iter().rposition(|f| f.home == self.mode);
        let at = at.unwrap_or_else(|| {
            self.list_filters.push(ListFilter {
                home: self.mode,
                text: String::new(),
                row: 0,
            });
            self.list_filters.len() - 1
        });
        edit(&mut self.list_filters[at].text);
        if self.list_filters[at].text.is_empty() {
            self.list_filters.remove(at);
        }
        self.menu_selected = self.first_match_row();
    }

    /// Where the highlight lands when the filter changes.
    fn first_match_row(&self) -> usize {
        match (self.mode, &self.game) {
            // The three slot rows are pinned above the items.
            (Mode::Inventory, Some(game)) => {
                if inventory_visible(game, self.filter_text()).is_empty() {
                    0
                } else {
                    INVENTORY_SLOT_ROWS
                }
            }
            _ => 0,
        }
    }

    /// How many rows the filtered list of the current home has, so a
    /// restored highlight can't land past its end.
    fn filtered_len(&self) -> Option<usize> {
        let game = self.game.as_ref()?;
        match self.mode {
            Mode::Inventory => {
                Some(INVENTORY_SLOT_ROWS + inventory_visible(game, self.filter_text()).len())
            }
            Mode::DevItemPicker => Some(dev_item_visible(game, self.filter_text()).len()),
            _ => None,
        }
    }

    /// Mode-change bookkeeping for `handle_key`: park the highlight of a home
    /// that was left, drop filters nobody returns to, and put a returning
    /// home's highlight back.
    pub(crate) fn retain_filters(&mut self, before: Mode, row_before: usize) {
        if let Some(f) = self
            .list_filters
            .iter_mut()
            .rev()
            .find(|f| f.home == before)
        {
            f.row = row_before;
        }
        let now = self.mode;
        self.list_filters.retain(|f| filter_lives(f.home, now));
        if let Some(f) = self.list_filters.iter().rev().find(|f| f.home == now) {
            let row = f.row;
            self.menu_selected = match self.filtered_len() {
                Some(len) => row.min(len.saturating_sub(1)),
                None => row,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substring_ignores_case() {
        assert_eq!(
            filter_matches(["foo bar", "Armor", "zip"], "ar"),
            vec![0, 1]
        );
    }

    #[test]
    fn empty_filter_keeps_every_row() {
        assert_eq!(filter_matches(["a", "b", "c"], ""), vec![0, 1, 2]);
    }

    #[test]
    fn no_match_is_empty() {
        assert!(filter_matches(["a", "b"], "zz").is_empty());
    }

    #[test]
    fn digits_and_spaces_match() {
        assert_eq!(
            filter_matches(["mk2 plate", "mk3", "plate"], "2 p"),
            vec![0]
        );
    }
}
