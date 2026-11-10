---
paths:
  - "crates/launcher/**"
  - "dev-saves/**"
  - "dev-arenas/**"
  - "docs/measurements/**"
  - "arena.sh"
  - "saves-roundtrip.sh"
---

# Dev tools

```sh
cargo run --bin savetool -- dump saves/save.bin s.ron   # edit a save; `pack` reverses
cargo run --bin savetool -- warp saves/save.bin 6       # runs the real breach
cargo run -- --template extraction                      # start from dev-saves/ (README lists them)
cargo run --bin savetool -- capture saves/save.bin <name>
cargo run --bin arena -- dev-arenas/opening-fight.ron [--out report.ron]
FERAL_DEV_ARENA=1 cargo run      # arena in the real battle UI; main menu [R]
cargo run -- --template stack --keys "Right Right" --screenshot out.png  # needs DISPLAY; Read the PNG
FERAL_DEV_REVEAL=1 cargo run -- --template stack   # whole Stack frame on the map
FERAL_DEV_NO_SIEGES=1 cargo run                      # the siege clock never runs
```

- Unset `FERAL_DEV_NO_SIEGES` before `cargo test`: exported, it fails siege
  tests in the engine and launcher.
- `docs/measurements/` is what the instruments already said — read it before
  a sweep or arena batch; its `README.md` is the bar for adding one. Balance
  curves are excluded (`balance_sim.rs`'s job).
- Scripts that call a built binary go through `${CARGO_TARGET_DIR:-target}`:
  Claude sessions build into a shared target dir (`.claude/settings.json`).
