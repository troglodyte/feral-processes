# Autofit gear

**Status:** Approved in chat 2026-11-02.

## Intent

Choosing gear for each slot of every wearer by hand is tedious once the pack
holds dozens of copies. `[A]` fits the best gear in the pack onto one wearer:
the player from the inventory page, or a companion from that companion's equip
page. Decompile chance only does anything on the player
(`components::Decompiler` is player-only), so a companion's choice ignores it.

## Behaviour (user-decided)

- **One wearer at a time.** There is no party-wide fit.
- **Player:**
  - Weapon and armor: the highest power rating.
  - Module: the highest `decompiler` value first. When two modules have the
    same decompile chance, the higher combat rating wins. If no candidate has
    any decompile chance, the module is the best combat-rated one.
- **Companion:**
  - All three slots use the power rating only. `decompiler` is ignored.
  - A module that has decompile chance plus combat or defence stats can be
    picked, but only on the strength of its combat or defence stats.
  - A module with no combat rating (decompiler-only) is never picked.
- **Power rating** is the existing `ItemPower.total`, the figure the gear
  screens already show. It is not a new formula.

## Behaviour (my calls, agreed in chat)

- **Pack only.** Candidates come from the player's cargo (`Inventory` plus
  `GearCopies`, the same source `equip_swap_rows` uses). Autofit never takes
  gear off another wearer.
- **Strictly better.** A slot changes only when the best candidate outranks
  what is worn. On a tie the worn copy stays on, so nothing churns and no
  turn is spent. Equipping does not change `GearCopy.wear`.
- **Never broken.** A broken copy is never a candidate.
- **Unrated ranks lowest (builder call).** A worn piece with no combat rating
  (`rate` returns `None`) loses to any rated candidate on the rating axis.
  That covers an empty slot, a broken worn copy, and a decompiler-only module
  on a companion. On the player's module the decompile chance is still
  compared first, so a worn decompiler-only module keeps its place unless a
  candidate has more decompile chance.
- **The same item at a higher level counts as an upgrade (builder call).** A
  spare copy rated at today's zone can beat the identical copy worn since an
  earlier zone, and autofit swaps it in. That is a real stat gain, and it is
  the same thing the player could already do by hand from the swap list. It
  counts as an equip in the report.
- **Decompile chance only steers the player's module slot.** On weapon and
  armor the player is rated like anyone else.
- **Levels.** A candidate is rated at the current `ZoneLevel`, which is the
  level `equip` will give it. A worn copy is rated at its own `level`. The
  seam rule keeps these two levels separate on purpose. `copy_power` rates
  everything at a fixed reference zone, so it cannot compare the two.
  Autofit therefore rates both through `rate_against(copy_bonus(copy,
  level), &ReferenceFoe::median(..))`. That is the same rating function at
  each copy's real level. The worn side goes through `worn_bonus`, so it
  matches what `equip` subtracts.
- **Cost.** One autofit costs one `tick()` in total, however many slots
  change. If nothing changes, it costs nothing.
- **Feedback.**
  - One log line names what went on, for example `Autofit: you equip X, Y.`
    or `Autofit: Rex equips X.`.
  - If nothing changed: `Autofit: already wearing the best gear.`
  - Per-item equip lines are not logged as well.
- **Refusals** match `equip`: during a battle, after game over, or when the
  wearer is not the player or one of the player's programs (`check_wearer`).
  The refusal is a `Result::Err` string shown the way equip refusals already
  are.

## Shape

- **Engine.** This is a new pure decide step, following the functional-core
  rule in CLAUDE.md:
  - **Gather.** For the wearer, collect each slot's worn rating and the
    candidates. A candidate is the copy, its slot, its rating at the current
    zone, and its `decompiler`.
  - **Decide.** `fn pick_autofit(input, is_player) -> Vec<GearCopy>` is pure
    and takes no RNG. It returns the copies to put on. Ties break on a stable
    order so the result is deterministic, and are tested.
  - **Report.** `AutofitReport` holds the list of
    `(EquipmentSlot, GearCopy)` that went on. The log line uses item names
    only, with no level, tier or rarity notes.
  - **Apply.** `Game::autofit(wearer) -> Result<AutofitReport, String>`
    equips through the existing equip internals without a per-item tick or
    log line, then ticks once and logs once. Factor the body of `equip` into
    a non-ticking, non-logging inner function that both `equip` and
    `autofit` call. Do not copy it.
- **App-core.**
  - `[A]` in `handle_inventory_key` fits the player.
  - `[A]` in `handle_companion_equip_key` fits `pending_equip_program`. It
    must be handled before that handler resolves `selected_index`.
  - The result shows as the log line. An `Err` goes through the existing
    message path.
- **GUI.** Add `[A] Autofit` to the key hints on the inventory page and on
  the companion equip page. Do not change the layout.

## Non-goals

- No party-wide fit and no taking gear from other wearers.
- No role or class weighting.
- No preview or confirm step; the swap list is still there for manual
  choices.
- No change to `copy_power`, `ItemPower` or the swap-row sort.

## Tests (intent)

- **Decide-step units:**
  - Highest rating wins per slot.
  - A tie keeps the worn copy.
  - Broken copies are skipped.
  - The player's module goes to the highest decompile chance, and a tie on
    decompile chance goes to combat rating.
  - A companion ignores decompile chance: a higher-decompile module with a
    lower rating loses.
  - A companion never takes a decompiler-only module.
- **`Game::autofit`:**
  - It ticks exactly once when something changes and not at all when nothing
    does.
  - It writes one log line.
  - Swapped-out gear returns to cargo.
  - It is refused in battle.
- **App-core:** `[A]` on each page reaches the right wearer.
- `balance_sim` is unaffected (no tuning change), so a spot-check is enough.
