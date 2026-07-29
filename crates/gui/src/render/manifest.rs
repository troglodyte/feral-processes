//! The manifest — one read-only stat sheet for the player, a program you own,
//! or a wild one.

use super::bars::*;
use super::manifest_layout::*;
use super::popup::*;
use super::*;
use feral_processes_engine::species::MoveDef;
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
    let species = match &view.subject {
        ManifestSubject::Program(p) => p.species_name.clone(),
        ManifestSubject::Player(_) => None,
    };
    let title = match species {
        Some(s) => format!("{}  ({s})", view.name),
        None => view.name.clone(),
    };
    painter.ui_bold(
        format!("{title}{}", if boss { "  [BOSS]" } else { "" }),
        text_x,
        rect.y + m.title() as f32,
        m.title(),
        if boss { RED } else { WHITE },
    );

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
    species.push(stat("Speed", p.base_speed.to_string()));
    sections.push(Section {
        title: "SPECIES",
        rows: section_rows(species),
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
        let label = match game.manifest(entity) {
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
        rows.push(creature_row(
            format!("[{}] {label}", menu_shortcut(i)),
            i == selected,
        ));
    }
    rows.push(text_row(""));
    rows.push(text_row("Esc to cancel"));
    draw_popup("Manifest", PopupSize::Large, &rows, painter, m);
}
