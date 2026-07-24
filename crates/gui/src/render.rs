//! All drawing for the graphics frontend. Mirrors what
//! `feral-processes-tui`'s `ui.rs` shows for each `Mode` — same engine data,
//! same information — laid out with macroquad's immediate-mode primitives
//! (filled rects for bars/tiles, drawn text for menus) instead of ratatui
//! widgets.

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
    Entity, EntityView, Game, MAX_FUSIONS, MessageKind, PetInfo, ResearchState,
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

/// Display styling for a message-log line, chosen by the engine-supplied
/// `MessageKind` rather than by sniffing the text — low-priority chatter
/// stays dim, gains/damage that matter get a color.
fn draw_message_line(kind: MessageKind, text: &str, x: f32, y: f32, fonts: &Fonts, m: &Metrics) {
    let color = match kind {
        MessageKind::Info => TEXT_DIM,
        MessageKind::Loot => GREEN,
        MessageKind::LevelUp => GREEN,
        MessageKind::Raid => ORANGE,
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
        Mode::BattleCompanion => {
            draw_battle(app, fx, fonts, &m);
            draw_battle_companion_menu(app, fonts, &m);
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

    let first_item = rows.iter().position(|r| matches!(r, Row::Item(_, _)));
    let last_item = rows.iter().rposition(|r| matches!(r, Row::Item(_, _)));
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
            .position(|r| matches!(r, Row::Item(_, true)))
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
        Row::Item(s, selected) => {
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
            fonts.ui(format!("{prefix}{s}"), x + m.pad, cy, m.font_size, TEXT);
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
        "Integrity",
        status.hp as f32,
        status.max_hp.max(1) as f32,
        RED,
        fonts,
        m,
    );
    cy = draw_bar(
        BarGeometry {
            x: x + m.inset,
            y: cy,
            w: w - m.inset * 2.0,
        },
        "Power",
        status.hunger,
        100.0,
        YELLOW,
        fonts,
        m,
    );
    cy = draw_bar(
        BarGeometry {
            x: x + m.inset,
            y: cy,
            w: w - m.inset * 2.0,
        },
        "Fatigue",
        status.fatigue,
        100.0,
        BLUE,
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

/// Draws a labeled bar (HP/Power/Fatigue) and returns the y coordinate for
/// whatever's drawn next.
fn draw_bar(
    g: BarGeometry,
    label: &str,
    value: f32,
    max: f32,
    color: Color,
    fonts: &Fonts,
    m: &Metrics,
) -> f32 {
    let ratio = (value / max).clamp(0.0, 1.0);
    fonts.ui(
        format!("{label} {value:.0}/{max:.0}"),
        g.x,
        g.y,
        m.label(),
        TEXT,
    );
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
        rows.push(item_row(
            format!(
                "[{}] {} - {}",
                menu_shortcut(i),
                game.item_name(&recipe.result),
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
            "You have: {held}        Buffer: {}/{}",
            status.inventory_used, status.inventory_capacity
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
        rows.push(item_row(
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
            "Inventory - Buffer {}/{} (row key to equip/fuse/erase):",
            status.inventory_used, status.inventory_capacity
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
        rows.push(item_row(
            format!(
                "[{}] {} Lv{} - HP {}/{}  ATK {}  DEF {}  PWR {}{}{}{}{}",
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
            let active = if p.is_companion { " (in party)" } else { "" };
            let job = p
                .job_structure
                .as_ref()
                .map(|s| format!(" (on a cronjob: {s})"))
                .unwrap_or_default();
            format!(
                "[{num}] {} Lv{} - HP {}/{}  ATK {}  DEF {}  PWR {}{fused}{active}{job}",
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
    let inventory = game.player_status().inventory;
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
        rows.push(item_row(
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
    rows.push(text_row(""));
    rows.push(Row::TextColored("Buy:".to_string(), TEXT));
    for (item, cost) in &trade.buy {
        rows.push(item_row(
            format!(
                "[{}] Buy {} ({cost} Core Fragments each)",
                menu_shortcut(idx),
                game.item_name(item)
            ),
            idx == selected,
        ));
        idx += 1;
    }
    rows.push(text_row(""));
    rows.push(text_row("Esc to cancel; Up/Down + Enter also work"));
    draw_popup("Trade", PopupSize::Large, &rows, fonts, m);
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

fn status_tag(status: &Option<String>) -> String {
    status
        .as_ref()
        .map(|s| format!(" [{s}]"))
        .unwrap_or_default()
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
    let pack_tag = if view.pack_remaining > 0 {
        format!(" [+{} more in the pack]", view.pack_remaining)
    } else {
        String::new()
    };
    let battle_fx = fx.battle_frame(view.wild_hp, view.player_hp, get_frame_time());
    let wild_bar = BarGeometry {
        x: margin,
        y,
        w: w - margin * 2.0,
    };
    y = draw_bar(
        wild_bar,
        &format!(
            "{}{}{}{} (ATK {} / DEF {} / PWR {})",
            view.wild_name,
            if view.wild_is_boss { " [BOSS]" } else { "" },
            status_tag(&view.wild_status_effect),
            pack_tag,
            view.wild_atk,
            view.wild_def,
            view.wild_power
        ),
        view.wild_hp as f32,
        view.wild_max_hp.max(1) as f32,
        RED,
        fonts,
        m,
    );
    draw_ghost_band(
        wild_bar,
        view.wild_hp as f32,
        battle_fx.wild_ghost,
        view.wild_max_hp.max(1) as f32,
        RED,
        m,
    );
    y += m.inset;
    let player_bar = BarGeometry {
        x: margin,
        y,
        w: w - margin * 2.0,
    };
    y = draw_bar(
        player_bar,
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
        fonts,
        m,
    );
    draw_ghost_band(
        player_bar,
        view.player_hp as f32,
        battle_fx.player_ghost,
        view.player_max_hp.max(1) as f32,
        CYAN,
        m,
    );
    y += m.inset;

    // Damage is inferred from the HP the view reports rather than from a
    // dedicated engine event — a battle round resolves entirely between
    // two frames, so the drop is unambiguous.
    if battle_fx.wild_damage > 0 {
        fx.spawn_float(
            format!("-{}", battle_fx.wild_damage),
            w / 2.0,
            wild_bar.y,
            RED,
        );
    }
    if battle_fx.player_damage > 0 {
        fx.spawn_float(
            format!("-{}", battle_fx.player_damage),
            w / 2.0,
            player_bar.y,
            TEXT,
        );
    }

    for companion in &view.companions {
        fonts.ui(
            format!(
                "Companion: {} (HP {}/{}, ATK {}, PWR {}){}",
                companion.name,
                companion.hp,
                companion.max_hp,
                companion.atk,
                companion.power,
                status_tag(&companion.status)
            ),
            margin,
            y,
            m.font_size,
            GREEN,
        );
        y += m.line_height;
    }

    fonts.ui(
        decompile_chance_line(view.decompile_chance),
        margin,
        y,
        m.font_size,
        MAGENTA,
    );
    y += m.line_height + m.inset;

    let log_bottom = screen_height() - m.line_height * 2.0;
    draw_rectangle(margin, y, w - margin * 2.0, log_bottom - y, PANEL_BG);
    draw_rectangle_lines(margin, y, w - margin * 2.0, log_bottom - y, 2.0, BORDER);
    let capacity = (((log_bottom - y) - margin) / m.line_height).max(1.0) as usize;
    let mut ly = y + margin;
    for (kind, line) in game.message_log(capacity) {
        draw_message_line(kind, &line, margin + m.inset, ly, fonts, m);
        ly += m.line_height;
    }

    let mut actions = vec!["[A]ttack".to_string()];
    if view.decompile_chance.is_some() {
        actions.push("[D]ecompile".to_string());
    }
    if !view.companions.is_empty() {
        actions.push("[C]ommand companion".to_string());
    }
    actions.push("[J]ack Out".to_string());
    fonts.ui(
        actions.join("   "),
        margin,
        screen_height() - m.font_size as f32,
        m.font_size,
        TEXT,
    );

    fx.draw_floats(fonts, m);
}

fn draw_battle_companion_menu(app: &mut App, fonts: &Fonts, m: &Metrics) {
    let selected = app.menu_selected;
    let Some(game) = &mut app.game else { return };
    let party = game.player_status().companions;
    let mut rows = vec![text_row(
        "Command which companion? It'll buff you instead of attacking.",
    )];
    for (i, c) in party.iter().enumerate() {
        rows.push(item_row(
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
    draw_popup("Command Companion", PopupSize::Large, &rows, fonts, m);
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
        text_row("In an intrusion:  a attack   d decompile   c command companion   j jack out"),
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
    #[test]
    fn every_mode_that_covers_the_log_pane_gets_the_status_banner() {
        for mode in [
            Mode::Research,
            Mode::Build,
            Mode::Craft,
            Mode::Trade,
            Mode::Inventory,
            Mode::Battle,
            Mode::BattleCompanion,
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
