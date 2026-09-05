//! `Mode::Tools`: the tool kit, reached from the party menu — see
//! `docs/superpowers/specs/2026-09-04-program-extraction-design.md`,
//! section 6. A flat list, `App::menu_selected` naming the highlighted row —
//! there is no per-row drill-down page the way `Mode::DownedPrograms` has
//! one, only the three verbs a row itself takes.

use crate::*;

impl App {
    /// `Esc` returns to the party menu through `App::close_screen` — this is
    /// only ever opened through the group menu, which already set
    /// `menu_origin`, so there is no `Mode::DownedPrograms`-style special
    /// case for a screen reached a second way.
    ///
    /// `F`/`I`/`X` act on the highlighted row in place, rather than picking
    /// it first the way a digit key does on every other screen: uppercase,
    /// `selected_index`'s reserved half, so an action key can never also
    /// move the highlight on the same keypress. Anything else is ordinary
    /// row selection.
    ///
    /// Row count comes from `Game::tool_rows` on every keypress rather than
    /// being cached — `handle_downed_programs_key`'s own reason: a store
    /// that shrank under the player (a slot pulled, a carrier spent
    /// elsewhere) cannot be indexed past its own end.
    pub(crate) fn handle_tools_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        if let GameKey::Char(action @ ('F' | 'I' | 'X')) = key {
            self.act_on_tool_row(action);
            return;
        }
        let Some(len) = self.game.as_ref().map(|g| g.tool_rows().len()) else {
            return;
        };
        if let Some(idx) = self.selected_index(key, len) {
            self.menu_selected = idx;
        }
    }

    /// `action` is one of `F`/`I`/`X`, guarded by the caller's pattern.
    /// `X` names the slot rather than the id — `Game::uninstall_tool`'s own
    /// signature — and a highlighted row with nothing installed passes
    /// `usize::MAX`, which lands on the same "That slot is empty." refusal
    /// a stale index gets rather than a bespoke message.
    fn act_on_tool_row(&mut self, action: char) {
        let Some(game) = self.game.as_ref() else {
            return;
        };
        let Some(row) = game.tool_rows().into_iter().nth(self.menu_selected) else {
            return;
        };
        let id = row.id;
        let slot = row.slot;
        let outcome = match action {
            'F' => self.game.as_mut().unwrap().forge_tool(&id),
            'I' => self.game.as_mut().unwrap().install_tool(&id),
            'X' => self
                .game
                .as_mut()
                .unwrap()
                .uninstall_tool(slot.unwrap_or(usize::MAX)),
            _ => unreachable!("guarded by handle_tools_key's pattern"),
        };
        self.report(outcome);
    }
}
