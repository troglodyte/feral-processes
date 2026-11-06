# 0044. A bounty is a Rust-rolled contract that owns its quarry

- Status: Proposed
- Decided by: (pending) user, on Claude's proposal
- Date: 2026-11-06
- Pattern: Plain structs owned in one place; Functional core, imperative shell; Tolerant reader
- Depends on: [0019](0019-content-defaults-to-rust.md), [0020](0020-functional-core-imperative-shell.md), [0021](0021-plain-rust-over-ecs-for-discrete-logic.md), [0022](0022-rng-never-depends-on-query-order.md), [0023](0023-saves-are-field-named-ron.md)

## Context

The Contract Broker is getting **bounties**
([spec](../superpowers/specs/2026-11-06-bounty-contracts-design.md)). A bounty names one boss-grade
elite program that roams a small patch of the zone surface. The compass tracks it, and the party fights
it with an escort. The contract fails if the party flees or loses.

The contract system as it stands doesn't support four things a bounty needs:

1. **Content kind.** Contracts come from `assets/contracts/*.ron` and `templates/*.ron`. A bounty would
   be a new objective shape in that data.
2. **No fail path.** A contract today can only complete or be abandoned.
3. **No world object.** No contract owns anything in the world. A bounty's quarry is a creature that
   has to exist, roam, survive culls, breaches and save/load, and disappear when the contract ends.
4. **No named individuals.** `Creature` holds only a species.

## Decision

1. **Bounties are rolled in Rust, not from a template file** (ADR 0019). There is one kind, its tuning
   is a handful of numbers, and the roll needs the zone surface and the sector, not text slots.
   `Objective` gets a `Bounty(BountyQuarry)` variant, appended last, so the resolved definition rides
   in `ActiveContract.def` like every other accepted contract. An authored or template `.ron` that names
   `Bounty` is skipped with a logged warning: a bounty can't exist without a placed quarry, and only the
   roll places one. `TemplateObjective` gets no new variant, and no `.ron` file gains a field.
2. **The contract owns the quarry's definition. The map holds a projection of it.** `BountyQuarry` is
   plain data: name, species, grade, escort roster (species and count), anchor tile and tether radius.
   It lives inside the contract. The roaming creature is an ECS entity, because it ticks like every
   other wanderer. It carries a marker component naming its contract, and it is **respawned from the
   definition** whenever it is missing (on load, after a breach re-tier or a cull) instead of being
   saved as a creature. So the save gains no entity data, and nothing can strand a live contract
   without its quarry.
3. **The escort isn't on the map.** When the party bumps into the quarry, the battle is built from
   the contract's roster and handed to the battle door directly, past `gather_pack`'s sweep and the
   zone's group ceilings. So no escort member wanders off, gets culled or joins another fight, and
   "one group, one slot" can't shrink the fight.
4. **A contract can fail.** Failure is a third exit next to complete and abandon. A bounty fight that ends with the
   quarry alive (fled or lost) removes the contract, despawns the quarry and announces it. A failed bounty's rolled
   id is filtered from the board until the next refresh, so a flee can't be retried on the same roll.
5. **Deciding stays pure.** The bounty roll (species, grade, name, roster, anchor, reward) is a pure
   `fn(input, &mut Rng) -> RolledBounty` over a gathered input sorted by stable id (ADR 0020, 0022).
   The fight's outcome is a pure verdict: did the quarry die (complete) or not (fail). It is read off a tag on
   the battle, never off `FightVerdict.won`, so killing the quarry and then fleeing the escort still pays.

## Consequences

- Save: additive only. A new `Objective` variant and a new `CompassTarget` variant, both appended last
  under the tolerant reader (ADR 0023), with no `SAVE_FORMAT_VERSION` bump. An older build loading a
  save that holds a bounty fails on the unknown variant. That is the standard forward-compat cost and
  doesn't count as breaking.
- Every exhaustive `match` on `Objective` and `CompassTarget` gains an arm. The compiler finds them.
- Respawning means the quarry's wander position isn't saved: after a load it's back at its anchor.
  That's acceptable because the anchor is inside the tether radius the player already tracks.
- Making a contract own a world entity becomes a precedent. A later kind (an escort job or a
  courier) would follow this one: definition in the contract, projection on the map, respawn when
  missing.
- `contracts/README.md` documents `Bounty` as an objective that can't be authored.

## Rejected

- **A `templates/bounty.ron`.** It would need new template fields (grade, roster, tether) for a kind
  with one entry, which is the cost ADR 0019 says isn't worth paying.
- **Saving the quarry and escort as creatures.** That needs creature save data and cull and breach
  exemptions for every member. A culled escort member would also quietly make the fight easier.
- **Bounty state in a separate `resources::Bounties`.** That's a second owner, which the contract and
  its quarry could disagree with. The contract already owns its whole definition.
