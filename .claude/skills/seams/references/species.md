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
