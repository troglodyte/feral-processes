# Sorties (mods)

Edit or add a `.ron` file in this directory and it's picked up automatically
the next time a game session starts — no recompiling required. A malformed
file is skipped with a warning logged in-game rather than crashing startup, so
a broken def costs the game that one site and nothing else.

**This directory may be deleted.** An empty catalogue is valid and inert: the
Relay's board offers nothing, no squad can be dispatched and nothing panics —
exactly the pre-sortie game. That is the same supported way to play that
deleting `assets/needs/` or `assets/memories/` is.

## What a site is

A **sortie** is a squad of idle base staff sent away from the base to fight a
run of battles off-screen and come home with what it could carry. A site says
where they are going: what it is called, how dangerous it is relative to the
sector you are in, and how many fights getting through it takes.

A site does **not** say how long the trip takes or what it pays. Duration is
derived from the risk offset and the battle count (`SORTIE_TRAVEL_BASE_TICKS`
and friends in `tuning.rs`), and the payout falls out of the fights actually
had.

Each file is one site:

```ron
(
    id: "cold_storage",
    name: "Cold Storage",
    description: "A decommissioned archive on the sector's edge.",
    risk: 0,
    battles_min: 5,
    battles_max: 7,
)
```

## Fields

| Field | Required | Meaning |
|---|---|---|
| `id` | yes | Unique. Two files claiming one id resolve by filename order, and the last one wins. |
| `name` | yes | What the board row leads with. |
| `description` | yes | One line of flavour under it, in the player's vocabulary. |
| `risk` | no, defaults `0` | Steps **above the sector baseline**. See below. |
| `battles_min` | yes | Fewest fights the board may offer this site at. At least 1. |
| `battles_max` | yes | Most fights, inclusive. Must not be below `battles_min`. |

### `risk` is an offset, never an absolute danger band

The opposition is drawn from the same habitat window an ordinary encounter
uses, at `danger_steps + risk`. So a `risk: 2` site stays two steps harder
than the sector you are in, whether that is sector 1 or sector 9 — it does not
become trivial as a run develops, and it does not become impossible either.

The trip's duration reads that offset too, and not the absolute band. A site
authored with a large `risk` is a long trip everywhere rather than a trip that
grows silently longer the deeper into a run you get.

## Two ways a file is refused at load

Both are skipped with a warning naming the file, and neither stops the game:

- `battles_min` is `0`. A site with no fights is a trip that pays nothing.
- `battles_max` is below `battles_min`. The board rolls a count inside the
  range, and an inverted range has nothing in it — refusing here reports the
  fault beside the file that caused it rather than a long way downstream.

## The board

Three sites are offered at a time, drawn from this catalogue and rotating on
their own every `SORTIE_BOARD_ROTATION_TICKS`. The board is **derived, never
stored**: it is recomputed from the world seed, the sector and the clock, so
reloading a save reproduces the same offers and there is nothing to re-roll.
Each offer's battle count is fixed the moment it appears, which is what lets
the Relay screen quote the trip's length before you sign for it.

A catalogue holding fewer than three sites simply offers what there is.
