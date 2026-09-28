---
paths:
  - "**/notif*"
  - "**/notify*"
  - "**/alerts*"
  - "**/achievements*"
---

# Load-bearing seams: Notifications

One sentence each, the rule alone. The trap is in the `seams` skill; the argument is `seam:<slug>` in the memory graph.

- **A notification is a table plus a queue plus one equality, and it is not
  content.** `crates/engine/src/notifications.rs` is the whole feature;
  there is no `assets/notifications/`.
- **`def` and both censuses are exhaustive matches**, `cell_mark`'s rule —
  a lookup with a fallback ships a new variant blank.
- **`NotificationKind::latch_key`'s strings are `profile.ron`'s format and
  the variant names are not**, which is why `seen_notifications` is a
  `Vec<String>`: a retired key must be inert, not a parse error that costs
  the player their achievements.
- **Two repeat policies, and the third was rejected on its name**: once per
  *run* would latch on the session-only queue and so mean once per
  *session*.
- **Achievements are a second *source*, not a second door** — built from the
  achievement def's own `name`/`description`, so a per-achievement copy of
  that prose is the copy that drifts.
- **The timing rule is one equality**, `show_next_notification` returning
  unless `mode == Mode::Playing` — every picker, fight and text entry falls
  out of it, where a list of safe modes is `is_battle`'s history.
- **The low-Power notice is a state read once a tick, at
  `LOW_POWER_ATTACK_THRESHOLD` rather than a fraction of its own**, because
  the drain that carries most runs across is a bevy system with no `Game` to
  notify from.
- **`DownedProgram` is gated on `StructureDef::recovery` standing, not on
  the Bay's id**, and fires from `bench_or_dissolve`'s Forgiving arm.
- **The screen has no scroll and is a fixed 75% panel, not a
  `draw_popup`**, so height is a layout constraint measured against the
  *panel* through the renderer's own `block_height`
  (`the_tallest_shipped_notification_fits_its_screen`, verified by mutation)
  and it belongs in `needs_status_banner` and `ALL_MODES`.
- **`alerts::post` is the one door onto the alert board, a free function
  because `set_machine_status` has no `Game`, and collapse identity is kind
  plus subject.**
