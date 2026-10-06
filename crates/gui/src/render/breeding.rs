//! The three breeding pages and the bay's incubation lines.
//!
//! Every string a page prints comes out of a function here that takes plain
//! data, so `tests` can measure it against its popup without a window.

use super::popup::*;
use super::*;
use feral_processes_engine::{BreedPreview, BreedSpeciesPreview, IncubationView, RollRange};

/// One parent row. A program that cannot breed right now says why in the
/// engine's own words and is drawn dim by the caller.
fn parent_label(num: char, p: &PetInfo, refusal: Option<&str>) -> String {
    let base = format!("[{num}] {} Lv{}  PWR {}", p.name, p.level, p.power);
    match refusal {
        Some(reason) => format!("{base} - {reason}"),
        None => base,
    }
}

fn push_parent_rows(rows: &mut Vec<Row>, game: &Game, candidates: &[&PetInfo], selected: usize) {
    for (i, p) in candidates.iter().enumerate() {
        let refusal = game.breed_refusal(p.entity).map(|r| r.reason());
        let label = parent_label(menu_shortcut(i), p, refusal);
        let row = if refusal.is_some() {
            colored_item_row(label, i == selected, TEXT_DIM)
        } else {
            tier_row(label, i == selected, p.fusions, p.rarity)
        };
        rows.push(with_icon(row, p.glyph, glyph_color(p.color)));
    }
}

fn range_text(r: &RollRange) -> String {
    format!("{} {:.2}-{:.2}", r.label, r.min, r.max)
}

/// What a breeding would make, as text rows: the species, the generation,
/// then the six rolls three to a line.
fn preview_lines(preview: &BreedPreview) -> Vec<String> {
    let species = match &preview.species {
        BreedSpeciesPreview::Certain(name) => format!("Child: {name}"),
        BreedSpeciesPreview::OneOf(a, b) => format!("Child: one of {a} or {b}"),
    };
    let mut lines = vec![species, format!("Generation {}", preview.generation)];
    lines.push("Potential, each roll lands in:".to_string());
    for chunk in preview.rolls.chunks(3) {
        let line: Vec<String> = chunk.iter().map(range_text).collect();
        lines.push(format!("  {}", line.join("   ")));
    }
    lines
}

fn label_of(pets: &[PetInfo], e: Option<Entity>) -> String {
    pets.iter()
        .find(|p| Some(p.entity) == e)
        .map_or_else(|| "it".to_string(), |p| p.name.clone())
}

pub(super) fn draw_breed_menu(
    game: &mut Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let pets = game.owned_pets();
    let candidates: Vec<&PetInfo> = pets.iter().collect();
    let mut rows = vec![text_row("Breed which program? Pick the first of two.")];
    if candidates.is_empty() {
        rows.push(text_row("(you have no compiled programs)"));
    }
    push_parent_rows(&mut rows, game, &candidates, selected);
    draw_popup("Breed", PopupSize::Large, &rows, refusal, painter, m);
}

pub(super) fn draw_breed_second_menu(
    game: &mut Game,
    first: Option<Entity>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(first) = first else { return };
    let pets = game.owned_pets();
    let candidates: Vec<&PetInfo> = pets.iter().filter(|p| p.entity != first).collect();
    let mut rows = vec![text_row(format!(
        "Breed {} with which program? Neither is used up.",
        label_of(&pets, Some(first))
    ))];
    // The highlighted row is the one a keypress would pick, so it is the
    // pair the preview describes. A resting program has no preview, because
    // it cannot be picked.
    if let Some(p) = candidates.get(selected)
        && game.breed_refusal(p.entity).is_none()
        && let Some(preview) = game.breed_preview(first, p.entity)
    {
        rows.extend(preview_lines(&preview).into_iter().map(text_row));
        rows.push(text_row(""));
    }
    if candidates.is_empty() {
        rows.push(text_row("(you have no other compiled programs)"));
    }
    push_parent_rows(&mut rows, game, &candidates, selected);
    draw_popup("Breed", PopupSize::Large, &rows, refusal, painter, m);
}

pub(super) fn draw_breed_confirm_menu(
    game: &mut Game,
    first: Option<Entity>,
    second: Option<Entity>,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let (Some(first), Some(second)) = (first, second) else {
        return;
    };
    let pets = game.owned_pets();
    let title = breed_title(
        &label_of(&pets, Some(first)),
        &label_of(&pets, Some(second)),
    );
    let mut rows = vec![text_row(&title[0]), text_row(&title[1]), text_row("")];
    if let Some(preview) = game.breed_preview(first, second) {
        rows.extend(preview_lines(&preview).into_iter().map(text_row));
        rows.push(text_row(""));
    }
    rows.push(text_row(seed_line(game.breeding_seeds_held())));
    rows.push(text_row("Both parents rest for a while afterwards."));
    rows.push(text_row(""));
    rows.push(text_row("Enter to breed, Esc to go back"));
    draw_popup("Breed", PopupSize::Small, &rows, refusal, painter, m);
}

/// Two lines, one parent each: a custom name can run to
/// `MAX_CUSTOM_NAME_LEN` and the confirm popup is half the window.
fn breed_title(first: &str, second: &str) -> [String; 2] {
    [format!("Breed {first}"), format!("with {second}?")]
}

fn seed_line(held: u32) -> String {
    format!("Costs 1 Breeding Seed (you have {held}).")
}

/// One line per bay slot for the structure sheet.
pub(super) fn incubation_lines(slots: &[IncubationView]) -> Vec<String> {
    slots
        .iter()
        .map(|slot| match &slot.child {
            Some(c) => format!(
                "  Incubating: {}, gen {} - {}",
                c.species,
                c.generation,
                c.when()
            ),
            None => "  Empty".to_string(),
        })
        .collect()
}

/// The sheet's closing line: `[B]` is offered only while a slot is free,
/// which is the same test `App` applies before opening the pages.
pub(super) fn bay_footer(slots: &[IncubationView]) -> &'static str {
    if slots.iter().any(|s| s.child.is_none()) {
        "[B] Breed   Any other key to close."
    } else {
        "Any key to close."
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use feral_processes_engine::IncubatingChild;
    use feral_processes_engine::breeding::BreedRefusal;

    fn widest_roll_preview() -> BreedPreview {
        let labels = ["HP", "ATK", "DEF", "Growth", "Assembly", "Extraction"];
        BreedPreview {
            species: BreedSpeciesPreview::OneOf("Polymorph".into(), "Quarantine Daemon".into()),
            generation: 12,
            rolls: labels
                .iter()
                .map(|&label| RollRange {
                    label,
                    min: 0.85,
                    max: 1.39,
                })
                .collect(),
        }
    }

    fn assert_fits(lines: &[String], size: PopupSize) {
        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = popup_body_width(1440.0, size, &m);
            for line in lines {
                let drawn = p.measure_ui_advance(line, m.font_size);
                assert!(
                    drawn <= room,
                    "a breeding line overflows its popup by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
        });
    }

    #[test]
    fn the_preview_fits_both_pages_that_draw_it() {
        let lines = preview_lines(&widest_roll_preview());
        assert_fits(&lines, PopupSize::Large);
        assert_fits(&lines, PopupSize::Small);
    }

    #[test]
    fn the_longest_parent_row_fits_the_picker() {
        let mut p = test_pet(&"W".repeat(feral_processes_engine::MAX_CUSTOM_NAME_LEN), "");
        p.level = 99;
        p.power = 99_999;
        let label = parent_label('z', &p, Some(BreedRefusal::Busy.reason()));
        // The icon and the highlight prefix are drawn ahead of the text.
        assert_fits(&[format!("      {label}")], PopupSize::Large);
    }

    #[test]
    fn every_refusal_reason_fits_a_parent_row() {
        let p = test_pet("Kestrel", "");
        let rows: Vec<String> = ALL_REFUSALS
            .iter()
            .map(|r| parent_label('a', &p, Some(r.reason())))
            .collect();
        assert_fits(&rows, PopupSize::Large);
    }

    const ALL_REFUSALS: [BreedRefusal; 8] = [
        BreedRefusal::Busy,
        BreedRefusal::SameProgram,
        BreedRefusal::NotYours,
        BreedRefusal::OnCooldown,
        BreedRefusal::NoFreeSlot,
        BreedRefusal::NoSeed,
        BreedRefusal::UnknownKind,
        BreedRefusal::Boss,
    ];

    #[test]
    fn the_confirm_page_lines_fit_their_popup() {
        let name = format!(
            "[Z99] {}",
            "W".repeat(feral_processes_engine::MAX_CUSTOM_NAME_LEN)
        );
        let lines = vec![
            breed_title(&name, &name)[0].clone(),
            breed_title(&name, &name)[1].clone(),
            seed_line(99_999),
            "Both parents rest for a while afterwards.".to_string(),
            "Enter to breed, Esc to go back".to_string(),
            "Breed which program? Pick the first of two.".to_string(),
        ];
        assert_fits(&lines, PopupSize::Small);
        assert_fits(
            &["Breed Kestrel with which program? Neither is used up.".to_string()],
            PopupSize::Large,
        );
    }

    #[test]
    fn the_widest_bay_line_and_footer_fit_the_structure_sheet() {
        let held = IncubationView {
            child: Some(IncubatingChild {
                species: "Quarantine Daemon".into(),
                generation: 12,
                ticks_left: 0,
                held: true,
            }),
        };
        let mut lines = incubation_lines(&[held.clone(), IncubationView { child: None }]);
        lines.push(bay_footer(&[IncubationView { child: None }]).to_string());
        assert_fits(&lines, PopupSize::Small);
    }

    #[test]
    fn a_full_bay_does_not_offer_breeding() {
        let full = IncubationView {
            child: Some(IncubatingChild {
                species: "Worm".into(),
                generation: 1,
                ticks_left: 5,
                held: false,
            }),
        };
        assert_eq!(bay_footer(std::slice::from_ref(&full)), "Any key to close.");
        assert!(bay_footer(&[full, IncubationView { child: None }]).starts_with("[B]"));
    }

    #[test]
    fn a_bay_line_says_what_is_in_the_slot_and_never_a_tick() {
        let lines = incubation_lines(&[
            IncubationView {
                child: Some(IncubatingChild {
                    species: "Trojan".into(),
                    generation: 2,
                    ticks_left: 7,
                    held: false,
                }),
            },
            IncubationView { child: None },
        ]);
        assert_eq!(lines[0], "  Incubating: Trojan, gen 2 - ready soon");
        assert_eq!(lines[1], "  Empty");
    }
}
