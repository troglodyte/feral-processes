#!/usr/bin/env python3
"""Synthesizes the battle map's routine cue into assets/sounds/routine.wav.

    python3 scripts/routine-sound.py

Standard library only and deterministic, so rerunning rewrites a
byte-identical file. Same format as `ambient-sounds.py` (22050 Hz mono
16-bit), peaking just under the hit and reaction clips.

A rising, warbling sweep with a bright overtone: it plays when a routine is
invoked, before its hits land, so it has to read as "something was run" and
stay out of the hit/crit/miss clips' register rather than sound like a blow.
"""

import math
import struct
import wave
from pathlib import Path

RATE = 22050
PEAK = 12000
OUT = Path(__file__).resolve().parent.parent / "assets" / "sounds"
TAU = 2 * math.pi


def routine():
    seconds = 0.22
    n = int(RATE * seconds)
    attack = int(RATE * 0.008)
    release = int(RATE * 0.07)
    phase = 0.0
    out = []
    for i in range(n):
        t = i / n
        freq = 420 + 900 * t * t
        freq *= 1 + 0.05 * math.sin(TAU * 32 * i / RATE)
        phase += TAU * freq / RATE
        tone = math.sin(phase) + 0.35 * math.sin(2 * phase) + 0.15 * math.sin(3 * phase)
        env = min(1.0, i / attack, (n - 1 - i) / release)
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
    write("routine.wav", routine())
