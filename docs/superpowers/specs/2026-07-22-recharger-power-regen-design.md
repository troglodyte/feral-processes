# Recharger Node power regeneration; Home as the rest gate

## Problem

The Recharger Node is a pure gate. It costs 5 Core Fragments, does nothing
on its own, and exists only so that pressing `r` is legal within 2 tiles of
it. Nothing about it reads as a *recharger* during play — it never touches
Power (`Needs::hunger`), the stat its name describes.

Meanwhile Home, the structure the whole base clusters around and the first
thing every player builds, is a teleport destination and nothing else. The
one place that should feel safe is mechanically inert.

Swap their roles. The Recharger Node becomes an actual power source that
keeps the player topped up across the base; Home becomes the place you rest.

## Design

### Power regeneration is a structure capability, not a hardcoded ID

`StructureDef` gains a field mirroring `passive_process` in shape:

```rust
/// A structure's power-regeneration capability — see
/// `StructureDef::power_regen`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PowerRegenDef {
    /// Power (`Needs::hunger`) restored per tick while the player is in
    /// range. Stacks additively across every in-range structure.
    pub per_tick: f32,
    /// Chebyshev distance (in tiles) the player must be within, same
    /// box-radius style as `PassiveProcessDef::radius`.
    pub radius: i32,
}
```

```rust
/// If set, this structure restores the player's Power every tick while
/// they stand within `radius` tiles — no assigned worker and no input
/// item, unlike `work` or `passive_process`. `#[serde(default)]` so
/// existing structure files (including mods) without this field keep
/// parsing as before.
#[serde(default)]
pub power_regen: Option<PowerRegenDef>,
```

No Rust code names `"recharger_node"`. Per CLAUDE.md's moddability rule, a
mod grants base power by setting the field.

### The system runs before decay

`systems::power_regen_system` adds `per_tick` to `Needs::hunger` for every
in-range structure that sets it, clamped to 100. It takes the same shape as
`passive_process_system`: a player query for `(&Position, &mut Needs)`, a
structure query for `(&Structure, &Position)`, and `Res<StructureDb>`.

Ordering against `needs_decay_system` is load-bearing and must be explicit —
`add_systems` takes an unordered tuple today, so the two are chained:

```rust
(systems::power_regen_system, systems::needs_decay_system).chain(),
```

Regen first. If decay ran first, a player limping into the base at 0.1 Power
would be driven to 0, take a point of starvation damage and trip the "Your
power reserves are critical!" log on the very tick the Recharger was about
to cover them. Running regen first means arriving in range stops the bleeding
immediately, which is the whole point of the structure.

Multiple in-range structures stack additively, matching `raid_defense` and
`inventory_bonus`. Since the result clamps at 100, stacking only buys a
faster refill.

The schedule block is currently duplicated verbatim between `Game::new`
(lib.rs:550) and `Game::load` (lib.rs:663). Adding an ordering constraint to
two copies is exactly where that duplication bites, so it is extracted into
one private `build_schedule() -> Schedule` used by both. This is the only
refactor in scope.

### Rest moves to Home

`home.ron` gains `enables_rest: Some((radius: 15))`. This needs **no engine
code**: `Game::rest` already calls `nearby_rest_structure`, which checks the
radius of any deployed structure whose def sets `enables_rest`.

`recharger_node.ron` drops `enables_rest` entirely and gains
`power_regen: Some((per_tick: 1.0, radius: 15))`. The two structures keep
doing visibly different things — one is where you sleep, one is what keeps
your reserves up while you work.

### Radii are base-scale, and approximate

Both radii are 15, matching `MAX_BUILD_DISTANCE_FROM_HOME`.

For Home this is exact: every structure must be built within 15 tiles of
Home, so "within 15 tiles of Home" *is* the base footprint.

For the Recharger it is an approximation, because a radius is measured from
the structure that owns it, not from Home. A Recharger at the base edge
covers the whole base plus roughly 15 tiles past the boundary on that side;
one near Home covers about exactly the base. Making it exact would require
teaching the schema to measure from Home, which special-cases one structure
id into a moddable data format to buy very little. Accepted as designed.

### Numbers

| Quantity | Value | Consequence |
|---|---|---|
| `per_tick` | 1.0 | Net +0.85/tick against `HUNGER_DECAY_PER_TICK` (0.15) |
| `radius` | 15 | Base-scale, per above |
| Recharger build cost | 10 Core Fragments (was 5) | Infinite base power is worth more than a rest gate |

At +0.85 net, empty→full is ~118 ticks and half→full is ~59. A rest cycle
(`REST_TICKS` = 40) taken inside the base now also recovers ~34 Power
alongside the full Fatigue and Integrity restore. Every one of these is a
single number in a single `.ron` file, so play-testing can retune without
touching Rust.

## Consequences

- **Power Cells lose their daily role at home.** The Terminal (3 CF,
  CoreFragment → PowerCell) and Power Conduit (14 CF) become expedition prep
  rather than standing infrastructure. Cells still matter in the field and
  for Home's 4-cell teleport. Their costs are unchanged; this is the intended
  shift, not an oversight.
- **Rest gets easier to reach.** It moves from a dedicated 5-fragment build
  to a structure every player already has, and from a 2-tile spot to the
  whole base. Rest is no longer a thing you go build for.
- **Existing saves need no migration.** Saves store structure kinds and the
  `StructureDb` is reloaded from assets at load time, so a deployed Recharger
  silently changes behavior. Nobody is locked out of resting, since Home must
  be built before anything else.
- **A save's deployed Recharger becomes a better deal than it was bought
  for.** It was paid for at 5; it now regenerates power. Not worth a
  migration to reconcile.
- **The Low Power Mode perk gets weaker at home and is unchanged in the
  field.** It scales decay, which regen now swamps inside the base.

## Implementation surface

- `crates/engine/src/structures.rs` — `PowerRegenDef`, `StructureDef::power_regen`.
- `crates/engine/src/systems.rs` — `power_regen_system`.
- `crates/engine/src/lib.rs` — extract `build_schedule()`; register the
  chained pair in it; add a `power_regen` clause to `structure_description`
  formatting the def's own radius, e.g. `recharges Power within 15 tiles`;
  update rest tests that assume the Recharger is the gate.
- `assets/structures/recharger_node.ron` — cost 10, drop `enables_rest`, add
  `power_regen`.
- `assets/structures/home.ron` — add `enables_rest: Some((radius: 15))`.
- `assets/structures/README.md` — document `power_regen`; the `enables_rest`
  comment stops citing the Recharger Node as its example and cites Home.
- `README.md` — Power row in the Stats table; Recharger Node and Home rows in
  the structures table; the "rest gate" structure-category prose; the `r`
  keybind row; the rest-related lines under Zones/Base defense.
- `CHANGELOG.md` — entry covering the role swap, the cost change, and the
  fact that old saves change behavior without migrating.

Renderer help text needs no change: the TUI and GUI help strings say
`r recharge` and describe what rest restores, but never name the Recharger
Node as the gate.

## Testing

`systems.rs` unit tests:

- an in-range structure adds exactly `per_tick`
- regen clamps at 100 and never overshoots
- an out-of-range structure is a no-op, at radius + 1 on each axis
- two in-range structures stack additively
- a structure whose def sets no `power_regen` is a no-op

`lib.rs` engine tests:

- ticking inside a base with a Recharger nets Power upward from a drained
  start; ticking the same number of ticks far from base nets it downward —
  regen and decay composed, in schedule order
- arriving at 0.1 Power takes no starvation damage on the tick regen covers
  it (the ordering constraint, asserted directly)
- rest works within 15 tiles of Home
- rest is a no-op with no Home in range
- `recharger_node.ron` loads with `power_regen` set, `enables_rest` unset,
  and a build cost of 10
- `home.ron` loads with `enables_rest` at radius 15
- `structure_description("recharger_node")` mentions recharging Power

The existing rest tests lean on a `spawn_recharger_node_at_player` helper
(lib.rs:7294); they are repointed at Home via the existing `place_home`
helper (lib.rs:5388).

Full `cargo test --workspace` is the gate.
