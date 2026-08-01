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
