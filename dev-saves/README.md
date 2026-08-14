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

## A template freezes creature stats, and a species retune does not reach it

A creature's stats are written into the save when it *spawns*, scaled by zone,
distance and Stack depth at that moment. A template is a save, so every
creature in one is a fossil of the `.ron` files as they stood the day it was
captured. Editing `base_hp` changes what spawns from now on and nothing that
already exists.

This has already cost a retune. On 2026-08-02 the Wintermute at `(-1, -8)`
still read **6489 HP** after two successive cuts to its `base_hp`, because
6489 was `1600 x 4` from before either of them — and the second cut was
argued from that number, against a creature no current spawn could produce.
Its stats were hand-edited to match, preserving the original potential roll
so it stays the same individual.

So when a balance change is the thing under test, check whether what you are
looking at is spawned or stored:

- **Spawned fresh, and therefore live** — Stack lair guards
  (`Game::spawn_lair_guard`), corridor encounters, ambient surface spawns,
  nest respawns. All of these read the species file at the moment they appear.
- **Stored, and therefore stale** — anything standing on the map when the
  template opens, and every tamed program in the party.

Grep the `.ron` for the species id before concluding a tuning change did
nothing.

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
| `chains` | The `extraction` world with a complete three-stage production chain standing and staffed, and the player parked beside its Assembly Bay. **Every research node is unlocked**, so the whole build menu and every bench recipe is open without playing the tree; cargo carries deep stacks of Core Fragments, the four bench intermediates, Power Cells and Outlets so a factory can be stood up in one session; and six unposted programs (two each Medic, Leech, Bastion, named `Spare …`) sit on the player's tile for the three base jobs, on top of the nine already on cronjobs. Mining Node `(2,0)` → Refinery `(3,0)` → Assembly Bay `(4,0)`, with Power Conduit `(4,2)` → Winding Node `(4,1)` feeding the bay's second ingredient from below — the bay needs *both* feeders orthogonally adjacent, which is why it sits in the corner. A second Data Cache stands at `(2,2)` because five machines need five programs. Press `c` to collect. The base is a **post-halving** one — its slab is the 9x9 a Home stamps today rather than the 15x15 it was captured at, and no structure sits outside it, so this is the one template where a Heap Pillar visibly moves the edge (three of them, 4 to 7, before a link at `(6, -8)` legitimately refuses the fourth). Re-trimmed on 2026-08-13 for that reason: left as captured it derived a radius of 7 from its own outlying structures and absorbed every Pillar. The player starts at `(4, -1)` with three free neighbours, because a structure is deployed onto one of your four orthogonal tiles and a start hemmed in by machines reads as the Pillar being broken rather than as you needing to walk. `the_chains_template_starts_with_a_chain_that_actually_runs` ticks it 400 times and fails if nothing comes out, so a layout knocked one tile out of alignment is caught here rather than in a session. |
| `rarity-preview` | The `extraction` world with one copy of an Arc Lance at every rare tier — Optimized, Overclocked, Unrolled, Bare-Metal — plus a Bare-Metal Ablative Plating fused to T2, a Honed Arc Lance and an Overclocked Singularity Matrix of Quiet Handshakes — so all four colours, both tier axes, an affix prefix, an affix suffix and the widest name the assets can build are on one inventory screen. **Rare gear cannot practically be playtested without this**: the tiers are rolled on a drop and the whole ladder sums to about 3.5%, so seeing four of them honestly means hours of kills. Open the inventory, then `[E]quip` a row to check the swap picker's columns and the equipped panel. It also still carries `extraction`'s four legacy `fused_gear` rows, which is what `a_pre_rarity_templates_fused_gear_survives_the_load` reads. |
| `extraction` | Zone 3, level 36, Forgiving. Nine tamed programs — four of them (Six-Slot, Five-Slot, Drainer, Four-Slot) carry 4–6 routines each, spanning damage, heal, buff, debuff, drain and field effects. A Compiler stands, so `can_extract_routines()` is true. Built for exercising the extraction and routine-install screens. |
