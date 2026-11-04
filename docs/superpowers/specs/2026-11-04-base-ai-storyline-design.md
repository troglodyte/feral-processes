# The base AI: a storyline at the base

**Status:** approved 2026-11-04. Architecture: [ADR 0042](../../adr/0042-storylines-are-plain-rust-arcs-owned-by-game.md) (Agreed).

## Intent

The game's long thread today is the zone grind. This adds a second one that lives at the base: one of
the player's own staff programs goes strange while researching, fuses with the Research Station and
becomes **the base AI**. From then on research belongs to it. At first it is a setback: research runs four
times slower. Training it brings research back and then takes it further, and unlocks research the base
could not otherwise reach.

This is the first storyline. More beats will follow around the same AI: it will want more programs, ask
for supporting machines to be built, and have demands and moods. Build this one so that each later beat is
an addition, never a rewrite.

"Feeding" the AI is called **training** it in everything the player reads.

## Goals

- The awakening and the fusion happen once per run, at a point that varies between runs, and are told
  through notifications labelled **Storyline**.
- After the fusion, research is harder at first. Training the AI is the way back, and it's worth doing.
- The AI's growth can be seen: its Analysis on the manifest is its real figure (120 when other programs
  top out around 20), and it gains levels.
- Ignoring the AI is a real choice ("somewhere in the middle"): the open research is still reachable, but
  slowly, and the research gated by the AI never opens.
- The structure takes the next beats (demands, moods, machines, hunger) without being reworked.

## Non-goals (this build)

- Demands, moods, supporting machines, or the AI taking programs on its own. The design leaves room for
  them and builds none of them.
- Any effect of the AI outside research. Pinning programs for study works exactly as it does today.
- Moddability. Story beats and their text are Rust.

## Anti-goals (a failure even if it technically works)

- A second Research Station, or any other trick, that gets round the slowdown.
- A dev-only path or save edit being the only way to see the beats. They must happen in normal play.
- A storyline that has no visible effect on research. The 4x slowdown is the point.
- An AI that can be lost by accident: removed, released, fused, sent on a sortie, or sold.

## The beats

### 1. Dormant

From the start of a run, the storyline holds a **trigger count** between 5 and 10, the same for the
whole run. *My call:* derive it from the run's seed so it costs no draw from the game RNG (see
Constraints). Once the number of completed research projects (`Research.0.len()`) reaches it, the
awakening begins. This happens whether or not anything is being researched at that moment. The count
is derived from the saved world seed, which every save carries, so old saves get one too.

### 2. The claim

- **Who:** the base staff program with the highest Analysis is chosen. It must be at the base and free
  to act: not in the party, not downed, not under study, not jailed. *My call:* a tie goes to the lower
  `ProgramId`. If no program qualifies, the awakening waits until one does.
- **Where:** it walks to the **nearest** Research Station and claims it. It drops whatever job it had.
- **If it stops being free on the way** (downed, jailed, taken into the party), *my call* is that the
  claim is abandoned and a new program is picked on the next check. The first alert doesn't fire a
  second time.
- **The player character is never a candidate.** Only tamed base staff are.
- **Alert:** the first Storyline notification fires when the claim begins and says that something is going
  wrong with that program.
- **If there's no station:** the awakening can't fire before the first research is done, so a station has
  existed. If the player has since removed every station, *my call* is that the awakening waits until one
  stands again.

### 3. The fugue

- **When it starts:** on arrival, the program enters a fugue at the station.
- **How long:** about a minute of game time, roughly 120 ticks at two a second, set by a `tuning/`
  constant.
- **Effect on research:** while the program walks there and while the fugue lasts, **no research happens
  anywhere**. No payout lands, and no study attempt banks.
- **Pinned subject:** a subject already pinned at that station stays pinned.
- **Saving:** the fugue counts world ticks, so it pauses whenever the world pauses. It is saved as ticks
  remaining, so a save made mid-fugue resumes it.

### 4. The fusion

- **Alert:** the second Storyline notification says the program seems to have fused with the station.
- **The program is locked to the station for good.** It is never scheduled anywhere else. It can't be
  fielded, released, fused, sold, or posted, and every refusal says why in storyline terms.
- **On the manifest:** it stays in the base manifest with its full stat sheet. *My call:* it is marked as
  the base AI there.
- **Only the AI researches, and only at its station.** No other program can work it.
- **Other stations stop.** Every other Research Station produces nothing from now on, and placing a new
  one is refused with a storyline line. *My call:* existing extra stations stay standing as dead machines
  and can be deconstructed for their usual refund.
- **The AI's station can't be deconstructed.** The refusal carries flavour text.
- **Slowdown:** at the moment of fusion, research runs at **25% of the pace that same program reached
  on the ordinary formula**, which makes it four times slower. *My call:* the slowdown applies to
  everything the station produces, both project progress and banked study attempts. Pinning a subject
  to study works as before, but at the AI's pace.

### 5. Growth (ongoing)

- **Analysis grows from time spent researching.** This uses the same shape as staff work skills
  (`skills/`, ADR 0038): ticks spent researching earn practice, and each point of Analysis costs more
  practice than the one before it. There is **no cap**.
- **Analysis is the program's real attribute.** It is the `analysis` entry in its `Attributes`, and the
  arc raises it, so the manifest, the dossier and every other reader see the true figure without an
  overlay. Read `a-baked-stat-needs-a-receipt` in memory before writing it. The thresholds are values of
  this attribute.
- **Speed follows Analysis through an AI-only term.** This is a verified fact, not a guess:
  - `node_payout(tier, zone)` doesn't read Analysis at all.
  - Analysis enters only through `mining_success_chance`, which is clamped to 1, so it saturates almost
    at once and the existing formula can't carry the AI's growth.
  - *My call:*
    - The ordinary research formula is left untouched, so the pace before the fusion doesn't change.
    - The AI's station output is the pace frozen at fusion × 0.25 × a growth factor.
    - The growth factor is 1 at the fusion Analysis and keeps rising with Analysis, with no ceiling.
    - Its shape and rate are set in the plan-time table.
  - The AI climbs back past its old pace once it has been trained, and keeps going.
- **Levels:**
  - The AI starts at level 0 at the fusion.
  - Each next level has an Analysis threshold, set in a table in `tuning/`.
  - When the AI's Analysis reaches the next threshold, it **stalls**: it earns no more practice until it is
    trained ("feed me!").
  - *My call:* while it is stalled, the alert board shows that the AI is waiting to be trained.
- **What the levels gate:** they unlock the AI-gated research (see Research below) and are where later
  beats will attach.

### 6. Training: "Train your AI"

- **When it appears:** once the AI is stalled at a threshold *and* the level's other criteria are met, a
  research project called **Train your AI** becomes selectable. In this build, the only other criterion
  is the pinned program the project itself requires. Later beats add criteria, such as a machine
  built or a demand met.
- **One entry:** it is a single repeating entry on the research screen, not a new node for each level.
- **What it costs:**
  - A **pinned subject**, which is consumed when the project completes. This works like a
    `requires_subject` project today (`spend_subject`).
  - *My call:* a research-point cost that grows with the level, so training takes time at the AI's own
    speed. The figures live in `tuning/`.
- **Completing it:**
  - The pinned program is consumed. The text says it was used to train the AI.
  - The AI gains a level and practice can accrue again.
  - A Storyline notification marks the level.
  - Any research gated on that level opens.
- **Normal research continues.** While Train your AI is not selected, the AI works ordinary projects at
  its current speed, and its Analysis stays held at the threshold.
- **Rules (my calls):**
  - **No overshoot:** Analysis is clamped exactly at the threshold, and practice past it is dropped,
    not banked.
  - **Switching is free:** the player can leave Train your AI and come back. Its points are kept in the
    same way as `ActiveResearch.progress`.
  - **The subject is spent only on completion.** Unpinning it before then costs nothing.
  - **A stall never interrupts the running project.** The AI keeps working it, and only its Analysis
    stops rising.

## Research

The user's direction: research done after the awakening should be costly enough that training the AI is
worth doing, and some research is locked behind the AI's progress.

- **AI-gated projects.** Research gains an optional minimum AI level. A gated project the AI hasn't
  reached stays visible on the research screen with the line "seems feasible, but we don't know where
  to start", and it can't be selected. *My call:* the line shows whenever the AI's level is too low,
  including before the awakening. This is a new field on `assets/research/*.ron`, so it is in the ADR
  and gets `#[serde(default)]` and a README update.
- **Costlier late research.** The cost of projects a player normally reaches after the awakening rises.
  The cheap first tier, roughly the projects costing 8 to 30 points that players finish before the
  awakening, is left as it is.
- **The table is decided at plan time, with the user.** The plan's first deliverable is this table, with
  measurements behind it. Work stops there for the user's approval before any tuning is applied.
  Building the systems doesn't wait on it: they are built against placeholder figures. Which projects are gated at which level, the
  new costs, the number of levels in this build, the thresholds and the training costs all go in one
  table that the plan presents and the user approves before it is applied. The table is checked with
  the existing instruments: `docs/measurements/` first, then `balance_sim` and whatever research-pace
  measurement fits. It is not guessed. The bars it has to clear:
  - An untrained AI can still finish every ungated project, but slowly.
  - A player who trains at each threshold ends up researching faster than before the fusion.
  - Every AI level up to the last one in this build unlocks at least one project.

## Constraints

- **Architecture: [ADR 0042](../../adr/0042-storylines-are-plain-rust-arcs-owned-by-game.md).**
  - Storylines live in `crates/engine/src/storyline/`.
  - They are plain Rust state owned by `Game` (ADR 0021), not ECS components, except the fused
    program's marker component that keeps the scheduler away from it, in the same way as
    `Jailed`/`UnderStudy`.
  - Each beat is gather → decide → apply (ADR 0020) and has unit tests on the decide step.
- **Determinism.** Choosing the program and the station sorts by stable id (ADR 0022). The trigger count
  must not shift the game RNG stream. Check `balance_sim` after the change.
- **Notifications.**
  - The new kinds go through `.claude/rules/seams-notifications.md`: an exhaustive `def` and fired-by
    census coverage.
  - The "Storyline" label is new. Notifications have no category today, so the ADR adds one. Drawing it
    is new work in `crates/gui/src/render/notify.rs`.
  - This change spans the engine, app-core (research-screen selection of Train your AI) and gui, so it
    takes the full spec-and-plan pipeline.
  - The text is plain language: no tick vocabulary and no occult naming. Its programs are "programs".
- **Saves.**
  - The storyline state is a new `#[serde(default)]` field, which defaults to dormant, so old saves
    load.
  - A save→load test covers every stage, mid-fugue included.
  - **The user's decision:** a save already past its trigger count, with more than 10 projects done,
    awakens immediately after load. This falls out of the rule and needs no special case. The user's own
    save, `fred`, is the one this applies to, so check against it.
- **Seams.** Read the `seams-base`, `seams-research`, `seams-notifications`, `seams-hud` and `seams-saves`
  rules before touching those subsystems. The base-labour scheduler (`game/base/assignment.rs`) must
  never pick the fused program or post anyone else at its station.
- **Dev saves.** Capture a template just before the awakening and one with the AI fused and stalled at
  level 0, and list both in the README.
- **Docs.** Add a CHANGELOG entry, the research README update, and *my call:* a short in-game help page
  about the base AI. Don't change the manual or the README.

## Left to the builder

- The exact names of types, variants and modules within the ADR's shape.
- The wording of every Storyline notification, refusal and help line, in plain language and in the
  game's voice. The user reviews it at the end.
- How the claim walk is scheduled: a Task or a direct post. The constraints are that the program walks
  there visibly, and that nothing else can claim it while it does.
- How the research screen shows Train your AI and the gated line, following the existing research
  screen and the rules that lists are highlight + Enter and that GUI text never clips.
- The practice-per-tick and cost-curve figures, until they're tuned against the plan-time table.
