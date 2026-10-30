# Sieges

- **The siege board is built, not `generate`d, and a siege comes in over
  the floor's edge.** `siege::board::build` seats the base's laid floor plus
  `siege_spawn_cells(floor, map)` — walkable non-floor field cells
  orthogonally next to the floor — in their bounding box; everything else
  stays `Blocked`, exactly how `Board::solid` started it. The spawn list is a
  `BTreeSet` walk, so its order is the coordinate order and never a query's.
  A sealed base (no walkable cell touches its floor) falls back to the first
  ring of walkable ground a breadth-first walk out through the rock reaches,
  up to `SIEGE_SPAWN_SEARCH_TILES`; past that there is no board and the clock
  resolves the siege off-screen. Fallback cells can be cut off from the
  floor, and a fight seated there could never end: `Game::
  sealed_siege_withdrawal`, asked once a round from `tactical_round_upkeep`,
  withdraws the whole pack once `SIEGE_SEALED_HOLD_ROUNDS` full rounds have
  passed and no besieger has a terrain walk to floor
  (`raiders::any_reaches_floor` — terrain only, so a crowd at a choke is not
  a seal; structures never seal a base, only terrain does). The fight
  still settles as a win, and ranged besiegers can hurt during the hold. One besieger with a path keeps the whole pack
  in the fight; the round counts from the opening, so a path that closes
  late withdraws at once. Besiegers that dig are deferred, not forgotten. `open_siege_with`
  draws one `GameRng` index over the list and seats raider *i* at
  `spawns[(start + i) % n]` — coordinate order, not a walk along the edge,
  so consecutive raiders share a column rather than a run of edge — and
  `siege_door` is `spawns[start]`. The pack's species is drawn at the first
  spawn cell any species lives on: `habitat_pools` reads the raw field
  biome, the floor itself is habitat for nothing, and the first cell alone
  is often mined ground (`Biome::Excavated`) whose empty pool once sent a
  siege the player stood in off-screen. `habitat_pools` draws no RNG, so
  the walk keeps the stream.
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
  `SaveData::creatures`, so `SiegeSave` carries `player_order` of its own;
  the player's board cell is re-derived from their `Position` (honest on the
  field — a tactical fight writes no world `Position`) through `to_board`,
  falling back to the door, and seated with `place_nearby`. The old
  `player_cell` field existed only for the anchor pin and went with it.
- **A besieger is an ordinary field body that the wild-population systems
  must never count.** `ensure_local_population` and its kin query
  `Without<Besieger>`, so a besieger is never counted or respawned against
  as if it were a wild creature. A capture strips the marker itself:
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
  `Durability` on it at all). `besieger_turn` and `besieger_walk_toward`
  test *melee range* to the door rather than equality with it: the door is
  a spawn cell another raider may be standing on, and one body to a cell
  means a carrier could otherwise never arrive to leave.
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
- **A nemesis leader survives its own siege.** `siege_pack` is no longer
  always a fresh sector pack: `open_siege_with` seats either
  `spawn_siege_pack`'s output (`open_siege`) or a nemesis band's real bodies
  (`nemesis_march`), and the empty-pack and nobody-seated refusals live in
  it so both callers get them — a refused fresh pack is despawned, a refused
  band is kept. The marching leader carries `NemesisHome`, and that marker is
  what exempts it from every place a besieger is deleted: the end-of-fight
  besieger sweep (`Without<NemesisHome>`), the morale-break exit in
  `siege::raiders` (it strips `Carrying`/`StolenFrom` and leaves the body
  alive instead of despawning it) and `mark_nemeses` (its grudge is not
  raised twice). Teardown then calls `nemesis_return_home`: marker and
  `Besieger` removed, leader back at its home cell, `NemesisMuster` restarted
  at 0, and `escalate_nemesis` for grudge +1 and a rarity rung. A leader the
  player decompiled mid-fight only loses the march. Drop the exemption at any
  one site and the leader is deleted, left wandering the base as a besieger,
  or escalated twice.

- **A fought siege's damage to the defending side is cut by
  `Game::siege_shields_over`** — "enemy hit" means damage to a non-hostile
  body while `siege_pack > 0` (every source on a siege board is the
  besiegers', and a tantrum never runs in a battle). Bodies cut it in
  `combat_damage`; structures through `shielded_structure_damage` (never
  below 1 for a landed hit). Each Shield that softened anything is recorded
  by anchor in `TacticalBattle::shields_softened` (saved in `SiegeSave`) and
  pays one coil in `spend_softening_shields` at teardown, not per hit; a
  destroyed Shield is simply not found. The off-screen
  `resolve_siege_offscreen` still reads the base-wide `structure_raid_defense`,
  so radius means nothing to a siege fought off-screen.
