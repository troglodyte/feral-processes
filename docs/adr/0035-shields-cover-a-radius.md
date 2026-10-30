# 0035 A Shield covers a radius that grows with its tier

- Status: Agreed
- Decided by: user, on Claude's proposal
- Date: 2026-10-30
- Pattern: —
- Depends on: [0019](0019-content-defaults-to-rust.md), [0032](0032-base-returns-to-the-field.md)

## Context

A Shield's `StructureDef::raid_defense` is summed base-wide by `Game::structure_raid_defense`, so where it stands does not matter.
It cuts only GC Entropy Sweeps (`Game::run_raid`), town raids (`Game::run_town_raid`) and the off-screen siege (`Game::resolve_siege_offscreen`).
A siege fought on the battle board never reads it: enemy hits on structures go straight to `Game::damage_structure`, and hits on bodies go through `Game::apply_damage_typed` with no Shield term.
`shield.ron` has no `upgrade:`, so a Shield cannot be upgraded.

The user wants a Shield to protect a circle around itself, shown on hover, with the circle growing per upgrade tier, and that circle to soften enemy hits in a fought siege.

## Decision

`StructureDef` gains `shield_radius: Option<ShieldRadiusDef>` with `base` and `per_tier`, `#[serde(default)]`.
The radius at tier `t` is `base + per_tier * (t - 1)`.
`shield.ron` sets `base: 4, per_tier: 2` (4, 6, 8, 10, 12 tiles) and gains an ordinary `upgrade:`.

A tile is covered when the circle centred on the anchor tile's centre touches the tile's square at all.
One pure function in a new engine module answers coverage, and every reader calls it: sweeps, the board, the outline and the gui circle.

A defence with `raid_defense` and no `shield_radius` (the Turret) stays base-wide.
A sweep on a target counts each base-wide defence plus each armed Shield that covers the target.
Town raids, the garrison and the off-screen siege keep the base-wide total.

In a fought siege, an enemy hit on a body or structure loses `raid_defense` for each armed Shield covering its tile.
For a body this is added to deflection, before the existing floor of 1.

## Alternatives considered

- A radius in Rust keyed by structure id. That is a per-id table where the data file already holds the defence number; Shield is a structure def, and its defence already lives in data.
- A flat `radius: u32` with no tier. The user asked for the radius to grow with upgrades.
- Making every `raid_defense` local, Turrets included. That changes a structure the user did not ask about.

## Consequences

Where a Shield stands now matters for sweeps, which is a balance change: a structure outside every circle takes the full `RAID_DAMAGE`.
`Game::raid_defense_active` stops being the outline's signal; the outline asks coverage per structure.
`assets/structures/README.md` documents the field.
The fights-use-things-up spec's non-goal "a tactical role for the Shield" is overturned on purpose.

## Evidence

`StructureDef::raid_defense`, `Game::structure_raid_defense`, `Game::armed_defences`, `Game::run_raid`, `Game::apply_damage_typed`, `battle::damage_after_mitigation_typed`, `Game::damage_structure`, `RepairDef::per_tier`, `TacticalBattle::siege_origin`, `assets/structures/shield.ron`.
