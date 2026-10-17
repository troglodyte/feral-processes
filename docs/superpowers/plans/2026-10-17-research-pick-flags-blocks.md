# Plan: research pick flags a block instead of refusing

Spec: `docs/superpowers/specs/2026-10-17-research-pick-flags-blocks-design.md`.
Branch `research-pick-flags-blocks`, primary checkout. TDD per step, commit
per green step. Gate for every phase: `cargo fmt`, `cargo clippy --workspace
--all-targets`, the named tests. Final gate: `cargo test --workspace`.

## Phase 1: engine (`feral-processes-engine`)

Files: `crates/engine/src/game/unlocks.rs`, `game/base/work_orders.rs`,
`views.rs`, `tests/research.rs`, `tests/work_orders.rs`.

1. `queue_work_order`: a `for_research` order skips `order_refusal` except
   the `"No such item."` arm (split `find_break` so that arm is reachable
   alone, or check `ItemDb` first). Test (`tests/work_orders.rs`): a
   research order for an item with no deployed producer is filed and
   `work_order_report` reads it `OrderState::Stalled`; the same item as a
   player's order is still refused; deploying the producer makes it
   `Wants`.
2. `select_research`: delete the `research_block` rung and its comment.
   Flip the listed refusal tests to assert `Ok` plus `ActiveResearch.id`.
   `the_zone_gate_is_refused_before_the_machines` and
   `no_research_node_is_gated_below_its_own_prerequisite` keep their
   ordering intent against the remaining rungs.
3. `research_nodes`: `blocked_by` filled for `Active` too (`Unlocked` only
   stays `None`). Rewrite the `blocked_by` doc in `views.rs`. Test: the
   active project with no Research Node has `blocked_by == research_block`.
4. `ResearchReadout::Blocked { name, reason }` in `views.rs`;
   `research_readout` returns it ahead of `Stalled` when
   `research_block(def)` is `Some`. Tests: no node standing → `Blocked`
   with the row's sentence; subject-gated node, nobody pinned → `Blocked`;
   pin → `Earning`; full data + missing machine does not complete, the
   material arriving completes it.
5. Seam reference `.claude/skills/seams/references/base-needs.md`: six
   refusals → five; the block is the flag. Memory graph observation on
   `seam:research-is-one-project-and-select-research-is-the-one-door`.

## Phase 2: gui and app-core

Files: `crates/gui/src/render/progression.rs`, `research_graph.rs`,
`hud/stock_block.rs`, `crates/app-core/src/tests/research.rs`.

1. `progression.rs`: `research_menu_rows` label is
   `[k] ! Name - price tag` when `blocked_by.is_some()`. Extract
   `pub(super) fn flag(node) -> &'static str` (`"! "` or `""`) so the graph
   box uses the same call. Tests: label carries `!` for `Active` and
   `Available` with a sentence, not without; mutation check by hand.
2. `research_graph.rs`: `box_label` first line prefixed with `flag(node)`.
   Test on the box label.
3. `stock_block.rs`: `research_lines` arm for `Blocked`: header, name
   (`BODY`), `! {reason}` wrapped in `ATTENTION`. Test like the `Stalled`
   ones.
4. app-core: `a_refused_selection_lands_on_the_status_line` must still
   refuse; check which rung it hits and switch it to a prerequisite or
   zone refusal if it relied on the block.

## Phase 3: content and release notes

1. `assets/help/60-your-base.md`: rewrite the "refused outright" and
   "refuse selection until a program is pinned" sentences.
2. `CHANGELOG.md` section at landing (version bump is the landing step,
   not this branch).
3. `docs/superpowers/INDEX.md` entry for the spec.
