//! The inventory (`i`) as a hub: what you carry, then who you carry it with.

use crate::*;

/// The YOU entry's tabs: what the player carries and knows, then the
/// errand page for routines and the Manifests entry door.
pub const YOU_TABS: &[HubTab] = &[
    HubTab::new("Items", Mode::Inventory),
    HubTab::new("Tools", Mode::Tools),
    HubTab::new("Phase keys", Mode::PhaseKeys),
    HubTab::new("Routines", Mode::PartyRoutines),
    HubTab::new("Perks", Mode::Perks),
    HubTab::new("Manifests", Mode::ManifestIntro),
];

/// The Roster entry's tabs: every program you own, then the errands that
/// act on programs.
pub const ROSTER_TABS: &[HubTab] = &[
    HubTab::new("Party", Mode::Companion),
    HubTab::new("Errands", Mode::PartyPrograms),
];

/// A party member's tabs. One: the equipment, memories and manifest are
/// popups over the hub (`draw_popup`, and the swap picker's chain returns to
/// whoever opened it), so they stay keys on this tab.
pub const MEMBER_TABS: &[HubTab] = &[HubTab::new("Overview", Mode::PartyMember)];

pub const PARTY_SECTION: &str = "PARTY";
pub const BESIDE_SECTION: &str = "BESIDE YOU";
pub const YOU_SECTION: &str = "YOU";
pub const PROGRAMS_SECTION: &str = "PROGRAMS";

/// The inventory's fixed entries, in rail order within their sections. The
/// PARTY entries (one per member) come between the first two, and BESIDE
/// YOU's (one per adjacent structure) after the last: both are the live
/// game's answer, not a table.
pub const INVENTORY_PAGES: &[FixedPage] = &[
    FixedPage {
        title: "You",
        tabs: YOU_TABS,
    },
    FixedPage {
        title: "Roster",
        tabs: ROSTER_TABS,
    },
    FixedPage {
        title: "Downed",
        tabs: &[HubTab::new("Downed", Mode::DownedPrograms)],
    },
];

pub(crate) struct InventoryHub;

impl HubKind for InventoryHub {
    fn title(&self) -> &'static str {
        "INVENTORY"
    }

    fn pages(&self, app: &App) -> Vec<HubPage> {
        let [you, roster, downed] = INVENTORY_PAGES else {
            unreachable!("the fixed table is three entries")
        };
        let mut pages = vec![HubPage {
            section: Some(YOU_SECTION),
            ..you.page()
        }];
        let members = app
            .game
            .as_ref()
            .map(|g| g.party_info())
            .unwrap_or_default();
        for (i, member) in members.into_iter().enumerate() {
            pages.push(HubPage {
                key: HubPageKey::Member(member.entity),
                title: member.name,
                glyph: None,
                section: (i == 0).then_some(PARTY_SECTION),
                tabs: MEMBER_TABS.to_vec(),
                marked: false,
                name_cut_at_end: true,
            });
        }
        pages.push(HubPage {
            section: Some(PROGRAMS_SECTION),
            ..roster.page()
        });
        pages.push(downed.page());
        pages.extend(super::structure_hub::beside_pages(app, BESIDE_SECTION));
        pages
    }

    /// Entries that do not share a tab list keep the tab being shown when
    /// the new entry has it, so turning along BESIDE YOU stays on Info.
    fn entry_tab(&self, app: &App, page: &HubPage) -> usize {
        page.tabs
            .iter()
            .position(|t| t.mode == app.mode)
            .unwrap_or(0)
    }

    fn show_tab(&self, app: &mut App, page: &HubPage, tab: usize) {
        app.mode = page.tabs[tab].mode;
        // Whatever a structure's page held goes when the hold leaves it,
        // wherever the leaving came from (a turn, a tick, a demolition).
        if let HubPageKey::Structure(entity) = page.key {
            match app.beside_page(entity, tab) {
                Some(structure_page) => app.enter_structure_page(entity, structure_page),
                None => app.leave_beside(),
            }
            return;
        }
        app.drop_page_state();
        if let HubPageKey::Member(member) = page.key {
            app.select_member(member);
        }
    }

    /// Enter or Right on the Manifests rail entry opens the sheet itself,
    /// instead of stepping onto a page with nothing to select — the game
    /// menu's Manual page's rule.
    fn key(&self, app: &mut App, key: GameKey) -> bool {
        if !app.hub_on_rail
            || app.mode != Mode::ManifestIntro
            || !matches!(key, GameKey::Enter | GameKey::Right)
        {
            return false;
        }
        app.menu_origin = Some(Mode::ManifestIntro);
        app.open_menu_manifest();
        true
    }

    /// A structure's page leaves what it held behind it: its basket, its
    /// open recipe, its pending bay.
    fn leave(&self, app: &mut App) {
        if app.held_structure().is_some() {
            app.drop_page_state();
        }
        app.close_screen();
    }

    fn extra_keys(&self, app: &App) -> Vec<(&'static str, &'static str)> {
        if app.hub_on_rail && app.mode == Mode::ManifestIntro {
            vec![("Enter", "open")]
        } else if !app.hub_on_rail && app.mode == Mode::StructureManifest {
            vec![("W", "work it"), ("U", "upgrade"), ("X", "demolish")]
        } else {
            Vec::new()
        }
    }
}
