# Notifications

- **A notification is a table plus a queue plus one equality.** The whole
  feature is `crates/engine/src/notifications.rs`: `NotificationKind` is the
  census, `NotificationKind::def` is the copy. There is **no
  `assets/notifications/`** — it shipped as a `.ron` catalogue behind a
  `NotificationDb` and that half was retired, because a file cannot say
  *when* a notification fires so every one already needed a Rust call site.
  Don't reach for `NeedDb`'s loader shape here. `Game::notify(kind)` is the
  one door, returns `bool`, and the queue holds **resolved** values —
  `ActiveContract`'s rule, kept alive by `achievement_system`, which pushes a
  `Notification` built from an achievement's own prose and names no kind.
- **`def` and the two censuses are matches, and that is load-bearing.**
  `cell_mark`'s rule: a table lookup with a fallback ships a new variant
  blank, a match fails to compile until somebody writes the words. Same for
  `every_notification_kind_is_fired_by_a_named_site` and
  `tutorials_latch_and_milestones_do_not` — a `&[(kind, site)]` table is
  something a new variant can simply be left out of. `all()` is what the
  width and height censuses walk, and its array length catches an addition.
- **Two repeat policies, and the third was rejected on its name**: once per
  *run* would latch on the session-only queue and so mean once per
  *session*. `OnceEver` latches on `achievements::Profile` — not the save
  file, cross-run, and riding the **existing** `PendingProfileWrites`
  channel, flushed *below* `after_tick`'s `in_arena()` return, so **"an
  arena session touches no disk" holds by omission**. Don't add a check.
  That channel took a second *field*, never a second resource.
- **`NotificationKind::latch_key`'s strings are a file format; the variant
  names are not.** They are the ids of the deleted `.ron` files and are
  already in players' `profile.ron`. Renaming one re-shows that tutorial to
  everybody, and `Profile::load` discards the *whole* profile — achievements
  included — on a parse error, which is why `seen_notifications` is a
  `Vec<String>` and not a typed id: a retired key must be inert, not fatal.
  The round-trip test cannot see either fault (it writes and reads the same
  build's keys); `a_profile_written_before_this_refactor_keeps_its_latches`
  is the one that can.
- **Achievements are a second *source*, not a second door** — built from the
  achievement def's own `name`/`description`, so a per-achievement copy of
  that prose is the copy that drifts.
- **The timing rule is one equality**, `show_next_notification` returning
  unless `mode == Mode::Playing` — every picker, fight and text entry falls
  out of it, where a list of safe modes is `is_battle`'s history. **What does
  not fall out is a burst**: `handle_notification_key` pops the next itself,
  since `after_tick` runs on a *tick* and a dismissal is not one, so the
  second notice would read as a swallowed keypress.
- **The screen has no scroll and draws no popup**, so height is a layout
  constraint (`the_tallest_shipped_notification_fits_its_screen`, walking
  `all()`, verified by mutation) and it belongs in `needs_status_banner` and
  `ALL_MODES`. Art is the **sprite seam unchanged** — substitutes for the
  glyph, never beside it, both halves asserted — and `def.sprite` is that
  live hook, not an unused field.
- **The low-Power notice is a state read once a tick, not a hook on a
  spend**, and it is the one kind that fires that way. Power leaves the
  player through `Game::spend_power` *and* through `needs_tick_system`'s flat
  per-tick drain, and the second — how most runs cross — is a bevy system
  with no `Game` to notify from, so `Game::note_low_power` reads the reserve
  from `tick_inner` instead. `Repeat::OnceEver` is what keeps a state read
  from being a per-tick alarm. **The threshold is
  `tuning::LOW_POWER_ATTACK_THRESHOLD`**, not a fraction of its own: it is
  half of `POWER_MAX` today, but the number that matters is where
  `battle::power_attack_multiplier` starts docking attacks, or the screen
  says "your attacks start to weaken" on a tick where they do not.
- **`DownedProgram` is gated on the Bay, not on the program.** It fires from
  `bench_or_dissolve`'s Forgiving arm — the one door a *death* goes through —
  and only while `Game::repair_bays` is empty, since a base that has one has
  nothing to be told. The gate reads `StructureDef::recovery`, not
  `"repair_bay"` (`dispatches_sorties`' rule), and its copy has to keep step
  with `Game::add_to_party`'s refusal, the other place the game says a downed
  program needs a Bay.
