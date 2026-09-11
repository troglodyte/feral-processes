# Research: a project the base works, not points it banks

## Context

Research today is a **purchase**. `assets/structures/research_node.ron`
declares `work: (produces: "research_data", ticks_per_unit: 14)`;
`research_data` is the game's only `banked: true` item, so
`systems::deliver_payout` routes its payout past every `Stock` buffer and
straight into the player's `Inventory`. A program posted at a Research Node
fills a pool. When the pool is deep enough, `Game::unlock_research` spends
`def.cost` out of it plus `def.materials` off the pack and the adjacent
shelves, all-or-nothing, and the node unlocks.

Three things fall out of that shape and none of them is good:

- **The base is not involved.** A Research Node is reachable only through
  `set_standing_job` — a hand-posted program — because `work_orders::
  chain_break` refuses every banked item by name and so no work order can
  ever ask for Research Data. Nothing about the research tree reaches the
  scheduler.
- **The materials are the player's errand.** `Game::research_material_held`
  is `Inventory::count` plus `adjacent_stock_count`: the player has to be
  standing next to the goods. A base that could *make* a Logic Wafer offers
  no help getting one.
- **There is no goal.** The pool is fungible, so nothing in the game knows
  what the player is working towards, and nothing can be blocked, warned
  about, or worked on.

This replaces the purchase with a **project**: the player picks one node,
the base works it, and the materials it needs become ordinary work orders.

## Decisions taken

Settled in brainstorming on 2026-09-11; recorded so they are not
relitigated.

| Question | Decision |
|---|---|
| What becomes of Research Data? | Progress, not a bank. Research Nodes still produce it; it flows into the active project. `cost` becomes how much research work the node takes. |
| A material the base cannot make? | **Refuse the selection outright**, naming the missing link. Nothing is queued, no project starts. |
| The material orders | Ordinary `WorkOrder`s filed at `OrderPriority::High`, visible and cancellable, withdrawn when the project ends. |
| Completion | Both gates: `progress >= cost` **and** the base holds the whole bill. The bill is consumed at the end, in one go. |
| Switching projects | Progress is kept **per node**, so abandoning and returning is not destructive. |
| The orders after filing | Filed **once**, at selection. Not topped up per tick. |
| Who pays the bill | The **base**, from `base_holding`. Not the player's pack. |
| `cost:` values | Left as authored. Pacing is a playtest question, not a spec one. |

## The shape

### `resources::ActiveResearch`

```rust
#[derive(Resource, Default, Serialize, Deserialize)]
pub struct ActiveResearch {
    pub id: Option<ResearchId>,
    /// Research Data delivered per node — every node, not just the active
    /// one. Switching projects is not destructive.
    pub progress: HashMap<ResearchId, u32>,
}
```

`resources::Research` — the unlocked set — is untouched, so every save keeps
everything it has already paid for. Both new fields are additive behind
`#[serde(default)]` on field-named RON, so this costs **no
`SAVE_FORMAT_VERSION` bump**. `SaveData` gains `active_research:
Option<ResearchId>` and `research_progress: Vec<(ResearchId, u32)>`, the
second **sorted on write** for the reason `SaveData::researched` is sorted:
the encoded bytes must not differ run to run.

### Where the work comes from

**The active project supplies a want to `schedule_base_labour`, the way an
order does.** `Game::research_wants` returns the deployed Research Nodes
(`work_orders::producers_of` against the research currency, which is already
sorted by tile) while a project is active, and nothing at all when none is.
It returns `Vec<(Entity, TaskKind)>` with `TaskKind::GatherResource` — the
shape `standing_wants` and `build_wants` return, not the `(Entity, u32)`
depth pairs `settle_orders` returns — because a Research Node is the top of
its own line and has no recipe tree behind it to measure a depth against.

Placement in the ladder: research wants are **prepended to the order block**
— behind build wants, which are prepended above everything, and ahead of the
standing queue and of dig wants, which are appended last. The player picked
this node explicitly; it outranks the queue they filed and forgot.

What this buys, and the reason it is the shape chosen:

- A base with no project selected staffs no Research Node, so those nodes
  read `MachineStatus::Idle` — *no program assigned*, which is exactly
  true. **No new `MachineStatus` variant, and the seam "a banked resource
  can never clog, so a Research Node has no full state" survives untouched.**
- A hand-posted `StandingJob` on a Research Node stays legal and still
  works; it simply is no longer the only way one gets staffed.

### Delivery

`systems::deliver_payout` grows one branch **ahead of** its banked branch: a
payout of `ItemDb::research_currency` lands in
`ActiveResearch::progress[id]`, saturating at the active node's `cost`.

With no active project it lands nowhere and returns 0 — the existing
"landed nothing" path, already what a full `Stock` does. That is the only
route by which a hand-posted program at a Research Node can waste a cycle,
and it is silent by design: the node is `Idle` in every other case.

The bank is gone. Research Data never enters an `Inventory` again.

### The material orders

`Game::select_research(id)` files one order per line of `def.materials`:

```rust
WorkOrder::batch(item.clone(), *need)
    .with_priority(OrderPriority::High)
```

through `Game::queue_work_order` — the ordinary door, so every refusal and
every log line it already owns applies. They are real queue entries: the
player sees them, re-bands them with `[P]`, and can cancel them.

`WorkOrder` gains exactly one field:

```rust
/// Filed on behalf of the active research project rather than by the
/// player. Withdrawn when the project ends.
#[serde(default)]
pub for_research: bool,
```

This is **provenance, not a plan**: it says who asked, never which machines
run or how far along they are, so the module header's "an item, a quantity
and a label, nothing else" rule holds. Set through a `with_research()`
setter beside `with_priority`, for the reason that one is a setter — a band
and a provenance are *values on* an order, not kinds of order.

Withdrawal on project end is `retain(|o| !o.for_research)`, which is why the
field must be `#[serde(default)]` and not `#[serde(skip)]`: a project
abandoned after a reload has to be able to take its orders with it.

**Filed once, at selection — never topped up.** Re-deriving the shortfall
each tick and re-filing would make `cancel_work_order` a no-op on exactly
the orders the player most wants to intervene in. If they cancel one, the
research waits at the material gate and the screen shows the shortfall;
re-filing is an ordinary order like any other.

Filing real orders is also what makes the rest of the base work for free:
`work_orders::queue_needs` is the closure of the **whole queue** under
`ItemDef::craftable`, so `hauling::consumer_beside` and the
attached-building rule already know the base wants a research material and
everything it is made of. **Nothing in the hauler changes.**

### The block

`Game::select_research(id)` refuses in this order, **every refusal before
anything is written** — `commit_caravan_basket`'s rule, and asserted per
refusal because a single test over one path passes against the others:

1. `is_game_over` / `has_active_battle` / `require_base` — as
   `unlock_research` does today.
2. Unknown id; already researched.
3. Missing prerequisites (`Game::missing_prereqs`), then the zone gate
   (`Game::research_zone_gate`) — the existing order, unchanged.
4. **A project is already active.** Names it and points at `[A]`.
5. **No Research Node deployed.** `chain_break` cannot answer this one: it
   refuses every banked item by construction, and its own doc names
   `research_data` as the example. So this is `producers_of(game,
   &research_currency).is_empty()`, reported with `makeable_by`'s
   two-sentence split — *"No Research Node deployed — that is what makes
   Research Data."*
6. **Each material through `work_orders::chain_break`**, in the order the
   file lists them, refused with that function's sentence verbatim — *"No
   Lathe deployed — that is what makes a Logic Wafer."* This is the whole
   of the "warn and block" requirement, and it costs no new derivation.

`Game::orderable_items`' rule applies one rung up: **the research screen
must mark a row blocked with the same call that refuses it**, so the screen
cannot offer a row the selection would then turn down. `ResearchStatus`
carries `blocked_by: Option<String>` filled from steps 5 and 6.

`views::ResearchState` gains a fourth variant, **`Active`**, and
`research_nodes` sorts it **first** — ahead of `Available`, `Locked`,
`Unlocked`. A variant rather than a flag, for the reason `Locked`'s two
reasons are fields rather than variants, read the other way: the four states
really are disjoint (a node cannot be both active and locked, and an active
one is by definition not yet unlocked), so a `bool` beside the enum would be
a second thing to keep in step with it.

`ResearchStatus::affordable` is **retired**. It meant "the player can pay
`cost` and every material line right now", which is a question nobody asks
any more: the cost is worked off over time and the bill is the base's. The
row colour rule it fed reads `blocked_by` instead.

### Completion

Checked once a tick, from `Game::tick_inner`, in `Game::settle_research`:

```
progress >= def.cost
  && every (item, need) in def.materials has base_holding(item) >= need
```

Then, and only then, the bill is consumed off the base's buffers, the node
is inserted into `resources::Research`, and the existing `unlocks_abilities`
/ `unlocks_tools` loops and the "Research complete" line run **exactly as
they do today**. `ActiveResearch::id` is cleared, the project's outstanding
orders are withdrawn, and its `progress` entry is **removed** — the node is
researched, nothing reads the figure again, and leaving it would grow the
save by one row per node for the life of the run.

Consuming from `base_holding` rather than the pack is the one behavioural
loss and it is deliberate: research is the base's job now, so the base pays.
A player who wants to contribute puts the goods into a Depot through the
transfer picker (`c`). `Game::research_material_held` is retired; the
screen's have/need figures re-point at `work_orders::base_holding`, so the
figure the screen draws and the figure the gate reads stay one call.

This needs a base-wide spend door. `hauling::take_from` is the one way a
unit leaves a `Stock`; the new `Game::spend_from_base(&[(ItemId, u32)])`
walks `stock::output_buffers` in the order that function already fixes and
routes every take through it.

## What moves

### Engine

| File | Change |
|---|---|
| `resources.rs` | `ActiveResearch` |
| `save.rs` | two fields, `#[serde(default)]`, the progress map sorted on write |
| `systems.rs` | `deliver_payout` gains the research-currency branch |
| `game/unlocks.rs` | `unlock_research` → `select_research` + `settle_research`; `research_material_held` retired; `research_nodes` gains progress and `blocked_by` |
| `game/base/work_orders.rs` | `WorkOrder::for_research` + `with_research`; `research_wants` and its place in `schedule_base_labour` |
| `game/base/stock.rs` | `spend_from_base` |
| `game/inspection.rs` | `attention` row: Research Nodes standing with no project selected |
| `views.rs` | `ResearchStatus` gains `progress`, `blocked_by`, loses `affordable`; `ResearchState::Active`; `ResearchMaterial::have` re-sourced |

### app-core

`handle_research_key`: `Enter` and the row selectors call `select_research`;
`[A]` abandons. **Uppercase**, because lowercase letters are row selectors.
Both the list view and the graph view route through the same call.

### gui

`render/progression.rs::draw_research_menu` and the graph view: a progress
figure per node, the blocked sentence wrapped on the detail panel, the
active project marked. The screen has no scroll, so **the row census must be
re-measured** — the tallest shipped node's detail panel now carries a
`chain_break` sentence, which runs to 158 characters.

### Assets and docs

- `assets/items/research_data.ron` — the description says "Spent on the
  research tree. Banked rather than carried". Both halves are now false.
  `banked: true` **stays**: it is what keeps the payout out of every `Stock`
  buffer, which is still exactly what is wanted.
- `assets/research/README.md` — `cost:` and `materials:` change meaning.
- `docs/research.md`, `docs/structures.md` — regenerated.
  `docs/manual.md` is carved out of the doc obligation and stays stale.
- `CHANGELOG.md` — a minor bump; no save break.

## Migration

A save written before this loads with no active project, an empty progress
map, and its `researched` set intact. Two things need saying:

- **Leftover banked Research Data is dropped at load**, with a log line.
  The stock strip folds in every `ItemDef::banked` pool by the flag, so a
  leftover pool would sit across the top of every base screen forever with
  nothing to spend it on.
- The strip's banked fold **stays** — it is written against the flag and
  never against a name, and a mod may ship another banked item.

## Testing

Engine, against the real assets:

- Selecting files one High order per material line, and each carries
  `for_research`.
- Each of the six refusals, **asserted per refusal**, each proving nothing
  was written: no order filed, no project set.
- A material with no deployed producer refuses with `chain_break`'s own
  sentence; building the producer makes the same selection succeed.
- No Research Node deployed refuses, with its own sentence.
- Progress accumulates from a posted program at a Research Node, saturates
  at `cost`, and lands nowhere with no project active.
- Completion needs **both** gates: progress alone does not unlock, a full
  bill alone does not unlock.
- The bill is consumed from a Depot the player is nowhere near.
- Abandoning withdraws the project's orders and **keeps** its progress; the
  player's own order for the same item survives.
- `schedule_base_labour` staffs a Research Node with a project active and
  does not without one.
- Ending a project frees its body **while the queue still has work**, and
  **leaves it standing on a run-dry base** — the documented behaviour of the
  `queue_is_empty` early return, asserted so nobody "fixes" it into a
  wholesale standdown on the first tick after a load.
- Save → load round trip carries the active project and the progress map,
  and a save written before the change loads clean with its `researched`
  set whole. (A RON round trip cannot catch a skipped field, so this is a
  save→load test and not a RON one.)
- `balance_sim` is unaffected — it models no base — but
  `cargo test -p feral-processes-engine balance_sim` is still a gate,
  because `tuning.rs` is touched if any constant moves.

app-core: the research screen selects, abandons, reports each refusal on
`status_line`, and the graph view routes to the same call.

gui: the tallest shipped node's detail panel fits, verified by mutation the
way `the_tallest_shipped_notification_fits_its_screen` is.

## Seams this moves

Three entries in `CLAUDE.md`'s load-bearing list change, and each needs its
argument written to the graph and its trap written to
`.claude/skills/seams/references/base.md` — the three writes, in that order.

- **New:** "Research is one project at a time, and `Game::select_research`
  is the one door — every refusal before anything is filed."
- **New:** "A research project's materials are ordinary work orders, and
  `WorkOrder::for_research` is provenance rather than a plan."
- **Amended:** the Research Node's status seam. "A banked resource can never
  clog, so a Research Node has no full state" still holds; what is added is
  that with no project selected it is `Idle` rather than anything new,
  because the scheduler simply does not staff it.

## Deliberately not in scope

- **Rebalancing `cost:`.** 8 → 540 across 35 nodes was priced as a bank
  saved across many purchases; as a single serialised project against
  `ticks_per_unit: 14` it reads differently. That is a playtest question and
  a separate content pass.
- **More than one project at a time.** One is what was asked for, and a
  second would need a priority rule between them.
- **Re-filing cancelled orders.** See above — it would make cancel a no-op.
- **Research Node upgrades.** `upgrade: (max_tier: 5)` already scales the
  payout through `node_payout`; it needs nothing new.
