# Outpost Names — Design

**Status:** Proposed, 2026-11-06. Lands on the `sites-hub` branch with the
Sites hub (`2026-11-06-sites-hub-design.md`).

## Why

There is one outpost def, so every outpost is called "Outpost (x, y)".
On the Sites rail that name is elided to "Outpost…, -250)", and the
coordinates are a tick-like figure the player never otherwise reads. The
player should be able to tell outposts apart at a glance and call them
what they like.

## Goals

- A new outpost gets a readable default name from its compass direction
  from the base: "Outpost north-east". A second outpost on the same
  heading gets "Outpost north-east 2", then 3, and so on.
- The player can rename an outpost at any time with `N`, from the
  outpost's Sites **Status** tab or its visit page.
- The name shows everywhere "Outpost (x, y)" shows today.

## Non-goals

- Renaming towns or programs. Program renaming (`Mode::RenamePet`) stays
  as it is.
- Unique names. Two outposts may share a name, as two towns can. The rail
  stays ordered and unique by tile.

## Anti-goals

- Coordinates in any outpost name a player sees.
- A naming field that makes you delete the old name letter by letter.

## Decided (by the user)

- **Rename any time**, not only at founding.
- **The default is the direction from base**, numbered on a collision.
- **The field is the shared `TextField`** (`crates/app-core/src/text_field.rs`),
  the one the routine and weapon name fields use. It opens holding the
  current name with the cursor at the end, and ←/→, Backspace and typing
  edit it in place.
- **The work lands on this branch**, before the Sites hub ships.

## Behaviour

- **Default name.** It is fixed at `found_outpost` and stored. Founding
  another outpost never renames an old one. The heading comes from
  `stack::heading(tile - anchor).label()`. With no anchor, or `Here`, the
  name is plain "Outpost". The name is the first of `D`, `D 2`, `D 3`, …
  that no other outpost's stored name equals exactly. Matching on prefix
  would be wrong: "Outpost north" is a prefix of "Outpost north-east".
- **Rename.** Enter commits the trimmed text. An empty field restores the
  default direction name, recomputed the same way. Esc leaves the name
  unchanged. The cap is `OUTPOST_NAME_MAX = 24`, the routine and gear
  fields' figure.
- **Where it shows.** `outpost_destination_name` returns the stored name.
  That covers the Sites rail, route lines, the cargo basket title, the
  world-map label, inspection, and the logs and alerts built from it.
- **Saves.** `OutpostSave` gains `#[serde(default)] name: String`. On
  load, an outpost with an empty name gets its default, assigned in tile
  order so the numbering is stable. No `SAVE_FORMAT_VERSION` bump.
- **Site log.** A rename writes no log line. It is the player's own
  action, not news.

## Constraints

- Plain language. No new RNG draws. The engine door is
  `Game::rename_outpost(tile, &str) -> Result<(), String>`, which refuses
  in its own words for an unknown tile.
- The name field mode lives in app-core's outposts module, not `lib.rs`.
  Esc and Enter return to the screen it was opened from: the Sites
  Status tab through the hub's held page, or the visit page.
- A 24-character name plus the glyph and `!` fits the rail at 1280x720,
  pinned by a width census.

## Left to the builder

- Whether the name-field mode is one `Mode::OutpostName` that remembers
  where to return, or two modes. Prefer one.
- The gui draw. Reuse whatever the routine and gear name rows draw, and
  don't write a third copy.
