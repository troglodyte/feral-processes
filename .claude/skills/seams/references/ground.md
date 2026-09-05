# The ground

- **`Game::terrain_at` is the one door onto what terrain does to you, and
  `Game::environment_biome_at` is the single definition of its gates (zone
  1, `Biome::Platform`).** Two copies of that check — one in `terrain_at`,
  one in `Game::note_static_turnover`'s epoch-boundary hook — is how neutral
  zone 1 lapses, so `note_static_turnover` calls the same gate rather than
  carrying its own. **The trap is that the biome's *name* is deliberately
  outside that gate** — `Biome::name` and the crossing line in `move_player`
  fire from the first step of a run, and
  `zone_one_takes_no_bite_but_still_names_the_ground` asserts both halves in
  one test because the effect half alone passes against a bare early return
  that swallowed the name too. Terrain never costs Power and never raises
  Trace, the player alone takes the damage, and the bite goes through
  `Game::apply_damage` — which is the whole of why mitigation applies to
  ground for free. `assets/sectors/` is gone outright, not merely
  deletable — `sectors.rs` went with it, and every zone is undifferentiated
  now, permanently: `WorldMap::classify` reads flat module constants
  (`VOID_ELEVATION`, `BLACK_ICE_ELEVATION`, `DEADLOCK_TEMPERATURE`,
  `NULL_TEMPERATURE`, `NULL_MOISTURE`, `BACKPLANE_MOISTURE`) with nothing
  left to name a zone or rotate its palette. Don't go looking for a
  directory to drop a sector file into; there is no loader left to read
  one. Content deletability's other precedent still stands on its own:
  `assets/environment/` used to be deletable the same supported way, and
  that directory no longer exists either — the catalogue is now
  `crates/engine/src/environment.rs`, an exhaustive Rust `def()` on
  `notifications.rs`'s shape, not a loader.
- **`EnvironmentEffect`'s fold is additive on every term except the ambush
  multiplier, which multiplies.** The struct carries all four terms at
  once (`attrition_percent`, `min_damage`, `extra_ticks`, `ambush_mult`)
  precisely so ground and weather can stack rather than replace each other.
  **The trap: the rename that produced this shape (`ground_effect` →
  `terrain_at`) fails every call site loudly, but the shape change itself
  does not.** Code that already holds an `EnvironmentEffect` and reads only
  `attrition_percent`/`min_damage` — the two terms that existed before
  weather — compiles clean and silently drops `extra_ticks` and
  `ambush_mult` from whatever it does next. There is exactly one production
  reader (the hook in `game/turn.rs`), and the test that holds it,
  `ground_and_weather_effects_stack`, asserts all three fold terms in one
  function rather than checking the bite alone. `clamped` cuts the *summed*
  effect to `MAX_ENVIRONMENT_ATTRITION` / `MAX_ENVIRONMENT_DRAG_TICKS` /
  `MAX_STATIC_AMBUSH_MULT` — a fold can exceed either source's own ceiling
  even when both are individually inside it, which is why the ceiling check
  moved from a load-time refusal to a census over both `GroundCondition::
  all()` and `StaticEvent::all()` in `tests::environment`.
- **Which `StaticEvent` is live is derived from `(seed, zone, biome,
  epoch)` and never stored.** `Game::static_at` reduces
  `static_seed(seed, zone, biome, epoch)` through `derive::index` — never
  `%`, `descriptions::Slot::tags`'s reason — against a weighted pool that
  includes `STATIC_CLEAR_WEIGHT`, so most epochs in most biomes are clear.
  **The trap is reaching for `Biome as u64` instead of the private
  `biome_ord`.** `Biome` derives `Serialize`, so its discriminant is
  save-adjacent — folding the raw discriminant into the seed would silently
  re-roll every existing world's weather the day a `Biome` variant is
  inserted or reordered, which is exactly the kind of change that looks
  inert in a diff. The derivation draws **no** `resources::GameRng` — a
  draw here would not survive a save/load and would shift every later roll
  in the run, `stack::generate`'s worldgen rule — and it takes the epoch as
  an argument rather than reading the clock internally, because
  `note_static_turnover` has to ask what was live in the epoch that just
  *ended* to announce a clearing; a version that only reads
  `current_tick()` cannot answer that question at all.
- **A breach's re-tier is two calls, and either one alone is inert.**
  `enter_next_zone` clears `PopulatedChunks` *and* calls
  `Game::clear_local_wild`. **The trap is that the first looks sufficient
  and is not**: emptying the set does send `ensure_local_population` back
  over covered ground, but `populate_chunk` refuses every placement while
  `local_hostile_count` is at or above `WILD_LOCAL_DENSITY_TARGET`, and that
  count includes the survivors — so the re-stock fills only the gaps and the
  old tier's programs stand on worked ground for the rest of the run.
  Nothing else removes them: `cull_to_cap` evicts chunks *outside*
  `POPULATION_CHUNK_MARGIN`, the exact complement, and `WILD_CREATURE_CAP`
  never fires. **The second trap is the test that hides it** — a breach test
  that despawns the wild by hand before breaching proves only "an empty
  chunk re-stocks at the new tier", which is not the question; breach on
  populated ground or the test is vacuous. `clear_local_wild` keeps a
  `NestGuardian` (it belongs to a place, and clearing it leaves the nest
  bare until its respawn timer) and a `Nemesis` (clearing it is the grudge
  being forgotten by the breach). Neither exception is held by the compiler.
- **Where a settlement stands is derived off `(world seed, region)`, never
  stored — `rock::RockDb::kind_at`'s rule, reached here by
  `settlements::placement::settlement_at`.** The trap is folding a region's
  `(rx, ry)` in as a single word instead of a byte at a time: neighbouring
  regions — exactly the comparison this has to get right — differ in one
  low bit of one coordinate, and one XOR-then-multiply round only carries a
  difference about the fold prime's own width upward, so a whole-word fold
  never reaches bit 63, the bit `derive::index` reads. Regions anti-correlate
  into stripes that read as arbitrary one region at a time — the measured
  failure `descriptions::Slot::tags` documents, reached here by
  `rock::block_seed`'s route — and **never `%`** is why `derive::index`
  exists at all. The derivation only answers a *candidate* cell: it cannot
  see terrain, so `Game::ensure_local_settlements` walks outward from it for
  somewhere walkable, bounded by `SETTLEMENT_SITE_SEARCH_TILES`, and records
  the resolved tile in `resources::KnownSettlement` rather than re-deriving
  it — a later change to how that walk breaks ties must not be able to move
  a town the party has already reached. `resources::Settlements` stores the
  *whole resolved* `SettlementDef`, `ActiveContract`'s reason: a catalogue
  file edited or deleted mid-run must not rename or strand a place already
  known. Keyed by `SettlementKey` (region coordinates) rather than `Entity`,
  which does not survive a save, in a `BTreeMap` so the save encoding does
  not depend on iteration order. **"Already known" does not mean "recently
  visited"** — unlike `PopulatedChunks`, which `cull_to_cap` is free to
  forget so old ground restocks with different wild programs on a return
  visit, a settlement's record is permanent, and a modder deleting
  `assets/settlements/` gets the pre-settlement game (an empty catalogue
  derives to `None` everywhere) rather than an error.
- **A settlement is the fourth arm of `move_player`'s bump ladder, and the
  one arm that admits nobody.** Checked in `game/turn.rs` after the
  surface-link arm and before the walkable read, `Game::find_settlement_at`
  (mirroring `find_surface_link_at`'s query shape — `(&Position,
  &Settlement)`, no `Entity`, no filter type) queues a cue and returns
  before the step below it runs — the player's `Position` never changes,
  unlike the Stack-entrance arm one line up, because a settlement is a
  landmark read from the outside rather than a door with a hallway behind
  it. **The trap is treating the cue, `resources::PendingVisit`, as a plain
  read.** A getter passes every test except the one that calls it twice,
  and without a drain the screen it opens reopens on the very next
  keypress the player spends trying to walk away, since nothing ever
  clears it. `Game::take_settlement_visit` is the drain —
  `take_effects`/`take_transits`'s shape, `Some` once, `None` after — and
  it is deliberately unserialized, `CurrentStack`'s reason: a save that
  restored a pending visit would reopen the screen the instant the file
  loaded. `find_target_in_direction`'s settlement query is ranked *last*,
  behind a live creature on the same tile, because materialization walks
  to the nearest walkable cell rather than the nearest empty one, so a
  settlement and a wild program can share a tile and the program is the
  one worth pointing `x` at.
- **`Game::adjust_standing` is the one writer of a town's opinion, and the
  band under it is derived on every read.** `resources::Standings` holds a
  signed `i32` per `SettlementKey`; `relations::band` maps it onto
  `Hostile`/`Cold`/`Neutral`/`Warm`/`Allied` through four `tuning`
  thresholds. **The trap is a mover that writes the map directly.** The
  door holds two things nothing else does — the clamp, and the
  announcement, which speaks only on a *band crossing*
  (`set_machine_status`'s rule) — so the mover that skips it either lets
  standing run past its bounds or announces every basket. **The second
  trap is storing the band**: a retune of a threshold then leaves old
  saves filed under a boundary that has moved, and nothing in the compiler
  notices the two records disagreeing.
- **A consequence of standing is a *named query* on the band, never a
  table of effects.** `Standing::refuses_service` is the one shipped and
  its match is exhaustive (`cell_mark`'s rule), so a sixth band with no
  answer fails to compile rather than shipping as neutral;
  `every_standing_band_answers_whether_it_refuses_service` is the census.
  This is `perks.rs`'s seam copied on purpose — town-sourced raids and
  Phase 6's route predation are each a new query answered by the same
  match, so **the change to refuse is a `Consequence` enum and a lookup
  table**, which buys nothing the exhaustive match does not already give
  and costs a save-format decision the moment anything stores one.
- **A town refusing service answers with a *closed* view, never `None`.**
  `Game::settlement_view`'s `None` is already spoken for —
  `App::close_if_settlement_gone` reads it as the party having stepped off
  the tile and drops them to the map — so reusing it shuts the screen
  under the player with no line saying why, which reads as a crash. The
  view carries `closed: bool` with empty rows instead, and the gate is
  applied a *second* time at the top of `Game::commit_settlement_basket`:
  a view is a draw, and only the commit can spend, which is where
  `commit_caravan_basket`'s "every refusal lands before anything is spent"
  rule actually bites.
- **`Relation::trade_credits` is a remainder, not a total.** Trade pays one
  standing point per `SETTLEMENT_TRADE_CREDITS_PER_POINT` Credits moved,
  both directions counted. **Drop the remainder and the mover becomes a
  rounding rule**: ten small baskets pay nothing while one large basket of
  the same volume pays the lot, which reads as the feature being broken
  rather than as a threshold.
- **A town's garrison is one clamped term in `Game::total_raid_defense`, and
  the clamp is on the settlement half alone.** `run_raid` subtracts that
  total from `RAID_DAMAGE` with `saturating_sub`, so **omitting** the clamp
  lets enough Allied neighbours take every sweep to zero — the raid still
  picks a target and still logs, so it reads as working and no assertion
  fires — while putting it **on the total** caps the player's own shield
  network with a settlement constant, and two Shields are supposed to zero a
  sweep because that is a thing the player built.
  `the_garrison_cap_does_not_touch_the_structure_half` fails under the
  second mistake and passes under the first, which is why both tests exist.
  `SETTLEMENT_GARRISON_MAX < RAID_DAMAGE` is asserted in `relations.rs`, so
  closing that gap by retune fails the build.
  `Standing::garrison_defense` is a **magnitude** and ramps from `Warm`;
  `gifts_programs` and `hosts_a_relay` are booleans at `Allied`. That is
  what keeps three new queries from being three spellings of one predicate.
- **A gifted program's *species* is derived and spends no draw; adopting it
  spends what every adoption spends.** The fold is `(world seed, region,
  gifts taken)` with `SETTLEMENT_GIFT_SALT`, so a reload cannot reroll it —
  but `Game::adopt_program` rolls rarity and stats like the caravan's
  purchased program and the Stack's salvaged one, so a test asserting "the
  gift draws no `GameRng`" fails, correctly. State it as
  `choosing_a_gifts_species_spends_no_draw` does: adopt the same species by
  hand in a control run and compare where the stream ends up. The pool is
  `habitat_pools`' **ordinary** half, **sorted** (the draw indexes into it);
  the apex half is excluded because a gifted boss inverts the whole "labour,
  not power" decision. That decision is `SETTLEMENT_GIFT_STAT_MULT` alone —
  a tuning claim, not a structural one, since `ProgramRole` is derived and
  nothing stops the player fielding a gift.
- **A relay landing searches from band 1, filters like a step, and only
  queues the visit cue if it lands in reach.** Band 0 would set the party
  down on the settlement tile, which admits nobody (`move_player`'s fourth
  arm returns before the walkable step); `walkable` alone is not the
  question, because that ladder also turns aside for a wild program, a nest
  and a Stack entrance, and "checks walkable, not empty" has already put a
  town on a Stack entrance once. The ring order is `spawning::ring_tiles`,
  shared with `standable_near` rather than copied. And broken terrain can
  put the nearest standable ground several tiles out, where `PendingVisit`
  would open a town page whose `[M]` and `[J]` are then refused by the
  Chebyshev-1 `settlement_reach` — so the cue asks that same question.
  **Neither travel door calls `require_surface`**: the Relay is in base
  space, so it would refuse the only place an outbound trip can start.
- **The town page's aid sentences are `pub const` in the engine because the
  census that measures them lives in gui and cannot build a `Game`.**
  `Game::world` is private, so a renderer test cannot ask what the live
  lines are; a hand-copied list would drift on the first rewording, and
  `draw_row` clips vertically and never horizontally, so the drift is a line
  lost off the right edge in silence. `AID_LINES` is what both ends read —
  the engine asserts every emitted line is in it *and* that every line in it
  is reachable, the renderer measures the array. Sentences and not figures,
  because a read-only screen's rows belong to the engine
  (`message_history`'s rule) and because this game has never shown the
  player a tick: the gift's wait is banded into words the way
  `memories::age_phrase` bands a memory's age.
- **Relay travel is one seam in two crates, and shipping half of it is
  silent.** `Game::spend_travel_ticks` must break on a fight or a game over
  — it is the fourth multi-tick loop in the engine and the other three all
  do, because a tick can start a fight (`nest_aggro_tick`) and the rest must
  be dropped rather than resolving a world the party has left. And both
  travel keys in app-core must call **`after_world_action` itself, not a
  copy of it** (`finish_compile`'s rule): `handle_key`'s tail pays
  `after_tick`, which does autosave and notifications, while
  `after_world_action` is the only thing outside the battle screens that
  writes `Mode::Battle`, drains `take_settlement_visit` and checks game
  over. With neither half, a trip past a roused nest spends every tick with
  the battle already running, the map is drawn over it until the player's
  next action, a notification can take the screen mid-fight, and the arrival
  cue sits queued to open a town page later for a town already left. The
  charge is therefore **at most** the quote; a test asserting exact equality
  is only valid for an uninterrupted trip.
- **Every aid sentence on the town page must be a call, and getting that
  wrong twice is the shipped history.** The garrison line restated
  `garrison_defense`'s radius check under a doc comment claiming a call —
  the fold returns a clamped *sum*, so `Game::town_garrisons` is the shared
  per-town half both must use. The gift and relay lines ignored
  `settlement_reach`, while both their doors ask it, so a town examined from
  four tiles off (which `x` does — this page opens from anywhere inside
  `EXAMINE_RANGE_TILES`) offered a gift and a trip and refused them in the
  same breath. Assert the page and the doors in one test, never the page
  alone.
- **An aid radius is a fraction of `settlements::placement::REGION_TILES`,
  never a flat number.** `SETTLEMENT_GARRISON_RADIUS` shipped at 40 and
  `ROUTE_PREDATION_RADIUS` at 15 against a derivation that stands one town
  per 256-tile region, inset 24 tiles from its border — the median nearest
  town is 147 tiles from the anchor, so over 2,000 sampled worlds the first
  found a garrison in **1.6%** and the second found a predator beside a lane
  in **none**. **The trap is that a dead radius is indistinguishable from a
  weak one at the keyboard**: the player cannot tell "the garrison is not
  noticeable" from "there was no garrison", so a playtest returns the same
  answer either way and the constant gets nudged instead of scaled. The
  garrison radius is now `REGION_TILES / 2` and sits **outside**
  `SETTLEMENT_NOTICE_RADIUS`, inverting the doc comment that held it at 40 —
  noticing a deed needs proximity to the deed and the party roams, while
  stationing a detachment needs only willingness, which
  `Standing::garrison_defense` already gates at `Warm`. **The second trap is
  reading the nearest-town route as still under-tuned**: its corridor is
  short and points away from everywhere else, so it is unpreyable at *any*
  radius, and route risk is a property of hauling past somebody — 8% of
  second-nearest lanes, 18% of third. The gate is
  `tests/settlement_aid_reach.rs`, which samples worlds off the real
  derivation rather than asserting the values, so a ratio check cannot go
  vacuous; `aid_reach_probe` is the `#[ignore]`d sweep,
  `wild_density_probe`'s shape. `SETTLEMENT_NOTICE_RADIUS` is still flat and
  still unmeasured.
