#!/usr/bin/env python3
"""Writes the starter battle-map prop sprites into assets/sprites/.

16x16 RGBA, near-white: the renderer tints a prop by one brightness and its
biome's faint cast, so the art carries shape and shading only. Cover pieces
are mostly opaque; decoration leaves most of the cell transparent so the
ground shows through. Every destructible piece also gets a `_cracked` twin,
shown below half its hit points. Deterministic, seeded per file name. Stdlib
only.

The script is a starting point, not the source of truth: a sprite redrawn in
the Sprite Forge is what ships, so an existing PNG is never overwritten
without --force.

    python3 scripts/battle-props.py [--force]
"""

import random
import struct
import sys
import zlib
from pathlib import Path

SIZE = 16
OUT = Path(__file__).resolve().parent.parent / "assets" / "sprites"


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


class Canvas:
    """A grid of grey levels in 0..1, or None for a transparent pixel."""

    def __init__(self):
        self.g = [[None] * SIZE for _ in range(SIZE)]

    def put(self, x, y, v):
        if 0 <= x < SIZE and 0 <= y < SIZE:
            self.g[y][x] = v

    def rect(self, x0, y0, x1, y1, v):
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                self.put(x, y, v)

    def hline(self, x0, x1, y, v):
        self.rect(x0, y, x1, y, v)

    def vline(self, x, y0, y1, v):
        self.rect(x, y0, x, y1, v)

    def bevel(self, x0, y0, x1, y1, lit=1.0, shade=0.55):
        """A raised edge: lit top and left, shadowed bottom and right."""
        self.hline(x0, x1, y0, lit)
        self.vline(x0, y0, y1, lit)
        self.hline(x0, x1, y1, shade)
        self.vline(x1, y0, y1, shade)

    def pixels(self):
        out = []
        for row in self.g:
            line = []
            for v in row:
                if v is None:
                    line.append((255, 255, 255, 0))
                else:
                    b = round(max(0.0, min(1.0, v)) * 255)
                    line.append((b, b, b, 255))
            out.append(line)
        return out


def server_rack(c, rng):
    c.rect(2, 1, 13, 14, 0.82)
    c.bevel(2, 1, 13, 14)
    for y in (3, 6, 9, 12):
        c.hline(4, 11, y, 0.5)
        c.hline(4, 8, y + 1 if y < 12 else y, 0.7)
    for i, y in enumerate((3, 6, 9, 12)):
        # A lit LED, or a dead one where the rack has gone dark.
        c.put(11, y, 1.0 if i % 2 == 0 else 0.35)
    c.rect(5, 13, 10, 13, 0.62)


def hull_plate(c, rng):
    c.rect(1, 2, 14, 13, 0.8)
    c.bevel(1, 2, 14, 13)
    c.vline(8, 3, 12, 0.55)
    c.hline(2, 13, 7, 0.6)
    for x, y in ((2, 3), (13, 3), (2, 12), (13, 12)):
        c.put(x, y, 1.0)
    for i in range(5):
        c.put(3 + i * 2, 9 + (i % 2), 0.68)


def pylon(c, rng):
    c.rect(6, 1, 9, 14, 0.8)
    c.bevel(6, 1, 9, 14)
    c.rect(4, 0, 11, 1, 0.9)
    c.rect(3, 14, 12, 15, 0.7)
    for y in (4, 8, 12):
        c.hline(6, 9, y, 0.55)
    for i in range(3):
        c.put(5 - i, 5 + i, 0.75)
        c.put(10 + i, 5 + i, 0.75)


def drone_chassis(c, rng):
    c.rect(4, 5, 11, 11, 0.78)
    c.rect(3, 7, 12, 9, 0.78)
    c.bevel(4, 5, 11, 11)
    c.rect(6, 7, 9, 9, 0.4)
    c.put(7, 8, 0.95)
    for dx in (-1, 1):
        x = 7 if dx < 0 else 8
        for i in range(1, 5):
            c.put(x + dx * i, 5 - i // 2 - 1, 0.7)
    c.hline(1, 3, 2, 0.6)
    c.hline(12, 14, 3, 0.6)
    c.put(1, 12, 0.7)
    c.put(2, 13, 0.55)


def power_cell(c, rng):
    c.rect(4, 3, 11, 13, 0.82)
    c.bevel(4, 3, 11, 13)
    c.rect(6, 1, 9, 2, 0.95)
    for p in ((8, 5), (7, 6), (6, 7), (7, 8), (8, 8), (9, 8), (8, 9), (7, 10), (6, 11)):
        c.put(*p, 1.0)
    c.hline(5, 10, 12, 0.5)


def coolant_tank(c, rng):
    c.rect(3, 2, 12, 14, 0.78)
    c.vline(4, 3, 13, 1.0)
    c.vline(5, 3, 13, 0.92)
    c.vline(12, 3, 13, 0.55)
    c.vline(11, 3, 13, 0.62)
    for y in (4, 11):
        c.hline(3, 12, y, 0.5)
    c.rect(7, 0, 8, 1, 0.9)
    c.rect(5, 0, 10, 0, 0.7)
    c.rect(6, 7, 9, 8, 0.45)
    c.put(7, 7, 0.95)


def conduit_wall(c, rng):
    c.rect(0, 4, 15, 11, 0.82)
    c.hline(0, 15, 4, 1.0)
    c.hline(0, 15, 11, 0.5)
    c.hline(0, 15, 7, 0.6)
    for x in (0, 5, 10, 15):
        c.rect(x, 2, min(x + 1, 15), 13, 0.92)
        c.vline(min(x + 1, 15), 2, 13, 0.55)
    for x in (3, 8, 13):
        c.put(x, 5, 0.95)
        c.put(x, 10, 0.65)


def conduit_wall_broken(c, rng):
    conduit_wall(c, rng)
    # A blown section: the pipe is gone and the flanges hang open.
    c.rect(6, 4, 9, 11, None)
    c.put(6, 5, 0.7)
    c.put(9, 9, 0.7)
    c.hline(6, 7, 11, 0.5)
    c.hline(8, 9, 4, 0.9)
    c.put(7, 7, 1.0)
    c.put(8, 8, 0.45)


def rubble(c, rng):
    for _ in range(11):
        x, y = rng.randrange(1, SIZE - 3), rng.randrange(2, SIZE - 3)
        w, h = rng.randint(2, 3), rng.randint(2, 3)
        c.rect(x, y, x + w - 1, y + h - 1, 0.62 + rng.uniform(0, 0.1))
        c.hline(x, x + w - 1, y, 0.95)
        c.hline(x, x + w - 1, y + h - 1, 0.45)


def cable_run(c, rng):
    y = 5
    for x in range(SIZE):
        y = max(2, min(13, y + rng.choice((-1, 0, 0, 1))))
        c.put(x, y, 0.95)
        c.put(x, y + 1, 0.55)
    y = 10
    for x in range(SIZE):
        y = max(6, min(14, y + rng.choice((-1, 0, 1))))
        c.put(x, y, 0.8)
        c.put(x, y + 1, 0.5)


def scorch(c, rng):
    for y in range(SIZE):
        for x in range(SIZE):
            d = ((x - 7.5) ** 2 + (y - 8.0) ** 2) ** 0.5
            if d < 6.5 and rng.random() < 1.0 - d / 7.5:
                c.put(x, y, 0.38 + rng.uniform(0, 0.15))


def glass_shards(c, rng):
    for _ in range(7):
        x, y = rng.randrange(1, 13), rng.randrange(1, 13)
        n = rng.randint(2, 4)
        for i in range(n):
            c.hline(x, x + n - 1 - i, y + i, 1.0)
        c.put(x, y, 0.7)


def dead_screen(c, rng):
    c.rect(1, 3, 14, 12, 0.7)
    c.bevel(1, 3, 14, 12)
    c.rect(3, 5, 12, 10, 0.4)
    for i in range(4):
        c.put(4 + i, 9 - i, 0.8)
    c.put(11, 5, 0.9)
    c.rect(6, 13, 9, 14, 0.7)
    c.hline(5, 10, 15, 0.7)


def crack(c, rng):
    """Damage over a finished sprite: a dark crack and a few knocked-out chips."""
    x = rng.randrange(5, 11)
    for y in range(SIZE):
        if c.g[y][x] is not None:
            c.g[y][x] = 0.2
        x = max(1, min(SIZE - 2, x + rng.choice((-1, 0, 1))))
        if c.g[y][x] is not None and y % 3 == 0:
            c.g[y][x - 1 if x > 1 else x] = 0.3
    for _ in range(5):
        x, y = rng.randrange(SIZE), rng.randrange(SIZE)
        if c.g[y][x] is not None and (x in (0, SIZE - 1) or y in (0, SIZE - 1) or rng.random() < 0.4):
            c.g[y][x] = None


# (name, drawer, has a cracked twin). Mirrors assets/battle-props/pieces/:
# the destructible pieces are exactly the ones that crack.
PIECES = [
    ("server_rack", server_rack, True),
    ("hull_plate", hull_plate, True),
    ("pylon", pylon, True),
    ("drone_chassis", drone_chassis, True),
    ("power_cell", power_cell, True),
    ("coolant_tank", coolant_tank, True),
    ("conduit_wall_broken", conduit_wall_broken, True),
    ("conduit_wall", conduit_wall, False),
    ("rubble", rubble, False),
    ("cable_run", cable_run, False),
    ("scorch", scorch, False),
    ("glass_shards", glass_shards, False),
    ("dead_screen", dead_screen, False),
]


def main():
    force = "--force" in sys.argv[1:]
    OUT.mkdir(parents=True, exist_ok=True)
    for name, draw, cracks in PIECES:
        key = f"prop_{name}"
        rng = random.Random(key)
        canvas = Canvas()
        draw(canvas, rng)
        targets = [(key, canvas)]
        if cracks:
            twin = Canvas()
            twin.g = [row[:] for row in canvas.g]
            crack(twin, random.Random(key + "_cracked"))
            targets.append((key + "_cracked", twin))
        for stem, cv in targets:
            path = OUT / f"{stem}.png"
            if path.exists() and not force:
                print(f"skip {path.name} (exists)")
                continue
            write_png(path, cv.pixels())
            print(f"wrote {path.name}")


if __name__ == "__main__":
    main()
