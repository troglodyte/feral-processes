use super::*;

/// The world grid, status panel, and message feed — the base layer shown
/// under `Mode::Playing` and every menu popup, same as `ui.rs::render_playing`.
/// The map screen and everything ambient around it. `status` is the
/// player's last refusal, and it is `Some` only when this *is* the screen —
/// every mode that draws a popup over the map shows the refusal inside that
/// popup instead, and drawing it here as well would put the same sentence
/// on screen twice.
pub(crate) fn draw_playing_base(
    app: &mut App,
    fx: &mut Fx,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
    reveal: bool,
) {
    let (tile_px, glyph_px) = map_cell(app.zoom);
    let status_line = refusal.map(str::to_string);
    // Read before the `game` borrow, like `status_line` above.
    let stack_zoom = app.stack_zoom;
    let clock = if app.paused {
        hud::status_bar::Clock::Paused
    } else {
        hud::status_bar::Clock::Running(app.world_speed.multiple())
    };
    // The one derivation of the five regions this screen draws into. Reads
    // no `Game`, so it is computed before the borrow below like every other
    // value gathered here.
    let char_w = painter.measure_ui_advance("M", m.font_size);
    let regions = hud::layout::regions(
        painter.screen_w(),
        painter.screen_h(),
        char_w,
        m,
        app.log_expanded,
    );
    // The pane's rows, chosen by app-core (see `pane_rows`), and the header
    // that says which channel they are. Both read before the `game` borrow.
    //
    // Counted in *entries* against a pane measured in rows, so the two
    // diverge the moment one entry wraps — `draw_log_pane` cuts from the
    // oldest end, so over-asking costs nothing and under-asking loses the
    // newest news. Two rows come off the top: the keybar's, and the filter
    // header's, which is the pane's first body row (`LOG_FILTER_ROWS`).
    let log_capacity = ((regions.log_pane.h - m.line_height * (1.0 + hud::layout::LOG_FILTER_ROWS))
        / m.line_height)
        .max(1.0) as usize;
    let log_lines = app.visible_log(log_capacity);
    let log_filter = app.log_filter;
    // Before the `game` borrow, like `log_filter` above.
    let info_tab = app.info_tab;
    let filtered_out = app.filtered_out_log_lines();
    // Before the `game` borrow, like `status_line` above. `None` outside
    // `Mode::Excavate` and its brush picker, which is what keeps the cursor
    // off the map the rest of the time without the renderer having to know
    // the mode's rules.
    let plan = app
        .excavate_cursor
        .filter(|_| {
            matches!(
                app.mode,
                Mode::Excavate | Mode::ExcavateBrush | Mode::Stockpile | Mode::WallBox
            )
        })
        .map(|cursor| PlanCursor {
            cursor,
            anchor: app.excavate_anchor,
        });
    // Before the `game` borrow, `plan`'s rule. No mode gate needed: a walk
    // is only ever set on `Mode::Playing`, and any key that leaves the map
    // ends it.
    let travel = app.travel_goal();
    // Before the `game` borrow, like `plan` and `log_filter` above — and
    // this one is a *read that releases*: `App::watch_center` drops
    // `App::watching` the moment the engine stops answering, which is the
    // single rule behind "the program was dissolved, dispatched, taken into
    // the party, or the party left base space".
    let watch = app.watch_center();
    let watching = app.watching;
    // After `watch_center`, which is what releases a camera that stopped
    // answering: the bar reads `watching` as it stands.
    let map_keybar = app.map_keybar();
    // Before the `game` borrow, `plan`'s rule. The preview is a **call**
    // into the geometry that resolves the routine, never a second
    // derivation, so a drawn blast and a delivered one cannot differ.
    let tactical_cursor = app.tactical_cursor;
    let tactical_preview: Vec<(i32, i32)> = match (app.tactical_cursor, app.pending_tactical) {
        (Some(aim), Some(feral_processes_app_core::TacticalIntent::Routine(index))) => app
            .game
            .as_mut()
            .map(|g| g.tactical_shape_cells(index, aim))
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    // Before the `game` borrow, `tactical_preview`'s rule and its own
    // reason: `tactical_placeable_cells` is empty for every shape but
    // `Radius` (the engine's own gate), so this needs no shape check here —
    // it is a call, not a second "is this a splash" test.
    let tactical_placeable: Vec<(i32, i32)> = match (app.tactical_cursor, app.pending_tactical) {
        (Some(_), Some(feral_processes_app_core::TacticalIntent::Routine(index))) => app
            .game
            .as_mut()
            .map(|g| g.tactical_placeable_cells(index))
            .unwrap_or_default(),
        // A relocation's two cursors outline the two sets its own door
        // refuses against — the bodies at arm's length, then the cells that
        // body may be sent to. Calls, not a second derivation: the outline
        // and the commit must not disagree about one cell.
        (Some(_), Some(feral_processes_app_core::TacticalIntent::TeleportSubject(_))) => app
            .game
            .as_ref()
            .map(|g| g.teleport_subjects())
            .unwrap_or_default(),
        (Some(_), Some(feral_processes_app_core::TacticalIntent::TeleportTo { subject, .. })) => {
            app.game
                .as_ref()
                .map(|g| g.teleport_destinations(subject))
                .unwrap_or_default()
        }
        _ => Vec::new(),
    };
    // Before the `game` borrow like the two above, and both are calls into
    // app-core's own rules: the path is the walk the click would commit and
    // the mark is read off the live board, so the board draws what a click
    // would do rather than a second guess at it.
    let tactical_overlay = tactical::Overlay {
        cursor: tactical_cursor,
        preview: tactical_preview,
        placeable: tactical_placeable,
        path: app.tactical_hover_path(),
        target: app.tactical_target_cell(),
    };
    let mode = app.mode;
    // A finished tactical fight's results are a popup over the board it was
    // fought on, and `TacticalBattle` went with the fight — so on that
    // screen the board is the copy the engine kept, and it counts as a
    // fight for everything the map pane gates on one.
    let finished_fight = mode == Mode::TacticalResult;
    let in_tactical = finished_fight || app.game.as_ref().is_some_and(|g| g.in_tactical_battle());
    // The keybar's content for as long as a fight is open, built before the
    // `game` borrow like every other read on this list. None on the results
    // screen: the popup says how to leave, and the frozen board's actor-less
    // view would read as the wild side still moving.
    let tactical_auto = app.tactical_auto;
    let tactical_actions: Option<Vec<(String, String)>> = app
        .game
        .as_mut()
        .filter(|_| !finished_fight)
        .and_then(|g| g.tactical_view())
        .map(|v| tactical::keybar_rows(mode, &v, tactical_auto, tactical_cursor));
    // A fight's bar replaces the map's whole, as one group.
    let keybar = tactical_actions.map_or(map_keybar, |actions| vec![actions]);
    let Some(game) = &mut app.game else { return };

    let stock_rows = game.base_stock();
    let research = game.research_readout();
    // One call, shared by every surface that reads it — the status bar's
    // badge here, and the info column's tab markers and collapsed bars once
    // phase 4's column lands. A second derivation is what would make "a
    // closed pane cannot hide an actionable state" a coincidence.
    let attention = game.attention();
    let status = game.player_status();
    // `Game::active_buffs` needs `&mut self`; fetched here rather than
    // inside `draw_status_panel`, which only ever needed `&Game` before
    // this and shouldn't have to start borrowing mutably just to draw.
    let buffs = game.active_buffs();
    // Read here, like `stock_rows`/`attention`/`status`/`buffs` above,
    // rather than inline at the surface branch's `draw_map_frame` call —
    // `Game::terrain_row` takes `&mut self` and this is where every other
    // such call already lands before the borrows below.
    let terrain = game.terrain_row();
    // Read here for `terrain`'s reason. `None` unless the camera is actually
    // on something, so the frame's ground readout keeps its mount for all
    // the time nobody is watching anything.
    let watch_label = watching.map(|e| game.creature_label(e));
    // Mounted on the log pane's top border by `draw_log_pane` at the end of
    // this function, so it is built here — before the `game` borrow it needs
    // — and held until then.
    let vitals = hud::log_frame::Vitals {
        status: &status,
        mining: game.mining(),
    };
    // Read here for `terrain`'s reason — it takes `&mut self`, and this is
    // where every other such call already lands before the borrows below.
    // `None` off the zone surface falls out of the engine's own answer
    // rather than from a second `Locale` check here, and `None` is also
    // what "nothing is selected" looks like.
    let compass = game.compass_bearing();
    // **Ahead of both**, because it is the most specific state: a tactical
    // fight is surface-only, so it can never be open at the same time as a
    // Stack view, and the order says which grid wins rather than leaving it
    // to two conditions agreeing.
    let board = if finished_fight {
        game.tactical_result_view()
    } else {
        game.tactical_view()
    };
    // Cleared here, unconditionally, ahead of the three-way branch below:
    // `draw_surface_map` and `draw_tactical_map` each overwrite it with
    // `Some(..)` in their own branch, so a Stack corridor leaves it `None`
    // rather than a click inheriting a layout from the screen the player
    // has since left.
    fx.set_map_click(None);
    if let Some(view) = board {
        let washes = tactical::draw_tactical_map(
            &view,
            &tactical_overlay,
            fx,
            painter,
            regions.map_pane,
            tile_px,
            glyph_px,
        );
        hud::map_frame::draw_map_frame(
            regions.map_pane,
            terrain,
            hud::map_frame::Threat {
                hostiles: view.bodies.iter().filter(|b| b.is_hostile).count(),
                shielded: game.raid_defense_active(),
            },
            // The camera is on the acting body and says so through the turn
            // strip below, which names it — a second label on the frame's
            // own mount would say it twice.
            None,
            painter,
            m,
        );
        // **The compass block's slot, and never both.** A block inside the
        // pane costs no layout — the compass moved inside for exactly that
        // reason — and a fight has no destination for a bearing to point
        // at, so the two can never want it at once.
        tactical::draw_turn_strip(&view, regions.map_pane, painter, m);
        tactical::draw_wash_legend(&washes, regions.map_pane, painter, m);
        tactical::draw_round_banner(fx, regions.map_pane, painter, m);
    } else if let Some(view) = game.stack_view() {
        draw_stack(
            &view,
            painter,
            regions.map_pane,
            m,
            status.power,
            status.max_power,
        );
        // Over the corridor, not part of it: the same map the `g` screen
        // draws, small enough to leave the view readable.
        if let Some(map) = game.frame_map() {
            draw_map_inset(&map, stack_zoom, painter, regions.map_pane, m);
        }
        // No surface entities are fetched down here, so there is nothing to
        // count hostiles among — the threat readout names what the surface
        // map shows, not what a stack frame's own view holds. `None` for the
        // same reason: a Stack frame has no biome for the ground readout to
        // name.
        hud::map_frame::draw_map_frame(
            regions.map_pane,
            None,
            hud::map_frame::Threat {
                hostiles: 0,
                shielded: game.raid_defense_active(),
            },
            // Never underground: watching is base space's, and the Stack
            // view is a corridor projection with no camera to move.
            None,
            painter,
            m,
        );
    } else {
        let entities = draw_surface_map(
            game,
            fx,
            painter,
            regions.map_pane,
            tile_px,
            glyph_px,
            &status,
            plan,
            travel,
            // The party's own cell unless the camera has been sent somewhere.
            watch.unwrap_or(status.position),
            reveal,
        );
        let hostiles = entities.iter().filter(|e| e.is_hostile).count();
        hud::map_frame::draw_map_frame(
            regions.map_pane,
            terrain,
            hud::map_frame::Threat {
                hostiles,
                shielded: game.raid_defense_active(),
            },
            watch_label.as_deref(),
            painter,
            m,
        );
        // **In this branch alone**, after the frame so it sits over the map:
        // the Stack's frame map owns this corner underground, and a
        // tactical board is the fight.
        hud::stock_block::draw_stock_block(
            regions.map_pane,
            research.as_ref(),
            &stock_rows,
            game.downed_store(),
            painter,
            m,
        );
    }
    // **After the frame, so it sits over the map rather than under it**, and
    // outside the branch because it is the same block in both — though
    // `compass_bearing` answers `None` underground, so the Stack never in
    // fact draws one.
    if let Some(row) = &compass
        && !in_tactical
    {
        hud::compass_block::draw_compass_block(regions.map_pane, row, painter, m);
    }

    // `structure_report` walks every structure — gathered behind the same
    // test the pane draws the BASE blocks behind, rather than every frame
    // the party has no base to report on.
    let has_base = game.has_home() && !game.is_underground();
    let (structures, builds, labour) = if has_base {
        (
            game.structure_report(),
            game.build_order_report(),
            game.labour_demand(),
        )
    } else {
        (Vec::new(), Vec::new(), Default::default())
    };
    let pets = game.owned_pets();
    // Unlike the BASE blocks this is not gated on `has_base`: a contract reads
    // from anywhere, and it is `&self` over at most `MAX_ACTIVE_CONTRACTS`
    // rows. `Game::contract_board` is the expensive one and is not called.
    let contracts = game.active_contracts();
    // `Game::copy_name` is the one place a copy's name is built, and it needs
    // the borrow this data outlives — so the names are resolved here rather
    // than the pane being handed a `Game`.
    let pack: Vec<hud::panes::PackRow> = status
        .inventory
        .iter()
        .map(|row| hud::panes::PackRow {
            qty: row.qty,
            name: game.copy_name(&row.copy),
            tier: row.copy.tier,
        })
        .collect();
    let pane = hud::panes::PaneData {
        roster: (status.pet_count, status.pet_capacity),
        carrying: status.inventory_used,
        buffs: &buffs,
        pack: &pack,
        pets: &pets,
        moods: game.staff_moods(),
        structures: &structures,
        stock: &stock_rows,
        builds: &builds,
        labour,
        contracts: &contracts,
        shielded: game.raid_defense_active(),
        has_base,
    };

    // The column draws its own fill and frame and hands back the body rect;
    // the open pane's rows are then fitted to it and drawn. The column does
    // not scroll, so `fitting_rows` counts what did not fit rather than
    // letting it fall off the bottom edge in silence.
    let body = hud::column::draw_info_column(
        regions.info_column,
        &hud::column::ColumnState {
            tab: info_tab,
            attention: &attention,
            pane: &pane,
        },
        painter,
        m,
    );
    let pane_rows = hud::panes::rows(info_tab, &pane);
    let (shown, cut) = hud::panes::fitting_rows(&pane_rows, body.h, m);
    hud::panes::draw_rows(body, &shown, cut, painter, m);

    hud::status_bar::draw_status_bar(
        regions.status_bar,
        &hud::status_bar::StatusBarState {
            zone: status.zone,
            position: status.position,
            tick: game.current_tick(),
            clock,
            power: game.base_power(),
            attention: &attention,
            unread_alerts: game.unread_alerts(),
            overloaded: {
                let (load, cap) = game.neural_load();
                load > cap
            },
        },
        painter,
        m,
    );

    hud::log_frame::draw_log_pane(
        regions.log_pane,
        &hud::log_frame::LogPane {
            entries: &log_lines,
            filter: log_filter,
            filtered_out,
            vitals: &vitals,
            refusal: status_line.as_deref(),
            border: fx.log_border(hud::palette::PANE_BORDER),
            keybar: &keybar,
        },
        painter,
        m,
    );
}
