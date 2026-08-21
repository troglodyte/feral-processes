# Item quality — Phase 3: crafting rolls it

> Roadmap: `2026-08-21-item-quality-plan.md`. Spec:
> `../specs/2026-08-21-item-quality-design.md`. The roadmap's **Global
> Constraints** and **Gates** sections apply to every task below and are not
> repeated here.

**Deliverable:** a compiled copy rolls its quality from a floor the player
builds — bench tier, the careful toggle — instead of arriving at a flat 100.
`CraftOrder` is the seam that gathers the floor's terms, so the follow-up
feature (a base-roster program compiling at a bench) has a second gatherer to
write rather than a formula to copy.

This is the phase the design intent lives in: **a developed base out-produces
the world.** It is also the phase to *play* before calling the numbers
correct.

## Decisions taken at the start of this phase

Two numbers the spec left open, and one thing the spec assumed that the
shipped assets do not have.

1. **Careful compile costs +50% of every ingredient, rounded up.**
   `QUALITY_CAREFUL_COST_PERCENT = 50`, applied to the *charged* cost — i.e.
   after `Perk::LeanCompiler`'s discount, which already floors each line at
   1. The rounding is `div_ceil`, so a 1-unit ingredient costs 2 and the
   toggle is never free. Chosen over doubling (too steep on the 14-fragment
   recipes) and over a flat `+1` (which inverts the pressure as recipes
   grow).

2. **`QUALITY_BASE = 80`, as specced.** A fresh player's craft is 80–100,
   which is a real early-game nerf against today's flat 100. That is the
   intent — the bench and the toggle are what buy it back — and the spec's
   softer variant (90) stays available if a session says otherwise.

3. **The two gear benches gain an upgrade path.** `fabricator` and `armory`
   are the benches all 25 shipped craftable-equipment recipes name, and
   *neither declares `upgrade`*, so neither ever carries `StructureTier` and
   the bench term would read 0 for every recipe in the shipped game. A term
   that is inert on shipped content is a half-finished feature, so this phase
   adds `upgrade:` to both, copying the six existing paths' shape
   (`max_tier: 5`, fragments + one zone material, which is what
   `every_upgrade_path_asks_for_a_zone_material` requires). Tier does nothing
   else for a structure without a `ResourceNode` — `resolve_gather_cycle` is
   its only other reader — so upgrading a bench buys quality and nothing
   else, which is exactly the purpose the spec wanted a bench upgrade to
   have.

**The perk term is Phase 4's**, so `CraftOrder` ships with two fields here
and gains `perk_level` with the variant that feeds it. A field that is always
0 is the dead flag this repo's principles forbid.

## Fallout to expect, not to fix

- **Crafted equipment stops stacking**, per the spec — five compiled blades
  are up to five rows in `GearCopies`, not one `Inventory` line. Any fixture
  asserting `Inventory` count after crafting a *piece of equipment* is
  reading the wrong store now; that is the fixture's bug, the same call
  Phase 2 made about drops. `inventory_used` already sums both stores.
- **Three signatures change** — `craft`, `craft_cost`, `max_craftable` all
  take `careful`. Every caller fails to compile, which is the point: a screen
  quoting one price while `craft` charges another is the exact bug
  `craft_cost`'s doc comment records.
- Seeded tests that craft equipment now consume a `GameRng` draw per unit and
  may move. A material still spends **no** draw.

## Task 1: The floor's terms

**Intent:** the constants and the one lookup the bench term needs, with
nothing calling them yet.

- [x] **Step 1: Write the failing tests.** In `crates/engine/src/tests/`:
      `best_structure_tier` is `None` with no such structure deployed, `1`
      for a deployed structure that carries no `StructureTier` (the
      fabricator before its upgrade path lands), and the **highest** of
      several deployed ones. Use `support.rs`'s existing structure fixture —
      it already takes a tier.
- [x] **Step 2: Run them and watch them fail.**
- [x] **Step 3: Add the constants** to `tuning.rs`'s quality section, beside
      `QUALITY_DROP_BASE`: `QUALITY_BASE`, `QUALITY_BENCH_PER_TIER`,
      `QUALITY_CAREFUL_BONUS`, `QUALITY_CAREFUL_COST_PERCENT`. Each gets the
      doc comment convention that section already uses — what it is, and the
      argument for the value.
- [x] **Step 4: Add `Game::best_structure_tier`** to `game/catalog.rs`,
      beside the other deployed-structure lookups. `Option<u32>`: `None` when
      no structure of that kind stands, otherwise the max tier, defaulting a
      tier-less structure to 1. Its doc says why tier-less reads as 1.
- [x] **Step 5: Run the tests, mutation-check them** (delete the `max`, watch
      the multi-structure test fail; delete the `unwrap_or(1)`, watch the
      tier-less one fail), **and commit.**

## Task 2: `CraftOrder` and the quality floor

**Intent:** the crafter seam and the floor expression, still uncalled.

- [x] **Step 1: Write the failing tests.** The floor rises by
      `QUALITY_BENCH_PER_TIER` per tier **above 1**; a recipe naming no bench
      is the same as a tier-1 bench; careful adds `QUALITY_CAREFUL_BONUS`; a
      developed order's floor may legitimately exceed `QUALITY_MAX` and the
      clamp in `roll_quality` is what holds it (the roadmap's one-clamp
      rule).
- [x] **Step 2: Run them and watch them fail.**
- [x] **Step 3: Carry the bench on `CraftRecipe`.** A
      `requires_structure: Option<String>` field in `views.rs`, filled by
      both arms of `craft_recipes` (the `craftable` half and the researched
      half). Without it `player_craft_order` would have to re-resolve the
      recipe out of two databases.
- [x] **Step 4: Add `CraftOrder` and its two writers** in
      `game/crafting.rs`, beside `craft`: the `pub(crate)` struct
      (`bench_tier`, `careful`), `Game::player_craft_order(recipe, careful)`
      which gathers it, and `Game::craft_quality_floor(&CraftOrder) -> u8`
      which is the one expression of `QUALITY_BASE + bench + care`. The
      doc comment says why this is a struct at one implementor — the second
      gatherer is named and requested — and that generalising it is not the
      lesson.
- [x] **Step 5: Run the tests, mutation-check, commit.**

## Task 3: The careful surcharge in the quoted price

**Intent:** one price, quoted and charged from one place, with the toggle in
it.

- [x] **Step 1: Write the failing tests.** Careful charges each line's
      `+50%` rounded up; a 1-unit line becomes 2; the surcharge sits **after**
      the Lean Compiler discount (a perked recipe pays the surcharge on the
      discounted number, never the authored one); `max_craftable(careful)`
      falls accordingly; `craft` refuses when the careful cost is not
      affordable but the plain one is.
- [x] **Step 2: Run them and watch them fail.**
- [x] **Step 3: Thread `careful` through the three signatures.**
      `craft_cost(result, careful)`, `max_craftable(result, careful)`,
      `craft(result, quantity, careful)`. The surcharge is applied in
      `craft_cost` alone — the single place a price is quoted — so the
      screen, `max_craftable` and `craft` cannot disagree. Fix every caller
      the compiler names; app-core and gui pass `false` for now and get the
      real flag in Task 5.
- [x] **Step 4: Run the engine suite, mutation-check, commit.**

## Task 4: `craft` rolls a copy per unit

**Intent:** the behaviour change.

- [x] **Step 1: Write the failing tests.** Compiling one piece of equipment
      lands a copy in `GearCopies` at a quality inside the band and **not**
      in `Inventory`; compiling a batch can produce more than one distinct
      copy (seeded, so the assertion is on distinctness rather than on a
      number); a better bench raises what a batch rolls; compiling a
      *material* still stacks in `Inventory`, stays plain, and spends **no**
      `GameRng` draw (assert against a sibling `Game` on the same seed, the
      shape `grant_gear_drop`'s no-draw test already uses).
- [x] **Step 2: Run them and watch them fail.**
- [x] **Step 3: Roll in `craft`.** Build the `CraftOrder` once, then per unit
      roll `roll_quality(floor)` and route the copy through `add_copies`;
      non-equipment keeps `Inventory::add(result, quantity)` in one call and
      touches no RNG. `add_copies` already picks the store off
      `GearCopy::is_plain`, so nothing here learns the rule.
- [x] **Step 4: Run the tests, mutation-check, then the full engine suite
      and repair the fixtures the store move breaks.** A fixture reading
      `Inventory` after crafting equipment is the fixture's bug.
- [x] **Step 5: Commit.**

## Task 5: The toggle through app-core and the Compile screen

**Intent:** the player can turn it on, and the screen quotes what it will
cost.

- [x] **Step 1: Write the failing app-core tests.** `[C]` on the
      quantity page toggles `App::careful_craft`; the flag is cleared when
      the page opens, so it never leaks between two compiles; `Enter`, `[F]`
      and `[M]` all pass it to the engine; `[M]` reads the careful maximum
      when the toggle is on.
- [x] **Step 2: Run them and watch them fail.**
- [x] **Step 3: Add the flag and the key** in `app-core`: the field beside
      `pending_craft`, cleared in `handle_craft_key` where the page opens,
      toggled in `handle_craft_quantity_key`, threaded through
      `commit_craft`.
- [x] **Step 4: Draw it** in `render/crafting.rs::draw_craft_quantity`: the
      cost line reflects the toggle (it already reads `craft_recipes`, so it
      moves to `craft_cost(result, careful)` to get one quoted number), a
      row stating the toggle's state and what it buys, and the key in the
      footer. `max_craftable` on that screen takes the flag too.
- [x] **Step 5: Run the app-core and gui suites**, including the popup width
      tests — the new rows are `Row::Text`, so they do not touch the item
      column, but the footer line grows.
- [x] **Step 6: Mutation-check the toggle** (delete the clear-on-open, watch
      the leak test fail), **commit.**

## Task 6: The benches become upgradable

**Intent:** the bench term is live on shipped content.

- [x] **Step 1: Widen the census first and watch it fail.**
      `every_upgrade_path_asks_for_a_zone_material` expects exactly six
      upgradeable structures; the number becomes eight and the message keeps
      saying why the count is pinned.
- [x] **Step 2: Add `upgrade:` to `fabricator.ron` and `armory.ron`**,
      copying the shape the six existing paths use, and extend each
      description to say what the tier buys — this is the only place a
      player learns that a better bench compiles better gear.
- [x] **Step 3: Run the asset censuses and the structures suite.**
- [x] **Step 4: Commit.**

## Task 7: The gates, the seam and the rule

- [x] **Step 1: Run the full gate** — `cargo fmt`, `cargo clippy
      --workspace`, `cargo test --workspace`. Also
      `cargo test -p feral-processes-engine balance_sim`: quality sits
      outside its gate by the documented exclusion, so a moved curve here
      means the exclusion is wrong, not the test.
- [x] **Step 2: Write the argument into `docs/seams.md`** — the crafter
      seam (why a struct at one implementor), the floor's terms, the
      careful surcharge riding the quoted price, and the bench-term-was-inert
      finding.
- [x] **Step 3: Put the rule in `CLAUDE.md`** under **Items, gear and
      economy**, and `cp CLAUDE.md AGENTS.md` in the same commit.
- [x] **Step 4: Tick the roadmap's phase table.**
- [x] **Step 5: Commit.**

## Phase exit

Green workspace suite, the roadmap ticked, and the phase **played** — this
is the one whose numbers cannot be called correct from a test. `--template
extraction` opens on a base with a bench standing; the questions are whether
80–100 reads as a nerf in the hand, whether the careful surcharge is worth
paying, and whether a tier-5 bench feels like it earned its fragments.
