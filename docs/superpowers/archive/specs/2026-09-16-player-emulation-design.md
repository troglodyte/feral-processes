# Player emulation

**Date:** 2026-09-16
**Status:** implemented, `v0.13.204` (Part A) and `v0.13.205` (Part B). Sequenced after
`2026-09-16-routine-research-tree-design.md` (todo #101).
**Todo:** #100, "druid like transformations".

In a battle, the player can **emulate** a species they have learned. For a
fixed number of rounds the player fights with that species' kit: its attack
and mitigation, its basic attacks, its reach, its affinity and its routines.
The emulated kit is scaled to the player's level and hits harder than a wild
program of the same species. A perk raises it further. The player learns a
species' **image** by extracting it from a downed program. The Emulate
routine itself is unlocked by research. Changing back is a battle action
that costs the turn.

"Transform" and "shapeshift" are the fantasy words this setting does not
use. The verb is *emulate*, the state is an *emulation*, and the thing
learned is an *image*.

## Decisions taken in brainstorming

| Question | Answer |
|---|---|
| Who can emulate | The player only |
| What can be emulated | Any species whose image the player has learned |
| How an image is learned | Extracted from a downed program, with a new tool category |
| What an emulation changes | The whole kit, scaled by player level, above a same-level wild program |
| Strength scaling | Player level, plus a perk |
| HP | Unchanged. The player keeps their own HP pool |
| Where and how long | Battle only, a fixed number of rounds |
| Changing back | A battle action that costs the turn and no Power |
| How Emulate is obtained | A research node |
| The kit while emulating | The species' routines only, plus the Change Back row |
| The player's tile | The emulated species' look in boss styling, with a player outline |
| Structure | One derivation of where a body's kit comes from (`Game::kit_of`) |

## 1. Architecture: where a body's kit comes from

### The seam

Today, several functions each decide separately whether a body is "the
player, who has no species" or "a creature with a `SpeciesDef`":

- `Game::swing_move_at` (`game/combat_round.rs`) hard-codes the player's
  unarmed strike;
- `Game::natural_range_of` (`game/combat_damage.rs`);
- `Game::swing_range` (`game/combat.rs`);
- `Game::ability_affinity` (`game/combat.rs`), which has a player arm for
  class affinities;
- `Game::actor_abilities` (`game/combat.rs`), the one resolver both battle
  models' menus read.

Emulation adds a third answer. Adding an `if let Some(emulation)` to each of
those functions would copy the same check five or six times, and a later
"companions can emulate" would have to find every copy. So the answer moves
into one function:

```rust
pub(crate) enum Kit<'a> {
    Unarmed,
    Species { def: &'a SpeciesDef, level: u32, emulated: Option<EmulatedStats> },
}

pub(crate) fn kit_of(&self, entity: Entity) -> Kit<'_>
```

`kit_of` reads `Emulation` first, then `Creature`, then falls back to
`Unarmed`. Each function listed above becomes an **exhaustive** match on
`Kit`. A future source of a kit is a new `Kit` variant, and every reader
fails to compile until it answers it. A future companion emulation changes
`kit_of` alone.

**Rejected:**

- **An `if let` at each reader.** This is the copy CLAUDE.md warns about,
  and a missed reader compiles clean.
- **A `KitSource` trait with one implementation per source.** It needs
  dynamic dispatch and an interface nothing else in the engine uses, and it
  loses the compile error when a new source arrives. Every source is engine
  code anyway: forms are content (species files), not new kinds of source.
- **Stashing and swapping `Stats`/`Routines`.** A level-up inside the fight
  (XP is paid per kill, and `add_xp` full-heals) and a gear bonus welded
  into `Stats` would both be lost or corrupted on restore. Neither failure
  fails to compile.

`kit_of` also changes the path for a player who is **not** emulating. The
existing combat suite is the regression gate for that path.

**Player-only affinity.** When a body's kit is `Unarmed` and the body is the
player, the class-affinity arm of `ability_affinity` still applies. When the
player is emulating, the species' affinity applies **instead**, and the
class affinity does not add on top: the form is the whole kit.

### Attack and mitigation

`Game::effective_atk` and `Game::effective_mitigation`
(`game/combat_round.rs`) are already the one door for each figure. Both
start from `Stats`, which already includes worn gear
(`apply_equipment_delta` adds it in). For an emulating body, the innate part
of that starting figure is replaced:

```
base = emulated.atk + worn gear atk bonus        (instead of Stats::atk)
base = emulated.mitigation + worn gear mitigation (instead of Stats::mitigation)
```

Everything after `base` is unchanged: `CombatBuff`, field buffs, the
wielded program's bonus, the low-Power attack penalty, and the
`MAX_MITIGATION_PERCENT` cap. The worn-gear bonus is read through the
existing `worn_bonus`/`gear_bonus` helpers, not recomputed.

`Stats` is never written by this feature. HP, healing, damage
(`apply_damage`), level-ups and "the player's HP hit zero" are all
untouched.

## 2. Strength

One pure function, beside `progression::stats_after_levels`:

```rust
pub fn emulated_stats(def: &SpeciesDef, player_level: u32, fidelity_level: u32)
    -> EmulatedStats   // { atk, mitigation }
```

1. Start from the species' base stats and grow them to the **player's**
   level with `stats_after_levels`, using the species' own
   `growth_multiplier`.
2. Multiply attack by `EMULATION_EDGE + EMULATION_EDGE_PER_PERK_LEVEL ×
   fidelity_level`.
3. Mitigation is never scaled by level (the existing rule on
   `Stats::mitigation`), but it takes the same multiplier, rounded and then
   capped by `effective_mitigation` as usual.

`EMULATION_EDGE` is above 1.0, so an emulation beats a wild program of the
same species at the same level. Both constants go in `tuning.rs`. Their
values are guesses until measured with the arena, because `balance_sim`
models no abilities.

`kit_of` calls `emulated_stats` for an emulating body. The picker's preview
(§5) calls it too, so the preview and the fight cannot disagree.

### The perk

`Perk::EmulationFidelity`, **appended** to the `Perk` enum (the variant
order is save format). It needs:

- a named query in `perks.rs`, `emulation_fidelity_level(perks)`, which
  `emulated_stats`' caller reads;
- a catalogue file, `assets/perks/emulation_fidelity.ron`, with its name,
  description and cost;
- a row in `groups.ron` for the picker's display order.

`every_perk_has_a_query_that_answers_what_it_is_worth` fails to compile
until the query exists.

## 3. The emulation itself

### State

```rust
#[derive(Component)]
pub struct Emulation { pub species: SpeciesId, pub rounds_left: u32 }
```

It is never saved: a fight in progress is never saved.

It is removed in exactly three ways:

1. **It lapses.** `rounds_left` ages in `tick_one_combatant`, the upkeep
   both battle models share, beside `tick_combat_buff`. At zero the
   component is removed and the log says *"Your emulation lapses."*
2. **The player changes back** (§4).
3. **The fight ends.** `finish_fight` removes it with the same no-op-safe
   pattern it uses for `Cloaked`, `ReachCharge` and `Tampered`. This covers
   a win, a loss and a jack-out, because every ending goes through
   `finish_fight`.

### The kit while emulating

- **Swing:** the species' `basic_attacks()`, rolled the way a creature's
  are, through the `Kit::Species` arm of `swing_move_at`. A worn weapon
  still overrides the damage band and reach, as it does for any body
  (`attack_range`).
- **Reach:** the species' natural range, through `swing_range`'s
  `Kit::Species` arm.
- **Routines:** the species' `SpeciesAbility` list filtered to
  `level <= player level`, capped at the player's `routine_slots`, in
  `actor_abilities`' `Kit::Species` arm. This is the same filter
  `install_innate_routines` uses; the plan extracts that filter into a
  function both call rather than copying it. The player's own `Routines`
  component is untouched and returns when the emulation ends.
- **Cooldowns:** `AbilityCooldowns` is keyed by `AbilityId`, not by
  position, so the swap cannot move a cooldown onto the wrong routine. A
  species routine that shares an id with one of the player's own shares its
  cooldown, which is correct.
- **Power:** species routines are priced in Power through
  `abilities::routine_power_cost`, as every runnable routine is.
  `ability_unavailable` and `spend_power` are unchanged.

## 4. Learning, unlocking, invoking, changing back

### Learning an image

- **Store:** `resources::EmulationImages(BTreeSet<SpeciesId>)`, saved as
  `emulation_images: Vec<SpeciesId>` behind `#[serde(default)]`. No
  `SAVE_FORMAT_VERSION` bump.
- **Writer:** a new `ToolCategory::Image` (`tools.rs`). Its arm of
  `extraction_yield` answers "teaches this species' image" instead of
  items, and `extract_program` consumes the downed program as extraction
  always does. It spends no `GameRng` draw. A new tool file,
  `assets/tools/image_capture.ron` (name decided in the plan), uses it.
- **Refusal:** extracting an image the player already has is refused
  **before anything is spent**, with its own test, per extraction's
  per-refusal rule.
- **Notification:** a new `NotificationKind::LearnedImage`, once per
  profile, pointing at the Emulate routine.

The screen's preview row for an `Image` tool reads the same
`extraction_yield` call, so it says "image: Scrapper" and never a figure
that differs from the grant.

### Unlocking the Emulate routine

`assets/abilities/emulate.ron`, with a new effect
`AbilityEffect::Emulate { rounds: u32 }`. It is not `tactical_only`, so it
works in both battle models, and it is not `field_runnable`.

Under #101 the research node is **synthesised**: `routine/emulate`. Nothing
in the field carries Emulate (no `wild_weight`, no species kit), so its
family is always visible once its `research_zone` is met and the routine
tree is open. No research file is authored. The plan checks three things
against #101 as built:

- the display name `"Emulate"` parses to a family with a single root rung;
- the synthesised etched disk for `emulate` does not give a way around the
  research (a trader or creation shelf offering it). If it does, the plan
  picks between excluding it from shelves and accepting the shortcut;
- `research_zone` is set to the zone the arena says the feature is worth.

If this feature is built **before** #101, the node is an authored
`assets/research/emulation.ron` using `unlocks_abilities`, and #101's
migration moves it like every other routine grant.

### Invoking

Choosing Emulate asks for an image before it resolves, the way a routine
asks for a target:

- `Game::emulation_options() -> Vec<EmulationOption>`, one row per learned
  image with the name, glyph, and the emulated attack and mitigation at the
  player's current level (a call to `emulated_stats`).
- The chosen image rides the action. The plan picks between an
  `image: Option<SpeciesId>` on `BattleAction::Special` and the existing aim
  field, against the current shapes in both models.
- `ability_unavailable` refuses Emulate with no learned images, and while
  already emulating (switching straight to a second form is out of scope).
- Resolving it inserts `Emulation { species, rounds_left: rounds }` and
  logs *"You emulate a Scrapper."*

### Changing back

`BattleAction::Revert`, a battle action row offered only while `Emulation`
is present. It costs the turn and no Power, removes the component, and logs
*"You drop the emulation."* In the tactical model it ends the turn through
`Game::hand_on_turn`, as any action does. Both models' option builders offer
it, so app-core needs no special case beyond drawing the row.

## 5. What the player sees

### The image picker

- One list in both battle models, built from `emulation_options()`.
  Lowercase letters select a row. Cancelling returns to the routine list and
  spends nothing.
- If it needs its own `Mode`, that mode goes into `ALL_MODES` by hand. That
  list does not fail to compile, and a missing mode draws as a blank screen.
- Its height is a layout constraint (these popups do not scroll). The plan
  adds a census that the widest shipped row and a full image library fit.

### The player's tile

- `EntityView` gains `form: Option<FormLook { glyph, sprite }>`, filled by
  the engine from `Emulation` and the species def. The tactical board's body
  view gains the same field. Setting `is_boss: true` was rejected, because
  `is_boss` also feeds XP and con logic.
- While `form` is `Some`, the battle views and the tactical board draw the
  form's sprite, or its glyph if there is no sprite, in **boss styling**:
  boss magenta ink, with the con read moved into the top-left earmark by
  `ConRead::of(.., is_boss_styled = true, ..)`. The player's drawn icon
  (`"@drawn"`) and the `@` are not drawn.
- **The player stays findable through a tile outline in the player's own
  colour** (`player_look_color`, the wizard choice or `PLAYER`). All four
  corners are already taken (con earmark, nemesis, staffed, patrol), and the
  top-right nemesis mark is already cyan, so a corner mark would read as
  "nemesis".
- The sprite rule's overdraw trap applies. The gui test asserts the form's
  sprite **and** the absence of both the `@` and the `@drawn` icon, plus the
  outline.

### Help

A page in `assets/help/` for emulation, linked from the extraction page. It
must clear the manual's censuses. It never mentions the `W` wield key.

## 6. Moddability and docs

- A new species is automatically a new form. No per-species emulation data.
- `assets/abilities/README.md`: the `Emulate` effect and its `rounds` field.
- The tools README: the `Image` category.
- `assets/perks/`: the new file, following the directory's convention.
- `CHANGELOG.md` at the merge. Not `docs/manual.md` or the root README.
- **Seam:** "`Game::kit_of` is the one answer to where a body's kit comes
  from." Three writes: the argument to the memory graph, the trap (a
  reader that branches on `player_entity()` or `Creature` directly) to the
  `seams` skill, and one sentence in CLAUDE.md.
- `docs/superpowers/INDEX.md`: this spec's row.

## 7. Testing

TDD with the failing test first, in `crates/engine/src/tests/`:

- **`kit_of`:** unarmed player, creature, emulating player.
- **No-form regression:** the existing combat suite, unchanged, plus one
  test per former player branch (`swing_move_at` still yields the unarmed
  strike for a non-emulating player).
- **Strength:**
  - an emulation outhits a same-level wild program of the same species;
  - each level of `EmulationFidelity` raises it;
  - worn gear still adds to attack and mitigation while emulating;
  - the mitigation cap still applies.
- **HP:** unchanged by emulating, by lapsing and by changing back.
- **Kit:** while emulating, `actor_abilities` returns the species' routines
  filtered by player level and capped by slots; afterwards, the player's own.
- **Duration:** an emulation lapses after `rounds` rounds in the group model
  and on a battle map.
- **Changing back:** costs the turn in both models, spends no Power.
- **Teardown:** `finish_fight` removes `Emulation` after a win, a loss and a
  jack-out.
- **Learning:**
  - an `Image` extraction inserts the species and consumes the program;
  - a duplicate is refused and nothing is spent;
  - the preview equals the grant.
- **Invoking:** refused with no images; refused while already emulating.
- **Save:** a save→load test for `emulation_images` (the RON round-trip
  alone cannot catch a skipped field).
- **gui:** a headless paint test for the emulated tile: form sprite drawn,
  `@` and `@drawn` absent, player outline present, con read in the earmark.
- **Mutation check:** delete the `Emulation` arm of `kit_of` and confirm the
  strength and kit tests fail.

Instruments:

- `dev-arenas/emulation.ron`: the player emulating against a same-zone pack,
  for fitting `EMULATION_EDGE`. Numbers go in `docs/measurements/`.
- A `dev-saves` template with a few learned images and Emulate installed, so
  the feature can be played without first earning it.

`cargo test --workspace` and `cargo clippy --workspace --all-targets` pass at
each phase boundary.

## 8. Phasing

1. **The seam, with no behaviour change:** `Kit`, `kit_of`, and the readers
   converted. The whole suite stays green.
2. **Engine:**
   - `Emulation`, `emulated_stats`, the tuning constants and the perk;
   - `AbilityEffect::Emulate`, `BattleAction::Revert`,
     `emulation_options`;
   - `EmulationImages` and its save field, `ToolCategory::Image`;
   - the ability, tool and perk files, and README updates.
3. **Screens:** the picker in both models, the Change Back row,
   `EntityView::form` and the boss-styled tile, the notification and the
   help page.
4. **Instruments:** the arena scenario, the measurement, the dev-save
   template and the tuned constants.

Each phase leaves the game playable.

## Out of scope

- Companions emulating (`kit_of` is where it would go).
- Emulating outside battle, or on the map.
- A form changing HP.
- Switching directly from one form to another.
- A map marker for species whose image is not yet learned. #101's Alt
  marker is the precedent if it is wanted later.

## Risks and what the tests cannot see

- **The strength constants are guesses.** `balance_sim` models no
  abilities or classes. The arena is the only instrument, and its numbers
  only compare within one build.
- **A strong species could replace the player's build.** If the best form
  outclasses every gear and perk choice, the feature flattens the player's
  own progression. The arena scenario should compare a form against the
  player's own kit at the same level.
- **Boss styling on the player's tile** might read as a hostile boss at a
  glance. The outline is the answer on paper; only a playtest can confirm
  it.
- **`kit_of` touches every fight**, not only emulating ones. Phase 1 lands
  alone so a regression is attributable.
- **A green suite is not evidence of play.** None of this will have been
  seen on a screen when phase 3 lands.
