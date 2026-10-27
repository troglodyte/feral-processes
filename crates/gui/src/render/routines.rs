//! The routine panel and the extraction flow.

use super::popup::*;
use super::*;
use feral_processes_engine::views::{BuiltRoutineRow, ExtractableRoutineView, RoutineSlotView};

pub(super) fn draw_routine_target(
    game: &mut Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let holders = game.routine_holders();
    let mut rows = vec![text_row("Whose routines?")];
    for (i, h) in holders.iter().enumerate() {
        rows.push(with_icon(
            item_row(
                format!("{} Lv{} - {}/{} slots", h.name, h.level, h.filled, h.slots),
                i == selected,
            ),
            h.glyph,
            glyph_color(h.color),
        ));
    }
    push_key_footer(&mut rows, &["Enter choose", "Esc back"]);
    draw_popup("Routines", PopupSize::Large, &rows, refusal, painter, m);
}

/// A routine's own text, indented under the row it belongs to.
///
/// **A `Row::Item`, never a `Row::Text`**, and every picker in this file owes
/// its descriptions to this. `popup_layout` ends the scrollable body at the
/// *last* `Row::Item` and pins whatever follows as a footer — so the final
/// routine's description, emitted as text, was drawn detached at the bottom
/// of the popup under the scroll indicator while every other description sat
/// inline under its own row. It read as missing. `build_menu_rows`
/// (`render/building.rs`) carries the same fix for the same reason.
fn description_row(description: &str) -> Row {
    colored_item_row(format!("    {description}"), false, TEXT_DIM)
}

/// The slot panel's rows, pure so the layout invariant above can be tested
/// without a `Game` or a `Painter`.
pub(super) fn routine_slot_rows(slots: &[RoutineSlotView], selected: usize) -> Vec<Row> {
    let mut rows = vec![text_row(
        "Pick a filled slot to clear it - the routine is destroyed - or an empty one to install.",
    )];
    for (i, s) in slots.iter().enumerate() {
        // A fixed slot is tagged rather than left to refuse when pressed:
        // the header a line above promises a filled slot can be cleared, and
        // the one routine that cannot is the one the player has run since
        // they booted. Not the `★` the install picker spends on exclusive
        // disks — that mark means "you get one of these", and this means
        // "this row is not a choice".
        let tag = if s.fixed { "  (fixed)" } else { "" };
        rows.push(item_row(format!("{}{}", s.name, tag), i == selected));
        if !s.description.is_empty() {
            rows.push(description_row(&s.description));
        }
    }
    rows
}

pub(super) fn draw_routines(
    game: &Game,
    holder: Option<Entity>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(holder) = holder else { return };
    let mut rows = routine_slot_rows(&game.routine_view(holder), selected);
    push_key_footer(&mut rows, &["Enter choose", "Esc back"]);
    draw_popup("Routines", PopupSize::Large, &rows, refusal, painter, m);
}

/// The install picker's rows: the routines built at the Workbench and not
/// yet in a slot.
pub(super) fn routine_install_rows(built: &[BuiltRoutineRow], selected: usize) -> Vec<Row> {
    let mut rows = vec![text_row(
        "Install which routine? Popping it back out later destroys it.",
    )];
    if built.is_empty() {
        rows.push(text_row(
            "(nothing built - compile a routine at a Workbench first)",
        ));
    }
    for (i, b) in built.iter().enumerate() {
        rows.push(item_row(b.name.clone(), i == selected));
        rows.push(description_row(&b.description));
    }
    rows
}

pub(super) fn draw_routine_install(
    game: &Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = routine_install_rows(&game.built_routine_rows(), selected);
    push_key_footer(&mut rows, &["Enter install", "Esc cancel"]);
    draw_popup(
        "Install routine",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

/// One extraction candidate's lines: the row the highlight lands on, then the
/// routines the program is carrying underneath.
///
/// Extraction destroys the program for exactly one of those routines, so
/// what each program carries is what decides which one to put on the block.
///
/// The routines shed onto their own lines through `continuation_lines`
/// rather than joining the row, for the reason that function states. A
/// program carrying nothing gets no line at all.
///
/// Returns the lines rather than drawing them so their width is measurable
/// without a window — see `the_widest_shipped_routine_kit_fits_the_extract_picker`.
/// The program's row is always present, so a caller may take it
/// unconditionally.
fn extract_candidate_rows(p: &PetInfo, routines: &[ExtractableRoutineView]) -> Vec<String> {
    let kit: Vec<String> = routines.iter().map(|r| r.name.clone()).collect();
    std::iter::once(format!("{} Lv{}{}", p.name, p.level, fusion_tag(p.fusions)))
        .chain(continuation_lines(&kit.join(", ")))
        .collect()
}

/// Pushes one candidate's rows: the selectable program row, then its
/// routines as dim unselected continuations. The same shape — and the same
/// reasoning about the highlight and the scroll anchor — as
/// `push_fuse_candidate`.
fn push_extract_candidate(
    rows: &mut Vec<Row>,
    routines: &[ExtractableRoutineView],
    p: &PetInfo,
    selected: bool,
) {
    let mut lines = extract_candidate_rows(p, routines).into_iter();
    let head = lines
        .next()
        .expect("extract_candidate_rows always emits the program's row");
    rows.push(with_icon(
        tier_row(head, selected, p.fusions, p.rarity),
        p.glyph,
        glyph_color(p.color),
    ));
    for line in lines {
        rows.push(colored_item_row(line, false, TEXT_DIM));
    }
}

pub(super) fn draw_extract(
    game: &mut Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let programs = game.owned_pets();
    let mut rows = vec![text_row(
        "Break down which program? Extraction destroys it and gives you the Subroutines of one of its routines.",
    )];
    if !game.can_extract_routines() {
        rows.push(text_row("(you need a Compiler standing somewhere first)"));
    }
    for (i, p) in programs.iter().enumerate() {
        let routines = game.extractable_routines(p.entity);
        push_extract_candidate(&mut rows, &routines, p, i == selected);
    }
    push_key_footer(&mut rows, &["Enter choose", "Esc cancel"]);
    draw_popup("Extract", PopupSize::Large, &rows, refusal, painter, m);
}

pub(super) fn extract_pick_rows(offered: &[ExtractableRoutineView], selected: usize) -> Vec<Row> {
    let mut rows = vec![text_row(
        "Take the Subroutines of which routine? The rest are lost with it.",
    )];
    for (i, a) in offered.iter().enumerate() {
        rows.push(item_row(a.name.clone(), i == selected));
        rows.push(description_row(&a.description));
    }
    rows
}

pub(super) fn draw_extract_pick(
    game: &Game,
    program: Option<Entity>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(program) = program else { return };
    let mut rows = extract_pick_rows(&game.extractable_routines(program), selected);
    push_key_footer(&mut rows, &["Enter choose", "Esc cancel"]);
    draw_popup("Extract", PopupSize::Large, &rows, refusal, painter, m);
}

pub(super) fn draw_extract_confirm(
    game: &Game,
    program: Option<Entity>,
    index: Option<usize>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let (Some(program), Some(index)) = (program, index) else {
        return;
    };
    let offered = game.extractable_routines(program);
    let Some(kept) = offered.get(index) else {
        return;
    };
    let lost: Vec<&str> = offered
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != index)
        .map(|(_, a)| a.name.as_str())
        .collect();
    let mut rows = vec![
        text_row(format!(
            "Take the Subroutines of {} and destroy the program?",
            kept.name
        )),
        text_row(""),
    ];
    if !lost.is_empty() {
        rows.push(text_row(format!("This loses: {}.", lost.join(", "))));
    }
    rows.push(text_row(""));
    rows.push(item_row("[y] Yes, destroy it", selected == 0));
    rows.push(item_row("[n] No, keep it", selected == 1));
    rows.push(text_row(""));
    rows.push(text_row(key_hints(&[
        "[Y] destroy it",
        "[N] or Esc cancel",
    ])));
    draw_popup("Extract", PopupSize::Large, &rows, refusal, painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;

    fn offered(name: &str) -> ExtractableRoutineView {
        ExtractableRoutineView {
            ability: name.to_lowercase().replace(' ', "_"),
            name: name.to_string(),
            description: String::new(),
        }
    }

    /// The picker's whole job is choosing which program to destroy, and the
    /// thing that decides it is what each one is carrying.
    #[test]
    fn an_extract_candidate_names_its_routines() {
        let lines = extract_candidate_rows(
            &test_pet("Kestrel", "w|a|m"),
            &[offered("Sandbox"), offered("Patch Routine")],
        );
        assert!(lines[0].contains("Kestrel"), "{lines:?}");
        assert!(
            !lines[0].contains("Sandbox"),
            "the row the eye scans stays the program: {lines:?}"
        );
        let under = lines[1..].join(" ");
        assert!(
            under.contains("Sandbox") && under.contains("Patch Routine"),
            "{lines:?}"
        );
    }

    /// A program carrying nothing gets no continuation line, rather than an
    /// empty one that reads as a rendering fault.
    #[test]
    fn an_extract_candidate_with_no_routines_gets_no_line() {
        let lines = extract_candidate_rows(&test_pet("Kestrel", "w|a|m"), &[]);
        assert_eq!(lines.len(), 1, "{lines:?}");
    }

    /// `draw_row` clamps a row vertically and nothing clamps it
    /// horizontally, so the kit has to shed onto continuation lines rather
    /// than off the right edge. The same census
    /// `the_widest_shipped_routine_kit_fits_the_fuse_picker` runs, against
    /// the worst case *this* screen can build.
    ///
    /// Measured against the real ability set rather than a literal, so an
    /// author naming a routine longer fails this instead of shipping a line
    /// that runs off the box.
    #[test]
    fn the_widest_shipped_routine_kit_fits_the_extract_picker() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/abilities");
        let (db, warnings) = feral_processes_engine::abilities::AbilityDb::load_dir(&dir)
            .expect("the abilities load");
        assert!(warnings.is_empty(), "{warnings:?}");
        let mut names: Vec<String> = db.all().map(|d| d.name.clone()).collect();
        names.sort_by_key(|n| std::cmp::Reverse(n.chars().count()));
        names.truncate(feral_processes_engine::tuning::ROUTINE_SLOT_CAP as usize);
        assert!(
            names.len() > 1,
            "the census found {} routines, so it is measuring nothing",
            names.len()
        );
        let kit: Vec<ExtractableRoutineView> = names.iter().map(|n| offered(n)).collect();
        let lines = extract_candidate_rows(&test_pet("Kestrel", "w|a|m"), &kit);
        with_painter(|p| {
            let m = ui_metrics(900.0);
            // 0.88 is `PopupSize::Large`'s width fraction, against the
            // 1440x900 geometry `ui_metrics` is calibrated for.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for line in &lines {
                let drawn = p.measure_ui_advance(line, m.font_size);
                assert!(
                    drawn <= room,
                    "an extract candidate's line overflows the picker by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
        });
    }
}
