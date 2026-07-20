//! All drawing for the graphics frontend. Mirrors what
//! `feral-processes-tui`'s `ui.rs` shows for each `Mode` — same engine data,
//! same information — laid out with macroquad's immediate-mode primitives
//! (filled rects for bars/tiles, drawn text for menus) instead of ratatui
//! widgets.

use macroquad::prelude::*;

use feral_processes_app_core::{App, MENU_SCAN_RADIUS, Mode, TradeChoice};
use feral_processes_engine::components::GlyphColor;
use feral_processes_engine::items::ItemId;
use feral_processes_engine::world::Biome;
use feral_processes_engine::{Entity, EntityView, Game, PetInfo};

const FONT_SIZE: f32 = 24.0;
const LINE_HEIGHT: f32 = 30.0;
const PANEL_BG: Color = Color::new(0.06, 0.07, 0.10, 0.95);
const BORDER: Color = Color::new(0.25, 0.65, 0.65, 1.0);
const TEXT: Color = Color::new(0.92, 0.92, 0.92, 1.0);
const TEXT_DIM: Color = Color::new(0.6, 0.6, 0.65, 1.0);
const SELECT_BG: Color = Color::new(0.25, 0.55, 0.55, 0.55);
const RED: Color = Color::new(0.9, 0.25, 0.25, 1.0);
const YELLOW: Color = Color::new(0.9, 0.8, 0.2, 1.0);
const BLUE: Color = Color::new(0.3, 0.55, 0.95, 1.0);
const CYAN: Color = Color::new(0.25, 0.85, 0.85, 1.0);
const MAGENTA: Color = Color::new(0.8, 0.35, 0.85, 1.0);
const GREEN: Color = Color::new(0.35, 0.85, 0.4, 1.0);

pub fn draw(app: &mut App) {
    clear_background(Color::new(0.02, 0.02, 0.03, 1.0));
    match app.mode {
        Mode::MainMenu => draw_main_menu(app),
        Mode::LoadGame => draw_load_game(app),
        Mode::SaveAction => draw_save_action(app),
        Mode::DifficultyPick => draw_difficulty_pick(app.menu_selected),
        Mode::GameOver => draw_game_over(app),
        Mode::Battle => draw_battle(app),
        Mode::BattleCompanion => {
            draw_battle(app);
            draw_battle_companion_menu(app);
        }
        Mode::Help => {
            draw_playing_base(app);
            draw_help();
        }
        _ => {
            draw_playing_base(app);
            draw_mode_overlay(app);
        }
    }
}

/// One line of a popup's body. `Item` rows are the numbered/lettered
/// options a menu key press resolves to (see `App::selected_index`);
/// `Text` rows are just informational.
enum Row {
    Text(String),
    TextColored(String, Color),
    Item(String, bool),
}

fn text_row(s: impl Into<String>) -> Row {
    Row::Text(s.into())
}

fn item_row(s: impl Into<String>, selected: bool) -> Row {
    Row::Item(s.into(), selected)
}

/// How much of the window a popup claims. Height always shrinks to fit
/// short content regardless of size (see `draw_popup`), so this really
/// only controls width in practice — `Small` exists for the handful of
/// one-line prompts that would otherwise be a lot of empty box around a
/// single sentence.
#[derive(Clone, Copy)]
enum PopupSize {
    /// Every list/detail menu with real content — deploy/compile/trade/
    /// inventory/party/etc. Sized to leave long rows room rather than
    /// running off the popup's edge, and to give scrollable lists (see
    /// `draw_popup`) more rows on screen before they need to scroll at all.
    Large,
    /// A short, single-purpose prompt with nothing to clip: a direction
    /// picker, a "that program is gone" message.
    Small,
}

/// Centered popup, sized as a percentage of the window — same idea as
/// `ui.rs`'s `centered_rect`, just in pixels instead of terminal cells.
///
/// `rows` is split around its first/last `Row::Item`: everything before is
/// a pinned header (the prompt line), everything after is a pinned footer
/// (e.g. "Esc to cancel"), and the `Item` span in between is the
/// scrollable body. Long lists (more structures/pets/etc. than fit the
/// popup) auto-scroll to keep the highlighted row in view instead of
/// silently running off the bottom with no way to see or reach it.
fn draw_popup(title: &str, size: PopupSize, rows: &[Row]) {
    let (pct_w, pct_h) = match size {
        PopupSize::Large => (0.88, 0.85),
        PopupSize::Small => (0.5, 0.85),
    };
    let w = screen_width() * pct_w;
    let h = (screen_height() * pct_h).min(rows.len() as f32 * LINE_HEIGHT + 70.0).max(120.0);
    let x = (screen_width() - w) / 2.0;
    let y = (screen_height() - h) / 2.0;

    draw_rectangle(x, y, w, h, PANEL_BG);
    draw_rectangle_lines(x, y, w, h, 2.0, BORDER);
    draw_text(title, x + 12.0, y + 24.0, FONT_SIZE + 4.0, CYAN);
    draw_line(x + 8.0, y + 34.0, x + w - 8.0, y + 34.0, 1.0, BORDER);

    let first_item = rows.iter().position(|r| matches!(r, Row::Item(_, _)));
    let last_item = rows.iter().rposition(|r| matches!(r, Row::Item(_, _)));
    let (header, body, footer): (&[Row], &[Row], &[Row]) = match (first_item, last_item) {
        (Some(first), Some(last)) => (&rows[..first], &rows[first..=last], &rows[last + 1..]),
        _ => (rows, &[], &[]),
    };

    let mut cy = y + 60.0;
    let max_y = y + h - 10.0;
    for row in header {
        cy = draw_row(row, x, w, cy, max_y);
    }

    let footer_h = footer.len() as f32 * LINE_HEIGHT;
    let body_bottom = (max_y - footer_h).max(cy);
    let raw_capacity = ((body_bottom - cy) / LINE_HEIGHT).floor().max(0.0) as usize;
    let scrolling = body.len() > raw_capacity;
    // Scrolling reserves one line above and below for "N more" indicators,
    // so the item rows themselves never get a partial cut-off line.
    let capacity = if scrolling { raw_capacity.saturating_sub(2).max(1) } else { raw_capacity };

    if !body.is_empty() {
        let selected_idx = body.iter().position(|r| matches!(r, Row::Item(_, true))).unwrap_or(0);
        let scroll_offset = if body.len() <= capacity {
            0
        } else {
            let max_offset = body.len() - capacity;
            selected_idx.saturating_sub(capacity / 2).min(max_offset)
        };

        if scrolling {
            let text = if scroll_offset > 0 {
                format!("^ {scroll_offset} more above")
            } else {
                String::new()
            };
            draw_text(&text, x + 16.0, cy, FONT_SIZE - 4.0, TEXT_DIM);
            cy += LINE_HEIGHT;
        }

        let visible_end = (scroll_offset + capacity).min(body.len());
        for row in &body[scroll_offset..visible_end] {
            cy = draw_row(row, x, w, cy, max_y);
        }

        if scrolling {
            let below = body.len() - visible_end;
            let text = if below > 0 {
                format!("v {below} more below")
            } else {
                String::new()
            };
            draw_text(&text, x + 16.0, cy, FONT_SIZE - 4.0, TEXT_DIM);
            cy += LINE_HEIGHT;
        }
    }

    for row in footer {
        cy = draw_row(row, x, w, cy, max_y);
    }
}

/// Draws one popup row and returns the y coordinate for the next one.
/// `max_y` is a last-resort safety clamp — normal layout keeps every row
/// within bounds via `draw_popup`'s capacity accounting, so this only ever
/// bites if that accounting is off by a line.
fn draw_row(row: &Row, x: f32, w: f32, cy: f32, max_y: f32) -> f32 {
    if cy > max_y {
        return cy;
    }
    match row {
        Row::Text(s) => {
            draw_text(s, x + 16.0, cy, FONT_SIZE, TEXT_DIM);
        }
        Row::TextColored(s, color) => {
            draw_text(s, x + 16.0, cy, FONT_SIZE, *color);
        }
        Row::Item(s, selected) => {
            if *selected {
                draw_rectangle(x + 6.0, cy - FONT_SIZE, w - 12.0, LINE_HEIGHT, SELECT_BG);
            }
            let prefix = if *selected { "> " } else { "  " };
            draw_text(&format!("{prefix}{s}"), x + 16.0, cy, FONT_SIZE, TEXT);
        }
    }
    cy + LINE_HEIGHT
}

/// Formats a `(item, quantity)` cost list, tagged `(have/need)` — same
/// convention as `ui.rs::cost_display`.
fn cost_display(cost: &[(ItemId, u32)], inventory: &[(ItemId, u32)]) -> Vec<String> {
    cost.iter()
        .map(|(item, qty)| {
            let have = inventory
                .iter()
                .find(|(i, _)| i == item)
                .map(|(_, q)| *q)
                .unwrap_or(0);
            format!("{} ({have}/{qty})", item.display_name())
        })
        .collect()
}

fn glyph_color(c: GlyphColor) -> Color {
    match c {
        GlyphColor::White => WHITE,
        GlyphColor::Gray => GRAY,
        GlyphColor::Green => GREEN,
        GlyphColor::DarkGreen => Color::new(0.0, 0.4, 0.0, 1.0),
        GlyphColor::Red => RED,
        GlyphColor::Yellow => YELLOW,
        GlyphColor::Blue => BLUE,
        GlyphColor::Magenta => MAGENTA,
        GlyphColor::Cyan => CYAN,
        GlyphColor::Brown => Color::new(0.55, 0.27, 0.07, 1.0),
        GlyphColor::Orange => Color::new(1.0, 0.55, 0.0, 1.0),
    }
}

fn biome_style(biome: Biome) -> (char, Color) {
    match biome {
        Biome::DataVoid => ('~', BLUE),
        Biome::BlackIce => ('^', RED),
        Biome::Mainframe => ('#', CYAN),
        Biome::OpenGrid => ('.', GREEN),
        Biome::NullSector => (':', GRAY),
        Biome::StaticField => ('%', WHITE),
    }
}

/// The world grid, status panel, and message feed — the base layer shown
/// under `Mode::Playing` and every menu popup, same as `ui.rs::render_playing`.
fn draw_playing_base(app: &mut App) {
    let zoom = app.zoom.clamp(1, 8) as f32;
    let status_line = app.status_line.clone();
    let Some(game) = &mut app.game else { return };

    let map_w = screen_width() * 0.7;
    let map_h = screen_height() * 0.72;
    let tile_px = 20.0 * zoom.min(3.0);
    let half_w = ((map_w / tile_px) / 2.0).max(1.0) as i32;
    let half_h = ((map_h / tile_px) / 2.0).max(1.0) as i32;

    let status = game.player_status();
    let tiles = game.view_tiles(half_w, half_h);
    let entities: Vec<_> = game
        .view_entities(half_w, half_h)
        .into_iter()
        .filter(|e| !e.is_tamed)
        .collect();
    let spawn_point = game.zone_spawn_point();

    draw_rectangle(0.0, 0.0, map_w, map_h, Color::new(0.03, 0.03, 0.05, 1.0));
    for (ry, row) in tiles.iter().enumerate() {
        for (rx, tile) in row.iter().enumerate() {
            let (mut ch, mut color) = biome_style(tile.biome);
            let px = rx as f32 * tile_px;
            let py = ry as f32 * tile_px;
            let mut bold = false;
            let mut staffed = false;
            for ev in &entities {
                let erx = ev.pos.0 - status.position.0 + half_w;
                let ery = ev.pos.1 - status.position.1 + half_h;
                if erx == rx as i32 && ery == ry as i32 {
                    ch = ev.glyph;
                    color = glyph_color(ev.color);
                    bold = ev.is_structure || ev.is_boss;
                    staffed = ev.is_structure && ev.structure_worker.is_some();
                }
            }
            let bg = Color::new(color.r * 0.18, color.g * 0.18, color.b * 0.18, 1.0);
            draw_rectangle(px, py, tile_px - 1.0, tile_px - 1.0, bg);
            let font_size = (tile_px * 0.8).max(14.0);
            let dims = measure_text(&ch.to_string(), None, font_size as u16, 1.0);
            let tx = px + (tile_px - dims.width) / 2.0;
            let ty = py + (tile_px + dims.height) / 2.0;
            draw_text(&ch.to_string(), tx, ty, font_size, if bold { color } else { color });
            // Marks where the player materialized on breaching into this
            // zone (see `Game::zone_spawn_point`) — an outline rather than
            // replacing the glyph, so whatever's actually standing there
            // (the player, a creature, a rebuilt structure) still reads
            // clearly on top of it.
            let spawn_rx = spawn_point.0 - status.position.0 + half_w;
            let spawn_ry = spawn_point.1 - status.position.1 + half_h;
            if rx as i32 == spawn_rx && ry as i32 == spawn_ry {
                draw_rectangle_lines(px, py, tile_px - 1.0, tile_px - 1.0, 2.0, MAGENTA);
            }
            // A structure with a pet actively cronjob-assigned gets a
            // yellow outline so it's visible at a glance without opening
            // the cronjob menu to check.
            if staffed {
                draw_rectangle_lines(px, py, tile_px - 1.0, tile_px - 1.0, 2.0, YELLOW);
            }
        }
    }
    draw_rectangle_lines(0.0, 0.0, map_w, map_h, 2.0, BORDER);

    draw_status_panel(map_w, 0.0, screen_width() - map_w, map_h, &status);

    let log_y = map_h;
    let log_h = screen_height() - map_h;
    draw_rectangle(0.0, log_y, screen_width(), log_h, PANEL_BG);
    draw_rectangle_lines(0.0, log_y, screen_width(), log_h, 2.0, BORDER);
    let mut ly = log_y + 22.0;
    if let Some(s) = &status_line {
        draw_text(s, 10.0, ly, FONT_SIZE, RED);
        ly += LINE_HEIGHT;
    }
    let capacity = ((log_h - 30.0) / LINE_HEIGHT).max(1.0) as usize;
    for line in game.message_log(capacity) {
        if ly > screen_height() - 6.0 {
            break;
        }
        draw_text(&line, 10.0, ly, FONT_SIZE, TEXT_DIM);
        ly += LINE_HEIGHT;
    }
}

fn draw_status_panel(x: f32, y: f32, w: f32, h: f32, status: &feral_processes_engine::PlayerStatus) {
    draw_rectangle(x, y, w, h, PANEL_BG);
    draw_rectangle_lines(x, y, w, h, 2.0, BORDER);

    let mut cy = y + 22.0;
    cy = draw_bar(x + 10.0, cy, w - 20.0, "Integrity", status.hp as f32, status.max_hp.max(1) as f32, RED);
    cy = draw_bar(x + 10.0, cy, w - 20.0, "Power", status.hunger, 100.0, YELLOW);
    cy = draw_bar(x + 10.0, cy, w - 20.0, "Fatigue", status.fatigue, 100.0, BLUE);
    cy += 6.0;

    let lines = [
        format!(
            "Level {}  (XP {}/{})  Perk Pts {}",
            status.level, status.xp, status.xp_to_next, status.perk_points
        ),
        format!("Zone {}", status.zone),
        format!("Position: ({}, {})", status.position.0, status.position.1),
        format!(
            "Attack {}  Defense {}  Power {}",
            status.atk, status.def, status.power
        ),
        format!("Decompiler {}", status.decompiler),
    ];
    for line in &lines {
        draw_text(line, x + 10.0, cy, FONT_SIZE, TEXT);
        cy += LINE_HEIGHT;
    }
    draw_text(
        &format!(
            "Party: {}/{}",
            status.companions.len(),
            feral_processes_engine::resources::MAX_PARTY_SIZE
        ),
        x + 10.0,
        cy,
        FONT_SIZE,
        GREEN,
    );
    cy += LINE_HEIGHT;
    for companion in &status.companions {
        draw_text(
            &format!(
                "Companion: {} (HP {}/{}, PWR {})",
                companion.name, companion.hp, companion.max_hp, companion.power
            ),
            x + 10.0,
            cy,
            FONT_SIZE,
            GREEN,
        );
        cy += LINE_HEIGHT;
    }
    cy += 6.0;
    draw_text("Inventory:", x + 10.0, cy, FONT_SIZE, TEXT);
    cy += LINE_HEIGHT;
    if status.inventory.is_empty() {
        draw_text("(empty)", x + 10.0, cy, FONT_SIZE, TEXT_DIM);
        cy += LINE_HEIGHT;
    }
    let keys = [
        "hjkl/arrows move  . wait  e drain  r recharge",
        "g scan   c compile   b deploy   w cronjob  G guard",
        "u symlink   i inspect   v inventory",
        "p companions  f fuse  t trade  x perks",
        "s save   q main menu   ? help   +/- zoom",
    ];
    let keys_line_height = LINE_HEIGHT - 4.0;
    let keys_block_h = keys.len() as f32 * keys_line_height + 10.0;
    let keys_y = y + h - keys_block_h;

    for (item, qty) in &status.inventory {
        if cy > keys_y - LINE_HEIGHT {
            break;
        }
        draw_text(&format!("{} x{}", item.display_name(), qty), x + 10.0, cy, FONT_SIZE, TEXT_DIM);
        cy += LINE_HEIGHT;
    }

    let mut ky = keys_y;
    for k in keys {
        draw_text(k, x + 10.0, ky, FONT_SIZE - 3.0, TEXT_DIM);
        ky += keys_line_height;
    }
}

/// Draws a labeled bar (HP/Power/Fatigue) and returns the y coordinate for
/// whatever's drawn next.
fn draw_bar(x: f32, y: f32, w: f32, label: &str, value: f32, max: f32, color: Color) -> f32 {
    let ratio = (value / max).clamp(0.0, 1.0);
    draw_text(&format!("{label} {value:.0}/{max:.0}"), x, y, FONT_SIZE - 2.0, TEXT);
    let bar_y = y + 6.0;
    draw_rectangle(x, bar_y, w, 14.0, Color::new(0.15, 0.15, 0.15, 1.0));
    draw_rectangle(x, bar_y, w * ratio, 14.0, color);
    draw_rectangle_lines(x, bar_y, w, 14.0, 1.0, BORDER);
    bar_y + 26.0
}

fn draw_mode_overlay(app: &mut App) {
    let selected = app.menu_selected;
    let Some(game) = &mut app.game else { return };
    match app.mode {
        Mode::Build => draw_build_menu(game, selected),
        Mode::BuildDirection => draw_direction_prompt("Deploy Direction", "Choose a direction to deploy (arrows/hjkl), Esc to cancel"),
        Mode::Craft => draw_craft_menu(game, selected),
        Mode::CraftQuantity => draw_craft_quantity(game, app.pending_craft, &app.craft_quantity_input),
        Mode::Cronjob => draw_worker_menu(game, "Assign Cronjob", "Assign which program to a cronjob?", selected),
        Mode::CronjobStructure => draw_structure_menu(game, "Assign Cronjob", "Cronjob which structure?", true, selected),
        Mode::Guard => draw_worker_menu(game, "Assign Guard", "Assign which program to guard duty?", selected),
        Mode::GuardStructure => draw_structure_menu(game, "Assign Guard", "Guard which structure? Any structure qualifies.", false, selected),
        Mode::Symlink => draw_symlink_menu(game, selected),
        Mode::InspectDirection => draw_direction_prompt("Inspect Direction", "Choose a direction to inspect (arrows/hjkl), Esc to cancel"),
        Mode::InspectDetail => draw_inspect_detail(game, app.pending_inspect),
        Mode::Inventory => draw_inventory(game, selected),
        Mode::InventoryItemAction => {
            let status = game.player_status();
            let stack_qty = app
                .pending_inventory_item
                .and_then(|item| status.inventory.iter().find(|(i, _)| *i == item).map(|(_, q)| *q))
                .unwrap_or(0);
            let fusion_tier = app
                .pending_inventory_item
                .map(|item| game.item_fusion_tier(item))
                .unwrap_or(0);
            draw_inventory_item_action(
                app.pending_inventory_item,
                status.zone,
                stack_qty,
                fusion_tier,
                selected,
            )
        }
        Mode::Companion => draw_companion_menu(game, selected),
        Mode::Fuse => draw_fuse_menu(game, selected),
        Mode::FuseSecond => draw_fuse_second_menu(game, app.pending_fuse_first, selected),
        Mode::FuseName => draw_fuse_name_menu(
            game,
            app.pending_fuse_first,
            app.pending_fuse_second,
            &app.fuse_name_input,
        ),
        Mode::Trade => draw_trade_menu(game, selected),
        Mode::TradeAction => draw_trade_action_menu(game, app.pending_trade_structure, selected),
        Mode::TradeQuantity => draw_trade_quantity_menu(
            game,
            app.pending_trade_structure,
            app.pending_trade_choice,
            &app.trade_quantity_input,
        ),
        Mode::Perks => draw_perks_menu(game, selected),
        _ => {}
    }
}

fn draw_direction_prompt(title: &str, body: &str) {
    draw_popup(title, PopupSize::Small, &[text_row(body)]);
}

fn draw_build_menu(game: &mut Game, selected: usize) {
    let status = game.player_status();
    let defs = game.structure_defs();
    let mut rows = vec![text_row("Esc to cancel; Up/Down + Enter also work"), text_row("")];
    for (i, def) in defs.iter().enumerate() {
        let raw_cost = game.structure_build_cost(def);
        let cost = cost_display(&raw_cost, &status.inventory);
        rows.push(item_row(format!("[{}] {} - {}", i + 1, def.name, cost.join(", ")), i == selected));
        rows.push(text_row(format!("    {}", structure_description(def))));
    }
    draw_popup("Deploy", PopupSize::Large, &rows);
}

fn structure_description(def: &feral_processes_engine::structures::StructureDef) -> String {
    let mut parts = Vec::new();
    if let Some(work) = &def.work {
        parts.push(format!("cronjob -> {}", work.produces.display_name()));
    }
    if let Some(passive) = &def.passive_process {
        parts.push(format!("{} -> {}", passive.consumes.display_name(), passive.produces.display_name()));
    }
    if parts.is_empty() {
        parts.push("no production".to_string());
    }
    parts.join(", ")
}

fn draw_craft_menu(game: &mut Game, selected: usize) {
    let status = game.player_status();
    let recipes = game.craft_recipes();
    let mut rows = vec![text_row("Esc to cancel; Up/Down + Enter also work"), text_row("")];
    for (i, recipe) in recipes.iter().enumerate() {
        let cost = cost_display(&recipe.cost, &status.inventory);
        rows.push(item_row(
            format!("[{}] {} - {}", i + 1, recipe.result.display_name(), cost.join(", ")),
            i == selected,
        ));
    }
    draw_popup("Compile", PopupSize::Large, &rows);
}

fn draw_craft_quantity(game: &mut Game, pending: Option<ItemId>, quantity_input: &str) {
    let Some(result) = pending else { return };
    let status = game.player_status();
    let recipe = game.craft_recipes().into_iter().find(|r| r.result == result);
    let mut rows = vec![text_row(format!("Compile how many {}?", result.display_name())), text_row("")];
    if let Some(recipe) = &recipe {
        let cost = cost_display(&recipe.cost, &status.inventory);
        rows.push(text_row(format!("Cost per unit: {}", cost.join(", "))));
        rows.push(text_row(""));
    }
    let shown = if quantity_input.is_empty() { "1" } else { quantity_input };
    rows.push(text_row(format!("Quantity: {shown}")));
    rows.push(text_row(""));
    rows.push(text_row(format!("Max affordable right now: {}", game.max_craftable(result))));
    rows.push(text_row(""));
    rows.push(text_row("Type digits, Enter to compile"));
    rows.push(text_row("[F] Compile 5   [M] Compile max affordable   Esc to go back"));
    draw_popup("Compile", PopupSize::Large, &rows);
}

fn draw_worker_menu(game: &mut Game, title: &str, prompt: &str, selected: usize) {
    let workers: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.is_tamed)
        .collect();
    // `view_entities` doesn't carry a raw power number, only a level and
    // an HP fraction — cross-reference `owned_pets` for it, same as the
    // fuse menu does.
    let pets = game.owned_pets();
    let mut rows = vec![text_row(format!("{prompt} (Esc to cancel; Up/Down + Enter also work)"))];
    if workers.is_empty() {
        rows.push(text_row("(no compiled programs nearby)"));
    }
    for (i, w) in workers.iter().enumerate() {
        let companion = if w.is_companion { " (in party)" } else { "" };
        let job = w.job_structure.as_ref().map(|s| format!(" (on a cronjob: {s})")).unwrap_or_default();
        let power = pets.iter().find(|p| p.entity == w.entity).map(|p| format!(" PWR {}", p.power)).unwrap_or_default();
        rows.push(item_row(
            format!(
                "[{}] {}{}{} at ({}, {}){}{}",
                i + 1,
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
    draw_popup(title, PopupSize::Large, &rows);
}

fn draw_structure_menu(game: &mut Game, title: &str, prompt: &str, workable_only: bool, selected: usize) {
    let structures: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| if workable_only { e.can_work } else { e.is_structure })
        .collect();
    let mut rows = vec![text_row(format!("{prompt} (Esc to cancel; Up/Down + Enter also work)"))];
    if structures.is_empty() {
        rows.push(text_row("(no structures nearby)"));
    }
    for (i, s) in structures.iter().enumerate() {
        let assigned = s.structure_worker.as_ref().map(|w| format!(" (assigned: {w})")).unwrap_or_default();
        let durability = s.durability.map(|(hp, max)| format!(" [HP {hp}/{max}]")).unwrap_or_default();
        rows.push(item_row(
            format!("[{}] {} at ({}, {}){}{}", i + 1, s.label, s.pos.0, s.pos.1, durability, assigned),
            i == selected,
        ));
    }
    draw_popup(title, PopupSize::Large, &rows);
}

fn draw_symlink_menu(game: &mut Game, selected: usize) {
    let status = game.player_status();
    let targets = game.symlink_targets();
    let mut rows = vec![text_row("Use symlink to which structure? (Esc to cancel; Up/Down + Enter also work)")];
    if targets.is_empty() {
        rows.push(text_row("(no symlink-capable structures deployed yet)"));
    }
    for (i, t) in targets.iter().enumerate() {
        let raw_cost = game.symlink_cost(t.entity).unwrap_or_default();
        let cost = cost_display(&raw_cost, &status.inventory);
        let durability = t.durability.map(|(hp, max)| format!(" [HP {hp}/{max}]")).unwrap_or_default();
        rows.push(item_row(
            format!("[{}] {} at ({}, {}){} - {}", i + 1, t.label, t.pos.0, t.pos.1, durability, cost.join(", ")),
            i == selected,
        ));
    }
    draw_popup("Symlink", PopupSize::Large, &rows);
}

fn draw_inspect_detail(game: &mut Game, entity: Option<Entity>) {
    let Some(view) = entity.and_then(|e| game.inspect(e)) else {
        draw_popup("Inspect", PopupSize::Small, &[text_row("That program is gone. Press any key to go back.")]);
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
    let moves: Vec<String> = view.moves.iter().map(|m| format!("{} (pow {})", m.name, m.power)).collect();

    let mut rows = vec![
        Row::TextColored(
            format!(
                "{}{}{}",
                view.name,
                view.level.map(|l| format!(" - Lv{l}")).unwrap_or_default(),
                if view.is_boss { " [BOSS]" } else { "" }
            ),
            if view.is_boss { RED } else { WHITE },
        ),
        text_row(format!("Status: {status}")),
        text_row(format!("Integrity: {}/{}", view.hp, view.max_hp)),
        text_row(format!("Attack {}   Defense {}   Power {}", view.atk, view.def, view.power)),
        text_row(format!("Decompile difficulty: {:.0}%", view.taming_difficulty * 100.0)),
    ];
    if view.is_hostile && !view.is_tamed {
        rows.push(Row::TextColored(
            format!("Decompile chance right now: {:.0}%", view.decompile_chance * 100.0),
            MAGENTA,
        ));
    }
    rows.push(text_row(format!(
        "Habitats: {}",
        if habitats.is_empty() { "unknown".to_string() } else { habitats.join(", ") }
    )));
    rows.push(text_row(format!(
        "Moves: {}",
        if moves.is_empty() { "none".to_string() } else { moves.join(", ") }
    )));
    if let Some(res) = view.work_resource {
        rows.push(text_row(format!("Work aptitude: {}", res.display_name())));
    }
    rows.push(text_row(""));
    rows.push(text_row("Press any key to go back, Esc to close"));
    draw_popup("Inspect", PopupSize::Large, &rows);
}

fn draw_inventory(game: &mut Game, selected: usize) {
    let status = game.player_status();
    let mut rows = vec![
        Row::TextColored(
            format!(
                "Level {}   Attack {}   Defense {}   Power {}   Decompiler {}",
                status.level, status.atk, status.def, status.power, status.decompiler
            ),
            CYAN,
        ),
        text_row(""),
        text_row("Equipped (number to unequip):"),
        equipped_row(1, "Weapon", status.weapon, selected == 0),
        equipped_row(2, "Armor", status.armor, selected == 1),
        equipped_row(3, "Module", status.module, selected == 2),
        text_row(""),
        text_row("Inventory (number to equip/erase):"),
    ];
    if status.inventory.is_empty() {
        rows.push(text_row("(empty)"));
    }
    for (i, (item, qty)) in status.inventory.iter().enumerate() {
        let tag = equip_preview_tag(*item, status.zone, game.item_fusion_tier(*item));
        rows.push(item_row(format!("[{}] {} x{}{}", i + 4, item.display_name(), qty, tag), selected == i + 3));
    }
    rows.push(text_row(""));
    rows.push(text_row("Esc to close; Up/Down + Enter also work"));
    draw_popup("Inventory", PopupSize::Large, &rows);
}

/// Formats an equippable item's stat bonus as it would be *if equipped
/// right now* — gear scales with the current zone level at the moment you
/// equip it (see `Game::equip`), so this previews that same number rather
/// than a flat, unscaled base value. Empty string for a non-equippable
/// item (in place of the old generic "(equippable)" tag).
fn equip_preview_tag(item: ItemId, zone_level: u32, fusion_tier: u32) -> String {
    let Some((_, base_mods)) = item.equipment() else {
        return String::new();
    };
    let mods = base_mods.scaled_for_level(zone_level).fused_for_tier(fusion_tier);
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

fn equipped_row(num: usize, label: &str, equipped: Option<feral_processes_engine::components::EquippedItem>, selected: bool) -> Row {
    match equipped.and_then(|e| e.item.equipment().map(|(_, mods)| (e, mods))) {
        Some((equipped, mods)) => {
            let mods = mods.scaled_for_level(equipped.level).fused_for_tier(equipped.fusion_tier);
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
            let note = if notes.is_empty() { String::new() } else { format!(" {}", notes.join(" ")) };
            item_row(
                format!("[{num}] {label}: {}{note} ({})", equipped.item.display_name(), parts.join(" ")),
                selected,
            )
        }
        None => item_row(format!("[{num}] {label}: (empty)"), selected),
    }
}

fn draw_inventory_item_action(item: Option<ItemId>, zone_level: u32, stack_qty: u32, fusion_tier: u32, selected: usize) {
    let Some(item) = item else {
        draw_popup("Item", PopupSize::Small, &[text_row("Nothing selected.")]);
        return;
    };
    let mut actions = vec!["[X] Erase".to_string()];
    if item.equipment().is_some() {
        if stack_qty >= feral_processes_engine::items::ITEM_FUSION_COST {
            actions.insert(0, "[U] Fuse (2 -> +10% bonus)".to_string());
        }
        actions.insert(0, "[E]quip".to_string());
    }
    let title = format!("{}{}", item.display_name(), equip_preview_tag(item, zone_level, fusion_tier));
    let mut rows = vec![Row::TextColored(title, TEXT), text_row("")];
    for (i, action) in actions.iter().enumerate() {
        rows.push(item_row(action.clone(), i == selected));
    }
    rows.push(text_row(""));
    rows.push(text_row("Esc to cancel; Up/Down + Enter also work"));
    draw_popup("Item", PopupSize::Large, &rows);
}

fn draw_companion_menu(game: &mut Game, selected: usize) {
    let pets = game.owned_pets();
    let mut rows = vec![text_row(
        "Pick a program to add to your party (max 3) - select a party member's own number to stand it down.",
    )];
    if pets.is_empty() {
        rows.push(text_row("(you don't have any compiled programs yet)"));
    }
    for (i, p) in pets.iter().enumerate() {
        let active = if p.is_companion { " (in party)" } else { "" };
        let job = p.job_structure.as_ref().map(|s| format!(" (on a cronjob: {s})")).unwrap_or_default();
        rows.push(item_row(
            format!(
                "[{}] {} Lv{} - HP {}/{}  ATK {}  DEF {}  PWR {}{}{}",
                i + 1,
                p.name,
                p.level,
                p.hp,
                p.max_hp,
                p.atk,
                p.def,
                p.power,
                active,
                job
            ),
            i == selected,
        ));
    }
    draw_popup("Party", PopupSize::Large, &rows);
}

/// Formats one fuse-candidate row with its full stat line, cross-
/// referencing `pets` (`Game::owned_pets`) by entity — `view_entities`
/// alone only carries a level and an HP fraction, not the raw HP/ATK/DEF/
/// PWR numbers a fusion decision actually depends on.
fn fuse_candidate_label(num: usize, c: &EntityView, pets: &[PetInfo]) -> String {
    match pets.iter().find(|p| p.entity == c.entity) {
        Some(p) => {
            let active = if p.is_companion { " (in party)" } else { "" };
            let job = p
                .job_structure
                .as_ref()
                .map(|s| format!(" (on a cronjob: {s})"))
                .unwrap_or_default();
            format!(
                "[{num}] {} Lv{} - HP {}/{}  ATK {}  DEF {}  PWR {}{active}{job}",
                c.label, p.level, p.hp, p.max_hp, p.atk, p.def, p.power
            )
        }
        None => format!("[{num}] {}{}", c.label, c.level.map(|l| format!(" Lv{l}")).unwrap_or_default()),
    }
}

fn draw_fuse_menu(game: &mut Game, selected: usize) {
    let candidates: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.is_tamed)
        .collect();
    let pets = game.owned_pets();
    let mut rows = vec![text_row("Fuse which program? Pick the first of two.")];
    if candidates.is_empty() {
        rows.push(text_row("(no compiled programs nearby)"));
    }
    for (i, c) in candidates.iter().enumerate() {
        rows.push(item_row(fuse_candidate_label(i + 1, c, &pets), i == selected));
    }
    draw_popup("Fuse", PopupSize::Large, &rows);
}

fn draw_fuse_second_menu(game: &mut Game, first: Option<Entity>, selected: usize) {
    let Some(first) = first else { return };
    let nearby = game.view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS);
    let first_label = nearby.iter().find(|e| e.entity == first).map(|e| e.label.clone()).unwrap_or_else(|| "it".to_string());
    let candidates: Vec<_> = nearby.into_iter().filter(|e| e.is_tamed && e.entity != first).collect();
    let pets = game.owned_pets();
    let mut rows = vec![text_row(format!("Fuse {first_label} with which program? Both are consumed."))];
    if candidates.is_empty() {
        rows.push(text_row("(no other compiled programs nearby)"));
    }
    for (i, c) in candidates.iter().enumerate() {
        rows.push(item_row(fuse_candidate_label(i + 1, c, &pets), i == selected));
    }
    draw_popup("Fuse", PopupSize::Large, &rows);
}

/// Free-text naming page shown after both fuse candidates are picked.
/// Blank and Enter keeps the default species name.
fn draw_fuse_name_menu(game: &mut Game, first: Option<Entity>, second: Option<Entity>, name_input: &str) {
    let (Some(first), Some(second)) = (first, second) else { return };
    let nearby = game.view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS);
    let label_of = |e: Entity| {
        nearby
            .iter()
            .find(|ev| ev.entity == e)
            .map(|ev| ev.label.clone())
            .unwrap_or_else(|| "it".to_string())
    };
    let rows = vec![
        text_row(format!("Fusing {} and {}.", label_of(first), label_of(second))),
        text_row(""),
        item_row(format!("Name it (optional, {} max): {name_input}", feral_processes_engine::MAX_CUSTOM_NAME_LEN), true),
        text_row(""),
        text_row("Type a name, Enter to fuse (blank keeps the default name)"),
        text_row("Esc to go back and re-pick the second program"),
    ];
    draw_popup("Fuse", PopupSize::Small, &rows);
}

fn draw_trade_menu(game: &mut Game, selected: usize) {
    let structures: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.can_trade)
        .collect();
    let mut rows = vec![text_row("Trade with which structure?")];
    if structures.is_empty() {
        rows.push(text_row("(no trading posts nearby)"));
    }
    for (i, s) in structures.iter().enumerate() {
        let durability = s.durability.map(|(hp, max)| format!(" [HP {hp}/{max}]")).unwrap_or_default();
        rows.push(item_row(format!("[{}] {} at ({}, {}){}", i + 1, s.label, s.pos.0, s.pos.1, durability), i == selected));
    }
    draw_popup("Trade", PopupSize::Large, &rows);
}

fn draw_trade_action_menu(game: &mut Game, structure: Option<Entity>, selected: usize) {
    let Some(structure) = structure else { return };
    let Some(trade) = game.trade_options(structure) else { return };
    let inventory = game.player_status().inventory;

    let mut rows = vec![Row::TextColored("Sell (from inventory):".to_string(), TEXT)];
    let sellable: Vec<_> = inventory.iter().filter(|(item, _)| *item != ItemId::CoreFragment).collect();
    if sellable.is_empty() {
        rows.push(text_row("(nothing to sell)"));
    }
    let mut idx = 0;
    for (item, qty) in &sellable {
        rows.push(item_row(
            format!("[{}] Sell {} x{qty} ({} Core Fragments each)", idx + 1, item.display_name(), trade.sell_rate),
            idx == selected,
        ));
        idx += 1;
    }
    rows.push(text_row(""));
    rows.push(Row::TextColored("Buy:".to_string(), TEXT));
    for (item, cost) in &trade.buy {
        rows.push(item_row(format!("[{}] Buy {} ({cost} Core Fragments each)", idx + 1, item.display_name()), idx == selected));
        idx += 1;
    }
    rows.push(text_row(""));
    rows.push(text_row("Esc to cancel; Up/Down + Enter also work"));
    draw_popup("Trade", PopupSize::Large, &rows);
}

fn draw_trade_quantity_menu(game: &mut Game, structure: Option<Entity>, choice: Option<TradeChoice>, quantity_input: &str) {
    let (Some(structure), Some(choice)) = (structure, choice) else { return };
    let Some(trade) = game.trade_options(structure) else { return };
    let (verb, item, unit_price) = match choice {
        TradeChoice::Sell(item) => ("Sell", item, trade.sell_rate),
        TradeChoice::Buy(item) => {
            let price = trade.buy.iter().find(|(i, _)| *i == item).map(|(_, c)| *c).unwrap_or(0);
            ("Buy", item, price)
        }
    };
    let shown = if quantity_input.is_empty() { "1" } else { quantity_input };
    let rows = vec![
        text_row(format!("{verb} how many {}?", item.display_name())),
        text_row(""),
        text_row(format!("Price: {unit_price} Core Fragments each")),
        text_row(""),
        text_row(format!("Quantity: {shown}")),
        text_row(""),
        text_row(format!("Type digits, Enter to {}, Esc to go back", verb.to_lowercase())),
    ];
    draw_popup("Trade", PopupSize::Large, &rows);
}

fn draw_perks_menu(game: &mut Game, selected: usize) {
    let status = game.player_status();
    let mut rows = vec![
        Row::TextColored(format!("Perk Points: {}", status.perk_points), CYAN),
        text_row(""),
    ];
    for (i, perk) in feral_processes_engine::Perk::all().iter().enumerate() {
        let level = status.unlocked_perks.iter().filter(|p| *p == perk).count();
        let tag = if level > 0 { format!(" (level {level})") } else { String::new() };
        rows.push(item_row(
            format!("[{}] {} - {} Perk Points{}", i + 1, perk.display_name(), perk.cost(), tag),
            i == selected,
        ));
        rows.push(text_row(format!("    {}", perk.description())));
    }
    rows.push(text_row(""));
    rows.push(text_row("Pick a number to buy another level. Esc to close"));
    draw_popup("Perks", PopupSize::Large, &rows);
}

fn status_tag(status: &Option<String>) -> String {
    status.as_ref().map(|s| format!(" [{s}]")).unwrap_or_default()
}

fn draw_battle(app: &mut App) {
    let Some(game) = &mut app.game else { return };
    let Some(view) = game.battle_view() else { return };

    let w = screen_width();
    let mut y = 20.0;
    y = draw_bar(
        20.0,
        y,
        w - 40.0,
        &format!(
            "{}{}{} (ATK {} / DEF {} / PWR {})",
            view.wild_name,
            if view.wild_is_boss { " [BOSS]" } else { "" },
            status_tag(&view.wild_status_effect),
            view.wild_atk,
            view.wild_def,
            view.wild_power
        ),
        view.wild_hp as f32,
        view.wild_max_hp.max(1) as f32,
        RED,
    );
    y += 10.0;
    y = draw_bar(
        20.0,
        y,
        w - 40.0,
        &format!(
            "You{} (ATK {} / DEF {} / PWR {} / DECOMP {})",
            status_tag(&view.player_status_effect),
            view.player_atk,
            view.player_def,
            view.player_power,
            view.player_decompiler
        ),
        view.player_hp as f32,
        view.player_max_hp.max(1) as f32,
        CYAN,
    );
    y += 10.0;

    for companion in &view.companions {
        draw_text(
            &format!(
                "Companion: {} (HP {}/{}, ATK {}, PWR {}){}",
                companion.name, companion.hp, companion.max_hp, companion.atk, companion.power, status_tag(&companion.status)
            ),
            20.0,
            y,
            FONT_SIZE,
            GREEN,
        );
        y += LINE_HEIGHT;
    }

    draw_text(
        &format!(
            "Decompile chance right now: {:.0}%{}",
            view.decompile_chance * 100.0,
            if view.can_tame { "" } else { " (needs an ICE Breaker)" }
        ),
        20.0,
        y,
        FONT_SIZE,
        MAGENTA,
    );
    y += LINE_HEIGHT + 10.0;

    let log_bottom = screen_height() - 60.0;
    draw_rectangle(20.0, y, w - 40.0, log_bottom - y, PANEL_BG);
    draw_rectangle_lines(20.0, y, w - 40.0, log_bottom - y, 2.0, BORDER);
    let capacity = (((log_bottom - y) - 20.0) / LINE_HEIGHT).max(1.0) as usize;
    let mut ly = y + 20.0;
    for line in game.message_log(capacity) {
        draw_text(&line, 30.0, ly, FONT_SIZE, TEXT_DIM);
        ly += LINE_HEIGHT;
    }

    let mut actions = vec!["[A]ttack".to_string()];
    if view.can_tame {
        actions.push("[D]ecompile".to_string());
    }
    if !view.companions.is_empty() {
        actions.push("[C]ommand companion".to_string());
    }
    actions.push("[J]ack Out".to_string());
    draw_text(&actions.join("   "), 20.0, screen_height() - 24.0, FONT_SIZE, TEXT);
}

fn draw_battle_companion_menu(app: &mut App) {
    let selected = app.menu_selected;
    let Some(game) = &mut app.game else { return };
    let party = game.player_status().companions;
    let mut rows = vec![text_row("Command which companion? It'll buff you instead of attacking.")];
    for (i, c) in party.iter().enumerate() {
        rows.push(item_row(
            format!("[{}] {} (HP {}/{}, ATK {}, PWR {}){}", i + 1, c.name, c.hp, c.max_hp, c.atk, c.power, status_tag(&c.status)),
            i == selected,
        ));
        rows.push(text_row(format!("    {}", c.ability)));
    }
    draw_popup("Command Companion", PopupSize::Large, &rows);
}

fn draw_main_menu(app: &App) {
    let mut options = vec!["[N] New Game".to_string()];
    if !app.list_saves().is_empty() {
        options.push("[L] Load Game".to_string());
    }
    options.push("[Q] Quit".to_string());
    let mut rows = vec![
        Row::TextColored("feral-processes".to_string(), TEXT),
        Row::TextColored("// jack into the Grid".to_string(), CYAN),
        text_row(""),
    ];
    for (i, opt) in options.iter().enumerate() {
        rows.push(item_row(opt.clone(), i == app.menu_selected));
    }
    if let Some(s) = &app.status_line {
        rows.push(text_row(""));
        rows.push(Row::TextColored(s.clone(), RED));
    }
    draw_popup("Main Menu", PopupSize::Large, &rows);
}

fn draw_load_game(app: &App) {
    let saves = app.list_saves();
    let mut rows = vec![text_row("Pick a save (Esc to cancel; Up/Down + Enter also work)")];
    if saves.is_empty() {
        rows.push(text_row("(no saves found)"));
    }
    for (i, save) in saves.iter().enumerate() {
        let summary = save.summary.as_deref().unwrap_or("(incompatible save - can still be deleted)");
        rows.push(item_row(format!("[{}] {} - {}", i + 1, save.name, summary), i == app.menu_selected));
    }
    draw_popup("Load Game", PopupSize::Large, &rows);
}

fn draw_save_action(app: &App) {
    let name = app
        .pending_save
        .as_ref()
        .and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "(unknown save)".to_string());
    let mut rows = vec![
        Row::TextColored(name, TEXT),
        text_row(""),
        item_row("[L]oad".to_string(), app.menu_selected == 0),
        item_row("[X] Delete".to_string(), app.menu_selected == 1),
        text_row(""),
        text_row("Esc to cancel; Up/Down + Enter also work"),
    ];
    if let Some(s) = &app.status_line {
        rows.push(text_row(""));
        rows.push(Row::TextColored(s.clone(), RED));
    }
    draw_popup("Save", PopupSize::Large, &rows);
}

fn draw_difficulty_pick(selected: usize) {
    let rows = vec![
        item_row("[P] Permadeath - flatlining is final; the session is archived to a log".to_string(), selected == 0),
        item_row("[F] Forgiving - flatlining costs you, but you reboot and keep going".to_string(), selected == 1),
        text_row(""),
        text_row("Esc to go back; Up/Down + Enter also work"),
    ];
    draw_popup("New Game", PopupSize::Large, &rows);
}

fn draw_game_over(app: &mut App) {
    let summary = app.game.as_mut().and_then(|g| g.history_summary()).unwrap_or_else(|| "Connection lost.".to_string());
    let rows = vec![
        Row::TextColored("FLATLINE".to_string(), RED),
        text_row(""),
        text_row(summary),
        text_row(""),
        text_row("Press any key to return to the main menu"),
    ];
    draw_popup("Session Terminated", PopupSize::Large, &rows);
}

fn draw_help() {
    let rows = vec![
        text_row("hjkl/arrows move   . wait   e drain   r recharge"),
        text_row("g scan   c compile   b deploy   w cronjob   G guard"),
        text_row("u symlink   i inspect   v inventory   p companions"),
        text_row("f fuse   t trade   x perks   s save   q main menu"),
        text_row("+/- zoom   [/] volume"),
        text_row(""),
        text_row("Every numbered menu also takes Up/Down + Enter, on top of"),
        text_row("typing a row's own number/letter directly."),
        text_row(""),
        text_row("In an intrusion:  a attack   d decompile   c command companion   j jack out"),
        text_row(""),
        text_row("Press any key to close"),
    ];
    draw_popup("Help", PopupSize::Large, &rows);
}
