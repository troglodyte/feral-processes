# Affix modding and the Affixes research tree

**Status:** design approved in brainstorming 2026-09-30; spec awaiting review.

## Goal

Let the player add and remove affixes on weapons and armour they own, and
give them affixes worth adding: a small set of research-only affixes found
in a third research tree. Two trial affixes lead it — a DECOMP affix for
weapons, and a flat per-hit damage reduction (Deflection) for armour.

Out of scope: Module affixes (the design must not prevent them), a
regenerating shield pool (a later effort; "shield" is reserved for it),
extracting an affix into a transferable item.

## Decisions (from brainstorming)

| Question | Decision |
|---|---|
| What a removal does | Destroys the affix. Applying is a recipe paid in materials. |
| Which affixes can be applied | Researched ones only. Found affixes can be stripped, never moved. |
| Capacity | `1 + fusion tier` affix slots per copy (max 4 at `MAX_FUSIONS` 3). |
| Decompile affix | Plain `decompiler` on a Weapon — inert on a companion, and the mod screen says so. |
| Damage reduction | New flat stat **Deflection** (DEFL), not scaled, hard-capped. |
| Tree sourcing | Nodes synthesised from affix files, like the Routines tree. |
| "Discoverable" | Every affix node is `discoverable`: hidden until a study attempt finds it. |
| Drop pool | An affix with a research block is research-only; it never drops. |

## 1. Data: `AffixDef::research`

A new optional field on `AffixDef` (`#[serde(default)]`):

```ron
(
    id: "deflecting",
    prefix: Some("Deflecting"),
    stats: (deflection: 2),
    slots: Some([Armor]),
    research: Some((
        cost: 20,                           // Research Data
        materials: [("logic_wafer", 4)],    // optional, default none
        min_zone: 2,                        // optional, default 0
        requires: [],                       // affix ids, optional
        apply_cost: [("core_fragment", 3)], // paid on every apply; required, non-empty
    )),
)
```

- **Research-only.** `Game::roll_affix` (`game/combat_rewards.rs`) filters
  out every affix with `research: Some(..)`. The shipped drop pool is
  unchanged, so no seeded drop roll moves.
- `weight` on a research-only affix is meaningless; setting it logs a load
  warning, not a refusal.
- **Load validation** (skip with a warning, `load_dir` pattern): an empty
  `apply_cost`; an unknown item id in `apply_cost`/`materials`; a
  `requires` naming an unknown or non-research affix. A dropped
  prerequisite cascades, as `ResearchDb::load_dir` does.
- Deleting or renaming a research-only affix is as safe as any other:
  copies carrying it read as one affix lighter (`Game::affixes_of`).

## 2. Deflection stat

- `EquipmentStats::deflection: i32`, `#[serde(default)]`. Flat damage
  removed from each incoming hit.
- **Read live** off `Game::gear_bonus`, like `accuracy`/`evasion` — not
  baked into `Stats`, and `apply_equipment_delta` must not invent a field.
- **Not scaled.** `fused_for_tier`, `for_rarity` and the gear-level step
  (`Game::copy_bonus`) leave `deflection` at its authored sum. Reason: an
  affix is added before scaling, so a scaled +2 on a level-5 copy blocks
  ~12 per hit, which with the damage floor of 1 makes armour near-immune to
  small hits.
- `tuning::DEFLECTION_MAX` (6) caps a wearer's total.
- `Game::mitigate_incoming_damage` (`game/combat_damage.rs`): percentage
  first, then subtract deflection, then the existing floor of 1. One
  function, so every damage source that already mitigates also deflects.
- Items may author it too (it is an `EquipmentStats` field); none ship with it.
- Shown in stat readouts beside MIT as `DEFL`.
- Affix calibration: new axis ceiling DEFL +4 in
  `every_shipped_affix_pays_and_none_pays_past_the_calibration`.

## 3. The Affixes research tree

- `ResearchTree::Affixes`, a third variant.
- `affix_tree::synthesise_nodes(&AffixDb) -> Vec<ResearchDef>`, merged into
  `ResearchDb` where `routine_tree::synthesise_nodes` is. One node per
  research-only affix: id `affix:<affix id>`, name/description derived from
  the affix (name, its stats, its slots), `cost`/`materials`/`min_zone` from
  the block, `requires` mapped to node ids, `tree: Affixes`,
  `discoverable: true`, `requires_subject: false`.
- Researched state is the existing `Research` id set (no `teaches`), so
  `node_researched` needs no branch and **no new save field**.
  `Game::affix_researched(&AffixId)` is the one query apply goes through.
- **Gate:** a base-tree node `assets/research/mod_bench.ron`
  ("Mod Bench"), new flag `opens_affix_tree: true`, requires
  `weapon_bench` and `armor_bench`, `requires_subject: true` (the zone-1
  bench rule), `unlocks_structures: ["mod_bench"]`.
  `Game::affix_tree_open()` uses `capability_unlocked`, keeping the lenient
  rule: no loaded node carries the flag → the tree is open from the start.
- **Discovery:** `eligible_discoveries` (`game/unlocks.rs`) admits
  `Affixes`-tree nodes as well as `Base`, but only while
  `affix_tree_open()`. Before the gate is researched the pool is exactly
  today's, so no seeded study roll moves.
- UI: a third research view, opened from the group menu beside Routines;
  lists nothing until the tree is open, and only discovered nodes after.

## 4. Mod Bench structure

`assets/structures/mod_bench.ron`: a plain bench (no worker, no power
draw beyond what a bench normally has — follow `fabricator.ron`/
`armory.ron`). Apply and remove require one **deployed** Mod Bench in the
base, the `requires_structure` rule: it must exist, the player need not
stand at it.

## 5. Apply and remove

```rust
pub fn apply_affix(&mut self, copy: &GearCopy, affix: &AffixId) -> Result<String, String>;
pub fn remove_affix(&mut self, copy: &GearCopy, affix: &AffixId) -> Result<String, String>;
pub fn affix_slots(&self, copy: &GearCopy) -> u32;          // 1 + copy.tier
pub fn appliable_affixes(&self, copy: &GearCopy) -> Vec<AffixId>; // researched, slot-eligible
```

Same shape and copy selection as `Game::fuse_item` (`game/crafting.rs`):
the copy is taken out of the ledger and re-keyed through
`GearCopy::with_affixes`, keeping the sorted-list invariant.

**Apply refuses**, each with a player-facing sentence, when:
no Mod Bench deployed; the affix is not researched; the copy's slot is not
in the affix's `slots` (only Weapon and Armor copies for now); the copy has
no free slot (`affixes.len() >= affix_slots`); `apply_cost` can't be paid.
Payment is taken the way a compile recipe's cost is (the plan confirms the
exact source), and only once every check has passed.

**Remove** refuses only without a Mod Bench, or when the copy lacks the
affix. It is free, destroys the affix, and works on any affix — dropped,
fused-in or applied. Removing one of two duplicates removes one.

**Over-cap copies** (fusion can union up to 8 affixes onto a 4-slot copy)
keep everything; they cannot take a new affix until removals bring them
under the cap.

## 6. UI (app-core + gui)

- An UPPERCASE action on the inventory item menu beside fusion opens a mod
  screen for that copy; shown only when the copy is a Weapon or Armor.
- The mod screen lists the copy's slots: each filled row can be removed,
  each empty row opens a picker of `appliable_affixes` with their stats and
  `apply_cost`. Refusals are shown in the existing refusal line.
- A DECOMP affix on a weapon shows a note that it does nothing on a
  companion.
- Keyboard only; lowercase letters select rows, actions are uppercase.

## 7. Trial content

| Affix | Grants | Slot | Zone | Requires |
|---|---|---|---|---|
| `deflecting` ("Deflecting") | DEFL 2 | Armor | 2 | — |
| `of_deflection` ("of Deflection") | DEFL 4 | Armor | 4 | `deflecting` |
| `of_introspection` ("of Introspection") | DECOMP 2 | Weapon | 2 | — |

Costs are first guesses for the plan to set against neighbouring nodes'
costs; the balance is untuned until played.

## 8. Saves and versioning

No new save fields expected: affixes already persist as a list on every
copy, and researched/discovered state is already id sets. The plan must
prove it with a real save → load test of a modded copy and of a
researched-and-discovered affix node (not only a RON round trip). No
`SAVE_FORMAT_VERSION` bump expected → a minor release.

## 9. Testing

Engine (TDD, failing test first):
- research-only affixes never roll on drops; the shipped pool is unchanged;
- synthesised nodes: tree, id, discoverable, requires mapping; invalid
  blocks skipped with a warning;
- the gate opens the tree; lenient rule with the flag stripped;
- the discovery pool is unchanged before the gate and admits affix nodes after;
- each apply refusal; apply pays exactly `apply_cost`; slot cap;
  over-cap copy; remove of a duplicate; the ledger re-key keeps copies findable;
- deflection: percentage then flat then floor 1; not scaled by tier,
  rarity or level; `DEFLECTION_MAX`;
- calibration ceilings include DEFL;
- save → load of a modded copy and of research state.

App-core: key flow into the mod screen, apply, remove, refusal line.
Gates: `cargo test --workspace`, clippy `--all-targets`, `balance_sim`
(expected unmoved — nothing new drops).

## 10. Docs

`assets/affixes/README.md` (research block, research-only, DEFL),
`assets/research/README.md` (`opens_affix_tree`, the Affixes tree),
`assets/structures/README.md` if the bench needs a note, `CHANGELOG.md`.
Not the manual, not the top-level README.
