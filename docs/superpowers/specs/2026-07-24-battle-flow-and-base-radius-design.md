# Battle flow and base radius

## Problem

Three complaints, two about combat pacing and one about base scale.

**The resolve popup hampers combat flow.** Every resolved round drops the
player onto `Mode::BattleResolve`, a full-screen overlay that must be
dismissed before planning the next round. In a fight that runs six rounds
that is six extra keystrokes that convey nothing.

They convey nothing *literally*: `BattleState::log`
(`crates/engine/src/resources.rs:135`) is never written to. It is initialized
empty at `lib.rs:2608` and at the `insert_battle` test helper (`lib.rs:6111`),
cloned into `BattleView::log` at `lib.rs:3166`, and that is its entire
lifecycle. All combat narration goes through `Game::log()` into the global
`MessageLog`. So `view.log` is always empty and the popup always renders its
`"The round passes quietly."` fallback, over a battle screen that was already
showing the real narration — both renderers draw `game.message_log(..)` on the
battle screen itself (`tui/src/ui.rs:1825`, `gui/src/render.rs:1920`).

The popup is a speed bump in front of a log pane that already works.

**Planning a full party is repetitive.** Every slot needs an individual
decision even when the intent is "everyone hit the thing" or "everyone brace".
With `MAX_PARTY_SIZE` at 5 that is up to six prompts for a round with one
actual decision in it.

**The base is too big.** `MAX_BUILD_DISTANCE_FROM_HOME` is 15, stamping a
31×31 platform.

## Approach

Four independent changes, deliberately kept separable. Nothing here changes
the save format — `BattleState` is not serialized — and nothing changes the
engine's public-API shape beyond one added method and two deleted dead fields.

Rejected along the way:

- **Keeping the popup but populating `BattleState::log` for real.** This is
  the "fix the bug as written" option: wire narration into the battle log so
  the popup finally shows something. It solves the wrong problem — the
  narration is already on screen, and the complaint is the dismissal
  keystroke, not the content.
- **`D` for all-defend with Decompile left on `d`.** Matches the original
  request literally, but puts a party-wide brace one shift key away from a
  capture attempt that spends a taming catalyst. Rejected in favour of the
  re-key below.
- **`F` for all-defend, leaving Defend on `f`.** Zero re-keying, but the
  party-wide key would not match the mnemonic and the shift-means-everyone
  rule would apply to `a` and `f` only by accident.

## Changes

### 1. Delete the resolve popup

`Mode::BattleResolve` is removed. `commit_battle_action`
(`app-core/src/lib.rs:1049`) resolves the round and returns to `Mode::Battle`,
or `Mode::Playing` when the fight ended — the branch it already has, minus the
popup detour.

Deleted with it:

| Item | Location |
|---|---|
| `Mode::BattleResolve` variant | `app-core/src/lib.rs:183` |
| `handle_battle_resolve_key` + its dispatch arm | `app-core/src/lib.rs:588`, `:1038` |
| `App::battle_log_mark` field, init, and assignment | `app-core/src/lib.rs:301`, `:375`, `:1059` |
| `render_battle_resolve` + match arm | `tui/src/ui.rs:50`, `:1941` |
| `draw_battle_resolve` + match arm | `gui/src/render.rs:113`, `:2000` |
| `BattleResolve` in the GUI `in_battle` check | `gui/src/lib.rs:166` |
| `BattleResolve` in the `needs_status_banner` test list | `gui/src/render.rs:2160` |
| `BattleState::log` field + both initializers | `engine/src/resources.rs:135`, `lib.rs:2608`, `:6111` |
| `BattleView::log` field + the clone that fills it | `engine/src/lib.rs:536`, `:3166` |

The two `BattleView` fields are provably dead once the popup is gone — the
popup was their only consumer — so they go rather than lingering as unused
API surface.

### 2. Round separator in the log

Without the popup the log is one continuous stream with no round boundaries.
Add a `MessageKind::Round` variant (`engine/src/resources.rs:32`) and log a
`── round 3 ──` line at the top of `battle_resolve_round`
(`engine/src/lib.rs:2897`), before initiative is rolled.

The number logged is the round being resolved — i.e. `battle.round` read
*before* the increment at `lib.rs:2949`. This is the same number the planning
screen shows in its `"Hostile programs — round {}"` header, so the separator
and the header agree without any off-by-one correction. The old popup title
needed `saturating_sub(1)` only because it drew *after* the increment.

Both renderers already switch on `MessageKind` to style a log line
(`tui/src/ui.rs` `message_line`, `gui/src/render.rs` `draw_message_line`), so
each gains one arm, styled dim.

### 3. Re-key and lowercase the action labels

In `Game::battle_action_options` (`engine/src/lib.rs:2811`):

| Action | Key was | Key becomes | Label was | Label becomes |
|---|---|---|---|---|
| Attack | `a` | `a` | `[A]ttack` | `[a]ttack` |
| Defend | `f` | `d` | `De[f]end` | `[d]efend` |
| Special | `s` | `s` | `[S]pecial` | `[s]pecial` |
| Decompile | `d` | `c` | `[D]ecompile` | `de[c]ompile` |
| Use item | `u` | `u` | `[U]se item` | `[u]se item` |
| Jack out | `j` | `j` | `[J]ack Out` | `[j]ack out` |

Lowercase throughout establishes the rule the party-wide commands depend on:
**a lowercase key acts for one member, its uppercase counterpart acts for the
whole party.** With Defend on `d`, `a`/`A` and `d`/`D` are symmetric and no
key sits one shift away from an unrelated action.

Jack out's label is hardcoded in both renderers (`tui/src/ui.rs:1856`,
`gui/src/render.rs:1937`) rather than coming from the engine; see change 4.

### 4. `A` / `D` party-wide commands

`handle_battle_key` lowercases every key at `app-core/src/lib.rs:929`, so `A`
and `a` are indistinguishable today. The uppercase commands are matched
*before* that lowercasing; the existing per-slot path below it is untouched.

- **`D`** — assigns `BattleAction::Defend` to every still-unplanned slot from
  the active one onward, then resolves the round.
- **`A`** — if more than one group is alive, opens the existing
  `Mode::BattleTarget` menu once; the chosen group is assigned as an
  `Attack` to every unplanned slot, then the round resolves. If exactly one
  group is alive, the menu is skipped and the round resolves immediately —
  there is no choice to present.

Both fill only *unplanned* slots. Pressing `A` three members into a round
never clobbers a choice already made deliberately.

Routing the target menu back to a party-wide fill needs one new `App` field
alongside `pending_battle_action`, recording that the menu was opened by `A`
rather than by a single slot's attack. `handle_battle_target_key`
(`app-core/src/lib.rs:979`) branches on it at the point where it currently
calls `commit_battle_action`.

**Where the party-wide commands live.** Jack out is a party-level command
hardcoded in both renderers, outside the `ActionOption` list. Adding two more
in the same style would mean three literal strings duplicated across two
renderers, against CLAUDE.md's rule that renderers never author action
strings.

Instead: add `Game::battle_party_commands() -> Vec<ActionOption>` returning
jack-out, all-attack and all-defend, and have both renderers append that list
to `view.options` the way they already draw the per-slot list. All-attack and
all-defend carry no `unavailable` reason; whether the target menu appears is
decided by `living_group_count()` at keypress time, not advertised in the
label. This also retires the existing jack-out duplication.

`battle_party_commands` supplies labels only. Key dispatch stays hand-written
in `handle_battle_key`, as jack-out's already is: the three commands do three
unrelated things, and there is no uniform "commit this action" path to route
them through the way `ActionOption`'s per-slot entries have. The engine owns
the strings; app-core owns the behaviour.

### 5. Base radius 15 → 7

`MAX_BUILD_DISTANCE_FROM_HOME` (`engine/src/lib.rs:300`) becomes `7`. The
platform slab stamped by `stamp_platform` (`lib.rs:4071`) goes 31×31 → 15×15.

The constant also serves as the danger origin: `distance_from_danger_origin`
(`lib.rs:4431`) subtracts it, so the platform counts as distance zero. The
first stat-escalation step therefore moves from 30 tiles from spawn to 22 —
danger ramps 8 tiles closer to home. This is intended.

`DISTANCE_STAT_STEP_TILES` (`lib.rs:70`) stays 15; it is a separate constant
that merely happened to match. The doc comment at `lib.rs:4429-4430` asserting
the two "are both 15" becomes false and is rewritten.

Every other reference to the constant is symbolic, including the tests at
`lib.rs:6849`, `:6861`, `:6887`, `:6945` and `:12717-12767`, so they follow
the new value automatically. `Platform` is not serialized (it is rebuilt from
the Home's position on load), so no save-format change.

### 6. Documentation

| Doc | What changes |
|---|---|
| `README.md:127-133` | intrusion key table: new keys, lowercase labels, `A`/`D` rows |
| `README.md:349` | "you page through what happened before planning the next one" is no longer true |
| `crates/tui/src/ui.rs:2118` | help line naming `a attack   d decompile` |
| `crates/gui/src/render.rs:2134` | same help line |
| `CHANGELOG.md` | entry for all four changes |

Out of scope, flagged rather than fixed: the help screens in both renderers
still describe the pre-roster combat model — `"c command a companion"` and
`"One command per round even with a full party"`
(`tui/src/ui.rs:2133-2136`, `gui/src/render.rs:2134`). That text was already
stale before this work. Only the key-naming lines this change actually
invalidates are corrected; the full rewrite is separate work.

CLAUDE.md is gitignored, so its intrusion-key references do not ship with this
branch either way.

## Testing

Reproducers first, per the repo's bug-fix rule.

**`app-core`:**

- `A` with two living groups routes to `Mode::BattleTarget`; picking a group
  plans every slot with an attack on it and resolves.
- `A` with one living group plans and resolves without ever entering
  `Mode::BattleTarget`.
- `D` fills only unplanned slots — a slot already set to something else keeps
  its choice.
- Committing the final slot's action lands in `Mode::Battle`, never a resolve
  screen. This replaces the `Mode::BattleResolve` assertions in the existing
  tests at `app-core/src/lib.rs:2413` and `:2440`.

**`engine`:**

- `battle_action_options` pins `d` to Defend and `c` to Decompile, so a future
  re-key cannot silently swap them.
- `battle_party_commands` offers all-attack, all-defend and jack-out.
- A resolved round logs exactly one `MessageKind::Round` line, numbered to
  match the planning header rather than the post-increment round.

No test may depend on wall-clock time or unseeded RNG. `cargo test
--workspace`, `cargo clippy --workspace` and `cargo fmt` are the gate before
this is called done.

## Not doing

- Populating a per-battle log separate from `MessageLog`. The global log is
  already on screen during a fight; a second one would be two sources of truth.
- Re-tuning `DISTANCE_STAT_STEP_TILES`, `PACK_SIZE_STEP_TILES` or any other
  danger dial to compensate for the smaller platform. The inward shift is the
  intent, and these dials are cheap to revisit after play-testing.
- Rewriting the stale combat help text beyond the lines this change
  invalidates.
