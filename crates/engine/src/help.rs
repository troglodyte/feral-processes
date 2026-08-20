//! The in-game manual: `assets/help/*.md`, one file per topic.
//!
//! A page is prose, not structured data, which is why these are markdown
//! and not RON — there is nothing to validate past "does it have a title",
//! so RON would buy house consistency at the cost of escaping every quote
//! in every paragraph.
//!
//! The grammar is deliberately five rules and no more; `assets/help/README.md`
//! is the schema reference. Ordering and identity come from the filename, so
//! there is no front matter and therefore no second parser.

use std::path::Path;

use crate::text;

/// The width every page is wrapped to. Pinned to a width the popup can
/// actually draw by `crates/gui/src/render/help.rs`, rather than being
/// restated there — a second constant is a second thing to keep in step.
pub const WRAP_COLUMNS: usize = 100;

/// How a wrapped bullet's continuation lines line up: under the text, not
/// under the dash.
const BULLET_MARKER: &str = "- ";
const BULLET_INDENT: &str = "  ";

/// One block of a page. Link syntax is already flattened to its label by the
/// time a block exists, so `page_rows` never sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HelpBlock {
    Paragraph(String),
    Bullet(String),
    Blank,
}

/// A further-reading entry, harvested from `[label](topic-id)` in the prose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpLink {
    pub label: String,
    pub target: String,
}

/// One authored topic.
#[derive(Clone, Debug)]
pub struct HelpPage {
    pub id: String,
    pub title: String,
    pub order: u32,
    pub blocks: Vec<HelpBlock>,
    /// In first-appearance order, deduped by target. Every entry resolves to
    /// a real page: `HelpDb::load_dir` drops the ones that do not, because a
    /// dead link must never render as a row that refuses when picked.
    pub links: Vec<HelpLink>,
}

#[derive(Default)]
pub struct HelpDb {
    /// Sorted by `(order, id)`, so the index is stable between runs.
    pages: Vec<HelpPage>,
}

impl HelpDb {
    /// Loads every `*.md` page in `dir`. A malformed page is skipped with a
    /// returned warning rather than aborting the load, as every other
    /// content db does.
    ///
    /// Link resolution is a **second pass**: a link's target cannot be
    /// checked until the whole directory has parsed.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut pages: Vec<HelpPage> = Vec::new();
        let mut warnings = Vec::new();

        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            // The schema reference lives in the directory it documents and,
            // uniquely among the asset dirs, shares an extension with the
            // content. Skipped by name and in silence — warning about it
            // every load would train everyone to ignore the warnings.
            if stem == "README" {
                continue;
            }
            // A file without the `NN-` prefix is skipped rather than
            // defaulted to order 0: ordering is the whole of what the
            // filename is for, and a silent default makes it ambiguous.
            let Some((order, id)) = split_stem(stem) else {
                warnings.push(format!(
                    "skipped help page {path:?}: a filename must start with an order, as in 10-{stem}.md"
                ));
                continue;
            };
            if pages.iter().any(|p| p.id == id) {
                warnings.push(format!(
                    "skipped help page {path:?}: id {id} is already taken"
                ));
                continue;
            }
            let source = std::fs::read_to_string(&path)?;
            match parse_page(&id, order, &source) {
                Ok(page) => pages.push(page),
                Err(why) => warnings.push(format!("skipped help page {path:?}: {why}")),
            }
        }

        let known: Vec<String> = pages.iter().map(|p| p.id.clone()).collect();
        for page in &mut pages {
            let id = page.id.clone();
            page.links.retain(|link| {
                let resolves = known.contains(&link.target);
                if !resolves {
                    warnings.push(format!(
                        "help page {id} links to {}, which is not a page",
                        link.target
                    ));
                }
                resolves
            });
        }

        pages.sort_by(|a, b| (a.order, &a.id).cmp(&(b.order, &b.id)));
        Ok((HelpDb { pages }, warnings))
    }

    pub fn pages(&self) -> &[HelpPage] {
        &self.pages
    }

    pub fn page(&self, id: &str) -> Option<&HelpPage> {
        self.pages.iter().find(|p| p.id == id)
    }
}

/// `10-start-here` becomes `(10, "start-here")`. `None` when there is no
/// leading run of digits followed by a `-`.
fn split_stem(stem: &str) -> Option<(u32, String)> {
    let (digits, rest) = stem.split_at(stem.find(|c: char| !c.is_ascii_digit())?);
    let rest = rest.strip_prefix('-')?;
    if digits.is_empty() || rest.is_empty() {
        return None;
    }
    Some((digits.parse().ok()?, rest.to_string()))
}

/// Parses one page's source. The title is taken from the **first non-blank
/// line only**, so a stray `#` further down is an ordinary paragraph and
/// cannot silently retitle a page.
pub fn parse_page(id: &str, order: u32, source: &str) -> Result<HelpPage, String> {
    let mut lines = source.lines();
    let title_line = lines
        .by_ref()
        .find(|l| !l.trim().is_empty())
        .ok_or_else(|| "the page is empty".to_string())?;
    let title = title_line
        .trim()
        .strip_prefix('#')
        .ok_or_else(|| "the first line must be a `# Title`".to_string())?
        .trim()
        .to_string();
    if title.is_empty() {
        return Err("the page's title is blank".to_string());
    }

    let mut blocks: Vec<HelpBlock> = Vec::new();
    let mut links: Vec<HelpLink> = Vec::new();
    for raw in lines {
        let line = raw.trim();
        if line.is_empty() {
            // One `Blank` per run, and never a leading one: a paragraph
            // break is what a blank line means, and two of them in the
            // source mean the same break.
            if matches!(blocks.last(), Some(HelpBlock::Blank)) || blocks.is_empty() {
                continue;
            }
            blocks.push(HelpBlock::Blank);
            continue;
        }
        let (text, found) = flatten_links(line);
        for link in found {
            if !links.iter().any(|l| l.target == link.target) {
                links.push(link);
            }
        }
        match text.strip_prefix("- ") {
            Some(bullet) => blocks.push(HelpBlock::Bullet(bullet.trim().to_string())),
            // Anything else continues the block above it, so an author may
            // wrap a long line in the source without splitting it on screen.
            None => match blocks.last_mut() {
                Some(HelpBlock::Paragraph(p)) | Some(HelpBlock::Bullet(p)) => {
                    p.push(' ');
                    p.push_str(&text);
                }
                _ => blocks.push(HelpBlock::Paragraph(text)),
            },
        }
    }

    if matches!(blocks.last(), Some(HelpBlock::Blank)) {
        blocks.pop();
    }
    if blocks.is_empty() {
        return Err("the page has a title and no body".to_string());
    }

    Ok(HelpPage {
        id: id.to_string(),
        title,
        order,
        blocks,
        links,
    })
}

/// Rewrites `[label](target)` to `label`, returning the links it found in
/// source order. Anything that is not a well-formed pair is left alone, so a
/// bare bracket in prose is just a bracket.
fn flatten_links(line: &str) -> (String, Vec<HelpLink>) {
    let mut out = String::new();
    let mut links = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let Some(close) = after.find(']') else { break };
        let Some(tail) = after[close + 1..].strip_prefix('(') else {
            out.push_str(&rest[..open + 1]);
            rest = after;
            continue;
        };
        let Some(end) = tail.find(')') else { break };
        out.push_str(&rest[..open]);
        let label = &after[..close];
        out.push_str(label);
        links.push(HelpLink {
            label: label.to_string(),
            target: tail[..end].to_string(),
        });
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    (out, links)
}

/// A page's prose, wrapped to `columns`. **Prose only** — the further-reading
/// rows are a menu, and a menu's rows are app-core's business.
pub fn page_rows(page: &HelpPage, columns: usize) -> Vec<String> {
    let mut rows = Vec::new();
    for block in &page.blocks {
        match block {
            HelpBlock::Blank => rows.push(String::new()),
            HelpBlock::Paragraph(text) => rows.extend(text::wrap(text, columns)),
            HelpBlock::Bullet(text) => {
                let budget = columns.saturating_sub(BULLET_MARKER.len());
                for (i, line) in text::wrap(text, budget).into_iter().enumerate() {
                    let lead = if i == 0 { BULLET_MARKER } else { BULLET_INDENT };
                    rows.push(format!("{lead}{line}"));
                }
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> HelpPage {
        parse_page("t", 10, source).expect("the fixture page must parse")
    }

    /// The title is the first non-blank line and nothing else, so an author
    /// writing a `#` further down gets a paragraph rather than silently
    /// retitling the page out from under the index.
    #[test]
    fn a_title_comes_only_from_the_first_non_blank_line() {
        let page = parse("\n\n# Start here\n\nsome prose\n\n# not a heading\n");
        assert_eq!(page.title, "Start here");
        assert!(
            page.blocks
                .contains(&HelpBlock::Paragraph("# not a heading".into())),
            "{:#?}",
            page.blocks
        );
    }

    #[test]
    fn bullets_and_blank_lines_survive_the_round_trip() {
        let page = parse("# T\n\nprose\n\n- one\n- two\n\nmore prose\n");
        assert_eq!(
            page.blocks,
            vec![
                HelpBlock::Paragraph("prose".into()),
                HelpBlock::Blank,
                HelpBlock::Bullet("one".into()),
                HelpBlock::Bullet("two".into()),
                HelpBlock::Blank,
                HelpBlock::Paragraph("more prose".into()),
            ]
        );
    }

    /// One authoring gesture does two things: the sentence reads as the
    /// label, and the target joins the further-reading list once however
    /// many times it is written.
    #[test]
    fn a_link_renders_as_its_label_and_registers_once() {
        let page = parse("# T\n\nSee [the zones](zones) now.\n\nAnd [the zones](zones) again.\n");
        let rows = page_rows(&page, 80);
        assert_eq!(rows[0], "See the zones now.");
        assert_eq!(rows[2], "And the zones again.");
        assert_eq!(
            page.links,
            vec![HelpLink {
                label: "the zones".into(),
                target: "zones".into()
            }]
        );
    }

    #[test]
    fn a_page_without_a_title_is_refused() {
        assert!(parse_page("t", 10, "no heading here\n\n# late\n").is_err());
        assert!(parse_page("t", 10, "").is_err());
        assert!(parse_page("t", 10, "# T\n").is_err(), "a title and no body");
    }

    #[test]
    fn page_rows_never_emits_a_row_wider_than_the_column_budget() {
        let long = "wrap ".repeat(60);
        let page = parse(&format!("# T\n\n{long}\n\n- {long}\n"));
        let rows = page_rows(&page, 40);
        assert!(rows.iter().all(|r| r.chars().count() <= 40), "{rows:#?}");
        assert!(
            rows.len() > 4,
            "the fixture has to actually wrap: {rows:#?}"
        );
    }

    /// A bullet's continuation lines line up under the text. Under the dash
    /// they read as a second, unmarked bullet.
    #[test]
    fn a_wrapped_bullet_indents_under_its_text() {
        let page = parse("# T\n\n- alpha bravo charlie delta echo\n");
        let rows = page_rows(&page, 20);
        assert_eq!(rows[0], "- alpha bravo");
        assert!(rows.len() > 1, "{rows:#?}");
        for row in &rows[1..] {
            assert!(row.starts_with(BULLET_INDENT), "{row:?}");
            assert!(!row.starts_with(BULLET_MARKER), "{row:?}");
        }
    }

    fn help_dir(tag: &str, files: &[(&str, &str)]) -> crate::tests::support::ScratchAssets {
        let dir = crate::tests::support::scratch_assets_dir(tag);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(name), body).unwrap();
        }
        dir
    }

    #[test]
    fn pages_come_back_sorted_by_order_then_id() {
        let dir = help_dir(
            "help_order",
            &[
                ("30-zed.md", "# Zed\n\nbody\n"),
                ("10-beta.md", "# Beta\n\nbody\n"),
                ("10-alpha.md", "# Alpha\n\nbody\n"),
                ("notes.txt", "ignored entirely"),
            ],
        );
        let (db, warnings) = HelpDb::load_dir(&dir).unwrap();
        assert_eq!(
            db.pages().iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            vec!["alpha", "beta", "zed"]
        );
        assert!(warnings.is_empty(), "{warnings:#?}");
        assert!(db.page("beta").is_some());
    }

    #[test]
    fn a_file_without_an_order_prefix_is_skipped_with_a_warning() {
        let dir = help_dir("help_prefix", &[("start-here.md", "# Start\n\nbody\n")]);
        let (db, warnings) = HelpDb::load_dir(&dir).unwrap();
        assert!(db.pages().is_empty());
        assert_eq!(warnings.len(), 1, "{warnings:#?}");
    }

    /// The directory's own schema reference is a `.md` file too, which no
    /// other asset directory has to contend with.
    #[test]
    fn the_directorys_readme_is_not_a_page_and_does_not_warn() {
        let dir = help_dir(
            "help_readme",
            &[
                ("README.md", "# Help pages\n\nthe schema reference\n"),
                ("10-real.md", "# Real\n\nbody\n"),
            ],
        );
        let (db, warnings) = HelpDb::load_dir(&dir).unwrap();
        assert_eq!(db.pages().len(), 1);
        assert!(warnings.is_empty(), "{warnings:#?}");
    }

    #[test]
    fn a_titleless_page_is_skipped_with_a_warning() {
        let dir = help_dir(
            "help_titleless",
            &[
                ("10-good.md", "# Good\n\nbody\n"),
                ("20-bad.md", "no title at all\n"),
            ],
        );
        let (db, warnings) = HelpDb::load_dir(&dir).unwrap();
        assert_eq!(db.pages().len(), 1);
        assert_eq!(warnings.len(), 1, "{warnings:#?}");
    }

    /// A dead link is dropped rather than kept, because the further-reading
    /// list is a menu: a row that refuses when picked is worse than a row
    /// that was never offered.
    #[test]
    fn a_link_to_a_nonexistent_page_is_dropped_and_warned_about() {
        let dir = help_dir(
            "help_dead_link",
            &[
                ("10-a.md", "# A\n\nsee [b](b) and [ghost](nowhere)\n"),
                ("20-b.md", "# B\n\nbody\n"),
            ],
        );
        let (db, warnings) = HelpDb::load_dir(&dir).unwrap();
        let a = db.page("a").unwrap();
        assert_eq!(
            a.links,
            vec![HelpLink {
                label: "b".into(),
                target: "b".into()
            }]
        );
        assert_eq!(warnings.len(), 1, "{warnings:#?}");
        assert_eq!(
            page_rows(a, 80)[0],
            "see b and ghost",
            "the prose still reads as written; only the menu row goes"
        );
    }

    #[test]
    fn an_absent_directory_is_an_error_rather_than_a_panic() {
        let dir = crate::tests::support::scratch_assets_dir("help_absent");
        assert!(HelpDb::load_dir(&dir).is_err());
    }
}
