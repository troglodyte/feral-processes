use super::*;

/// The structure roster: everything standing in the zone and every program
/// posted to it.
///
/// Read-only, and the one screen that shows the base as a whole rather than
/// what happens to be within `MENU_SCAN_RADIUS` — see
/// `Game::structure_report`, which is also where the row order is decided so
/// that this draws it rather than inventing one.
///
/// An idle workable structure is drawn in yellow and says so in words: it is
/// the only thing on this screen the player can act on, and the point of
/// looking is usually to find it.
pub(crate) fn structures_rows(game: &mut Game, selected: usize) -> Vec<Row> {
    let Roster {
        reports: report,
        lines,
        rows: roster,
    } = Roster::of(game);
    let assigned: usize = report.iter().map(|s| s.assignees.len()).sum();
    let idle = StructureReport::idle_count(&report);
    let (draw, supply) = game.base_power();
    let battery = game.base_battery();
    let mut rows = vec![
        text_row(format!(
            "{} structure{}, {assigned} program{} assigned, {idle} idle",
            report.len(),
            if report.len() == 1 { "" } else { "s" },
            if assigned == 1 { "" } else { "s" },
        )),
        grid_header_row(draw, supply, battery),
        text_row(""),
    ];
    if report.is_empty() {
        rows.push(text_row("You have deployed nothing yet."));
    }
    for (i, row) in roster.iter().enumerate() {
        // A structure's sub-lines are `Row::Item` (never selected) rather than
        // `Row::Text` so they sit inside the popup's scrollable body:
        // `popup_layout` ends that body at the *last* Item and pins whatever
        // follows it as a footer, which would otherwise leave the final
        // structure's assignees stuck on screen while the list scrolled past
        // them.
        let (headline, color, detail) = match *row {
            RosterRow::Structure(at) => {
                let s = &report[at];
                (
                    structure_headline(s),
                    if structure_is_idle(s, &[]) {
                        YELLOW
                    } else {
                        TEXT
                    },
                    structure_detail_lines(s, None, structure_is_idle(s, &[])),
                )
            }
            RosterRow::Line(key) => {
                let members: Vec<&StructureReport> =
                    report.iter().filter(|s| s.line == Some(key)).collect();
                match lines.iter().find(|l| l.key == key) {
                    Some(line) => (
                        line_headline(line),
                        if StructureReport::line_is_idle(members.iter().copied()) {
                            YELLOW
                        } else {
                            TEXT
                        },
                        members
                            .iter()
                            .flat_map(|s| s.assignees.iter())
                            .map(|a| (format!("  {}", assignee_line(a)), TEXT_DIM))
                            .collect(),
                    ),
                    None => continue,
                }
            }
        };
        rows.push(colored_item_row(headline, i == selected, color));
        for (line, color) in detail {
            rows.push(colored_item_row(line, false, color));
        }
    }
    rows
}

/// A production line's roster row: its machines in feed order, then what the
/// line is doing (spec §5).
pub(crate) fn line_headline(line: &LineReport) -> String {
    let doing = match (&line.missing_text, line.active) {
        (Some(text), _) => text.clone(),
        (None, Some(active)) => {
            let name = line
                .members
                .iter()
                .position(|&m| m == active)
                .map_or("", |at| line.names[at].as_str());
            format!("working {name}")
        }
        (None, None) => status_word(line.status).to_string(),
    };
    format!("{} · {doing}", line.names.join(" → "))
}

pub(super) fn status_word(status: MachineStatus) -> &'static str {
    match status {
        MachineStatus::Running => "running",
        MachineStatus::Starved => "starved",
        MachineStatus::Clogged => "clogged",
        MachineStatus::Unstaffed => "no one at it",
        MachineStatus::Stranded => "cut off",
        MachineStatus::Idle => "idle",
        MachineStatus::Unpowered => "dark",
        MachineStatus::Dry => "out of fuel",
        MachineStatus::Uncharged => "uncharged",
        MachineStatus::Unconfigured => "nothing set",
    }
}

/// The roster's second header row: the base's grid, red when it is short.
///
/// Reads `draw` and `supply` straight from `Game::base_power` rather than
/// comparing any one machine's numbers — which machine went dark is
/// `MachineStatus::Unpowered`'s job, not this row's. **"Grid", never
/// "Power"**: `Power` already names a creature's `PowerReserve` in the status
/// column, and this pane sits two panels from it.
pub(super) fn grid_header_row(draw: u32, supply: u32, battery: Option<(u32, u32)>) -> Row {
    let mut text = format!("Grid  {draw} / {supply}");
    if let Some((stored, capacity)) = battery {
        text.push_str(&format!("  ·  Battery {stored} / {capacity}"));
    }
    if draw > supply {
        Row::TextColored(text, RED)
    } else {
        text_row(text)
    }
}

/// A workable structure with nobody posted to it — the one thing on either
/// structure screen the player can immediately act on, which is why both
/// colour it yellow.
///
/// A call and not a second copy: `Game::attention` counts the same thing in
/// the engine, so the two readers are in different crates and nothing would
/// fail to compile if one drifted. `line` is the whole line when `s` is a
/// member of one: a member of a staffed line has no assignee of its own and
/// is not idle.
pub(crate) fn structure_is_idle(s: &StructureReport, line: &[&StructureReport]) -> bool {
    if s.line.is_some() {
        s.workable && StructureReport::line_is_idle(line.iter().copied())
    } else {
        s.is_idle()
    }
}

/// The one-line summary of a structure: what it is, where, how far, and how
/// battered. Shared with the inspector's single-structure sheet.
pub(crate) fn structure_headline(s: &StructureReport) -> String {
    let tier = s.tier.map(|t| format!(" T{t}")).unwrap_or_default();
    let durability = s
        .durability
        .map(|(hp, max)| format!("  {hp}/{max} HP"))
        .unwrap_or_default();
    format!(
        "{}{tier}  ({}, {})  {}d{durability}",
        s.label, s.pos.0, s.pos.1, s.distance
    )
}

/// Everything under the headline — idleness, assignees, a stall, the two
/// buffers — as `(text, colour)` pairs.
///
/// Extracted rather than written twice: the Structures roster and the inspector's
/// sheet describe the same machine, and a detail screen that disagreed with
/// the roster about whether something was starved is exactly the drift
/// `CLAUDE.md` means by a mirror having to be a call. The two differ only in
/// which `Row` kind they wrap these in — the roster needs `Row::Item` so its
/// lines scroll, the sheet does not scroll at all.
pub(crate) fn structure_detail_lines(
    s: &StructureReport,
    line: Option<&LineReport>,
    idle: bool,
) -> Vec<(String, Color)> {
    let mut lines = Vec::new();
    // A row of its own: the headline already carries label, position,
    // distance and durability, and the widest of those leaves no room for it.
    if let Some(room) = &s.room {
        lines.push((format!("  {room}"), TEXT_DIM));
    }
    if let Some(line) = line {
        lines.push((
            format!("  Part of a line: {}.", line.names.join(" → ")),
            TEXT_DIM,
        ));
    }
    if idle {
        lines.push(("  idle — nobody assigned".to_string(), YELLOW));
    }
    for a in &s.assignees {
        lines.push((format!("  {}", assignee_line(a)), TEXT_DIM));
    }
    // A stall is drawn in yellow for the same reason an idle structure is:
    // it is a thing the player can walk over and fix.
    if let Some(line) = stall_line(s) {
        lines.push((format!("  {line}"), YELLOW));
    }
    // Directly under the stall, which is the line it explains: "uncharged"
    // says the buffer is empty and this says what fills it. Drawn while it
    // holds some too — the `in:` row below counts the stock, not the
    // ceiling — and dim, because the crew fetches it on its own.
    if let Some((item, capacity)) = &s.supply {
        lines.push((format!("  takes {item}, holds up to {capacity}"), TEXT_DIM));
    }
    if let Some((stored, capacity)) = s.battery {
        lines.push((format!("  charge {stored}/{capacity}"), TEXT_DIM));
    }
    if let Some(line) = buffer_line("in", &s.input, None) {
        lines.push((line, TEXT_DIM));
    }
    if let Some(line) = buffer_line("out", &s.output, Some(s.output_capacity)) {
        lines.push((line, TEXT_DIM));
    }
    // The tool a rig strips with — the starter on one nobody has fitted.
    if let Some(tool) = &s.standing_tool {
        lines.push((format!("  set up with the {tool}"), TEXT_DIM));
    }
    // A Holding Cell: who is inside and how the next roll stands. The odds
    // are the engine's own (`Game::jail_odds`), the figure the roll is made
    // against, so this line cannot quote a different chance.
    if let Some(prison) = &s.prison {
        match &prison.prisoner {
            Some(p) => {
                lines.push((format!("  holds {}", p.name), TEXT_DIM));
                let clock = if p.attempts >= p.max_attempts {
                    format!(
                        "  attempts {}/{}  waiting for room in the output",
                        p.attempts, p.max_attempts
                    )
                } else {
                    format!(
                        "  attempts {}/{}  beats {}/{}  next roll {}%",
                        p.attempts, p.max_attempts, p.progress, p.attempt_ticks, p.odds_percent
                    )
                };
                lines.push((clock, TEXT_DIM));
            }
            None => lines.push(("  empty".to_string(), TEXT_DIM)),
        }
        if !prison.warded {
            lines.push(("  no warden — nothing is decompiling".to_string(), YELLOW));
        }
    }
    // Dim either way: the crew reloads a spent pod on its own, so there is
    // nothing here for the player to walk over and fix.
    match s.pod {
        Some(PodState::Charged) => lines.push(("  charged — ready to drop".into(), TEXT_DIM)),
        Some(PodState::Recharging { percent }) => {
            lines.push((format!("  recharging — {percent}% reloaded"), TEXT_DIM));
        }
        None => {}
    }
    lines
}

/// Why a machine is stalled, or `None` when it is running or is not a
/// machine at all. `Idle` says nothing here — the "nobody assigned" line
/// already above it is the same fact in better words.
pub(super) fn stall_line(s: &StructureReport) -> Option<&'static str> {
    match s.status? {
        MachineStatus::Starved => Some("starved — nothing is feeding it"),
        MachineStatus::Clogged => Some("clogged — collect from it with c"),
        MachineStatus::Unstaffed => Some("no one at it — its program is away"),
        MachineStatus::Stranded => Some("cut off — its program can't reach it"),
        // Says the fact and stops. It used to end "build a Recharger Node",
        // which is the wrong move on the base most likely to be reading this
        // line: one whose Rechargers are all standing there `Dry`. A fifth
        // would go dry beside them. The grid figure on the status bar is
        // where the player reads how short, and a dry supplier says so in its
        // own words.
        MachineStatus::Unpowered => Some("dark — the grid is short"),
        MachineStatus::Dry => Some("out of fuel — no Power Cells beside it"),
        MachineStatus::Uncharged => Some("uncharged — its supply has run out"),
        MachineStatus::Unconfigured => Some("nothing set — stand beside it and press R"),
        MachineStatus::Running | MachineStatus::Idle => None,
    }
}

/// One buffer as a line, or `None` when it is empty — a base of empty
/// buffers would otherwise double the length of this screen to say nothing.
pub(super) fn buffer_line(
    label: &str,
    stock: &[(String, u32)],
    capacity: Option<u32>,
) -> Option<String> {
    if stock.is_empty() {
        return None;
    }
    let contents = stock
        .iter()
        .map(|(name, n)| format!("{n} {name}"))
        .collect::<Vec<_>>()
        .join(", ");
    Some(match capacity {
        Some(cap) => {
            let used: u32 = stock.iter().map(|(_, n)| n).sum();
            format!("  {label}: {contents}  [{used}/{cap}]")
        }
        None => format!("  {label}: {contents}"),
    })
}

/// One assignee row: who it is, how it is holding up, what it is doing, and
/// how far into a cycle it is. A guard has no cycle to be partway through —
/// `systems::task_progress_system` ignores the kind entirely — so it gets no
/// progress figure rather than a permanent `0/0`.
///
/// The vitals are here because this is the only screen that can carry them
/// for a posted program: at its post it is not drawn on the map, and the
/// inspector names only what is drawn, so there is no way to open its own
/// manifest without first calling it off the job.
pub(super) fn assignee_line(a: &Assignee) -> String {
    let who = format!("{}{}", a.label, assignee_vitals(a));
    match a.kind {
        TaskKind::GatherResource => format!("{who} — cronjob {}/{}", a.progress, a.required),
        TaskKind::Guard => format!("{who} — guarding"),
        // A dig site is not a structure, so nothing on this screen can be
        // holding an `Excavate` post — the row exists because the match is
        // exhaustive, and it reads like the cronjob one for the day
        // something does.
        TaskKind::Excavate => format!("{who} — cutting {}/{}", a.progress, a.required),
        // A build site is not a structure either, so this row is unreachable
        // for the same reason the one above it is. It reads like the cronjob
        // one for the day something puts a builder on this screen.
        TaskKind::Construct => format!("{who} — building {}/{}", a.progress, a.required),
        TaskKind::Tend(_) => format!("{who} — tending {}/{}", a.progress, a.required),
    }
}

/// `" Lv7 HP 18/22"`, or empty for anything missing the components. Matching
/// the party roster's `Lv{n} HP {a}/{b}` so one program reads the same on
/// both screens.
pub(super) fn assignee_vitals(a: &Assignee) -> String {
    let level = a.level.map(|l| format!(" Lv{l}")).unwrap_or_default();
    let hp =
        a.hp.map(|(hp, max)| format!(" HP {hp}/{max}"))
            .unwrap_or_default();
    format!("{level}{hp}")
}

/// How wide the item column is. A name longer than this is cut rather than
/// allowed to push the four figures out of their columns: the page is read
/// down a column, and one modded name would otherwise skew every row under
/// it.
pub(super) const OUTPUT_NAME_COLUMN: usize = 22;

/// The page's rows, out of the report rather than out of a `Game` — the
/// split `memory_page_rows` makes, and for its reason: the height and width
/// censuses have to measure the page at its **worst** case, which is a
/// report a fixture can state and a `Game` would have to be played into.
pub(crate) fn base_output_rows(report: &BaseOutputReport) -> Vec<Row> {
    let mut rows = vec![Row::TextColored(
        format!("sector {} · this run", report.zone),
        CYAN,
    )];

    if report.mined.is_empty() && report.compiled.is_empty() {
        rows.push(text_row("The base has not made anything yet."));
    } else {
        rows.push(Row::TextColored(
            format!(
                "  {:<width$}{:>8}{:>9}{:>8}{:>8}",
                "",
                "sector",
                "machine",
                "hand",
                "run",
                width = OUTPUT_NAME_COLUMN,
            ),
            TEXT_DIM,
        ));
        // Both sections carry the same four figures rather than the sector/run
        // pair the design sketch gave MINED alone. An item sits in one
        // section on *dominant* provenance, so a Power Cell a machine makes
        // more of than the player does lands under MINED — and dropping the
        // hand column there would hide exactly the units the page exists to
        // count.
        for (heading, section) in [("MINED", &report.mined), ("COMPILED", &report.compiled)] {
            if section.is_empty() {
                continue;
            }
            rows.push(Row::TextColored(heading.to_string(), YELLOW));
            for row in section.iter() {
                rows.push(text_row(base_output_line(row)));
            }
        }
    }

    // Every row `Game::attention` holds, never a slice of them: it caps
    // itself at four by construction, and the page's row budget is set
    // against that — a page that dropped the fourth would be silent about
    // exactly the state the player opened it to act on.
    rows.push(text_row(""));
    for row in &report.attention {
        rows.push(Row::TextColored(
            format!("needs attention: {}", row.text),
            if row.threat { RED } else { YELLOW },
        ));
    }

    // The sentence and the key share a row because rows are what this page
    // is short of. The sentence itself is the one thing this page is better
    // placed to teach than any other screen: without it, a sector total that
    // grew while the party was underground reads as a counting bug.
    rows.push(text_row(
        "The base works while you are away — the sector total counts that time.",
    ));
    rows
}

/// One item's line: the name, the four figures, and the shape of the last
/// few windows.
pub(super) fn base_output_line(row: &BaseOutputRow) -> String {
    let name = if row.name.chars().count() > OUTPUT_NAME_COLUMN {
        let mut cut: String = row.name.chars().take(OUTPUT_NAME_COLUMN - 1).collect();
        cut.push('…');
        cut
    } else {
        row.name.clone()
    };
    format!(
        "  {:<width$}{:>8}{:>9}{:>8}{:>8}  {}",
        name,
        row.sector,
        row.machine,
        row.hand,
        row.run,
        spark_line(&row.spark),
        width = OUTPUT_NAME_COLUMN,
    )
}

/// The eight block glyphs a sparkline is drawn from, shortest first.
///
/// U+2581..U+2588, which DejaVu Sans Mono — the UI face, and the one every
/// popup row is measured in — carries; the map's unscii face is never
/// reached from a popup.
pub(super) const SPARK_BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// A row's recent windows as bars, scaled to **that row's own** peak.
///
/// Per row rather than across the page: a sparkline beside a four-figure
/// total is being read for a shape — is this climbing or falling — and
/// scaling every row to the busiest item on the page flattens each of the
/// slower ones into a single flat line that says nothing.
///
/// The windows are the ones the ledger *kept*: a bucket with no events is
/// never stored, so a quiet stretch closes up rather than drawing as a gap.
pub(super) fn spark_line(values: &[u32]) -> String {
    let max = values.iter().copied().max().unwrap_or(0);
    values
        .iter()
        .map(|v| {
            if max == 0 {
                SPARK_BARS[0]
            } else {
                SPARK_BARS[(*v as usize * (SPARK_BARS.len() - 1)) / max as usize]
            }
        })
        .collect()
}
