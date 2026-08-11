# Companion refactoring — permanent upgrades for tamed programs

*2026-08-11*

## The problem

A tamed program's stats are baked once, at spawn:

```
species.base × zone_mult × depth_mult × rarity_mult × potential_roll
```

(`game/spawning.rs:165-217`). Nothing ever rescales them. Levelling adds a flat
`+12 HP / +1 ATK / +1 DEF` (`tuning.rs:47-49`) and stops dead at
`CREATURE_MAX_LEVEL = 12`. Enemies double every zone (`ZONE_STAT_GROWTH = 2`,
`resources.rs:651`).

So a companion caught in zone 1 is permanently anchored to zone-1 numbers. The
only existing answers are to throw it away and tame a fresh one at the deeper
zone's multiplier, or to fuse it — capped at `MAX_FUSIONS = 3`. Neither is a
decision the player gets to make about *this* program; both are ways of
replacing it.

There is also no signal that this is happening. `ZonePortal` exists and records
the zone a program spawned in, but it is display-only: a tag appended to the
species name by `entity_label` (`game/party.rs:159-165`). Nothing tells a player
in zone 4 that the Scrapper they have carried since the opening ring is four
doublings behind the ground it is standing on.

This is the design half of the parked "zone and depth stat scaling compound"
thread. It addresses the zone axis only.

## What this adds

Two independent, permanent, player-driven upgrade tracks for companions, both
delivered by a new production chain off the Mining Node.

**Recompile Kernel** — raises a companion one zone tier, never above the
player's current zone, multiplying its stat block by `ZONE_STAT_GROWTH`. The
"stay relevant after a breach" lever, and the new bench's automated product.

**Percentage stat buffs** — `+5%` craftable, `+12%` rare drop, one item per
stat. Bounded by a per-companion slot cap. The specialisation lever.

### Why percentages

A `+15 HP` buff is meaningless once a companion has 500 HP, and the whole point
of this feature is that a companion keeps growing across breaches. Percentages
also **commute** with the zone bump — `×1.05` and `×2` multiply in either order
— so a buff bought in zone 1 is worth exactly as much after three breaches as
one bought today, and there is no ordering the player can exploit. That
commutativity is a property to pin with a test, not a happy accident.

The cost of percentages is rounding. `+5%` of a Drone's 3 ATK rounds back to 3,
so a percentage buff would silently do nothing to exactly the weak companions
it exists to rescue. The apply rule therefore floors at `+1`.

### Why a per-companion cap

A Mining Node produces Core Fragments forever and turns are free, so anything
craftable off that chain is an unbounded faucet. The zone bump bounds itself —
never above the player's current zone. The percentage buffs do not, so they take
a slot cap (`MAX_COMPANION_REFACTORS = 5`) in the same spirit as `MAX_FUSIONS`.
The cap also makes the choice interesting: which stat gets the slots.

The two tracks do **not** share the pool. Bumps cost no slots, because a player
who has to spend three of five slots by zone 4 just staying current has had the
feature taken away from them at exactly the point it was supposed to help.

## Naming

`Rarity::label` (`components.rs:898`) already shows players "Optimized" and
"Overclocked" for Silver and Gold programs, so the optimizer/optimization
vocabulary is taken and would collide on screen. This feature uses **refactor**
throughout.

## Design

### The chain

```
Mining Node ──core_fragment──> Annealing Node ──annealed_core──> Refactor Bench
                                (4 cf, 12 ticks)                  (assembles
                                                                  recompile_kernel,
                                                                  3 cores, 20 ticks)
```

One new refiner, one new bench, one new intermediate — the shipped shape, which
is a straight line of single-ingredient recipes with each intermediate matched
1:1 to its bench. Both structures sit behind one new research node. The bench's
`build_cost` names `annealed_core`, so the line that runs the bench is the line
that pays for it.

A bench needs no new mechanism to carry several recipes: `AssembleDef` names one
automated product, but a bench is also `requires_structure` for any number of
hand-crafted recipes. The Armory already automates `hardened_shell` and gates
five more. So the Refactor Bench *assembles* Recompile Kernels on a timer — the
thing you need repeatedly, after every breach — and *gates* the three craftable
percentage buffs.

### The items

| id | effect | source |
|---|---|---|
| `annealed_core` | intermediate | Annealing Node, 4 `core_fragment` |
| `recompile_kernel` | zone bump | Refactor Bench, assembled, 3 `annealed_core` |
| `buffer_extension` | +5% max HP | crafted at the Refactor Bench |
| `inline_cache` | +5% ATK | crafted at the Refactor Bench |
| `bounds_check` | +5% DEF | crafted at the Refactor Bench |
| `paged_arena` | +12% max HP | drop-only |
| `jit_cache` | +12% ATK | drop-only |
| `guard_page` | +12% DEF | drop-only |

Prices must satisfy `no_craftable_item_is_worth_more_than_its_ingredients`.

The three rare items declare no `craftable` and reach the player through the
existing `ItemDef::droppable` table, naming the two boss species at a high
chance and a handful of mid-tier ordinary species at a low one. That covers both
requested sources without a line of new loot code: `award_loot` rolls
`droppable` on every kill, and `grant_nest_cache` makes three passes over the
same table (`game/zone.rs:105-118`). They are not equipment, so they cannot leak
into `surface_boss_loot`'s zone band, which filters on `equipment.is_some()`.

### Schema

New in `items_db.rs`, beside `ConsumeDef`:

```rust
pub struct CompanionUpgradeDef {
    #[serde(default)] pub hp_percent: f32,
    #[serde(default)] pub atk_percent: f32,
    #[serde(default)] pub def_percent: f32,
    #[serde(default)] pub zone_bump: bool,
}
```

`ItemDef` gains `#[serde(default)] pub upgrade: Option<CompanionUpgradeDef>`.
Everything defaulted, so existing mods keep parsing untouched.

The three percent fields must be added to `ItemDef::non_finite_field`
(`items_db.rs:142`). That guard exists so a NaN in a `.ron` rejects the file
rather than poisoning arithmetic downstream, and it currently covers
`taming_potency` and the two `consume` floats.

Magnitudes live in the item `.ron`, like `EquipmentStats` and `taming_potency` —
a new upgrade item is a file, never a code change. Only the *cap* is tuning,
matching `MAX_FUSIONS`.

### Engine

A new component, absence meaning zero exactly as `FusionCount` does:

```rust
pub struct Refactors(pub u32);
```

One entry point, `Game::refactor_companion(target, item)`, in a new
`game/refactor.rs`. Check order follows `install_routine`'s stated rule — the
item is spent **last**, once every refusal has had its chance:

1. not in battle, not game over
2. `target` is `Tamed` by the player
3. the item declares `upgrade`
4. zone bump: `ZonePortal < ZoneLevel`, else "already current for this zone"
5. percent: `Refactors < MAX_COMPANION_REFACTORS`
6. the item is in `Inventory`
7. then take one, apply, increment, log

The two arms share one apply function so they cannot drift:

- **percent** — `new = max(old + 1, (old as f32 * (1.0 + pct/100.0)).round())`.
  The `+1` floor is load-bearing, per the rounding argument above.
- **zone bump** — multiply `max_hp`/`atk`/`def` by `ZONE_STAT_GROWTH`, increment
  `ZonePortal`.
- **both** — raise `hp` by the same delta rather than full-healing. A level-up
  full-heals; a refactor must not, or it becomes a combat item.

An item may set both a percent and `zone_bump`; a mod could. It consumes a slot
if and only if some percent is non-zero.

The wielded-program bonus is computed live off the program's `Stats`
(`Game::wielded_program`), so a refactored program is worth more to wield for
free. Nothing to wire.

### The fusion fix

`fuse_companions` hardcodes `ZonePortal(1)` on the result (`party.rs:679`).
Today that is harmless — the field is a display tag. The moment a bump multiplies
current stats and caps against that field, fuse → bump → fuse becomes an
unbounded stat loop, because a fusion carries the parents' stats forward while
resetting the tier that is supposed to bound them.

The result takes `ZonePortal(max(a, b))` and `Refactors(max(a, b))`, which is the
argument `Rarity` and `FusionCount` already carry four lines above: the fused
stats derive from parents whose numbers already include both, so re-applying
would pay twice, and taking the max stops a fusion laundering a maxed program
back into a fresh one.

### Save

`CreatureSave` gains `refactors: u32`; `zone` is already there.
**`SAVE_FORMAT_VERSION` 26 → 27** — bincode has no field-level compatibility, so
any shape change to `CreatureSave` bumps it. Write side is
`lifecycle.rs:640-724`, where the query is already at bevy's 15-tuple maximum and
uses a nested group for that reason; read side is `lifecycle.rs:400-490`.

### Views and UI

`PetInfo` gains `refactors: u32` — it already carries `fusions` and `rarity`, so
this is the same shape. `ProgramManifest` gains the refactor count *and* the zone
tier: today a player has no signal at all that a zone-1 companion is
structurally behind in zone 4, and the manifest is where that belongs.

One new `PARTY_ROWS` entry, `"Refactor a program"` → pick companion → pick
upgrade → apply. Two pages, mirroring the `Mode::RoutineTarget` install flow.
`surface_only: false`, since this touches no zone-map state through `Position`
and so works underground. Its `available` predicate must require both an owned
pet and at least one upgrade item in cargo, per the group-menu rule that a row
survives only if its first screen would have a row.

**One route in, not two.** Deliberately no `[R]` action on the inventory item
screen; the party menu is the single entry point.

## Out of scope

**Stack depth.** Depth scales enemies `1.35×` per frame on top of zone, and the
bump is capped at the current zone, so it does nothing for a deep dive. That is
the other half of the parked compounding thread, and that thread should start
with measurement rather than being pre-empted by a design decision made here.

**The player.** These upgrade companions only.

## What the gates cannot see

`balance_sim` models companions (`companion_stats`,
`companion_level_for_player_level`) but has no notion of an optional player
investment, so its curve tests will not move and will not gate these magnitudes.
The arena is the instrument: `FERAL_DEV_ARENA=1 cargo run`, or a scenario under
`dev-arenas/`. A green balance suite is not approval of the numbers here.

The five chain and economy census tests *are* real gates on the chain being
shaped right — `no_craftable_item_is_worth_more_than_its_ingredients`,
`every_shipped_assembler_recipe_is_a_single_ingredient`,
`each_bench_is_built_out_of_what_its_own_feeder_makes`,
`no_shipped_assembler_builds_another_benchs_product`, and
`only_the_starters_and_scavenged_gear_need_no_research_or_bench`. If one fails
against the new assets, the asset is wrong.
