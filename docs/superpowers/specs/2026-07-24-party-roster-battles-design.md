# Party Roster Battles — design

**Status:** approved design, not yet implemented
**Date:** 2026-07-24

## Problem

An intrusion is a duel with an audience.

`BattleState` (`resources.rs:126`) holds a flat `Vec<Entity>` pack in which
only `wild_creatures[0]` — the "front" — can be attacked. Every other member
waits in line, contributing nothing but retaliation. On the player's side the
whole team gets **one** action per round, and spending it on a companion
(`battle_command_companion`, lib.rs:2578) doesn't even attack: the companion
grants a buff and the round ends.

The result is that neither group is really a group:

| | Modelled as | Actually behaves as |
|---|---|---|
| Wild pack | `Vec<Entity>`, 2–6 deep | one target with a damage multiplier |
| `Party` | up to 3 tamed programs | one buff dispenser + damage sponges |

`MoveDef { name, power, effect }` (`species.rs:90`) is already data-driven and
already loaded for every species, but only `wild_retaliate` ever reads it. The
player's strike is a hardcoded `move_power: 5` (lib.rs:2557) and companions
never use their moveset at all.

## Goals

A Bard's Tale combat screen: enemies listed as addressable species groups with
health, the party listed as a roster with health and stats, and an action
chosen for **every** party member each round, resolved together in initiative
order.

## Non-goals

- Hard party ranks, and the party-reorder screen they would require.
- Player-side reach limits — the reach rule is one-directional (§4).
- A ranged/melee distinction for the player's own strike.
- Multi-target or area-effect moves.
- Formation or positioning beyond slot order.
- Persisting a battle in progress across save/load. Battles remain
  session-scoped; `BattleState` stays out of `SaveData`.

---

## 1. Enemy groups

`BattleState` gains a group model, replacing the flat pack:

```rust
pub struct EnemyGroup {
    pub species: SpeciesId,
    pub members: Vec<Entity>,   // members[0] is the front; only it takes hits
}

pub struct BattleState {
    pub player: Entity,
    pub groups: Vec<EnemyGroup>,
    pub round: u32,
    pub planned: Vec<Option<BattleAction>>,  // one slot per party member
    pub log: Vec<String>,
    pub finished: bool,
    pub player_won: bool,
}
```

`start_battle` (lib.rs:2451) partitions whatever `gather_pack` returns by
species, in **first-appearance order**. `gather_pack` iterates an ECS query,
so the ordering guarantee has to come from the partition step itself, not from
the query — see the unsorted-habitat-lookup flake this repo already produced
for why an incidental iteration order is not a stable order.

If more than `MAX_ENEMY_GROUPS` (4) distinct species cluster, the four largest
engage and the remainder stay on the map as un-engaged hostiles. They are
never silently despawned; the player meets them on the next bump.

Group indices 0 and 1 are *engaged*, 2 and 3 are *back*. Wiping a group removes
it and shifts the list, promoting a back group into melee range.
`finish_front_pack_member` (lib.rs:2649) generalises to
`finish_group_member(group_idx)`, keeping its existing loot / XP /
nest-respawn behaviour intact.

## 2. The round loop

Planning is engine-owned, so both renderers stay dumb:

```rust
pub fn battle_action_options(&self, slot: usize) -> Vec<ActionOption>;
pub fn battle_set_action(&mut self, slot: usize, action: BattleAction) -> Result<(), String>;
pub fn battle_clear_action(&mut self, slot: usize);   // back up one slot
pub fn battle_resolve_round(&mut self);               // all slots planned
pub fn battle_jack_out(&mut self);                    // party-level, immediate
```

Resolution:

1. Roll initiative for every living actor — party and enemy — as
   `base_speed + rng.random_range(0..=INITIATIVE_DIE)`.
2. Sort descending, breaking ties on a stable key (party before enemies, then
   slot / group index), so seeded tests are reproducible.
3. Walk the order. Skip anything already dead. A stunned actor loses its turn
   (`is_stunned` already exists). If a planned target group died earlier in the
   round, retarget to the lowest surviving group index; idle if none remain.
4. Append each result to `BattleState::log`, which the UI pages through.

Initiative reuses the existing seeded `GameRng`, so this adds no new flake
surface.

## 3. Actions, built to extend

The extensibility requirement is the reason for splitting what the menu
*offers* from what an action *is*:

```rust
pub enum ActionKind { Attack, Special, Defend, Decompile, UseItem }

pub enum TargetSpec { None, EnemyGroup, InventoryItem }

pub struct ActionOption {
    pub kind: ActionKind,
    pub key: char,                    // engine assigns the hotkey
    pub label: String,                // "[A]ttack"
    pub detail: String,               // "Rally: +3 ATK for 3 rounds"
    pub target: TargetSpec,
    pub unavailable: Option<String>,  // Some(reason) => render greyed
}

pub enum BattleAction {
    Attack { group: usize },
    Special { group: usize },
    Defend,
    Decompile { group: usize },
    UseItem { item: ItemId },
}
```

Adding a sixth action later is one `ActionKind` variant, one `BattleAction`
variant, one arm in `resolve_one_action`, and one availability rule in
`battle_action_options` — **zero renderer changes**.

That inverts today's arrangement, where the TUI hardcodes the action strings
(`tui/src/ui.rs:1859`) and app-core hardcodes the keys
(`app-core/src/lib.rs:873`). Both must dispatch off `ActionOption` instead.

Actions in detail:

- **Attack** — rolls from the actor's species moveset, the way
  `wild_retaliate` already does, instead of the hardcoded power-5 strike.
  Companions deal damage for the first time.
- **Special** — the species' existing `special_ability` (Rally / Shield /
  Heal / Debuff), now one choice among several rather than the only one.
  Player has none, so this row is companion-only.
- **Defend** — brace: `DEFEND_DEF_BONUS` for the round, and a raised share of
  incoming fire. New mechanic; gives a wounded companion something to do.
- **Decompile** / **Use Item** — player-only. Decompile targets a group's
  front member and spends a catalyst via the existing
  `taming::capture_chance`.

Jack Out stays a party-level command, not a per-member action.

## 4. Reach

Only the front two enemy groups can melee. Back groups need a ranged move.
This is the balance valve that makes a twelve-enemy fight survivable, and it
creates the central tension: killing the front group *promotes* a back group
into melee range, so clearing front-to-back is not automatically correct.

The rule is deliberately **one-directional** — the party may target any group.
Symmetric reach would leave a player with no ranged move unable to touch the
back rank at all, and the player's strike is staying melee-only (§Non-goals).

## 5. Schema additions

Both `#[serde(default)]`, so shipped and third-party `.ron` files keep parsing
untouched, per the moddability rule in `CLAUDE.md`:

- `SpeciesDef::base_speed: i32` — defaults to `DEFAULT_BASE_SPEED`.
- `MoveDef::ranged: bool` — defaults to `false` (melee). A back group picks
  only from its ranged moves; with none it idles with a flavour line rather
  than failing silently.

`assets/species/README.md` gains both fields in the same change — that doc is
the schema reference for anyone modding the game.

A data pass over all 17 `assets/species/*.ron` files authors `base_speed` and
tags at least one ranged move on species that should threaten from the back
rank. Without this pass every back group is inert and the reach mechanic is
invisible in play.

## 6. Balance

New constants live in `balance.rs` beside the existing ones:

- `MAX_PARTY_SIZE` 3 → 5 (`resources.rs:136`). `BASE_PET_CAPACITY` stays 3, so
  a full roster remains a Data Cache progression goal rather than a freebie.

  **This buff compounds.** `party_stat_bonus` already feeds a share of every
  party member's ATK/DEF into the player's effective stats (`effective_atk`,
  lib.rs:2791). Going 3 → 5 therefore raises the player's *passive* ATK/DEF by
  ~67% **and** adds two more attackers **and** spreads incoming damage across
  two more bodies — three multiplicative gains from one constant. The pack-size
  increase below is what offsets it, and getting that ratio right is the single
  most load-bearing number in this design. It has to be swept in `balance.rs`,
  not eyeballed.
- `MAX_ENEMY_GROUPS = 4`, `ENGAGED_GROUPS = 2`.
- `max_pack_size` (lib.rs:3843) cap becomes `(zone * 3).min(MAX_PACK_SIZE)`
  with `MAX_PACK_SIZE = 12`. Distance growth is unchanged.
- `PACK_GATHER_RADIUS` 2 → 3, so enough hostiles cluster to fill a big pack.
- `DEFAULT_BASE_SPEED`, `PLAYER_BASE_SPEED`, `INITIATIVE_DIE`.
- `DEFEND_DEF_BONUS`, `DEFEND_AGGRO_WEIGHT`, and front/back slot targeting
  weights, generalising `COMPANION_RETALIATION_CHANCE`.

**`balance.rs` needs a rewrite.** Its offline projections simulate the current
loop directly — one action per round, a `RALLY_CADENCE` of 4 (balance.rs:21).
Once companions attack and initiative interleaves, those regression tests
certify a game that no longer exists.

## 7. Persistence — this breaks saves

Soft ranks make party order mechanically meaningful. `CreatureSave`
(`save.rs:48`) records only an `is_companion` bool; order is rebuilt from
creature-iteration order on load (lib.rs:790–801). Adding a slot index is a
shape change to `CreatureSave`.

bincode has no field-level compatibility — see the long comment at
`save.rs:146` documenting the exact footgun this project already hit, where
`#[serde(default)]` silently produced corruption rather than defaults. So:

`SAVE_FORMAT_VERSION` goes **9 → 10, and every existing save is rejected** with
the existing clear error. There is no way around this short of dropping
persisted party order.

## 8. Renderers

`BattleView` is rewritten around two lists plus the menu:

```rust
pub struct BattleView {
    pub groups: Vec<EnemyGroupView>,   // letter, species, count, front HP, engaged/back, status
    pub party: Vec<PartySlotView>,     // slot, name, HP, ATK/DEF, status, planned action, front/back
    pub active_slot: Option<usize>,    // whose action is being picked
    pub options: Vec<ActionOption>,    // for active_slot
    pub round: u32,
    pub log: Vec<String>,
    pub decompile_chance: Option<f32>,
}
```

app-core gains `Mode::BattleTarget` (picking a group) and `Mode::BattleResolve`
(paging the narration), replacing `Mode::BattleCompanion`.

TUI `render_battle` (`ui.rs:1753`) becomes the two-panel roster; GUI
`draw_battle` (`render.rs:1758`) mirrors it.

`Fx::battle_frame` (`fx.rs:219`) tracks exactly two HP scalars for its
ghost-bar trail. It generalises to a keyed lookup —
`Fx::bar_ghost(key, value, dt) -> BarFx` — so every group and party row
animates independently.

The engine's `Game` struct stays the entire public API surface. Neither
renderer touches the ECS `World`.

## 9. Build order

Each step ends green, so work can stop between any two:

1. Engine group model + planning API + resolution, initiative included.
   Renderers get a minimal port just to compile.
2. `base_speed` / `ranged` schema, species data pass, README update.
3. Defend, soft-rank targeting weights, party and pack size changes.
4. Save format bump for party order.
5. `balance.rs` rewrite against the new round loop.
6. TUI roster screen.
7. GUI roster screen + `Fx` generalisation.

## 10. Verification

Seeded unit tests throughout — no `sleep()`, no wall-clock, no unseeded RNG:

- **Grouping** — a mixed pack partitions by species in deterministic order; a
  five-species cluster engages only the four largest and leaves the rest on
  the map.
- **Promotion** — wiping group 0 promotes group 2 from back to engaged.
- **Reach** — a back group whose species has only melee moves idles and deals
  0 damage; give it a ranged move and it connects.
- **Initiative** — a fixed seed produces a stable order; a faster species acts
  before a slower one across a large sample.
- **Round integrity** — an actor killed mid-round never acts; an action whose
  target group died mid-round retargets rather than panicking or no-opping.
- **Planning API** — `battle_set_action` on an out-of-range slot or a dead
  member is refused; `battle_resolve_round` before every slot is planned is a
  no-op; `battle_clear_action` walks the cursor back.
- **Menu is data** — `battle_action_options` marks Decompile unavailable with
  a reason when no catalyst is held, and neither renderer contains a
  hardcoded action string.
- **Save round trip** — party order survives save/load; a v9 save is rejected
  with the existing clear error rather than decoding to garbage.
- **Assets** — every shipped species parses with the two new fields, and a
  `.ron` file omitting both still loads. That is the modding contract.

Full gate before calling it done: `cargo test --workspace`,
`cargo clippy --workspace`, `cargo fmt`.

Balance is arithmetic-proven in `balance.rs`, not played — consistent with how
raid tuning and the travelling base were verified. A twelve-enemy fight
against a party of six is a large enough swing that the projections are the
only evidence available until someone actually plays it, and the work should
say so rather than claim it is tuned.

## Documentation obligations

- `assets/species/README.md` — `base_speed` and `ranged` (§5).
- `CHANGELOG.md` — the save-format break is a user-visible breaking change and
  must be called out, not buried in a feature line.
- Root `README.md` — this change falsifies more of it than any recent feature
  has. Verified stale-on-landing:

  | Line | Claim this breaks |
  |---|---|
  | 111 | party "(max 3)" |
  | 127–133 | the whole intrusion key table, incl. `c` = "buff you instead of attacking" |
  | 314 | Fatigue cost framed around commanding a companion |
  | 316 | battle damage formula, now move-driven for the whole party |
  | 504–526 | the entire Companions section, esp. 510 ("fighting alongside you") and 524–526 (`[C]ommand companion`, "with exactly one, it acts immediately") |

  Line 516–519 documents the passive party stat contribution and stays true,
  but its numbers move with `MAX_PARTY_SIZE` (§6).
