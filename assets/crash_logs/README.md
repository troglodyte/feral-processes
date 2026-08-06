# Crash logs (mods)

Drop a `.ron` file in this directory and it's picked up automatically the
next time a game session starts — no recompiling required. A malformed file
is skipped with a warning logged in-game rather than crashing startup.

## What a crash log is

Pure flavour, and the only content directory that is. Nothing here has
stats, costs or prerequisites: a crash log is a line the rotten substrate of
a Stack frame gives up when the party stops and reads it.

`CellKind::Fault` and `CellKind::Corruption` are the two cells that carry
one. Every other cell of a frame reads as something else entirely.

## Schema

```ron
(
    id: "unique_snake_case_id",   // must be unique across all crash log files
    lines: [
        "One line of log.",
        "Another. A file may hold as many as you like.",
    ],
)
```

`lines` is optional and defaults to empty, so a file that declares only an
id parses and simply contributes nothing.

## How a line gets picked

Every loaded file's `lines` are flattened into one pool, **ordered by `id`**
— not by filename, and not by whatever order the filesystem hands the
directory over in.

Which line a given patch of rot reads is then a fixed function of where that
patch is: the zone, the depth of the frame, and the cell's coordinates. It
is never a random draw.

Two consequences worth knowing before you add files:

- **The same patch of rot always says the same thing**, across a save and
  reload and across sessions. That is deliberate — a place has a history,
  and a history that changed when you reloaded would not be one.
- **Adding or removing a file re-shuffles every existing patch**, because
  the pool it indexes into got longer or shorter and the ids re-sorted.
  Nothing breaks; the world just says different things in the same places.

An empty directory is legal. With no lines loaded at all, rotten ground
falls back to what ordinary ground says.
