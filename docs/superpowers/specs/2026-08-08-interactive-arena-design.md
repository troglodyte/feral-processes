# The interactive arena — design

**Date:** 2026-08-08
**Status:** designed, not implemented.
**Builds on:** `2026-08-07-battle-arena-design.md`, which shipped the
headless harness this extends.

## The problem

The arena runs real fights, and nobody can watch one.

`cargo run --bin arena -- dev-arenas/opening-fight.ron` measures a fight
properly: real damage rolls, real initiative, real enemy AI, real gear,
seeded and repeatable. What it cannot do is let a person play it. The party
plays the game's own All-Attack every round, which is deliberate — a policy
engine written for the tester could invent decisions the game never makes —
but it means **no companion Special ever fires**. `dev-arenas/README.md`
states this as the tool's blind spot, and it is the same gap `balance_sim`
has: ability magnitudes are ungated by both.

So the harness answers "what does this pack cost a party that only swings"
and nothing answers "what does this pack *feel* like". A number can say a
fight is winnable at 62% and say nothing about whether the win came from a
routine the player had to notice they held.

The second gap is authoring. A scenario is a `.ron` file, so trying "what if
it had one more of those, and I were wearing the other weapon" means editing
text, saving, re-running, reading a report. That is a fine loop for taking a
measurement and a bad one for finding the fight worth measuring.

## What this is for, in order

Ranked deliberately; the ranking decides the tie-breaks below.

1. **Playing an authored fight.** The whole battle UI — Specials, items,
   targeting, jacking out — against a composition chosen rather than rolled.
   This is what closes the ability blind spot, and it closes it by having a
   person press the keys rather than by writing a policy that would drift.
2. **Building the fight in the UI.** Pick the player, the loadout, the
   party, the pack; fight it; adjust; fight it again. No text editor in the
   loop.
3. **Crossing to the headless harness.** What you built by feel is written
   back out as a `.ron`, so the same fight can then be run fifty times for a
   win rate. And the reverse: a loss seed from a report is pinned here and
   watched by hand.
4. **Costing nothing when it is not being used.** It is a dev tool inside
   the shipped game. A player must not see it, and a normal run must not pay
   for it.

Explicitly not goals: measuring anything (the headless bin measures; this
one is played), and being reachable in a release build without opting in.

## The shape

Four crates move, which is what makes this a spec rather than an inline
session.

| Crate | What lands |
|---|---|
| `engine` | `arena::stage` and `arena::Watch` — the fight-staging seam and the outcome reader, both shared with the headless path |
| `app-core` | `ArenaSession`, five `Mode`s, the builder's row table, the three save-path omissions |
| `gui` | `render/arena.rs` — builder, pickers, result screen |
| `launcher` | installs the template resolver, and `resolve_template` moves into the lib so both consumers share one copy |

No save-format change. `Scenario` is not a save, so `SAVE_FORMAT_VERSION` is
untouched.

## 1. One way into a staged fight

The interactive fight and the headless measurement must not be two
implementations of "set up this scenario". `arena::run`'s middle is split
out:

```rust
pub struct Staged {
    pub game: Game,
    pub watch: Watch,
    pub warnings: Vec<String>,
}

pub fn stage(scenario: &Scenario, assets_dir: &Path, seed: u64)
    -> Result<Staged, String>;
```

`stage` runs `build_player`, then `build_opponents`, inserts
`GameRng(StdRng::seed_from_u64(seed))`, sets `keep_battle_narration`, and
calls `begin_battle` — handing back a `Game` sitting in an active battle
with nobody having acted yet. `run_rep` loses that preamble and becomes only
the auto-play loop over an already-staged game. `run` is then `stage` plus
`run_rep`, once per rep.

This is the `Game::enter_frame` argument one level up: descending by link,
climbing one and falling through a fault differ in exactly one thing and
agree on the rest, so the spine lives once. Here the one thing that differs
is who presses the keys.

`stage` takes `seed` as a parameter rather than reading `scenario.seed`,
because rep *n* runs at `scenario.seed + n` and the result screen's
next-seed key is the same increment. A `stage` that read the field would
force both callers to mutate a scenario they do not own.

**No new public `Game` method.** `begin_battle`, `spawn_wild_creature_scaled`
and the `world` field stay reachable from inside the engine and nowhere
else, so the compiler barrier keeping the renderer out of the ECS is exactly
as strong as it was.

### `Watch` — why the outcome is not read in app-core

Reading what a fight cost is not obvious, and both of the non-obvious parts
are already documented in `run.rs`:

- **HP is sampled per round, and a round that granted a level is skipped.**
  A level-up full-heals in `progression::add_xp`, and the killing blow is
  usually the level — so a fraction read after the fight reports a hard-won
  win as costing nothing.
- **"Won" is read off the opponents, never off the player.** A battle does
  not end when the player's HP hits zero: a Forgiving defeat is absorbed
  *inside* the round that lands it by `difficulty::death_handling_system`,
  which reboots the player to a fraction of max HP. By the time anything
  outside could look, the player is alive again.

An app-core implementation that recomputed either would be the copy
`CLAUDE.md` forbids — and it is the copy nobody runs, since the headless
path would keep working while the screen quietly lied. So:

```rust
pub struct Watch { /* seed, player, party, opponents, level, hp, rounds, transcript */ }

impl Watch {
    pub fn observe(&mut self, game: &Game);          // after each resolved round
    pub fn finish(&self, game: &Game) -> RepRecord;  // what the fight cost
}
```

`observe` bumps the round count, samples HP unless a level was granted, and
appends `game.battle_log()` — after every round, never at the end, because
`end_battle` calls `retain_outcomes_since_battle` and deletes the
blow-by-blow. `finish` derives `won` from the opponents and
`companions_downed` from the party members noted at staging time.

`run_rep` and app-core both call it. There is one answer to "what did the
fight cost", and it is in the crate that knows why the question is hard.

## 2. The arena session, and its three omissions

```rust
pub(crate) struct ArenaSession {
    scenario: Scenario,
    path: Option<PathBuf>,      // the .ron it was loaded from or saved to
    seed: u64,                  // the seed of the current or next fight
    watch: Option<Watch>,
    outcome: Option<RepRecord>,
    warnings: Vec<String>,
    catalog: ArenaCatalog,      // SpeciesDb + ItemDb for the pickers
}
```

Held as `App::arena: Option<ArenaSession>`. **The presence of that `Option`
is the whole enforcement of "an arena session never touches disk"**, and
that is the load-bearing part of this design. Three things must not happen
during an arena fight, and each is invisible if it regresses:

- **No save.** Already inert: `current_save_path` stays `None`, and both
  `save_game` and `maybe_autosave` early-return on it. This one costs
  nothing to hold and is asserted anyway, because a later change that gave
  the arena a save path would break it silently.
- **No profile write.** `flush_profile_writes` must not run. An arena kill
  is not a run, and a rung earned here would be written to the real
  `profile.ron` and then paid out to every future new game by
  `grant_profile_rewards`.
- **No run history, and no Game Over.** `check_game_over` must not write
  `run_history.log`. A `Save` player source can carry Permadeath in, so a
  lost arena fight is a reachable `is_game_over` — and it belongs on the
  result screen, not on the real Game Over page.

`after_tick` already exists as "everything that has to happen after the
world may have ticked, in one place so a third tick site cannot pick up one
half and miss the other", so it early-returns for an arena session and both
of its callees are covered at once. `check_game_over` is not inside it and
takes its own guard. One predicate, `App::in_arena()`, is read by both —
not three ad-hoc `self.arena.is_some()` checks, because what has to stay
true is a list and only a named predicate makes the list checkable.

The catalog is loaded when the arena screen opens rather than in
`App::new`. The screen is reachable from the main menu, where there is no
`Game` to ask for `species_defs()`, so app-core loads `SpeciesDb` and
`ItemDb` itself the way it already loads `AchievementDb`. Lazily, because
goal 4 says a normal run must not pay for a dev tool.

## 3. The builder

The builder edits a `feral_processes_engine::arena::Scenario` **directly**.
There is no parallel builder type and no second way to express a fight: the
struct being mutated is the struct the `.ron` holds and the struct the bin
runs, so a knob added to the schema cannot exist in one tool and not the
other.

Five modes:

| Mode | What it is |
|---|---|
| `ArenaBuilder` | the scenario editor, and the screen everything returns to |
| `ArenaLoad` | pick a `dev-arenas/*.ron` |
| `ArenaSave` | filename entry — the `fuse_name_input` text-input idiom |
| `ArenaPick` | one picker, four targets |
| `ArenaResult` | what the fight cost |

`ArenaPick` is deliberately one mode rather than four. Party species,
opponent species, equip item and inventory item are all "choose a row from a
catalogue and put it somewhere"; which somewhere is a `pending_arena_pick`
field, following `Mode::ManifestPick` and `SwapChoice`. Four near-identical
modes would be four near-identical handlers to keep in step.

Numbers — level, zone, opponent count, inventory qty, fusion tier, seed,
reps — adjust inline with Left/Right on the highlighted row rather than
opening a mode. `Enter` on an `Add …` row opens the picker; `Backspace`
removes the highlighted row.

### The rows are one function

`App::arena_builder_rows() -> Vec<ArenaRow>` is the single source of both
the rows the handler dispatches against and the labels gui draws. This is
the `base_menu_rows` rule and it applies for the same reason: **rows are
hidden dynamically.** `equip`, `inventory` and `party` are an *error* on a
`Save` or `Template` player rather than being ignored — the engine refuses
them, because a save brings its entire run across and an authored loadout
beside it is a contradiction rather than an override. So those rows
disappear when the source is not `Fresh`. A renderer that rebuilt the list
from a static table would be right until the first hidden row and then draw
a different row from the one under the highlight.

### Templates, and the crate boundary

`PlayerSource::Template(name)` stays selectable, which needs a seam.
`dev_template` lives in the launcher — `arena` the bin lives there for that
exact reason — and app-core cannot see it. So:

```rust
pub struct DevTemplates {
    pub names: Vec<String>,
    pub resolve: fn(&str) -> Result<PathBuf, String>,
}
```

`App::dev_templates: Option<DevTemplates>`, installed by `main.rs`. A plain
`fn` rather than a boxed closure: `dev_template`'s resolution captures
nothing. Absent — any frontend that is not this launcher — the Template
source is simply not offered, and a loaded scenario naming one reports the
engine's existing error.

This also fixes a copy that already exists: `resolve_template` is in
`bin/arena.rs` today and the game needs the same three lines. It moves into
the launcher's `[lib]` beside `dev_template`, where the bin and `main.rs`
both call it.

## 4. Fighting, and the result

`Enter` on the builder stages the scenario at the current seed and drops
into `Mode::Battle` — the existing battle UI, entire. Specials fire, items
are spent, targets are chosen, jacking out is on the table. That is the
point: the blind spot closes because a person is pressing the keys, not
because a second policy was written.

**`settle_after_round` is the hook, and it is one hook.** It already exists
as "the shared tail of every action that can end a battle": `watch.observe`
goes there, and on `!still_active` the mode becomes `ArenaResult` instead of
`Playing`. One wrinkle to fix first — the jack-out arm calls it only when
the battle ended, so a *failed* flee resolves a round that the tail never
sees and the HP sample for it is lost. Calling it unconditionally is a
no-op when the battle is still live (the mode is already `Battle` and the
reveal restart is already guarded), so that arm becomes unconditional and
the hook is genuinely single rather than nearly single.

The result screen shows won or lost, rounds, HP fraction, companions down,
the seed, any staging warnings, and the round-by-round transcript the
`Watch` collected — scrollable, since `keep_battle_narration` is set and an
arena session never returns to a map pane for the prune to protect.

- `[R]` refights the same seed.
- `[N]` refights at `seed + 1`.
- `[Esc]` returns to the builder with the scenario intact.

`[N]` is the manual version of `reps`, which is why `reps` is preserved and
editable in the builder but never acted on here: the GUI always fights one.
A scenario built by feel and saved out carries the rep count the headless
run should use.

**Fleeing is the only way out mid-fight, and it records as a loss.** That
matches the headless path, where a fled fight leaves the pack standing and
`finish` reads the opponents. An abandon key that did not count would be a
third notion of an outcome.

**Staging warnings are shown, never applied.** `build_opponents` warns when
a composition exceeds `group_size_ceiling` or `enemy_group_ceiling` for the
zone and builds it anyway — explicit authoring is the point of a tester, and
"what if zone 1 threw nine at me" is a legitimate question. The warnings
surface in the status line when the fight opens and again on the result
screen. Nothing is capped, so nothing is silent.

## 5. The gate

`FERAL_DEV_ARENA`, read once in `App::new` into a bool, following
`FERAL_DEV_REVEAL`'s shape (`stack_view.rs` reads it straight from the
environment, so this is the established pattern rather than a new one). The
main-menu option list already builds dynamically — Load Game is conditional
on there being saves — so the Arena row is one more clause in the same
place, in both `handle_main_menu_key` and `render/meta.rs`.

Unset, nothing about this feature is reachable and nothing about it is
loaded.

## Testing

**Engine.** A staged game has an active battle and has not acted. `stage`
followed by `run_rep` reproduces `run` exactly at the same seed — the
property the split rests on. The two existing sampling tests in `run.rs`
(`a_level_up_on_the_killing_blow_does_not_report_the_fight_as_free`,
`a_won_fights_transcript_survives_end_battle`) move onto `Watch`, since that
is where the logic now lives.

**app-core.** The `Fresh`-only rows hide when the player source is a save,
asserted through `arena_builder_rows` rather than through the static table.
The result screen's next-seed key advances by one. And each of the three
omissions gets its own test — no autosave, no profile write, no history
write — because an omission is invisible otherwise and the regression is a
later change adding one of them back without noticing.

**gui.** The builder, the picker and the result screen draw. Row width on
the picker is measurable headlessly through `paint::with_painter`.

**Full suite** before anything is called done, per the standing gate.

## Docs

- `dev-arenas/README.md` grows the interactive half: how to reach it, what
  it does that the bin does not (Specials), and the loop between the two —
  build by feel, save, run for a win rate, pin a loss seed, watch it.
- `CHANGELOG.md` gets its own `## X.Y.Z` section, and the workspace version
  bumps at the merge.
- `CLAUDE.md` gains a load-bearing-seams entry for the arena session's three
  omissions, since that is exactly the kind of fact that costs tool calls to
  rediscover and reads as absent code.
- `README.md` and `docs/manual.md` stay carved out.

## Decisions taken rather than asked

- **Fleeing counts as a loss** (§4), for consistency with the headless path.
- **Ids are picked, never typed**, so the builder needs no validation — an
  unknown species or item is unreachable rather than rejected.
- **The catalog loads lazily**, so an unset gate costs a normal run nothing.
- **`dev_template` stays in the launcher.** Moving it to app-core would make
  the resolver seam unnecessary, but it would also drag `repo_root()` and
  `capture` — a savetool concern — across a boundary that `CLAUDE.md`
  currently states as a reason the `arena` bin cannot live in the engine.
  The injected `fn` is the smaller change and does not invalidate that note.
