//! The Develop screen: a program's Kernel Rings, and the talents the levels
//! they bought pay for.
//!
//! Two pages — pick a program, then the one page that spends on it. The ring
//! block and the talent ladder share that second page deliberately: opening a
//! ring and spending the point it earns are the same decision loop, and
//! splitting them would make the player back out to see what they just bought.

use super::popup::*;
use super::*;

/// Page one: which program.
pub(super) fn draw_develop(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let programs = game.owned_pets();
    let held = game.privilege_rings_held();
    let mut rows = vec![
        text_row("Develop which program? A ring raises its ceiling; the levels are still earned."),
        text_row(format!("Privilege Rings in cargo: {held}.")),
    ];
    for (i, p) in programs.iter().enumerate() {
        rows.push(with_icon(
            tier_row(
                format!(
                    "[{}] {} Lv{}  rings {}/{}",
                    menu_shortcut(i),
                    p.name,
                    p.level,
                    p.ring,
                    KERNEL_RING_MAX
                ),
                i == selected,
                p.fusions,
                p.rarity,
            ),
            p.glyph,
            glyph_color(p.color),
        ));
    }
    draw_popup("Develop", PopupSize::Large, &rows, painter, m);
}

/// Page two: what this program has, and what can be spent on it.
///
/// The ring block is one block on the page rather than the whole page, so the
/// talent ladder can join it below without moving anything the player has
/// already learned where to look for.
pub(super) fn draw_develop_program(
    game: &mut Game,
    target: Option<Entity>,
    _selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(target) = target else {
        return;
    };
    let Some(subject) = game.owned_pets().into_iter().find(|p| p.entity == target) else {
        return;
    };
    let cap = game.companion_level_cap(target);
    let held = game.privilege_rings_held();

    let mut rows = vec![
        text_row(format!("Developing {}.", subject.name)),
        text_row(format!("Level {} of {cap}.", subject.level)),
        text_row(""),
        text_row(format!(
            "Kernel rings: {}/{KERNEL_RING_MAX} open.",
            subject.ring
        )),
    ];
    if subject.ring >= KERNEL_RING_MAX {
        rows.push(text_row(
            "Every ring is open — this program can go no wider.",
        ));
    } else {
        let cost = Game::ring_cost(subject.ring);
        rows.push(text_row(format!(
            "Ring {} costs {cost} Privilege Rings; you hold {held}.",
            subject.ring + 1
        )));
        rows.push(item_row("[R] Open the next kernel ring", true));
    }
    draw_popup("Develop", PopupSize::Large, &rows, painter, m);
}
