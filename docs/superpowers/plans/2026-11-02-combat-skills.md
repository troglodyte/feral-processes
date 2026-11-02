# Combat skills: plan

**Spec:** `docs/superpowers/specs/2026-11-02-combat-skills-design.md`. Read it
first; it decides the behaviour and lists Claude's calls. **ADR:** 0040 (and
0038/0039 it amends). This plan says where the work goes and how it is gated.

**Worktree:** `.claude/worktrees/work-skills`, branch `combat-skills`.

**Rules for every phase:**

- TDD: write the failing test first. Commit at each green step.
- Run `cargo fmt` and `cargo clippy --workspace --all-targets` before each
  commit.
- Never push. Never `git checkout`, `git stash` or `git add -A`. Stage
  explicit paths.
- Unset `FERAL_DEV_NO_SIEGES` before tests.
- Fetch `origin/main` at each phase boundary. Merge if a peer release landed.
- Read `.claude/rules/seams-combat.md` and `seams-base.md` before touching a
  door they name. Run `design-patterns` and `module-boundaries` before
  Phase 1.
- RNG draw order must not change. Skills take no draws.
- Iterate with `cargo test -p feral-processes-engine <name>`. Run
  `cargo test --workspace` at each phase end.

**Phase sizing:** each phase fits one subagent context. Phases 1-4 are engine
only; 5 is content; 6 is UI; 7 is docs and release prep. Phases run in order.
Phase 5 can start once 3 is green, if that saves wall-clock.

---

## Phase 1: skill core (engine, `skills/`, `game/base/skill_scale.rs`)

**Files:** `skills/mod.rs`, `skills/tests.rs`, `tuning/skills.rs`,
`game/base/skill_scale.rs`, `game/lifecycle/spawn_saved.rs`, `views/work.rs`,
and any reader of `Skill::ALL` (`rg -n "Skill::ALL"`).

**Interfaces:**

- `Skill` gains `Aim, Force, Defence, Routines`, appended. `name`, `label` and
  `role_noun` must be exhaustive. The nouns are placeholders until Phase 6.
- `Skill::WORK: [Skill; 6]` and `Skill::COMBAT: [Skill; 4]`. `ALL` is their
  union.
- `skills::scale(level: u32, spread: f32) -> f32 = 1 + spread*(level - NEUTRAL)/NEUTRAL`,
  with `tuning::SKILL_NEUTRAL_LEVEL = 10`.
- Work's scale in `skill_scale.rs` becomes `scale(level, WORK_SKILL_SPREAD = 0.5)`.
  Delete `SKILL_SCALE_FLOOR`/`PER_LEVEL`.
- `Skills::overlay(seed: Skills, rows) -> Skills`, or the equivalent: saved rows
  overlaid on a seed.

**Tests:**

- `scale` is neutral at 10 and symmetric. The work numbers are unchanged:
  the existing work-pace tests pass with no edits.
- Every work reader iterates `WORK`, so the staff table and the selected-row
  line show no combat rows.
- **The 0.21.16 load trap:** a save listing only the six work skills loads
  combat tracks at the species seed, not 0. Write it as a save→load test, not
  a RON round trip.
- The census: `Skill::ALL` is `WORK ∪ COMBAT` with no overlap.

**Gate:** `cargo test --workspace` is green, and `balance_sim` is untouched.

## Phase 2: attributes and the base-figure door (engine, assets)

**Files:**
- `attributes.rs`, with `DerivedStat::Accuracy`
- `progression.rs` (`derive`, `DerivedBase`/`DerivedStats` gain `accuracy`)
- `components/stats.rs` (`Derived::accuracy`)
- `assets/attributes/{analysis,amplitude}.ron`, `assets/attributes/README.md`
- `assets/classes/*.ron`, `assets/classes/README.md`
- the class-attribute census (`rg -n "six" crates/engine/src/tests`)
- a new `game/figures.rs`, or the home `module-boundaries` picks

**Interfaces:**

- `Game::base_figure(entity, DerivedStat) -> f64`.
  - The player arm reads the derived output.
  - The program arm reads the stored stat, or 0 where programs have none.
  - It is the one door. Callers never branch on the player.
- Amplitude: `effects: [(stat: Atk, per_point: 1.0)]`. Analysis drops `Atk`
  and gains `Accuracy` at a per-point figure fitted so the highest-Analysis
  class gains a modest edge. Record the reasoning in the attribute's
  `does:` line and the README.
- Each class's `amplitude` = `amplitude.base + (its analysis - analysis.base)`,
  so its starting ATK is unchanged.
- **Old player saves:** an absent Amplitude loads as the class's authored
  value, and its ATK must not move on the next derive. Check
  `attributes::authored_or_base` and the player load path.

**Tests:**

- For every class, ATK at creation is identical before and after. Capture the
  figures before the change.
- Accuracy rises only for classes above the Analysis base.
- `base_figure` returns the derived figure for the player and the stored
  figure for a program.
- An old-player-save fixture keeps its ATK through load → level-up derive.

**Gate:** `cargo test -p feral-processes-engine balance_sim`.
- ATK curves must not move.
- An accuracy rise, if any curve moves, is explained in the commit message
  before anything is accepted.
- Never loosen an assertion.

## Phase 3: combat doors and practice (engine)

**Files:**
- `battle.rs` (`Combatant::damage_scale`, `resolve_attack_inner`, `expected_damage`)
- `game/combat_damage.rs` (`combatant_profile`, `resolve_and_apply_attack_among`)
- `game/combat_round.rs` (`effective_mitigation`)
- `abilities.rs` (`routine_power_cost`), `game/field.rs` (`spend_power`)
- every `routine_power_cost` caller (`rg -n "routine_power_cost"`)
- `balance_sim.rs` and the arena constructors (`damage_scale: 1.0`)
- `tuning/skills.rs`, and a new `game/combat_skills.rs` or the home
  `module-boundaries` picks, holding the effect functions and the practice
  hooks

**Interfaces (all effects are pure functions of the level):**

- **Aim:** `accuracy *= scale(Aim, COMBAT_SKILL_SPREAD = 0.1)`, in
  `combatant_profile`. The Analysis contribution enters `accuracy_of` through
  `base_figure(…, Accuracy)` added to the existing bonus argument.
- **Force:** `Combatant::damage_scale = scale(Force)`. A landed `Hit`/`Crit`
  is `round(dmg * damage_scale)`. `expected_damage` multiplies the same way.
- **Defence:** `+(level − 10)` points added to `base` inside
  `effective_mitigation`, before status scaling and the existing clamp.
- **Routines:** `routine_power_cost(def, level: Option<u32>)` multiplies by
  `2 − scale(Routines)`.
  - `ability_unavailable`, `spend_power` and the routine page pass the
    invoker's level. A view with no invoker passes `None`.
- **Practice:**
  - In `resolve_and_apply_attack_among`, after the roll: Aim for the
    attacker, Defence for the defender, and Force for the attacker on
    `Hit`/`Crit`.
  - Routines practises inside `spend_power`.
  - Each hook is a no-op for a body without `Skills`, and never fires for a
    prop, nest or rock.
  - Constants: `AIM_PRACTICE_PER_ROLL`, `FORCE_PRACTICE_PER_HIT`,
    `DEFENCE_PRACTICE_PER_ROLL`, `ROUTINES_PRACTICE_PER_CHARGE`. Set them from
    `docs/measurements/` (run length, fights per zone) against the pace
    intent, with the assumption written in each doc comment.
  - The level-up line reuses Phase 1's announce path.

**Tests:**

- **Each effect:** at level 10 it is the identity. At 0 it is worse and at 20
  better, by the spread.
- **Wild body:** a body without `Skills` gets byte-identical profiles and
  costs to today.
- **Practice:**
  - A miss practises Aim and not Force.
  - A hit practises both.
  - The defender practises Defence.
  - A refused routine practises nothing.
  - A fizzled routine practises Routines.
  - A passive never practises.
- **Prices agree:** the refusal, the charge and the page quote one price at a
  non-neutral level.
- **Seeded fight:** the draw stream is unchanged by skills. Compare the
  battle log of a skill-10 party with one without `Skills`.

**Gate:** `cargo test --workspace`. Mutation-check the four effect sites and
the four hooks: remove each one, and a test must fail.

## Phase 4: the player carries skills (engine, saves, classes)

**Files:**
- `classes.rs` (`ClassDef::starting_skills`, `apply_kit` or its creation caller)
- `game/creation.rs`
- `save/mod.rs` (`PlayerSave::skills`, `#[serde(default)]`)
- `game/lifecycle/{save,load}.rs`
- `game/base/skill_scale.rs` (player guard)
- the class census

**Interfaces:**

- `ClassDef::starting_skills: Vec<(Skill, u32)>`, `#[serde(default)]`.
  - Census: combat skills only, each within the cap.
  - A malformed entry is skipped with a warning (the `load_dir` pattern).
- The player gets `Skills` when the class is applied, seeded from the class.
  A classless player is all 0.
- **Load:** the player's saved rows are overlaid on the class seed.
- **Work stays blind to the player:** `skill_scale.rs` treats the player
  entity as having no skills, at one site.
- Fusion: confirm `fuse_companions` carries the dominant parent's `Skills`.
  Fix it if not.

**Tests:**

- **Saves:** player skills save→load. An old player save (no field) loads at
  the class seed.
- **Work:** the player working a structure keeps today's pace exactly, even
  with work tracks at 0. Mutation-check the guard.
- **Practice:** the player practises Aim, Force, Defence and Routines in a fight.
- **Fusion:** a fused child keeps its dominant parent's skill levels.

**Gate:** `cargo test --workspace`.

## Phase 5: content seeds (assets)

**Files:**
- `assets/species/*.ron` (combat entries in `starting_skills`),
  `assets/species/README.md`
- `assets/classes/*.ron` (`starting_skills`), `assets/classes/README.md`

**Content:**

- **Classes:** Striker→Force, Bastion→Defence, Invoker→Routines,
  Saboteur→Aim. Map the other four by what each class's file says it is for,
  at the same few levels as species work knacks.
- **Species:** one combat knack per species where its role or moves point at
  one; none where nothing does. List every number in the READMEs.

**Gate:**
- The species and class censuses are green.
- `balance_sim` is unchanged, since it is skill-blind. Confirm this rather
  than assume it.
- One arena run of `dev-arenas/opening-fight.ron` with the party at seed
  levels against level 10. Read `docs/measurements/README.md` first, and add
  a measurement only if it meets that bar.

## Phase 6: screens (app-core, gui)

**Files:**
- engine views:
  - the program skills view (`views/manifest.rs`/`work.rs`)
  - a player skills view on whichever player view feeds the stats page
- app-core: the mode that shows the player's stats
- gui: `render/manifest/{mod,sections}.rs`, and the player stats page renderer

**Work:**

- **Program SKILLS box:** a COMBAT group under the work rows. If it does not
  fit at 1280x720, use two columns. Measure with `--screenshot` on a dev-save
  and Read the PNG.
- **Player:** a SKILLS box with the four combat rows.
- **Level-up nouns:** pick final nouns for the four skills (plain, no occult
  words). "You" for the player.
- **Points / dossier:** Amplitude shows as the seventh row, and Analysis's
  text reads as to-hit.
- Run the popup row-width and the no-clip headless tests that exist for these
  panels.

**Gate:**
- `cargo test --workspace`.
- One screenshot each of a program's info page and the player's page,
  Read and checked.

## Phase 7: seams, docs and release prep

- **Seams, three places each** (graph, `seams` skill, rules file):
  - the class seam ("affinities and starting combat skills");
  - a new seam: "a combat figure is `base_figure` × `skills::scale`, one
    door each";
  - the cost-door signature.
- **Docs:**
  - ADR 0040 is already Accepted (2026-11-02, with the plan);
  - update `docs/superpowers/INDEX.md`;
  - add a `CHANGELOG.md` entry, plain language and player-facing. A save
    still loads, so this is not breaking; take the digit from the preamble.
- **Review:** the final whole-branch review, opus. Give it the diff as a file
  and the spec's "calls" list as the rule to check against. Each fix gets its
  own review.
- Stop there. Landing is `deploy` on the user's word.
