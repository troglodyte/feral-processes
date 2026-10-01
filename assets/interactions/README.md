# Interactions (mods)

Edit or add a `.ron` file in this directory and it's picked up automatically
the next time a game session starts. A malformed file is skipped with a
warning logged in-game rather than crashing startup.

**This directory may be deleted.** An empty catalogue is valid and inert: no
program ever talks to another, and the game is exactly the one before
interactions existed.

## What an interaction is

Every so often (`tuning::INTERACTION_PERIOD`) the base pairs up its idle staff
— base staff with no task, standing within a couple of tiles of each other. Each
pair may speak: the chance is `tuning::INTERACTION_CHANCE` scaled by how
sociable the speaker is (Reserved, Sociable or Chatty, derived from its id and
shown on the SOCIAL tab). If it does, one interaction is picked from this
directory by weight, and it leaves a **memory** (`assets/memories/`) on the
listener, and optionally one on the speaker. A program is in at most one pair
per pass.

```ron
(
    id: "compliment",
    name: "A compliment",
    listener_memory: "complimented_by",
    weight: 1.0,
    by_band: {Enemy: 0.05, Rival: 0.2, Neutral: 1.0, Friend: 1.5, Close: 2.0},
    by_disposition: {Amiable: 1.8, Abrasive: 0.3},
)
```

| Field | Meaning |
|---|---|
| `id` | Unique across the directory; the first file by name wins a clash. |
| `name` | For this table and later screens; not drawn yet. |
| `listener_memory` | A memory id written on the **listener**, about the **speaker**. It must be a `Program`-subject memory that ships. A def naming one that does not resolve is skipped with a warning. Ignored when `gossip` is true, but still required. |
| `speaker_memory` | Optional. A memory id written on the **speaker**, about the **listener**. Leave it off for a remark that only lands on the one who hears it. |
| `weight` | The base pick weight. Finite and `>= 0`. |
| `by_band` | Optional. A multiplier per band of the **speaker's** opinion of the listener: `Enemy`, `Rival`, `Neutral`, `Friend`, `Close`. A band left out is `1.0`. |
| `by_disposition` | Optional. A multiplier per **speaker** disposition: `Steady`, `Amiable`, `Abrasive`, `Languid`, `Dogged`. One left out is `1.0`. |
| `gossip` | Optional, default `false`. See below. |
| `exchanges` | Optional, default none. What the pair says: a list of exchanges, each a list of `Line`s. See below. |

The pick weight of a def is `weight * by_disposition * by_band`. A negative or
non-finite number in any of the three is a load-time fault. Every weight at
zero means nothing is said.

The memories an interaction writes are an engine concern in one respect only:
a def's memory must carry a `Program` subject, and it is written about the
other program in the pair.

## Exchanges

When an interaction fires, the engine picks one of the def's `exchanges` and
both programs keep a record of it, shown on the TALK tab. A record stores which
exchange was said, not the words, so editing a line rewords the history.

```ron
exchanges: [
    [
        (by: Speaker, text: "Status of {topic}?"),
        (by: Listener, text: "Unchanged."),
    ],
],
```

- A `Line` is `(by: Speaker, text: "...")` or `(by: Listener, text: "...")`.
- An exchange has **2 to 4 lines** (`tuning::CONVERSATION_MAX_LINES`). One
  outside that range is dropped with a warning naming the file and the
  exchange's index; the def and its other exchanges still load.
- Text may use three slots, written in braces: `{speaker}`, `{listener}` and
  `{topic}`. Any other `{name}` drops the exchange with a warning. Names are
  filled in when the page is drawn, so a rename reads through.
- `{topic}` is something the speaker holds a memory about (never the speaker
  or the listener). When the speaker has nothing on its mind, only exchanges
  **without** `{topic}` are eligible, so give every non-gossip def at least one.
  A gossip def should use `{topic}` in every exchange: it is always about the
  third program it told.
- A def with no exchanges is valid. Its records show one line, `<speaker> and
  <listener>: <name>`. The same line is shown when a record names an exchange
  or def that has since been removed or reordered.
- Keep lines short: they are drawn in a narrow panel and wrap.

## Gossip

A def with `gossip: true` writes no fixed memory. The speaker instead tells
the listener its **strongest** memory about some third program — the one
heaviest by felt intensity, ties to the lower program id — that carries a
`spreads_as` (`assets/memories/README.md`). The listener is given the
`spreads_as` memory, about that same program.

- **Never about the listener**, or it would be handed an opinion of itself,
  and **never about the speaker**.
- **One hop.** The memory a listener receives is the `spreads_as` def, which
  may not itself spread, so a rumour is never retold.
- A gossip def weighs nothing when the speaker holds nothing tellable.
- A program that has since left is still gossiped about, under the name its
  memory stamped.
- `speaker_memory`, if set, is written as usual.
