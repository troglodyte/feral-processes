# Level-up summary (TODO #57)

**Status:** design approved in conversation 2026-09-24; spec awaiting review.

## Intent

A player level-up is today a log line plus indented stat rows, usually
buried in a battle results tally. The player cannot tell whether they got
*stronger in a way that matters*. This adds one page, shown after the
results screen, reading back what the level(s) changed — stats, and what
those stats mean against a typical program of the current sector.

Decided in conversation:

- **When:** after the battle results screen closes; immediately (on the
  next return to the map) when XP arrived outside a fight.
- **Figures:** stats before → after; hit chance and per-swing damage
  against a typical foe; swings to win / its swings to down you; Perk
  Points and Decompiler skill gained.
- **Scope:** player level-ups only. Not companions, not perk purchases.

## The page

```
LEVEL 5 → 7
  Max Integrity   84 → 102
  ATK             14 → 18
AGAINST A TYPICAL SECTOR 3 PROGRAM
  Hit chance      71% → 76%
  Per swing      9.4 → 12.1
  Swings to win    8 → 6
  Its swings to down you   11 → 13
TO SPEND
  +2 Perk Points (3 unspent)   [P] Perks
  +2 Decompiler skill
                                  [Esc] Close
```

Several levels gained before the page shows are **one page** (5 → 7). A
stat row that did not move is omitted (mitigation never grows with level).

## Engine

**One new piece of state: `resources::PendingLevelUp(Option<LevelSnapshot>)`.**
`Game::award_player_xp` writes it on a levelling award **only when it is
`None`**, from the player's state *before* `add_xp` runs, so the "before"
column is the level the player held when the first of several level-ups
began. It is **not saved** — `RunFeats`' precedent: quitting between the
level-up and the page loses the page, never the levels.

`LevelSnapshot` holds `level`, `max_hp`, `mitigation`, the Perk Point and
Decompiler skill counts, and the player's **`battle::Combatant`** as
`combatant_profile` resolved it at that moment. Storing the resolved
combatant rather than re-deriving one at the old level is deliberate: a
re-derivation would be a second copy of `combatant_profile` with a level
parameter, and the copy is what drifts.

**`Game::take_level_up_report() -> Option<views::LevelUpReport>`** is a
drain (`take_notification`'s shape): it builds the report from the
snapshot and the player's current state, then clears the resource.

**The typical foe** is `balance_sim::median_ordinary_species` at the current
`ZoneLevel` — the same definition `Game::worn_detail`'s `NominalHostile`
already uses — with full stats from `balance_sim::wild_stats_at_zone` (made
`pub(crate)`; a call, not a copy). Its `Combatant` is `accuracy_of` /
`evasion_of` at `base_speed` and the zone level, its `atk`, and its basic
attack's range. The foe is the **same** in both columns; only the player
moves. It is a projection, not a spawn guarantee — the page names the
sector, as the gear page does.

**Every figure is a call:**

- hit chance — `battle::hit_chance(player.accuracy, foe.evasion)`
- per swing — `battle::expected_damage(player, foe)`
- swings to win — `ceil(effective_hp(foe) / expected_damage(player, foe))`
- its swings to down you — `ceil(effective_hp(max_hp, mitigation) /
  expected_damage(foe, player))`

where `effective_hp` is `battle::effective_hp`. The player's "now"
combatant comes from `combatant_profile` with the same swing the snapshot
used, so the two columns differ only by what the level changed. The
player's swing is their basic attack as a battle resolves it — located in
the plan, not restated here.

`views::LevelUpReport` carries `from_level`, `to_level`, the changed stat
rows (`progression::StatRow`, so the arrow format is the log's own), the
zone, the four before/after figures, and the Perk Point / Decompiler
deltas plus current unspent Perk Points.

## App-core

- **`Mode::LevelUp`**, added to `ALL_MODES` and `needs_status_banner` — the
  list does not fail to compile, and an omission ships a blank screen.
- Opened from `after_tick` under **`show_next_notification`'s gate**
  (`mode == Mode::Playing`), and checked **before** the notification queue,
  so `LevelCapReached` shows after the page. Leaving the results screen
  returns to `Playing`, so no change to `leave_battle_result` is needed.
- The report is held on `App` while the page is open (as
  `pending_notification` is).
- `Esc` closes to `Playing` and lets the next notification through. `P`
  (uppercase — lowercase letters are row selectors) closes and opens
  `Mode::Perks`, which is `Locality::Anywhere`.

## Gui

`render/level_up.rs`, drawn through `Painter` only. Fixed rows, **no
scroll**, so height and width are layout constraints held by a census test
over the widest reachable report (largest numbers, longest sector label),
measured with `paint::with_painter`.

## Tests

Engine:
- two level-ups inside one fight produce one report whose `from_level` is
  the level held before the fight;
- the report drains once (second `take` is `None`);
- each figure equals a direct `hit_chance` / `expected_damage` /
  `effective_hp` call for the same combatants;
- an award that does not level leaves no report;
- an unchanged stat produces no row.

App-core:
- level up in a fight, leave the results screen → `Mode::LevelUp`;
- a queued notification waits behind the page and shows on `Esc`;
- `P` lands in `Mode::Perks`.

Gui: the widest report fits its panel.

## Out of scope

Companion level-ups, perk purchases, persisting an unshown page across a
quit, and filtering the typical foe to what can actually spawn in the
current biome (the `NominalHostile` doc's own reason: that forks
`habitat_pools`).
