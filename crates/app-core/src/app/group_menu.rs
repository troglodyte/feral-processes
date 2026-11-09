//! The hub command pages — the base's Structures stopgap (`b`) and the
//! party's two (`p`, then Programs / Routines) — and the rule that decides
//! which of their rows are on screen right now. The base's other launchers
//! became the tabs they opened (hub merge, ADR 0045).
//!
//! These exist because the map screen bound 27 keys. A row here is the
//! *only* way to reach the screen behind it: the old direct keys were
//! retired rather than kept as aliases, so the flat surface actually shrank
//! instead of growing a second system to document. The base half used to be
//! a menu of its own; it lives in the hub now so the base is one window.

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
    available: fn(&mut App) -> bool,
}

/// The BASE section's Structures page: a stopgap that keeps the three
/// structure verbs reachable until hub-merge phase 3 makes them keys on the
/// structure's Info tab in `i`. Delete it with the page.
///
/// Each is a field action (`Game::require_field`'s callers), so a row is
/// also hidden in the Stack.
const STRUCTURE_ROWS: &[GroupEntry] = &[
    GroupEntry {
        label: "Work a structure yourself",
        target: Mode::WorkStructure,
        available: |app| on_the_field(app) && !app.workable_structures().is_empty(),
    },
    GroupEntry {
        label: "Upgrade a structure",
        target: Mode::UpgradeDirection,
        // The *scan radius*, where the screen it opens reaches one tile:
        // a row that appeared only while you stood beside a machine would
        // hide the verb from a player who never has.
        available: |app| on_the_field(app) && !app.upgradeable_structures().is_empty(),
    },
    GroupEntry {
        label: "Demolish a structure",
        target: Mode::Remove,
        available: |app| on_the_field(app) && !app.nearby_structures().is_empty(),
    },
];

fn on_the_field(app: &App) -> bool {
    app.game.as_ref().is_some_and(|g| !g.is_underground())
}

const PROGRAM_ROWS: &[GroupEntry] = &[
    GroupEntry {
        label: "Read a manifest",
        target: Mode::Manifest,
        available: |app| !app.manifest_subjects().is_empty(),
    },
    GroupEntry {
        // Two, not one: fusion consumes both programs, so a lone companion
        // has nothing to fuse with and the second picker would be empty.
        label: "Fuse two programs",
        target: Mode::Fuse,
        // Fusion is a researched capability (Task 10, `Game::fusion_unlocked`)
        // — the research alone, no structure and no locality change, so this
        // is a second term on the same closure rather than a new row.
        available: |app| {
            app.game
                .as_mut()
                .is_some_and(|g| g.owned_pets().len() >= 2 && g.fusion_unlocked())
        },
    },
    GroupEntry {
        // Both halves matter: a program to spend an upgrade on, and an
        // upgrade to spend. Either missing leaves the second page empty.
        // Not `base_only` — a refactor reaches no zone-map state through
        // `Position`, so it works four frames down.
        label: "Refactor a program",
        target: Mode::Refactor,
        available: |app| {
            app.game
                .as_mut()
                .is_some_and(|g| !g.owned_pets().is_empty() && !g.companion_upgrades().is_empty())
        },
    },
    GroupEntry {
        // One half, not two: a program is enough. Whether a ring can be
        // opened depends on cargo, but the same page shows the talent ladder
        // — a developed program with no rings left to buy still has points to
        // spend. Not `base_only`, like the refactor row above it.
        label: "Develop a program",
        target: Mode::Develop,
        available: |app| {
            app.game
                .as_mut()
                .is_some_and(|g| !g.owned_pets().is_empty())
        },
    },
];

const ROUTINE_ROWS: &[GroupEntry] = &[
    GroupEntry {
        label: "Install a routine",
        target: Mode::RoutineTarget,
        available: |app| {
            app.game
                .as_mut()
                .is_some_and(|g| !g.routine_holders().is_empty())
        },
    },
    GroupEntry {
        label: "Extract a routine",
        target: Mode::Extract,
        available: |app| {
            app.game
                .as_mut()
                .is_some_and(|g| !g.owned_pets().is_empty())
        },
    },
];

/// The static table behind a command page, or `None` for any other mode.
fn command_table(page: Mode) -> Option<&'static [GroupEntry]> {
    match page {
        Mode::BaseStructures => Some(STRUCTURE_ROWS),
        Mode::PartyPrograms => Some(PROGRAM_ROWS),
        Mode::PartyRoutines => Some(ROUTINE_ROWS),
        _ => None,
    }
}

/// Whether `mode` is one of the hub's command pages.
pub(crate) fn is_command_page(mode: Mode) -> bool {
    command_table(mode).is_some()
}

impl App {
    /// A command page's rows as they stand right now, or `None` when `page`
    /// is not a command page. The handler that picks from this, the renderer
    /// that draws it and the Overview's count all call *this* function —
    /// rows are hidden dynamically, so a caller building its own copy of the
    /// list would drift out of index with the handler and row 2 would open a
    /// different screen from the one under the highlight.
    pub fn command_rows(&mut self, page: Mode) -> Option<Vec<GroupMenuRow>> {
        let table = command_table(page)?;
        Some(self.group_rows(table))
    }

    /// A row survives when the screen it opens would have at least one row
    /// of its own.
    ///
    /// That second clause deliberately asks only the *first* screen a row
    /// opens. A row can therefore open on a screen whose own next step is
    /// empty — a cheap mistake rather than a dead end, since Esc backs out
    /// into the menu (see `App::close_screen`). Asking the whole chain would
    /// need a bespoke predicate per row, which is the duplication this table
    /// exists to avoid.
    fn group_rows(&mut self, entries: &'static [GroupEntry]) -> Vec<GroupMenuRow> {
        entries
            .iter()
            .filter(|e| (e.available)(self))
            .map(|e| GroupMenuRow {
                label: e.label,
                target: e.target,
            })
            .collect()
    }

    /// Opens the screen a group-menu or command-page row points at. The
    /// callers have already set `menu_origin`, so Esc comes back to them.
    pub(crate) fn open_row_target(&mut self, target: Mode) {
        match target {
            Mode::Manifest => self.open_menu_manifest(),
            target => self.mode = target,
        }
    }

    /// Where Esc from a screen goes: back to the screen that opened it, or
    /// to the map if it was opened from the map.
    ///
    /// `menu_origin` is consumed here and cleared whenever the map is
    /// reached (see `App::handle_key`), so completing an action — which
    /// drops straight to the map — can't leave a stale origin for some
    /// unrelated screen's Esc to follow later.
    pub(crate) fn close_screen(&mut self) {
        self.mode = self.menu_origin.take().unwrap_or(Mode::Playing);
    }
}
