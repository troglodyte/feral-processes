# feral-processes — GitHub Pages site

This branch holds **only** the landing page. It is an orphan branch: it shares
no history with `main`, and nothing here is part of the game.

Live at <https://troglodyte.github.io/feral-processes/> once Pages is pointed at
it (Settings → Pages → Source: *Deploy from a branch* → `gh-pages` / `(root)`).

```
index.html      the whole page — one file, no build step, no dependencies
img/            screenshots and clips
.nojekyll       serve files as-is; do not run Jekyll over them
```

## Screenshots

Every image in `img/` is a 1280×720 capture from the game itself, taken from
a `main` checkout with a display:

```sh
cargo run -- --template <name> --keys "<keys>" --screenshot out.png
```

| image | how it was taken |
|---|---|
| `hero.png` | `--template siege` |
| `base.png` | `--template chains` |
| `staff.png` | `--template chains --keys "b 4"` |
| `research.png` | `--template chains --keys "b 9"` |
| `social.png` | `--template bonds --keys "p Enter M Tab"` |
| `memories.png` | `--template bonds --keys "p Enter R"` |
| `roster.png` | `--template bonds --keys "p Enter"` |
| `downed.png` | `--template bonds --keys "D"` |
| `towns.png` | `--template settlements` |
| `stack.png` | `--template stack` |
| `intrusion.png` | `FERAL_DEV_ARENA=1`, `--keys "R l Down×7 Enter f"` (dev-arenas `full-group.ron`) |

Keys run before the first frame and no time passes, so a walk can't be
scripted: a move key only queues the step. Never ship a `FERAL_DEV_REVEAL=1`
shot, because it labels itself `[DEV REVEAL]`. A tactical battle map needs
the Options toggle, which writes the real `profile.ron`, so there isn't one
yet.

To add one to the gallery, drop the PNG in `img/` and copy a `<figure
class="shot">` block in the `#media` section. `full` spans both columns.

## Keeping it honest

The stats row states eight content counts, and the version appears twice — the
hero badge and the footer. Those are counts of `assets/*/*.ron` on `main` and a
read of the workspace version, and every one of them drifts. Re-derive them
rather than trusting the page:

```sh
cd /path/to/main-checkout
for d in species abilities items structures contracts classes affixes perks; do
  printf '%-12s %s\n' "$d" "$(ls assets/$d/*.ron | wc -l)"
done
grep -m1 '^version' Cargo.toml
```

The counts on the page as of v0.14.6: 17 species, 103 routines, 75 items,
43 structures, 48 contracts, 8 classes, 23 affixes, 22 perks.

The root `README.md` on `main` is carved out of the doc-update obligation and is
already stale on several of these; do not copy its numbers. `docs/manual.md` is
carved out too — the live manual is `assets/help/`, which is what the page links
to.

## What the page must not claim

Retired mechanics that earlier copy described, and that must stay off it:

- Breaching does **not** rebuild the sector. The world is one continuous map per
  run; a breach raises the danger tier and leaves the base, the structures and
  every town found standing where they were.
- Beating a wild program drops **no** materials on the spot. The body is carried
  home and broken down with a tool.
- Tactical battle maps are **opt-in and off by default**. Don't present them as
  the default combat model.
- Production is **one worker per line** (0.14.6), not one per machine.
- Research is an **active project** that consumes a pinned subject from
  sector 2, not a bank of points.
- Level-ups are **spent** (six points across the attributes, 0.14.0), not
  automatic stat growth.
- The GC Entropy Sweep is **rare and telegraphed**, not constant, and
  sector 1 never sees one.

The page names genres ("basebuilder", "staff management with memories and
personalities", "old-school dungeon crawler"), never other games.
