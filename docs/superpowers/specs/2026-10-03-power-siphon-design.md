# Power Siphon — design

**Date:** 2026-10-03 · **Weight:** spec-and-plan (engine + app-core + gui, new
save field, new schema field)

## Intent

A base structure, inspired by RimWorld's bio-reactor mod, that holds one of
the player's **living base-staff programs** and supplies the base grid for as
long as it is held. The price is the program itself: it is out of the
workforce, it builds a grudge that drags its Morale down the longer it stays,
and it comes out hurt. Once it is released, the existing memory and Morale
rules carry it back up over time. Nothing new governs recovery.

What the user decided:

- Grid **supply**, not Power Cells or the player's Power stat.
- **Only base staff** (`ProgramRole::Staff`). Downed wild programs
  (`DownedProgram` records) are dead or disabled, and cannot go in.
- A held program stays until the player releases it. Release costs a grudge
  plus **Integrity loss**.
- **Flat +4 supply**, tunable in data. It does not scale with level.
- **No build cap.** The limit is how many staff programs the player will give
  up.
- The grudge **builds to low-mood behaviour** over a long hold.
- Name: **Power Siphon**.

Out of scope: crew reactions to seeing a program held, downed wild programs,
level-scaled output.

## Why it does not break the economy

`power-is-not-a-limiting-resource` asks what a repeatable source spends that
cannot be earned by repeating it. Here the answer is a staff program's labour
and Morale. A siphon is grid **capacity**, not per-tick income, and each one
takes a program off a machine. That is what caps the count instead of
`max_deployed`.

## Components

### 1. Structure data — `assets/structures/power_siphon.ron`

```ron
(
    id: "power_siphon",
    name: "Power Siphon",
    description: "Pins a staff program into the grid and runs the base off it. Walk up to it and press <key> to load or release.",
    glyph: <tbd at plan, checked free>,
    color: Yellow,
    build_cost: [("core_fragment", 24), ("bytecode_block", 4)],
    power_supply: 4,
    siphons: true,
)
```

New `StructureDef` field `siphons: bool`, `#[serde(default)]`, documented in
`assets/structures/README.md`. Supply is the existing `power_supply`, so
tuning it is a data edit. Build cost is user-chosen (cheaper than the
Line Driver's 30 fragments + 6 grain; the real cost is the program).

### 2. Holding — the `UnderStudy` pattern

The held program stays a live entity, the same way a program pinned for
study does (`game/base/study.rs`, `pin_subject` / `unpin_subject`):

- `components::Siphoned { siphon: Entity }` on the program.
- New `ProgramRole::Siphoned` in `game/party.rs`, worked out in
  `program_role` the way `UnderStudy` is. Labour scheduling, needs-seeking
  and acting-out errands must all skip it, as they skip `UnderStudy`. The plan
  lists each place that matches on `UnderStudy` and gives every one a ruling.
- `Game::siphon_program(program, siphon) -> Result<(), String>` and
  `Game::release_siphoned(program) -> Result<(), String>`. Every refusal is
  checked before anything is written, following `pin_subject`'s rule. The
  refusals: not owned, not `Staff`, siphon already occupied, game over or in
  battle.
- Deconstructing an occupied siphon releases its program first, with the
  same release cost.

### 3. Grid supply — `game/base/power.rs`

`ledger` counts a siphon's `power_supply` only while some program holds a
`Siphoned` pointing at it. This sits beside the `is_fuelled` check, and both
`ledger` callers (the tick system and `Game::base_power`) pick it up. An empty
siphon supplies 0 and draws 0. The plan decides whether occupancy should also
gate the `power_regen` trickle that shares `is_fuelled`. The default is no:
a siphon has no `power_regen`.

### 4. The grudge — `assets/memories/siphoned.ron`

A `BaseTile`-subject memory, about the siphon's tile. It is a **stretch**
memory (see the memories README): written on a period for as long as the hold
lasts, so `strike_cap` amounts to a real stretch of the run. Once the program
is released, it fades by `half_life` with no new code.

- Valence, strike cap and half-life are chosen so that a capped grudge
  crosses `MORALE_SULKS_AT` on its own. That is the decision "builds to low
  mood".
- **This makes it the second kind that crosses the tantrum line on its own.**
  `unslotted` is exempted by name in `tests/disposition.rs`, and `siphoned`
  joins that exemption explicitly. The README paragraph claiming `unslotted`
  is the only one gets rewritten.
- The write period is a named constant in `tuning.rs`
  (`SIPHON_GRUDGE_PERIOD`).
- A held program on the acting-out ladder cannot walk to an amenity, so
  nothing interrupts the grudge until release. That is intended: the way
  back is to let it out.

### 5. Release cost

On release the program loses `SIPHON_RELEASE_INTEGRITY_LOSS` (75% of
`max_hp`, in `tuning.rs`), floored at 1 HP. It does not die. A Repair Bay
heals it through existing rules.

### 6. Save

`CreatureSave` gains `siphon: Option<(i32, i32)>` with `#[serde(default)]`.
The siphon is named by tile and resolved after `restore_structures`, the same
way `study_station` is. If it does not resolve, it is dropped silently, which
is that field's existing rule. The plan checks whether bincode tolerates the
added field or `SAVE_FORMAT_VERSION` needs a bump, because save.rs says
`serde(default)` only helps RON. A bump is a breaking release. Includes a
save → load test (`ron-round-trip-cannot-catch-a-skipped-field`).

### 7. App-core and GUI

At an empty siphon, the interact key opens a picker of `Staff` programs. At
an occupied siphon, it offers to release the program, and the prompt says it
will come out hurt. The roster shows the role as "Held in Power Siphon". The
key is UPPERCASE (`lowercase-letters-are-row-selectors`). The plan picks it
from the free keys at a structure.

## Testing

Engine unit tests, written before each piece:

- `ledger` supply rises by 4 while the siphon is occupied, and by 0 when empty
  or after release.
- Each refusal in `siphon_program` writes nothing.
- A held program is never handed a task and never leaves the tile.
- After `SIPHON_GRUDGE_PERIOD × strike_cap` ticks held, Morale is past
  `MORALE_SULKS_AT`. After release, it climbs back as the memory decays.
- Release takes off the Integrity loss and never goes below 1 HP.
  Deconstructing the siphon releases its program.
- Save → load keeps the program held, and the grid supply is unchanged.
- Census: the new `.ron` files parse, and the disposition exemption lists
  `siphoned`.

`balance_sim` is unaffected (no tuning of combat, species or items), but
gets run anyway as the full gate.

## Docs

`assets/structures/README.md` (the `siphons` field),
`assets/memories/README.md` (the new kind and the exemption), CHANGELOG at
release.
