//! The perk and research pickers.

use super::popup::*;
use super::*;
use feral_processes_engine::ResearchStatus;
use feral_processes_engine::perks::{Perk, PerkDef};

/// What a description is indented by under the entry it belongs to. Four
/// columns rather than `continuation_lines`' seven: that indent is a fact
/// about `with_icon`'s glyph slot, and neither picker's rows carry an icon.
const DESCRIPTION_INDENT: &str = "    ";

/// One entry's description, wrapped to the popup and indented under the row
/// it belongs to. Both pickers print prose the same way, and the shipped
/// assets carry up to about 240 characters of it against a
/// `PopupSize::Large` body of roughly 114 — printed raw it ran off the
/// right edge in silence, since `draw_row` clamps a row vertically and
/// nothing clamps it horizontally.
///
/// `DESCRIBE_WRAP_COLUMNS` rather than `ROW_WRAP_COLUMNS` because this is
/// prose, which reads better narrow than wide — the same width the Recipes
/// screen wraps a product's description to, so the two screens cannot
/// disagree about how wide the game's prose runs.
///
/// The lines stay `Row::Item`, which is what `perks_menu_rows` documents as
/// load-bearing: `popup_layout` cuts the scrollable body at the last
/// `Row::Item`, so a description made of `Row::Text` would be torn off the
/// entry it describes and pinned to the foot of the box.
fn description_rows(description: &str) -> impl Iterator<Item = Row> + '_ {
    wrap_text(
        description,
        DESCRIBE_WRAP_COLUMNS - DESCRIPTION_INDENT.chars().count(),
    )
    .into_iter()
    .map(|line| colored_item_row(format!("{DESCRIPTION_INDENT}{line}"), false, TEXT_DIM))
}

/// The perk picker's rows. A perk's description is a *dim item row* rather
/// than a `Row::Text`, and the help line sits in the header rather than under
/// the list, for the reason `build_menu_rows` does the same: `popup_layout`
/// cuts the scrollable body at the last `Row::Item` and pins everything after
/// it, so a trailing `Row::Text` description is torn off the perk it belongs
/// to and drawn at the foot of the box, where it neither scrolls nor sits
/// under its own row. See `every_progression_description_stays_inside_the_
/// scrollable_body`.
pub(super) fn perks_menu_rows(
    points: u32,
    defs: &[PerkDef],
    held: &[Perk],
    selected: usize,
) -> Vec<Row> {
    let mut rows = vec![
        Row::TextColored(format!("Perk Points: {points}"), CYAN),
        text_row("Pick a row's key to buy another level. Esc to close"),
        text_row(""),
    ];
    for (i, def) in defs.iter().enumerate() {
        let level = held.iter().filter(|p| **p == def.id).count();
        let tag = if level > 0 {
            format!(" (level {level})")
        } else {
            String::new()
        };
        rows.push(item_row(
            format!(
                "[{}] {} - {} Perk Points{}",
                menu_shortcut(i),
                def.name,
                def.cost,
                tag
            ),
            i == selected,
        ));
        rows.extend(description_rows(&def.description));
    }
    rows
}

pub(super) fn draw_perks_menu(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let status = game.player_status();
    let rows = perks_menu_rows(
        status.perk_points,
        &game.perk_defs(),
        &status.unlocked_perks,
        selected,
    );
    draw_popup("Perks", PopupSize::Large, &rows, painter, m);
}

/// The two colours a locked research row can take, dimmed well below `TEXT`
/// so the rows a player *can* pick stay the loudest thing on the screen —
/// full-strength `ORANGE` and `BLUE` would make the unbuyable rows the ones
/// that catch the eye, which is the inversion this screen started with.
///
/// Hue rather than brightness carries which wall a row is behind, because
/// two dim neutrals a shade apart are indistinguishable in a line of text:
/// amber is a wall you clear inside the tree, blue one you clear by
/// breaching. Both stay clear of the dim grey an already-researched row
/// takes, so the three unpickable states never read as each other.
const LOCKED_BY_PREREQ: Color = Color::new(0.72, 0.50, 0.22, 1.0);
const LOCKED_BY_ZONE: Color = Color::new(0.38, 0.52, 0.78, 1.0);

/// What a research row says about itself after its name and price. This is
/// the only place a locked node is labelled, so a node held up by both a
/// prerequisite and a breach has to read as held up by both — the engine
/// reports the two reasons separately (`ResearchState::Locked`) precisely so
/// that neither is dropped here, and joining them is this function's whole
/// job.
///
/// A function rather than an inline `match` for the reason
/// `render/party.rs::companion_help` is one: it is the only way a test can
/// hold the string without standing a window up.
fn state_tag(state: &ResearchState) -> String {
    match state {
        ResearchState::Unlocked => " (researched)".to_string(),
        ResearchState::Available => String::new(),
        ResearchState::Locked { missing, min_zone } => {
            let mut reasons = missing.clone();
            reasons.extend(min_zone.map(|z| format!("Zone {z}")));
            format!(" (needs {})", reasons.join(", "))
        }
    }
}

/// What colour a research row is drawn in — the whole of what a player
/// scanning the list learns before reading a single tag.
///
/// Five states on one axis. The two a player can act on are the loudest —
/// green for a node the tree recommends taking next, plain `TEXT` for any
/// other available one — and the three they cannot are quieter, told apart
/// by hue rather than by shade. `recommended` is deliberately read **only** on the
/// `Available` arm — a recommended node still behind a prerequisite is not
/// somewhere the player can go, and painting it green would send them at a
/// row that refuses them. Its prerequisite is green instead, which is what
/// `ResearchDb::recommended_ids`'s closure exists to guarantee: the green
/// row is always one you can actually buy.
///
/// A locked node names both of its reasons in `state_tag` but has only one
/// colour, so the harder wall wins: a breach is something the whole run has
/// to do, a prerequisite something this screen can do next.
fn row_color(node: &ResearchStatus) -> Color {
    match &node.state {
        ResearchState::Unlocked => TEXT_DIM,
        ResearchState::Available if node.recommended => GREEN,
        ResearchState::Available => TEXT,
        ResearchState::Locked { min_zone, .. } if min_zone.is_some() => LOCKED_BY_ZONE,
        ResearchState::Locked { .. } => LOCKED_BY_PREREQ,
    }
}

/// The research picker's rows, in the shape `perks_menu_rows` documents and
/// for the same reason: nothing may follow the last `Row::Item`.
pub(super) fn research_menu_rows(held: u32, nodes: &[ResearchStatus], selected: usize) -> Vec<Row> {
    let mut rows = vec![
        Row::TextColored(format!("Research Data: {held}"), CYAN),
        text_row("Pick a row's key to research it. Esc to close"),
        text_row(""),
    ];
    for (i, node) in nodes.iter().enumerate() {
        let tag = state_tag(&node.state);
        let label = format!(
            "[{}] {} - {} Research Data{tag}",
            menu_shortcut(i),
            node.name,
            node.cost
        );
        rows.push(colored_item_row(label, i == selected, row_color(node)));
        rows.extend(description_rows(&node.description));
    }
    rows
}

pub(super) fn draw_research_menu(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let research_currency = game.research_currency();
    let held = game.banked(&research_currency);
    let nodes = game.research_nodes();
    let rows = research_menu_rows(held, &nodes, selected);
    draw_popup("Research", PopupSize::Large, &rows, painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use feral_processes_engine::DifficultyMode;

    /// The three questions a playtest of this screen would ask, answered
    /// without standing a window up: a gated node explains itself, a
    /// doubly-held node names both reasons, and an ungated locked node reads
    /// exactly as it did before the gate existed.
    #[test]
    fn a_locked_row_names_every_reason_it_is_locked() {
        assert_eq!(
            state_tag(&ResearchState::Locked {
                missing: Vec::new(),
                min_zone: Some(3),
            }),
            " (needs Zone 3)",
            "a node whose only obstacle is the breach has to say so — this is \
             the row that tells a player the tier is worth breaching for"
        );
        assert_eq!(
            state_tag(&ResearchState::Locked {
                missing: vec!["Neural Interfacing".to_string()],
                min_zone: Some(3),
            }),
            " (needs Neural Interfacing, Zone 3)",
            "both reasons, or clearing one leaves the row saying the same thing"
        );
        assert_eq!(
            state_tag(&ResearchState::Locked {
                missing: vec!["Automation".to_string()],
                min_zone: None,
            }),
            " (needs Automation)",
            "a bootstrap node is untouched by the gate existing"
        );
    }

    #[test]
    fn an_unlocked_row_reads_as_spent_and_an_available_one_says_nothing() {
        assert_eq!(state_tag(&ResearchState::Unlocked), " (researched)");
        assert_eq!(state_tag(&ResearchState::Available), "");
    }

    /// The five states have to reach the *colour* and not only the tag: a
    /// player scanning the list picks a bright row without reading it. What
    /// this pins is the ordering of attention — green is takeable now, white
    /// is takeable, the two walls are quieter than both, and spent is
    /// quietest — plus the one case a colour has to arbitrate, a node held
    /// by a prerequisite and a breach at once.
    #[test]
    fn every_research_state_is_drawn_in_its_own_colour() {
        let node = |state: ResearchState, recommended: bool| ResearchStatus {
            id: "n".to_string(),
            name: "N".to_string(),
            description: String::new(),
            cost: 10,
            state,
            affordable: true,
            recommended,
        };
        let prereq = |missing: &str| ResearchState::Locked {
            missing: vec![missing.to_string()],
            min_zone: None,
        };

        assert_eq!(
            row_color(&node(ResearchState::Available, true)),
            GREEN,
            "the row a run should take next is the one thing on the screen              that has to be findable without reading"
        );
        assert_eq!(row_color(&node(ResearchState::Available, false)), TEXT);
        assert_eq!(
            row_color(&node(prereq("Automation"), false)),
            LOCKED_BY_PREREQ
        );
        assert_eq!(
            row_color(&node(
                ResearchState::Locked {
                    missing: Vec::new(),
                    min_zone: Some(3),
                },
                false,
            )),
            LOCKED_BY_ZONE,
        );
        assert_eq!(row_color(&node(ResearchState::Unlocked, false)), TEXT_DIM);

        assert_eq!(
            row_color(&node(
                ResearchState::Locked {
                    missing: vec!["Automation".to_string()],
                    min_zone: Some(3),
                },
                false,
            )),
            LOCKED_BY_ZONE,
            "held by both walls, the breach wins — it is the one the whole              run has to clear, and `state_tag` still names both"
        );

        assert_eq!(
            row_color(&node(prereq("Power Grid"), true)),
            LOCKED_BY_PREREQ,
            "a recommended node still owing a prerequisite is not somewhere              the player can go; green would send them at a row that refuses              them, and its prerequisite is the green one instead"
        );
    }

    /// The five row colours are only worth having if they survive being read
    /// as text on the popup's dark panel. Distances rather than literals, so
    /// a palette retune is free to move any of them — the same bar
    /// `the_tier_colours_are_separable_from_their_neighbours` holds the
    /// rarity bars to.
    #[test]
    fn the_research_row_colours_are_separable_from_one_another() {
        let dist = |a: Color, b: Color| (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs();
        let palette = [
            ("GREEN", GREEN),
            ("TEXT", TEXT),
            ("LOCKED_BY_PREREQ", LOCKED_BY_PREREQ),
            ("LOCKED_BY_ZONE", LOCKED_BY_ZONE),
            ("TEXT_DIM", TEXT_DIM),
        ];
        for (i, (name, color)) in palette.iter().enumerate() {
            for (other_name, other) in palette.iter().skip(i + 1) {
                assert!(
                    dist(*color, *other) > 0.25,
                    "{name} is only {:.2} from {other_name} — two research                      rows a player has to tell apart would read the same",
                    dist(*color, *other)
                );
            }
        }
        // A locked row must also be visibly quieter than the two rows that
        // can be picked, or the unbuyable half of the tree is what draws the
        // eye. Perceptual luminance rather than a channel sum: the eye is
        // roughly ten times more sensitive to green than to blue, so a plain
        // `r + g + b` calls this blue brighter than `GREEN` and would have
        // waved a genuinely too-loud colour through in the other direction.
        //
        // Nothing is claimed about a locked row against a *spent* one. Those
        // are separated by hue against neutral, which no single scalar
        // orders, and the tag says which is which regardless.
        let luminance = |c: Color| 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
        for (name, color) in [
            ("LOCKED_BY_PREREQ", LOCKED_BY_PREREQ),
            ("LOCKED_BY_ZONE", LOCKED_BY_ZONE),
        ] {
            assert!(
                luminance(color) < luminance(TEXT) && luminance(color) < luminance(GREEN),
                "{name} outshines a row the player can actually take"
            );
        }
    }

    /// `draw_row` clamps a row vertically and nothing clamps it
    /// horizontally, so a row wider than its popup runs off the right edge
    /// in silence. Both pickers print a description under every entry, and
    /// the prose the assets carry runs to about 240 characters against a
    /// `PopupSize::Large` body of roughly 114 — so the tail of most of them
    /// was drawn outside the box.
    ///
    /// Measured against the real shipped assets rather than a fixture, for
    /// `the_widest_recipe_row_fits_the_popup_it_is_drawn_in`'s reason: what
    /// sets the width here is whichever node or perk carries the longest
    /// prose, and a fixture would go stale the first time one moved.
    /// `with_painter`'s 1440x900 is the geometry `ui_metrics` is calibrated
    /// against, so the font here is the unscaled body size.
    #[test]
    fn the_widest_progression_row_fits_the_popup_it_is_drawn_in() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(7, DifficultyMode::Forgiving, assets).expect("shipped assets load");
        let nodes = game.research_nodes();
        let defs = game.perk_defs();
        let status = game.player_status();
        assert!(
            !nodes.is_empty() && !defs.is_empty(),
            "the shipped assets declare both a research tree and a perk list"
        );

        let screens = [
            ("Research", research_menu_rows(40, &nodes, 0)),
            (
                "Perks",
                perks_menu_rows(3, &defs, &status.unlocked_perks, 0),
            ),
        ];

        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for (screen, rows) in &screens {
                let mut measured = 0usize;
                for row in rows {
                    // `draw_row` prefixes every `Row::Item` with two columns
                    // of its own for the selection caret, which are as much
                    // of the drawn line as the text is.
                    let drawn_text = match row {
                        Row::Text(text) => text.clone(),
                        Row::TextColored(text, _) => text.clone(),
                        Row::Item { text, .. } => format!("  {text}"),
                    };
                    measured += 1;
                    let drawn = p.measure_ui_advance(&drawn_text, m.font_size);
                    assert!(
                        drawn <= room,
                        "the {screen} picker overflows its popup by {:.0}px \
                         ({drawn:.0} drawn into {room:.0} of room):\n{drawn_text}",
                        drawn - room
                    );
                }
                assert!(measured > 0, "the {screen} picker drew no rows to measure");
            }
        });
    }
}
