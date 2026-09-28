---
paths:
  - "**/help*"
  - "**/text*"
  - "assets/help/**"
---

# Load-bearing seams: Help and documentation

One sentence each, the rule alone. The trap is in the `seams` skill; the argument is `seam:<slug>` in the memory graph.

- **The manual's index is a menu and a page is a document**, and that is why
  `Mode::HelpPage` exists rather than the index taking a second job.
- **`[label](topic-id)` is why there is no `see_also:` field.** One gesture
  writes the sentence and the further-reading row.
- **The wrap is `text::wrap` in the engine**, and
  `render/popup.rs::wrap_text` is a call to it.
