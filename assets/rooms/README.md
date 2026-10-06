# Rooms (mods)

Drop a `.ron` file in this directory and it's picked up the next time a game
session starts. A malformed file is skipped with a warning; an absent
directory is an empty catalogue, and no room then has a role.

A room is a 4-connected set of walkable base cells bounded by rock, a
`barrier` structure's anchor (the Wall) or a `door` structure's anchor (the
Door). It is detected from the grid every time it is needed and never saved.
The region holding the Home is the commons and never has a role. A room
whose structures match no file here is roleless. Only a structure's anchor
cell counts toward what the room holds, via its `room_tags`
(see `assets/structures/README.md`).

## Schema

```ron
(
    id: "unique_snake_case_id",   // unique across all room files
    name: "display name",         // lower case; read as "Fine <name>"
    priority: 30,                 // highest matching priority wins; a tie goes
                                  // to the lower id. Shipped: quarters and
                                  // dormitory 30, rec_room 20, workshop 10
    requires: [                   // every entry must be met; empty matches nothing
        (tag: "bed", min: 1, max: Some(1)),   // max is optional (None = no cap)
    ],
    living: true,                 // optional, default false: roommate thoughts apply
    tint: (90, 140, 220),         // optional, default black: Alt-overlay RGB
)
```

Shipped roles: `quarters` (exactly one `bed`), `dormitory` (two or more),
`rec_room` (a `recreation`), `workshop` (a `workshop`).

## Quality

Each room is graded Cramped, Plain, Fine or Superb from its area, the share
of its cells carrying a finish with a `comfort`, and how crowded it is with
structures. Weights and band edges are the `ROOM_*` constants in
`tuning.rs`.
