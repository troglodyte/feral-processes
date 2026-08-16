# Todos
2. some ambiguity|routines for examining entities
3. environment effects
4. unorthodox solutions
5. high stakes reward
6. easter eggs (hidden commands & effects)
7. remove 'move abilities' and use routines
8. more machine learning
9. i want the description of what i'm seeing as i'm walking in the stack, not with x | examine
11. structures to keep base
12. next zone unlocks are research -> upgrade base to zone 2
20. too easy to level up, lets slow that down a bit
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
22. base staff should show stats in the selection menu


# Bugs
1. if i deploy another mining node, and there's an existing work order for core fragments, that additional mining node should be manned. there's a mining node that's attached to another structure, so now all the output from the mining node is consumed by the second structure. expected behaivior i put a work order in for core fragments, and any available entity starts working on all available mining nodes until the work order is finished.
