# Rock kinds, a swing floor, and a mining toggle

**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header. See `../../INDEX.md`.

Base-space rock is one flat number today. `tuning::BASE_ROCK_DURABILITY` is
24 for every cell in every zone at every depth, and `Game::swing_damage` is
the band's mean plus `effective_atk`, so a level-1 player opens a wall in
three swings and a developed one opens it in one. That was written down as
intentional — `tuning.rs`'s own doc calls the one-shot "the reward for
levelling" — and this spec reverses it, on the grounds that a wall which
falls to an accidental keypress is not terrain.

Three changes, one feature: rock gets **kinds** (data, so ores later are a
file drop), a swing can never take more than a fraction of a wall so
levelling can speed digging without trivialising it, and the player's bump
stops mining unless they have asked it to.

## Why

**Walking is not mining.** `move_in_base` turns a step into solid rock into
a swing. In slice 1 that step was refused for free; slice 2 made cutting the
point and turned the refusal into an attack. The consequence nobody costed
is that navigating your own base at a developed level *demolishes it* — you
walk a corridor, clip a corner, and the corner is gone. The excavation plan
(`m`) already exists for deliberate digging by the crew, so the player's
bump is the only gesture in the game that destroys terrain without being
asked twice.

**A wall that falls in one hit is not a cost.** Everything else in the base
slice is drawn against digging being expensive: `BASE_ENTROPY_REFILL_TICKS`
puts a clock on the frontier precisely because "cutting is cheap and
flooring is not". If cutting is *free*, the clock is measuring nothing and
the crew's whole `TaskKind::Excavate` cycle is something you would never
leave running, because doing it yourself is faster by an order of magnitude.

**There is nowhere for an ore to live.** The TODO note asks for hardness
"based on wall type, we might introduce ores later". Base space has no wall
type at all — `BaseGrid` is a sparse map in which absent *means* solid, so
there is not so much as a struct field to hang one on. Whatever shape that
takes has to be decided before an ore can exist, and it is most of the work
here.

## What the player does

Mining is **off** when a run starts. Walking into rock is refused for free,
with a sentence saying why, and costs no turn.

Press `n` to arm it. The map's status column says so. Now a step into rock
is a swing, exactly as it is today, and pressing `n` again disarms it.

Rock is no longer uniform. A wall face the player can *see* — one with air
against it — is drawn in its own colour, so an exposed seam of something
denser reads as something to go at rather than as more black. Rock deeper
than that face is unknown, and stays unknown until it is cut into. Examining
a wall names its kind on exactly the same terms.

A denser wall takes more swings, and no wall of any kind ever falls to one.

**The crew is untouched by all of this.** A marked cell is dug by whoever is
posted to it whether the player's own mining is armed or not, and it is dug
at the same per-swing floor the player meets.

## The shape of the decision

Six settled choices, so they are not relitigated.

### A wall's kind is derived from its coordinate, never stored

`base_grid.rs` opens by explaining that there is no `Solid` variant, because
"not in the map" already says it and storing one for every untouched
coordinate would make the sparse map not sparse. That argument survives this
feature intact, so the kind cannot be a field on a cell.

Two alternatives were considered and refused. **Storing it** — a
`BaseCell::Solid { kind }`, or a parallel map — either de-sparsifies base
space (a kind written out for every coordinate the player might one day
reach) or writes lazily on first touch, which makes the save grow as the
player wanders and still cannot answer "what is that wall over there"
without writing to it first. **Rolling it onto the `DigSite` at the first
swing** is the cheapest code by a distance, but the kind then does not exist
until the wall is hit, so it can never be drawn; and it would draw from
`resources::GameRng`, which shifts every later roll in the run.

So: `rock::kind_at(seed, x, y)`, a pure function, in the same discipline
`descriptions.rs` and `sectors.rs` already follow — an FNV-1a fold reduced
through `derive::index`, never `%`, never `GameRng`, never an `StdRng`
sequence. It answers for any coordinate whether or not anything has ever
touched it, which is what makes drawing an unmined face possible at all.

### Base space carries its own seed, because base space travels

**This is the trap that would have been found in play rather than in a
test.** `WorldMap::seed()` looks like the run's identity and is not:
`enter_next_zone` mints the next zone's map from
`seed().wrapping_add(0x9E37_79B9)`, so the world seed is different in every
zone. `BaseGrid` and everything in it, meanwhile, *survives a breach* — the
base is the one thing a run carries between zones.

Salted off `WorldMap::seed()`, every vein in the base would therefore
reshuffle each time the player portals, and a wall left half-cut would come
back a different kind with a different `max_hp` under its saved
`Durability`. So `BaseGrid` gains a `seed: u32` of its own, minted once at
`Game::new` and saved with the grid.

`#[serde(default)]`, so an existing save loads at seed 0 — a valid,
deterministic layout, not a special case — and this costs **no
`SAVE_FORMAT_VERSION` bump**, per the additive-field rule.

### Kinds are content and live in `assets/rock/`

A rock kind is a name, a hardness, a swing floor, a spawn weight and a
brightness. Every one of those is content by the project's own rule, and an ore is
a rock kind plus a drop — so putting the kinds in `tuning.rs` would make the
ore feature a Rust change for something the moddability rule says must be a
file drop.

**A brightness, and neither a colour nor a hue.** The map's one colour
rule is that hue answers "can I walk here"; `render/base.rs:209` already
records the consequence for anything that has to be told apart *within* a
band — "brightness rather than hue, since hue is already spoken for". A
free RGB would let the first mod ship a green wall that reads as crossable.
An authored *hue* is no better: `biome_tint` rotates every biome's hue by
however far the sector's own anchor has moved, so an authored hue would
fight that rotation and a seam would change kind as the player breached.

So `RockDef` authors a `shade` — a brightness factor against
`Biome::Entropy`'s own colour, ordinary rock at 1.0 and denser kinds above
it. Hue is untouched, the sector rotation still applies on top, and a dense
seam is a brighter patch of the same hole in the map, which is the same axis
`Excavated` and `Entropy` are already told apart on. `RockDb::load_dir`
validates the factor against a band and rejects the file with a warning if
it falls outside, exactly as `SectorDef` validates its own palette.

`RockDb::load_dir` follows `ItemDb::load_dir` exactly: one `.ron` per kind,
a malformed file skipped with a logged warning rather than a panic, and
`assets/rock/README.md` written in the same change as the schema.

**An empty directory is a supported install.** With no files at all the db
falls back to a single built-in kind carrying today's durability and shade, so
deleting `assets/rock/` restores *uniform* base-space rock the same
supported way deleting `assets/environment/` or `assets/policies/` does.

It does **not** restore the one-shot. The fallback kind carries the same
floor of 2 that ordinary rock does, because the swing floor is the bug fix
and not content: a player who deletes the directory is asking for one kind
of rock, not for their base back to being demolishable by a misstep. Stating
it here because the obvious reading of "restores the pre-feature game" is
the wrong one at exactly this field. The property must be held
at both ends — the fallback in the db *and* every reader tolerating a
one-kind world — rather than by gating the feature on the directory being
non-empty, which is how that property comes to hold by accident at one site
and lapse at another.

Three kinds ship. Ordinary rock keeps today's 24 and a floor of 2; two
denser kinds sit above it at lower weights, brighter against the same hue so
density reads as something in the rock rather than as a different material. Ores are deliberately **not**
part of this: an ore is a drop plus a use plus an economy, and `RockDef`
gains its `drops` field when that feature is designed, additively and for
free.

### Kinds come in patches, not pepper

`kind_at` folds *block* coordinates rather than tile coordinates, so a run
of adjacent cells resolves to the same kind and a dense kind reads as a seam
with an inside. Folded per tile it would be salt and pepper: every wall a
different colour, no seam to follow, and the visibility rule below would
convey nothing.

The block size is a knob and it is **unmeasured**, like every other knob in
this slice. Too small and an exposed face says nothing about what is behind
it; too large and one exposed corner gives away a whole wing. Play it and
record what it said under `docs/measurements/`.

### One swing can never take a whole wall

Per-kind durability alone does not fix the reported bug, it moves it: a
100-HP wall falls in one swing to a player who hits for 100. Scaling
durability with the player was refused outright — `tuning.rs` already argues
that a scaled wall "would make digging cost the same forever, which is the
one thing it must not do", and that argument is right.

So the fix is a **floor on the swing count**, expressed as a cap on the
damage one swing may land: `max_hp.div_ceil(min_swings)`. It sits inside
`Game::strike_rock`, which is the one place rock takes damage, so the player
and the crew meet it identically and neither can be given its own copy.

It is **level-independent**, which is the whole reason it is the right
shape. Levelling still speeds digging — three swings at level 1 down to the
kind's floor — it simply cannot reach one. At ordinary rock's 24 and a floor
of 2 the cap is 12, and a level-1 player's ~11 a swing is under it, so the
opening game's dig rate does not move at all: this bites exactly where the
bug was reported and nowhere else.

### Visibility is a face, and it is a display rule only

Colouring every wall in the base would hand the player a map of everything
they will ever dig. Colouring only the faces that have air against them
makes *exposing* a face the act of prospecting, which is a mechanic rather
than an arrow.

`BaseGrid::is_exposed(x, y)`: solid, with at least one **orthogonal**
walkable neighbour. Orthogonal rather than 8-way — a diagonal neighbour does
not expose a face. Derived per lookup and never cached, and
`base_entropy_system` is the argument: a re-knitting cell changes the
exposure of its four neighbours, and a cached flag would need keeping in
step with every open, floor and revert.

**The examine ray reads the same predicate as the map.** This is the trap
worth naming: if the map hides a kind but `x` names it, the hiding is
decorative and the mechanic is dead on arrival. One function, two readers —
the shape `views::drawn_on_surface_map` already holds for the surface map
and its examine ray. Unexposed rock examines as plain Entropy.

**And it is a display filter, never a gameplay one.** `strike_rock` resolves
the true kind through `kind_at` whether the wall is exposed or not, so
swinging blind into deep rock meets a dense wall's real durability and real
floor, and finding that out the hard way is the point. The regression to
head off is a later "fix" that resolves unexposed rock to the default kind
to make the two halves agree.

## Architecture

### Engine

**`crates/engine/src/rock.rs`** (new). `RockDef` — `id`, `name`, `weight`,
`durability`, `min_swings`, `color` — and `RockDb` with `load_dir` on
`ItemDb`'s pattern plus the empty-directory fallback. `kind_at(seed, x, y)`
lives here and knows nothing about `Game`: it takes a seed and a coordinate,
the way `descriptions.rs` takes a subject and a seed, and `game/` owns
mixing the seed.

**`crates/engine/src/base_grid.rs`.** `BaseGrid` gains `seed: u32`
(`#[serde(default)]`) and `is_exposed(x, y)`. The seed is private with a
reader, like `cells`.

**`crates/engine/src/game/base_space.rs`.** `strike_rock` resolves the kind,
sizes the spawned `Durability` from it, and caps the swing. `move_in_base`'s
solid branch reads `MiningMode` and refuses for free when it is off — the
refusal sits *below* `break_off_job`, which stays where it is, because
either way the player stopped working to try it.

**`crates/engine/src/resources.rs`.** `MiningMode(bool)`, defaulting to off.

**`crates/engine/src/game/lifecycle.rs`.** The `DigSite` load path re-derives
the kind from the site's saved `Position` and clamps `hp` against the
kind's durability instead of the flat constant. The site itself saves
nothing new — its kind is a property of where it stands.

**`crates/engine/src/game/inspection.rs`.** `view_tiles` fills the tile's
rock channel for exposed solid cells only; examine names the kind under the
same predicate.

**`crates/engine/src/world.rs`.** `Tile` gains the rock channel — an
`Option<f32>` brightness factor, `None` everywhere on the zone surface and
on every base cell that is not an exposed face. `Biome::Entropy`, `Biome::walkable` and
`rim` are untouched, so the map's shoreline logic does not move — which is
the reason the channel is a second field rather than a `Biome` variant per
kind. (A variant per kind is in any case impossible once kinds are data: the
enum is fixed at compile time and the file count is not.)

**`crates/engine/src/tuning.rs`.** `BASE_ROCK_DURABILITY` stays as the
fallback kind's number and gains a companion for the vein block size. Its
doc comment's claim that a wall "takes one late" is now false and is
rewritten rather than left to rot.

### app-core

`n` on the map screen toggles mining through `Game::toggle_mining`, and
`return`s — arming a tool is not an action and must not cost a turn. The
status column gains an `OwnLine` entry while mining is armed; `OwnLine`
rather than `Inline` because that column cannot grow horizontally and an
over-wide row is drawn off the panel in silence.

### gui

`render/base.rs` scales `Biome::Entropy`'s colour by the tile's factor where
it carries one and falls through to the flat colour where it does not. The
scaling happens *before* `biome_tint`'s hue rotation, so a face stays inside
the impassable band under every sector palette and is a brighter patch of
the hole it is part of rather than a new visual vocabulary. No new key binding work beyond `n` reaching `GameKey::Char`.

## Testing

TDD, failing test first, per the project's standing rule at every size.

**Hardness.** A developed player cannot open a wall of any shipped kind in
one swing. A level-1 player's swing count on ordinary rock is *unchanged*
from today — the two together are what say the floor bit where the bug was
and nowhere else. A denser kind takes strictly more swings than ordinary
rock at the same `swing_damage`.

**Derivation.** The kind at a coordinate is stable across a save/load, and
stable across a **breach** — the second is the test that would have caught
the `WorldMap::seed()` trap. `kind_at` draws no `GameRng`: a fold either
side of a call leaves the stream identical. Adjacent cells inside one block
agree; the shipped weights produce more than one kind over a sampled region.

**Mining mode.** Off by default in a new game. Off, a bump into rock spends
no turn and damages nothing. On, it swings exactly as today. The toggle
survives a save/load, and an existing save loads with it off. **Off, a
posted crew still cuts a marked cell** — the test that keeps the toggle the
player's own and not the base's.

**Visibility.** A cell with air orthogonally beside it reports exposed; one
with only a diagonal neighbour does not; deep rock does not. Cutting a cell
exposes its four neighbours, and entropy re-knitting one un-exposes them
again. Examine and `view_tiles` agree on the same wall — asserted against
each other rather than against a hardcoded string, so the two cannot drift.
**A swing at unexposed dense rock meets its real durability**, which is the
display-filter-only rule stated as a test.

**Assets.** An empty `assets/rock/` loads and plays, *and* a wall still
takes two swings with the directory gone — the half of that property that
the obvious reading gets backwards. A malformed file is skipped with the
rest of the directory intact, and so is one whose `shade` falls outside its
band. A census over the real assets: every shipped kind has a positive
weight, a `min_swings` of at least 2, a durability at or above the
fallback's, and a `shade` inside the band.

**The widened census.** `mining_a_wall_never_undercuts_a_mining_node`
(`tests/base_space.rs:1861`) currently holds one wall's fragment rate per
tick under a Mining Node's. It must now run over **every** kind, since a
kind that takes four times as long to cut changes the denominator.

Gates: `cargo test --workspace`, `cargo test -p feral-processes-engine
balance_sim` (the swing floor touches nothing `balance_sim` models, so the
curves must be *unmoved* — a moved curve here is a signal, not a pass),
`cargo clippy --workspace`, `cargo fmt`.

## Documentation

`CHANGELOG.md` with a version section and the digit decided by its own
preamble; this is additive and loads existing saves, so it is not a break.
`assets/rock/README.md` as the schema reference, written in the same change.
A seam line in `CLAUDE.md` and its argument in `docs/seams.md` under the
same title — the base-space seed and the display-filter rule are both the
kind of fact that costs tool calls to rediscover. `docs/manual.md` and the
root `README.md` are carved out of the doc obligation by standing practice
and stay untouched.

## Open questions

Three, all numbers, and none of them blocking:

1. **The durability and `min_swings` values for the two denser kinds.**
   Shipped at defensible starting values, unmeasured.
2. **The vein block size.** Argued above; the failure at each end is stated,
   the right value is not known.
3. **Whether ordinary rock's floor of 2 is enough.** It leaves a developed
   player cutting plain rock twice as fast as a fresh one and no faster, and
   whether that reads as terrain or as a tax is a play question.

`docs/measurements/` is where the answers go if they are ever measured.
