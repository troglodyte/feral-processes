# In-game help pages

Every `.md` file in this directory is one page of the manual the player
reads with `?` inside the game. Adding a topic is dropping a file in here —
no Rust change, no rebuild, no registration anywhere.

Loaded by `HelpDb::load_dir` (`crates/engine/src/help.rs`). This file is the
schema reference; keep it in step with that parser.

## The filename is the identity

    10-start-here.md   →   order 10, id "start-here"

The leading digits are the sort key and the rest is the id a link points at.
Reordering the index is renaming files; there is no front matter, and so no
second thing to keep in step with the name. Ties on the number break on the
id, so the index is stable between runs.

A file **without** the `NN-` prefix is skipped with a warning rather than
defaulted to order 0 — ordering is the whole of what the filename is for,
and a silent default makes it ambiguous. Files that are not `.md` are
ignored without comment.

## The grammar

Five rules, and deliberately no more:

| Line | Means |
| --- | --- |
| `# Title` on the first non-blank line | the page's title |
| a blank line | a paragraph break |
| `- text` | a bullet |
| anything else | paragraph text, wrapped when it is read |
| `[label](topic-id)` inline | reads as `label`, and adds a further-reading row |

There is no emphasis, no second heading, no nesting, no tables and no code
fences. A `#` anywhere but the first line is an ordinary paragraph, so a
stray one cannot silently retitle a page.

Lines inside a block are joined, exactly as a blank line being the paragraph
break implies. So you may wrap a long sentence — or a long bullet — across
as many source lines as you like:

    - m Excavation plan — space anchors a box, space again marks it,
      Esc backs out.

is one bullet. Wrapping happens at read time against the popup's width, so
column alignment padded with spaces will not survive; use bullets where you
want rows.

## Links

`[label](topic-id)` does two things from one authoring gesture: the sentence
reads as `label`, and `topic-id` joins that page's further-reading list —
deduped, in first-appearance order. That is why there is no `see_also:`
field: the cross-reference is written once, where it belongs in the
sentence.

A link whose target is not a real page is **dropped from the list** and
reported as a load warning. The prose still reads as written; only the menu
row goes. A page carries at most nine links, because a further-reading row
is followed by typing its shortcut.

## What gets a page skipped

Skipped with a logged warning, never a panic — same contract as every other
asset directory:

- a filename with no `NN-` prefix
- an id already claimed by another file
- no `# Title` on the first non-blank line, or a blank one
- a title with no body under it

## House style

- Write for somebody who has just started a run, not for somebody reading
  the source. `docs/manual.md` is the exhaustive out-of-game reference and
  has no relationship to these files.
- **Never name the keys `W`, `T` or `Z`.** They are deliberately
  undocumented — see `crates/engine/EASTER_EGGS.md`. A census over this
  directory enforces it.
