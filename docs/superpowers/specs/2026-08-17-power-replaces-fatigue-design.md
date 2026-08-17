# Power replaces Fatigue

**Status:** approved 2026-08-17, not implemented.

> `INDEX.md` warns that this header is the one line in a spec nobody ever
> revises. Answer "did this ship" from `CHANGELOG.md` and a grep, never from
> here.

Closes the `TODO.md` item reading "remove fatigue. use power as well as
cooldown for routine calls. allow the ability to use consumables like 'power
battery' during combat. every companion would also track there power level."
Cited by its text rather than its number: the numbering has already shifted
once, and `2026-08-17-nemesis-design.md` claims to close "#24" too.

## The problem

The game has two need meters and only one of them is a mechanic.

**Power** (`Needs::hunger`, surfaced everywhere as "Power") drains at 0.15 a
tick, is the only thing that can kill you by attrition, and scales your
attack down to ×0.5 below 50. It is a real clock.

**Fatigue** (`Needs::fatigue`) *refills* at 0.08 a tick and is spent by
exactly two things in the entire game — the Stack's `Phase` and `Jump`
routines. Battle Specials stopped charging it on 2026-08-08 and are priced
in cooldowns alone. It is a meter, a save field, a `FieldBuffKind`, a
`ConsumeDef` field, a serde default and a row on the status bars, all in
service of two routines.

Meanwhile the thing a player actually spends all game — calling routines —
is priced only in cooldowns, which are a *pacing* device, not a *budget*.
Cooldowns say "not again yet". Nothing says "not any more".

The change: delete Fatigue, and make Power the budget that routine calls
draw on, alongside the cooldowns they already have. Give every companion its
own reserve so a party's casting is a resource the player manages rather
than a free ability that fires whenever it is off cooldown.

## Where the scarcity lives, and where it does not

**Decided: Power stays soft on the surface and bites underground.** The
Recharger Node is not touched, `Perk::LowPowerMode` is not touched, and the
position recorded at `perks.rs:91` — that Power has a structural answer and
Trace does not — still stands on the surface.

This is a property of the code rather than a rule anyone has to remember. A
Recharger's `power_regen` has a radius and a base is where rechargers get
built, so a Stack run already carries whatever Power the party walked in
with. The design does not add a scarcity mechanic; it adds a *spender* to a
place that already had no supply.

**This depends on one bug fix, and the feature is decorative without it.**
`systems::power_regen_system` reads the player's `Position`. While
underground that `Position` is pinned to the surface entrance tile — so a
Stack link inside a Recharger's radius regenerates Power the entire way
down. Harmless today, because nothing underground spends Power. The moment
routines are priced in it, it means "site a base near a link and Power is
free in the Stack", which deletes the whole answer above.

The fix is `Res<Locale>` and an early skip while underground. This is
exactly the trap `CLAUDE.md` records for `nest_aggro_tick`: a reader of the
player's `Position` that needs the underground guard even though it never
went through `require_surface`. The distinction that decides it is not "does
this act" but "does this claim something about where the party is", and a
regen tied to standing near a structure claims precisely that.

## Vocabulary

One word, **Power**, for a thing that currently has three names — a struct
called `Needs`, a field called `hunger`, and a status bar reading "PWR".

- component `components::PowerReserve`
- constants `components::POWER_MIN`, `components::POWER_MAX` — today
  `NEED_MIN` / `NEED_MAX`, renamed in place. They stay in `components.rs`
  rather than moving to `tuning.rs`, for the reason their own doc comment
  already gives: they are the type's documented invariant rather than a
  difficulty knob.
- ability field `AbilityDef::power_cost`

Not `Power` for the component. `Stats::power()`, `AbilityEffect`'s buff
magnitudes and `FieldBuffKind`'s per-tick `power` argument all already use
that word for unrelated things.

The word **fatigue** leaves the codebase and the assets entirely. A grep for
it after this lands should return nothing but `CHANGELOG.md`.

## `PowerReserve` holds its own clamp

`components.rs:122` documents the invariant today:

> anything writing `hunger` or `fatigue` has to clamp to it

It is held by convention across roughly ten sites, each hand-rolling
`.max(NEED_MIN)` or `.min(NEED_MAX)`. Removing Fatigue leaves `Needs` a
one-field struct, which is the moment to convert that convention into a
barrier — the same move as `Game`'s private `world` field.

```rust
pub struct PowerReserve(f32);
```

The float is **private**. The API is the whole set of operations the ten
call sites actually perform, and nothing speculative:

- `new(f32)` — clamps; the load path and spawn path
- `get() -> f32` — the view structs and the attack multiplier
- `holds(cost: f32) -> bool` — the refusal checks
- `spend(cost: f32)` — clamps at `POWER_MIN`
- `restore(amount: f32)` — clamps at `POWER_MAX`
- `fill()` — `Game::rest`, which sets outright rather than adding
- `raise_to_at_least(floor: f32)` — `difficulty.rs`'s Forgiving reboot,
  the one site that raises to a floor rather than adding

`raise_to_at_least` exists because that call is genuinely a third shape, not
because a future caller might want it. If the Forgiving reboot ever changes
to an additive top-up, delete it.

The `views.rs` DTOs stay plain `power: f32`. They are read-only and cross
the crate boundary to gui; there is nothing for them to protect.

## One cost field, one gate, one spender

### The field

Two cost fields become one.

- delete `AbilityDef::fatigue_cost`
- delete `power_cost` from the `AbilityEffect::FieldBuff` variant
- add `AbilityDef::power_cost: f32`, `#[serde(default)]`

**The default is 0.0, not the current 5.0.** `DEFAULT_ROUTINE_FATIGUE_COST`
is 5.0 for a reason its own doc comment records: it was the price of
commanding a companion, back when the field reached battle. It reaches only
`Phase` and `Jump` today. This change widens its reach to *every* routine in
the game, and a nonzero default under that reach silently prices every
ability a mod ships. Free-by-default is the only safe default once a field's
audience widens; a mod that means to charge says so.

It is also what keeps the five uncosted shipped abilities behaving exactly
as they do today — see the content pass below. Every ability the game means
to charge for already carries a number, so the default is a genuine
fallback rather than a disguised game-wide price.

`DEFAULT_ROUTINE_FATIGUE_COST` is deleted. So is
`a_field_buff_leaving_fatigue_cost_at_its_default_is_silent` and the
dead-fields-warning exemption it guards — with one field there is no longer
a second one to leave at its default by accident.

### The gate

`Game::ability_unavailable` (`game/combat.rs:852`). One function, already
the single seam that both `battle_special_options` (which greys the row) and
`battle_set_action` (which refuses the plan) read, so a greyed row can never
be planned. Its doc comment currently states the *opposite* position in as
many words — "A need is deliberately not among them" — and rewriting that
paragraph is part of the change, not a tidy-up after it.

The check is `reserve.holds(def.power_cost)`, returning the same shape of
reason string the cooldown branch returns. It reads the reserve of the
**entity being asked about**, which is what makes companion reserves work
with no second code path.

`Game::field_routines` (`game/field.rs:30`) loses its `match` on the effect
entirely. Every row reads `def.power_cost` and every unit label is "PWR";
the `(cost, held, unit)` tuple collapses to `(cost, reserve, "PWR")`. The
ordering comment about stating the permanent objection ("only in the Stack")
ahead of the temporary one still holds and stays.

### The spender

One function, `Game::spend_power(entity, cost)`, replacing `spend_fatigue`
(`field.rs:403`) and the inline write at `field.rs:310`.

**The caster pays.** The player's Special draws on the player's reserve, a
companion's on its own. This falls out of taking `entity` rather than
assuming the player, and it is the entire implementation of "every companion
tracks their power level" on the spending side.

Spending happens where the Special resolves, after every refusal. The
existing note on `cast_jump` — that a lethal Jump still charges, because the
routine ran and what it found at the address is not refundable — carries
over unchanged.

## Companion reserves

### There are four doors into the roster, and the fourth is the trap

A program becomes a companion at four sites, all of which need a
`PowerReserve` inserted, plus the load path for companions already in a
save:

| site | what it is |
|---|---|
| `lifecycle.rs::grant_starting_program` | the program a new game starts with |
| `combat_rewards.rs:811` | a successful capture |
| `spawning.rs::adopt_program` | the one non-fight path |
| `party.rs::fuse_companions` | **spawns its own tuple, bypassing `adopt_program`** |

The first three each insert the identical tuple
`(Tamed { owner: player }, Experience::default())`. Fusion does not: it
despawns both parents and calls `world.spawn` with a component list it
assembles itself. `CLAUDE.md` already records that `fuse_companions` "does
its own `retain`/`despawn` and skips the detachment logging" — this is the
same divergence, one component further on.

Nothing about `world.spawn` or `.insert` fails to compile when a component
is missing from one of four hand-written tuples, so **the four sites get a
shared constructor**: `Game::roster_parts()` returning
`(Tamed, Experience, PowerReserve)`, called by all four. This is the pattern
`work_node_parts()` already sets in this repo, and `CLAUDE.md` names that
helper's omission as a failure that "reads as a payout curve that moved
rather than as a fixture short something". A fused companion silently unable
to cast would read exactly the same way — as fusion being bad, not as a
missing component.

A missing reserve is treated as **refusing**, never as unlimited:
`ability_unavailable` reads `Option<&PowerReserve>` and a `None` refuses.
Between a companion that cannot cast and one with infinite Power, the
former is the failure that gets reported.

Reserves are full on arrival at every door.

**`needs_tick_system` stays `With<Player>`.** This is the load-bearing
decision of the whole section. A companion's reserve never drains passively
— it only moves when the companion spends or something restores it. That
keeps the starvation branch (`stats.hp -= 1` at zero) and
`battle::power_attack_multiplier` player-only *by construction* rather than
by a guard someone can forget to write on the next system that touches
reserves.

A companion at zero Power is not punished. It cannot call routines and falls
back to plain attacks. There is no companion equivalent of starving, and
nothing about an empty reserve may lower HP.

**Refill is `Game::rest`.** It already full-heals every owned program,
including ones left behind guarding a structure during a raid, and it
already refuses anywhere but inside the base. Topping up every owned
program's reserve in the same loop is one line and gives the party's casting
budget the same base-bound shape as everything else.

**In-battle refill of a companion is explicitly out of this stage.**
`Game::consume_item` hardcodes `self.player_entity()`, so handing a
companion a Power Cell mid-fight means `BattleAction::UseItem` grows a
target and the item picker grows a second step. Rest as the sole refill is
the cleaner scarcity story and far less UI. Revisit only if drained
companions play as dead weight — that is a question for the arena and a
session, not for this spec.

Note that consumables in battle already work and need nothing:
`BattleAction::UseItem` spends the round, `Game::battle_usable_items` lists
what the player holds, and `power_cell.ron` restores 25 Power. The item's
half of the original ask is shipped.

## The renderer already has the column

`crates/gui/src/render/battle.rs` draws a Fatigue column in the battle party
roster — `fatigue_cell`, width `FATIGUE_W`, pinned by
`the_fatigue_column_holds_its_place`. It reads `PartyRow::fatigue:
Option<f32>` and renders `—` for every companion, because
`a_companions_fatigue_cell_is_a_dash` is the truth today: only the player
has a `Needs`.

That column *is* the companion Power display. It is already laid out,
already width-pinned, and currently dead for every row but one. The change
is a rename to `power_cell` / `POWER_W`, and companions start returning
`Some` instead of `None`.

`a_companions_fatigue_cell_is_a_dash` inverts rather than being deleted: a
companion's cell must now show a number, and the dash case survives only for
a row with no reserve at all. Given the four-doors trap above, that dash is
worth keeping precisely as the visible symptom of a missed door.

The manifest and status bars already render Power off `hunger` and need only
the field rename. No new screen, no new menu row, no layout change — which
matters, because `CLAUDE.md` records the manifest column packer as
order-sensitive and its fixtures as having hidden a live overflow once.

## Content repoint

### `FieldBuffKind::Coolant` merges into `Trickle`

With one need there is one per-tick restore kind. `Coolant` is deleted and
its arm in `apply_field_buff_tick` goes with it.

This collides two shipped abilities, which become the same ability:

| file | today | after |
|---|---|---|
| `coolant_flush.ron` | +1 Fatigue/turn, 90 turns, 15 Power | identical to below |
| `trickle_charge.ron` | +1 Power/turn, 80 turns, 20 Power | unchanged |

Delete `coolant_flush.ron` and remove it from `field_ops.ron`'s
`unlocks_abilities`. Its description string ("+1 Fatigue per turn for 90
turns") is the only other place it is named.

**`trickle_charge` becomes the underground Power economy, and that is a
deliberate acceptance rather than an oversight.** It nets +60 Power for 20
spent, so a party that installs it has an in-Stack generator bought with a
routine slot. That is a legitimate answer to scarcity — the slot is the
price — but it softens the Stack considerably and its numbers must be
re-tuned as part of this change rather than discovered later. It is the
single highest-leverage number in the feature.

### `ConsumeDef::fatigue`

Deleted. No shipped item sets it; it is dead schema. Verified by grep across
`assets/items/*.ron`.

`game/catalog.rs:288`'s `+{:.0} rest` line goes with it.

### The two movement routines

`Phase` (12.0) and `Jump` (20.0) keep their numbers, now denominated in
Power.

This is a substantial hidden retune and must be treated as one. Twenty
Fatigue was worth about 250 ticks of walking at `FATIGUE_REGEN_PER_TICK`.
Twenty Power, underground, with the regen hole closed, is a fifth of an
irreplaceable reserve. `tuning.rs:1483` already admits the current number is
"arithmetic, not playtested"; it is now arithmetic against a different
denominator.

### The 71 ability files: the costs already exist

**No costs are authored for this change.** The numbers are already in the
files, and the content pass is a mechanical key flip.

`AbilityDef::fatigue_cost` is documented in three places as reaching only
`Phase` and `Jump` — `tuning.rs:445` says the default "covers a mod that
omits it rather than anything in the game", and `combat.rs:850` says the
field "is read only by the two Stack field routines". Both are true about
what the *engine reads*. Neither is true about what the *assets contain*:

| files | today | after |
|---|---|---|
| 55 | author a `fatigue_cost`, **never read by anything** | key renamed to `power_cost`, values unchanged |
| 11 | author `power_cost` inside the `FieldBuff(…)` effect | hoisted to the top level, values unchanged |
| 5 | author no cost at all | untouched; inherit the 0.0 default |

The 55 are vestigial from before 2026-08-08, when battle Specials still
charged Fatigue. They were priced when the field meant exactly what
`power_cost` is about to mean — "what running this costs the player" — and
they are a real distribution rather than a placeholder: 0.0 to 20.0 against
a pool of 100, clustered at 8–12. This change brings dead data back to life
instead of inventing new data beside it.

`decompile.ron` already carries `fatigue_cost: 0.0`, independently agreeing
with the argument that taming is priced in its ICE Breaker catalyst and must
not become a third thing to afford.

The five uncosted files — `deadlock`, `hot_patch`, `memory_leak`,
`priority_boost`, `sandbox` — inherit 0.0 and so keep behaving exactly as
they do today. That is the desirable outcome rather than an accepted gap:
`priority_boost` is the fallback every companion has when its species grants
nothing, and a companion whose only routine is unaffordable has nothing to
choose but a plain attack.

### One knob, because the scale is wrong even though the shape is right

Those 55 values were tuned against a Fatigue pool refilling at 0.08 a tick —
cheap and renewable. They will now be spent from an irreplaceable
underground reserve. The relative ordering between abilities is worth
keeping; the absolute scale almost certainly is not.

`tuning::ROUTINE_POWER_COST_MULTIPLIER`, default 1.0, applied wherever a
cost is read — so both the refusal in `ability_unavailable` and the spend in
`spend_power` scale together and cannot disagree. It covers `Phase` and
`Jump` too; one knob, no exemptions.

This gives tuning two levels with different costs. The whole curve moves by
editing one constant and rebuilding. A single ability moves by editing its
`.ron` and restarting the game — no rebuild, which is the faster loop and
the one that matters during a session.

Because the multiplier is a difficulty knob rather than content, it lives in
`tuning.rs`, where `CLAUDE.md` requires it.

### `proc_wielded_routine` stays free

`tuning.rs:490` states that the 25% proc rate *is* the routine's whole
price, and the `W` key is an undocumented easter egg a gui test holds the
help text to never naming. Charging the wielded program's reserve would
quietly degrade it. This leaves a hole — wield a program with an expensive
routine and fire it free at 25% — bounded by the proc rate, and accepted.

## Hostiles get no reserve

Enemy Specials stay priced in cooldowns alone.

`Game::choose_wild_action` decides move and target as a single joint choice
scored against `assets/policies/enemy_battle.ron`, and those weights were
trained against today's action distribution. A Power constraint changes
which moves are available in which rounds, which changes that distribution,
which costs a retrain — and `CLAUDE.md` records that individual trained
weights are unidentifiable across seeds, so a retrain is not a cheap
re-run.

The player gains nothing visible from enemies budgeting Power. Left out.

## Save format

**One `SAVE_FORMAT_VERSION` bump**, earned by `PlayerSave::fatigue`. It is a
field removed, which the format's own rule says still costs a bump even
though the file is field-named RON.

The field is deleted rather than kept and ignored. `CLAUDE.md`'s
no-backwards-compat-cruft rule and the save rule agree here: a retained dead
field costs the same bump on the next property and leaves a lie in the
struct.

`PlayerSave::hunger` is renamed to `power` in the same bump. Renaming a
field under RON is a breaking read, and the bump is already being spent.

`CreatureSave` gains `power: f32` behind `#[serde(default = …)]` returning
`POWER_MAX`, so companions in an existing save load charged rather than
empty. That half is free and would not have earned a bump on its own.

## Testing

The failing reproducer comes first in each case.

**The regen hole.** A player underground, with a Recharger inside its radius
of the entrance tile, does not gain Power on a tick. This test must fail
before the `Res<Locale>` guard exists — it is the one bug fix in the change
and the one most likely to be written as a vacuous test, since the whole
system is a no-op when no Recharger is in range. Stand the Recharger up
explicitly and assert the *surface* case still regenerates in the same test
file, or the guard could be a `return` at the top and still pass.

**The gate.** A companion with an empty reserve has its Special greyed with
a reason, and `battle_set_action` refuses the same plan. Both halves, since
the seam's whole purpose is that they cannot disagree.

**The caster pays.** A companion casting draws down the companion's reserve
and leaves the player's untouched. This is the assertion that "every
companion tracks their power level" actually shipped.

**No passive drain on companions.** Tick a companion through many ticks and
assert its reserve has not moved. Guards the `With<Player>` decision, which
is otherwise invisible.

**No starvation by proxy.** A companion at zero Power over many ticks loses
no HP. Permadeath makes an accidental attrition kill a real bug.

**A fused companion can cast.** Fuse two programs and assert the result
holds a full reserve and its Special is not greyed. This is the one door
with no compiler barrier behind it, and the bug it guards is silent.

**Save round trip.** A companion's reserve survives; a pre-bump save loads
with companions at full.

**`spend` and `restore` clamp.** Directly on `PowerReserve`, since the type
now owns the invariant.

Delete-the-fix check: each of the above must fail with its fix reverted.
`CLAUDE.md` records two tests written on 2026-08-09 that passed with their
fix removed and read as coverage.

### What is not gated

`balance_sim` models no abilities at all, so **none of the 66 inherited
costs is covered by the balance regression suite**, and neither is
`ROUTINE_POWER_COST_MULTIPLIER` nor `trickle_charge`'s retune. The
`balance_sim` curve tests will pass against a game whose entire casting
economy has changed.

That is the accepted trade rather than an oversight. Tuning happens in play,
which is what the multiplier and the per-file `.ron` values are for; the
suite's job here is to prove the *mechanism* works — costs are charged, the
right entity pays, an empty reserve refuses — not that any number is right.

The instruments that can see the numbers are `dev-arenas/` and a session.
Re-run the shipped scenarios once the flip lands, and read
`docs/measurements/README.md` before running anything broader.

One test does belong to the numbers: an assertion over the real assets that
every `power_cost` is finite and non-negative. `AbilityDef::validate`
already refuses a non-finite `fatigue_cost`, and that check must survive the
rename rather than being lost with it.

## Out of scope

- **The base power grid** — the `TODO.md` item reading "base needs power,
  structures consume power, and power rechargers produce power". Same
  resource, and it will want the Recharger's role reconsidered, which this
  change deliberately leaves alone. Sequencing it after means the Recharger
  is settled once rather than twice.
- **In-battle targeting of consumables**, as above.
- **Hostile reserves**, as above.
- **Scaling a reserve's maximum with level.** Everything caps at
  `POWER_MAX`. A level-1 and a level-20 companion hold the same pool, which
  is a legitimate tuning lever to reach for later and not a gap to close
  now.
