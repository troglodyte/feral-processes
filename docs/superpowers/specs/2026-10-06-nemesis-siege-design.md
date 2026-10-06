# Nemesis siege — design

**Date:** 2026-10-06
**Branch:** `nemesis-siege`

## Intent

A nemesis left alone should not sit idle. Over time it gathers a few more
programs into a band and then leads a small siege on the player's base. The
player can see the band growing on the map and break it up early by fighting
it. A nemesis that is never dealt with keeps getting stronger.

What the user chose (2026-10-06):

- The band is made of wild programs that are visible on the map, not a
  hidden counter.
- The target is the player's base, using the existing siege machinery.
- The trigger is separate from the siege clock, so this is extra pressure.
- Aftermath: if the nemesis is downed, its grudge is over. If it survives,
  it goes home alone with grudge +1 and starts gathering again.

## Today

- `Nemesis(u32)` (`components.rs`) is one wild `Hostile` creature with a
  grudge count. It wanders like any hostile and does nothing over time
  (`game/combat_teardown.rs::mark_nemeses`).
- Sieges (`game/siege/`) spawn a fresh pack of one habitat species.
  `pack_size(zone)` gives the size. There is no leader.
- `cull_to_cap` (`game/spawning.rs`) can despawn a nemesis that is far
  from the player. That would end this feature before it starts, so it is
  fixed here.
- `gather_pack` (`game/combat.rs`) already pulls nearby hostiles into a
  map fight, so a band trailing its leader joins any fight against it.

## Design

### 1. Gathering

New components:

- `NemesisMuster { ticks: u32 }` on a nemesis: ticks since it last
  recruited, or since its band filled.
- `NemesisFollower(Entity)` on a recruit: points at its leader.

A muster system runs every world tick. For each nemesis:

- It skips the tick if the nemesis is in battle or `Pursuing`.
- Every `NEMESIS_RECRUIT_INTERVAL` ticks, while the band has fewer than
  `NEMESIS_BAND_MAX` followers, it recruits one body.
- **Who can be recruited:** the nearest wild `Hostile` within
  `NEMESIS_RECRUIT_RADIUS` that has none of these: `Nemesis`,
  `NemesisFollower`, `NestGuardian`, a boss, `Besieger`.
- **If no one qualifies:** one body of a habitat species at the nemesis's
  cell spawns beside it, at the same scaling `spawn_group` uses. Wild
  population is sparse away from the player, so without this fallback a
  distant nemesis would never gather.
- **Following:** a follower does not wander. It steps toward its leader
  whenever it is more than 2 cells away.
- **Band full:** when the band reaches `NEMESIS_BAND_MAX`, the alert
  "‹Name› has gathered a band." is posted and the march countdown starts.

Starting values, written in ticks at 2 ticks per second:

| Constant | Value | Meaning |
|---|---|---|
| `NEMESIS_RECRUIT_INTERVAL` | 600 | one recruit every 5 minutes |
| `NEMESIS_RECRUIT_RADIUS` | 12 | cells |
| `NEMESIS_BAND_MAX` | 3 | followers, so a siege of 4 against 6–12 for a regular one |
| `NEMESIS_MARCH_DELAY` | 960 | `SIEGE_WARN_FLOOR_TICKS`: 8 minutes from full band to march |

With these values a nemesis marches about 23 minutes after its last
encounter with the player.

**Leaving the band:** a follower stops following, and its tag is removed,
in three cases:

- its leader is despawned;
- its leader stops being a nemesis;
- it becomes a nemesis itself. `mark_nemeses` marks every hostile that
  survives a jack-out, followers included, and a newly marked follower
  becomes its own leader.

A recruit killed in a fight simply leaves the band smaller. The muster then
refills it on the normal interval.

**Cull exemption:** `cull_to_cap` and every other wild-population sweep
that despawns hostiles skip `Nemesis` and `NemesisFollower`. This is the
same treatment `clear_local_wild` already gives `Nemesis`. Followers also
survive a breach.

### 2. Trigger

- Once a nemesis's band has been full for `NEMESIS_MARCH_DELAY` ticks, it
  marches.
- It holds under the same conditions `siege_check` holds for: no base,
  nothing to besiege, or a fight already running. While it holds, its
  countdown stays full and it re-checks every tick.
- **One at a time:** if a regular siege or another nemesis siege is
  running, it holds. If a regular siege fires on the same tick, the
  regular siege goes first.
- The siege clock (`SiegePressure`) is untouched.
- `FERAL_DEV_NO_SIEGES` and `dev_set_sieges` freeze nemesis marches too.
  The flag means "no sieges", and a test that relies on it should not be
  surprised by one.

### 3. The siege

The attackers are the real band. Nothing is freshly spawned.

1. `NemesisHome(Position)` is inserted on the leader, recording its zone
   cell.
2. If the player is at the base, the band is seated by `open_siege`'s
   seating path. Every member is tagged `Besieger`, and
   `TacticalBattle::siege_pack` is the band size. The nemesis's taunt is
   logged as the siege opens, the same way it is when a map fight opens.
3. If the player is away, `resolve_siege_offscreen` is used with strength
   equal to the band size instead of `pack_size(zone)`.

**Refactor:** `open_siege` and `resolve_siege_offscreen` take the pack as a
parameter: the bodies, or a strength for off-screen. The regular siege
passes `spawn_siege_pack`'s bodies and `pack_size(zone)`. The nemesis siege
passes its band. Without this split, there would be a second copy of the
seating and shortfall code.

### 4. Aftermath

- **Nemesis downed in the siege:** it despawns with the other besiegers.
  The alert "‹Name› will trouble you no more." is posted. The grudge is
  over.
- **Pack breaks and the nemesis survives:** at home this means a morale
  break. Off-screen, the nemesis always survives.
  - Surviving followers despawn, as every `Besieger` does today.
  - The leader is exempted from the despawn that `end_battle` applies to
    besiegers. Its `Besieger` tag is removed and it moves back to its
    `NemesisHome` cell (then `NemesisHome` is removed).
  - Its grudge goes up by 1 through `mark_nemeses`'s escalation path:
    one rarity rung up, fully healed. That path becomes a shared function
    that both callers use, not a copy.
  - `NemesisMuster` resets to 0.

### 5. Saves

These new fields go on `CreatureSave`, all `#[serde(default)]`:

- `nemesis_muster_ticks: u32`
- `nemesis_home: Option<(i32, i32)>`
- `follows_nemesis: Option<…>`: the leader's index in the creature list,
  using whatever stable reference the save already uses for links between
  creatures. The plan names it after reading `save.rs`.

Old saves load with no bands. This is not a breaking change, so
`SAVE_FORMAT_VERSION` is unchanged. A siege in progress is already saved by
`SiegeSave`. `NemesisHome` comes back through the creature fields.

### 6. Tests (`crates/engine/tests/nemesis_siege.rs`)

**Gathering:**
- a nemesis recruits the nearest eligible wild body after
  `NEMESIS_RECRUIT_INTERVAL`;
- it refuses each excluded kind;
- it spawns a body when none is in range;
- it stops at `NEMESIS_BAND_MAX`;
- a follower closes in on a leader that has moved away.

**Cull:** `cull_to_cap` spares a nemesis and its followers.

**Trigger:**
- a full band marches after `NEMESIS_MARCH_DELAY`;
- it holds with no base, during a fight, and while a regular siege is
  running;
- `FERAL_DEV_NO_SIEGES` freezes it.

**Siege:**
- at home, the band is seated, `siege_pack` equals the band size, and the
  taunt is logged;
- off-screen, strength equals the band size.

**Aftermath:**
- a downed nemesis is gone;
- a nemesis that survives returns to its home cell alone with grudge +1
  and a reset muster.

**Saves:**
- a band survives a real save/load round trip, with followers linked to
  the right leader;
- `NemesisHome` survives a save in the middle of a siege.

**Map fight:** fighting a nemesis brings its trailing band into the fight
(through `gather_pack`).

**Mutation check:**
- the cull exemption;
- the leader's exemption from the besieger despawn.

**Dev save:** `dev-saves/nemesis.ron`, captured with
`savetool capture`, holds a nemesis and a partial band near a base in
sector 2 or higher.

## Out of scope

- A nemesis besieging a settlement.
- Bands of followers for ordinary hostiles.
- Bands larger than `NEMESIS_BAND_MAX`, or bands that scale with grudge.
  That is a tuning follow-up if sieges feel too small.

## Docs

- A `CHANGELOG.md` entry under unreleased.
- `.claude/rules/seams-sieges.md`: add a line saying `siege_pack` can come
  from a nemesis band, and that the leader survives the besieger despawn.
- Add the matching `seam:` reasoning to the graph and the `seams` skill.
