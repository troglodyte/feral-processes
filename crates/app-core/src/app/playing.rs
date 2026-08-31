//! The map screen — movement, and the keys that open every other mode.

use crate::DEV_CONSOLE_KEY;
use crate::*;

/// One step, reported honestly: `true` only when the world actually moved.
///
/// `Game::move_player` returns nothing, and on the zone surface assuming an
/// action was fine — every step there spends a turn, a bounce off a wall
/// included. Base space did not work that way when it shipped: a step into
/// solid rock was refused outright and cost nothing, so reporting it as an
/// action would have cleared the status line explaining an earlier refusal
/// and queued a footstep for a step that never happened. Slice 2 turned that
/// bounce into a swing, which does spend a turn — and this function needed
/// no edit for it, which is the whole argument for reading the clock instead
/// of assuming per locale.
///
/// The clock is what all three locales agree on, so that is what this reads.
/// The game-over clause is the one case a real action leaves the clock
/// standing still: ground that kills you sets `GameOver` before
/// `Game::tick` runs and `tick` then returns without advancing — but
/// `App::after_world_action` still has to see an action, or the run would
/// never reach the death screen.
fn stepped(game: &mut Game, dx: i32, dy: i32) -> bool {
    let before = game.current_tick();
    game.move_player(dx, dy);
    game.current_tick() > before || game.is_game_over().is_some()
}

/// Which keys hand the map's camera back to the party — see `App::watching`.
///
/// Esc is the advertised way out. The eight movement keys are here because
/// walking while the camera sits somewhere else is walking blind, and the
/// player who presses a direction has already stopped watching whether or
/// not they thought of it that way.
fn releases_the_camera(key: GameKey) -> bool {
    matches!(
        key,
        GameKey::Esc
            | GameKey::Up
            | GameKey::Down
            | GameKey::Left
            | GameKey::Right
            | GameKey::Char('k')
            | GameKey::Char('j')
            | GameKey::Char('h')
            | GameKey::Char('l')
    )
}

impl App {
    pub(crate) fn handle_playing_key(&mut self, key: GameKey) {
        // Watching is a camera, not a mode: every other key still does
        // exactly what it does, which is what keeps this from needing a
        // refusal path of its own. Esc consumes the press, having nothing
        // else to do on this screen; a step releases and **still steps**,
        // because a swallowed movement key reads as the game having frozen.
        if self.watching.is_some() && releases_the_camera(key) {
            self.watching = None;
            if key == GameKey::Esc {
                return;
            }
        }
        match key {
            // The two group menus. Seventeen keys used to sit on this
            // screen doing what these two now reach; see `group_menu.rs`
            // for why none of them survive as an alias.
            // Dev-only, and inert unless FERAL_DEV_CONSOLE opened the gate.
            GameKey::Char(c) if c == DEV_CONSOLE_KEY && self.dev_console_enabled() => {
                self.mode = Mode::DevConsole;
                self.menu_selected = 0;
                return;
            }
            GameKey::Char('b') => {
                self.mode = Mode::BaseMenu;
                return;
            }
            GameKey::Char('p') => {
                self.mode = Mode::PartyMenu;
                return;
            }
            // No menu of its own: the pack is a single screen, so the group
            // key opens it directly.
            GameKey::Char('i') => {
                self.mode = Mode::Inventory;
                return;
            }
            GameKey::Char('u') => {
                self.mode = Mode::Symlink;
                return;
            }
            // Examine, in the roguelike sense — `i` went to the pack, and
            // perks moving into the party menu freed the key a player would
            // guess for it anyway.
            GameKey::Char('x') => {
                self.mode = Mode::InspectDirection;
                return;
            }
            // Demolish, aimed rather than picked from a list. Refused here
            // rather than in the direction handler because the prompt would
            // otherwise open onto four directions that mean nothing: the
            // structures are all in base space, and outside it the player's
            // `Position` is either their tile on the open grid or pinned to
            // the Stack entrance. Matches the `base_only` flag on the group
            // menu's Demolish row, which hides that route for the same
            // reason, and `Game::remove_structure`'s own `require_base`.
            GameKey::Char('d') => {
                if self.game.as_ref().is_some_and(|g| g.in_base()) {
                    self.mode = Mode::RemoveDirection;
                } else {
                    self.refuse(
                        "Nothing of yours to demolish here — the base is through the anchor.",
                    );
                }
                return;
            }
            // Flat despite belonging to the party, like `c` and `t` below:
            // these three are pressed every few turns while walking, and a
            // group menu is a keystroke tax on anything that frequent.
            GameKey::Char('a') => {
                self.mode = Mode::FieldRoutine;
                return;
            }
            // The Excavation plan. Refused out here rather than in the mode
            // itself for the reason `d` above is: the cursor would open over
            // the player's tile on the open grid, which is a coordinate in a
            // different space entirely, and every cell it could reach would
            // refuse a mark. `Game::toggle_mark_box` has no locale guard of
            // its own because nothing but this mode calls it.
            GameKey::Char('m') => {
                match self.game.as_ref().and_then(|g| g.base_pos()) {
                    Some(party) => {
                        self.excavate_cursor = Some(party);
                        self.excavate_anchor = None;
                        self.mode = Mode::Excavate;
                    }
                    None => self
                        .refuse("Nothing to excavate out here — the rock is through the anchor."),
                }
                return;
            }
            GameKey::Char('t') => {
                // Underground the same key opens whoever is selling *here*.
                // The trader list would otherwise scan from a `Position`
                // pinned to the surface entrance tile and offer to trade
                // with a base four frames overhead — the same hole the
                // `base_only` group-menu rows and the `d` refusal above are
                // each closing at their own door.
                if self
                    .game
                    .as_mut()
                    .is_some_and(|g| g.stack_market().is_some())
                {
                    self.menu_selected = 0;
                    self.mode = Mode::StackMarket;
                    return;
                }
                if self.game.as_ref().is_some_and(|g| g.is_underground()) {
                    self.refuse("There's nobody selling anything here.");
                    return;
                }
                // Closed the same way `d` above is: `Game::view_entities`
                // now refuses to answer a `Structure` query outside base
                // space (every trader is one), so the list this used to
                // open on the open grid would always come back empty —
                // Task 5 left it noted rather than fixed in `docs/seams.md`,
                // and this is the guard flip that note asked for. Both
                // remaining branches are surface-or-base only, `is_underground`
                // having already returned above, so `in_base` cannot collide
                // with the Stack case handled there.
                if !self.game.as_ref().is_some_and(|g| g.in_base()) {
                    self.refuse("Nobody's selling out here — the traders are through the anchor.");
                    return;
                }
                // Opening the trader list from the map is a fresh visit, not
                // the tail of a sale begun in the inventory.
                self.trade_origin = TradeOrigin::Trader;
                self.pending_trade_choice = None;
                self.mode = Mode::Trade;
                return;
            }
            // Arms the player's own bump into base-space rock. A `return`
            // rather than a fallthrough for `f`'s reason: picking a tool up
            // is not an action and must not cost a turn.
            //
            // Refused out here rather than in the engine for the reason `d`
            // and `m` are: outside base space there is no rock to cut, and
            // arming a tool against ground that has none is a keypress with
            // nothing to show for it.
            GameKey::Char('n') => {
                match self.game.as_mut() {
                    Some(game) if game.in_base() => {
                        game.toggle_mining();
                    }
                    _ => self.refuse("Nothing to cut out here — the rock is through the anchor."),
                }
                return;
            }
            GameKey::Char('L') => {
                self.mode = Mode::History;
                return;
            }
            // Next to `L` because it acts on the same log, and a `return`
            // rather than a fallthrough because changing what you can see
            // must not cost a turn. History keeps the only uppercase letter
            // left on this screen: `l` walks east.
            GameKey::Char('f') => {
                self.log_filter = self.log_filter.next();
                return;
            }
            // Doubles the log pane's height and back. A `return` for `f`'s
            // reason: resizing the pane you are reading is not an action.
            // Bound up here, beside the digits below and for the same
            // reason: this match runs before the hand-off to
            // `handle_stack_key`, so the toggle already reaches both
            // locales without a second arm down there to drift from this
            // one — and the log pane it resizes is drawn on the surface and
            // in the Stack view alike.
            GameKey::Char(' ') => {
                self.log_expanded = !self.log_expanded;
                return;
            }
            // The info column's three panes. A `return` for `f`'s reason:
            // changing which pane you are reading is not an action and must
            // not cost a turn.
            //
            // **One binding covers both locales.** This match runs before
            // the `is_underground()` hand-off to `handle_stack_key` below,
            // so there is no second arm to write down there — and there must
            // not be, or the two would drift. The design spec asks for two
            // bindings; what it is actually asking for is that the keys work
            // underground, which `the_digits_work_underground` is what says.
            // A key that reached `handle_stack_key` instead would fall
            // through its `_ => {}` as a swallowed keypress with no refusal
            // and nothing in the log, which is how `r` shipped broken.
            GameKey::Char('1') => {
                self.info_tab = InfoTab::Base;
                return;
            }
            GameKey::Char('2') => {
                self.info_tab = InfoTab::Crew;
                return;
            }
            GameKey::Char('3') => {
                self.info_tab = InfoTab::Pack;
                return;
            }
            GameKey::Char('4') => {
                self.info_tab = InfoTab::Contracts;
                return;
            }
            GameKey::Char('s') => {
                self.save_game();
                return;
            }
            GameKey::Char('q') => {
                self.mode = Mode::QuitRunConfirm;
                return;
            }
            GameKey::Char('?') => {
                self.mode = Mode::Help;
                return;
            }
            // Whichever map is on screen: underground the zone map is not
            // drawn at all, so resizing its tiles from down there would be
            // a keypress with nothing to show for it.
            GameKey::Char('+') | GameKey::Char('=') => {
                if self.game.as_ref().is_some_and(|g| g.is_underground()) {
                    self.stack_zoom = (self.stack_zoom + 1).min(STACK_MAP_MAX_ZOOM);
                } else {
                    self.zoom = (self.zoom + 1).min(MAX_ZOOM);
                }
                return;
            }
            GameKey::Char('-') | GameKey::Char('_') => {
                if self.game.as_ref().is_some_and(|g| g.is_underground()) {
                    self.stack_zoom = self.stack_zoom.saturating_sub(1).max(STACK_MAP_MIN_ZOOM);
                } else {
                    self.zoom = self.zoom.saturating_sub(1).max(MIN_ZOOM);
                }
                return;
            }
            _ => {}
        }

        let is_move_key = matches!(
            key,
            GameKey::Up
                | GameKey::Down
                | GameKey::Left
                | GameKey::Right
                | GameKey::Char('k')
                | GameKey::Char('j')
                | GameKey::Char('h')
                | GameKey::Char('l')
        );
        // Underground the same four keys steer a party that has a facing:
        // forward, back, and turn in place. Deliberately the same keys rather
        // than a separate set — walking is walking, and the view makes which
        // one you're doing obvious.
        if self.game.as_ref().is_some_and(|g| g.is_underground()) {
            self.handle_stack_key(key, is_move_key);
            return;
        }

        // Set after the `self.game` borrow below releases, exactly as the
        // Stack path does it: a refusal is not an action, so it leaves
        // `acted` false and `after_world_action` returns before it can clear
        // the line that explains why.
        let mut refusal = None;
        // What the transfer picker will open on, if `c` found anything —
        // the rows and the Depot room, handed out past the `self.game`
        // borrow together.
        let mut opening: Option<(Vec<TransferRow>, Option<u32>)> = None;
        let acted = {
            let Some(game) = &mut self.game else { return };
            match key {
                GameKey::Up | GameKey::Char('k') => stepped(game, 0, -1),
                GameKey::Down | GameKey::Char('j') => stepped(game, 0, 1),
                GameKey::Left | GameKey::Char('h') => stepped(game, -1, 0),
                GameKey::Right | GameKey::Char('l') => stepped(game, 1, 0),
                GameKey::Char('.') => {
                    game.wait();
                    true
                }
                GameKey::Char('e') => {
                    game.use_power_source();
                    true
                }
                // Surface only, and not bound in the Stack block below: a
                // base's buffers are something you walk up to, and the
                // engine refuses it underground anyway.
                //
                // One key for both directions. It was two — `c` to collect
                // and `P` to put away — and an item on a shelf *and* in the
                // pack had a row on each screen with no way to see the
                // other.
                GameKey::Char('c') => {
                    let offer = game.transfer_offer();
                    if offer.is_empty() {
                        // Straight back through the engine, which speaks
                        // its own refusal and spends no turn. A
                        // `status_line` copy of that sentence here would be
                        // a second home for it, and a copy of an engine
                        // message reads as the key doing nothing.
                        game.refuse_transfer();
                        true
                    } else {
                        // Handed out past the `self.game` borrow, the way
                        // `refusal` is. Opening a screen is not an action.
                        //
                        // The room travels with the offer, and it travels as
                        // an `Option`: `None` is no Depot beside you at all,
                        // `Some(0)` a Depot with nothing left, and the
                        // screen has to be able to tell them apart.
                        opening = Some((offer, game.transfer_room()));
                        false
                    }
                }
                // Refused through the shared tail rather than silently, for
                // the reason `<`, `>` and `v` below are: the engine already
                // prices a rest by locale and knows why it will not happen,
                // so the sentence has to be its own. A rest that failed
                // used to report nothing at all here, which is what a bug
                // report about the key being dead looks like from outside.
                GameKey::Char('r') => match game.rest() {
                    Ok(()) => true,
                    Err(reason) => {
                        refusal = Some(reason);
                        false
                    }
                },
                // The same two keys the Stack binds, read the way the
                // Stack reads them: the pair is *up and down*, not in and
                // out. Base space is a platform you step onto, so `<` goes
                // up into it and `>` comes back down to the grid — the
                // opposite assignment to the Stack's, which is the point,
                // since a link genuinely goes down and the anchor does not.
                //
                // It was `>` in and `<` out, on the reasoning that the
                // anchor is a door like the link is and one pair should
                // mean one thing. What that read as in play was backwards:
                // the keys carry a direction before they carry a role, and
                // `>` on the anchor said "descend" while the party rose.
                //
                // Both are bound in this one block rather than split across
                // a locale check up here, because the engine already
                // dispatches: `enter_base` asks `require_surface` and
                // `leave_base` asks `require_base`, so pressing either in
                // the wrong place gets the engine's own words rather than a
                // second opinion from app-core that could disagree with it.
                GameKey::Char('<') => match game.enter_base() {
                    Ok(()) => true,
                    Err(reason) => {
                        refusal = Some(reason);
                        false
                    }
                },
                GameKey::Char('>') => match game.leave_base() {
                    Ok(()) => true,
                    Err(reason) => {
                        refusal = Some(reason);
                        false
                    }
                },
                // Lay a VectorStasis Tile over the carved cell underfoot.
                // `v` for the tile's own name: `t` is trade, and `T` is
                // spoken for by two battle screens that nothing may
                // document — see `crates/engine/EASTER_EGGS.md`, which is
                // why a key that has to appear in the help screen cannot
                // have it. Bound in this block rather than behind an
                // `in_base` check up top for the same reason `>` and `<`
                // are: `Game::lay_tile` asks `require_base` itself, so
                // pressing it on the open grid gets the engine's own words
                // instead of a second opinion from here.
                GameKey::Char('v') => match game.lay_tile() {
                    Ok(()) => true,
                    Err(reason) => {
                        refusal = Some(reason);
                        false
                    }
                },
                _ => false,
            }
        };
        // Through `App::refuse` rather than straight onto `status_line`:
        // a refusal is one sentence on two surfaces, and the banner ages
        // out after a few seconds while the log is what the player scrolls
        // back through. Assigning the field alone put every refused verb on
        // this screen on the banner only.
        if let Some(reason) = refusal {
            self.refuse(reason);
        }
        if let Some((offer, room)) = opening {
            self.open_transfer(offer, room);
        }
        self.after_world_action(acted, is_move_key);
    }

    /// Movement for a party that has a facing. Up walks forward along it;
    /// left and right turn in place and down turns the party clean around,
    /// which is what makes the Stack a first-person space rather than a
    /// top-down one seen at an angle.
    ///
    /// Everything else on the map screen is left alone — the mode keys above
    /// already ran, and the ones that need open grid refuse in the engine
    /// (see `Game::require_surface`).
    fn handle_stack_key(&mut self, key: GameKey, is_move_key: bool) {
        // The same `g` that is a no-op on the surface. Checked before the
        // game is borrowed below, and costing no tick: reading your own map
        // is not an action, and the Stack advancing a turn every time you
        // checked where you were would punish mapping.
        if key == GameKey::Char('g') {
            self.mode = Mode::FrameMap;
            return;
        }

        // Set after the `self.game` borrow below releases. A refusal is not
        // an action, so it leaves `acted` false and `after_world_action`
        // returns before it can clear the line that explains why.
        let mut refusal = None;
        let acted = {
            let Some(game) = &mut self.game else { return };
            match key {
                GameKey::Up | GameKey::Char('k') => {
                    game.step_forward();
                    true
                }
                GameKey::Down | GameKey::Char('j') => {
                    game.turn_around();
                    true
                }
                GameKey::Left | GameKey::Char('h') => {
                    game.turn_left();
                    true
                }
                GameKey::Right | GameKey::Char('l') => {
                    game.turn_right();
                    true
                }
                GameKey::Char('>') => {
                    game.descend();
                    true
                }
                GameKey::Char('<') => {
                    game.ascend();
                    true
                }
                GameKey::Char('.') => {
                    game.wait();
                    true
                }
                GameKey::Char('e') => {
                    game.use_power_source();
                    true
                }
                // Bound here as well as on the surface, because a rest is
                // priced by locale and not gated by it: `Game::rest` takes
                // neither `require_surface` nor `require_base`, and burns a
                // charge underground exactly as it does on the open grid.
                // Left out of this block it was not a refusal the player
                // could read but a dead key falling through `_ => false`.
                //
                // Binding it was not the whole fix, though it was reported
                // as one: a rest that *was* reached and then refused still
                // said nothing on this screen, so a player four frames down
                // could not tell the two apart. The refusal is the engine's
                // and goes through the shared tail, like `o` below.
                GameKey::Char('r') => match game.rest() {
                    Ok(()) => true,
                    Err(reason) => {
                        refusal = Some(reason);
                        false
                    }
                },
                // Not 't', which the mode block above already spends on the
                // trader list before this arm is ever reached. 'o' is free
                // and matches the glyph the orphan draws as in both views.
                GameKey::Char('o') => match game.adopt_orphan() {
                    Ok(()) => true,
                    Err(reason) => {
                        refusal = Some(reason);
                        false
                    }
                },
                // Undocumented on purpose — see `crates/engine/EASTER_EGGS.md`.
                // Uppercase, so it can never collide with the row shortcuts,
                // and `Z` specifically because every other capital here is a
                // shift-slip of a key the player presses constantly: `K J H L`
                // are movement, `S` is saving, `Q` is quitting, and a fumbled
                // step that quietly spent a turn and raised Trace would be a
                // bug the player could not diagnose.
                GameKey::Char('Z') => match game.listen() {
                    Ok(()) => true,
                    Err(reason) => {
                        refusal = Some(reason);
                        false
                    }
                },
                _ => false,
            }
        };
        // Through `App::refuse` rather than straight onto `status_line`:
        // a refusal is one sentence on two surfaces, and the banner ages
        // out after a few seconds while the log is what the player scrolls
        // back through. Assigning the field alone put every refused verb on
        // this screen on the banner only.
        if let Some(reason) = refusal {
            self.refuse(reason);
        }
        self.after_world_action(acted, is_move_key);
    }

    /// The bookkeeping that follows any action that advanced the world,
    /// whichever locale it happened in: clearing the status line, dropping
    /// into `Mode::Battle` if one just started, the movement cue, and the
    /// game-over check.
    ///
    /// Shared by the surface and Stack paths rather than copied into both.
    /// The battle transition especially: Phase 2 puts random encounters
    /// underground, and a second copy of this is exactly the kind of thing
    /// that gets updated on one side only.
    pub(crate) fn after_world_action(&mut self, acted: bool, is_move_key: bool) {
        if !acted {
            return;
        }
        self.status_line = None;
        let entered_battle = self
            .game
            .as_ref()
            .map(|g| g.has_active_battle())
            .unwrap_or(false);
        if entered_battle {
            self.mode = Mode::Battle;
        }
        if is_move_key {
            self.pending_sounds.push(if entered_battle {
                SoundEvent::BattleStart
            } else {
                SoundEvent::Step
            });
        }
        self.check_game_over();
        if self.mode == Mode::GameOver {
            self.pending_sounds.push(SoundEvent::Defeat);
        }
    }
}
