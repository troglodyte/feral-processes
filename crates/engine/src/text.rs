//! Text shaping shared by the engine and the renderer.
//!
//! The wrap lives here rather than in gui because a read-only screen's row
//! count is owned by app-core: a per-row transform done in the renderer
//! opens the screen on rows that are not drawn.

/// Greedy word wrap to `columns`, for prose too long to sit on one popup
/// row — an item's authored description, chiefly.
///
/// A word longer than `columns` is emitted whole on its own line rather
/// than split or dropped: one row running wide is a smaller problem than
/// losing text, and the alternative is hyphenating identifiers.
pub fn wrap(text: &str, columns: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if line.is_empty() {
            line.push_str(word);
        } else if line.chars().count() + 1 + word.chars().count() <= columns {
            line.push(' ');
            line.push_str(word);
        } else {
            lines.push(std::mem::take(&mut line));
            line.push_str(word);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_breaks_on_spaces_and_never_exceeds_the_column_budget() {
        let text = "A fragment of stolen authorization, cut for locks the Stack \
                    no longer uses.";
        let lines = wrap(text, 30);
        assert!(
            lines.iter().all(|l| l.chars().count() <= 30),
            "no wrapped line may run past the budget: {lines:?}"
        );
        assert_eq!(
            lines.join(" "),
            text,
            "wrapping must preserve the words and their order exactly"
        );
    }

    #[test]
    fn a_short_description_stays_on_one_line() {
        assert_eq!(
            wrap("Standard armor. Solid protection.", 72),
            vec!["Standard armor. Solid protection."]
        );
    }

    /// A single word longer than the budget has nowhere to break, so it gets
    /// its own overlong line rather than being silently truncated or
    /// dropped. Losing text is worse than one row running wide.
    #[test]
    fn an_unbreakable_word_gets_its_own_line_rather_than_being_lost() {
        let lines = wrap("tiny supercalifragilisticexpialidocious end", 10);
        assert!(lines.contains(&"supercalifragilisticexpialidocious".to_string()));
        assert_eq!(
            lines.join(" "),
            "tiny supercalifragilisticexpialidocious end"
        );
    }
}
