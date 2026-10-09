#!/usr/bin/env python3
"""Writes the starter battle-map ground tiles into assets/sprites/.

16x16 RGBA, near-white: the renderer tints each tile by its kind's
brightness and its biome's faint cast, so the art carries pattern only.
Deterministic, seeded per file name. Stdlib only, so it runs anywhere the
repo is checked out.

The script is a starting point, not the source of truth: a tile redrawn in
the Sprite Forge is what ships, so an existing PNG is never overwritten
without --force.

    python3 scripts/ground-tiles.py [--force]
"""

import random
import struct
import sys
import zlib
from pathlib import Path

SIZE = 16
OUT = Path(__file__).resolve().parent.parent / "assets" / "sprites"

KINDS = ["open", "rough", "cover", "blocked"]
BIOMES = ["opengrid", "deadlock", "nullsector", "backplane", "platform"]


def write_png(path, pixels):
    """pixels: SIZE rows of SIZE (r, g, b, a) byte tuples."""

    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    raw = b"".join(b"\x00" + bytes(c for px in row for c in px) for row in pixels)
    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )
    path.write_bytes(png)


def kind_base(kind, rng):
    """The kind's shape as a grid of grey levels in 0..1."""
    g = [[0.86 + rng.uniform(-0.03, 0.03) for _ in range(SIZE)] for _ in range(SIZE)]
    if kind == "rough":
        # Broken, cluttered ground: rubble chunks with a lit corner.
        for _ in range(9):
            x, y = rng.randrange(SIZE - 1), rng.randrange(SIZE - 1)
            g[y][x] = g[y][x + 1] = g[y + 1][x] = 0.64
            g[y + 1][x + 1] = 0.70
            g[y][x] = 0.98
    elif kind == "cover":
        # A raised block: lit top edge, shadowed bottom and right.
        for y in range(SIZE):
            for x in range(SIZE):
                if 2 <= x <= 13 and 2 <= y <= 13:
                    g[y][x] = 0.94 + rng.uniform(-0.02, 0.02)
                else:
                    g[y][x] = 0.74
        for x in range(2, 14):
            g[2][x] = 1.0
            g[13][x] = 0.66
        for y in range(3, 14):
            g[y][13] = 0.66
            g[y][2] = 0.97
    elif kind == "blocked":
        # A void with a rim.
        for y in range(SIZE):
            for x in range(SIZE):
                edge = x in (0, SIZE - 1) or y in (0, SIZE - 1)
                g[y][x] = 0.82 if edge else 0.40 + rng.uniform(-0.04, 0.04)
    return g


def apply_motif(g, biome, kind, rng):
    """The biome's motif, kept faint so the kind's shape still leads."""
    # A void keeps its dark interior; only its rim takes the motif.
    def touch(x, y, delta):
        if kind == "blocked" and 0 < x < SIZE - 1 and 0 < y < SIZE - 1:
            return
        g[y][x] = min(1.0, max(0.6, g[y][x] + delta))

    if biome == "opengrid":
        for i in range(SIZE):
            for j in (0, 8):
                touch(i, j, -0.08)
                touch(j, i, -0.08)
    elif biome == "deadlock":
        for y in range(1, SIZE, 4):
            for x in range(SIZE):
                touch(x, y, -0.05)
        x, y = rng.randrange(4, 12), 0
        while y < SIZE:
            touch(x, y, -0.18)
            x = min(SIZE - 1, max(0, x + rng.choice((-1, 0, 1))))
            y += 1
    elif biome == "nullsector":
        for _ in range(14):
            touch(rng.randrange(SIZE), rng.randrange(SIZE), -0.24)
    elif biome == "backplane":
        for _ in range(2):
            y = rng.randrange(3, 13)
            x0, x1 = sorted(rng.sample(range(SIZE), 2))
            for x in range(x0, x1 + 1):
                touch(x, y, -0.12)
            y1 = rng.choice((0, SIZE - 1))
            for yy in range(min(y, y1), max(y, y1) + 1):
                touch(x1, yy, -0.12)
            touch(x0, y, 0.14)
    elif biome == "platform":
        for x in range(SIZE):
            touch(x, 7, -0.12)
            touch(x, 15, -0.12)
        for y in range(0, 7):
            touch(7, y, -0.12)
        for y in range(8, 15):
            touch(15, y, -0.12)
        for x, y in ((2, 2), (12, 2), (4, 10), (10, 12)):
            touch(x, y, 0.12)


def tile(name, kind, biome):
    rng = random.Random(zlib.crc32(name.encode()))
    g = kind_base(kind, rng)
    if biome:
        apply_motif(g, biome, kind, rng)
    return [[(round(v * 255),) * 3 + (255,) for v in row] for row in g]


def main():
    args = sys.argv[1:]
    if args not in ([], ["--force"]):
        sys.exit(f"usage: {sys.argv[0]} [--force]")
    force = bool(args)
    jobs = [(f"ground_{k}", k, None) for k in KINDS]
    jobs += [(f"ground_{b}_{k}", k, b) for b in BIOMES for k in KINDS]
    for name, kind, biome in jobs:
        path = OUT / f"{name}.png"
        if path.exists() and not force:
            print(f"skip {path.name} (exists; --force to overwrite)")
            continue
        write_png(path, tile(name, kind, biome))
        print(f"wrote {path.name}")


if __name__ == "__main__":
    main()
