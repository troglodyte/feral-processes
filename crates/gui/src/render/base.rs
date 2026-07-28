//! The map screen: terrain, entities, effects, and the status panel beside them.

use super::bars::*;
use super::*;

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
pub(super) fn draw_playing_base(app: &mut App, fx: &Fx, painter: &Painter, m: &Metrics) {
    let (tile_px, glyph_px) = map_cell(app.zoom);
    let status_line = app.status_line.clone();
    let Some(game) = &mut app.game else { return };

    let map_w = painter.screen_w() * 0.7;
    let map_h = painter.screen_h() * 0.72;
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

    painter.rect(0.0, 0.0, map_w, map_h, Color::new(0.03, 0.03, 0.05, 1.0));
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
            painter.rect(px, py, tile_px - 1.0, tile_px - 1.0, bg);
            let glyph = ch.to_string();
            let dims = painter.measure_map(&glyph, glyph_px);
            let tx = px + (tile_px - dims.width) / 2.0;
            let ty = py + (tile_px + dims.height) / 2.0;
            painter.map(&glyph, tx, ty, glyph_px, color);
            // Marks where the player materialized on breaching into this
            // zone (see `Game::zone_spawn_point`) — an outline rather than
            // replacing the glyph, so whatever's actually standing there
            // (the player, a creature, a rebuilt structure) still reads
            // clearly on top of it.
            let spawn_rx = spawn_point.0 - status.position.0 + half_w;
            let spawn_ry = spawn_point.1 - status.position.1 + half_h;
            if rx as i32 == spawn_rx && ry as i32 == spawn_ry {
                painter.rect_lines(px, py, tile_px - 1.0, tile_px - 1.0, 2.0, MAGENTA);
            }
            // A structure with a pet actively cronjob-assigned gets a
            // yellow outline so it's visible at a glance without opening
            // the cronjob menu to check.
            if staffed {
                painter.rect_lines(px, py, tile_px - 1.0, tile_px - 1.0, 2.0, YELLOW);
            }
            // The shield network is base-wide, not per-structure, so every
            // structure carries the same faint pulse while one is standing.
            // Drawn under the flash so a raid still reads on top of it.
            if let Some(pulse) = shield_outline.filter(|_| shielded) {
                painter.rect_lines(px, py, tile_px - 1.0, tile_px - 1.0, 2.0, pulse);
            }
            let world = (
                status.position.0 + rx as i32 - half_w,
                status.position.1 + ry as i32 - half_h,
            );
            if let Some(flash) = fx.tile_flash(world) {
                painter.rect(px, py, tile_px - 1.0, tile_px - 1.0, flash);
            }
        }
    }
    painter.rect_lines(0.0, 0.0, map_w, map_h, 2.0, BORDER);

    draw_status_panel(
        Rect::new(map_w, 0.0, painter.screen_w() - map_w, map_h),
        &status,
        game,
        painter,
        m,
    );

    let log_y = map_h;
    let log_h = painter.screen_h() - map_h;
    painter.rect(0.0, log_y, painter.screen_w(), log_h, PANEL_BG);
    painter.rect_lines(
        0.0,
        log_y,
        painter.screen_w(),
        log_h,
        2.0,
        fx.log_border(BORDER),
    );
    let mut ly = log_y + m.inset + m.font_size as f32 / 2.0;
    if let Some(s) = &status_line {
        painter.ui(s, m.inset, ly, m.font_size, RED);
        ly += m.line_height;
    }
    let capacity = ((log_h - m.line_height) / m.line_height).max(1.0) as usize;
    for (kind, line) in game.message_log(capacity) {
        if ly > painter.screen_h() - m.gap {
            break;
        }
        draw_message_line(kind, &line, m.inset, ly, painter, m);
        ly += m.line_height;
    }
}

fn draw_status_panel(
    rect: Rect,
    status: &feral_processes_engine::PlayerStatus,
    game: &Game,
    painter: &Painter,
    m: &Metrics,
) {
    let Rect { x, y, w, h } = rect;
    painter.rect(x, y, w, h, PANEL_BG);
    painter.rect_lines(x, y, w, h, 2.0, BORDER);

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
        painter,
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
        painter,
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
        painter,
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
        painter.ui(line, x + m.inset, cy, m.font_size, TEXT);
        cy += m.line_height;
    }
    painter.ui(
        format!(
            "Party: {}/{}",
            status.companions.len(),
            feral_processes_engine::tuning::MAX_PARTY_SIZE
        ),
        x + m.inset,
        cy,
        m.font_size,
        GREEN,
    );
    cy += m.line_height;
    painter.ui(
        format!("Pets: {}/{}", status.pet_count, status.pet_capacity),
        x + m.inset,
        cy,
        m.font_size,
        GREEN,
    );
    cy += m.line_height;
    for companion in &status.companions {
        painter.ui(
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
    painter.ui("Inventory:", x + m.inset, cy, m.font_size, TEXT);
    cy += m.line_height;
    if status.inventory.is_empty() {
        painter.ui("(empty)", x + m.inset, cy, m.font_size, TEXT_DIM);
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
        painter.ui(
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
        painter.ui(k, x + m.inset, ky, m.small(), TEXT_DIM);
        ky += keys_line_height;
    }
}
