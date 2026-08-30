# Notifications

One `.ron` file per notification. A notification takes the whole screen for
a moment and is dismissed by any key — reserved for the handful of things
that are genuinely irreversible, or that the player has never seen before.

Drop a file in and it exists. **Deleting this whole directory is a supported
way to play**: an absent directory loads silently, no screen ever opens, and
the game is exactly the pre-notification game. A malformed file costs that
one notification and warns; it never stops the game starting.

## The seam

The catalogue is **data**; the triggers are **Rust**. A `.ron` file cannot
say *when* it fires, because when it fires is a hook into a particular
moment in a particular function. An engine call site names an id, and this
file says what the player reads.

That means **a notification nothing fires is a notification nobody sees**.
`tests/assets.rs` pairs every shipped id with the site that fires it, and a
def shipped without a pairing fails the build. If you are modding, your own
files are not covered by that census — check the id against the table below.

## Schema

```ron
(
    id: "tutorial_first_descent",
    title: "The Stack",
    body: "Below the on-ramp the ground stops being a map...",
    sprite: "stack_entrance",
    glyph: '>',
    color: Cyan,
    repeat: OnceEver,
)
```

| Field | Required | Meaning |
|---|---|---|
| `id` | yes | Unique. What a call site names. Two files claiming one id resolve by filename order. |
| `title` | yes | The heading. Drawn large and **does not wrap** — keep it short. |
| `body` | yes | The paragraph under it, wrapped at draw time. `\n\n` starts a new paragraph. |
| `sprite` | no | A name in `assets/sprites/`. Falls back to `glyph` when there is no such texture, which today is almost always. |
| `glyph` | no | The character drawn above the title when there is no sprite. Defaults to `!`. |
| `color` | no | One of the `GlyphColor` hues. Defaults to `White`. |
| `repeat` | no | `Always` (default) or `OnceEver`. |

### Placeholders

A `title` or `body` may carry `{hole}` placeholders, filled by the call site
through `Game::notify_filled`. That is what lets one file be read for many
subjects: `onboarding_mission.ron` is a single briefing filled from whichever
contract the chain has just handed out, rather than eleven files each
repeating a mission's own name and description.

The holes a def uses are the ones its site fills — there is no catalogue of
them, because a placeholder is a contract between one file and one call site.
A hole nobody fills is **left standing** rather than blanked, so it shows up
as `{objective}` on screen and in a census instead of reading as a missing
word.

Shipped holes:

| Def | Holes | Filled by |
|---|---|---|
| `onboarding_mission` | `{name}`, `{objective}`, `{description}` | `Game::ensure_tutorial_held`, from the mission's own `ContractDef` |

**A templated def's real height is the filled one.** The screen has no
scroll, so `every_onboarding_briefing_fits_its_screen_once_filled` measures
the template filled with every shipped mission — the plain
`the_tallest_shipped_notification_fits_its_screen` would be measuring
`{description}`, seventeen characters where a paragraph will go.

### `repeat`

`Always` fires every time its site is reached — right for news about *this*
moment, like a breach or a contract landing.

`OnceEver` fires once and never again, on this machine, **across every run**.
That is what a tutorial tip wants.

The onboarding chain's briefing is deliberately `Always`, and named
`onboarding_mission` rather than `tutorial_*` to say so: the chain runs on
every new game, so a briefing latched across runs would leave a second
playthrough's eleven missions unexplained. The latch lives in `profile.ron` beside your
achievements, not in a save, so starting a new run does not re-show them.
Deleting `profile.ron` shows them again (and costs you your achievements).

There is deliberately no once-per-*run* policy. Its latch would have to live
in memory, so it would really mean once per *session* and fire again after a
reload — a name that lies is worse than a missing policy.

## A fired notification can carry one more line than the file does

What a screen draws is `Notification`, not `NotificationDef` — the resolved
value the queue holds, built from your file at the moment it fires. Most
fields are copied straight across, but a firing site may attach a `detail`
line the `.ron` file has no way to author: a figure only the engine knows at
that moment, such as `Game::complete_contract` naming what a contract just
paid. There is no `detail:` field to set here, and there never will be — it
is a parameter to the door (`Game::notify_with_detail`), not content.

## Achievements do not belong here

An achievement earned already raises a notification, built from that
achievement's own `name` and `description` in `assets/achievements/`. Do not
author a second file here repeating the text: the copy that drifts is the one
nobody runs.

## Shipped ids and what fires them

| id | Fires at |
|---|---|
| `tutorial_base_founding` | `Game::place_structure`, founding the Home |
| `tutorial_first_descent` | `Game::descend_to` |
| `tutorial_first_raid` | `Game::raid_check`, as a sweep opens |
| `tutorial_first_work_order` | `Game::queue_work_order`, on success |
| `milestone_breach` | `Game::enter_next_zone` |
| `milestone_contract` | `Game::complete_contract` |
