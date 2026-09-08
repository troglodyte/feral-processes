# A program with nothing left starts a fight

**Status:** designed 2026-09-08.

Base morale is a three-rung ladder that ends in refusal. A program sulks at
`MORALE_SULKS_AT`, downs tools at `MORALE_DOWNS_TOOLS_AT`, and then does
nothing at all, indefinitely, until its grudges decay. The worst thing a
miserable program can do to a base today is stand still in it.

This adds the rung past that one: a program far enough into the hole rounds
on another member of staff and hits it. Nobody dies. Whoever comes out worst
is swept into a Repair Bay by machinery that already exists, the aggressor
gets some relief, and the one that got hit remembers exactly who did it.

It is deliberately a *consequence*, not a mechanic the player operates. There
is no key, no screen and no setting. The only way to see one is to run a base
badly enough that a program has nowhere else to go.

## Decisions

1. **Mood is the single trigger, and needs already feed it.** The obvious
   design has two gates — an unmet need *and* bad morale — but `frayed_here`
   already writes a `BaseTile` grudge when a program runs out of what it
   needs, and `Game::morale` is the unrestricted sum of every memory. A base
   with nothing servicing a need therefore *already* drives morale down. A
   second, parallel need check would be a second meter answering the question
   the ladder exists to answer, which is the shape `update_disgruntled`'s own
   doc comment argues against ("`morale` and not `opinion_of`" — a claim
   about the body, not about one machine). Needs feed mood; mood is the gate.

2. **It is a fourth `Grievance`, not a new axis.** `Grievance::LashingOut`,
   appended after `DownedTools`. Appending is what makes this cheap in three
   separate ways: `Ord` derives from declaration order, so the ratchet in
   `update_disgruntled` sorts the new rung worst without touching the
   comparison; `SaveData`'s `disgruntled: Option<Grievance>` is field-named
   RON encoding the *variant name*, so no `SAVE_FORMAT_VERSION` bump; and the
   exit side stays the single `morale >= MORALE_RECOVERED_AT` comparison, so
   the whole ladder keeps **one** hysteresis gap rather than growing one per
   rung.

3. **The rung must be unreachable by one memory and reachable by two.** This
   is `MORALE_DOWNS_TOOLS_AT`'s own rule one step further down, and it is the
   only thing keeping the number honest. The worst single grudge the game
   ships is `mauled_by` felt by an `Abrasive` program, at valence x
   `strike_cap` x `DISPOSITION_MEMORY_SWING` = **-44.8**. `-50` was chosen to
   sit just past it; `MORALE_LASHES_OUT_AT` sits at **-75**, past what one
   memory can reach and inside what a pattern of two can. Both bounds are
   asserted against the real `assets/memories/`, mirroring
   `no_single_memory_can_down_a_programs_tools` and
   `two_bad_memories_can_still_down_a_programs_tools`. A rung nothing can
   reach is a deleted feature; a rung one bad afternoon reaches is a base
   that brawls constantly.

4. **The victim is the one it likes least, and reach decides who is eligible
   at all.** Candidates are base staff within `TANTRUM_REACH_TILES`, not
   `Downed`, not already brawling. Among them the pick is the lowest
   `opinion_of(aggressor, MemorySubject::Program(id))`, falling through to
   nearest and then to `Entity` order so the choice is deterministic on an
   all-neutral field.

   **No target in reach means no tantrum**, and that is what keeps this
   feature free of pathing. A program on the `LashingOut` rung has already
   left the posting half of `schedule_base_labour` and is drifting through
   `drift_idle_staff`, so it wanders into range of somebody on its own. A
   "walk to your enemy" arm would be a second walk with its own interruption
   rules, bought for nothing the player could see.

5. **Damage is clamped before `apply_damage`, never inside it.**
   `Game::apply_damage` is the one damaging path in the game and it floors HP
   at 0 — and reaching 0 *is* a kill, announced. Nothing in it refuses a
   lethal blow. The established idiom for guaranteed non-lethal damage is
   `game/throw.rs`'s `THROWN_ITEM_DAMAGE.min(hp - 1).max(0)`, and a tantrum
   uses exactly that. **This clamp is the whole of "nobody dies."** Anyone
   who later moves the tantrum's damage calculation without carrying the
   clamp with it turns a bad mood into a way to lose companions.

6. **The Repair Bay needs no new code.** `admit_the_badly_hurt` already runs
   inside `schedule_base_labour`, and already inserts `Downed` on any staff
   program below `BAY_ADMISSION_HP_FRACTION`. Placing the tantrum step
   *between* `update_disgruntled` and `admit_the_badly_hurt` means a blow
   landed this tick is answered by the bay this tick, through the one writer
   that already owns that decision. Damage is sized so that even the
   *shortest* brawl crosses 20% — see Tuning, where that bound is what sets
   the constant, and is asserted rather than multiplied out.

7. **A brawl is not saved.** `resources::Brawls` is a plain resource with no
   save field. A brawl lasts four to eight ticks; damage is applied as it
   goes, so a save mid-fight loses nothing but the summary lines. The
   alternative — keying it by `ProgramId` and re-resolving after the roster
   is restored, the way a patrol's tether defers — is correct and buys a
   four-tick window. It is not worth a save field and a deferred-restore
   step.

8. **Catharsis is load-bearing, not flavour.** `Disgruntled` ratchets and
   never eases: once a program is on the `LashingOut` rung it stays there
   until morale climbs all the way back to `MORALE_RECOVERED_AT`. Without
   something pushing back, a program past -75 fights every time the roll
   comes up, for the rest of the run. The `vented` memory is the push back.
   One vent does not clear a -75 hole and is not meant to — a badly-run base
   *does* descend into repeated fighting, which is the point — but the meter
   moves the right way, and `TANTRUM_COOLDOWN_TICKS` bounds the rate while it
   does.

9. **The grudge is the first negative `Program`-subject memory the game
   ships.** The two that exist (`idled_with`, `bonded_in_battle`) are both
   positive, so `MemorySubject::Program` has never yet been read as
   "something I hold against a specific colleague." The plumbing is already
   right for it: `Game::remember` resolves the subject's display name at the
   *write*, so a grudge still names the program after that program is gone.

10. **A new `EffectKind`, so raids stay silent.** The red flash itself is
    free — `EffectKind::Hit` already exists, `push_effect` reads `Position`
    off any entity rather than off a `Structure`, and gui already paints it
    `FLASH_RED` in base space. What is *not* free is sound: there is no
    bridge today from a base tick to a sound cue, and raids flash silently.
    `EffectKind::Brawl` draws identically to `Hit` and additionally makes gui
    fire `SoundEvent::Hit`. Teaching gui to sound every base-space `Hit`
    instead would be less code and would also give raids audio they have
    never had, which is a change to a shipped feature nobody asked for.

11. **`MessageKind::Tantrum` rather than reusing `Raid`.** Reusing `Raid`
    buys the log-pane border flash from `fx.rs::observe_log` for free, and
    files a scuffle between two staff as a GC Entropy Sweep — which is both a
    lie to the player and a lie to `retain_outcomes_since_battle`, whose
    keep-list includes `Raid`. A tantrum is base news and should be pruned
    like base news.

## Schema

Two new files under `assets/memories/`, and no schema change — both use
fields `MemoryDef` already has.

`vented.ron`, the aggressor's relief:

```ron
(
    id: "vented",
    name: "Let it out",
    blurb: "Put it into something that could take it.",
    valence: 5.0,
    half_life: 2000,
    subject: Nothing,
    strike_cap: 3,
)
```

`turned_on_me.ron`, the victim's grudge:

```ron
(
    id: "turned_on_me",
    name: "Turned on me",
    blurb: "It came at me, and nothing had been asked of either of us.",
    valence: -7.0,
    half_life: 5000,
    subject: Program,
    strike_cap: 4,
)
```

`vented` is short-lived and capped low on purpose: relief is meant to take the
edge off, not to be a way of *farming* morale by starting fights.
`turned_on_me` outlasts it, which is why a base that answers a bad mood with
nothing gets worse rather than better.

Both need a row in `MEMORY_TRIGGERS` (`crates/engine/src/tests/assets.rs`),
which is what makes `every_shipped_memory_def_is_reachable_from_a_trigger`
pass.

## Types

```rust
// components.rs — appended, never inserted
pub enum Grievance { Sulking, DownedTools, LashingOut }

// resources.rs — not saved
#[derive(Resource, Default)]
pub struct Brawls(pub Vec<Brawl>);

pub struct Brawl {
    pub aggressor: Entity,
    pub victim: Entity,
    pub ticks_left: u32,
    pub dealt: i32,   // aggressor -> victim, cumulative
    pub taken: i32,   // victim -> aggressor, cumulative
}

pub enum EffectKind { Hit, Deflected, Destroyed, Brawl }
pub enum MessageKind { /* ... */ Tantrum }
```

`Brawl` holds `Entity` rather than `ProgramId` precisely because it is not
saved; entity ids are stable within a session, which is the whole lifetime
this record has.

## Tuning

A new `// Staff tantrums` light subsection beside `// Acting out` in
`crates/engine/src/tuning.rs`, which is where the morale ladder already
lives.

| Constant | Value | Held by |
|---|---|---|
| `MORALE_LASHES_OUT_AT` | -75.0 | reachable by two memories, not one |
| `TANTRUM_CHANCE_PER_TICK` | 0.02 | — |
| `TANTRUM_REACH_TILES` | 3 | — |
| `TANTRUM_TICKS_MIN` / `_MAX` | 4 / 8 | min < max |
| `TANTRUM_DAMAGE_FRACTION` | 0.25 | crosses 20% at the *short* end of the tick range |
| `TANTRUM_COOLDOWN_TICKS` | 400 | — |

Every one of these is unmeasured, for the reason the `// Acting out` comment
already gives: morale is a signed sum of decayed intensities with no natural
scale, and nothing in `balance_sim` models base production. They are chosen
against the shipped valences and against `BAY_ADMISSION_HP_FRACTION`, and
they are the first thing to revisit after a base has been watched.

`TANTRUM_DAMAGE_FRACTION` is the one with a real constraint on it, and the
constraint binds at the **short** end of the range, not the long one: a
four-tick brawl is the shortest one that can happen, so four blows starting
from full health must already cross `BAY_ADMISSION_HP_FRACTION`. That needs
at least 20% of max HP per blow before mitigation, which is why this number
is a quarter rather than the single-digit fraction a "scuffle" suggests.

Two things make the arithmetic softer than it looks, and both push the same
way. `Game::apply_damage` runs `mitigate_incoming_damage`, so the clamp is
applied to the *input* and the landed figure is lower than the fraction
says; and a brawl rarely starts from full health, since a program this far
into the hole has usually been in one before. So the constant is pinned by
`a_short_brawl_still_fills_the_bay` driving a real fight against real
mitigation, **not** by the multiplication above. If a retune moves
mitigation, that test is what notices.

At the long end the `.min(hp - 1)` clamp does the rest of the work: eight
blows at a quarter each would be 200% of max HP, so a long brawl simply
leaves both parties on 1 Integrity rather than killing either. Heavy is the
point; the clamp is what keeps heavy from being fatal.

## Flow

Inside `schedule_base_labour`, after `update_disgruntled` and before
`admit_the_badly_hurt`:

1. **Advance every open brawl.** For each, both parties swing; each blow is
   `(TANTRUM_DAMAGE_FRACTION * max_hp).min(hp - 1).max(0)` through
   `apply_damage`, accumulated into `dealt`/`taken`, with a
   `push_effect(target, EffectKind::Brawl)` per landed blow. Decrement
   `ticks_left`.
2. **Close what has finished.** A brawl ends on `ticks_left == 0`, on either
   party being `Downed` or gone, or on either leaving base staff. Closing
   writes the result lines and both memories.
3. **Open at most one new brawl per candidate.** For each `LashingOut`
   program not `Downed`, not brawling and off cooldown, roll
   `TANTRUM_CHANCE_PER_TICK`; on a hit, pick a victim by decision 4 and log
   the alert.

Opening is deliberately the *last* of the three steps, so a brawl that starts
this tick throws its first blow on the next one. The alert therefore always
precedes any damage in the log, and a fight is never opened and advanced in
the same pass — which is what keeps `ticks_left` an honest count of
exchanges rather than one that is sometimes short by one.

Then `admit_the_badly_hurt` runs as it already does and takes whoever fell
under the line.

## The lines

All `MessageKind::Tantrum` through `log_base_kind`.

On opening:

> Cutter rounds on Sieve.

On closing, up to three lines:

> Cutter hurt Sieve in a tantrum, causing 34 damage.
> Sieve fought back, causing 21 damage.
> Cutter came out of it calmer. Sieve will not forget it.

The second line is **omitted** rather than printed as `0 damage` when the
victim never landed one — which happens whenever the bay took the victim on
the first exchange.

## Testing

- The ladder climbs in order, extending `the_ladder_climbs_in_order` to the
  third rung.
- `MORALE_LASHES_OUT_AT` is past the worst single shipped grudge, and inside
  two of them. Both against the real `assets/memories/`.
- **A tantrum never kills.** Drive a full brawl against a victim left on 1 HP
  and assert it is still alive. Deleting the clamp must fail this.
- **A tantrum does fill the bay, at the short end.**
  `a_short_brawl_still_fills_the_bay` — drive a brawl between two
  full-health staff for `TANTRUM_TICKS_MIN` and assert at least one carries
  `Downed`. `TICKS_MIN` and not `MAX`: the long case passes for free, and a
  test written against it would go green with a damage constant far too low
  to hold the guarantee this feature is sold on.
- Both memories land, and `turned_on_me` names the aggressor's `ProgramId`.
- No candidate in reach means no brawl **and no RNG drawn** — the tick
  consumes nothing from `GameRng` when nothing is on the rung, so this
  feature cannot shift the seeded stream. `Game::run_routes`' predation test
  is the pattern.
- The cooldown holds: a program that has just finished a brawl cannot open
  another before `TANTRUM_COOLDOWN_TICKS`.
- A `Downed` program is neither an aggressor nor a victim.
- `MEMORY_TRIGGERS` census (automatic once the rows are added).

## Out of scope

No key, no screen, no notification, no `Game::attention` entry. A program in
the bay is already how the base tells the player something went wrong, and
the log lines are the narration. Adding an attention row would make a
tantrum something the player is expected to *respond* to, and there is
nothing to do about one except fix the base that caused it.
