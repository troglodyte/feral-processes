# Contracts

**Status:** approved 2026-08-14, not implemented.

> `INDEX.md` warns that this header is the one line in a spec nobody ever
> revises. Answer "did this ship" from `CHANGELOG.md` and a grep, never from
> here.

Closes `TODO.md` #21, "missions, quests".

## The problem

The game has no statement of what to do next. Research is a shopping list,
achievements are cross-run and one-shot, and the zone ladder says only "go
deeper". A player who has stood their base up and beaten a stack has no
answer to "and now?" beyond repeating it a level down.

Contracts are that answer: short, named, finite objectives with a payout,
issued by a structure you build, that give a session a shape.

## The design change this makes on purpose

**Progression stops being earned only by fighting.**

That invariant is load-bearing today — it is why the iso Market refuses to
sell Portal Fragments, why scan and the Terminal were closed, and why free
rest was removed. It is recorded in `assets/structures/black_market.ron` and
in the project memory.

Contracts deliberately amend it: **XP is a legal contract reward on any
objective**, including delivery and construction. The intent is to shift a
share of the run's XP from kills onto contracts, so that what advances the
player is *the thing the game asked them to do* rather than grinding whatever
is nearest. Magnitudes on both sides — contract XP and creature XP — are
expected to move together, iteratively, once this is playable.

This is a design decision, not an oversight. Anyone reading the invariant
later and "restoring" it by gating XP behind combat objectives is undoing the
feature. What survives unchanged is the narrower rule underneath it: **Portal
Fragments are still earned only by fighting and descending.** They are not a
contract reward, and the route from base production straight to breaching
stays closed.

## Vocabulary

Player-facing and code-facing alike: a **contract**, issued by a **Contract
Broker**.

`quest` and `mission` are the player's words for the genre, not this game's.
Two candidate names collide with live concepts and are refused: *job* already
means a species class's behaviour at a post (Leech, Bastion, Medic), and
*cronjob* already means a program posted to a machine.

- module `crates/engine/src/contracts.rs`, `ContractDef`, `ContractId`
- content `assets/contracts/*.ron`
- structure `assets/structures/contract_broker.ron`, glyph `!` (unused)
- `Mode::Contracts`

## Data model

`assets/contracts/` is a real content directory, on the `assets/achievements/`
model: one file per contract, `ContractDb::load_dir`, a malformed file skipped
with a logged warning rather than a panic, and a README that is the schema.

```ron
(
    id: "clear_the_nursery",
    name: "Clear the Nursery",
    // Authored, never derived from the objective — a modder controls how
    // their contract reads, exactly as AchievementDef::description does.
    description: "Six drones have nested in the shallows. Thin them out.",
    objective: Kill(species: Some("drone"), count: 6),
    reward: [Credits(40), Xp(120)],
    // Gates whether a board may *offer* this contract, never whether an
    // accepted one may be finished — breaching mid-contract must not strand
    // it. Spelled as ResearchDef::min_zone is: 0 and absent mean the same
    // thing, and a second spelling for it is a second thing to get wrong.
    min_zone: 1,
    repeatable: false,
)
```

### `Objective`

Five variants. The column that matters is the last one — the whole
instrumentation cost of this feature is one counter.

| Variant | How progress is measured | New call sites |
|---|---|---|
| `Deliver { item, count }` | items handed over at the Broker | 0 |
| `Descend { depth }` | polled off `resources::Locale` | 0 |
| `Breach { zone }` | polled off `resources::ZoneLevel` | 0 |
| `Build { structure }` | polled — is one deployed? | 0 |
| `Kill { species: Option<String>, count }` | a counter written beside `award_loot` in `finish_member` | 1 |

`Descend` reads `Locale`, never `Position` — `Position` is pinned to the
surface entrance tile while the party is underground, so a depth taken from it
would be a surface coordinate. Same trap `achievement_system` documents.

`Kill` takes `Option<String>` for the same reason `Trigger::BossDefeated`
does: `None` means any kill, `Some(id)` names a species.

`Tame` is deliberately absent from v1. A program joins the roster through two
doors — `Game::adopt_program` and the decompile capture — so it costs two call
sites rather than one, and neither is as cleanly funnelled as the kill site.

### `Reward`

A `Vec<Reward>`, not one: a contract paying both Credits and an item is the
common case, and a single-reward field would force two contracts to express it.

| Variant | Pays |
|---|---|
| `Credits(n)` | `n` added to the player's `ids::CREDITS` |
| `Item(id, n)` | `n` plain copies of that item |
| `Xp(n)` | `n` XP to the player, through `award_player_xp` |

Two bounds, both deliberate:

**Gear rewards grant plain `Ordinary` copies and do not go through
`Game::grant_gear_drop`.** That function is the only door a copy above
`Ordinary` enters the game by, and crafting, buying and buying back are
already deliberately not callers — found gear is categorically better than
made gear, which is the whole reason to go looking rather than shopping. A
contract payout is closer to made than found. `crafted_gear_is_never_rare`
has a sibling here, and the absence needs a test because an omission is
invisible.

**`Reward::PortalFragments` does not exist.** Not "exists and is unused" —
absent, so it cannot be added by a mod file either.

A reward of `0` is rejected at load with a warning, as `Reward::PerkPoints(0)`
already is: a contract that pays nothing is a mistake that reads as a working
file.

## The board

**Offers are derived, never stored.** `game/stack_market.rs` is the precedent
and the argument is identical: the player is shown an offer before they accept
it, so the answer has to survive a save and load, and `GameRng`'s stream
position is not persisted.

A local `StdRng` seeded from `(world seed, zone, epoch)` where
`epoch = clock.tick / CONTRACT_REFRESH_CYCLES`. Four properties come free:

- survives save/load with no save field
- spends no `GameRng` draw, so opening the screen shifts nobody's stream —
  the failure this repo has now been bitten by three times
- cannot be rerolled by save-scumming
- rotates on its own as the run proceeds

Salted off the world seed with its own named constant, per `FrameSpec::salted`
— one scheme, not a second seed source that could collide with the Stack's.

The derived list is filtered against the run's active and completed contracts,
so a contract already taken or already finished is not offered again.

`Game::contract_board()` answers "is there a Broker in range" and "what is on
it" in one call, the way `Game::stack_market` does, so no screen asks those
separately and then disagrees about the answer. Range is
`view_entities(MENU_SCAN_RADIUS, …)` filtered on a new `EntityView` flag —
the same shape `can_trade` already has, and the same scan the trade screen
uses to find a trading post.

## Run state and the save

Two new fields on `SaveData`, both `#[serde(default)]`:

```rust
pub contracts: Vec<ActiveContractSave>,
pub contracts_done: Vec<ContractId>,
```

Since v29 the payload is field-named RON, so **an additive field costs no
`SAVE_FORMAT_VERSION` bump, no migration and no tool.** Both are named
structs rather than tuples: a positional tuple is the one shape RON cannot
widen later, which is what made `PlayerSave::fused_gear` a legacy field.

`ActiveContract` stores the **whole resolved `ContractDef`**, not an id plus
parameters. Same argument `EquippedItem` stores an entire `GearCopy`:
forgetting a property must not be expressible, and a contract file edited or
deleted mid-run must not strand or silently rewrite a contract already
accepted. A save naming a contract whose file is gone therefore still
finishes and still pays.

This also retires a documented limitation. `assets/achievements/README.md`
says there is deliberately no "kill N bosses in one run" trigger because
*"counting within a run needs saved run state the game doesn't keep"*. This is
that state; a counting achievement trigger becomes cheap once it exists. Out
of scope here, but the README's claim will need correcting when it lands.

## Progress

One system, `contract_system`, is the only writer of contract progress — the
argument `achievement_system` makes about being the one place that decides
what has been earned, and the reason neither has to be kept in step with a
scatter of call sites.

It polls the four state-shaped objectives and drains a new `RunFeats::kills`
field for `Kill`. A **separate field** from `bosses_defeated`, each drained by
exactly one system, so the two systems have no ordering dependency on each
other and neither can eat the other's events.

`RunFeats` stays unsaved. It is a per-tick drain queue; what accumulates is
the saved progress counter on the `ActiveContract`, written by this system.

Completion is announced, then paid. `Game::complete_contract` is the single
door: it moves the id into `contracts_done`, drops the `ActiveContract`, and
grants each `Reward`. XP goes through `award_player_xp` so a level-up
full-heals exactly as it does from a kill.

## Screen

One `Mode::Contracts`, two stacked sections: **active** contracts with their
progress, then **available** offers from a Broker in range. Empty second
section when no Broker is nearby, which is the honest reading — you can always
see what you have taken, and can only take more where they are issued.

Verbs: accept, abandon, and hand over a `Deliver` objective's items.

One base-menu row, `available` when a Broker is in range **or** any contract
is active — so the row can never advertise a screen with nothing on it, which
is what `group_rows` requires. Not `surface_only`: the screen reaches no
zone-map state through `Position`, and reading your active contracts four
frames down is exactly when you want to.

Row count is owned by app-core and drawn by gui, per the read-only-screen
rule — any per-row transform lives in the engine so the two sides cannot
disagree about which row is under the highlight.

## Tuning

New `pub const`s in `crates/engine/src/tuning.rs`, with every other difficulty
knob:

- `MAX_ACTIVE_CONTRACTS` — 3
- `CONTRACT_REFRESH_CYCLES` — 400, how long a board's offers stand before the
  epoch advances and it re-derives
- `CONTRACT_BOARD_SLOTS` — 3, how many offers a board shows at once

Both figures are opening guesses. The refresh is the one to watch in play (see
Risks), and neither is reachable by any existing instrument.

Contract *rewards* are authored per file, not tuned here, and are bounded by
`item_value`'s existing two ceilings for Credits and items. XP is bounded by
nothing today; that is the knob expected to move first once this is played.

## Phases

Each ends green and is independently committable. Phases 1–3 are engine-only.

**Phase 1 — the type and its catalogue.** `contracts.rs`: `ContractId`,
`ContractDef`, `Objective`, `Reward`, `ContractDb::load_dir` with validation
(empty id, duplicate id, zero reward, unknown item/species/structure id) and
returned warnings. `assets/contracts/` with a README that is the schema, and
roughly eight authored contracts spanning all five objective variants. No game
wiring at all. Tests: load, reject each malformed shape, and a census that
every shipped contract names ids that exist.

**Phase 2 — progress and completion.** `resources::ActiveContracts`, the two
`SaveData` fields and their round trip, `RunFeats::kills` and the one counter
in `finish_member`, `contract_system`, `Game::complete_contract` and the
reward grants. Driven directly against `Game` — no board, no screen. Tests:
each objective advances and completes, a completed contract pays once, gear
rewards are never above `Ordinary`, a save round trip preserves progress.

**Phase 3 — the Broker.** `assets/structures/contract_broker.ron`, the
research node that unlocks it, the seeded offer derivation,
`Game::contract_board`, `accept_contract` / `abandon_contract` /
`deliver_to_contract`, and the `EntityView` flag. Tests: the same board is
derived after a save/load, the epoch rotates it, `GameRng` is untouched by
reading it, an active or completed contract is not re-offered, and
`MAX_ACTIVE_CONTRACTS` is refused rather than silently capped.

**Phase 4 — the screen.** `Mode::Contracts`, `app/contracts.rs`, the
group-menu row, `render/contracts.rs` and its dispatch. Two crates. Tests:
the row hides when there is nothing to show, row indices resolve to the right
section, and a popup-width census on the widest shippable row — the failure
mode `TODO.md` bugs 1 and 2 already record.

**Phase 5 — rolled contracts.** `ContractTemplate`, the roll, and its validity
rules. Rolled contracts resolve into the *same* `ContractDef` the authored
files parse into, so there is one accept path, one progress path and one
completion path — an authored contract is a template with no free variables.
Deliberately last: a rolled objective naming an item this sector cannot
produce is an unfinishable contract, and those rules are far easier to write
after playing the authored ones.

## Deliberately excluded

- **Deadlines and expiry.** Adds a failure state, a clock reader and an
  abandonment path for no proven value. Add it after playing, if the board
  feels static.
- **Reputation, tiers, contract chains.** YAGNI until the basic loop is
  played.
- **`Tame` objectives.** Two call sites rather than one; revisit with
  evidence that players want them.
- **A counting achievement trigger.** Cheap once phase 2 lands, but it is a
  separate feature and belongs in its own change.

## Risks

- **XP magnitudes are ungated.** `balance_sim` is RNG-free and models one
  run's combat curve; it cannot see a contract at all. The first authored XP
  numbers are guesses, and the intended shift of XP away from kills has no
  instrument. Expect to iterate from play, and expect
  `balance_sim`'s level curves to move when creature XP is retuned to match —
  that movement is the signal, not a broken test.
- **The board can read as static.** With no deadlines and a long refresh, a
  player who takes nothing sees the same three offers for a long stretch.
  `CONTRACT_REFRESH_CYCLES` is the only control; watch it in play.
- **Phase 4 is where width bugs live.** Two open `TODO.md` bugs are popup rows
  overflowing their body. A contract row carries a name, a progress figure and
  a reward summary; measure it rather than assuming.
