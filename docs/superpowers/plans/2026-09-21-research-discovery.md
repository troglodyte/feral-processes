# Research Discovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A research node marked `discoverable` is hidden from the base tree until an idle Research Station with a subject pinned in its pen studies its way onto it.

**Architecture:** Two halves and one door between them. The **bevy half** is one branch in `systems::deliver_payout`: with no project selected, a Research Station's otherwise-wasted payout banks into `ActiveResearch::study` instead of landing nowhere. The **`Game` half** is `Game::settle_study`, called from `game/turn.rs` right after `settle_research`, which spends a full bar as one roll over the eligible undiscovered pool and writes a hit through `Game::discover_research`. Visibility is one predicate, `Game::base_node_visible`, read by `listed_research` (so the menu and the flow chart cannot disagree) and by `select_research` (so a typed id cannot buy what was never shown).

**Tech Stack:** Rust, `bevy_ecs` 0.19, RON assets, `serde`.

**Spec:** `docs/superpowers/specs/2026-09-21-research-discovery-design.md` — read it before Task 1. Its §1 "Data", §3 "The roll" and §8 "Tests" are the contract this plan implements.

## Global Constraints

- **`#[serde(default)]` on every new asset and save field.** `ResearchDef::discoverable` and both `SaveData` fields. Default `false`/empty, so nothing hides itself by surprise and no mod file is touched.
- **No `save::SAVE_FORMAT_VERSION` bump.** The save is field-named RON and both additions are additive. An older save loads with nothing discovered; that is the new rule applied, not a migration.
- **Exactly one new `Resource`,** `resources::DiscoveredResearch`. Study progress is a *field on `ActiveResearch`*, never a second resource — see spec §1. A new resource shifts bevy's query iteration order, which this repo has recorded as a cause of seeded tests moving; Task 3 budgets a pass for exactly one such shift.
- **`deliver_payout`'s signature does not change**, and neither `SystemParam` bundle (`CronjobLookups`, `PlayerGatherLookups`) gains a parameter. Both already carry `ResMut<ActiveResearch>`.
- **`Game::discover_research` is the one door** a discovery is written through — set insert, log line, notification. Nothing else writes `DiscoveredResearch`.
- **Both new log lines are `log_base`**, never `log()`: a plain `log()` is `MessageKind::Info` and is pruned by `retain_outcomes_since_battle`.
- **Copy, verbatim:** discovery line `"The study uncovers {name}."`; miss line `"The study turns up nothing."`; the `select_research` refusal `"Unknown research."` (the same sentence the routine tree gives — telling the player a node exists is what a hidden node must not do).
- **Vocabulary:** research is *studied* and a node is *uncovered*; no "spell", "cast", "discovery chance" in player-facing copy.
- **The 20 discoverable ids**, used verbatim by the census in Task 1: `ablative`, `armor_bench`, `cache_coherence`, `capacitance`, `charge_density`, `cold_archive`, `cortex`, `deep_analysis`, `dispatch`, `firewall`, `memory_mapping`, `model_inspection`, `monofilament`, `neural_amp`, `overclock`, `paging`, `segmentation`, `shard_mapping`, `virtual_memory`, `weapon_bench`. `routine_fabrication` and `program_refactoring` carry `requires_subject` and stay **visible**.
- **Gates to run before any commit:** `cargo fmt`, then `cargo clippy --workspace --all-targets` (`--all-targets` is load-bearing), then the task's own tests. `cargo test --workspace` is the gate at Task 3 and Task 8 only.
- **Do not bump the workspace version or write a `CHANGELOG.md` section on this branch.** That happens once, at the merge.
- **One deliberate deviation from the spec,** stated here so it is not read as drift: `ActiveResearch::credit_study` returns `()` rather than the `u32` `credit` returns. Nothing consumes the figure — `deliver_payout`'s study branch returns `0` so `cycle_line` stays silent — and an unused return is a reader's question with no answer. The saturating arithmetic is `credit`'s, unchanged.

---

## File Structure

| File | Task | Responsibility |
|---|---|---|
| `crates/engine/src/research.rs` | 1 | `ResearchDef::discoverable` |
| `assets/research/*.ron` (20 files) | 1 | the content decision |
| `assets/research/README.md` | 1 | the schema doc |
| `crates/engine/src/tests/assets.rs` | 1 | the two censuses |
| `crates/engine/src/notifications.rs` | 2 | `NotificationKind::ResearchDiscovered` |
| `crates/engine/src/tests/notifications.rs` | 2 | the three enum censuses |
| `crates/engine/src/resources.rs` | 3 | `DiscoveredResearch`, `ActiveResearch::study` |
| `crates/engine/src/save.rs` | 3 | `discovered_research`, `study_progress` |
| `crates/engine/src/game/lifecycle.rs` | 3 | registration, load, save |
| `crates/engine/src/game/unlocks.rs` | 4, 5, 7 | `discover_research`, `base_node_visible`, `listed_research`, `select_research`, `eligible_discoveries`, `settle_study` |
| `crates/engine/src/game/catalog.rs` | 5 | `Game::research_defs` |
| `crates/engine/src/systems.rs` | 6 | `deliver_payout`'s study branch |
| `crates/engine/src/tuning.rs` | 7 | `STUDY_ATTEMPT_DATA`, `STUDY_DISCOVERY_CHANCE` |
| `crates/engine/src/game/turn.rs` | 7 | the `settle_study` call |
| `crates/engine/src/tests/research.rs` | 4–7 | the feature's tests |
| `crates/gui/src/render/research_graph.rs`, `render/notify.rs` | 5 | keeping three censuses non-vacuous |

**Why `settle_study` and `eligible_discoveries` live in `game/unlocks.rs` and not in `game/base/study.rs`.** They read `Game::prereq_satisfied` and `Game::research_zone_gate`, both private to `unlocks.rs`. Putting the roll next to the pen would mean widening two private gates to `pub(crate)` to buy a file boundary; `pinned_subject` is already `pub`, so the dependency runs cheaply the other way. `unlocks.rs` is research's own module and already owns `settle_research`, which `settle_study` is called beside.

---

### Task 1: The `discoverable` field and the content that carries it

**Files:**
- Modify: `crates/engine/src/research.rs` (the `ResearchDef` struct, after `requires_subject`)
- Modify: `assets/research/{ablative,armor_bench,cache_coherence,capacitance,charge_density,cold_archive,cortex,deep_analysis,dispatch,firewall,memory_mapping,model_inspection,monofilament,neural_amp,overclock,paging,segmentation,shard_mapping,virtual_memory,weapon_bench}.ron`
- Modify: `assets/research/README.md`
- Test: `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `research::ResearchDef::discoverable: bool` (public field, `#[serde(default)]`, default `false`). Every task after this one reads it.

- [ ] **Step 1: Write the two failing censuses**

Append to `crates/engine/src/tests/assets.rs`:

```rust
/// **The census.** `discoverable` is `#[serde(default)]`, so a shipped tree
/// that authors it nowhere is *invisible* rather than wrong — every node
/// lists, every test stays green, and the feature has silently not shipped.
/// This repo has already been bitten by exactly that.
///
/// `routine_fabrication` and `program_refactoring` are named on the other
/// side deliberately: they carry `requires_subject` like the twenty, and
/// leaving them visible is what keeps the routine tree and companion fusion
/// arriving when they do today rather than behind a dice roll.
#[test]
fn exactly_the_twenty_named_research_nodes_are_discoverable() {
    let game = Game::new(4118, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut found: Vec<&str> = game
        .world
        .resource::<ResearchDb>()
        .all()
        .filter(|d| d.discoverable)
        .map(|d| d.id.as_str())
        .collect();
    found.sort();
    assert_eq!(
        found,
        vec![
            "ablative",
            "armor_bench",
            "cache_coherence",
            "capacitance",
            "charge_density",
            "cold_archive",
            "cortex",
            "deep_analysis",
            "dispatch",
            "firewall",
            "memory_mapping",
            "model_inspection",
            "monofilament",
            "neural_amp",
            "overclock",
            "paging",
            "segmentation",
            "shard_mapping",
            "virtual_memory",
            "weapon_bench",
        ],
        "the discoverable set is a content decision and nothing in ResearchDef states it"
    );
    for id in ["routine_fabrication", "program_refactoring"] {
        let def = game
            .world
            .resource::<ResearchDb>()
            .get(id)
            .expect("both ship");
        assert!(
            def.requires_subject && !def.discoverable,
            "{id} costs a subject and must still be visible from turn one"
        );
    }
}

/// **A discoverable node must be reachable.** A node whose `requires`
/// closure bottoms out in something that can itself never be discovered is
/// a dead branch, and it ships silent: the menu simply never grows it.
///
/// Walked to a fixed point rather than recursively, so a cycle in the tree
/// terminates here instead of blowing the stack — `ResearchDb::load_dir`
/// does not reject one.
#[test]
fn every_discoverable_research_node_is_reachable_from_the_visible_tree() {
    let game = Game::new(4119, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<ResearchDb>();
    // Seed: everything visible from turn one is reachable by definition.
    let mut reachable: std::collections::HashSet<&str> = db
        .all()
        .filter(|d| !d.discoverable)
        .map(|d| d.id.as_str())
        .collect();
    loop {
        let grown: Vec<&str> = db
            .all()
            .filter(|d| !reachable.contains(d.id.as_str()))
            .filter(|d| d.requires.iter().all(|r| reachable.contains(r.as_str())))
            .map(|d| d.id.as_str())
            .collect();
        if grown.is_empty() {
            break;
        }
        reachable.extend(grown);
    }
    let stranded: Vec<&str> = db
        .all()
        .filter(|d| d.discoverable && !reachable.contains(d.id.as_str()))
        .map(|d| d.id.as_str())
        .collect();
    assert!(
        stranded.is_empty(),
        "these nodes can never enter the eligible pool, so they can never be found: {stranded:?}"
    );
    assert!(
        db.all().any(|d| d.discoverable),
        "the fixture is vacuous against a tree with nothing discoverable"
    );
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine discoverable`
Expected: FAIL to compile — `no field 'discoverable' on type '&ResearchDef'`.

- [ ] **Step 3: Add the field**

In `crates/engine/src/research.rs`, inside `pub struct ResearchDef`, immediately after the `requires_subject` field:

```rust
    /// Hidden from the base tree until the base has discovered it — see
    /// `resources::DiscoveredResearch` and `Game::settle_study`. A Research
    /// Station with no project selected banks its payout into study
    /// attempts, and an attempt with a subject pinned rolls over the
    /// eligible undiscovered pool.
    ///
    /// **Its own field, not `requires_subject` doing double duty.** *Does
    /// buying this cost a program?* and *is this hidden until found?* are
    /// different questions; one flag answering both makes a node that costs
    /// a subject but should be visible from turn one — `routine_fabrication`
    /// is exactly that — unrepresentable.
    ///
    /// `#[serde(default)]` is load-bearing in the usual direction and in one
    /// more: the default is `false`, so a mod's tree cannot hide itself by
    /// the field arriving.
    #[serde(default)]
    pub discoverable: bool,
```

- [ ] **Step 4: Author the twenty files**

```bash
cd /home/trog/code/feral-processes
for f in ablative armor_bench cache_coherence capacitance charge_density \
         cold_archive cortex deep_analysis dispatch firewall memory_mapping \
         model_inspection monofilament neural_amp overclock paging \
         segmentation shard_mapping virtual_memory weapon_bench; do
  sed -i 's/^\( *\)requires_subject: true,$/\1requires_subject: true,\n\1discoverable: true,/' \
    "assets/research/$f.ron"
done
grep -c 'discoverable: true' assets/research/*.ron | grep -v ':0' | wc -l   # must print 20
```

- [ ] **Step 5: Document the field**

In `assets/research/README.md`, in the annotated example block, immediately after the `requires_subject: true,` entry and its comment, add:

```
    // Optional; defaults to false. Hides the node from the base research
    // tree until the base has *found* it. A Research Station with a program
    // posted on it and no project selected studies instead of extracting:
    // each full bar is one attempt, and an attempt with a tamed program
    // pinned in the pen rolls over every node that is discoverable, still
    // undiscovered, has its prerequisites researched and is inside the zone
    // you have reached. A hit announces itself and the node joins the menu,
    // where it is then researched exactly as any other — `requires_subject`
    // included.
    //
    // Independent of `requires_subject` on purpose. That one says buying
    // this costs a program; this one says the node is hidden until found. A
    // node may be either, both or neither. The twenty shipped nodes that set
    // this are every `requires_subject` node except `routine_fabrication`
    // and `program_refactoring`, which stay visible so the routine tree and
    // fusion keep arriving when they do today.
    //
    // A node whose `requires` closure can never be satisfied from the
    // visible tree can never be found — the census
    // `every_discoverable_research_node_is_reachable_from_the_visible_tree`
    // fails the build on one.
    discoverable: true,
```

Then add to that file's **Rules** list:

```
- **A discoverable node must be reachable from the visible tree.** Its
  prerequisites must bottom out in nodes that are visible from turn one or
  are themselves discoverable and reachable. A node behind a prerequisite
  nothing can ever discover is a dead branch that ships silent.
```

- [ ] **Step 6: Run the censuses and the asset suite**

Run: `cargo fmt && cargo clippy --workspace --all-targets && cargo test -p feral-processes-engine assets`
Expected: PASS, including the two new tests and the pre-existing research censuses.

- [ ] **Step 7: Commit**

```bash
git add crates/engine/src/research.rs crates/engine/src/tests/assets.rs assets/research/
git commit -m "Research nodes can be marked discoverable"
```

---

### Task 2: `NotificationKind::ResearchDiscovered`

**Files:**
- Modify: `crates/engine/src/notifications.rs` (the enum, `all()`, `def()`, `latch_key()`)
- Test: `crates/engine/src/tests/notifications.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `notifications::NotificationKind::ResearchDiscovered`, fired in Task 4 through `Game::notify_filled(kind, &[("name", ..), ("description", ..)], None)`.

- [ ] **Step 1: Add the variant, its copy and its key**

In `crates/engine/src/notifications.rs`:

1. After the `ResearchComplete` variant in `enum NotificationKind`:

```rust
    /// A study attempt has uncovered a research node — `Game::settle_study`,
    /// through `Game::discover_research`.
    ///
    /// **`Always`**: a run has up to twenty discoveries and each is news.
    /// Templated off the node's own `name` and `description`, so there is no
    /// second copy of that prose to drift — the achievement seam's argument.
    /// No `detail`: what the node *unlocks* is what finishing it buys, and
    /// quoting it here would read as the node already being researched.
    ResearchDiscovered,
```

2. In `pub fn all() -> [NotificationKind; 17]`, change the length to `18` and append `NotificationKind::ResearchDiscovered,` after `NotificationKind::ResearchComplete,`.

3. In `def()`, after the `ResearchComplete` arm:

```rust
            NotificationKind::ResearchDiscovered => NotificationDef {
                title: "Research Discovered",
                body: "{name}\n\n{description}",
                sprite: None,
                // `ResearchComplete`'s glyph and hue: the same machine did
                // the work, and the two halves of one node's story should
                // not look like two different systems.
                glyph: 'R',
                color: GlyphColor::Cyan,
                repeat: Repeat::Always,
            },
```

4. In `latch_key()`, after the `ResearchComplete` arm:

```rust
            // `Always`, so nothing is ever latched under this — but the
            // match is exhaustive and the string is a file format from here
            // on all the same.
            NotificationKind::ResearchDiscovered => "milestone_research_discovered",
```

- [ ] **Step 2: Run the notification censuses to see them fail**

Run: `cargo test -p feral-processes-engine notifications`
Expected: FAIL to compile in `crates/engine/src/tests/notifications.rs` — the `site` match and the `tutorials_latch_and_milestones_do_not` match are both non-exhaustive.

- [ ] **Step 3: Extend the three censuses**

In `crates/engine/src/tests/notifications.rs`:

- In the `site` match, after the `ResearchComplete` arm:

```rust
            NotificationKind::ResearchDiscovered => "Game::discover_research",
```

- In `tutorials_latch_and_milestones_do_not`, add `| NotificationKind::ResearchDiscovered` to the `Repeat::Always` group, beside `NotificationKind::ResearchComplete`.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo fmt && cargo clippy --workspace --all-targets && cargo test -p feral-processes-engine notifications && cargo test -p feral-processes-gui notif`
Expected: PASS. The gui height census walks `all()` and measures `{description}` unfilled for this kind, which is shorter than `ResearchComplete`'s filled census — nothing there needs changing.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/notifications.rs crates/engine/src/tests/notifications.rs
git commit -m "A discovery has a notification to arrive in"
```

---

### Task 3: `DiscoveredResearch`, `ActiveResearch::study`, and the save

**Files:**
- Modify: `crates/engine/src/resources.rs:164-183` (the `ActiveResearch` struct and impl; the new resource goes beside `DiscoveredRoutines` at ~`:227`)
- Modify: `crates/engine/src/save.rs` (`SaveData` fields after `research_progress:~1301`; the `Default` block at ~`:1867`)
- Modify: `crates/engine/src/game/lifecycle.rs:~498` (registration), `:1177` (load), `:2519` (save)
- Test: `crates/engine/src/tests/research.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `resources::DiscoveredResearch(pub BTreeSet<research::ResearchId>)` — a `Resource`.
  - `resources::ActiveResearch::study: u32` (public field) and `ActiveResearch::credit_study(&mut self, amount: u32, cap: u32)` (`pub(crate)`, returns `()`).
  - `save::SaveData::discovered_research: Vec<ResearchId>` and `save::SaveData::study_progress: u32`.

- [ ] **Step 1: Write the failing save tests**

Append to `crates/engine/src/tests/research.rs`:

```rust
/// A RON round-trip alone is not enough: `#[serde(skip)]` on a new field
/// leaves that test green while the field never reaches disk. This goes
/// through the real `Game::save`/`Game::load` pair.
#[test]
fn a_save_round_trips_what_the_base_has_discovered_and_banked() {
    let mut game = Game::new(4420, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world
        .resource_mut::<crate::resources::DiscoveredResearch>()
        .0
        .insert("paging".to_string());
    game.world.resource_mut::<ActiveResearch>().study = 3;

    let path = std::env::temp_dir().join(format!(
        "feral_research_discovery_save_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(
        loaded
            .world
            .resource::<crate::resources::DiscoveredResearch>()
            .0
            .contains("paging"),
        "a discovery is permanent and must survive a reload"
    );
    assert_eq!(
        loaded.world.resource::<ActiveResearch>().study,
        3,
        "a part-banked study attempt must survive a reload"
    );
}
```

The `#[serde(default)]` half belongs in `crates/engine/src/save.rs`'s own
test module, beside `a_save_written_before_tools_existed_loads_with_the_
starter_tool`, whose shape it copies exactly — that module has the
`sample_data` / `save_to_file` / `load_from_file` helpers and the
line-anchored key-removal idiom, and the engine's saves are RON text on
disk, so an older save really is this file with two lines cut:

```rust
    /// `discovered_research` and `study_progress` are additive behind
    /// `#[serde(default)]`, so a save written before discovery shipped must
    /// load with nothing found and nothing banked. For a run in progress
    /// that means the discoverable half of its tree hides itself and is
    /// studied back out, which is the new rule correctly applied rather
    /// than a migration.
    #[test]
    fn a_save_written_before_discovery_loads_with_nothing_discovered() {
        let path = std::env::temp_dir().join(format!(
            "feral_processes_save_no_discovery_{}.bin",
            std::process::id()
        ));
        let mut data = sample_data();
        data.discovered_research = vec!["paging".to_string()];
        data.study_progress = 5;
        save_to_file(&path, &data).unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        let older: String = text
            .lines()
            .filter(|line| {
                let t = line.trim_start();
                !t.starts_with("discovered_research:") && !t.starts_with("study_progress:")
            })
            .collect::<Vec<_>>()
            .join("\n");
        // Line-anchored, for the reason the tools migration beside this one
        // takes the same care: a bare `.contains` would be satisfied by a
        // substring of some other field and pass for the wrong reason.
        assert!(
            older.lines().count() < text.lines().count(),
            "both keys have to actually be gone for this to prove anything"
        );
        std::fs::write(&path, &older).unwrap();

        let loaded = match load_from_file(&path) {
            Ok(loaded) => loaded,
            Err(e) => panic!("a file written before discovery existed must still load: {e}"),
        };
        let _ = std::fs::remove_file(&path);
        assert!(loaded.discovered_research.is_empty());
        assert_eq!(loaded.study_progress, 0);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine a_save_round_trips_what_the_base a_save_written_before_discovery`
Expected: FAIL to compile — `DiscoveredResearch` is not a resource and `ActiveResearch` has no field `study`.

- [ ] **Step 3: Add the resource and the field**

In `crates/engine/src/resources.rs`, add the field to `ActiveResearch`:

```rust
    /// Research currency banked toward the next study attempt — where a
    /// Research Station's payout lands while `id` is `None`.
    ///
    /// **A field here rather than a resource of its own.** `ActiveResearch`
    /// is already the answer to *where research currency lands*, so this is
    /// a second place it can land rather than a second type to ask:
    /// `systems::deliver_payout`'s signature does not change, neither of its
    /// two `SystemParam` bundles gains a parameter, and the world gains one
    /// resource instead of two — one shift in bevy's iteration order rather
    /// than a self-inflicted second.
    ///
    /// Capped at a single attempt (`tuning::STUDY_ATTEMPT_DATA`), so a
    /// Station studying with an empty pen banks one attempt and holds rather
    /// than stockpiling.
    pub study: u32,
```

and the method, below `credit`:

```rust
    /// Banks `amount` toward the next study attempt, saturating at `cap`.
    ///
    /// `credit`'s arithmetic with a different destination. It reports
    /// nothing because nothing consumes the figure: `deliver_payout`'s study
    /// branch returns `0` so `systems::cycle_line` stays silent, and the
    /// study speaks for itself through `Game::settle_study`.
    pub(crate) fn credit_study(&mut self, amount: u32, cap: u32) {
        self.study += amount.min(cap.saturating_sub(self.study));
    }
```

Then, beside `DiscoveredRoutines`:

```rust
/// Which research nodes the base has discovered — see
/// `research::ResearchDef::discoverable` and `Game::settle_study`. A node in
/// here is listed in the base tree and buyable; one that is `discoverable`
/// and not in here is invisible.
///
/// **The extension point.** This is a set of ids and is innocent of study:
/// a second discovery source — a Stack find, a contract reward, a schematic
/// on a settlement's shelf — is a new caller of `Game::discover_research`,
/// not a new field, a new gate or a save change. `DiscoveredRoutines` above
/// is shaped this way already.
///
/// A `BTreeSet` for `KnownRoutines`' own reason: the save writes this set
/// out, and a `HashSet`'s iteration order would make the encoded bytes
/// differ run to run.
#[derive(Resource, Default)]
pub struct DiscoveredResearch(pub BTreeSet<crate::research::ResearchId>);
```

- [ ] **Step 4: Register, save and load it**

- `crates/engine/src/game/lifecycle.rs`, beside `world.init_resource::<crate::resources::DiscoveredRoutines>();` in `Game::new`:

```rust
        world.init_resource::<crate::resources::DiscoveredResearch>();
```

- In the load path, extend the existing `ActiveResearch` literal and add the set:

```rust
        world.insert_resource(crate::resources::ActiveResearch {
            id: data.active_research,
            progress: data.research_progress.into_iter().collect(),
            study: data.study_progress,
        });
        world.insert_resource(crate::resources::DiscoveredResearch(
            data.discovered_research.into_iter().collect(),
        ));
```

- In the save path, after the `research_progress:` block:

```rust
            discovered_research: self
                .world
                .resource::<crate::resources::DiscoveredResearch>()
                .0
                .iter()
                .cloned()
                .collect(),
            study_progress: self.world.resource::<crate::resources::ActiveResearch>().study,
```

- `crates/engine/src/save.rs`, after the `research_progress` field:

```rust
    /// Which research nodes the base has discovered — see
    /// `resources::DiscoveredResearch`. Already sorted: the resource is a
    /// `BTreeSet`, so the encoded bytes do not depend on iteration order.
    /// `#[serde(default)]` so a save written before discovery existed loads
    /// with nothing found, which is the new rule correctly applied rather
    /// than a migration — the discoverable half of that run's tree hides
    /// itself and is studied back out.
    #[serde(default)]
    pub discovered_research: Vec<crate::research::ResearchId>,
    /// Research currency banked toward the next study attempt — see
    /// `resources::ActiveResearch::study`. `#[serde(default)]` for
    /// `discovered_research`'s reason.
    #[serde(default)]
    pub study_progress: u32,
```

and in the `Default` impl, beside `research_progress: Vec::new(),`:

```rust
            discovered_research: Vec::new(),
            study_progress: 0,
```

- [ ] **Step 5: Run the new tests**

Run: `cargo fmt && cargo clippy --workspace --all-targets && cargo test -p feral-processes-engine research && cargo test -p feral-processes-engine save::`
Expected: PASS. The second command is separate because the absent-field test lives in `save.rs`'s own module and `-p … research` does not reach it.

- [ ] **Step 6: The iteration-order pass — run the whole suite**

Run: `cargo test --workspace`
Expected: PASS, **or** a small number of seeded-outcome failures. A new `Resource` shifts bevy's query iteration order, which this repo has recorded as a cause of seeded tests moving; this is the one shift the feature is allowed.

If any fail: confirm the failure is a moved *outcome* and not a moved *rule* — read the assertion, and check whether the same test passes under a different seed. Fix by re-seeding the fixture (change the `Game::new` seed), never by loosening the assertion. Record in the commit message which tests were re-seeded and why.

- [ ] **Step 7: Commit**

```bash
git add crates/engine/src/resources.rs crates/engine/src/save.rs \
        crates/engine/src/game/lifecycle.rs crates/engine/src/tests/research.rs
git commit -m "The base remembers what it has discovered and what it has banked"
```

---

### Task 4: `Game::discover_research`, the one door

**Files:**
- Modify: `crates/engine/src/game/unlocks.rs` (a new method beside `settle_research`)
- Test: `crates/engine/src/tests/research.rs`

**Interfaces:**
- Consumes: `resources::DiscoveredResearch` (Task 3), `NotificationKind::ResearchDiscovered` (Task 2).
- Produces: `pub fn discover_research(&mut self, id: &str) -> bool` on `Game` — `true` when the id resolved and was not already discovered. `pub`, not `pub(crate)`: Task 5's gui censuses call it, and a future second discovery source may live outside the engine.

- [ ] **Step 1: Write the failing test**

Append to `crates/engine/src/tests/research.rs`:

```rust
/// The one door: one id written, one base line, one notification — and a
/// second call on the same node is inert, which is what lets any caller fire
/// it without first asking whether the node is already known.
#[test]
fn discovering_a_node_writes_one_id_one_line_and_one_notification() {
    let mut game = Game::new(4430, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let name = game
        .world
        .resource::<ResearchDb>()
        .get("paging")
        .expect("paging ships")
        .name
        .clone();
    while game.take_notification().is_some() {}

    assert!(game.discover_research("paging"), "a fresh node is news");

    assert_eq!(
        game.world
            .resource::<crate::resources::DiscoveredResearch>()
            .0
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["paging".to_string()],
        "exactly one id, and only the one asked for"
    );
    let said: Vec<String> = game
        .message_history(500)
        .iter()
        .filter(|l| l.text.contains("uncovers"))
        .map(|l| l.text.clone())
        .collect();
    assert_eq!(said, vec![format!("The study uncovers {name}.")]);
    assert_eq!(game.notifications_pending(), 1);

    // Inert the second time: no second id, no second line, no second popup.
    while game.take_notification().is_some() {}
    assert!(!game.discover_research("paging"), "already found");
    assert_eq!(game.notifications_pending(), 0);
    assert_eq!(
        game.message_history(500)
            .iter()
            .filter(|l| l.text.contains("uncovers"))
            .count(),
        1
    );

    // An id nothing defines is refused rather than written — a save editor's
    // typo must not mint a node the tree has never heard of.
    assert!(!game.discover_research("not_a_node"));
    assert_eq!(
        game.world
            .resource::<crate::resources::DiscoveredResearch>()
            .0
            .len(),
        1
    );
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p feral-processes-engine discovering_a_node_writes_one_id`
Expected: FAIL to compile — `no method named 'discover_research'`.

- [ ] **Step 3: Write the door**

In `crates/engine/src/game/unlocks.rs`, immediately above `settle_research`:

```rust
    /// **The one door a discovery is written through** — `Game::remember`'s
    /// rule. `Game::settle_study` is its only caller today; a second
    /// discovery source is a second caller of this and nothing else.
    ///
    /// Reports whether this was news. Idempotent by construction: a node
    /// already in the set writes nothing, says nothing and pops nothing, so
    /// no caller needs a check of its own — and an id nothing defines is
    /// refused before the set is touched, since a name the tree never heard
    /// of would sit there forever gating nothing.
    ///
    /// No `detail`: `Game::research_unlocks` is what *finishing* the node
    /// buys, and quoting it on the discovery would read as the node already
    /// being researched.
    pub fn discover_research(&mut self, id: &str) -> bool {
        let Some(def) = self.world.resource::<ResearchDb>().get(id).cloned() else {
            return false;
        };
        if !self
            .world
            .resource_mut::<crate::resources::DiscoveredResearch>()
            .0
            .insert(def.id.clone())
        {
            return false;
        }
        // `log_base`, matching selection, abandonment and completion: a
        // discovery is base news and can land while the party is four frames
        // down the Stack. A plain `log()` is `MessageKind::Info`, which
        // `retain_outcomes_since_battle` prunes.
        self.log_base(format!("The study uncovers {}.", def.name));
        self.notify_filled(
            crate::notifications::NotificationKind::ResearchDiscovered,
            &[("name", &def.name), ("description", &def.description)],
            None,
        );
        true
    }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo fmt && cargo clippy --workspace --all-targets && cargo test -p feral-processes-engine research`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/game/unlocks.rs crates/engine/src/tests/research.rs
git commit -m "One door writes a discovery"
```

---

### Task 5: Visibility — `base_node_visible`, the menu, and the refusal

**Files:**
- Modify: `crates/engine/src/game/unlocks.rs` (`listed_research:428`, `select_research:888`, the new predicate)
- Modify: `crates/engine/src/game/catalog.rs` (beside `structure_defs:25`)
- Modify: `crates/gui/src/render/research_graph.rs` (the three tests at ~`:752`, `:858`, `:947`, `:984`), `crates/gui/src/render/notify.rs` (~`:410`)
- Modify: fixtures across `crates/engine/src/tests/` that select a discoverable node
- Test: `crates/engine/src/tests/research.rs`

**Interfaces:**
- Consumes: `discoverable` (Task 1), `DiscoveredResearch` (Task 3), `Game::discover_research` (Task 4).
- Produces:
  - `pub(crate) fn base_node_visible(&self, def: &ResearchDef) -> bool` on `Game`.
  - `pub fn research_defs(&self) -> Vec<ResearchDef>` on `Game` — `structure_defs`' shape, so a renderer (and its censuses) can walk the catalogue without reaching into `world`.

- [ ] **Step 1: Write the four failing tests**

Append to `crates/engine/src/tests/research.rs`:

```rust
/// A discoverable node is off **both** surfaces until it is found, and on
/// both after. One predicate feeds `listed_research`, and `research_nodes`
/// and `research_graph` are both built from it, so the menu and the flow
/// chart cannot disagree about what exists.
#[test]
fn a_discoverable_node_is_absent_from_both_research_surfaces_until_discovered() {
    let mut game = Game::new(4440, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let listed = |g: &Game| {
        g.research_nodes(ResearchTree::Base)
            .iter()
            .any(|n| n.id == "paging")
    };
    let drawn = |g: &Game| g.research_graph(ResearchTree::Base).cell("paging").is_some();

    assert!(!listed(&game), "an undiscovered node is not a menu row");
    assert!(!drawn(&game), "and not a box on the chart either");

    game.discover_research("paging");

    assert!(listed(&game));
    assert!(drawn(&game));
}

/// Leaving a node off the menu is not enough: an id typed into a save editor
/// or a stale UI row must be refused too — and refused **before anything is
/// written**, `commit_caravan_basket`'s rule.
#[test]
fn select_research_refuses_an_undiscovered_node_before_anything_is_written() {
    let mut game = Game::new(4441, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    let station = game.study_station().expect("the fixture stood one");
    pin_subject_at_pen(&mut game, program, station);

    let err = game.select_research("paging").unwrap_err();

    assert_eq!(
        err, "Unknown research.",
        "a hidden node must not be told apart from one that does not exist"
    );
    assert_eq!(active_research(&game), None, "and no project is started");
    assert!(game.work_orders().is_empty(), "and no bill is filed");

    // The reachability half: discovering it makes the identical call succeed,
    // so the refusal cannot ship permanent.
    game.discover_research("paging");
    game.select_research("paging")
        .expect("a discovered node is an ordinary node");
}

/// The regression gate on the visible spine. A node that is not
/// `discoverable` is listed and selectable exactly as it was — including the
/// two that cost a subject and stay visible on purpose.
#[test]
fn a_node_that_is_not_discoverable_is_untouched_by_the_hiding_rule() {
    let mut game = Game::new(4442, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    let ids: Vec<String> = game
        .research_nodes(ResearchTree::Base)
        .iter()
        .map(|n| n.id.clone())
        .collect();
    for id in [
        "automation",
        "power_grid",
        "commerce",
        "teardown",
        "fortification",
        "routine_fabrication",
        "program_refactoring",
    ] {
        assert!(ids.contains(&id.to_string()), "{id} must stay visible");
    }

    game.select_research("automation")
        .expect("the visible spine is bought exactly as before");
}

/// "Researched means known" reaches across this gate too: a node already
/// bought is never hidden by a later change to what gates it — a save from
/// before this feature, or a `.ron` edit that adds `discoverable` to
/// something the player already owns.
#[test]
fn an_already_researched_node_stays_listed_even_when_it_is_discoverable() {
    let mut game = Game::new(4443, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "armor_bench");
    assert!(
        game.world
            .resource::<crate::resources::DiscoveredResearch>()
            .0
            .is_empty(),
        "the fixture is vacuous if the chain discovered it on the way"
    );

    assert!(
        game.research_nodes(ResearchTree::Base)
            .iter()
            .any(|n| n.id == "armor_bench"),
        "a node you already own cannot become invisible"
    );
}
```

`views::ResearchGraph` holds `cells`, not `nodes`, and `ResearchGraph::cell(id)` is the lookup — verified against `crates/engine/src/views.rs:198`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine research::a_discoverable_node_is_absent`
Expected: FAIL — the node is listed, because nothing filters yet.

- [ ] **Step 3: Write the predicate and wire both readers**

In `crates/engine/src/game/unlocks.rs`, above `listed_research`:

```rust
    /// Whether a **base** node is visible at all — `listed_research`'s Base
    /// filter and `select_research`'s refusal, so the menu, the flow chart
    /// and the door cannot disagree about what exists.
    ///
    /// `node_researched` is in there for the routine tree's own
    /// "researched means known" rule: a node already bought is never hidden
    /// by a later change to what gates it — a save from before this feature,
    /// or a `.ron` edit that adds `discoverable` to something the player
    /// already owns.
    pub(crate) fn base_node_visible(&self, def: &ResearchDef) -> bool {
        !def.discoverable
            || self.node_researched(def)
            || self
                .world
                .resource::<crate::resources::DiscoveredResearch>()
                .0
                .contains(&def.id)
    }
```

Change `listed_research`'s Base arm to:

```rust
        if tree == ResearchTree::Base {
            return db
                .all()
                .filter(|d| d.tree == ResearchTree::Base)
                .filter(|d| self.base_node_visible(d))
                .collect();
        }
```

In `select_research`, immediately after the `if def.tree == ResearchTree::Routines { ... }` block and before the already-researched check:

```rust
        // A hidden base node is refused for the routine tree's exact reason,
        // one rung down: leaving it off the menu is not enough, because an id
        // typed into a save editor — or a stale UI row — could otherwise buy
        // something the player was never shown. The sentence is the routine
        // tree's own, because telling the player a node exists is precisely
        // what a hidden node must not do.
        if def.tree == ResearchTree::Base && !self.base_node_visible(&def) {
            return Err("Unknown research.".to_string());
        }
```

**`has_research_tree` is deliberately not touched.** It asks whether the catalogue holds any `Base` node, not whether anything is listed right now, so the base tree's menu row stays reachable even for a mod whose whole tree is discoverable.

- [ ] **Step 4: Add the catalogue accessor for the renderer**

In `crates/engine/src/game/catalog.rs`, beside `structure_defs`:

```rust
    /// Every research node the catalogue holds, **unfiltered by discovery**
    /// — `structure_defs`' shape and its reason: a renderer needs the shipped
    /// content to measure itself against, and `Game::research_nodes` answers
    /// the narrower question of what the player may see right now.
    ///
    /// The gui's layout censuses read this: measured through
    /// `research_nodes` they would shrink to the seven visible nodes the
    /// moment discovery shipped, and pass vacuously on the twenty widest.
    pub fn research_defs(&self) -> Vec<crate::research::ResearchDef> {
        self.world
            .resource::<ResearchDb>()
            .all()
            .cloned()
            .collect()
    }
```

- [ ] **Step 5: Run the whole workspace and sweep the fixtures**

Run: `cargo test --workspace 2>&1 | tail -60`

Expect failures in three shapes. Fix each through the real door, never by loosening an assertion:

1. **A test that calls `select_research("<discoverable id>")`** — `paging` (13 sites), `weapon_bench` (1), `cache_coherence` (1), across `tests/research.rs` and `tests/building.rs`. Add `game.discover_research("<id>");` immediately before the selection. Find them with:
   `rg -n 'select_research\("(ablative|armor_bench|cache_coherence|capacitance|charge_density|cold_archive|cortex|deep_analysis|dispatch|firewall|memory_mapping|model_inspection|monofilament|neural_amp|overclock|paging|segmentation|shard_mapping|virtual_memory|weapon_bench)"' crates/`
   Do **not** put the call inside `base_with_a_research_node`: the four tests written in Step 1 use that fixture and must still see a hidden tree.
   `support::unlock_research_chain` needs no change — it writes `ActiveResearch.id` directly and never passes through `select_research`.
2. **An engine test asserting on `research_nodes`/`research_graph` contents** in `tests/research_graph.rs`, `tests/inspection.rs` and friends. If the test is about a *named* node, discover it first; if it is about the shape of the list, discover the whole catalogue first:
   `for def in game.research_defs() { game.discover_research(&def.id); }`
3. **The three gui censuses.** In `crates/gui/src/render/research_graph.rs` (the label-width census at ~`:752`, `every_tier_is_drawn_when_the_cursor_reaches_it` at ~`:858` — it names "Cache Coherence", "Segmentation" and "Overclock" — and the two panel tests at ~`:947` and ~`:984`) and in `crates/gui/src/render/notify.rs`'s `every_research_alert_fits_its_screen_once_filled` at ~`:410`: insert the same whole-catalogue loop right after the `Game::new` line, and add a line to each doc comment saying why:

```rust
        // Every shipped node, not only the visible ones: this census
        // measures content against a screen with no scroll, and
        // `research_nodes` narrows to what has been discovered.
        for def in game.research_defs() {
            game.discover_research(&def.id);
        }
```

(Those fixtures bind `game` immutably today; change to `let mut game`.)

- [ ] **Step 6: Re-run the whole workspace**

Run: `cargo fmt && cargo clippy --workspace --all-targets && cargo test --workspace`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add -u crates/
git commit -m "A discoverable node is hidden until the base finds it"
```

---

### Task 6: Accrual — an idle Station banks its payout

**Files:**
- Modify: `crates/engine/src/systems.rs:484-500` (`deliver_payout`'s research branch)
- Test: `crates/engine/src/tests/research.rs`

**Interfaces:**
- Consumes: `ActiveResearch::credit_study` (Task 3), `tuning::STUDY_ATTEMPT_DATA` (introduced here with its final value; Task 7 documents and prices it).
- Produces: nothing new. `deliver_payout`'s signature is unchanged.

**Note on ordering:** this task needs `tuning::STUDY_ATTEMPT_DATA` to exist. Add it here as a bare `pub const STUDY_ATTEMPT_DATA: u32 = 4;` with a one-line doc pointing at Task 7, and let Task 7 write the arithmetic behind it. Do not invent a different value.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/research.rs`:

```rust
/// One payout lands in exactly one place, and the branch is `id.is_none()`
/// — never both, asserted on the same payout.
#[test]
fn a_research_payout_feeds_the_study_or_the_project_and_never_both() {
    let mut game = Game::new(4450, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    let worker = spawn_tamed(&mut game, 10, 3);
    park_at_post(&mut game, worker, node);
    game.set_standing_job(node, true, false).unwrap();

    for _ in 0..400 {
        game.tick();
    }

    let banked = game.world.resource::<ActiveResearch>().study;
    assert!(
        banked > 0,
        "an idle Station's cycles must bank toward a study attempt, not land nowhere"
    );
    assert!(
        game.world.resource::<ActiveResearch>().progress.is_empty(),
        "with nothing selected, nothing may reach a project's progress"
    );

    game.select_research("automation").unwrap();
    let before = game.world.resource::<ActiveResearch>().study;
    for _ in 0..400 {
        game.tick();
    }

    assert_eq!(
        game.world.resource::<ActiveResearch>().study,
        before,
        "with a project selected the study banks nothing — one or the other"
    );
    assert!(
        game.world
            .resource::<ActiveResearch>()
            .progress
            .values()
            .sum::<u32>()
            > 0,
        "and the project is the one that moves"
    );
}

/// The gate is `id.is_none()` and **not** `landed == 0`. A project sitting
/// at its cost also lands nothing and will settle next tick; feeding the
/// study from it would make "study or advance" a lie for one tick an
/// attentive player could farm.
#[test]
fn a_project_sitting_at_its_cost_banks_nothing_toward_a_study() {
    let mut game = Game::new(4451, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    let worker = spawn_tamed(&mut game, 10, 3);
    park_at_post(&mut game, worker, node);
    let cost = game
        .world
        .resource::<ResearchDb>()
        .get("cold_archive")
        .expect("a deep, expensive node nothing in this fixture will finish")
        .cost;
    game.discover_research("cold_archive");
    {
        let mut research = game.world.resource_mut::<ActiveResearch>();
        research.id = Some("cold_archive".to_string());
        research.progress.insert("cold_archive".to_string(), cost);
    }

    for _ in 0..400 {
        game.tick();
    }

    assert_eq!(
        game.world.resource::<ActiveResearch>().study,
        0,
        "a full project lands nothing and must bank nothing"
    );
}
```

If `cold_archive`'s prerequisites or zone gate let `settle_research` complete it inside 400 ticks, the fixture is wrong — pick a node whose `requires` are unmet so `settle_research`'s own gate holds it, and assert that it is still the active project at the end.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine research::a_research_payout_feeds`
Expected: FAIL — `study` stays `0`, because an idle Station's payout still lands nowhere.

- [ ] **Step 3: Add the branch**

In `crates/engine/src/systems.rs`, replace the research branch of `deliver_payout` with:

```rust
    if items.research_currency() == Some(resource) {
        // **`id.is_none()`, not `landed == 0`.** They are different states: a
        // project sitting *at* its cost also lands nothing and will settle
        // next tick, and feeding the study from it would make "study or
        // advance" a lie for one tick an attentive player could farm.
        //
        // Returning `0` is what keeps `cycle_line` silent, exactly as it is
        // today — the study speaks for itself through `Game::settle_study`
        // rather than through a cycle line reading "extracted 0 Research
        // Data".
        if research.id.is_none() {
            research.credit_study(payout, crate::tuning::STUDY_ATTEMPT_DATA);
            return 0;
        }
        let cap = research
            .id
            .as_ref()
            .and_then(|id| research_defs.get(id))
            .map_or(0, |def| def.cost);
        return research.credit(payout, cap);
    }
```

And add to `crates/engine/src/tuning.rs`, for now:

```rust
/// Research Data banked per study attempt — priced in Task 7.
pub const STUDY_ATTEMPT_DATA: u32 = 4;
```

Also extend `deliver_payout`'s doc comment: the existing paragraph ending "With no project selected it lands nowhere and returns 0" is now false. Replace that sentence with one saying the payout banks toward a study attempt instead, still returning 0 so the cycle stays silent.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo fmt && cargo clippy --workspace --all-targets && cargo test -p feral-processes-engine research`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/systems.rs crates/engine/src/tuning.rs crates/engine/src/tests/research.rs
git commit -m "An idle Research Station banks its payout into study"
```

---

### Task 7: The roll — `settle_study`, and what it costs

**Files:**
- Modify: `crates/engine/src/game/unlocks.rs` (two new methods below `discover_research`)
- Modify: `crates/engine/src/game/turn.rs:261` (the call, immediately after `self.settle_research();`)
- Modify: `crates/engine/src/tuning.rs` (a new section; `STUDY_ATTEMPT_DATA` gets its real doc)
- Test: `crates/engine/src/tests/research.rs`

**Interfaces:**
- Consumes: `Game::discover_research` (Task 4), `Game::pinned_subject`, `Game::prereq_satisfied`, `Game::research_zone_gate`, `ActiveResearch::study`.
- Produces: `pub(crate) fn settle_study(&mut self)` and `fn eligible_discoveries(&self) -> Vec<ResearchId>` on `Game`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/research.rs`:

```rust
/// **Three early returns hold the bar; only a real attempt spends it.** A
/// full bar with an empty pen, or with nothing left to find, is banked and
/// fires the instant the condition clears — which is what makes a discovery
/// legible as a consequence of the player's action rather than of a timer
/// they cannot see. A spend-on-hold is the bug that makes it feel arbitrary.
#[test]
fn a_study_attempt_is_held_until_a_subject_and_a_pool_are_both_there() {
    let mut game = Game::new(4460, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    let full = crate::tuning::STUDY_ATTEMPT_DATA;

    // Held: a full bar, an empty pen.
    game.world.resource_mut::<ActiveResearch>().study = full;
    game.settle_study();
    assert_eq!(
        game.world.resource::<ActiveResearch>().study,
        full,
        "nothing is pinned, so the attempt waits rather than being spent"
    );

    // Held: a subject, but nothing eligible to find. Zone 1 with nothing
    // researched is exactly that state — every discoverable node either
    // waits on `automation` or is gated at `min_zone >= 2`.
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    assert!(
        !game.is_researched("automation"),
        "the fixture is vacuous if the pool is already open"
    );
    game.settle_study();
    assert_eq!(
        game.world.resource::<ActiveResearch>().study,
        full,
        "nothing to find, so the attempt waits"
    );

    // Spent: both conditions clear.
    unlock_research_chain(&mut game, "automation");
    game.settle_study();
    assert_eq!(
        game.world.resource::<ActiveResearch>().study,
        0,
        "with a subject and a pool, the attempt is spent whether or not it hits"
    );
}

/// The pool is the whole of what may be found: a prereq-unsatisfied node and
/// a zone-gated one are never picked, over enough forced attempts that the
/// absence means something. A discovery is therefore always immediately
/// researchable.
#[test]
fn a_study_never_discovers_outside_the_eligible_pool() {
    let mut game = Game::new(4461, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    unlock_research_chain(&mut game, "automation");
    set_zone(&mut game, 1);

    for _ in 0..500 {
        game.world.resource_mut::<ActiveResearch>().study = crate::tuning::STUDY_ATTEMPT_DATA;
        game.settle_study();
    }

    let found: Vec<String> = game
        .world
        .resource::<crate::resources::DiscoveredResearch>()
        .0
        .iter()
        .cloned()
        .collect();
    assert!(
        !found.is_empty(),
        "500 forced attempts must find something, or this test proves nothing"
    );
    for id in &found {
        let def = game.world.resource::<ResearchDb>().get(id).unwrap().clone();
        assert_eq!(
            def.min_zone, 0,
            "{id} is gated above zone 1 and must not have been found here"
        );
        for req in &def.requires {
            assert!(
                game.is_researched(req),
                "{id} was found with {req} unresearched"
            );
        }
    }
}

/// `settle_study` draws **no** `GameRng` on a tick where no attempt is made
/// — the arena's own rule, so a base that is not studying cannot shift the
/// seeded stream out from under everything else in the run.
#[test]
fn settle_study_draws_no_rng_when_no_attempt_is_made() {
    let mut game = Game::new(4462, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    assert_eq!(
        game.world.resource::<ActiveResearch>().study,
        0,
        "no bar, so no attempt"
    );

    fn peek(g: &mut Game) -> u64 {
        use rand::RngExt;
        g.world
            .resource_mut::<crate::resources::GameRng>()
            .0
            .random()
    }

    reseed_rng(&mut game, 55);
    let without = peek(&mut game);

    reseed_rng(&mut game, 55);
    game.settle_study();
    let with = peek(&mut game);

    assert_eq!(
        without, with,
        "an idle study must not touch the shared GameRng stream"
    );
}

/// The miss line: an attempt that is spent and finds nothing says so, and
/// says it as base news. `resources::condense` already folds repeats on all
/// three log surfaces, so this needs no rate limit of its own.
#[test]
fn a_failed_study_attempt_says_so() {
    let mut game = Game::new(4463, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    unlock_research_chain(&mut game, "automation");

    for _ in 0..200 {
        game.world.resource_mut::<ActiveResearch>().study = crate::tuning::STUDY_ATTEMPT_DATA;
        game.settle_study();
    }

    assert!(
        game.message_history(500)
            .iter()
            .any(|l| l.text.contains("The study turns up nothing.")),
        "200 attempts against a sub-1.0 chance must miss at least once, and a miss is news"
    );
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine research::a_study_attempt_is_held`
Expected: FAIL to compile — `no method named 'settle_study'`.

- [ ] **Step 3: Write the pool and the roll**

In `crates/engine/src/game/unlocks.rs`, below `discover_research`:

```rust
    /// Every node a study may find right now: a **base** node that is
    /// `discoverable`, not already discovered, not already researched, has
    /// every prerequisite satisfied and is inside the zone the party has
    /// reached.
    ///
    /// **A discovery is therefore always immediately researchable** — the
    /// node arrives on the menu available rather than locked, which is what
    /// makes finding one feel like a reward instead of a promissory note.
    ///
    /// Draws no RNG and is its own function, so a second discovery source in
    /// future filters this pool or narrows it rather than restating the
    /// rule. `ResearchDb::all` is ordered (cheapest first, ties by id), so
    /// the pool a seeded run indexes into is stable.
    fn eligible_discoveries(&self) -> Vec<ResearchId> {
        let db = self.world.resource::<ResearchDb>();
        let discovered = &self
            .world
            .resource::<crate::resources::DiscoveredResearch>()
            .0;
        db.all()
            .filter(|d| d.tree == ResearchTree::Base)
            .filter(|d| d.discoverable)
            .filter(|d| !discovered.contains(&d.id))
            .filter(|d| !self.node_researched(d))
            .filter(|d| d.requires.iter().all(|r| self.prereq_satisfied(r)))
            .filter(|d| self.research_zone_gate(d).is_none())
            .map(|d| d.id.clone())
            .collect()
    }

    /// One study attempt, if the base has banked one and can spend it.
    ///
    /// A `Game` method for `run_repair_bays`' own reason: it draws
    /// `GameRng`, names the node, logs through `log_base` and raises a
    /// notification, none of which a bevy system can reach.
    ///
    /// **The three early returns hold the bar rather than spending it.** A
    /// full bar with an empty pen, or with nothing left to find, is banked
    /// and fires the instant the condition clears. That is what makes a
    /// discovery legible as a consequence of the player's action rather than
    /// of a timer they cannot see — and it is why the accrual in
    /// `systems::deliver_payout` needs no notion of a subject at all.
    pub(crate) fn settle_study(&mut self) {
        if self.world.resource::<ActiveResearch>().study < crate::tuning::STUDY_ATTEMPT_DATA {
            return;
        }
        if self.pinned_subject().is_none() {
            return;
        }
        let pool = self.eligible_discoveries();
        if pool.is_empty() {
            return;
        }
        // Past every hold: the attempt is spent whether or not it lands.
        self.world.resource_mut::<ActiveResearch>().study = 0;
        let hit = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(crate::tuning::STUDY_DISCOVERY_CHANCE)
        };
        if !hit {
            // Repeats, and `resources::condense` already folds repeats on
            // all three log surfaces — no rate limit of its own.
            self.log_base("The study turns up nothing.");
            return;
        }
        // One draw, over an order `ResearchDb::all` documents as stable.
        let id = {
            let mut rng = self.world.resource_mut::<GameRng>();
            pool[rng.0.random_range(0..pool.len())].clone()
        };
        self.discover_research(&id);
    }
```

- [ ] **Step 4: Call it from the tick**

In `crates/engine/src/game/turn.rs`, immediately after `self.settle_research();`:

```rust
        // Immediately after the project settles, for the same reason it sits
        // where it does: a Research Station staffed this tick is already
        // posted. After rather than before, so a tick that *completes* a
        // project leaves `ActiveResearch::id` empty and the Station's next
        // cycle banks toward a study — the two halves of "study or advance"
        // hand over on the same tick rather than one apart.
        self.settle_study();
```

- [ ] **Step 5: Price the two constants**

In `crates/engine/src/tuning.rs`, add a section immediately after `routine_research_cost` (before the `Player emulation` box), and delete the placeholder `STUDY_ATTEMPT_DATA` added in Task 6:

```rust
// ─────────────────────────────────────────────────────────────────────────
// Research: discovery by study
// ─────────────────────────────────────────────────────────────────────────
//
// See `Game::settle_study`. A Research Station with no project selected
// banks its payout into study attempts instead of landing it nowhere; each
// full bar with a subject pinned is one roll over the eligible undiscovered
// pool.
//
// **Priced against the Station, not fitted against `balance_sim`**, which
// models no base production at all. The arithmetic, at the worst rung the
// feature is ever played at — a tier-1 Station in zone 1, staffed by a
// program at the baseline aptitude with no memories:
//
//   cycle length   `research_node.ron`'s `ticks_per_unit: 14`, unscaled by
//                  tier (`systems::work_ticks_at_speed` scales on speed and
//                  build quality, not tier)
//   yield rate     `systems::mining_success_chance(level 1, …)` = 0.5
//   payout         `systems::node_payout(tier 1, zone 1)` = 1
//   ⇒ ~1 Research Data per 28 ticks, and the world runs at
//     `app_core::WORLD_SPEED_MULTIPLIER` = 2 ticks a second: **14s per unit**
//
// So one attempt is ~56 s of an idle Station, and a discovery is ~2.7
// minutes of them — "a handful of minutes at the keyboard, not an hour." By
// mid-run (tier 3, zone 2) the payout is 4 a cycle and the same discovery is
// ~40 s, which is the right shape: the Station you invested in uncovers the
// tree faster.
//
// The second anchor is the tree's own prices. `automation` costs 8 Research
// Data and `armor_bench` 24, so an attempt is half the cheapest node and a
// sixth of a bench — **finding a node is cheaper than researching it**, or
// discovery would read as a second research cost rather than as a prelude.
//
// **This is the one number in the feature that only play can confirm**, and
// nothing here can be played by an agent. What the above is blind to: how
// often a player actually leaves a Station idle, and whether twenty
// discoveries spread across ten sectors feels like uncovering a tree or like
// waiting for one.

/// Research Data banked per study attempt — `ActiveResearch::study`'s cap,
/// and the bar `Game::settle_study` spends. See the section note above.
pub const STUDY_ATTEMPT_DATA: u32 = 4;

/// The roll a spent study attempt makes. A miss logs and costs the attempt;
/// there is no pity counter, because the bar itself is already the pacing.
/// See the section note above.
pub const STUDY_DISCOVERY_CHANCE: f64 = 0.35;
```

- [ ] **Step 6: Run the tests**

Run: `cargo fmt && cargo clippy --workspace --all-targets && cargo test -p feral-processes-engine research`
Expected: PASS.

- [ ] **Step 7: Run the whole workspace**

Run: `cargo test --workspace`
Expected: PASS. `settle_study` now runs on every tick of every test that ticks; anything that fails here is a seeded outcome moving because a study drew where nothing drew before. Confirm each failure is an outcome and not a rule, and re-seed the fixture rather than loosening the assertion.

- [ ] **Step 8: Commit**

```bash
git add crates/engine/src/game/unlocks.rs crates/engine/src/game/turn.rs \
        crates/engine/src/tuning.rs crates/engine/src/tests/research.rs
git commit -m "A study attempt rolls over what the base could find"
```

---

### Task 8: The gate

**Files:**
- Modify: `crates/engine/src/tests/research.rs` (one end-to-end test)
- Modify: `assets/research/README.md` (only if Task 1's wording drifted)

**Interfaces:**
- Consumes: everything above.
- Produces: nothing.

- [ ] **Step 1: Write the end-to-end test**

The seven tasks above each test one seam. This is the one test that proves the seams are joined — an idle Station with a subject pinned uncovers a node with no hand-set field anywhere.

```rust
/// **The feature, end to end, through nothing but ticks.** Every test above
/// reaches into `ActiveResearch::study` or calls `settle_study` directly;
/// this one posts a program, pins a subject and waits, so a seam left
/// unjoined — the accrual not wired, `settle_study` not called from the tick
/// — fails here and nowhere else.
#[test]
fn an_idle_station_with_a_subject_pinned_uncovers_a_node_on_its_own() {
    let mut game = Game::new(4470, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    unlock_research_chain(&mut game, "automation");
    let worker = spawn_tamed(&mut game, 10, 3);
    park_at_post(&mut game, worker, node);
    game.set_standing_job(node, true, false).unwrap();
    let subject = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, subject, node);
    assert_eq!(active_research(&game), None, "the Station studies, not works");

    for _ in 0..4000 {
        game.tick();
    }

    let found: Vec<String> = game
        .world
        .resource::<crate::resources::DiscoveredResearch>()
        .0
        .iter()
        .cloned()
        .collect();
    assert_eq!(
        found,
        vec!["armor_bench".to_string()],
        "zone 1 with only automation researched has exactly one eligible node"
    );
    assert!(
        game.research_nodes(ResearchTree::Base)
            .iter()
            .any(|n| n.id == "armor_bench"),
        "and it arrives on the menu"
    );
    assert!(
        game.world.get::<Stats>(subject).is_some(),
        "discovery is free — the subject survives every attempt"
    );
}
```

If 4000 ticks is not enough, read the Station's actual cycle in the fixture before raising it — a number that had to be raised is evidence the pricing in Task 7 is wrong, not evidence the test is impatient. Check whether `weapon_bench` is also eligible (it sits behind `routine_fabrication`, which `unlock_research_chain(…, "automation")` does not research) and relax the `assert_eq!` to a `contains` if the tree says otherwise.

- [ ] **Step 2: Run the full gate**

```bash
cargo fmt
cargo clippy --workspace --all-targets
cargo test --workspace
```

Expected: all three clean. Record the actual test count from the run — `CLAUDE.md`'s figure moves with the branch.

- [ ] **Step 3: Check the documentation obligations**

- `assets/research/README.md` documents `discoverable` (Task 1). Re-read it against the final behaviour.
- `grep` for claims this change falsifies:
  `rg -n "research tree" docs/ README.md CHANGELOG.md | head -30` — anything describing the base tree as fully visible from turn one needs correcting. `docs/manual.md` is carved out; leave it stale.
- Do **not** bump the workspace version or add a `CHANGELOG.md` section; both happen at the merge.

- [ ] **Step 4: Commit**

```bash
git add crates/engine/src/tests/research.rs assets/research/README.md
git commit -m "The study finds a node with nothing but ticks"
```

- [ ] **Step 5: Say what has not been verified**

This feature has had zero screen time. `STUDY_ATTEMPT_DATA` and `STUDY_DISCOVERY_CHANCE` are priced from the Station's arithmetic, not from play, and the spec says plainly that they are the one thing only play can confirm. Report that to the user in one sentence when handing the branch over — a green suite is not evidence of play.

---

## Out of scope (spec §9 — do not build)

- No `Game::attention` row for a Station studying, idle, or holding a full bar with nothing to find.
- No discovery source other than study. `Game::discover_research` being `pub` is the whole of the provision made for one.
- No steering by the subject's species — the roll is uniform, asked and answered.
- No player-facing study progress readout. Whether the Station's examine line should quote `ActiveResearch::study` is a question for after the rate is felt.
- `PinMark::Strained` is unchanged: a subject under study is not being spent, so it stays `Settled`.
