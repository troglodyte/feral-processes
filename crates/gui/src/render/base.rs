//! The map screen: terrain, entities, effects, and the status panel beside them.

use super::bars::*;
use super::dungeon::draw_dungeon;
use super::field::draw_status_buffs;
use super::*;

/// How far a bare tile's background may stray from its biome's flat colour,
/// as a fraction either side. Enough to break up a field of identical tiles,
/// not enough to read as two different biomes.
const SHADE_JITTER: f32 = 0.08;
/// How dark the map pane's corners get relative to its centre. Floored well
/// short of illegible: the vignette is depth, and must never be the reason a
/// hostile at the pane's edge goes unnoticed.
const VIGNETTE_MIN: f32 = 0.75;

/// A tile's own brightness multiplier, so a field of one biome reads as
/// ground rather than as a flat colour swatch.
///
/// Hashed from the world coordinate, never the screen cell: the camera now
/// slides continuously across tiles, and a shade tied to screen position
/// would crawl over the terrain as it went. The two axes are mixed with
/// different constants because a symmetric hash bands the map along its
/// diagonal, which reads as a pattern instead of as texture.
fn tile_shade(world: (i32, i32)) -> f32 {
    let mut h =
        (world.0 as u32).wrapping_mul(0x9E37_79B9) ^ (world.1 as u32).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2545_F491);
    h ^= h >> 13;
    let t = (h & 0xFFFF) as f32 / 65535.0;
    1.0 - SHADE_JITTER + 2.0 * SHADE_JITTER * t
}

/// Radial dimming toward the edge of the map pane, given a tile's offset from
/// the pane centre and the pane's half-extent, both in pixels.
///
/// Anchored to the pane rather than to the grid, so it stays put while the
/// camera slides beneath it. Normalising by the half-extent is what keeps the
/// gradient the same shape at every zoom step and window size, and squaring
/// the radius keeps the centre broadly flat so the falloff reads only near
/// the edges.
fn vignette(dx: f32, dy: f32, half_w_px: f32, half_h_px: f32) -> f32 {
    let r = ((dx / half_w_px).powi(2) + (dy / half_h_px).powi(2))
        .sqrt()
        .min(1.0);
    1.0 - (1.0 - VIGNETTE_MIN) * r * r
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
pub(super) fn draw_playing_base(app: &mut App, fx: &mut Fx, painter: &Painter, m: &Metrics) {
    let (tile_px, glyph_px) = map_cell(app.zoom);
    let status_line = app.status_line.clone();
    // Read before the `game` borrow below: the results of a battle that just
    // ended are at the tail of the log and scroll in one at a time.
    let hidden = app.hidden_log_lines();
    let Some(game) = &mut app.game else { return };

    let map_w = painter.screen_w() * 0.7;
    let map_h = painter.screen_h() * 0.72;

    let status = game.player_status();
    // `Game::active_buffs` needs `&mut self`; fetched here rather than
    // inside `draw_status_panel`, which only ever needed `&Game` before
    // this and shouldn't have to start borrowing mutably just to draw.
    let buffs = game.active_buffs();
    if let Some(view) = game.dungeon_view() {
        draw_dungeon(&view, painter, map_w, map_h, m);
    } else {
        draw_surface_map(game, fx, painter, map_w, map_h, tile_px, glyph_px, &status);
    }

    draw_status_panel(
        Rect::new(map_w, 0.0, painter.screen_w() - map_w, map_h),
        &status,
        &buffs,
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
    // Fetch the hidden tail as well, so chopping it still leaves a full
    // pane's worth of older lines to draw.
    let lines = game.message_log(capacity + hidden);
    let shown = lines.len().saturating_sub(hidden);
    for (kind, line) in lines
        .iter()
        .take(shown)
        .skip(shown.saturating_sub(capacity))
    {
        if ly > painter.screen_h() - m.gap {
            break;
        }
        draw_message_line(*kind, line, m.inset, ly, painter, m);
        ly += m.line_height;
    }
}

/// The message log in full — everything the pane at the bottom of the map
/// has room for a few lines of.
///
/// The footer states the screen's three limits rather than leaving the player
/// to infer them from an absence: the engine keeps `MESSAGE_LOG_CAP` lines,
/// repeats are folded into one row apiece, and
/// `MessageLog::retain_outcomes_since_battle` drops a finished intrusion's
/// blow-by-blow, so an old fight reads as its results.
pub(super) fn draw_history(game: &Game, selected: usize, painter: &Painter, m: &Metrics) {
    let entries = game.message_history(MESSAGE_LOG_CAP);
    let mut rows = history_rows(&entries, selected);
    rows.push(text_row(""));
    rows.push(text_row(format!(
        "The last {MESSAGE_LOG_CAP} lines, repeats folded. A finished intrusion keeps its results, not its blow-by-blow."
    )));
    rows.push(text_row("Up/Down to scroll, Esc to close."));
    draw_popup("History", PopupSize::Large, &rows, painter, m);
}

/// The scrollable body of the history screen: one row per folded entry (see
/// `Game::message_history`), oldest first to match the map's pane, each in its
/// `MessageKind` colour through the same `message_color` that pane's
/// `draw_message_line` calls.
///
/// Rows are `Row::Item` because that is what `popup_layout` scrolls; the
/// highlight is the scroll position, not a selection, and
/// `App::handle_history_key` accepts nothing that would pick one. Which makes
/// the row count load-bearing: app-core counts the same folded entries to
/// bound that highlight, so a row here without one there is a highlight
/// pointing at nothing.
fn history_rows(entries: &[LogEntry], selected: usize) -> Vec<Row> {
    if entries.is_empty() {
        return vec![text_row("Nothing has happened yet.")];
    }
    entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            counted_item_row(
                &entry.text,
                entry.repeats,
                i == selected,
                message_color(entry.kind),
            )
        })
        .collect()
}

/// The zone map: terrain, entities and effects, drawn top-down into the pane
/// at the origin. The other half of the pane's contents is `draw_dungeon`,
/// which replaces this entirely while the party is underground.
#[allow(clippy::too_many_arguments)]
fn draw_surface_map(
    game: &mut Game,
    fx: &mut Fx,
    painter: &Painter,
    map_w: f32,
    map_h: f32,
    tile_px: f32,
    glyph_px: u16,
    status: &feral_processes_engine::PlayerStatus,
) {
    // One ring wider than the pane can show, so the camera's sub-tile offset
    // has a tile to slide in from instead of a blank trailing edge. Every
    // grid-to-world conversion below goes through `hw`/`hh` for that reason —
    // the ring shifts the whole grid by one cell.
    let half_w = ((map_w / tile_px) / 2.0).max(1.0) as i32;
    let half_h = ((map_h / tile_px) / 2.0).max(1.0) as i32;
    let hw = half_w + 1;
    let hh = half_h + 1;

    let (off_x, off_y) = fx.camera_offset(status.position, painter.delta());
    let tiles = game.view_tiles(hw, hh);
    let entities: Vec<_> = game
        .view_entities(hw, hh)
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
            // The extra ring costs a tile of leading offset, so the pane
            // frames the same view it did before the camera existed.
            let px = (rx as f32 - 1.0 - off_x) * tile_px;
            let py = (ry as f32 - 1.0 - off_y) * tile_px;
            let mut staffed = false;
            let mut shielded = false;
            let mut critical = false;
            let mut occupied = false;
            for ev in &entities {
                let erx = ev.pos.0 - status.position.0 + hw;
                let ery = ev.pos.1 - status.position.1 + hh;
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
                    occupied = true;
                }
            }
            let world = (
                status.position.0 + rx as i32 - hw,
                status.position.1 + ry as i32 - hh,
            );
            // Bare ground only. Where something is standing, the background
            // carries the damage-dimmed glyph colour, and jittering that
            // would muddy a structure's durability read.
            let shade = if occupied { 1.0 } else { tile_shade(world) };
            let vig = vignette(
                px + tile_px / 2.0 - map_w / 2.0,
                py + tile_px / 2.0 - map_h / 2.0,
                map_w / 2.0,
                map_h / 2.0,
            );
            let dim = shade * vig;
            let mut bg = Color::new(
                bg_source.r * 0.18 * dim,
                bg_source.g * 0.18 * dim,
                bg_source.b * 0.18 * dim,
                1.0,
            );
            if critical {
                bg = Color::new((bg.r + 0.18).min(1.0), bg.g, bg.b, bg.a);
            }
            painter.rect(px, py, tile_px - 1.0, tile_px - 1.0, bg);
            // The glyph takes the vignette but not the shade: depth should
            // apply to everything on the map evenly, while per-tile jitter is
            // a property of the ground, not of what stands on it.
            let color = Color::new(color.r * vig, color.g * vig, color.b * vig, color.a);
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
            let spawn_rx = spawn_point.0 - status.position.0 + hw;
            let spawn_ry = spawn_point.1 - status.position.1 + hh;
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
            if let Some(flash) = fx.tile_flash(world) {
                painter.rect(px, py, tile_px - 1.0, tile_px - 1.0, flash);
            }
        }
    }
    painter.rect_lines(0.0, 0.0, map_w, map_h, 2.0, BORDER);
}

fn draw_status_panel(
    rect: Rect,
    status: &feral_processes_engine::PlayerStatus,
    buffs: &[feral_processes_engine::ActiveBuffView],
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

    // Computed ahead of the routines section (rather than just above the
    // inventory loop, which is the only place that used to need it) so
    // `draw_status_buffs` can clip its own rows against the same footer
    // the inventory list already does — a party running routines on every
    // slot of every holder can outgrow the column exactly the way a full
    // inventory can.
    let keys = [
        "hjkl/arrows move  . wait  e drain  r recharge",
        "g scan   c compile   b deploy   w cronjob  G guard  R demolish",
        "u symlink   i inspect   v inventory",
        "p companions  f fuse  t trade  x perks  a routine",
        "s save   q main menu   ? help   +/- zoom",
    ];
    let keys_line_height = m.line_height - m.gap;
    let keys_block_h = keys.len() as f32 * keys_line_height + m.inset;
    let keys_y = y + h - keys_block_h;

    cy = draw_status_buffs(buffs, x + m.inset, cy, keys_y, painter, m);
    painter.ui("Inventory:", x + m.inset, cy, m.font_size, TEXT);
    cy += m.line_height;
    if status.inventory.is_empty() {
        painter.ui("(empty)", x + m.inset, cy, m.font_size, TEXT_DIM);
        cy += m.line_height;
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(text: &str, repeats: usize) -> LogEntry {
        LogEntry {
            kind: MessageKind::Info,
            text: text.to_string(),
            repeats,
        }
    }

    fn suffix_of(row: &Row) -> Option<&str> {
        match row {
            Row::Item { suffix, .. } => suffix.as_deref(),
            _ => None,
        }
    }

    /// The count is an annotation on the row, not part of the sentence — so it
    /// only appears where there is something to count.
    #[test]
    fn a_folded_row_carries_its_count_and_a_lone_row_carries_none() {
        let entries = vec![entry("extracted 2 Data Shard.", 14), entry("a raid", 1)];
        let rows = history_rows(&entries, 0);
        assert_eq!(rows.len(), 2, "one row per entry, folded or not");
        assert_eq!(suffix_of(&rows[0]), Some("×14"));
        assert_eq!(suffix_of(&rows[1]), None);
    }

    #[test]
    fn an_empty_history_says_so_instead_of_listing_nothing() {
        let rows = history_rows(&[], 0);
        assert_eq!(rows.len(), 1);
        assert!(matches!(rows[0], Row::Text(_)), "nothing to scroll to");
    }

    #[test]
    fn tile_shade_is_stable_for_a_given_world_coordinate() {
        // The camera slides continuously over a tile now. A shade derived
        // from anything but the world coordinate would shimmer as it went.
        assert_eq!(tile_shade((12, -7)), tile_shade((12, -7)));
    }

    #[test]
    fn tile_shade_stays_within_the_jitter_band() {
        for x in -60..60 {
            for y in -60..60 {
                let s = tile_shade((x, y));
                assert!(
                    (1.0 - SHADE_JITTER..=1.0 + SHADE_JITTER).contains(&s),
                    "shade {s} at ({x}, {y}) escaped the band"
                );
            }
        }
    }

    #[test]
    fn tile_shade_actually_varies_between_neighbours() {
        let row: Vec<f32> = (0..40).map(|x| tile_shade((x, 5))).collect();
        let first = row[0];
        assert!(
            row.iter().any(|s| (s - first).abs() > SHADE_JITTER / 4.0),
            "a whole row came out flat — the hash isn't spreading"
        );
    }

    /// A hash that treats the axes alike bands the map along the diagonal,
    /// which reads as a pattern rather than as texture.
    #[test]
    fn tile_shade_distinguishes_the_two_axes() {
        assert_ne!(tile_shade((3, 9)), tile_shade((9, 3)));
    }

    #[test]
    fn the_vignette_leaves_the_centre_of_the_pane_untouched() {
        assert_eq!(vignette(0.0, 0.0, 400.0, 300.0), 1.0);
    }

    #[test]
    fn the_vignette_bottoms_out_at_its_floor_and_never_below() {
        assert!((vignette(400.0, 0.0, 400.0, 300.0) - VIGNETTE_MIN).abs() < 1e-6);
        // The corners sit past the unit radius and must clamp rather than
        // keep darkening.
        assert!(vignette(400.0, 300.0, 400.0, 300.0) >= VIGNETTE_MIN);
        assert!(vignette(9999.0, 9999.0, 400.0, 300.0) >= VIGNETTE_MIN);
    }

    #[test]
    fn the_vignette_darkens_monotonically_outward() {
        let mut previous = f32::MAX;
        for i in 0..=20 {
            let v = vignette(i as f32 * 20.0, 0.0, 400.0, 300.0);
            assert!(
                v <= previous,
                "brightened at step {i}: {v} after {previous}"
            );
            previous = v;
        }
    }

    /// Normalising by the pane's half-extent is what keeps the gradient the
    /// same shape at every zoom step and window size.
    #[test]
    fn the_vignette_depends_on_position_within_the_pane_not_on_its_size() {
        let small = vignette(100.0, 75.0, 400.0, 300.0);
        let large = vignette(200.0, 150.0, 800.0, 600.0);
        assert!((small - large).abs() < 1e-6, "{small} vs {large}");
    }
}
