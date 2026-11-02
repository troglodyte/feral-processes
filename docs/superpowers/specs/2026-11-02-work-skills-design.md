# Work skills

**Status:** design, awaiting the user's approval. ADRs [0038](../../adr/0038-tamed-programs-carry-practised-skills.md) and [0039](../../adr/0039-species-carry-starting-skill-levels.md) are Proposed. No code may build on them until they are Agreed.

## Intent

A tamed program should get better at base work the longer it does it, the way colonists do in RimWorld and dwarves do in Dwarf Fortress.
A long-serving builder works visibly faster than one tamed yesterday.
Practice turns a crew into something the player has built up, which makes the base worth keeping and its programs worth not losing.
Later, a high skill may also make better items. That is out of scope here, but the design must not block it.

The machinery is general. Combat and other areas will add skills later, so the skill core must not know about base work (0038).

## Goals

- Six work skills: **Extract, Assemble, Research, Build, Dig, Farm**.
- Only tamed programs have skills. Everything else is unaffected.
- Every skill starts low. Level 0 is **slower than today's speed**, the middle of the scale is today's speed, and the top is well above it.
- A skill rises only by doing that kind of work, and it never decays.
- The pace is Dwarf Fortress-slow. A program doing one job from early on reaches competent after a zone or two and master late in a full game. Every number is a named constant in `tuning/skills.rs`.
- A species starts a few levels up in the work that suits it (0039).
- The player sees skills in three places:
  - the Base staff table
  - the program's detail page in the manifest
  - a message-log line on each level gained

## Non-goals

- Quality of output (better items from skilled workers). This is future work.
- Combat skills, or skills for the player character. The player keeps today's speed.
- Decay or rust.
- Per-species learning rates, or passions.
- Skills for Guard duty or hauling. Neither has a work rate to scale.

## Anti-goals

- A base that becomes unplayably slow. Level 0 is clumsy, not crippling. The user chose that old saves' crews start at the bottom too, so the floor must stay comfortable.
- Changing anything for an entity without skills. Wild programs, settlement workers and the player must behave exactly as today.
- Message spam that buries the log. Repeated level-ups must read well through the existing history condensing.

## Decided by the user

- Skills only for tamed programs, starting low, rising over time like RimWorld/DF.
- Faster work now; possibly better items later.
- No decay, for now.
- New tames start slower than today, as in RimWorld.
- **Existing saves:** already-tamed programs start at the bottom too, so a loaded base slows down until its crew practises.
- Species start higher in some skills.
- Display: staff table, detail page, and a message on level-up.
- Pace: a master takes most of a full game, and it is tunable.

## The design

### Skill core (`crates/engine/src/skills/`)

This is the shape set by 0038: `Skill` (closed enum, six work variants), `SkillTrack { level, practice }`, a `Skills` component holding one track per skill, and the pure functions `practise` and `practice_to_next`.

- **Levels** run from 0 to `SKILL_LEVEL_CAP`. *Claude's call: 20, as in RimWorld.*
- **Practice** is earned as one point per tick of that kind of work actually done. "Actually done" means a tick on which the program's job progress advanced, not a tick spent walking or waiting for stock.
- **The cost of each level grows** with the level, so early levels come fast and late ones slowly. Use a geometric or polynomial curve. The builder picks the shape and constants against the pace target below, and states the intended pace in `tuning/skills.rs`'s doc comment in plain terms: about how much play to reach competent, and about how much to reach master.
- **Pace target.** A dedicated worker reaches the competent level after roughly two zones of play and the cap near the end of a full run. Take a zone's length from `docs/measurements/` if anything there gives it. Otherwise say what was assumed.
- At the cap, practice stops accruing.

### What a level is worth: the work scale

The base labour code, not `skills/`, has one function that maps a level to a speed factor.

- `0.5 + 0.05 × level` (the user chose half speed at level 0). Level 0 works at half today's speed, level 10 at today's speed, and level 20 at 1.5×.
- The three numbers are constants in `tuning/skills.rs`.
- An entity without `Skills` reads 1.0.

Each work site has one function that names which skill a posting practises: `work_skill_for(...) -> Option<Skill>`. It is an exhaustive match, like `Duty::admits`, so a new task kind fails to compile rather than silently practising nothing.

| Work | Skill | Where its rate is set today | How the scale applies |
| --- | --- | --- | --- |
| A machine running an Extract-role recipe or a fixed `work` block | Extract | baked at assignment: `Game::work_ticks_for` to `systems::work_ticks_at_speed` | A new factor in that calculation, next to `SiteScales` |
| A machine running an Assemble-role recipe | Assemble | the same path; accrues in `assembler_system` | Same |
| A Research Station (`research_node`, `studies: true`) | Research | the same path; accrues in `task_progress_system` | Same. It must be classified as Research, not Extract, by one rule |
| A build site | Build | per tick, on the site: `raise_one_tick`, `BuildSite::required_ticks` | Per worker, see below |
| A dig swing | Dig | baked at `post_digger` (`BASE_DIG_TICKS_PER_SWING`) | Divide the swing ticks by the scale |
| A farm task | Farm | baked at `post_tender` (`FARM_DIMPLE_TICKS` / `FARM_HARVEST_TICKS`) | Divide the task ticks by the scale |

Rules for applying the scale:

- **Baked rates are re-read at each cycle, swing or task boundary**, so a level gained mid-posting shows on the next cycle without a re-post. *Claude's call:* `Task::required` stays the stored, baked tick count; the boundary recomputes it from the worker's current level. Nothing new is stored per posting, and an in-flight posting from an old save finishes its current cycle at the old rate and picks up the scale at the next boundary.
- **Which skill a machine cycle practises** is settled at the cycle's start from the recipe it runs: an Assemble-role recipe → Assemble, an Extract-role recipe or any fixed `work` block whatever its resource → Extract, a Research Station → Research. A recipe switch takes effect at the next cycle. Any other posting kind returns `None` (practises nothing, runs at 1.0); the plan lists every `None` arm explicitly.
- **One rate-setter per machine.** A machine runs one posted worker (`post_worker`), whose skill sets the rate and earns the practice. A worker without `Skills` posted anywhere runs at 1.0 and earns nothing.
- **Practice is flat per working tick**, not scaled by speed: a slow worker earns the same practice per tick it works. A species' starting level begins with zero practice toward the next.
- **Build** advances a shared site meter one step per worker tick. A skill scale there needs a per-worker fractional carry, or a finer-grained meter. The builder chooses. The requirement is that a level-0 builder raises a site about half as fast as a level-10 one, with no RNG, and that two builders on one site still add up.
- Divided tick counts round deterministically and never fall below 1.
- **The player's own work** (`work_structure`, `player_gather_system`) is unchanged.
- The player's class scale (`classes::work_tick_scale`) is untouched. It still multiplies with the skill scale.

Practice accrues at the same place progress accrues, for the worker whose progress moved.
A level gained there is reported to the shell as an outcome, so the message is posted from one place.
This follows the functional core rule: the decision is `skills::practise`, and the system applies and logs it.
Where a site iterates a query, any ordering that touches RNG stays sorted by `ProgramId`. Skills draw no RNG themselves.

### Who gets skills, and when

- `Game::roster_parts` inserts `Skills`, seeded from the species' `starting_skills` (0039). Every door that makes a program tamed goes through it: taming, breeding, outposts, new game, spawning.
- *Claude's call:* release and death treat `Skills` the way they treat the other `roster_parts` components (the builder checks which). A released program that is tamed again re-seeds from its species; bred offspring start from their species, with nothing inherited.

### Save

`CreatureSave.skills` as set by 0038: saved by name, defaulted, no format bump.

- An absent field (every save made before this) loads as the species' starting levels.
- *Claude's call:* the user said "start at the bottom", and the species start is the bottom for that species, so an old program is seeded the same way a fresh tame is.
- The change needs a save-then-load test through the real path, not only a RON round trip.

### Species content (0039)

`SpeciesDef.starting_skills`. Each of the 19 shipped species gets one or two entries in the work its existing work profile points at, at a few levels. *Claude's call: 2 to 5, never above 5.* The builder authors them and lists the numbers in `assets/species/README.md`.

### Display

- **Base staff table** (`Game::work_table`, `views::work`, `render/building/staffing.rs`): each row shows that program's skill levels. The layout is the builder's call, within the rule that GUI text never clips or wraps. The six skills do not map one-to-one onto the four duty columns: Operate covers three skills, and Dig covers digging and farming.
- **Program detail page** (`Game::program_manifest`, `views::manifest`, `render/manifest/sections.rs`): a SKILLS section for tamed programs only, listing each skill with its level and progress to the next level. A wild program's page has no section.
- **Level-up message** (every level, the user's choice): one line in the base log, `MessageKind::LevelUp`, through the existing `push_base_kind` path. For example, "Kernel Moth is now a level 4 builder." No tick vocabulary. Repeats must condense acceptably in `message_history`.

## Constraints

- `Game` is the whole API. The views carry skills out, and app-core and gui never touch `Skills`.
- Constants in `tuning/skills.rs`, split by domain (do not grow `tuning/mod.rs`).
- Read `.claude/rules/seams-base.md` (and the `seams` skill's labour traps) before changing the labour systems.
- **New seam:** "every work site reads the skill scale through `work_skill_for`". It gets the three writes in the order the `seams` skill documents.
- Run the `design-patterns` and `module-boundaries` skills at plan time on the `skills/` core and the per-site hook.
- CHANGELOG entry at landing. This is a save-compatible change, so the bump is not breaking.

## Testing

- `practise` and `practice_to_next`: level rollover, multiple levels from one large amount, cap, no practice past cap, curve strictly increasing.
- The work scale: level 0 below 1.0, the middle level at 1.0, the cap above it, monotonic.
- Per site: for each of the six skills, a high-level worker finishes measurably sooner than a level-0 one, and practises the right skill and no other. Use a `dev-saves/` template where one fits.
- An entity without `Skills` works at exactly today's rate (regression pin).
- The player's own work rate is unchanged.
- Save then load keeps levels and practice. An old save without the field loads with species starting levels.
- Species census: every `starting_skills` level is within the cap.
- Existing labour tests that count ticks will move because level 0 is slower. Pin a level (likely the "today's speed" level) where the test is about something other than skills, and update the number where the test is about speed. Never loosen an assertion to make it pass.
- `balance_sim` after the change. It is battle-only, so a move would mean something leaked.

## Left to the builder

- The curve's shape and constants against the pace target, assuming a dedicated worker is working most of its ticks and stating the assumed share in the tuning doc comment. The user said the pace is tuned after it lands, so the first constants need to be reasonable, not exact.
- The Build site's fractional mechanism.
- The staff-table layout.
- Message wording.
- Which tests pin a level and which update.
