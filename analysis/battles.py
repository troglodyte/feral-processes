"""Loading `train --log-dir` output into DataFrames.

A sweep writes one JSONL file per (config, pass, scenario) — see
`log_path` in `crates/launcher/src/bin/train.rs` for why it is split that
finely. The three labels live in the filename rather than in the records,
so reading a sweep means parsing names and then concatenating.

The schema of a line is `dev-logs/README.md`, which is the reference; this
module only knows the two record types the policy questions need.
"""

from __future__ import annotations

import json
from pathlib import Path

import pandas as pd

# The two `Pass` variants the trainer writes. Matching against these rather
# than splitting on "-" is the whole trick: both the label and the scenario
# may contain hyphens ("03-midgame-group"), so a positional split
# mis-attributes every row without ever failing.
PASSES = ("baseline", "trained")


def parse_name(stem: str) -> tuple[str, str, str]:
    """`"pin3-trained-03-midgame-group"` -> `("pin3", "trained", "03-midgame-group")`."""
    for pass_name in PASSES:
        token = f"-{pass_name}-"
        at = stem.find(token)
        if at != -1:
            return stem[:at], pass_name, stem[at + len(token) :]
    raise ValueError(f"{stem!r} names no pass; expected one of {PASSES}")


def _read(path: Path, keep: set[str]) -> list[dict]:
    """Every record in `path` whose `t` is in `keep`, tagged with its file.

    Filtering during the read rather than after: a sweep is a few hundred
    megabytes and the two types worth analysing are a small slice of it, so
    building a frame of everything first is the difference between fitting
    in memory and not.
    """
    label, pass_name, scenario = parse_name(path.stem)
    rows = []
    with path.open() as handle:
        for line in handle:
            record = json.loads(line)
            if record["t"] not in keep:
                continue
            record["config"] = label
            record["pass"] = pass_name
            record["scenario"] = scenario
            rows.append(record)
    return rows


def load(log_dir: str | Path, kinds: set[str]) -> pd.DataFrame:
    """Every record of the given `t` values across a sweep directory."""
    log_dir = Path(log_dir)
    paths = sorted(log_dir.glob("*.jsonl"))
    if not paths:
        raise FileNotFoundError(f"no .jsonl under {log_dir}")
    rows = [row for path in paths for row in _read(path, kinds)]
    if not rows:
        raise ValueError(f"no records of {kinds} in {len(paths)} files under {log_dir}")
    return pd.DataFrame(rows)


def swings(log_dir: str | Path) -> pd.DataFrame:
    """One row per enemy swing, with the target's HP fraction worked out.

    `target_hp_before` is the field the whole schema was designed around:
    four attackers act inside one round, so a per-round snapshot cannot show
    focus fire and only a per-swing reading can.
    """
    frame = load(log_dir, {"enemy_choice"})
    frame["target_hp_frac"] = frame["target_hp_before"] / frame["target_max_hp"]
    frame["at_player"] = frame["target_slot"] == 0
    return frame


def fights(log_dir: str | Path) -> pd.DataFrame:
    """One row per fight: how it ended."""
    return load(log_dir, {"fight_end"})


# Above this absolute Pearson r, two per-swing observables are treated as
# telling the same story. 0.7 rather than something stricter because the
# failure this guards is not subtle: the case that prompted it was r = 1.0,
# a treatment defined as a threshold on the very feature it was confounded
# with. A borderline pair is worth a look; a perfect one is worth a stop.
CONFOUND_THRESHOLD = 0.7

# The per-swing observables worth checking against each other. Deliberately
# short: these are the columns a conclusion actually gets phrased in terms
# of, and a wider net produces pairs nobody was going to claim anything
# about.
OBSERVABLES = ["target_bracing", "target_hp_frac", "at_player"]


def confounds(swings: pd.DataFrame, threshold: float = CONFOUND_THRESHOLD) -> list:
    """Pairs of observables too correlated to be discussed separately.

    Returns `[(a, b, r), ...]`, alphabetical within a pair.

    **Why this exists.** The 2026-08-10 bracing run made a member Defend
    exactly when it fell under half HP, so `target_bracing` was a pure
    function of `target_hp_frac` — and since the policy's largest weight is
    on the latter, every statement about bracing was also a statement about
    being wounded, with no way to tell which was doing the work. The numbers
    looked clean and answered nothing. A reader cannot spot that by looking
    at a mean, so the report says it rather than leaving it to be noticed.

    A column that never varies is skipped: its correlation is undefined, and
    undefined is not the same as "checked and fine". Under the default
    `PartyPlan::AllAttack` nobody braces at all, which is exactly that case.
    """
    present = [c for c in OBSERVABLES if c in swings.columns]
    numeric = swings[present].astype(float)
    varying = [c for c in present if numeric[c].nunique() > 1]
    found = []
    for i, a in enumerate(varying):
        for b in varying[i + 1 :]:
            r = numeric[a].corr(numeric[b])
            if pd.notna(r) and abs(r) >= threshold:
                found.append((a, b, float(r)))
    return found


def confounds_by(swings: pd.DataFrame, keys: list) -> dict:
    """`confounds` per run, keyed by the values of `keys`. Clean runs omitted.

    **Pooling is not safe here**, which is the trap the first version of
    this shipped with. A sweep holds several configs and most use the
    default All-Attack plan where nobody braces at all; those constant rows
    swamp the collinear ones and the pooled correlation comes out near zero.
    The guard then reports all clear on precisely the dataset it exists to
    flag. A confound is a property of one run's design, so it has to be
    measured inside one run.
    """
    out = {}
    for key, group in swings.groupby(keys, observed=True):
        if not isinstance(key, tuple):
            key = (key,)
        found = confounds(group)
        if found:
            out[key] = found
    return out


# The four columns that identify a fight. `fight` alone is not enough and
# neither is any three of these: `arena::run` numbers each call's fights from
# 1, and one call is one (config, pass, scenario).
FIGHT_KEY = ["config", "pass", "scenario", "fight"]


def party_sizes(log_dir: str | Path) -> pd.DataFrame:
    """How many slots each fight opened with, from `fight_start`.

    **Not derivable from the swings.** Taking the highest `target_slot` seen
    would call a fight solo whenever the enemy ignored the companions — which
    is precisely the behaviour run 1 of the training was rejected for, so
    that shortcut deletes the evidence it is meant to measure.
    """
    starts = load(log_dir, {"fight_start"})
    starts["party_size"] = starts["party"].apply(len)
    return starts[FIGHT_KEY + ["party_size"]]
