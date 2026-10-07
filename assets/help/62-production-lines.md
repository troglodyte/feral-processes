# Production lines

Machines that feed each other form a line. A machine feeds its neighbour when it stands on one of
the neighbour's four orthogonal tiles and its output is something the neighbour takes in, either
as an ingredient or as the power it burns. A Mining Node beside a Lathe is a line. Two Mining Nodes
side by side are not, because neither makes anything the other needs.

A line is one job, and one worker staffs it. Setting a standing job on any machine in a line sets
it on every machine in that line, and the base posts a single program to run the line rather than
one to each machine. The base roster shows the line as one row, naming the machine its worker is
running now.

The worker pulls from the end. It runs the machine nearest the finished product that can make
progress, and drops back to an earlier machine only when the later ones are short of input. While a
batch is still wanted, it finishes that batch before moving to another machine.

- An ingredient that no machine in the line makes comes from a Depot. The worker fetches it, so
keep the Depot stocked and in reach.
- If the line is stuck on such an ingredient, its row says which machine needs what.
- Teardown Rigs always stand alone. A rig is never part of a line, even when it touches one.

One worker per line is slower than one worker per machine. The trade is that the rest of your crew
stays free for other work. Splitting a line in two by leaving a gap between the machines gives each
half its own worker.

A Mod Bench makes no goods, so it works apart from the lines above. Research Mod Bench, which
needs the weapon and armor benches first, then build one for 18 Core Fragments. One standing
anywhere in the base is enough, and you do not have to be beside it. Open a weapon or armor in your
pack and press M to modify its affixes. A piece has one affix slot, plus one for each time it has
been fused. Enter on an empty slot lists the affixes you have researched for that kind of gear, and
fitting one is paid in materials from your pack. R strips the highlighted affix for free, and the
affix is destroyed, whether you found it or fitted it. The Mod Bench research also opens the
Affixes research tree, whose nodes stay hidden until a study attempt finds them. Some affixes can
only be had this way and never drop.

See [your base](your-base) for building and staffing.
