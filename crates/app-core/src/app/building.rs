//! Placing, staffing, demolishing and upgrading base structures.

use crate::*;

/// What `Mode::StructureAssign` is showing: the structure picked on the
/// roster, and the standing instructions that can be set on it.
///
/// The name travels with the rows because this popup covers the roster row it
/// was opened from — a picker with nothing else on it would leave the player
/// guessing which machine they were pointing at.
///
/// **This was a program picker until 2026-08-14.** Posting a program to a
/// machine by hand is gone: `schedule_base_labour` decides who stands where,
/// and what a player says about a particular machine is now "keep this one
/// running" or "keep this one guarded". The spec's phrasing is the argument
/// — the question "should this machine always be working" belongs to the
/// machine, so it is a toggle on the structure screen rather than a menu
/// row of its own.
pub struct Staffing {
    pub target: String,
    pub rows: Vec<StaffRow>,
}

/// One standing instruction that can be toggled on the structure highlighted
/// on the roster — see `App::staffing` and `components::StandingJob`.
///
/// A row rather than a bare bool because one of them is not a standing job
/// at all: the "work it yourself" row calls `Game::work_structure`, since
/// the player is not staff and the scheduler never moves them. Keeping all
/// of them in one ordered list is what stops the renderer and the handler
/// disagreeing about which index means what, the same rule
/// `App::base_menu_rows` holds for a menu whose rows are hidden dynamically.
#[derive(Clone)]
pub struct StaffRow {
    pub label: String,
    pub kind: StaffAction,
    /// Whether the instruction is currently set — `None` on a row that is
    /// an action rather than a toggle.
    pub on: Option<bool>,
}

/// What a `StaffRow` does when picked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StaffAction {
    /// Toggle `StandingJob::work` — keep a body on this machine whenever no
    /// order needs it elsewhere.
    StandingWork,
    /// Toggle `StandingJob::guard`.
    StandingGuard,
    /// Work it yourself, right now. Not a standing anything.
    WorkYourself,
}

impl App {
    /// What can be said about `pending_post_structure`: the two standing
    /// instructions it can carry, and — if you are standing beside it — the
    /// offer to work it yourself.
    ///
    /// The "yourself" row is filtered on `StructureReport::player_adjacent`
    /// rather than offered everywhere and refused, matching
    /// `App::upgradeable_structures` — `Game::work_structure` takes only the
    /// four orthogonal neighbours, and the roster is zone-wide, so on almost
    /// every row that offer would be a dead end.
    ///
    /// A standing *work* job is offered only where a program could be
    /// posted, and a standing *guard* only where a sweep could land — the
    /// same two questions `Game::set_standing_job` refuses on, asked here so
    /// the screen cannot list a row the engine would reject.
    pub fn staffing(&mut self) -> Option<Staffing> {
        let structure = self.pending_post_structure?;
        let row = self
            .game
            .as_mut()
            .map(|g| g.structure_report())
            .unwrap_or_default()
            .into_iter()
            .find(|s| s.entity == structure)?;
        let (work, guard) = self
            .game
            .as_ref()
            .and_then(|g| g.standing_job(structure))
            .unwrap_or((false, false));
        let mut rows = Vec::new();
        if row.workable {
            rows.push(StaffRow {
                label: "Keep this machine running".to_string(),
                kind: StaffAction::StandingWork,
                on: Some(work),
            });
        }
        if row.durability.is_some() {
            rows.push(StaffRow {
                label: "Keep a guard on this".to_string(),
                kind: StaffAction::StandingGuard,
                on: Some(guard),
            });
        }
        if row.player_adjacent {
            rows.push(StaffRow {
                label: "Work it yourself".to_string(),
                kind: StaffAction::WorkYourself,
                on: None,
            });
        }
        Some(Staffing {
            target: row.label,
            rows,
        })
    }

    /// Applies whichever instruction was picked to the structure the roster
    /// was showing, and goes back to that row — see `Mode::StructureAssign`.
    ///
    /// A toggle stays on the screen so a player can set both; working it
    /// yourself leaves, because it spends the turn.
    pub(crate) fn handle_structure_assign_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.leave_staffing();
            return;
        }
        let Some(structure) = self.pending_post_structure else {
            self.mode = Mode::Structures;
            return;
        };
        let rows = self.staffing().map(|s| s.rows).unwrap_or_default();
        let Some(idx) = self.selected_index(key, rows.len()) else {
            return;
        };
        let (work, guard) = self
            .game
            .as_ref()
            .and_then(|g| g.standing_job(structure))
            .unwrap_or((false, false));
        let kind = rows[idx].kind;
        let Some(game) = &mut self.game else { return };
        match kind {
            StaffAction::StandingWork => {
                self.status_line = game.set_standing_job(structure, !work, guard).err();
                self.menu_selected = idx;
            }
            StaffAction::StandingGuard => {
                self.status_line = game.set_standing_job(structure, work, !guard).err();
                self.menu_selected = idx;
            }
            StaffAction::WorkYourself => {
                self.status_line = game.work_structure(structure).err();
                self.leave_staffing();
            }
        }
    }

    /// Back to the roster, on the structure that was being staffed. Looked up
    /// by entity rather than by a remembered index because the roster is
    /// rebuilt from `Game::structure_report` on the way back in.
    fn leave_staffing(&mut self) {
        let structure = self.pending_post_structure.take();
        self.mode = Mode::Structures;
        let Some(structure) = structure else { return };
        if let Some(row) = self
            .game
            .as_mut()
            .map(|g| g.structure_report())
            .unwrap_or_default()
            .iter()
            .position(|s| s.entity == structure)
        {
            self.menu_selected = row;
        }
    }

    /// Every tamed program the player owns — the candidates for a cronjob or
    /// a guard posting.
    ///
    /// The whole roster rather than what is within `MENU_SCAN_RADIUS`, for
    /// the reason `handle_fuse_key` draws from `owned_pets`: a companion's
    /// `Position` is the tile it was captured on and is never written again,
    /// so a distance filter hides programs by where they were beaten. It hid
    /// them from the *row* too — `base_menu_rows` drops a row whose screen
    /// would be empty, so a player whose only program was tamed 40 tiles ago
    /// lost the Cronjob row entirely and never learned posting exists.
    /// Neither `assign_cronjob` (which now starts the program from the
    /// player's own tile) nor `assign_guard` asks anything about where the
    /// program is standing.
    ///
    /// This and the three lists below exist because each was written twice:
    /// once in the handler that picks from it, once in the renderer that
    /// draws it. The base menu's row-availability check (see
    /// `App::base_menu_rows`) would have been a third copy, and a menu that
    /// offers a row leading to an empty screen is exactly the drift that
    /// invites.
    pub fn nearby_programs(&mut self) -> Vec<EntityView> {
        let Some(game) = &mut self.game else {
            return Vec::new();
        };
        game.owned_program_views()
    }

    /// Nearby structures that accept a posted program. The same list whether
    /// the work is done by a program (`Mode::CronjobStructure`) or by the
    /// player themselves (`Mode::WorkStructure`) — it is the same job either
    /// way, see `Game::work_structure`.
    pub fn workable_structures(&mut self) -> Vec<EntityView> {
        self.scanned(|e| e.can_work)
    }

    /// Every nearby structure, whatever it is: a guard posts to any of them
    /// and demolition takes any of them.
    pub fn nearby_structures(&mut self) -> Vec<EntityView> {
        self.scanned(|e| e.is_structure)
    }

    /// Nearby structures that declare an upgrade path. Filtered on `tier`
    /// rather than just `is_structure`: offering an un-upgradeable structure
    /// and then refusing it would be a worse menu than not listing it.
    pub fn upgradeable_structures(&mut self) -> Vec<EntityView> {
        self.scanned(|e| e.is_structure && e.tier.is_some())
    }

    fn scanned(&mut self, keep: impl Fn(&EntityView) -> bool) -> Vec<EntityView> {
        let Some(game) = &mut self.game else {
            return Vec::new();
        };
        game.view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| keep(e))
            .collect()
    }

    pub(crate) fn handle_build_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let Some(game) = &self.game else { return };
        let defs = game.buildable_structure_defs();
        if let Some(idx) = self.selected_index(key, defs.len()) {
            self.pending_structure = Some(defs[idx].id.clone());
            self.mode = Mode::BuildDirection;
        }
    }

    pub(crate) fn handle_build_direction_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_structure = None;
            self.close_screen();
            return;
        }
        let dir = match key {
            GameKey::Up | GameKey::Char('k') => Some((0, -1)),
            GameKey::Down | GameKey::Char('j') => Some((0, 1)),
            GameKey::Left | GameKey::Char('h') => Some((-1, 0)),
            GameKey::Right | GameKey::Char('l') => Some((1, 0)),
            _ => None,
        };
        let Some((dx, dy)) = dir else { return };
        let Some(id) = self.pending_structure.take() else {
            self.mode = Mode::Playing;
            return;
        };
        if let Some(game) = &mut self.game {
            match game.place_structure(&id, dx, dy) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
        }
        self.mode = Mode::Playing;
    }

    /// Picks a nearby workable structure for the player to work themselves —
    /// see `Game::work_structure`. **The player is not staff**, so this flow
    /// survived work orders untouched: the scheduler never moves you.
    pub(crate) fn handle_work_structure_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let structures = self.workable_structures();
        if let Some(idx) = self.selected_index(key, structures.len()) {
            let Some(game) = &mut self.game else { return };
            match game.work_structure(structures[idx].entity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            self.mode = Mode::Playing;
        }
    }

    pub(crate) fn handle_remove_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let structures = self.nearby_structures();
        if let Some(idx) = self.selected_index(key, structures.len()) {
            let picked_entity = structures[idx].entity;
            let picked_is_home = structures[idx].is_home;
            if picked_is_home {
                self.pending_remove_structure = Some(picked_entity);
                self.mode = Mode::RemoveConfirm;
                return;
            }
            let Some(game) = &mut self.game else { return };
            match game.remove_structure(picked_entity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            self.mode = Mode::Playing;
        }
    }

    /// `d` + a direction: demolish whatever stands on that neighbouring
    /// tile.
    ///
    /// The adjacency is `Game::adjacent_structure`'s, not a cone like `x`'s —
    /// this key destroys what it finds, so it has to be something you are
    /// standing next to. Home routes into the same warning the menu's picker
    /// uses, read off the same `is_home` field, so the two ways in cannot
    /// disagree about what cascades.
    pub(crate) fn handle_remove_direction_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let dir = match key {
            GameKey::Up | GameKey::Char('k') => (0, -1),
            GameKey::Down | GameKey::Char('j') => (0, 1),
            GameKey::Left | GameKey::Char('h') => (-1, 0),
            GameKey::Right | GameKey::Char('l') => (1, 0),
            _ => return,
        };
        let Some(game) = &mut self.game else { return };
        let Some(found) = game.adjacent_structure(dir.0, dir.1) else {
            self.status_line = Some("Nothing to demolish that way.".to_string());
            self.mode = Mode::Playing;
            return;
        };
        if found.is_home {
            self.pending_remove_structure = Some(found.entity);
            self.mode = Mode::RemoveConfirm;
            return;
        }
        match game.remove_structure(found.entity) {
            Ok(()) => self.status_line = None,
            Err(e) => self.status_line = Some(e),
        }
        self.mode = Mode::Playing;
    }

    pub(crate) fn handle_upgrade_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let structures = self.upgradeable_structures();
        if let Some(idx) = self.selected_index(key, structures.len()) {
            let picked = structures[idx].entity;
            let Some(game) = &mut self.game else { return };
            match game.upgrade_structure(picked) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            self.mode = Mode::Playing;
        }
    }

    pub(crate) fn handle_remove_confirm_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_remove_structure = None;
            self.close_screen();
            return;
        }
        let options = ['y', 'n'];
        let idx = self
            .selected_index(key, options.len())
            .or_else(|| match key {
                GameKey::Char(c) => options.iter().position(|&o| o == c.to_ascii_lowercase()),
                _ => None,
            });
        match idx.map(|i| options[i]) {
            Some('y') => {
                if let Some(structure) = self.pending_remove_structure.take() {
                    let Some(game) = &mut self.game else { return };
                    match game.remove_structure(structure) {
                        Ok(()) => self.status_line = None,
                        Err(e) => self.status_line = Some(e),
                    }
                }
                self.mode = Mode::Playing;
            }
            Some('n') => {
                self.pending_remove_structure = None;
                self.mode = Mode::Playing;
            }
            _ => {}
        }
    }

    /// Lists every deployed symlink-capable structure (e.g. Home) anywhere
    /// on the map — not scan-radius-limited like the build/cronjob
    /// menus — and teleports the player to the picked one.
    pub(crate) fn handle_symlink_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let Some(game) = &mut self.game else { return };
        let targets = game.symlink_targets();
        if let Some(idx) = self.selected_index(key, targets.len()) {
            let Some(game) = &mut self.game else { return };
            match game.use_symlink(targets[idx].entity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            self.mode = Mode::Playing;
        }
    }
}

/// One row on `Mode::WorkOrders` — either a queued order or the trailing
/// row that starts a new one.
///
/// A row rather than a bare report, for the reason `StaffRow` is one: the
/// "new order" row is not an order at all, and keeping both in one ordered
/// list is what stops the renderer and the handler disagreeing about which
/// index means what. That is the rule `App::base_menu_rows` holds for a
/// menu whose rows are hidden dynamically, one screen down.
#[derive(Clone)]
pub struct WorkOrderRow {
    /// The order this row reports, or `None` on the trailing "new order"
    /// row.
    pub order: Option<WorkOrderReport>,
}

/// One row on `Mode::BaseStaff`: a program you own and the role it is in.
#[derive(Clone)]
pub struct BaseStaffRow {
    pub program: EntityView,
    /// Which of `ProgramRole`'s roles this program is filling — the engine's
    /// one derivation, not a re-decision. `None` only for an entity that is
    /// not a program the player owns, which this screen never lists.
    ///
    /// Carried as the role rather than an on-staff flag because the roles are
    /// what the screen is *about*, and a boolean cannot grow a fourth one.
    pub role: Option<ProgramRole>,
    /// What it is doing right now — "working the Mining Node", "guarding the
    /// Shield", "in party", "idle".
    ///
    /// Total rather than `Option`: it distinguishes the post a staffer is on
    /// from a staffer waiting between postings, which the role alone does not.
    pub doing: String,
    /// What this program is worth at a post — see `views::WorkProfile`. The
    /// screen's whole decision, since the scheduler picks who stands where
    /// and all the player chooses is who is available to it.
    ///
    /// `None` only for a species the db has never heard of, which in play
    /// means a mod that failed to load. The renderer says so rather than
    /// quoting numbers nobody authored.
    pub work: Option<WorkProfile>,
}

impl App {
    /// The work order screen's rows: every queued order, then the row that
    /// queues another.
    ///
    /// The trailing row is dropped when nothing is orderable, so the screen
    /// never offers a picker that would open empty — the same question
    /// `BASE_ROWS`'s `available` closure asks one level up.
    pub fn work_order_rows(&mut self) -> Vec<WorkOrderRow> {
        let Some(game) = &self.game else {
            return Vec::new();
        };
        let mut rows: Vec<WorkOrderRow> = game
            .work_order_report()
            .into_iter()
            .map(|report| WorkOrderRow {
                order: Some(report),
            })
            .collect();
        if !game.orderable_items().is_empty() {
            rows.push(WorkOrderRow { order: None });
        }
        rows
    }

    /// Enter on the trailing row queues another order; Backspace drops the
    /// highlighted one.
    ///
    /// Cancelling needs no confirmation because it **unwinds nothing** —
    /// there are no per-machine targets to roll back and no reserved stock
    /// to release, so the next tick simply derives a different answer and
    /// re-queueing costs a keypress.
    pub(crate) fn handle_work_orders_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let rows = self.work_order_rows();
        if key == GameKey::Backspace {
            if self.menu_selected < rows.len() && rows[self.menu_selected].order.is_some() {
                let index = self.menu_selected;
                if let Some(game) = &mut self.game {
                    self.status_line = game.cancel_work_order(index).err();
                }
                self.menu_selected = self.menu_selected.saturating_sub(1);
            }
            return;
        }
        let Some(idx) = self.selected_index(key, rows.len()) else {
            return;
        };
        if rows[idx].order.is_none() {
            self.pending_order = None;
            self.order_quantity_input.clear();
            self.mode = Mode::WorkOrderPick;
            self.menu_selected = 0;
        }
    }

    /// What the base could be told to make — `Game::orderable_items`, which
    /// asks the same chain question `queue_work_order` refuses on, so this
    /// picker cannot offer a row the queue would then reject.
    pub fn orderable_items(&self) -> Vec<(ItemId, String)> {
        self.game
            .as_ref()
            .map(|g| g.orderable_items())
            .unwrap_or_default()
    }

    pub(crate) fn handle_work_order_pick_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::WorkOrders;
            self.menu_selected = 0;
            return;
        }
        let items = self.orderable_items();
        if let Some(idx) = self.selected_index(key, items.len()) {
            self.pending_order = Some(items[idx].0.clone());
            self.order_quantity_input.clear();
            // Cleared on the way in rather than on the way out, exactly as
            // `careful_craft` is: a flag left set would file the next batch
            // as a level the base holds forever, from a page that had gone
            // back to saying nothing about it.
            self.standing_order = false;
            self.mode = Mode::WorkOrderQuantity;
        }
    }

    /// Digits and Enter, the shape `Mode::CraftQuantity` already uses, plus
    /// `[S]` for a standing order — the careful-compile toggle's shape one
    /// screen over, cleared on the way in for the same reason.
    pub(crate) fn handle_work_order_quantity_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc => {
                self.pending_order = None;
                self.order_quantity_input.clear();
                self.mode = Mode::WorkOrderPick;
            }
            GameKey::Backspace => {
                self.order_quantity_input.pop();
            }
            GameKey::Char(c) if c.is_ascii_digit() && self.order_quantity_input.len() < 4 => {
                self.order_quantity_input.push(c);
            }
            GameKey::Char('s') | GameKey::Char('S') => {
                self.standing_order = !self.standing_order;
            }
            GameKey::Enter => {
                let Some(item) = self.pending_order.take() else {
                    self.mode = Mode::WorkOrders;
                    return;
                };
                let qty: u32 = self.order_quantity_input.parse().unwrap_or(1).max(1);
                self.order_quantity_input.clear();
                if let Some(game) = &mut self.game {
                    self.status_line = game.queue_work_order(item, qty, self.standing_order).err();
                }
                self.mode = Mode::WorkOrders;
                self.menu_selected = 0;
            }
            _ => {}
        }
    }

    /// Every program the player owns, with which side of the party/staff
    /// split it is on and what it is doing.
    pub fn base_staff_rows(&mut self) -> Vec<BaseStaffRow> {
        let programs = self.nearby_programs();
        let Some(game) = &self.game else {
            return Vec::new();
        };
        programs
            .into_iter()
            .map(|program| {
                let role = game.program_role(program.entity);
                let on_staff = role == Some(ProgramRole::Staff);
                // Off the staff, `Game::program_activity` is the engine's one
                // answer to "what is this program doing" — it already tells
                // the party, the wield and idleness apart, so this screen
                // cannot disagree with the sale and erase screens about a
                // program neither of them can see a `Task` on.
                BaseStaffRow {
                    doing: if on_staff {
                        game.staff_activity(program.entity)
                    } else {
                        game.program_activity(program.entity)
                    },
                    work: game.work_profile(program.entity),
                    role,
                    program,
                }
            })
            .collect()
    }

    /// Scrolling and closing, and deliberately nothing else.
    ///
    /// **This screen writes nothing.** A program you own and are not
    /// fighting with *is* base staff — `ProgramRole` derives the roles from
    /// the party and the wield, so there is no marker here to toggle and no
    /// state for a key to put a row out of step with. What the player
    /// changes is the party; the base takes whatever is left.
    ///
    /// Kept as its own handler rather than folded into a generic scroll,
    /// because this is where a role the player *does* choose between would
    /// be picked.
    pub(crate) fn handle_base_staff_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let rows = self.base_staff_rows().len();
        if let Some(idx) = self.selected_index(key, rows) {
            self.menu_selected = idx;
        }
    }
}
