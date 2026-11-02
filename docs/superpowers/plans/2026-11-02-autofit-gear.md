# Autofit gear: plan

**Spec:** `docs/superpowers/specs/2026-11-02-autofit-gear-design.md`. Read it
first; it decides the behaviour. This plan only says where the work goes.

**Worktree:** `.claude/worktrees/autofit-gear`, on branch
`worktree-autofit-gear`.

**Rules for every phase:**

- TDD: write the failing test first.
- Commit at each green step.
- Run `cargo fmt` and `cargo clippy --workspace --all-targets` before each
  commit.
- Never push.
- Never `git checkout`, `git stash` or `git add -A`. Stage explicit paths.
- Load these seam rules before touching their areas:
  `.claude/rules/seams-items.md` (gear) and `seams-screens.md` (refusals and
  ticks).

## Phase 1: Engine (`crates/engine`)

### Files

- **New `src/game/autofit.rs`.** Register it in `src/game/mod.rs`. It holds
  the gather/decide/apply code for this feature, following the
  functional-core rule in CLAUDE.md.
- **`src/game/crafting.rs`.** Split `Game::equip` (around line 1068):
  - `pub(crate) fn equip_quietly(&mut self, wearer, copy: &GearCopy) ->
    Result<(), String>` takes everything from the `slot_occupant` check
    through the `Equipment` insert. It takes the copy out of the pack,
    applies both deltas and sets the slot.
  - `equip` keeps the battle and game-over guard and `check_wearer`, then
    calls `equip_quietly`, builds its notes and log line, and ticks.
  - The outward behaviour of `equip` must not change, so the existing
    equipment tests must pass with no edits.
- **New `src/tests/autofit.rs`.** Register it in `src/tests/mod.rs`.
  `tests/equipment.rs` is already 2.2k lines, so don't add to it.

### Interfaces

```rust
pub struct AutofitReport { pub equipped: Vec<(EquipmentSlot, GearCopy)>, pub message: String }
impl Game { pub fn autofit(&mut self, wearer: Entity) -> Result<AutofitReport, String> }

// pure, in autofit.rs, no &Game / &World / RNG
pub(crate) struct Candidate { copy: GearCopy, power: Option<i32>, decompiler: i32 }
pub(crate) struct SlotInput { slot: EquipmentSlot, worn: Option<(Option<i32>, i32)>, // (power, decompiler)
                              candidates: Vec<Candidate> }        // pre-sorted, stable
pub(crate) fn pick_autofit(slots: &[SlotInput], is_player: bool) -> Vec<(EquipmentSlot, GearCopy)>
```

The field shapes are up to the implementer. The pure/impure split is fixed.

### Gather (`&self`)

- **Refusals first**, before anything else, and the same as `equip`'s: game
  over, an active battle, and `check_wearer`.
- **Candidates** are the rows of `player_status().inventory` whose item
  `equipment_of` assigns to the slot. That list covers both stores, the same
  source `equip_swap_rows` uses. Skip any copy where `is_broken(copy)` is
  true (`game/wear.rs`).
- **Ratings use the one rating function, at real levels:**
  - The rating is `.total` of `gear_power::rate_against(mods,
    &ReferenceFoe::median(species_db))`. Build the `ReferenceFoe` once per
    call.
  - A candidate's `mods` is `copy_bonus(copy, ZoneLevel)`.
  - The worn copy's `mods` is `worn_bonus(&worn)`.
  - Do **not** use `copy_power`: it rates everything at a fixed reference
    zone. The spec explains why.
- **`decompiler`** comes from the same `mods`.
- **Candidate sort order:** sort by a deterministic key, for example the
  name from `copy_name`, then tier, quality, wear and rarity. The sort is
  what decides which of two equal-rated candidates wins.

### Decide (pure)

**Ranking key per slot:**

| Wearer | Slot | Key |
|--------|------|-----|
| Player | Module | `(decompiler, power or MIN)` |
| Player | Weapon, Armor | `power`, with `None` ineligible |
| Companion | Any | `power`, with `None` ineligible; `decompiler` is ignored |

- A worn piece with `power` of `None`, or an empty slot, ranks below any
  eligible candidate.
- On the player's module, an empty slot ranks as `(0, MIN)`.
- Pick the first candidate with the maximum key. Emit it only if its key is
  strictly greater than the worn piece's key.

### Apply (`&mut self`)

- Call `equip_quietly` for each pick in `EquipmentSlot::ALL` order.
- If anything changed:
  - Log one line. For the player it is `Autofit: you equip X, Y.` and for a
    companion `Autofit: {entity_label} equips X.`. Names come from
    `item_name`.
  - Then call `self.tick()` exactly once.
- If nothing changed:
  - The message is `Autofit: already wearing the best gear.`
  - Log it, and do not tick.

### Tests (intent)

**`pick_autofit` units:**

- Picks the highest rating.
- A tie with the worn piece keeps it.
- An equal-rated pair of candidates resolves deterministically.
- An unrated worn piece loses to any rated candidate.
- An empty slot gets filled.
- Player's module:
  - A higher-decompile module beats a better combat module.
  - When decompile chance ties, combat rating wins.
  - A decompiler-only module is eligible.
- A companion:
  - Ignores decompile chance: a module with a higher rating but lower
    decompile chance wins.
  - Never takes a decompiler-only (unrated) module.
- The player's weapon and armor ignore decompile chance.

**`Game::autofit` integration** (prefer a `dev-saves/` template or the
`tests/support/` fixtures):

- Ticks once when something changes, and does not tick when nothing does.
  Read the turn/tick counter.
- Writes exactly one log line.
- The swapped-out copy returns to the pack.
- Refused during a battle.
- Refused for a program the player does not own.
- A companion's swap draws from the player's pack.
- A broken copy is never equipped.

**Mutation check:** break the strict `>` to `>=`, and remove the
companion's decompile-ignore. Each must make a test fail.

### Gate

- `cargo test -p feral-processes-engine autofit equipment`
- `cargo test -p feral-processes-engine balance_sim` must be unchanged.

## Phase 2: App-core (`crates/app-core`)

### Files

`src/app/inventory.rs` and `src/app/party.rs`. Add tests to
`src/tests/inventory.rs` and `src/tests/party.rs`.

### The keys

- **Inventory page:** in `handle_inventory_key`, next to the `U` branch,
  `GameKey::Char('A')` calls `game.autofit(player)`.
  - `Ok(r)` sets `self.status_line = Some(r.message)`, the same as the
    fuse-all confirmation.
  - `Err(e)` calls `self.refuse(e)`.
  - Uppercase only: the page filter takes lowercase letters.
- **Companion equip page:** in `handle_companion_equip_key`, handle
  `GameKey::Char('A')` **before** `selected_index` is resolved. It fits
  `pending_equip_program`, with the same `Ok`/`Err` handling.

### Tests

- `A` on each page reaches the right wearer: the player's slot or the
  companion's slot changes.
- Lowercase `a` on the Inventory page still goes to the filter.
- A refusal reaches the status line.

### Gate

`cargo test -p feral-processes-app-core inventory party`

## Phase 3: GUI and copy

### Files and changes

- `crates/gui/src/render/base_report/frame.rs`: in the `Mode::Inventory`
  keybar, add `keycap("A", "autofit")` after `U`.
- `crates/gui/src/render/party.rs`, `draw_companion_equip`: add
  `"[A] autofit"` to the `key_hints`.
- Run `rg -i "fuse all" assets/help` and add an `A` line wherever the
  inventory keys are listed. Do not touch the manual or the README.

### Gate

- Check that the hint row and the keybar don't overflow. See the
  `popup-row-width-is-testable-headlessly` memory and the
  `gui-text-never-clips-or-wraps` memory. A width test should already cover
  them.
- `cargo test --workspace`, with `FERAL_DEV_NO_SIEGES` unset.
- `cargo clippy --workspace --all-targets`.

## Finish

- An opus whole-branch review against the spec. Send the diff as a file.
- Then the user decides on `deploy`. The CHANGELOG entry and the version
  bump happen there, not on the branch.
