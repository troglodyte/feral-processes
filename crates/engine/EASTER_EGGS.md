# Hidden keys

Four keys in this game do something real and are named by nothing the
player can read. This file is the only complete list of them.

It sits in the crate root rather than under `src/` so it is not mistaken for
module documentation, and nothing player-facing links to it.

| Key | Screen | What it does |
| --- | --- | --- |
| `W` | companion screen | Wields the highlighted program as your weapon (`Game::wield_program`). |
| `Z` | the Stack (map screen) | Listens: reads the description bank's paragraph for the cell on rotten ground, otherwise gives the bearing and distance of the nearest unspent feature. Costs a turn and raises Trace (`Game::listen`). |
| `T` | battle roster | Your front companion says a line at the wild group. No turn, no round (`Game::taunt`). |
| `T` | battle item picker | Throws the highlighted consumable at the wild group instead of using it. One point of damage, and it cannot kill (`Game::throw_item`). |

## The rule

**No help text, prompt, label or menu row may name any of these keys.**

That is not a style preference. Each of them is only a discovery because
nothing announces it; documenting one on screen does not improve the feature,
it deletes it.

The rule is easy to break by accident, because breaking it looks like doing
your job: someone adding a line to the help screen, or a battle action that
happens to want the letter `T`, has no way to know they are undoing
something. That is the whole reason this file exists.

## What holds it

Two assertions, because there are only two places a key can reach the
player:

- `crates/gui/src/render/meta.rs` —
  `the_help_screen_never_names_a_hidden_key`. `draw_help` is the *only*
  screen in the game that lists key bindings; it covers the map, the Stack,
  trading and battle in one list. The test asserts no row contains `W`, `T`
  or `Z` as a standalone whitespace-delimited token, which is the binding
  idiom that screen uses (`s save`, `L history`, `A all attack`). Tokens,
  never substrings — the rows are full of those letters inside ordinary
  words, and of the lowercase `t` that legitimately binds trade.
- `crates/engine/src/tests/easter_eggs.rs` —
  `no_battle_action_or_party_command_claims_a_hidden_key`. Everything else a
  player reads about battle keys comes from `Game::battle_action_options`
  and `Game::battle_party_commands`, which are engine data both renderers
  build their prompt from. Nothing claims these letters today; the test is
  what fails if a future battle action does.

There is one more: `crates/gui/src/render/party.rs` holds the companion
screen's help lines to never naming `W` or the verb for it. It predates this
file and covers only that screen.

## The shape they share

If you are adding a fifth, follow the same rules — they are not arbitrary:

- **Uppercase, intercepted before the row shortcuts.** `App::selected_index`
  rejects every non-lowercase char precisely so screen actions can own the
  uppercase space, so an uppercase key can never collide with the
  digits-then-lowercase row scheme however long a list grows.
- **On a screen that already exists.** A hidden screen needs hidden
  rendering, which is where an easter egg turns into a feature.
- **No new saved state.** None of these touch
  `save::SAVE_FORMAT_VERSION`.
- **Never a `GameRng` draw.** Both the crash-log line and the taunt line are
  chosen deterministically. A cosmetic draw shifts the shared stream and
  silently rewrites every seeded test in the suite.
- **Mind the shift-slips.** On the map screen `K J H L S Q` are all a
  fumbled press away from a key the player uses constantly, and binding a
  turn-costing action to one of them would spend a turn and raise Trace for
  no reason the player could diagnose. `Z` was chosen for having no
  neighbour to slip from.

The design argument for the three added on 2026-08-06 is in
`docs/superpowers/archive/specs/2026-08-06-easter-eggs-design.md`.
