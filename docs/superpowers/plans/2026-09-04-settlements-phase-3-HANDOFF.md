# Settlements Phase 3 — handoff

Branch `feat/settlements` in `/home/trog/code/wt-persistent-world`.
Read this, then `docs/superpowers/plans/2026-09-04-settlements-phase-3-market.md`
(the plan), then `CLAUDE.md`. Invoke the `seams` skill for **items** and
**hud** before touching the screen.

## Landed (tree clean, all committed)

| commit | what |
|---|---|
| `1af32aa4` | the Phase 3 plan |
| `d6cd9d51` | Task 1 — `commerce::settle_basket`, the shared commit core |
| `b08e39b1` | shelf draw + offer wording shared with a settlement |
| `9b3497b6` | Tasks 2–3 — the shelf, Temperament prices, buyback |
| `0fea7126` | Task 5 — the three seam writes |
| `676f1d58` | **Task 4 WIP** — the market screen, compiles, tests NOT written |

Gates as last run by hand: `cargo test --workspace` green at **4,405**,
`balance_sim` green, `cargo fmt --check` clean.

## What is left, in order

1. **Finish Task 4.** `676f1d58` compiles but has no tests. Needed:
   app-core tests (bump/`x` open the mode, Esc clears, basket commits
   through the shared core, a short basket spends nothing, Shift/Ctrl
   still modify) and gui tests (a row names the town's offer; a fit
   census over the **real** catalogue, both dimensions).
   **Verify `ALL_MODES` in `crates/gui/src/render/mod.rs` is `[Mode; 94]`**
   and that `Mode::SettlementMarket` is named in the modifier fold at
   `crates/app-core/src/app/input.rs:184` — omitted, Shift and Ctrl fold
   to bare arrows and silently become plain steps.
2. **Fix a stale doc comment.** `crates/engine/src/settlements/mod.rs`,
   `Temperament`, still says *"read by nothing yet: the hooks are prices
   (Phase 3)"*. Phase 3 shipped — `buy_mult()`/`sell_mult()` are called
   from three sites in `game/settlement_market.rs`. The sentence is false.
3. **One unproven claim.** `a_settlement_baskets_quoted_cost_is_exactly_what_gets_charged`
   (`crates/engine/src/tests/settlement_market.rs`) guards the drift
   surface where a vendor passes a `cost` its own closure does not charge.
   It looks well built — its doc comment shows it understands only a
   `Material` row with `qty > 1` can catch the bug — but **nobody has
   mutation-tested it**. Break the `* qty` in the charge, watch it go red,
   restore with `git checkout --` (the file is committed).
4. **Whole-branch review** (opus). Non-optional per this repo's process.
5. **`CLAUDE.md` / `AGENTS.md` are gitignored** and cannot ride the branch.
   **Two phases** of seam rules now sit only in this worktree and must be
   copied into the primary checkout by hand. Ask first — other sessions
   are live there.
6. **Rebase.** Branch is ~10 commits behind `main`, then land.

## Traps that have already fired here today

- **Never `git stash`.** A plain stash does not capture untracked files;
  one nearly lost this phase's work. Commit first and compare against the
  commit instead.
- **`stash@{0}` is foreign** — it belongs to `worktree-nests-feature`.
  Never pop, apply, or drop it. Stashes are per-repo, not per-worktree.
- **Never pipe `cargo test` through `grep`/`tail`** — the pipeline's exit
  code masks a failure. Redirect to a file.
- **Two known intermittents**: `a_posted_worker_levels_up_in_the_base_log_beside_its_machine`
  and `a_leech_fills_a_node_buffer_faster_than_a_striker`. One red that
  passes on a single re-run is a flake; do not chase it.
- **Do not poll for subagent completion.** Completions arrive as
  notifications. Polling `git log` in a loop was the single largest
  waste of this session.

## Out of scope, by decision

Standing and refuse-service are **Phase 4**. Do not add a standing gate to
the market now — it would be a second price formula for Phase 4 to unpick.
