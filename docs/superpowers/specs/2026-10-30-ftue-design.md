# First-time user experience (FTUE) — design

**Status:** Approved (implementation started 2026-10-30).

## Intent

The user wants to write short messages for the player — lore, gameplay help,
or context about the game — that pop up the first time something happens:
a research finishes, a structure is built, an item is made, or a game event
occurs (a first raid, first descent, low Power, …). Today each such message
is a `NotificationKind` variant plus match arms spread through
`notifications.rs`, with the trigger buried at the call site. The goal is
**one place to read and edit every first-time message**, so writing a new
one never means searching the codebase.

This feature absorbs the existing tutorial notifications. "Tutorial" and
"lore" become one thing: the FTUE.

## Goals

- One Rust module, `crates/engine/src/ftue.rs` (or `ftue/` if it grows),
  holding every first-time message as a flat list of entries:
  trigger + title + body (+ optional sprite/glyph/colour, as notifications
  have now).
- Four trigger kinds:
  - `Research(id)` — that research completes.
  - `Built(id)` — that structure comes to stand in the base.
  - `Crafted(id)` — that item is made.
  - `Event(FtueEvent::…)` — a named game event; an enum the engine fires
    from the call sites the tutorials use today.
- Adding a message for an existing research/structure/item = adding one
  entry to the list. No other file changes.
- Each message shows **once per playthrough**. A new game shows it again.
- The popup is the existing notification screen and queue, with three
  choices:
  - **Dismiss** (Enter/Esc) — close; a new playthrough shows it again.
  - **Don't show again** — this message never shows again, any run.
  - **Turn off guidance** — no FTUE message shows again, any run.
- The Options screen gets: a guidance **on/off** toggle, and a
  **"show all again"** action that turns guidance on and clears every
  "don't show again".
- The existing tutorial notifications move into the FTUE list and follow
  its rules (once per playthrough, subject to the three choices).

## Non-goals

- No `assets/` folder or `.ron` files. The user said Rust is fine as long
  as it is in one place.
- Milestone notifications (`Repeat::Always`: Breach, ContractClosed,
  ResearchComplete, ResearchDiscovered, LevelCapReached, PhaseKeyFound,
  ProgramDecompiled, SettlementGrown, SweepsBegin) stay as they are. They
  are news, not guidance, and "turn off guidance" does not silence them.
- The alert board (`alerts.rs`, `N` key) is untouched.
- No codex/log for re-reading old messages.

## Anti-goals

- A typo in an id that silently never fires. Every `Research`/`Built`/
  `Crafted` id in the list is checked against the real asset databases by
  a test; an unknown id fails `cargo test`.
- "Once per playthrough" that really means once per session (fires again on
  every reload). The per-run latch lives in the save. This is the reason the
  notifications seam rejected a per-run policy; storing it in the save is
  what makes the policy honest now.
- Turning guidance off by an accidental keypress with no way back — the
  Options screen is the way back.
- A tutorial firing twice in one playthrough because a call site is reached
  repeatedly (call sites fire unconditionally today; the latch decides).

## Decided by the user

- Rust, one module, named FTUE (not "lore").
- Triggers: research, built structure, crafted item, game events.
- Once per playthrough.
- Popup choices: dismiss / don't show again / turn off guidance.
- Options: turn all back on / all off.
- Existing tutorials merge into the FTUE format.
- **Old saves:** a save from before this change loads with nothing shown,
  so each ported tutorial re-appears once the next time it triggers. No
  migration code. Save compatibility isn't a concern (sole player), so the
  `SAVE_FORMAT_VERSION` bump may be breaking if that is simpler.

## My calls (the user may overturn)

- **Persistence split.**
  - Per-playthrough "already shown" set → the save file (`save.rs`; bump
    `SAVE_FORMAT_VERSION` per its rules; load an older save with an empty
    set). Breaking-ness per `CHANGELOG.md`'s preamble.
  - "Don't show again" set and the guidance switch → `profile.ron`
    (`achievements::Profile`), `#[serde(default)]`, plain strings for the
    same reason `seen_notifications` is strings (a retired key must be
    inert, never a parse error that costs the player their achievements).
- **Each FTUE entry has a stable string key** that is what reaches disk in
  both places — derived from the trigger (e.g. `research:deep_mining`,
  `event:first_raid`). Existing `seen_notifications` entries
  (`tutorial_first_raid`, …) are *not* migrated into "don't show again":
  they recorded "seen once", not "never again", and under the new rule a
  new playthrough should show them. `seen_notifications` stays in the
  profile struct as an inert field only if removing it would break parsing
  — the builder checks; prefer removing it if `ron` ignores unknown fields
  in this struct, otherwise keep it, unread.
- **`Built` fires when the structure finishes and stands**, not when the
  blueprint is placed: crew builds via `complete_new_build`
  (`game/base/construction.rs`); instantly placed structures (the founding
  Home in `place_structure`) at placement. Upgrades don't count.
- **`Crafted` fires from every path that makes an item**: hand craft
  (`game/crafting.rs`), work orders (`game/base/work_orders.rs`
  `settle_orders`), and machine assembly (`systems.rs` assemble). The
  assembler runs in a bevy system with no `Game`; the builder picks how
  that reaches the FTUE (e.g. a queued fact drained by `Game` each tick,
  the way low Power is read). One message per item regardless of path.
- **`Research` fires after the existing `ResearchComplete` milestone** in
  the queue, so the player reads "research complete" then the guidance.
- **Several messages at once queue** behind each other, as notifications
  do today. No cap.
- **Timing unchanged:** shows only in `Mode::Playing` (the existing
  `show_next_notification` rule).
- **Keys on the popup:** Enter/Esc dismiss, `D` don't show again, `T` turn
  off guidance. Hint text follows `key-hint-case-is-the-handler-case`.
  Check `D`/`T` aren't raw keys that fire in every mode
  (`gui-raw-keys-fire-in-every-mode`).
- **When guidance is off or a message is "don't show again"**, nothing is
  queued and the per-run latch is still set (so turning guidance back on
  mid-run doesn't flood the player with everything they skipped).
- **Exhaustive matches stay where they guard something:** `FtueEvent` text
  lives in the FTUE list, and a test asserts every `FtueEvent` variant has
  exactly one entry (and every entry key is unique), replacing the
  compile-time `def` match for tutorials. The notification `def` match
  keeps covering milestones.
- **Onboarding (`OnboardingMission`, `OnboardingComplete`) stays a
  notification**, not FTUE: it is a contract chain's news, repeats per run
  already, and turning guidance off shouldn't hide a contract. (Builder:
  flag if its current policy says otherwise.)

## Constraints

- Engine owns the list, triggers, latches and decisions; app-core owns the
  popup's keys and the Options entry; gui draws. The renderer never touches
  the `World` (`CLAUDE.md`). Drawing goes through `Painter`.
- Notification seam rules (`.claude/rules/seams-notifications.md`) apply:
  fixed 75% panel with no scroll — every FTUE body must fit
  (`the_tallest_shipped_notification_fits_its_screen` extends to FTUE
  entries); the screen stays in `needs_status_banner` and `ALL_MODES`.
- The seam doc changes (the per-run policy now exists, tutorials leave the
  enum): update the rules file, the `seams` skill and `seam:` graph entry
  together, per `CLAUDE.md`.
- New save field → save→load test, not only a RON round-trip
  (`ron-round-trip-cannot-catch-a-skipped-field`).
- The decision step is pure (functional core): given the trigger, the
  per-run set, the profile's sets and the switch → show / don't show, and
  which latches to write. Test it directly.
- `CHANGELOG.md` entry; a release per `CLAUDE.md`.

## Left to the builder

- Exact type and field names, file split if `ftue.rs` grows past readable.
- How the assembler path reports a crafted item.
- Exact wording of the popup's key hints and the Options rows (plain
  language, `public-copy-must-be-plain-language`).
- Porting each tutorial's text verbatim into the FTUE list.
