//! The inventory (`i`) as a hub: what you carry, then who you carry it with.

use crate::*;

/// The inventory's table, in rail order: what you carry, then who you carry
/// it with. The roster and Perks are pages in their own right; the two
/// command pages hold the errands that act on programs and on routines.
pub const INVENTORY_PAGES: &[HubPage] = &[
    HubPage {
        title: "Items",
        screens: &[Mode::Inventory],
    },
    HubPage {
        title: "Phase keys",
        screens: &[Mode::PhaseKeys],
    },
    HubPage {
        title: "Downed programs",
        screens: &[Mode::DownedPrograms],
    },
    HubPage {
        title: "Tools",
        screens: &[Mode::Tools],
    },
    HubPage {
        title: "Companions",
        screens: &[Mode::Companion],
    },
    HubPage {
        title: "Programs",
        screens: &[Mode::PartyPrograms],
    },
    HubPage {
        title: "Routines",
        screens: &[Mode::PartyRoutines],
    },
    HubPage {
        title: "Perks",
        screens: &[Mode::Perks],
    },
];

pub(crate) struct InventoryHub;

impl HubKind for InventoryHub {
    fn title(&self) -> &'static str {
        "INVENTORY"
    }

    fn table(&self) -> &'static [HubPage] {
        INVENTORY_PAGES
    }
}
