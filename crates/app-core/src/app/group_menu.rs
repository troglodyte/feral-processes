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
    /// Where this row's action is legal, which for most of the base menu is
    /// "in base space" — see `Locality`.
    ///
    /// It used to be a `surface_only` bool and used to ask
    /// `is_underground()`, which was the same question while "not in the
    /// Stack" and "where the base is" were one condition. They are two now —
    /// `docs/seams.md` carries the split — and this names the half it always
    /// meant: the engine refuses each of these rows anywhere but base space,
    /// so offering them on the open grid advertises a screen whose every
    /// action is a dead end.
    ///
    /// A field in a readable table rather than an `in_base()` check folded
    /// into each `available` closure, because it has to be kept in step with
    /// that list in the engine and a table is what makes that checkable.
    /// Emptiness alone would not do the job: every row below that reads
    /// `App::nearby_*` scans around the party, which in base space is a
    /// different coordinate space entirely — so those menus would cheerfully
    /// list a base the party is nowhere near.
    locality: Locality,
    available: fn(&mut App) -> bool,
}

/// Where a group-menu row's action is legal.
///
/// Three answers and not two, because the build row genuinely has a third:
/// deploying is a `Game::require_base` caller like everything else in the
/// base menu, *except* for the run's first Home. That one is the act that
/// opens base space at all, so it has to be reachable from the open grid or
/// a fresh run can never start a base — and a row that was hidden until you
/// were inside would hide the only way of getting inside.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Locality {
    /// Legal wherever the party is standing.
    Anywhere,
    /// Base space only — one of `Game::require_base`'s callers.
    Base,
    /// Base space, and the open grid while no Home stands: founding.
    BaseOrFounding,
}

impl Locality {
    /// Whether a row with this locality belongs on screen right now.
    fn permits(self, app: &App) -> bool {
        let Some(game) = app.game.as_ref() else {
            return self == Locality::Anywhere;
        };
        match self {
            Locality::Anywhere => true,
            Locality::Base => game.in_base(),
            // Deliberately not "anywhere but the Stack": the founding deploy
            // asks `Game::require_surface`, so the open grid is the only
            // place outside the base it is permitted from.
            Locality::BaseOrFounding => {
                game.in_base() || (!game.has_home() && !game.is_underground())
            }
        }
    }
}

const BASE_ROWS: &[GroupEntry] = &[
    GroupEntry {
        // The one row with a third answer: see `Locality::BaseOrFounding`.
        label: "Deploy a structure",
        target: Mode::Build,
        locality: Locality::BaseOrFounding,
        available: |app| {
            app.game
                .as_ref()
                .is_some_and(|g| !g.buildable_structure_defs().is_empty())
        },
    },
    GroupEntry {
        label: "Compile an item",
        target: Mode::Craft,
        locality: Locality::Anywhere,
        available: |app| {
            app.game
                .as_ref()
                .is_some_and(|g| !g.craft_recipes().is_empty())
        },
    },
    GroupEntry {
        // Base-only for the reason every scan row is, and the engine
        // agrees rather than trusting the flag: `queue_work_order` calls
        // `Game::require_base`. The screen is read-only but it *claims
        // something about where the base is*, which is the test
        // `find_target_in_direction` established — a report answered from a
        // `Position` that is pinned somewhere else would describe a base the
        // party is nowhere near.
        label: "Work orders",
        target: Mode::WorkOrders,
        locality: Locality::Base,
        available: |app| {
            app.game
                .as_ref()
                .is_some_and(|g| !g.work_orders().is_empty() || !g.orderable_items().is_empty())
        },
    },
    GroupEntry {
        label: "Base staff",
        target: Mode::BaseStaff,
        locality: Locality::Base,
        available: |app| !app.nearby_programs().is_empty(),
    },
    GroupEntry {
        label: "Work a structure yourself",
        target: Mode::WorkStructure,
        locality: Locality::Base,
        available: |app| !app.workable_structures().is_empty(),
    },
    GroupEntry {
        label: "Upgrade a structure",
        target: Mode::Upgrade,
        locality: Locality::Base,
        available: |app| !app.upgradeable_structures().is_empty(),
    },
    GroupEntry {
        label: "Demolish a structure",
        target: Mode::Remove,
        locality: Locality::Base,
        available: |app| !app.nearby_structures().is_empty(),
    },
    GroupEntry {
        label: "Structure roster",
        target: Mode::Structures,
        locality: Locality::Anywhere,
        available: |app| {
            app.game
                .as_mut()
                .is_some_and(|g| !g.structure_report().is_empty())
        },
    },
    GroupEntry {
        label: "Research",
        target: Mode::Research,
        locality: Locality::Anywhere,
        available: |app| {
            app.game
                .as_ref()
                .is_some_and(|g| !g.research_nodes().is_empty())
        },
    },
    GroupEntry {
        // Not base-only, and deliberately so: mission status is the
        // question worth answering four frames down, and the board itself is
        // a property of the sector rather than of where the party is
        // standing. Off the base the screen opens read-only — see
        // `Game::broker_reach`.
        //
        // `broker_reach` rather than `contract_board`, which is what this row
        // used to ask: this closure runs every frame the menu is open, and a
        // board that no longer refuses on distance rolls every template and
        // samples the habitat ring before it can answer. The proximity check
        // used to short-circuit all of that.
        label: "Contracts",
        target: Mode::Contracts,
        locality: Locality::Anywhere,
        available: |app| {
            app.game.as_mut().is_some_and(|g| {
                g.broker_reach() != BrokerReach::NoBroker || !g.active_contracts().is_empty()
            })
        },
    },
    GroupEntry {
        // Not `Locality::Base`, and the row is not `surface_only` either:
        // `caravan_reach` already measures base space, so a second locality
        // clause here would be the same fact read twice and the redundant
        // half is the one that rots.
        //
        // `caravan_reach` rather than `caravan_view`, which is the trap the
        // Contracts row above records: this closure runs every frame the menu
        // is open, and the view rolls a whole shelf — gear copies, affixes
        // and all — before it can answer a question about where the player is
        // standing.
        label: "Caravan",
        target: Mode::Caravan,
        locality: Locality::Anywhere,
        available: |app| {
            app.game
                .as_mut()
                .is_some_and(|g| g.caravan_reach() == CaravanReach::AtCaravan)
        },
    },
    GroupEntry {
        // Not `Locality::Base`, and for the Caravan row's reason directly
        // above: `dispatch_reach` already measures base space, so a second
        // locality clause here would be the same fact read twice.
        label: "Dispatch",
        target: Mode::Dispatch,
        locality: Locality::Anywhere,
        available: |app| {
            app.game
                .as_mut()
                .is_some_and(|g| g.dispatch_reach() != DispatchReach::NoRelay)
        },
    },
    GroupEntry {
        // Not base-only, and for the Recipes row's reason one notch further:
        // the ledger is *history*, so it reads the same wherever the party
        // is standing — and the base keeps producing while they are in the
        // Stack, which is exactly when the question is worth asking.
        //
        // `has_base_output` rather than the report itself, which is the trap
        // the Contracts row records: this closure runs every frame the menu
        // is open and the report resolves a def per item and walks every
        // structure before it can answer.
        label: "Base output",
        target: Mode::BaseOutput,
        locality: Locality::Anywhere,
        available: |app| app.game.as_ref().is_some_and(|g| g.has_base_output()),
    },
    GroupEntry {
        // Not base-only: the chains come off the loaded assets, not off a
        // scan around the player, so this one row means the same thing four
        // frames down as it does standing in the base.
        label: "Recipes",
        target: Mode::Recipes,
        locality: Locality::Anywhere,
        available: |app| {
            app.game
                .as_ref()
                .is_some_and(|g| !g.recipe_chains().is_empty())
        },
    },
];

const PARTY_ROWS: &[GroupEntry] = &[
    GroupEntry {
        // **Not `Locality::Base`, even though the two party verbs behind it
        // are.** This row is the whole roster screen — gear, memories, the
        // manifest, a rename — and only the join and the stand-down are
        // decided at home. Hidden off-base it would take reading your own
        // programs away four frames down along with the one thing that
        // needed guarding. `add_companion` and `stand_down_companion` carry
        // `require_base` themselves and refuse onto the status line, which
        // is a sentence the player can act on rather than a row that is
        // simply gone. See `docs/seams.md`'s guard table.
        label: "Companions",
        target: Mode::Companion,
        locality: Locality::Anywhere,
        available: |app| {
            app.game
                .as_mut()
                .is_some_and(|g| !g.owned_pets().is_empty())
        },
    },
    GroupEntry {
        label: "Read a manifest",
        target: Mode::ManifestPick,
        locality: Locality::Anywhere,
        available: |app| !app.manifest_subjects().is_empty(),
    },
    GroupEntry {
        // Two, not one: fusion consumes both programs, so a lone companion
        // has nothing to fuse with and the second picker would be empty.
        label: "Fuse two programs",
        target: Mode::Fuse,
        locality: Locality::Anywhere,
        available: |app| app.game.as_mut().is_some_and(|g| g.owned_pets().len() >= 2),
    },
    GroupEntry {
        label: "Install a routine",
        target: Mode::RoutineTarget,
        locality: Locality::Anywhere,
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
        locality: Locality::Anywhere,
        available: |app| {
            app.game
                .as_ref()
                .is_some_and(|g| !g.etchable_routines().is_empty())
        },
    },
    GroupEntry {
        // Both halves matter: a program to spend an upgrade on, and an
        // upgrade to spend. Either missing leaves the second page empty.
        // Not `base_only` — a refactor reaches no zone-map state through
        // `Position`, so it works four frames down.
        label: "Refactor a program",
        target: Mode::Refactor,
        locality: Locality::Anywhere,
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
        locality: Locality::Anywhere,
        available: |app| {
            app.game
                .as_mut()
                .is_some_and(|g| !g.owned_pets().is_empty())
        },
    },
    GroupEntry {
        label: "Extract a routine",
        target: Mode::Extract,
        locality: Locality::Anywhere,
        available: |app| {
            app.game
                .as_mut()
                .is_some_and(|g| !g.owned_pets().is_empty())
        },
    },
    GroupEntry {
        label: "Perks",
        target: Mode::Perks,
        locality: Locality::Anywhere,
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

    /// A row survives when the party is standing where its action is legal,
    /// and when the screen it opens would have at least one row of its own.
    ///
    /// That second clause deliberately asks only the *first* screen a row
    /// opens. "Work orders" can therefore open on a queue and then land on
    /// an empty item picker — a cheap mistake rather than a dead end, since
    /// Esc backs out into the menu (see `App::close_screen`).
    /// Asking the whole chain would need a bespoke predicate per row, which
    /// is the duplication this table exists to avoid.
    fn group_rows(&mut self, entries: &'static [GroupEntry]) -> Vec<GroupMenuRow> {
        // The locality pass is resolved up front rather than chained into
        // the same iterator as `available`: both read `self`, and only the
        // second one needs it mutably.
        let here: Vec<&GroupEntry> = entries
            .iter()
            .filter(|e| e.locality.permits(self))
            .collect();
        here.into_iter()
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
