//! All drawing for the graphics frontend: one screen per `Mode`, laid out
//! with macroquad's immediate-mode primitives (filled rects for bars and
//! tiles, drawn text for menus). Reads engine data through `App` and never
//! touches the ECS `World`.

use macroquad::prelude::*;

use crate::fx::Fx;
use crate::text::{Fonts, Metrics, map_cell, terrain_color, ui_metrics};
use feral_processes_app_core::{
    App, MENU_SCAN_RADIUS, Mode, TradeChoice, equip_preview_tag, inventory_item_actions,
    menu_shortcut,
};
use feral_processes_engine::components::GlyphColor;
use feral_processes_engine::items::ItemId;
use feral_processes_engine::world::Biome;
use feral_processes_engine::{
    Entity, EntityView, Game, MAX_FUSIONS, MessageKind, PetInfo, ProgramSaleOption, ResearchState,
};

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
const ORANGE: Color = Color::new(0.95, 0.55, 0.15, 1.0);

/// Offset that keeps party-slot bar keys clear of the enemy-group keys they
/// share `Fx::bar_ghost`'s map with. Far above `MAX_ENEMY_GROUPS`, so the
/// two ranges can never meet.
const PARTY_BAR_KEY_BASE: u64 = 1000;

/// How far toward grey a back-rank group's bar is pulled — enough to read
/// as "can't reach you" beside an engaged group without becoming
/// unreadable.
const BACK_RANK_DESATURATION: f32 = 0.55;

/// Pulls `color` toward its own grey, for drawing something that's present
/// but not currently in play.
fn desaturate(color: Color) -> Color {
    let grey = (color.r + color.g + color.b) / 3.0;
    let mix = |c: f32| c + (grey - c) * BACK_RANK_DESATURATION;
    Color::new(mix(color.r), mix(color.g), mix(color.b), color.a)
}

/// Display styling for a message-log line, chosen by the engine-supplied
/// `MessageKind` rather than by sniffing the text — low-priority chatter
/// stays dim, gains/damage that matter get a color.
fn draw_message_line(kind: MessageKind, text: &str, x: f32, y: f32, fonts: &Fonts, m: &Metrics) {
    let color = match kind {
        MessageKind::Info => TEXT_DIM,
        MessageKind::Loot => GREEN,
        MessageKind::LevelUp => GREEN,
        MessageKind::Raid => ORANGE,
        MessageKind::Round => TEXT_DIM,
    };
    if kind == MessageKind::LevelUp {
        fonts.ui_bold(text, x, y, m.font_size, color);
    } else {
        fonts.ui(text, x, y, m.font_size, color);
    }
}

/// Whether `mode` needs `App::status_line` redrawn on top of whatever it
/// just drew. `Playing` already shows it in the log pane, and the main-menu
/// and save popups carry it as a row inside the panel; every other mode
/// covers the log pane with a popup, which would otherwise bury the one
/// message explaining why a menu pick was refused.
fn needs_status_banner(mode: Mode) -> bool {
    !matches!(mode, Mode::Playing | Mode::MainMenu | Mode::SaveAction)
}

/// Draws `status` in a strip along the bottom edge, below every popup —
/// `draw_popup` caps a panel at 85% of the window height and centers it, so
/// the bottom 7.5% is always clear.
fn draw_status_banner(status: &str, fonts: &Fonts, m: &Metrics) {
    let dims = fonts.measure_ui(status, m.font_size);
    let baseline = screen_height() - m.pad;
    draw_rectangle(
        0.0,
        baseline - dims.height - m.pad / 2.0,
        screen_width(),
        dims.height + m.pad,
        PANEL_BG,
    );
    fonts.ui(status, m.inset, baseline, m.font_size, RED);
}

pub fn draw(app: &mut App, fx: &mut Fx, fonts: &Fonts) {
    let m = ui_metrics(screen_height());
    clear_background(Color::new(0.02, 0.02, 0.03, 1.0));
    match app.mode {
        Mode::MainMenu => draw_main_menu(app, fonts, &m),
        Mode::LoadGame => draw_load_game(app, fonts, &m),
        Mode::SaveAction => draw_save_action(app, fonts, &m),
        Mode::DifficultyPick => draw_difficulty_pick(app.menu_selected, fonts, &m),
        Mode::GameOver => draw_game_over(app, fonts, &m),
        Mode::Battle => draw_battle(app, fx, fonts, &m),
        Mode::BattleTarget => {
            draw_battle(app, fx, fonts, &m);
            draw_battle_target_menu(app, fonts, &m);
        }
        Mode::BattleItem => {
            draw_battle(app, fx, fonts, &m);
            draw_battle_item_menu(app, fonts, &m);
        }
        Mode::BattleSpecial => {
            draw_battle(app, fx, fonts, &m);
            draw_battle_special_menu(app, fonts, &m);
        }
        Mode::BattleAlly => {
            draw_battle(app, fx, fonts, &m);
            draw_battle_ally_menu(app, fonts, &m);
        }
        Mode::Help => {
            draw_playing_base(app, fx, fonts, &m);
            draw_help(fonts, &m);
        }
        _ => {
            draw_playing_base(app, fx, fonts, &m);
            draw_mode_overlay(app, fonts, &m);
        }
    }
    if let Some(status) = &app.status_line
        && needs_status_banner(app.mode)
    {
        draw_status_banner(status, fonts, &m);
    }
}

/// One line of a popup's body. `Item` rows are the numbered/lettered
/// options a menu key press resolves to (see `App::selected_index`);
/// `Text` rows are just informational.
enum Row {
    Text(String),
    TextColored(String, Color),
    Item {
        text: String,
        selected: bool,
        /// Draws the row in the bold face when selected. Reserved for lists
        /// where the row is a *creature you are addressing* rather than a
        /// command you are picking — see `draw_battle_target_menu`.
        bold: bool,
    },
}

fn text_row(s: impl Into<String>) -> Row {
    Row::Text(s.into())
}

fn item_row(s: impl Into<String>, selected: bool) -> Row {
    Row::Item {
        text: s.into(),
        selected,
        bold: false,
    }
}

/// `item_row` for a list of creatures — see `Row::Item::bold`.
fn creature_row(s: impl Into<String>, selected: bool) -> Row {
    Row::Item {
        text: s.into(),
        selected,
        bold: true,
    }
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
fn draw_popup(title: &str, size: PopupSize, rows: &[Row], fonts: &Fonts, m: &Metrics) {
    let (pct_w, pct_h) = match size {
        PopupSize::Large => (0.88, 0.85),
        PopupSize::Small => (0.5, 0.85),
    };
    let w = screen_width() * pct_w;
    let h = (screen_height() * pct_h)
        .min(rows.len() as f32 * m.line_height + m.line_height * 2.0 + m.inset)
        .max(m.line_height * 4.0);
    let x = (screen_width() - w) / 2.0;
    let y = (screen_height() - h) / 2.0;

    draw_rectangle(x, y, w, h, PANEL_BG);
    draw_rectangle_lines(x, y, w, h, 2.0, BORDER);
    fonts.ui(
        title,
        x + m.font_size as f32 / 2.0,
        y + m.font_size as f32,
        m.title(),
        CYAN,
    );
    // Sits below the title's own size rather than a fixed offset, so a
    // larger font pushes the rule down instead of striking through it.
    let divider_y = y + m.title() as f32 + m.gap;
    let divider_inset = m.pad / 2.0;
    draw_line(
        x + divider_inset,
        divider_y,
        x + w - divider_inset,
        divider_y,
        1.0,
        BORDER,
    );

    let first_item = rows.iter().position(|r| matches!(r, Row::Item { .. }));
    let last_item = rows.iter().rposition(|r| matches!(r, Row::Item { .. }));
    let (header, body, footer): (&[Row], &[Row], &[Row]) = match (first_item, last_item) {
        (Some(first), Some(last)) => (&rows[..first], &rows[first..=last], &rows[last + 1..]),
        _ => (rows, &[], &[]),
    };

    let mut cy = y + m.line_height * 2.0;
    let max_y = y + h - m.inset;
    for row in header {
        cy = draw_row(row, x, w, cy, max_y, fonts, m);
    }

    let footer_h = footer.len() as f32 * m.line_height;
    let body_bottom = (max_y - footer_h).max(cy);
    let raw_capacity = ((body_bottom - cy) / m.line_height).floor().max(0.0) as usize;
    let scrolling = body.len() > raw_capacity;
    // Scrolling reserves one line above and below for "N more" indicators,
    // so the item rows themselves never get a partial cut-off line.
    let capacity = if scrolling {
        raw_capacity.saturating_sub(2).max(1)
    } else {
        raw_capacity
    };

    if !body.is_empty() {
        let selected_idx = body
            .iter()
            .position(|r| matches!(r, Row::Item { selected: true, .. }))
            .unwrap_or(0);
        let scroll_offset = if body.len() <= capacity {
            0
        } else {
            let max_offset = body.len() - capacity;
            selected_idx.saturating_sub(capacity / 2).min(max_offset)
        };

        if scrolling {
            let text = if scroll_offset > 0 {
                format!("↑ {scroll_offset} more above")
            } else {
                String::new()
            };
            fonts.ui(&text, x + m.pad, cy, m.small(), TEXT_DIM);
            cy += m.line_height;
        }

        let visible_end = (scroll_offset + capacity).min(body.len());
        for row in &body[scroll_offset..visible_end] {
            cy = draw_row(row, x, w, cy, max_y, fonts, m);
        }

        if scrolling {
            let below = body.len() - visible_end;
            let text = if below > 0 {
                format!("↓ {below} more below")
            } else {
                String::new()
            };
            fonts.ui(&text, x + m.pad, cy, m.small(), TEXT_DIM);
            cy += m.line_height;
        }
    }

    for row in footer {
        cy = draw_row(row, x, w, cy, max_y, fonts, m);
    }
}

/// Draws one popup row and returns the y coordinate for the next one.
/// `max_y` is a last-resort safety clamp — normal layout keeps every row
/// within bounds via `draw_popup`'s capacity accounting, so this only ever
/// bites if that accounting is off by a line.
fn draw_row(row: &Row, x: f32, w: f32, cy: f32, max_y: f32, fonts: &Fonts, m: &Metrics) -> f32 {
    if cy > max_y {
        return cy;
    }
    match row {
        Row::Text(s) => {
            fonts.ui(s, x + m.pad, cy, m.font_size, TEXT_DIM);
        }
        Row::TextColored(s, color) => {
            fonts.ui(s, x + m.pad, cy, m.font_size, *color);
        }
        Row::Item {
            text: s,
            selected,
            bold,
        } => {
            if *selected {
                // Anchored to the same `m.pad` the row text uses, so the
                // highlight keeps leading its text by one inset at every
                // font size instead of drifting left as the text grows.
                let bleed = m.pad - m.inset;
                draw_rectangle(
                    x + bleed,
                    cy - m.font_size as f32,
                    w - bleed * 2.0,
                    m.line_height,
                    SELECT_BG,
                );
            }
            let prefix = if *selected { "> " } else { "  " };
            let label = format!("{prefix}{s}");
            if *selected && *bold {
                fonts.ui_bold(label, x + m.pad, cy, m.font_size, TEXT);
            } else {
                fonts.ui(label, x + m.pad, cy, m.font_size, TEXT);
            }
        }
    }
    cy + m.line_height
}

/// Formats a `(item, quantity)` cost list, tagged `(have/need)` — same
/// convention as `ui.rs::cost_display`.
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
        Biome::Platform => ('_', DARKGRAY),
    }
}

/// The world grid, status panel, and message feed — the base layer shown
/// under `Mode::Playing` and every menu popup, same as `ui.rs::render_playing`.
fn draw_playing_base(app: &mut App, fx: &Fx, fonts: &Fonts, m: &Metrics) {
    let (tile_px, glyph_px) = map_cell(app.zoom);
    let status_line = app.status_line.clone();
    let Some(game) = &mut app.game else { return };

    let map_w = screen_width() * 0.7;
    let map_h = screen_height() * 0.72;
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
    let shield_outline = fx.shield_outline(game.raid_defense_active());

    draw_rectangle(0.0, 0.0, map_w, map_h, Color::new(0.03, 0.03, 0.05, 1.0));
    for (ry, row) in tiles.iter().enumerate() {
        for (rx, tile) in row.iter().enumerate() {
            let (mut ch, biome_color) = biome_style(tile.biome);
            let mut color = terrain_color(biome_color);
            // Background starts from the full-saturation biome color, not
            // `color` — unlike the entity branch below, terrain's tile
            // background is deliberately not desaturated, so bare ground
            // keeps its biome identity instead of the whole map going grey.
            let mut bg_source = biome_color;
            let px = rx as f32 * tile_px;
            let py = ry as f32 * tile_px;
            let mut staffed = false;
            let mut shielded = false;
            let mut critical = false;
            for ev in &entities {
                let erx = ev.pos.0 - status.position.0 + half_w;
                let ery = ev.pos.1 - status.position.1 + half_h;
                if erx == rx as i32 && ery == ry as i32 {
                    ch = ev.glyph;
                    color = glyph_color(ev.color);
                    staffed = ev.is_structure && ev.structure_worker.is_some();
                    // Structures wear their raid damage: the glyph dims as
                    // durability drops, and a nearly-destroyed one washes
                    // its tile red, so the base's condition reads at a
                    // glance instead of only from the inspect menu.
                    (color, critical) = fx.structure_condition(ev.durability, color);
                    // Background follows the damage-dimmed glyph colour, so a
                    // worn structure darkens its whole tile rather than just
                    // its glyph.
                    bg_source = color;
                    shielded = ev.is_structure;
                }
            }
            let mut bg = Color::new(
                bg_source.r * 0.18,
                bg_source.g * 0.18,
                bg_source.b * 0.18,
                1.0,
            );
            if critical {
                bg = Color::new((bg.r + 0.18).min(1.0), bg.g, bg.b, bg.a);
            }
            draw_rectangle(px, py, tile_px - 1.0, tile_px - 1.0, bg);
            let glyph = ch.to_string();
            let dims = fonts.measure_map(&glyph, glyph_px);
            let tx = px + (tile_px - dims.width) / 2.0;
            let ty = py + (tile_px + dims.height) / 2.0;
            fonts.map(&glyph, tx, ty, glyph_px, color);
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
            // The shield network is base-wide, not per-structure, so every
            // structure carries the same faint pulse while one is standing.
            // Drawn under the flash so a raid still reads on top of it.
            if let Some(pulse) = shield_outline.filter(|_| shielded) {
                draw_rectangle_lines(px, py, tile_px - 1.0, tile_px - 1.0, 2.0, pulse);
            }
            let world = (
                status.position.0 + rx as i32 - half_w,
                status.position.1 + ry as i32 - half_h,
            );
            if let Some(flash) = fx.tile_flash(world) {
                draw_rectangle(px, py, tile_px - 1.0, tile_px - 1.0, flash);
            }
        }
    }
    draw_rectangle_lines(0.0, 0.0, map_w, map_h, 2.0, BORDER);

    draw_status_panel(
        Rect::new(map_w, 0.0, screen_width() - map_w, map_h),
        &status,
        game,
        fonts,
        m,
    );

    let log_y = map_h;
    let log_h = screen_height() - map_h;
    draw_rectangle(0.0, log_y, screen_width(), log_h, PANEL_BG);
    draw_rectangle_lines(
        0.0,
        log_y,
        screen_width(),
        log_h,
        2.0,
        fx.log_border(BORDER),
    );
    let mut ly = log_y + m.inset + m.font_size as f32 / 2.0;
    if let Some(s) = &status_line {
        fonts.ui(s, m.inset, ly, m.font_size, RED);
        ly += m.line_height;
    }
    let capacity = ((log_h - m.line_height) / m.line_height).max(1.0) as usize;
    for (kind, line) in game.message_log(capacity) {
        if ly > screen_height() - m.gap {
            break;
        }
        draw_message_line(kind, &line, m.inset, ly, fonts, m);
        ly += m.line_height;
    }
}

fn draw_status_panel(
    rect: Rect,
    status: &feral_processes_engine::PlayerStatus,
    game: &Game,
    fonts: &Fonts,
    m: &Metrics,
) {
    let Rect { x, y, w, h } = rect;
    draw_rectangle(x, y, w, h, PANEL_BG);
    draw_rectangle_lines(x, y, w, h, 2.0, BORDER);

    // Clears the panel border by one inset, then drops to the first
    // baseline; both terms grow with the font the rows are drawn in.
    let mut cy = y + m.inset + m.font_size as f32 / 2.0;
    cy = draw_bar(
        BarGeometry {
            x: x + m.inset,
            y: cy,
            w: w - m.inset * 2.0,
        },
        &format!("Integrity {}/{}", status.hp, status.max_hp.max(1)),
        status.hp as f32,
        status.max_hp.max(1) as f32,
        BarStyle::plain(RED),
        fonts,
        m,
    );
    cy = draw_bar(
        BarGeometry {
            x: x + m.inset,
            y: cy,
            w: w - m.inset * 2.0,
        },
        &format!("Power {:.0}/100", status.hunger),
        status.hunger,
        100.0,
        BarStyle::plain(YELLOW),
        fonts,
        m,
    );
    cy = draw_bar(
        BarGeometry {
            x: x + m.inset,
            y: cy,
            w: w - m.inset * 2.0,
        },
        &format!("Fatigue {:.0}/100", status.fatigue),
        status.fatigue,
        100.0,
        BarStyle::plain(BLUE),
        fonts,
        m,
    );
    cy += m.gap;

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
        fonts.ui(line, x + m.inset, cy, m.font_size, TEXT);
        cy += m.line_height;
    }
    fonts.ui(
        format!(
            "Party: {}/{}",
            status.companions.len(),
            feral_processes_engine::resources::MAX_PARTY_SIZE
        ),
        x + m.inset,
        cy,
        m.font_size,
        GREEN,
    );
    cy += m.line_height;
    fonts.ui(
        format!("Pets: {}/{}", status.pet_count, status.pet_capacity),
        x + m.inset,
        cy,
        m.font_size,
        GREEN,
    );
    cy += m.line_height;
    for companion in &status.companions {
        fonts.ui(
            format!(
                "Companion: {} (HP {}/{}, PWR {})",
                companion.name, companion.hp, companion.max_hp, companion.power
            ),
            x + m.inset,
            cy,
            m.font_size,
            GREEN,
        );
        cy += m.line_height;
    }
    cy += m.gap;
    fonts.ui("Inventory:", x + m.inset, cy, m.font_size, TEXT);
    cy += m.line_height;
    if status.inventory.is_empty() {
        fonts.ui("(empty)", x + m.inset, cy, m.font_size, TEXT_DIM);
        cy += m.line_height;
    }
    let keys = [
        "hjkl/arrows move  . wait  e drain  r recharge",
        "g scan   c compile   b deploy   w cronjob  G guard  R demolish",
        "u symlink   i inspect   v inventory",
        "p companions  f fuse  t trade  x perks",
        "s save   q main menu   ? help   +/- zoom",
    ];
    let keys_line_height = m.line_height - m.gap;
    let keys_block_h = keys.len() as f32 * keys_line_height + m.inset;
    let keys_y = y + h - keys_block_h;

    for (item, qty) in &status.inventory {
        if cy > keys_y - m.line_height {
            break;
        }
        fonts.ui(
            format!("{} x{}", game.item_name(item), qty),
            x + m.inset,
            cy,
            m.font_size,
            TEXT_DIM,
        );
        cy += m.line_height;
    }

    let mut ky = keys_y;
    for k in keys {
        fonts.ui(k, x + m.inset, ky, m.small(), TEXT_DIM);
        ky += keys_line_height;
    }
}

/// Where a stat bar goes. `draw_bar` and the `draw_ghost_band` trailing it
/// take one of these rather than three loose floats, so the two can't drift
/// apart into a band that misses the bar it belongs to.
#[derive(Clone, Copy)]
struct BarGeometry {
    x: f32,
    y: f32,
    w: f32,
}

impl BarGeometry {
    /// Top of the track, shared so `draw_bar` and `draw_ghost_band` can't
    /// disagree about where it is.
    fn track_y(&self, m: &Metrics) -> f32 {
        self.y + m.gap
    }
}

/// A bar's track is a deliberate visual weight — a rule under the label,
/// not a block — so unlike the text it flanks it stays put as the UI font
/// scales.
const BAR_TRACK_H: f32 = 14.0;

/// How a bar is painted, as opposed to where `BarGeometry` puts it. Bundled
/// rather than passed loose because the two together push `draw_bar` past
/// clippy's argument threshold, same reasoning as `BarGeometry` itself.
#[derive(Clone, Copy)]
struct BarStyle {
    color: Color,
    /// Draws the label in the bold face — reserved for the party member
    /// currently choosing an action, so the row you're addressing wins
    /// against the colour the rest of the roster is already using.
    bold: bool,
}

impl BarStyle {
    fn plain(color: Color) -> Self {
        Self { color, bold: false }
    }
}

/// Draws a labeled bar (HP/Power/Fatigue) and returns the y coordinate for
/// whatever's drawn next.
fn draw_bar(
    g: BarGeometry,
    label: &str,
    value: f32,
    max: f32,
    style: BarStyle,
    fonts: &Fonts,
    m: &Metrics,
) -> f32 {
    let BarStyle { color, bold } = style;
    let ratio = (value / max).clamp(0.0, 1.0);
    // `label` is drawn exactly as given. This used to append
    // `" {value}/{max}"`, which pinned HP to the end of the text and made
    // the battle rosters' HP column impossible — a drawing primitive is the
    // wrong place to be deciding text layout.
    if bold {
        fonts.ui_bold(label, g.x, g.y, m.label(), TEXT);
    } else {
        fonts.ui(label, g.x, g.y, m.label(), TEXT);
    }
    let bar_y = g.track_y(m);
    draw_rectangle(
        g.x,
        bar_y,
        g.w,
        BAR_TRACK_H,
        Color::new(0.15, 0.15, 0.15, 1.0),
    );
    draw_rectangle(g.x, bar_y, g.w * ratio, BAR_TRACK_H, color);
    draw_rectangle_lines(g.x, bar_y, g.w, BAR_TRACK_H, 1.0, BORDER);
    // Leaves the next row's label room above its own baseline, so stacked
    // bars keep their spacing as the label grows.
    bar_y + BAR_TRACK_H + m.font_size as f32 / 2.0
}

/// How far one stacked bar row advances y, including the trailing `m.inset`
/// every caller adds between rows. Mirrors `draw_bar`'s return value, so the
/// two have to move together — it exists because `draw_battle` bottom-anchors
/// the party block and therefore has to know its height *before* drawing it.
fn bar_row_height(m: &Metrics) -> f32 {
    m.gap + BAR_TRACK_H + m.font_size as f32 / 2.0 + m.inset
}

/// A lagging "ghost" band trailing a bar's real value, so a hit in battle
/// reads as a visible drain rather than a jump. Call after `draw_bar` with
/// the same geometry — `draw_bar` lays down an opaque track that would
/// otherwise bury this — and it fills only the gap between the two values.
fn draw_ghost_band(g: BarGeometry, value: f32, ghost: f32, max: f32, color: Color, m: &Metrics) {
    let ratio = (value / max).clamp(0.0, 1.0);
    let ghost_ratio = (ghost / max).clamp(0.0, 1.0);
    if ghost_ratio <= ratio {
        return;
    }
    draw_rectangle(
        g.x + g.w * ratio,
        g.track_y(m),
        g.w * (ghost_ratio - ratio),
        BAR_TRACK_H,
        Color::new(color.r, color.g, color.b, 0.45),
    );
}

fn draw_mode_overlay(app: &mut App, fonts: &Fonts, m: &Metrics) {
    let selected = app.menu_selected;
    let Some(game) = &mut app.game else { return };
    match app.mode {
        Mode::Build => draw_build_menu(game, selected, fonts, m),
        Mode::BuildDirection => draw_direction_prompt(
            "Deploy Direction",
            "Choose a direction to deploy (arrows/hjkl), Esc to cancel",
            fonts,
            m,
        ),
        Mode::Craft => draw_craft_menu(game, selected, fonts, m),
        Mode::CraftQuantity => draw_craft_quantity(
            game,
            app.pending_craft.clone(),
            &app.craft_quantity_input,
            fonts,
            m,
        ),
        Mode::EraseQuantity => draw_erase_quantity(
            game,
            app.pending_erase.clone(),
            &app.erase_quantity_input,
            fonts,
            m,
        ),
        Mode::Cronjob => draw_worker_menu(
            game,
            "Assign Cronjob",
            "Assign which program to a cronjob?",
            selected,
            fonts,
            m,
        ),
        Mode::CronjobStructure => draw_structure_menu(
            game,
            "Assign Cronjob",
            "Cronjob which structure?",
            true,
            selected,
            fonts,
            m,
        ),
        Mode::Guard => draw_worker_menu(
            game,
            "Assign Guard",
            "Assign which program to guard duty?",
            selected,
            fonts,
            m,
        ),
        Mode::GuardStructure => draw_structure_menu(
            game,
            "Assign Guard",
            "Guard which structure? Any structure qualifies.",
            false,
            selected,
            fonts,
            m,
        ),
        Mode::Remove => draw_remove_menu(game, selected, fonts, m),
        Mode::RemoveConfirm => draw_remove_confirm(selected, fonts, m),
        Mode::Upgrade => draw_upgrade_menu(game, selected, fonts, m),
        Mode::Symlink => draw_symlink_menu(game, selected, fonts, m),
        Mode::InspectDirection => draw_direction_prompt(
            "Inspect Direction",
            "Choose a direction to inspect (arrows/hjkl), Esc to cancel",
            fonts,
            m,
        ),
        Mode::InspectDetail => draw_inspect_detail(game, app.pending_inspect, fonts, m),
        Mode::Inventory => draw_inventory(game, selected, fonts, m),
        Mode::InventoryItemAction => {
            let zone = game.player_status().zone;
            let fusion_tier = app
                .pending_inventory_item
                .as_ref()
                .map(|item| game.item_fusion_tier(item))
                .unwrap_or(0);
            draw_inventory_item_action(
                game,
                app.pending_inventory_item.clone(),
                zone,
                fusion_tier,
                selected,
                fonts,
                m,
            )
        }
        Mode::Companion => draw_companion_menu(game, selected, fonts, m),
        Mode::Fuse => draw_fuse_menu(game, selected, fonts, m),
        Mode::FuseSecond => draw_fuse_second_menu(game, app.pending_fuse_first, selected, fonts, m),
        Mode::FuseName => draw_fuse_name_menu(
            game,
            app.pending_fuse_first,
            app.pending_fuse_second,
            &app.fuse_name_input,
            fonts,
            m,
        ),
        Mode::Trade => draw_trade_menu(game, selected, fonts, m),
        Mode::TradeAction => {
            draw_trade_action_menu(game, app.pending_trade_structure, selected, fonts, m)
        }
        Mode::TradeQuantity => draw_trade_quantity_menu(
            game,
            app.pending_trade_structure,
            app.pending_trade_choice.clone(),
            &app.trade_quantity_input,
            fonts,
            m,
        ),
        Mode::TradeProgramConfirm => {
            draw_trade_program_confirm(app.pending_trade_program.as_ref(), fonts, m)
        }
        Mode::Perks => draw_perks_menu(game, selected, fonts, m),
        Mode::Research => draw_research_menu(game, selected, fonts, m),
        _ => {}
    }
}

fn draw_direction_prompt(title: &str, body: &str, fonts: &Fonts, m: &Metrics) {
    draw_popup(title, PopupSize::Small, &[text_row(body)], fonts, m);
}

fn draw_build_menu(game: &mut Game, selected: usize, fonts: &Fonts, m: &Metrics) {
    let status = game.player_status();
    let defs = game.buildable_structure_defs();
    let descriptions: Vec<String> = defs
        .iter()
        .map(|def| game.structure_description(def))
        .collect();
    let mut rows = vec![
        text_row("Esc to cancel; Up/Down + Enter also work"),
        text_row(""),
    ];
    for (i, def) in defs.iter().enumerate() {
        let raw_cost = game.structure_build_cost(def);
        let cost = cost_display(game, &raw_cost, &status.inventory);
        rows.push(item_row(
            format!("[{}] {} - {}", menu_shortcut(i), def.name, cost.join(", ")),
            i == selected,
        ));
        rows.push(text_row(format!("    {}", descriptions[i])));
    }
    draw_popup("Deploy", PopupSize::Large, &rows, fonts, m);
}

fn draw_craft_menu(game: &mut Game, selected: usize, fonts: &Fonts, m: &Metrics) {
    let status = game.player_status();
    let recipes = game.craft_recipes();
    let mut rows = vec![
        text_row("Esc to cancel; Up/Down + Enter also work"),
        text_row(""),
    ];
    for (i, recipe) in recipes.iter().enumerate() {
        let cost = cost_display(game, &recipe.cost, &status.inventory);
        let blurb = game
            .item_blurb(&recipe.result)
            .map(|b| format!(" ({b})"))
            .unwrap_or_default();
        rows.push(item_row(
            format!(
                "[{}] {}{} - {}",
                menu_shortcut(i),
                game.item_name(&recipe.result),
                blurb,
                cost.join(", ")
            ),
            i == selected,
        ));
    }
    draw_popup("Compile", PopupSize::Large, &rows, fonts, m);
}

fn draw_craft_quantity(
    game: &mut Game,
    pending: Option<ItemId>,
    quantity_input: &str,
    fonts: &Fonts,
    m: &Metrics,
) {
    let Some(result) = pending else { return };
    let status = game.player_status();
    let recipe = game
        .craft_recipes()
        .into_iter()
        .find(|r| r.result == result);
    let mut rows = vec![
        text_row(format!("Compile how many {}?", game.item_name(&result))),
        text_row(""),
    ];
    if let Some(recipe) = &recipe {
        let cost = cost_display(game, &recipe.cost, &status.inventory);
        rows.push(text_row(format!("Cost per unit: {}", cost.join(", "))));
        rows.push(text_row(""));
    }
    let shown = if quantity_input.is_empty() {
        "1"
    } else {
        quantity_input
    };
    rows.push(text_row(format!("Quantity: {shown}")));
    rows.push(text_row(""));
    rows.push(text_row(format!(
        "Max affordable right now: {}",
        game.max_craftable(&result)
    )));
    rows.push(text_row(""));
    rows.push(text_row("Type digits, Enter to compile"));
    rows.push(text_row(
        "[F] Compile 5   [M] Compile max affordable   Esc to go back",
    ));
    draw_popup("Compile", PopupSize::Large, &rows, fonts, m);
}

fn draw_erase_quantity(
    game: &mut Game,
    item: Option<ItemId>,
    quantity_input: &str,
    fonts: &Fonts,
    m: &Metrics,
) {
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
    let rows = vec![
        text_row(format!("Erase how many {}?", game.item_name(&item))),
        text_row(""),
        text_row(format!("Quantity: {shown}")),
        text_row(""),
        text_row(format!(
            "You have: {held}        Buffer: {}",
            status.inventory_used
        )),
        text_row(""),
        text_row("Type digits, Enter to erase"),
        text_row("[A] Erase all   Esc to go back"),
    ];
    draw_popup("Erase", PopupSize::Large, &rows, fonts, m);
}

fn draw_worker_menu(
    game: &mut Game,
    title: &str,
    prompt: &str,
    selected: usize,
    fonts: &Fonts,
    m: &Metrics,
) {
    let workers: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.is_tamed)
        .collect();
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
    draw_popup(title, PopupSize::Large, &rows, fonts, m);
}

fn draw_structure_menu(
    game: &mut Game,
    title: &str,
    prompt: &str,
    workable_only: bool,
    selected: usize,
    fonts: &Fonts,
    m: &Metrics,
) {
    let structures: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| {
            if workable_only {
                e.can_work
            } else {
                e.is_structure
            }
        })
        .collect();
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
    draw_popup(title, PopupSize::Large, &rows, fonts, m);
}

fn draw_remove_menu(game: &mut Game, selected: usize, fonts: &Fonts, m: &Metrics) {
    let structures: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.is_structure)
        .collect();
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
    draw_popup("Demolish Structure", PopupSize::Large, &rows, fonts, m);
}

fn draw_upgrade_menu(game: &mut Game, selected: usize, fonts: &Fonts, m: &Metrics) {
    let structures: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.is_structure && e.tier.is_some())
        .collect();
    let mut rows = vec![text_row(
        "Upgrade which structure? Each tier costs more and yields more. (Esc to cancel; Up/Down + Enter also work)",
    )];
    if structures.is_empty() {
        rows.push(text_row("(no upgradeable structures nearby)"));
    }
    for (i, s) in structures.iter().enumerate() {
        rows.push(item_row(
            format!(
                "[{}] {} at ({}, {}) [Mk{}]",
                menu_shortcut(i),
                s.label,
                s.pos.0,
                s.pos.1,
                s.tier.unwrap_or(1),
            ),
            i == selected,
        ));
    }
    draw_popup("Upgrade Structure", PopupSize::Large, &rows, fonts, m);
}

fn draw_remove_confirm(selected: usize, fonts: &Fonts, m: &Metrics) {
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
    draw_popup("Confirm Demolish Home", PopupSize::Small, &rows, fonts, m);
}

fn draw_symlink_menu(game: &mut Game, selected: usize, fonts: &Fonts, m: &Metrics) {
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
    draw_popup("Symlink", PopupSize::Large, &rows, fonts, m);
}

fn draw_inspect_detail(game: &mut Game, entity: Option<Entity>, fonts: &Fonts, m: &Metrics) {
    let Some(view) = entity.and_then(|e| game.inspect(e)) else {
        draw_popup(
            "Inspect",
            PopupSize::Small,
            &[text_row("That program is gone. Press any key to go back.")],
            fonts,
            m,
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
        text_row(format!(
            "Attack {}   Defense {}   Power {}",
            view.atk, view.def, view.power
        )),
        text_row(format!(
            "Decompile difficulty: {:.0}%",
            view.taming_difficulty * 100.0
        )),
    ];
    if let Some(quality) = &view.quality {
        rows.push(text_row(format!("Potential: {quality}")));
    }
    if view.fusions > 0 {
        rows.push(text_row(format!(
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
        rows.push(Row::TextColored(
            decompile_chance_line(view.decompile_chance),
            MAGENTA,
        ));
    }
    rows.push(text_row(format!(
        "Habitats: {}",
        if habitats.is_empty() {
            "unknown".to_string()
        } else {
            habitats.join(", ")
        }
    )));
    rows.push(text_row(format!(
        "Moves: {}",
        if moves.is_empty() {
            "none".to_string()
        } else {
            moves.join(", ")
        }
    )));
    if let Some(res) = view.work_resource {
        rows.push(text_row(format!("Work aptitude: {}", game.item_name(&res))));
    }
    rows.push(text_row(""));
    rows.push(text_row("Press any key to go back, Esc to close"));
    draw_popup("Inspect", PopupSize::Large, &rows, fonts, m);
}

fn draw_inventory(game: &mut Game, selected: usize, fonts: &Fonts, m: &Metrics) {
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
        equipped_row(1, "Weapon", status.weapon.clone(), selected == 0, game),
        equipped_row(2, "Armor", status.armor.clone(), selected == 1, game),
        equipped_row(3, "Module", status.module.clone(), selected == 2, game),
        text_row(""),
        text_row(format!(
            "Inventory - Buffer {} (row key to equip/fuse/erase):",
            status.inventory_used
        )),
    ];
    if status.inventory.is_empty() {
        rows.push(text_row("(empty)"));
    }
    for (i, (item, qty)) in status.inventory.iter().enumerate() {
        let fusion_tier = game.item_fusion_tier(item);
        let tag = equip_preview_tag(game, item, status.zone, fusion_tier);
        rows.push(item_row(
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
    rows.push(text_row(""));
    rows.push(text_row("Esc to close; Up/Down + Enter also work"));
    draw_popup("Inventory", PopupSize::Large, &rows, fonts, m);
}

fn equipped_row(
    num: usize,
    label: &str,
    equipped: Option<feral_processes_engine::components::EquippedItem>,
    selected: bool,
    game: &Game,
) -> Row {
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
            item_row(
                format!(
                    "[{num}] {label}: {}{note} ({})",
                    game.item_name(&equipped.item),
                    parts.join(" ")
                ),
                selected,
            )
        }
        None => item_row(format!("[{num}] {label}: (empty)"), selected),
    }
}

fn draw_inventory_item_action(
    game: &Game,
    item: Option<ItemId>,
    zone_level: u32,
    fusion_tier: u32,
    selected: usize,
    fonts: &Fonts,
    m: &Metrics,
) {
    let Some(item) = item else {
        draw_popup(
            "Item",
            PopupSize::Small,
            &[text_row("Nothing selected.")],
            fonts,
            m,
        );
        return;
    };
    let title = format!(
        "{}{}",
        game.item_name(&item),
        equip_preview_tag(game, &item, zone_level, fusion_tier)
    );
    let mut rows = vec![Row::TextColored(title, TEXT), text_row("")];
    for (i, (_, label)) in inventory_item_actions(game, &item).iter().enumerate() {
        rows.push(item_row(label.clone(), i == selected));
    }
    rows.push(text_row(""));
    rows.push(text_row("Esc to cancel; Up/Down + Enter also work"));
    draw_popup("Item", PopupSize::Large, &rows, fonts, m);
}

fn draw_companion_menu(game: &mut Game, selected: usize, fonts: &Fonts, m: &Metrics) {
    let pets = game.owned_pets();
    let mut rows = vec![text_row(
        "Pick a program to add to your party (max 3) - select a party member's own number to stand it down.",
    )];
    if pets.is_empty() {
        rows.push(text_row("(you don't have any compiled programs yet)"));
    }
    for (i, p) in pets.iter().enumerate() {
        let activity = activity_tag(&p.activity);
        let quality = p
            .quality
            .as_ref()
            .map(|q| format!(" [{q}]"))
            .unwrap_or_default();
        let fused = fusion_tag(p.fusions);
        rows.push(item_row(
            format!(
                "[{}] {} Lv{} - HP {}/{}  ATK {}  DEF {}  PWR {}{}{}{}",
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
                activity
            ),
            i == selected,
        ));
    }
    draw_popup("Party", PopupSize::Large, &rows, fonts, m);
}

/// Formats one fuse-candidate row with its full stat line, cross-
/// referencing `pets` (`Game::owned_pets`) by entity — `view_entities`
/// alone only carries a level and an HP fraction, not the raw HP/ATK/DEF/
/// PWR numbers a fusion decision actually depends on.
/// How a program's fusion depth reads in a menu row — nothing at all for
/// a program that's never been fused, a plain count while it still has
/// fusions left, and an explicit "maxed" note once it's hit
/// `MAX_FUSIONS` and can't be an input to another fusion.
fn fusion_tag(fusions: u32) -> String {
    match fusions {
        0 => String::new(),
        n if n >= MAX_FUSIONS => format!(" (fused {n}/{MAX_FUSIONS} - maxed)"),
        n => format!(" (fused {n}/{MAX_FUSIONS})"),
    }
}

fn fuse_candidate_label(num: char, c: &EntityView, pets: &[PetInfo]) -> String {
    let fused = fusion_tag(c.fusions);
    match pets.iter().find(|p| p.entity == c.entity) {
        Some(p) => {
            let activity = activity_tag(&p.activity);
            format!(
                "[{num}] {} Lv{} - HP {}/{}  ATK {}  DEF {}  PWR {}{fused}{activity}",
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

fn draw_fuse_menu(game: &mut Game, selected: usize, fonts: &Fonts, m: &Metrics) {
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
        rows.push(item_row(
            fuse_candidate_label(menu_shortcut(i), c, &pets),
            i == selected,
        ));
    }
    draw_popup("Fuse", PopupSize::Large, &rows, fonts, m);
}

fn draw_fuse_second_menu(
    game: &mut Game,
    first: Option<Entity>,
    selected: usize,
    fonts: &Fonts,
    m: &Metrics,
) {
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
    let mut rows = vec![text_row(format!(
        "Fuse {first_label} with which program? Both are consumed."
    ))];
    if candidates.is_empty() {
        rows.push(text_row("(no other compiled programs nearby)"));
    }
    for (i, c) in candidates.iter().enumerate() {
        rows.push(item_row(
            fuse_candidate_label(menu_shortcut(i), c, &pets),
            i == selected,
        ));
    }
    draw_popup("Fuse", PopupSize::Large, &rows, fonts, m);
}

/// Free-text naming page shown after both fuse candidates are picked.
/// Blank and Enter keeps the default species name.
fn draw_fuse_name_menu(
    game: &mut Game,
    first: Option<Entity>,
    second: Option<Entity>,
    name_input: &str,
    fonts: &Fonts,
    m: &Metrics,
) {
    let (Some(first), Some(second)) = (first, second) else {
        return;
    };
    let nearby = game.view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS);
    let label_of = |e: Entity| {
        nearby
            .iter()
            .find(|ev| ev.entity == e)
            .map(|ev| ev.label.clone())
            .unwrap_or_else(|| "it".to_string())
    };
    let rows = vec![
        text_row(format!(
            "Fusing {} and {}.",
            label_of(first),
            label_of(second)
        )),
        text_row(""),
        item_row(
            format!(
                "Name it (optional, {} max): {name_input}",
                feral_processes_engine::MAX_CUSTOM_NAME_LEN
            ),
            true,
        ),
        text_row(""),
        text_row("Type a name, Enter to fuse (blank keeps the default name)"),
        text_row("Esc to go back and re-pick the second program"),
    ];
    draw_popup("Fuse", PopupSize::Small, &rows, fonts, m);
}

fn draw_trade_menu(game: &mut Game, selected: usize, fonts: &Fonts, m: &Metrics) {
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
        let durability = s
            .durability
            .map(|(hp, max)| format!(" [HP {hp}/{max}]"))
            .unwrap_or_default();
        rows.push(item_row(
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
    draw_popup("Trade", PopupSize::Large, &rows, fonts, m);
}

fn draw_trade_action_menu(
    game: &mut Game,
    structure: Option<Entity>,
    selected: usize,
    fonts: &Fonts,
    m: &Metrics,
) {
    let Some(structure) = structure else { return };
    let Some(trade) = game.trade_options(structure) else {
        return;
    };
    let status = game.player_status();
    let inventory = status.inventory.clone();
    let currency = game.currency();

    let mut rows = vec![Row::TextColored("Sell (from inventory):".to_string(), TEXT)];
    let sellable: Vec<_> = inventory
        .iter()
        .filter(|(item, _)| *item != currency)
        .collect();
    if sellable.is_empty() {
        rows.push(text_row("(nothing to sell)"));
    }
    let mut idx = 0;
    for (item, qty) in &sellable {
        // Same tag the inventory shows, so what you're about to part with
        // reads identically on both screens — fusion tier included, since
        // that's exactly what you'd want to check before selling.
        let tag = equip_preview_tag(game, item, status.zone, game.item_fusion_tier(item));
        rows.push(item_row(
            format!(
                "[{}] Sell {} x{qty}{} ({} Core Fragments each)",
                menu_shortcut(idx),
                game.item_name(item),
                tag,
                trade.sell_rate
            ),
            idx == selected,
        ));
        idx += 1;
    }
    rows.push(text_row(""));
    rows.push(Row::TextColored("Buy:".to_string(), TEXT));
    for (item, cost) in &trade.buy {
        // Fusion tier 0: stock is unfused, so the tag shows what you'd get
        // buying it, not what some copy in your buffer happens to be.
        let tag = equip_preview_tag(game, item, status.zone, 0);
        rows.push(item_row(
            format!(
                "[{}] Buy {}{} ({cost} Core Fragments each)",
                menu_shortcut(idx),
                game.item_name(item),
                tag
            ),
            idx == selected,
        ));
        idx += 1;
    }
    // Only shown by a trader that buys programs — see
    // `TradeDef::program_sell_divisor`. Omitted entirely otherwise, rather
    // than shown empty, so an items-only trader's screen is unchanged.
    let programs = game.program_sale_options(structure);
    if !programs.is_empty() {
        rows.push(text_row(""));
        rows.push(Row::TextColored(
            "Sell programs (permanent):".to_string(),
            TEXT,
        ));
        for program in &programs {
            rows.push(item_row(
                format!(
                    "[{}] Sell {} Lv{} — power {} → {} Core Fragments{}",
                    menu_shortcut(idx),
                    program.name,
                    program.level,
                    program.power,
                    program.payout,
                    activity_tag(&program.activity),
                ),
                idx == selected,
            ));
            idx += 1;
        }
    }
    rows.push(text_row(""));
    rows.push(text_row("Esc to cancel; Up/Down + Enter also work"));
    draw_popup("Trade", PopupSize::Large, &rows, fonts, m);
}

/// Confirms a program sale. The only screen that says what else the sale
/// takes down, since selling detaches the program from its party slot,
/// cronjob or guard post without asking.
fn draw_trade_program_confirm(option: Option<&ProgramSaleOption>, fonts: &Fonts, m: &Metrics) {
    let Some(option) = option else { return };
    let mut rows = vec![
        text_row(format!(
            "Sell {} (Lv {}, power {}) for {} Core Fragments?",
            option.name, option.level, option.power, option.payout
        )),
        Row::TextColored(
            "This erases the program for good. It cannot be undone.".to_string(),
            RED,
        ),
    ];
    if !option.detaches.is_empty() {
        rows.push(text_row(""));
        for detached in &option.detaches {
            rows.push(Row::TextColored(format!("It {detached}."), ORANGE));
        }
    }
    rows.push(text_row(""));
    rows.push(text_row("[y] sell    [n] keep it    Esc to cancel"));
    draw_popup("Confirm sale", PopupSize::Small, &rows, fonts, m);
}

fn draw_trade_quantity_menu(
    game: &mut Game,
    structure: Option<Entity>,
    choice: Option<TradeChoice>,
    quantity_input: &str,
    fonts: &Fonts,
    m: &Metrics,
) {
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
    let rows = vec![
        text_row(format!("{verb} how many {}?", game.item_name(&item))),
        text_row(""),
        text_row(format!("Price: {unit_price} Core Fragments each")),
        text_row(""),
        text_row(format!("Quantity: {shown}")),
        text_row(""),
        text_row(format!(
            "Type digits, Enter to {}, Esc to go back",
            verb.to_lowercase()
        )),
    ];
    draw_popup("Trade", PopupSize::Large, &rows, fonts, m);
}

fn draw_perks_menu(game: &mut Game, selected: usize, fonts: &Fonts, m: &Metrics) {
    let status = game.player_status();
    let mut rows = vec![
        Row::TextColored(format!("Perk Points: {}", status.perk_points), CYAN),
        text_row(""),
    ];
    for (i, perk) in feral_processes_engine::Perk::all().iter().enumerate() {
        let level = status.unlocked_perks.iter().filter(|p| *p == perk).count();
        let tag = if level > 0 {
            format!(" (level {level})")
        } else {
            String::new()
        };
        rows.push(item_row(
            format!(
                "[{}] {} - {} Perk Points{}",
                menu_shortcut(i),
                perk.display_name(),
                perk.cost(),
                tag
            ),
            i == selected,
        ));
        rows.push(text_row(format!("    {}", perk.description())));
    }
    rows.push(text_row(""));
    rows.push(text_row(
        "Pick a row's key to buy another level. Esc to close",
    ));
    draw_popup("Perks", PopupSize::Large, &rows, fonts, m);
}

fn draw_research_menu(game: &mut Game, selected: usize, fonts: &Fonts, m: &Metrics) {
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
    let mut rows = vec![
        Row::TextColored(format!("Research Data: {held}/{bank_limit}"), CYAN),
        text_row(""),
    ];
    for (i, node) in nodes.iter().enumerate() {
        let tag = match &node.state {
            ResearchState::Unlocked => " (researched)".to_string(),
            ResearchState::Available => String::new(),
            ResearchState::Locked { missing } => format!(" (needs {})", missing.join(", ")),
        };
        rows.push(item_row(
            format!(
                "[{}] {} - {} Research Data{tag}",
                menu_shortcut(i),
                node.name,
                node.cost
            ),
            i == selected,
        ));
        rows.push(text_row(format!("    {}", node.description)));
    }
    rows.push(text_row(""));
    rows.push(text_row("Pick a row's key to research it. Esc to close"));
    draw_popup("Research", PopupSize::Large, &rows, fonts, m);
}

/// The decompile-odds readout shared by the battle and inspect panels. With
/// no taming catalyst in inventory there are no odds to quote — decompiling
/// isn't available at all — so the line says what's missing instead of a
/// percentage. It stays deliberately generic: which item is a catalyst is
/// item data, not something a renderer gets to name.
fn decompile_chance_line(chance: Option<f32>) -> String {
    match chance {
        Some(c) => format!("Decompile chance right now: {:.0}%", c * 100.0),
        None => "Decompile chance right now: needs a taming catalyst".to_string(),
    }
}

/// A program's current activity as a bracketed suffix — `" (in party)"`,
/// `" (Mining Node)"`, `" (guarding Data Cache)"`, `" (idle)"`. The wording
/// itself is `Game::program_activity`'s; every dialog that lists programs
/// appends it through here so they cannot drift apart.
fn activity_tag(activity: &str) -> String {
    format!(" ({activity})")
}

fn status_tag(status: &Option<String>) -> String {
    status
        .as_ref()
        .map(|s| format!(" [{s}]"))
        .unwrap_or_default()
}

/// Pads `s` to exactly `width` monospace cells, truncating with `…` when it
/// overruns. Exactness is the contract, not a suggestion: the header row and
/// every roster row are assembled from these, so a cell that comes out the
/// wrong width shifts every column after it and the ledger stops lining up.
///
/// Chars, not bytes — a species name carries a zone tag and a companion name
/// is player-chosen, so either can hold multi-byte glyphs that byte slicing
/// would panic on. The UI font advances `…` exactly like every other glyph;
/// `tests/font_rasterization.rs` checks that.
fn cell(s: &str, width: usize) -> String {
    if s.chars().count() > width {
        s.chars()
            .take(width.saturating_sub(1))
            .chain(std::iter::once('…'))
            .collect()
    } else {
        format!("{s:<width$}")
    }
}

/// `cell`, but flushed right. Numeric columns align on their last digit so a
/// column of them can be compared by scanning down it.
fn right(s: &str, width: usize) -> String {
    if s.chars().count() > width {
        cell(s, width)
    } else {
        format!("{s:>width$}")
    }
}

/// Column widths for both battle rosters, in monospace cells. Constants here
/// rather than in `app-core`: there is one renderer to serve, so there is
/// nothing to share them with and nothing to drift against.
const MARK_W: usize = 3;
const NAME_W: usize = 18;
/// `hp/max` is one cell, so a single `HP` header can sit over the pair.
/// Nine fits `9999/9999`; deep-zone stat scaling doubles per zone, so four
/// digits a side is already well past anything reachable.
const HP_W: usize = 9;
const STAT_W: usize = 3;
/// Widest value is `ENGAGED`.
const REACH_W: usize = 7;
/// The one line shape both rosters and both headers are built from, so a
/// column cannot move in a row without moving in its header too.
fn roster_line(
    mark: &str,
    name: &str,
    hp: &str,
    atk: &str,
    def: &str,
    reach: &str,
    tail: &str,
) -> String {
    format!(
        "{}{} {} {} {} {} {}",
        cell(mark, MARK_W),
        cell(name, NAME_W),
        cell(hp, HP_W),
        right(atk, STAT_W),
        right(def, STAT_W),
        cell(reach, REACH_W),
        tail,
    )
}

/// `RANGE` because a hostile group's third column is its reach, not a
/// front/back rank the player chose.
fn hostile_header() -> String {
    roster_line("   ", "GROUP", "HP", "ATK", "DEF", "RANGE", "STATUS")
}

fn party_header() -> String {
    roster_line("   ", "NAME", "HP", "ATK", "DEF", "POS", "ACTION")
}

/// `roster_line` with the two stat columns taken as the numbers they are.
fn roster_row(
    mark: &str,
    name: &str,
    hp: &str,
    atk: i32,
    def: i32,
    reach: &str,
    tail: &str,
) -> String {
    roster_line(
        mark,
        name,
        hp,
        &atk.to_string(),
        &def.to_string(),
        reach,
        tail,
    )
}

fn draw_battle(app: &mut App, fx: &mut Fx, fonts: &Fonts, m: &Metrics) {
    let Some(game) = &mut app.game else { return };
    let Some(view) = game.battle_view() else {
        return;
    };

    let w = screen_width();
    // The battle screen sits straight on the window instead of inside a
    // panel, so it holds off the edges by more than panel content does.
    let margin = m.inset * 2.0;
    let mut y = margin;
    let dt = get_frame_time();

    fonts.ui(
        format!("Hostile programs — round {}", view.round),
        margin,
        y,
        m.font_size,
        TEXT,
    );
    y += m.line_height;
    fonts.ui(hostile_header(), margin, y, m.label(), TEXT_DIM);
    y += m.line_height;

    for (idx, g) in view.groups.iter().enumerate() {
        let bar = BarGeometry {
            x: margin,
            y,
            w: w - margin * 2.0,
        };
        let ghost = fx.bar_ghost(idx as u64, g.front_hp, dt);
        let name = if g.count > 1 {
            format!("{} {}s", g.count, g.species_name)
        } else {
            g.species_name.clone()
        };
        // Back groups are desaturated so the reach rule is legible at a
        // glance, rather than something to infer from the log.
        let color = if !g.engaged {
            desaturate(RED)
        } else if g.is_boss {
            MAGENTA
        } else {
            RED
        };
        y = draw_bar(
            bar,
            &roster_row(
                &format!("{}  ", g.letter),
                &format!("{name}{}", if g.is_boss { " [BOSS]" } else { "" }),
                &format!("{}/{}", g.front_hp, g.front_max_hp),
                g.atk,
                g.def,
                if g.engaged { "ENGAGED" } else { "BACK" },
                // The engine owns the wording of a condition
                // ("Bleeding (2)"); upper-casing is presentation, but
                // abbreviating a vocabulary this renderer does not define
                // would not be. `OK` rather than blank, because an empty
                // cell in a ledger reads as missing data.
                &g.status_effect
                    .as_deref()
                    .map(str::to_uppercase)
                    .unwrap_or_else(|| "OK".to_string()),
            ),
            g.front_hp as f32,
            g.front_max_hp.max(1) as f32,
            BarStyle::plain(color),
            fonts,
            m,
        );
        draw_ghost_band(
            bar,
            g.front_hp as f32,
            ghost.ghost,
            g.front_max_hp.max(1) as f32,
            color,
            m,
        );
        // Damage is inferred from the HP the view reports rather than from
        // a dedicated engine event — a round resolves entirely between two
        // frames, so the drop is unambiguous. Floats spawn at their own
        // row now that there is more than one bar to attribute them to.
        if ghost.damage > 0 {
            fx.spawn_float(format!("-{}", ghost.damage), w / 2.0, bar.y, RED);
        }
        y += m.inset;
    }

    y += m.inset;

    // Hostiles on top, your party on the bottom, with the log between them —
    // the two rosters read as opposing sides with the narration of what
    // passed between them in the middle. The party block is bottom-anchored
    // above the action bar so the log can take exactly the slack left over,
    // which is why its height has to be computed rather than accumulated.
    let log_bottom = screen_height() - m.line_height * 2.0;
    // Two line heights, not one: the block's title *and* its column header
    // sit above the first bar. Getting this wrong shifts the whole party
    // block, since it is bottom-anchored off this figure.
    let party_height = m.line_height * 2.0 + view.party.len() as f32 * bar_row_height(m) + m.inset;
    let party_top = (log_bottom - party_height).max(y);

    let log_height = party_top - y;
    draw_rectangle(margin, y, w - margin * 2.0, log_height, PANEL_BG);
    draw_rectangle_lines(margin, y, w - margin * 2.0, log_height, 2.0, BORDER);
    // Floors at 0, not 1. On a window too short to seat both rosters this
    // pane collapses to nothing, and forcing a line into it drew narration
    // at the party block's first row — which the party header then painted
    // over.
    let capacity = ((log_height - margin) / m.line_height).max(0.0) as usize;
    let mut ly = y + margin;
    for (kind, line) in game.message_log(capacity) {
        if ly + m.line_height > party_top {
            break;
        }
        draw_message_line(kind, &line, margin + m.inset, ly, fonts, m);
        ly += m.line_height;
    }

    y = party_top;
    fonts.ui(
        format!("Your party — DECOMP {}", view.player_decompiler),
        margin,
        y,
        m.font_size,
        TEXT,
    );
    y += m.line_height;
    fonts.ui(party_header(), margin, y, m.label(), TEXT_DIM);
    y += m.line_height;

    for p in &view.party {
        let bar = BarGeometry {
            x: margin,
            y,
            w: w - margin * 2.0,
        };
        let ghost = fx.bar_ghost(PARTY_BAR_KEY_BASE + p.slot as u64, p.hp, dt);
        let active = view.active_slot == Some(p.slot);
        let color = if active { CYAN } else { GREEN };
        y = draw_bar(
            bar,
            &roster_row(
                &format!("{}{} ", if active { ">" } else { " " }, p.slot + 1),
                &p.name,
                &format!("{}/{}", p.hp, p.max_hp),
                p.atk,
                p.def,
                if p.front { "FRONT" } else { "BACK" },
                // A member's own condition rides in the ACTION column
                // rather than getting a seventh fixed cell that would be
                // empty on almost every row.
                &format!(
                    "{}{}",
                    p.planned.as_deref().unwrap_or("—"),
                    status_tag(&p.status_effect),
                ),
            ),
            p.hp as f32,
            p.max_hp.max(1) as f32,
            BarStyle {
                color,
                bold: active,
            },
            fonts,
            m,
        );
        draw_ghost_band(
            bar,
            p.hp as f32,
            ghost.ghost,
            p.max_hp.max(1) as f32,
            color,
            m,
        );
        if ghost.damage > 0 {
            fx.spawn_float(format!("-{}", ghost.damage), w / 2.0, bar.y, TEXT);
        }
        y += m.inset;
    }

    // The action bar is drawn from whatever the engine offers, never from
    // strings authored here — so a new action reaches both renderers
    // without either being touched.
    let mut actions: Vec<String> = view
        .options
        .iter()
        .map(|o| match &o.unavailable {
            None => o.label.clone(),
            Some(reason) => format!("{} ({reason})", o.label),
        })
        .collect();
    // Party-level commands come from the engine too, so the two renderers
    // cannot drift on them either.
    actions.extend(game.battle_party_commands().into_iter().map(|c| c.label));
    fonts.ui(
        actions.join("   "),
        margin,
        screen_height() - m.font_size as f32,
        m.font_size,
        TEXT,
    );

    fx.draw_floats(fonts, m);
}

/// Which of the acting member's abilities does the Special spend? Rows come
/// from the engine, same contract as the action bar.
fn draw_battle_special_menu(app: &mut App, fonts: &Fonts, m: &Metrics) {
    let selected = app.menu_selected;
    let Some(game) = &mut app.game else { return };
    let Some(slot) = game.battle_active_slot() else {
        return;
    };
    let mut rows = vec![text_row("Which ability?")];
    for (i, o) in game.battle_special_options(slot).into_iter().enumerate() {
        rows.push(creature_row(
            format!("[{}] {}", i + 1, o.detail),
            i == selected,
        ));
    }
    draw_popup("Pick a special", PopupSize::Large, &rows, fonts, m);
}

/// Who does this buff or heal land on? Lists you and every standing
/// companion — the whole point of aiming one.
fn draw_battle_ally_menu(app: &mut App, fonts: &Fonts, m: &Metrics) {
    let selected = app.menu_selected;
    let title = app.battle_target_title();
    let Some(game) = &mut app.game else { return };
    let mut rows = vec![text_row("Apply to whom?")];
    for (i, a) in game.battle_ally_options().into_iter().enumerate() {
        rows.push(creature_row(
            format!("[{}] {} — {}", i + 1, a.name, a.detail),
            i == selected,
        ));
    }
    draw_popup(&title, PopupSize::Large, &rows, fonts, m);
}

/// Which group does the pending action hit? Shows per-group decompile odds,
/// since that's the one action where the choice of target is a real gamble
/// rather than a preference.
fn draw_battle_target_menu(app: &mut App, fonts: &Fonts, m: &Metrics) {
    let selected = app.menu_selected;
    let title = app.battle_target_title();
    let Some(game) = &mut app.game else { return };
    let Some(view) = game.battle_view() else {
        return;
    };
    let mut rows = vec![text_row("Target which group?")];
    for (i, g) in view.groups.iter().enumerate() {
        let odds = match g.decompile_chance {
            Some(c) => format!(" — decompile {:.0}%", c * 100.0),
            None => String::new(),
        };
        rows.push(creature_row(
            format!(
                "[{}] {} x{} — {}/{} HP {}{}",
                g.letter,
                g.species_name,
                g.count,
                g.front_hp,
                g.front_max_hp,
                if g.engaged { "<engaged>" } else { "<back>" },
                odds,
            ),
            i == selected,
        ));
    }
    draw_popup(&title, PopupSize::Large, &rows, fonts, m);
}

/// Which consumable does this slot spend? Lists only what's actually
/// usable — the action is greyed out with a reason before it gets here.
fn draw_battle_item_menu(app: &mut App, fonts: &Fonts, m: &Metrics) {
    let selected = app.menu_selected;
    let Some(game) = &mut app.game else { return };
    let items = game.battle_usable_items();
    let mut rows = vec![text_row(
        "Use which item? It costs this member their round.",
    )];
    for (i, item) in items.iter().enumerate() {
        rows.push(item_row(
            format!("[{}] {}", menu_shortcut(i), game.item_name(item)),
            i == selected,
        ));
    }
    draw_popup("Use an item", PopupSize::Large, &rows, fonts, m);
}

fn draw_main_menu(app: &App, fonts: &Fonts, m: &Metrics) {
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
    draw_popup("Main Menu", PopupSize::Large, &rows, fonts, m);
}

fn draw_load_game(app: &App, fonts: &Fonts, m: &Metrics) {
    let saves = app.list_saves();
    let mut rows = vec![text_row(
        "Pick a save (Esc to cancel; Up/Down + Enter also work)",
    )];
    if saves.is_empty() {
        rows.push(text_row("(no saves found)"));
    }
    for (i, save) in saves.iter().enumerate() {
        let summary = save
            .summary
            .as_deref()
            .unwrap_or("(incompatible save - can still be deleted)");
        rows.push(item_row(
            format!("[{}] {} - {}", menu_shortcut(i), save.name, summary),
            i == app.menu_selected,
        ));
    }
    draw_popup("Load Game", PopupSize::Large, &rows, fonts, m);
}

fn draw_save_action(app: &App, fonts: &Fonts, m: &Metrics) {
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
    draw_popup("Save", PopupSize::Large, &rows, fonts, m);
}

fn draw_difficulty_pick(selected: usize, fonts: &Fonts, m: &Metrics) {
    let rows = vec![
        item_row(
            "[P] Permadeath - flatlining is final; the session is archived to a log".to_string(),
            selected == 0,
        ),
        item_row(
            "[F] Forgiving - flatlining costs you, but you reboot and keep going".to_string(),
            selected == 1,
        ),
        text_row(""),
        text_row("Esc to go back; Up/Down + Enter also work"),
    ];
    draw_popup("New Game", PopupSize::Large, &rows, fonts, m);
}

fn draw_game_over(app: &mut App, fonts: &Fonts, m: &Metrics) {
    let summary = app
        .game
        .as_mut()
        .and_then(|g| g.history_summary())
        .unwrap_or_else(|| "Connection lost.".to_string());
    let rows = vec![
        Row::TextColored("FLATLINE".to_string(), RED),
        text_row(""),
        text_row(summary),
        text_row(""),
        text_row("Press any key to return to the main menu"),
    ];
    draw_popup("Session Terminated", PopupSize::Large, &rows, fonts, m);
}

fn draw_help(fonts: &Fonts, m: &Metrics) {
    let rows = vec![
        text_row("hjkl/arrows move   . wait   e drain   r recharge"),
        text_row("g scan   c compile   b deploy   w cronjob   G guard   R demolish"),
        text_row("u symlink   i inspect   v inventory   p companions"),
        text_row("f fuse   t trade   x perks   T research   s save   q main menu"),
        text_row("+/- zoom   [/] volume   \\ visual effects"),
        text_row(""),
        text_row("Every numbered menu also takes Up/Down + Enter, on top of"),
        text_row("typing a row's own number/letter directly."),
        text_row(""),
        text_row("In an intrusion:  a attack   d defend   s special   c decompile"),
        text_row("                  u use item   j jack out"),
        text_row("                  A all attack   D all defend (shift = the whole party)"),
        text_row(""),
        text_row("Press any key to close"),
    ];
    draw_popup("Help", PopupSize::Large, &rows, fonts, m);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A refused menu pick ("Requires Automation first.") only reaches the
    /// player through `App::status_line`, and every gameplay menu draws a
    /// popup over the log pane that used to be the sole place it appeared —
    /// which made a refusal indistinguishable from a dead keypress.
    /// Where the ragged last column must start: every fixed cell plus the
    /// single space separating each pair. Spelled out here rather than in
    /// the layout itself, so it is an independent expectation the tests hold
    /// `roster_line` to — if the format string gains or loses a separator,
    /// these tests fail instead of quietly agreeing with it.
    const TAIL_COL: usize = MARK_W + NAME_W + 1 + HP_W + 1 + STAT_W + 1 + STAT_W + 1 + REACH_W + 1;

    /// Reads `line` from char offset `col` — the rows are monospace, so a
    /// char offset *is* a screen column.
    fn at(line: &str, col: usize) -> String {
        line.chars().skip(col).collect()
    }

    /// The header and every row are assembled from one set of widths, so the
    /// ragged last column begins at the same offset on every line of a
    /// roster block. That alignment is the whole ledger effect — asserted
    /// rather than left to two format strings staying in step by hand.
    #[test]
    fn every_roster_line_puts_its_tail_at_the_same_column() {
        let lines = [
            hostile_header(),
            roster_row(
                "A  ",
                "4 Null Daemons",
                "18/30",
                9,
                4,
                "ENGAGED",
                "BLEEDING (2)",
            ),
            roster_row("B  ", "Warden Process", "44/44", 14, 9, "BACK", "OK"),
            party_header(),
            roster_row(">1 ", "You", "21/30", 11, 6, "FRONT", "Attack A"),
            roster_row(" 2 ", "Sparkgrub", "18/18", 7, 3, "FRONT", "Defend"),
        ];
        for line in &lines {
            assert_eq!(
                line.chars().take(TAIL_COL).count(),
                TAIL_COL,
                "{line:?} is shorter than the fixed columns"
            );
        }
        assert!(at(&lines[0], TAIL_COL).starts_with("STATUS"));
        assert!(at(&lines[1], TAIL_COL).starts_with("BLEEDING"));
        assert!(at(&lines[3], TAIL_COL).starts_with("ACTION"));
        assert!(at(&lines[4], TAIL_COL).starts_with("Attack A"));
    }

    /// A whole roster block, character-exact. The other tests assert the
    /// invariants; this one shows what the screen actually reads like, so a
    /// change to any width is reviewable as a diff of the output rather than
    /// of arithmetic.
    #[test]
    fn a_roster_block_reads_as_an_aligned_table() {
        let block = [
            hostile_header(),
            roster_row(
                "A  ",
                "4 Null Daemons",
                "18/30",
                9,
                4,
                "ENGAGED",
                "BLEEDING (2)",
            ),
            roster_row("B  ", "Warden Process", "44/44", 14, 9, "BACK", "OK"),
            roster_row("C  ", "Sentinel [BOSS]", "120/120", 22, 15, "BACK", "OK"),
        ]
        .join("\n");
        assert_eq!(
            block,
            "   GROUP              HP        ATK DEF RANGE   STATUS\n\
             A  4 Null Daemons     18/30       9   4 ENGAGED BLEEDING (2)\n\
             B  Warden Process     44/44      14   9 BACK    OK\n\
             C  Sentinel [BOSS]    120/120    22  15 BACK    OK"
        );
    }

    /// And each header label sits over the column it names.
    #[test]
    fn the_header_labels_sit_over_their_columns() {
        let h = party_header();
        assert!(at(&h, MARK_W).starts_with("NAME"));
        assert!(at(&h, MARK_W + NAME_W + 1).starts_with("HP"));
        assert!(at(&h, TAIL_COL).starts_with("ACTION"));
    }

    /// An over-long name is clipped rather than allowed to shove the stats
    /// rightward — the failure this whole design exists to prevent.
    #[test]
    fn a_long_name_does_not_shift_the_columns_after_it() {
        let long = roster_row(
            "A  ",
            "4 Corrupted Null Daemons of Yendor",
            "8/8",
            3,
            1,
            "ENGAGED",
            "OK",
        );
        assert_eq!(long.chars().take(TAIL_COL).count(), TAIL_COL);
        assert!(long.contains('…'), "the clipped name has to show it");
        assert!(at(&long, TAIL_COL).starts_with("OK"));
    }

    /// Numbers right-align so a column of them can be compared by scanning
    /// down it, which is the reason for having columns at all.
    #[test]
    fn stat_columns_are_right_aligned() {
        let row = roster_row("A  ", "Glitch", "8/8", 3, 1, "ENGAGED", "OK");
        let atk = MARK_W + NAME_W + 1 + HP_W + 1;
        assert_eq!(
            row.chars().skip(atk).take(STAT_W).collect::<String>(),
            "  3"
        );
        assert_eq!(
            row.chars()
                .skip(atk + STAT_W + 1)
                .take(STAT_W)
                .collect::<String>(),
            "  1"
        );
    }

    #[test]
    fn cell_pads_short_content_to_exactly_the_column_width() {
        assert_eq!(cell("You", 8), "You     ");
        assert_eq!(cell("", 3), "   ");
        assert_eq!(cell("exact", 5), "exact");
    }

    /// A name longer than its column has to lose its tail. Letting it
    /// through would push every column after it right, which defeats the
    /// entire point of a ledger.
    #[test]
    fn cell_truncates_over_width_content_and_marks_it() {
        let out = cell("4 Corrupted Null Daemons", 12);
        assert_eq!(out.chars().count(), 12);
        assert!(
            out.ends_with('…'),
            "a clipped cell has to show that it was clipped, got {out:?}"
        );
    }

    /// Counted in chars, not bytes: `zone_tagged_name` and a player-chosen
    /// companion name can both hold multi-byte glyphs, and slicing one
    /// mid-glyph would panic.
    #[test]
    fn cell_counts_characters_not_bytes() {
        assert_eq!(cell("Ünïcödé", 7).chars().count(), 7);
        assert_eq!(cell("Ünïcödé", 4).chars().count(), 4);
        assert_eq!(cell("Ünïcödé", 9).chars().count(), 9);
    }

    #[test]
    fn every_mode_that_covers_the_log_pane_gets_the_status_banner() {
        for mode in [
            Mode::Research,
            Mode::Build,
            Mode::Craft,
            Mode::Trade,
            Mode::Inventory,
            Mode::Battle,
            Mode::BattleTarget,
            Mode::BattleItem,
            Mode::Help,
            Mode::LoadGame,
        ] {
            assert!(
                needs_status_banner(mode),
                "{mode:?} draws over the log pane, so its refusals need the banner"
            );
        }
    }

    #[test]
    fn modes_that_already_show_the_status_line_dont_double_up() {
        for mode in [Mode::Playing, Mode::MainMenu, Mode::SaveAction] {
            assert!(
                !needs_status_banner(mode),
                "{mode:?} already surfaces status_line itself"
            );
        }
    }
}
