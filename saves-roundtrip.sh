#!/usr/bin/env bash
#
# saves-roundtrip.sh — Convert every save between the packed .bin format and
# editable RON, using the launcher crate's savetool.
#
# Usage:
#   ./saves-roundtrip.sh dump [saves_dir]   every <dir>/*.bin  -> <dir>/ron/*.ron
#   ./saves-roundtrip.sh pack [saves_dir]   every <dir>/ron/*.ron -> <dir>/*.bin
#
#   saves_dir  Directory holding the saves (default: saves)
#
# Edit the RON between the two steps — that is the whole point of the pair.
# Running dump then pack with no edit in between is a format round-trip check:
# it should leave every save loadable and semantically identical.
#
# CLOSE THE GAME FIRST. It autosaves, so a running game will overwrite whatever
# `pack` just wrote, and a save dumped while it runs is stale the moment the
# next autosave lands.
#
# `pack` overwrites saves, so it copies each one to <name>.bin.<timestamp>.bak
# first. The timestamp is what keeps a second pack from burying the pre-edit
# original under a backup of the edit — a fixed .bak name loses the very copy
# you would want back. The game's save picker filters on the .bin extension, so
# a .bak sitting beside a save never shows up in the load menu.

set -uo pipefail

MODE="${1:-}"
SAVES_DIR="${2:-saves}"
# Tab completion hands over "saves/", and every path below appends its own
# separator — so strip trailing slashes rather than printing "saves//ron/x.ron".
# Guarded so a bare "/" survives as itself.
while [[ "$SAVES_DIR" == */ && "$SAVES_DIR" != "/" ]]; do
    SAVES_DIR="${SAVES_DIR%/}"
done
RON_DIR="$SAVES_DIR/ron"

if [[ "$MODE" != "dump" && "$MODE" != "pack" ]]; then
    sed -n '3,30p' "$0" | sed 's/^# \{0,1\}//'
    exit 1
fi

if [[ ! -d "$SAVES_DIR" ]]; then
    echo "no such directory: $SAVES_DIR" >&2
    exit 1
fi

# Built once up front rather than per save: `cargo run` would re-check the
# workspace on every single call, and this loops over all of them.
echo "building savetool..."
cargo build -q --bin savetool || exit 1
SAVETOOL="target/debug/savetool"

failed=0
converted=0

if [[ "$MODE" == "dump" ]]; then
    mkdir -p "$RON_DIR"
    shopt -s nullglob
    for bin in "$SAVES_DIR"/*.bin; do
        name="$(basename "$bin" .bin)"
        if "$SAVETOOL" dump "$bin" "$RON_DIR/$name.ron"; then
            converted=$((converted + 1))
        else
            echo "  FAILED: $bin" >&2
            failed=$((failed + 1))
        fi
    done
    shopt -u nullglob
    echo "dumped $converted save(s) to $RON_DIR"
else
    if [[ ! -d "$RON_DIR" ]]; then
        echo "no such directory: $RON_DIR — run '$0 dump' first" >&2
        exit 1
    fi
    shopt -s nullglob
    for ron in "$RON_DIR"/*.ron; do
        name="$(basename "$ron" .ron)"
        target="$SAVES_DIR/$name.bin"
        # Back up before overwriting; a save is the only copy of a run, and a
        # fixed .bak name would let a second pack overwrite the original with
        # a copy of the edit.
        [[ -f "$target" ]] && cp "$target" "$target.$(date +%Y%m%d-%H%M%S).bak"
        if "$SAVETOOL" pack "$ron" "$target"; then
            converted=$((converted + 1))
        else
            echo "  FAILED: $ron" >&2
            failed=$((failed + 1))
        fi
    done
    shopt -u nullglob
    echo "packed $converted save(s) into $SAVES_DIR (originals kept as *.bin.bak)"
fi

if [[ "$failed" -gt 0 ]]; then
    echo "$failed conversion(s) failed" >&2
    exit 1
fi
