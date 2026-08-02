# dev-saves

Named worlds you can put the game into on demand, for deliberate testing.

A template is a save rendered as RON — exactly what `savetool dump` emits —
checked in so that a state you had to *play* up to becomes a state you can
*generate*. Testing routine extraction shouldn't start with an hour of taming
programs.

```sh
cargo run -- --template extraction   # regenerate the world and play it
cargo run --bin savetool -- template # what's available
```

## Seeing the whole frame

`FERAL_DEV_REVEAL=1` draws the entire Stack frame on both maps — the corner
inset and the `g` screen — instead of only what the party has walked. It
exists because testing a Stack feature otherwise starts by walking a maze
until you happen upon the cell you meant to test.

```sh
FERAL_DEV_REVEAL=1 cargo run -- --template stack
```

**The map only.** The first-person view, the encounter rolls and everything
the party can do are untouched, so a session with it on is the real game with
the lights on. The "% mapped" figure still counts what has actually been
walked rather than what is drawn, and the `g` screen's heading says
`[DEV REVEAL]` so a screenshot taken with it on cannot be mistaken for the
real thing.

Read once at startup (`stack_view.rs::dev_reveal`), so nothing can toggle it
mid-run and the shipped build never asks. Any value but empty or `0` counts
as on.

## The two directions

```sh
# world  ->  playable save (saves/dev_<name>.bin, overwritten every time)
cargo run --bin savetool -- template extraction
cargo run --bin savetool -- template extraction /tmp/somewhere/else.bin

# a save you have  ->  a new template here (then commit it)
cargo run --bin savetool -- capture saves/save_1785450302.bin extraction
```

## Two rules that are load-bearing

**A template is regenerated, never played in place.** `--template` writes a
copy to `saves/dev_<name>.bin` and opens *that*. The game autosaves, so a
session started directly on a `.ron` here would rewrite it, and the fixture
would quietly decay into a record of the last thing anyone did to it. The copy
under `saves/` is expendable; this directory is source. If a session produces
state worth keeping, `capture` it back deliberately.

**Generating overwrites.** `saves/dev_extraction.bin` is replaced on every
generate, progress and all. That is the point — the same starting state every
run. Rename the `.bin` if you want to keep a session.

## Why RON and not `.bin`

Bincode has no field names on disk, so a `SAVE_FORMAT_VERSION` bump
invalidates every `.bin` outright and there is no migration path. RON is
field-named: a new `#[serde(default)]` field on `SaveData` still parses, and
generating stamps the current version on the way out. When a format change
*does* break a template, the fix is to hand-edit the file — which is the whole
reason templates are stored in the editable format.

`every_checked_in_template_still_loads` (`crates/launcher/src/dev_template.rs`)
is the gate that tells you. It generates every template in this directory
through the real `Game::load` and fails if one doesn't survive. Note that it
checks more than "did it load": a creature whose species id has disappeared
from `assets/species/` is *skipped* on load rather than rejected, so a gutted
template would otherwise open fine and just be missing the programs it exists
to provide. The test compares the tamed count across the load to catch that.

## Adding one

`capture` a save, commit the `.ron`, and run `cargo test -p feral-processes`.
Name it after what it is *for* — `extraction`, not `save3`. Keep the set
small; each one is a fixture somebody has to repair after a format change.

## What's here

| template | what it sets up |
|---|---|
| `stack` | The `extraction` world, but standing on frame 3 of a 6-frame stack instead of on the surface. Built for playtesting the Stack layer — Trace's bands and phase 3's cell kinds — without walking to a link and descending three times first. The party lands on that frame's way up, since `stack::generate` always puts `entry` at `(1, 1)`, so there is room to climb as well as descend. |
| `extraction` | Zone 3, level 36, Forgiving. Nine tamed programs — four of them (Six-Slot, Five-Slot, Drainer, Four-Slot) carry 4–6 routines each, spanning damage, heal, buff, debuff, drain and field effects. A Compiler stands, so `can_extract_routines()` is true. Built for exercising the extraction and routine-install screens. |
