"""The filename parser, which is the one place this can be silently wrong.

Every other function here either raises or produces obviously-empty output
when it goes wrong. `parse_name` mis-attributing rows produces a full,
plausible report about the wrong configs.
"""

import pytest

from battles import parse_name


def test_a_scenario_name_with_hyphens_survives():
    assert parse_name("pin3-trained-03-midgame-group") == (
        "pin3",
        "trained",
        "03-midgame-group",
    )


def test_a_label_with_hyphens_survives():
    # The case that actually justifies searching for the pass token instead
    # of splitting positionally. `stem.split("-", 2)` gets the scenario above
    # right — it only breaks when the *label* carries a hyphen, and `--label`
    # takes whatever it is given. Verified by watching the positional version
    # fail this and only this.
    assert parse_name("pin-3-trained-03-midgame-group") == (
        "pin-3",
        "trained",
        "03-midgame-group",
    )


def test_the_two_passes_are_told_apart():
    assert parse_name("unpinned-baseline-01-opening-solo")[1] == "baseline"
    assert parse_name("unpinned-trained-01-opening-solo")[1] == "trained"


def test_a_name_naming_no_pass_is_refused():
    # Rather than guessed at: a file that does not follow the convention is
    # something else in the directory, and quietly folding it into the frame
    # is how a stale run from last week joins this week's numbers.
    with pytest.raises(ValueError):
        parse_name("battles")
