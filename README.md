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

The media section has five placeholder slots. Each is a `<div class="ph">`
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

The page states content counts (17 species, 77 routines, 64 items, 26
structures, 30 contracts, 18 perks), the research tree size, and the version in
the hero badge and the footer. Those are counts of `assets/*/*.ron` on `main`
and a read of the workspace version — they drift. Re-check them when you touch
this page:

```sh
git -C /path/to/main-checkout ls-files 'assets/species/*.ron' | wc -l
```

The root `README.md` on `main` is carved out of the doc-update obligation and is
already stale on several of these; do not copy its numbers.
