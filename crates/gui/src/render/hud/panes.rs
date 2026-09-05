//! What the info column's open pane draws: the BASE, CREW, PACK and
//! CONTRACTS bodies.
//!
//! The column **does not scroll**, the same as the gear inspect and memories
//! pages, so the body rect's height is a layout constraint and not a starting
//! point. That is the whole reason this module builds rows before it draws
//! any: [`fitting_rows`] is `strip::fitting`'s rule turned ninety degrees —
//! what does not fit is **counted**, never drawn past the bottom edge in
//! silence — and it is written once here rather than three times, once per
//! pane, where it would be three sites agreeing rather than one fact.
//!
//! The three builders are pure functions of view data. They take no
//! `Painter`, which is what lets the census assert on the rows a pane *would*
//! draw at a given window size without standing a renderer up — the property
//! `hud::layout` already relies on one scale up.
//!
//! See `docs/superpowers/archive/specs/2026-08-27-paned-command-hud-design.md`.

use feral_processes_app_core::{InfoTab, item_fusion_note};
use feral_processes_engine::{
    ActiveBuffView, BuildOrderRow, ContractRow, LabourDemand, PetInfo, StockRow, StructureReport,
};

use super::palette;
use super::strip::Piece;
use crate::paint::{Painter, Rect, TextRun};
use crate::render::field::{TagStyle, buff_entries};
use crate::render::fusion_color;
use crate::render::popup::Row as PopupRow;
use crate::text::Metrics;

/// Stock rows shown in the PRODUCTION block before the count takes over.
const PRODUCTION_ROWS: usize = 6;
/// Build requests listed before the count takes over.
const BUILD_ROWS: usize = 4;
/// Roster rows listed before the count takes over.
const CREW_ROWS: usize = 12;
/// Party pips on the CREW tab's collapsed bar.
const CREW_PIPS: usize = 5;
/// Cells a contract's name gets before its progress tail. Wide enough for
/// every name the shipped assets can build, which
/// `no_shipped_contract_is_truncated_in_the_column` is what holds.
const CONTRACT_NAME_CELLS: usize = 30;
/// Cells a contract's objective gets on its second row.
const CONTRACT_OBJECTIVE_CELLS: usize = 36;

/// One line of a pane body.
///
/// Exhaustive rather than a struct with an `is_rule` flag, `cell_mark`'s
/// rule: [`draw_rows`] matches on it, so a fourth kind fails to compile
/// instead of drawing as a blank line nobody notices.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::render) enum Row {
    /// Text, with an optional right-aligned tail. **The tail is where a
    /// keycap chip goes** — the handoff's rule is that a row the player can
    /// act on carries its key at the right end of that same row, never on a
    /// line of its own, so the key is never separated from the thing it acts
    /// on.
    Text { left: Vec<Piece>, right: Vec<Piece> },
    /// A hairline across the body, between two blocks.
    Rule,
}

pub(in crate::render) fn text(left: Vec<Piece>) -> Row {
    Row::Text {
        left,
        right: Vec::new(),
    }
}

pub(in crate::render) fn with_tail(left: Vec<Piece>, right: Vec<Piece>) -> Row {
    Row::Text { left, right }
}

/// A block's sub-head — `PRODUCTION`, `DEFENCE`. Its own row so the blocks
/// read as blocks and not as one run of rows.
pub(in crate::render) fn subhead(name: &str) -> Row {
    text(vec![(name.to_string(), palette::PANE_TITLE, true)])
}

/// A keycap chip: the key, then what it opens.
pub(in crate::render) fn chip(key: char, verb: &str) -> Vec<Piece> {
    vec![
        (format!("{key}"), palette::EMPHASIS, true),
        (format!(" {verb}"), palette::LABEL, false),
    ]
}

/// How tall one row draws.
fn row_height(row: &Row, m: &Metrics) -> f32 {
    match row {
        Row::Text { .. } => m.line_height,
        Row::Rule => m.gap,
    }
}

/// The longest prefix of `rows` that fits `avail`, and how many were cut.
///
/// **`stock::fits`' rule on the vertical axis.** A pane has no scrollbar to
/// defer a row to, so the overflow is reported rather than dropped — the
/// caller spends a row saying how many did not fit, which is what stops the
/// column lying about what the base is doing. A trailing [`Row::Rule`] is
/// dropped rather than counted: a divider with nothing under it is not
/// information the player lost.
///
/// Reserves room for the overflow row itself whenever it will be needed, or
/// the count would be the thing that overflows.
pub(in crate::render) fn fitting_rows(rows: &[Row], avail: f32, m: &Metrics) -> (Vec<Row>, usize) {
    let total: f32 = rows.iter().map(|r| row_height(r, m)).sum();
    if total <= avail {
        return (rows.to_vec(), 0);
    }
    // One row of the budget belongs to the "+N more" line, which only exists
    // in this branch — measured against the full list above, a pane that
    // fits exactly would be cut by the space reserved to say so.
    let budget = avail - m.line_height;
    let mut used = 0.0;
    let mut taken = 0;
    for row in rows {
        let h = row_height(row, m);
        if used + h > budget {
            break;
        }
        used += h;
        taken += 1;
    }
    let mut shown = rows[..taken].to_vec();
    while matches!(shown.last(), Some(Row::Rule)) {
        shown.pop();
    }
    (shown, rows.len() - taken)
}

/// Draws `rows` down `at`, and says so when any were cut.
///
/// The overflow line is drawn in [`palette::ATTENTION`] — it is the player
/// being told the pane is not showing them everything, which is a thing they
/// can act on by opening the full screen the chip names.
pub(in crate::render) fn draw_rows(
    at: Rect,
    rows: &[Row],
    cut: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let size = m.font_size;
    let mut cy = at.y + m.inset + size as f32 / 2.0;
    for row in rows {
        match row {
            Row::Text { left, right } => {
                draw_line(at, cy, left, right, painter, m);
                cy += m.line_height;
            }
            Row::Rule => {
                let y = cy - m.line_height / 2.0 + m.gap / 2.0;
                painter.line(
                    at.x + m.inset,
                    y,
                    at.x + at.w - m.inset,
                    y,
                    1.0,
                    palette::DIVIDER,
                );
                cy += m.gap;
            }
        }
    }
    if cut > 0 {
        let more = vec![(format!("+{cut} more"), palette::ATTENTION, false)];
        draw_line(at, cy, &more, &[], painter, m);
    }
}

fn draw_line(at: Rect, cy: f32, left: &[Piece], right: &[Piece], painter: &Painter, m: &Metrics) {
    let size = m.font_size;
    if !left.is_empty() {
        let runs: Vec<TextRun> = left
            .iter()
            .map(|(t, c, b)| TextRun {
                text: t,
                bold: *b,
                color: *c,
            })
            .collect();
        painter.ui_runs(&runs, at.x + m.inset, cy, size);
    }
    if !right.is_empty() {
        let tail: String = right.iter().map(|(t, _, _)| t.as_str()).collect();
        let w = painter.measure_ui_advance(&tail, size);
        let runs: Vec<TextRun> = right
            .iter()
            .map(|(t, c, b)| TextRun {
                text: t,
                bold: *b,
                color: *c,
            })
            .collect();
        painter.ui_runs(&runs, at.x + at.w - m.inset - w, cy, size);
    }
}

/// Everything the three pane bodies read, gathered once by
/// `draw_playing_base` before the `Game` borrow it needs is given up.
///
/// One struct rather than three argument lists because the caller gathers it
/// in one place either way, and because a field added for one pane is then
/// visibly available to the other two rather than being fetched twice.
#[derive(Default)]
pub(in crate::render) struct PaneData<'a> {
    /// Programs owned against `Game::pet_capacity`. Two scalars rather than
    /// the whole `PlayerStatus`, because these and `carrying` are every field
    /// of it the three panes read — a narrower dependency, and a fixture that
    /// does not have to build twenty-three fields to test a headcount.
    pub roster: (usize, usize),
    /// `PlayerStatus::inventory_used`. Units carried, not rows: the pack has
    /// no capacity, so there is no `n/max` to draw.
    pub carrying: u32,
    pub buffs: &'a [ActiveBuffView],
    /// The pack, with every copy's name already resolved — `Game::copy_name`
    /// is the one place a name is built and it needs the `Game` this struct
    /// outlives.
    pub pack: &'a [PackRow],
    pub pets: &'a [PetInfo],
    pub structures: &'a [StructureReport],
    pub stock: &'a [StockRow],
    pub builds: &'a [BuildOrderRow],
    pub labour: LabourDemand,
    /// What the run is signed to, from `Game::active_contracts` — which is
    /// `&self` and capped at `MAX_ACTIVE_CONTRACTS`, so it costs a frame
    /// nothing. `Game::contract_board` is the one that must never be reached
    /// from here: it is `&mut`, it rolls templates and it samples the
    /// habitat ring.
    pub contracts: &'a [ContractRow],
    pub shielded: bool,
    /// Base space only: out on the surface there is no base to report on, so
    /// the BASE pane says where the base is instead of drawing empty blocks.
    pub in_base: bool,
}

/// One pack row, with its name already built.
pub(in crate::render) struct PackRow {
    pub qty: u32,
    pub name: String,
    pub tier: u32,
}

/// The open pane's body, as rows.
///
/// Exhaustive on `InfoTab`, `cell_mark`'s rule: a fourth tab fails to compile
/// here rather than opening on a blank pane.
pub(in crate::render) fn rows(tab: InfoTab, d: &PaneData) -> Vec<Row> {
    match tab {
        InfoTab::Base => base_rows(d),
        InfoTab::Crew => crew_rows(d),
        InfoTab::Pack => pack_rows(d),
        InfoTab::Contracts => contract_rows(d),
    }
}

fn head(name: &str, key: char, verb: &str) -> Row {
    with_tail(
        vec![(name.to_string(), palette::PANE_TITLE, true)],
        chip(key, verb),
    )
}

fn dim(t: impl Into<String>) -> Piece {
    (t.into(), palette::FAINT, false)
}

fn body(t: impl Into<String>) -> Piece {
    (t.into(), palette::BODY, false)
}

fn label(t: impl Into<String>) -> Piece {
    (t.into(), palette::LABEL, false)
}

/// Cuts to `w` cells without padding, for a row whose tail is right-aligned
/// and so needs no column under it — [`cell`]'s padding would only push the
/// measured width up for nothing.
fn clip(t: &str, w: usize) -> String {
    t.chars().take(w).collect()
}

/// Pads to `w` cells. The column is monospace-measured, so a fixed-width cell
/// is what keeps a table's columns under each other.
fn cell(t: &str, w: usize) -> String {
    let mut s: String = t.chars().take(w).collect();
    while s.chars().count() < w {
        s.push(' ');
    }
    s
}

// ---------------------------------------------------------------- BASE

/// Structures, production, defence, the build queue and the labour shortfall.
///
/// The blocks are the handoff's five, in its order. `PROGRAMS AVAILABLE` is
/// `Game::labour_demand` and the idle-structure count rather than the
/// handoff's guess at a model, per the spec.
fn base_rows(d: &PaneData) -> Vec<Row> {
    let mut out = vec![head("BASE", 'b', "base")];
    if !d.in_base {
        out.push(text(vec![dim(
            "no base space \u{2014} you are on the surface",
        )]));
        return out;
    }

    out.push(text(vec![(
        format!("{}{}{}", cell("  NAME", 20), cell("PROGRAM", 13), "OUT"),
        palette::LABEL,
        false,
    )]));
    for s in d.structures.iter().filter(|s| s.workable) {
        let idle = s.assignees.is_empty();
        let program = s
            .assignees
            .first()
            .map(|a| a.label.clone())
            .unwrap_or_else(|| "\u{2014} none \u{2014}".to_string());
        let out_qty: u32 = s.output.iter().map(|(_, q)| q).sum();
        let mark = if idle { "! " } else { "  " };
        let colour = if idle {
            palette::ATTENTION
        } else {
            palette::BODY
        };
        out.push(text(vec![(
            format!(
                "{mark}{}{}{}",
                cell(&s.label, 18),
                cell(&program, 13),
                out_qty
            ),
            colour,
            false,
        )]));
    }
    if !d.structures.iter().any(|s| s.workable) {
        out.push(text(vec![dim("  no workable structures")]));
    }

    out.push(Row::Rule);
    out.push(subhead("PRODUCTION"));
    if d.stock.is_empty() {
        out.push(text(vec![dim("  the shelves are empty")]));
    }
    for row in d.stock.iter().take(PRODUCTION_ROWS) {
        out.push(with_tail(
            vec![label(format!("  {} ", row.tag)), body(row.name.clone())],
            vec![body(row.qty.to_string())],
        ));
    }

    out.push(Row::Rule);
    out.push(subhead("DEFENCE"));
    out.push(text(vec![
        label("  SHIELDS  "),
        if d.shielded {
            ("holding".to_string(), palette::HEALTHY, false)
        } else {
            ("no defence".to_string(), palette::ATTENTION, false)
        },
    ]));
    // The weakest standing structure, which is the one a sweep takes first.
    match d
        .structures
        .iter()
        .filter_map(|s| s.durability.map(|dur| (dur, s.label.clone())))
        .min_by_key(|((cur, max), _)| (*cur * 100) / (*max).max(1))
    {
        Some(((cur, max), name)) => out.push(with_tail(
            vec![label("  WEAKEST  "), body(name)],
            vec![(
                format!("{cur}/{max}"),
                if cur < max {
                    palette::THREAT
                } else {
                    palette::BODY
                },
                false,
            )],
        )),
        None => out.push(text(vec![label("  WEAKEST  "), dim("nothing raidable")])),
    }

    out.push(Row::Rule);
    out.push(subhead("BUILD QUEUE"));
    if d.builds.is_empty() {
        out.push(with_tail(
            vec![dim("  queue empty")],
            chip('b', "mark a site"),
        ));
    }
    for b in d.builds.iter().take(BUILD_ROWS) {
        let note = if b.delivered < b.materials {
            format!("{}/{} mat", b.delivered, b.materials)
        } else {
            format!("{}/{} tk", b.ticks, b.required_ticks)
        };
        out.push(with_tail(
            vec![dim("  \u{00b7} "), body(b.structure.clone())],
            vec![label(note)],
        ));
    }

    out.push(Row::Rule);
    out.push(subhead("PROGRAMS AVAILABLE"));
    let idle = d
        .structures
        .iter()
        .filter(|s| s.workable && s.assignees.is_empty())
        .count();
    let short = d.labour.shortfall();
    out.push(text(vec![
        label("  posted   "),
        body(format!(
            "{}/{}",
            d.labour.wanted.min(d.labour.staff),
            d.labour.staff
        )),
    ]));
    if idle > 0 || short > 0 {
        out.push(with_tail(
            vec![(
                format!("  {idle} idle \u{00b7} {short} short"),
                palette::ATTENTION,
                false,
            )],
            chip('b', "assign"),
        ));
    } else {
        out.push(text(vec![dim("  every post is filled")]));
    }
    out
}

// ---------------------------------------------------------------- CREW

/// The roster, then the running routines.
///
/// Built from `Game::owned_pets` rather than `PlayerStatus::companions`
/// because that is the list carrying a level and an activity — the two
/// things this pane is for — and because it is one list rather than two that
/// have to be kept from disagreeing about who is in the party.
///
/// A buff's holder rides its own indented row rather than the row's tail. The
/// column is a fixed slice of the window and cannot widen, which is the same
/// reason `draw_status_buffs` picks `TagStyle::OwnLine`; the ceiling is held
/// by `no_crew_row_overflows_the_column`.
fn crew_rows(d: &PaneData) -> Vec<Row> {
    let mut out = vec![head("CREW", 'p', "party")];
    out.push(with_tail(
        vec![
            label("ROSTER  "),
            body(format!("{}/{}", d.roster.0, d.roster.1)),
        ],
        if d.roster.0 >= d.roster.1 {
            vec![("full".to_string(), palette::ATTENTION, false)]
        } else {
            Vec::new()
        },
    ));
    out.push(text(vec![(
        format!("{}{}{}", cell("  UNIT", 16), cell("LV", 4), "HP"),
        palette::LABEL,
        false,
    )]));
    if d.pets.is_empty() {
        out.push(text(vec![dim("  nobody on the roster")]));
    }
    // Party first: the members standing beside you are what the pane is most
    // often opened for, and `owned_pets` is not ordered.
    let mut roster: Vec<&PetInfo> = d.pets.iter().collect();
    roster.sort_by_key(|p| (p.party_slot.is_none(), p.party_slot, p.name.clone()));
    for p in roster.iter().take(CREW_ROWS) {
        let mark = if p.party_slot.is_some() {
            "\u{00bb} "
        } else {
            "  "
        };
        let hurt = p.hp * 2 < p.max_hp.max(1);
        out.push(with_tail(
            vec![
                (
                    format!(
                        "{mark}{}{}",
                        cell(&p.name, 14),
                        cell(&p.level.to_string(), 4)
                    ),
                    if p.party_slot.is_some() {
                        palette::BODY
                    } else {
                        palette::FAINT
                    },
                    false,
                ),
                (
                    format!("{}/{}", p.hp, p.max_hp),
                    if hurt { palette::THREAT } else { palette::BODY },
                    false,
                ),
            ],
            vec![label(p.activity.clone())],
        ));
    }

    out.push(Row::Rule);
    out.push(subhead("ROUTINES"));
    if d.buffs.is_empty() {
        out.push(text(vec![dim("  nothing running")]));
    }
    // `buff_entries` and not a second formatting of the same four fields —
    // what a buff row says has one statement, and this is the second panel
    // reading it. `OwnLine` because the column is a fixed slice of the
    // window and cannot widen to carry a holder tag inline.
    for entry in buff_entries(d.buffs, TagStyle::OwnLine) {
        for row in entry {
            out.push(match row {
                PopupRow::Item {
                    text: t, suffix, ..
                } => with_tail(
                    vec![body(format!("  {t}"))],
                    suffix.map(|s| vec![dim(s)]).unwrap_or_default(),
                ),
                PopupRow::TextColored(t, _) => text(vec![dim(format!("  {t}"))]),
                PopupRow::Text(t) => text(vec![dim(format!("  {t}"))]),
            });
        }
    }
    out
}

// ---------------------------------------------------------------- PACK

/// The pack, which is a list and nothing else.
///
/// There is **no capacity row**: `components::Inventory` is an unbounded
/// `Vec`, so a `n/max` figure here would be inventing a limit the simulation
/// does not have. `inventory_used` is a count of what is carried, and is
/// drawn as one.
fn pack_rows(d: &PaneData) -> Vec<Row> {
    let mut out = vec![head("PACK", 'i', "pack")];
    out.push(text(vec![
        label("CARRYING  "),
        body(format!("{} units", d.carrying)),
    ]));
    out.push(Row::Rule);
    if d.pack.is_empty() {
        out.push(text(vec![dim("  the pack is empty")]));
    }
    for row in d.pack {
        let note = match row.tier {
            0 => String::new(),
            t => format!(" {}", item_fusion_note(t)),
        };
        out.push(with_tail(
            vec![(
                format!("  {}{note}", row.name),
                fusion_color(row.tier).unwrap_or(palette::BODY),
                false,
            )],
            vec![body(row.qty.to_string())],
        ));
    }
    out
}

// ------------------------------------------------------------ CONTRACTS

/// Whether a contract has met what it asks for and is waiting to be handed
/// in. One expression, read by the pane's colour and by its collapsed bar,
/// so the two cannot disagree about what "ready" means.
fn is_ready(c: &ContractRow) -> bool {
    c.progress >= c.target
}

/// What the run is signed to, two rows apiece: the name with its progress,
/// and the objective under it.
///
/// This pane does **not** short-circuit off-base the way BASE does. A
/// contract reads from anywhere — off the base and underground alike, which
/// is `Game::active_contracts`' own rule — so there is no locale for the
/// body to be empty in.
///
/// The chip says `b` because the Contracts screen hangs off the base group
/// menu, at `Locality::Anywhere`.
fn contract_rows(d: &PaneData) -> Vec<Row> {
    let mut out = vec![head("CONTRACTS", 'b', "contracts")];
    if d.contracts.is_empty() {
        out.push(text(vec![dim("  nothing signed")]));
        return out;
    }
    for c in d.contracts {
        let ready = is_ready(c);
        out.push(with_tail(
            vec![dim("  \u{00b7} "), body(clip(&c.name, CONTRACT_NAME_CELLS))],
            vec![(
                format!("{}/{}", c.progress, c.target),
                if ready {
                    palette::ATTENTION
                } else {
                    palette::BODY
                },
                ready,
            )],
        ));
        out.push(text(vec![dim(format!(
            "    {}",
            clip(&c.objective_line, CONTRACT_OBJECTIVE_CELLS)
        ))]));
    }
    out
}

/// A closed tab's one-line summary, for its collapsed bar.
///
/// Read **only when the tab has no attention row** — a condition needing the
/// player outranks a headcount, and `hud::column` picks between them. So this
/// is the calm state's readout and never competes with the `!` half.
///
/// Exhaustive on `InfoTab` for [`rows`]' reason: a fourth tab collapses to a
/// blank bar otherwise, which is the one thing the collapsed bars exist to
/// prevent.
pub(in crate::render) fn summary(tab: InfoTab, d: &PaneData) -> String {
    match tab {
        InfoTab::Base => {
            if !d.in_base {
                return "on the surface".to_string();
            }
            let workable = d.structures.iter().filter(|s| s.workable).count();
            let held: u32 = d.stock.iter().map(|r| r.qty).sum();
            format!("{workable} nodes \u{00b7} {held} held")
        }
        InfoTab::Crew => {
            let party = d.pets.iter().filter(|p| p.party_slot.is_some()).count();
            let pips: String = "\u{2589}".repeat(party.min(CREW_PIPS));
            format!("{pips} {}/{}", d.roster.0, d.roster.1)
        }
        // Units, not rows: the pack has no capacity, so the figure that means
        // anything is how much is being carried.
        InfoTab::Pack => format!("{} units", d.carrying),
        InfoTab::Contracts => {
            if d.contracts.is_empty() {
                return "nothing signed".to_string();
            }
            let ready = d.contracts.iter().filter(|c| is_ready(c)).count();
            match ready {
                0 => format!("{} signed", d.contracts.len()),
                n => format!("{n} ready \u{00b7} {} signed", d.contracts.len()),
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use feral_processes_engine::components::{GlyphColor, MachineStatus, Rarity};
    use feral_processes_engine::{ActiveBuffView, PetInfo, ProgramRole, StockRow, StructureReport};

    fn entity() -> feral_processes_engine::Entity {
        feral_processes_engine::Entity::PLACEHOLDER
    }

    fn pet(name: &str, slot: Option<u32>) -> PetInfo {
        PetInfo {
            entity: entity(),
            glyph: 'p',
            color: GlyphColor::Green,
            name: name.to_string(),
            level: 12,
            hp: 120,
            max_hp: 120,
            atk: 40,
            mitigation: 20,
            power: 99,
            party_slot: slot,
            // The fixture's two roles, and the pair the CREW pane splits on.
            role: if slot.is_some() {
                ProgramRole::InParty
            } else {
                ProgramRole::Staff
            },
            activity: "Research Node".to_string(),
            quality: None,
            fusions: 0,
            refactors: 0,
            ring: 0,
            talents: 0,
            rarity: Rarity::Ordinary,
            wielded: false,
            gear: "w|a|m".to_string(),
        }
    }

    fn structure(label: &str, idle: bool) -> StructureReport {
        StructureReport {
            entity: entity(),
            kind: "mining_node".to_string(),
            label: label.to_string(),
            pos: (0, 0),
            distance: 3,
            tier: Some(2),
            durability: Some((18, 24)),
            is_home: false,
            workable: true,
            player_adjacent: false,
            input: Vec::new(),
            output: vec![("Cache Grain".to_string(), 7)],
            output_capacity: 20,
            status: Some(MachineStatus::Running),
            assignees: if idle {
                Vec::new()
            } else {
                vec![feral_processes_engine::Assignee {
                    entity: entity(),
                    label: "Longnamed Program".to_string(),
                    kind: feral_processes_engine::components::TaskKind::GatherResource,
                    progress: 3,
                    required: 10,
                    level: Some(9),
                    hp: Some((10, 10)),
                }]
            },
        }
    }

    fn buff(name: &str, holder: Option<&str>) -> ActiveBuffView {
        ActiveBuffView {
            name: name.to_string(),
            magnitude: "+12 MIT".to_string(),
            remaining: "until rest".to_string(),
            holder_label: holder.map(str::to_string),
        }
    }

    /// A base under load: every block populated, and the longest names the
    /// shipped content can produce.
    fn busy<'a>(
        pets: &'a [PetInfo],
        structures: &'a [StructureReport],
        stock: &'a [StockRow],
        buffs: &'a [ActiveBuffView],
        pack: &'a [PackRow],
        contracts: &'a [ContractRow],
    ) -> PaneData<'a> {
        PaneData {
            roster: (33, 33),
            carrying: 480,
            buffs,
            pack,
            pets,
            structures,
            stock,
            builds: &[],
            labour: LabourDemand {
                wanted: 12,
                staff: 8,
            },
            contracts,
            shielded: true,
            in_base: true,
        }
    }

    /// A contract row at the widest the shipped assets can build, so the
    /// census measures the worst case rather than a convenient one.
    fn contract(name: &str, objective: &str, progress: u32, target: u32) -> ContractRow {
        ContractRow {
            issuer: None,
            issuer_name: None,
            id: feral_processes_engine::contracts::ContractId::from(name),
            name: name.to_string(),
            description: String::new(),
            objective_line: objective.to_string(),
            reward_line: String::new(),
            progress,
            target,
            tutorial: false,
        }
    }
    use super::*;
    use crate::paint::{painted_text, with_painter};
    use crate::text::ui_metrics;

    fn line(n: usize) -> Row {
        text(vec![(format!("row {n}"), palette::BODY, false)])
    }

    fn lines(n: usize) -> Vec<Row> {
        (0..n).map(line).collect()
    }

    /// A pane inside its budget is drawn whole and reports nothing.
    #[test]
    fn a_pane_that_fits_is_not_cut() {
        let m = ui_metrics(720.0);
        let rows = lines(5);
        let (shown, cut) = fitting_rows(&rows, m.line_height * 5.0, &m);
        assert_eq!(cut, 0, "a pane that fits exactly was cut");
        assert_eq!(shown.len(), 5);
    }

    /// The column has no scrollbar, so the overflow is a number the player is
    /// told and never a row that silently is not there.
    #[test]
    fn an_overflowing_pane_counts_what_it_dropped() {
        let m = ui_metrics(720.0);
        let rows = lines(20);
        let (shown, cut) = fitting_rows(&rows, m.line_height * 10.0, &m);
        assert!(cut > 0, "twenty rows in ten rows of room were not counted");
        assert_eq!(shown.len() + cut, rows.len(), "rows vanished uncounted");
    }

    /// **The trap this reserve exists for.** The `+N more` line is itself a
    /// row, so a fitter that fills the budget to the brim draws its own
    /// overflow notice past the bottom edge — the exact silence the count was
    /// added to break. Delete the `- m.line_height` in `fitting_rows` and
    /// this fails.
    #[test]
    fn the_overflow_line_has_room_to_be_drawn() {
        let m = ui_metrics(720.0);
        for budget in 2..14 {
            let avail = m.line_height * budget as f32;
            let rows = lines(40);
            let (shown, cut) = fitting_rows(&rows, avail, &m);
            assert!(cut > 0, "the fixture must overflow at {budget}");
            let drawn: f32 = shown.iter().map(|r| row_height(r, &m)).sum();
            assert!(
                drawn + m.line_height <= avail,
                "at {budget} rows the pane drew {drawn:.1} and the +N more line \
                 needs {:.1} more of {avail:.1}",
                m.line_height
            );
        }
    }

    /// A divider with nothing under it is not information the player lost, so
    /// it comes off the end rather than being reported as a dropped row.
    #[test]
    fn a_trailing_rule_is_dropped_not_counted() {
        let m = ui_metrics(720.0);
        let mut rows = lines(3);
        rows.push(Row::Rule);
        rows.extend(lines(30));
        let (shown, _) = fitting_rows(&rows, m.line_height * 6.0, &m);
        assert!(
            !matches!(shown.last(), Some(Row::Rule)),
            "a divider was left hanging off the end of the pane"
        );
    }

    /// The count is not merely returned — it reaches the screen.
    #[test]
    fn an_overflowing_pane_says_so_on_screen() {
        let m = ui_metrics(720.0);
        let rows = lines(40);
        let at = Rect::new(900.0, 40.0, 400.0, m.line_height * 8.0);
        let (shown, cut) = fitting_rows(&rows, at.h, &m);
        let (_, shapes) = with_painter(|p| draw_rows(at, &shown, cut, p, &m));
        let text = painted_text(&shapes).join(" ");
        assert!(
            text.contains(&format!("+{cut} more")),
            "the pane dropped {cut} rows without saying so: {text:?}"
        );
    }

    /// A chip rides the right end of its own row, never a line of its own —
    /// the handoff's rule that the key is never separated from the thing it
    /// acts on.
    #[test]
    fn a_tail_is_right_aligned_on_its_own_row() {
        let m = ui_metrics(720.0);
        let at = Rect::new(0.0, 0.0, 400.0, 200.0);
        let rows = [with_tail(
            vec![("4 nodes idle".to_string(), palette::BODY, false)],
            chip('b', "base"),
        )];
        let (_, shapes) = with_painter(|p| draw_rows(at, &rows, 0, p, &m));
        let ys: Vec<f32> = shapes
            .iter()
            .filter_map(|s| match &s.shape {
                bevy_egui::egui::epaint::Shape::Text(t) => Some(t.pos.y),
                _ => None,
            })
            .collect();
        assert!(ys.len() >= 2, "the row drew fewer than two pieces: {ys:?}");
        assert!(
            ys.windows(2).all(|w| (w[0] - w[1]).abs() < 0.01),
            "the tail fell onto its own line: {ys:?}"
        );
    }

    /// The column body at the smallest supported window, and what a pane may
    /// lay against it.
    fn column_body(m: &Metrics) -> Rect {
        let mut out = Rect::new(0.0, 0.0, 0.0, 0.0);
        with_painter(|p| {
            let char_w = p.measure_ui_advance("M", m.font_size);
            let r = crate::render::hud::layout::regions(1280.0, 720.0, char_w, m, false);
            out = crate::render::hud::column::regions(r.info_column, m).body;
        });
        out
    }

    /// **The column does not scroll.** A row past the bottom is now counted
    /// rather than dropped in silence, so this census is no longer about
    /// silence — it is about the count being rare. A base under load must
    /// fit whole at 1280x720, or the player is told "+N more" as the normal
    /// state of the HUD and the figure stops meaning anything.
    ///
    /// `the_tallest_gear_page_fits_its_popup`'s trap in a taller box.
    #[test]
    fn the_tallest_column_pane_fits_its_column() {
        let m = ui_metrics(720.0);
        let body = column_body(&m);
        let pets: Vec<PetInfo> = (0..6)
            .map(|i| pet(&format!("Program {i}"), (i < 3).then_some(i)))
            .collect();
        let structures: Vec<StructureReport> = (0..6)
            .map(|i| structure(&format!("Research Node {i}"), i % 2 == 0))
            .collect();
        let stock: Vec<StockRow> = (0..6)
            .map(|i| StockRow {
                item: feral_processes_engine::items::ItemId::from(format!("item_{i}")),
                tag: "CG".to_string(),
                name: format!("Cache Grain {i}"),
                qty: 480,
            })
            .collect();
        let buffs = [
            buff("Ablative Layer", Some("Program 0")),
            buff("Patch", None),
        ];
        let pack: Vec<PackRow> = (0..6)
            .map(|i| PackRow {
                qty: 12,
                name: format!("Compiled Shard {i}"),
                tier: 0,
            })
            .collect();
        let contracts = [
            contract("Reclamation Order", "Terminate 12 wild programs", 8, 12),
            contract("Deep Sounding", "Stand 4 frames down a Stack", 0, 1),
            contract("Cache Requisition", "Deliver 12 Cache Grain", 12, 12),
        ];
        let d = busy(&pets, &structures, &stock, &buffs, &pack, &contracts);
        for tab in InfoTab::ALL {
            let (_, cut) = fitting_rows(&rows(tab, &d), body.h, &m);
            assert_eq!(
                cut,
                0,
                "{} drops {cut} rows off a base under load at 1280x720",
                tab.label()
            );
        }
    }

    /// The column truncates through [`cell`], so a wide name cannot overflow
    /// it — which makes a width census alone vacuous here. What can still go
    /// wrong is the truncation *biting*: a shipped contract whose name is
    /// cut mid-word reads as a bug in the pane rather than as content that
    /// outgrew its cell. So this measures the shipped set against the cells
    /// rather than against the column.
    ///
    /// `Game::contract_catalogue` is the widest row the assets can build —
    /// templates resolved at their longest, not the authored strings, which
    /// carry `{target}` holes and understate it.
    #[test]
    fn no_shipped_contract_is_truncated_in_the_column() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = feral_processes_engine::Game::new(
            41,
            feral_processes_engine::DifficultyMode::Forgiving,
            assets,
        )
        .expect("shipped assets");
        let catalogue = game.contract_catalogue();
        assert!(
            !catalogue.is_empty(),
            "the census measured nothing \u{2014} the shipped set has to reach here"
        );
        for row in &catalogue {
            assert!(
                row.name.chars().count() <= CONTRACT_NAME_CELLS,
                "\"{}\" is {} cells and the column gives a name {CONTRACT_NAME_CELLS}",
                row.name,
                row.name.chars().count()
            );
            assert!(
                row.objective_line.chars().count() <= CONTRACT_OBJECTIVE_CELLS,
                "\"{}\" is {} cells and the column gives an objective \
                 {CONTRACT_OBJECTIVE_CELLS}",
                row.objective_line,
                row.objective_line.chars().count()
            );
        }
    }

    /// **The column is a fixed slice of the window and cannot widen**, and
    /// `draw_line` clips nothing horizontally — so an over-wide row is drawn
    /// off the panel in silence. This is the ceiling the buff-tag rule moved
    /// to when the buffs left the status panel for the CREW tab: the widest
    /// shipped buff row already spent all but a few cells of the old column,
    /// and a companion's `(holder)` tag once drew 360px past it.
    ///
    /// Measures the left run and the right tail **joined**, because
    /// `draw_line` right-aligns the tail against the same body width the left
    /// run starts in — measuring the head alone budgets for a row narrower
    /// than the one that is drawn, `caravan.rs`' rule.
    #[test]
    fn no_column_row_overflows_the_column() {
        let m = ui_metrics(720.0);
        let body = column_body(&m);
        let room = body.w - m.inset * 2.0;
        let pets = [pet("Grubtender Prime", Some(0)), pet("Hexweave", None)];
        let structures = [
            structure("Research Node", true),
            structure("Assembly Lathe", false),
        ];
        let stock = [StockRow {
            item: feral_processes_engine::items::ItemId::from("cache_grain"),
            tag: "CG".to_string(),
            name: "Cache Grain".to_string(),
            qty: 9999,
        }];
        // The documented failure: a long routine on a long-named holder.
        let buffs = [buff("Ablative Layer", Some("Grubtender Prime"))];
        let pack = [PackRow {
            qty: 999,
            name: "Recompiled Pulse Blade".to_string(),
            tier: 3,
        }];
        let contracts = [
            contract("Reclamation Order", "Terminate 12 wild programs", 8, 12),
            contract("Deep Sounding", "Stand 4 frames down a Stack", 0, 1),
            contract("Cache Requisition", "Deliver 12 Cache Grain", 12, 12),
        ];
        let d = busy(&pets, &structures, &stock, &buffs, &pack, &contracts);
        with_painter(|p| {
            for tab in InfoTab::ALL {
                for row in rows(tab, &d) {
                    let Row::Text { left, right } = &row else {
                        continue;
                    };
                    let joined: String = left
                        .iter()
                        .chain(right.iter())
                        .map(|(t, _, _)| t.as_str())
                        .collect();
                    let drawn = p.measure_ui_advance(&joined, m.font_size);
                    assert!(
                        drawn <= room,
                        "a {} row overflows the column by {:.0}px \
                         ({drawn:.0} drawn into {room:.0}):\n{joined}",
                        tab.label(),
                        drawn - room
                    );
                }
            }
        });
    }

    /// Every tab summarises to something, and none of them to nothing — a
    /// blank collapsed bar is the one state the bars exist to prevent.
    #[test]
    fn every_tab_has_a_live_summary() {
        let pets = [pet("Archive", Some(0))];
        let d = busy(&pets, &[], &[], &[], &[], &[]);
        for tab in InfoTab::ALL {
            let s = summary(tab, &d);
            assert!(
                !s.trim().is_empty(),
                "{} summarises to nothing",
                tab.label()
            );
        }
    }
}
