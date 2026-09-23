# Changelog: 0.13.0 to 0.13.99

Release notes for [feral-processes](README.md), `0.13.99` back to `0.13.0`. The versioning
policy is at the top of [CHANGELOG.md](CHANGELOG.md). Newer releases are in [CHANGELOG.md](CHANGELOG.md); older ones in [CHANGELOG-0.4-to-0.12.md](CHANGELOG-0.4-to-0.12.md).

## 0.13.99

**Caravan routes: load cargo at the Relay, send it to a town you know, and
the Credits come home.**

**A route can be left standing, reloading and departing again until you
sever it.**

**A Hostile town near the line your caravan walks takes a cut of it.**

**The Relay has a screen at last, and sorties finally have one too** —
`[S]` picks a squad for a sortie site, `[C]` builds a cargo manifest for a
town, `[X]` severs a standing route.

## 0.13.98

**Towns post their own work.** Every settlement now keeps a job board,
opened with `[J]` from its hub page, drawing on exactly the same contracts
the Broker does — no new schema, no new objective kinds, nothing to author.

**How many jobs a town posts is how much it likes you**, from one at Cold up
to four for an Allied town, and a Hostile one posts none at all. Courting a
town buys you work as well as a better price.

**A town leads its board with what it is good for** — a Materials town posts
the fetching, a Programs town the hunting — but it is a preference and not a
filter, so a board is never short of jobs it could have posted.

**A job is delivered where it was signed.** A town's contract is handed over
at that town's counter and the Broker will not take it, which is what makes
walking out to a settlement worth the trip.

**Finishing a town's job raises its regard for you, and handing one back
lowers it** — by less than finishing raises it, so a job you turn out not to
want is never worse than one you never took.

**Two towns in one sector post different work**, and a town's board turns
over more slowly than the Broker's, so what you saw on the way out is still
there when you arrive.

## 0.13.97

**Towns remember you now.** Every settlement carries a standing that runs
from Hostile through Neutral to Allied, shown on its hub page, and the game
says so the moment you cross from one band into the next.

**Trading at a town's counter earns its regard**, both directions counted —
what you spend and what it pays you are equally business it did — and small
baskets bank their leftover volume toward the next point rather than
rounding to nothing.

**So does clearing a nest or collapsing a stack near one.** A town hears
about what happens within sixty tiles of it and nothing beyond that.

**A town that has come to hate you shuts its counter.** The market screen
still opens and says so, rather than dropping you back to the map with no
explanation.

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
`standings` is additive behind `#[serde(default)]`, so a save from before
this release opens with every town at Neutral, which is where a new run
starts anyway.

## 0.13.96

**Existing saves load, and one resource is deliberately dropped from them.**
`save::SAVE_FORMAT_VERSION` stays at 32 — `settlements` is additive behind
`#[serde(default)]` and defaults to empty, so a save from before this
release simply materializes its towns as the party reaches them. The
exception is `StackMemory`: a frame's seed now folds in its tier, so a
memory written by an older build describes a maze this build no longer
generates. A save without the new `stack_memory_tiered` marker has its Stack
fog of war cleared on load rather than restored onto the wrong cells.

- **The world is persistent. A breach raises the tier; it does not rebuild
  the sector.** There is one map for the run. The party does not move, the
  ground does not change, the base stays where it stands, and every shelf,
  entrance and structure survives the crossing — what changes is what walks
  on it. A place can only be worth returning to if it is still there.
- **Towns stand in the world, and where each one stands is derived from the
  world seed rather than rolled.** Walk into one to enter it; it admits
  nobody, so the tile is a door rather than a floor. `x` toward one from a
  distance names it the same way.
- **A settlement has a name, a kind, a specialty and a temperament, and all
  four are content** — `assets/settlements/*.ron`, documented in that
  directory's README. Deleting the directory is a supported install and
  gives back a world with no towns in it.
- **`[M]` opens a town's market: one shelf, one basket, one commit.** Buying
  and selling settle in a single turn through the same core the caravan's
  own counter uses, so a basket can be funded by its own sales, and the
  buyback shelf a town keeps outlives anything built on its tile.
- **A town's temperament prices both directions, and Mercantile is not the
  average of the other two** — it competes on what it charges you and takes
  its margin on what it pays you.
- **`assets/sectors/` is retired.** Per-zone terrain deltas had nothing left
  to vary once a breach stopped carving new ground, so the biome palette is
  one fixed signal for the whole run.

## 0.13.95

**Existing saves load, with one deliberate exception.**
`save::SAVE_FORMAT_VERSION` stays at 32 — both `DownedPrograms` and `Tools`
are additive behind `#[serde(default)]`. `downed_programs` defaults to
empty, exactly the pre-extraction game. `tools` does not: a save written
before this release paid its material income through the kill drop this
release retires, so an absent `tools` key defaults to the starter tool
rather than an empty loadout — a migration for a save that predates the
concept entirely, not a re-grant into one that already made a choice.

- **A defeated wild program is left behind as a downed program you carry,
  not a pile of raw materials.** Species, level, rarity, boss flag and a
  rolled condition all travel with it in a new player store,
  `components::DownedPrograms`, capped at 10; a kill, a nest cache and a
  boss kill all leave one.
- **A carried tool strips a downed program down into what it drops.** The
  new `assets/tools/*.ron` catalogue ships two — `salvage_clamp` (the
  starter tool, forged into slot 1 at creation) and `core_tap` — each
  reaching a different category of the program for a different pool of
  items, at a ticked time cost.
- **`Mode::DownedPrograms`, opened from the pack, lists what you're
  carrying and previews exactly what each installed tool would give before
  you commit to one.** The previewed figure and the granted one can never
  differ — both read `Game::extraction_yield`, the one derivation.
- **The starter tool pays a median kill the same as the material drop it
  replaces.** `SpeciesDef::work_resource`'s old kill-time drop is retired;
  every shipped species keeps paying what it always did through the new
  `rich_in` field, which defaults to `work_resource` and needed no
  authoring pass.
- **Extraction is deterministic, not a dice roll.** The yield is a
  weighted apportionment of a fixed unit count, so it spends no RNG and a
  quoted preview always matches what you actually get.

## 0.13.94

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` is untouched —
nothing here reaches the save at all.

- **Powering down out in the field can be interrupted.** A charged rest rolls
  `REST_AMBUSH_CHANCE` once the charge has been taken; on a hit a pack engages,
  the outlet is spent and nothing is restored.
- **A free rest inside your base never rolls**, and it is safe by placement
  rather than by a locale check — the roll rides the branch that takes the
  charge, so base space cannot lapse into being rollable.
- **A jumped rest clears nothing**, which is the rule a refused rest already
  followed: the heal, the Power refill and the field buffs all sit below the
  roll.
- **A roll that hits but fields no pack lapses into an ordinary rest**, so a
  charge is never burnt for no fight at all.
- **`surface_ambush_pack` is an extraction out of `maybe_ambush`, not a copy.**
  A rest is the first roll site that cannot know its locale by construction, so
  the two pack builders are named as a pair and each states its placement rules
  once.
- **Three app-core fixtures no longer use a field rest as a one-line map
  action**, since `r` can now open a battle.
- **Sprite Forge's test scratch directory no longer races itself.** Keyed on
  the pid alone it was shared by all 21 sprite-forge tests at once, so a full
  workspace run failed intermittently on `AlreadyExists` — never when the file
  was run alone, which is the shape that reads as unrelated.

## 0.13.93

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` is untouched —
nothing here reaches the save at all.

- **A tile's con read is the glyph's own hue again, and the corner earmark is
  what the tiles that cannot spend that hue pay instead.** No mark small
  enough to share a cell with four other channels carries a scan as well as
  the ink in the middle of the cell does.
- **`ConRead` is the one place that is decided** — `Glyph(rung)`,
  `Earmark(rung)` or `None`, one value rather than two conditions agreeing at
  two draw sites, so "never both and never neither" is a property of the type
  rather than of a convention.
- **Two tiles cannot spend the hue: one drawn as a sprite, and a boss.** Art
  is authored near-white precisely so egui's tint multiplies through it, and a
  boss's magenta is the ink rather than a fifth rung.
- **The boss's corner mark goes with it**, its census retargeted from "not any
  other mark" to "not any con rung" — the reading a magenta glyph could
  actually be mistaken for.
- **`ConRead::of` takes the sprite call's own answer and never
  `sprite.is_some()`**, so a name the table has nothing under falls back to a
  glyph that can still carry the rung.

## 0.13.92

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` is untouched —
nothing here reaches the save at all, and the whole feature is inert without
both `FERAL_DEV_SPRITES` and a checkout behind the build.

- **A developer can draw a sprite in-game for any species, structure or map
  fixture, and the map shows it without a restart.** A main-menu row behind
  `FERAL_DEV_SPRITES` opens a picker of every subject the map can draw art
  for; the editor saves a 16x16 PNG into `assets/sprites/` under the name the
  loader already looks for, so no `.ron` file and no Rust changes.
- **Art is turned off by renaming it to `<name>.png.off`, never by deleting
  it.** `scan_sprite_dir` filters on the extension being exactly `png`, so a
  disabled sprite is already invisible to the loader and the glyph comes back
  untouched.
- **The editor's canvas mechanics are shared with the player's icon editor
  rather than copied.** `icon::Canvas` and app-core's `CanvasEditor` hold the
  cursor, the brush, the undo ring and the paint guard; both editors own one
  and keep their own sink.
- **`SPRITE_PALETTE` is a separate constant from `ICON_PALETTE`, and must
  stay that way.** The player icon's `v2` codec is 64 hex digits, so a
  sixteenth colour in `ICON_PALETTE` would encode as a digit meaning
  transparent and silently blank a drawing the player cannot get back.
- **The dev palette leads with a bright-biased value ramp** because the
  renderer applies a tile's colour as a multiplying tint, so near-white art
  inherits the species hue, `biome_tint` and the damage dimming for free.
- **A sprite can be painted with the mouse**, the first pointer input
  anywhere in the renderer, resolved to a cell before it reaches app-core and
  confined to this one screen.
- **A drag is one undo entry, and a click that changes nothing records
  none.**

## 0.13.91

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
both new fields are additive behind `#[serde(default)]`, so a save written
before this release loads with an empty receipt.

- **A perk screen and a companion's talent ladder can each be refunded whole
  for a flat Credit price.** `X` opens a confirmation; `y` clears that
  ledger, hands back every point spent, and takes the stats those purchases
  baked in back out.
- **A stat a purchase baked in now leaves a receipt, because neither grant is
  invertible.** `Perk::Buffer` and `TalentNode::Stat` read the value at
  purchase and floor at a whole point, so refunding without a record would let
  buy-wipe-rebuy compound maximum Integrity without limit.
- **A refund no longer makes the next banked-XP Perk Point cheap.**
  `convert_overflow_xp` prices each point off perks *ever* bought rather than
  off the list a respec empties, which is what keeps banked cap XP bounded.
- **Refunding Process Pool is refused while the roster is full**, rather than
  leaving more programs owned than there are slots to hold them.
- **A talent refund takes back the tree's routines and leaves the player's
  alone**, so a routine installed from a disk survives the wipe.

## 0.13.90

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this release is test-only and stores nothing.

- **A screen that reads a modifier arrow must be named in `App::handle_key`'s
  fold, and a census now says so.** The fold rewrites
  `ShiftLeft`/`ShiftRight`/`CtrlLeft`/`CtrlRight` to bare arrows for every
  screen outside a four-mode allowlist, so a picker that grows the four arms
  without being listed gets them unreachable — the modifiers silently become
  plain steps and nothing fails to compile.
- **It reads source, because that direction is unreachable at runtime.** The
  fold precedes dispatch, so an unlisted screen's modifier arms are dead by
  construction and behave identically to bare `Left` whether the bug is
  present or not; comparing the two lists in source is the only place they
  meet.
- **`crates/app-core` is warning-clean.** `seed_profile` takes `&Path`, which
  was the crate's only clippy warning.

## 0.13.89

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
nothing here is stored, and both new def fields are additive and
`#[serde(default)]`, so an existing `.ron` parses untouched.

- **A species or structure can name its own sprite.** `sprite:` in its `.ron`,
  defaulting to the def's own id, so dropping `assets/sprites/rootkit.png`
  into the tree is the whole of giving the Rootkit art.
- **The loader reads the directory instead of a list in Rust.** The table
  holds exactly what is on disk, so a name with no file behind it is now
  unreachable rather than a warning several frames late.
- **Nothing on screen changes yet.** All 47 shipped defs have no art and draw
  the glyph they always did; this is the code half of entity sprites, and the
  art is the next slice.

## 0.13.88

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
the icon string gained a `v2` form, and a `v1` string written by 0.13.87
still decodes, folded onto the new grid.

- **The icon you draw is 8x8 now.** 256 cells was a lot to fill with the
  arrow keys, and the canvas is the one screen where the work is
  proportional to the grid.
- **The sprite is still 16x16**, so each drawn cell paints a 2x2 block — the
  format art will land on later is untouched, and under nearest sampling the
  result is identical to a native 8x8 texture either way.
- **A drawing made under 0.13.87 survives.** Each 2x2 block folds to the most
  frequent painted colour in it, so a silhouette comes through at half
  resolution rather than being thrown away.

## 0.13.87

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
the two new fields are additive and `#[serde(default)]`, so a save written
before this release loads with no drawing and behaves exactly as it did.

- **You can draw your own icon now.** A 16x16 editor on the character-creation
  wizard's Icon step, from a fixed fifteen-colour palette — five steps of value
  and ten hues, because shading with value is what makes a figure read at this
  size.
- **`Tab` moves between the canvas and the palette**, so the arrows mean one
  thing at a time rather than changing meaning based on a mode you have to
  remember you are in.
- **What you draw follows you into the next run.** It is kept in the profile,
  which is what outlives a run, and copied into the save so that redrawing it
  later cannot repaint a character you already made.
- **A drawn icon is the one sprite in the game drawn untinted.** Every other
  sprite inherits a multiplying tint and must be authored near-white; the
  player's own tile inherits none of the hues that rule protects, so a drawing
  can carry its own colour there and nowhere else.

## 0.13.86

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
nothing here is stored; the con read has always been derived at draw time.

- **A program's glyph tells you what it is again.** Its colour used to be
  replaced outright by the con read for anything hostile, so a tile could say
  what a program is or how dangerous it is and never both.
- **How dangerous something is, is a bar along the bottom of its tile now** —
  green through red, the mirror of the rare-tier bar along the top.
- **A boss wears its own corner mark**, so being a boss no longer costs it the
  con read.
- **A creature that is both a boss and a nemesis shows both.** The two used to
  compete for the same colour and the nemesis won outright, which meant one of
  the two facts about the most dangerous thing on the screen went undrawn.

## 0.13.85

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
nothing here is stored; all three numbers are read at draw time.

- **The clouds move now.** They shipped at a wind slow enough that the
  leading edge spent three seconds crossing a tile, and since the edge is
  spread over several tiles by design, no single tile visibly changed inside
  the time anyone looks at one — so weather that was drawn every frame read
  as a still image. Doubled, in the direction it already had.
- **The map is darker, and the shadows are deeper.** A cloud rides the
  ground's shade and never the vignette, so a glyph under one is still drawn
  at full brightness; that is what makes a deeper shadow free, and it is why
  the vignette floors moved last and by the smallest step that reads.
- **The spawn ring is gone.** The magenta outline marked where you
  materialized on breaching a zone, which is a fact with nothing to spend it
  on once you have walked a few tiles away from it.

## 0.13.84

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
the ledger is an additive `#[serde(default)]` field, so a save written before
it existed loads with an empty one, which is exactly what that run recorded.

- **The base tells you what it has made for you**, on a new "Base output" row
  in the base menu: one line per item, the sector's total beside the run's,
  and what a machine made beside what you made by hand.
- **Every extract, compile, stall, haul and unit that entered or left the run
  is recorded when `FERAL_DEV_LOG` is set**, through one door that also feeds
  the player's screen — so the page and the analysis a retune is done from
  cannot disagree about what happened.
- **`dev-logs/README.md` documents all eight base records**, with the `jq`
  recipes for the questions they were added to answer.

## 0.13.83

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **Compiling something by hand no longer runs the clock at thirty times
  speed.** The progress bar spent the world's ticks sixty a second against
  the two a second the world moves at when you are standing still, so a
  batch aged your base by minutes while you watched a few seconds of
  progress — needs draining, machines cycling, raid pressure building and
  wild programs spawning, all of it sprinting for as long as the screen was
  up. The bar now spends ticks at the rate everything else does.
- **Hand-compiling costs a tenth of the ticks it did.** The tick price and
  the bar's speed are the same number seen twice, so slowing the bar without
  cutting the price would have made one Hardened Shell two and a half
  minutes of watching it fill. It is fifteen seconds now, and the median
  recipe is five. What a bench still buys you is everything except speed: it
  works while you do something else, it costs you no Power of your own, and
  a developed one reaches quality your hands cannot.

## 0.13.82

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
how many times a body swings is worked out from its class and level when the
blow lands, so there is nothing new to store.

- **A Striker swings twice.** Your class said what you were good at and then
  had nothing to do with an ordinary attack: affinities only ever reached
  the routines you ran, so a Striker and a Medic threw the same punch. From
  level 8 a Striker takes a second swing every round — a full one, not a
  lesser follow-up — and so does any companion or wild program whose own
  affinities make it a Striker. Level 8 is past the first sector's ceiling,
  so it is something you cross into rather than start with, and the second
  swing aims at whatever is in front of you after the first one lands.
- **A Crash now costs a Striker the rest of the round.** Fumbling badly
  enough to crash cost you "your next action", which meant exactly one blow
  back when everyone threw one. For anyone with two, it takes both.

## 0.13.81

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
the three new class variants append, so a save already naming one of the
five keeps the class it had.

- **Three classes no species can ever be.** A player's class was one of the
  same five roles a program can have, which meant every class had to be
  expressible as a spread of affinity multipliers — and so nothing that
  wasn't combat could be one. The player has their own class list now, and
  three classes on it that no species could hold: a **Decompiler**, whose
  every decompile attempt lands better and who trades damage for it; an
  **Invoker**, who carries two more routine slots than anyone else at every
  level, the level cap included; and a **Fabricator**, whose posted machines
  finish every work cycle a fifth sooner. The five original roles are
  untouched and still shared with the programs that have always had them.
  Each new class's effect is a named query in the engine rather than a field
  in its file, on the same seam a perk already uses — so the catalogue in
  `assets/classes/` stays a catalogue, and deleting it turns the affinity
  spread and the opening kit off without turning the effect off with them.

## 0.13.80

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
every new field is additive behind `#[serde(default)]`.

- **Compiling by hand costs the machine's own cycle, ten times over.** Every
  item the base's eleven assemblers make could be hand-compiled at the
  identical recipe cost, in a single tick, with no worker, no adjacency and
  no power — so the machines were a convenience rather than a route to
  anything. A hand-compile now spends ten times the cycle the machine that
  makes it would have taken: a Blank Substrate is 120 ticks against a
  Lathe's 12, a Hardened Shell 300 against an Armory's 30. The recipe is
  still yours; the shortcut is what got priced. A Compiling screen watches
  the ticks go by and any key stops it, keeping the units that finished and
  refunding the one in progress. Those are ordinary game ticks, so they cost
  Power like any other span of them — and a batch that would run your
  reserve out is refused whole rather than leaving you empty in the field.
- **The Zone Portal asks for a terminal product from every base chain, and
  the bill grows with the sector.** Breaching needed ten Portal Fragments
  and nothing else, which meant the entire production chain was optional and
  a run could reach the last sector having never built a machine. Opening
  the way now takes 24 Fragments and the output of the Power, armour and
  substrate chains besides; the Trace and Cache chains join at sector two
  and the refactoring chain at three, as their research comes into reach.
  Each line is ramped from the sector it was introduced in rather than from
  the first, so a demand that arrives late arrives at its authored price.
- **The base's power grid burns Power Cells to stay up.** A Recharger Node
  cost ten Core Fragments once and supplied the grid forever, which made the
  whole ledger decoration and the Line Driver pointless. A supplier now
  takes a Power Cell off an adjacent buffer every twenty ticks, and one that
  cannot pay goes quiet on both halves — the grid it feeds and the Power it
  trickles into you. The Home is exempt and stays free, so a cold base can
  still raise the Power Conduit that fuels everything else. The fuel is
  named in the structure's own file, so a mod can run its grid on something
  else.

## 0.13.79

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **The recovery mark rides the program being mended, and it is green.** A
  Repair Bay wore a bouncing red `+` while it was healing somebody, which
  put the mark on the building rather than on the body it was about to
  give you back — and painted it in the colour this map otherwise spends
  only on a raid's flash and a structure taking a hit. It now bounces over
  the program itself, in the same green a climbing Integrity wears
  everywhere else.

## 0.13.78

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **Deploying a Home is free, and the anchor lands where you stand.** It
  cost five Core Fragments against a starting kit that carried five or six,
  which made founding a formality for a default character and a dead end for
  a bought one — the creation wizard's Kit step replaces the class kit, so a
  run that spent its allowance on gear could not open a base at all and
  nothing said why. And the door now settles on the tile you deployed from
  rather than back at the sector's arrival point: walk to ground you want to
  come back to, then press it. The Home itself still stands on base space's
  own origin, so the way out is where it always was. Founding while standing
  on a Stack link is refused — an anchor sharing that tile could never be
  stepped on to be entered.
- **Two onboarding notices.** The first time your Power reserve goes under
  half, the game says so and names the two ways out — the same threshold
  your attacks start weakening at, not a second number beside it. And the
  first time a program of yours is benched with no Repair Bay standing, it
  says what a benched program can and cannot do and what brings one back. A
  base that already has a Bay is told nothing: the program walks there
  itself.
- The character creator's second step is titled **Achievements**, and its
  footer says what it is.

## 0.13.77

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **The field ramp tops out at 39 tiles instead of 135.** 128 tiles was
  picked off the world being unbounded rather than off the scale the map
  actually works at — everything the player reaches for sits inside a couple
  of dozen tiles, and the furthest the game scatters its own content is 40 —
  so the gradient read flat because nobody walked out of the first tenth of
  it. The cap now lands where the outermost Stack link already does: walk to
  the far links and you have seen the whole of it.

## 0.13.76

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
every new `PlayerSave` field is additive behind `#[serde(default)]`.

- **A run is bought as well as chosen.** A starting kit step sits between
  the class and the look, spending Credits off a shelf of what the run can
  afford — the class kit is now a floor rather than the whole of what you
  open with.
- **Four Perk Points are spent at creation, and the tutorial contract they
  replaced is gone.** Anything left unspent arrives with the run rather than
  being forfeited at the door.
- **A page before the class step names what earlier runs have already
  earned you**, so the profile's contribution is visible while you are still
  deciding what to build.
- **Twenty stat points, spent on top of the baseline rather than
  redistributed.** No step lets you leave an allowance unspent by accident,
  and every screen says how big its pool is.
- **The class picker says in words what each class trades**, and the
  manifest is headed by your name and your class.
- **The far field of a zone is the next zone's doorstep.** Zone 1 had no
  interior — every tile of it fielded one tier-0 pool at x1, which conned
  green against the bare baseline before creation spent a point. A surface
  spawn now ramps with distance from the opening ring's edge, capped at
  exactly one zone step, so a strong opening build has somewhere to walk
  without breaching.

## 0.13.75

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
every new `PlayerSave` field is additive behind `#[serde(default)]`.

- **A run opens on seven questions instead of two keystrokes.** Difficulty,
  class, look, stat points, a starter routine, a name and a summary, in one
  screen with one back button; `[R]` rolls whatever is left and jumps to the
  end.
- **A player has a class now, and it grants affinities and nothing else.**
  Five files in `assets/classes/`, each raising one category of routine and
  holding a different one down — so a class is a trade, and an empty
  directory is still a supported install that plays as the game did before.
- **A class brings its own opening kit**, replacing the four items every run
  used to start with.
- **Five points to spend on top of the stats you already had**, priced per
  axis: Integrity, Attack and Decompiler at one, Mitigation at three because
  levelling never raises it.
- **The free routine slot is filled at creation**, from an opt-in pool any
  ability file can join with `starter: true` — and the routine is *known*,
  not merely installed, so it can be etched onto a disk later.
- **You pick your own glyph and colour**, from six swatches held separable
  from every content hue by the palette census.
- **A run has a name, and the save list shows it** instead of a filename.
- **The summary says what your achievement record is about to pay** before
  you commit to the run, read off the same derivation that actually pays it.
- **`Mode::DifficultyPick` is gone**, folded into the wizard's first step.

## 0.13.74

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **A notification waits for Esc, and no longer for any key at all.** The
  full-screen news took whatever you pressed next as a dismissal, so a
  keystroke aimed at the map blew past it unread — and a burst of three
  could vanish to three unrelated presses.

## 0.13.73

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **A recharge repairs the programs standing with you, and nobody else.**
  Base staff mend at a Repair Bay and a squad away on a sortie lives on the
  provisions you paid for at dispatch — where a rest four frames down the
  Stack used to reach back and repair a base the party had not seen in an
  hour.

## 0.13.72

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **A badly hurt staff program takes itself to a Repair Bay.** Anyone left
  under a fifth of their Integrity breaks off, walks there and comes back at
  full — where a Bay used to serve only a program killed in a Forgiving run,
  which made it an inert building under Permadeath.
- **A Repair Bay wears a bouncing red `+` while somebody is in it.** The
  building you paid for gave no sign it was doing its job, and two busy Bays
  bounce out of step.

## 0.13.71

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **A worn piece keeps its affix on the manifest.** The name used to run off
  the end of its box, and the half you lost was always the tail — which is
  where the affix is printed.
- **EQUIPMENT is a full-width band on your own page.** A program's page keeps
  the columned box and elides a name that will not fit, from the middle,
  since gear carries meaning at both ends.
- **Each worn row is tagged `WEP`, `ARM` or `MOD`**, the vocabulary every
  other list that names gear already uses.
- **A negative affix reads `-30 DEF`**, not `+-30 DEF`.

## 0.13.70

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **A finished contract is summed up in its own words.** The completion
  screen and the log line name the job, what it asked for and what it paid,
  where every contract in the game used to close on one identical sentence
  about the Broker settling up.
- **The onboarding chain closes on its own screen.** It counts the missions
  behind you, never credits a Broker who is not standing yet, and the last
  one is where you are told the board has opened.

## 0.13.69

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **The way home is a routine you earn, not a key you always had.** Symlink
  is `AbilityEffect::Symlink` now — researched at Symbolic Links, etched,
  installed and run from the routine list — where `u` opened a one-row picker
  of teleport-capable structures on the first turn of every run.
- **A GC Entropy Sweep waits for the second sector.** The base you raise in
  sector 1 is never swept, so the first ground you lose is ground you had a
  breach to prepare for.
- **The replace picker rates the pieces it offers.** Every other list that
  names gear already carried the figure, and the one screen whose job is "is
  this better than what I have on" answered only in per-axis deltas.
- **The Coherence amenity is a Log Analyzer Bay.** "Defrag Bay" and "Repair
  Bay" read as the same building at a glance and are not.

## 0.13.68

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **The transfer picker's two figures are where the units would land.** `you`
  and `container` move as the basket is edited — a take fills the pack and
  empties the shelf in front of you — replacing a `change` column beside two
  columns that reported a *ceiling* and so never moved at all.
- **Putting cargo in a Depot no longer reads as spending money.** The pack
  column was the Depot's shared room seen through one row, so filling any row
  lowered the figure printed on every other one, Credits included.
- **Credits are offered on neither side of a transfer.** A currency is what a
  transfer is priced in, never a thing that moves into a Depot.

## 0.13.67

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **The arrows set a compile batch.** Right increases and Left decreases on
  the compile quantity page, Shift jumps to the most this batch can afford
  and Ctrl halves the gap to it — the gesture the depot picker and the
  caravan basket already use for a number, on the one quantity screen that
  was still digits and Enter.
- **The companion screen heads each role.** The roster is a run per role now
  — in your party, away on a sortie, base staff — where a dispatched program
  used to be listed among the staff it is no longer part of.

## 0.13.66

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **A base with nothing that services a need no longer earns a grudge for
  it.** The program is still told, in the same sentence as before; what it
  stops doing is holding the base to account for a building the player may
  not have researched, may not have the materials for, and has never been
  told they want.
- **A walled-off amenity still earns one**, which is the half of this with an
  errand attached — the base had an answer and could not deliver it.

## 0.13.65

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
`SaveData::game_over` is additive behind `#[serde(default)]`.

- **A Permadeath run that flatlines cannot be reloaded.** The slot is sealed
  with the run's own verdict, the load list shows it as FLATLINED, and
  `Game::load` refuses it — the mode used to leave the last autosave sitting
  there, at most fifty cycles before the death and with no record of it.
- **A machine already kept worked satisfies "Man the Node".** The mission
  could only be finished by turning the standing job off and on again, which
  pulls the body off the machine and is the opposite of what it asks for.
- **A GC Entropy Sweep now waits for a base of five**, up from three, which
  is past the roster a run opens with.
- **A program that has downed tools no longer counts as defending the base.**
  A base whose whole staff had walked off read as fully staffed and was swept
  until its machines were gone.
- **One bad memory can no longer stop a program working.** The line was set
  against a memory's valence alone and the worst single grudge in the game
  reaches nearly three times it, so a program worn thin on a base with
  nothing servicing the need downed tools for longer than the run that
  earned it. Downing tools takes a pattern now; one grudge still sulks.

## 0.13.64

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **Routine slots come in pairs now, doubling every kit at every level.** A
  companion still reaches its ceiling at level 6 and the player at 25, with
  twelve slots waiting there instead of six.
- **A new game opens with a free slot beside `decompile`**, where the
  player's one starting slot used to be spoken for.
- **The program page's ROUTINES box still draws six rows**, so a kit past
  that spends its last line on a "+N more" note — the manifest has the least
  clearance in the renderer and buys nothing new here.

## 0.13.63

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **`w` on a program's manifest sends the map's camera to it, and Esc hands
  it back** — the base runs at two ticks a second now and there was no way
  to watch any of it happen except by standing where it did.
- **What may be watched is whatever the sim actually walks**, which is
  `ProgramRole::Staff` less the guards: a party member, a wielded program, a
  squad away on a sortie and a guard all keep the tile they were last
  written at, and parking a camera on one claims a program is somewhere it
  isn't.
- **Deliberately not `position_is_honest`**, the neighbouring rule, which
  goes false for a worker standing at its own machine — the one moment the
  player most wanted to be looking.
- **The camera reads `Game::watch_position` every frame and lets go the
  moment it answers nothing**, so a program dissolved, dispatched, taken
  into the party or left behind in base space is one rule rather than a list
  of endings.
- **A step releases the camera and still steps**, because walking with the
  view somewhere else is walking blind and a swallowed movement key reads as
  the game having frozen.
- **The watch line takes the ground readout's mount rather than a border of
  its own**, since `map_pane`'s bottom border carries nothing by design and a
  strip there would re-lay the whole grid the moment `w` was pressed.

## 0.13.62

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **The world runs at two ticks a real second, where it ran at one.** The
  base read as frozen — idle staff drifted a tile every six seconds and a
  dig swing took twelve — and `WORLD_SPEED_MULTIPLIER` in app-core is now
  the one knob for it.
- **Nothing is rebalanced by that**, because every rate in the game is
  priced in ticks: raids, need decay, caravans and production all keep
  their rates relative to each other, and what changes is how much world
  time a real second buys.
- **Wild programs on the map keep the wall-clock pace they had**, held
  there by two constants pinned to the new knob — the wander cooldown,
  hoisted out of `wander_ai_system` into `WANDER_COOLDOWN_MIN_TICKS` /
  `MAX_TICKS`, and a halved `WILD_SPAWN_CHANCE`.
- **A nest guardian is deliberately not held back**, so a pursuer and the
  player both move at two tiles a second and you still outrun a chase in a
  straight line without ever shaking it.
- **The spec for wanderers in the Stack is written and nothing is built.**

## 0.13.61

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **The base anchor is drawn as a portal now, not a `#`.** Its glyph was
  chosen by elimination rather than by meaning — `>` and `<` were claimed by
  Stack links, Blue and Cyan were reserved — so it was the one fixture on
  the zone map that said nothing about itself.
- **`assets/sprites/anchor.png` is a ring with a bright core**, hard-edged
  rather than anti-aliased, and near-white so it inherits the anchor's tint
  the same way every sprite does.
- **The "Base Space" notice draws the same art and no longer names a
  character**, so the tutorial and the map agree on what to look for.
- **`EntityView::is_anchor` is what the renderer reads**, the anchor being
  the one map fixture that is neither a creature nor a `Structure`.

## 0.13.60

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **Cloud shadows drift across the zone map: soft patches of shade, off-axis
  and slow enough to be caught rather than watched.**
- **They are pure atmosphere and mean nothing.** A shadow crossing a biome
  under a Static event is not that event and does not mark it — Static claims
  a whole biome at a time and still reads out on the map pane's border.
- **Base space and the Stack have no sky, and neither takes a shadow.**
- **A cloud falls on bare ground only**, so a structure's damage wash and a
  glyph's own colour are never dimmed by something passing overhead.
- **The vignette goes on meaning the Power reserve alone**, which is why the
  shade rides the per-tile jitter instead.

## 0.13.59

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **The surface has weather: four Static events, biome-wide and either on or
  off, that stack on whatever the ground was already doing.**
- **Which event is live is derived from the world seed, the sector, the biome
  and the clock epoch, so there is no save field, no roll to scum, and it
  rotates on its own.**
- **Static is the first thing to bias the ambush roll, so ground you have
  crossed safely all run can turn dangerous without the map changing.**
- **Packet Flood claims Open Grid, which has no standing condition, so the
  biome most of the map is made of is now safe *usually* rather than
  always.**
- **Ground effects finally say what they are doing: the crossing line names
  the condition, its description arrives the first time you meet it, and
  Static announces its arrival and its passing.**
- **The map pane's top-left border stops reading `SECTOR MAP` and reads the
  ground you are standing on instead.**
- **The first Static of a run takes the screen, on the tutorial half of
  `NotificationKind`.**
- **Ambient effects are a table in Rust, not a catalogue on disk:
  `assets/environment/` and `EnvironmentDb` are gone, and `GroundCondition`
  and `StaticEvent` are the census.** This is a mod-facing content directory
  removed — a third-party file dropped there is no longer loaded.
- **`Game::terrain_at` is the one door onto what a place does to you, and the
  zone-1 gate and the base-slab refusal have one definition between them.**

## 0.13.58

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **The surface map's vignette floor answers to the Power reserve, 0.75 at a
  full one down to 0.60 at an empty one.**
- **The Stack corridor says the same thing down the axis it has: the fog
  thickens as the reserve drains, and the cell the party stands in front of
  is identical at every reserve.**
- **Neither half dims what the player must not miss — the surface floor
  stops well short of illegible, and `MARK_FOG` takes no reserve term.**
- **No engine number moves; `view_cone` and `visible_rows` hand over the
  cells they always did, so this is a look and not a sight-range change.**
- **`hud::log_frame`'s `POWER_MAX` points at `components::POWER_MAX` rather
  than a hand-copied `100.0` whose own doc comment named it.**
- **Notification copy is a table in Rust, not a catalogue on disk:
  `assets/notifications/` and `NotificationDb` are gone, `NotificationKind`
  is the census and `NotificationKind::def` the copy.**

## 0.13.57

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32;
`PlayerSave::tutorial_seeded` is additive behind `#[serde(default)]`, which
is exactly what field-named RON retired migrations for. A run in progress
when this lands is never told to build a Home it built forty hours ago —
the whole chain is filed as finished at load.

Onboarding is a **chain of contracts** now: eleven missions, one live at a
time, handed to the player rather than offered, drawn green, and suppressing
the ordinary board until the last one is done.

- **The first hour was discoverable and none of it was offered.** A new run
  dropped you on a surface with five Core Fragments and no instruction.
  Everything it asks of you — raise a Home, examine a thing, decompile a
  program, stand up a Broker, work the transfer screen, queue a standing
  order, spend a perk point — you had to find. The `starter: true` queue was
  the first attempt and was not enough: it biased a board draw, it needed a
  Contract Broker you had not built, and it said nothing about order.

- **A tutorial mission is an ordinary contract.** `ContractDef` gains
  `tutorial: Option<u32>` — a *step*, spaced 10 apart so inserting one later
  renumbers nothing. Nothing downstream can tell one from a contract you
  signed for: the ordinary `contract_system` advances it and the ordinary
  `complete_contract` settles it. `ContractDb::tutorial_chain` is the one
  derivation of what the chain is, and the run's position in it is **the
  first mission not in `contracts_done`** — no cursor, no index, nothing
  that can disagree with the save about where you are.

- **`Game::ensure_tutorial_held` is the one writer**, and it bypasses
  `accept_contract` entirely. Three things follow as *omissions rather than
  checks*, which is the point of routing it that way: `MAX_ACTIVE_CONTRACTS`
  never sees it, so the cap keeps meaning what it meant; `broker_reach`
  never sees it, which is what lets the first five missions exist before a
  Broker does; and `offerable` never sees it, so no `min_zone` holds the
  chain up. It cannot be given back — an unbreakable chain with a give-back
  key is not a chain — and `[A]` refuses with a sentence on both the popup
  and the log rather than doing nothing.

- **Two new objectives, and six new verbs behind one of them.**
  `Objective::Hold` is *have this many in your pack at once*: state-shaped,
  latched, no Broker, and met four frames down — which is what lets the
  chain teach that fighting pays in stock before a Contract Broker is
  standing. `Objective::Perform { deed }` carries all six new verbs over a
  closed `Deed` enum, because a deed is an engine event a mod cannot emit,
  so a string would buy openness onto nothing while costing a mission that
  names a deed that does not exist and never finishes.

- **`already_met` takes one `ObjectiveState` instead of three positional
  arguments.** It has two readers that must not drift, so every objective
  added used to cost a signature change at both; the next one costs a field.
  The board still asks it at depth 0 with an empty pack, deliberately: the
  board is *the sector's*, and answered from where the party is standing a
  `Descend(1)` would drop out of the pool the moment you went one frame
  down — and since the draw uses `swap_remove`, a pool one entry shorter
  reshuffles every slot. A board that changed as you walked.

- **The first decompile cannot fail.** A run of bad rolls would end
  onboarding permanently, which is the one thing an unbreakable chain must
  not do. The catalyst is still spent — the lesson that decompiling is
  priced in catalysts is the half that stays — and the branch reads the live
  mission's *objective* rather than a flag, so it is content-driven and
  disarms itself the moment the chain moves on.

- **Every mission opens with a full-screen briefing** carrying the
  contract's own name, objective and description, and pointing at `[4]`.
  One templated `assets/notifications/onboarding_mission.ron` filled at fire
  time rather than eleven files repeating what the contract already says, so
  a twelfth mission still costs one asset file and no Rust.
  `Game::notify_filled` and `Game::notify_with_detail` are both thin calls
  onto one private `queue_notification`, so the latch and the resolved push
  still live in exactly one place.

- **A contract's description ran off the popup.** The screen pushed it
  unwrapped while every other prose surface in `render/` goes through
  `description_rows`, and the width census measured only the row above it —
  which is how eleven descriptions several times longer than any other
  contract's shipped against a green suite. `contract_rows` is split out of
  `draw_contracts` so the census now measures what the screen emits rather
  than what the helper would have returned.

- **Three censuses keep the chain finishable.** A mission you cannot finish
  ends onboarding for the rest of the run with no key to press, so the
  shipped set is held to: build costs never outrun payouts, every `Deed` has
  a `note_deed` caller outside the tests, and every id a mission names
  resolves. A fourth holds the briefing to the screen it is drawn on, filled
  — the plain height census would be measuring `{description}`, seventeen
  characters where a paragraph goes.

## 0.13.56

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
The info column opens on the pane the party's locale asks for, the depot
picker is a table whose arrows point at its own columns, and a settled
contract names its payout on the screen that announces it.

- **The info column always opened on BASE, wherever the party was.**
  Standing out on the surface — with nothing base-shaped in front of you —
  meant reading a pane about the base while the crew you were walking with
  sat one keypress away. The tab now follows the party across the base
  boundary: walking out selects CREW, walking back in selects BASE, and a
  zone breach lands on whichever the far side wants. `App::sync_info_tab_to_locale`
  is the one writer, called from `after_tick` so every verb able to move the
  party reaches it through one hook rather than each setting the tab itself,
  and it only rewrites the tab when the side actually changes — so a manual
  `1`/`2`/`3`/`4` press survives until the next crossing. A fresh or loaded
  run opens on the true tab rather than waiting for a crossing that, from
  its point of view, already happened. The Stack rides the same boolean:
  underground reads as outside, so descending selects CREW and surfacing
  reselects BASE.

- **The transfer picker packed three figures into one unlabelled suffix.**
  A row read `3 / -5 .. +128`, which said nothing about which number was
  your pack and which was the shelf you were standing next to — the screen
  the whole feature exists for, and it was the hardest one in the game to
  read. It is four named columns now, `item | change | you | container`,
  under a header. `you` and `container` are still `App::put_available` and
  `App::take_available` — the same live ceilings, so a `you` reading 0 while
  the pack holds units is still the screen saying the other rows have spent
  the Depot's room — but the `-`/`+` decoration is gone, because the heading
  now says which end it is. A row nobody has touched reads a bare `0` in the
  change column rather than `+0`: a direction with a quantity of nothing is
  not a direction.

- **The arrows now point at the columns they move stock toward.** With the
  container as the rightmost column, Left pulls units off it toward you and
  Right pushes them from you into it — the reverse of what shipped, which
  had been inverted by request back when the screen had no columns to be
  inverted against. Shift still takes a row to the end its arrow heads for
  and Ctrl still halves the gap to it. The sign convention underneath is
  untouched: negative still puts and positive still takes, so the basket,
  the clamp and `transfer_items` never learned about the change. The
  controls page was telling the player the old arrows and now tells them the
  new ones.

- **A closed contract never said what it paid.** The alert screen took over
  the display to announce the Broker had settled up, and the figure reached
  the player only as a log line they had to scroll back through afterwards.
  The payout now sits on that screen, in the notification's own colour so it
  reads as a figure rather than as more prose, and it is built from the same
  call the log line makes so the two cannot quote different numbers. What
  carries it is a detail on the queued notification rather than a field in
  the `.ron` — a payout is what the firing site knows and a content file
  cannot author — and the screen has no scroll, so the census that says the
  shipped catalogue fits now measures every notification as though it were
  the one carrying the longest payout the shipped contracts can word.

## 0.13.55

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
A held contract reaches the HUD, two keys stop being invisible, a skipped
battle reveal makes a sound again, and a dig plan the crew cannot reach
stops soaking the crew.

- **Contracts are a fourth info-column tab, on `4`.** A held contract had no
  HUD presence at all — no pane row, no attention kind, nothing in the
  status bar — so the only always-visible figure was a bare count on the
  manifest stat sheet, and reading `8/10` meant opening the base menu and
  walking into the Contracts screen. Two rows a contract now: the name with
  its progress, the objective under it. The tail takes the attention colour
  once progress reaches the target, since a contract you can walk in and
  hand over is the actionable state, and the pane reads from anywhere the
  Stack included. The key is bound beside the other three rather than
  behind the Stack's own handler, so it works underground.

- **The Contracts screen's footer was painted outside its own popup.** One
  122-character sentence measuring 1175px into 1105px of body at 1280x720 —
  and since nothing clips horizontally, its last seven characters were drawn
  over the map beyond the border and "Esc to close" lost its verb. It is two
  lines now, and the abandon hint leads the second: `[A]` had been sitting
  at character 74, in the third clause, dim, the only bracketed letter on a
  screen whose every other shortcut is a digit. On screen the whole time and
  still unfindable, which was the bug. The overflow census the other popup
  screens already carry now covers this one too.

- **The controls page names the side column's digits.** `1`-`3` were never
  documented and `4` would have joined them unmentioned. The tab row was the
  only place they appeared.

- **Skipping a battle reveal was silent.** Pressing the action key mid-fight
  to dump the rest of the round jumped the reveal to the end without walking
  the loop that plays the swing cues, so a fight fought at any speed made no
  sound at all. A skip now plays the *loudest* blow among the lines it
  dumps — one cue rather than one per line, or a wipe is six clips inside a
  frame, and loudest rather than first so a round that landed a crit sounds
  like a crit however many plain hits came with it.

- **A dig plan nobody could reach starved the one cell they could.** The
  scheduler already dropped the boxed-in interior of a marked block before
  budgeting bodies for it, but left the *other* refusal — a cell with a
  perfectly good face and no route to it — below the cut that trims wants to
  the number of programs. A run of those (a pocket entropy sealed off, or a
  plan drawn past the walk cap) sorts first in tile order, costs no body when
  its turn comes, and pushes the reachable rim off the end of the list: the
  crew stands idle in front of a plan, and nothing says why. Both refusals
  are now answered above the cut, sharing the block that already dropped
  unreachable build requests. Asked one walk per want that would have been a
  Dijkstra field per face across a hundred-cell plan every tick, so the
  field is built from the *body* instead, once, leaving each want a lookup —
  in a connected base, one walk for the whole scheduler.

## 0.13.54

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
One rendering fault, reported from play, and the general test that should
have caught it two releases ago.

- **The log pane's filter header was cut through the middle.** `0.13.53`
  moved the vitals onto the log pane's top border and the
  `LOG All · Field · Base` header into the pane's body beneath it — but a
  border strip paints an opaque background *centred on* the line it rides,
  so the vitals reached nine pixels down into the body and wiped the top of
  the header a moment after it was drawn. 4.32px of a 13px row, across its
  whole width. The body now starts below what its own strips paint, at both
  ends of the pane, from one expression rather than a figure per edge.

- **The pane was asking for fewer rows than it had room to draw.** A
  pre-existing miscount, unrelated to the above and in the direction that
  costs the player news: the collapsed pane requested three message lines
  against room for four, and the expanded one seven against eight. Both are
  now what the pane actually shows, asserted through a real draw rather than
  by arithmetic agreeing with itself.

- **The keybar end had the same latent fault and no symptom.** The body's
  floor sat inside the reach of the keybar's own background quad; nothing
  was clipped only because row baselines happened to miss that band at every
  supported window size. It is now held by the same expression as the top
  edge rather than by coincidence.

This is the third release to fix one bug — an opaque background painted over
text already drawn — and each of the first two was let through by a test
that checked one named rectangle instead of what was actually painted. So
the fix that matters here is the test: `draw_log_pane` is driven through a
real painter and **no fill painted after a piece of text may overlap that
text's ink**, for every text the pane draws, in both its states. It reads
ink rather than layout boxes, because the leading around a line is space
nothing draws into and measuring it would raise a false alarm on every row.

## 0.13.53

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32
and no save field is added — the one thing this release does store, a
notification's "once ever" latch, goes on the cross-run profile rather than
the save. A new feature and five bugs, all six of them things the map screen
was doing in front of the player.

- **Full-screen notifications.** `assets/notifications/` is a data
  catalogue in the shape `assets/needs/` established: an absent directory
  loads silently empty, a malformed file costs that one notification and
  never the startup, and deleting the directory restores the game exactly as
  it was. `Game::notify` is the one door — it resolves the def, checks the
  repeat latch and queues a resolved value — and the queue is drained only
  from `Mode::Playing`. That single equality is the whole timing rule, so a
  fight, any picker and text entry all keep a notification waiting rather
  than having it thrown over them, and a mode added later is covered without
  being named. Achievements are a second *source* and not a second door: the
  card is built from the achievement's own name and description, so a new
  achievement earns one with no notification work. Two repeat policies, once
  per session and once ever; the third — once per *run* — is rejected in the
  doc comment, because a session-only queue would make it mean the first.

- **The expanded log moved the map.** SPACE's four extra rows were paid for
  out of the map pane's height, so every press re-laid the whole grid out
  under the player — a key whose entire job is "show me more of what I have
  already read" moved the thing being looked at. The map is now derived from
  the *collapsed* log at every window size and the log's bottom edge is the
  fixed one, so the expanded pane grows upward as an overlay.

- **The info column overhung the log pane by a row.** It ran to the window's
  bottom edge while the log stopped one row short of it — the reserve the
  keybar's glyphs straddle — leaving the two panes' bottom edges out of line.
  The column now ends where the log does.

- **The vitals were being cut in half, and then hidden altogether.** A
  border strip centres its background on the line it rides and reaches past
  it on *both* sides, so the log pane's `LOG All · Field · Base` header —
  riding the log's top border — painted over the lower half of the
  `MIT ATK STR DEC` readout riding the map's bottom border, baseline
  included, every frame. Two strips were sharing one line. The vitals take
  the border, because they are the readout that must not be covered, and the
  filter header goes back to heading the log's body, top-aligned with the
  messages it names. Mounting the vitals on the log pane also means they
  travel with it, so expanding the log no longer hides them.

- **Long log lines drew across the info column.** Nothing clips a row
  horizontally, so a line longer than the pane did not stop at its right
  edge — it ran over the roster beside it, reading as a fault in that column
  rather than in the log. Lines wrap now, measured against the UI face's own
  advance rather than a fixed column count, and what will not fit is cut
  from the **oldest** end: the pane is handed its rows oldest first, so
  stopping at the bottom would have dropped the newest news.

- **A structure's description ran off the deploy screens.** Both the deploy
  menu and the direction prompt emitted an authored description as one
  unwrapped row, and the widest shipped one ran some 1715px past the popup's
  body before vanishing — 303 characters against a budget of about 114. Both
  wrap now, through the helper the perk and research pickers were already
  using rather than a second copy of it.

## 0.13.52

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32
and no save field is added. Four bugs, three of them things the game was
already doing wrong in front of the player and one of them a run-ender for a
base that had only just been founded.

- **An opening base is too thin to raid.** A GC Entropy Sweep is meant to be
  attrition the base absorbs, and a base with two bodies has nothing to
  absorb it with — early raids were taking the whole staff down and leaving
  every program `Downed`, with nobody standing to defend against the next
  one. `tuning::RAID_MIN_BASE_STAFF` (3) is the floor `Game::raid_check` now
  measures against, counted through the existing `Game::base_staff` and so
  through `party::role_of`, with `Downed` bodies excluded — a base already
  reduced to wreckage is exactly the state this closes. The gate sits
  **after** the RNG roll, `maybe_spawn_wild_creature`'s rule, so it shifts
  no seeded stream. Eight existing raid tests ran with **zero** staff and
  were silently gated out by it; they stand bodies up now rather than the
  gate being weakened to suit them.
- **A downed program says so.** One benched by a Forgiving death read as
  `idle` on the manifest and on the examine line, which is the same word a
  companion nobody has ever posted anywhere gets. `bench_or_dissolve` calls
  `detach_from_play` *before* inserting `Downed`, so the program has no
  party slot, no wield slot and no `Task` — every branch in
  `Game::program_activity` found nothing and fell through. One early return
  in that derivation, which both surfaces already share, so the fix lands on
  both at once and costs the renderer nothing. The word is `recovering`,
  which is what `run_repair_bays` already says when a Bay finishes the job.
- **The status bar was eating the map pane's title.** A border strip centres
  its background quad *on* the line it rides, reaching half its own height
  above it, and `map_pane.y` sat exactly on the status bar's bottom edge —
  so the bar's opaque fill, drawn after `draw_map_frame`, took the top of
  SECTOR MAP and the threat readout with it. The panes below the bar now
  start `TOP_STRIP_CLEARANCE_RATIO * m.small()` further down, built off
  `strip::PAD_RATIO` rather than a copied literal so the two cannot drift.
  The bar's own rect is unchanged. The test locates the **painted quad**
  through paint order rather than asserting a layout number, because a
  layout figure passes against the bug.
- **SPACE doubles the map's log pane** to eight rows and back, off
  `App::log_expanded` on `stack_zoom`'s precedent. Bound in
  `handle_playing_key`'s top match, which runs before the hand-off to
  `handle_stack_key`, so it works underground too rather than falling
  through that dispatch's `_ => {}` the way `r` once did. The pane's row
  count is already derived from its height, so the taller pane fills itself.
  Not advertised on the keybar — that strip has no slack at 1280x720 — so it
  is documented on the controls page instead.

## 0.13.51

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32
and no save field is added — but **this release changes how hard fights
are**, in one direction and mostly underground, so a run in progress will
notice.

Fights had two ceilings and neither bounded the other. `Game::group_pack`
caps a fight per group (`MAX_GROUP_SIZE`) and per group count
(`MAX_ENEMY_GROUPS`), so what one intrusion could actually field was those
two multiplied — four hundred programs in principle, and thirty-three
against a party of four at zone 3, five frames down.

Two things kept that hidden. A **surface** fight is whoever `gather_pack`
found standing together, which measures 2.6 bodies at zone 3 against a
ceiling of twelve, so the missing bound was also the bound nothing ever
reached. A **Stack** ambush is not: `stack_encounter_pack` takes one species
pick and one full group roll per group slot the curve allows, so it fills
the ceiling by construction. The same curve was producing a shallow-Stack
fight four times the size of the surface fight beside it.

`0.13.21`'s change to `Game::danger_steps` — the zone step added to the
depth step underground — is what turned that from large into fatal. It was
aimed at which species a frame may field, and `danger_steps` also feeds both
group-size curves. Its effect on lair fights was measured at the time; its
effect on the ordinary wandering pack was not, and the Stack has been
unwinnable from **depth 3 down** ever since: 2.0% at depth 3 and 0.0% at
depth 5 for a party at the zone level cap, over 200 staged fights each.

- **`tuning::MAX_PACK_BODIES` is the bound the other two never expressed.**
  Eight bodies across every group in the fight, trimmed off the largest group
  each pass — so a mixed pack keeps its variety and loses its depth, four
  species two deep rather than one species eight deep. The bodies it turns
  away **stay standing on the map** and are met on the next bump, exactly as
  the surplus past `MAX_ENEMY_GROUPS` already did. Nothing is despawned and
  no encounter is skipped.
- **What that does to the Stack:** depths 2 through 5 now run 100%, 93.5%,
  67% and 39.5% for a party at its zone's level cap — a curve that descends,
  where before it fell off a cliff between the second frame and the third.
- **Two fights outside the Stack move with it**, because the bound is on
  every fight rather than on the Stack's alone. A zone-3 depth-3 lair goes
  from 71% to 91%, and a **surface** pack at zone 6 — which could roll 17.5
  bodies — from 64% to 100%. The late-game surface has never had a real
  ceiling on how many programs may engage at once, and now does.
- **The arena could not field the party the game permits.** `set_level`
  clamped every scenario's companions to `arena_level_ceiling()`, which was
  the live ceiling until Kernel Rings stopped buying levels in `0.13.36`;
  since then it has sat *below* the zone cap from zone 2 on, silently
  clamping **upward** authoring. A zone-3 scenario asking for the level-23
  party that zone allows was staged at 12. The ceiling is now the higher of
  the two, which keeps the zone-1 property that figure was introduced for —
  five shipped scenarios author `level: 12` and zone 1's cap is below it.
- **A benched companion counts as one that went down.** `companions_downed`
  read `hp > 0`, and `0.13.36`'s Forgiving benching leaves a dead program on
  the roster at HP 1 — so the arena's "companions down" column has been
  structurally zero since it landed.

`docs/measurements/2026-08-28-stack-depth-curve-after-danger-steps.md` has
the sweep behind the constant, including the isolation of the three terms
that feed a deep-Stack fight — volume, the species window and the per-body
stat step — and the finding that the species window is a **threshold rather
than a gradient**: pulling it back one step or two does nothing at all, and
three steps is a cliff.

## 0.13.50

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32,
and this release adds no save field, changes no formula and moves no number.
`Perk`'s variant order is save format — bincode encodes an enum positionally,
so `PlayerSave::unlocked_perks` holds indices into it — and nothing here
touches that order.

An internal change with no gameplay effect, recorded because the seam it
moves is one somebody will have to reason about later.

Every perk's effect was a hook written at the formula that used it: the
mining roll, the hunger multiplier, a decompile's HP term, recipe costs, a
Trace rise, roster capacity, a kill's salvage, the base repair rate, a
compiled copy's quality floor, a direct `Stats` write. So the question "which
formula does this perk touch" was answerable only from the enum's doc
comments — prose, which cannot be kept in step with the code beside it.

`perks.rs` now holds one named query per perk and the call sites read them:
the module owns the answer, the caller owns the application, which is the
inversion `needs::strain` and `party::role_of` already use. Ten perk tuning
constants went dead at their old call sites in the process, which is the
evidence the hooks actually moved rather than being renamed in place.

- **The queries come in two families, and the difference is load-bearing.**
  Most take the player's `Perks` and name their own variant, so no call site
  says `Perk::` at all any more. `mining_roll_bonus` and
  `quality_floor_bonus` take a bare **level** instead, because
  `CycleModifiers` and `CraftOrder` carry that level to a formula whose
  subject may be a program rather than the player — a `Perks` argument there
  would quietly hand a program the player's investment, which is the exact
  failure `CraftOrder` was built to make impossible.
- **Three perks are applied at purchase rather than read at a formula**, so
  they get the only shape they can. `purchase_stat_gain` says what buying
  `Attacker`, `Defender` or `Buffer` grants — including `Buffer`'s
  percentage-of-current-max and its floor — while `unlock_perk` stays the one
  writer of `Stats`.
- **The floors moved with their perks.** Power may be switched off entirely
  by enough levels of Low Power Mode, because a Recharger Node already
  deletes that pressure; Trace never may, because it is the Stack's only
  escalation and descending has to keep costing something; and a recipe line
  never reaches zero, which is the infinite-Credit fault the caravan's price
  floor guards from the other side. Each of those used to live at the call
  site, where it read as an implementation detail rather than as the design.
- **The module is a census, and it is enforced.**
  `every_perk_has_a_query_that_answers_what_it_is_worth` is exhaustive on
  `Perk`, so a nineteenth variant fails to compile until it has a home there,
  and each arm asserts the query actually *moves* — a perk wired to a zero
  constant fails as loudly as one with no query at all.

## 0.13.49

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
The two new save fields are additive behind `#[serde(default)]`, and the one
a pre-existing file cannot carry is answered by deriving it rather than
defaulting it — see below.

Every program on your base roster is now somebody in particular, and none of
them will tell you who.

A program is born with one of five **dispositions**, derived from its own id
and then kept. Nothing draws it and no screen names it. You find out the way
you find out about people: one body is always the first to wander off to the
Sandbox, another grinds on long after everyone else has gone; one lets a bad
week go and another is still sore about a machine that jammed on it a
thousand ticks ago.

It moves two things and nothing else — how fast a program runs its reserves
down, and how hard what it remembers lands on it. `Languid` and `Dogged` are
the poles of the first, `Amiable` and `Abrasive` of the second, and `Steady`
is neutral on both. There is no work-rate dial: a Languid program does not
mine any slower per tick, it is simply off being seen to more of the time,
which is what "doesn't want to finish tasks" actually looks like from the
outside.

On top of that, a program can now **have had enough**. Morale — the signed
sum of everything it remembers — has two lines under it. Cross the first and
the program sulks: it still works, but not at a machine it holds a grudge
against, and if it is already standing at one it walks off and a willing body
takes the post on the same tick. Cross the second and it downs tools
altogether, leaves the labour pool, and the work order screen's shortfall
grows by one with the reason sitting on its manifest.

It comes back when its grudges have faded and better days have landed on top
— there is no amenity to walk to for this one, and nothing you can build that
fixes it directly. The gap between going out and coming back is deliberate:
one number would have a body dropping its tools and picking them up again on
alternate ticks.

The two features are the same feature. An Abrasive program's grudges are
scaled up, so it reaches the line on strictly less history than an Amiable
one — which is how a hidden temperament becomes something you can actually
see, through *when* somebody breaks rather than through a label.

An existing save gains all of this. A file written before dispositions
carries no disposition, and the load path derives one from each program's id
through the same formula a fresh program takes — so your base comes back with
the roster it would always have had, not a crew of identical neutral bodies.

**Both morale thresholds are unmeasured.** Morale has no natural scale and
the balance simulator models no base production at all, so the two numbers
were chosen against the shipped memory valences rather than against a played
run. They are the first thing to revisit.

## 0.13.48

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32,
and this release adds no save field at all — the walk below is drawn from a
queue the engine drains every frame and never writes to disk.

A squad you dispatch from the Relay used to vanish from the map on the frame
it left. That is exactly correct as simulation and reads as a bug, so the
bodies now walk: out from wherever each was standing, across the base pocket,
to the anchor, and out of phase. The return is the same thing backwards — in
through the anchor and back to the tile each body left from. A squad's
members set off one after another rather than together, so four programs read
as a file leaving.

- The walk is **purely cosmetic**, and that is the load-bearing decision.
  Nothing in the simulation stands on any of those cells: `dispatch_sortie`
  and `return_sortie` each queue one `resources::TransitCue` per body — a
  glyph and the cells it walks through — and forget it in the same call.
  `Game::take_transits` drains the queue, `take_effects`' counterpart, and a
  frontend that ignores it draws the game exactly as it did before.
  `ProgramRole::Sortie`'s five omissions are untouched, which is the whole
  point: giving an away program a live `Position` for a few ticks is the
  "which space is this?" bug class that role was built to stay out of.
- Departure and arrival are the **same cue with its ends swapped**, so there
  is no direction field to get wrong.
- The walk follows the dug ground rather than the straight line home.
  `base_space::transit_path` pathfinds on `BaseGrid::walkable` alone and
  admits no blocked set — the door is the cell the Home stands on, so a walk
  that refused occupied tiles could never reach its own destination. A body
  ghosting past a machine for a fraction of a second is invisible; one
  ghosting through rock reads as a rendering fault.
- The draw sits behind the gate a raid's flash already sits behind. A cue
  names base-space cells, so one drawn while the pane is showing the zone
  surface would file a squad of glyphs across whatever unrelated ground
  shares those numbers, the party's own tile among them.
- A body standing somewhere that is not walkable base space gets no walk at
  all rather than a straight line — the ordinary state of a program adopted
  on the surface that has not drifted into the base yet.

A cue is drained on the frame it is queued, so a return walk you are not home
to watch is dropped rather than saved for later. That matches how a raid's
flash already behaves, and it is what keeps the whole feature free.

## 0.13.47

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this release adds two save fields, both additive, so a save written before it
loads with nobody away from the base.

The idle half of the roster gets a second front. A developed program you are
not fielding has done exactly one thing until now, which is work the base;
this is the engine for sending a squad of them out to fight somewhere instead.

**There is no screen for it yet.** Everything below is reachable from the
engine and covered by tests; the Relay's board and the dispatch picker are a
second piece of work. Nothing in a running game changes until those land.

### Added

- **Sorties.** A squad of base staff can be dispatched from a Dispatch Relay
  to a named site, where it fights a run of real battles off-screen over a
  number of ticks and comes home with what it could carry — or with fewer
  bodies than it left with. The fights use the same swing and routine
  resolution a fight in front of you does: the same hit ladder, the same
  damage bands, the same mitigation, the same Power costs.
- **A trip pays for itself four ways.** The bodies work no machine for the
  whole trip and the base reads as short-handed while they are gone;
  provisioning is charged from base stock at dispatch; the programs take real
  damage and can be lost; and a kill off-screen pays less than the same kill
  with you in the fight. Power does not recover in the field and there is no
  rest out there, so Specials taper across a long trip — which is what makes
  a twenty-fight site genuinely harder rather than merely longer.
- **The trip aborts on the first casualty.** Remaining battles are skipped,
  the survivors keep what they earned, and the return travel still runs —
  there is no teleport home. Under Forgiving the program that dropped comes
  back benched and walks itself to a Repair Bay; under Permadeath it is gone,
  but the abort caps the disaster at one.
- **`assets/sorties/` is a new content directory** — one `.ron` file per
  site, naming it, how dangerous it is relative to the sector you are in, and
  how many fights it takes. Four ship. Deleting the directory restores the
  pre-sortie game exactly, the same supported way deleting `assets/needs/`
  does. `assets/sorties/README.md` is the schema.
- **The Dispatch Relay and its research node**, both data. `StructureDef`
  gains `dispatches_sorties`, so a mod can add a second dispatch structure or
  move the gate without touching the engine.

### Changed

- **The Relay's board is derived, never stored**, the way a Broker's is: it
  is recomputed from the world seed, the sector and the clock, so reloading
  reproduces the same offers, there is nothing to re-roll, and it rotates on
  its own. Each offer's fight count is fixed the moment it appears, which is
  what lets the trip's length be quoted before you sign for it.

## 0.13.46

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this release changes what a battle sounds like and what the dig crew does when
it runs out of substrate, and stores nothing.

A swing you can hear the shape of, and a dig crew that stops standing over a
cell it has nothing to floor with.

### Added

- **Hits, crits and misses sound different**, and each cue plays as that
  swing's own line is revealed in the log rather than all at once the moment
  the round resolves — so what you hear tracks what you are reading. A fumble
  sounds like a miss, since the line already narrates how badly it went.

### Fixed

- **A dig crew that runs out of Blank Substrate no longer strands the
  program.** A body posted to floor a cut cell stayed on it for the rest of
  the run, waiting on a material only a Lathe makes — which that same body
  would have had to go and run. A floor job with nothing to lay is set aside
  until the substrate exists, and the program goes back to useful work.
  Cutting is unaffected: a marked wall still comes down with an empty store.
- **A base that runs dry twice now says so twice.** The crew named an empty
  substrate store once per cell and then never again, so a shortage that came
  back was silent.
- **A base whose last job just became unworkable stands its worker down.** The
  quiet-base check could pass while a program still held a posting that had
  stopped existing, leaving it parked there.

## 0.13.45

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this release changes what the game says when it refuses you and stores
nothing.

A rest that does not happen says why. `r` was reported dead in the Stack
again after it was bound there; it is not dead, but a rest that could not
happen accounted for itself nowhere the player was looking, which from
outside is the same event as a key that does nothing.

### Fixed

- **Every rest that is refused now says so**, on the status banner and in
  the log, on the surface and underground alike. Three of the four ways a
  rest could fail said nothing at all, and the fourth spoke through a log
  line the channel filter could hide.
- **An empty stack in the pack is no longer mistaken for a rest charge.** A
  slot that had been emptied still counted as something to power down with,
  so the refusal that should have named it was skipped and the rest failed
  silently one step later.
- **Every other refusal on the map and Stack screens reaches the log too.**
  Both screens were writing the banner directly, so a refused verb aged out
  of view after a few seconds and left nothing to scroll back to.

## 0.13.44

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this release changes what colours the map draws in and stores nothing.

The marks and washes drawn over the map's glyphs are drawn from the HUD's
palette too. The tail of the previous release, which swept the glyphs and
left the overlays on top of them alone.

### Changed

- **An Excavation plan is blue.** Its washes, the box being previewed and the
  ring the party wears with cutting tools armed were an amber close enough to
  the machine-attention yellow that a cell you had marked and a machine that
  had stalled read as the same news. A plan is you having acted; the yellow is
  the base asking you to.
- **A stranded machine's mark is the attention colour**, the same one its
  glyph wears, and a staffed machine's is the green a running one has. What
  separates the two marks is still the blink and the colour together.
- **A raid's flash is the same red the rest of the screen uses for harm.**
  Every cue about something being attacked now comes from one place, so a
  retune moves all of them at once.

## 0.13.43

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this release changes what colours the map draws in and stores nothing.

The map's glyphs are drawn from the HUD's palette. Phase 6, and the last of
the Paned Command HUD: the entity colours were the one part of the screen
still painted from the renderer's old table.

### Changed

- **Every entity on the map is drawn from the palette's sixteen entries.**
  The eleven colours species and structure files author resolve through one
  table, so a program's glyph is the same colour on the map and on every
  screen that shows it. Nothing about authoring a colour changed: a mod names
  the same eleven.
- **The player's `@` has a colour nothing else can take** — br cyan, the
  palette's player role, read off being the player rather than off the cyan
  the player happens to spawn with.
- **A stalled machine asks for attention in yellow instead of red.** A
  clogged, stranded or unpowered machine is the base asking you for
  something, and it now wears the same colour the status bar uses to say so.
  Red on this screen means hostility and inbound harm, and nothing else. A
  machine that is merely waiting — short of input, or with its program
  walking back — keeps the dimmer yellow it had, because "go and fix this"
  and "this is fine, give it a moment" are the whole information in that
  colour.
- **The danger ladder climbs in hue.** A creature's glyph still reads green
  through red by how badly it would beat you, with the middle rungs moved so
  that no two of them read as the same colour under the map's dimming.

## 0.13.42

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this release draws the right-hand column differently and stores nothing new.

The column's three tabs get real contents, and the old status panel comes
out. Phase 5 of the Paned Command HUD, and the end of the ~250-line
undifferentiated text dump the whole design was aimed at.

### Added

- **BASE, CREW and PACK have bodies of their own.** BASE lists the base's
  structures with the program on each and what it is holding, then
  production, defence, the build queue and the programs the roster has
  spare; CREW lists the roster against its capacity and then every program
  with its level, Integrity and what it is doing, party members first,
  followed by the routines currently running; PACK lists what the party is
  carrying.
- **What does not fit is counted.** The column does not scroll, so a pane
  that runs past its body says `+N more` on the last row rather than
  drawing off the bottom edge in silence. The notice is itself a row and
  is budgeted for, so it cannot be the thing that overflows.
- **A collapsed tab says what its pane holds** — the base's structure
  count, the crew's headcount, the units in the pack — instead of
  `nominal`. A condition still outranks the summary, and both are built
  from the same data as the open pane's rows, so a bar and the pane it
  stands for cannot disagree.

### Changed

- **The four stats, the bars, the zone and the stock strip are no longer
  drawn twice.** The vitals strip has carried them since phase 2 and the
  status bar since phase 1, so what the old panel drew was a second copy.
  The rows that had nowhere else to be — the roster, the running routines
  and the pack — are what the three panes are.
- **A companion's buff row names its holder on its own line** in the
  column, as it already did, and the battle screen keeps naming it inline.
  The column is a fixed slice of the window and cannot widen for a tag.

### Removed

- **`draw_status_panel`.** The column it drew is now the info column's open
  tab, and every row it held is either in one of the three panes or was
  already on the status bar or the vitals strip.

## 0.13.41

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
which pane of the info column is open is view state, exactly as the log's
filter is, and nothing here is written to a save.

The info column wears three tabs and hides nothing behind them. Phase 4 of
the Paned Command HUD, and the one it is really about: a single derivation
answers "what needs the player right now", and every surface that could
report it reads that same answer.

### Added

- **A status-bar badge saying what needs you** — the most urgent condition,
  upper-cased, with the key that opens the screen it is acted on from. When
  nothing holds it reads `ALL NOMINAL` in green, because the calm state is a
  real state and is drawn rather than left as a gap.
- **The info column is tabbed: `1` BASE, `2` CREW, `3` PACK.** One pane is
  open and the other two collapse to a summary row each. The keys work
  underground as well as on the surface.
- **A closed pane can never hide something you need to act on.** Its tab
  wears a `!` and its collapsed row says what the condition is, in yellow
  for work and red for harm. The badge, the tab marks and the collapsed
  rows are three readouts of one derivation, so they cannot disagree about
  what is going on.
- **Four conditions are reported**: a structure below full Integrity, nodes
  standing without a program, unspent Perk Points, and a roster at capacity.
  A damaged structure sorts above the rest, since the badge shows only the
  leading one.

### Changed

- **The status column is now the open tab's body**, drawn inside the
  column's frame rather than drawing its own. What it holds has not moved
  yet — that is the next phase — so it reads as it did, in a slightly
  shorter box.

## 0.13.40

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this release draws the same screen differently and stores nothing new.

The log pane wears its filters and its keys on its borders. Phase 3 of the
Paned Command HUD: two rows come off the pane's body and onto its frame, and
a channel gutter goes down its left edge.

### Added

- **A channel gutter down the log.** Every line is tagged with where its news
  came from — `FIELD` or `BASE`, the two the `f` key already cycles — with a
  pickup marked `GAIN` and an inbound sweep marked `ALERT`, because those two
  are their own news whatever channel they arrived on.

### Changed

- **The log's filter row moved onto the pane's top border**, where it costs
  no body row. That is one more line of log at every window size, and the row
  now names `L history` alongside `f cycle`.
- **The four-line block of eighteen keys under the status column is one row
  on the log pane's bottom border.** It is ordered by priority and measured:
  what does not fit is dropped from the end rather than drawn off the panel
  in silence. Movement, `b`, `i`, `? help` and `q menu` are never dropped,
  `? help` in particular being where every key the bar had to cut still
  lives — all of them are in the manual's Controls page. `t trade` and
  `s save` do not fit at any supported window size and stay cut, as the
  design handoff had them.

### Fixed

- **A log line can no longer draw through the keybar.** With the filter row
  off the body the log runs a row deeper, toward the border the keys are
  mounted on, and the keybar paints last — so a collision would have shown as
  keys sitting on top of a half-covered line rather than as anything a reader
  would look for.

## 0.13.39

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
the run's spent freebies are a new save field behind a `#[serde(default)]`,
so a save written before this loads with its free Broker still owed.

The run's first Contract Broker costs nothing. Contracts are what onboards a
run — the Broker has always been behind no research for that reason — and
its five Core Fragments were the last thing standing between a new player
and the board.

### Added

- **A Contract Broker is free to deploy until you have one.** The build menu
  quotes it as `free` and the deploy prompt says `Free to deploy`. The
  waiver is spent when the crew actually raises it, so filing a request and
  then cancelling does not burn it, and a second request filed alongside the
  free one is quoted in full.
- **Structures can be authored free-the-first-time**, a `first_free` flag in
  `assets/structures/*.ron`. One per run rather than one per zone: the base
  travels through a breach and so does whatever is standing in it. The
  shipped Broker is the only structure that sets it.

### Changed

- **A structure removed for a bill nobody paid still refunds the usual
  share** of what its file says it costs — one Core Fragment, once, for a
  demolished free Broker.

## 0.13.38

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
Nothing here is stored — this is what the HUD draws, not what it keeps.

The map pane is framed, and it wears the player's vitals on its own borders
rather than spending map rows on them.

### Added

- **`SECTOR MAP` and a threat readout ride the map pane's top border.** The
  threat line says how many hostiles are on the map and whether anything in
  the base is contributing raid defence.
- **The vitals strip rides its bottom border** — Integrity, Power, level and
  XP as meters, then unspent perk points, mitigation, attack, strength,
  decompiler and whether mining is armed. On a narrow window the strip drops
  from the end rather than running off the pane, so the meters are always the
  part that survives.
- **Unspent perk points are called out in the attention colour**, and the row
  is absent entirely when there are none to spend.

### Changed

- **The frame is the same underground.** The Stack corridor draws into the
  same framed pane and carries the same vitals; only the map's contents
  change.

### Known rough edge

- Integrity and Power are drawn twice for now — once on the new vitals strip
  and once in the old right-hand panel. That panel is replaced outright in a
  coming release and the duplication goes with it.

## 0.13.37

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
Nothing here is stored: the HUD work is layout and colour, and the two fixes
below change what is drawn rather than what is kept.

Groundwork for a redesigned HUD, plus two base-space drawing fixes that had
not yet been released.

### Added

- **The status bar.** One row across the top of every screen that draws the
  world behind it, carrying who and where you are — identity, zone, position
  and tick — alongside what the base is holding. It absorbs the stock strip
  rather than sitting above it, so the readout costs no extra row.

### Changed

- **The HUD's geometry is derived in one place.** The map pane, the log pane
  and the right-hand column are no longer three independent fractions of the
  window. They come from one calculation, which is what lets the column run
  to the bottom edge and the log stop at the column's left edge instead of
  passing underneath it.
- **The right-hand column is narrower and full height.** Its contents are
  unchanged for now and will look cramped; the panes that belong in it land
  over the next few releases.

### Fixed

- **A builder and a digger are drawn while they work.** A tamed program
  holding a build or dig post disappeared off the map for the whole job, so
  filing a build request made a program vanish and a structure appear a few
  hundred ticks later with nothing having visibly walked, fetched or built.
  Marking a wall did the same to whoever went to cut it. The "somebody is on
  this job" mark was missing from both ends of a build posting, and is back.
- **The build caret bounces in the middle of its slab** instead of sitting
  high in the tile. It was riding the staffed mark's upward-only curve, whose
  rest position is an inset off the tile's bottom edge rather than the centre
  of a slab.

## 0.13.36

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
the one field this adds to a creature is additive behind `#[serde(default)]`,
and nothing else in either feature is stored at all. A save written before
this comes up with every program upright, and a program already developed
past its zone's new cap keeps every level and every stat it earned.

Two changes that meet in the middle. Losing a program in a fight is no longer
losing it, and how far anyone can develop is now a property of how far you
have got rather than of what you are.

### Added

- **A program that dies under Forgiving is benched, not destroyed.** It comes
  out of your party downed, keeps its place on the roster, and walks itself to
  a **Repair Bay** — a new passive building that writes it back to full
  Integrity a tick at a time and stands it off the bench when it is whole. No
  worker, no input, no research: 16 salvage and somewhere to put it.
- **Without a Bay standing, a downed program stays down.** It lies where it
  fell, off the labour pool, still holding its roster slot. Selling it or
  extracting a routine are what free the slot in the meantime, and the refusal
  says so rather than leaving you to guess.
- **XP earned at the level cap buys Perk Points** instead of vanishing. The
  price rises with every perk you already hold, so the exchange tapers rather
  than becoming a second grind; whatever is left unspent turns into real
  levels the moment a breach lifts the cap.

### Changed

- **One level cap over the whole party, and the zone sets it.** The player had
  no ceiling at all and a companion had a personal one; both now stop at the
  same number, which rises with every sector breached. A companion can stand
  level with you, which is what makes developing one worth the XP.
- **A Kernel Ring buys talent tiers rather than levels.** Every ring still
  opens two, three rings still buy exactly one full tree, and a program you
  built before this has precisely the points it had. What changed is what the
  ring is *for*: depth in one program's tree, not permission to be bigger than
  its roster-mates.
- **Who is in your party is decided at home.** Joining and standing down both
  need base space now. Wielding a program in the field is untouched.
- **A raid that kills a defender benches it too**, on the same terms as a
  fight.

## 0.13.35

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
both fields this adds to a creature are additive behind `#[serde(default)]`,
and a program in an older file simply comes up with full reserves.

Your programs want things now. Not much, and never as a new way to lose — a
body that has been at it too long walks off, sees to itself, and comes back.

### Added

- **Programs on the base staff carry reserves that fall on their own**, and
  faster while they work. Two ship: **Coherence**, which a process spends by
  running, and **Slack**, which it spends by running the same thing over and
  over. Both are `.ron` files in `assets/needs/`, so a third costs two files
  and no Rust — and deleting the directory restores the game exactly as it
  was.
- **Two buildings answer them.** The **Defrag Bay** puts a program back in
  order; the **Sandbox** is scratch memory nobody is watching. Neither takes
  a worker or an input, and a program whose reserve has run critical walks to
  one on its own, stands there until it is whole, and goes back to work.
- **The manifest's WORK box says where a program stands**, in words rather
  than a number — steady, fraying, strained, critical — with what it is off
  doing beside the need it is doing it for. The roster's activity line says
  the same thing instead of calling a program on an errand "idle".
- **Programs that idle together think better of each other.** A new
  `idled_with` memory, written once when a program finishes at an amenity,
  naming whoever else was there.
- **A program with nowhere to go says so, once, and holds it against the
  corner it was standing in** — the new `frayed_here` memory. Nothing in the
  base servicing a need and nothing being able to *reach* what does are
  different complaints, and the base tells you so in different sentences.

### Changed

- **A program off shift is not counted as a body the base has.** The work
  order screen's shortfall grows while somebody is seeing to itself, which is
  the reading you want: the base is short of hands, and the manifest says
  why. A body already carrying a load finishes the delivery first.
- **A run-down program extracts less reliably.** One capped term in the same
  roll morale already rides, and it reaches extraction only. A program with
  what it needs contributes exactly nothing, so the shipped rates are
  untouched.
- **The manifest's MOVES box trims at three rows instead of four**, to pay
  for the need rows. Nothing shipped has more than two moves.

## 0.13.34

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
a build request is a new entity in the save and every field it added is
additive behind `#[serde(default)]`, which is exactly the case field-named
RON was adopted to make free.

Building is something the base does, not something you do. A deploy is a
request your crew fetches for and raises, and so is an upgrade.

### Added

- **Every structure but the Home is now filed rather than built.**
  `Game::place_structure` answers every refusal it always did — researched,
  in base space, standing on laid floor, cell free, under `max_deployed` —
  and then spawns a `components::BuildSite` on the cell carrying the resolved
  bill of materials. `schedule_base_labour` posts a body to it ahead of every
  work order, and that body walks to whichever shelf or pack holds what the
  bill calls for, carries `HAUL_CARRY_CAPACITY` at a time, sets the load down
  on the site and raises the structure over `BUILD_TICKS_PER_MATERIAL` ticks
  per unit of material.
- **The Home stays a player verb**, and that is not a special case to be
  tidied away later: founding is the one build with nobody to ask, since base
  space does not exist before a Home stands — no roster inside it, no shelf to
  fetch from, and `require_base` refusing entry for want of the Home you are
  building.
- **Nothing is charged at filing.** A request the base cannot afford yet is a
  legitimate thing to file — production catches up, and the crew starts the
  moment the last unit exists — so the old shortfall *refusal* at the menu
  became a shortfall *report* from the builder standing at the site, said once
  per drought rather than once ever.
- **`d` plus a direction calls off the request on that tile** when no
  structure stands there: the same gesture as demolishing, because it is the
  same question the player is asking of that cell. No confirmation step —
  nothing is destroyed, and the units already carried there go straight back
  onto a shelf.
- **The map paints a site as a dark slab with a bouncing caret**, and examine
  says what is going up, what is still to be fetched and who is on it.
  `views::BuildOrderRow` is the one derivation all three read, so the map, the
  examine line and the order list cannot report a percentage the crew
  disagrees with.
- **The build menu's `(have/need)` column counts the base's shelves as well
  as the pack**, since both are stores a builder fetches from.

### Changed

- **Upgrading a structure is a build request too.** It was the last structure
  cost paid out of the player's own pack — the complaint arrives the moment
  you stand on a full Depot beside a Mk1 Lathe and are told "Not enough Cache
  Grain", with the stock strip along the top of that very screen saying the
  base is holding it. `Game::upgrade_structure` keeps every refusal it had in
  the same order, drops the shortfall check and the charge, and files a site
  on the machine's own tile; the crew fetches the bill and the tier lands when
  the work is done. The upgrade menu now quotes the pack **and** the base's
  shelves, or it would price the job against a store the verb no longer reads.
- **The machine keeps running while its upgrade stands.** Standing it down for
  its own upgrade brings back the deadlock closed below, on a base that files
  three at once — the machines making the materials the requests are waiting
  on are the ones switched off. The site carries no glyph for the same reason:
  the machine is still there and still drawing that cell, so a build frame
  over it would be a lie about the tile.
- **One component covers both jobs.** `BuildSite::goal` is `New` or
  `Upgrade { to_tier }`, and exactly one step branches on it — completion.
  The crew, the walk, the scheduler wants, both announcement latches, the
  reachability check and the refund on cancel are shared. A site names a
  **tile** and never an entity, so a machine destroyed underneath its own
  upgrade leaves nothing dangling.
- **Materials are not spent until the structure is raised.** They leave their
  shelf when a builder picks them up and stand on the cell until the job
  finishes, which is what makes calling a request off a refund of goods that
  still exist rather than a rebate.

### Fixed

- **A request the base could not supply deadlocked it.** Build wants outrank
  production, so a one-program base posted its only body to a site with
  nothing to fetch, the body stood there, and the Mining Node that would make
  the very material the site was waiting for was never worked again. The crew
  said "nothing to raise it with" once and the base was finished for the run
  — reached by a player doing the supported thing, since filing a request the
  base cannot afford is the whole reason filing charges nothing. A dry site
  now drops out of the want list until a next unit exists; the flicker that
  admits is the behaviour you want, mine a unit, carry it over, go back to
  mining.
- **An unreachable request starved the base silently.** A walled-in site
  posted a body, lost it in the same tick when the walk failed, and was handed
  the same body again on the next — forever, while the production want the
  truncation cut to make room for it went unfilled and nothing was logged. The
  reachability check now sits *above* the cut, asks the staff, and says so
  once.
- **The dry report is said once per drought, not once per request.** The latch
  was documented as clearing when a source appears and never did, which for a
  build — waiting on a bill of several items over many trips — leaves a base
  that ran out early silent about running out later.
- **A pending upgrade no longer eats one of its kind's deployment slots**, and
  a machine destroyed by a sweep or demolished by the player takes its pending
  request with it, handing back whatever had already been carried there.

## 0.13.33

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

The base stock strip lists banked pools, and a machine holds its row at zero.

### Fixed

- **The stock strip never drew the base's banked pool.** `Game::base_stock`
  walked output buffers alone, and a banked item never reaches one —
  `deliver_payout` sends a Research Node's yield straight past the node's own
  `output` into the player's bank, which is the whole of what `ItemDef::banked`
  buys. So Research Data, the only banked product in the game, had no row on
  the strip at all, and `research_data.ron` had been carrying an `abbrev` of
  `R` for the strip's benefit the entire time it could not be drawn. It is
  folded in **by the flag and never by name**, and `stock::output_buffers` is
  deliberately *not* widened to reach it — a work order for a banked item is
  still refused on the grounds that no shelf holds it.

### Changed

- **A pile keeps its row while its buffer is empty.** The strip used to list
  only what the base was holding right now, so a tag appeared and vanished as
  haulers cleared shelves — the same reshuffling under the reader's eye that
  sorting by quantity would cause, on a readout whose whole job is being
  glanceable. A row now exists if the base holds any of an item **or** is set
  up to make it: `stock::producible` seeds a zero for each deployed
  structure's `work.produces` and its `assembles.item`. Both halves, because
  an assembler declares no `work` block at all, so a `produces`-only rule
  would leave every crafting machine in the base off the strip until its
  first unit landed.
- **Deliberately narrower than "any structure", and narrower than the recipe
  list.** A Depot makes nothing, and seeding off what a building could *hold*
  would put a row on a one-row readout for every item in the game. A
  researched bench recipe is compiled into the *player's* pack and never into
  a base buffer, so a row for one would be a zero that could never move — on
  the shipped tree that is invisible, since all six researched recipes name
  equipment and `ItemDef::category` already filters those off the strip, which
  is exactly why it is written down. A banked pool the player has none of
  takes the same rule applied to the one item with no buffer to stand in for
  it: not seeded, or every run would open on a row for a resource nothing in
  the base makes yet.

## 0.13.32

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

Idle staff wander the base, and armed cutting tools say so.

### Changed

- **An idle program drifts around the base instead of orbiting the Home.**
  It used to be walked onto a fixed Chebyshev ring at a set distance from the
  Home, a tile every six ticks; `wander_step` now offers it one of the eight
  neighbours of the tile it is *standing on*, or a hold, on the same cadence.
  Relative rather than absolute is the whole difference — a program the
  scheduler has just freed strolls away from the post it left, rather than
  snapping to a tile computed from its index.
- **Laid floor is the leash, and there is no radius to tune.** Entropy
  reverts a mined cell nobody is standing on, so a wanderer that strolled
  into a fresh corridor could be sealed in behind it — unpostable for the
  rest of the run. Floor never reverts, so the paving the base has actually
  laid is exactly how far a body may roam. It still never steps onto a tile
  another idle body holds, or onto the party's own cell.
- The walk stays a pure, RNG-free function of its arguments, folded a byte at
  a time so the step counter reaches the high bits the reducer reads — folded
  whole, every program drifts in the same straight line forever.

### Added

- **The party's tile wears a ring while cutting tools are armed.** `n` arms
  the player's own bump into base-space rock, and the only trace of it was
  the log line at the moment it was toggled: walk away, come back, and there
  was nothing left to read the mode off. The ring is the excavation plan's
  yellow, because a mark and a swing are the same job.
- A ring rather than a colour on the `@` itself, so the sprite that stands in
  for that glyph carries the cue too. It is gated on base space and not on
  the flag alone — nothing disarms the tools on the way back out through the
  anchor, and out on the zone map there is no rock to cut.
- The controls page had never listed `n` at all. It does now, beside `m`.

## 0.13.31

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

F3 shows what a frame actually costs.

### Added

- **A frame-timing readout, on `F3`.** Four figures in the bottom-right over
  the last completed second: the frame count, the mean, the worst frame, and
  the draw pass. The renderer's shape-building pass has had a measured
  baseline since the debug-profile work, but the rest of a frame — bevy's
  schedule, egui's tessellator, the upload and the GPU — had never been
  measured at all. That gap is why the map going jerky read as an animation
  bug for months: the camera code was intact, there were simply no frames to
  draw it in.
- **The peak is the figure that earns its place.** Sixty frames with one at
  90 ms still averages under 18, so a mean alone reports a stuttering game as
  a smooth one. The draw figure is the same pass the bench measures, so the
  two are directly comparable, and the gap between it and the mean is the
  part nobody has seen.
- **The meter is fed every frame whether the readout is on or not**, or the
  first second after pressing `F3` reads as a cold start every time. A
  function key rather than a letter for the reason backslash is, and one
  better: letters reach the game as typed text, and `F3` produces none on any
  layout.
- No new `Painter` operation — `rect`, `ui` and `measure_ui` already existed,
  so the drawing seam is untouched.

## 0.13.30

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

Four more ways to hit the whole field.

### Added

- **Seven new routines, all data.** The Everyone tier — the top of the scope
  ladder, where a routine reaches every hostile on the field at once — had
  ten entries and already covered the whole effect vocabulary, so what was
  missing was variety inside it rather than a mechanic. No engine or schema
  change: every one of these is a `.ron` file in `assets/abilities/`, and a
  mod can add an eighth the same way.
- **Skim Everyone** tops off the one family that had a Single and a Group
  rung and nothing above them. It lands exactly on the Everyone tier's floor
  on both axes — 15 Power, four rounds — which makes it the cheapest way in
  the game to touch the whole field, and is the whole of Skim's identity as
  Leech's cheap sibling at every rung.
- **Segfault grows a Group and an Everyone rung.** Its Single rungs sat
  within a point of Packet Shred's on every axis, so the promotion needed an
  identity of its own: the band widens with the scope. Segfault Everyone
  rolls 16–40 against Packet Shred Everyone's 19–31 — a better top end and a
  worse floor — paid for with a fifth round of cooldown, three more Power and
  all but one point of aim.
- **Row Hammer** is a new family, and it is built on the same axis from the
  other end: low damage, high accuracy, at all three rungs. A whiff across
  the whole field is the biggest single roll a fight ever asks for, so this
  is the answer to Packet Shred Everyone being a gamble — chip everything,
  every time.
- **Snoop Everyone** is exclusive, off the Overseer at 0.30. A field-wide
  drain at a rate the ordinary tier may not have: `cycle_harvest` is capped
  low precisely so that reaching wider stays a trade rather than a straight
  upgrade, and behind a boss that stops being a concern, because there is no
  cheaper rung of it to undercut.

### Changed

- **The hunt-only pool widens from 28 routines to 34.** The six new ordinary
  rungs are found on wild carriers, the path five of the ten existing
  Everyone routines already take. That takes the pool's authored weight from
  185 to 212, so each routine already in it now turns up about an eighth less
  often. The census pinning that count carries the figure and the reasoning
  now, since the pool's size is a design decision rather than an accident.

## 0.13.29

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

The Transfer picker's figures line up.

### Changed

- **The Transfer screen (`c`) draws its quantities in a column.** Each row
  carries the signed amount and both live ceilings, and those figures were
  staggering with the item names above them — `suffix_x` places a suffix one
  inset past the *advance* of the row's own label, so a short name pulled its
  figures left and a long one pushed them right. The screen read as a list
  where it is really a table.

  The name is now padded out to the widest one on screen and each figure is
  right-aligned in its own width. The UI face is monospace and a trailing
  space advances exactly as a glyph does, so equal-width labels put every
  suffix at the same x — `draw_row` and `popup.rs` are untouched, and the
  figures keep the dim annotation colour a suffix is drawn in.

  Widths are measured from the rows actually listed rather than fixed, so a
  shelf of short names draws a narrow table instead of a wide one full of
  empty space. A name longer than the column is not truncated: it pushes its
  own figures right and leaves the rest of the table alone, since losing
  characters off an item's name to keep a column straight is the worse of the
  two failures.

## 0.13.28

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

The map can draw a sprite where a glyph was.

### Added

- **A one-cell sprite may stand in for an entity's map glyph.** Drop a
  16x16 RGBA PNG into `assets/sprites/` and the map draws it in the cell
  the glyph would have occupied. This release ships the pipeline and one
  placeholder — the player's — rather than a set of art; per-species and
  per-structure sprites are a later change.

  **The size is not arbitrary.** Map glyphs are drawn at `16 x zoom` with
  zoom clamped to 1..4, so a 16px source lands on exactly 16, 32, 48 or
  64px — whole multiples, sampled nearest-neighbour, which is the same
  contract `unscii-16` is already held to and the reason the font
  rasterization test asserts zero antialiased pixels at each step. A
  sprite authored at any other size still draws; it just blurs at some
  zoom, silently and only on screen. A census refuses one.

  **A sprite substitutes for the glyph rather than drawing over it.** The
  overdraw bug looks pixel-perfect against opaque art and breaks the
  moment a sprite has any transparency, at which point the old character
  shows through — so the test asserts both the sprite drawn *and* the
  `@` absent.

  **Colour is a multiplying tint**, so art authored near-white inherits
  everything that already colours a glyph — the difficulty read, the boss
  and nemesis overrides, the biome tint, the damage dimming — with no
  second mechanism and no change to any of them. Art carrying its own hue
  fights all four.

  **The directory is optional by construction.** A missing directory, a
  missing file, or a name nothing is authored for all end at the glyph, so
  deleting `assets/sprites/` restores the previous map exactly — the same
  supported way deleting `assets/environment/` restores the pre-effects
  game. That is what will let a modded species ship without art rather
  than ship invisible.

### Fixed

- **The asset server now reads the path the launcher resolved**, instead of
  guessing one of its own. Bevy resolves its asset root against the build
  machine's manifest directory in a dev build and the executable's
  directory once installed; left alone it would have been a second site
  deciding a runtime path, which works where it was built and nowhere
  else. Nothing player-visible today, since nothing was loading an asset
  through it before this release.

- **`crates/engine/src/lib.rs` no longer carries an unresolved `git stash
  pop`.** 5,056 lines of a stale inline test block, conflict markers
  included, had been committed on top of the one-line module declaration;
  the workspace did not compile. Resolved to the upstream side.

## 0.13.27

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

Gear says what it is worth, and the wagon is one basket.

### Added

- **Every piece of gear carries one combat rating.** Six stat axes and no
  scalar meant "is this better?" was a question the game never answered.
  `Game::copy_power` answers it in one figure, in a fixed-width column that
  runs down every list naming an item — cargo, a trader's shelf, a recipe's
  result, the equipment panel, the Stack market, the wagon.

  **It is absolute, not measured for whoever is holding it**, so one number
  means one thing on all six screens. Every copy is priced against a single
  reference wearer in `tuning.rs`, and that wearer is *derived* rather than
  invented: the zone is the midpoint of the range `balance_sim` sweeps, the
  level is what its geared sweep reports as the minimum to clear that zone,
  and the stats are `stats_after_levels` of `PLAYER_BASE_STATS` at that
  level. A reference far from where players actually stand would make every
  figure in the game wrong in the same direction.

  Four terms, and none of them restates a formula that already exists.
  Attack and mitigation go through `Stats::power`, which already prices
  mitigation as the effective HP it buys rather than summing a percentage
  into a total. The damage band is a **difference** against the band it
  replaces, because a weapon *overrides* the natural attack — so a weapon
  worse than bare fists rates negative, which is the whole reason the term
  is not a sum. Accuracy and evasion are **proportional**, priced through
  `battle::hit_chance` as the fraction they move the throughput they act on:
  a probability is not a quantity. A Decompiler module buys taming rather
  than combat and gets no term at all.

  So there are three cells and three meanings, and they do not overlap: a
  figure is a rating, an em dash is *no answer* (a module with no combat
  axis, a consumable), and a blank is a row that is not an item — a Routine
  Disk on the wagon, an empty slot. A dash on one of those would claim the
  disk had been rated and found wanting.

  The **swap picker's delta may disagree with the column, and that is
  correct**: gear locks in the level it was equipped at, so a worn piece and
  a candidate are scaled at two different levels. The column is a property
  of the copy; the delta is a property of the swap.

- **The inspect page breaks the rating down.** `[I]` on any piece now says
  what the figure came off — offense, survivability, accuracy, evasion —
  with the axes that contributed nothing left out. One line rather than one
  per axis, because that page has no scroll and had no rows to spare; it
  paid for the line out of the affix block, which already had a cap and
  degrades by counting what it cannot draw.

### Changed

- **The visiting caravan is one basket, committed by Enter.** Buying was a
  row at a time and selling opened a per-item quantity page, so a visit that
  cleared a stack of Core Fragments and picked up two things was a dozen
  keypresses and a dozen turns.

  Every row now carries an amount, edited with the arrows — Shift jumps to
  the end of a row, Ctrl halves the gap, `[A]` fills your cargo rows, `[N]`
  clears — and Enter commits the lot. A header line says what the basket
  leaves in your purse, so six rows are no longer set blind.
  `Mode::CaravanQuantity` is gone.

  **The commit sells before it buys**, which is what lets a basket be funded
  by its own sales — the entire reason the two sections are one basket
  rather than two screens. And **every refusal lands before anything is
  spent**: a caravan has no buyback, so a half-committed basket is the one
  bug a player cannot undo. The whole visit costs **one turn**, not one per
  line.

  On this screen Right increases and Left decreases. The transfer picker's
  inverted arrows are specified for a single row spanning both directions;
  here the sign is fixed by which section a row is in, so inverting would
  read as a slip.

- **The wagon's two lists are grouped by category.** The offers came off the
  roll shuffled by construction — a weapon, a program, a second weapon — and
  a deep shelf read as a heap. Both lists now run in one order under their
  own headings. The grouping is a property of the *view*: the shelf itself
  stays in deal order, because which equipment slot a wagon leads with
  rotates per visit and sorting the shelf would open every wagon with a
  weapon.

## 0.13.26

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

A companion can reach into the pack mid-fight.

### Fixed

- **A companion can spend its round on a consumable.** The `[u]se item` row
  was offered only to slot 0, and `Game::consume_item` hardcoded the player as
  the recipient — so a companion that had run its Power reserve dry had no way
  to refill it, while the pack sat full of Power Cells. That matters because a
  companion's Special is charged to its *own* reserve
  (`spend_power(entity, ..)`), not to the player's.

  The item row is now offered to every party slot, and `consume_item` takes a
  recipient. **The pack stays the player's and only the effect moves**:
  `Inventory` lives on the player alone and is the party's one shared kit, so
  a companion draws from the same stack the player would, but the Power
  restore, the heal, any armed field buff and the log line all land on
  whoever spent the round taking it. The reserve and stat writes became
  no-ops rather than unwraps, matching `spend_power`'s asymmetry.

  There is no ally-targeting picker: the player cannot hand a cell to a
  companion, the companion spends its own round on one. Both frontends were
  untouched — the picker already resolved against `battle_active_slot`, and
  its prompt already read "It costs this member their round."

## 0.13.25

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

Routines miss less, and accuracy is something you can buy.

### Fixed

- **An even matchup no longer means half your swings whiff.**
  `battle::hit_chance` returned exactly 0.5 for two identical combatants, and
  that was the whole of "routines miss too often". Measured against the
  shipped roster with the real functions, a player carrying no accuracy gear
  sat between 0.44 and 0.64 for the first ten levels — and both apex species,
  the ones that guard a Stack lair, are the fastest things in the game and so
  the hardest to hit at every level.

  A routine and a basic attack have always shared one resolution path, so the
  rate was never routine-specific. What is specific is the *cost*: a basic
  attack shrugs a miss off, while a routine has already spent its Power and
  armed its cooldown by the time the roll happens. Thirteen of the twenty-five
  damaging routines are multi-target with an independent roll for each
  recipient, so a single sweep at 55% printed two or three "goes wide" lines
  in a row.

  The attacker's Accuracy is now multiplied by `ATTACKER_ACCURACY_ADVANTAGE`
  before the ratio, putting an even matchup at 0.583. A multiplier rather than
  a flat bonus, because the ratio form is scale-free and has to stay that way
  — a flat `+n` would wash out as levels grow. It is necessarily symmetric,
  since the function takes two numbers and cannot know which side is the
  player; hostiles come off the hit-chance floor a high-level player had
  pinned them to.

- **An affix that paid only Accuracy, only Evasion or only a damage band was
  refused at load as granting nothing.** Both emptiness checks in
  `AffixDef::fault` enumerated three of `EquipmentStats`' six fields, so the
  accuracy axis could only ever ride along on an ATK affix — which is part of
  why it stayed on three weapons for as long as it did. Both now destructure
  the whole struct, so a seventh stat is a compile error rather than a field
  silently uncounted.

### Added

- **Routines are aimed.** A new `accuracy` key on an ability file adds flat
  Accuracy to the roll that routine makes, and to nothing else. The shipped
  roster grades it by how narrow the routine is — 6 for a single target, 4
  for a whole group, 2 for every hostile on the field — so a sweep trades
  odds for reach rather than being strictly better than an aimed shot. Flat
  and unscaled on purpose: a hostile's Evasion grows with the zone while
  yours grows with your level, so aim is a problem early and solves itself
  late.

- **Target Lock**, an eighteenth perk. +2 Accuracy per level on every attack
  you make, for 3 Perk Points, under the Combat heading.

- **An Accuracy talent node**, the fifth kind a tree may offer, one per class
  ladder and one in the generic tree. Companions get the axis through their
  tree exactly as the player gets it through a perk, and the two never stack.

- **Two accuracy affixes**, `Zeroed` (Weapon) and `of Direct Access` (Weapon
  and Module) — the latter the only one reaching a Module, so the axis is
  buyable by a program already carrying the weapon it wants.

### Changed

- **Eleven of thirteen weapons now author Accuracy**, up from three. The two
  heaviest author none, so the axis is a trade rather than a free line on
  everything.

- **A zone-1 group is no longer a single program.** `zone_group_cap(1)` was 1,
  which made the balance suite's zone-1 fixture a five-against-one fight
  rather than the body ratio the rest of the curve is about.
  `ZONE_ONE_GROUP_CAP` is a floor under the curve, so zone 1 lifts and no
  later zone's step moves. It also ends the Trace group lever's zone-1
  inertness, which was always a consequence of the old cap rather than an
  intent.

### Documentation

- `docs/seams.md` records why the parity baseline is no longer 0.5, and gains
  an entry for the two accuracy doors — what an *entity* brings to every
  swing against what one *invocation* brings.
- `docs/items.md` had never shown Accuracy or Evasion at all: `shiv_routine`
  read "atk+1" while carrying +3 Accuracy. Ten rows corrected.

## 0.13.24

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

Routines roll a band, and you run them.

### Changed

- **Every routine that moves Integrity now rolls a range instead of a fixed
  figure.** The mechanism had been in the engine for several releases —
  damage bands, the scaling that widens them with level and affinity, eight
  paragraphs of schema documentation — and not one of the 77 shipped
  abilities ever used it, so every Special dealt exactly the same number
  every time. All 34 damage, drain and healing routines now author a spread
  of about a quarter of their power, which is the ratio the roster's basic
  attacks have used all along. `Heal` gained the range it never had.

  The band is **centred on the old number**, so nothing got stronger or
  weaker: the average of every routine in the game is exactly what it was,
  and the balance suite passed the change without a single curve moving.
  What changed is that a big hit and a small one are now different rolls
  rather than the same roll twice.

- **The word "cast" is gone.** You *run* a routine, or invoke one; the thing
  you did is an *invocation*. "Cast" and "spell" are fantasy words and this
  game has none, so they are out of the manual, the ability descriptions,
  the perk pages and the log lines — and, unlike the 2026-08-05 Raid rename
  which stopped at the player-facing half, out of the code as well. No asset
  field was ever named `cast`, so nothing a mod author writes had to change.

  Ability descriptions that quoted a number now quote the band: Segfault
  Single v3.0 reads "Damage 13–21 to one target" where it read "Damage 17".

### Fixed

- The ability catalogue in `docs/abilities.md` had been six abilities short
  of the game for at least a release — Clock Skew, Core Dump, Hot Spare,
  Interrupt, Parity and Quarantine were never listed, while the page's own
  prose counted them. It also described two passives where eight ship. Both
  are corrected, and the page now shows what each routine rolls rather than
  its centre.

## 0.13.23

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

One key moves cargo both ways.

### Changed

- **Taking and putting are one screen.** `c` beside a machine or a Depot now
  opens a single window with a row per item, carrying what the shelves hold
  of it *and* what your pack could hand over. The amount on each row is
  signed: **Right takes off a shelf, Left puts your own cargo into a Depot**,
  and Enter moves the whole basket in one action and one turn. `P` is gone —
  an item that was on a shelf and in the pack at once used to have a row on
  each of two screens, with no way to see the other from either.
- **A transfer takes before it gives.** Emptying a full Depot and refilling it
  from your pack now lands in one commit; done the other way round the put
  would have been refused for want of room, silently.
- **The screen tells a full Depot apart from no Depot.** A Mining Node draws
  no room line at all, where a Depot with nothing left draws one reading 0.
  The put ends of every row read `-0` in that case while the take ends stay
  live, which is the state the report this came from could not explain.
- **`[A]` still means take everything**, and now overwrites a put you had
  set on a row with nothing on its shelf. Shift goes to the end of a row and
  Ctrl halves the gap, in whichever direction the plain arrow was heading.

### Fixed

- **A Depot at exactly its capacity no longer reads as a broken screen.** The
  old deposit picker showed rows whose ceiling was zero with nothing on screen
  saying why; the room line and the live per-row figures now say it outright.

## 0.13.22

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

A visiting caravan is worth walking over to.

### Changed

- **A caravan's shelf never lists the same thing twice.** Every category is
  drawn from without replacement now: a routine is its ability, a program its
  species, a stack of cargo its item, and gear the whole rolled copy — so two
  copies of one item are on the wagon together only when their rarity, affix
  or quality tell them apart. Before this a wagon could stand there with three
  stacks of Power Cells and two of the same disk.
- **Both traders carry fifty rows**, up from twelve and ten. A Salvage Convoy
  now sets out 23–34 distinct pieces of equipment, and about eleven of them
  are standout stock rather than three.
- **The Kennel Run deals more gear than routines.** There are sixteen
  non-boss species, so on a fifty-row shelf its program pool runs dry a third
  of the way down and everything after it comes out of the other two
  categories — which at its old weights made a program trader whose actual
  wagon was a rack of disks. It is `gear: 4` / `routines: 2` now, and stocks
  the whole roster besides.
- **`rows` is a ceiling rather than a count.** A category that runs dry stops
  being dealt from and the rest of the shelf fills out of the others, so how
  much of anything a wagon can hold is bounded by how many files are
  installed. A shelf deeper than every pool it draws from stops when the last
  one empties. `assets/caravans/README.md` documents both.

Rows past the thirty-fifth have no letter of their own — `menu_shortcut` runs
out at nine digits and twenty-six letters — so a deep shelf is walked with the
arrows below that point, as a long inventory already is.

## 0.13.21

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

The Stack answers four complaints.

### Fixed

- **Down turns you around instead of walking you backwards.** Underground,
  Down (or `j`) is now an about-face: the party pivots on the spot and costs
  a turn for it, exactly as turning left or right does. Backing up is gone
  entirely rather than moved to another key — you turn around and walk
  forward, which is what everyone was doing anyway.

- **You can power down in the Stack.** `r` was bound on the surface and
  simply absent underground, so the key did nothing at all: no rest, no
  refusal, nothing in the log to say why. Resting itself was never gated by
  where you are standing — free on your own slab, one Power Outlet anywhere
  else, the Stack included — so the whole of the bug was a missing key
  binding. `e` and a Power Cell always worked down there and still do; they
  are different things, and only `r` was broken.

- **Deep zones field deeper stacks.** How dangerous an underground fight is
  used to be read off the frame's depth alone, so the first frame of a stack
  was a single lone program whether you had just started the game or had
  breached your way to zone 9 to get there. Depth and zone now both count.
  A zone-3 stack fields the fights a zone-3 stack should, from the first
  frame down, and going deeper still escalates on top of that.

- **There is a boss at the bottom.** The thing guarding a lair was drawn
  from the same danger window everything else spawns from, and the hand-
  authored apex programs sit so far up that window that no stack short of
  six frames could reach one. Every shallower lair quietly served an
  ordinary program with a boss's stat line instead. The bottom of a stack
  now fields a real apex where its terrain has one, at any depth. Ambushes
  and wild spawns are unchanged: an apex you never saw coming is still the
  thing the window exists to prevent, and a lair is walked into on purpose.

## 0.13.20

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

Somebody comes to you for once.

### Added

- **Traders visit your base.** Stand an iso Market up and, every so often, a
  caravan walks in out of the sector, phases through the anchor, sets out its
  stock beside your counter for a while, and rolls back out again. You will
  see it coming across the map and standing in the base; `x` toward it says
  who it is. While it is docked, `b` → **Caravan** opens the wagon.

- **What is on the wagon depends on who turned up.** The Salvage Convoy runs
  mostly worn equipment; the Kennel Run walks programs on short leashes. A
  wagon carries gear with its own rarity and quality rolls, Routine Disks,
  programs, and stacks of the materials your base eats — everything you could
  get another way, for a markup, without the trip. Portal Fragments are not
  on it and never will be: breaching is still earned by fighting and
  descending.

- **They will take what you are carrying**, at the same rate your own counter
  pays. There is no buyback — a caravan does not come back for it — and `[S]`
  sells a whole stack at once.

- **A visit is a property of your base and cannot be rerolled.** When one is
  due, which trader it is, which way it walks in from and what it carries are
  all derived from the base's own seed, so a save and reload finds exactly
  the trader you left standing there, with exactly the rows you had not
  bought yet. Breaching leaves both behind.

- **New moddable content directory, `assets/caravans/`.** One `.ron` file per
  trader: its name, its line, its glyph, how deep its shelf is and how that
  shelf is split between gear, disks, programs and materials. Deleting the
  directory gives you the game exactly as it was before caravans existed.
  Schema in `assets/caravans/README.md`.

## 0.13.19

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

When the game says no, it now says so where you are looking.

### Fixed

- **A refusal is drawn inside the screen you typed into.** Pick a research
  node your zone is too low for and "Requires Zone 3 first." now sits under
  the popup's title, above the rows, in red. It used to be painted as a strip
  along the **bottom edge of the window** — on screen the whole time, and a
  full window's height away from the centred popup you were reading. Every
  screen with a popup does this: research, perks, building, crafting, trade,
  the roster, work orders, contracts, the lot. The four screens that draw no
  popup — the battle screen and the two full-pane frame maps — keep the strip,
  because they have nowhere else to put it.
- The popup **grows** by the line rather than covering a row with it, and the
  numbered options never renumber, so a refusal appearing cannot move the row
  a keypress was about to land on.

### Added

- **Refusals are kept in the message log.** They were never written there
  before, so a refusal you looked away from was gone for good after four
  seconds. `L` now has them, and the map's log pane shows them in red.
  Refusals raised *inside a fight* are deliberately still not logged: the
  battle pane is paced line by line as the round scrolls in, and a message
  from a submenu would arrive as narration and swallow a keypress on the way
  past.
- A confirmation is not a refusal — "Game saved." and a fuse's receipt stay
  out of that history, which is a record of the game saying no.

## 0.13.18

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

A hold order finally holds, and stripping a program takes one key.

### Added

- **`U` takes a program's whole loadout back off.** The roster's `E` opens one
  program's three slots and the picker's `(Unequip)` row empties one of them,
  so handing one program's gear to another was a keypress and a slot choice,
  three times over. `U` is that in one press, gear straight back into your
  cargo.

### Fixed

- **A satisfied hold order stands its line down.** A hold order reaching its
  level put the order to sleep but left the bodies standing on their machines,
  so the line kept running for the rest of the run — a base told to hold ten
  ICE Breakers had seventy-three on the shelf and was still making more.
  Programs now come off a machine nothing is asking for and go back to milling
  about. Reported from a live save: ticked three thousand times that base
  reached 222 against an order of 10, and now stops at exactly 10.
- The same sweep no longer takes a body off a **clogged** machine while a
  Depot is standing. That body is the only thing that can carry the clog away
  and let the machine run again, and freeing it left the machine full for the
  rest of the run.

## 0.13.17

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
Nothing here is stored: the new memories go into a store every program
already carries, and the figure they move is derived on every read.

Programs remember their working life, and how a program feels about its work
finally does something.

### Added

- **A program remembers the machine it works and the work it does.** Until
  now the roster remembered fights and strandings and nothing else — a body
  could spend a whole run at one node and hold no opinion of it. Four new
  memory kinds cover a program's working life: settling in at a machine that
  runs, resenting one that is always backed up, the grind of cutting rock,
  and being caught at a machine when a GC Entropy Sweep comes through.
- **Morale changes how reliably a program extracts.** The memories screen has
  headed itself with a Morale figure since 0.13.7 and nothing read it. A
  contented program now works a node a shade more reliably and a miserable one
  fizzles cycles more often. It is a small effect with a hard ceiling, and it
  is **symmetric** — the positive memory kinds are worth something now, rather
  than every memory being a liability.
- Both effects are as quiet as the parking hook that came before them. Nothing
  is logged, no number appears on the base screens, and the memories page
  (`R` from the roster) stays the one place that explains why a program is
  working the way it is.

### Changed

- **`MemorySubject::Structure` and `::Activity` have writers.** Both shipped
  with the substrate in 0.13.7 as subject kinds nothing could ever be written
  about. All six are live now.
- A memory about a machine names the machine's **kind**, not that particular
  machine, so it survives the machine being destroyed and a rebuilt one is
  remembered as the same thing. Settling in and resenting share that subject
  and pull opposite ways, so a machine kind that mostly runs nets out to a
  mild fondness over a run and one that spends its life clogged nets out to a
  grudge.
- A digger remembers cutting rock as a *kind of work* and has no machine to
  remember instead — a dig site is not a structure — so the memory follows the
  program rather than the hole.
- `assets/memories/README.md` documents the four new kinds and the axis that
  divides them, which is not their sign: a sweep is an **edge**, remembered
  the moment it lands, while the other three are **stretches of service**
  written on a period. A half-life authored for a stretch is not comparable to
  one authored for `mauled_by`.
- **Deleting `assets/memories/` still gives back the pre-memory game
  exactly**, extraction rates included. That property now holds at a third
  site and cost no code to keep there.

## 0.13.16

The defensive figure reaches the screens you compare programs on, and the
game settles on one word for it.

### Added

- **Attack and mitigation on the roster and the manifest picker.** Both lists
  quoted HP and PWR and nothing about a fight: the roster is where you decide
  which program goes in the party and which one gets the gear, and the picker
  exists to choose between subjects. Rows now read `ATK 8  MIT 5%` between the
  two, in the words the fuse picker and the field-cast picker already use. The
  picker's own row for you is unchanged — it quotes no HP or PWR either, so a
  lone pair of combat figures would be the only numbers on it.
- **A width census for the manifest picker**, which had none. At the widest
  reachable name it now measures 1170px into a 1243px body, making it the
  tighter of the two lists; the roster's head sits at 986.

### Changed

- **One word and one unit for mitigation, everywhere.** The map's status
  column said `Defense 12` for the number the manifest sheet has always called
  `Mitigation 12%`. The rename did not fit where it stood — `Attack 1234
  Mitigation 75%  Strength 1234` runs 38px past a column that cannot grow
  horizontally and that nothing clips, so it would have been drawn off the
  panel in silence. The four figures regroup onto two lines instead, offense
  on one and mitigation with the decompiler on the other, rather than taking a
  fifth line out of the buff and inventory lists below.
- The same word across eight talent descriptions, two companion upgrade items,
  the Defender perk, the achievements screen, the buff panel's row name, the
  two battle log lines announcing a boost landing and fading, and five
  schema-doc lines.

### Fixed

- **Nine ability descriptions were quoting a third of what they do.** The
  combat model rewrite converted `Buff(kind: Def, power: 4)` into
  `Buff(kind: Mitigation, power: 12)` and left the prose behind, so `bastion`
  has read "+4 DEF" against an authored 12 ever since, `acid_wash` "-5 DEF"
  against -15, and `bastion_shield_v3` "by 7" against 20. They now say what
  they apply, in the form the two that were already correct use — `long_winter`
  and `ablative_layer` have read "-25% incoming damage" all along. No
  magnitude changed; only the claims about them.
- `no_shipped_description_calls_mitigation_defense` is the gate that was
  missing. A screen's wording is held by the test that renders it, but nothing
  compiles a `.ron` description, which is how a rename sweep comes to stop at
  the code. Lower-case "defense" is deliberately allowed: a research node
  describing "automated perimeter defense" is using the ordinary word.

## 0.13.15

Base-space rock stops being one flat number, and walking stops being a way
to demolish your own base.

### Fixed

- **A swing can never take a whole wall.** `Game::swing_damage` grows all run
  against a rock durability that did not, so past a level every cell fell to
  one bump and navigating a developed base took its corners out a keypress at
  a time. `Game::strike_rock` now caps one swing at `durability / min_swings`
  for the cell's kind. Level-independent on purpose: scaling durability with
  the player would make digging cost the same forever. Levelling still cuts
  the swing count down to the kind's floor, it just cannot reach one — and at
  ordinary rock's 24 with a floor of 2 the cap is 12, above a level-1 swing,
  so the opening game's dig rate is unchanged.

### Added

- **Rock kinds, in `assets/rock/`.** One `.ron` per kind carrying a
  durability, a swing floor, a spawn weight and a brightness. Which kind a
  cell is, is *derived* from base space's own seed and the block the
  coordinate falls in — nothing is stored, so `BaseGrid` stays sparse and a
  wall nobody has touched still knows what it is. Kinds come in patches with
  an inside rather than as pepper, and an ore later is a file drop.
  `assets/rock/README.md` is the schema. An empty directory is supported and
  gives uniform rock — though not one-swing walls, since the swing floor is a
  fix and not content.
- **Mining is a tool you take out.** `n` in base space arms the player's own
  bump; disarmed, a step into rock is refused for free — no damage, no dig
  site, no turn. Off when a run starts, and off for an existing save, which
  never expressed a preference. A posted crew is unaffected: a mark is an
  instruction the base was already given, and putting your own tools away
  says nothing about it.
- **An exposed rock face shows what it is made of.** A wall with air
  orthogonally against it is drawn brighter for its kind and named by
  examine; rock behind a face stays anonymous. So exposing a face is the act
  of prospecting rather than reading a map of everything you will ever dig —
  cut a cell and its four neighbours light up, let entropy take it back and
  they go dark. Seeing a kind is a display rule only: a swing at unseen rock
  meets that kind's real durability.

### Notes

- The save gains a `mining` flag and base space gains a seed, both additive
  behind `#[serde(default)]`, so **existing saves load unchanged** and
  `SAVE_FORMAT_VERSION` does not move. A save from before this release lays
  its seams out from seed 0 — a valid layout, not a special case.
- The durability values, the swing floors and the vein block size are
  **unmeasured**, like every other knob in this slice. The design and what is
  open are in
  `docs/superpowers/archive/specs/2026-08-23-rock-kinds-and-mining-mode-design.md`.

## 0.13.14

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this moves where saves live, not what is in them.

### The game can be zipped up and played on another machine

Every runtime path used to derive from the absolute path of the machine that
compiled the binary, which is why `README.md` told players the clone had to
stay put. The game had no distributable build on any platform as a result.

Nothing else in the codebase was ever Linux-only — there is exactly one
`cfg(target_os)` in the whole tree, and the dependency graph resolves for
Windows and macOS without a system library either of them lacks. It was the
*distribution model* that was tied to a checkout, and only by accident.

`crates/launcher/src/paths.rs` is now the single answer to where the game
finds anything. It picks an installed layout when an `assets/` directory
sits beside the executable — or at `../Resources/assets`, which is where a
macOS `.app` bundle keeps them — and falls back to the repo otherwise.
Installed-ness is sniffed rather than flagged, because a build flag that can
be forgotten produces a zip that works only on the machine that made it, and
nobody finds out until a stranger unzips it.

The deliverable is an executable plus a loose `assets/` tree, never a single
file. Fonts and the sound cues are embedded, but species, items, structures,
abilities, talents, perks, achievements and help pages have to stay droppable
— that is the moddability rule, and it survives the move intact.

### Saves, your profile and the run history move to the OS data directory

`%APPDATA%\feral-processes\` on Windows,
`~/Library/Application Support/feral-processes/` on macOS, and
`$XDG_DATA_HOME/feral-processes/` (or `~/.local/share/feral-processes/`) on
Linux.

This happens in every layout, a development build included. Writing beside
the executable would leave a copy unzipped under `Program Files` unable to
save at all, and the failure mode there is a game that appears to save and
silently doesn't. Making it conditional — data directory when installed,
repo when developing — would have been two code paths where one will do, and
would mean a development build could not reproduce a player's report about
where their saves went.

An existing checkout's `saves/`, `profile.ron` and `run_history.log` are
moved across once, automatically, on the next launch. It is a move rather
than a copy, because two save directories that drift apart is worse than one
that changed address, and it does nothing at all if the destination already
holds a save — so it cannot fire twice or land on top of something newer.
The old one-time migration of the pre-`saves/` `save.bin` is folded into it
rather than left running beside it.

### macOS is covered by the same module

The probe that finds an installed build's assets checks
`../Resources/assets` after checking beside the executable, which is where a
macOS `.app` bundle keeps them, and the data directory resolves to
`~/Library/Application Support/feral-processes/`. Neither needed
macOS-specific code — the engine and app-core were already portable, and
this is the whole of what the platform asked for.

The recommendation is to ship a plain binary plus `assets/` in a zip, the
same layout as Windows, rather than a `.app` bundle. A bundle costs no path
code but does cost a plist, an icon and a build step, and Gatekeeper is no
kinder to one. What a bundle would fix is that double-clicking a plain
binary in Finder opens a Terminal window behind the game — macOS's version
of the console that `windows_subsystem` suppresses, and the one place the
two platforms are not symmetric.

`packaging/macos-readme.txt` ships in that zip, covering where saves live,
how to clear the download quarantine flag Gatekeeper checks, and that a mod
is a file dropped into `assets/`.

### A release build on Windows opens no console window behind the game

Debug builds keep theirs, which is what anyone developing on Windows would
want. The cost is that a release build has nowhere to print a startup
failure, so the two a player can actually reach — a missing `assets/` folder
and a data directory that cannot be created — now write `startup-error.txt`
beside the executable as well as to stderr.

### Unverified on Windows and macOS

Neither runtime has been exercised: window creation, wgpu (DX12 and Metal),
audio, input, the console suppression, SmartScreen and Gatekeeper, or
whether `%APPDATA%` and `~/Library/Application Support` resolve as expected. Verification for this is manual by choice, and the
ten-step checklist lives in the spec
(`docs/superpowers/archive/specs/2026-08-19-windows-and-macos-distribution-design.md`).
The Linux suite being green is not evidence about any of it.

## 0.13.13

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
nothing here is persisted.

### A defeated group no longer hands its turn to the pack behind it

Kill the last program in a group and the group behind it got to hit you
twice in the same round: once on its own initiative, and again on the dead
group's. The quieter half of the same fault ran the other way — a group
whose place in the order had shifted lost its round in silence, which reads
as a pack going passive the moment anything dies.

Initiative is rolled once at the top of a round, but a hostile was named in
that order by its *position* — which group, which member. A kill drops the
dead member, and drops the group entirely once that empties it, so every
position behind the casualty shifts down one and the order ends up pointing
at whoever moved into the gap. The order now names each hostile outright,
so what acts on a program's turn is that program or nothing.

A group promoted forward by a kill in front of it is also properly engaged
now, rather than swinging from the rank it held before anyone died.

The party side is untouched: a party slot stays a slot, because nobody
leaves the party mid-battle and the round's plan is written against those
slots. Aiming a strike at a group that falls before your turn still spends
the turn rather than redirecting it, exactly as it has since 0.2.

## 0.13.12

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
nothing new is persisted. `Stock` and `Inventory` are untouched.

### Put your cargo into a Depot

A Depot was the only structure you could take *out* of and never put *into*.
`P` beside one now opens a window over your pack — a row per item, a quantity
per row, and Enter puts exactly that basket into the base's hands. It is the
collect window's mirror and shares its keys, `[A]`/`[N]` and the Shift and
Ctrl arrows included.

The goods go into the Depot's output buffer, which is the point rather than
an implementation detail: that is the same buffer `base_holding` sums and the
same one a work order's feeders draw from. A production line stalled for want
of an ingredient you were personally carrying had no fix before this.

What may go in is plain, ordinary cargo. Fused, rare and high-quality copies
stay on you — a Depot stores items by name and has nowhere to record what
made a copy special, so one put away would come back out ordinary. Banked
Research Data stays out for a different reason: a bank is not cargo.

A Depot's room is one budget shared across every row, so filling one row
lowers what the rest may reach, and the window says how much is left. An
over-ask is clamped rather than refused, and a Depot with no room left takes
nothing and costs no time.

### Also

The collect and deposit windows are now one key table rather than two copies
of one. Nothing about the collect window changed — but the inverted Left/Right
that window specifies, and the Ctrl step that halves the gap, now exist once
instead of twice, where they cannot drift apart.

## 0.13.11

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
`ItemDef::enables_rest` is an additive, defaulted asset field, and the two
retired structure fields were never written into a run.

### Resting is priced by where you are, and never costs time

Rest no longer needs a structure standing in reach, and **no rest advances
the clock**. Inside base space it is free: the walk home is the whole of the
price. Anywhere else — the open grid, or four frames down the Stack — it
spends one unit of an item whose def carries the new `enables_rest`, and the
Power Outlet is the one shipped item that does.

The two halves hold each other up. A base rest that ticked could be spammed
to farm production, raid pressure and need decay; a priced rest that ticked
was the game's only bulk source of time. Neither is true now, and
`Game::wait` is the only thing left that passes time without an action.

`RestDef`, `StructureDef::enables_rest`, `nearby_rest_structure`,
`rest_cost` and `REST_TICKS` are gone, none of them with a reader left. The
two help pages that called a recharge a full night now say what it is: free
at the base, an outlet in the field, instant either way.

### A marked room your crew would never touch

Marking a block of rock out in the open and leaving idle programs standing
in the base got you nothing at all — no swing, no walk, and no line in the
log saying why.

Every cell of such a block is boxed in but its rim, and a boxed-in cell is
refused *silently* on purpose: it is the ordinary interior of any plan and
it resolves itself as the shell in front of it comes down. But the refusal
happened after the crew's work had already been budgeted, so the interiors —
which sort first — spent the whole budget, and the rim was cut off the end
of the list before anyone looked at it. A thirty-six cell room with one
reachable cell and six spare programs sat untouched for the rest of a run.

A cell nothing can stand beside is no longer a job at all. The budget goes
to cells a program can be sent to, and each cell behind them becomes a job
the moment the rock in front of it opens — so a room now unpeels from its
face inward, with the crew fanning out as it widens. A marked cell that has
a face and no route to it is unchanged: that one is your errand, and it
still says so once.

## 0.13.10

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this is an input and text change and nothing about it is written into a run.

### The collect window's arrows are the other way round

Left now adds one to the highlighted row and Right removes one. The window
shipped a release ago with the conventional mapping and it read backwards in
play, so the polarity is reversed on this screen alone.

It is the one inverted Left/Right in the game — the manifest pager, the
arena row editor and every movement handler still step Right positive — so
the window now says which arrow does which instead of the old
"Left/Right set the amount", and the base's help page says it in prose too.
Nothing else about the screen moved: digits and Backspace still type an
amount, `[A]` and `[N]` still fill and clear every row, Enter still takes
exactly the basket and Esc still costs nothing.

### Shift and Ctrl on the collect window's arrows

The two modifiers are different verbs. **Shift is an end**: Shift+Left
fills the highlighted row to what is on the shelf, Shift+Right puts it back
to nothing. **Ctrl is a step that halves the gap** to whichever end it is
heading for — on a shelf of 7, Ctrl+Left walks 4, 6, 7, and Ctrl+Right
walks back down 3, 1, 0. Press it again and you get half of what is *left*,
not the same number twice.

The step rounds up, which is what makes it finish: rounded down, a gap of
one gives a step of nothing and the key would go dead with the row neither
full nor empty.

Both are per row, which is what separates them from `[A]` and `[N]` — those
are the same two ends across every row at once. Holding a key is safe either
way: Shift is already at its end, and Ctrl converges on one.

Every other screen is unchanged. A modifier reaches the collect picker and
nowhere else, so Shift with an arrow still walks, still pages a manifest and
still moves a building cursor exactly as the bare arrow does.

## 0.13.9

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
nothing about this is written into a run.

### `c` takes what you ask for

`c` beside your base used to empty every output buffer touching you into
your pack, wholesale. There is no deposit verb — nothing in the game puts
units back into a base buffer — so a misfired keypress beside a working
line was not a convenience cost but a permanent one: the ingredients a
chain was about to pull were now in your pack, and the only route back was
to make them again.

It now opens a window. One row per item on offer, pooled across every
machine touching you, and every row starts at **zero**. Up and Down pick a
row; digits, Backspace and Left/Right set an amount; Enter takes exactly
that basket, in one action and one turn. `[A]` fills every row to its
maximum and `[N]` clears them all, so taking everything is still two keys.
Esc takes nothing and costs nothing, the same way a collect that finds
nothing has always cost nothing.

An amount clamps as you type it: `50` against 12 on the shelf leaves the
row reading 12, rather than silently ignoring the second digit. An over-ask
at the moment you commit is clamped too — a raid or a hauler can empty a
shelf while the window is open, and a basket that has gone briefly
optimistic hands over what is there rather than refusing.

Underneath, taking everything is now literally selecting everything and
then committing: one reach rule, one taking path, and one place the
"nothing to collect here" refusal is spoken. Units leave a buffer through
`hauling::take_from` alone, where the old wholesale path removed the entry
by hand — correct only because it always took the entry whole.

### The dig crew pays for its tile out of the base's own stores

A crew that had cut a marked cell open floored it only if the *player* was
personally carrying a Blank Substrate — 12 on the base's shelves and 0 in
the pack left two marks standing over two finished cuts, and not a word
about why. The crew now draws from the base's output buffers, in tile
order and over the same set the stock strip counts, with the player's pack
as the fallback. `Game::lay_tile` is untouched: a player verb still pays
the way every player verb does.

The silence was half of it. A crew with nothing to lay now says so once,
beside the once-only complaint a boxed-in crew already made. Neither latch
is saved, so a reload says both again.

## 0.13.8

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this is a rendering change and nothing about it is written into a run.

### The equipped panel says what your gear was compiled at

The `WEP` / `ARM` / `MOD` column carries a copy's quality in its emphasis —
dimmed under spec, bold above it, gold at the top — and the three rows
naming what you are *actually wearing* were the only gear rows in the game
without one. The screen showing your loadout could not say how well any of
it was made.

They carry it now, on the cargo screen and on a program's gear page alike.
The column replaces the spelled-out slot rather than sitting beside it:
`WEP` and `Weapon:` say the same thing, and printing both puts one column
on a row twice. An empty slot still names the slot it is empty of.

## 0.13.7

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
A save written before this release loads with every program given a fresh
identity and an empty memory store — the three new fields are additive on
field-named RON.

### What a program remembers

Programs on your roster now form memories of what happens to them, and hold
them. Four kinds ship:

- **Fought beside** — a bond with each program that survived a won fight
  alongside it.
- **Won against the odds** — a fight the party had no business walking away
  from.
- **Mauled by** — the species that took better than a third of a program's
  health in a single landed blow. The longest-lived of the four.
- **Left stranded here** — a grudge against one corner of the base, formed
  when a program is posted somewhere it turns out it cannot reach.

**A memory fades unless something reinforces it.** Intensity is worked out
from the clock every time it is read rather than being counted down, so
nothing ticks and nothing drifts; a repeat of the same event resets the
clock and deepens the mark, up to a limit each kind sets for itself. A
memory that has faded far enough is dropped the next time that program
forms one.

**`R` on a roster program opens what it holds** — a page headed by that
program's Morale, the signed sum of everything it currently remembers, with
each memory's strength and how long ago it was, in words rather than in
ticks. Rows arrive strongest first.

### A program will not be parked where it was left stranded

The one place a memory changes behaviour, and it is deliberately quiet: an
idle program loitering in your base will not be stood on a tile it holds a
grudge against, and takes a different spot on the ring instead. It costs the
program one beat of standing still — the same thing already happens when
the ring offers it a tile a machine is standing on.

The loop closes on itself. A program is parked somewhere, posted to a
machine, finds no route to it, and is marked stranded *where it is
standing*. That is the tile it remembers, and that is the tile it will not
be put back on.

Nothing else about staffing changed. The scheduler still decides the whole
assignment by priority and then diffs it, with no score anywhere in it.

### Memory kinds are a content directory

`assets/memories/` is a mod directory like species, items and abilities —
each kind is one file naming its valence, its half-life, what it can be
about, and how far it compounds. The schema is documented in
`assets/memories/README.md`. **Deleting the directory gives back the
pre-memory game exactly**, the same supported way deleting
`assets/environment/` or `assets/policies/` does: memories already formed
are kept but weigh nothing, the page draws no rows, and the parking hook
goes inert.

## 0.13.6

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
Nothing here is stored — both features read figures the game already held.

### The player sheet states its to-hit pair

COMBAT grows to six rows on the player page — Damage, Attack, **Accuracy**,
**Evasion**, Mitigation, Power. The two new figures come from the same
`accuracy_of` / `evasion_of` calls `resolve_attack` consults, so the sheet
cannot quote you odds the fight then disagrees with.

Player-only, and that is a layout budget rather than a data gap: the
program page's worst case clears its footer by 17.3px against a 10px floor,
so one more row anywhere on it overflows at 1280x720. Both figures are
carried for both subjects, so buying that page room later is a layout
change and not a data change.

### A RUN box says what the run is holding

Credits, Portal Fragments, the difficulty mode, the cycle, and how many
contracts the run is signed to. Credits and Portal Fragments are banked
pools that deliberately sit outside your inventory, which is why the cargo
row has never said anything about either. Trace is absent on purpose —
it is underground-only and the Stack view already draws it.

### A stock strip says what the base is holding, on every screen

One row across the top of the window, on every screen that draws the world
behind it — including under a menu, since a popup is capped at 85% of the
window and leaves the top band clear. Each pile is a two-letter tag and a
quantity.

**It is a readout, not a second opinion.** `Game::base_stock` reads the same
output buffers the base's own holdings are summed from, so the strip cannot
drift from what the base actually has. Piles are ordered by item id rather
than by quantity — a strip that re-sorted as buffers filled would move
every tag under the eye of the player reading it — and the row is
*measured* rather than estimated, so the piles that fit are named and the
rest are counted in a `+N` tail instead of being drawn off the edge in
silence.

Tags are derived from an item's name, so a modded item gets one for free,
with an override for the one shipped collision: Research Data takes `[R]`
and the Research Disk keeps `[RD]`.

The panes below the strip now take their origin from the caller rather than
from the window. Both views already funnelled their geometry through one
converter each, so the offset is stated once and everything follows it.

## 0.13.5

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
An order filed before this release loads as a one-shot batch at the Normal
band, which is what it was.

### The base works every order at once, in priority order

The queue used to be a to-do list: the base worked the front order and
nothing else, so a base with more programs than that one line could use
parked the spares beside a machine that already had somebody on it. It is a
production policy now — every unsatisfied order is worked at the same time,
and the order nearest the top gets first refusal on every body.

Priority is still queue position, and nothing was added to make that work.
The wants come back in queue order and the scheduler already cut the list
from the end, the same mechanism that has always made dig jobs the lowest
priority. Two orders wanting the same machine are counted once, so the
higher one keeps it.

**Your staffed base is materially more productive as a result**, and nothing
in the suite can see that — `balance_sim` has no base term at all. It is a
pacing question for play.

### An order can be a level the base holds, not just a batch it makes

`[S]` on the quantity page files a **standing** order. A batch is finished
and removed; a level is held — when the shelf drains, the order re-arms and
the base makes more.

That closes a hole the old behaviour had no answer for: collecting from a
machine empties its whole output buffer, so walking past your own base
drained the very stock the order had just declared complete, and the order
was already gone. There is no hysteresis and no refill threshold, because
the drain is a burst rather than a trickle and there is nothing to
oscillate around.

A standing order says nothing when it tops itself up — "complete" is a lie
about something that is not complete — so **filing** one announces itself
instead.

The quantity page also now says that the target is what the *base* holds:
machine and depot buffers, not your pockets. `0/20` while carrying forty of
the thing is the figure working correctly.

### An order can be filed above or below the queue

`[P]` on the quantity page cycles a priority band — **High**, **Normal**,
**Low** — and it raises first, since raising is what the feature is for.
Before this the only control you had over the base's attention was cancel
and refile, which lands the order you care about at the *bottom*: strictly
worse than useless.

The band is an insert position rather than a second sort. An order lands
after the last order of equal or higher band and nothing reads the field
again, so ties still break by the order you filed them in and one band is
still a queue. Refiling an order now restores its band instead of dropping
it to the foot of the list.

### The queue screen says what the base is doing about each order

Every order carries a tag: **WORKING** (somebody is standing on its chain
right now), **QUEUED** (it wants machines and the base ran out of bodies
before it got here), **HOLDING** (a standing order at its level — the
feature working), or **STALLED** (the line broke).

The two that needed telling apart are HOLDING and STALLED. Both want
nobody, and one of them is a base that needs rebuilding. WORKING is read
off who is actually posted rather than off what the scheduler asked for,
because two machines in the want list never get a body — one the base has
been built around with no route to it, and one held by a program the
scheduler is not allowed to move — and calling those "working" sends you
off to watch a machine nobody will ever stand at.

### A broken line says so in the log

A stalled order was news only if you opened the queue screen on purpose.
It now logs **once**, on the way into the stall, and again if it breaks a
second time after you have repaired it. Not every tick: a line you have
already been told about is not news.

The log line is the headline alone — "Work order stalled: 30 x Routine
Disk." The sentence naming *which* machine went missing stays on the queue
screen, where it is wrapped; the commoner of its two shapes runs to 198
characters, and the map's log pane draws a line as exactly one unwrapped
row about 135 cells wide.

Reloading a save announces the stall again. The run that was told is over.

### The screen says how short of bodies the base is

A header above the key hints: how many posts the queue asked for against
how many programs the base has. The scheduler cuts its list to the bodies
it has and the posts past the end vanished in silence, so a three-machine
line with two programs said "no one" on the third machine and nothing
anywhere said you were one program short.

It answers "why is nothing happening" from the other side to the tags: a
tag says which order has the base's attention, this says whether the base
has anyone to give it. It is **silent when you have bodies to spare**,
because a line that shows on every visit is a line nobody reads by the
third one.

## 0.13.4

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

### The perk screen has sections

Seventeen perks in one undifferentiated column made you read all of them to
find the three you cared about. They now sit under four headings —
**Combat**, **Affinities**, **Fieldcraft**, **Workshop** — in that order.
The shortcut keys still run straight down the list across the headings
rather than restarting under each one, so nothing about typing a key
changed except which letter lands where.

**The layout is a file.** `assets/perks/groups.ron` names each section,
what sits under it, and where it sits, all in one statement — rename a
heading, reorder the sections, move a perk between them, none of it needs a
recompile. The format is documented in `assets/perks/README.md`. It is one
statement rather than a `group:` field on each of the seventeen catalogue
entries because membership alone orders nothing: a per-perk label would
need a second rule for which heading came first, and two authored halves of
one layout drift apart the first time someone edits only one.

Three things it deliberately does not do. Deleting the file gives back the
flat, unheaded list exactly as it was. A perk no section names is still
offered, in a trailing unlabelled run at the foot of the screen — a typo
costs a heading, never a row you can spend points at. And a malformed
layout is skipped with a warning, costing the headings and none of the
perks.

### The results screen says you won

A win was the only ending in the game with no line of its own: the screen
went from the killing blow straight to `Salvage:`. It now heads the results
with **"You won!"**. A jack-out and a flatline are untouched, since both
already declare themselves one line higher.

The experience lines gained an `Experience:` header and the same indent the
salvage rows carry, so the two blocks read as a pair rather than as a list
and some loose numbers.

### The decisive round's blows outlive the fight

**Fixed:** the final round of a battle was narrated and then deleted before
the screen had revealed a single line of it, so the results appeared to
jump from the kill straight to the salvage. The round that decided the
fight is the one you most want to watch, and it was the one round you never
saw.

The narration is now pruned when you *leave* the results screen rather than
when the fight ends. Two consequences worth knowing: the closing roster now
stands beside the final blows instead of replacing them, and a companion
detaching after a loss scrolls past on the results screen rather than
vanishing with everything else.

## 0.13.3

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
Gear gained a quality figure, but it is an additive field with a named
default, so every copy in a file written before it existed loads at exactly
100% — the numbers that run already had.

### Gear is something you compile, not something you find

Every blade you compiled used to be byte-identical to the last, which put
the whole "is this one better than mine" question in the hands of the loot
table. A carried copy now records **how well that particular copy was
compiled**, as a percentage of what the item was designed to do, and it
shows in the name: `Overclocked Arc Lance of Static (115%)`. A copy at
exactly spec says nothing, the way an ordinary tier does not print a word.

**Your base is what makes good gear.** The floor a compile rolls from is
built out of things you own: the tier of the bench it is compiled at, a
perk, and whether you asked for a careful job. A fresh player's compile
lands in 80–100; a developed base's reaches 110–130. **A field drop rolls
from a poorer floor than any of that** — 70–90 — so the world is a lottery
ticket and the base is the gearing path. That is a real change to the early
game: your first compiles are *weaker* than they were, when every craft was
exactly 100.

**A batch is a spread, not five of one thing.** Each unit rolls its own
luck, so compiling five gives you five copies to compare and keep the best
of. A copy that rolls exactly 100 still stacks; the rest each take a row.

**`[C]` on the quantity page toggles a careful compile** — more quality for
more materials, priced at every figure that page quotes, including the
max-affordable line. The toggle clears when the page opens, so it can never
outlive the batch you asked for.

**Both compile benches can be upgraded now.** The Fabricator and the Armory
had no upgrade path, which meant the bench term did nothing on any shipped
recipe. Upgrading one buys better gear and nothing else.

**Tighten Tolerances** is the seventeenth perk: a higher floor on everything
you compile, read at the compile rather than banked at purchase, so gear
already in your buffer keeps what it was made at.

**The category tag on a list row carries the read.** `WEP`, `ARM`, `MOD` are
drawn in the copy's own emphasis — dimmed under spec, bold above it, gold at
the top — while the row's colour goes on meaning fusion and rarity. Only the
two extremes spend a colour, and a copy at spec is drawn exactly as it
always was.

### `[I]` inspects a piece of gear from anywhere it is named

Seven screens name gear and none of them could answer "what does that
actually grant". `[I]` now opens one page from all of them — the cargo list
and its action page, the swap picker, a program's slot page, a trader's
three shelves and a Stack stall.

The page draws the whole of a granted routine rather than its name: what
fires it, what it lands on, what it hits for **at the wearer's level**, its
cooldown and its price. Plus the stat block at the level the copy would go
on at, the accuracy it buys, the hit chance that works out to, and what it
compiled at. A piece carrying neither a damage band nor accuracy quotes no
hit chance at all, since that figure is the wearer's and printing it under
armour reads as a claim about the armour.

### The map draws one space, and it is the one you are standing in

Three reports, one cause: the base's coordinates and a zone's spawn point
are both usually (0, 0), so a tile in one freely aliased a tile in the
other.

- **Your `@` never moved inside the base.** It does now.
- **Stack entrances, nests and the anchor drew inside the base.** They
  carry a glyph and are neither structure nor program, so both existing
  gates looked straight past them.
- **Your base roster drew scattered across the open grid.** Idle staff are
  parked in base coordinates every tick, which is exactly what made them
  look like programs standing out in the sector.

**A GC Entropy Sweep's flash belongs to the base too.** A sweep landing
while you were out on the grid washed a tile of open ground and threw
debris across it — usually the tile you were standing on. The log line and
the pane's own flash already carry that news without claiming a tile.

### Eight more manual pages

Intrusions, Your companions, Routines and field buffs, Perks, Your base,
Supplies and salvage, The Stack, and Before you breach — all written
against the source rather than from memory. The Controls page also listed
two base rows that stopped existing when work orders replaced manual
posting, and omitted three party rows; both now match what the menus draw.

### Also

- **`<` and `>` at the anchor read as up and down now**, not in and out.
  `<` phases up into the base and `>` drops back to the grid — the mirror
  of what the Stack binds rather than a copy of it.
- The equip swap row sheds its stat column onto a continuation rather than
  running it off the edge of the popup. The quality figure costs seven
  cells, and the widest row had 3.7 to spare.

## 0.13.2

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
the manual is content read off disk and nothing about it is written into a
run.

### A manual you read inside the game

`?` used to draw a card of key bindings that closed again on any key. It now
opens a manual: an index of topics, each one a page you can scroll, with
further reading at the bottom of each page that you follow by typing its
key. Four pages ship — Start here, Controls, Zones and breaching, and
Getting stronger — and the old key-bindings card is one of them rather than
a second help surface you have to know about separately.

**`?` no longer closes on any key.** That is a real change to an existing
reflex: the screen is navigable now, so Esc backs out of it one page at a
time, like every other screen. Esc from the index closes the manual.

The index is a menu and a page is a document, which is why they take
different keys. On the index, Up/Down and Enter or a row's own key pick a
topic. On a page, Up/Down scroll the prose, Enter does nothing, and a
further-reading row is followed by typing the key beside it. Esc pops one
level of wherever you have read to, so following three links and walking
back out lands you where you started.

### Pages are files you can edit

The manual is `assets/help/*.md` — ordinary markdown, five rules' worth of
it, documented in `assets/help/README.md`. Adding a topic is dropping a file
in that directory: no rebuild, no registration, and the filename is both the
ordering and the id a link points at. `[label](topic-id)` in a sentence
reads as the label and adds the further-reading row in one gesture, so a
cross-reference is written once, where it belongs.

A malformed page is skipped with a warning rather than refusing to start, as
every other asset directory does, and a link pointing at nothing is dropped
from the list instead of drawing a row that refuses when you pick it.

### A program you own and aren't fighting with is base staff

Landed on `main` after `0.13.1` was tagged and ships here. Base staff used
to be a marker you assigned by hand from the Base Staff screen. It is
derived now: a program on your roster that is not in your party and not
held as your weapon **is** base staff, with no verb to assign it and nothing
to forget to assign. The screen keeps its rows, its activity and its work
profiles, and loses only its write.

Two things follow from that. Posting a worker no longer pins it — the
poster is in the pool, so the scheduler may move it next tick — and your
base's output now scales with the size of your roster, bounded only by how
many programs you can hold.

Existing saves are unaffected: the old marker is still written and simply
read nowhere.

## 0.13.1

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
the walls you have started on and the plans you have drawn are a new,
additive save field, and a file written before it existed loads with no dig
sites, which is exactly what that run had.

### The base grows, and you cut it out yourself

0.13.0 moved the base into its own space and said growing it was out of
scope. This is that scope. The pocket you start with is a room in solid
entropy, and every cell past it is rock you can take down.

**Rock is hit, not walked through.** Step into a solid cell in base space and
you swing at it, the same way you wear a nest down. There is no new key and
no direction prompt — the wall is a thing you attack. Swings are
deterministic, so a wall never becomes a slot machine: it takes about three
hits at level 1 and one hit late in a run, and **that is the reward for
levelling rather than a curve that scales away from you**. Rock is the same
rock in every zone at every depth; what changes is you. A cell that opens
sometimes shakes a Core Fragment loose.

**A cut cell is not floor yet.** `v` lays a VectorStasis Tile on the cell
you're standing on for one Blank Substrate, and that is what makes ground
permanent and buildable. Bare cut ground is the frontier, and the frontier
does not keep: leave a cell open long enough and base space takes it back to
solid rock, at full thickness, so re-opening it costs the swings it cost the
first time. Laid tile is never reclaimed at any age, and neither is a cell
somebody is standing on — a base can be left alone while its owner is off in
a zone without coming home to a smaller one.

**`m` opens the Excavation plan**, a mode rather than an action: the cursor
costs no time and no tick, `space` drops an anchor, moving previews a
rectangle, and `space` again commits it. Marking and clearing are the same
verb, decided by the cell you anchored on — anchor a marked cell and the box
clears instead. A marked wall runs the whole way through on one mark: it gets
cut, the mark survives the cut, and then it gets floored.

**And you don't have to be there.** Post programs to your base and marked
cells are cut and floored while you are off in a sector. Dig jobs are the
**lowest** priority the base has, below work orders and standing jobs, so a
spare body digs and a needed one does not — marking a corridor can never
stop production. A marked cell your crew genuinely cannot reach says so
once, and then stays quiet; a marked cell walled in by the rest of your own
plan says nothing at all, because it opens itself as the shell comes down.

### Also

- Three contract and environment texts still described the base as a "slab"
  stamped onto the zone surface, which it stopped being in 0.13.0. A Hunt
  contract now speaks of what is loose within sight of your anchor.
- All four of the new tuning values — rock durability, the fragment chance,
  how long the frontier holds, and how fast a posted digger swings — are
  unmeasured starting values.
- GC Entropy Sweeps were landing on marked rock instead of your machines. A
  dig site carries a durability pool, which is what the raid picked its
  target by, so a large plan drew nearly every sweep away from the base —
  and a sweep that finished one off took the mark and every swing of
  progress with it while the wall stayed standing, so it healed to full.
  Sweeps target buildings again.
- A base staffer standing between postings could have the ground reclaimed
  out from under it and be sealed into solid rock permanently — unable to be
  posted, and unable to walk anywhere, for the rest of the run. The frontier
  now counts anybody standing on it, not just somebody mid-job.
- Clearing a plan the crew had already started on did not call them off: the
  site keeps its chip progress by design, and nothing downstream was reading
  the mark, so a digger finished a wall you had told it to leave.
- Every program dug at the player's rate. A crew program now swings its own
  species' attack band, so which program you put on a wall is worth
  something beyond its attack score.
- Clearing a mark off a cell the frontier had already reclaimed left an
  invisible record behind — drawn nowhere, wanted by nobody, and saved from
  then on.
- A marked cell the crew could reach could still be reported cut off. The
  base picked one face of a post up front — the nearest, ties to the lower
  x — and gave up if that one had no route, which for a cell on a rock spur
  is the side facing unbroken rock. It tries the other faces now, and
  because a dig site announces being stuck only once, the old behaviour
  skipped that cell for the rest of the run.
- Committing a large plan no longer costs more the more of the base you
  have already dug. The box was looked up tile by tile against every dig
  site standing, which is a full scan per cell — a maximum box ran 625 of
  them. It builds one lookup per press now.
- A wall you chipped and walked away from is called a Chipped Wall rather
  than a Marked Cell: most dig sites are not in any plan, and the manifest
  said they were.
- Pressing the tile key with nothing to press left the refusal in the base
  log as well as on screen, once per press. The base log records what the
  base did, not what it declined to.
- The fragment-payout bound is now stated per tick, against the Mining
  Node's own rate. As a per-cell comparison it could not fail: a cut cell
  pays at most one fragment against the four a Blank Substrate costs, so
  every legal value of the knob passed it. At the shipped numbers the real
  bound still has about fourteen times the slack it needs, so it is a
  backstop rather than a tight gate.

## 0.13.0

**Saves written by 0.12.0 and earlier will not load.**
`save::SAVE_FORMAT_VERSION` moves from 31 to 32. The base's own coordinate
space and everything standing in it are new save state, and there is no
sensible way to derive a pocket dimension's floor plan or a structure's
place in it from a v31 save that never recorded either.

### The base is out of phase

The base has left the zone surface for its own pocket-dimension coordinate
space, entered through a permanent, indestructible anchor (`#`, gray) that
now stands wherever your run starts, on the zone surface, alongside you.
Step onto it to phase in; step onto the same cell from inside to phase back
out. The anchor travels with you across a breach, appearing at the new
zone's spawn point, and cannot be destroyed or moved.

Deploying your first Home lays a pre-cleared, 69-cell pocket of floor around
the anchor's door rather than stamping platform tiles into the world you're
standing in — the base is *there*, not *here*, and every machine, trader and
posted program you build stands in that space from then on. Breaching no
longer touches the base at all: nothing despawns, nothing repositions,
nothing resets. It is exactly where you left it the next time you step
through the anchor, on any zone.

The Heap Pillar and Heap Block, the two structures that used to widen the
base's footprint on the surface, are retired along with the surface
footprint they widened. Growing the base is out of scope for this release.

### Two things that follow from being out of phase, and are not bugs

**A rest can no longer be interrupted.** Nothing on the surface can reach a
party that is off in its own coordinate space, so the mid-rest battle check
that used to be able to cut a rest short never fires there.

**The base is now completely safe from surface threats.** A wild program, a
raid, anything that hunts on the zone surface has no way to reach a base
that isn't on it. Both are the direct consequence of the relocation, not a
balance change.

### Also

- Two biomes were added for the new space, `Entropy` and `Excavated`, drawn
  through the existing surface renderer — nothing new to build for them on
  the graphics side.
- A Recharger Node now regens the party while they're genuinely standing in
  base space, and the structure roster's "Work it yourself" row now appears
  when you're really beside a machine in there — both were measuring the
  player's surface tile against a base-space structure's coordinates and
  came up wrong for the whole of the base.
## 0.12.1

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 31.

One release covering six changes, rather than the per-change releases the
preamble asks for. All six had already landed on `main` unversioned; this
section is written to match what the tag actually holds rather than to
pretend the rule was followed.

### Companions act instead of inflating your sheet

A companion used to feed a tenth of its ATK and mitigation into the player's
own stats, floored at one point each, *on top of* taking its own turn,
swinging and soaking hits. Recruiting was paying twice for one body. The
passive half is gone; the roster acts in its own right.

A **wielded** program keeps its bonus, and the difference is the argument for
it: wielding takes the program out of the party, so it never takes a turn and
its share of stats is the only thing it contributes.

### The map glides again

Reported from play: walking the map had gone jerky. The camera code was never
the problem — there were no frames to draw the glide in. The root
`Cargo.toml` carried no `[profile.dev]`, so bevy, wgpu and egui compiled
entirely unoptimised into the build `cargo run` produces, which is the build
the game is played from. The renderer's shape-building pass alone measured
**51.4 ms a frame** against release's 2.0 ms at an identical shape count —
under 20 fps before the tessellator had done anything.

Dependencies now build at `opt-level = 3` and the four workspace crates at
`1`. The debug draw pass lands at 2.3 ms. The price is one cold rebuild of
the 557-crate graph.

### A posted program walks from where it was standing

The base scheduler read *your* tile twice, and both readings went wrong once
idle staff started loitering. A program milling by Home teleported onto you
the instant it got a job and walked in from wherever you were; and walking
away from the base stopped it filling a single machine, so the pool stood
idle beside the order it was hired to work. A base that only runs while you
are stood in it is not a base.

Posting now writes no position at all, and every question is asked of the
body being sent.

### The research screen says what each row is waiting on

Every row in the research menu (`T`) is now coloured by what stands between
you and it. Green is a node on a recommended path you can buy right now,
plain white any other available one. The three you cannot take are quieter
and told apart by hue: amber for a node waiting on another node, blue for one
waiting on a breach, grey for what is already researched. A node held by both
walls draws blue, and the tag after its name still names both reasons.

Which nodes are recommended is **data**, not Rust: a research file sets
`recommended: true` on a destination and the whole chain leading to it
inherits the colour, so the green row is always one that can actually be
bought. The shipped tree points a new run at the Compiler and at
Fortification. See `assets/research/README.md`.

### Also in this release

The **Power Outlet** costs 5 Core Fragments again. A stray asset edit had
dropped it to 2 while leaving its value at 5 — an item worth more than its
recipe is an infinite Credit loop, and the price census caught it.

### For anyone measuring

The removed party stat bonus was never modelled by `balance_sim`, so no curve
in the suite moved — a doc comment there claiming it as one of three ways
party size compounds was a copy of the game's behaviour rather than a reading
of the module's, and now says two. Measured in the arena instead, 200 reps
across four party-bearing scenarios: fights run 0.3-0.4 rounds longer and the
player keeps 1-3 points less Integrity.

The debug profile also cuts the engine suite from 38.6 s to 6.7 s and
app-core's from 10.2 s to 1.7 s; CLAUDE.md's build section is rewritten
around the new figures, and its claim that the old ~24 s was an unavoidable
RON artifact is retired. Full numbers and blind spots for both are in
`docs/measurements/`.

