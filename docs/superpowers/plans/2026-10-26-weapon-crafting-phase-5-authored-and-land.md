# Phase 5 — Authored weapons, then land

Read first: plan index; spec "What changes for authored weapons";
`.claude/rules/content-schema.md`.

## Tasks

- [ ] 1. Confirm `shiv_routine` drops (sprite, drone) and `kinetic_edge`
  (sub_process, glitch) come from species met before `weapon_bench` is
  reachable. If not, **stop and ask**.
- [ ] 2. Remove `craftable` from the 8 authored weapons. Update
  `standard_and_premium_gear_is_made_from_intermediates` and its
  fragment-only sibling to drop weapons from their sets (don't delete them).
  Census: no authored weapon is craftable.
- [ ] 3. `assets/items/README.md` notes weapons are designed, not crafted.
- [ ] 4. Full gate: `cargo test --workspace`, `balance_sim`, clippy, fmt.
- [ ] 5. Opus whole-branch review against the index's Review focus; diff
  handed as a file. Fixes get their own review.
- [ ] 6. CHANGELOG entry (unversioned on the branch; release at landing).
  Mark ADR 0031 Accepted and add the INDEX.md row.

## Gate

Green full suite, review findings resolved or listed for the user. Then the
`deploy` skill on the user's word.
