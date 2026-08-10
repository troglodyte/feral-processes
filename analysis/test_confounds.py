"""The collinearity guard.

Written after an experiment that could not answer its own question: the
arena's brace rule fired at under 50% HP, so `target_bracing` and
`target_hp_frac` moved together perfectly and no reading could separate
"the enemy responds to Defend" from "the enemy finishes the wounded".

The report now refuses to present those numbers without saying so.
"""

import pandas as pd

from battles import confounds


def _swings(bracing, hp_frac):
    return pd.DataFrame(
        {
            "target_bracing": bracing,
            "target_hp_frac": hp_frac,
            "at_player": [False] * len(bracing),
        }
    )


def test_a_treatment_that_tracks_a_feature_is_flagged():
    # Braces exactly when hurt — the arena's own 2026-08-10 rule.
    braced = [True, True, True, False, False, False]
    hp = [0.1, 0.2, 0.3, 0.8, 0.9, 1.0]
    found = confounds(_swings(braced, hp))
    assert ("target_bracing", "target_hp_frac") in {
        (a, b) for a, b, _ in found
    }, f"the collinear pair was not reported: {found}"


def test_an_independent_treatment_is_not_flagged():
    # Bracing varies on its own, uncorrelated with health.
    braced = [True, False, True, False, True, False]
    hp = [0.1, 0.2, 0.9, 0.3, 0.8, 0.4]
    assert confounds(_swings(braced, hp)) == []


def test_pooling_runs_that_never_braced_hides_the_confound():
    """The bug the first wiring shipped with.

    A sweep holds several configs and most of them use the default
    All-Attack plan, where nobody braces. Pooled, the constant rows swamp
    the collinear ones and the guard reports all clear — on the very dataset
    it exists to flag. It has to look per run.
    """
    collinear = _swings([True, True, False, False], [0.1, 0.2, 0.9, 1.0])
    collinear["config"] = "brace"
    never = _swings([False] * 8, [0.1, 0.3, 0.4, 0.6, 0.7, 0.8, 0.9, 1.0])
    never["config"] = "all-attack"
    pooled = pd.concat([collinear, never], ignore_index=True)

    assert confounds(pooled) == [], "the pooled frame is expected to look clean"

    from battles import confounds_by

    per_run = confounds_by(pooled, ["config"])
    assert ("brace",) in per_run, f"the bracing run was not flagged: {per_run}"
    assert ("all-attack",) not in per_run


def test_a_column_that_never_varies_is_not_reported():
    # The All-Attack case: nobody braces, so the correlation is undefined.
    # Undefined must not read as "no confound found" *or* as a false alarm —
    # a constant column is simply not evidence either way.
    braced = [False] * 6
    hp = [0.1, 0.4, 0.5, 0.8, 0.9, 1.0]
    assert confounds(_swings(braced, hp)) == []
