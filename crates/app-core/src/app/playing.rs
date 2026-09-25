//! The map screen — movement, and the keys that open every other mode.

use crate::DEV_CONSOLE_KEY;
use crate::*;

/// What `c` hands out past the `self.game` borrow: everything
/// `App::open_transfer` needs. Opening a screen is not an action, so it
/// cannot happen inside the borrow that answered the key.
struct TransferOpening {
    rows: Vec<TransferRow>,
    carriers: Vec<TransferCarrier>,
    room: Option<u32>,
    rack_room: u32,
}

/// The shared "did the world actually move" rule behind `stepped` and
/// `stepped_paced`: `f` performs the step and answers `(bite, drag_owed)`,
/// and this adds the one `acted` read both need — extracted so the two
/// can't drift on what counts as an action the way they once did, sharing
/// this line by hand in each.
///
/// `Game::move_player`'s own return says whether an action happened only
/// indirectly, and on the zone surface assuming an action was fine — every
/// step there spends a turn, a bounce off a wall included. Base space did
/// not work that way when it shipped: a step into solid rock was refused
/// outright and cost nothing, so reporting it as an action would have
/// cleared the status line explaining an earlier refusal and queued a
/// footstep for a step that never happened. Slice 2 turned that bounce
/// into a swing, which does spend a turn — and this function needed no
/// edit for it, which is the whole argument for reading the clock instead
/// of assuming per locale.
///
/// The clock is what all three locales agree on, so that is what this
/// reads. The game-over clause is the one case a real action leaves the
/// clock standing still: ground that kills you sets `GameOver` before
/// `Game::tick` runs and `tick` then returns without advancing — but
/// `App::after_world_action` still has to see an action, or the run would
/// never reach the death screen.
fn stepped_with(game: &mut Game, f: impl FnOnce(&mut Game) -> (i32, u32)) -> (bool, i32, u32) {
    let before = game.current_tick();
    let (bite, owed) = f(game);
    let acted = game.current_tick() > before || game.is_game_over().is_some();
    (acted, bite, owed)
}

/// One step, reported honestly: `true` only when the world actually moved.
///
/// `bite` is an out-parameter rather than a second return value because the
/// four movement arms of the match this feeds all have to answer `bool` like
/// every other arm on that screen. It receives what the ground took off the
/// party on this step — `0` for clean ground, and for a shove at a wall,
/// which spends a turn without costing Integrity.
fn stepped(game: &mut Game, dx: i32, dy: i32, bite: &mut i32) -> bool {
    let (acted, b, _drag_owed) = stepped_with(game, |g| (g.move_player(dx, dy), 0));
    *bite = b;
    acted
}

/// `stepped`'s own shape for the clocked walk: calls `Game::move_player_paced`
/// instead of `Game::move_player`, so drag ground's extra ticks come back as
/// a number owed rather than being spent inline — `App::spend_walk_tick`
/// holds that count in `drag_ticks_owed` and pays it one `idle_tick` a clock
/// tick, which is `travel-on-the-clock`'s drag fix (task A). `stepped` itself
/// stays as it is for the paused, turn-based path, where spending drag
/// inline is still correct — `acting_while_paused_still_spends_a_turn`.
fn stepped_paced(game: &mut Game, dx: i32, dy: i32, bite: &mut i32) -> (bool, u32) {
    let (acted, b, owed) = stepped_with(game, |g| g.move_player_paced(dx, dy));
    *bite = b;
    (acted, owed)
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

/// The eight keys that step the party on this screen — arrows and their
/// `hjkl` aliases. Shared by the walk-queueing arm below and the Stack
/// hand-off, so the one set of keys can't drift between the two readings.
fn is_move_key(key: GameKey) -> bool {
    matches!(
        key,
        GameKey::Up
            | GameKey::Down
            | GameKey::Left
            | GameKey::Right
            | GameKey::Char('k')
            | GameKey::Char('j')
            | GameKey::Char('h')
            | GameKey::Char('l')
    )
}

/// SPACE (pause) and `,` (world speed) — the two keys `handle_playing_key`'s
/// own match binds "up here, beside the digits below", for the toggle's own
/// reason: the clock runs underground too. Neither is a move key, but a
/// travel set while paused is meant to wait for the clock (the design's own
/// words), so the walk-clearing check below has to know them by name rather
/// than by the "any key that isn't itself a step" rule everything else on
/// this screen follows — task C's fix, since pressing SPACE to resume a
/// paused travel used to cancel the very travel it was unpausing.
fn is_clock_key(key: GameKey) -> bool {
    matches!(key, GameKey::Char(' ') | GameKey::Char(','))
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
        // A queued walk — a step as much as a travel — is intent for a
        // tick not yet spent, and any key that isn't itself a step says the
        // player wants this one doing something else instead. An arrow
        // overwrites a walk rather than needing this to catch it too — see
        // the walk-queueing arm below. Matching `Walk::Travel` alone used to
        // leave a queued `Walk::Step` behind for a following tick-spending
        // key (an arrow, then `.` or `r`, inside one frame) to spend
        // unasked.
        //
        // **Except the clock's own two keys.** SPACE and `,` don't spend a
        // tick and don't mean "do something else instead" — a travel set
        // while paused is meant to wait for the clock, so unpausing it with
        // the very key that resumes the clock must not read as the player
        // asking for anything but that (task C).
        if !is_move_key(key) && !is_clock_key(key) && self.walk.is_some() {
            self.walk = None;
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
            // The downed-program store, which is not a pocket of the pack:
            // a body goes down in most fights and the tool page is where it
            // is spent, so it gets a door of its own rather than one reached
            // two screens deep. Kept on `D` — the same key the pack binds —
            // because one screen with two doors on two different keys is
            // worse than the extra reach it saves. Uppercase is forced
            // there, not here: `selected_index` reserves shifted letters for
            // screen actions, and `d` is demolish on this screen anyway.
            //
            // Bound up here beside the digits and for their reason: this
            // match runs before the hand-off to `handle_stack_key`, so one
            // arm reaches both locales and there is no second one to drift
            // from it. Extraction is priced by locale and gated by none —
            // `extraction_yield` reads a bench tier and nothing about where
            // the party stands — so a store reachable only on the surface
            // would be the keyboard disagreeing with the engine.
            GameKey::Char('D') => {
                self.pending_downed_program_index = None;
                self.mode = Mode::DownedPrograms;
                return;
            }
            // Examine, in the roguelike sense — `i` went to the pack, and
            // perks moving into the party menu freed the key a player would
            // guess for it anyway.
            GameKey::Char('x') => {
                self.mode = Mode::InspectDirection;
                return;
            }
            // Demolish, aimed rather than picked from a list. Open in both
            // spaces now: this used to be refused outside base space because
            // out there the four directions pointed at nothing ownable — every
            // `Structure` stands in base space and the player's surface
            // `Position` is either their own tile or pinned to the Stack
            // entrance. Traps made that false for the first time, so the
            // question is no longer *where are you* but *which space decides
            // what `d` means*, which is `handle_remove_direction_key`'s own
            // branch.
            GameKey::Char('d') => {
                self.mode = Mode::RemoveDirection;
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
                        self.excavate_brush = None;
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
                // Closed the same way `d` above is: `Game::view_entities` now
                // refuses to answer a `Structure` query outside base space
                // (every trader is one), so the list this used to open on the
                // open grid would always come back empty — Task 5 left it
                // noted rather than fixed in
                // `seam:require-surface-used-to-mean-not-in-the-stack-and-ten-of`,
                // and this is the guard flip that note asked for. Both
                // remaining branches are surface-or-base only,
                // `is_underground` having already returned above, so `in_base`
                // cannot collide with the Stack case handled there.
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
            // Bound up here, beside `L`/`u`/`f` and for their reason: this
            // match runs before the `is_underground()` hand-off below, so
            // the board opens on the surface, in base space and in the
            // Stack alike with no second arm down there to drift from this
            // one. Reading the board is not an action, hence the `return`,
            // and opening it is the one thing that marks every alert read.
            GameKey::Char('N') => {
                if let Some(game) = self.game.as_mut() {
                    game.mark_alerts_read();
                }
                self.menu_selected = 0;
                self.mode = Mode::Alerts;
                return;
            }
            // Where the run's known destinations lie. A `return` for `L`'s
            // reason — reading the map is not an action and must not cost a
            // turn. Lowercase because it is bound on the zone map, where
            // lowercase is the convention; `u` is one of the three letters
            // left there and none of the three is mnemonic.
            GameKey::Char('u') => {
                self.mode = Mode::Compass;
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
            GameKey::Tab => {
                self.log_expanded = !self.log_expanded;
                return;
            }
            // The idle clock: SPACE holds it, `,` cycles its speed. Up
            // here for the toggle's reason — the clock runs underground
            // too — and a `return` because none of them is an action.
            GameKey::Char(' ') => {
                self.paused = !self.paused;
                return;
            }
            GameKey::Char(',') => {
                self.world_speed = self.world_speed.next();
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

        let is_move_key = is_move_key(key);
        // Underground the same four keys steer a party that has a facing:
        // forward, back, and turn in place. Deliberately the same keys rather
        // than a separate set — walking is walking, and the view makes which
        // one you're doing obvious.
        if self.game.as_ref().is_some_and(|g| g.is_underground()) {
            self.handle_stack_key(key, is_move_key);
            return;
        }

        // Not paused: queue the step for the clock to spend on its next
        // tick (`App::spend_walk_tick`) instead of moving right here — the
        // whole of `travel-on-the-clock`'s fix, since `handle_key` used to
        // spend a tick per press and gui's key repeat fires far faster than
        // any `WorldSpeed`. An arrow overwrites whatever was already
        // queued, travel included, so at most one step is ever pending.
        // Paused leaves this alone: `acting_while_paused_still_spends_a_turn`
        // is the turn-based path below, unchanged.
        if is_move_key && !self.paused {
            let delta = match key {
                GameKey::Up | GameKey::Char('k') => (0, -1),
                GameKey::Down | GameKey::Char('j') => (0, 1),
                GameKey::Left | GameKey::Char('h') => (-1, 0),
                GameKey::Right | GameKey::Char('l') => (1, 0),
                _ => unreachable!("is_move_key guards this to the four directions"),
            };
            self.walk = Some(Walk::Step(delta.0, delta.1));
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
        let mut opening: Option<TransferOpening> = None;
        // What the ground took off the party, for the cue
        // `after_world_action` picks. Declared out here because the four
        // movement arms below sit inside a `self.game` borrow and have to
        // keep answering `bool`.
        let mut ground_bite = 0;
        let acted = {
            let Some(game) = &mut self.game else { return };
            match key {
                GameKey::Up | GameKey::Char('k') => stepped(game, 0, -1, &mut ground_bite),
                GameKey::Down | GameKey::Char('j') => stepped(game, 0, 1, &mut ground_bite),
                GameKey::Left | GameKey::Char('h') => stepped(game, -1, 0, &mut ground_bite),
                GameKey::Right | GameKey::Char('l') => stepped(game, 1, 0, &mut ground_bite),
                // Zone surface only on this path — the Stack binds its own
                // `.` below, and base space binds none at all. Falling
                // through the guard to `_ => false` leaves it a **dead key**:
                // no turn, no refusal, nothing on the line.
                //
                // That is the deliberate exception to what every other
                // locale-shy key on this screen does. `r`, `<`, `>` and `v`
                // each hand the engine's own sentence to `App::refuse`,
                // precisely because a key that appears to do nothing is what
                // a bug report looks like from outside. Waiting is exempt
                // because there is no engine refusal to hand over — the
                // engine will happily tick in base space — so a sentence here
                // would be app-core's own opinion about time, invented at the
                // keyboard and with nothing behind it. Base space is time
                // spent by walking it, and the key reads as absent because
                // it is.
                GameKey::Char('.') if !game.in_base() => {
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
                // **Not `c`**: a rig carries `Stock`, so `c` already opens
                // the transfer picker at one to collect what it has
                // stripped, and one key meaning two things depending on
                // whether a buffer happens to be empty is worse than a
                // second key.
                //
                // **And not `T`**, which was the first choice and is one of
                // the hidden keys `crates/engine/EASTER_EGGS.md` lists — a
                // documented feature cannot have a key no page may name, and
                // `no_shipped_help_page_names_a_hidden_key` is what says so.
                // `F` is free here and already means "set up the machine
                // beside you" on the transfer picker, where `[F]` opens a
                // Depot's filter.
                GameKey::Char('F') => self.open_rig_tool(),
                GameKey::Char('c') => {
                    let offer = game.transfer_offer();
                    let carriers = game.rack_offer();
                    // An empty offer is still worth a screen when a Depot
                    // is standing here: `[F]` is reached from inside the
                    // picker, and a Depot built five seconds ago with an
                    // empty pack beside it is exactly when the player wants
                    // to set it up. Without this the one Depot that most
                    // needs configuring is the one that cannot be.
                    let configurable = !game.adjacent_depot_entities().is_empty();
                    if offer.is_empty() && carriers.is_empty() && !configurable {
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
                        opening = Some(TransferOpening {
                            rows: offer,
                            carriers,
                            room: game.transfer_room(),
                            rack_room: game.total_rack_room(),
                        });
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
        if let Some(o) = opening {
            self.open_transfer(
                o.rows,
                o.carriers,
                o.room,
                o.rack_room,
                TransferSource::Base,
            );
        }
        self.after_world_action(acted, is_move_key, ground_bite);
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
        self.after_world_action(
            acted,
            is_move_key,
            0, /* the Stack has no ambient ground; its corruption tiles narrate their own damage */
        );
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
    /// `ground_bite` is what ambient ground took off the party on this step
    /// — `0` for every caller that wasn't a surface step, which is all of
    /// them but one. It picks the movement cue and nothing else: a step that
    /// costs Integrity has to *sound* like taking a hit rather than like
    /// walking, because the damage is otherwise indistinguishable from the
    /// step that caused it. The Stack's corruption tiles are deliberately
    /// not routed through here — they come down a different path and have
    /// always narrated their own damage.
    pub(crate) fn after_world_action(&mut self, acted: bool, is_move_key: bool, ground_bite: i32) {
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
            self.mode = self.opened_battle_mode();
        }
        // Beside the battle check above and not folded into it: the engine
        // hands over a cue rather than a mode because it cannot see
        // app-core's `Mode` at all. The bump ladder's settlement arm
        // (`find_settlement_at`, the fourth arm) queues the visit and then
        // calls `self.tick()` itself, and `tick_inner` calls
        // `pursuit_tick` right after `nest_respawn_tick` — so a
        // `Pursuing` guardian already adjacent to the player can start a
        // battle *inside that same tick*. The battle wins the mode, but the
        // cue must still be drained: left in `PendingVisit` it would reopen
        // `Mode::Settlement` on some later, unrelated action once the fight
        // is over, so this always calls `take_visit` and only
        // assigns the mode when no battle started.
        if let Some(visit) = self.game.as_mut().and_then(|g| g.take_visit())
            && !entered_battle
        {
            match visit {
                Visit::Settlement(key) => {
                    self.pending_settlement = Some(key);
                    self.mode = Mode::Settlement;
                }
                Visit::Outpost(tile) => {
                    self.pending_outpost = Some(tile);
                    self.mode = Mode::OutpostVisit;
                }
            }
        }
        if is_move_key {
            self.pending_sounds.push(if entered_battle {
                SoundEvent::BattleStart
            } else if ground_bite > 0 {
                // The battle cue, not one of its own: this *is* taking
                // damage, and a player who has fought already knows what it
                // means. A separate clip would be a second thing to learn
                // for a sensation the game has a word for.
                SoundEvent::Hit
            } else {
                SoundEvent::Step
            });
        }
        self.check_game_over();
        if self.mode == Mode::GameOver {
            self.pending_sounds.push(SoundEvent::Defeat);
        }
    }

    /// `update_realtime`'s loop body: spends one clock tick on the pending
    /// walk, or idles when there is none. Split out of that loop so the
    /// `&mut self.game` borrow a step needs can end before `after_world_action`
    /// and `refuse` — each `&mut self` whole — are called; that borrow-scoping
    /// is the only reason this isn't written inline there.
    ///
    /// Routes every step through `after_world_action` with `is_move_key:
    /// true`, exactly as `handle_playing_key`'s own arrow arms do — the
    /// same call is what picks the Step/Hit/BattleStart cue, opens a
    /// settlement or outpost visit, and switches to a fight, so a walked
    /// step can't pick a different cue or skip one of those than a typed
    /// one does.
    ///
    /// **Drag ground owed is paid first, ahead of the walk.** A step onto
    /// drag ground reports its extra ticks through `drag_ticks_owed`
    /// (`stepped_paced`/`Game::move_player_paced`) rather than spending them
    /// inline the way the paused path still does — this is what stops
    /// `travel-on-the-clock`'s clocked walk from fast-forwarding the world
    /// by more than one tick per clock tick. Each owed tick is one plain
    /// `idle_tick`, exactly what a standing-still player already spends
    /// every real-time tick — the walk itself does not advance while any
    /// are outstanding, so the player waits on drag ground rather than
    /// crossing it in a burst.
    pub(crate) fn spend_walk_tick(&mut self) {
        if self.drag_ticks_owed > 0 {
            self.drag_ticks_owed -= 1;
            if let Some(game) = &mut self.game {
                game.idle_tick();
            }
            return;
        }
        let Some(walk) = self.walk else {
            if let Some(game) = &mut self.game {
                game.idle_tick();
            }
            return;
        };
        let mut ground_bite = 0;
        let mut no_route = false;
        let mut drag_owed = 0;
        let acted = {
            let Some(game) = &mut self.game else { return };
            let before = game.current_tick();
            let acted = match walk {
                Walk::Step(dx, dy) => {
                    self.walk = None;
                    let (acted, owed) = stepped_paced(game, dx, dy, &mut ground_bite);
                    drag_owed = owed;
                    acted
                }
                Walk::Travel { goal, in_base } => {
                    // The space the travel was set in no longer matches
                    // where the party stands — a base entrance or exit
                    // taken by some other means since. `Game::travel_step`
                    // has no notion of "the wrong space"; it would just
                    // answer for whichever space it's asked about, so the
                    // check belongs here, before it's asked at all.
                    if game.in_base() != in_base {
                        self.walk = None;
                        false
                    } else {
                        match game.travel_step(goal) {
                            TravelStep::Toward(dx, dy) => {
                                let before = current_travel_tile(game, in_base);
                                let (acted, owed) = stepped_paced(game, dx, dy, &mut ground_bite);
                                drag_owed = owed;
                                // A route that didn't actually move anybody
                                // — a hostile stepped onto the planned cell,
                                // say — would otherwise be asked again next
                                // tick and answer the same `Toward` forever.
                                if current_travel_tile(game, in_base) == before {
                                    self.walk = None;
                                }
                                acted
                            }
                            TravelStep::Last(dx, dy) => {
                                self.walk = None;
                                let (acted, owed) = stepped_paced(game, dx, dy, &mut ground_bite);
                                drag_owed = owed;
                                acted
                            }
                            TravelStep::Arrived | TravelStep::Gone => {
                                self.walk = None;
                                false
                            }
                            TravelStep::NoRoute => {
                                self.walk = None;
                                no_route = true;
                                false
                            }
                        }
                    }
                }
            };
            // A queued step is spent on the clock's own tick even when it
            // moved nobody — a bounce off base rock with mining off, a
            // barrier, or a walk that found nothing to do (`Arrived`,
            // `Gone`, `NoRoute`, a space mismatch) all leave `GameClock`
            // exactly where they found it, and `spend_walk_tick` runs once
            // per clock tick. Without this, holding a direction into a wall
            // would freeze the world instead of merely refusing to walk
            // through it — `travel-on-the-clock`'s own "one tick, one step"
            // promise applies to a step that goes nowhere too.
            if game.current_tick() == before && game.is_game_over().is_none() {
                game.idle_tick();
            }
            acted
        };
        self.drag_ticks_owed = drag_owed;
        // A step that hurt the player ends the walk right here — "the
        // player should stop moving if weather is making them take
        // damage" (task B). The damage is already announced by
        // `Game::move_player_paced` itself (the "takes N off you." log
        // line `after_world_action`'s `Hit` cue plays over), so this adds
        // no second sentence — it only stops the *next* step from being
        // queued. A `Walk::Step` is a one-off already cleared above; this
        // is what actually matters for `Walk::Travel`, which would
        // otherwise keep walking the party across ground that is hurting
        // them.
        if ground_bite > 0 {
            self.walk = None;
        }
        self.after_world_action(acted, true, ground_bite);
        if no_route {
            self.refuse("No clear way there.");
        }
        // Immediate rather than waiting for `update_realtime`'s own guard
        // to catch it next call: `after_world_action` is what switches
        // `mode` away from `Playing` (a fight, a settlement or outpost
        // visit), and a travel surviving that would resume wherever it
        // left off the moment the screen it opened is left.
        //
        // `drag_ticks_owed` is left alone even here: a step onto drag
        // ground that also opened a fight still owes what it owes, and
        // `App::install_game` is the one place that debt clears.
        if self.mode != Mode::Playing {
            self.walk = None;
        }
    }
}

/// The tile the player occupies in `in_base`'s space, `Game::travel_origin`'s
/// own read — `pub(crate)` to the engine alone, so `spend_walk_tick` asks it
/// through the two public doors that answer the same question: the base
/// coordinate stays pinned in `resources::Locale::Base`, and the surface
/// tile is the one `Position` never goes stale for once underground and
/// base space are both ruled out (`Game::travel_step`'s own doc).
fn current_travel_tile(game: &Game, in_base: bool) -> Option<(i32, i32)> {
    if in_base {
        game.base_pos()
    } else {
        Some(game.player_status().position)
    }
}
