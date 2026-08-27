//! The Develop screen: a program's Kernel Rings, and the talents the levels
//! they bought pay for.
//!
//! Two pages — pick a program, then the one page that spends on it. The ring
//! block and the talent ladder share that second page deliberately: opening a
//! ring and spending the point it earns are the same decision loop, and
//! splitting them would make the player back out to see what they just bought.

use super::popup::*;
use super::*;
use feral_processes_engine::TalentOption;

/// Page one: which program.
pub(super) fn draw_develop(
    game: &mut Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
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
    draw_popup("Develop", PopupSize::Large, &rows, refusal, painter, m);
}

/// Page two: what this program has, and what can be spent on it.
///
/// The ring block is one block on the page rather than the whole page, so the
/// talent ladder can join it below without moving anything the player has
/// already learned where to look for.
pub(super) fn draw_develop_program(
    game: &mut Game,
    target: Option<Entity>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(target) = target else {
        return;
    };
    let Some(subject) = game.owned_pets().into_iter().find(|p| p.entity == target) else {
        return;
    };
    let cap = game.level_cap();
    let held = game.privilege_rings_held();

    let points = game.talent_points(target);
    let options = game.talent_options(target);

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
        rows.push(item_row("[R] Open the next kernel ring", false));
    }

    rows.push(text_row(""));
    rows.push(text_row(format!(
        "Talent points: {} unspent of {} earned.",
        points.unspent(),
        points.earned
    )));
    if points.earned == 0 {
        rows.push(text_row(
            "Levels earned past the ceiling pay for talents — go and earn one.",
        ));
    }
    // The ladder, tier by tier, so what has been bought and what is still out
    // of reach read as one shape. The numbered rows are exactly the ones
    // `handle_develop_program_key` offers.
    let mut shortcut = 0;
    let mut tier_shown = 0;
    for option in &options {
        if option.tier != tier_shown {
            tier_shown = option.tier;
            rows.push(text_row(format!("Tier {tier_shown}")));
        }
        if option.taken {
            rows.push(text_row(format!(
                "  * {} — {}",
                option.name, option.description
            )));
        } else if tier_is_next(&options, option.tier) {
            // Numbered even when the point is not there yet, because the row
            // still resolves to a key and `take_talent` is what says no —
            // greying it out silently would leave the player pressing a
            // number and reading nothing.
            let row = item_row(
                format!(
                    "  [{}] {} ({}) — {}",
                    menu_shortcut(shortcut),
                    option.name,
                    option.tag,
                    option.description
                ),
                shortcut == selected,
            );
            shortcut += 1;
            rows.push(row);
        } else {
            rows.push(text_row(format!("  - {} ({})", option.name, option.tag)));
        }
    }
    draw_popup("Develop", PopupSize::Large, &rows, refusal, painter, m);
}

/// Whether `tier` is the one a point would be spent in next — the first with
/// nothing taken in it. The same rule `handle_develop_program_key` numbers its
/// rows by, so the shortcut a row shows is the shortcut that buys it.
fn tier_is_next(options: &[TalentOption], tier: u32) -> bool {
    options
        .iter()
        .map(|o| o.tier)
        .find(|t| !options.iter().any(|o| o.tier == *t && o.taken))
        == Some(tier)
}
