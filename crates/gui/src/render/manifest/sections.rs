//! The STATS face's section builders: what each box on the sheet says, as
//! rows of text, before any of it is laid out or drawn.

use super::layout::*;
use super::*;
use feral_processes_engine::{RankedStat, RosterRow};

pub(super) fn stat(label: impl Into<String>, value: impl Into<String>) -> SectionRow {
    SectionRow::Stat(label.into(), value.into())
}

fn ranked(label: &str, value: String, stat: RankedStat) -> SectionRow {
    SectionRow::Ranked(label.to_string(), value, stat)
}

impl Section {
    /// A ledger box with no hint; `with_hint` adds one.
    fn ledger(title: impl Into<String>, column: LedgerColumn, rows: Vec<SectionRow>) -> Section {
        Section {
            title: title.into(),
            column,
            rows,
            hint: String::new(),
        }
    }

    fn with_hint(mut self, hint: impl Into<String>) -> Section {
        self.hint = hint.into();
        self
    }
}

/// Every box the STATS ledger stacks, in the order they stack within their
/// column: FIGHT, then the subject-specific boxes from `player_sections` or
/// `program_sections`, then GEAR and ROUTINES **last** — appended here, not
/// by either of those, so this function is the one place that knows the
/// page's full section set (a wearer is a wearer, whoever carries the gear).
///
/// The rows that are ranked against the roster carry their `RankedStat`; the
/// draw step looks the place up in one `SubjectRanks`. Every section a
/// builder pushes needs a matching assertion in the column census
/// (`the_player_ledger_columns` / `the_program_ledger_columns`) — the
/// branch's original regression was a missing box, not a wrong row count.
pub(super) fn sections_for(game: &Game, view: &ManifestView) -> Vec<Section> {
    let player = matches!(&view.subject, ManifestSubject::Player(_));
    let mut fight = vec![
        ranked("Damage", view.damage.clone(), RankedStat::Damage),
        ranked("Attack", view.atk.to_string(), RankedStat::Attack),
    ];
    // The to-hit half of a fight, on the player's page alone. A program's
    // odds matter as much; `ManifestView` carries both figures for both
    // subjects so restoring these two rows there is a layout change and not
    // a data change.
    //
    // One decimal because both are halves: `ACCURACY_PER_LEVEL` is 0.5, so
    // a level-1 player reads 11.5 and an integer would quote a number the
    // attack roll does not use.
    if player {
        fight.push(stat("Accuracy", format!("{:.1}", view.accuracy)));
        fight.push(stat("Evasion", format!("{:.1}", view.evasion)));
    }
    fight.push(ranked(
        "Mitigation",
        format!("{}%", view.mitigation),
        RankedStat::Mitigation,
    ));
    fight.push(ranked("Power", view.power.to_string(), RankedStat::Power));
    let mut sections = vec![Section::ledger(
        "FIGHT",
        LedgerColumn::Fight,
        section_rows(fight),
    )];
    match &view.subject {
        ManifestSubject::Player(p) => player_sections(&mut sections, view, p),
        ManifestSubject::Program(p) => program_sections(&mut sections, game, p),
    }
    if !view.equipment.is_empty() {
        sections.push(Section::ledger(
            "GEAR",
            LedgerColumn::Fight,
            section_rows(view.equipment.iter().map(equip_row).collect()),
        ));
    }
    if !view.routines.is_empty() {
        sections.push(routines_section(view));
    }
    sections
}

/// `n of 6` in the subhead, the filled slots as rows, and the empty ones as
/// one `N slots open` line in `hud-attention` — an empty slot is something
/// to spend, not a row to read.
fn routines_section(view: &ManifestView) -> Section {
    // `r.unseen` is a filled slot whose routine the player has not met:
    // `RoutineSlotView::unseen`'s doc warns a renderer off reading the
    // name instead, since an empty slot's name is "(empty)", never blank.
    let filled = |r: &&feral_processes_engine::RoutineSlotView| r.ability.is_some() || r.unseen;
    let rows: Vec<SectionRow> = view
        .routines
        .iter()
        .filter(filled)
        .map(|r| {
            let name = if r.unseen {
                "??? — a routine you haven't seen".to_string()
            } else {
                r.name.clone()
            };
            stat(format!("{}", r.index + 1), name)
        })
        .collect();
    let open = view.routines.len() - rows.len();
    let n = rows.len();
    // The cap is on the filled slots alone: a note counting hidden routines
    // must not swallow the `slots open` line, which is the one that asks
    // something of the player.
    let mut rows = section_rows(rows);
    if open > 0 {
        rows.push(SectionRow::Callout(format!("{open} slots open")));
    }
    Section::ledger("ROUTINES", LedgerColumn::Fight, rows)
        .with_hint(format!("{n} of {}", view.routines.len()))
}

/// GROWTH, CLASS and PERKS stack in the middle column, RUN in the last.
/// XP is a row here as well as on the vitals line: the line says how full
/// the bar is and this says how far off the next level is.
fn player_sections(sections: &mut Vec<Section>, view: &ManifestView, p: &PlayerManifest) {
    let mut growth = Vec::new();
    if let (Some((xp, to_next)), Some(level)) = (view.xp, view.level) {
        growth.push(stat(
            "Next level",
            format!("{} xp to Lv {}", to_next.saturating_sub(xp), level + 1),
        ));
    }
    growth.push(if p.perk_points > 0 {
        SectionRow::Alert(
            "Perk points".to_string(),
            format!("{} unspent", p.perk_points),
        )
    } else {
        stat("Perk points", "0")
    });
    growth.push(stat("Decompiler", p.decompiler.to_string()));
    sections.push(Section::ledger(
        "GROWTH",
        LedgerColumn::Growth,
        section_rows(growth),
    ));

    if let Some(class) = &p.class {
        sections.push(Section::ledger(
            format!("CLASS · {}", class.name),
            LedgerColumn::Growth,
            section_rows(
                class
                    .bonuses
                    .iter()
                    .map(|(axis, mult)| stat(axis, format!("x{mult:.2}")))
                    .collect(),
            ),
        ));
    }

    if !p.perks.is_empty() {
        sections.push(Section::ledger(
            "PERKS",
            LedgerColumn::Growth,
            section_rows(
                p.perks
                    .iter()
                    .map(|(name, level)| stat(name.clone(), format!("Lv {level}")))
                    .collect(),
            ),
        ));
    }

    // What this run holds, as opposed to what the player *is*. Credits and
    // Portal Fragments are `ItemDef::banked` pools and so sit outside the
    // inventory the cargo row counts.
    //
    // Trace is deliberately absent: `render/stack.rs` already draws it, and
    // it is underground-only, so a second reading here would be a duplicate
    // that is blank most of the time.
    sections.push(Section::ledger(
        "RUN",
        LedgerColumn::Duty,
        section_rows_capped(
            vec![
                stat("Credits", p.credits.to_string()),
                stat("Portal Fragments", p.portal_fragments.to_string()),
                stat("Contracts", p.active_contracts.to_string()),
                stat("Cargo carried", p.cargo_used.to_string()),
                stat("Difficulty", difficulty_label(p.difficulty)),
                stat("Cycle", p.cycle.to_string()),
                stat("Position", format!("{}, {}", p.position.0, p.position.1)),
            ],
            MAX_RUN_ROWS,
        ),
    ));
}

/// A band under the ledger, full width.
pub(super) enum Band {
    /// The player's PARTY table: yourself first, then each companion.
    Party(Vec<PartyRow>),
    /// A program's LATELY lines: the mood, then the strongest memories.
    Lately(Vec<SectionRow>),
    /// SCORE's closing lines: the multiplier, then the run score.
    Total(Vec<SectionRow>),
}

pub(super) struct PartyRow {
    pub(super) glyph: char,
    pub(super) color: GlyphColor,
    pub(super) name: String,
    pub(super) hp: i32,
    pub(super) max_hp: i32,
    pub(super) atk: i32,
    pub(super) mitigation: i32,
    pub(super) level: u32,
    pub(super) doing: String,
    /// The player's own row, drawn in `hud-emphasis`.
    pub(super) own: bool,
}

impl Band {
    pub(super) fn title(&self) -> &'static str {
        match self {
            Band::Party(_) => "PARTY",
            Band::Lately(_) => "LATELY",
            Band::Total(_) => "TOTAL",
        }
    }

    pub(super) fn hint(&self) -> &'static str {
        match self {
            Band::Party(_) | Band::Total(_) => "",
            Band::Lately(_) => "R all memories",
        }
    }

    /// Rows under the subhead, the PARTY table's column heads included.
    pub(super) fn rows(&self) -> usize {
        match self {
            Band::Party(rows) => 1 + rows.len(),
            Band::Lately(rows) | Band::Total(rows) => rows.len(),
        }
    }
}

/// The band this subject's STATS page closes with, `None` for a wild program
/// (no memories) and for a player with no one in the party to tabulate.
///
/// A companion's glyph and level come off the roster, which is already the
/// engine's one derivation of both for the rail; `CompanionInfo` carries
/// neither.
pub(super) fn band_for(view: &ManifestView, roster: &[RosterRow]) -> Option<Band> {
    match &view.subject {
        ManifestSubject::Player(p) => {
            let mut rows = vec![PartyRow {
                glyph: view.glyph,
                color: view.color,
                name: view.name.clone(),
                hp: view.hp,
                max_hp: view.max_hp,
                atk: view.atk,
                mitigation: view.mitigation,
                level: view.level.unwrap_or(1),
                doing: String::new(),
                own: true,
            }];
            rows.extend(p.party.iter().filter_map(|c| {
                let r = roster.iter().find(|r| r.entity == c.entity)?;
                Some(PartyRow {
                    glyph: r.glyph,
                    color: r.color,
                    name: c.name.clone(),
                    hp: c.hp,
                    max_hp: c.max_hp,
                    atk: c.atk,
                    mitigation: c.mitigation,
                    level: r.level,
                    doing: c.ability.clone(),
                    own: false,
                })
            }));
            Some(Band::Party(rows))
        }
        ManifestSubject::Program(p) => p.mood.as_ref().map(|mood| Band::Lately(mood_rows(mood))),
    }
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

pub(super) fn equip_row(slot: &ManifestEquipSlot) -> SectionRow {
    let mut bonus: Vec<String> = Vec::new();
    // `{:+}` and not a literal `+`: an affix may carry a **negative**
    // component beside its bonus (see `affixes::AffixDef::stats`), and a
    // hardcoded sign renders that as `+-30 DEF`.
    if slot.atk != 0 {
        bonus.push(format!("{:+} ATK", slot.atk));
    }
    if slot.mitigation != 0 {
        bonus.push(format!("{:+} MIT", slot.mitigation));
    }
    if slot.deflection != 0 {
        bonus.push(format!("{:+} DEFL", slot.deflection));
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

/// POTENTIAL and DEVELOPMENT stack in the middle column; MOVES joins FIGHT
/// above GEAR; WORK, AFFINITIES and SPECIES stack in the last.
///
/// Every `sections.push` here needs a matching assertion in
/// `the_program_ledger_columns`, at its real cap and not a smaller
/// placeholder.
pub(super) fn program_sections(sections: &mut Vec<Section>, game: &Game, p: &ProgramManifest) {
    if !p.moves.is_empty() {
        let mut rows: Vec<SectionRow> = p.moves.iter().take(MAX_MOVE_ROWS).map(move_row).collect();
        let more = p.moves.len().saturating_sub(MAX_MOVE_ROWS);
        if more > 0 {
            rows.push(SectionRow::Note(format!("+{more} more on the dossier")));
        }
        sections.push(Section::ledger("MOVES", LedgerColumn::Fight, rows));
    }

    if let Some(q) = &p.potential {
        sections.push(Section::ledger(
            "POTENTIAL",
            LedgerColumn::Growth,
            section_rows_capped(
                vec![
                    ranked(
                        "Overall",
                        format!("{} ({}%)", q.label, q.percent),
                        RankedStat::Potential,
                    ),
                    stat("HP roll", roll_readout(q.hp_roll)),
                    stat("Attack roll", roll_readout(q.atk_roll)),
                    stat("Defense roll", roll_readout(q.def_roll)),
                    stat("Growth roll", roll_readout(q.growth_roll)),
                    stat("Assembly roll", roll_readout(q.assembly_roll)),
                    stat("Extraction roll", roll_readout(q.extraction_roll)),
                ],
                MAX_POTENTIAL_ROWS,
            ),
        ));
    }

    // Only for a program that has been developed or that has points to
    // hold and spend, the way the `fused` and `upgraded` header tags only
    // show once they mean something.
    if p.ring > 0 || p.talents_earned > 0 || p.points.is_some() {
        let mut rows = vec![
            stat(
                "Kernel rings",
                format!("{}/{} (to Lv {})", p.ring, p.max_ring, p.level_cap),
            ),
            stat(
                "Talents",
                format!("{}/{} spent", p.talents_spent, p.talents_earned),
            ),
        ];
        if let Some(points) = &p.points {
            rows.push(stat(
                "Points",
                if points.holding {
                    format!("{} (holding)", points.banked)
                } else {
                    points.banked.to_string()
                },
            ));
        }
        let section = Section::ledger("DEVELOPMENT", LedgerColumn::Growth, section_rows(rows));
        sections.push(if p.points.is_some() {
            section.with_hint("H hold · S spend")
        } else {
            section
        });
    }

    // What this program is like to *post* somewhere, as opposed to what it
    // is. Speed lives here rather than in SPECIES: it means initiative in a
    // fight and pace at a machine, and one number said twice on one page
    // reads as two different numbers.
    let mut work = Vec::new();
    if let Some(class) = p.base_job {
        work.push(stat("Base job", base_job_label(class)));
    }
    // The only ranked row that is not a number on the sheet: the rank is by
    // the two build rolls, and the value is the resource they are good at.
    if let Some(res) = &p.work_resource {
        work.push(ranked(
            "Work aptitude",
            game.item_name(res).to_string(),
            RankedStat::WorkAptitude,
        ));
    }
    work.push(stat("Speed", p.base_speed.to_string()));
    work.push(stat("Analysis", p.analysis.to_string()));
    // The header already carries the post as one of a run of tags, where a
    // worker's is the bare structure name and reads as decoration. Stated as
    // a labelled row it reads as the assignment it is.
    if let Some((kind, structure)) = &p.post {
        work.push(stat(post_label(*kind), structure.clone()));
    }
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
    sections.push(Section::ledger(
        "WORK",
        LedgerColumn::Duty,
        section_rows_capped(work, MAX_WORK_ROWS),
    ));

    if !p.affinities.is_empty() {
        sections.push(Section::ledger(
            "AFFINITIES",
            LedgerColumn::Duty,
            section_rows_capped(
                p.affinities
                    .iter()
                    .map(|&(kind, v)| stat(kind.label(), format!("{v:.2}x")))
                    .collect(),
                MAX_AFFINITY_ROWS,
            ),
        ));
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
    sections.push(Section::ledger(
        "SPECIES",
        LedgerColumn::Duty,
        section_rows(species),
    ));
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
        TaskKind::Tend(_) => "Tending",
    }
}

/// The MEMORIES box's rows: the mood line, then the strongest
/// `MANIFEST_MEMORY_ROWS` entries behind it.
///
/// Split out of `program_sections` for `memory_page_rows`' reason — the width
/// census has to measure this box at its **worst** case, and a mood carrying
/// the widest shipped def against the widest shipped subject is a state a
/// fixture can state and a `Game` would have to be played into.
///
/// **The band and the number together**, unlike the needs rows above, which
/// are banded alone. A need's bar is one reserve on a known scale; morale is a
/// signed sum with no bar anywhere, so the word alone could not be compared
/// between two programs and the number alone reads as a quantity of nothing.
/// `views::morale_band` owns the word — a renderer that picked its own would
/// be the second place in the codebase deciding what a morale figure means.
///
/// A memory's own row leads with **what it is about** where it has a subject,
/// because that is the half a player scans for: three programs mauled by the
/// same species is the pattern, and the def's name is the same handful of
/// phrases down the whole column. The blurb is deliberately absent — it is a
/// property of the kind rather than of the program, and the `R` page is where
/// it is said.
pub(super) fn mood_rows(mood: &ManifestMood) -> Vec<SectionRow> {
    let mut rows = vec![stat(
        "Mood",
        format!("{} ({})", mood.band, super::party::strength(mood.sum)),
    )];
    if mood.memories.is_empty() {
        // The same state the `R` page words for itself, kept short here
        // because this is a value column and not a page.
        rows.push(stat("Carrying", "nothing yet"));
    }
    for entry in mood.memories.iter().take(MANIFEST_MEMORY_ROWS) {
        let label = match &entry.subject {
            Some(subject) => format!("{} — {subject}", entry.name),
            None => entry.name.clone(),
        };
        rows.push(stat(label, super::party::strength(entry.intensity)));
    }
    rows
}

pub(super) fn move_row(mv: &MoveDef) -> SectionRow {
    let mut tags = vec![format!("pow {}", mv.power)];
    if mv.ranged {
        tags.push("ranged".to_string());
    }
    if let Some(effect) = &mv.effect {
        tags.push(format!(
            "{} {:.0}% for {}",
            effect.kind.0,
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
