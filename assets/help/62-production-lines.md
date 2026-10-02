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

See [your base](your-base) for building and staffing.
