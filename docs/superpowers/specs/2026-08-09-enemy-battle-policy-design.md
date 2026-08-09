# Learned enemy battle policy — design

**Date:** 2026-08-09
**Status:** approved, not implemented
**Scope:** `crates/engine`, `crates/launcher` — two crates, so this earns the
full spec-and-plan pipeline per `CLAUDE.md`'s process-weight rule.
**Save format:** unchanged. Weights are an asset, not run state.

## What exists today

A wild program makes four decisions per round in
`Game::wild_retaliate` (`crates/engine/src/game/combat_enemy.rs`), and none
of them is a decision:

| Decision | Today |
|---|---|
| Routine or move | Routine if any is off cooldown, else move. First installed wins. |
| Which move | Uniform random over the species' moveset, filtered by reach (`ENGAGED_GROUPS`) |
| Reach for the move's status effect | Flat `WILD_ABILITY_CHANCE` (0.2), composed with the move's own 0.3–0.5 |
| Which target | `roll_enemy_target`, weighted by `battle::slot_aggro_weight` |

`TODO.md` carries `[ ] machine learning for enemies`. This is that.

## Why this repo can host it

`crates/engine/src/arena/` is already a seeded, deterministic, headless
battle harness: `arena::stage` builds a fight from a `Scenario`, `run_rep`
plays it out, and `arena::Watch` reads the outcome — including the two
non-obvious parts, that "won" is read off the opponents rather than the
player's HP, and that an HP fraction is sampled per round and skipped on a
round that granted a level. That is the environment and the reward function,
already written and already tested.

## The limitation this design accepts

`run_rep` plays the party's side as **All-Attack** —
`battle_plan_remaining(Attack { group: 0 })` for every slot, which is the
game's own `[A]`. Its doc comment says why: "not a policy engine written for
the tester, so the arena cannot drift from the game by inventing decisions
the game never makes." No companion Specials fire in auto-play.

So the policy trains against a **fixed, non-adaptive opponent**. That is a
benchmark, not a sparring partner. The honest headline claim is "beats
All-Attack by N percentage points", and the risk to state plainly is
overfitting to a party that never braces and never spends a Special.
Training both sides is a separate project and would have to argue with that
doc comment first.

## Decisions

| Question | Answer |
|---|---|
| What learns | Enemy battle policy. Other targets (balance tuning, adaptive difficulty, procedural generation) deferred. |
| Where it runs | Trained offline, frozen weights ship as an asset. |
| Which decisions | Target and move, chosen **jointly**. Routine timing and the effect coin-flip are untouched. |
| Objective | Enemy win rate, with sampling temperature as the dial-back. |
| One brain or many | One global policy over species *features*, so a modded species gets sensible behaviour with no weights of its own. |
| Output to action | Seeded sample from the learned distribution via `GameRng`, at a `tuning.rs` temperature. |
| Defend | The policy may learn about bracing, but a census test pins Defend against a retrain that guts it. |

## Non-goals

- Routine selection and timing (`WILD_ROUTINE_CHANCE` is 0.06; almost all
  training signal would go to a branch that rarely fires)
- The `WILD_ABILITY_CHANCE` effect gate
- Overworld `wander_ai_system` and `pursuit_field`
- Party-side or companion AI
- Any runtime or online learning

## Architecture

Four pieces across two crates.

| Piece | Where | What it is |
|---|---|---|
| The model | `crates/engine/src/policy.rs` | Pure math: feature vector, dot product, softmax. No `World` access, unit-testable alone. |
| Feature extraction | `crates/engine/src/game/combat_policy.rs` | Reads battle state into the feature vector. Needs `Game`, so it sits with the other `game/` modules. |
| The weights | `assets/policies/enemy_battle.ron` | Data, loaded like every other DB. |
| The trainer | `crates/launcher/src/bin/train.rs` | CEM over the arena. Sits with `savetool` and `arena` for the same `default-run` reason those two do. |

**The seam is one function.** `Game::choose_wild_action(wild, group, player)
-> Option<(MoveDef, Entity)>` returns the pair a wild program will use, and is
the only place that decision is made. `None` means nothing reaches — the
existing "circles beyond reach" path. It lives in `game/combat_policy.rs`
beside the feature extraction, and `wild_retaliate` calls it in place of the
two rolls currently inline there. Both the uniform baseline and the learned
policy exit through it — the same "one walk" / "one way in" shape as
`view_cone`, `Game::enter_frame` and `Game::arrive`.

`roll_enemy_target` is **not** orphaned. The routine branch still calls it
(routines are out of scope) and so does the no-weights fallback path. Two
real callers.

## The scoring model

For each candidate `(move, target)` pair:

```
score = ln(slot_aggro_weight(slot, bracing))  +  w · φ(move, target, attacker)
```

The aggro term is a **fixed prior with a pinned coefficient of 1.0**, not a
learned feature. Softmax exponentiates, so `exp(ln 3) = 3` returns the
existing weight table exactly, and the learned part composes as a multiplier:

```
exp( ln(aggro) + w·φ )  ==  aggro × exp(w·φ)
```

The learned model therefore **multiplies today's odds rather than replacing
them**. Two properties follow, and both are the reason for the choice:

1. **An all-zero weight vector reproduces today's distribution exactly** —
   uniform move choice crossed with slot-weighted targeting. Installing this
   feature untrained cannot move the game. Without the prior, all-zero
   weights would score every target identically: with a player and four
   companions the front three would go from 27% to 20% and the back two from
   9% to 20%, a real behaviour change caused by wiring alone.
2. **The existing design keeps working by default** rather than having to be
   rediscovered from scratch by a trainer.

`slot_aggro_weight` stays the one function both sides call, per the rule that
a doc comment claiming to mirror other code must be a call and not a copy.

Sampling is a softmax at `ENEMY_POLICY_TEMPERATURE` (new `tuning.rs`
constant), drawn through `GameRng`. Subtract the max score before `exp` so a
large weight cannot overflow. Temperature is the shipping control: 0
approaches argmax, and a high temperature approaches the uniform baseline.

## The feature vector

Roughly twenty features, each normalised, **no bias term** — a bias cancels
under a softmax over actions.

**Move:** power relative to that species' own best move (relative rather than
absolute, so it holds for any modded roster), ranged, carries an effect,
effect is `Stun`, effect is `Bleed`, the effect's own `.ron` chance.

**Target:** HP fraction, is-the-player, effective DEF against this attacker,
already stunned, already bleeding, is bracing, estimated damage as a fraction
of remaining HP, would-kill.

**Attacker:** own HP fraction, in a front group (`group < ENGAGED_GROUPS`).

**Interactions** — the cheap way to buy a linear model the nonlinearity that
matters here: `carries_effect × target_healthy`,
`stun × not_already_stunned`, `bleed × not_already_bleeding`.

The damage estimate calls `battle::compute_damage`. It is a call, not a copy.

### Why action-features rather than one output per action

The same weight vector scores every candidate pair, so the parameter count
does not depend on how many moves a species has or how large the party is. A
mod shipping a species with seven moves is scored by the same brain with no
retraining. A network with one output per action could not do that, and the
"one global policy" decision would fail its own moddability rationale.

## The weight file

`assets/policies/enemy_battle.ron`, keyed **by feature name, not position**:

```ron
(
    features: [
        ("target_hp_frac",   -1.84),
        ("would_kill",        2.31),
        ("est_damage_frac",   0.97),
    ],
)
```

An unknown name warns and is ignored; an absent name defaults to zero. A
positionally-encoded vector would silently score garbage the first time
someone added a feature; this degrades instead.

It also makes the trained result legible — `target_hp_frac: -1.84` *is* the
sentence "finish the wounded one". Being able to read what was learned is a
large part of why a linear model is the right choice for a proof of concept,
and is what the deliverable in "Training" below reports.

Loading follows `SpeciesDb::load_dir` / `ItemDb::load_dir`: a malformed file
is skipped with a logged warning, never a panic.

## Training

```sh
cargo run --release --bin train -- --out assets/policies/enemy_battle.ron \
    --scenarios dev-training/ --iters 30 --pop 40 --reps 200 --seed 1
```

Cross-entropy method: sample `pop` weight vectors from a Gaussian, evaluate
each over the scenario set, keep the top decile, refit mean and variance,
repeat for `iters`. No gradient code, and it optimises the actual metric
rather than a differentiable surrogate. CEM is a standard derivative-free RL
method, competitive with policy gradients at this parameter count — not a
stand-in for one.

**Fitness** is enemy win rate, with mean player HP lost as a tiebreak. At low
win rates a pure win/loss signal is too sparse for the early generations to
have any gradient at all; grinding the party down is the honest partial
credit.

**Seeding.** Every candidate within a generation is evaluated on the *same*
seed set, so a comparison between candidates is signal rather than luck. The
seed set is re-rolled between generations so the result does not overfit to
one particular 200 fights. The whole run is reproducible from `--seed`.

**Scenario sets.** `dev-training/` is new — a sibling of `dev-saves/` and
`dev-arenas/`, spanning levels, zones, depths and party compositions. The
three hand-authored `dev-arenas/` scenarios are held back as the test set and
are never trained on.

**The deliverable** is the report the trainer emits: baseline win rate at
all-zero weights, final win rate, a per-scenario breakdown, and the
highest-magnitude weights by name. That report is what the proof of concept
actually proves.

## Error handling

Every failure resolves to "play exactly as today":

- No weights file → the uniform baseline, no warning (this is a valid state)
- Malformed RON → skipped with a logged warning
- Unknown feature name → warned and ignored; the rest of the file loads
- Non-finite weight → the file is rejected **at load**, not per-round, the
  same shape as `a_drain_with_a_non_finite_heal_fraction_is_skipped`
- Empty candidate set (nothing reaches) → the existing "circles beyond reach"
  line, untouched

## Testing

Engine:

- `an_all_zero_policy_reproduces_the_uniform_baseline` — the equivalence the
  aggro prior buys, and the proof the seam is wired without asserting on any
  learned number
- `a_policy_that_prefers_the_wounded_focus_fires` — hand-authored weights,
  asserting **behaviour**, not that a function was called
- `bracing_still_draws_more_fire_than_not_under_the_shipped_weights` — the
  Defend census. A retrain that cancels the aggro prior fails the suite
  instead of quietly devaluing a designed player action. `balance_sim` cannot
  cover this: its own docs say it passes `defending: false` throughout and
  models no Defend actions.
- `a_high_temperature_approaches_the_uniform_baseline`
- `the_same_seed_and_weights_replay_the_same_fight`
- `a_malformed_policy_file_is_skipped`
- `an_unknown_feature_name_is_ignored`
- `a_modded_species_with_more_moves_still_scores` — the moddability claim,
  walked with a modded species rather than assumed

Trainer, in the launcher's existing `[lib]`:

- CEM converges on a toy quadratic objective — proves the optimiser with no
  game involved
- A fixed seed reproduces a training run

Each of these must fail with the change removed — delete the implementation
and watch the test go red — **with one deliberate exception**.
`an_all_zero_policy_reproduces_the_uniform_baseline` passes trivially when
the policy does not exist, because with no policy the behaviour *is* the
baseline. It is an equivalence guard against future drift, not a
fix-removal test, and the spec records that so nobody later mistakes it for
coverage. Two tests written on 2026-08-09 were vacuous and read as coverage;
this is the same trap, caught in advance.

## Known gaps

- **`balance_sim` is blind to this.** It is RNG-free and models no abilities,
  so a policy that makes real fights substantially harder moves none of its
  curves. The usual balance regression gate does not apply. The arena report
  is the only instrument, and `ENEMY_POLICY_TEMPERATURE` is the only control.
- **Trained against All-Attack**, with no companion Specials — see "The
  limitation this design accepts".
- **A green suite is not evidence of play.** How a learned enemy *feels* is
  unmeasurable headlessly and needs `FERAL_DEV_ARENA=1 cargo run`, `[R]`,
  before and after.

## Documentation obligations

- `assets/policies/README.md` — new, and required: it is the schema reference
  for anyone modding a policy, per the moddability rules.
- `CHANGELOG.md` — a section at the version this lands under.
- `CLAUDE.md` — a load-bearing-seam entry for `Game::choose_wild_action` being
  the one place a wild program's swing is decided, and for the aggro prior
  being what makes an untrained policy identical to today.
- `docs/manual.md` and root `README.md` are carved out and stay stale.
