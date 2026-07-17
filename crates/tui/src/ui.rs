use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Gauge, Paragraph, Wrap};
use ratatui::Frame;

use feral_processes_engine::components::GlyphColor;
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
        Mode::Playing | Mode::Build | Mode::BuildDirection | Mode::Work | Mode::WorkStructure => {
            render_playing(f, app)
        }
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
    let entities = game.view_entities(half_w, half_h);

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
        Mode::Work => render_work_menu(f, area, game),
        Mode::WorkStructure => render_work_structure_menu(f, area, game),
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
    let mut grid: Vec<Vec<(char, Color)>> = tiles
        .iter()
        .map(|row| row.iter().map(|t| tile_style(t.biome)).collect())
        .collect();

    for ev in entities {
        let rx = ev.pos.0 - center.0 + half_w;
        let ry = ev.pos.1 - center.1 + half_h;
        if rx < 0 || ry < 0 {
            continue;
        }
        if let Some(row) = grid.get_mut(ry as usize) {
            if let Some(cell) = row.get_mut(rx as usize) {
                *cell = (ev.glyph, glyph_color(ev.color));
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
                .flat_map(|(ch, color)| {
                    std::iter::repeat(Span::styled(ch.to_string(), Style::new().fg(color))).take(zoom)
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
    ];
    lines.push(Line::from(""));
    lines.push(Line::from("Inventory:"));
    if status.inventory.is_empty() {
        lines.push(Line::from("  (empty)"));
    }
    for (item, qty) in &status.inventory {
        lines.push(Line::from(format!("  {} x{}", item.display_name(), qty)));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("hjkl/arrows move  e drain  r recharge"));
    lines.push(Line::from("g scan    c compile orb"));
    lines.push(Line::from("b deploy  w assign subroutine"));
    lines.push(Line::from("s save    q quit   ? help"));
    lines.push(Line::from("+/- zoom"));
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("System")),
        chunks[3],
    );
}

fn render_build_menu(f: &mut Frame, area: Rect, game: &mut Game) {
    let popup = centered_rect(60, 50, area);
    f.render_widget(Clear, popup);
    let defs = game.structure_defs();
    let mut lines = vec![Line::from("Deploy what? (Esc to cancel)")];
    for (i, def) in defs.iter().enumerate() {
        let cost: Vec<String> = def
            .build_cost
            .iter()
            .map(|(item, qty)| format!("{} {}", qty, item.display_name()))
            .collect();
        lines.push(Line::from(format!("[{}] {} ({})", i + 1, def.name, cost.join(", "))));
    }
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Deploy")),
        popup,
    );
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

fn render_work_menu(f: &mut Frame, area: Rect, game: &mut Game) {
    let popup = centered_rect(60, 50, area);
    f.render_widget(Clear, popup);
    let workers: Vec<_> = game
        .view_entities(crate::MENU_SCAN_RADIUS, crate::MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.is_tamed)
        .collect();
    let mut lines = vec![Line::from("Assign which program to work? (Esc to cancel)")];
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
        Paragraph::new(lines).block(Block::bordered().title("Assign Subroutine")),
        popup,
    );
}

fn render_work_structure_menu(f: &mut Frame, area: Rect, game: &mut Game) {
    let popup = centered_rect(60, 50, area);
    f.render_widget(Clear, popup);
    let structures: Vec<_> = game
        .view_entities(crate::MENU_SCAN_RADIUS, crate::MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.can_work)
        .collect();
    let mut lines = vec![Line::from("Work which structure? (Esc to cancel)")];
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
        Paragraph::new(lines).block(Block::bordered().title("Assign Subroutine")),
        popup,
    );
}

fn render_battle(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let Some(game) = &mut app.game else { return };
    let Some(view) = game.battle_view() else { return };

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(5),
        Constraint::Length(3),
    ])
    .split(area);

    let wild_ratio = (view.wild_hp as f64 / view.wild_max_hp.max(1) as f64).clamp(0.0, 1.0);
    f.render_widget(
        Gauge::default()
            .block(Block::bordered().title(view.wild_name.clone()))
            .gauge_style(Style::new().fg(Color::Red))
            .ratio(wild_ratio)
            .label(format!("{}/{}", view.wild_hp, view.wild_max_hp)),
        chunks[0],
    );

    let player_ratio = (view.player_hp as f64 / view.player_max_hp.max(1) as f64).clamp(0.0, 1.0);
    f.render_widget(
        Gauge::default()
            .block(Block::bordered().title("You"))
            .gauge_style(Style::new().fg(Color::Cyan))
            .ratio(player_ratio)
            .label(format!("{}/{}", view.player_hp, view.player_max_hp)),
        chunks[1],
    );

    let log_capacity = chunks[2].height.saturating_sub(2) as usize;
    let log_lines: Vec<Line> = game
        .message_log(log_capacity.max(1))
        .into_iter()
        .map(Line::from)
        .collect();
    f.render_widget(
        Paragraph::new(log_lines)
            .block(Block::bordered().title("Intrusion"))
            .wrap(Wrap { trim: true }),
        chunks[2],
    );

    let mut actions = vec!["[A]ttack".to_string()];
    if view.can_tame {
        actions.push("[D]ecrypt".to_string());
    }
    actions.push("[J]ack Out".to_string());
    f.render_widget(
        Paragraph::new(Line::from(actions.join("   "))).block(Block::bordered()),
        chunks[3],
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
        Line::from("e                   drain a power cell"),
        Line::from("r                   recharge overnight (restores fatigue, uses power)"),
        Line::from("g                   scan the sector for power cells"),
        Line::from("c                   compile an ICE Breaker (3 Core Fragments)"),
        Line::from("b                   deploy a structure"),
        Line::from("w                   assign a compiled program to work a structure"),
        Line::from("s                   save session"),
        Line::from("q                   quit"),
        Line::from("+ / -               zoom the grid in / out"),
        Line::from(""),
        Line::from("In an intrusion:  a attack   d decrypt (needs an ICE Breaker)   j jack out"),
        Line::from(""),
        Line::from("Defeating or decrypting a rogue program grants XP. Compiled programs"),
        Line::from("gain XP from completed work cycles. Leveling up fully restores Integrity."),
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
