# Selling programs to a trader

## Problem

There is no way to get rid of a tamed program.

`Game::remove_companion` reads like one but is not: it stands a program down
from the active battle party and leaves it tamed, owned, and still counting
against the roster. The only path that actually reduces the count is
`fuse_companions`, which consumes two to make one — so it needs a second
program you are also willing to give up, and it refuses once either parent
hits `MAX_FUSIONS`.

That matters because ownership is capped. `pet_capacity()` is
`BASE_PET_CAPACITY` (3) plus each deployed structure's `pet_slot_bonus`, and
decompiling refuses outright at the cap — the battle screen greys the action
with `roster is full`, and the log says:

> Your roster is full (3/3) — fuse two programs together or deploy a Data
> Cache to make room.

Both of those outs cost something and neither is "I don't want this one." A
player holding three programs they have outgrown has no way to trade any of
them for a better catch.

## Approach

Sell a program to a trading structure for Core Fragments, at a tenth of its
power.

`Stats::power()` already exists — `max_hp + atk + def`, used for difficulty
colouring — and `PetInfo::power` already surfaces it, so the valuation needs
no new metric and inherits level growth and fusion gains for free.

Rejected along the way:

- **A plain "release" on the pet screen**, no trader involved. Simpler, but
  it makes shedding a program free and instant, which turns the roster cap
  into a non-constraint. Routing it through a trader keeps the cap meaningful
  and gives the act a price attached to what you are giving up.
- **Paying `TradeDef::sell_rate` per program, like an item.** One uniform
  price for a level-1 Glitch and a thrice-fused Sentinel makes the sale
  either free money or an insult. Power scaling is the point.
- **Extending `sell_item` to take an entity.** It is built around an
  `ItemId`, a stack count, and a quantity prompt; a program is a single
  unique entity with no quantity. Sharing the function would mean two
  branches sharing nothing but a name.

## Design

### The trader decides whether it buys programs, and at what rate

`TradeDef` gains one field:

```rust
/// Divisor applied to a program's `Stats::power()` to price it when sold
/// here. `None` — the default — means this trader deals in items only.
#[serde(default)]
pub program_sell_divisor: Option<u32>,
```

`assets/structures/black_market.ron` sets `program_sell_divisor: Some(10)`,
which is the "a tenth of its power" the feature was asked for.

A field on the structure rather than a constant in Rust, following
`power_regen`: no engine code names a structure id, a modded trader can pay
better or refuse programs entirely, and `#[serde(default)]` keeps every
existing structure file — including mods — parsing untouched. `None` as the
default is the conservative choice: a mod's trader does not silently start
buying creatures because the game gained the ability.

### Price

```
payout = max(1, power / divisor)      // integer division, truncating
```

The floor of 1 exists so a sale can never pay nothing while still destroying
the program — that reads as a bug regardless of how weak the creature was. A
`divisor` of 0 is treated as "does not buy programs", the same as `None`,
rather than dividing by zero.

At divisor 10, a Glitch tamed in the starting zone (36 + 3 + 1 = 40 power)
sells for 4 Fragments, and a heavily levelled fusion in the hundreds sells
for tens. For scale: `black_market.ron` pays `sell_rate: 1` per item, stocks
an ICE Breaker at 4, and costs 16 to deploy. So the weakest realistic program
is worth four items or exactly one ICE Breaker — enough to be worth doing,
nowhere near enough to farm.

The floor of 1 therefore almost never fires in shipped content; it is there
for a modded species with single-digit stats, not for the base game.

### The engine owns the rows

Two additions to `Game`, following the same verbatim-render contract as
`battle_action_options`:

```rust
/// One sellable program at `structure`, priced. Renderers draw these and
/// never compute a price of their own.
pub struct ProgramSaleOption {
    pub entity: Entity,
    /// e.g. "Sparkgrub"
    pub name: String,
    pub level: u32,
    pub power: i32,
    pub payout: u32,
    /// What this sale would also cancel, already worded for display — e.g.
    /// "leaves your battle party", "stops working the Mining Node". Empty
    /// when the program is idle.
    pub detaches: Vec<String>,
}

impl Game {
    /// Every program the player could sell at `structure`, or empty if it
    /// does not buy them.
    pub fn program_sale_options(&mut self, structure: Entity) -> Vec<ProgramSaleOption>;

    /// Sells `creature` at `structure`. Despawns it.
    pub fn sell_companion(&mut self, structure: Entity, creature: Entity) -> Result<(), String>;
}
```

`detaches` is built in the engine rather than assembled by the renderer
because it has to name a structure (`entity_label`) and know what `Task`
means — neither of which a renderer should be reaching for.

### `sell_companion`'s order of operations

The sequence matters, because the creature's destruction is irreversible and
one of the checks can fail:

1. Refuse if the game is over or a battle is active, as `sell_item` does.
2. Refuse if `structure` has no `TradeDef`, or its `program_sell_divisor` is
   `None`/`Some(0)`.
3. Refuse if `creature` has no `Tamed`, or its owner is not the player.
4. Compute `payout`, then `check_room(&currency, payout)?`.
5. Only now: detach, log, despawn, credit, `tick()`.

Step 4 before step 5 is the same reasoning `sell_item` documents about its
own ordering — the Fragments have a bank limit, and taking the creature
before discovering there is no room for the payment would destroy it for
nothing.

### Auto-detach

Selling cancels whatever the program was doing, because being asked to go
undo it first is friction after you have already confirmed a sale.

Both a cronjob and a guard post are the same `Task` component
(`TaskKind::GatherResource` / `Guard`), and party membership is the `Party`
resource, so detaching is `remove::<Task>()` plus a `retain` — the same two
operations `assign_guard` already performs when it takes a program out of the
party.

Each detach logs what it cost, so a structure that stops producing says so:

> Sparkgrub stops working the Mining Node.

### Confirmation

**This part is my call, not yours — you asked for the spec before answering
it, so it is the one thing here to push back on.**

Selling takes a confirmation step, mirroring `Mode::RemoveConfirm` (the
demolish-a-structure flow). Two reasons: the sale permanently destroys a
levelled creature, and — because detaching is automatic — the confirmation is
the only place that can tell the player what *else* the sale takes down
before it happens. `ProgramSaleOption::detaches` exists to be shown there.

The cheap alternative is one keypress straight from the row, matching item
sales. If you would rather have that, delete the confirm mode and show
`detaches` on the row itself; nothing else in this design changes.

### Screen flow

The trade screen (`Mode::Trade`, `t` at a Market) grows a second section.
Items keep their existing `Mode::TradeAction` → `Mode::TradeQuantity` path;
a program needs no quantity, so it goes straight to a new confirm mode:

```
Mode::Trade ── pick an item ──→ TradeAction ──→ TradeQuantity
            └─ pick a program ─→ TradeProgramConfirm ──→ Trade
```

Adding a `Mode` variant is now a compile error until it is classified in
`Mode::is_battle`'s exhaustive match, which is exactly what that match is
for.

### Save format

Unchanged. Selling despawns an entity and moves Fragments; nothing new is
persisted.

## Testing

Engine, all pure and seeded:

- Price: `power / divisor` truncating; the floor of 1 for a very weak
  program; `Some(0)` and `None` both refuse.
- Ordering: with the Fragment bank full, the sale refuses **and the creature
  still exists**. This is the test that matters most — it is the one bug the
  step ordering exists to prevent.
- Refusals: a structure that does not trade, a trader that does not buy
  programs, an untamed creature, a program owned by someone else.
- Auto-detach: a cronjob worker, a guard, and a party member each sell
  cleanly, leaving no `Task`, no `Party` entry, and a log line naming what
  stopped.
- The roster: selling frees a slot, so decompiling at a full roster succeeds
  immediately afterward. This is the whole point of the feature and deserves
  an end-to-end test.
- `program_sale_options` is empty for a non-buying trader and priced for a
  buying one.

App-core: picking a program row reaches the confirm mode; confirming sells;
cancelling leaves the program alive and returns to `Mode::Trade`.

Plus the standing gate: `cargo test --workspace` (449 at time of writing),
`cargo clippy --workspace --all-targets` clean, `cargo fmt`.

## Documentation

- `assets/structures/README.md` — the new `program_sell_divisor` field, per
  the rule that a schema change updates the schema reference in the same
  change.
- `README.md` — the trade screen's new section, and the roster cap now having
  a third escape.
- **`crates/engine/src/lib.rs:3735`** — the roster-full refusal says "fuse two
  programs together or deploy a Data Cache to make room" and becomes
  incomplete the moment this ships. Selling is the third out and the most
  direct one.
- `CHANGELOG.md` — under `## Unreleased`.

## Not doing

- Buying programs from a trader. The complaint was about shedding them.
- A price that accounts for `Potential` (individual quality) or `FusionCount`
  separately. Both already move `Stats`, so power picks them up; weighting
  them again would be double-counting.
- Selling the player's last program, or any "are you sure, this is your only
  one" special case. If a player wants an empty roster that is their call.
- Selling from anywhere but a trading structure. Keeping the cap meaningful
  is the reason the trader is involved at all — and a Market is player-built
  (16 Core Fragments, `black_market.ron`), so this is a thing you set up at
  your base rather than a landmark you have to find.
