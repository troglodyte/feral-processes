#!/usr/bin/env python3
"""Synthesizes the base-space ambience clips into assets/sounds/.

    python3 scripts/ambient-sounds.py

Standard library only, fixed seed: rerunning rewrites byte-identical files,
so a tweak here shows up in git as exactly the clips it changed.

Same format as the action cues (22050 Hz mono 16-bit), and peaks around the
same level as them; how loud ambience plays under the cues is the gui's
mix constant (`crates/gui/src/ambience.rs`), not baked in here.
"""

import math
import random
import struct
import wave
from pathlib import Path

RATE = 22050
PEAK = 10000
OUT = Path(__file__).resolve().parent.parent / "assets" / "sounds"
TAU = 2 * math.pi


def write(name, samples):
    top = max(abs(s) for s in samples) or 1.0
    frames = b"".join(
        struct.pack("<h", int(round(s / top * PEAK))) for s in samples
    )
    with wave.open(str(OUT / name), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(frames)


def envelope(i, n, attack, release):
    """Linear attack/release in samples, so no clip starts or ends on a click."""
    return min(1.0, i / attack if attack else 1.0, (n - 1 - i) / release if release else 1.0)


def drone():
    """An 8 s loop. Every frequency and LFO rate is a multiple of 1/8 Hz, so
    each completes whole cycles and the last sample runs into the first."""
    seconds = 8
    n = RATE * seconds
    rng = random.Random(7)
    # Circular one-pole low-pass: the second pass starts from the state the
    # first ended in, so the hiss is continuous across the loop point too.
    noise = [rng.uniform(-1, 1) for _ in range(n)]
    hiss = [0.0] * n
    state = 0.0
    for _ in range(2):
        for i in range(n):
            state += 0.02 * (noise[i] - state)
            hiss[i] = state
    out = []
    for i in range(n):
        t = i / RATE
        swell = 0.75 + 0.25 * math.sin(TAU * 0.125 * t)
        hum = (
            math.sin(TAU * 55.0 * t)
            + 0.8 * math.sin(TAU * 55.25 * t)
            + 0.35 * math.sin(TAU * 110.125 * t)
        )
        # A slow pitch wobble high above the hum: the "warble".
        warble = 0.12 * math.sin(TAU * 220.0 * t + 3.0 * math.sin(TAU * 0.5 * t))
        out.append(swell * hum + warble * (0.5 + 0.5 * math.sin(TAU * 0.25 * t)) + 3.0 * hiss[i])
    return out


def tone(freqs, seconds, vibrato_hz=0.0, vibrato_depth=0.0, harmonic=0.0, attack=0.005, release=0.03):
    """`freqs` is (start, end) for a linear sweep; phase is accumulated so the
    sweep and vibrato don't jump."""
    n = int(RATE * seconds)
    start, end = freqs
    phase = 0.0
    out = []
    for i in range(n):
        t = i / RATE
        f = start + (end - start) * i / n
        f *= 1 + vibrato_depth * math.sin(TAU * vibrato_hz * t)
        phase += TAU * f / RATE
        s = math.sin(phase) + harmonic * math.sin(3 * phase)
        out.append(s * envelope(i, n, int(attack * RATE), int(release * RATE)))
    return out


def concat(*parts, gap=0.0):
    out = []
    silence = [0.0] * int(gap * RATE)
    for k, part in enumerate(parts):
        if k:
            out += silence
        out += part
    return out


def ping():
    """A decaying ping with two fading echoes, as if across a large room."""
    n = int(RATE * 0.25)
    dry = [math.sin(TAU * 1568 * i / RATE) * math.exp(-i / (RATE * 0.05)) for i in range(n)]
    delay = int(RATE * 0.18)
    out = [0.0] * (n + 2 * delay)
    for k, gain in enumerate((1.0, 0.35, 0.12)):
        for i, s in enumerate(dry):
            out[i + k * delay] += gain * s
    return out


def trill():
    rng = random.Random(11)
    notes = [rng.choice((988, 1175, 1319, 1568, 1760)) for _ in range(8)]
    return concat(*(tone((f, f), 0.035, harmonic=0.2, attack=0.002, release=0.008) for f in notes))


def main():
    write("ambient_drone.wav", drone())
    write("ambient_beep.wav", tone((1200, 1200), 0.09, harmonic=0.25))
    write("ambient_chirp.wav", concat(tone((880, 880), 0.06), tone((1320, 1320), 0.06), gap=0.03))
    write("ambient_warble_up.wav", tone((300, 900), 0.4, vibrato_hz=18, vibrato_depth=0.06, release=0.08))
    write("ambient_warble_down.wav", tone((700, 250), 0.5, vibrato_hz=12, vibrato_depth=0.08, release=0.1))
    write("ambient_ping.wav", ping())
    write("ambient_trill.wav", trill())


if __name__ == "__main__":
    main()
