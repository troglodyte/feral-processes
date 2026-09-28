---
paths:
  - "**/species*"
  - "**/handles*"
  - "assets/species/**"
---

# Load-bearing seams: Species and data

One sentence each, the rule alone. The trap is in the `seams` skill; the argument is `seam:<slug>` in the memory graph.

- **A species' class is derived and has exactly one derivation**,
  `SpeciesDef::affinity_class` over `AffinityClass::of_axis`.
- **A species' *stat block* is derived too**, and its one definition is
  `species::stat_shape_faults`.
- **Two censuses are reported to the tuner rather than enforced on it**, and
  which is which is a cost question.
- **A program's handle is derived from its `ProgramId` by `handles::of`,
  never stored, and the permutation's salt is save format.**
