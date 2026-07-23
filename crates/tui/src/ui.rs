use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Gauge, Paragraph, Wrap};

use feral_processes_app_core::{
    App, MENU_SCAN_RADIUS, Mode, TradeChoice, inventory_item_actions, menu_shortcut,
};
use feral_processes_engine::components::{EquippedItem, GlyphColor};
use feral_processes_engine::items::ItemId;
use feral_processes_engine::world::{Biome, Tile};
use feral_processes_engine::{
    EntityView, Game, MAX_FUSIONS, MessageKind, PetInfo, PlayerStatus, ResearchState,
};

/// Display styling for a message-log line, chosen by the engine-supplied
/// `MessageKind` rather than by sniffing the text — low-priority chatter
/// stays dim, gains/damage that matter get a color and (for level-ups) bold.
fn message_style(kind: MessageKind) -> Style {
    match kind {
        MessageKind::Info => Style::new().fg(Color::Gray),
        MessageKind::Loot => Style::new().fg(Color::Green),
        MessageKind::LevelUp => Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
        MessageKind::Raid => Style::new().fg(Color::Rgb(255, 140, 0)),
    }
}

fn message_line(kind: MessageKind, text: String) -> Line<'static> {
    Line::styled(text, message_style(kind))
}

pub fn render(f: &mut Frame, app: &mut App) {
    match app.mode {
        Mode::MainMenu => render_main_menu(f, app),
        Mode::LoadGame => render_load_game_menu(f, app),
        Mode::SaveAction => render_save_action_menu(f, app),
        Mode::DifficultyPick => render_difficulty_pick(f, app.menu_selected),
        Mode::GameOver => render_game_over(f, app),
        Mode::Battle => render_battle(f, app),
        Mode::BattleCompanion => {
            render_battle(f, app);
            render_battle_companion_menu(f, app);
        }
        Mode::Help => render_help(f),
        Mode::Playing
        | Mode::Build
        | Mode::BuildDirection
        | Mode::Craft
        | Mode::CraftQuantity
        | Mode::Cronjob
        | Mode::CronjobStructure
        | Mode::Guard
        | Mode::GuardStructure
        | Mode::Remove
        | Mode::RemoveConfirm
        | Mode::Symlink
        | Mode::InspectDirection
        | Mode::InspectDetail
        | Mode::Inventory
        | Mode::InventoryItemAction
        | Mode::EraseQuantity
        | Mode::Companion
        | Mode::Fuse
        | Mode::FuseSecond
        | Mode::FuseName
        | Mode::Trade
        | Mode::TradeAction
        | Mode::TradeQuantity
        | Mode::Perks
        | Mode::Research => render_playing(f, app),
    }
}

fn render_playing(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let chunks = Layout::vertical([Constraint::Min(10), Constraint::Length(8)]).split(area);
    let top = Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(chunks[0]);

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
    let spawn_point = game.zone_spawn_point();

    f.render_widget(
        build_map_paragraph(
            &tiles,
            &entities,
            status.position,
            spawn_point,
            half_w,
            half_h,
            zoom,
        ),
        viewport_area,
    );
    render_status_panel(f, top[1], &status, game);

    let mut log_lines: Vec<Line> = Vec::new();
    if let Some(s) = &status_line {
        log_lines.push(Line::styled(s.clone(), Style::new().fg(Color::Red)));
    }
    let log_capacity = chunks[1].height.saturating_sub(2) as usize;
    log_lines.extend(
        game.message_log(log_capacity.max(1))
            .into_iter()
            .map(|(kind, text)| message_line(kind, text)),
    );
    f.render_widget(
        Paragraph::new(log_lines)
            .block(Block::bordered().title("Feed"))
            .wrap(Wrap { trim: true }),
        chunks[1],
    );

    let selected = app.menu_selected;
    match mode {
        Mode::Build => render_build_menu(f, area, game, selected),
        Mode::BuildDirection => render_build_direction(f, area),
        Mode::Craft => render_craft_menu(f, area, game, selected),
        Mode::CraftQuantity => render_craft_quantity_menu(
            f,
            area,
            game,
            app.pending_craft.clone(),
            &app.craft_quantity_input,
        ),
        Mode::EraseQuantity => render_erase_quantity_menu(
            f,
            area,
            game,
            app.pending_erase.clone(),
            &app.erase_quantity_input,
        ),
        Mode::Cronjob => render_cronjob_menu(f, area, game, selected),
        Mode::CronjobStructure => render_cronjob_structure_menu(f, area, game, selected),
        Mode::Guard => render_guard_menu(f, area, game, selected),
        Mode::GuardStructure => render_guard_structure_menu(f, area, game, selected),
        Mode::Remove => render_remove_menu(f, area, game, selected),
        Mode::RemoveConfirm => render_remove_confirm(f, area, selected),
        Mode::Symlink => render_symlink_menu(f, area, game, selected),
        Mode::InspectDirection => render_inspect_direction(f, area),
        Mode::InspectDetail => render_inspect_detail(f, area, game, app.pending_inspect),
        Mode::Inventory => render_inventory_screen(f, area, game, selected),
        Mode::InventoryItemAction => {
            let zone = game.player_status().zone;
            let fusion_tier = app
                .pending_inventory_item
                .clone()
                .map(|item| game.item_fusion_tier(item))
                .unwrap_or(0);
            render_inventory_item_action(
                f,
                area,
                app.pending_inventory_item.clone(),
                zone,
                fusion_tier,
                selected,
                game,
            )
        }
        Mode::Companion => render_companion_menu(f, area, game, selected),
        Mode::Fuse => render_fuse_menu(f, area, game, selected),
        Mode::FuseSecond => {
            render_fuse_second_menu(f, area, game, app.pending_fuse_first, selected)
        }
        Mode::FuseName => render_fuse_name_menu(
            f,
            area,
            game,
            app.pending_fuse_first,
            app.pending_fuse_second,
            &app.fuse_name_input,
        ),
        Mode::Trade => render_trade_menu(f, area, game, selected),
        Mode::TradeAction => {
            render_trade_action_menu(f, area, game, app.pending_trade_structure, selected)
        }
        Mode::TradeQuantity => render_trade_quantity_menu(
            f,
            area,
            game,
            app.pending_trade_structure,
            app.pending_trade_choice.clone(),
            &app.trade_quantity_input,
        ),
        Mode::Perks => render_perks_menu(f, area, game, selected),
        Mode::Research => render_research_menu(f, area, game, selected),
        _ => {}
    }
}

/// One row of a numbered/lettered menu, indented and reverse-styled when
/// `selected` — the Up/Down-arrow highlight (see `App::selected_index`),
/// layered on top of the row's own direct-key shortcut, which still works
/// no matter which row is highlighted.
fn menu_line(text: String, selected: bool) -> Line<'static> {
    if selected {
        Line::styled(
            format!("> {text}"),
            Style::new().add_modifier(Modifier::REVERSED),
        )
    } else {
        Line::from(format!("  {text}"))
    }
}

fn build_map_paragraph<'a>(
    tiles: &[Vec<Tile>],
    entities: &[EntityView],
    center: (i32, i32),
    spawn_point: (i32, i32),
    half_w: i32,
    half_h: i32,
    zoom: u16,
) -> Paragraph<'a> {
    // Third element: bold, set for structures and bosses so they visually
    // pop out from terrain and ordinary creatures on the map even when a
    // glyph or color happens to be shared (the game's limited 10-color
    // palette can't guarantee every structure/boss a color nothing else uses).
    let mut grid: Vec<Vec<(char, Color, bool)>> = tiles
        .iter()
        .map(|row| {
            row.iter()
                .map(|t| {
                    let (ch, color) = tile_style(t.biome);
                    (ch, color, false)
                })
                .collect()
        })
        .collect();

    // Marks where the player materialized on breaching into this zone (see
    // `Game::zone_spawn_point`) — drawn before entities so anything
    // standing on that tile still takes visual priority over the marker.
    let srx = spawn_point.0 - center.0 + half_w;
    let sry = spawn_point.1 - center.1 + half_h;
    if srx >= 0
        && sry >= 0
        && let Some(row) = grid.get_mut(sry as usize)
        && let Some(cell) = row.get_mut(srx as usize)
    {
        *cell = ('O', Color::LightMagenta, true);
    }

    for ev in entities {
        let rx = ev.pos.0 - center.0 + half_w;
        let ry = ev.pos.1 - center.1 + half_h;
        if rx < 0 || ry < 0 {
            continue;
        }
        if let Some(row) = grid.get_mut(ry as usize)
            && let Some(cell) = row.get_mut(rx as usize)
        {
            *cell = (
                ev.glyph,
                glyph_color(ev.color),
                ev.is_structure || ev.is_boss,
            );
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
                    std::iter::repeat_n(Span::styled(ch.to_string(), style), zoom)
                })
                .collect();
            std::iter::repeat_n(Line::from(spans), zoom)
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
        GlyphColor::Orange => Color::Rgb(255, 140, 0),
    }
}

fn render_status_panel(f: &mut Frame, area: Rect, status: &PlayerStatus, game: &Game) {
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
            format!(
                "Level {}  (XP {}/{})   Perk Points: {}",
                status.level, status.xp, status.xp_to_next, status.perk_points
            ),
            Style::new().fg(Color::Cyan),
        ),
        Line::styled(
            format!("Zone {}", status.zone),
            Style::new().fg(Color::Magenta),
        ),
        Line::from(format!(
            "Position: ({}, {})",
            status.position.0, status.position.1
        )),
        Line::from(format!(
            "Attack {}   Defense {}   Power {}",
            status.atk, status.def, status.power
        )),
        Line::from(format!("Decompiler {}", status.decompiler)),
        Line::styled(
            format!(
                "Party: {}/{}",
                status.companions.len(),
                feral_processes_engine::resources::MAX_PARTY_SIZE
            ),
            Style::new().fg(Color::Green),
        ),
    ];
    for companion in &status.companions {
        lines.push(Line::from(format!(
            "Companion: {} (HP {}/{}, Power {})",
            companion.name, companion.hp, companion.max_hp, companion.power
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Inventory:"));
    if status.inventory.is_empty() {
        lines.push(Line::from("  (empty)"));
    }
    for (item, qty) in &status.inventory {
        lines.push(Line::from(format!("  {} x{}", game.item_name(item), qty)));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("hjkl/arrows move  . wait   e drain  r recharge"));
    lines.push(Line::from("g scan    c compile"));
    lines.push(Line::from("b deploy  w assign cronjob  G assign guard"));
    lines.push(Line::from("R demolish structure"));
    lines.push(Line::from("u use symlink"));
    lines.push(Line::from("i inspect (pick a direction)"));
    lines.push(Line::from("v inventory/equipment"));
    lines.push(Line::from("p companion  f fuse  t trade  x perks"));
    lines.push(Line::from("s save    q main menu   ? help"));
    lines.push(Line::from("+/- zoom"));
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("System")),
        chunks[3],
    );
}

/// Formats a `(item, quantity)` cost list for display, with each entry
/// tagged `(have/need)` against the player's current inventory — e.g.
/// "Core Fragment (0/3)" if the player has none, "Core Fragment (10/3)" if
/// they have plenty. Shared by every dialog that spends items (compile,
/// deploy, symlink) so the player can see at a glance whether they can
/// afford it without leaving the menu.
fn cost_display(game: &Game, cost: &[(ItemId, u32)], inventory: &[(ItemId, u32)]) -> Vec<String> {
    cost.iter()
        .map(|(item, qty)| {
            let have = inventory
                .iter()
                .find(|(i, _)| i == item)
                .map(|(_, q)| *q)
                .unwrap_or(0);
            format!("{} ({have}/{qty})", game.item_name(item))
        })
        .collect()
}

fn render_craft_menu(f: &mut Frame, area: Rect, game: &mut Game, selected: usize) {
    let popup = centered_rect(60, 40, area);
    f.render_widget(Clear, popup);
    let status = game.player_status();
    let recipes = game.craft_recipes();
    let mut lines = vec![
        Line::from("Compile what? (Esc to cancel; Up/Down + Enter also work)"),
        Line::from(""),
    ];
    for (i, recipe) in recipes.iter().enumerate() {
        let cost = cost_display(game, &recipe.cost, &status.inventory);
        lines.push(menu_line(
            format!(
                "[{}] {} — {}",
                menu_shortcut(i),
                game.item_name(&recipe.result),
                cost.join(", ")
            ),
            i == selected,
        ));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Compile")),
        popup,
    );
}

fn render_craft_quantity_menu(
    f: &mut Frame,
    area: Rect,
    game: &mut Game,
    pending: Option<ItemId>,
    quantity_input: &str,
) {
    let popup = centered_rect(60, 30, area);
    f.render_widget(Clear, popup);
    let Some(result) = pending else { return };
    let status = game.player_status();
    let recipe = game
        .craft_recipes()
        .into_iter()
        .find(|r| r.result == result);
    let mut lines = vec![
        Line::from(format!("Compile how many {}?", game.item_name(&result))),
        Line::from(""),
    ];
    if let Some(recipe) = &recipe {
        let cost = cost_display(game, &recipe.cost, &status.inventory);
        lines.push(Line::from(format!("Cost per unit: {}", cost.join(", "))));
        lines.push(Line::from(""));
    }
    let shown = if quantity_input.is_empty() {
        "1"
    } else {
        quantity_input
    };
    lines.push(Line::from(format!("Quantity: {shown}")));
    lines.push(Line::from(""));
    lines.push(Line::from(format!(
        "Max affordable right now: {}",
        game.max_craftable(result)
    )));
    lines.push(Line::from(""));
    lines.push(Line::from("Type digits, Enter to compile"));
    lines.push(Line::from(
        "[F] Compile 5   [M] Compile max affordable   Esc to go back",
    ));
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Compile")),
        popup,
    );
}

fn render_erase_quantity_menu(
    f: &mut Frame,
    area: Rect,
    game: &mut Game,
    item: Option<ItemId>,
    quantity_input: &str,
) {
    let popup = centered_rect(60, 40, area);
    f.render_widget(Clear, popup);
    let Some(item) = item else { return };
    let status = game.player_status();
    let held = status
        .inventory
        .iter()
        .find(|(i, _)| *i == item)
        .map(|(_, q)| *q)
        .unwrap_or(0);
    let shown = if quantity_input.is_empty() {
        "1".to_string()
    } else {
        quantity_input.to_string()
    };
    let lines = vec![
        Line::from(format!("Erase how many {}?", game.item_name(&item))),
        Line::from(""),
        Line::from(format!("Quantity: {shown}")),
        Line::from(""),
        Line::from(format!(
            "You have: {held}        Buffer: {}/{}",
            status.inventory_used, status.inventory_capacity
        )),
        Line::from(""),
        Line::from("Type digits, Enter to erase"),
        Line::from("[A] Erase all   Esc to go back"),
    ];
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Erase")),
        popup,
    );
}

fn render_build_menu(f: &mut Frame, area: Rect, game: &mut Game, selected: usize) {
    let popup = centered_rect(70, 60, area);
    f.render_widget(Clear, popup);
    let status = game.player_status();
    let defs = game.buildable_structure_defs();
    let descriptions: Vec<String> = defs
        .iter()
        .map(|def| game.structure_description(def))
        .collect();
    let mut lines = vec![
        Line::from("Deploy what? (Esc to cancel; Up/Down + Enter also work)"),
        Line::from(""),
    ];
    for (i, def) in defs.iter().enumerate() {
        let raw_cost = game.structure_build_cost(def);
        let cost = cost_display(game, &raw_cost, &status.inventory);
        let text = format!("[{}] {} — {}", menu_shortcut(i), def.name, cost.join(", "));
        lines.push(if i == selected {
            menu_line(text, true)
        } else {
            Line::styled(text, Style::new().add_modifier(Modifier::BOLD))
        });
        lines.push(Line::from(format!("    {}", descriptions[i])));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Deploy")),
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

fn render_cronjob_menu(f: &mut Frame, area: Rect, game: &mut Game, selected: usize) {
    let popup = centered_rect(60, 50, area);
    f.render_widget(Clear, popup);
    let workers: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.is_tamed)
        .collect();
    // `view_entities` doesn't carry a raw power number, only a level and
    // an HP fraction — cross-reference `owned_pets` for it, same as the
    // fuse menu does.
    let pets = game.owned_pets();
    let mut lines = vec![Line::from(
        "Assign which program to a cronjob? (Esc to cancel; Up/Down + Enter also work)",
    )];
    if workers.is_empty() {
        lines.push(Line::from("(no compiled programs nearby)"));
    }
    for (i, w) in workers.iter().enumerate() {
        let companion = if w.is_companion { " (in party)" } else { "" };
        let job = w
            .job_structure
            .as_ref()
            .map(|s| format!(" (on a cronjob: {s})"))
            .unwrap_or_default();
        let power = pets
            .iter()
            .find(|p| p.entity == w.entity)
            .map(|p| format!(" PWR {}", p.power))
            .unwrap_or_default();
        lines.push(menu_line(
            format!(
                "[{}] {}{}{} at ({}, {}){}{}",
                menu_shortcut(i),
                w.label,
                w.level.map(|l| format!(" Lv{l}")).unwrap_or_default(),
                power,
                w.pos.0,
                w.pos.1,
                companion,
                job
            ),
            i == selected,
        ));
    }
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Assign Cronjob")),
        popup,
    );
}

fn render_cronjob_structure_menu(f: &mut Frame, area: Rect, game: &mut Game, selected: usize) {
    let popup = centered_rect(60, 50, area);
    f.render_widget(Clear, popup);
    let structures: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.can_work)
        .collect();
    let mut lines = vec![Line::from(
        "Cronjob which structure? (Esc to cancel; Up/Down + Enter also work)",
    )];
    if structures.is_empty() {
        lines.push(Line::from("(no workable structures nearby)"));
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
        lines.push(menu_line(
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
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Assign Cronjob")),
        popup,
    );
}

fn render_guard_menu(f: &mut Frame, area: Rect, game: &mut Game, selected: usize) {
    let popup = centered_rect(60, 50, area);
    f.render_widget(Clear, popup);
    let workers: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.is_tamed)
        .collect();
    let pets = game.owned_pets();
    let mut lines = vec![Line::from(
        "Assign which program to guard duty? (Esc to cancel; Up/Down + Enter also work)",
    )];
    if workers.is_empty() {
        lines.push(Line::from("(no compiled programs nearby)"));
    }
    for (i, w) in workers.iter().enumerate() {
        let companion = if w.is_companion { " (in party)" } else { "" };
        let job = w
            .job_structure
            .as_ref()
            .map(|s| format!(" (on a cronjob: {s})"))
            .unwrap_or_default();
        let power = pets
            .iter()
            .find(|p| p.entity == w.entity)
            .map(|p| format!(" PWR {}", p.power))
            .unwrap_or_default();
        lines.push(menu_line(
            format!(
                "[{}] {}{}{} at ({}, {}){}{}",
                menu_shortcut(i),
                w.label,
                w.level.map(|l| format!(" Lv{l}")).unwrap_or_default(),
                power,
                w.pos.0,
                w.pos.1,
                companion,
                job
            ),
            i == selected,
        ));
    }
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Assign Guard")),
        popup,
    );
}

fn render_guard_structure_menu(f: &mut Frame, area: Rect, game: &mut Game, selected: usize) {
    let popup = centered_rect(60, 50, area);
    f.render_widget(Clear, popup);
    let structures: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.is_structure)
        .collect();
    let mut lines = vec![Line::from(
        "Guard which structure? Any structure qualifies. (Esc to cancel; Up/Down + Enter also work)",
    )];
    if structures.is_empty() {
        lines.push(Line::from("(no structures nearby)"));
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
        lines.push(menu_line(
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
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Assign Guard")),
        popup,
    );
}

fn render_remove_menu(f: &mut Frame, area: Rect, game: &mut Game, selected: usize) {
    let popup = centered_rect(60, 50, area);
    f.render_widget(Clear, popup);
    let structures: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.is_structure)
        .collect();
    let mut lines = vec![Line::from(
        "Demolish which structure? Removing Home destroys the whole base. (Esc to cancel; Up/Down + Enter also work)",
    )];
    if structures.is_empty() {
        lines.push(Line::from("(no structures nearby)"));
    }
    for (i, s) in structures.iter().enumerate() {
        let durability = s
            .durability
            .map(|(hp, max)| format!(" [HP {hp}/{max}]"))
            .unwrap_or_default();
        let home_tag = if s.is_home { " (Home)" } else { "" };
        lines.push(menu_line(
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
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Demolish Structure")),
        popup,
    );
}

fn render_remove_confirm(f: &mut Frame, area: Rect, selected: usize) {
    let popup = centered_rect(60, 40, area);
    f.render_widget(Clear, popup);
    let options = ["Yes, demolish everything", "No, cancel"];
    let mut lines = vec![
        Line::styled(
            "Removing Home destroys every other structure in this base and refunds",
            Style::new().fg(Color::Rgb(255, 140, 0)),
        ),
        Line::styled(
            "30% of each one's materials. This can't be undone.",
            Style::new().fg(Color::Rgb(255, 140, 0)),
        ),
        Line::from(""),
    ];
    for (i, opt) in options.iter().enumerate() {
        lines.push(menu_line(
            format!("[{}] {}", if i == 0 { "y" } else { "n" }, opt),
            i == selected,
        ));
    }
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Confirm Demolish Home")),
        popup,
    );
}

fn render_symlink_menu(f: &mut Frame, area: Rect, game: &mut Game, selected: usize) {
    let popup = centered_rect(60, 50, area);
    f.render_widget(Clear, popup);
    let status = game.player_status();
    let targets = game.symlink_targets();
    let mut lines = vec![Line::from(
        "Use symlink to which structure? (Esc to cancel; Up/Down + Enter also work)",
    )];
    if targets.is_empty() {
        lines.push(Line::from("(no symlink-capable structures deployed yet)"));
    }
    for (i, t) in targets.iter().enumerate() {
        let raw_cost = game.symlink_cost(t.entity).unwrap_or_default();
        let cost = cost_display(game, &raw_cost, &status.inventory);
        let durability = t
            .durability
            .map(|(hp, max)| format!(" [HP {hp}/{max}]"))
            .unwrap_or_default();
        lines.push(menu_line(
            format!(
                "[{}] {} at ({}, {}){} — {}",
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
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Symlink")),
        popup,
    );
}

fn render_companion_menu(f: &mut Frame, area: Rect, game: &mut Game, selected: usize) {
    let popup = centered_rect(70, 60, area);
    f.render_widget(Clear, popup);
    let pets: Vec<PetInfo> = game.owned_pets();
    let mut lines = vec![Line::from(
        "Pick a program to add to your party (max 3) — select a party member's own number to stand it down. (Esc to cancel; Up/Down + Enter also work)",
    )];
    if pets.is_empty() {
        lines.push(Line::from("(you don't have any compiled programs yet)"));
    }
    for (i, p) in pets.iter().enumerate() {
        let active = if p.is_companion { " (in party)" } else { "" };
        let job = p
            .job_structure
            .as_ref()
            .map(|s| format!(" (on a cronjob: {s})"))
            .unwrap_or_default();
        let quality = p
            .quality
            .as_ref()
            .map(|q| format!(" [{q}]"))
            .unwrap_or_default();
        let fused = fusion_tag(p.fusions);
        lines.push(menu_line(
            format!(
                "[{}] {} Lv{} — HP {}/{}  ATK {}  DEF {}  PWR {}{}{}{}{}",
                menu_shortcut(i),
                p.name,
                p.level,
                p.hp,
                p.max_hp,
                p.atk,
                p.def,
                p.power,
                quality,
                fused,
                active,
                job
            ),
            i == selected,
        ));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Party")),
        popup,
    );
}

/// Formats one fuse-candidate row with its full stat line plus party/
/// cronjob status, cross-referencing `pets` (`Game::owned_pets`) by entity
/// — `view_entities` alone only carries a level and an HP fraction, not
/// the raw HP/ATK/DEF/PWR numbers (or party/job status) a fusion decision
/// actually depends on.
/// How a program's fusion depth reads in a menu row — nothing at all for
/// a program that's never been fused, a plain count while it still has
/// fusions left, and an explicit "maxed" note once it's hit
/// `MAX_FUSIONS` and can't be an input to another fusion.
fn fusion_tag(fusions: u32) -> String {
    match fusions {
        0 => String::new(),
        n if n >= MAX_FUSIONS => format!(" (fused {n}/{MAX_FUSIONS} — maxed)"),
        n => format!(" (fused {n}/{MAX_FUSIONS})"),
    }
}

fn fuse_candidate_label(num: char, c: &EntityView, pets: &[PetInfo]) -> String {
    let fused = fusion_tag(c.fusions);
    match pets.iter().find(|p| p.entity == c.entity) {
        Some(p) => {
            let active = if p.is_companion { " (in party)" } else { "" };
            let job = p
                .job_structure
                .as_ref()
                .map(|s| format!(" (on a cronjob: {s})"))
                .unwrap_or_default();
            format!(
                "[{num}] {} Lv{} — HP {}/{}  ATK {}  DEF {}  PWR {}{fused}{active}{job}",
                c.label, p.level, p.hp, p.max_hp, p.atk, p.def, p.power
            )
        }
        None => format!(
            "[{num}] {}{}{fused}",
            c.label,
            c.level.map(|l| format!(" Lv{l}")).unwrap_or_default()
        ),
    }
}

fn render_fuse_menu(f: &mut Frame, area: Rect, game: &mut Game, selected: usize) {
    let popup = centered_rect(60, 50, area);
    f.render_widget(Clear, popup);
    let candidates: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.is_tamed)
        .collect();
    let pets = game.owned_pets();
    let mut lines = vec![Line::from(
        "Fuse which program? Pick the first of two. (Esc to cancel; Up/Down + Enter also work)",
    )];
    if candidates.is_empty() {
        lines.push(Line::from("(no compiled programs nearby)"));
    }
    for (i, c) in candidates.iter().enumerate() {
        lines.push(menu_line(
            fuse_candidate_label(menu_shortcut(i), c, &pets),
            i == selected,
        ));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Fuse")),
        popup,
    );
}

fn render_fuse_second_menu(
    f: &mut Frame,
    area: Rect,
    game: &mut Game,
    first: Option<feral_processes_engine::Entity>,
    selected: usize,
) {
    let popup = centered_rect(60, 50, area);
    f.render_widget(Clear, popup);
    let Some(first) = first else { return };
    let nearby = game.view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS);
    let first_label = nearby
        .iter()
        .find(|e| e.entity == first)
        .map(|e| e.label.clone())
        .unwrap_or_else(|| "it".to_string());
    let candidates: Vec<_> = nearby
        .into_iter()
        .filter(|e| e.is_tamed && e.entity != first)
        .collect();
    let pets = game.owned_pets();
    let mut lines = vec![Line::from(format!(
        "Fuse {first_label} with which program? Both are consumed by the fusion. (Esc to cancel; Up/Down + Enter also work)"
    ))];
    if candidates.is_empty() {
        lines.push(Line::from("(no other compiled programs nearby)"));
    }
    for (i, c) in candidates.iter().enumerate() {
        lines.push(menu_line(
            fuse_candidate_label(menu_shortcut(i), c, &pets),
            i == selected,
        ));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Fuse")),
        popup,
    );
}

/// Free-text naming page shown after both fuse candidates are picked —
/// same shape as the craft/trade quantity pages, just typing characters
/// instead of digits. Blank and Enter keeps the default species name.
fn render_fuse_name_menu(
    f: &mut Frame,
    area: Rect,
    game: &mut Game,
    first: Option<feral_processes_engine::Entity>,
    second: Option<feral_processes_engine::Entity>,
    name_input: &str,
) {
    let popup = centered_rect(55, 30, area);
    f.render_widget(Clear, popup);
    let (Some(first), Some(second)) = (first, second) else {
        return;
    };
    let nearby = game.view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS);
    let label_of = |e: feral_processes_engine::Entity| {
        nearby
            .iter()
            .find(|ev| ev.entity == e)
            .map(|ev| ev.label.clone())
            .unwrap_or_else(|| "it".to_string())
    };
    let lines = vec![
        Line::from(format!(
            "Fusing {} and {}.",
            label_of(first),
            label_of(second)
        )),
        Line::from(""),
        Line::from(format!(
            "Name it (optional, {} max): {name_input}",
            feral_processes_engine::MAX_CUSTOM_NAME_LEN
        )),
        Line::from(""),
        Line::from("Type a name, Enter to fuse (blank keeps the default name)"),
        Line::from("Esc to go back and re-pick the second program"),
    ];
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Fuse")),
        popup,
    );
}

fn render_trade_menu(f: &mut Frame, area: Rect, game: &mut Game, selected: usize) {
    let popup = centered_rect(60, 50, area);
    f.render_widget(Clear, popup);
    let structures: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.can_trade)
        .collect();
    let mut lines = vec![Line::from(
        "Trade with which structure? (Esc to cancel; Up/Down + Enter also work)",
    )];
    if structures.is_empty() {
        lines.push(Line::from("(no trading posts nearby)"));
    }
    for (i, s) in structures.iter().enumerate() {
        let durability = s
            .durability
            .map(|(hp, max)| format!(" [HP {hp}/{max}]"))
            .unwrap_or_default();
        lines.push(menu_line(
            format!(
                "[{}] {} at ({}, {}){}",
                menu_shortcut(i),
                s.label,
                s.pos.0,
                s.pos.1,
                durability
            ),
            i == selected,
        ));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Trade")),
        popup,
    );
}

/// Combined sell/buy line-item picker: sell offers (one per distinct
/// inventory item, excluding Core Fragments) are numbered first, then buy
/// offers from the structure's trade list.
fn render_trade_action_menu(
    f: &mut Frame,
    area: Rect,
    game: &mut Game,
    structure: Option<feral_processes_engine::Entity>,
    selected: usize,
) {
    let popup = centered_rect(65, 60, area);
    f.render_widget(Clear, popup);
    let Some(structure) = structure else { return };
    let Some(trade) = game.trade_options(structure) else {
        return;
    };
    let inventory = game.player_status().inventory;
    let currency = game.currency();

    let mut lines = vec![Line::styled(
        "Sell (from inventory):",
        Style::new().add_modifier(Modifier::BOLD),
    )];
    let sellable: Vec<_> = inventory
        .iter()
        .filter(|(item, _)| *item != currency)
        .collect();
    if sellable.is_empty() {
        lines.push(Line::from("  (nothing to sell)"));
    }
    let mut idx = 0;
    for (item, qty) in &sellable {
        lines.push(menu_line(
            format!(
                "[{}] Sell {} x{qty} ({} Core Fragments each)",
                menu_shortcut(idx),
                game.item_name(item),
                trade.sell_rate
            ),
            idx == selected,
        ));
        idx += 1;
    }
    lines.push(Line::from(""));
    lines.push(Line::styled(
        "Buy:",
        Style::new().add_modifier(Modifier::BOLD),
    ));
    for (item, cost) in &trade.buy {
        lines.push(menu_line(
            format!(
                "[{}] Buy {} ({cost} Core Fragments each)",
                menu_shortcut(idx),
                game.item_name(item)
            ),
            idx == selected,
        ));
        idx += 1;
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Esc to cancel; Up/Down + Enter also work"));

    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Trade")),
        popup,
    );
}

fn render_trade_quantity_menu(
    f: &mut Frame,
    area: Rect,
    game: &mut Game,
    structure: Option<feral_processes_engine::Entity>,
    choice: Option<TradeChoice>,
    quantity_input: &str,
) {
    let popup = centered_rect(60, 30, area);
    f.render_widget(Clear, popup);
    let (Some(structure), Some(choice)) = (structure, choice) else {
        return;
    };
    let Some(trade) = game.trade_options(structure) else {
        return;
    };

    let (verb, item, unit_price) = match choice {
        TradeChoice::Sell(item) => ("Sell", item, trade.sell_rate),
        TradeChoice::Buy(item) => {
            let price = trade
                .buy
                .iter()
                .find(|(i, _)| *i == item)
                .map(|(_, c)| *c)
                .unwrap_or(0);
            ("Buy", item, price)
        }
    };
    let shown = if quantity_input.is_empty() {
        "1"
    } else {
        quantity_input
    };
    let lines = vec![
        Line::from(format!("{verb} how many {}?", game.item_name(&item))),
        Line::from(""),
        Line::from(format!("Price: {unit_price} Core Fragments each")),
        Line::from(""),
        Line::from(format!("Quantity: {shown}")),
        Line::from(""),
        Line::from(format!(
            "Type digits, Enter to {}, Esc to go back",
            verb.to_lowercase()
        )),
    ];
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Trade")),
        popup,
    );
}

fn render_perks_menu(f: &mut Frame, area: Rect, game: &mut Game, selected: usize) {
    let popup = centered_rect(65, 55, area);
    f.render_widget(Clear, popup);
    let status = game.player_status();
    let mut lines = vec![
        Line::styled(
            format!("Perk Points: {}", status.perk_points),
            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Line::from(""),
    ];
    for (i, perk) in feral_processes_engine::Perk::all().iter().enumerate() {
        let level = status.unlocked_perks.iter().filter(|p| *p == perk).count();
        let tag = if level > 0 {
            format!(" (level {level})")
        } else {
            String::new()
        };
        let mut style = if level > 0 {
            Style::new().fg(Color::Green)
        } else {
            Style::new()
        };
        let prefix = if i == selected {
            style = style.add_modifier(Modifier::REVERSED);
            "> "
        } else {
            "  "
        };
        lines.push(Line::styled(
            format!(
                "{prefix}[{}] {} — {} Perk Points{}",
                menu_shortcut(i),
                perk.display_name(),
                perk.cost(),
                tag
            ),
            style,
        ));
        lines.push(Line::from(format!("    {}", perk.description())));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(
        "Pick a row's key to buy another level (Up/Down + Enter also work). Esc to close",
    ));
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Perks")),
        popup,
    );
}

fn render_research_menu(f: &mut Frame, area: Rect, game: &mut Game, selected: usize) {
    let popup = centered_rect(70, 65, area);
    f.render_widget(Clear, popup);
    let research_currency = game.research_currency();
    let held = game
        .player_status()
        .inventory
        .iter()
        .find(|(item, _)| *item == research_currency)
        .map(|(_, n)| *n)
        .unwrap_or(0);
    let bank_limit = game.bank_limit_of(&research_currency).unwrap_or(0);
    let nodes = game.research_nodes();
    let mut lines = vec![
        Line::styled(
            format!("Research Data: {held}/{bank_limit}"),
            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Line::from(""),
    ];
    for (i, node) in nodes.iter().enumerate() {
        let (tag, mut style) = match &node.state {
            ResearchState::Unlocked => (" (researched)".to_string(), Style::new().fg(Color::Green)),
            ResearchState::Available if node.affordable => (String::new(), Style::new()),
            ResearchState::Available => (String::new(), Style::new().fg(Color::DarkGray)),
            ResearchState::Locked { missing } => (
                format!(" (needs {})", missing.join(", ")),
                Style::new().fg(Color::DarkGray),
            ),
        };
        let prefix = if i == selected {
            style = style.add_modifier(Modifier::REVERSED);
            "> "
        } else {
            "  "
        };
        lines.push(Line::styled(
            format!(
                "{prefix}[{}] {} — {} Research Data{tag}",
                menu_shortcut(i),
                node.name,
                node.cost
            ),
            style,
        ));
        lines.push(Line::from(format!("    {}", node.description)));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(
        "Pick a row's key to research it (Up/Down + Enter also work). Esc to close",
    ));
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Research")),
        popup,
    );
}

/// Popup shown over the battle screen (`Mode::BattleCompanion`) when more
/// than one companion is active, to pick which one acts this round. A
/// single active companion skips this and is commanded directly.
fn render_battle_companion_menu(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let popup = centered_rect(60, 40, area);
    f.render_widget(Clear, popup);
    let selected = app.menu_selected;
    let Some(game) = &mut app.game else { return };
    let party = game.player_status().companions;
    let mut lines = vec![Line::from(
        "Command which companion? It'll buff you instead of attacking. (Esc to cancel; Up/Down + Enter also work)",
    )];
    for (i, c) in party.iter().enumerate() {
        lines.push(menu_line(
            format!(
                "[{}] {} ({}){}",
                menu_shortcut(i),
                c.name,
                c.ability,
                status_tag(&c.status)
            ),
            i == selected,
        ));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Command Companion")),
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

fn render_inspect_detail(
    f: &mut Frame,
    area: Rect,
    game: &mut Game,
    entity: Option<feral_processes_engine::Entity>,
) {
    let popup = centered_rect(60, 60, area);
    f.render_widget(Clear, popup);
    let Some(view) = entity.and_then(|e| game.inspect(e)) else {
        f.render_widget(
            Paragraph::new(Line::from(
                "That program is gone. Press any key to go back.",
            ))
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
            format!(
                "{}{}{}",
                view.name,
                view.level.map(|l| format!(" — Lv{l}")).unwrap_or_default(),
                if view.is_boss { " [BOSS]" } else { "" }
            ),
            Style::new()
                .fg(if view.is_boss {
                    Color::Red
                } else {
                    Color::White
                })
                .add_modifier(Modifier::BOLD),
        ),
        Line::from(format!("Status: {status}")),
        Line::from(format!("Integrity: {}/{}", view.hp, view.max_hp)),
        Line::from(format!(
            "Attack {}   Defense {}   Power {}",
            view.atk, view.def, view.power
        )),
        Line::from(format!(
            "Decompile difficulty: {:.0}%",
            view.taming_difficulty * 100.0
        )),
    ];
    if let Some(quality) = &view.quality {
        lines.push(Line::from(format!("Potential: {quality}")));
    }
    if view.fusions > 0 {
        lines.push(Line::from(format!(
            "Fusions: {}/{MAX_FUSIONS}{}",
            view.fusions,
            if view.fusions >= MAX_FUSIONS {
                " (can't be fused again)"
            } else {
                ""
            }
        )));
    }
    if view.is_hostile && !view.is_tamed {
        lines.push(Line::styled(
            format!(
                "Decompile chance right now: {:.0}%",
                view.decompile_chance * 100.0
            ),
            Style::new().fg(Color::Magenta),
        ));
    }
    lines.push(Line::from(format!(
        "Habitats: {}",
        if habitats.is_empty() {
            "unknown".to_string()
        } else {
            habitats.join(", ")
        }
    )));
    lines.push(Line::from(format!(
        "Moves: {}",
        if moves.is_empty() {
            "none".to_string()
        } else {
            moves.join(", ")
        }
    )));
    if let Some(res) = view.work_resource {
        lines.push(Line::from(format!(
            "Work aptitude: {}",
            game.item_name(&res)
        )));
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

fn render_inventory_screen(f: &mut Frame, area: Rect, game: &mut Game, selected: usize) {
    let popup = centered_rect(70, 70, area);
    f.render_widget(Clear, popup);
    let status = game.player_status();

    let mut lines = vec![
        Line::styled(
            format!(
                "Level {}   Attack {}   Defense {}   Power {}   Decompiler {}",
                status.level, status.atk, status.def, status.power, status.decompiler
            ),
            Style::new().fg(Color::Cyan),
        ),
        Line::from(""),
        Line::styled(
            "Equipped (number to unequip):",
            Style::new().add_modifier(Modifier::BOLD),
        ),
        equipped_line(1, "Weapon", status.weapon.clone(), selected == 0, game),
        equipped_line(2, "Armor", status.armor.clone(), selected == 1, game),
        equipped_line(3, "Module", status.module.clone(), selected == 2, game),
        Line::from(""),
        Line::styled(
            format!(
                "Inventory — Buffer {}/{} (row key to equip/fuse/erase):",
                status.inventory_used, status.inventory_capacity
            ),
            Style::new().add_modifier(Modifier::BOLD),
        ),
    ];
    if status.inventory.is_empty() {
        lines.push(Line::from("  (empty)"));
    }
    for (i, (item, qty)) in status.inventory.iter().enumerate() {
        let fusion_tier = game.item_fusion_tier(item.clone());
        let tag = equip_preview_tag(game, item, status.zone, fusion_tier);
        lines.push(menu_line(
            format!(
                "[{}] {} x{}{}",
                menu_shortcut(i + 3),
                game.item_name(item),
                qty,
                tag
            ),
            selected == i + 3,
        ));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Esc to close; Up/Down + Enter also work"));

    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Inventory")),
        popup,
    );
}

fn equipped_line(
    num: usize,
    label: &str,
    equipped: Option<EquippedItem>,
    selected: bool,
    game: &Game,
) -> Line<'static> {
    match equipped.and_then(|e| game.equipment_of(&e.item).map(|(_, mods)| (e, mods))) {
        Some((equipped, mods)) => {
            let mods = mods
                .scaled_for_level(equipped.level)
                .fused_for_tier(equipped.fusion_tier);
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
            let mut notes = Vec::new();
            if equipped.level > 1 {
                notes.push(format!("Lv{}", equipped.level));
            }
            if equipped.fusion_tier > 0 {
                notes.push(format!("T{}", equipped.fusion_tier));
            }
            let note = if notes.is_empty() {
                String::new()
            } else {
                format!(" {}", notes.join(" "))
            };
            menu_line(
                format!(
                    "[{num}] {label}: {}{note} ({})",
                    game.item_name(&equipped.item),
                    parts.join(" ")
                ),
                selected,
            )
        }
        None => menu_line(format!("[{num}] {label}: (empty)"), selected),
    }
}

/// Formats an equippable item's stat bonus as it would be *if equipped
/// right now* — gear scales with the current zone level at the moment you
/// equip it (see `Game::equip`), so this previews that same number rather
/// than a flat, unscaled base value. Empty string for a non-equippable
/// item (in place of the old generic "(equippable)" tag).
fn equip_preview_tag(game: &Game, item: &ItemId, zone_level: u32, fusion_tier: u32) -> String {
    let Some((_, base_mods)) = game.equipment_of(item) else {
        return String::new();
    };
    let mods = base_mods
        .scaled_for_level(zone_level)
        .fused_for_tier(fusion_tier);
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
    if fusion_tier > 0 {
        parts.push(format!("fusion T{fusion_tier}"));
    }
    format!(" ({})", parts.join(" "))
}

fn render_inventory_item_action(
    f: &mut Frame,
    area: Rect,
    item: Option<ItemId>,
    zone_level: u32,
    fusion_tier: u32,
    selected: usize,
    game: &Game,
) {
    let popup = centered_rect(50, 30, area);
    f.render_widget(Clear, popup);
    let Some(item) = item else {
        f.render_widget(
            Paragraph::new(Line::from("Nothing selected.")).block(Block::bordered().title("Item")),
            popup,
        );
        return;
    };
    let actions = inventory_item_actions(game, &item);
    let mut lines = vec![
        Line::styled(
            format!(
                "{}{}",
                game.item_name(&item),
                equip_preview_tag(game, &item, zone_level, fusion_tier)
            ),
            Style::new().add_modifier(Modifier::BOLD),
        ),
        Line::from(""),
    ];
    for (i, (_, label)) in actions.iter().enumerate() {
        lines.push(menu_line(label.clone(), i == selected));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Esc to cancel; Up/Down + Enter also work"));
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Item")),
        popup,
    );
}

/// Formats an active battle status effect (e.g. "Bleeding (2)") as a
/// bracketed suffix for a title/label — `" [Bleeding (2)]"` — or an empty
/// string if there's no active condition.
fn status_tag(status: &Option<String>) -> String {
    status
        .as_ref()
        .map(|s| format!(" [{s}]"))
        .unwrap_or_default()
}

fn render_battle(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let Some(game) = &mut app.game else { return };
    let Some(view) = game.battle_view() else {
        return;
    };

    let mut constraints = vec![Constraint::Length(3), Constraint::Length(3)];
    if !view.companions.is_empty() {
        constraints.push(Constraint::Length(view.companions.len() as u16));
    }
    constraints.push(Constraint::Length(1));
    constraints.push(Constraint::Min(5));
    constraints.push(Constraint::Length(3));
    let chunks = Layout::vertical(constraints).split(area);
    let mut i = 0;

    let wild_ratio = (view.wild_hp as f64 / view.wild_max_hp.max(1) as f64).clamp(0.0, 1.0);
    let pack_tag = if view.pack_remaining > 0 {
        format!(" [+{} more in the pack]", view.pack_remaining)
    } else {
        String::new()
    };
    f.render_widget(
        Gauge::default()
            .block(Block::bordered().title(format!(
                "{}{}{}{} (ATK {} / DEF {} / PWR {})",
                view.wild_name,
                if view.wild_is_boss { " [BOSS]" } else { "" },
                status_tag(&view.wild_status_effect),
                pack_tag,
                view.wild_atk,
                view.wild_def,
                view.wild_power
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
                "You{} (ATK {} / DEF {} / PWR {} / DECOMP {})",
                status_tag(&view.player_status_effect),
                view.player_atk,
                view.player_def,
                view.player_power,
                view.player_decompiler
            )))
            .gauge_style(Style::new().fg(Color::Cyan))
            .ratio(player_ratio)
            .label(format!("{}/{}", view.player_hp, view.player_max_hp)),
        chunks[i],
    );
    i += 1;

    if !view.companions.is_empty() {
        let lines: Vec<Line> = view
            .companions
            .iter()
            .map(|companion| {
                Line::styled(
                    format!(
                        "Companion: {} (HP {}/{}, ATK {}, PWR {}){}",
                        companion.name,
                        companion.hp,
                        companion.max_hp,
                        companion.atk,
                        companion.power,
                        status_tag(&companion.status)
                    ),
                    Style::new().fg(Color::Green),
                )
            })
            .collect();
        f.render_widget(Paragraph::new(lines), chunks[i]);
        i += 1;
    }

    f.render_widget(
        Paragraph::new(Line::styled(
            format!(
                "Decompile chance right now: {:.0}%{}",
                view.decompile_chance * 100.0,
                if view.can_tame {
                    ""
                } else {
                    " (needs an ICE Breaker)"
                }
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
        .map(|(kind, text)| message_line(kind, text))
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
    if !view.companions.is_empty() {
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
    let mut options = vec!["[N] New Game".to_string()];
    if !app.list_saves().is_empty() {
        options.push("[L] Load Game".to_string());
    }
    options.push("[Q] Quit".to_string());
    let mut lines = vec![
        Line::styled("feral-processes", Style::new().add_modifier(Modifier::BOLD)),
        Line::styled(
            "// jack into the Grid",
            Style::new().fg(Color::Cyan).add_modifier(Modifier::ITALIC),
        ),
        Line::from(""),
    ];
    for (i, opt) in options.iter().enumerate() {
        lines.push(menu_line(opt.clone(), i == app.menu_selected));
    }
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

fn render_load_game_menu(f: &mut Frame, app: &App) {
    let area = f.area();
    let saves = app.list_saves();
    let mut lines = vec![Line::from(
        "Pick a save (Esc to cancel; Up/Down + Enter also work)",
    )];
    if saves.is_empty() {
        lines.push(Line::from("(no saves found)"));
    }
    for (i, save) in saves.iter().enumerate() {
        let summary = save
            .summary
            .as_deref()
            .unwrap_or("(incompatible save — can still be deleted)");
        lines.push(menu_line(
            format!("[{}] {} — {}", menu_shortcut(i), save.name, summary),
            i == app.menu_selected,
        ));
    }
    let popup = centered_rect(70, 60, area);
    f.render_widget(Clear, popup);
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Load Game")),
        popup,
    );
}

fn render_save_action_menu(f: &mut Frame, app: &App) {
    let area = f.area();
    let popup = centered_rect(50, 30, area);
    f.render_widget(Clear, popup);
    let name = app
        .pending_save
        .as_ref()
        .and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "(unknown save)".to_string());
    let mut lines = vec![
        Line::styled(name, Style::new().add_modifier(Modifier::BOLD)),
        Line::from(""),
        menu_line("[L]oad".to_string(), app.menu_selected == 0),
        menu_line("[X] Delete".to_string(), app.menu_selected == 1),
        Line::from(""),
        Line::from("Esc to cancel; Up/Down + Enter also work"),
    ];
    if let Some(s) = &app.status_line {
        lines.push(Line::from(""));
        lines.push(Line::styled(s.clone(), Style::new().fg(Color::Red)));
    }
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Save")),
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
        Line::from(
            "r                   recharge overnight (restores fatigue and Integrity, uses power)",
        ),
        Line::from("g                   scan the sector for core fragments"),
        Line::from(
            "c                   open the compile menu; then pick a quantity (digits+Enter, F for 5, M for max affordable)",
        ),
        Line::from("b                   deploy a structure"),
        Line::from("w                   assign a compiled program to a cronjob"),
        Line::from(
            "G                   assign a compiled program to guard a structure against raids (any structure, no cronjob needed)",
        ),
        Line::from(
            "R                   demolish a nearby structure (30% material refund; demolishing Home destroys the whole base)",
        ),
        Line::from(
            "u                   use symlink: instantly teleport to a deployed symlink structure (e.g. Home)",
        ),
        Line::from(
            "i                   pick a direction, inspect the first program that way (stats/moves, no intrusion)",
        ),
        Line::from("v                   inventory/equipment: equip, unequip, erase items"),
        Line::from(
            "T                   research tree: spend Research Data to unlock structures and recipes",
        ),
        Line::from(
            "p                   your pets: full stats for every compiled program you own; add/stand down party (max 3)",
        ),
        Line::from("f                   fuse two nearby compiled programs into one stronger one"),
        Line::from(
            "t                   trade with a nearby Black Market: sell items, buy consumables",
        ),
        Line::from("x                   perks: spend Perk Points on permanent passive unlocks"),
        Line::from(
            "s                   save session (also autosaves to the same file every 50 ticks)",
        ),
        Line::from(
            "q                   return to the main menu (unsaved progress is lost — save first)",
        ),
        Line::from("+ / -               zoom the grid in / out"),
        Line::from(""),
        Line::from("Every numbered/lettered menu can also be navigated with Up/Down + Enter,"),
        Line::from("on top of typing a row's own number or letter directly."),
        Line::from(""),
        Line::from("In an intrusion:  a attack   d decompile (needs an ICE Breaker)"),
        Line::from(
            "                  c command a companion to buff you (picks one if more than one is active)   j jack out",
        ),
        Line::from(""),
        Line::from("Defeating or decompiling a rogue program grants XP. Compiled programs"),
        Line::from("gain XP from completed work cycles. Leveling up fully restores Integrity."),
        Line::from("Flatlining (or jacking out of a fight) docks a mild 20% XP setback —"),
        Line::from("never a de-level, just a modest step back."),
        Line::from(""),
        Line::from("Equipping a weapon/armor/module grants a flat Attack/Defense/Decompiler"),
        Line::from("bonus while worn. Equip up to one item per slot; equipping a second"),
        Line::from("item in an occupied slot swaps the old one back to your inventory."),
        Line::from(""),
        Line::from("Up to 3 companions (p) can fight alongside you at once. Commanding one"),
        Line::from("(c) in an intrusion doesn't attack — it buffs you instead of you acting"),
        Line::from("that round: a temporary Attack rally by default, or its species' own"),
        Line::from("special ability if it has one (a bigger buff, a heal, or a debuff on"),
        Line::from("the wild program). One command per round even with a full party. The"),
        Line::from("wild program's retaliation can still land on the player or any party"),
        Line::from("member. Assigning a party member to a cronjob (w) stands it down, and"),
        Line::from("vice versa — one job at a time per program. Every active party member"),
        Line::from("gains half your XP from a kill or decompile, and can level up from it."),
        Line::from(""),
        Line::from("Defeated rogue programs sometimes leave a Portal Fragment. Deploy (b) a"),
        Line::from("Zone Portal from enough of them, then walk onto it to breach into the"),
        Line::from("next zone: a fresh sector where wild programs' stats have doubled. Your"),
        Line::from("compiled programs travel with you; deployed structures and wild programs"),
        Line::from("are left behind, and there's no portal back down. Each zone level's Portal"),
        Line::from("costs twice as many fragments as the last to deploy."),
        Line::from(""),
        Line::from("Rare boss programs (bold on the map, tagged [BOSS]) sometimes take a"),
        Line::from("habitat's spawn slot instead of an ordinary program. Much tougher, but"),
        Line::from("defeating one guarantees a cache of several Portal Fragments at once."),
        Line::from(""),
        Line::from(
            "Fuse (f) two compiled programs into one: pick two, and both are consumed (a program \
             can only be fused 3 times)",
        ),
        Line::from("to produce a new tamed program at the higher of the two levels, whose"),
        Line::from("species (and so moves/work aptitude) matches whichever input was that"),
        Line::from("level (ties favor the first pick). Each stat is the higher input's value"),
        Line::from("plus half the lower one's — always stronger than either alone, but not"),
        Line::from("simply their sum. A good way to turn duplicate catches into one keeper."),
        Line::from(""),
        Line::from("Trade (t) with a nearby Black Market: sell any inventory item (except"),
        Line::from("Core Fragments) for Core Fragments at its flat sell rate, or buy specific"),
        Line::from("consumables it lists — Portal Fragments included — for Core Fragments."),
        Line::from("Turns excess loot and a Core Fragment surplus into whatever you're short on."),
        Line::from(""),
        Line::from("Some moves also inflict a status condition on top of their damage,"),
        Line::from("shown bracketed on the intrusion screen: Bleeding deals extra damage"),
        Line::from("each round; Stunned costs the afflicted side their next action. Only"),
        Line::from("one condition is active at a time — a fresh one overwrites the old."),
        Line::from(""),
        Line::from("Every deployed structure has raid Durability (shown [HP x/y] in the"),
        Line::from("cronjob/symlink/trade menus). Occasionally a raid damages a random"),
        Line::from("structure: a program assigned to it — whether cronjob-working it (w)"),
        Line::from("or just posted to guard it (G) — fights the raid off, reducing the"),
        Line::from("damage by its Defense at a flat cost to its own HP (it stands down if"),
        Line::from("knocked out, but isn't destroyed); an unassigned structure just takes"),
        Line::from("the full hit. Guarding works on any structure, including ones with no"),
        Line::from(
            "cronjob recipe at all. Durability regenerates slowly over time either way, and",
        ),
        Line::from("recharging overnight (r) fully heals every tamed program you own, not"),
        Line::from("just your active party — including one left behind defending a raid."),
        Line::from(""),
        Line::from("Every level-up also grants a Perk Point (shown in the status panel)."),
        Line::from("Spend them (x) on permanent passive unlocks: Keen Scavenger (+15%"),
        Line::from("scan chance), Low Power Mode (30% slower Power drain), Exploit Focus"),
        Line::from("(+5 effective Decompiler skill), and Lean Compiler (compiling costs 1"),
        Line::from("less of each required item, min 1 each). Each perk unlocks once."),
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

fn render_difficulty_pick(f: &mut Frame, selected: usize) {
    let area = f.area();
    let lines = vec![
        Line::from("Choose difficulty"),
        Line::from(""),
        menu_line(
            "[P] Permadeath - flatlining is final; the session is archived to a log".to_string(),
            selected == 0,
        ),
        menu_line(
            "[F] Forgiving - flatlining costs you, but you reboot and keep going".to_string(),
            selected == 1,
        ),
        Line::from(""),
        Line::from("Esc to go back; Up/Down + Enter also work"),
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
