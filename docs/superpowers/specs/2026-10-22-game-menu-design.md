# Game menu: options, audio, graphics and quit in one hub

**Date:** 2026-10-22

## Intent

The player reaches for options mid-run and they aren't there: `Mode::Options`
opens only from the main menu. Audio and graphics settings aren't on any
screen at all; they are bare hotkeys owned by the renderer, and none of
them persist. This change gathers every setting, plus save and quit, into
one **Game menu** built on the existing side-rail hub (`app::hub`). Esc on
the map opens it.

## Goals

- Options are reachable during a run (Esc from the map).
- One place for gameplay, audio and graphics settings, and for leaving the run.
- Settings persist across launches in `profile.ron`, like
  `tactical_battles` already does.

## Non-goals

- No new settings. Specifically no fullscreen toggle, no music/SFX split
  and no key rebinding.
- Map zoom is unchanged. It stays a map control: the mouse wheel and
  `+`/`-` (user's decision). It is not in the menu.
- Battle screens don't get an entry point. The menu opens from the map
  (`Mode::Playing`) and the main menu only.

## Anti-goals

- A setting that forgets itself on relaunch.
- Two ways to change the same thing that can disagree. Every retired
  hotkey is removed, not kept alongside.
- Quitting without saving, reached by a single slip. The existing
  confirmation's guarantee (`handle_quit_run_confirm_key`: a failed save
  keeps the player on screen with the error) must survive.

## The design (user-decided)

**Opening it.**
- **Esc on the map** opens the Game menu. If the camera is following
  something (`App::watching`), the first Esc still only releases the
  camera, as it does now.
- **`[q]` on the map is retired.** Quitting lives in the menu.
- **`[s]` stays** as the instant save.
- **The main menu's `[o]`** opens the same hub. Pages that need a run
  (Save / Quit) are absent there, and Esc on the rail returns to the main
  menu.

**Pages.** The page set and order are the builder's call, but they must
cover these:

| Content | Today | In the menu |
|---|---|---|
| Tactical surface battles on/off | `Mode::Options`, main menu only | a gameplay page |
| Volume, 0–100% in 10% steps | `[` / `]`, renderer-only, resets to 20% each launch | audio page; Left/Right adjust |
| Visual effects on/off | `\`, renderer-only, resets each launch | graphics page |
| Frame-timings overlay on/off | `F3`, renderer-only | graphics page |
| Save; save and quit to main menu; quit without saving | `[q]` → `Mode::QuitRunConfirm` | a save/quit page, only during a run |

**Retired hotkeys:** `q` (on the map), `[`, `]`, `\` and `F3`. With
`[`/`]` gone, the gui's `volume_keys` gate (it exists only so those keys
don't fight the hub's page turn) is deleted.

**Settings persist.** Volume, effects and the overlay are added to
`Profile` (`crates/engine/src/achievements.rs`) as `#[serde(default)]`
fields with the current defaults. That means volume defaults to 0.2, which
is `DEFAULT_VOLUME`; effects default to on; the overlay defaults to off.
Each change writes `profile.ron` immediately, the way `App::toggle_option`
does now. A failed write shows on the status line and costs only that
setting. `Profile::load` throws away the whole profile on a parse error,
so new fields must stay `serde(default)`, and an older `profile.ron`
without them must still load. Test that.

**The renderer reads, the app owns.** Today `crates/gui/src/lib.rs` keeps
`fe.volume`, `fe.fx.enabled` and `fe.perf_on`, and documents them as
"GUI-only concerns (`App` knows nothing about either)". After this change
the values live in `App.profile`. The gui reads them each frame, and only
app-core writes them. This moves ownership across the app-core/gui
boundary, and is the one point that may need an ADR. See "Open before
code" below.

**The sim pauses while the menu is open.** This is already true:
`App::update_realtime` only ticks in `Mode::Playing`. A queued walk is
dropped, as with any other screen opened from the map.

## Builder's call

- Page titles, order, and whether gameplay and graphics share a page.
- How a toggle row and a volume row are drawn and driven (Enter vs
  Left/Right). Follow the existing Options screen and hub conventions.
- Whether `Mode::Options` and `Mode::QuitRunConfirm` become the hub's
  screens or are replaced by new modes. Either way, no orphaned mode is
  left behind (CLAUDE.md: delete what's unused).
- How the save/quit page keeps the three choices and the failed-save hold.
- The volume toast: whether adjusting volume in the menu still shows it
  (the value is already on screen).

## Constraints and touch points

- Hub framework: `crates/app-core/src/app/hub.rs`. A new hub is a `Hub`
  variant, an arm in `Hub::kind`, and a `HubKind` impl in its own module.
  It needs `pages(&App)` because Save / Quit is conditional, and
  `leave(&App)` because Esc goes back to the map or the main menu
  depending on where it was opened from.
- Today's options: `crates/app-core/src/app/menus.rs`
  (`option_rows`, `toggle_option`, `handle_options_key`). Gui drawing:
  `crates/gui/src/render/mod.rs` (`draw_options`,
  `draw_quit_run_confirm`).
- Quit: `crates/app-core/src/app/lifecycle.rs`
  (`handle_quit_run_confirm_key`, `leave_run`); `[q]` binding at
  `crates/app-core/src/app/playing.rs` (`handle_playing_key`).
- Esc on the map: `releases_the_camera` in `playing.rs`.
- Renderer hotkeys and state: `crates/gui/src/lib.rs` (`volume_keys`
  block, `\` and `F3` handlers, `DEFAULT_VOLUME`, `draw_toast`).
- Keycaps and help that name retired keys:
  `crates/gui/src/render/hud/log_frame.rs` (`keycap("q", "menu")`) and
  `assets/help/20-controls.md`. Update both. Don't touch `docs/manual.md`.
- Seam rules: read `.claude/rules/seams-screens.md` and `seams-hud.md`
  before changing screens or the keybar.
- Existing tests to move or extend: `crates/app-core/src/tests/options.rs`,
  `crates/app-core/src/tests/quitting.rs`.
- Process: two crates plus new `profile.ron` fields, so this gets a plan
  (superpowers:writing-plans), TDD and the full workspace gate.

## Settled after review

- **Tactical battles applies live (user).** Today the engine reads it from
  the `Profile` resource `install_profile` hands it once per run, which is
  why `Mode::Options` was main-menu-only (its doc comment says so; rewrite
  that comment). Flipping the row mid-run now pushes the new value into the
  running game through a `Game` method, so the next surface fight uses it.
  No `world` accessor.
- **No ADR (user).** Moving volume, effects and the overlay into
  `App.profile` is three settings joining an existing settings file, not a
  new boundary. The reasoning is recorded here.
- **The in-memory value wins (my call).** A change applies at once even if
  the write fails. The paths that re-read `profile.ron` mid-session
  (`lifecycle.rs`, new-run and load paths) must not revert settings the
  player just changed. Either they keep the in-memory settings, or the
  settings live outside the reloaded struct. That choice is the builder's.
- **Esc order (my call).** Esc on the map releases a watching camera first,
  then cancels any in-map state that Esc already cancels today. Only an
  Esc with nothing else to do opens the menu.
- **Quit without saving keeps a confirm step (my call).** Save and quit
  doesn't need one.
- **"Quit" means back to the main menu (my call)**, via `leave_run`, as
  `[q]` does today. Exiting the program stays the main menu's own `[q]`.
