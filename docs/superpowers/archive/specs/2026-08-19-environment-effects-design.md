# Environment effects, phase 1: ground that does something

**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header. See `../../INDEX.md`.

## The problem

The surface has no environment. A `Tile` is `(biome, walkable)` and nothing
more: a biome decides where you can walk, what spawns there, and its colour
on the map. Standing anywhere on the surface does nothing to you, and the
player has never been told what ground they are standing on — every
occurrence of "Null Sector", "Open Grid", "Data Void" and "Black Ice" in the
tree is inside a doc comment.

Sectors (0.8.14) gave a zone a character, but the mechanical half is
deliberately one knob: a sector moves where the world generator's biome
boundaries fall, and roster, buildability and palette all follow from that
without a second setting to disagree with it. Nothing about a sector acts on
the player at runtime.

Every "this place does something to you" mechanic in the game lives in the
Stack — `CellKind::Corruption` bleeds a fraction of max HP per step,
`CellKind::Fault` drops the party a frame.

## Decisions taken, and why

Recorded so they are not re-litigated. Each was chosen against alternatives
that were considered and rejected.

**Effects are keyed to the biome.** Not to the sector, and not to a separate
hazard overlay. Keying to the biome means a sector's character becomes
mechanical through the one knob sectors already turn — Cold Storage bites
because it *is* mostly Deadlock ground, exactly as its roster and its
buildability already fall out of that. It also keeps an effect *avoidable*:
the map already draws biomes in distinct colours, so which ground you cross
is a routing decision.

Keying to the sector was rejected because it is unavoidable by construction,
which contradicts "property of the ground", and because it puts a second
setting on sectors that can disagree with the first. A hazard overlay — a
separate noise layer scattering patches independent of biome — was rejected
as a whole new worldgen layer whose main risk is turning the surface into a
second Stack.

**Zone 1 is neutral**, matching sectors, and for the same reason: the
opening zone's roster is chosen from species a fresh player can actually
beat, and the opening ring is exactly where per-step attrition is most
lethal. The gate lives inside one function so it cannot lapse at one of
several call sites.

The *names* are not gated. A zone-1 player still gets the log line naming
the ground they walked onto — learning the world's vocabulary is not a
difficulty knob, and withholding it would make the vocabulary appear from
nowhere at the first breach.

**Terrain never costs Power, and never raises Trace.** Power is not a
limiting resource — a Recharger Node deletes it as a cost near a base, so a
Power-draining biome is free at home and only bites in the field, which is
backwards. `Game::raise_trace` returns silently on the surface, so a
Trace-denominated effect would be a mechanic that only exists underground.

**`Biome::StaticField` is renamed to `Biome::Deadlock`**, to free the word
"Static" for the weather layer in phase 2 without a player reading "Static is
rising" while standing in Static Field.

The rename is free, and that is not obvious. The on-disk save is text RON
(`save.rs:651` writes `format!("{SAVE_FORMAT_VERSION}\n{text}")`, and the
reader is `read_to_string`; bincode survives only inside a test as a
byte-identity comparator, and the doc comment at `save.rs:258` calling the
save positional bincode is **stale and should be corrected in this change**).
RON names enum variants, and `Biome` reaches the save through
`SaveData::tile_overrides`. It also reaches every species mod, since
`SpeciesDef::habitats` is `Vec<Biome>`.

So the rename carries `#[serde(alias = "StaticField")]` on the renamed
variant. New saves and new files write `Deadlock`; existing saves and every
third-party species mod still parse. **No `SAVE_FORMAT_VERSION` bump, and no
mod breakage.** Without the alias this would be a save-format break, which is
the definition of "breaking" in this repo.

**Two effect kinds, not one.** `Attrition` alone would make every environment
"the ground hurts you". `Drag` bites by advancing the world around you rather
than by taking HP, which gives the vocabulary a second flavour and a
non-lethal option for zone-1-adjacent tuning later.

## What phase 1 does not do

Weather, encounter-rate effects and terrain-aware combat are phases 2, 3 and
4 (in that order — see the last section). Phase 1 ships the vocabulary, the
data file, the biome names and the standing-ground half only.

## Scope note: three effects is the ceiling

Of seven biomes, `DataVoid` and `BlackIce` are holes and cannot be stood on,
`Platform` is the base slab and is the one safe ground in the game, and
`OpenGrid` is the default ground and should stay neutral so that "ground that
does something" reads as an exception rather than as a tax.

That leaves **`Deadlock`, `NullSector` and `Mainframe`** — at most three
shipped ambient effects. This is a real ceiling on how much variety standing
ground can carry, and it is the argument for the variety the user asked for
living in the weather pools of phase 2 rather than here.

## Data: `assets/environment/*.ron`

One file per effect, loaded by `EnvironmentDb::load_dir` on the pattern
`SpeciesDb`/`StructureDb`/`ItemDb`/`AbilityDb`/`PerkDb` already follow: a
malformed file is skipped with a logged warning, never a panic, and the rest
of the directory still loads.

**Deleting the directory restores today's game exactly** — the same supported
way to play that deleting `assets/sectors/`, `assets/affixes/` or
`assets/policies/enemy_battle.ron` already is.

```ron
(
    id: "frost_lock",
    name: "Frost-locked",
    description: "Long-idle allocations. Every step through costs you.",
    biomes: ["Deadlock"],
    effect: Attrition(hp_percent: 0.01, min_damage: 1),
)
```

| Field | Required | Meaning |
| --- | --- | --- |
| `id` | yes | Unique key. A second file with the same id replaces the first. |
| `name` | yes | Player-facing. Named in the log line on entering the ground. |
| `description` | yes | Player-facing, one sentence. |
| `biomes` | yes | Which biomes run this. Deserialises as `Vec<Biome>`. |
| `effect` | yes | `Attrition` or `Drag`; see below. |

There is deliberately **no `occurrence` field in phase 1**. Every file here
is ambient. Phase 2 adds `#[serde(default)] occurrence: Occurrence`
defaulting to `Ambient`, which is the additive-and-defaulted shape that costs
no version bump and leaves every existing file — including a modder's —
parsing untouched. Shipping the field now with one honoured variant would be
an unused flag.

### Effects

```rust
pub enum EnvironmentEffect {
    Attrition { hp_percent: f32, min_damage: i32 },
    Drag { extra_ticks: u32 },
}
```

`Attrition` is a fraction of max HP, floored at `min_damage`, applied through
`Game::apply_damage`. A fraction rather than a flat figure for the reason
`bleed_corruption` already gives: terrain is uncorrelated with player level,
so any constant is lethal at level 1 and free by mid-run. Routing through
`apply_damage` — the one path that lowers a creature's HP — means a
Mitigation field buff blunts it for free, which is what a mitigation field
ought to do.

`Drag` makes a step cost `1 + extra_ticks` ticks. `move_player` already ends
in `self.tick()`; this calls it again.

**The player alone**, matching `bleed_corruption`: corrupting the party would
route program deaths and the permadeath path through something that is not a
fight.

### Three load-time refusals

Mirroring the two checks a sector file already carries — refuse the file
rather than shipping a broken world:

1. **An effect naming `Platform`.** The slab is the one safe ground,
   established in four places already; whether that holds is not a file's
   decision to revoke.
2. **A magnitude past its ceiling.** `hp_percent` above
   `MAX_ENVIRONMENT_ATTRITION` and `extra_ticks` above
   `MAX_ENVIRONMENT_DRAG_TICKS`, both new `pub const`s in `tuning.rs`. An
   authored `hp_percent: 0.5` is death in two steps; an authored
   `extra_ticks: 10_000` is a hang.
3. **Two ambient effects claiming one biome.** One ambient effect per biome.
   This is an authoring error rather than a merge rule, and failing fast at
   load beats picking a winner by directory order.

Naming a hole (`DataVoid`, `BlackIce`) is *not* refused — it is unreachable
rather than wrong, and refusing it would make a mod that names all six
biomes for convenience fail to load.

## Engine changes

### Biome names

`Biome::name(self) -> &'static str` in `world.rs`, an exhaustive match. This
is new player-facing content: Data Void, Deadlock, Null Sector, Mainframe,
Open Grid, Black Ice, Platform. It stays in Rust rather than becoming data
because the biome set is a fixed enum — mods extend species, structures,
items and now environments, but not the six shapes the generator sorts noise
into.

### One door

```rust
pub fn ground_effect(&self, x: i32, y: i32) -> Option<&EnvironmentDef>
```

The single definition, on the shape `views::drawn_on_surface_map` uses for
examine: one function that the movement hook and any later screen both read,
rather than two copies that drift. It returns `None` when the zone is 1, when
the tile is `Platform`, and when no file claims the tile's biome — the zone-1
gate lives here specifically so it cannot be forgotten at a second call site.

### The hook

In `Game::move_player`'s `walkable` branch (`game/turn.rs`), after the
`Position` write and **before `maybe_ambush`**. That order mirrors
`Game::arrive`'s established rule underground — the ground's bite is a
property of arriving, and it lands ahead of the encounter roll. Every branch
above that point has already returned, so walking into a creature, a nest, a
link or a portal cannot also be bitten, and shoving at a wall is not travel.

Order within the hook:

1. Resolve the effect for the destination tile.
2. `Attrition` → `apply_damage`.
3. The transition log line (below).
4. `maybe_ambush()` — already guarded by `is_game_over()`, so attrition that
   kills cannot also start a fight. **This needs its own test**; it is the
   one interaction in phase 1 where two systems meet at a lethal edge.
5. `self.tick()`, plus `extra_ticks` further ticks for `Drag`.

### The readout

One log line, fired when the destination tile's biome differs from the one
stepped off. It names the biome and, when the ground has an effect, that
effect's `name` and what it does. `MessageKind::Info`; the `MessageSource`
tag follows the table in that enum's doc comment.

Nothing is stored. Both biomes are in hand inside `move_player`, so the
transition is derived rather than remembered — no save field and no
`SAVE_FORMAT_VERSION` bump anywhere in this phase.

**Known cost:** biome boundaries are noise-generated and ragged, so stepping
back and forth across one logs each time. `resources::condense` folds
repeats into a row and a count on all three log surfaces, which blunts it.
Whether it still reads as noise is a feel question that only play answers,
and it is recorded as an open question rather than pre-solved with a step
counter nobody has evidence is needed.

## Testing

Engine unit tests, fixtures from `crates/engine/src/tests/support.rs`:

- Attrition applies on a step onto claimed ground, and not on a step that
  bounced off a wall.
- Attrition that kills does not then start an ambush.
- A Mitigation field buff blunts attrition — it goes through `apply_damage`.
- `Drag` advances the clock by `1 + extra_ticks`.
- The party is untouched; the player alone takes it.
- Zone 1 takes no effect, **and still gets the biome name in the log**.
- `Platform` takes no effect.
- The log line fires on a biome change and not on a step within one biome.
- An absent or empty `assets/environment/` reproduces today's game exactly.
- A malformed file is skipped and the rest of the directory loads.
- Each of the three load-time refusals rejects its file and keeps the others.
- A save written with a `StaticField` tile override still loads after the
  rename (the `serde(alias)` half), and a save written after it reads
  `Deadlock`.

Assets census in `tests/assets.rs`, over the real shipped files: every
authored magnitude is inside its `tuning.rs` ceiling, and no shipped file
claims `Platform`.

**`balance_sim` will not gate any of this.** It is a battle simulator and
models no walking at all, so attrition rates are ungated there in exactly the
way the Power economy already is. The suite proves the mechanism; the numbers
are a play question.

## Rename checklist

32 Rust references across 7 files, plus 7 species files' `habitats` lists,
plus `assets/species/README.md`. Two traps, both already recorded as
standing practice:

- **Gate on the new vocabulary, not the old.** Grepping the removed word is
  blind to what is half-converted around it, and `--type rust` misses player
  text and habitat lists in `.ron`.
- The sector schema field `static_temperature` is renamed to
  `deadlock_temperature` for consistency, which touches
  `assets/sectors/cold_storage.ron`, `assets/sectors/README.md` and five Rust
  files. This *is* a mod-schema change and belongs in the changelog entry as
  one, since a third-party sector file using the old key would silently lose
  its threshold. Consider a `serde(alias)` here too.
- `save.rs:258`'s claim that the save is positional bincode is stale and is
  corrected in this change.

## Documentation obligations

- `assets/environment/README.md` — new, the schema reference, on the pattern
  of `assets/sectors/README.md`.
- `assets/species/README.md` — the habitat list's biome names.
- `assets/sectors/README.md` — the renamed threshold field and the biome it
  names.
- `CHANGELOG.md` — one section, and it must say the sector schema key moved.
- `CLAUDE.md` and `docs/seams.md` — a new seam entry for `ground_effect`
  being the one door and the zone-1 gate living inside it, following the rule
  that the reasoning goes in `seams.md` and the rule goes in `CLAUDE.md`.
- `docs/manual.md` and the root `README.md` are carved out and stay stale.

## The later phases

Sketched only enough not to be re-derived. Each gets its own spec.

**Phase 2 — Static.** Weather, player-facing "Static", with individual events
named from the machine-under-load frame: Leaking Memory, Thread Storm, Packet
Flood, Signal Noise. Tied to biomes, biome-wide, **either on or off** — not a
front that moves through a biome. It shares this phase's file format, loader
and effect vocabulary entirely; it adds `occurrence: Static(weight)` and a
per-biome pool with an implicit clear weight.

Which event is live is derived — `f(world seed, zone, biome, epoch)`, on the
precedent of the Broker's board being seeded off `(world seed, zone, epoch)`.
No save field, no `GameRng` draw (worldgen must not draw from it), no
save-scumming, and it rotates on its own. Epoch length is a fixed
`tuning.rs` constant, which is what makes "on for x ticks, then dissipates"
derivable without state; a per-event duration is the thing that would force a
saved field, and is the tradeoff to weigh in that spec rather than assume.

The readout gains a second trigger — the ground changed under you — which is
derivable from whether the tick crossed an epoch boundary, so a reload does
not re-announce.

**Phase 3 — encounters.** Biome and Static bias the ambush roll.
`maybe_ambush` already reads the tile's biome to skip the slab, so the term
lands at an existing read.

**Phase 4 — combat reads its terrain.** Capture the tile at `start_battle`
and apply a modifier. `BattleState` has no notion of place today, and battles
are never serialised, so this costs no save bump.

## Open questions

- **Does boundary ping-pong read as noise?** Recorded above. A play question.
- **Is three ambient flavours enough for the ground to feel authored**, or
  does Open Grid need one too, at the cost of "does something" becoming a
  tax rather than an exception?
- **Does attrition make a zone-2 arrival too sharp?** The gate is zone 1, so
  the first bite lands the moment a player breaches, at the same moment the
  roster steps up. Phase 1 ships the mechanism; whether the first sector
  should be the one that teaches it is a tuning question for play.
