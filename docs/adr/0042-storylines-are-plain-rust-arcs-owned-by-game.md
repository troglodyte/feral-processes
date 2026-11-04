# 0042. Storylines are plain Rust arcs owned by Game

- Status: Proposed
- Decided by: Claude — proposed, awaiting the user
- Date: 2026-11-04
- Pattern: Plain structs owned in one place; Functional core, imperative shell
- Depends on: [0019](0019-content-defaults-to-rust.md), [0020](0020-functional-core-imperative-shell.md), [0021](0021-plain-rust-over-ecs-for-discrete-logic.md), [0022](0022-rng-never-depends-on-query-order.md), [0023](0023-saves-are-field-named-ron.md)

## Context

The game has no storyline system. `story.rs` loads only the ending screens. The base AI
([spec](../superpowers/specs/2026-11-04-base-ai-storyline-design.md)) is the first storyline, and the user
has said more beats will follow around it: demands, moods, supporting machines, growing hunger. Other
storylines may follow too.

The base AI has to do three things the current code doesn't support:

1. Hold a run-long, staged state that is saved.
2. Label notifications **Storyline**. `NotificationKind` has no category today.
3. Gate research on the AI's level, which needs a new field on `assets/research/*.ron`.

## Decision

1. **A `storyline/` engine module.** `crates/engine/src/storyline/` holds a plain `Storyline` struct
   that `Game` owns. It has one field per arc, starting with `base_ai: BaseAi`. Each arc is a stage enum
   that carries its data: dormant (trigger count) → claiming (program, station) → fugue (program,
   station, end tick) → fused (program, station, level, practice). The arc is not ECS. The only ECS
   piece is a marker component on the fused program, in the same way as `Jailed` and `UnderStudy`, so
   the base-labour scheduler and every "who is free" query exclude it through the path they already
   use.
2. **Beats are gather → decide → apply.** Each tick, `Game` gathers a plain input struct: completed
   research count, eligible staff sorted by `ProgramId` with their Analysis, stations sorted by id, and
   the current tick. It passes this to a pure `base_ai::step(&BaseAi, &Input) -> Outcome` and applies
   the result. `Outcome` is concrete to this arc. A later beat adds a stage or a field and a branch in
   `step`, and the shell stays the same.
3. **Story text is Rust.** This follows ADR 0019: beats and their wording are code, not an
   `assets/story/` kind.
4. **Notifications gain a category.** `NotificationDef` gets an optional category label, and the panel
   shows it above the title. Storyline is the first category, and every existing kind has none. The
   census and the exhaustive `def` match carry it as they carry title and body.
5. **Research gains `requires_ai_level: Option<u32>`.** It is `#[serde(default)]` and documented in
   `assets/research/README.md`. A node whose level isn't met shows as known but not selectable, with
   its own line.
6. **"Train your AI" is not a `ResearchId`.** `Research` is a set of one-shot ids, so a repeating project
   doesn't fit it. Train your AI is a second kind of research target, chosen when the active research
   is selected. Its cost, its subject and its completion come from the `base_ai` arc and `tuning/`, and
   it reuses the existing payout and subject-spending code.
7. **Saves.** `Storyline` serializes into `SaveData` as one field-named, `#[serde(default)]` field
   (ADR 0023). A save without it loads dormant.
8. **Determinism.** The trigger count is derived from the run's seed, not drawn from `GameRng`, so adding
   the arc doesn't shift any existing stream (ADR 0022).

## Alternatives considered

- **Story beats as data** (`assets/story/*.ron` with triggers and effects). Rejected for now, per ADR
  0019. There is one arc with a handful of beats, and each beat's effect is new code anyway. A data
  format would need a trigger language before there is a second storyline to show what it needs.
- **A generic event/effect engine** (a list of `StoryEvent { trigger, effect }`). Rejected. CLAUDE.md
  keeps `Outcome` concrete for each feature and never one generic `Effect` enum. A second storyline
  will show what the arcs really share, and that's when to extract it.
- **The AI's state on ECS components** (level and practice on the fused program's entity). Rejected per
  ADR 0021. This is discrete, turn-based logic that one place owns, and only the scheduler-exclusion
  marker needs to be on the entity.
- **Train your AI as a research asset reset on each level.** Rejected. It would break the rule that
  "researched" means `Research` contains the id (`seams-research`).
- **A "Storyline" prefix on titles instead of a category.** Rejected. It mixes a label into authored text
  and can't be styled separately.

## Consequences

- **Unlocks:**
  - A home for every later base-AI beat, and for other storylines.
  - A notification category that other features can use.
  - Research gated on story progress.
- **Breaks or costs:**
  - A new save field.
  - A new field on research `.ron`.
  - A change to the notification def table, so every kind's `def` arm is touched.
  - A new research-target kind that the research screen and `settle_research` must handle.
- **Revisit when** a second storyline arrives. Extract only what the two arcs actually share.

## Evidence

- `story::` and `assets/story/ending.ron`: the only existing story code.
- `notifications::NotificationKind`, `game::notify`, `.claude/rules/seams-notifications.md`
- `resources::Research`, `resources::ActiveResearch`, `game::unlocks::settle_research`, `spend_subject`
- `components::staff::Jailed`, `UnderStudy`, `game::base::assignment::assign_by_priority`
- `skills::` (ADR 0038) for the practice-and-cost shape the AI's Analysis growth copies
- `systems::node_payout`, `analysis_over_base`
