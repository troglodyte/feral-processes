# Stun moves: which lever moves them

**Date:** 2026-08-10
**Instrument:** `train --assets <tree> --log-dir`, then `analysis/`
**Follows:** [the pin sweep](2026-08-10-enemy-policy-pin-sweep.md), which found
trained policies collapsing toward one move per species.

## The claim

Three moves were dead in the shipped game — cipher's Encrypt, crawler's
Freeze, rootkit's Privilege Escalation, each picked on 1–2% of swings where
a choice existed. All three are **Stun**; no Bleed move is dead.

**Repricing cannot fix this, and the 2026-08-09 report's framing pointed at
the wrong lever.** Raising each stun move to its sibling's power flips usage
from ~2% to ~96% — the *sibling* becomes the dead move and variety is
unchanged. Power behaves as a switch, not a dial: at
`ENEMY_POLICY_TEMPERATURE` 1.0 the softmax over these learned weights is
sharp enough that there is probably no power value yielding a mixed
distribution.

Raising **stun duration** 1→2, leaving power alone, is the lever that works
for those three moves: usage 12–20%, top-move share ~0.98 → 0.80–0.88, no
meaningful difficulty cost. **But see the correction below** — across the
whole roster the retrain that shipped with it made most species *less*
varied, so the change did not do at the game level what it did at the level
it was measured.

## How to reproduce it

Copy `assets/` to a scratch tree, edit the three moves, then:

```sh
./target/release/train --assets <tree> --out <tree-weights>.ron \
    --scenarios dev-training --iters 30 --pop 40 --reps 200 --seed 1 \
    --pin target_is_player,target_bracing,target_def_rel \
    --log-dir dev-logs/policy-sweep --label <variant>
```

Measure in **groups 0–1 only**. Further back a non-ranged move cannot reach
at all, so including those swings measures formation rather than choice —
40% of cipher's swings are group-2 and would halve its apparent rate.

## The numbers

Stun-move share of swings, groups 0–1:

| species | shipped | power parity | duration 2 |
|---|---|---|---|
| cipher (Encrypt) | 0.011 | 0.960 | 0.134 |
| crawler (Freeze) | 0.018 | 0.990 | 0.123 |
| rootkit (Priv. Esc.) | 0.023 | 0.959 | 0.204 |

Top-move share — the variety measure, and the one that matters:

| species | shipped | power parity | duration 2 |
|---|---|---|---|
| cipher | 0.99 | 0.96 | **0.87** |
| crawler | 0.98 | 0.99 | **0.88** |
| rootkit | 0.98 | 0.96 | **0.80** |

Parity leaves it flat. That is the whole argument against repricing.

Enemy win rate, as improvement over each config's **own** baseline, which is
the only valid comparison:

| config | baseline | trained | delta |
|---|---|---|---|
| shipped assets | 0.254 | 0.551 | +0.297 |
| power parity | 0.262 | 0.563 | +0.301 |
| duration 2 | 0.263 | 0.565 | +0.302 |

## The asset and the weights are a matched pair

**Duration is not a policy feature.** The 19 features encode
`move_effect_stun`, `move_effect_chance` and `move_has_effect` — nothing for
how long a stun lasts. The enemy cannot observe the change.

What the change does is alter what the *trainer* measures: longer stuns win
more fights, so the search stops penalising stun moves. Retrained against
duration-2 assets, `move_effect_stun` went **−2.81 → +0.75** and
`move_has_effect` **−0.68 → +0.87**.

So the `.ron` edit alone is inert. Shipping it without retraining
`assets/policies/enemy_battle.ron` delivers none of the above and looks like
a change that did nothing.

## Correction: the roster went the other way

**Added after the fact.** The top-move table above covers the three species
whose files were edited. That is a cherry-picked sample, chosen because they
were the ones changed, and it is not what happened to the game.

A retrain replaces all nineteen weights, so every species' move choice
moves. Measured across the whole roster, shipped `pin3` → retrained
`longstun`, groups 0–1, **lower is more varied**:

| species | before | after | |
|---|---|---|---|
| cipher | 0.99 | 0.87 | edited |
| crawler | 0.98 | 0.88 | edited |
| rootkit | 0.98 | 0.80 | edited |
| trojan | 0.91 | 0.60 | |
| construct | 1.00 | 0.94 | |
| scrapper | 0.74 | 0.72 | |
| overseer | 1.00 | 1.00 | |
| virus | 0.78 | 0.85 | |
| wintermute | 0.88 | 0.94 | |
| worm | 0.94 | 1.00 | |
| sprite | 0.87 | 0.98 | opening ring |
| proxy | 0.81 | 0.95 | |
| zero_day | 0.54 | 0.70 | |
| drone | 0.79 | 0.95 | opening ring |
| sub_process | 0.57 | 0.77 | opening ring |
| glitch | 0.58 | 0.82 | opening ring |

Three edited species got more varied. **Eight unedited ones got less**,
including all four opening-ring programs a new player meets. At the roster
level this change *reduced* move variety, which is the opposite of its
stated purpose.

### Why, and why it may not matter

The cause is not stuns. `move_power_rel` flipped **−5.00 → +2.14** in the
retrain — the largest single weight change, and the only sign flip that
touches every species. Preferring the higher-power move collapses choice for
any species whose moves differ only in power, which is most of the roster;
the four opening-ring programs carry no effect moves at all.

The case that this is acceptable: for drone (Buzz 4, Recon Ping 3) "variety"
is a coin flip between near-identical options, and concentrating on the
better one is correct play rather than lost tactics. The variety worth
having is cipher's — damage against a status effect — and that improved.

**But the coefficient may not be meaningful.** `move_power_rel` and
`est_damage_frac` are strongly correlated, so a linear model's split between
them is under-determined: many combinations score almost identically and the
optimiser slides along that ridge when the landscape shifts. `est_damage_frac`
barely moved (10.10 → 10.13) while its correlated partner swung 7.14, which
is what a slide looks like. **Unresolved:** retrain on two further seeds. If
`move_power_rel` lands near +2 each time it is a finding; if it scatters
while fitness holds, a roster-wide behaviour change is riding on noise.

### The process failure

Three species were measured because three species were edited. Nobody
checked the other thirteen, and the change shipped on a claim that is false
at the level anyone would care about. Same shape as the census test that
turned out to be one species picked by filesystem order — a conclusion drawn
from whichever subset was convenient to look at.

## What it does not say

- **The balance gate is blind to this.** `cargo test balance_sim` passes,
  and that is not evidence: it is RNG-free and models no abilities, so an
  effect magnitude is invisible to it. This change shipped **ungated** by any
  automated balance check. Say so rather than citing the green run.
- **The opening game is unchanged, and the reason is unknown.**
  `a_trained_policy_rarely_picks_an_effect_carrying_move` measures cipher in
  a level-1 1v1 (its fixture takes `species_defs().next()`, which is cipher
  by filesystem order — not a designed choice). It still reports 1 in 300
  after the change. The measured gain lives in the `dev-training` population,
  which is mid- and late-game group fights. A mechanism was proposed —
  low-HP targets cap `est_damage_frac` and widen the damage gap — and the
  data **contradicts** it: across cipher's two scenarios usage rises as
  target HP falls (0.10 at 328 HP, 0.26 at 284 HP). Both facts are solid;
  the link between them is not established.
- **Nothing about whether it plays well.** A 2-turn stun is the textbook
  case of a mechanic that measures well and feels unfair. rootkit is common
  and would apply one on roughly 8% of its swings (0.20 usage × 0.40 proc).
  This shipped unplayed, and that is the first thing to check at a keyboard.
