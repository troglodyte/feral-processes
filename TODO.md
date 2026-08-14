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

# Bugs
3. when the battle log goes beyound the visual buffer, it should scroll, it currently gets truncated
4. heals in battle log should be green
1. A maxed-fusion Gold affixed copy overflows the inventory popup: the widest
   the shipped assets can build is 1311px into a 1243px body at zone 10, and
   the row runs off the right edge taking its equip tag with it. The excess is
   `equip_preview_tag`'s `" - maxed"`, appended on the stated grounds that
   this screen has the room. Pre-existing, measured 2026-08-13;
   `no_shipped_inventory_row_overflows_its_popup` covers every tier below
   maxed and excludes that case by name.
2. The roster's widest row overflows the Party popup the same way, and by far
   more: a maxed-fusion rare program with a full custom name, a zone tag, a
   quality tier, the wield mark, an activity and CRITICAL measures 1636px into
   the same 1243px body — 393px over. Pre-existing and worse than Bug 1
   because the name half is player-authored rather than shipped, so no census
   over the assets can bound it. Measured 2026-08-13, when the `w|a|m` loadout
   cell was added; the cell is 54px of that total and the row was already
   339px over without it. The fix is a chop, not a shorter tag — the row has
   six optional tags and drops none of them.
