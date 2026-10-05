# Battle effects library — design

**Status:** spec, awaiting review.

## Intent

Tactical battles should show *what* hit, not just *that* something hit:
laser-like pulses to a target, explosions, zaps, slashes, a sustained beam,
muzzle flashes, smoke, and screen shake. Which look plays is chosen by the
weapon or routine, from a moddable **library of named effects**. Shake is a
generic field any effect can set; explosions set it.

What the user said: a library; weapons and routines select from it; best-guess
assignments for existing content; shake on explosions plus a generic field
`.ron` files can reference. Loaded in gui, like sprites.

## Today

`crates/gui/src/fx.rs` draws, on a battle map: a `BoltCue` streak (one line,
attacker's glyph colour, `BOLT_SECONDS` = a third of a turn beat), a hit wash
plus spark burst and floating `-N` from `TacticalFxQueue`, heal/reaction marks,
landings, a round banner. No shake. **A routine cast on a battle map emits no
travel or area cue** — a blast over five cells reads as five unrelated hits.

## The library — `assets/effects/*.ron`

One file per effect, loaded by gui at startup (sprites' precedent: purely
presentational, the engine never reads it). Schema:

```ron
(
    id: "laser_pulse",
    color: None,              // Option<GlyphColor>; None = the attacker's glyph colour
    muzzle: false,            // brief flash on the attacker's cell(s)
    travel: Pulses(count: 3), // Streak | Pulses(count: u8) | Beam(hold: f32) | Zap | None
    impact: [Sparks],         // zero or more: Sparks | Explosion(radius: f32) | Zap | Slash | Smoke
    shake: 0.0,               // screen shake strength, 0 = none
)
```

- Every field but `id` is `#[serde(default)]`; defaults reproduce today's
  streak (`color: None`, `muzzle: false`, `travel: Streak`, `impact: [Sparks]`,
  `shake: 0.0`).
- A malformed file is skipped with a logged warning (`*Db::load_dir` pattern);
  a missing directory leaves only the built-in `streak` default.
- `assets/effects/README.md` documents the schema and each primitive.

### Primitives

| Primitive | Look | Lifetime |
|---|---|---|
| `Streak` | today's line, head + fading tail | `BOLT_SECONDS` |
| `Pulses(n)` | n short dashes fired in sequence along the line | all n inside `BOLT_SECONDS` |
| `Beam(hold)` | full-length line, held, then fades | inside `BOLT_SECONDS`; `hold` is a fraction of it |
| `Zap` (travel) | jagged polyline, re-jittered a few times | inside `BOLT_SECONDS` |
| `Sparks` | today's `EffectKind::Hit` burst | today's |
| `Explosion(r)` | expanding ring + heavier burst, on every covered cell | ~`HIT_FLASH_SECONDS`-scale |
| `Zap` (impact) | short crackle lines on the target cell | short |
| `Slash` | arc swipe across the target cell | short |
| `Smoke` | grey puff that drifts up and fades | longest, ~0.6 s |

**Timing rule (load-bearing):** every *travel* primitive finishes inside
`BOLT_SECONDS`, so it cannot still be in flight when the next body acts (the
existing reason `BOLT_SECONDS` derives from `TACTICAL_TURNS_PER_SECOND`).
Impacts start at arrival and may linger, as the hit wash already does.

Jitter (zap, smoke drift) uses `spark_scatter`-style stable hashes of cell and
index — no RNG, no per-frame re-roll, deterministic in tests.

Pulses are cosmetic: one attack is still one damage roll and one `-N`.

## Content selects — the `fx` field

- `ItemDef` and `AbilityDef` gain `#[serde(default)] fx: Option<String>`.
  Absent = `"streak"`. Existing and modded files keep parsing unchanged.
  `assets/items/README.md` and `assets/abilities/README.md` updated.
- A swing uses the wielded weapon's `fx`; unarmed uses `"streak"`. Reaction
  swings follow the same rule. A routine uses its own `fx`.
- An unknown id at draw time falls back to `"streak"` (with a one-time
  warning), never a panic. A census test (below) makes that fallback
  unreachable for shipped content.

## Engine changes

1. **`BoltCue` gains `fx: Option<String>`**, filled at the three push sites
   (`tactical/turn.rs` swing and reaction, `game/tamper.rs`). The engine
   passes it through and never interprets it.
2. **Routines on a battle map emit a cue.** A new `RoutineCue { from, aim,
   cells: Vec<(i32,i32)>, color, fx }` queued in `BoltQueue` (or a sibling
   queue if the per-`Resource` iteration-order trap argues against widening —
   decided in the plan by checking `new-resource-shifts-query-iteration-order`).
   Pushed in `run_tactical_routine` **before** effects resolve, matching the
   swing's "pushed before the blow lands" rule. Travel is drawn from `from`
   to `aim`; area impacts (Explosion, Smoke) draw on every cell of `cells`;
   single-cell impacts draw at `aim`. Self/ally-only routines with no travel
   still get their impacts.
3. Not serialized (the queues' existing rule). No save-format change.

**Seam update:** `seams-tactical.md`'s "a `BoltCue` … with no `kind` field"
rule changes to: the cue names a library effect by id and the engine never
reads it; the travel-timing rule still holds per primitive. Update all three
places (graph, `seams` skill, rules file).

## Screen shake

- `Fx` holds a shake *energy* that each effect's `shake` adds to at impact
  time (additive, so overlapping explosions stack rather than reset), capped,
  decaying exponentially over ~0.3 s.
- The offset is a stable-hash wobble scaled by energy, added to
  `Fx::camera_offset` on the battle map only, bounded to a fraction of a tile
  so the turn arrow and aim cursor (bounds-checked, no lag clamp) stay
  readable.
- `Fx::enabled == false` → no shake, like every other effect.

## Starter library and best-guess assignment

Files: `streak`, `laser_pulse` (`Pulses(3)`, muzzle), `beam` (`Beam`, muzzle),
`zap` (`Zap` travel + `Zap` impact), `slash` (no travel, `Slash`),
`explosion` (`Streak` travel, `Explosion` + `Smoke` impact, `shake: 1.0`).

Assignment, by inspecting each file during implementation:
- weapons with `range > 1` → `laser_pulse`; melee weapons → `slash`;
- routines with a `Radius` shape → `explosion`; routines whose effect is a
  status rider / debuff → `zap`; heavy single-target `Damage` → `beam`;
  buffs, heals, utility → no `fx` (impact-only default, or none).

The plan records the resulting table so the user can re-assign by hand.

## Testing

- Unit tests for each primitive's timing/shape function in `fx.rs` (pulse
  phases all inside `BOLT_SECONDS`; beam hold inside it; explosion ring radius
  monotone; shake decays to zero and stacks additively; zap jitter stable
  across frames).
- `a_disabled_fx_draws_no_bolt`'s pattern extended: disabled fx draws no
  pulses/explosion and produces zero shake offset.
- Loader: a malformed effect file is skipped; a missing directory yields the
  built-in `streak`; serde defaults reproduce today's streak.
- **Census:** every `fx:` named in `assets/items/` and `assets/abilities/`
  exists in `assets/effects/` (gui crate test reading the real assets).
- Engine: a tactical routine cast queues one `RoutineCue` with the shape's
  cells; a swing's `BoltCue` carries the weapon's `fx`.
- Screenshots via `--template … --screenshot` of a ranged fight and a blast
  routine, read back.
- `cargo test --workspace`, clippy `--all-targets`, `balance_sim` unaffected
  (no tuning change).

## Out of scope

Sound per effect; per-species effects; effects outside battle maps (raids,
base space); a shake setting separate from the effects toggle.
