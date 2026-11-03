use super::*;

/// The picker opens straight off a research choice, so it has to say why it
/// wants a body and what happens to it: `Game::spend_subject` despawns the
/// subject when the study settles.
#[test]
fn the_study_picker_explains_the_subject_does_not_survive() {
    let staff = vec!["aa".to_string(), "bb".to_string()];
    let rows = pin_subject_rows(None, &staff, 1);
    let said: Vec<&str> = rows
        .iter()
        .take_while(|r| !matches!(r, Row::Item { .. }))
        .filter_map(|r| match r {
            Row::Text(s) => Some(s.as_str()),
            _ => None,
        })
        .collect();
    let said = said.join(" ");
    assert!(said.contains("study"), "{said}");
    assert!(said.contains("won't survive"), "{said}");
    let items: Vec<bool> = rows
        .iter()
        .filter_map(|r| match r {
            Row::Item { selected, .. } => Some(*selected),
            _ => None,
        })
        .collect();
    assert_eq!(
        items,
        vec![false, true],
        "the explanation must not shift the cursor"
    );
}

/// `draw_row` clips rather than wraps, so each line is measured against the
/// popup's own width.
#[test]
fn the_study_picker_explanation_fits_its_popup() {
    let rows = pin_subject_rows(None, &["aa".to_string()], 0);
    crate::paint::with_painter(|p| {
        let m = crate::text::ui_metrics(900.0);
        let room = 1440.0 * 0.88 - m.pad * 2.0;
        for row in &rows {
            if let Row::Text(text) = row {
                let drawn = p.measure_ui_advance(text, m.font_size);
                assert!(drawn <= room, "overflows by {:.0}px:\n{text}", drawn - room);
            }
        }
    });
}

#[test]
fn a_pinned_subject_is_one_release_row_with_no_warning() {
    let rows = pin_subject_rows(Some("aa"), &[], 0);
    assert_eq!(rows.len(), 1);
    assert!(matches!(&rows[0], Row::Item { text, .. } if text == "Release aa from study"));
}
