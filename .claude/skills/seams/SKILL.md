---
name: seams
description: Use before changing a load-bearing seam in feral-processes - the base and its labour scheduling, the Stack, combat/XP/balance, items and gear, sorties, memories, needs, notifications, the HUD, saves, logs or screens. Carries the trap behind each rule `.claude/rules/seams-*.md` states in one line. Also use when one of those rules reads as arbitrary, or when adding a seam to it.
---

# Load-bearing seams: the traps

`.claude/rules/seams-<subsystem>.md` states each seam as **one sentence — the
rule alone**, path-scoped so it loads when a matching file is read. This skill
holds the second half: **the trap the rule exists to close**, in the compressed
form that used to live in `CLAUDE.md` itself. The **memory graph** is the third
tier — the full argument, the measurement, the history, and what was tried and
rejected.

Three tiers, and which one you want depends on what you are doing:

| | where | read it when |
|---|---|---|
| the rule | `.claude/rules/seams-*.md`, loaded by path | before touching that subsystem |
| the trap | `references/*.md` here | before changing code in that subsystem |
| the argument | the memory graph, `seam:<slug>` | before changing **the seam itself** |

## How to use this

1. Find the subsystem you are about to touch in the table below.
2. Read that reference file — the whole file, not a grep. The traps are
   cross-referenced (`cell_mark`'s rule, `NoPost::BoxedIn`'s rule,
   `party::role_of`'s reason) and a single bullet read alone loses them.
3. If you are changing the seam rather than working within it, read its
   argument out of the graph before you edit:

   ```
   memory_search(<the rule's own words>, subsystem: "seams")   # find the seam
   memory_get_entity("seam:<slug>")                            # read it whole
   ```

   Search returns a 400-character window per hit, which is a result list and
   not the argument — always follow it with `memory_get_entity`. Scope the
   search with `subsystem: "seams"`: an argument runs to 2,500 characters on
   average, and a long observation dilutes term frequency badly enough that a
   bare identifier can rank a short unrelated note above the seam that
   discusses it.

   The slug is the entry's title, lowercased, apostrophes dropped, every other
   run of non-alphanumerics collapsed to `-`, cut to 60 characters on a word
   boundary. Don't reconstruct it from a rules-file rule — the rule sentence
   and the argument's title are not the same string. Search, then read.

| subsystem | reference | seams |
|---|---|---:|
| the base: structures, building & raids | `references/base-structures.md` | 25 |
| the base: labour scheduling & postings (assignment, wander, squeeze-past) | `references/base-labour.md` | 17 |
| the base: production chains, machines, hauling, depots & the transfer screen | `references/base-production.md` | 39 |
| the base: digging, rock & the base grid | `references/base-digging.md` | 17 |
| the base: needs, morale, tantrums, repair bays, pins & research | `references/base-needs.md` | 31 |
| the base: instrumentation (the production ledger, base output) | `references/base-instrumentation.md` | 6 |
| combat: kit, Power, routines & abilities | `references/combat-kit.md` | 15 |
| combat: attack resolution, mitigation, XP, levels, talents & the cap | `references/combat-progression.md` | 24 |
| combat: spawning, bosses, difficulty & the roster doors | `references/combat-spawning.md` | 13 |
| combat: battle flow, rewards, arena, rest & pursuit | `references/combat-battle.md` | 24 |
| combat: tactical battles — turn flow, movement & targeting | `references/combat-tactical-core.md` | 25 |
| combat: tactical battles — reactions, fx, AI aim & squads | `references/combat-tactical-actions.md` | 22 |
| items, gear copies, quality, crafting, the caravan, the economy | `references/items.md` | 34 |
| the Stack (frames, descents, lairs, descriptions, first-person views) | `references/stack.md` | 22 |
| saves, the log, refusals, screens, the Broker board, paths | `references/screens.md` | 21 |
| what a program remembers (memories, morale, opinion) | `references/memories.md` | 12 |
| the HUD (attention, panes, the palette, glyph colour) | `references/hud.md` | 14 |
| sorties | `references/sorties.md` | 10 |
| the player's implants and Phase Keys | `references/implants.md` | 4 |
| notifications | `references/notifications.md` | 6 |
| species and data (classes, stat shapes, censuses) | `references/species.md` | 6 |
| help pages and documentation | `references/help.md` | 3 |
| the ground (terrain effects, Static weather, settlements and towns) | `references/ground.md` | 24 |
| traps (honeypots: placing, the tick, what a catch is worth) | `references/traps.md` | 4 |
| sieges (the board, besiegers, turrets, the siege save) | `references/sieges.md` | 10 |

## Adding a seam

A new seam is **three writes, in this order**:

1. The **argument** goes to the graph — the measurement, what was tried, what
   was rejected:

   ```
   memory_add_observations(
     entity: "seam:<slug>", entityType: "seam", subsystem: "seams",
     observations: [<the title>, <the argument>])
   ```

   Two observations, title first: the short one is what search ranks on, the
   long one is what `memory_get_entity` serves. `entityType: "seam"` is
   load-bearing — the SessionStart digest pins only `user`, `preference`,
   `constraint` and `convention` types, so this type is what keeps 331
   arguments out of every session's opening context.
2. The **trap** goes to the matching `references/*.md` here, as a bullet in
   the existing house style: a bold rule sentence, then the trap.
3. The **rule** goes to the matching `.claude/rules/seams-*.md`, as a bullet
   of **exactly one sentence**. That budget is the point — the rules were
   151 KB of `CLAUDE.md` once, by letting each seam's trap creep back in
   beside its rule. Widen the file's `paths:` if the new seam lives in a file
   it does not match.

Still three writes: moving the argument into the graph changed where the third
one lands, not how many there are. Two arrives only if the trap tier moves too.

Each was verified against the source, not remembered. Verify again before
relying on one, and correct **all three** places if it has moved.
