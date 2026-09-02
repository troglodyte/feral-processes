# Research cost and zone gate

**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header. See `../../INDEX.md`.

## The problem

The research tree finishes too early. Players have everything researched
long before the run is over, so the tree reads as a checklist to clear
rather than a set of decisions.

Two things cause it, and only one of them is the price.

**The tree is cheap.** Twenty-one nodes, costs 8–50, **561 Research Data
for the whole thing**. Research Data has exactly one source — a Research
Node worked by a posted program — and it is the game's only *banked* item,
paying a deliberate flat 1 per cycle. A cycle is 14 ticks scaled by the
worker's speed, times a success roll that runs 50% at Mk1 and 90% at Mk5.
Two posted programs clear the entire tree in roughly 6,000 worker-ticks.

**Nothing about the tree is gated on progress.** `ResearchDef` carries
`cost` and `requires` and nothing else. Every node is buyable from turn one
if you are willing to wait, and Research Data survives a breach — a point
the sector-traits spec already made against the old design of a zone:

> Every structure, item, recipe and research node is available from turn
> one, and Research Data survives a breach — so the tech tree could in
> principle be finished without ever breaching.

So the tree is paced by patience alone. Raising prices alone would not fix
that; it would make the same checklist take longer, which is the complaint
with a bigger number on it.

## What changes

Three parts, in the order they matter:

1. **A hard zone gate.** A research node may declare the zone it becomes
   available in. Below that zone the node is visible, explained and
   unbuyable at any price.
2. **A re-priced ladder.** The bootstrap tier stays roughly where it is;
   the mid and deep tiers go up. Total 561 → 1258.
3. **The tap is untouched.** Research Data still pays a flat banked 1 per
   cycle.

Part 3 is a decision, not an omission. `LEECH_YIELD_BONUS`'s doc comment
records that the flat 1 "is the whole of what keeps an uncapped bank honest
against a fixed research ladder" — the bank has no ceiling, so the ladder is
the only thing bounding it. Giving the tap a zone term was considered and
rejected: it would pay more exactly where the gate has just released more to
buy, cancelling out the pacing this change exists to create.

## The gate

`ResearchDef` gains one field:

```ron
// Optional; defaults to 0, meaning available from turn one. The zone the
// player must have reached before this node can be researched.
min_zone: 3,
```

`#[serde(default)]`, so every existing file and every third-party mod keeps
parsing untouched, and a mod that never heard of the field ships a fully
ungated tree exactly as it does today.

The precedent is already in the codebase and already argued in `CLAUDE.md`:
`Game::upgrade_ceiling` is `min(def.max_tier, ZoneLevel)`, and reaching zone
*N* is what unlocks Mk*N*. Three rules carry over from that entry, and each
is a way this feature could go wrong:

**A gated node stays listed.** `upgrade_structure`'s entry records why a
structure at its zone ceiling is not filtered out of the menu: filtering
would delete the whole Upgrade row for all of zone 1, and a player who had
never breached would never learn upgrading exists. Identical here. The
zone-3 tier sitting visibly in the menu at zone 1 *is* the feature — it is
the reason to go breach.

**The gate is checked before the cost.** `upgrade_structure` checks both
ceilings before the materials check "so the player is never sent to find
fragments they couldn't have spent". Same argument: a player at zone 1
looking at a zone-3 node must be told about the zone, not told they are
short on Research Data.

**The gate is on buying, not on having.** It guards `unlock_research` only.
`resources::Research` stores the ids already unlocked and is untouched, so a
save from before this change keeps every node it had bought, whatever zone
the player is standing in. There is no migration and no
`SAVE_FORMAT_VERSION` bump.

### The gate must never block a breach

This is the one way the feature could softlock a run: gate a node that
unlocks a structure the player needs in order to reach the zone that
ungates it.

It is safe today, by accident rather than by design. The structure that
opens the next zone is `portal` (`assets/structures/portal.ron`, 10 Portal
Fragments), and it is named in no research file's `unlocks_structures`.
`assets/research/README.md` states the rule that makes that meaningful: *a
structure named by no research file is buildable by default*. That is also
how the Home, Mining Node, Research Node and Recharger Node stay available
from turn one.

The invariant to assert is not "no node may name the portal" — a mod is
free to put it behind research — but the weaker, sufficient one:

> A node naming `portal` in `unlocks_structures` must not carry a
> non-zero `min_zone`.

Gating the portal behind the zone it is needed to reach is the softlock;
merely researching it is not. The assertion is vacuously true today, which
is the point — it is currently safe by accident, and "safe by accident" is
exactly what a later content edit removes without anyone noticing. See
**Testing**.

## The bands

Nine bootstrap nodes stay ungated. The tree opens fully at zone 3, which is
the pacing target: everything is *available* by zone 3.

| Zone | Node | Today | Proposed |
|---|---|---|---|
| — | `automation` | 8 | 8 |
| — | `power_grid` | 10 | 10 |
| — | `commerce` | 12 | 14 |
| — | `self_exec` | 12 | 14 |
| — | `fortification` | 15 | 18 |
| — | `field_ops` | 16 | 20 |
| — | `armor_bench` | 18 | 24 |
| — | `weapon_bench` | 18 | 24 |
| — | `routine_fabrication` | 20 | 26 |
| | *subtotal* | *129* | *158* |
| 2 | `overclock` | 22 | 45 |
| 2 | `firewall` | 22 | 45 |
| 2 | `neural_amp` | 25 | 55 |
| 2 | `runtime_patching` | 28 | 60 |
| 2 | `adaptive_plating` | 32 | 70 |
| 2 | `program_refactoring` | 34 | 75 |
| | *subtotal* | *163* | *350* |
| 3 | `monofilament` | 40 | 110 |
| 3 | `ablative` | 40 | 110 |
| 3 | `cortex` | 45 | 125 |
| 3 | `deep_analysis` | 46 | 130 |
| 3 | `kernel_privileges` | 48 | 135 |
| 3 | `address_translation` | 50 | 140 |
| | *subtotal* | *269* | *750* |
| | **total** | **561** | **1258** |

The opening tier is nearly untouched, deliberately: the complaint is that
the tree *finishes* too early, not that the first bench arrives too early,
and a base that cannot stand up its first machine is a worse opening rather
than a slower one.

**The bands are monotone in prerequisite.** Every node's `min_zone` is
greater than or equal to that of each node it `requires`. A node gated
below its own prerequisite would be a gate that never fires — the prereq
lock would always outlive it — and reads in the menu as a reason that
disappears without the node becoming available. Asserted; see **Testing**.

**The gate and the tap compound without extra tuning.** `upgrade_ceiling`
caps a Research Node at Mk1 in zone 1 (50% success), Mk2 in zone 2, Mk3 in
zone 3 — so the band the player can buy earliest is also the band they earn
slowest, and each breach speeds up the bank at the same moment it releases
more to spend it on. No new constant expresses this; it falls out of two
mechanics that already exist.

A consequence worth knowing rather than enforcing: after repricing, the
three bands no longer overlap in cost (band 1 tops out at 26, band 2 starts
at 45, band 3 at 110). `ResearchDb::all()` sorts cheapest-first, so the menu
happens to read as three clean tiers. That is a property of these numbers,
not a rule — a mod may price wherever it likes and nothing refuses it.

## What the menus show

`views::ResearchState::Locked` gains the zone:

```rust
Locked { missing: Vec<String>, min_zone: Option<u32> }
```

A separate `ZoneLocked` variant was considered and rejected: a node can be
prereq-locked *and* zone-locked at once (`cortex` at zone 1 with
`neural_amp` unresearched), and a separate variant forces an arbitrary
precedence between the two while losing the other reason. Pushing `"Zone 3"`
into the existing `missing: Vec<String>` was also rejected — it renders
correctly and tests as a string, which is how a display and a refusal drift
apart.

`Game::research_nodes` computes the two independently and reports both.
`crates/gui/src/render/progression.rs` is the one place a locked node is
labelled; it joins the reasons it is given, so a doubly-locked node reads
`(needs Neural Interfacing, Zone 3)`. Ordering within `research_nodes`'s
Available/Locked/Unlocked sort is unchanged — a zone-locked node is Locked.

`unlock_research`'s refusal order becomes: game-over/battle, unknown id,
already researched, **missing prereqs, min_zone**, cost. The prereq message
fires before the zone message when both apply, which is the existing order
extended rather than rearranged.

## Not in scope: attack routines by zone

The original request paired this with an observation — that zone 2 wants
group attacks and zone 3 wants all-target attacks, and that research could
gate on that. It cannot, as things stand, and the reason is worth recording
so the idea is not re-derived from scratch.

Seven of the twenty-one nodes grant abilities. Every routine they teach is
`OneAlly` or `WholeParty` support, with exactly one exception:
`kernel_privileges` → `null_route` (`AllEnemies`). **There is no
`WholeEnemyGroup` attack anywhere in the research tree.** Group and
all-target attacks reach the player's side of a fight through *species
kits* — a companion's `.ron` file — not through research.

So there is nothing to gate. What this spec does do is put
`kernel_privileges` in the zone-3 band, which is the one case the
observation actually names.

Making the research tree a source of attack routines is a content change of
its own: new ability-granting nodes, priced and banded, and a balance
argument about what it does to a fight that `balance_sim` cannot make (it
models no abilities at all). Separate spec.

## Testing

Engine tests live in `crates/engine/src/tests/research.rs`; schema tests in
`crates/engine/src/research.rs`'s own module.

Behaviour:

- A node above the player's zone reports `Locked` carrying that zone.
- `unlock_research` refuses it even with the cost banked and prereqs met —
  and **the Research Data is not spent**. The second half is what fails if
  the refusal is ever moved after the payment.
- Breaching to the node's zone makes it `Available`. This is the pairing
  that fails when the gate is removed, per the standing rule that a test
  passing with the fix deleted is not coverage.
- A node can report a missing prereq *and* a zone at once.
- The zone refusal fires before the cost refusal: a broke player at too low
  a zone hears about the zone.
- A zone-gated node still appears in `research_nodes()` rather than being
  filtered out.

Schema:

- `min_zone` defaults to 0 when the field is absent, and an existing file
  with no `min_zone` loads unchanged.

Censuses over the real assets:

- **No node is gated below its own prerequisite.** Catches a band edit that
  makes a gate unreachable.
- **Nothing needed to breach is locked behind research.** No node naming
  `portal` in `unlocks_structures` carries a non-zero `min_zone`. This is
  the softlock guard and the test most worth having: it is currently true
  by accident, and one content edit could remove it silently. Assert it
  against the loaded `ResearchDb` rather than by reading the files, so a
  node dropped at load time cannot make it pass for the wrong reason.

Existing tests that move with this change:

- `research.rs::the_shipped_tree_loads_clean` asserts `cortex` costs 45.
- `crates/app-core/src/tests/research.rs::picking_an_unaffordable_research_
  node_reports_why_and_stays_open` is priced against the current ladder and
  should be re-checked.

Gates: `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`.
`balance_sim` models combat and has no research term, so it neither gates
this nor is disturbed by it — but it is cheap and should still be run,
because the item-price censuses live in the same suite.

## What this is blind to

**Whether 1258 actually lands at zone 3.** Research pace is measured in
ticks; zone pace is measured in Portal Fragments from Stack lair guardians
(`STACK_BOSS_PORTAL_FRAGMENT_DROP`, 4–8 × depth, from `STACK_LINKS_PER_ZONE`
= 3 stacks a zone). The two are independent axes, so no cost number can
*make* the tree finish at zone 3 — a player who rushes stacks arrives with
less banked than one who grinds zone 1. The costs set a time budget; the
gate is what does the structural pacing, and it does it exactly.

How many ticks a run through three zones takes has never been measured.
`docs/measurements/` has nothing on it, `balance_sim` models no base, and
there is no zone-3 template in `dev-saves/`. The arithmetic here says
roughly 13,800 worker-ticks with two posted programs — three would be the
player's entire `BASE_PET_CAPACITY`, leaving no party at all.

This is deliberately left as data. Each band is one multiplier away from
being retuned in pure `.ron`, with no code change and no release beyond a
patch. If the pacing wants a real answer rather than an estimate, the way
to get one is a `dev-saves/` capture of a played run at zone 3 and a
`docs/measurements/` file recording its tick count — which is worth doing
once and would then serve every future economy question, not just this one.

## Files

- `crates/engine/src/research.rs` — `min_zone` field, schema test
- `crates/engine/src/views.rs` — `ResearchState::Locked` gains `min_zone`
- `crates/engine/src/game/unlocks.rs` — gate in `research_nodes` and the
  refusal in `unlock_research`
- `crates/gui/src/render/progression.rs` — the locked-node label
- `assets/research/*.ron` — 21 files: costs, and `min_zone` on twelve
- `assets/research/README.md` — schema, plus the two rules above
  (monotone bands, and nothing needed to breach behind a gate)
- `crates/engine/src/tests/research.rs` — the behaviour tests
- `CHANGELOG.md` — a `## 0.8.15` section at merge

**Version: 0.8.15.** No save-format bump, so by `CHANGELOG.md`'s policy
this is a patch — at `0.x`, breaking means a player's save stops loading,
and a `#[serde(default)]` field on a `.ron` def is not that.
