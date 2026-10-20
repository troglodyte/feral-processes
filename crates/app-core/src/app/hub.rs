//! A hub: a rail of pages down the left, the page on the right, one cursor
//! and one nav rule for all of them. What a particular hub is — its title,
//! its pages, how a page opens and how the hub is left — is its `HubKind`,
//! one impl in that hub's own module. Nothing here or in gui names a hub.
//!
//! A new hub is a `Hub` variant, its arm in `Hub::kind`, and a `HubKind`
//! impl. A simple one writes `title` and `table` and takes every default.

use crate::*;

/// One page of a hub: a rail entry, and the screens `Right`/`Left` step
/// through on it.
pub struct HubPage {
    pub title: &'static str,
    pub screens: &'static [Mode],
}

/// What a hub is. The defaults are a fixed hub: a static table, reopened
/// where the player left it, left for wherever it was opened from.
pub trait HubKind: Sync {
    /// The rail heading.
    fn title(&self) -> &'static str;

    /// The rail heading as `(head, tail)`: the tail is kept whole when the
    /// head is elided to fit.
    fn heading(&self, _app: &App) -> (String, String) {
        (self.title().to_string(), String::new())
    }

    /// The fixed rail, in order, which `hub_of` and the reopen memory read.
    /// Empty for a hub whose pages depend on the `App`.
    fn table(&self) -> &'static [HubPage] {
        &[]
    }

    /// The rail as it stands now. `[` and `]` walk it in this order.
    fn pages(&self, _app: &App) -> Vec<&'static HubPage> {
        self.table().iter().collect()
    }

    /// Whether `App::open_hub` reopens the screen the player was last on.
    fn remembers(&self) -> bool {
        true
    }

    /// Shows page `page`, on its first screen.
    fn turn(&self, app: &mut App, page: usize) {
        app.mode = self.pages(app)[page].screens[0];
    }

    /// `Esc` on the rail.
    fn leave(&self, app: &mut App) {
        app.close_screen();
    }

    /// A key this hub binds beyond the shared rule, from either half.
    /// Answers whether it took it.
    fn key(&self, _app: &mut App, _key: GameKey) -> bool {
        false
    }

    /// Which pages carry the rail's `!`, by page index. Shorter than the
    /// rail (empty by default) means the rest are unmarked.
    fn marks(&self, _app: &App) -> Vec<bool> {
        Vec::new()
    }

    /// The keys `key` binds right now, as `(key, verb)` for the keybar.
    fn extra_keys(&self, _app: &App) -> Vec<(&'static str, &'static str)> {
        Vec::new()
    }
}

/// Every hub. The variant is the hub's identity — what `hub_at` names and
/// what the reopen memory is indexed by; its `HubKind` is everything else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hub {
    Base,
    Inventory,
    Structure,
}

impl Hub {
    pub const ALL: [Hub; 3] = [Hub::Base, Hub::Inventory, Hub::Structure];

    pub fn kind(self) -> &'static dyn HubKind {
        match self {
            Hub::Base => &super::base_report::BaseReport,
            Hub::Inventory => &super::inventory_hub::InventoryHub,
            Hub::Structure => &super::structure_hub::StructureHubKind,
        }
    }

    /// Index into `App::hub_last`.
    pub(crate) fn slot(self) -> usize {
        self as usize
    }
}

/// Which hub `mode` sits in by the fixed tables alone, as `(hub, page,
/// screen)`. A hub whose pages need the `App` is not here: `App::hub_for`
/// knows those.
pub fn hub_of(mode: Mode) -> Option<(Hub, usize, usize)> {
    Hub::ALL
        .into_iter()
        .find_map(|hub| find_in(hub, hub.kind().table().iter(), mode))
}

fn find_in<'a>(
    hub: Hub,
    pages: impl Iterator<Item = &'a HubPage>,
    mode: Mode,
) -> Option<(Hub, usize, usize)> {
    pages.enumerate().find_map(|(p, page)| {
        page.screens
            .iter()
            .position(|&s| s == mode)
            .map(|screen| (hub, p, screen))
    })
}

impl App {
    /// Which hub the current mode belongs to and where it sits there.
    pub fn hub_at(&self) -> Option<(Hub, usize, usize)> {
        self.hub_for(self.mode)
    }

    /// `hub_at` for another mode, for the popups drawn over a hub screen
    /// (`base_popup_parent`). A screen in a hub with no fixed table counts
    /// only while that hub's pages offer it, so the modes the structure hub
    /// borrows (`Transfer`, `BuildProgram`) are plain screens the rest of
    /// the time.
    pub fn hub_for(&self, mode: Mode) -> Option<(Hub, usize, usize)> {
        hub_of(mode).or_else(|| {
            Hub::ALL
                .into_iter()
                .filter(|hub| hub.kind().table().is_empty())
                .find_map(|hub| find_in(hub, hub.kind().pages(self).into_iter(), mode))
        })
    }

    /// Opens `hub` where the player left it, or on its first page the first
    /// time, with the cursor on the rail.
    pub(crate) fn open_hub(&mut self, hub: Hub) {
        let kind = hub.kind();
        let last = self.hub_last[hub.slot()].filter(|_| kind.remembers());
        match last {
            Some(mode) => self.mode = mode,
            None => kind.turn(self, 0),
        }
        self.hub_on_rail = true;
    }

    /// Runs ahead of every hub screen's own handler and answers whether it
    /// took the key. One rule for every hub. The hub has one cursor, on the
    /// rail or in the screen (`hub_on_rail`). On the rail, Up/Down walk the
    /// pages (wrapping), Right or Enter moves the cursor into the screen, Esc
    /// leaves the hub (`HubKind::leave`), and every other key is swallowed —
    /// a row jump or a screen action with no cursor on the screen would act
    /// on a row the player cannot see is chosen. In the screen, a page's
    /// screens lie in a row to the right of the rail: Right steps to the
    /// next (stopping at the last), Left to the previous and off the first
    /// back to the rail, and Esc goes straight to the rail — unless the
    /// screen claims the key first (`screen_claims`). Everything else falls
    /// through. `[`/`]` turn pages from either half, and a hub's own keys
    /// (`HubKind::key`) are asked before any of it.
    ///
    /// `Esc` on the rail leaves the hub rather than stepping back a page:
    /// the rail is a set of siblings, not a history.
    pub(crate) fn handle_hub_nav(&mut self, key: GameKey) -> bool {
        let Some((hub, page, screen)) = self.hub_at() else {
            return false;
        };
        let kind = hub.kind();
        if kind.key(self, key) {
            return true;
        }
        let pages = kind.pages(self);
        let count = pages.len();
        let screens = pages[page].screens;
        let (next, prev) = ((page + 1) % count, (page + count - 1) % count);
        match key {
            GameKey::Char(']') => kind.turn(self, next),
            GameKey::Char('[') => kind.turn(self, prev),
            _ if self.hub_on_rail => match key {
                GameKey::Down => kind.turn(self, next),
                GameKey::Up => kind.turn(self, prev),
                GameKey::Right | GameKey::Enter => self.hub_on_rail = false,
                GameKey::Esc => kind.leave(self),
                _ => {}
            },
            _ if self.screen_claims(key) => return false,
            GameKey::Esc => self.hub_on_rail = true,
            GameKey::Left => match screen.checked_sub(1) {
                Some(prev) => self.mode = screens[prev],
                None => self.hub_on_rail = true,
            },
            GameKey::Right if screen + 1 < screens.len() => self.mode = screens[screen + 1],
            _ => return false,
        }
        true
    }

    /// Whether the screen in the hub wants `key` before the hub's own rule
    /// does: an arrow or `Esc` the screen spends on a cursor or a question of
    /// its own.
    fn screen_claims(&mut self, key: GameKey) -> bool {
        match key {
            // With a program picked Esc steps back to the list, which the
            // downed-program handler owns; Left would strand that pick on
            // the rail.
            GameKey::Esc | GameKey::Left if self.in_downed_drill_down() => true,
            // A question the page is asking is answered by it, not
            // abandoned.
            GameKey::Esc => self
                .splice_rig
                .as_ref()
                .is_some_and(|s| s.confirm.is_some()),
            // The duty table spends Left and Right on its column cursor, so
            // it only leaves off its first and last columns.
            GameKey::Left => self.mode == Mode::BaseStaff && self.work_column > 0,
            GameKey::Right => self.on_inner_staff_column(),
            _ => false,
        }
    }

    fn in_downed_drill_down(&self) -> bool {
        self.mode == Mode::DownedPrograms && self.pending_downed_program_index.is_some()
    }

    /// Whether Right on the duty table still has a column to move to.
    fn on_inner_staff_column(&mut self) -> bool {
        self.mode == Mode::BaseStaff && self.work_column + 1 < self.base_staff_table().columns.len()
    }
}
