# analysis

Reading battle telemetry. Python, because the question here is "what does
this distribution look like" and that is what pandas and matplotlib are for.

The training itself stays in Rust. The objective function is the real game
(`arena::run`), so an optimiser living over here would have to call back into
Rust for every one of its 1.9M fitness evaluations — and `crates/launcher/
src/cem.rs` already works and is tested. This directory reads what the
trainer wrote; it does not replace it.

```sh
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt
.venv/bin/python -m pytest            # the filename parser
.venv/bin/python policy_report.py --log-dir ../dev-logs/policy-sweep
```

## Where the data comes from

`train --log-dir <dir> --label <name>` records its two **evaluation** passes
— the all-zero baseline and the trained result — and writes one file per
`(label, pass, scenario)`:

```sh
cargo run --release --bin train -- \
    --out dev-logs/policy-sweep/pin3.ron \
    --scenarios dev-training --iters 30 --pop 40 --reps 200 --seed 1 \
    --log-dir dev-logs/policy-sweep --label pin3 \
    --pin target_is_player,target_bracing,target_def_rel
```

The 1.9M-fight search between those two passes is **not** logged, and that
is deliberate: it is candidates that were discarded, and recording it would
cost tens of gigabytes to describe weight vectors nothing ever used.

The three labels live in the filename rather than in the records, which is
why `battles.parse_name` exists and why it searches for the pass token
instead of splitting on hyphens — both the label and the scenario may
contain them. The line schema itself is `dev-logs/README.md`.

## What the report answers

- **Who gets hit** — the share of enemy swings aimed at the player rather
  than a companion. Run 1 of the 2026-08-09 training was rejected for
  driving this to 99.8%, and this is that number measured rather than
  recalled. Solo scenarios are excluded, since a 100% share there is
  arithmetic.
- **Focus fire** — the target's HP fraction at the moment it was chosen,
  as a distribution. This is what `target_hp_frac: -10.86` looks like from
  the outside, and the histogram is the point: two policies can share a mean
  while one spreads its swings and the other alternates finishing blows with
  opening ones.
- **Move variety** — the share of each species' swings taken by its single
  most-used move. A stand-in for "does it still use its kit", deliberately
  not a join against `assets/species/*.ron`: a RON reader here would be a
  second parser to keep in step with the engine's, which is the call
  `docs/roster-gen.py` already made for the same reason.
- **Outcomes** — the per-scenario table the training report carries,
  regenerated from the fights rather than transcribed.

## What it cannot answer

**Defend, unless the sweep asked for it.** By default `arena::run_rep`
plays the party as All-Attack, so nobody braces and `target_bracing` is
`False` on every swing. Pass `--party-plan brace` to `train` and it stops
being false; `check_bracing` reports the count either way rather than
leaving it to be assumed. Any sweep recorded before 2026-08-10 predates the
option and is blind to Defend by construction.

**Companion Specials.** Still unexercised by any plan — the scripted party
braces and does nothing else.
