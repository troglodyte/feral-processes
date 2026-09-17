# Floor finishes

One `.ron` file per finish: a decorative shade, and optionally a carved
sprite, that the dig crew paints onto floor already laid in the base. It is
purely cosmetic unless `comfort` is set, in which case a program standing on
it can come away fonder of the place.

**A finish only ever sits on laid `Floor`.** It never affects walkability,
never replaces the tile underneath, and never counts toward anything a
structure or a haul route reads — see `crates/engine/src/base_grid.rs`.

Drop a file in, restart, and it is in the world. Nothing else to edit.

## Schema

```ron
(
    id: "cobalt_carpet",
    name: "Cobalt Carpet",
    description: "A deep blue weave, laid over the tile.",
    shade: Cobalt,
    sprite: None,
    comfort: Some("at_ease_on"),
)
```

| Field | Type | Meaning |
|---|---|---|
| `id` | string | Save key and, by convention, the sprite key — see `sprite` below. |
| `name` | string | The excavate brush's label and the examine line. |
| `description` | string | One line of flavour. |
| `shade` | one of the ten below | A named tint, resolved to a colour in the renderer. There is no free RGB field — see "Why shade is a fixed set". |
| `sprite` | `Option<string>`, optional | Sprite key override — falls back to `id`, and an `@`-prefixed override is ignored, exactly like a species' or a structure's `sprite:` field (`assets/sprites/README.md`). |
| `comfort` | `Option<string>`, optional | The id of a def in `assets/memories/` that a program standing on this finish writes. `None` (the default) means the finish is purely cosmetic. |

There is no `cost` field: every finish costs
`crate::tuning::FLOOR_FINISH_COST` Blank Substrate, a fixed tuning constant,
on top of the one the tile itself already spent to be floor at all.

## The ten shades

`Cobalt`, `Teal`, `Moss`, `Olive`, `Ochre`, `Umber`, `Wine`, `Plum`,
`Violet`, `Slate`. A shade name the enum does not have fails that file's
parse — the standard skip-and-warn, the same as any other malformed file.

## Why `shade` is a fixed set, not an RGB triple

The map's one colour rule elsewhere is that hue answers "can I walk here" —
cool is walkable, hot is blocked. A finish only ever sits on floor already
laid, so it is admitted to warm hues too, but a free RGB field would let a
mod paint a carpet that reads as an exposed rock face or a raid's threat
wash. Keeping the set closed and resolving it in the renderer
(`crates/gui/src/render/terrain.rs::shade_color`) is what lets a census hold
every shade to a minimum distance from everything it must not be mistaken
for.

## Sprites are optional, and the fill always draws

A finish is a pattern **over** the floor, not a substitute for one: the
shade fill and its edge line draw regardless, and the sprite — if the
finish names one and `assets/sprites/` has it — is drawn as a tint over
that fill. A missing sprite still reads as the finish; see
`assets/sprites/README.md` for the format (16x16, near-white, PNG) that
lets the renderer tint it by the shade.

## `comfort` and the memory it writes

A finish with `comfort: Some(id)` must name a def in `assets/memories/`
whose `subject` is `BaseTile` — a program standing on it, ticked to the
posting period, writes that memory about the tile it is standing on. The
finish carries no number of its own: the memory def's own `valence` and
`strike_cap` decide how strong the lift is and how far it compounds.
`comfort: None` (the default, and `slate_inlay`'s choice) means the finish
is decoration only.

## Failure modes

A malformed file, or one naming a shade the enum does not have, is skipped
with a logged warning — the rest of the directory still loads.

**An absent directory is a supported install**: no brush is offered beyond
plain, no finish ever appears, and the game is exactly what it was before
this feature — the same supported way deleting `assets/settlements/` or
`assets/memories/` restores an earlier game.

## What is here

- `cobalt_carpet.ron` / `moss_weave.ron` — comfortable finishes, both
  naming `at_ease_on`.
- `slate_inlay.ron` — a purely decorative finish, no `comfort`.
