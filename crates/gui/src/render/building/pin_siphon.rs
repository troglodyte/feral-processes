use super::*;

/// `Mode::PinSubject` — modelled on `draw_build_program`'s shape.
///
/// **One row, not two.** `Game::pinned_subject` decides which of two things
/// this popup is showing: with a subject already pinned, the single row
/// that releases it, matching `App::handle_pin_subject_key`'s own read of
/// the same call so the row on screen and the row the key resolves can
/// never disagree; with nobody pinned, `Game::base_staff` as a picker.
pub(crate) fn draw_pin_subject(
    game: &mut Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let rows: Vec<Row> = match game.pinned_subject() {
        Some(subject) => {
            let name = game.creature_label(subject);
            vec![item_row(
                format!("Release {name} from study"),
                selected == 0,
            )]
        }
        None => {
            let staff = game.base_staff();
            if staff.is_empty() {
                vec![text_row("(nobody on the base staff to pin)")]
            } else {
                staff
                    .into_iter()
                    .enumerate()
                    .map(|(i, entity)| {
                        item_row(game.creature_label(entity).to_string(), i == selected)
                    })
                    .collect()
            }
        }
    };
    draw_popup(
        "Study a program",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

/// `Mode::PinPrisoner` — one row per downed-program record, `draw_pin_subject`'s
/// picker shape. A record `Game::jail_blocker` refuses is shown greyed with
/// the blocker's own fragment after it, rather than hidden, so the player can
/// read what to fix (stand beside a free cell, forge a protocol).
pub(crate) fn draw_pin_prisoner(
    game: &mut Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let rows = pin_prisoner_rows(game, selected);
    draw_popup(
        "Hold a program",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

pub(super) fn pin_prisoner_rows(game: &Game, selected: usize) -> Vec<Row> {
    let records = game.downed_program_rows();
    if records.is_empty() {
        return vec![text_row("(no downed programs to hold)")];
    }
    records
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            let mut label = r.name.to_string();
            if let Some(tier) = r.rarity.label() {
                label.push_str(&format!("  {tier}"));
            }
            match game.jail_blocker(i) {
                Some(block) => colored_item_row(
                    format!("{label}  ({})", block.row_fragment()),
                    i == selected,
                    TEXT_DIM,
                ),
                None => item_row(label, i == selected),
            }
        })
        .collect()
}

/// The rows of `Mode::Siphon`, `draw_pin_subject`'s shape: an occupied
/// siphon is the one row that releases its holder and quotes the price, an
/// empty one is the staff picker. Which of the two is `App::handle_siphon_key`'s
/// own read of `Game::siphon_holder`, so the row drawn is the row resolved.
///
/// The quoted figure comes from `Game::siphon_release_ceiling_percent`, the
/// rule the release itself charges, never a literal here.
pub(super) fn siphon_rows(
    holder: Option<&str>,
    staff: &[String],
    selected: usize,
    left_pct: u32,
) -> Vec<Row> {
    if let Some(name) = holder {
        return vec![item_row(
            format!("Release {name} (it comes out at {left_pct}% Integrity or less)"),
            selected == 0,
        )];
    }
    if staff.is_empty() {
        return vec![text_row("(nobody on the base staff to hold)")];
    }
    staff
        .iter()
        .enumerate()
        .map(|(i, name)| item_row(name.clone(), i == selected))
        .collect()
}

/// `Mode::Siphon`.
pub(crate) fn draw_siphon(
    game: &mut Game,
    siphon: Option<Entity>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let rows = siphon_page_rows(game, siphon, selected);
    draw_popup("Power Siphon", PopupSize::Large, &rows, refusal, painter, m);
}

/// The siphon screen's rows, shared by the popup and the structure hub.
pub(crate) fn siphon_page_rows(
    game: &mut Game,
    siphon: Option<Entity>,
    selected: usize,
) -> Vec<Row> {
    let holder = siphon
        .and_then(|s| game.siphon_holder(s))
        .map(|e| game.creature_label(e));
    let staff: Vec<String> = if holder.is_some() {
        Vec::new()
    } else {
        game.base_staff()
            .into_iter()
            .map(|e| game.creature_label(e))
            .collect()
    };
    siphon_rows(
        holder.as_deref(),
        &staff,
        selected,
        Game::siphon_release_ceiling_percent(),
    )
}
