# Changelog

Release notes for [feral-processes](README.md).

Versions follow [semantic versioning](https://semver.org). While the project
is `0.x`, a **breaking** change bumps the minor version and a compatible one
bumps the patch. For a single-player game with no public API, "breaking"
means one thing above all: **a save-format bump**, where existing saves stop
loading (see `save::SAVE_FORMAT_VERSION`). Every crate in the workspace
shares one version, set in the root `Cargo.toml`.

Dated entries below `0.2.0` predate versioning and are kept as written.

## Unreleased

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
  follows names your choice — `Pick a target (Heal)` rather than a bare
  prompt. Backing out with Esc steps back one screen at a time. A planned
  Special also reads on the roster as the ability it will spend (`Heal -> A`)
  instead of the generic word.
- **Buffs and heals can be aimed at any party member.** A Rally, Shield or
  Heal now lists your own side — you and every standing companion — instead
  of always landing on you. A debuff still picks an enemy group. Companions
  could never actually hold a buff before this: only the player is spawned
  with a buff slot, so one aimed elsewhere would have changed nothing.
- **Species can define more than one special ability.** `special_ability`
  becomes `special_abilities`, a list — see `assets/species/README.md`.
  Modders: rename the field and wrap the value in `[]`. A file still using
  the old singular name keeps loading, but its ability is ignored and the
  companion falls back to the generic rally. No shipped species declared one,
  so nothing in the base game changes yet.
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
- **Battle keys are lowercase, and Decompile moved to `c`.** Defend takes `d`,
  so the per-slot keys `a`/`d` and their party-wide counterparts `A`/`D` line
  up: shift means "everyone does this". Nothing sits one shift key away from a
  different action.

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

- **The base platform shrank from a 15-tile radius to 7.** The platform edge
  is also where the danger curve starts measuring, so hostiles now get tougher
  8 tiles nearer to home — the first stat-escalation step moves from 30 tiles
  out to 22.

## 0.2.0 — 2026-07-24

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

## 2026-07-24

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

## 2026-07-23

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

## 2026-07-22

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

## 2026-07-21

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

## 2026-07-20

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

## 2026-07-19

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
