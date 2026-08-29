# Help and documentation

- **The manual's index is a menu and a page is a document**, and that is why
  `Mode::HelpPage` exists rather than the index taking a second job.
  `popup_layout`'s scrolling keeps the *selected* row visible, so a
  menu-idiom page opens scrolled to wherever its first selectable row is —
  links at the bottom means opening at the end of the prose, links on top
  means long prose is unreachable. A page scrolls; a link is **typed**.
- **`[label](topic-id)` is why there is no `see_also:` field.** One gesture
  writes the sentence and the further-reading row. Resolution is a second
  pass in `HelpDb::load_dir` (a target cannot be checked until the whole
  directory has parsed) and a dead target is **dropped**, never kept — a
  menu row that refuses when picked is worse than one never offered.
- **The wrap is `text::wrap` in the engine**, and `render/popup.rs::wrap_text`
  is a call to it. A read-only screen's row count is owned by app-core, so a
  per-row transform in the renderer opens the screen on rows that are not
  drawn. **The trap is `assets/help/README.md`**: the one asset directory
  whose schema reference shares an extension with its content, so `load_dir`
  skips that name explicitly, and the easter-egg census reads **parsed
  pages** rather than raw files — the README names all three hidden keys in
  the course of forbidding them.
