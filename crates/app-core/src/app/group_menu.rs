//! The two group menus — `b` for the base, `p` for the party — and the
//! rule that decides which of their rows are on screen right now.
//!
//! These exist because the map screen bound 27 keys. A row here is the
//! *only* way to reach the screen behind it: the old direct keys were
//! retired rather than kept as aliases, so the flat surface actually shrank
//! instead of growing a second system to document.

use crate::*;

/// One row of a group menu, as a renderer needs it. The target `Mode` is
/// deliberately not public — a frontend draws these, it doesn't dispatch
/// them.
pub struct GroupMenuRow {
    pub label: &'static str,
    pub(crate) target: Mode,
}

/// A row's static definition. `available` asks the same question the screen
/// behind the row would ask when it opens, so a row can't advertise a menu
/// that turns out to be empty.
struct GroupEntry {
    label: &'static str,
    target: Mode,
    /// Whether this row's action is one of `Game::require_surface`'s
    /// callers, and so must disappear while the party is underground.
    ///
    /// A flag in a readable table rather than an `is_underground()` check
    /// folded into each `available` closure, because it has to be kept in
    /// step with that list in the engine and a table is what makes that
    /// checkable. Emptiness alone would not do the job: every row below
    /// that reads `App::nearby_*` scans around the player's `Position`,
    /// which stays pinned to the surface entrance tile while the party is
    /// in the Stack — so those menus would cheerfully list a base four
    /// frames overhead.
    surface_only: bool,
    available: fn(&mut App) -> bool,
}

const BASE_ROWS: &[GroupEntry] = &[
    GroupEntry {
        label: "Deploy a structure",
        target: Mode::Build,
        surface_only: true,
        available: |app| {
            app.game
                .as_ref()
                .is_some_and(|g| !g.buildable_structure_defs().is_empty())
        },
    },
    GroupEntry {
        label: "Compile an item",
        target: Mode::Craft,
        surface_only: false,
        available: |app| {
            app.game
                .as_ref()
                .is_some_and(|g| !g.craft_recipes().is_empty())
        },
    },
    GroupEntry {
        // Surface-only for the reason every scan row is, and the engine
        // agrees rather than trusting the flag: `queue_work_order` calls
        // `Game::require_surface`. The screen is read-only but it *claims
        // something about where the base is*, which is the test
        // `find_target_in_direction` established — a report answered from a
        // `Position` pinned to the surface entrance tile would describe a
        // base four frames overhead.
        label: "Work orders",
        target: Mode::WorkOrders,
        surface_only: true,
        available: |app| {
            app.game
                .as_ref()
                .is_some_and(|g| !g.work_orders().is_empty() || !g.orderable_items().is_empty())
        },
    },
    GroupEntry {
        label: "Base staff",
        target: Mode::BaseStaff,
        surface_only: true,
        available: |app| !app.nearby_programs().is_empty(),
    },
    GroupEntry {
        label: "Work a structure yourself",
        target: Mode::WorkStructure,
        surface_only: true,
        available: |app| !app.workable_structures().is_empty(),
    },
    GroupEntry {
        label: "Upgrade a structure",
        target: Mode::Upgrade,
        surface_only: true,
        available: |app| !app.upgradeable_structures().is_empty(),
    },
    GroupEntry {
        label: "Demolish a structure",
        target: Mode::Remove,
        surface_only: true,
        available: |app| !app.nearby_structures().is_empty(),
    },
    GroupEntry {
        label: "Structure roster",
        target: Mode::Structures,
        surface_only: false,
        available: |app| {
            app.game
                .as_mut()
                .is_some_and(|g| !g.structure_report().is_empty())
        },
    },
    GroupEntry {
        label: "Research",
        target: Mode::Research,
        surface_only: false,
        available: |app| {
            app.game
                .as_ref()
                .is_some_and(|g| !g.research_nodes().is_empty())
        },
    },
    GroupEntry {
        // Not surface-only, and deliberately so: the offers half asks
        // `Game::contract_board`, which answers `None` underground on its own
        // rather than through a `nearby_*` scan around a `Position` pinned to
        // the surface entrance tile — so underground this reduces to "is
        // anything in hand", which is exactly the question worth answering
        // four frames down.
        label: "Contracts",
        target: Mode::Contracts,
        surface_only: false,
        available: |app| {
            app.game.as_mut().is_some_and(|g| {
                g.contract_board().is_some_and(|board| !board.is_empty())
                    || !g.active_contracts().is_empty()
            })
        },
    },
    GroupEntry {
        // Not surface-only: the chains come off the loaded assets, not off a
        // scan around the player, so this one row means the same thing four
        // frames down as it does standing in the base.
        label: "Recipes",
        target: Mode::Recipes,
        surface_only: false,
        available: |app| {
            app.game
                .as_ref()
                .is_some_and(|g| !g.recipe_chains().is_empty())
        },
    },
];

const PARTY_ROWS: &[GroupEntry] = &[
    GroupEntry {
        label: "Companions",
        target: Mode::Companion,
        surface_only: false,
        available: |app| {
            app.game
                .as_mut()
                .is_some_and(|g| !g.owned_pets().is_empty())
        },
    },
    GroupEntry {
        label: "Read a manifest",
        target: Mode::ManifestPick,
        surface_only: false,
        available: |app| !app.manifest_subjects().is_empty(),
    },
    GroupEntry {
        // Two, not one: fusion consumes both programs, so a lone companion
        // has nothing to fuse with and the second picker would be empty.
        label: "Fuse two programs",
        target: Mode::Fuse,
        surface_only: false,
        available: |app| app.game.as_mut().is_some_and(|g| g.owned_pets().len() >= 2),
    },
    GroupEntry {
        label: "Install a routine",
        target: Mode::RoutineTarget,
        surface_only: false,
        available: |app| {
            app.game
                .as_mut()
                .is_some_and(|g| !g.routine_holders().is_empty())
        },
    },
    GroupEntry {
        // Its own row rather than only the `[e]` detour off the install
        // screen: that detour opens from an *empty* slot, and every routine
        // slot in the game starts full, so a player who had never popped one
        // out could not reach the screen that makes disks at all.
        label: "Etch a routine disk",
        target: Mode::RoutineEtch,
        surface_only: false,
        available: |app| {
            app.game
                .as_ref()
                .is_some_and(|g| !g.etchable_routines().is_empty())
        },
    },
    GroupEntry {
        // Both halves matter: a program to spend an upgrade on, and an
        // upgrade to spend. Either missing leaves the second page empty.
        // Not `surface_only` — a refactor reaches no zone-map state through
        // `Position`, so it works four frames down.
        label: "Refactor a program",
        target: Mode::Refactor,
        surface_only: false,
        available: |app| {
            app.game
                .as_mut()
                .is_some_and(|g| !g.owned_pets().is_empty() && !g.companion_upgrades().is_empty())
        },
    },
    GroupEntry {
        label: "Extract a routine",
        target: Mode::Extract,
        surface_only: false,
        available: |app| {
            app.game
                .as_mut()
                .is_some_and(|g| !g.owned_pets().is_empty())
        },
    },
    GroupEntry {
        label: "Perks",
        target: Mode::Perks,
        surface_only: false,
        available: |app| app.game.as_ref().is_some_and(|g| !g.perk_defs().is_empty()),
    },
];

impl App {
    /// The base menu's rows as they stand right now. Both the handler that
    /// picks from this and the renderer that draws it call *this* function —
    /// rows are hidden dynamically, so a renderer building its own copy of
    /// the list would drift out of index with the handler and row 2 would
    /// open a different screen from the one under the highlight.
    pub fn base_menu_rows(&mut self) -> Vec<GroupMenuRow> {
        self.group_rows(BASE_ROWS)
    }

    /// The party menu's rows — same contract as `base_menu_rows`.
    pub fn party_menu_rows(&mut self) -> Vec<GroupMenuRow> {
        self.group_rows(PARTY_ROWS)
    }

    /// A row survives when it is not surface-only underground, and when the
    /// screen it opens would have at least one row of its own.
    ///
    /// That second clause deliberately asks only the *first* screen a row
    /// opens. "Work orders" can therefore open on a queue and then land on
    /// an empty item picker — a cheap mistake rather than a dead end, since
    /// Esc backs out into the menu (see `App::close_screen`).
    /// Asking the whole chain would need a bespoke predicate per row, which
    /// is the duplication this table exists to avoid.
    fn group_rows(&mut self, entries: &'static [GroupEntry]) -> Vec<GroupMenuRow> {
        let underground = self.game.as_ref().is_some_and(|g| g.is_underground());
        entries
            .iter()
            .filter(|e| !(e.surface_only && underground))
            .filter(|e| (e.available)(self))
            .map(|e| GroupMenuRow {
                label: e.label,
                target: e.target,
            })
            .collect()
    }

    pub(crate) fn handle_base_menu_key(&mut self, key: GameKey) {
        self.handle_group_menu_key(key, BASE_ROWS);
    }

    pub(crate) fn handle_party_menu_key(&mut self, key: GameKey) {
        self.handle_group_menu_key(key, PARTY_ROWS);
    }

    fn handle_group_menu_key(&mut self, key: GameKey, entries: &'static [GroupEntry]) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let rows = self.group_rows(entries);
        if let Some(idx) = self.selected_index(key, rows.len()) {
            self.menu_origin = Some(self.mode);
            self.mode = rows[idx].target;
        }
    }

    /// Where Esc from a screen goes: back to the group menu that opened it,
    /// or to the map if it was opened from the map.
    ///
    /// `menu_origin` is consumed here and cleared whenever the map is
    /// reached (see `App::handle_key`), so completing an action — which
    /// drops straight to the map — can't leave a stale origin for some
    /// unrelated screen's Esc to follow later.
    pub(crate) fn close_screen(&mut self) {
        self.mode = self.menu_origin.take().unwrap_or(Mode::Playing);
    }
}
