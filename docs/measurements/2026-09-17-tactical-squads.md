# What five folded rootkits are worth against five loose ones

**The claim.** At `swing_share = 0.5` (the shipped `FORMATIONS[0]` value), a
folded squad of five rootkits clears the same mid-grade party about **one
round faster** than five loose rootkits of (near-)equal total power — 7.7
rounds against 8.7 over 50 reps each — and costs the party about 4 points
less HP (94.7% left against 91.0%). Both fights are **walkovers**: 100% win
rate, 0 companions downed, in every rep of both scenarios. The direction
matches the spec's own prediction (fewer large hits lose less to mitigation,
damage lands on one target, no overkill is wasted between members) but the
size of the gap is modest, and win rate carries no signal here at all — this
mid-grade party beats either pack every time, so `swing_share` cannot be
judged unwinnable-vs-winnable from this pair; only the two continuous
figures (rounds, HP left) move.

## How to reproduce it

Built once, both scenarios run from that same binary (arena figures compare
within one build only — CLAUDE.md, `dev-arenas/README.md`):

```sh
cargo build --release --bin arena
./target/release/arena dev-arenas/squad.ron --out /tmp/squad-report.ron
./target/release/arena dev-arenas/squad-control.ron --out /tmp/squad-control-report.ron
```

Both scenarios: `Fresh(level: 20, zone: 3)` player geared with Plasma
Router / Bastion Lattice / Singularity Matrix, three level-12 Scrappers each
carrying an Arc Lance and Hardened Shell — `tactical-full-group.ron`'s own
party, reused rather than invented (CLAUDE.md: the mid-grade party
convention here is Scrappers, not Sentinels). `model: Tactical`,
`approach: Some(East)`, `reps: 50`, `seed: 3` for both, so the pair shares a
seed as the spec asks. `squad.ron` fields `opponents: [(species: "rootkit",
count: 5)]`; `squad-control.ron` fields `[(species: "rootkit", count: 4),
(species: "virus", count: 1)]` — four of a kind is one short of
`FORMATIONS[0].members`, so `squads::plan` cannot fold it, and Virus was
picked for its stats being close to Rootkit's (hp 122/atk 10/mitigation 8
against 132/12/3) rather than a much weaker or stronger species, to keep the
control's total pack power close to the squad's.

Ran on an otherwise-idle machine (16 logical CPUs, load average ~0.9 at the
time) with nothing else CPU-heavy running; a single `arena` run is
single-threaded and fast enough (well under a second for 50 reps) that
contention was not a practical concern here regardless.

Verified before trusting the numbers: `report.warnings` is `[]` for both
files (checked directly in the written `.ron` reports), and the `fought`
line / transcript at `reps: 1` shows the squad fighting as one unlabelled
`Rootkit` (no per-body hex id, one HP pool, "The rogue program crashes and
deletes itself!" printed five times on its death — five separate payouts)
against the control's five individually hex-prefixed bodies (`Rootkit 3`,
`Virus 3`, ...) dying one at a time. That distinction is the whole feature,
so it was checked by eye before reading any aggregate.

## The numbers

Both are new measurements; there is nothing to replicate against.

| | win rate | rounds (mean / median / sd / range) | player HP left | companions downed |
|---|---|---|---|---|
| `squad.ron` (5 rootkits, folded) | 100% (50/50) | 7.72 / 8 / 1.04 / 6-11 | 94.7% (sd 10.0) | 0.00 every rep |
| `squad-control.ron` (4 rootkit + 1 virus, loose) | 100% (50/50) | 8.70 / 9 / 1.33 / 7-14 | 91.0% (sd 10.3) | 0.00 every rep |

The gap: **-0.98 rounds** and **+3.7 points of HP** for the folded squad,
against this party. Both means sit well inside one standard deviation of the
other distribution's spread, so the *direction* is the reading to trust more
than the exact size of the gap — 50 reps each is enough to say the squad is
not slower or costlier, not enough to pin the gap to a decimal.

`squad.ron`'s `opponents` count (5) sits *at*, not past, this fight's own
ceiling: `Game::group_size_ceiling()` folds in `party_count_steps` (three
fielded level-12 companions) on top of the zone's `danger_steps`, so it
reaches 5 here rather than the 4 `full-group.ron`'s naked-player comment
measured for zone 3. Confirmed by the empty `warnings` list rather than
assumed — see `dev-arenas/squad.ron`'s own comment.

## What it does not say

- **Both fights are walkovers.** Nine of fourteen shipped scenarios already
  were (`docs/measurements/2026-08-19-combat-model-slice-1.md`), and this
  pair joins them: a mid-grade three-Scrapper party is not stressed by five
  zone-appropriate rootkits, folded or not, so this measurement cannot say
  whether `swing_share = 0.5` is *survivable* at the edge — only that the
  edge is not visible from here. A pack sized to actually threaten this
  party (more rootkits, or a higher zone) would be needed to read win rate
  as a signal at all, and the spec asked for exactly this pairing, not that
  harder one.
- **The party plays only `AllAttack`-equivalent tactical AI** — every body
  swings and never invokes (`dev-arenas/README.md`'s standing gap). No
  companion Special ever fires, so this is a floor on the party's output in
  both scenarios equally; it cannot bias the *comparison* between them, only
  the absolute numbers.
- **One species pair.** Rootkit was chosen because it appears in the
  existing `full-group.ron`/`tactical-full-group.ron` pair already in this
  directory, and Virus for having comparable stats; a different pack
  species (higher variance in `spread`, a stun-heavy kit, a ranged-only
  species) could change how much a folded squad's "two large hits" edge is
  worth. Not swept here.
- **One seed pair (3, 3+n for reps 0-49), one build.** A `tuning.rs` change
  reshuffles the `GameRng` stream as well as any curve it moves, so this
  report is only valid against the exact commit it was measured on
  (`0b264583`, "Gui: a squad draws over its whole footprint") — a re-run
  after a `swing_share` retune must be re-measured, not diffed against these
  numbers.
- **No sweep of `swing_share` itself.** This file answers "is folding worth
  anything, and in which direction" for the shipped `0.5`; it does not say
  what value the constant should move to, and per this task's own scope that
  retune is left to the human, along with the `balance_sim` re-gate a
  `tuning.rs` edit would need.
