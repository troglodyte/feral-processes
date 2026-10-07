# Story text (mods)

`ending.ron` is the sequence of screens shown when the player confirms the
Basin Exit. Drop in an edited copy to change it; no recompiling.

```ron
(
    screens: [
        (
            title: "Screen title",
            body: [
                "One paragraph per string.",
                "Another paragraph.",
            ],
        ),
    ],
)
```

- `screens` is paged through in order, then play resumes.
- Text is not wrapped by hand; keep each paragraph a few sentences, and keep
  titles short.
- A missing file uses a short built-in ending silently. A file that does not
  parse, or lists no screens, is skipped with a logged warning and the
  built-in ending is used, so the game stays completable.
