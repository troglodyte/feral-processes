# Load-bearing seams

The long form. `CLAUDE.md` carries the rule and the trap for each of these
in a line or two; this file carries the argument, the measurement and the
history behind it — which is the half that actually stops someone
"fixing" a seam back into the bug it was built to close.

**Read the matching entry here before changing a seam `CLAUDE.md` names.**
A rule with its reasoning stripped off reads like an arbitrary constraint,
and an arbitrary constraint is what people delete.

Facts that cost tool calls to rediscover every session. Each was verified
against the source, not remembered — verify again before relying on one, and
correct it here if it has moved.

### The player's `Position` stays on the surface while they are underground

**The player's `Position` stays on the surface while they are underground.**
Stack coordinates and facing live in `resources::Locale`, and `Position`
is pinned to the entrance tile the party walked in through. This is
deliberate and load-bearing: `Position` is the space structures, wild
programs, nests, cronjob targets, raid pathing and the build radius all
share, so Stack coordinates in it would silently point every one of
those systems at the wrong tile. The payoff is that nothing on the surface
knows the Stack exists, and the base keeps running while you are four
frames down. `Game::require_surface` and `Game::require_base` between them
guard the eleven actions that reach into a map through `Position` — see
the next entry for which is which, and why that is two guards rather than
one.
**A read-only screen can fall into the same hole**, which is why
`find_target_in_direction` (`game/inspection.rs`) finds nothing at all
underground — structures and creatures alike. It takes no action and moves
nothing, so `require_surface` does not apply and never would have caught
it; it simply *reports*, and what it would report is your base lying off to
the east, or a wild program four frames overhead lying "that way", while
you stand in a corridor. The test for whether a `Position` reader needs the
guard is therefore not "does it act" but "does it claim something about
where the party is": contrast `maybe_spawn_wild_creature`, which reads the
same pinned tile and only places things, and `nest_aggro_tick` below, which
reads it and drags you into a fight. Underground, `x` routes to
`Game::describe_view_direction` instead — a claim about the frame the party
is actually in.

### `require_surface` used to mean "not in the Stack", and ten of its eleven callers turned out to mean "in the base"

**`Game::require_surface` used to mean "not in the Stack", and ten of its
eleven callers turned out to mean "in the base".** While there were two
locales, "not underground" and "on the surface proper" were the same
condition, so no site had ever had to say which it meant. `Locale::Base`
made them two conditions and forced eleven independent re-readings — and
**a wrong one is silent**: nothing fails to compile, and both guards refuse
in the Stack, so the Stack's own tests cannot tell them apart either. What
each site settled on:

| Site | Guard | Why |
| --- | --- | --- |
| `game/base/building.rs` `place_structure` | `require_base` | deploys a `Structure`, and every `Structure` is in base space |
| `game/base/building.rs` `upgrade_structure` | `require_base` | same, on one already standing |
| `game/base/building.rs` `remove_structure` | `require_base` | same |
| `game/base/building.rs` `work_structure` | `require_base` | stands the player at a machine's station tile |
| `game/base/work_orders.rs` `queue_work_order` | `require_base` | reads which machines the base has standing |
| `game/base/transfer.rs` `transfer_offer` | `require_base` | reads the buffers of the machines around you, and the pack against them |
| `game/base/transfer.rs` `transfer_items` | `require_base` | moves a basket in both directions between the two |
| `game/base/transfer.rs` `refuse_transfer` | `require_base` | says why there was nothing to move |
| `game/trade.rs` `sell_item` | `require_base` | a trader is a deployed `Structure` |
| `game/trade.rs` `buy_back` | `require_base` | same, and its shelf is keyed on the trader's tile |
| `game/trade.rs` `sell_companion` | `require_base` | same |
| `game/trade.rs` `buy_item` | `require_base` | same |
| `game/turn.rs` `rest` | *none* | takes no guard at all now — it **reads** the locale to price itself, see below |

The test for whether a reader needs a guard at all is unchanged — not
"does it act" but "does it claim something about where the party is". What
is new is that answering yes no longer picks the guard for you. **Adding a
`require_surface` caller now asserts the action belongs to the wild**, not
merely that it is not a Stack action; if what it touches is a `Structure`,
it wants `require_base`.

`Game::is_underground` stays strictly Stack-only through all of this,
deliberately: base space is off the surface too, and widening that
predicate would apply every Stack rule in the game — no Power supply,
`Trace`, the frame view — to the base. The two systems that guard on it
rather than on a `require_*` therefore ask *two* questions now
(`is_underground() || in_base()`); see `nest_aggro_tick`'s entry below.
`power_regen_system` used to match `Locale::Surface` positively rather than
asking `is_underground`, so it refused base space the moment the variant
existed — luck rather than design, and it went on to leave the Recharger
Node's whole purpose unreachable in real play once every `Structure` moved
into base-space coordinates and no Recharger could stand anywhere else.
The final review caught it: the system now measures against
`Game::base_pos` while in base space, the same dispatch `Game::scan_center`
makes, and refuses only the Stack — no supply underground stays a rule
of its own, not a side effect of the surface-only guard.

**`rest` was the contested row twice over, and has now left the table.** It
first kept `require_surface`, on the spec's authority
(`docs/superpowers/specs/2026-08-19-base-out-of-phase-design.md`), while its
own code argued the other way: it demanded a structure whose def set
`enables_rest` within reach of the player's base cell, Home was the only
shipped one, and Home stands in base space. Once the base's structures moved
into base-space coordinates, the only tile from which `nearby_rest_structure`
could legitimately succeed was the locale the surface guard refused — and the
only way it could have succeeded on the surface at all was the
coordinate-space collision between a base-space structure and a surface
`Position`, exactly the class of bug the base guard exists to close. Task 6
made the one-line change to `require_base`.

Resting is no longer gated by locale at all. It is **priced** by one:
free inside base space, one unit of an item whose def sets
`ItemDef::enables_rest` anywhere else, the open grid and the Stack alike.
`require_base` came out with the structure requirement, and
`nearby_rest_structure`, `rest_cost` and `RestDef` came out with it — nothing
about a rest asks where a building stands any more.

**The trap is that `rest` now reads as an unguarded `Position`-adjacent
action, and it is not one.** A guard added back here does not tighten
anything; it deletes the field half of the mechanic outright, and it deletes
it silently, because the base half goes on working and is the half anyone
testing by hand reaches first. The row above is deliberately kept in the
table, spelling out *none*, rather than dropped from it: an action missing
from a guard table reads as an oversight, and an oversight is what gets
"fixed".

**The second half of the same change is that no rest advances the clock**,
and the two halves hold each other up. A free rest that ticked could be
spammed at the base to farm production, raid pressure, need decay and
`Temporary` wear; a priced rest that ticked was the only bulk time source in
the game, so removing the price without removing the ticks would have made
walking home a production exploit. `Game::wait` is now the only way time
passes without an action, one tick at a time. `REST_TICKS` is gone rather
than set to zero, so there is no constant left for someone to "restore".

Three consequences worth knowing before touching this. A counted field buff
now comes out of a rest **unaged** — the until-rest drop is a thing `rest`
does on purpose, not a side effect of time passing, which is what
`resting_drops_until_rest_buffs_and_leaves_counted_ones_aged` asserts. A rest
can no longer be interrupted anywhere, since there are no ticks for
`nest_aggro_tick` to open a battle on, so the refund path a half-paid rest
needed went with it: the two gates and the payment run in that order and the
restore is unconditional from there. And a Stack run is now bounded by
charges carried rather than by the Power reserve alone —
`power_regen_system`'s underground guard still holds (a Recharger cannot
reach four frames down), but the scarcity it protects is purchasable at an
outlet a heal.

### Examine names only what the surface map draws, and that rule is one function

**Examine names only what the surface map draws, and that rule is one
function.** `views::drawn_on_surface_map` is it: everything untamed is
drawn, a tamed program only while its `Position` is one the sim keeps
honest. That parameter was "is this worker away from its post" until work
orders arrived on 2026-08-14 and gave a *second* kind of tamed program a
true tile — an idle base staff member, which `schedule_base_labour` parks
on a ring around the Home every tick. `EntityView::position_is_honest` is
the wider value; `worker_away_from_post` stayed narrow beside it because
the gui's "someone is on this job" mark reads *that* one, and an idle
program is on no job. A party companion still has neither and is still
not drawn. `render/base.rs`
filters the map with it and `Game::find_target_in_direction` filters its
ray with it, so the set you can see and the set `x` can name are the same
one. They disagreed until 2026-08-13, and the case that exposed it is the
one every base has: a posted `GatherResource` worker stands *orthogonally
beside* its machine (`hauling::at_station`), and at its post it is not
drawn — so aiming at a machine resolved on an invisible program one tile
in front of it while the machine's own glyph sat under the cursor. Two
consequences follow. A posted program is unreachable from the map by
design, which is why `views::Assignee` carries its level and HP: the
structure's sheet is the only screen that can report on it. And the
scan is *a ray one tile wide* — it was a 90° cone at `MENU_SCAN_RADIUS`
(40 tiles, against a map pane of roughly 16x9), so an eastward press
could name a program forty tiles east *and* forty north. The reach is
now `tuning::EXAMINE_RANGE_TILES`, an engine constant rather than a
borrowed menu window, and the tie between two things on one tile is
decided by named `STRUCTURE_ON_TILE`/`CREATURE_ON_TILE` ranks folded into
a *total* `(step, kind, entity)` order — `min_by_key` returns the first of
several equal minima, which is exactly where bevy's unstable iteration
order leaks in. What the ray does *not* see is nests, surface links and
zone portals: all three draw a glyph, none carries `Creature` or
`Structure`, so aiming at one reports whatever lies beyond it. That is a
known gap, recorded in `TODO.md`, and it is the one place the "names what
it draws" rule above does not yet hold in both directions.

**There is a second rule beside this one, and it took three bug reports
to see that neither subsumes the other.** `drawn_on_surface_map` answers
whether a program's `Position` is a tile the sim keeps up to date. It does
*not* answer which coordinate space that tile is a tile of — see **The map
draws one space, and `stands_in_base_space` is which** — and reading the
first as if it were the second is what put the base's whole roster out on
the open grid. The two are asked together, in that order, at both
readers.

### `use_symlink` is the one action that leaves the Stack instead of being refused by it

**`use_symlink` is the one action that leaves the Stack instead of being
refused by it.** It used to be `require_surface`'s twelfth caller, and the
dangerous one, since underground `Position` *is* the entrance you climb
back out of. It now calls `clear_stack` and then teleports — so it is
not an exception to the rule above but an application of it, and the
ordering is the whole argument: the locale drops only after every check
has passed, so a refused symlink can neither strand the party on the
surface nor write `Position` while `Locale::Stack` is live. Anything
else that ever needs to move the player out of the Stack goes through
`clear_stack` in that order, not around it.

### A Forgiving death is the second thing that leaves the Stack, and the only one that isn't an action

**A Forgiving death is the second thing that leaves the Stack, and the
only one that isn't an action.** `difficulty::death_handling_system`
warps the player to their nearest structure, and that structure is on the
surface — so it wrote a surface tile into a `Position` still pinned to the
entrance, leaving the party rebooted-on-paper and still four frames down
with their way out overwritten. It cannot call `clear_stack`: it is a bevy
system with queries, not a `Game`. So the reset itself moved into
`game::stack::surfaced`, which returns the three resources leaving the
Stack means, and both sides apply it — `Game::clear_stack` by inserting
them, `StackLocale::surface` (a `SystemParam` gathering the same three) by
assignment. A fourth resource added to that tuple fails to compile at both
call sites, which is the whole reason the values are returned rather than
written. Same ordering rule as `use_symlink`: the locale drops *before*
the warp writes `Position`.

### Beating a stack's guardian is the third thing that leaves it, and the only one that takes the way back out with it

**Beating a stack's guardian is the third thing that leaves it, and the
only one that takes the way back out with it.** `Game::collapse_stack`
ejects the party, despawns the `SurfaceLink` at the entrance, drops every
`StackMemory` entry keyed to that tile, and opens a replacement link on
the nearest legal ground. Four things about it are load-bearing.
The trigger is a **field**, `BattleState::lair` (`resources::LairFight`),
written by `rouse_lair` as the fight opens and read by `end_battle` — not
a positional "is the party standing on a cleared lair" read at teardown,
because a Forgiving reboot can land between the kill and the end of the
fight (the guardian falls, its escort flatlines the player,
`death_handling_system` surfaces them in the trailing `tick`) and there
would be no `Locale::Stack` left to name the entrance. It costs no
`SAVE_FORMAT_VERSION` bump for `decompile_attempts`' reason: battles are
never serialised.
**That field names the guardian, and a lair fight is mostly not
guardian.** `award_loot` fires for every hostile in the game that goes
down and `spawn_pack` gives a boss an escort past the first frame, so
`mark_lair_cleared` reading only "did something die while the party stood
on a Lair cell" spent the lair — and collapsed the whole stack on the way
out — for a party that cut down one escort and jacked out with the
guardian untouched. Fixed 2026-08-15 by passing the victim down and
matching it against `LairFight::guardian`, the head of the pack
`rouse_lair` spawned. Note what the rule is *not*: the collapse is keyed
on the guardian dying, not on the fight being won, so killing the
guardian and running from its escort still finishes the stack. Both
directions are pinned, and the escort half was mutation-checked.
With the frame now recorded at rouse time there is one record rather than
two — `end_battle` asks `lair_cleared` back off `FrameMemory`, which it
could not do while the entrance came from a `Locale` the reboot had
already dropped.
**Taming the guardian is refused rather than handled, and the record is
written anyway.** Taming spends no `award_loot`, so a capture cleared
nothing: the lair refilled on the next visit over a stack that could never
be finished, and the guardian could be farmed. Closed 2026-08-15 at both
ends. `battle_set_action` refuses a guardian as a decompile target beside
the boss it already refused — and since 2026-08-18 every guardian *is* a
boss, drawn from the danger window and marked one even when the draw came
out of the ordinary pool, so the guardian clause now catches nothing the
boss clause would have missed. It stays because the two are different
statements: "this is a boss" and "this is what the stack is guarding" are
read from different places, and a mod that widens either must not be able
to open a gap between them. And
`attempt_decompile` calls `mark_lair_cleared` regardless, which is now
unreachable and kept deliberately: the record is what the collapse reads,
and a third way out of a fight should not have to remember to write it.
`Game::lair_guarded_by` is the one statement of "this is the guardian" that
both sides ask, because the two disagreeing is exactly a stack the player
may empty and never finish.
The **replacement site is found before the old link comes down**. A zone
with no link left is a run that can never breach again — `award_loot`
underground is the game's only source of Portal Fragments — so a search
that finds nowhere legal inside `STACK_COLLAPSE_RELINK_TILES` skips the
collapse entirely and leaves the stack standing. That is also the one
branch where `FrameMemory::cleared` still has work to do: everywhere else
the stack ceases to exist before the record could be read again.
The trade keeps the sector's link count flat, so nothing downstream has to
reason about a zone running dry, and the replacement's depth comes free
from `frames_at`.
`Game::link_site_free` is the single statement of what ground a link needs
and `stack::ring_offset` the single definition of a Chebyshev ring; the
zone scatter and the replacement both read them rather than each carrying
a copy of the walkable/Platform/nest/structure filter.

### World generation must not draw from `resources::GameRng`

**World generation must not draw from `resources::GameRng`.** Stack
frames (`stack::generate`), entrance placement
(`Game::spawn_surface_links`) and which program guards a stack
(`Game::pick_lair_species`) each seed a local `StdRng`. Two reasons, both
learned the hard way: `GameRng`'s stream position is not persisted, so a
level drawn from it would regenerate differently after a save/load and
strand the party inside rock; and drawing from the shared stream shifts
every later roll in the run, which silently rewrote the outcome of a
seeded combat test three files away. `FrameSpec::rng_seed` is the one
scheme to salt off — don't invent a second that could collide with it.

### A Stack frame is regenerated; what the party *saw* of it is saved

**A Stack frame is regenerated; what the party *saw* of it is saved.**
`stack::generate` is a pure function of `FrameSpec`, so nothing about
the maze goes in the save. `resources::StackMemory` does: mapped cells,
emptied caches, opened seals, cleared lairs, fight sites — that is the
run's history, not the world's shape, and no seed can hand it back. It is
keyed by `(link tile, depth)` and is zone-local, so like `BuybackLedger`
it has to be wiped **by name** in `enter_next_zone`.

### `view_cone` is the one walk both Stack views are built from

**`view_cone` is the one walk both Stack views are built from.**
The first-person view and the map's record of what has been seen call it,
so the map cannot mark a cell the view never showed. Sight stops at
`CellKind::blocks_sight` — rock, and any door, since a shut door is the
point of a door. **Never at `ahead == 0`**, though: that row is the cell
the party is standing in, and a cell cannot hide the party from their own
surroundings. Both consumers carry that exception explicitly —
`remember_view` and `render/stack.rs::draws_as_face`.
It is `fn`, not `pub(crate) fn`, and stays that way: `game/stack_view.rs`
holds it and both its consumers, so "one walk" is enforced by the module
boundary rather than by everyone remembering. A third consumer that needs
it widened is the signal to re-read this entry, not to add `pub(crate)`.
**`visible_rows` gained a third consumer in 0.9.0 and the boundary held**
— `announce_passage` lives in `game/stack_view.rs` beside the other two
rather than the walk being widened, which is what that last sentence was
asking for.

### A Stack cell is narrated on two axes, and which one a caller is on is decided by whether the party *found* something or *walked* somewhere

**A Stack cell is narrated on two axes, and which one a caller is on is
decided by whether the party *found* something or *walked* somewhere.**
`announce_sighting` fires on discovery: the first time a cell comes into
view, once ever, from a turn as readily as from a step, and only for a
cell `Game::notability` ranks. `announce_passage` fires from `Game::arrive`
on arrival, has no notion of new, and describes whatever the party's line
of sight resolves to ahead. Four things about the pair are load-bearing.
**`notability` no longer decides whether a cell has anything to say**,
only whether finding it is news — floor and doors are unranked and are
narrated by the passage axis, whose whole reason for existing is that
their authored `sighted` prose was unreachable in play. A consequence
worth knowing before trimming the bank: `tests::descriptions`'s census
used to be deliberately broader than what could reach the screen, and is
not any more — every `sighted` pool it checks is now live, because
`ahead_target` falls back to the nearest *walkable* cell and every one of
those subjects is walkable.
**Both axes resolve the cell through `ahead_target`**, which is
`describe_view_direction`'s own dead-ahead pick, so the corridor cannot
announce a cache that `x` then declines to name. The test for that
deliberately does not compare the two expressions — one *is* the other,
which is a tautology that passes with either side rewritten; it finds a
vantage whose ray crosses plain floor before reaching a feature and holds
both to naming the feature.
**Whether a cell speaks is derived, like what it says** —
`Game::narrates_passage` folds `PASSAGE_SALT` with the cell coordinates
off `FrameSpec::salted`, never `GameRng`, for the three reasons every
other description decision does it: a draw would not survive a reload,
it would shift every later roll in the run, and the corridor would keep
a different rhythm every time it was walked. A *separate* salt from
`DESCRIPTION_SALT` on purpose — one salt for both would tie "does this
speak" to "what does it say", which is `Slot::tags`' argument one level
up.
**The wall guard is not defensive coding.** `ahead_target` answers with
the party's own cell when the ray holds nothing walkable, which is the
right answer for `x` ("describe the corridor I am in") and the wrong one
here — `fill_bearing` renders that "right under you", so the line would
claim the corridor runs on through the party's feet. That also makes a
"a blocked step narrates nothing" test vacuous by construction, since a
step is blocked by rock dead ahead and this guard fires either way; the
turn test is the one that actually pins the trigger.

### `walkable()` and `blocks_sight()` are not complements, and were until doors landed

**`walkable()` and `blocks_sight()` are not complements, and were until
doors landed.** Before them, everything that stopped the view was rock and
rock could not be stood on, so "the party is inside an occluder" was
unreachable and both cone consumers quietly assumed it. A door is both
walkable and sight-blocking, and standing in one filled the first-person
view with flat door and truncated the map to the party's own row. Any new
cell kind that is walkable *and* blocks sight inherits that trap.
Phase 3's `Breakpoint`, `Fault` and `Corruption` deliberately don't, and
neither does phase 4's `Orphan` — all four are walkable and see-through,
pinned by `the_new_cell_kinds_are_walkable_and_see_through`. A fifth that
needs to block sight is the signal to re-read this entry, not to just add
it.

### The renderer's first-person cell marks were a `_ => None` match, and that is why a new `CellKind` could ship invisible

**The renderer's first-person cell marks were a `_ => None` match, and
that is why a new `CellKind` could ship invisible.** `render/stack.rs`'s
`cell_mark` is now exhaustive: a new `StackCellView` will not compile
until someone decides what it looks like down a corridor. Before that, a
variant compiled clean and drew as bare floor — the party would have
walked into corruption with nothing on screen to warn them. Keep it
exhaustive; the wildcard is what made the gap silent.
**The exhaustive match is not by itself enough, and doors are how that
was learned.** They had an arm — `None`, because a door is drawn as a
*face* filling its slice rather than as floor, so `face_color` was doing
the telling. A colour is exactly what the fog eats: measured, a door
three cells off drew as rgb(33, 26, 11) against rock's rgb(13, 38, 38),
and the player found out a corridor ended in a door by walking into it.
So the table is no longer about floors — the caller places one mark on
the floor of an open cell and in the middle of a face otherwise, chosen
by the same `draws_as_face` that decided the geometry. A new variant that
is `solid()` needs a mark for the same reason rock doesn't have one: rock
is the default reading of a blocked corridor, and everything else has to
say so. Marks also fade on their own `MARK_FOG` rather than on `FOG` —
the geometry's fog is a depth cue, and applying it to the layer whose
whole job is to be spotted from the far end of the view is what put the
glyph at 24% brightness on near-black.

### The frame map is drawn twice and defined once

**The frame map is drawn twice and defined once.** `render/frame_map.rs`
has two entry points — `draw_frame_map`, the full screen on `g`, and
`draw_map_inset`, the always-on corner of the first-person pane — and they
differ only in where the grid goes and what text surrounds it. Everything
a cell *is* lives in `draw_grid`, `tile_color` and `cell_glyph`, which both
call. A third caller widens `layout`'s `fill` parameter; it does not get
its own copy of the glyph table, which is the one screen where drift
silently misinforms rather than merely looking wrong. `render/stack.rs`'s
`cell_mark` is the deliberate second table, for the corridor rather than
the map, and its doc comment records which glyphs it holds in step.

### A sealed door is `walkable()`

**A sealed door is `walkable()`.** The generator has to see through it:
connectivity, dead-end detection and link placement would otherwise
treat a whole sealed wing as unreachable and strand the frame's furthest
cell. Whether the party may actually pass is decided in `Game::step`
against `FrameMemory::opened`, nowhere else.

### `Game::apply_damage` (`game/combat_damage.rs`) is the only code path that lowers a creature's HP

**`Game::apply_damage` (`game/combat_damage.rs`) is the only code path that
*damages* a creature.** Every other write to `Stats::hp` is a heal, one
of the two full-heals (`rest` in `game/turn.rs`, level-up in
`game/unlocks.rs`), or `needs_tick_system`, which is `With<Player>`. Put a
check that must see all damage here, not at the call sites. Every rung of the
fumble ladder goes through it, which is exactly the kind of thing someone
would otherwise write a direct `Stats::hp` write for.

**There is one other thing that lowers HP, and its shape is the point.**
`Game::kill_outright` exists because mitigation reaching the damage path made
`apply_damage(player, hp)` non-lethal — the player's innate 2% leaves a point
behind, after a line promising that materialising inside solid substrate is
not survivable. It is spelled as its own verb rather than as a large `dmg`
precisely so it cannot become a general mitigation bypass: there is no amount
to pass, so nothing can reach for it to make an ordinary hit hurt more. Both
it and `apply_damage` funnel through one private `lower_hp`, so "one place
lowers HP" survives the second door and the death check cannot be missed by
either.

**`apply_damage` returns what actually landed.** Mitigation is applied inside
it, so the figure differs from the one asked for against any defender with
any at all — and callers use the return for their log line and, in `Drain`'s
case, for the heal. This is the same trap `restore_hp` already closes from
the other side: printing the requested number lets a heal claim twenty points
on a target with three to spare, and lets a swing claim damage the target
never took.

### The save is field-named RON, and that is what retired save migrations

**The save is field-named RON, and that is what retired save
migrations.** Every `SAVE_FORMAT_VERSION` bump from v19 to v28 was a
struct *gaining* a field — nine in a row, no removals, no changed
meanings — and bincode being positional is the only reason any of them
broke a player's save. Since v29 the payload is the same RON
`savetool dump` prints, so a field added behind `#[serde(default)]`
loads out of a file written before it existed: **an additive change now
costs no version bump, no migration code and no tool.** What still earns
a bump is a field *removed*, or one whose meaning changes under a name
it keeps, and that needs real migration code no encoding could have
saved you from. Two details are load-bearing. The version is the file's
**first line** rather than a field, so a file this build cannot read is
refused *by version* — a sentence a player can act on — instead of by a
parse error about a byte offset; `OLD_FORMAT_REFUSAL` covers the two
ways a pre-v29 binary save fails, since one situation must not produce
two different sentences. And `#[serde(default)]` on a new field is no
longer merely for the RON round trip, as the older field docs in
`save.rs` still say — it is now the whole compatibility story, so leaving
it off is what forces the next needless bump.
**A positional tuple is the one shape this does not save you from**, and
gear rarity is where that was measured. RON parses a `(` in a struct
position as the start of *named* fields, so a `Vec<(A, B, C)>` cannot be
widened and cannot be converted to a named struct with defaulted trailing
fields either — it raises `ExpectedIdentifier` at the first element
rather than falling through to serde's `visit_seq`. Two fields were in
that shape (`PlayerSave::fused_gear`, `SaveData::buyback`); both keep
their type, became read-only and `skip_serializing_if` empty, and are
drained on load into named successors. Nothing sets
`deny_unknown_fields`, so both can be deleted together in a later release
with no bump. The moral for a new field: **prefer a named struct to a
tuple**, or the next property costs a legacy field.
`a_pre_rarity_templates_fused_gear_survives_the_load` is the real-file
gate — it loads a checked-in v29 `dev-saves/` template rather than a
hand-written string, and is what would catch the drain being dropped in a
later tidy-up.

### Destroying a tamed program has two paths, not one

**Destroying a tamed program has two paths, not one.**
`dissolve_tamed_program` (`game/trade.rs`) handles sale, extraction,
battle death and a raid defender's death — it strips the program's gear
back to cargo, logs detachments, drops the program from `Party`, strips
its `Task`, and despawns. But `fuse_companions` (`game/party.rs`) does its
own `Party::retain` and `despawn` inline and skips the detachment logging,
so fusing a program off a cronjob goes silent where selling the same
program announces it. It does *not* skip the gear strip, because gear is
the player's property rather than the program's and losing it to a fusion
would be a real loss — it carries its own `strip_gear` call, for the
ordering reason in the next entry. Know which you are extending before
adding a third.

### No stats operation may run while a gear bonus is sitting in `Stats`

**No stats operation may run while a gear bonus is sitting in `Stats`.**
Gear bonuses are written straight into `Stats` by
`apply_equipment_delta`, and since 0.8.0 any program the player owns can
wear them. Three operations read a program's `Stats` and would scale or
bank whatever they find: `refactor.rs::refactored` **multiplies**
(`*= ZoneLevel::tier_step`, `raised(x, percent)`), `fuse_companions`'s
`fuse_stat` combines both parents' numbers into a new entity's, and
`program_payout` prices a program off `Stats::power()`. In every case the
later unequip subtracts only the *unscaled* bonus, welding the difference
permanently into the program's base stats with no record of where it came
from — which is `EquippedItem::fusion_tier`'s trap reached by a new route.
`Game::gear_bonus` is the single definition of what a wearer's gear is
worth and `Game::strip_gear` returns the lot to cargo; four sites read
them and each takes a different shape for a stated reason.
`refactor_companion` **lifts and replaces** rather than stripping, because
the program survives so its gear stays on and the recorded `EquippedItem`
makes the add-back exact. `dissolve_tamed_program` strips at the top,
which is what covers all four of its callers at once.
`fuse_companions` strips both parents **before** the `Stats` snapshot, and
that ordering is the whole argument. `sell_companion` strips explicitly
*as well*, because `program_payout` runs before the dissolve — after every
reachable refusal, so a refused sale leaves the loadout alone.
`strip_gear` is deliberately not three `unequip` calls: `unequip` refuses
during a battle and calls `tick()`, and a companion dying mid-battle is
precisely when this runs. Both orderings are pinned by tests that were
mutation-checked rather than merely written.

### `Trace` is a resource because `descend_to`/`ascend_to` rebuild the `Locale::Stack` variant

**`Trace` is a resource because `descend_to`/`ascend_to` rebuild the
`Locale::Stack` variant.** It reads like a field on that variant — it is
entirely about where you are and how the place regards you — and it was
specced as one. Both frame transitions *construct* a fresh variant rather
than mutating the live one, so a field there is silently zeroed on every
descent, which is exactly when the meter should be accumulating. As a
resource it survives frame changes for free and resets wherever the party
surfaces — which is one *value*, `stack::surfaced`, applied by
`Game::clear_stack` and by `StackLocale::surface` for the Forgiving death
that has no `Game` to call it on (see that entry above). Every exit is
covered because none of them names the three resources itself.
`Game::raise_trace` is the only thing that raises it, and holds the
`is_underground` guard for all three sources, because `award_loot` fires
for every kill in the game and almost all of those are on the surface.

### Distance from home decides exactly one thing, and it is not difficulty

**Distance from home decides exactly one thing, and it is not difficulty.**
`Game::distance_from_danger_origin` has a single consumer,
`in_opening_ring`. It used to feed a stat multiplier (up to 3x) and the
group-size curve, and that was removed on 2026-08-05 for two reasons: a
zone had no consistent difficulty of its own, and it leaked underground —
every Stack spawn is placed at the *surface entrance tile*, so descending
through a far-flung link scaled the whole frame by that link's distance.
`danger_steps` is still the one input both group curves read, so they
cannot disagree. It takes the zone step on the surface, and the zone step
**plus** the depth step in the Stack. Depth used to *replace* the zone
underground, on the argument that a stack should escalate by how far down
it goes rather than inheriting whatever its entrance sat at. That was
wrong in play: a depth-1 frame is step 0 in every zone, so the first frame
of a zone-9 stack fielded a single program exactly as zone 1 did, and the
zone the player had spent a Portal reaching bought them nothing until they
were several frames down. Summing keeps both commitments visible and keeps
the curve linear. A new difficulty knob keyed to where the party is
*standing* still reintroduces both of the 2026-08-05 bugs.

### Every difficulty curve in the game is linear

> **Retired as a *correctness* property on 2026-08-19**, when the combat
> model replaced the subtractive damage floor with a percentage cut capped
> below immunity (`tuning::MAX_MITIGATION_PERCENT`). The argument below turned
> on `compute_damage` flooring every swing at 1 once enemy DEF passed your
> ATK, so that a compounding curve did not merely get hard, it stopped
> responding to your stats at all. That failure mode no longer exists: a
> percentage cut always leaves damage proportional to what you deal, and
> `HIT_CHANCE_MIN` keeps expected damage strictly positive besides.
>
> **Do not restore the old reading, and do not delete the rule either.** What
> survives is everything except the word "correctness": a geometric enemy
> curve racing a linear player curve still outruns it wherever the
> coefficients are put, the tier step is still a ratio, and `balance_sim`
> still bounds per-zone *steps* rather than ratios for the reason given
> below. The rest of this entry is kept as written because the measurements
> in it are the record of why the curves were linearised at all.

**Every difficulty curve in the game is linear.** `ZONE_STAT_STEP`,
`STACK_DEPTH_STAT_STEP` and `GEAR_LEVEL_STEP` all *add* per level; they
used to multiply (x2 per zone, x1.35 per frame, x2 per gear level). The
player's side of the fight has only ever been linear — `ATK_PER_LEVEL` is
1, an item is worth a flat point or four — so a geometric enemy curve is a
geometric quantity racing a linear one, which has an end wherever the
coefficients are put. Under the subtractive `power + atk - def` rule of the
time, floored at one point, past that end every swing landed on 1 and the
fight stopped responding to levels, gear or roster at all — see the note
above for why that half no longer applies. Measured before the change: a zone-3 depth-5 lair guardian was
unbeatable at level 90 in the best gear the game ships, and the level a
zone demanded ran 1, 15, 30, 63, 131 — doubling, so zone 6 wanted more
than any reachable level. It now runs 1, 15, 24, 32, 47, 61, 76, 90, 106,
121 out to zone 10. Gear tracks the zone curve deliberately, as it did the
old one: geometric gear against linear zones inverts the bug rather than
fixing it. Two consequences that are easy to undo by accident. A linear
curve's tier step is a *ratio* (3/2 from tier 2), so
`ZoneLevel::raised_a_tier` applies it rather than returning an `i32` that
truncates to 1 and silently makes a Recompile Kernel a no-op. And
`balance_sim` did not catch the original bug and could not have — its
guards bound the *ratio* between consecutive zones from above, so any
compounding curve with a small enough base passes. It now sweeps to zone
10, which a geometric curve cannot reach inside `MAX_LEVEL_SEARCHED`, and
asserts the per-zone *steps* stay flat, which is the property that tells a
curve that ends from one that does not.

### One draw, four bands: `battle::resolve_attack` is how every creature-versus-creature attack resolves

**A single `r` in `[0, 1)` decides the whole outcome**, banded in a fixed
order: crit (clamped to at most the hit chance), hit, fumble (clamped to at
most `1 - hit chance`), miss. One draw rather than three is not a
micro-optimisation. It bounds how far the RNG stream shifts per swing, which
is what let 2000-odd seeded tests survive the change at all; and it makes
crit and fumble mutually exclusive *by construction* rather than by a check
somebody can drop. `crit_and_fumble_are_mutually_exclusive_by_construction`
sweeps the unit interval to say so exhaustively rather than by sampling.

**`hit_chance` is the ratio form `k*acc / (k*acc + eva)`, and a difference
form must not replace it.** The ratio is scale-free: doubling both sides
leaves it where it was, so a zone that multiplies everything by its tier
multiplier changes no hit rate anywhere, and the geometric-versus-linear
hazard that shaped every other curve in the game cannot reappear on this axis
at all. `base + k * (acc - eva)` makes hit rate depend on absolute scale, so
deep zones drift silently toward always-hit or always-miss. Two combatants
with *nothing* — a mod species authoring `base_speed: 0` at level 1 — get an
even matchup rather than a divide by zero, and that arm resolves *as* parity
rather than restating a figure, so it tracks `k` instead of drifting from it.

**`k` is `ATTACKER_ACCURACY_ADVANTAGE`, and it is the whole reason the parity
baseline is not 0.5.** It was 0.5 until 2026-08-25, deliberately — an even
matchup, the number every constant in the section was read against. What that
missed is that an even matchup is not what the game actually fields: measured
against the shipped roster with the real functions, a player carrying no
accuracy gear sat at 0.44-0.64 for the first ten levels, and both apex
species — the lair guardians — are the fastest things in the game and so the
hardest to hit at every level. Roughly half of everything whiffed early.

It surfaced as "routines miss too often", and the asymmetry in that complaint
is real even though the *rate* is not routine-specific: a routine and a basic
attack share one path (`resolve_and_apply_attack`), but a basic attack shrugs
a miss off, while a routine has already spent its Power and armed its
cooldown by the time the roll happens. Thirteen of the twenty-five damaging
routines are multi-target with an independent roll per recipient, so one
sweep at 55% printed two or three "goes wide" lines in a row.

**A multiplier and not an addend**, because a flat `+n` on accuracy washes
out as levels grow — the same scale-dependence the difference form is
forbidden for, arriving by another door. 1.4 puts parity at 0.583.

**Necessarily symmetric**, because `hit_chance` is a pure function of two
numbers and cannot know which side is the player. Hostiles take the same
edge, which notably lifts them off `HIT_CHANCE_MIN` — a level-40-plus player
had pinned them to the floor. The player's *asymmetric* edge is the flat
accuracy sources instead (see the accuracy-door seam below).

**What the balance gate caught, and what it meant.** Zones 2-10 were
untouched (25 to 146 rounds against the round floor). Zone 1 fell from 3
rounds to 2 — because `zone_group_cap(1)` was **1**, making that fixture a
five-against-*one* fight rather than the body ratio the rest of the curve is
about. The fix was `ZONE_ONE_GROUP_CAP`, applied as the clamp's *lower bound*
so no later zone's step moves, rather than backing the constant off to 1.25
to fit a degenerate fixture. It also ends `TRACE_GROUP_MULT`'s zone-1
inertness, which was always a consequence of the group curve rather than an
intent of that constant.

**Accuracy and Evasion are derived, never stored.** No `Stats` field, no save
field, so they cannot drift from their inputs: `base_speed` plus level plus
every flat source, which `Game::accuracy_bonus` sums and which are read live
because, unlike `atk` and `mitigation`, none is baked into `Stats`. `atk` is
deliberately absent from both — feeding it to-hit *and* damage compounds
quadratically. Speed comes from `Game::combat_speed`, which is one rule for
initiative and to-hit alike; they disagreed briefly, and the player came out
acting first against an average opponent while hitting as though slower than
one.

**Draw counts are pinned per outcome** (`draw_counts_are_pinned_per_outcome`):
a miss and the two status rungs cost one, a hit, a crit and a Recoil cost two.
Pinning them is what stops crit or fumble silently becoming an extra draw and
shifting every seeded run's stream. `DamageRange::roll` is written as an
offset from `min` rather than `random_range(min..=max)` for the same reason —
a degenerate band must still spend exactly one draw, or the stream would shift
with the party's loadout.

**The Opening rung's free swing must not itself fumble.** Without the
`allow_fumble: false` guard one bad roll chains into an unbounded exchange and
the deepest rung stops being the run-ender the ladder is shaped to avoid. The
type already forbids a nested `Fumble`, so classifying the outcome proves
nothing; `the_opening_rung_does_not_recurse` bounds the *draws* an Opening may
spend, which is what actually catches it.

**`Game::attack_nest` is deliberately outside all of this.** A structure has
no speed and cannot dodge, so it keeps the deterministic path it always had —
identical swings stay identical, or wearing a nest down becomes a slot
machine. `combat_policy` is the other non-roller: it is *choosing* a swing,
not making one, so it takes `expected_damage`'s mean and spends no draw.

### Flat Accuracy has one door per axis, and the two axes are not the same one

**`Game::accuracy_bonus` is what an *entity* brings to every swing it makes.**
Gear, plus `Perk::TargetLock` for the player, plus `TalentNode::Accuracy` for
a companion. The perk is hooked here rather than at the roll because a perk's
hook belongs where its sources meet — `Obfuscation` sits inside `raise_trace`
rather than at the six things that raise Trace, and a hook that has to be
repeated is the signal the perk is aimed at the wrong seam. The player/
companion split is on identity against `player_entity()`, `ability_affinity`'s
rule, so a perk and a talent can never stack by construction rather than by
the player happening to have no `Talents`.

**`battle::Swing` is what an *invocation* brings**, and that is
`AbilityDef::accuracy` and nothing else. The damage band was the only
per-invocation property for a long time and travelled as a bare
`DamageRange`; accuracy is the second, and loose parameters are what
`Combatant`'s own doc rejects, since two of the four call sites have nothing
to say about accuracy at all. It aims one swing:
`resolve_and_apply_attack` builds the *defender's* profile from
`Swing::plain`, or an Opening rung's free counter would be aimed by the very
routine it is countering, and `combat_policy`'s projection stays a
basic-attack projection.

**A routine's accuracy is flat and never scaled by level**, unlike every
magnitude beside it. A hostile's Evasion grows with the *zone* while the
invoker's Accuracy grows with their *level*, and a player levels far faster
than zones advance — so to-hit is already a solved problem late and an
unsolved one early. A bonus that scaled would be largest exactly where it is
needed least. The shipped roster grades it by how narrow the routine is (6
single-target, 4 whole-group, 2 all-enemies), so a sweep trades odds for
reach rather than being strictly better.

**The serde default has to stay 0** for a mod's file to parse untouched,
which is exactly why the shipped roster needs a census — `spread` shipped
documented and authored by none of 77 files for several releases. This one
runs both ways: every rolling routine must author it, and no non-rolling
routine may, since the field is read by nothing there and a modder reads the
shipped roster as the schema.

**The trap is enumerating `EquipmentStats`' fields by hand.** Both emptiness
arms in `AffixDef::fault` named three of six, so an affix paying only
accuracy, only evasion or only damage was refused at load as granting
nothing. The accuracy axis could therefore only ever ride along on an ATK
affix — which is part of why it stayed on three weapons for as long as it
did, and why the fix was found by trying to author the first pure-accuracy
affix rather than by reading the code. `EquipmentStats::is_empty` and
`has_upside` **destructure** rather than field-access, on `cell_mark`'s rule:
a seventh stat is a compile error there rather than a field silently
uncounted.

**A companion's node is read on demand, never baked**, for the reason above:
there is no `Stats` field to bake into, and a baked one would be re-applied
on every load. It is `Affinity`'s shape exactly, down to
`Game::talent_accuracy` mirroring `talent_affinity_mult`. Bounded by
`MAX_TALENT_ACCURACY_POINTS` because Accuracy feeds a ratio — unbounded, one
node would walk a companion to `HIT_CHANCE_MAX` on its own and make every
later tier in its tree moot.

### Mitigation is percentage points, and `Game::effective_mitigation` is the one door

**`Game::effective_mitigation` caps at `MAX_MITIGATION_PERCENT` itself**, so
nothing downstream can see an uncapped percentage and no reader has to
remember to clamp. It sums innate `Stats::mitigation`, an active
`CombatBuff::Mitigation`, any running `FieldBuffKind::Mitigation`, and the
player's party and wielded bonuses.

**The trap is that `Stats::mitigation` already carries gear.**
`Game::apply_equipment_delta` bakes an equipped item's `atk` and `mitigation`
straight into `Stats`, so adding `gear_bonus` inside `effective_mitigation`
would double-count every worn piece — the same trap "no stats operation may
run while a gear bonus is sitting in `Stats`" names from the other direction.
The other three gear axes (`damage`, `accuracy`, `evasion`) have no `Stats`
field and are read live; `apply_equipment_delta` must not invent one for them.

**It is never scaled by level or zone**, and that is the rule most likely to
be "corrected" by someone restoring symmetry with the other stats. A
percentage that grows per level approaches immunity, so
`progression::stats_after_levels`, the wild spawner, `balance_sim`'s
`wild_stats_at_zone` and `refactor::refactored`'s `zone_bump` all leave it
exactly as authored. `DEF_PER_LEVEL` was deleted rather than left unused, and
`LevelGain` lost its `def` field with it. **Levelling's defensive growth is
evasion instead** — which is why the balance curves looked like lockouts for
the seven commits between the rename landing and `balance_sim` learning to
read evasion.

Removing the tier step from `refactored` also fixed something else. Trade's
`earned_power` divides a bought zone tier back out by the tier ratio, which
only recovers the base while `power()` is homogeneous in that ratio; with
mitigation sitting the step out, "bought tiers buy no Credits" is exact again
instead of leaking a Credit.

**`Stats::power` prices mitigation as the effective HP it buys**,
`max_hp / (1 - mitigation/100)`, because summing a percentage into a total
the way `max_hp + atk + def` did is meaningless. The cap is load-bearing here
too, in a second way: it is what keeps that denominator away from zero. Every
con colour, every kill's XP and every trade valuation in the game moved when
this landed, since all of them read `power()`.

**One unit, one name.** `FieldBuffKind::Def` was deleted and `BuffKind::Def`
renamed to `Mitigation`: once `Stats::def` was percentage points there was no
flat-defence axis left for a second name to describe, and two names on one
axis is what makes both unreadable wherever they are summed. Two formulas
outside combat took the percentage too — the raid defender's structure
mitigation and the enemy policy's projected damage — and the policy one was
*live* rather than latent: a braced target subtracting 20 looked untouchable,
and `bracing_still_draws_more_fire_under_the_shipped_weights` caught it. The
raid path clamps to 100 rather than to the combat cap, deliberately: a raid is
not an attack on a creature, and "fends off a sweep without a scratch" is a
shipped outcome the combat cap would have deleted silently.

### A kill's XP is priced by challenge, and the price shares its threshold with the colour the glyph is already drawn in

**A kill's XP is priced by challenge, and the price shares its threshold
with the colour the glyph is already drawn in.** `progression::kill_xp` is
the victim's whole `max_hp` scaled by `power_ratio / DIFFICULTY_EASY_MAX`,
clamped to `XP_CHALLENGE_FLOOR`..`XP_CHALLENGE_CEIL` (0.25..2.0) — so the
rule a player can state off the map is **green pays less, yellow and up
pays full or more**. Reading `power_ratio` (`game/inspection.rs`) rather
than a second notion of relative difficulty is what stops the glyph lying
about the reward, and it makes this the *third* reader of the con
thresholds after `difficulty_color` and `capture_chance`. `Game::kill_xp`
is the one place the two powers are gathered; its denominator is the
player's power **alone**, deliberately not the party's, because counting
the roster in would dock the player XP for recruiting a companion. Both
clamps are load-bearing in opposite directions: without the floor an
over-levelled party earns nothing in the opening ring, which is the one
place the game keeps fights trivial on purpose; without the ceiling a
Stack guardian earns a multiplier on top of a bar `STACK_DEPTH_STAT_STEP`
has already inflated, which is the double-count behind "four depth-3
fights were worth five levels". A third floor sits in `kill_xp` itself —
a kill always pays at least 1, since a quarter of a small enough bar
rounds to zero and a kill that silently pays nothing reads as a bug.
Measured pacing is `docs/measurements/2026-08-15-challenge-xp-pacing.md`,
and `balance_sim` gates none of it: that simulator models no XP at all, so
the slowdown is answerable only by play.

### Levels come at half the count and twice the size, and that half is power-neutral by construction rather than by measurement

**Levels come at half the count and twice the size, and that half is
power-neutral by construction rather than by measurement.** Every constant
denominated in entity level carries `K = 2` (`HP_PER_LEVEL` 24,
`ATK`/`DEF_PER_LEVEL` 2, `PERK_POINTS_PER_LEVEL` 2,
`DECOMPILER_SKILL_PER_LEVEL` 2, both `ABILITY_*_SCALE_PER_LEVEL` doubled)
and every constant denominated in *levels per* something carries its
reciprocal (`PLAYER_ROUTINE_SLOT_PER_LEVEL` 5,
`COMPANION_ROUTINE_SLOT_PER_LEVEL` 1, `CREATURE_MAX_LEVEL` 6,
`WORK_XP_LEVEL_CAP` 5, `ABILITY_SCALE_LEVEL_CAP` 20).
`XP_PER_LEVEL_STEP` carries `K^2` — 80 — and the square is the whole
argument: cumulative XP to a level is `(STEP / 2) * L^2`, so halving the
levels needed for a given power has to be paid for by four times the step
or the same power arrives for a quarter of the XP. Two instruments
confirmed the neutrality rather than an argument: `balance_sim`'s reach
curve halved while staying linear, and
`a_perked_mid_run_kernel_panic_lands_in_the_intended_band` reproduced its
existing damage band at level 5 where it used to sit at 10, band
untouched. **Species ability unlock levels are in that same currency and
live in the assets**, so they moved too (signature 6 -> 4) — left alone,
the capstone would have landed exactly *at* the halved cap, delivering the
reward as growth ends. One behaviour genuinely changed: at a slot per
companion level a level-up brings room for whatever it unlocks, so no
shipped species can reach the routine-eviction branch, which now needs a
mod granting two routines on one rung (`CONTENDING_UNLOCK_SPECIES`).
A fourth thing that is *not* in this currency and must not be swept into
it: `PLAYER_BASE_STATS` is an offset, not a rate.

### The ring buys room; the fights buy the points

**The ring buys room; the fights buy the points.** A Privilege Ring — dropped
by a lair guardian and by nothing else — is spent at the Develop screen to open
a Kernel Ring on **one** companion, and `Game::companion_level_cap` is the one
expression of what that is worth: `CREATURE_MAX_LEVEL + ring * LEVELS_PER_RING`.
`open_kernel_ring` grants no stats, no level and no XP, which is what keeps the
feature inside "progression is earned by fighting": the ceiling moves and the
party still has to go and earn the levels under it.

Two call sites deliberately do **not** go through it, and both look like bugs
from the outside. `systems.rs`'s cronjob payout keeps passing
`Some(CREATURE_MAX_LEVEL)`; its own `WORK_XP_LEVEL_CAP` guard already stops a
posted worker at 5, so leaving it alone is the whole of how a developed program
cannot be ground up at a Mining Node — pinned by
`a_ringed_cronjob_worker_still_stops_at_the_work_cap`, which drives real
`task_progress_system` cycles rather than calling `add_xp`, because the guard
under test is in `systems.rs` and calling `add_xp` would test nothing. The two
arena sites (`arena::set_level` and app-core's level stepper) take
`tuning::absolute_companion_level_cap()` instead, because an arena scenario
authors its own composition and has no `KernelRing` to read — and `Ability`,
`Affinity` and `RoutineSlot` talents are invisible to `balance_sim`, so the
arena is the only instrument that can see them and one clamped at 6 could not
stage the fight the trees exist to change.

That clamp move has a consequence worth knowing: five shipped `dev-arenas/`
scenarios author `party: [(… level: 12 …)]` and were silently getting level 6
after `HP_PER_LEVEL`'s `K = 2` halved the cap. They now field what they say.
Old reports from them are not comparable to new ones — see
`docs/measurements/2026-08-19-developed-companion-worth.md`, which also carries
the figure that decided the sale question: every ring open is 1.95x a
companion's power at the base cap, and a fully spent generic tree is 2.12x.

### Talent points are derived, never stored

**Talent points are derived, never stored.** `Game::talent_points` reads
`earned` off the level (`level - CREATURE_MAX_LEVEL`) and `spent` off the
length of `components::Talents`. There is no count on the component and none in
the save, and that is not tidiness: a stored count can desync from the level
*and* from the list, and both desyncs are invisible until a player finds they
have a point that buys nothing or a node they never bought. Nothing that
derives it can drift.

The same argument makes the tier rule free. A tier costs exactly one point and
tiers are taken in order, so "which tier is next" is `Talents::0.len()` — there
is no cursor to keep in step. `take_talent` resolves an id against **that tier
of that companion's own tree**, and splits its refusals: a node deeper in the
same tree and a node from another class's tree leave the player different
errands, and a test stages the second case with a node that really exists in a
*different* tree, which is what a naive "is this id known anywhere" check gets
wrong.

One ordering inside `take_talent` is load-bearing in the opposite direction to
the usual rule: the receipt is written **before** the effect is applied, because
`install_unlocked_routines` asks `talent_abilities` what this program's talents
grant and that has to include the node just bought. Every refusal is already
behind that line, so nothing can leave a receipt for something that did not
happen.

### A `Stat` talent bakes into `Stats` at purchase, and load must not re-apply it

**A `Stat` talent bakes into `Stats` at purchase, and load must not re-apply
it.** `CreatureSave` already writes `hp`/`max_hp`/`atk`/`def`, so a saved
program's numbers *are* its talents; re-applying the list on load would compound
the bonus on every reload. `components::Talents` is a **receipt**, exactly as
`Refactors` is and for the same reason `Rarity`'s tag is written without its
multiplier. The test is a save → load → assert that the stat is *unchanged*, and
a RON round trip cannot stand in for it: a field that fails to travel looks
identical to one that does from the round trip's side.

The purchase itself goes through `refactor::raised` rather than restating the
arithmetic, which is what carries its never-less-than-a-whole-point floor — 8%
of a Drone's 3 ATK rounds straight back to 3, so without the floor the node
would do nothing to exactly the weak programs it exists to help. And gear is
lifted and put back around the write (`gear_bonus` / `apply_equipment_delta`),
because a bonus sitting in `Stats` during a multiplication is scaled while the
later unequip subtracts only the unscaled amount — the `EquippedItem::fusion_tier`
trap, welded in permanently.

The other three node kinds are read on demand instead of baked, each at the one
seam that already answers its question: `RoutineSlot` in `Game::routine_slots`'
**companion arm only** (the player is not a companion and must not read a
companion tree, and `abilities::companion_routine_slots` stays a pure function
of level because `balance_sim` and several tests read it as one); `Affinity` in
`Game::ability_affinity`'s **creature arm only**, clamped to `AFFINITY_MAX` the
way the perk arm is, since a mod's tree may author any magnitude; and `Ability`
folded into the `declared`/`reached` lists both install paths already build,
rather than a second install path beside them — which is what guarantees a
granted routine competes for slots exactly as a species-kit unlock does and
leaves a *carried* routine, the prize the program was decompiled for, in place.

### Fusion keeps the dominant parent's ring and talents

**Fusion keeps the dominant parent's ring and talents.** `fuse_companions` is
one of four doors into the roster and the one that assembles its **own**
component list rather than going through `Game::roster_parts()`; it also does
its own `retain`/`despawn` and skips the detachment logging
`dissolve_tamed_program` performs. Nothing fails to compile when a component is
missing from a hand-written tuple, and the symptom — a fused companion that lost
its development — reads as "fusion is bad" rather than as a dropped field. That
is the failure this entry exists to head off, and it is the same argument
`Refactors`/`PurchasedTiers` already earned there.

The dominant parent is the one whose species and level the child takes, and it
is deliberately also the one whose development it inherits: taking both parents'
would make fusion a way to launder two developed programs into one, and taking
neither would burn a lair guardian's drop. Nothing re-applies a `Stat` node
during the fusion — `fuse_stat`'s inputs are the parents' own numbers, which
already carry it, and the strip-gear-before-the-snapshot rule above is why the
order there cannot be relaxed either.

### `Experience::xp_to_next` is derived on load and never read back from the save

**`Experience::xp_to_next` is derived on load and never read back from the
save.** It is a second copy of `xp_for_level(level)` that agrees only while
`XP_PER_LEVEL_STEP` holds still — a save written under an older step hands
out its next level at the old price, silently, once per entity in the
roster. Both load paths in `game/lifecycle.rs` call `xp_for_level` instead.
The field stays *written*, because removing one is what earns a
`SAVE_FORMAT_VERSION` bump while being ignored costs nothing.
`Experience::default` held the same literal and drifted the same way.

### `balance_sim` has no Stack term at all, so the arena is the only instrument for a lair

**`balance_sim` has no Stack term at all, so the arena is the only
instrument for a lair.** It sweeps zones with surface group sizes and
models no depth, no lair and no abilities. `Encounter::Lair` exists
because `Encounter::Stack` rolls `stack_encounter_pack`, which passes
`allow_boss: false` — so the guardian, the only boss the Stack fields and
the only source of Portal Fragments, was unmeasurable. It is also
mandatory and not depth-selectable: a stack's lair is on its bottom frame
and `frames_for` sets that depth from the link's distance to the spawn
point, so a player can be handed a 6-frame stack as their only remaining
lair. `docs/measurements/2026-08-12-stack-lair-reachability.md` is what
the instrument said; `dev-saves/deep-lair.ron` is the run it said it about.

### The opening ring needs an explicit radius, and used to derive one

**The opening ring needs an explicit radius, and used to derive one.**
`in_opening_ring` was spelled as "both curves say a fight here is one
program", which was exact while distance drove those curves. With fixed
zone scaling it is true across *all* of zone 1 — `zone_group_cap(1)` is 1
everywhere — so that spelling would silently turn the whole opening zone
into a nursery. It is now `OPENING_RING_TILES` from the danger origin.
**It derived a second time and was decoupled a second time**, on
2026-08-13: it was spelled `= MAX_BUILD_DISTANCE_FROM_HOME`, "exactly your
base and its doorstep, travelling with the base for free", which held only
while a base was one fixed size. The Heap Pillar made it two things at
once — halving the start would have shrunk the nursery to 4 for the
opening minutes, and every Pillar afterwards would have *widened* it,
which is a difficulty knob keyed to base geometry and exactly what the
2026-08-05 removal was for. It is now its own literal 7. **Slice 2 of the
out-of-phase base revives the hazard the second decoupling closed** — a
player digs the pocket outward for the rest of the run, so a ring derived
from base geometry would be a nursery that grows every time somebody
swings at a wall. The literal is what stops that, and it is worth more now
than when it was written. Nothing would have caught either half: `balance_sim` is RNG-free and models no spawn
positions, and the census below is of the roster rather than of the
radius.
The ring is also why the four species it draws from (drone, glitch,
sprite, sub_process) sat out the 2026-08-05 roster raise: they are the
only four `beatable_by_a_fresh_player` clears, and `habitat_pools` falls
back to the biome's *unfiltered* roster when nothing qualifies — so
raising them empties the ring while leaving it looking intact.
`the_shipped_roster_has_species_on_both_sides_of_the_opening_ring` is what
catches that, and it is a census that a retune can break from either end.

### Wild population is a property of place, and the density target is what "populated" means

**Wild population is a property of place, and the density target is what
"populated" means.** `WILD_LOCAL_DENSITY_TARGET` is how many `Hostile`s
belong within `WILD_SPAWN_RADIUS_TILES` — a 25x25 box, near enough the map
pane's own ~33x19 that the knob is legible on screen.
`Game::local_hostile_count` is the single definition of "how crowded is it
here". `Game::ensure_local_population` stocks any world chunk within
`POPULATION_CHUNK_MARGIN` of the player's own that `resources::PopulatedChunks`
has not already marked, and `maybe_spawn_wild_creature` tops the local box
back up — so space is filled by the first and regrowth is done by the
second, each with a job the other cannot do.

The history is the argument, because the shape has been wrong twice and the
second wrong shape looked right. Originally there was no target at all and
nothing ever removed a creature, so density was simply the integral of where
the player had stood: a real save measured 65 in one box around a worked-at
base and 7 in the entire map past 40 tiles. `0.5.12` added the density
target and seeded a zone across a 40-tile disc, which flattened the peak and
was believed to have fixed it. It had not. Measured on 2026-08-18
(`docs/measurements/2026-08-18-wild-population-halo.md`): after 20,000 ticks
of pottering around a base, the boxes at 0, 25, 50, 75 and 100 tiles out
held 15, 10, 6, 3 and 1 against a target of 12 — the same halo, one third as
steep. Walking 300 tiles in a line left **zero to two per box** past 60
tiles.

The reason is arithmetic and not tuning, which is why a third pass at the
knobs would have failed too. `world::WorldMap` is unbounded and generates a
chunk of terrain whenever anything asks about a tile in it, so there is no
finite area a one-time seed can cover; and `move_player` ticks once per
tile, so a player crossing one 25-tile density box buys 25 ticks of
`WILD_SPAWN_CHANCE` — about 1.25 rolls against a target of 12. Raising the
chance to 1.0 still only places ~25 per box crossed, while firing a spawn
beside the player every single tick. Whatever the constants say, a
player-relative spawner in an unbounded world cannot populate ground at
walking speed.

Five things about the shape are load-bearing:

- **The mark is written before the chunk is stocked, not after.** A chunk
  that turns out to hold nothing placeable — open water, a biome with no
  habitat species, ground a neighbour already crowded — is marked anyway,
  or it is retried every tick for the rest of the run.
- **`POPULATION_CHUNK_MARGIN` is 1, not 0.** Stocking only the chunk the
  player stepped into pops programs into view inside ground they can already
  see: a chunk is 32 tiles and the pane shows ~33x19. One chunk of margin is
  32 tiles at worst, which preserves what `WILD_SPAWN_RADIUS_TILES` was
  chosen for — a spawn lands off-screen and is walked into.
- **It draws from `GameRng` rather than seeding a local `StdRng` off the
  chunk coordinates**, which is the opposite of `stack::generate` and is
  deliberate. A frame must regenerate identically because the party has
  *seen* it. A chunk's population is explicitly not reproducible:
  `cull_to_cap` evicts and forgets whole chunks, so walking back is meant to
  find different programs. Pinning it to the place would be a promise the
  eviction breaks on purpose, and it keeps `try_spawn_habitat_creature` —
  which owns species, rarity, the opening ring and boss substitution — free
  of a threaded seed.
- **`PopulatedChunks` is zone-local**, so like `BuybackLedger` and
  `StackMemory` it must be wiped **by name** in `enter_next_zone`. A mark
  carried forward tells the new sector that ground it has never stocked is
  already full, which empties the new zone exactly where the old one was
  populated.
- **`cull_to_cap` takes its candidates from where hostiles actually stand,
  not from the mark set.** Reading the marks instead is the version that
  reads better and is wrong: a program that wanders across a chunk boundary
  into unstocked ground would be immune to eviction forever, so `wander_ai_system`
  would slowly reopen the very leak `WILD_CREATURE_CAP` exists to close. The
  mark is dropped as a *consequence* of evicting a chunk, which is why it may
  be absent. It evicts whole chunks because a chunk is the unit population is
  placed in — thinning one leaves it marked-but-empty, and unmarking it for
  one creature leaves it marked-unstocked while still full. It never touches
  the player's own neighbourhood, which is the one place an eviction would be
  watched.

Two more, inherited from the density target and still true. The gate sits in
`maybe_spawn_wild_creature` and *not* in `spawn_wild_nearby`, because
`dev_force_encounter` shares that body and must still place a fight in a
crowd — density paces an ambient spawn, it is not part of what a spawn *is*.
And it is checked **after** the roll, which keeps the scan off the 95% of
ticks that spawn nothing and leaves the RNG sequence the seeded spawn tests
depend on untouched on a miss.

`chunk_wild_population()` is derived from the target and the two sizes
rather than tuned, for the reason the deleted `initial_wild_population()`
was: the density a patch of ground is born at and the density the ambient
roll maintains must not be able to drift apart. Its predecessor pair (14
rolls across 15 tiles) agreed by luck and would have disagreed the moment
either moved.

What this cost: `WILD_CREATURE_CAP` stops being decorative. It had never
fired at a peak of 215; the travel measurement alone reaches 716, and a long
session of exploration will reach 2000 and start evicting, which nothing has
exercised in play. Simulation cost went from ~0.08ms to ~0.23ms a tick in a
debug build at ~700 hostiles — the game ticks once per player action and once
a second idle, so it is nowhere near a frame, but it is three times what it
was.

### `Tile::open_to_hostiles` is the base slab's fourth reader, and `walkable` alone has never been the rule

**`Tile::open_to_hostiles` is the base slab's fourth reader, and
`walkable` alone has never been the rule.** The slab is the one safe
ground: `maybe_ambush` refuses to roll while the player stands on it,
`stamp_platform` purges what is standing there when the floor is laid,
and `pursuit_field` keeps a provoked swarm off it. `wander_ai_system`
quietly disagreed — it checked `walkable` — so an ordinary wild program
could stroll onto a base a *pursuing* guardian was forbidden to enter.
It went unnoticed because `stamp_platform` clears the slab as it is laid
and the population was small enough that few programs ever stood beside
one; raising the density made "adjacent to the edge, one step from the
inside" the common case and the existing spawn-side test caught it by
accident. A fifth mover goes through the predicate, not beside it.

### `BaseGrid` is the one base resource that is not zone-local

**`BaseGrid` is the one base resource that is not zone-local, and
`Game::enter_next_zone` says so by omission.** Four resources are
zone-scoped and wiped by name on every breach, all inside that one
function: `resources::BuybackLedger` and `resources::PopulatedChunks` are
replaced outright with `insert_resource(::default())`, `resources::StackMemory`
the same, and the two currencies (`Game::currency`, `Game::craft_currency`)
are drained from the player's `Inventory` item by item. `BaseGrid` never
appears in `enter_next_zone` at all — not reset, not migrated, not even
read. That silence is deliberate: the base is out of phase, not on the
zone surface, so a breach has nothing to say about it. Every `Structure`'s
`Position` is a `BaseGrid` coordinate (see the entry below), untouched by
the sweep that despawns `Hostile`, `Nest` and `SurfaceLink`, and the grid
the base is carved out of carries forward whole, the same way the
structures standing on it do. `breaching_does_not_touch_the_base_grid`
(`tests/zone.rs`) pins exactly this: it builds a base, breaches, and
asserts the `BaseGrid` compares equal by value — not by cell count, so a
breach that rewrote every cell to the same count at different coordinates
would still fail it.
`BaseGrid`'s own module doc says it is saved wholesale in `SaveData`
"the same way `resources::StackMemory`/`PopulatedChunks` are" — that
sentence is about the *encoding*, a plain embedded field rather than a
mirrored save-shaped type, and does not extend to the *lifecycle*. Those
two are re-created every breach and `BaseGrid` is not, which is what makes
the four neighbours a pattern rather than a rule: zone-local state gets
wiped by name at breach, and `BaseGrid` is the deliberate exception to it,
not a fifth entry that was left out. **The next base resource has to ask
which of the two it is before reaching for either.** Something a zone's
own geometry could invalidate — the way a Stack link's map or a chunk's
population mark would be — is a wipe-by-name candidate like its four
neighbours. Something that lives in base space, the way `BaseGrid` and
every `Structure` on it do, needs the omission instead, and nothing
enforces that choice: there is no test that fails if a future resource is
wired into the sweep by habit, or forgotten out of it when it should have
been. The call has to be made deliberately, against what space the
resource actually lives in, not against which pattern is already sitting
in `enter_next_zone` to copy.

### Base space carries its own seed, because base space travels

**Base space carries its own seed, and it is not `WorldMap::seed()`.**

`rock::RockDb::kind_at` decides what any base-space coordinate is made of by
folding a seed with the block the coordinate falls in. The obvious seed to
fold is the world's, and it is wrong.

`WorldMap::seed()` reads like the run's identity. It is not:
`enter_next_zone` mints the next zone's map from
`seed().wrapping_add(0x9E37_79B9)`, so the world seed is a *zone's* identity
and is different in every one of them. `BaseGrid`, meanwhile, is among the
handful of things that survive a breach intact — the base is what a run
carries between zones.

Salted off the world seed, therefore, every seam in the base would reshuffle
the moment the player portalled. A player who had spent a zone learning where
the hard rock in their base was would come back to a different base. Worse,
and quieter: a wall left half-cut keeps its `Durability` across the breach,
so it would come back as a *different kind* with a different ceiling under an
already-spent meter — a Fused wall reloading as ordinary rock at 24 max_hp
with 90 points of progress on it.

So `BaseGrid` mints its own `seed: u32` at `Game::new` and saves it with the
grid. `#[serde(default)]`, so a save written before kinds existed loads at
seed 0 — a valid deterministic layout rather than a special case — and
additive, so no `SAVE_FORMAT_VERSION` bump.

`base_spaces_seed_and_its_seams_survive_a_breach` is the test, and it asserts
**both** halves in one function: that the world seed moved, and that the base
seed and its seams did not. The first assertion is what makes the second mean
anything — without it the test passes against a game in which neither seed
ever changes, which is exactly the state a refactor could leave it in.

### A rock kind is a brightness, never a hue

**A rock kind authors a `shade`, and the reason it cannot author a hue or a
colour is the map's oldest rule.**

`sectors.rs`'s `SectorPalette` states it: hue answers "can I walk here", and
the spread *within* a band is what tells terrain apart. `render/base.rs`
records the consequence for anything told apart inside one band — "brightness
rather than hue, since hue is already spoken for". Rock is a hole in the map
and shares the hot family with `DataVoid` and `BlackIce`.

A free RGB on `RockDef` would let the first mod ship a green wall, which
reads to a player as crossable ground. An authored *hue* is no better and is
worse in one specific way: `biome_tint` rotates every biome's hue by however
far the current sector's anchor has moved, so an authored hue would fight
that rotation and a seam the player had learned to recognise would change
appearance on a breach — the same failure the base seed above exists to
prevent, arriving through the palette instead.

So a kind carries a brightness factor against `Biome::Entropy`'s own colour,
`RockDb::load_dir` refuses a file outside `SHADE_BAND`, and the renderer
scales *before* `biome_tint`'s rotation. A dense seam is a brighter patch of
the hole it is part of, which is the same axis `Excavated` and `Entropy` are
already separated on, and it stays inside the impassable band under every
sector palette.

The band's lower bound is 1.0 rather than 0.0 for a reason worth keeping: an
exposed face darker than the wall around it would be *harder* to see than
anonymous rock, which inverts the whole feature.

### Seeing a rock kind is a display rule and never a gameplay one

**A wall's kind decides what a swing meets, whether or not the player can see
it.**

Only an exposed face — solid rock with air orthogonally against it — is drawn
in its kind's brightness or named by examine. That asymmetry is the feature:
colouring every wall would hand the player a map of everything they will ever
dig, so exposing a face is the act of prospecting.

The trap is that the asymmetry looks like an inconsistency, and the tempting
"fix" is to make unseen rock *resolve* to the default kind so the two halves
agree. That would delete the point. `Game::strike_rock` resolves the true
kind through `wall_at` regardless of exposure, so swinging blind into deep
rock meets a Fused wall's real 120 and its real floor of four, and finding
that out the hard way is what makes prospecting worth doing.
`a_swing_at_unseen_rock_meets_its_real_kind` is the test standing in front of
that fix.

The other half of the same rule runs the other way: **the map and the examine
ray must never disagree about a wall they can both see.** If the map hid a
kind while `x` named it, the hiding would be decorative. Here the agreement
happens to be structural — the ray stops at the first solid cell, whose
predecessor is walkable by construction, so every cell it can reach is
already a face — but "by construction" is precisely the kind of claim that
stops being true when someone lets the ray run one cell further.
`the_map_and_the_examine_ray_agree_about_a_wall` asserts the two readers
against **each other** rather than against a hardcoded string, so they cannot
drift apart.

`is_exposed` is derived per lookup and never cached, for `Platform`'s radius
reason turned inside out: three separate verbs move it. Cutting a cell
exposes four neighbours, flooring one changes nothing, and
`base_entropy_system` re-knitting a cell un-exposes them again — a cached
flag would need keeping in step with all three, and the one that would be
forgotten is entropy's, because it fires on a timer rather than on a
keypress.

### Mining is a tool the player takes out, and the crew never reads it

**`resources::MiningMode` governs the player's own bump and nothing else.**

Slice 1 refused a step into base-space rock for free. Slice 2 made cutting
the point and turned that refusal into a swing, on the grounds that the wall
is a thing you attack. What that did not cost was the interaction with
`swing_damage` growing all run: at a developed level, walking a corridor in
your own base destroyed the corridor's corners, and the player's bump became
the one gesture in the game that destroys terrain without being asked twice.

The toggle is off when a run starts and off for any save that never expressed
a preference — arming a terrain-destroying tool on behalf of a player who
never asked is the wrong default in both directions.

**The crew must never read it.** A mark is an instruction the base was
already given through the excavation plan; putting your own tools away says
nothing about a job already posted. Wired into `run_dig_crew`, disarming the
bump would stall every dig job in the base at once, and the symptom — bodies
walking to marked cells and standing there — reads as the crew being broken
rather than as a toggle doing something nobody asked it to.
`a_posted_crew_cuts_a_marked_cell_with_the_players_mining_off` is the test.

The cost of the gate is paid entirely in test fixtures: eleven of them dug by
walking into a wall. They arm the tool through the named
`game_at_the_frontier_cutting` rather than a line repeated at each site,
because the failure mode of forgetting it is that the wall simply never comes
down — which reads as the dig being broken rather than as the fixture being
short something, exactly like `work_node_parts()` and `park_at_post()`.

### The base's footprint is `BaseGrid::is_floor` and nothing else

**The base's footprint is `BaseGrid::is_floor` and nothing else.**
`resources::Platform` — the cached, derived circle-or-claim shape the base
used to have on the zone surface, `build_radius`, `Platform::covers`, the
Heap Pillar's ring and the Heap Block's claimed tiles among them — is gone
outright, along with every structure and tuning constant that fed it.
Deploying the first Home now calls `Game::lay_starting_pocket`, which
lays `BaseCell::Floor` over a `STARTING_POCKET_RADIUS` chamfered box of
`base_grid::BaseGrid` at base space's own origin — one shape, laid once,
never grown or re-stamped by anything slice 1 shipped — slice 2 is what
grows it, and does so by writing the grid, not by reviving a derivation. Its own doc
comment states the point directly: "It writes no `WorldMap` tile. That is
the point of the whole relocation: `Biome::Platform` stops being stamped
into the zone surface, and the base's footprint becomes
`BaseGrid::is_floor` and nothing else." Every reader that used to ask
`Platform::covers` "is this part of the base" now asks `is_floor`
instead: `place_structure` refuses a build off laid floor, and
`Game::broker_reach` (`game/contracts.rs`) answers `AtBroker` only when
the player's base-space tile is `is_floor`, both a straight `is_floor(x, y)`
call with no derivation in between. `BaseGrid::walkable` is a related but
wider question — `Open` cells count too, the mined-but-unfloored state
slice 2's digging produces — so `move_in_base`'s step check and
`hauling`'s walk both read `walkable`, not `is_floor`; a caller asking
whether ground is *buildable* wants the narrower predicate, and one asking
whether a program may merely *stand there* wants the wider one. Conflating
them would let a build land on carved-but-unfloored rock, or refuse a
hauler a path across ground nobody has floored yet.
There is no growth axis left to derive, and no cached value to keep in
step with one. Slice 2 shipped both directions of change and both write
`BaseGrid` directly, the same way `lay_starting_pocket` does, rather than
feeding a formula sitting between the structures and the shape:
`Game::floor_cell` is the one place a cell becomes `Floor` in play, and
`base::entropy::base_entropy_system` is the one place laid ground is
*not* taken back — it reverts unfloored `Open` cells only, so the
footprint this entry names is permanent once it exists and a base can be
left alone while its owner is off in a zone. The starting pocket is
therefore no longer the whole of it; what has not changed is that
`is_floor` remains the only question anyone asks.

### `Structure` is the space tag

**`Structure` is the space tag.** Every `Structure` entity is spawned in
exactly one place, `Game::place_structure` (`game/base/building.rs`), and
every coordinate it can compute for a new structure's `Position` is a
base-space one — either `base_space::BASE_EXIT_CELL` for the founding
Home, or `base_pos()` plus the player's chosen offset for everything
after. That single spawn site is the whole guarantee: nothing else in the
game ever inserts a `Structure` component, so a `With<Structure>` query
answers a base-space question by construction and needs no separate
locale field or marker component to say so — the component's mere
presence already says it. A second spawn site computing a *surface*
`Position` for a `Structure` would silently break the guarantee, because
nothing checks it; every reader below trusts the invariant rather than
re-verifying it.
That is why six separate readers gate on `Game::in_base` rather than on
`is_underground` or on the numbers they are handed: `find_blocking_structure_at`
and `find_zone_portal_at` (`game/zone.rs`) both return `None` outright
off base, `Game::adjacent_structure` and `Game::find_target_in_direction`
(`game/inspection.rs`) do the same for a keypress and a ray, `view_entities`
keeps a hit only `if self.world.get::<Structure>(*e).is_some() { return
in_base; }`, and app-core's `traders_in_range` inherits the gate for free
because it is built on `view_entities`. Before each was re-read this
slice, a `Structure`'s live `Position` and a numerically identical surface
tile — commonly true near `(0, 0)`, since a base's own origin and the
zone spawn point usually share it — resolved as the same tile to a query
that never asked which space it meant. Five of those six sites were live
bugs found and fixed during this slice, not defended against on
suspicion. `find_blocking_structure_at` and `find_zone_portal_at` gained
the `if !self.in_base() { return None; }` guard in the same commit
(verified not to regress the founding Home's call, which is always
vacuously `None` there anyway — no Home means no other structure to find,
since removal cascades). `adjacent_structure`'s guard was too narrow
before this slice — `is_underground()` alone, which is Stack-only and
does not distinguish base space from the surface — and was widened to
`!self.in_base()`. `find_target_in_direction`'s `Structure` query was
*deliberately* left ungated at first, on the argument that fixing it broke
five ray-mechanics fixtures that built locale-naive `Structure`
fixtures; review overturned that call as a real, reachable bug rather than
an accepted tradeoff — "build a Home, walk back near the spawn tile, press
`x`" — and it was gated to match the other four in a follow-up commit,
with a new test reproducing that exact scenario
(`find_target_in_direction_refuses_a_base_structure_seen_from_the_surface`).
`view_entities` is the general fix rather than a sixth individual one: a
`Structure` is kept only while `in_base`, an untamed `Creature` only while
not — and `traders_in_range` needed no change of its own once that
landed, because it is built entirely on `view_entities` and inherited the
closure for free. **A caller that gates a `Structure` query on anything
other than `in_base` — or skips the gate because the coordinates
"obviously" already agree — is reopening one of these.** That per-kind
pair of gates was itself not general enough, which the entry below is
about.

### The map draws one space, and `stands_in_base_space` is which

**An entity is drawn on the map exactly when the space its `Position` is
a tile of is the space the party is in, and
`Game::stands_in_base_space` is the one statement of which space that
is.** A `Structure` or a `Tamed` program stands in base space; everything
else with a `Glyph` — a wild program, a nest, a Stack entrance, the
anchor — is a fixture of the zone map. `view_entities` and
`find_target_in_direction` both select on it, because the map and the
examine ray have to stay the same set.

The entry above got two thirds of this and stopped, gating `Structure`
one way and an untamed `Creature` the other. Three player-visible faults
lived in the third, and all three are the same aliasing: base space's
origin and the zone spawn point are both usually `(0, 0)`.

- **`Tamed` had no gate at all**, on the argument recorded in
  `view_entities`' own doc that `drawn_on_surface_map` was already the
  right filter for a program and that "every entity with an honest one
  happens to be base-space today". Both halves were true and the
  conclusion did not follow: honest *means* base-space, so on the surface
  those are precisely the ones drawn wrongly. `schedule_base_labour`
  parks idle staff on a ring around the Home every tick, which is what
  makes them honest, so a developed base put its whole roster out on the
  open grid.
- **Nothing gated the entities that are neither.** A `SurfaceLink`, a
  `Nest` and the anchor carry a `Position` and a `Glyph` and no
  `Structure` or `Creature`, so both gates looked straight past them and
  a Stack entrance drew inside the base. The anchor is the one that
  always reproduces: there is exactly one per run and it stands on the
  zone spawn point, so it drew on top of the Home.
- **The player is in both spaces at once** and so has no answer here. Its
  `Position` stays pinned to the anchor tile on the surface while the
  party is out of phase (`resources::Locale`), and `view_entities` is the
  map's only source of the `@` it draws — so the player's icon sat still
  at whatever base cell the anchor aliased onto, however far the party
  walked. It is held out of the space test and read through
  `scan_center` instead, which on the surface *is* that same pinned tile
  and so is a no-op there.

A party companion and a posted guard are the two tamed programs whose
`Position` is never written again, so theirs is the tile they were beaten
on — a surface tile, or one four frames down. Answering "base space" for
them is still right for the question being asked: a companion is standing
beside you rather than where it was caught, and `position_is_honest` is
what says so. Between the two rules a stale tile is drawn in neither
space, which is the property that actually matters.

Three engine tests had asserted the old behaviour from the wrong side of
the anchor — that idle staff are drawn, that `x` can name one, that a
tamed program keeps its own glyph colour — all three on the zone surface,
where an owned program has never been. They ask from inside the base now.
A fixture that reaches for `view_entities` merely to *locate* an owned
program wants `Game::owned_program_views`, which is the roster rather
than a window onto the map.

### A raid's flash is base-space too, and the pane has to say so

**Every `VisualEffect` the engine queues names a structure's tile, so the
whole queue is base-space by construction — and `render/base.rs` draws it
only while the pane is showing base space.** All three `Game::push_effect`
callers are structure damage (`run_raid`'s deflect, the mitigated deflect
in `damage_structure`'s defender path, and the damage itself), and a
`Structure` stands in base space by the rule above.

The entry above swept the *entities* and missed the two draw sites that
paint over them. `fx.tile_flash(world)` ran once per tile and
`fx.draw_bursts` once per pane, neither asking `base_pos` — so a GC
Entropy Sweep landing while the party was out on the grid washed a red
tile and threw debris onto whatever open ground happened to share the
struck machine's base-space numbers. The tell was the tile the party was
standing on: base space's origin and the zone spawn point are both
usually `(0, 0)`, the same aliasing the three faults above are, and the
structure being flashed is not drawn on the zone map at all any more, so
there was nothing on the tile to explain the red.

Eighty lines up, the spawn-point outline already guards itself with `if
base_pos.is_none()` and its comment names this exact hazard — "comparing
it against a base-space `center` would be the same cross-space aliasing
`view_entities` refuses now". The flash and the sparks were simply not in
that pass.

**Suppressed rather than relocated onto the anchor.** A raid already
reaches a player who is out of the base on a channel that claims no tile:
the log pane runs its own flash (`LOG_FLASH_SECONDS`) and a
`MessageKind::Raid` line lands in the log. Moving the cue to the anchor
would invent a position the engine never queued and pile every
simultaneous hit onto one tile; and a tile cue exists to say *which*
machine was struck, which is a thing the zone map cannot show.

`VisualEffect` deliberately did **not** grow a space tag. Every effect
queued today is base-space, so the field would have one variant and the
renderer would still be the thing deciding — an abstraction bought
against a second space that does not exist. If something ever queues a
*surface* effect, that is the change that earns the tag, and this entry
is what says so.

The two guards are proved separately: `a_raid_flash_draws_in_base_space_
and_never_on_the_surface` counts painted rects in the flash's own colour
for the wash and compares line-segment counts against an effect-free draw
for the sparks. Both halves put the effect on the pane's centre tile, so
neither can pass by being scrolled off-screen. Deleting either guard
fails it: the wash half reports one rect where it wants none, the spark
half fourteen extra lines.

### A `DigSite` is the second non-`Structure` entity standing in base space

**`components::DigSite` carries a base-space `Position` and is not a
`Structure`.** The entry above is the rule this widens: `Structure` is the
space tag, so a `With<Structure>` query answers a base-space question by
construction and needs nothing to say so. That rule had exactly one prior
exception — a posted program, which stands in the pocket carrying a
`Position` and a `Task` and no `Structure` — and slice 2 adds the
second. A cell of rock the player has started cutting, or marked for a
crew, is an entity with `DigSite`, `Durability` and a `Position` in the
pocket's coordinates. It is spawned lazily, by the first swing or by the
mark, which is why `base_grid::BaseCell` gains no `Rock` variant: absent
from `BaseGrid` still means solid and untouched, and a parallel cell
variant would be a second representation to keep in step. The alternative
considered and rejected was exactly that variant, `Rock { chipped }`,
which fails the moment a crew has to be *posted* to one —
`schedule_base_labour` works in `(Entity, TaskKind)` pairs and cannot
address a coordinate.

Two consequences, both found by writing the code rather than by predicting
it in the plan.

**The first is that neither system that moves or progresses a posted body
can touch a dig job.** Each resolves its target through a query a dig site
cannot answer: `haul_step_system`'s `HaulStructure` requires a
`&Structure`, and `task_progress_system`'s `WorkedNode` requires a
`ResourceNode` and a `Stock`. A `TaskKind::Excavate` arm added to either
would have been dead code that silently never ran. So the crew's whole
cycle is `Game::run_dig_crew`, a `&mut Game` method called from
`tick_inner` immediately after `schedule_base_labour` and for that call's
own stated reason — a digger posted this tick swings this tick. It has to
be `&mut Game` work regardless of the query problem, because the cycle
ends in `Game::strike_rock` and `Game::floor_cell`, which are *the* place
rock takes damage and *the* place a cell becomes floor; a bevy system
could reach neither and would have had to keep second copies of the swing
damage, the fragment roll, the substrate spend and the cell write. Four
formulas with one door each is the drift `balance_sim.rs` has already been
bitten by four times. What the split genuinely needed was the walk, and it
was **shared rather than rewritten**: `hauling::step_to_post` was lifted
out of `haul_step_system`'s tail so a digger and a machine worker walk the
same walk. A digger that loses its route drops the `Task` rather than
standing in the dark forever, which is what keeps the stall announcement
below honest — `schedule_base_labour` only ever looks at sites nobody is
posted to.

**The second is that a `Position` read against the wrong space is silent
here in exactly the way 0.13.0 shipped two fixes for** — a Recharger Node
refilling the party four frames down the Stack, and the structure roster's
"Work it yourself" row appearing off a numerically identical surface tile.
`base_entropy_system` is where that class would land next, because it has
to know which cells are occupied before it takes any back, and occupancy
comes from two places that must not be confused. The party's cell is
`Locale::Base`'s own coordinates — **never** the player's `Position`,
which is pinned to the anchor tile on the zone surface — while a posted
program's cell *is* its `Position`, because a posted program is the other
base-space exception. The occupancy query excludes `Player` for the same
reason: the player can hold a `Task` too, since `Game::work_structure`
posts them, and `With<Task>` alone does not mean "a body standing in base
space". Getting that wrong does not crash or fail a test; it reverts the
cell somebody is standing on, and "the party is inside solid rock" is a
state the Stack spent its own seam making unreachable.

**And `With<Task>` alone does not mean "a body standing in base space" in
the other direction either**, which is why the query is
`Or<(With<Task>, With<Tamed>)>` and then narrowed by `role_of`. A `Task` is
what a body is *doing*; occupancy is where it is *standing*. Staff between
postings hold none, and
`drift_idle_staff` cannot always walk one on — it declines a candidate
tile that is occupied or is not laid floor, and `schedule_base_labour`
early-returns without moving anyone at all on a game over or during a
battle, while this system keeps running from the schedule regardless. A cell reverted
under an idle staffer is not a cosmetic loss: `hauling::post_field` gates
its own **start** tile on `BaseGrid::walkable`, so the field is empty, the
body reports `NoRoute` forever, and it can never be posted or walked again
for the rest of the run.

### A mark is one verb, and what it means is decided by the cell under it

**`Game::toggle_mark_box` is the entire designation vocabulary.** A marked
solid cell means *cut it*, a marked `Open` cell means *floor it*, and the
mark **survives the cut** — so one verb runs a wall the whole way from rock
to laid tile, with the same body doing both halves. There is no second
designation kind and no separate erase verb, because **the anchor cell
decides**: a box whose first corner is already marked clears instead of
marking. That is why the anchor is read before anything in the box is
written, and why the box is normalised rather than assumed ordered — a plan
dragged up-left is the same plan dragged down-right.

What the single verb buys is the default path. "Mark the walls you want
gone, come back to floor" is otherwise a combination the player has to
assemble out of two designations, and the crew would have to be told which
half a given cell is on — a fact already sitting in `BaseGrid`. What it
costs is that the meaning is **not stored anywhere**, and that is the part
to keep: `dig_wants` yields one want per marked site whichever half it is
on, and `run_dig_crew` re-derives the half from the cell every cycle.
Someone adding a `Mode` field to `DigSite` to "make it explicit" is adding
the second representation this avoided.

A `Floor` cell takes no mark at all — there is nothing left to do to it,
and a site spawned over one would be a job the crew could never finish.
Clearing a mark retires the `DigSite` **unless it is still holding chip
progress**, so unmarking a wall you had already started on does not heal
it. *Progress* is the meter strictly between its ends: a full one is a wall
nobody has touched, and a spent one on a cell that is still solid is what
entropy leaves behind when it reverts a marked `Open` cell — `strike_rock`
refills that on the next swing anyway. Keeping either leaves an entity
drawn nowhere, wanted by nobody, and written to every save from then on.

**And a kept site is exactly why `run_dig_crew` has to check the mark
itself.** A cancelled plan does not look like a despawned entity: the site
survives holding its progress, `dig_wants` stops listing it, and
`schedule_base_labour` never takes a body off a post it has nowhere better
to send — so nothing upstream frees the digger. Without the check the crew
finishes a wall the player told it to leave.

The trap runs the other way through `Durability`, and it runs twice. The
first is the raid query: a `DigSite` carries `Durability` and no `Nest`,
which is exactly the pair `run_raid` used to mean "a building". A marked
box is up to 625 of them, so sweeps landed on rock instead of machines,
and a sweep that destroyed a site dropped the mark and every swing of chip
progress while `BaseGrid` still reported the cell solid — the wall healed
to full. Both raid target queries are `With<Structure>` now, for
`repair_system`'s reason: `Durability` alone has never meant "a building",
and a positive filter keeps the next carrier out by construction rather
than needing a fourth exclusion.

The second: entropy reverts a cut cell to *solid*, not to chipped, and a marked site outlives that revert
with its `hp` sitting at zero. `strike_rock` refills it to `max_hp` when it
finds one, or the next swing lands on a spent meter and opens a whole wall
for free — which makes the promise `BASE_ENTROPY_REFILL_TICKS` is there to
make read backwards.

### A dig site's two unreachable states are not symmetrical

**`hauling::post_reach`'s `BoxedIn`/`NoRoute` split is what decides whether
the base says anything**, and the two answers are deliberately given
different voices. A marked cell with every neighbour still solid is the
normal interior of any block a player marks — a 3x3 wing has one at its
centre before a single swing lands — and it resolves itself as the shell
comes down, so it is skipped **silently**. A marked cell that has a
standable neighbour and still no route to it is stuck until the player does
something about it, and **says so once**.

Once is enforced by `DigSite::announced_stuck`, a latch written by
`schedule_base_labour` and read nowhere else, following `set_machine_status`'s
rule exactly: entering a state is news, staying in it is not. The reason the
two states need different treatment is the one CLAUDE.md already gives for
the `BoxedIn`/`NoRoute` split itself — they leave the player different
errands. Collapse them into one silent skip and the only signal that a plan
is unworkable is gone; collapse them into one complaint and every interior
cell of every marked block puts a line in the log for the rest of the run.

`announced_stuck` is deliberately **not saved**. It is true of a
conversation rather than of the world, and a reload is exactly the moment
the player should be told again.

### Dig wants are appended last in `schedule_base_labour`, and the priority *is* the position in that list

**There is no priority field anywhere in the scheduler.**
`schedule_base_labour` builds one `wanted` list in order — worked orders,
then standing jobs, then dig sites — and then `wanted.truncate(staff.len())`
cuts from the **end**. Position in that list is the whole ranking, and
appending dig wants last is the entirety of what makes a spare body dig
while a needed one does not.

**Anything inserted above them silently starves production.** The diff that
follows runs against the *truncated* list, so a want that never survives the
cut is simply never posted, and nothing anywhere reports that it was
dropped. The failure mode is a base that stops running because the player
marked a corridor — which is the thing decision 7 of the slice-2 spec exists
to prevent, and it is held by an append site rather than by a check.

`dig_wants` is **structural rather than a stock count**, for
`work_orders::feeders_for`'s reason: what makes a site a want is that the
player marked it, not whether the base can currently pay for the substrate
that finishes it. A want that flickered as the shelf drained would walk
bodies on and off the frontier for the rest of the run. It is also sorted by
tile before it is returned, for `assembler_system`'s reason — bevy's
iteration order is not stable, and two diggers whose swings land in a
different order between runs would make the same base save differently.
`run_dig_crew` sorts its diggers the same way and for the same reason.

**A cell with no exposed face is not a want, and that exception is what
makes the truncation safe.** The rest of the scheduler tests reachability
*below* the cut — `can_walk_to_dig` refuses a boxed-in site silently and by
design, because the interior of any marked block is boxed in and resolves
itself as the shell comes down. Listed as wants, though, those interiors are
what the budget is spent on: they sort first in tile order, `continue` costs
no body, and the rim — the one or two cells a program could actually have
been sent to — is cut off the end of the list. A player's real save was
found in exactly that state on 2026-08-22: a 36-cell room marked out in open
rock, one cell of it reachable, six idle programs, and not a swing taken at
it in the rest of the run. Nothing was logged, because the only refusal
involved is the one deliberately kept quiet.

`hauling::has_station` is the shared predicate rather than a second reading
of what a face is. It is the half of `NoPost::BoxedIn` that does not depend
on who is asking — `from` ranks a target's faces and never adds or removes
one — which is why it can be answered before the bodies are counted, and it
costs four grid lookups rather than a walk. `NoPost::NoRoute` stays below the
cut where it was: a site with a face and no route is the player's errand and
says so once, so it is visible when it starves something, which a silent
refusal never is.

### Mining does not go through `battle::resolve_attack`

**Rock is hit, not rolled against.** `Game::strike_rock` takes
`Game::swing_damage` — the weapon band's mean plus `effective_atk`, floored
at 1 — and subtracts it. No hit chance, no crit band, no fumble, and no
`GameRng` draw for the damage at all. The argument is `Game::attack_nest`'s,
which CLAUDE.md already states for structures: a wall has no speed and
cannot dodge, and identical swings have to land identical damage or wearing
one down becomes a slot machine you cannot plan a wing around.

`swing_damage` is **shared with the crew rather than copied for it**, and
that is what makes a stronger program dig a wall out in fewer swings rather
than faster ones. Its band comes from `natural_range_of` — the one
conversion from an entity to what it swings for, giving a `Creature` its own
species move, a weapon's band where one is worn, and `PLAYER_UNARMED_DAMAGE`
only as the fallback the player (who carries no `Creature`) actually falls
through to. Naming the player's fists here instead was silent in exactly the
way a shared formula is supposed to prevent: every unarmed crew program dug
at the player's rate, so which program you put on a wall changed only its
`atk` and a Scrapper and a Medic cut at nearly the same speed. `BASE_DIG_TICKS_PER_SWING` is the crew's *rate* and
`swing_damage` is its *bite*; the two knobs mean different things and a
retune that confuses them turns levelling into a speed bonus for the base
instead of a reward for the player.

`BASE_ROCK_DURABILITY` is **never scaled by zone, depth or level.** The rock
is the same rock all run, so the thing that changes is the player: a wall
that takes about three swings at level 1 takes one late, and that *is* the
reward rather than a curve to tune. A scaled wall makes digging cost the
same forever, which is the one thing it must not do — the same shape of
argument that keeps mitigation off the level curve.

The one RNG draw in the whole action is the fragment roll on the break, and
it is `GameRng` on purpose: this is a live action rather than world
generation, so nothing about it has to be reproduced by a reload. Its
chance is bounded above by what flooring the same cell costs — a Blank
Substrate is four Core Fragments at a Lathe — so a dug cell returns a
fraction of what finishing it costs *by construction*. Raising it past that
ratio turns the wall into a fragment tap that undercuts the Mining Node,
and `mining_a_wall_never_pays_more_than_flooring_it_costs` holds it against
the real assets rather than against a number written in `tuning.rs`.

### A cost the base incurs is paid from the base's stores; a cost the player incurs is paid from their pack

**`stock::spend_from_base` is the base's side of that, and the dig crew's
tile is its one caller.** The crew cuts a marked cell and floors what it
opened, and the Blank Substrate it spends comes out of the same buffers
`work_orders::base_holding` counts and the stock strip lists —
`stock::output_buffers`, drained in tile order for `assembler_system`'s
reason. The player's `Inventory` is the **fallback**, tried only when no
shelf holds one.

It shipped the other way round: `crew_lays_tile` read the player's
`Inventory` and nothing else, on the argument that a tile costs one
substrate "out of the same store every build is paid from". That argument
was true of the code and true of `Game::lay_tile` and `deploy_structure`
either side of it — and it was still wrong, because the crew is the first
thing in the game that spends on the *base's* initiative rather than at the
player's keypress. The base's own Lathe made the substrate, its own hauler
walked it to its own Depot, and then the base could not spend it.

Found in a real save (2026-08-21), not in a test. A player marked two
cells, watched the crew cut both through, and watched nothing else happen
for the rest of the session: 12 Blank Substrate on the shelves, 0 in the
pack, both marks still standing. Every part of the machinery was working —
`dig_wants` listed both sites, `schedule_base_labour` posted two bodies,
`step_to_post` walked one out, `strike_rock` broke both walls — and the
whole feature was invisibly dead at the last line.

**Which buffers is the load-bearing choice, and it is the widest set on
purpose.** Depot shelves alone (`depot_holding`, what a hauler may fetch
from) would keep the crew from raiding a production line — and would leave
a base with no Depot reading `BS 12` across the top of the screen while
refusing to pave, which is the same silence in a new place. The strip is
the player's one statement of what the base has; a cost that cannot spend
what the strip counts makes the strip a lie. A three-tier rule (Depot,
then machines, then the pack) buys the line-protection back and was
rejected for being unreadable: no screen explains why one tile came off the
shelf and the next out of a Lathe.

**The silence was half the bug and has its own latch.**
`DigSite::announced_dry` says it once, beside `announced_stuck` and under
`set_machine_status`' rule — entering a state is news, staying in it is
not. Two fields rather than one, because the two leave the player different
errands: no route is a wall to cut, no substrate is a shelf to fill.
Neither is saved, so a reload says both again. There is no clearing branch
for the dry latch, unlike the stuck one: the state ends with the site,
since the tile that resolves it despawns the entity holding the latch.

`Game::lay_tile` is deliberately untouched and still pays from the pack
alone. It is a player verb, paid the way every other player verb is — the
asymmetry *is* the rule.

### Every routine that moves Integrity rolls a band

**Every routine that moves Integrity rolls a band, and the census is what
keeps it that way.** The mechanism long predates this: `battle::DamageRange`,
`DamageRange::centred`, `abilities::scaled_range`, and a `spread` field on
`AbilityEffect::Damage` and `Drain`. What was missing is that **not one of
the 77 shipped ability files ever authored a `spread`**, while all 34
species basic-attack moves have authored one since ranges shipped. So for
several releases every routine in the game dealt exactly one number, on a
field documented as giving mods damage ranges for free.

`Heal` gained the same field on the same terms in 0.13.24 and rolls through
the same `DamageRange` — so the low end is floored at 0 and both ends scale
with the invoker, rather than a second formula growing up beside the first.

**The serde default must stay 0.** That is what lets a mod's file parse
untouched, and it is exactly why the shipped roster needs a census rather
than a validator: a degenerate band is a legitimate authored choice and an
unauthored one is indistinguishable from it.
`every_shipped_integrity_routine_rolls_a_band` names the three variants
rather than matching `_ =>`, on `cell_mark`'s rule — an eleventh
Integrity-moving effect should fail to compile there rather than skip the
check.

**Spreads are authored at about 25% of `power`**, the median of the ratios
the species moves already use (they run 0.12 to 0.40). The band is
*centred*, so `DamageRange::mean` is unchanged — and `expected_damage` is
built on `mean`, so `balance_sim` passed the whole change without a single
curve moving. That is the evidence the retune is variance and not a
difficulty change.

**One draw whatever the width.** `DamageRange::roll` is written as an offset
from `min` rather than `random_range(min..=max)` precisely so a degenerate
band still consumes exactly one draw; that is what makes authoring a spread
on an ability unable to shift a seeded run's stream, and it is why this
landed with no seed churn at all.

The trap the change exposed: four tests asserted the determinism and *said
so in their comments* ("this ability's band is degenerate, so the roll is
exactly `scaled`"). Each now pins the property it was actually for — that
the affinity-scaled band is the one rolled from — with a guard asserting
the wrong-actor or neutral-affinity band sits clear of it, since containment
in a band nothing else could produce is what makes the assertion mean
anything.

### `Game::choose_wild_action` (`game/combat_policy.rs`) is the one place a wild program's swing is decided

**`Game::choose_wild_action` (`game/combat_policy.rs`) is the one place a
wild program's swing is decided** — which move, and at whom, as a single
joint choice. Both the trained policy and the uniform baseline exit
through it, so `roll_enemy_target` keeps exactly two callers: that
fallback, and `wild_retaliate`'s routine branch.
The score is `ln(slot_aggro_weight(..)) + w·features`, and the `ln` is the
load-bearing part: softmax exponentiates, so `exp(ln 3) == 3` and an
all-zero policy reproduces today's distribution *exactly* rather than
approximately. Without the prior, all-zero weights would make every target
equally likely — the front three dropping 27%→20% and the back two rising
9%→20%, a real change caused by wiring alone. The aggro table therefore
enters with a pinned coefficient of 1.0 and is **never** a learned feature.
What it cannot do is survive being *out-scaled*, which is the next entry.

### Three policy features are pinned to zero in the shipped weights, and that is a design boundary rather than a tuning accident

**Three policy features are pinned to zero in the shipped weights, and
that is a design boundary rather than a tuning accident.**
`target_is_player`, `target_bracing` and `target_def_rel` are held at 0 by
the trainer's `--pin` flag. Left free, training learns to kill the player
and ignore the party (companions took 0.2% of swings), and — with the
first two pinned — to dodge the brace by reading the +6 DEF it grants
instead. `bracing_still_draws_more_fire_under_the_shipped_weights` is the
census; it caught both routes and is what a retrain fails rather than
shipping past. `DEFEND_AGGRO_WEIGHT` was raised 4→7 for the same reason
and its doc comment carries the arithmetic: even fully pinned, a
damage-aware policy has a reason to walk past the tank, and neither
pinning nor `ENEMY_POLICY_TEMPERATURE` can flip that sign — temperature
divides the prior and the learned term alike.
**`balance_sim` is blind to all of this** (RNG-free, models no abilities),
so the usual balance gate does not apply; the arena is the instrument and
`ENEMY_POLICY_TEMPERATURE` is the only shipping control. Weights are an
asset, so deleting `assets/policies/enemy_battle.ron` restores the
pre-policy game exactly and is a supported way to play.

### `is_boss` marks an apex species, and any species can be rolled into a boss

**`is_boss` marks an *apex* species — always a boss, never engine-scaled —
while any species can be *rolled* into one.** The flag used to be the whole
of what a boss was: an ordinary spawn of a species whose `.ron` base stats
are large, so the gap was data and moved by editing two files. That is
still true of the two shipped apex species, and is why nothing scales
them — a blanket multiplier would discard the authoring.

What changed is that boss-hood stopped being a property of the species
alone. Outside the opening ring a per-tile roll at `BOSS_SPAWN_CHANCE`
marks the spawn, drawing from the whole window: apex where the danger step
admits one, an ordinary species where it does not. A rolled boss takes
`tuning::BOSS_STAT_MULT` and nothing else — no rare tier on top, for the
same reason an apex spawn never rolled one, since the multiplier is the
whole of what it is worth. So the shape is "easy bosses early, hard bosses
deep" without a second difficulty axis: `APEX_ENTRY_STEP` is the only new
number, and before it every boss is a rolled one.

Two halves hold the fact. `components::Boss` is written at every boss
spawn, rolled or apex, and saved (`CreatureSave::boss`, additive behind
`#[serde(default)]`, so no `SAVE_FORMAT_VERSION` bump). `Game::is_boss_
creature` is the **one door** and keeps the species fallback beside the
component, because a fixture that hand-spawns an apex species outside
`spawn_pack` never gets a component and must still be a boss. Three
readers go through that door rather than reading `SpeciesDef::is_boss`:
the payout gate in `award_loot`, the view builder, and the arena's boss
census. A fourth reader added against the species flag would silently pay
nothing for a rolled boss.

A boss still spawns as its own group, and past zone 1 also brings an
escort group drawn from its own tile's `habitat_pools` — which is the
right pool for both boss sites, since the surface roll passes its own tile
and `rouse_lair` passes the Stack entrance whose biome chose the boss. The
escort is never itself a boss. Zone 1 fields one group, so the opening
zone's boss is the one that still fights alone.

`balance_sim` gates none of this: it models no bosses at all (see
`toughest_ordinary_species`, which excludes them), so `BOSS_STAT_MULT` is
an arena question. Measured 2026-08-18 on `dev-arenas/lair-on-curve.ron`,
the one shipped scenario whose guardian is drawn ordinary: 2.6 rounds at
1.0 against 3.9 at 1.75, win rate 100% either way. `geared-vs-boss` and
`deep-lair` did not move at all, which is the apex exemption showing up as
a measurement.

### A species' danger band is derived, and the window is what decides where it spawns

**A species' danger band is derived, and the window is what decides where
it spawns.** `SpeciesDef::danger_band` reads `growth_multiplier` against
`GROWTH_TIERS`, snapping a between-rungs value to the nearest — the same
concession `tier_budget` makes on the same midpoints, so a mod is never
refused, it just stops being readable against the shipped ladder. It is
derived rather than authored for the reason `affinity_class` is: a rung is
a fact about numbers the species already carries, and a second authored
field is a second thing that can disagree with the first. `is_boss` is
read **first**, because both shipped apex species sit at 2.0 — off the
ladder's top — and reading the multiplier first would file them beside the
ordinary hard species.

The window is `tuning.rs`'s: band `b` is live from `b * TIER_ENTRY_STEPS`
through `+ TIER_WINDOW_STEPS` inclusive, apex from `APEX_ENTRY_STEP`. It
is read against `Game::danger_steps`, the **same scalar** the two
group-size curves already take — the zone step on the surface, the zone
step plus the frame depth underground — so there is no second difficulty
axis to keep in step with
the first. The top band and apex **never exit**, whatever the constants
say: steps are unbounded because zones and depth are, so a closed top
empties the world past step 7.

Two things about the plumbing are easy to get wrong. `habitat_pools` takes
`depth` as a **parameter** rather than reading the party's locale, for the
reason `SpawnEscalation`'s doc already gives — ambient surface spawns and
nest respawns keep rolling on every tick while the party is underground,
and a step read inside would size those from the party's depth. And both
windowed pools build on the sorted primitives, because the draw picks **by
index**: concatenating two sorted vectors does not give a sorted one, which
is why `pick_habitat_species` sorts the union.

The per-biome fallback is load-bearing rather than defensive. Where the
window admits nothing a biome holds, `windowed_matches` falls back to the
band **nearest** the window, ties resolving upward. That fires against the
real assets at both ends: Deadlock ships no band-0 species and OpenGrid
no band-2. `every_biome_fields_something_at_every_danger_step` is the
census, and the honest fix for either hole is a species file, not a wider
window. Apex is never a fallback — a boss is a rare outcome the window
admits, not a biome's last resort.

### Which side of the ground a boss dies on decides what it pays, and one of the two answers is the game's only source of the breaching currency

**Which side of the ground a boss dies on decides what it pays, and one
of the two answers is the game's only source of the breaching currency.**
`award_loot` splits on `stack_pos()`: underground, `STACK_BOSS_PORTAL_
FRAGMENT_DROP` times the frame's depth — and nothing else in the game
pays a Portal Fragment at all, so a run that never descends never
breaches. On the surface, `surface_boss_loot`'s zone-banded gear
instead. Three traps follow. The gate is `is_boss` **and** underground,
which is right because `stack_encounter_pack` never rolls a boss, so a
lair guardian is the only boss that can be underground and its escort is
ordinary species — a payout keyed on "died in a lair" would pay for the
escort too. `surface_boss_loot` derives its band from `ItemDef::value`,
giving that field a second meaning documented in
`assets/items/README.md`: a mispriced equippable trades fine and drops
at the wrong point in the run. And `pick_lair_species` used to *fall
back* to the toughest ordinary program when a biome fielded no boss,
returning `is_boss: false` — so removing a habitat from the last boss
covering some terrain made every stack under it unbreachable while
looking like a tuning edit. **That is closed.** The guardian is drawn from
the biome's apex pool **ungated by `APEX_ENTRY_STEP`**, with the windowed
ordinary pool as the fallback, and is marked a boss either way, so a biome
with no eligible apex species yields a rolled guardian that pays normally.
The ungating is itself a fix (2026-08-24): the draw used to be windowed at
the lair's own depth, and since a stack runs `STACK_FRAMES_MIN` 2 to
`STACK_FRAMES_MAX` 6 frames while apex species need step
`APEX_ENTRY_STEP` = 4, every lair shallower than depth 5 silently served
an ordinary species with `BOSS_STAT_MULT` on it. The player's report was
that dungeons had no bosses at the bottom. The step gate still holds for
ambush and wild boss rolls, where an unheralded apex is the thing it
exists to prevent; a lair is walked into deliberately and announces
itself. `a_lair_guardian_is_a_boss_even_where_the_biome_has_no_apex_
species` pins it against a db with the apex species removed, since both
shipped ones list all four biomes and the case is otherwise unreachable.
`every_biome_a_stack_link_can_open_in_fields_a_boss` (`species.rs`)
remains the census over the shipped roster, and it checks only walkable
biomes because `spawn_surface_links` refuses an unwalkable tile —
`Biome::walkable` is the one predicate both ask.

### `Game::adopt_program` is the one way a program joins the roster without being beaten in a fight

**`Game::adopt_program` is the one way a program joins the roster
without being beaten in a fight.** Two callers with opposite premises —
`adopt_orphan` takes something abandoned in a Stack dead end and charges
a taming catalyst, `grant_nest_cache` takes what survived a nest's
wreckage and charges nothing — agreeing completely on what *becoming* a
companion means. It was one copy until the nest orphan landed, and
`install_innate_routines` is the step a third copy would drop. What it
deliberately omits is load-bearing too, and `adopt_orphan`'s doc holds
the reasoning: no `StackSpawn` (`end_battle` would despawn a companion
that never fought), no XP, no `Party` push. Neither caller checks
`pet_capacity` inside it, because one refuses the action outright and
the other has already destroyed the thing that was paying.

### Trace's group-size lever is a `spawn_pack` parameter, never a resource read inside it

**Trace's group-size lever is a `spawn_pack` parameter, never a resource
read inside it.** That function's doc already records this leak happening
once with `depth_mult`: surface ambient spawns and nest respawns keep
rolling on every `tick` while the party is underground, so anything the
spawn reads off the party's own state scales those too. The multiplier is
also clamped back under `zone_group_cap` (`trace_group_ceiling`) — Trace
makes the party reach their zone's ceiling faster, never raises it, and
that clamp is the only reason the lever is inert in zone 1 rather than
turning a cap of 1 into a pack of 3.

### A Stack cell that can be used up needs both halves

**A Stack cell that can be used up needs both halves.** A cache, a seal,
a breakpoint, an orphan and a lair each have a `CellKind` in the frame
*and* a record in `FrameMemory` saying it has been spent. Both halves live
in `game/stack_features.rs`. Both views consult that record
(`cache_unopened`, `seal_open`, `breakpoint_spent`, `orphan_present`,
`lair_cleared`) so an
emptied cache stops being advertised; forget the record and the thing
refills every time the party steps off and back on. `Fault` and
`Corruption` are the counter-example that proves the rule rather than
exceptions to it: neither is *used up*, so neither has a record — a fault
drops you every time and corrupted ground bites every time you cross it.

### An orphan's *species* is pinned to the frame seed; its *stats* are not, and the split is deliberate

**An orphan's *species* is pinned to the frame seed; its *stats* are not,
and the split is deliberate.** `Game::orphan_species` draws from a local
`StdRng` salted off `FrameSpec::rng_seed`, because the party has to be
able to see what a program is before spending an `ice_breaker` on it —
so the answer has to survive a save/load, which a `GameRng` draw would
not. Everything else about the creature (`spawn_wild_creature_scaled`'s
potential roll and wild routines) comes off `GameRng` at adoption, like
every other spawn in the game: *what it is* is a property of the place,
*what it is worth* is a property of the moment you took it.
The seam that keeps those two from being one copy of the biome rules is
`Game::habitat_pools` — `pick_habitat_species` split at the point where
it starts spending `GameRng`, so both callers share the habitat and
opening-ring logic and differ only in which RNG draws from it. Don't
copy the pool-building into a third caller; widen that function.

### There is one way into a frame, and it is `Game::enter_frame`

**There is one way into a frame, and it is `Game::enter_frame`.**
Descending by link, climbing up one and falling through a fault differ in
exactly one thing — which cell of the newly generated frame the party
lands on — and agree on the rest: generate, install `CurrentStack`,
rewrite `Locale`, `remember_view`. That spine lives once. The landing is a
closure over the generated frame rather than a cell, because two of the
three callers cannot name theirs until the frame exists: an ascent lands
on `link_down`, and a fall is placed by distance in `stack::fault_landing`.
A fourth way in goes through here, not beside it.

### There is likewise one way to arrive *on a cell*, and it is `Game::arrive`

**There is likewise one way to arrive *on a cell*, and it is
`Game::arrive`.** Same argument one level down: a step, a phase through a
wall (`AbilityEffect::Phase`) and a wild jump (`AbilityEffect::Jump`)
differ in how the party got there and agree completely on what happens
next — `bleed_corruption`, `open_cache`, `rouse_lair`, `trip_breakpoint`,
`take_fault`, `maybe_stack_encounter`, in that order. Two orderings in it
are load-bearing and documented on the function: corruption first because
it is a property of *arriving* rather than something the cell offers, and
the fault before the encounter roll so a party that fell rolls in the
frame they landed in. It deliberately does **not** call `remember_view` —
each caller does that itself first, and `step` calls it even on a blocked
step, which is a rule about facing rather than about arriving. The
regression that matters is a fourth arrival path quietly skipping the
tail, so `a_jump_fires_the_arrival_tail` asserts *behaviour* (a cache
emptied by a jump) rather than that a function was called.

### `Game::run_field_routine` is Stack-only for two of the three effects it runs, and `require_surface` is not what does it

**`Game::run_field_routine` is Stack-only for two of the three effects it
runs, and `require_surface` is not what does it.** That guard exists for
actions reaching zone-map state through a `Position` pinned to the
entrance tile; `Phase` and `Jump` have the opposite problem, reading and
writing `Locale::Stack`'s own coordinates, so what they need is the
*presence* of that locale and the refusal is `Game::stack_pos` returning
`None`. `AbilityEffect::field_only` is the one predicate saying which
effects reach this path at all, read by `field_routines`,
`battle_special_options`, `wild_routine_ready` and `use_ability`'s
`unreachable!` — that arm is only unreachable because the other three
agree with it.

### A lethal Wild Jump never writes `Locale`

**A lethal Wild Jump never writes `Locale`.** `die_in_the_rock` damages
the player and stops; the party does not briefly stand inside the rock and
get rescued. Rock is the one `CellKind` that is both unwalkable *and*
sight-blocking, so a party inside one is exactly the occluder trap doors
sprang — a first-person view of flat wall and a map truncated to the
party's own row. Not writing `Locale` is what makes that state unreachable
rather than merely unlikely, so neither `view_cone` consumer needs a new
exception. What happens next was already built: Forgiving warps the party
out through `stack::surfaced`, permadeath ends the run.

### The wielded program's bonus is computed live, and that is what makes both destruction paths correct without either knowing it exists

**The wielded program's bonus is computed live, and that is what makes
both destruction paths correct without either knowing it exists.**
`resources::WieldedProgram` is allowed to hold a stale `Entity`;
`Game::wielded_program` drops one whose `Stats` are gone, and every read
goes through it. So selling, extracting, fusing away or killing the
program you are holding ends the wield by omission — neither
`dissolve_tamed_program` nor `fuse_companions` was touched, and a third
destruction path added later inherits the same immunity for free. The
regression to head off is a later "fix" adding an explicit clear to both:
`selling_the_wielded_program_ends_the_wield` and
`fusing_away_the_wielded_program_ends_the_wield` assert the safety net
rather than that anything was cleared, which is what makes the omission
legible as a design. It is deliberately *not* a field on `Equipment`
either — that slot holds an `EquippedItem`, and a bonus baked into
`Stats` by an equip that can never be matched by an unequip is the
`EquippedItem::fusion_tier` trap again, permanent free stats with no
record of where they came from.

### The wielded program's proc runs as the *program*, not the player

**The wielded program's proc runs as the *program*, not the player.**
`Game::proc_wielded_routine` hands the program to `use_ability`, which
reads `ability_user_level`, `ability_affinity` and `effective_atk` off
whoever it is given — so a proc scales by that program's level, species
affinity and ATK, and *which* program you wield is what the feature is
worth. Two orderings on `party_member_attacks` hold it together: the proc
fires after the strike's kill is resolved (so a routine never lands on a
corpse) and not at all once `finish_group_member` has ended the battle.
`wieldable_routines` is the one predicate for what may fire, excluding
`field_only` effects (no battle recipient) and `Decompile` (a free capture
roll spending an unauthorised ICE Breaker, and resolved by group index
rather than by recipient anyway). The `W` key that starts all this is an
easter egg: `render/party.rs`'s `companion_help` is a function rather than
two inline strings purely so a test can hold the screen to never naming it.

### Destroying a structure also has two paths

**Destroying a structure also has two paths.** `damage_structure`
(`game/upkeep.rs`, raids) and `remove_structure` (`game/building.rs`,
demolition, which cascades from a Home) each clear worker `Task`s and
despawn inline. Anything that must happen as a structure comes down needs
wiring into both — `announce_lost_shelf` is the one thing currently in
that position.

### A species' class is derived and has exactly one derivation

**A species' class is derived and has exactly one derivation.**
`SpeciesDef::affinity_class` reads the single affinity axis a species
raises; `AffinityClass::of_axis` is the mapping, asked of a species'
affinities from one end and of an ability's category from the other.
`Game::creature_class` is the only door from an entity to it. It was
`#[cfg(test)]` until the base jobs needed it, and the two censuses now
look their row up from the shipping function rather than deriving it —
which is what makes a census passing evidence about the game rather than
about the test. `None` (a boss, or a mod raising two axes) must mean *no
base job* rather than a default class.

### A species' *stat block* is derived too, and its one definition is `species::stat_shape_faults`

**A species' *stat block* is derived too, and its one definition is
`species::stat_shape_faults`.** Total == growth band's budget x class
weight exactly, axis shares to ±1, a speed band per class, and a
`growth_multiplier` on one of `GROWTH_TIERS`' three rungs — the budget
being a step function is why the last of those matters, since a value
between rungs derives a whole block from a number nobody chose. It moved
out of `#[cfg(test)]` in 0.8.2 for the same reason `creature_class` did:
a second consumer appeared. The roster tuner
(`crates/launcher/src/tuner/constraints.rs`) had never known any of it,
and its first real search proposed 14 field moves of which 13 were
invalid by construction. Three things about the shape are load-bearing.
It returns the **verdict** rather than the ingredients — exporting
`tier_budget` and the shares would have left the comparison itself
duplicated in the tuner, which is the copy nobody runs. It returns
**every** fault rather than the first, because the census used to stop at
the earliest failure and that is how two budget violations were reviewed
as a speed problem. And **bosses are exempt**, which is not an
optimisation: they carry no affinities, and a boss's ATK was the one move
the tuner has ever found worth having. Nothing in `SpeciesDb::load_dir`
calls it, so a mod is never refused by it — what a mod loses is the
guarantee that its role is readable from its numbers.

### Two censuses are reported to the tuner rather than enforced on it, and which is which is a cost question

**Two censuses are reported to the tuner rather than enforced on it, and
which is which is a cost question.** `balance_sim::reach_rule_verdict`
runs a level search per call; `species::extraction_aptitude_faults` is a
property of the whole roster's *distribution* rather than of any one
move. Neither can be paid on every candidate, so both run once on the
winner and land in `report.md` — a human is reading a diff by then, and
the "no silent caps" rule is what makes reporting the right answer rather
than a weaker one. Both were promoted out of their own tests and both
censuses now assert through them, so the tuner's copy cannot drift.

### Three of the five classes do something at a post, and each of the three sits in a different system

**Three of the five classes do something at a post, and each of the three
sits in a different system.** A Leech's `LEECH_YIELD_BONUS` is added
inside `resolve_gather_cycle`, a Bastion's `BASTION_DEF_MULTIPLIER` in
`run_raid`, a Medic's `MEDIC_REPAIR_PER_INTERVAL` in `structure_regen`.
Three things about them are load-bearing and none is guessable from the
others. The Leech bonus rides the **scaled** branch only — a banked or
`flat_payout` node keeps its flat 1, because that flatness is the whole
of what holds an uncapped research bank against a fixed ladder — which is
why `CycleModifiers` carries the *class* rather than a finished bonus a
caller computed without being able to see the exclusion. The Bastion job
is a multiplier on mitigation that already existed: `run_raid` finds its
defender by `Task::target` alone, so every posted program has always
mitigated by its DEF, and a test asserting the Bastion figure alone would
pass against a build where DEF had simply been doubled for everyone. The
Medic job counts `TaskKind::Guard` **only**, deliberately narrower than
that defender — mitigating is a passive property of whoever is standing
there, mending is what the post *is* — and `structure_regen`'s early
return had to start asking about both sources, since a base with no Patch
Node is the case a posted Medic is most for and was the one case that
returned before reaching the repair.

### A structure's upgrade tier is bounded twice, and the two bounds mean different things

**A structure's upgrade tier is bounded twice, and the two bounds mean
different things.** `Game::upgrade_ceiling` is `min(def.max_tier, zone)` —
the def's ceiling is permanent, the zone's is not, and reaching zone *N* is
what unlocks Mk*N*, so nothing upgrades at all before the first breach.
`upgrade_structure` checks them in that order deliberately: a maxed-out
structure in a shallow zone must read as *finished*, not as waiting on a
breach it would never benefit from, and both come before the materials
check so the player is never sent to find fragments they couldn't have
spent. The ceiling is a function rather than an inlined `min` because
`EntityView` carries it (with `max_tier` beside it) so the upgrade menu can
label a stalled row — and *neither value alone* says whether the stall is
temporary. A structure at its zone ceiling stays listed rather than being
filtered: `app/group_menu.rs` drops a row whose screen would be empty, so
filtering would delete the whole Upgrade row for all of zone 1 and a player
who had never breached would never learn upgrading exists.

### "Raid" is the code's word and "GC Entropy Sweep" is the player's

**"Raid" is the code's word and "GC Entropy Sweep" is the player's.**
`MessageKind::Raid`, `RAID_*`, `Game::raid_check` and the `.ron` fields
`raid_defense`/`raidable` deliberately kept their names through the
2026-08-05 rename — the fields are mod schema, so moving them would have
broken every existing mod for a piece of vocabulary. (This entry used to
claim the enum is saved as well. It is not: `MessageKind` has no
`Serialize` derive and never reaches disk, which is why appending
`MessageKind::Complete` in 0.8.36 cost no bump.) Anything a player reads says sweep; anything a
modder or a compiler reads says raid. New player-facing text follows the
first rule, and note the phrasing trap the rename hit: the sweep is a noun
phrase, so "takes N sweep damage" doesn't work and the line reads "loses N
Durability to a GC Entropy Sweep".

### You run a routine; the noun is an invocation

**You *run* (or *invoke*) a routine; the noun is an *invocation*.** "Cast"
and "spell" are fantasy words and this setting has none. Unlike the Raid
rename one entry up, this one went all the way through the identifiers:
`Game::cast_field_routine` → `run_field_routine`, `Mode::FieldCast*` →
`Mode::FieldRoutine*`, `FieldCastTarget`/`FieldCastPick` →
`FieldRoutineTarget`/`FieldRoutinePick`, `scales_with_caster` →
`scales_with_invoker`. That is the more expensive choice and it was made
deliberately — Raid's argument is that `.ron` field names are mod schema,
and **nothing here is**: no asset field was ever named `cast`, so the whole
rename is internal and breaks no mod.

**Two collisions decide the vocabulary, and they are why "cast" could not
simply become "run" everywhere.** A *run* is a playthrough — "a seeded
run's RNG stream", "survive a run" — so "a refused run" reads as a lost
game. And a *runner* would sit beside hauling's own bodies. So the verb is
**run**, the agent noun is **invoker**, and the event noun is
**invocation**; a blanket verb substitution produced "the whole run rather
than per recipient" and "leave a hostile's running untouched", both of
which had to be walked back by hand.

Two things are deliberately left alone, and both are the word in its
programming sense rather than its fantasy one: a float-to-integer
conversion in `game/trace.rs` and a raycast in `game/stack_view.rs`. Both
were reworded anyway, because the gate is cheaper to keep absolute than to
carve exceptions into.

`no_player_facing_text_says_cast_or_spell` walks the ability, item,
structure and species databases plus the parsed help pages. It matches
**whitespace-and-punctuation-delimited tokens, never substrings** — the
shipped ability id `broadcast_storm` and the ordinary prose "spelled out"
both contain the letters, and a substring rule would fail on content that
is entirely correct. It reads *parsed* help pages for the same reason
`no_shipped_help_page_names_a_hidden_key` does: `assets/help/README.md` is
schema documentation, not a page. Player-facing strings assembled in Rust
are **not** covered — there is no way to enumerate them from a test — so
that half is held by review.

### An item's price is bounded twice, and the second bound is the one that isn't obvious

**An item's price is bounded twice, and the second bound is the one that
isn't obvious.** `ItemDef::value` feeds `Game::item_value`, the one place
a price is decided (`sell_item`, `buyback_unit_cost` and the trade screen
all read it; `sell_rate` is the trader's multiplier on it, not the price).
Build salvage is deliberately sellable and a Mining Node produces it
forever, so **a craftable worth more than its ingredients is an infinite
Credit loop** — that much is guessable. What isn't: a `work.produces`
structure makes its item out of *nothing* on a timer, so that item's value
is really a Credit-per-tick rate the recipe ceiling cannot see. The
Compiler used to be the live case — printing ICE Breakers every 8 ticks
with `flat_payout`, where pricing them at their 3-Fragment recipe would
have beaten a Mining Node nearly fourfold. It now `assembles` them out of
a neighbour's Core Fragments instead, so the ICE Breaker is bounded by the
recipe ceiling like anything else and no shipped structure sets
`flat_payout` at all. The second bound is therefore currently slack, not
gone: it still covers the four remaining `work.produces` structures, and
it is what would catch the next one. Both bounds are asserted over the
real assets
(`no_craftable_item_is_worth_more_than_its_ingredients`,
`every_base_produced_item_sits_at_the_floor_price`), so a retune that
breaks one fails rather than quietly minting money. Worth comes from what
a base can't manufacture, which is why the ladder runs printable 1 →
scavenged 3-8 → standard 12-16 → researched 20-60 → premium 80-120.

### A worn item and a candidate to replace it are scaled at two different levels, and that is the point rather than a bug

**A worn item and a candidate to replace it are scaled at two different
levels, and that is the point rather than a bug.** Gear locks in the zone
level it was equipped at (`EquippedItem::level`) and doubles per level
(`GEAR_LEVEL_GROWTH`), so `equip_swap_rows` measures the worn copy at its
*recorded* level and every candidate at the *current* zone's. Collapsing
those to one level would hide the case the screen exists for: a spare copy
of the weapon already on your back is a real upgrade after a breach.
`stat_summary` is the one formatter turning those stats into `+4 ATK` —
the inventory tag, the equipped panel and the picker's two columns all
call it, which is three copies avoided, not a style preference.

### A carried copy of gear is one value, `items::GearCopy`, and `Inventory` is by definition the *plain-copy* store

**A carried copy of gear is one value, `items::GearCopy`, and
`Inventory` is by definition the *plain-copy* store.** A copy is
`(item, rarity, tier)`: it earns a place in `components::GearCopies` by
having been fused (`ITEM_FUSION_COST` copies in, one at the tier above
out) or by having dropped at a rare tier. `GearCopy::is_plain` is the one
predicate deciding which store, and `Game::count_copies`/`take_copies`/
`add_copies` are the only three functions that ask it — as three copies of
that boolean, a drift would write a copy to one store and look it up in
the other, which reads to a player as gear vanishing out of cargo.
That split is the whole reason none of this touches the production chain:
recipes, `Stock`, `assembler_system`, hauling and banking all read
`Inventory`, so none of them can meet a special copy and none needs a tier
or rarity rule. Nothing puts a player's copy into a `Stock` either, so the
machine half is unreachable rather than merely untested;
`a_fused_copy_is_not_a_recipe_ingredient` walks the bench half with a
modded recipe, since no shipped one is priced in equipment.
Because copies differ, **every entry point naming an item takes the whole
copy** (`equip`, `fuse_item`, `sell_item`, `buy_back`, `erase_item`,
`SwapChoice::Equip`), and `PlayerStatus::inventory` is
`Vec<InventoryRow>` — one row per `GearCopy` — rather than a pair a caller
could sum across. Fusion matches on the whole copy too, so an Overclocked
copy only fuses with another Overclocked one: two copies that differ are
not two of a thing, and there is no midpoint tier for a mixed pair to
land on.

### `Game::copy_bonus` is the one expression for what a piece of gear is worth, and the order of its three axes is load-bearing rather than stylistic

**`Game::copy_bonus` is the one expression for what a piece of gear is
worth, and the order of its three axes is load-bearing rather than
stylistic.** `scaled_for_level` then `fused_for_tier` then `for_rarity`,
over a base the affix has already been added to. Two of the three carry a
per-step floor and a floor does not commute with a multiplier, so a call
site that reordered the chain would silently change the stat — and since
`apply_equipment_delta` writes the result straight into `Stats`, an unequip
computing a differently-ordered or differently-scaled figure from its equip
welds the difference permanently into the wearer's base stats with no
record of where it came from. That is `EquippedItem::fusion_tier`'s
documented trap reached by two new routes at once, which is why
`EquippedItem` stores a whole `GearCopy` rather than loose fields:
forgetting a property is not expressible. `worn_bonus` is now just
`copy_bonus` at the level a worn copy remembers; equip, unequip, fuse,
`strip_gear`, `gear_bonus` and the manifest resolve through it.
`the_gear_axes_do_not_commute_so_the_order_is_load_bearing` and
`unequipping_a_rare_copy_leaves_no_bonus_behind` are the pair that hold
it, and both were mutation-checked rather than merely written.
**Sharing the formatter was never enough, and the affix is how that was
learned.** `worn_bonus` was `pub(crate)`, so the four screens that price
gear — the inventory tag, the swap picker's candidate and worn columns,
and the gui's equipped panel — could not call it and each rebuilt the
chain by hand out of `equipment_of`. They agreed while a `GearCopy` had
the properties they were written against and then all four dropped the
affix at once, understating a row by *the affix times the zone*: an
"Overdriven Kinetic Edge" priced as a bare Kinetic Edge, +6 ATK against
the 15 equipping it actually granted at zone 3, and the equipped panel
dropped the rare tier too (27 ATK shown as 6). `stat_summary`'s doc had
promised all four worked "over the same `scaled_for_level().
fused_for_tier()` pair", which is the "a doc claiming to mirror other code
must be a call, not a copy" rule failing in the one direction it is
hardest to notice — the copies were *right* when written. Fixed 2026-08-13
by making `copy_bonus` `pub` and the three axis methods on
`EquipmentStats` `pub(crate)`: outside the engine the hand-rolled chain
now fails to compile, which is the barrier rather than the convention.

### A copy's quality is the fourth axis, and it is an integer

**A copy's quality is the fourth axis, and it is an integer.** `GearCopy`
gained a `quality: u8` — a percentage of the item's authored bonus, 100
meaning "compiled exactly to spec" — so that how well *this* copy was made
is a property of the copy rather than of the item. The design argument is
in `docs/superpowers/specs/2026-08-21-item-quality-design.md`; the phased
build is `docs/superpowers/plans/2026-08-21-item-quality-plan.md`.

**Why an integer and not an `f32`.** `GearCopy` is the key of the
`components::GearCopies` ledger — `add`, `count` and `take` find their rows
by `==` — and `EquippedItem` holds the same key so an equip and its unequip
can be matched. An `f32` field takes `Eq` with it, and with `Eq` the whole
keyed-by-value seam. A `u8` percent has more resolution than the band
needs and costs nothing.

**Why the `serde` default is a named function.** `#[serde(default =
"default_quality")]`, never a bare `#[serde(default)]`. `u8`'s own
`Default` is 0, so the bare form would load every piece of gear in every
existing save at 0% of its authored bonus — a total loss of stats
presenting as a balance bug rather than as a failed load, which is the
worst shape a save fault can take. The same argument holds three times
over: a *carried* copy rides `serde` through `data.player.gear_copies`, a
*worn player* copy rides three flat `PlayerSave` fields, and a *worn
companion* copy rides `EquippedItemSave` — three independent defaults, so
`a_pre_quality_save_loads_its_gear_as_designed` strips the field out of a
real save file on disk and each route's default fails that test on its own.
A RON round-trip in memory cannot prove a defaulting fault; only a file
can. The field is additive on named structs, so it costs no
`SAVE_FORMAT_VERSION` bump.

**Why it sits third in `copy_bonus`'s chain.** Affix into the base, then
`scaled_for_level`, then `for_quality`, then `fused_for_tier`, then
`for_rarity`. `for_quality` deliberately carries **no** per-step floor —
the two axes after it have one because a *discrete rung* has to be
observable at the 1..=4 points gear ships at, while quality is a continuous
gradient that a floor would flatten onto one number. Being floor-free is
exactly why it cannot go last: applied to an unscaled 4-point stat it is
eaten by rounding, and worse, it inverts the rare ladder. Worked, at level
1 on a base-4 stat: quality last gives a `Silver` copy at 70% `round(4 *
1.5 = 6 * 0.7) = 4` against an `Ordinary` copy at 130% `round(4 * 1.3) =
5`, so the rarer copy prices *below* the common one and the row colour is a
lie about which is better. Quality third gives 5 against 5 — the ladder
stands. `the_rare_ladder_survives_the_whole_quality_band` is that
comparison; `a_rarer_copy_beats_an_ordinary_one_of_equal_quality` is the
honest form of the rare guarantee, which is *against a copy of equal
quality* rather than globally.

**The band is the engine's and the palette is the renderer's.**
`items::quality_band` buckets the percentage into four rungs against three
`tuning.rs` cuts, centred so `QUALITY_DEFAULT` reads as no change and every
copy in every existing save is repainted by nothing. The thresholds live in
the engine because several renderer sites will read them, and that is the
argument `Rarity::label` and `Game::copy_name` already make; the colour and
weight stay the renderer's, because a band carrying an emphasis as well as
a hue is not expressible as a `Color`.

**`Game::roll_quality` is the one formula and the one clamp, and it lives
beside `roll_gear_rarity`.** A per-copy axis is rolled from more than one
file — `grant_gear_drop` in `game/combat_rewards.rs`, and crafting in
`game/crafting.rs` — so the ladder belongs where both callers reach it
rather than in whichever of them was written first, which is exactly the
argument `rarity_for_roll` already makes for the tier ladder shared between
programs and gear. A drop passes the flat `QUALITY_DROP_BASE`; crafting
passes a floor built out of a bench tier, a perk and the careful toggle.
The spread is drawn **in steps** of `QUALITY_STEP` rather than drawn fine
and rounded: rounding a uniform draw onto a lattice gives the two end
buckets half the width of the others, which biases exactly the ends of the
band the player is reading for. The sum is taken in `u16` because a
developed base's floor legitimately exceeds `QUALITY_MAX` and must saturate
rather than overflow the `u8` the band is expressed in.

**The quality roll is the third roll in `grant_gear_drop`, and last on
purpose.** For a given seed a dropped copy's rare tier and affix are
exactly what they were before quality existed, so only what follows the
drop in the shared stream moves. It sits below the non-equippable early
return, so a material still spends **no** draw —
`a_material_drop_spends_no_rarity_roll` is the guard, and every kill in the
game drops a work resource, so a draw there would shift the stream on
essentially every fight. `QUALITY_DROP_BASE` is deliberately below the
crafting floor: leaving drops at a flat `QUALITY_DEFAULT` would let an
average find beat a bad craft, and giving them the crafting band would mean
a base conferred no reliability advantage. The world does not make good
gear; your base does.

**An equipment drop no longer stacks in `Inventory`**, because a rolled
quality is almost never `QUALITY_DEFAULT` and so `GearCopy::is_plain` goes
false. That is the fourth `&&` doing its job, and it is the same
consequence the spec accepts for crafting, arriving on the drop side one
phase early. `add_copies`, `count_copies` and `take_copies` all route by
the one predicate and every screen that names gear already lists both
stores, so nothing is lost — but a **fixture** that drops a weapon and then
reads `Inventory` is reading the wrong store, and the fix is the fixture.

**The swap row's stat column is a tag, not part of its head, and the
numbers are why.** At 900px the `PopupSize::Large` body is 1243.2px and a
UI cell is 10.8438px — 114.65 cells. `wrapped_row_lines` never breaks the
head, so anything joined into it has to fit at the worst case. The head was
`[a] {name:<50} {stats:<20}`, which with `draw_row`'s two-space prefix
measured 111 cells and fitted with 3.7 to spare. `Game::copy_name`'s
quality figure costs seven of them: the longest name the shipped assets
build is exactly the 50 cells `SWAP_NAME_COLUMN` was set to, and with
" (130%)" it is 57 — a 1278.8px head against a 1243.2px body, 35.6px lost
in silence, since `draw_row` clips vertically only. No shorter format
rescues it; dropping the parentheses saves two cells and still overflows.
So the stat column joined the delta as a tag, which sheds onto a
continuation exactly when it has to. **The trap is the padding**: it lives
*inside* the tag, because a bare `" {stats}"` would slide the delta out of
its column on every row, and the unequip row carries a padded **blank**
rather than an empty string, since `wrapped_row_lines` skips an empty tag
by design. Ordinary rows are untouched — a 61-cell head plus a 21-cell stat
tag is 82 against `ROW_WRAP_COLUMNS`' 100.

### The crafter is the axis of change, and `CraftOrder` is what captures it

**`CraftOrder` is a struct at a single implementor, deliberately.** The
direct version — `Game::craft` reading the bench tier and the toggle inline
— is the null hypothesis and normally wins at one implementor, which is
what the design-patterns dialog says and what the spec records. It loses
here for one stated reason: the second gatherer is already named and
requested — a base-roster program compiling at a bench while the player is
somewhere else. The named axis of change is *who is compiling, and where*,
so the crafter is what the type captures, and `Game::craft_quality_floor`
never learns there is more than one. `Game::player_craft_order` is today's
gatherer.

**The four terms are emphatically not an axis.** They are addends in one
legible expression, and a trait with an implementor per term would be the
over-engineered reading of this. The floor is
`QUALITY_BASE + bench + perk + care`, summed in `u32` and saturated: a modded
bench with an absurd `max_tier` otherwise truncates a `u8` to *the bottom
of the band*, which is the opposite of what it earned. The clamp is not
here — `Game::roll_quality` holds the one clamp for every source of a copy
— so the floor may legitimately read above `QUALITY_MAX`.

**`CraftRecipe` carries its bench because the floor cannot go looking for
it a second time.** The two halves of `craft_recipes` read
`requires_structure` out of two different databases — an item's own
`craftable` def and a research file's `unlocks_recipes` — so a second
lookup would have to know both. `Game::best_structure_tier` answers with
the *best* deployed structure of a kind rather than the nearest: a bench is
something the base owns, not something the player walks to, and
`craft_recipes` already asks only whether one is standing anywhere. A
structure carrying no `StructureTier` reads as **tier 1**, not tier 0, so a
bench with no upgrade path and one that has never been upgraded are the
same thing to a player.

**The bench term was inert on shipped content until the benches gained an
upgrade path.** All 25 craftable-equipment recipes name the Fabricator or
the Armory, and neither declared `upgrade`, so neither ever carried a
`StructureTier` and every recipe read tier 1 — a headline term of the
feature doing nothing in the shipped game. Both now carry the path the six
nodes already had. Tier is read nowhere else for a structure with no
`ResourceNode` (`resolve_gather_cycle` is its only other reader), so
upgrading a compile bench buys better gear and nothing else, which is the
purpose the spec wanted a bench upgrade to have.
`every_upgrade_path_asks_for_a_zone_material` counts eight now, and the
count is pinned so a path that goes missing cannot drop out of the scan.

**The perk term is gathered per crafter, not read inside the floor.**
`Perk::TightenTolerances` is the player's, and the second gatherer the
`CraftOrder` seam exists for is a base-roster program, which has no perks of
its own — so `player_craft_order` asks `player_perk_level` and
`craft_quality_floor` never learns whose levels these are. Reading the
player's perks inside the floor would compile just as well and would quietly
hand a program the player's investment the moment the second gatherer lands,
which is the failure the type was built to make impossible.

**It is read at the compile rather than applied at purchase**, unlike
`Attacker`, `Defender` and `Buffer`, which write straight to `Stats` in
`unlock_perk` and so need an arm in its match. What a quality perk is worth
is a property of each copy compiled *after* it, and gear already carried
keeps the quality it was compiled at — a copy is a record of the moment it
was made, the same reading that puts an orphan's stats outside its frame
seed. So the hook is one addend and the `_ => {}` arm covers the purchase.
Priced at one `QUALITY_STEP`, the same as a bench tier, so the two read as
the same size of investment from opposite directions: fragments spent on the
base against Perk Points spent on the player. Player *level* is deliberately
not a term at all — `scaled_for_level` already scales gear to its wearer, so
a level term inside quality would double-dip on the same input and compound
against itself late in a run. The perk is that idea spent as a choice.

**The careful surcharge is applied in `craft_cost` and nowhere else.**
`craft_cost`, `max_craftable` and `craft` all take `careful`, so a screen
cannot quote one price while the compile charges another — the bug that
function's doc already records against the Lean Compiler discount, which is
the other term in the same expression. The order is discount **then**
surcharge, rounded up: reversed, a fully perked recipe with every line
floored at 1 would be careful for free. The quoted maximum takes the flag
for the same reason — a careful `[M]` sized off the plain price is a batch
the compile then refuses, which is a mutation the app-core tests caught
only once they asserted that both sides compiled *something*.

**A compile rolls per unit, and a copy at exactly spec still stacks.**
Five compiles are five copies to compare rather than a stack of five
identical ones — the compile-a-batch-and-keep-the-best loop the axis exists
for. A copy that rolls `QUALITY_DEFAULT` is plain by definition and lands
in `Inventory`; everything else takes a `GearCopies` row. So a test
counting a batch has to read **both** stores or it comes back short by
however many rolled perfectly, and a fixture reading `Inventory` alone
after compiling gear is the fixture's bug — the same call the drop side
made one phase earlier. Anything that is not equipment stacks exactly as it
did and spends **no** `GameRng` draw; `only_gear_spends_a_quality_roll`
holds that by compiling one unit against five and comparing the stream,
which works because `craft` ticks once whatever the batch size.

**The toggle is the page's and is cleared when the page opens.** Clearing
on close would leave the flag alive between two compiles: the next batch
would quietly pay half again for a floor the player did not ask for, on a
screen that had gone back to saying nothing about it.

### The category tag is a column on the row, not a substring of it

**Six screens print a `WEP` / `ARM` / `MOD` tag, and every one of them used
to `format!` it into the middle of a row string.** `[a] ×3  WEP  Arc
Lance` was one `String` by the time a renderer saw it, so there was no span
left to paint — and painting the whole row is not available, because a
row's colour already means fusion-then-rarity and has to go on meaning
that. Quality is a *third* thing a row has to say, which is exactly the
argument `Row::Item::icon` already makes for a token that is visually set
apart: two axes never collide on one glyph.

So the tag is a field. `with_tag` is the combinator, `ItemTag` the value,
and `quality_tag_style` the one palette. The alternative — keep the
`format!` and record the offset the tag starts at — was rejected in the
spec as five-way duplication with a subtler failure mode, and it is worse
than that: once a constructor is joining the pieces, storing the join
*and* an index into it is two representations of one row to keep in step.
Store the pieces; `item_text` is the join.

**The tag carries what precedes it, and that is what keeps every row where
it was.** The column is not at the start of a row — it follows `[a] ` on
the Compile screen and `[a] ×3 ` on the other five — so a slot reserved
after the selection caret the way `ICON_SLOT` is would have moved the
shortcut out of the lead position on six screens, and left a trader's
program rows (which carry no tag) ragged against its item rows. `ItemTag`
holds a `lead` instead, `draw_row` lays the row out as three `TextRun`s,
and nothing moves a pixel. `row_lead` is the one definition of those
columns, for `qty_column`'s reason: a lead one space out puts the tag half
a character off the column it is supposed to form, and
`a_lifted_tag_row_reads_exactly_as_the_hand_formatted_one_did` pins all
six leads against the literals they were lifted out of.

**The drawn pieces and the measured string must be one row.** `suffix_x`
measures a row's label to place its suffix past it, so a label measured
without the column would drop a suffix on top of the row's own tail — the
bug that function was split out of `draw_row` to hold off in the first
place. `tag_pieces` and `item_text` are held to joining to the same string
by a test rather than by a comment. The same applies one level up: the two
screens that wrap a long row onto continuations (`craft_rows`,
`inventory_row_lines`) measure with the column joined back on, or they
budget for a row narrower than the one they draw, and both hand their head
line back *without* it so the row builder can take the pieces.

**The ramp is emphasis, not hue, and it is monotone.** Gray at normal
weight, default, default at bold, gold at bold: only the two extremes
spend a colour, so an ordinary copy is never painted an alarming one and
the eye is drawn to what is worth looking at. A green-to-red con ramp was
rejected for spending four hues on an axis the row colour beside it is
already spending two on. The top band keeps the weight the band below it
earned — gold at normal weight would read as *less* emphatic than the rung
under it, so the ladder would invert at its own peak. And the as-designed
band is literally no change, default colour and default weight, which is
what makes this repaint nothing already on screen: every copy in every
existing save sits at `QUALITY_DEFAULT`. The con ramp did not have that
property either.

**Width is unchanged in all four bands**, because no characters are added
and the two UI faces are one monospace design at two weights —
`emphasising_part_of_a_line_does_not_shift_the_rest_of_it` is what says
so, and `ui_runs` lays the whole line out as one galley rather than
placing the pieces itself. The known collision is `GOLD`, which is also
`rarity_color`'s colour for `Rarity::Gold`, so an Overclocked exceptional
copy shows a gold name and a gold tag meaning two different things. They
are different columns and each colour means exactly one thing in its own,
which is the distinction the `icon` field already draws; `YELLOW` is the
alternative and sits close enough to `GOLD` that telling them apart may be
worse than the collision. Worth a look in a session.

**A row naming something no copy exists of yet passes `None`** — a
recipe's result on the Compile screen, a trader's stock — and draws
exactly as it always did. Only equipment carries quality, so `USE` / `MAT`
/ `CUR` tags are untouched whether their caller passes a copy or not.

### A copy's name is built in exactly one place, `Game::copy_name`

**A copy's name is built in exactly one place, `Game::copy_name`.** It is
the rare tier's word plus the affix's decoration of the item name —
"Overclocked Arc Lance of Static" — and every surface reads it: the
inventory list, the swap picker, the trade and market rows, the base
pane, the manifest's gear row, the erase prompt, a buyback shelf row and
a drop line. Building a name in a renderer is what would let a drop line
and the screen you open next disagree about what you just picked up,
which is the same argument `Rarity::label` makes for the tier word alone.
Note the consequence for `equip_preview_tag`: it deliberately does *not*
repeat the tier, because the name beside it already carries it.

### An item's extra effects are three lengths of one derivation, and each has its own audience

**`Game::item_effects` is the one place a listing screen learns what an
item does beyond its stat block.** It returns one short line per effect an
item declares — a passive routine granted while worn, what consuming it
does, what refactoring a companion with it does, what it adds to a
decompile — and `render/inventory.rs::effect_lines` is the one place those
become indented rows. The inventory list, a trader's three shelves, a
Stack market's sell shelf and the item action screen all draw it.

The three lengths are deliberate and are **not** interchangeable.
`item_blurb` is a two-or-three word gloss for the crafting menu, which
lists things you do *not* have and needs a reason to want one.
`item_effects` is the middle length, for a screen listing what you *do*
have, where the answer has to fit on a row beside four columns.
`item_grant` hands the describe page a routine's full authored prose. The
middle one **calls** `item_grant` rather than reading `grants` a second
time, and prices a pre-battle buff through
`FieldBuffKind::magnitude_label`, the same call the running buff list
makes — so a bottle and the buff it arms cannot quote different numbers.

**A stat bonus is not an effect here.** It already rides
`equip_preview_tag` on the row's own line, and repeating it underneath is
the column twice — the same call `item_blurb` makes about not naming a
slot beside an `ItemCategory::short_label` column.

The trap is **units**, and both shipped shapes are counter-intuitive.
`CompanionUpgradeDef`'s percentages are percentage *points* —
`refactor::raised` divides by 100 — so a `× 100.0` here reported a Buffer
Extension's +5% HP as +500%. `ItemDef::taming_potency` is the opposite: a
0..1 fraction, and not an addend at all but the **base**
`taming::capture_chance` multiplies by resistance, skill and any running
`CaptureBoost`. The line says "base capture 40%" rather than "+40%"
because a flat-bonus reading is a claim the formula does not make. Both
are pinned by tests that assert against the shipped defs' own values
rather than against a literal containing a `%`, which is what let the
100× error pass the first time.

Adding a fifth effect field to `ItemDef` is caught by
`every_shipped_item_with_an_effect_field_gets_a_line`, a census in both
directions — the guard against a field shipping while reaching no screen,
which is exactly how `power_cost` reached nothing for three releases.

### The inspect page is one derivation, and `[I]` opens it from every list that names gear

**`Game::gear_detail` is the one call behind the gear inspect page.** It
returns the copy's name, its authored prose, its stat block at the level
it would go on at, what it does to the wearer's chance of landing a
swing, the item's other effects, and the granted routine's mechanics.
Every figure on it is a call rather than a copy: `copy_bonus` for the
stats, `stat_summary` for their formatting, `battle::hit_chance` for the
odds, `Game::routine_detail` for the grant.

The reason it is one call is the reason `copy_bonus` is one call, one
axis further along. Four screens once rebuilt the *gear* scaling chain by
hand and all four dropped the affix on the day it landed. A routine's
magnitudes are the same hazard: `AbilityEffect::Damage`'s authored
`power` is the level-1 figure, so a renderer reading it would quote a
level-12 player a number no run of theirs ever uses. `routine_detail`
scales through the same `abilities::scaled_range` / `scaled_hp_power` /
`scaled_stat_power` the invocation does, at the wearer's level and affinity —
so a granted passive on a program is priced for the program.

**The mechanics are three exhaustive matches, `cell_mark`'s rule.**
`PassiveTrigger::phrase`, `AbilityTarget::phrase` and the effect line all
match every variant with no `_ =>` arm, because as a fallback arm an
eleventh effect or a sixth targeting mode ships reading as one of the
ones that already exist — or reading as nothing at all, which is exactly
how a new `CellKind` once shipped invisible.

**The page draws the grant in full, so `item_effects`' one-line `Grants:`
row would be the same fact twice.** `item_effects_besides_grant` is a
*shorter length of the derivation* rather than a trim of its output —
`item_effects` is now that plus the grant line, exactly as `item_effects`
is itself a shorter length of `item_grant`. The alternative, string-
matching `"Grants:"` off a finished list, collides with the first modded
item whose own effect line starts that way.

**One page and one subject field, reached from seven screens.**
`app-core`'s `GearInspect` carries the copy, who it is measured for, and
where Esc goes; `App::open_gear_inspect` is the only thing that writes
it. Each of the three is a distinct failure if inherited from the screen
before: the wrong copy described, the right copy priced for the wrong
body, or the player stranded a screen out from where they were. `[d]` on
the item-action list lands on the same page rather than a second one, so
there is no second place a routine's mechanics could be stated
differently.

**The key is uppercase because `selected_index` reserves shifted
letters.** A key that both moved the highlight and opened a page cannot
exist on a screen where the plain letters pick rows — the same rule
`[S]`/`[B]` quick-trading follows.

**The hit chance is a projection, and both halves of that are
deliberate.** There is no opponent until a fight starts, so
`views::NominalHostile` is `balance_sim::median_ordinary_species` — the
game's own definition of a middling program, the baseline its
survivability sweeps already assume — at the current zone level with no
gear. That is what an ambient wild spawn actually fields:
`ability_user_level` falls back to `ZoneLevel` for a creature with no
`Experience`, and `evasion_of` reads a species' `base_speed`, which no
zone multiplier touches. It is deliberately **not** filtered to the
danger band that can spawn in this zone — that needs a biome and would
fork `habitat_pools` into a second copy of the band rules — so the line
names the zone it measured against instead of pretending to be exact.
The renderer draws it only for a piece carrying a damage band or
accuracy: the figure is the *wearer's*, so under a mitigation-only module
it is the player's bare accuracy with an irrelevant item's name above it,
which reads as a claim about the armour.

**The page has no scroll.** `draw_popup` pages a `Row::Item` span and
this page is all text rows, so a row past the bottom is dropped by
`draw_row` in silence — and the rows at risk are the routine's cooldown
and the line saying how to leave. `the_tallest_gear_page_fits_its_popup`
is a census over every shipped item, swept across window heights because
`ui_metrics` clamps the font at both ends and the tightest box is the
smallest window rather than the one a test happens to run at. The
tallest shipped page is the Crash Handler's, at 17 rows against 31.

### An affix is data and its absence is supported

**An affix is data and its absence is supported.** `assets/affixes/*.ron`
is a real content directory — `AffixDef` is a name fragment plus an
`EquipmentStats` delta, so a mod adds one by dropping in a file and the
engine names nothing. `Game::roll_affix` spends **no** RNG draw when the
pool is empty, so deleting the directory restores the pre-affix game
exactly, the way deleting `assets/policies/enemy_battle.ron` does.
Three things about the roll are load-bearing. It is **two** rolls —
`GEAR_AFFIX_CHANCE` then the per-affix `weight` — because folding them
would mean adding an affix changed how often affixes appear, which is the
same split `WILD_ROUTINE_CHANCE` and `wild_weight` keep. It is
**independent of the rare tier** rather than gated behind it: rarity is
the chase at ~3.5% and the affix is the variety at ~20%, and coupling
them leaves the ordinary drops — almost all of them — as featureless as
they were, which is the complaint the feature answers. And the affix's
stats are added to the **base** inside `copy_bonus`, before all three
scaling axes, so an affix grows with the run instead of dwindling — which
is also what made the screens' hand-rolled copies of that chain wrong by a
margin that grew with the zone, above.
A save naming a removed affix reads as unaffixed rather than failing to
load, because every reader goes through `Game::affix_of`.

### Rarity is one ladder for programs and gear, and gear rolls it only on a drop

**Rarity is one ladder for programs and gear, and gear rolls it only on a
drop.** `components::Rarity` has five rungs; `spawning::rarity_for_roll`
is the single cumulative walk both `roll_rarity` (a wild program) and
`roll_gear_rarity` (a dropped item) take, so an Overclocked weapon is
exactly as rare as an Overclocked program and the shared word means one
thing. `Game::grant_gear_drop` is the one way a copy above `Ordinary`
enters the game, and its four callers are combat drops, surface boss
loot, Stack caches and nest caches. **Crafting, buying and buying back
are deliberately not callers** — found gear is categorically better than
made gear, which is the whole reason to go looking rather than shopping,
and `crafted_gear_is_never_rare` asserts that absence because an omission
is invisible otherwise. A non-equippable item takes an early return and
spends **no** RNG draw: every kill drops a work resource, so a draw there
would shift the shared stream on essentially every fight. `balance_sim`
stays ignorant of gear rarity the way it already is of creature rarity,
and here the exclusion is safe in the easy direction — a lucky drop only
makes the player stronger, so the curve it gates is the unlucky floor.

### Gear fusion has two records of the same tier, and only one of them is clamped

**Gear fusion has two records of the same tier, and only one of them is
clamped.** `GearCopies` is the ledger for a *carried* copy: it decides
what a future equip or fusion gets, and `Game::load` clamps each row to
`MAX_FUSIONS` because gear fusion was uncapped before it shared that
ceiling with programs. That clamp goes through `GearCopies::add` rather
than building the `Vec`, because collapsing two rows onto one key would
otherwise leave a duplicate row that `count` under-reports.
`EquippedItem::fusion_tier` is the *receipt* for a bonus already spent —
`apply_equipment_delta` writes straight into `Stats` and the load path
restores those numbers verbatim, so lowering the worn tier would make an
unequip subtract less than the equip added and weld the difference
permanently into the player's base stats. A legacy worn copy therefore
keeps its old bonus until it comes off. `loading_a_legacy_over_
ceiling_tier_clamps_the_ledger_not_the_worn_copy` is what stops a later
tidy-up "finishing the job".

### A trader's shelf row is `(GearCopy, qty)`, and the key is not decoration

**A trader's shelf row is `(GearCopy, qty)`, and the key is not
decoration.** `resources::BuybackLedger` keeps what was sold to it, so
buying back a mis-sold T3 returns a T3 and a mis-sold Bare-Metal returns
a Bare-Metal; keyed on the item alone it would hand back an ordinary copy
and silently delete eight base copies of work. The unit price is the same
at every tier, which is deliberate — `Game::item_value` is untouched, so
`ItemDef::value`'s second meaning (the boss-loot bands
`surface_boss_loot` derives from it) is undisturbed — and it is also why
the key has to be exact: with no price difference to notice, buying the
*same copy* back is the only thing that makes a mis-sale recoverable.
`selling_a_rare_copy_buys_back_the_same_copy` sells two copies of one
item deliberately, because with only one on the shelf the merge never
misfires and the test passes against the bug.

### `render/mod.rs::fusion_color` and `popup.rs::fusion_row` are the one colour rule for anything fused

**`render/mod.rs::fusion_color` and `popup.rs::fusion_row` are the one
colour rule for anything fused**, programs and gear alike, because both
stop at `MAX_FUSIONS`. Eleven menus call `fusion_row`; a twelfth goes
through it rather than repeating the match. Two screens deliberately do
not: the battle roster's row colour already means HP state and the
manifest's means glyph colour plus boss, and a second meaning on the same
axis makes both unreadable. A caller with a louder rule of its own checks
that first — `draw_companion_menu`'s CRITICAL red wins — which is why
`fusion_color` returns `Option` rather than a defaulted colour.

### `Stock`'s `output` is public and its `input` is private, and that asymmetry is the whole of a chain's directionality

**`Stock`'s `output` is public and its `input` is private, and that
asymmetry is the whole of a chain's directionality.** Neighbours pull from
a machine's `output`; nothing outside a machine ever touches its `input`.
That is why a chain flows one way without belts existing — a machine can
only take what its upstream has *finished*, never reach into what upstream
is still working on.
`Errand::Load` is the only write to an `input` outside `assembler_system`
and is **not** an exception to that: the rule is about a *neighbour* not
reaching in, and this is the machine's own posted program loading its own
hopper, up to the same `INPUT_STOCK_BATCHES` ceiling the pull phase fills
to. A second writer that is not the machine's own worker is the signal to
re-read this entry. The player collects by exactly the same rule, which is
why `game/collect.rs::ORTHOGONAL` is named once and read by both
`collect_adjacent` and `assembler_system`'s pull phase. A third reach rule
that could differ from those two is the signal to re-read this entry.

### Putting cargo into a Depot mirrors the collect trio, and the one place the mirror does not hold is the ceiling

> **Superseded in part.** The two screens and the six public doors described
> below were merged into one `Mode::Transfer` screen and one
> `game/base/transfer.rs` commit door — see *Taking and putting are one
> screen* at the end of this file. What survives unchanged is why a Depot is
> `Stock::output` rather than a stash, why the pack side filters `banked`,
> `stores` and gear copies, and why the room is one shared budget. The
> function names in the rest of this entry are historical.

A Depot was the only structure the player could take *out* of and never put
*into*. `game/base/deposit.rs` is the other half, and it is deliberately a
reflection of `collect.rs` function for function: `adjacent_depots`,
`depositable`, `deposit_items`, `deposit_adjacent` against `adjacent_stock`,
`collectable_adjacent`, `collect_items`, `collect_adjacent`. Two modules that
read as reflections of each other are two modules a reader can check against
one another, and every rule collect states about ordering, clamping, ticking
and refusing has a twin here rather than a second argument.

**It goes into `Stock::output`, and that is the feature rather than an
implementation detail.** `base_holding` and `work_orders::feeders_for`
already count depot buffers, so a deposit is not a stash — it is handing the
base your materials. A work order that stalls for want of an ingredient the
player is personally carrying was a real and otherwise unfixable state. A
third buffer on `Stock` for a pure stash was considered and dropped for
exactly that reason: it would keep the goods away from the systems the verb
exists to feed, and every reader of `output` would grow a second case.

**A Depot, not any `Stock`.** Mirroring collect exactly and accepting every
adjacent buffer is the simplest code and is wrong: it lets the player push
Cache Grain into a Mining Node's output, and *a unit pushed into a machine's
output reads as something that machine produced* — the same objection that
already keeps the dig crew's put-back depot-only. `StructureDef::stores` is
the filter, and it is the field the hauling system already uses to know a
Depot.

**Plain copies only, and banked items excluded.** `Stock` keys by `ItemId`
alone, so a rare or fused or high-quality copy put into one comes back out
ordinary. `Inventory` is by definition the plain-copy store and `GearCopies`
holds everything else, which is why "nothing puts a player's copy into a
`Stock`" is a standing seam — the production chain has no rarity rule
because it can never meet a special copy. Reading `Inventory` and never
`GearCopies` keeps that true by construction rather than by a check.
`ItemDef::banked` is filtered for a different reason: a bank is not cargo,
and Research Data in a depot would be a second bank the base could spend.
`PlayerStatus::inventory` already filters it, and this list is the second
reader that has to.

**The trap is that a Depot's room is one budget shared across every row.**
This is the only place the mirror does not hold. A shelf gives each row an
independent ceiling — what that item is sitting on the machine — while a
Depot has one `output_room()` across every item, so filling one row lowers
all the rest. It is enforced at both ends and for different reasons. In the
picker, because `handle_basket_key` deliberately has the property that *a
number that cannot exceed what is available is worth having by construction
rather than by a check at the commit*. In the engine, because
`deposit_items` is `pub` and the picker is not its only possible caller —
the clamp is what lets the function state its contract without reference to
who called it.

`App::basket_available` is that axis, and it subtracts only the *other*
rows. Counting the highlighted row against its own budget makes every key a
no-op the moment the basket fills: the row can be lowered but never raised
again, because its own units are already spending the budget it is asking
against.

**The two pickers share one key table rather than keeping two copies.**
`app/basket.rs`, with the ceiling carried as `App::basket_room:
Option<u32>` — `None` for a shelf, `Some(r)` for a shared budget. The table
is sixty lines of deliberately subtle semantics: an inverted Left/Right that
is *specified* to be inverted, a `div_ceil` that is what makes the Ctrl step
terminate, and a saturating digit accumulation that lets a held key reach
the clamp rather than overflow. Two copies would drift, and the inversion is
precisely what a later reader "restores" — only one copy would be under the
test that says so in as many words. A Take/Put toggle inside one mode was
the alternative and was rejected: the mode would then branch on a direction
flag through every handler and the whole renderer, and the two directions
genuinely differ. **Two modes sharing a key table is the smaller seam than
one mode carrying an axis.**

**In the fill loop, room is read before anything leaves the pack.**
`Inventory::take(item, outstanding.min(room))`, never `take` then clamp —
the latter silently eats the player's cargo into a full Depot. `take`
already clamps to what is held and drops a slot that reaches zero, so its
return value *is* the pack-side clamp rather than a second check in front of
it. Capacity is never exceeded, which is `hauling::deposit`'s rule: an
over-capacity write would make that field a suggestion, and a full Depot is
a decided failure mode rather than an exception to one.

**Both refusal sentences live in `deposit_adjacent` and nowhere else** — no
adjacent Depot versus nothing to put away, distinct because they leave the
player different errands. app-core routes an empty offer straight back
through the engine rather than keeping a copy, exactly as `c` does. The test
for that must assert the **log**: a `status_line` copy of the sentence is
wiped by `after_world_action` before anything can read it, so asserting
`status_line == None` passes against a deliberate copy and proves nothing.
That test shipped vacuous in its first draft and was caught by deleting the
`deposit_adjacent()` call and watching it *not* fail.

The key is `P`, uppercase because every mnemonic lowercase letter is taken —
`p` is the party menu, `d` demolish, `s` save — and the four free ones
(`n`, `w`, `y`, `z`) name nothing.

### A collect is one reach rule and one taking path, and the neighbour scan is sorted for a reason take-all could never see

> **Superseded in part.** `collectable_adjacent`, `collect_items` and
> `collect_adjacent` are gone; the reach rule, the sort and the single taking
> path all survive, now through `take_from_adjacent` — see *Taking and
> putting are one screen* at the end of this file.

**A collect is one reach rule and one taking path.** The reach is
`collect::ORTHOGONAL` through `Game::adjacent_stock`, the one private scan
both halves of the feature read. The take is `hauling::take_from`, the one
way units leave a buffer by hand — collect is its fourth caller. Everything
else is a wrapper: `Game::collectable_adjacent` is the `&self` view of what
is on offer, `Game::collect_items` takes an exact basket, and
`Game::collect_adjacent` is literally "select everything, then commit". Two
taking paths could drift on clamping, on the tick or on the log line, and
the one that drifts is the one nobody runs.

**The scan is sorted by `(x, y)`, and that is `assembler_system`'s reason in
a second place.** Bevy's query iteration order is not stable, and with two
neighbours holding the same item a *partial* take must drain them in the
same order every run — unsorted, an identical save answers the same keypress
differently between two runs, leaving a different buffer non-empty each
time. **Take-all could not see this**, which is why the code went so long
without a sort and why the test for it
(`a_partial_take_across_two_neighbours_drains_them_in_tile_order`) has to
span two structures. A bare `reverse()` is not the mutation that proves it:
reversing an unsorted vec is arbitrary, so the honest inversion is
`sort_by(|a, b| b.cmp(a))`.

Three smaller things the shape decides.

**The refusal is stated once.** "There is nothing to collect here." lives in
`collect_adjacent`, and app-core's `c` arm routes its own empty case back
through that function rather than setting a `status_line` of its own — a
`status_line` copy of an engine message reads as the key doing nothing. The
guards sit *above* the refusal and refuse silently: a collect asked for
during a battle or from the surface is not the base telling you its shelves
are bare, and hoisting the refusal over them makes it say so.

**An over-ask is clamped, not refused.** The buffers can only shrink between
the screen opening and the commit — a raid, a hauler, an assembler pull — so
a basket that has gone briefly optimistic hands over what is there.
`collect_items` returns what actually landed rather than what was asked for,
which is `apply_damage`'s rule: a log line printing the requested figure
claims goods the player never received.

**Left adds and Right removes, against every other Left/Right in the game.**
The manifest pager, the arena row editor and all four movement handlers step
`Right` positive; the collect picker alone steps it negative. That is a
deliberate request from the user, made after the screen shipped with the
conventional mapping, and it is the one thing on this screen that cannot be
derived from the rest of the UI — which is why the hint line names the two
arrows explicitly rather than saying "Left/Right set the amount", and why
`assets/help/60-your-base.md` says it in prose too. A player who guesses
from any other screen guesses wrong, so the screen has to tell them.

The trap is that it reads exactly like a slip. `handle_collect_key`'s doc
comment and `left_and_right_step_by_one_and_saturate`'s both say in as many
words that the inversion is the specification, so a later hand "restoring
consistency" fails a test that explains itself rather than one that merely
goes red.

**A modifier is four `GameKey` variants, and app-core is what makes them
inert everywhere else.** `GameKey` names physical gestures rather than
intentions — `Left` is the left arrow, not "west" — so Shift and Ctrl on the
two horizontal arrows are a fourth pair of variants, `ShiftLeft`/`ShiftRight`
and `CtrlLeft`/`CtrlRight`. The alternative shape, a payload
(`Left { shift, ctrl }`), was rejected on cost: it rewrites every
`GameKey::Left` arm in movement, building, inspection, the arena and the
Stack, all to serve the one screen that asked for a modifier.

The trap that shape carries is that **every other key handler ends in a
`_ => {}`**, so promoting a modified arrow to a variant nothing else matches
makes Shift+Left silently dead on every screen in the game rather than
failing anywhere. `App::handle_key` folds `ShiftLeft`/`CtrlLeft` back to
`Left` for every mode but `Mode::Collect`, in one condition, above the
dispatch. It is deliberately *not* done in the renderer: gui always sends the
modified form, because what a modifier *means* belongs on the same side of
the seam as the mode that decides it. `a_modifier_is_stripped_outside_the_collect_picker`
is the pin, and it is proved by deleting the fold rather than by reading it.

`with_modifiers` in gui promotes the horizontal pair alone. Up and Down move
a cursor on every screen that reads them and have no second meaning, so a
modified Up would be a dead key with nothing to catch it —
`only_the_horizontal_arrows_take_a_modifier` asserts the vertical pair,
Enter, Esc, Backspace and a letter all survive a held modifier unchanged.
Shift wins a tie over Ctrl: landing on "all" by accident is the milder
surprise, being what the screen's own `[A]` already does.

**Shift is a target and Ctrl is a step, and that difference is the reason
both exist.** Shift names an end of the range and is idempotent under the
key repeat driving these arrows. Ctrl closes half the gap to the end it is
heading for — `n + (available - n).div_ceil(2)` going up, `n - n.div_ceil(2)`
coming down — so a second press halves what is *left* rather than landing on
the same number twice. That makes Ctrl directional where Shift's two arrows
are two different ends.

**`div_ceil` on the step is what makes it terminate.** Rounded down, a gap of
one gives a step of zero and the key goes dead with the row neither full nor
empty. The claim has its own test on a shelf of **8**, not 7:
on 8 the ceiling and the floor agree on every step but the last (0, 4, 6, 7,
then 8 against a stranded 7), so the mutation that proves it reaches the tail
instead of failing at the first press. On an odd shelf the two diverge
immediately, which proves rounding matters but says nothing about
termination.

**`require_base` and `base_pos`'s `None` are the same locale condition**, so
neither can be mutation-proved with the other still standing — removing both
is the honest mutation, and
`nothing_is_on_offer_while_the_party_cannot_collect` says so in its own doc
comment. `base_pos` is kept beside the guard because it also yields the
coordinate the scan needs.

### `assembler_system` sorts machines by `(x, y)` before pulling, and the test for it asserts *which* machine won

**`assembler_system` sorts machines by `(x, y)` before pulling, and the
test for it asserts *which* machine won.** Bevy's query iteration order is
not stable, so two machines competing for one feeder's scarce output
resolve differently between runs without the sort — a flaky-test source and
a base that behaves differently after a reload. The test spawns the two
competitors in the *opposite* order to their positions on purpose: spawned
in position order it would pass on iteration order alone, which is the
exact bug the sort exists to prevent. Verified by removing the sort and
watching it fail, not by assuming.

### Planning is per machine, not per base

**Planning is per machine, not per base.** Reading a neighbour's `output`
while writing your own `input` is the same `Query<&mut Stock>`, so the pull
phase plans against a snapshot and then applies — but it does that *inside*
the per-machine loop. Planning the whole base at once and applying
afterwards compiles just as well and lets two machines both take the same
units, silently undoing the sort above.

### `Stock` keys by `ItemId` in a `BTreeMap`, and `ItemId` derives `Ord` only for that

**`Stock` keys by `ItemId` in a `BTreeMap`, and `ItemId` derives `Ord`
only for that.** Two reasons and both bite: iteration order feeds the pull
phase, and a `HashMap` would make the save encoding differ run to run.

### A machine's recipe is the assembled item's own `craftable.cost`, resolved through `systems::assembly_recipe`

**A machine's recipe is the assembled item's own `craftable.cost`, resolved
through `systems::assembly_recipe`.** There is deliberately no recipe on
`AssembleDef` — a bench recipe and a machine recipe for the same item
cannot drift because there is only one of them, and every craftable item a
mod adds is automatable for free. All nine shipped assembler recipes set
`requires_structure` to their own machine, so hand-crafting is the manual
fallback for a machine you own rather than a way around building it;
ungated they broke `only_the_starters_and_scavenged_gear_need_no_research_
or_bench`, which is a stated policy and not a census.
`no_shipped_assembler_builds_another_benchs_product` is what holds that, so
a tenth machine pointing at someone else's bench fails rather than shipping
a product its owner cannot make by hand.

### Every shipped `assembles` recipe is one ingredient, and that is a property of the *items*, not the machines

**Every shipped `assembles` recipe is one ingredient, and that is a
property of the *items*, not the machines.** A production line is a
straight line — raw tap → refiner → bench, each machine pulling from
exactly one upstream — and the four intermediates match the four benches
1:1 (Bytecode Block→Armory, Blank Substrate→Disk Press, Logic
Wafer→Fabricator, Charge Coil→Assembly Bay). Because a machine runs its
product's own `craftable.cost`, a second ingredient added to any of those
four items silently turns its bench back into a corner puzzle needing two
lines stood up before a single unit comes out — which is what shipped
until 2026-08-05 and what the flatten removed.
`every_shipped_assembler_recipe_is_a_single_ingredient` holds it. Two
consequences worth knowing before touching this: the Patch Routine runs on
Charge Coils rather than Bytecode Blocks *specifically* so the Winding Node
keeps an automated consumer — repoint it and that machine becomes dead
weight; and a bench's `build_cost` names its own feeder's product, so the
line that runs a bench is also the line that pays for it
(`each_bench_is_built_out_of_what_its_own_feeder_makes`). The engine's
multi-input support is untouched and mods may ship two-ingredient
assemblers — `chains::a_machine_short_one_of_its_two_ingredients_stays_
starved` walks that path with a modded machine, since no shipped one can.

### Installing a routine is the one place a `KnownRoutines` entry meets an item, and the item is spent last

**Installing a routine is the one place a `KnownRoutines` entry meets an
item, and the item is spent last.** Knowledge (`resources::KnownRoutines`,
written only by `unlock_research` and `extract_routine`) says *what* the
player may install; an `ids::ROUTINE_DISK` in cargo is *whether they can*.
`install_routine` checks battle, ownership, knowledge and a free slot
before it looks for the disk, and takes the disk only once all of those
have passed — the same ordering argument `use_symlink` makes about
`clear_stack`. Uninstalling returns nothing, which is the whole point:
a slot is a commitment. Two consequences that are easy to undo by
accident — a new game *knows* `DECOMPILE_ABILITY_ID` (nothing else grants
it, so popping it out would otherwise end taming for the run), and a
displaced innate routine is lost rather than banked, because there is
nowhere for a routine off a slot to wait.

### `MachineStatus::Stranded` is `Unstaffed` plus the knowledge that waiting will not fix it

**`MachineStatus::Stranded` is `Unstaffed` plus the knowledge that waiting
will not fix it**, and it exists because a base can now be built around its
own machine. The two systems split by writer, not by fact:
`haul_step_system` is the only thing that walks a field, so it marks the
*worker* (`components::Stranded`); `task_progress_system` stays the only
writer of a machine's status and picks the more specific reading when it
sees the marker. Giving the status two writers makes them ping-pong every
tick, and `set_machine_status` logs on every transition. The marker is read
one tick after it is written — `task_progress_system` runs first in the
chain — and that lag is accepted rather than reordering a chain whose order
is load-bearing for the clog/pickup handoff.

### `set_machine_status` is the one place a stall is announced, and it logs only on transition

**`set_machine_status` is the one place a stall is announced, and it logs
only on transition.** `idle_machine_system`, `task_progress_system` and
`assembler_system` all call it, so "entering a state is news, staying in it
is not" cannot hold in one producer and lapse in another. A base with four
stalled machines would otherwise put four lines in the pane every tick.

### "Nobody is posted here" is one pass over every machine, and used to be a branch inside the assembler

**"Nobody is posted here" is one pass over every machine, and used to be a
branch inside the assembler.** `idle_machine_system` announces `Idle` for
any machine with no `GatherResource` task pointed at it. It was
`assembler_system`'s, which visits only structures declaring `assembles` —
so nothing ever wrote a status for an *extractor* with no program, and
`MachineStatus` defaults to `Running`. A freshly deployed Research Node
therefore drew green on the map as though it were producing, and a machine
whose worker was killed or reassigned kept its last status for the rest of
the run. Fixed 2026-08-11. The matching half is that
`task_progress_system` now announces `Running` while a cycle is still
*ramping up*: that state used to ride on the `Running` default, which held
only because nothing announced anything until the first payout, and a long
cycle would otherwise read as idle on every tick but the one it pays out
on. `TaskKind::Guard` deliberately does not count as working a machine.

### A banked resource can never clog, so a Research Node has no "full" state

**A banked resource can never clog, so a Research Node has no "full"
state.** `deliver_payout` sends a `banked` item straight to the player's
bank, which has no ceiling, and `research_data` is the shipped example. Its
four reachable statuses are therefore `Idle`, `Unstaffed`, `Stranded` and
`Running` — grey, yellow, red, green — and there is no yellow-for-full to
be had without first giving the bank a cap, which is a balance decision
(`CLAUDE.md`'s item-price entry explains what the flat banked payout is
holding up) rather than a rendering one.

### A group menu's rows are hidden dynamically, so `base_menu_rows` / `party_menu_rows` must be the *only* source of them

**A group menu's rows are hidden dynamically, so `base_menu_rows` /
`party_menu_rows` must be the *only* source of them.** The handler
dispatches `rows[idx].target` and `render/group_menu.rs` draws
`rows[i].label` — both from the same call. A renderer that rebuilt the
list from the static table would be right until the first hidden row and
then silently open a different screen from the one under the highlight.
The table itself is in `app/group_menu.rs`, and a row survives two
clauses: it is not `base_only` while the party is outside base space,
*and* the screen it opens would have at least one row. The second clause
deliberately asks only the first screen — Cronjob can list a program and
land on an empty structure picker, which is survivable because Esc backs
into the menu. `base_only` is a flag in that table rather than an
`in_base()` check inside each predicate, because what it has to stay in
step with is `require_base`'s caller list in the engine, and only a table
makes that checkable. Emptiness alone would not do it: every
`App::nearby_*` scan reads the player's `Position`, which is pinned to the
Stack entrance underground and to the anchor tile in base space, so those
rows would offer to demolish a base the party is nowhere near.

**The flag was `surface_only` and asked `is_underground()` until the base
moved out of phase**, which was the same question while "not in the Stack"
and "where the base is" were one condition. It is the app-core twin of the
eleven-site re-reading above, and it went wrong the same silent way: on the
zone surface the row was offered, the player picked it, and the engine
refused — a menu row that reads as a dead key. Two flat map keys carried
the same imprecision and moved with it: `d` (demolish, aimed) and Enter on
the structure roster, both now `in_base()` rather than `!is_underground()`.
`t` (the trader list) is the one left: it still opens on the open grid, and
every row behind it is a `require_base` call that refuses.

### A work order stores what was asked for, never how it will be done

**A work order stores what was asked for, never how it will be done.** An
item, a quantity, and a `standing` flag saying whether the quantity is a
batch or a level — three labels on the request itself. No
per-machine plan, no unit targets, no progress counters — which machines a
line needs, in what order, who is on each and how far along it is are all
recomputed from live world state every time they are asked. This is the
same call `Game::contract_board`, `descriptions.rs`, `Game::wielded_program`
and the Stack's regenerated frames each make, and
it buys the same things: the derivation cannot go stale against a base
that has been rebuilt, it needs no save field beyond the order, and it
costs no migration. The alternative was considered and rejected —
multiplying the recipe tree through at queue time into fixed per-machine
targets gives a tidier progress bar and a plan that is confidently wrong
the moment a machine is demolished, which is the second copy `CLAUDE.md`
records drifting four times.
Two consequences are paid explicitly. **"Percent done" is not a stored
number**: `Game::work_order_report` *calls* `wants`, so the screen and the
scheduler agree by construction rather than by a comment. And **cancelling
unwinds nothing**, because nothing was wound — the next tick simply derives
a different answer, which is the decision paying out somewhere it was not
designed for.
The two functions everything runs through are `can_progress` (output has
room, and for an assembler one batch of every ingredient is within reach)
and `wants` (the recursive walk, deepest first). The second half of
`can_progress` is the load-bearing one: `assembler_system`'s pull
phase sits *behind* the "is anyone posted here" gate, so a machine with
nobody on it never fills its own input — "the input is empty" is therefore
not the same question as "this machine has nothing to do", and reading it
as one leaves every empty bench permanently unstaffed. It is also what
*releases* a worker: a clogged machine cannot progress, so it stops
wanting a body. That is how "work the deepest requirement until it is made,
then move on" falls out without the scheduler sequencing any phases.
**`wants` roots its walk at every machine that makes the ordered item, not
at one of them**, and `producers_of` is plural for that reason. It returned
a single `Option<Entity>` until 2026-08-16, and the case that exposed it is
the one a player reaches by playing well: a Mining Node whose output is
eaten by the assembler beside it never fills the order, so you deploy a
second one — and it stood unstaffed forever, because the walk had rooted at
the first by tile order. `chain_break` shares the plurality for the mirror
failure: one whole line is enough, so an unfed twin bench standing off in a
corner refused an order the base could already fill. The merge is
`walk_wants`' `deepest` map, so a feeder shared by two lines is still kept
once at the furthest depth either reached it.
**"Within reach" has three sources and one definition**,
`work_orders::batch_within_reach`: the machine's own input, an orthogonally
adjacent feeder's output, and a Depot shelf. `can_progress` asks it of a
`&Game`, `haul_step_system` asks it of its queries, and the two must not
drift — a scheduler that staffs a bench the walker will not fetch for
leaves a body at a starved machine forever, and the reverse sends one on an
errand nothing wanted. It is trivial arithmetic on purpose: it replaced a
`beside.min(want - held)` cap that could never change the answer, since the
cap bites only when `beside > want - held`, which already implies
`held + beside > per_batch`.
The **third** source is also what makes the shelf beat the bench.
`walk_wants` skips recursing into a feeder for an ingredient a Depot holds
a full batch of — without that, deepest-first sends the one spare body
upstream to hand-make what the base is already holding. A *batch*, not a
unit: a shelf too thin to run a cycle is no answer, and skipping the feeder
on the strength of it strands the order with nobody working anything
(`the_feeder_is_wanted_again_once_the_shelf_will_not_cover_a_batch` is the
mutation-checked gate). `depot_holding` is deliberately narrower than
`base_holding` — Depots only, never a machine's own output buffer, or a
bench would count its feeder's output twice and skip staffing the very
machine that made it.
That third source held for `can_progress` and the hauler and **not** for
`chain_break`, and the drift cost a real base its work orders. A save from
2026-08-19 stood a Compiler, a Lathe and a Mining Node in a row two tiles
apart, with a Depot on the slab holding twelve Core Fragments: the hauler
would have walked to the shelf and both benches would have run, but
`break_at` demanded an orthogonal feeder, so `orderable_items` filtered
both products out and `App::base_menu_rows` dropped the row entirely. The
player got no picker and no sentence saying why — a fourth, narrower,
hand-rolled copy of the reach rule, which is the exact failure mode
`CLAUDE.md` records biting this repo four times.

`work_orders::feeders_for` is now the one answer to "where can this
ingredient come from": every orthogonal producer, or, when nothing beside
it makes the ingredient and a Depot is standing, every deployed producer of
it wherever it stands. `break_at` and `walk_feeders` both call it, so the
picker and the scheduler cannot disagree about the topology again.

It is **structural, never a live stock count**, and the split is the point.
`chain_break` answers whether a line can *ever* move — keyed to what a
shelf happens to hold, the picker would offer an item and stop offering it
as the shelf drained. What varies with stock is `walk_feeders`' own
shelf-before-bench skip above, which answers the different question of who
to post *now*. Both survive together: with a batch on the shelf the
upstream producer is left alone, and once the shelf runs thin the walk
reaches it through the depot route rather than returning an empty want list
and stalling the order forever
(`the_producer_behind_a_depot_route_is_staffed_when_the_shelf_runs_thin`
and `a_stocked_shelf_still_keeps_the_body_off_the_producer_behind_it` are
the pair). A base with **no** Depot standing is unchanged: `feeders_for`
returns nothing, and the refusal still names the missing link.

### Every unsatisfied order is worked at once, and `settle_orders` is where priority lives

**`settle_orders` returns the accumulated wants of every unsatisfied,
non-stalled order in queue order**, not the want list of the first one that
has work in it. The queue is a production policy rather than a to-do list: a
base with more bodies than the front order can use works the one behind it
too, instead of parking the spare programs beside a line that is already
fully staffed.

**Priority needed no new code, and that is the point.** The accumulated list
comes back in queue order and `schedule_base_labour`'s
`truncate(staff.len())` cuts from the **end** — the same mechanism that
already made dig wants lowest. Order 1's machines are at the front and get
first refusal on every body, order 2 fills from what is left, and standing
jobs and dig sites are still appended after all of them. A sort or a score
here would be a second ranking rule beside the one the append sites already
hold, and the two would drift.

**The dedupe is an ordering constraint, not an optimisation, and its failure
mode is not the one you would guess.** Two orders can want the same feeder.
Counted twice, the duplicate occupies a second slot in `wanted` — so
`truncate` drops one want from the bottom to make room for it. The obvious
symptom, two bodies standing on one machine, never appears:
`post_worker` calls `displace_task_holder`, so the second posting evicts the
first and the machine still reads as staffed by exactly one program. What
actually happens is a program left idle with no post and a want silently
dropped — on the base that exposed it, the second order's own bench. So a
test that asserts "one body at the shared feeder" passes with the dedupe
removed and gates nothing; `a_machine_two_orders_want_is_posted_once`
asserts that **every** staff member got a post and that the second order's
bench is one of them, which is what goes red.
Keeping the **first** occurrence rather than the last is what makes the
higher-priority order hold the position.

**Nothing in the suite gates the throughput this buys.** `balance_sim` has
no base term at all, the arena models player combat, and no test can see
base output against the zone curve. A staffed base is now materially more
productive and that is a pacing question for play, stated here rather than
mitigated because there is no instrument to mitigate it with.

### A satisfied standing order is skipped, not removed

**`WorkOrder::standing` makes an order a level the base holds rather than a
batch it makes once**, and the whole of the mechanism is which branch
`settle_orders` takes when `base_holding >= qty`: a one-shot order is
completed, announced and removed; a standing one takes `index += 1`, the
branch a stalled order already takes, and is re-evaluated next tick like
everything else.

The gap it closes is that a target level which deletes itself the moment it
is reached is not a level. `collect_adjacent` moves a machine's whole
`Stock::output` into the player's `Inventory` — still one keystroke away
behind the collect picker's `[A]` — so the shelf empties every
time the player walks past it — and under one-shot orders alone, the order
that was meant to keep it full had already been removed. The reconciliation
loop the module is built around could not close.

**Skipped, not returned, is the one correctness point.** A dormant order
contributes no wants, and handing those straight back out of `settle_orders`
would starve every order behind it for as long as the shelf stayed full.
That is the same failure the concurrency change above exists to prevent, one
tick later, and `an_order_below_a_satisfied_standing_order_is_worked` is
what goes red for it.

**No hysteresis, and none is needed.** The instinct is that re-arming at
exactly `qty` thrashes — one unit leaves, the chain wakes, bodies are pulled
off the next order to make one unit — and that is a fair worry, because it
is precisely the failure `schedule_base_labour`'s diff exists to prevent one
level up. It does not apply here, because the drain is bursty in
practice: `collect_adjacent` empties the *whole* buffer, so holding goes
to zero, the order runs the full `qty`, and sleeps. There is nothing to
oscillate around. **The collect picker weakened "by construction" to "in
practice"** — a player may now take part of a shelf rather than all of it,
so a trickle is reachable by hand where it used to be impossible. Take-all
is still two keys (`[A]`, Enter) and is still the common case, so no
`refill_at` was added; if play shows oscillation, this is the paragraph to
re-read first. The one genuine trickle case is a standing order on an
intermediate that a downstream assembler eats a batch at a time, and there
the downstream order's own `wants` walk already staffs that same machine, so
the bodies land in the same places. No `refill_at` field and no `tuning.rs`
fraction were added; if play shows oscillation, `qty.saturating_sub(batch)`
is a one-line change later.

**It says nothing on top-up.** "Work order complete" is a lie about
something that is not complete, and detecting the moment an order fell
asleep needs stored state the order deliberately does not have. Filing one
is announced differently instead — "Standing work order filed: hold N x X" —
because until the queue screen learns to draw a dormant tag, that log line
is the only place the flag shows itself.

**`Game::queue_work_order` takes the whole order, built by
`WorkOrder::batch` or `WorkOrder::level`.** What an order carries is the
axis that keeps moving — it went from two fields to three here and takes a
priority band next — and a signature that spelled the fields out was being
swept through some fifty call sites once per phase, arriving at
`(item, 3, false, OrderPriority::Normal)`: two positional literals nobody
can name. A batch and a level are *different errands* rather than one
errand with a flag on it, which is why they are two named constructors
rather than a `standing: bool` parameter; anything that is not a kind of
order goes on as a `with_*` setter instead. `WorkOrder` is re-exported from
the engine root for it, the shape `game::contracts::{BrokerReach,
ContractRefusal}` already uses, since `game::base::work_orders` is
`pub(crate)` and the type was not nameable from app-core at all.

**`base_holding` counts machine and depot buffers only.** "Hold 20 Cache
Grain" therefore means 20 on the shelf, and 40 in the player's pocket are
invisible to it. That is the correct reading — the order is a statement
about the base — but `0/20` while carrying 40 reads as a bug the first time
it happens, so the quantity page says which figure it is showing. That page
also moved from `PopupSize::Small` to `Large` when it gained the toggle: its
widest sentence already ran 8px past a small box, and `draw_row` never clips
a row horizontally, so nothing would have said so.

### A work order's band is an insert position, not a second sort

**`OrderPriority` decides where an order lands in the queue, once, at
filing.** `queue_work_order` inserts it after the last order of
equal-or-higher band instead of pushing; nothing reads the field again.

The obvious build is the expensive one. A `priority` field plus a sort at
scheduling time compiles just as well and makes Vec order and effective
order two different things — and every index in the system then has to know
which of the two it is holding. `cancel_work_order` takes a **raw Vec
index**, `work_order_report` returns in Vec order, and the screen indexes
straight into that report, so the sort would have had to be threaded
through all three or they would start naming different rows. Keeping the
Vec in effective order leaves `settle_orders`, `cancel_work_order`,
`work_order_report` and the screen untouched. The stored field is a label;
position remains the one thing the scheduler reads.

**After the last order of equal band, not before the first**, which is what
makes ties break by insertion order — one band is still a queue. The
mutation is worth knowing: inserting before the first equal-priority order
reds not only the tie test but four older tests that had nothing to do with
bands and everything to do with the queue keeping the order things were
filed in.

**`OrderPriority` is deliberately not `Ord`.** `High < Normal` is true under
any encoding that puts High first, and reads backwards at every call site
that would use it, so the comparison goes through a private `rank()`
instead.

**Set at filing, and that is why there is no reorder verb.** `[P]` on the
quantity page cycles the band, raising first — three bands means one
direction costs two presses, and raising is the common intent, since before
bands the only control over the base's attention was cancel-and-refile,
which lands the order you care about at the *bottom*. Refiling now restores
the band instead. Reordering *within* a band is knowingly left open:
`move_work_order` is about twenty lines and composes with bands if a short
queue turns out to need the resolution.

### `schedule_base_labour` decides the whole assignment by priority and then diffs it against what is posted, and both halves of that are load-bearing

**`schedule_base_labour` decides the whole assignment by priority and then
diffs it against what is posted, and both halves of that are load-bearing.**
Filling greedily around the postings that already exist compiles just as
well and leaves a body standing on a standing job while an order goes
unworked — because the body was already somewhere "wanted". Truncating the
priority-ordered want list to the staff count first is what makes an order
outrank a standing job for a scarce body.
The diff is then the anti-thrash rule: anyone already at a post the
assignment keeps **stays exactly where they are**. Without it the scheduler
walks the whole roster across the base whenever a buffer changes by one
unit, and restarts every cronjob's progress from zero doing it.
**A body holding a `Carrying` is never freed**, whatever the assignment
says. Freeing one drops `Carrying` along with the `Task`, and the units are
out of the machine's stock by then — so cancelling an order mid-walk
*destroyed* the goods rather than releasing them. Rare while a worker only
ever set off from a clogged machine; routine now that one sets off every
cycle.
**Nor is a body standing on a machine with no output room while a Depot
stands.** It is the same rule one case earlier rather than a second one: a
clogged machine cannot progress, so it drops out of `wanted`, and the body
on it is the only thing that can carry the clog away and give the machine a
route back *into* `wanted`. Freed instead, the machine sits full for the
rest of the run. The Depot term is what keeps
`a_lone_body_walks_the_line_downstream_as_each_machine_stops_being_useful`
true — with nowhere to deliver there is no errand, so there is nothing to
stay for.
**It never takes a body off a machine unless it has somewhere to put it —
and only on a base with an empty queue.** The rule is not in the spec and
the `chains` template is what caught its absence: with every wanted post
already filled — a base whose queue has run dry, or *any* base loaded from
a save written before work orders — the scheduler stood down every worker
on the first tick, which is exactly the regression `Game::load`'s
absorption rule exists to prevent.

**Both cases that argument names are an empty queue, and gating on that is
what closed the hole it left.** Unqualified, the guard also caught the base
whose orders are all *satisfied*. A standing order reaching its level is
skipped rather than removed, so its machines drop out of `wanted` while
staying in `posted` — every want left was filled, the guard fired, and the
line kept running for the rest of the run. Reported from a live save: a
Compiler making ICE Breakers against a hold-at-10 order with 73 already on
the shelf. Ticking that save 3,000 times reaches **222**; with the queue
term it stops at exactly 10. A queue is an instruction, so where there is
one the assignment is the whole truth and a body it does not name goes back
to the ring.

The blunter fix — deleting the guard outright, on the grounds that
`StandingJob` is how a machine is deliberately kept running with no order
behind it — was tried and rejected on cost: 81 engine tests and the
`chains` template post a program with no queue standing behind it, and
every one of them would have had to say `StandingJob` to keep measuring
what it measures. The queue term costs one line and leaves that whole class
untouched. What it does **not** cover is a base whose *last* order is a
batch that completes: the order leaves the queue, the queue is empty, and
the bodies keep their posts. That is the run-dry case the guard was written
for and it behaves as it always has.
It is a `&mut Game` method called from `tick_inner` immediately **before**
`schedule.run`, not a bevy system: posting logs, reads defs through
`work_ticks_for` and writes `Party`. Running it there buys the same "posted
this tick, progresses this tick" a chained system would have, and puts it
beside `maybe_spawn_wild_creature`, which is there for the same reason.
**It draws no RNG at all** — idle staff park on a deterministic function of
`(centre, index, tick)` through `stack::ring_offset`. `CLAUDE.md` records
three occasions where a shifted stream silently rewrote a seeded test in an
unrelated file, and a milling draw taken every tick for every idle program
would shift it harder than anything else in the game.

### How short of bodies the base is, is a cached figure taken before the cut

**`resources::LabourDemand` holds two `usize`s written once a tick by
`schedule_base_labour`: how many posts the queue asked for, and how many
staff there were to fill them.** `Game::labour_demand` reads it back; the
work order screen draws a header off it, and says nothing when the
shortfall is zero.

**It is written before `wanted.truncate(staff.len())`, and that is the
whole seam.** The figure the player needs is the one the cut throws away —
the posts that fall off the end vanish in silence, which is why a base with
three running machines and two programs shows "no one" on the third and
nothing anywhere saying why. Recorded after the truncate the number is
`staff.len()` by construction and the shortfall is *always* zero, so the
header never draws and the feature is inert while every test that only
checks the two figures exist stays green. The test that goes red is
`a_base_short_of_bodies_reports_the_difference`, and the mutation is one
line moved.

**Cached rather than derived on demand, for `Platform`'s radius reason.**
Both figures live inside `schedule_base_labour`, which is `&mut self` and
has side effects — `settle_orders` drops a completed order and announces a
stall — so a screen cannot ask for them by calling it, and a
second walk that rebuilt the want list would be the copy that drifts.
`record_labour_demand` is the one writer for the same reason: two write
sites taking the two numbers from different points in the tick is how they
stop describing the same moment.

**The second write site is the `staff.is_empty()` early return**, which is
a valid quiet state and the one a player is most likely to have the screen
open on — a base with no roster at all. Left unwritten there the demand
reads as no wants rather than no bodies, which is the opposite of what
happened, and the resource would keep whatever the last staffed tick put
in it. The two early returns above it are deliberately *not* written: a
game over or a live battle is not a state this screen is reachable from,
and a stale figure there says nothing wrong to anybody.

**Not saved.** It is rewritten on the next tick either way, so there is no
`SAVE_FORMAT_VERSION` question — and a figure restored from a save would
describe a base that has since been rebuilt. It is inserted at both
constructor doors like `PowerGrid`, so a screen opened on the frame a game
loads finds an empty demand rather than a missing resource.

**The header is silent at zero on purpose.** It answers "why is nothing
happening" from the other direction to the state tag: the tag says which
order has the base's attention, this says whether the base has anyone to
give it. A line that shows on every visit is a line nobody reads by the
third one. `labour_header` is a pure function of the demand for
`work_order_lines`' reason — it is an unwrapped head line, so it is a row
that can actually run off the popup body, and the width test measures it
at three digits.

### A program's role is derived, and there is no "owned but idle" state

**`Game::program_role` is the one derivation of what a program you own is
doing with itself**, and `ProgramRole`'s three variants — `Wielded`,
`InParty`, `Staff` — are disjoint and exhaustive. A program you own that is
not fighting beside you and not held as your weapon **is** base staff. The
scheduler posts it; nothing assigns it.

This replaced a stored `components::BaseStaff` marker toggled by hand from
the Base Staff screen, and the marker's two verbs (`assign_base_staff`,
`release_base_staff`) are gone with it. The reason to derive rather than to
auto-insert is the one this file keeps making: a stored bit and the thing it
is supposed to mirror are two sources of truth, and the one that drifts is
whichever nobody looks at. The old marker needed upkeep at four sites —
`add_companion` stripped it, `assign_base_staff` set it and popped the
party, `wield_program` had to stand a member down, and the load path carried
an *absorption rule* to rescue a base staffed before the feature shipped.
None of those exist now, because `Party` and `WieldedProgram` are already
authoritative and `Staff` is what is left over.

**The precedence was already duplicated before this landed**, which is what
made the enum worth its indirection over a derived boolean.
`Game::program_activity` ordered wielded ahead of party in its own prose,
and a boolean `base_staff()` would have restated the same exclusion from
the other side as a chain of negations. One enum, one ordering, and a fourth
role added here is one arm plus a compiler error at every reader — which is
the shape to keep, because a fourth role is planned.

**The rule itself is `party::role_of`, a free function over values**, for
`stack::surfaced`'s reason: `base_entropy_system` is a bevy system with no
`Game` to ask, and it must not carry a second copy of who counts as staff.
Its query is deliberately *wider* than the rule — `Or<(With<Task>,
With<Tamed>)>`, which catches party members too — and narrows with
`role_of`, because the narrowing is the half that must not exist twice. A
cell reverted under a body seals it in solid rock for the rest of the run,
which is what a drifted copy would cost.

**The save field stayed.** `CreatureSave::staff` is still written and is
read nowhere, exactly as `Experience::xp_to_next` is: *removing* a field is
what earns a `SAVE_FORMAT_VERSION` bump, and this change earns none. A save
claiming `staff: true` for a program that loads back into the party cannot
make the two disagree, because nothing consults it.

**Two consequences worth knowing before touching this.** First, `Game::
assign_cronjob` no longer pins a worker: the poster is in the pool now, so
the scheduler owns the posting and will move it on the next tick. It has no
call site outside engine tests — manual posting was deleted in 0.8.35 — but
a fixture that hand-posts and then expects the body to stay put is testing
something that stopped being true, and the symptom is a chain that quietly
drains instead of clogging. Second, there is no longer any way to hold a
program back from the base, so **base output scales with roster size** and
`pet_capacity` is the only thing bounding it. That was accepted as a
balance change, not smuggled in as a convenience.

### `accepts_a_program` is the one predicate for "a program can be posted here"

**`accepts_a_program` is the one predicate for "a program can be posted
here".** `view_entities`'s `can_work`, `structure_report`'s `workable` and
`set_standing_job` all read it. A structure a screen offers and the engine
refuses is a dead end; one a screen hides but the engine would take is
unreachable.
It answers only half the question, though: it is about the *structure*,
and the other half is the walk to it. `hauling::post_reach` is that one —
`at_station`, or a route through `post_field`, in the order
`haul_step_system` asks them — and `post_field` is *called* by both sides
rather than copied, so a posting that is accepted is a posting that
arrives. It reports `NoPost::BoxedIn` apart from `NoPost::NoRoute` because
the two leave the player different errands, and because once buildings
block a walk "too far away" became a lie about a machine you were standing
beside.
**Its asker moved on 2026-08-14 and the question survived the move**, which
is the part worth knowing. `assign_cronjob` used to ask it before a menu
posting; that function is gone with manual posting, and
`schedule_base_labour` asks it instead — a machine with no route is
*skipped* rather than filled. Filling it would strand a body there for the
rest of the run while the order it was meant to work went unstaffed, and
the deletion very nearly took `post_reach` with it. `tests/support.rs`
keeps `assign_cronjob` as a fixture built out of these same primitives,
never a copy of the removed body: fifty tests need a program on a machine
and have nothing to say about how it got there.

### A posted program sets off from its own tile, and the player's tile is read nowhere in the scheduler

**A posted program sets off from its own tile, and the player's tile is read
nowhere in the scheduler.** `Game::post_worker` writes no `Position` at all —
the same omission `post_guard` already made, for the same reason — and
`schedule_base_labour` asks `can_walk_to_post` from the tile of the body it is
about to send rather than from the player's.

This inverts what the seam used to say, and the argument it used to make is
worth keeping because it was *correct at the time*. A tamed program's
`Position` was written at capture and never again — `views.rs`'s
`worker_away_from_post` doc says so, and `render/base.rs` refuses to draw a
companion because "drawing it would claim it is somewhere it isn't". Nothing
synced it as the player walked, `enter_next_zone` being the one event that
re-collected the roster onto a tile. So the stored value was the tile the
program was beaten on, which can be anywhere the player has ever fought, and
anything measuring a distance from it was measuring noise: that is how a
worker got posted to a machine two tiles from home while standing 23 tiles
north, outside `HAUL_WALK_RADIUS` of its own station tile, never stepping and
never producing while the cronjob read as scheduled. Overwriting it from the
player's tile at posting time was the fix, and the consequence was written
down as a design decision — *the walk to a post is bought by posting from a
distance* — because posting was a player action.

Both halves of that premise are gone. Manual posting went with work orders,
so no player is stood anywhere in particular when a body gets a job; and
`park_idle_staff` (0.11.x, `drift_idle_staff` since) writes every free staff
member's `Position` every tick, so the value is live rather than stale. What was left was one seam
failing in two directions at once, both reported from play: a program
loitering by Home **teleported onto the player** the instant the scheduler
gave it a job and walked in from there, and — sharper, because it stops the
base rather than looking wrong — `post_reach` measured from the player meant
**walking out of the walk field stopped the scheduler filling a single
machine**, leaving the pool stood idle beside the order it was hired to work.
A base that only runs while you stand in it is not a base.

Three things hold the new rule. `drift_idle_staff` runs **before** the
assignment, which is now load-bearing rather than incidental: it is what
guarantees the tile a program is standing on when it *gets* a post is a real
one. The step-5 loop **peeks** the idle pool (`idle.last()`) and only pops
once the reach check has passed, so a machine skipped for want of a route
still costs no body — the ordering the old code got for free by checking
before it popped. And `post_worker` keeps no `from` parameter at all, so
there is no seam for a second caller to pass the wrong tile through; the
`assign_cronjob` test fixture writes the `Position` itself, which is what
keeps fifty tests measuring what they always measured.

`park_at_post` is still only meaningful *after* an assignment, and
`stand_player_at_post` is still the before — but only inside that fixture.
Nothing in the shipped game reads the player's tile to decide where a
program starts walking.

### An idle program wanders the base, and laid floor is the leash

Staff with nothing to do used to be walked around a fixed Chebyshev ring at
`IDLE_STAFF_RING_TILES` from the Home, one tile every `IDLE_STAFF_STEP_TICKS`
— `park_tile`, a pure function of `(home, staff index, tick)`. It read as a
picket line. What it looks like now is `wander_step`: one of the eight
neighbours of the tile the body is **standing on**, or a hold, on the same
cadence.

**Relative rather than absolute is the whole of the difference.** A program
the scheduler has just freed strolls away from the post it left, instead of
snapping onto a tile computed from its index — which is also why the drift
survives a save with no field of its own, since `Position` is already
written and the walk resumes from wherever the body was left.

**It is still a pure function of its arguments and still draws no RNG.**
That is not fastidiousness. `CLAUDE.md` records three separate occasions
where a shifted stream silently rewrote a seeded test in an unrelated file,
and world generation is barred from `GameRng` outright for the same reason;
a milling draw taken every tick for every idle program would shift it harder
than anything else in the game. `idle_staff_take_no_rng_draws` predates the
drift and still holds it.

**The fold is a byte at a time**, following `sectors::sector_seed`, and that
is load-bearing rather than stylistic: `derive::index` reads bit 63, one
XOR-then-multiply round carries a difference only about the prime's own width
upward, and a step counter differs from its predecessor in its lowest bits
alone. Folded as a single word it never reaches the bit that decides, and
every program drifts the same direction forever.
`a_wanderer_uses_every_direction_and_sometimes_holds_still` is what says it
reached — nine outcomes over 900 beats, where a fold that reached nothing
shows one. The ninth outcome is the hold, and it is there because a body that
never pauses reads as unnatural as one that never moves.

**`park_tile` survives as `entry_tile`, and it has a job the drift cannot
do.** A tamed program's `Position` is the surface tile it was beaten on; it
has no base-space cell until something gives it one, and this pass is still
what does — the property `post_worker` stopped writing a `Position` on the
strength of. So a body not standing on laid floor takes the ring, arrives,
and drifts from there. In an established base that fires once per program
and never again.

**Laid floor, not `walkable`, and that is the leash.** `base_entropy_system`
reverts a mined `Open` cell nobody is standing on, and a body holds only the
cell under its own feet — so a wanderer that strolled down a fresh corridor
would be sealed in behind it, and `hauling::post_field` gates its own start
tile on `walkable`, which leaves that body unpostable and unreachable for
the rest of the run. `Floor` never reverts. Confining the drift to it closes
that by construction, which is why there is no roam radius to tune: the
question "how far may it go" is answered by what the player has paved.
`a_drifting_program_stays_on_laid_floor_and_off_the_structures` carves an
unfloored corridor off the pocket's edge on purpose — without one, `walkable`
and `is_floor` answer the same on every tile the fixture owns and the test
passes against either rule.

**Two rejections the ring got for free.** It never put two programs on one
tile because their ring offsets were spread by index; a drift has to be told,
and the tiles the idle pool holds are collected at the top of the beat and
grown with every tile written during it. Nothing is ever removed from that
set — a vacated tile stays spoken for until the next beat, which costs a body
one step it will be offered again and buys independence from the order the
pool is walked in. And the party's own cell is refused, read off `Locale` and
never the player's `Position`, which is pinned to the anchor tile out on the
zone surface: a program standing on the party hides the `@`.

**The cost, stated rather than tuned away.** A body can now be idling at the
far side of the floor from the machine the scheduler next wants it at, where
the ring kept the whole pool within three tiles of Home. That is real, it is
small, and bodies walking in from wherever they were is the point of the
feature. If it ever reads badly the fix is a leash radius, not the ring.

### `task_progress_system` and `assembler_system` both write `Task::progress`, for disjoint targets, and are `.chain()`ed anyway

**`task_progress_system` and `assembler_system` both write `Task::progress`,
for disjoint targets, and are `.chain()`ed anyway.** Bevy can see the
conflict but not the disjointness, and an arbitrary-but-fixed order is not
the same as a stated one. An assembler's rate comes from **`Task::required`,
not from its def's `ticks_per_unit`** — the def's number is only the
baseline, scaled by the posted program's `base_speed` and baked in at
assignment by `Game::work_ticks_for`. This inverted a previously documented
seam (0.7.x, species classes phase 2), on the evidence that
`upgrade_structure` never touches `ticks_per_unit`, so nothing changes a
machine's rate after a program is on it, and `displace_task_holder` allows
only one `GatherResource` task per structure. Known cost: a cronjob in a
save written before the change keeps its old rate until it is re-posted.
A test fixture that hand-writes a `Task` against an assembler must set
`required` to that machine's real `ticks_per_unit` — a hand-written `1` used
to be inert and is now "finish a batch every tick".

### A test fixture that hand-spawns a work node needs `work_node_parts()`, and a fixture that posts a program to one needs `park_at_post()`

**A test fixture that hand-spawns a work node needs `work_node_parts()`,
and a fixture that posts a program to one needs `park_at_post()`.** A node
short of `Stock` or `MachineStatus` is skipped by `task_progress_system`'s
query and silently produces nothing; a worker left where it was spawned is
not orthogonally adjacent to its machine, so the `Unstaffed` gate holds
production at zero until it has *walked* there. Both read as a payout curve
that moved rather than as a fixture short something.

### A trader's buyback shelf is keyed by `(kind, tile)`, not by `Entity`

**A trader's buyback shelf is keyed by `(kind, tile)`, not by `Entity`**
(`resources::BuybackLedger`). It deliberately outlives the building, so a
raided Market rebuilt on the same footprint reopens with its stock. That
also means `enter_next_zone` has to clear it *explicitly* — which brings
up the wider trap: **breaching does not despawn structures.** The base
travels, repositioned around the new spawn point; the only despawn is the
no-Home fallback. Anything zone-local has to be wiped by name.

### `BattleState::planned` indexes `Party` positionally

**`BattleState::planned` indexes `Party` positionally** (see
`actor_entity`). Nothing may leave `Party` mid-battle — removing a member
shifts every member behind it into the wrong slot. Deferred removal is why
`end_battle` exists.

### An initiative order names the party by slot and the wild side by identity

**An initiative order names the party by slot and the wild side by
identity**, and the asymmetry is the point. `battle::Actor::Party(slot)` is
an index because `Party` cannot shrink mid-battle (the entry above) and
because the plan is indexed by slot, so an emptied slot still has to be
addressable. `battle::Actor::Enemy(Entity)` is an entity because the wild
side *does* shrink mid-round.

Its doc comment said the opposite for a year — "an index rather than an
`Entity`, so a resolution walk can survive members dying mid-round" — and
that is exactly the case it could not survive. `roll_initiative` runs once
at the top of the round; `remove_member` then drops each dead member and
drops the group entirely once that empties it, shifting every index behind
it down one. A stale `Enemy { group, slot }` resolved at its own turn named
whoever had since slid into its place.

Both halves shipped, and **a count of enemy swings can see neither** — the
shift conserves the number of actors, since removing one member leaves the
last index resolving to `None`. What it does not conserve is who:

- the group behind a fallen one swung **twice**, once on its own initiative
  and again on the dead group's, which is the bug as a player reads it: a
  kill making the next pack member hit you in the same breath;
- a group whose index had moved off the end **lost its round in silence**,
  which reads as the pack going passive the moment something dies.

So `a_group_that_falls_mid_round_neither_lends_nor_steals_a_turn` counts by
**move name** rather than by line count: each test species carries one named
move, and the log prints a move's name on every outcome — hit, miss, crit
and all four fumble rungs alike. It is swept across seeds because the
player's opening strike is capped at `HIT_CHANCE_MAX` and misses on some of
them; the punching-bag group has no move at all, so one swing each is the
answer whether it falls or not, and the sweep needs no branch. The fixture
installs its species into `SpeciesDb` with speeds of 1 and 100 — far
outside the shipped roster's 6..14, because the gap has to exceed
`INITIATIVE_DIE` for the acting order to be a fact rather than a seed's
opinion — and states its own groups through
`support::insert_battle_with_groups`, since `group_pack`'s ceiling is a
zone reading that would collapse them.

**The group index did not become an entity with it.** `wild_retaliate`
still takes one, because that is *where the program is standing*: it gates
reach against `ENGAGED_GROUPS` and names the group an own-side routine
lands on, and `policy.rs`'s back-group tests pass a synthetic index to a
hostile that is really in group 0. What changed is that the two real
callers now read it live, off `Game::group_of(entity)`, at the moment the
program swings — so a group promoted forward by a kill in front of it is
engaged, rather than swinging from the index it held before anyone died.

### A log line carries two independent axes, `MessageKind` and `MessageSource`

**A log line carries two independent axes, `MessageKind` and
`MessageSource`.** Kind is read by three consumers that each mean something
different by it — `render/mod.rs`'s colour table,
`retain_outcomes_since_battle`'s prune, and `condense`'s notion of line
identity — which is why "this came from the base" is a second field rather
than a `BaseRaid`-style variant: a raid alert has to stay
`MessageKind::Raid` for all three *while* being base news. `Field` is the
default, so `log`/`log_kind` are untouched and only the base-side sites use
`log_base`/`log_base_kind`. Power reserves are field on purpose — a need
follows you into the Stack. A new tagging decision goes in the table in
`MessageSource`'s doc comment, not in a caller's head.

### `MessageSource` has two readers, and the battle pane's is not the filter

**`MessageSource` has two readers, and the battle pane's is not the
filter.** `App::log_filter` is the map pane's, and it is a player's choice.
The battle pane's is unconditional: `battle_rows` drops base news outright,
because `MessageLog::since_round` slices by *position* and the trailing
`tick` in `battle_resolve_round` pushes whatever the base did into the round
the party is fighting — a sweep, a clog, a cronjob payout, scrolling into
the fight with no round header or roster change to explain it, and again
into the result screen. Two things about where that filter sits are
load-bearing. It is app-core's and **not** `Game::battle_log`'s, because
everything that paces the reveal counts *raw* lines — `revealed_count`,
`finish_reveal` and `BattleTimeline`'s frames all index the unfiltered
range — so a source filter upstream would leave the pacing counting rows
the pane never draws. And `battle_rows` truncates *before* it filters for
the same reason, which is the mirror image of `pane_rows` doing it the other
way round. The known cost, accepted rather than overlooked: the reveal still
spends a beat on a base line the pane never draws, so a round that ends with
one holds `is_revealing` for an extra ~0.25s. Base news is ~0.25 lines a
tick against a running base, so that is usually no beat at all; the fix
would be a source-aware chop in `pane_rows`, which a contiguous raw suffix
cannot express.

### The reveal is gated on `Mode::is_battle`, and that gate is what keeps it off the map

**The reveal is gated on `Mode::is_battle`, and that gate is what keeps it
off the map.** `App::unrevealed` is the one definition of "the narration is
behind" — `is_revealing` and `hidden_log_lines` both read it — and it
returns 0 on every non-battle screen. The reason is that
`MessageLog::round_start` is deliberately *never closed*: the results are
still scrolling in after the fight, so the range has to outlive it, and the
consequence is that `Game::battle_log` goes on growing with ordinary map and
base news for the rest of the run once a fight has opened it. Ungated, that
paced the map's own log pane at `REVEAL_LINES_PER_SECOND` forever, and —
through `handle_key`'s unconditional skip — **swallowed one keypress for
every line a running base logged**, which is what the player felt as keys
not registering. Fixed 2026-08-12. The gate is the mode rather than "a
battle is live" because `Mode::BattleResult` outlives `BattleState` and is
exactly when the results are scrolling in. Note what this leaves: the
`hidden` parameter of `pane_rows` is now always 0 from `App::visible_log`,
since no battle mode draws the map — kept because it is what states the
chop-before-filter order, not because anything currently chops.

### The map log pane is filtered; the history screen (`L`) is not, and that asymmetry is deliberate

**The map log pane is filtered; the history screen (`L`) is not, and that
asymmetry is deliberate.** `App::log_filter` thins the pane through
`pane_rows`, whose stage order is load-bearing: an unrevealed tail would be
counted in *raw* lines (`hidden_log_lines`) so it comes off before the
filter, the filter comes off before the fold or identical text from the
base and from the field would merge and then be drawn under whichever
channel won, and the fold comes off before the capacity cut or a screenful
of base chatter — or of one sentence repeated — leaves the field pane blank
with older field lines still in reach. History stays complete because its
row count is shared — app-core bounds the scroll while gui draws the rows,
so filtering one side only would open the screen on a row that isn't
drawn.

### All three log surfaces fold repeats, and the fold is always the last stage

**All three log surfaces fold repeats — the history screen, the map pane
and the battle pane — and the fold is always the last stage before the
capacity cut.** `resources::condense` is the one definition and was the
history screen's alone until 2026-08-18, when a round that killed seven
programs was reported as reading "The rogue program crashes and deletes
itself!" seven times over. That line (`Game::finish_member`) names nothing,
so seven copies carry exactly as much as one and a count.

Two placements were available and only one is safe. **Collapsing in
storage** is what `condense`'s own doc has always refused: `MessageLog`
carries the mark arithmetic that `since_round` and
`retain_outcomes_since_battle` slice with, and merging a new round's first
line backwards across `open_round` would drop it out of the battle pane's
range entirely. **Folding upstream of the pacing** breaks a different
thing: `revealed`, `hidden_log_lines` and `BattleTimeline`'s frames all
count *raw* lines, and `Game::battle_view_at` replays the roster by that
same figure — so a folded count reaching any of them steps the roster to
the wrong moment.

So the fold sits in `pane_rows` and `battle_rows`, on the rows about to be
drawn, after the truncation and the source filter. The consequence is
visible and was chosen rather than tolerated: seven kills still take seven
beats to scroll in, and what the player watches is the count ticking up on
one row. The alternative — one row appearing and the reveal skipping six
beats — would make a wipe read as a single kill.

It borrows `CONDENSE_LOOKBACK` rather than folding adjacent runs because
adjacent runs would fold nothing here: `finish_member` logs "Another rogue
program from the pack engages!" between kills whenever the front rank
falls, so the copies are never neighbours. The window is what the history
screen already used for exactly this shape — two producers interleaving
their lines each cycle.

gui draws the count through one function, `draw_message_line`, in the same
`×N` form `popup.rs::counted_item_row` writes on the history screen, so a
repeated line reads the same on all three surfaces.

### `MessageLog::retain_outcomes_since_battle` keeps only `Outcome`, `Loot`, `LevelUp`, `Raid` and `Complete`

**`MessageLog::retain_outcomes_since_battle` keeps only `Outcome`, `Loot`,
`LevelUp`, `Raid` and `Complete`.** A plain `log()` is `Info` and is pruned when the
battle ends. A line that must follow the player onto the map needs one of
those four kinds, and anything logged during `end_battle` is subject to the
prune depending on where it lands relative to the call.

### A read-only screen's row count is owned by app-core and drawn by gui, so any per-row transform has to live in the engine

**A read-only screen's row count is owned by app-core and drawn by gui, so
any per-row transform has to live in the engine.** The history screen
scrolls by moving `menu_selected` (`App::handle_history_key`,
`App::opening_row`), which bounds itself against a count app-core derives
itself — while `render/base.rs` builds the rows. `Game::message_history`
folds repeated lines into one row apiece, and both sides call it; folding
in the renderer instead would have opened the screen on a row that isn't
drawn. Same shape for `Game::structure_report` and the `B` roster.

### `world.get::<Stats>(e).is_none()` is the idiom for "this entity is gone"

**`world.get::<Stats>(e).is_none()` is the idiom for "this entity is
gone"** (`tests/trade.rs`). Don't reach for `World::get_entity`.

### Engine test fixtures live in `crates/engine/src/tests/support.rs`

**Engine test fixtures live in `crates/engine/src/tests/support.rs`** —
`spawn_tamed`, `spawn_wild_on_player_tile`, `insert_battle`,
`flee_until_clear`, `set_level`, `test_assets_dir`, `resolve_round_with`,
`spawn_bare_nest`, `spawn_pursuing_guardian`, `park_at_post`. Look there
before writing a new one.

### `Pursuing` must only ever be inserted alongside `NestGuardian`

**`Pursuing` must only ever be inserted alongside `NestGuardian`.**
`nest_aggro_tick`'s driving pass collects `With<Pursuing>` unconditionally,
but its leash pass reads `NestGuardian` to find a nest to measure from —
an untethered `Pursuing` has no leash, is never cleared by `despawn_nest`
(which strips both together), and would chase indefinitely. Not reachable
today; all three insertion sites (`attack_nest`, `nest_respawn_tick`, and
the `spawn_pursuing_guardian` test fixture) already pair the two. This
invariant used to live only in a test comment.

### `walkable()` alone does not decide where a `Pursuing` nest guardian may step

**`walkable()` alone does not decide where a `Pursuing` nest guardian may
step.** `pursuit_field` (`game/pursuit.rs`) excludes `Biome::Platform`
separately, on top of the ordinary walkability check — the leash is
measured from the nest, which cannot by itself keep a swarm off a base
built within leash range of one. A second "walkable but off-limits" rule
belongs in that filter, not beside it in a caller.

### There is one Dijkstra walk on the surface, and the step rule is a parameter

**There is one Dijkstra walk on the surface, and the step rule is a
parameter.** `walk_field` (`game/pursuit.rs`) is the search;
`pursuit_field` is a one-line wrapper that adds the `Biome::Platform`
exclusion above. A hauling program has to cross the base slab, which is
exactly the tile set that filter removes, so the two callers genuinely
disagree about one tile and agree about everything else. A third caller
widens the predicate rather than copying the walk — this is the
"a doc comment claiming to mirror other code must be a call, not a copy"
rule applied before the second copy existed.
The predicate takes **the coordinate as well as the tile**, because only
one of the two rules is about terrain: a hauler must also refuse any tile
a `Structure` stands on, which is entity state and unreadable from a
`Tile`. That was added on 2026-08-11, when a hauler still walked straight
over machines the player had to walk around. `station_tiles` carries the
same filter — an occupied neighbour nominated as a station is a tile the
worker is sent to stand *on* — but `post_field` admits the worker's own
tile whatever occupies it, since `place_structure` never checks whether a
program is standing there and a worker built over would otherwise be
absent from its own field forever. You may step off an occupied tile,
never onto one.

**`station_tiles` yields all four faces, not the nearest one**, and
`post_field` walks them in that order and stops at the first that routes.
The four faces of a target are not always in the same part of the base, and
picking one up front made the nearest face stand for the whole answer: a
`(distance, x, y)` tie went to the lower `x` whether or not anything could
be reached through it. That was survivable while every target was a machine
the player had built somewhere they could stand. It stopped being
survivable with dig sites, whose faces are disconnected *by construction* —
a marked cell on a rock spur has the corridor on one side and unbroken rock
on the other — and whose refusal latches `DigSite::announced_stuck`, so one
tie broken the wrong way skipped the site for the rest of the run. A post
that already resolved still resolves through the same tile at the same
cost, because the old choice is still first in the order; the extra walks
are paid only where the answer was about to be `NoRoute`, and there are at
most three of them.

### `Carrying` is the only thing hauling stores, and the carry cap is what lets it be one `(item, qty)` pair

**`Carrying` is the only thing hauling stores, and the carry cap is what
lets it be one `(item, qty)` pair.** Where a posted program is headed and
whether it has arrived (`hauling::at_station`, over `collect::ORTHOGONAL`)
are both derived from `Position`, so there is no state field to desync
into a worker standing at its machine insisting it is still walking.
What it is *doing* is `hauling::Errand`, derived once per worker per tick
and never stored: `Deposit` (clear a load into a structure's output),
`Load` (an ingredient arriving home, into a machine's input), `Collect`
(draw an ingredient off a shelf) and `Tend` (stand at the post, and pick
up if there is anything to shed). One enum rather than a destination
beside a separate arrival branch, because there are four of each and they
have to agree — a worker sent to a depot and then asked to unload into a
machine spins there for the rest of the run. Every variant carries
**owned** data, which is not stylistic: deriving the errand reads the
structures query and applying it writes to that same query, so a variant
holding a borrow would keep the read alive across the write.
**Direction needs no field**, and that is what keeps `CreatureSave::
carrying` — a positional tuple RON cannot widen — untouched: a load the
machine's own recipe has room for is an ingredient coming home, anything
else is product being cleared. That
is also why a demolished depot needs no notification: it stops being the
answer on the next tick. `Stock::output` is a `BTreeMap` and may hold
several ids, so an *uncapped* drain would have needed
`Carrying(BTreeMap<..>)` and a matching map in the save.
Two things do have to be written by hand. Both destruction paths
(`remove_structure`, `damage_structure`) drop `Carrying` with the `Task`,
or a worker keeps a load with nowhere to put it forever. And
`task_progress_system` gates production on `at_station && !carrying` —
the second half is not belt-and-braces: a worker that produced on the tick
it arrived home with a rejected load would refill the very room that load
needs and be left holding the remainder.

### Departure lives in `haul_step_system`, not in `task_progress_system`'s clogged branch, because it has to know whether a depot exists

**Departure lives in `haul_step_system`, not in `task_progress_system`'s
clogged branch, because it has to know whether a depot exists.** The
invariant it buys is that a base with no depot behaves exactly as it did
before depots shipped (`with_no_depot_a_clogged_machine_just_stays_
clogged`); put the pickup at the clog and a depot-less base sheds five
units into a load nobody can ever put down.
**A clog is no longer the only trigger.** `hauling::consumer_beside` asks
whether an orthogonal neighbour's own recipe names this machine's product —
the *attached building* that makes an output buffer a feed buffer. Without
one, the worker takes each cycle's payout to the nearest depot instead of
hoarding twenty units where the base cannot count them. Asked of the
**recipe**, not of whether the neighbour is currently pulling: an unstaffed
or clogged consumer is still the building the output belongs to, and the
clog path covers a line that has genuinely backed up.

**But the recipe alone was never enough, and "an unstaffed consumer is
still the building the output belongs to" was the sentence that hid it.**
The `consumers` list `consumer_beside` reads is now built only from the
assemblers **the base has a reason to run** — the work-order queue naming
what one makes, or a standing work job on it. A Lathe standing beside a
Mining Node with nothing asking for Blank Substrate pulls *nothing*
(`assembler_system` returns before its pull phase with no program posted),
so counting it reserved the node's whole twenty-unit buffer for a machine
that would never take a unit of it. Measured on the reported case — one
worker, an order for sixty Core Fragments, a Lathe beside the node — the
first fragment reached the Depot at tick 500 and the order closed around
1350, against a delivery on the first cycle for the same node standing
alone. Nothing was lost: `base_holding` counts machine buffers, so the
order still completed, which is why this read as sluggishness rather than
as a stall and survived to 0.9.2.

The reason to run is `work_orders::queue_needs`, the closure of the queue's
items under `ItemDef::craftable`. Three things about its shape are
load-bearing:

- **Over items, not over deployed machines.** `haul_step_system` has
  `WorkOrders` and `ItemDb` and no `Game`, so an entity walk like `wants`'
  would have had to be copied into the system — the second copy that
  drifts. The item graph is also the more stable question: whether the base
  has been *asked* for something does not flicker as machines pass in and
  out of `can_progress`, which a `wants` reading would have made it do
  several times a cycle.
- **A closure, not the ordered item.** An order for Routine Disks reaches
  Core Fragments two links down, and a one-hop rule would take the Lathe
  for a bystander and dismantle the very line the order was filed to run
  (`an_order_two_links_downstream_still_keeps_the_feeder_hoarding`).
- **The whole queue, not the order being worked.** A line feeding order
  three must not be taken apart while order one is worked; the only thing
  that would come of it is the same goods walked to a Depot and back.

The standing-job half is not a special case bolted on: a standing work job
is the player saying *keep this running* outside any order, and an empty
queue is not an instruction to take a hand-built line apart
(`a_standing_job_on_the_neighbour_is_reason_enough_to_keep_feeding_it`).

**What this does not do is bound the hoard**, and that was the other
candidate fix — reserve the neighbour's `input_room` and haul the surplus.
It is wrong, and the reason is worth keeping: a producer that hauls its
surplus never clogs, and **the clog is what hands a lone body downstream**
(`can_progress` is false for a clogged machine, so it stops wanting a
body). A one-worker Mining Node → Lathe → Disk Press line was watched
oscillating on exactly that trigger for 600 ticks; bounding the hoard
freezes the body at the extractor and the line never runs at all.
**The cost falls on extractors alone**, and that asymmetry is worth
knowing before retuning either half: `task_progress_system` gates on
`at_station`, so a Mining Node pays for every trip its program makes,
while `assembler_system` finds its worker by `Task::target` and never asks
where it is standing — a bench keeps pressing while its program shuttles.
So depot placement now paces a lone extractor and nothing else. That is
the number most likely to want retuning, and if it feels wrong in play the
fix is one predicate (depart at a full carry load rather than every cycle),
not a redesign.
A depot is `StructureDef::stores`, a `#[serde(default)] bool`, and not
"has a `Stock` and runs no job" — `place_structure` gives *every*
structure a `Stock`, so that rule would make a Home, a Shield, a Portal
and a Data Cache all depots.

### A `NestGuardian`'s tether refuses a step only when it both leaves `NEST_TETHER_RADIUS` and fails to close on the nest

**A `NestGuardian`'s tether refuses a step only when it both leaves
`NEST_TETHER_RADIUS` and fails to close on the nest** (`wander_ai_system`,
`systems.rs`). The simpler "outside the radius" check was total until
pursuit could displace a guardian past its own tether — at which point
every neighbouring tile still counted as leaving the radius, and the
guardian had no legal move at all and froze for the rest of the run.
Anything that can push a `NestGuardian` beyond `NEST_TETHER_RADIUS`
depends on this fix already being in place.

### `nest_aggro_tick` is a reader of the player's `Position` and therefore needs the underground guard, even though it is not a player action and so never went through `require_surface`

**`nest_aggro_tick` is a reader of the player's `Position` and therefore
needs the underground guard, even though it is not a player action and so
never went through `require_surface`.** Without it, a provoked guardian
walks to the surface entrance tile the party descended through and opens
a *surface* battle while `Locale::Stack` is live — which makes
`fight_depth` apply Stack-depth scaling to it and makes `raise_trace` fire
on every kill. Contrast `maybe_spawn_wild_creature`, which reads the same
pinned `Position` and is harmless because it only *places* creatures. The
distinction that matters is whether the code drags the player into
something, not whether it reads `Position`.
**It asks two questions now, not one** — `is_underground() || in_base()` —
because `Position` is pinned to the anchor tile in base space exactly as it
is to the entrance tile in the Stack, and `is_underground` deliberately
stays Stack-only. Same for `power_regen_system`, which is on the other
shape of the same guard: it matches `Locale::Surface` positively, so a
third locale refused it for free.

### `start_battle` is the only path that caps a pack, and `begin_battle` is the one that opens a battle

**`start_battle` is the only path that caps a pack, and `begin_battle` is
the one that opens a battle.** The split exists for `arena`, which authors
its own composition and must not have it truncated to
`group_size_ceiling() x enemy_group_ceiling()` — a scenario asking for
nine at zone 1 would otherwise silently be given one, and the tool would
answer a question nobody asked. So `start_battle` is now
`group_pack` then `begin_battle`, and everything a fight *is* lives in the
latter. A third caller that wants capping calls `group_pack` itself; one
that does not goes straight to `begin_battle`. The two ceiling helpers are
`pub(crate)` solely so the arena can *warn* that a composition exceeds
them, which is the "no silent caps" rule — they decide nothing there.

### There are two battle rosters, and which one a caller wants is decided by whether it *draws* or *acts*

**There are two battle rosters, and which one a caller wants is decided
by whether it *draws* or *acts*.** `battle_resolve_round` resolves a
whole round in one call while the frontend scrolls the narration in at
`REVEAL_LINES_PER_SECOND`, so for a second or two the live roster is
ahead of what the player has read. `Game::battle_view` is that live
truth; `Game::battle_view_at(revealed)` replays `resources::
BattleTimeline`, a frame recorded after every battle log line, and is
what `App::battle_view` and therefore `render/battle.rs::draw_battle`
read. The two disagree, deliberately, and the rule is: anything mapping
a typed group letter onto `BattleState::groups` takes the live one —
`App::battle_target_key` and the picker `draw_battle_target_menu`
draws for it — because a rewound row would resolve a letter against a
group that has already died. That stays safe only because `handle_key`
skips the reveal rather than acting while `is_revealing` holds.
Three things about the timeline are load-bearing. It stores **rendered
rows**, not entities or HP numbers, because `finish_member` despawns a
victim mid-round and an emptied group is dropped from
`BattleState::groups`, re-lettering everything behind it — rows make
deaths, counts, letters and decompile odds rewind together, and there is
no dead entity to read them back off. Frames carry a *line count* rather
than an index, and lookup takes the last frame at or under it, because
`battle_resolve_round`'s trailing `tick` lets background systems push
straight into `MessageLog` with no frame taken — an unframed line simply
holds the previous frame on screen. And a frame is taken at **zero**
lines, before the round header goes out: `App::revealed_count` reports
zero for the whole gap between a round resolving and the next frame
drawn, so falling back to live rows there would flash the finished round
for one frame before the narration started. Damage flashes and floating
numbers were already inferred from frame-to-frame HP deltas
(`render/battle.rs`), so they fire per line without knowing any of this
exists.

### A finished fight keeps the battle screen; it does not hand off to a summary page

**A finished fight keeps the battle screen; it does not hand off to a
summary page.** `Mode::BattleResult` renders `draw_battle` — same
rosters, same log pane, with the decisive round and the results scrolling
into it and the action bar replaced by a continue prompt. The mode exists only so
keys stop planning actions against a battle that is over; it is not a
second screen, and a `draw_battle_result` of its own was built, played,
and removed for exactly that reason.
What makes it work is `BattleTimeline::closing`, captured at the **top**
of `end_battle` — before `dissolve_tamed_program` drops the dead out of
`Party` and despawns them, which is the only moment a companion that
died winning the fight still exists to be drawn. `App::battle_view`
answers from it on that mode alone, gated on the mode rather than on
"no live battle" so a stale roster cannot surface on the map
(`the_closing_roster_does_not_leak_onto_the_map`). Its hostile half is
**empty on a win** and populated on a jack-out, both deliberately:
`finish_member` only reaches `end_battle` once `remove_member` has
emptied the last group, so the pane clearing is what winning looks like,
and what you ran from is what fleeing looks like.

### A battle does not end when the player's HP hits zero, and three different things heal them before anyone outside can look

**A battle does not end when the player's HP hits zero, and three
different things heal them before anyone outside can look.** The round
loop resolves to the last enemy falling or the player fleeing; a defeat is
absorbed *inside* the round that lands it by `difficulty::
death_handling_system`, which in Forgiving reboots the player to a
fraction of max HP. So "did the player win" cannot be read off their HP
afterwards — `arena::run_rep` reads it off the opponents instead, which is
what winning actually means. The same trap bites the *measurement*: a
level-up full-heals in `progression::add_xp`, and the killing blow is
usually the level, so an HP fraction sampled after the fight reports a
hard-won win as free. The arena samples per round and skips any round that
granted a level.

### A fight's rewards are granted per kill and announced once, and the split is the whole of it

**A fight's rewards are granted per kill and announced once, and the
split is the whole of it.** `award_loot` and `award_player_xp` still run
inside `finish_member`, where they always did; what they now do is fold
into `BattleState::rewards` (`resources::BattleRewards`), which
`Game::settle_rewards` flushes at the top of `end_battle` as one salvage
tally and one XP line per fighter. **Moving the award itself to the flush
is the change to refuse**: a level-up full-heals inside
`progression::add_xp` and the killing blow is usually the level (the entry
above), so a party that levelled on the last kill would finish the fight
at the HP it had before the heal, and every arena and `balance_sim` number
would move with it. The level is therefore still announced live and
tersely — the HP bar snapping to full mid-fight needs a cause on screen at
the moment it happens — while the totals and the stat block wait.
Four things about it are load-bearing. The buffer is a field on
`BattleState` rather than a `Resource`, which buys both of that field's
neighbours' payoffs at once: battles are never serialised, so no
`SAVE_FORMAT_VERSION` bump, and no new resource to shift bevy's query
iteration order under an unrelated test. The flush sits **above**
`dissolve_tamed_program` (the last moment a companion that died winning
is still nameable) and ahead of `retain_outcomes_since_battle`, whose four
surviving kinds are exactly the ones the tally carries — the prune waits
for the player to leave the results screen now (its own entry below), but a
tally written after it would still be in the right place and the wrong
order. One flush
point covers a win and a jack-out alike because `end_battle` is the only
place `BattleState` is dropped — you keep what you killed before you ran.
And with no battle live, `record_drop` and `award_player_xp` announce on
the spot through the *same formatters* the flush uses, so the two paths
cannot word a drop differently; nothing reaches `award_loot` outside a
fight today, and that fallback is what stops the next thing that does from
paying the player silently.
The tally is a header plus one indented row per distinct `GearCopy`, not
one comma-joined line, because `pane_rows` draws a `LogLine` as exactly
one row and never wraps it — a joined line's width would grow with the
haul. Merging on the whole copy is `BuybackLedger`'s argument reached
again: keyed on the item alone, an Overclocked copy would be tallied as
another ordinary one.

### The prune waits for the player to leave the results screen

**`MessageLog::retain_outcomes_since_battle` deletes lines, and used to run
inside `battle_resolve_round`.** `end_battle` called it, and `end_battle`
runs inside the round that ends the fight — so the decisive round's
blow-by-blow was deleted before a frontend had revealed a single line of
it. Everything downstream was correct and the player still never saw the
final blows: `settle_after_round` flipped to `Mode::BattleResult` and
restarted the reveal, and what scrolled in was the kill line followed
immediately by `Salvage:`. The fight appeared to skip its own ending.

The prune is now `Game::prune_battle_narration`, called when the player
*leaves* that screen. The results page reads: the final round's swings, the
outcome, the salvage, the XP — which is the order the fight happened in.

`Mode::BattleResult` has exactly one key handler (`input.rs` routes every
key in that mode to `handle_battle_result_key`) and nothing ticks there, so
app-core has two exits to get right and no more: that handler on the way to
the map, and `check_game_over` on a run that ends on the losing round. Both
go through `App::leave_battle_result`. Miss one and the blow-by-blow follows
the player onto the map, which is the whole thing the prune exists to stop.
A two-stage prune — everything before the final round at `end_battle`, the
rest at the exit — was considered and rejected: it guards against a third
exit that does not exist, at the cost of a second prune shape.

Two consequences worth knowing. The roster beside the narration is
`BattleTimeline::closing` for the whole of the results screen — the frames
are still dropped at `end_battle`, because they index a roster that goes
with `BattleState` — so the final blows are read against an already-empty
hostile pane. That was already true of the kill line; this extends it by a
few rows rather than introducing it. And `dissolve_tamed_program`'s `Info`
detachment lines used to be written and pruned in the same call, never
reaching anything; they now scroll past on the results page before the
prune takes them, so a companion that died winning reads its death line and
then its departure.

`MessageLog::keep_battle_narration` is unaffected and still exists for
`arena`, whose report *is* the blow-by-blow: a flag rather than better
reading, set only there. Nothing else should set it; the prune is right for
every reader that has a pane.

### A won fight says so, and it is the only ending that needed telling

**A won fight says so, and it is the only ending that needed telling.**
`settle_rewards` heads the results with "You won!", read off
`BattleState::groups` being empty — the same one definition `end_battle`'s
telemetry takes eight lines later, and for its reason: a defeat is absorbed
inside the round that lands it by `difficulty::death_handling_system`, so
the player's HP afterwards says nothing about the outcome.

**The other two endings are deliberately left alone, and that is not a
narrowing of the feature.** They already declare themselves in this exact
slot, one line higher: `battle_flee` logs "You jack out safely." (or the
counter-strike wording) immediately before calling `end_battle`, and a
flatline is announced by `death_handling_system` inside the round that
lands it. A headline for those would be a second line saying what the first
just said. A `BattleOutcome` parameter threaded through `end_battle`'s four
call sites was specced for this and dropped once that was clear — the
information the flush needs was already in `BattleState`, and a parameter
carrying it would have been a second answer to a question with one.

The XP lines take an `Experience:` header and the two-space indent
`Salvage:` rows already carry. The lines are **built before the header is
written** (`announce_xp` over `player_xp_lines` / `companion_xp_lines`), so
a header can never stand over an empty block; asking two predicates whether
anything is about to print would have been a second copy of the guards
inside the builders, and the copy that drifts is the one nobody runs. The
two out-of-battle callers take the same builders and log them unindented
and unheaded, since outside a fight those lines are standalone news rather
than rows in a block.

### There is one way into a staged arena fight, `arena::stage`, and one reader of what one cost, `arena::Watch`

**There is one way into a staged arena fight, `arena::stage`, and one
reader of what one cost, `arena::Watch`.** The headless bin and the game's
arena screen (`FERAL_DEV_ARENA=1`, main menu `[R]`) are two people pressing
the keys on one code path — so they cannot disagree about the RNG stream,
`keep_battle_narration`, who counts as an opponent, or the two non-obvious
parts of reading an outcome (HP sampled per round and skipped on a level;
"won" read off the opponents, never the player). `run_rep` is now only the
auto-play loop. An app-core copy of the outcome logic is the copy `CLAUDE.md`
forbids *and* the one nobody runs: the headless path would keep working
while the screen quietly lied.
`staging_then_running_matches_run_at_the_same_seed` is what holds it.

### An arena session touches no disk, and all three of those are omissions

**An arena session touches no disk, and all three of those are
omissions.** `App::in_arena()` is the one predicate: `after_tick`
early-returns on it, covering `flush_profile_writes` and `maybe_autosave`
together, and `check_game_over` guards separately — it is not a post-tick
concern, and a `Save` player source can carry Permadeath in, so a lost
arena fight *is* a reachable `is_game_over` that belongs on the result
screen rather than in `run_history.log`. The one that costs real money if
it regresses is the profile: a rung earned in a tester's fight would be
written to the real `profile.ron` and then paid out to every future new
game by `grant_profile_rewards`. Each has its own test asserting on the
*file* (`an_arena_fight_writes_no_save`, `..._no_profile`,
`an_arena_loss_writes_no_run_history`), because an omission is invisible
otherwise and the regression is a later change adding one back.
`settle_after_round` is the single hook the fight is read through, and the
jack-out arm calls it unconditionally: a *refused* flee resolves a round
too.

### Battle telemetry is the fourth thing an arena session touches, and it is allowed to write

**Battle telemetry is the fourth thing an arena session touches, and it is
allowed to write.** The entry above holds because a tester's fight must not
corrupt a save or pay a real profile reward; `dev-logs/battles.jsonl` does
neither, and the arena is the single place a recorded fight is most wanted.
So `App::flush_battle_telemetry` sits **above** `after_tick`'s `in_arena()`
early return rather than below it, and `an_arena_fight_still_writes_
telemetry` is what stops the next reader taking that invariant as absolute
and folding the flush back in for tidiness. Three further things are
load-bearing and each is an omission or an ordering. `serde_json` is
app-core's dependency and never the engine's — the engine derives
`Serialize` and hands over values, which is why `telemetry.rs`'s own test
round-trips through `ron` and the JSON properties are asserted in app-core
against the real written file. `Game::record` takes
`impl FnOnce(&Game) -> Record` rather than a value, because an eager form
builds three `String`s per swing even while disabled and `train` pays that
1.9M times a session; the `&Game` parameter is what makes the lazy form
borrow-check while `record` holds `&mut self`. And `arena::stage` takes the
flag as a **parameter** rather than reading it off the installed `Game`,
because `stage` calls `begin_battle` itself — a game armed after staging
joins its own fight already in progress and loses `fight_start`, which is
how this was found. `App::install_game` is the matching one door for every
other path, so a fourth site that installs a `Game` cannot silently collect
nothing.

### `nest_aggro_tick` is the first code in the game to call `start_battle` from inside `tick_inner`

**`nest_aggro_tick` is the first code in the game to call `start_battle`
from inside `tick_inner`** — every other caller is player-action-driven.
That is why `rest`'s tick loop needed a battle check it never needed
before (`Game::rest`, `game/turn.rs`); anything else that starts a fight
from a tick inherits the same obligation.

### A profile pays at `Game::new` and never at `Game::load`, and the enforcement is an omission

**A profile pays at `Game::new` and never at `Game::load`, and the
enforcement is an omission.** `install_profile` says *what has been
earned* and both paths call it — `achievement_system` must not re-earn a
rung on a loaded save. `grant_profile_rewards` says *pay for it*, and only
app-core's new-game path calls it. A save already has its bonuses baked
into `Stats` and `Perks::points`, so paying again on load doubles them on
every single reload — invisibly, since a stat carries no record of where
it came from. That is why the two are separate operations rather than a
flag on a shared constructor: the rule then lives at one call site instead
of inside `Game::new`, whose signature this also leaves alone (667 call
sites, essentially all engine tests). `crates/app-core/src/app/lifecycle.rs`
carries a comment on the *absence* of the call, because an omission is
invisible otherwise, and `loading_a_save_does_not_re_apply_rewards` is what
actually holds it.

### `resources::RunFeats` is a per-tick drain queue and is not saved

**`resources::RunFeats` is a per-tick drain queue and is not saved.**
`award_loot`'s `is_boss` branch pushes a species id and does nothing else;
`achievement_system` drains it **unconditionally**, earned or not, in the
same tick. Forget the clear and one boss kill re-earns forever the moment a
later rung becomes reachable — and note that a test reading `RunFeats`
after a fight sees an empty queue either way, which is why
`killing_a_boss_records_its_species` calls `award_loot` directly. Not
saving it is only sound because every authored boss trigger names a single
species: the trigger is satisfied by the kill itself, and the thing that
accumulates is `achievements::Profile`, written to disk the moment a rung
is earned. A "kill N bosses in one run" trigger needs real saved run state
and a `SAVE_FORMAT_VERSION` bump; it is not the small addition it looks
like.

### A Stack description is derived, never stored

**A Stack description is derived, never stored.** `descriptions.rs` picks
a fragment through `index`, which reduces a per-slot `fold(seed, Slot)` of
`FrameSpec::salted` — itself a continuation of `rng_seed`'s FNV fold,
already carrying world seed, entrance tile and depth — via Lemire's
`(seed as u128 * len) >> 64`, never `% len`: `%` on a two-entry pool reads
only the bit `fold`'s multiply never disturbs, which anti-correlated two
slots perfectly before the high-bit reducer replaced it. `fold` re-mixes
per `Slot` rather than reusing `salted` bare, which is what keeps a
paragraph's opener, detail and coda from all landing on the same index.
That is what makes the same door read the same way across a reload with
**no `SAVE_FORMAT_VERSION` bump and no cache**, and a different stack read
differently for free. Three things break it: reaching for `GameRng` (a
draw does not survive a reload and shifts every later roll), reaching for
`StdRng` (its sequence is not stable across a `rand` upgrade, so a
dependency bump would silently reshuffle every description in the game),
and letting a caller pass its own seed (two call sites then drift on *how*
they salt). `assets/descriptions/README.md` is the schema and the
authoring prompt. If you ever find yourself adding a cache or a save field
for description text, something has started reading run state it
shouldn't.

### A Broker's board is derived, never stored, and that is what buys all four of its properties at once

**A Broker's board is derived, never stored, and that is what buys all
four of its properties at once.** `Game::contract_board` draws from a
local `StdRng` seeded off `(world seed, zone, epoch)` with its own
`CONTRACT_BOARD_SALT`, where `epoch = tick / CONTRACT_REFRESH_CYCLES` —
`game/stack_market.rs`'s argument reached by a different route, and the
forced one: the player is shown an offer *before* accepting it, so the
answer has to survive a save and load. What follows is that the board
needs no save field, spends no `GameRng` draw (so opening a screen shifts
nobody's stream — the failure this repo has been bitten by three times),
cannot be rerolled by save-scumming, and rotates on its own.

It is also readable from **anywhere**, underground included, and that is
the same fact rather than a second one: a board seeded off the sector and
the epoch makes no claim about where the party is standing, so there is
nothing for distance to invalidate. Until 0.9.3 it was `None` underground,
which was not an oversight either — reach *was* measured from the player's
`Position`, and `Position` is pinned to the surface entrance tile down
there, so a range check made from it would have seated the party at a
Broker four frames above, the same trap `find_target_in_direction` fell
into. What retired that reading was retiring the range check; see
**Reading a Broker's board and signing it are two questions** below.
`Game::active_contracts` was always the half that read anywhere, and the
base-menu row has always been `base_only: false` precisely because the
*engine* answers rather than the frontend guessing.

### Reading a Broker's board and signing it are two questions, and one call answers both

**Reading a Broker's board and signing it are two questions, and one call
answers both.** `Game::broker_reach` returns a three-state `BrokerReach`
— `NoBroker`, `OffBase`, `AtBroker` — and it is the only thing that builds
one. `board_defs` refuses on `NoBroker` alone, so the offers are listed
wherever the player is; `accept_contract` and `deliver_to_contract` require
`AtBroker`; the base menu's row test and the screen's own header read the
same value.

Three booleans' worth of state out of one call rather than two predicates,
for `NoPost::BoxedIn`'s reason for sitting beside `NoPost::NoRoute`: five
things ask this, and two independent booleans let a screen draw a board it
will then refuse to take from. `ContractRefusal::NotAtBroker` is the
matching half on the way back out — distinct from `NotOffered` because the
two leave the player different errands, one a walk home and one a contract
that was never on offer.

What `AtBroker` measures is the **base**, through `BaseGrid::is_floor`, and
not the distance to the Broker. That is not a relaxation for its own sake:
`place_structure` refuses everything but a Home until a Home is standing
and every deployed structure stands on laid floor, so a Broker is on the
base by construction and its own tile carries no information the base's
floor does not. The old rule was `CONTRACT_BOARD_RANGE_TILES: 2` — arm's
length, which read as arbitrary from the far corner of a base the player
had built
themselves. The constant is gone rather than widened, because a base's
footprint is already derived and grows: a number here would have frozen the
desk at the radius a base *starts* at, which is
`MAX_BUILD_DISTANCE_FROM_HOME`'s standing trap.

The underground refusal is written as its own early return rather than
left to the slab check, even though the entrance tile a `Position` is
pinned to sits outside the slab by construction —
`spawn_surface_links` draws it from the ring just outside. Leaning on that
would make a contract rule depend on where links are allowed to land.

Two consequences worth knowing before touching either side. The base
menu's row test calls `broker_reach` and **not** `contract_board`, which is
what it used to call: that closure runs every frame the menu is open, and a
board that no longer refuses on distance rolls every template and samples
the habitat ring before it can answer — the proximity check used to
short-circuit all of it. And an engine fixture that stands a Broker up has
to stand a **Home** up with it: the load path derives `Platform::center`
from `Game::home_position`, so a slab stamped without one comes back from a
save as no base at all, and `template_pools` reads its species half off the
ring around the slab. That is how the first draft of this change made
`the_same_rolled_contract_comes_back_after_a_save_and_load` fail — the
fixture, not the feature.

### A starter contract jumps the board queue, and only in the first sector

**A starter contract jumps the board queue, and only in the first sector.**
`ContractDef::starter` is a `#[serde(default)]` flag, and `Game::board_defs`
partitions the eligible pool on it and fills its three slots from the
starters before it touches anything else. The reason is arithmetic rather
than taste: the board draws uniformly, a zone-1 pool is nine authored
contracts plus up to five rolled, and three slots out of fourteen make a new
run's first job a coin flip. `min_zone: 0` is not the same statement — it
says a contract *may* be offered, not that it is offered first — and it was
what the shipped set had been relying on.

Two properties fall out of how the partition is written. The second tier
draws exactly as the single loop used to, so a board with no starters left
spends the same `StdRng` draws in the same order it did before starters
existed — no seeded board moved. And the partition predicate carries
`ZoneLevel <= 1`, so past the first sector the flag stops meaning anything
at all: a starter is still offerable (nothing about killing three programs
becomes unfinishable in zone 4), it simply stops outranking. That gate was
added because the `contracts` dev template — a zone-3 world — had its board
taken over by beginner errands, which is exactly what a mid-run save would
have looked like on load. Onboarding ends at the breach, and one of the
seven shipped starters *is* that breach.

Templates carry no such field and `ContractTemplate::roll` writes
`starter: false`, so a rolled contract can never jump the queue. An arc that
this sector's habitat pools got a vote in is not an arc.

### `RunFeats` has two fields and two drainers, one each, and merging them is the change to refuse

**`RunFeats` has two fields and two drainers, one each, and merging them
is the change to refuse.** `award_loot` pushes every kill's species into
`kills` beside the existing `bosses_defeated` push — one site, so the two
records cannot drift about what counts as a kill — but
`achievement_system` drains only the second and `contract_system` only
the first. Both are registered **unchained** in the schedule on the
stated grounds that they share no mutable state; a single shared queue
would silently make that false, since whichever ran first would eat the
other's events. Neither field is saved: they are per-tick drain queues,
and what accumulates is `resources::ActiveContracts`, which is.

### `ActiveContract` stores the whole resolved `ContractDef`, not an id

**`ActiveContract` stores the whole resolved `ContractDef`, not an id.**
`EquippedItem` holding an entire `GearCopy` is the same argument:
forgetting a property must not be expressible, and a contract file edited
or deleted mid-run must not strand or silently rewrite one already
accepted. A save naming a contract whose file is gone still finishes and
still pays. The pair of `SaveData` fields cost no `SAVE_FORMAT_VERSION`
bump — additive, `#[serde(default)]`, field-named RON since v29 — and
`a_save_written_before_contracts_existed_still_loads` strips them back
out of a real save file rather than assuming it.
This also retires a documented limitation:
`assets/achievements/README.md` used to say a "kill N bosses in one run"
trigger was impossible because counting within a run needs saved run
state the game doesn't keep. That state now exists.

### Contracts deliberately amend "progression is earned by fighting", and the amendment is narrower than it looks

**Contracts deliberately amend "progression is earned by fighting", and
the amendment is narrower than it looks.** XP is a legal contract reward
on *any* objective, delivery and construction included — the intent being
that what advances the player is the thing the game asked for rather than
whatever was nearest. Anyone reading the old invariant later and
"restoring" it by gating XP behind combat objectives is undoing the
feature. What survives unchanged is the rule underneath: **Portal
Fragments are still earned only by fighting and descending.**
`Reward::PortalFragments` does not exist — absent rather than unused, so
a mod file cannot reach it — and `the_shipped_contracts_name_things_that_
exist` refuses `Reward::Item("portal_fragment", n)`, which is the same
thing through the back door. Nothing gates the XP magnitudes:
`balance_sim` is RNG-free and cannot see a contract at all, so those
numbers are opening guesses answerable only by play.

### A nemesis is marked by the one predicate that already decides a win, and no call site has to know about it

**A nemesis is marked by the one predicate that already decides a win, and
no call site has to know about it.** `Game::end_battle` already computes
the win as `battle.groups.is_empty()` — the same read the telemetry `won`
field uses, because "won" has to be read off the enemies rather than the
player: a Forgiving defeat is absorbed mid-round by
`difficulty::death_handling_system`, which reboots the player before
`end_battle` ever runs, so their HP afterwards says nothing about the
outcome. `Game::mark_nemeses` reuses that same expression, so a Forgiving
defeat and a jack-out mark a nemesis for the identical reason a won fight
marks nobody: both leave `groups` non-empty. There is deliberately no
`fled: bool` parameter threaded through and no marking at any of
`end_battle`'s four call sites — a fifth cannot forget to call it, because
none of them call it directly.

Its position inside `end_battle` is a window, not a preference: below the
`StackSpawn` stray sweep (`Without<Tamed>`, so a Stack pack that outlived
the fight despawns first) and above `BattleState`'s removal a few lines
down. Above the sweep, a Stack loss would try to mark entities already
gone; below the `BattleState` removal, `mark_nemeses`'s own read of the
battle roster (via `all_living_enemies`) would have nothing left to query.
A Stack fight therefore marks nobody as a consequence of call order, not
because of any check `mark_nemeses` itself makes.

### Rarity's multiplier now touches `Stats` at two sites, not one

**Rarity's multiplier now touches `Stats` at two sites, not one.**
`components::Rarity`'s own doc used to claim `stat_mult` was "applied
exactly once, inside `Game::spawn_wild_creature_scaled`" — true until this
feature, and left uncorrected there would have been the kind of stale
claim `CLAUDE.md`'s comment-discipline rule exists to catch.
`Game::promote_rarity` is the second site: a nemesis mark that ratchets
`Rarity` up a rung multiplies `Stats` by the **step** between the old and
new tier's `stat_mult` (`new.stat_mult() / old.stat_mult()`), never by the
new tier's absolute value. Applying the absolute value a second time would
compound the spawn roll on top of itself, the exact bug
`Game::load` and `Game::fuse_companions` already have to dodge by *never*
re-applying `stat_mult` at all — this is the one place besides the spawn
roll that is allowed to. `Rarity::ALL`'s own top is the ceiling past which
the step is `1.0` and a promotion stops touching `Stats`, though the
grudge that got a program there keeps rising regardless.

### The nemesis ladder is bounded by `Rarity::ALL` itself, so it needs no second ceiling

**The nemesis ladder is bounded by `Rarity::ALL` itself, so it needs no
second ceiling.** Escalating a nemesis by rarity rungs rather than by a
custom multiplier means the feature inherits `Rarity`'s own curve for
free: `Ordinary` → `Prismatic` compounds to roughly 2.15x, decelerating,
because each rung's `stat_mult` step shrinks as the tier climbs. That is
why nothing in `tuning.rs` caps how many times a single program can be
lost to — `CLAUDE.md`'s "every difficulty curve in the game is linear"
rule is about a curve that scales with *zone* or *depth*, and a nemesis's
own growth is neither: it is bounded, self-limiting, and orthogonal to
both. A second ceiling constant would only be needed if the ladder itself
were unbounded, and `Rarity::ALL` already isn't.

### Nothing in the nemesis feature draws from `GameRng`

**Nothing in the nemesis feature draws from `GameRng`.** Naming
(`nemesis::name_seed`), taunt selection (`nemesis::NemesisDb::taunt`) and
the promotion multiplier all derive from values that need no roll of their
own: species and `Potential` are fixed at spawn, and the grudge count is a
plain incrementing counter read live off `Nemesis` rather than anything
rolled — it is not fixed at spawn at all, it is what `mark_nemeses` has
racked up so far. The same reasoning `descriptions.rs` already established
for Stack flavour text still applies: a derived value survives a save/load
and a stored one drifts the moment the formula changes.
The sharper reason here is `end_battle` itself: it is the one path both a
real fight and a staged `arena` fight tear down through, and `arena`
exists precisely so a scenario's numbers are reproducible run to run. A
`GameRng` draw inside `mark_nemeses` would shift every later roll in every
arena scenario that ends a fight non-trivially — silently, since nothing
about a nemesis mark looks like it should touch the RNG stream. "World
generation must not draw from `resources::GameRng`" above makes the same
argument for a Stack frame's own seed; this is the battle-teardown
instance of the identical trap.

### The glyph's colour is no longer reserved for `difficulty_color` alone

**The glyph's colour is no longer reserved for `difficulty_color` alone.**
`EntityView::rarity`'s doc used to say the map draws a rare tier as a bar
rather than a recolour "because `color` is already carrying
`difficulty_color`... and the glyph can only hold one" reading — true when
written, and it is why rarity still never touches the glyph. A nemesis is
the exception, added on purpose: `difficulty_color` takes a fourth
parameter, `is_nemesis`, checked *before* `is_boss` so a program that is
both draws as a nemesis. The precedent is `is_boss`'s own always-magenta
override — this is a second non-power reading winning the same channel,
not a new kind of exception. What makes spending the con read acceptable
here and nowhere else: a nemesis is a program you have already fought, so
"can I win this fight" is the least informative thing left for its tile to
say. `crates/gui/src/render/base.rs` draws a second, independent corner
mark for the same fact, so the read survives even for a player who reads
shape before hue.

### There are four doors into the roster, and `Game::roster_parts()` is the only barrier

**There are four doors into the roster, and `Game::roster_parts()` is the
only barrier.** A program becomes a companion at
`lifecycle::grant_starting_program`, a successful capture in
`combat_rewards`, `spawning::adopt_program`, and `party::fuse_companions` —
and the fourth is the trap. The first three inserted the identical tuple;
fusion despawns both parents and calls `world.spawn` with a component list
it assembles itself, the same divergence "Destroying a tamed program has two
paths" already records one component earlier.

Nothing about `world.spawn` or `.insert` fails to compile when a component
is missing from one of four hand-written tuples, so a shared constructor is
the only barrier available. This is the pattern `work_node_parts()` sets,
and the failure is the same shape: a fused companion silently unable to run
reads as *fusion producing a bad program*, not as a missing component.

Fusion takes `roster_parts` and then overrides the one piece genuinely its
own — the child's level — rather than opting out. Test fixtures go through
it too: when the reserves landed, three of seven new tests failed on
`spawn_tamed` rather than on the feature, which is exactly what
`work_node_parts`'s entry warns about.

### `PowerReserve`'s float is private, and the clamp is the type's rather than each caller's

**`PowerReserve`'s float is private, and the clamp is the type's rather
than each caller's.** `components.rs` documented "anything writing `hunger`
or `fatigue` has to clamp to it" as an invariant held by convention across
roughly a dozen sites, each hand-rolling `.max(NEED_MIN)` or
`.min(NEED_MAX)`. One forgotten clamp and a reserve reads negative or
overfull to `battle::power_attack_multiplier` and every status bar.

Deleting the Fatigue meter left `Needs` a one-field struct, which was the
moment to convert the convention into a compiler barrier — the same move as
`Game`'s private `world` field. The API is exactly the seven operations the
call sites perform: `new` (clamping, for the load paths), `get`, `holds`,
`spend`, `restore`, `fill` for `Game::rest` which sets outright, and
`raise_to_at_least` for `difficulty.rs`'s Forgiving reboot, the one site
that raises *to* a floor. An eighth operation is a signal to re-read the
call site, not to widen the type. Tests write through `PowerReserve::new`
rather than a test-only setter, for the same reason.

`POWER_MIN`/`POWER_MAX` stay in `components.rs` rather than moving to
`tuning.rs`: they are the type's documented range, not a difficulty knob.
`ROUTINE_POWER_COST_MULTIPLIER` is the knob, and it is in `tuning.rs`.

The rename also forced a collision into the open. `views::PlayerStatus`
carried both the reserve and the `Stats::power()` strength scalar, and the
status screen printed them two lines apart — "Attack 11 Defense 6 Power 47"
above a "Power 62/100" bar. The scalar is `strength` there now. Changing its
type from `i32` to `f32` in the process caught three test sites that had
been comparing a reserve against a difficulty rating.

### `ability_unavailable` is the one gate and `spend_power` the one charge, both priced through `routine_power_cost`

**`ability_unavailable` is the one gate and `spend_power` the one charge,
both priced through `abilities::routine_power_cost`.** A routine is priced
in a cooldown *and* a Power cost: the cooldown says "not again yet", the
reserve says "not any more". Two sites reading `def.power_cost * MULTIPLIER`
independently is the drift a comment cannot prevent, and here the two are a
refusal and a charge — disagreeing means a routine the picker offers and the
run cannot pay for, or one charged more than the row quoted.

Both read the reserve off **the entity in question**, and that single
parameter is the whole of "every companion tracks their power level". A
companion's Special draws on the companion's reserve with no second code
path.

The two ends are deliberately asymmetric. `ability_unavailable` treats a
missing `PowerReserve` as **refusing**: between a companion that cannot run
because a roster door skipped `roster_parts` and one with silently unlimited
Power, the first is the failure that gets reported. `spend_power` treats one
as a **no-op**, which is what makes hostiles safe without a branch — they
hold no reserve by design, because `choose_wild_action`'s weights were
trained against today's action distribution and a Power constraint would
cost a retrain that `CLAUDE.md` already records as not cheap.

**The charge is at the `BattleAction::Special` resolution site, not in
`use_ability`.** `use_ability` is also the path `proc_wielded_routine` and
hostile runs take, and both stay free — the proc's 25% rate is that
feature's whole price. Moving the charge into `use_ability` compiles, works,
and makes `a_proc_charges_neither_the_player_nor_the_program` fail.

### `power_regen_system` needs the underground guard

**`power_regen_system` needs the underground guard**, a third entry in the
same family as `nest_aggro_tick` above. It reads the player's `Position`,
which is pinned to the surface entrance tile for the whole of a Stack run —
so a link sited inside a Recharger's radius regenerated Power four frames
down. Harmless while nothing underground spent Power; the moment routine
calls are priced in it, it means "site a base near a link and Power is free
in the Stack", which deletes the only scarcity the Stack has.

The distinction that decides it is the one `nest_aggro_tick`'s entry states:
not "does this act" but "does this claim something about where the party is",
and a regen tied to standing near a structure claims precisely that. The
test asserts both halves in one function, because the underground half alone
passes against a bare `return` at the top of the system.

### Every routine in the game was already priced; the field just reached nothing

**Every routine in the game was already priced; the field just reached
nothing.** `AbilityDef::fatigue_cost` was documented in three places as
reaching only `Phase` and `Jump`. That was true about what the *engine read*
and false about what the *assets contained*: 55 files authored a cost nothing
consumed, priced back when the field meant exactly what `power_cost` means
now, and 11 more authored one inside their `FieldBuff` effect. Folding the
two fields into one was a key rename, not an authoring pass — which is what
made a change that looked like 71 files of content work into a mechanical
flip, verified by diffing the sorted multiset of all 65 numbers rather than
by eye.

The default moved from 5.0 to **0.0**, and that is load-bearing. The old
default was the price of commanding a companion, a mechanic that stopped
charging on 2026-08-08, and it survived only because the field reached two
routines. Free-by-default is the only safe default once a field's audience
widens to every ability in the game; a mod that means to charge says so. It
is also what keeps the five uncosted shipped files behaving exactly as they
did — `priority_boost` above all, the fallback every companion has when its
species grants nothing.

### `Trickle` is the one restore kind that does not scale with its invoker

**`Trickle` is the one restore kind that does not scale with its invoker**,
and the rule that excludes it is the one `scales_with_invoker` already
stated: a value that already carries its own ceiling does not need a second
one stacked on top.

`Regen` and `Trickle` look like the same shape — both restore a pool over
time — but `Regen`'s ceiling is `max_hp`, which grows with level, so a
scaled heal stays the same fraction of the bar. Power's ceiling is
`POWER_MAX`, a fixed 100 forever. Scaled, an authored `power: 1` is 7 a turn
at `ABILITY_SCALE_LEVEL_CAP`, which pins a full reserve for the buff's whole
duration and makes the authored number untunable — the level term swamps
whatever the file says.

This surfaced retuning `trickle_charge`, which is now the only in-Stack
Power source and so the highest-leverage number in the feature. Its numbers
(80 turns at 20, to 60 turns at 25) buy back about a quarter of a reserve
per run and take 60 underground turns to collect, which is a real Trace and
encounter cost: a sustain rather than a tap.

### A field buff's lifetime is decided by its kind *and* its source, and the source half is the load-bearing one

**A field buff's lifetime is decided by its kind and its source, and
`ActiveFieldBuff::runs_until_rest` is the one predicate.** A routine-armed
buff of a read-on-demand kind has no turn count at all: it runs until the
party rests, or until a Forgiving reboot ends the expedition, or until
another routine of the same kind displaces it. Nothing else touches it, and
nothing decrements it — `remaining` stops being a lifetime for those, which
is why an in-flight buff from an old save simply stops ageing and needs no
migration.

**The `source` half is what a rule keyed on `kind` alone would have got
wrong.** `ItemEffect::prebattle_buff` arms the very same struct from a
one-shot consumable with its own authored `ticks`, and `patch_routine` — the
one shipped item with a side effect at all — arms Mitigation for 120 of
them. Worse, `field_buff_power_of` deliberately *sums* a `Consumable` and a
`Routine` entry of one kind rather than choosing between them, which is
`arm_field_buff`'s whole reason for two separate displacement rules. So a
kind-only rule would have made that item permanent and stacked its 10% under
Ablative Layer's for the rest of the expedition. A routine is repeatable and
priced in a reserve that rest refills; an item is spent. Only the routine's
half of that pair was ever meant to last the trip.

**`Regen` and `Trickle` are the exceptions, and both directions of that are
load-bearing.** They are the only kinds with a per-tick effect
(`apply_field_buff_tick`); everything else is read on demand and does nothing
as a turn passes — so an until-rest `Regen` is unbounded healing and an
until-rest `Trickle` is unbounded Power, which is the entirety of the Stack's
scarcity, freshly retuned two entries above. They are also the only two that
use `interval`, whose cadence is phased off `remaining` — a counter an
until-rest buff does not have. Excluding them is what let `tick_field_buffs`
keep its cadence filter untouched and add one early return.

`duration` therefore defaults, and `field_buff_duration_mismatch` refuses
both invalid corners at load rather than resolving either quietly: a lifetime
authored on a kind that ignores it (the modder's 90-turn shield is permanent
and nothing says so — refused, where `field_only_dead_fields` merely warns
about a dead `cooldown`, because a dead cooldown leaves a routine that still
works as written), and a counting kind with none, which armed at 0 and
expired on the turn it was run. That second corner was silently reachable
before the field defaulted.

Two smaller consequences worth knowing. The drop is a **free function on the
component**, `components::drop_until_rest_buffs`, for exactly the reason
`field_buff_power_of` is one: the two callers are `Game::rest`, a method, and
`difficulty::death_handling_system`, a plain bevy system with no `Game` to
reach through — the same split `stack::surfaced` already makes in that arm.
And in `rest` it sits **down with the heal and the refill rather than up with
the gates**, so a refusal, the mid-loop game-over bail and a swarm catching
the party mid-standby all leave the loadout where it was: a rest that never
happened clears nothing.

**The economy this creates is untested by anything.** The Power cost stops
being a budget and becomes a one-time toll, since rest refills the reserve
for free — the steady state is a party that is always fully buffed, at
roughly 66 of 100 PWR for four of the eight converted routines, and
`run_field_routine` charges the *player's* reserve even when a companion
holds the routine, so it is one pool for the lot. `balance_sim` models no
abilities (next entry), so nothing in the suite can see this. It shipped on
the reading that the displacement rule is cap enough and repricing is a
`.ron` edit if play says otherwise.

**The player-facing tag is `"rest"`, one word, and that is a width
constraint rather than a style choice.** `draw_status_buffs` measures
nothing and `draw_row` clips rows vertically but never horizontally, so the
map's status column takes whatever it is given and silently runs off the
panel. Measured at the 1440x900 geometry `ui_metrics` is calibrated for:
`"until rest"` overflows a player-borne row by 24px and `"til rest"` by 2px.
`the_widest_until_rest_buff_row_fits_the_status_column` pins it. That test
deliberately stops at the player's own rows — a companion-borne row carries a
trailing holder tag and already overran the column by ~200px before any of
this, which is a pre-existing bug recorded in `TODO.md` rather than something
this tag caused.

### `balance_sim` gates none of the Power economy

**`balance_sim` gates none of the Power economy.** It models no abilities at
all, so none of the 66 inherited costs is covered by the balance regression
suite, and neither is `ROUTINE_POWER_COST_MULTIPLIER` nor `trickle_charge`'s
retune. Its curve tests pass against a game whose entire running economy has
changed.

That is the accepted trade rather than an oversight, and it is the same
shape as "`balance_sim` has no Stack term at all" above. The suite's job
here is to prove the *mechanism* — costs are charged, the right entity pays,
an empty reserve refuses — and no number in it. The instruments that can see
the numbers are `dev-arenas/` and a session.

### The ledger is one pure function with two callers, and `runs_a_job()` is its "is this a machine" predicate

**`game::base::power::ledger` is one pure function with two callers**:
`systems::power_grid_system`, which parks the result in `resources::PowerGrid`
for the systems behind it in the chain, and `Game::base_power`, which the
base pane's grid header reads directly so the number is right on the very
first frame after a load — before any tick has run and while `PowerGrid`
still holds its `Default`. One function rather than a rule stated twice: the
tick-cache path and the read-before-any-tick path would otherwise be two
copies of "sum supply, sum draw over machines, cut in `(x, y)` order",
exactly the shape `CLAUDE.md`'s comment-discipline rule already warns is the
one that drifts.

**`StructureDef::runs_a_job()` — `self.work.is_some() || self.assembles.is_some()`
— is the ledger's "is this a machine" predicate, and the ledger is the
*fourth* caller that has to agree with it, not the first.** Three call sites
already used it to answer the same question before the grid existed:
`building.rs`'s spawn path and `lifecycle.rs`'s load path both gate whether an
entity gets a `MachineStatus` component at all, and
`Game::accepts_a_program` gates whether the cronjob menu offers a structure a
posting. `ledger` is what decides whether that same structure can go dark —
and it has to walk in step with the other three, because `PowerLedger::dark`
holds `Entity`, and the only thing that can *read* a dark verdict back off an
entity is a `MachineStatus` component. A structure that `runs_a_job()` for
the ledger's purposes but didn't get one at spawn would be counted against
the supply, possibly darkened, and have nowhere to show it — silently eating
budget on the player's behalf with no way to diagnose it from the pane. A
structure that has a `MachineStatus` but the ledger doesn't count is the
opposite fault: it never draws and can never go dark no matter how far the
base overruns, which reads as a machine the grid quietly forgot to charge.
Four sites reading the same boolean off the same method is what keeps those
two failure modes from opening up between them; a fifth "is this a machine"
check written by hand anywhere in the base code is the one to catch in
review.

### One writer and three guards, and why the power system runs first

**`idle_machine_system` is the single writer of `MachineStatus::Unpowered`,
and three systems behind it in the chain — `task_progress_system`,
`assembler_system`, and `player_gather_system` — guard on the same fact
(`resources::PowerGrid::is_dark`) without writing a status of their own.**
That is three guards, not two. The plan and the design spec both under-count
it at two, because both were written against the two-system chain the base
had before this feature — a cronjob worker (`task_progress_system`) and an
assembler (`assembler_system`). `player_gather_system`, the player
hand-working a node themselves, was not a guard either draft anticipated
needing, and it turned out to need one for the same reason the other two do:
without it, a player standing at a dark node could still call
`deliver_payout` and pull a real, paid-out unit of production out of a
machine the ledger had already declared dark — the mechanic working exactly
as before, just with the pane's status label lying about it. Worse, that
payout path *writes* `MachineStatus::Running` on its own tick, so a player
willing to stand at the node could flip a machine `idle_machine_system` had
just set to `Unpowered` back to `Running` every single tick — the very
twice-per-transition logging the design rejected a last-in-chain power
system specifically to avoid, reopened through a third door neither the plan
nor the spec had a system standing behind at the time either was written.
This was a controller ruling made during Task 3's implementation, not a
scope change signed off in either document; both should be read as
describing a system that no longer exists until they're corrected to match.

**Why the guards write nothing.** All three run *after* `idle_machine_system`
in the chain, so the tick's `Unpowered` verdict is already on the component
by the time any of them would touch it. A guard that also wrote a status —
`Running` on progress, or anything else — would have `idle_machine_system`
put `Unpowered` back on at the top of the *next* tick, and the guard flip it
right back, forever, for as long as the base stayed short. `set_machine_status`
logs on every transition, so that isn't just a wasted write: it's a message
in the base log every tick the grid is short, for however many machines are
dark.

**Why `power_grid_system` runs first, ahead of even `idle_machine_system`.**
`Game::build_schedule` chains it at the head of the base group specifically
so which machines are dark is decided *before* anything reads or writes a
`MachineStatus` that tick. A power system placed last in the chain instead
would compute this tick's cut only after the other four systems had already
acted on last tick's verdict — so a machine that just lost supply would keep
producing for one more tick under a status that no longer described it, and
the correction landing last would itself be the second write of the tick,
which is the same ping-pong the three-guard design exists to prevent, just
moved from "guards write" to "the source of truth arrives late."

### Why `Unpowered` was allowed a sixth `MachineStatus` variant where `output_stranded` was refused one

**`components::MachineStatus` gained a sixth variant, `Unpowered`, at the top
of its precedence over the five that existed before it — but `views.rs`'s
`output_stranded` field, a fact of the same rough shape ("this machine isn't
producing and the player needs to know why"), was deliberately kept *out* of
the enum rather than added as its own sixth variant when depots shipped.**
The two decisions look inconsistent until the actual test is named, and it's
stated at `views.rs:504`: a `MachineStatus` variant has to be **one
machine's own state** — a fact fully determined by looking at that structure
alone — not a fact about the base as a whole that merely happens to be
displayed on every structure it affects.

`output_stranded` fails that test. It's true (or false) for *every* machine
whose output is full while the base has nowhere to route it — one condition,
"no depot with room exists," read off the whole base and stamped onto
however many machines are backed up behind it at once. Folding it into
`MachineStatus` would have meant deciding its precedence against all five
existing variants for a fact that isn't really about any one of them; a
`Clogged` machine and a `Stranded` machine could both also be
`output_stranded`, and the enum can hold exactly one variant at a time.

`Unpowered` passes the same test only because of the ledger's `(x, y)` cut
order. The *shortfall* is base-wide — one number, `draw > supply` — but
*which machines lose the cut* is not: the ledger sorts every job-running
machine by position and walks the list once, so machine A can end up
`Unpowered` while machine B, two tiles over and otherwise identical, keeps
running because the budget ran out on A first. Whether *this* machine is
dark is therefore fully decided by looking at this machine's own draw
against its own place in the cut order — the same kind of fact `Running`,
`Starved`, `Clogged`, `Unstaffed`, `Stranded` and `Idle` already are, just
sourced from the grid rather than from a `Task` or a `Stock`. A base-wide
shortfall reads through the cut order into a fact that is, tile by tile,
each machine's own — which is exactly why `views.rs:504`'s test says yes to
a sixth `MachineStatus` variant here where it said no to `output_stranded`.

**`Unpowered`'s precedence over `Clogged` can pause a `Tend` errand that a
plain clog would not have.** `Errand::Tend` (`hauling.rs:670`) skips a
machine when it is both *not* `Clogged` and has an attached downstream
neighbour — so a backed-up machine that also feeds a neighbour keeps getting
tended regardless of any other status. Once the grid goes short and
`idle_machine_system` overwrites that machine's status with `Unpowered`,
`clogged` reads false, and an attached machine stops being tended for as
long as the base stays dark, even though its buffer is still sitting full.
This is self-correcting rather than a bug: the machine makes no progress
while dark anyway, so nothing is lost by not hauling from it, and the moment
supply returns and the machine reports `Clogged` again — one tick later —
tending resumes. It is, however, an unstated consequence of putting
`Unpowered` at the top of the precedence table: any future reader of
`Errand::Tend` who assumes "clogged" and "backed up" are synonyms will be
surprised that a dark, backed-up machine reads neither.

### A zone's material is a content decision, and two censuses are what hold it

`assets/items/cache_grain.ron` and the Cache Tap that produces it exist so a
breach changes *what* a base can make rather than only how fast. Nothing in
`ItemDef` records that Cache Grain is what the second zone pays you — it is a
plain item with a floor price, and the tier it belongs to is a fact about the
content set, not about the type.

That makes it exactly the kind of decision that gets reverted one `.ron` file
at a time. An upgrade cost trimmed back to fragments, a zone-gated blueprint
whose recipe drops its material — each reads as a small balance tweak in
isolation, and there is no compiler and no formula that notices when the last
one goes. So the rule is written as two censuses in
`crates/engine/src/tests/assets.rs`, over `ZONE_MATERIALS`:
`every_zone_gated_gear_recipe_asks_for_a_zone_material` and
`every_upgrade_path_asks_for_a_zone_material`.

**Why the upgrade half costs nothing in zone 1.** `Game::upgrade_ceiling` is
`min(def.max_tier, zone)`, and a structure deploys at tier 1 — so at zone 1
the ceiling *is* 1 and every upgrade is already refused before any material
is counted. Naming a zone-2 material in an upgrade cost therefore cannot
strand an unbreached run; it can only spend a material that exists by the
time the tier does. That coincidence is what made the upgrade ladder the
right place to put the second payoff, and it is worth re-deriving before
anyone "fixes" the ladder by making tier 2 reachable in zone 1.

**Why the gear half rides the research file rather than the item file.** Six
research nodes carry both `min_zone` and `unlocks_recipes`, so the gate and
the recipe are already one file and one edit. Putting the material on
`ItemDef::craftable` instead would have split them across two files that can
disagree, and would have hit the scavenged tier —
`scavenged_gear_stays_benchless_and_fragment_only` exists to keep a raided-
flat run able to re-equip, and a zone material in one of those recipes is
precisely what it refuses.

**Cache Grain survives a breach and Core Fragments do not.** `enter_next_zone`
wipes exactly `currency()` and `craft_currency()`; everything else in the
inventory travels. Cache Grain declares no `role`, so it crosses — which is
deliberate, since the alternative is a run that breaches into zone 3 holding
nothing the zone-2 ladder wants and cannot upgrade until it has re-tapped.
The layering property has its own test,
`core_fragments_keep_flowing_once_the_second_zone_material_arrives`: a new
tier must not retire the one below it, or every recipe still denominated in
fragments is stranded.

**Fixtures go through `stock_upgrade_materials`.** Seven existing tests went
red at once when the first material landed, none of them about upgrades
costing a new thing. The helper derives what to stock from the shipped
`UpgradeDef`s, so a second material tier joins every fixture the moment its
`.ron` file exists. It deliberately leaves the currencies alone: several
callers assert on an exact fragment delta across the upgrade, and topping
those up would quietly rewrite what they measure.

### The status column cannot grow, so a buff row's holder tag gets its own line

Two panels draw the same buff list and only one of them can widen. The battle
box calls `buff_panel_width`, measures its content and sizes itself to fit —
so an inline `(holder)` tag costs it nothing but pixels it can take. The map's
status column is a fixed slice of the window (`1 - base::PANE_W`, one inset
either side) and `draw_row` clips rows **vertically only**, so a row too wide
for it runs off the panel in silence.

Measured at the 1440x900 geometry `ui_metrics` is calibrated for: the column
holds **38.5 monospace cells**, the widest shipped routine row plus its
`rest` suffix already spends 376 of its 417 pixels, and a companion-borne row
drew **777px — 360px off the panel**. That is why no amount of clipping the
tag was going to fix this: there were **3.8 characters** of room left before
the tag started, and `" (X)"` is four.

So `TagStyle` is a parameter on `buff_entries` rather than a fact about a
buff. `Inline` keeps the tag in the row text for the panel that can widen;
`OwnLine` gives it a dimmed, indented row of its own for the panel that
cannot. One statement of what a row says, two layouts, chosen by the caller
that knows its own constraint. The alternative considered was making both
panels use the second line, which is simpler — and silently halves
`BATTLE_BUFF_ROW_CAP` from four companion buffs to two, in a panel with no
overflow problem to solve.

**The indent is four spaces, not two.** `draw_row` gives a `Row::Item` a
two-space prefix of its own and a text row none, so four is what lands the
tag two cells right of the name it belongs to.

**`cap_entries` counts entries and rows at once**, which is why it takes
`Vec<Vec<Row>>` rather than a flat list. Under `OwnLine` a companion's buff
is two rows, so a flat cap would strand a holder line's name outside the list
*and* report "+2 more" for one hidden routine. An entry is kept whole or not
at all, and odd room is left unspent rather than spent on half a routine.

**The trap to watch in the test.** A width test that iterates rows and
`continue`s on anything that is not a `Row::Item` measures nothing under
`OwnLine` — the tag is no longer on an item row — and passes against no fix
at all. `the_widest_companion_borne_buff_row_fits_the_status_column` measures
both kinds, each the way `buff_panel_width` measures it: advance for an item
row (its two-space prefix has no ink), ink for a text row. Mutating the style
back to `Inline` reproduces the original 360px overflow, which is what says
the test is holding something.

### A basic attack is an ability; the two are applied by different arithmetic on purpose

A wild program's turn has always had two branches — run a Special
(`use_ability`) or swing a basic attack — and until now they carried two
different *types*. `MoveDef` was name, power, an optional status rider and a
reach flag, which is `AbilityEffect::Damage` plus one field; the effect's own
doc comment had said so for as long as it has existed.

`species::basic_attack_ability` is now the one conversion, and combat names
`MoveDef` nowhere. `moves:` stays the authored shape, so no species file and
no mod needed editing, and `SpeciesDef::basic_attacks()` is what the four
readers take. A converted attack is `power_cost: 0`, `cooldown: 0` — a
fallback that could be on cooldown leaves a hostile with no action at all —
and `wild_weight: 0`, which keeps thirty-odd filler attacks out of the pool a
Routine Disk rolls from. Its id is `{species}.basic.{index}`, derived from
position rather than name: two species may ship an attack of the same name,
and a name is player-facing text a modder may translate.

**The two arms used to compute damage by different arithmetic, and that gap
closed on 2026-08-19.** Both now go through `battle::resolve_attack` — one
attack roll, one band roll, the attacker's flat `atk` added, the defender's
mitigation taken off as a percentage. The old table (raw `atk` against
`effective_def` for a basic attack, `scaled_hp_power` off `effective_atk`
against raw `def` for a Special) described a difference that no longer
exists, and the warning attached to it — that merging the two would scale
every enemy swing by level and affinity — was answered by the merge happening
as part of a deliberate combat-model change with its own `balance_sim`
re-baseline, rather than as a tidy-up.

**What still differs is where the band comes from**, and that difference is
load-bearing rather than residual:

| | basic attack | Special |
|---|---|---|
| band | the wielder's weapon, else the move's own (`Game::attack_range`) | the ability's own, through `abilities::scaled_range` |

A weapon **overrides** a natural attack rather than adding to it, which is
what makes which weapon you carry matter more than a flat bonus would; and it
is keyed on the weapon carrying a band, not on the slot being occupied, so a
modded weapon authoring none leaves its wielder swinging naturally rather
than silently disarmed. A Special is the ability's own damage and takes no
weapon band at all — a routine is not swung.

**`ranged` lives on `AbilityDef` but is read by one path.** Honouring it in
`use_ability` would silently stop back-row hostiles running Specials they run
today, since every authored ability defaults to `ranged: false`.

**`balance_sim` still reads `species.moves`**, the authored form, on purpose.
It is the balance gate; reading the converted list could only ever produce
the same numbers, and reading the authored one makes that obvious instead of
requiring a proof. It reads `spread` alongside `power` now
(`average_move_range`), so a species that swings wide is projected as
swinging wide rather than as swinging for its average every time.

### The ground

**`Game::ground_effect` is the one door onto what terrain does to you, and
the zone-1 gate lives inside it.**

Ambient effects are keyed to the biome and loaded from
`assets/environment/*.ron`. Three things could each have held the zone-1
rule — the loader, the reader, or the hook in `move_player` — and only one of
them survives a second consumer.

The loader is wrong because a db is not a place: `EnvironmentDb` has no idea
what zone the party is in, and giving it one would make the same install load
differently depending on when it was asked. The hook is wrong for the reason
this repo has hit four times elsewhere: a rule at a call site is a rule that
holds until the second call site appears, and the obvious second one is
already visible — a screen that names the ground you are standing on, an
examine line, a scan. Either would read the db directly, get an answer, and
show the player a hazard that zone 1 does not apply.

So the reader holds it, and the reader takes a *coordinate* rather than a
biome. Taking a biome would have been a smaller function and would have let
the caller do the map lookup it was doing anyway; it would also have meant
every caller could get the same answer without going through the gate, which
is the property the whole arrangement exists for.

**The trap is that the biome's name is deliberately on the other side of
it.** Zone 1's neutrality is about *effects* — the opening zone is where a
run learns the game, and ground that bites there is a tax on the tutorial
rather than an exception to it. None of that argues for hiding what the
ground is called, and the first player-facing name the terrain has ever had
would be a strange thing to withhold from a new player specifically.

The shape of the mistake is a later change that "tidies" the hook by wrapping
the whole block, name included, in the gate. It looks like a simplification
and it costs a run's first three zones their only sense of place.
`zone_one_takes_no_bite_but_still_names_the_ground` is what refuses it, and
it asserts both halves in one function on purpose: the effect half alone
passes against exactly the bare early return being guarded against.

Three smaller rules, each with the same one-place shape:

- **Terrain never costs Power and never raises Trace.** Both are resources
  the player spends deliberately and budgets for something else — Power for
  routines, Trace as the Stack's own pressure. Ground that drained either
  would price walking in a currency already committed, and would make the
  Stack's Power scarcity a function of the surface route taken to reach it.
- **The player alone takes environment damage.** Corrupting the party would
  route program deaths — and, under Permadeath, the run-ending path — through
  something that is not a fight, which is the one thing the game's whole
  progression spine says is where consequences come from.
- **The bite goes through `Game::apply_damage`.** That is the one code path
  that lowers a creature's HP, so mitigation, affinities and every other
  incoming-damage rule apply to terrain without a line of code. A "simpler"
  direct write to `Stats::hp` would silently make Ablative Layer stop working
  on the one damage source it most obviously should.

The load-time refusals are argued in `assets/environment/README.md`, since
they are the modding contract rather than an engine seam. The one worth
repeating here is that the base slab may not be claimed: it is the one safe
ground in the game, nothing spawns there and no ambush fires there, and a
base is stamped over whatever terrain it lands on — so ground that bit there
would make the safe floor depend on where the player happened to build.

### The manual's index is a menu and a page is a document

`?` used to draw `HELP_ROWS`, a const in the renderer, over a screen that
closed again on any key. It is now `assets/help/*.md` behind two screens,
and the split between them is the load-bearing decision.

The two answer to different idioms and a page cannot be both. Selection-
driven scrolling — `popup_layout`'s, which every list screen shares — keeps
the *selected* row visible. Put the further-reading rows at the bottom of a
menu-idiom page and it opens scrolled to the end of the prose, because the
first selectable row is down there. Put them at the top instead and long
prose is unreachable, because the highlight never leaves the head of the
list. Scroll and select are the same key on this screen, and a document
needs scroll, so links take a typed label rather than a highlight.

That is the whole reason `Mode::HelpPage` exists rather than the index
gaining a second job. The index stays an ordinary numbered menu, which is
why more than nine topics is already solved by `menu_shortcut`.

**The link rule replaces a `see_also:` field.** `[label](topic-id)` does two
things from one authoring gesture: the sentence reads as `label`, and
`topic-id` joins that page's further-reading list, deduped, in
first-appearance order. A separate field would be the cross-reference
written twice — once in the sentence that motivates it and once in a list —
and the copy that drifts is the one nobody reads. Resolution is a **second
pass** in `load_dir`, because a target cannot be checked until the whole
directory has parsed; an unresolvable target is dropped from `links` and
warned about rather than kept, since a menu row that refuses when picked is
worse than a row that was never offered. The prose still reads as written:
only the row goes.

**The wrap is engine-side** (`text::wrap`, lifted out of
`render/popup.rs::wrap_text`, which is now a call) for the reason the
read-only-screen seam gives at length: a screen's row count is owned by
app-core, so a per-row transform done in the renderer opens the screen on
rows that are not drawn. A second wrap implementation here is exactly the
copy this repo has been bitten by four times.

Markdown rather than RON, and the argument is not house style: a page is
prose with nothing to validate past "does it have a title", so RON would buy
consistency at the cost of escaping every quote in every paragraph — on
content whose whole point is that a player can edit it. Identity and
ordering come from the filename (`10-start-here.md`), so there is no front
matter and therefore no second parser; a file without the `NN-` prefix is
skipped rather than defaulted to order 0, because ordering is the whole of
what the filename is for.

Two traps. `assets/help/README.md` is a `.md` file in a directory of `.md`
content — the one asset directory whose schema reference shares an extension
with the thing it documents — so `load_dir` skips the name explicitly and in
silence. And the easter-egg census moved here with the content it guards: it
reads parsed pages rather than raw files, because that README names all
three hidden keys in the course of forbidding them.

### The panes take their origin from the caller, and each view states it once

**The panes take their origin from the caller, and each view states it
once.** `draw_playing_base` hands `draw_surface_map`, `draw_stack` and
`draw_map_inset` a `Rect` rather than a width and a height, because the
base stock strip claims a row off the top of the window and everything
below it has to start clear of that row.

That was affordable only because both views already funnelled their
geometry through a single converter. The corridor's whole projection —
faces, floors, ceilings, walls, marks, the lot — derives from
`stack::slice`, so the origin is added to one `cy` and every one of the
~700 lines of perspective drawing follows it; `column_slice` is a
one-line wrapper on top. The surface map's every world-to-pixel
conversion goes through `base::tile_origin_px`, which has three call
sites all passing identical arguments. `frame_map::inset_rect` is the
third. Threading a `top` through the drawing code cell by cell would
have been a change nothing could hold in step, and the first thing added
afterwards would have been drawn at the window's origin again.

**The trap is a literal `0.0` in either file.** A new draw call in
`render/stack.rs` or `draw_surface_map` that reaches for the window
rather than the pane draws under the strip, and there is no test that
would see it — the panes are geometry, not text, and nothing measures
where they start. A `Rect` in the signature is what makes the question
unavoidable: the pane is the only origin in scope.

Two rejected alternatives are worth recording, because both look cheaper
than they are. **Clipping instead of offsetting** — leaving the panes at
the window's origin and cutting the strip's band out with
`Painter::clipped` — costs the map's top border and puts the corridor's
centred perspective off-centre, since a symmetric projection cropped
asymmetrically no longer looks like one. **Drawing the strip along the
bottom instead**, in the band `draw_status_banner` already uses, needs no
offset at all, but stacks a second full-width strip under the first and
puts the readout in the one place a refusal message is already
competing for.

### A one-cell sprite substitutes for a glyph, and the table it comes from is the fifteenth `Painter` operation

**A sprite replaces an entity's glyph in the same cell; it never draws
beside it, and a name the table has nothing under falls back to that
glyph.** `Painter::sprite` returns `bool` for exactly that reason —
reporting the miss rather than silently drawing nothing is what lets the
one call site in `render/base.rs` keep the glyph path as an `else`.

The grid was already shaped for this before any of it was written.
`text::map_cell` returns a tile edge of `20 x zoom` and a glyph box of
`16 x zoom`, with zoom clamped to 1..4 — so a 16x16 sprite is drawn at
16, 32, 48 or 64px, whole multiples of its source. That ladder is not a
coincidence and not a convenience: `crates/gui/tests/font_rasterization.rs`
already holds unscii to it and asserts **zero antialiased pixels** at each
step, because unscii ships as vectorized outlines of a bitmap and is
pixel-crisp only if the rasterizer lands on the pixel grid. A sprite
inherits the same contract, and `crates/gui/tests/sprites.rs` is the
census that refuses one authored at any other size — such a sprite still
*draws*, it just resamples at some zoom, silently and only on screen.

**`ImageSampler::nearest()` at load is what makes that true, and it is one
line with no local symptom.** `bevy_egui` binds the `GpuImage`'s own
sampler when it renders a user texture (`src/render/systems.rs:211` and
`:250`), and Bevy's default is linear — so the integer ladder buys nothing
without it, and dropping it produces blurred art rather than an error.

**The sprite table is refcounted, not borrowed.** `SpriteTable` reaches
`Painter` as an `Arc`, so `Painter` keeps its freedom from lifetime
parameters and `render/`'s several hundred `&Painter` signatures stay
exactly as they are; the cost is one atomic bump a frame. A borrow was the
obvious first shape and was rejected on that ripple alone. `clipped`
rebuilds a `Painter` by hand and must carry the table through — it is the
one place a missing field compiles fine and shows up as sprites vanishing
inside the Stack corridor's clip and nowhere else.

**Two conventions differ from `map`, and both look like the same thing
until they are wrong.** `map` takes a *baseline* and is centred by the
caller against measured **ink** extents (`TextDims` is ink, not advance —
see `Painter::measure`'s own note); `sprite` takes a **top-left** and
fills its square exactly, because a square sprite has neither side bearing
nor descender to measure. Reading the two as one convention is a half-cell
offset that reads as a camera fault rather than a drawing one. And
`color` is a **tint**, which egui *multiplies* — so art authored near-white
inherits `difficulty_color`, the boss and nemesis overrides, `biome_tint`
and the damage dimming for free, with no second mechanism and no change to
any of them. Art that carries its own hue fights all four: a red sprite
under a green con colour goes black. Shade with value, never hue.

**The trap is overdraw, and it is invisible against the placeholder.**
Painting the sprite *over* a glyph that is still there looks pixel-perfect
while the art is opaque white, and breaks the moment a sprite has any
transparency — at which point an `@` shows through it. So
`the_player_sprite_stands_in_for_the_at_sign` asserts both halves at once:
one textured mesh painted **and** no `@` among the glyphs. The sprite half
alone passes against the bug.

**Optional by construction, and the property is held at every end.** A
missing directory, a missing file, a file that fails to decode, and a name
nothing has been authored for all converge on the same outcome: the name is
absent from the table, `sprite` reports it drew nothing, the glyph draws.
Deleting `assets/sprites/` therefore restores the glyph map exactly, the
same supported way deleting `assets/environment/` restores the pre-effects
game — and it is what will let a modded species ship without art rather
than ship invisible. Never gate the draw or the loader on the directory
being non-empty: that makes the property hold by accident at one site and
lapse at another.

**Registration waits for `LoadState::Loaded`.** `EguiUserTextures::add_image`
mints a `TextureId` eagerly, before the pixels exist, so registering at load
time would put a name in the table that paints an *unbacked quad* for the
first frames of a run. Gating on the load state means the table only ever
holds sprites that can actually be drawn, and the glyph covers the gap at no
cost — which is only affordable because the fallback above is free.

**The second-path-resolver trap came with this and is worth its own line.**
Bevy's `AssetPlugin` resolves `assets/` itself when left alone — against
`CARGO_MANIFEST_DIR` in a dev build, the executable's directory once
installed. That is precisely the trap this file records under
`crates/launcher/src/paths.rs`: it works on the build machine, works
nowhere else, and nothing fails to compile. `gui::asset_plugin` feeds it
the path `paths::resolve()` already produced, taken off the `App` that is
already carrying it rather than passed as a second parameter, so the two
cannot disagree. An absolute `file_path` survives bevy's
`get_base_path().join()` because an absolute join replaces the base — which
is what makes this override the guess rather than sit under it.

#### What this cost, and what the earlier costing got right

A sprite tileset was costed on 2026-07-27 and parked on 2026-08-06 in
favour of procedural vector tiles, on the grounds that **the blocker is art
rather than code** — 112 tiles at 16 edge-and-corner variants per biome
across seven biomes, 329 for full blob sets. That reasoning stands and is
untouched here, because it is a claim about **terrain**. Autotiling is what
sets that bill, and entities do not tile: 17 species plus 26 structures plus
the player and a handful of fixtures is roughly fifty independent 16x16
images with no seamless edges to get right. The two questions were being
answered as one, and separating them is what made this small.

The 2026-08-06 note also called **texture lifetime** the hard part, needing
a Bevy resource threaded into `Painter::for_frame` and estimating the draw
call itself at "roughly 30 lines" of hand-built `egui::Mesh` with UVs. Both
have since dissolved: `bevy_egui` 0.41 ships `EguiUserTextures` as a
`Resource` whose `add_image`/`image_id` hand back an `egui::TextureId`, so
the asset server owns the pixels and `Painter` only ever holds cheap
per-frame data — and `egui::Shape::image` builds the textured mesh already,
making the operation about ten lines. **Do not re-derive the old estimate
from that note**; it was accurate when written.

Note that there is no `Shape::Image` variant in epaint 0.35 — `Shape::image`
produces a `Shape::Mesh` carrying a `texture_id` and a UV'd quad. The test
helper `painted_images` reads meshes back and skips those with
`TextureId::default()`, which is what egui's own primitives carry; a helper
matching on a variant name here compiles against nothing.

**What is deliberately not proven.** Nothing here demonstrates that the
texture reaches the GPU: Bevy's PNG loader is registered by `bevy_render`,
so an end-to-end load test needs an adapter and a window, which the suite
has neither of. What the tests cover is the whole path up to that point —
the asset root, the file's shape, the loader's paths, the operation's
geometry and tint, and the substitution at the call site. Whether the art
*reads* at zoom 1 is a judgement about the art and was always going to be.

### A base stock tag is derived, unique, and two letters

**A base stock tag is derived, unique, and two letters.** `ItemDef::tag`
takes the initials of the name's words — `Core Fragment` becomes `CF` —
so a modded item is listed on the strip without its author adding a
field, the same bargain `ItemDef::category` strikes. `abbrev` overrides
it and exists for exactly one reason: across the 23 shipped materials
and currencies the derivation collides once, `Research Data` against
`Routine Disk`, and the strip carries the tag and the quantity and
nothing else — so a shared tag is a readout that lies about which pile
is filling, and it fails with both rows drawn and both looking right.
`Research Data` takes the override and settles for `R`: `RD` reads as
the disk, a banked currency reads fine on its initial alone, and a tag
**shorter** than two characters is as legal as the derivation's own
one-letter fallback.

`no_two_shipped_stock_items_share_a_tag` is the census, and it is a
census rather than a refusal inside `load_dir` because a collision is a
content accident, not a malformed file: a mod's own is its author's to
settle, and nothing here refuses their item. It walks `Material` and
`Currency` alone — what `Game::base_stock` lists — and skips etched
disks through `ItemId::etched_ability`. Every disk derives the same
family tag by construction, and 66 of them could not have distinct
two-letter tags; none can reach a `Stock` in the first place, so the
exclusion costs nothing.

The strip's width is **measured, never estimated from a character
count.** The UI font is proportional, and the status column seam is the
warning: a row wider than its panel is not clipped, it is drawn off the
end in silence. `stock::fits` measures each candidate line including the
`+N` tail the piles left over will need, so what does not fit is counted
rather than dropped.

`Game::base_stock` reads the same buffers `work_orders::base_holding`
sums, through `stock::output_buffers` — one statement of which buffers
are the base's, or the strip becomes a second opinion about the base
rather than a readout of it.

**Plus every `ItemDef::banked` pool, which is the one holding those
buffers can never report.** `deliver_payout` sends a banked item straight
past its own node's `output` into the player's bank — that is the whole
of what the flag buys, and it is why a Research Node has no "full" state.
So the base's only banked product had no row on the strip at all, and
`research_data.ron` had been carrying an `abbrev` of `R` for the strip's
benefit the entire time it could not be drawn. It is folded in **by the
flag and never by name**, and `output_buffers` is deliberately *not*
widened to reach it: a work order for a banked item is refused on the
grounds that no shelf holds it, and that refusal is correct.

**A row exists if the base holds any of it *or* is set up to make it.**
The strip is one row wide and it is read at a glance, which is the same
constraint that forbids sorting by quantity — and a tag that only exists
while its buffer is non-empty breaks it the same way, by reshuffling
every time a hauler clears a shelf. `stock::producible` seeds a zero for
each deployed structure's `work.produces` **and** its `assembles.item`;
both halves, because an assembler declares no `work` block at all, so a
rule reading `produces` alone leaves every crafting machine in the base
off the strip until its first unit lands.

Two narrowings hold that rule up, and both were the alternative it was
picked over. It is not **any structure**: a Depot makes nothing, and
seeding off what a building could *hold* puts a row on a one-row readout
for every item in the game. And it is not the **researched recipe list**:
a bench recipe is compiled into the player's own pack and never into a
base buffer, so a row for one would be a zero that could never move. On
the shipped tree that second one is invisible — all six researched
recipes name equipment, which `ItemDef::category` already filters off the
strip — which is exactly why it is written down here rather than left to
be rediscovered by the first mod that researches a material.

The banked pool takes the same rule, applied to the one item that has no
buffer to stand in for it: a pool the player has none of is **not**
seeded, or every run would open on a row for a resource nothing in the
base makes yet. `a_base_holding_nothing_lists_nothing` is what says so,
and it fails against a fold that seeds unconditionally.

It is ordered by item id and not by
quantity: a strip that re-sorted as buffers filled and drained would
move every tag under the eye of the player reading it, which is the one
thing a glanceable row cannot do. And it makes no claim about where the
party is standing, so like a Broker's board it reads the same four
frames down the Stack and needs no `require_surface`.

### `Game::remember` is the one door a memory is written through

**`Game::remember` is the one door a memory is written through.** It is to
`components::Memories` what `Game::apply_damage` is to `Stats::hp`: a rule
that must see *every* memory has one place to go. Nothing else in the engine
pushes a `Memory`, and the four triggers — `note_maul` off
`resolve_and_apply_attack`, `form_victory_memories` off `end_battle` (twice:
a bond and, if the fight was uphill at the bell, the fight itself), and
`note_strandings` off `tick_inner` — are all callers of it rather than
writers beside it.

**A `who` with no `Memories` is a no-op**, the same deliberate asymmetry
`Game::spend_power` makes for a missing `PowerReserve`. The store is minted
at `Game::roster_parts` and nowhere else, so its absence *is* "not on the
roster" — which keeps hostiles, structures and the player safe here without
a branch at any of the four call sites. That is also why the player is
neither a holder of a bond nor a subject of one: it falls out of `ProgramId`
never being minted for them, not out of a `Player` check anybody could
forget.

The refusals are **returned**, not logged and not asserted. The spec asked
for a subject-kind mismatch to warn, and the engine has no runtime warning
channel — `load_dir` warnings are startup `String`s and the message log is
player-facing text this feature is forbidden to write. Four observable
outcomes (`Written`, `NoStore`, `UnknownDef`, `WrongSubject`), one per no-op,
is what makes the no-op rule testable without a `debug_assert!` sitting in
the middle of the test that asserts it. It is deliberately not `#[must_use]`:
a trigger firing on a body that may or may not be on the roster is the normal
case, and `NoStore` is the answer it is entitled to ignore.

**It draws no RNG at all** — no `GameRng`, no local `StdRng`. That is what
keeps every seeded test and every `dev-arenas/` report where they are, and it
is why none of the RNG-stream-shift diagnostics apply to anything this
feature breaks. **And it writes no log line.** The screen is the surface. A
line every time a machine strands a body would flood the map's log pane and
drag the fold, filter and reveal seams into a feature that does not need
them; announcing memories is a `MessageKind`/`MessageSource` decision to make
deliberately rather than to acquire by default.

The order inside it is load-bearing: the def resolves **before** the store is
touched, so an install with `assets/memories/` deleted is *inert* rather than
merely quiet.

### A memory's intensity is derived from the clock, never stored or ticked

**A memory's intensity is derived from the clock, never stored or ticked.**
`components::Memory` carries what happened — the def, the subject, the tick
it last landed on, how many times — and not what it is worth.
`Memory::intensity` is `valence * min(strikes, strike_cap) * 2^-(elapsed /
(half_life * MEMORY_HALF_LIFE_MULTIPLIER))`, evaluated on every read.

This is `Platform`'s radius, a program's role and a Broker's board again:
nothing ticks, nothing oscillates, reinforcement is a single field write, and
a stored weight cannot drift out of step with the clock the way a per-tick
decrement can. It also means a save carries no derived number that a retune
would invalidate — changing a `.ron` half-life retunes memories already
formed, which is the behaviour a content edit should have.

The decay is a **magnitude scale and never a sign flip**, because `morale` is
a signed sum over the figure: a grudge that decayed into a fondness would
read as a program cheering up because it was hurt a while ago. `elapsed` is
`saturating_sub`, since a hand-edited save can hold a memory reinforced later
than `now` and an underflow there is a panic in release arithmetic rather
than a wrong number.

`MEMORY_HALF_LIFE_MULTIPLIER` reaches the formula through
`intensity_with`, which takes the dial as a **parameter** for `walk_field`'s
reason: at its shipped neutral value of 1.0 a test cannot tell a formula that
honours the dial from one that ignores it, so the only way to *prove* it
reaches the denominator is to vary it.

**Eviction is lazy and lives in one place** — the tail of `remember`, so
nothing sweeps. It drops what has faded and then the weakest while the store
is over `MEMORY_CAP_PER_PROGRAM`, by **magnitude at both**: a signed
comparison evicts every grudge and keeps every fondness, which is not a
memory system, since the deepest scar a program carries is the smallest
number in its store. An entry naming a def no file defines is **kept** by the
threshold sweep — restoring a removed mod file restores the memories that
named it — but cannot be scored, so when the cap forces a choice it goes
first: a memory the game cannot weigh must not hold a slot against one it
can.

### `MemorySubject::BaseTile` names the space, because two spaces share one pair of integers

**`MemorySubject::BaseTile` names the space in the type, and that is the
whole reason it is not called `Place`.** Base space and the zone surface are
the same two integers meaning different things, and reading one as the other
is what put the base's roster on the open grid once already
(`stands_in_base_space`, under **The base**). Naming the space in the variant
is what stops that recurring in a subject payload nobody would think to
check.

It matters at both ends. `note_strandings` writes the **worker's own
`Position`**, which for a posted program is base space — not its post, since
a stranded body is stranded precisely because it is *not* at its post, "left
stranded here" is a claim about where it is standing, and a memory keyed to
the machine's tile could never be read by the drift hook it exists for
(`drift_idle_staff` already refuses a tile a `Structure` stands on). And the
row renders it as base coordinates rather than as a map location.

A *surface* variant, when content asks for one, is **zone-local** and has to
be wiped by name in `Game::enter_next_zone` alongside `StackMemory`,
`BuybackLedger` and `PopulatedChunks`. A base tile needs no such wipe,
because the base travels with the party across a breach.

Serde lives on `MemorySubject` directly rather than on a `save::` mirror of
it, which is the opposite call from `save::CronjobKind`. A mirror would be a
second copy of a six-variant enum that a new variant must be added to twice
with nothing failing to compile if it isn't — and the whole point of
`kind()`'s exhaustive match is that a new variant *must* fail to compile. The
on-disk form is field-named RON, so variants encode by name and reordering
them is not a save-format change, unlike `perks::Perk`, which bincode encodes
positionally.

### The memories page is one derivation and has no scroll

**The memories page is one derivation and has no scroll.** `R` from the
roster opens `Mode::CompanionMemories`, and every figure on it comes out of
`Game::memory_report` and `Game::morale` — `Game::gear_detail`'s rule. A
renderer that weighed a memory itself would be the fourth screen in this repo
to keep a private copy of a formula the engine already owns; the subject of a
row it could not render at all, since a species needs `SpeciesDb`, a
structure `StructureDb`, and a destroyed program the name the record captured
when it was written.

**`R` and not the `M` the spec asked for.** `M` on the roster has opened the
manifest since well before memories existed. Uppercase either way, for the
reason `W`/`N`/`E`/`P`/`M` all are: an uppercase key reaches app-core as a
distinct key and so can never collide with `menu_shortcut`'s
digits-then-lowercase scheme however large the roster grows. The pair
`(Companion, CompanionMemories)` is in `input.rs`'s `keeps_highlight` table
beside the manifest's, so Esc comes back to the row the player was reading
down the list — the page indexes nothing with the highlight, so there is
nothing for it to reset to.

**The report evicts nothing.** It is `&self` and it skips a faded entry
rather than dropping it: a read-only screen that rewrote the roster it is
drawing would make what a program remembers depend on whether anybody looked.
An entry whose def no file defines contributes no row, `memory_sum`'s rule,
and that is where the empty-database property comes from at this end — with
`assets/memories/` deleted the store is intact and the page says nothing has
happened yet.

Rows are ordered by **magnitude**, `evict`'s rule mirrored rather than
described. `sort_by` and not `sort_unstable_by` so a tie keeps insertion
order — and there is deliberately **no test for that**, because none can
exist: measured, the unstable sort returns equal keys in insertion order too
at every length a store can reach, the cap being 12 and the unstable sort
running insertion sort under 20. A test that cannot tell the two calls apart
is coverage-shaped, so the guarantee is taken from the standard library's
contract and stated in the doc comment instead.

**Age reaches a row in words, not in ticks.** Nothing in any screen or any
log line has ever shown the player a tick, so a count here would be the
first, and a number the player has no scale for is not an answer. It is
banded against the **def's own half-life** — the only yardstick that makes
two memories comparable, since 6,000 ticks is fresh for a mauling and ancient
for a bad shift. Four bands and no more: past two half-lives a memory is
under a quarter of what it was and close to what `evict` drops, so everything
out there is simply long ago.

**The blurb is said once per kind.** A store holds several entries of one def
— three corners of the base that strand a worker, four species that have
nearly ended it — and printing one sentence of flavour verbatim four times
down a page is worse than not printing it. Rows arrive strongest first, so
the copy that keeps it is the one that characterises the program.

**Two censuses, one per axis, because the page has no scroll.**
`draw_popup` pages a `Row::Item` span and this page has none, so a row past
the bottom is dropped in silence — the trap
`the_tallest_gear_page_fits_its_popup` exists to catch, and
`the_tallest_memory_page_fits_its_popup` is its mirror. Nothing clamps a row
*horizontally* at all, which is `no_memory_row_overflows_its_popup`'s axis
and the one where the lost tail is the strength and the age, the two figures
the row is read for. Both build the worst case from the **real catalogue**,
with every def name made distinct so the blurb is never deduped away —
deduped, the width census would measure eleven rows carrying no blurb and
pass against a page that overflows. `MEMORY_CAP_PER_PROGRAM` is therefore a
layout constraint before it is a feel one: raising it past what fits means
giving the page a scroll first, not editing the number. Caught in the
building — at 12 entries over two rows each the page ran 29 rows into a
23-row popup, which is why the blurb is on the row rather than under it.

### The first hook is `drift_idle_staff`, and it is a rejection rather than a score

`drift_idle_staff` already declines a candidate tile on four grounds — one a
`Structure` stands on, one that is not laid floor, the party's own cell, and
one another idle body holds — and leaves the body exactly where it was
standing when it declines. The memory hook is a fifth rejection of that
shape:

```
opinion_of(worker, BaseTile { x, y }) < MEMORY_AVOIDANCE_THRESHOLD
```

So it opens no new failure mode and needs no fallback. A rejected candidate
costs the program one beat of standing still, which is already that
function's documented behaviour, and the next beat of `IDLE_STAFF_STEP_TICKS`
offers a different neighbour.

**It is `drift_idle_staff` and not `schedule_base_labour`, deliberately.** The
scheduler is documented as deciding the whole assignment by priority and then
diffing it, with no sort and no score anywhere in it. A memory term there
*is* a score, and it would also put a memory in the path of the anti-thrash
rule and the never-free-a-`Carrying`-holder rule — two rules whose whole
value is that they are unconditional. "Doesn't want to work" belongs there
eventually, as content, once the interaction can be designed rather than
bolted onto a diff.

**`opinion_of` and not `morale`.** The question is what the program holds
against *one corner of the base*; the sum over everything would keep a
program that has had a bad run off every tile at once, which reads as the
drift being broken rather than as a grudge. The comparison is signed
rather than a magnitude, so a tile a program feels nothing about and a tile
it feels well about both pass — a fondness must never be able to trigger an
avoidance.

**The loop closes on itself, which is what makes the hook more than
decoration.** A body is standing in a corner of the base, posted to a
machine, finds no route, and `haul_step_system` marks it `Stranded` where it
is standing. `note_strandings` writes that tile, and the next time the
program drifts back into that corner the tile is refused. That is the whole
mechanism: the corner it was left standing in is the corner it will not take.

**`MEMORY_AVOIDANCE_THRESHOLD` is not pinned to `stranded_at`'s valence**,
even though that def is currently the only thing that can trip it. The hook
asks whether a program holds anything against a tile, not whether one
particular memory is in its store, so a second negative `BaseTile` def has to
reach it without editing the constant. At the shipped def — valence -6.0,
half-life 3000 — one stranding keeps a program off that tile for exactly one
half-life.

**The test that nearly shipped hollow, and how the drift changed its
shape.** `a_faded_grudge_stops_keeping_a_program_away` advances the clock to
fade the memory and then asserts the body takes the tile anyway. Under the
ring, advancing the clock also **moved the ring**, so the first version was
measuring a tile the grudge was never about — a second copy of
`a_grudge_against_another_tile_does_not_move_a_program`, green against a
threshold replaced by a bare `< 0.0`. The ring answered that by advancing a
whole number of ring periods. A drift has no period to advance by, so the
tile is chosen at the **far end** of the fade and the grudge implanted
against it before the clock moves — and the clock is *wound* rather than
ticked, so the body does not move out from under the answer and
`wander_step` is asked from one tile at both ends. The precondition is
asserted out loud either way. It was the mutation pass that found this
originally, not the suite, and the mutation still fails the test.

**The whole fixture stands its bodies clear of the Home and well inside the
starting pocket**, which is a second way this family can hollow out. Parked
where `spawn_tamed` drops one, half the tiles a drift offers fall outside
the pocket and are refused by the *floor* rule — so the body holds still,
the assertion passes, and the grudge was never consulted. Eight floor
neighbours and no structure among them is what leaves the memory as the only
thing that can refuse.

### The second hook is morale, and it is one capped addend in one formula

`Game::morale` shipped with no reader outside the screen that printed it. The
memories page headed itself with a figure that changed nothing, which made the
whole page a readout of a simulation nobody was running. This is the reader:
`CycleModifiers::morale`, into `systems::mining_success_chance`.

**Why extraction and not something broader.** The base has three work paths
and no single dial reaches all three — extractors price a cycle through
`resolve_gather_cycle`, assemblers through `Task::required`, diggers through
`swing_damage`. `work_ticks_for` was the tempting one because it already
branches on `work` versus `assembles` and so reaches two of the three in one
place, but it writes `Task::required` **once, when the job is assigned**, so
the figure freezes at post time. Morale is a slow quantity and a posting can
outlast several half-lives, so that version would have been stale exactly
where it mattered. `CycleModifiers` was the other candidate and won on being
live per cycle and on being the struct's stated job: "what the *worker* brings
to a gather cycle, as opposed to what the node is."

**Signed around a baseline of zero**, which is `base_int`'s idiom in the very
same expression and `base_speed`'s in `work_ticks_at_speed`. That is not
tidiness — it is what buys three properties at once, none of them by a branch:

- a program with no memories sums nothing and contributes `0.0`;
- the player contributes `0.0` because the player has no `Memories` at all,
  the store being minted at `roster_parts` and nowhere else;
- a deleted `assets/memories/` contributes `0.0` because `sum_intensity`
  skips every entry whose def no file defines.

That third one is the empty-catalogue property holding at a site nobody wrote
a line of code for. It is worth noticing how cheap that was: the property was
established once, at the fold, and every later reader inherits it.

**The term needs its own cap, and the outer clamp is not it.**
`mining_success_chance` already ends in `clamp(0.0, 1.0)`, but that clamp
exists because `GameRng::random_bool` panics outside the range — a different
job. `morale` is a signed sum of up to `MEMORY_CAP_PER_PROGRAM` entries and
is unbounded at both ends, so without `MEMORY_MORALE_MAX_SHIFT` a bad run
drives a node to never yielding and the base stops producing, which reads as
the base being broken rather than as a program being unhappy. The subtle part
is that the outer clamp would **hide** the missing cap: a test reading the
finished chance cannot tell a working cap from an overshoot the clamp
swallowed at the low end. `morale_shift` is split out so the cap is reachable
directly, and that is the whole reason it is a function rather than an inline
expression.

Never scaled by level, zone or depth, for `effective_mitigation`'s reason: a
term that grows with the player approaches its cap and stops meaning anything.

**`morale` here and `opinion_of` at the drift hook**, which is the mirror of
that seam's choice and deliberate at both ends. Parking asks about one corner
of the base, so the sum over everything would keep a program that has had a
bad run off *every* tile at once. This asks about the body, so the restriction
would make it a per-machine preference — and the moment anything acts on a
per-machine preference it wants to be in the posting decision, which is
`schedule_base_labour` and the seam this feature is under orders to stay out
of.

**Nothing gates it numerically.** `balance_sim` models no base production at
all — no `node_payout`, no `resolve_gather_cycle`, no cycle length — so the
balance regression suite is blind to this in both directions. The evidence
that the economy has not moved is the morale-zero sweep and the cap, not a
curve. The ten pre-existing `mining_success_chance` tests, passed `0.0` and
staying green, are most of it; a new sweep states it as a property rather
than leaving it implicit across ten call sites.

### A work memory is either an edge or a stretch, and the period is what a stretch has instead

`Game::note_postings` runs from `tick_inner` immediately after the schedule,
beside `note_strandings` and for that call's stated reason — the base systems'
commands have just flushed and the clock has not moved on. What it does
differently is fire on a period rather than on an edge, and that difference is
the whole design.

`note_strandings` can be edge-triggered because `Stranded::since` gives it an
edge to read. A posting gives none: nothing distinguishes the first tick at a
machine from the thousandth. Three shapes were available and two are wrong.

**A per-tick write** is wrong, and `note_strandings`' own doc comment had
already written the argument down before this feature existed — it "would
saturate `strike_cap` in three ticks and hold the grudge at full intensity for
as long as the route stayed broken, which makes `strikes` mean nothing".
There is a second cost it does not name: `remember` evicts at the tail of
every write, so a per-tick writer makes eviction effectively **eager** for any
program holding a posting and lazy for every idle one. What a program
remembers would then depend on whether it happened to be working, which
nothing in the design wants.

**Firing on a completed cycle** is wrong for a related reason. It makes
`strikes` a cycle count, which saturates in seconds at a fast machine and
never at a slow one — so the same length of service would mean different
things at different machines, and the memory claims to be about service.

**A period** measures time served. `MEMORY_POSTING_PERIOD` is derived off
`GameClock`, so there is no counter, no field on `Task`, and nothing to save —
`Platform`'s radius, a program's role, a Broker's board and a Stack
description all follow the same instinct.

A first draft of this had the two bevy-side triggers pushing onto a
`RunFeats`-style per-tick drain queue, because `task_progress_system` and
`set_machine_status` have no `Game` to call `remember` on. The post-schedule
pass makes the queue unnecessary, and not building it also sidesteps a known
trap: registering a new `Resource` shifts bevy's query iteration order and has
surfaced latent unsorted-query failures in this repo before.

**`swept_here` is the one edge**, in `damage_structure`, because a sweep *is*
an event. The trap there was that the function already collected the workers
posted at its target — but only on the destroyed branch, to clear their
cronjobs — so the surviving branch had never looked at who was standing there
at all. A trigger written only where the query already was would have covered
half the cases and passed a test written on the same half. Both the kind
lookup and the worker query are hoisted above the branch, since the destroyed
side despawns the structure the kind is read off.

**A `Structure` memory names the kind, not the entity.** That is what makes it
sound to form one on the branch that is about to despawn the machine, and it
is the right fiction for a base whose structures are demolished and
re-deployed: a rebuilt Lathe is the same Lathe to a program that was hurt at
one. `settled_in` and `jammed_here` share that subject and oppose in sign, so
a machine kind that mostly runs nets out to a mild fondness over a run and one
that spends its life backed up nets out to a grudge.

Two exclusions are structural rather than checked, which is the point of both.
A digger gets the `Activity` memory and no `Structure` one because a `DigSite`
is the one `Task` target that is not a structure, so that arm has nothing to
read. And the player needs no exclusion despite being able to hold a `Task`,
because `remember` is a no-op on a body with no `Memories` — the same
asymmetry that keeps hostiles and structures safe at the four triggers that
came before.

**`MEMORY_TRIGGERS` in `tests/assets.rs` caught the first of these before the
suite did.** A def that ships with nothing writing it fails the build, and the
table is spelled out rather than derived because there is nothing to derive it
from: the catalogue is data, the triggers are Rust, and `MemoryDef` carries no
`trigger` field on purpose.

### There is one place a runtime path is decided

**There is one place a runtime path is decided,
`crates/launcher/src/paths.rs`**, and `main` reads nothing else. It answers
three questions and no more: where the loose asset tree is, where player
data goes, and whether this build has a repo behind it.

Before this, every runtime path derived from `dev_template::repo_root()`,
which is `env!("CARGO_MANIFEST_DIR")` — the absolute path of the machine
that compiled the binary. That is why `README.md` told players the clone had
to stay put, and why the game had no distributable build on *any* platform.
Windows is what makes it unavoidable rather than merely untidy: nothing else
in the tree is Linux-only. Measured 2026-08-19, there is exactly one
`cfg(target_os)` in the whole codebase, the dependency graph resolves for
`x86_64-pc-windows-msvc` and `aarch64-apple-darwin` without a `-sys` crate
needing pkg-config, and Linux is the fussy target rather than the portable
one. The code was already portable; the *distribution model* was not.

**The trap is a second site resolving a path against `CARGO_MANIFEST_DIR`
because it is convenient in a dev build.** Such a site works on the build
machine, works nowhere else, and nothing about it fails to compile — so the
failure is invisible until a stranger unzips the game. There is no compiler
barrier here the way `Game`'s private `world` field is one; what holds this
is that `main` has a `Paths` in scope and no reason to reach past it.

The four dev bins keep `repo_root()` on purpose, and it still lives in this
module so the dependency runs `dev_template -> paths` and never the other
way — `data_dir`'s no-`HOME` fallback would otherwise be a cycle. A tool
that is only ever run out of a checkout should find its material in the
checkout. So "one place" is a claim about the *game's* paths, not about
every path in the workspace.

**Installed-ness is sniffed, not flagged.** `resolve()` asks whether an
`assets/` directory sits beside `current_exe()`, then whether one sits at
`../Resources/assets` (a macOS bundle), and falls back to the repo. A
shipped build cannot run without its assets, so the probe tests something
required to be true. A `--features bundled` flag was rejected: forgetting it
produces a zip that works only on the build machine, and with verification
manual by choice, a footgun only a stranger can trip is the worst available
shape. A `build.rs` copying `assets/` into `target/debug/` was rejected
outright — it duplicates ~200 files per build and puts a stale copy between
the developer and an asset edit, which is hostile in a game whose content
lives in those files.

**Player data goes to the OS data directory in every layout, a repo build
included.** Writing beside the executable was rejected because a build
unzipped under `Program Files` cannot write there, and the failure mode is a
game that appears to save and silently doesn't. A split rule — data
directory when installed, repo when developing — was rejected as two code
paths where one will do, and because it would mean a dev build cannot
reproduce a player's report about where their saves went. The cost of
uniformity is that `cargo run` and a shipped build share one save directory,
which for a single-developer game is a feature.

`dev` is `Some` **iff** the repo layout was chosen: the two are one decision,
not two. An installed build hands `App::new` dev paths under the data
directory that will never exist, and that is not a special case — the arena
and template rows are gated behind `FERAL_DEV_ARENA`, and both
`dev_template::list` and the arena catalog already read a missing directory
as nothing to offer.

**`resolve()` is infallible, and that is a constraint rather than a
preference.** The spec sketched a `Result`, but `dev_template::working_copy`
must also learn the saves directory, and it is called from
`dev_template::resolve`, which app-core holds as a bare `fn` pointer in
`DevTemplates`. A fallible lookup would ripple through that signature and
force an app-core change, which is exactly what this being a launcher-only
change buys. So `data_dir()` falls back to `repo_root()` with no `HOME` and
`resolve()` falls back to the repo layout when `current_exe()` fails.
Whether the resolved assets directory *exists* is then `main`'s check, which
is where that error belonged anyway.

**The migration is a move, not a copy, and one-shot without a marker file.**
`migrate_from_repo` does nothing at all when the data directory already
holds a `.bin` — that is what stops a second run eating a newer save, and it
needs no state of its own to be true. It subsumes the inline legacy
`save.bin` migration that used to sit in `main`, which was deleted rather
than left beside it: two things moving the same file is worse than either.
`profile.ron` moves too, though the spec's decisions section named only
saves — it carries the achievement ladder's earned rewards, and leaving it
behind silently resets a player's profile. Each move is
rename-then-copy-and-delete, since a cross-device `rename` fails with
`EXDEV` and the two directories can easily sit on different filesystems, and
every failure is swallowed: a game that refuses to start because a file
could not be moved is worse than one that starts with its saves in the old
place.

**`FERAL_ASSETS_DIR` is the one override, and a second per-path override is
refused.** A modder can point a build at an alternative asset tree without
disturbing the install, and it is the natural switch for testing a shipped
build against repo assets. A matrix of per-path overrides is how "one module
decides every path" stops being true. An empty value reads as unset,
matching `dev_console::dev_flag`'s rule for the `FERAL_DEV_*` flags — its
`!= "0"` half does not carry over, since `0` is a legitimate directory name
and this is a path rather than a boolean.

**The console window goes away in release builds only**, via
`#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]`.
A debug build keeps stderr, which is what a developer on Windows would want.
The consequence is that the `eprintln!`-then-exit paths go silent in a
release build: three are dev/CLI paths a player cannot reach, and the two
that remain — a missing assets directory and a saves directory that cannot
be created — write `startup-error.txt` beside the executable *and*
`eprintln!`, unconditionally both. Branching on which is which is a `cfg`
nobody would maintain, and a message box is a dependency bought for two
error strings.

**macOS falls out of the same module, and that was the point of the
`../Resources/assets` probe.** It is three lines, it went in from the start
rather than as a later special case, and it is the entire `.app` provision —
inside a bundle the executable sits at `Contents/MacOS/` and its resources at
`Contents/Resources/`. It is checked *after* the beside-the-exe probe, and
`a_mac_bundle_finds_its_assets_in_resources` is what says so on a Linux
runner. `dirs::data_dir()` gives `~/Library/Application Support/` with no
work at all.

The probe uses `parent()` rather than `join("..")`. The `..` form resolves
identically for `is_dir()`, but the resulting path is handed to the asset
loader and then printed in a startup error, and a reader should recognise
what it is looking at.

**The recommendation is a plain binary, not a bundle**, until the game is
handed to someone who will not open a Terminal. A bundle costs no path code
now, but it costs a plist, an icon and a build step, and Gatekeeper's
click-through is no better on one than the `xattr -dr com.apple.quarantine`
a plain zip can document. What a bundle *would* fix is that double-clicking
a plain binary in Finder opens a Terminal window behind the game — macOS's
version of the console that `windows_subsystem` suppresses, and the one
place the two platforms are not symmetric.

Verified 2026-08-23 against the current tree, not carried over from the
spec's audit: `cargo check -p feral-processes-app-core --target
aarch64-apple-darwin` passes, and so do `dirs` and `dirs-sys`, which the
audit predated. The graph then fails at `blake3`'s build script handing the
host `cc` `-arch arm64 -mmacosx-version-min=11.0` — a missing macOS
toolchain, not a portability defect. Cross-compiling is deliberately not
supported: a Mac is needed for the manual checklist anyway.

**What is unverified.** Everything about the Windows runtime: window
creation, DX12 through wgpu, WASAPI audio, keyboard input, the console
suppression, SmartScreen, and whether `%APPDATA%` resolves as expected. The
macOS runtime is unverified in exactly the same way and for the same reason
— Metal through wgpu, CoreAudio, Gatekeeper, and whether
`~/Library/Application Support` resolves as expected.
Verification is manual by choice — there is no CI — so the ten-step
checklist in
`docs/superpowers/specs/2026-08-19-windows-and-macos-distribution-design.md`
is the honest statement of what ships untested until someone runs it on a
Windows machine. A green Linux suite is not evidence that any of it works.

### A refusal is one sentence on two surfaces, and `App::refuse` is the one door

**A refusal is one sentence on two surfaces, and `App::refuse` is the one
door.** It sets `App::status_line` and calls `Game::note_refusal`, which
pushes a `MessageKind::Refusal` line. The popup the player typed into draws
the first; the message log keeps the second.

The bug this closed was reported as "the message is only in the log, under
the pop up screen." Half of that was wrong and the half that was wrong is
the interesting half. Refusals never reached the log at all: every one is an
`Err(String)` the engine returns, app-core parked it in `status_line`, and
`render/mod.rs::draw_status_banner` painted it as a red strip **along the
bottom edge of the window**. It was on screen, four seconds at a time, the
whole time — a full window's height away from the centred popup the player
was reading. A message nobody looks at and a message that was never written
are the same message, and the fix has to answer the first reading rather than
the second.

**So the refusal moved inside the popup**, under the title, above the rows.
`draw_popup` takes it as an argument rather than reading it off anything,
which is what made all 83 call sites fail to compile until each had decided.
That was the point of threading it: a new screen cannot silently omit it,
because the parameter has no default.

It is **counted by `popup_layout` but is not a `Row`.** `popup_layout` splits
its rows into a header, the `Row::Item` span the body pages through, and a
footer; a refusal added as a `Row` would sit in the header and be harmless
today, and would be a renumbering hazard the moment anyone builds the status
into a row list that already has items. The count is what the layout needs —
the panel grows a line rather than covering one, and the body's capacity
drops by one so nothing falls off the bottom.

**`draw` is the one place that knows which surface is on top**, so it is
where the message is handed out. Where an arm draws two things — the main
menu under the quit confirmation, the map under a mode overlay, the arena
builder under the save prompt — the underlying one takes `None`. Two of
those were found by the census rather than by reading: `Mode::Playing` fell
into the arm that hands the map `None` and so showed the refusal *nowhere*,
and `Mode::ArenaSave` drew it twice.

**`needs_status_banner` inverted.** It used to mean "everything except the
four screens that show the line themselves"; it now names the four that draw
no popup at all — `Battle`, `BattleResult`, `FrameMap`, `FieldRoutineCell`.
`Battle` is the load-bearing one, for the reason below.

**`Game::note_refusal` is silent while a battle is open, and that is the one
rule with teeth.** `MessageLog::since_round` slices the battle pane by
*position*, and `App::advance_reveal` paces the reveal by counting **raw**
lines. A line pushed from a battle submenu therefore lands inside the round's
range: drawn as narration with no round header to explain it, and swallowing
one keypress' worth of reveal on the way past. This is the same trap
`MessageLog::round_start` records one section up. The refusal still reaches
the player — the battle screen's strip is what carries it — which is why
`Battle` had to stay in `needs_status_banner`.

**Not every `status_line` write is a refusal.** "Game saved.", a fuse's
receipt, and the save-system's IO failures keep assigning the field directly.
The log is a record of the game saying *no*; routing a confirmation through
`refuse` would put "Game saved." in the history a player scrolls back through
looking for why nothing happened. `App::report` is the verdict form the 28
`Ok(())`/`Err(e)` call sites collapse into, and it takes the finished
`Result` rather than a closure so the `&mut self.game` borrow every caller
holds ends before `refuse` needs the whole of `self`.

**The census is `every_screen_draws_a_refusal_exactly_once`.** It drives all
86 `Mode`s through `draw` and counts what `paint::painted_text` actually
recorded. Both halves have caught a real defect — a screen that showed it
nowhere and a screen that showed it twice — and neither is visible to a test
that only checks a draw call did not panic. Nineteen modes need pending state
a fresh run has not got (a chosen trade, a program picked to fuse) and draw
nothing at all; they are listed as such rather than quietly excluded, and the
census still asserts they draw it **zero** times.

### Taking and putting are one screen, and the commit takes before it gives

A Depot could be collected from with `c` and deposited into with `P`, and an
item sitting on a shelf *and* in the pack had a row on each screen with no way
to see the other. The two screens are now one, `Mode::Transfer`, whose per-row
amount is **signed**: negative puts into an adjacent Depot, positive takes off
an adjacent `Stock`.

**The engine grew one module and lost six doors.** `game/base/transfer.rs`
holds the union offer, the room, the two refusals and the one commit. It does
not reimplement either half: `collect_items` and `deposit_items` were first
split so their moving bodies became `take_from_adjacent` and
`give_to_adjacent` — `pub(crate)` movers that hold **no guards**, neither
tick nor log — and `transfer_items` calls both, logging once and ticking once.
That split is what lets one action move cargo in both directions; leaving the
guards in the movers would have meant checking them twice and, worse, would
have left two functions each able to spend a turn.

**Take before give is the one ordering constraint, and it is load-bearing.**
A rebalance that empties a full Depot and refills it from the pack only lands
both halves in this order. The other way round the give clamps to zero for
want of room and the failure is *silent* — `give_to_adjacent` returns an empty
list and nothing is said. It is pinned by a test whose fixture is a Depot at
exactly `capacity`.

**`transfer_room` exists so a zero can be told from an absence.**
`deposit_room` answers 0 both for a Depot with nothing left and for no Depot
at all, and the screen has to draw those differently: the room line is omitted
entirely when there is no Depot, because a line reading `Depot room remaining:
0` beside a Mining Node claims the base is full when it has no shelf. So the
engine answers `Option<u32>`, app-core carries it as `basket_room` unchanged
in type and changed in meaning, and nothing infers the `None` from a zero.

**The two ceilings are different shapes, and only one is shared.**
`take_available` is the row's own shelf. `put_available` is the pack row
capped by the Depot room the *other* rows have not spent — subtracting only
the others is what lets the highlighted row be lowered and raised while it is
being edited, which is `basket_available`'s old argument carried across. A
pending *take* deliberately does not credit the put budget: a take may come
off a machine that is not a Depot, so crediting it would offer room that never
appears. Under-offering is safe precisely because the commit takes first.

**The key table stayed one table and grew a sign.** `half_way_to` generalises
the Ctrl step over the sign, still `div_ceil` on the *magnitude* of the gap so
a gap of one closes rather than stranding; digits accumulate in the row's
current sign, and a row at zero types a take. **Left puts in and Right takes
out** — the inversion collect shipped with, kept, and now named in as many
words by `left_puts_in_and_right_takes_out`. `[A]` writes the take ceiling
over every row, clearing a pending give; that is a decision about what "take
everything" means on one axis, not an oversight.

### `Game::copy_power` is the one door to a rating, and every term in it is a call

A player comparing two pieces of gear had six stat axes and no scalar. Every
list row that names an item now carries one figure, and one derivation
produces it: `game/gear_power.rs`.

**It is absolute, not relative to whoever is holding it.** Every copy is
priced against one fixed reference wearer in `tuning.rs` — a mid-run player,
derived rather than invented: the zone is the midpoint of the range
`balance_sim` sweeps, the level is what its geared sweep reports as the
minimum to clear that zone, and the three stat figures are
`stats_after_levels` of `PLAYER_BASE_STATS` at that level.
`the_power_reference_wearer_is_a_levelled_player` asserts the derivation
rather than trusting the arithmetic in the comments. A reference far from
where players actually stand makes every figure in the game wrong in the same
direction, which is hard to notice and easy to ship.

Absolute is what lets one number mean one thing on the inventory list, a
trader's shelf, a recipe's result and the inspect page. The **swap picker's
delta may disagree with it, and that is correct** — gear locks in
`EquippedItem::level`, so the worn piece and a candidate are scaled at two
different levels. The column is a property of the *copy*; the delta is a
property of the *swap*. `a_copys_rating_does_not_move_with_the_zone` is what
says so, because the reader who "fixes" the disagreement by making the column
contextual gives one copy two numbers on two screens.

**Four terms, and none of them restates a formula that already exists.**
Attack and mitigation go through `Stats::power` — the game's own "how strong
is this" scalar, which already prices mitigation as the effective HP it buys
rather than summing a percentage into a total. The damage band is a
*difference* against `POWER_REFERENCE_DAMAGE`, because a weapon **overrides**
the natural attack rather than adding to it (`Game::attack_range`), so a band
worse than bare fists is worth negative offense — that test is the one that
fails if the term is ever written as a sum. Accuracy and evasion are
**proportional**, priced through `battle::hit_chance` as the fraction they
move the throughput they act on: a probability is not a quantity and must
never be summed into a total. `EquipmentStats::decompiler` gets **no term** —
it buys taming, not combat.

`rate` is split out from `Game::copy_power` as a pure function so each term
can be exercised on its own axis; a shipped item mixes them, and a term
nothing catches is a term that can be deleted later by accident. All six
were mutation-proved: each deleted term reddens exactly one test.

**`None` means "no combat axis", never "rated zero".** A Decompiler module, a
consumable and a material all answer `None`, and the two censuses in
`tests/assets.rs` hold both halves over the real assets — every shipped
equipment item rates, and nothing that is not equipment does.

### `PowerCell` has three cells and three meanings, and they are not interchangeable

The rating reaches six screens through the existing `with_tag` seam rather
than through six `format!` calls, and sits **between the category tag and the
name** so it inherits the fixed-width `row_lead`. The figures forming a
straight edge down the list is the entire feature; in `Row::Item::suffix` they
would stagger, because `suffix_x` places a suffix one inset past each row's
*own* right edge.

`Rated(n)` is a rating. `Unrated` is an em dash: there is no answer, not a bad
one. `Blank` is a row that is not an item at all — the wagon's Routine and
Program offers, an empty equipment slot, a program held in the weapon slot —
where a dash would claim the disk had been rated and found wanting.

**A fifth parameter on `with_tag`, not a defaulted builder**, deliberately:
every call site is made to decide and the compiler is what makes them. That is
the same move the tag column itself made.

The row is now four `ui_runs` pieces, and `a_tagged_rows_pieces_join_back_into_its_text`
still holds them to joining into exactly the string `draw_row` measures — a row
measured from one set of pieces and drawn from another is a suffix landing on
its own tail.

**The gear inspect page pays for its breakdown out of the affix block.** The
page has no scroll, and `the_tallest_gear_page_fits_its_popup` refused the
row-per-axis form the breakdown was first written as: a Crash Handler built a
25-row page into a 23-row popup at 600px. Compacted to one line it was still
one row over, and the page had exactly zero headroom — so `GEAR_AFFIX_ROW_CAP`
went from 3 to 2. That block already had a cap, and it degrades by *counting*
what it cannot draw rather than dropping it in silence, which is why it is the
one that can lose a row and still say what it is for.

### The caravan is one basket, and the commit sells before it buys

The wagon was a list you bought from a row at a time, with a per-item quantity
page for selling. It is now a basket: every row carries an amount, Enter
commits them all through `Game::commit_caravan_basket`, and
`Mode::CaravanQuantity` was deleted outright.

**Two ordering rules, and they are the whole reason the function exists.**
Every refusal lands before anything is spent — `buy_caravan_offer` already
held that and said why: a purchase that took the Credits and then failed is
the one bug the player cannot undo, and a caravan has no buyback to put it
right with. A basket makes that stricter rather than looser. And **sells land
before buys**, `transfer_items`' take-before-give rule, which is what lets a
basket be funded by its own sales — the entire reason the two sections are one
basket rather than two screens.

The funding test starts the player below the purchase price with cargo whose
sale covers it, and **asserts the resulting Credits rather than the outcome**.
That distinction is load-bearing: with the order reversed the goods are still
delivered, because the aggregate affordability check has already passed and
`Inventory::take` clamps — the price simply vanishes out of a purse that had
nothing in it. Only the arithmetic catches that, which the mutation run
confirmed.

**One tick for the whole commit**, not one per line: the basket is the visit.
`close_if_gone` still runs after it, because the tick spent may be the one the
trader leaves on.

To hold "every refusal first", `buy_caravan_offer` and `sell_to_caravan` were
each split into a side-effect-free refusal half and an infallible apply half.
The Program arm's roster check is counted **down across the basket** rather
than re-read per row — two programs asked one at a time would both pass
against a roster with one slot left — and the species resolution
`adopt_program` would answer `None` for is asked up front, where the old code
learned about it after the goods had started moving.

**`Mode::Caravan` had to be named in the modifier fold at the top of
`App::handle_key`.** Miss it and the four modified-arrow variants are folded
to bare `Left`/`Right` before the caravan handler sees them, so Shift and Ctrl
silently become plain steps and nothing anywhere fails.
`shift_left_empties_a_sell_row_in_one_press` is the only thing that catches
it.

**Right increases and Left decreases — not the transfer picker's inversion.**
That inversion is specified for a single row spanning both directions, so its
amount is signed and an arrow picks an end. Here the sign is fixed by which
section a row is in, so inverting would read as a slip.
`left_puts_in_and_right_takes_out` is about `Mode::Transfer` and stays
untouched. `[A]` fills the **sell** rows only: on the picker it writes the
per-row ceiling over every row, and here that ceiling is the sell side —
filling the offer side would spend the whole purse on one keypress, on a
screen with no buyback.

**The two ceilings differ in shape, mirroring the picker.**
`caravan_sell_available` is the row's own stack, per row and static;
`caravan_budget` is one budget shared across the offer rows — Credits, plus
the basket's pending sales, minus its *other* pending buys. Subtracting only
the others is what lets the highlighted row be lowered and raised while it is
being edited. An offer clamps to `0..=1`: a shelf slot is spent whole, and
`CaravanOffer::qty` is part of the price the player was quoted. All three
take the `CaravanView` as a parameter rather than reading a cached copy,
which is what keeps the figures the screen draws and the figures the keys
clamp against the same numbers.

**Grouping is a property of the view, not of the shelf.** The offers are
sorted by `Game::caravan_group` in `caravan_view` and deliberately not in
`caravan_shelf`: the deal is a round-robin across the three equipment slots
whose leading slot rotates per visit, and sorting the shelf itself would make
that rotation unobservable and open every wagon with a weapon. `index` is
handed out before the sort and is the tiebreak after it, so the rows move on
screen and no shelf identity moves with them — `CaravanMemory` keys on it and
`buy_caravan_offer` resolves by it. `caravan_group` returns rank *and* heading
together so the sort and the header cannot disagree about where a run starts,
and it is exhaustive on `CaravanOfferKind`: two of the four kinds are not
items and have no `ItemCategory` to head under.

### A deploy is a request, and `Game::spawn_structure` is the one place a structure is written

**A deploy is a request now, and the Home is the only build the player's own
hands finish.** `Game::place_structure` answers every refusal it always
answered — researched, in base space, standing on laid floor, cell free,
under `max_deployed` — and then, for anything but a Home, spawns a
`components::BuildSite` on the cell instead of the structure. A body posted
by `schedule_base_labour` fetches the bill of materials by hand, sets it
down there, and raises the thing over `tuning::BUILD_TICKS_PER_MATERIAL`
ticks per unit of material.

**Why the Home is exempt, and why that is not a special case to be tidied
away.** Founding is the one build with nobody to ask. Base space does not
exist before a Home stands: there is no roster inside it, no shelf to fetch
from, and `require_base` refuses entry for want of the Home you are trying
to build. A run that had to ask a program to raise its first Home could
never start a base at all. So founding keeps the whole of the old verb — the
pack-charging, the shortfall refusal, the structure standing before the call
returns — and everything downstream of it is crew labour.

**Nothing is charged at filing, and that is the decision the rest hangs
off.** The alternative was to refuse a request the base could not yet afford,
which is what the verb did for its whole life before this. What that trades
away is the only thing a queue is for: production catches up, and a request
filed against an empty base starts on its own the moment a Mining Node makes
the last unit. It also makes the reporting obligation move rather than
vanish — the old refusal logged the shortfall because it was the one build
refusal that left the player an errand, and the errand is still there, so a
builder standing at a site with nothing anywhere to fetch says so once,
latched on `BuildSite::announced_dry` under `set_machine_status`'
only-on-transition rule. The latch **clears when a source appears**, unlike a
dig site's, because a build waits on a bill of several items over many trips:
said once and never again, a base that ran dry in zone 2 would stay silent
about running dry in zone 6.

**The materials are not spent until the structure is raised**, and that is
what makes a cancel a refund rather than a rebate. Units leave their shelf
when a builder picks them up and stand on the cell from then on; they are
consumed by the site being despawned at completion, never by an arithmetic
step of their own. So `Game::cancel_build_request` hands back goods that
still exist, and `save::BuildSiteSave::delivered` is the load-bearing save
field — dropped, a reload destroys them and the crew fetches them a second
time out of a base that no longer has them.

**`Game::spawn_structure` is the one place a structure's component list is
written**, extracted here because it acquired a second caller with nothing
else in common: the player founding a Home, and the crew finishing a
request. This is `Game::roster_parts`' argument applied to the other roster.
Left inline in `place_structure`, the list would have had to be copied into
`run_build_crew`, nothing would fail to compile when the two drifted, and a
crew-built machine quietly missing its `MachineStatus` or its `ResourceNode`
reads as the base being broken rather than as a missing line. It performs no
checks at all, deliberately: every refusal belongs to whoever decided to
build, and for a crew-finished request those were answered when it was filed
— which is why `max_deployed` counts pending requests alongside standing
structures, or a player could queue a whole base's worth of a capped machine
and every one would be raised.

**Build wants are prepended to `schedule_base_labour`'s want list, and that
is the whole of "a build outranks production."** The priority *is* the
position in that list, since `truncate(staff.len())` cuts from the end —
dig wants are appended last for the mirror-image reason. Three properties
fall out of the existing scheduler rather than needing anything new: a base
with a spare body never sheds a worker, because the diff leaves every
still-wanted posting where it is and hands out the idle pool first; a base
short of bodies takes one off a machine, because the truncation cuts the
lowest-priority want; and a base with nobody at all simply leaves requests
standing, which is a state and not a fault. The one thing that did need
changing is the empty-queue standdown guard: read off `WorkOrders` alone, a
base whose only instruction was a build request would be reasoning from
"nobody has told this base anything" while somebody had.

**The fetch is the genuinely new machinery, and it is not the dig crew's.**
The dig crew's substrate draw goes through `stock::spend_from_base`, which
*teleports* a unit off a shelf; a builder has to walk there, which is what
the feature is for. `construction::Source` is the two stores it can walk to —
every deployed structure's output buffer, the same set the stock strip counts,
plus the party's pack. The pack is reachable **only while the party is in base
space**: a builder walks over and takes it out of your hands, so there has to
be a pair of hands there, and four frames down the Stack the pack is simply
not a source. It sorts last among equals, so a builder that can reach a shelf
takes the shelf.

**The put-back is deliberately narrower than the draw.** `stock::
return_to_depots` is Depots alone, where `spend_from_base` is every buffer,
because a unit pushed into a Mining Node's output is indistinguishable from a
unit that node produced and would be hauled away and counted as a cycle's
yield. The dig crew's substrate handling already carries this same asymmetry.
Anything that fits nowhere is logged rather than dropped in silence — a base
with no Depot, the party away, a build cancelled is reachable, and a player
who is not told simply sees the stock strip fall.

**`views::BuildOrderRow` exists before the screen that will draw it**, and
that is on purpose rather than speculative generality. Three readers want
the same answer today: the map, which needs to know a cell is a pending build
at all; the examine line, which is the only place the materials standing on a
site are visible; and `Game::build_order_report`, the list a build-order
screen will page. Every figure in it is a call — `BuildSite::outstanding`,
`required_ticks`, `structure_name`, `item_name` — so no screen can report a
percentage the crew disagrees with. `BuildSite::required_ticks` is likewise
**derived from the stored cost and never stored beside it**: a saved figure
could only ever drift from the bill of materials next to it after a retune,
which is `Platform`'s radius argument reaching the save format.

**Two things a future widening must not quietly break.** "One builder at a
time" is a property of the scheduler naming a site once, not a count on the
component — a second builder is a scheduler change and costs no save-format
bump. And `TaskKind::Construct` is the first task kind whose holder may be
*carrying*: `schedule_base_labour`'s never-free-a-`Carrying`-holder rule
already covers it and has to, because freeing the body drops the `Carrying`
with the `Task` and those units have already left the shelf they came off.
