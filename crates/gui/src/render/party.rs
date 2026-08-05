//! The companion roster and the three-page fusion flow.

use super::popup::*;
use super::*;

pub(super) fn draw_companion_menu(
    game: &mut Game,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let pets = game.owned_pets();
    let mut rows = vec![
        text_row(format!(
            "Pick a program to add to your party (max {MAX_PARTY_SIZE}) - select a party member's own number to stand it down."
        )),
        text_row(
            "< and > move the highlighted member along the battle line; the front slot draws the most fire.",
        ),
    ];
    if pets.is_empty() {
        rows.push(text_row("(you don't have any compiled programs yet)"));
    }
    for (i, p) in pets.iter().enumerate() {
        let slot = p
            .party_slot
            .map(|s| format!("#{} ", s + 1))
            .unwrap_or_default();
        let activity = activity_tag(&p.activity);
        let quality = p
            .quality
            .as_ref()
            .map(|q| format!(" [{q}]"))
            .unwrap_or_default();
        let fused = fusion_tag(p.fusions);
        let critical = hp_critical(p.hp, p.max_hp);
        let text = format!(
            "[{}] {slot}{} Lv{} - HP {}/{}  PWR {}{}{}{}{}",
            menu_shortcut(i),
            p.name,
            p.level,
            p.hp,
            p.max_hp,
            p.power,
            quality,
            fused,
            activity,
            if critical { " - CRITICAL" } else { "" }
        );
        // CRITICAL outranks the fusion colour: one is a state to act on
        // this turn, the other a permanent property to read at leisure.
        rows.push(if critical {
            critical_item_row(text, i == selected)
        } else {
            fusion_row(text, i == selected, p.fusions)
        });
    }
    draw_popup("Party", PopupSize::Large, &rows, painter, m);
}

/// Formats one fuse-candidate row with the full stat line a fusion
/// decision depends on.
fn fuse_candidate_label(num: char, p: &PetInfo) -> String {
    let fused = fusion_tag(p.fusions);
    let activity = activity_tag(&p.activity);
    format!(
        "[{num}] {} Lv{} - HP {}/{}  ATK {}  DEF {}  PWR {}{fused}{activity}",
        p.name, p.level, p.hp, p.max_hp, p.atk, p.def, p.power
    )
}

pub(super) fn draw_fuse_menu(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let candidates = game.owned_pets();
    let mut rows = vec![text_row("Fuse which program? Pick the first of two.")];
    if candidates.is_empty() {
        rows.push(text_row("(you have no compiled programs)"));
    }
    for (i, p) in candidates.iter().enumerate() {
        rows.push(fusion_row(
            fuse_candidate_label(menu_shortcut(i), p),
            i == selected,
            p.fusions,
        ));
    }
    draw_popup("Fuse", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_fuse_second_menu(
    game: &mut Game,
    first: Option<Entity>,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(first) = first else { return };
    let pets = game.owned_pets();
    let first_label = pets
        .iter()
        .find(|p| p.entity == first)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "it".to_string());
    let candidates: Vec<_> = pets.into_iter().filter(|p| p.entity != first).collect();
    let mut rows = vec![text_row(format!(
        "Fuse {first_label} with which program? Both are consumed."
    ))];
    if candidates.is_empty() {
        rows.push(text_row("(you have no other compiled programs)"));
    }
    for (i, p) in candidates.iter().enumerate() {
        rows.push(fusion_row(
            fuse_candidate_label(menu_shortcut(i), p),
            i == selected,
            p.fusions,
        ));
    }
    draw_popup("Fuse", PopupSize::Large, &rows, painter, m);
}

/// Free-text naming page shown after both fuse candidates are picked.
/// Blank and Enter keeps the default species name.
pub(super) fn draw_fuse_name_menu(
    game: &mut Game,
    first: Option<Entity>,
    second: Option<Entity>,
    name_input: &str,
    painter: &Painter,
    m: &Metrics,
) {
    let (Some(first), Some(second)) = (first, second) else {
        return;
    };
    let pets = game.owned_pets();
    let label_of = |e: Entity| {
        pets.iter()
            .find(|p| p.entity == e)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "it".to_string())
    };
    let mut rows = vec![
        text_row(format!(
            "Fusing {} and {}.",
            label_of(first),
            label_of(second)
        )),
        text_row(""),
        item_row(
            format!(
                "Name it (optional, {} max): {name_input}",
                feral_processes_engine::MAX_CUSTOM_NAME_LEN
            ),
            true,
        ),
        text_row(""),
    ];
    // The result's kit is derived fresh from its species, not merged from
    // its parents' — anything installed manually on either one (research,
    // extraction, a swap) does not survive, so this is the last screen where
    // backing out with Esc still saves it.
    let losses = game.fusion_routine_losses(first, second);
    if !losses.is_empty() {
        let names: Vec<&str> = losses.iter().map(|a| a.name.as_str()).collect();
        rows.push(text_row(format!(
            "This may lose: {} (anything not innate to the result).",
            names.join(", ")
        )));
        rows.push(text_row(""));
    }
    rows.push(text_row(
        "Type a name, Enter to fuse (blank keeps the default name)",
    ));
    rows.push(text_row("Esc to go back and re-pick the second program"));
    draw_popup("Fuse", PopupSize::Small, &rows, painter, m);
}
