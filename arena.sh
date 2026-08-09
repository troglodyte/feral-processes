#!/usr/bin/env bash
#
# arena.sh — Run a battle scenario offline, by short name instead of path.
#
# Usage:
#   ./arena.sh                        list the scenarios in dev-arenas/
#   ./arena.sh <name>                 run it
#   ./arena.sh <name> --reps N        override the file's rep count
#   ./arena.sh <name> --ab            run it twice: with and without the
#                                     trained enemy policy
#   ./arena.sh play                   launch the game's own arena screen
#
#   <name>  a scenario in dev-arenas/, with or without the .ron
#
# Release build, always: the arena is thousands of fights and a debug build
# runs them roughly an order of magnitude slower.
#
# `--ab` is the comparison this exists for. It moves
# assets/policies/enemy_battle.ron aside for the second run and puts it back
# afterwards, including on Ctrl-C or a crash — that restore is a trap rather
# than a trailing line, because a script that dies half way through would
# otherwise leave the game installed without its policy and nothing on
# screen to say so.

set -uo pipefail
cd "$(dirname "$0")"

ARENAS_DIR="dev-arenas"
POLICY="assets/policies/enemy_battle.ron"
STASHED="$POLICY.ab-stash"

usage() {
    sed -n '3,24p' "$0" | sed 's/^# \{0,1\}//'
}

list_scenarios() {
    shopt -s nullglob
    for f in "$ARENAS_DIR"/*.ron; do
        basename "$f" .ron
    done
    shopt -u nullglob
}

# Always put the policy back. Registered before anything moves it, so an
# interrupt between the move and the run cannot strand it.
restore_policy() {
    if [[ -f "$STASHED" ]]; then
        mv -f "$STASHED" "$POLICY"
    fi
}
trap restore_policy EXIT INT TERM

NAME="${1:-}"

if [[ -z "$NAME" || "$NAME" == "-h" || "$NAME" == "--help" ]]; then
    usage
    echo
    echo "scenarios in $ARENAS_DIR:"
    list_scenarios | sed 's/^/  /'
    exit 0
fi

if [[ "$NAME" == "play" ]]; then
    echo "launching the game — [R] Arena on the main menu"
    exec env FERAL_DEV_ARENA=1 cargo run --release
fi

shift
REPS=""
AB=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --reps)
            REPS="${2:-}"
            if [[ -z "$REPS" ]]; then
                echo "--reps needs a number" >&2
                exit 1
            fi
            shift 2
            ;;
        --ab) AB=1; shift ;;
        *) echo "unknown option: $1" >&2; exit 1 ;;
    esac
done

SCENARIO="$ARENAS_DIR/${NAME%.ron}.ron"
if [[ ! -f "$SCENARIO" ]]; then
    echo "no such scenario: $SCENARIO" >&2
    echo "try one of:" >&2
    list_scenarios | sed 's/^/  /' >&2
    exit 1
fi

echo "building arena..."
cargo build -q --release --bin arena || exit 1
ARENA="target/release/arena"

# A rep override rewrites the scenario into a temp file rather than editing
# the real one: these files are checked in, and the hand-play copies
# deliberately ship `reps: 1`.
RUN_FILE="$SCENARIO"
if [[ -n "$REPS" ]]; then
    RUN_FILE="$(mktemp -t "arena-$NAME-XXXX.ron")"
    sed "s/reps: *[0-9]\+,/reps: $REPS,/" "$SCENARIO" > "$RUN_FILE"
    trap 'rm -f "$RUN_FILE"; restore_policy' EXIT INT TERM
fi

run_once() {
    "$ARENA" "$RUN_FILE"
}

if [[ "$AB" -eq 0 ]]; then
    run_once
    exit $?
fi

if [[ ! -f "$POLICY" ]]; then
    echo "no $POLICY installed — there is nothing to compare against" >&2
    exit 1
fi

echo "=== with the trained policy ==="
run_once
echo
echo "=== with the policy moved aside (baseline) ==="
mv "$POLICY" "$STASHED"
run_once
restore_policy
echo
echo "policy restored to $POLICY"
