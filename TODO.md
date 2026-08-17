# Todos
2. some ambiguity|routines for examining entities
3. environment effects
4. unorthodox solutions
5. high stakes reward
6. easter eggs (hidden commands & effects)
7. remove 'move abilities' and use routines
8. more machine learning
11. structures to keep base
12. next zone unlocks are research -> upgrade base to zone 2
21. Four perk candidates considered on 2026-08-14 and passed on, each for a
    reason that is about the hook rather than about the idea. Recorded so the
    next batch doesn't re-derive them.
    - **Build radius.** `Game::build_radius` is cached on
      `resources::Platform` with exactly three writers, all of which also
      write `center`. A perk needs either a fourth writer or the slab stays
      un-widened until the next stamp, which reads as the purchase doing
      nothing.
    - **Posted-program work speed.** `work_ticks_for` bakes the rate into
      `Task::required` at assignment, so buying it changes nothing until
      every cronjob is re-posted. A live version hooks
      `task_progress_system`'s per-tick increment instead, which is the
      bigger change of the two and the one worth doing if this is wanted.
    - **Raid defense.** `RAID_DAMAGE` is 4 and a Shield gives 2, so the perk
      would cap out at 4 levels and then be worth nothing. Perks are uncapped
      steady stacks; a knob with a 4-level ceiling is the wrong shape for one.
    - **Trader sell rate.** `TradeDef::sell_rate` is a `u32` multiplier, so a
      fractional perk changes the arithmetic — and it is the closest thing
      here to reopening the unbounded-income hole the scan perk was deleted
      for, since a Mining Node produces sellable salvage forever. Teardown
      shipped instead: bounded by fights taken, which is the spine.
22. Taking the thought of zones away — brainstormed and **parked** on
    2026-08-17 with no shape chosen. The itch is that zones are
    interchangeable and breaching is a grind whose only reward is advancing;
    the crux found is that `min_zone` gates on Portals *funded* rather than
    danger *beaten*, and rekeying them to a high-water mark is separable from
    the world shape. Three candidate shapes, two open questions and two
    corrected seam docs are written up in
    `docs/superpowers/specs/2026-08-17-zones-as-difficulty-parked.md`.
    Supersedes or absorbs item 12. Note in passing:
    `Game::distance_stat_multiplier` **does not exist** and six doc comments
    still name it — deleted 2026-08-05 in `30608eb`, worth fixing whether or
    not this is picked up.
23. Infinite synergy and item stacking — explored and **parked** on
    2026-08-17 with no shape chosen. The headline finding is that **there is
    nothing to stack**: all 31 gear items and all 18 affixes are flat
    `atk`/`def`/`decompiler` lines, and exactly one item in the game has a
    side effect at all (`patch_routine`, Mitigation 10). Six independent
    closures already block compounding, no two by the same mechanism — the
    largest reachable stack today is 35% mitigation. So the crux found is
    that **burn-out is not a governor you add before the content, it is what
    you trade for opening one specific closure**; pick the closure first.
    Three candidate shapes, three open questions (the save-format one is
    where the diminishing counter resets) and one doc-precision note are
    written up in
    `docs/superpowers/specs/2026-08-17-item-synergy-burnout-parked.md`.
    Note in passing: backlash cannot be denominated in Trace —
    `Game::raise_trace` returns silently on the surface, so it would be a
    mechanic that only exists underground.
25. base needs power to run, structures consume power, and power rechargers produce power. for now power rechargers can be anywhere in the base, no proximity. requires more power rechargers for more buildings.
27. add visual indicator of entity in the stack
28. shields via outside battle routine
29. zone changes will give you access to new materials via the mining node so that you can make more advanced structures, etc

# Bugs
1. spawned entities tend to gather around the players base, when traveling outside this group so that it's no longer on the screen, the entity population is almost non existent. we've run into this issue before, i think it needs further tuning
2. A companion-borne buff row overruns the map's status column. Measured
   2026-08-17 at the 1440x900 geometry `ui_metrics` is calibrated for: a row
   whose name fills `BUFF_NAME_W` draws 614px into 417px of room once the
   trailing `(holder)` tag is on it — ~200px off the panel, and it predates
   the until-rest work by a long way. `draw_status_buffs` measures nothing
   and `draw_row` clips rows vertically but never horizontally, so it runs
   off in silence; the boxed battle copy of the same panel is fine, because
   `buff_panel_width` measures. Same class as the unbounded player-authored
   name the Party popup carries.
   `render::field::tests::the_widest_until_rest_buff_row_fits_the_status_column`
   bounds the player's own rows and deliberately stops short of this one.
