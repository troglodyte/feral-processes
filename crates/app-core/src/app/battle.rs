//! The intrusion screens: the action roster and the pickers layered over
//! it, from a key press to a committed `BattleAction`.

use crate::*;

impl App {
    /// Shared tail of every battle-turn key handler: queues the sound for
    /// the action just taken, then — once `check_game_over` has had a
    /// chance to move `self.mode` to `Mode::GameOver` — queues whichever
    /// outcome sound actually applies. A `Flee` action never gets a
    /// `Victory` sound layered on top of it even though it also ends the
    /// battle, since jacking out isn't a win.
    fn push_battle_outcome_sounds(&mut self, action_sound: SoundEvent, still_active: bool) {
        self.pending_sounds.push(action_sound);
        self.check_game_over();
        if self.mode == Mode::GameOver {
            self.pending_sounds.push(SoundEvent::Defeat);
        } else if !still_active && action_sound != SoundEvent::Flee {
            self.pending_sounds.push(SoundEvent::Victory);
        }
    }

    /// Chooses this round's action for the active party slot, or runs a
    /// party-level command. Every key comes from `Game::battle_action_options`
    /// or `Game::battle_party_commands`, so adding a battle action needs no
    /// change here and the two renderers cannot drift apart.
    pub(crate) fn handle_battle_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.battle_back_up();
            return;
        }
        let GameKey::Char(raw) = key else { return };

        // Undocumented on purpose — see `crates/engine/EASTER_EGGS.md`.
        // Intercepted here, ahead of everything, and matched on the raw char
        // rather than the folded one, exactly as `W` rides ahead of
        // `selected_index` on the companion screen. Both stages below were
        // checked rather than assumed to be free: party commands are
        // `A`/`D`/`j` and per-slot actions are `a`/`d`/`s`/`u`, so neither
        // `T` nor the `t` the case-folding retry would look for hits
        // anything. The refusal is unreachable — this handler only runs in
        // `Mode::Battle` — so there is nothing to put on the status line.
        if raw == 'T' {
            if let Some(game) = &mut self.game {
                let _ = game.taunt();
            }
            return;
        }

        // Party-wide commands are matched on the raw char first, so uppercase
        // `A`/`D` stay distinct from the lowercase per-slot Attack/Defend. The
        // lowercase retry is what lets a shifted `J` still jack out; `a`/`d`
        // can't match it, so they fall through to the per-slot menu below.
        let party = self
            .game
            .as_ref()
            .map(|g| g.battle_party_commands())
            .unwrap_or_default();
        if let Some(command) = party
            .iter()
            .find(|c| c.key == raw)
            .or_else(|| party.iter().find(|c| c.key == raw.to_ascii_lowercase()))
        {
            self.run_party_command(command.kind, command.needs_target);
            return;
        }

        // Every other battle key folds case: the prompt's bracketed letter is
        // lowercase, so a shifted press is a slip, and swallowing it would
        // cost the player a round.
        let c = raw.to_ascii_lowercase();

        let Some(game) = &self.game else { return };
        let Some(slot) = game.battle_active_slot() else {
            return;
        };
        let Some(option) = game
            .battle_action_options(slot)
            .into_iter()
            .find(|o| o.key == c)
        else {
            return;
        };
        if let Some(reason) = option.unavailable {
            self.status_line = Some(format!("Can't do that — {reason}."));
            return;
        }
        match option.target {
            TargetSpec::None => {
                if let Some(action) = action_from(option.kind, Collected::default()) {
                    self.commit_battle_action(slot, action);
                }
            }
            TargetSpec::EnemyGroup => {
                self.pending_battle_action = Some(option.kind);
                self.menu_selected = 0;
                self.mode = Mode::BattleTarget;
            }
            TargetSpec::SpecialAbility => {
                self.pending_battle_action = Some(option.kind);
                self.pending_special_ability = None;
                self.menu_selected = 0;
                self.mode = Mode::BattleSpecial;
            }
            TargetSpec::InventoryItem => {
                self.pending_battle_action = Some(option.kind);
                self.menu_selected = 0;
                self.mode = Mode::BattleItem;
            }
        }
    }

    /// Runs a party-level command: jacking out, or planning every open slot
    /// at once. All-attack defers to `Mode::BattleTarget` when the engine
    /// says there is more than one group to choose between.
    fn run_party_command(&mut self, kind: PartyCommandKind, needs_target: bool) {
        match kind {
            PartyCommandKind::JackOut => {
                // Bound inside the arm, not before the match: the other two
                // arms call `&mut self` methods, and a `game` borrow held
                // across the match would collide with them.
                let Some(game) = &mut self.game else { return };
                let escaped = game.battle_flee();
                let still_active = game.has_active_battle();
                // Unconditionally: a *refused* flee still resolves a round
                // (`battle_flee` retaliates, bumps `round` and ticks), so
                // the tail has to see it. Harmless while the battle is
                // live — the mode is already `Mode::Battle` and the reveal
                // restart is guarded on the fight being over.
                self.settle_after_round(still_active);
                // A pinned attempt is not an escape: what the player hears
                // is the volley it drew, not the jack-out that didn't
                // happen. The engine logs why, so there's nothing to add to
                // the status line.
                let sound = if escaped {
                    SoundEvent::Flee
                } else {
                    SoundEvent::Attack
                };
                self.push_battle_outcome_sounds(sound, still_active);
            }
            PartyCommandKind::AllDefend => self.plan_every_slot(BattleAction::Defend),
            PartyCommandKind::AllAttack => {
                if needs_target {
                    self.pending_party_attack = true;
                    self.menu_selected = 0;
                    self.mode = Mode::BattleTarget;
                } else {
                    self.plan_every_slot(BattleAction::Attack { group: 0 });
                }
            }
        }
    }

    /// Plans every open slot with `action` and resolves the round. The
    /// resolve-and-transition tail matches `commit_battle_action`'s, since a
    /// full party is a full party however it got there.
    fn plan_every_slot(&mut self, action: BattleAction) {
        let Some(game) = &mut self.game else { return };
        if let Err(reason) = game.battle_plan_remaining(action) {
            self.status_line = Some(reason);
            return;
        }
        if !game.battle_round_ready() {
            self.mode = Mode::Battle;
            return;
        }
        game.battle_resolve_round();
        let still_active = game.has_active_battle();
        self.settle_after_round(still_active);
        self.push_battle_outcome_sounds(SoundEvent::Attack, still_active);
    }

    /// Picks which enemy group the action chosen in `Mode::Battle` hits —
    /// either one slot's, or the party-wide `[A]ll attack`.
    pub(crate) fn handle_battle_target_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            // A Special got here through the ability picker, so backing out
            // returns there rather than discarding both choices at once.
            if self.pending_special_ability.take().is_some() {
                self.menu_selected = 0;
                self.mode = Mode::BattleSpecial;
                return;
            }
            self.pending_battle_action = None;
            self.pending_party_attack = false;
            self.mode = Mode::Battle;
            return;
        }
        let Some(game) = &self.game else { return };
        let Some(view) = game.battle_view() else {
            return;
        };
        let active_slot = view.active_slot;
        let group_count = view.groups.len();
        // Groups are addressed by letter on screen, so accept the letter as
        // well as the row number `selected_index` already handles.
        let picked = match key {
            GameKey::Char(c) if c.is_ascii_alphabetic() => {
                let idx = (c.to_ascii_lowercase() as u8 - b'a') as usize;
                (idx < group_count).then_some(idx)
            }
            other => self.selected_index(other, group_count),
        };
        let Some(group) = picked else { return };
        // Checked before `active_slot`: a party-wide fill plans every open
        // slot, so it has no one slot to be waiting on.
        if self.pending_party_attack {
            self.pending_party_attack = false;
            self.plan_every_slot(BattleAction::Attack { group });
            return;
        }
        let Some(slot) = active_slot else { return };
        let Some(kind) = self.pending_battle_action else {
            return;
        };
        let Some(action) = action_from(
            kind,
            Collected {
                group: Some(group),
                ability: self.pending_special_ability,
                ..Collected::default()
            },
        ) else {
            return;
        };
        self.pending_battle_action = None;
        self.pending_special_ability = None;
        self.commit_battle_action(slot, action);
    }

    /// Title for the target picker, naming the action being aimed — e.g.
    /// `Pick a target (Heal)`. Lives here rather than in either renderer so
    /// the two can't drift, and reads every name out of the engine's own
    /// option lists rather than authoring one.
    pub fn battle_target_title(&self) -> String {
        let plain = || "Pick a target".to_string();
        let (Some(game), Some(kind)) = (&self.game, self.pending_battle_action) else {
            return plain();
        };
        let Some(slot) = game.battle_active_slot() else {
            return plain();
        };
        // A Special is named by the ability picked a step earlier, not by
        // the generic "[s]pecial" the action menu shows.
        let name = if kind == ActionKind::Special {
            self.pending_special_ability.and_then(|i| {
                game.battle_special_options(slot)
                    .into_iter()
                    .find(|o| o.index == i)
                    .map(|o| o.name)
            })
        } else {
            game.battle_action_options(slot)
                .into_iter()
                .find(|o| o.kind == kind)
                .map(|o| o.label.replace(['[', ']'], ""))
        };
        match name {
            Some(name) => format!("Pick a target ({name})"),
            None => plain(),
        }
    }

    /// Picks which of the acting member's abilities a Special spends, then
    /// hands off to `Mode::BattleTarget` for who it lands on.
    pub(crate) fn handle_battle_special_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_battle_action = None;
            self.pending_special_ability = None;
            self.mode = Mode::Battle;
            return;
        }
        let Some(game) = &self.game else { return };
        let Some(slot) = game.battle_active_slot() else {
            return;
        };
        let options = game.battle_special_options(slot);
        let Some(idx) = self.selected_index(key, options.len()) else {
            return;
        };
        let chosen = &options[idx];
        // Which picker comes next is the ability's own business, read off the
        // engine's option rather than decided here.
        self.pending_special_ability = Some(chosen.index);
        self.menu_selected = 0;
        match chosen.targeting {
            SpecialTargeting::Ally => self.mode = Mode::BattleAlly,
            SpecialTargeting::Enemy => self.mode = Mode::BattleTarget,
            // Nothing left to choose — commit the action now rather than
            // opening a picker with one meaningless row. Which side it
            // sweeps is the engine's answer, not a guess made here.
            SpecialTargeting::None => {
                let target = if chosen.sweeps_party {
                    SpecialTarget::WholeParty
                } else {
                    SpecialTarget::AllEnemies
                };
                let action = BattleAction::Special {
                    ability: chosen.index,
                    target,
                };
                self.pending_battle_action = None;
                self.pending_special_ability = None;
                self.commit_battle_action(slot, action);
            }
        }
    }

    /// Picks which party member a buff or heal lands on, completing a
    /// party-facing Special.
    pub(crate) fn handle_battle_ally_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_special_ability = None;
            self.menu_selected = 0;
            self.mode = Mode::BattleSpecial;
            return;
        }
        let Some(game) = &self.game else { return };
        let Some(slot) = game.battle_active_slot() else {
            return;
        };
        let allies = game.battle_ally_options();
        let Some(idx) = self.selected_index(key, allies.len()) else {
            return;
        };
        let Some(kind) = self.pending_battle_action else {
            return;
        };
        let Some(action) = action_from(
            kind,
            Collected {
                ability: self.pending_special_ability,
                ally: Some(allies[idx].slot),
                ..Collected::default()
            },
        ) else {
            return;
        };
        self.pending_battle_action = None;
        self.pending_special_ability = None;
        self.commit_battle_action(slot, action);
    }

    /// Picks which consumable the action chosen in `Mode::Battle` spends.
    pub(crate) fn handle_battle_item_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_battle_action = None;
            self.mode = Mode::Battle;
            return;
        }
        let Some(game) = &self.game else { return };
        let Some(slot) = game.battle_active_slot() else {
            return;
        };
        let items = game.battle_usable_items();

        // Undocumented on purpose — see `crates/engine/EASTER_EGGS.md`.
        // Ahead of `selected_index`, which is where an uppercase key has to
        // go: that function rejects every non-lowercase char precisely so a
        // screen action can own the uppercase space, however long the list
        // grows. Sharing the letter with the roster's taunt is intended —
        // the two handlers can never both fire, and one letter meaning "do
        // the reckless thing" on both battle screens is easier to remember
        // than two.
        if key == GameKey::Char('T') {
            let thrown = items.get(self.menu_selected.min(items.len().saturating_sub(1)));
            let thrown = thrown.cloned();
            self.pending_battle_action = None;
            self.mode = Mode::Battle;
            if let (Some(item), Some(game)) = (thrown, self.game.as_mut())
                && let Err(reason) = game.throw_item(&item)
            {
                self.status_line = Some(reason);
            }
            return;
        }

        let Some(idx) = self.selected_index(key, items.len()) else {
            return;
        };
        let Some(kind) = self.pending_battle_action else {
            return;
        };
        let Some(action) = action_from(
            kind,
            Collected {
                item: Some(items[idx].clone()),
                ..Collected::default()
            },
        ) else {
            return;
        };
        self.pending_battle_action = None;
        self.commit_battle_action(slot, action);
    }

    /// Records `action` for `slot`, and resolves the round once every slot
    /// has one.
    pub(crate) fn commit_battle_action(&mut self, slot: usize, action: BattleAction) {
        let Some(game) = &mut self.game else { return };
        if let Err(reason) = game.battle_set_action(slot, action) {
            self.status_line = Some(reason);
            // Back to the roster even on failure. Whichever picker led here
            // cleared its pending action before calling, so leaving the
            // popup up strands the player: every row bails on the missing
            // pending state, making Esc the only way out.
            self.mode = Mode::Battle;
            return;
        }
        self.mode = Mode::Battle;
        if !game.battle_round_ready() {
            return;
        }
        game.battle_resolve_round();
        let still_active = game.has_active_battle();
        self.settle_after_round(still_active);
        self.push_battle_outcome_sounds(SoundEvent::Attack, still_active);
    }

    /// The shared tail of every action that can end a battle.
    ///
    /// A battle that just ended has had its log pruned to results by the
    /// engine, so the reveal restarts and those results scroll into the map's
    /// log pane rather than appearing whole.
    ///
    /// It is also the one hook an arena fight is read through: the round is
    /// handed to the session's `Watch` here, so the played fight and the
    /// headless one share the same answer to what a fight cost.
    fn settle_after_round(&mut self, still_active: bool) {
        self.observe_arena_round();
        if !still_active && self.in_arena() {
            self.finish_arena_fight();
            return;
        }
        self.mode = if still_active {
            Mode::Battle
        } else {
            Mode::BattleResult
        };
        if !still_active {
            self.restart_reveal();
        }
    }

    /// Any key leaves the results page for the map. Deliberately not a
    /// menu: there is nothing to choose, and a key pressed while the
    /// results are still scrolling in has already been spent skipping the
    /// reveal by `handle_key`'s own guard — so the player reads them either
    /// way and never dismisses loot they never saw.
    pub(crate) fn handle_battle_result_key(&mut self, _key: GameKey) {
        self.mode = Mode::Playing;
    }

    /// Backs the planning cursor up one slot, so a misclick can be undone.
    /// Clearing a slot also clears every slot after it — the player chose
    /// those in light of the choice they're now taking back.
    fn battle_back_up(&mut self) {
        let Some(game) = &mut self.game else { return };
        let Some(slot) = game.battle_active_slot() else {
            return;
        };
        if slot == 0 {
            self.status_line = Some("Nothing to undo — pick an action, or [j]ack out.".to_string());
            return;
        }
        game.battle_clear_action(slot - 1);
    }
}
