# Combat skills

**Status:** draft 2026-11-02; ADR [0040](../../adr/0040-combat-skills-and-the-figure-pipeline.md) Proposed.

## Intent

The work skills from 0.21.16 make a long-serving program a better worker. This feature does the same for fighting.
A program that has swung, been hit and run routines fight after fight should fight better than a fresh tame of the same species and level. The player grows the same way.

Each combat figure gets **two sources**:
- an **attribute**, which says what a body *is*;
- a **skill**, which says what it has *practised*.

To-hit and damage come from different attributes and different skills, so a careful program and a heavy hitter are distinct things that grow apart.

**This is built for a follow-up.** Today only the player's attributes become combat stats (`progression::derive`). A program's attributes sit mostly unused, and it fights off stored stats.
The user wants programs' attributes to count in a later effort. This design puts the "attribute part" of every figure behind one door that is blind to whether the body is the player or a program. Then the follow-up changes what that door reads for programs and nothing else.

## Goals

- **Four combat skills** on the existing machinery (`skills/`, ADR 0038): **Aim, Force, Defence, Routines**.
- **The figures** and where each part comes from:

  | Figure | Attribute (player today, programs in the follow-up) | Skill |
  |---|---|---|
  | to-hit (accuracy) | **Analysis** | Aim |
  | damage (ATK) | **Amplitude**, a new attribute | Force |
  | mitigation | Footprint (as today) | Defence |
  | routine Power cost | none; Bandwidth already sizes the Power pool | Routines |

- **Analysis loses its ATK effect.** Amplitude takes it, and Analysis gains an accuracy effect.
- **Who carries the skills:** tamed programs and the player, combat skills only for the player. Wild and settlement programs carry none.
- **Skill levels:** the 0 to 20 curve of the work skills. Level 0 is slightly worse than today, 10 is exactly today, 20 is the same distance better. The user chose **±10%**.
- **Practice comes from doing it:**
  - **Aim**: every attack roll the body makes, swing or damaging routine, hit or miss.
  - **Force**: every attack the body *lands*.
  - **Defence**: every attack roll aimed at the body.
  - **Routines**: every routine the body pays Power for.
- **Starting levels:** species knacks (0039), plus a knack from the player's class.
- **Old saves:** load at the starting levels. The user accepted that a party steps slightly down on load.
- **Shown in three places:** the program's SKILLS box, a SKILLS box for the player, and the existing level-up line.

## Non-goals

- **Programs' attributes counting.** That is the named follow-up (below). Here a program's attribute part stays what it is today: its stored stat.
- **Work skills for the player.**
- **Evasion, crit and fumble from skills.** Entropy's crit and fumble are untouched.
- **Skills for wild programs.**
- **Decay.**

## Anti-goals

- **A fresh party losing fights it wins today.** ±10% is "a bit rusty".
- **The player building at half speed** because it now carries `Skills`.
- **Any change for a body without `Skills`.** Wild programs, settlement programs, summons and reinforcements fight exactly as today.
- **The player's numbers shifting from the attribute move alone.**
  - A class's starting Amplitude must reproduce the ATK its Analysis gave it, so the move leaves every class's starting ATK where it was.
  - The new to-hit effect is measured from the attribute's `base`, as every attribute effect is. A class at the Analysis base gains nothing; a class authored above it gains a little to-hit, which is the point.
- **Level-up spam.** The line condenses like the work line.

## How it works

### The figure pipeline (the reusable part)

Every combat figure is computed in two steps. Each step is one function, and the steps never mix.

1. **The body's base figure**: `Game::base_figure(entity, DerivedStat)`, one door that does not care whether the body is the player.
   - **Today:** for the player it is the `progression::derive` output it already uses (attributes applied over the class base). For a program it is the stored stat, or 0 for a figure programs never had (accuracy from an attribute).
   - **The follow-up** changes only the program arm: build a `DerivedBase` from the species and call the same pure `progression::derive`.
   - The combat doors must read the attribute part through this door, never `Stats` or `Derived` directly. A census test, or a grep in the review, holds them to it.
2. **The practised scale**: one pure function, `skills::scale(level, spread) -> f32 = 1 + spread * (level - NEUTRAL) / NEUTRAL`, where `NEUTRAL` = 10.
   - Combat passes `spread` = 0.10.
   - The work scale (`skill_scale.rs`'s `SKILL_SCALE_FLOOR + PER_LEVEL * level`) is that same line with `spread` = 0.5, so it becomes a call to it, not a copy (CLAUDE.md: a "mirrors" claim must be a call).
   - What the scale is *worth* for each figure stays in the reading area (0038).

Each combat door applies its skill on top of the base:

- **To-hit:** `Game::combatant_profile`.
  - `accuracy_of(...)` takes the attribute's accuracy contribution through the existing bonus argument.
  - The result is multiplied by `scale(Aim)`, beside the existing jam cut.
  - This covers swings, reactions and damaging routines, since all of them build a profile here (user decision).
- **Damage:** a landed hit's final damage is multiplied by `scale(Force)`.
  - `Combatant` gains `damage_scale: f64`, default 1.0. `resolve_attack_inner` applies it, and so does `battle::expected_damage`, so the forecast, the AI and the roll agree.
  - `balance_sim` builds its `Combatant`s at 1.0.
- **Mitigation:** `Game::effective_mitigation` adds `(Defence level − 10)` points to the base, the same as a gear point. That happens before the status scaling and the existing `clamp(0, MAX_MITIGATION_PERCENT)`.
- **Routine cost:** `abilities::routine_power_cost(def, level: Option<u32>)` multiplies by `2 − scale(Routines)`, i.e. 1.1 at level 0 and 0.9 at 20.
  - `None` means neutral, for a caller with no invoker (the craft and market views).
  - The routine page quotes the inspected body's level.
  - It stays the one price expression (seam).

### Skills

- `Skill` gains `Aim, Force, Defence, Routines`, appended. Saves name skills, so order is display only.
- Two consts split the families: `Skill::WORK` (the six) and `Skill::COMBAT` (the four).
  - Every work reader iterates `WORK`: the staff table, the selected-row line, `skill_scale.rs` and the work view.
  - Every combat reader iterates `COMBAT`.
  - `Skill::ALL` stays the union for the save and the census.

### Practice hooks

Each hook sits at the one door for its event, after the existence checks:

- **Aim and Defence:** where an attack roll resolves for a creature against a creature. One hook covers swings, reactions and damaging routines in both combat models. A prop, a nest or rock never practises.
- **Force:** the same place, on a `Hit` or `Crit` outcome.
- **Routines:** inside `Game::spend_power`, the one charge (seam). A passive is never charged, a refusal comes before the charge, and a fizzle under a reaction keeps its charge and so counts.

Each skill has its own practice-per-event constant in `tuning/skills.rs`.
**Pace intent:** a body that fights most encounters reaches 10 after about two zones and the cap near the end of a full run. The builder sets the numbers from `docs/measurements/` and writes the assumption into the doc comment.
Practice in an arena fight is discarded with the session.

### Who carries skills

- **Tamed programs:** `roster_parts`, as today.
- **The player:** `Skills` is inserted when the class is applied at creation (`creation.rs`, beside `classes::apply_kit`), seeded from the class.
  - A player with no class has every combat skill at 0.
  - **Work stays blind to the player:** `skill_scale.rs` treats the player as having no skills, in one place.
- **Fusion:** the child keeps the dominant parent's whole `Skills`, with the ring and talents. `fuse_companions` assembles its own list (seam), so the builder checks it carries `Skills` today and pins that with a test.

### Starting levels and content

- **Species:** `starting_skills` (0039) gains combat entries. Each species gets one combat knack where its role points at one.
- **Class:** `ClassDef` gains `starting_skills`, `#[serde(default)]`, the same shape, combat skills only (census).
  - An obvious start is Striker→Force, Bastion→Defence, Invoker→Routines, Saboteur→Aim. The other four are the plan's call.
  - This bends the seam "a class grants affinities and nothing else" (0040).
- **Amplitude:** a new `assets/attributes/amplitude.ron`, effect `Atk` 1.0 per point.
  - Analysis drops `Atk` and gains `Accuracy` at a per-point figure the plan fits so a high-Analysis class gains a modest edge.
  - `DerivedStat` gains `Accuracy`, and `Derived` and `DerivedStats` gain `accuracy`.
  - All eight classes author `amplitude:` at the value that keeps their starting ATK. The census "all eight classes author all six" becomes seven.
  - Both `assets/attributes/README.md` and `assets/classes/README.md` are updated.
- **Player save:** an old player save has no Amplitude, so it loads at Amplitude's `base` with its Analysis unchanged. Its ATK is stored in `Stats`, so the player's ATK holds until the next derive. The builder makes the next derive reproduce it: the attribute load path fills a missing attribute from the class's authored value, if `authored_or_base` doesn't already. A save→load test pins it.

### Saves

- **Programs:** saved skill rows are **overlaid on `species.seed_skills()`**. Today a non-empty list loads as-is with unlisted tracks at 0, so a 0.21.16 save, which has six work rows, would load at combat 0. Overlaying fixes that.
- **Player:** `PlayerSave` gains `skills`, `#[serde(default)]`, overlaid on the class seed.
- No `SAVE_FORMAT_VERSION` bump (0023).
- Save→load tests for both, plus a 0.21.16-shaped save.

### Screens

- **Program SKILLS box:** a COMBAT group under the work rows. If it does not fit at 1280x720, it becomes two columns, measured as the 0.21.16 line was.
- **Player:** a SKILLS box with the four combat rows on the player's own stats page. The builder picks the page, and app-core gets a view struct for it.
- **Staff table:** `WORK` only.
- **Level-up line:** one noun per combat skill, no occult words. It says "You" for the player.
- **Points / dossier:** Amplitude appears as a seventh attribute row. Analysis's `does:` text now describes to-hit.

### Balance

- `balance_sim` builds bodies with no skills (scale 1.0), so skills move none of its curves.
- The attribute move *does* reach it, since it models the player's derived stats. Its ATK must come out unchanged and its accuracy can rise slightly.
- The `balance_sim` gate is checked after the Amplitude change, and any moved curve is explained before it is accepted.
- The ±10% skill band is ungated. One arena check of a level-0 party against level 10 is recorded in `docs/measurements/` if it meets that README's bar.

## Follow-up: programs' attributes count

A separate effort, noted here so it is not lost.
Change `Game::base_figure`'s program arm to `progression::derive` over a species-built `DerivedBase` and the program's own `Attributes`.
That gives programs Analysis to-hit, Amplitude damage, Footprint mitigation and HP, and the rest.
It will move every program's numbers and needs its own spec, ADR and `balance_sim` refit. Nothing in this feature should need to change for it.

## Constraints

- `skills/` names no combat type (0038). `skills::scale` is mechanism. What a level is worth lives at each door.
- One door each for accuracy, mitigation and Power cost (seams). The cost function takes the level as data.
- Skills take no RNG draws.
- `PlayerClass` order is save format. `.ron` assets deny unknown fields (0016).

## Calls Claude made (check these)

1. One `skills::scale` line serves work and combat, and work's formula is rewritten as a call to it (same numbers).
2. Force applies through a new `Combatant::damage_scale`, not a scaled `atk`, so the roll's spread scales too and `expected_damage` matches.
3. Defence points enter before status scaling, like gear.
4. A routine cost with no invoker in hand is priced at neutral.
5. A classless player starts at combat 0.
6. `balance_sim` stays skill-blind. Only the attribute move is gated.
7. A miss practises Aim, and a fizzled routine practises Routines.

## Left to the builder

- Practice-per-event constants.
- Species, class and Amplitude seed numbers.
- Analysis's per-point accuracy.
- Level-up nouns.
- Where the player's SKILLS box lives.
- The SKILLS box layout.
