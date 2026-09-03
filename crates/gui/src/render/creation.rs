//! The character-creation wizard's screen — one popup per `CreationStep`,
//! drawn from `App::creation_rows`.
//!
//! Every step draws through the same `draw_popup` every other menu uses —
//! one bordered box, one `Row` list — so the wizard's chrome, refusal
//! placement and keyboard highlight are the game's existing menu idiom and
//! not a second one invented for onboarding. What is specific to this
//! screen is `build_row` (numbering, and the Icon/Colour steps' per-row
//! icon) and those two steps' and the Summary's preview cell, painted
//! separately after the popup through `draw_look_preview`.
//!
//! **The wizard promises no scroll.** Every other list menu in the game is
//! fine paging a long catalogue with `draw_popup`'s built-in scroll (see
//! `popup::popup_layout`'s `scrolling` flag) — a trade shelf or a deploy
//! list is read a page at a time anyway. A nine-step onboarding flow is
//! not: `the_tallest_creation_step_fits_its_screen` is what holds that
//! promise, at the smallest window the game supports, against the real
//! shipped `assets/`. A class or ability catalogue is moddable and could
//! still grow past what a screen with no scroll can show — see the test's
//! own doc comment.

use super::popup::{
    PopupSize, Row, colored_item_row, description_rows, draw_popup, item_row, popup_rect, text_row,
    with_icon,
};
use super::*;
use feral_processes_app_core::{CreationRow, CreationStep};
use feral_processes_engine::CharacterChoice;
use feral_processes_engine::tuning::{CREATION_PERK_POINTS, CREATION_STAT_POINTS};
#[cfg(test)]
use feral_processes_engine::tuning::{
    MAX_PROFILE_PERK_POINTS, MAX_PROFILE_STARTING_PROGRAMS, MAX_PROFILE_STAT_POINTS,
};

/// The Icon/Colour/Summary preview cell's side, in `Metrics::line_height`
/// units — big enough to read the glyph or sprite clearly, small enough to
/// sit in the popup's top-right corner without crowding the row list under
/// it.
const PREVIEW_CELL_LINES: f32 = 3.0;

pub(super) fn draw_create_character(
    app: &App,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let step = app.creation_step();
    let drawn = step_rows(app, step);
    let title = format!(
        "New Game — {} ({}/{})",
        step.title(),
        step.index() + 1,
        CreationStep::ALL.len()
    );

    // Both halves of the look draw the same cell: splitting the old `Look`
    // step in two must not cost either half its preview, which is the one
    // thing on either screen showing the glyph and the swatch together.
    //
    // The cell reads the popup's own box back from `popup_rect` rather than
    // a second guess at where `draw_popup` put it — the two calls share one
    // derivation of the box's geometry, so a resize can't put the cell
    // outside its border.
    let show_preview = matches!(
        step,
        CreationStep::Icon | CreationStep::Colour | CreationStep::Summary
    );
    let cell = show_preview
        .then(|| preview_cell_rect(popup_rect(PopupSize::Large, &drawn, refusal, painter, m), m));
    draw_popup(&title, PopupSize::Large, &drawn, refusal, painter, m);
    if let Some(cell) = cell {
        draw_look_preview(&previewed_look(app, step), painter, cell, m);
    }
}

/// The look the cell paints: the choice as it stands, with **the
/// highlighted row laid over it** on the two steps that offer one.
///
/// The cursor previews, and that is what pays for splitting the old `Look`
/// step in two. Picking a row now advances off the screen, so a cell that
/// only ever showed the *committed* choice would show the player their new
/// colour on the step after the one they chose it on — the swatch list
/// would be the one screen in the wizard whose own decision it could not
/// show. Reading the highlight instead means the cell answers "what would
/// this one look like" while the cursor is still moving, and Enter merely
/// keeps the answer.
///
/// Built from `App::creation_rows` rather than from `CREATION_ICONS` and
/// `CREATION_COLOURS` directly, so the row the cell reads is the row the
/// list drew and the two cannot index differently.
fn previewed_look(app: &App, step: CreationStep) -> CharacterChoice {
    let mut look = app.creation_choice().clone();
    if matches!(step, CreationStep::Icon | CreationStep::Colour) {
        match app.creation_rows().get(app.menu_selected) {
            Some(CreationRow::Icon { glyph, sprite }) => {
                look.glyph = *glyph;
                look.sprite = sprite.clone();
            }
            Some(CreationRow::Colour { index }) => look.colour = Some(*index),
            _ => {}
        }
    }
    look
}

/// The full `Row` list a step draws: its own rows via `build_row`, a blank
/// line, then its footer. One function for `draw_create_character` and
/// `the_tallest_creation_step_fits_its_screen` to share, so a step that
/// grows a row is a change both the screen and the height census see —
/// two independent copies of this construction is how a census could pass
/// against a row list the screen no longer draws.
fn step_rows(app: &App, step: CreationStep) -> Vec<Row> {
    let selected = app.menu_selected;
    let rows = app.creation_rows();
    let mut drawn: Vec<Row> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| build_row(step, row, i, selected == i))
        .collect();
    if drawn.is_empty() {
        // A step with nothing to offer — an empty `assets/classes/`, say —
        // still draws a row, or the popup would be a blank box the player
        // cannot tell from a broken screen.
        drawn.push(text_row("Nothing to choose here."));
    }
    // The Perks step alone explains the row under the cursor, because the
    // nineteen perk *names* are opaque and their descriptions are far too
    // wide to sit on the rows. One line, so the screen keeps its promise
    // of no scroll — nineteen rows plus this plus the footer is 22 of the
    // 28 `popup_max_rows` allows.
    if step == CreationStep::Perks
        && let Some(CreationRow::Perk { row, .. }) = rows.get(selected)
    {
        drawn.push(text_row(""));
        drawn.push(text_row(row.description.clone()));
    }
    // The Colour step explains itself once a drawing exists — the drawn
    // icon then owns the map tile and the swatch stops meaning what every
    // row above it still promises. `App::creation_colour_note` is the
    // engine's sentence (Task 6); wrapped and indented like the Perks
    // description above it, since it runs well past one row at
    // `PopupSize::Large`.
    if step == CreationStep::Colour
        && let Some(note) = app.creation_colour_note()
    {
        drawn.push(text_row(""));
        drawn.extend(description_rows(&note));
    }
    drawn.push(text_row(""));
    drawn.push(text_row(footer(app, step)));
    drawn
}

/// One `CreationRow` as a drawable menu row: numbered with its shortcut on
/// every step but the three where the cursor is a highlight rather than a
/// pick (`Mode::Transfer`'s rule — a digit is a quantity there, never a row
/// shortcut), and carrying an icon on the Icon and Colour steps' own row
/// kinds.
fn build_row(step: CreationStep, row: &CreationRow, i: usize, selected: bool) -> Row {
    let text = row_line(row);
    let label = match step {
        CreationStep::Profile
        | CreationStep::Kit
        | CreationStep::Points
        | CreationStep::Perks
        | CreationStep::Name
        | CreationStep::Summary => text,
        _ => format!("[{}] {text}", feral_processes_app_core::menu_shortcut(i)),
    };
    // The one Summary row that is a picture: drawn in the swatch the run
    // will actually wear, so the last screen before the run starts shows
    // the look rather than spelling it.
    if let CreationRow::Look { colour, .. } = row {
        return colored_item_row(label, selected, super::player_look_color(*colour));
    }
    let base = item_row(label, selected);
    match row {
        // The icon rows show their own glyph — a preview of the shape on
        // offer, independent of colour, which the combined preview cell
        // covers separately.
        CreationRow::Icon { glyph, .. } => with_icon(base, *glyph, TEXT),
        // A swatch row wears its own colour on an `@` — the player sees
        // every option painted in the hue it would actually be, not just
        // its name.
        CreationRow::Colour { index } => {
            with_icon(base, '@', hud::palette::PLAYER_CHOICES[*index as usize])
        }
        // A solid square for a drawing that exists, an outline for one
        // that does not — the row's own words already say "Your drawing"
        // or "Draw your own…", so this is the glyph-slot preview every
        // other Icon/Colour row carries, not a second place saying it.
        CreationRow::DrawnIcon { drawn: true } => with_icon(base, '■', TEXT),
        CreationRow::DrawnIcon { drawn: false } => with_icon(base, '□', TEXT_DIM),
        _ => base,
    }
}

/// One row as a line of text. Exhaustive on `CreationRow`, `cell_mark`'s
/// rule: a new row kind must be given words rather than falling into a
/// blank line.
fn row_line(row: &CreationRow) -> String {
    match row {
        CreationRow::Earned(line) => line.clone(),
        CreationRow::Difficulty { label, detail, .. } => format!("{label} - {detail}"),
        CreationRow::Class(class) => format!("{} - {}", class.name, class.trade),
        CreationRow::Icon { glyph, sprite } => format!("{glyph}  ({sprite})"),
        CreationRow::DrawnIcon { drawn } => match drawn {
            true => "Your drawing".to_string(),
            false => "Draw your own…".to_string(),
        },
        CreationRow::Colour { index } => format!("Colour {}", index + 1),
        CreationRow::Stat {
            stat,
            spent,
            value,
            cost,
        } => {
            // The bar's width is this axis's own ceiling if the *whole*
            // pool went to it — not what the other axes have already
            // spent, which changes row to row and would make the bar's
            // length itself a second, unlabelled figure to read.
            let width = (CREATION_STAT_POINTS / (*cost).max(1)).max(*spent);
            let bar: String = (0..width)
                .map(|u| if u < *spent { '#' } else { '-' })
                .collect();
            format!(
                "{:<12} {value:>4}  [{bar}] {spent} bought @ {cost}",
                stat.label()
            )
        }
        // No bar, unlike the Stat row above: at 1 Credit an item's own
        // ceiling is the whole allowance, so a bar would be 25 cells wide on
        // most of two dozen rows. The remaining allowance rides the footer
        // instead, where it is read once rather than inferred per row.
        // An untaken row shows no count at all: at two dozen rows a column
        // of `x0` is the loudest thing on the screen and says nothing.
        CreationRow::Item { row, taken } => {
            let held = match taken {
                0 => String::new(),
                n => format!("x{n}"),
            };
            format!("{:<24} {:>2}c   {held}", row.name, row.price)
        }
        // The Kit row's shape, in Perk Points. The description is **not**
        // on the row: the shipped ones run to 117 characters, which drew
        // 291px past the popup body at 1280x720, and a perk name alone
        // says nothing where an item's names a thing you recognise. It
        // goes under the list instead, for whichever row the cursor is on
        // — see `step_rows`.
        CreationRow::Perk { row, taken } => {
            let held = match taken {
                0 => String::new(),
                n => format!("x{n}"),
            };
            format!("{:<24} {:>2}p   {held}", row.name, row.cost)
        }
        CreationRow::Routine(routine) => {
            format!(
                "{} - {} ({:.0} Power)",
                routine.name, routine.effect, routine.power_cost
            )
        }
        CreationRow::Look { label, glyph, .. } => format!("{label:<12} {glyph}"),
        CreationRow::Name { typed } => format!("Name: {typed}_"),
        CreationRow::Summary { label, value } => format!("{label:<12} {value}"),
    }
}

/// What each step's keys are, in one line under its rows.
///
/// Takes the `App` for the two steps that spend an allowance, which are the
/// footers carrying a live figure: neither screen has anywhere else to put
/// one. The Kit step has two dozen rows and no per-row bar; the Points step
/// has a bar per row, but each bar is that axis's own ceiling rather than
/// the pool, so four of them never add up to how much is left — and the
/// step opens on a *rolled* spread that has already spent the lot, which
/// the player has no way to tell from a blank one.
fn footer(app: &App, step: CreationStep) -> String {
    match step {
        CreationStep::Kit => format!(
            "{}c left - Left/Right takes (Shift/Ctrl); [r] rerolls the basket; \
             Enter moves on once it is spent",
            app.creation_credits_left()
        ),
        CreationStep::Points => {
            let left = app.creation_points_left();
            format!(
                "{}/{CREATION_STAT_POINTS} points spent, {left} left - \
                 Left/Right spends (Shift: all, Ctrl: half); \
                 Enter moves on once it is spent",
                CREATION_STAT_POINTS - left
            )
        }
        CreationStep::Perks => {
            // What the achievement ladder is about to add is named here
            // rather than left to the Summary's profile rows: those sit on
            // another screen among every other reward, and a picker that
            // reads "4 of 4" while the run opens on 6 reads as a defect.
            let earned = match app.profile_perk_points() {
                0 => String::new(),
                n => format!(", +{n} later from your profile"),
            };
            format!(
                "{} of {CREATION_PERK_POINTS} Perk Points{earned} - Left/Right buys; \
                 Enter moves on, unspent points carry over",
                app.creation_perk_points_left()
            )
        }
        _ => plain_footer(step).to_string(),
    }
}

/// The seven steps whose keys never change.
fn plain_footer(step: CreationStep) -> &'static str {
    match step {
        CreationStep::Difficulty => "[p]/[f] picks; Esc backs out to the menu",
        CreationStep::Profile => {
            "Bonuses from past achievements, granted when this run starts - \
             Enter or Right moves on; Left goes back"
        }
        CreationStep::Class => "Up/Down + Enter picks; Left/Right pages; Esc goes back",
        // Written by `footer` above, which is the only caller — every
        // step `CreationStep::spends` names carries a live figure.
        CreationStep::Kit | CreationStep::Points | CreationStep::Perks => "",
        // One arm, because the two halves of a look are one key table —
        // an icon row and a swatch row are picked the same way and skipped
        // the same way, and two copies of the sentence could drift.
        CreationStep::Icon | CreationStep::Colour => {
            "Up/Down + Enter picks; [n] or Right moves on; Left goes back"
        }
        CreationStep::Routine => "Up/Down + Enter; [n] or Right takes none; Left goes back",
        // The last two steps, and the only place the wizard says what
        // *finishes* it — the summary is accepted, the name starts the run.
        CreationStep::Summary => "Enter or Right accepts; Left goes back",
        CreationStep::Name => "Type a name; Enter starts the run; Esc goes back",
    }
}

/// Where the preview cell sits inside `popup`'s box: tucked into the
/// top-right corner, clear of the row list under it. The row labels this
/// screen ever draws (`[3] Colour 4`, `Class ...`, a Summary line) are
/// short enough at `PopupSize::Large`'s width to leave that corner clear;
/// `every_creation_step_draws_a_refusal_exactly_once` and the row census
/// draw the real rows and would show a collision if one ever reached it.
fn preview_cell_rect(popup: Rect, m: &Metrics) -> Rect {
    let size = m.line_height * PREVIEW_CELL_LINES;
    Rect::new(
        popup.x + popup.w - m.pad - size,
        popup.y + m.line_height * 2.0,
        size,
        size,
    )
}

/// The look in `choice` — `previewed_look`'s answer, not necessarily the
/// committed one — painted the way `base.rs` paints the player's own tile:
/// a sprite substituting for the glyph where one is named and loaded, the
/// glyph otherwise, tinted by the same 0-based `PLAYER_CHOICES` index with
/// the same `PLAYER` fallback for "no colour chosen yet".
///
/// Both halves of that resolution are **calls** to `render::
/// player_look_color` and `render::player_sprite_name`, the same two the
/// map's own tile goes through — a second copy here would be free to
/// drift, and the wizard would promise a look the map draws differently.
fn draw_look_preview(choice: &CharacterChoice, painter: &Painter, cell: Rect, m: &Metrics) {
    painter.rect(cell.x, cell.y, cell.w, cell.h, PANEL_BG);
    painter.rect_lines(cell.x, cell.y, cell.w, cell.h, 1.0, BORDER);

    let color = super::player_look_color(choice.colour);
    let inset = m.pad / 2.0;
    let art = cell.w - inset * 2.0;
    let name = super::player_sprite_name(&choice.sprite);
    // The same three rungs `render/base.rs` walks for the player's own
    // tile, in the same order and with the same neutral tint on the top
    // one — the cell is the only place the player sees their pick before
    // the run starts, so a cell resolving a look differently from the map
    // would promise something the map then does not draw. Neutral is plain
    // white here rather than `base.rs`'s grey: there is no vignette on a
    // popup to scale it by.
    let drew = (choice.icon.is_some()
        && painter.sprite(
            crate::sprites::DRAWN_ICON_KEY,
            cell.x + inset,
            cell.y + inset,
            art,
            Color::new(1.0, 1.0, 1.0, color.a),
        ))
        || name.is_some_and(|n| painter.sprite(n, cell.x + inset, cell.y + inset, art, color));
    if !drew {
        let glyph = choice.glyph.to_string();
        let size = art as u16;
        let dims = painter.measure_map(&glyph, size);
        // A sprite fills its square from a top-left; a glyph is drawn from
        // a baseline and centred against measured ink — reading the two as
        // one convention is the half-cell offset `paint::Painter::sprite`'s
        // doc comment warns about, so the two branches lay out separately
        // rather than sharing `cell`'s corner.
        let tx = cell.x + inset + (art - dims.width) / 2.0;
        let ty = cell.y + inset + (art + dims.height) / 2.0;
        painter.map(&glyph, tx, ty, size, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_app_core::{CREATION_COLOURS, GameKey};

    /// **Task 7's decoration for the sixth Icon-step row.** `row_line`
    /// (Task 6) already gives `CreationRow::DrawnIcon` its words; this is
    /// the row's icon slot, the one `build_row`'s decoration match used to
    /// leave at `_ => base` — undrawn. Drawn and undrawn must look
    /// different from each other, the same promise every other Icon/Colour
    /// row on this step makes.
    #[test]
    fn the_drawn_icon_row_carries_a_preview_that_differs_by_state() {
        let drawn = build_row(
            CreationStep::Icon,
            &CreationRow::DrawnIcon { drawn: true },
            5,
            false,
        );
        let undrawn = build_row(
            CreationStep::Icon,
            &CreationRow::DrawnIcon { drawn: false },
            5,
            false,
        );
        let icon_of = |row: Row| match row {
            Row::Item { icon, .. } => icon,
            _ => None,
        };
        let (drawn_icon, undrawn_icon) = (icon_of(drawn), icon_of(undrawn));
        assert!(
            drawn_icon.is_some() && undrawn_icon.is_some(),
            "both states must carry a preview icon, not just words"
        );
        assert_ne!(
            drawn_icon.map(|(g, _)| g),
            undrawn_icon.map(|(g, _)| g),
            "a drawn and an undrawn preview must not look identical"
        );
    }

    /// `CREATION_COLOURS` is app-core's count of a table only this crate
    /// holds, so nothing but this can hold the two in step. A wizard
    /// offering more swatches than the palette has draws nothing for the
    /// last of them, and one offering fewer makes a shipped colour
    /// unreachable.
    #[test]
    fn the_wizard_offers_every_shipped_swatch() {
        assert_eq!(
            CREATION_COLOURS as usize,
            hud::palette::PLAYER_CHOICES.len(),
            "the Look step and the palette disagree about how many colours there are"
        );
    }

    /// A fresh app on the main menu with a scratch profile — the wizard
    /// needs no run, which is the whole reason it holds its own catalogue.
    fn wizard_app() -> App {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let tmp = std::env::temp_dir().join(format!("fp_gui_wizard_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let mut app = App::new(
            root.join("assets"),
            tmp.join("saves"),
            tmp.join("history.log"),
            tmp.join("profile.ron"),
            root.join("dev-arenas"),
            tmp.join("telemetry.jsonl"),
        );
        app.handle_key(GameKey::Char('n'));
        app
    }

    /// `wizard_app`, but the profile on disk has already cleared every
    /// rung the tuning ceilings allow, at the row-maximising distribution.
    ///
    /// `profile_preview_rows` draws one Summary line per **earned
    /// achievement**, not per point — so spreading
    /// `MAX_PROFILE_STAT_POINTS` and `MAX_PROFILE_PERK_POINTS` across
    /// one-point rungs maximises the row count the ceilings permit (14),
    /// where concentrating the same totals into fewer, bigger rungs would
    /// not. That is one more than the 13 `assets/achievements/` ships
    /// today (its stat total is 7, one under the ceiling) — the gap this
    /// test exists to close, since a mod is free to spend the eighth.
    ///
    /// The synthetic ladder lives in its own `achievements/`; every other
    /// subdirectory is symlinked in from the real `assets/` untouched, so
    /// Class, Look and Routine keep reading the real shipped catalogue and
    /// this substitution changes nothing but the Summary step's row count.
    ///
    /// `name` scopes the scratch tree to one caller. It opens by wiping
    /// that tree, so two tests sharing a path delete each other's assets
    /// mid-run — cargo runs them on separate threads of one process, so
    /// the pid alone does not separate them.
    fn wizard_app_with_maximal_profile(name: &str) -> App {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let real_assets = root.join("assets");
        let tmp =
            std::env::temp_dir().join(format!("fp_gui_wizard_max_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let assets = tmp.join("assets");
        std::fs::create_dir_all(&assets).unwrap();
        for entry in std::fs::read_dir(&real_assets).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name() == "achievements" {
                continue;
            }
            std::os::unix::fs::symlink(entry.path(), assets.join(entry.file_name())).unwrap();
        }

        let achievements = assets.join("achievements");
        std::fs::create_dir_all(&achievements).unwrap();
        let mut profile = feral_processes_engine::achievements::Profile::default();
        let mut earn = |id: String, reward: &str| {
            std::fs::write(
                achievements.join(format!("{id}.ron")),
                format!(
                    "(id: \"{id}\", name: \"{id}\", description: \"d\", \
                     trigger: ZoneReached(2), reward: {reward})"
                ),
            )
            .unwrap();
            profile.record(feral_processes_engine::achievements::Earned {
                id: id.as_str().into(),
                first_tick: 1,
                permadeath: false,
                rolled_stat: Some(feral_processes_engine::achievements::MainStat::Atk),
            });
        };
        for i in 0..MAX_PROFILE_STAT_POINTS {
            earn(format!("max_stat_{i}"), "RandomMainStat(1)");
        }
        for i in 0..MAX_PROFILE_PERK_POINTS {
            earn(format!("max_perk_{i}"), "PerkPoints(1)");
        }
        for i in 0..MAX_PROFILE_STARTING_PROGRAMS {
            earn(format!("max_program_{i}"), "StartingProgram(\"scrapper\")");
        }

        let saves = tmp.join("saves");
        std::fs::create_dir_all(&saves).unwrap();
        let profile_path = tmp.join("profile.ron");
        profile.save(&profile_path).unwrap();

        let mut app = App::new(
            assets,
            saves,
            tmp.join("history.log"),
            profile_path,
            root.join("dev-arenas"),
            tmp.join("telemetry.jsonl"),
        );
        app.handle_key(GameKey::Char('n'));
        app
    }

    /// Leaves `step` the cheapest legal way — shared by every test that
    /// needs to visit each step in turn.
    ///
    /// A function rather than the key-per-step table it replaces: the two
    /// steps that hand out an allowance refuse to be left while anything
    /// on them is still affordable, so walking past them is a pass over
    /// their rows rather than one keystroke, and a table cannot say that.
    fn walk_past(app: &mut App, step: CreationStep) {
        let mut spend_every_row = |app: &mut App| {
            for i in 0..app.creation_rows().len() {
                app.menu_selected = i;
                app.handle_key(GameKey::ShiftRight);
            }
            app.menu_selected = 0;
            app.handle_key(GameKey::Enter);
        };
        match step {
            CreationStep::Difficulty => app.handle_key(GameKey::Char('f')),
            CreationStep::Profile => app.handle_key(GameKey::Enter),
            CreationStep::Class => app.handle_key(GameKey::Char('1')),
            CreationStep::Kit | CreationStep::Points | CreationStep::Perks => spend_every_row(app),

            // **The Icon step is walked by drawing, not by `[n]`.** Every
            // census below walks the wizard through here, and the Colour
            // step grows a wrapped note the moment a drawing exists — so a
            // walk that took a preset measured the one Colour step the
            // feature cannot produce and left the tallest, widest form of
            // that step unmeasured.
            CreationStep::Icon => {
                draw_an_icon(app);
                app.handle_key(GameKey::Right);
            }
            CreationStep::Colour | CreationStep::Routine => app.handle_key(GameKey::Char('n')),
            CreationStep::Summary | CreationStep::Name => app.handle_key(GameKey::Enter),
        }
    }

    /// Paints one pixel through the real editor and keeps it, leaving the
    /// wizard on the Icon step with a drawing on the choice — `Enter` in
    /// the editor returns there rather than advancing.
    ///
    /// Shared by `walk_past` and the two tests that need a kept drawing, so
    /// the state the censuses measure is the one the real key table
    /// produces.
    fn draw_an_icon(app: &mut App) {
        while app.menu_selected != app.creation_rows().len() - 1 {
            app.handle_key(GameKey::Down);
        }
        app.handle_key(GameKey::Enter);
        assert!(app.icon_editor_view().is_some(), "the editor did not open");
        app.handle_key(GameKey::Char(' '));
        app.handle_key(GameKey::Enter);
        assert!(
            app.creation_choice().icon.is_some(),
            "keeping the drawing must land it on the choice"
        );
    }

    /// **The refusal census, turned ninety degrees.** `ALL_MODES` walks the
    /// wizard as one mode, so it only ever exercises whichever step the
    /// census app happens to have left the cursor on. Walking
    /// `CreationStep::ALL` is what says the other six can say why they
    /// refused something — a step drawing no popup at all would swallow its
    /// own refusal in silence.
    #[test]
    fn every_creation_step_draws_a_refusal_exactly_once() {
        const REFUSAL: &str = "Requires Zone 3 first.";
        let mut app = wizard_app();
        let mut steps = Vec::new();
        for (i, step) in CreationStep::ALL.iter().enumerate() {
            assert_eq!(
                app.creation_step(),
                *step,
                "the walk fell out of step at {step:?}"
            );
            app.status_line = Some(REFUSAL.to_string());
            let m = ui_metrics(900.0);
            let (_, shapes) =
                crate::paint::with_painter(|p| draw_create_character(&app, Some(REFUSAL), p, &m));
            let drawn = crate::paint::painted_text(&shapes)
                .iter()
                .filter(|t| t.contains(REFUSAL))
                .count();
            assert_eq!(
                drawn, 1,
                "{step:?} painted the refusal {drawn} times, not once"
            );
            steps.push(*step);
            walk_past(&mut app, *step);
        }
        assert_eq!(steps.len(), CreationStep::ALL.len());
    }

    /// Every step draws at least one row of its own — exhaustive over
    /// `CreationStep::ALL`, so an eighth step added without a draw arm
    /// fails to compile rather than shipping a blank popup. A blank popup
    /// is indistinguishable from a broken screen, and against the real
    /// `assets/` an empty step would mean the class or routine catalogue
    /// silently failed to load.
    #[test]
    fn every_creation_step_draws_its_rows() {
        let mut app = wizard_app();
        for (i, step) in CreationStep::ALL.iter().enumerate() {
            let m = ui_metrics(900.0);
            let (_, shapes) =
                crate::paint::with_painter(|p| draw_create_character(&app, None, p, &m));
            let drawn = crate::paint::painted_text(&shapes);
            assert!(
                drawn.iter().any(|t| t.contains(step.title())),
                "{step:?} did not draw its own heading: {drawn:?}"
            );
            assert!(
                drawn.len() > 2,
                "{step:?} drew nothing but chrome: {drawn:?}"
            );
            walk_past(&mut app, *step);
        }
    }

    /// **The wizard has no scroll.** `draw_popup`'s box grows to fit its
    /// content up to `popup::popup_max_rows` — 28, at 1280x720 (the
    /// smallest window the game is built for) and `PopupSize::Large`'s
    /// fractions — and only turns its scroll on past that ceiling. Every
    /// other list menu in the game is fine reaching it; a trade shelf or a
    /// deploy list is read a page at a time anyway. The wizard is not: this
    /// is what holds every step's `Row` list, worst case with a refusal
    /// also showing (the tallest the popup ever draws), under that ceiling.
    ///
    /// **The Summary step's worst case is a fully-cleared profile, not an
    /// empty one.** `wizard_app_with_maximal_profile` earns every rung
    /// `MAX_PROFILE_STAT_POINTS`/`MAX_PROFILE_PERK_POINTS`/
    /// `MAX_PROFILE_STARTING_PROGRAMS` allow — measuring against an empty
    /// profile (what a fresh wizard actually starts with) would pass this
    /// census while a player with a full achievement record ran off the
    /// bottom of the window, exactly the gap this test used to have.
    ///
    /// **Verified by mutation**, not merely written: padding the Look
    /// step's rows past 28 turns `popup::popup_scrolls` from `false` to
    /// `true` and this test red, with the exact row count and the ceiling
    /// in the failure message. See the task's own report for the
    /// transcript.
    ///
    /// `Class` and `Routine` read a moddable catalogue (`assets/classes/`,
    /// the `starter: true` abilities in `assets/abilities/`), each five
    /// rows today — nowhere near the ceiling. This is checked against
    /// what's actually shipped, `notify.rs`'s
    /// `the_tallest_shipped_notification_fits_its_screen` precedent, not a
    /// hypothetical mod's; a mod that grew either catalogue enough to cross
    /// 28 rows would need to reopen this — flagged rather than solved here.
    #[test]
    fn the_tallest_creation_step_fits_its_screen() {
        const REFUSAL: &str = "Requires Zone 3 first.";
        let mut app = wizard_app_with_maximal_profile("height");
        let m = ui_metrics(720.0);
        let mut tallest = 0usize;
        for (i, step) in CreationStep::ALL.iter().enumerate() {
            let drawn = step_rows(&app, *step);
            tallest = tallest.max(drawn.len());
            let scrolls = super::super::popup::popup_scrolls(
                720.0,
                PopupSize::Large,
                &drawn,
                Some(REFUSAL),
                &m,
            );
            assert!(
                !scrolls,
                "{step:?} needs to scroll at 1280x720 with {} rows drawn \
                 and a refusal showing — this screen has no scroll, so \
                 trim the step or give it one",
                drawn.len()
            );
            walk_past(&mut app, *step);
        }
        assert!(
            tallest > 0,
            "the census measured no rows at all — the walk above never reached a step"
        );
    }

    /// **The wizard has no wrap either**, which is the axis the height
    /// census above cannot see. `draw_row` clamps a row vertically and
    /// never horizontally, so a row wider than the popup body simply runs
    /// off the panel in silence — two shipped screens already did that
    /// because nobody measured, and `row_line`'s `Class` arm concatenates
    /// three authored strings (`name`, `axes`, `kit`) out of a **moddable**
    /// directory with no wrap between them.
    ///
    /// Measured through `popup::row_label_text`, the string `draw_row`
    /// itself hands the painter — prefix and icon slot included — against
    /// `popup::popup_body_width`, one pad in from each edge of a
    /// `PopupSize::Large` box. Both shipped window shapes are walked
    /// because the two scale differently: the box is a fraction of the
    /// window's *width* while the font is a fraction of its *height*, so
    /// which one is tightest in columns is not a thing to reason about
    /// from either number alone.
    ///
    /// Against what's actually shipped, `the_tallest_creation_step_fits_
    /// its_screen`'s precedent — a mod whose class name and kit together
    /// outrun the box would need to reopen this and put the kit on a
    /// `popup::continuation_lines` continuation, the shape `draw_craft_menu`
    /// already uses for a recipe and its cost.
    #[test]
    fn no_creation_row_runs_past_the_popup_body() {
        for (screen_w, screen_h) in [(1280.0f32, 720.0f32), (1440.0, 900.0)] {
            let mut app = wizard_app_with_maximal_profile(&format!("width_{screen_h}"));
            let m = ui_metrics(screen_h);
            let body = super::super::popup::popup_body_width(screen_w, PopupSize::Large, &m);
            crate::paint::with_painter(|p| {
                for (i, step) in CreationStep::ALL.iter().enumerate() {
                    for row in step_rows(&app, *step) {
                        let label = super::super::popup::row_label_text(&row);
                        let width = p.measure_ui_advance(&label, m.font_size);
                        assert!(
                            width <= body,
                            "{step:?} draws a {width}px row inside a {body}px body at                              {screen_w}x{screen_h}: {label:?}"
                        );
                    }
                    walk_past(&mut app, *step);
                }
            });
        }
    }

    /// The Colour step's preview cell paints the chosen glyph in the chosen
    /// colour when no sprite is loaded — `with_painter`'s empty sprite
    /// table, the same fallback path `assets/sprites/` missing entirely
    /// takes on the map. Chosen away from the default `('@', PLAYER)` pair
    /// so this cannot pass on a preview that never looked at the choice at
    /// all.
    ///
    /// **Both halves are picked on separate screens and the cell still
    /// shows them together** — which is the property splitting the old
    /// `Look` step had to keep. The glyph is chosen on the step before and
    /// has to survive the advance to be painted here.
    #[test]
    fn the_look_preview_draws_the_chosen_glyph_and_colour() {
        let mut app = wizard_app();
        // Difficulty, the profile summary, the first class, then the Kit
        // step's whole allowance spent — what it costs to leave.
        for step in CreationStep::ALL.iter().take(4) {
            walk_past(&mut app, *step);
        }
        assert_eq!(app.creation_step(), CreationStep::Icon);
        // The third icon (`*`) and then the fourth swatch — both by
        // keyboard, through the real key table, walking `menu_selected` to
        // each target rather than hardcoding a step count that would go
        // stale if a row were ever added ahead of it.
        while app.menu_selected != 2 {
            app.handle_key(GameKey::Down);
        }
        app.handle_key(GameKey::Enter);
        assert_eq!(
            app.creation_choice().glyph,
            '*',
            "the icon pick didn't take"
        );
        assert_eq!(
            app.creation_step(),
            CreationStep::Colour,
            "taking an icon moves on to the swatches"
        );
        while app.menu_selected != 3 {
            app.handle_key(GameKey::Down);
        }

        // Drawn with the cursor resting on the fourth swatch and nothing
        // yet committed — `previewed_look`'s whole point, and the state the
        // player is actually in while choosing.
        let m = ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_create_character(&app, None, p, &m));
        let glyphs = crate::paint::painted_map_glyphs(&shapes);
        let expected = hud::palette::PLAYER_CHOICES[3];
        assert!(
            glyphs.iter().any(|(g, c)| g == "*"
                && (c.r - expected.r).abs() < 1e-3
                && (c.g - expected.g).abs() < 1e-3
                && (c.b - expected.b).abs() < 1e-3),
            "the preview cell did not paint '*' in PLAYER_CHOICES[3]: {glyphs:?}"
        );

        // And Enter keeps exactly what was previewed.
        app.handle_key(GameKey::Enter);
        assert_eq!(
            app.creation_choice().colour,
            Some(3),
            "the swatch pick didn't take"
        );
    }

    /// The **Icon** step's half of `previewed_look`: the cell paints the
    /// glyph under the cursor before anything is committed.
    ///
    /// The colour test above only exercises the `Colour` arm — with the
    /// `Icon` arm dropped it still passes, since the glyph it asserts on
    /// has been committed by then. This is what fails if the cursor stops
    /// previewing a shape.
    #[test]
    fn the_icon_step_previews_the_highlighted_glyph() {
        let mut app = wizard_app();
        for step in CreationStep::ALL.iter().take(4) {
            walk_past(&mut app, *step);
        }
        assert_eq!(app.creation_step(), CreationStep::Icon);
        // The fourth icon, `!` — chosen away from row 0 so a preview
        // ignoring the cursor draws the default `@` instead.
        while app.menu_selected != 3 {
            app.handle_key(GameKey::Down);
        }
        assert_eq!(
            app.creation_choice().glyph,
            '@',
            "nothing is committed yet — the cursor has only moved"
        );

        let m = ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_create_character(&app, None, p, &m));
        // The row list draws every option's glyph too, so the preview is
        // told apart by its colour: a row icon is flat `TEXT`, while the
        // cell tints by `player_look_color`, still `PLAYER` here since no
        // swatch has been chosen. A preview ignoring the cursor paints
        // '@' in that colour instead, which is the mutation this catches.
        let want = hud::palette::PLAYER;
        assert!(
            crate::paint::painted_map_glyphs(&shapes)
                .iter()
                .any(|(g, c)| g == "!"
                    && (c.r - want.r).abs() < 1e-3
                    && (c.g - want.g).abs() < 1e-3
                    && (c.b - want.b).abs() < 1e-3),
            "the preview cell did not paint the highlighted '!'"
        );
    }

    /// **The Points step's footer is the Kit step's, one screen over.**
    /// The pool is spent across four rows and every row draws only its own
    /// bar, so nothing on the screen says how big the pool is or how much
    /// of it is left — the player was reading the *step number* in the
    /// title (`Points (6/9)`) as the point count. Both figures live in the
    /// footer, where `Kit` already puts its allowance.
    ///
    /// Asserted against `CREATION_STAT_POINTS` and `App::
    /// creation_points_left` rather than a copy of the sentence, and
    /// **after a spend as well as before** — a footer naming the pool but
    /// not tracking it would pass on the fresh half alone.
    #[test]
    fn the_points_step_footer_says_what_is_spent_and_what_is_left() {
        let mut app = wizard_app();
        for step in CreationStep::ALL.iter().take(6) {
            walk_past(&mut app, *step);
        }
        assert_eq!(app.creation_step(), CreationStep::Points);

        let m = ui_metrics(900.0);
        // The title says `Points` with a capital P, so the lowercase word
        // picks the footer out and cannot match the heading.
        let pool_row = |app: &App| {
            let (_, shapes) =
                crate::paint::with_painter(|p| draw_create_character(app, None, p, &m));
            let drawn = crate::paint::painted_text(&shapes);
            drawn
                .iter()
                .find(|t| t.contains("points"))
                .unwrap_or_else(|| panic!("no row named the point pool: {drawn:?}"))
                .clone()
        };

        // The step opens on a rolled spread that spends the pool exactly,
        // so the fresh figures are the full ones — which is the state the
        // player actually lands on and the one the missing footer made
        // unreadable.
        let fresh = pool_row(&app);
        assert!(
            fresh.contains(&format!(
                "{CREATION_STAT_POINTS}/{CREATION_STAT_POINTS} points spent"
            )) && fresh.contains("0 left"),
            "the rolled spread spends the whole pool and the footer must say so: {fresh:?}"
        );

        // Clear every axis — which axes the roll landed on is not fixed,
        // so walking all four is what keeps this off the roll's luck.
        for _ in 0..4 {
            app.handle_key(GameKey::ShiftLeft);
            app.handle_key(GameKey::Down);
        }
        let left = app.creation_points_left();
        let spent = CREATION_STAT_POINTS - left;
        assert_eq!(
            left, CREATION_STAT_POINTS,
            "clearing every axis frees the pool"
        );
        let after = pool_row(&app);
        assert!(
            after.contains(&format!("{spent}/{CREATION_STAT_POINTS} points spent"))
                && after.contains(&format!("{left} left")),
            "the footer must follow the spend: {after:?}"
        );
    }

    /// **The Summary reads the icon back in the swatch it will be worn
    /// in**, not in the row colour every other Summary line uses. The
    /// preview cell in the corner already showed the pair together; the
    /// line that names the icon did not, so the one screen that reads the
    /// character back described the look in two different colours.
    ///
    /// Asserted through `painted_runs_in`, which filters on the exact UI
    /// colour — the row is drawn in `PLAYER_CHOICES[3]`, a swatch chosen
    /// away from the `PLAYER` fallback so a line that ignored the choice
    /// would not match.
    #[test]
    fn the_summary_reads_the_icon_back_in_its_chosen_colour() {
        let mut app = wizard_app();
        for step in CreationStep::ALL.iter().take(4) {
            walk_past(&mut app, *step);
        }
        assert_eq!(app.creation_step(), CreationStep::Icon);
        walk_past(&mut app, CreationStep::Icon);

        // The fourth swatch, by keyboard through the real key table.
        while app.menu_selected != 3 {
            app.handle_key(GameKey::Down);
        }
        app.handle_key(GameKey::Enter);
        assert_eq!(app.creation_choice().colour, Some(3));
        walk_past(&mut app, CreationStep::Points);
        walk_past(&mut app, CreationStep::Perks);
        walk_past(&mut app, CreationStep::Routine);
        assert_eq!(app.creation_step(), CreationStep::Summary);

        let m = ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_create_character(&app, None, p, &m));
        let want = hud::palette::PLAYER_CHOICES[3];
        let tinted = crate::paint::painted_runs_in(&shapes, want, false);
        assert!(
            tinted.iter().any(|t| t.contains("Icon")),
            "no Summary row was drawn in the chosen swatch: {tinted:?}"
        );
    }

    /// A sprite substitutes for the preview's glyph exactly as it does on
    /// the map — this is the loaded-texture half of the fallback the test
    /// above exercises the empty half of.
    #[test]
    fn the_look_preview_prefers_a_loaded_sprite_over_the_glyph() {
        let mut app = wizard_app();
        // Difficulty, the profile summary, the first class, then the Kit
        // step's whole allowance spent — what it costs to leave.
        for step in CreationStep::ALL.iter().take(4) {
            walk_past(&mut app, *step);
        }
        assert_eq!(app.creation_step(), CreationStep::Icon);
        // The first icon, '@' / "player" — the pick advances to the Colour
        // step, which draws the same preview cell.
        app.handle_key(GameKey::Enter);
        assert_eq!(app.creation_step(), CreationStep::Colour);

        let mut sprites = crate::paint::SpriteTable::default();
        sprites.insert("player", bevy_egui::egui::TextureId::User(7));
        let m = ui_metrics(900.0);
        let (_, shapes) =
            crate::paint::with_sprites(sprites, |p| draw_create_character(&app, None, p, &m));

        assert_eq!(
            crate::paint::painted_images(&shapes).len(),
            1,
            "exactly one sprite, the preview's"
        );
        let glyphs = crate::paint::painted_map_glyphs(&shapes);
        assert!(
            !glyphs.iter().any(|(g, _)| g == "@"),
            "the '@' must give way to the sprite, not sit under it: {glyphs:?}"
        );
    }

    /// The preview cell's own top rung: a kept drawing is what the cell
    /// shows, over both the named sprite and the glyph.
    ///
    /// The wizard and the map must not be able to disagree about what was
    /// chosen — the cell is the only place the player sees their pick
    /// before the run starts, and it resolves through the same three rungs
    /// `render/base.rs` walks.
    #[test]
    fn the_look_preview_prefers_the_drawn_icon() {
        let mut app = wizard_app();
        for step in CreationStep::ALL.iter().take(4) {
            walk_past(&mut app, *step);
        }
        assert_eq!(app.creation_step(), CreationStep::Icon);

        // The sixth row opens the editor; one painted pixel and `Enter`
        // keeps it and returns here, so the cell is drawn on the step the
        // player is actually standing on with a real drawing on the choice.
        draw_an_icon(&mut app);
        assert_eq!(app.creation_step(), CreationStep::Icon);

        // Both keys present, so this cannot pass on a lookup that missed.
        let mut sprites = crate::paint::SpriteTable::default();
        sprites.insert("player", bevy_egui::egui::TextureId::User(7));
        sprites.insert(
            crate::sprites::DRAWN_ICON_KEY,
            bevy_egui::egui::TextureId::User(9),
        );
        let m = ui_metrics(900.0);
        let (_, shapes) =
            crate::paint::with_sprites(sprites, |p| draw_create_character(&app, None, p, &m));

        let images = crate::paint::painted_images(&shapes);
        assert_eq!(images.len(), 1, "exactly one sprite, the preview's");
        assert_eq!(
            images[0].0,
            bevy_egui::egui::TextureId::User(9),
            "the drawing must win the cell from the named sprite"
        );
        let tint = images[0].2;
        assert_eq!(
            (tint.r(), tint.g()),
            (tint.g(), tint.b()),
            "the cell draws the drawing untinted too, or it would promise a \
             look the map does not draw: {tint:?}"
        );
        let glyphs = crate::paint::painted_map_glyphs(&shapes);
        assert!(
            !glyphs.iter().any(|(g, _)| g == "@"),
            "the '@' must give way to the drawing, not sit under it: {glyphs:?}"
        );
    }

    /// **Task 7's other carried-forward item.** `App::creation_colour_note`
    /// (Task 6) was implemented and tested in app-core but wired into no
    /// gui screen. Once a drawing exists, the Colour step must draw it.
    #[test]
    fn the_colour_step_draws_the_drawn_icon_note() {
        let mut app = wizard_app();
        for step in CreationStep::ALL.iter().take(4) {
            walk_past(&mut app, *step);
        }
        assert_eq!(app.creation_step(), CreationStep::Icon);

        // The sixth row opens the editor; one painted pixel and `Enter`
        // keeps it and returns to the Icon step, so `Right` is what lands
        // on Colour with a real drawing on the choice.
        draw_an_icon(&mut app);
        app.handle_key(GameKey::Right);
        assert_eq!(app.creation_step(), CreationStep::Colour);

        let m = ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_create_character(&app, None, p, &m));
        let drawn: String = crate::paint::painted_text(&shapes).join(" ");
        assert!(
            drawn.contains("glyph"),
            "the Colour step must draw creation_colour_note() once an icon \
             is drawn: {drawn:?}"
        );
    }

    /// ...and the ordinary case — no drawing — must not show it, or the
    /// note would sit on every fresh wizard's Colour step for no reason.
    #[test]
    fn the_colour_step_stays_quiet_with_no_drawn_icon() {
        let mut app = wizard_app();
        for step in CreationStep::ALL.iter().take(4) {
            walk_past(&mut app, *step);
        }
        // A preset rather than `walk_past`, which now draws — the note's
        // absence is exactly what the preset half of the Icon step buys.
        app.handle_key(GameKey::Char('1'));
        assert_eq!(app.creation_step(), CreationStep::Colour);
        assert!(app.creation_choice().icon.is_none());

        let m = ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| draw_create_character(&app, None, p, &m));
        let drawn: String = crate::paint::painted_text(&shapes).join(" ");
        assert!(
            !drawn.contains("glyph"),
            "no drawing means no note: {drawn:?}"
        );
    }
}
