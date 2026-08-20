# 2026-08-19 — What an unoptimised dependency graph costs a frame

## The claim

`cargo run` — the documented and actual way this game is started — was
producing **under 20 fps**, and had been degrading for months without any
single change looking guilty. The cause was `Cargo.toml` carrying no
`[profile.dev]` section at all, so bevy, wgpu and egui compiled entirely
unoptimised into the playable build.

The renderer's own shape-building pass, timed alone (no bevy, no wgpu, no
egui tessellation, no GPU), took **51.4 ms a frame** in debug against
**2.0 ms** in release — a 25x gap, all of it in dependency code. Setting
dependencies to `opt-level = 3` and this workspace's four crates to
`opt-level = 1` brought debug to **2.3 ms**, at the price of one cold
rebuild of the 557-crate graph.

The symptom this was found from was a play report: *the smooth map
movement is gone, it's jerky now.* The camera code was intact. At 19 fps
a 12-per-second exponential glide gets roughly two frames per 90 ms key
repeat, so the map arrives in two visible chunks per step. **The animation
had not been lost; there were no frames to draw it in.**

The engine test suite fell from 38.6 s to 6.7 s in the same change, which
retires the standing claim in `CLAUDE.md` that ~24 s was an unavoidable
debug-build artifact of RON save/load.

## How to reproduce it

The frame cost was measured with a scratch test in `crates/gui/src/lib.rs`,
deleted after the run rather than kept — it is thirty lines and its value
was the number, not the coverage. It loads `dev-saves/chains.ron` (a
developed base: machines, staff, work orders, the busiest map the game
draws), then times twenty `render::draw` calls through `paint::with_painter`
and subtracts a twenty-call baseline of `with_painter(|_| ())`, since that
helper builds a fresh egui context and installs fonts on every call.

```rust
let (_, shapes) = paint::with_painter(|p| render::draw(&mut app, &mut fx, p));
// ...twenty more, timed, minus a twenty-call empty-closure baseline
```

Suite times are `time cargo test --workspace`, warm, reading the per-binary
`test result` lines rather than the wall clock, so compilation is excluded.
Rebuild times are `touch` on one file then `time cargo build` /
`cargo test --no-run`, taking the *second* run — the first under any new
profile has no incremental cache and reads 30x too slow.

Machine: the development box, 16 threads. Absolute milliseconds will differ
elsewhere; the **ratios** are the finding.

## The numbers

Renderer shape-building pass, `dev-saves/chains.ron`, per frame:

| build | draw-only | shapes |
|---|---|---|
| debug, no profile section (before) | **51.4 ms** | 1703 |
| debug, deps at 3, our crates at 0 | 3.7 ms | 1703 |
| debug, deps at 3, our crates at 1 | **2.3 ms** | 1703 |
| release | 2.0 ms | 1703 |

The shape count does not move, which is what identifies this as a
code-generation cost rather than the renderer emitting more work. On a
fresh zone-1 game with no base the debug figure was 40 ms against 1664
shapes — a developed base is only ~2% more geometry, so **the map's cost is
the terrain pass, not the base on it**.

Test suite, warm, debug:

| suite | before | our crates at 0 | our crates at 1 |
|---|---|---|---|
| engine (2071 tests) | 38.6 s | 13.6 s | **6.7 s** |
| app-core (349) | 10.2 s | 3.6 s | **1.7 s** |
| gui (284) | 1.0 s | 0.4 s | **0.1 s** |

Rebuild cost, the thing being traded away:

| action | after |
|---|---|
| cold build of the whole graph | 3 m 28 s wall (37 m CPU), once |
| `cargo check --workspace`, no change | 1.8 s |
| touch one engine file, `cargo test --no-run` | 2.5 s |
| touch one gui file, `cargo build` | 2.0 s |

The warm loop is unchanged in practice, which is what makes `opt-level = 1`
on our own crates worth its compile cost here: it halves the suite again,
and the suite is the gate that runs constantly.

## What it does not say

- **It is not a frame time.** 51.4 ms is the shape-building pass alone. The
  real frame also carries bevy's schedule, egui's tessellator, buffer upload
  and the GPU. The true debug frame was *worse* than 51 ms, never better —
  so "under 20 fps" is a ceiling, not an estimate.
- **It does not confirm the fix on screen.** The jerkiness was reported from
  play and the fix is measured on the bench. Nobody has watched the map
  glide since.
- **It says nothing about whether 1703 shapes is a reasonable number.** No
  budget for the vector-terrain pass was ever set, and none is set here. If
  the map grows another layer, this measurement is the baseline to compare
  against, not a licence.
- **The ratios are from one machine.** A slower box was never in the loop, so
  how far under 60 fps other hardware sat is unknown.
- **`opt-level = 1` on our own crates is a judgement, not a measured
  requirement.** Dependencies alone reach 3.7 ms, which is already ample.
  The extra level was taken for the suite time; if it ever obstructs a
  debugger session, dropping it costs 1.4 ms a frame and nothing else.
