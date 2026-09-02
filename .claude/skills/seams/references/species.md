# Species and data

- **A species' class is derived and has exactly one derivation**,
  `SpeciesDef::affinity_class` over `AffinityClass::of_axis`.
  `Game::creature_class` is the only door from an entity to it. `None` must
  mean *no base job* rather than a default class.
- **A species' *stat block* is derived too**, and its one definition is
  `species::stat_shape_faults`. It returns the **verdict** rather than the
  ingredients, returns **every** fault rather than the first, and **bosses are
  exempt**. Nothing in `SpeciesDb::load_dir` calls it, so a mod is never
  refused by it.
- **Two censuses are reported to the tuner rather than enforced on it**, and
  which is which is a cost question. Both run once on the winner and land in
  `report.md`; both were promoted out of their own tests so the tuner's copy
  cannot drift.
- **The player's class is `classes::PlayerClass`, not `AffinityClass`**, and
  the first five variants keep the latter's names *and order*. That is what
  made the split free: no `.ron` file needed editing and the positional
  bincode save reads back unchanged, so appending cost no
  `SAVE_FORMAT_VERSION` bump — which makes the variant order save format,
  `Perk`'s rule. `AffinityClass` stays a *species'* derived role and keys
  `ClassShape`, `TalentDb` and `base_job_label`; adding the three
  player-only classes there would have made every one of those exhaustive
  matches answer for a class no species can hold. **There is deliberately no
  mapping between the two enums** — a `PlayerClass::affinity_class()` would
  look harmless and would immediately invite the base-job code to ask the
  player what job they do.
- **A class's *effect* is a named query in `classes.rs` and its catalogue
  entry is `.ron`** — `perks.rs`'s seam one directory over, exhaustive over
  `PlayerClass` so a ninth variant fails to compile. The queries never
  consult `ClassDb`, so an effect survives a deleted `assets/classes/` while
  the affinity spread and kit still go neutral; that asymmetry is deliberate
  and documented in that directory's README. The trap is `format_trade`: it
  is built from `affinities` alone, and all three player-only classes damp
  an axis without raising one, so without `spike_label` the Decompiler
  advertises itself as `"Weaker damage"` and nothing else.
- **The Invoker's routine slots are added past the cap and not re-clamped**,
  which is what the companion arm beside it has always done with
  `talent_routine_slots` — the cap bounds the level curve, not the total.
  Threading the term inside `player_routine_slots` before its own clamp
  reads as more careful and converges to nothing at level 25, deleting the
  one thing the class is named for; it also costs that function the purity
  `balance_sim` reads it as. A test at level 1 alone passes under both
  designs, so the census asserts at four levels including past the cap.
