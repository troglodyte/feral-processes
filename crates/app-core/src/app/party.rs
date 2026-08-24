//! Companion inspection and the three-page program fusion flow.

use crate::*;

impl App {
    /// Lists every tamed program you own, wherever it is. Every action on
    /// this screen acts on the highlight — `P` adds the highlighted program
    /// to the party or stands it down again, `<` and `>` shift it along the
    /// battle line, `N`/`E`/`W` rename, gear and wield it, `U` takes all of
    /// that gear back off, `M` reads its manifest — so a row's number or
    /// letter *only* moves the highlight.
    ///
    /// It used to toggle party membership on the spot, which meant a stray
    /// digit, or an Enter aimed at a screen that has never had one, stood a
    /// party member down with nothing to undo it but noticing.
    pub(crate) fn handle_companion_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let shift = match key {
            GameKey::Char('<') | GameKey::Char(',') => Some(SlotShift::Forward),
            GameKey::Char('>') | GameKey::Char('.') => Some(SlotShift::Back),
            _ => None,
        };
        if let Some(shift) = shift {
            self.shift_party_slot(shift);
            return;
        }
        if key == GameKey::Char('W') {
            self.toggle_wield();
            return;
        }
        if key == GameKey::Char('N') {
            self.begin_rename();
            return;
        }
        if key == GameKey::Char('E') {
            self.open_companion_equip();
            return;
        }
        if key == GameKey::Char('P') {
            self.toggle_party_member();
            return;
        }
        if key == GameKey::Char('M') {
            self.open_companion_manifest();
            return;
        }
        if key == GameKey::Char('R') {
            self.open_companion_memories();
            return;
        }
        if key == GameKey::Char('U') {
            self.strip_highlighted();
            return;
        }

        let Some(game) = &mut self.game else { return };
        let rows = game.owned_pets().len();
        // Highlight only. Enter resolves to `menu_selected.min(len - 1)`, so
        // it reassigns the highlight to itself — inert, which is what this
        // screen wants it to be.
        if let Some(idx) = self.selected_index(key, rows) {
            self.menu_selected = idx;
        }
    }

    /// Takes every piece of gear back off the highlighted program in one
    /// press, into your own cargo.
    ///
    /// A loop over `Game::unequip` rather than an engine verb of its own.
    /// Each slot is an independent removal that already logs its own line
    /// and spends its own tick, so stripping three slots here and stripping
    /// them one at a time through `E` and the picker cost the player exactly
    /// the same — an `unequip_all` in the engine would be a second variant
    /// of `unequip` with a different clock, and nothing outside this key
    /// would ever want it.
    ///
    /// The empty-slot refusal is **dropped**: over three slots it is the
    /// ordinary case rather than news, and `unequip` states it one slot at a
    /// time in a voice written for the picker. A program wearing nothing at
    /// all is the one case with something to say — three silent refusals
    /// leave no log line anywhere, so without this the key reads as broken.
    /// Any *other* refusal is surfaced as it comes and stops the sweep,
    /// since a game that has just said no to one slot will say no to the
    /// next two as well.
    fn strip_highlighted(&mut self) {
        let row = self.menu_selected;
        let Some(game) = &mut self.game else { return };
        let Some((program, name)) = game
            .owned_pets()
            .get(row)
            .map(|p| (p.entity, p.name.clone()))
        else {
            return;
        };
        let mut stripped = 0;
        let mut refusal = None;
        for slot in EquipmentSlot::ALL {
            if game.worn(program, slot).is_none() {
                continue;
            }
            match game.unequip(program, slot) {
                Ok(()) => stripped += 1,
                Err(e) => {
                    refusal = Some(e);
                    break;
                }
            }
        }
        self.status_line = match (stripped, refusal) {
            (_, Some(e)) => Some(e),
            (0, None) => Some(format!("{name} isn't wearing anything.")),
            _ => None,
        };
    }

    /// Stands the highlighted program in the party, or stands it down if it
    /// is already there. Bound to an uppercase key for the reason `W`, `N`
    /// and `E` are: uppercase reaches app-core as a distinct key, so it can
    /// never collide with `menu_shortcut`'s digits-then-lowercase scheme
    /// however large the roster grows.
    ///
    /// Standing down is never refused; joining is, once the party is full,
    /// and that refusal is the one thing on this screen with something to
    /// say, so it goes to the status line.
    fn toggle_party_member(&mut self) {
        let row = self.menu_selected;
        let Some(game) = &mut self.game else { return };
        let Some((entity, member)) = game
            .owned_pets()
            .get(row)
            .map(|p| (p.entity, p.party_slot.is_some()))
        else {
            return;
        };
        if member {
            game.remove_companion(entity);
            self.status_line = None;
        } else {
            let outcome = game.add_companion(entity);
            self.report(outcome);
        }
    }

    /// Types a new display name for the program picked with `N`; Enter
    /// commits it and blank clears back to the species name. Esc backs into
    /// the roster leaving the name alone.
    ///
    /// The entity comes from `pending_rename` rather than from
    /// `menu_selected`: `owned_pets` is not ordered by name today, but
    /// re-reading the highlight on Enter would silently rename whatever row
    /// happened to be under it if that ever changed.
    pub(crate) fn handle_rename_pet_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc => {
                self.pending_rename = None;
                self.rename_input.clear();
                self.mode = Mode::Companion;
            }
            GameKey::Backspace => {
                self.rename_input.pop();
            }
            GameKey::Char(c)
                if !c.is_control()
                    && self.rename_input.chars().count()
                        < feral_processes_engine::MAX_CUSTOM_NAME_LEN =>
            {
                self.rename_input.push(c);
            }
            GameKey::Enter => {
                let name = std::mem::take(&mut self.rename_input);
                let Some(entity) = self.pending_rename.take() else {
                    self.mode = Mode::Companion;
                    return;
                };
                let Some(game) = &mut self.game else { return };
                // Always `Some`, even when empty: the engine reads a blank
                // as "drop the override", which is the only way back to the
                // species name. `None` here would mean "leave it alone".
                let outcome = game.rename_companion(entity, Some(name));
                self.report(outcome);
                self.mode = Mode::Companion;
            }
            _ => {}
        }
    }

    /// Opens the naming page for the highlighted program, seeded with the
    /// name it already carries so a small correction isn't a retype.
    ///
    /// Handled before `selected_index` for the same reason `W` is, and
    /// bound to an uppercase key so it can never collide with
    /// `menu_shortcut`'s digits-then-lowercase scheme. Unlike `W`, this one
    /// is advertised in the roster's help text.
    /// Opens the highlighted program's three equipment slots.
    ///
    /// Handled before `selected_index` and bound to an uppercase key for the
    /// reason `toggle_wield` gives: uppercase reaches app-core as a distinct
    /// key, so it can never collide with `menu_shortcut`'s
    /// digits-then-lowercase scheme however large the roster grows.
    fn open_companion_equip(&mut self) {
        let row = self.menu_selected;
        let Some(game) = &mut self.game else { return };
        let Some(program) = game.owned_pets().get(row).map(|p| p.entity) else {
            return;
        };
        self.pending_equip_program = Some(program);
        self.status_line = None;
        // The roster's highlight indexes a different list from the three
        // slots, and can sit well past their end.
        self.menu_selected = 0;
        self.mode = Mode::CompanionEquip;
    }

    /// Opens the highlighted program's stat sheet — the same manifest the
    /// party menu's picker and the map's `x` open, reached from the screen
    /// the player is already comparing programs on.
    ///
    /// Handled before `selected_index` and bound to an uppercase key for the
    /// reason `open_companion_equip` gives. Unlike that one it leaves
    /// `menu_selected` alone: the manifest indexes nothing with it, and the
    /// parked row is what Esc comes back to when ←/→ has paged the sheet onto
    /// the player, who has no roster row of their own.
    fn open_companion_manifest(&mut self) {
        let row = self.menu_selected;
        let Some(game) = &mut self.game else { return };
        let Some(program) = game.owned_pets().get(row).map(|p| p.entity) else {
            return;
        };
        self.pending_manifest = Some(program);
        self.manifest_origin = ManifestOrigin::Roster;
        self.status_line = None;
        self.mode = Mode::Manifest;
    }

    /// Opens what the highlighted program remembers.
    ///
    /// Handled before `selected_index` and bound to an uppercase key for the
    /// reason `open_companion_equip` gives. Like `open_companion_manifest`
    /// and unlike that one it **leaves `menu_selected` alone**: the page
    /// indexes nothing with it, and the parked row is what Esc comes back to.
    fn open_companion_memories(&mut self) {
        let row = self.menu_selected;
        let Some(game) = &mut self.game else { return };
        let Some(program) = game.owned_pets().get(row).map(|p| p.entity) else {
            return;
        };
        self.pending_memory_program = Some(program);
        self.status_line = None;
        self.mode = Mode::CompanionMemories;
    }

    /// What `pending_memory_program` remembers. A page and not a menu —
    /// there is nothing on it to pick, so Esc is the only key it answers,
    /// the way `Mode::StructureManifest` and `Mode::CellDescribe` do.
    pub(crate) fn handle_companion_memories_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_memory_program = None;
            self.status_line = None;
            self.mode = Mode::Companion;
        }
    }

    /// The slot page for `pending_equip_program`: three rows, each opening
    /// the same `Mode::EquipSwap` picker the inventory uses, with the target
    /// set so the swap lands on the program rather than the player.
    pub(crate) fn handle_companion_equip_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_equip_program = None;
            self.status_line = None;
            self.mode = Mode::Companion;
            return;
        }
        let Some(program) = self.pending_equip_program else {
            self.mode = Mode::Companion;
            return;
        };
        // The same three rows the inventory leads with, so they answer `[I]`
        // the same way — about the program wearing them, not the player.
        if key == GameKey::Char('I') {
            let slot = EquipmentSlot::ALL[self.menu_selected.min(EquipmentSlot::ALL.len() - 1)];
            self.inspect_worn(Some(program), slot, Mode::CompanionEquip);
            return;
        }
        let Some(idx) = self.selected_index(key, EquipmentSlot::ALL.len()) else {
            return;
        };
        let slot = EquipmentSlot::ALL[idx];
        let Some(game) = &mut self.game else { return };
        let name = game
            .owned_pets()
            .into_iter()
            .find(|p| p.entity == program)
            .map(|p| p.name)
            .unwrap_or_else(|| "That program".to_string());
        // An occupied slot always offers to be emptied, so an empty picker
        // only arises for a bare slot with nothing in cargo that fits — a
        // dead end, reported here the way the player's own picker reports it.
        if equip_swap_rows(game, program, slot).is_empty() {
            let line = format!("Nothing in cargo fits {name}'s {} slot.", slot.label());
            self.refuse(line);
            return;
        }
        self.pending_swap_slot = Some(slot);
        self.pending_swap_target = Some(program);
        self.menu_selected = 0;
        self.mode = Mode::EquipSwap;
    }

    fn begin_rename(&mut self) {
        let row = self.menu_selected;
        let Some(game) = &mut self.game else { return };
        let Some(pet) = game.owned_pets().get(row).map(|p| p.entity) else {
            return;
        };
        self.rename_input = game.custom_name(pet).unwrap_or_default();
        self.pending_rename = Some(pet);
        self.mode = Mode::RenamePet;
    }

    /// Takes the highlighted program in hand as a weapon, or puts it down if
    /// it is the one already there.
    ///
    /// Handled before `selected_index` the way `<` and `>` are, and bound to
    /// an uppercase key on purpose: uppercase reaches app-core as a distinct
    /// key and is already used that way elsewhere, so it can never collide
    /// with `menu_shortcut`'s digits-then-lowercase scheme however large the
    /// roster grows.
    ///
    /// **Nothing on the screen names this key.** That omission is the whole
    /// feature — see `render::party`'s `COMPANION_HELP`, which a gui test
    /// holds to it.
    fn toggle_wield(&mut self) {
        let row = self.menu_selected;
        let Some(game) = &mut self.game else { return };
        let Some(pet) = game.owned_pets().get(row).map(|p| (p.entity, p.wielded)) else {
            return;
        };
        let result = if pet.1 {
            game.unwield_program()
        } else {
            game.wield_program(pet.0)
        };
        self.status_line = result.err();
    }

    /// Shifts the highlighted program one slot along the battle line, and
    /// moves the highlight with it — so repeated presses walk one member to
    /// the front rather than swapping the same pair back and forth. Safe to
    /// compute the new row arithmetically: `owned_pets` leads with the party
    /// in slot order, and the swap stays inside that prefix.
    fn shift_party_slot(&mut self, shift: SlotShift) {
        let row = self.menu_selected;
        let Some(game) = &mut self.game else { return };
        let Some(entity) = game.owned_pets().get(row).map(|p| p.entity) else {
            return;
        };
        match game.move_party_member(entity, shift) {
            Ok(()) => {
                self.status_line = None;
                self.menu_selected = match shift {
                    SlotShift::Forward => row.saturating_sub(1),
                    SlotShift::Back => row + 1,
                };
            }
            Err(e) => self.status_line = Some(e),
        }
    }

    /// Picks the first of two tamed programs to fuse together. Draws from
    /// the whole roster rather than what's within `MENU_SCAN_RADIUS`:
    /// `Game::fuse_companions` has no distance requirement, so scanning by
    /// distance only hid programs left working across the map.
    pub(crate) fn handle_fuse_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let Some(game) = &mut self.game else { return };
        let candidates = game.owned_pets();
        if let Some(idx) = self.selected_index(key, candidates.len()) {
            self.pending_fuse_first = Some(candidates[idx].entity);
            self.mode = Mode::FuseSecond;
        }
    }

    /// Picks which program to develop, then goes on to the one page that
    /// spends on it. Two pages for the same reason the refactor flow has two:
    /// which program is a separate decision from what to do to it.
    pub(crate) fn handle_develop_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let Some(game) = &mut self.game else { return };
        let programs = game.owned_pets();
        if let Some(idx) = self.selected_index(key, programs.len()) {
            self.pending_develop_target = Some(programs[idx].entity);
            self.mode = Mode::DevelopProgram;
        }
    }

    /// Opens the next Kernel Ring on the chosen program and stays on this
    /// page, the way `handle_refactor_item_key` does: a player opening two
    /// rings in a row should not be sent back to pick the program again, and
    /// the page is where the ceiling it bought is shown. A refusal lands in
    /// the status line and the page holds.
    pub(crate) fn handle_develop_program_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_develop_target = None;
            self.mode = Mode::Develop;
            return;
        }
        let Some(target) = self.pending_develop_target else {
            self.mode = Mode::Develop;
            return;
        };
        if matches!(key, GameKey::Char('r') | GameKey::Char('R')) {
            let Some(game) = &mut self.game else { return };
            let outcome = game.open_kernel_ring(target);
            self.report(outcome);
            return;
        }
        // The ladder's numbered rows are the next untaken tier's choices —
        // the same rows the screen draws — so the row the player is looking at
        // and the row this buys cannot disagree. Whether they can *afford* one
        // is deliberately not asked here: `take_talent` refuses and says where
        // points come from, and a second copy of that sentence in app-core is
        // the drift `ability_unavailable`'s row fragments already cost once.
        let rows = self
            .game
            .as_mut()
            .map(|g| g.talent_options(target))
            .unwrap_or_default();
        let next_tier = rows
            .iter()
            .map(|r| r.tier)
            .find(|tier| !rows.iter().any(|r| r.tier == *tier && r.taken));
        let offered: Vec<_> = rows
            .into_iter()
            .filter(|r| Some(r.tier) == next_tier)
            .map(|r| r.id)
            .collect();
        if let Some(idx) = self.selected_index(key, offered.len()) {
            let Some(game) = &mut self.game else { return };
            let outcome = game.take_talent(target, &offered[idx]);
            self.report(outcome);
            // The list the highlight was drawn against just moved on to the
            // next tier, and the mode has not changed, so `handle_key`'s own
            // reset never fires — the same trap `handle_refactor_item_key`
            // clamps for, and a talent is just as permanent as an upgrade.
            self.menu_selected = 0;
        }
    }

    /// Picks which program to permanently upgrade, then goes on to what to
    /// spend on it. Two pages rather than one, mirroring the routine install
    /// flow: which program and which upgrade are separate decisions, and the
    /// second list is short enough to read only once the first is made.
    pub(crate) fn handle_refactor_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let Some(game) = &mut self.game else { return };
        let programs = game.owned_pets();
        if let Some(idx) = self.selected_index(key, programs.len()) {
            self.pending_refactor_target = Some(programs[idx].entity);
            self.mode = Mode::RefactorItem;
        }
    }

    /// Spends the chosen upgrade and stays on this page, so a player working
    /// through five slots on one program is not sent back to pick it again
    /// each time. A refusal lands in the status line and the page holds.
    pub(crate) fn handle_refactor_item_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_refactor_target = None;
            self.mode = Mode::Refactor;
            return;
        }
        let Some(target) = self.pending_refactor_target else {
            self.mode = Mode::Refactor;
            return;
        };
        let Some(offered) = self.game.as_ref().map(|g| g.companion_upgrades()) else {
            return;
        };
        if let Some(idx) = self.selected_index(key, offered.len()) {
            let item = offered[idx].item.clone();
            let Some(game) = &mut self.game else { return };
            let outcome = game.refactor_companion(target, &item);
            self.report(outcome);
            let left = self
                .game
                .as_ref()
                .map(|g| g.companion_upgrades().len())
                .unwrap_or(0);
            if left == 0 {
                // Nothing left to pick anywhere in the flow. Backing out one
                // page looks safe and is not: the program picker is still
                // fully populated, and every row on it opens this page with
                // no rows on it — the blank screen backing out was meant to
                // avoid.
                self.pending_refactor_target = None;
                self.close_screen();
            } else {
                // The list just shrank under the highlight. `selected_index`
                // resolves Enter to `menu_selected.min(len - 1)` and the mode
                // has not changed, so `handle_key`'s own reset never fires —
                // a highlight left past the end means the next Enter spends a
                // permanent, irreversible upgrade the player was not looking
                // at. `handle_trade_key` clamps for the same reason.
                self.menu_selected = self.menu_selected.min(left - 1);
            }
        }
    }

    /// Picks the second program to fuse with the one from `handle_fuse_key`,
    /// then actually runs the fusion.
    pub(crate) fn handle_fuse_second_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_fuse_first = None;
            self.close_screen();
            return;
        }
        let Some(first) = self.pending_fuse_first else {
            self.mode = Mode::Playing;
            return;
        };
        let Some(game) = &mut self.game else { return };
        let candidates: Vec<_> = game
            .owned_pets()
            .into_iter()
            .filter(|p| p.entity != first)
            .collect();
        if let Some(idx) = self.selected_index(key, candidates.len()) {
            self.pending_fuse_second = Some(candidates[idx].entity);
            self.fuse_name_input.clear();
            self.mode = Mode::FuseName;
        }
    }

    /// Types a name (up to `feral_processes_engine::MAX_CUSTOM_NAME_LEN`
    /// characters) for the program that'll result from fusing
    /// `pending_fuse_first`/`pending_fuse_second`; Enter runs the fusion
    /// (blank keeps the default species name). Esc backs up one step to
    /// re-pick the second program, rather than aborting the whole fusion —
    /// the first pick is still good.
    pub(crate) fn handle_fuse_name_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc => {
                self.pending_fuse_second = None;
                self.fuse_name_input.clear();
                self.mode = Mode::FuseSecond;
            }
            GameKey::Backspace => {
                self.fuse_name_input.pop();
            }
            GameKey::Char(c)
                if !c.is_control()
                    && self.fuse_name_input.chars().count()
                        < feral_processes_engine::MAX_CUSTOM_NAME_LEN =>
            {
                self.fuse_name_input.push(c);
            }
            GameKey::Enter => {
                let (Some(first), Some(second)) = (
                    self.pending_fuse_first.take(),
                    self.pending_fuse_second.take(),
                ) else {
                    self.mode = Mode::Playing;
                    return;
                };
                let name = (!self.fuse_name_input.is_empty()).then(|| self.fuse_name_input.clone());
                self.fuse_name_input.clear();
                let Some(game) = &mut self.game else { return };
                let outcome = game.fuse_companions(first, second, name);
                self.report(outcome);
                self.mode = Mode::Playing;
            }
            _ => {}
        }
    }
}
