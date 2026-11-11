//! A hub page draws no key footer of its own: the hub's key bar is the one
//! place keys are explained. A popup keeps its footer.

use super::refusals::ALL_MODES;
use super::*;
use crate::paint::{painted_text, with_painter};
use crate::render::popup::DROPPED_KEY_HINTS;
use feral_processes_app_core::{App, KEY_HINT_SEP};

fn dropped_after_draw(app: &mut App, fx: &mut Fx) -> (Vec<String>, Vec<String>) {
    DROPPED_KEY_HINTS.with(|d| d.borrow_mut().clear());
    let (_, shapes) = with_painter(|p| draw(app, fx, p, false, false));
    let dropped = DROPPED_KEY_HINTS.with(|d| d.borrow().clone());
    (painted_text(&shapes), dropped)
}

/// Every hub screen, drawn on its page, in the states that change its rows.
fn hub_cases() -> Vec<(App, Mode)> {
    let mut cases = Vec::new();
    for mode in ALL_MODES {
        let mut app = census_app();
        app.mode = mode;
        app.settle_hub();
        if base_report::HubFrame::of(&app, mode).is_some() {
            cases.push((app, mode));
        }
    }
    // Deploy with a category open is a second set of rows.
    let mut app = census_app();
    let category = feral_processes_app_core::BuildMenu::of(app.game.as_mut().unwrap(), None)
        .rows
        .into_iter()
        .find_map(|r| match r {
            feral_processes_app_core::BuildRow::Category(c) => Some(c),
            _ => None,
        })
        .expect("a fresh run offers a deploy category");
    app.mode = Mode::Build;
    app.settle_hub();
    app.build_category = Some(category);
    cases.push((app, Mode::Build));
    cases
}

#[test]
fn hub_pages_draw_no_key_footer() {
    let mut fx = Fx::new();
    let cases = hub_cases();
    assert!(
        cases.len() > 40,
        "the census reached {} hub pages",
        cases.len()
    );
    for (mut app, mode) in cases {
        let (text, _) = dropped_after_draw(&mut app, &mut fx);
        if let Some(line) = text.iter().find(|t| t.contains(KEY_HINT_SEP)) {
            panic!("{mode:?} draws a key footer on the hub page: {line:?}");
        }
    }
}

/// The leading key of a footer chip as the characters the bar spells it
/// with: `Left/Right` is `←` and `→`, `[A]` is `A`, a modifier is dropped
/// (the arrows it modifies are what the bar has to carry).
fn key_chars(chip: &str) -> Vec<String> {
    let token = chip.split_whitespace().next().unwrap_or_default();
    let token = token
        .trim_start_matches("Shift+")
        .trim_start_matches("Ctrl+");
    // Descriptive chips that stand for a family of keys.
    match chip {
        c if c.starts_with("Type to filter") => return vec!["a-z".to_string()],
        c if c.starts_with("Type digits") => return vec!["0-9".to_string()],
        c if c.starts_with("Backspace") => return vec!["Bksp".to_string()],
        _ => {}
    }
    token
        .split('/')
        .flat_map(|part| match part {
            "Left" => vec!["←".to_string()],
            "Right" => vec!["→".to_string()],
            "Up" => vec!["↑".to_string()],
            "Down" => vec!["↓".to_string()],
            "Arrows" => ["↑", "↓", "←", "→"].map(String::from).to_vec(),
            other => vec![other.trim_matches(['[', ']']).to_string()],
        })
        .collect()
}

#[test]
fn every_dropped_footer_key_is_on_the_key_bar() {
    let mut fx = Fx::new();
    let mut missing = Vec::new();
    for (mut app, mode) in hub_cases() {
        let bar = base_report::HubFrame::of(&app, mode).unwrap().bar_keys();
        let (_, dropped) = dropped_after_draw(&mut app, &mut fx);
        for line in dropped {
            for chip in line.split(KEY_HINT_SEP) {
                let wanted = key_chars(chip);
                // `hjkl` and the like are aliases of the arrows already named.
                if wanted
                    .iter()
                    .all(|k| k == "hjkl" || bar.iter().any(|b| b.contains(k.as_str())))
                {
                    continue;
                }
                missing.push(format!("{mode:?}: {chip:?} (bar: {bar:?})"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "keys the footer named and the bar lacks:\n{}",
        missing.join("\n")
    );
}

/// A popup is not a hub page, so it keeps its own footer.
#[test]
fn popups_keep_their_footer() {
    let mut app = census_app();
    let game = app.game.as_mut().unwrap();
    let m = crate::text::ui_metrics(720.0);
    let (_, shapes) = with_painter(|p| {
        crate::render::building::draw_build_menu(game, None, 0, None, p, &m);
    });
    assert!(
        painted_text(&shapes)
            .iter()
            .any(|t| t == "Enter choose  ·  Esc back"),
        "{:?}",
        painted_text(&shapes)
    );
}
