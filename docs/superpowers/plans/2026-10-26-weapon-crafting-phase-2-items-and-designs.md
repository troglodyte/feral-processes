# Phase 2 — Items, designs, save (engine, additive)

Read first: plan index; spec "Life of a design and a build", "Save";
`.claude/rules/seams-items.md`, `content-schema.md`; `AbilityDb::set_crafted`
(`abilities.rs`) and `game/routine_craft.rs` as the model.

## Files (expected; grep before assuming)

- `items_db.rs`: crafted tier + `ItemDb::set_crafted_gear(&[GearDesign],
  &AffixDb)` registering `crafted_gear/<n>`; `get` answers both tiers. Audit
  every iteration over items (authored-only vs both) and decide each. Part
  items synthesised beside `synthesise_parts` (one per `WeaponPart`, Frame
  per version), name prefix "Weapon part · ".
- `resources.rs` + `lib.rs`: player state — designs, `next_gear_design_id`
  (never reused), `KnownAffixes` (`BTreeSet<AffixId>`).
- New `game/gear_craft.rs`: `gear_part_cap`, `craft_weapon_part`,
  `create_design(name, parts)`, `build_design(id)` (pays parts +
  frame material bill via `pay_from_reach`, mints Ordinary tier-0 copy),
  `delete_design(id)` → typed refusal (ADR 0027) if a copy exists in
  inventory, worn, companions' gear, or `BuybackLedger`. Each mutator calls
  `set_crafted_gear` in apply.
- `save.rs`: `gear_designs`, `next_gear_design_id`, `known_affixes`, all
  `#[serde(default)]`; no version bump. `set_crafted_gear` before inventory /
  equipment / companions / shelves restore.
- New `views/gear_craft.rs`: design list rows, new-design category rows with
  fit/greyed reason, live preview (same numbers `gear_detail` shows), parts
  held/missing, part-craft rows (held / cost / locked).

## Tasks (a commit per green step)

- [ ] 1. Crafted tier + door; a crafted id resolves via `get`, equips, wears,
  fuses (two builds of one design), sells.
- [ ] 2. Part items; census: every catalogue part has an item.
- [ ] 3. Create / build / cap enforced; build consumes parts + bill.
- [ ] 4. Delete refused per holder site (one test each), allowed when none.
- [ ] 5. Save→load: design + worn built copy + fused copy + known affix →
  **load** → resolvable, rider/exploit present. Delete removes from save and
  tier.
- [ ] 6. Views, unit-tested.

## Gate

Engine tests, `balance_sim`, `cargo test --workspace`. Handback: the item
iteration audit list and the delete-refusal holder list.
