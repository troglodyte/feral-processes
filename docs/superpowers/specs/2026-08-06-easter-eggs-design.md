# Three more hidden keys

**Date:** 2026-08-06
**Status:** designed

Three easter eggs in the shape of the wielded program (`W`): an uppercase key
on a screen that already exists, doing something real, with nothing on screen
naming it.

- **`Z` in the Stack** — listen. On rotten ground it reads the local crash
  log; anywhere else it points at the nearest thing in the frame you haven't
  spent yet. Costs a turn and raises Trace.
- **`T` on the battle roster** — your front companion taunts the wild group.
  Pure flavour, species-authored, no turn.
- **`T` in the battle item picker** — throw the highlighted consumable at the
  wild group instead of using it. One point of damage, and it can't kill.

Nothing in the game's text advertises any of them.

## The rules they all follow

`W`'s doc (`app/party.rs`) already states most of this, and these three
inherit it rather than restating it:

- **Uppercase, intercepted before the row shortcuts.** `selected_index`
  rejects any non-lowercase char precisely so screen actions can own the
  uppercase space (`input.rs:24-30`), so an uppercase key can never collide
  with the digits-then-lowercase row scheme however long a list grows.
- **No new `Mode`.** Every one of these lives on a screen the player is
  already looking at. A hidden screen would need hidden rendering, which is
  where an easter egg turns into a feature.
- **No new saved state.** `SAVE_FORMAT_VERSION` is untouched.

### The key space is smaller than it looks

On the map screen — which is also the Stack screen, since `handle_stack_key`
runs after the same mode block — movement is `k j h l`, saving is `s`, and
quitting is `q`. So `K J H L S Q` are all **shift-slips of a key the player
presses constantly**, and binding a turn-costing action to one of them means
a fumbled step silently spends a turn and raises Trace. `L` is taken by
history anyway; the other five are ruled out by the slip alone.

`Z` is chosen for having no neighbour to slip from. It carries no mnemonic,
which costs nothing for a key no text will ever name.

In battle the space is checked rather than assumed: party commands are
`A`/`D`/`j` (`combat.rs:900`) and per-slot actions are `a`/`d`/`s`/`u`
(`combat.rs:841`). `T` matches neither, and neither does the `t` that
`handle_battle_key`'s case-folding retry would look for. It is free at both
stages.

## 1. `Z` — listening in the Stack

### What it reports

Two readings, chosen by the cell the party is standing on:

- **`Fault` or `Corruption`** → one line of that place's crash log.
- **anything else** → the direction, *relative to the party's facing*, of the
  nearest unspent feature in the frame: an unopened cache, an unburnt seal, a
  present orphan, an uncleared lair.

Relative rather than compass on purpose. The frame map already gives absolute
positions for everything walked; a bearing off your own facing is the one
thing it cannot tell you, and it is what a first-person sense should return.

The four features are exactly the ones with a `FrameMemory` record —
`cache_unopened`, `seal_open`, `orphan_present`, `lair_cleared`
(`game/stack_features.rs`). `Fault` and `Corruption` are excluded because
they deliberately have no record: neither is used up, so "unspent" is not a
question that can be asked about them, and listing every fault in the frame
would drown the reading in terrain.

### What it costs

A turn, and `tuning::TRACE_PER_LISTEN`, set at **3** — under
`TRACE_PER_SEAL`'s 5, since listening takes nothing out of the frame, and
well under `TRACE_PER_CACHE`'s 10.

**Charged whether or not anything is heard.** Listening is loud in itself,
and a swept-clean frame reporting silence is the information the turn bought.
The alternative — free when there is nothing to find — makes `Z` a
zero-risk sweep the player would mash on every tile.

### Where the refusal lives

`Game::listen` reads `Locale::Stack`'s own coordinates and facing, so its
refusal is `Game::stack_pos()` returning `None` — the `Phase`/`Jump` case,
not the `require_surface` case. `require_surface` guards actions that reach
*zone-map* state through a `Position` pinned to the entrance tile; this
action never touches `Position` at all.

Being a reader that *claims something about where the party is*, it would
otherwise be squarely in `find_target_in_direction`'s trap. It escapes by
construction: everything it names is a cell of the current frame, so there is
no surface state for it to misreport.

### The crash log is data

A new content directory, `assets/crash_logs/`, following the `load_dir`
pattern the other five databases use: one `.ron` file per entry, malformed
files skipped with a logged warning rather than a panic, and an
`assets/crash_logs/README.md` documenting the schema in the same shape as
`assets/species/README.md`.

**Which line a given patch of rot reads is derived from `(zone, depth, cell
x, y)`, never from `GameRng`.** Two reasons, both already learned here: a
`GameRng` draw does not survive a save/load, so the same corrupted tile would
say something different after a reload; and drawing from the shared stream to
pick a *cosmetic string* shifts every later roll in the run. This is
`Game::orphan_species`' rule — what a place is, is a property of the place.

## 2. `T` — taunting

Your front living companion says a line at the wild group. No turn, no round,
no effect on the fight. If the party is empty, the player says it instead, so
the key never silently does nothing.

Intercepted in `handle_battle_key` immediately after the `GameKey::Char(raw)`
destructure and before the party-command lookup, matching on `raw` rather
than the folded `c` — the same position and the same argument as `W`'s
intercept ahead of `selected_index`.

### The lines are species data

```rust
/// Lines this species says when taunted into speech. Cosmetic; a species
/// with none falls back to a generic line.
#[serde(default)]
pub taunts: Vec<String>,
```

`#[serde(default)]` per the schema rule, so every existing `.ron` file and
every third-party mod keeps parsing untouched, and
`assets/species/README.md` gains the field in the same change.

Unlike `priority_boost`, this has no must-exist requirement: the engine's
generic fallback covers a species that authors none, which is every shipped
species until someone writes lines for it. Shipping lines for a handful of
the starter species is enough to make the key feel authored.

### Which line, and why not `GameRng`

Chosen by a counter that advances per press within a battle, so repeated
presses cycle through the species' lines.

Deliberately not a `GameRng` draw. A cosmetic key that advances the shared
stream shifts every later roll in the run — which is how a seeded combat test
was silently rewritten from three files away once already. A key a player
might press twenty times in a fight is the worst possible place to put that.
A counter is also directly testable.

## 3. `T` — throwing

In `Mode::BattleItem`, throws the **highlighted** row of
`battle_usable_items()` at the wild group instead of using it: one unit
consumed, `tuning::THROWN_ITEM_DAMAGE` (1) applied through
`Game::apply_damage`, and a log line naming what bounced off.

The picker already has a highlight and arrow navigation through
`selected_index`, so this needs no rendering change at all — the same
property that let `W` ride the companion screen.

Sharing the letter with the taunt is safe and intended: the two live in
different handlers (`Mode::Battle` vs `Mode::BattleItem`) and can never both
fire, and one letter meaning *do the reckless thing* on both battle screens
is easier to remember than two.

### It resolves immediately and cannot kill

**No round cost, no `ActionKind`.** An action kind would have to appear in
`battle_action_options`, which is the list both renderers draw the prompt
from — the secret would be printed on screen. Resolving immediately is what
keeps it off that list.

That leaves it as free damage, which is fine: one point per consumable is a
ruinous rate, the consumables are finite and cost Credits, and there is no
loop to exploit.

**A throw cannot take a target below 1 HP.** Not squeamishness — a kill
resolving here would end a battle from *outside* the round loop, next to
`BattleState::planned`'s positional indexing into `Party` and the deferred
removal `end_battle` exists to do. Clamping at 1 makes that state unreachable
rather than merely unlikely, in the same way a lethal Wild Jump never writes
`Locale`. It is also the better joke.

Damage goes through `Game::apply_damage` rather than writing `Stats::hp`,
because that function is the one path that lowers a creature's HP and
anything watching damage must see this too.

## Keeping them hidden

`crates/engine/EASTER_EGGS.md` — the crate root, not `src/`, so it is not
mistaken for module documentation and no player-facing page links to it. It
lists the four keys (`W` included), what each does, and the rule that no help
text may name them. This exists because an omission is invisible: the next
person to write a help string has no way to know they are breaking something.

Held the way `companion_help` is held, but there is less to hold than it
looks. `render/meta.rs::draw_help` is the *only* screen in the game that
lists key bindings — it covers the map, the Stack, trading and battle in one
`Vec` of rows — and `companion_help` is the one other place a key is named.
Everything else a player reads about battle keys comes from
`battle_action_options` and `battle_party_commands`, which are engine data.

So two assertions, not four:

- A gui test over `draw_help`'s rows: none contains `W`, `T` or `Z` as a
  standalone whitespace-delimited token. That is the binding idiom this
  screen uses (`s save`, `L history`, `A all attack`), so it catches a real
  documentation of the key while staying satisfiable — the rows are full of
  ordinary prose containing those letters inside words, and of the lowercase
  `t` that legitimately binds trade.
- An engine test that no `ActionOption` or `PartyCommand` key is `W`, `T` or
  `Z`. Nothing today claims them, and this is what would fail if a future
  battle action did — which would print the letter in the prompt both
  renderers build from that list.

## Testing

Engine, against the real assets:

- Listening on a frame with one unopened cache names the direction it is in,
  and the direction is relative to facing — the same cache read from two
  facings gives two different answers.
- Listening after emptying that cache reports silence, and still charges
  Trace both times.
- Listening on a `Corruption` cell reads a crash log line, and the *same*
  cell reads the same line after a save/load round trip.
- Listening on the surface is refused and costs nothing.
- A malformed crash-log `.ron` is skipped, not fatal.
- Throwing at a target on 1 HP leaves it on 1 HP and the battle running.
- Throwing consumes exactly one unit of the highlighted item.
- A species with no `taunts` still produces a line.
- Taunting twice cycles, and taunting does not advance `GameRng` — asserted
  by a seeded fight producing an identical outcome with and without a taunt
  in the middle.

app-core, for the key routing:

- `Z` on the surface map does nothing and costs no turn.
- `T` on the battle roster does not commit an action for the active slot.
- `T` in the item picker does not fall through to the row shortcuts.

gui, for the omission: the `draw_help` token assertion above.

## Not in scope

- No mechanical taunt. Nudging `capture_chance` would put a hidden key on
  taming, an axis `balance_sim` models nothing of.
- No throwing of non-usable cargo. Reaching arbitrary cargo from a battle
  needs either a second picker or a mode flag on the existing one, which is
  more surface than the joke is worth.
- No achievement or profile hook for finding any of these. `RunFeats` is a
  per-tick drain queue and `Profile` is cross-run state; a "found the hidden
  key" rung needs real saved run state and a save-format bump.
