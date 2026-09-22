# Sieges

- **The siege board is built, not `generate`d.** `siege::board::build`
  flood-fills four-way from `BASE_EXIT_CELL` through `BaseGrid::walkable`,
  seated in the bounding box of what it found, and the fill's own frontier
  *is* the wall — nothing here does a separate wall pass, because whatever
  the fill never reaches simply stays `Blocked`, exactly how `Board::solid`
  started it. That is also what makes a sealed-off pocket excluded from the
  board by construction rather than by a check: a cell with no walkable path
  to the door is never visited, so a player who walled off their far works
  has genuinely protected it. Rock is never breached, which is why the fill
  is 4-way and not 8-way — a diagonal step would let the board slip past a
  wall no corridor was ever cut through.
- **A siege never touches `fights_tactically` or `start_battle`.**
  `Game::open_siege` mints its own `BattleSpec` and opens `TacticalBattle`
  directly, so a siege is tactical whatever the player's battle-map profile
  toggle says (design decision #2) — the one place a fight's *model* is not
  chosen by inspecting the pack the way `start_battle` chooses it.
- **A body with nothing in reach takes no turn.** `skip_disengaged_turns`
  thins a round to the bodies actually in contact, which is the whole
  mitigation for `TacticalBattle` documenting a ~13-body ceiling on a
  linear-scanned `Vec` while a developed base holds a hundred. Three call
  sites all matter: `open_siege` calls it once before the very first actor
  (nothing else calls it before a turn has been handed on, so a disengaged
  body that won initiative outright would otherwise get a full AI turn on
  round one); `hand_on_turn` calls it on every ordinary wrap; and
  `besieger_leaves` calls it explicitly because `TacticalBattle::remove`
  hands the cursor to whoever now sits where the departed body was *without*
  going through `end_turn`, so nothing else on that path would otherwise
  catch a disengaged body landing on the cursor. Staff never reinforce —
  "a body four rooms from the fighting does not walk toward it" is the
  accepted consequence, not a bug — and they act at all only through
  `tactical_ai_actor`'s fourth arm, gated on `battle.siege_pack > 0`: there
  is no defence-roster screen, so a staff body is never commanded.
- **`TacticalBattle` still gains no `Serialize`.** The precedent is exact:
  a sortie's membership rides `CreatureSave::sortie_index` rather than a
  member list, because entity ids are not stable across a save. A siege in
  progress is assembled from scratch by `siege::persist::assemble` (board
  cells, round, turn, actions left, `siege_pack`) into a `SiegeSave`, and
  restored by `siege::persist::restore`, which rebuilds a fresh
  `TacticalBattle` rather than deserialising one. Membership rides
  `CreatureSave`'s own `siege_cell`/`siege_order`/`besieger`/`stolen_from`
  fields, additive behind `#[serde(default)]`. The player is never one of
  `SaveData::creatures`, so `SiegeSave` carries `player_order` and
  `player_cell` as fields of their own — `player_cell` exists at all only
  because "Base-space `Position` is pinned to the anchor" (an existing
  seam) means `Position` cannot answer for the party's actual board cell on
  reload, and `restore` runs *before* `Game::restore_locale`, so
  `Game::base_pos` has nothing to answer either way at that point.
- **A besieger is the third base-space `Position` body**, after a posted
  program and a `DigSite` — the seam "`Structure` no longer answers 'which
  space is this?' on its own" sharpens further. `Game::stands_in_base_space`
  is the one door that matters: without its `Besieger` arm, a raider's
  base-space cell reads as an ordinary zone-surface tile the two coordinate
  spaces happen to alias onto. Wild-population systems (`ensure_local_
  population` and its kin) query `Without<Besieger>` for the same reason a
  besieger must never be counted or respawned against as if it were a
  surface creature. A capture strips the marker itself:
  `Game::decompile_body` converts a besieger in place (never despawns and
  respawns it), removing `(Besieger, Carrying, StolenFrom)` in the same
  breath it removes `Hostile`. The end-of-fight stray sweep in
  `combat_teardown.rs` queries `(With<Besieger>, Without<Tamed>)` for
  exactly that reason — a besieger captured mid-fight is still `With
  <Besieger>` the instant that sweep runs, so the `Without<Tamed>` guard is
  load-bearing, not defensive style, and it also protects a save loaded
  between a capture and the sweep.
- **Structure damage on the board goes through the existing
  `damage_structure` door**, the same one a GC Entropy Sweep and
  `resolve_siege_offscreen` use, so a structure destroyed in a siege is
  destroyed exactly the way any other one is — `event` is just a different
  noun phrase (`"a siege"`) in the log line. `besieger_wreck` checks
  `Structure`'s *absence* to detect destruction rather than `Durability`'s,
  because a `Durability`-less structure makes `damage_structure` return
  before ever touching that component — reading a missing `Durability` as
  "destroyed" would remove an intact structure from the board on the very
  first swing. The Home is exactly that structure (`raidable: false` in
  `assets/structures/home.ron`, so `spawn_structure` never inserts
  `Durability` on it at all) and it always stands on `BASE_EXIT_CELL`, the
  door — so `besieger_turn` and `besieger_walk_toward` test *melee range*
  to the door rather than equality with it; one body to a cell means the
  Home itself already occupies the only cell a raider could otherwise stand
  on to leave.
- **`TacticalBattle::siege_pack` is two things at once**: the morale-break
  denominator (`siege_morale_broken` reads it as the pack's original size,
  set once at the count `open_siege` actually seated, never
  `raiders.len()`) and the "is this a siege at all" signal everywhere else —
  `0` reads as an ordinary tactical fight to `siege_morale_broken`,
  `siege::persist::assemble` (which returns `None` rather than saving a
  fight that isn't one) and `tactical_ai_actor`'s fourth arm alike. A raider
  spawned but turned away for want of a free seat near the door is never
  counted, and if *nothing* seats, `open_siege` despawns every raider and
  returns `false` rather than opening a fight with hostiles in name only.
- **`SIEGE_WARN_FLOOR_TICKS` (960, 8 minutes) is unvalidated.** The design
  doc is explicit that nothing in this repo models the walk home from a
  deep Stack frame — the number is defensible, not measured. Play it before
  trusting it, the same status `RAID_PRESSURE`-style constants started at
  and the same caveat traps' and sorties' unmeasured figures carry.
- **OPEN BALANCE ITEM — an off-screen siege can cost an established base
  literally nothing.** `resolve_siege_offscreen`'s shortfall is
  `pack_size(zone)` (capped at `SIEGE_PACK_MAX` = 12) minus `defence`, where
  `defence = total_raid_defense() + turret_defense() + staff.len() *
  SIEGE_STAFF_DEFENSE (2)`. Eight on-shift staff alone already reach 16,
  past the pack-size cap, before `total_raid_defense` or a single turret
  contributes anything — so a well-staffed base sees `shortfall == 0`:
  nothing stolen, nothing damaged, nobody benched, no log line. This is the
  design doc's own named risk ("Abstract resolution must not be the better
  option... If missing a siege is cheaper than fighting one, the incentive
  is inverted") landing exactly as written, and it shipped anyway — a user
  decision to leave it for playtesting rather than block the feature on a
  number nothing in this repo can measure without play. Don't "fix" this
  quietly by retuning `SIEGE_STAFF_DEFENSE` or the pack cap; it is an open
  question for the user, not a bug.
- **A barrier's sight screen is a `Board` overlay, never a `Cover` cell.**
  `SiegeSave` stores the board's *cells*, and `persist::restore` reseats
  every structure through `TacticalBattle::place`, which refuses a cell that
  is not walkable — so a wall written into the cells as `Cover` would drop
  off the board on the first load, silently, and grant `cover_between`'s
  evasion besides. `Board::screens` is never saved; `seat_structures` is the
  one writer (both `open_siege` and `restore` call it, which is how a load
  gets it back), and `TacticalBattle::remove` lifts it with the body, which
  is why neither of the two sites that destroy a structure on the board
  mentions sight at all. `StructureDef::swept` is the companion flag: it
  narrows the sweep's pool *without* taking `Durability` away, because
  `raidable: false` does that and would make a wall unbreakable in a siege.
