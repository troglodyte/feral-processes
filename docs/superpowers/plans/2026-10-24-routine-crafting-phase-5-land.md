# Phase 5 — Review and land (controller, not a subagent)

- [ ] 1. `git fetch`; merge `origin/main`; full `cargo test --workspace`,
  `balance_sim`, clippy.
- [ ] 2. Whole-branch review, **opus**, diff given as a file. Brief it with the
  index's Review focus and the ADR, not this plan's conclusions. Fixes from it
  get their own short review.
- [ ] 3. Mutation-check the three load-bearing tests: compile census,
  save→load round trip, "no authored grant" census.
- [ ] 4. Mark ADR 0029 and the spec shipped; `docs/superpowers/INDEX.md` row.
- [ ] 5. CHANGELOG `## X.Y.Z`: **breaking** (save 38→39 refused).
- [ ] 6. `fred` save: ask the user first, then per spec "Save" (savetool dump →
  edit → pack; back up the original).
- [ ] 7. Land via the `deploy` skill (merge → tag → push → worktree cleanup),
  only on the user's word.
