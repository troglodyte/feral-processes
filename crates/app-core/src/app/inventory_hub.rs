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
    HubTab::new("Roster", Mode::Companion),
    HubTab::new("Errands", Mode::PartyPrograms),
];

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

    fn pages(&self, _app: &App) -> Vec<HubPage> {
        let [you, roster, downed] = INVENTORY_PAGES else {
            unreachable!("the fixed table is three entries")
        };
        vec![
            HubPage {
                section: Some(YOU_SECTION),
                ..you.page()
            },
            HubPage {
                section: Some(PROGRAMS_SECTION),
                ..roster.page()
            },
            downed.page(),
        ]
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

    fn extra_keys(&self, app: &App) -> Vec<(&'static str, &'static str)> {
        if app.hub_on_rail && app.mode == Mode::ManifestIntro {
            vec![("Enter", "open")]
        } else {
            Vec::new()
        }
    }
}
