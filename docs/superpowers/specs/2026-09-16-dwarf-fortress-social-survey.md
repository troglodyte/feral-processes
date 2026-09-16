# Dwarf Fortress social behaviours: what is worth importing

**Status:** survey, not a design. The candidates selected out of it get their
own design docs.

Written 2026-09-16, against the shipped state of `disposition.rs`,
`memories.rs`, `needs.rs`, `components::Grievance` and
`game/base/tantrum.rs`.

## What DF's social sim actually is

Six machines, in dependency order:

1. **Personality** — five-factor facets (~50), ~30 cultural values, goals.
   Decides how hard an event lands on *this* dwarf and what it seeks out.
2. **Preferences** — likes a material, a creature, a food, a colour. A
   flavour generator that also emits small real thoughts on contact.
3. **Thoughts → emotions → stress** — every event emits a thought carrying an
   emotion at a strength; emotions fade; the running sum is stress;
   personality scales the intake. Up to eight *core memories* never fade and
   keep re-firing.
4. **Needs** — about eighteen, each met by an activity or an assigned
   location.
5. **Relationships and gossip** — a per-pair value built by conversation and
   laddered into named bands, plus a rumour network: witnessing an event lets
   you tell others, who get a weaker version of the thought.
6. **The spiral** — stress bands drive tantrum, melancholy or berserk;
   property destruction and brawls give every witness a new bad thought;
   the loop has no damper.

It generates stories because of three properties and not because of the
content on top of them: events **propagate beyond who saw them**, the same
event **reads differently per individual**, and the negative loop has **no
ceiling**. feral-processes already has the second and the third. It has none
of the first.

## What this game already has

| DF machine | Here | State |
|---|---|---|
| Personality facets | `disposition.rs` — five temperaments, two axes, hidden, minted from `ProgramId` | Exists; only drain rate and memory weight |
| Preferences | — | Absent |
| Thoughts → stress | `memories.rs`, `Game::morale` = `sum_intensity` | Exists, and factored better than DF's: a memory is *about* something, decays by half-life, caps strikes |
| Core memories | — | Absent; everything decays |
| Needs | `needs.rs` — two defs, hysteresis via `OffShift`, amenities | Exists; shallow catalogue |
| Relationship value | `opinion_of(who, Program(id))` over `bonded_in_battle`, `idled_with`, `turned_on_me` | Exists as a float; no ladder, no name, almost no consequence |
| Gossip | — | Absent; a memory is written only to whoever was there |
| The spiral | `Grievance` (Sulking → DownedTools → LashingOut); `tantrum.rs` picks its victim by lowest opinion and the victim writes `turned_on_me` | Exists, closed, and the closest thing to DF in the codebase |
| Reputation | — | Absent |
| Strange moods | — | Absent |
| Named individuals | `CustomName` if the player renames, else two Scrappers are both "Scrapper" | The blocker under half the list below |

## The candidates

### Tier 1 — cheap, and they make what exists legible

1. **Per-program handles.** Auto-mint a name at `roster_parts`, from a pool in
   `assets/`. Without it, "Scrapper holds a grudge against Scrapper" is
   unreadable and every relationship feature is invisible. A prerequisite,
   not a feature.
2. **A relationship band, derived and never stored.** `Standing::band`'s
   pattern turned on `opinion_of(a, Program(b))` — a named query, exhaustive,
   no save field.
3. **Preferences.** One or two derived off `ProgramId`: a material, a
   structure kind, a locale. Derived, never stored — `settlement_at`'s rule.
4. **More need defs.** Two is thin for a system built to hold a dozen.
   Hard-capped by `MAX_NEED_ROWS` and the WORK box.

### Tier 2 — the real import

5. **Gossip.** A memory written to a witness is re-written at reduced valence
   to programs in reach, at a rate, with a propagation cap. This is what turns
   one tantrum into a base-wide event.
6. **Witnessing.** A tantrum currently writes `turned_on_me` on the victim
   alone; DF writes on everyone who saw it. Small change, large delta, and
   the natural substrate for gossip.
7. **A social axis on `Disposition`.** `Amiable`/`Abrasive` are named socially
   but only scale memory weight. How much a program bonds versus how much it
   spreads is the lever that keeps gossip from feeling uniform.
8. **Reputation.** What a program is *known for*, derived from its own ledger
   rather than stored — `Game::program_role`'s pattern.

### Tier 3 — each is its own feature

9. **Core memories.** A small non-decaying set. Gives morale a floor, and
   redefines the invariant that intensity is derived on every read.
10. **Strange moods.** A program claims a bench, demands materials, produces a
    unique item or breaks. Fits the production chains well; costs a new item
    provenance path, a new machine state and a new death.
11. **Programs talking.** Interaction events between idle staff. High cost,
    and `message_history` condenses the payoff away.

## Left in DF deliberately

- **Values and beliefs.** Thirty axes of belief violated by events: the part
  of DF nobody can read, producing the most inexplicable behaviour. Does not
  survive a HUD column of 38.5 cells.
- **Social skills as a skill tree.** A second progression axis not earned by
  fighting, straight through the design spine.
- **Justice, nobles and mandates.** The base has no political layer.
- **Kinship and breeding.**

## Recommendation

The cluster that earns its cost is **1, 2, 5 and 6** as one feature — "what
the base knows about itself" — built on the memory system that already
exists. It is what makes the shipped tantrum spiral read like DF rather than
like a private argument between two entities the player cannot tell apart.

Candidates 3 and 4 are independent and can land separately.
