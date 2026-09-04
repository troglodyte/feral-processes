# Items, gear and economy

- **An item's price is bounded twice, and the second bound is the
  non-obvious one.** A craftable worth more than its ingredients is an
  infinite Credit loop; a `work.produces` structure makes its item out of
  *nothing* on a timer, so that item's value is really a Credit-per-tick rate
  the recipe ceiling cannot see. The second bound is currently slack, not
  gone. Both are asserted over the real assets.
- **A zone's material is a content decision, and two censuses are what hold
  it.** Nothing in `ItemDef` says Cache Grain is what zone 2 pays you, so
  `ZONE_MATERIALS` in `tests/assets.rs` plus
  `every_zone_gated_gear_recipe_asks_for_a_zone_material` and
  `every_upgrade_path_asks_for_a_zone_material` are the whole rule. The
  upgrade half is free in zone 1 because `upgrade_ceiling` is
  `min(max_tier, zone)` and a structure deploys at tier 1. The gear half
  rides the **research** file, where `min_zone` and `unlocks_recipes` are
  already one edit; on `ItemDef` instead it would split the gate from the
  recipe and collide with `scavenged_gear_stays_benchless_and_fragment_only`.
  Cache Grain **crosses a breach** (no `role`, and `enter_next_zone` wipes
  only the two currencies); a new tier must never retire the one below it.
  Fixtures stock through `stock_upgrade_materials`.

- **A carried copy of gear is one value, `items::GearCopy`**, and `Inventory`
  is by definition the *plain-copy* store. `GearCopy::is_plain` decides which
  store and exactly three functions ask it. Every entry point naming an item
  takes the whole copy. Nothing puts a player's copy into a `Stock`, so the
  machine half is unreachable rather than untested.
- **`Game::copy_bonus` is the one expression for what gear is worth, and the
  order of its axes is load-bearing**: `scaled_for_level`, quality,
  `fused_for_tier`, `for_rarity`, over a base the affix has already been added
  to. Two carry a per-step floor and a floor does not commute with a
  multiplier. Sharing the *formatter* was never enough — four screens rebuilt
  the chain by hand and all four dropped the affix at once. `copy_bonus` is
  `pub` and the axis methods `pub(crate)` so the hand-rolled chain now fails
  to compile.
- **A copy's quality is a fourth axis and an integer.** `GearCopy` is the
  `GearCopies` ledger's key and `EquippedItem` holds the same key, so an
  `f32` takes `Eq` with it. Its `serde` default is the *named function*
  `default_quality`, never a bare `#[serde(default)]` — `u8`'s own default
  is 0, which loads every existing save's gear at 0% of its bonus. Worn
  gear is four more flat save fields, not a nested copy, so the field can
  be forgotten in four places at once. In `copy_bonus` it sits **third**,
  after `scaled_for_level` and before the two floored axes: it carries no
  floor of its own, so applied last it is eaten by rounding and can invert
  the rare tiers on a 4-point stat.
- **`Game::roll_quality` is the one formula and the one clamp**, and it sits
  beside `roll_gear_rarity` because two files roll the same axis: a drop
  passes the flat `QUALITY_DROP_BASE`, crafting a floor it builds. The
  spread is drawn **in steps** of `QUALITY_STEP`, never drawn fine and
  rounded — that halves the two end buckets. It is the **third** roll in
  `grant_gear_drop` and last on purpose, so a seeded copy's tier and affix
  are where they were, and it sits below the non-equippable early return so
  a material still spends **no** draw. The trap is that an equipment drop
  fails `is_plain` and so lands in `GearCopies` rather than stacking in
  `Inventory` — a fixture reading the old store is the fixture's bug.
- **`CraftOrder` is a struct at one implementor and the second is named**,
  a base-roster program compiling at a bench. The axis of change is *who is
  compiling, and where*; the four terms of `craft_quality_floor` are addends
  in one expression and never an axis. **The perk term is what the type is
  for**: `Perk::TightenTolerances` is gathered by `player_craft_order`, not
  read inside the floor, because a program compiling at a bench has none of
  its own. It is read at the compile rather than applied at purchase, and
  priced at one `QUALITY_STEP`. Player *level* is deliberately not a term:
  `scaled_for_level` already scales gear to its wearer. The floor saturates
  rather than wrapping, and the clamp stays `roll_quality`'s alone.
  `best_structure_tier` reads the **best** deployed bench, and a structure
  with no `StructureTier` is **tier 1**. **The trap is that the term was
  inert on shipped content**: every craftable-equipment recipe names the
  Fabricator or the Armory and neither could be upgraded, which is why both
  now carry an upgrade path that buys nothing but better gear.
- **The careful surcharge is applied in `craft_cost`, and all three price
  questions take the flag.** Discount **then** surcharge, rounded up, or a
  fully perked recipe with every line floored at 1 is careful for free. A
  quoted maximum off the plain price is a batch the compile refuses, so
  `[M]` reads `max_craftable(careful)` — a test that only checks the two
  sides *differ* passes against that bug. The toggle is cleared when the
  quantity page **opens**, so it cannot outlive its batch.
- **A compile rolls per unit, and a copy at exactly spec still stacks.** A
  batch is a spread to compare, not N of one thing; a copy that rolls
  `QUALITY_DEFAULT` is plain and lands in `Inventory`, so a test counting a
  batch must read **both** stores. Non-equipment spends **no** draw, which
  is now read against *the same span of bare ticks* rather than against a
  batch of a different size: a compile's clock cost scales with the batch,
  so one unit against five no longer isolates the draw.
- **The swap row's stat column is a tag, not part of its head.**
  `wrapped_row_lines` never breaks the head, and the quality figure's seven
  cells put the joined form 35.6px past a 1243.2px popup body, lost in
  silence. The padding lives *inside* the tag or the delta slides out of its
  column, and the unequip row carries a padded **blank**, since an empty tag
  is skipped by design.
- **The category tag is a column on the row, not a substring of it.**
  `Row::Item::tag` carries the `WEP`/`ARM`/`MOD` token *and the lead in
  front of it*, so `draw_row` lays a row out as three `ui_runs` pieces and
  no row moves. `row_lead` is the one definition of the columns before it
  and `item_text` the one join — measure a row without the column and
  `suffix_x` drops its suffix on the row's own tail, or a wrap budgets for a
  row narrower than it draws. The ramp is **emphasis, not hue** and is
  monotone (gray, default, bold, gold), and the as-designed band is
  literally no change. A row naming something no copy exists of yet — a
  recipe's result, a trader's stock — passes `None`.
- **`Game::copy_power` is the one door to a gear rating, and every term in it
  is a *call*.** `Stats::power` for attack and mitigation, `battle::hit_chance`
  for accuracy and evasion — a probability is not a quantity and is priced as
  the fraction it moves the throughput it acts on, never summed into the
  total. The band is a **difference** against `POWER_REFERENCE_DAMAGE`,
  because a weapon *overrides* the natural attack: written as a sum, a weapon
  worse than bare fists rates positive. `decompiler` gets no term. The rating
  is **absolute**, priced against one reference wearer in `tuning.rs` that is
  *derived* from `balance_sim`'s swept curve and `stats_after_levels`, never
  invented — so **the swap picker's delta may legitimately disagree with it**,
  and the fix is not to make the column contextual. Since 0.13.69 the two sit
  on the same row: a candidate can rate above the worn piece and still show a
  negative delta on an axis, which is the two columns answering different
  questions. `None` means "no combat axis", never "rated zero", and two
  censuses over the real assets hold both halves.
- **`PowerCell` has three cells and three meanings.** `Rated(n)` is a rating,
  `Unrated` is an em dash (*no answer*, not a bad answer), `Blank` is a row
  that is not an item. It is a **fifth parameter on `with_tag`**, not a
  defaulted builder, so every call site is made to decide; and it sits
  **between the tag and the name**, inheriting the fixed-width `row_lead`,
  because in `Row::Item::suffix` the figures would stagger with the name
  lengths above them. The gear inspect page paid for its breakdown row out of
  `GEAR_AFFIX_ROW_CAP` — that page has zero headroom and no scroll.
- **The caravan is one basket and `Game::commit_caravan_basket` is the one
  commit door.** Every refusal lands before anything is spent, and **sells land
  before buys** so a basket can be funded by its own sales. The funding test
  asserts the resulting **Credits**, not the outcome: with the order reversed
  the goods still arrive, because `Inventory::take` clamps and the price
  vanishes out of an empty purse. **One tick for the whole commit.** The two
  ceilings mirror the transfer picker — `caravan_sell_available` per row and
  static, `caravan_budget` one budget minus the *other* rows — and an offer
  clamps to `0..=1`. **`Mode::Caravan` is named in the modifier fold at
  `app/input.rs`**, or Shift and Ctrl are folded to bare arrows and silently
  become plain steps. **Right increases and Left decreases** here: the picker's
  inversion is for a signed row and does not apply. `[A]` fills the sell rows
  only.
- **The wagon's grouping lives in `caravan_view`, never in `caravan_shelf`.**
  The shelf is a round-robin whose leading slot rotates per visit; sorting it
  would make that unobservable and open every wagon with a weapon.
  `Game::caravan_group` returns rank *and* heading together so the sort and the
  header cannot disagree, is exhaustive on `CaravanOfferKind`, and `index` is
  handed out before the sort so rows move on screen and no shelf identity moves
  with them.
- **A worn item and a candidate are scaled at two different levels, and that
  is the point.** Gear locks in `EquippedItem::level`; collapsing the two
  hides the case the screen exists for.
- **A copy's name is built in exactly one place, `Game::copy_name`.** Building
  a name in a renderer is what lets a drop line and the next screen disagree
  about what you picked up.
- **An item's extra effects are three lengths of one derivation.**
  `item_blurb` is the crafting menu's two-word gloss, `Game::item_effects` the
  listing screens' one line per effect, `item_grant` the describe page's full
  prose — and the middle one *calls* the last rather than re-reading `grants`.
  `render/inventory.rs::effect_lines` is the one place those become rows. A
  stat bonus is **not** an effect here; it rides `equip_preview_tag` already.
  The trap is units, in opposite directions: `CompanionUpgradeDef`'s
  percentages are percentage *points* (`refactor::raised` divides by 100),
  while `taming_potency` is a 0..1 **base** that `capture_chance` multiplies,
  not an addend.
- **The gear inspect page is one derivation, `Game::gear_detail`, opened
  with `[I]` from every list that names gear.** Every figure is a call —
  `copy_bonus` for the stats, `battle::hit_chance` for the odds,
  `routine_detail` for the grant, which scales a routine's magnitudes for
  its **wearer** through the same helpers the invocation uses. Trigger,
  target and effect are exhaustive matches, `cell_mark`'s rule.
  `item_effects_besides_grant` is a shorter *length* of `item_effects`, not
  a trim of its output. `app-core`'s `GearInspect` is the one subject field
  and `open_gear_inspect` the one writer: copy, wearer and return mode are
  three distinct failures if inherited. **Two traps.** The hit chance is a
  projection against `balance_sim::median_ordinary_species` at the zone
  level — labelled as one, and drawn only for a piece carrying a band or
  accuracy. And **the page has no scroll**: `draw_popup` pages a `Row::Item`
  span and this page has none, so a row past the bottom is dropped in
  silence — `the_tallest_gear_page_fits_its_popup` is what says it fits.
- **An affix is data and its absence is supported.** `Game::roll_affix` spends
  **no** RNG draw on an empty pool. It is **two** rolls, and **independent of
  the rare tier** rather than gated behind it. Affix stats are added to the
  **base**, before all three scaling axes.
- **Rarity is one ladder for programs and gear**, `spawning::rarity_for_roll`.
  `Game::grant_gear_drop` is the one way a copy above `Ordinary` enters the
  game; crafting, buying and buying back are deliberately **not** callers. A
  non-equippable takes an early return and spends **no** draw.
- **Gear fusion has two records of the same tier, and only one is clamped.**
  `GearCopies` is the ledger and is clamped on load through `GearCopies::add`;
  `EquippedItem::fusion_tier` is the *receipt* for a bonus already spent, so
  lowering it makes an unequip subtract less than the equip added.
- **A trader's shelf row is `(GearCopy, qty)`, and the key is not
  decoration** — keyed on the item alone it hands back an ordinary copy for a
  rare one. The unit price is the same at every tier, which is why the key has
  to be exact.
- **`render/mod.rs::fusion_color` and `popup.rs::fusion_row` are the one
  colour rule for anything fused.** Two screens deliberately opt out, because
  a second meaning on the same axis makes both unreadable. `fusion_color`
  returns `Option` so a caller with a louder rule wins.
- **Installing from a disk spends the disk last, and creation is the second
  door into a slot and spends no item at all.** `install_routine` checks
  battle, ownership, knowledge and a free slot before it looks for the disk.
  Uninstalling returns nothing. A new game *knows* `DECOMPILE_ABILITY_ID`,
  and a displaced innate routine is lost. **The trap is that `install_routine`
  is no longer where a rule about routine slots belongs**:
  `abilities::install_starter` writes `KnownRoutines` *and* the slot through
  `Game::write_routine` directly, with no disk, no ownership check and no
  `exclusive` check, so anything added to `install_routine` is bypassed by
  every character made after it lands. What holds the pool's shape instead is
  a census — `every_starter_is_single_target` and
  `every_starter_is_not_exclusive` in `tests/assets.rs` — plus the wizard
  only offering `Game::starter_routine_rows`. Knowledge as well as the
  install is deliberate: it is what lets a starter be etched onto a disk
  later rather than lost when the slot is wanted for something else.
- **Hand-compiling is priced at `Game::hand_craft_ticks` — the cycle of the
  machine that exists to do the job times `HAND_CRAFT_TICK_MULT` — and
  `Game::craft` is that loop drained to completion.** The lookup is the
  `assembles` block naming the item, else the `work` block producing it, else
  `HAND_CRAFT_DEFAULT_CYCLE`, over `StructureDb::all`'s sorted order so a mod
  with two machines for one item resolves the same way every session. **Two
  traps.** A second copy of `mult × cycle` — in the screen that draws the
  progress bar, most obviously — lets the quoted number and the spent number
  drift with nothing failing to compile, which is why `hand_craft_ticks` is
  `pub` and the arithmetic lives nowhere else. And a refusal that lands after
  `begin_hand_craft` has armed `resources::HandCraft` has already spent time
  before refusing: every refusal is checked first, and the resource is
  inserted last. `advance_hand_craft` is the **only** code that spends a
  unit's ingredients or grants one — at the unit's start and end
  respectively, never per batch, so an abort keeps the finished units and
  refunds the one in flight; that is *materials are not spent until the
  structure is raised* again, and it closes the edge where a build crew
  empties the pack (`Source::Pack`) part-way through a compile. **The
  multiplier is 1 and is paired with `app_core::COMPILE_TICKS_PER_SECOND`**
  — the tick price divided by the bar's rate *is* the wall-clock wait, so
  raising this without checking that rate ships a stare nobody measured; it
  was 10 against a bar running at thirty times the world's tick rate, and
  fixing the rate alone would have made one Hardened Shell two and a half
  minutes of watching. At parity what still makes a bench the answer is that
  it runs unattended, burns no player Power, and reaches quality bare hands
  cannot (`QUALITY_BENCH_PER_TIER`). The loop breaks on a game over or a battle opening, exactly where
  `move_player`'s drag ticks break and for the same reason, and treats the
  break as an abort. **A sixth refusal guards the reserve**: the
  ticks a batch spends drain Power at `HUNGER_DECAY_PER_TICK` like any
  others, so one projected to leave less than
  `HAND_CRAFT_POWER_FLOOR` standing is refused *whole* — a batch quietly
  shortened reads as the key half working. The projection **calls**
  `systems::power_drain_per_tick` rather than restating the rate, and is
  blind to a Recharger's trickle on purpose so it is a worst case. The floor
  is a margin and not `POWER_MIN`, or a batch ending at 0.15 starves on the
  next background tick. `max_craftable` carries the same ceiling, the
  careful surcharge's rule again: a quoted maximum the compile refuses reads
  as `[M]` doing nothing. `HandCraft` is **not saved** (`RunFeats`' precedent) and
  is inserted by `begin` rather than at both constructors. The batch is
  announced **once, on the way out**, with the count actually granted — a
  line per unit turns a batch of twelve into twelve rows of log, and a line
  at the start promises units an abort never delivers.

- **`DownedPrograms` is a third player store, and it is not `Inventory`.**
  `Inventory::count`/`::take` read the *first* matching row, which is fine
  for a plain stack and wrong for two kills of different level and rarity —
  merging them into a per-species row would either force an instance rule
  onto every other `Inventory` reader (recipes, `Stock`, hauling, banking)
  or silently collapse two programs that shouldn't collapse. `GearCopies`
  is the precedent for exactly this shape; `DownedPrograms` reads it and
  grants *into* `Inventory` via `grant_loot`, never the reverse.
- **`FIGHT_CONDITION_WEIGHT` ships at `0.0`, and the fight axis is
  structurally inert, not merely zero-weighted.** `apply_damage` clamps HP
  to zero before `award_loot` ever runs, so `Game::overkill_term` reads a
  dead entity's `hp` and is always `0.0` on the real kill path — raising the
  weight off zero today multiplies a term that can never move. Making the
  axis live needs the raw blow threaded from `resolve_attack`'s caller into
  `award_loot`, which nothing in phase 1 does; don't "fix" the weight
  without first checking whether the term does.
- **A downed program's `level` is `Game::ability_user_level`, not the
  player's own level.** Stamping the player's level would measure *when in
  the run* rather than *what was killed*, and since `extraction_yield`
  multiplies `DownedProgram::grade()` (which folds `level` in) against a
  flat base, it would break drop-neutrality by construction. Known gap:
  `ZoneLevel` doesn't move underground, so a Stack kill's depth never
  registers on the program it leaves.
- **`Game::extract_program` is the one door, and its refusals are asserted
  per refusal, not by one path standing for all three.** A single test over
  one refusal (say, game-over) proves nothing about whether the
  out-of-range-index check spends anything before returning its own error —
  `tests/extraction.rs` has one test per refusal, each checking both the
  store and `Inventory` are byte-identical to before the call.
- **`Game::extraction_yield` takes `&self` and draws no `GameRng`, which is
  what lets the screen's preview and the grant agree without coincidence.**
  A version drawing per unit would either spend a draw from a screen that
  grants nothing (corrupting the seeded stream) or quote a distribution
  instead of a figure — either way "3 Core Fragments" previewed and 2 or 4
  granted reads as a bug that was never wrong, just nondeterministic.
- **The drop-neutrality gate is a single point, and `apportion` conserves
  the unit *total* under any weighting** — so a tool's `yields` weights
  have no lever on the gate at all, only which items a fixed count becomes.
  `TOOL_BASE_UNITS`'s own comment names the real band the gate admits
  (`[2.475, 4.125)`, verified empirically): every value in it reads equally
  drop-neutral to the test though the tool feels roughly twice as strong at
  one end as the other in play. The gate also measures per *extraction*,
  not per *kill* as decision 8 words it — `MAX_DOWNED_PROGRAMS`'s cap and
  `tool.ticks`' time cost are both real leakage a play session would catch
  and this test cannot.
- **`apportion` is Hamilton's method (largest-remainder), not a per-unit
  draw**, and it carries the Alabama paradox: with three or more pool
  entries, one extra unit can *shrink* another entry's share rather than
  only ever adding to the total. Both shipped pools have two entries, where
  the paradox cannot occur — a three-item modded pool exhibiting it is the
  method working as documented, not a regression to chase.
