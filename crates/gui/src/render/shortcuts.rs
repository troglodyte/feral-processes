//! The held-`?` shortcut sheet: `App::shortcut_sheet` drawn as a popup over
//! whatever screen it was summoned on. Read-only; app-core decides the keys.

use super::popup::*;
use super::*;
use feral_processes_app_core::{ShortcutGroup, ShortcutSheet};

/// Characters between the two columns.
const COLUMN_GAP: usize = 4;
/// Characters between a key and its label.
const KEY_GAP: usize = 2;

/// One drawn line: a group title or a `key  label` row.
struct Cell {
    text: String,
    title: bool,
}

fn column_of(groups: &[&ShortcutGroup]) -> Vec<Cell> {
    let key_w = groups
        .iter()
        .flat_map(|g| g.keys.iter())
        .map(|(k, _)| k.chars().count())
        .max()
        .unwrap_or(0);
    let mut out = Vec::new();
    for (i, g) in groups.iter().enumerate() {
        if i > 0 {
            out.push(Cell {
                text: String::new(),
                title: false,
            });
        }
        out.push(Cell {
            text: g.title.to_string(),
            title: true,
        });
        for (key, label) in &g.keys {
            out.push(Cell {
                text: format!("{key:<key_w$}{}{label}", " ".repeat(KEY_GAP)),
                title: false,
            });
        }
    }
    out
}

/// The sheet as one or two columns of cells. Groups stay whole; the split is
/// the first group boundary at or past half the lines, and only happens when
/// a single column would not fit `max_rows`.
fn columns(sheet: &ShortcutSheet, max_rows: usize) -> Vec<Vec<Cell>> {
    let all: Vec<&ShortcutGroup> = sheet.groups.iter().collect();
    let single = column_of(&all);
    if single.len() <= max_rows || all.len() < 2 {
        return vec![single];
    }
    let total: usize = all.iter().map(|g| g.keys.len() + 2).sum();
    let mut seen = 0;
    let mut cut = all.len() - 1;
    for (i, g) in all.iter().enumerate() {
        seen += g.keys.len() + 2;
        if seen * 2 >= total {
            cut = (i + 1).min(all.len() - 1);
            break;
        }
    }
    vec![column_of(&all[..cut]), column_of(&all[cut..])]
}

fn column_chars(col: &[Cell]) -> usize {
    col.iter()
        .map(|c| c.text.chars().count())
        .max()
        .unwrap_or(0)
}

/// Text width in characters of the whole drawn block.
#[cfg(test)]
fn block_chars(cols: &[Vec<Cell>]) -> usize {
    cols.iter().map(|c| column_chars(c)).sum::<usize>() + COLUMN_GAP * (cols.len() - 1)
}

pub(super) fn draw_shortcuts(sheet: &ShortcutSheet, painter: &Painter, m: &Metrics) {
    let cols = columns(
        sheet,
        popup_max_rows(painter.screen_h(), PopupSize::Large, m),
    );
    let height = cols.iter().map(Vec::len).max().unwrap_or(0);
    let blank = vec![text_row(""); height];
    draw_popup(&sheet.title, PopupSize::Large, &blank, None, painter, m);
    let rect = popup_rect(PopupSize::Large, &blank, None, painter, m);
    let char_w = painter.measure_ui_advance("M", m.font_size);
    let mut x = rect.x + m.pad;
    for col in &cols {
        for (i, cell) in col.iter().enumerate() {
            let y = rect.y + m.line_height * (2.0 + i as f32);
            let color = if cell.title { CYAN } else { TEXT };
            painter.ui(&cell.text, x, y, m.font_size, color);
        }
        x += (column_chars(col) + COLUMN_GAP) as f32 * char_w;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_app_core::{ShortcutArea, shortcut_sheet_for};

    const AREAS: [ShortcutArea; 5] = [
        ShortcutArea::Base,
        ShortcutArea::Field,
        ShortcutArea::Stack,
        ShortcutArea::TacticalMap,
        ShortcutArea::ClassicBattle,
    ];

    /// Width and height census at the smallest window: a sheet too long
    /// loses its tail, one too wide runs off the box, both in silence.
    #[test]
    fn every_sheet_fits_its_popup_at_1280x720() {
        let m = ui_metrics(720.0);
        crate::paint::with_painter(|p| {
            let char_w = p.measure_ui_advance("M", m.font_size);
            let (pct_w, _) = popup_fractions(PopupSize::Large);
            let room = 1280.0 * pct_w - 2.0 * m.pad;
            let max_rows = popup_max_rows(720.0, PopupSize::Large, &m);
            for area in AREAS {
                let sheet = shortcut_sheet_for(area);
                let cols = columns(&sheet, max_rows);
                let wide = block_chars(&cols) as f32 * char_w;
                assert!(wide <= room, "{area:?} is {wide}px into {room}px");
                for col in &cols {
                    assert!(
                        col.len() <= max_rows,
                        "{area:?} has a column of {} rows, room for {max_rows}",
                        col.len()
                    );
                }
            }
        });
    }

    #[test]
    fn the_popup_draws_the_sheets_title_and_keys() {
        let m = ui_metrics(720.0);
        let sheet = shortcut_sheet_for(ShortcutArea::Base);
        let (_, shapes) = crate::paint::with_painter(|p| draw_shortcuts(&sheet, p, &m));
        let drawn = crate::paint::painted_text(&shapes);
        assert!(drawn.contains(&sheet.title), "{drawn:#?}");
        assert!(drawn.iter().any(|t| t.starts_with("v ")), "{drawn:#?}");
    }
}
