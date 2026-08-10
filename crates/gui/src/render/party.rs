//! The companion roster and the three-page fusion flow.

use super::popup::*;
use super::*;

/// The help lines above the roster.
///
/// **They deliberately say nothing about wielding a program as a weapon.**
/// That command is an easter egg: it is reachable, it changes the run, and
/// nothing in the game's text points at it. Extracted into a const so
/// `the_companion_screen_never_advertises_the_hidden_key` can read them —
/// a later helpful edit then fails a test rather than quietly spoiling it.
fn companion_help() -> [String; 3] {
    [
        format!(
            "Pick a program to add to your party (max {MAX_PARTY_SIZE}) - select a party member's own number to stand it down."
        ),
        "< and > move the highlighted member along the battle line; the front slot draws the most fire."
            .to_string(),
        "N renames the highlighted program; clear the name to go back to its species."
            .to_string(),
    ]
}

pub(super) fn draw_companion_menu(
    game: &mut Game,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let pets = game.owned_pets();
    let mut rows: Vec<_> = companion_help().into_iter().map(text_row).collect();
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
        // No row colour of its own: `fusion_row` already loses to CRITICAL
        // below, and a third meaning on that axis makes all three unreadable.
        let wielded = if p.wielded { " (WEP)" } else { "" };
        let critical = hp_critical(p.hp, p.max_hp);
        let text = format!(
            "[{}] {slot}{} Lv{} - HP {}/{}  PWR {}{}{}{}{}{}",
            menu_shortcut(i),
            p.name,
            p.level,
            p.hp,
            p.max_hp,
            p.power,
            quality,
            fused,
            wielded,
            activity,
            if critical { " - CRITICAL" } else { "" }
        );
        // CRITICAL outranks the fusion colour: one is a state to act on
        // this turn, the other a permanent property to read at leisure.
        rows.push(with_icon(
            if critical {
                critical_item_row(text, i == selected)
            } else {
                fusion_row(text, i == selected, p.fusions)
            },
            p.glyph,
            glyph_color(p.color),
        ));
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
        rows.push(with_icon(
            fusion_row(
                fuse_candidate_label(menu_shortcut(i), p),
                i == selected,
                p.fusions,
            ),
            p.glyph,
            glyph_color(p.color),
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
        rows.push(with_icon(
            fusion_row(
                fuse_candidate_label(menu_shortcut(i), p),
                i == selected,
                p.fusions,
            ),
            p.glyph,
            glyph_color(p.color),
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

/// The rename page's two footer lines, extracted so
/// `the_rename_footer_fits_its_popup` can measure them — this page is
/// `PopupSize::Small`, which is half the window, and these are the widest
/// thing on it by a wide margin.
fn rename_help() -> [&'static str; 2] {
    [
        "Enter to confirm; clear the field to go back to the species name",
        "Esc to leave the name as it is",
    ]
}

/// Free-text naming page for a program already on the roster, opened with
/// `N`. Seeded with the name it already carries, so the field is empty only
/// when the player has emptied it — which is what clears the name.
pub(super) fn draw_rename_menu(
    game: &mut Game,
    target: Option<Entity>,
    name_input: &str,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(target) = target else {
        return;
    };
    let pets = game.owned_pets();
    let subject = pets
        .iter()
        .find(|p| p.entity == target)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "it".to_string());
    let mut rows = vec![
        text_row(format!("Renaming {subject}.")),
        text_row(""),
        item_row(
            format!(
                "Name ({} max): {name_input}",
                feral_processes_engine::MAX_CUSTOM_NAME_LEN
            ),
            true,
        ),
        text_row(""),
    ];
    rows.extend(rename_help().into_iter().map(text_row));
    draw_popup("Rename", PopupSize::Small, &rows, painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;

    /// `draw_row` clamps a row vertically and nothing clamps it
    /// horizontally, so a footer wider than its popup silently runs off the
    /// right edge. The rename page is `PopupSize::Small` — half the window,
    /// where the fusion pages that inspired it are `Large` — so its footers
    /// have meaningfully less room than they look like they do.
    #[test]
    fn the_rename_footer_fits_its_popup() {
        with_painter(|p| {
            let m = ui_metrics(900.0);
            // 0.5 is `PopupSize::Small`'s width fraction; 1440x900 is the
            // geometry `ui_metrics` is calibrated against.
            let room = 1440.0 * 0.5 - m.pad * 2.0;
            for line in rename_help() {
                let drawn = p.measure_ui_advance(line, m.font_size);
                assert!(
                    drawn <= room,
                    "the rename footer overflows its popup by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
        })
    }

    /// The easter-egg census. Wielding a program as your weapon is
    /// deliberately undocumented in-game, and the help lines above the
    /// roster are the one place a well-meaning edit would give it away.
    #[test]
    fn the_companion_screen_never_advertises_the_hidden_key() {
        for line in companion_help() {
            let lower = line.to_lowercase();
            assert!(
                !lower.contains("weapon"),
                "the help must not name what the key does: {line:?}"
            );
            assert!(!lower.contains("wield"), "nor the verb for it: {line:?}");
            assert!(
                !line.contains('W'),
                "nor press the key in front of the player: {line:?}"
            );
        }
    }
}
