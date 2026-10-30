use super::*;

/// The zone map: terrain, entities and effects, drawn top-down into the pane
/// at the origin. The other half of the pane's contents is `draw_stack`,
/// which replaces this entirely while the party is underground.
///
/// Returns the entities it drew — `draw_map_frame`'s threat readout counts
/// hostiles among them, and hoisting them out here is what keeps that a
/// second look at a Vec already built rather than a second call to
/// `Game::view_entities`.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_surface_map(
    game: &mut Game,
    fx: &mut Fx,
    painter: &Painter,
    pane: Rect,
    tile_px: f32,
    glyph_px: u16,
    status: &feral_processes_engine::PlayerStatus,
    plan: Option<PlanCursor>,
    travel: Option<TravelGoal>,
    center: (i32, i32),
    reveal: bool,
) -> Vec<EntityView> {
    // Two rings wider than the pane can show. The first is the tile the
    // camera's sub-tile offset slides in from, without which the trailing
    // edge goes blank; the second exists so *that* tile has neighbours of its
    // own to compare biomes with, since a tile cannot know whether it sits on
    // the edge of the walkable world without seeing what is beside it. Every
    // grid-to-world conversion below goes through `hw`/`hh` for that reason —
    // the rings shift the whole grid by two cells.
    let half_w = ((pane.w / tile_px) / 2.0).max(1.0) as i32;
    let half_h = ((pane.h / tile_px) / 2.0).max(1.0) as i32;
    let hw = half_w + RINGS;
    let hh = half_h + RINGS;

    // Hoisted out of the tile loop: it is one reading of one reserve, and the
    // whole grid is drawn against the same one.
    let floor = vignette_floor(status.power, status.max_power);
    let (off_x, off_y) = fx.camera_offset(center, painter.delta(), Some(crate::fx::CAMERA_MAX_LAG));
    let tiles = game.view_tiles_at(center, hw, hh);
    // Same indexing as `tiles`, `Game::view_finishes_at`'s own guarantee —
    // read once per frame, beside it, rather than per tile.
    let finishes = game.view_finishes_at(center, hw, hh);
    // Same shape and the same reason: a fresh `QueryState` and a cloned
    // `Vec` per tile inside this loop cost ~2,500 `QueryState`
    // constructions and ~1,250 allocations a frame at a typical pane's tile
    // count — `view_finishes_at`'s own precedent, read once here instead.
    let structure_floor = game.view_structure_floor_at(center, hw, hh);
    let pinned = game.view_pinned_at(center, hw, hh);
    // Same indexing again. Everywhere off the surface this is all `InSight`,
    // so nothing below carries a locale check of its own.
    let sights = game.sight_view_at(center, hw, hh);
    let radius = game.perception_radius();
    let entities: Vec<_> = game
        .view_entities_at(center, hw, hh)
        .into_iter()
        // A tamed program is drawn while it is out on an errand, while it is
        // on a job with no glyph at the far end — a builder walking its
        // materials over and raising them, a digger cutting a wall — and
        // while it is loitering with no job at all. At a *machine's* post it
        // sits under that machine's own glyph, so a base at rest reads as
        // buildings and motion is the only thing that draws the eye. A guard
        // and a party member stay hidden for a harder reason: neither is
        // ever walked, so each keeps whatever tile it was standing on when
        // it took the job — out on the surface, or four frames down — and
        // drawing it would claim it is somewhere it isn't.
        //
        // Which *space* a program stands in is a separate question and is
        // already settled inside `view_entities`
        // (`Game::stands_inside_the_base`); this one is only ever asked about
        // programs the party is in the same space as.
        //
        // Through the engine's predicate rather than spelled out here,
        // because `Game::find_target_in_direction` filters its ray with the
        // same rule so that `x` can only name what this draws.
        .filter(|e| drawn_on_surface_map(e.is_tamed, e.position_is_honest))
        // The fog: what is out of sight is not drawn, and so is not counted
        // by the Threat readout or chased by a travel goal either — both
        // read this Vec.
        .filter(|e| {
            let sight = sights
                .get((e.pos.1 - center.1 + hh) as usize)
                .and_then(|row| row.get((e.pos.0 - center.0 + hw) as usize))
                .copied()
                .unwrap_or(Sight::Unseen);
            shown_at(sight, e)
        })
        .collect();
    let marked = game.marked_cells();
    let stockpiles = game.stockpile_cells();
    // Read once for the whole map rather than per tile.
    let cutting = game.mining();
    let farm_tiles = game.farm_tiles();
    // Each plot cell's ambient `dim`, recorded by the tile loop so the plot
    // pass after it scales its outline and crop by the same light the
    // ground beneath took.
    let mut plot_dim: std::collections::HashMap<(i32, i32), f32> = Default::default();
    let floor_marks: std::collections::HashMap<(i32, i32), (char, Color)> = game
        .floor_items()
        .into_iter()
        .map(|item| (item.pos, floor_item_mark(&item.look)))
        .collect();
    // Alt shows the rooms.
    let rooms = if reveal {
        game.view_rooms()
    } else {
        Vec::new()
    };
    let room_tint: std::collections::HashMap<(i32, i32), (u8, u8, u8)> = rooms
        .iter()
        .flat_map(|r| r.cells.iter().map(|&c| (c, r.tint)))
        .collect();
    // Outposts are field fixtures; the Stack draws through `render/stack.rs`
    // and never reaches this function.
    let outpost_marks = game.outpost_marks();
    // Which of those tiles already carries an entity's own glyph and marks
    // — `draw_outpost_marks`'s own doc: an outpost's glyph must not paint
    // over the player's `@` or a wild program standing on its tile, and its
    // pips share the nemesis mark's top-right corner (`marks.rs`'s corner
    // census), so a nemesis on the tile has to win that corner. The growth
    // bar needs no such check — it holds the bottom edge, and every corner
    // mark that could share it is built to lift clear of that edge instead.
    let mut outpost_occupants: std::collections::HashMap<(i32, i32), bool> = Default::default();
    for ev in &entities {
        outpost_occupants
            .entry(ev.pos)
            .and_modify(|nemesis| *nemesis |= ev.nemesis)
            .or_insert(ev.nemesis);
    }
    let shield_outline = fx.shield_outline(game.raid_defense_active());

    // Stashed for the pointer system, `MapClickLayout`'s own doc: these are
    // exactly `tile_origin_px`'s arguments for this frame, including
    // `off_x`/`off_y` after they have already eased toward their target —
    // a click has to invert the frame it was actually drawn against.
    let layout = MapClickLayout {
        pane,
        center,
        half: (half_w, half_h),
        off: (off_x, off_y),
        tile_px,
    };
    fx.set_map_click(Some(layout));

    painter.rect(
        pane.x,
        pane.y,
        pane.w,
        pane.h,
        Color::new(0.03, 0.03, 0.05, 1.0),
    );
    for (ry, row) in tiles.iter().enumerate() {
        for (rx, tile) in row.iter().enumerate() {
            // An exposed rock face is brighter than the hole it is part of,
            // and *only* brighter: scaled before `biome_tint` is applied, so
            // a seam stays inside the impassable band. Hue is already spoken
            // for by passability, which is why a kind cannot author one.
            let biome_color = match tile.rock_shade {
                Some(shade) => brighten(biome_tint(tile.biome), shade),
                None => biome_tint(tile.biome),
            };
            // Terrain no longer carries a glyph — the biome is drawn as
            // geometry — so this stays `None` unless something is standing
            // here. That is the whole division of labour on this map:
            // terrain is shapes, actors are glyphs.
            let mut ch = None;
            let mut color = biome_color;
            let mut bg_source = biome_color;
            let world = (center.0 + rx as i32 - hw, center.1 + ry as i32 - hh);
            let (px, py) = tile_origin_px(
                world,
                center,
                (half_w, half_h),
                (off_x, off_y),
                tile_px,
                pane,
            );
            // The fetched rings exist to be *read* — by the camera slide and
            // by `draw_tile_edges` — not to be drawn. Nothing clips this pane,
            // and the log panel below it is drawn at 0.95 alpha, so a row
            // sitting past the bottom edge shows through it as a band of
            // terrain behind the text. Culling here rather than shrinking the
            // grid keeps every visible tile's neighbours in hand.
            if px >= pane.x + pane.w
                || py >= pane.y + pane.h
                || px + tile_px <= pane.x
                || py + tile_px <= pane.y
            {
                continue;
            }
            let sight = sights[ry][rx];
            if sight == Sight::Unseen {
                painter.rect(px, py, tile_px - 1.0, tile_px - 1.0, FOG_UNSEEN);
                continue;
            }
            let mut machine_status = None;
            let mut linked_edges: &[(i32, i32)] = &[];
            let mut shielded = false;
            let mut critical = false;
            // A structure and an actor genuinely share a tile now that
            // posted workers are drawn: `place_structure` never makes its
            // tile unwalkable, and a hauling program's route to a depot
            // crosses the base slab. So the two are gathered separately
            // rather than last-one-wins, which resolved by
            // `view_entities` iteration order and would have flickered a
            // worker in and out of existence as it walked over a machine.
            let mut structure: Option<&EntityView> = None;
            let mut actor: Option<&EntityView> = None;
            // A third category beside those two, and it has to be: a build
            // site is not a `Structure`, so the `is_structure` test above
            // would drop it into `actor` — where it would either be hidden
            // by the builder standing on it or hide the builder itself. It
            // is ground being worked, which is what the frame below draws it
            // as.
            let mut building: Option<&EntityView> = None;
            // The entity wearing the mark, and whether its work has hit the
            // dead end below.
            let mut mark: Option<(Entity, bool)> = None;
            for ev in &entities {
                let erx = ev.pos.0 - center.0 + hw;
                let ery = ev.pos.1 - center.1 + hh;
                if erx != rx as i32 || ery != ry as i32 {
                    continue;
                }
                // A machine being upgraded carries its own pending row, so
                // `build` alone no longer means "nothing is standing here
                // yet": tested first, a working machine would draw as a bare
                // build slab for as long as its upgrade was on order.
                if ev.build.is_some() && !ev.is_structure {
                    building = Some(ev);
                } else if ev.is_structure {
                    structure = Some(ev);
                } else if !matches!(actor, Some(a) if a.is_player) {
                    actor = Some(ev);
                }
                if if ev.is_structure {
                    ev.structure_attended
                } else {
                    ev.wears_job_mark
                } {
                    mark = Some((ev.entity, ev.output_stranded));
                }
            }
            let occupied = structure.is_some() || actor.is_some();
            if let Some(ev) = structure {
                machine_status = ev.machine_status;
                linked_edges = &ev.linked_edges;
                shielded = ev.shielded;
                ch = Some(ev.glyph);
                // Structures wear their raid damage: the glyph dims as
                // durability drops, and a nearly-destroyed one washes
                // its tile red, so the base's condition reads at a
                // glance instead of only from the inspect menu.
                let dimmed;
                (dimmed, critical) = fx.structure_condition(ev.durability, glyph_color(ev.color));
                // Background follows the damage-dimmed *authored* colour,
                // deliberately taken before the status override below.
                // The tile wash already means raid damage — a clogged
                // machine tinting its tile red too would make a
                // half-destroyed one and a full buffer look alike. Taken
                // from the structure even when someone is standing on it:
                // the wash is the building's condition, not the walker's.
                bg_source = dimmed;
                // A machine's glyph is its state: the `$` of a Mining
                // Node reads green running, dim yellow starved, the
                // attention yellow when it is asking for you, grey idle. Which structure it is stays legible from the
                // glyph itself, so the authored colour is only carrying
                // identity a machine can spare. Anything that runs no job
                // keeps its authored colour.
                //
                // Damage-tinted through the same call, so a battered
                // machine still dims rather than reading box-fresh.
                //
                // A drop pod terminal runs no job — it carries no
                // `MachineStatus` at all — so a spent one cannot ride the
                // arm above; it is checked here instead, `WARN`'s own
                // reading: the crew is already walking the bill over, so
                // waiting is what fixes it.
                color = match ev.machine_status {
                    Some(status) => {
                        fx.structure_condition(ev.durability, machine_color(status))
                            .0
                    }
                    None if ev.pod_charged == Some(false) => {
                        fx.structure_condition(ev.durability, hud::palette::WARN).0
                    }
                    None => dimmed,
                };
            }
            // An actor takes the tile's glyph off a structure and never the
            // other way round. The machine keeps every channel it has —
            // status outline, chain links, shield, damage wash — and gives
            // up only the glyph, which is the one thing that cannot show two
            // things at once.
            if let Some(ev) = actor {
                // While emulating, the tile draws the image's own glyph —
                // never the player's `@` — todo #100 Task 6. `form` is
                // `Some` for the player alone (`Game::form_look`'s only
                // writer is `components::Emulation`), so this is checked
                // ahead of `is_player` rather than folded into it.
                ch = Some(ev.form.as_ref().map_or(ev.glyph, |f| f.glyph));
                // The `@` is the one glyph whose colour is a role. Every
                // other entity wears the hue its file authored; the player
                // wears a colour off the character-creation wizard —
                // `PlayerLook::colour` is a **0-based** index into
                // `hud::palette::PLAYER_CHOICES`, and `None` is *no answer*
                // rather than a bad one: the wizard was never opened, or the
                // save predates it, so the glyph keeps the `PLAYER` role
                // colour. Read off `is_player` rather than off the authored
                // `GlyphColor::Cyan`, which a structure is free to author
                // too.
                //
                // **A boss is the other exception, and it spends the hue on
                // purpose.** Magenta is what a boss reads as, which is why
                // it is the one hostile whose con rung has to go somewhere
                // else — see `ConRead`. An emulating player spends it for
                // the same reason and the same styling — see `EntityView::
                // form`'s doc for why this isn't `is_boss: true` instead —
                // so it is checked first, ahead of `is_player`, or the
                // player's own role colour would win the tile back.
                color = if ev.form.is_some() {
                    boss_color()
                } else if ev.is_player {
                    player_look_color(ev.look.as_ref().and_then(|look| look.colour))
                } else if ev.is_boss {
                    boss_color()
                } else {
                    glyph_color(ev.color)
                };
            }
            // Under programs and structures, above the floor: a floor item
            // only takes a glyph nothing else has claimed.
            if ch.is_none()
                && let Some(&(glyph, hue)) = floor_marks.get(&world)
            {
                (ch, color) = (Some(glyph), hue);
            }
            // Bare ground only. Where something is standing, the background
            // carries the damage-dimmed glyph colour, and jittering that
            // would muddy a structure's durability read.
            // The cloud rides `shade` and not `vig`, which buys three
            // things. `occupied` already excludes it, so a shadow falls on
            // bare ground only — the rule the shade jitter and the biome
            // pattern already follow, and for their reason: a moving dim
            // over a structure would muddy the durability wash. The glyph
            // path below takes `vig` alone, so what stands on the ground
            // stays lit under a cloud. And `vignette` goes on meaning the
            // Power reserve and nothing else.
            let remembered = sight == Sight::Remembered;
            // Memory is still: the world out there is not moving for you.
            let cloud = if !remembered {
                fx.cloud_shade(world)
            } else {
                1.0
            };
            let shade = if occupied || remembered {
                1.0
            } else {
                tile_shade(world) * cloud
            };
            let vig = vignette(
                px + tile_px / 2.0 - (pane.x + pane.w / 2.0),
                py + tile_px / 2.0 - (pane.y + pane.h / 2.0),
                pane.w / 2.0,
                pane.h / 2.0,
                floor,
            ) * fog_dim(sight, world, status.position, radius, |dx, dy| {
                sights
                    .get((ry as i32 + dy) as usize)
                    .and_then(|row| row.get((rx as i32 + dx) as usize))
                    .is_some_and(|&s| s != Sight::InSight)
            });
            let dim = shade * vig;
            if !farm_tiles.is_empty() {
                plot_dim.insert(world, dim);
            }
            let mut bg = at_level(bg_source, GROUND_LEVEL * dim);
            if critical {
                bg = Color::new((bg.r + GROUND_LEVEL).min(1.0), bg.g, bg.b, bg.a);
            }
            let cell = Rect::new(px, py, tile_px - 1.0, tile_px - 1.0);
            painter.rect(cell.x, cell.y, cell.w, cell.h, bg);
            // Bare ground only, for the same reason the shade jitter above is:
            // where something is standing, the tile is carrying that thing's
            // damage-dimmed colour, and a biome pattern drawn through it would
            // muddy the durability read.
            if !occupied {
                // A finish is a pattern painted over laid floor, not a
                // second biome — `draw_biome`'s own `Platform` arm is what a
                // cell with no finish still gets.
                match (tile.biome, finishes[ry][rx].as_ref()) {
                    (Biome::Platform, Some(finish)) => draw_finish(painter, cell, finish, dim),
                    _ => draw_biome(painter, tile.biome, cell, at_level(biome_color, dim), world),
                }
                draw_tile_edges(painter, &tiles, rx, ry, cell, biome_color, vig, cloud);
            }
            // A multi-cell structure's own floor — one of its non-anchor
            // footprint cells — takes a flat fill over whatever biome was
            // just drawn underneath. `structure_floor` is `view_tiles_at`'s
            // own indexing, read once above rather than requeried here.
            if structure_floor[ry][rx] {
                painter.rect(
                    cell.x,
                    cell.y,
                    cell.w,
                    cell.h,
                    at_level(STRUCTURE_FLOOR_FILL, vig),
                );
            }
            // Under structures and above the floor, so a room reads as a
            // property of the ground.
            if let Some(&(r, g, b)) = room_tint.get(&world) {
                painter.rect(
                    cell.x,
                    cell.y,
                    cell.w,
                    cell.h,
                    Color::new(
                        r as f32 / 255.0,
                        g as f32 / 255.0,
                        b as f32 / 255.0,
                        ROOM_TINT_ALPHA,
                    ),
                );
            }
            // A structure the crew has not raised yet: a flat dark slab with
            // a darker edge, drawn over the ground and under everything that
            // stands on it.
            //
            // **A block rather than the structure's own glyph in grey.**
            // What is going up here is not the question a player glances at
            // this cell to answer — the caret says "work is happening", the
            // examine page says what and how far along, and a dim `M` would
            // read as an Extractor that had gone dark. It also keeps the
            // cell legible as *not yet a thing*: nothing else on this map is
            // a filled block, so a site is never mistaken for a machine.
            //
            // Both take the vignette and not the tile shade, matching the
            // glyph rule below: this is something standing on the ground,
            // not the ground.
            if building.is_some() {
                painter.rect(
                    cell.x,
                    cell.y,
                    cell.w,
                    cell.h,
                    at_level(BUILD_SITE_FILL, vig),
                );
                painter.rect_lines(
                    cell.x,
                    cell.y,
                    cell.w,
                    cell.h,
                    BUILD_SITE_EDGE_PX,
                    at_level(BUILD_SITE_EDGE, vig),
                );
            }
            // The glyph takes the vignette but not the shade: depth should
            // apply to everything on the map evenly, while per-tile jitter is
            // a property of the ground, not of what stands on it.
            let color = Color::new(color.r * vig, color.g * vig, color.b * vig, color.a);
            // A body a research project is spending right now rattles in its
            // pen. **The ink alone** — the offset is added to the sprite and
            // the glyph below and to nothing else, so the pen's four
            // squares, the rarity bar, the other corner marks and the
            // progress bar all keep
            // reading bare `px`/`py` and the pen stays still around it. See
            // `Fx::strain_jitter`.
            //
            // Keyed on the actor, `centred_bob`'s rule: two subjects in two
            // Stations rattle out of step.
            let (jx, jy) = match actor {
                Some(ev) if pinned[ry][rx] == PinMark::Strained => {
                    // The ink's own margin is the budget — `Fx::strain_jitter`
                    // has the argument. Recomputed rather than hoisted off the
                    // `inset` below it, which is scoped to the sprite attempt.
                    fx.strain_jitter(ev.entity, (tile_px - glyph_px as f32) / 2.0)
                }
                _ => (0.0, 0.0),
            };
            // The sprite attempt is hoisted out of the glyph draw below
            // because **its answer is what decides where the con read
            // goes** — see `ConRead::of`. `is_some_and` and not an `if let`
            // so `drew_sprite` stays a `let` rather than a `mut` seeded with
            // a default, which is the shape that silently ships wrong the
            // day a branch is added.
            let drew_sprite = ch.is_some_and(|_| {
                // A sprite *substitutes* for the glyph rather than drawing
                // beside it, so it can only appear on a tile that would have
                // had one — and a miss falls back to that glyph, which is
                // what keeps `assets/sprites/` optional.
                //
                // The anchor is the one fixture left named here. It is not
                // content — a permanent feature of every zone, with no
                // `.ron` file to carry a `sprite:` field — so its name stays
                // written in this match rather than read off the view. The
                // player's name comes off `PlayerLook` instead: the
                // character-creation wizard's own choice, in place of the
                // literal `"player"` this used to read regardless of who was
                // playing. Every other entity — a creature or a structure —
                // already carries its own resolved name on `EntityView::
                // sprite`, so there is nothing left to special-case for it.
                //
                // The anchor is the case the sprite seam was worth building
                // for. Its `#` was chosen by elimination rather than by
                // meaning — `>` and `<` were claimed by Stack links and
                // Blue and Cyan were reserved, see `Game::new`'s anchor
                // spawn — so it is the one glyph on this map that says
                // nothing at all about what stands there.
                // Whether the player has a drawing at all is read off
                // their own look and not off the table, so this says which
                // rung the tile is on rather than which keys happen to be
                // loaded. A drawing the table has nothing under — one that
                // is still uploading, or a blank canvas `sync_drawn_icon`
                // declined — misses and falls to the rung below, like any
                // other sprite.
                // `form.is_none()`: an emulating player draws the image's
                // own sprite below, in the ordinary tinted rung, and never
                // the untinted drawn icon — the overdraw trap the other
                // way round, since the two would otherwise substitute for
                // the same glyph on the same tile.
                let drawn_icon = actor.is_some_and(|ev| {
                    ev.is_player && super::wears_drawing(ev.look.as_ref(), ev.form.is_some())
                });
                // **An actor's sprite outranks a structure's, exactly as its
                // glyph already does above** — `structure` is only reached
                // when no actor stands on the tile. A creature's own
                // `EntityView::sprite` is already the resolved name (Task
                // 2's `sprite_name` fallback), so the two non-player,
                // non-anchor arms below are the same read for a creature and
                // a structure alike.
                //
                // **`form` outranks `is_player`**, todo #100 Task 6: while
                // emulating, the image's own sprite substitutes for the
                // glyph exactly as any other creature's does, and never the
                // player's chosen look — `EntityView::form`'s doc has the
                // reason it isn't `is_boss: true` instead.
                let sprite = match actor {
                    Some(ev) if ev.form.is_some() => {
                        ev.form.as_ref().and_then(|f| f.sprite.as_deref())
                    }
                    Some(ev) if ev.is_player => ev
                        .look
                        .as_ref()
                        .and_then(|look| player_sprite_name(&look.sprite)),
                    Some(ev) if ev.is_anchor => Some("anchor"),
                    Some(ev) => ev.sprite.as_deref(),
                    None => structure.and_then(|ev| ev.sprite.as_deref()),
                };
                // Centred on the cell, not on measured ink: a square sprite
                // has neither side bearing nor descender. `glyph_px` and not
                // `tile_px`, so the sprite keeps the glyph's margin inside
                // its tile and stays on the integer ladder — 16, 32, 48, 64.
                let inset = (tile_px - glyph_px as f32) / 2.0;
                // The same key `Fx`'s bobs phase on, so a worker's sprite
                // frame and its bob drift together.
                let phase_key = actor.or(structure).map_or(0, |ev| ev.entity.to_bits());
                // **The player's own drawing is the top rung, and it is the
                // one sprite in the game drawn untinted.** Every other
                // sprite is authored near-white and inherits its tile's
                // colour through egui's multiplying tint — that is what
                // `assets/sprites/README.md` asks art for. A drawing is the
                // exception on purpose: the player picked the
                // colours themselves, and multiplying those by an indigo or
                // rose swatch turns most of them black.
                //
                // So the tint here is neutral **at the vignette's value** —
                // the hue is dropped, the depth shading is kept, exactly as
                // for every other tile. What that costs is the Colour
                // step's swatch on this one tile and nothing else: the
                // `is_player` arm above sets `color` to
                // `player_look_color(...)` and nothing but `vig` touches it
                // after. `App::creation_colour_note` tells the player so on
                // the step where they choose it.
                //
                // **Putting the hue back reads as a bug fix and is not
                // one.** If this ever looks wrong, the answer is art
                // authored near-white — which a hand-drawn icon is not.
                let neutral = Color::new(vig, vig, vig, color.a);
                (drawn_icon
                    && painter.sprite(
                        crate::sprites::DRAWN_ICON_KEY,
                        px + inset + jx,
                        py + inset + jy,
                        glyph_px as f32,
                        neutral,
                        phase_key,
                    ))
                    || sprite.is_some_and(|name| {
                        painter.sprite(
                            name,
                            px + inset + jx,
                            py + inset + jy,
                            glyph_px as f32,
                            color,
                            phase_key,
                        )
                    })
            });
            // Decided once, read twice — here for the glyph's ink and below
            // for the corner. `actor` and nothing else: a structure has no
            // con read, and the engine hands `None` for everything that is
            // not hostile.
            let con = ConRead::of(
                actor.and_then(|ev| ev.difficulty),
                actor.is_some_and(|ev| ev.is_boss || ev.form.is_some()),
                drew_sprite,
            );
            if let Some(ch) = ch
                && !drew_sprite
            {
                let glyph = ch.to_string();
                let dims = painter.measure_map(&glyph, glyph_px);
                let tx = px + (tile_px - dims.width) / 2.0 + jx;
                let ty = py + (tile_px + dims.height) / 2.0 + jy;
                painter.map(&glyph, tx, ty, glyph_px, con.glyph_ink(color, vig));
            }
            // The caret, bouncing in the middle of the slab.
            //
            // Drawn here rather than through the `ch` path above because
            // that path has no vertical offset to give it — and the bounce
            // is the whole point: a build site is the one cell on the map
            // where *nothing is happening yet* is the wrong reading. It
            // reuses the raised cosine the "someone is on this job" mark
            // rides, so the two motions on this map agree with each other
            // rather than being two independently-invented curves.
            // Phase-keyed by entity for that helper's own reason: two sites
            // side by side bounce out of step and read as two jobs rather
            // than one animation.
            //
            // **`centred_bob` and not `staffed_bob`**: this caret's rest
            // position is the centre of its own slab, not an inset off the
            // tile's bottom edge, so it has room on both sides. Riding the
            // upward-only form it spent its whole cycle at or above centre
            // and read as sitting high in the tile.
            //
            // Over the glyph layer, so a builder standing on the cell is
            // drawn under its own work. Under the marks and outlines below,
            // which are all about state rather than about the tile.
            if let Some(ev) = building {
                // A wreck waiting for a program has no crew on it, so it
                // wears a still "!" rather than the working caret.
                let waiting = ev.build.as_ref().is_some_and(|b| b.awaiting_program);
                let glyph = if waiting {
                    "!".to_string()
                } else {
                    ev.glyph.to_string()
                };
                let dims = painter.measure_map(&glyph, glyph_px);
                let lift = if waiting {
                    0.0
                } else {
                    fx.centred_bob(ev.entity)
                };
                painter.map(
                    &glyph,
                    px + (tile_px - dims.width) / 2.0,
                    py + (tile_px + dims.height) / 2.0 - lift,
                    glyph_px,
                    Color::new(ORANGE.r * vig, ORANGE.g * vig, ORANGE.b * vig, ORANGE.a),
                );
            }
            // A body a Bay is mending, or one an amenity is refilling, drawn
            // on the same layer as the build caret and for the same reason: it is a mark that needs the
            // middle of the tile and a vertical offset, and the glyph path
            // above has neither to give it.
            //
            // **`actor` and not `structure`** — the mark says *this program
            // is being refilled*, so it rides the body. Which is also the
            // glyph the tile is drawing: an actor takes the tile's glyph off
            // a structure, so on a Bay with somebody standing in it the `+`
            // and the body it belongs to are the same cell's ink. Passed the
            // structure instead, a Bay whose reach is more than its own cell
            // would mark itself while the program it is healing stood
            // unmarked a tile away.
            //
            // **Base space, gated the way the raid flash is.** Both ends of
            // this stand in base space and `EntityView::recovering` is
            // derived from base-space `Position`s, so on the zone map this
            // cell's coordinates mean something else entirely — the
            // cross-space aliasing `view_entities` and the spawn-point
            // outline both refuse. Neither would be in `entities` out there
            // anyway; the gate is what keeps that an assertion rather than a
            // coincidence.
            draw_rising_mark(
                painter,
                actor,
                fx,
                Rect::new(px, py, tile_px, tile_px),
                glyph_px,
                vig,
            );
            // A rare-spawn tier draws as a bar along the top edge rather
            // than by recolouring the glyph, because the glyph's colour is
            // usually spoken for: a hostile is tinted by `difficulty_color`
            // (green through red by power ratio against the player), which
            // is the "can I win this fight" read. That read is not sacred,
            // though — a nemesis spends it on purpose (see the mark just
            // below and `EntityView::rarity`'s doc), so "cannot be given
            // up" is no longer true of it. It just doesn't happen to be
            // rarity that spends it. Two readings, two channels — the glyph
            // says how dangerous (or, for a nemesis, that you already know),
            // the bar says how rare.
            //
            // Keyed off `actor` and never `structure`: a structure has no
            // tier, and the actor is what owns this channel. Vignette but
            // not the tile shade, matching the glyph's rule above.
            //
            // Through `marks::draw_rarity_bar`, shared with the tactical
            // board so a silver or gold body reads the same colour on both
            // grids.
            if let Some(ev) = actor {
                draw_rarity_bar(painter, ev.rarity, px, py, tile_px, vig);
            }
            // The con read's **fallback** home, a wedge folded into the
            // top-left corner — `ConRead::Earmark` and nothing else, so a
            // tile that already said it with its ink cannot say it twice.
            // A shape rather than a second strip: the top edge belongs to
            // the rarity bar, and form is what tells two readings there
            // apart when hue cannot. Painted after that bar and dropped
            // clear of it — see `difficulty_mark_points`.
            //
            // **The Alt marker borrows this same corner while `reveal` is
            // held**, suppressing the earmark for that one frame rather
            // than drawing over it — `draw_unseen_marker`'s doc has the
            // argument for why the two must never share a tile.
            // `corner_marker` is the whole gate, so this loop cannot drift
            // from what it tests.
            let unseen = actor.is_some_and(|ev| ev.unseen_routine);
            let (earmark, marker) = corner_marker(con, unseen, reveal);
            draw_difficulty_mark(painter, earmark, px, py, tile_px, vig);
            if marker {
                draw_unseen_marker(painter, px, py, glyph_px, vig);
            }
            // A body under study wears four small red squares on the
            // tile-edge ring — `marks::draw_pin_marks`'s own doc has the
            // geometry.
            // Base-space only, `view_structure_floor_at`'s own reason above.
            // **`marker` gates the top-left corner here too**, extending
            // this same "the Alt marker borrows the top-left corner" rule
            // rather than inventing a second arbitration between the two —
            // see `corner_marker`'s doc. `pinned` is read once above,
            // `structure_floor`'s own reason.
            if pinned[ry][rx] != PinMark::Unpinned {
                draw_pin_marks(painter, px, py, tile_px, marker, vig);
            }
            // A nemesis draws a mark on top of its glyph — belt and braces,
            // since a nemesis is worth noticing even at a glance that only
            // catches shape and not hue. Its own corner, so a nemesis that
            // is also rare, or also a boss (all three are independent),
            // shows every one of them without any being spent to make room.
            if actor.is_some_and(|ev| ev.nemesis) {
                let mark = nemesis_mark_rect(px, py, tile_px);
                painter.rect(
                    mark.x,
                    mark.y,
                    mark.w,
                    mark.h,
                    at_level(hud::palette::glyph(GlyphColor::Cyan), vig),
                );
            }
            // A town's patrol wears its own corner, in the town's own hue —
            // `GlyphColor::Orange` is what a settlement's glyph draws in and
            // the one variant no species authors, so "this belongs to a
            // town" reads without being told and without spending the
            // identity hue this program already has.
            if let Some(ev) = actor {
                draw_mood_mark(painter, ev, px, py, tile_px, vig);
            }
            if actor.is_some_and(|ev| ev.patrol.is_some()) {
                let mark = patrol_mark_rect(px, py, tile_px);
                painter.rect(
                    mark.x,
                    mark.y,
                    mark.w,
                    mark.h,
                    at_level(hud::palette::glyph(GlyphColor::Orange), vig),
                );
            }
            // The base-space mirror of that outline: an armed bump is a mode
            // with no other trace on the screen — the log said so once, at
            // the moment it was toggled, and a player who walked away and
            // came back has nothing left to read it off. A ring rather than
            // a colour on the `@` itself, so the sprite that stands in for
            // that glyph carries the cue too.
            if cutting && actor.is_some_and(|ev| ev.is_player) {
                painter.rect_lines(px, py, tile_px - 1.0, tile_px - 1.0, 2.0, CUTTING_OUTLINE);
            }
            // While emulating, the tile's ink is spent on boss magenta —
            // todo #100 Task 6 — so the player stays findable through an
            // outline in their own colour instead. Every corner is already
            // claimed (the con earmark, the nemesis mark, the staffed mark,
            // the patrol mark), and the top-right is already cyan, which is
            // what rules out a fifth corner mark reading as "nemesis"
            // instead. `player_look_color` and not the `PLAYER` role colour
            // directly, `color`'s own fallback one line above.
            if let Some(ev) = actor
                && ev.is_player
                && ev.form.is_some()
            {
                painter.rect_lines(
                    px,
                    py,
                    tile_px - 1.0,
                    tile_px - 1.0,
                    2.0,
                    player_look_color(ev.look.as_ref().and_then(|look| look.colour)),
                );
            }
            // The shield network is base-wide, not per-structure, so every
            // structure carries the same faint pulse while one is standing.
            // Drawn under the flash so a raid still reads on top of it, and
            // through the same open-sided outline so it cannot paint a joined
            // pair's shared wall back in.
            if let Some(pulse) = shield_outline.filter(|_| shielded) {
                outline_open(painter, px, py, tile_px - 1.0, pulse, linked_edges);
            }
            // A machine wears its state as its outline, and drops the walls
            // it shares with a machine it trades with — so a working chain
            // draws as one continuous shape and a machine that should be
            // joined and isn't shows a seam. Nothing is drawn *between*
            // tiles; the join is the absence of a line.
            //
            // Drawn after the shield pulse, which also outlines every
            // structure: an actionable per-machine state has to win over
            // ambient base-wide info, or a starved machine goes unnoticed
            // inside a shielded base. A structure that runs no job has no
            // status and keeps only the pulse.
            if let Some(color) = machine_status.map(machine_color) {
                outline_open(painter, px, py, tile_px - 1.0, color, linked_edges);
            }
            // A machine with nothing set to make: a `?` in the top-right
            // quarter, after the outline so a wall never draws over it.
            if structure.is_some_and(|ev| ev.unconfigured) {
                draw_unconfigured_mark(painter, px, py, tile_px);
            }
            // A Research Station still holding its last spent subject: an
            // `x` in the top-left quarter until a body carries it away.
            if structure.is_some_and(|ev| ev.holds_spent_program) {
                draw_spent_program_mark(painter, px, py, tile_px);
            }
            // How far along the work on this cell is — see
            // `EntityView::job_progress`. **After the outline**, which is
            // drawn along `tile_px - 1.0` and two pixels thick: painted
            // before it, the bar of a machine with a status is covered by
            // that machine's own bottom wall.
            //
            // Which occupant owns the bar and what hue it wears is
            // `cell_bar`, extracted so the precedence between a machine and
            // a request standing on the same cell is testable. The actor is
            // offered as-is: staff share the field with the machines.
            if let Some((done, color)) = cell_bar(structure, building, actor) {
                draw_progress_bar(painter, Some(done), px, py, tile_px, color, vig);
            }
            // How full a Depot or Rack is, up the right edge — after the outline for
            // the progress bar's reason, since the shield pulse draws a wall
            // along that edge too. See `draw_shelf_fill`.
            draw_shelf_fill(
                painter,
                structure.and_then(|ev| ev.shelf_fill),
                px,
                py,
                tile_px,
                vig,
            );
            // Finished product sitting in a machine rather than in storage:
            // the dot that says "the items are in here". A Depot's fill
            // bar already says how full it is, and the view exempts it.
            draw_held_output(
                painter,
                structure.is_some_and(|ev| ev.holds_output),
                px,
                py,
                tile_px,
                color,
            );
            // "Someone is on this job", on a channel of its own rather than
            // sharing the outline with machine state. It was a yellow outline
            // until machines took that channel over, at which point a machine
            // could never show it at all — leaving grey `Idle` as the only
            // trace of an unstaffed one, a colour meaning absence on an axis
            // that also carries three other things. Now the outline says what
            // the machine is doing and the mark says whether anyone is on it.
            //
            // One sentence covers where it goes: **on the program when the
            // program is drawn, and on the structure when it isn't.** So a
            // machine wears it while its worker stands at its post, the
            // worker takes it along the moment it leaves to deliver, and a
            // guard — which is never drawn — leaves it on the structure for
            // good. Exactly one mark per posted program at every instant,
            // which `a_worked_machine_and_its_worker_never_both_wear_the_
            // mark` holds from the engine side.
            //
            // The machine is not left silent while its worker is out: it
            // goes `Unstaffed` on the outline channel, "its program is
            // away." And with no depot built there is no errand at all, so
            // nothing ever leaves and nothing is ever drawn — see
            // `with_no_depot_a_clogged_machine_just_stays_clogged`.
            //
            // A machine that is full with nowhere to send its output is the
            // one case where the mark stops moving: its worker will never
            // leave, so a bob would promise motion that is never coming. It
            // blinks in place instead — see `Fx::stranded_blink`.
            if let Some((marked, stranded)) = mark {
                // The attention colour as well as still: colour and motion
                // say the same thing at once, so a stranded machine is
                // legible from a paused screenshot and not only from
                // watching it. It wears the same role its glyph does,
                // because they are saying the same thing — what separates
                // this from a staffed mark is the blink and the green, and
                // what separates it from a clogged machine was never the
                // mark's hue but the fact that a clogged one still has a
                // worker to bob.
                let (lift, alpha, base) = if stranded {
                    (0.0, fx.stranded_blink(), hud::palette::ATTENTION)
                } else {
                    (fx.staffed_bob(marked), 1.0, hud::palette::HEALTHY)
                };
                let m = staffed_mark_rect(px, py, tile_px, lift);
                painter.rect(m.x, m.y, m.w, m.h, Color { a: alpha, ..base });
            }
            if let Some(flash) = fx.tile_flash(world) {
                painter.rect(px, py, tile_px - 1.0, tile_px - 1.0, flash);
            }
        }
    }
    // Over the tiles and under the plan: a plot is on the ground, and a plan
    // is drawn over it.
    // A floor item's glyph was drawn in the tile loop, so it claims the
    // cell from the crop the way an entity does.
    let occupied: std::collections::HashSet<(i32, i32)> = entities
        .iter()
        .map(|e| e.pos)
        .chain(floor_marks.keys().copied())
        .collect();
    draw_farm_plots(
        painter,
        &farm_tiles,
        &occupied,
        &plot_dim,
        |world| {
            tile_origin_px(
                world,
                center,
                (half_w, half_h),
                (off_x, off_y),
                tile_px,
                pane,
            )
        },
        (tile_px, glyph_px),
        pane,
    );
    // Over the tiles and under the sparks: a plan is a thing drawn on the
    // ground, and a burst is a thing happening above it.
    draw_excavation_plan(
        painter,
        &stockpiles,
        &marked,
        plan,
        |world| {
            tile_origin_px(
                world,
                center,
                (half_w, half_h),
                (off_x, off_y),
                tile_px,
                pane,
            )
        },
        tile_px,
        pane,
    );
    // The surface's own mirror of the plan pass above: a record with no
    // entity has no other way onto the map, `render/outposts.rs::
    // draw_outpost_marks`'s own doc.
    draw_outpost_marks(
        painter,
        &outpost_marks,
        |world| {
            tile_origin_px(
                world,
                center,
                (half_w, half_h),
                (off_x, off_y),
                tile_px,
                pane,
            )
        },
        tile_px,
        glyph_px,
        pane,
        |tile| outpost_occupants.get(&tile).copied(),
    );
    // After every tile, so a speck drifting across a tile boundary is not
    // cut by the next tile's ground. Kept off any tile something stands on
    // instead of under it, since the tile loop drew ground and glyph
    // together and there is no layer between them to slip into.
    let occupied: std::collections::HashSet<(i32, i32)> = entities.iter().map(|e| e.pos).collect();
    let specks = fx.wildlife(
        (center.0 - hw, center.1 - hh),
        (center.0 + hw, center.1 + hh),
    );
    draw_wildlife(
        painter,
        &specks,
        |world| {
            let rx = usize::try_from(world.0 - center.0 + hw).ok()?;
            let ry = usize::try_from(world.1 - center.1 + hh).ok()?;
            let tile = tiles.get(ry)?.get(rx)?;
            (tile.walkable && sights[ry][rx] == Sight::InSight && !occupied.contains(&world))
                .then(|| biome_tint(tile.biome))
        },
        |world| {
            tile_origin_px(
                world,
                center,
                (half_w, half_h),
                (off_x, off_y),
                tile_px,
                pane,
            )
        },
        |x, y| {
            vignette(
                x - (pane.x + pane.w / 2.0),
                y - (pane.y + pane.h / 2.0),
                pane.w / 2.0,
                pane.h / 2.0,
                floor,
            )
        },
        tile_px,
        pane,
    );
    // After every tile so debris lands on top of the base rather than under
    // it, and before the border so a spark from a structure at the pane's
    // edge cannot draw over the frame.
    fx.draw_bursts(painter, tile_px, |world| {
        tile_origin_px(
            world,
            center,
            (half_w, half_h),
            (off_x, off_y),
            tile_px,
            pane,
        )
    });
    // Behind the same gate as the debris, and for a sharper version of
    // the same reason: a walk names base-space cells, and a squad drawn
    // filing out of the anchor while the pane is showing the zone
    // surface would be walking across open ground the party is standing
    // on. Over the tiles, since a body walks on top of the floor.
    fx.draw_walkers(painter, tile_px, glyph_px, |world| {
        tile_origin_px(
            world,
            center,
            (half_w, half_h),
            (off_x, off_y),
            tile_px,
            pane,
        )
    });
    fx.draw_speech(painter, tile_px, glyph_px, |world| {
        tile_origin_px(
            world,
            center,
            (half_w, half_h),
            (off_x, off_y),
            tile_px,
            pane,
        )
    });
    if reveal {
        super::alt_overlay::draw_alt_labels(
            painter,
            &layout,
            &rooms,
            &entities,
            &farm_tiles,
            &structure_floor,
        );
    }
    // Over every body and effect, since it marks one of them when the walk
    // is a chase; resolved against `entities` — what this frame drew — so
    // the outline follows the hostile and never lands where nothing is.
    let destination = match travel {
        Some(TravelGoal::Tile(x, y)) => Some((x, y)),
        Some(TravelGoal::Creature(target)) => {
            entities.iter().find(|e| e.entity == target).map(|e| e.pos)
        }
        None => None,
    };
    if let Some(cell) = destination {
        let (px, py) = tile_origin_px(
            cell,
            center,
            (half_w, half_h),
            (off_x, off_y),
            tile_px,
            pane,
        );
        let culled = px >= pane.x + pane.w
            || py >= pane.y + pane.h
            || px + tile_px <= pane.x
            || py + tile_px <= pane.y;
        if !culled {
            let size = tile_px - 1.0;
            painter.rect_lines(px, py, size, size, 2.0, hud::palette::DESTINATION);
        }
    }
    painter.rect_lines(pane.x, pane.y, pane.w, pane.h, 2.0, BORDER);
    entities
}
