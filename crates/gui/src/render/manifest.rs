//! The manifest — one read-only stat sheet for the player, a program you own,
//! or a wild one.

use super::bars::*;
use super::manifest_layout::*;
use super::popup::*;
use super::*;
use feral_processes_engine::species::{AffinityClass, MoveDef};
use feral_processes_engine::{
    ManifestEquipSlot, ManifestSubject, ManifestView, PlayerManifest, ProgramManifest,
};

/// How big the header glyph is drawn, relative to the UI title size — enough
/// to read as a portrait rather than another line of text, and sized to span
/// the header's two text lines without spilling into the meters below (the
/// header is `HEADER_ROWS` × `line_height` tall, which is a hair over twice
/// `m.title()`).
const HEADER_GLYPH_SCALE: u16 = 2;

/// What the manifest's footer can offer, which depends on how the screen was
/// opened. Bundled rather than passed as two loose bools, which read as
/// `true, false` at the call site and say nothing.
pub(super) struct ManifestNav {
    /// ←/→ page between subjects. False for a wild program, which isn't in
    /// the owned list and so has nowhere to page to.
    pub(super) cyclable: bool,
    /// Esc returns to the picker rather than to the map.
    pub(super) from_picker: bool,
}

pub(super) fn draw_manifest(
    game: &mut Game,
    entity: Option<Entity>,
    nav: ManifestNav,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(view) = entity.and_then(|e| game.manifest(e)) else {
        draw_popup(
            "Manifest",
            PopupSize::Small,
            &[text_row("That program is gone. Esc to go back.")],
            painter,
            m,
        );
        return;
    };

    let meters = meter_rows(&view);
    let sections = sections_for(game, &view);
    let l = manifest_layout(
        painter.screen_w(),
        painter.screen_h(),
        meters.len(),
        &sections,
        m,
    );

    painter.rect(l.frame.x, l.frame.y, l.frame.w, l.frame.h, PANEL_BG);
    painter.rect_lines(l.frame.x, l.frame.y, l.frame.w, l.frame.h, 2.0, BORDER);

    draw_header(&view, l.header, painter, m);
    for (rect, meter) in l.meters.iter().zip(&meters) {
        let g = BarGeometry {
            x: rect.x,
            y: rect.y + m.label() as f32,
            w: rect.w,
        };
        draw_bar(
            g,
            &format!("{}  {}", meter.label, meter.readout),
            meter.value,
            meter.max,
            BarStyle::plain(meter.color),
            painter,
            m,
        );
    }
    for (rect, section) in l.sections.iter().zip(&sections) {
        draw_section(section, *rect, painter, m);
    }

    let mut footer = Vec::new();
    if nav.cyclable {
        footer.push("←/→ other programs");
    }
    footer.push(if nav.from_picker {
        "Esc back to list"
    } else {
        "Esc back"
    });
    painter.ui(
        footer.join("      "),
        l.footer.x,
        l.footer.y + m.font_size as f32,
        m.small(),
        TEXT_DIM,
    );
}

/// One meter on the sheet.
struct Meter {
    label: &'static str,
    readout: String,
    value: f32,
    max: f32,
    color: Color,
}

fn meter_rows(view: &ManifestView) -> Vec<Meter> {
    let mut meters = vec![Meter {
        label: "INTEGRITY",
        readout: format!("{}/{}", view.hp, view.max_hp),
        value: view.hp as f32,
        max: view.max_hp.max(1) as f32,
        color: GREEN,
    }];
    if let Some((xp, to_next)) = view.xp {
        meters.push(Meter {
            label: "EXPERIENCE",
            readout: format!("{xp}/{to_next}"),
            value: xp as f32,
            max: to_next.max(1) as f32,
            color: CYAN,
        });
    }
    // Needs are player-only — no creature in the sim carries `Needs`.
    if let ManifestSubject::Player(p) = &view.subject {
        meters.push(Meter {
            label: "POWER",
            readout: format!("{:.0}/100", p.hunger),
            value: p.hunger,
            max: 100.0,
            color: YELLOW,
        });
        meters.push(Meter {
            label: "FATIGUE",
            readout: format!("{:.0}/100", p.fatigue),
            value: p.fatigue,
            max: 100.0,
            color: BLUE,
        });
    }
    meters
}

fn draw_header(view: &ManifestView, rect: Rect, painter: &Painter, m: &Metrics) {
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

    let boss = matches!(&view.subject, ManifestSubject::Program(p) if p.is_boss);
    let rarity = match &view.subject {
        ManifestSubject::Program(p) => p.rarity,
        ManifestSubject::Player(_) => Rarity::Ordinary,
    };
    let species = match &view.subject {
        ManifestSubject::Program(p) => p.species_name.clone(),
        ManifestSubject::Player(_) => None,
    };
    let title = match species {
        Some(s) => format!("{}  ({s})", view.name),
        None => view.name.clone(),
    };
    // Both flags, and the tier keeps its own colour claim: a boss reads red
    // and an Overclocked spawn gold, so a program that is both would have to
    // give one up if this were a single string in a single colour. `[BOSS]`
    // wins the title because it is the one that decides whether to open the
    // fight; the tier follows it, drawn in the same silver/gold the map's
    // bar uses so the two channels agree.
    let head = format!("{title}{}", if boss { "  [BOSS]" } else { "" });
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
            text_x + painter.measure_ui(&head, m.title()).width,
            rect.y + m.title() as f32,
            m.title(),
            tier_color,
        );
    }

    let mut tags: Vec<String> = Vec::new();
    if let Some(level) = view.level {
        tags.push(format!("Lv {level}"));
    }
    match &view.subject {
        ManifestSubject::Program(p) => {
            if let Some(q) = &p.potential {
                tags.push(format!("{} ({}%)", q.label, q.percent));
            }
            if p.fusions > 0 {
                tags.push(format!("fused {}/{}", p.fusions, p.max_fusions));
            }
            if p.refactors > 0 {
                tags.push(format!("upgraded {}/{}", p.refactors, p.max_refactors));
            }
            // Only when it is behind, the way `fused` only shows once it has
            // been. A program level with the zone needs no telling; one that
            // is three doublings back has nothing else on the page saying so,
            // and the bare zone tag on its name reads as decoration without
            // the player's own number beside it.
            if p.zone_tier < p.player_zone {
                tags.push(format!(
                    "zone {} — you're in {}",
                    p.zone_tier, p.player_zone
                ));
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
        }
    }
    if let Some(status) = &view.status_effect {
        tags.push(status.clone());
    }
    painter.ui(
        tags.join("   "),
        text_x,
        rect.y + m.title() as f32 + m.line_height,
        m.font_size,
        TEXT_DIM,
    );
}

fn draw_section(section: &Section, rect: Rect, painter: &Painter, m: &Metrics) {
    painter.rect_lines(rect.x, rect.y, rect.w, rect.h, 1.0, BORDER);
    painter.ui(
        section.title,
        rect.x + m.inset,
        rect.y + m.line_height,
        m.small(),
        CYAN,
    );
    let mut cy = rect.y + m.line_height + m.gap;
    for row in &section.rows {
        cy += section_row_h(m);
        match row {
            SectionRow::Stat(label, value) => {
                painter.ui(label, rect.x + m.inset, cy, m.font_size, TEXT_DIM);
                let dims = painter.measure_ui(value, m.font_size);
                painter.ui(
                    value,
                    rect.x + rect.w - m.inset - dims.width,
                    cy,
                    m.font_size,
                    TEXT,
                );
            }
            SectionRow::Note(text) => {
                painter.ui(text, rect.x + m.inset, cy, m.font_size, TEXT);
            }
        }
    }
}

fn stat(label: impl Into<String>, value: impl Into<String>) -> SectionRow {
    SectionRow::Stat(label.into(), value.into())
}

/// Builds COMBAT, dispatches to `player_sections` or `program_sections` for
/// the subject-specific middle, then appends ROUTINES **last** — after
/// whichever of those two returns, not as one of their own pushes. That
/// makes this function, not either of them, the one place that knows the
/// page's *full* section set.
///
/// `manifest_layout`'s column packer is an exact 2-partition
/// (`best_column_split`), not the order-sensitive greedy it used to be, so
/// the *set* of boxes is what decides whether a page fits — order no
/// longer changes the tallest column's height. Order still decides which
/// specific box lands in which column when two partitions tie (see
/// `best_column_split`'s doc), which is what
/// `columned_sections_fill_left_then_right` pins. Either way, every
/// section this function or its callee pushes must have a matching entry
/// in `manifest_layout::tests::worst_case_program` or `worst_case_player`
/// — the branch's original regression was a missing box, not a wrong row
/// count or a wrong order, and a fixture that omits a box passes every
/// test while the real page still doesn't fit.
fn sections_for(game: &Game, view: &ManifestView) -> Vec<Section> {
    let mut sections = vec![Section {
        title: "COMBAT",
        rows: section_rows(vec![
            stat("Attack", view.atk.to_string()),
            stat("Defense", view.def.to_string()),
            stat("Power", view.power.to_string()),
        ]),
        full_width: false,
    }];
    match &view.subject {
        ManifestSubject::Player(p) => player_sections(&mut sections, p),
        ManifestSubject::Program(p) => program_sections(&mut sections, game, p),
    }
    if !view.routines.is_empty() {
        sections.push(Section {
            title: "ROUTINES",
            rows: section_rows(
                view.routines
                    .iter()
                    .map(|r| stat(format!("{}", r.index + 1), r.name.clone()))
                    .collect(),
            ),
            full_width: false,
        });
    }
    sections
}

/// Every player box is columned — no full-width bands. Six boxes across two
/// columns is what 720px has room for; two of them promoted to bands would
/// cost about 180px the budget doesn't have (a band is as tall as a columned
/// box but consumes a whole row of the grid).
///
/// XP is deliberately not a row here: the Experience meter above already
/// reads `xp/to_next`.
///
/// Every `sections.push` here needs a matching entry in
/// `manifest_layout::tests::worst_case_player` — see `sections_for`'s doc
/// for why. Keep ROUTINES **last** in that fixture too, matching the real
/// page — not because the packer's exact-partition column split cares
/// about order (it doesn't decide whether the page fits), but because
/// `sections_for` appends ROUTINES after this function returns, and a
/// fixture that silently drops or reorders a box is the failure mode this
/// whole file exists to catch.
fn player_sections(sections: &mut Vec<Section>, p: &PlayerManifest) {
    sections.push(Section {
        title: "PROGRESSION",
        rows: section_rows(vec![
            stat("Decompiler", p.decompiler.to_string()),
            stat("Perk points", p.perk_points.to_string()),
            stat("Cargo carried", p.cargo_used.to_string()),
            stat("Position", format!("{}, {}", p.position.0, p.position.1)),
        ]),
        full_width: false,
    });

    if !p.equipment.is_empty() {
        sections.push(Section {
            title: "EQUIPMENT",
            rows: section_rows(p.equipment.iter().map(equip_row).collect()),
            full_width: false,
        });
    }
    if !p.perks.is_empty() {
        sections.push(Section {
            title: "PERKS",
            rows: section_rows(
                p.perks
                    .iter()
                    .map(|(name, level)| stat(name.clone(), format!("Lv {level}")))
                    .collect(),
            ),
            full_width: false,
        });
    }
    if !p.party.is_empty() {
        sections.push(Section {
            title: "PARTY",
            rows: section_rows(
                p.party
                    .iter()
                    .map(|c| {
                        stat(
                            c.name.clone(),
                            format!("HP {}/{}  ATK {}  DEF {}", c.hp, c.max_hp, c.atk, c.def),
                        )
                    })
                    .collect(),
            ),
            full_width: false,
        });
    }
}

fn equip_row(slot: &ManifestEquipSlot) -> SectionRow {
    let mut bonus: Vec<String> = Vec::new();
    if slot.atk != 0 {
        bonus.push(format!("+{} ATK", slot.atk));
    }
    if slot.def != 0 {
        bonus.push(format!("+{} DEF", slot.def));
    }
    if slot.decompiler != 0 {
        bonus.push(format!("+{} DECOMP", slot.decompiler));
    }
    if slot.fusion_tier > 0 {
        bonus.push(format!("T{}", slot.fusion_tier));
    }
    SectionRow::Stat(
        format!("{}: {}", slot.slot, slot.item_name),
        bonus.join(" "),
    )
}

/// Every `sections.push` here needs a matching entry in
/// `manifest_layout::tests::worst_case_program`, at its real cap not a
/// smaller placeholder — see `sections_for`'s doc for why. The affinities
/// regression this screen shipped with was not a wrong row count, it was a
/// missing box: AFFINITIES existed here before the fixture knew a fifth
/// columned box existed at all. Adding a section here without adding it
/// there passes every test and still ships a page that doesn't fit.
fn program_sections(sections: &mut Vec<Section>, game: &Game, p: &ProgramManifest) {
    if let Some(q) = &p.potential {
        sections.push(Section {
            title: "POTENTIAL",
            rows: section_rows(vec![
                stat("HP roll", roll_readout(q.hp_roll)),
                stat("Attack roll", roll_readout(q.atk_roll)),
                stat("Defense roll", roll_readout(q.def_roll)),
                stat("Growth roll", roll_readout(q.growth_roll)),
                stat("Overall", format!("{} ({}%)", q.label, q.percent)),
            ]),
            full_width: false,
        });
    }

    if !p.affinities.is_empty() {
        sections.push(Section {
            title: "AFFINITIES",
            rows: section_rows_capped(
                p.affinities
                    .iter()
                    .map(|&(kind, v)| stat(kind.label(), format!("{v:.2}x")))
                    .collect(),
                MAX_AFFINITY_ROWS,
            ),
            full_width: false,
        });
    }

    let mut species = vec![stat(
        "Habitats",
        if p.habitats.is_empty() {
            "unknown".to_string()
        } else {
            p.habitats
                .iter()
                .map(|b| format!("{b:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        },
    )];
    if let Some(res) = &p.work_resource {
        species.push(stat("Work aptitude", game.item_name(res).to_string()));
    }
    // A boss can't be decompiled at all, so both rows are dropped rather
    // than quoting odds that can never be rolled — and the "needs a taming
    // catalyst" fallback below would be an outright lie about why.
    if !p.is_boss {
        species.push(stat(
            "Decompile difficulty",
            format!("{:.0}%", p.taming_difficulty * 100.0),
        ));
        species.push(stat(
            "Decompile chance now",
            match p.decompile_chance {
                Some(c) => format!("{:.0}%", c * 100.0),
                // Which item is a catalyst is item data, not something a
                // renderer gets to name.
                None => "needs a taming catalyst".to_string(),
            },
        ));
    }
    species.push(stat("Growth", format!("{:.2}x", p.growth_multiplier)));
    sections.push(Section {
        title: "SPECIES",
        rows: section_rows(species),
        full_width: false,
    });

    // What this program is like to *post* somewhere, as opposed to what it
    // is. Its own box rather than two more SPECIES rows because SPECIES was
    // already sitting on `MAX_SECTION_ROWS`, and a row past that cap does
    // not fail — it truncates to "+N more" and the reading silently
    // disappears. Speed moved across rather than being duplicated: it means
    // initiative in a fight and pace at a machine, and one number said twice
    // on one page reads as two different numbers.
    let mut work = vec![
        stat("Speed", p.base_speed.to_string()),
        stat("Analysis", p.base_int.to_string()),
    ];
    if let Some(class) = p.base_job {
        work.push(stat("Base job", base_job_label(class)));
    }
    sections.push(Section {
        title: "WORK",
        rows: section_rows(work),
        full_width: false,
    });

    if !p.moves.is_empty() {
        sections.push(Section {
            title: "MOVES",
            rows: section_rows_capped(p.moves.iter().map(move_row).collect(), MAX_BAND_ROWS),
            full_width: true,
        });
    }
}

/// What a class does when its program is posted to a structure, beside the
/// class' own name — "repair (Medic)".
///
/// Both halves earn their place. The job is what the row is *for*, and the
/// class name is the only place in the game a player meets the vocabulary
/// `assets/species/README.md` is written in; a row saying only "Medic" would
/// be a word with no referent, and one saying only "repair" would leave the
/// five classes unnamed everywhere.
///
/// Exhaustive on purpose: a sixth class must decide what it does at a post
/// before it compiles, which is the same argument `render/stack.rs`'s
/// `cell_mark` makes about a new `CellKind`.
fn base_job_label(class: AffinityClass) -> String {
    let job = match class {
        AffinityClass::Striker | AffinityClass::Saboteur => "none",
        AffinityClass::Bastion => "guard",
        AffinityClass::Medic => "repair",
        AffinityClass::Leech => "extraction",
    };
    format!("{job} ({class:?})")
}

fn move_row(mv: &MoveDef) -> SectionRow {
    let mut tags = vec![format!("pow {}", mv.power)];
    if mv.ranged {
        tags.push("ranged".to_string());
    }
    if let Some(effect) = &mv.effect {
        tags.push(format!(
            "{:?} {:.0}% for {}",
            effect.kind,
            effect.chance * 100.0,
            effect.duration
        ));
    }
    SectionRow::Stat(mv.name.clone(), tags.join(", "))
}

/// A potential roll as a number plus a coarse glance-readable tier. 1.0 is
/// neutral; the roll range is `MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL`.
fn roll_readout(roll: f32) -> String {
    let tier = if roll >= 1.15 {
        "+++"
    } else if roll >= 1.05 {
        "++"
    } else if roll > 0.95 {
        "="
    } else if roll > 0.85 {
        "-"
    } else {
        "--"
    };
    format!("{roll:.2}  {tier}")
}

pub(super) fn draw_manifest_pick(
    game: &mut Game,
    subjects: &[Entity],
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row("Read whose manifest?")];
    for (i, &entity) in subjects.iter().enumerate() {
        let view = game.manifest(entity);
        let icon = view.as_ref().map(|v| (v.glyph, glyph_color(v.color)));
        let label = match view {
            Some(v) => match &v.subject {
                ManifestSubject::Player(_) => format!("You - Lv{}", v.level.unwrap_or(1)),
                ManifestSubject::Program(p) => format!(
                    "{} Lv{} - HP {}/{}  PWR {}{}",
                    v.name,
                    v.level.unwrap_or(1),
                    v.hp,
                    v.max_hp,
                    v.power,
                    p.activity
                        .as_ref()
                        .map(|a| activity_tag(a))
                        .unwrap_or_default()
                ),
            },
            None => "(gone)".to_string(),
        };
        let row = creature_row(format!("[{}] {label}", menu_shortcut(i)), i == selected);
        // A despawned subject has no glyph left to draw, and its row already
        // says "(gone)" — the slot stays reserved so the list keeps its
        // column.
        rows.push(match icon {
            Some((glyph, color)) => with_icon(row, glyph, color),
            None => with_icon(row, ' ', TEXT),
        });
    }
    rows.push(text_row(""));
    rows.push(text_row("Esc to cancel"));
    draw_popup("Manifest", PopupSize::Large, &rows, painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::render::manifest_layout::manifest_layout;
    use crate::text::ui_metrics;

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
    fn plain_program(base_speed: i32, base_int: i32) -> ProgramManifest {
        ProgramManifest {
            species_name: Some("Testmon".to_string()),
            is_hostile: false,
            is_tamed: true,
            is_companion: true,
            is_boss: false,
            activity: None,
            potential: None,
            fusions: 0,
            max_fusions: 3,
            rarity: Rarity::Ordinary,
            refactors: 0,
            max_refactors: 3,
            zone_tier: 1,
            player_zone: 1,
            habitats: vec![],
            moves: vec![],
            work_resource: Some("core_fragment".into()),
            taming_difficulty: 0.5,
            decompile_chance: None,
            growth_multiplier: 1.0,
            base_speed,
            base_int,
            affinities: vec![],
            // The two boss species are the only shipped programs with no
            // class, and a boss cannot be tamed or posted — so a job row is
            // the ordinary case and this fixture is deliberately the one
            // that omits it, the same way it keeps a `work_resource` to
            // hold SPECIES at its worst case.
            base_job: None,
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
                    SectionRow::Stat(label, _) => Some(label.clone()),
                    SectionRow::Note(_) => None,
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
            "base_int is shown as Analysis: {work:?}"
        );

        let species = labels("SPECIES");
        assert!(
            !species.iter().any(|l| l == "Speed"),
            "Speed must not be left behind in SPECIES too: {species:?}"
        );
        assert_eq!(
            species.len(),
            5,
            "a non-boss species with a work_resource is SPECIES' real worst \
             case (Habitats, Work aptitude, Decompile difficulty, Decompile \
             chance now, Growth) — it must land below MAX_SECTION_ROWS: {species:?}"
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

    /// `manifest_layout::tests::worst_case_program` lists ROUTINES before
    /// MOVES, but `sections_for` does not: `program_sections` pushes MOVES
    /// last (it's the full-width band), and ROUTINES is appended only after
    /// `program_sections` returns (see `sections_for`'s own doc). That drift
    /// is currently harmless — MOVES is the only `full_width` box, so
    /// `best_column_split` filters it out before packing the columned two,
    /// and order stops mattering the moment a box leaves that set — but this
    /// project has previously shipped a layout fixture that drifted from
    /// what the renderer actually emits and hid a real overflow behind a
    /// green suite. Pinning the real sequence here is what would catch that
    /// again if a future change ever made full-width order matter.
    #[test]
    fn sections_for_emits_moves_before_routines() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(
            11,
            feral_processes_engine::DifficultyMode::Forgiving,
            assets,
        )
        .expect("shipped assets load");

        let mut program = plain_program(14, 12);
        program.moves = vec![MoveDef {
            name: "Strike".to_string(),
            power: 5,
            effect: None,
            ranged: false,
        }];
        // Every program a player can page to has a class, so the WORK box is
        // three rows here and two only for a boss.
        program.base_job = Some(AffinityClass::Striker);
        let view = ManifestView {
            entity: Entity::PLACEHOLDER,
            name: "Testmon".to_string(),
            glyph: 'x',
            color: GlyphColor::White,
            level: None,
            xp: None,
            hp: 10,
            max_hp: 10,
            atk: 5,
            def: 5,
            power: 15,
            status_effect: None,
            routines: vec![feral_processes_engine::RoutineSlotView {
                index: 0,
                ability: None,
                name: "(empty)".to_string(),
                description: String::new(),
            }],
            subject: ManifestSubject::Program(program),
        };

        let sections = sections_for(&game, &view);
        let shape: Vec<(&str, usize, bool)> = sections
            .iter()
            .map(|s| (s.title, s.rows.len(), s.full_width))
            .collect();

        assert_eq!(
            shape,
            vec![
                ("COMBAT", 3, false),
                ("SPECIES", 5, false),
                ("WORK", 3, false),
                ("MOVES", 1, true),
                ("ROUTINES", 1, false),
            ],
            "sections_for's real emission order — the shape the layout \
             fixture must mirror: {shape:?}"
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
    /// rolled, maxed on both permanent ceilings, far behind the zone, and
    /// posted to a long-named structure — reachable, but not something a
    /// fixture would stumble into.
    #[test]
    fn the_widest_header_tag_line_fits_the_header() {
        let tags = [
            "Lv 30".to_string(),
            "Excellent (99%)".to_string(),
            format!("fused {MAX_FUSIONS}/{MAX_FUSIONS}"),
            format!("upgraded {MAX_COMPANION_REFACTORS}/{MAX_COMPANION_REFACTORS}"),
            "zone 1 — you're in 9".to_string(),
            // The longest activity string a program can report, against the
            // longest-named structure that accepts one.
            "hauling to Recharger Node".to_string(),
            "STUNNED".to_string(),
        ];
        let line = tags.join("   ");

        with_painter(|p| {
            let m = ui_metrics(900.0);
            let l = manifest_layout(1440.0, 900.0, 4, &[], &m);
            // The glyph portrait and a pad sit left of the text; the tag line
            // starts there and has the rest of the header to run into.
            let portrait = p.measure_map("@", m.title() * 2).width + m.pad;
            let room = l.header.w - portrait;
            let drawn = p.measure_ui_advance(&line, m.font_size);
            assert!(
                drawn <= room,
                "the manifest header's tags overflow it by {:.0}px \
                 ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                drawn - room
            );
        });
    }
}
