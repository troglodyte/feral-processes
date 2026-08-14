//! The routine panel and the extraction flow.

use super::popup::*;
use super::*;
use feral_processes_engine::views::{
    EtchedDiskView, ExtractableRoutineView, KnownRoutineView, RoutineSlotView,
};

pub(super) fn draw_routine_target(
    game: &mut Game,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let holders = game.routine_holders();
    let mut rows = vec![text_row("Whose routines?")];
    for (i, h) in holders.iter().enumerate() {
        rows.push(with_icon(
            item_row(
                format!(
                    "[{}] {} Lv{} - {}/{} slots",
                    menu_shortcut(i),
                    h.name,
                    h.level,
                    h.filled,
                    h.slots
                ),
                i == selected,
            ),
            h.glyph,
            glyph_color(h.color),
        ));
    }
    draw_popup("Routines", PopupSize::Large, &rows, painter, m);
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
        "Pick a filled slot to clear it — the disk is already spent — or an empty one to install.",
    )];
    for (i, s) in slots.iter().enumerate() {
        rows.push(item_row(
            format!("[{}] {}", menu_shortcut(i), s.name),
            i == selected,
        ));
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
    painter: &Painter,
    m: &Metrics,
) {
    let Some(holder) = holder else { return };
    let rows = routine_slot_rows(&game.routine_view(holder), selected);
    draw_popup("Routines", PopupSize::Large, &rows, painter, m);
}

/// The install picker's rows. The two trailing rows are a deliberate footer —
/// a legend pinned below the list rather than scrolling with it — which is
/// the one thing `popup_layout` is allowed to hold back here.
pub(super) fn routine_install_rows(
    disks: &[EtchedDiskView],
    blanks: u32,
    selected: usize,
) -> Vec<Row> {
    let mut rows = vec![text_row(
        "Install which disk? Installing spends it — popping the routine back out later returns nothing.",
    )];
    if disks.is_empty() {
        rows.push(text_row(
            "(you are carrying no etched disks — press [e] to burn one, or find one on a boss)",
        ));
    }
    for (i, d) in disks.iter().enumerate() {
        // Exclusive disks are marked because they are the ones a wrong
        // choice cannot be undone on: an ordinary routine can be etched
        // again for a blank, and this one cannot be etched at all.
        let tag = if d.exclusive { "  ★ exclusive" } else { "" };
        rows.push(item_row(
            format!("[{}] {} ×{}{}", menu_shortcut(i), d.name, d.qty, tag),
            i == selected,
        ));
        rows.push(description_row(&d.description));
    }
    rows.push(text_row(""));
    rows.push(text_row(format!(
        "[e] burn a blank with a routine you know    Blanks: {blanks}"
    )));
    rows
}

pub(super) fn draw_routine_install(game: &Game, selected: usize, painter: &Painter, m: &Metrics) {
    let rows = routine_install_rows(&game.etched_disks_held(), game.blank_disks_held(), selected);
    draw_popup("Install Routine", PopupSize::Large, &rows, painter, m);
}

/// The etch picker's rows.
///
/// A routine already in cargo carries a dim `×N held` note: this screen is
/// where a blank is spent for good, and the question it has to answer before
/// that is whether the player is starting a stock or adding to one they
/// forgot they had. Nothing at all on a routine held zero of — a `×0` on
/// every line is the noise the annotation exists to avoid.
pub(super) fn routine_etch_rows(
    known: &[KnownRoutineView],
    blanks: u32,
    selected: usize,
) -> Vec<Row> {
    let mut rows = vec![
        text_row("Burn which routine onto a blank? The blank is gone either way."),
        text_row(format!("Blanks: {blanks}")),
    ];
    if known.is_empty() {
        rows.push(text_row(
            "(you know no routines — research one, or extract one from a program)",
        ));
    }
    for (i, r) in known.iter().enumerate() {
        rows.push(annotated_item_row(
            format!("[{}] {}", menu_shortcut(i), r.name),
            (r.held > 0).then(|| format!("×{} held", r.held)),
            i == selected,
            TEXT,
        ));
        rows.push(description_row(&r.description));
    }
    rows
}

pub(super) fn draw_routine_etch(game: &Game, selected: usize, painter: &Painter, m: &Metrics) {
    let rows = routine_etch_rows(&game.etchable_routines(), game.blank_disks_held(), selected);
    draw_popup("Etch Disk", PopupSize::Large, &rows, painter, m);
}

/// One extraction candidate's lines: the row its shortcut selects, then the
/// routines the program is carrying underneath.
///
/// Extraction destroys the program for exactly one of those routines, and
/// `Game::extract_routine` *refuses* one the player already knows — so a
/// program whose whole kit is known is worth nothing on the block, and
/// without this the only way to find that out is to open each program in
/// turn. The `(known)` tag is what says it; the next page says the same
/// thing about a single row with `(already known)`.
///
/// The routines shed onto their own lines through `continuation_lines`
/// rather than joining the row, for the reason that function states. A
/// program carrying nothing gets no line at all.
///
/// Returns the lines rather than drawing them so their width is measurable
/// without a window — see `the_widest_shipped_routine_kit_fits_the_extract_picker`.
/// The program's row is always present, so a caller may take it
/// unconditionally.
fn extract_candidate_rows(
    num: char,
    p: &PetInfo,
    routines: &[ExtractableRoutineView],
) -> Vec<String> {
    let kit: Vec<String> = routines
        .iter()
        .map(|r| {
            let known = if r.known { " (known)" } else { "" };
            format!("{}{known}", r.name)
        })
        .collect();
    std::iter::once(format!(
        "[{num}] {} Lv{}{}",
        p.name,
        p.level,
        fusion_tag(p.fusions)
    ))
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
    i: usize,
    p: &PetInfo,
    selected: bool,
) {
    let mut lines = extract_candidate_rows(menu_shortcut(i), p, routines).into_iter();
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

pub(super) fn draw_extract(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let programs = game.owned_pets();
    let mut rows = vec![text_row(
        "Break down which program? Extraction destroys it and teaches you one of its routines.",
    )];
    if !game.can_extract_routines() {
        rows.push(text_row("(you need a Compiler standing somewhere first)"));
    }
    for (i, p) in programs.iter().enumerate() {
        let routines = game.extractable_routines(p.entity);
        push_extract_candidate(&mut rows, &routines, i, p, i == selected);
    }
    draw_popup("Extract", PopupSize::Large, &rows, painter, m);
}

pub(super) fn extract_pick_rows(offered: &[ExtractableRoutineView], selected: usize) -> Vec<Row> {
    let mut rows = vec![text_row("Learn which routine? The rest are lost with it.")];
    for (i, a) in offered.iter().enumerate() {
        let known = if a.known { " (already known)" } else { "" };
        rows.push(item_row(
            format!("[{}] {}{known}", menu_shortcut(i), a.name),
            i == selected,
        ));
        rows.push(description_row(&a.description));
    }
    rows
}

pub(super) fn draw_extract_pick(
    game: &Game,
    program: Option<Entity>,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(program) = program else { return };
    let rows = extract_pick_rows(&game.extractable_routines(program), selected);
    draw_popup("Extract", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_extract_confirm(
    game: &Game,
    program: Option<Entity>,
    index: Option<usize>,
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
        text_row(format!("Learn {} and destroy the program?", kept.name)),
        text_row(""),
    ];
    if !lost.is_empty() {
        rows.push(text_row(format!("This loses: {}.", lost.join(", "))));
    }
    rows.push(text_row("Enter to confirm, Esc to back out."));
    draw_popup("Extract", PopupSize::Large, &rows, painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;

    fn offered(name: &str, known: bool) -> ExtractableRoutineView {
        ExtractableRoutineView {
            ability: name.to_lowercase().replace(' ', "_"),
            name: name.to_string(),
            description: String::new(),
            known,
        }
    }

    /// The picker's whole job is choosing which program to destroy, and the
    /// thing that decides it is what each one is carrying. `extract_routine`
    /// refuses a routine already known, so a kit that is all `(known)` is a
    /// program with nothing to give — the tag is the only thing on this
    /// screen that says so.
    #[test]
    fn an_extract_candidate_names_its_routines_and_marks_the_known_ones() {
        let lines = extract_candidate_rows(
            'a',
            &test_pet("Kestrel", "w|a|m"),
            &[offered("Sandbox", false), offered("Patch Routine", true)],
        );
        assert!(lines[0].contains("Kestrel"), "{lines:?}");
        assert!(
            !lines[0].contains("Sandbox"),
            "the row the eye scans stays the program: {lines:?}"
        );
        let under = lines[1..].join(" ");
        assert!(
            under.contains("Patch Routine (known)"),
            "a routine already known is marked: {lines:?}"
        );
        assert!(
            under.contains("Sandbox") && !under.contains("Sandbox (known)"),
            "one not yet known is named plainly: {lines:?}"
        );
    }

    /// A program carrying nothing gets no continuation line, rather than an
    /// empty one that reads as a rendering fault.
    #[test]
    fn an_extract_candidate_with_no_routines_gets_no_line() {
        let lines = extract_candidate_rows('a', &test_pet("Kestrel", "w|a|m"), &[]);
        assert_eq!(lines.len(), 1, "{lines:?}");
    }

    /// `draw_row` clamps a row vertically and nothing clamps it
    /// horizontally, so the kit has to shed onto continuation lines rather
    /// than off the right edge. The same census
    /// `the_widest_shipped_routine_kit_fits_the_fuse_picker` runs, against
    /// the worst case *this* screen can build — which is wider, because
    /// every name here can carry a `(known)` tag the fuse picker has no
    /// equivalent of.
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
        names.truncate(feral_processes_engine::tuning::COMPANION_ROUTINE_SLOT_CAP as usize);
        assert!(
            names.len() > 1,
            "the census found {} routines, so it is measuring nothing",
            names.len()
        );
        let kit: Vec<ExtractableRoutineView> = names.iter().map(|n| offered(n, true)).collect();
        let lines = extract_candidate_rows('a', &test_pet("Kestrel", "w|a|m"), &kit);
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
