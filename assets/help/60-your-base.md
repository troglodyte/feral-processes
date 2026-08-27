# Your base

Your base is not on the map. It is its own space, entered through a permanent gray anchor that
stands wherever your run began: walk onto that tile and press < to phase up into it, and walk back
to the door cell inside and press > to come back down. The anchor cannot be destroyed or moved,
and it travels with you across a breach, so the base you built in zone 1 is the base you walk into
in zone 6, exactly as you left it.

Until you deploy a Home the anchor is dark and there is nothing on the other side. Deploying one is
the single build allowed out on the open grid, and it clears a small pocket of floor around the
door. From then on every machine, bench and trader you build stands in there rather than out on the
ground you are standing on. Demolishing the Home cascades through everything on the floor, so it is
not a thing to do idly.

Everything past that first pocket is solid, and you cut it out yourself.

- Walk into a wall and you swing at it, the same way you wear a nest down. Swings are deterministic,
  so a wall is never a gamble: it takes about three hits early in a run and one hit late. Rock is
  the same thickness in every zone at every depth — what changes is you. A cell that opens sometimes
  shakes a Core Fragment loose.
- A cut cell is not floor yet. v lays a VectorStasis Tile on the cell you are standing on for one
  Blank Substrate, and only laid tile is permanent and buildable. Bare cut ground is the frontier,
  and the frontier does not keep — leave it long enough and it goes back to solid at full thickness.
  Laid tile is never reclaimed, and neither is a cell somebody is standing on.
- m opens the Excavation plan. The cursor costs no time at all: space drops one corner, moving
  previews a rectangle, and space again commits it. Starting the box on a cell that is already
  marked clears instead of marking, which is why there is no separate erase. A marked wall is cut
  and then floored in one go, and the mark outlives the cut.

You do not have to be there for any of it. Programs on your roster dig while you are off in a
sector — but digging is the lowest priority the base has, below work orders and standing jobs, so a
spare body digs and a needed one does not. Marking a corridor can never stall production.

Building works the same way, and the Home is the one exception. Every other structure you pick out
of the build menu is a *request*: it marks the cell with a dark slab and an orange caret, and one of
your programs comes and raises it. Nothing is charged when you file it, so you may ask for a machine
the base cannot afford yet and let production catch up — the builder will say once that it has
nothing to fetch, and start the moment the last part exists. It gathers from anywhere the base keeps
things: a Depot, a machine's own output shelf, or straight out of your pack if you are standing in
there with it. It carries five at a time, so a big machine takes several trips, and the parts pile up
on the cell as they arrive — press x at it to see what is still to come and how far along it is. A
structure takes two ticks to raise for every part it costs.

Building is the *highest* priority the base has, above work orders and standing jobs — the mirror of
digging. A spare body takes the job if you have one; if you do not, somebody comes off a machine
until the thing is up. If your whole roster is out fighting beside you, nothing gets built until
somebody is free. Changing your mind costs nothing: d and a direction at a pending site calls it off
and puts every part already carried there back on a shelf.

Machines run on the Grid, and the Grid has to cover them. Every tick the base sums what it supplies
against what its machines draw, and anything over the line is cut in a fixed order until the rest
fit. A cut machine reads dark: no progress, no pulling from its neighbours, and nothing to be had by
working it by hand either. Home supplies four and so does each Recharger Node, there is no limit on
how many Rechargers you build, and a Line Driver is pure supply with nothing else attached. A
machine draws whether or not anybody is posted to it, which makes an idle machine kept for later a
real expense. This is the Grid and it is not your Power — the two are separate resources that happen
to share a word.

The base staffs itself. Any program you own that is not out fighting with you is staff, and the
scheduler decides the whole assignment every tick by priority: work orders first, then standing
jobs, then digging. A work order is an item and a quantity and nothing else — say what you want and
the base works out which machines make it, who stands on each, and what has to be fetched. Cancelling
one unwinds nothing.

The rest of what a base does:

- Extractors produce on a timer. Assemblers pull their input out of whatever is touching them, so a
  chain is a line of machines laid next to each other. A Depot gives a machine with nothing
  downstream somewhere to empty into, and gives a worker somewhere to fetch from.
- c opens a window listing everything the machines touching you are holding, pooled into one row per
  item. Set an amount on the rows you want — type it, or nudge it with the arrow keys, where Left
  adds and Right removes. Hold Shift to jump to one end of that row — all of it, or none. Hold Ctrl
  to close half the distance to whichever end you are heading for, so pressing it again takes half of
  what is left. Press
  Enter to take exactly what you have set; [A] fills every row to its maximum, so taking the lot is
  still two keys. What you leave stays where the base's own chains
  can pull it, and leaving with Esc costs nothing.
- A structure upgrades to whichever is lower, its own maximum tier or the zone you are in — so a
  structure at its ceiling in zone 1 stays listed and starts moving again after a breach. An upgrade
  is a request like any other build: nothing is charged when you ask for it, your crew fetches the
  parts out of the base and works on the spot, and the machine keeps running the whole time. Call one
  off from the build orders screen and you get back whatever has been carried there.
- A GC Entropy Sweep chews on a random structure now and then, and what it takes off is permanent
  unless something repairs it. A Shield soaks damage off every sweep against everything you own, a
  Patch Node recompiles damage across the whole base, and a program posted to the structure that gets
  hit defends it with its own Mitigation.
- A Research Node is the only source of Research Data, and Research Data crosses a breach. The tree
  is what unlocks the benches, the recipes and several routines.
- A Contract Broker posts work the sector is paying for. Read its board from anywhere; sign and
  deliver at the base.

Read on: [supplies](supplies), [your companions](companions), and [getting stronger](getting-stronger).
