# Contracts phase 5 — handoff notes

Written 2026-08-14, immediately after phases 1–4 shipped in **0.8.29**. These
are what building phases 1–4 taught that
[`2026-08-14-contracts.md`](2026-08-14-contracts.md)'s Task 13 could not have
known. Read that task for the goal; read this for what is in the way.

## Before anything else

**Task 13 is gated on play, and nothing here lifts that gate.** The plan's own
words: a rolled objective naming an item this sector cannot produce is an
unfinishable contract, and those validity rules are far easier to write once
someone knows what a good contract feels like. Two figures have no instrument
at all — `balance_sim` is RNG-free and cannot see a contract:

- **Contract XP magnitudes.** The authored numbers (90–300) are guesses. The
  intended shift of XP away from kills is unmeasured.
- **`CONTRACT_REFRESH_CYCLES` (400).** With no deadlines and a long refresh, a
  player who takes nothing sees the same three offers for a long stretch.

To reach contracts in play: research Contract Brokerage (10 Research Data),
build the Contract Broker (`!`, 14 Core Fragments), stand within
`CONTRACT_BOARD_RANGE_TILES` (2), base menu → Contracts.
`FERAL_DEV_REVEAL` and the `dev-saves/` templates do not cover this yet — no
template stands a Broker up, so a playtest starts with the research buy.

## The one structural blocker

**Everything on the accept path re-resolves the def out of `ContractDb` by id.**
Three sites in `crates/engine/src/game/contracts.rs`:

| Site | What it does | Why a rolled contract breaks it |
|---|---|---|
| `offerable_contracts` | returns `Vec<ContractId>` | a rolled offer has no id in the db to return |
| `contract_board` | draws an id, then `ContractDb::get(&id)` to build the row | `get` returns `None` and the slot is silently dropped |
| `accept_contract` | `ContractDb::get(id).cloned()` for the def it stores | returns `NotOffered` for something visibly on the board |

There is also a quieter one: `accept_contract`'s `AlreadyDone` check asks the
db whether the contract is `repeatable`. A rolled id in `done` resolves to
`None`, so `is_some_and` is false and it reads as **not** repeatable. That is
probably the right default, but it is an accident rather than a decision.

The fix is one shape change, and doing it first makes Task 13 mostly
mechanical: **carry the `ContractDef` through the board instead of the id.**
`offerable_contracts` becomes `Vec<ContractDef>`, the board rolls defs, and
`accept_contract` takes the def off the board it already built rather than
looking it up again. Nothing else needs to change, because
`resources::ActiveContract` already stores the **whole resolved def** — it was
built that way for a different reason (a file edited mid-run must not strand a
contract already accepted), and it is exactly what a rolled contract needs.

Note what this does *not* require: no second accept path, no second progress
path, no second completion path. The plan says an authored contract is a
template with no free variables; the storage side already agrees. If you find
yourself writing a second completion path, stop — the design has gone wrong.

## Seams to widen, not copy

- **`Game::habitat_pools`** is the existing split for "which species belong
  here", already shared by `pick_habitat_species` and `orphan_species`. A
  rolled `Kill` objective's validity check goes through it. Do not rebuild the
  pool logic — the split exists precisely at the point where one caller starts
  spending `GameRng` and the other must not.
- **`Game::board_seed`** is the FNV-1a fold over
  `(world seed, zone, epoch, CONTRACT_BOARD_SALT)`, byte at a time. A rolled
  contract's *own* parameters (which species, how many, which item) must salt
  off this, not off a second scheme — `FrameSpec::salted`'s rule. Rolled ids
  must be stable within an epoch for the same reason the board is: the player
  sees the offer before accepting it, and it has to survive a save and load.
- **`Game::contract_row` / `objective_line` / `reward_line`** already word
  every objective and reward engine-side. A rolled contract needs no new
  wording code, only a `description` — which is the one field a template
  cannot derive and must author with a hole in it.
- **`Game::contract_catalogue`** exists only for the gui width census. If
  rolled contracts can produce longer names or reward lines than the authored
  eight, that census stops covering the real widest row —
  `no_shipped_contract_row_overflows_its_popup` in
  `crates/gui/src/render/contracts.rs` would then need to measure templates at
  their widest roll, not just the catalogue. It was mutation-checked (a long
  name fails it by 567px), so it does bite; it just would not be looking at
  everything any more.

## Traps already paid for — do not re-derive

- **Never `GameRng`, anywhere in this feature.** Its stream position is not
  persisted and a draw shifts every later roll in the run.
- **`ContractDb::load_dir` treats an absent directory as silent and empty**
  (`AffixDb`'s rule). Roughly 60 existing test fixtures build a partial assets
  dir; making the loader strict fails all of them at once, and the README
  already promises deleting the directory gives the pre-contract game. A
  `templates/` subdirectory therefore also has to be optional.
- **`contract_board` returns `None` underground.** `Position` is pinned to the
  surface entrance tile, so a range check made from it seats the party at a
  Broker four frames above. `active_contracts` is the half that reads
  anywhere, and the base-menu row is `surface_only: false` only because the
  engine refuses rather than the frontend guessing.
- **Completion runs from `tick_inner` as `Game::settle_contracts`**, not from
  inside `contract_system`. The plan has the system calling
  `complete_contract`, which a bevy system cannot do — paying writes the
  inventory and grants XP. `contract_system` raises the number and stops.
- **`RunFeats` has two fields with one drainer each.** Merging `kills` into
  `bosses_defeated` makes two unchained systems order-sensitive.
- **A new field on a resource shifts bevy's query iteration order.** A failure
  in an untouched subsystem right after touching `RunFeats` or `ActiveContracts`
  is most likely a latent unsorted-query test.

## State of the branch

0.8.29 is merged to local `main`, committed and tagged `v0.8.29`, **not
pushed** — deferred deliberately so phase 5 goes out with it. The `contracts`
branch is still standing for the same reason: the deploy convention confirms a
merge against the *remote* before deleting a branch, and that check cannot pass
until the push happens. Delete it after.

Gates as of the tag: `cargo test --workspace` 2289 passing, clippy clean,
`cargo fmt` applied, `balance_sim` curves unmoved.
