# Phase 3 — Research, sources, combat (engine, additive)

Read first: plan index; spec "Where weapon parts come from", "Research",
"Synergy, concretely", combat paragraph of "How parts become numbers";
`.claude/rules/seams-research.md`, `seams-combat.md`; memory
`stun-costs-no-turn-on-a-battle-map`; `routine_tree.rs`, `affix_tree.rs`.

## Files (expected; grep before assuming)

- `research.rs`: `ResearchTree::Weapons`. New `weapon_tree.rs` synthesising
  nodes from `WeaponPart::all()` (frames v1→v3 `min_zone = version`, Exploit
  requires matching Rider node, `weapon_capacity/<n>`), all `discoverable`.
  `tuning::weapon_research_cost` beside `routine_research_cost`.
- `game/unlocks.rs`: tree open when `weapon_bench` researched (Rust, no
  `opens_*` field); `listed_research` arm; `eligible_discoveries` gains an
  explicit Weapons-when-open branch beside Affixes.
- `game/gear_craft.rs`: `salvage_weapon(copy)` — consumes, hands out
  `decompose` parts, discovers their nodes, records carried affixes in
  `KnownAffixes`. Design affix picker reads researched `affix:<id>` ∪ known.
- Stack market + caravan shelves: frames and modifiers via `shelf()`; no
  Exploit, no Proc.
- `game/combat_damage.rs`: rider arms its status in
  `resolve_and_apply_attack` beside `type_side_effect`, through existing
  status-arming code, seeded draw. Exploit +% in the swing's damage calc
  before `apply_damage` / `apply_damage_typed` (those stay untouched).

## Tasks

- [ ] 1. Tree + nodes + cost; closed before `weapon_bench`, open after;
  capacity nodes raise `gear_part_cap`.
- [ ] 2. Discovery branch: Study can discover weapon nodes only when open.
- [ ] 3. Salvage: parts out, nodes discovered, affixes known, Proc only from
  salvage.
- [ ] 4. Shelves carry frames/modifiers, never Exploit/Proc.
- [ ] 5. Rider arms on a landed hit (seeded), never on a miss. Exploit raises
  damage only when the status is present. A routine-applied Exposed +
  Exploit(Exposed) battle test.
- [ ] 6. `balance_sim` unmoved.

## Gate

Engine tests, `balance_sim`, `cargo test --workspace`. Handback: where the
rider draw sits in the RNG order, and any authored species whose fights
changed numbers.
