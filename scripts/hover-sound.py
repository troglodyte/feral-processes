#!/usr/bin/env python3
"""Synthesizes the pointer's hover cue into assets/sounds/hover.wav.

    python3 scripts/hover-sound.py

Standard library only and deterministic, so rerunning rewrites a
byte-identical file. Same format as `ambient-sounds.py` (22050 Hz mono
16-bit).

A soft, high, very short tick: it plays every time the pointer moves onto
a control, so it sits well under the step cue's level and decays before a
quick sweep down a list can pile it up into a buzz.
"""

import math
import struct
import wave
from pathlib import Path

RATE = 22050
# The step cue peaks at ~7200; a cue heard this often stays far below it.
PEAK = 2600
OUT = Path(__file__).resolve().parent.parent / "assets" / "sounds"
TAU = 2 * math.pi


def hover():
    seconds = 0.025
    n = int(RATE * seconds)
    attack = int(RATE * 0.0015)
    out = []
    for i in range(n):
        t = i / RATE
        tone = math.sin(TAU * 3200 * t) + 0.4 * math.sin(TAU * 5100 * t)
        env = min(1.0, i / attack) * math.exp(-t / 0.006)
        out.append(tone * env)
    return out


def write(name, samples):
    top = max(abs(s) for s in samples) or 1.0
    frames = b"".join(struct.pack("<h", int(round(s / top * PEAK))) for s in samples)
    with wave.open(str(OUT / name), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(frames)


if __name__ == "__main__":
    write("hover.wav", hover())
