# Battle effects (mods)

One `.ron` file per named effect. Weapons (`assets/items/`) and routines
(`assets/abilities/`) pick one with their `fx: Some("<id>")` field; absent
means `"streak"`. Purely presentational: only the renderer reads this
directory and the engine never does. A malformed file is skipped with a
logged warning; a missing directory leaves only the built-in `streak`; an
unknown id at draw time falls back to `streak` with a one-time warning.

## Schema

```ron
(
    id: "laser_pulse",        // required, unique
    color: None,              // Option<GlyphColor>; None = the attacker's glyph colour
    muzzle: false,            // brief flash on the attacker's cell(s)
    travel: Pulses(count: 3), // Streak | Pulses(count: u8) | Beam(hold: f32) | Zap | Ball | None
    impact: [Sparks],         // zero or more: Sparks | Explosion(radius: f32) | Zap | Slash | Smoke | Flash
    shake: 0.0,               // screen shake strength, 0 = none
)
```

Every field but `id` may be left out; the defaults are `color: None`,
`muzzle: false`, `travel: Streak`, `impact: [Sparks]`, `shake: 0.0`, which is
the original single-line bolt.

## Travel (from attacker to target)

Every travel finishes inside one bolt duration, so it is never still in
flight when the next body acts.

| Value | Look |
|---|---|
| `Streak` | one line with a bright head and a fading tail |
| `Pulses(count: n)` | `n` short dashes fired in sequence along the line |
| `Beam(hold: f)` | the full-length line, held for fraction `f` of the duration, then fading |
| `Zap` | a jagged polyline, re-jittered a few times |
| `Ball` | a small filled disc that flies from attacker to target and lands inside the duration; takes the effect's `color` |
| `None` | nothing travels; impacts only |

## Impact (at the target, or on every covered cell for an area routine)

| Value | Look |
|---|---|
| `Sparks` | the blow's own hit burst, drawn only when damage lands; listing it adds nothing over `impact: []` |
| `Explosion(radius: r)` | an expanding ring and a heavier burst |
| `Zap` | short crackle lines |
| `Slash` | an arc swipe across the cell |
| `Smoke` | a grey puff that drifts up and fades |
| `Flash` | the whole tile tinted in the effect's `color`, fading out; drawn on every covered cell of an area |

`shake` adds to the screen's shake energy at impact; overlapping effects add.

## Shipped

`streak`, `laser_pulse`, `beam`, `zap`, `slash`, `explosion`, `heal` (green), `buff` (cyan). The last two are for ally-facing routines.
