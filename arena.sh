#!/usr/bin/env bash
#
# arena.sh — Run a battle scenario offline, by short name or from a menu.
#
# Usage:
#   ./arena.sh                        pick a scenario and an action from a menu
#   ./arena.sh <name>                 run it
#   ./arena.sh <name> --reps N        override the file's rep count
#   ./arena.sh <name> --ab            run it twice: with and without the
#                                     trained enemy policy
#   ./arena.sh play                   launch the game's own arena screen
#   ./arena.sh --list                 print the scenario names, one per line
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
#
# The menu only appears on a terminal. Piped or redirected, a bare call
# prints the usage above instead of blocking forever on a read that will
# never be answered.

set -uo pipefail
cd "$(dirname "$0")"

ARENAS_DIR="dev-arenas"
POLICY="assets/policies/enemy_battle.ron"
STASHED="$POLICY.ab-stash"

usage() {
    sed -n '3,28p' "$0" | sed 's/^# \{0,1\}//'
}

scenario_names() {
    shopt -s nullglob
    for f in "$ARENAS_DIR"/*.ron; do
        basename "$f" .ron
    done
    shopt -u nullglob
}

# The first comment line of a scenario file, as a one-line descriptor for
# the menu. Every scenario in this directory opens with a comment saying
# what question it is for, so the menu can be built out of the files rather
# than out of a list here that would drift the first time one is added.
describe() {
    local line
    line=$(grep -m1 '^//' "$ARENAS_DIR/$1.ron" 2>/dev/null | sed 's|^// *||')
    if [[ ${#line} -gt 58 ]]; then
        line="${line:0:55}..."
    fi
    printf '%s' "$line"
}

# Always put the policy back. Registered before anything can move it, so an
# interrupt between the move and the run cannot strand it.
restore_policy() {
    if [[ -f "$STASHED" ]]; then
        mv -f "$STASHED" "$POLICY"
    fi
}
cleanup() {
    restore_policy
    if [[ -n "${TMP_SCENARIO:-}" ]]; then
        rm -f "$TMP_SCENARIO"
    fi
}
trap cleanup EXIT INT TERM

ARENA="target/release/arena"

build_arena() {
    echo "building arena..."
    cargo build -q --release --bin arena || exit 1
}

# Runs $1 (a scenario path), optionally at a custom rep count in $2.
run_scenario() {
    local scenario="$1" reps="${2:-}"
    local run_file="$scenario"
    # A rep override rewrites the scenario into a temp file rather than
    # editing the real one: these files are checked in, and the hand-play
    # copies deliberately ship `reps: 1`.
    if [[ -n "$reps" ]]; then
        TMP_SCENARIO="$(mktemp -t arena-run-XXXX.ron)"
        sed "s/reps: *[0-9]\+,/reps: $reps,/" "$scenario" > "$TMP_SCENARIO"
        run_file="$TMP_SCENARIO"
    fi
    "$ARENA" "$run_file"
}

run_ab() {
    local scenario="$1" reps="${2:-}"
    if [[ ! -f "$POLICY" ]]; then
        echo "no $POLICY installed — there is nothing to compare against" >&2
        return 1
    fi
    echo "=== with the trained policy ==="
    run_scenario "$scenario" "$reps"
    echo
    echo "=== with the policy moved aside (baseline) ==="
    mv "$POLICY" "$STASHED"
    run_scenario "$scenario" "$reps"
    restore_policy
    echo
    echo "policy restored to $POLICY"
}

launch_game() {
    echo "launching the game — [R] Arena on the main menu"
    exec env FERAL_DEV_ARENA=1 cargo run --release
}

# ── the menu ─────────────────────────────────────────────────────────────

pick_scenario() {
    local names=("$@")
    echo
    echo "Scenarios in $ARENAS_DIR:"
    echo
    local i=1
    for n in "${names[@]}"; do
        printf '  %2d) %-20s %s\n' "$i" "$n" "$(describe "$n")"
        i=$((i + 1))
    done
    echo
    echo "   p) play in the game's own arena screen (the real battle UI)"
    echo "   q) quit"
    echo
}

pick_action() {
    echo
    echo "What to do with $1:"
    echo
    echo "   1) run it (the file's own rep count)"
    echo "   2) compare with and without the trained enemy policy"
    echo "   3) run it at a rep count you choose"
    echo "   b) back to the scenario list"
    echo
}

menu() {
    local names
    mapfile -t names < <(scenario_names)
    if [[ ${#names[@]} -eq 0 ]]; then
        echo "no scenarios in $ARENAS_DIR" >&2
        exit 1
    fi

    while true; do
        pick_scenario "${names[@]}"
        local choice
        read -r -p "scenario> " choice || exit 0
        case "$choice" in
            q|Q|"") exit 0 ;;
            p|P) launch_game ;;
        esac
        if ! [[ "$choice" =~ ^[0-9]+$ ]] || (( choice < 1 || choice > ${#names[@]} )); then
            echo "pick 1-${#names[@]}, p, or q"
            continue
        fi
        local name="${names[$((choice - 1))]}"
        local scenario="$ARENAS_DIR/$name.ron"

        pick_action "$name"
        local action
        read -r -p "action> " action || exit 0
        case "$action" in
            b|B|"") continue ;;
            1) build_arena; run_scenario "$scenario"; return ;;
            2) build_arena; run_ab "$scenario"; return ;;
            3)
                local reps
                read -r -p "reps> " reps || exit 0
                if ! [[ "$reps" =~ ^[0-9]+$ ]] || (( reps < 1 )); then
                    echo "reps must be a positive number"
                    continue
                fi
                echo
                echo "  1) run it   2) compare with and without the policy"
                local sub
                read -r -p "action> " sub || exit 0
                build_arena
                case "$sub" in
                    2) run_ab "$scenario" "$reps" ;;
                    *) run_scenario "$scenario" "$reps" ;;
                esac
                return
                ;;
            *) echo "pick 1, 2, 3 or b" ;;
        esac
    done
}

# ── argument handling ────────────────────────────────────────────────────

NAME="${1:-}"

case "$NAME" in
    -h|--help)
        usage
        exit 0
        ;;
    --list)
        scenario_names
        exit 0
        ;;
    play)
        launch_game
        ;;
    "")
        # A read on a non-terminal never gets answered, so a piped or
        # redirected call gets the usage rather than a hang.
        if [[ -t 0 ]]; then
            menu
            exit $?
        fi
        usage
        echo
        echo "scenarios in $ARENAS_DIR:"
        scenario_names | sed 's/^/  /'
        exit 0
        ;;
esac

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
    scenario_names | sed 's/^/  /' >&2
    exit 1
fi

build_arena
if [[ "$AB" -eq 1 ]]; then
    run_ab "$SCENARIO" "$REPS"
else
    run_scenario "$SCENARIO" "$REPS"
fi
