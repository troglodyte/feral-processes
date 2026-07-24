# Battle Flow and Base Radius Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the dismissible round-resolve popup, add shift-for-the-whole-party `[A]`/`[D]` battle commands, and shrink the base platform radius from 15 to 7.

**Architecture:** Party-wide planning is game logic, so it lives in the engine as `Game::battle_plan_remaining` and `Game::battle_party_commands`, not in app-core's key handler. This matters for testability: a bump-battle reachable from app-core is always exactly one enemy group and one party slot (verified empirically across seeds 0..200), so multi-slot and multi-group behaviour can only be asserted in the engine, where the private `insert_battle` test helper can construct them. app-core keeps only the key-to-command wiring.

**Tech Stack:** Rust, `bevy_ecs` (standalone), ratatui + crossterm (TUI), macroquad (GUI). 5-crate Cargo workspace.

## Global Constraints

- The engine's `Game` struct is the entire public API surface both renderers talk to via app-core. Neither renderer touches the ECS `World` directly.
- Renderers never author an action string. Every battle label comes from the engine.
- Comments explain *why*, never *what*. No `// removed` markers, no backwards-compat shims — if something is unused, delete it.
- No flaky tests: no `sleep()`, no wall-clock dependence, no unseeded RNG.
- Run `cargo fmt` and `cargo clippy --workspace` after every change; fix warnings rather than silencing them.
- `cargo test --workspace` is the final gate (200+ tests, ~1s).
- If many tests fail at once with `NotFound` on an assets path, that is stale build artifacts from the `petmud` directory rename, not real failures. Fix with `cargo clean -p feral-processes-engine -p feral-processes-app-core` — **not** a full `cargo clean`, `target/` is ~4 GB.
- Branch is `feat/battle-flow-and-base-radius`, already created off `main`. Two untracked files (`git-deep-dive-spelunking.sh`, `repo-hotspots.md`) predate this work — never stage them.

---

## File Structure

| File | Responsibility in this change |
|---|---|
| `crates/engine/src/resources.rs` | `MessageKind::Round` variant; delete `BattleState::log` |
| `crates/engine/src/battle.rs` | New `PartyCommand` / `PartyCommandKind` types |
| `crates/engine/src/lib.rs` | Round separator log line; re-keyed action options; `battle_plan_remaining`; `battle_party_commands`; delete `BattleView::log`; base radius constant |
| `crates/app-core/src/lib.rs` | Delete `Mode::BattleResolve` and `battle_log_mark`; route `A`/`D`/`j` through one party-command handler |
| `crates/tui/src/ui.rs` | Delete resolve overlay; style `MessageKind::Round`; draw party commands from the engine |
| `crates/gui/src/render.rs` | Same three, GUI-side |
| `crates/gui/src/lib.rs` | Drop `BattleResolve` from the `in_battle` check |
| `README.md`, `CHANGELOG.md` | Key table and behaviour claims |

---

### Task 1: Round separator in the message log

Replaces the popup's title as the thing that marks where one round ends and the next begins. Adding a `MessageKind` variant breaks the exhaustive `match` in both renderers, so the styling arms ship in this same task or the workspace will not compile.

**Files:**
- Modify: `crates/engine/src/resources.rs:32-38`
- Modify: `crates/engine/src/lib.rs:2897-2901` (`battle_resolve_round`)
- Modify: `crates/tui/src/ui.rs:21-28` (`message_style`)
- Modify: `crates/gui/src/render.rs:56-68` (`draw_message_line`)
- Test: `crates/engine/src/lib.rs` (existing `mod tests`)

**Interfaces:**
- Produces: `MessageKind::Round`, a new variant of the existing public enum re-exported at `crates/engine/src/lib.rs:43`.

- [ ] **Step 1: Write the failing test**

Add to the engine's `mod tests`, near the other battle-resolution tests (around `crates/engine/src/lib.rs:12875`):

```rust
/// The resolve popup used to title itself with the round number. With the
/// popup gone the log is the only place that boundary exists, so the
/// separator has to be logged exactly once and numbered to match the
/// planning header — not the post-increment round.
#[test]
fn resolving_a_round_logs_one_round_separator_numbered_for_the_round_that_ran() {
    let mut game = Game::new(77, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    insert_battle(&mut game, player, vec![wild]);

    let round_before = game.battle_view().unwrap().round;
    game.battle_set_action(0, BattleAction::Defend).unwrap();
    game.battle_resolve_round();

    let separators: Vec<String> = game
        .message_log(200)
        .into_iter()
        .filter(|(kind, _)| *kind == MessageKind::Round)
        .map(|(_, text)| text)
        .collect();
    assert_eq!(
        separators.len(),
        1,
        "one resolved round should log exactly one separator, got {separators:?}"
    );
    assert!(
        separators[0].contains(&round_before.to_string()),
        "the separator should name the round that just ran ({round_before}), got {:?}",
        separators[0]
    );
}
```

If `"glitch"` is not a species id in `assets/species/`, substitute the first id from `game.species_defs()` the way `crates/engine/src/lib.rs:10015` does. Check the directory before assuming.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p feral-processes-engine resolving_a_round_logs_one_round_separator -- --nocapture`
Expected: FAIL to compile with "no variant named `Round`".

- [ ] **Step 3: Add the `MessageKind::Round` variant**

`crates/engine/src/resources.rs:32`:

```rust
pub enum MessageKind {
    #[default]
    Info,
    Loot,
    LevelUp,
    Raid,
    /// Marks where one resolved battle round ends and the next begins.
    /// The battle screen's log pane is continuous, so without this the
    /// narration of six rounds reads as one undifferentiated block.
    Round,
}
```

- [ ] **Step 4: Log the separator**

In `battle_resolve_round` (`crates/engine/src/lib.rs:2897`), immediately after the guard clause and **before** initiative is rolled:

```rust
pub fn battle_resolve_round(&mut self) {
    if !self.battle_round_ready() || self.is_game_over().is_some() {
        return;
    }
    // Read before the increment at the end of this method, so the number
    // matches the planning screen's own "round N" header.
    let round = self.world.resource::<BattleState>().round;
    self.log_kind(MessageKind::Round, format!("── round {round} ──"));
    let player = self.world.resource::<BattleState>().player;
    // ... rest unchanged
```

- [ ] **Step 5: Add the styling arm in both renderers**

`crates/tui/src/ui.rs:22` — dim, so it separates without competing with the narration:

```rust
MessageKind::Round => Style::new().fg(Color::DarkGray),
```

`crates/gui/src/render.rs:57`:

```rust
MessageKind::Round => TEXT_DIM,
```

`TEXT_DIM` is already defined at `crates/gui/src/render.rs:25`.

- [ ] **Step 6: Run the test and the workspace build**

Run: `cargo test -p feral-processes-engine resolving_a_round_logs_one_round_separator`
Expected: PASS

Run: `cargo build --workspace`
Expected: clean — both renderers' exhaustive matches now cover `Round`.

- [ ] **Step 7: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/resources.rs crates/engine/src/lib.rs crates/tui/src/ui.rs crates/gui/src/render.rs
git commit -m "feat: log a round separator so battle narration stays readable in place"
```

---

### Task 2: Re-key and lowercase the action labels

Establishes the rule the party-wide commands depend on: a lowercase key acts for one member, its uppercase counterpart acts for the whole party. Defend moves `f` → `d` and Decompile moves `d` → `c` so that `a`/`A` and `d`/`D` are symmetric and no key sits one shift away from an unrelated action.

**Files:**
- Modify: `crates/engine/src/lib.rs:2811-2879` (`battle_action_options`)
- Modify: `crates/app-core/src/lib.rs:2292` (the `keys.contains(&'f')` assertion)
- Test: `crates/engine/src/lib.rs` (existing `mod tests`)

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: the key assignments `a`/`d`/`s`/`c`/`u` that Tasks 5 and 6 rely on.

- [ ] **Step 1: Write the failing test**

Add to the engine's `mod tests`:

```rust
/// Uppercase A and D became party-wide commands, which only works if the
/// per-slot keys underneath them are Attack and Defend. Decompile moved off
/// `d` to make room. Pinned here so a future re-key cannot silently swap a
/// brace for a capture attempt that spends a taming catalyst.
#[test]
fn battle_action_keys_are_lowercase_with_defend_on_d_and_decompile_on_c() {
    let mut game = Game::new(78, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    insert_battle(&mut game, player, vec![wild]);

    let options = game.battle_action_options(0);
    let key_for = |kind: ActionKind| {
        options
            .iter()
            .find(|o| o.kind == kind)
            .unwrap_or_else(|| panic!("the player's menu should offer {kind:?}"))
            .key
    };
    assert_eq!(key_for(ActionKind::Attack), 'a');
    assert_eq!(key_for(ActionKind::Defend), 'd');
    assert_eq!(key_for(ActionKind::Decompile), 'c');
    assert_eq!(key_for(ActionKind::UseItem), 'u');

    for option in &options {
        assert!(
            option.label.contains(&format!("[{}]", option.key)),
            "{:?} advertises key {:?} but its label is {:?} — the bracketed \
             letter must be the lowercase key the player actually presses",
            option.kind,
            option.key,
            option.label
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p feral-processes-engine battle_action_keys_are_lowercase`
Expected: FAIL — `assert_eq!(key_for(ActionKind::Defend), 'd')` gets `'f'`.

- [ ] **Step 3: Re-key and relabel**

In `battle_action_options` (`crates/engine/src/lib.rs:2811`), change five `key`/`label` pairs. Everything else in each `ActionOption` — `kind`, `detail`, `target`, `unavailable` — is untouched.

| Location | `key` | `label` |
|---|---|---|
| `lib.rs:2819-2820` | `'a'` | `"[a]ttack"` |
| `lib.rs:2827-2828` | `'d'` (was `'f'`) | `"[d]efend"` (was `"De[f]end"`) |
| `lib.rs:2838-2839` | `'s'` | `"[s]pecial"` |
| `lib.rs:2852-2853` | `'c'` (was `'d'`) | `"de[c]ompile"` (was `"[D]ecompile"`) |
| `lib.rs:2867-2868` | `'u'` | `"[u]se item"` |

- [ ] **Step 4: Update the stale assertion in app-core**

`crates/app-core/src/lib.rs:2292` still asserts Defend is on `f`:

```rust
assert!(
    keys.contains(&'a') && keys.contains(&'d'),
    "the engine should always offer at least Attack and Defend, got {keys:?}"
);
```

Leave the rest of that test alone — Task 5 rewrites it properly.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p feral-processes-engine battle_action_keys_are_lowercase`
Expected: PASS

Run: `cargo test -p feral-processes-app-core battle`
Expected: PASS — app-core reads keys from the engine, so the re-key flows through.

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/lib.rs crates/app-core/src/lib.rs
git commit -m "feat: lowercase battle action keys, moving defend to d and decompile to c"
```

---

### Task 3: `Game::battle_plan_remaining`

The party-wide fill itself. Lives in the engine both because it is game logic and because it is the only place a multi-slot party can be constructed in a test.

**Files:**
- Modify: `crates/engine/src/lib.rs` (add next to `battle_set_action` at `:2747`)
- Test: `crates/engine/src/lib.rs` (existing `mod tests`)

**Interfaces:**
- Consumes: `Game::battle_set_action(slot: usize, action: BattleAction) -> Result<(), String>`, `Game::slot_can_act(slot: usize) -> bool` (private, `crates/engine/src/lib.rs`).
- Produces: `pub fn battle_plan_remaining(&mut self, action: BattleAction) -> Result<(), String>` — used by Task 6.

- [ ] **Step 1: Write the failing tests**

Two tests. The first is the core contract; the second guards the knocked-out-slot invariant that commit `fe3fcde` fixed — `battle_active_slot` and `battle_round_ready` both skip slots failing `slot_can_act`, so filling one would put an action on a member that cannot take it.

```rust
/// `[A]`/`[D]` fill the party in one keypress, but must never overwrite a
/// choice the player already made deliberately — they pressed it partway
/// through planning, not before starting.
#[test]
fn battle_plan_remaining_fills_only_unplanned_slots() {
    let mut game = Game::new(79, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let species = game.species_defs().into_iter().next().unwrap().id;
    let companion = game.spawn_wild_creature(&species, 4, 5).unwrap();
    game.world.resource_mut::<Party>().0.push(companion);
    let wild = game.spawn_wild_creature(&species, 5, 5).unwrap();
    insert_battle(&mut game, player, vec![wild]);

    // Slot 0 (the player) picks for itself; slot 1 is left open.
    game.battle_set_action(0, BattleAction::Attack { group: 0 })
        .unwrap();
    game.battle_plan_remaining(BattleAction::Defend).unwrap();

    let planned = &game.world.resource::<BattleState>().planned;
    assert_eq!(
        planned[0],
        Some(BattleAction::Attack { group: 0 }),
        "the slot that was already planned must keep its own choice"
    );
    assert_eq!(
        planned[1],
        Some(BattleAction::Defend),
        "the open slot should have been filled"
    );
    assert!(game.battle_round_ready(), "every actionable slot is planned");
}

/// A knocked-out companion's slot is skipped by `battle_active_slot` and
/// doesn't block `battle_round_ready`. Filling it would hand an action to a
/// member that can't take one.
#[test]
fn battle_plan_remaining_skips_a_slot_that_cannot_act() {
    let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let species = game.species_defs().into_iter().next().unwrap().id;
    let companion = game.spawn_wild_creature(&species, 4, 5).unwrap();
    game.world.resource_mut::<Party>().0.push(companion);
    let wild = game.spawn_wild_creature(&species, 5, 5).unwrap();
    insert_battle(&mut game, player, vec![wild]);

    // Drop the companion, so slot 1 can no longer act.
    game.world.get_mut::<Stats>(companion).unwrap().hp = 0;
    assert!(
        !game.slot_can_act(1),
        "a companion at 0 HP should not be able to act — test premise is wrong"
    );

    game.battle_plan_remaining(BattleAction::Defend).unwrap();

    let planned = &game.world.resource::<BattleState>().planned;
    assert_eq!(planned[0], Some(BattleAction::Defend));
    assert_eq!(
        planned[1], None,
        "a slot that can't act must stay unplanned, not be handed an action"
    );
}
```

Before running: confirm `slot_can_act` is the actual predicate name and that a 0-HP companion makes it false. Read it at `crates/engine/src/lib.rs:2738` and follow the definition. If knocking a companion out needs something other than setting `Stats::hp` to 0, use whatever that path actually is rather than forcing the field.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p feral-processes-engine battle_plan_remaining`
Expected: FAIL to compile with "no method named `battle_plan_remaining`".

- [ ] **Step 3: Implement**

Add directly below `battle_set_action` (`crates/engine/src/lib.rs:2768`):

```rust
/// Assigns `action` to every slot that is still unplanned and able to act
/// — the party-wide `[A]`/`[D]` commands. Slots that already hold a choice
/// keep it, and a slot failing `slot_can_act` (a knocked-out companion) is
/// left alone, matching what `battle_active_slot` would have skipped.
pub fn battle_plan_remaining(&mut self, action: BattleAction) -> Result<(), String> {
    let Some(battle) = self.world.get_resource::<BattleState>() else {
        return Err("No active intrusion.".to_string());
    };
    let open: Vec<usize> = (0..battle.planned.len())
        .filter(|&slot| battle.planned[slot].is_none())
        .collect();
    for slot in open {
        if !self.slot_can_act(slot) {
            continue;
        }
        self.battle_set_action(slot, action.clone())?;
    }
    Ok(())
}
```

Validation (a dead target group, a slot out of range) is not duplicated — `battle_set_action` already does it and `?` propagates the same `String` the single-slot path returns.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p feral-processes-engine battle_plan_remaining`
Expected: PASS (2 tests)

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/lib.rs
git commit -m "feat: add battle_plan_remaining for party-wide round planning"
```

---

### Task 4: `PartyCommand` and `Game::battle_party_commands`

Jack out is currently a literal string hardcoded in both renderers, outside the `ActionOption` list. Adding all-attack and all-defend in that style would mean three literals duplicated across two renderers, against the rule that renderers never author action strings. One engine-owned list fixes all three.

These are deliberately **not** `ActionOption`s: they never become a `BattleAction` for one slot, so sharing `ActionKind` would force meaningless arms into `action_from` and `resolve_one_action`.

Whether all-attack needs a target is decided here, by the engine, rather than in app-core — that keeps the decision testable where a multi-group pack can actually be built.

**Files:**
- Modify: `crates/engine/src/battle.rs` (add after `ActionOption` at `:81`)
- Modify: `crates/engine/src/lib.rs` (add after `battle_action_options` at `:2879`; extend the `pub use` at `:43` region as needed)
- Test: `crates/engine/src/lib.rs` (existing `mod tests`)

**Interfaces:**
- Consumes: `Game::living_group_count(&self) -> usize` (private, `crates/engine/src/lib.rs:2631`).
- Produces:
  - `pub enum PartyCommandKind { AllAttack, AllDefend, JackOut }`
  - `pub struct PartyCommand { pub kind: PartyCommandKind, pub key: char, pub label: String, pub needs_target: bool }`
  - `pub fn battle_party_commands(&self) -> Vec<PartyCommand>`

  Task 6 matches on `kind` and branches on `needs_target`; both renderers draw `label`.

- [ ] **Step 1: Write the failing test**

```rust
/// All-attack asks which group only when there is a choice to make. With a
/// single group left the prompt is pure friction, which is the whole
/// complaint this work started from.
#[test]
fn all_attack_needs_a_target_only_while_more_than_one_group_lives() {
    let mut game = Game::new(82, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let mut species = game.species_defs().into_iter().map(|s| s.id);
    let first = species.next().unwrap();
    let second = species.next().expect("assets ship at least two species");

    let solo = game.spawn_wild_creature(&first, 5, 5).unwrap();
    insert_battle(&mut game, player, vec![solo]);
    let needs = |game: &Game| {
        game.battle_party_commands()
            .into_iter()
            .find(|c| c.kind == PartyCommandKind::AllAttack)
            .expect("all-attack should always be offered")
            .needs_target
    };
    assert!(
        !needs(&game),
        "one group means no choice, so all-attack shouldn't open a picker"
    );

    let a = game.spawn_wild_creature(&first, 5, 5).unwrap();
    let b = game.spawn_wild_creature(&second, 6, 5).unwrap();
    insert_battle(&mut game, player, vec![a, b]);
    assert_eq!(
        game.battle_view().unwrap().groups.len(),
        2,
        "two different species should partition into two groups — test premise"
    );
    assert!(
        needs(&game),
        "two groups means a real focus-fire choice, so all-attack must ask"
    );
}

/// The renderers draw this list verbatim instead of hardcoding strings.
#[test]
fn battle_party_commands_offers_all_attack_all_defend_and_jack_out() {
    let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let species = game.species_defs().into_iter().next().unwrap().id;
    let wild = game.spawn_wild_creature(&species, 5, 5).unwrap();
    insert_battle(&mut game, player, vec![wild]);

    let commands = game.battle_party_commands();
    let keys: Vec<char> = commands.iter().map(|c| c.key).collect();
    assert_eq!(
        keys,
        vec!['A', 'D', 'j'],
        "uppercase for the party-wide pair, lowercase for jack out"
    );
    for command in &commands {
        assert!(
            command.label.contains(&format!("[{}]", command.key)),
            "{:?} advertises key {:?} but its label is {:?}",
            command.kind,
            command.key,
            command.label
        );
    }
}
```

Check `assets/species/` actually ships two or more `.ron` files before relying on `species.next()` twice. It should — confirm rather than assume.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p feral-processes-engine party_command`
Expected: FAIL to compile — the types do not exist.

- [ ] **Step 3: Add the types**

`crates/engine/src/battle.rs`, after `ActionOption` (`:81`):

```rust
/// A command that applies to the whole party at once rather than to the slot
/// currently choosing. Deliberately not an `ActionOption`: these never
/// become one slot's `BattleAction`, so sharing `ActionKind` would force
/// meaningless arms into `action_from` and `Game::resolve_one_action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyCommandKind {
    AllAttack,
    AllDefend,
    JackOut,
}

/// One party-level entry in the battle action bar. Renderers draw this
/// verbatim, exactly as they do `ActionOption`.
#[derive(Debug, Clone)]
pub struct PartyCommand {
    pub kind: PartyCommandKind,
    /// Uppercase for the party-wide pair, so shift reads as "everyone does
    /// this" against the lowercase per-slot keys.
    pub key: char,
    pub label: String,
    /// Whether the UI must collect an enemy group before this can run.
    /// All-attack sets it only while more than one group is alive.
    pub needs_target: bool,
}
```

- [ ] **Step 4: Add the accessor**

`crates/engine/src/lib.rs`, after `battle_action_options` (`:2879`):

```rust
/// The party-level commands, which apply to every slot at once instead of
/// to `battle_active_slot`. Kept here rather than as renderer literals so
/// the two frontends cannot drift — the same reason
/// `battle_action_options` exists.
pub fn battle_party_commands(&self) -> Vec<PartyCommand> {
    vec![
        PartyCommand {
            kind: PartyCommandKind::AllAttack,
            key: 'A',
            label: "[A]ll attack".to_string(),
            needs_target: self.living_group_count() > 1,
        },
        PartyCommand {
            kind: PartyCommandKind::AllDefend,
            key: 'D',
            label: "[D] all defend".to_string(),
            needs_target: false,
        },
        PartyCommand {
            kind: PartyCommandKind::JackOut,
            key: 'j',
            label: "[j]ack out".to_string(),
            needs_target: false,
        },
    ]
}
```

Note the all-attack label is `"[A]ll attack"` — the bracketed letter is the key and reads as part of the word. The test's `label.contains("[A]")` holds.

`battle` is already `pub mod` (`crates/engine/src/lib.rs:2`), so declaring the two types `pub` in `battle.rs` is the whole export — there is no crate-root re-export to add. `lib.rs` needs them in scope; extend the import at `crates/engine/src/lib.rs:26`:

```rust
use battle::{
    ActionKind, ActionOption, BattleAction, EnemyGroup, PartyCommand, PartyCommandKind, TargetSpec,
};
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p feral-processes-engine party_command`
Expected: PASS (2 tests)

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/battle.rs crates/engine/src/lib.rs
git commit -m "feat: engine-owned party command list for jack out and the party-wide pair"
```

---

### Task 5: Delete the resolve popup end-to-end

`Mode::BattleResolve` and both renderer overlays go in one commit — deleting the variant alone breaks two exhaustive matches.

The popup never showed anything: `BattleState::log` is written by nobody. It is initialized empty at `crates/engine/src/lib.rs:2608` and in the `insert_battle` helper at `:6111`, cloned into `BattleView::log` at `:3166`, and that is the whole lifecycle. All narration goes through `Game::log()` into `MessageLog`, which both battle screens already draw (`crates/tui/src/ui.rs:1825`, `crates/gui/src/render.rs:1920`). So the overlay always rendered its `"The round passes quietly."` fallback over a screen that was already showing the real thing. Both dead fields go with it.

**Files:**
- Modify: `crates/app-core/src/lib.rs` — `:181-183`, `:298-301`, `:375`, `:588`, `:1036-1045`, `:1059`, `:2365`, `:2413`, `:2440`
- Modify: `crates/engine/src/resources.rs:135`
- Modify: `crates/engine/src/lib.rs` — `:536`, `:2608`, `:3166`, `:6111`
- Modify: `crates/tui/src/ui.rs` — `:50-53`, `:1939-1973`
- Modify: `crates/gui/src/render.rs` — `:113-116`, `:1999-2025`, `:2160`
- Modify: `crates/gui/src/lib.rs:164-167`

**Interfaces:**
- Consumes: nothing from Tasks 1–4.
- Produces: `commit_battle_action` now leaves `App::mode` as `Mode::Battle` (fight continues) or `Mode::Playing` (fight over) — never a third state. Task 6 builds on that.

- [ ] **Step 1: Update the two tests that assert the popup**

These are the reproducers: they currently pass *because* of the popup, and must fail once it is asserted gone.

`crates/app-core/src/lib.rs:2410-2417` becomes:

```rust
assert!(
    matches!(app.mode, Mode::Battle | Mode::Playing | Mode::GameOver),
    "the only slot was planned, so the round should have resolved straight \
     back into planning; got {:?}",
    app.mode
);
```

`crates/app-core/src/lib.rs:2422-2445` — rename the test and make the same swap:

```rust
/// A solo player is a one-slot party, so choosing an untargeted action
/// completes the round immediately and drops straight back into planning.
/// No narration page in between: the battle screen's log pane already shows
/// what happened.
#[test]
fn completing_every_slot_resolves_the_round_without_a_narration_page() {
    let mut app = battling_app();
    let slots = app
        .game
        .as_ref()
        .unwrap()
        .battle_view()
        .unwrap()
        .party
        .len();
    assert_eq!(slots, 1, "the test seed's player starts with no companions");

    app.handle_key(GameKey::Char('d'));

    assert!(
        matches!(app.mode, Mode::Battle | Mode::Playing | Mode::GameOver),
        "the only slot was planned, so the round should have resolved straight \
         back into planning; got {:?}",
        app.mode
    );
}
```

Note the key changed from `'f'` to `'d'` — Task 2 moved Defend.

Also drop the `Mode::BattleResolve` arm from the match list at `crates/app-core/src/lib.rs:2365`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-app-core completing_every_slot`
Expected: FAIL — the app is in `Mode::BattleResolve`, which is no longer accepted.

- [ ] **Step 3: Delete the mode and its handler in app-core**

- Remove the `BattleResolve` variant and its doc comment (`:181-183`).
- Remove the `battle_log_mark` field and its doc comment (`:298-301`) and its initializer (`:375`).
- Remove the dispatch arm (`:588`).
- Remove `handle_battle_resolve_key` entirely (`:1036-1045`).
- In `commit_battle_action`, delete the `battle_log_mark` assignment (`:1059`) and collapse the mode choice:

```rust
game.battle_resolve_round();
let still_active = game.has_active_battle();
self.mode = if still_active {
    Mode::Battle
} else {
    Mode::Playing
};
self.push_battle_outcome_sounds(SoundEvent::Attack, still_active);
```

- [ ] **Step 4: Delete the dead log fields in the engine**

- `crates/engine/src/resources.rs:135` — remove `pub log: Vec<String>,` from `BattleState`.
- `crates/engine/src/lib.rs:536` — remove `pub log: Vec<String>,` from `BattleView`.
- `crates/engine/src/lib.rs:2608` and `:6111` — remove the `log: Vec::new(),` initializers.
- `crates/engine/src/lib.rs:3166` — remove `log: battle.log.clone(),`.

`BattleState` is not serialized (nothing in `crates/engine/src/save.rs` references it), so this is not a save-format change. Confirm with `rg -n "BattleState" crates/engine/src/save.rs` before committing.

- [ ] **Step 5: Delete both renderer overlays**

- `crates/tui/src/ui.rs:50-53` — remove the `Mode::BattleResolve` match arm.
- `crates/tui/src/ui.rs:1939-1973` — remove `render_battle_resolve` and its doc comment.
- `crates/gui/src/render.rs:113-116` — remove the match arm.
- `crates/gui/src/render.rs:1999-2025` — remove `draw_battle_resolve` and its doc comment.
- `crates/gui/src/render.rs:2160` — remove `Mode::BattleResolve,` from the `every_mode_that_covers_the_log_pane_gets_the_status_banner` list.
- `crates/gui/src/lib.rs:164-167` — the `in_battle` check becomes:

```rust
let in_battle = matches!(app.mode, Mode::Battle | Mode::BattleTarget);
```

`needs_status_banner` (`crates/gui/src/render.rs:75`) is a negated match on three modes and does not name `BattleResolve`, so it needs no change.

- [ ] **Step 6: Run the full suite**

Run: `cargo test --workspace`
Expected: PASS. If the compiler still reports `BattleResolve`, one reference was missed — `rg -n "BattleResolve|battle_log_mark" crates/` should come back empty.

- [ ] **Step 7: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/
git commit -m "fix: drop the round-resolve popup, which paged over an always-empty log

BattleState::log was never written to, so the overlay always rendered its
empty-log fallback on top of a battle screen that was already showing the
real narration from MessageLog. Removing it takes the dead BattleView::log
and BattleState::log fields with it."
```

---

### Task 6: Wire `A` / `D` / `j` through one party-command handler

`handle_battle_key` lowercases every key at `crates/app-core/src/lib.rs:929`, so `A` and `a` are indistinguishable today. Matching party commands by exact char **before** that fold makes uppercase meaningful without touching the per-slot path, and folds the special-cased `if c == 'j'` block into the same lookup.

The match order is what makes this work:

| Pressed | Exact match in `['A','D','j']` | Lowercased match | Result |
|---|---|---|---|
| `A` | AllAttack | — | party-wide attack |
| `a` | none | none | per-slot Attack |
| `D` | AllDefend | — | party-wide defend |
| `d` | none | none | per-slot Defend |
| `j` | JackOut | — | flee |
| `J` | none | JackOut | flee |
| `S`/`C`/`U` | none | none | folds to per-slot, as before |

**Files:**
- Modify: `crates/app-core/src/lib.rs` — `:295-297` (new field), `:374` (init), `:916-976` (`handle_battle_key`), `:978-1009` (`handle_battle_target_key`)
- Modify: `crates/tui/src/ui.rs:1841-1856`, `crates/gui/src/render.rs:1928-1937`
- Test: `crates/app-core/src/lib.rs` (existing `mod tests`)

**Interfaces:**
- Consumes: `Game::battle_party_commands()`, `PartyCommand`, `PartyCommandKind` (Task 4); `Game::battle_plan_remaining` (Task 3).
- Produces: nothing later tasks depend on.

- [ ] **Step 1: Write the failing tests**

Remember the constraint from the architecture note: a bump-battle from `battling_app()` is always exactly one group and one party slot. These tests therefore cover the single-group wiring and the no-swallow rule; the multi-group and multi-slot behaviour is already covered by Tasks 3 and 4 in the engine, where those states can be built.

```rust
/// The complaint that started this work: with one group left there is no
/// focus-fire choice to make, so all-attack must resolve on the single
/// keypress instead of stopping to ask.
#[test]
fn all_attack_with_one_group_resolves_without_opening_the_target_picker() {
    let mut app = battling_app();
    assert_eq!(
        app.game.as_mut().unwrap().battle_view().unwrap().groups.len(),
        1,
        "a bump battle is a single group — test premise"
    );

    app.handle_key(GameKey::Char('A'));

    assert_ne!(
        app.mode,
        Mode::BattleTarget,
        "one group means no choice, so all-attack shouldn't open the picker"
    );
    assert!(
        app.pending_battle_action.is_none(),
        "nothing should be left pending once the round resolved"
    );
}

/// `D` is a party-wide command, not the per-slot Defend that `d` runs.
/// Both have to reach the engine.
#[test]
fn all_defend_resolves_the_round() {
    let mut app = battling_app();
    app.handle_key(GameKey::Char('D'));
    assert!(
        matches!(app.mode, Mode::Battle | Mode::Playing | Mode::GameOver),
        "all-defend plans every slot, so the round should have resolved; got {:?}",
        app.mode
    );
}
```

Then **rewrite** `battle_action_keys_come_from_the_engine_and_ignore_case` (`crates/app-core/src/lib.rs:2267-2311`). Its premise — "the prompt says `[A]ttack`, so a player has every reason to hold Shift, and case is normalized" — is exactly what this change inverts. The doc comment above it must be rewritten too, not just the body:

```rust
/// The action set lives in the engine. If app-core or a renderer hardcoded
/// a key, the two frontends would drift the moment an action was added —
/// which is the exact failure this indirection exists to prevent. So the
/// keys under test are read from the engine rather than written here.
///
/// Case handling is deliberately split. The per-slot prompts are lowercase
/// (`[a]ttack`, `[d]efend`), and uppercase `A`/`D` are the party-wide
/// commands — so those two must NOT fold. Every other battle key still
/// folds, since a shifted keypress there is a slip, and swallowing it costs
/// the player a round.
///
/// Asserts only that each key was routed at all — which action it resolves
/// to is the engine's business, and depends on the gear and party the seed
/// happens to hand out.
#[test]
fn battle_action_keys_come_from_the_engine_with_only_the_party_pair_case_sensitive() {
    let probe = battling_app();
    let game = probe.game.as_ref().unwrap();
    let per_slot: Vec<char> = game
        .battle_action_options(0)
        .iter()
        .map(|o| o.key)
        .collect();
    assert!(
        per_slot.contains(&'a') && per_slot.contains(&'d'),
        "the engine should always offer at least Attack and Defend, got {per_slot:?}"
    );
    let party: Vec<char> = game
        .battle_party_commands()
        .iter()
        .map(|c| c.key)
        .collect();
    assert_eq!(party, vec!['A', 'D', 'j']);

    // Every key the engine advertises must route as pressed, and the
    // shifted form of each lowercase one must route too.
    let mut probes: Vec<char> = per_slot.clone();
    probes.extend(per_slot.iter().map(|k| k.to_ascii_uppercase()));
    probes.extend(party.iter().copied());
    probes.push('J');

    for key in probes {
        let mut app = battling_app();
        app.handle_key(GameKey::Char(key));
        let acted =
            !app.take_sounds().is_empty() || app.status_line.is_some() || app.mode != Mode::Battle;
        assert!(
            acted,
            "[{key}] is advertised by the engine, but the keypress was swallowed"
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p feral-processes-app-core all_attack_with_one_group`
Expected: FAIL — `A` currently folds to `a`, opening the target picker.

- [ ] **Step 3: Add the pending-party-command field**

`crates/app-core/src/lib.rs`, next to `pending_battle_action` (`:295-297`):

```rust
/// Set when `Mode::BattleTarget` was opened by the party-wide `[A]ll
/// attack` rather than by one slot's Attack — the group picked then plans
/// every open slot instead of just `battle_active_slot`.
pub pending_party_attack: bool,
```

Initialize it to `false` in `App::new` (`:374`, alongside `pending_battle_action: None`).

- [ ] **Step 4: Match party commands before the case fold**

Replace the top of `handle_battle_key` (`crates/app-core/src/lib.rs:920-942`) — everything from the `GameKey::Char` binding through the end of the `if c == 'j'` block:

```rust
fn handle_battle_key(&mut self, key: GameKey) {
    if key == GameKey::Esc {
        self.battle_back_up();
        return;
    }
    let GameKey::Char(raw) = key else { return };

    // Party-wide commands are matched on the raw char first, so uppercase
    // `A`/`D` stay distinct from the lowercase per-slot Attack/Defend. The
    // lowercase retry is what lets a shifted `J` still jack out; `a`/`d`
    // can't match it, so they fall through to the per-slot menu below.
    let party = self
        .game
        .as_ref()
        .map(|g| g.battle_party_commands())
        .unwrap_or_default();
    if let Some(command) = party
        .iter()
        .find(|c| c.key == raw)
        .or_else(|| party.iter().find(|c| c.key == raw.to_ascii_lowercase()))
    {
        self.run_party_command(command.kind, command.needs_target);
        return;
    }

    // Every other battle key folds case: the prompt's bracketed letter is
    // lowercase, so a shifted press is a slip, and swallowing it would cost
    // the player a round.
    let c = raw.to_ascii_lowercase();

    let Some(game) = &self.game else { return };
    // ... the rest of the method is unchanged, starting at the
    // `battle_active_slot` lookup
}
```

- [ ] **Step 5: Add `run_party_command`**

Directly below `handle_battle_key`:

```rust
/// Runs a party-level command: jacking out, or planning every open slot at
/// once. All-attack defers to `Mode::BattleTarget` when the engine says
/// there is more than one group to choose between.
fn run_party_command(&mut self, kind: PartyCommandKind, needs_target: bool) {
    match kind {
        PartyCommandKind::JackOut => {
            // Bound inside the arm, not before the match: the other two arms
            // call `&mut self` methods, and a `game` borrow held across the
            // match would collide with them.
            let Some(game) = &mut self.game else { return };
            game.battle_flee();
            let still_active = game.has_active_battle();
            if !still_active {
                self.mode = Mode::Playing;
            }
            self.push_battle_outcome_sounds(SoundEvent::Flee, still_active);
        }
        PartyCommandKind::AllDefend => self.plan_every_slot(BattleAction::Defend),
        PartyCommandKind::AllAttack => {
            if needs_target {
                self.pending_party_attack = true;
                self.menu_selected = 0;
                self.mode = Mode::BattleTarget;
            } else {
                self.plan_every_slot(BattleAction::Attack { group: 0 });
            }
        }
    }
}

/// Plans every open slot with `action` and resolves the round. The
/// resolve-and-transition tail matches `commit_battle_action`'s, since a
/// full party is a full party however it got there.
fn plan_every_slot(&mut self, action: BattleAction) {
    let Some(game) = &mut self.game else { return };
    if let Err(reason) = game.battle_plan_remaining(action) {
        self.status_line = Some(reason);
        return;
    }
    if !game.battle_round_ready() {
        self.mode = Mode::Battle;
        return;
    }
    game.battle_resolve_round();
    let still_active = game.has_active_battle();
    self.mode = if still_active {
        Mode::Battle
    } else {
        Mode::Playing
    };
    self.push_battle_outcome_sounds(SoundEvent::Attack, still_active);
}
```

`battle_flee` was previously called from the inlined `if c == 'j'` block; that block is gone, replaced by this arm. Confirm the signature at `crates/app-core/src/lib.rs:935` before wiring it.

Extend app-core's battle import (`crates/app-core/src/lib.rs:13`):

```rust
use feral_processes_engine::battle::{ActionKind, BattleAction, PartyCommandKind, TargetSpec};
```

- [ ] **Step 6: Route the target picker back to the party fill**

In `handle_battle_target_key` (`crates/app-core/src/lib.rs:979`), the Esc branch must clear the new flag, and the pick must branch on it. Replace the Esc guard:

```rust
if key == GameKey::Esc {
    self.pending_battle_action = None;
    self.pending_party_attack = false;
    self.mode = Mode::Battle;
    return;
}
```

and replace the tail, from the `let Some(group) = picked else { return };` line (`:1000`) onward:

```rust
let Some(group) = picked else { return };
if self.pending_party_attack {
    self.pending_party_attack = false;
    self.plan_every_slot(BattleAction::Attack { group });
    return;
}
let Some(kind) = self.pending_battle_action else {
    return;
};
let Some(action) = action_from(kind, Some(group), None) else {
    return;
};
self.pending_battle_action = None;
self.commit_battle_action(slot, action);
```

The `let Some(slot) = view.active_slot else { return };` guard earlier in the method (`:989`) sits above this. Check whether it can reject a party-wide pick — if `active_slot` could be `None` while a party attack is pending, move the party branch above that guard.

- [ ] **Step 7: Draw the party commands from the engine in both renderers**

`crates/tui/src/ui.rs:1855-1856` — replace the hardcoded Jack Out push:

```rust
// Party-level commands come from the engine too, so the two renderers
// cannot drift on them either.
for command in game.battle_party_commands() {
    actions.push(Span::styled(command.label, Style::new().fg(Color::Cyan)));
    actions.push(Span::raw("   "));
}
```

This sits after the `view.options` `flat_map` that builds `actions`. Note `view` borrows `game`; if the borrow checker objects, collect the commands into a local before building `actions` rather than reaching for `.clone()`.

`crates/gui/src/render.rs:1936-1937` — replace the hardcoded push:

```rust
actions.extend(game.battle_party_commands().into_iter().map(|c| c.label));
```

- [ ] **Step 8: Run the tests**

Run: `cargo test -p feral-processes-app-core battle`
Expected: PASS, including the two new tests and the rewritten key test.

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 9: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/
git commit -m "feat: [A]ll attack and [D] all defend plan the whole party in one keypress"
```

---

### Task 7: Base radius 15 → 7

**Files:**
- Modify: `crates/engine/src/lib.rs:300` (the constant), `:4425-4430` (the doc comment that goes stale)

**Interfaces:** none — every other reference to the constant is symbolic and follows automatically.

- [ ] **Step 1: Change the constant**

`crates/engine/src/lib.rs:300`:

```rust
const MAX_BUILD_DISTANCE_FROM_HOME: i32 = 7;
```

- [ ] **Step 2: Fix the doc comment this falsifies**

`crates/engine/src/lib.rs:4425-4430` currently ends with "the build radius and `DISTANCE_STAT_STEP_TILES` are both 15", which is now wrong. `DISTANCE_STAT_STEP_TILES` (`crates/engine/src/lib.rs:70`) stays 15 — it is a separate constant that merely happened to match. Rewrite the tail:

```rust
/// Chebyshev distance from `(x, y)` to the edge of safe territory: the
/// platform's edge once a Home exists, the bare `ZoneSpawnPoint` before
/// then. Both danger curves measure from this rather than straight from
/// the spawn point, so the whole base counts as distance zero instead of
/// sitting part-way up the first escalation step. The build radius (7) and
/// `DISTANCE_STAT_STEP_TILES` (15) are independent dials: shrinking the
/// platform pulls the first step inward, to 22 tiles from spawn.
```

- [ ] **Step 3: Run the distance and platform tests**

Run: `cargo test -p feral-processes-engine distance_stat_multiplier`
Run: `cargo test -p feral-processes-engine max_pack_size`
Run: `cargo test -p feral-processes-engine platform`
Expected: PASS — these reference the constant symbolically (`crates/engine/src/lib.rs:6849`, `:6861`, `:6887`, `:6945`, `:12717-12767`).

- [ ] **Step 4: Run the full suite**

Run: `cargo test --workspace`
Expected: PASS. A failure here most likely means a test hardcoded a coordinate that assumed a 31×31 slab — fix the test to derive from the constant rather than re-hardcoding 7.

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/lib.rs
git commit -m "balance: shrink the base platform from a 15-tile radius to 7

The constant also anchors the danger curve, so the first stat-escalation
step moves from 30 tiles out to 22. Intended: a smaller base means hostiles
get tougher nearer to home."
```

---

### Task 8: Documentation

The repo rule is that `assets/*/README.md` is not the whole doc obligation — grep the root README for claims a change falsifies.

**Files:**
- Modify: `README.md:127-133`, `README.md:345-351`
- Modify: `crates/tui/src/ui.rs:2118`
- Modify: `crates/gui/src/render.rs:2134`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Update the README key table**

`README.md:127-133` currently lists `a`/`f`/`s`/`d`/`u`/`j` with capitalized labels. Replace the table body with:

```markdown
| Key | Action |
| --- | --- |
| `a` | Attack — then pick which enemy group to hit |
| `d` | Defend — brace for the round: a Defense bonus, and you draw more of the incoming fire |
| `s` | Special (party members only) — a rally (ATK boost) by default, or the species' own ability if it has one. Costs you a flat chunk of Fatigue |
| `c` | Decompile (you only) — attempt to compile/tame a group's front program. Needs a taming catalyst, which the ICE Breaker is |
| `u` | Use item (you only) — spend a consumable as that slot's action for the round |
| `j` | Jack out (flee) — costs a mild XP setback, same as flatlining. A party-level command, not a per-member action |
| `A` | All attack — every unplanned slot attacks. Asks which group only if more than one is left |
| `D` | All defend — every unplanned slot braces |
```

Add a line under the table noting the rule: a lowercase key acts for the member currently choosing; its uppercase counterpart acts for the whole party.

- [ ] **Step 2: Fix the README claim the popup removal falsifies**

`README.md:349` currently ends "...the round resolves in initiative order, and you page through what happened before planning the next one." Drop the paging clause — narration now lands in the log pane on the battle screen itself, separated by a dim `── round N ──` line.

- [ ] **Step 3: Fix the help lines in both renderers**

`crates/tui/src/ui.rs:2118` reads `"In an intrusion:  a attack   d decompile (needs a taming catalyst)"`. Decompile is `c` now:

```rust
Line::from("In an intrusion:  a attack   d defend   c decompile (needs a taming catalyst)"),
```

`crates/gui/src/render.rs:2134` reads `"In an intrusion:  a attack   d decompile   c command companion   j jack out"`:

```rust
text_row("In an intrusion:  a attack   d defend   c decompile   j jack out"),
```

Add a line to each naming the party-wide pair, immediately below the line above. TUI (`Line::from`, matching its neighbours):

```rust
Line::from("                  A all attack   D all defend   u use item   j jack out"),
```

GUI (`text_row`, matching its neighbours):

```rust
text_row("                  A all attack   D all defend   u use item"),
```

**Out of scope, do not fix here:** both help screens still describe the pre-roster combat model (`crates/tui/src/ui.rs:2133-2136` — "Up to 3 companions... One command per round even with a full party"; the `c command companion` phrasing in the GUI line above). That text was already stale before this work, and rewriting it is a separate job. Correct only the key names this change invalidates.

- [ ] **Step 4: Add the CHANGELOG entry**

`CHANGELOG.md` currently opens with `## 0.2.0 — 2026-07-24` (`CHANGELOG.md:13`) and has no Unreleased section. Add one **above** it rather than cutting a new version — the workspace shares one version set in the root `Cargo.toml`, so choosing a release number is the user's call, not a side effect of this plan. None of these changes touch `save::SAVE_FORMAT_VERSION`, so whenever it is cut it is a patch bump under the repo's own rule.

```markdown
## Unreleased

### Combat flow

- **The round-resolve page is gone.** A resolved round used to open a
  full-screen narration overlay that had to be dismissed before planning the
  next one. It never had anything to show — its log source was never written
  to — while the battle screen's own log pane was already carrying the real
  narration. Rounds now resolve straight back into planning, separated in the
  log by a dim `── round N ──` line.
- **`[A]ll attack` and `[D] all defend`** plan every open party slot in one
  keypress. All-attack asks which group only when more than one is still up.
  Neither overwrites a slot you already chose for.
- **Battle keys are lowercase, and Decompile moved to `c`.** Defend takes `d`,
  so the per-slot keys `a`/`d` and their party-wide counterparts `A`/`D` line
  up: shift means "everyone does this". Nothing sits one shift key away from a
  different action.

### Balance

- **The base platform shrank from a 15-tile radius to 7.** The platform edge
  is also where the danger curve starts measuring, so hostiles now get tougher
  8 tiles nearer to home — the first stat-escalation step moves from 30 tiles
  out to 22.
```

- [ ] **Step 5: Verify nothing else in the docs went stale**

Run: `rg -n "De\[f\]end|\[A\]ttack|\[D\]ecompile|\[U\]se item|\[J\]ack|15 tiles" README.md CHANGELOG.md`
Expected: no hits outside `docs/superpowers/` (the spec and plan archives are historical records and are left as written).

Note: `CLAUDE.md` is gitignored, so its own key references never ship with this branch either way. Mention this in the handoff rather than editing it silently.

- [ ] **Step 6: Final gate**

Run: `cargo test --workspace`
Run: `cargo clippy --workspace`
Run: `cargo fmt --check`
Expected: all clean.

- [ ] **Step 7: Commit**

```bash
git add README.md CHANGELOG.md crates/tui/src/ui.rs crates/gui/src/render.rs
git commit -m "docs: refresh the intrusion key table and drop the narration-paging claim"
```

---

## Verification

Nothing in this plan was play-tested — it is arithmetic and unit tests. Two things specifically warrant a human at the keyboard before this is called finished:

1. **Combat pacing.** The popup removal is the whole point; whether the log pane reads well without it is a judgment call. On a short terminal the TUI log pane is `Constraint::Min(4)` (`crates/tui/src/ui.rs:1752`), which is two visible lines after borders — a round with a full party and a multi-member pack will produce more narration than that and scroll. If it reads badly, the fix is the layout constraint, not the design.
2. **The smaller base.** A 15×15 platform with the danger curve pulled 8 tiles inward changes early-run feel more than the one-line diff suggests.
