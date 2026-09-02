# The base power grid

**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header. See `../../INDEX.md`.

> `INDEX.md` warns that this header is the one line in a spec nobody ever
> revises. Answer "did this ship" from `CHANGELOG.md` and a grep, never from
> here.

Closes the `TODO.md` item reading "base needs power to run, structures
consume power, and power rechargers produce power. for now power rechargers
can be anywhere in the base, no proximity. requires more power rechargers
for more buildings." Cited by its text rather than its number, following
`2026-08-17-power-replaces-fatigue-design.md` — the numbering has shifted
once already.

That spec put this one out of scope in as many words, and gave the reason:
the grid "will want the Recharger's role reconsidered, which this change
deliberately leaves alone. Sequencing it after means the Recharger is
settled once rather than twice." This is that sequel, and the Recharger's
role is settled here.

## The problem

A base has no reason not to sprawl. Every structure is bounded by its build
cost and by `MAX_BUILD_DISTANCE_FROM_HOME`, both of which are paid once; a
structure that exists costs nothing to keep. So the optimal base is every
machine you can afford, laid out in whatever order they were unlocked, and
the only ongoing decision a base asks of the player is where to post
programs.

The Recharger Node makes this sharpest. It is base-wide already — 10 core
fragments buys `power_regen: (per_tick: 1.0, radius: 10)` for the whole
slab, forever — so there has never been a reason to build a second one. It
is a one-time purchase that deletes a need meter, and then it is furniture.

The grid gives both of those a cost that recurs: a machine consumes supply
for as long as it stands, and the Recharger is the thing that supplies it.
"Requires more power rechargers for more buildings" is exactly the sentence
that turns a one-time purchase into a scaling one.

## Two Powers, and only one of them is this

The word is already spent, twice, and neither use moves.

- **`PowerReserve`** is a per-creature ability pool, `0.0..POWER_MAX`, spent
  on routines and priced through `abilities::routine_power_cost`. It is one
  entity's own state.
- **The Recharger Node's `power_regen`** trickles that reserve back while
  the party stands on the base, and **Power Cells** (`consume: (power:
  25.0)`) restore it from the inventory.

The grid is neither. It is a **base-level capacity**: a number the base
supplies and a number its machines demand, compared. It never touches a
`PowerReserve`, no Recharger output is split between the two, and no machine
consumes a Power Cell. The two systems share a fiction and nothing else.

**Player-facing, this one is called the Grid, not Power.** The fiction
already supports it — the opening line is "You materialize at the edge of
the Grid" and the Power Cell "is the staple of staying on the Grid" — and
the alternative is a base pane reading `Power 6 / 8` two panels away from a
status column reading `Power 43/50`, meaning something unrelated. This is a
deliberate change from the design as first presented in chat, where both
were called Power; the collision is the reason.

## The model: supply against draw, recomputed every tick

Nothing is stored and nothing accumulates. Every tick:

```
supply = Σ power_supply over deployed structures
draw   = Σ power_draw   over deployed structures
```

If `draw <= supply` the base runs exactly as it does today. If `draw >
supply`, machines are cut until it fits:

```
sort machines by (x, y)
budget = supply
for m in sorted:
    if budget >= m.draw  { budget -= m.draw }   // runs
    else                 { m is dark }          // Unpowered
```

`(x, y)` is the sort `assembler_system` already uses, and for the same
reason: bevy's query iteration order is not stable, so two machines
competing for the last unit of supply would resolve differently between
runs. It is arbitrary but never changes — the player learns that the far
corner drops first and lays out around it.

Note the loop does **not** stop at the first machine that doesn't fit. A
3-draw machine that can't fit in a 2-unit budget goes dark, and a 1-draw
machine after it still runs. Stopping at the first failure would make one
big machine dark an arbitrary tail behind it.

### A machine draws whether or not anyone is posted to it

The building is plugged in; the worker is a separate question. Three
reasons, in order of weight:

1. It is what the TODO asks for. "Requires more power rechargers for more
   buildings" is a claim about buildings.
2. Drawing only when staffed makes the ledger depend on
   `schedule_base_labour`'s output, which runs in the same tick and reassigns
   bodies by priority. A machine going dark could free a body, which changes
   the assignment, which changes the ledger. There is no reason to open that
   loop.
3. It is the only version the player can plan against. Supply covers the
   base you built, not the base you happen to have staffed this minute.

The cost is that an idle machine you keep around "for later" is a real
expense. That is the intended pressure — demolish it or power it.

## The ledger is one pure function

```rust
// crates/engine/src/game/base/power.rs
pub(crate) struct PowerLedger {
    pub supply: u32,
    pub draw: u32,
    pub dark: HashSet<Entity>,
}

pub(crate) fn ledger(world: &World, db: &StructureDb) -> PowerLedger
```

Two callers, which is the whole point of it being a function rather than a
rule written twice:

- **`power_grid_system`**, new, which stores the result in
  `resources::PowerGrid`.
- **`Game::base_power() -> (u32, u32)`**, returning `(draw, supply)`, which
  the base pane reads.

`PowerLedger` itself stays engine-internal — `dark` holds `Entity`, and the
renderer has no business with those. The per-machine half already has a
public road: `EntityView::machine_status` reads `Unpowered` like any other
status, so the gui learns which machine is dark the same way it learns which
is clogged.

A **damaged** structure — one with `Durability` below its maximum but still
standing — draws and supplies exactly as an undamaged one does. Damage is
already its own axis with its own repair loop, and making a half-wrecked
Recharger supply half a grid would couple two systems for no gain. It stops
supplying when it is destroyed, which both destruction paths already handle
by despawning it.

This is the `battle::attackers_in_group` pattern named in `CLAUDE.md`: a
doc comment claiming to mirror another module's formula must be a call, not
a copy. A renderer that re-derives "is this machine dark" from supply and
draw is the copy that drifts.

`resources::PowerGrid` is a per-tick derived cache and is **not saved** — it
holds nothing a reload cannot recompute from the structures themselves,
which is the same argument that keeps `resources::RunFeats` out of the save.
The view calls `ledger` directly rather than reading the resource, so the
base pane is correct on the first frame after a load, before any tick has
run.

## `MachineStatus::Unpowered`, and why a sixth variant is allowed here

`views.rs` records, at the `output_stranded` field, exactly the argument
that would refuse this:

> Deliberately *not* a sixth `MachineStatus` — that enum is one machine's
> own state, and this is a fact about every depot at once, so folding it in
> would stop the enum meaning one thing and force a precedence call against
> all five existing variants.

Power passes that test where `output_stranded` failed it. Under an `(x, y)`
cut order, machine A runs while machine B two tiles over is dark: the
shortfall is base-wide, but **which machine is cut is that machine's own
state**, and there is no way to say it about the base as a whole. The
precedence call the comment warns about is answered below rather than
dodged.

**`Unpowered` takes top precedence over all five existing variants.**
Nothing else the player can do makes a dark machine run — posting a program,
clearing a clog, feeding an input, building a depot, clearing a route are
all wasted moves. It is the only status whose fix is "build a Recharger", so
it must be the one shown.

## Where the status is written

`idle_machine_system` becomes the single writer of `Unpowered`. It already
makes one pass over **every** `Structure` and already runs first in the
chained base group, which is why it is the right site: precedence against
`Idle` gets stated once, in the one place that can see both facts, instead
of being spread across three systems that would each have to agree.

```
(
    systems::power_grid_system,       // new, first: computes the ledger
    systems::idle_machine_system,     // writes Unpowered, then Idle
    systems::task_progress_system,    // guard: skip a dark machine
    systems::player_gather_system,    // guard: skip a dark machine
    systems::assembler_system,        // guard: skip a dark machine
    hauling::haul_step_system,
).chain()
```

`task_progress_system` and `assembler_system` each get a one-line `continue`
guard so a dark machine makes no progress. **A third guard belongs on
`player_gather_system` as well**, added during implementation once the two
above turned out not to be the whole surface: it is the player hand-working a
node directly, and without a guard a player standing at a dark machine could
still call `deliver_payout` and pull a real unit of production out of it —
the mechanic working exactly as before, just with the pane's status lying
about it. Worse, `deliver_payout` writes `MachineStatus::Running` on its own
tick, which would flip a machine `idle_machine_system` had just set to
`Unpowered` right back, every single tick, reopening the very twice-per-tick
transition log a last-in-chain power system was refused specifically to
avoid — just through a third door instead of the one that design refused.
So it is one writer plus **three** guards, not two. None of them write the
status — that stays `idle_machine_system`'s alone.

**The alternative was refused:** a power system running *last* and
overwriting whatever the others decided. `set_machine_status` logs only on
transition, so within a single tick `task_progress_system` would set
`Running` (logging "The Lathe resumes.") and the power system would set
`Unpowered` (logging that it is dark) — every tick, forever, for as long as
the base is short. Running the ledger first is what keeps the log quiet.

The log line follows the existing table's shape:

```
MachineStatus::Unpowered => format!("The {name} is dark — the grid can't power it."),
```

## The schema: two fields on `StructureDef`

Both `#[serde(default)]`, so every existing structure file — including any
mod written before today — keeps parsing, as a building that neither draws
nor supplies.

```rust
/// What this structure needs from the base's grid to run. Summed against
/// `power_supply` every tick; see `game::base::power`. A machine whose
/// draw doesn't fit the base's remaining supply reports
/// `MachineStatus::Unpowered` and makes no progress.
#[serde(default)]
pub power_draw: u32,

/// What this structure contributes to the base's grid. Independent of
/// `power_regen`, which restores a creature's `PowerReserve` and is a
/// different resource entirely.
#[serde(default)]
pub power_supply: u32,
```

`power_supply` is a separate field from `power_regen` rather than a third
member of `PowerRegenDef`, because the two answer different questions and
one structure is about to have both with different values. Folding them
together would also mean a mod granting grid supply is forced to grant party
regen with it.

`assets/structures/README.md` gains both fields in the same change — the
standing rule for any schema change.

## Which structures draw, and the numbers

Only structures declaring `work` or `assembles` — **15 of the 25 shipped
structures**. The other ten are free: Home, the Recharger Node, the Depot,
the Shield, the Data Cache, the Heap Pillar, the Patch Node, the Portal, the
Black Market and the Contract Broker. Storage, defence and a shopfront are
not what "more buildings" means, and taxing them would charge the player for
defending a base rather than for running one.

| Structure | | |
| --- | --- | --- |
| Home | `power_supply: 4` | the baseline |
| Recharger Node | `power_supply: 4` | each one built |
| Mining Node, Log Scraper, Research Node, Power Conduit | `power_draw: 1` | extractors |
| Lathe, Transcriber, Winding Node, Refinery, Disk Press, Annealing Node | `power_draw: 2` | base assemblers |
| Fabricator, Compiler, Armory, Assembly Bay, Refactor Bench | `power_draw: 3` | late assemblers |

**The Home baseline cannot be gamed or lost.** `building.rs:27` refuses a
second Home outright ("A Home is already deployed."), and `raidable: false`
means a GC Entropy Sweep cannot take it down. So the 4 is singular by
construction — there is no cheaper-per-supply exploit in building Homes at 5
fragments against Rechargers at 10.

At 4, a fresh base runs four extractors free and the first real production
chain costs a Recharger. `max_deployed` is unset on the Recharger, and
`building.rs:64` treats 0 as unlimited, so "more rechargers for more
buildings" has no ceiling to hit.

**These numbers are unmeasured and are a starting point for play, not a
claim.** `balance_sim` has no base-production term at all — it models
battles — so it gates none of this, exactly as it gates none of the Power
economy. The instrument is a session on `--template chains`, which opens on
a running base.

### The templates will load dark

Three of the six will load short, and each already stands one Recharger:

| template | draw | supply | short by |
| --- | --- | --- | --- |
| `chains` | 15 | 8 | 7 |
| `contracts` | 15 | 8 | 7 |
| `deep-lair` | 17 | 8 | 9 |
| `extraction`, `rarity-preview`, `stack` | 6 | 8 | — |

That is the mechanic working rather than a bug, but it makes those three
worse at their job, which is opening on a state you can test *from*. They
are repaired by adding Recharger entries to the checked-in `.ron` directly
— no re-capture and no play needed, since a template is a save file in RON.

The gate is already written: `dev_template.rs`'s
`the_chains_template_starts_with_a_chain_that_actually_runs` fails the
moment the numbers land, which is why authoring the numbers and repairing
the templates have to be one commit rather than two.

## What the player sees

- A **`Grid  6 / 8`** line in the base pane header, red when short. Read
  from `Game::base_power`.
- A dark machine reads `Unpowered` in the building pane —
  `render/building.rs`'s status table gains an arm — and draws red on the
  map, via `render/base.rs`'s status colour table. Red is right: it is the
  same class as `Clogged` and `Stranded`, a stall the player must act on,
  not `Yellow`'s "will resolve itself".
- One log line on transition, from `set_machine_status`, as above.

**Building is not refused when you cannot power it.** One rule instead of
two, and the machine says so itself the moment it is placed. A build-time
refusal would also have to explain which *other* machine the new one would
darken, which the `(x, y)` order makes non-obvious.

## Save format

No `SAVE_FORMAT_VERSION` bump.

- Both new `StructureDef` fields are additive and behind `#[serde(default)]`
  — they are asset schema, not save schema, and `assets/*` is not saved.
- `resources::PowerGrid` is derived per tick and not saved.
- `MachineStatus` gains a variant, and that costs nothing because
  `MachineStatus` is never saved at all: it derives `Component, Clone, Copy,
  Debug, Default, PartialEq, Eq` (`components.rs:635`) — no serde traits —
  and appears nowhere in `save.rs`. It initialises to `Running` and is
  corrected on the first tick after load, so a save from before this change
  loads exactly as it always has and that first tick decides which of its
  machines are dark.

This is the additive-behind-`serde(default)` case the save-format seam
covers explicitly. Nothing is removed and nothing changes meaning under a
name it keeps.

## Testing

The failing test first, in each case.

**The ledger** (`game::base::power`, unit):

- Supply sums across a Home and two Rechargers; draw sums across mixed
  machines.
- A base exactly at capacity has nothing dark — the boundary is `<=`, not
  `<`.
- The cut order is `(x, y)`, asserted with the competing machines **spawned
  in the opposite order to their positions**, the way
  `assembler_system`'s sort test does it, or the test passes against an
  unsorted implementation.
- A 3-draw machine that doesn't fit the remaining budget goes dark and a
  1-draw machine after it still runs — the loop does not stop at the first
  failure.
- A passive structure (Depot, Shield) contributes nothing to draw.

**The systems:**

- A dark machine makes no `Task::progress` — through
  `task_progress_system` and through `assembler_system` separately, since
  they are two guards.
- A dark **and** unstaffed machine reports `Unpowered`, not `Idle`. This is
  the precedence test, and it must be written so that it fails against an
  implementation that writes `Idle` second.
- Entering and leaving the dark state logs exactly once each, not once per
  tick. The regression this guards is the refused last-in-chain design.
- Delete the guard and watch the progress test fail. `CLAUDE.md`'s standing
  rule about vacuous tests applies with force here, because a machine with
  no worker makes no progress anyway — the fixture must have a **posted
  worker** or the test passes with the whole feature removed.

**Over the real assets** (`tests/assets.rs`):

- Every shipped structure declaring `work` or `assembles` declares a
  non-zero `power_draw`. This is the census that stops the sixteenth machine
  shipping free.
- Home's `power_supply` is at least the combined draw of a Mining Node, a
  Log Scraper and a Research Node — the three a new base stands up first.
  Stated as a concrete sum rather than as "covers the opening", which is not
  a thing a test can check.
- Every shipped `power_supply` is on a structure that is not a machine, so
  supply and draw never both sit on one building. Reported rather than
  enforced would be wrong here: it is a one-line check and the corner is
  genuinely incoherent.

**Save:** a round trip of a base over capacity loads and comes back dark
rather than erroring.

## Documentation obligations

Every one of these lands in the same change, not after it.

- `assets/structures/README.md` — both new fields, since it is the schema
  reference a modder reads.
- `docs/structures-gen.py` — a **hand transcription**, not a parser. It gains
  two columns, `draw` and `supply`, both `0` on the ten structures that have
  neither.

  Folding the numbers into the existing free-form effect string was the first
  plan and is wrong: that column is `makes / does`, and for a producer or an
  assembler it holds the **item id**, which the script reads back to draw the
  production-line diagram. Appending "draws 2" to it would corrupt the
  diagram. The effect string is only free-form on the utility rows.

  Regenerate `docs/structures.md` from it in the same change.
- `CHANGELOG.md` and the workspace version, at the merge, per the
  one-release-per-change rule.
- `CLAUDE.md` and `docs/seams.md` gain the matching pair of entries: the
  rule in one, the argument in the other, under the same title. The seams
  worth stating are the single-writer/two-guards split, why `Unpowered` was
  allowed a sixth variant where `output_stranded` was refused one, and that
  the ledger is one pure function with two callers.
- `docs/manual.md` and the root `README.md` are carved out and stay stale.

## Out of scope

- **Proximity, wiring or radius.** The TODO says so in as many words: "for
  now power rechargers can be anywhere in the base, no proximity."
- **Stored power.** No accumulator, no buffer, no brownout that recovers on
  its own. Supply against draw is the whole model, and adding storage later
  is a change to one function.
- **Tier-scaled draw.** A structure's `power_draw` is flat, and an upgraded
  machine draws what a fresh one does. It is an obvious knob and it should
  wait until the flat version has been played.
- **Any coupling to `PowerReserve`.** Named again here because it is the
  change someone will reach for on the strength of the shared word.
- **A player-set priority order.** The `(x, y)` order is deterministic and
  needs no new saved state; a priority field is a UI flow, a save field and
  a lot of clicks for a base of fifteen buildings. Worth revisiting only if
  play says the arbitrary order is genuinely painful.
- **Refusing a build you cannot power**, as argued above.
