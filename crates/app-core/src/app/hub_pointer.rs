//! What a pointer hit means on a hub, every one of them the keys it stands
//! for (ADR 0046): a rail click is the page turn, a tab click is `Tab`, a
//! row click is the arrows to that row and a second click on it is `Enter`,
//! a scroll marker is a page key and the wheel is `↑`/`↓` on the hovered
//! half. Nothing here knows which hub it is on.

use crate::*;

/// The most steps a row click walks, a bound on a list that does not move
/// its cursor with the arrows.
const MAX_ROW_WALK: usize = 512;

impl App {
    pub(crate) fn hub_pointer_press(&mut self, hit: PointerHit, double: bool) {
        match hit {
            PointerHit::Page(key) => self.pointer_page(key),
            PointerHit::Tab(to) => self.pointer_tab(to),
            PointerHit::Row(row) if self.hub_at().is_some() => self.pointer_row(row, double),
            PointerHit::More(dir) if self.hub_at().is_some() => {
                self.focus_hub_page();
                self.handle_key(match dir {
                    Direction::Up => GameKey::PageUp,
                    Direction::Down => GameKey::PageDown,
                });
            }
            PointerHit::Region(Region::Rail) if self.hub_at().is_some() => {
                self.pointer_focus_rail();
            }
            PointerHit::Region(Region::Page) if self.hub_at().is_some() => self.focus_hub_page(),
            _ => {}
        }
    }

    /// A rail click is what the keys do: `Esc` to the rail (unless the page
    /// holds `Esc`), then `↑`/`↓` along it, the shorter way round. On the
    /// held page it is the first alone.
    fn pointer_page(&mut self, key: HubPageKey) {
        let Some((hub, page, _)) = self.hub_at() else {
            return;
        };
        if !self.pointer_focus_rail() {
            return;
        }
        let pages = hub.kind().pages(self);
        let Some(to) = pages.iter().position(|p| p.key == key) else {
            return;
        };
        let forward = (to + pages.len() - page) % pages.len();
        if forward <= pages.len() - forward {
            (0..forward).for_each(|_| self.handle_key(GameKey::Down));
        } else {
            (0..pages.len() - forward).for_each(|_| self.handle_key(GameKey::Up));
        }
    }

    /// A tab click is `Tab` pressed until the tab is shown.
    fn pointer_tab(&mut self, to: usize) {
        let Some((hub, page, tab)) = self.hub_at() else {
            return;
        };
        let tabs = hub.kind().pages(self)[page].tabs.len();
        if to < tabs {
            (0..(to + tabs - tab) % tabs).for_each(|_| self.handle_key(GameKey::Tab));
        }
    }

    /// Puts the cursor on the rail as `Esc` does, and answers whether it is
    /// there. A page holding `Esc` for a question of its own
    /// (`screen_claims`) keeps the cursor: the pointer escapes nothing a key
    /// cannot.
    fn pointer_focus_rail(&mut self) -> bool {
        if !self.hub_on_rail && !self.screen_claims(GameKey::Esc) {
            self.hub_on_rail = true;
        }
        self.hub_on_rail
    }

    /// The wheel is `↑`/`↓` on whichever half of the hub is under it.
    pub(crate) fn hub_pointer_wheel(&mut self, over: PointerHit, notches: i32) {
        if self.hub_at().is_none() {
            return;
        }
        match over {
            PointerHit::Page(_) | PointerHit::Region(Region::Rail) => {
                if !self.pointer_focus_rail() {
                    return;
                }
            }
            PointerHit::Row(_) | PointerHit::More(_) | PointerHit::Region(Region::Page) => {
                self.focus_hub_page()
            }
            _ => return,
        }
        let key = if notches > 0 {
            GameKey::Up
        } else {
            GameKey::Down
        };
        for _ in 0..notches.unsigned_abs() {
            self.handle_key(key);
        }
    }

    /// Moves the cursor from the rail into the page, as `→` does.
    fn focus_hub_page(&mut self) {
        if self.hub_on_rail {
            self.handle_key(GameKey::Right);
        }
    }

    /// Walks the arrows to `row`; a click on the row the cursor is already
    /// on, or a double-click, then presses `Enter`. A click that only
    /// brought the cursor in from the rail is not a second click.
    fn pointer_row(&mut self, row: usize, double: bool) {
        let second = !self.hub_on_rail && self.menu_selected == row;
        self.focus_hub_page();
        let (key, steps) = if row >= self.menu_selected {
            (GameKey::Down, row - self.menu_selected)
        } else {
            (GameKey::Up, self.menu_selected - row)
        };
        for _ in 0..steps.min(MAX_ROW_WALK) {
            self.handle_key(key);
        }
        if second || double {
            self.handle_key(GameKey::Enter);
        }
    }
}
