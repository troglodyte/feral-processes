//! The build, staffing, demolition, upgrade and symlink pickers.

use super::popup::*;
use super::*;

/// One buildable structure as the build menu needs it: everything that
/// required a `Game` to work out, already worked out.
pub(super) struct BuildEntry {
    pub label: String,
    pub description: String,
    pub category: StructureCategory,
}

/// The heading a group opens with, or `None` for Home — a single structure
/// under a "Shelter" banner is a heading longer than the thing it labels.
fn category_heading(category: StructureCategory) -> Option<&'static str> {
    match category {
        StructureCategory::Home => None,
        StructureCategory::Extractor => Some("── Extractors — produce on their own ──"),
        StructureCategory::Assembler => Some("── Assemblers — fed by what they touch ──"),
        StructureCategory::Utility => Some("── Utility ──"),
        StructureCategory::Trade => Some("── Trade ──"),
        StructureCategory::Defence => Some("── Defence ──"),
    }
}

/// The build menu's rows, pure so the layout invariant below can be tested
/// without a `Game` or a `Painter`.
///
/// **Every row after the first is a `Row::Item`, including the headings and
/// the descriptions.** `popup_layout` ends the scrollable body at the *last*
/// `Row::Item` and pins whatever follows as a footer — so a description
/// emitted as `Row::Text` would slice the final structure's description off
/// the bottom of the list and strand it under the scroll indicator, detached
/// from the row it describes. `draw_structures` carries the same fix for the
/// same reason.
///
/// `entries` is assumed to arrive grouped by category, which
/// `StructureDb::all` guarantees; a heading is emitted whenever the category
/// changes, so an ungrouped list would simply repeat headings rather than
/// mislabel anything.
pub(super) fn build_menu_rows(entries: &[BuildEntry], selected: usize) -> Vec<Row> {
    let mut rows = vec![text_row("Esc to cancel; Up/Down + Enter also work")];
    let mut current: Option<StructureCategory> = None;
    for (i, entry) in entries.iter().enumerate() {
        if current != Some(entry.category) {
            current = Some(entry.category);
            if let Some(heading) = category_heading(entry.category) {
                rows.push(colored_item_row("", false, TEXT_DIM));
                rows.push(colored_item_row(heading, false, TEXT_DIM));
            }
        }
        rows.push(item_row(
            format!("[{}] {}", menu_shortcut(i), entry.label),
            i == selected,
        ));
        rows.push(colored_item_row(
            format!("    {}", entry.description),
            false,
            TEXT_DIM,
        ));
    }
    rows
}

pub(super) fn draw_build_menu(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let status = game.player_status();
    let defs = game.buildable_structure_defs();
    let entries: Vec<BuildEntry> = defs
        .iter()
        .map(|def| {
            let raw_cost = game.structure_build_cost(def);
            let cost = cost_display(game, &raw_cost, &status.inventory);
            BuildEntry {
                label: format!("{} - {}", def.name, cost.join(", ")),
                description: def.description.clone(),
                category: def.category(),
            }
        })
        .collect();
    let rows = build_menu_rows(&entries, selected);
    draw_popup("Deploy", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_worker_menu(
    game: &mut Game,
    workers: &[EntityView],
    title: &str,
    prompt: &str,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    // `view_entities` doesn't carry a raw power number, only a level and
    // an HP fraction — cross-reference `owned_pets` for it, same as the
    // fuse menu does.
    let pets = game.owned_pets();
    let mut rows = vec![text_row(format!(
        "{prompt} (Esc to cancel; Up/Down + Enter also work)"
    ))];
    if workers.is_empty() {
        rows.push(text_row("(no compiled programs nearby)"));
    }
    for (i, w) in workers.iter().enumerate() {
        let pet = pets.iter().find(|p| p.entity == w.entity);
        let power = pet.map(|p| format!(" PWR {}", p.power)).unwrap_or_default();
        let activity = pet.map(|p| activity_tag(&p.activity)).unwrap_or_default();
        rows.push(item_row(
            format!(
                "[{}] {}{}{} at ({}, {}){}",
                menu_shortcut(i),
                w.label,
                w.level.map(|l| format!(" Lv{l}")).unwrap_or_default(),
                power,
                w.pos.0,
                w.pos.1,
                activity
            ),
            i == selected,
        ));
    }
    draw_popup(title, PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_structure_menu(
    structures: &[EntityView],
    title: &str,
    prompt: &str,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row(format!(
        "{prompt} (Esc to cancel; Up/Down + Enter also work)"
    ))];
    if structures.is_empty() {
        rows.push(text_row("(no structures nearby)"));
    }
    for (i, s) in structures.iter().enumerate() {
        let assigned = s
            .structure_worker
            .as_ref()
            .map(|w| format!(" (assigned: {w})"))
            .unwrap_or_default();
        let durability = s
            .durability
            .map(|(hp, max)| format!(" [HP {hp}/{max}]"))
            .unwrap_or_default();
        rows.push(item_row(
            format!(
                "[{}] {} at ({}, {}){}{}",
                menu_shortcut(i),
                s.label,
                s.pos.0,
                s.pos.1,
                durability,
                assigned
            ),
            i == selected,
        ));
    }
    draw_popup(title, PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_remove_menu(
    structures: &[EntityView],
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row(
        "Demolish which structure? Removing Home destroys the whole base. (Esc to cancel; Up/Down + Enter also work)",
    )];
    if structures.is_empty() {
        rows.push(text_row("(no structures nearby)"));
    }
    for (i, s) in structures.iter().enumerate() {
        let durability = s
            .durability
            .map(|(hp, max)| format!(" [HP {hp}/{max}]"))
            .unwrap_or_default();
        let home_tag = if s.is_home { " (Home)" } else { "" };
        rows.push(item_row(
            format!(
                "[{}] {} at ({}, {}){}{}",
                menu_shortcut(i),
                s.label,
                s.pos.0,
                s.pos.1,
                durability,
                home_tag
            ),
            i == selected,
        ));
    }
    draw_popup("Demolish Structure", PopupSize::Large, &rows, painter, m);
}

/// The bracketed tier tag on an upgrade-menu row.
///
/// A structure sitting at its ceiling is still listed rather than filtered
/// out — see `App::upgradeable_structures` — so the row has to say why it
/// has stopped. Whether that is temporary is exactly the difference between
/// `ceiling` and `max_tier`: below `max_tier` the zone is what's holding it,
/// and one more breach frees the next tier. At `max_tier` it is simply
/// finished, and the plain tag it has always shown is right.
fn tier_tag(s: &EntityView) -> String {
    let tier = s.tier.unwrap_or(1);
    match (s.ceiling, s.max_tier) {
        (Some(ceiling), Some(max_tier)) if tier >= ceiling && ceiling < max_tier => {
            let next = tier + 1;
            format!("Mk{tier} — zone {next} unlocks Mk{next}")
        }
        _ => format!("Mk{tier}"),
    }
}

pub(super) fn draw_upgrade_menu(
    structures: &[EntityView],
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row(
        "Upgrade which structure? Each tier costs more and yields more. (Esc to cancel; Up/Down + Enter also work)",
    )];
    if structures.is_empty() {
        rows.push(text_row("(no upgradeable structures nearby)"));
    }
    for (i, s) in structures.iter().enumerate() {
        rows.push(item_row(
            format!(
                "[{}] {} at ({}, {}) [{}]",
                menu_shortcut(i),
                s.label,
                s.pos.0,
                s.pos.1,
                tier_tag(s),
            ),
            i == selected,
        ));
    }
    draw_popup("Upgrade Structure", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_remove_confirm(selected: usize, painter: &Painter, m: &Metrics) {
    let rows = vec![
        Row::TextColored(
            "Removing Home destroys every other structure in this base and refunds".to_string(),
            ORANGE,
        ),
        Row::TextColored(
            "30% of each one's materials. This can't be undone.".to_string(),
            ORANGE,
        ),
        text_row(""),
        item_row("[y] Yes, demolish everything", selected == 0),
        item_row("[n] No, cancel", selected == 1),
    ];
    draw_popup("Confirm Demolish Home", PopupSize::Small, &rows, painter, m);
}

pub(super) fn draw_symlink_menu(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let status = game.player_status();
    let targets = game.symlink_targets();
    let mut rows = vec![text_row(
        "Use symlink to which structure? (Esc to cancel; Up/Down + Enter also work)",
    )];
    if targets.is_empty() {
        rows.push(text_row("(no symlink-capable structures deployed yet)"));
    }
    for (i, t) in targets.iter().enumerate() {
        let raw_cost = game.symlink_cost(t.entity).unwrap_or_default();
        let cost = cost_display(game, &raw_cost, &status.inventory);
        let durability = t
            .durability
            .map(|(hp, max)| format!(" [HP {hp}/{max}]"))
            .unwrap_or_default();
        rows.push(item_row(
            format!(
                "[{}] {} at ({}, {}){} - {}",
                menu_shortcut(i),
                t.label,
                t.pos.0,
                t.pos.1,
                durability,
                cost.join(", ")
            ),
            i == selected,
        ));
    }
    draw_popup("Symlink", PopupSize::Large, &rows, painter, m);
}

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
pub(super) fn draw_structures(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let report = game.structure_report();
    let assigned: usize = report.iter().map(|s| s.assignees.len()).sum();
    let idle = report
        .iter()
        .filter(|s| s.workable && s.assignees.is_empty())
        .count();
    let mut rows = vec![
        text_row(format!(
            "{} structure{}, {assigned} program{} assigned, {idle} idle",
            report.len(),
            if report.len() == 1 { "" } else { "s" },
            if assigned == 1 { "" } else { "s" },
        )),
        text_row(""),
    ];
    if report.is_empty() {
        rows.push(text_row("You have deployed nothing yet."));
    }
    for (i, s) in report.iter().enumerate() {
        rows.push(colored_item_row(
            structure_headline(s),
            i == selected,
            if structure_is_idle(s) { YELLOW } else { TEXT },
        ));
        // A structure's sub-lines are `Row::Item` (never selected) rather than
        // `Row::Text` so they sit inside the popup's scrollable body:
        // `popup_layout` ends that body at the *last* Item and pins whatever
        // follows it as a footer, which would otherwise leave the final
        // structure's assignees stuck on screen while the list scrolled past
        // them.
        for (line, color) in structure_detail_lines(s) {
            rows.push(colored_item_row(line, false, color));
        }
    }
    rows.push(text_row(""));
    rows.push(text_row("Up/Down to scroll, Esc to close."));
    draw_popup("Structures", PopupSize::Large, &rows, painter, m);
}

/// A workable structure with nobody posted to it — the one thing on either
/// structure screen the player can immediately act on, which is why both
/// colour it yellow.
pub(super) fn structure_is_idle(s: &StructureReport) -> bool {
    s.workable && s.assignees.is_empty()
}

/// The one-line summary of a structure: what it is, where, how far, and how
/// battered. Shared with the inspector's single-structure sheet.
pub(super) fn structure_headline(s: &StructureReport) -> String {
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
/// Extracted rather than written twice: the `B` roster and the inspector's
/// sheet describe the same machine, and a detail screen that disagreed with
/// the roster about whether something was starved is exactly the drift
/// `CLAUDE.md` means by a mirror having to be a call. The two differ only in
/// which `Row` kind they wrap these in — the roster needs `Row::Item` so its
/// lines scroll, the sheet does not scroll at all.
pub(super) fn structure_detail_lines(s: &StructureReport) -> Vec<(String, Color)> {
    let mut lines = Vec::new();
    if structure_is_idle(s) {
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
    if let Some(line) = buffer_line("in", &s.input, None) {
        lines.push((line, TEXT_DIM));
    }
    if let Some(line) = buffer_line("out", &s.output, Some(s.output_capacity)) {
        lines.push((line, TEXT_DIM));
    }
    lines
}

/// Why a machine is stalled, or `None` when it is running or is not a
/// machine at all. `Idle` says nothing here — the "nobody assigned" line
/// already above it is the same fact in better words.
fn stall_line(s: &StructureReport) -> Option<&'static str> {
    match s.status? {
        MachineStatus::Starved => Some("starved — nothing is feeding it"),
        MachineStatus::Clogged => Some("clogged — collect from it with c"),
        MachineStatus::Running | MachineStatus::Idle => None,
    }
}

/// One buffer as a line, or `None` when it is empty — a base of empty
/// buffers would otherwise double the length of this screen to say nothing.
fn buffer_line(label: &str, stock: &[(String, u32)], capacity: Option<u32>) -> Option<String> {
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

/// One assignee row: who it is, what it is doing, and how far into a cycle it
/// is. A guard has no cycle to be partway through —
/// `systems::task_progress_system` ignores the kind entirely — so it gets no
/// progress figure rather than a permanent `0/0`.
fn assignee_line(a: &Assignee) -> String {
    match a.kind {
        TaskKind::GatherResource => {
            format!("{} — cronjob {}/{}", a.label, a.progress, a.required)
        }
        TaskKind::Guard => format!("{} — guarding", a.label),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(tier: u32, ceiling: u32, max_tier: u32) -> EntityView {
        EntityView {
            entity: Entity::PLACEHOLDER,
            pos: (0, 0),
            glyph: 'n',
            color: GlyphColor::White,
            label: "Mining Node".into(),
            is_player: false,
            is_tamed: false,
            is_companion: false,
            is_hostile: false,
            is_structure: true,
            is_home: false,
            tier: Some(tier),
            ceiling: Some(ceiling),
            max_tier: Some(max_tier),
            is_boss: false,
            can_work: false,
            can_trade: false,
            structure_worker: None,
            hp_fraction: None,
            level: None,
            durability: None,
            fusions: 0,
            machine_status: None,
            linked_edges: Vec::new(),
        }
    }

    #[test]
    fn a_tier_below_the_ceiling_reads_as_it_always_has() {
        assert_eq!(tier_tag(&view(2, 5, 5)), "Mk2");
    }

    #[test]
    fn a_tier_stopped_by_the_zone_says_which_zone_would_free_it() {
        assert_eq!(tier_tag(&view(1, 1, 5)), "Mk1 — zone 2 unlocks Mk2");
    }

    #[test]
    fn a_tier_stopped_by_the_defs_own_ceiling_says_nothing_about_zones() {
        assert_eq!(tier_tag(&view(5, 5, 5)), "Mk5");
    }
}
