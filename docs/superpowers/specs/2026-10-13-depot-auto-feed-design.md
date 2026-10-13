# Depot auto-feed

A Depot placed beside a machine that makes something should become that
machine's output bin without the player setting it up: filtered to what the
machine makes, and filled by the machine itself rather than by a carrier.
A machine on another side that consumes those items already pulls them out
of the Depot, so a Depot can sit in the middle of a production line.

Scope is the engine (`crates/engine`) plus the docs that describe Depots.
No schema field, no save-format change, no app-core or gui change.

## Decisions

| Question | Decision |
|---|---|
| Consumer side | Already shipped: `assembler_system` (`systems.rs:2083`) pulls a recipe's ingredients from any orthogonal neighbour's `Stock.output`, a Depot included, and only while a worker is posted. Untouched. |
| Pull gating | The Depot pulls from a producer only while that producer is worked (a `GatherResource` task targets it), the same test as the consumer side. |
| Filter timing | Set once, when a Depot and a producer first touch, whichever was built second. After that the filter is the player's. |
| Existing filter | A Depot with no filter becomes allow-only for the producer's output. A Depot that already has a filter only gains that output; a player's filter is never narrowed. |
| Two producers | The Depot allows both outputs and pulls from both. |
| Producer that also consumes | Included. A Depot between a Mining Node and a Lathe allows fragments and blank substrate; the Lathe pulls fragments out and the Depot pulls blank substrate in. |
| Haulers | A producer beside a Depot that accepts its output counts as attached: the crew leaves its output buffer alone, as for a producer feeding a machine. A full Depot backs the producer up. |
| Map joins | A Depot is drawn joined to a producer whose output it accepts. |
| Save format | Unchanged. The filter is set as a one-time event and `DepotFilter` is already saved. |

## Engine — `crates/engine`

### What a structure makes

A pure function, `structures::output_item(def: &StructureDef) -> Option<&ItemId>`:
`work.produces`, else `assembles.item`. The auto-filter, the intake system,
the hauler check and the map join all call it, so "producer" has one
definition.

### Auto-filter — `game/base/depot_filter.rs`

`Game::auto_filter_depots(built: Entity)`, called from `construction.rs:569`
right after `spawn_structure` finishes a crew build. (The only other caller,
`building.rs:268`, is the Home.)

- If `built` is a Depot: for each orthogonal neighbour with an `output_item`,
  apply that item to `built`.
- If `built` has an `output_item`: apply it to each orthogonal Depot.
- Apply = if the Depot has no `DepotFilter`, `set_all_depot_filters(false)`
  then `set_depot_filter(&[item], true)`; otherwise `set_depot_filter(&[item], true)`.
  Both go through the existing writers, which keep "an empty denied set
  removes the component".

Orthogonal means `collect::ORTHOGONAL`, never a new offset list.

### Intake — `depot_intake_system` in `systems.rs`

Runs in the same schedule as `assembler_system` (`lifecycle.rs:767`), after
it, so a consumer touching the producer directly gets first pick.

For each Depot in `(x, y)` order, for each `ORTHOGONAL` neighbour that:

- has an `output_item` the Depot's filter accepts,
- is worked (see below), and
- is not dark on the grid (`assembler_system`'s guard),

move `min(neighbour output of that item, Depot output_room)` from the
neighbour's `Stock.output` to the Depot's `Stock.output`, through
`hauling::take_from`. Plan against a snapshot and then apply, for
`assembler_system`'s borrow reason. Build the reach map with
`collect::feeders_by_tile`.

The filter test must be the same predicate as `Game::depot_accepts`. That one
takes `&Game`, so extract the body into a free function over
`Option<&DepotFilter>` and have both call it.

"Is worked": extract `assembler_system`'s inline
`tasks.iter().find(|t| t.target == machine && GatherResource)` into one
function that both systems call.

### Haulers — `game/base/hauling.rs`

`consumer_beside` (or its caller at the `attached` computation, ~line 1477)
also answers yes when an orthogonal Depot accepts the item. The existing
attached and clogged paths then apply unchanged. Departure stays in
`haul_step_system`; a base with no Depot behaves exactly as before.

### Map joins — `game/inspection.rs`

`linked_edges_by_structure` records a Depot–producer link when the Depot
accepts the producer's `output_item`, from both ends, as it does for a
machine pair. This keeps the drawn line and the pull from disagreeing.

## Tests (each written failing first, engine `tests/`)

1. A Depot built beside a Mining Node gets an allow-only `core_fragment` filter.
2. A Mining Node built beside an existing unfiltered Depot: the same result.
3. A Depot with a manual filter only gains the producer's item; its other
   entries are unchanged.
4. A Depot between two producers allows both outputs.
5. A worked Mining Node's output moves into the Depot; an unstaffed one's does not.
6. A full Depot stops pulling; the producer's buffer is left intact.
7. An item the Depot's filter refuses (player edited it after the auto-filter) is not pulled.
8. Haulers do not carry off a node's output when an accepting Depot is beside it.
9. Mining Node → Depot → Lathe: fragments reach the Lathe's input with no carrier.
10. The map join is drawn between a Depot and a producer whose output it accepts, and not when it doesn't.

Use a `dev-saves/` template where one has a base; mutation-check the intake
gate and the filter test. `balance_sim` must not move. Full gate:
`cargo test --workspace`, `cargo clippy --workspace --all-targets`.

## Docs

- `assets/structures/depot.ron` (and the mk2–mk6 descriptions if they repeat
  the text): say it fills itself from a machine beside it.
- `assets/help/62-production-lines.md` and `60-your-base.md`: the Depot as a
  line buffer.
- `.claude/rules/seams-base.md`, the `seams` skill and the graph: a new seam,
  "a Depot pulls only what it accepts, only from a worked producer, and the
  auto-filter fires once at build". Use the three-write order the skill documents.
- `CHANGELOG.md` at release.

## Out of scope

- Re-deriving the filter when a neighbour is demolished.
- Depots pushing into consumers (consumers already pull).
- Depot-to-Depot chains: a Depot has no `output_item`, so it is never a producer.
