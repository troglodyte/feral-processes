use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Gauge, Paragraph, Wrap};
use ratatui::Frame;

use feral_processes_engine::components::GlyphColor;
use feral_processes_engine::items::ItemId;
use feral_processes_engine::world::{Biome, Tile};
use feral_processes_engine::{EntityView, Game, PlayerStatus};

use crate::{App, Mode};

pub fn render(f: &mut Frame, app: &mut App) {
    match app.mode {
        Mode::MainMenu => render_main_menu(f, app),
        Mode::DifficultyPick => render_difficulty_pick(f),
        Mode::GameOver => render_game_over(f, app),
        Mode::Battle => render_battle(f, app),
        Mode::Help => render_help(f),
        Mode::Playing
        | Mode::Build
        | Mode::BuildDirection
        | Mode::Craft
        | Mode::Cronjob
        | Mode::CronjobStructure
        | Mode::InspectDirection
        | Mode::InspectDetail
        | Mode::Inventory
        | Mode::InventoryItemAction
        | Mode::Companion => render_playing(f, app),
    }
}

fn render_playing(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let chunks = Layout::vertical([Constraint::Min(10), Constraint::Length(8)]).split(area);
    let top = Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)]).split(chunks[0]);

    let mode = app.mode;
    let zoom = app.zoom.clamp(1, 8);
    let status_line = app.status_line.clone();
    let Some(game) = &mut app.game else { return };

    let viewport_area = top[0];
    let half_w = ((viewport_area.width.saturating_sub(2) as i32) / (2 * zoom as i32)).max(1);
    let half_h = ((viewport_area.height.saturating_sub(2) as i32) / (2 * zoom as i32)).max(1);

    let status = game.player_status();
    let tiles = game.view_tiles(half_w, half_h);
    // Tamed programs are compiled off the field once captured — they no
    // longer occupy a map tile (the terrain underneath shows through), even
    // though the entity itself still exists for cronjob assignment (`w`).
    let entities: Vec<_> = game
        .view_entities(half_w, half_h)
        .into_iter()
        .filter(|e| !e.is_tamed)
        .collect();

    f.render_widget(
        build_map_paragraph(&tiles, &entities, status.position, half_w, half_h, zoom),
        viewport_area,
    );
    render_status_panel(f, top[1], &status);

    let mut log_lines: Vec<Line> = Vec::new();
    if let Some(s) = &status_line {
        log_lines.push(Line::styled(s.clone(), Style::new().fg(Color::Red)));
    }
    let log_capacity = chunks[1].height.saturating_sub(2) as usize;
    log_lines.extend(game.message_log(log_capacity.max(1)).into_iter().map(Line::from));
    f.render_widget(
        Paragraph::new(log_lines)
            .block(Block::bordered().title("Feed"))
            .wrap(Wrap { trim: true }),
        chunks[1],
    );

    match mode {
        Mode::Build => render_build_menu(f, area, game),
        Mode::BuildDirection => render_build_direction(f, area),
        Mode::Craft => render_craft_menu(f, area, game),
        Mode::Cronjob => render_cronjob_menu(f, area, game),
        Mode::CronjobStructure => render_cronjob_structure_menu(f, area, game),
        Mode::InspectDirection => render_inspect_direction(f, area),
        Mode::InspectDetail => render_inspect_detail(f, area, game, app.pending_inspect),
        Mode::Inventory => render_inventory_screen(f, area, game),
        Mode::InventoryItemAction => render_inventory_item_action(f, area, app.pending_inventory_item),
        Mode::Companion => render_companion_menu(f, area, game),
        _ => {}
    }
}

fn build_map_paragraph<'a>(
    tiles: &[Vec<Tile>],
    entities: &[EntityView],
    center: (i32, i32),
    half_w: i32,
    half_h: i32,
    zoom: u16,
) -> Paragraph<'a> {
    // Third element: bold, set for structures so they visually pop out from
    // terrain and creatures on the map even when a glyph or color happens
    // to be shared (the game's limited 10-color palette can't guarantee
    // every structure a color no species also uses).
    let mut grid: Vec<Vec<(char, Color, bool)>> = tiles
        .iter()
        .map(|row| row.iter().map(|t| { let (ch, color) = tile_style(t.biome); (ch, color, false) }).collect())
        .collect();

    for ev in entities {
        let rx = ev.pos.0 - center.0 + half_w;
        let ry = ev.pos.1 - center.1 + half_h;
        if rx < 0 || ry < 0 {
            continue;
        }
        if let Some(row) = grid.get_mut(ry as usize) {
            if let Some(cell) = row.get_mut(rx as usize) {
                *cell = (ev.glyph, glyph_color(ev.color), ev.is_structure);
            }
        }
    }

    // Each world tile becomes a `zoom x zoom` block of identical characters,
    // so a higher zoom shows fewer tiles but each one reads more clearly.
    let zoom = zoom.max(1) as usize;
    let lines: Vec<Line<'a>> = grid
        .into_iter()
        .flat_map(|row| {
            let spans: Vec<Span<'a>> = row
                .into_iter()
                .flat_map(|(ch, color, bold)| {
                    let mut style = Style::new().fg(color);
                    if bold {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    std::iter::repeat(Span::styled(ch.to_string(), style)).take(zoom)
                })
                .collect();
            std::iter::repeat(Line::from(spans)).take(zoom)
        })
        .collect();

    Paragraph::new(lines).block(Block::bordered().title(format!("Grid ({zoom}x)")))
}

fn tile_style(biome: Biome) -> (char, Color) {
    match biome {
        Biome::DataVoid => ('~', Color::Blue),
        Biome::BlackIce => ('^', Color::Red),
        Biome::Mainframe => ('#', Color::Cyan),
        Biome::OpenGrid => ('.', Color::Green),
        Biome::NullSector => (':', Color::Gray),
        Biome::StaticField => ('%', Color::White),
    }
}

fn glyph_color(c: GlyphColor) -> Color {
    match c {
        GlyphColor::White => Color::White,
        GlyphColor::Gray => Color::Gray,
        GlyphColor::Green => Color::Green,
        GlyphColor::DarkGreen => Color::Rgb(0, 100, 0),
        GlyphColor::Red => Color::Red,
        GlyphColor::Yellow => Color::Yellow,
        GlyphColor::Blue => Color::Blue,
        GlyphColor::Magenta => Color::Magenta,
        GlyphColor::Cyan => Color::Cyan,
        GlyphColor::Brown => Color::Rgb(139, 69, 19),
    }
}

fn render_status_panel(f: &mut Frame, area: Rect, status: &PlayerStatus) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(0),
    ])
    .split(area);

    let hp_ratio = (status.hp as f64 / status.max_hp.max(1) as f64).clamp(0.0, 1.0);
    f.render_widget(
        Gauge::default()
            .block(Block::bordered().title("Integrity"))
            .gauge_style(Style::new().fg(Color::Red))
            .ratio(hp_ratio)
            .label(format!("{}/{}", status.hp, status.max_hp)),
        chunks[0],
    );

    let power_ratio = (status.hunger as f64 / 100.0).clamp(0.0, 1.0);
    f.render_widget(
        Gauge::default()
            .block(Block::bordered().title("Power"))
            .gauge_style(Style::new().fg(Color::Yellow))
            .ratio(power_ratio)
            .label(format!("{:.0}/100", status.hunger)),
        chunks[1],
    );

    let fatigue_ratio = (status.fatigue as f64 / 100.0).clamp(0.0, 1.0);
    f.render_widget(
        Gauge::default()
            .block(Block::bordered().title("Fatigue"))
            .gauge_style(Style::new().fg(Color::Blue))
            .ratio(fatigue_ratio)
            .label(format!("{:.0}/100", status.fatigue)),
        chunks[2],
    );

    let mut lines = vec![
        Line::styled(
            format!("Level {}  (XP {}/{})", status.level, status.xp, status.xp_to_next),
            Style::new().fg(Color::Cyan),
        ),
        Line::from(format!(
            "Position: ({}, {})",
            status.position.0, status.position.1
        )),
        Line::from(format!("Attack {}   Defense {}", status.atk, status.def)),
        Line::from(format!("Decompiler {}", status.decompiler)),
    ];
    if let Some(companion) = &status.companion {
        lines.push(Line::from(format!(
            "Companion: {} (HP {}/{})",
            companion.name, companion.hp, companion.max_hp
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Inventory:"));
    if status.inventory.is_empty() {
        lines.push(Line::from("  (empty)"));
    }
    for (item, qty) in &status.inventory {
        lines.push(Line::from(format!("  {} x{}", item.display_name(), qty)));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("hjkl/arrows move  . wait   e drain  r recharge"));
    lines.push(Line::from("g scan    c compile"));
    lines.push(Line::from("b deploy  w assign cronjob"));
    lines.push(Line::from("i inspect (pick a direction)"));
    lines.push(Line::from("v inventory/equipment"));
    lines.push(Line::from("p companion"));
    lines.push(Line::from("s save    q main menu   ? help"));
    lines.push(Line::from("+/- zoom"));
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("System")),
        chunks[3],
    );
}

fn render_craft_menu(f: &mut Frame, area: Rect, game: &mut Game) {
    let popup = centered_rect(60, 40, area);
    f.render_widget(Clear, popup);
    let recipes = game.craft_recipes();
    let mut lines = vec![Line::from("Compile what? (Esc to cancel)"), Line::from("")];
    for (i, recipe) in recipes.iter().enumerate() {
        let cost: Vec<String> = recipe
            .cost
            .iter()
            .map(|(item, qty)| format!("{} {}", qty, item.display_name()))
            .collect();
        lines.push(Line::from(format!(
            "[{}] {} ({})",
            i + 1,
            recipe.result.display_name(),
            cost.join(", ")
        )));
    }
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }).block(Block::bordered().title("Compile")),
        popup,
    );
}

fn render_build_menu(f: &mut Frame, area: Rect, game: &mut Game) {
    let popup = centered_rect(70, 60, area);
    f.render_widget(Clear, popup);
    let defs = game.structure_defs();
    let mut lines = vec![Line::from("Deploy what? (Esc to cancel)"), Line::from("")];
    for (i, def) in defs.iter().enumerate() {
        let cost: Vec<String> = def
            .build_cost
            .iter()
            .map(|(item, qty)| format!("{} {}", qty, item.display_name()))
            .collect();
        lines.push(Line::styled(
            format!("[{}] {} ({})", i + 1, def.name, cost.join(", ")),
            Style::new().add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(format!("    {}", structure_description(def))));
    }
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }).block(Block::bordered().title("Deploy")),
        popup,
    );
}

/// A one-sentence summary of what a structure does, derived from its
/// `work`/`passive_process` recipe rather than a separate authored field —
/// any structure a modder drops in automatically gets a sensible line here.
fn structure_description(def: &feral_processes_engine::structures::StructureDef) -> String {
    let mut parts = Vec::new();
    if let Some(work) = &def.work {
        parts.push(format!("cronjob → {}", work.produces.display_name()));
    }
    if let Some(passive) = &def.passive_process {
        parts.push(format!(
            "{} → {}",
            passive.consumes.display_name(),
            passive.produces.display_name()
        ));
    }
    if parts.is_empty() {
        parts.push("no production".to_string());
    }
    parts.join(", ")
}

fn render_build_direction(f: &mut Frame, area: Rect) {
    let popup = centered_rect(50, 20, area);
    f.render_widget(Clear, popup);
    let lines = vec![Line::from(
        "Choose a direction to deploy (arrows/hjkl), Esc to cancel",
    )];
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Deploy Direction")),
        popup,
    );
}

fn render_cronjob_menu(f: &mut Frame, area: Rect, game: &mut Game) {
    let popup = centered_rect(60, 50, area);
    f.render_widget(Clear, popup);
    let workers: Vec<_> = game
        .view_entities(crate::MENU_SCAN_RADIUS, crate::MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.is_tamed)
        .collect();
    let mut lines = vec![Line::from("Assign which program to a cronjob? (Esc to cancel)")];
    if workers.is_empty() {
        lines.push(Line::from("(no compiled programs nearby)"));
    }
    for (i, w) in workers.iter().enumerate() {
        lines.push(Line::from(format!(
            "[{}] {}{} at ({}, {})",
            i + 1,
            w.label,
            w.level.map(|l| format!(" Lv{l}")).unwrap_or_default(),
            w.pos.0,
            w.pos.1
        )));
    }
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Assign Cronjob")),
        popup,
    );
}

fn render_cronjob_structure_menu(f: &mut Frame, area: Rect, game: &mut Game) {
    let popup = centered_rect(60, 50, area);
    f.render_widget(Clear, popup);
    let structures: Vec<_> = game
        .view_entities(crate::MENU_SCAN_RADIUS, crate::MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.can_work)
        .collect();
    let mut lines = vec![Line::from("Cronjob which structure? (Esc to cancel)")];
    if structures.is_empty() {
        lines.push(Line::from("(no workable structures nearby)"));
    }
    for (i, s) in structures.iter().enumerate() {
        lines.push(Line::from(format!(
            "[{}] {} at ({}, {})",
            i + 1,
            s.label,
            s.pos.0,
            s.pos.1
        )));
    }
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Assign Cronjob")),
        popup,
    );
}

fn render_companion_menu(f: &mut Frame, area: Rect, game: &mut Game) {
    let popup = centered_rect(60, 50, area);
    f.render_widget(Clear, popup);
    let candidates: Vec<_> = game
        .view_entities(crate::MENU_SCAN_RADIUS, crate::MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.is_tamed)
        .collect();
    let mut lines = vec![Line::from(
        "Pick a companion to fight beside you — select its own number again to stand it down. (Esc to cancel)",
    )];
    if candidates.is_empty() {
        lines.push(Line::from("(no compiled programs nearby)"));
    }
    for (i, c) in candidates.iter().enumerate() {
        let active = if c.is_companion { " (active companion)" } else { "" };
        lines.push(Line::from(format!(
            "[{}] {}{}{}",
            i + 1,
            c.label,
            c.level.map(|l| format!(" Lv{l}")).unwrap_or_default(),
            active
        )));
    }
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }).block(Block::bordered().title("Companion")),
        popup,
    );
}

fn render_inspect_direction(f: &mut Frame, area: Rect) {
    let popup = centered_rect(50, 20, area);
    f.render_widget(Clear, popup);
    let lines = vec![Line::from(
        "Choose a direction to inspect (arrows/hjkl), Esc to cancel",
    )];
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Inspect Direction")),
        popup,
    );
}

fn render_inspect_detail(f: &mut Frame, area: Rect, game: &mut Game, entity: Option<feral_processes_engine::Entity>) {
    let popup = centered_rect(60, 60, area);
    f.render_widget(Clear, popup);
    let Some(view) = entity.and_then(|e| game.inspect(e)) else {
        f.render_widget(
            Paragraph::new(Line::from("That program is gone. Press any key to go back."))
                .block(Block::bordered().title("Inspect")),
            popup,
        );
        return;
    };

    let status = if view.is_tamed {
        "compiled (yours)".to_string()
    } else if view.is_hostile {
        "rogue".to_string()
    } else {
        "idle".to_string()
    };
    let habitats: Vec<String> = view.habitats.iter().map(|b| format!("{b:?}")).collect();
    let moves: Vec<String> = view
        .moves
        .iter()
        .map(|m| format!("{} (pow {})", m.name, m.power))
        .collect();

    let mut lines = vec![
        Line::styled(
            format!("{}{}", view.name, view.level.map(|l| format!(" — Lv{l}")).unwrap_or_default()),
            Style::new().add_modifier(Modifier::BOLD),
        ),
        Line::from(format!("Status: {status}")),
        Line::from(format!("Integrity: {}/{}", view.hp, view.max_hp)),
        Line::from(format!("Attack {}   Defense {}", view.atk, view.def)),
        Line::from(format!("Decompile difficulty: {:.0}%", view.taming_difficulty * 100.0)),
    ];
    if view.is_hostile && !view.is_tamed {
        lines.push(Line::styled(
            format!("Decompile chance right now: {:.0}%", view.decompile_chance * 100.0),
            Style::new().fg(Color::Magenta),
        ));
    }
    lines.push(Line::from(format!(
        "Habitats: {}",
        if habitats.is_empty() { "unknown".to_string() } else { habitats.join(", ") }
    )));
    lines.push(Line::from(format!(
        "Moves: {}",
        if moves.is_empty() { "none".to_string() } else { moves.join(", ") }
    )));
    if let Some(res) = view.work_resource {
        lines.push(Line::from(format!("Work aptitude: {}", res.display_name())));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Press any key to go back, Esc to close"));

    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Inspect")),
        popup,
    );
}

fn render_inventory_screen(f: &mut Frame, area: Rect, game: &mut Game) {
    let popup = centered_rect(70, 70, area);
    f.render_widget(Clear, popup);
    let status = game.player_status();

    let mut lines = vec![
        Line::styled(
            format!(
                "Level {}   Attack {}   Defense {}   Decompiler {}",
                status.level, status.atk, status.def, status.decompiler
            ),
            Style::new().fg(Color::Cyan),
        ),
        Line::from(""),
        Line::styled("Equipped (number to unequip):", Style::new().add_modifier(Modifier::BOLD)),
        equipped_line(1, "Weapon", status.weapon),
        equipped_line(2, "Armor", status.armor),
        equipped_line(3, "Module", status.module),
        Line::from(""),
        Line::styled("Inventory (number to equip/drop/destroy):", Style::new().add_modifier(Modifier::BOLD)),
    ];
    if status.inventory.is_empty() {
        lines.push(Line::from("  (empty)"));
    }
    for (i, (item, qty)) in status.inventory.iter().enumerate() {
        let tag = if item.equipment().is_some() { " (equippable)" } else { "" };
        lines.push(Line::from(format!("[{}] {} x{}{}", i + 4, item.display_name(), qty, tag)));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Esc to close"));

    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Inventory")),
        popup,
    );
}

fn equipped_line(num: usize, label: &str, item: Option<ItemId>) -> Line<'static> {
    match item.and_then(|i| i.equipment().map(|(_, mods)| (i, mods))) {
        Some((item, mods)) => {
            let mut parts = Vec::new();
            if mods.atk != 0 {
                parts.push(format!("+{} ATK", mods.atk));
            }
            if mods.def != 0 {
                parts.push(format!("+{} DEF", mods.def));
            }
            if mods.decompiler != 0 {
                parts.push(format!("+{} DECOMP", mods.decompiler));
            }
            Line::from(format!("[{num}] {label}: {} ({})", item.display_name(), parts.join(" ")))
        }
        None => Line::from(format!("[{num}] {label}: (empty)")),
    }
}

fn render_inventory_item_action(f: &mut Frame, area: Rect, item: Option<ItemId>) {
    let popup = centered_rect(50, 30, area);
    f.render_widget(Clear, popup);
    let Some(item) = item else {
        f.render_widget(
            Paragraph::new(Line::from("Nothing selected.")).block(Block::bordered().title("Item")),
            popup,
        );
        return;
    };
    let mut actions = vec!["[D]rop".to_string(), "[X] Destroy".to_string()];
    if item.equipment().is_some() {
        actions.insert(0, "[E]quip".to_string());
    }
    let lines = vec![
        Line::styled(item.display_name(), Style::new().add_modifier(Modifier::BOLD)),
        Line::from(""),
        Line::from(actions.join("   ")),
        Line::from(""),
        Line::from("Esc to cancel"),
    ];
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Item")),
        popup,
    );
}

fn render_battle(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let Some(game) = &mut app.game else { return };
    let Some(view) = game.battle_view() else { return };

    let mut constraints = vec![Constraint::Length(3), Constraint::Length(3)];
    if view.companion.is_some() {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(1));
    constraints.push(Constraint::Min(5));
    constraints.push(Constraint::Length(3));
    let chunks = Layout::vertical(constraints).split(area);
    let mut i = 0;

    let wild_ratio = (view.wild_hp as f64 / view.wild_max_hp.max(1) as f64).clamp(0.0, 1.0);
    f.render_widget(
        Gauge::default()
            .block(Block::bordered().title(format!(
                "{} (ATK {} / DEF {})",
                view.wild_name, view.wild_atk, view.wild_def
            )))
            .gauge_style(Style::new().fg(Color::Red))
            .ratio(wild_ratio)
            .label(format!("{}/{}", view.wild_hp, view.wild_max_hp)),
        chunks[i],
    );
    i += 1;

    let player_ratio = (view.player_hp as f64 / view.player_max_hp.max(1) as f64).clamp(0.0, 1.0);
    f.render_widget(
        Gauge::default()
            .block(Block::bordered().title(format!(
                "You (ATK {} / DEF {} / DECOMP {})",
                view.player_atk, view.player_def, view.player_decompiler
            )))
            .gauge_style(Style::new().fg(Color::Cyan))
            .ratio(player_ratio)
            .label(format!("{}/{}", view.player_hp, view.player_max_hp)),
        chunks[i],
    );
    i += 1;

    if let Some(companion) = &view.companion {
        f.render_widget(
            Paragraph::new(Line::styled(
                format!(
                    "Companion: {} (HP {}/{}, ATK {})",
                    companion.name, companion.hp, companion.max_hp, companion.atk
                ),
                Style::new().fg(Color::Green),
            )),
            chunks[i],
        );
        i += 1;
    }

    f.render_widget(
        Paragraph::new(Line::styled(
            format!(
                "Decompile chance right now: {:.0}%{}",
                view.decompile_chance * 100.0,
                if view.can_tame { "" } else { " (needs an ICE Breaker)" }
            ),
            Style::new().fg(Color::Magenta),
        )),
        chunks[i],
    );
    i += 1;

    let log_capacity = chunks[i].height.saturating_sub(2) as usize;
    let log_lines: Vec<Line> = game
        .message_log(log_capacity.max(1))
        .into_iter()
        .map(Line::from)
        .collect();
    f.render_widget(
        Paragraph::new(log_lines)
            .block(Block::bordered().title("Intrusion"))
            .wrap(Wrap { trim: true }),
        chunks[i],
    );
    i += 1;

    let mut actions = vec!["[A]ttack".to_string()];
    if view.can_tame {
        actions.push("[D]ecompile".to_string());
    }
    if view.companion.is_some() {
        actions.push("[C]ommand companion".to_string());
    }
    actions.push("[J]ack Out".to_string());
    f.render_widget(
        Paragraph::new(Line::from(actions.join("   "))).block(Block::bordered()),
        chunks[i],
    );
}

fn render_main_menu(f: &mut Frame, app: &App) {
    let area = f.area();
    let mut lines = vec![
        Line::styled("feral-processes", Style::new().add_modifier(Modifier::BOLD)),
        Line::styled(
            "// jack into the Grid",
            Style::new().fg(Color::Cyan).add_modifier(Modifier::ITALIC),
        ),
        Line::from(""),
        Line::from("[N] New Game"),
    ];
    if app.save_exists() {
        lines.push(Line::from("[L] Load Game"));
    }
    lines.push(Line::from("[Q] Quit"));
    if let Some(s) = &app.status_line {
        lines.push(Line::from(""));
        lines.push(Line::styled(s.clone(), Style::new().fg(Color::Red)));
    }
    let popup = centered_rect(40, 40, area);
    f.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(Block::bordered().title("Main Menu")),
        popup,
    );
}

fn render_help(f: &mut Frame) {
    let area = f.area();
    let lines = vec![
        Line::styled("Controls", Style::new().add_modifier(Modifier::BOLD)),
        Line::from(""),
        Line::from("hjkl / arrow keys   move (bumping a rogue program starts an intrusion)"),
        Line::from(".                   wait in place (advances one tick)"),
        Line::from("e                   drain a power cell"),
        Line::from("r                   recharge overnight (restores fatigue and Integrity, uses power)"),
        Line::from("g                   scan the sector for power cells"),
        Line::from("c                   open the compile menu (create an ICE Breaker and any future recipes)"),
        Line::from("b                   deploy a structure"),
        Line::from("w                   assign a compiled program to a cronjob"),
        Line::from("i                   pick a direction, inspect the first program that way (stats/moves, no intrusion)"),
        Line::from("v                   inventory/equipment: equip, unequip, drop, destroy items"),
        Line::from("p                   pick a nearby compiled program as your active companion"),
        Line::from("s                   save session"),
        Line::from("q                   return to the main menu (unsaved progress is lost — save first)"),
        Line::from("+ / -               zoom the grid in / out"),
        Line::from(""),
        Line::from("In an intrusion:  a attack   d decompile (needs an ICE Breaker)"),
        Line::from("                  c command companion (if one is active)   j jack out"),
        Line::from(""),
        Line::from("Defeating or decompiling a rogue program grants XP. Compiled programs"),
        Line::from("gain XP from completed work cycles. Leveling up fully restores Integrity."),
        Line::from(""),
        Line::from("Equipping a weapon/armor/module grants a flat Attack/Defense/Decompiler"),
        Line::from("bonus while worn. Equip up to one item per slot; equipping a second"),
        Line::from("item in an occupied slot swaps the old one back to your inventory."),
        Line::from(""),
        Line::from("A companion (p) fights alongside you: commanding it (c) in an intrusion"),
        Line::from("has it attack using its own Attack stat instead of you acting that round."),
        Line::from("It's never targeted by the wild program's retaliation. Assigning it to a"),
        Line::from("cronjob (w) stands it down as companion, and vice versa — one job at a time."),
        Line::from(""),
        Line::from("Press any key to close"),
    ];
    let popup = centered_rect(72, 70, area);
    f.render_widget(Clear, popup);
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Help")),
        popup,
    );
}

fn render_difficulty_pick(f: &mut Frame) {
    let area = f.area();
    let lines = vec![
        Line::from("Choose difficulty"),
        Line::from(""),
        Line::from("[P] Permadeath - flatlining is final; the session is archived to a log"),
        Line::from("[F] Forgiving - flatlining costs you, but you reboot and keep going"),
        Line::from(""),
        Line::from("Esc to go back"),
    ];
    let popup = centered_rect(60, 40, area);
    f.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(Block::bordered().title("New Game")),
        popup,
    );
}

fn render_game_over(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let summary = app
        .game
        .as_mut()
        .and_then(|g| g.history_summary())
        .unwrap_or_else(|| "Connection lost.".to_string());
    let lines = vec![
        Line::styled(
            "FLATLINE",
            Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Line::from(""),
        Line::from(summary),
        Line::from(""),
        Line::from("Press any key to return to the main menu"),
    ];
    let popup = centered_rect(60, 40, area);
    f.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(Block::bordered().title("Session Terminated")),
        popup,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}
