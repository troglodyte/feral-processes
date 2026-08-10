"""What a trained enemy policy actually does, from a sweep's telemetry.

    python policy_report.py --log-dir dev-logs/policy-sweep

Answers the questions the 2026-08-09 training report had to assert in prose
or pin with a single unit test: who the enemy swings at, how hurt they are
when it picks them, and how much of its moveset it still uses. Each is a
distribution over real swings rather than a summary statistic over fights.

**It cannot answer the Defend question**, and says so rather than leaving a
reader to assume otherwise — `arena::run_rep` plays the party as All-Attack,
so nobody in any of these fights ever braced. See `check_bracing`.
"""

from __future__ import annotations

import argparse
from pathlib import Path

import matplotlib

# Chosen before pyplot is imported: this runs headless and the default
# interactive backend fails outright without a display.
matplotlib.use("Agg")

import matplotlib.pyplot as plt  # noqa: E402
import pandas as pd  # noqa: E402

import battles  # noqa: E402

# Baseline first everywhere, so a table and a chart read in the same
# direction as the sentence "baseline -> trained".
PASS_ORDER = ["baseline", "trained"]


def check_bracing(swings: pd.DataFrame) -> str:
    """Whether Defend is observable at all in this data. Usually it is not.

    Reported rather than assumed: if the arena ever grows a party plan that
    braces, this line starts showing a non-zero count and the focus-fire
    numbers below become answerable for bracing targets too.
    """
    braced = int(swings["target_bracing"].sum())
    if braced == 0:
        return (
            "target_bracing is False on all "
            f"{len(swings):,} swings — the harness plays All-Attack, so "
            "Defend is not exercised and nothing here bears on it."
        )
    return f"{braced:,} of {len(swings):,} swings landed on a bracing target."


def targeting(swings: pd.DataFrame) -> pd.DataFrame:
    """Share of swings that went at the player rather than a companion.

    Only over scenarios that field a companion: in a solo fight the player
    is the only legal target and a 100% share there is arithmetic, not
    behaviour, and would drag every average toward 1.0.
    """
    with_party = swings[swings["party_size"] > 1]
    return (
        with_party.groupby(["config", "pass"], observed=True)["at_player"]
        .agg(["mean", "count"])
        .rename(columns={"mean": "player_share", "count": "swings"})
        .reset_index()
    )


def focus_fire(swings: pd.DataFrame) -> pd.DataFrame:
    """How much HP the chosen target had left, as a fraction of its max.

    This is `target_hp_frac`'s weight made visible. A policy that finishes
    the wounded picks targets low on this scale; the uniform baseline picks
    them wherever they happen to be.
    """
    return (
        swings.groupby(["config", "pass"], observed=True)["target_hp_frac"]
        .agg(["mean", "median", "count"])
        .reset_index()
    )


def move_concentration(swings: pd.DataFrame) -> pd.DataFrame:
    """What share of a species' swings went to its single most-used move.

    A stand-in for "does the enemy still vary its moves", and deliberately
    not a join against the species files: which move carries an effect is in
    `assets/species/*.ron`, and a RON reader here would be a second parser to
    keep in step with the engine's — the same call `docs/roster-gen.py` made.
    1.0 means the species used exactly one move all sweep.
    """
    per_move = (
        swings.groupby(["config", "pass", "actor", "move"], observed=True)
        .size()
        .rename("n")
        .reset_index()
    )
    totals = per_move.groupby(["config", "pass", "actor"], observed=True)["n"].transform("sum")
    per_move["share"] = per_move["n"] / totals
    top = (
        per_move.groupby(["config", "pass", "actor"], observed=True)["share"]
        .max()
        .rename("top_move_share")
        .reset_index()
    )
    # Averaged over species rather than over swings: otherwise whichever
    # program happens to be most numerous decides the number.
    return (
        top.groupby(["config", "pass"], observed=True)["top_move_share"]
        .agg(["mean", "count"])
        .rename(columns={"count": "species"})
        .reset_index()
    )


def outcomes(fights: pd.DataFrame) -> pd.DataFrame:
    """Per scenario, what the fights cost — the report's own table.

    `won` is the *player's* result as the engine records it (read off the
    opponents being gone, never off the player's HP), so the enemy win rate
    is its complement.
    """
    grouped = (
        fights.groupby(["config", "pass", "scenario"], observed=True)
        .agg(
            enemy_win=("won", lambda s: 1.0 - s.mean()),
            player_hp=("player_hp_frac", "mean"),
            downed=("companions_downed", "mean"),
            rounds=("rounds", "mean"),
            fights=("won", "size"),
        )
        .reset_index()
    )
    return grouped


def _bar_by_config(frame: pd.DataFrame, value: str, title: str, ylabel: str, path: Path) -> None:
    """One grouped bar chart: configs on x, a bar per pass."""
    pivot = frame.pivot(index="config", columns="pass", values=value)
    pivot = pivot[[p for p in PASS_ORDER if p in pivot.columns]]
    axis = pivot.plot(kind="bar", figsize=(8, 4.5), rot=0)
    axis.set_title(title)
    axis.set_ylabel(ylabel)
    axis.set_xlabel("")
    axis.legend(title="")
    axis.figure.tight_layout()
    axis.figure.savefig(path, dpi=140)
    plt.close(axis.figure)


def _hp_histogram(swings: pd.DataFrame, path: Path) -> None:
    """The focus-fire distribution, which a mean flattens into nothing.

    Two policies can share a mean target HP fraction while one spreads its
    swings evenly and the other splits them between finishing blows and
    opening ones.
    """
    configs = sorted(swings["config"].unique())
    fig, axes = plt.subplots(1, len(configs), figsize=(4.5 * len(configs), 4), sharey=True)
    if len(configs) == 1:
        axes = [axes]
    for axis, config in zip(axes, configs):
        subset = swings[swings["config"] == config]
        for pass_name in PASS_ORDER:
            values = subset[subset["pass"] == pass_name]["target_hp_frac"]
            if values.empty:
                continue
            axis.hist(values, bins=20, range=(0, 1), alpha=0.55, label=pass_name, density=True)
        axis.set_title(config)
        axis.set_xlabel("target HP fraction when chosen")
        axis.legend()
    axes[0].set_ylabel("density")
    fig.tight_layout()
    fig.savefig(path, dpi=140)
    plt.close(fig)


def _table(frame: pd.DataFrame, floats: int = 3) -> str:
    return frame.to_string(index=False, float_format=lambda v: f"{v:.{floats}f}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--log-dir", default="dev-logs/policy-sweep", type=Path)
    parser.add_argument("--out-dir", default="analysis/out", type=Path)
    args = parser.parse_args()
    args.out_dir.mkdir(parents=True, exist_ok=True)

    swings = battles.swings(args.log_dir)
    fights = battles.fights(args.log_dir)

    # `targeting` drops solo scenarios, so it needs to know which fights had
    # a party — from `fight_start`, never from the swings themselves. See
    # `battles.party_sizes` for why that distinction is load-bearing.
    swings = swings.merge(
        battles.party_sizes(args.log_dir), on=battles.FIGHT_KEY, how="left"
    )
    missing = int(swings["party_size"].isna().sum())
    if missing:
        raise ValueError(f"{missing:,} swings matched no fight_start — files are inconsistent")

    print("=" * 72)
    print("SWEEP:", args.log_dir)
    print(
        f"{len(swings):,} enemy swings, {len(fights):,} fights, "
        f"configs: {', '.join(sorted(swings['config'].unique()))}"
    )
    print("=" * 72)

    print("\n-- Defend ------------------------------------------------------")
    print(check_bracing(swings))

    # Before any table, not after: a confounded pair means the numbers below
    # do not mean what their column headings say, and a reader who has
    # already read them will not go back.
    print("\n-- Confounds ---------------------------------------------------")
    # Per run, never pooled: most configs in a sweep use the default plan
    # and never brace, and those constant rows hide a perfect correlation
    # inside the one config that does.
    found = battles.confounds_by(swings, ["config", "pass"])
    if not found:
        print("no run has a collinear observable pair above r =", battles.CONFOUND_THRESHOLD)
    for (config, pass_name), pairs in sorted(found.items()):
        for a, b, r in pairs:
            print(
                f"WARNING  {config}/{pass_name}: {a} and {b} move together "
                f"(r = {r:+.2f}).\n"
                f"         Any statement about one is also a statement about the\n"
                f"         other; this run cannot separate them. Change the rule\n"
                f"         so they vary independently before concluding anything."
            )

    print("\n-- Who gets hit (scenarios with a party) ------------------------")
    target = targeting(swings)
    print(_table(target))

    print("\n-- Focus fire: target HP fraction when chosen -------------------")
    print(_table(focus_fire(swings)))

    print("\n-- Move variety: share taken by each species' top move ----------")
    print(_table(move_concentration(swings)))

    print("\n-- Outcomes per scenario ---------------------------------------")
    print(_table(outcomes(fights)))

    _bar_by_config(
        target,
        "player_share",
        "Share of enemy swings aimed at the player",
        "fraction of swings",
        args.out_dir / "targeting.png",
    )
    _bar_by_config(
        focus_fire(swings).rename(columns={"mean": "value"}),
        "value",
        "Mean target HP fraction when chosen (lower = finishing the wounded)",
        "HP fraction",
        args.out_dir / "focus_fire.png",
    )
    _hp_histogram(swings, args.out_dir / "focus_fire_hist.png")
    print(f"\nwrote charts to {args.out_dir}/")


if __name__ == "__main__":
    main()
