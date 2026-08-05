# Zone-gated upgrade tiers, and renaming raids to GC Entropy Sweeps

Date: 2026-08-05

Two independent changes, shipped together because both are small and both
touch the base-side vocabulary the player reads.

## 1. A structure's upgrade tier is capped by the zone you have breached

### The rule

A structure's reachable tier is `min(def.upgrade.max_tier, zone_level)`.

- Zone 1 → Mk1 only. Nothing can be upgraded before the first breach.
- Zone 3 → Mk3.
- Zone 6 and beyond → Mk5, because every shipped upgradeable structure
  declares `max_tier: 5`.

Placement is unchanged: `Game::build_structure` already inserts
`StructureTier(1)` for any def that declares an upgrade path, so a structure
deployed in any zone starts at Mk1 and climbs from there.

This follows an existing rule rather than inventing one. Gear level is
already capped the same way — reaching zone *N* is what unlocks level *N*
gear (`tuning::GEAR_LEVEL_GROWTH`'s doc comment, enforced in `Game::equip`).
Structure tiers now read the same way, and the two ladders line up 1:1: the
zone stat curve is pinned over `1..=5` and so is `max_tier`.

### Where the rule lives

One function, `Game::upgrade_ceiling(&StructureDef) -> Option<u32>`,
returning `min(upgrade.max_tier, zone_level)` — or `None` when the def
declares no upgrade path at all.

Two callers:

- `Game::upgrade_structure` checks the current tier against it instead of
  against `upgrade.max_tier` directly.
- `Game::view_entities` fills a new `EntityView::ceiling: Option<u32>`
  from it, so the renderer can label a row without recomputing the formula.

That second caller is the point of extracting the function rather than
inlining a `min`. A doc comment claiming the menu label "matches" the
refusal rule would be a copy, and the copy that drifts is the one nobody
runs — see the code principle in `CLAUDE.md`.

### Capped structures stay listed

`App::upgradeable_structures` keeps filtering on `tier.is_some()` only. A
structure sitting at its zone ceiling is still offered, and picking it is
still refused.

That is deliberate, and matches what already happens at `max_tier`: a Mk5
node is listed today and then refused with "already fully upgraded". The
alternative — filtering the ceiling out — makes the whole **Upgrade** row
vanish from the base menu in zone 1, because `crates/app-core/src/app/group_menu.rs`
hides a group-menu row whose screen would have no rows. A player who has
never breached would never see that upgrading exists.

So the row carries its own explanation:

```
[a] Mining Node at (3, 4) [Mk1 — zone 2 unlocks Mk2]
[b] Log Scraper at (5, 2) [Mk2]
```

The suffix appears only when the structure is at its zone ceiling *and*
that ceiling is below the def's `max_tier`. At the def's own ceiling the
row reads as it does today.

Refusal message:

> Mining Node can't go past Mk1 until you breach to zone 2.

At the def's `max_tier` the existing "already fully upgraded" message is
unchanged — the two ceilings give different messages because they mean
different things, one temporary and one permanent.

### Save format

Untouched. `StructureTier` is already saved and the ceiling gates only
*further* upgrades, never a downgrade. A pre-existing save holding a Mk4
node in zone 1 keeps it and simply cannot climb higher. No
`SAVE_FORMAT_VERSION` bump.

### Balance

This removes the Core Fragment upgrade sink from zone 1 entirely.
Fragments there go only into building and recipes, and breaching becomes
the thing that opens the sink.

That is the intent of the change, but it is a real economy shift that
nothing gates: `balance_sim` models one run's battle curve and does not see
build economy at all. It lands unmeasured, like the other bounded-income
knobs.

### Tests

- Upgrading at the zone ceiling is refused, and the message names the zone
  that would unlock the next tier.
- The ceiling rises across a breach: refused in zone 1, the same structure
  upgrades to Mk2 in zone 2, and Mk3 is then refused.
- The def's own `max_tier` still wins in a deep zone — at zone 9 a
  `max_tier: 5` structure stops at Mk5 with the "fully upgraded" message.
- app-core: a structure at its zone ceiling is still returned by
  `upgradeable_structures`. This pins the discoverability decision above,
  which would otherwise erode silently into a filter.

## 2. "raid" becomes "GC Entropy Sweep"

Player-facing text only. Rust identifiers (`MessageKind::Raid`,
`RAID_DAMAGE`, `Game::raid_check`), the `.ron` schema fields
`raid_defense` and `raidable`, and the saved `MessageKind` enum all keep
their names. No save bump, no mod breakage, no schema churn.

### What changes

- Four log lines in `crates/engine/src/game/upkeep.rs`.
- Three structure descriptions: `home.ron`, `shield.ron`, `patch_node.ron`.
- One test assertion on real player text, `crates/engine/src/tests/raids.rs`.
  Every other "raid" in the test suite is either internal vocabulary or a
  test-authored string that never reaches a player.
- Docs: `README.md`, `assets/structures/README.md`,
  `assets/items/README.md`, `docs/content-gaps.md`, and a `CHANGELOG.md`
  entry.

`docs/manual.md` is deliberately left stale, per the standing instruction
that it is carved out of the documentation obligation.

### One wording decision

"takes 4 raid damage" does not survive the substitution — "takes 4 GC
Entropy Sweep damage" forces a noun phrase into an adjective slot. That
line becomes:

> {label} loses 4 Durability to a GC Entropy Sweep!

Internal prose (tuning constant docs, invariant comments, `CLAUDE.md`)
keeps saying "raid", because it is describing `raid_check` and the
identifiers did not move.
