---
paths:
  - "**/items*"
  - "**/gear*"
  - "**/crafting*"
  - "**/commerce*"
  - "**/trade*"
  - "**/caravan*"
  - "**/extraction*"
  - "**/tools*"
  - "**/affixes*"
  - "**/inventory*"
  - "**/rig_tool*"
  - "**/creation*"
  - "**/settlement_market*"
  - "**/stack_market*"
  - "assets/items/**"
---

# Load-bearing seams: Items, gear and economy

One sentence each, the rule alone. The trap is in the `seams` skill; the argument is `seam:<slug>` in the memory graph.

- **An item's price is bounded twice, and the second bound is the
  non-obvious one.** A craftable worth more than its ingredients is an
  infinite Credit loop; a `work.produces` structure makes its item out of
  *nothing* on a timer, so that item's value is really a Credit-per-tick
  rate the recipe ceiling cannot see.
- **A zone's material is a content decision, and two censuses are what hold
  it.** Nothing in `ItemDef` says Cache Grain is what zone 2 pays you, so
  `ZONE_MATERIALS` in `tests/assets.rs` plus
  `every_zone_gated_gear_recipe_asks_for_a_zone_material` and
  `every_upgrade_path_asks_for_a_zone_material` are the whole rule.
- **A research node's material bill may only name what that node's own
  prerequisites can make**, `min_zone` granting nothing and `assembles`
  being no source.
- **A research bill is paid from the pack, topped up off the adjacent
  shelves**, and the whole bill is refused before a unit moves.
- **A weapon's reach lives on `ItemDef` and is authored as an enemy-facing
  plural `AbilityTarget`**, so it is off `copy_bonus`'s four scaling axes by
  construction and each combat model converts it with the converter it
  already has.
- **A carried copy of gear is one value, `items::GearCopy`**, and
  `Inventory` is by definition the *plain-copy* store.
- **`Game::copy_bonus` is the one expression for what gear is worth, and the
  order of its axes is load-bearing**: `scaled_for_level`, quality,
  `fused_for_tier`, `for_rarity`, over a base the affix has already been
  added to.
- **A copy's quality is a fourth axis and an integer.** `GearCopy` is the
  `GearCopies` ledger's key and `EquippedItem` holds the same key, so an
  `f32` takes `Eq` with it.
- **`Game::roll_quality` is the one formula and the one clamp**, and it sits
  beside `roll_gear_rarity` because two files roll the same axis: a drop
  passes the flat `QUALITY_DROP_BASE`, crafting a floor it builds.
- **`CraftOrder` is a struct at one implementor and the second is named**, a
  base-roster program compiling at a bench.
- **The careful surcharge is applied in `craft_cost`, and all three price
  questions take the flag.** Discount **then** surcharge, rounded up, or a
  fully perked recipe with every line floored at 1 is careful for free.
- **A compile rolls per unit, and a copy at exactly spec still stacks.** A
  batch is a spread to compare, not N of one thing; a copy that rolls
  `QUALITY_DEFAULT` is plain and lands in `Inventory`, so a test counting a
  batch must read **both** stores.
- **The swap row's stat column is a tag, not part of its head.**
  `wrapped_row_lines` never breaks the head, and the quality figure's seven
  cells put the joined form 35.6px past a 1243.2px popup body, lost in
  silence.
- **The category tag is a column on the row, not a substring of it.**
  `Row::Item::tag` carries the `WEP`/`ARM`/`MOD` token *and the lead in
  front of it*, so `draw_row` lays a row out as three `ui_runs` pieces and
  no row moves.
- **`Game::copy_power` is the one door to a gear rating, and every term in
  it is a *call*.** `Stats::power` for attack and mitigation,
  `battle::hit_chance` for accuracy and evasion — a probability is not a
  quantity and is priced as the fraction it moves the throughput it acts on,
  never summed into the total.
- **`PowerCell` has three cells and three meanings.** `Rated(n)` is a
  rating, `Unrated` is an em dash (*no answer*, not a bad answer), `Blank`
  is a row that is not an item.
- **The caravan is one basket and `Game::commit_caravan_basket` is the one
  commit door.** Every refusal lands before anything is spent, and **sells
  land before buys** so a basket can be funded by its own sales.
- **`Game::settle_basket` is the commit core both the caravan and a
  settlement call, and the currency charge deliberately stays out of
  it** — each vendor's own closure charges its own currency, held to the
  quoted cost by a test rather than the compiler.
- **The wagon's grouping lives in `caravan_view`, never in
  `caravan_shelf`.** The shelf is a round-robin whose leading slot rotates
  per visit; sorting it would make that unobservable and open every wagon
  with a weapon.
- **A worn item and a candidate are scaled at two different levels, and that
  is the point.** Gear locks in `EquippedItem::level`; collapsing the two
  hides the case the screen exists for.
- **A copy's name is built in exactly one place, `Game::copy_name`.**
  Building a name in a renderer is what lets a drop line and the next screen
  disagree about what you picked up.
- **An item's extra effects are three lengths of one derivation.**
  `item_blurb` is the crafting menu's two-word gloss, `Game::item_effects`
  the listing screens' one line per effect, `item_grant` the describe page's
  full prose — and the middle one *calls* the last rather than re-reading
  `grants`.
- **The gear inspect page is one derivation, `Game::gear_detail`, opened
  with `[I]` from every list that names gear.**
- **An affix is data and its absence is supported.** `Game::roll_affix`
  spends **no** RNG draw on an empty pool.
- **Rarity is one ladder for programs and gear**,
  `spawning::rarity_for_roll`.
- **Gear fusion has two records of the same tier, and only one is clamped.**
  `GearCopies` is the ledger and is clamped on load through
  `GearCopies::add`; `EquippedItem::fusion_tier` is the *receipt* for a
  bonus already spent, so lowering it makes an unequip subtract less than
  the equip added.
- **A trader's shelf row is `(GearCopy, qty)`, and the key is not
  decoration** — keyed on the item alone it hands back an ordinary copy for
  a rare one.
- **`render/mod.rs::fusion_color` and `popup.rs::fusion_row` are the one
  colour rule for anything fused.** Two screens deliberately opt out,
  because a second meaning on the same axis makes both unreadable.
- **Hand-compiling is priced at `Game::hand_craft_ticks`, the cycle of the
  machine that exists to do the job times a constant, and `Game::craft` is
  that loop drained to completion.**
- **That constant and `COMPILE_TICKS_PER_SECOND` are read together**, because
  the tick price divided by the bar's rate is the wall-clock wait.
- **Installing from a disk spends the disk last and refunds nothing on the
  way out; creation is the second door into a slot and spends no item at
  all**, `abilities::install_starter`.
- **A hand-compile's ticks drain Power like any others, and a batch
  projected to leave less than `HAND_CRAFT_POWER_FLOOR` is refused whole
  rather than shortened** — `max_craftable` carries the same ceiling.
- **`DownedPrograms` is a third player store beside `Inventory` and
  `GearCopies`, and it is not `Inventory`.** `Inventory`'s `count`/`take`
  read the *first* matching row, which is what lets recipes, `Stock`,
  hauling and banking treat it as a plain-copy store with no instance rule;
  a downed program's species, level, rarity, boss flag and condition make
  it instanced, so it needed the third store rather than a new rule bent
  into the first one.
- **`Game::extract_program` is the one door a downed program is spent
  through, and every refusal lands before anything is spent** — asserted
  per refusal, since a single test over one path passes against the others.
- **An extraction rolls *how many* units and never *which*** — one
  `GameRng` draw on `Game::extraction_band` (a `DamageRange`, so a
  zero-width band still draws), then `apportion`'s fixed split in the pure
  `Game::extraction_yield`; the preview names `extraction_items` without
  counts and the rig gates on the band's `max`.
- **The starter tool's knowledge is derived, never stored** — `knows_tool`
  answers true for `tuning::STARTER_TOOL_ID` and `tool_rows` unions the same
  helper, which is what makes pulling it recoverable and repairs saves
  already written.
- **A synthetic item id needs a minted `ItemDef` behind it**,
  `ItemDb::synthesise_tool_carriers` beside `synthesise_etched_disks` —
  `item_name`'s raw-id fallback means a missing one is silent.
- **A tool carrier is sold everywhere and bought nowhere**, filtered through
  `ItemId::tool_id` in `caravan::stock_pool` and `ItemDb::creation_shelf`.
- **`extraction_band` and `extraction_ticks` read the bench tier
  themselves; there is no `structure_tier` parameter.** Each has two
  callers — the preview or the rig's gate, and the act — and an argument is
  the crack a quoted figure and a granted one would differ through.
- **A standing extraction bench buys time; upgrading it buys materials.**
  The yield term is `bench_tier - 1` (a never-upgraded structure reads as
  tier 1, so a `tier` term would pay a full `TOOL_TIER_SCALE_STEP` for
  merely having built a Compiler); the tick term is the full tier, as a
  divisor floored at one. Neither is a gate — extraction works in the
  field, at the base and in the Stack alike.
- **`Game::take_routine` is the one place a routine comes off a program**,
  shared by `extract_routine` (a tamed program) and a `Routines`-category
  tool through `extract_program` (a downed one). The effect is shared; the
  refusals are not — the tamed door needs a bench and an ownership check,
  the tool door needs neither. The exclusive branch popping the disk
  instead of teaching is what keeps exactly one copy in a run.
- **A downed program's routine pool is derived from species and level**, so
  it cannot offer what that individual was actually carrying, and no
  shipped species declares an exclusive routine — the Reader's exclusive
  branch is implemented and unreachable in shipped content. Not dead code.
- **A sortie banks its downed programs and delivers them in
  `return_sortie`**; an off-screen battle never writes the player's store.
  The delivery loop stops at the first refusal, because once the store is
  full it stays full and `message_history` condenses repeats — a log-entry
  count cannot tell "said once" from "said eight times".
