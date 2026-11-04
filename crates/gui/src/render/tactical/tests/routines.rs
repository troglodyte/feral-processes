use super::*;
use feral_processes_engine::battle::SpecialTargeting;

fn option(range: Option<AbilityRange>) -> SpecialOption {
    SpecialOption {
        index: 0,
        name: "Null Route".to_string(),
        detail: "Damage one".to_string(),
        targeting: SpecialTargeting::Enemy,
        sweeps_party: false,
        unavailable: None,
        cooldown: 0,
        provokes: 0,
        range,
    }
}

/// The picker says how far each routine reaches, beside its name, so the
/// player chooses before the aim cursor tells them it falls short.
#[test]
fn a_routine_row_says_how_far_it_reaches() {
    let head =
        |min, max| tactical_routine_lines(&option(Some(AbilityRange { min, max })))[0].clone();
    assert_eq!(head(0, 4), "Null Route — range 4");
    assert_eq!(head(2, 5), "Null Route — range 2–5");
    assert_eq!(head(0, 0), "Null Route — self");
    assert_eq!(tactical_routine_lines(&option(None))[0], "Null Route");
}

/// The description sits under the row it belongs to, and a refusal stays on
/// the row itself, where the greyed name already says something is wrong.
#[test]
fn a_routine_s_description_goes_under_it_and_its_refusal_beside_it() {
    let lines = tactical_routine_lines(&SpecialOption {
        unavailable: Some("needs 4 Power".to_string()),
        ..option(Some(AbilityRange { min: 0, max: 4 }))
    });
    assert_eq!(lines[0], "Null Route — range 4 — needs 4 Power");
    assert_eq!(lines[1..], continuation_lines("Damage one")[..]);
}

/// Every line of every shipped routine fits the picker at the smallest
/// window — a popup row is never clipped or wrapped, so a line that does not
/// fit runs off the box in silence.
#[test]
fn every_shipped_routine_fits_the_picker_at_1280x720() {
    use crate::render::popup::{popup_body_width, row_label_text};
    use feral_processes_engine::abilities::AbilityDb;

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/abilities");
    let (db, warnings) = AbilityDb::load_dir(&dir).expect("the abilities load");
    assert!(warnings.is_empty(), "{warnings:?}");
    let m = ui_metrics(720.0);
    let body = popup_body_width(1280.0, PopupSize::Large, &m);
    with_painter(|p| {
        for def in db.all() {
            let lines = tactical_routine_lines(&SpecialOption {
                name: def.name.clone(),
                detail: def.description.clone(),
                ..option(Some(def.tactical_range()))
            });
            for line in lines {
                let text = row_label_text(&item_row(line, true));
                let width = p.measure_ui_advance(&text, m.font_size);
                assert!(
                    width <= body,
                    "{}'s line draws {width}px into a {body}px body: {text:?}",
                    def.id
                );
            }
        }
    });
}
