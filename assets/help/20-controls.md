# Controls

Every menu is a list you move through with Up/Down and pick with Enter (the battle screen also labels its rows with a number or letter you can type). Esc
backs out one level, and finishing a job returns you to the map. A menu lists only what you can
actually do from where you stand, so a row you cannot see leads to an empty screen rather than a
refusal.

Moving and living:

- hjkl or the arrow keys — move
- . — wait a turn. Not bound inside your base: time out of phase is spent by walking it, and the
  key does nothing there rather than refusing.
- e — drain a Power Cell
- r — recharge: restores Integrity and Power for you and the programs at your side. A program that
  is not with you is not repaired by it — base staff mend at a Repair Bay, and a squad away on a
  sortie lives on the provisions you paid for at dispatch. Free inside your base; anywhere else it
  burns a Power Outlet. It takes no time either way. Outside your base it can also be interrupted —
  something finds you mid-recharge, the Outlet is spent and nothing is restored. Inside your base it
  never is.

Your menus and stores:

- b — the base report, which is also where the base's commands live. A rail of pages runs down
  the left: Overview, then four command pages, then the report pages below. Build holds deploy,
  work a structure yourself, upgrade and demolish. Research holds research, routine and affix
  research, study a program and hold a program. Trade holds contracts, caravan and dispatch.
  Compile holds compile an item and recipes. Pick a row with Up/Down and Enter.
  The rest of the rail is Staff, Production, Stock, Work orders, Staffing, Build orders and
  Alerts, each a page of the report beside it. One highlight moves through both: Up and Down
  walk the rail, Right (or Enter) steps onto the page's rows, and Right again walks on through
  a page's other screens. Left steps back a screen and then to the rail, Esc goes straight to
  the rail, and [ and ] turn pages from anywhere. Esc on the rail leaves for wherever you opened
  it from. b reopens the page you left. The Overview shows where the base is short, and Enter
  opens the highlighted page.
- ? — this manual
- i — your inventory and your party, a rail of pages like the base report's: Items (your pack),
  Phase keys, Downed programs and Tools, then Companions (your roster), Programs (manifests,
  fuse, refactor, develop), Routines (install, etch, extract) and Perks. The rail and page keys
  work as they do under b, and i reopens the page you left.
- D — the Downed programs page of your inventory: the downed programs you are holding, and the
  tools that take them apart. The same key opens it from Items. See
  [taking a program apart](extraction).

Acting on the world:

- c — open the structure beside you. It has one screen with a page for everything you can do with
  that structure: Inventory, Filter (a Depot), Recipe (a machine), Tool (a Teardown Rig), Siphon,
  Splice, Grow (a farm plot), Rebuild (a wreck) and Info. The rail and page keys work as in the
  base report: Up/Down walk the pages, Right steps onto a page, Left or Esc steps back to the rail,
  and Esc on the rail leaves. With more than one structure beside you, Tab steps to the next one
  clockwise from north. Inventory is that one structure's own shelf, with a row per item and the
  columns you, container — + takes off the shelf toward you and - puts your own cargo onto it.
  [A] takes the lot; Shift+Left/Right goes to the end of a row and Ctrl+Left/Right halves the gap;
  typing digits sets an amount; [N] clears the basket
- t — trade
- a — call a routine
- x — examine a direction
- d — demolish a direction
- v — lay a VectorStasis Tile on the cell you are standing on
- n — cutting tools out, or away again: with them out, walking into rock cuts it yourself, and your
  own tile wears a blue ring for as long as they are. Costs no time either way.
- m — Excavation plan: space anchors a box, space again marks it, Esc backs out. Marked rock is cut
  and, with the default brush, floored; F opens the brush list to pick plain, a floor colour or
  strip. P switches to marking a farm plot instead. Drawing a plan costs no time.
- z — stockpile zone: the same box as the Excavation plan, but it sets floor aside as a place for
  loose things. Starting a box on a cell that is already zoned clears the box instead. Drawing a zone costs
  no time.
- o — pick up the thing lying on the cell you are standing on and keep it with you
- y — put something down: a list of what you carry, and the chosen thing is set on the floor
- < — phase up into the base, standing on the anchor
- > — phase back down to the grid, at the exit

Reading and housekeeping:

- 1-4 — the side column: what your base is holding, your crew, your pack, and the contracts you
  are signed to. A closed tab still summarises itself along the bottom, and wears a mark when it
  needs you.
- L — history
- N — the base report on its Alerts page, highlight already on the rows: production blockers and
  program events, capped and dismissed with x or d. Opening it marks everything read.
- u — the compass: home, the settlements and the Stack entrances you know of, each with its
  heading and distance. Enter points the compass at the highlighted place, and the zone map keeps
  that heading in view; X clears it. It reads the zone surface only, and costs no time.
- g — on the zone surface, the world map: every region you have walked, with the towns, outposts,
  Stack links, nests and routes you know of. The arrows pan it; Tab switches them to the place list, where
  Up and Down pick a place, and back again. A click on the map or the list picks a place too
  (click a shared spot again for the next place on it). C points the
  compass at the picked place (a nest is not a destination), P puts the view back on you, and Esc
  closes it. A town's tint shows how it regards you, and its arrow shows
  whether its trade is rising, holding or falling. It costs no time. In the Stack, g is that
  frame's map instead.
- f — filter the log: all, field, or base
- tab — double the log pane's height, and back
- space — pause the world, and resume. You can still act while paused; each action takes its
  own turn
- , — cycle the world's speed: x1, x2, x4
- s — save
- Esc — Game menu: gameplay, audio and graphics settings, and Save & quit
- +/- — zoom

On the companions screen, < and > move the highlighted member along the battle line; the front slot
draws the most fire. A routine slot takes an etched disk, never a routine you merely know — on the
install page, e burns a blank with something you know to make one. When trading, S sells one and B
buys one from the highlighted row, with no quantity page in between; S works in your pack too.

I inspects whatever piece of gear is under the highlight, from any list that names one — your pack,
the swap picker, a program's slots, a trader's shelves, a Stack stall's cargo rows. It reads out the
full stat block at the level the piece would go on at, your accuracy with it in its slot, and, if the
piece grants a routine, what fires that routine, what it hits and for how much. See
[supplies](supplies) for what the numbers on a copy mean.

In the Stack:

- hjkl or the arrow keys — forward, turn left, turn right, turn around
- > — descend
- < — climb, or leave the link. The same pair the anchor uses, and the same sense: < goes up, >
  goes down. The base sits above the grid and the Stack below it, so the anchor's two keys are
  the mirror of these.
- o — adopt an orphaned process, which costs a catalyst
- t — trade, if somebody is selling on this cell. They keep no buyback and the shelf does not
  refill.
- g — the map, showing only what you have seen
- +/- — zoom the corner map; the whole frame closes in

In an intrusion:

- a — attack
- d — defend
- s — special
- r — drop your current emulation, if you have run one. See [emulation](emulation).
- u — use an item
- j — jack out
- A — all attack, and D all defend: shift commands the whole party
- R — resolve: play the rest of the fight out now
- Up/Down — scroll the narration, on the results page too

On a [battle map](battle-maps), which surface fights use if you turn them on
in the Game menu (Esc), the keys are different: arrows or the numpad step the acting
body, a attacks, d defends, s runs a routine, U uses an item, V drops an active emulation,
E ends the turn, A hands your side over to fight on its own, and R resolves the fight.

This screen is the manual now, not a card: Esc backs out of it a page at a time, like every other
screen. [Start here](start-here) if you have not read anything else, or read what the keys are for:
[intrusions](intrusions), [your base](your-base), and [the Stack](the-stack).
