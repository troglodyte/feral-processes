# Notifications

- **A notification is a catalogue plus a queue plus one equality.**
  `assets/notifications/` is `NeedDb`'s seam again, deletable and inert —
  never gate a trigger, a system or the screen on the db being non-empty.
  `Game::notify(id)` is the one door and the queue holds **resolved**
  values, `ActiveContract`'s rule.
- **Two repeat policies, and the third was rejected on its name**: once per
  *run* would latch on the session-only queue and so mean once per
  *session*. `OnceEver` latches on `achievements::Profile` — not the save
  file, cross-run, and riding the **existing** `PendingProfileWrites`
  channel, flushed *below* `after_tick`'s `in_arena()` return, so **"an
  arena session touches no disk" holds by omission**. Don't add a check.
  That channel took a second *field*, never a second resource.
- **Achievements are a second *source*, not a second door** — built from the
  achievement def's own `name`/`description`, so a `.ron` per achievement
  repeating it is the copy that drifts.
  `every_shipped_notification_is_fired_by_a_named_site` is the whole rule
  that a shipped def is fired by something; there is no `trigger:` field to
  derive it from, `MEMORY_TRIGGERS`' shape.
- **The timing rule is one equality**, `show_next_notification` returning
  unless `mode == Mode::Playing` — every picker, fight and text entry falls
  out of it, where a list of safe modes is `is_battle`'s history. **What does
  not fall out is a burst**: `handle_notification_key` pops the next itself,
  since `after_tick` runs on a *tick* and a dismissal is not one, so the
  second notice would read as a swallowed keypress.
- **The screen has no scroll and draws no popup**, so height is a layout
  constraint (`the_tallest_shipped_notification_fits_its_screen`, verified
  by mutation) and it belongs in `needs_status_banner` and `ALL_MODES`. Art
  is the **sprite seam unchanged** — substitutes for the glyph, never beside
  it, both halves asserted.
