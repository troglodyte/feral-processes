# Reinitialization Protocol — design

**Status:** approved in brainstorm 2026-09-28.

## Intent

A second road to staff, out of combat: spend a crafted **Reinitialization
Protocol** ("Resurrects a downed program as staff") on a `DownedProgram`
record the player is carrying, and it boots up as a tamed program. Decompiling
mid-fight stays the cheap, chancy primary path; this one is certain and paid
for in refined and advanced materials. Records only come from kills, so it
stays inside "progression is earned by fighting".

## Decisions

| Question | Decision |
|---|---|
| Success | Always succeeds. No roll, no `GameRng` draw. |
| Cost | One protocol per use, flat — does not scale with level or rarity. |
| Recipe | 1 `ice_breaker` + 2 `logic_wafer` + 1 `charge_coil` + 1 `cache_grain`, at the Fabricator. |
| Which records | Any carried record in `DownedPrograms` **except `boss: true`**. Racked and hoppered records are out of scope for now. |
| Level | Always 1 — `record.level` is really the zone the kill happened in (a wild body carries no `Experience` to restore). Condition is not read. |
| Rarity | The record's rarity, not a fresh roll. |
| Carried routine | Installed on the program if the record has one. |
| Roster room | Pet slots are soft (the usual `unslotted` memory); `ROSTER_HARD_CAP` refuses. |
| Where it spawns | The player's tile, as `adopt_program` does. |

## Components

### 1. Content: `assets/items/reinitialization_protocol.ron`

Display name "Reinitialization Protocol", description carrying "Resurrects a
downed program as staff." `craftable` with the recipe above and
`requires_structure: fabricator`. **No `taming_potency`**, so
`Game::taming_catalyst` can never spend one as a decompile catalyst. No cache
drop, no contract reward. Update `assets/items/README.md` if it lists items
by role.

### 2. Engine

- **`Game::reinitialize_program(index: usize) -> Result<(), String>`** in
  `game/extraction.rs`, beside `extract_program` (same store, same
  index-into-`DownedPrograms` shape). Order: validate index → refuse boss →
  refuse no protocol → refuse at `ROSTER_HARD_CAP` → only then spend the
  protocol, remove the record and spawn. Every refusal leaves the store and
  inventory untouched.
- **Rarity pinned at spawn.** `spawn_wild_creature_scaled` rolls rarity and
  bakes it into `Stats`; rarity's doc allows it applied only there and in
  `promote_rarity`. Thread an optional forced rarity through the spawn path
  (`adopt_program` → `spawn_wild_creature_scaled`) rather than adjusting
  stats afterwards. Existing callers pass "roll", so no RNG-stream change for
  them.
- **Level.** No level-setting call at all — `adopt_program_pinned` mints a
  fresh `Experience::default()` (level 1) through `roster_parts`, exactly
  the way a decompile does, so the resurrected program needs nothing raised
  and `arena::set_level` stays the arena's own.
- **Carried routine.** Installed via the same path extraction's routine
  reader / routine installation uses; the plan names the exact call after
  reading it.
- Log one `MessageKind::Outcome` line on success; note a contract `Deed` only
  if taming's `Deed::Tamed` is meant to count this (plan decides by reading
  contracts that key on it — default: it does **not**, since it isn't a
  fight).

### 3. app-core + gui

On `Mode::DownedPrograms`' per-record page (`pending_downed_program_index =
Some(_)`), an uppercase **R** action: *[R]einitialize — resurrect downed
program*. Plan confirms `R` is free there and how uppercase actions are
delivered as `GameKey`. When unavailable (boss, no protocol, hard cap) the
action line shows why instead of disappearing; the engine exposes that
reason (e.g. `reinitialize_blocker(index) -> Option<ReinitBlock>`) so the
screen and the action share one check. After success the screen returns to
the list (the record is gone).

### 4. Help and docs

A line in the relevant `assets/help/` page (supplies or downed programs).
CHANGELOG entry at release. No manual/README edits.

## Save format

None. The item is ordinary inventory; records and tamed programs already
save. `SAVE_FORMAT_VERSION` does not move.

## Testing

Engine unit tests, TDD:

- success spends one protocol, removes exactly that record, adds one tamed
  program on the roster;
- the resurrected program is always level 1, whatever the record's level
  or condition;
- the spawned program's `Rarity` equals the record's;
- carried routine is installed; `None` carried installs nothing extra;
- boss record refused, nothing spent;
- no protocol refused, nothing spent;
- at `ROSTER_HARD_CAP` refused, nothing spent;
- existing rarity-rolling spawns unchanged (existing suite covers it).

app-core: pressing R on a record page calls through and returns to the list;
blocked reason renders. `balance_sim` checked after the item lands.
