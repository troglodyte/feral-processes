# Sprites from the defs

**Date:** 2026-09-03
**Status:** approved, not implemented

A species or structure can name a one-cell sprite, and the loader finds it by
reading the directory instead of a list in Rust. Ships with **no art and no
visible change**; that is the acceptance criterion, not a caveat.

## Why now

The sprite seam shipped in 0.13.x as a working pipeline with two images, and
`crates/gui/src/sprites.rs` still holds the proof it was built with:

```rust
const SPRITES: &[&str] = &["player", "anchor"];
```

Its own doc comment says what should replace it — "when sprites become a
`sprite:` field on species and structures, the names come from the asset
files and this list goes away."

Two earlier documents reach the same conclusion independently.
`docs/superpowers/reports/2026-07-27-renderer-graphics-assessment.md` was
kept out of the 2026-08-13 plan deletion because of one live finding:
switching to Bevy "did not by itself put a single sprite on screen." And the
entity-sprite half of that question is small in a way the terrain half is
not — entities do not tile, so there are no seamless edges and no autotile
variants, just 17 species + 30 structures as independent 16x16 images.
(Counted off the asset tree, not remembered — a standing note had 26.)

Nothing between here and there is art. It is one field, one view field and
one loader.

## What was decided

**Convention, with an override.** `sprite: Option<String>` defaulting to the
def's own id. Dropping `assets/sprites/rootkit.png` into the tree is the
whole of giving the Rootkit art — no file edit, which is the moddability
rule's own preferred shape. The field exists for the two cases convention
cannot serve: two defs sharing one image, and a modder who wants a different
filename. A required field was rejected as 47 files restating what `id`
already says.

**The loader scans the directory.** This is the decision that changes the
failure mode, and it changes it in the right direction. Today a name with no
file behind it is a warning several frames late, which is why
`every_sprite_the_loader_asks_for_is_on_disk` had to exist at all — nothing
else distinguishes "the path is wrong" from "no art yet". After the scan the
table holds exactly what is on disk, so **nothing can be missing**: a def
whose id has no image falls back to its glyph, silently, which is the state
all 47 defs are in on the day this lands.

Reading the defs for the list was rejected for two reasons. It couples gui
startup to engine database load order, and the main menu exists before any
`Game` does; and a sprite with no def behind it could then never be loaded,
which forecloses fixtures like the anchor.

**Species and structures together.** One field, one view field, one loader,
two `README.md` updates. Splitting doubles the schema-doc churn and buys
nothing — the change is the same shape on both sides.

**The wizard's five preset icons are out of scope.** Four of them
(`operator`, `weaver`, `spike`, `drifter`) name art that has never existed
and draw their glyph. They are player-facing options, and a player can draw
their own icon now, which is arguably what those rows stood in for. They are
revisited with prebuilt drawings, not here.

## Non-goals

Terrain tiles — still parked, and still on art rather than code: autotiling
bills 16 variants per biome minimum and 47 for a full blob set, across seven
biomes, and `draw_biome` + `draw_tile_edges` already autotile from the
neighbour grid with no asset at all. Per-tier or per-status structure art.
Any sprite for an entity that is not a species or a structure. Drawing any
art. A tool for authoring art — that is the next slice, and this is what
unblocks it.

## The change

**`SpeciesDef` and `StructureDef`** each gain `sprite: Option<String>`,
`#[serde(default)]`, beside `glyph`. Both README schema references document
it in the same change, per the rule that a schema change and its docs land
together.

**`SpeciesDef::sprite_name()` / `StructureDef::sprite_name()`** resolve the
field against the id, so "the name defaults to the id" is written once
rather than at each reader.

**`views::EntityView`** gains `sprite: Option<String>`, resolved in the
engine. The renderer cannot read an asset database and `EntityView`
deliberately carries no species id, so the engine has to do the resolving —
the same reason `PlayerLook::sprite` is a name and not an id.

A `String` per entity per frame, deliberately, and not an interned handle.
Views are rebuilt every frame, so this allocates; entities on screen number
in the dozens, and the principle is no optimization ahead of evidence that it
is needed. If it ever appears in a frame profile, `Arc<str>` is a one-line
change behind the same field.

**`crates/gui/src/sprites.rs`** drops `SPRITES` and reads the directory
instead. The root is already in hand and must not be re-derived: `assets_dir()`
is what `asset_plugin` feeds to `AssetPlugin::file_path`, so the scan reads
`assets_dir().join("sprites")` and bevy keeps loading by the same relative
path it does today. Resolving it any other way would be a second site deciding
a runtime path, which `crates/launcher/src/paths.rs` exists to prevent.
The sprite's name is the file stem. `ImageSampler::nearest()` is unchanged
and stays load-bearing: bevy_egui binds the image's own sampler and bevy's
default is linear.

**`render/base.rs`** asks the view for a sprite name for any entity, not
just the player. The player's three-rung fallback is unchanged and still
outranks this; everything else is two rungs, sprite then glyph.

**The anchor keeps its hardcoded name.** It has no def to carry a field, and
that site already documents why it is named in Rust. The scan still finds
`anchor.png`.

## What does not change

Entity art stays governed by `assets/sprites/README.md`'s near-white rule.
A sprite is tinted by multiplication and so inherits the species' authored
hue, `biome_tint` and the damage dimming for free — which is exactly why the
player's drawn icon had to be carved out as the one untinted sprite. Nothing
here touches that seam, and a species sprite authored in colour will fight
every one of those rules.

`Painter::sprite` is untouched. It still reports whether it drew, and a
caller that gets `false` still draws the glyph, which is what keeps
`assets/sprites/` optional by construction.

## Testing

**The census that matters, in `tests/assets.rs`: every authored `sprite:`
override must resolve to a file that exists.** A convention-named sprite
that is absent is the normal case and must stay silent — but an override is
a human typing a filename, and a typo there is invisible, because it looks
exactly like art nobody has drawn yet. That asymmetry is the whole reason
this test exists, and it is the one thing the scan's "nothing can be
missing" property gives up.

Also:
- A def with no art draws its glyph; a def with art draws the sprite **and
  not the glyph** — the overdraw rule, asserted on the absent glyph as well
  as the mesh.
- The scan survives a missing directory, a non-PNG file, and a malformed
  image, each silently, and each with its own test. Deleting
  `assets/sprites/` must restore the glyph map exactly.
- `the_shipped_sprites_are_one_cell` stays and now guards every file the
  scan finds rather than every name a const lists.
- **`every_sprite_the_loader_asks_for_is_on_disk` is deleted**, not adapted.
  It exists to catch a name in `SPRITES` with no file behind it, and after
  the scan that state is unreachable. Keeping it as a test of the shipped
  directory against itself would assert nothing — the no-cruft rule applies
  to tests as much as to code, and the override census is what replaces the
  guarantee it was actually providing.
- A def's `sprite_name()` falls back to its id, and an override wins.

## Files

**engine** — `species.rs`, `structures.rs` (the field and `sprite_name()`),
`views.rs` (`EntityView::sprite`), `game/inspection.rs` (resolve it),
`tests/assets.rs` (the override census).

**gui** — `sprites.rs` (the scan replaces `SPRITES`), `render/base.rs` (ask
the view for any entity), `tests/sprites.rs`.

**docs** — `assets/species/README.md`, `assets/structures/README.md`,
`assets/sprites/README.md`, and the `docs/seams.md` drawing-seam entry if
the loader's rule is now stated inaccurately there.

No save-format change: nothing here is stored.
