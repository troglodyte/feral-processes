# Respawn routine — design

**Status:** approved in brainstorm 2026-09-28.

## Intent

A discoverable area-effect routine, **Respawn**, that raises fallen bodies on
the battle map — the grey `x` marks `TacticalBattle::fall` leaves — as
temporary fighters on the **caster's** side. The player's answer to a board
full of wreckage; in a wild carrier's hands, a reason to kill it first. A new
perk, **Spawn Priority**, makes both raised bodies and forks stronger.

## Decisions

| Question | Decision |
|---|---|
| Name | Routine **Respawn** (`respawn.ron`), perk **Spawn Priority**. |
| Where | Battle map only (`tactical_only`). The group model has no fallen marks; `ability_unavailable` refuses with `BattleMapOnly`. |
| Which bodies | Either side's. **Not** bosses, forks, or already-raised bodies — those leave a plain, unraisable `x`. Structures leave a plain `x` (no `Creature`). |
| How many | Effect `Reanimate { count }`; up to `count` raisable marks in the shape rise, nearest the aim cell first. Authored `count: 3`. |
| Whose side | The caster's: a player/companion cast raises onto the party side, a `Hostile` cast onto the hostile side. |
| Strength | The fallen body's snapshot `Stats` (max_hp, atk, mitigation) × multiplier; `hp = max_hp`. |
| Multiplier | `REANIMATE_STAT_MULT` (0.75) + `SPAWN_PRIORITY_STAT_PER_LEVEL` (0.10) × Spawn Priority level. **No cap** — above 1.0 is intended. |
| Perk reads | The **player's** perks for a party-side cast (as `summon_rarity_window` does). A hostile cast gets no perk bonus. |
| Forks | Spawn Priority also adds its per-level amount to forks' `SUMMON_STAT_MULT` (0.6) in `fork_programs`. |
| Lifetime | `Summoned` containment: no XP, no `Tamed`/`Experience`/`ProgramId`, never roster or save, swept by `finish_fight`. |
| Recast | Replaces **that side's previous Respawn set** only. Forks keep their own pool; neither dissolves the other or the opposing side's. |
| Falling again | A raised body that falls leaves a plain `x`. |
| Carrier | `wild_weight` (joins the hunt-only set, like `bit_rot`). Any wild program may spawn carrying it and casts it. No species kit names it. |
| Hostile raised bodies | Killing one pays **nothing** — no XP, loot, `DownedProgram`, nest or patrol consequence. They still count as `Hostile` for win detection. |
| Look | Raised bodies draw glyph and sprite tint in a new `palette::RESPAWNED`, a dark red-grey (start ≈ `(0.42, 0.24, 0.24)`, tuned by screenshot). Both sides. |

## Design

### The fallen snapshot

`Fallen` (`tactical/mod.rs`) gains `raise: Option<FallenBody>`, where
`FallenBody { species: String, rarity: Rarity, stats: Stats }`. `fall` fills
it only for a `Creature` that is not `Boss` and not `Summoned`; the entity is
still alive with all components at both call sites, so this is a read before
the despawn. Nothing about `TacticalBattle` is saved, so no save-format
change. (Also move `TacticalBattle`'s doc comment back onto
`TacticalBattle`: the last commit left it attached to `Fallen`.)

### The effect

`AbilityEffect::Reanimate { count: u32 }`. `tactical_only()` returns true;
`affinity_kind()` is `None`; every exhaustive match gets an arm (the group
`use_ability` arm is `unreachable!` like Teleport's). `routine_tree::gets_node`
keeps excluding `Summon` only.

`tactical_use_routine` refuses before any cost when no raisable mark lies in
the shape's cells (`reach::shape_cells`, same line-of-sight clipping as
ordinary recipients) or none of those cells is free to stand on. A new
`RoutineRefusal` variant carries the "nothing to respawn" string so app-core
can show it.

`run_tactical_routine` gets a `Reanimate` branch: dissolve the caster side's
previous Respawn set, then for each chosen mark spawn the species at the
sentinel (`spawn_wild_creature_scaled` with a pinned rarity), overwrite
`Stats` from the snapshot × multiplier, set the side (`Hostile` kept or
stripped per caster; `WanderAi` stripped), insert `Summoned` + a new
`Respawned` marker + `PowerReserve`, seat it on the mark's own cell (or
`deploy::nearest_free` to it when something now stands there) behind the
cursor (`insert_after_cursor`), and remove the mark. The multiplier is one
pure function both this and `fork_programs` call for the perk term.

`Respawned` is what separates the two summon pools and what the view reads;
`dissolve_summons` narrows to forks (`Summoned` without `Respawned`).

### Hostile summons

Today every `Summoned` body is party-side. A hostile raised body is
`Hostile + Summoned + Respawned`:

- `tactical_ai_actor` already drives it; `tactical_sides_from` already sides
  it by `Hostile`.
- `reap_tactical_dead`'s hostile branch gets a `Summoned` guard: fall (plain
  `x`), despawn, pay nothing.
- `finish_fight` already sweeps every `Summoned` before `mark_nemeses`.
- Tactical Decompile refuses any `Summoned` target (it would otherwise join
  the roster and then be swept).

### Wild casting

`ready_from_candidates` filters `tactical_only()` out of wild routines; it
must admit `Reanimate` when the body is on the battle map, while the group
model keeps filtering it. `best_aim` scores a Reanimate aim by the raisable
marks its shape covers (the hallucination-decoy counting is the precedent)
and rejects an aim covering none. The party-side `aimable_this_turn` needs the
same reading for a companion that carries Respawn.

### Discovery

The README's family classification lists tactical-only families as always
visible. Respawn has a carrier, so it must classify as **discoverable**; the
plan verifies which rule wins in the classifier and adjusts it (and the
README) if tactical-only overrides carrier. `wild_pool().len()` grows by one
(the assertion in `tests/assets.rs`).

### The perk

`Perk::SpawnPriority` appended to the enum (order is save format), to
`Perk::all()`, the `one_level_is_worth_something` match, `tests/perks.rs`'s
index asserts, `assets/perks/spawn_priority.ron`, `groups.ron`, and the perks
README. Query fn `spawn_priority_level(perks)`.

### The look

`TacticalBody` gains `respawned: bool` (from `Respawned`). The tactical
renderer uses `palette::RESPAWNED` in place of `glyph_color(body.color)` for
glyph ink and sprite tint when set. The con read under a hostile raised body
is unchanged. Only through `Painter`.

### A latent bug to settle first

`reap_tactical_dead` sends a non-`Hostile` body that is not the player and not
in `Party` down the base-staff branch. Tactical forks are never pushed to
`Party`, so a fork dying mid-fight on the map appears to take that branch.
Respawn shares the path. Start with a reproducer; fix it if it reproduces.

## Testing

Engine unit tests, each written failing first:

- A party cast raises the raisable marks in the shape onto the party side at
  snapshot × 0.75, removes those marks, and caps at `count` nearest-first.
- A boss, a fork, or a raised body leaves an unraisable mark.
- Refusals: no raisable mark in shape, and group-model battle — no Power spent.
- Spawn Priority raises both the Respawn and the fork multiplier; above 1.0
  is allowed.
- Recast replaces only that side's Respawn set; forks and the other side's
  set survive.
- A hostile carrier casts it; its raised bodies are `Hostile`, AI-driven, and
  killing one pays no XP/loot/`DownedProgram`.
- Decompile refuses a raised body.
- All raised bodies are gone after `finish_fight`.
- The view flags `respawned` for a raised body and not a fork.
- Discovery: Respawn hidden until extracted.

Plus the fork-death reproducer above, `balance_sim` unchanged (no ability
modelling, but run it as the gate), a `dev-arenas/` scenario with a carrier
and a Respawn-equipped player, and a `--screenshot` of the colour.

## Docs

`assets/abilities/README.md` (effect, hunt-only list, discovery if the
classifier changes), `assets/perks/README.md`, `CHANGELOG.md`.
