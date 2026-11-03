---
paths:
  - "**/research*"
  - "**/research/**"
  - "**/routine_tree*"
  - "**/affix_tree*"
---

# Load-bearing seams: The research tree

One sentence each, the rule alone. The trap is in the `seams` skill; the argument is `seam:<slug>` in the memory graph.

- **The research tree's flow-chart layout is derived once, in
  `Game::research_graph`, and keyed by `ResearchId`** — `views::
  ResearchGraph::step` is the one rule for what an arrow key does, and
  app-core computes no neighbours.
- **A routine-tree node is synthesised from the Rust part catalogue, never
  authored, and "researched" means `Research` contains its id** — `part/<key>`
  or `capacity/<n>`; no research grants a finished routine.
- **A routine part is found through exactly three doors — extraction, a
  drop or trade that pays an exclusive's parts, and the study pool — all over
  `routine_tree::findable_parts`, the two paying doors rolled by
  `Game::roll_part_finds_for`;
  nothing findable (or a closed tree) draws nothing, an exclusive is never
  "carried", and a part no program carries is studied off any subject.**
