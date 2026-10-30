# 0036 A Patch Node repairs a radius that grows with its tier

- Status: Agreed
- Decided by: user, on Claude's proposal
- Date: 2026-10-30
- Pattern: —
- Depends on: [0035](0035-shields-cover-a-radius.md), [0019](0019-content-defaults-to-rust.md)

## Context

`StructureDef::repair` (`RepairDef { per_tier }`) is summed base-wide by `Game::total_repair_rate`, so a Patch Node's position does not matter.
Every `STRUCTURE_REGEN_INTERVAL` ticks, every deployed structure gets that total back.
`RepairDef`'s doc comment says it has no radius on purpose, because every structure already stands near the Home.
`Perk::Failover` adds `FAILOVER_REPAIR_PER_LEVEL` to the same base-wide rate.
Patch Nodes do nothing in a siege fought on the battle board.

ADR 0035 gave Shields a circle that grows with tier, shown on hover, and one coverage function (`shields::covers`).
The user wants Patch Nodes to work the same way: a light green circle, and a green + on whatever a node heals.
They also want a node to heal structures, base staff and the player inside its circle during a fought siege, every other round, with the interval tunable.

## Decision

`ShieldRadiusDef` and the coverage functions move out of `shields/` into a shared engine module and are renamed `RadiusDef`.
The fields stay `base` and `per_tier`, and so does the rule: radius at tier `t` is `base + per_tier * (t - 1)`, and a tile is covered when the circle touches its square.
`shield_radius` keeps its name and takes the shared type.

`RepairDef` becomes `{ amount: u32, radius: RadiusDef }`, replacing `per_tier`.
`patch_node.ron` sets `amount: 1, radius: (base: 4, per_tier: 2)`, so the radius runs 4, 6, 8, 10, 12 tiles.
Upgrading a node grows only its circle. The amount does not scale with tier.

**Off-battle.** Every `STRUCTURE_REGEN_INTERVAL`, each damaged structure regains `amount` from every Patch Node whose circle covers any cell of its footprint.
Overlapping nodes add together. Structures outside every circle regain nothing from nodes.
Patch Nodes heal only structures off-battle. Healing bodies stays the Repair Bay's job.

**Fought siege.** Every `PATCH_SIEGE_PULSE_ROUNDS` rounds (default 2), each Patch Node still standing on the field heals what its circle covers by `PATCH_SIEGE_HEAL` (default 2): structures by that much Durability, and the base's staff and the player by that much HP, capped at the maximum.
Enemies are never healed. Overlapping nodes stack. Both constants live in `tuning/`.
The round number decides when a pulse fires, so it needs no new save field.

**Failover.** `Perk::Failover` is removed from the perks a player can pick.
The enum variant stays so saves that hold it still load, and a held level keeps adding base-wide repair as it does today.
Returning to the perk is left for later.

**Display.** The circle draws through the Shield's circle path with a light-green colour, on hover and in the deploy and upgrade previews.
A healed structure, and in a siege a healed body, flashes a green + on its tile, through the same effect path that flashes a softened Shield hit.

## Alternatives considered

- Keep `ShieldRadiusDef` and give `RepairDef` a field of that type. A Patch Node's reach would be named after a Shield, and a third circle would copy it again.
- Keep `per_tier` alongside the radius. The user asked for upgrades to grow only the circle.
- Delete `Perk::Failover`. Saves holding it would stop loading. The user asked to remove it or hide it for now.
- Heal bodies off-battle too. That overlaps the Repair Bay. The user chose structures only.

## Consequences

Where a Patch Node stands now matters, which is a balance change: a structure outside every circle no longer mends without Failover.
The `RepairDef` doc comment stating "deliberately has no radius" is replaced.
Patch Nodes become a factor in fought sieges, so siege balance moves. Re-check `balance_sim`, and the arena if a siege scenario covers this.
`assets/structures/README.md` documents the changed `repair` field and the shared radius type.

## Evidence

`RepairDef`, `Game::total_repair_rate`, `STRUCTURE_REGEN_INTERVAL`, `perks::repair_rate_bonus`, `Perk::Failover`, `reach::{RadiusDef, radius_at, covers, covers_any}`, `TacticalBattle::round`, `TacticalBattle::shields_softened`, `app::reach_circles::ReachCircle`, `gui/src/render/reach_circle.rs`, `assets/structures/patch_node.ron`.
