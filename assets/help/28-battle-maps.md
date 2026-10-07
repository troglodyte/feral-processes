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
over move:

- Blue, move — every cell it can still step to.
- Green, cover — a cell it can reach with something between it and the
  other side, so attempts on it there fail more often.
- Red, danger — a cell it can reach, but a hostile beside the way gets a free
  attempt at it on the way past. See opportunity attacks below.
- Yellow, aim — while a blast is being aimed, the cells its centre may land
  on; while running Teleport, the bodies that can be picked up and
  then the cells they can be sent to. It replaces the three above until the
  cursor closes.
- White, hits — while a routine is being aimed, the cells it will actually
  hit.

A small green sunrise in a body's top-right corner means it is standing in
cover against the hostile that is acting. It shows only on a hostile's turn. A purple path ending in a purple outline is
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
  theirs. Short words just above it are what is affecting it; a number after
  one is how many times it has stacked. See status effects below.
- A body drawn faint is cloaked or is a decoy. One of yours drawn dark red
  has been brought back by Respawn. A grey x is where something fell.
- When a blow lands, the cell flashes red and a red -N rises with what was
  lost; a green +N is integrity restored, and a yellow ! is a free attempt
  being taken. Each number is what actually landed, after mitigation and caps.
- Attacks are drawn to suit the weapon or routine. Ranged weapons fire a
  train of pulses, melee weapons sweep a quick arc, heavy single hits draw a beam, and
  blast routines go off as an explosion that shakes the screen. A heal sends
  a green ball to its target, and a buff does the same in cyan. A bolt takes
  the colour of whatever fired it unless its look sets one.

On your turn:

- Arrows or the numpad — step one cell, diagonals included. The bar counts
  down what you have left; a slow body gets fewer steps than a quick one.
- a — attack a body your weapon reaches. The cursor opens on your own cell;
  move it onto the body you mean and press Enter. Walking into a hostile is
  the same attack.
- d — defend. The acting body takes less from every attempt for the rest of
  the round, so it is worth most to whoever acts early and nothing at all
  to whoever acts last — the turn strip tells you which you are.
- s — run one of the acting body's routines, the same list the intrusion
  screen calls specials. Pick one and the cursor opens again, showing the
  cells it will cover.
- U — use an item from your pack. It spends the action, so it ends the turn.
- V — drop an active emulation, if you are running one.
- E — end the turn without spending it.
- A — hand the whole side over and watch. Everyone of yours closes and
  attacks on their own, at the pace the wild side moves at, and runs routines
  as you would, spending Power on them. Any key at all takes control back, and
  it is off again as soon as the fight ends, so a fight you want to watch is
  one you ask for each time.
- R — play the rest of the fight out at once and go to the results. Your side
  fights the way A does. If the fight cannot be settled within 200 rounds, it
  hands control back with "Couldn't settle it — finish by hand."

Move first and act second. Once you have attacked or run something you
cannot move or act again this turn; E ends it, and A and R still work.

Opportunity attacks: stepping out of reach of a hostile costs you a free
attempt from it, and so does running a routine while one stands next to you.
Closing in on a body, or circling it while staying beside it, is free; only
disengaging is paid for. Every body gets one of these a round, and gets it back
when its own turn comes round, so a second hostile beside the path takes its
swing but a body that has already used its reaction does not. The free swing
cannot fumble, and a cloaked body provokes nobody. The red cells on
the movement wash are the steps that would provoke one. A routine cut
off this way is spent: its Power is gone and its cooldown starts, but nothing
lands. Decompile provokes nobody. The same rule binds the wild side, so a
program that walks away from you is swung at too. Walking off the edge of the
board provokes nothing.

Status effects: a body can carry several at once. A second effect no longer
replaces the first, so a fumble that leaves you Exposed and then Stunned keeps
both, and each counts down its own rounds. Exposed and Stunned have tags like
any other; three more are worth knowing. Data Poisoning, Thermal Throttle and
Write Lock are the routines that apply them.

- PSN, Poisoned — stacks up to five times, each dose adding one stack. Every
  stack deals its damage at the end of every round, so PSN×3 hurts three times
  as much as one.
- THR, Throttled — cuts the body's attack by a quarter.
- LCK, Locked — nothing heals it: repair routines, regeneration and healing
  items all restore nothing until it wears off.

Any other effect that is applied again keeps the longer of its remaining rounds.
On the map a tag reads PSN or PSN×3. The intrusion screen adds the rounds left
in brackets, as in PSN×3 (2), and lists every effect on a group or a party
member.

While the cursor is open, the arrows and numpad move it, Enter commits and
Esc backs out without spending anything. Nothing checks whose side a cell is
on — a blast centred badly lands on your own, and an attempt aimed at a
companion is a legal one.

Walking off the edge of the board is how you leave a fight you do not want,
and you can walk a companion out the same way — nobody is dragged back. The
wild side never does it: a hostile stays on the board until it is beaten.

Taking a program still means wearing it down first and running Decompile on
it, which lives under s here like every other [routine](routines).
