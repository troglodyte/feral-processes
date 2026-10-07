# Siege rebuild, the interact key, and build order

**Date:** 2026-10-07

## Goal

A siege or raid that wrecks part of the base should not leave the player
re-deploying every structure by hand, or stuck in a base that can no longer
earn the materials to rebuild itself. Three pieces:

1. Destroyed structures are remembered and refiled as rebuild sites once the
   siege is over.
2. `[c]` becomes a generic "interact with the structure I'm touching" key;
   its first new interaction is committing a program to a rebuild that
   needs one.
3. Every pending build is worked in order of a derived production level, so
   a miner is rebuilt before the assembler that depends on its output.

## Decisions (from the brainstorm)

| Question | Decision |
|---|---|
| What marking does | Auto-file rebuild sites, but only once no siege is running |
| Cost and tier | Full fresh-deploy cost, tier 1 |
| Which destruction marks | Everything through `Game::damage_structure` (sieges and raids); `remove_structure` (player demolish) never does |
| `[c]` with several neighbours | One interaction → act at once; several → direction prompt |
| What "level" is | Derived production-chain depth, nothing authored |
| Ordering scope | Every build site, player deploys included |
| Ordering strictness | A workable lower-level site blocks higher-level ones |
| Tile blocked at filing time | Drop the ruin, say so in the log |

## 1. Ruins and rebuild sites (engine)

**Recording.** `Game::damage_structure` (`game/base/upkeep.rs`), on the
durability-zero branch, appends a ruin `(StructureId, x, y)` to a new
`Ruins` resource before the despawn. Not `remove_structure`: a demolish is
the player's choice, not a loss.

**Filing.** A base-tick step: if `Ruins` is non-empty and no siege is
running, drain it, and for each ruin:

- tile not free for that structure → drop it and log
  "The <name> wreck at the <side> could not be rebuilt: the ground is taken."
  (wording to match the log's existing voice);
- otherwise file a `BuildSite::new` at fresh-deploy cost, tier 1, through the
  same path `place_structure` uses (`game/base/building.rs`), minus the
  program commit.

Doing it in the tick rather than in each siege-end path means one call
site whatever ends the siege (board, tactical, off-screen), and a raid —
which is not a siege — files on the next tick. The plan names the existing
"siege is running" predicate or adds one beside `game/siege/clock.rs`.

**Program-needing rebuilds.** If `StructureDef::needs_program()` is true the
site is filed with `program: None` and a new `BuildSite::awaiting_program:
bool` set. `build_is_workable` returns false for an awaiting site, and it is
not announced dry (that latch is about materials). It renders as a pending
build with a "needs a program" marker, and cancels the usual way
(`cancel_build_request`).

**Committing.** A new `Game::commit_rebuild_program(site: Entity, program:
Entity) -> Result<(), BuildError>` validates and spends the program through
the existing `commit_for_build`, stores it on `BuildSite::program`, and clears
`awaiting_program`. Candidates come from the existing `build_candidates(&kind,
goal)`.

## 2. The interact key (engine API + app-core)

**Engine.** One new read: `Game::adjacent_interactions() ->
Vec<Interaction>`, where

```rust
pub struct Interaction { pub dir: Direction, pub kind: InteractionKind }
pub enum InteractionKind { Transfer, RebuildProgram(Entity /* site */) }
```

`Transfer` is present when today's `[c]` would open the transfer screen
(`transfer_offer` / `rack_offer` / `adjacent_depot_entities` non-empty). The
enum is the extension point: rig, siphon and splice can move onto it later,
but this change does not move them.

**App-core** (`app/playing.rs`, today's `[c]` arm):

- no interactions → `refuse_transfer()` as now;
- one → act on it: `Transfer` opens `Mode::Transfer` exactly as now,
  `RebuildProgram` opens the program picker;
- several (counted by distinct direction) → a direction prompt reusing the
  build-direction input handling; the chosen side's interaction runs, a side
  with none is refused.

The program picker reuses `Mode::BuildProgram`'s list and keys with a new
`PendingBuild` variant for "commit to an existing site", ending in
`commit_rebuild_program`. The footer for the picker says what is being
rebuilt.

## 3. Build order by level (engine)

**Level.** A pure function over the loaded structure set, computed once at
load and stored on `StructureDb`:

- a structure that produces without consuming (a `work` node) is level 0;
- any other structure is 1 + the maximum level of the producers of every
  item in its build cost and its consumed inputs (`assembles` / `strips`);
- an item with no producing structure (traded, looted) contributes nothing;
- a structure's own output in its own cost is ignored (the Mining Node costs
  Core Fragments);
- cycles collapse: members of a producer cycle share one level (iterate to a
  fixed point capped at the structure count, or SCC condense).

Structures that neither produce nor consume (walls, depots) take level from
their build cost alone.

**Scheduler.** `build_wants` (`game/base/work_orders.rs`) sorts sites by
`(level, x, y)` instead of `(x, y)`, then keeps only workable sites whose
level equals the lowest level among workable sites. Dry sites and awaiting
sites are not workable and so never block — a stuck miner cannot freeze the
base. Upgrade and recharge goals use the target structure's level.

This changes behaviour for the player's own deploys; it gets a CHANGELOG
line.

## Save format

Bincode is positional, so this is a `SAVE_FORMAT_VERSION` bump (36 → 37):

- `SaveData::ruins: Vec<RuinSave>` (kind, x, y);
- `BuildSiteSave::awaiting_program: bool`.

Level is derived at load and is not saved. Pre-37 saves stop loading, so
the release is a breaking one per `CHANGELOG.md`'s preamble.

## Testing

Engine:

- siege destruction records a ruin; no ruin from `remove_structure`;
- ruins file nothing while a siege runs and file once it ends;
- a raid's ruin files on the next tick;
- blocked tile at filing time → ruin dropped, logged, no site;
- non-program rebuild is raised by the crew to tier 1 at full cost;
- an awaiting site is never worked or announced dry until
  `commit_rebuild_program`, then is;
- level function on hand-built defs: raw producer, two-step chain, cycle,
  unproduced item, self-costing node;
- real assets: Mining Node's level < Assembly Bay's;
- scheduler: miner and assembler pending → only the miner is worked; miner
  dry → the assembler is worked;
- save → load round trip of ruins and an awaiting site (a real save/load,
  not the RON round trip).

App-core:

- `[c]` with one Depot opens Transfer unchanged;
- `[c]` beside one awaiting site opens the picker and commits;
- `[c]` with both → direction prompt, each side routes correctly.

Gates: `balance_sim` (not expected to move), `cargo clippy --workspace
--all-targets`, `cargo test --workspace`.

## Docs

CHANGELOG entry (breaking save, new build order, `[c]` interact). Seams to
update in the same change: `.claude/rules/seams-base.md` (`[c]` is now the
interact dispatcher; destruction records a ruin; build order is by level)
plus the matching `seams` skill entry and `seam:` graph node.

## Out of scope

- Moving rig / siphon / splice onto `[c]`.
- Restoring a destroyed structure's tier, stock or assigned worker.
- Authored build priorities.
