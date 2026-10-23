//! The two lines every filtering list shares: what was typed, and the
//! notice when nothing matches it.

use super::popup::*;

/// The line above the list, drawn only while a filter is typed.
pub(super) fn filter_line(filter: &str) -> Option<Row> {
    (!filter.is_empty()).then(|| text_row(format!("filter: {filter}")))
}

/// Stands in for the rows when the filter leaves none.
pub(super) fn no_match_row(filter: &str) -> Row {
    text_row(format!("Nothing matches \"{filter}\"."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_filter_line_is_only_drawn_for_a_typed_filter() {
        assert!(filter_line("").is_none());
        assert!(matches!(filter_line("ar"), Some(Row::Text(t)) if t == "filter: ar"));
    }
}
