# Regenerates docs/perks.md. Run from the repo root: python3 docs/perks-gen.py
#
# Transcribed by hand rather than parsed, for the same reason
# docs/roster-gen.py is. This page is the one that has to reach across the
# moddable seam: the name and cost columns come from assets/perks/*.ron, and
# the magnitude column comes from crates/engine/src/tuning.rs, which is
# deliberately not data. Update both halves when either moves, then rerun.
AFFINITY_MAX = 2.0
AFF_SCALED, AFF_UNSCALED = 0.05, 0.15
POINTS_PER_LEVEL = 1

P = [
 # variant           name                cost per-level effect                       tuning constant                              where it hooks
 ("KeenScavenger",  "Keen Scavenger",    2, "+1pp mining success",                   "KEEN_SCAVENGER_BONUS_PER_LEVEL = 0.01",     "systems::mining_success_chance"),
 ("LowPowerMode",   "Low Power Mode",    2, "-1pp Power drain, floor 0",             "LOW_POWER_MODE_REDUCTION_PER_LEVEL = 0.01", "the hunger-decay multiplier"),
 ("ExploitFocus",   "Exploit Focus",     3, "-3pp of the target's HP penalty",       "EXPLOIT_FOCUS_HP_PENALTY_REDUCTION_PER_LEVEL = 0.03", "taming::capture_chance"),
 ("LeanCompiler",   "Lean Compiler",     3, "-1 of each ingredient, floor 1",        "LEAN_COMPILER_DISCOUNT_PER_LEVEL = 1",      "Game::craft_recipes' costs"),
 ("Attacker",       "Attacker",          2, "+1 ATK, permanent",                     "ATTACKER_BONUS_PER_LEVEL = 1",              "a direct Stats write at purchase"),
 ("Defender",       "Defender",          2, "+1 DEF, permanent",                     "DEFENDER_BONUS_PER_LEVEL = 1",              "a direct Stats write at purchase"),
 ("Buffer",         "Buffer",            3, "+1% max Integrity, at least +10",       "BUFFER_BONUS_PERCENT_PER_LEVEL = 0.01",     "a direct Stats write, plus a full heal"),
 ("DamageAffinity", "Payload Tuning",    2, "+15% Damage magnitude",                 "AFFINITY_PERK_BONUS_PER_LEVEL_UNSCALED",    "the player's own casts only"),
 ("HealAffinity",   "Field Medic",       2, "+5% Heal magnitude",                    "AFFINITY_PERK_BONUS_PER_LEVEL",             "the player's own casts only"),
 ("BuffAffinity",   "Overclocker",       2, "+5% Buff magnitude",                    "AFFINITY_PERK_BONUS_PER_LEVEL",             "the player's own casts only"),
 ("DebuffAffinity", "Corruption Vector", 2, "+5% Debuff magnitude",                  "AFFINITY_PERK_BONUS_PER_LEVEL",             "the player's own casts only"),
 ("DrainAffinity",  "Siphon Protocol",   2, "+15% Drain damage",                     "AFFINITY_PERK_BONUS_PER_LEVEL_UNSCALED",    "the player's own casts only"),
]
K = "variant name cost effect const hook".split()
R = [dict(zip(K, r)) for r in P]

# Which of the two affinity rates each of the five uses, keyed off the
# constant name so the table above stays the single place either is stated.
AFF_RATE = {r["variant"]: (AFF_UNSCALED if "UNSCALED" in r["const"] else AFF_SCALED)
            for r in R if r["variant"].endswith("Affinity")}

one_of_each = sum(r["cost"] for r in R)


def table(header, rows, align):
    sep = "|" + "|".join(("---:" if a == "r" else ":---") for a in align) + "|"
    body = "\n".join("| " + " | ".join(str(c) for c in r) + " |" for r in rows)
    return "| " + " | ".join(header) + " |\n" + sep + "\n" + body


def price_ladder():
    out = ["PERK POINT PRICE", ""]
    for cost in sorted({r["cost"] for r in R}, reverse=True):
        members = [r["name"] for r in R if r["cost"] == cost]
        out.append(f"{cost}  {', '.join(members)}")
    out.append("")
    out.append(f"one level of all {len(R)}: {one_of_each} points")
    return "```\n" + "\n".join(out) + "\n```"


def affinity_ramp(width=40):
    """Both rates walking from 1.00 toward the AFFINITY_MAX clamp."""
    out = [f"AFFINITY BY PERK LEVEL                 (clamped at {AFFINITY_MAX:.2f})", ""]
    for variant, rate in sorted(AFF_RATE.items(), key=lambda kv: -kv[1]):
        name = next(r["name"] for r in R if r["variant"] == variant)
        cap_level = next(n for n in range(1, 100) if 1.0 + rate * n >= AFFINITY_MAX)
        cost = next(r["cost"] for r in R if r["variant"] == variant)
        filled = round(cap_level / 20 * width)
        bar = "#" * filled + "." * (width - filled)
        rate_col = f"+{rate:.0%}/lvl"
        out.append(
            f'{name:<18} {rate_col:<9} {bar}  {cap_level:>2} lvl / {cap_level * cost:>2} pts'
        )
    return "```\n" + "\n".join(out) + "\n```"


doc = f"""# Perk catalogue

Every perk a player can buy, charted from `assets/perks/` and
`crates/engine/src/tuning.rs`. Twelve of them, and there will be twelve until
someone writes Rust.

**These numbers are a transcription, not a read.** They were copied out on
2026-08-05 and will drift the moment either source is edited; regenerate the
page rather than trusting it blind.

This is the one page in `docs/` that has to reach across the moddable seam,
because a perk is deliberately split in half. What it is **called**, how it
**reads** and what it **costs** live in `assets/perks/*.ron` and can be edited
without a compiler. How much it **gives per level** lives in `tuning.rs` and
cannot. That split is the game's standing rule — content is moddable, how hard
the game is, is not — and it means a `.ron` edit can make a perk cheaper or
dearer but never stronger.

| | |
|---|---|
| perks | {len(R)} |
| prices | {" and ".join(str(c) for c in sorted({r["cost"] for r in R}))} Perk Points |
| one level of everything | {one_of_each} points |
| points earned | {POINTS_PER_LEVEL} per player level, plus up to 5 from the [achievement ladder](achievements.md) |
| affinity perks | {len(AFF_RATE)} of {len(R)}, sharing two rates |

## The catalogue

Listed in `Perk::all()` order, which is the order the picker shows and — for
the enum behind it — the save format. Saves are bincode, which encodes an enum
positionally, so this order is load-bearing: append, never reorder.

{table(["", "Perk", "Cost", "Per level", "Hooks into"],
       [[f'`{r["variant"]}`', r["name"], r["cost"], r["effect"], r["hook"]] for r in R],
       ["l", "l", "r", "l", "l"])}

## Price

{price_ladder()}

Only three perks cost 3, and what they have in common is that they change a
*rate* rather than a number: Buffer scales with the Integrity you already
have, Lean Compiler pays out on every craft for the rest of the run, and
Exploit Focus is worth more the healthier the program you are trying to take.
The nine at 2 are flat.

Note what {one_of_each} points means against how they arrive. A Perk Point is
{POINTS_PER_LEVEL} per player level and at most 5 more from a fully cleared
profile, so buying one level of all twelve is most of the first thirty levels
of a run. Perks are not a shopping list to complete; they are a shape to
commit to.

## Where the magnitudes live

{table(["Perk", "Constant in `tuning.rs`"],
       [[r["name"], f'`{r["const"]}`'] for r in R],
       ["l", "l"])}

Every one of those is a hook into a different formula — a mining roll, a
hunger multiplier, a capture chance's HP term, a recipe cost, a direct `Stats`
write. There is no shared shape between them, which is exactly why `PerkDef`
has no `effect:` field and why a thirteenth perk is a new `Perk` variant plus
a hook wherever its effect belongs, rather than a new file.

## The five affinity perks

These are the one place the twelve *do* share a shape: each multiplies one
`AffinityKind` category — Damage, Heal, Buff, Debuff, Drain — for the player's
own ability casts. Never a companion's: a companion's affinity is its species'
business, and a party-wide perk would multiply against it.

They cost the same and they do not pay the same.

{affinity_ramp()}

Payload Tuning and Siphon Protocol run at three times the rate of the other
three, which looks like a mistake and is not. Damage and Drain skip the
level-scaling every other category gets from `abilities::ability_power_scale`,
so at the shared rate they would be a strictly worse purchase than plain
`Attacker` — the higher rate is what buys them back to parity, and the cost of
reaching the clamp sooner.

The clamp is `AFFINITY_MAX`, the same ceiling a species file is held to at
load. Perk *levels* are uncapped, so without it a long enough run would let
the player's own casts exceed the bound every affinity in the game is bound
by. `AffinityKind::perk_bonus_per_level` is what picks the right rate, so a
perk's category decides it rather than a match repeated at five call sites.

---

Source of truth is `assets/perks/` for names and costs, and
`crates/engine/src/tuning.rs` for magnitudes. Unlike species, structures,
items and abilities, **you cannot add a perk with a file** — the catalogue
keys off the `Perk` enum, so a `.ron` naming anything else fails to parse and
is skipped. To update this page, edit the table at the top of
[`docs/perks-gen.py`](perks-gen.py) and run `python3 docs/perks-gen.py` from
the repo root. The schema is documented in
[`assets/perks/README.md`](../assets/perks/README.md).
"""

import pathlib
pathlib.Path("docs/perks.md").write_text(doc)
print(doc)
