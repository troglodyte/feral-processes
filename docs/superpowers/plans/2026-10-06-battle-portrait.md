# Battle Portrait Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Bard's Tale–style picture window on the group battle screen showing the acting combatant's sprite during the reveal, and the first-in-line hostile otherwise.

**Architecture:** The engine decides who is shown and snapshots it into each `RosterFrame`, so replay is exact and headless-testable; `BattleView.portrait` is the one field the renderer reads. The gui splits the log panel's rect and draws the portrait through `Painter::sprite` with a glyph fallback.

**Tech Stack:** Rust, bevy_ecs (engine), bevy_egui via `paint.rs` (gui).

**Spec:** `docs/superpowers/specs/2026-10-06-battle-portrait-design.md`

Per repo CLAUDE.md, this plan carries files, interfaces, test intent and gates — not finished code. Line numbers are as of `origin/main` 098bb693; verify before editing.

## Spec refinements (decided while planning)

- `PortraitView.is_player` becomes `drawn_icon: bool` — the gui only needs to know whether to try `@drawn`; it is `true` exactly when base.rs:1105 would draw the drawn icon (`is_player && form.is_none() && look.icon.is_some()`).
- The "currently acting" entity lives on `BattleTimeline` (transient, unsaved), not `BattleState`, so no save-format question arises. Each frame stores the *resolved* portrait at snapshot time, so a dead/despawned actor still replays correctly.
- The front-enemy fallback uses `front_of_group(0)` (skips cloaked members) so the portrait never reveals a cloaked program.

## Global Constraints

- Only `crates/gui/src/paint.rs` names the graphics library; `render/` draws only through `Painter`.
- Sprite fallback is gated on `Painter::sprite`'s `bool` return, never on the name.
- `Game.world` gets no accessor; the renderer sees only views.
- No save-format change (`SAVE_FORMAT_VERSION` untouched); no asset schema change.
- No player-facing tick vocabulary; comments explain why, not what.
- `cargo fmt`, `cargo clippy --workspace --all-targets` clean after each task.
- Unset `FERAL_DEV_NO_SIEGES` before `cargo test`.

## Review Focus

1. A swing by an actor that dies on the same line (or is despawned at fight end) — replay must still show it, not panic or go blank. → Task 2 test.
2. A cloaked front member — portrait must not reveal it. → Task 2 test.
3. A tiny window / few log rows — pane must vanish rather than overlap the log or produce a zero/negative-size rect. → Task 3 test.
4. A player with a form (tamed/emulated look) — `drawn_icon` must be `false` so the form's sprite shows, matching the map. → Task 1 test.
5. Round header and non-swing lines (stun, buffs) between swings — portrait holds the current actor within its turn, falls back to the front enemy outside any turn. → Task 2 test.

---

### Task 1: `PortraitView` and the shared look resolver (engine)

**Files:**
- Modify: `crates/engine/src/views.rs` (new `PortraitView` near `FormLook` ~1036)
- Modify: `crates/engine/src/game/inspection.rs` (`build_views` ~1168 — extract the species/structure sprite lookup)
- Possibly modify: `crates/engine/src/tactical/view.rs:550-574` (same lookup — call the extracted fn)
- Test: `crates/engine/src/tests/battle_timeline.rs` (or a new `tests/portrait.rs`, registered in `tests/mod.rs`)

**Interfaces:**
- Produces:
  ```rust
  #[derive(Clone, Debug, PartialEq)]
  pub struct PortraitView {
      pub sprite: Option<String>,
      pub glyph: char,
      pub color: GlyphColor,
      pub name: String,
      /// Try the player's drawn icon first (`@drawn`), as the map does.
      pub drawn_icon: bool,
  }
  impl Game { pub(crate) fn portrait_of(&self, entity: Entity) -> Option<PortraitView>; }
  ```
  Plus one extracted `pub(crate)` sprite-name resolver (entity → `Option<String>`, species then structure) that `build_views`, `portrait_of` and (if it is the same logic) `tactical/view.rs` all call. Sprite priority for the portrait matches base.rs:1123: form sprite → player sprite (empty = none) → species/structure sprite. Name = the same display name the party/enemy rows use.

- [ ] Write failing tests: `portrait_of` a hostile gives its species sprite name, glyph, colour, `drawn_icon == false`; `portrait_of` the player with a drawn icon and no form gives `drawn_icon == true`; with a form → `drawn_icon == false` and the form's sprite.
- [ ] Run `cargo test -p feral-processes-engine portrait` → fails to compile / fails.
- [ ] Implement `PortraitView`, extract the resolver, implement `portrait_of`.
- [ ] Run the tests → pass; `cargo test -p feral-processes-engine` green; fmt + clippy.
- [ ] Commit "Battle portrait: PortraitView and shared sprite resolver".

### Task 2: who is shown — timeline, frames, views (engine)

**Files:**
- Modify: `crates/engine/src/resources.rs` (`BattleTimeline` ~1619, `RosterFrame` ~1588, `ClosingRoster`)
- Modify: `crates/engine/src/game/combat_round.rs` (`battle_resolve_round` loop ~140, `snapshot_roster` ~1042, `assemble_view` ~1022, `battle_view`/`battle_view_at` ~995/1007, `battle_result_view` ~1068; closing capture at the top of `end_battle`)
- Modify: `crates/engine/src/views.rs` (`BattleView` ~1986)
- Fix: every other `BattleView { … }` / `RosterFrame { … }` literal (`rg -n "BattleView \{|RosterFrame \{" crates/`), including app-core/gui test fixtures.
- Test: `crates/engine/src/tests/battle_timeline.rs`

**Interfaces:**
- Consumes: `Game::portrait_of`, `PortraitView` (Task 1); `front_of_group(0)`; `actor_entity(actor)`.
- Produces:
  - `BattleTimeline.acting: Option<Entity>` — set to the actor's entity at the top of each initiative iteration in `battle_resolve_round`, cleared after the loop (and wherever the round exits early).
  - `RosterFrame.portrait: Option<PortraitView>` and `ClosingRoster.portrait: Option<PortraitView>`.
  - `BattleView.portrait: Option<PortraitView>`.
  - One pure selector both snapshot and live paths call: `acting.and_then(portrait_of).or_else(|| front_of_group(0).and_then(portrait_of))`.
  - `battle_view` → selector on live state; `battle_view_at` → the frame's `portrait` (live selector when no frame); `battle_result_view` → `closing.portrait` (front-or-None, so `None` on a win).

- [ ] Write failing tests in `battle_timeline.rs` (model: `the_opening_line_of_a_round_shows_the_roster_untouched`, setup `a_fight_that_survives_a_round`, `insert_battle`, `player_attacks`):
  - before any round, `battle_view().portrait` is the hostile front member;
  - after `player_attacks`, the revealed line of the player's swing shows the player, and the hostile's retaliation line shows the hostile; line 0 (round header) shows the front hostile;
  - non-swing lines inside an actor's turn hold that actor (Review Focus 5);
  - a hostile killed by the swing that is narrated still appears on its own line's frame (Review Focus 1);
  - a cloaked front member is not shown (Review Focus 2) — use the existing cloak status setup;
  - after a win, `battle_result_view().portrait` is `None`; after a jack-out it is the front hostile.
- [ ] Run → fail.
- [ ] Implement fields, set/clear `acting`, snapshot portrait in `snapshot_roster`, wire the three views, fix literals.
- [ ] Run engine tests → green; `cargo test -p feral-processes-engine balance_sim` unchanged; fmt + clippy.
- [ ] Commit "Battle portrait: the engine picks who is shown each line".

### Task 3: the portrait pane (gui)

**Files:**
- Modify: `crates/gui/src/render/battle.rs` (`draw_battle` log panel ~412-458; new `fn draw_battle_portrait(p: &Painter, portrait: &PortraitView, rect: Rect, m: &Metrics) -> …` and a pure `fn portrait_split(log: Rect, line_height: f32) -> Option<(Rect, Rect)>` returning (portrait pane, remaining log rect))
- Minimum log width is a named `const` in battle.rs beside the existing column-width constants (~119-139), not `tuning.rs` (which is sim tuning).
- Test: `mod tests` in `crates/gui/src/render/battle.rs` (~750)

**Interfaces:**
- Consumes: `BattleView.portrait: Option<PortraitView>`; `crate::sprites::DRAWN_ICON_KEY`; `Painter::sprite(name, x, y, size, color) -> bool`; `hud::palette::glyph` for the glyph colour.
- Behaviour: side = (log height − one caption row) floored to a multiple of 16 px; `None` from `portrait_split` when side < 16 or the remainder < the min log width. Draw: if `drawn_icon`, `sprite(DRAWN_ICON_KEY, …, neutral)`; if that is false (or not `drawn_icon`) `sprite(name, …, neutral)`; if that is false, `text` the glyph at pane size in its palette colour. Caption = `name` centred in the caption row. The log (rows, scroll hint, buffs) draws into the remaining rect; buffs stay anchored top-right.

- [ ] Write failing tests:
  - `portrait_split` pure tests: side is a multiple of 16; returns `None` on a narrow and on a short log (Review Focus 3); remainder starts right of the pane.
  - With `with_sprites` (model: tactical.rs:2207) and a full-app battle (model: `draw_battle_paints_a_rare_partys_tier_tag` ~1327): the front hostile's sprite texture is painted inside the pane and its glyph is not painted there; with an empty `SpriteTable` the glyph is painted; the caption text appears.
- [ ] Run `cargo test -p feral-processes-gui battle` (check crate name in `crates/gui/Cargo.toml`) → fail.
- [ ] Implement.
- [ ] Tests green; fmt + clippy.
- [ ] Visual check: `cargo run -- --template stack --keys "…" --screenshot /tmp/…png` into a fight (find the keys that start a fight from the `stack` template; `dev-saves/README` lists templates). Read the PNG; check pane placement, crisp pixels, log reflow.
- [ ] Commit "Battle portrait: draw the portrait pane left of the battle log".

### Task 4: seam rule and changelog

**Files:**
- Modify: `.claude/rules/drawing-seam.md` (one-line scoped exception)
- Modify: the `seams` skill (`.claude/skills/seams/…` — find via `rg -l "drawing" .claude/skills`) — the trap: portrait is a picture pane, its glyph is only the fallback gated on `sprite()`'s return
- Graph: `memory_add_observations` on `seam:drawing-seam` with the argument (follow the order the `seams` skill documents)
- Modify: `CHANGELOG.md` — unreleased entry describing the portrait in plain language (version bump happens at landing, not on the branch)

- [ ] Make the three seam writes in the skill's documented order.
- [ ] Changelog entry.
- [ ] Gate: `cargo test --workspace` green (FERAL_DEV_NO_SIEGES unset); clippy clean.
- [ ] Commit "Battle portrait: drawing-seam exception and changelog".

### Final

- [ ] Whole-branch review (opus), diff given as a file.
- [ ] Land per `landing-work-is-merge-tag-push-cleanup` (patch bump) — only on the user's go-ahead.
