//! The STATS face's section builders: what each box on the sheet says, as
//! rows of text, before any of it is laid out or drawn.

use super::layout::*;
use super::*;

pub(super) fn stat(label: impl Into<String>, value: impl Into<String>) -> SectionRow {
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
pub(super) fn sections_for(game: &Game, view: &ManifestView) -> Vec<Section> {
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
        overflow: 0,
        hint: "",
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
            overflow: 0,
            hint: "",
        });
    }
    if !view.routines.is_empty() {
        sections.push(Section {
            title: "ROUTINES",
            rows: section_rows(
                view.routines
                    .iter()
                    .map(|r| {
                        // `r.unseen` rather than `r.name.is_empty()`: an
                        // empty slot's name is "(empty)", never blank, so
                        // the flag is the only door — reading the string
                        // instead is exactly the leak `RoutineSlotView::
                        // unseen`'s doc warns a renderer away from.
                        let name = if r.unseen {
                            "??? — a routine you haven't seen".to_string()
                        } else {
                            r.name.clone()
                        };
                        stat(format!("{}", r.index + 1), name)
                    })
                    .collect(),
            ),
            full_width: false,
            overflow: 0,
            hint: "",
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
        rows: section_rows(
            p.class
                .iter()
                // The class *name* rides the page's title, beside the
                // player's own — this row is what that class is worth,
                // which the title has no room for and which is the thing
                // a player opens a stat sheet to check.
                .map(|c| stat("Class bonus", c.bonuses.clone()))
                .chain([
                    stat("Decompiler", p.decompiler.to_string()),
                    stat("Perk points", p.perk_points.to_string()),
                    stat("Cargo carried", p.cargo_used.to_string()),
                    stat("Position", format!("{}, {}", p.position.0, p.position.1)),
                ])
                .collect(),
        ),
        full_width: false,
        overflow: 0,
        hint: "",
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
            overflow: 0,
            hint: "",
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
            overflow: 0,
            hint: "",
        });
    }

    sections.push(Section {
        title: "RUN",
        rows: section_rows(run),
        full_width: false,
        overflow: 0,
        hint: "",
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

/// Every `sections.push` here needs a matching entry in
/// `manifest_layout::tests::worst_case_program`, at its real cap not a
/// smaller placeholder — see `sections_for`'s doc for why. The affinities
/// regression this screen shipped with was not a wrong row count, it was a
/// missing box: AFFINITIES existed here before the fixture knew a fifth
/// columned box existed at all. Adding a section here without adding it
/// there passes every test and still ships a page that doesn't fit.
pub(super) fn program_sections(sections: &mut Vec<Section>, game: &Game, p: &ProgramManifest) {
    if let Some(q) = &p.potential {
        sections.push(Section {
            title: "POTENTIAL",
            rows: section_rows_capped(
                vec![
                    stat("HP roll", roll_readout(q.hp_roll)),
                    stat("Attack roll", roll_readout(q.atk_roll)),
                    stat("Defense roll", roll_readout(q.def_roll)),
                    stat("Growth roll", roll_readout(q.growth_roll)),
                    stat("Assembly roll", roll_readout(q.assembly_roll)),
                    stat("Extraction roll", roll_readout(q.extraction_roll)),
                    stat("Overall", format!("{} ({}%)", q.label, q.percent)),
                ],
                MAX_POTENTIAL_ROWS,
            ),
            full_width: false,
            overflow: 0,
            hint: "",
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
            overflow: 0,
            hint: "",
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
        overflow: 0,
        hint: "",
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
        stat("Analysis", p.analysis.to_string()),
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
        overflow: 0,
        hint: "",
    });

    // What this program is carrying, under what it adds up to.
    //
    // **Present exactly when the program has a `Memories` store**, which is
    // every program on the roster and nothing else — so a wild one drops the
    // box without an ownership check here, and an owned one nothing has
    // happened to yet keeps it and says so. An install with
    // `assets/memories/` deleted lands in that second case at every program
    // at once, which is the supported way to play without this feature.
    if let Some(mood) = &p.mood {
        sections.push(Section {
            title: "MEMORIES",
            rows: section_rows(mood_rows(mood)),
            full_width: false,
            overflow: 0,
            hint: "",
        });
    }

    // Only for a program that has been developed or that has points to
    // hold and spend, the way the `fused` and `upgraded` header tags only
    // show once they mean something: an undeveloped wild program's box would
    // be rows of zero on a page whose column budget is already the tightest
    // thing in the renderer. The bank lives here rather than in a box of its
    // own because the page has no row to spare at 1280x720 (measured: one
    // more row of any box overflows the fullest program page), so the ring
    // and the ceiling it buys share a row.
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
        sections.push(Section {
            title: "DEVELOPMENT",
            rows: section_rows(rows),
            full_width: false,
            overflow: 0,
            hint: if p.points.is_some() {
                "[H] hold  [S] spend"
            } else {
                ""
            },
        });
    }

    if !p.moves.is_empty() {
        sections.push(Section {
            title: "MOVES",
            // The overflow rides the title rather than a "+N more" row:
            // at `MAX_MOVE_ROWS` = 1 the note would *be* the box, and a
            // species with two moves drew neither.
            rows: p.moves.iter().take(MAX_MOVE_ROWS).map(move_row).collect(),
            // **Columned, and it was the page's full-width band until
            // MEMORIES arrived.** A band and two columned boxes side by side
            // occupy the same grid row, so demoting this one paid for the new
            // box outright — measured, the program page holds the same
            // clearance at 1280x720 as it did before either change. The
            // widest shipped move row is narrower than the widest gear row by
            // a distance, which is why EQUIPMENT could not have made this
            // trade instead; `no_move_row_is_cut_to_fit_its_column` is what holds
            // that, rather than the arithmetic.
            full_width: false,
            overflow: p.moves.len().saturating_sub(MAX_MOVE_ROWS),
            hint: "",
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
