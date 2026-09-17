# 2026-09-17 — The arena cannot see opportunity attacks

## The claim

Opportunity attacks change **nothing** about the only shipped tactical arena
scenario, and not because they are weak: **zero reactions fire in fifty
reps**. The before and after runs of `dev-arenas/tactical-full-group.ron`
are identical to every digit the bin prints — 100% win rate, 6.7 mean
rounds, 96% mean player Integrity, no companions down — and a grep of the
report's narration finds no interrupt line at all.

The reason is structural rather than incidental. Both triggers need
something the AI never does in a straight brawl. **Leaving reach** is not in
the planner's repertoire: `cell_merit` pays `TACTICAL_AI_REACH_SCORE` for
standing somewhere it can swing from, and `scored_cells` only offers cells
that *beat* the one a body is on, so a hostile already in reach holds its
ground by construction (`seam:staying-put-is-the-default`). **Invoking in
melee** never happens either — every line in the report is a basic attack
(`Rootkit 3 injects Kernel Panic`, `Scrapper 3's Ram is refused`) because no
body in that scenario carries a routine `wild_routine_ready` will pick.

So the feature's whole weight lands on **the player**, who is the only body
on the board that disengages or invokes at will. That is a design outcome
worth knowing before anyone tunes it: a reaction multiplier raised because
"the arena says it does nothing" would be tuning against an instrument that
is blind to it, and the number it moved would land on the player alone.

## How to reproduce it

The two runs are the same command on either side of the feature. Sharing
`CARGO_TARGET_DIR` is what keeps the baseline build warm — a fresh worktree
otherwise rebuilds the 557-dependency Bevy graph.

```sh
# After (on the branch):
cargo run --bin arena -- dev-arenas/tactical-full-group.ron --out after.ron

# Before:
git worktree add /tmp/pre-reactions 4ddb97d6
cd /tmp/pre-reactions
CARGO_TARGET_DIR=/home/trog/code/feral-processes/target \
  cargo run --bin arena -- dev-arenas/tactical-full-group.ron

# What makes the identical numbers a finding rather than a coincidence:
rg -c interrupt after.ron      # 0
```

The scenario is 50 reps at seed 7: a level-20 geared player and three
level-12 Scrappers against four Rootkits, `model: Tactical`, approaching
from the East.

## The numbers

| | before (`4ddb97d6`) | after (`45e11557`) |
| --- | --- | --- |
| win rate | 100.0% (50/50) | 100.0% (50/50) |
| rounds | mean 6.7, median 7 | mean 6.7, median 7 |
| player Integrity left | mean 96% | mean 96% |
| companions down | 0.00 | 0.00 |
| reactions fired | — | **0** |

Nothing here is new about the fight; the whole finding is the last row. The
identical stream is itself evidence: a reaction that fired would draw from
`GameRng`, so a shifted mean would have been the first symptom. `walk_risk`
spends no draw (`battle::expected_damage` is RNG-free), which is why the
rest of the run reproduces exactly.

## What it does not say

- **Nothing about whether reactions are priced right.** They were never
  exercised. `TACTICAL_AI_REACTION_WEIGHT` and the absence of a damage
  multiplier on a reaction swing are both unmeasured, and this run cannot
  be quoted about either.
- **Nothing about the player's side**, which is where the feature actually
  lands. Measuring that needs a person at the keyboard, or a scenario whose
  party disengages — neither exists today.
- **Nothing about the fizzle.** No body in this scenario invokes anything,
  so the routine trigger is untouched by these fifty reps.
- `balance_sim` has no board and gates none of this, which is why the
  question was put to the arena at all.

## Open questions

- **Should a scenario exist that can see this?** It would need an AI that
  disengages — a hostile with a minimum range, or a "withdraw when hurt"
  rule the planner does not have — or a party carrying routines it will
  invoke in melee. Both are new behaviour, not new files, so the honest
  position today is that the instrument does not reach the feature.
- **Should the AI ever disengage at all?** If it should, the reaction cost
  is already priced and waiting for it (`Game::walk_risk`); if it should
  not, half of this feature is permanently the player's problem alone and
  the AI's half is dead weight worth revisiting.
