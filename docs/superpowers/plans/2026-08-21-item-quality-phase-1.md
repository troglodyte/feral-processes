# Item quality — Phase 1: the axis

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Put the `quality` field on `GearCopy`, wire it through the save,
give it its scaling function and its band, and prove an existing save loads
at 100 — without anything rolling it yet.

**Architecture:** Engine only. Every copy in the game is at
`QUALITY_DEFAULT` when this phase lands, so `copy_bonus` returns exactly
what it returns today and no screen changes. What ships is the seam:
the ledger key gains a fourth property, `EquipmentStats::for_quality` takes
its place third of four in the scaling chain, and `items::quality_band`
exists for the renderer to read in Phase 5.

**Tech Stack:** Rust, `bevy_ecs` 0.19, `serde` + RON saves.

**Spec:** `docs/superpowers/specs/2026-08-21-item-quality-design.md`

**Roadmap:** `docs/superpowers/plans/2026-08-21-item-quality-plan.md` — its
**Global Constraints** section applies to every task here. Read it first.

**Branch:** `item-quality` (already checked out). Check
`git branch --show-current` before every commit — another session has
fast-forwarded and deleted a branch mid-task in this repo before.

---

### Task 1: The field, its default, and the fourth `&&`

**Files:**
- Modify: `crates/engine/src/tuning.rs` (new constant, near
  `GEAR_RARITY_MIN_BONUS_PER_RUNG` at line 1394)
- Modify: `crates/engine/src/items.rs:181-231` (`GearCopy`, `plain`,
  `is_plain`)
- Modify: every `GearCopy { .. }` struct literal in the workspace — about
  30 sites, listed in Step 4
- Test: `crates/engine/src/tests/equipment.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `tuning::QUALITY_DEFAULT: u8` (= 100);
  `items::GearCopy::quality: u8`; `items::default_quality() -> u8` (private
  to `items.rs`, named in the `serde` attribute).

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/equipment.rs` (it already has
`use super::*;`-style imports at the top — add `crate::tuning::QUALITY_DEFAULT`
to them):

```rust
/// **Quality is the fourth thing that makes a copy special**, so it joins
/// `is_plain`'s `&&` and with it the choice of cargo store. A copy that
/// compiled off spec is not interchangeable with one that did, which is
/// exactly the question `Inventory`-or-`GearCopies` asks.
#[test]
fn an_off_spec_copy_is_not_plain() {
    let whip = ItemId::from(ids::MONOFILAMENT_WHIP);
    let plain = GearCopy::plain(whip);
    assert!(plain.is_plain(), "a copy compiled to spec still stacks");
    assert_eq!(plain.quality, QUALITY_DEFAULT);

    let off_spec = GearCopy {
        quality: QUALITY_DEFAULT - 5,
        ..plain.clone()
    };
    assert!(!off_spec.is_plain());

    let over_spec = GearCopy {
        quality: QUALITY_DEFAULT + 5,
        ..plain
    };
    assert!(!over_spec.is_plain(), "better than spec is special too");
}

/// The store split follows `is_plain` and nothing else, so an off-spec copy
/// lands in the ledger that can tell two copies apart rather than stacking
/// namelessly in `Inventory`.
#[test]
fn an_off_spec_copy_lands_in_the_gear_ledger() {
    let assets = test_assets_dir();
    let mut game = Game::new(211, DifficultyMode::Forgiving, &assets).unwrap();
    let whip = ItemId::from(ids::MONOFILAMENT_WHIP);
    let off_spec = GearCopy {
        quality: QUALITY_DEFAULT + 10,
        ..GearCopy::plain(whip.clone())
    };

    game.add_copies(&off_spec, 1);

    let player = game.player_entity();
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().count(&whip),
        0,
        "an off-spec copy must not stack in the plain-copy store"
    );
    assert_eq!(game.count_copies(&off_spec), 1);
}
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p feral-processes-engine an_off_spec_copy`
Expected: FAIL to compile — `struct GearCopy has no field named quality`.

- [ ] **Step 3: Add the constant**

In `crates/engine/src/tuning.rs`, beside the gear constants:

```rust
// ---------------------------------------------------------------------------
// Item quality
// ---------------------------------------------------------------------------

/// What a copy compiled exactly to its authored spec is worth, as a
/// percentage of `ItemDef::equipment`'s numbers — and what every copy in
/// every save written before the field existed loads as.
///
/// It is the identity element of `EquipmentStats::for_quality`, which is
/// why the whole band is expressed as a percentage of it rather than as a
/// multiplier: an authored item is the reference, not a floor.
pub const QUALITY_DEFAULT: u8 = 100;
```

- [ ] **Step 4: Add the field**

In `crates/engine/src/items.rs`, add `QUALITY_DEFAULT` to the existing
`crate::tuning::*` import, then add the field last in `GearCopy`:

```rust
    /// How well this particular copy was compiled, as a percentage of the
    /// item's authored bonus — see `EquipmentStats::for_quality`.
    /// `QUALITY_DEFAULT` is "exactly as designed".
    ///
    /// **An integer on purpose.** This struct is the key of the
    /// `components::GearCopies` ledger and of `EquippedItem`; both find
    /// rows by `==`, so a float would take `Eq` and the keyed-by-value seam
    /// with it.
    ///
    /// `default = "default_quality"` rather than a bare `#[serde(default)]`:
    /// `u8`'s `Default` is 0, which would load every piece of gear in every
    /// existing save at 0% of its authored bonus — a total loss of stats
    /// presenting as a balance bug rather than as a failed load.
    #[serde(default = "default_quality")]
    pub quality: u8,
```

Below the struct, beside it rather than in `tuning.rs`, because it exists
only to be named in that attribute:

```rust
/// `serde`'s default for `GearCopy::quality` — see that field.
fn default_quality() -> u8 {
    QUALITY_DEFAULT
}
```

Then `plain` and `is_plain`:

```rust
    pub fn plain(item: ItemId) -> Self {
        Self {
            item,
            rarity: Rarity::Ordinary,
            tier: 0,
            affix: None,
            quality: QUALITY_DEFAULT,
        }
    }
```

```rust
    pub fn is_plain(&self) -> bool {
        self.rarity == Rarity::Ordinary
            && self.tier == 0
            && self.affix.is_none()
            && self.quality == QUALITY_DEFAULT
    }
```

- [ ] **Step 5: Fix every struct literal**

Run `cargo test --workspace --no-run` and work the compiler's list. There
are about 30, in three groups — treat them differently:

*Production sites — spell the value out, so the site says what it means:*
- `crates/engine/src/game/combat_rewards.rs:87` (`grant_gear_drop`) —
  `quality: QUALITY_DEFAULT` for now; Phase 2 replaces it with the roll.
- `crates/engine/src/game/crafting.rs:643` (`fuse_item`) — already
  `..copy.clone()`, so **it needs no change**: fusion consumes copies of
  one exact key, so the fused copy keeps the quality both parents had.
  Verify this rather than assuming it.
- `crates/engine/src/game/lifecycle.rs:367` and `:513` — the two pre-0.8.9
  legacy stores being drained. `quality: QUALITY_DEFAULT`: those copies
  predate the field by two years of releases.
- `crates/engine/src/game/lifecycle.rs:42` (`worn_from_save`) — leave the
  literal alone for now and let it stay broken; **Task 2 gives it a
  parameter.** If you need the workspace to compile between the two tasks,
  pass `QUALITY_DEFAULT` and delete that line in Task 2.
- `crates/engine/src/arena/scenario.rs:140` — `quality: QUALITY_DEFAULT`; a
  scenario authors no quality and must stay comparable to its old reports.

*Test fixtures — use struct-update syntax, so a fifth property does not
break them again:*
- `crates/engine/src/tests/support.rs:1854` (`gear`),
  `crates/app-core/src/tests/support.rs:821` (`affixed_gear`) and `:964`
  (`gear`), and the ~15 literals in `tests/{gear_detail,combat_rewards,
  gear_passives,trade,affixes,equipment}.rs` and
  `crates/app-core/src/tests/inventory.rs`.
- Shape: `GearCopy { rarity, tier, ..GearCopy::plain(item.clone()) }`.

*Renderer fixtures:* `crates/gui/src/render/inventory.rs:625` and `:732`,
same struct-update shape.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p feral-processes-engine an_off_spec_copy`
Expected: PASS, both.

- [ ] **Step 7: Mutation-check**

Delete the `&& self.quality == QUALITY_DEFAULT` clause, re-run: both tests
must fail. Restore it and confirm they pass again. Record the result in the
commit body.

- [ ] **Step 8: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/ 
git commit -m "feat(engine): a gear copy carries the quality it compiled at"
```

---

### Task 2: The save carries it, worn and carried

**Files:**
- Modify: `crates/engine/src/save.rs:34-60` (`PlayerSave`'s nine flat
  equipment fields), `:325-353` (`EquippedItemSave`), and the `PlayerSave`
  fixture around `:912`
- Modify: `crates/engine/src/game/lifecycle.rs:35-51` (`worn_from_save`),
  `:53-61` (`worn_to_save`), `:456-477` (the three `worn_from_save` calls),
  `:1182-1207` (the three slots' save writers)
- Test: `crates/engine/src/tests/equipment.rs`

**Interfaces:**
- Consumes: `GearCopy::quality`, `tuning::QUALITY_DEFAULT` from Task 1.
- Produces: `save::EquippedItemSave::quality: u8`;
  `save::PlayerSave::{weapon_quality, armor_quality, module_quality}: u8`;
  `worn_from_save(item, level, fusion_tier, rarity, affix, quality)`.

**Why this is a separate task from Task 1.** Carried copies ride `serde`
through `data.player.gear_copies` and need no code at all. **Worn copies do
not** — the save keeps them as flat fields on purpose (see
`EquippedItemSave`'s doc: nesting is a shape change RON cannot absorb), so
each worn slot needs its own additive field. Missing this is invisible
until someone reloads wearing a copy that is not at 100, which is Phase 2.

- [ ] **Step 1: Write the failing test**

```rust
/// A worn copy's quality is four flat save fields rather than a nested
/// `GearCopy`, which is the shape `EquippedItemSave`'s doc argues for — so
/// it is also four places the field can be forgotten. This is the test that
/// notices.
#[test]
fn a_worn_off_spec_copy_survives_save_and_load() {
    let assets = test_assets_dir();
    let whip = ItemId::from(ids::MONOFILAMENT_WHIP);
    let mut game = Game::new(212, DifficultyMode::Forgiving, &assets).unwrap();
    let copy = GearCopy {
        quality: QUALITY_DEFAULT + 15,
        ..GearCopy::plain(whip.clone())
    };
    game.add_copies(&copy, 1);
    game.equip(game.player_entity(), &copy).unwrap();
    let atk_before = game.player_status().atk;

    let path = std::env::temp_dir().join(format!(
        "feral_processes_quality_worn_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    let worn = loaded
        .worn(loaded.player_entity(), crate::items::EquipmentSlot::Weapon)
        .expect("the whip is still on");
    assert_eq!(worn.copy.quality, QUALITY_DEFAULT + 15);
    assert_eq!(
        loaded.player_status().atk,
        atk_before,
        "reloading must not change what the worn copy is worth"
    );
}
```

`Game::worn(wearer, slot) -> Option<EquippedItem>` lives at
`crates/engine/src/game/party.rs:630` and is already `pub` — do not add a second accessor.

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p feral-processes-engine a_worn_off_spec_copy`
Expected: FAIL — the reloaded copy reads `QUALITY_DEFAULT`, not 115.

- [ ] **Step 3: Add the save fields**

`EquippedItemSave` gains, after `affix`:

```rust
    /// How well the worn copy was compiled — see `items::GearCopy::quality`.
    /// Additive behind a default of `QUALITY_DEFAULT` rather than `u8`'s own
    /// `Default` of 0, which would silently strip a worn item of its whole
    /// bonus on the first reload.
    #[serde(default = "default_worn_quality")]
    pub quality: u8,
```

with, beside it in `save.rs`:

```rust
fn default_worn_quality() -> u8 {
    crate::tuning::QUALITY_DEFAULT
}
```

`PlayerSave` gains the same field three times, following its existing
`weapon_/armor_/module_` naming and each `#[serde(default = "default_worn_quality")]`.

- [ ] **Step 4: Wire both directions**

`worn_to_save` adds `quality: worn.copy.quality`. `worn_from_save` takes a
sixth parameter `quality: u8` and sets it on the `GearCopy` it builds. The
three call sites in `Game::load` pass `data.player.<slot>_quality`; the
three writers in `Game::save` pass
`equipment.<slot>.as_ref().map(|e| e.copy.quality).unwrap_or(QUALITY_DEFAULT)`.
Fix the `PlayerSave` fixture around `save.rs:912`.

**Do not bump `SAVE_FORMAT_VERSION`.** Every field here is additive and
defaulted on a named struct, which the save seam says costs no bump.

- [ ] **Step 5: Run the test**

Run: `cargo test -p feral-processes-engine a_worn_off_spec_copy`
Expected: PASS.

- [ ] **Step 6: Mutation-check**

Change `worn_to_save`'s new line to `quality: QUALITY_DEFAULT`; the test
must fail. Restore, confirm green.

- [ ] **Step 7: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine
git commit -m "feat(engine): worn gear saves the quality it was compiled at"
```

---

### Task 3: An existing save loads its gear at 100

**Files:**
- Test only: `crates/engine/src/tests/equipment.rs`

**Interfaces:**
- Consumes: everything from Tasks 1 and 2.
- Produces: nothing. This is the guard that the whole no-version-bump
  argument rests on.

**Why textual.** The save is field-named RON with the version as its first
line (`save.rs:747`), so a pre-quality save is exactly this save with the
`quality:` lines removed. A RON round-trip in memory cannot prove a
defaulting fault — the standing note on `#[serde(skip)]` is the same trap —
so the file has to be edited on disk and loaded back.

- [ ] **Step 1: Write the failing test**

```rust
/// **A save written before this field loads its gear at 100.** That claim
/// is the whole reason the field costs no `SAVE_FORMAT_VERSION` bump, and
/// it is a claim about a file, so this edits one: a real save with every
/// `quality:` line stripped is byte-for-byte what the previous release
/// wrote.
#[test]
fn a_pre_quality_save_loads_its_gear_as_designed() {
    let assets = test_assets_dir();
    let whip = ItemId::from(ids::MONOFILAMENT_WHIP);
    let plating = ItemId::from(ids::ABLATIVE_PLATING);
    let mut game = Game::new(213, DifficultyMode::Forgiving, &assets).unwrap();
    // One worn (flat save fields) and one carried (serde'd `GearCopy`),
    // because they travel by different routes.
    let worn = GearCopy::plain(whip.clone());
    game.add_copies(&worn, 1);
    game.equip(game.player_entity(), &worn).unwrap();
    let carried = GearCopy {
        tier: 1,
        ..GearCopy::plain(plating.clone())
    };
    game.add_copies(&carried, 2);

    let path = std::env::temp_dir().join(format!(
        "feral_processes_pre_quality_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    let stripped: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("quality:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        stripped.len() < text.len(),
        "the save has to have carried the field, or this proves nothing"
    );
    std::fs::write(&path, &stripped).unwrap();

    let loaded = Game::load(&path, &assets).expect("an older save still loads");
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded
            .worn(loaded.player_entity(), crate::items::EquipmentSlot::Weapon)
            .expect("still wearing it")
            .copy
            .quality,
        QUALITY_DEFAULT,
    );
    assert_eq!(loaded.count_copies(&carried), 2, "the carried copy is unchanged");
}
```

If the RON pretty-printer puts `quality: 100,` on the same line as another
field, the filter above strips nothing and the assertion at the top of the
test says so. In that case strip with a `replace("quality: 100,", "")` on
the whole string instead — the assertion is what makes either version
honest.

- [ ] **Step 2: Run it**

Run: `cargo test -p feral-processes-engine a_pre_quality_save`
Expected: PASS on the first run — Tasks 1 and 2 already did the work. This
task's deliverable is the *proof*, so go straight to the mutation check.

- [ ] **Step 3: Mutation-check, both halves**

1. Change `GearCopy::quality`'s attribute to a bare `#[serde(default)]` →
   the carried assertion must fail (it loads at 0).
2. Restore it, change `EquippedItemSave::quality` to a bare
   `#[serde(default)]` → the worn assertion must fail.
3. Restore both, confirm green.

Both halves must fail independently, or the test is only covering one route.

- [ ] **Step 4: Commit**

```bash
git add crates/engine/src/tests/equipment.rs
git commit -m "test(engine): a save written before quality loads gear as designed"
```

---

### Task 4: `EquipmentStats::for_quality`

**Files:**
- Modify: `crates/engine/src/items.rs` — add after `for_rarity` (ends
  ~line 435), inside the same `impl EquipmentStats`
- Test: `crates/engine/src/items.rs`'s own `#[cfg(test)] mod tests`
  (line 450), beside `scaled_for_level_adds_100_percent_of_base_per_level_above_1`

**Interfaces:**
- Consumes: `tuning::QUALITY_DEFAULT`.
- Produces: `pub(crate) fn for_quality(self, quality: u8) -> EquipmentStats`.

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn for_quality_is_a_percentage_of_the_authored_bonus() {
        let base = EquipmentStats {
            atk: 10,
            mitigation: 4,
            damage: crate::battle::DamageRange { min: 5, max: 15 },
            ..EquipmentStats::default()
        };

        let same = base.for_quality(QUALITY_DEFAULT);
        assert_eq!(same.atk, 10, "100% is the identity");
        assert_eq!(same.damage.min, 5);
        assert_eq!(same.damage.max, 15);

        let good = base.for_quality(130);
        assert_eq!(good.atk, 13);
        assert_eq!(good.mitigation, 5);
        assert_eq!((good.damage.min, good.damage.max), (7, 20),
            "both ends of a band scale, or a high roll collapses it to a point");

        let poor = base.for_quality(70);
        assert_eq!(poor.atk, 7);
        assert_eq!((poor.damage.min, poor.damage.max), (4, 11));
    }

    /// The two rules `for_rarity` states, reachable here by the same route:
    /// quality sharpens what an item does rather than handing it a stat it
    /// never had, and improving a copy never deepens a drawback affix's
    /// penalty.
    #[test]
    fn for_quality_leaves_a_zero_at_zero_and_a_negative_where_it_is() {
        let base = EquipmentStats {
            atk: 0,
            evasion: -3,
            ..EquipmentStats::default()
        };
        for quality in [70u8, QUALITY_DEFAULT, 130] {
            let scaled = base.for_quality(quality);
            assert_eq!(scaled.atk, 0, "a stat the item does not have stays absent");
            assert_eq!(scaled.evasion, -3, "a drawback is never deepened");
        }
    }
```

- [ ] **Step 2: Run and watch them fail**

Run: `cargo test -p feral-processes-engine for_quality`
Expected: FAIL to compile — no method `for_quality`.

- [ ] **Step 3: Implement it**

```rust
    /// This item's bonus scaled for how well *this copy* was compiled
    /// (`QUALITY_DEFAULT` = the authored numbers, no scaling) — see
    /// `items::GearCopy::quality`. Applied on top of `scaled_for_level` and
    /// underneath the two floored axes; `Game::copy_bonus` owns that order
    /// and argues for it.
    ///
    /// **No per-step floor, unlike its two siblings.** Theirs exist to make
    /// a *discrete rung* observable at the magnitudes gear ships at; quality
    /// is continuous and is meant to be a fine gradient, and a floor would
    /// flatten the whole band onto one number on a 4-point stat.
    ///
    /// Being floor-free is also why it cannot go last: a bare percentage on
    /// an unscaled 4-point stat is eaten by rounding, and worse, it can
    /// invert the rare tiers — base atk 4 gives a `Silver` copy at 70% the
    /// same 4 an `Ordinary` copy at 130% rounds up to 5, which makes the row
    /// colour a lie about which copy is better.
    ///
    /// A stat at zero stays at zero and a negative one is left alone, both
    /// for the reasons `for_rarity` gives.
    pub(crate) fn for_quality(self, quality: u8) -> EquipmentStats {
        let factor = quality as f64 / QUALITY_DEFAULT as f64;
        let scale = |v: i32| {
            if v <= 0 {
                return v;
            }
            (v as f64 * factor).round() as i32
        };
        EquipmentStats {
            atk: scale(self.atk),
            mitigation: scale(self.mitigation),
            decompiler: scale(self.decompiler),
            damage: scale_range(self.damage, scale),
            accuracy: scale(self.accuracy),
            evasion: scale(self.evasion),
        }
    }
```

- [ ] **Step 4: Run them**

Run: `cargo test -p feral-processes-engine for_quality`
Expected: PASS, both.

- [ ] **Step 5: Mutation-check**

Replace `scale_range(self.damage, scale)` with `self.damage`; the band
assertions must fail. Then drop the `if v <= 0` guard; the second test must
fail. Restore both.

- [ ] **Step 6: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/items.rs
git commit -m "feat(engine): gear scales by the quality its copy compiled at"
```

---

### Task 5: `for_quality` takes its place in the chain

**Files:**
- Modify: `crates/engine/src/game/crafting.rs:306-334` (`Game::copy_bonus`)
- Test: `crates/engine/src/tests/equipment.rs`

**Interfaces:**
- Consumes: `EquipmentStats::for_quality` from Task 4.
- Produces: no signature change. `Game::copy_bonus(&self, copy, level)`
  now honours `copy.quality`.

- [ ] **Step 1: Write the failing tests**

```rust
/// **A rare tier's floor is guaranteed against a copy of equal quality**,
/// not globally — which is the honest form of the guarantee and the reason
/// the two floored axes stay last in the chain. Swept over the whole band
/// and both ends of the level range.
#[test]
fn a_rarer_copy_beats_an_ordinary_one_of_equal_quality() {
    let assets = test_assets_dir();
    let game = Game::new(214, DifficultyMode::Forgiving, &assets).unwrap();
    let whip = ItemId::from(ids::MONOFILAMENT_WHIP);

    for quality in [70u8, 85, QUALITY_DEFAULT, 115, 130] {
        for level in [1u32, 10] {
            let ordinary = GearCopy {
                quality,
                ..GearCopy::plain(whip.clone())
            };
            let silver = GearCopy {
                rarity: Rarity::Silver,
                ..ordinary.clone()
            };
            let plain_stats = game.copy_bonus(&ordinary, level).unwrap();
            let rare_stats = game.copy_bonus(&silver, level).unwrap();
            assert!(
                rare_stats.atk > plain_stats.atk,
                "Silver at {quality}% and level {level} must beat Ordinary at the same quality"
            );
            assert!(rare_stats.damage.max >= plain_stats.damage.max);
        }
    }
}

/// Quality lands *after* level scaling, which is what gives it a number
/// with enough resolution to bite. Applied last instead, a 4-point stat at
/// level 1 would round the whole band flat.
#[test]
fn quality_moves_a_copys_bonus_at_every_level() {
    let assets = test_assets_dir();
    let game = Game::new(215, DifficultyMode::Forgiving, &assets).unwrap();
    let whip = ItemId::from(ids::MONOFILAMENT_WHIP);
    let spec = GearCopy::plain(whip.clone());
    let good = GearCopy {
        quality: 130,
        ..spec.clone()
    };
    let poor = GearCopy {
        quality: 70,
        ..spec.clone()
    };

    for level in [1u32, 5, 10] {
        let at_spec = game.copy_bonus(&spec, level).unwrap().atk;
        assert!(game.copy_bonus(&good, level).unwrap().atk > at_spec);
        assert!(game.copy_bonus(&poor, level).unwrap().atk < at_spec);
    }
}
```

- [ ] **Step 2: Run and watch them fail**

Run: `cargo test -p feral-processes-engine quality_moves_a_copys_bonus`
Expected: FAIL — the two copies price identically, because `copy_bonus`
does not read `quality` yet.

- [ ] **Step 3: Wire it in**

In `Game::copy_bonus`, the tail becomes:

```rust
        Some(
            affixed
                .scaled_for_level(level)
                .for_quality(copy.quality)
                .fused_for_tier(copy.tier)
                .for_rarity(copy.rarity),
        )
```

Add to that function's doc comment, above the existing prose:

```rust
    /// **The order of the four axes is load-bearing.** The affix is folded
    /// into the base, then level, then quality, then the two floored axes.
    /// Quality carries no floor and so cannot go last (see
    /// `EquipmentStats::for_quality`); keeping fusion and rarity after it is
    /// what preserves the honest form of their guarantee — a rare tier's
    /// floor is worth a rung *against a copy of equal quality*.
```

- [ ] **Step 4: Run the whole engine suite**

Run: `cargo test -p feral-processes-engine`
Expected: PASS. Everything is still at `QUALITY_DEFAULT`, so no existing
figure may move. **If a balance or equipment test moves here, stop** — it
means something is constructing a copy at a quality it did not intend, and
that is a fault in Task 1's literal sweep, not a number to re-baseline.

- [ ] **Step 5: Mutation-check**

Remove the `.for_quality(copy.quality)` line; both new tests must fail.
Then move it to the end of the chain and re-run: `quality_moves_a_copys_bonus`
should still pass but `a_rarer_copy_beats_an_ordinary_one_of_equal_quality`
must fail at level 1 — that failure *is* the ordering argument, so confirm
you see it before restoring.

- [ ] **Step 6: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine
git commit -m "feat(engine): copy_bonus prices quality between level and the floored axes"
```

---

### Task 6: `items::quality_band`

**Files:**
- Modify: `crates/engine/src/tuning.rs` (four more constants beside
  `QUALITY_DEFAULT`)
- Modify: `crates/engine/src/items.rs` (the enum and the function, beside
  `for_quality`)
- Test: `crates/engine/src/items.rs`'s `mod tests`

**Interfaces:**
- Consumes: `tuning::QUALITY_DEFAULT`.
- Produces: `items::QualityBand { Under, AsDesigned, Above, Exceptional }`
  (`pub`, `Clone + Copy + Debug + PartialEq + Eq`) and
  `items::quality_band(quality: u8) -> QualityBand` (`pub`);
  `tuning::{QUALITY_MIN, QUALITY_MAX, QUALITY_UNDER_MAX, QUALITY_SPEC_MAX,
  QUALITY_ABOVE_MAX}`.

**Why the engine owns the thresholds:** five renderer sites will build the
tag in Phase 5 and an engine-owned rule is what stops them drifting — the
argument `Rarity::label` and `Game::copy_name` already make. The renderer
owns the palette, because a band carrying a *weight* as well as a hue is
not expressible as a colour.

- [ ] **Step 1: Write the failing test**

```rust
    /// Every boundary in the four-band ladder, and the one that matters
    /// most: `QUALITY_DEFAULT` lands in the band that reads as no change,
    /// so every copy in every existing save is repainted by nothing.
    #[test]
    fn quality_band_buckets_the_whole_range() {
        use crate::tuning::{QUALITY_MAX, QUALITY_MIN};
        assert_eq!(quality_band(QUALITY_MIN), QualityBand::Under);
        assert_eq!(quality_band(90), QualityBand::Under);
        assert_eq!(quality_band(95), QualityBand::AsDesigned);
        assert_eq!(quality_band(QUALITY_DEFAULT), QualityBand::AsDesigned);
        assert_eq!(quality_band(105), QualityBand::AsDesigned);
        assert_eq!(quality_band(110), QualityBand::Above);
        assert_eq!(quality_band(120), QualityBand::Above);
        assert_eq!(quality_band(125), QualityBand::Exceptional);
        assert_eq!(quality_band(QUALITY_MAX), QualityBand::Exceptional);
    }
```

- [ ] **Step 2: Run and watch it fail**

Run: `cargo test -p feral-processes-engine quality_band`
Expected: FAIL to compile — no `quality_band`, no `QualityBand`.

- [ ] **Step 3: Add the constants**

Beside `QUALITY_DEFAULT` in `tuning.rs`:

```rust
/// The clamp on a rolled quality. Both ends are reachable — a fresh
/// player's craft can hit the floor and a developed base's can hit the
/// ceiling — so they are the band, not guard rails.
pub const QUALITY_MIN: u8 = 70;
pub const QUALITY_MAX: u8 = 130;

/// The three cuts in `items::quality_band`'s four-rung ladder: at or below
/// the first reads as under spec, the middle band as designed, the third as
/// above spec, and anything higher as exceptional.
///
/// The middle band is centred on `QUALITY_DEFAULT` on purpose: every copy
/// in every existing save sits there, so the ladder repaints nothing that
/// is already on screen.
pub const QUALITY_UNDER_MAX: u8 = 90;
pub const QUALITY_SPEC_MAX: u8 = 105;
pub const QUALITY_ABOVE_MAX: u8 = 120;
```

- [ ] **Step 4: Add the band**

In `items.rs`, beside `for_quality`:

```rust
/// Which of four rungs a copy's quality reads as. The renderer maps this to
/// a colour and a weight; the thresholds are the engine's so the five sites
/// that draw a category tag cannot come to disagree about them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualityBand {
    Under,
    AsDesigned,
    Above,
    Exceptional,
}

/// Which band `quality` falls in — see `QualityBand` and
/// `tuning::QUALITY_UNDER_MAX` and its two siblings.
pub fn quality_band(quality: u8) -> QualityBand {
    match quality {
        q if q <= QUALITY_UNDER_MAX => QualityBand::Under,
        q if q <= QUALITY_SPEC_MAX => QualityBand::AsDesigned,
        q if q <= QUALITY_ABOVE_MAX => QualityBand::Above,
        _ => QualityBand::Exceptional,
    }
}
```

Add the three new constants to `items.rs`'s `crate::tuning` import.

- [ ] **Step 5: Run it**

Run: `cargo test -p feral-processes-engine quality_band`
Expected: PASS.

- [ ] **Step 6: Mutation-check**

Change `q <= QUALITY_SPEC_MAX` to `q < QUALITY_SPEC_MAX`; the test must
fail at 105. Restore.

- [ ] **Step 7: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine
git commit -m "feat(engine): a copy's quality reads as one of four bands"
```

---

### Task 7: The seam, written down, and the full gate

**Files:**
- Modify: `docs/seams.md` (the items/gear section)
- Modify: `CLAUDE.md` (the **Items, gear and economy** list), then
  `cp CLAUDE.md AGENTS.md`

**Interfaces:**
- Consumes: everything above.
- Produces: the written rule, which is what stops the next session
  re-deriving the chain order or "fixing" the `serde` default.

**Do not touch** `docs/manual.md`, root `README.md`, or `TODO.md`.
`CHANGELOG.md` and the version bump happen at the merge, not here.

- [ ] **Step 1: Write the seam entry**

Add to `docs/seams.md`, under the same heading `CLAUDE.md` uses, an entry
titled **"A copy's quality is the fourth axis, and it is an integer"**
carrying the argument rather than the rule: why `f32` breaks the ledger
key, why the default is a named function and not `#[serde(default)]`, why
the chain order puts a floor-free axis third, and the worked
`Silver`-at-70%-vs-`Ordinary`-at-130% inversion that says it cannot go
last. Cite the spec by path.

- [ ] **Step 2: Write the rule**

Add to `CLAUDE.md`'s **Items, gear and economy** list, in that file's voice
— the rule and the trap, no argument:

```markdown
- **A copy's quality is a fourth axis and an integer.** `GearCopy` is the
  `GearCopies` ledger's key and `EquippedItem` holds the same key, so an
  `f32` takes `Eq` with it. Its `serde` default is the *named function*
  `default_quality`, never a bare `#[serde(default)]` — `u8`'s own default
  is 0, which loads every existing save's gear at 0% of its bonus. Worn
  gear is four more flat save fields, not a nested copy, so the field can
  be forgotten in four places at once. In `copy_bonus` it sits **third**,
  after `scaled_for_level` and before the two floored axes: it carries no
  floor of its own, so applied last it is eaten by rounding and can invert
  the rare tiers on a 4-point stat.
```

- [ ] **Step 3: Copy the twin**

```bash
cp CLAUDE.md AGENTS.md
```

They are gitignored twins with no tracking to catch drift, so the copy is
the only thing keeping them in step.

- [ ] **Step 4: The full gate**

```sh
cargo fmt
cargo clippy --workspace
cargo test --workspace
```

Expected: all green, with the suite count up by the seven tests this phase
added and **no existing figure moved**. Also run
`cargo test -p feral-processes-engine balance_sim` explicitly and confirm
it is untouched: quality sits outside its gate by the same exclusion that
keeps `Rarity` out, so a moved curve here means something is rolling a
quality it should not be.

- [ ] **Step 5: Commit**

```bash
git add docs/seams.md CLAUDE.md AGENTS.md
git commit -m "docs: the quality axis, its integer key and its place in the chain"
```

---

## Phase 1 done when

- `cargo test --workspace` is green and no pre-existing test's expected
  numbers changed.
- A save written by the previous release loads, wearing gear, at 100.
- Nothing in the game rolls a quality yet — `rg 'quality' crates/engine/src
  --type rust` shows the field, the axis, the band, the save wiring and the
  tests, and no call to a roll.

**Hand-off to Phase 2:** `grant_gear_drop` at
`crates/engine/src/game/combat_rewards.rs:87` is where the first roll goes,
beside the rarity and affix rolls it already makes, and it must keep
spending **no** `GameRng` draw on a non-equippable.
