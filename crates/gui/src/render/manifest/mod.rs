//! The manifest — one read-only stat sheet for the player, a program you own,
//! or a wild one.

mod band;
mod keybar;
pub(super) mod layout;
mod ledger;
mod rail;
mod sections;
mod stats;

use super::popup::*;
use super::score;
use super::social::{draw_tab_strip, social_sections};
use super::talk::talk_sections;
use super::*;
use feral_processes_app_core::ManifestTab;
use feral_processes_engine::components::TaskKind;
use feral_processes_engine::species::{AffinityClass, MoveDef};
use feral_processes_engine::{
    DifficultyMode, ManifestEquipSlot, ManifestMood, ManifestSubject, ManifestView, PlayerManifest,
    ProgramManifest,
};
use layout::*;
use sections::*;

/// How big the header glyph is drawn, relative to the UI title size — enough
/// to read as a portrait rather than another line of text, and sized to span
/// the header's two text lines without spilling into the meters below (the
/// header is `HEADER_ROWS` × `line_height` tall, which is a hair over twice
/// `m.title()`).
const HEADER_GLYPH_SCALE: u16 = 2;

/// How many memories the MEMORIES box names under its mood line.
///
/// A **renderer** constant and not an engine one: `ManifestMood` carries
/// `MANIFEST_MOOD_MEMORIES` of them precisely so this is a layout decision
/// the width census can measure past, rather than a truncation baked into the
/// view where a fixture could never build a row this box does not draw.
///
/// Two, because that is what the box has room for — see
/// `manifest_layout::MAX_MOVE_ROWS` for what a third row would cost. The `R`
/// page is where the whole store is read; this is the glance.
pub(super) const MANIFEST_MEMORY_ROWS: usize = 2;

/// What the manifest's footer can offer, which depends on how the screen was
/// opened. Bundled rather than passed as two loose bools, which read as
/// `true, false` at the call site and say nothing.
pub(super) struct ManifestNav {
    /// ←/→ page between subjects. False for a wild program, which isn't in
    /// the owned list and so has nowhere to page to.
    pub(super) cyclable: bool,
    /// Esc returns to the list this was opened from — the manifest picker or
    /// the roster — rather than to the map. Which of the two is app-core's
    /// business (`ManifestOrigin`); the footer only has to know there is one.
    pub(super) back_to_list: bool,
    /// `W` sends the map's camera to this program and goes back to the map.
    ///
    /// False for anything the sim does not walk — a party member, your
    /// wielded program, a squad away on a sortie, a guard — and for every
    /// program when the party is not in base space, which is where staff
    /// stand. `Game::watch_position` is the one rule; this is its `is_some`.
    pub(super) watchable: bool,
    /// Which face of an owned program's sheet is up. Only read when the
    /// subject has a SOCIAL face at all.
    pub(super) tab: ManifestTab,
    /// `[L]` reopens the last level-up page. `None` off your own sheet,
    /// where the key is not bound; `Some(false)` greys it, since a key that
    /// sometimes vanishes is one the player stops looking for.
    /// `App::level_up_reopenable` is the one rule.
    pub(super) level_up: Option<bool>,
}

pub(super) fn draw_manifest(
    game: &mut Game,
    entity: Option<Entity>,
    nav: ManifestNav,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(view) = entity.and_then(|e| game.manifest(e)) else {
        draw_popup(
            "Manifest",
            PopupSize::Small,
            &[text_row("That program is gone. Esc to go back.")],
            refusal,
            painter,
            m,
        );
        return;
    };

    // `Some` only for a program you own, which is also exactly who has a tab
    // strip: the strip never offers a face the subject does not have.
    let social = game.social(view.entity);
    let own = matches!(view.subject, ManifestSubject::Player(_));
    let sections = match (&social, nav.tab) {
        _ if own && nav.tab == ManifestTab::Score => score::manifest_sections(&game.score_card()),
        (Some(s), ManifestTab::Social) => social_sections(s),
        (Some(_), ManifestTab::Talk) => {
            let exchanges = game.conversations(view.entity).unwrap_or_default();
            let screen = (painter.screen_w(), painter.screen_h());
            talk_sections(&exchanges, screen, painter, m)
        }
        _ => {
            let face = stats::StatsFace::gather(game, view, &nav, social.is_some());
            stats::draw_stats(&face, painter, m);
            return;
        }
    };
    let l = manifest_layout(painter.screen_w(), painter.screen_h(), &sections, m);

    painter.rect(l.frame.x, l.frame.y, l.frame.w, l.frame.h, PANEL_BG);
    painter.rect_lines(l.frame.x, l.frame.y, l.frame.w, l.frame.h, 2.0, BORDER);

    let escaped = game.phase_keys().story_complete;
    draw_header(
        &view,
        escaped,
        (social.is_some() || own).then_some(nav.tab),
        l.header,
        painter,
        m,
    );
    for (rect, section) in l.sections.iter().zip(&sections) {
        draw_section(section, *rect, painter, m);
    }

    let footer = footer_text(&nav, view.remembers());
    let footer_y = l.footer.y + m.font_size as f32;
    painter.ui(&footer, l.footer.x, footer_y, m.small(), TEXT_DIM);
    if let Some((hint, color)) = level_up_hint(&nav) {
        let x = l.footer.x + painter.measure_ui_advance(format!("{footer}{FOOTER_GAP}"), m.small());
        painter.ui(hint, x, footer_y, m.small(), color);
    }
}

/// Three spaces, not six: with all five keys offered, six ran 216px past
/// the frame at 1280x1440 (`the_fullest_footer_fits_the_narrowest_window`).
const FOOTER_GAP: &str = "   ";

const LEVEL_UP_HINT: &str = "[L] level up";

/// `[L]` trails the rest of the footer in its own run, so it alone can be
/// greyed.
fn level_up_hint(nav: &ManifestNav) -> Option<(&'static str, Color)> {
    nav.level_up.map(|open| {
        (
            LEVEL_UP_HINT,
            if open { TEXT_DIM } else { hud::palette::FAINT },
        )
    })
}

/// Every key the sheet answers, since this line is the only place any of
/// them is advertised. `[D]` is unconditional: `Game::dossier_report`
/// answers for any body the sheet itself can show. `[R]` is
/// `ManifestView::remembers`, the answer app-core refuses on. `[H]` and `[S]`
/// are not here: the DEVELOPMENT box carries its own hint.
fn footer_text(nav: &ManifestNav, remembers: bool) -> String {
    let mut footer = Vec::new();
    if nav.cyclable {
        footer.push("←/→ programs");
    }
    if nav.watchable {
        footer.push("[W] watch");
    }
    footer.push("[D] dossier");
    if remembers {
        footer.push("[R] memories");
    }
    footer.push(if nav.back_to_list {
        "Esc back to list"
    } else {
        "Esc back"
    });
    footer.join(FOOTER_GAP)
}

fn draw_header(
    view: &ManifestView,
    escaped: bool,
    tab: Option<ManifestTab>,
    rect: Rect,
    painter: &Painter,
    m: &Metrics,
) {
    let glyph_size = m.title() * HEADER_GLYPH_SCALE;
    let glyph = view.glyph.to_string();
    painter.map(
        &glyph,
        rect.x,
        rect.y + glyph_size as f32 * 0.85,
        glyph_size,
        glyph_color(view.color),
    );
    let text_x = rect.x + painter.measure_map(&glyph, glyph_size).width + m.pad;

    let boss = is_boss(view);
    let rarity = header_rarity(view);
    // Both flags, and the tier keeps its own colour claim: a boss reads red
    // and an Overclocked spawn gold, so a program that is both would have to
    // give one up if this were a single string in a single colour. `[BOSS]`
    // wins the title because it is the one that decides whether to open the
    // fight; the tier follows it, drawn in the same silver/gold the map's
    // bar uses so the two channels agree.
    let head = header_title(view);
    painter.ui_bold(
        head.clone(),
        text_x,
        rect.y + m.title() as f32,
        m.title(),
        if boss { RED } else { WHITE },
    );
    if let Some(tier_color) = rarity_color(rarity) {
        painter.ui_bold(
            rarity_tag(rarity),
            text_x
                + painter
                    .measure(crate::paint::Face::UiBold, &head, m.title())
                    .width,
            rect.y + m.title() as f32,
            m.title(),
            tier_color,
        );
    }

    if let Some(active) = tab {
        draw_tab_strip(
            active,
            matches!(view.subject, ManifestSubject::Player(_)),
            rect.x + rect.w,
            rect.y + m.title() as f32,
            painter,
            m,
        );
    }

    let tags = header_tags(view, escaped);
    painter.ui(
        tags.join("   "),
        text_x,
        rect.y + m.title() as f32 + m.line_height,
        m.font_size,
        TEXT_DIM,
    );
}

/// The second line of the header: level, tier, post, and the statuses.
pub(super) fn header_tags(view: &ManifestView, escaped: bool) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    if let Some(level) = view.level {
        tags.push(format!("Lv {level}"));
    }
    match &view.subject {
        ManifestSubject::Program(p) => {
            if let Some(q) = &p.potential {
                tags.push(format!("{} ({}%)", q.label, q.percent));
            }
            if p.generation > 0 {
                tags.push(format!("gen {}", p.generation));
            }
            if p.fusions > 0 {
                tags.push(format!("fused {}/{}", p.fusions, p.max_fusions));
            }
            if p.refactors > 0 {
                tags.push(format!("upgraded {}/{}", p.refactors, p.max_refactors));
            }
            if p.is_companion {
                tags.push("in party".to_string());
            } else if let Some(activity) = &p.activity {
                tags.push(activity.clone());
            } else if p.is_hostile {
                tags.push("rogue".to_string());
            }
        }
        ManifestSubject::Player(p) => {
            tags.push(format!("Zone {}", p.zone));
            tags.push(format!("Pets {}/{}", p.pet_count, p.pet_capacity));
            if escaped {
                tags.push("Escaped".to_string());
            }
        }
    }
    if !view.statuses.is_empty() {
        tags.push(super::battle::status_tags_text(&view.statuses));
    }
    tags
}

fn draw_section(section: &Section, rect: Rect, painter: &Painter, m: &Metrics) {
    painter.rect_lines(rect.x, rect.y, rect.w, rect.h, 1.0, BORDER);
    painter.ui(
        &section.title,
        rect.x + m.inset,
        rect.y + m.line_height,
        m.small(),
        CYAN,
    );
    if section.overflow > 0 {
        let title_w = painter.measure_ui_advance(&section.title, m.small());
        painter.ui(
            format!(" +{}", section.overflow),
            rect.x + m.inset + title_w,
            rect.y + m.line_height,
            m.small(),
            TEXT_DIM,
        );
    }
    if !section.hint.is_empty() {
        let hint_w = painter.measure_ui_advance(&section.hint, m.small());
        painter.ui(
            &section.hint,
            rect.x + rect.w - m.inset - hint_w,
            rect.y + m.line_height,
            m.small(),
            TEXT_DIM,
        );
    }
    let mut cy = rect.y + m.line_height + m.gap;
    for row in &section.rows {
        cy += section_row_h(m);
        match row {
            SectionRow::Stat(label, value)
            | SectionRow::Ranked(label, value, _)
            | SectionRow::Alert(label, value) => {
                let fitted = fitted_stat_row(painter, label, value, rect, m);
                painter.ui(&fitted.label, rect.x + m.inset, cy, fitted.size, TEXT_DIM);
                painter.ui(
                    &fitted.value,
                    rect.x + rect.w - m.inset - fitted.value_w,
                    cy,
                    fitted.size,
                    TEXT,
                );
            }
            SectionRow::Note(text) | SectionRow::Callout(text) => {
                painter.ui(text, rect.x + m.inset, cy, m.font_size, TEXT);
            }
        }
    }
}

/// A stat row as `draw_section` will actually draw it — both halves already
/// cut to the box they have to share.
pub(super) struct FittedStatRow {
    pub(super) label: String,
    pub(super) value: String,
    /// One size for the whole row. A value at the body size beside a shrunken
    /// label reads as two rows, and the width the value gives up is exactly
    /// what the label is short of.
    pub(super) size: u16,
    /// The fitted value's measured width, which is also what it is placed by.
    pub(super) value_w: f32,
}

/// Cuts one stat row to `rect`: drawn at the body size when the pair fits, at
/// `m.small()` when only that does, and with the **label** elided into
/// whatever the value leaves when even that overruns.
///
/// The value is the half that stays whole, because it is the column a player
/// scans *down* a box. One `m.gap` is held back between the two so they never
/// touch.
///
/// Returned rather than drawn so the width census can measure the row the
/// renderer draws instead of a restated copy of this arithmetic — the mistake
/// `no_column_row_overflows_the_column` records for the info column.
pub(super) fn fitted_stat_row(
    painter: &Painter,
    label: &str,
    value: &str,
    rect: Rect,
    m: &Metrics,
) -> FittedStatRow {
    let room = rect.w - m.inset * 2.0;
    let pair = |size| {
        painter.measure_ui_advance(label, size) + painter.measure_ui_advance(value, size) + m.gap
    };
    let size = if pair(m.font_size) <= room {
        m.font_size
    } else {
        m.small()
    };
    let value = elided_to_fit(painter, value, size, room);
    let value_w = painter.measure_ui(&value, size).width;
    let label = elided_to_fit(painter, label, size, room - value_w - m.gap);
    FittedStatRow {
        label,
        value,
        size,
        value_w,
    }
}

/// `text` cut to `room` at `size`: unchanged when it already fits, and
/// otherwise the widest head-and-tail of it joined by a `…`.
///
/// **Middle-elided, never cut from the right.** Nothing on this sheet clips
/// horizontally — `draw_section` draws a row as two plain strings — so an
/// overlong one used to run over the neighbouring column or straight off the
/// window, and the half a player lost was always the tail. On the one row
/// long enough to need this, EQUIPMENT's, the tail is where `Game::copy_name`
/// puts a gear copy's suffix affix, its `+N` affix count and its quality
/// figure; the head is where it puts the tier word and the prefix affix. Both
/// ends carry meaning, so both ends are what survive.
///
/// Chars, not bytes: an item name is content, and a mod's can hold multi-byte
/// glyphs that byte slicing would panic on — `battle::cell`'s reason. Each
/// candidate is measured rather than divided out of a per-character advance,
/// so this stays correct if the UI face is ever something other than the
/// monospace it is today.
fn elided_to_fit(painter: &Painter, text: &str, size: u16, room: f32) -> String {
    if painter.measure_ui_advance(text, size) <= room {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    for keep in (0..chars.len()).rev() {
        // The odd character goes to the head, which is where the row's own
        // label ("MOD: ") sits — losing that first reads as a different row
        // rather than as a shortened one.
        let head = keep.div_ceil(2);
        let candidate: String = chars[..head]
            .iter()
            .chain(std::iter::once(&'…'))
            .chain(&chars[chars.len() - (keep - head)..])
            .collect();
        if painter.measure_ui_advance(&candidate, size) <= room {
            return candidate;
        }
    }
    String::new()
}

fn is_boss(view: &ManifestView) -> bool {
    matches!(&view.subject, ManifestSubject::Program(p) if p.is_boss)
}

pub(super) fn header_rarity(view: &ManifestView) -> Rarity {
    match &view.subject {
        ManifestSubject::Program(p) => p.rarity,
        ManifestSubject::Player(_) => Rarity::Ordinary,
    }
}

/// The bold title line before the tier tag: the subject's title and, for a
/// boss, its `[BOSS]` flag.
pub(super) fn header_title(view: &ManifestView) -> String {
    format!(
        "{}{}",
        subject_title(view),
        if is_boss(view) { "  [BOSS]" } else { "" }
    )
}

/// What heads the page: who this is, and in parentheses what kind of thing
/// they are.
///
/// **The parenthetical answers one question for both subjects.** A program
/// fills it with its species; the player fills it with their class, which
/// is the same question asked of a player — and the reason the player's
/// half exists at all is that the sheet used to head itself "You" and name
/// no class, so the two facts a run is most identified by were readable
/// only on the wizard screen that asked for them.
///
/// Neither half is guaranteed: a program whose species failed to resolve
/// and a classless run both fall back to the name alone.
fn subject_title(view: &ManifestView) -> String {
    match subject_kind(view) {
        Some(kind) => format!("{}  ({kind})", view.name),
        None => view.name.clone(),
    }
}

/// The parenthetical half of `subject_title`, on its own for the band.
pub(super) fn subject_kind(view: &ManifestView) -> Option<String> {
    match &view.subject {
        ManifestSubject::Program(p) => p.species_name.clone(),
        ManifestSubject::Player(p) => p.class.as_ref().map(|c| c.name.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::ledger::fit_ledger_row;
    use super::*;
    use crate::paint::with_painter;
    use crate::render::manifest::layout::{ManifestLayout, frame_rect, manifest_layout};
    use crate::text::ui_metrics;
    use feral_processes_engine::{ProgramPoints, RankedStat};

    /// A `ProgramManifest` for a species that declares no *box-level*
    /// optionals (no POTENTIAL, no AFFINITIES), so the only boxes
    /// `program_sections` emits are the unconditional ones. Hand-built
    /// rather than pulled off a real creature because what is under test is
    /// which box a *row* lands in, and a shipped species would tie that to
    /// whatever the roster happens to say today.
    ///
    /// `work_resource` is set rather than left `None`, deliberately: within
    /// SPECIES it is the one row that is itself conditional, and a non-boss
    /// species carrying one is the real worst case for that box's row count
    /// (5, not 4) — see `work_rows_live_in_their_own_box_and_species_drops_
    /// below_its_cap` below, which is what would stop catching a species-box
    /// overflow if this quietly went back to `None`.
    fn plain_program(base_speed: i32, analysis: i32) -> ProgramManifest {
        ProgramManifest {
            species_name: Some("Testmon".to_string()),
            is_hostile: false,
            is_tamed: true,
            is_companion: true,
            is_boss: false,
            activity: None,
            post: None,
            potential: None,
            generation: 0,
            fusions: 0,
            max_fusions: 3,
            rarity: Rarity::Ordinary,
            refactors: 0,
            max_refactors: 3,
            ring: 0,
            max_ring: 3,
            level_cap: 6,
            talents_spent: 0,
            talents_earned: 0,
            habitats: vec![],
            moves: vec![],
            work_resource: Some("core_fragment".into()),
            taming_difficulty: 0.5,
            decompile_chance: None,
            growth_multiplier: 1.0,
            base_speed,
            analysis,
            affinities: vec![],
            // The two boss species are the only shipped programs with no
            // class, and a boss cannot be tamed or posted — so a job row is
            // the ordinary case and this fixture is deliberately the one
            // that omits it, the same way it keeps a `work_resource` to
            // hold SPECIES at its worst case.
            base_job: None,
            // Empty on purpose: the NEEDS box is conditional and its own
            // test builds one, so this fixture holds every other box at its
            // worst case without that one moving under it.
            needs: vec![],
            // `None` on purpose, for `needs`' reason: this is the wild
            // program's state, so every other box sits at its worst case
            // without the MEMORIES box moving under it. `owned_program`
            // below is the fixture that carries one.
            mood: None,
            // `None` for `mood`'s reason: the wild program is unseated.
            points: None,
        }
    }

    /// `plain_program` as the player owns it: a mood, and so the MEMORIES
    /// box. The two are the page's real alternatives — a program with a
    /// `Memories` store is on the roster, and one without it is wild.
    fn owned_program(memories: Vec<MemoryRow>) -> ProgramManifest {
        ProgramManifest {
            is_tamed: true,
            mood: Some(ManifestMood {
                sum: -11.0,
                band: morale_band(-11.0),
                memories,
            }),
            points: Some(ProgramPoints {
                banked: 3,
                holding: true,
            }),
            ..plain_program(6, 6)
        }
    }

    /// A `MemoryRow` differing only in the fields these tests care about.
    fn memory(name: &str, subject: Option<&str>, intensity: f32) -> MemoryRow {
        MemoryRow {
            name: name.to_string(),
            blurb: "b".to_string(),
            subject: subject.map(str::to_string),
            intensity,
            age: "recently".to_string(),
        }
    }

    /// Speed belongs in WORK, not SPECIES, and the reason is a row cap rather
    /// than taxonomy: SPECIES sat at exactly `MAX_SECTION_ROWS`, where a
    /// seventh row does not fail anything — it silently truncates to "+N
    /// more" and the data just vanishes. Moving Speed out buys the headroom
    /// that Analysis then spends, and leaves some for the base-job row.
    #[test]
    fn work_rows_live_in_their_own_box_and_species_drops_below_its_cap() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(
            11,
            feral_processes_engine::DifficultyMode::Forgiving,
            assets,
        )
        .expect("shipped assets load");

        let mut sections = Vec::new();
        program_sections(&mut sections, &game, &plain_program(14, 12));

        let labels = |title: &str| -> Vec<String> {
            sections
                .iter()
                .find(|s| s.title == title)
                .unwrap_or_else(|| panic!("no {title} box was emitted"))
                .rows
                .iter()
                .filter_map(|r| match r {
                    SectionRow::Stat(label, _) | SectionRow::Ranked(label, _, _) => {
                        Some(label.clone())
                    }
                    _ => None,
                })
                .collect()
        };

        let work = labels("WORK");
        assert!(
            work.iter().any(|l| l == "Speed"),
            "Speed moves into WORK: {work:?}"
        );
        assert!(
            work.iter().any(|l| l == "Analysis"),
            "analysis is shown as Analysis: {work:?}"
        );

        let species = labels("SPECIES");
        assert!(
            !species.iter().any(|l| l == "Speed"),
            "Speed must not be left behind in SPECIES too: {species:?}"
        );
        assert_eq!(
            species.len(),
            4,
            "a non-boss species is SPECIES' real worst case (Habitats, \
             Decompile difficulty, Decompile chance now, Growth) — it must \
             land below MAX_SECTION_ROWS: {species:?}"
        );
        assert!(
            work.iter().any(|l| l == "Work aptitude"),
            "Work aptitude is ranked among programs, so it sits in WORK: {work:?}"
        );
        assert!(
            species.len() < MAX_SECTION_ROWS,
            "SPECIES has to come off its cap, or the next row added to it \
             vanishes into '+N more': {species:?}"
        );
    }

    /// A class the player can post is stated on the one screen that says
    /// what a program is like to post. Without it the three base jobs are
    /// invisible: nothing else names a class anywhere in the game, and a
    /// player would have to notice a Medic mending a wall by watching a
    /// Durability number tick.
    #[test]
    fn the_work_box_names_what_a_class_does_at_a_post() {
        let job_row = |class| {
            let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
            let game = Game::new(
                11,
                feral_processes_engine::DifficultyMode::Forgiving,
                assets,
            )
            .expect("shipped assets load");
            let mut program = plain_program(14, 12);
            program.base_job = class;
            let mut sections = Vec::new();
            program_sections(&mut sections, &game, &program);
            sections
                .iter()
                .find(|s| s.title == "WORK")
                .expect("a WORK box is always emitted")
                .rows
                .iter()
                .find_map(|r| match r {
                    SectionRow::Stat(label, value) if label == "Base job" => Some(value.clone()),
                    _ => None,
                })
        };

        assert_eq!(
            job_row(Some(AffinityClass::Medic)).as_deref(),
            Some("repair (Medic)")
        );
        assert_eq!(
            job_row(Some(AffinityClass::Bastion)).as_deref(),
            Some("guard (Bastion)")
        );
        assert_eq!(
            job_row(Some(AffinityClass::Leech)).as_deref(),
            Some("extraction (Leech)")
        );
    }

    /// The two classes with no base job say so rather than going quiet. The
    /// asymmetry is the design — with three pet slots, a program at a
    /// machine is one absent from the party — and a blank row would read as
    /// missing data instead of as a decision.
    #[test]
    fn a_class_with_no_base_job_says_so_on_the_row() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(
            11,
            feral_processes_engine::DifficultyMode::Forgiving,
            assets,
        )
        .expect("shipped assets load");
        let mut program = plain_program(14, 12);
        program.base_job = Some(AffinityClass::Striker);
        let mut sections = Vec::new();
        program_sections(&mut sections, &game, &program);

        let work = sections.iter().find(|s| s.title == "WORK").unwrap();
        assert!(
            work.rows.iter().any(|r| matches!(
                r,
                SectionRow::Stat(label, value) if label == "Base job" && value == "none (Striker)"
            )),
            "a Striker's row has to state the absence, and WORK has {} rows",
            work.rows.len()
        );
    }

    /// A `ManifestView` around `program`, with one routine slot so ROUTINES
    /// is emitted, and whatever gear the caller hands it.
    fn program_view(program: ProgramManifest, equipment: Vec<ManifestEquipSlot>) -> ManifestView {
        ManifestView {
            entity: Entity::PLACEHOLDER,
            name: "Testmon".to_string(),
            glyph: 'x',
            color: GlyphColor::White,
            level: None,
            xp: None,
            hp: 10,
            max_hp: 10,
            atk: 5,
            mitigation: 5,
            damage: "3–7".to_string(),
            power: 15,
            accuracy: 12.5,
            evasion: 9.5,
            statuses: Vec::new(),
            routines: vec![feral_processes_engine::RoutineSlotView {
                index: 0,
                ability: None,
                name: "(empty)".to_string(),
                description: String::new(),
                fixed: false,
                unseen: false,
            }],
            equipment,
            subject: ManifestSubject::Program(Box::new(program)),
        }
    }

    fn worn(slot: &str) -> ManifestEquipSlot {
        ManifestEquipSlot {
            slot: slot.to_string(),
            item_name: "Arc Lance".to_string(),
            gear_level: 1,
            fusion_tier: 0,
            atk: 4,
            mitigation: 0,
            decompiler: 0,
            deflection: 0,
        }
    }

    #[test]
    fn a_worn_slot_with_deflection_draws_defl_beside_mit() {
        let mut slot = worn("Armor");
        slot.mitigation = 5;
        slot.deflection = 2;
        let SectionRow::Stat(_, bonus) = equip_row(&slot) else {
            panic!("an equipment row is a stat row");
        };
        assert!(bonus.contains("+5 MIT +2 DEFL"), "{bonus}");
        slot.deflection = 0;
        let SectionRow::Stat(_, bonus) = equip_row(&slot) else {
            panic!("an equipment row is a stat row");
        };
        assert!(!bonus.contains("DEFL"), "{bonus}");
    }

    /// Window sizes a width census is measured at.
    ///
    /// Whole shapes, and not `manifest_layout`'s cross product of widths and
    /// heights: `ui_metrics` scales the font off the window's **height**
    /// alone, so pairing 1280 wide with 1440 tall puts 33px text in a frame
    /// 1177px across — an aspect ratio no display has, and one that overruns
    /// rows all over the renderer. The vertical sweep can take the cross
    /// product because a taller window only ever buys it room. This is the
    /// 16:9 and 16:10 ladder from the tightest window the game supports up,
    /// the same geometry `no_manifest_pick_row_overflows_its_popup` measures
    /// against — it just holds one point of it.
    const CENSUS_WINDOWS: [(f32, f32); 8] = [
        (1280.0, 720.0),
        (1366.0, 768.0),
        (1440.0, 900.0),
        (1600.0, 900.0),
        (1680.0, 1050.0),
        (1920.0, 1080.0),
        (1920.0, 1200.0),
        (2560.0, 1440.0),
    ];

    /// The widest EQUIPMENT row the shipped assets can build, and the two
    /// affix words in its name.
    ///
    /// The name is the ceiling app-core's
    /// `no_shipped_copy_name_outgrows_the_swap_name_column` measures, derived
    /// the same way rather than copied: fusion is the only thing that stacks
    /// affixes, so a copy can carry `ITEM_FUSION_COST ^ MAX_FUSIONS` of them,
    /// of which `Game::copy_name` names the first with a prefix and the first
    /// with a suffix and counts the rest as `+N`. Padding with copies of the
    /// two longest words rather than with other affixes keeps which pair
    /// `copy_name` picks out of however the ids happen to sort.
    ///
    /// The bonus column is `Game::copy_bonus` at level 10 on the same copy,
    /// matching what app-core's
    /// `no_shipped_gear_summary_outgrows_the_swap_stats_column` prices — the
    /// value is a second axis competing for the same box, so a census that
    /// left it empty would measure the easy case.
    fn worst_equipment_row(game: &Game) -> (ManifestEquipSlot, String, String) {
        use feral_processes_engine::affixes::AffixDef;
        use feral_processes_engine::components::Rarity;
        use feral_processes_engine::items::{EquipmentSlot, GearCopy};
        use feral_processes_engine::tuning::{ITEM_FUSION_COST, MAX_FUSIONS, QUALITY_MAX};

        let defs = game.affix_defs();
        let longest = |pick: fn(&AffixDef) -> Option<&String>| {
            defs.iter()
                .filter(|a| pick(a).is_some())
                .max_by_key(|a| pick(a).map(|w| w.chars().count()).unwrap_or(0))
                .map(|a| (a.id.clone(), pick(a).cloned().unwrap_or_default()))
                .expect("the shipped set has affixes on both sides of a name")
        };
        let (prefix_id, prefix_word) = longest(|a| a.prefix.as_ref());
        let (suffix_id, suffix_word) = longest(|a| a.suffix.as_ref());

        let ceiling = (ITEM_FUSION_COST as usize).pow(MAX_FUSIONS);
        let mut affixes = vec![prefix_id; ceiling.div_ceil(2)];
        affixes.resize(ceiling, suffix_id);

        let slot = EquipmentSlot::ALL
            .into_iter()
            .max_by_key(|s| s.short_label().chars().count())
            .expect("EquipmentSlot::ALL is not empty");

        let mut worst: Option<(ManifestEquipSlot, usize)> = None;
        for def in game
            .item_defs()
            .into_iter()
            .filter(|d| d.equipment.is_some())
        {
            let copy = GearCopy::with_affixes(
                def.id.clone(),
                Rarity::ALL[Rarity::ALL.len() - 1],
                MAX_FUSIONS,
                affixes.clone(),
                QUALITY_MAX,
            );
            let Some(mods) = game.copy_bonus(&copy, 10) else {
                continue;
            };
            let row = ManifestEquipSlot {
                slot: slot.short_label().to_string(),
                item_name: game.copy_name(&copy),
                gear_level: 10,
                fusion_tier: copy.tier,
                atk: mods.atk,
                mitigation: mods.mitigation,
                decompiler: mods.decompiler,
                deflection: mods.deflection,
            };
            let cells = row.item_name.chars().count();
            if worst.as_ref().is_none_or(|(_, w)| cells > *w) {
                worst = Some((row, cells));
            }
        }
        let (row, _) = worst.expect("the shipped set has equippable gear");
        (row, prefix_word, suffix_word)
    }

    /// The EQUIPMENT box's rect on both pages that draw one, at `w` x `h` —
    /// the real one, off `manifest_layout`, rather than a width written down
    /// here.
    ///
    /// Every `EquipmentSlot` is worn, which is the box's own worst case: its
    /// *width* is settled by whether the box is a band or a column whatever
    /// the rows say, but a taller box can land in the other column, and which
    /// column a box lands in is what decides its `x`.
    fn equipment_rects(game: &Game, w: f32, h: f32) -> Vec<(&'static str, Rect)> {
        let mut program = plain_program(14, 12);
        program.base_job = Some(AffinityClass::Striker);
        program.post = Some((TaskKind::GatherResource, "Mining Node".to_string()));
        let kit = || vec![worn("WEP"), worn("ARM"), worn("MOD")];
        let mut player = player_view(plain_player());
        player.equipment = kit();

        let mut out = Vec::new();
        for (who, view) in [
            ("program", program_view(program, kit())),
            ("player", player),
        ] {
            let rect = ledger_rect(game, &view, "GEAR", w, h).expect("a kitted page draws GEAR");
            out.push((who, rect));
        }
        assert_eq!(out.len(), 2, "both pages draw a GEAR box");
        out
    }

    fn census_game() -> Game {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        Game::new(
            11,
            feral_processes_engine::DifficultyMode::Forgiving,
            assets,
        )
        .expect("shipped assets load")
    }

    /// The column the ledger really gives section `title` on `view`'s page at
    /// `w` x `h`, rather than a width written down here. `None` when the page
    /// does not draw that section.
    ///
    /// A rail is present and the bands are at their tallest (`you`, the table
    /// header and a full party): the full rail eats width and the bands eat
    /// height, so a census measuring a thinner page would be measuring a
    /// column the player never sees.
    fn ledger_rect(game: &Game, view: &ManifestView, title: &str, w: f32, h: f32) -> Option<Rect> {
        let m = ui_metrics(h);
        let (char_w, _) = with_painter(|p| p.measure_ui_advance("M", m.font_size));
        let l = ManifestLayout::new(
            frame_rect(w, h),
            &m,
            char_w,
            layout::Face::Stats,
            WORST_RAIL_ROWS,
            WORST_BAND_ROWS,
        );
        sections_for(game, view)
            .iter()
            .find(|s| s.title == title)
            .and_then(|s| s.column)
            .map(|c| l.ledger[c.index()])
    }

    /// A rail with forty subjects and its party rule.
    const WORST_RAIL_ROWS: usize = 41;
    /// The PARTY table at its tallest: its head, you and a full party.
    const WORST_BAND_ROWS: usize = 2 + feral_processes_engine::tuning::MAX_PARTY_SIZE;

    /// The box `manifest_layout` really gives `title` on a program's page at
    /// `w` x `h`, rather than a width written down here. `None` when the page
    /// does not draw that box at all.
    ///
    /// The program is owned and fully developed, which is the state that puts
    /// the most columned boxes on the page — and which column a box lands in
    /// is what decides its `x`, so a census measuring a thinner page would be
    /// measuring a box the player never sees.
    fn program_box_rect(
        game: &Game,
        program: ProgramManifest,
        title: &str,
        w: f32,
        h: f32,
    ) -> Option<Rect> {
        let view = program_view(program, vec![worn("WEP"), worn("ARM"), worn("MOD")]);
        ledger_rect(game, &view, title, w, h)
    }

    /// The fullest program page there is: owned, developed, posted, kitted.
    fn worst_case_owned(moves: Vec<MoveDef>, memories: Vec<MemoryRow>) -> ProgramManifest {
        let mut program = owned_program(memories);
        program.base_speed = 14;
        program.analysis = 12;
        program.base_job = Some(AffinityClass::Striker);
        program.post = Some((TaskKind::GatherResource, "Mining Node".to_string()));
        program.ring = 3;
        program.level_cap = 12;
        program.talents_earned = 6;
        program.talents_spent = 6;
        program.moves = moves;
        program
    }

    /// **MOVES sits in a third of the frame**, so a move's tag list can take
    /// most of its column and the name is what `fit_ledger_row` elides.
    ///
    /// Containment is not a testable property of this row: the label is cut
    /// into whatever the value leaves, so `label_end <= value_start` holds by
    /// construction. What is held is the rule itself: the value stays whole.
    /// At 1280x720 the widest shipped names are middle-elided to a stub, which
    /// the report on this change records rather than this test pinning.
    ///
    /// Over the real `assets/species/` catalogue rather than a hand-picked
    /// worst case — a move's width is its authored name plus its tag list,
    /// and choosing the widest by eye is how a census goes stale when a
    /// species is added. Measured against the box `manifest_layout` really
    /// gives MOVES, at every window in `CENSUS_WINDOWS`.
    ///
    /// The size is *not* asserted: `fitted_stat_row` drops the whole row to
    /// `m.small()` before it cuts anything, and a smaller row is still a
    /// complete one. Only the cut is a loss.
    #[test]
    fn a_move_row_keeps_its_value_whole() {
        let game = census_game();
        let moves: Vec<MoveDef> = game
            .species_defs()
            .into_iter()
            .flat_map(|def| def.moves.clone())
            .collect();
        assert!(!moves.is_empty(), "the shipped species have moves");

        with_painter(|p| {
            for (w, h) in CENSUS_WINDOWS {
                let m = ui_metrics(h);
                // One move on the page, so the box is measured where the
                // renderer puts it for a species with a single wide move —
                // the row is what is under test, not the box's height.
                for mv in &moves {
                    let program = worst_case_owned(
                        vec![mv.clone()],
                        vec![memory("Mauled by", Some("Husk"), -8.0)],
                    );
                    let rect = program_box_rect(&game, program, "MOVES", w, h)
                        .expect("a program with a move draws a MOVES box");
                    let SectionRow::Stat(label, value) = move_row(mv) else {
                        panic!("a move row is a stat row");
                    };
                    let row = fit_ledger_row(p, &label, &value, rect.w, false, &m);
                    assert_eq!(
                        row.value, value,
                        "the value is the half that stays whole ({:?} at {w}x{h})",
                        mv.name
                    );
                }
            }
        });
    }

    /// **Nothing on this sheet clips horizontally.** A ledger row is two
    /// plain strings, the label from the column's left edge and the value
    /// flushed to its right one, so a row wider than its column used to draw
    /// over the neighbour or off the window in silence.
    ///
    /// Measured against the column `ManifestLayout` really gives GEAR, on both
    /// pages, at every window in `CENSUS_WINDOWS`. It is the widest row on the
    /// sheet by a distance and the only one that has ever needed cutting.
    #[test]
    fn no_gear_row_overflows_its_column() {
        let game = census_game();
        let (slot, _, _) = worst_equipment_row(&game);
        let SectionRow::Stat(label, value) = equip_row(&slot) else {
            panic!("an equipment row is a stat row");
        };

        with_painter(|p| {
            for (w, h) in CENSUS_WINDOWS {
                let m = ui_metrics(h);
                for (who, rect) in equipment_rects(&game, w, h) {
                    let row = fit_ledger_row(p, &label, &value, rect.w, false, &m);
                    let label_w = p.measure_ui_advance(&row.label, row.size);
                    assert!(
                        label_w + row.value_w <= rect.w,
                        "the widest GEAR row overflows its column by {:.0}px on the {who} \
                         page at {w}x{h}:\n{}\n{}",
                        label_w + row.value_w - rect.w,
                        row.label,
                        row.value
                    );
                }
            }
        });
    }

    /// A `StatsFace` around `view` without a `Game`: a roster of `subjects`
    /// rows with the view's subject first, every ranked stat placed, and the
    /// builders' real output for the rest.
    fn stats_face(game: &Game, view: ManifestView, subjects: usize) -> stats::StatsFace {
        use feral_processes_engine::{CallToAct, RankRow, RosterRow, SubjectRanks};
        let roster: Vec<RosterRow> = (0..subjects)
            .map(|i| RosterRow {
                entity: if i == 0 {
                    view.entity
                } else {
                    Entity::from_raw_u32(i as u32 + 100).expect("a valid entity index")
                },
                name: "Cascade Logic Longname".repeat(2),
                glyph: 'x',
                color: GlyphColor::Green,
                level: 99,
                in_party: i < 3,
                needs_you: (i % 2 == 0).then_some(CallToAct::Idle),
            })
            .collect();
        let rank = |stat| RankRow {
            stat,
            place: 17,
            of: subjects as u32,
            pips: 3,
        };
        let faces = stats::faces_of(
            matches!(view.subject, ManifestSubject::Player(_)),
            matches!(view.subject, ManifestSubject::Program(_)),
        );
        stats::StatsFace {
            ranks: Some(SubjectRanks {
                rows: [
                    RankedStat::Damage,
                    RankedStat::Attack,
                    RankedStat::Mitigation,
                    RankedStat::Power,
                    RankedStat::Potential,
                    RankedStat::WorkAptitude,
                ]
                .map(rank)
                .to_vec(),
            }),
            call: Some(CallToAct::PerkPoints(12)),
            sections: sections_for(game, &view),
            band: band_for(&view, &roster),
            tags: header_tags(&view, true),
            keys: keybar::KeybarState {
                cyclable: true,
                faces: !faces.is_empty(),
                remembers: true,
                watchable: true,
                level_up: Some(false),
                points: true,
            },
            faces,
            roster,
            view,
        }
    }

    /// The fullest page of each kind: a kitted, perked, developed, posted
    /// subject, against a forty-strong roster.
    fn fullest_faces(game: &Game) -> Vec<(&'static str, stats::StatsFace)> {
        let mut player = plain_player();
        player.perks = vec![("Obfuscation".to_string(), 2); 4];
        player.party = (0..5)
            .map(|i| feral_processes_engine::CompanionInfo {
                entity: Entity::from_raw_u32(i + 100).expect("a valid entity index"),
                name: "Cascade Logic Longname".to_string(),
                hp: 1234,
                max_hp: 1234,
                atk: 99,
                mitigation: 90,
                power: 12,
                ability: "priority_boost and a long ability".to_string(),
                gear: String::new(),
            })
            .collect();
        let kit =
            || -> Vec<ManifestEquipSlot> { (0..3).map(|_| worst_equipment_row(game).0).collect() };
        let mut pview = player_view(player);
        pview.equipment = kit();
        pview.routines = (0..6)
            .map(|index| feral_processes_engine::RoutineSlotView {
                index,
                ability: None,
                name: "(empty)".to_string(),
                description: String::new(),
                fixed: false,
                unseen: index == 0,
            })
            .collect();
        let program = worst_case_owned(
            // The widest move row the catalogue can build leads, because
            // MOVES names only the first.
            {
                let mut moves: Vec<MoveDef> = game
                    .species_defs()
                    .into_iter()
                    .flat_map(|d| d.moves.clone())
                    .collect();
                let width = |mv: &MoveDef| match move_row(mv) {
                    SectionRow::Stat(l, v) => l.chars().count() + v.chars().count(),
                    _ => 0,
                };
                moves.sort_by_key(|mv| std::cmp::Reverse(width(mv)));
                moves.truncate(3);
                moves
            },
            vec![
                memory("Mauled by", Some("Husk"), -8.0),
                memory("Jammed here", Some("Lathe"), -4.0),
            ],
        );
        let mut gview = program_view(program, kit());
        gview.level = Some(7);
        gview.xp = Some((40, 100));
        vec![
            ("player", stats_face(game, pview, 40)),
            ("program", stats_face(game, gview, 40)),
        ]
    }

    /// **The whole STATS face, painted at every census window.** Every piece
    /// of ink lies inside the frame, and no two pieces of text overlap. The
    /// second is what catches a label running into its value, a column
    /// spilling into the next, or a tag line running under the call to act —
    /// none of which a per-row measurement of the same strings can see,
    /// because it is the *placement* that collides.
    #[test]
    fn the_stats_face_stays_in_its_frame_and_overlaps_nothing() {
        let game = census_game();
        for (w, h) in CENSUS_WINDOWS {
            let m = ui_metrics(h);
            let frame = frame_rect(w, h);
            for (who, face) in fullest_faces(&game) {
                let (_, shapes) = with_painter(|p| stats::paint_stats(frame, &face, p, &m));
                let boxes = crate::paint::painted_text_boxes(&shapes);
                let drawn: String = boxes.iter().map(|(_, t, _)| t.as_str()).collect();
                for part in ["FIGHT", "Esc", "INTEG", "Tab"] {
                    assert!(drawn.contains(part), "{who} at {w}x{h} never drew {part}");
                }
                for (_, text, b) in &boxes {
                    assert!(
                        b.x >= frame.x - 1.0
                            && b.y >= frame.y - 1.0
                            && b.x + b.w <= frame.x + frame.w + 1.0
                            && b.y + b.h <= frame.y + frame.h + 1.0,
                        "{who} at {w}x{h}: {text:?} at {b:?} leaves the frame {frame:?}"
                    );
                }
                // A run of `█` is a meter, and a block glyph's ink is taller
                // than the row it sits in at the larger fonts: the PARTY
                // table's bars touch by design, like the bars they are.
                let boxes: Vec<_> = boxes
                    .into_iter()
                    .filter(|(_, text, _)| !text.chars().all(|c| c == '█'))
                    .collect();
                for (i, (_, a_text, a)) in boxes.iter().enumerate() {
                    for (_, b_text, b) in &boxes[i + 1..] {
                        let overlap_w = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
                        let overlap_h = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
                        assert!(
                            overlap_w <= 1.0 || overlap_h <= 1.0,
                            "{who} at {w}x{h}: {a_text:?} and {b_text:?} overlap ({a:?} {b:?})"
                        );
                    }
                }
            }
        }
    }

    /// The LATELY band puts each value just past the widest label, and the
    /// widest memory label the shipped catalogue can build is never cut to
    /// do it.
    #[test]
    fn no_lately_row_is_cut_to_fit_the_band() {
        let game = census_game();
        let mut subjects: Vec<String> = game
            .species_defs()
            .into_iter()
            .map(|d| d.name.clone())
            .chain(game.structure_defs().into_iter().map(|d| d.name.clone()))
            .collect();
        subjects.push("the base at (-128, -128)".to_string());
        let widest_subject = subjects
            .iter()
            .max_by_key(|s| s.chars().count())
            .expect("the shipped catalogues are not empty")
            .clone();
        let rows: Vec<MemoryRow> = game
            .memory_defs()
            .into_iter()
            .map(|def| memory(&def.name, Some(&widest_subject), -99.0))
            .collect();
        assert!(!rows.is_empty(), "the shipped catalogue has memories");

        with_painter(|p| {
            for (w, h) in CENSUS_WINDOWS {
                let m = ui_metrics(h);
                let char_w = p.measure_ui_advance("M", m.font_size);
                let band_w = ManifestLayout::new(
                    frame_rect(w, h),
                    &m,
                    char_w,
                    layout::Face::Stats,
                    WORST_RAIL_ROWS,
                    2,
                )
                .bands
                .w;
                for entry in &rows {
                    let band = mood_rows(&ManifestMood {
                        sum: -99.0,
                        band: morale_band(-99.0),
                        memories: vec![entry.clone(), entry.clone()],
                    });
                    let split = ledger::lately_split(p, &band, band_w, char_w, &m);
                    for row in &band {
                        let SectionRow::Stat(label, value) = row else {
                            panic!("a LATELY row is a stat row");
                        };
                        let label_w = p.measure_ui_advance(label, m.font_size);
                        let value_w = p.measure_ui_advance(value, m.font_size);
                        assert!(
                            label_w <= split - char_w,
                            "{label} is cut to fit the LATELY band at {w}x{h}"
                        );
                        assert!(
                            split + value_w <= band_w,
                            "{value} overruns the LATELY band at {w}x{h}"
                        );
                    }
                }
            }
        });
    }

    /// **The box is gated on the store, not on ownership.** A wild program
    /// has no `Memories` component, so `Game::manifest` hands the renderer
    /// `None` and the box is absent — which is also what an install with
    /// `assets/memories/` deleted looks like from the *other* direction: the
    /// store is still there, so the box stays and says nothing has happened.
    /// Those two states must not collapse into one, or deleting the
    /// catalogue would silently take a box off every roster page.
    #[test]
    fn the_lately_band_follows_the_store_and_not_the_catalogue() {
        let wild = program_view(plain_program(6, 6), Vec::new());
        assert!(
            band_for(&wild, &[]).is_none(),
            "a wild program has no store and so no LATELY band"
        );

        let empty = program_view(owned_program(Vec::new()), Vec::new());
        let Some(Band::Lately(box_rows)) = band_for(&empty, &[]) else {
            panic!("an owned program with an empty catalogue keeps its band");
        };
        assert_eq!(
            box_rows,
            vec![
                SectionRow::Stat("Mood".to_string(), "uneasy (-11)".to_string()),
                SectionRow::Stat("Carrying".to_string(), "nothing yet".to_string()),
            ],
            "the box says what it knows rather than disappearing"
        );
    }

    /// The mood line leads with the word and carries the number, and each
    /// memory row leads with what it is *about* — the half a player scans a
    /// column for. Only `MANIFEST_MEMORY_ROWS` of them are drawn, however
    /// many the view carries.
    #[test]
    fn the_lately_band_names_the_mood_then_the_strongest_few() {
        let view = program_view(
            owned_program(vec![
                memory("Mauled by", Some("Husk"), -8.0),
                memory("Jammed here", Some("Lathe"), -4.0),
                memory("Hard won", None, 3.0),
            ]),
            Vec::new(),
        );
        let Some(Band::Lately(rows)) = band_for(&view, &[]) else {
            panic!("an owned program draws the LATELY band");
        };

        assert_eq!(
            rows,
            vec![
                SectionRow::Stat("Mood".to_string(), "uneasy (-11)".to_string()),
                SectionRow::Stat("Mauled by — Husk".to_string(), "-8".to_string()),
                SectionRow::Stat("Jammed here — Lathe".to_string(), "-4".to_string()),
            ],
            "the third memory is carried by the view and drawn by neither"
        );
        assert_eq!(rows.len(), 1 + MANIFEST_MEMORY_ROWS);
    }

    /// `manifest_layout::tests::worst_case_program` lists ROUTINES before
    /// MOVES, but `sections_for` does not: `program_sections` pushes MOVES
    /// last, and EQUIPMENT and ROUTINES are appended only after
    /// `program_sections` returns (see `sections_for`'s own doc).
    ///
    /// **That drift stopped being free when MOVES lost its `full_width`
    /// flag.** While it was the page's band, `best_column_split` filtered it
    /// out before packing the columned rest and its position could not change
    /// anything. It is now one columned box among eight, so its place in the
    /// sequence decides which column it lands in whenever two partitions tie
    /// — which is exactly what `columned_sections_fill_left_then_right`
    /// pins. This project has previously shipped a layout fixture that
    /// drifted from what the renderer emits and hid a real overflow behind a
    /// green suite; pinning the real sequence here is what catches that.
    ///
    /// The gear is what makes this a program's *worst* case rather than a
    /// typical one: EQUIPMENT was a player-only box until any program the
    /// player owns could wear gear, and a companion page that silently
    /// stopped emitting it would otherwise pass this test unchanged.
    #[test]
    fn sections_for_emits_moves_before_a_programs_equipment_and_routines() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(
            11,
            feral_processes_engine::DifficultyMode::Forgiving,
            assets,
        )
        .expect("shipped assets load");

        let mut program = owned_program(vec![
            memory("Mauled by", Some("Husk"), -8.0),
            memory("Jammed here", Some("Lathe"), -4.0),
        ]);
        program.base_speed = 14;
        program.analysis = 12;
        program.moves = vec![MoveDef {
            name: "Strike".to_string(),
            power: 5,
            spread: 2,
            effect: None,
            ranged: false,
        }];
        // Every program a player can page to has a class, and a posted one
        // names its structure — so the WORK box is four rows here and two
        // only for an unposted boss.
        program.base_job = Some(AffinityClass::Striker);
        program.post = Some((TaskKind::GatherResource, "Mining Node".to_string()));
        // Developed, because DEVELOPMENT is emitted only for a program that
        // has been — and the worst case is what the layout fixture mirrors.
        program.ring = 3;
        program.level_cap = 12;
        program.talents_earned = 6;
        program.talents_spent = 6;
        let view = program_view(program, vec![worn("WEP"), worn("ARM")]);

        let sections = sections_for(&game, &view);
        let shape: Vec<(&str, Option<LedgerColumn>, usize)> = sections
            .iter()
            .map(|s| (s.title.as_str(), s.column, s.rows.len()))
            .collect();

        use LedgerColumn::{Duty, Fight, Growth};
        assert_eq!(
            shape,
            vec![
                ("FIGHT", Some(Fight), 4),
                ("MOVES", Some(Fight), 1),
                ("DEVELOPMENT", Some(Growth), 3),
                ("WORK", Some(Duty), 5),
                ("SPECIES", Some(Duty), 4),
                ("GEAR", Some(Fight), 2),
                ("ROUTINES", Some(Fight), 1),
            ],
            "the program ledger's columns, in the order they stack: {shape:?}"
        );
    }

    /// The rows the roster ranks are exactly the spec's: Damage, Attack,
    /// Mitigation and Power over everyone, Potential and Work aptitude over
    /// programs. A row tagged by mistake draws a pip ruler beside a number
    /// the engine never ranked.
    #[test]
    fn the_ranked_rows_are_exactly_the_specs_set() {
        let game = census_game();
        let ranked = |view: &ManifestView| -> Vec<RankedStat> {
            sections_for(&game, view)
                .iter()
                .flat_map(|s| s.rows.iter())
                .filter_map(|r| match r {
                    SectionRow::Ranked(_, _, stat) => Some(*stat),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(
            ranked(&player_view(plain_player())),
            vec![
                RankedStat::Damage,
                RankedStat::Attack,
                RankedStat::Mitigation,
                RankedStat::Power
            ]
        );
        let mut program = owned_program(vec![]);
        program.potential = Some(feral_processes_engine::ManifestPotential {
            label: "Fine".to_string(),
            percent: 50,
            hp_roll: 1.0,
            atk_roll: 1.0,
            def_roll: 1.0,
            growth_roll: 1.0,
            assembly_roll: 1.0,
            extraction_roll: 1.0,
        });
        assert_eq!(
            ranked(&program_view(program, vec![])),
            vec![
                RankedStat::Damage,
                RankedStat::Attack,
                RankedStat::Mitigation,
                RankedStat::Power,
                RankedStat::Potential,
                RankedStat::WorkAptitude
            ]
        );
    }

    /// PARTY is yourself first, then each companion, with the glyph and level
    /// the roster already derives — `CompanionInfo` carries neither.
    #[test]
    fn the_party_band_lists_you_then_the_companions_off_the_roster() {
        let companion = Entity::from_raw_u32(7).expect("a valid entity index");
        let mut player = plain_player();
        player.party = vec![feral_processes_engine::CompanionInfo {
            entity: companion,
            name: "Scrapper".to_string(),
            hp: 8,
            max_hp: 10,
            atk: 4,
            mitigation: 2,
            power: 12,
            ability: "Rally".to_string(),
            gear: String::new(),
        }];
        let roster = vec![feral_processes_engine::RosterRow {
            entity: companion,
            name: "Scrapper".to_string(),
            glyph: 'S',
            color: GlyphColor::Green,
            level: 3,
            in_party: true,
            needs_you: None,
        }];
        let Some(Band::Party(rows)) = band_for(&player_view(player), &roster) else {
            panic!("a player closes with the PARTY band");
        };
        let who: Vec<(&str, char, u32, bool)> = rows
            .iter()
            .map(|r| (r.name.as_str(), r.glyph, r.level, r.own))
            .collect();
        assert_eq!(
            who,
            vec![("You", '@', 6, true), ("Scrapper", 'S', 3, false)]
        );
        assert_eq!(rows[1].doing, "Rally");
    }

    /// The player's ledger: FIGHT, GEAR and ROUTINES; GROWTH, CLASS and
    /// PERKS; RUN. PARTY is a band under the ledger and not a box.
    #[test]
    fn the_player_ledger_columns() {
        let game = census_game();
        let mut player = plain_player();
        player.perks = vec![("Obfuscation".to_string(), 2)];
        let mut view = player_view(player);
        view.equipment = vec![worn("WEP")];
        view.routines = vec![feral_processes_engine::RoutineSlotView {
            index: 0,
            ability: None,
            name: "(empty)".to_string(),
            description: String::new(),
            fixed: false,
            unseen: false,
        }];

        use LedgerColumn::{Duty, Fight, Growth};
        let sections = sections_for(&game, &view);
        let shape: Vec<(&str, Option<LedgerColumn>)> = sections
            .iter()
            .map(|s| (s.title.as_str(), s.column))
            .collect();
        assert_eq!(
            shape,
            vec![
                ("FIGHT", Some(Fight)),
                ("GROWTH", Some(Growth)),
                ("CLASS · Decompiler", Some(Growth)),
                ("PERKS", Some(Growth)),
                ("RUN", Some(Duty)),
                ("GEAR", Some(Fight)),
                ("ROUTINES", Some(Fight)),
            ],
        );
    }

    /// MOVES names the first move and points at the dossier for the rest.
    #[test]
    fn a_program_with_more_moves_than_rows_names_the_first_and_counts_the_rest() {
        let game = census_game();
        let mv = |name: &str| MoveDef {
            name: name.to_string(),
            power: 5,
            spread: 2,
            effect: None,
            ranged: false,
        };
        let mut program = owned_program(vec![]);
        program.moves = vec![mv("Strike"), mv("Lunge")];
        let view = program_view(program, vec![]);

        let sections = sections_for(&game, &view);
        let moves = sections.iter().find(|s| s.title == "MOVES").unwrap();

        assert_eq!(
            moves.rows,
            vec![
                move_row(&mv("Strike")),
                SectionRow::Note("+1 more on the dossier".to_string())
            ],
            "the second move is counted, not lost"
        );
    }

    /// `n of 6` in the subhead, and the `N slots open` line survives a kit
    /// long enough to need its own `+N more` note.
    #[test]
    fn routines_count_their_filled_slots_and_keep_the_open_line_past_the_cap() {
        let game = census_game();
        let slot = |index, filled: bool| feral_processes_engine::RoutineSlotView {
            index,
            ability: filled.then(|| "decompile".into()),
            name: if filled {
                "Decompile Single"
            } else {
                "(empty)"
            }
            .to_string(),
            description: String::new(),
            fixed: false,
            unseen: false,
        };
        let mut view = program_view(plain_program(6, 6), Vec::new());
        view.routines = (0..12).map(|i| slot(i, i < 7)).collect();
        let sections = sections_for(&game, &view);
        let routines = titled(&sections, "ROUTINES");
        assert_eq!(routines.hint, "7 of 12");
        assert_eq!(
            routines.rows.last(),
            Some(&SectionRow::Callout("5 slots open".to_string()))
        );
        assert!(
            routines
                .rows
                .contains(&SectionRow::Note("+2 more".to_string())),
            "{:?}",
            routines.rows
        );
    }

    /// An unseen slot draws the concealment line — never the blanked name,
    /// never the ability id — because `sections_for` reads nothing but the
    /// `unseen` flag itself. Spec §4 "Concealment"'s inspect-sheet clause.
    #[test]
    fn an_unseen_routine_slot_draws_the_concealment_line() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(
            11,
            feral_processes_engine::DifficultyMode::Forgiving,
            assets,
        )
        .expect("shipped assets load");
        let program = plain_program(14, 12);
        let mut view = program_view(program, Vec::new());
        view.routines = vec![feral_processes_engine::RoutineSlotView {
            index: 0,
            ability: None,
            name: String::new(),
            description: String::new(),
            fixed: false,
            unseen: true,
        }];

        let sections = sections_for(&game, &view);
        let routines = sections
            .iter()
            .find(|s| s.title == "ROUTINES")
            .expect("one unseen slot still emits the ROUTINES box");
        assert!(
            routines.rows.iter().any(|r| matches!(
                r,
                SectionRow::Stat(_, value) if value == "??? — a routine you haven't seen"
            )),
            "an unseen slot must draw the concealment line, not its blanked \
             fields: {:?}",
            routines.rows
        );
    }

    /// A wild program wears nothing, so the box is absent rather than drawn
    /// empty — the same rule the player's page has always followed for a
    /// slot it isn't using.
    #[test]
    fn a_program_wearing_nothing_gets_no_equipment_box() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(
            11,
            feral_processes_engine::DifficultyMode::Forgiving,
            assets,
        )
        .expect("shipped assets load");

        let sections = sections_for(&game, &program_view(plain_program(14, 12), vec![]));
        assert!(
            !sections.iter().any(|s| s.title == "GEAR"),
            "an empty loadout emits no box: {:?}",
            sections
                .iter()
                .map(|s| s.title.as_str())
                .collect::<Vec<_>>()
        );
    }

    /// The post is stated as a labelled row, and the verb is the whole
    /// difference between the two kinds — a guard posted to a Shield Wall
    /// and a worker posted to one mean different things, and a single
    /// "Posted to" for both would erase that.
    #[test]
    fn the_work_box_states_where_a_program_is_posted() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(
            11,
            feral_processes_engine::DifficultyMode::Forgiving,
            assets,
        )
        .expect("shipped assets load");

        let post_rows = |post| {
            let mut program = plain_program(14, 12);
            program.post = post;
            let mut sections = Vec::new();
            program_sections(&mut sections, &game, &program);
            sections
                .iter()
                .find(|s| s.title == "WORK")
                .expect("a WORK box is always emitted")
                .rows
                .iter()
                .filter_map(|r| match r {
                    SectionRow::Stat(label, value)
                        if label == "Posted to" || label == "Guarding" =>
                    {
                        Some((label.clone(), value.clone()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };

        assert_eq!(
            post_rows(Some((TaskKind::GatherResource, "Mining Node".to_string()))),
            vec![("Posted to".to_string(), "Mining Node".to_string())]
        );
        assert_eq!(
            post_rows(Some((TaskKind::Guard, "Shield Wall".to_string()))),
            vec![("Guarding".to_string(), "Shield Wall".to_string())]
        );
        assert!(
            post_rows(None).is_empty(),
            "an idle program has no post to state"
        );
    }

    /// The header's tag line is one unclamped `painter.ui` call, so a run of
    /// tags wider than the header rect runs off it rather than wrapping —
    /// the same hazard `every_upgrade_items_description_fits_the_refactor_
    /// picker` covers on the other screen, and this line had no test at all
    /// until the refactor tags landed on it.
    ///
    /// The worst case is built here rather than sampled from a real `Game`,
    /// because it is a program that is simultaneously high-level, excellently
    /// rolled, maxed on both permanent ceilings, and posted to a long-named
    /// structure — reachable, but not something a fixture would stumble into.
    #[test]
    fn the_widest_header_tag_line_fits_the_header() {
        let tags = [
            "Lv 30".to_string(),
            "Excellent (99%)".to_string(),
            format!("fused {MAX_FUSIONS}/{MAX_FUSIONS}"),
            format!("upgraded {MAX_COMPANION_REFACTORS}/{MAX_COMPANION_REFACTORS}"),
            // The longest activity string a program can report, against the
            // longest-named structure that accepts one.
            "hauling to Recharger Node".to_string(),
            "STUNNED".to_string(),
        ];
        let line = tags.join("   ");

        // Both shipped window sizes. The header is a fraction of the window,
        // so the *small* one is the worst case — and it was the one this
        // census did not measure at all until someone went looking for the
        // headroom a longer tag would have needed.
        for (w, h) in [(1280.0, 720.0), (1440.0, 900.0)] {
            with_painter(|p| {
                let m = ui_metrics(h);
                let l = manifest_layout(w, h, &[], &m);
                // The glyph portrait and a pad sit left of the text; the tag
                // line starts there and has the rest of the header to run into.
                let portrait = p.measure_map("@", m.title() * 2).width + m.pad;
                let room = l.header.w - portrait;
                let drawn = p.measure_ui_advance(&line, m.font_size);
                assert!(
                    drawn <= room,
                    "at {w}x{h} the manifest header's tags overflow it by \
                     {:.0}px ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            });
        }
    }

    /// **The player's sheet is headed by their own name and their class**,
    /// and carries what that class is worth as a row of its own.
    ///
    /// The title's parenthetical is the same slot a program's *species*
    /// fills, which is the question it answers for either subject: what
    /// kind of thing is this. The bonus goes in a row rather than the
    /// title because the title has no width for it and because it is a
    /// figure and not a name.
    #[test]
    fn the_player_sheet_is_headed_by_their_name_and_class() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(
            23,
            feral_processes_engine::DifficultyMode::Forgiving,
            assets,
        )
        .expect("shipped assets load");

        let mut view = player_view(plain_player());
        view.name = "Kestrel".to_string();
        assert_eq!(subject_title(&view), "Kestrel  (Decompiler)");

        let growth = sections_for(&game, &view);
        let class = titled(&growth, "CLASS · Decompiler");
        assert_eq!(stat_value(class, "Drain"), Some("x1.25"));
        assert_eq!(stat_value(class, "Damage"), Some("x0.85"));

        // A classless run — every save from before character creation, and
        // every `Game::new` — heads itself with the name alone and grows
        // no row. Without this arm the fallback could be dropped and only
        // an old save would find out.
        let mut plain = plain_player();
        plain.class = None;
        let mut view = player_view(plain);
        view.name = "You".to_string();
        assert_eq!(subject_title(&view), "You");
        assert!(
            sections_for(&game, &view)
                .iter()
                .all(|s| !s.title.starts_with("CLASS")),
            "a classless run grows no CLASS box"
        );
    }

    /// A player with nothing optional set — no perks, no party — so a test
    /// naming a box is naming one this fixture actually produces.
    fn plain_player() -> PlayerManifest {
        PlayerManifest {
            power: 60.0,
            max_power: 100.0,
            decompiler: 3,
            perk_points: 1,
            perks: Vec::new(),
            position: (4, 9),
            zone: 2,
            pet_count: 1,
            pet_capacity: 4,
            cargo_used: 12,
            party: Vec::new(),
            credits: 250,
            portal_fragments: 7,
            difficulty: DifficultyMode::Permadeath,
            cycle: 4310,
            active_contracts: 2,
            // A class, because every run started through the wizard has
            // one and the sheet's title and its bonus row both read it —
            // a `None` fixture would measure a page the player never sees.
            class: Some(feral_processes_engine::PlayerClassView {
                name: "Decompiler".to_string(),
                bonuses: "Drain x1.25  Damage x0.85".to_string(),
            }),
        }
    }

    fn player_view(player: PlayerManifest) -> ManifestView {
        ManifestView {
            entity: Entity::PLACEHOLDER,
            name: "You".to_string(),
            glyph: '@',
            color: GlyphColor::White,
            level: Some(6),
            xp: Some((40, 100)),
            hp: 30,
            max_hp: 40,
            atk: 9,
            mitigation: 12,
            damage: "4–9".to_string(),
            power: 44,
            accuracy: 14.0,
            evasion: 11.5,
            statuses: Vec::new(),
            routines: Vec::new(),
            equipment: Vec::new(),
            subject: ManifestSubject::Player(player),
        }
    }

    fn stat_value<'a>(section: &'a Section, label: &str) -> Option<&'a str> {
        section.rows.iter().find_map(|r| match r {
            SectionRow::Stat(l, v) | SectionRow::Ranked(l, v, _) | SectionRow::Alert(l, v)
                if l == label =>
            {
                Some(v.as_str())
            }
            _ => None,
        })
    }

    fn titled<'a>(sections: &'a [Section], title: &str) -> &'a Section {
        sections
            .iter()
            .find(|s| s.title == title)
            .unwrap_or_else(|| panic!("the page emits a {title} box"))
    }

    /// Both halves in one test on purpose. The player half alone passes
    /// against a version that pushes the two rows unconditionally, and that
    /// version overflows the program page at 1280x720 — where the fixture in
    /// `manifest_layout` would catch it only if someone remembered to widen
    /// `worst_case_program` to match.
    #[test]
    fn the_to_hit_pair_is_on_the_player_page_and_not_the_program_page() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(
            21,
            feral_processes_engine::DifficultyMode::Forgiving,
            assets,
        )
        .expect("shipped assets load");

        let player = sections_for(&game, &player_view(plain_player()));
        let combat = titled(&player, "FIGHT");
        assert_eq!(stat_value(combat, "Accuracy"), Some("14.0"));
        assert_eq!(
            stat_value(combat, "Evasion"),
            Some("11.5"),
            "one decimal, or a stat sheet quotes a number the attack roll              does not use"
        );
        assert_eq!(
            combat.rows.len(),
            MAX_SECTION_ROWS,
            "COMBAT sits exactly on the cap, which is safe only while its              row list stays fixed-length"
        );

        let program = sections_for(&game, &program_view(plain_program(14, 12), Vec::new()));
        let combat = titled(&program, "FIGHT");
        assert_eq!(combat.rows.len(), 4);
        assert_eq!(stat_value(combat, "Accuracy"), None);
        assert_eq!(
            stat_value(combat, "Evasion"),
            None,
            "the program page has 17.3px of clearance at 1280x720 against a              10px floor — one more row anywhere on it overflows"
        );
    }

    #[test]
    fn the_run_box_says_what_the_run_holds() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(
            22,
            feral_processes_engine::DifficultyMode::Forgiving,
            assets,
        )
        .expect("shipped assets load");

        let sections = sections_for(&game, &player_view(plain_player()));
        let run = titled(&sections, "RUN");
        assert_eq!(stat_value(run, "Credits"), Some("250"));
        assert_eq!(stat_value(run, "Portal Fragments"), Some("7"));
        assert_eq!(
            stat_value(run, "Difficulty"),
            Some("Permadeath"),
            "the sheet has to use the word the difficulty picker uses"
        );
        assert_eq!(stat_value(run, "Cycle"), Some("4310"));
        assert_eq!(stat_value(run, "Contracts"), Some("2"));
        assert_eq!(stat_value(run, "Cargo carried"), Some("12"));
        assert_eq!(stat_value(run, "Position"), Some("4, 9"));
        assert_eq!(run.rows.len(), 7, "one row under the cap, and no note");
    }

    /// The footer is the only place `w` is advertised, so it has to be
    /// gated on the same answer `App::start_watching` refuses on — a key
    /// offered on a party member's sheet and then refused reads as the
    /// feature being broken, and one never offered at all reads as it not
    /// existing.
    #[test]
    fn the_footer_offers_w_exactly_when_watching_would_work() {
        let m = ui_metrics(900.0);
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut game = feral_processes_engine::Game::new(
            7,
            feral_processes_engine::DifficultyMode::Forgiving,
            &root.join("assets"),
        )
        .expect("the shipped assets must load");
        let subject = game.manifest_subjects()[0];

        let footer = |watchable| {
            let mut game = feral_processes_engine::Game::new(
                7,
                feral_processes_engine::DifficultyMode::Forgiving,
                &root.join("assets"),
            )
            .expect("the shipped assets must load");
            let (_, shapes) = with_painter(|p| {
                draw_manifest(
                    &mut game,
                    Some(subject),
                    ManifestNav {
                        cyclable: false,
                        back_to_list: false,
                        watchable,
                        tab: ManifestTab::Stats,
                        level_up: None,
                    },
                    None,
                    p,
                    &m,
                );
            });
            crate::paint::painted_text(&shapes).join("")
        };

        assert!(
            footer(true).contains("W watch"),
            "the key has to be on the sheet or nobody finds it"
        );
        assert!(
            !footer(false).contains("W watch"),
            "and must not be offered where it would be refused"
        );
    }

    /// The "Escaped" marker rides the player's own header and appears only
    /// once the story is complete.
    #[test]
    fn the_players_sheet_says_escaped_only_after_the_exit() {
        use feral_processes_engine::{DifficultyMode, Game, save};
        let m = ui_metrics(900.0);
        let assets = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let sheet = |escaped: bool| {
            let mut game =
                Game::new(7, DifficultyMode::Forgiving, &assets).expect("shipped assets");
            let path = std::env::temp_dir().join(format!("gui_escaped_{escaped}.sav"));
            game.save(&path).unwrap();
            let mut data = save::load_from_file(&path).unwrap();
            data.player.phase_keys.story_complete = escaped;
            save::save_to_file(&path, &data).unwrap();
            let mut game = Game::load(&path, &assets).unwrap();
            let _ = std::fs::remove_file(&path);
            let subject = game.manifest_subjects()[0];
            let (_, shapes) = with_painter(|p| {
                draw_manifest(
                    &mut game,
                    Some(subject),
                    ManifestNav {
                        cyclable: false,
                        back_to_list: false,
                        watchable: false,
                        tab: ManifestTab::Stats,
                        level_up: Some(false),
                    },
                    None,
                    p,
                    &m,
                );
            });
            crate::paint::painted_text(&shapes).join("|")
        };
        assert!(sheet(true).contains("Escaped"));
        assert!(!sheet(false).contains("Escaped"));
    }

    /// `[D]` works on every sheet — `Game::dossier_report` answers for any
    /// body still standing — so the footer offers it unconditionally, and
    /// the footer is the only place a player can learn the page exists.
    #[test]
    fn the_footer_always_offers_the_dossier() {
        for cyclable in [false, true] {
            for back_to_list in [false, true] {
                for watchable in [false, true] {
                    let nav = ManifestNav {
                        cyclable,
                        back_to_list,
                        watchable,
                        tab: ManifestTab::Stats,
                        level_up: None,
                    };
                    assert!(footer_text(&nav, false).contains("[D] dossier"));
                }
            }
        }
    }

    /// `[L]` is drawn only on your own sheet, in the footer's colour when it
    /// would open the page and greyed when there is nothing to spend.
    #[test]
    fn the_level_up_hint_greys_rather_than_hides() {
        let nav = |level_up| ManifestNav {
            cyclable: false,
            back_to_list: false,
            watchable: false,
            tab: ManifestTab::Stats,
            level_up,
        };
        assert_eq!(level_up_hint(&nav(None)), None);
        assert_eq!(
            level_up_hint(&nav(Some(true))),
            Some((LEVEL_UP_HINT, TEXT_DIM))
        );
        assert_eq!(
            level_up_hint(&nav(Some(false))),
            Some((LEVEL_UP_HINT, hud::palette::FAINT))
        );
    }

    /// `[R]` is offered exactly when `ManifestView::remembers` says there is
    /// a page behind it, which is what app-core refuses on.
    #[test]
    fn the_footer_offers_memories_only_when_the_subject_remembers() {
        let nav = ManifestNav {
            cyclable: false,
            back_to_list: false,
            watchable: false,
            tab: ManifestTab::Stats,
            level_up: None,
        };
        assert!(footer_text(&nav, true).contains("[R] memories"));
        assert!(!footer_text(&nav, false).contains("[R]"));
    }

    /// A seated program's DEVELOPMENT box carries its bank, says when the
    /// bank is held and hints `[H]` and `[S]`; an unseated, undeveloped one
    /// has no such box.
    #[test]
    fn a_seated_programs_development_box_carries_its_points() {
        let game = census_game();
        let mut sections = Vec::new();
        program_sections(&mut sections, &game, &owned_program(vec![]));
        let dev = sections
            .iter()
            .find(|s| s.title == "DEVELOPMENT")
            .expect("a seated program draws DEVELOPMENT");
        assert!(
            dev.rows
                .contains(&SectionRow::Stat("Points".into(), "3 (holding)".into())),
            "{:?}",
            dev.rows
        );
        assert_eq!(dev.hint, "H hold · S spend");

        let mut wild = Vec::new();
        program_sections(&mut wild, &game, &plain_program(6, 6));
        assert!(wild.iter().all(|s| s.title != "DEVELOPMENT"));
    }

    /// The footer is one unwrapped line, so with every key offered it has to
    /// fit the frame at the narrowest supported window and the largest font.
    #[test]
    fn the_fullest_footer_fits_the_narrowest_window() {
        let program = ManifestNav {
            cyclable: true,
            back_to_list: true,
            watchable: true,
            tab: ManifestTab::Stats,
            level_up: None,
        };
        // Your own sheet is the only one with `[L]`, and never has `[W]` (the
        // player walks no base: `Game::program_role` is `None` untamed) or
        // `[R]` (only an owned program remembers).
        let own = ManifestNav {
            watchable: false,
            level_up: Some(true),
            ..program
        };
        let footers = [
            footer_text(&program, true),
            format!("{}{FOOTER_GAP}{LEVEL_UP_HINT}", footer_text(&own, false)),
        ];
        for h in [720.0, 1080.0, 1440.0] {
            let m = ui_metrics(h);
            let l = manifest_layout(1280.0, h, &[], &m);
            for footer in &footers {
                let (width, _) = with_painter(|p| p.measure_ui_advance(footer, m.small()));
                assert!(
                    width <= l.footer.w,
                    "at 1280x{h} `{footer}` is {width}px against {}px",
                    l.footer.w
                );
            }
        }
    }

    /// The POTENTIAL box names both build rolls in full, on their own rows,
    /// and trims nothing to do it.
    ///
    /// The "+N more" assertion is the point: `section_rows_capped` is what
    /// this box switched to, and a cap set one short would silently swallow
    /// a roll and read as "that's all of them".
    #[test]
    fn the_potential_box_lists_both_build_rolls() {
        let mut program = plain_program(10, 10);
        program.potential = Some(feral_processes_engine::views::ManifestPotential {
            hp_roll: 1.11,
            atk_roll: 1.06,
            def_roll: 0.94,
            growth_roll: 1.16,
            assembly_roll: 1.19,
            extraction_roll: 0.81,
            percent: 71,
            label: "Above Average".to_string(),
        });

        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(
            11,
            feral_processes_engine::DifficultyMode::Forgiving,
            assets,
        )
        .expect("shipped assets load");
        let mut sections = Vec::new();
        program_sections(&mut sections, &game, &program);
        let potential = sections
            .iter()
            .find(|s| s.title == "POTENTIAL")
            .expect("the box is emitted");

        let labels: Vec<&str> = potential
            .rows
            .iter()
            .filter_map(|r| match r {
                SectionRow::Stat(label, _) => Some(label.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            labels.contains(&"Assembly roll") && labels.contains(&"Extraction roll"),
            "the build rolls are not named: {labels:?}"
        );

        let values: Vec<&str> = potential
            .rows
            .iter()
            .filter_map(|r| match r {
                SectionRow::Stat(_, value) => Some(value.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            values.iter().any(|v| v.starts_with("1.19"))
                && values.iter().any(|v| v.starts_with("0.81")),
            "the two rolls carry the wrong figures: {values:?}"
        );

        assert!(
            !potential
                .rows
                .iter()
                .any(|r| matches!(r, SectionRow::Note(n) if n.contains("more"))),
            "a roll was silently trimmed off the box"
        );
    }
}
