# Spend preview: fight figures on the Points screen and the Perks menu (TODO #57, remainder)

**Status:** built in `v0.14.1`; unplayed.

## Intent

`2026-09-24-level-up-summary-design` shipped the level-up page: stats
before → after and a duel against a typical program of the current sector.
Two gaps remain:

1. **0.14.0 hollowed the page out.** A level-up grows no stat by itself any
   more — six stat points are banked and spent on the Points screen
   (`Mode::AllocateStats`). The page's "after" column is read before that
   spend, so it moves only level-based accuracy and evasion, and the Points
   screen shows per-attribute stat deltas with no fight figures.
2. **Perks were out of scope.** Buying one logs "You buy the X perk" and
   nothing says what it changed.

Decided in conversation:

- **Points screen:** the duel block, now → with the pending spend,
  recomputed on every keypress. On both routes in (level-up `Enter`, Perks
  `S`). Not on the creation wizard's Points step: there is no sector yet.
- **Perks menu:** a preview of what one more level of the highlighted perk
  changes.
- **After buying a perk:** a read-back page, the level-up page's shape.
- **The preview is the real computation**, not a delta added to a
  snapshot — approach 1 below.

## Why not add a delta

Attribute points do not always reach combat. Under emulation
`Game::emulated_base` (`game/kit.rs`) replaces ATK and Mitigation with the
emulated species' figures plus gear and the `BoughtStats` receipt only;
Parity/Analysis spent then change nothing in a fight. A preview built as
"snapshot + derived delta" would claim a gain the fight will not pay — and
it would be a second model of how attributes and perks reach
`combatant_profile`, which `CLAUDE.md` forbids ("a call, not a copy").
Threading hypothetical inputs through `combatant_profile`/`effective_atk`/
`effective_mitigation` (the third option) is exact but reshapes code every
fight path calls, for three screens' benefit.

## Engine

### One comparison, three callers

`take_level_up_report`'s duel arithmetic moves into a pure function in
`game/level_up.rs`:

```rust
pub(crate) fn duel_comparison(
    before: &LevelSnapshot, after: &LevelSnapshot,
    foe: battle::Combatant, foe_ehp: f64, zone: u32,
) -> DuelComparison
```

`views::DuelComparison { zone, hit_chance: (f64, f64), per_swing: (f64,
f64), swings_to_win: (u32, u32), swings_to_down_you: (u32, u32) }` — the
four fields leave `LevelUpReport`, which gains `duel: DuelComparison` in
their place (and keeps `zone` only via `duel.zone`). Every figure stays a
call into `battle::hit_chance` / `expected_damage` / `effective_hp` and
`swings_to`, as today.

### Split each commit into its apply core and its outward effects

- `spend_stat_points` → validation + `apply_stat_spend(entity, spend)`
  (writes `Attributes`, `StatPoints`, recompute, the current-HP raise). The
  public method is validation then the core; unchanged behaviour.
- `unlock_perk` → validation, point deduction + `apply_perk_level(player,
  perk)` (pushes to `Perks::unlocked`, `purchase_stat_gain`, receipt,
  recompute) + the log line and `note_deed`. The core carries no log, no
  deed and no cost check.

### Trial and roll back

```rust
fn trial<T>(&mut self, apply: impl FnOnce(&mut Game), read: impl FnOnce(&Game) -> T) -> T
```

Clones the player's `Attributes`, `StatPoints`, `Perks`, `BoughtStats`,
`Stats`, `Decompiler`, `PowerReserve` and `Derived` — every component the
two cores and `recompute_derived` (with `apply_equipment_delta`) write —
runs `apply`, runs `read`, and re-inserts the clones. No recompute on
restore: the clones *are* the recomputed state.

Public API (`Game` is the renderer's only door):

- `Game::preview_stat_spend(&mut self, spend: &[(AttributeId, u32)]) ->
  Option<DuelComparison>` — `None` when there is no player; an unaffordable
  spend is still previewed (the screen already clamps to the pool).
- `Game::preview_perk(&mut self, perk: Perk) -> Option<PerkPreview>` —
  `PerkPreview { stats: Vec<StatRow>, duel: DuelComparison }`, stat rows
  Max HP / ATK / Mitigation that moved (`take_level_up_report`'s filter).
  Previewed regardless of Perk Points, so an unaffordable perk still
  reads what it would do.
- `Game::buy_perk(&mut self, perk: Perk) -> Result<PerkReport, String>` —
  snapshot, `unlock_perk`, snapshot, compare.
  `PerkReport { name, level, description, preview: PerkPreview }`.
  `unlock_perk` stays `pub` for its other callers.

"Before" is always `snapshot_player` of the live state; "after" is
`snapshot_player` inside the trial. Both go through `duel_comparison`.

### The risk, and the test that holds it

A future change to `recompute_derived` that writes a component not on the
trial's list would leak a preview into the game. **Test:** for a spend and
for each perk, the game's whole save taken before and after a preview is
identical — through the real save path (`Game::save` to a per-test temp
file if there is no in-memory builder), not a hand-picked field list, so a
new persisted component is covered without editing the test. Mutation check: drop one
component from the trial's list; the test must fail.

## App-core

- **Points screen:** `App::allocation_duel: Option<DuelComparison>`, one
  writer, `App::refresh_allocation_duel`, called from
  `open_stat_allocation` and after every spend key. `None` on the creation
  wizard's Points step (it never calls it).
- **Perks menu:** `App::perk_previews: Vec<Option<PerkPreview>>`, indexed
  as `perk_defs()`. One writer, `App::refresh_perk_previews`, called by an
  `App::open_perks` that every entrance to `Mode::Perks` routes through
  (the group menu, `leave_allocation`, `LevelUp`'s `P`, the respec
  confirm, the purchase page), so no route shows a stale list. The renderer reads the entry at `menu_selected`.
- **Purchase page:** `Mode::PerkBought` with
  `App::pending_perk_report: Option<PerkReport>`. `handle_perks_key` calls
  `buy_perk` instead of `unlock_perk`; success opens the page, failure
  reports as today. `Enter` or `Esc` returns to `Mode::Perks` with
  `menu_selected` kept, so buying several levels is key-page-key.
  Registered with `Mode::LevelUp` in `needs_status_banner` and the
  all-modes lists.

## Gui

- `level_up.rs`'s `duel_lines` takes `&DuelComparison`, becomes
  `pub(super)`, and is the one drawer of the four rows on all three
  screens (its heading line included: "AGAINST A TYPICAL SECTOR N
  PROGRAM").
- **Points screen** (`points.rs`): the heading and four rows below the
  attribute rows, above the footer.
- **Perks menu** (`progression.rs`): **one line**, not a block — the popup
  has no scroll and "a row here is a row the eighteenth perk loses". Under
  the instruction line, for the highlighted perk:
  `Next level: ATK 14→16 · hit 71%→71% · per swing 9.4→10.1 · win in 6→5 ·
  down in 9→9`. A perk that moves no fight figure reads
  `Next level: no change against a typical sector N program`.
- **Purchase page:** new `render/perk_bought.rs`, `level_up.rs`'s panel
  shape: "NAME — LEVEL L", stat rows, the duel block; for a perk with no
  fight change, its description instead of the block. `[Enter] Back to
  perks`.

Fit is held by census tests, as `the_widest_report_fits_its_screen` does
today: the widest Points screen with the block, the Perks menu with the
preview line at its full perk count, and the widest purchase page.

## Testing

- Engine: `duel_comparison` extraction — existing level-up report tests
  pass unedited (after the field move). Preview equals the outcome: for a
  spend and for each stat perk, `preview_*` then the real commit, the
  commit's report matches the preview. Emulation: a Parity/Analysis spend
  under `Kit::Emulated` previews no ATK change. The save-identity test
  above.
- App-core: the Points screen's figures move on a spend key and are `None`
  on the creation Points step; Enter on the Perks menu opens
  `Mode::PerkBought` and `Enter` returns with `menu_selected` kept.
- Gui: the three census fit tests.
- `balance_sim` untouched — no constant, species or item moves.

## Out of scope

Companion previews; the creation wizard's Points step; the Perks step of
creation; persisting an unshown purchase page across a quit; the typical
foe filtered by biome (`2026-09-24`'s reason); Striker second swing
(`snapshot_player`'s doc: the page counts swings, not rounds).
No save-format change.
