//! The inventory (`i`) as a hub: what you carry, then who you carry it with.

use crate::*;

/// The inventory's table, in rail order: what you carry, then who you carry
/// it with. The roster and Perks are pages in their own right; the two
/// command pages hold the errands that act on programs and on routines.
pub const INVENTORY_PAGES: &[FixedPage] = &[
    FixedPage {
        title: "Items",
        screens: &[Mode::Inventory],
    },
    FixedPage {
        title: "Phase keys",
        screens: &[Mode::PhaseKeys],
    },
    FixedPage {
        title: "Downed programs",
        screens: &[Mode::DownedPrograms],
    },
    FixedPage {
        title: "Tools",
        screens: &[Mode::Tools],
    },
    FixedPage {
        title: "Companions",
        screens: &[Mode::Companion],
    },
    FixedPage {
        title: "Manifests",
        screens: &[Mode::ManifestIntro],
    },
    FixedPage {
        title: "Programs",
        screens: &[Mode::PartyPrograms],
    },
    FixedPage {
        title: "Routines",
        screens: &[Mode::PartyRoutines],
    },
    FixedPage {
        title: "Perks",
        screens: &[Mode::Perks],
    },
];

pub(crate) struct InventoryHub;

impl HubKind for InventoryHub {
    fn title(&self) -> &'static str {
        "INVENTORY"
    }

    fn pages(&self, _app: &App) -> Vec<HubPage> {
        INVENTORY_PAGES.iter().map(FixedPage::page).collect()
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
