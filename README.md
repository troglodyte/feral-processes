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

## Adding a screenshot or a clip

The media section has five placeholder slots (a gameplay clip, a battle map, a
base running its chains, a town hub, and the roster). Each is a `<div class="ph">`
block with an HTML comment above the section explaining the swap. To fill one:

1. Drop the file into `img/`.
2. Replace that whole `<div class="ph"> … </div>` block with:

```html
<figure class="shot">
  <img src="img/your-file.png" alt="describe what it shows" loading="lazy">
  <figcaption>Your caption.</figcaption>
</figure>
```

A GIF uses the same `<img>` tag. An mp4 uses:

```html
<figure class="shot">
  <video src="img/your-clip.mp4" autoplay muted loop playsinline></video>
  <figcaption>Your caption.</figcaption>
</figure>
```

Keep the `full` class on a block to make it span both gallery columns.

## Keeping it honest

The stats row states eight content counts, and the version appears twice — the
hero badge and the footer. Those are counts of `assets/*/*.ron` on `main` and a
read of the workspace version, and every one of them drifts. Re-derive them
rather than trusting the page:

```sh
cd /path/to/main-checkout
for d in species abilities items structures contracts research affixes perks; do
  printf '%-12s %s\n' "$d" "$(ls assets/$d/*.ron | wc -l)"
done
grep -m1 '^version' Cargo.toml
```

The counts on the page as of v0.13.150: 17 species, 86 routines, 68 items,
36 structures, 40 contracts, 34 research nodes, 20 affixes, 19 perks.

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

Several of the systems the page describes — sorties, caravan routes, town raids
and patrols, the extraction chain — shipped green and have had little or no
screen time. The copy describes their mechanics, which are real; it does not
claim they are tuned.
