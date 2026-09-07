# Every structure eats a program

A base is built out of the things you tamed. Deploying a structure costs one
tamed program, permanently; advancing a structure a tier costs another, and the
deeper the tier the deeper the program it demands. The roster stops being a
collection and becomes a currency with a second spending door beside fusion.

**One branch, one release.** The rule, the picker and the refund are one
mechanic — a rule with no picker is a refusal nobody can satisfy, and a picker
with no refund is a cancel that quietly robs you.

## Context

Today a build order costs materials and nothing else. `Game::place_structure`
(`game/base/building.rs:11`) walks a short ladder of gates — research
(`structure_unlocked`), the Home-first rule, locale, floor, occupancy,
`max_deployed` — prices the bill through `Game::structure_build_cost`
(`game/catalog.rs:834`), and then spawns a **request**, not a structure: a
`BuildSite` entity on the cell. Materials are hauled to it by the build crew and
`Game::spawn_structure` (`building.rs:236`) raises the thing when the bill is
met. `Game::cancel_build_request` (`building.rs:356`) pulls the request and
returns what was delivered.

The load-bearing fact for this spec is that **upgrades take the same road**.
`Game::upgrade_structure` (`building.rs:568`) does not mutate a structure in
place; it files a `BuildSite` carrying `BuildGoal::Upgrade { to_tier }`, and
`components.rs:2005` states why: *"the rules for supply, progress and cancel are
identical for a deploy and an upgrade, so exactly one step branches on this."*

So there are not two flows to gate. There is one `BuildSite`, entered by two
doors, and a program attaches to it exactly once.

On the other side, a tamed program is a full ECS entity marked
`Tamed { owner }`. Two of its components matter here:

- `ZonePortal(u32)` (`components.rs:139`) — the zone it was tamed in, fixed at
  spawn and never changed by carrying it deeper. It is the trailing number in
  the displayed name (`Scrapper 2`), written by `Game::zone_tagged_name`.
- `ProgramId(u32)` (`components.rs:1470`) — the save-stable name for one owned
  program, minted at `Game::roster_parts`, *"the single barrier all four doors
  into the roster pass through."*

A fresh run owns **zero** programs; one is granted only as an achievements
profile reward (`lifecycle.rs:2257`). That single fact decides the Home
exemption below.

## Decisions taken

| Question | Decision |
|---|---|
| Fate of the program | Consumed. The entity is despawned. |
| What gates the tier | `ZonePortal >= tier`. A floor, not a match — a zone-5 program may raise a Mk1. |
| Home | Exempt at **every** tier, deploy and upgrade alike. |
| Upgrades | Each tier eats another program, at that tier's depth. |
| Spend moment | Committed at the picker; spent for good at completion. |
| Cancel an unbuilt order | Refunds the program. |
| Deconstruct a standing structure | Returns nothing. The program is long gone. |
| Picker position | After the direction step, as the final confirm. |
| No qualifying program | The row greys and states the requirement. |

## 1. The rule

One number decides everything: the tier this `BuildSite` will produce.

```
BuildGoal::New                  -> 1
BuildGoal::Upgrade { to_tier }  -> to_tier
```

A program qualifies iff `ZonePortal >= that tier`. Home is exempt outright and
asks for nothing.

Two engine functions, both new, both on `Game`:

- `program_tier_required(goal: BuildGoal) -> u32` — the table above.
- `programs_for_build(tier: u32) -> Vec<PetInfo>` — `owned_pets()` filtered by
  `zone_tier(e) >= tier`, keeping `owned_pets`' existing sort so the picker
  agrees with every other roster screen about order.

Because every structure deploys at `StructureTier(1)`, the deploy requirement is
uniformly "own any program". Depth only bites on upgrades. This is deliberate
and was confirmed in design: the deploy picker is a *what are you willing to
spend* choice, not a *can you afford it* one. No new asset field; `tier` is the
only number in the rule.

`upgrade_ceiling` (`building.rs:499`) already clamps tiers to `ZoneLevel`, so
the program requirement never demands a zone the player could not have reached
— you cannot be offered a Mk3 upgrade outside zone 3, and a zone-3 program is
therefore obtainable whenever the requirement can be asked. The two rules agree
by construction rather than by coincidence; a test must say so, because if
`upgrade_ceiling` ever loosens, this requirement becomes unsatisfiable in
silence.

## 2. The lifecycle: committed, then spent

| Moment | Program |
|---|---|
| Picker confirmed | **Committed.** Entity despawned, snapshot rides the `BuildSite` |
| `cancel_build_request` | **Refunded.** Snapshot rehydrated onto the roster |
| Site destroyed with its cell | Refunded — **verify this path exists** |
| `spawn_structure` completes | **Spent.** Snapshot dropped; nothing survives |
| `remove_structure` on a standing structure | Nothing to return |

The player-visible story is one line: *you spend it when the crew finishes, you
get it back only if you call the job off*.

`building.rs:250` implies a site can also be *"wiped with the cell it stood
on"*. If that path does not run through `cancel_build_request`, it is a second
place a program can be destroyed with no refund, and the plan must route it
through the same door rather than leave a silent hole.

Retiring the program at commit follows fusion's teardown verbatim
(`party.rs:1004-1007`): retain it out of `Party`, then `world.despawn`. The plan
must confirm the three loose ends fusion does not itself clean, because a
committed program can be in states a fusion sacrifice usually is not:

- the **wielded** program (`CreatureSave::wielded`) — must be cleared on commit
  and restored on refund, or the run wields a despawned entity;
- a **standing job** or in-flight `Task` in base space;
- a **cronjob** whose target is this program.

Refunding goes back through `Game::roster_parts` — the one barrier into the
roster — and restores the snapshot's original `ProgramId`, so memories keyed to
that program survive the round trip rather than coming back to a stranger.

## 3. Where the program is kept

`BuildSite` gains one field:

```rust
/// The tamed program committed to this order, retired from the roster the
/// moment the request was filed. Carried as a value and never an `Entity`:
/// entity ids are not stable across a save round trip, the same reason
/// `HopperEntry` carries a `DownedProgram` rather than a reference.
///
/// `None` only for a Home, the one exempt structure.
pub program: Option<CreatureSave>,
```

**Why `CreatureSave` and not a purpose-built snapshot.** A tamed program carries
twenty-odd components — `Stats`, `Experience`, `Potential`, `Rarity`,
`FusionCount`, `Refactors`, `PurchasedTiers`, `KernelRing`, `Talents`,
`Routines`, `Equipment`, `CustomName`, memories, needs, disposition.
`CreatureSave` (`save.rs:314`) is already their complete, lossless, versioned
description. A parallel `CommittedProgram` would duplicate every one of those
fields and drift — precisely the argument `BuildGoal` already makes against a
second component: *"the copy that drifts is the one nobody runs."*

The cost is a deliberate coupling: `components.rs` gains a dependency on
`save.rs`. That is the trade, and it is named here so the review does not
rediscover it as a surprise.

This needs two functions extracted from the bulk save/load loops, which today
have no per-creature seam:

- `Game::creature_save_for(&mut self, e: Entity) -> Option<CreatureSave>`
- `Game::spawn_creature_from_save(&mut self, c: &CreatureSave) -> Entity`

Both are lifted out of existing loop bodies rather than written fresh, and the
bulk paths must then *call them* — two implementations of "what a creature is"
is the one outcome this refactor must not produce. The load-side extraction is
the delicate half: the loop threads `next_program_id` minting and `wielded`
resolution through its iterations, and those belong to the caller, not to the
extracted function.

## 4. The picker

One new `Mode::BuildProgram`, entered from both doors:

```
deploy:  Mode::Build -> Mode::BuildDirection -> Mode::BuildProgram -> filed
upgrade: structure list             -> Mode::BuildProgram -> filed
```

App state carries the pending order until the picker confirms it. `App` already
holds `pending_structure` (`lib.rs:1987`); it gains the direction and the goal,
so the picker knows which tier it is filtering for and what to file on confirm.

Rows reuse the roster format verbatim. `companion_row_lines`
(`gui/src/render/party.rs:243`) already renders exactly the *"normally displayed
data"* the design calls for:

```
[a] Scrapper 2 Lv7 - HP 44/44  ATK 12  MIT 8%  PWR 30  (blade, plate) [Sharp]
```

The zone is already in the name, so the qualifying number is visible without a
new column. The picker calls `draw_popup` (`gui/src/render/popup.rs:636`) like
the five screens that already do this — fuse, develop, refactor, routines,
extract. **No shared picker abstraction.** There is one screen here, not two,
and a lone helper among five open-coded siblings costs more than it buys.

Lowercase letters stay row selectors, per the standing rule. The picker's rows
*are* selectors, so this screen needs no uppercase action at all.

The confirm must say the thing out loud. A program leaving the roster forever on
a keypress is the harshest irreversible action in the base loop, and the prompt
should name the program and the word *permanently*.

### The trap this screen will fall into

`ALL_MODES` is hand-written and the draw match ends in `_ => {}`. A new `Mode`
variant that is not added to both ships as a **blank screen** that compiles
clean. `ALL_MODES` lives in `app-core/src/lib.rs`, `gui/src/render/mod.rs`,
`gui/src/render/creation.rs`, `gui/src/render/sprite_forge.rs` and
`app-core/src/tests/tools.rs`; its length is a known semantic merge conflict.

## 5. The build menu and the upgrade prompt

Both need to state the requirement before a keypress is spent.

- **Build menu** (`build_menu_rows`, `gui/src/render/building.rs:40`): the
  requirement is uniform — own a program or you cannot deploy anything. One
  refusal line on the screen, not a per-row tag: *"Deploying costs a tamed
  program. You have none."* Rows grey together.
- **Upgrade prompt**: per-row, because the requirement varies with the target
  tier. A row reads `Fabricator Mk2 -> Mk3 - 12 core fragment · needs a zone 3
  program`, and greys when the roster has nothing that deep.

Greying, not hiding: a structure that silently vanishes from a menu reads as a
bug, and the rule has to teach itself somewhere.

## 6. Save

`BuildSiteSave` (`save.rs:790`) gains the matching field:

```rust
#[serde(default)]
pub program: Option<CreatureSave>,
```

Additive behind a default, so it costs **no `SAVE_FORMAT_VERSION` bump** — the
same precedent `goal`, `hopper` and `power_fuel` each state in their own doc
comments. A file written before this feature loads every site with `None`, which
is exactly what that run had.

A RON round-trip test cannot catch a field that fails to serialise. This needs a
real save-to-file, load-from-file test that files an order, saves, loads, and
cancels — and gets the program back with its `ProgramId`, level, equipment and
custom name intact.

## 7. Where the code goes

| File | Change |
|---|---|
| `engine/src/components.rs` | `BuildSite::program` |
| `engine/src/game/base/building.rs` | commit in `place_structure` / `upgrade_structure`; refund in `cancel_build_request`; drop in `spawn_structure` |
| `engine/src/game/party.rs` | `programs_for_build`; refund via `roster_parts` |
| `engine/src/game/catalog.rs` | `program_tier_required` |
| `engine/src/save.rs` | `BuildSiteSave::program`; per-creature save/load seam |
| `engine/src/game/lifecycle.rs` | extract `spawn_creature_from_save` from the load loop |
| `app-core/src/app/building.rs` | `Mode::BuildProgram` handling; pending order state |
| `app-core/src/app/input.rs` | dispatch entry |
| `app-core/src/lib.rs` | `Mode` variant, `ALL_MODES`, pending fields |
| `gui/src/render/building.rs` | picker screen; menu refusal; upgrade row requirement |

## 8. Tests

Engine:

1. Deploying without a program is refused, and the message names what is needed.
2. Deploying with a program despawns it and files a site carrying its snapshot.
3. A zone-5 program raises a Mk1 — `>=`, not `==`.
4. Upgrading to Mk3 refuses a zone-2 program and accepts a zone-3.
5. Home deploys with no program, and Home upgrades to Mk2 with no program.
6. Cancelling an unbuilt order returns the program with `ProgramId`, level,
   equipment, custom name and refactor count intact.
7. Completing the build leaves nothing to return; `remove_structure` afterwards
   refunds materials and no program.
8. Committing the wielded program clears `wielded`; the refund restores it.
9. Committing a party member removes it from `Party`; the refund does not
   silently re-add it to a full party.
10. Save → load → cancel returns the program (file round trip, not RON).

App-core:

11. The picker lists only qualifying programs, in `owned_pets` order.
12. `Mode::BuildProgram` is in `ALL_MODES` and draws something.

Each test must fail with the change removed. A gated consequence can be green
and unreachable — test 4 in particular has to prove a zone-3 program is
*obtainable* at the point the Mk3 upgrade is offered, not merely that the
comparison works.

## Open, deliberately

- **No per-structure program floor.** `StructureDef::min_program_zone` was
  considered and rejected for now: it needs authoring across 31 files to express
  something the tier number already says. If deploys later need to differ by
  structure, that field is the shape it takes.
- **Balance is unmeasured.** This adds a program sink beside fusion with no
  facing source. Whether the taming rate supports a base is a play question, and
  `balance_sim` models no such thing.
- **No structure remembers what it ate.** Naming the program on the finished
  structure is flavour this spec does not spend budget on.

## Before implementation

- The load-loop extraction is the risk in this change. Save/load is the
  subsystem where a regression is silent and arrives a week later.
- Nothing here has been on a screen. A green suite is not evidence of play, and
  the picker is a screen an agent cannot see.
