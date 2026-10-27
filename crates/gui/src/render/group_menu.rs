//! The dev keypad and the command-row format the hub's command pages share;
//! the commands themselves are hub pages, see `render::base_report`.

use super::popup::*;
use super::*;

/// The dev keypad. Same popup shape as the group menus and drawn beside
/// them deliberately — it is one more list of labelled actions, and a file
/// of its own would only separate it from the helpers it shares.
///
/// The labels come from `App::dev_row_label` over `App::dev_console_rows`,
/// the same table the handler dispatches, so a label here can never name an
/// action the press does not take.
pub(super) fn draw_dev_console(
    labels: &[String],
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mut out = vec![text_row("Dev build only")];
    for (i, label) in labels.iter().enumerate() {
        out.push(item_row(label.to_string(), i == selected));
    }
    push_key_footer(&mut out, &["Enter choose", "Esc back"]);
    draw_popup("Dev console", PopupSize::Large, &out, refusal, painter, m);
}

/// One `[a] Label` command row: the hub's command pages
/// share it so the format has one home.
pub(super) fn group_row(label: &str, selected: bool) -> Row {
    item_row(label.to_string(), selected)
}
