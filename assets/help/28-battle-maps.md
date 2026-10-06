# Battle maps

A fight on the surface can open on a map instead of the usual intrusion
screen. Turn it on with Tactical surface battles in Options; it is off until
you do. Fights underground and fights with a nest guardian keep the
[intrusion](intrusions) screen either way.

The board is drawn where the world map was, and a strip inside it names the
round and every body in the order they act, fastest first. An arrow bounces
over the head of whoever is acting — blue for one of yours, red for one of
theirs. Everyone on your side is yours to command — a companion's turn is
your turn, not something you watch, unless you hand the side over with A
below.

The ground under the acting body is washed to show what its turn can do, and
a key in the bottom-left corner of the board names whichever washes are
showing. A cell wears one of the first three at most — danger over cover
over move — and the blue outline still runs round everything it can reach:

- Blue, move — every cell it can still step to.
- Green, cover — a cell it can reach with something between it and the
  other side, so attempts on it there fail more often.
- Red, danger — a cell it can reach, but a hostile beside the way gets a free
  attempt at it on the way past.
- Yellow, aim — while a blast is being aimed, the cells its centre may land
  on; while running Teleport, the bodies that can be picked up and
  then the cells they can be sent to. It replaces the three above until the
  cursor closes.
- White, hits — while a routine is being aimed, the cells it will actually
  hit.

A small green sunrise in a body's top-right corner means it is standing in
cover against whoever is acting. A purple path ending in a purple outline is
a hostile's next move and target, shown while Inference Probe Single is
reading it.

The bodies themselves carry marks too:

- A body's own colour, on the other side, is how hard it is against you —
  green much weaker, yellow an even match, orange tougher, red far
  stronger. When a picture or a special colour already fills the body, a
  small triangle in its top-left corner carries that colour instead.
- A coloured bar along the top edge is its rarity — silver, gold, platinum
  or prismatic. An ordinary body has none.
- The bar along the bottom edge is its integrity — green for yours, red for
  theirs. Short words just above it are what is affecting it, and how many
  rounds that has left.
- A body drawn faint is cloaked or is a decoy. A dark red body has been
  brought back by Respawn. A grey x is where something fell.
- When a blow lands, the cell flashes red and a red number rises with what
  was lost; a green number is integrity restored, and a yellow ! is a free
  attempt being taken. A bolt is drawn in the colour of whatever fired it.

On your turn:

- Arrows or the numpad — step one cell, diagonals included. The bar counts
  down what you have left; a slow body gets fewer steps than a quick one.
- a — attack something next to you. The cursor opens on your own cell;
  move it onto the body you mean and press Enter.
- d — defend. The acting body takes less from every attempt for the rest of
  the round, so it is worth most to whoever acts early and nothing at all
  to whoever acts last — the turn strip tells you which you are.
- s — run one of the acting body's routines, the same list the intrusion
  screen calls specials. Pick one and the cursor opens again, showing the
  cells it will cover.
- E — end the turn without spending it.
- A — hand the whole side over and watch. Everyone of yours closes and
  attacks on their own, at the pace the wild side moves at, and nobody runs a
  routine — basic attempts only. Any key at all takes control back, and it is
  off again as soon as the fight ends, so a fight you want to watch is one you
  ask for each time.

Move first and act second. The action ends the turn, so once you have
attacked or run something the only key left is the one that hands it on.

While the cursor is open, the arrows and numpad move it, Enter commits and
Esc backs out without spending anything. Nothing checks whose side a cell is
on — a blast centred badly lands on your own, and an attempt aimed at a
companion is a legal one.

Walking off the edge of the board is how you leave a fight you do not want,
and you can walk a companion out the same way — nobody is dragged back. The
wild side never does it: a hostile stays on the board until it is beaten.

Taking a program still means wearing it down first and running Decompile on
it, which lives under s here like every other [routine](routines).
