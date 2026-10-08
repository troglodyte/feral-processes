//! The Excavation plan's brush picker: plain, every loaded finish with its
//! swatch, strip. The rows are `App::excavate_brush_rows`, the same list
//! the keys pick from.

use super::popup::*;
use super::*;
use feral_processes_app_core::BrushRow;

/// A full block, so the swatch is the shade the map will paint rather than
/// a letter tinted it.
const SWATCH: char = '█';

pub(super) fn draw_excavate_brush(
    rows: &[BrushRow],
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    draw_popup(
        "Floor brush",
        PopupSize::Large,
        &body_rows(rows, selected),
        refusal,
        painter,
        m,
    );
}

fn body_rows(rows: &[BrushRow], selected: usize) -> Vec<Row> {
    let mut body = vec![
        text_row("Pick what the next box marks. Esc to go back."),
        text_row(""),
    ];
    body.extend(rows.iter().enumerate().map(|(i, row)| {
        let item = item_row(row.label.clone(), i == selected);
        match row.shade {
            Some(shade) => with_icon(item, SWATCH, terrain::shade_color(shade)),
            None => item,
        }
    }));
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_engine::components::FinishOrder;
    use feral_processes_engine::floors::{FloorId, FloorShade};

    /// A finish row wears its shade as a swatch, and plain and strip wear
    /// none — the picker is where the player chooses a colour, so the
    /// colour has to be on the row.
    #[test]
    fn a_finish_row_draws_its_shade_as_a_swatch() {
        let rows = [
            BrushRow {
                brush: None,
                label: "plain".to_string(),
                shade: None,
            },
            BrushRow {
                brush: Some(FinishOrder::Apply(FloorId::from("x"))),
                label: "Wine".to_string(),
                shade: Some(FloorShade::Wine),
            },
        ];
        let icons: Vec<_> = body_rows(&rows, 0)
            .into_iter()
            .filter_map(|row| match row {
                Row::Item { icon, .. } => Some(icon),
                _ => None,
            })
            .collect();
        assert_eq!(
            icons,
            vec![None, Some((SWATCH, terrain::shade_color(FloorShade::Wine)))]
        );
    }
}
