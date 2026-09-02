# The tutorial contract chain

**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header. See `../../INDEX.md`.

A new run drops the player onto a surface with a pack, five Core Fragments
and no instruction. Everything the first hour asks of them — raise a Home,
examine a thing, put a program down, decompile one, stand up a Broker, place
a Mining Node, work the transfer screen, queue a standing order, post a
program, spend a perk point — is discoverable and none of it is *offered*.
The existing `starter: true` queue was the first attempt at this and it is
not enough: it biases a board draw, it needs a Contract Broker the player has
not built yet, and it says nothing about order.

This makes onboarding a **chain**: a fixed sequence of contracts, one live at
a time, handed to the player rather than offered, that cannot be declined or
reordered, and that suppresses the ordinary board until it is finished.

The feature is deliberately mostly content. **The chain, its one live step
and the green row are Rust; every mission is a `.ron` file.** A twelfth
mission must cost one asset file and — where its verb already exists — no
Rust at all. That is the acceptance criterion the implementation is held to,
and it is stated here because it is the thing that decays first.

## What this is not

Not a scripted overlay, not a pointer at a key, not a modal that blocks
input. A tutorial mission is an ordinary `resources::ActiveContract` with an
ordinary objective, progressed by the ordinary `contract_system` and settled
by the ordinary `complete_contract`. Nothing downstream of the chain can tell
a tutorial mission from a contract the player signed for, which is the same
property `ContractTemplate` already buys for rolled contracts.

It is also not a second screen. The chain is read on the contracts screen the
game already has, opened the way it already opens.

## The chain

### Expressed as a field, not a directory

`ContractDef` gains one field:

```ron
tutorial: Some(60),
```

An ordinary contract leaves it off. `#[serde(default)]`, so every existing
`.ron` file and every mod keeps parsing untouched, and the addition costs no
`SAVE_FORMAT_VERSION` bump for the same reason it costs no asset edit.

Tutorial missions live in `assets/contracts/` beside everything else. Three
alternatives were considered and rejected:

- **A separate `assets/contracts/tutorial/` directory with an order
  manifest.** The manifest is a second census that drifts: a file in the
  directory and not in the manifest is invisible, with no warning and no
  failing test. The repo has been bitten by exactly this shape before —
  `docs/*-gen.py` are hand transcriptions and one was a node short for a
  whole release.
- **A `tutorial_after: Option<ContractId>` linked list.** No numbers to
  collide, but a cycle and a fork are both expressible and both have to be
  refused at load, and the whole chain cannot be read at a glance.
- **A hardcoded `const TUTORIAL_CHAIN: &[&str]` in Rust.** Cheapest, and it
  puts content in the engine, which the moddability rule forbids.

The number is a **step**, not an index: the shipped missions are spaced 10
apart so inserting one later never renumbers the others.

### Refused at load

Beside everything `ContractDb::load_dir` already refuses:

- **Two files claiming the same step.** Silent ambiguity otherwise — the
  sort would resolve it by id and the chain would run in an order nobody
  authored.
- **`tutorial` together with `starter`.** A tutorial mission is never
  offered, so a starter flag on one is a statement about a board slot it can
  never occupy. Refusing it is how the two systems stay legible as separate
  things.
- **`tutorial` together with `repeatable`.** The chain's position is derived
  from `done`; a repeatable mission would leave and re-enter the chain
  forever.

`min_zone` on a tutorial mission is not refused but is inert, and the
README says so.

### The position is derived, never stored

`ContractDb::tutorial_chain()` is the one derivation: every def with a
`tutorial` step, sorted by step. The run's position in it is **the first
mission whose id is not in `ActiveContracts::done`** — a field the save
already carries. There is no cursor, no index, no progress field, and
therefore nothing that can disagree with `done` about where the player is.

This is `Game::morale` and `views::BuildOrderRow`'s rule again: every figure
is a call.

## Handing a mission out

`Game::ensure_tutorial_held` is the one door. If the chain has a current step
and that mission is not in `ActiveContracts::active`, it pushes it at
progress 0 and logs one line.

**It is the only writer**, and the callers are:

- `Game::new`, after the world is built — so the very first contracts screen
  a run opens already has the first mission in hand.
- `Game::load`, after the run is restored — so a save taken mid-chain
  resumes with no seeding path of its own to keep in step.
- `Game::settle_contracts`, at the end of its loop — so finishing mission
  *n* hands out mission *n+1* in the same tick, with no gap the player could
  see.

The player never accepts a tutorial mission, so it never passes through
`accept_contract`. Three consequences fall out as **omissions rather than
checks**, which is the point of routing it this way:

- `MAX_ACTIVE_CONTRACTS` never sees it. The chain's mission sits beside up
  to three signed contracts, and the cap keeps meaning what it meant.
- `Game::broker_reach` never sees it. A mission is in hand with no Broker
  standing anywhere, which is what lets the first five missions exist at
  all.
- `Game::offerable` never sees it. A tutorial mission is not filtered by
  `min_zone`, by `already_met` or by anything else the board applies.

Giving one back is the one place a check is needed rather than an omission,
and it takes **two** guards because the invariant and the sentence live on
different sides of the seam. `Game::abandon_contract` refuses a tutorial
mission and returns false, so the engine's "exactly one mission is held"
property does not depend on a caller remembering. `App::handle_contracts_key`
reads the new `ContractRow::tutorial` flag it is already holding and calls
`App::refuse` before it asks — that is the one door for a refusal's sentence
(`input.rs:355`), landing it on both the popup and the log, which a bare
`false` from the engine cannot do. An unbreakable chain with a give-back key
is not a chain.

## Suppressing the board

While the chain is unfinished, `Game::board_defs` returns an empty list —
one early return above the existing starter partition, so the starter code
is untouched.

`Some(vec![])` rather than `None`: the Broker exists and is reachable, and
`None` means "no Broker standing", which is a different claim two other
readers already depend on.

The Offered header says why. `render/contracts.rs::offered_header` already
takes `BrokerReach` and returns a string; it gains a second input for
"onboarding is live" and returns a line naming the errand, the same way the
`OffBase` line does. A section that simply read *Nothing on the board* under
a Broker the player just built reads as broken.

When the last mission completes, `board_defs` stops returning early and the
board fills normally — including the seven `starter: true` contracts, which
now do the job they were written for at the moment the board first opens.
Two of them (`break_ground`, `first_reading`) name structures the chain has
already raised and are retired by `Objective::already_met` with no new code.

**No existing contract file is edited by this feature.**

## The new objective vocabulary

Six of the eleven missions need verbs `Objective` does not have. They are
added as **one** variant, not six.

### `Objective::Perform { deed: Deed }`

`Deed` is a typed enum in the engine, not a string:

```rust
pub enum Deed {
    Examined,
    Tamed,
    TookFromContainer,
    QueuedStandingOrder,
    UnlockedPerk,
    PostedStaff,
}
```

Typed rather than string-keyed because a deed is an **engine event**, not
content: a mod cannot add one, so the openness a string buys is openness onto
nothing. What a string would buy instead is a mission naming a deed that does
not exist, loading without a warning and never completing — the same failure
the README already documents for a `Terminate` naming a missing species, and
one there is no reason to repeat where the vocabulary is closed.

`ron` spells it `Perform(deed: Examined)`.

### How a deed is recorded

`resources::RunFeats` gains a third field, `deeds: Vec<Deed>`, drained by
`game::contracts::contract_system` and by nothing else. That is the rule the
existing two fields already hold — `bosses_defeated` and `kills` each have
exactly one drainer, which is what removes any ordering dependency between
the systems that read them. A shared queue would make that unsound the moment
one ate the other's events.

`Game::note_deed(Deed)` is the one door a deed is written through, and the
six triggers are **callers of it, not writers beside it** — `Game::remember`'s
rule. All six sites already exist and are already `&mut self`:

| Deed | Written from |
|---|---|
| `Examined` | `Game::find_target_in_direction`, on a hit (`game/inspection.rs`) |
| `Tamed` | `Game::attempt_decompile`, on success (`game/combat_rewards.rs`) |
| `TookFromContainer` | `Game::transfer_items`, when the take side moved anything (`game/base/transfer.rs`) |
| `QueuedStandingOrder` | `Game::queue_work_order`, on an order with `standing` set (`game/base/work_orders.rs`) |
| `UnlockedPerk` | `Game::unlock_perk`, on success (`game/unlocks.rs`) |
| `PostedStaff` | `Game::post_worker` (`game/base/building.rs`) |

A deed carries no parameters. `QueuedStandingOrder` does not name the item or
the count, and `PostedStaff` does not name the structure: the mission's
description tells the player what to order and where to post, and a
parameterised deed would be a second place the same instruction is written.
If a later mission genuinely needs to distinguish two postings, that is a new
`Deed` variant, not a field on this one.

`contract_system` advances a `Perform` objective by the count of matching
deeds in the queue, exactly as it advances `Terminate` by matching kills.
`Objective::target()` for `Perform` is 1.

### `Objective::Hold { item: ItemId, count: u32 }`

A sixth state-shaped objective: *have this many in your pack*. It exists
because the chain has to teach "fighting pays in stock" before a Contract
Broker is standing, and `Deliver` — the only stock objective the game had —
is handed over at a Broker by construction.

It is state-shaped, so it belongs in `already_met` beside `Descend`, `Breach`
and `Build`, and is polled by `contract_system` through exactly that
predicate. That is the existing rule and it must not be bent: `already_met`
is the one statement of "is this already true", read by the system that
advances and by the board that refuses to offer.

`already_met` currently takes `(depth, zone, standing)`. `Hold` makes it
four, and the next objective would make it five. **Fold the arguments into
one `ObjectiveState` struct** built at each of the two call sites — carrying
depth, zone, the standing structures and the player's carried counts. Two
readers, one struct, and adding a seventh objective costs a field rather than
a signature change at both.

`Hold` is genuinely useful outside the tutorial and the README documents it
as an ordinary objective, not a tutorial one. Note it is **not** the same as
`Deliver`: nothing is spent, and it can be met four frames down.

## The free first decompile

While `tutorial_first_decompile` is the run's live mission,
`Game::attempt_decompile` skips the `taming::capture_chance` roll and
succeeds.

**The catalyst is still spent.** The lesson the step exists to teach is that
decompiling is priced in catalysts; a step that consumed nothing would leave
the player to discover that later, from a failure they were never shown the
reason for.

The branch reads the live tutorial mission rather than a flag on the run, so
it is derived like everything else here and disarms itself the moment the
chain moves on. It sits after the no-catalyst guard and after the resistance
read, so the odds the battle screen has been showing stay honest about what
the *next* attempt would have been.

This is the one place the feature changes a shipped formula's outcome, and
it is bounded to a single mission of a single run.

## Green

`views::ContractRow` gains `tutorial: bool`.

`render/contracts.rs::contract_line` builds its `Row::Item` with
`color: GREEN` when the flag is set. Both halves already exist: `Row::Item`
has a `color` field, and `GREEN` is already defined in `render/mod.rs`.

Green is a free axis on this screen. The popup `color` field means fusion
tier on the gear screens, CRITICAL HP on the party screen and idleness on the
roster — the contracts screen has never used it, so nothing is being
overloaded and no second meaning lands on an axis that already had one.

The existing width census, `no_shipped_contract_row_overflows_its_popup`,
walks `Game::contract_catalogue`, so a tutorial mission's row is measured
against the real font for free — provided `contract_catalogue` keeps
returning the whole shipped set, tutorial missions included. It must.

## Existing saves

The tutorial runs on new games only. A run in progress when this ships does
not suddenly get told to build a Home it built forty hours ago.

`PlayerSave` gains `#[serde(default)] tutorial_seeded: bool`. Additive behind
a default, so it costs **no `SAVE_FORMAT_VERSION` bump** — the save has been
field-named RON since long before this, and that is exactly what retired
migrations.

- `Game::new` sets it true.
- `Game::load` reading it false files every id in `tutorial_chain()` into
  `ActiveContracts::done` and sets it true. The chain is finished before it
  starts, `board_defs` never suppresses, and nothing else in the feature
  fires.

There is deliberately **no skip key**. A second way to reach "chain finished"
is a second thing to test and a second thing to get wrong, and the chain is
eleven missions the player wanted to do anyway.

**A `#[serde(default)]` field needs a save→load test, not only the RON round
trip.** The round trip stays green whether or not the field is written; only
a real save and a real load can see it survive.

## The shipped chain

| Step | Id | Objective | What it teaches |
|---|---|---|---|
| 10 | `tutorial_first_light` | `Build("home")` | Nothing else can be raised until the Home stands |
| 20 | `tutorial_take_a_look` | `Perform(Examined)` | `x` |
| 30 | `tutorial_scrap_run` | `Hold("core_fragment", 12)` | Fighting pays in stock |
| 40 | `tutorial_first_decompile` | `Perform(Tamed)` | Decompiling, and that it costs a catalyst |
| 50 | `tutorial_sign_here` | `Build("contract_broker")` | Where contracts come from |
| 60 | `tutorial_break_ground` | `Build("mining_node")` | A machine works while you don't — and the crew hauls what it cuts |
| 70 | `tutorial_collect` | `Perform(TookFromContainer)` | `c` |
| 80 | `tutorial_standing_order` | `Perform(QueuedStandingOrder)` | A standing order for 20 Core Fragments |
| 90 | `tutorial_first_reading` | `Build("research_node")` | Banking readings |
| 100 | `tutorial_man_the_node` | `Perform(PostedStaff)` | Posting a program to a machine |
| 110 | `tutorial_spend_it` | `Perform(UnlockedPerk)` | Perks |

Step 60's *description* carries what would otherwise have been a twelfth
mission — that the crew moves the Node's output on its own, so the thing to
do is stand in the base and watch. There is no mechanic for a hauler taking
from the player's pack (`haul_step_system` moves stock between structures
only), so a mission asking for one would be unfinishable.

**Perks are last, not seventh.** A perk point is a level-up reward
(`PERK_POINTS_PER_LEVEL * gain.levels`). Asked for early, a fresh run may
have none, and the chain stops dead on a screen with nothing to buy. Ten
XP-paying missions ahead of it is what makes the step reachable.

## The chain must not stall

An unbreakable chain fails in a way an optional contract does not: a mission
the player cannot finish ends the run's onboarding permanently, with no key
to press. Three stalls are possible and each gets a census over the real
assets, in `tests/assets.rs` beside `ZONE_MATERIALS` and `MEMORY_TRIGGERS`:

1. **Economy.** Four missions cost materials to finish — Home (5 Core
   Fragments), Contract Broker (5), Mining Node (12), Research Node (10). A
   test walks the chain in step order carrying a running balance, starting
   from the run's starting inventory, crediting each mission's `Reward::Item`
   payout and debiting each `Build` objective's `build_cost`, and asserts the
   balance never goes negative. Shipped rewards are priced to satisfy it,
   with headroom — the player also spends fragments on things the chain does
   not know about.
2. **Vocabulary.** Every `Deed` variant named by a shipped mission has a
   writer, and every `Deed` variant has at least one caller of
   `Game::note_deed`. Exhaustive over the enum, `cell_mark`'s rule, so a
   variant with no emit site fails the build rather than shipping a mission
   that never completes.
3. **Well-formedness.** Steps are unique; ids resolve; every `Build` names a
   real structure and every `Hold` a real item; no mission is `starter` or
   `repeatable`. The shipped set is checked here; a mod is not, which is the
   same line every other content directory draws.

A fourth property is a unit test rather than a census: **exactly one tutorial
mission is held at any instant**, from `Game::new` through the last
completion, including across a save and load.

## Files

| File | Change |
|---|---|
| `crates/engine/src/contracts.rs` | `ContractDef::tutorial`; `Deed`; `Objective::Perform`, `Objective::Hold`; `ObjectiveState`; `ContractDb::tutorial_chain`; the three load refusals |
| `crates/engine/src/game/contracts.rs` | `ensure_tutorial_held`; `Game::note_deed`; `board_defs` early return; `abandon_contract` refusal; `contract_system` deed and hold arms |
| `crates/engine/src/resources.rs` | `RunFeats::deeds` |
| `crates/engine/src/game/inspection.rs` | `Examined` |
| `crates/engine/src/game/combat_rewards.rs` | `Tamed`; the forced roll |
| `crates/engine/src/game/base/transfer.rs` | `TookFromContainer` |
| `crates/engine/src/game/base/work_orders.rs` | `QueuedStandingOrder` |
| `crates/engine/src/game/unlocks.rs` | `UnlockedPerk` |
| `crates/engine/src/game/base/building.rs` | `PostedStaff` |
| `crates/engine/src/game/lifecycle.rs` | `ensure_tutorial_held` at `new` and `load`; the `tutorial_seeded` back-fill |
| `crates/engine/src/save.rs` | `PlayerSave::tutorial_seeded` |
| `crates/engine/src/views.rs` | `ContractRow::tutorial` |
| `crates/engine/src/tests/assets.rs` | the three censuses |
| `crates/app-core/src/app/contracts.rs` | the abandon refusal, through `App::refuse` |
| `crates/gui/src/render/contracts.rs` | `GREEN` on a tutorial row; `offered_header`'s onboarding line |
| `assets/contracts/*.ron` | eleven new files |
| `assets/contracts/README.md` | `tutorial`, `Perform`, `Hold`, and a Tutorial section |

Two crates and a save-format field, so this takes the full spec-and-plan
pipeline per `CLAUDE.md`'s process-weight rule.

## Seams this touches

Read `docs/seams.md` and the `seams` skill before changing these, and update
all three places if any of them moves:

- **`ActiveContract` stores the whole resolved `ContractDef`, not an id.** A
  tutorial mission is copied into `active` the same way, so a file edited
  mid-run cannot strand the chain either.
- **A Broker's board is derived, never stored.** The suppression is an early
  return inside that derivation, so it inherits every property the seed
  buys — survives a reload, cannot be save-scummed, spends no `GameRng`
  draw.
- **Contracts deliberately amend "progression is earned by fighting."** The
  chain pays XP on build and perform objectives, which is the same intended
  amendment and not a new one.
- **`Game::remember` is the one door a memory is written through.**
  `Game::note_deed` is built to that shape and for that reason.
- **`render/stack.rs`'s `cell_mark` is exhaustive, and must stay so.**
  `Deed`'s census follows it.

## What this is blind to

- **It is not playtested, and cannot be until it ships.** Every claim here
  about pacing — eleven missions, perks last, the fragment headroom — is
  reasoned from the assets, not observed. `--template` captures should be
  taken at three points in the chain once it is playable.
- **Nothing measures how long the chain takes.** The funding census proves
  it cannot stall; it says nothing about whether step 30 is a two-minute
  errand or a twenty-minute grind.
- **The eleven missions are a first cut of the wording.** The descriptions
  are the only place a player is told what to do, and they are the part
  most likely to need a second pass after someone plays it.
