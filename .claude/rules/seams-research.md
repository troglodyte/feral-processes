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
- **A routine node is synthesised from `AbilityDb`, never authored, and
  "researched" means `KnownRoutines` contains it** — `Game::node_researched`
  is the one door every reader of a routine node's state goes through.
