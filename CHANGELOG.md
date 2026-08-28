# Changelog

Release notes for [feral-processes](README.md).

Versions follow [semantic versioning](https://semver.org). While the project
is `0.x`, a **breaking** change bumps the minor version and a compatible one
bumps the patch. For a single-player game with no public API, "breaking"
means one thing above all: **a save-format bump**, where existing saves stop
loading (see `save::SAVE_FORMAT_VERSION`). Every crate in the workspace
shares one version, set in the root `Cargo.toml`.

At `1.x` and beyond the same definition of "breaking" moves up a level: a
save-format bump takes the **major**, new content or a new feature takes the
**minor**, and a fix or a balance retune takes the **patch**. The rule that
survives the boundary is the one that matters — what counts as breaking is
decided by what happens to a player's save, not by what happens to a type
signature.

**A release is cut per change that lands on `main`, not per batch.** A
feature or fix merged to `main` bumps the version, gets its own section
below, and is tagged; commits on a branch stay unversioned until they land.
This is a correction, adopted after `0.3.0` — everything from `v0.2.0` to
`v0.3.0` accumulated in a single `Unreleased` section over 2,200 lines long,
which is a changelog nobody can read and a version number that says nothing
about what is installed.

Entries below `0.2.0` predate versioning and are kept as written, newest
first, separated by a rule.

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
  `docs/superpowers/specs/2026-08-23-rock-kinds-and-mining-mode-design.md`.

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
(`docs/superpowers/specs/2026-08-19-windows-and-macos-distribution-design.md`).
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

## 0.12.0

**Saves written by 0.11.9 and earlier will not load.**
`save::SAVE_FORMAT_VERSION` moves from 30 to 31. `Stats::def` became
`Stats::mitigation` and changed *unit* — subtractive absorption became
percentage points — and a field whose meaning changes under a name it keeps
is the one case field-named RON cannot rescue: a v30 file would load `def: 6`
into a percentage slot and read as 6% mitigation rather than 6 points of soak.
It is refused by version instead. `FieldBuffKind::Def` folding into
`Mitigation` rides the same bump.

### Every attack rolls to hit

An attack is now resolved against the defender's **Evasion** rather than
landing automatically. Accuracy and Evasion are derived from a species'
`base_speed` plus its level plus gear — never stored, so they cannot drift —
and the odds are the scale-free ratio `accuracy / (accuracy + evasion)`,
clamped between 25% and 95%. A zone that scales everything therefore changes
no hit rate anywhere.

One roll decides the outcome across four bands: a **critical hit** doubles the
rolled portion of the damage (not the flat attack bonus, which would make
crits scale with every attack source in the game), a plain hit, a **fumble**,
or a miss.

### A four-rung fumble ladder

A fumbled swing lands on one of four rungs, chosen by how deep into the fumble
band the roll fell. **Exposed** cuts your Evasion until your next turn.
**Recoil** turns half your own damage back on you. **Opening** gives the
target a free swing. **Crash** costs you your next action. Rungs replace one
another rather than stacking — a cumulative top rung is a run-ender — and a
free swing that itself fumbles resolves as a plain miss rather than chaining.

`Exposed` is available to content immediately: any species move can inflict it
from its `.ron`, so a debuffer species needs no engine change.

### Weapons carry damage ranges

Gear authors a `damage: (min, max)` band, and **a weapon overrides a natural
attack rather than adding to it** — a companion still rolls a species move
each turn for its name and its status rider, but the weapon supplies the
numbers. Every shipped move and every damage ability gained a `spread` around
its authored power, so damage varies rather than being a single number.
Modded content needs no editing: an omitted `spread` is the deterministic
value it always was.

Weapons and armour now trade along two axes each. Shiv Routine, Kinetic Edge
and Black Ice Pick take a narrow band plus **accuracy**; Monofilament Whip and
Plasma Router take a wide band and none. Sandbox Liner, Scrap Ward and Static
Mesh trade most of their mitigation for **evasion**.

### Defense is now Mitigation, and it is a percentage

`DEF` becomes `MIT` on every screen: percentage points of damage reduction,
summed from your species, your gear and your buffs, and capped at 75% so
nothing reaches immunity. It is the one stat **levelling never raises** — a
percentage that grows per level approaches immunity, so a zone tier and a
level-up both leave it exactly as authored, and levelling buys evasion
instead. A Recompile Kernel no longer raises it by a zone tier either.

### Every con colour and every kill's XP in the game has moved

This is a consequence rather than a side effect, and it gets its own line.
`Stats::power` — the "how strong is this" scalar behind the difficulty colour
a program's glyph is drawn in, the price of a kill's XP, and what a trader
pays for a program — now prices mitigation as the effective HP it buys
(`max_hp / (1 - mitigation/100)`) instead of summing a percentage into a
total, which was meaningless. Nothing about the world changed; what you are
told about it did.

### Also

- A weapon's damage band shows wherever a gear stat already did — the
  inventory tag, the equipped panel, the swap picker and a program's manifest
  — through one formatter, so no two screens can disagree about it.
- The gear-swap picker wraps a long row onto a continuation line instead of
  running off the popup, the same way the inventory list already did.
- Materialising inside solid substrate is lethal again. It goes through a
  named `kill_outright` rather than a large damage figure, because mitigation
  was leaving the player standing on a single point of Integrity.
- A missed Drain restores nothing, and a missed swing lands no status rider.
- Battle log lines report the damage that actually landed rather than the
  damage that was rolled, which differ once mitigation is a percentage.

## 0.11.9

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 30.
Both new save fields are additive on a field-named RON struct, so a save
written before this release loads as a program that has been developed
neither way.

### Companions can be developed past the level cap

A **Privilege Ring** drops from a lair guardian in the Stack, and from
nothing else. Spend it at the new **Develop a program** screen (party menu) to
open a **Kernel Ring** on one companion, raising *that companion's* level
ceiling by two — up to three rings, at one, two and three rings apiece, so a
fully developed program is six guardians' worth of descents.

A ring grants no stats, no level and no XP. It buys room; the fights still buy
the levels. A posted worker's cronjob XP is deliberately untouched, so a
developed program cannot be ground up at a Mining Node.

### Every level past the cap pays a talent point

Each level a companion earns above level 6 pays one point into its **class
talent tree**, spent on one of two choices in the next tier — six tiers deep,
one per level a fully ringed companion can earn. The four node kinds are a stat
percentage, a sharpened affinity for one ability category, a routine granted
outright, and one more routine slot. Points are derived from the level and the
list, so nothing can desync.

Fusing keeps the dominant parent's rings and talents — the parent whose species
and level the child takes — and loses the other's.

**The trees are moddable content.** `assets/talents/*.ron` ships one tree per
class plus a generic tree for a program with no readable class; a sixth class's
tree is a file, not a Rust change. See `assets/talents/README.md`.

### Also in this release

A field routine cast from a companion is charged to that companion's own Power
reserve rather than the player's — the fix that was sitting unreleased on the
branch this landed from.

### For anyone measuring

The arena's companion level clamp moved from `CREATURE_MAX_LEVEL` to the
absolute ring cap, because a scenario authors its own party and has no ring to
read. Five shipped `dev-arenas/` scenarios author `level: 12` and were silently
getting level 6 since the cap was halved; they now field what they say, so old
reports from them are not comparable to new ones.
`dev-arenas/developed-companion.ron` is the new scenario, and
`docs/measurements/2026-08-19-developed-companion-worth.md` is what it said.

## 0.11.8

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 30.
A biome was renamed, which sounds like a save break and is not: the old name
is kept as an alias, so a save written before this release loads and reads
back as the new one.

**One mod-facing schema change.** A sector file's `static_temperature` key is
now `deadlock_temperature`, matching the biome it names. The old key still
works — a sector written before this release applies its threshold exactly as
it did — but new sectors should use the new name. See
`assets/sectors/README.md`.

### The ground has a name, and past the first sector it does something

Terrain has been scenery since the game started. It decided what spawned on
it and what colour it drew, and that was the whole of it — the player could
not even find out what they were walking on.

Every biome now has a name, and crossing from one into another says so. That
much happens from the first step of a new run, in every sector, including the
first.

Past sector 1, three of them also do something. Deadlock queues you: a step
onto it costs an extra tick, and the world keeps running through it — a
production cycle finishes, a need decays, a wandering program gets one move
closer. Null Sector and Mainframe cost Integrity instead, a small fraction of
your maximum on every step, so crossing one is a supply problem rather than a
countdown. Open Grid, which most of the map is made of, stays exactly as it
was. Sector 1 is neutral ground throughout; it is where a run learns the
game, and ground that bit there would be a tax on the tutorial rather than an
exception to it.

The bite goes through the same path a hit in a fight does, so mitigation
applies to it — Ablative Layer is worth something on a long crossing. Only
the player takes it; the party is never touched. Terrain never costs Power
and never raises Trace.

### Ambient effects are a content directory

`assets/environment/` is the new one, and it works the way the others do: one
`.ron` file per effect, a malformed one skipped with a warning rather than a
crash, and a schema reference in `assets/environment/README.md`. A file names
the biomes it claims and one of two effect shapes. **Deleting the directory
restores the pre-effects game exactly**, the same supported way deleting
`assets/sectors/` does.

Three refusals are enforced at load, each protecting something a file has no
business revoking: the base slab may not be claimed — it is the one safe
ground in the game — and both magnitudes are capped, against ground that
kills in two steps and against a step that reads as a hang.

### The cold biome is called Deadlock

Static Field is now Deadlock. The rename frees the word "static" for a
weather layer this is the first phase of, and the new name says what the
ground is: allocations that never resolved and are still waiting.

## 0.11.7

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 30.
Nothing here touches what a save holds — one of these is a base-logic fix and
the other is a renderer change to what an existing screen prints.

### A work order sees the route its haulers already walk

Standing a Compiler and a Lathe on the slab could leave the Request row
missing from the base menu entirely, with nothing anywhere saying why. The
picker only offers an item some standing machine can actually make, and the
question it asked was whether a feeder for every ingredient stood
*orthogonally beside* the bench.

That was never the rule the base ran on. A worker fetches a missing
ingredient off a Depot shelf and walks it back, and the scheduler has staffed
a machine on the strength of that since Depots shipped. So a base with a
Mining Node, a Depot holding fragments, and two benches a couple of tiles
away was already able to run both of them — and was refused an order for what
they make, silently, because the machine that decides what to offer and the
machine that decides who to post were reading two different rules.

They read one rule now: a bench is fed by a neighbour, or, when a Depot is
standing, by any producer in the base. The walk that decides who to post
reaches through the same route, which is what keeps the order moving once the
shelf runs thin rather than stalling with nobody working anything. A base
with no Depot behaves exactly as it did, and the refusal still names the
ingredient nothing is making.

### An item says what it does, where you are looking at it

Seven of the shipped modules grant a passive routine while worn. Six items
permanently upgrade a companion. There are two consumables and a taming
catalyst. None of that appeared on any screen that *lists* items — an
inventory row said what the item would add to your stats and nothing else,
and the effect was visible only on the description page two keypresses
further in.

Every extra effect an item carries now prints on its own line under the item,
in the inventory, on a trader's three shelves, on a Stack market's sell list
and on the action screen you open a row into. A stat bonus stays where it
was, on the row itself, since it was never the thing that was missing.

## 0.11.6

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 30.
Nothing here touches what a save holds — this is a renderer fix to how one
screen lays its text out.

### A research node's description no longer runs off the page

The Research and Perks pickers each print a description under every entry,
and each printed it as a single line. The prose the assets carry runs to
about 240 characters; the popup those screens open in holds about 114. So
most of most descriptions was drawn outside the box — and silently, because
a popup row is clamped vertically and nothing clamps it horizontally.

Descriptions now wrap, at the same width the Recipes screen already wraps a
product's prose to, so the two screens cannot disagree about how wide the
game's prose runs. A wrapped description stays attached to the entry it
belongs to and scrolls with it rather than being torn off and pinned to the
foot of the box.

## 0.11.5

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 30.
Everything added here is asset data keyed by string — a new item, two new
structures, a research node — and a save stores those ids, not their
definitions. A patch for that reason: while the project is `0.x`, "breaking"
means a save that stops loading.

### A breach unlocks a material, not just a bigger number

Breaching used to change only the *rate* on the two raw materials: the Mining
Node has produced Core Fragments and nothing else since it shipped, and a
zone's contribution was a bonus per cycle. Cache Coherence, researchable once
you have breached, unlocks the Cache Tap — and Cache Grain is what the second
sector and everything past it is built out of.

It layers rather than replaces. Core Fragments remain the everyday currency
and stay extractable forever, because every recipe still denominated in them
would otherwise strand. Unlike the currencies, Cache Grain survives a breach:
a run arriving in a new sector holding none of it could not upgrade anything
until it had re-tapped.

Three things ask for it. The Line Driver is a new build that feeds the grid
harder than a Heap Pillar and claims no ground doing it, which is what keeps
a base growing once all five Pillars are standing. Every structure upgrade
past the first tier now wants it — free in zone 1, where the tier ceiling
already refuses every upgrade. And the six research nodes that hand over a
gear recipe now denominate those recipes in it, so zone-gated gear is made of
the zone's material.

### A companion's buff no longer runs off the panel

A routine running on a companion drew its holder's name on the same row as
the routine, and that row ran 360px past the edge of the map's status column
— silently, because rows there are clipped vertically and never
horizontally. The column holds 38 characters and the widest routine row
already spent all but four of them, so there was never room to shorten the
tag into. The holder now draws on a dimmed line of its own beneath the
routine, and the battle screen's copy of the panel is unchanged, since that
one measures itself and can simply grow.

A truncated list also counts routines again rather than lines: "+2 more" had
been about to mean two hidden rows, which is one hidden routine.

### One kind of attack

A wild program's turn has always branched two ways — cast a Special, or swing
a basic attack — and the two were different kinds of thing in the code for no
reason anyone had written down. A basic attack is now an ability like any
other, converted from the species file at load. Nothing about play changes:
every seeded fight in the suite plays out move for move as it did, and no
species file needed editing, mods included.

What this does not do is change how a basic attack's damage is worked out. It
stays flat authored power, where a Special scales with level and species
affinity — merging those would make every enemy swing scale too, which is a
difficulty change rather than a tidy-up, and one to argue on its own evidence.

## 0.11.4

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 30.
`Platform::claimed` is additive behind `#[serde(default)]`: a save written
before ground claims existed loads with an empty claim set, which is what it
had. A patch rather than a minor for that reason — while the project is `0.x`,
"breaking" means a save that stops loading, and this has none.

### A base that grows a tile at a time

The Heap Block claims one tile of ground at the platform's edge instead of
standing a structure on it. Where the Heap Pillar is the tool for area — a
whole ring in every direction — the Block is the tool for shape, so a base can
be run out along a ridge or squared off around a Stack link rather than only
ever growing as a circle. It costs one Blank Substrate and sits behind Page
Allocation, a cheap early research.

A claim has to touch the base, and it **refuses** a Stack link, a nest or a
hostile rather than taking the ground out from under them the way a Pillar's
ring does. Paid-for ground travels with the base on a breach.

`StructureDef::claims_ground` is the data half, so a modder ships a
ground-claiming build with no Rust. The footprint half is stored rather than
derived, because a claim leaves no entity to count it back from — `covers`
stays the one statement of where the base is.

### The Pillar feeds the grid

A Heap Pillar now supplies grid energy while it stands, and costs six Blank
Substrates alongside its fragments. Growing the base is also how the base is
powered, and both growth tools now run off the Lathe a base already needed for
Routine Disks.

### Every log pane folds a repeated line into one row and a count

A round that wipes seven programs used to push "The rogue program crashes and
deletes itself!" seven times. The line names nobody, so seven copies say
exactly what one and a count say. The history screen already folded repeats;
the map pane and the battle pane now do too.

A wipe still takes one beat per kill, and what you watch is the count ticking
up — the fold sits on the rows about to be drawn, after the truncation, because
the battle reveal, the unread-line count and the battle roster replay all count
raw lines. Seven deaths would otherwise read as one.

## 0.11.3

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 30.
`CreatureSave::boss` is additive behind `#[serde(default)]`, which under
field-named RON costs no bump: a file written before rolled bosses existed
loads with every creature un-bossed, which is what it was. A patch rather
than a minor for that reason — under the policy above, "breaking" while the
project is `0.x` means a save that stops loading, and this has none.

### Which species a zone or a depth may field

A species' danger band is now derived from its `growth_multiplier`, and each
band is eligible only inside a window of danger steps — the zone number on
the surface, the frame depth underground, the same scalar the group-size
curves already take. So a fresh run meets the gentle end of the ladder and
nothing else, the middle band takes over a few zones in, and the hardest band
arrives last and never leaves. It used to be possible for a level-1 player to
walk seven tiles off their landing site and meet the toughest program in the
game.

No asset changed. The band is derived rather than authored, for the reason a
species' class is: a rung is a fact about numbers the species already
carries, and a second authored field is a second thing that can disagree with
the first. A modded multiplier between rungs snaps to the nearest and is
never refused.

Where a biome has a hole in its ladder, the pool falls back to the nearest
band that biome does hold, so no biome is ever empty. That fires against the
real roster at both ends — Static Field ships no easiest-band species and
Open Grid no hardest-band one — and the honest fix for either is a species
file, not a wider window.

### Any species can spawn as a boss

`is_boss` in `assets/species/*.ron` now marks an *apex* species: always a
boss, never scaled by the engine, and eligible only deep into a run. Boss-hood
itself became a per-individual roll available to every species — outside the
opening ring a spawn can come up a boss, and where no apex species is
eligible yet it is an ordinary one scaled up instead. Bosses now arrive early
and easy, late and hand-authored, rather than being two fixed programs you
either had met or hadn't.

Neither kind rolls a rare tier, since the boss multiplier is the whole of
what one is worth and a tier on top would be a second, invisible one. The
opening ring refuses a boss the same way it already refused a rare tier.

A lair guardian is drawn from the same window at its own depth and is always
a boss. That closes a standing trap: a guardian in a biome with no apex
species used to come back not-a-boss and pay no Portal Fragments, leaving a
stack unbreachable in everything but name.

## 0.11.2

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 30.
`ItemDef::grants` is asset schema behind `#[serde(default)]`, and the passive
it names is **derived at fire time** off the wearer's `Equipment` rather than
written into `Routines` — so nothing about a granted routine reaches a file,
and taking the item off ends it by omission. `PopulatedChunks`, below, is an
additive save field behind the same default: an older save carries no marks
and stocks the ground around wherever it left the player.

### Gear that carries a routine

Every passive routine in the game was a boss drop, which made `exclusive` and
`triggers` look like the same idea. They never were. An item may now name a
routine it grants while worn, and eight scavenged pieces do — three weapons,
two pieces of armour, three modules — carrying six new passives between them.
A granted routine costs no turn and no routine slot; the item is what was
spent.

Two new triggers come with them, each with its one call site in
`battle_resolve_round`. `RoundStart` fires as the round opens, which is the
trigger that fires every round there is, so all three routines on it pay a
four-round cooldown. `AllyWounded` fires when a party member crosses
`tuning::WOUNDED_INTEGRITY_FRACTION` — a third of Integrity — **downward in
one round**, which is a crossing rather than a level: a fight that grinds
somebody down slowly never fires it, and a burst that drops them does.

`AllyDropped` stays reachable by exactly one item, deliberately. A dropped
companion is dissolved with no revive at any difficulty, so a routine paying
out there only pays a player who has already lost more than the payout is
worth.

Gear is wearable by any owned program, so a companion carries a granted
passive as readily as the player does — and unlike a Special, a passive costs
no turn, so it is the first companion-side ability effect that fires without
anybody choosing it. The item describe page names what a piece grants.

### What the eight items are worth

Measured before the merge rather than after, against 100 paired fights per
item, controlled by the item's own `grants` line and validated against the
shipped stat twins: `docs/measurements/2026-08-18-gear-passive-worth.md`.

Nothing in it moved a number, which is the result. `WOUNDED_INTEGRITY_FRACTION`
fires in 5% of the runs of a fight the party wins easily, 25% of the fights
they win on curve, and **100% of losses in every band** — neither dead nor a
slower `RoundStart`. Uptime on a four-round cooldown measured 0.230 firings a
round against a nominal 0.250, and it does not front-load.

Two things the run found that nobody had asked about. A bare `def: 2` module
with no grant at all is worth more at level 12 than the strongest grant
measured, and worth **exactly nothing** at level 36, where that same item's
grant ends fights — so the two halves of a granting item invert in importance
across the level range. And `deadman_relay` is not merely comparable to the
etched disk that carries the same routine, it is identical: 100 of 100 reps
agreed on outcome, rounds, Integrity and companions lost. Which of them is
worth carrying is a question about slots, not about effect.

**None of it was played.** `balance_sim` models no abilities and gates none
of this, so the arena and a session are the only instruments, and only the
first has been run. Whether three passive lines in a round read as your gear
working or as noise in the log is still open.

### The sector is populated where you go, not where you have been

Wild population was placed only relative to the player — a one-time seeded
disc around the arrival point, plus a 5%-per-tick roll within twelve tiles of
wherever they stood. The map is unbounded and generated a chunk at a time, so
no finite seed could cover it, and the roll could not fill space either:
walking costs a tick a tile, which buys about one spare roll per density box
against a target of twelve.

Measured before: zero to two programs per box across 240 tiles of walked
ground, and a clean halo around the base — 15 at the player, then 10, 6, 3, 1
at 25-tile intervals. After: 8, 13, 12, 12, 12, 15, 14, 14, 14, 13, 8, 12
across the same 300 tiles.

Population is a property of place now. `PopulatedChunks` records which world
chunks the sector has stocked and `ensure_local_population` stocks any chunk
within one of the player's, so ground arrives populated a chunk ahead of the
pane rather than filling in behind you. `WILD_CREATURE_CAP` stops being
decorative: its cull is shared by both placers and evicts whole chunks,
farthest first, taking the mark with the creatures.

### Two refusals in the Stack

Pressing `o` on an orphan with no ICE Breaker reported as "nothing happens".
The key was bound and the engine was refusing correctly; the refusal was
borrowed from a battle row, where a lowercase fragment sits under a greyed
option — alone on the status line for four seconds it reads as no response at
all. Both refusals are sentences now, and the underfoot row names the missing
item instead of advertising `[o] adopt` regardless.

A full roster was the same bug wearing the other obstacle: a party holding a
catalyst but no room still read `[o] adopt` and only learned otherwise by
pressing it. Both obstacles go through one `Game::adopt_block` that the key
refuses on and the row warns with, so the offer and the key cannot drift —
which is the invisible kind of drift, the row going on offering while the key
quietly refuses.

## 0.11.1

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 30.
The two new structure fields are asset schema behind `#[serde(default)]`, and
`MachineStatus` — which gains a variant here — is never serialised at all: it
carries no serde traits and appears nowhere in `save.rs`. A save written
before this release loads, and the first tick decides which of its machines
are dark.

### The base power grid

A base had no reason not to sprawl. Every structure was bounded by its build
cost and by `MAX_BUILD_DISTANCE_FROM_HOME`, both paid once; a structure that
existed cost nothing to keep. So the optimal base was every machine you could
afford, and the only ongoing decision a base asked of you was where to post
programs.

Machines now draw on a grid, and the grid has to cover them. Every tick the
base sums what its structures supply against what its machines draw, and if
the draw is over, machines are cut in `(x, y)` order until the rest fit. A cut
machine reads **dark** — it makes no progress on a cronjob, an assembler
neither works nor pulls from its neighbours, and hand-working it yourself
yields nothing either. The base pane header reads `Grid  15 / 16`, red when
short.

The cut does not stop at the first machine that will not fit: a 3-draw machine
that cannot fit a 2-unit budget goes dark while a 1-draw machine behind it
still runs. Stopping early would darken an arbitrary tail. The order is
arbitrary but it never changes, so the far corner drops first and you can lay
out around it.

**A machine draws whether or not anyone is posted to it.** The building is
plugged in; the worker is a separate question. That makes an idle machine you
keep around "for later" a real expense — demolish it or power it — and it is
the only version you can plan against, since supply covers the base you built
rather than the base you happen to have staffed this minute.

The Recharger Node is what settles this. It was a one-time purchase that
deleted a need meter and then became furniture; there was never a reason to
build a second. It now supplies 4 to the grid, as Home does, and there is no
cap on how many you build.

**This is called the Grid, never Power.** `Power` already means a program's own
reserve, two panels away in the status column — the Recharger still trickles
that back, and the two are separate resources that happen to share a word.

**The numbers are unmeasured.** `balance_sim` models battles and has no
base-production term at all, so it gates none of this. Home supplies 4,
each Recharger 4; extractors draw 1, base assemblers 2, late assemblers 3.
They are a starting point for a session, not a tuned claim, and the first
thing to change if play says otherwise.

Three of the six `dev-saves/` templates stood more machines than a single
Recharger could carry and are given the Rechargers to match — `chains` and
`contracts` run 15 against 16, `deep-lair` 17 against 20. A new gate keeps the
next template from being captured short.

## 0.11.0

*`0.10.0` is skipped. A stray tag of that name was pushed onto a branch
commit that carried no release, and moving a published tag is worse than
spending a number.*

**Existing saves will not load.** `save::SAVE_FORMAT_VERSION` goes 29 → 30:
`PlayerSave::fatigue` is a field *removed*, which field-named RON does not
save you from, and `PlayerSave::hunger` is renamed to `power` in the same
bump. `CreatureSave::power` is additive and rides it. The six `dev-saves/`
templates are hand-edited to match.

### Power replaces Fatigue

The game had two need meters and only one of them was a mechanic.

**Power** drained at 0.15 a tick, was the only thing that could kill you by
attrition, and scaled your attack down below 50. **Fatigue** *refilled* at
0.08 a tick and was spent by exactly two things in the entire game — the
Stack's Phase and Jump. Battle Specials stopped charging it on 2026-08-08.
It was a meter, a save field, a `FieldBuffKind`, a `ConsumeDef` field, a
serde default and a row on the status bars, all in service of two routines.

Meanwhile the thing you actually spend all game — calling routines — was
priced only in cooldowns, which are a pacing device rather than a budget.
A cooldown says "not again yet". Nothing said "not any more".

Fatigue is gone. Power is now what every routine call draws on, alongside
the cooldown it already had, and **every companion holds its own reserve**.
A companion's Special is paid out of the companion's Power, so a party's
casting is something you manage rather than a free action that fires
whenever it is off cooldown. A companion at zero Power falls back to plain
attacks; nothing about an empty reserve costs it Integrity.

Scarcity stays soft on the surface and bites underground, and that is a
property of the code rather than a rule anyone maintains: a Recharger has a
radius, and a base is where Rechargers get built, so a Stack run carries
whatever the party walked in with. **That depended on a bug fix.**
`power_regen_system` reads the player's `Position`, which is pinned to the
surface entrance tile for the whole of a Stack run — so a link sited inside
a Recharger's radius refilled the party four frames down. Harmless while
nothing underground spent Power; it would have deleted the feature outright.

`Game::rest` refills Power, for you and for every program you own. That is
the sole refill, and it gives the party's casting budget the same
base-bound shape as everything else.

Two things did not change, deliberately. Hostiles get no reserve — their
policy weights were trained against today's action distribution, and a Power
constraint would cost a retrain nobody would see the benefit of. The wielded
program's 25% proc stays free, because that rate is already its whole price.

### No costs were authored for this

The numbers were already in the files. `AbilityDef::fatigue_cost` was
documented in three places as reaching only Phase and Jump — true about what
the engine read, and false about what the assets contained: 55 ability files
carried a cost nothing consumed, priced back when the field meant exactly
what it means again now, and 10 more carried one inside their `FieldBuff`
effect. The content pass was a key rename with the values untouched,
verified by diffing the sorted multiset of all 65 numbers rather than by eye.

The serde default moved from 5.0 to **0.0**. The old number was the price of
commanding a companion, a mechanic that stopped charging in 2026-08-08, and
it survived only because the field reached two routines; keeping it while
widening the field to every ability in the game would silently price every
ability a mod ships. The five uncosted files are untouched and stay free —
`priority_boost` most of all, since it is the fallback every companion has
when its species grants nothing.

`tuning::ROUTINE_POWER_COST_MULTIPLIER` scales the whole curve at once. The
shipped values' *ordering* is worth keeping and their *scale* is inherited
from a pool that refilled itself, so 1.0 is a starting point rather than a
measured answer. `balance_sim` models no abilities, so none of this is
gated by the balance suite — the arena and a session are the instruments.

### Trickle Charge, retuned

With one need there is one per-tick restore kind, so `Coolant` merges into
`Trickle` and `coolant_flush.ron` is deleted — the two were the same ability
once Fatigue was gone. That leaves Trickle Charge as the only source of
Power underground, and it was retuned rather than left at its Fatigue-era
numbers: 80 turns at a cost of 20 becomes 60 turns at 25. One cast buys back
about a quarter of a reserve and takes 60 underground turns to collect,
which is a real Trace and encounter cost — a sustain rather than a tap.

Its *scaling* was the larger find. Trickle ran its authored magnitude
through the caster's level, so `power: 1` is 7 a turn at the level cap:
Power pinned at full for the buff's whole duration, and an authored number
the level term swamps. It no longer scales, by the rule that arm of the
match already stated — Regen's ceiling is `max_hp` and grows with level,
Power's is a fixed 100 forever.

### Field routines run until you rest

Eight of the ten out-of-battle routines no longer count turns down. Overclock,
Hardened Shell, Ablative Layer, Long Winter, Deep Scan, Trace Analysis,
Salvage Routine and Stealth Protocol run until the party rests, and a
Forgiving reboot is the only other thing that ends one. That turns them from
something you cast just before a fight — timing an 80-turn window against a
walk you can't predict the length of — into an expedition loadout you buy at
base and carry until you come home.

Repair Loop and Trickle Charge keep their counters, and the line is a rule
rather than a list: `Regen` and `Trickle` are the only kinds with a per-tick
effect, so an until-rest one is unbounded healing or unbounded Power, and
Power underground is the whole of the Stack's scarcity. They are also the only
two that use `interval`, whose cadence is phased off the very counter an
until-rest buff no longer has.

An *item's* buff still keeps its own clock whatever kind it arms — a Patch
Routine is spent when you use it, where a routine can be run again on the next
charge. That distinction is why the rule reads the buff's source and not just
its kind: a consumable and a routine of one kind stack rather than displacing
each other, so a permanent item buff would have compounded under the routine's.

For modders, `duration` on a `FieldBuff` is now decided by `kind`, and both
mistakes are refused at load with a named reason rather than resolved
quietly: a duration on a kind that ignores it, and a missing one on a kind
that counts (which armed at zero and expired on the turn it was cast — silently
possible before). Buff rows show `rest` where they used to show a tick count.
No save-format change; a buff already running in an old save simply stops
ageing.

### Hardened Shell, for the whole party

Hardening the party meant casting Hardened Shell Single four times: four
turns, and 56 Power for a full roster. **Hardened Shell Party** is the same
+4 DEF on everyone off one cast for 32 Power, taught by a new **Mesh
Plating** node — zone 3, 120 Research Data, hanging off Adaptive Plating.

The price is the whole design, and it is bounded from both sides: above one
Single, so the narrow cast is still what you reach for when only one program
is going to get hit, and below covering the party a Single at a time, or
nobody would ever run the wide one — the only thing it would still buy is
turns, and turns are free. Both bounds are asserted against the shipped
assets as a *relationship* rather than as three numbers, so a Power retune
moves all of them freely and only an inversion fails.

Data only. `Long Winter Party` was already a whole-party field buff, so
nothing in the cast path changed — this is two `.ron` files, which is what
the moddability rule is for.

## 0.9.3

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 29
— `CreatureSave::nemesis_grudges` is additive and defaults to 0, so nothing
written before this release loads any differently.

### A program you lose to remembers you losing

Every wild program used to be interchangeable. Win or lose, the encounter
left nothing behind — a fight you bailed out on was indistinguishable a
minute later from one that never happened.

A creature that beats the party, or that the party jacks out on, is now
marked a **nemesis**. It gets a name on the spot, drawn from
`assets/nemesis/names.ron` and salted off its species and stat roll so two
nemeses of the same species read as different individuals. Meeting it again
opens the fight with a taunt of its own, from `assets/nemesis/taunts.ron`,
picked from a flat pool by folding the grudge count into the same seed —
so a longer history tends to land on a different line rather than climbing
some tone ladder. A grudge promotes the creature's `Rarity` a rung and fully
heals it, so a nemesis is not just a name — it is a program you are actively
losing ground to. The ladder is bounded by `Rarity::ALL` itself (2.15x
cumulative, decelerating), and a hard cap of ten held nemeses at once refuses
a new mark once full rather than bumping one out.

Both marks read anywhere the creature does: the map draws a corner mark
beside its rarity bar, and its glyph draws in a reserved colour regardless
of the usual con read — the one exception being a program that is also a
zone boss, where the nemesis mark wins. You already know what fighting this
one costs; that is the whole justification for spending the "can I win this
fight" read on its tile.

A breach clears every nemesis in the zone, the same as every other wild
hostile — there is no separate ledger for the feature to leave behind.
Rewards flow entirely through the promoted `Rarity` and the existing
challenge-scaled XP curve; nothing new was added to either.

### Contracts meet a new run where it starts

Contracts were meant to onboard a run and were sitting behind the tree they
were supposed to introduce: the Contract Broker wanted a 10 Research Data
node and 14 Core Fragments before it could be built. The Contract Brokerage
research node is deleted rather than zeroed — a structure no research file
names is unlocked by default — and the build drops to 5 Core Fragments, so
the Broker is buildable from turn one. An old save that had already
researched Contract Brokerage still loads; the id simply names nothing now.

The board itself was a coin flip. It draws three slots uniformly out of
everything eligible, and a zone-1 pool is nine authored contracts plus up to
five rolled, so "deliver twenty-five Core Fragments" was as likely to be a
new run's first job as anything. Seven **starter** contracts now lead the
queue in sector 1 — a first kill, a first delivery, a Mining Node, a Research
Node, a Recharger Node, one frame down a stack, and the breach into sector 2.
Past zone 1 a starter is still offerable, just no longer ahead of anything,
so a mid-run board is untouched and no seeded board moved.

### The board reads anywhere, and signs on the base

One call was answering two different questions — whether there was a board to
draw, and whether the player could act on it — measured as two tiles from the
Broker's own tile. So a player who had built the thing could not read their
own mission status from across their own base, and the board went dark four
frames down a Stack.

`Game::broker_reach` splits it into three states: no Broker, off the base, at
the Broker. The offers list wherever the party is standing, underground
included, because a board seeded off the sector and the epoch makes no claim
about where anyone is — there was never anything for distance to invalidate.
Accepting and delivering still require standing on the base, and the screen's
header says so rather than leaving you to press a key and read a refusal.

"At the Broker" now measures the base slab rather than the distance to the
Broker. A Broker is on the slab by construction, so its tile carries no
information the slab does not, and `CONTRACT_BOARD_RANGE_TILES` was deleted
rather than widened — a constant there would have frozen the desk at the
radius a base *starts* at, while a base's footprint is derived and grows.

### A staff row says what a program is worth at a post

The Base Staff screen's decision is who the scheduler may draw on, and the
row left you to judge by name. Each row now names the program's cycle speed,
extraction aptitude and base job, with its current activity on a continuation
line beneath. `Game::work_profile` answers all three off the same
`SpeciesDef`, so the walk from an entity to its def is written once; the
class label goes through the manifest's existing exhaustive mapping, so a
sixth class cannot ship without deciding what it does at a post.

### Fixed

- **The `0` key crashed any menu that was open.** The digit-to-row conversion
  guarded itself with `d >= 1` but wrote the subtraction inside `then_some`,
  which takes its argument by value — so `0usize - 1` was evaluated on every
  `0` keypress whether the guard held or not, panicking the menu and taking
  the renderer down with it. `checked_sub` makes the absence of a row `0`
  structural rather than a guard someone has to keep in step.
- **A feed buffer needs a neighbour with a reason to run.** A Lathe standing
  beside a Mining Node counted as an attached building on its recipe alone,
  so the node hoarded its whole twenty-unit buffer for a machine that pulls
  nothing while unstaffed. Attachment now means the base has a reason to run
  that assembler — the work-order queue naming what it makes, or a standing
  work job on it. With an order in for Core Fragments, the first fragment
  reached the Depot at tick 500 against the first cycle for the same node
  standing alone.
- **Teardown's description names the salvage it actually adds.** "Work
  resource" is the name of a `SpeciesDef` field, not a word the game shows
  anywhere else, and it said nothing about what a kill drops. It now names
  Core Fragments, and the line comes back under `ROW_WRAP_COLUMNS` — nothing
  clamps a popup row horizontally, and the old 172-column line ran well past
  the body.

## 0.9.2

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 29.
A save written before this release keeps its level and its banked XP; what
changes is the price of the next level.

### XP measures how hard the fight was, not how big the target was

Levelling was too fast, and the reason was that a kill paid out its victim's
HP bar. A bar is a property of the thing you hit, not of the fight — so the
cheapest way to level was to find the fattest thing you could already beat
without effort and keep beating it. Four fights three frames down a Stack
could hand over five levels.

A kill is now priced by challenge: the victim's bar, scaled by how it
compared to you. The scale is the same one the con colour has always used to
tell you whether something was safe, so what the screen calls a hard fight is
now literally what pays more, clamped between a quarter and double. Something
far beneath you still pays *something* — the floor is there so a player with
nowhere harder to go is slowed rather than stopped — and something far above
you cannot pay unboundedly, because the ceiling is what stops one lucky kill
from skipping a tier.

The denominator is your own power alone, not the party's. Counting companions
would dock you XP for recruiting them, which is the opposite of what the
roster is for.

Measured against the real roster: reaching level 5 in the opening zone costs
about 34 kills where it used to cost about 6. Grinding zone-1 drones from
level 5 to 10 costs 208 kills — the dead end, and deliberately so. The same
stretch costs 37 kills four frames down a zone-1 Stack, which is the way out
of it. Playing on curve, at the level a zone actually asks for, settles at
about 4 kills a level. The numbers and how to reproduce them are in
`docs/measurements/2026-08-15-challenge-xp-pacing.md`.

### Levels come at half the count and twice the size

Slowing XP alone would have made every level worth the same as before while
arriving half as often, which is just a longer game. So the ladder was
rebuilt around it: there are half as many levels to a run, and each one
grants twice as much. Every per-level constant carries that factor, and the
XP a level costs carries its square, so the *power* curve is unchanged —
you are as strong at a given point in the run as you were before, you simply
got there in fewer, larger steps.

Species ability unlock levels moved into the same currency, so a routine
still arrives at the same point in a program's growth rather than at half of
it. That is a data change across all seventeen species files; the schema note
in `assets/species/README.md` says what the levels now mean, and a mod's own
species files should be halved to match.

Both halves of the claim were checked by instruments rather than argued:
`balance_sim`'s reach curve halved while staying linear, and the ability
magnitude pin reproduced its existing band at half the level with the band
itself untouched.

### Fixed

- **A level's XP threshold is derived on load, not trusted from the file.**
  It was already being written into the save and read back out, which meant a
  save written under the old curve would have carried the old threshold into
  the new one. Both load paths now compute it from the level, so the field is
  written and ignored — removing it is what would cost a save-format bump.

## 0.9.1

### Screens

- **The manifest goes back to full size.** 0.9.0 drew the sheet at
  two-thirds scale so it would read as a panel; it reads worse. The frame is
  back at 92% x 90% of the window and the page is back on the window's own
  metrics, so `manifest_layout` no longer scales anything or hands metrics
  back to the caller. `M` on the roster is untouched — that half of 0.9.0
  stays.

## 0.9.0

**Saves written by 0.8.x still load.** `save::SAVE_FORMAT_VERSION` is
unchanged at 29, so by the rule above this would ordinarily be a patch. The
minor is deliberate: seventeen changes landed on `main` between the 0.8.35
release and this one without being cut individually, and one number covering
the lot reads more honestly than back-dating sixteen sections. The
per-change rule stands; this is the correction, not a new practice.

### The Stack describes itself as you walk it

The description bank shipped with three lengths of prose per subject, and
the one-sentence length had a single reader: the line that fires the first
time a feature comes into view. So a plain corridor said **nothing at all**
— the floor and doorway prose was written, shipped, and unreachable — and
ground already on your map was silent forever after, which made walking back
through a frame a wordless trudge.

There are now two narration axes. **Discovery** is unchanged: the first
sight of something worth walking to, once ever. **Passage** is new, fires as
you arrive somewhere, and describes whatever your line of sight settles on
ahead — with no notion of *new*, so a corridor you have walked ten times
still has something to say about itself.

Both resolve through one pick, the same one the examine key uses, so the
corridor cannot announce a cache that `x` then declines to name.

**Which cells speak is a property of the place**, folded out of the frame
and the coordinates rather than rolled — so a corridor keeps its rhythm
across a save and reload and on every later walk, and opening a log line
costs nothing from the run's shared randomness. Roughly one cell in three,
which is a first guess at a feel question.

A **live Stack market** is now announced like any other find. It was the one
unspent feature missing from that ranking, so nothing ever mentioned a stall
and the examine ray looked straight through one.

### Work orders: the base fetches, delivers, and finishes

The base that started staffing itself in 0.8.35 was still hoarding and
still hand-making things it already owned.

**A worker fetches from a Depot before making the thing.** The scheduler
worked deepest-first, so a feeder outranked the bench it fed and your one
spare body went upstream to hand-make what was already sitting on a shelf.
A full batch on a shelf now settles it. One statement of "is a batch of this
in reach" answers for both the scheduler and the walker, so a bench that
gets staffed is a bench that gets fetched for.

**A machine with nothing downstream delivers as it produces.** Pickup used
to wait for a clog, so a lone extractor sat on twenty units the base could
not count. If no neighbour's recipe wants its output, the worker walks each
cycle's payout to the nearest Depot. Depot placement now paces a lone
extractor — deliberately, and the number most likely to want retuning after
play.

**A work order staffs every machine that makes the item, not one.** Deploy a
second Mining Node because the first one's output is eaten by the assembler
beside it, and it used to stand empty for the rest of the run. The mirror
failure went too: an unfed twin bench in a corner no longer refuses an order
the base can already fill through a line that is whole.

**Cancelling an order mid-walk no longer destroys the load** the worker was
carrying, and a **finished order reads green** in the base log instead of
sitting dim among the routine payouts.

### The Stack

- **An escort's death no longer collapses the stack.** Cut down one of a
  guardian's escort, jack out with the guardian untouched, and the lair was
  spent and the whole stack folded on the way out.
- **A guardian can no longer be walked off with.** Taming one recorded
  nothing, so the lair refilled on the next visit over a stack that could
  never be finished — and the guardian could be farmed indefinitely.

### Battle

- **The battle log can be walked back.** Narration that outgrew the pane was
  gone from every screen in the game — a round runs to 18 lines against a
  pane seating about 15, and the results page lands the salvage tally and
  every fighter's XP in the same space. Up and Down now walk the window on
  both the battle screen and the results page, with a hint naming what is
  out of sight.
- **The summary reports one decompile verdict, not one per catalyst.** Six
  catalysts fed to one program left six near-identical refusals on the
  results screen, only the newest of which still said anything true.

### Screens

- **The manifest reads as a panel** at two-thirds its old scale, and **`M` on
  the roster** opens the highlighted program's sheet — Esc comes back with
  that row still highlighted.
- **A roster row and an inventory row shed their tags** onto an indented
  continuation instead of running off the popup. The worst roster row ran
  382px past the edge, taking the activity and CRITICAL marks with it; the
  worst inventory row ran 68px over, taking the whole answer to what the item
  would do if you put it on. Ordinary rows are untouched.
- **The last perk's description is no longer torn off the list.** It was
  drawn detached at the foot of the box under a blank scroll indicator. The
  research picker had the identical defect and gets the identical fix.
- **An idle program stops claiming to be in your party** on the Base Staff
  screen. "Neither staff nor party" is the state a program is tamed into and
  the one it is stood down to — the common case read as the one it was least
  likely to be.
- **A party drain narrates green** like a patch, keeping its numbers. It is
  the one line in the game with a figure on either side of it.

## 0.8.35

### Work orders: the base staffs itself

You used to staff the base one program at a time, forever. Open the base
menu, pick a program, pick a structure, repeat — and then repeat it again
every time a machine clogged, a worker was wanted elsewhere, or a chain
needed its upstream link fed before its downstream one could run. That is
not a decision, it is bookkeeping.

**Now you say what you want made.** "3 Routine Disks" — and the base works
out that this means Core Fragments through a Lathe into a Disk Press, and
staffs itself in that order with whoever you have given it.

An order is a **target level, not a production run**: three already sitting
in a Depot satisfy it on the spot. Completion is measured across Depots and
machine output buffers, because what the base *holds* is the question and
where it holds it is not.

**A work order stores an item and a quantity, and nothing else.** No
per-machine plan, no unit targets, no progress counters. Which machines a
line needs, in what order, and who is on each is recomputed from live world
state every tick — the same call this game already makes for the build
radius, a Broker's board, a Stack description and the wielded program's
bonus. The status screen runs the scheduler's own walk, so what you read is
what the base believes by construction rather than by a comment claiming
the two agree. Multiplying the recipe tree through at queue time would have
produced a tidier progress bar and a plan that is confidently wrong the
moment a machine is demolished.

An order is refused when it is placed if the line can never move, and the
refusal names the break: the machine you have not built, or the link with
no feeder beside it. Research is refused too, and not as a special case —
Research Data is banked as it is gathered, so it reaches no output buffer
and nothing can hold a stock of it. The Research Node is still staffed, as
a standing job.

### Base staff

Programs are now on one side or the other: in your party, or in the base.
The two are disjoint sets drawing on one roster, which makes explicit a
choice that used to be a side effect of a menu action — every program at a
machine is one absent from a fight.

Staff with nothing to do loiter on a ring around the Home instead of being
invisible, and `x` can name them, because what the map draws and what the
inspector names is one rule.

### Standing jobs

A machine can be told to keep running, and a structure to keep a guard,
whether or not an order asks for it. Standing jobs are filled only by a
body no order needs, and give that body up the moment one does — which is
how a Research Node gets worked at all, and the only way a guard post
survives the sweep that makes it worth having.

The toggle lives on the structure roster rather than in a menu of its own:
the question "should this machine always be running" belongs to the
machine.

### What was removed

"Assign a cronjob" and "Post a guard" are gone from the base menu, and the
engine methods behind them are gone from the engine. **Working a structure
yourself is untouched** — you are not staff, and the scheduler never moves
you.

An existing base keeps its workers. Any program holding a job when a save
loads is absorbed into the staff pool rather than being stood down, and
every new field is additive and defaulted, so **no save-format bump** and
nothing to migrate.

### Not gated by anything automatic

`balance_sim` is RNG-free and models no base at all — it cannot see a work
order, a posted program, or a production chain, and the arena models player
combat. Nothing here is covered by an automatic balance gate, and the
question it raises — whether automating staffing makes the base too
productive for the zone curve — is answerable only by playing it.

## 0.8.34

### Three bugs where an index stood in for the thing it named

All three were the same mistake wearing different clothes: a number that
identified something by *where it sat* rather than by *what it was*, and
then the thing moved.

**An attack aimed at one group landed on another.** A planned action stored
its target as a plain index into the enemy line, but a group that dies is
dropped from that line the moment it dies, sliding everything behind it down
one. Aim at the second group, watch a faster party member destroy the first,
and your strike landed on the third — whoever had moved up into the slot you
picked. The same fault at the tail end sent a plan aimed past the end of the
line onto the front group instead, which is where it was most likely to be
wasted on something already dying.

A plan names a group, not a slot. It now resolves against the line as it
stood when the plan was made, so the aim follows its group to wherever that
group is now standing, or the turn is spent and nothing is hit. Spending it
is deliberate: a heavy hit or a decompile going off on a target nobody
picked is worse than a wasted turn, and a decompile in particular could burn
an ICE Breaker on the wrong program entirely. The wielded program's proc
answers the same aim as the strike that triggered it, rather than the
shifted one.

**Bumping a pack dragged in any boss standing near it.** Assembling the
enemy side of a map fight sweeps in every hostile within a short radius of
whatever you walked into, and that sweep had no idea what a boss was. Bosses
roam the open map like anything else, so one that happened to be loitering
beside an ordinary cluster joined the fight — which is the opposite of what
being a boss means, since a boss is precisely the thing that arrives as its
own fight with its own escort. It is now left out of the sweep. Walking into
the boss itself still fights the boss, and the escort it brings with it is
untouched.

**Creatures spawned onto ground they could never leave.** A spawn checked
that its anchor tile was walkable and then scattered the rest of the group
around it without checking anything, while a creature's own movement is
stricter than plain walkability — it refuses rock, and it refuses your base
slab. A pack member or nest guardian scattered across a biome boundary
therefore had no legal move for the rest of the run, and a nest guardian's
tether meant nothing could ever push it off, either. Both now fall back to
the anchor tile when the scatter lands somewhere unusable.

That last one moves creatures around, which shifts the random stream a
little further downstream. Nothing about a save is affected, and existing
saves load unchanged.

## 0.8.33

### Ten more affixes, three of which charge you for the privilege

The affix pool shipped with eight entries, and two facts about it made a
find repeat sooner than it should have. Only four could land on a Module, so
the slot you keep for yourself was the one whose drops varied least. And
DECOMP had exactly one affix in the whole set, at two points — so a capture
build's only roll was an uncommon one, with no small common version beneath
it the way ATK and DEF both had.

Ten more, filling both:

| Affix | Grants | Slots |
|---|---|---|
| Tempered | +1 ATK | any |
| Patched | +1 DECOMP | Module, Armor |
| Shimmed | +2 DEF | Module |
| Rigged | +2 ATK | Module |
| of Deep Cache | +1 DEF +1 DECOMP | Module, Armor |
| of Sidechannel | +1 ATK +1 DECOMP | Module |
| Volatile | +2 ATK **-1 DEF** | Armor |
| of Deadlock | +2 DEF **-1 ATK** | Weapon |
| of Cold Boot | +1 ATK +2 DEF | Armor, Module |
| of Hot Swap | +2 ATK +2 DEF **-1 DECOMP** | any |

The last three are new in kind: an affix may now cost you something. With
only three stats in the game and a calibration ceiling of +3 on any one of
them, a drawback that merely undercut an existing affix would be strictly
worse than it and nobody would ever want it, however common it was. So each
of the three puts a stat on a slot no clean affix will — ATK on armour, DEF
on a weapon — and bills that slot's own axis for it. Nothing in the set
grants four points across two stats without one; `of Hot Swap` does, and
pays for it in a capture skill your companions do not have, so a copy of it
handed to a program costs nothing at all.

A drawback scales with the run, because the affix is folded into the base
before gear level is applied — a cost that quietly stopped mattering after a
breach would read as a free upgrade rather than as a choice. Fusion and rare
tiers deliberately do *not* deepen it: those are what you spend to improve a
copy, and spending to make your own gear worse on one axis is a trade nobody
would take.

Two smaller consequences. An affix that grants nothing positive is now
refused at load with a warning, the way one granting no stats at all always
was — a mod's pure penalty is a roll no player has a reason to keep. And
every slot is now asserted to have something to roll, since a slot with an
empty pool is one where drops stay exactly as interchangeable as they were
before affixes existed.

Saves are unaffected: a copy that never rolled one of these still names the
affix it has, or none.

## 0.8.32

### Four perks that leave the player's own body

The perk picker was lopsided. Seven of its twelve entries were your own stats
and your own casts, and the other five reached mining, hunger, decompiling and
crafting — so a perk was something you bought to hit harder, and never
something you bought to change how the run worked. Four more, one each into
the four systems the catalogue had never touched:

```
  [13] Obfuscation    3 pts   -10% to every Trace rise, floored
  [14] Process Pool   3 pts   +1 tamed program you may own
  [15] Teardown       4 pts   +1 work resource from every kill
  [16] Failover       2 pts   +1 Durability per repair interval
```

**Obfuscation** makes the Stack quieter: everything that raises Trace — a
kill, a cache, a seal, a breakpoint — costs 10% less per level. It can never
buy silence. Low Power Mode is allowed to stop hunger draining entirely
because a Recharger Node already deletes hunger as a cost, but Trace is the
only pressure the Stack applies, so however many levels are stacked a source
still costs something.

**Process Pool** raises how many programs you may own, through the same
capacity a Data Cache feeds — which means it is the slots a GC Entropy Sweep
cannot take back.

**Teardown** strips a defeated process to the frame: a kill drops 2-4 work
resources, and each level adds one to that. It is the steepest thing in the
catalogue relative to what it modifies, which is what the 4-point price is
for, and it is bounded by the fights you take rather than by a machine that
runs while you sleep.

**Failover** repairs your base with no Patch Node standing. A base without one
took sweep damage that was simply permanent; now it mends a point per repair
interval per level.

Perks are still half data: these four are named, described and priced in
`assets/perks/`, and how much each gives per level stays in `tuning.rs`, where
a mod cannot reach it.

### The perk chart said +3 where the game said +2

Fixed: `docs/perks.md` charted Attacker and Defender at +3 Attack and Defense
per level against a game that has granted 2 since the curves were linearised.
The page is a hand transcription rather than a parser, which is what its own
staleness warning is about — the warning now names this case.

## 0.8.31

### Fifteen more contracts

The authored set was eight, thin enough that a board of three slots repeated
itself inside an hour. It is now twenty-three, spread across every objective
shape and banded by sector so what a Broker offers moves with the run:

```
  Offered
  [1] Coil Order - Deliver 8 Charge Coil - pays 55 Credits, 140 XP
  [2] Guardian Bounty - Terminate 1 Wintermute - pays 130 Credits, 2 ICE Breaker, 340 XP
  [3] Deep Sounding - Stand 5 frames down a Stack - pays 95 Credits, 20 Core Fragment, 260 XP
```

Four of the fifteen are repeatable, so every band keeps something to grind
rather than emptying as its one-offs are finished. Each names something the
sector can actually supply: deliveries are bulk stock only, the builds are
either ungated or behind research a run that deep bought long ago, and the
deepest sounding stops at frame 5, since only a Stack link at the far edge of
a sector's scatter runs any deeper.

### The Research Backlog asked for the bank

Fixed: *Research Backlog* asked you to deliver ten **Research Data**, which is
banked progress rather than cargo. No cargo screen lists it — the research
tree is the one place the number appears — so the errand read as impossible
while quietly being payable out of research you had not spent yet. It now
asks for ten Logic Wafers, which is the same readings transcribed.

A banked item is no longer something a contract file can name by accident;
`assets/contracts/README.md` states the rule for mods, and the shipped set is
checked by a test.

## 0.8.30

### Rolled contracts

The Broker's board was eight authored contracts and nothing else, so a long
run saw the same offers come round again. It now also carries **templates**:
contracts with holes in them, filled in against whatever the sector can
actually supply.

```
  Offered
  [1] Hunt: Sub-Process - Terminate 9 Sub-Process - pays 63 Credits, 180 XP
  [2] Requisition: Blank Substrate - Deliver 14 Blank Substrate - pays 28 Credits, 70 XP
  [3] Sounding: Frame 4 - Stand 4 frames down a Stack - pays 45 Credits, 180 XP
```

A rolled contract is not a second kind of contract. It becomes exactly the
thing an authored file parses into, so it is accepted, tracked, finished and
paid by the same code — an authored contract is simply a template with no free
variables. Five ship, one per objective shape, and `assets/contracts/README.md`
documents the format for mods.

**A rolled contract is always finishable.** It draws its species from the
programs living on your base's doorstep, its delivery from stock this sector
can actually produce, and its structure from what you have unlocked and do not
already own. When a sector can supply nothing valid, the template rolls nothing
and the slot goes to something else — an empty slot beats an errand you cannot
run. Offers stay put across a save and load, like the rest of the board.

### Contracts you had already finished

Fixed: the board could offer a contract the run had **already met**, which paid
out in full the moment you accepted it. A base with a Refinery standing was
offered *Stand Up a Refinery* — 45 Credits, five Power Cells and 140 XP for
pressing a key — and a run in sector 3 was offered *Reach sector 3*. Both
shipped in 0.8.29.

### Deliveries are bulk goods only

A delivery is asked for by the score, so it now names only cheap, accumulable
stock rather than anything in the catalogue. Without this a Requisition could
ask for twenty etched Routine Disks, which is a run's worth of research stated
as an errand. Portal Fragments were never askable and still are not: they are
the breaching currency, and their only source is a boss at the bottom of a
stack.

### Terminate, not Kill

`Objective::Kill` is now `Objective::Terminate`, in the asset schema and in the
line you read on the contracts screen — "Terminate 6 Drone" rather than
"Defeat 6 Drone". A mod naming `Kill` needs the one-word edit.

This does **not** bump the save format. The variant is serialised inside a
contract you are holding, so the only save it could break is one carrying an
accepted contract — and contracts shipped one release ago behind a research
buy and a build, so no such save exists. Bumping would have refused every real
save to protect a file that cannot be written.

### For testers

`cargo run -- --template contracts` opens standing at a Broker, in the `chains`
world with its research unlocked and its cargo deep, so the loop can be played
without first buying Contract Brokerage and building the Broker.

## 0.8.29

### Contracts

The game had no statement of what to do next. Research is a shopping list,
achievements are cross-run and one-shot, and the zone ladder says only "go
deeper" — so a player who had stood their base up and beaten a stack had no
answer to "and now?" beyond doing it again one level down.

Contracts are that answer. Build a **Contract Broker** (glyph `!`, unlocked by
the new Contract Brokerage research node), stand beside it, and the sector
tells you what it is paying for: thin out the drones, deliver twenty-five Core
Fragments, stand three frames down a stack, breach to sector 3. Take up to
three at once. Each pays in full when you finish it.

```
  Held
  [1] Clear the Nursery - Defeat 6 Drone [4/6] - pays 40 Credits, 120 XP
  [2] Fragment Quota - Deliver 25 Core Fragment [0/25] - pays 35 Credits, 90 XP
  Offered
  [3] First Descent - Stand 3 frames down a Stack - pays 50 Credits, 200 XP
```

**What a Broker offers is derived, not stored.** It comes off the world seed,
the sector and the clock, so the same three offers come back after a save and
load, cannot be rerolled by reloading, and rotate on their own as the run goes
on. Contracts already taken or already finished drop off the board.

Four of the five objectives finish themselves wherever you are, including four
frames down — a contract you are holding is readable anywhere, even where
there is no Broker in reach. Deliveries are the exception: those items are
handed over at the Broker, and it takes only as many as the contract still
needs.

**This deliberately amends "progression is earned by fighting."** XP is a
legal contract reward on any objective, including delivery and construction,
so what advances you can be the thing the game asked for rather than whatever
was nearest. What survives unchanged is the narrower rule underneath it:
Portal Fragments are still earned only by fighting and descending. They are
not a contract reward, and there is no variant for them to be one through.

Contracts are data — `assets/contracts/*.ron`, one file per contract, with
`assets/contracts/README.md` as the schema. Delete the directory and the
board is empty, which is exactly the game as it was.

The reward figures are opening guesses. `balance_sim` cannot see a contract at
all, so nothing gates them but play.

## 0.8.28

### A beaten stack collapses behind you

Killing a stack's guardian used to leave everything exactly as it was: the
link still open, the frames still walkable, the lair a cleared room at the
bottom of a maze with nothing left in it. The deepest thing you can do in the
game ended with a long walk back up through rooms you had already emptied.

It now ends the stack. The floor gives way, the party is flung up through
collapsing frames onto open grid, and the link you walked in through caves in
behind you. The run's record of the place goes with it — there is no map to
come back to, because there is nowhere to come back to.

```
  The stack folds in on itself. You are flung up through collapsing
  frames and land hard on open grid.
  The ground answers somewhere else: a new link opens south-east at 7 tiles.
```

**The sector never loses a link.** The replacement opens on the nearest legal
ground before the old one comes down, so a zone always has a way back
underground — which matters more than it sounds, because a Stack boss is the
only thing in the game that pays a Portal Fragment, and a zone with no link
left is a run that can never breach again. The new stack's depth follows the
same rule every link does, read off how far out it lands, so finishing a deep
one nearby tends to hand you back a shallower one.

The trade is that a collapse forfeits whatever you left behind: an unopened
cache two frames up, a seal you never shouldered, an orphan in a dead end.
The lair sits on the bottom frame, so you have walked the whole stack by the
time you reach it — killing the guardian is now the deliberate end of that
stack rather than a step you can walk back from.

## 0.8.27

### One key fuses every matching pair in cargo

A long run leaves the buffer full of duplicate gear, and turning it into
fused copies meant opening each stack's action page and pressing `[U]` once
per pair — a dozen keypresses and a dozen turns for work with no decision in
it. `[U]` on the inventory screen now does the lot at once:

```
  You fuse 5 pairs:
    2 Ablative Plating -> tier 1
    2 Kinetic Edge -> tier 1
    1 Overclocked Arc Lance -> tier 2
```

It is **one pass, not a cascade**: four ordinary copies come out as two T1s
rather than one T2, and an odd copy is left ordinary. What you get is one
rung up the ladder for everything you had a pair of, which keeps the deeper
tiers a choice about where to spend the spares rather than something a
convenience key decides for you. A copy you are wearing still counts as one
of its pair, exactly as it does on the per-item page, so pressing this once
and pressing `[U]` down the list are the same fusions.

The whole batch costs **one turn**, not one per pair. Charging need decay,
sweep pressure and spawn rolls per fusion would make the shortcut cost more
than the typing it saves. A press with nothing to fuse is refused and spends
no turn at all.

## 0.8.26

### The run-a-routine picker says who needs it

Every field routine that lands on one ally is about a stat — Repair Loop
about Integrity, Overclock about Attack, Hardened Shell and Ablative Layer
about Defense. The screen that asked you to choose a body listed names and
levels and nothing else, so the one decision it exists for had to be made
from memory. Each row now carries the numbers the buff is about to touch:

```
  Run Overclock Single on whom?

  @ [a] You Lv12 - HP 88/120  ATK 24  DEF 18  PWR 130
  p [b] Kestrel Lv5 - HP 40/40  ATK 12  DEF 9  PWR 61
  p [c] Sable Lv7 - HP 12/58  ATK 15  DEF 11  PWR 84
          replaces Repair Loop Single HP+7/4t — 62t left
```

That last line is the other half. Running a routine on someone who already
has one of the same kind *replaces* it rather than stacking with it, and
nothing on the screen said so — so the tag names what is about to go, and
names the routine rather than saying "already running", because two
different routines can arm the same kind. It is deliberately narrower than
"what is running": a buff armed by a consumable survives the cast, so
listing it would be a lie about what you are about to lose.

## 0.8.25

### The extract picker names what each program is carrying

Breaking a program down destroys it for exactly one of its routines, and
until now the screen that asked you to choose said nothing about what was
inside any of them — the only way to find out was to open each program in
turn and back out again. Each row now carries its kit on the line beneath
it:

```
  s [a] Sprite Lv7
        Patch Routine (known), Static Burst
  g [b] Glitch Lv4 ++
        Priority Boost (known), Overclock (known)
```

The `(known)` tag is the part that decides it. Extraction *refuses* a
routine you already know, so a program whose whole kit is tagged is worth
nothing on the block — and that was invisible until you had walked two
screens in to find it.

The list is the same call the next page makes, so the two cannot describe a
program two ways, and the routines shed onto their own lines rather than
running off the popup's right edge. That wrapping is now one piece of code
shared with the fuse picker, which grew the same lines a release ago.

## 0.8.24

### The battle roster says what each member is wearing

The party's side of the intrusion screen gains a `GEAR` column, between
`POS` and `FATIGUE`, carrying the same `w|a|m` loadout cell the roster and
the status panel already show — a letter per filled equipment slot, a dot
for an empty one. A companion you kitted out and one you never got round to
are now told apart in the place it costs you to find out the hard way.

It is the engine's cell rather than one the renderer assembles, for the
reason the other two screens' is: a loadout that reads one way in a fight
and another on the sheet you set it from is worse than not showing it. And
it is fixed-width by construction, so `FATIGUE` and `ACTION` sit at the same
column whether a row is fully geared or completely bare.

## 0.8.23

### The etch screen is somewhere you can get to, and says what you already hold

Etching is where a blank Routine Disk becomes a routine you can install, and
it was buried: the only way in was *Install a routine* → pick a holder →
pick an **empty** slot → `e`. Every routine slot in the game starts full, so
a player who had never popped one out could not reach it at all. The party
menu now opens it directly.

It also answers the question it exists to ask. A blank is spent for good, so
each routine says how many finished disks of it are already in cargo — you
find out you have three before you burn a fourth, not after:

```
Burn which routine onto a blank? The blank is gone either way.
Blanks: 4

[1] Bastion Single v1.0            ×2 held
    +3 DEF to one ally for 3 rounds
[2] Overclock Single
    +4 ATK to one ally for 90 turns
```

**Fixed:** the last routine on that screen showed no description. The
popup's scrolling list ends at its last pickable row and pins whatever
follows to the bottom of the box, so the final description was drawn adrift
under the scroll indicator instead of under the routine it belonged to. The
slot panel, the install picker and the extraction picker all had it too, and
all four are fixed together.

## 0.8.22

### The fuse picker names what each program is carrying

Fusing two programs builds the result's routines fresh from its species, so
anything installed on either parent — researched, extracted, swapped in off
a disk — is gone. The game said so, but only on the last page of the flow,
after both picks were already made.

Both pickers now say it while the picks are still free. Each candidate
carries its routines on the line beneath its stats:

```
[a] Kestrel Lv6 - HP 22/28  ATK 8  DEF 5  PWR 19 (in party)
       Hyperthread Single v1.0, Sandbox
```

A full six-slot kit is wider than the popup, so a long list wraps onto
further lines rather than running off the right edge.

## 0.8.21

### A fight tells you what it paid at the end, not between the blows

Killing something used to interrupt the fight to tell you about it. Every
kill put a loot line and an XP line into the battle log, so a pack of five
scrolled the blow-by-blow past you behind ten lines of bookkeeping — and the
one thing you actually wanted, what the whole fight came to, was scattered
across all of them.

A fight now closes with the answer. One salvage tally, with copies of the
same thing counted together, and one experience line per fighter carrying
the fight's total and the stats it bought:

```
The rogue program crashes and deletes itself!
Salvage:
  1 Bastion Lattice of Static [ARM]
  1 Black ICE Pick [WEP]
You gain 1717 XP, reaching level 23.
  Max HP 318 → 354
  ATK 46 → 49
  DEF 42 → 45
  Perk Points 0 → 3
  Decompiler 9 → 12
```

Two things stay where they were. Nothing is *paid* any later than before —
a level still lands on the kill that earns it, and still heals you there, so
a fight plays out exactly as it did. And because a level heals you, reaching
one is still announced the moment it happens, briefly: your HP bar snapping
back to full mid-fight is something you need explained then, not afterwards.

Running from a fight pays the tally too. You keep what you killed before
you left.

An Overclocked copy is counted apart from an ordinary one rather than summed
into it, so the tally never tells you that you found two of a thing when one
of them was the good one.

## 0.8.20

### Staff a machine from the screen that told you it was idle

The structure roster now does something. Highlight a machine, press Enter,
and pick who works it — the roster is the only screen that shows the whole
base at once, and it has always drawn an unstaffed machine in yellow, so it
is where you find out that something is sitting idle. Until now the only
thing you could do about that was close it, open the base menu, pick a
program, and pick the machine again from a different list.

Picking drops you straight back on the roster, on the same row, with the
program you just posted listed under it. That is the point of doing it here:
the answer appears where you were already looking, and staffing three idle
machines is three presses of Enter rather than three round trips through a
menu.

The list leads with **Yourself**, which puts you on the machine by hand —
offered only when you are standing on one of the four tiles beside it, since
that is the only place working it yourself has ever been allowed. Everything
else on the list is a program you own, shown with the same level, power and
current errand the cronjob picker shows.

The old flows are untouched. "Assign a cronjob" still answers *where do I put
this program*, which is the right question when you have a program in mind;
the roster answers *who goes on this*, which is the right question when you
have just spotted an idle node. Posting a guard stays on its own screen, and
Enter on something that takes no worker — your Home, a Shield — says so
rather than doing nothing.

Underground the roster still reads exactly as before, but Enter does nothing:
while you are in the Stack the game holds your position at the entrance tile
you climbed in through, so a posting made from down there would measure its
walk from the wrong end of the map. The hint at the foot of the screen stops
offering the key rather than refusing it after the fact.

## 0.8.19

### A base you build outward

A base starts half the size it used to — a 9x9 slab of about 69 buildable
tiles rather than 15x15 — and the **Heap Pillar** grows it back, one ring at
a time. Build one and the edge creeps out a tile in every direction; build
another and it creeps again, up to five of them — a base ends at 19x19,
better than twice the ground it opens with. Five is a number in the
structure's own file rather than in the engine, so it is one edit to change
and any structure can declare a limit the same way. It costs 14 Core
Fragments, sits behind a new research node (Heap Allocation, 30 Research Data
off Power Grid), and cannot be demolished.

Like anything else, a Pillar is deployed onto one of the four tiles beside
you, so growing the base is a walk-and-build loop rather than a menu you
hold down.

The complaint this answers is feel rather than capacity. A real base does not
fill 213 tiles, so the old slab was the same size in the first minute of a
run as in the tenth hour and nothing you did ever made it bigger. Growing in
single-tile steps is the whole of the settlement reading: the edge moves
often, by a little, and you are the reason.

Irreversibility is the design rather than a limitation. A Pillar that could
come down would leave structures standing outside a shrinking slab, and there
is no good answer to that — so there is no shrinking slab. Demolishing your
Home still cascades over everything, Pillars included, and the base resets to
its starting size.

Growth claims the ground it takes, exactly as deploying a Home always has:
wild programs and nests in the new ring go, and so does a Stack link, with a
line in the log naming the tile. The one thing it will not bury is the
sector's **last** link — the Stack is the only place Portal Fragments come
from, so a base that swallowed the last way down would end the run's
breaches — and that refusal comes before it charges you.

Two knock-on fixes come with it, and one of them would have ended runs. A
zone's Stack links were drawn from boxes around your arrival point, and a
slab wide enough swallows those boxes whole — at which point the placement
loop spends its whole shared attempt budget failing to land the first link
and the sector gets **none at all**. No links means no Stack, no Stack means
no Portal Fragments, and the run can never breach again; it would have read
as a bad seed rather than as a bug. Every link is now drawn from a band
starting just outside the base, whatever size the base is — the on-ramp from
a narrow band and the other two from a wide one — which also repairs a
squeeze that existed at the old radius.

One thing a very large base does change: a program can only be posted to a
machine within sixty tiles of where you are standing. The walk a posted
program takes is a search whose cost grows with the square of its reach, and
past that point a single walking worker costs more per turn than the turn is
worth. The cronjob menu refuses the post rather than accepting one that
never arrives, and tells you to get closer. And stack depth is measured from the edge of
your base rather than from its centre, so growing does not quietly make every
descent in the sector deeper.

The opening nursery is unchanged at 7 tiles. It used to be defined *as* the
build radius, which would have shrunk it for the opening minutes and then
widened it every time you built a Pillar — a difficulty knob keyed to base
geometry, which is the thing removed when distance stopped scaling anything.
Home's rest radius and the Recharger Node's reach now cover a fully grown
base instead of a fixed 7 tiles.

Existing saves load untouched, and keep the base they already had. Nothing
about the footprint is stored — a base's width is rediscovered from the
structures a save already carries, under a rule that holds everywhere: the
slab always covers every structure standing on it. Without that, a base
built before this release would have kept its old floor while only the inner
9x9 accepted a building, which on a real save meant 156 tiles that looked
exactly like base and refused to be built on, with you standing on one of
them. The one wrinkle, and it costs real Core Fragments so it is worth stating
plainly: on a base built before this release, a Pillar buys **nothing** until
the bonuses have caught up with the width that base already had. A base
sitting at the old radius of 7 absorbs three of them before the fourth moves
anything. Only a pre-release save can be in that position, and a base built
from here grows on the first one.

## 0.8.18

### Rotten substrate can kill you now

A step onto corrupted ground in the Stack costs 10% of your maximum
Integrity, up from 3%. A patch is three cells, so walking one end to end is
about a third of the bar, and turning round and walking back out is most of
the rest.

At 3% a patch was a toll. It could not kill, so the only question it ever
asked was whether the way round was longer than a tenth of your health, and
the answer was almost always no — you paid it and forgot it. At 10% a party
that is already hurt can die on the third cell, which makes the detour a
decision you can get wrong. That is the whole reason the Stack has a second
kind of walkable floor.

Nothing else moved. Corruption still comes as two patches of three per
frame, sparse enough that most routes miss them; it still measures against
your maximum rather than your remaining Integrity, because Stack depth is
uncorrelated with your level and any flat figure is lethal at one end of a
run and free at the other; and it still goes through the one damage path in
the game, so a Mitigation field buff blunts it exactly as it always has.

## 0.8.17

### The roster's row shortcuts stopped standing programs down

`P` now adds the highlighted program to your party, or stands a member back
down. A row's number or letter only moves the highlight, and Enter — a key
this screen has never had a use for — does nothing at all.

Every other action on the roster already worked this way: `<` and `>` shift a
member along the battle line, `N` renames, `E` fits gear, all of them reading
the highlight. Party membership was the exception and the only destructive
one, so the screen answered a digit typed at the wrong moment by pulling a
program out of the party, with nothing to undo it but noticing. The help
lines name the new key, and `the_companion_screen_names_the_party_key` is
what keeps them naming it — with the row shortcuts inert, that line is the
only thing pointing at the one action this screen exists for.

## 0.8.16

### Counts read down a column instead of trailing the name

Every screen that prints a quantity now leads with it — the inventory list,
the base pane's cargo column, a trader's sell and buyback rows, and the Stack
market's — instead of appending `x3` after the item name. The column pads to
three digits, so the names form a straight left edge and a stack of 4 is
distinguishable from a stack of 40 at a glance rather than by reading to the
end of each row.

`qty_column` is the one definition of that column, and it lives in app-core
beside `equip_preview_tag`, which those same five screens already share. A
count that read one way on the screen you checked and another on the screen
you sell from is what costs a player a copy they meant to keep. It grows past
three digits rather than truncating: a wrong quantity is worse than a wide
row.

### A program's gear is readable from the list it's in

The roster and the status panel now carry a `w|a|m` cell per program — one
letter per filled equipment slot, a dot holding the place of an empty one.
Both screens get it from one function, so a loadout cannot read one way in
the panel and another in the roster you opened from it.

The cell is fixed width for the reason it exists at all. These lists include
programs you never open a gear screen for — a posted worker, a bench-warmer —
so "which of these is still bare" has to be answerable by scanning down a
column, and a cell that shrank when a slot was empty would leave nothing to
scan. It sits directly after the stats and ahead of every optional tag
(quality, fusion depth, the wield mark, the activity), because those come and
go per row and a column placed after one of them lines up only with the rows
that happen to carry it.

### Fixed

Nothing. Two overflow measurements came out of this work and neither is
fixed: the widest shipped inventory row and the widest roster row both run
off the right edge of their popup, by 68px and 393px. Both predate these
changes and both are recorded in `TODO.md` — the fix in each case is a
decision about which tag loses, not a shorter one.

## 0.8.15

### The research tree no longer finishes early

Twenty-one nodes, 561 Research Data, and nothing about any of it gated on
progress: every node was buyable from turn one if you were willing to wait,
and Research Data survives a breach. So the tree was paced by patience alone,
and it read as a checklist to clear rather than a set of decisions. You had
everything researched long before the run was over.

Raising the prices alone would have been the same checklist with a bigger
number on it. **Twelve nodes now declare the zone they open in** — six at
zone 2, six at zone 3 — and the ladder reprices from 561 to 1258.

Below its zone a node is **still listed**, still priced, still described,
and unbuyable at any balance; the row says what it is waiting on. That
visibility is the point rather than an oversight. A zone-3 tier sitting in
the menu while you are in zone 1 is the reason to go breach, and hiding it
would mean a player who never breached never learns the tier is there. It is
the same argument the upgrade menu already makes for a structure stalled at
its zone ceiling. A node held up by both a prerequisite and a breach says
both: *(needs Neural Interfacing, Zone 3)*.

The opening nine nodes are **nearly untouched** — 129 Research Data becomes
158. The complaint was that the tree finishes early, not that the first bench
arrives early, and a base that cannot stand up its first machine is a worse
opening rather than a slower one. The two dearest bands are where the change
lands: 163 becomes 350, and 269 becomes 750.

The tap is deliberately unchanged. A Research Node still pays a flat 1 per
cycle into a bank with no ceiling, which means the ladder is the only thing
bounding it — giving the tap a zone term would have paid out more at exactly
the moment the gate released more to buy, cancelling the pacing this exists
to create. What compounds instead is something already in the game: a
Research Node is capped at Mk1 in zone 1, Mk2 in zone 2, Mk3 in zone 3, and
its cycle succeeds 50% of the time at Mk1 against 90% at Mk5. The band you
can buy earliest is the band you earn slowest, and every breach speeds the
bank up at the same moment it releases more to spend it on.

The bands are content: `min_zone` in `assets/research/*.ron`, one optional
field that defaults to ungated. A mod's research file that never heard of it
keeps parsing and ships a fully open tree exactly as before, and retuning
which node sits in which band is a `.ron` edit with no code change. Two rules
are enforced against the loaded tree rather than left to care — a node may
not be gated below its own prerequisite, and nothing unlocking the Zone
Portal may be gated at all, since that is the structure you reach the next
zone *with*.

**Existing saves load untouched.** The gate is on buying, not on having, so
a save that already paid for a node keeps it whatever zone it is standing in.

## 0.8.14

### A breach now lands you somewhere

Breaching used to change numbers and nothing else. A zone-7 sector was
generated by exactly the same rules as zone 1 — the same six biomes at the
same five thresholds — so the deal on offer was: lose your stockpile, keep
your base, fight harder things, get Mk+1.

Every zone past the first is now a **sector** with a character of its own.
Cold Storage is frost-locked, Static Field spreading over ground that is
almost empty of it anywhere else. Fractured Allocation is more gap than
ground, awkward to cross and awkward to build in. Null Expanse is unmapped
pages to the horizon. A breach announces where you have landed, and the map
shifts its colours to match.

The mechanical half is one knob: a sector moves where the world generator's
biome boundaries fall, and everything else follows from that without a second
setting to disagree with it. What lives there follows, because the wild
roster is filtered by the biome of the tile a program spawns on. Where you
can build follows, because a hole in the map is a hole in the map. There is
deliberately no separate species-pool bias and no per-sector difficulty.

**Zone 1 is always neutral**, whatever is installed. The opening zone fields
only programs a fresh player can actually beat, and biasing its biome mix
would have moved that roster while looking like a cosmetic change.

Sectors are content, not code: `assets/sectors/*.ron`, one file per sector,
documented in that directory's `README.md`. A mod adds one by dropping in a
file, and deleting the directory restores the previous game exactly — the
same supported way to play that removing the affix pool or the trained enemy
policy already is. Two load-time checks refuse a file rather than shipping a
broken world: a sector that would leave too little standable ground to
materialize on, and a palette that would tell you a hole in the map is safe
to walk into.

Which sector a zone gets is derived from the world seed and the zone number,
both of which your save already holds — so this needed no save-format change,
and **existing saves load untouched**. One consequence worth knowing: in a
run that had already breached, ground you have not yet explored may generate
under the new thresholds, so a map can change shape at a boundary you have
not walked to. Everywhere you have been, and everything you have built, is
unaffected.

## 0.8.13

### Every screen now prices the affix it is naming

Gear picked up on the Grid can roll an affix — an Overdriven weapon, a
Reinforced plate, something of Static — worth a small flat bonus on top of
what the item already grants. That bonus is added before gear scaling rather
than after, so an affix grows with the run instead of dwindling into a
rounding error, which is the whole reason to be pleased about finding one.

The numbers on the screens were not being told any of this. The swap picker
would name a candidate "Overdriven Kinetic Edge" in one column and, in the
next column along, price it as a plain Kinetic Edge. Because the affix is
folded in before the level multiplier, the gap was not a fixed point or two —
it grew with how deep you were. At zone 3 that row promised +6 ATK where
putting it on actually granted 15. At zone 5 it read +10 against 25.

The same blind spot ran through the inventory tag, the delta column, the
`(Unequip)` row — which understated what taking a good weapon off would cost
you — and the equipped panel, which dropped the rare tier as well and could
report an Overclocked Overdriven Kinetic Edge worth 27 ATK as being worth 6.

Nothing about your character changed: the stats you were actually fighting
with were always correct, and no save is affected. What changed is that the
screens now agree with them, so a comparison between two pieces of gear is a
comparison you can trust — and an affixed find no longer looks like a
downgrade to the plain copy already on your back.

## 0.8.12

### A companion's gear, and the machine it is standing at, on its own sheet

The manifest is the page you open to find out what something *is*, and for a
program you own it had two holes in it.

Gear was the first. Any program you own has been able to wear a weapon,
armour and a module since 0.8.0, but the manifest's EQUIPMENT box was the
player's alone — the only way to see what a companion had on was the Program
Gear screen you equip from, which tells you what is in the slots and nothing
about what those slots are currently worth. A companion's page now carries
the same box the player's does, measured the same way: each item at the zone
level and fusion tier recorded when it went on, not at a fresh preview of
today's. A program wearing nothing shows no box, the same way an empty slot
has never been listed as "(none)".

The post was the second. A program on a cronjob did say so, as one entry in
the run of tags under its name — but a worker's tag is the bare structure
name, which beside "Lv 14" and "Excellent (91%)" reads as decoration rather
than as an assignment. The WORK box, which is already the box about what a
program is like to *post* somewhere, now states it outright: **Posted to
Mining Node** for a worker, **Guarding Shield Wall** for a guard, and no row
at all for one that is idle or in your party. The verb is the whole
difference between the two jobs, and it is the reason the row is not one
label for both.

Nothing about how gear or posts work has changed — this is what the screen
reports, not what the game does. Existing saves load untouched.

## 0.8.11

### Decompiling reads the gap between you and what you are pointing at

Wearing a program down before spending a catalyst on it has always been most
of the odds, and against anything near your own strength it still is. Against
trash it made no sense. A program far enough beneath you dies to a single
strike, so it can never be shown to you at low Integrity — and the attempt was
priced as though you had looked at the option and declined it. Decompiling
something harmless was therefore *worse* than decompiling something dangerous,
which is backwards, and it made the easiest programs on the Grid the most
annoying ones to collect.

The gap now counts. Once a target is far enough below you that it reads Green,
its remaining Integrity stops entering the odds at all: there is no softening
to reward, so you are no longer charged for skipping it. With the shipped ICE
Breaker that takes a healthy Green-con drone from roughly one attempt in eight
to closer to one in two. The relief fades in across the Yellow band rather
than switching on, so an even fight is very nearly the fight it was.

The same reading runs the other way. A program more than half again your
strength — the ones already drawn in Red — gets harder to decompile the
further above you it is, down to a floor of 60% of the odds at two and a half
times your power. Nothing becomes impossible; a long shot stays a long shot.

The number driving both is the one already on your screen. It is the same
comparison that paints a wild program Green, Yellow, Orange or Red, so the
color you are looking at and the odds you are quoted can no longer disagree
about which of the two of you is stronger.

One consequence worth knowing if you have bought into it: **Exploit Focus does
nothing against a target you badly outclass.** The perk cuts the penalty for
attempting a healthy program, and against a Green-con one that penalty is now
already gone. It is worth exactly what it always was everywhere else, which is
where it was always meant to matter.

## 0.8.10

### Examine looks down a line

Pressing `x` and picking a direction used to search a 90° wedge out to forty
tiles. Anything leaning your way counted — a thing forty tiles east *and*
forty north was "east" — and forty tiles is more than twice what the map pane
shows in either direction, so the inspector regularly opened a sheet for
something you could not see and had no way to find. It now looks down the row
or column you are facing, one tile wide, twelve tiles out. A creature one tile
off your row is missed, which is the price: you can step or turn, and what `x`
names is now what was in front of you.

Two things came out of the same rewrite. Two candidates the same distance away
could resolve differently between runs, because the scan asked for "whichever
came back first" from a source that does not promise an order — so the same
press could give two answers on two loads of the same save. And aiming at a
machine with a program posted to it would open the *program's* sheet, which
was odd on its own and stranger still because a program at its post is not
drawn on the map: the answer was something invisible standing one tile in
front of the thing you were pointing at. The inspector now names only what the
map draws, and a tile holding both a machine and a program names the machine.

That leaves a posted program with no sheet of its own, so its machine's sheet
now carries its level and health beside its cronjob progress. The `B` roster
shows the same, since both screens are built from one place.

Nests, Stack entrances and zone portals are still passed straight through —
they draw a glyph but are not yet things the inspector can name, so aiming at
one reports whatever lies beyond it.

## 0.8.9

### A dropped weapon can come up rare, and can carry a name

Gear that drops now rolls two things it never used to. The first is a rare
tier, off the same five-rung ladder a wild program rolls on — so an
Overclocked Arc Lance is exactly as rare as an Overclocked program, and the
word means one thing wherever you read it. The ladder itself grew two rungs
at the top, Unrolled and Bare-Metal, for gear and programs alike.

The second is an affix: a name fragment and an extra stat, giving you
"Overclocked Arc Lance of Static" — the colour you read off the row and the
words you read off the name, rolled independently. A tier is the chase, at
about one drop in thirty across the whole ladder. An affix is the variety,
at about one in five, which is what stops the other drops being the fourth
copy of something you already have. An affix's stats are added before the
gear scales, so a good one grows with the run rather than dwindling.

Only *found* gear rolls either. Crafting, buying and buying one back are
deliberately excluded: what you go looking for should beat what you can
shop for. A surface boss now pays at Optimized or better, since that fight's
whole job is to hand you something better than the ground you crossed to
reach it — where a Stack lair pays progression and a nest pays roster.

Affixes are content, not code. `assets/affixes/*.ron` ships eight, a mod
adds one by dropping in a file, and deleting the directory restores the
previous game exactly, down to the random number stream.

Existing saves load untouched. A copy you were already carrying reads as
ordinary and unaffixed, and gear you sold before this release still buys
back as the copy you sold.

## 0.8.8

### A key pressed while the map log scrolls in is no longer eaten

Battle results scroll in a line at a time after a fight, and a keypress
during that reveal skips to the end rather than acting — which is right, on
the battle screen. The reveal range was never closed, though, because the
results are still arriving when the fight ends. So `battle_log` went on
growing with ordinary map and base news for the rest of the run, and the map
believed it was still revealing something.

The result was a key silently spent for every line a running base logged.
Stand next to a working machine, press a direction, and the step would
sometimes just not happen — with nothing on screen to say why.

The reveal now applies only on the battle screen, gated in the one place
both the "is it revealing" check and the line count read. The map's log pane
stops holding lines back too, which was the same leak seen from the other
side.

## 0.8.7

### Somebody is selling things four frames down

A Stack frame can now stand a market on one of its junctions — not every
frame, and never twice — where somebody nobody asked about is trading out
of a folded tarp. Press `t` on the cell.

They are ephemeral in the way the word means. There is **no buyback**: a
surface Market keeps what you sell it and offers it back at double, and
this one keeps no record at all, so what leaves your pack is gone. And the
shelf does not refill — a row that has been bought stays bought for the
run, and a stall with nothing left on it packs up and reads as plain
corridor in both Stack views, exactly as an emptied cache does.

What is on the shelf is two routines, each at three sizes of bundle:
enough disks for **one program** at 150 Credits, for **your party** at 300,
or for **everything you own** at 1000. What is being sold is the *writing*,
not the routine — nothing here teaches you anything, so research and
extraction remain the only two ways to actually know one. The disks go
straight into cargo, so a party with every slot full can still buy, and who
ends up running them is a question you answer later at the routine panel.
Some markets also carry a program, priced off what it would spawn at as
this depth, compiled to your control on the spot.

Two things a trader down here will not sell. The 28 hunt-only routines
stay hunt-only — a shop selling those is the "buy it instead of hunting
for it" shortcut they exist to prevent — and no boss, for the reason an
orphaned process is never one either.

What is on a given shelf is a function of the stack, the entrance and the
depth, so it survives a save and a reload: you can look at a price, climb
out to go and earn it, and come back to the same stall.

### Some routines are not for sale, or for research, or for anyone

A routine now reaches a slot in two steps rather than one. Etching burns a
blank Routine Disk with something you know and hands you an **etched disk**;
installing spends that disk on a slot. Popping the routine back out still
returns nothing — the disk went at install. The routine panel's install page
lists the disks you are carrying, and `e` there opens the etch page.

That split is worth a paragraph only because of what it makes possible.
**Six routines exist that nobody can learn**, and therefore nobody can etch.
No research node teaches them, no species is born running one, and no shop
lists one at a bundle price. The only way to hold one is to hold the disk
somebody else already wrote — and there are exactly two of those.

Wintermute drops **Kernel Shear**, which tears a whole group open and leaves
every one of them bleeding; **Null Cache**, which drains a group and returns
every point of it to you rather than a third; and **Deadman**. The Overseer
drops **Hard Fault**, two rounds of nothing across every hostile on the
field; **Long Winter**, a quarter off all incoming damage for three hundred
turns; and **Watchdog**. Roughly one disk a kill, and a Stack trader deep
enough down will occasionally have one on the shelf for 1400 Credits — far
more than six copies of anything you could have made yourself.

Deadman and Watchdog are a new kind of thing. They sit in a slot, appear in
no menu, and never take a turn: Deadman goes off when one of yours goes
down, and Watchdog clears the condition off everybody the moment one sticks
to its holder. A deadman's switch fires precisely because nobody is holding
it any more.

Breaking a program down at a Compiler pops an exclusive routine's disk back
out intact instead of teaching it, which is the only way to move one — and
it still costs the whole program. There is never more than one copy.

Two smaller things fell out of it. `t` underground used to open the
*surface* trader picker, which scans from a position pinned to the entrance
tile — so it would cheerfully offer to trade with a base four frames
overhead; it now answers about the cell you are standing on. And the frame
map's glyph table is exhaustive, so the next cell kind added cannot ship
drawing as bare floor.

## 0.8.6

### The log pane's header lists every filter

The header read `LOG [Field]  F to filter`, which names the active setting
and nothing else — not what the other two are, not which way the key steps.
Seeing a base line under a header you believed said Field then leaves no way
to tell whether the filter is broken or the line is tagged wrong; the honest
reading, that you had cycled one press short, is the one the header gave you
no way to check. It also advertised `F`, which is bound to nothing: the key
is `f`, and reaching for shift did nothing at all.

It now reads `LOG  All · Field · Base   f to cycle   12 base hidden`, with
the active filter in bold green against the other two in dim grey. Two
colours on one line means the header is drawn as `Painter::ui_runs`
segments rather than a single string, so `log_pane_header` returns styled
pieces.

The options are listed in `LogFilter::ALL`, and a test pins that order to
the one `LogFilter::next` walks — a row of options the key steps through in
some other order would be worse than naming none of them.

Unchanged: the history screen (`L`) is still deliberately unfiltered, and
the counted "N base hidden" tail still only appears when the suppressed
channel has traffic in it.

## 0.8.5

### A compile row says what kind of thing it makes

The Compile screen listed each recipe by name, by what it grants and by what
it costs, and never said what the thing actually *is*. A Handshake Forge and
a Bytecode Block read the same way, so the only way to tell the piece of gear
from the machine feedstock was to already know — or to compile one and go
looking for it in cargo.

Every row now leads with the same kind tag the inventory and a trader's
shelf have always shown, and the list is grouped by kind, so the tag reads as
a heading for the run of rows beneath it: everything you spend, then
everything you wear, then everything you hoard. A modded item gets its tag
for free, since the tag comes off what its file already declares.

### Long rows stop running off the edge of a screen

The four recipes priced in Portal Fragments — Singularity Matrix, Nullsteel
Plate, Oracle Core and Phase Carapace — were too wide to fit the Compile
popup, and had been since they were added. The right-hand end of the
ingredient list was simply drawn past the border and off the panel, so the
recipes that cost the most were the ones you could not read the price of.

A row that no longer fits now wraps: the item keeps the line with its
shortcut on it, and its ingredients continue on the line beneath. The
Recipes screen wraps the same way if anything there ever grows that wide.

## 0.8.4

### The Recipes screen says why you would make the thing

The chains screen has always answered "how do I make this": what to build,
in what order, back to the raw salvage it bottoms out in. It never said a
word about what the thing at the end of the chain is *for*, so deciding
whether a two-machine line was worth standing up meant leaving the screen,
finding a copy of the product in cargo, and reading its description there —
or already knowing.

Each chain now opens on that description, above its steps. It is the same
prose the inventory's describe page shows, drawn from the item's own file,
so the two cannot end up describing one item differently and a modded item
gets its line for free.

## 0.8.3

### A door beside you looks like a door

Walking a Stack corridor, a door dead ahead draws brown with a `+` over it.
A door one step to your left drew as the same cyan wall as the rock it is
set into — so the only way to find a side passage was to walk into it.

The side wall now takes its colour from whatever is actually standing
there, the way the wall ahead of you already did. Sealed doors keep their
own red, so a way on you cannot open yet still reads as different from one
you can.

This is the half of the fault that survived the last attempt at it. That
one made a door legible *down* a corridor, by marking it with a glyph after
the distance fog was found to be eating the colour; this one is about the
door you are standing next to, where the neighbouring cell is off the edge
of the view entirely and its colour is the only thing left to see it by.

## 0.8.2

### The guardian at the bottom of a stack stops outclassing you

A lair guardian is the fight a run cannot walk away from — it is the only
thing in the game that pays a Portal Fragment, so a zone you cannot breach
is a run that has quietly ended. Measured on a party arriving in the shape
the game expects, a depth-2 lair in zone 3 was clearing about once in four.
It now clears a little over half the time. The change is one number, the
Overseer's attack; nothing else about the fight moved.

### The roster tuner can no longer propose a roster the game would reject

Developer tooling, and the rest of this release is all of that kind.

A species' stats are not authored freely — its growth band sets a budget,
its class sets what share of that budget it carries and how the three stats
divide it, and its speed sits in a band the class decides. That is what
makes "tanky for its tier" a readable thing rather than a number you have to
already know the ladder to interpret. The tuner moves exactly those fields
and had never been told any of it, so its first real search proposed
fourteen changes of which thirteen would have broken the shipped suite. Two
of the fourteen were never even reported, because the check that catches
them stopped at the first thing it found.

It now knows. A candidate that puts a species off its budget, off its
class's shares, outside its speed band, or on a growth multiplier between
the ladder's rungs is thrown out before any fight is run — and the rules are
the game's own, called rather than restated, so the two cannot drift apart.
Bosses stay exempt, which is deliberate: they sit outside the class system,
and a boss's attack was the one move the tool has ever found worth having.

Two further checks are too expensive to pay on every candidate — one runs a
level search, the other reads the whole roster's distribution — so they run
once on the winner and are **reported** rather than enforced. By then a
person is reading a diff and deciding, and a report that hides what it
checked is worse than one that admits what it could not.

The rejection count now says *which rule*, which turned out to matter
immediately: the first run under the new rules threw out forty candidates
against twenty-one fought and proposed nothing at all. That is the search's
move generator being wrong rather than the roster being right, and
`dev-tuning/NOTES.md` records it as the next piece of work instead of
letting a converged-looking run stand as evidence.

None of this touches a save, a species file, or anything a player sees.

## 0.8.1

### Difficulty rises in steps now, instead of doubling

A zone used to double every wild program's stats, and every frame you
descended into a stack multiplied them again. Your own side of the fight has
never worked that way — a level is worth one point of ATK, an item a few flat
points — so the two curves were never in a race you could win. Past a certain
depth the arithmetic stopped mattering: every blow you landed came out as 1,
and no amount of levelling, gear or roster changed it, because there was
nothing left for those numbers to do.

Both curves are now linear. A zone adds to enemy stats rather than doubling
them, a frame of Stack adds a little more, and your gear tracks the same
shape so it neither falls behind nor runs away. Deep is still hard — it is
just hard in a way you can answer.

What that means concretely, measured on a real stuck save at zone 1: the
guardian at the bottom of a five-frame stack went from a fight won 3 times in
40 to one won every time, and a six-frame stack from unwinnable at any level
to a twenty-round fight. Deeper in, a zone-three guardian was unbeatable at
level 90 in the best gear the game ships; it now wants about level 110. The
levels a zone asks of you go up by roughly the same amount each time rather
than doubling, which is the whole point — there is no longer a zone where the
game quietly stops being finishable.

Existing saves keep playing. If you are wearing something you equipped at
zone 2 or deeper, its bonus was banked at the old rate and stays that way
until you take it off, at which point you keep a few points you did not
strictly earn.

### Also

- **The arena can stage a lair guardian.** `Encounter::Lair` on the arena
  screen and in a scenario file fights the thing at the bottom of a stack,
  which nothing could reach before — corridor ambushes never roll a boss.
  `dev-saves/deep-lair.ron` is the run the numbers above came from.
- A Recompile Kernel's zone bump was doing nothing past the second tier. It
  applies properly now.

## 0.8.0

### Your programs can wear your gear

Every program you own now wears the same three slots you do — Weapon, Armor
and Module — out of the same cargo. Press `E` on the roster (`p`, then
Companions) and the program under the highlight gets a page of its own three
slots; picking one opens the replacement picker you already know, showing
what each swap would change for *that program*.

A copy is interchangeable. What comes off your back goes on a program's, and
comes back off again into your cargo — nothing is bound to a wearer and
nothing is consumed by being worn. That is the point: you have been hoarding
every second copy of every weapon you ever found, and the programs fighting
beside you have been going in bare while you outgrew them.

Gear always comes home. Sell a program, extract its routine, fuse it away,
lose it in a fight or lose it defending the base, and whatever it was wearing
is back in your cargo. A trader appraises the program and never the gear it
happens to be holding, so kitting one out before selling it neither raises
nor lowers what you are paid.

Two things worth knowing before you spend anything:

- A **Decompiler bonus does nothing on a program.** Only you ever attempt a
  capture. Ten shipped items carry the stat, they still equip, and the slot
  page says so at the top rather than leaving you to work it out.
- A refactor still upgrades the program and only the program. Whatever it is
  wearing is lifted off for the arithmetic and put straight back, so a
  Recompile Kernel cannot quietly bake a borrowed weapon into a program's
  permanent stat block.

### Saves are now plain text, and this should be the last time they break

Every save-format break this game has ever had — nine of them, v19 through
v28 — was one thing: a struct gained a field. None removed anything, none
changed what anything meant. They broke your saves anyway, because the file
was a positional binary encoding where an added field shifts everything
after it.

The save is now the same field-named text `savetool dump` has always
printed, with the version on the first line. A field added from here on
loads out of a file written before it existed, so **an additive change no
longer breaks your save at all**. What still can is a field removed, or one
whose meaning changes — and that needs a real migration whatever the file
looks like.

Two things follow. Saves are readable and editable in any text editor, no
`savetool` round trip required. And they are bigger: the measured save went
from 13 KB to 190 KB, which costs 1.46 ms to write and read back rather than
74 µs — far under a frame, so autosave is unaffected.

### Saves from 0.7.x will not load

The save format moves to v29 — v28 for the loadout, which is new state that
has to be written per creature, and v29 for the text format above.
**Existing saves stop loading** — the game says so plainly rather than
failing strangely.

If you want to carry a run across, dump it to RON with a 0.7.x build first,
then pack it with this one:

```sh
cargo run --bin savetool -- dump saves/<your-save>.bin save.ron   # on 0.7.x
cargo run --bin savetool -- pack save.ron saves/<your-save>.bin   # on 0.8.0
```

The dump has no loadout in it and every program comes through wearing
nothing, which is correct — none of them could have been wearing anything.
The `dev-saves/` templates need no migration at all for the same reason.

## 0.7.7

### A decompile that fails still gets you somewhere

Every attempt on a program's ICE now leaves the next one better off. A failed
decompile used to cost a catalyst and change nothing at all — the odds cell
read the same number afterwards as before, so a run of bad luck was pure loss
and the only thing that moved was your stock of ICE Breakers.

Each attempt against a given program is now worth +10% on the next attempt
against *that same program*, up to five attempts' worth — a hard ceiling of
1.5x. The failure line says which side of it you are on:

```
The program's ICE holds — decompile failed! Its defences fray a little.
The program's ICE holds — decompile failed! Its defences are as frayed as they will get.
```

The counter belongs to the fight, not to you and not to the species. Jack out
and come back and the program is met with its ICE intact, which is what keeps
this "you are wearing this one down" rather than a pity meter you could bank
against a later target by farming an easy one. The cap is the other half of
that: it multiplies alongside the skill and Capture Boost terms rather than
adding to the base, so persistence can never out-scale a species' own
`taming_difficulty` — a stubborn program stays a gamble however many
catalysts get fed to it.

The battle screen needed no changes: it already quotes live odds per group,
so the number simply climbs as you work, and it rewinds with the narration
along with everything else on the row.

## 0.7.6

### A level-up says what it gave

Levelling announced itself and stopped there — the stats it grew were left for
you to notice on the character screen later, if at all. The player, a party
member and a posted worker now each print a block of stat lines under the
announcement, one per stat that actually moved:

```
You gain 40 XP and reach level 5!
  Max HP 108 → 120
  ATK 14 → 15
  DEF 11 → 12
  Perk Points 3 → 4
  Decompiler 4 → 5
```

The Perk Point and the point of Decompiler skill a level pays were previously
announced nowhere at all, so a player could bank points for a run without
learning they had any. Those two rows are the player's alone; companions get
neither.

A stat that didn't move is left out rather than printed as unchanged, since
attack and defense both grow by one point a level and a low growth multiplier
rounds them away on a given level.

The lines carry `before → after` rather than a bare `+1 DEF` for a reason
worth recording: the history screen folds identical lines together, so two
programs each gaining a point of defense in the same fight would have
collapsed into one row and deleted the second one's line outright.

### A posted worker's level-up is its own line

It used to be glued to the end of the extraction line as a tail, which left it
drawn as loot rather than as a level-up, and dropped from the results a
finished intrusion keeps. It is now a base-log line in its own right, named by
the machine the program is posted to — so a base running several cronjobs says
which one grew.

## 0.7.5

### Haulers walk around the base instead of through it

A posted program's route was a walk over terrain, and a structure never makes
its tile unwalkable — the player is blocked from one separately. So a hauler
carrying a load to a depot walked straight over the machines its owner had to
walk around. It now refuses any tile a structure stands on, and the tile a
worker is sent to stand on while working or delivering is picked the same way.
One exception, and it is deliberate: a worker may always step *off* its own
tile, because deploying a structure never checks whether a program is standing
there and a worker built over would otherwise be stuck for the rest of the run.

Some tightly packed bases will find a machine that no longer has anywhere to
stand. That is not new — you could never have collected from such a machine
either, since you cannot stand on a building — it was simply invisible before.

### Machines say when their program cannot reach them

A new **cut off** state, distinct from a machine whose program is merely away:
it means no route exists at all, and waiting will not fix it. It draws red
rather than yellow, because it is asking you to go and clear a path.

Posting now tells *walled in* apart from *no route to it from here*, instead
of calling both "too far away" — which had become a lie about a machine you
were standing beside.

### Demolish with `d` and a direction

`d` on the map aims at one of the four neighbouring tiles and demolishes what
is there, without opening the base menu. Adjacent only: this key destroys what
it finds, so you have to be standing next to it. Removing your Home still asks
first, and the key is refused underground. The menu route is unchanged.

### Fixes

- A machine with nobody posted to it now reports **idle** and draws grey. It
  never did for an extractor: only assemblers were ever told they were idle,
  and the status defaults to *running*, so a freshly deployed Research Node
  drew green as though it were producing, and a machine whose program was
  killed or reassigned kept its old state for the rest of the run.
- A machine part-way through a long cycle now reads as running rather than
  falling back to that default.
- The manifest shows a rare spawn's **Optimized**/**Overclocked** flag. It was
  previously readable only as a bar on the map tile and a tag on the battle
  roster, with nowhere to go and check.

## 0.7.4

### The roster tuner lands

`cargo run --bin tuner` is a seeded hill-climb over the shipped species
roster: it perturbs `assets/species/` into a scratch install, fights the
result against authored arena scenarios, and scores the outcome against an
objective read as data from `dev-tuning/objective.ron`. It has been parked
on a branch since `0.5.10`; the code is unchanged apart from being brought
up to the current arena API, and the whole of it is developer tooling —
`assets/` is never written to, and the tool proposes rather than applies.

Two things about it are worth knowing before running one. Its constraint
layer refuses any candidate that would nerf a program the player actually
fields, so a search cannot buy an easier fight by weakening the party's own
roster. And its headline finding is recorded in `dev-tuning/NOTES.md`:
**Stack depth 5 is not a roster problem.** Zone and depth stat scaling
compound — 4x at zone 3 times 3.32x at depth 5 is 13.3x base stats — which
floors every party hit at `MIN_DAMAGE`, and no species file inside any sane
bound can undo a multiplier. The tool was built to answer that question and
the answer is that it should not try.

## 0.7.3

### Fix: the `chains` template's spare programs carry their species kit

The six spare programs `0.7.2` added were hand-authored holding a single
`priority_boost` between them, so they opened with nothing but the default
buff. A save's routine list is taken verbatim on load —
`install_innate_routines` runs when a program *comes into existence*, a
decompile or a fusion, and never on a load — so nothing filled the gap in.
At level 12 they hold six slots and now carry what their species grants:
`redundancy_sync`/`rollback_v1` for the Medics, `skim_group`/`skim_v3` for
the Leeches, `overclock_array`/`sandbox` for the Bastions, with
`priority_boost` kept beside them.

Their jobs at a post were never affected — a class's work at a structure is
passive and reads nothing from the routine slots.

## 0.7.2

### The `chains` dev template is now a factory sandbox

`cargo run -- --template chains` already stood up a three-stage production
chain, but it opened with eight of the twenty-one research nodes and fifteen
Core Fragments — enough to watch the chain it ships with, not enough to build
a second one. Testing the factory therefore still started by playing the
research tree. It now opens with:

- **Every research node unlocked**, so the whole build menu and every bench
  recipe is available. The eleven structures no research file names were
  already unlocked by default, so nothing is behind a gate any more.
- **Deep cargo**: 600 Core Fragments, 120 each of the four bench
  intermediates (Bytecode Block, Blank Substrate, Logic Wafer, Charge Coil)
  plus Annealed Cores and Raw Trace, 120 Power Cells, 100 Outlets, 20 Portal
  Fragments, 5000 Credits, 500 Research Data, 200 ICE Breakers, 50 Routine
  Disks — including the four intermediates that gate a bench's own
  `build_cost`.
- **Six unposted programs** standing on the player's tile — two each of the
  three classes that do something at a post, named `Spare Medic A/B`,
  `Spare Leech A/B` and `Spare Bastion A/B` — so the base jobs are testable
  without taming first. They are on top of the nine already running
  cronjobs; the five-slot party is unchanged.

Developer tooling only: no shipped asset, no engine code and no save format
changed, and the two gates that keep a template honest —
`every_checked_in_template_still_loads` and
`the_chains_template_starts_with_a_chain_that_actually_runs` — both still
pass against it.

## 0.7.1

### Three of the five classes now have a job at your base

A class has meant a stat shape and a kit — both about a fight. It now also
decides what a program does when you post it to a structure.

- A **Leech** draws an extra unit out of every successful gather cycle.
- A **Bastion**'s Defense counts twice against a GC Entropy Sweep on the
  structure it is guarding.
- A **Medic** repairs that structure by 2 Durability every 20 ticks, which
  is the first repair in the game that isn't a Patch Node — and the only
  one a base with no Patch Node has at all.

A **Striker** and a **Saboteur** do nothing at a post, and that is the
decision the feature is for. You have three pet slots: every program at a
machine is one absent from the party, so a base that runs itself is a party
that can't fight, and the classes that are worth posting are exactly the
ones you least want in the line. The manifest's WORK box now names each
program's job beside its Speed and Analysis, so the trade is legible before
you make it rather than after.

Two limits worth knowing. A Leech draws nothing extra from a Research Node
— banked resources pay a flat rate to everyone, which is what keeps the
research ladder priced — and a Medic only mends while it is *guarding*: put
it on a cronjob and it is extracting, not repairing.

### Every program now knows two moves, and which two says what it is

Until now, eight of the seventeen species taught a captured program
nothing at all — it fell back on `priority_boost` and stayed there for the
rest of the run, and the five that did teach something taught it in no
particular pattern. Every non-boss species now grants exactly two
abilities: a **class utility at level 2**, shared by all three members of
its class, and a **tier rung at level 6** that it holds alone.

So a program tells you what it is twice. The Drone, the Worm and the
Rootkit all open with Skim Group, which is what a Leech does; what
separates them is the second unlock, where the Drone learns Skim Single
v1.0 and the Rootkit v3.0. The same split the stats already make — the
tier sets the budget, the class spends it — now runs through the kit.

Fourteen new routines, in five families. Three of those families are new
(Segfault, Rollback, Skim) and one of them fills a real gap: **Drain was
previously findable only in the field**, so the Leech class had no move it
was allowed to know. Skim is a Leech's own drain, and the three hunt-only
Leech routines stay exactly where they were — the hunt-only pool is
unchanged at twenty-eight, and nothing that was a prize has become
standard issue.

**Nothing unlocks at level 1**, deliberately. `priority_boost` is what a
companion falls back on when its species has taught it nothing *yet*, and
extracting it from such a companion is the only way to get it — so a
species granting anything at level 1 would delete it from the game. It also
means a freshly tamed program reads as generic for a level before it reads
as its class.

Two routines you may already own display slightly differently: Bastion
Single is now **Bastion Single v1.0** and Bit Rot Single is **Bit Rot
Single v1.0**, because both are now the bottom rung of a ladder. Nothing
about either has changed and no save is affected — the names moved, the
ids did not.

`dev-arenas/class-mirror.ron` is the instrument for all of this, and it
exists because neither offline harness can be: `balance_sim` models no
abilities and the `arena` bin never fires a Special. It stages a Bastion,
a Medic and a Leech of one tier against a pack picked for the one thing a
kit needs, which is rounds to spend it in.

### Seventeen species stop being one axis wearing seventeen names

Every non-boss program now belongs to one of five classes — Striker,
Bastion, Medic, Saboteur, Leech — and none of them is a field in a file.
A class is the affinity axis a species raises, the shape of its stats and
the pace it moves at, all saying the same thing. A Crawler is now a wall
with 102 HP behind 9 DEF and almost no bite; a Scrapper of the same tier
is 80 HP, 12 ATK and 3 DEF. Before today those two were 75/8/4 and 98/9/5
— the same creature, one of them slightly larger.

What makes the role readable is that it is **independent of tier**. A
species' growth band sets a stat budget and its class decides both how
much of that budget it gets and how it spends it, so "low DEF for its
size" reads the same at tier 1 and tier 3. It could not before: a tier-3
striker out-tanks a tier-1 tank on raw HP, which made "tanky" a thing you
could only see if you already knew the ladder. Every raised affinity is
1.3 and every damped one 0.85, so the magnitudes say nothing and the axis
says everything; the manifest's AFFINITIES box, previously hidden for
eleven of seventeen species, now has something to show for all fifteen
ordinary programs.

Speed carries the class too, which since the last entry means it carries
into the base: Bastions and Leeches are slow at a machine and Saboteurs
and Medics quick. The Drone in particular drops from 13 to 8 — the
commonest early worker is now a slow one, and what it gets back for that
arrives in a later release.

**The Construct has moved down the ladder**, from the 1.5 growth band to
1.0, because the four species already carrying affinities plus the Virus'
pinned 1.5 fill that tier's five roles. It is a tier-1 wall now, 49/2/4,
and cheap to compile at 0.35; its Crash came down from power 13 to 8,
which was the highest move power of any ordinary program and made no
sense on an opening-ring statline. It does now spawn in the opening ring,
which is five species rather than four.

**The projected progression curve moved**, and is recorded rather than
retuned. `balance_sim`'s median party species is a Proxy instead of a
Scrapper, and the levels its sweeps project to clear each zone with a
full party are:

| zone | grind-only | geared | full roster |
|---|---|---|---|
| 2 | 19 → 15 | 14 → 10 | 11 → 8 |
| 3 | 40 → 30 | 29 → 26 | 26 → 22 |
| 4 | 77 → 63 | 55 → 61 | 49 → 55 |
| 5 | 149 → 131 | 100 → 127 | 96 → 117 |

The shallow zones got easier and the deep ones harder, both for the same
reason: the toughest ordinary program is a Sentinel, and a Sentinel is a
Bastion, so its ATK fell from 9 to 6 while its DEF held. The hardest
ordinary fight in the game is now a longer, safer one. The gate itself
asserts the curve's *shape* — monotonic, geometric, gear beating grind —
and all of that still holds.

### A species' stats now say something about work, not only about fighting

Two of a species' base stats picked up a second job today. `base_speed` —
until now read only to order turns in a fight — also sets a species' pace at
a machine: post a Sprite (`base_speed: 14`) to a Mining Node and its 10-tick
cycle becomes 8; post a Construct (`base_speed: 6`) and the same node takes
12. `base_int`, a stat with no meaning at all before this, is read as a
fourth term on the extraction roll, alongside the node's own tier and the
player's Keen Scavenger perk — a Cipher and a Construct posted to the same
Mk1 node now visibly disagree, 0.58 against 0.40. Working a node yourself is
untouched by either change: the player has no species, so both read at
exactly the roster's baseline, which is what keeps posting a sharp program
better than doing the job by hand, and a dull one worse.

The manifest has a new WORK box to say so — Speed and Analysis, the two
numbers that describe what a program is like to post somewhere, sit
together rather than getting buried in SPECIES or duplicated across two
places on the same screen. An assembler now runs at the rate baked in when
its program was posted, rather than at its structure's flat
`ticks_per_unit` regardless of who was staffing it — previously two very
different species posted to the same Assembly Bay ran identically. A
cronjob already posted before this update, including one loaded from an
older save, keeps its old rate until reassigned; nothing recalculates a
running job's pace mid-cycle.

Both effects are sized by their own tuning constant
(`MINING_SUCCESS_PER_INT`, `WORK_TICKS_PER_SPEED`), independent of the node
or machine's own numbers, so how much a species' choice matters can be
retuned without touching how much the machine itself matters.

## 0.7.0

**Breaking: existing saves will not load.** `SAVE_FORMAT_VERSION` goes 26 →
27 because every program now records the upgrades spent on it. To carry a
game across, dump it to RON *before* installing this version and pack it
back afterwards — RON is field-named, so the new fields fill themselves in:

```sh
cargo run --bin savetool -- dump saves/save.bin s.ron   # on the old build
cargo run --bin savetool -- pack s.ron saves/save.bin   # on the new one
```

### Refactoring: your programs can keep up now

A tamed program's stats were baked once, at capture, and never rescaled.
Enemies double every zone. So a Scrapper caught in the opening ring was
permanently anchored to zone-1 numbers, and the only answers were to throw it
away for a fresh catch or to fuse it — both of which are ways of *replacing*
a program rather than decisions about the one you have. Nothing on screen
even said it was happening.

There are two permanent, player-driven upgrade tracks now, both off a new
production line: Mining Node → **Annealing Node** → **Refactor Bench**, behind
one research node. Apply them from the party menu; it works underground, which
is where you are most likely to notice a companion falling behind.

- **Recompile Kernel** — rebuilds a program for the zone you are standing in,
  doubling its stats. Refused once it has caught up with you, which is what
  bounds it, and it costs no upgrade slot: nobody should have to burn a
  permanent slot merely staying current. The bench assembles these on a timer,
  because it is the thing you want again after every breach.
- **Six percentage buffs**, one per stat — `+5%` crafted at the bench, `+12%`
  off a boss or, rarely, off a mid-tier program. Each spends one of a
  program's five permanent upgrade slots, so which stat gets them is the
  choice.

Percentages rather than flat amounts, because a companion's numbers keep
growing and a `+15 HP` buff means nothing at 500 HP. They also commute with
the zone rebuild, so a buff bought in zone 1 is worth as much three breaches
later and there is no ordering to exploit. Small stats floor at `+1`: `+5%` of
a Drone's 3 ATK would otherwise round back to 3 and do nothing to exactly the
programs the feature exists to rescue.

**The manifest now tells you when a program is behind**, which it never did —
a zone tag reading "1" means nothing without the zone you are standing in
printed beside it.

**Traders pay for what a program is, not what you spent on it.** A trader
offers a tenth of a program's power, so upgrading one and selling it would
have turned printable salvage into Credits — the one currency that survives a
breach — at a rate that compounded with every rebuild. Bought tiers are
divided back out of the sale. Tiers *earned* by taming something deep are
untouched: beating it is what the game charges for that.

### Also

- Fusing two programs keeps the higher zone tier and the higher upgrade count
  rather than resetting both, which would otherwise have laundered a maxed-out
  program back into a fresh one.
- Item files may now declare an `upgrade` block; see
  `assets/items/README.md`. A negative percentage or one that declares no
  effect is refused at load with the rest of the malformed files.

## 0.6.0

**Breaking: existing saves will not load.** `SAVE_FORMAT_VERSION` goes 25 →
26 because every creature now records the rare tier it rolled. To carry a
game across, dump it to RON *before* installing this version and pack it
back afterwards — RON is field-named, so the new field fills itself in:

```sh
cargo run --bin savetool -- dump saves/save.bin s.ron   # on the old build
cargo run --bin savetool -- pack s.ron saves/save.bin   # on the new one
```

### Rare programs: Optimized and Overclocked

Wild programs used to vary along one invisible axis — a ±20% roll per stat,
readable only as a "quality" label on a screen you reach *after* catching
something. There was no moment on the map that said *that one, go get that
one*.

There is now. A wild spawn can come up **Optimized** (uncommon, 1.5x stats)
or **Overclocked** (rare, 1.8x). Both multiply on top of the existing roll,
so an Overclocked program lands between 1.44x and 2.16x an ordinary one of
its species — a real threat, and a real prize.

- **You can see one coming.** A rare program wears a silver or gold bar
  along the top of its tile. Its glyph still shows the difficulty colour,
  because how dangerous something is and how rare it is are two different
  things you need at once.
- **Catching one keeps it.** Decompiling never re-rolls stats, so a program
  you take stays Optimized or Overclocked for the rest of the run — and
  fusing two keeps the better of the pair rather than laundering it away.
- **It pays for itself.** A kill already pays the defeated program's max HP
  as XP, so a rare one is worth proportionally more.
- **Bosses and the opening ring are excluded.** A boss's stats are authored
  by hand, and the first ring around your landing site is guaranteed to be
  beatable by a fresh player. Neither gets a tier.

Both the spawn chances and the multipliers are tuning constants
(`SILVER_SPAWN_CHANCE`, `GOLD_SPAWN_CHANCE`, `SILVER_STAT_MULT`,
`GOLD_STAT_MULT`), so the rate and the reward move independently.

## 0.5.23

### The examine key no longer reads through shut doors

`0.5.22` shipped `x`-then-a-direction with a real defect: the ray it walks
to find something to describe never checked whether anything was in the
way. A door two cells ahead is opaque to the eye and to the map, but not to
`x` — so standing in a corridor and looking at a closed door reported the
unopened cache sitting behind it, seal intact. You could read the contents
of a room by looking at the door to it.

The ray now stops where sight stops, using the same occlusion rule the
first-person view and the frame map have always used. It stops *at* the
blocker rather than before it, so looking at a door still describes the
door — a wall in plain sight is a thing you can look at. Standing inside a
doorway still shows you the corridor beyond, which is the one case where a
sight-blocking cell is also one you can occupy.

Two smaller corrections to the same key. The cell immediately to your left
or right could not be examined at all — the ray skipped its whole nearest
rank rather than just the square underfoot — and nothing distinguished left
from right, so the two could have been swapped without any test noticing.

### Flavour text that described mechanics the game does not have

Three shipped fragments claimed rules that were never implemented. Rotten
substrate said standing on it "a while costs more"; the bleed fires once
when you arrive, so waiting is free. Corruption said it "spreads slower
than you walk", and a cleared lair was "already starting to rot back over";
corruption is placed when the frame is generated and never spreads at all.
All three now describe what actually happens.

### Fixes

- The description bank's schema doc contradicted itself on how long an
  underfoot line may be and on whether a fragment may run to two sentences,
  and did not mention that the frame-arrival subject is the one place a
  `{bearing}` token would reach the screen unexpanded. It is the prompt an
  author works from, so an error in it propagates into content.
- Engine tests no longer leak scratch directories under `/tmp` when a test
  panics; they now use the same cleanup guard the rest of the suite does.
  The failure mode this avoids is inode exhaustion, not disk space.

## 0.5.22

### Generated flavour prose for the Stack

The Stack's first-person view used to hand you one string per cell: a key
prompt like `"A link leads down  [>] descend"` and nothing else. Every cell
now composes real prose around that prompt from an offline-authored bank,
on three surfaces:

- **Underfoot.** The row under the view still carries its key prompt, but
  the description in front of it is drawn from the bank instead of a fixed
  literal.
- **The log.** Walking a notable cell into view — an unopened cache, a live
  breakpoint, a sealed door — writes one line about it, at most once per
  move, for the single most notable thing that just came into sight.
  Arriving on a new frame writes a one-line mood beat of its own, once per
  frame regardless of how many steps it takes to get there.
- **Examine.** Press `x` underground, then a direction, to read a full
  paragraph about whatever's ahead, to either side, or underfoot.

A given cell of a given stack always reads the same way — the door you
walked past once reads identically the second time — and a different stack,
or a different depth of the same one, reads differently. Nothing is stored
to make that true: every line is derived from the world seed, the entrance,
the depth and the cell, the same way the Stack's own layout already is.
**No save-format change** — an existing save loads with the feature already
working, nothing to migrate.

### The crash-log reader draws from the same bank

The `Z` key's crash-log reading used to draw from its own small, separate
pool of eight lines. That pool is gone; `Z` now composes its reading from
the shared corruption vocabulary instead. The words you read are unchanged
— every line carried over verbatim — only where they come from did.

### Fix: examining underground no longer answers with a surface creature

Pressing `x` while in the Stack used to run the surface creature-and-structure
scan, because the party's on-map position stays pinned to the entrance tile
the whole time underground — so it could name a creature standing near that
entrance as your target while you were several frames below it. Examining
underground now always describes the Stack cell you're facing instead (see
above); the surface scan refuses outright rather than answering with stale
ground data.

## 0.5.21

### Rename a program

Press `N` on the companion roster to give a program your own name for it.
The page opens on the name it already has, so fixing a typo isn't a retype;
clearing the field puts its species name back. It works on any program you
own, wherever it is — in the party, posted to a cronjob, standing guard —
because a name changes nothing about where it is or what it's doing.

Names survive saving, and always did: `CustomName` has been complete since
fusion learned to name its result, and fusion was simply the only way to
reach it. **No save-format change** — an existing save loads unchanged, and
a program you already named by fusing keeps that name.

Renaming is refused during a battle. The reason is the log rather than the
roster: the battle screen replays *rendered* rows as its narration scrolls
in, so a name changed mid-fight would leave what you're reading and what
you're looking at disagreeing about who is being hit.

### The stun measurement was reported on a cherry-picked sample

`docs/measurements/2026-08-10-stun-move-levers.md` claimed the 2-turn stun
retrain made enemies vary their moves more. It measured only the three
species whose files were edited. Across the whole roster three improved and
**eight got worse**, including all four opening-ring programs a new player
meets. The entry now carries the full table and the correction; nothing in
the game changed, but the note that justified a shipped change did.

### Most trained weights turn out to mean nothing

Retraining the enemy policy three times, changing only the optimiser's seed,
**seven of the sixteen free features flip sign** — while the enemy win rate
lands within 2 points every time. Only `target_hp_frac` and
`est_damage_frac` are stable. Three quite different-looking policies play
about equally well, so a single weight is not evidence of anything and
several claims across the measurement notes were reading them as if it were.
Written up in `docs/measurements/2026-08-10-weight-identifiability.md`, with
corrections threaded back through the two entries that leaned on a
coefficient. The shipped policy is unchanged and still doubles the enemy win
rate; what changed is what we are entitled to say about why.

## 0.5.20

Tooling and measurement only — nothing about a played game changes.

### The arena can exercise Defend

`arena::run_rep` played the party as All-Attack, so nobody ever braced and
no arena measurement could say anything about Defend at all.
`RunOptions::party` adds two plans: `BraceWhenHurt` (a member under half HP
Defends) and `BraceInRotation` (one slot per round, whatever anyone's
health). `--party-plan brace|rotate` reaches them from `train`.

All-Attack is still the default, so every number published before this keeps
meaning what it meant, and `the_default_party_plan_never_braces` holds it.

### The analysis refuses to present a confounded number

`analysis/policy_report.py` now checks each run for observables that move
together and warns above the first table. It was written after a run that
could not answer its own question — the brace rule fired on a threshold over
the policy's largest feature, so bracing and being wounded were one
variable. The check is grouped per run, because pooling a sweep hides the
confound behind the configs that never brace.

### What the instruments said

`docs/measurements/` gains the pin sweep, the stun-move levers, and the
bracing runs. The last of these confirms the three pinned policy features
are justified — and corrects the 2026-08-09 reason for one of them, which
read a weight that training could not have learned, since under All-Attack
`target_bracing` is constant and fitness is indifferent to it.

## 0.5.19

### Wild programs use their stun moves again

Three moves were effectively dead — cipher's Encrypt, crawler's Freeze and
rootkit's Privilege Escalation, each chosen on 1-2% of swings where the
program had a choice. All three are Stun, and each is priced below its
damage-only sibling, so skipping them was correct expected-damage play.

Each now stuns for **two turns instead of one**. Their power is untouched:
raising it to match the sibling was measured first and flips the problem
rather than fixing it, taking the stun move to ~96% and killing the *other*
move instead. Duration moves usage to 12-20% and leaves the enemy's win
rate where it was.

The retrained weights ship with it, and have to: stun duration is not one of
the policy's features, so the enemy cannot see the change directly. What it
changes is what training measures — longer stuns win more fights, so the
search stopped penalising the moves that carry them.

Measured in `docs/measurements/2026-08-10-stun-move-levers.md`, including
what the run could not establish.

## 0.5.18

### The trainer can now show its working

`train` records the two evaluation passes either side of its search —
the all-zero baseline and the trained result — as battle telemetry, one
file per config, pass and scenario. The 1.9M-fight search between them
stays unlogged on purpose: those are candidates that were thrown away,
and keeping them would cost tens of gigabytes to describe weight vectors
nothing ever used.

```sh
cargo run --release --bin train -- --label pin3 --log-dir dev-logs/policy-sweep …
```

Nothing about a played game changes. `arena::run` gained a `RunOptions`
argument that defaults to collecting nothing, so the headless arena bin
and the game's own arena screen behave exactly as before.

### Reading that telemetry, in Python

`analysis/` is the first Python in the repo with dependencies — pandas
and matplotlib, behind a venv and a `requirements.txt`. It answers who
the enemy swings at, how hurt the target is when chosen, and how much of
its moveset each species still uses. The training itself stays in Rust:
the objective function is the real game, so an optimiser over here would
have to call back into `arena::run` for every one of its evaluations.

### A place for what the instruments said

`docs/measurements/` — one file per question actually answered, each
carrying the commands that produced it and the blind spots it had. The
first entry finds that the three pinned policy features are a design
boundary a free search will always cross: trained unpinned, the enemy
downs **zero** companions across 1,600 fights where the baseline downs
267.

## 0.5.17

### A routine you can't run stops being pickable

Picking a routine the game had already greyed out — no ICE Breaker for a
Decompile, a cooldown still running, a full roster — opened the target
picker anyway. You chose the routine, chose who to point it at, and only
then found out it was never going to fire. The picker now refuses the
press on the reason already printed on the row: `Can't use Decompile — no
taming catalyst.`

Both other places a routine is chosen, the battle action menu and the
field cast list, already worked this way. The ability picker was the one
step of the three that skipped the check.

## 0.5.16

### A drop says what kind of thing it is

The battle log is where a player meets a dropped item for the first time,
and it named the item and nothing else. Every screen that *lists* an item —
the inventory, a trader's shelf — puts its category beside the name, so the
log was the one place you had to already know whether a "Hardened Shell"
was armour, a module or stock for the bench. Drop lines now carry the same
tag those screens do: `It also drops a Hardened Shell [ARM]!`

`Game::item_name_tagged` is the one formatter, reading the same
`ItemCategory::short_label` the columns do, so the two cannot drift. The
tag goes after the name rather than in front of it because a sentence has
no column to put it in.

## 0.5.15

### A played fight leaves something behind

The game could be played, and it could be measured, but not at the same
time. `arena` runs a fight offline and reports it; a fight a person plays
by hand produced nothing — the arena session writes no save and no profile
by design, the message log dies with the process, and the only artifact was
recall. So the question the trained enemy policy shipped with, whether a
party actually using its routines still loses to it, could be played out and
then not answered.

`FERAL_DEV_LOG=1` now records what happens inside a fight to
`dev-logs/battles.jsonl` — one JSON object per line, per swing, round and
decision. Five kinds: the fight opening with its party and pack, a snapshot
at the top of each round, every enemy's chosen move and target, every party
member's chosen action, and the outcome.

The number it exists for is `target_hp_before`, the target's HP *before* the
hit lands. Focus fire is a distribution over that number and a per-round
snapshot cannot show it: four attackers all act inside one round, and by the
snapshot they have finished. It is recorded at the single point where a wild
program's move and target are decided, so no swing can be missed.

`dev-logs/README.md` is the schema, one row per field. Unset — which is
every ordinary run and every player's build — nothing is collected, nothing
is written, and no file is created.

### An arena fight is allowed to write this one thing

An arena session deliberately touches no disk: no save, no profile, no run
history. That rule exists so a tester's fight cannot corrupt a save or pay
out profile rewards to a real player. A dev-only log does neither, and the
arena is the single place a recorded fight is most wanted — so telemetry is
a stated exception, with a test of its own sitting beside the three that
assert the opposite about everything else.

## 0.5.14

### Damage lands when the log says it does

A round used to resolve all at once. Every HP bar dropped to its
end-of-round value before the first line of narration was legible, so the
text was reporting a fight the screen had already finished — you read
"You unleash a data strike for 12 damage" against a bar that had absorbed
that hit and the three after it a second ago.

The roster now steps with the narration. A bar holds still until its own
line lands, then drops; a pack of three keeps reading as three until the
line announcing the kill, and only then does the next one step into the
front rank. Group letters, status tags, decompile odds and the planned
action all move at the same pace, so the whole roster is a picture of the
moment being described rather than of the moment after.

Nothing about the fight itself changed — the same round resolves in the
same order with the same outcome. What changed is that you can watch it.
The damage flashes and floating numbers came along for free: they were
already reading the difference between one frame's HP and the next, so
they now fire one hit at a time without having been touched.

### A fight ends on its own screen instead of dumping you on the map

Winning used to close the battle screen on the spot and leave the loot and
XP sliding past in the map's log pane, which is where a fight's results
were least likely to be read. The screen now stays up: same rosters, same
pane, with the results arriving at the same pace as the fight's narration
and the action bar replaced by `[any key] continue`.

It holds for every ending — a win, a jack-out, or a defeat you were
rebooted from. A key pressed while the results are still arriving releases
them rather than dismissing, so loot cannot be skipped past unseen. A run
that actually ended still goes straight to the game-over screen.

On a win the hostile pane empties out, header and all, because by then
there is genuinely nothing left in it. Jack out instead and the pack is
still listed, which is a better look at what you ran from than the map
ever gave you.

## 0.5.13

### Wild programs have learned to fight

Which move a wild program swings and who it swings at used to be two
uniform rolls. They are now one decision, scored against a policy trained
offline over 1.9 million arena fights and shipped as
`assets/policies/enemy_battle.ron`. Across the eight training scenarios the
other side's win rate goes from 32% to 61%.

What changed at the table: they finish what they started. A companion at a
sliver of health is the one that gets hit, a kill that is available gets
taken, and the move chosen is the one that does the most to *that* target
rather than whichever came up. In a group fight this is a different
opponent. One-on-one it is much the same, because there was never a choice
to make there.

The trained file is legible on purpose — it is a list of named weights, and
`assets/policies/README.md` reads the shipped one out loud. Delete the file
and the game plays exactly as it did before; that is a supported way to
play, not a broken install. A mod can ship its own, and a species with a
moveset nobody trained against is scored by the same weights, because the
policy reasons about what a move *is* rather than about which move it is.

### Bracing is a stronger draw

`DEFEND_AGGRO_WEIGHT` is raised from 4 to 7, and the reason is the feature
above. Defend's pull was tuned against an opponent picking at random, and
it does not survive one that thinks: reducing incoming damage is what
bracing does, so anything choosing by damage has a reason to hit somebody
else instead. At the old value bracing was quietly *counterproductive*
against a trained program. It now does what its description says again.

Two rounds of training were thrown away establishing that. Left to
themselves the weights learned to kill the player and ignore the party
entirely, and — when that was forbidden — to walk past whoever braced by
reading their Defence instead. Both routes are now closed off, a test
fails if either reopens, and
`docs/superpowers/reports/2026-08-09-enemy-policy-training.md` has the
whole account, including the mechanic this turned up that has *not* been
fixed: every species prices its status-effect move below its plain one, so
a program that thinks about damage will never use one.

### For anyone poking at the internals

- `train` is a third launcher binary, alongside `savetool` and `arena`. It
  runs a cross-entropy search over the arena harness; `dev-training/` holds
  the eight scenarios it learns from, and `dev-arenas/` is held back as the
  test set and never trained on.
- `tuning::ENEMY_POLICY_TEMPERATURE` is the one dial. Raise it to blunt the
  policy without retraining; 0 makes it play its best move every time.
- No save-format change. Weights are an asset, so retraining reaches a run
  already in progress.

## 0.5.12

### The sector is populated everywhere, not just where you have been standing

Wild programs gathered around wherever you spent your time. A base you had
been working at for a while would be ringed by them — sixty-five inside a
single screen on one save — while the ground an hour's walk out held almost
nothing. Travel far enough and the sector read as abandoned.

Both halves of that were one fault. Programs spawned near you and nothing
ever removed them, so the population was a record of where you had stood
rather than a property of the place. Resting is forty ticks in one spot,
and every machine you tend is more, so your base collected them at a rate
nothing else in the sector could match. Meanwhile the far ground had never
been seeded at all — a new sector's opening population was scattered within
a few steps of where you arrived — and walking costs a turn a tile, which
is faster than programs appear. You were outrunning them.

There is now a density the sector is held at: roughly a screenful of wild
programs around wherever you are. A new sector is seeded to that across the
whole area you might travel, out as far as its Stack links, and the ongoing
spawning tops it back up rather than adding without limit. Standing still
no longer accumulates a crowd, and arriving somewhere new no longer means
arriving somewhere empty.

### Wild programs stay off your base platform

The platform is meant to be the one safe ground, and three separate rules
in the game already said so — you cannot be ambushed on it, anything
standing there is cleared when you lay the floor, and a nest's swarm will
not chase you onto it. An ordinary wandering program was never told, and
could simply walk in. It took a crowded sector to make it common enough to
notice, which is why it surfaces now.

## 0.5.11

### Rooms in the Stack stop being corridors with the sides missing

Standing in an open frame and looking down it, the floor and ceiling ran
away ahead of you and everything to your left and right was flat black.
Corridors were fine — their walls filled that space — so the fault only
showed in the rooms and chambers that arrived in `0.5.9`, and it made a
hall read as a narrow passage someone had cut the sides out of.

The view was drawing one column of cells, straight ahead. The cells beside
you were being looked at for a single thing: whether to put a wall there.
When the answer was no, nothing was drawn at all, and the background
showed through.

The whole cross-section is drawn now, so the cells to either side are
floor, wall, doorway, cache or lair the same as the one ahead — including
their markers, so a cache off to your left is something you can spot
rather than something you find by walking into it. What is *behind* them
is unchanged: a passage running past the rock ahead of you shows, and one
running behind it still doesn't.

Two things remain that no cell accounts for — the far end of a corridor
that continues past what you can make out, and the outer edges of a hall
wider than your field of view. Both used to be the same flat black, which
read as a hole in the world. They are now the dark the light doesn't reach.

## 0.5.10

### The test suite stops filling up `/tmp`

A build died with `No space left on device` on a filesystem that was 15%
full. The number that had run out was not bytes but **inodes** — 1,048,576
of 1,048,576 — and the suite was producing them at 10,741 per run, which
exhausts the table in about 97 runs.

`scratch_assets_dir` builds a test a private copy of the whole shipped
asset set, eight directories and ~190 files, and left deleting it to the
caller. Two shapes defeated that: a test that panics on a failed assert
never reaches its own cleanup line, and a helper returning a bare `Game`
had nowhere to put one. Neither is exotic — between them they had left
5,437 stale installs on the machine.

Cleanup is now an RAII guard, `ScratchAssets`, so it happens on the
unwinding path too. Its `Drop` is deliberately best-effort: turning a
failed removal into a second panic mid-unwind would abort the process and
bury the assertion that actually failed.

Measured over a full workspace run, before and after: 62 directories and
10,741 inodes become 9 and 34. The remainder is app-core's save fixtures
and the arena's, which are different helpers and are left for their own
change.

## 0.5.9

### The Stack is not all maze any more

Every frame was carved by the same maze generator, and the way down sat on
the single furthest cell from where you came in. Between them that made one
kind of trip: pick a direction, weave the whole map, find the opposite
corner. Doing it again three frames later was the same trip with different
walls.

A frame now rolls one of three shapes. The maze is still one of them,
unchanged. **Rooms** is rectangles joined by corridors, with more corridors
than it takes to connect them, so a wrong turn is usually a loop rather
than a walk back. **Chambers** is four open halls joined in a ring by
passages three cells wide — the one you cross without looking for a door.
Which you get is a property of the stack and the depth, so a particular
hole in the ground always opens onto the same thing, and climbing down a
level changes it.

The way down moved too, and on its own it is half of what made a frame feel
long. It is drawn from the far half of the frame now rather than from the
one cell furthest away: still a real walk, still never just around the
corner, but not the opposite corner every single time.

Caches and the orphaned program still live at the end of side passages —
which rooms and halls do not naturally have, so the generator now carves
them on purpose. That closed an old gap on the way past: the maze itself
ran short of them about one frame in four and quietly shipped two caches
instead of three.

A save made underground still loads. A frame is rebuilt from the world seed
rather than stored, so the walls around a saved party have genuinely moved —
if yours were standing where a wall now is, they come back at the way up
instead.

## 0.5.8

### The lair has no key

Reaching the guardian at the bottom of a stack meant carrying an Access
Shard, and shards came out of caches. So a descent could end at the door
you came for, with nothing to do about it but climb back out and hope the
next run's caches were kinder — a gate that decided whether the deepest
fight in the game happened at all, on a dice roll made three frames higher
up.

A sealed door is now a barrier rather than a lock. You shoulder it open by
walking into it, it stays open behind you, and what it costs is the noise:
the same Trace it always raised, now the whole of the price. Everything
else about it holds — it still walls the lair off, you still cannot see
past it, and phasing or jumping into the wing behind it is still refused,
because the lair is entered through its door.

The Access Shard itself is dormant rather than gone. Nothing asks for one
any more and caches have stopped stocking them, which makes a cache's haul
slightly thinner than it was; the shard keeps its name and its price, and
what it is for instead is an open question.

## 0.5.7

### A debug port might not give you the frame

Jacking into a breakpoint handed over the whole frame's map, every time, for
a walk and the loudest Trace raise in the game. The only question it asked
was whether the walk was shorter than mapping the frame on foot, and the
answer was always yes.

A jack-in now takes six times in ten. When it fails the port stutters and
resolves only the substrate you are standing in — a small patch around you,
but one that reads *through* walls, which the view down a corridor never
does, so a failed jack still tells you which way the junction you are on
actually goes.

Either way it is one try: the port burns out on the attempt, and the Trace
is charged for jacking in rather than for what came back. There is one port
per frame, so this is a decision about a walk you can see the length of, not
a lever to pull until it pays.

## 0.5.6

### Doors you can see coming

A door at the end of a Stack corridor was invisible until you were standing
in front of it. Doors are drawn as a face filling the passage, in their own
colour, and that colour was all that separated one from the rock beside it —
so the fog that makes the corridor recede also took the only thing saying
"door". Three cells out, both were near-black.

A door now carries the same orange `+` the frame map marks it with, and a
sealed one the same red, painted in the middle of its face. The marks that
were already there — links, caches, lairs, faults, corruption, orphans —
fade more slowly with distance than the walls do, so the layer that exists
to be spotted down a corridor stays legible at the far end of the view.

## 0.5.5

### A posted program starts from where you are

Posting a program to a structure could produce a cronjob that never ran. The
machine sat yellow, its working animation never played, and the log said
nothing beyond "Cronjob scheduled." — the program was standing somewhere it
could not walk back from, and never took a step for the rest of the run.

A program's position on the map is set when you beat it and never changes
again: it does not trail after you as you walk, which is why your companions
are not drawn on the map at all. So a program tamed far from home was still,
as far as the sim was concerned, standing out in the wild — and a machine
more than fourteen tiles from that spot was unreachable, whatever the
program's actual whereabouts.

A program now sets off from your own tile, because that is where you have
been carrying it. One consequence is worth knowing: the walk to a post is
bought by posting from a distance. Post while stood at the machine and the
program is already at its station; post from across the base and you will
watch it walk in. A structure too far away for the walk is now refused
outright, with a message, instead of accepting a cronjob that could never
start.

The cronjob and guard pickers were filtering the same stale positions, so a
program tamed more than forty tiles out was missing from both lists — and
because the base menu hides a row whose screen would be empty, a player whose
only program was tamed that far away lost the Cronjob row entirely. Both
pickers now offer every program you own, wherever you caught it.

## 0.5.4

### Structures throw sparks when a sweep hits them

A GC Entropy Sweep used to be a coloured wash over a tile and a line in the
log. Damage and destruction now throw debris: streaks flung outward from the
tile, decelerating and fading, twenty of them when a structure comes down and
fourteen when one merely takes a hit.

The two bursts are shaped differently on purpose. A structure coming down
throws wreckage over its neighbours — 3.2 tiles, over 0.7 seconds. A hit stays
inside its own tile, and that bound is not decoration: a sweep lands a hit on
every structure it damages, so debris that crossed tile edges would turn a
raid on a large base into a solid sheet rather than a series of impacts. With
reach spoken for, a hit's weight comes from its count and its lifetime.

Each spark's trail is tied to how fast it is actually travelling, so a burst
opens with long streaks and settles into short ones. A fixed length reads as a
ring of dashes sliding outward rather than as something thrown.

Nothing about this is engine state. The sparks are derived from the flash
records the renderer already kept, so they share a flash's colour, lifetime
and retirement for free, and the scatter is a hash of tile and spark index
with no time term — which is what structurally prevents a burst re-rolling its
own shape every frame.

### A dev console, behind `FERAL_DEV_CONSOLE`

Provoking a sweep in play means waiting on a 1.2%-per-tick roll, and wearing a
single structure down to nothing takes hundreds of them. That cost is why
visual work ships unplayed, so there is now a keypad for it: `` ` `` on the map
opens five rows — force a sweep, damage the nearest structure, destroy it,
spawn a wild encounter, or burn 25 cycles.

Gated by an environment variable, like `FERAL_DEV_ARENA` and
`FERAL_DEV_REVEAL` before it, and unreachable in a build a player runs.

Every row calls the same code the game calls. `raid_check` and
`maybe_spawn_wild_creature` were split into a roll and a body, and the console
fires the body — so what it puts on screen is evidence about the game rather
than about the console. The damage row takes a percentage of maximum
Durability and is held one short of lethal, because the point of it is to be
pressed repeatedly at the thing you are watching.

### The base slab's corners, on saves that predate them

`0.5.3` chamfered the base platform, and a save written before it kept its
square slab — the cut is applied when the floor is stamped, and loading
restores a zone map verbatim. No migration is needed and none was added:
breaching re-stamps the slab at the new spawn point through the same
predicate, so a legacy base is repaired by the next zone. That is now pinned
by a test, since it is the reason not to write the migration.

## 0.5.3

### The base slab has its corners cut

The platform stamped around a Home was a flat 15x15 square, and it read as
one — a box the game had put down, rather than something the player had
built. Its four corners are now chamfered: each loses the corner tile and
the two beside it.

- The cut is **footprint, not paint**. `Platform::covers` is the one
  statement of the shape, and the build check measures against the same
  predicate — so a tile with no floor under it has nothing standing on it
  either, and a machine cannot hang off a rounded corner onto wild ground.
  A build refused there reads "Too far from Home".
- It costs **12 of 225 buildable tiles**, and the depth is one constant.
  `tuning::PLATFORM_CORNER_CUT` is the chamfer in diagonal steps; `0`
  restores the square exactly.
- Taking the slab up still sweeps the whole build box rather than the cut
  shape, which is the one place the two deliberately disagree. Nothing else
  overrides terrain near a base, so clearing a tile that was never stamped
  costs nothing — while a save written before today still has floor at its
  corners and would otherwise keep it forever once the Home came down.
- The renderer needed no change at all. The slab is drawn from
  `Biome::Platform`, so a cut corner draws as the terrain it reverted to.

An existing save keeps its square slab until the next breach re-stamps one,
since the shape lives in the saved tile overrides rather than in the seed.

## 0.5.2

### Watch a program carry a load to the depot

Hauling has been simulated since depots landed and was never once visible.
A posted program walked to its machine, took a clogged buffer's output to
the nearest depot with room, put it down and walked back — every step of it
pathfound, saved, and drawn nowhere, because the map filtered every tamed
program off the screen. The base was a still life with numbers changing in
it.

- A posted worker is now **drawn while it is out on an errand**, and only
  then. At its post it sits under its machine's own glyph, so a base at rest
  still reads as buildings and motion is the one thing that draws the eye —
  a program appearing *is* the news that it has left to deliver.
- The bobbing mark goes **with it**. One sentence decides where the mark
  lives: on the program when the program is drawn, and on the structure when
  it isn't. So a machine wears it at rest exactly as before, the worker
  takes it along on the trip, and a guard — which is never drawn, since
  nothing walks one to its post — leaves it on the structure for good.
  Exactly one mark per posted program at every instant.
- A machine that is **full with nowhere to send its output** is a dead end:
  no errand starts unless a depot has room, so the worker never leaves and
  the buffer never drains. Its mark stops bobbing, turns orange and blinks
  slowly. Keyed on a depot having *room* rather than on one existing — a
  depot that has filled up is no better than none.
- Nothing tamed but a working program is ever drawn. Nothing walks a guard,
  an idle program or a party member, so each keeps whatever tile it was
  standing on when it took the job, and drawing it would put a glyph
  somewhere the program isn't.

No simulation changed. `at_station` and the `collect::ORTHOGONAL` list it
shares with the player's own collect are untouched — a worker still stands
beside its machine, it simply isn't drawn there. With no depot built there
is still no errand, so on a depot-less base nothing is ever drawn at all.

## 0.5.1

### Rolled encounters: fight what the zone would actually throw

The arena could only ever answer half the tuning question. `opponents`
names a composition — "what if zone 1 threw nine at me" — which is a fair
question and one the game itself would never ask. The other half is what the
game *does* throw, and finding that out meant playing to the fight.

- A scenario may now name a **context** instead of a composition:
  `encounter: Some(Field(biome: OpenGrid))` or
  `encounter: Some(Stack(biome: Mainframe, depth: 5))`. Staging then runs
  the game's own spawn machinery for that context and fights whatever comes
  out. Mutually exclusive with `opponents` — one scenario asks one question.
- The zone comes from the player row rather than the encounter, because
  `ZoneLevel` is one resource driving both gear and enemy scaling and a
  second zone would be two answers to one question. The biome is on the
  encounter, because it alone decides the species pool.
- A `Stack` encounter **descends for real**, through the same
  `Game::enter_frame` play uses, so the depth multiplier, the group curve
  and Trace all apply. Depth is not a stat multiplier bolted on — depth 5
  fields four groups where depth 1 fields one.
- `reps` gains a meaning it did not have: a rolled encounter rolls its own
  pack, so fifty reps **sample the distribution** a context fields rather
  than repeating one composition. Every rep therefore records the
  composition it fought — the bin prints it, the result screen draws it, and
  the report carries it.
- A rolled pack is capped by the zone's own ceilings, unlike an authored
  one, because it *is* the game's own fight. It warns about nothing:
  nothing was asked for past a ceiling, because nothing was asked for.
- On the arena screen, an `Encounter:` row cycles Authored / Field / Stack,
  with a biome picker and a depth. The biome list is built from the loaded
  roster — walkable, and lived in by something — so the picker cannot offer
  a biome the roll would refuse, and a mod adding the first StaticField
  resident gets it offered for free.

Three limits are stated in `dev-arenas/README.md` rather than left to be
found: zone 1's field roll is the opening ring and so cannot reach that
zone's ungentled roster, a field roll is one habitat spawn roll and so
fields one species group, and a Stack roll is an ambush and never a boss.

### Fixed

- Arena opponents were spawned before the rep's seed was installed, so every
  rep fielded the same potential rolls and only the battle varied with the
  seed. The seed now covers the composition and the fight together. A loss
  seed pinned from an older report no longer replays the same fight.

## 0.5.0

### The interactive arena: play the fight you were only measuring

`0.4.1` shipped a harness that runs a real fight offline and reports what it
cost. What it could not do was let anyone *watch* one. The party plays the
game's own All-Attack every round — deliberately, so the tester cannot
invent decisions the game never makes — but that means **no companion
Special ever fires**, and an arena number is a floor on the party's output
rather than a measurement of it. The second gap was authoring: a scenario is
a `.ron` file, so "what if it had one more of those, and I were wearing the
other weapon" meant editing text, saving, and re-running.

Both close here. `FERAL_DEV_ARENA=1` puts an **Arena** row on the main menu.
Behind it is the same `Scenario` the `arena` bin runs — not a parallel
builder type, so a knob added to the schema cannot exist in one tool and not
the other — edited row by row, and fought in the **whole battle interface**.
Specials fire, items are spent, targets are chosen, jacking out is on the
table, because a person is pressing the keys.

- **The builder.** Up/Down move, Left/Right adjust the number under the
  highlight, Enter opens a picker of species or items drawn from the asset
  directories, Backspace removes a row. The loadout rows disappear when the
  player source is not `Fresh`, because the engine treats an authored
  loadout beside a save as an error rather than ignoring it.
- **The result screen.** Won or lost, rounds, HP left, companions down, the
  seed, the staging warnings, and the scrollable round-by-round transcript.
  `[R]` refights the same seed and `[N]` steps to the next — the same
  `seed + n` the bin's reps walk, so a fight watched here replays there.
- **Round trips with the bin.** `[L]` loads a scenario from `dev-arenas/`
  and `[S]` writes one back, so a fight built by feel is measured fifty
  times without retyping, and a loss seed from a report is watched by hand.
- **Nothing is capped, and nothing is silent.** A composition past what the
  zone could really field is built as asked and warned about — on the status
  line as the fight opens, and again on the result screen. "What if zone 1
  threw nine at me" is the question the tool exists to answer.

Unset, the flag makes none of this reachable and none of it loaded. An arena
session also touches no disk at all: no save, no `profile.ron` — a rung
earned in a tester's fight would otherwise be paid out to every future new
game — and no `run_history.log`, so a lost fight against a Permadeath save
lands on the result screen rather than on Game Over.

Under it, the engine's staging and outcome-reading were split out of the
headless loop (`arena::stage`, `arena::Watch`), so the played fight and the
measured one are one code path and cannot disagree about the RNG stream or
about what a fight cost. Both shipped scenarios produce byte-identical
reports across that refactor.

No save-format change.

## 0.4.1

### The battle arena: run a real fight without playing to it

Tuning a fight used to mean one of two things — start the game and grind to
where the fight lives, or reach for `balance_sim`, which answers instantly
and answers a different question (no RNG, no initiative, no abilities, no
items, no status effects). The arena is the third option: the real fight, on
demand, repeatable, with the composition chosen rather than rolled.

- A scenario is a RON file in `dev-arenas/`. It names who is fighting — a
  fresh player with an authored loadout, a save, or a `dev-saves/` template —
  and who they are fighting, then how many seeded reps to run.
- `cargo run --bin arena -- dev-arenas/opening-fight.ron`. At `reps: 1` it
  prints the transcript round by round in the game's own wording; above 1 it
  prints win rate, mean and median rounds, mean HP left, companions downed,
  and **the seeds of the losses** — pin one and that fight replays alone.
  Either way it writes a structured report for working with later.
- Opponents are spawned for real, so the zone multiplier, the potential roll
  and wild routines all apply. The composition is honoured verbatim past
  what that zone could really field, because "what if zone 1 threw nine at
  me" is a legitimate question — with a warning naming the ask, the ceiling
  and the zone, never a silent cap.
- Three scenarios ship: the fight the game opens on, a geared zone-3 party
  against a full group, and a template player against a boss.
- Its blind spot is stated rather than hidden: the party plays the game's
  own All-Attack, which fires no companion Specials, so an arena number is a
  floor on the party's output. `dev-arenas/README.md` says so too.

Nothing the arena does is written back to a save, which is what lets it
point at a real one without risk. It is a measuring instrument, not an
assertion — `balance_sim` remains the balance regression gate.

### Fixed

- A test fixture could set a companion to a level play cannot reach, and
  gave it no stat growth for the levels it did set. `set_level` now awards XP
  through `progression::add_xp`, so the growth lands and `CREATURE_MAX_LEVEL`
  binds.

## 0.4.0

### Breaking: gear fuses per physical copy, not per item type

Existing saves will not load (`SAVE_FORMAT_VERSION` 24 -> 25). There is no
migration path — see that constant's docs.

- Fusing gear used to upgrade the item *type*, so every spare and every copy
  picked up afterwards equipped at the fused tier. It read as a display bug
  in the inventory screen and was an accurate report of the model. A fusion
  now consumes two copies at one tier and yields **one** stronger copy at the
  tier above; spares stay ordinary, the way a fused program already worked.
- The ladder's price in base copies rises accordingly: 2 for a T1, 4 for a
  T2, 8 for a T3. A T3 used to cost 6 and upgrade the whole stack.
- The inventory, trade and gear-swap screens list one row per `(item, tier)`,
  so `Arc Lance T1/3 x1` and `Arc Lance x3` are two rows. A worn copy still
  counts as one of the two a fusion needs and picks up the new tier live.
- A trader's buyback shelf remembers the tier it took, so buying back a
  mis-sold T3 returns a T3 rather than an ordinary copy. Unit prices are
  unchanged at every tier.
- Fused copies are not recipe or machine input, and never were: every recipe
  reads the ordinary cargo stack, which is now by definition the unfused one.

## 0.3.4

### Changed: the base floor is darker again

- `Biome::Platform` drops to roughly two-thirds of the `0.3.3` navy. Same
  colour, less of it — the floor sits further under the glyphs and
  machine-status outlines it is a backdrop for.

## 0.3.3

### Changed: the base floor is dark navy, not grey

- `Biome::Platform` goes back to the dark navy of `0.3.1`. The grey shipped in
  `0.3.2` was the right darkness but the wrong colour, which only became
  answerable once the floor had been seen on screen — `0.3.2` was cut against a
  build nobody had actually looked at.

## 0.3.2

### Changed: the base floor is dark grey

- `Biome::Platform`, the floor the player lays under a base, goes from deep
  navy to a neutral dark grey — the only achromatic terrain tint in the game.
  It is the only biome that covers whole screens, and the base is the one place
  a dozen glyphs and machine-status outlines have to read at once, so the floor
  now carries no hue to compete with them. The map's passability rule is
  unaffected: it turns on red-dominance, and grey is not red-dominant.
- The intermediate step is unrecorded above: this floor was bright cyan through
  `0.3.0` and was darkened to deep navy during `0.3.1` without a changelog
  entry. Noted here so the trail from cyan to grey is not missing a link.

## 0.3.1

### Added: two content surfaces for mods

- `assets/crash_logs/` — a new content directory, one `.ron` file per
  entry, with the lines pooled and ordered by id. See
  `assets/crash_logs/README.md`.
- `taunts` on species files — optional cosmetic lines a program says in a
  fight. `#[serde(default)]`, so every existing species file and every mod
  keeps parsing untouched. See `assets/species/README.md`.

No save-format change, so this is a patch: existing saves load unchanged.

## 0.3.0

### Breaking: save format 23 → 24

`CreatureSave` gained a field, so existing `.bin` saves stop loading.
Templates under `dev-saves/` are RON and were updated in place.

### Changed: the zone map is terrain now, not a wall of characters

Ground used to be a character per tile — `#` for Mainframe, `~` for the Data
Void, `^` for Black Ice — in seven different colours. It is drawn instead:
circuit traces, drifting specks, a lit slab under your base, shards over the
ice. Entities keep their glyphs, so the things that move read as things that
move rather than competing with the floor.

Colour now answers one question and pattern answers the other. Every biome
you can walk on is cool cyan-teal; the two you cannot — the Data Void and
Black Ice — are hot amber. They were previously just two more colours in a
palette of seven, which meant the map never actually told you where the
ground stopped. It does now: the walkable tile beside a hole lights the edge
it shares with it, so every impassable region is ringed like a shoreline.

### Added: your base has people in it now

A program posted to a machine used to be a name on a screen, frozen on the
tile you caught it on. It now stands at its machine, and when that machine's
output buffer fills it carries a load to the nearest Depot and walks back.
You can watch it happen.

Two consequences. A machine no longer stops dead when it fills — it sheds
five units at a time and keeps going, so how long your base runs unattended
is a question about where you put the Depot rather than a fixed number. And
a program takes a moment to reach its post when you assign it: it walks
there, and produces nothing until it arrives. A machine whose program is
away says so.

The Depot itself holds a hundred units and costs twelve Core Fragments. You
collect from it the way you collect from anything else — stand next to it.
Build a second one across a sprawled base and half your programs' walks get
shorter. Build none and nothing changes at all.

### Breaking: save format 22 → 23

`CreatureSave` gained a field, so existing `.bin` saves stop loading.
Templates under `dev-saves/` are RON and were updated in place.

### Added: a program can be more than a companion

There is something you can do with a tamed program besides fielding it or
putting it to work. It is not written down anywhere, and it is not going
to be — but a program used that way lends you its strength directly, and
sometimes acts on its own while you fight. Which program you pick decides
what that is worth.

It costs you the program's other roles while it lasts: it will not stand
in the battle line at the same time, and it takes the place of whatever
you were carrying. Both come back the moment you stop.

### Changed: the way to the next zone runs through the Stack

Portal Fragments now come from one place — the guardian in a Stack lair —
and the payout is `4..=8` per frame of depth, so the bottom of a stack is
worth the walk back up. Three faucets closed to make that true: the flat
35% roll every kill used to carry, the cache in a Stack wall, and the
surface boss. You breach by going down, or you don't breach.

Each of the three fights that survive now pays exactly one kind of thing:

- **A Stack lair boss pays progression** — fragments, and nothing else pays
  them.
- **A surface boss pays power** — high-end gear instead of fragments, drawn
  from a band of the item value ladder that climbs with the zone. A zone-1
  boss hands out standard and researched pieces; by zone 4 it is dropping
  the premium tier. The band is derived from the `value` and `equipment`
  fields items already have rather than a new one, so a modded item joins
  the pool by existing — see `assets/items/README.md`.
- **A nest pays roster** — Credits as a floor, and a coin flip on an
  orphaned program of the nest's own species, which joins you free. Clear
  the nest of the program you want. A full roster loses it and says so.

Nests no longer pay fragments at all, which is the same rule from the other
side: sustained surface work is worth doing, but it is not a route to the
next sector.

Saves load unchanged. Existing runs keep whatever fragments they had
banked.

### Breaking: field buffs can fire on a cadence — save format 21 → 22

`FieldBuff` abilities take an optional `interval` in their `.ron`: how many
turns pass between firings, defaulting to 1 (every turn, which is what they
all did before). It means something only to the three over-time kinds —
`Regen`, `Coolant`, `Trickle` — since the rest have no per-tick effect to
space out. See `assets/abilities/README.md`.

Repair Loop Single is the first ability to use it: **+2 Integrity every 4th
turn for 300 turns**, where it was +2 every turn for 100. Slightly more total
healing over three times the window, so it reads as a long convalescence
rather than a burst.

A running buff carries its own cadence, which is a new field on saved state
— every existing save stops loading. The checked-in `dev-saves/` templates
are field-named RON and survive.

### Added: entity menus show the icon of what they list

Every picker that lists a program or a structure — party, fuse, extract,
cronjob, guard, work, demolish, upgrade, symlink, routines, field cast,
manifest — now draws that entity's map glyph in its map colour beside the
row. The glyph's colour is independent of the row's, which on those screens
already means fusion tier, CRITICAL Integrity or idleness.

Those menus also arrive in a fixed order now: alphabetically, then by
position. They were being drawn in ECS iteration order, which is not stable,
so the same menu could list the same base differently between openings. The
party keeps slot order, since arranging the battle line is what that screen
is for.

### Added: deploying tells you what you're short of

A deploy refused for want of materials now says so in the base log, naming
every shortfall at once with what it needs and what you hold — the refusal
used to live only in the status line, which clears itself after four seconds.

The Deploy Direction screen also names the structure you're about to place,
with its description and its cost, instead of being an unlabelled compass.

### Changed: Attacker and Defender pay 3 a level

Both perks now grant +3 permanent Attack or Defense per level instead of +1,
for the same 2 Perk Points. They were the two flattest purchases in the
catalogue and, at 1 a level against a Perk Point earned per player level,
buying either read as doing nothing.

This moves them past the Payload Tuning and Siphon Protocol affinity perks
for the opening levels of a run — those multiply a magnitude that grows with
level, so they still overtake a flat +3 from around player level 3 and keep
widening. Their own rate is unchanged; what changed is that the flat perk is
now the better early buy rather than the worse one.

### Changed: Research Data is banked, not carried

Research Data is no longer something you pick up. A Research Node delivers
it straight to you the moment a cycle completes, so there is no walking over
to the node and no collect key; it has no inventory row; and no trader will
touch it. The only place it appears is the research screen, which is the
only place it does anything.

It was already half-abstract — it never counted against your cargo — and the
rest of it behaving like a crate of salvage was the part that never made
sense. Two things that were true and are no longer: it had a 200 ceiling,
which is gone entirely, and it could be sold for 1 Credit apiece, which made
a Research Node a slow money printer.

For mods, `bank_limit` on an item is replaced by `banked: true`. Everything
above follows from that one flag, so a mod's own banked currency behaves the
same way without asking for it — see `assets/items/README.md`. One gap worth
knowing: an `assembles` machine producing a banked item still fills its own
buffer and still has to be collected. Existing saves are unaffected.

### Changed: the staffed mark bobs

The green square saying "a program is posted here" now lifts a few pixels
and settles, about once a second, so a working base reads as busy rather
than as a field of static dots.

Each tile is offset in phase from its neighbour. In lockstep the whole base
blinks as one and looks like a screen artifact; out of step it looks like
separate workers. The phase is keyed to the world tile rather than the
screen, so a mark doesn't reshuffle its timing as the camera pans.

The lift is upward only. The mark's rest position is held off the tile's
bottom edge on purpose — `outline_open` draws a chained pair's shared wall
as an absence of line, and a mark flush into that corner reads as painting
one back in — so a down-swing would spend an inset that is load-bearing.
Effects-off holds it still, like every other ambient animation.

### Fixed: the Compile screen quoted a price Lean Compiler had already cut

A player who had bought Lean Compiler was still told an ICE Breaker needed
three Core Fragments while holding two — and the compile would have gone
through, because `Game::craft` charges the discounted price. The same screen
contradicted itself a line further down: "Max affordable right now" reads the
discounted path and said 1.

The discount used to be applied in `craft_cost`, which `craft` calls, while
the screen formatted the raw recipe out of `craft_recipes`. It is now applied
in `craft_recipes` itself, the one point every player-facing recipe passes
through, and `craft_cost` is a lookup into it. Quoting and charging can no
longer come from two places.

Nothing a structure consumes moved. A machine runs its product's authored
`craftable.cost` through `systems::assembly_recipe`, which reads `ItemDb`
directly, and the Recipes chains do the same — so the player's bench perk
stops at the player's bench, which
`lean_compiler_does_not_discount_what_a_structure_consumes` now holds.

### A structure with a program posted to it wears a green corner mark

"Someone is assigned here" used to be a yellow tile outline, and machines
took that outline over when they started wearing their state on it —
running, starved, clogged, idle. From then on a machine could never draw
the yellow at all, so the only surviving trace of an unstaffed one was the
grey of `Idle`: a colour meaning absence, on an axis already carrying three
other things. A base of machines gave no reading of which ones had a
program on them.

It is now a small green square in the tile's bottom-left corner, drawn for
every structure with a worker, machine or not. The outline says what the
machine is doing; the mark says whether anybody is on it. The mark is held
clear of the tile's edges on purpose — a chained pair's shared wall is
drawn as an *absence* of line, and a square flush into the corner would
read as painting one back in.

The two channels disagree in exactly one case, correctly: a guard counts as
assigned but runs no job, so a guarded machine draws grey and marked.

### Two routines that act on a Stack frame instead of reading it

Everything the player had for a frame either read the maze — the view cone,
the map, a breakpoint — or was inflicted by it: a fault, corrupted ground.
Nothing let them act on its shape. Two new field routines do, reached
through a new research node (**Address Translation**, 50 Research Data,
after Deep Analysis) and installed like any other: a Routine Disk and one of
your six slots.

| Routine | Costs | Does |
|---|---|---|
| Buffer Overrun Party | 12 Fatigue | Steps through exactly one wall ahead, landing on the open cell beyond |
| Wild Jump Party | 20 Fatigue | Moves the party to any cell of the frame you point at — and kills you if that cell is solid |

Buffer Overrun is deliberately one wall thick. Any deeper and a cast from
the frame edge cuts a diagonal across the whole maze; at one wall it opens
the room next door and nothing further. It is refused, spending nothing, if
the rock runs deeper than one cell or if the far side is off the frame.

Wild Jump is a `goto` to an address nobody validated. Aim it with a cursor
on the frame map: cells you have already walked are safe *because* you have
walked them, and the unlit part of the map is exactly the risk. Nothing
warns you beyond the map itself — that is the mechanic. Arrival hazards fire
on a jump exactly as on a step, so jump onto a fault and you fall through
it, and jump onto an uncleared lair and you have roused it.

Both refuse a landing behind an unopened seal, and refuse landing on the
seal itself — a refusal rather than a death, since the rule exists to keep
"earn your way to the guardian" intact rather than to punish a misclick.
Both are Stack-only and greyed with the reason on open grid, both raise
Trace on success, and neither ever appears in the battle Special menu or on
a wild program.

### Eight routines that fill the holes in the ability families

Charting the abilities made a gap visible that a directory listing never
could: the files are named for flavour, so nothing about `bus_fault` sitting
in `assets/abilities/` says it is the Pipeline Stall family reaching the
whole field with nothing between it and one target. Six families had holes
like that. Eight new routines close every one of them:

| Routine | Reaches | Does |
|---|---|---|
| Pipeline Stall Group | one group | 6 damage, 30% chance to lock up |
| Fork Bomb Everyone | the field | 8 damage, 20% chance to leave bleeding |
| Leech Everyone | the field | 4 damage, a fifth of it back to you |
| Etch Everyone | the field | −3 DEF for 3 rounds |
| Throttle Everyone | the field | −3 ATK for 3 rounds |
| Etch Single | one target | −5 DEF for 3 rounds |
| Throttle Single | one target | −5 ATK for 3 rounds |
| Flush Cache Single | one ally | clears its status condition |

Every one is hunt-only, like every routine already in its family — the way
to get them is to meet a wild program running one and decompile it rather
than kill it. That takes the hunt-only set from twenty to twenty-eight.

The magnitudes are not new judgement calls. The shipped set already charges
a consistent price per scope — Single costs 8–10 Fatigue on a 2–3 round
cooldown, Group 11–13 on 3–4, Everyone 15–18 on 5 — and rider chances
already decay as the scope widens, 60% at Fork Bomb Single down to 35% at
Group. Each new routine reads off that ladder, sits below its own family's
next tier down on magnitude and above it on cost. Packet Shred Everyone
stays the one routine that gets better per point as it gets wider, which is
the whole reason it is a boss routine no player is taught.

Two saps that only ever hit a group now run the full width — a −5 DEF strip
on one target for 8 Fatigue against −3 across the field for 16 — and Flush
Cache stops being a party-only routine, so clearing the one member who is
actually bleeding costs 4 rather than 7.

`every_battle_ability_family_is_contiguous_from_single_upward` is what found
the Pipeline Stall hole and what will find the next one: a family occupies a
run of scopes starting at Single, with no gaps. Field routines are excluded,
since a scope ladder is not what that half of the set is organised by.
`every_everyone_scope_routine_pays_the_everyone_tier_price` pins the ladder
the magnitudes were derived against — nothing gated it before.

### A charted stat sheet for every content directory

`docs/roster.md` now has six siblings, one per remaining moddable directory
under `assets/`: items, abilities, structures, research, achievements and
perks. Each follows the roster's shape — a transcribed table at the top of a
`docs/*-gen.py`, ASCII charts generated from it, and prose that says what the
numbers mean rather than restating them.

What the charts surface is mostly relationships the individual files cannot
show. The research page draws the tech tree from its own `requires` edges and
prices each node by its whole chain, which is how the six end-of-branch nodes
turn out to carry more in prerequisites than the dearest single node costs.
The structures page derives the four production lines from which item feeds
which. The abilities page reads the naming scheme back out of the display
names — an id is flavour, a name is a spec — and finds twelve families, none
of them with a hole in it. The achievements page shows the ladder spending
its Perk Point ceiling exactly, 5 of 5, so a fourteenth rung paying points
has nowhere to go.

Two of those pages document a fact this work had to check rather than assume:
only two of the four end benches are built out of their own feeder's product,
not four, and `each_bench_is_built_out_of_what_its_own_feeder_makes` asserts
exactly that pair.

`assets/fonts/` and `assets/sounds/` are skipped — binaries with nothing to
chart.

### Eight pieces of content renamed out of the occult register

The Grid runs on malware and systems vocabulary, and eight things had drifted
away from it. Three species — **Ghost is now ZeroDay** (map glyph `z`),
**Wraith is now Crawler** (`r`), **Phantom is now Proxy**, which keeps its
`p` and whose Backdoor and Spoof moves already read that way. Three items:
**Daemon Fang → Shim Blade**, **Probe Daemon → Probe Service**, **Wraithsteel
Plate → Nullsteel Plate**. One routine: **Ghost Protocol Party → Stealth
Protocol Party**. One achievement: **"Ghost in the Wire" → "Something in the
Wire"**. ZeroDay's Haunt attack is now Fray, and its Static Wail is Static
Burst.

This renames ids and filenames, not just display names, following the
precedent set when the Daemon species became SubProcess. **Existing saves
lose these species, items and the routine on load** — their ids no longer
resolve. The `dev-saves/` templates were updated in place and still work.

One consequence worth recording, because it will recur: species are loaded in
sorted-id order, so renaming three of them moved every later draw from the
shared `GameRng` stream and changed the outcome of nine seeded tests. All
nine turned out to be latent fragility rather than the rename breaking
anything — a Stack ambush that now fires one step earlier and blocks a
fixture's walk, an opening-ring census that swept nest guardians it was
documented not to cover, a taming test counting refused attempts as charged
ones, and a nest test whose walkable strip was narrower than the tether
square a replacement guardian scatters across. Each was fixed at the cause,
so the next content change does not move them again.

### Fusion reads at a glance, and gear stops at three like everything else

Anything you have fused now draws in its own colour wherever a menu lists
it: cyan while it can still go into another fusion, magenta once it is at
3/3 and is a finished product. That covers the party screen, both fuse
pickers, the extract and cronjob pickers, the inventory and its equip
picker, and both of a trader's sell lists. The battle roster and the `B`
roster are deliberately left alone — their colours already mean HP state and
species.

Gear fusion is now capped at three, the same ceiling a program's lineage has
always had. It was previously unbounded, which made a stack of one item type
into an open-ended stat multiplier. Item rows name the ceiling the way a
program's do (`fusion T2/3`, `fusion T3/3 - maxed`), and a fusion refused for
being at the cap spends nothing.

A save that already holds an item above tier 3 has that ledger entry lowered
to 3 on load. A copy you are *wearing* keeps the bonus it was equipped with
until you take it off — its stats are already banked, and quietly restating
them would have left the difference welded into your base stats.

### Difficulty comes from the zone, not from how far you walked

Wild programs no longer get stronger the further you are from your base.
Distance from the danger origin used to scale their stats by up to 3x and
double group size every 15 tiles, on top of the zone's own doubling. Both
are gone. A zone now has one consistent difficulty, and the escalation you
feel comes from the commitments you make — funding a Portal, descending a
link — rather than from which direction you wandered.

This also fixes a leak into the Stack that was never intended. Every
underground spawn is placed at the surface tile you descended through, so
a link out at the edge of the sector multiplied *every fight in that whole
stack* by its own distance. Two stacks at the same depth could differ
threefold on nothing but where their entrances happened to sit.

Group size and group count still ride one shared curve, so they cannot
disagree about how dangerous a place is; it just reads the zone on the
surface and the frame depth in the Stack.

**The zone-1 opening ring survives**, now as an explicit radius —
`OPENING_RING_TILES`, the build radius — around your base rather than a
band derived from the old distance curves. It travels with your base and
still fields only programs a fresh player can actually beat.

### The field hits harder and bosses hit less hard

With distance gone, a zone's baseline is all it has, so the field roster's
base stats are up 25%. The four species the opening ring draws from — Scan
Drone, Glitch, Sprite and Sub-Process — deliberately sit it out: they are
the only four a bare level-1 player beats solo, and raising them would
quietly empty the ring.

Bosses move the other way. At Stack depth 2 an Overseer was 2.6x an
ordinary program's HP and 3.1x its attack, which made the gap between a
depth-2 fight and a depth-2 lair far larger than the depth between them.
Overseer and Wintermute now sit near 1.5x the toughest ordinary program,
and past zone 1 a boss arrives with an escort group of an ordinary species
from its own habitat. A boss is a harder *fight* now rather than a harder
single opponent. Zone 1 holds one group, so the opening zone's boss is
still met alone.

The grind-only level curve moves from 1/15/33/61/117 to 1/19/40/77/149
across zones 1-5. The shape is unchanged at roughly 2x per zone; the whole
curve sits higher.

### Upgrade tiers are gated by the zone you have breached to

A structure is deployed at Mk1 wherever you build it, and the zone you have
reached is what unlocks the tiers above: zone 2 frees Mk2, zone 3 frees Mk3,
up to the Mk5 every shipped producer caps at. Nothing upgrades at all in zone
1. Tiers already paid for are unaffected and ride through portals with the
rest of the base.

Previously a Mining Node could be walked to Mk5 without ever breaching, which
made the deepest producer in the game reachable from the opening zone. It now
follows the rule gear already followed — reaching zone *N* is what unlocks
level *N* gear — and the two ladders line up, both spanning 1..=5.

A structure sitting at its zone ceiling is still listed in the upgrade
picker rather than filtered out, and the row says which zone would free the
next tier. Hiding it would have taken the whole **Upgrade a structure** row
out of the base menu for all of zone 1, and a player who had never breached
would never have learned that upgrading exists.

This shifts the economy: Core Fragments in zone 1 now go only to building and
recipes, with the upgrade sink opening on the first breach. Nothing gates
that number — `balance_sim` models a run's battle curve, not its build
economy — so it is unmeasured, and unplayed.

### Raids are now GC Entropy Sweeps

A rename, player-facing text only. The Grid's periodic garbage collection
passing over anything you have left standing reads as what it is, rather than
as bandits. `{structure} takes N raid damage` also became `{structure} loses
N Durability to a GC Entropy Sweep` — the event is a noun phrase and does not
survive being pushed into an adjective slot.

No mechanical change. `MessageKind::Raid`, the `raid_defense` and `raidable`
structure fields and the tuning constants keep their names, so saves load and
existing mods parse untouched.

### The inspector reads structures, not just programs

`x` and a direction found the nearest *creature* that way and opened its
manifest; a Refinery standing right in front of you was invisible to it. It
now finds the nearest creature **or structure** and opens the right sheet for
whichever it is.

Nearest wins, with no priority by kind — both are gathered in one walk, which
is what makes that answerable at all. A creature exactly as far away as a
structure keeps the target, since it is the one that might have wandered off
by the time you look again.

The structure sheet is the `B` roster's row for that one machine — tier,
position, durability, who is posted to it, whether it is idle or starved, and
what is in both buffers. It is built by calling the roster's own line
builders, so the two screens cannot drift into disagreeing about the same
machine. Any key closes it.

**Structures are not offered while you are in the Stack.** Your `Position`
stays pinned to the surface entrance tile down there, so an unguarded scan
would report your base four frames overhead as lying off to your east.
Creatures are unaffected — that is already how the inspector behaves
underground.

### Production lines are lines again

A bench took two intermediates off two separate feeder legs, so anything the
factory made needed **two chains stood up and a corner to put the bench in**
before a single unit came out. Standing up a Fabricator meant a Log Scraper,
a Transcriber, a Power Conduit *and* a Winding Node — five machines, five
programs against a starting roster cap of three — and getting the geometry
right on top. That is a puzzle, not a decision.

Every shipped machine recipe is now **one ingredient**, and the four
intermediates match the four benches one-to-one:

```
Mining Node   -> Refinery     -> Armory        (Hardened Shell)
Mining Node   -> Lathe        -> Disk Press    (Routine Disk)
Log Scraper   -> Transcriber  -> Fabricator    (Trace Sniffer)
Power Conduit -> Winding Node -> Assembly Bay  (Patch Routine)
```

Three machines, three programs, a straight run of three tiles — so one line
is exactly what a fresh roster can staff, and which line you build first is
which thing you get first. The recipes:

| item | was | now |
|---|---|---|
| Hardened Shell | 2 Blocks + 2 Coils | 3 Bytecode Blocks |
| Routine Disk | Substrate + Wafer | 2 Blank Substrate |
| Trace Sniffer | 3 Wafers + 2 Coils | 5 Logic Wafers |
| Patch Routine | Block + Coil | 3 Charge Coils |

The Patch Routine swapping Blocks for Coils is the one that isn't just
arithmetic: without it the Winding Node loses every automated consumer and
becomes dead weight. With it, no intermediate is orphaned and all four raw
taps stay in play.

**Build costs follow the same rule.** The Assembly Bay cost Bytecode Blocks
while *running* on Charge Coils, so standing one up needed two unrelated
lines — exactly the tangle this removes. A bench is now bought with its own
feeder's product: the Assembly Bay costs Charge Coils, the Disk Press costs
Blank Substrate.

Researched armour and module recipes are untouched and still name several
ingredients — those are hand-crafts at a bench, with no adjacency and no
geometry, so they were never the un-fun part. The engine's multi-input
support is untouched too, and mods may still ship two-ingredient assemblers.

### The Recipes screen says which structure to build for each ingredient

A chain used to start at its raw materials with no hint where they came
from: a Routine Disk began at four Core Fragments and four Raw Trace, and
knowing that meant a Mining Node and a Log Scraper was something you brought
to the screen rather than something you got from it.

An ingredient no recipe makes now leads with the structure that taps it, so
a chain read top to bottom is the build order:

```
Product: Routine Disk
  Mining Node (Core Fragment x4)      -> Lathe       -> Blank Substrate x1
  Log Scraper (Raw Trace x4)          -> Transcriber -> Logic Wafer x1
  Blank Substrate x1 + Logic Wafer x1 -> Disk Press  -> Routine Disk x1
```

An ingredient an earlier step of the same chain produces is named on its
own — a Power Cell reads bare rather than naming the Power Conduit, because
the bench step that makes those ones is already a line above it, and a chain
claiming two sources for one item is worse than either. Ingredients nothing
produces, like scavenged Portal Fragments, likewise stand alone.

Products now carry the units one batch yields, which is what marks the
column as an item rather than another structure. Extractors quote none:
a node's payout scales with its upgrade tier and the zone, so there is no
one number to print.

### Dying in the Stack on Forgiving actually gets you out of it

A Forgiving reboot puts you at your nearest construction. Underground, it was
writing that construction's tile into your position while leaving you in the
maze — so you rebooted at home on paper and came to still four frames down,
with the entrance tile your position had been pinned to overwritten in the
process. The only way home was to walk the frames back up.

A reboot now surfaces the party first, then warps, and says so. With no base
standing you still surface, back onto the link you walked in through.

### Stuns and bleeds now last as long as they say

A status condition was being charged for the round it was applied in. End-of-
round upkeep ran on every round including that one, so the first tick of every
duration was spent within moments of the condition landing.

Stun was the casualty. Every stun in the game is `duration: 1` — the shipped
species moves, Deadlock, Pipeline Stall — so a stun was armed mid-round and
gone by the end of it. It cost the victim a turn only when the attacker also
happened to win initiative that same round; land it on something that had
already acted and it did nothing at all before shaking off. Hard Lock's
advertised two rounds gave one. Bleed had the quieter half of the same bug:
Memory Leak's "Bleed 2 per round for 3 rounds" dealt its first tick instantly
and showed two rounds on the roster.

Conditions now skip the upkeep for the round they land in, so `duration`
counts the rounds after it. A `duration: 1` stun reliably costs its victim the
next round's action; a three-round bleed ticks three times over the next three
rounds. Landing a stun on something that has not yet acted still stalls it
immediately, on top of the round it is owed — outrunning your target is
supposed to be worth something.

Both sides feel this. Wild carriers and stun-carrying species moves have been
getting the same nothing out of their stuns, and now they don't.

### The Compiler runs on Core Fragments

It used to print ICE Breakers out of nothing every 8 ticks. It now compiles
them out of Core Fragments pulled from whatever is touching it — a Mining
Node, in practice — the same way the Refinery and the Assembly Bay work. Its
recipe is the ICE Breaker's own three-Fragment recipe, so the bench and the
machine cannot drift apart; hand-crafting one is unchanged and still needs no
bench at all.

Two consequences worth knowing before you rebuild your base around it. A
Compiler needs a program posted to it *and* a feeder orthogonally adjacent,
so it can no longer be worked by hand. And a Compiler and a Refinery both
touching one Mining Node compete for its output — the lower-left machine
takes its share first — so a shared feeder now needs enough throughput for
both, or a node apiece.

At three Fragments per unit against a Mk1 node's one per ten ticks, the
Compiler is a fragment sink you feed from a stockpile rather than a line that
idles at full rate. Its `upgrade` block is retained but buys nothing: an
assembler's rate is fixed by its definition, and tier does not touch it.

### A Data Cache widens the roster by five

Two slots against a base of three was not enough to matter. A full Stack
descent offers four or five orphaned programs, and a roster of five refused
most of them; the five-machine production line wanted every slot a run had.
One cache — still ten Core Fragments — now takes you from three programs to
eight, which covers a whole descent or a whole factory floor and leaves a
party to adventure with.

### Lower-case map keys

Collecting from adjacent structures is now `c` rather than `C`, and the log
filter is `f` rather than `F`. Nothing else on the map asked for a modifier,
and the two that did were the two you press most often while walking. `L` for
the message history keeps its shift: `l` walks east.

### Swap gear from the slot it's in

Picking one of the three equipped rows on the inventory screen (`i`) used to
unequip on the spot, so changing a weapon meant unequipping it, hunting the
replacement down a cargo list of any length, and equipping that. It now opens
a replacement list for that slot: every piece of gear you carry that fits it,
best first, with the row that empties the slot last. Each row shows the bonus
you would get and, beside it, the change from what you are wearing.

A candidate is previewed at the level it *would* equip at, while the worn item
is measured at the level it remembers. That gap is the point — gear doubles per
zone level and locks in when you put it on, so a spare copy of the weapon
already on your back is a genuine upgrade after a breach, and the list now says
so out loud instead of leaving you to work it out.

No engine change: `Game::equip` already returned the outgoing item to cargo
itself, so a swap is one call rather than an unequip that could leave you
bare-handed if the equip behind it were refused — see
[Items and equipment](README.md#items-and-equipment).

### Achievements that outlive the run

Thirteen rungs across four axes — how deep a sector you breach into, how far
down the Stack you get, how many cycles you survive, and which boss programs
you put down. Earning one stamps `profile.ron` at the repo root immediately,
not at run end: a permadeath run that ends badly still keeps what it proved.

The reward is paid at the **start of the next run** — a point into one of
Attack, Defense, Integrity or Decompiler, a Perk Point, or a free program
already yours and waiting to be deployed. Loading a save pays nothing, since
that save's stats already hold what the profile bought when the run began.

`A` on the main menu lists every rung, earned or not, with what it pays and —
where earned — the cycle, the mode and which stat the roll landed on.

Achievements are data: `assets/achievements/*.ron`, one file per rung, so
adding one is a file drop. What is *not* data is the ceiling — at most 8 stat
points, 5 Perk Points and 1 starting program across the whole ladder, asserted
over the real asset files. `balance_sim` simulates one run's curve and cannot
see a cross-run profile, so that assertion is the only gate on how much the
profile is worth. The shipped ladder spends 7 / 5 / 1.

**No save is affected.** The profile is deliberately not part of `SaveData` —
a save is one run and this spans them — so `SAVE_FORMAT_VERSION` stays at 21,
and it will stay there for any field added to the profile later, since RON is
self-describing and every field is `#[serde(default)]`.

The three cycle thresholds (500 / 2000 / 5000) are arithmetic guesses. Nothing
in the repo says how long a run actually lasts and no test can; they want
checking against a real run.

### Every ability name says what it hits

Forty-one abilities had forty-one unrelated names, so nothing about
"Broadcast Storm" told you it hit the whole field while "Cascade Overflow"
hit one group. The picker shows the name before it shows anything else, and
that was the one thing it couldn't tell you.

Names are now `<Family> <Scope>`. The scope is **Single**, **Group** or
**Everyone** against enemies, and **Single** or **Party** on your own side.
The family names an effect rather than a file: `Packet Shred` is plain
damage, `Fork Bomb` is damage carrying a Bleed, `Pipeline Stall` carries a
Stun, `Patch` heals, `Hard Lock` stuns. So the group attack that bleeds is
Fork Bomb Group and the one that doesn't is Packet Shred Group v2.0.

Where two abilities in one family share a scope and differ only in size,
a version tag separates them — Patch Single v1.0, v2.0 and v3.0 restore 8,
25 and 50 Integrity. A major bump is a real step up; a minor one is the
same thing slightly bigger.

Field routines keep their own names and take the suffix, so Deep Scan is
Deep Scan Party. Creature *moves* are untouched — they have no targeting
scope for a suffix to describe, so the Overseer still opens with Kernel
Panic even though the ability of that name is now Packet Shred Single.

**No save is affected.** Ability ids, filenames and the `routine_<id>`
items minted from them are all unchanged; only the display string moved.
A mod that named its abilities the old way keeps working — the loader has
no opinion, and the two new tests only cover the shipped set.

### Armour and modules are made out of the factory

Standard and premium gear was crafted at a bench out of raw Core or Portal
Fragments, which meant the production chains and the gear economy never
touched. Thirteen recipes — six armour, seven modules — now spend what your
base refines instead. Weapons are deliberately untouched, and the Scavenged
tier still compiles from raw fragments with no base standing, because that
is what a fresh run, or one just raided flat, equips out of.

The ingredient follows the stat, so a recipe reads as what the piece does:
Logic Wafers buy Decompiler, Bytecode Blocks buy Attack and bulk, Charge
Coils buy Defense. Armour draws off the Mining Node and modules off the Log
Scraper, while both want Coils — so the Winding Node is now the first feeder
with three machines pulling on it, and where you put things starts to matter.

Premium gear trades roughly a fifth of its Portal Fragment price for refined
goods, so your base does some of the work boss drops used to.

The **Armory** and **Fabricator** each assemble one item on repeat — Hardened
Shells and Trace Sniffers — while staying the hand-craft bench for every
other recipe that names them. They take a program and want both feeders
touching them like any machine, which also moves them into the Assembler
group in the build menu.

Item values are unchanged, so the price ladder does not move and no balance
curve shifts. Saves are unaffected. An Armory or Fabricator standing in an
existing base will report itself *idle* once until you post a program to it.

### A routine is knowledge plus a disk you have to build

**Save format bumped to 21 — existing saves will not load.**

A routine was an item, minted per ability at startup and dropped free into
cargo by research. It is now two things: knowing it (permanent, from a
research node or from breaking a program down at a Compiler) and a blank
**Routine Disk** the base manufactures. Installing burns a disk; popping the
routine back out returns nothing.

Four new machines make the disks — Log Scraper → Transcriber and Mining Node
→ Lathe, both feeding a Disk Press — behind a new **Routine Fabrication**
node that **Self-Execution** now requires, so no node can teach a routine
you have no way to install. Species kits still install free at spawn, and a
new game already knows Decompile.

### Items are worth what they are worth

Every item sold for exactly 1 Credit. A trader's `sell_rate` *was* the
price, applied to whatever crossed the counter, so a Singularity Matrix — 24
Portal Fragments and the rarest thing in the game — fetched the same single
Credit as a Shiv Routine.

Items now carry a `value` in their `.ron` file, and `sell_rate` becomes the
trader's multiplier on it: what a thing is worth is a property of the thing,
what a trader pays is a property of the trader. Printable goods sit at 1,
scavenged gear at 3–8, standard gear at 12–16, the drop-only researched
pieces at 20–60, and premium gear at 80–120.

The ladder is bounded by two rules, both now tested against the shipped
assets, because breaking either turns a base into a Credit press: nothing is
worth more than the ingredients of its own recipe, and nothing a structure
produces on a timer is worth more than the floor. The second is the one that
isn't obvious — a Compiler makes ICE Breakers out of nothing every 8 ticks,
so pricing them at their recipe cost would have out-earned a Mining Node
nearly fourfold. Value comes from what a base *can't* manufacture.

Modded items without a `value` keep trading at 1, exactly as before.

### Power Cells stop piling up

Cells arrived from three directions: the Glitch and the Scrapper both
dropped them 2–4 at a time, and a Stack cache rolled for one at 0.4 — the
highest chance of any item, more than double the next non-catalyst. The
Scrapper now drops Core Fragments, leaving the Glitch as the only source
that walks around, and the cache chance falls to 0.15.

A cache's expected haul goes from 1.55 items to 1.30 — still the "a little
over one" the modding docs promise, and now a test rather than a promise.

The manual's roster table said the Sprite dropped nothing and the SubProcess
dropped Power Cells; both actually drop Core Fragments, and had done for
some time. Its column header also called that field "Works for", which is
not what it does — a tamed program's output comes from the structure it is
posted to, whichever program you assign.

### The map screen's keys are grouped behind `b`, `p` and `i`

**Breaking for muscle memory.** The map bound 27 keys. Seventeen of them now
sit behind three: `b` opens a base menu (deploy, compile, cronjob, work it
yourself, guard, upgrade, demolish, structure roster, research), `p` opens a
party menu (companions, manifests, fuse, install and extract routines,
perks), and `i` opens the pack directly — it is one screen, so it needs no
menu of its own.

Thirteen keys are retired outright and do nothing: `c` `w` `W` `G` `R` `U`
`B` `T` `f` `m` `M` `d` `v`. Keeping them as aliases would have meant the
flat surface never actually shrank, and the help screen would have had two
systems to document. Inspect-a-direction moves from `i` to `x` — perks
moving into the party menu freed the key a player would guess for it anyway.

Collect (`C`), trade (`t`), cast a field routine (`a`) and symlink (`u`)
stay flat despite being on-topic: they are pressed every few turns while
walking, and a menu is a keystroke tax on anything that frequent.

A menu lists only what you can do from where you stand. A row is shown when
its screen would have at least one row of its own — so with nothing
deployed there is no *Demolish*, and with one companion there is no *Fuse* —
and rows whose action needs open ground disappear while you are in the
Stack, where your `Position` is still pinned to the entrance tile above.

Esc from a screen a menu opened returns to that menu rather than to the map;
finishing a job still drops you back to the map.

### The base menu has a *Recipes* row

A read-only list of every conversion your base can run, each walked back to
the raw inputs it bottoms out in. A Patch Routine reads as the four steps it
actually takes — fragments into a Refinery, cells into a Winding Node, then
both into the Assembly Bay — rather than as two ingredients you then have to
go look up one at a time.

The list is built from the loaded assets rather than written down anywhere,
so a modded machine appears in it for free, and the recipe shown is the same
one the machine stages. A step with no structure beside it is one you
compile by hand.

Reference data rather than a view of your base, so unlike the rest of the
base menu it stays available in the Stack.

### The window opens fullscreen, and the UI text is smaller

Was a 1440x900 window. It now opens borderless fullscreen at whatever the
monitor is, and body text drops from 24px to 18px at the reference height —
so the extra room goes to map tiles and log rows rather than to a magnified
UI. The text change applies at every window size, not just large ones.

Nothing else moved: every size on screen is either a fraction of the window
or derived from the font, and the zone map has always shown *more tiles*
rather than bigger ones as the window grows. Zoom (`+`/`-`) remains the only
thing that changes tile size.

There is no key to leave fullscreen, and so no windowed size is carried
alongside it.

### Adjacency-fed production chains

**Save format 19 → 20.** Existing saves stop loading; the checked-in
`dev-saves/` templates are RON and survive.

Machines now have local storage and feed each other by *touching*. A
structure declaring `assembles` pulls ingredients out of the output buffers
of the four structures orthogonally adjacent to it — never diagonally — and
builds one unit at a time. A chain is a physical line across the base, a
machine with two ingredients needs both feeders beside it, and a machine
nobody visits fills up and stops.

A machine's recipe is the *item's* own `craftable.cost`. There is no second
recipe format, so a bench recipe and a machine recipe cannot drift apart, and
any craftable item a mod adds — including a multi-ingredient one — is
automatable for free.

**Extractors deposit into their own output buffer, not your inventory.** This
is the largest felt change: fragments stop appearing in your pocket while you
are away, and you come home and harvest with `C`, which empties every
structure orthogonally touching you. Structures block movement, so you always
stand beside one rather than on it — standing in the crook of an L empties
three buildings, a sprawled-out line costs you trips. It is also the only
thing that makes clogging real; a node paying straight into the player is an
infinite source and nothing upstream of it can ever back up.

Three stall states, each said once on the way in rather than every tick:
*starved* (input short), *clogged* (output full) and *idle* (no program). `B`
shows every buffer and every stall.

Every machine needs an assigned program, assemblers included, so **roster
capacity is what buys chain length, not fragments**. The five-machine line
needs five programs against a starting cap of three, which makes the Data
Cache the population building.

New content: Bytecode Block, Charge Coil and Patch Routine — the game's first
multi-ingredient recipe, and the first shipped item to arm a pre-battle buff
— produced by a new Refinery, Winding Node and Assembly Bay. The Assembly Bay
costs Bytecode Blocks to build, so the two-machine line a starting roster can
afford is what pays for the third stage.

Removed: `passive_process` (zero shipped users, superseded entirely by
`assembles`) and the node deposit pool — `ResourceNode::amount`/`capacity`,
`WorkDef::capacity` and `StructureSave::resource_amount`. A node is a tap
rather than a reserve now; the output buffer is what paces it.

New `dev-saves/chains.ron` stands the whole chain staffed and running, with
the player parked beside its Assembly Bay.

### The log pane filters by base or field

`F` on the map cycles the log pane through All → Field → Base → All. The base
talks constantly — cronjob payouts, failed extractions, construction, raid
damage — and in the six rows the pane has room for, that steadily pushed
combat and Stack lines off the top before they could be read.

Every log line now carries a `MessageSource` alongside its `MessageKind`. The
two are separate axes on purpose: kind is read by the colour table, by
`retain_outcomes_since_battle`'s prune, and by `condense`'s notion of line
identity, and a raid alert has to stay `MessageKind::Raid` for all three while
still being base news. `Field` is the default, so only the ~24 base-side log
calls moved. Power reserves stay field — a need follows you into the Stack.

The pane header names the active filter and counts what it is holding back, so
a raid landing while you are watching the other channel still announces
itself. The history screen (`L`) is deliberately unfiltered: it is the
complete record, and app-core's row count for it stays the one the renderer
draws.

No save-format bump — the message log was never persisted.

### Nests fight back, and destroying one pays

Bumps `SAVE_FORMAT_VERSION` 18 → 19 — existing saves need a new game.
`CreatureSave` gains `nest_position` and `pursuing`, and a new `NestSave`
records each nest's position, Durability and pending respawns.

- **Attacking a nest now provokes every one of its guardians.** They abandon
  their tether and path toward you around obstacles — a bounded cost field
  built once per tick from the player's tile (the `pathfinding` crate's first
  use in this engine) — then fold in whatever else is standing nearby the
  moment one makes contact, into an ordinary battle. A besieged nest keeps
  this going: a guardian that respawns while its nest already has pursuers
  joins the chase the moment it appears, so grinding one down without
  finishing it off no longer buys a permanent lull on its own — reaching
  your own base or successfully jacking out of the fight still does (see
  below on both).
- **A pursuer gives up 15 tiles (Chebyshev) from its own nest, more than 20
  from you, or once it simply has no route to you at all** — whichever it
  hits first — and walks itself home. **Reaching your base ends the chase
  outright, for the whole swarm at once**: no guardian will set foot on the
  platform, and standing inside it leaves every pursuer in the zone with no
  route to you, not just whichever one was closest, so going home disbands
  the chase rather than merely holding it at the door. New tuning:
  `NEST_AGGRO_LEASH_RADIUS`, `NEST_PURSUIT_STEPS_PER_TICK` (1, exactly player
  speed — outrunnable in a straight line, never shakeable),
  `NEST_PATH_SEARCH_MARGIN`.
- **Destroying a nest now pays a cache**: a multiple of its species'
  work-resource drop, Portal Fragments scaled by how far below your current
  zone it sits, and three rolls of its equipment table. No XP — the
  guardians already paid that on the way down, and a nest chipped but not
  finished pays nothing yet.
- **Persisted across save/load**: a nest's position, Durability and pending
  respawns; each guardian's tether and whether it's currently pursuing.
  Without this a reload would launder a half-destroyed nest and its cache,
  and quietly clear whatever swarm was chasing you.
- **Fixed: a guardian dragged outside its tether radius could freeze
  permanently.** `wander_ai_system` refused any step that left
  `NEST_TETHER_RADIUS`; once something could push a guardian past that
  radius — the chase above — every neighbouring tile still counted as
  "leaving" it, so the guardian had no legal move at all. It now refuses a
  step only when doing so both leaves the radius and fails to close the
  distance, so a displaced guardian makes its way home instead of standing
  frozen.
- **Fixed: jacking out of a fight with a nest's guardians could put you
  right back into it before you got a single input.** `battle_flee`'s
  teardown moved no one and left every pursuer still marked, so the very
  next tick's pursuit step saw the same pack still adjacent and started a
  new battle on the spot — under permadeath, every attempt to leave cost the
  same XP setback for nothing. A step-away fix was tried and rejected: with
  `NEST_PURSUIT_STEPS_PER_TICK` exactly matching a one-tile move, the
  guardian's own ordinary step closed the gap straight back to adjacency in
  the same tick, on any open ground. A successful jack-out now clears
  `Pursuing` from every guardian that was actually in that fight instead —
  it shakes the pack rather than trying to outrun it. They aren't gone:
  `NestGuardian` survives, so they resume ordinary tethered wandering and
  the nest re-provokes them the next time you hit it. A guardian that
  wasn't part of that fight keeps chasing regardless, and a failed jack-out
  attempt shakes nobody.

### The zone group cap is a line, not a curve

- **`ZONE_GROUP_GROWTH` (geometric, x3) becomes `ZONE_GROUP_STEP` (additive,
  +9)**, so `zone_group_cap` runs 1, 10, 19, 28, 37 instead of 1, 3, 9, 27,
  81. Geometric growth from a base of 1 spent the whole playable range in
  single digits — zone 2 capped every group at 3, holding surface packs and
  Stack packs alike to three programs against a party of five whatever the
  distance and depth curves had earned — and then ran away past zone 4 into
  caps no encounter is designed around. Zone 1 stays solo, which
  `in_opening_ring` and the fresh-player species checks depend on.
- The trade is deliberate and it cuts both ways: the early zones gained
  range, zones 4+ lost their runaway (zone 5's cap falls 81 -> 37), and the
  hard `MAX_GROUP_SIZE` ceiling is now first reached at zone 12 rather than
  zone 6. This is the ceiling, not the roll — `spawn_pack` still draws
  uniformly in `1..=ceiling`, so a zone produces a wider *range* of fights
  rather than uniformly bigger ones.

### Depth is the Stack's distance

Engine-only, no save-format bump. Nothing new is persisted — depth already
lived in `Locale::Stack`, and both group curves are derived per fight.

- **Descending now widens the fight.** The surface escalates an encounter by
  distance from the danger origin; the Stack could not, because the party's
  `Position` is pinned to the entrance tile they walked in through, so
  `max_group_size` measured the base's own doorstep however far down they
  had gone. Every frame at every depth fielded one program in one group
  unless Trace had reached Hunted. `Game::danger_steps` is now the single
  input to both curves, taking frames descended underground
  (`GROUP_SIZE_STEP_FRAMES`, one frame per step) and tile distance on the
  surface.
- **A Stack ambush draws one species pick per group the depth allows**
  (`Game::stack_encounter_pack`). `group_pack` partitions by species, so a
  single draw was a single group however many the count permitted — raising
  the count alone would have been the same no-op that scaling only the spawn
  once was for `TRACE_GROUP_MULT`.
- **`SpawnEscalation` replaces `spawn_pack`'s loose `depth_mult`/`group_mult`
  pair** and carries depth alongside them. All three are properties of where
  the party is, and none may be read inside the spawn: ambient surface spawns
  and nest respawns keep rolling while the party is underground. Bundling
  gives that rule one home, and `SpawnEscalation::surface()` names the
  no-escalation case.
- Depth 1 is unchanged — it is the Stack's opening ring, and `in_opening_ring`
  and the fresh-player species checks still see a zone-1 surface fight as one
  program. `zone_group_cap` still binds: zone 2 caps every group at 3 whatever
  the depth.

### Bounded income: rest costs a consumable, scan is deleted

Engine-only, no save-format bump. The outlet is an ordinary inventory item,
`RestDef::cost` is asset data, and `Perk::KeenScavenger` keeps its save
index — nothing new is persisted.

- **Rest now spends a Power Outlet**, a new craftable item
  (`assets/items/outlet.ron`, 5 Core Fragments), and refuses with no outlet
  held. `Game::rest` checks and spends it after every existing gate
  (game-over, active battle, `require_surface`, `nearby_rest_structure`) and
  before the 40-tick loop, so a refused rest never consumes one and a rest
  that starts has already bought its ticks. The price lives on the
  structure that grants rest — `home.ron`'s `enables_rest` gained a `cost`
  field on `RestDef`, `#[serde(default)]` so an older `.ron` still parses,
  as a free rest, unchanged from before the field existed.
- **The scan action (`g` on the surface) is gone**, along with `Game::forage`,
  `forage_chance`, and the three `FORAGE_CHANCE_*` constants. `g` keeps its
  Stack meaning — the frame map — untouched; on the surface it is now a
  no-op.
- **`Perk::KeenScavenger` now boosts the mining roll instead of scan**,
  since its entire effect was a scan bonus and deleting the variant would
  have shifted every later save index. The flavour survives, the index
  doesn't move, and the perk now backs the thing this change wants players
  investing in: `mining_success_chance`, 50% at a level-1 node.
- **Two Power Outlets go into new-game starting inventory**, beside the 3
  ICE Breakers, 3 Power Cells and 5 Core Fragments, to cover getting
  established before the base earns anything of its own.
- **Portal Fragments are no longer sold at the iso Market.** Breaching is now
  earned by fighting — a 35% drop from any defeated program, a guaranteed 3–6
  from a boss, and a chance from a Stack cache. That listing was the only
  route from base production straight to progression, which is exactly what
  made settling in one zone and selling salvage a substitute for engaging
  with it. Credits surviving a breach no longer buys a way past content
  either, so a hole `enter_next_zone` previously priced rather than closed is
  now closed at the source.
- **`balance_sim::ticks_to_afford_portal` and its three tests are deleted.**
  It measured how long a worked node took to fund a breach *through that buy
  price*. With no buy price the question no longer exists, and a gate
  measuring a route the game doesn't have is worse than no gate. The engine
  suite drops from 1156 to 1153.
- **The Terminal is gone.** It converted a Core Fragment into a Power Cell
  every tick while you stood within 2 tiles, for a 3-Fragment build cost —
  the cheapest thing you could deploy, and a second free-Power source beside
  the Recharger Node. Power Grid research now unlocks the Power Conduit
  alone. It was the only shipped structure using `passive_process`, so that
  schema field and `passive_process_system` now serve mods only; both stay
  documented in `assets/structures/README.md`.
- **Deploying a Home now obliterates any Stack link under its platform**,
  the same way it already obliterates the hostiles and nests standing there.
  This is not a rare collision: `STACK_NEAREST_LINK_TILES` puts a zone's
  first link 5–8 tiles from where the player arrives, against a slab of
  `MAX_BUILD_DISTANCE_FROM_HOME` 7, so a Home near the arrival point swallowed
  one on a large fraction of seeds — leaving an entrance stranded inside the
  base, on floor nothing can spawn on. Breaching was never affected: there the
  platform is stamped before links are placed, and placement already skips
  platform tiles.
- **Core Fragments now drop from species a fresh player can actually beat.**
  Ranked by stat total, all three Power Cell droppers (`glitch`,
  `sub_process`, `scrapper`) sat in the gentle band the opening ring fields,
  while three of the four Fragment droppers (`worm`, `virus`, `construct`)
  sat behind fights a new player loses — so `drone` was effectively the only
  early Fragment source, and ten of seventeen species drop nothing at all.
  That was backwards: Fragments are what you build with, and Power Cells
  restore Power, which a Recharger Node gives away free. `sub_process` now
  drops Core Fragments instead of Power Cells, and `sprite` — which dropped
  nothing — now drops them too. `glitch` and `scrapper` keep Power Cells, so
  the early game still has a source before a Recharger is affordable.
- **A defeated program now drops 2–4 of its work resource, up from 1–2.**
  With scan gone, kills are the only source of Core Fragments outside a
  built base, and the first Mining Node — 12 fragments on top of Home's 5 —
  was about eight kills away while a single rest cost five. Only seven
  species carry a work resource (four Core Fragments, three Power Cells), so
  Power Cell drops double too; splitting the range per resource would be new
  asset data for a seven-species problem.
- Why: nothing limited how much a player could earn. Scanning was 1 tick for
  ~0.6 Core Fragments in a rich biome, returned roughly 50x the Power it
  burned, and inside a Recharger Node's radius Power *rose* while scanning —
  so Power was never a real cost, and research, gear and structures all
  reduced to keyboard time on one tile. The fix moves the limiter onto the
  actions themselves: **the base is the farm**. Rest is now an investment
  against 40 ticks of base output rather than a free action — at zone 1,
  tier 1, a Mining Node worked for a full rest cycle yields ~2 Core
  Fragments against a 5-fragment rest (a net loss), and break-even is
  roughly three worked nodes, which under the one-cronjob-per-structure rule
  means three structures and three programs, not three stacked on one node.
- This is unplayed balance — 5 fragments per rest and 2 starting outlets are
  arithmetic, not evidence, the same footing the Trace bands were on before
  playing moved them. The starting outlet count is the softener most likely
  to need retuning.

### One program per structure, per job

Engine-only, no save-format bump. A save that already has three programs on
one node keeps them until one is reassigned — the rule is enforced where a
job is handed out, not retroactively.

- **A structure runs one cronjob and holds one guard**, counted separately,
  so a worked Mining Node can still be defended. Assigning a second program
  to the same job stands the first one down and says so in the log.
- **Working a structure yourself holds the cronjob slot too.** `work_structure`
  puts the same `Task` on the player that a cronjob worker carries, so a
  player mining a node while a program worked it was the same double-payout
  in a different shape. Putting a cronjob on the node you're standing at now
  breaks your own concentration.
- Why this mattered: `task_progress_system` iterates *per worker* against the
  target node, and an emptied node refills to capacity, so three programs on
  one Mining Node was three times the income from one structure forever.
- Occupancy is read off the `Task` components rather than cached on the
  structure. `Task` is already the only record of who works what, and eight
  sites remove one — raid damage, demolition, sale, breach, party recall,
  fusion, rest and zone change — so a cached field would have had to be kept
  in step with all eight.

### Bosses are a wall, not a wait

Both boss `.ron` files, plus the comment on Decompile's boss refusal. No
save-format bump — a boss already in a save keeps the stats it spawned with.

- **Overseer drops from 1450 HP to 200, Wintermute from 1600 to 220.** They
  were 12–13x the toughest ordinary program's HP while their attack was only
  ~3x it and their defense ~1.5x, so a boss could not really kill you faster
  than a Sentinel could — you just had to hit it forty more times. That is a
  long fight, not a hard one.
- **A boss's real size is the multiplier stack, not its base.** Every wild
  spawn is scaled by zone x distance x Stack depth, so on frame 3 of a
  zone-3 stack a way-out tile multiplies base stats by roughly 11x. At 1450
  that was a **6,000 HP** program on screen; at 200 it is about 2,400,
  against 1,300 for a Sentinel standing next to it. The ratio is what was
  retuned; the absolute number follows the tile.
- Projected against `balance_sim`'s round loop, a *single* Overseer at 1450
  needed a level-15 party at zone 1, where a full zone-cap *group* of the
  toughest ordinary species needs level 1; at zone 5 neither boss was
  beatable at level 200. At 200 HP that zone-1 fight lands at **level 5 and
  9 rounds**, Wintermute at level 9 and 13.
- Bosses still cannot spawn in the opening ring — `beatable_by_a_fresh_player`
  remains false for both, asserted by
  `the_shipped_roster_has_species_on_both_sides_of_the_opening_ring`.
- Decompile still refuses a boss, but its comment no longer argues from
  `base_hp`, which has now moved twice. The durable reason is
  `growth_multiplier`: 2.0 on both bosses against 1.5 on every ordinary
  species, so a captured boss outgrows the roster it joins whatever it
  costs to bring down.
- `balance_sim` otherwise does not gate this and did not move:
  `toughest_ordinary_species` filters bosses out by design, so its curves are
  blind to boss stats. The numbers above came from a throwaway probe, not a
  committed test.

### The frame map, in the corner of the corridor

No save-format change: the renderer draws a view the engine already had.

- **The map of the frame you are in is always on screen**, in the top-left
  of the first-person corridor. Reading it no longer means leaving the view
  you are reading it *about*.
- **`g` still opens the full screen**, and the two answer different
  questions: the corner map is which way you are facing and where you have
  been, the full one is where the wing you have not walked is — three times
  the cell size, plus the legend that names every glyph. The corner map
  carries no text of its own for that reason.
- Both are drawn by one function, so a cell can never mean one thing on the
  small map and another on the large one.
- **`+` and `-` zoom the corner map**, from the whole frame down to a
  seven-cell window around the party — the same keys that size the surface
  map's tiles, since the two are never both on screen. They are separate
  settings, so a dive spent reading the maze up close does not resize the
  zone map you climb back out to. Against a wall the window slides to stay
  inside the frame instead of showing you the outside of it.
- Not playtested. How much of the corridor an always-on inset should cover
  is a matter of taste, and the size was picked by what stays legible at
  1280x720 rather than by playing with it.

### The orphaned process: a second way into the roster

**Save-format bump** — `SAVE_FORMAT_VERSION` is now 18 and existing saves
will not load. `dev-saves/` templates are unaffected; they are field-named
RON and keep parsing.

- **Orphaned processes** (`o`) — a program still running in a dead end with
  nothing left to serve, drawn from the same biome as everything else under
  that link. Press `o` while standing on one and it joins your roster for an
  ICE Breaker: no capture roll to win, no fight to survive, and no Trace,
  since taking one is a rescue rather than something you broke. Once per
  orphan; the dead end reads as empty afterwards.
- **The roster is the limit, not the catalysts.** A six-frame stack offers
  four or five programs against a base capacity of three, so a thorough
  descent gets refused near the bottom and the last orphans are scenery
  until you have built the space for them. Whether that reads as pressure
  toward capacity-granting structures or as a dead mechanic is the open
  question — the balance simulator models no roster and cannot answer it.
- **One per frame, in a dead end the caches did not take.** The placement
  pass runs after the caches and wants the same site type, so a frame needs
  four plain dead ends to field one and about a quarter of frames haven't
  got that many. "One per frame" is really three frames in four.
- **Which program it is survives a save and load.** The species is drawn
  from an RNG seeded off the frame itself rather than the run's shared
  stream, so you can see what is down there before deciding to pay for it.
  What it turns out to be *worth* is still rolled at adoption, like every
  other spawn in the game.
- Not playtested. The numbers above are arithmetic against a measured frame.

### Cell kinds: the Stack gets more than one kind of floor

**Save-format bump** — this shipped at `SAVE_FORMAT_VERSION` 17; the
orphaned process above took it to 18 before either was released.
`dev-saves/` templates are unaffected; they are field-named RON and keep
parsing.

- **Breakpoint** (`*`) — an exposed debug port on a junction. Walking onto
  one maps the entire frame at a stroke, walls included, and costs the
  single largest Trace spike in the game: two and a half caches' worth. One
  per frame, and each works once. The free map is meant to be a decision
  about the rest of the frame rather than a reflex.
- **Fault** (`v`) — a hole in the floor. Step on it and you drop a frame,
  landing in the far half of the one below rather than on its way up, so a
  fall is a quick way down and a long way back. Never generated on a bottom
  frame, which has nothing below it. Raises no Trace — falling is clumsy,
  not loud.
- **Corruption** — rotten substrate that costs Integrity every step through
  it, drawn as coloured ground rather than a glyph because it comes in
  stretches of three rather than single cells. That is the whole point: a
  lone bad cell is a toll you pay without thinking, where a stretch is
  something you can see and route around. The cost is a percentage of your
  maximum Integrity, so it stays meaningful at level 1 and at level 30 —
  Stack depth is uncorrelated with your level, and any flat number would be
  lethal at one end and free at the other.
- The frame map and the first-person corridor teach one vocabulary: the same
  glyphs and colours in both, with the map's legend extended to match.

### Trace: the Stack notices what you take

**Save-format bump** — this shipped at `SAVE_FORMAT_VERSION` 16; the cell
kinds above took it to 17 before either was released. `dev-saves/`
templates are unaffected; they are field-named RON and keep parsing.

- **A greed meter, not a timer.** Trace rises when you crack a cache, burn a
  seal or kill something underground. Walking is free — a meter driven by
  time or distance would tax exploration and punish the careful player,
  which is backwards for a maze whose per-frame map memory exists to reward
  learning it.
- **Four bands — Quiet, Noticed, Traced, Hunted** — shown in the Stack
  heading, with each crossing announced in the log. The band is all you see;
  there is no number, because it is there to tell you whether to press on
  rather than to be played up to a threshold.
- **Each band draws ambushes more often, scales what arrives, and sends more
  of it at once.** The stat scaling reaches the lair guardian too, so a
  party that stripped a stack on the way down meets its boss at the band
  they earned.
- **Trace clears on surfacing**, by either the link or a symlink. Not a free
  reset: caches, seals and lairs are one-shot per stack, so a trip up to
  shed a band means coming back to less worth taking.
- Every number is arithmetic against a measured frame and has not yet been
  playtested. The per-source ratios are grounded; where the band lines fall
  is the part that still needs playing.

### Trading in one keypress

- **`S` sells one, `B` buys one, straight off the highlighted row.** No
  quantity page, and the list stays put. A trade visit is normally a run of
  trades, so the four-screen round trip was being paid per item. `S` also
  works from the inventory screen when there is a trader in range; with two
  in range it asks which, since their buyback shelves are separate.
- **A program row is deliberately excluded.** `S` there opens the same
  confirmation Enter does — selling a levelled program is permanent, and a
  single keypress is exactly the mis-hit that confirmation exists to catch.
- **Inventory and trade lists are grouped by category** — consumables,
  weapons, armour, modules, routines, salvage, currency — with each row
  tagged `USE`/`WEP`/`ARM`/`MOD`/`RTN`/`MAT`/`CUR`. The category is derived
  from the fields an item already declares, so a modded item is grouped
  without its author adding anything.
- **Uppercase letters no longer pick menu rows.** They are reserved for
  screen actions now, matching what battle already did with `A` and `D`.
  Lowercase row shortcuts are unchanged.

### The dungeon is the Stack

- **Renamed the underground layer, top to bottom: dungeon → the Stack,
  level → frame, shaft → stack, entrance/breach → link.** Nothing about how
  it plays changed — this is a vocabulary pass, not a mechanics one. "Link"
  covers both scales deliberately: the hole on the zone map and the way
  between frames are the same kind of thing at different scales, and one
  word for both says so. The help screen, the deep-scan log line, the
  descend/climb/bottom-out messages, the map heading, the first-person
  prompts, the README and manual all speak the new vocabulary now. That
  makes three player-visible senses of "link," not two: `use_symlink`
  already used the word for the action that leaves the Stack ("The symlink
  hauls you up out of the stack and drops you at {name}"). In Unix register
  a symlink is a symbolic link, so the overlap reads as coherent rather
  than accidental — noted here so it doesn't look overlooked, not because
  it's being reopened.
- **"Breach" survives, narrowed to zone travel.** That word already meant
  three different things — the old dungeon entrance, stepping through a
  Zone Portal, and "ICE breached!" in combat — and renaming sense one is
  what frees the word to mean just the other two. "You breach the portal
  and materialize in a level {n} sector" and "ICE breached!" are unchanged.
- **Item and structure `.ron` ids and field names are untouched.**
  `access_shard` is still `access_shard` — only its human-readable
  `description` changed, along with `assets/items/README.md`'s
  `cache_drop` schema note. Ids are save-and-recipe keys mods depend on;
  they were never part of what this pass renamed.

### The party line is yours to arrange

- **`<` and `>` on the companions screen move a member along the battle
  line.** Slot order already decided who drew enemy fire — front slots are
  weighted heavier in the target roll — but the only way to change it was to
  stand a program down and re-add it, which appends to the back. `Game::
  move_party_member` swaps two adjacent slots and nothing else; no tick, so
  shuffling the roster stays free the way adding and standing down already
  were.
- **Refused during a battle.** `BattleState::planned` indexes the party
  positionally, so a mid-round swap would hand two slots each other's
  planned action.
- **The roster now leads with the party in slot order,** with each member's
  slot on its row. The sort lives in `Game::owned_pets` rather than in a
  renderer: app-core maps number keys by index while gui draws the rows, and
  sorting in one but not the other picks a different program than the one
  the player pressed.

### You can work a node yourself

- **`W` works a structure the way a program does.** The player takes the
  same `Task` a cronjob worker carries, advanced by the same tick and paid
  through the same `resolve_gather_cycle`, so the yield off a given node is
  identical whoever is standing at it. Extracted rather than reimplemented
  — a second payout formula for the player is precisely the drift this
  codebase has been bitten by before.
- **Moving breaks off the job.** There is no separate working mode: the job
  runs while the world ticks, and any attempt to move drops it — including
  one that walks into a fight or bounces off a wall, since either way you
  stopped working to do it.
- **No XP, unlike a cronjob.** A posted program levels from its work; the
  player doesn't, or a node becomes a risk-free XP faucet. The job is also
  not saved, so loading puts you beside the node rather than mid-cycle at
  it — that costs at most one cycle and no save-format bump.

### Fatigue recovers on its own

- **Fatigue now regenerates every tick instead of draining.** It was never a
  survival need — nothing starves you for running out of it, and its one
  job is paying for battle routines (`AbilityDef::fatigue_cost`). But the
  only full restore is resting, which refuses anywhere outside your base,
  so a long trip underground drained the pool with no way to refill it and
  quietly took your abilities away. Hunger is now the only clock that runs
  down, and the only one that can kill you.
- **Regen keeps running during a battle.** A routine's cost is a throttle
  on how often you can afford it in a fight, not a fixed budget for the
  whole fight. At `FATIGUE_REGEN_PER_TICK` a 5.0-cost routine is worth
  about 62 ticks of walking — arithmetic, not playtested.

### A save can be edited from the command line

- **`savetool` reads a save out as RON, takes it back, and can warp it
  forward.** Saves are bincode, which has no field names on disk and can't
  be hand-edited, so testing anything deep in a run meant playing there.
  `dump` renders one as text, `pack` re-encodes an edited one at the
  current save version, and `warp <n>` advances a save to a later zone by
  running the real breach — the base still travels and spawns still scale,
  rather than a zone number being overwritten in place.
- **`pack` always stamps the current save version**, so dumping before a
  format bump and packing after is the one way to carry a save across a
  version change. There is still no automatic migration, by design.
- **`dev-saves/` holds named worlds you can generate into.** A state you
  had to play up to becomes one you can regenerate in a second:
  `savetool capture <save.bin> <name>` records a save as a checked-in RON
  fixture, and `cargo run -- --template <name>` regenerates it and boots
  straight in. Shipped with `extraction` — nine tamed programs, four of
  them carrying 4–6 routines across every effect category, and a standing
  Compiler — because testing routine extraction otherwise starts with an
  hour of taming.
- **A template is copied, not played in place.** `--template` writes
  `saves/dev_<name>.bin` and opens that, overwriting it every run. The game
  autosaves, so opening the fixture directly would rewrite it, and it would
  decay into a record of the last session instead of a known world.
- **Templates are RON so they can outlive a save-format bump**, and
  `every_checked_in_template_still_loads` says whether they have. It
  generates each one through the real `Game::load` and compares the tamed
  count across it — a species id that has left `assets/species/` is skipped
  on load rather than rejected, so a gutted template would otherwise still
  open and simply be missing the programs it exists to provide.

### Field routines: buffs cast outside battle that keep running into one

- **An ability can now be field-only.** `AbilityEffect::FieldBuff` is the
  marker — there's no separate flag — and an ability carrying it never
  appears in the in-battle Special picker and is never used by a wild
  carrier. Instead it arms a buff from the map: press `a` to open the cast
  screen, spend Power and a turn, and the buff starts ticking. There is no
  cooldown. The buff keeps running through whatever battle follows and
  survives a save — unlike the buff a companion's Special arms mid-fight,
  which is still wiped the instant that battle ends. A panel under the map,
  and another in the battle roster, list every buff currently running,
  however it was cast.
- **Ten field routines ship, one per `FieldBuffKind`.** Four land on one
  ally you choose — Repair Loop (heal over time), Overclock (flat Attack),
  Hardened Shell (flat Defense), Ablative Layer (percent damage reduction)
  — and six always land on the player regardless of who casts them, since
  they're pressure or economy knobs the whole run feels rather than one
  combatant's stats: Coolant Flush (Fatigue over time), Trickle Charge
  (Power over time), Deep Scan (capture odds), Trace Analysis (XP), Ghost
  Protocol (encounter chance), Salvage Routine (drop chance). Scaling cuts
  across that scope split, not along it: five kinds — Repair Loop, Overclock,
  Hardened Shell, Coolant Flush and Trickle Charge — scale with the caster's
  level and affinity like any other ability; the other five — Ablative
  Layer and the four remaining percentage kinds — deliver their authored
  number unscaled, so a routine's percent bonus can't be pushed past its own
  ceiling by a high-level, high-affinity caster. See
  `assets/abilities/README.md`'s `FieldBuff` section for the full scope and
  targeting rules.
- **Three research nodes hand the ten over**, branching off `self_exec`
  where the other player routines already hang: Field Operations (16) for
  Repair Loop, Coolant Flush and Trickle Charge, then Adaptive Plating (32)
  for Hardened Shell, Overclock and Ablative Layer, and Deep Analysis (46)
  for Deep Scan, Trace Analysis, Ghost Protocol and Salvage Routine. As with
  every other researched routine, unlocking a node drops the item into cargo
  and installing it is a separate act. No species grants one and none has a
  wild-carrier weight, so a companion's own kit is unchanged.
  `every_shipped_field_routine_can_actually_be_obtained` pins this: a
  routine in no research node and no species kit fails the suite, because an
  ability file existing is not the same as a player ever seeing it.
- **An item's `prebattle_buff` now arms the same mechanism**, fixing two
  bugs it had carried since it shipped: the buff it armed was destroyed at
  the end of every battle despite the name, and it never survived a save.
  Both are now true of it, because it's the same `FieldBuff` a routine arms.
  See `assets/items/README.md` — the field now carries a `kind` and counts
  in ticks, not rounds.
- **The numbers are unplayed.** Every duration, magnitude and Power cost
  across the ten shipped routines is an arithmetic-plausible starting
  guess, not a tuned value — most visibly `trickle_charge`, which returns
  more Power over its run than it costs to cast, a choice made knowingly
  rather than caught late. None of this has been through an actual
  playthrough.
- **Save format bumped to v15.** Both `PlayerSave` and `CreatureSave` gained
  a `field_buffs` list. **A save written under v14 will not load** —
  bincode has no field-level migration path, so it's rejected up front with
  a clear message, the same policy as every prior bump.

### Raid damage is something you repair, not something you wait out

- **New structure: the Patch Node.** It repairs every deployed structure in
  the base — itself included — by 1 Durability per upgrade tier, every 20
  ticks. Several of them stack, the way Shields do, and it upgrades to Mk5
  for 6 per interval. Unlocked by **Fortification**, the same research node
  that unlocks the Shield, which now reads as the two halves of one answer:
  a Shield stops raid damage landing, a Patch Node undoes what lands anyway.
- **Raid damage is now permanent until you repair it.** Structures used to
  regenerate 4 Durability per interval for free — exactly `RAID_DAMAGE`, so
  one interval fully undid one raid and a base won the attrition race by
  doing nothing at all. Raids were an inconvenience with a timer on them.
  Free regen is gone entirely (`STRUCTURE_REGEN_AMOUNT` deleted rather than
  lowered), so every point of repair now comes from a deployed Patch Node.
  This is gentler than it sounds — an undefended structure still takes ~7
  raids to lose, and workers and guards already cut incoming damage — but
  the slope finally runs the other way if you ignore it.
- **Fixed: nests were quietly regenerating.** A nest carries Durability like
  a structure does, and the regen pass healed every Durability holder
  indiscriminately — so chipping a nest down with bump-attacks was racing its
  own healing, and walking away from a half-destroyed one gave the progress
  back. The manual already said a nest's Durability "is only ever spent by
  you"; the code just didn't agree. The pass is now `With<Structure>`, so
  neither the baseline trickle nor a Patch Node's repair reaches a nest.
- `StructureDef` gains an optional `repair: Some((per_tier: N))` field, so a
  mod can declare a repairer of its own without touching Rust — documented
  in `assets/structures/README.md`. Existing structure files parse unchanged.

### Fatigue is visible where it is spent

- **The battle roster has a `FATIGUE` column.** Fatigue prices every routine
  and refuses the ones you can't pay for, and the intrusion screen was the one
  place it wasn't shown — so "not enough Fatigue" quoted a figure nothing on
  screen named. It sits on your row alone: Fatigue is one pool, yours, spent
  whichever member runs the routine, so a companion's cell is a `—` rather
  than a fourth copy of your number.
- **The ability picker prices each routine** (`8 FTG`) next to what it does,
  and lists an unavailable one greyed with the engine's own reason — a
  cooldown, no catalyst, a full roster, or Fatigue you don't have. It
  previously showed descriptions alone, with neither the name nor the cost nor
  the reason.

### Two read-only screens

- **`L` shows the message log in full.** The pane under the map fits a few
  lines, so anything that scrolled past was gone. The new screen is the last
  `MESSAGE_LOG_CAP` (100) lines, oldest-first like the pane, each in its
  `MessageKind` colour, scrolled with Up/Down and opening on the newest line —
  the one that just left the pane. Its footer states its own three limits
  instead of leaving them to be discovered: the 100-line bound, the folding
  below, and that `retain_outcomes_since_battle` keeps a finished intrusion's
  results rather than its narration.
- **Repeated lines fold into one row with a dim `×N`.** Automation is
  repetitive by nature — a base with three cronjobs pushes a yield line per
  producer per cycle — and the screen was mostly forty copies of the same
  extraction with the raid you were looking for buried among them. An
  identical line now folds into a recent one even with a couple of lines
  between it, which is what catches several cronjobs interleaving their
  yields; the same line again much later stays its own row, so two starvation
  warnings still read as two events. The stored log is untouched, folding is
  the screen's alone — the pane under the map and a battle's narration still
  show every line.
- **`B` shows every structure and what is assigned to it.** Tier, tile,
  distance and raid Durability per structure, and *every* program posted to
  each one — a cronjob worker and a guard can share a structure, which the
  map's `structure_worker` label could never show, since it comes from a map
  keyed by the task's target and collapses the two. A workable structure with
  nobody on it is flagged idle. Backed by a new `Game::structure_report`,
  which is deliberately zone-wide where `view_entities` takes a radius: the
  base sits near its Home but the player wanders, and a roster that thinned
  out as they walked away would be worse than none.
- Neither screen takes an action or advances a tick.

### Balance

- **Routines now scale with the level of whoever runs them.** Ability
  `Damage` and `Drain` never did: `battle::compute_damage` is
  `power + ATK - DEF`, and the caster's ATK was held to carry the whole
  progression. It cannot — `ATK_PER_LEVEL` is 1 against `HP_PER_LEVEL`'s 12,
  and zone depth doubles Integrity again on top — so an authored power fell
  further behind its target every level. A level-10 player with the Damage
  affinity perk five deep, spending the heaviest single-target routine in
  the game against a 400-Integrity program, hit for **35**. The same cast
  now lands **147**.
- **There are two level curves now, not one, split by what the magnitude is
  measured in.** `Damage`, `Drain`, `Heal` and `Debuff` are HP figures and
  scale on the new, steeper `ABILITY_HP_SCALE_PER_LEVEL`; `Buff` and
  `FieldBuff` are stat points or percentages and stay on
  `ABILITY_STAT_SCALE_PER_LEVEL`, unchanged at its old rate. Putting a `Buff`
  on the HP curve would have turned a +3 attack routine into a tripling — the
  single-curve version of this change was not viable, which is why the split
  exists rather than one raised number.
- Heals move with damage: a Checksum Repair on a perked level-10 caster goes
  from 78 to 156, a Cold Boot from 156 to 313. Bleed, whose power is HP per
  round, moves with them for the same reason.
- Hostile carriers get the same curve, read off the zone rather than a level
  they don't have (`Game::ability_user_level`), so a wild routine at zone 4
  goes from 16 to 42. That is the cost of the change and it is deliberate:
  the player's own level outruns the zone number they are standing in.
- No authored `.ron` power changed. `balance_sim` models no abilities at all,
  so its curves do not move and never covered any of this — the new
  `a_perked_level_ten_kernel_panic_lands_in_the_intended_band` is the only
  regression gate on ability magnitudes.

### Fixed

- **A heal announced what it rolled, not what it landed.** "Medic patches
  you for 23 HP." was the scaled figure the ability produced, printed
  before the target's ceiling was applied — so a patch on someone three
  points down claimed twenty-three, and a patch on someone at full health
  claimed all of it while doing nothing. Both the `Heal` line and Drain's
  "restoring" figure now report HP actually restored, which is `0` when
  there was none to give back. `Game::restore_hp` is the single clamp both
  read from, the way `apply_damage` is for damage.
- **The party roster no longer prints ATK and DEF.** The row had grown long
  enough to crowd the quality, fusion and activity tags that only appear
  situationally; Integrity and Power carry the at-a-glance comparison. The
  fuse pickers still show the full stat line, since that is the screen where
  the numbers decide something.
- **A trade dropped you back on the map.** Every completed transaction —
  item sale, purchase, buyback, program sale — closed the trader's screen,
  so clearing a full pack a stack at a time meant walking `t` → trader →
  row → quantity again for every line. A finished trade now lands back on
  the trader's list, and only a death mid-visit takes the screen away. The
  list also shows your Credit balance now: the payout is announced in the
  log pane, which the trade popup covers.
- **The sell list offered the money and hid the salvage.** The screen drew
  your inventory minus Credits (a trader won't buy Credits for Credits),
  but the key handler behind it filtered out Core Fragments instead — so
  the two lists disagreed by a row whenever you carried both, and picking
  the top sell row offered the trader your Credits and got refused. Both
  sides now filter on the trade currency.
- **A battle refusal never faded.** Every message clears itself after four
  seconds, because it is drawn over the action bar it explains — but any key
  press restarted that window, on the theory that a message left standing
  deserved its full time. In a battle, where planning a round is several key
  presses and nothing clears a refusal on success the way the map menus do,
  that meant "Can't do that — 3 more rounds." stayed over the action bar for
  as long as the player kept playing. A message now keeps the window it was
  raised with, and only a *new* one gets a fresh window.
- **A held letter or digit drove the screen its own press opened.** Extracting
  a routine looked broken — nothing salvaged, the program still alive, and no
  refusal saying why. Letters and digits reach the game from
  `KeyboardInput::text`, which carries the OS auto-repeat (that is what makes
  `hjkl` walking work) and which `KeyRepeat::block_held` cannot reach. So a
  `2` held on the extraction picker chose the routine, opened the
  confirmation, and the repeat landed there — where every key but Enter backs
  out. The new `TextGate` applies `block_held`'s own rule to that path: one
  screen per press, until the key is released. The same hole dismissed the
  dungeon map and the help page instantly, and could carry a held key
  straight through `Mode::GameOver`.

### Affinities

- **A species can now be good at something, not just tougher or softer.**
  Five categories — `damage`, `heal`, `buff`, `debuff`, `drain`, one for
  every `AbilityEffect` that carries a magnitude — get a per-species
  multiplier declared in the species' `.ron` file, clamped to 0.5–2.0 at
  load. It applies to whatever is *installed* in a program's routine slot,
  not only to abilities its species natively grants, so a species with a
  strong heal affinity and no innate heal is now a reason to pop a
  researched or extracted heal routine onto it rather than another program.
  Six shipped species carry one strength and one weakness apiece —
  SubProcess leans heal over damage, Sentinel buff over damage, Cipher
  debuff over heal, Rootkit drain over buff, Scrapper damage over heal,
  Ghost damage over buff — and the rest of the roster, both bosses
  included, stays neutral across the board. `Cleanse` and `Decompile` have
  no magnitude, so no affinity ever touches them.
- **Five new perks buy the same specialization for the player**: Payload
  Tuning, Field Medic, Overclocker, Corruption Vector, and Siphon Protocol,
  2 Perk Points each, clamped at the same 2.0 ceiling a species' own
  affinity is. Field Medic, Overclocker and Corruption Vector are +5% per
  level; Payload Tuning and Siphon Protocol are +15%, because `Damage` and
  `Drain` skip the level-scaling the other three categories get for free —
  at the shared 5% rate they were a strictly worse Perk Point than the
  `Attacker`/`Defender` perks for nearly every shipped ability, and 15% is
  what makes them competitive up to their (lower, faster-reached) ceiling.
  A player perk only ever sharpens the *player's* own casts — a companion
  answers to its species instead — and the two sources can never stack: the
  player has no `Creature` component to hold a species affinity, and a
  companion has no `Perks` to hold a perk one.
- **The manifest screen shows a program's non-neutral affinities**, up to
  two, with any further ones collapsed into a "+N more" line rather than
  crowding the panel.
- **No save-format bump.** Species affinities are `.ron` data, reloaded
  fresh every start same as any other species field; the five new `Perk`
  variants were appended to the end of the enum rather than inserted, so
  every index an existing save already holds in `PlayerSave::unlocked_perks`
  still points at the same perk it always did.
- **The magnitudes are unplayed.** The two per-level rates, the 0.5–2.0
  clamp, and all twelve shipped species values are arithmetic-plausible and
  nothing more — none of it has been through an actual playthrough, and
  `balance_sim` structurally cannot vouch for it either, since that
  simulator models no abilities at all. A green suite here is evidence the
  arithmetic doesn't crash, not evidence the numbers are right.

### Dungeons

- **First-person dungeon levels.** Every zone is now seeded with a handful
  of **breaches** (`>`) standing on open ground. Walking onto one drops the
  party into a procedurally generated dungeon level, drawn as a receding
  first-person corridor rather than the top-down grid — the movement keys
  become forward, back, turn left and turn right, and `<`/`>` take the
  stairs. Levels are mazes with their dead ends partly braided back into
  loops, generated deterministically from the world seed and the depth
  walked to, so a level is the same every time you return to it and costs
  the save file nothing.
- **A map of what you have seen.** `g` — the same key that scans the ground
  on the surface — opens the party's map of the level they are standing in.
  It is drawn north-up and records only what has actually been in view:
  corridors walked, stairs found, and the corridors something jumped you in.
  Cells never seen stay dark and are drawn differently from rock that has
  been seen. Opening it costs no time. Each breach keeps its own map, and
  each level of a shaft its own; the map is written to the save, because a
  level regenerates from its seed but what you have seen of it is history.
- **Doors, and a vault at the bottom.** Levels now hang a few plain doorways
  in their corridors — walkable, but you cannot see past them, which turns a
  stretch of corridor into a decision. The lair is walled off behind **sealed
  doors** on every way in, and opening one burns an **Access Shard**: a new
  item found in caches. Once opened a door stays open, including across a
  save, so the way back out is free and a party that spent its last shard
  getting in is not stranded. A shaft is now a chain: caches pay for the
  shard, the shard buys the vault, the vault holds the guardian.
- **Something is holding the bottom.** The deepest room of a shaft is now a
  lair, marked `&`. Walking in starts the fight; what you face is drawn from
  the breach tile's biome, from the boss pool rather than the ordinary one,
  and is seeded off the level itself — so leaving and coming back cannot
  reroll a shaft's guardian into something easier. Biomes with no boss (no
  shipped Static Field species is one) field the toughest ordinary program
  they have instead, which at the bottom of a six-level shaft is no small
  thing. Jacking out leaves the lair held; killing what is in it clears the
  shaft for good, and that survives a save.
- **Caches.** Dead ends now hold something. `braid` deliberately leaves half
  of the maze's dead ends in place, and until now walking one was purely a
  waste of time; three cells per level hide a cache, marked `!` in the view
  and on the map. Walking onto one empties it, once and for good — the map
  stops advertising an emptied cache, so it answers "where is there still
  something" rather than "where was there once". Payout is depth-scaled
  Credits (the one currency that survives a breach), a chance at a portal
  fragment that grows with depth, and whatever the item set declares.
- **Items can declare themselves cache loot.** New optional `cache_drop`
  field on an item `.ron` — the chance a dungeon cache holds one. Rolled per
  cache per declaring item, so a mod adding items makes caches richer without
  touching engine code. See `assets/items/README.md`.
- **A breach is a shaft with a bottom.** Descent used to be unbounded — the
  depth counter simply went up, with the stat multiplier compounding the
  whole way and nothing down there. A breach now opens onto a shaft of fixed
  length, read off how far the walk to it was, so the same distance that
  scales what lives down there scales how much of it there is. The bottom
  level generates with no stairs down at all. Breaches also now carve their
  own dungeons: the entrance tile is part of the level seed, where before
  every hole in a sector opened onto the same maze at a given depth.
- **The surface keeps running while you are underground.** The player's
  position on the zone map stays pinned to the breach they entered by, so
  cronjobs keep paying out, needs keep decaying, and a raid can land on the
  base while the party is several levels down. Actions that reach into the
  zone map — deploying, cronjobs, guarding, demolishing, upgrading,
  trading, resting and scanning — are refused underground and say so. Party,
  inventory, routine, fusion and perk management are not: sorting your gear
  in a dungeon is a thing the genre expects.
- **The symlink is the way out.** `u` works at any depth, and is the only
  guarded action that changes locale rather than being refused by it: it
  surfaces the party and then teleports them to the structure they picked,
  for the usual cost. It cannot be used to flee a fight — symlinks are
  refused mid-battle wherever you are — and it gives up the descent, since
  the breach puts you back on level one. It does not give up the maps: those
  are keyed by breach and depth, so every level already walked comes back
  drawn.
- **Arriving in a sector scans for breaches.** The log reports how many are
  in it and which way the nearest lies, and one breach is always placed
  within sight of where you materialize — the other two are a walk. Without
  this the layer was effectively invisible: at the default zoom the map pane
  shows about ±16 by ±9 tiles, and three breaches scattered across a
  40-tile radius left most seeds with none on screen and nothing to suggest
  they existed.
- **Dungeons have teeth.** Every step underground rolls for an intrusion.
  The pack is drawn from the biome the breach opens in — a dungeon has no
  biome of its own, so it reads as the substrate beneath the ground above
  it — and each level of depth multiplies enemy stats, and with them the XP
  a kill pays. Shoving at a wall never draws a fight; only a step that
  covered ground does. Jacking out ends the fight where you stand, and the
  pack is cleaned up rather than left waiting at the breach mouth for when
  you climb out.
- **Save format bumped to v13**, so saves written by earlier builds no
  longer load. `SaveData` gained the locale (depth, cell, facing and
  entrance) and the zone's breach tiles; the level itself is not stored,
  regenerating from the seed.


### Economy

- **Traders deal in Credits, not Core Fragments.** Core Fragments were both
  the raw salvage every build cost and recipe consumes *and* the money a
  trader paid you, which made the iso Market a scrap dispenser rather than a
  merchant. Selling now pays **Credits**, a new item minted by nothing else,
  and buying spends them. Core Fragments stay exactly what they were for
  building, mining and scanning — and are now sellable, since they are
  ordinary goods to a trader.
- **Credits survive a zone breach.** Core Fragments and Portal Fragments are
  still wiped in the crossing, so a zone must still fund its own exit, but
  converting a doomed stockpile into money before you go now works and is
  the point of having a trader. Selling into a 1-Credit floor rate and
  buying back at 3–8 means the conversion is lossy; crafting from salvage is
  still cheaper than sell-then-buy, so the Market is for junk you cannot use
  and for value you want to keep, never for efficiency.
- `TradeCurrency` joins `Currency`, `ResearchCurrency` and `CraftCurrency` as
  a required economy role, so which item is money stays a data change rather
  than a code change. An item set that claims no `TradeCurrency` is refused
  at startup with the role named, as the other three already were. Prices and
  payouts now read the currency's name out of its `.ron` instead of saying
  "Core Fragments" in twelve hardcoded places — swapping the currency item in
  a mod no longer leaves the UI lying about it.
- No save-format change: `Inventory` is keyed by item id, so existing saves
  load with zero Credits, which is also where a new run starts.
- **Traders remember what you sold them, and will sell it back at double.**
  A sale used to be final, which made hoarding the safe play and selling a
  last resort. Everything you sell now goes onto that trader's buyback
  shelf, purchasable back at twice its `sell_rate` per unit. The shelf is
  finite — it holds only your own sales — and every round trip is a net
  loss, so it is a way to walk back a mistake, never a way to make money.
- A shelf belongs to the **tile** a trader stands on rather than to the
  building, so a Market levelled by a raid and rebuilt on the same footprint
  reopens with its stock intact, and two Markets in one zone keep separate
  shelves. Rebuild somewhere else and you have opened a new store; losing a
  trader that still holds stock now says what was on it and how to get it
  back. Shelves are wiped on a breach, alongside build salvage and breach
  keys — a shelf that crossed would be exactly the stockpile that wipe
  exists to strand.
- Selling a *program* still destroys it. It is not shelved, and cannot be
  bought back.
- **Save-format bump to 12.** `SaveData` carries every shelf, since a shelf
  outlives its building and can sit on a tile holding nothing at all.
  Existing saves stop loading.

### Moddability

- **Perk names, descriptions and prices are data.** `assets/perks/` now
  holds one `.ron` file per perk carrying its name, its picker line and its
  Perk Point cost, so retitling or re-pricing the menu is a file edit. It is
  a *catalogue* rather than a content directory: the twelve perks stay
  fixed, because a perk's effect is a hook into one particular formula — the
  scan roll, the hunger multiplier, the decompile HP term, recipe costs, a
  direct stat write — with no shape shared *across the full set* to put in
  a file, so a thirteenth perk still means Rust. (The five affinity perks
  added since are the one family that does share a shape among themselves —
  one generic `Perk::affinity_kind` hook, and one of two per-level rates
  depending on category, cover all five — but the magnitude is still a
  difficulty knob, not a per-perk `effect:` field, so the catalogue stays
  name/description/cost only.) Per-level magnitudes stay in `tuning.rs`
  with the rest of the
  difficulty knobs; only cost moved. Deleting a file removes that perk from
  the picker without touching levels a save already holds.

### Balance

- **Exploit Focus buys healthier targets, not better odds.** The perk used
  to grant +1 effective Decompiler skill per level, which was the same thing
  levelling up already hands you for free — so three levels' worth of Perk
  Points bought one level's worth of automatic growth, and the perk was
  strictly the worst purchase on the menu (about +0.5 percentage points of
  decompile chance for 3 points, against Attacker's +1 permanent Attack for
  2). It now shaves 3% per level off how much a target's *remaining
  Integrity* counts against the attempt. That is a different axis from the
  Decompiler stat: it is worth most against a program at full health and
  nothing at all against one you have already drained, so it buys you the
  option of cracking something early rather than a flat lift on every roll.
  Draining a target first is still worth up to 3.6x and no stack of the perk
  inverts that.
- **Bosses can no longer be decompiled.** A boss is built to be a wall —
  roughly thirteen times the Integrity of the toughest ordinary species —
  and capturing one moved that wall into your roster, where fusing two of
  them compounded it into something no encounter in the game could threaten,
  with two more fusions still to spare. Decompile is now refused the moment
  you aim it at a boss, so it costs you neither the round nor the catalyst,
  and the inspect and battle screens quote no odds where there is no attempt
  to make. Fusing a boss is gone with it, since fusion needs two compiled
  programs. Bosses keep their stats, their guaranteed Portal Fragment cache,
  and their place as the thing you beat rather than the thing you own. A save
  that already holds a captured boss keeps it.
- **A program brought to 0 HP is deleted for good.** It used to be knocked
  offline — dropped from your party at the end of the fight, then healed
  back to full for free the next time you recharged. Now it is destroyed,
  along with every routine installed on it, and nothing drops. This applies
  in Forgiving as well as Permadeath (that setting still governs only what
  happens when *you* flatline), and outside battle as well as in it: a
  cronjob worker that runs out of Integrity defending a structure is lost
  even though you were not there. Programs have no passive healing, so raid
  damage is attrition you have to manage by coming home to recharge. The
  battle pane and the party menu now flag any program at or below a third of
  its Integrity in red.
- **Structure income no longer doubles with every zone.** A worked node's
  payout used to be its upgrade tier multiplied by the zone's enemy-difficulty
  curve, so a Mk5 node paid 5 a cycle in zone 1 and 80 in zone 5 while build
  costs, upgrades and market prices all stayed flat — Core Fragments stopped
  being a constraint by about zone 2. Payout is now the tier *plus* one per
  zone below the current one: a Mk1 node pays 1 in zone 1 and 3 in zone 3, a
  Mk5 pays 5 and 7. Depth still pays, upgrading still pays, and neither
  compounds the other. Banked resources such as Research Data are unchanged
  at a flat 1 per cycle. Existing saves keep whatever they have banked; only
  the rate changes.
- **Decompiler skill can no longer be stacked past a species' resistance.**
  Skill used to add flat percentage points to a decompile chance, while the
  odds it was added to could never exceed 0.33 — so at about player level 30
  every attempt pinned to the 95% clamp, and a boss was no harder to take
  than a Drone. Skill now multiplies your odds instead, leaving the target's
  species and how far you have weakened it inside what gets multiplied. A
  fully-weakened Overseer at skill 40 sits near 30% rather than 95%; the same
  attempt against a Drone is near 59%. Low-skill play is essentially
  unchanged.

### Battle

- **Battle narration scrolls in instead of landing all at once.** A resolved
  round used to appear as a block of text; its lines now arrive at a steady
  pace you can read. Pressing any key skips straight to the end, and the
  action bar stays hidden until the narration finishes, so a key pressed
  mid-scroll never spends your round by accident.
- **Each battle's log pane starts empty.** The pane used to show the tail of
  one shared rolling log, so a fight opened on the end of the previous one.
- **Only a battle's results follow you back to the map.** The blow-by-blow
  stays in the battle it belongs to; what you read once the map is back is
  the kill, the XP, the loot, the level-ups and the decompile verdict — and
  those scroll in at the same pace. A raid that lands mid-fight is kept too,
  since that is world news rather than battle narration. One consequence
  worth knowing: the final round's blow-by-blow is dropped, so you see that
  you won and what you got, not the blow that won it.

### Renderer

- **The map glides under you instead of jumping a tile at a time.** There was
  no camera at all: the view was rebuilt around your integer position each
  frame, so a step moved the entire map by one whole tile in one frame. A
  camera now trails you and eases into place, and the grid is drawn shifted by
  whatever fraction of a tile it is currently behind. Because you are drawn at
  your own position rather than pinned to the centre, you visibly lead the
  camera while it catches up. It never falls more than a tile behind, which
  also means breaching a zone arrives as a cut rather than a long pan across
  terrain you are not in any more. Turning effects off makes the camera
  instant, the same way it makes an HP bar instant.
- **The ground has texture and the pane has depth.** Every tile of a biome
  used to draw at one identical colour, so a stretch of Open Grid read as a
  colour swatch. Each tile now varies slightly, by an amount fixed to its
  world coordinate — so the variation belongs to the ground and stays put as
  you walk over it. Tiles with something standing on them are left alone,
  since their colour is already carrying that structure's damage. Separately,
  the map dims toward the edges of its pane. That one is decoration rather
  than information: it stops well short of hiding anything, and a hostile at
  the edge of the view is as visible as it ever was.
- **Quitting asks first.** `q` in a run used to drop it on the spot, so one
  mistyped key cost every tick since the last autosave with nothing asked and
  nothing said. It now offers *save and quit*, *quit without saving*, or *keep
  playing* — and if that save fails it keeps you where you are with the error
  on screen rather than leaving anyway. `q` at the main menu, which ends the
  process, takes a plain yes/no; it sits one key from New Game and Load Game.
- **The log and the menus are colour-coded by what a line actually is.** Items
  you receive read blue instead of sharing green with a level-up. A hostile
  program's blow reads red, and orange when the move it used also inflicts a
  condition — at the time this shipped no hostile had a Special of its own,
  so a condition-bearing move was the whole of what made one special; a wild
  carrier running its own routine is now the exception. A party member's hit
  stays quiet except for the damage figure itself, which is picked
  out in bold white rather than the old all-or-nothing bolding of the whole
  sentence. On the research screen a node you have already unlocked greys out,
  so the list reads as what is left to buy rather than a flat menu. The Shield
  structure is blue on the map instead of red, which read as a threat.
- **A manifest screen (`d`) shows everything the sim knows about you or any
  program.** Integrity and XP meters (plus Power and Fatigue for you), combat
  stats, all four potential rolls behind the Excellent/Poor tag, installed
  routines, and — for a program — habitats, moves, work aptitude, growth and
  speed; for you, each equipped item with the bonus it is actually granting,
  and every perk with its level. `←`/`→` page between you and everything you
  own, and `Esc` steps back to the list rather than all the way out. Several of those numbers had no readout anywhere before: the
  individual rolls, growth, base speed, and a perk's purchased level.
  It replaces the old inspect popup rather than sitting beside it, so `i` +
  direction now opens the manifest for the program it finds.
- **The first frame no longer crashes on a missing font.** The window opened
  and the game died immediately with `FontFamily::Name("fp-ui") is not bound
  to any fonts`: the fonts were installed from inside the egui pass, but
  `set_fonts` only takes effect at the start of the *next* pass, so the first
  frame drew against families that did not exist yet.
- **The frontend runs on Bevy and egui instead of macroquad.** The engine was
  always built on `bevy_ecs`, so the sim and the renderer now share an ECS
  version — though they still meet only through `Game`, as before. Nothing
  about the screens changed: the same tiles, bars, popups and menus, drawn by
  the same code. Two player-visible differences, both improvements: typed keys
  now follow your keyboard layout instead of assuming QWERTY key positions,
  and overlapping sound cues play together rather than cutting each other off.
- **Drawing goes through one seam.** The ~3,000 lines that draw the screens no
  longer name a graphics library at all; they name a `Painter` with twelve
  operations on it, and the library sits behind that. That is what made the
  backend swap a change to one file rather than to every menu, and what would
  make the next one cheap too.
- Startup on a machine with no display still fails with a readable message
  rather than a backtrace.

### Encounters

- **Crossing open ground can get you ambushed.** Every walked step carries a
  small chance that a biome-appropriate pack drops in beside you and engages
  on the spot, with no option to route around it. Previously every fight was
  one you chose by walking into a program drawn on the map, which made travel
  free and the map a puzzle of avoidance. An ambush never fields a boss or a
  nest — those stay something you find and choose to take on — and never
  fires on your base platform, which remains safe ground. Tuned by
  `RANDOM_ENCOUNTER_CHANCE`.
- **Jacking out is an attempt, not a guarantee.** `j` used to end any fight
  unconditionally for a flat XP setback, so a hopeless battle cost 20% of
  in-level XP and nothing else. Whether you get clear is now a roll weighing
  your whole party's summed power against the whole pack's, times a fresh
  luck draw on every attempt — clamped so no escape is hopeless and none is
  certain. A failed attempt burns the round and draws a free volley from
  every engaged group, but costs **no** XP: you pay the setback only for an
  escape you actually got, so retrying bleeds Integrity rather than
  progression.

### Combat flow

- **The round-resolve page is gone.** A resolved round used to open a
  full-screen narration overlay that had to be dismissed before planning the
  next one. It never had anything to show — its log source was never written
  to — while the battle screen's own log pane was already carrying the real
  narration. Rounds now resolve straight back into planning, separated in the
  log by a dim `── round N ──` line.
- **`[A]ll attack` and `[D] all defend`** plan every open party slot in one
  keypress. All-attack asks which group only when more than one is still up.
  Neither overwrites a slot you already chose for.
- **Choosing Special now asks which ability, then who gets it.** Commanding a
  party member opens a picker of that member's abilities, and the screen that
  follows names your choice — `Pick a target (Hot Patch)` rather than a bare
  prompt. Backing out with Esc steps back one screen at a time. A planned
  Special also reads on the roster as the ability it will spend
  (`Hot Patch -> A`) instead of the generic word.
- **Buffs and heals can be aimed at any party member.** A buff or heal now
  lists your own side — you and every standing companion — instead
  of always landing on you. A debuff still picks an enemy group. Companions
  could never actually hold a buff before this: only the player is spawned
  with a buff slot, so one aimed elsewhere would have changed nothing.
- **Abilities are moddable data.** Drop a `.ron` file in `assets/abilities/`
  and it's picked up at startup, same as a species, structure or item — see
  `assets/abilities/README.md`. What used to be a fixed set of four things
  in Rust (rally, shield, heal, debuff) is now whatever the files say. A
  species names the ability ids it grants and the companion level each
  unlocks at, so a program's kit grows as it levels instead of being fixed
  when it's tamed. Ten abilities ship; seven species now declare kits, and
  the rest still fall back to Priority Boost.
- **Abilities can hit more than one target.** Three new shapes: every member
  of one enemy group (Cascade Overflow), every hostile program on the field
  (Broadcast Storm, Null Route), and the whole party at once (Redundancy
  Sync, Overclock Array). A sweeping ability skips the target picker
  entirely — there's nothing left to choose — and a party-facing one skips
  members who are already down rather than wasting the heal.
- **Rogue programs can now be destroyed from any rank.** Only a group's
  front member could ever take damage or die. Anything behind it was
  untouchable, which is what made area effects impossible; a back-rank
  program killed by one now drops out of its group and pays out its loot and
  XP exactly as a front kill does.
- **Abilities have cooldowns and their own Fatigue costs.** A sweep is a
  decision rather than a rotation: Broadcast Storm sits out four rounds and
  costs 15 Fatigue against the flat 5 an ordinary command charges. An
  ability that's cooling or that you can't afford is greyed in the picker
  with the reason, and can't be planned at all — no silently wasted round.
  Cooldowns last one intrusion and are never saved.
- **The trade screen shows equipment tags.** Sell and buy rows now carry the
  same `(WEP +3 ATK)` tag the inventory does, so you can tell a weapon from a
  module — and check a fusion tier — without backing out to compare. Sell
  rows show the tier of the copy you actually hold; buy rows show unfused
  stock.
- **The compile screen says what each item does.** Every recipe now carries a
  short gloss next to its name — `Power Cell (+25 power)`, `Arc Lance
  (+3 atk)` — so you can tell what you're building without leaving the menu.
  Read off each item's own definition rather than written per item, so a
  modded item gets one too.
- **The log moved between the two rosters.** Hostiles stay on top, your party
  is now along the bottom, and the narration sits between them, so the two
  sides face each other across the account of what passed between them.
- **The creature you're addressing is bold.** The party member currently
  choosing an action, and the group highlighted in the target picker, are
  drawn in bold on top of the existing highlight — those rows sit among others
  already coloured by faction and reach, and needed to win against that.
- **Both battle rosters are a stat table now.** Each side gets a column
  header and hard columns — `GROUP`/`NAME`, `HP`, `ATK`, `DEF`,
  `RANGE`/`POS`, `STATUS`/`ACTION` — with the numbers right-aligned, so you
  can compare two groups' DEF by scanning down a column instead of reading
  two sentences. Reach and status conditions move out of inline
  `<engaged>`-style tags into columns of their own, and a member with no
  condition reads `OK` rather than blank. The HP bars are unchanged.
- **Decompile odds are back on the battle screen, as a `DECOMP` column.** The
  hostile roster now carries each group's live compile chance beside the HP
  that drives it, so you can watch a group become worth taming as you wear its
  front program down. The engine had been computing per-group odds all along
  and the only place they surfaced was the target picker — you had to commit to
  Decompile before you could see whether it was worth trying, and during a
  companion's turn there was nowhere to see them at all. The column reads `—`
  when you hold no taming catalyst, since there is no attempt to quote odds
  for; the action bar still carries the reason. See
  [Decompile chance](README.md#stats).
- **Battle keys are lowercase, and Decompile moved to `c`.** Defend takes `d`,
  so the per-slot keys `a`/`d` and their party-wide counterparts `A`/`D` line
  up: shift means "everyone does this". Nothing sits one shift key away from a
  different action.

### Programs

- **You can sell a compiled program at an iso Market.** Until now there was
  no way to get rid of one: standing a program down with `p` frees a battle
  slot but leaves it owned and still counting against your roster cap, and
  fusing needs a second program you are also willing to lose. So a roster
  full of programs you had outgrown was a dead end, with decompiling refused
  outright at the cap. Selling pays a tenth of the program's power (max HP +
  Attack + Defense, rounded down, never less than 1) and frees the slot.
- **It is permanent, and it says so.** The sale erases the program, and the
  confirmation names anything the sale cancels — a party slot, a cronjob, a
  guard post — since it takes those down for you rather than refusing.
- **Modders:** whether a trader buys programs, and what it pays, is the new
  `program_sell_divisor` inside a structure's `trade` block. Omit it and the
  trader deals in items only, so existing structure files are unaffected. See
  `assets/structures/README.md`.
- **Fusing (`f`) now offers every program you own**, not just the ones
  standing within 40 tiles of you. Fusion itself never had a distance
  requirement — only the picker did, so a roster of workers left at far-off
  nodes reported "no compiled programs nearby" and there was nothing to fuse
  without walking the map to collect them first. Both pages now list the
  whole roster, the same way the pets screen does.

### Structures

- **The Recharger Node actually recharges you now.** Instead of gating rest,
  it passively restores 1 Power per tick anywhere within 7 tiles — the whole
  base footprint — with no assigned worker and no input item. Being home
  means never watching your reserves drain. Its cost rises from 5 to 10 Core
  Fragments to match. Power Cells and the Terminal are now expedition gear
  rather than daily upkeep.
- **Resting moved to Home.** `r` (recharge overnight) now works anywhere
  within 7 tiles of Home rather than within 2 tiles of a Recharger Node, so
  the base you already built is the place you rest. Existing saves need no
  migration and nobody is locked out — Home has always had to be built before
  anything else. A Recharger Node deployed under the old rules simply stops
  gating rest and starts regenerating Power, at the price you already paid —
  see [Structures](README.md#structures).
- **Modders get a `power_regen` field.** Any structure can restore Power in a
  radius by setting it; nothing in the engine names a structure id to do it.
  See `assets/structures/README.md`.

### Balance

- **Every difficulty knob is in one file now.** Tuning values were spread
  across three tiers with no boundary between them: 56 constants in the
  crate root interleaved with module declarations, 28 more scattered across
  seven other modules, and — worst — roughly thirty numbers that mattered
  most sitting anonymous inside formulas. The zone stat doubling, the
  steepest curve in the game, was an unnamed bit-shift. All of it is now
  `crates/engine/src/tuning.rs`, grouped into labelled sections with its
  documentation intact. No behaviour changed; the values are identical.
  Content stays data — species, items, structures, abilities and research
  are still `.ron` files and were not pulled into Rust.
- **`balance.rs` is `balance_sim.rs`.** It was never a table of balance
  constants; it is a 977-line offline battle simulator used as a
  regression-test harness, and the name sent anyone hunting for tuning
  values to the wrong file.
- Two formulas had been quietly duplicated rather than shared: the mining
  reliability curve was copied into the balance simulator whose own doc
  claimed it mirrored the real one, and a cronjob node's default worker
  capacity was repeated as a bare literal. Both now read one definition.
- **A pack is a swarm now, sized by depth and distance.** A wild group used
  to be a handful of programs however deep you were. Its ceiling is now the
  zone — 1 at zone 1, tripling each level (3 / 9 / 27 / 81) to a hard 100
  from zone 6 — and you only meet that ceiling by walking to it: a group
  doubles every 15 tiles from the edge of your platform. Four groups at a
  hundred apiece is the most one intrusion can hold. A swarm that size is
  an attrition problem rather than an instant wipe, because only some of a
  group can bring weapons to bear in a round — ten of a hundred, the square
  root of its size — and because anything past the front two groups has to
  shoot or idle. Members over a ceiling aren't deleted; they stay standing
  on the map and are met on the next bump.
- **How many groups meet you rides that same curve.** The *number* of
  groups used to jump straight to four anywhere on the map, while their
  *size* started at one and grew with distance — so the two halves of the
  same danger curve disagreed, most sharply right where you'd notice. It is
  now one group at your doorstep and one more per 15-tile step, up to four.
  An encounter beside your platform is a single program however deep you've
  breached.
- **A zone-1 opening is winnable.** Zone 1 caps a group at one member, but
  four groups could still engage — so a bump near where you materialize was
  routinely a four-on-one against a player who has no companions yet. Across
  25 generated worlds, 21 had a four-program fight inside 15 tiles of the
  spawn point. Offline projection scores that as a loss against every shipped
  species, and scores eleven of the fifteen ordinary species as a loss even
  one-on-one. So on top of the group-count fix, the first 15-tile step of a
  zone-1 breach now only spawns programs a bare level-1 player can actually
  beat solo, and never a boss. Where a biome has nothing gentle — no shipped
  StaticField species qualifies — it fields its mildest rather than its whole
  roster. Twenty of twenty fresh worlds now win their opening fight, taking
  4-8 rounds to do it.
- **Modders:** nothing new to author for any of this, but a species' base
  stats now decide whether it may open a run, not just how hard it hits. A
  new species that a level-1 player with 90 HP, 6 ATK and 2 DEF can't beat
  one-on-one simply won't spawn in that first step. It is still met
  everywhere else.
- **The base platform shrank from a 15-tile radius to 7.** The platform edge
  is also where the danger curve starts measuring, so hostiles now get tougher
  8 tiles nearer to home — the first stat-escalation step moves from 30 tiles
  out to 22.
- **Your fragments and cores no longer survive a zone breach.** Portal
  Fragments and Core Fragments are cleared as you step through the portal, so
  each zone has to fund its own exit instead of being chain-breached on a
  stockpile farmed somewhere safer. Research Data is banked progress and is
  kept, as are gear, supplies, fusion tiers, your party and your whole base.
  Modders: the wipe is keyed on the `Currency` and `CraftCurrency` economy
  roles, so a custom currency item resets too — see
  `assets/structures/README.md`.
- **A Zone Portal's cost ramp softened from ×zone to +50% per zone.** It cost
  10 Portal Fragments times your zone level; it now costs 10 plus half that
  again per zone — 10 / 15 / 20 / 25 / 30 through zone 5, where it used to
  reach 50. With currency no longer surviving the trip, the old ramp would
  have been a from-zero grind of ~143 kills for a zone-5 breach.

### Frontend

- **The text (TUI) frontend is gone.** It had not been user-selectable for
  some time — the launcher kept it only as a fallback for machines with no
  display and for GUI crashes — and maintaining a second renderer meant every
  screen change was made twice. The macroquad GUI is now the only frontend.
- **A graphical display is required.** Running with no `DISPLAY` or
  `WAYLAND_DISPLAY` now exits with `No display detected; feral-processes needs
  a graphical display.` instead of falling back to text. Playing over SSH or
  on a headless box is no longer possible.

### Routines

- **Abilities are installable routines now, occupying level-derived slots.**
  A companion gets one slot per two levels (capped at six, so six at level
  12); the player gets one per ten levels, starting with one so the first
  *free* slot lands at level 10. The `COMPANION_ROUTINE_SLOT_*` and
  `PLAYER_ROUTINE_SLOT_*` constants live in `crates/engine/src/tuning.rs`.
- **A species' innate kit installs itself.** It's pre-installed at tame or
  fuse time and topped up whenever a level-up reaches a later unlock. An
  innate routine can still be popped back out and plugged into a different
  program — nothing is permanently welded in.
- **Routine extraction.** With a Compiler standing anywhere on the map — no
  proximity requirement — break a program you own down into exactly one of
  its routines. The program and every other routine it carried are
  destroyed.
- **Decompile is an ability now, not its own battle command.** The player
  starts a new game with it pre-installed, reached through the Special menu
  like anything else. It greys out with a reason — no taming catalyst, or a
  full roster — instead of silently refusing the round.
- **Research hands over routine items, not the ability itself.** A
  researched unlock is compiled straight into cargo; installing it into a
  slot is a separate act from researching it, and two nodes naming the same
  ability now stack two copies of the item instead of granting it once.
- **Item and structure descriptions are authored in their `.ron` files.**
  `Game::structure_description`'s Rust derivation is gone, so a mod controls
  its own text end to end for both.
- **New map keys:** `m` opens the routine panel (install, swap, pop out);
  `M` opens extraction — needs a Compiler built somewhere, not nearby.
- **Save format is v11.** Older saves are rejected with a clear message, the
  same policy as every prior bump — this project does no save migration.
- **Wild programs can spawn already carrying a routine their species never
  grants, and will spend a round running it against you** — the first time a
  hostile's kit has ever been more than its species moveset. Decompiling the
  carrier hands the routine over already installed; destroying the carrier
  destroys the routine with it.
- **Twenty new routines exist that no species and no research node grants.**
  Finding a carrier in the field is the only way to get one.
- **Ability magnitudes now scale with the user's level**, so a heal or a
  buff picked up early doesn't fall behind the fights it's used in by the
  late game.
- **A routine that finds no free slot on capture now goes to cargo instead
  of being destroyed.** Popping a slot free later lets you install it same
  as any other routine item.
- **Every ability has a cooldown now, except Decompile** — it stays
  spammable, since it's the capture mechanic rather than a combat move.
  (Field routines, added later, ignore `cooldown` entirely: a field-only
  ability never reaches the Special picker, so there's nothing to throttle.)

## 0.2.0

### Breaking

- **Saves from earlier versions no longer load.** Party order is now
  persisted, because it decides which members stand in the front slots and
  draw more enemy fire — so it can't be rebuilt from creature-iteration
  order the way it used to be. That changes the shape of a saved program
  record, and the save format has no field-level compatibility.
  `SAVE_FORMAT_VERSION` is **v10**; a v9 save is rejected up front with a
  clear message rather than decoding into corruption.

### Party roster battles

- **Intrusions are now a party-versus-party round battle**, not a duel with
  an audience. The whole battle screen is replaced: hostile groups listed on
  top, your party listed below, both with HP and stats, and an action chosen
  for **every** party member before the round resolves.
- **Enemies fight as species groups.** A pack is sorted by species, so three
  Glitches are one addressable unit (`A  3 Glitches`) rather than three
  rows. Only a group's front member can be hit; empty a group and it drops
  off the list, promoting whatever stood behind it. At most four groups
  engage at once — a bigger cluster sends its four largest and leaves the
  rest on the map rather than despawning them.
- **Companions fight.** A party member that Attacks rolls one of its own
  species' moves and deals real damage. The old `[C]ommand companion`
  action is gone; its buff is now the **Special** action, one of the options
  that member can be given for the round, and still costs you Fatigue.
- **Only the front two enemy groups can reach you.** A group further back
  can use only moves its species flags `ranged`, and does nothing at all if
  it has none. This is the valve that keeps a twelve-program pack
  survivable, and it makes clearing front-to-back a real decision — wiping
  the front group promotes a back group into melee range.
- **Initiative is rolled every round.** Every combatant on both sides rolls
  `base_speed + d10` and acts in one interleaved order. `base_speed` is new
  per-species data, spanning 6 (Construct) to 14 (Sprite); the player rolls
  from 11. Both new species fields are optional, so existing mods keep
  loading untouched — see `assets/species/README.md`.
- **Defend** is a new action: a Defense bonus for the round, plus a much
  larger share of the incoming fire. It's applied before anyone acts, so
  bracing covers you against a faster enemy rather than being a coin flip on
  the initiative roll.
- **Soft ranks.** Enemy targeting is now weighted by party slot instead of a
  flat 30% chance to hit a companion. The first three slots draw noticeably
  more fire than the ones behind them, but every member stays reachable — a
  back slot is safer, never safe.
- **Bigger fights.** Party capacity goes 3 → 5, and a pack can reach 12
  programs (3 per zone level) instead of one more than the zone number.
- **The action menu is engine-generated.** Both frontends draw whatever the
  engine offers, including the reason an action is currently unusable, so
  the two can't drift and a new action needs no renderer change.

### Fixed

- **A seeded game is reproducible again.** The habitat spawn pools were
  returned in `HashMap` iteration order, which is randomized per process,
  and the per-tile spawn roll indexes into them — so two runs on the same
  seed picked different species and diverged from there. Only ever visible
  as intermittently-failing tests, never as a wrong-looking game, but it
  made any seeded test unreliable.

## Before versioning

- **Your base travels between zones**: breaching used to despawn every
  structure you'd deployed, which made staying anywhere long enough to
  build one a bad trade — the base was the only part of your progress that
  got deleted when you advanced, and the only one whose output never grew.
  Deploying a **Home** now lays a 15-tile **platform** under your base:
  flattened flooring that obliterates the terrain, nests and rogue programs
  in that radius, and that nothing wild will ever spawn on again. Step
  through a Zone Portal and the whole thing — platform, every structure on
  it, their damage, their stored resources, and their running cronjobs —
  rematerializes around the new sector's entry point in exactly the layout
  it left in. A base founded in zone 1 is the same base you're standing in
  at zone 6. Demolishing Home tears the platform up and the natural terrain
  comes back. See [Zones and portals](README.md#zones-and-portals).
- **A Zone Portal is consumed when you step onto it**: it's the one
  structure that doesn't make the trip. Without that, a portal carried
  forward with the rest of the base would make every breach after the first
  one free, and the 10-fragments-per-zone-level cost would stop meaning
  anything after zone 1.
- **Cronjob output scales with zone depth**: a finished cycle no longer
  drops exactly one unit. It's multiplied by your zone level on the same
  doubling curve wild programs' stats already used — zone 2 pays double
  zone 1, zone 3 quadruple — so a base keeps pace with the sector it's
  sitting in instead of falling further behind every time you breach. A Mk3
  Mining Node in zone 4 drops 24 Core Fragments a cycle where a fresh one
  in zone 1 drops 1. Research Data is the exception: it's banked against a
  200 cap, so it still pays one unit a cycle at any depth. See
  [Getting started](README.md#getting-started-building-and-running-cronjobs).
- **Structures can be upgraded (`U`)**: producing structures now have tiers,
  Mk1 through Mk5. Reaching tier N costs that structure's upgrade price
  times N, and a tier both multiplies what a cycle pays out and raises the
  odds the cycle pays out at all — a Mk1 Mining Node fizzles about half the
  time, a Mk5 about one time in ten. Upgrades ride through a portal with
  everything else, so they're the thing worth pouring materials into across
  a whole run. This is data, not Rust: any structure `.ron` file can
  declare an `upgrade` block, and every file that omits one stays
  un-upgradeable exactly as before, mods included — see
  `assets/structures/README.md` and [Structures](README.md#structures).
- **Danger now scales from your base's edge, not its centre**: wild stats
  still climb 25% every 15 tiles you wander out, but that count starts at
  the platform boundary instead of the tile you materialized on. Since the
  platform is itself 15 tiles across, the first step up moved from 15 tiles
  from Home to **30** — the whole base reads as safe territory rather than
  sitting exactly on the first escalation step. Pack sizes moved with it.
  Before your first Home there's no platform, so it measures from the entry
  point exactly as it always did.
- **Your base is a genuine safe haven**: because platform flooring lists no
  species as living there, nothing wild spawns inside your build radius at
  any zone depth. Raids are the only threat that still reaches it (see
  [Base defense](README.md#base-defense)). There's nothing to scavenge on
  it either — `g` always comes up empty on your own floor.
- **Save format bumped to v9.** Saves written by earlier versions no longer
  load.

---

- **Home can no longer be raided**: raids used to pick from every deployed
  structure, Home included, so a bad roll could destroy the one structure
  that gates every other build, anchors your symlinks, and can only exist
  once — stranding you rather than costing you something. Home is now a
  **non-raidable** structure: it has no Durability, shows no `[HP x/y]`
  anywhere, and is never selected as a raid target. Posting a guard on it
  (`G`) is refused outright now instead of silently wasting a program on a
  raid that will never come — see [Base defense](README.md#base-defense).
  This is data, not a special case in Rust: any structure `.ron` file can
  set `raidable: false` and get the same protection, and every file that
  omits the field stays raidable exactly as before, mods included — see
  `assets/structures/README.md`. You can still demolish Home yourself with
  `R`, cascade and all; non-attackable isn't indestructible.
- **The inventory screen now says which slot an item would take**: an
  equippable item's preview tag leads with its slot — `(WEP +4 ATK)`,
  `(ARM +4 DEF)`, `(MOD +3 DECOMP)` — instead of showing the bonus alone
  and leaving you to infer the slot from which stat moved. That inference
  was only ever reliable while every weapon happened to be pure Attack, and
  the 25-piece catalog's hybrids (a Recursion Blade is +2 Attack **and** +1
  Defense) broke it. Shows on both the inventory list and the item action
  menu, in both frontends — see [Equipment](README.md#equipment).
- **Taming catalysts are data, not one named item**: a decompile attempt now
  spends whichever item in your inventory declares the highest
  `taming_potency` (ties go to the first item id alphabetically), so a
  catalyst dropped in as a `.ron` file works exactly like the shipped ICE
  Breaker, and a stronger one is used in preference to it. No engine logic
  names the ICE Breaker any more — it's ordinary starting gear now, not a
  privileged item — see `assets/items/README.md`. The
  decompile-odds readout changed to match: it quotes the odds for the
  catalyst you'd actually spend, and with no catalyst in hand it reads
  "needs a taming catalyst" instead of a percentage for an attempt you
  can't make — see [Decompile chance](README.md#stats). For a player
  carrying only ICE Breakers, taming plays exactly as before.
- **Item files carrying `NaN` or infinity are refused**: RON accepts bare
  `NaN` and `inf` literals, and they used to survive every clamp downstream
  — a `NaN` `taming_potency` outranked every real catalyst, won the roll,
  then panicked the RNG. A non-finite `taming_potency`, `consume.power`, or
  `consume.fatigue` now disqualifies the whole file, which is skipped with a
  logged warning like any other malformed one. Relatedly, the
  duplicate-economy-role warning stopped Debug-printing ids at modders
  (`ItemId("core_fragment")` now reads `core_fragment`) — see
  `assets/items/README.md`.
- **README caught up with three systems it never documented**: the research
  tree, nests, and audio had all shipped without a word in the README, and
  the research tree in particular had silently invalidated its account of
  the opening — the build menu, the Structures table, and the Equipment
  recipes all described a game with no research gating. Now covered by new
  [Research](README.md#research), [Nests](README.md#nests), and
  [Audio](README.md#audio) sections, plus a rewrite of everything research
  touches. Also corrected: the Data Cache's cost and purpose, the market's
  name, the missing Research Node row, Portal Fragment and Research Data
  missing from the Items table, the carrying-capacity rules, `T`/`[`/`]`/`\`
  missing from the controls list, and a leftover claim that the player
  shares the level-12 cap. The backdated entries above cover the same
  ground from the release-notes side.

---

- **Items are now data-driven**: every item (Core Fragment, Power Cell, ICE
  Breaker, Overclock Core, Firewall Plating, Neural Amplifier, Portal
  Fragment, Research Data, Monofilament Whip, Ablative Plating, Cortex Hack)
  is now a `.ron` file under `assets/items/` instead of a hardcoded `ItemId`
  Rust enum variant, and `ItemId` itself is now a string newtype rather than
  an enum — drop a new item in as data, same as species and structures, no
  recompiling needed. This changes what a save stores, bumping the save
  format to **v8** (old saves need a new game). **Breaking for mods**: any
  species/structure/research file that named an item the old bare-variant
  way (e.g. `CoreFragment`) must switch to its quoted string id (e.g.
  `"core_fragment"`) — see `assets/items/README.md` for the schema and the
  full id mapping — see [Items](README.md#items) and
  [Modding](README.md#modding).
- **Crafting gained a data-declared starter-recipe path**: an item's own
  `.ron` file can now define its always-available "starter" recipe via a
  `craftable` field, rather than the two starter recipes (ICE Breaker,
  Power Cell) being hardcoded in Rust — see `assets/items/README.md`.
- **Consume action added to the inventory item menu**: `[C]onsume` now
  appears for any item that declares a `consume` block, applying whatever
  mix of Power/Fatigue/heal/pre-battle buff it defines. The `e` key changed
  to match: it now drains the first Power-restoring item found in inventory
  instead of being hardwired to Power Cells specifically. No player-facing
  mechanic changed — the 11 shipped items behave exactly as before — see
  [Items](README.md#items).

---

- **A research tree now gates most of the base**: press `T` to spend
  **Research Data** on a tree of 12 nodes. Research Data comes from a new
  **Research Node** structure (10 Core Fragments, buildable from the start)
  worked by a cronjob like a Mining Node, and it banks separately from your
  cargo rather than competing with it. Seven of the thirteen structures —
  Compiler, Terminal, Power Conduit, iso Market, Shield, Fabricator, Armory
  — plus every equipment recipe now sit behind a node, so the opening isn't
  "build whatever you can afford" any more: Home, Mining Node, Research
  Node, Recharger Node, Data Cache, and the Zone Portal are what you start
  with, and the rest is earned. Unlocking is permanent, and a recipe node
  grants only the blueprint — its bench still has to be deployed for the
  recipe to appear in the compile menu. The tree is data, not code: each
  node is a `.ron` file in `assets/research/`, and a structure named by no
  research file stays buildable from turn one, so existing structure mods
  keep working untouched — see [Research](README.md#research) and
  `assets/research/README.md`.
- **Carrying capacity is now a real constraint**: everything you carry
  counts against a shared cargo limit — your **Buffer** — starting at 30
  units. **Data Cache** is what raises it (+10 each, stacking, and its cost
  dropped from 15 Core Fragments to 10 to match its new job); it was
  previously just flavor. Paying an input cost that would overflow the
  Buffer — compiling, buying, unequipping — is refused outright rather than
  clamped, so nothing you already spent gets destroyed. Research Data is
  exempt, banked against its own 200-unit ceiling, so a pile of loot can't
  starve a Research Node's output — see [Items](README.md#items).
- **Programs can only be fused 3 times**: every fusion result is one level
  "deeper" than its deepest parent, and a program that's 3 fusions deep is
  a finished product — it can't be an input to another fusion. The pets
  (`p`), inspect (`i`), and Fuse (`f`) screens all show a program's fusion
  depth, flagging a maxed one. This bounds chain-fusing, which previously
  had no ceiling at all beyond your supply of duplicates. Persisted across
  save/load (bumps the save format to **v6** — old saves need a new game)
  — see [Fusing programs](README.md#fusing-programs).
- **The player's level cap is gone**: you now keep leveling forever
  (gaining stats and a Perk Point each time), while tamed programs still
  stop at level 12 as before. Long runs stay worth grinding instead of
  dead-ending at the shared ceiling — see the Stats table.

- **A busier, better-behaved wild population**: the world-wide cap on wild
  programs is up from 24 to 100, so an area you're exploring stays alive
  with things to fight and tame. To keep the simulation cheap, hitting the
  cap now culls the wild program *farthest* from you rather than blocking
  the spawn — a population you wandered away from quietly thins out
  instead of starving the area you're actually in. Programs near you are
  never culled. One caveat: a nest you've left far behind can lose a
  guardian this way, and it won't queue a respawn for it; walk back and
  the usual kill/tame/respawn cycle rebuilds the nest as normal.

---

- **Recharger Node is now a permanent structure**: it no longer collapses
  after 20 ticks — like every other structure, it just needs to be built
  within 15 tiles of your Home. Existing saves need no migration; a
  Recharger Node that was mid-countdown when this shipped simply stops
  decaying — see [Structures](README.md#structures).
- **The Black Market is now the iso Market**: renamed in the same
  structure-tuning pass (`fbd2bed`) that raised its cost. Nothing about how
  it trades changed — same flat sell rate, same stock — see
  [Trading](README.md#trading).
- **Wild creature nests**: Scrapper, Worm, Wraith, and Trojan can now
  spawn as a stationary Nest instead of an ordinary lone creature/pack —
  it keeps 2-5 guardians of its species tethered within 5 tiles, and any
  guardian that's killed or tamed is replaced 10 ticks later. Walk into
  the nest itself to attack it (it never attacks back); destroying it
  frees any surviving guardians to wander normally and stops further
  respawns. New species schema field: `can_nest` — see
  [Nests](README.md#nests) and `assets/species/README.md`.
- **Individual creatures now roll their own stat variance**: every
  creature independently rolls HP/Attack/Defense within ±20% of its
  species/zone-scaled baseline when it's created (wild spawn or fusion
  result), plus its own ±20% roll on its species' growth rate — so no two
  individuals of the same species are quite identical, and some out-level
  their littermates. Shown as a **Potential** tag (Poor through Excellent,
  with a percent) in the pets screen (`p`) and inspect screen (`i`).
  Fusing two programs averages their rolls into the result. Persisted
  across save/load (bumps the save format to v5 — old saves need a new
  game) — see [Companions](README.md#companions).
- **Level capped at 12 for everyone**: the player and every tamed program
  alike now stop leveling — and stop accumulating XP at all — once they
  hit level 12, regardless of source. This sits above the existing
  level-10 cronjob-work cap (work still stops paying XP at 10; battling
  can still carry a worker from 10 up to the new ceiling) — see the Stats
  table and [Getting started](README.md#getting-started-building-and-running-cronjobs).
- **Tamed programs grow faster the higher their species' tier**: a new
  per-species `growth_multiplier` (`assets/species/README.md`) scales a
  tamed program's per-level stat gains — Easy species stay at the
  standard rate, Medium is 1.25x, Hard is 1.5x, and both bosses are 2x —
  so a tougher catch keeps out-leveling an easy one, not just starting
  stronger. Player leveling is unaffected (the player has no species) —
  see [Companions](README.md#companions).
- **Resting now requires a Recharger Node**: `r` (recharge/rest) only
  works while you're within 2 tiles of a deployed Recharger Node (5 Core
  Fragments to build) — there's no other way to rest anymore. The node is
  also **temporary**: it collapses on its own after 20 ordinary
  game-clock ticks with no one resting near it, though actually resting
  near it doesn't burn down that clock any faster than leaving it idle
  would — see [Structures](README.md#structures).
- **GUI default volume lowered to 20%** (was 60%) — still adjustable with
  `[`/`]` in-game.
- **Command Companion picker condensed to a single line**: each entry now
  reads like `[1] Cipher (Rally Team)` — just the companion's name and a
  terse name for its ability — instead of a stats line (HP/ATK/PWR)
  followed by a separate line spelling out the exact numeric effect. Full
  stats are still one keypress away in the pets screen (`p`) — see
  [Companions](README.md#companions).
- **Buffer perk now scales with max Integrity**: each level adds 1% of
  your current max Integrity instead of a flat +10, with a +10 floor so
  it's never worse than before — a meaningful boost once your max HP has
  grown well past its starting value — see [Perks](README.md#perks).
- **README corrected to match the current structure-tuning pass**: the
  structure-cost table, Terminal's passive rate, and a couple of boss
  move/loot footnotes had drifted out of sync with an earlier balance
  commit (`fbd2bed`) that raised most build costs and sped up the
  Terminal. Data Cache, Mining Node, Power Conduit, Compiler, Fabricator,
  Armory, the market, and Shield all cost more Core Fragments than
  documented, and the Zone Portal costs 10 Portal Fragments per zone
  level, not 5 — see [Structures](README.md#structures) and
  [Zones and portals](README.md#zones-and-portals). The Terminal now cooks every
  tick, not every 15 — see [Getting started](README.md#getting-started-building-and-running-cronjobs).
  Overseer was also missing from the Neural Amplifier's loot sources and
  the stun move list, and credited with a "Corrupt" move it doesn't have
  — see [Current roster](README.md#current-roster) and [Equipment](README.md#equipment).
- **The world now ticks in real time while you're out and about**: once a
  second passes, one full game tick advances on its own — structures regen,
  wild programs can spawn, raids can roll — even if you're just standing
  still. This pauses the instant you open any menu (build, inventory,
  trade, ...) and never fires during a battle, so nothing sneaks up on you
  mid-dialog or mid-fight.
- **Structures must be built within 15 tiles of Home, and Home can be
  demolished**: only one Home can exist at a time, and every other
  structure now has to be deployed within 15 tiles of it. The new `R` key
  demolishes a nearby structure for a 30% material refund; demolishing
  Home cascades to demolish the whole base (with a confirmation warning
  first, since it's irreversible) — see [Structures](README.md#structures).
- **Message feed is color-coded by importance**: routine lines stay gray,
  loot/crafting gains are green, leveling up is bold green, and taking raid
  damage on a base structure is orange — in both the GUI and text UI.
- **Graphics is now the default frontend, no more startup prompt**: the
  launcher goes straight into the GUI instead of asking Graphics-or-Text;
  the `--gui`/`--tui`/`--ascii` flags are gone. The text UI still runs
  automatically if no display is available or the GUI crashes, but it's no
  longer user-selectable — see [Playing](README.md#playing).
- **Wild programs scale with distance from your zone's entry point**: on
  top of the existing per-zone doubling, wandering away from where you
  breached in adds up to another 3× to wild stats the farther out you go —
  see [Zones and portals](README.md#zones-and-portals).
- **Gear scaling brought down to 2× per level (was 2.5×)**: gear was
  overtaking zone scaling badly enough that a fully-geared level 1 player
  could trivialize zones 5+, while grinding without gear couldn't keep pace
  past zone 5 at all. 2× keeps gear and levels both mattering together —
  see [Equipment](README.md#equipment).
- **Wild programs can spawn in packs**: bump into one and any packmates
  spawned alongside it join the fight together. Pack size caps at your
  current zone level + 1 (zone 1 → 2, zone 2 → 3, ...), reached gradually
  the farther the encounter is from your zone's entry point — twice the
  distance it takes per-creature stat scaling to grow. Only the lead
  program can be attacked or decompiled at a time, but every packmate still
  alive retaliates each round, and defeating or taming the leader just
  brings the next one forward — see [The loop](README.md#the-loop).

---

- **Cronjob work now caps out at level 10**: a worker stops earning XP from
  structure work once it hits level 10 — resources keep coming, but further
  leveling requires battling.
- **Home is required to build anything else**: the build menu always lists
  Home first, then Mining Node, then Compiler, and nothing else can be
  deployed until a Home exists — see [Playing](README.md#playing). Since zone
  transitions leave structures behind, that means rebuilding a Home first
  in every new zone too.
- **Fuse and cronjob/guard/party pickers show pet status everywhere**: any
  menu listing your compiled programs now flags party membership and
  cronjob/guard assignment on every row, not just some of them.
- **Fuse duplicate equipment**: from the inventory item menu, `[U]` fuses 2
  copies of an item into a permanent +10% (stacking) bonus for that item
  type — see [Equipment](README.md#equipment).
- **Three new perks**: **Attacker** (+1 Attack/level), **Defender** (+1
  Defense/level), and **Buffer** (+10 max Integrity/level, fully healing on
  purchase) — see [Perks](README.md#perks).
- **Graphical frontend added**: a second, windowed UI alongside the
  original terminal one, with automatic fallback to the text UI if no
  display is available — see [Playing](README.md#playing). Menus scroll to keep
  your selection in view instead
  of clipping, size themselves to use most of the screen, and a structure
  with a cronjob worker assigned gets a yellow outline on the map.
- **Companions passively boost your stats**: every active party member adds
  10% (minimum 1) of its own current Attack and Defense to yours, stacking
  across the whole party and updating live as it levels — see
  [Companions](README.md#companions).
- **Low Power weakens your attacks**: below 50% Power your Attack falls off
  linearly, down to half strength at 0 — on top of, not instead of, the
  existing tick damage from fully running out. Commanding a companion in
  battle also now costs a flat chunk of Fatigue — see the Stats table and
  [Companions](README.md#companions).
- **Fuse now lets you name the result**: after picking both programs, type
  an optional name (12 characters max) for what they become — see
  [Companions](README.md#companions). The Fuse and cronjob/guard pickers also now
  show each candidate's full stats (or Power rating) instead of just a
  level.
- **Decompile odds lowered**: ICE Breaker potency and the Decompiler skill
  bonus were both tuned down — weakening a target first now matters a lot
  more than before, rather than skill alone making most attempts a sure
  thing.
- **Command Companion shows what it'll do**: the picker now lists each
  party member's actual ability (its species' special ability, or the
  computed default rally) instead of just its stats.
- Renamed the Daemon species to **SubProcess** throughout.
- **Battles lengthened**: tripled HP across the board — player starting/max
  HP (30 → 90), per-level HP growth (+4 → +12), and every species'
  `base_hp`. Attack/Defense and damage formulas are untouched, so fights
  just take longer, not deadlier.
- **Save system fixed and reworked**: saves now carry a format version and
  are rejected cleanly (instead of crashing) if incompatible. Save slots
  moved from a single `save.bin` to a `saves/` directory; `L` from the main
  menu lists every save with a summary and lets you Load or Delete each
  one. An existing `save.bin` is migrated into `saves/` automatically.
- **Gear now has levels**: every equipped item's bonus scales 150% per
  level above 1, unlocked by reaching the matching zone depth — see
  [Equipment](README.md#equipment).
- **Shield** structure added: passively reduces raid damage against every
  deployed structure, stacking across however many you build — see
  [Structures](README.md#structures) / [Base defense](README.md#base-defense).
- **Perks reworked**: no longer one-time unlocks — each perk can be bought
  repeatedly, with every level adding a flat +1 to its bonus at the same
  Perk Point cost — see [Perks](README.md#perks).
- **Guard assignment** (`G`): post a compiled program to defend any
  structure against raids, without needing a cronjob — see
  [Base defense](README.md#base-defense).
- **Gear crafting reworked**: the Fabricator/Armory no longer run a
  cronjob to grind out gear — building one unlocks compiling that gear
  (Overclock Core / Firewall Plating) for Portal Fragments instead — see
  [Equipment](README.md#equipment).
- **Companions buff instead of attacking**: commanding a companion in
  battle now grants the player a buff (a rally by default, or a species'
  own special ability) rather than dealing damage directly — see
  [Companions](README.md#companions).
- **Mining is harder**: Mining Node cronjobs take twice as long per cycle
  and gate the payout behind a level-based success chance instead of
  always yielding — see [Getting started](README.md#getting-started-building-and-running-cronjobs).
- **Power rating** added throughout the UI (status panel, pets screen,
  battle screen, inspect) — a rough overall-strength number (max
  HP + Attack + Defense) alongside the individual stats.
