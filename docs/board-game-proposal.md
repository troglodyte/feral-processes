# Feral Processes: Board Game Proposal

Nov 9, 2026 · Michael Harris

Copy of the shared doc at
<https://claude.ai/artifact/LXw3Y8QjAYxgQtCo6UrX3Q>, exported at revision 16.
The live doc is the one being edited.

Feral Processes can become a solo or co-op board game: explore Grid tiles, fight and recruit programs, dive a Stack deck while a Trace track climbs, and build a base where machines that touch feed each other. Most of the content already exists. The real work is cutting the simulation down to what people can track by hand.

## Rules sketch

Solo or co-op, players race the Trace track to collect every artifact piece from the Stack. Pieces are shared. The player who places the last one decrypts the exit URL, logs out of the Grid, and saves the other players still stuck in the virtual world. Two pressures push back: Trace punishes greed in the Stack, and garbage collection punishes anything left idle at the base. All numbers below are starting points for playtesting.

### Currency

- **Core fragments** are the common currency. They pay for decompiling ordinary entities and building basic structures.
- **Power cores** are rare. They pay for advanced structures and strong entities.

### Components

- **Entity deck.** Wild programs. Four lie face up in the Grid market. Each card shows its decompile cost, its work skill (Extract, Farm or Force) and its dive value from 1 to 4.
- **Structure market.** Base tiles with a cost, worker slots, and what each worked slot produces.
- **Base board.** A 4×4 grid per player. Matching structures that touch produce +1 extra each.
- **Stack deck** and **artifact pieces.** Five pieces, A to E, shared by the team.

### A round

1. **Income.** Every structure slot with an entity working it pays out. An empty slot pays nothing. Then draw a garbage-collection card (below).
2. **Actions.** Take two actions, in any order, repeats allowed:
   - **Decompile.** Pay an entity's cost and take it from the market. Place it on a structure slot or keep it in reserve.
   - **Build.** Pay a structure's cost and place it on your base board.
   - **Reassign.** Move any of your entities between slots and reserve.
   - **Dive.** Send one to three reserve entities into the Stack (below).
3. **Refill.** Refill the entity and structure markets.

The loop: entities work structures, structures pay income, income buys more entities and structures. An entity sent diving earns nothing at the base that round.

### Diving the Stack

1. Draw a Stack card. It shows a target number, usually 4 to 6, and which artifact piece it guards.
2. Roll one die per entity sent and add each entity's dive value to its own die.
3. **Any result at or above the target:** take that artifact piece. A piece the team already holds pays 2 power cores instead.
4. **Any natural 1:** that entity is consumed. Discard it.

Sending more entities raises the odds of a piece and the odds of losing one. Strong divers are also your best workers, so every dive costs income.

### The Trace track

A shared track from 0 to 12. It is the clock the team races.

- **+1** for each entity sent on a dive.
- **+1** for each artifact piece taken.
- **At 4:** every Stack target rises by 1.
- **At 8:** a hunter is loose. On a dive, a natural 1 or 2 now consumes the entity.
- **At 12:** the Grid locks down and the team loses.

A Scrubber structure lowers Trace by 1 each income while an entity works it. It competes with income for workers.

### Garbage collection

The GC deck starts with 12 cards: 9 quiet, 3 Sweep. Each round during income, draw one.

- **Quiet:** nothing happens. Discard it.
- **Sweep:** every structure with an empty slot takes a damage token. A second token derezzes it. Each entity in reserve rolls a die; on 1 or 2, GC collects it.
- **Repair:** removing a damage token costs 1 core fragment as part of a Build action.
- **Escalation:** when Trace passes 4 and again at 8, shuffle 2 more Sweep cards into the GC deck.

The two pressures work against each other. Diving with many entities raises Trace, while holding entities back in reserve leaves them for GC. Neither turtling nor rushing is safe.

### End of the game

When the team holds all five pieces, the player who placed the last one decrypts the exit URL. That player exits the virtual world and saves the other players still stuck in it. Solo, you are the one who escapes. If Trace reaches 12 first, everyone stays trapped.

## Physical components

A medium box, about 280 × 280 × 70 mm, for 1 to 4 players. Square tiles on a gridded base board carry the adjacency rule. The assembled artifact gives the game its ending moment.

| Component | Count | Production version | Prototype stand-in |
| --- | --- | --- | --- |
| Entity cards | 48 (12 species × 2 tiers × 2) | Poker-size cards, 63 × 88 mm, with sprite art, cost, skill and dive value | Printed cards in sleeves over old playing cards |
| Entity tokens | 48 | Round 25 mm punchboard tokens with the same sprite, placed on structure slots | Paper discs or numbered cubes |
| Structure tiles | 36 (12 types × 3) | 50 mm square punchboard tiles. Edge pips show which neighbours they feed | Card-stock squares |
| Base boards | 4 | Dual-layer boards with a 4 × 4 grid of recessed wells so tiles don't shift | Printed sheet |
| Main board | 1 | Grid market (4 entity slots), structure market, and the Trace track 0–12 with gates at 4, 8 and 12 | Printed sheet |
| Trace marker | 1 | Translucent magenta acrylic marker | Any coin |
| Stack deck | 36 | Poker-size room cards: target number and the piece guarded | Index cards |
| GC deck | 16 (9 quiet, up to 7 Sweep) | Mini cards, 44 × 68 mm. Sweeps kept apart until escalation | Index cards |
| Artifact pieces | 5 | Interlocking acrylic or punchboard shards that join into a plate showing the exit URL | Paper puzzle pieces |
| Core fragments | 60 | Small cyan translucent plastic crystals | Glass beads |
| Power cores | 25 | Larger amber acrylic cylinders | Dice or coloured cubes |
| Damage tokens | 24 | Small red punchboard corruption tokens | Red cubes |
| Dice | 6 | Custom d6. The 1 face shows a segfault glyph | Standard d6 |
| Player aids | 4 | Card with the round order and dive rules | Printed card |

The sprites in `assets/sprites` are pixel art, so they print cleanly at token size. Custom dice and acrylic add most of the cost; the stand-ins work for every playtest.

## What carries over

Most systems already have a tabletop form. Machine adjacency is the strongest fit; per-program bookkeeping is the weakest.

| In the video game | On the table | Fit |
| --- | --- | --- |
| Machines feed by touching | Tile-laying engine builder on a base board; adjacency is the whole rule | Strongest |
| Front groups reach you | Front-row and back-row creature cards; ranged moves reach the back row | Native |
| The Stack and Trace | A dungeon deck of room cards; loot moves a Trace marker; gates on the track release threats; seeded boss at the bottom | Native |
| Exploring the Grid | Tiles drawn from a bag; deeper zones are a separate tier of tiles and monsters | Good |
| Decompiling a hostile | Beat a monster card, then roll to recruit it, with a bonus for damage dealt | Good |
| Routines in level slots | Ability cards tucked under a creature; each level opens a slot | Good |
| Research tree | A tech board, or unlock cards bought in order | Good |
| Entropy Sweeps, caravans | A garbage-collection deck that damages unworked structures and collects idle entities | Good |
| Permadeath or Forgiving | One-shot mode, plus a campaign with sealed unlocks between sessions | Good |
| Base runs while you're away | One upkeep phase each round: pay needs, collect output, resolve events | Needs redesign |
| Needs, XP, gear, fusion | Two or three tracked values per creature, on a dial or sleeve tokens | Needs redesign |

Example card: Botnet (132 HP, 12 attack, speed 9) becomes HP 4, ATK 3, SPD 2, with Flood as "ranged, hits 2, 4+: Bleed". Example Trace track of 10: a patrol joins the room deck at 3, a hunter enters at 6, the frame locks at 9, and at 10 the party is ejected and loses a program. Both are illustrative, not balanced.

## What has to go

Bookkeeping is the main cut.

- **Tracking each program.** Needs, XP, levels, gear, fusion and morale for every creature works on a computer and stalls a table. Keep two or three values.
- **Background ticking.** Tick-by-tick production becomes one upkeep phase. Needs decay becomes "lose a token unless serviced."
- **Large numbers.** Stats drop to small integers, about one to six. Deeper zones use tier cards instead of doubling every stat. Dice or a card deck replaces the seeded RNG.
- **Hidden simulation.** Anything computed out of sight, such as aggro weights or affinity multipliers, becomes a visible rule or goes away.

## Scope

A first box holds about a quarter of the content, a typical deck-and-tiles game for 60 to 120 minutes. The rest can go in expansions.

| Content | Asset files today | First box | Becomes |
| --- | --- | --- | --- |
| Species | 18 | 12 | Creature cards, two tiers each |
| Abilities | ~75 | 24 | Ability cards |
| Items | ~80 | 20 | Gear and resource cards |
| Structures | ~40 | 12 | Base tiles |
| Research | ~40 | 10 | Tech board spaces |

Counts are files under `assets/`, including variants.

## What the repo already gives

- **Card data.** Species, items, abilities and structures are `.ron` files. A short script exports them to CSV for a card-layout tool, or renders print-and-play PDFs directly. Days of work.
- **Flavour text.** `assets/memories`, `assets/descriptions`, `assets/thoughts` and `assets/story` hold card and rulebook text in the game's voice.
- **Art.** More than 100 sprites in `assets/sprites` work as prototype art we already own.
- **Balance testing.** The board rules can be a small Rust simulator, like `balance_sim` and the arena today. Thousands of simulated games can run before the first table playtest.

## Path

A playable paper prototype is a few weeks of part-time design; most of that is deciding what to leave out.

1. **Rules sketch.** One page covering the core loop: explore, fight and recruit, dive the Stack, upkeep the base. Most of the design time.
2. **Export script.** From `.ron` to card data, with stats rescaled to board numbers.
3. **Rules simulator.** Optional. Catches broken numbers before anyone sits down.
4. **Print-and-play prototype.** Playtest and revise; expect several rounds.
5. **Production.** Print-on-demand for small runs, or crowdfunding for a full print run. Art, rulebook editing and manufacturing are most of the cost.

## Open questions

- [x] **Player count:** decided. Solo and co-op, racing the Trace track.
- [ ] **Session length:** a 60-minute one-shot or a multi-session campaign. This decides how much base-building survives.
- [ ] **Box size:** a small card game, or a medium box with a board and tiles.
- [ ] **Production route:** print-and-play only, print-on-demand, or a crowdfunded run.
