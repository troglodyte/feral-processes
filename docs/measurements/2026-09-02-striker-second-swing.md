# What a Striker's second swing is worth

**Date:** 2026-09-02
**Build:** `v0.13.82` (`bd38fbcb`), immediately after combat slice 2 shipped.
**Question:** slice 2 gave a Striker a second ordinary swing from
`EXTRA_ATTACK_LEVEL` (8). Risk 1 of
[`the slice 2 spec`](../superpowers/specs/2026-09-02-combat-model-slice-2-design.md)
says that roughly doubles a Striker's damage and that nothing re-derives the
level cap against it, because `balance_sim` models no class and cannot see the
feature at all. So: how much is it actually worth?

## Why the arena and not `balance_sim`

`balance_sim::simulate_roster_fight` takes `Stats` blocks and models no class
or level in its inner loop — the blind spot is recorded in that function. It
therefore reports **no change at all** for this feature, and its five curve
tests passing after slice 2 is evidence the instrument cannot see it, not
evidence the feature is balanced.

The arena can see it, and this is the first time it could see a class at all.
Before slice 2 a class was a spread of multipliers over *authored routine
power*, `PartyPlan::AllAttack` invokes no routine, and an ordinary swing never
touched `Game::ability_affinity` — so `dev-arenas/README.md` and all seven
`player-class-*.ron` files stated the headless bin could not see a class. A
second **ordinary** swing is exactly what All-Attack throws, which is what
made this measurable and what falsified that claim. All eight files were
corrected in the same change.

## What was run

Two scenario files identical in every field but `class`, in a scratch
directory, then:

```sh
cargo run --bin arena -- swing-Striker.ron
cargo run --bin arena -- swing-Medic.ron
```

```ron
(
    player: Fresh(level: 8, zone: 2),
    character: (
        class: Some(Striker),        // the only field that differs; Medic in the control
        stats: (0, 0, 0, 0),
        routine: Some("stack_smash"),
    ),
    party: [
        (species: "glitch", level: 5),
        (species: "glitch", level: 5),
    ],
    opponents: [
        (species: "sub_process", count: 5),
        (species: "sub_process", count: 5),
    ],
    reps: 20,
    seed: 1,
)
```

Level 8 to clear the threshold; zone 2 because `zone_level_cap` floors zone 1
at 6 and a level-8 player there would be asking for a level the cap refuses.
The stat pool is left unspent so the pool — which the bin *has* always seen —
cannot contribute to the difference.

## The numbers

| Class | Win rate | Rounds | Player HP left | Companions down |
| --- | --- | --- | --- | --- |
| **Striker** | **60%** (12/20) | mean 19.2, median 19 | 26% | 1.25 |
| **Medic** (control) | **0%** (0/20) | mean 15.6, median 14 | 0% | 1.35 |

Striker loss seeds: 1 2 6 9 14 15 16 19. Medic lost all twenty.

**The whole gap is the second swing.** Nothing else in the two files differs,
and no other class effect reaches an ordinary swing: `capture_boost_pct`,
`routine_slot_bonus` and `work_tick_scale` are the other three named class
queries and none touches combat damage.

## What this run was blind to

- **One pack, one level, one zone.** This says what the second swing is worth
  in *this* fight. It does not say where the crossover is, what it is worth at
  the level cap, or what it does to a Stack lair.
- **20 reps has known noise in this repo** — see the tuner-target work. 0/20
  against 12/20 is far outside it, but a 10-point win-rate difference from
  this rig would not be.
- **The pack was trimmed.** Both runs printed `sub_process x5 asked for; zone
  2 would field at most 2`, twice. Both were trimmed identically so the
  comparison holds, but the fight is not the ten-body one the file describes.
- **Compare within this build only.** A moved baseline in a later report is a
  reshuffled RNG stream, not a difficulty change.
- **Nobody has played it.** This is a headless All-Attack projection. It says
  nothing about whether a two-swing round *reads* as a slog, which is the
  other half of Risk 1 and is not answerable by any instrument here.

## What a decision would rest on

If a 0%→60% swing in one fight is representative, `ZONE_LEVEL_CAP_STEP = 11`
is fitted against a per-round throughput that a Striker above level 8 no
longer has, and the cap's lower bound — which CLAUDE.md calls a correctness
bound rather than a difficulty knob — needs re-deriving rather than re-running.

The cheap dial if it plays too strong is making the second swing a **reduced**
roll rather than a full one; the spec chose a full roll for simplicity and
because `expected_damage` then needs no change.
