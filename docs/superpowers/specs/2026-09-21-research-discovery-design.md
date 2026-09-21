# Discovering research by study

**Date:** 2026-09-21
**Status:** designed, not implemented.
**Todo:** no number; raised in conversation.
**Predecessor:** `archive/specs/2026-09-20-research-station-study-design.md`,
which built the Research Station, its pen, `requires_subject` and the spend.
This spec is its sequel and changes none of it.

Research becomes two steps. Today a node is visible from turn one and the
only question is whether you can afford it. After this, a node that is
**discoverable** is invisible until the base has *found* it: a researcher
posted at the Research Station, with a tamed program pinned in the pen,
studies. Each study attempt is a roll. On a hit the base discovers a node —
a log line, a popup, and the node appears in the research tree, where it is
then researched exactly as it is today, subject spend included.

The point is that the tree stops being a menu the player reads on turn one
and becomes something the base uncovers. It also gives the Research Station
something to do between projects: today a Station with no project selected
completes its cycle, lands nothing, and says nothing, by deliberate design
(`systems::cycle_line`). That dead cycle becomes the study.

## Decisions taken in brainstorming

| Question | Answer |
|---|---|
| What does a discovery cost the subject? | Nothing. The pinned program survives every attempt and every discovery. It is still spent at `settle_research` when the node is actually researched — unchanged. |
| What picks the node discovered? | Uniform random over the eligible pool. The subject is fuel, not a lens; which program is pinned does not steer what is found. |
| Study or advance a project? | One or the other. A Station with a project selected feeds it as today; a Station with **no** project selected studies instead. |
| What is eligible to be found? | Undiscovered, `discoverable`, prerequisites researched, zone reached. A discovery is therefore always immediately researchable. |
| Which nodes are hidden? | Whichever author a new `discoverable` field. **Not** `requires_subject`. |
| Does a discovery pop up? | Yes — `NotificationKind::ResearchDiscovered`, `Repeat::Always`. A failed attempt logs and does not pop up. |

### Why `discoverable` is its own field and not `requires_subject`

22 of the 27 shipped nodes carry `requires_subject`, and the 5 that do not
are exactly the zone-1 infrastructure spine (Automation, Power Grid,
Teardown, Fortification, Isometric Commerce). So `requires_subject` looks
like it already means "hidden until found". It does not: the two coincide by
authoring accident, not by design.

They are different questions. *Does buying this cost a program?* and *is
this hidden until found?* One flag answering both makes a future node that
costs a subject but should be visible from turn one — a tutorial rung — and
one that is hidden but free to buy, equally unrepresentable. This repo has
recorded that trap before under other names.

Splitting them also buys the extension the feature is for:

- **A second discovery source costs nothing.** `DiscoveredResearch` is a set
  of ids. Study writes into it, but the set is innocent of study. A Stack
  find, a contract reward, a settlement selling a schematic is a new *caller*
  of `Game::discover_research` — not a new field, not a new gate, not a save
  change. `resources::DiscoveredRoutines` is shaped this way already:
  extraction writes it and nothing about it says extraction.
- **Which nodes a source may reveal is the source's filter**, at that source's
  own call site, never a field on the node.
- **The two flags compose.** `discoverable: true, requires_subject: true` is
  the case described above. `discoverable: false, requires_subject: true`
  keeps `routine_fabrication` and `program_refactoring` exactly where they
  are today, so the routine tree and companion fusion do **not** move behind
  a dice roll. If that is later wanted it is one line in two `.ron` files.

## 1. Data

`research::ResearchDef` gains one field:

```rust
/// Hidden from the base tree until the base has discovered it — see
/// `resources::DiscoveredResearch` and `Game::settle_study`.
#[serde(default)]
pub discoverable: bool,
```

`#[serde(default)]` for the standing rule: every existing file and every
mod keeps parsing untouched, and the default is `false`, so **nothing hides
itself by surprise**. A field defaulting the other way would make a modder's
tree vanish.

`resources` gains **one** resource:

```rust
/// Which research nodes the base has discovered. Ids rather than anything
/// resolved, and a `BTreeSet` for `KnownRoutines`' own reason: the save
/// writes this set out and a `HashSet`'s order would make the bytes differ
/// run to run.
#[derive(Resource, Default)]
pub struct DiscoveredResearch(pub BTreeSet<ResearchId>);
```

Study progress is **a field on `ActiveResearch`**, not a resource of its own:

```rust
pub struct ActiveResearch {
    pub id: Option<ResearchId>,
    pub progress: HashMap<ResearchId, u32>,
    /// Research currency banked toward the next study attempt — where the
    /// payout lands while `id` is `None`.
    pub study: u32,
}
```

with a `credit_study(amount, cap)` that is `credit`'s shape exactly.

**This is the load-bearing structural choice of the feature and it is worth
stating why.** `ActiveResearch` is already the answer to *where research
currency lands*, and this adds the second place it can land rather than a
second type to ask. Three things fall out of that:

- **`deliver_payout`'s signature does not change**, so neither of its two
  callers changes, and neither `SystemParam` bundle gains a parameter. Both
  bundles carry a doc saying `research` was bundled in the first place
  because the parameter list had reached clippy's threshold; a second
  resource would push straight back on that.
- **One new resource registration instead of two.** A new `Resource` shifts
  bevy's query iteration order, which this repo has recorded as a cause of
  seeded tests moving. One shift is unavoidable — `DiscoveredResearch` has
  to exist — and the plan should expect seeded outcomes to move once and
  budget a pass for it. Two would be a self-inflicted second.
- The save field is `study_progress` either way.

`DiscoveredResearch` stays its own resource because it is the extension
point: a second discovery source writes into it and touches nothing else.

`save::SaveData` gains `discovered_research: Vec<ResearchId>` and
`study_progress: u32`, both `#[serde(default)]`. The save is field-named
RON and this is additive, so **no `SAVE_FORMAT_VERSION` bump**: an older
save loads with nothing discovered, which for a run in progress means the
discoverable half of its tree hides itself. That is the correct reading of
the new rule and not a migration.

## 2. Accrual — the bevy half

`systems::deliver_payout`'s research branch becomes — with no change to its
signature, since it already holds `&mut ActiveResearch`:

```rust
if items.research_currency() == Some(resource) {
    if research.id.is_none() {
        research.credit_study(payout, tuning::STUDY_ATTEMPT_DATA);
        return 0;
    }
    let cap = /* the active def's cost, as today */;
    return research.credit(payout, cap);
}
```

Gated on `research.id.is_none()` and not on `landed == 0`, because those are
two different states: a project sitting *at* its cost also lands nothing and
will settle next tick, and feeding the study from it would make "study or
advance" a lie for one tick an attentive player could farm.

It returns `0`, so `cycle_line` stays silent exactly as it does today — the
study speaks for itself in §3 rather than through a cycle line that would
say "extracted 0 Research Data".

The accrual is **not** gated on a subject being pinned. The bevy system has
no `Game` to ask, and every judgement in this feature belongs in one place.
Progress caps at a single attempt, so a Station that studies with an empty
pen banks one attempt and then holds — it does not stockpile, and the
player-facing story ("nothing happens until you pin something") still holds
because the *roll* is what a subject gates.

## 3. The roll — the `Game` half

`Game::settle_study`, called from `game/turn.rs` immediately after
`self.settle_research()`. It is a `Game` method for `run_repair_bays`' own
reason: it draws `GameRng`, names the node, logs through `log_base` and
raises a notification, none of which a bevy system can reach.

```
if research.study < STUDY_ATTEMPT_DATA         -> return   (not yet an attempt)
if pinned_subject().is_none()                 -> return   (hold the attempt)
let pool = eligible();
if pool.is_empty()                            -> return   (hold the attempt)
research.study = 0;                                       (the attempt is spent)
if !rng.random_bool(STUDY_DISCOVERY_CHANCE)   -> log the miss; return
discover_research(pool[rng.random_range(..pool.len())]);
```

Three early returns **hold** the progress rather than spending it. A full
bar with nothing to find, or with an empty pen, is banked and fires the
instant the condition clears. That is what makes a discovery legible as a
consequence of the player's action rather than of a timer they cannot see.

`eligible()` is: `tree == Base`, `discoverable`, not in
`DiscoveredResearch`, not already researched, every `requires` satisfied
(`prereq_satisfied`), and `research_zone_gate` clear. It draws no RNG and is
its own function, so a second discovery source in future filters the same
pool or narrows it.

`Game::discover_research(id)` is the one door a discovery is written
through, `Game::remember`'s rule:

```
DiscoveredResearch.insert(id)
log_base("The study uncovers {name}.")
notify_filled(ResearchDiscovered, &[("name", ..), ("description", ..)], None)
```

The miss line is `log_base("The study turns up nothing.")`. Both follow
`settle_research`'s own lines ("Research project started: {name}.",
"Research complete: {name}.", "{name} is spent in the study.") in being
base news rather than a plain `log()`, which is `MessageKind::Info` and is
pruned by `retain_outcomes_since_battle`.

The miss line repeats, and `resources::condense` already folds repeats on
all three log surfaces, so it needs no rate limit of its own.

## 4. Visibility

One predicate, `Game::base_node_visible(def)`:

```rust
!def.discoverable
    || self.is_researched(&def.id)
    || self.world.resource::<DiscoveredResearch>().0.contains(&def.id)
```

`is_researched` is in there for the routine tree's own "researched means
known" rule: a node already bought is never hidden by a later change to what
gates it — a save from before this feature, or a `.ron` edit that adds
`discoverable` to something the player already owns.

`listed_research`'s `Base` arm filters through it. Both `research_nodes` and
`research_graph` go through `listed_research` already, so the menu and the
flow chart cannot disagree.

**`select_research` refuses an undiscovered base node too**, beside the
existing routine-tree refusal and for its stated reason: leaving a node off
the menu is not enough, because an id typed into a save editor or a stale UI
row could otherwise buy something the player was never shown. The sentence
is `"Unknown research."` — the same one the routine tree gives, because
telling the player a node exists is precisely what a hidden node must not do.

**`has_research_tree(Base)` is unchanged.** It asks whether the catalogue
holds any `Base` node, not whether anything is listed right now, so the base
tree's menu row stays reachable. The 5 non-discoverable nodes mean the base
tree is never empty in shipped content anyway, but the rule holds for a mod
whose whole tree is discoverable.

**`PinMark::Strained` is unchanged and that is deliberate.** A subject under
study is not being spent — discovery is free — so it stays `Settled`. Only a
subject whose project is earning rattles, which is `strained_subject`'s
existing answer.

## 5. The popup

`NotificationKind::ResearchDiscovered`, a direct sibling of
`ResearchComplete`:

```rust
NotificationKind::ResearchDiscovered => NotificationDef {
    title: "Research Discovered",
    body: "{name}\n\n{description}",
    sprite: None,
    glyph: 'R',
    color: GlyphColor::Cyan,
    repeat: Repeat::Always,
},
```

`Repeat::Always` because there are up to 20 discoveries in a run and each is
news. The body is the node's own authored `name` and `description`, so there
is no second copy of that prose to drift — the achievement seam's argument.

`latch_key` is unreachable for an `Always` kind but the match is exhaustive,
so it gets an arm like its siblings.

Timing needs no special case: a discovery resolves on a tick with the player
on the map, which is `Mode::Playing` — the one equality the whole
notification system's timing rule is built on.

The screen's height census (`the_tallest_shipped_notification_fits_its_
screen`) is satisfied by construction: `ResearchComplete` already renders
these exact description strings into this exact panel.

## 6. Tuning

Two constants in `tuning.rs`, in the research section:

- `STUDY_ATTEMPT_DATA: u32` — Research Data banked per study attempt.
- `STUDY_DISCOVERY_CHANCE: f64` — the roll.

Priced together against `systems::node_payout` and the Station's cycle
length so that "a few failed attempts" is a handful of minutes at the
keyboard, not an hour. The implementation plan must state the arithmetic
it picked and what cycle length it assumed; this is the one number in the
feature that only play can confirm, and nothing here can be played by an
agent.

## 7. Content

20 files get `discoverable: true`: every node carrying `requires_subject`
except `routine_fabrication.ron` and `program_refactoring.ron`, which stay
visible so the routine tree and fusion keep arriving when they do today.

At zone 1 with nothing researched the eligible pool is **empty** — every
discoverable node either requires `automation` (free, visible) or is
`min_zone >= 2`. So the opening is: research Automation from the visible
spine, then the pool becomes `{armor_bench}`, then `weapon_bench` behind
`routine_fabrication`. The tree unfolds one or two rungs at a time with no
extra gating invented for it.

`assets/research/README.md` documents the new field in the same change, per
the standing schema-docs rule.

## 8. Tests

Engine, in `tests/research.rs` unless noted:

1. A discoverable node is absent from `research_nodes` and
   `research_graph` until discovered, and present after.
2. `select_research` refuses an undiscovered discoverable node with
   `"Unknown research."`, and refuses it **before anything is written** —
   no active project, no work orders filed.
3. A non-discoverable node is listed and selectable exactly as today
   (the regression gate on the 5 free nodes and on the two `discoverable:
   false` subject nodes).
4. An already-researched node stays listed even if its def is discoverable
   and its id is not in the set.
5. `deliver_payout` credits `ActiveResearch::study` when no project is
   active and `ActiveResearch::progress` when one is — and **never both**,
   asserted on the same payout.
6. A project sitting at its cost credits the study nothing.
7. `settle_study` holds progress with an empty pen, holds it with an empty
   pool, and spends it when both clear — three assertions on
   `ActiveResearch::study`, since a spend-on-hold is the bug that makes a
   discovery feel arbitrary.
8. A seeded run discovers only from the eligible pool: prereq-unsatisfied
   and zone-gated nodes are never picked, over enough forced attempts to
   make the absence mean something.
9. `settle_study` draws **no** `GameRng` on a tick where no attempt is made
   — the arena's own rule, so study cannot shift the seeded stream for a
   base that is not studying.
10. A discovery writes exactly one id, logs one base line, and queues one
    notification.
11. Save → load round-trips `DiscoveredResearch` and `ActiveResearch::study`, and
    a save written without the fields loads with both empty. A RON
    round-trip alone is not enough here — this repo has recorded that a
    `#[serde(skip)]` leaves that test green.

Assets, in `tests/assets.rs`:

12. The census: exactly the 20 named nodes are `discoverable`, and
    `routine_fabrication`/`program_refactoring` are not. A
    `#[serde(default)]` field that no shipped file authors is invisible
    rather than wrong, which this repo has already been bitten by.
13. Every discoverable node is reachable: its `requires` closure bottoms
    out in non-discoverable nodes or in nodes that are themselves
    discoverable and reachable. A discoverable node whose only prerequisite
    can never be discovered is a dead branch that ships silent.

## 9. Out of scope

- No `Game::attention` row for a Station studying, idle, or with a full bar
  and nothing to find. Worth a follow-up once the rate is played; adding it
  blind is a row nobody has learned to want.
- No discovery source other than study. The door is built so one is a
  caller, and that is the whole of the provision made for it.
- No steering by the subject's species. Asked and answered: uniform.
- No player-facing study progress readout. `ActiveResearch::study` is a number the
  engine holds; whether the Station's own examine line should quote it is a
  question for after the rate is felt.
