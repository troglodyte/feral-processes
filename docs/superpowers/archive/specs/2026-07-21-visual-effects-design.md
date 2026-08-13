# Damage and shield visual effects

## Problem

Damage is invisible. A raid picks a structure, resolves, and announces
itself as one line in the message log — the same log that scrolls with
loot, level-ups, and movement chatter. A structure at 4/30 durability
renders identically to one at full health. The shield network's best
outcome, absorbing a raid completely, produces no state change at all: no
durability drops, so the only trace is the string "Your shield network
fends off a raid without a scratch!" going by.

The result is a base whose condition can only be read by opening menus, and
combat feedback that is entirely textual. The game has audio cues for
battle actions (`SoundEvent`) but nothing visual for damage.

Scope: the macroquad GUI only. The engine gains a renderer-agnostic seam so
the TUI can adopt it later, but no TUI work happens here.

## Design

### Only one of the four effects needs an engine change

Three of the four effects are derivable from state the engine already
exposes:

| Effect | Source |
|---|---|
| Damaged-structure tinting | `EntityView.durability: Option<(u32, u32)>` |
| Battle hit feedback | `BattleView.player_hp` / `wild_hp`, diffed per frame |
| Log-pane flash | `Game::message_log` returning `(MessageKind, String)` |
| **Map hit flashes** | **nothing — needs a new event** |

Map flashes are the exception because the headline case is a raid the
shield network absorbs, which by definition changes no observable state.
Triggering that flash by matching the log's text would couple the renderer
to a human-readable string; a structured event is the honest seam.

### The effect queue

In `resources.rs`, alongside `MessageLog`:

```rust
pub enum EffectKind { Hit, Deflected, Destroyed }
pub struct VisualEffect { pub pos: (i32, i32), pub kind: EffectKind }

#[derive(Resource, Default)]
pub struct EffectQueue { effects: Vec<VisualEffect> }
```

`Game::take_effects() -> Vec<VisualEffect>` drains it, mirroring
`App::take_sounds`. The queue is capped at `EFFECT_QUEUE_CAP` (32, oldest
dropped) the same way `MessageLog` caps at `MESSAGE_LOG_CAP`, so a frontend
that never drains cannot grow it without bound. The TUI drains and discards
in `run_loop`, exactly as it already does for sounds.

The queue is transient. It is not serialized, so `save::SAVE_FORMAT_VERSION`
does not change and old saves load unaffected.

Positions are world coordinates, not screen coordinates. The renderer
translates them per frame, so a flash stays pinned to its tile while the
player moves and simply is not drawn when its tile scrolls off-screen.

### Push sites

All three live in the raid path:

- `damage_structure` pushes `Hit`, or `Destroyed` when durability reaches 0
- `raid_check`'s shield-network branch (`raid_damage == 0`) pushes `Deflected`
- `raid_check`'s worker-fends-off branch (`mitigated == 0`) pushes `Deflected`

`Deflected` covers both no-damage outcomes rather than splitting shield from
worker. They render the same, and a second variant would encode a
distinction nothing consumes.

Raid targets are selected by `With<Durability>`, which does not imply
`Position`. Every raidable structure carries one in practice, but the query
does not guarantee it, so a push whose entity has no `Position` is skipped
rather than defaulted to the origin — a flash on the wrong tile is worse
than no flash.

### One more engine addition

`Game::total_raid_defense` is private. Expose
`Game::raid_defense_active() -> bool` wrapping `total_raid_defense() > 0`,
so the renderer can tell whether the shield network is standing without
reaching into `StructureDb` itself.

### GUI animation state

`render::draw(&mut app)` is stateless, and macroquad has no persistent
renderer object. Animation needs somewhere to live, so a new `gui/fx.rs`
defines:

```rust
pub struct Fx {
    pub enabled: bool,
    tile_flashes: Vec<TileFlash>,   // world pos, kind, start time
    ghost_hp: BattleGhosts,         // lagging bar values
    floats: Vec<FloatingNumber>,
    log_flash_until: f64,
    last_log_line: Option<(MessageKind, String)>,
}
```

`game_loop` owns it next to `volume` — GUI-local state that `App` knows
nothing about, following the precedent volume already set. `render::draw`
takes `&mut Fx`.

All timing uses macroquad's `get_time()`. No wall-clock or RNG dependency
enters the engine, so the test suite gains no flakiness.

### Effect 1 — map hit flashes

In `draw_playing_base`'s tile loop, after the background and glyph are
drawn, overlay a tinted rectangle for any active flash on that tile:

```
alpha = PEAK_ALPHA * (1 - elapsed / duration)
```

| Kind | Color | Duration |
|---|---|---|
| `Hit` | red | 0.25s |
| `Deflected` | cyan | 0.25s |
| `Destroyed` | white | 0.40s |

`PEAK_ALPHA` is 0.55 — visible against the dim tile backgrounds without
hiding the glyph underneath. Expired flashes are retained out of the vector
each frame.

### Effect 2 — battle feedback

`draw_bar` is left as-is; it serves the status panel and does not need the
weight. `draw_bar_ghosted` wraps it: it draws a dim band spanning from the
current value out to a lagging "ghost" value, then delegates to `draw_bar`
for the real bar on top. The ghost eases toward the true value at a fixed
rate per second, so a big hit reads as a visible drain rather than a jump.

Floating numbers spawn when a frame observes an HP decrease: the delta
rises ~24px over 0.6s while fading, drawn red above the wild's bar and
white above the player's.

No screen shake and no low-HP vignette. Both were considered and cut — the
battle screen is dense with text and full-screen motion costs legibility.

### Effect 3 — ambient structure condition

Derived every frame from `EntityView.durability`, holding no state:

- Glyph color scales toward grey by `1 - hp/max`, floored at `MIN_TINT`
  (0.45) so a nearly-destroyed structure stays readable
- Below `CRITICAL_DURABILITY_FRACTION` (0.34) the tile background picks up
  a faint red

When `raid_defense_active()`, structures also get a low-amplitude cyan
outline pulse — alpha oscillating 0.06 to 0.16 on a sine of `get_time()`.
It sits below the existing yellow staffed-structure and magenta spawn-point
outlines in visual weight, so it reads as a field rather than competing
with them.

### Effect 4 — log pane flash

`Fx` remembers the last line returned by `message_log`. When a new final
line arrives carrying `MessageKind::Raid`, the log pane's border color
lerps from red back to `BORDER` over 0.35s. Comparing the last line rather
than tracking a count avoids needing a new engine accessor for total log
length.

### Toggle

`\` toggles `Fx::enabled`, handled in `game_loop` as a `KeyCode` alongside
the `[` / `]` volume keys. GUI-local and not persisted, same as volume.

Backslash rather than a letter: letters reach the game through
`get_char_pressed` into `App::handle_key`, where they would collide with
existing bindings.

When disabled, `take_effects` is still drained so the queue cannot back up,
and every draw path falls through to current behavior.

## Testing

Engine, in `lib.rs`'s existing test module:

- a raid that damages a structure pushes `Hit` at that structure's position
- a raid fully absorbed by the shield network pushes `Deflected`, not `Hit`
- a raid absorbed by a cronjob worker pushes `Deflected`
- a raid that reduces durability to 0 pushes `Destroyed`
- the queue caps at `EFFECT_QUEUE_CAP`, dropping oldest
- `take_effects` drains, leaving the queue empty

The `raid_check` roll is probabilistic, so tests that need a raid to land
follow the existing loop-over-seeds pattern in
`deployed_shields_reduce_raid_damage_to_an_undefended_structure` rather than
depending on a single seed. The `Hit` and `Destroyed` cases call
`damage_structure` directly and need no loop.

GUI: the animation math is extracted as pure functions in `fx.rs` —
`flash_alpha(elapsed, duration)`, `damaged_tint(hp, max)`,
`ghost_step(ghost, current, dt)` — each unit-tested for its endpoints and
clamping. Drawing itself stays untested, consistent with the rest of the
renderer.

`cargo test --workspace` is the gate.

## Files

| File | Change |
|---|---|
| `crates/engine/src/resources.rs` | `EffectKind`, `VisualEffect`, `EffectQueue` |
| `crates/engine/src/lib.rs` | register resource, push sites, `take_effects`, `raid_defense_active`, tests |
| `crates/tui/src/lib.rs` | drain and discard effects |
| `crates/gui/src/fx.rs` | new — `Fx` state, pure helpers, tests |
| `crates/gui/src/render.rs` | thread `&mut Fx`; flashes, ghost bars, floats, tinting, log flash |
| `crates/gui/src/lib.rs` | own `Fx`, `\` toggle |

No `.ron` schema changes, so `assets/structures/README.md` and
`assets/species/README.md` are untouched.
