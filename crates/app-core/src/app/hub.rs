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
    /// A site on `b`'s rail.
    Site(RouteDestinationId),
}

/// One tab of a page: the name the title row draws and the screen it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HubTab {
    pub name: &'static str,
    pub mode: Mode,
}

impl HubTab {
    pub const fn new(name: &'static str, mode: Mode) -> Self {
        Self { name, mode }
    }
}

/// One page of a hub: a rail entry, and the tabs `Tab` cycles on it.
#[derive(Clone, Debug, PartialEq)]
pub struct HubPage {
    pub key: HubPageKey,
    pub title: String,
    pub glyph: Option<char>,
    /// A header drawn above this page on the rail.
    pub section: Option<&'static str>,
    pub tabs: &'static [HubTab],
    /// The rail's `!`.
    pub marked: bool,
    /// A name the rail may cut: the tail gives way ("Outpost north-e…"),
    /// where a fixed title is elided in the middle.
    pub name_cut_at_end: bool,
}

/// A fixed hub's table row: what `HubPage` was. Keeps the tables `const`.
pub struct FixedPage {
    pub title: &'static str,
    pub tabs: &'static [HubTab],
}

impl HubPage {
    /// The screens the page's tabs show, in tab order.
    pub fn modes(&self) -> impl Iterator<Item = Mode> + '_ {
        self.tabs.iter().map(|t| t.mode)
    }

    pub fn offers(&self, mode: Mode) -> bool {
        self.modes().any(|m| m == mode)
    }
}

impl FixedPage {
    pub fn page(&self) -> HubPage {
        HubPage {
            key: HubPageKey::Named(self.title),
            title: self.title.to_string(),
            glyph: None,
            section: None,
            tabs: self.tabs,
            marked: false,
            name_cut_at_end: false,
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
        app.mode = self.pages(app)[page].tabs[0].mode;
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

/// The first page of the section `step` sections along from the one
/// holding `page`, wrapping; `None` when the rail has fewer than two
/// sections, so the keys do nothing on a hub without them.
fn section_start(pages: &[HubPage], page: usize, step: isize) -> Option<usize> {
    let starts: Vec<usize> = (0..pages.len())
        .filter(|&i| pages[i].section.is_some())
        .collect();
    if starts.len() < 2 {
        return None;
    }
    let here = starts.iter().rposition(|&s| s <= page);
    let to = match (here, step > 0) {
        (Some(i), _) => (i as isize + step).rem_euclid(starts.len() as isize) as usize,
        (None, true) => 0,
        (None, false) => starts.len() - 1,
    };
    Some(starts[to])
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
        let screen = pages[page].tabs.iter().position(|t| t.mode == mode)?;
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
            .filter(|(key, mode)| pages.iter().any(|p| &p.key == key && p.offers(*mode)));
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
                .any(|p| p.key == key && p.offers(mode)),
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
            if pages.iter().any(|p| p.key == place.key && p.offers(shown)) {
                return;
            }
            if let Some(page) = pages.iter().find(|p| p.offers(shown)) {
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
                if page.offers(shown) {
                    offered.push(HubPlace { hub, key: page.key });
                }
            }
        }
        if let [only] = offered.as_slice() {
            self.hub_place = Some(only.clone());
        }
    }

    /// Runs ahead of every hub screen's own handler and answers whether it
    /// took the key. One rule for every hub (ADR 0045). The hub has one
    /// cursor, on the rail or in the page (`hub_on_rail`). On the rail,
    /// Up/Down walk the pages (wrapping), Right or Enter moves the cursor
    /// into the page, Esc leaves the hub (`HubKind::leave`), and every other
    /// key is swallowed: a row jump or a page action with no cursor on the
    /// page would act on a row the player cannot see is chosen. In the page,
    /// Left (off its leftmost position) and Esc go back to the rail, unless
    /// the page claims the key first (`screen_claims`); Right is the page's
    /// own. From either half, `Tab`/`Shift+Tab` switch the page's tabs,
    /// `[`/`]` turn pages and `{`/`}` jump sections, and a hub's own keys
    /// (`HubKind::key`) are asked before any of it.
    ///
    /// `Esc` on the rail leaves the hub rather than stepping back a page:
    /// the rail is a set of siblings, not a history.
    pub(crate) fn handle_hub_nav(&mut self, key: GameKey) -> bool {
        let Some((hub, page, tab)) = self.hub_at() else {
            return false;
        };
        let kind = hub.kind();
        if kind.key(self, key) {
            return true;
        }
        let pages = kind.pages(self);
        let count = pages.len();
        let (next, prev) = ((page + 1) % count, (page + count - 1) % count);
        match key {
            GameKey::Char(']') => self.turn_hub_to_index(hub, next),
            GameKey::Char('[') => self.turn_hub_to_index(hub, prev),
            GameKey::Char('}') if !self.screen_claims(key) => {
                if let Some(to) = section_start(&pages, page, 1) {
                    self.turn_hub_to_index(hub, to);
                }
            }
            GameKey::Char('{') if !self.screen_claims(key) => {
                if let Some(to) = section_start(&pages, page, -1) {
                    self.turn_hub_to_index(hub, to);
                }
            }
            GameKey::Tab => self.step_hub_tab(&pages[page], tab, 1),
            GameKey::ShiftTab => self.step_hub_tab(&pages[page], tab, -1),
            _ if self.hub_on_rail => match key {
                GameKey::Down => self.turn_hub_to_index(hub, next),
                GameKey::Up => self.turn_hub_to_index(hub, prev),
                GameKey::Right | GameKey::Enter => self.hub_on_rail = false,
                GameKey::Esc => kind.leave(self),
                _ => {}
            },
            _ if self.screen_claims(key) => return false,
            GameKey::Esc | GameKey::Left => self.hub_on_rail = true,
            _ => return false,
        }
        true
    }

    /// Shows the tab `step` along from `tab` on `page`, wrapping. A page
    /// with one tab has nowhere to go, and the key is still the hub's: a
    /// `Tab` that fell through would be a second meaning on that page.
    fn step_hub_tab(&mut self, page: &HubPage, tab: usize, step: isize) {
        let to = (tab as isize + step).rem_euclid(page.tabs.len() as isize) as usize;
        self.mode = page.tabs[to].mode;
    }

    /// Whether the page wants `key` before the hub's own rule does: a key
    /// the page spends on a cursor or a question of its own.
    pub(crate) fn screen_claims(&mut self, key: GameKey) -> bool {
        match key {
            // With a program picked Esc steps back to the list, which the
            // downed-program handler owns; Left would strand that pick on
            // the rail.
            GameKey::Esc | GameKey::Left if self.in_downed_drill_down() => true,
            // A typed filter takes Esc first; the next Esc is the hub's.
            GameKey::Esc if self.filter_applied() => true,
            // A question the page is asking is answered by it, not
            // abandoned; an open Deploy category steps back to the list of
            // categories first.
            GameKey::Esc => {
                self.splice_rig
                    .as_ref()
                    .is_some_and(|s| s.confirm.is_some())
                    || self.quit_unsaved_pending
                    || (self.mode == Mode::Build && self.build_category.is_some())
            }
            // A manifest shown as a hub tab walks its faces with the arrows
            // (`step_manifest_face`); off its first face Left is still its
            // own, and from the first it goes to the rail.
            GameKey::Left if self.mode == Mode::Manifest => self.manifest_tab != ManifestTab::Stats,
            // A page with a filter keeps the braces for it.
            GameKey::Char('{' | '}') => self.filter_applied(),
            // A level row spends Left on the level; a toggle's Left still
            // goes to the rail. (Right is the page's on every page, so it
            // needs no claim.)
            GameKey::Left if self.quit_unsaved_pending => true,
            // The research tree view walks its nodes with every arrow.
            GameKey::Left if self.research_graph_view && is_research_tree(self.mode) => true,
            GameKey::Left if self.on_level_row() => true,
            // The Trader tab's arrows are the basket's: Left lowers the
            // highlighted row, so the rail is Esc away.
            GameKey::Left if self.mode == Mode::Caravan => true,
            // The duty table spends Left on its column cursor, so it only
            // leaves off its first column.
            GameKey::Left => self.mode == Mode::BaseStaff && self.work_column > 0,
            _ => false,
        }
    }

    fn in_downed_drill_down(&self) -> bool {
        self.mode == Mode::DownedPrograms && self.pending_downed_program_index.is_some()
    }
}

/// The four research trees' screens, which share one handler and its tree
/// view.
fn is_research_tree(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::Research | Mode::RoutineResearch | Mode::AffixResearch | Mode::WeaponResearch
    )
}
