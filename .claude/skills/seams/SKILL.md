---
name: seams
description: Use before changing a load-bearing seam in feral-processes - the base and its labour scheduling, the Stack, combat/XP/balance, items and gear, sorties, memories, needs, notifications, the HUD, saves, logs or screens. Carries the trap behind each rule CLAUDE.md states in one line. Also use when a rule in CLAUDE.md's "Load-bearing seams" section reads as arbitrary, or when adding a seam to it.
---

# Load-bearing seams: the traps

`CLAUDE.md` states each seam as **one sentence — the rule alone**. This skill
holds the second half: **the trap the rule exists to close**, in the compressed
form that used to live in `CLAUDE.md` itself. `docs/seams.md` is the third tier
— the full argument, the measurement, the history, and what was tried and
rejected.

Three tiers, and which one you want depends on what you are doing:

| | where | read it when |
|---|---|---|
| the rule | `CLAUDE.md`, always in context | always |
| the trap | `references/*.md` here | before changing code in that subsystem |
| the argument | `docs/seams.md`, same title | before changing **the seam itself** |

## How to use this

1. Find the subsystem you are about to touch in the table below.
2. Read that reference file — the whole file, not a grep. The traps are
   cross-referenced (`cell_mark`'s rule, `NoPost::BoxedIn`'s rule,
   `party::role_of`'s reason) and a single bullet read alone loses them.
3. If you are changing the seam rather than working within it, read the
   matching `###` entry in `docs/seams.md` before you edit.

| subsystem | reference | seams |
|---|---|---:|
| the base, base space, labour, work orders, digging, building, needs | `references/base.md` | 76 |
| combat, damage, XP, levels, talents, perks, balance, spawning | `references/combat.md` | 66 |
| items, gear copies, quality, crafting, the caravan, the economy | `references/items.md` | 33 |
| the Stack (frames, descents, lairs, descriptions, first-person views) | `references/stack.md` | 22 |
| saves, the log, refusals, screens, the Broker board, paths | `references/screens.md` | 21 |
| what a program remembers (memories, morale, opinion) | `references/memories.md` | 11 |
| the HUD (attention, panes, the palette, glyph colour) | `references/hud.md` | 14 |
| sorties | `references/sorties.md` | 10 |
| notifications | `references/notifications.md` | 5 |
| species and data (classes, stat shapes, censuses) | `references/species.md` | 6 |
| help pages and documentation | `references/help.md` | 3 |
| the ground (terrain effects, Static weather) | `references/ground.md` | 3 |

## Adding a seam

A new seam is **three writes, in this order**:

1. The **argument** goes to `docs/seams.md` as a new `###` section — the
   measurement, what was tried, what was rejected.
2. The **trap** goes to the matching `references/*.md` here, as a bullet in
   the existing house style: a bold rule sentence, then the trap.
3. The **rule** goes to `CLAUDE.md` under the same `###` heading, as a bullet
   of **exactly one sentence**. That budget is the point — `CLAUDE.md` reached
   151 KB by letting each seam's trap creep back in beside its rule, and it is
   loaded on every turn.

Each was verified against the source, not remembered. Verify again before
relying on one, and correct **all three** files if it has moved.
