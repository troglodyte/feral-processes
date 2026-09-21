# Honeypot traps

**Status:** design approved, unimplemented.

A placeable consumable that stands on the zone surface, blocks the player,
and over game-time catches a non-boss program that could have spawned on
its tile — leaving a downed program to be collected by hand.

---

## 1. What it is

A **Honeypot** is a decoy: a crafted item the player drops on an adjacent
surface tile. It sits there indefinitely. Every `TRAP_PERIOD_TICKS` it rolls
`TRAP_CAPTURE_CHANCE`; on a hit it captures one non-boss program drawn from
the same species window a wild spawn on that tile would use, and holds it.
Walking into a sprung honeypot takes the program into `DownedPrograms` and
consumes the honeypot. Walking into an armed one is a bump.

The vocabulary is deliberate. A honeypot is what this setting calls a decoy
that hostile programs wander into; "trap" and "snare" are the fantasy words
the rest of the game avoids. The identifiers are `honeypot`, `decoy_bench`,
`deception` — but the engine's component is `components::Trap`, because the
concept is general and a mod may ship a second one.

## 2. Decisions taken

These were settled in design and are not open in implementation.

1. **The catch is held, not delivered.** A sprung honeypot keeps its program
   until the player walks to it. Placement is therefore a thing you have to
   remember, which is what makes a field of them a decision rather than a
   background trickle.
2. **It never expires.** The only ways a honeypot leaves the map are being
   collected or being destroyed.
3. **Destroying returns nothing.** Reusing the demolish gesture, no
   confirmation, and a sprung honeypot loses what it caught.
4. **Placement is capped at `TRAP_PLACEMENT_CAP` (50).** Hidden from the
   player; it exists to bound the per-tick cost and the carpet, not to be a
   number anyone plays around.
5. **It blocks the player and nothing else.** Wild programs, pursuing nest
   guardians and town patrols ignore it.
6. **The catch is worse than a kill**: no battle loot at all, a condition
   penalty, and `carried: None` so a trapped program never yields a routine.
   Progression stays earned by fighting; a honeypot field is a materials
   trickle.
7. **A honeypot has a quality, and the catch cannot exceed it.** Quality is
   a `Rarity` ceiling authored on the item def. The shipped honeypot caps at
   `Silver`.
8. **Honeypots survive a zone breach** and keep working at the new tier.

## 3. Why quality is authored and not rolled

Rolling quality per copy the way `GearCopy` does would make a honeypot
*instanced*, and that is incompatible with the feature's own requirements.
`Inventory` is by definition the plain-copy store — `count`/`take` read the
first matching row — and `Stock` keys by a bare `ItemId`. An instanced
honeypot could not stack in the pack, could not sit in a machine's output
buffer, and could not be the subject of a work order. It would need a fourth
player store on `DownedPrograms`' precedent to buy nothing the player can
see.

So quality is one figure on the item def, identical across every copy. A
better honeypot is a second `.ron` file behind a later research node, which
is the moddability rule working as intended.

## 4. The gating, which is entirely data

The requirement "researchable" and the requirement "the base can fill work
orders for it" pull against each other, because `systems::assembly_recipe`
reads `ItemDef::craftable` and never a research node's `unlocks_recipes`,
and `Game::orderable_items` refuses any item no deployed machine produces. A
research-gated recipe is therefore unassemblable by construction.

One `requires_structure` resolves it with no new Rust:

- `assets/research/deception.ron` — `unlocks_structures: ["decoy_bench"]`.
- `assets/structures/decoy_bench.ron` — `assembles: Some((item: "honeypot",
  ticks_per_unit: 20))`.
- `assets/items/honeypot.ron` — `craftable: Some((cost: [("ice_breaker", 1),
  ("core_fragment", 4)], requires_structure: Some("decoy_bench")))`, plus
  the new `trap: Some((rarity_cap: Silver))`.

`Game::craft_recipes` will not offer the hand-compile until a Decoy Bench
stands; `orderable_items` will not list it until one is deployed; the bench
cannot be built until Deception is researched. Three gates, one field.

Two censuses bite on the research file and must be satisfied rather than
worked around: `every_research_material_is_reachable_through_that_nodes_own
_prerequisites` (the `materials:` bill may only name goods the node's own
transitive `requires` closure can actually make — `min_zone` grants nothing,
and `assembles` is not a source) and
`every_shipped_research_node_costs_materials` (the bill may not be empty).

The honeypot's `value` stays at 1. A craftable priced above its ingredients
is an infinite Credit loop, and ICE Breaker is itself worth 1.

## 5. The item field

```rust
/// Authored on any item the player may place on the ground as a trap.
#[serde(default)]
pub trap: Option<TrapDef>,

pub struct TrapDef {
    /// The best rarity this honeypot will ever catch. A roll above it is
    /// clamped down, never rerolled.
    pub rarity_cap: Rarity,
}
```

A struct rather than a bare `Option<Rarity>`: it matches how every other
capability on `ItemDef` is authored (`craftable`, `consume`, `upgrade`), and
it is where a second honeypot tier's own period or chance would land if one
ever earns them. `#[serde(default)]` so every existing item file and every
mod keeps parsing.

**Period and capture chance are not authored.** They are difficulty, and
difficulty is code here. They live in `tuning.rs`.

The new capability owes a line in `Game::item_effects`, which is the middle
of the three lengths of one derivation (`item_blurb`, `item_effects`,
`item_grant`). An item capability with no entry there is invisible on every
listing screen that explains what a thing does.

`assets/items/README.md` gets the field documented in the same change.

## 6. The entity

```rust
pub struct Trap {
    /// Which item def this was placed from. The rarity ceiling is resolved
    /// from the def on every read rather than copied here — see below for
    /// why this follows the nest precedent and not the contract one.
    pub item: ItemId,
    pub next_roll: u32,
    pub caught: Option<DownedProgram>,
}
```

Spawned with `Position` and `Glyph { ch: '^' }`. Drawing and examining are
free: `Game::view_entities_at` is a generic `(Entity, &Position, &Glyph)`
query and `views::drawn_on_surface_map` is neutral to entity kind.

A honeypot carries **no `Hostile`**, which is what makes surviving a breach
an omission rather than a check — `Game::clear_local_wild` is
`With<Hostile>`. Nothing in the compiler holds that, so it gets its own
test, and that test must breach on *populated* ground: a breach test that
despawns the wild by hand first is vacuous.

**Storing the id and not the resolved def is a decision against a stated
precedent, and the reason is what kind of thing a honeypot is.**
`ActiveContract` stores the whole resolved `ContractDef` so a file edited or
deleted mid-run cannot strand or rewrite an agreement already signed. A
honeypot is not an agreement; it is a device standing in the world, and the
precedent for those is `restore_nests`, which resolves its species live and
skips silently when the species is gone. Live resolution is also what lets a
retuned `.ron` retune honeypots already on the ground, which is the
behaviour a modder expects of a device and not of a contract. The cost is
accepted: editing a honeypot's `rarity_cap` mid-run changes what an
already-placed one will catch.

**There is no corner-mark channel left to spend.** The tile's accounting is
complete — top edge the rarity bar, top-left the con earmark, top-right a
nemesis, bottom-right a town patrol, bottom-left the staffed mark, all four
edges the status outline, the background wash a structure's `Durability`,
and the bottom edge the progress bar. A sprung honeypot therefore says so
**through the centre glyph itself**: the char changes, `^` armed to a
different char sprung. Not a mark, and not a tint — `ATTENTION`/`WARN`/
`THREAT` are overlay *roles* and a glyph's own colour comes from
`hud::palette::glyph`, which is a hue table and not a role table. The two
are different systems and must not be crossed.

The authored hue must avoid: bright cyan (the player's `@`, which nothing
else may take), bright yellow (unreachable from content by census), and
`GlyphColor::Orange` (a settlement's, and the one variant no species
authors). Magenta and cyan are spoken for by the boss and nemesis marks.
Red would read as a threat, which a honeypot is not. `GlyphColor::Yellow`
is the suggestion; any hue clearing that list will do.

`EntityView::difficulty` stays `None`. A honeypot is not `Hostile`, so it
gets *no* con reading rather than one worth nothing — which is the existing
rule for anything non-hostile, not a new exception.

## 7. Placement

`Inventory` row → `Mode::InventoryItemAction` → a new `[p]lace` action, shown
when the item has a `trap:` def → new `Mode::TrapDirection`.

`Mode::TrapDirection` is `handle_build_direction_key`'s exact shape: four
cardinal keys, the direction key itself commits, Esc cancels. It is a new
`Mode` variant, which **does not fail to compile** — it must be added by
hand to `render/mod.rs::ALL_MODES` (bumping the array length from 113), and
it joins the `every_screen_draws_a_refusal_exactly_once` census that drives
every `Mode` through `draw`. It is a popup like every other picker, so it
does **not** join `needs_status_banner`, which names only the screens that
draw no popup at all.

Examining a honeypot with `x` needs **no new `Mode`** — it reuses the
existing examine popup. Any detail rows it shows are built through
`description_rows` in the engine, so the description census covers them and
a read-only screen's rows stay owned by app-core rather than composed in the
renderer.

`Game::place_trap(item, dx, dy) -> Outcome` refuses, **every refusal landing
before anything is spent and each asserted by its own test**:

- not on the open zone surface (base space, the Stack, a battle, game over);
- the item is not held, or has no `trap:` def;
- the target tile is not `walkable`;
- the target tile holds a wild creature, a nest, a Stack on-ramp, a
  settlement, or another honeypot — asked through the same `find_*_at`
  queries `move_player`'s ladder uses, never a second set;
- `TRAP_PLACEMENT_CAP` honeypots already stand.

On success: take one unit from `Inventory`, spawn the entity with
`next_roll = TRAP_PERIOD_TICKS`, spend one tick. app-core owes
`after_tick()`.

Placement refuses an occupied tile, but **nothing stops a wild program
spawning onto a honeypot later** — `standable_near` and `scatter_open_tile`
check walkable, not empty, which has already put a town on a Stack entrance
once. That is accepted, not fixed: the creature arm of `move_player`'s
ladder is above the honeypot arm, so the player fights it and the honeypot
is still there afterwards.

## 8. The tick

`Game::run_traps()` runs inside `tick_inner`, beside
`maybe_spawn_wild_creature`.

**Honeypots are walked in `(x, y)` order, sorted before any draw.**
`assembler_system`'s rule: bevy's query iteration order is not stable, so
two honeypots whose periods elapse on the same tick would consume the shared
`GameRng` in an order that varies between runs, and that surfaces as an
intermittent seeded-test failure somewhere else entirely.

Per honeypot: if `next_roll > 0`, decrement and return — **no RNG draw**.
This is the point of the countdown: 50 honeypots cost about a tenth of a
draw a tick rather than fifty. When the period elapses, reset `next_roll`,
skip if already `caught`, then roll `TRAP_CAPTURE_CHANCE` once.

On a hit, with **no entity spawned at any point**:

1. `Game::pick_habitat_species(x, y, None, allow_boss: false)` — the
   existing "what could spawn here" answer. Non-boss by argument, never by a
   filter written here.
2. `Game::roll_rarity(&species_def, x, y, false)`, then
   `.min(def.rarity_cap)`.
3. **Clamp the rarity before rolling condition, never after.** This mirrors
   `downed_program_for`'s boss-rarity floor and exists for the same reason:
   `roll_condition` prices condition against the rarity the program actually
   ships with, and `DownedProgram::grade()` folds both, so a condition rolled
   against the pre-clamp rarity leaves the grade overstated for exactly the
   catches the ceiling exists to hold down.
4. `items::roll_condition(clamped_rarity, false, 0.0)`, then subtract
   `TRAP_CONDITION_PENALTY`, saturating at a floor. One roll, one door, and
   the penalty is visible in `tuning.rs` rather than baked into a second
   formula.
5. `level`: the current `ZoneLevel`, which is what `ability_user_level`
   answers for a wild program. There is no entity to ask, so this is the one
   place the feature restates a formula instead of calling one — the
   implementation should extract the wild arm into a `Game`-level helper
   both `ability_user_level`'s fallback and `run_traps` call, so the two
   cannot drift. Known gap inherited, not introduced: `ZoneLevel` does not
   move underground, and honeypots are surface-only, so it does not bite.
6. `carried: None`.

**The draw order is part of the contract**: capture roll, then species pick,
then rarity, then condition. A seeded test pins that sequence, so reordering
it is a deliberate change and not a refactor.

`run_traps` deliberately does **not** call `Game::field_escalation`. The
escalation terms exist to scale a spawned body's `Stats`, and a
`DownedProgram` carries none — its level is `ZoneLevel` and its worth is
`grade()`, which folds level, rarity and condition. An escalation term here
would be a number with nothing to apply to, and joining that function's
caller census would misreport what the feature does.

The result is stored on the component and the `Glyph`'s `ch` is rewritten so
a sprung honeypot reads differently from an armed one, per §6. A log line is emitted. Both of its axes are chosen deliberately rather than
defaulted: it is **not** base news, or `battle_rows` drops it unconditionally
and it is invisible whenever a fight is open — which `run_traps` can reach,
since it runs every tick. As a plain `Info` line it is pruned by
`retain_outcomes_since_battle` like any other narration, which is the right
lifetime for a transient event. `resources::condense` folds repeats across
all three log surfaces, so the line must name the species or the tile if two
catches are to read as two rows, and a test counting entries to prove one
line fired must sum `repeats` or it is vacuous.

Honeypots tick wherever the player is, including in base space and in the
Stack. The species window comes from the honeypot's own tile, not the
party's.

This is a new `GameRng` consumer, so it shifts the seeded stream once a
honeypot exists — which is why the countdown draws nothing and why existing
seeded tests are unaffected until one is placed.

## 9. Collecting, blocking, destroying

A new arm in `Game::move_player`'s ladder, **after the settlement arm and
before the generic walkable read**. `Game::find_trap_at` returns the
`Entity` as well as the data — unlike `find_settlement_at`, which needs no
entity — because collecting despawns it.

- Sprung: `Game::push_downed_program`. On success, despawn and log. On
  failure the store is full, that door already logs its own refusal, and the
  honeypot stays sprung. The player does not move either way.
- Armed: a bump. The player does not move. It spends a tick, as the
  settlement arm does.

Player-only blocking costs nothing to enforce: `pursuit_field` and the
patrol walk never consult this ladder, so there is nothing to opt out.

**A sprung honeypot does not join `Game::attention`, deliberately.** It
would be a legitimate row — unlike the rejected "pack full" idea, the downed
store has a real capacity in `MAX_DOWNED_PROGRAMS` — but it costs a new
`AttentionRow::kind` arm threaded through `hud::column::tab_of`'s exhaustive
match, and threat rows sort ahead of it so it would rarely be the badge's
leading line anyway. The refusal already logs when collection fails. This is
the natural follow-up once the feature has been played, not part of it.

Destroying reuses `d` + direction. **This lifts a deliberate gate.** `d` is
currently refused outside base space, with a comment in
`app/playing.rs` explaining that out there the four directions point at
nothing ownable — the structures are all in base space and the player's
surface `Position` is either their tile or pinned to the Stack entrance.
Honeypots make that false for the first time. The gate goes, the refusal
line is rewritten, and the comment is replaced with the new reason rather
than deleted. `handle_remove_direction_key` gains a surface branch: in base
space it is structure-then-build-site as today, on the surface it is
honeypot only.

## 10. Save

`SaveData::traps: Vec<TrapSave>` behind `#[serde(default)]`. The save is
field-named RON, so an additive field costs **no `SAVE_FORMAT_VERSION`
bump**.

```rust
pub struct TrapSave {
    pub item: ItemId,
    pub position: (i32, i32),
    pub next_roll: u32,
    pub caught: Option<DownedProgram>,
}
```

`TrapSave` is a **named struct and not a tuple struct**. That is the one
shape the additive-field rule does not protect: a positional tuple gains a
legacy field the next time a property is added.

`Game::trap_saves_for` and `Game::restore_traps` mirror the nest pair. No
`pending_cronjobs` deferral: nothing forward-references a honeypot, and a
honeypot references nothing but its own item id, which is dropped silently
if the def no longer resolves.

**A RON round trip cannot catch a skipped field.** The gate is a real save
and a real load, asserting position, `next_roll` and a held `caught`
survive.

## 11. Tuning

A new labelled section in `crates/engine/src/tuning.rs`:

| constant | proposed | note |
|---|---|---|
| `TRAP_PLACEMENT_CAP` | 50 | hidden from the player |
| `TRAP_PERIOD_TICKS` | 480 | 4 minutes at 2 ticks/sec |
| `TRAP_CAPTURE_CHANCE` | 0.15 | one catch per ~27 minutes per honeypot |
| `TRAP_CONDITION_PENALTY` | 15 | points off a 0..=100 condition, after `roll_condition` |

The arithmetic worth arguing with: at the 50 cap that is roughly 1.9 catches
a minute, which fills the 10-slot `MAX_DOWNED_PROGRAMS` store faster than
the player can walk the field. The real brakes are the walk to each sprung
honeypot, the ICE Breaker each one costs, and the store cap — not the rate.

**A known, deliberate leak:** `extraction_yield` multiplies
`DownedProgram::grade()`, and the drop-neutrality gate behind
`TOOL_BASE_UNITS` measures per *extraction*, not per kill. Honeypots are a
new source of downed programs outside the kill path, so that gate cannot see
them. The rarity ceiling, the condition penalty and `carried: None` are what
hold the economy; no test does, and this is the number a playtest is for.

## 12. What this touches

- **engine**: `items_db.rs` (`trap` field, `TrapDef`), `items.rs`,
  `components.rs` (`Trap`), `game/` (`place_trap`, `run_traps`,
  `find_trap_at`, the `move_player` arm, the level helper extraction),
  `save.rs`, `tuning.rs`, `tests/`.
- **app-core**: `Mode::TrapDirection`, the inventory `[p]lace` action, the
  direction handler, the `d` gate.
- **gui**: `ALL_MODES` and `needs_status_banner` only. No drawing code — the
  glyph pipeline is generic.
- **assets**: three new `.ron` files, one new `assets/help/` page,
  `assets/items/README.md`.
- **docs**: `CHANGELOG.md`. Not `docs/manual.md`, not the root `README.md`.

## 13. Test intent

Beyond the per-refusal placement tests and the save round trip already
named:

- the catch never exceeds the ceiling, over enough seeded rolls to be
  meaningful;
- the catch is never a boss;
- `carried` is always `None`;
- the countdown spends no `GameRng` — assert the stream is where it was
  after a span of ticks shorter than one period with a honeypot standing;
- the draw order is the documented one, pinned against a seed;
- two honeypots whose periods elapse on the same tick resolve by `(x, y)`
  and not by query order — the assertion is that a second run of the same
  seed produces the same two catches;
- a full downed store leaves the honeypot sprung and moves nothing;
- a honeypot survives a breach taken on populated ground;
- `d` on the surface destroys one and returns nothing;
- an item with no `trap:` def refuses placement;
- the shipped assets clear the research-material reachability census and the
  craftable-value bound.

## 14. Seams this creates

Three writes each, per the `seams` skill, at implementation time:

- **A trap's quality is authored on the item def, never rolled per copy** —
  because an instanced consumable cannot stack in `Inventory`, sit in a
  `Stock` buffer, or be work-ordered.
- **A honeypot clamps rarity before rolling condition** —
  `downed_program_for`'s boss-floor rule, second caller.
- **A honeypot's capture spends no `GameRng` until its period elapses, and
  honeypots roll in `(x, y)` order** — the countdown is what keeps 50 of
  them off the seeded stream, and the sort is what keeps two elapsing
  together from resolving by bevy's unstable query order.
- **A honeypot resolves its def live by id, against `ActiveContract`'s
  precedent** — it is a device standing in the world, like a nest, not an
  agreement already signed.
