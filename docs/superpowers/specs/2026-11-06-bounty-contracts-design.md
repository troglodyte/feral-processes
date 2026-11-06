# Bounty Contracts — Design

**Status:** Proposed, 2026-11-06. ADR
[0044](../../adr/0044-a-bounty-is-a-rust-rolled-contract-that-owns-its-quarry.md) (Proposed).

## Why

Contracts today are counts and errands: kill N of a species, deliver, descend, build. The user wants a
**hunt for one specific, dangerous individual**. The Broker names a target, the compass leads to it, and
the fight is a set piece against a boss-grade program and the programs guarding it.

## What the player sees

1. The Contract Broker's board sometimes includes a **bounty**. Its row reads like the others: the
   name (for example "Bounty: Vesk the Long Wire"), a line about the target, and the payout.
2. Accepting it places the target on the zone surface. The compass picker gets a new row, `Bounty: Vesk
   the Long Wire · NE · 42`, with a live, exact bearing and distance like a town's.
3. The target **roams a small area** around where it was placed. The compass row follows it.
4. Walking into the target starts a battle with the target **and its whole escort on the field from the
   first turn**. The target's name appears in the battle.
5. **Win**: the named target goes down. The contract completes and pays large credits and XP through
   the usual completion summary. Whether the escort is also cleared doesn't matter: once the target
   is dead, running from the escort or losing to it doesn't affect the bounty. *(User's decision.)*
6. **Flee or lose while the target still stands**: the target gets away. The contract **fails**, its compass row disappears, and a
   message says so. The board may offer a new bounty later.

## Goals

- A bounty kind on the run's own Contract Broker. Its target is a **named elite program**: an ordinary
  habitat species raised to boss grade, with a generated name. Every bounty rolls a new one.
- The target is on the **zone surface** (never in a Stack) and roams within a tether radius of its
  anchor.
- **Exact** compass tracking, live as it moves.
- **An escort in the pack**, all present from turn 1. No reinforcements arrive mid-fight.
- **One try**: fleeing or losing fails the contract.
- **Danger scales with the sector**: the target's grade and the escort's size come from the zone level.
- **Reward**: credits and XP only, well above a Hunt contract at the same sector.

## Non-goals

- Bounties inside Stacks, or a target in a lair.
- Vague, "warmer/colder" or last-sighting tracking.
- Reinforcements, phases or scripted boss behaviour.
- Guaranteed drops, trophies or unique items. Ordinary loot from the kills still happens.
- Difficulty tiers on the board.
- Town-posted bounties (`issuer: Some(town)`).
- A signature routine for the target. *(Claude's call: left out as YAGNI. The grade multiplier is the
  difficulty lever.)*

## Anti-goals

- **A target that can disappear for no reason.** A cull, a breach re-tier or a save/load must never
  leave a live bounty without its quarry, and the compass must never point at nothing while the contract
  stands.
- **A fight shrunk by the zone's group caps.** The escort the contract names is the escort you fight.
- **Accidental engagement.** Bumping an ordinary wanderer near the target must not pull the target into
  that fight, and the target must not be swept into someone else's pack.
- **Occult or slogan naming** in generated names (standing content rule). Use plain, machine-flavoured
  names.
- **Free repeatable income.** The payout is earned by a real fight that can be lost (standing design
  rule).
- Player-facing tick vocabulary in any new text.

## Decided by the user

- Target on the zone surface.
- Target is a named elite program: an ordinary species made boss-grade, newly rolled each time.
- Compass is exact, like a town.
- Escort is in the pack from turn 1.
- Target roams nearby, and the compass follows it live.
- Fleeing or losing fails the contract.
- Danger scales with the sector, with no tiered offers.
- Reward is large credits and XP.
- The bounty pays when the target dies. The escort doesn't have to be cleared.

## Claude's calls (overridable)

- **Board placement.** At most one bounty on a board per refresh. It takes one of the
  `CONTRACT_BOARD_SLOTS`, rolled by the same seeded, derived board roll, so it is stable across save and
  load and can't be rerolled by reloading.
- **One bounty held at a time.** It counts against `MAX_ACTIVE_CONTRACTS`. While you hold one, the board
  offers no second.
- **Lowest sector is 2** (`min_zone: 2`). Zone 1 pins groups to size 1, and the opening ring blocks
  bosses. A boss-and-escort set piece is out of place there.
- **Placement.** On accept, the anchor is a walkable surface tile a tuned distance band from the player
  (roughly 25–60 tiles), outside any settlement footprint and not on a Stack link. Tether radius is
  about 6 tiles. All of these numbers are `tuning/bounty.rs` constants.
- **The target is the only thing on the map.** The escort exists only in the contract and is fielded at
  the bump (ADR 0044 §3).
- **Respawn, not save.** The target is respawned at its anchor from the contract whenever it is missing,
  including after a load (ADR 0044 §2).
- **Abandoning** a bounty works like any contract (it's from the run's own Broker, so there's no
  standing cost). The target despawns.
- **Breach** to a new sector mid-bounty: the bounty stays, and its target respawns at its anchor at its
  *original* grade. That matches "breaching never strands a contract you already hold".
- **Kills in a bounty fight** count as normal kills: XP, loot and `Terminate` progress on other contracts.
- **The target doesn't pursue** the player. It wanders inside the tether, and engaging it is always the
  player's choice.
- **Names** come from a small Rust word bank, `<Name> the <Epithet>`, rolled from the bounty's seed. Not
  saved separately: it's in the contract's definition.
- **Fail hook placement.** The contract fails in `finish_fight` whenever a bounty-tagged fight ends
  with the quarry alive. A Forgiving-mode reboot doesn't rewind state, so the failure stands, and it fires exactly
  once from teardown.
- **No same-roll retry.** A failed bounty's rolled id is recorded and filtered from the board until the
  next refresh rolls a new one. Fleeing then reloading the board can't hand you the same target again.
  Holding a bounty filters that slot the same way an accepted contract's slot is filtered today.
- **Everything is fixed at roll time**: grade, roster, reward, name and the board line. The board line
  is generated from the name and species, for example "Vesk the Long Wire, a boss-grade crawler, is
  running with an escort of six. Bring it down." It isn't authored.
- **Respawn triggers** are load, breach re-tier and cull. One cheap check for the single marked
  entity, in the existing contract tick, covers all three. A breach doesn't regenerate terrain, so the
  anchor stays valid.
- **The anchor only has to be walkable.** No reachability search: the surface is open ground, and the
  same filter as the existing free-tile placement is enough.
- **The escort may span several groups.** It is grouped by species the way `group_pack` groups today.
  The zone's group-count and group-size ceilings don't apply to a bounty fight, in either combat model.
- **Roaming ships in the first playable slice.** A static first version isn't a deliverable.
- **Failure text.** "Vesk the Long Wire got away. Bounty failed." Through the existing notification and
  log path, fired once.

## How it fits (for the builder)

Findings from the code (verify before relying on them):

- **Contracts:** `Objective` (`crates/engine/src/contracts.rs:205`), `ContractDef` (:422), the board
  (`game/contracts.rs`: `contract_board`, `board_defs`, `rolled_contracts`, `offerable`, `board_seed`),
  accept (:1313), abandon (:1384), and the single completion door `settle_contracts`/`complete_contract`
  (:239/:261). There's no fail path yet. Add one beside abandon, with its own announcement.
- **Compass:** `CompassTarget` (`settlements/mod.rs:46`, append `Bounty(<contract id>)` last),
  `Game::compass_targets` (`game/inspection.rs:2064`, add a row group after outposts),
  `compass_bearing` (falls back to Home when the selection vanishes. Also clear the stored selection when
  a bounty ends).
- **Wanderers:** `wander_ai_system` (`systems.rs:316`) has no general tether. Only `NestGuardian`
  (`NEST_TETHER_RADIUS`) does. The quarry needs the same kind of tether. `cull_to_cap`
  (`spawning.rs:1555`) and `Game::clear_local_wild` must spare the quarry or let it respawn.
- **Bump → battle:** `move_player_inner` → `find_wild_creature_at` → `gather_pack` → `start_battle`.
  The quarry's bump builds its own groups from the roster and calls the battle door directly (the arena
  uses `begin_battle(groups)` this way). Check how the tactical model (`open_tactical_battle`,
  `squads::plan`) gets the same uncapped roster. `gather_pack` must exclude the quarry, the way it
  already drops other bosses.
- **Elite stats:** stats are baked at spawn (`spawn_wild_creature_pinned`: base × zone × depth ×
  `BOSS_STAT_MULT` × rarity × potential). The grade is a sector-scaled multiplier on that path, with
  a raised `Rarity`. `spawn_pack`/`pick_escort_species` (`spawning.rs:2007/2057`) is the existing
  boss-plus-escort precedent for picking escort species.
- **Outcome:** `finish_fight` (`game/combat_teardown.rs`) is the one teardown for both combat models,
  with `FightVerdict{won, …}`. Tag the battle (`BattleState.lair` is the precedent) so teardown knows it
  was a bounty fight. The verdict depends on **whether the quarry died**, not on `won`. The quarry's
  kill (the `award_loot` path, where kills are recorded) marks the bounty met, and it completes
  through the normal door. A fight that ends with the quarry alive (fled or lost) fails it. A Forgiving-mode reboot after a loss still counts as lost.
- **Module:** a new `crates/engine/src/bounty/` directory (roll, name bank, outcome, tests), with
  constants in `tuning/bounty.rs`. Both combat models, the compass and contracts call into it.
- **app-core / gui:** the contracts screen and the compass picker already render whatever rows the
  engine hands them. Expected work: the bounty row's glyph and colour in the picker and on the map, the
  target's name in the battle header, and the failure notification. No new mode.
- **Save:** additive only (ADR 0044 consequences). A save → load test with a live bounty is required
  (standing rule: a RON round trip can't catch a skipped field).
- **Balance:** add no species or item. Run `balance_sim` anyway if any shared tuning moves. Check
  bounty difficulty with an arena scenario (`dev-arenas/`) of a sector-2 and a sector-6 bounty against a
  mid-grade party. Record the result in `docs/measurements/` only if it meets that README's bar.
- **Docs:** `assets/contracts/README.md` (Bounty can't be authored), `CHANGELOG.md`, and an `INDEX.md`
  row.

## Left to the builder

- Exact tuning numbers: grade multiplier curve, escort size per sector, reward formula, distance band,
  tether radius, board odds.
- The name word bank's contents (within the naming rules).
- Whether the bump route goes through a small `start_bounty_battle` or a flag on `start_battle`, as long
  as the zone caps don't apply and `gather_pack` doesn't sweep.
- Phasing of the plan.
