# 0030 — Player sprites override shipped art, per name, from the data folder

- Status: Agreed
- Decided by: user, on Claude's proposal
- Date: 2026-10-26
- Pattern: Layered lookup (user layer over shipped layer)
- Depends on: —

## Context

The Sprite Forge (`crates/app-core/src/app/sprite_forge.rs`) is a dev tool.
`App::sprite_forge_enabled` opens it only with `FERAL_DEV_SPRITES` set *and*
a source checkout, because every write lands in `assets/sprites/`, the
shipped art. An installed build may not be able to write there (a zip under
`Program Files`), and a game update would overwrite whatever it did write.

The user wants every player to edit any sprite to their own liking.
Saves, `profile.ron` and `run_history.log` already live in the per-user data
folder (`launcher::paths::data_dir`), so a player-writable place exists.

## Decision

A player's sprite edits live in `<data_dir>/sprites/`, named exactly as
`assets/sprites/` names them (`<name>.png`, `<name>.colour.png`, `.png.off`,
`<name>.anim.ron`). The shipped tree is never written at runtime.

**Lookup is per name, whole-name.** If the user folder holds any file for a
name, the user folder alone decides that name: both variants, the on/off
state and the frame pace. Otherwise the shipped files decide it. Two layers
are never mixed within one name.

Turning a shipped sprite off copies its art into the user folder as
`.png.off`, so the existing rename invariant (`apply_sprite_write`) still
holds inside the one folder it ever touches. **Revert** deletes every user
file for that name, and the shipped art takes over again.

There is one behaviour for everyone. `FERAL_DEV_SPRITES` and the checkout
gate are deleted, along with `App::install_sprite_dir`. Shipping new art
means copying a file from the data folder into `assets/sprites/` by hand.

## Alternatives considered

- **Write into `assets/sprites/` for everyone.** It fails silently on a
  read-only install, and an update overwrites the player's work.
- **Per-file layering**, where a user `.colour.png` sits beside a shipped
  `.png`. Which file draws would then depend on two folders at once, and the
  rename invariant would span both. Whole-name lookup leaves one folder to
  reason about.
- **Keep a dev path into `assets/` when running from a checkout.** Rejected
  by the user: one behaviour for all.

## Consequences

- The Forge becomes a player screen, reached from a Game Menu rail page.
- The sprite loader reads two roots. Bevy's `AssetServer` resolves only
  under `assets/`, so the user layer needs its own source or a direct decode.
- `assets/sprites/README.md` gains the user-folder rule.
- No save-format change. Sprites are not in a save.

## Evidence

`App::sprite_forge_enabled`, `dev_sprite_forge_flag`,
`crates/gui/src/sprites.rs` (`load`, `register`, `scan_library`,
`apply_sprite_write`, `drain_writes`), `crates/launcher/src/paths.rs`
(`data_dir`, `DevPaths::sprites`), `crates/app-core/src/app/game_menu.rs`.
