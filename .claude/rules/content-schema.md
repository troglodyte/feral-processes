---
paths:
  - "assets/**"
  - "crates/engine/src/perks.rs"
  - "crates/engine/src/talents*"
  - "crates/engine/src/achievements*"
  - "crates/engine/src/tuning.rs"
  - "crates/engine/src/items.rs"
  - "crates/engine/src/species*"
  - "crates/engine/src/structures*"
  - "crates/engine/src/abilities*"
  - "crates/engine/tests/assets.rs"
---

# Content directories: the non-obvious parts

The per-directory schema is each `assets/*/README.md`. These are the rules
that are not in any one README.

- **Items:** `ItemId` (`crates/engine/src/items.rs`) is a string newtype, not
  an enum; the `ids` module there reaches shipped items from Rust for test
  setup and data-defined recipes, but a new item never requires Rust.
- **Abilities:** `priority_boost` must exist — it is the fallback for a
  companion whose species grants nothing.
- **Achievements:** the four `Trigger` and three `Reward` shapes are the whole
  vocabulary. The *ceiling* on what the ladder may pay is not data:
  `tuning::MAX_PROFILE_*`, asserted over the real assets by
  `the_full_ladder_stays_under_its_ceiling`, because `balance_sim` models one
  run and cannot see a cross-run profile.
- **Talents:** the five `TalentNode` kinds are the whole vocabulary; a sixth
  class's tree is a file. `Accuracy` is read on demand alongside `Affinity`
  (no `Stats` field to bake into). The tier count is fixed at
  `KERNEL_RING_MAX * LEVELS_PER_RING` tiers of two choices each, or the tree is
  skipped with a warning; six censuses in `tests/assets.rs` hold shipped trees
  to the design, so a new node kind costs every tree an existing choice.
- **Help pages** are markdown, not RON: five block rules, no front matter; the
  filename is both the ordering and the link id.
- **Perks are half data, deliberately.** `assets/perks/*.ron` is a catalogue
  (name, description, Perk Point cost) loaded by `PerkDb`, reached via
  `Game::perk_defs`. The `Perk` variants stay in `perks.rs` because each
  effect hooks a particular formula; `PerkDef` has no `effect` field.
  - A new perk = a new `Perk` variant + a **named query in `perks.rs`**, called
    from the site that applies it. `every_perk_has_a_query_that_answers_what_it_is_worth`
    is exhaustive on `Perk`, so a variant with no query fails to compile.
  - Two signature families, not interchangeable: most take the player's
    `Option<&Perks>` and name their own variant (no call site says `Perk::`);
    `mining_roll_bonus` and `quality_floor_bonus` take a bare **level**,
    because `CycleModifiers`/`CraftOrder` carry it to formulas whose subject
    may be a program — a `Perks` argument there hands a program the player's
    investment.
  - `StatGain` perks: `purchase_stat_gain` says what buying one grants;
    `unlock_perk` stays the one writer of `Stats`.
  - Per-level magnitudes stay in `tuning.rs`; only cost crossed to data.
  - A hook belongs where its sources meet (`Obfuscation` is read inside
    `raise_trace`, not at the six raisers). A repeated hook means the wrong seam.
  - **Variant order is save format** (bincode is positional;
    `PlayerSave::unlocked_perks` holds indices): append, or bump
    `SAVE_FORMAT_VERSION`.
- **`tuning.rs` is deliberately code, not data**: content is moddable,
  difficulty is not. Every hardcoded knob is a documented `pub const` there in
  labelled sections; new tuning goes there, never inline, and never a
  duplicate of a `.ron` value.
