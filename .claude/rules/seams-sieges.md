---
paths:
  - "**/siege*"
  - "**/siege/**"
  - "**/nemesis*"
---

# Load-bearing seams: Sieges

One sentence each, the rule alone. The trap is in the `seams` skill; the argument is `seam:<slug>` in the memory graph.

- **The siege board is built, not `generate`d** — `siege::board::build`
  flood-fills from `BASE_EXIT_CELL` through walkable base-space cells, and
  the fill's own frontier is the wall.
- **A siege never passes through `fights_tactically` or `start_battle`** —
  `Game::open_siege` is its own door, so a siege is always tactical whatever
  the profile toggle says.
- **A body with nothing in reach takes no turn** — `skip_disengaged_turns`
  and its guards, so staff never reinforce and only act under the AI's own
  siege arm in `tactical_ai_actor`, never commanded.
- **`TacticalBattle` still gains no `Serialize`** — a siege in progress is
  assembled into `SiegeSave` (`siege::persist::assemble`/`restore`) with
  membership riding `CreatureSave`'s `siege_cell`/`siege_order`/`besieger`/
  `stolen_from`, and `player_cell` is saved because base-space `Position` is
  pinned to the anchor and cannot answer for the party's board cell.
- **A besieger is the third base-space `Position` body**, after a posted
  program and a `DigSite` — `stands_in_base_space` is the door, wild
  population systems exclude it by `Without<Besieger>`, a capture strips the
  marker in `decompile_body`, and the end-of-fight sweep is `Without<Tamed>`
  for exactly that reason.
- **Structure damage on the board goes through `damage_structure`** — a
  `Durability`-less structure (the Home, `raidable: false`, standing on the
  door cell) is untargetable, and that is also why raiders leave from
  melee range of the door rather than by stepping onto it.
- **`TacticalBattle::siege_pack`, the count actually seated, is both the
  morale-break denominator and the "is this a siege" signal** — `0` reads as
  an ordinary tactical fight everywhere that checks it.
- **A barrier's sight screen is a `Board` overlay, never a `Cover` cell** —
  `Board::screens`, unsaved, set by `seat_structures` and lifted by
  `TacticalBattle::remove`.
- **`SIEGE_WARN_FLOOR_TICKS` is unvalidated** — 8 minutes, picked as a
  defensible number, not a measured one; nothing in this repo models the
  walk home from a deep Stack frame.
- **OPEN BALANCE ITEM: an off-screen siege can cost an established base
  nothing.** `resolve_siege_offscreen`'s shortfall is `pack_size(zone)`
  (capped at `SIEGE_PACK_MAX` 12) minus defence, and 8 on-shift staff alone
  (`SIEGE_STAFF_DEFENSE` 2 each) already clears that cap before
  `total_raid_defense` or a turret adds anything — shortfall 0, nothing
  stolen, nothing broken, nobody benched. Left as-is for playtesting,
  contrary to the design's "missing a siege must not be cheaper."
- **`siege_pack` can come from a nemesis band (`open_siege_with`), and its leader survives the besieger sweep and the morale-break exit through `NemesisHome`** — `nemesis_return_home` sends it home with grudge +1 via `escalate_nemesis`.
