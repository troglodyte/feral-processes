# 2026-08-19 — What removing the passive party bonus cost the player

## The claim

Companions used to lend the player a passive tenth of their own ATK and DEF
(`Game::party_stat_bonus`, `PARTY_PASSIVE_STAT_DIVISOR = 10`, floored at 1
per member) on top of taking a turn of their own. Removing it is a **small
nerf, not a re-tune**: across four party-bearing arena scenarios at 200 reps
each, every fight got slightly longer (+0.3 to +0.4 rounds) and the player
finished with 1-3 points less Integrity. Three of the four were already
100% walkovers and stayed 100%. The one scenario that is not a walkover,
`policy-full-kit`, moved 25.0% -> 21.0%, which is **within sampling noise**
at this rep count and should not be quoted as a 4-point drop.

The more important finding is structural: **`balance_sim` never modelled
this term at all**, despite a doc comment on
`a_full_party_survives_a_full_group_at_each_zone` claiming it as one of
three ways party size compounds. Removing the feature moved **no curve in
the whole balance suite**. That doc has been corrected in place.

## How to reproduce it

The shipped scenarios ship at 20-50 reps, which is too noisy for a delta this
size (see **What it does not say**). Copy them out and raise `reps` rather
than editing tracked assets:

```sh
for f in full-group geared-vs-boss policy-full-kit deep-lair; do
  sed 's/^\( *\)reps: [0-9]*,/\1reps: 200,/' dev-arenas/$f.ron > /tmp/$f.ron
  cargo run -q --bin arena -- /tmp/$f.ron
done
```

Run once before the change and once after, in that order, in the same
working tree. `cargo test -p feral-processes-engine balance_sim` is the
other half of the finding and takes no setup: it passes unchanged either
way.

## The numbers

200 reps per scenario per build; 1,600 fights total.

| scenario | win rate before | after | rounds before | after | player HP before | after |
|---|---|---|---|---|---|---|
| `full-group` | 100.0% (200/200) | 100.0% (200/200) | 7.6 | 7.9 | 98% | 97% |
| `geared-vs-boss` | 100.0% (200/200) | 100.0% (200/200) | 3.8 | 4.1 | 99% | 99% |
| `policy-full-kit` | 25.0% (50/200) | 21.0% (42/200) | 14.5 | 14.4 | 21% | 18% |
| `deep-lair` | 100.0% (200/200) | 100.0% (200/200) | 6.9 | 7.3 | 99% | 98% |

New, not a replication: nothing had measured this term's worth before. The
three walkovers reproduce the standing finding from
[the combat model's first slice](2026-08-19-combat-model-slice-1.md) that
most shipped scenarios gate nothing about difficulty — they are at the
ceiling and can only report the nerf as rounds and Integrity, not as wins.

The size of the term itself, from
`a_roster_no_longer_inflates_the_players_own_attack_or_defense`: a player
whose own stats are `(6 ATK, 2 mitigation)` read `(106, 22)` with five
deliberately beefy companions posted. That is the fixture's worst case, not
a shipped one, but it is the shape of what was removed — the bonus scaled
with the roster's stats without bound below the mitigation cap.

## What it does not say

- **The win-rate delta is not significant.** At p ~= 0.25 and n = 200 the
  standard error is about 3.1 points, so 25.0% -> 21.0% is roughly 1.3
  standard errors. The direction is corroborated by all four scenarios
  moving the same way on rounds and Integrity, which is why the claim above
  is "small nerf" and not "4 points". Settling the magnitude needs ~1,000
  reps on `policy-full-kit` against a matched baseline build.
- **Arena numbers compare within one build only.** Removing an ATK term
  changes how many attacks a fight takes, which reshuffles the RNG stream —
  so these are two different sequences of 200 fights, not the same 200
  refought. Aggregates over 200 distinct seeds are the comparable quantity;
  no individual seed's outcome carries across.
- **Nothing here measures feel.** Whether a roster reads as *worth having*
  once it only acts, rather than also inflating the player's own sheet, is
  the actual open question and no instrument in this repo can see it. This
  was open question 4 in
  `docs/superpowers/plans/2026-08-19-combat-model-ac-and-weapon-damage.md`.
- **No Stack term.** `deep-lair` is the only underground scenario here and
  it is a walkover; the depth-4 and depth-5 fights that
  [the depth curve](2026-08-19-stack-depth-curve.md) found marginal were not
  re-run, and those are where losing a defensive term is most likely to
  matter.
- **The player's displayed Attack and Mitigation drop.** `player_status()`
  and the manifest both read `effective_atk`/`effective_mitigation`, so a
  player with a full roster sees their own numbers fall on the sidebar the
  moment this lands. That is the intended consequence, not a regression, but
  it is the visible half and no test asserts what it looks like.

## Open questions

- Does `policy-full-kit` actually drop, and by how much? 1,000 reps against
  a matched baseline would settle it.
- Should anything compensate? Nothing was changed to offset the removal. The
  four walkover scenarios say there is room at the top of the curve; the one
  marginal scenario says the room is not evenly distributed.
- `stack-depth-5` was already 0% before this. It was not re-run, and this
  change can only have made it worse.
