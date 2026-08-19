# Todos
2. some ambiguity|routines for examining entities
4. unorthodox solutions
5. high stakes reward
8. more machine learning
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
27. add visual indicator of entity in the stack
28. companions also have perks, special unlocks and trees for entities, allow further level progression
29. move playwer perks to .ron's so they can be modded
30. introduce AC "armor class' type of metric, affects the chance of missing both from entity side and player & companion side. 

# Bugs
