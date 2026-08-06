# Content gaps: engine capability vs. shipped assets

Surveyed 2026-07-30 by reading the `.ron` schemas in
`crates/engine/src/{structures,items_db,species,abilities,research}.rs`
against every file under `assets/`. The recurring pattern: the engine has
more mechanics wired up, tested and reachable than there is content using
them. Most of the entries below are a `.ron` file and no Rust at all.

Everything here was verified against source, not remembered. The file:line
citations are the receipts — re-check them before acting, and correct this
document if one has moved. None of it has been playtested, so the ordering
reflects capability-vs-content distance, not what the game most needs.

## Built, tested, and used by zero content

These are engine features with a complete implementation, a test, and no
asset exercising them.

### Temporary structures

`StructureDef::temporary` (`structures.rs:212`, `TemporaryDef` at
`structures.rs:98`) inserts a `Temporary` component at deploy
(`game/building.rs:101`), which `age_temporary_structures`
(`game/turn.rs:111`) ages each tick and collapses at zero. There is a
deliberate carve-out: ticks spent inside a `Game::rest` cycle don't count,
so resting beside one doesn't wear it down faster than leaving it standing.

**No shipped structure sets it.** `grep -l "temporary:" assets/structures/*.ron`
returns nothing.

A cheap, collapsing forward Recharger or portable Terminal is one file, and
gives expeditions a deploy-and-abandon option instead of permanent base
sprawl. The rest carve-out suggests whoever built this had exactly that in
mind.

### Pre-battle buff consumables

`ConsumeDef::prebattle_buff` (`items_db.rs:40`, `PrebattleBuff` at
`items_db.rs:46`) is armed by `Game::use_item` (`game/turn.rs:342`), survives
on the map because buffs only tick in battle, and is carried into the next
intrusion (`game/combat.rs:143`). It has a passing test:
`a_prebattle_buff_armed_on_the_map_is_live_at_the_next_intrusion`
(`tests/turn.rs:345`).

**No item declares one.** A combat stim — `kind: Atk, power: N, rounds: 3` —
is one file.

### Consumables in general

`ConsumeDef` supports `power`, `fatigue`, `heal` and the pre-battle buff
above, all optional so one item can do several. `power_cell.ron` is the
**only** item in the game with a `consume:` block.

Nothing in the game heals Integrity or sheds Fatigue outside of `rest`. This
is the largest single gap between what the engine does and what ships — a
repair patch and a coolant flush are two files, no Rust.

## Thin, not absent

Mechanics that work and have exactly one piece of content each.

- **Taming catalysts.** `taming_potency` is on one item (`ice_breaker.ron`).
  `taming::capture_chance` already reads potency, so tiered catalysts — cheap
  and weak, Fabricator-crafted and strong — are pure data. The tame loop
  currently has no progression axis of its own.
- **Bosses.** `is_boss` (`species.rs:204`) is on 2 of 17 species (Overseer,
  Wintermute). It buys exclusion from the normal per-tile habitat roll, rare
  spawning in its place, and — depending on which side of the ground it dies
  on — either the game's only Portal Fragment payout or a draw from the
  zone's gear band. Stats are authored in the file — there is no engine-side
  boss multiplier — so one boss per biome is 4 files.

  Two species covering four walkable biomes is thinner than it reads, because
  a biome with no boss makes every Stack under it pay nothing at all. That is
  a census, not a convention: `every_biome_a_stack_link_can_open_in_fields_a_boss`
  fails if a habitat edit uncovers one.
- **Symlink targets.** `teleport_cost` (`structures.rs:152`) is on `home.ron`
  alone. A cheap one-way waypoint needs no Rust.
- **Traders.** `trade` (`structures.rs:168`) is on `black_market.ron` alone. A
  specialist trader with different stock and rates is data. Note
  `StructureDb::strip_reserved_trade_goods` — a trader may not deal in the
  `Currency` or `CraftCurrency` item.

## One-line inconsistency

Three structures are cronjob-workable (`work: Some(...)`): Mining Node,
Research Node, Power Conduit. **Two have an `upgrade:` block; Power Conduit
does not.** Adding one is a single line and closes a gap players will feel,
since upgrading is how the base economy is meant to grow.

## Looks easy, isn't

Recorded so the next survey doesn't rediscover them as opportunities.

- **Upgrading Shield / Data Cache / Recharger Node / Compiler does nothing.**
  A tier multiplies work payout and becomes `ResourceNode::level`, and those
  four have no `work` block. The Compiler is the live one: it still ships an
  `upgrade` block, so five tiers really can be bought for 168 Core Fragments
  and really do buy nothing. An assembler's rate is its def's
  `ticks_per_unit`, which tier does not touch. Deleting those two lines is
  the fix; making tier mean something for assemblers is the feature. `raid_defense` (read flat at `game/upkeep.rs:104`) and
  `pet_slot_bonus` (read flat at `game/catalog.rs:200`) do not consult tier.
  They scale by building *more* of them — additive across every deployed
  structure — which already works.
- **New buff or status kinds are Rust.** `BuffKind` is Atk/Def
  (`components.rs:417`); `StatusKind` is Bleed/Stun (`components.rs:385`). A
  third variant means a hook in a formula, not a file — the same seam as
  `Perk`.
- **`SpeciesDef::equipment_drop` being unused is not a gap.** The inverse
  seam, `ItemDef::droppable`, is used by all 31 gear items and both are
  merged per kill (`Game::equipment_drops_for`). The item-side direction won;
  the species-side field is redundant, not neglected.
- **`work_resource` does not gate cronjobs.** `Game::assign_cronjob`
  (`game/building.rs:304`) checks ownership and `ResourceNode`, never species.
  The field feeds drop tables (`game/combat_rewards.rs:50`) and the inspection
  screen. Any program can work any node — 10 of 17 species have
  `work_resource: None` and are not thereby unemployable.
- **`DataVoid` and `BlackIce` having no species is correct.** Both are
  unwalkable barrier terrain (`world.rs:96`), and `assets/species/README.md`
  says so explicitly.

## Well covered, for contrast

Ability content is not a gap. Across 31 files all five `AbilityTarget` shapes
are used, both `BuffKind`s and both `StatusKind`s appear, `wild_weight` is set
on 20, and cooldowns spread 0–5. Six of seven `AbilityEffect` variants are in
play. Adding abilities is easy but fills no hole.
