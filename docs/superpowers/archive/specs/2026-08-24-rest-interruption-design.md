# Powering down in the field can be interrupted

**Status:** **implemented and shipped in `v0.13.94`**, approved 2026-09-04.
Built as designed, with the three open questions below answered: no
first-strike penalty and no Trace raise (the burned charge is the whole
cost), and the log line does name the charge. The roll is
`Game::roll_rest_interrupt` — *not* the `Game::rest_interrupted` this
document's own history section says was documented but never existed. The
seam entry went into `docs/seams.md` and `CLAUDE.md` **with the merge**,
which is the ordering rule this feature exists as a monument to.

A charged rest rolls for an ambush once the charge has been taken. On a hit a
pack engages and nothing is restored. A free rest inside base space never
rolls, so the slab stays the one safe ground in the game.

## Why this document exists at all

This feature was built once, on a branch cut from `0.13.11`, and never
merged. `main` moved on to `0.13.20` without it. What made that expensive was
not the lost work — it is one commit and it is recoverable — but that
**CLAUDE.md described the mechanic as shipped**, in detail. (`docs/seams.md`
on `main` did *not* — the branch's own commit message says it rewrote that
file too, but that edit went down with the branch. The false claim lived only
in the gitignored twin, which is the harder half to audit.) The entry named
`Game::rest_interrupted` and
`REST_AMBUSH_CHANCE`, spelled out where the roll sat relative to the payment
and the restore, and warned about a trap in three app-core tests. None of it
existed in the source. How long that stood is unknowable — CLAUDE.md is
gitignored and has no history; the branch's commits are dated 2026-08-23.

That is the second recorded instance of a seam doc describing unmerged code
in this repo. The doc claims were removed on 2026-08-24 when the branch was
deleted. **If this spec is implemented, the seam entry goes back — and not
before.** The ordering matters more than the feature does.

## What is already there (verified 2026-08-24, against `v0.13.21`)

- `Game::rest` (`crates/engine/src/game/turn.rs:712`). Its structure today:
  the two gates (`is_game_over`, `has_active_battle`), then — for a party
  **not** in base space — resolving a charge through `rest_charge_in_pack`,
  refusing by name via `rest_charge_name` if there is none, and taking one
  unit. Then the heal, the Power refill, `drop_until_rest_buffs_on_party`,
  the roster walk, and one of two log lines.
- Its doc comment currently asserts: *"Nothing can fail after the charge is
  taken, so there is no refund path: the two gates and the payment run in
  that order and the restore is unconditional from there."* **This spec
  falsifies that sentence.** It must be rewritten in the same change, not
  left to drift.
- **No rest advances the clock.** `Game::wait` is the only way time passes
  without an action. This is load-bearing for the free half and must survive.
- `Game::maybe_ambush` runs only from `move_player`; `maybe_stack_encounter`
  runs only from a Stack step. Both know their locale by construction.
- `r` is bound in both key dispatches as of `v0.13.21` — surface at
  `playing.rs:306`, Stack in `handle_stack_key`. Before that fix it was a
  dead key underground, which is why nobody noticed the ambush was missing
  down there either.

## The design

**The roll sits below the payment and above the restore.** Three properties
fall out of that placement and each is worth stating:

1. **It rides the branch that takes the charge**, so a free base rest never
   reaches it. Base space stays safe without a locale check of its own.
2. **There is no refund.** The outlet is spent and nothing is restored. That
   is the mechanic, not an oversight — powering down in the open is what
   left the party exposed. A refund makes the risk free and the number
   meaningless.
3. **A rest that is jumped clears nothing** — the heal, the roster refill and
   `drop_until_rest_buffs_on_party` all sit below the interrupt. This is the
   rule a *refused* rest already follows, so the two failure modes agree.

**This is the first roll site that cannot know its locale by construction.**
Every other spawn path is reached from exactly one kind of movement. A rest
happens anywhere, so the pack has to be chosen: `stack_encounter_pack`
underground, the surface ambush pack above. The prior art split a
`surface_ambush_pack` out of `maybe_ambush` so the placement rules are stated
once rather than copied — that split is the right shape and should be kept.

**A roll that hits but fields no pack must lapse into an ordinary rest.**
Otherwise the charge burns for nothing at all, which is the one outcome a
player cannot read as anything but a bug.

### The constant

`REST_AMBUSH_CHANCE`, in `tuning.rs`, at `0.15` in the prior art. Its doc
there makes the argument worth preserving: it is **not comparable** to the
per-step encounter rates above it, because a rest is one discrete event
rather than a stream. It is the whole risk of powering down in the field, not
a rate that accumulates over a walk. Modest on purpose — an outlet that buys
a fight too often stops reading as a way to recover at all.

## Decisions already settled, so they are not relitigated

- **Charged rests only.** Free base rests never roll.
- **No refund**, and no partial restore.
- **Not gated by locale** — the Stack and the open grid both roll, because
  both are "not the slab". This is the same shape as rest's pricing.
- **The clock still does not advance.** The interrupt starts a battle; it
  does not fast-forward anything.

## Open questions

1. **Does an interrupted rest raise Trace underground?** `Game::raise_trace`
   holds the `is_underground` guard for all three current sources. A fourth
   source is a design decision, not a mechanical one, and the prior art did
   not address it.
2. **Should the party get a first-strike penalty**, or is the lost charge the
   whole cost? The prior art said the charge is the whole cost. Worth asking
   once in play before adding a second penalty on top.
3. **Does the log line need to name the charge that was burned?** A player
   who loses an outlet to a fight they did not choose will want to know it
   was spent.

## What implementing it touches

- `crates/engine/src/game/turn.rs` — `Game::rest`, and its doc comment, which
  currently asserts the opposite of this feature.
- `crates/engine/src/tuning.rs` — `REST_AMBUSH_CHANCE` and its argument.
- `crates/engine/src/game/spawning.rs` — splitting `surface_ambush_pack` out
  of `maybe_ambush`, so the placement rules have one home.
- `assets/help/` — the page describing resting should say it can be
  interrupted outside your base.
- `docs/seams.md` and `CLAUDE.md` — the entry goes back **only** once the
  code is merged. Note CLAUDE.md is gitignored and twinned with AGENTS.md;
  edit CLAUDE.md then `cp`.

## Test intents

- A charged rest that rolls an interrupt restores nothing, keeps the charge
  spent, and leaves a battle open.
- A charged rest that does not roll an interrupt behaves exactly as today.
- A free base rest never rolls at all — assert on the RNG stream, not just on
  the outcome, or a 15% chance passes this by luck.
- An interrupt that fields no pack lapses into an ordinary, complete rest.
- Underground and surface both roll, and each draws its own pack kind.
- Mutation-prove every one of them: delete the fix, watch it fail, restore.

## Gates

`cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`. Balance is
ungated here — `balance_sim` models no rest and no Stack term — so the arena
and a session are the only instruments for whether 15% is right.
