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
frames down. `Game::require_surface` guards the eleven actions that reach
into the zone map through `Position`.
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
lowers a creature's HP.** Every other write to `Stats::hp` is a heal, one
of the two full-heals (`rest` in `game/turn.rs`, level-up in
`game/unlocks.rs`), or `needs_tick_system`, which is `With<Player>`. Put a
check that must see all damage here, not at the call sites.

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
cannot disagree, but it takes the zone on the surface and the depth in the
Stack. A new difficulty knob keyed to where the party is standing
reintroduces both bugs.

### Every difficulty curve in the game is linear, and that is a correctness property rather than a tuning preference

**Every difficulty curve in the game is linear, and that is a correctness
property rather than a tuning preference.** `ZONE_STAT_STEP`,
`STACK_DEPTH_STAT_STEP` and `GEAR_LEVEL_STEP` all *add* per level; they
used to multiply (x2 per zone, x1.35 per frame, x2 per gear level). The
player's side of the fight has only ever been linear — `ATK_PER_LEVEL` is
1, an item is worth a flat point or four — so a geometric enemy curve is a
geometric quantity racing a linear one, which has an end wherever the
coefficients are put. Under `battle::compute_damage`'s subtractive
`power + atk - def` floored at `MIN_DAMAGE`, past that end every swing
lands on 1 and the fight stops responding to levels, gear or roster at
all. Measured before the change: a zone-3 depth-5 lair guardian was
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
2026-08-05 removal was for. It is now its own literal 7. Nothing would
have caught either half: `balance_sim` is RNG-free and models no spawn
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

### The base's radius is derived, never stored, and is written at exactly three sites

**The base's radius is derived, never stored, and is written at exactly
three sites.** `Game::build_radius` is
`MAX_BUILD_DISTANCE_FROM_HOME` plus every deployed structure's
`build_radius_bonus`, clamped to `MAX_BUILD_RADIUS_TILES` — the same shape
`pet_capacity` has over `pet_slot_bonus`. It is *cached* on
`resources::Platform` for the reason that resource exists at all: the
footprint has to be readable from `&self` while the derivation needs
`&mut self` to query. The three writers are `stamp_platform`,
`clear_platform` and the load path in `game/lifecycle.rs`, which are
exactly the three that write `center`; a fourth lets the cache disagree
with the structures it comes from.
Derived-not-stored buys three things for free, and each would otherwise be
work. **No `SAVE_FORMAT_VERSION` bump** — the slab's tiles come back
through `tile_overrides` and the width comes back off the saved
structures. **It survives a breach** — `enter_next_zone` repositions the
base and re-stamps, and breaching despawns nothing, so the Pillars travel.
**The no-Home fallback stays correct** — no Home means no slab means no
Pillars, and the next Home stamps at the starting radius.
**The floor under all of it is that the slab always covers every structure
standing on it**, and that rule is why halving the starting radius did not
break every existing save. It is inert for a base built under the current
rules — `place_structure` refuses anything outside the footprint, so the
outermost structure is never further out than the radius already is, which
is also what stops an ordinary structure built at the very edge ratcheting
the base outward. What it is for is a base built under *older* rules: the
`chains` template measured 225 tiles of stamped floor against a buildable
69, so 156 tiles looked exactly like base and refused to be built on, with
the player standing on one of them. It belongs in `build_radius` and not
in the load path, because `enter_next_zone` re-stamps from the derivation
— a load-path fix hands the base back and takes it away again at the next
breach. Known cost, accepted: on a base that arrived wider than the
starting radius, a Pillar is absorbed until the bonuses catch up with the
width it came in at. Only a pre-halving save can be in that position.
**What actually bounds growth is `StructureDef::max_deployed`**, a count
of how many of a structure may stand at once — 5 on the Heap Pillar, 0
(unlimited) everywhere else, and the field a mod sets to bound a structure
whose effect accumulates. It is a def field rather than a `tuning.rs`
constant because it is a property of the structure, and the refusal sits
with the other refusals in `place_structure`, above the materials check.
**`MAX_BUILD_RADIUS_TILES` is a backstop, not a target.** It is 100, so
the Pillar's price is what paces growth and nothing downstream has to
reason about an unbounded footprint; `clear_platform` needs a bound for
its sweep, which is the other reason it exists. Two costs scale with the
square of the radius and were measured in a debug build at 1 walking
worker per tick: 8 ms at radius 10, 28 ms at 20, 65 ms at 30, 764 ms at
100, with the save running 137 KB to 3.5 MB across the same span. The tick
cost is why `haul_walk_radius` carries `HAUL_WALK_MAX_TILES` — capping the
reach keeps `post_reach` and `haul_step_system` in agreement because both
read that one function, so a base too wide to cross refuses the post
rather than accepting one that never arrives. The save cost is untouched
and would be removed by not writing slab tiles at all, since the slab is
derivable from centre and radius; nobody has needed that yet.
`MAX_BUILD_DISTANCE_FROM_HOME` is therefore the radius a base *starts* at,
and reading it where the question is about a base that exists is the
mistake to watch for: four readers were converted on 2026-08-13
(`place_structure`, `distance_from_danger_origin`,
`spawn_initial_creatures` — since deleted, along with the whole
player-relative seeding it belonged to — and `HAUL_WALK_RADIUS`, which
stopped being a constant at all — a fully grown base would otherwise refuse postings
across its own width, and `post_reach` is the single predicate the cronjob
menu and the assignment share). `clear_platform` is the one deliberate
holdout: it sweeps `MAX_BUILD_RADIUS_TILES`, not the live radius, because
it has to cover the largest slab that could ever have existed — halving
the start put every pre-existing save in exactly the position a
pre-corner-cut save was already in.
**A growing base buries what stands in the ring it claims, including a
Stack link, and the one it may not take is the last.** That is the
playtested correction to a spec decision: refusing any link at all read as
a permanent, unexplained cap on base size set by world generation — a
measured save had a link eight tiles out, so the base could never pass
radius 7 from any tile, and a link cannot be removed because walking onto
one descends. It was also stricter than the Home, which has always
despawned links under the slab it stamps (`stamp_platform`). The surviving
rule is the one the refusal was actually for: `award_loot` underground is
the game's only source of Portal Fragments, so a zone with no link left is
a run that can never breach.
**Growth is one-way, and that is what removes the shrink question rather
than answering it.** `remove_structure` refuses anything with a bonus
outside a Home cascade, so there is no reachable state with structures
standing outside the slab, no partial `clear_platform`, and nothing the
build rules call impossible.
**The Heap Block is the one part of the footprint this does not cover**,
and the entry below is where that argument lives. The radius is still
derived and still written at those same three sites; what the claimed set
adds is a fourth *field* written at the same three, not a fourth writer.

### The base grows on two independent axes, and only one of them is derivable

**The base grows on two independent axes, and only one of them is
derivable.** A Heap Pillar adds a ring to the circle, in every direction
at once, and is counted back off the structures standing in the base. A
Heap Block claims a single tile in the one direction the player chose, and
leaves nothing behind to count — it *cannot*, because the whole point of
the tile is that it stays empty and buildable. So
`resources::Platform::claimed` is stored where `radius` is derived, as
offsets from `center`, and rides an additive `#[serde(default)]` field on
`SaveData`. No version bump: the encoding has been field-named RON since
29. Offsets rather than absolute tiles is what makes the claims travel
with the base on a breach, the same way every structure's position does —
paid-for ground is part of the base, and the base travels whole.
`Platform::covers` stays the one statement of the footprint (circle *or*
claim), so `broker_reach`, `place_structure`'s reach check and
`stamp_platform`'s obliteration sweep pick up claimed ground with no code
of their own, and `open_to_hostiles` gets it free off the stamped biome.
`in_shape` is the circle alone, split out because `stamp_platform` needs
to ask which claims the circle has since grown over — those are dropped,
so the saved set stays the ground actually bought.
**The trap this closes is `build_radius`'s covering term.** "The slab
always covers every structure standing on it" was inert while
`place_structure` refused everything outside the footprint; claimed ground
makes it reachable, and one machine on a paved tile twenty out would grow
the circle to radius 20 *in every direction* — a slab over the sector, and
a Pillar's ring then added to that. So the term skips structures standing
on a claimed tile. That is also what makes "a Pillar adds one ring to the
circle, never an extension of the paving" true rather than incidental, and
`a_machine_on_claimed_ground_does_not_grow_the_slab` is the test that
fails if either half is undone.
**A claim refuses rather than obliterates**, which is the deliberate
difference from the Pillar's ring. A ring lands on whatever it lands on
and the last-link rule is what bounds the damage; a single tile is
trivially re-sited, so a link, a nest or a hostile standing there is a
refusal — and a nest left inside the slab would breed guardians in the one
place nothing is meant to stand. It must also *touch* the base, which
keeps the footprint one connected blob and prices distance in tiles rather
than in a second radius: paving twenty out costs twenty claims. The far
ceiling is `MAX_BUILD_RADIUS_TILES` for a reason that is not balance —
`clear_platform` sweeps exactly that box, and floor outside it would be
left behind forever.
**Both growth tools now run off the same chain.** A Block costs one Blank
Substrate and a Pillar six alongside its fragments, so the Lathe a base
already needs for Routine Disks is what feeds its expansion too — and the
Pillar supplies grid energy while it stands, which is the one thing that
makes growth pay for something other than area.

### A slab wide enough eats the Stack on-ramp's draw box, and the failure is the whole zone rather than one link

**A slab wide enough eats the Stack on-ramp's draw box, and the failure is
the whole zone rather than one link.** `spawn_surface_links` widens `reach`
to `STACK_LINK_SCATTER_TILES` only once `placed > 0`, against an attempt
budget of `count * 40` **shared across all three links** — so an on-ramp
that can never land spends every attempt at `placed == 0` and the zone gets
*zero* links. No links means no Stack, and `award_loot` underground is the
game's only source of Portal Fragments, so the run cannot breach again; it
reads to the player as a bad seed. Measured before the fix: zero at
`MAX_BUILD_RADIUS_TILES`, since `Platform::covers` claims every tile of a
box whose largest `|dx| + |dy|` is `2 * STACK_NEAREST_LINK_TILES` once
that stops exceeding `2 * radius - PLATFORM_CORNER_CUT`. The on-ramp now
draws from the Chebyshev ring band just *outside* the slab, walked
directly rather than rejection-sampled because every attempt has to yield
a candidate. Two consequences. `STACK_NEAREST_LINK_TILES` no longer means
"on screen" — at the ceiling the slab is wider than the pane is tall — so
`announce_surface_links` is what keeps the layer discoverable, and the
test that pinned the viewport now pins the doorstep and the scan. And
`frames_for` takes the build radius and subtracts it, the same correction
`distance_from_danger_origin` makes, or pushing the on-ramp out would
deepen every stack in the sector as the base grew — a difficulty change
caused entirely by a cosmetic one, against the depth curve that is already
the live concern.

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
group-size curves already take — zone on the surface, frame depth
underground — so there is no second difficulty axis to keep in step with
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
real assets at both ends: StaticField ships no band-0 species and OpenGrid
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
looking like a tuning edit. **That is closed.** The guardian is now drawn
from the window at its own depth — apex where the depth admits one, the
windowed ordinary pool otherwise — and is marked a boss either way, so a
biome with no eligible apex species yields a rolled guardian that pays
normally. `a_lair_guardian_is_a_boss_even_where_the_biome_has_no_apex_
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

### `Game::cast_field_routine` is Stack-only for two of the three effects it runs, and `require_surface` is not what does it

**`Game::cast_field_routine` is Stack-only for two of the three effects it
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

### The wielded program's proc casts as the *program*, not the player

**The wielded program's proc casts as the *program*, not the player.**
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
clauses: it is not `surface_only` while underground, *and* the screen it
opens would have at least one row. The second clause deliberately asks
only the first screen — Cronjob can list a program and land on an empty
structure picker, which is survivable because Esc backs into the menu.
`surface_only` is a flag in that table rather than an `is_underground()`
check inside each predicate, because what it has to stay in step with is
`require_surface`'s caller list in the engine, and only a table makes
that checkable. Emptiness alone would not do it: every `App::nearby_*`
scan reads the player's `Position`, which is pinned to the surface
entrance tile in the Stack, so those rows would offer to demolish a base
four frames overhead.

### A work order stores an item and a quantity, and nothing else

**A work order stores an item and a quantity, and nothing else.** No
per-machine plan, no unit targets, no progress counters — which machines a
line needs, in what order, who is on each and how far along it is are all
recomputed from live world state every time they are asked. This is the
same call `Game::build_radius`, `Game::contract_board`, `descriptions.rs`,
`Game::wielded_program` and the Stack's regenerated frames each make, and
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
What this does **not** change is `chain_break`: a feeder must still stand
beside the bench for an order to be queued at all, so a production line is
still a line. The shelf changes who gets the scarce body, not the topology.

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
**It never takes a body off a machine unless it has somewhere to put it.**
That rule is not in the spec and the `chains` template is what caught its
absence: with every wanted post already filled — a base whose queue has run
dry, or *any* base loaded from a save written before work orders — the
scheduler stood down every worker on the first tick, which is exactly the
regression `Game::load`'s absorption rule exists to prevent.
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

### A posted program sets off from the player's tile, and that is what makes its `Position` mean anything at all

**A posted program sets off from the player's tile, and that is what makes
its `Position` mean anything at all.** A tamed program's `Position` is
written at capture and never again — `views.rs`'s `worker_away_from_post`
doc says so, and `render/base.rs` refuses to draw a companion because
"drawing it would claim it is somewhere it isn't". Nothing syncs it as the
player walks; `enter_next_zone` is the one event that re-collects the
roster onto one tile. So the stale value is the tile the program was
beaten on, which can be anywhere the player has ever fought, and anything
measuring a distance from it is measuring noise: that is how a worker got
posted to a machine two tiles from home while standing 23 tiles north,
outside `HAUL_WALK_RADIUS` of its own station tile, never stepping and
never producing while the cronjob read as scheduled. `post_worker` (which
`schedule_base_labour` drives, and which was `assign_cronjob`'s body until
work orders took the menu away)
writes the tile from the player's, because posting is the moment it starts
being walked (`haul_step_system`) and the truthful value is where you were
standing when you handed the program over. The consequence is a design
decision, not an oversight: **the walk to a post is bought by posting from
a distance**, and posting while stood at the machine starts the program at
its station. The three tests pinning the walk-in put that distance on the
player for this reason, and `park_at_post` is only meaningful *after* an
assignment now — `stand_player_at_post` is the before.

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
filter, and the filter comes off before the capacity cut or a screenful of
base chatter leaves the field pane blank with older field lines still in
reach. History stays complete because its row count is shared — app-core
bounds the scroll while gui draws the rows, so filtering one side only
would open the screen on a row that isn't drawn.

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
over machines the player had to walk around. `station_tile` carries the
same filter — an occupied neighbour nominated as a station is a tile the
worker is sent to stand *on* — but `post_field` admits the worker's own
tile whatever occupies it, since `place_structure` never checks whether a
program is standing there and a worker built over would otherwise be
absent from its own field forever. You may step off an occupied tile,
never onto one.

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
rosters, same log pane, with the pruned results scrolling into it and
the action bar replaced by a continue prompt. The mode exists only so
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
is still nameable) and **above** `retain_outcomes_since_battle` (whose
four surviving kinds are exactly the ones the tally carries). One flush
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

### `MessageLog::retain_outcomes_since_battle` deletes lines, and runs inside `battle_resolve_round`

**`MessageLog::retain_outcomes_since_battle` deletes lines, and runs
inside `battle_resolve_round`.** So the narration of the round that *ends*
a fight is unreachable from outside no matter how carefully a caller reads
the log — capturing per round gets every round but the decisive one. That
is why `MessageLog::keep_battle_narration` exists: a flag rather than
better reading, set only by `arena`, whose report is the blow-by-blow the
prune is designed to keep off a map pane. Nothing else should set it; the
prune is right for every reader that has a pane.

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
base-menu row has always been `surface_only: false` precisely because the
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

What `AtBroker` measures is the **base**, through `Platform::covers`, and
not the distance to the Broker. That is not a relaxation for its own sake:
`place_structure` refuses everything but a Home until a Home is standing
and the slab always covers every structure on it, so a Broker is on the
base by construction and its own tile carries no information the slab does
not. The old rule was `CONTRACT_BOARD_RANGE_TILES: 2` — arm's length,
which read as arbitrary from the far corner of a base the player had built
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
and the failure is the same shape: a fused companion silently unable to cast
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
cast cannot pay for, or one charged more than the row quoted.

Both read the reserve off **the entity in question**, and that single
parameter is the whole of "every companion tracks their power level". A
companion's Special draws on the companion's reserve with no second code
path.

The two ends are deliberately asymmetric. `ability_unavailable` treats a
missing `PowerReserve` as **refusing**: between a companion that cannot cast
because a roster door skipped `roster_parts` and one with silently unlimited
Power, the first is the failure that gets reported. `spend_power` treats one
as a **no-op**, which is what makes hostiles safe without a branch — they
hold no reserve by design, because `choose_wild_action`'s weights were
trained against today's action distribution and a Power constraint would
cost a retrain that `CLAUDE.md` already records as not cheap.

**The charge is at the `BattleAction::Special` resolution site, not in
`use_ability`.** `use_ability` is also the path `proc_wielded_routine` and
hostile casts take, and both stay free — the proc's 25% rate is that
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

### `Trickle` is the one restore kind that does not scale with its caster

**`Trickle` is the one restore kind that does not scale with its caster**,
and the rule that excludes it is the one `scales_with_caster` already
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
per cast and take 60 underground turns to collect, which is a real Trace and
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
expired on the turn it was cast. That second corner was silently reachable
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
`cast_field_routine` charges the *player's* reserve even when a companion
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
retune. Its curve tests pass against a game whose entire casting economy has
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
