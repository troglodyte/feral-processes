---
paths:
  - "**/research*"
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
