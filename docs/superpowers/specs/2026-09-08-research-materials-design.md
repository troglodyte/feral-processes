# Research costs materials

**Status:** designed 2026-09-08.

Research is currently a purchase: `ResearchDef::cost` in Research Data, taken
from the player's `Inventory` by `Game::unlock_research`, and nothing else.
A Research Node posted with one program pays for the whole tree given
enough wall-clock. That makes research a timer rather than a reason to run a
base.

This adds a second, authored cost in goods — heavy enough that a node is a
production run you plan for.

## Decisions

1. **A node's material bill is authored in its `.ron`**, not derived from its
   Research Data cost. A derived bill makes every node ask for the same
   goods, which reads as one repeated tax and denies a mod any say. Authored,
   a bill can be denominated in what the node's *own branch* produces —
   plating research wants plate.

2. **Paid from the pack first, then the adjacent shelves.** Research stays a
   player cost (`CLAUDE.md`: *a cost the player incurs is paid from their
   pack*), and the shelf half is reach the player already has — the same
   `adjacent_stock()` orthogonal neighbours `transfer_offer` reads, so the
   Transfer screen and the research screen cannot disagree about what "the
   shelves" means. Off the base `adjacent_stock` is empty and the pack half
   stands alone, so the Research row stays `Locality::Anywhere` and needs no
   new gate.

3. **Every refusal lands before anything is spent**, and is asserted **per
   refusal** — `Game::extract_program`'s rule. The shortfall is computed
   against `pack + shelves` and refused whole; only then does
   `take_from_adjacent` move a unit.

4. **A node may only name materials its own prerequisites can produce.**
   This is the whole of "don't require items that can't be unlocked first",
   and it is a census over the real assets rather than a convention: for each
   node, walk its transitive `requires` closure, collect the structures those
   nodes unlock plus the structures no research gates at all, and take the
   fixpoint of what that set can produce and craft. Every material must be in
   it.

5. **Research Data costs are unchanged.** Materials are the new brake;
   moving both at once makes the retune unreadable.

6. **`weapon_bench` requires `routine_fabrication`.** The census found the
   Fabricator is a dead end without it: it assembles Trace Sniffers out of
   Logic Wafers, which need the Transcriber and Raw Trace, both behind
   `routine_fabrication`, which `weapon_bench` did not require. The player
   pays 26 Research Data they always had to pay, and the weapon branch's
   bills can be denominated in wafers and sniffers.

## Schema

`ResearchDef` gains one field:

```rust
/// Goods consumed alongside `cost` when this node is researched.
#[serde(default)]
pub materials: Vec<(ItemId, u32)>,
```

Additive behind `#[serde(default)]`, so no `SAVE_FORMAT_VERSION` bump and no
existing `.ron` — including a mod's — needs touching. `Research` is a set of
researched ids; nothing about what a node cost is stored.

## The spend path

`Game::unlock_research`, after the existing guards and the Research Data
check, before anything is spent:

1. For each `(item, need)`, `have = Inventory::count(item) +
   adjacent_stock() output`. Short → `Err`, naming the item, what is needed
   and what is held. Nothing has moved.
2. `take_from_adjacent(shortfall)` pulls the difference onto the pack.
3. `Inventory::take` each material, then the Research Data as today.

`take_from_adjacent` is guard-free, log-free and tick-free by construction;
this caller already owns the announcement.

## The view

`views::ResearchStatus` gains `materials: Vec<ResearchMaterial>` —
`{ name: String, need: u32, have: u32 }`, `have` counting pack plus shelves,
built in `Game::research_nodes`. `affordable` folds the materials in, so the
existing row colour and affordability rules cover the new cost with no gui
rule of their own.

The name comes from `Game::item_name`, in the engine, for `Game::copy_name`'s
reason: a renderer building the name is how a bill and a refusal come to
disagree about what you are short of.

## The screen

`render/progression.rs::research_menu_rows` draws one extra line per node,
under the description, listing `Name have/need` per material. The screen is a
`draw_popup` with no scroll, so the row count and width are layout
constraints — `the_research_page_fits_its_popup`'s job.

## Content

Bills ramp raw → intermediate → branch terminal. Raw equivalents in the last
column; a Refinery is 18 Core Fragments and a zone-1 Zone Portal is ~140 plus
24 Portal Fragments.

| Node | RD | Materials | ~raw |
|---|---|---|---|
| automation | 8 | Core Fragment 30 | 30cf |
| power_grid | 10 | Core Fragment 30 | 30cf |
| teardown | 12 | Bytecode Block 8 | 32cf |
| commerce | 14 | Core Fragment 40 | 40cf |
| self_exec | 14 | Routine Disk 4, Logic Wafer 4 | 32cf+16rt |
| fortification | 18 | Bytecode Block 8, Power Cell 8 | 48cf |
| field_ops | 20 | Routine Disk 6, Logic Wafer 6 | 48cf+24rt |
| symbolic_links | 22 | Routine Disk 6, Logic Wafer 6 | 48cf+24rt |
| armor_bench | 24 | Bytecode Block 12, Charge Coil 4 | 72cf |
| weapon_bench | 24 | Logic Wafer 6, Bytecode Block 8 | 32cf+24rt |
| routine_fabrication | 26 | Bytecode Block 12, Charge Coil 4 | 72cf |
| cache_coherence | 40 | Bytecode Block 20, Charge Coil 8 | 128cf |
| dispatch | 45 | Bytecode Block 18, Charge Coil 10 | 132cf |
| firewall | 45 | Hardened Shell 4, Bytecode Block 14 | 104cf |
| overclock | 45 | Trace Sniffer 2, Logic Wafer 8 | 72rt |
| neural_amp | 55 | Trace Sniffer 3, Logic Wafer 8 | 92rt |
| runtime_patching | 60 | Routine Disk 12, Logic Wafer 10 | 96cf+40rt |
| adaptive_plating | 70 | Routine Disk 12, Logic Wafer 12 | 96cf+48rt |
| program_refactoring | 75 | Bytecode Block 24, Charge Coil 12 | 168cf |
| ablative | 110 | Hardened Shell 8, Bytecode Block 24 | 192cf |
| monofilament | 110 | Trace Sniffer 5, Logic Wafer 12 | 148rt |
| mesh_plating | 120 | Routine Disk 16, Logic Wafer 16 | 128cf+64rt |
| cortex | 125 | Trace Sniffer 6, Logic Wafer 12 | 168rt |
| deep_analysis | 130 | Routine Disk 16, Logic Wafer 16 | 128cf+64rt |
| kernel_privileges | 135 | Routine Disk 16, Logic Wafer 16, Bytecode Block 16 | +64cf |
| address_translation | 140 | Routine Disk 18, Logic Wafer 18, Bytecode Block 16 | +64cf |

None of these cross a breach — only Research Data and Credits do — so each
sector's base has to be genuinely running again rather than stockpiled ahead.

## Tests

- `unlock_research` spends materials from the pack.
- `unlock_research` pulls the shortfall off an adjacent shelf.
- One test per refusal, each asserting `Inventory` and every `Stock` are
  unchanged: short on materials, and short on materials while holding the
  Research Data.
- A node with no `materials` behaves exactly as before.
- Census: every material of every node is producible through that node's own
  prerequisite closure.
- Census: the shipped tree actually declares materials — a bill deleted by
  hand must fail the build, not read as "this node is free".
- The research page fits its popup.

## Docs

`assets/research/README.md` (the field), `assets/help/60-your-base.md` and
`assets/help/50-routines.md` (both say the tree runs on Research Data alone),
`CHANGELOG.md`.
