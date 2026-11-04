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
    let at = |min, max| tactical_routine_label(&option(Some(AbilityRange { min, max })));
    assert_eq!(at(0, 4), "Null Route — range 4 — Damage one");
    assert_eq!(at(2, 5), "Null Route — range 2–5 — Damage one");
    assert_eq!(at(0, 0), "Null Route — self — Damage one");
    assert_eq!(
        tactical_routine_label(&option(None)),
        "Null Route — Damage one"
    );
}

/// The range sits beside the name so it is drawn whatever a long
/// description does to the row's tail: every shipped routine's name and
/// range fit the picker at the smallest window.
#[test]
fn every_shipped_routine_s_name_and_range_fit_the_picker_at_1280x720() {
    use crate::render::popup::{popup_body_width, row_label_text};
    use feral_processes_engine::abilities::AbilityDb;

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/abilities");
    let (db, warnings) = AbilityDb::load_dir(&dir).expect("the abilities load");
    assert!(warnings.is_empty(), "{warnings:?}");
    let m = ui_metrics(720.0);
    let body = popup_body_width(1280.0, PopupSize::Large, &m);
    with_painter(|p| {
        for def in db.all() {
            let label = tactical_routine_label(&SpecialOption {
                name: def.name.clone(),
                detail: String::new(),
                ..option(Some(def.tactical_range()))
            });
            let text = row_label_text(&item_row(label, true));
            let width = p.measure_ui_advance(&text, m.font_size);
            assert!(
                width <= body,
                "{}'s name and range draw {width}px into a {body}px body: {text:?}",
                def.id
            );
        }
    });
}
