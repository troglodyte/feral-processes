# Variations: head (rows 0-7) of one base + body (rows 8-15) of another,
# recoloured by lightness rank onto a ramp in the species' authored hue.
import sys
from PIL import Image
SP = "/home/trog/code/feral-processes/assets/sprites"
OUT = sys.argv[1]

RAMPS = {  # darkest -> lightest (Tailwind 950..200)
 "stone":   "1c1917 44403c 57534e 78716c a8a29e d6d3d1 e7e5e4 f5f5f4",
 "zinc":    "18181b 27272a 3f3f46 52525b 71717a a1a1aa d4d4d8 e4e4e7",
 "red":     "450a0a 7f1d1d 991b1b b91c1c dc2626 ef4444 f87171 fca5a5",
 "rose":    "4c0519 881337 9f1239 be123c e11d48 f43f5e fb7185 fda4af",
 "emerald": "022c22 064e3b 065f46 047857 059669 10b981 34d399 6ee7b7",
 "green":   "052e16 14532d 166534 15803d 16a34a 22c55e 4ade80 86efac",
 "lime":    "1a2e05 365314 3f6212 4d7c0f 65a30d 84cc16 a3e635 d9f99d",
 "blue":    "172554 1e3a8a 1e40af 1d4ed8 2563eb 3b82f6 60a5fa 93c5fd",
 "teal":    "042f2e 134e4a 115e59 0f766e 0d9488 14b8a6 2dd4bf 99f6e4",
 "brown":   "1c0f05 3b2410 4a2e14 5c3a1a 7a5128 8a6a3a a8875a c9ab80",
 "fuchsia": "4a044e 701a75 86198f a21caf c026d3 d946ef e879f9 f0abfc",
}
# species: (head base, body base, ramp, flip_body_vertical_accent, extras)
SPECIES = {
 "crawler":     ("glitch",   "virus",     "stone"),
 "drone":       ("cipher",   "scrapper",  "zinc"),
 "overseer":    ("construct","virus",     "red"),
 "proxy":       ("scrapper", "zero_day",  "emerald"),
 "rootkit":     ("virus",    "glitch",    "rose"),
 "sentinel":    ("scrapper", "construct", "blue"),
 "sprite":      ("glitch",   "zero_day",  "teal"),
 "sub_process": ("cipher",   "glitch",    "green"),
 "trojan":      ("construct","scrapper",  "brown"),
 "wintermute":  ("virus",    "construct", "fuchsia"),
 "worm":        ("zero_day", "virus",     "lime"),
}
BOSSES = {"overseer", "wintermute"}

def light(c):
    r, g, b = c[:3]
    return 0.299 * r + 0.587 * g + 0.114 * b

def load(n):
    return Image.open(f"{SP}/{n}.colour.png").convert("RGBA")

def build(name, head, body, ramp):
    h, b = load(head), load(body)
    im = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
    im.paste(h.crop((0, 0, 16, 8)), (0, 0))
    im.paste(b.crop((0, 8, 16, 16)), (0, 8))
    px = im.load()
    if name in BOSSES:  # a crown: horn tips pushed out and up
        for x, y in ((2, 0), (13, 0), (1, 1), (14, 1)):
            px[x, y] = (255, 255, 255, 255) if y == 0 else px[7, 2]
    cols = sorted({px[x, y][:3] for x in range(16) for y in range(16)
                   if px[x, y][3] and px[x, y][:3] != (255, 255, 255)}, key=light)
    stops = [tuple(int(s[i:i+2], 16) for i in (0, 2, 4)) for s in RAMPS[ramp].split()]
    lo, hi = light(cols[0]), light(cols[-1])
    m = {c: stops[round((light(c) - lo) / (hi - lo) * (len(stops) - 1))] for c in cols}
    for y in range(16):
        for x in range(16):
            p = px[x, y]
            if p[3] and p[:3] in m:
                px[x, y] = m[p[:3]] + (255,)
    return im

S = 10 * 16
names = list(SPECIES)
sheet = Image.new("RGBA", (len(names) * (S + 8) + 8, S + 16), (11, 17, 23, 255))
for i, n in enumerate(names):
    im = build(n, *SPECIES[n])
    im.save(f"{OUT}/{n}.colour.png")
    big = im.resize((S, S), Image.NEAREST)
    sheet.alpha_composite(big, (8 + i * (S + 8), 8))
sheet.save(f"{OUT}/../sheet.png")
