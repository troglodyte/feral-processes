# Focused alerts

**Mockup:** <https://claude.ai/artifact/8a4q8LNUc3xqy6wbtXbg8J> (option C chosen)

## Why

The notice screen (`crates/gui/src/render/notify.rs`) is a centred panel
covering 75% of the window. When a notice is about one thing on the map,
such as a staff member walking off to fuse with the Research Station, the
panel hides the very thing it describes. The player reads about the
moment instead of seeing it. The user wants story beats to feel involved:
the alert points at the thing, and the player can follow it.

Success: when a focused alert opens, the player sees the panel on one
side, the subject marked on the map beside it, and a line connecting the
two. For a program, one key follows it once the alert closes.

## Goals

1. **Focused alert layout.** A notice that carries a map subject draws
   the panel docked to one side instead of centred, then scrolls the
   camera so the subject sits in the clear part of the map. The subject
   is marked with pulsing corner brackets, with a thin leader line from
   the panel's edge to the brackets (mockup option C).
2. **[W] Watch.** When the subject is a program, the alert offers
   `[W] Watch`. W dismisses the alert and sets `App::watching` to that
   program: the existing follow mechanism, released by Esc as today.
3. **[D] Don't show again** for every repeating (milestone) notice kind,
   remembered in the profile. Guidance popups keep their existing D/T.
4. **Which notices are focused** (user's list):

   | Notice | Subject | Watch |
   |---|---|---|
   | `BaseAiAwakening` "Something is going wrong with {name}" | the host program | yes |
   | `BaseAiFused` "It Has Fused" | the host, now seated on the Research Station | yes (the subject is the host, now on the station, so Watch still works) |
   | `ProgramDecompiled` | the prisoner who joined | yes |
   | Base staff downed at the base (new milestone kind, see 5) | the downed staff member | yes, while `watch_position` returns a position |
   | Guidance "Your Base" | the base anchor tile | no |
   | Outpost alerts (see 6) | the outpost's tile | no |

   Everything else keeps the centred panel unchanged.
5. **Base staff downed at the base.** The user's rule: the program-down
   alert fires only when the program is base staff, downed at the base,
   for example in a brawl between staff (`game/base/tantrum.rs`). Then the
   body is on the base map to mark. Downs in a battle, a sortie or an
   off-screen siege (`bench_or_dissolve` callers) are unchanged: they keep
   the alert-board entry and the one-time "Program Down" guidance. Add a
   milestone `NotificationKind` for this, fired every time. Goal 3's [D]
   can silence it. A staff member who *breaks off for repairs*
   (`game/base/repair.rs::admit_the_badly_hurt`) isn't downed in this
   sense and doesn't fire it (my call). The builder finds every path that
   downs base staff at the base and fires from each one.
6. **The player's own outposts get focused alerts.** That means the
   player-founded outposts (`resources::Outposts`), not towns or
   settlements. Today stale, declining, raided and dark go only to the
   alert board via `alerts::post`. Each also fires a focused notice; the
   board entry stays. Add a new event, **outpost reached a new growth
   tier**. Nothing fires for it today: `outposts::tier` is derived on read,
   so it needs a latch beside `Outpost::announced`.

## Non-goals

- Towns and settlements (`SettlementGrown`) stay centred. They aren't the
  player's.
- No change to the alert board itself, or to which notices exist other than
  goals 5 and 6.
- No pause change: the game already pauses during every notice, because
  ticks run only in `Mode::Playing`.
- No camera switching between maps (see Constraints).

## Anti-goals

- The panel or marker hides the subject: that is the bug this fixes.
- A focused alert that marks the wrong thing, or an empty spot, because the
  subject moved or died between posting and showing. Fall back to the
  centred panel.
- Repeating alerts (raided outpost, program down) that the player can't
  silence.

## Constraints

- **Paused.** Notices already pause the game, and that stays.
- **Same map only.** A notice can fire while the party is underground (the
  Stack has no camera, `render/base/playing.rs:264`) or on a different
  surface than the subject. When the subject isn't on the map the
  player is currently viewing, draw the ordinary centred panel and offer no
  marker. Watch follows the same rule `start_watching` already enforces
  (`Game::watch_position` returning `None`).
- **Resolve the subject at show time, not post time.** Notices queue, and an
  entity can move or vanish while it waits. The marker reads the subject's
  position when the notice is drawn. The camera centre follows that, the
  way `watch_center` resolves `watching` each frame.
- **Seams** (`.claude/rules/seams-notifications.md`, read the `seams` skill
  before changing them):
  - "The screen has no scroll and is a fixed 75% panel." The docked panel
    is a second, narrower shape, so the fits-on-screen census
    (`the_tallest_shipped_notification_fits_its_screen`) must also measure
    every focused kind against the docked panel, at the smallest supported
    window. Update the seam's three places (graph, skill, rules file).
  - `def` and the fired-by-site census are exhaustive matches. The new
    kinds get arms in both.
  - Guidance opt-outs live in `Profile::ftue`, and the engine's copy is
    authoritative during a run. Milestone opt-outs follow the same shape:
    D goes through `Game` and then `flush_profile_writes`.
- **Drawing seam:** the brackets and line are drawn through `Painter`
  only (`.claude/rules/drawing-seam.md`).
- **Architecture:** gui never touches `World`. The subject reaches app-core
  and gui as plain data on `Notification`, resolved through a `Game`
  method like `watch_position`.
- **Profile format:** milestone opt-outs are a new `#[serde(default)]`
  field, stored as `Vec<String>` of kind slugs (the `ftue.never`
  precedent, so a retired kind is inert). This is not a
  `SAVE_FORMAT_VERSION` change. Needs a profile save→load test.

## Decided

- **Look:** option C, corner brackets pulsing plus a leader line.
- **Panel side (my call):** the side opposite the subject's screen
  position after centring, which keeps the line short and the camera move
  small. Right when there's no clear preference.
- **Watch reuses `App::watching`.** No new follow mechanism.
- **Don't show again is per kind**, not per subject.

The builder check raised these, and I settled each one myself:

- **The queue isn't saved** (`notifications.rs`: nothing reaches disk).
  A notice names its subject when it's posted, as an entity or a tile.
  Its position is read when it's shown. A reload drops queued notices, as
  it does today, so there's no save-format change.
- **The marker and Watch use one position source**, `Game::watch_position`
  for entities and the tile itself for tiles. When there's no position,
  the notice draws centred.
- **[D] goes only on kinds that can fire more than once in a run.**
  `BaseAiAwakening` and `BaseAiFused` fire once per run and get no [D].
  `BaseAiLevelUp` does.
- **Silencing a kind skips its screen only.** Any alert-board entry or log
  line still happens. The milestone opt-out and the guidance "never" list
  are separate: silencing staff-down doesn't silence the "Program Down"
  guidance, and the reverse is true too.
- **Outposts get one notice per outpost per event.** The tier latch is
  unsaved and reseeded on load, as `Outpost::announced` is. Ticks only run
  in play, so a tier can't be reached while the game is closed. Founding
  an outpost doesn't fire a tier notice, and neither does dropping a tier,
  because the declining-trend alert already covers that.
- **"On the displayed map"** needs a `Game` method answering whether an
  entity or tile is on the surface the player is viewing. There isn't one
  today (outpost keys carry no zone id), so the builder adds it.
- **The census window** is the existing census's `smallest_panel()` size,
  measured against the docked panel.
- **The pulse is always on.** The game has no reduced-motion setting.

## Left to the builder

- The subject's representation on `Notification`: an entity, a tile, or
  an enum of both. Outposts and the base anchor are tiles, programs are
  entities. Also how `FtueTrigger::Event` gains an optional subject for
  "Your Base".
- The docked panel's width and height fractions. Mockup: about 38% wide
  and 88% tall, with a 2% margin.
- Where the camera centre override lives for the duration of the notice.
  `watch_center` is the precedent.
- Copy for the two new kinds (program down, outpost tier), plus focused
  copy for the outpost trend, raid and dark alerts. Plain language
  (`public-copy-must-be-plain-language`), no tick vocabulary.
- Hint line layout: `[Enter] Continue  [W] Watch  [D] Don't show again`,
  with only the keys that apply.
- Whether an ADR is needed. Check `docs/adr/README.md`'s pattern index
  against "a notification carries a map subject".
