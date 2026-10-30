use super::*;

/// `draw_row` clips vertically only, so a release row wider than its
/// popup loses the price off the right edge. The widest name a program
/// can carry is a custom one, so the row is measured at that.
#[test]
fn the_release_row_fits_its_popup_and_quotes_the_charged_price() {
    let name = "W".repeat(24);
    let pct = Game::siphon_release_ceiling_percent();
    let rows = siphon_rows(Some(&name), &[], 0, pct);
    let Row::Item { text, .. } = &rows[0] else {
        panic!("the release is an item row");
    };
    assert!(text.contains(&format!("{pct}%")));
    crate::paint::with_painter(|p| {
        let m = crate::text::ui_metrics(900.0);
        let room = 1440.0 * 0.88 - m.pad * 2.0;
        let drawn = p.measure_ui_advance(text, m.font_size);
        assert!(drawn <= room, "overflows by {:.0}px:\n{text}", drawn - room);
    });
}

#[test]
fn an_empty_siphon_lists_the_staff_and_an_empty_staff_says_so() {
    let staff = vec!["aa".to_string(), "bb".to_string()];
    assert_eq!(siphon_rows(None, &staff, 1, 25).len(), 2);
    assert!(matches!(siphon_rows(None, &[], 0, 25)[0], Row::Text(_)));
}
