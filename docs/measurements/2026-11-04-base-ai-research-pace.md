# The base AI: research pace, and a proposed balance table

## The claim

A typical researcher makes about **0.029 Research Data a tick** (one point every
~17 seconds of play) at one Research Station. Today's base tree is 3,274 points
across 39 nodes, so a single station finishes it in **15.6 hours of
research**.

The placeholder AI figures fail two of the spec's three bars:

- Practice is too dear. An AI trained at every threshold reaches only level 3 by
  the end of the tree, and its pace peaks at 0.93x the pre-fusion pace, so it
  never gets back to the old speed.
- Nothing is gated, so no level unlocks anything.

The proposed table below clears all three bars for a fusion Analysis of 15 to
30:

- An untrained AI finishes all 33 ungated nodes in about 29 hours, against 23
  hours at the old pace on the same costs.
- An AI trained at every threshold finishes all 39 nodes in about 19 hours. It
  is back to the old pace about 4 hours after the awakening and ends at 1.76x.
- Each of the four levels opens at least one node.

The table is a proposal. Nothing in `assets/` or `tuning/` has been changed.

## How to reproduce it

```sh
unset FERAL_DEV_NO_SIEGES
cargo test -p feral-processes-engine storyline::measure -- --ignored --nocapture
```

The harness is `crates/engine/src/storyline/measure.rs`, and both tests are
`#[ignore]`d. It is deterministic: no RNG draws and no wall clock.

- **`measure_ordinary_research_pace`** loads `dev-saves/{fred,study,extraction,chains}.ron`
  against the real `assets/`. For every base staff program it calls
  `Game::host_research_pace` at the first Research Station. This is the same
  call the fusion freezes into `FusedAi::pace_at_fusion`, and it goes through
  `systems::expected_gather_per_tick` and the program's real Research skill,
  species speed, morale and need strain. It also prints the host that
  `pick_host` would claim.
- **`measure_tree_completion`** walks the real base `ResearchDb` (the 39
  `assets/research/*.ron` nodes; the synthesised routine and affix nodes are
  excluded) tick by tick for three players and three triggers (5, 7 and 10):
  - **NoFusion** researches at the ordinary pace, with gates open.
  - **Untrained** fuses at the trigger, sits through `BASE_AI_FUGUE_TICKS` and
    never selects training.
  - **Trained** selects Train your AI the tick the AI stalls, while a level is
    left.

  Each tick replays `Game::tick_fused_ai`: `carry += pace(..)`, a whole-unit
  credit capped at the cost, then either training completion or `practise`
  when there was room. The **Live** rows call the real `practise`,
  `analysis_cap` and `training_cost`. The **Proposed** rows use a stand-in with
  the same shapes over the figures below. When the table is applied, Live must
  print exactly what Proposed printed, and Proposed is then deleted.

Times assume the world's two ticks a second (`Normal` speed). The `Fastest`
speed halves them.

## The numbers

### Ordinary pace (new)

| Template | Zone | Researched | Stations (posted) | Staff | Analysis median / max | Pace min / median / max (per tick) | Host: Analysis, pace |
|---|---|---|---|---|---|---|---|
| `fred` (played) | 1 | 13 | 1 (0) | 24 | 11 / 22 | 0.0114 / 0.0244 / 0.0370 | 22, **0.02909** |
| `extraction` | 3 | 13 | 2 (2) | 4 | 11 / 15 | 0.0110 / 0.0206 / 0.0227 | 15, 0.02265 |
| `chains` | 3 | 21 | 2 (2) | 15 | 13 / 17 | 0.0119 / 0.0220 / 0.0244 | 17, 0.02442 |
| `study` | — | — | — | 0 | — | — | its only program is pinned |

"Typical" here means `fred`'s host: Analysis 22 and pace 0.02909. `fred` is the
only save that came from real play, and the spec names it as the save past its
trigger. The host is, by rule, the staff program with the highest Analysis.
Across the three bases that figure is 15 to 22, which fits the spec's "others
top out around 20".

Pace does not follow Analysis. The second-best program in `fred`, at Analysis
21, is the fastest at 0.037, because `mining_success_chance` saturates and the
spread comes from species speed and skill. The host's pace is what gets frozen,
and it sits at the upper middle of the range.

### Tree completion, live placeholders (new)

The tree is 39 nodes and 3,274 points, with no gates. The fusion is at Analysis
22 with a pace of 0.02909. The trigger is 7; triggers 5 and 10 move every time
by under 0.4 hours.

| Player | Tree done | After the awakening | Nodes | Level | End pace |
|---|---|---|---|---|---|
| No fusion (today) | **15.6 h** | — | 39 | — | 1.00x |
| Fused, never trained | **42.3 h** | 41.9 h | 39 | 0 | 0.36x |
| Trained at each threshold | **21.6 h** | 21.1 h | 39 | 3 of 5 | 0.93x (never back to 1.0x) |

The arithmetic behind the failure: at `50 + 2g²` practice a point, reaching
+100 Analysis takes about 662,000 ticks, or 92 hours. That is six times the
whole tree. Because `practice_quad` is a `u32`, any quadratic term of at least 1
costs at least 328,000 ticks to reach +100. One practice a tick cannot carry
the curve, whatever the integers are.

### The proposed table

| Knob | Today (placeholder) | Proposed |
|---|---|---|
| Levels in this build | 5 | **4** |
| `BASE_AI_LEVEL_OFFSETS` | `[10, 25, 45, 70, 100]` | **`[20, 45, 70, 100]`** |
| Practice a tick | 1 | **8** (new `BASE_AI_PRACTICE_PER_TICK`) |
| `BASE_AI_PRACTICE_BASE` / `_QUAD` | 50 / 2 | **1600 / 1** |
| Ticks per Analysis point | 50 + 2g² | 200 + g²/8 (g = points since the fusion) |
| Training cost, levels 0→1, 1→2, 2→3, 3→4 | 20, 40, 60, 80, 100 | **20, 40, 60, 80** (the current `20 + 20·level`, unchanged) |
| `BASE_AI_START_SPEED`, `BASE_AI_FUGUE_TICKS` | 0.25, 120 | unchanged (set by the spec) |

Practice per tick enters as `practise(&fused, analysis, BASE_AI_PRACTICE_PER_TICK)`
in `tick_fused_ai`. The pure function is unchanged, because calling it with
`ticks = 8` is eight single-practice steps.

With an Analysis of 22 at fusion, the pace at each cap is:

| Cap | Pace |
|---|---|
| +20 | 0.48x |
| +45 | 0.76x |
| +70 | 1.05x |
| +100 | 1.39x |

After the last level the AI has no cap and keeps climbing.

**AI gates** (a node that requires a gated node is gated too, so every ungated
node stays reachable):

| Level | Opens | Requires |
|---|---|---|
| 1 | `memory_mapping`, `deep_analysis` | `paging`; `routine_fabrication` |
| 2 | `cortex` | `neural_amp` |
| 3 | `model_inspection`, `wide_broadcast_decompile` | `cortex`; `broadcast_decompile` |
| 4 | `cold_archive` | `shard_mapping` |

**Costs.** The 13 nodes costing 8 to 30 are unchanged: `automation`,
`power_grid`, `containment`, `teardown`, `commerce`, `siphoning`,
`fortification`, `outpost_founding`, `deception`, `armor_bench`,
`weapon_bench`, `routine_fabrication` and `remote_decompile`. Every node
costing 40 or more goes up about 1.5x, rounded to a multiple of 5:

| Old cost | New cost | Nodes |
|---|---|---|
| 40 | 60 | `cache_coherence`, `charge_density`, `mod_bench` |
| 45 | 70 | `dispatch`, `firewall`, `overclock` |
| 50 | 75 | `drop_pods`, `paging`, `wetware_splicing` |
| 55 | 85 | `long_range_decompile`, `neural_amp` |
| 60 | 90 | `memory_mapping` |
| 75 | 110 | `program_refactoring` |
| 90 | 135 | `capacitance`, `deep_splicing` |
| 110 | 165 | `ablative`, `monofilament` |
| 120 | 180 | `broadcast_decompile`, `segmentation` |
| 125 | 190 | `cortex` |
| 130 | 195 | `deep_analysis` |
| 160 | 240 | `model_inspection` |
| 220 | 330 | `virtual_memory`, `wide_broadcast_decompile` |
| 360 | 540 | `shard_mapping` |
| 540 | 810 | `cold_archive` |

The tree becomes 4,809 points. The 33 ungated nodes hold 2,954 of them.

### Tree completion under the proposal (new)

These are for the same host (Analysis 22, pace 0.02909) and a trigger of 7.
"Old pace" means the pre-fusion pace on the new costs, with gates ignored.

| Player | Tree done | Nodes | Levels reached at (after the awakening) | Back to old pace | End pace | Programs spent |
|---|---|---|---|---|---|---|
| Old pace, same costs | **23.0 h** | 39 | — | — | 1.00x | 34 |
| Fused, never trained | **29.3 h** | 33 (every ungated node) | — | never | 0.48x | 28 |
| Trained at each threshold | **18.7 h** | 39 | 0.8, 2.2, 4.6, 9.5 h | 3.9 h | 1.76x | 38 (4 trainings) |

For comparison, today's game finishes its own tree in 15.6 hours.

Against the spec's bars:

| Bar | Result |
|---|---|
| An untrained AI can finish every ungated project, slowly | Met: 33 of 33 nodes in 29.3 h, 1.27x the old pace's time on the same costs and 1.9x today's tree |
| Trained at every threshold ends up faster than before the fusion | Met: the end pace is 1.76x, it is back to the old pace 3.9 h after the awakening, and the whole tree takes 18.7 h against 23.0 h |
| Every level unlocks at least one project | Met: see the AI gates table |

The untrained AI stalls at its first cap about 36 minutes after the fusion. From
then on it waits on its first training, which is when the "waiting to be
trained" alert shows.

**Sensitivity to the host's Analysis** (trigger 7, everything else held):

| Fusion Analysis | Untrained | Trained | Old pace, same costs | Trained back to old pace | Trained end pace |
|---|---|---|---|---|---|
| 15 | 24.1 h | 14.9 h | 23.0 h | 1.9 h | 2.27x |
| 22 (`fred`) | 29.3 h | 18.7 h | 23.0 h | 3.9 h | 1.76x |
| 30 | 33.4 h | 22.4 h | 23.0 h | 7.5 h | 1.44x |

The trigger count barely matters. Across triggers 5 to 10 every proposed time
stays within 0.4 hours.

## What it does not say

- **These are research-bound times, not run lengths.** The walk ignores
  `min_zone`, materials, discovery and subject capture. It assumes a project is
  always selected and a pinned program is always on hand, and it researches
  cheapest-first. A real run waits on zones: `cold_archive` needs zone 6. Read
  the hours as a floor on the research share of a run, and the ratios between
  players as the finding.
- **One station.** `fred` has one Research Station, but `chains` and
  `extraction` have two, both worked. A player who ran two stations before the
  fusion was researching at about 2x p0. After the fusion they are held to one,
  and even the trained AI at 1.76x does not match them over the whole tree. That
  is the spec's anti-goal working as intended, but it makes the setback larger
  for multi-station bases than these tables show.
- **The host's Analysis moves the result most.** `pace` scales as Analysis over
  fusion Analysis, while the thresholds are absolute points. A sharper host
  (Analysis 30 or more) needs more points to break even and only just clears
  the trained-is-faster bar (22.4 h against 23.0 h). A host much above 30 would
  likely fail it, though that was not run. Fusion Analysis was measured only at
  15, 17 and 22 on these saves.
- **Expected value, not cycles.** The ordinary pace is the expected units per
  tick, so the cycle-to-cycle variance and the mining roll are averaged away.
  Morale and need strain are frozen at their values in the captured save.
- **Only the base tree.** The routine and affix nodes also cost Research Data at
  the same station. They are excluded here, and under the AI they would take
  longer too.
- **Programs spent are counted, not paced.** The trained player spends 38
  programs on studies and trainings, against 34 for the no-fusion player. The
  time to catch them is not modelled.
- **`balance_sim` was not run.** Nothing was applied, so it could not move.
  Task 8 step 4 runs it.

## Open questions

- **Gating tools.** `cortex` and `deep_analysis` hand over extraction tools.
  Under this table those tools are out of reach for good if the AI is never
  trained. Is that intended, or should the gates move to nodes that grant no
  tool (for example `capacitance` or `segmentation`)?
- **Multi-station bases.** Should the trained AI's growth be fitted to a
  two-station baseline instead? Doing that would need a steeper growth term
  than `pace`'s linear ratio, or a larger final offset.
