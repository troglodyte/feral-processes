# 2026-10-07 — What the running game costs: CPU, GPU and memory

## The claim

This is the first whole-process figure. The 2026-08-19 entry timed only the
renderer's shape-building pass. In the debug build, `cargo run`, the
game uses **about a quarter of one core** on a developed base map. It holds
**about 450 MB of RAM and 580 MB of GPU memory** and keeps the GPU at
**3-4%**. None of those is a problem on the development box.

The one real cost it found was the **main menu, at 59% of a core**, more
than twice the busy map. `App::list_saves` parsed every save file in full
on every frame, and both the main menu (only to decide whether to show
`[L] Load Game`) and the load list call it every frame. With the four saves
on the box (~1 MB) that was the whole difference. The menu used **9.5%**
with an empty saves directory. Caching the parsed entries per file, keyed on
size and modification time, brought it to **9.8%** with the saves present.
The cost had scaled with however many saves a player kept.

## How to reproduce it

A scratch script, run from the repo root after `cargo build`. It starts the
binary, waits 8 s for startup and asset loading, then samples for 10 s.
CPU is from `/proc/<pid>/stat` (utime + stime over wall time), memory from
`/proc/<pid>/status`, and GPU from `nvidia-smi` every 500 ms.

```sh
./target/debug/feral-processes [--template chains] >log 2>&1 &
pid=$!; sleep 8
t0=$(awk '{print $14+$15}' /proc/$pid/stat); c0=$(date +%s.%N)
nvidia-smi --query-gpu=utilization.gpu,memory.used,power.draw \
  --format=csv,noheader -lms 500 > gpu.csv & n=$!
sleep 10
t1=$(awk '{print $14+$15}' /proc/$pid/stat); c1=$(date +%s.%N); kill $n
echo "($t1-$t0)/100/($c1-$c0)*100" | bc -l      # % of one core
grep -E 'VmRSS|VmHWM' /proc/$pid/status
nvidia-smi --query-compute-apps=pid,used_memory --format=csv,noheader
kill $pid
```

The no-saves run sets `XDG_DATA_HOME` to an empty directory, which
`crates/launcher/src/paths.rs` resolves the saves directory under via
`dirs::data_dir()`. That leaves real saves untouched. `perf record` was
not available: `perf_event_paranoid` is 4 on the box.

Machine: the development box, 16 threads, RTX 5070 Ti on Vulkan, NVIDIA
driver 595.91.07, Wayland. Before launch the GPU idled at 0% and 16 W.

## The numbers

All new: no whole-process figure existed before.

| scene | CPU (% of one core) | RSS | GPU memory | GPU use / power |
|---|---|---|---|---|
| `--template chains` (developed base map) | 23.8% | 448 MB | 579 MB | 4%, ~22 W |
| main menu, 4 saves (~1 MB), before the fix | 58.6% (58.7% on a rerun) | 400 MB | 579 MB | 3%, ~19 W |
| main menu, empty saves directory | 9.5% | — | — | — |
| main menu, 4 saves, after the fix | 9.8% | 400 MB | — | — |

The fix is the `list_saves` cache on `App` (`crates/app-core/src/app/lifecycle.rs`),
pinned by `list_saves_parses_a_file_again_only_once_it_changes`.

## What it does not say

- **No frame rate and no frame-time spikes.** CPU share says how busy the
  process is, not how smooth it looks. The F3 overlay (`crates/gui/src/perf.rs`)
  reports fps, mean, peak and draw time. It was not read for this entry.
- **One machine, and a strong one.** 4% of an RTX 5070 Ti says nothing about
  integrated graphics. The same box has an AMD iGPU that was not tried.
- **Ten seconds per scene.** Memory growth over a long session is unmeasured.
  `VmHWM` equalled `VmRSS` at the sample, so it was still at its peak, or
  still climbing.
- **Debug build only.** Release was not run. The 2026-08-19 entry puts the
  draw pass within 15% of release under the current `[profile.dev]`.
- **Battle and the Stack were not sampled.** The map figure is `chains`,
  the busiest base. A large battle or a revealed Stack frame may cost more.

## Open questions

- **Does the game need to redraw when nothing changes?** No
  `WinitSettings`/`UpdateMode` is set, so bevy redraws continuously. That is
  ~10% of a core on a static menu and ~24% on a static map. A reactive
  update mode would cut idle cost, but the glide, effects and toasts would
  need to request redraws.
- **Is ~450 MB RSS and ~580 MB of GPU memory expected for this game?** Most
  of it is probably bevy, wgpu, egui and font and sprite atlases, but nothing
  has broken it down.
- **What do fps and peak frame time look like on the map, in battle and in
  the Stack, with the F3 overlay on?** Read in both debug and release.
- **Does memory grow over a multi-hour session?** Sample RSS every minute
  through a long play or `--keys` run.
- **How does it run on the AMD iGPU?** Force that adapter, for example with
  `WGPU_ADAPTER_NAME`, and repeat the table.
