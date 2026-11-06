//! A hub: a rail of pages down the left, the page on the right, one cursor
//! and one nav rule for all of them. What a particular hub is — its title,
//! its pages, how a page opens and how the hub is left — is its `HubKind`,
//! one impl in that hub's own module. Nothing here or in gui names a hub.
//!
//! The rail is data the hub builds (`HubKind::pages`), and the page being
//! shown is *held* in `App` by key and checked against the `Mode`
//! (ADR 0043): `hub_at` answers only while the mode is one of the held
//! page's screens. A jump that knows its page names it
//! (`App::enter_hub_at`); one that does not falls back to `App::settle_hub`.
//!
//! A new hub is a `Hub` variant, its arm in `Hub::kind`, and a `HubKind`
//! impl with `pages`.

use crate::*;
use feral_processes_engine::RouteDestinationId;

/// A page's identity: stable across the rail growing or shrinking, which an
/// index is not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HubPageKey {
    /// A fixed page: its title.
    Named(&'static str),
    /// A site on the Sites hub's rail.
    Site(RouteDestinationId),
}

/// One page of a hub: a rail entry, and the screens `Right`/`Left` step
/// through on it.
#[derive(Clone, Debug, PartialEq)]
pub struct HubPage {
    pub key: HubPageKey,
    pub title: String,
    pub glyph: Option<char>,
    /// A header drawn above this page on the rail.
    pub section: Option<&'static str>,
    pub screens: &'static [Mode],
    /// The rail's `!`.
    pub marked: bool,
}

/// A fixed hub's table row: what `HubPage` was. Keeps the tables `const`.
pub struct FixedPage {
    pub title: &'static str,
    pub screens: &'static [Mode],
}

impl FixedPage {
    pub fn page(&self) -> HubPage {
        HubPage {
            key: HubPageKey::Named(self.title),
            title: self.title.to_string(),
            glyph: None,
            section: None,
            screens: self.screens,
            marked: false,
        }
    }
}

/// What a hub is. The defaults are a hub reopened where the player left it,
/// left for wherever it was opened from.
pub trait HubKind: Sync {
    /// The rail heading.
    fn title(&self) -> &'static str;

    /// The rail heading as `(head, tail)`: the tail is kept whole when the
    /// head is elided to fit.
    fn heading(&self, _app: &App) -> (String, String) {
        (self.title().to_string(), String::new())
    }

    /// The rail as it stands now, and the only source of it. `[` and `]`
    /// walk it in this order.
    fn pages(&self, app: &App) -> Vec<HubPage>;

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
    GameMenu,
}

impl Hub {
    pub const ALL: [Hub; 4] = [Hub::Base, Hub::Inventory, Hub::Structure, Hub::GameMenu];

    pub fn kind(self) -> &'static dyn HubKind {
        match self {
            Hub::Base => &super::base_report::BaseReport,
            Hub::Inventory => &super::inventory_hub::InventoryHub,
            Hub::Structure => &super::structure_hub::StructureHubKind,
            Hub::GameMenu => &super::game_menu::GameMenu,
        }
    }

    /// Index into `App::hub_last`.
    pub(crate) fn slot(self) -> usize {
        self as usize
    }
}

/// Which hub is open and the key of the page it shows. Checked against the
/// `Mode` on every read (`App::hub_for`), never trusted blind.
#[derive(Clone, Debug)]
pub(crate) struct HubPlace {
    pub hub: Hub,
    pub key: HubPageKey,
}

impl App {
    /// Which hub the current mode belongs to and where it sits there.
    pub fn hub_at(&self) -> Option<(Hub, usize, usize)> {
        self.hub_for(self.mode)
    }

    /// `hub_at` for another mode, for the popups drawn over a hub screen
    /// (`base_popup_parent`). Answers from the held page alone: its page,
    /// found by key on the rail as it stands now, must offer `mode`.
    pub fn hub_for(&self, mode: Mode) -> Option<(Hub, usize, usize)> {
        let place = self.hub_place.as_ref()?;
        let pages = place.hub.kind().pages(self);
        let page = pages.iter().position(|p| p.key == place.key)?;
        let screen = pages[page].screens.iter().position(|&s| s == mode)?;
        Some((place.hub, page, screen))
    }

    /// The held page's key while `hub_at` is `Some`.
    pub fn hub_page(&self) -> Option<&HubPageKey> {
        self.hub_at()?;
        self.hub_place.as_ref().map(|p| &p.key)
    }

    /// Opens `hub` where the player left it, or on its first page the first
    /// time, with the cursor on the rail. A remembered page that has since
    /// left the rail (the base report's Breed, its bay gone), or that no
    /// longer offers the remembered screen, is forgotten rather than drawn
    /// outside the hub.
    pub(crate) fn open_hub(&mut self, hub: Hub) {
        let kind = hub.kind();
        let pages = kind.pages(self);
        let Some(first) = pages.first() else { return };
        let last = self.hub_last[hub.slot()]
            .clone()
            .filter(|_| kind.remembers())
            .filter(|(key, mode)| {
                pages
                    .iter()
                    .any(|p| &p.key == key && p.screens.contains(mode))
            });
        match last {
            Some((key, mode)) => {
                self.hub_place = Some(HubPlace { hub, key });
                self.mode = mode;
            }
            None => {
                self.hub_place = Some(HubPlace {
                    hub,
                    key: first.key.clone(),
                });
                kind.turn(self, 0);
            }
        }
        self.hub_on_rail = true;
    }

    /// Holds `key` in `hub` and shows its first screen through `kind.turn`.
    pub(crate) fn turn_hub_to(&mut self, hub: Hub, key: &HubPageKey) {
        if let Some(page) = hub.kind().pages(self).iter().position(|p| &p.key == key) {
            self.turn_hub_to_index(hub, page);
        }
    }

    fn turn_hub_to_index(&mut self, hub: Hub, page: usize) {
        let kind = hub.kind();
        let Some(key) = kind.pages(self).get(page).map(|p| p.key.clone()) else {
            return;
        };
        // Held before the turn: a turn that closes the hub is then cleared
        // by `settle_hub` rather than left naming a page it never showed.
        self.hub_place = Some(HubPlace { hub, key });
        kind.turn(self, page);
    }

    /// The explicit door (ADR 0043): holds `key` in `hub` and shows `mode`,
    /// which must be one of that page's screens. In release a mismatch
    /// leaves `hub_at` `None` rather than panicking. Any jump onto a screen
    /// more than one page offers must use this or a turn.
    pub(crate) fn enter_hub_at(&mut self, hub: Hub, key: HubPageKey, mode: Mode) {
        debug_assert!(
            hub.kind()
                .pages(self)
                .iter()
                .any(|p| p.key == key && p.screens.contains(&mode)),
            "{key:?} does not offer {mode:?}"
        );
        self.hub_place = Some(HubPlace { hub, key });
        self.mode = mode;
    }

    /// The default for a jump that names no page: keeps the held page while
    /// it offers the screen shown, else moves to another page of the same
    /// hub that does, else adopts the one page across every hub that offers
    /// it. A screen offered by several pages is never adopted by guess, and
    /// a screen no page offers (a launched screen) leaves the place alone,
    /// where `hub_at` answers `None` until the mode comes back.
    pub fn settle_hub(&mut self) {
        if matches!(self.mode, Mode::Playing | Mode::MainMenu) {
            self.hub_place = None;
            return;
        }
        let shown = base_popup_parent(self.mode).unwrap_or(self.mode);
        if let Some(place) = &self.hub_place {
            let pages = place.hub.kind().pages(self);
            if pages
                .iter()
                .any(|p| p.key == place.key && p.screens.contains(&shown))
            {
                return;
            }
            if let Some(page) = pages.iter().find(|p| p.screens.contains(&shown)) {
                let key = page.key.clone();
                if let Some(place) = &mut self.hub_place {
                    place.key = key;
                }
                return;
            }
        }
        let mut offered = Vec::new();
        for hub in Hub::ALL {
            for page in hub.kind().pages(self) {
                if page.screens.contains(&shown) {
                    offered.push(HubPlace { hub, key: page.key });
                }
            }
        }
        if let [only] = offered.as_slice() {
            self.hub_place = Some(only.clone());
        }
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
            GameKey::Char(']') => self.turn_hub_to_index(hub, next),
            GameKey::Char('[') => self.turn_hub_to_index(hub, prev),
            _ if self.hub_on_rail => match key {
                GameKey::Down => self.turn_hub_to_index(hub, next),
                GameKey::Up => self.turn_hub_to_index(hub, prev),
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
            // A typed filter takes Esc first; the next Esc is the hub's.
            GameKey::Esc if self.filter_applied() => true,
            // A question the page is asking is answered by it, not
            // abandoned.
            GameKey::Esc => {
                self.splice_rig
                    .as_ref()
                    .is_some_and(|s| s.confirm.is_some())
                    || self.quit_unsaved_pending
            }
            // A level row spends Left and Right on the level; a toggle's
            // Left still goes to the rail.
            GameKey::Left if self.quit_unsaved_pending => true,
            GameKey::Left | GameKey::Right if self.on_level_row() => true,
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
