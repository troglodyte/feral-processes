# Player sprite editor

**Status:** approved 2026-10-26. Decision record:
[ADR 0030](../../adr/0030-player-sprites-override-shipped-art-per-name.md)
(Agreed).

## Intent

Any player can change how the game looks by redrawing its sprites. Today
the Sprite Forge does exactly this, but it is a dev tool: it is behind
`FERAL_DEV_SPRITES` plus a source checkout, and it writes into the shipped
`assets/sprites/`. This change makes it a player feature that is always
available and saves to the player's own data folder.

## What the player sees

1. **Entry.** The Game Menu hub's rail gains a **Sprites** page after
   Graphics and before Save and Quit. The Game Menu is the hub opened both
   in a run and from the main menu's `[O] Options`, so this one page
   covers both entry points the user asked for. The main menu's dev `[D]`
   row is deleted.
2. **Picker.** The page body is today's `Mode::SpritePicker` list: every
   name the map can draw a sprite for, with its glyph or art preview. Rows
   follow the highlight + Enter rule. A row the player has edited carries a
   marker (wording is the builder's call), so they can tell their art from
   the shipped art.
3. **Editor.** Enter on a row opens `Mode::SpriteEditor` with the
   **current** art already loaded: the player's edit if one exists,
   otherwise the shipped art, otherwise a blank canvas. Both frames, the
   pace and the colour variant all carry over. This is what
   `open_sprite_subject` already does; it now reads the layered library.
4. **Save.** `[s]` saves to the player's folder. The map shows the new art
   at once, as it does today.
5. **Everything the Forge does** stays available to players (user
   decision): turning a sprite on or off, and the plain/colour variant.
   All of it writes only to the player's folder.
6. **Revert** (user decision). A key on the picker row deletes the
   player's files for that name, and the shipped art shows again. A name
   with no player files has nothing to revert, so the key refuses it with a
   status-line message, as other screens do. Revert asks for y/n confirmation first,
   because it deletes the player's work.
7. **Esc** from the editor returns to the picker, and Esc on the picker
   returns to the rail, the same as other hub pages.

## Storage (ADR 0030)

- The player's folder is `<data_dir>/sprites/`, with the same file naming
  as `assets/sprites/`. The folder is created on the first write.
- Lookup is whole-name: any user file for a name means the user folder
  alone decides that name.
- Turning a shipped sprite off writes its art into the user folder as
  `.png.off`. Turning it back on renames the file within the user folder.
- `assets/sprites/` is never written at runtime.
- **A user `.png.off` with no user `.png` hides the shipped art.** The
  glyph draws, by the whole-name rule. Turning off a name that has only
  shipped files is therefore a **copy** of the shipped file into the user
  folder as `.off`, not a rename, so `SpriteOp::Disable`'s handling gains
  that case. *(Claude's call.)*
- Saving the plain variant over a name whose shipped art is the colour
  variant hides the shipped colour art. That is acceptable: the editor
  opens on the current variant, so this only happens when the player
  chose it. The pace of a name in the user folder comes from the user
  `.anim.ron` (written by every save) or the default, never from the
  shipped one. *(Claude's call.)*
- **A failed write is shown to the player**, as a status-line message
  ("couldn't save sprite …"), not just logged. Today
  `WriteOutcome::Failed` only warns. *(Claude's call: the anti-goal
  forbids a silent failure.)*

## Goals

- Every player gets the same screen, with the same behaviour in a checkout
  and in an installed build (user decision: one behaviour for all).
- A game update never loses a player's edits, and the player's edits never
  damage the install.

## Non-goals

- Sharing or importing sprite packs, or a mod format.
- Adding new sprite names. The picker lists only names the map can
  already draw.
- Changing the editor's drawing tools.
- A dev shortcut that writes into `assets/`.

## Anti-goals

- A player edit that silently fails to show, or reappears after a revert.
- A shipped file modified or deleted at runtime.
- The same name drawn from both folders at once.

## Constraints

- The drawing seam: only `paint.rs` names a graphics library, and
  `render/` draws through `Painter`.
- app-core does no file I/O. Writes go through `SpriteWrite` /
  `take_sprite_writes`, and the gui drains them, as today.
- The launcher's `paths.rs` is the one place runtime paths are decided.
  The user sprite folder is derived there and handed in, like the assets
  folder is.
- Bevy's `AssetServer` loads only under the configured assets root, so the
  user layer needs another route. A second named asset source and decoding
  the PNG directly are both acceptable; the builder picks.
- The player's drawn creation icon (`profile.ron`, PlayerIcon codec) is a
  separate thing and is untouched.
- Writes stay keyboard-driven. The existing mouse drawing stays as it is.
- Two or more crates change, so this goes through the spec-and-plan
  pipeline.

## Removed

`FERAL_DEV_SPRITES`, `dev_sprite_forge_flag`, `App::sprite_forge_flag`,
`App::install_sprite_dir` / `sprite_dir_installed`, `DevPaths::sprites`,
`launcher/main.rs`'s `paths.dev.is_some()` gate around the install, the
main-menu `[D]` row and `main_menu_options`' `sprite_forge` parameter.
The dev console's "Open Sprite Forge" row (`dev_console.rs`) goes too,
because the rail page replaces it *(Claude's call)*. Any test or doc that
names these changes as well, including the CLAUDE.md dev command list if
it mentions the flag.

## Wiring the reviewer flagged

- `gui::sprites::install_library` is gated on `sprite_forge_enabled`
  today, so the decoded library exists only in dev. It becomes
  unconditional at startup. The cost is one decode of every sprite, about
  as many as the map already loads.
- `load`, `register`, `read_frame_ms`, `scan_library` and `drain_writes`
  all read `assets_dir()/sprites` today. Each one reads the merged
  two-layer view instead, and that merge is one pure function both sides
  call (user names shadow shipped ones), not a copy in each.
- The launcher derives `<data_dir>/sprites` in `paths.rs` and hands it to
  `App` as a path, the same way `assets_dir` is handed in. The gui reads
  it from `App`. *(Claude's call.)*
- Esc from the editor goes back to the picker on the same row. Esc from
  the picker goes back to the rail on the Sprites page.

## Left to the builder

- The rail page's label, the edited-row marker and the revert key letter.
  The key must not clash with the picker's existing keys.
- How the user layer is loaded into bevy.
- Whether the Forge's picker and editor modes become hub pages or are
  entered from one. Esc behaviour as described above is what matters.
- Updating `assets/sprites/README.md` and the `CHANGELOG.md` entry.
