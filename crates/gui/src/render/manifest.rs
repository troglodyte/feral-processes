//! The manifest — one read-only stat sheet for the player, a program you own,
//! or a wild one.

use super::bars::*;
use super::manifest_layout::*;
use super::popup::*;
use super::*;
use feral_processes_engine::components::TaskKind;
use feral_processes_engine::species::{AffinityClass, MoveDef};
use feral_processes_engine::{
    DifficultyMode, ManifestEquipSlot, ManifestSubject, ManifestView, PlayerManifest,
    ProgramManifest,
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
    /// Esc returns to the list this was opened from — the manifest picker or
    /// the roster — rather than to the map. Which of the two is app-core's
    /// business (`ManifestOrigin`); the footer only has to know there is one.
    pub(super) back_to_list: bool,
    /// `w` sends the map's camera to this program and goes back to the map.
    ///
    /// False for anything the sim does not walk — a party member, your
    /// wielded program, a squad away on a sortie, a guard — and for every
    /// program when the party is not in base space, which is where staff
    /// stand. `Game::watch_position` is the one rule; this is its `is_some`.
    pub(super) watchable: bool,
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
    if nav.watchable {
        footer.push("[w] watch");
    }
    footer.push(if nav.back_to_list {
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
    // PowerReserve are player-only — no creature in the sim carries `PowerReserve`.
    if let ManifestSubject::Player(p) = &view.subject {
        meters.push(Meter {
            label: "POWER",
            readout: format!("{:.0}/100", p.power),
            value: p.power,
            max: 100.0,
            color: YELLOW,
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
            SectionRow::Note(text) => {
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

fn stat(label: impl Into<String>, value: impl Into<String>) -> SectionRow {
    SectionRow::Stat(label.into(), value.into())
}

/// Builds COMBAT, dispatches to `player_sections` or `program_sections` for
/// the subject-specific middle, then appends EQUIPMENT and ROUTINES **last**
/// — after whichever of those two returns, not as one of their own pushes.
/// That makes this function, not either of them, the one place that knows the
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
    let mut combat = vec![
        stat("Damage", view.damage.clone()),
        stat("Attack", view.atk.to_string()),
    ];
    // The to-hit half of a fight, on the player's page alone. Not a
    // judgement that a program's odds matter less — the program page's
    // worst case clears its footer by 17.3px against a 10px floor, so one
    // more row anywhere on it overflows at 1280x720. `ManifestView` carries
    // both figures for both subjects precisely so restoring these two rows
    // there is a layout change and not a data change.
    //
    // One decimal because both are halves: `ACCURACY_PER_LEVEL` is 0.5, so
    // a level-1 player reads 11.5 and an integer would quote a number the
    // attack roll does not use.
    if matches!(&view.subject, ManifestSubject::Player(_)) {
        combat.push(stat("Accuracy", format!("{:.1}", view.accuracy)));
        combat.push(stat("Evasion", format!("{:.1}", view.evasion)));
    }
    combat.push(stat("Mitigation", format!("{}%", view.mitigation)));
    combat.push(stat("Power", view.power.to_string()));
    let mut sections = vec![Section {
        title: "COMBAT",
        rows: section_rows(combat),
        full_width: false,
    }];
    match &view.subject {
        ManifestSubject::Player(p) => player_sections(&mut sections, p),
        ManifestSubject::Program(p) => program_sections(&mut sections, game, p),
    }
    // Both subjects, appended here rather than pushed by either arm, for the
    // same reason ROUTINES is: a wearer is a wearer, and one function knows
    // the page's full section set. It was `player_sections`' third push
    // until any program the player owns could wear gear.
    if !view.equipment.is_empty() {
        sections.push(Section {
            title: "EQUIPMENT",
            rows: section_rows(view.equipment.iter().map(equip_row).collect()),
            // **A band on the player's page and a columned box on a
            // program's**, the same split — and the same argument — as the
            // to-hit pair above: the player page has the clearance for a band
            // and the program page has none. A gear row is the widest row on
            // this sheet by a distance, because `Game::copy_name` spends its
            // width on a tier word, a prefix affix, a suffix phrase and a
            // quality figure, and the bonus column is beside it. In a
            // half-width box the affix at the *end* of that name is the first
            // thing `fitted_stat_row` has to cut; across the whole frame
            // nothing a drop can roll needs cutting at all.
            full_width: matches!(&view.subject, ManifestSubject::Player(_)),
        });
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
/// for why. Keep EQUIPMENT and ROUTINES **last** in that fixture too,
/// matching the real page — not because the packer's exact-partition column
/// split cares about order (it doesn't decide whether the page fits), but
/// because `sections_for` appends both after this function returns, and a
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
    // What this run holds, as opposed to what the player *is*. Credits and
    // Portal Fragments are `ItemDef::banked` pools and so sit outside the
    // inventory PROGRESSION's cargo row counts — which is why that row said
    // nothing about either, and why they are here rather than beside it.
    //
    // Trace is deliberately absent: `render/stack.rs` already draws it, and
    // it is underground-only, so a second reading here would be a duplicate
    // that is blank most of the time.
    let run = vec![
        stat("Credits", p.credits.to_string()),
        stat("Portal Fragments", p.portal_fragments.to_string()),
        stat("Difficulty", difficulty_label(p.difficulty)),
        stat("Cycle", p.cycle.to_string()),
        stat("Contracts", p.active_contracts.to_string()),
    ];

    if !p.party.is_empty() {
        sections.push(Section {
            title: "PARTY",
            rows: section_rows(
                p.party
                    .iter()
                    .map(|c| {
                        stat(
                            c.name.clone(),
                            format!(
                                "HP {}/{}  ATK {}  MIT {}%",
                                c.hp, c.max_hp, c.atk, c.mitigation
                            ),
                        )
                    })
                    .collect(),
            ),
            full_width: false,
        });
    }

    sections.push(Section {
        title: "RUN",
        rows: section_rows(run),
        full_width: false,
    });
}

/// The word the difficulty picker uses, so the sheet and the screen that set
/// the mode cannot name it two ways.
///
/// Exhaustive on purpose: a third mode must decide how it reads here before
/// it compiles, the argument `render/stack.rs`'s `cell_mark` makes about a
/// new `CellKind`.
fn difficulty_label(mode: DifficultyMode) -> String {
    match mode {
        DifficultyMode::Permadeath => "Permadeath".to_string(),
        DifficultyMode::Forgiving => "Forgiving".to_string(),
    }
}

fn equip_row(slot: &ManifestEquipSlot) -> SectionRow {
    let mut bonus: Vec<String> = Vec::new();
    // `{:+}` and not a literal `+`: an affix may carry a **negative**
    // component beside its bonus (see `affixes::AffixDef::stats`), and a
    // hardcoded sign renders that as `+-30 DEF`.
    if slot.atk != 0 {
        bonus.push(format!("{:+} ATK", slot.atk));
    }
    if slot.mitigation != 0 {
        bonus.push(format!("{:+} DEF", slot.mitigation));
    }
    if slot.decompiler != 0 {
        bonus.push(format!("{:+} DECOMP", slot.decompiler));
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
    // The header already carries the post as one of a run of tags, where a
    // worker's is the bare structure name and reads as decoration. Stated as
    // a labelled row it reads as the assignment it is — and this is the box
    // about what the program is like to *post*, so it is where a player
    // looking for that answer already is.
    if let Some((kind, structure)) = &p.post {
        work.push(stat(post_label(*kind), structure.clone()));
    }
    // The reserves share this box rather than taking one of their own: the
    // program page has the least clearance in the renderer, and a NEEDS box
    // did not fit at 1280x720 even at two rows. They belong here anyway —
    // this is the box about what the program is like to *post*, and a
    // program that keeps walking off to defragment is exactly that.
    //
    // Trimmed to `MAX_NEED_ROWS` **before** the box's own cap, so a modded
    // catalogue spends its "+N more" on needs rather than pushing the post
    // row off the end. Absent entirely for a program carrying no reserves and
    // for an install with `assets/needs/` deleted.
    for row in p.needs.iter().take(MAX_NEED_ROWS) {
        work.push(stat(
            row.name.clone(),
            match &row.servicing {
                Some(verb) => format!("{} — {verb}", row.band),
                None => row.band.to_string(),
            },
        ));
    }
    if p.needs.len() > MAX_NEED_ROWS {
        work.push(stat(
            "…",
            format!("+{} more", p.needs.len() - MAX_NEED_ROWS),
        ));
    }

    sections.push(Section {
        title: "WORK",
        rows: section_rows(work),
        full_width: false,
    });

    // Only for a program that has been developed, the way the `fused` and
    // `upgraded` header tags only show once they mean something: an
    // undeveloped program's box would be three rows of zero on a page whose
    // column budget is already the tightest thing in the renderer.
    if p.ring > 0 || p.talents_earned > 0 {
        sections.push(Section {
            title: "DEVELOPMENT",
            rows: section_rows(vec![
                stat("Kernel rings", format!("{}/{}", p.ring, p.max_ring)),
                stat("Level ceiling", p.level_cap.to_string()),
                stat(
                    "Talents",
                    format!("{}/{} spent", p.talents_spent, p.talents_earned),
                ),
            ]),
            full_width: false,
        });
    }

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
pub(super) fn base_job_label(class: AffinityClass) -> String {
    let job = match class {
        AffinityClass::Striker | AffinityClass::Saboteur => "none",
        AffinityClass::Bastion => "guard",
        AffinityClass::Medic => "repair",
        AffinityClass::Leech => "extraction",
    };
    format!("{job} ({class:?})")
}

/// What a post *is*, as the row's own label — the structure's name is the
/// value beside it, so "Posted to  Mining Node" and "Guarding  Shield Wall"
/// both read as a sentence. The verb is the whole difference between the two
/// kinds, which is the same distinction `Game::program_activity` draws for
/// its terse one-liner.
///
/// Exhaustive on purpose: a third `TaskKind` must decide how it reads on
/// this row before it compiles.
fn post_label(kind: TaskKind) -> &'static str {
    match kind {
        TaskKind::GatherResource => "Posted to",
        TaskKind::Guard => "Guarding",
        // The value beside it is the cell, from `entity_label` — "Cutting
        // Marked Cell (6, 0)" reads as the sentence the other two do.
        TaskKind::Excavate => "Cutting",
        // The value beside it is `entity_label`'s name for the site —
        // "Building  Depot (under construction)".
        TaskKind::Construct => "Building",
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

/// One row on the manifest picker.
///
/// Returned rather than drawn so its width is measurable without a window —
/// see `no_manifest_pick_row_overflows_its_popup`. The shortcut is passed in
/// for `companion_row_lines`' reason: it is the row's position in the list,
/// which a `ManifestView` has no idea about.
///
/// A program's row carries `ATK` and `MIT` because the picker is a screen for
/// choosing between subjects, and those are what the choice turns on. **The
/// player's row deliberately carries neither**: it quotes no HP and no PWR
/// either, so a lone pair of combat figures would be the only numbers on it
/// and would read as a comparison the row cannot complete.
fn manifest_pick_label(shortcut: char, view: &ManifestView) -> String {
    let body = match &view.subject {
        ManifestSubject::Player(_) => format!("You - Lv{}", view.level.unwrap_or(1)),
        ManifestSubject::Program(p) => format!(
            "{} Lv{} - HP {}/{}  ATK {}  MIT {}%  PWR {}{}",
            view.name,
            view.level.unwrap_or(1),
            view.hp,
            view.max_hp,
            view.atk,
            view.mitigation,
            view.power,
            p.activity
                .as_ref()
                .map(|a| activity_tag(a))
                .unwrap_or_default()
        ),
    };
    format!("[{shortcut}] {body}")
}

pub(super) fn draw_manifest_pick(
    game: &mut Game,
    subjects: &[Entity],
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row("Read whose manifest?")];
    for (i, &entity) in subjects.iter().enumerate() {
        let view = game.manifest(entity);
        let icon = view.as_ref().map(|v| (v.glyph, glyph_color(v.color)));
        let shortcut = menu_shortcut(i);
        let label = match view {
            Some(v) => manifest_pick_label(shortcut, &v),
            None => format!("[{shortcut}] (gone)"),
        };
        let row = creature_row(label, i == selected);
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
    draw_popup("Manifest", PopupSize::Large, &rows, refusal, painter, m);
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
            post: None,
            potential: None,
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
            // Empty on purpose: the NEEDS box is conditional and its own
            // test builds one, so this fixture holds every other box at its
            // worst case without that one moving under it.
            needs: vec![],
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
            status_effect: None,
            routines: vec![feral_processes_engine::RoutineSlotView {
                index: 0,
                ability: None,
                name: "(empty)".to_string(),
                description: String::new(),
            }],
            equipment,
            subject: ManifestSubject::Program(Box::new(program)),
        }
    }

    /// The widest manifest-picker row the game can put on screen, as
    /// `(label, why)`.
    ///
    /// The ingredients are the roster census's, for the reason `test_pet` is
    /// shared: the two lists name the same programs, so a picker census with
    /// a shorter name would be measuring a row the roster has already proved
    /// reachable.
    fn widest_manifest_pick_rows() -> Vec<(String, String)> {
        let name = format!(
            "Overclocked {} 10",
            "M".repeat(feral_processes_engine::MAX_CUSTOM_NAME_LEN)
        );
        let mut program = plain_program(14, 12);
        program.activity = Some("guarding Contract Broker".to_string());
        let mut view = program_view(program, Vec::new());
        view.name = name;
        view.level = Some(6);
        view.hp = 1;
        view.max_hp = 1234;
        view.atk = 1234;
        view.mitigation = feral_processes_engine::tuning::MAX_MITIGATION_PERCENT;
        view.power = 1234;
        vec![(
            manifest_pick_label('a', &view),
            "a posted program with a renamed, overclocked, zone-tagged name".to_string(),
        )]
    }

    /// The picker exists to choose between subjects, so its rows carry the
    /// two figures that choice turns on — in the same words and the same
    /// unit the roster and the fuse picker already use.
    #[test]
    fn a_manifest_pick_row_carries_the_combat_figures() {
        let mut view = program_view(plain_program(14, 12), Vec::new());
        view.name = "Kestrel".to_string();
        view.level = Some(4);
        view.atk = 8;
        view.mitigation = 5;
        let label = manifest_pick_label('a', &view);
        assert!(label.contains("ATK 8"), "{label}");
        assert!(label.contains("MIT 5%"), "{label}");
    }

    /// Nothing clamps a popup row horizontally, and a picker row does not
    /// wrap the way a roster row does — it has no tags to shed — so the
    /// whole label has to fit or its tail leaves the screen.
    #[test]
    fn no_manifest_pick_row_overflows_its_popup() {
        with_painter(|p| {
            let m = ui_metrics(900.0);
            // 0.88 is `PopupSize::Large`'s width fraction, against the
            // 1440x900 geometry `ui_metrics` is calibrated for.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for (label, why) in widest_manifest_pick_rows() {
                // Every row draws through `with_icon`, so it carries the
                // selection prefix and the glyph's reserved slot.
                let drawn = p.measure_ui_advance(format!("     {label}"), m.font_size);
                assert!(
                    drawn <= room,
                    "the widest manifest picker row ({why}) overflows its \
                     popup by {:.0}px ({drawn:.0} into {room:.0}):\n{label}",
                    drawn - room
                );
            }
        });
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
        }
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
            };
            let cells = row.item_name.chars().count();
            if worst.as_ref().is_none_or(|(_, w)| cells > *w) {
                worst = Some((row, cells));
            }
        }
        let (row, _) = worst.expect("the shipped set has equippable gear");
        (row, prefix_word, suffix_word)
    }

    /// Every EQUIPMENT row a *drop* can put on this sheet, paired with the
    /// affix word in its name.
    ///
    /// `Game::grant_gear_drop` rolls **one** affix, so a single word — a
    /// prefix or a suffix — is the shape a player meets before they ever fuse
    /// anything, and it is the shape the bug was reported against. Every
    /// equippable item against every shipped affix, at the top rare tier and
    /// `QUALITY_MAX`, which are the two other axes `copy_name` spends
    /// characters on.
    fn dropped_equipment_rows(game: &Game) -> Vec<(ManifestEquipSlot, String)> {
        use feral_processes_engine::components::Rarity;
        use feral_processes_engine::items::{EquipmentSlot, GearCopy};
        use feral_processes_engine::tuning::QUALITY_MAX;

        let slot = EquipmentSlot::ALL
            .into_iter()
            .max_by_key(|s| s.short_label().chars().count())
            .expect("EquipmentSlot::ALL is not empty");

        let mut rows = Vec::new();
        for def in game
            .item_defs()
            .into_iter()
            .filter(|d| d.equipment.is_some())
        {
            for affix in game.affix_defs() {
                let copy = GearCopy::with_affixes(
                    def.id.clone(),
                    Rarity::ALL[Rarity::ALL.len() - 1],
                    0,
                    vec![affix.id.clone()],
                    QUALITY_MAX,
                );
                let Some(mods) = game.copy_bonus(&copy, 10) else {
                    continue;
                };
                let word = affix
                    .prefix
                    .clone()
                    .or_else(|| affix.suffix.clone())
                    .expect("AffixDef::fault refuses an affix with neither");
                rows.push((
                    ManifestEquipSlot {
                        slot: slot.short_label().to_string(),
                        item_name: game.copy_name(&copy),
                        gear_level: 10,
                        fusion_tier: copy.tier,
                        atk: mods.atk,
                        mitigation: mods.mitigation,
                        decompiler: mods.decompiler,
                    },
                    word,
                ));
            }
        }
        assert!(!rows.is_empty(), "the shipped set has affixable gear");
        rows
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
        let m = ui_metrics(h);
        let mut program = plain_program(14, 12);
        program.base_job = Some(AffinityClass::Striker);
        program.post = Some((TaskKind::GatherResource, "Mining Node".to_string()));
        let kit = || vec![worn("WEP"), worn("ARM"), worn("MOD")];
        let mut player = player_view(plain_player());
        player.equipment = kit();

        let mut out = Vec::new();
        for (who, view, meters) in [
            ("program", program_view(program, kit()), 2),
            ("player", player, 3),
        ] {
            let sections = sections_for(game, &view);
            let l = manifest_layout(w, h, meters, &sections, &m);
            for (rect, s) in l.sections.iter().zip(&sections) {
                if s.title == "EQUIPMENT" {
                    out.push((who, *rect));
                }
            }
        }
        assert_eq!(out.len(), 2, "both pages draw an EQUIPMENT box");
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

    /// **Nothing on this sheet clips horizontally.** `draw_section` draws a
    /// stat row as two plain strings — the label from the box's left inset,
    /// the value flushed to its right one — so a row wider than its box used
    /// to draw over the neighbouring column or off the window entirely, in
    /// silence.
    ///
    /// Measured against the box `manifest_layout` really gives EQUIPMENT, on
    /// both pages, at every window in `CENSUS_WINDOWS`. It is the widest row
    /// on the sheet by a distance and the only one that has ever needed
    /// cutting, which is why the census names it rather than every box.
    #[test]
    fn no_equipment_row_overflows_its_box() {
        let game = census_game();
        let (slot, _, _) = worst_equipment_row(&game);
        let SectionRow::Stat(label, value) = equip_row(&slot) else {
            panic!("an equipment row is a stat row");
        };

        with_painter(|p| {
            for (w, h) in CENSUS_WINDOWS {
                let m = ui_metrics(h);
                for (who, rect) in equipment_rects(&game, w, h) {
                    let row = fitted_stat_row(p, &label, &value, rect, &m);
                    let label_end = rect.x + m.inset + p.measure_ui_advance(&row.label, row.size);
                    let value_start = rect.x + rect.w - m.inset - row.value_w;
                    assert!(
                        label_end <= value_start,
                        "the widest EQUIPMENT row's halves collide by {:.0}px on the {who} \
                         page at {w}x{h}:\n{}\n{}",
                        label_end - value_start,
                        row.label,
                        row.value
                    );
                    assert!(
                        value_start >= rect.x + m.inset,
                        "the widest EQUIPMENT row's value escapes its box on the {who} page \
                         at {w}x{h}:\n{}",
                        row.value
                    );
                }
            }
        });
    }

    /// **The bug this branch was opened for.** A gear copy's name carries its
    /// affix at *both* ends — `Game::copy_name` puts a prefix word in front
    /// of the item name and a suffix phrase behind it — and nothing on this
    /// sheet clipped, so an overlong row ran off the box and what a player
    /// lost was always the tail.
    ///
    /// Held on **the player's page**, where the EQUIPMENT box is a band with
    /// the whole frame to draw a name in. A program's is a half-width column
    /// box and cannot be a band: that page clears its footer by 17.3px
    /// against a 10px floor, so a second band on it overflows at 1280x720 —
    /// `the_real_worst_case_pages_fit_the_tightest_window` is what measures
    /// that. There the widest rows are still cut, and `elided_to_fit` keeping
    /// both ends is what decides *which* characters go; the containment
    /// itself is `no_equipment_row_overflows_its_box`.
    #[test]
    fn a_dropped_equipment_row_keeps_the_affix_in_its_name() {
        let game = census_game();
        let rows = dropped_equipment_rows(&game);

        with_painter(|p| {
            for (w, h) in CENSUS_WINDOWS {
                let m = ui_metrics(h);
                for (who, rect) in equipment_rects(&game, w, h) {
                    if who != "player" {
                        continue;
                    }
                    for (slot, word) in &rows {
                        let SectionRow::Stat(label, value) = equip_row(slot) else {
                            panic!("an equipment row is a stat row");
                        };
                        assert!(
                            label.contains(word),
                            "the row the engine hands over already has to name it: {label}"
                        );
                        let fitted = fitted_stat_row(p, &label, &value, rect, &m);
                        assert!(
                            fitted.label.contains(word),
                            "the affix went missing at {w}x{h}:\n{}\nroom={:.1} label={:.1} \
                             value={:.1} ({value}) size={}",
                            fitted.label,
                            rect.w - m.inset * 2.0,
                            p.measure_ui_advance(&label, fitted.size),
                            p.measure_ui_advance(&value, fitted.size),
                            fitted.size,
                        );
                    }
                }
            }
        });
    }

    /// `manifest_layout::tests::worst_case_program` lists ROUTINES before
    /// MOVES, but `sections_for` does not: `program_sections` pushes MOVES
    /// last (it's the full-width band), and EQUIPMENT and ROUTINES are
    /// appended only after `program_sections` returns (see `sections_for`'s
    /// own doc). That drift is currently harmless — MOVES is the only
    /// `full_width` box, so `best_column_split` filters it out before packing
    /// the columned rest, and order stops mattering the moment a box leaves
    /// that set — but this project has previously shipped a layout fixture
    /// that drifted from what the renderer actually emits and hid a real
    /// overflow behind a green suite. Pinning the real sequence here is what
    /// would catch that again if a future change ever made full-width order
    /// matter.
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

        let mut program = plain_program(14, 12);
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
        let shape: Vec<(&str, usize, bool)> = sections
            .iter()
            .map(|s| (s.title, s.rows.len(), s.full_width))
            .collect();

        assert_eq!(
            shape,
            vec![
                ("COMBAT", 4, false),
                ("SPECIES", 5, false),
                ("WORK", 4, false),
                ("DEVELOPMENT", 3, false),
                ("MOVES", 1, true),
                ("EQUIPMENT", 2, false),
                ("ROUTINES", 1, false),
            ],
            "sections_for's real emission order — the shape the layout \
             fixture must mirror: {shape:?}"
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
            !sections.iter().any(|s| s.title == "EQUIPMENT"),
            "an empty loadout emits no box: {:?}",
            sections.iter().map(|s| s.title).collect::<Vec<_>>()
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

    /// A player with nothing optional set — no perks, no party — so a test
    /// naming a box is naming one this fixture actually produces.
    fn plain_player() -> PlayerManifest {
        PlayerManifest {
            power: 60.0,
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
            status_effect: None,
            routines: Vec::new(),
            equipment: Vec::new(),
            subject: ManifestSubject::Player(player),
        }
    }

    fn stat_value<'a>(section: &'a Section, label: &str) -> Option<&'a str> {
        section.rows.iter().find_map(|r| match r {
            SectionRow::Stat(l, v) if l == label => Some(v.as_str()),
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
        let combat = titled(&player, "COMBAT");
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
        let combat = titled(&program, "COMBAT");
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
        assert_eq!(run.rows.len(), 5, "one row under the cap, and no note");
    }

    /// The player's counterpart to
    /// `sections_for_emits_moves_before_a_programs_equipment_and_routines`.
    /// `manifest_layout::tests::worst_case_player` is a hand-written mirror
    /// of this shape, and a fixture that has drifted from what the renderer
    /// emits passes the clearance sweep while the real page overflows — the
    /// exact regression that file exists to catch.
    #[test]
    fn the_fullest_player_page_emits_the_boxes_its_layout_fixture_models() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(
            23,
            feral_processes_engine::DifficultyMode::Forgiving,
            assets,
        )
        .expect("shipped assets load");

        let mut player = plain_player();
        player.perks = vec![("Obfuscation".to_string(), 2)];
        player.party = vec![feral_processes_engine::CompanionInfo {
            entity: Entity::PLACEHOLDER,
            name: "Scrapper".to_string(),
            hp: 10,
            max_hp: 10,
            atk: 4,
            mitigation: 2,
            power: 12,
            status: None,
            ability: "priority_boost".to_string(),
            gear: String::new(),
        }];
        let mut view = player_view(player);
        view.equipment = vec![worn("WEP")];
        view.routines = vec![feral_processes_engine::RoutineSlotView {
            index: 0,
            ability: None,
            name: "(empty)".to_string(),
            description: String::new(),
        }];

        // `full_width` too, and not just the titles: EQUIPMENT is a band on
        // this page and a columned box on a program's (see `sections_for`),
        // so the flag is part of what `worst_case_player` has to mirror — a
        // fixture that models a band as a column packs a page the renderer
        // never draws.
        let shape: Vec<(&str, bool)> = sections_for(&game, &view)
            .iter()
            .map(|s| (s.title, s.full_width))
            .collect();
        assert_eq!(
            shape,
            vec![
                ("COMBAT", false),
                ("PROGRESSION", false),
                ("PERKS", false),
                ("PARTY", false),
                ("RUN", false),
                ("EQUIPMENT", true),
                ("ROUTINES", false),
            ],
        );
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
                    },
                    None,
                    p,
                    &m,
                );
            });
            crate::paint::painted_text(&shapes).join("")
        };

        assert!(
            footer(true).contains("[w] watch"),
            "the key has to be on the sheet or nobody finds it"
        );
        assert!(
            !footer(false).contains("[w] watch"),
            "and must not be offered where it would be refused"
        );
    }
}
