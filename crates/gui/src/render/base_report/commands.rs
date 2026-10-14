//! The command pages (Build, Research, Trade, Compile): one row per thing
//! that can be done from where the party stands. The rows come from
//! `App::command_rows`, the table the handler dispatches, so a label here can
//! never name a different action from the one under the highlight.

use super::super::group_menu::group_row;
use super::*;
use feral_processes_app_core::GroupMenuRow;

pub(super) fn command_rows(rows: &[GroupMenuRow], selected: usize) -> Vec<Row> {
    labelled_rows(&rows.iter().map(|r| r.label).collect::<Vec<_>>(), selected)
}

/// Split from `command_rows` because a `GroupMenuRow` can only be built by
/// app-core, whose rows need a `Game` the gui tests cannot make.
fn labelled_rows(labels: &[&str], selected: usize) -> Vec<Row> {
    if labels.is_empty() {
        return vec![text_row("Nothing here can be done from where you are.")];
    }
    labels
        .iter()
        .enumerate()
        .map(|(i, label)| group_row(i, label, i == selected))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::fit::{assert_rows_fit, text_of};
    use super::*;

    #[test]
    fn no_rows_gives_the_explanatory_line() {
        let out = labelled_rows(&[], 0);
        assert_eq!(out.len(), 1);
        assert_eq!(
            text_of(&out[0]),
            "Nothing here can be done from where you are."
        );
    }

    #[test]
    fn three_rows_are_numbered_and_the_selected_one_highlighted() {
        let out = labelled_rows(&["One", "Two", "Three"], 1);
        let texts: Vec<_> = out.iter().map(text_of).collect();
        assert_eq!(texts, ["[1] One", "[2] Two", "[3] Three"]);
        let selected = |r: &Row| matches!(r, Row::Item { selected: true, .. });
        assert!(!selected(&out[0]) && selected(&out[1]) && !selected(&out[2]));
    }

    /// Every label in `group_menu.rs`'s BUILD/RESEARCH/TRADE/COMPILE tables;
    /// gui cannot build a `Game` to read them. Update with the tables.
    const LABELS: &[&str] = &[
        "Deploy a structure",
        "Work a structure yourself",
        "Upgrade a structure",
        "Demolish a structure",
        "Research",
        "Routine research",
        "Affix research",
        "Study a program",
        "Hold a program",
        "Contracts",
        "Caravan",
        "Dispatch",
        "Compile an item",
        "Recipes",
    ];

    #[test]
    fn every_command_label_fits_the_content_rect() {
        assert_rows_fit(&labelled_rows(LABELS, 0), "command");
    }
}
