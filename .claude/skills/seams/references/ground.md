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
