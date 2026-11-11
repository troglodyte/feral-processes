//! The base report hub: `BASE_REPORT_PAGES`, the arrow/`[`/`]`/`Esc` nav
//! that runs ahead of every hub screen's own handler, and the doors in
//! (`b`, `N`).

use feral_processes_engine::alerts::AlertKind;

use crate::tests::alerts::{alert, seeded};
use crate::tests::support::*;
use crate::*;

/// The first tab of every page on `b`'s rail as it stands now.
fn rail_modes(app: &App) -> Vec<Mode> {
    Hub::Base
        .kind()
        .pages(app)
        .iter()
        .map(|p| p.tabs[0].mode)
        .collect()
}

/// The first tab of the last page on `b`'s rail.
fn last_page_mode(app: &App) -> Mode {
    *rail_modes(app).last().expect("b has pages")
}

/// Walks `[` along `b`'s rail until the page whose first tab is `mode`.
fn back_to(app: &mut App, mode: Mode) {
    for _ in 0..rail_modes(app).len() {
        if app.mode == mode {
            return;
        }
        app.handle_key(GameKey::Char('['));
    }
    assert_eq!(app.mode, mode, "{mode:?} is on the rail");
}

fn page_modes() -> Vec<Mode> {
    BASE_REPORT_PAGES
        .iter()
        .map(|p| p.tabs[0].mode)
        .collect::<Vec<_>>()
}

#[test]
fn the_rail_runs_its_pages_in_order() {
    assert_eq!(
        page_modes(),
        vec![
            Mode::BaseOverview,
            Mode::Build,
            Mode::BaseOutput,
            Mode::BaseStock,
            Mode::StaffWellbeing,
            Mode::Research,
            Mode::Alerts,
        ]
    );
    assert_eq!(hub_of(Mode::BaseOutput), Some((Hub::Base, 2, 0)));
    assert_eq!(hub_of(Mode::BasePower), Some((Hub::Base, 2, 2)));
    assert_eq!(hub_of(Mode::Structures), Some((Hub::Base, 4, 4)));
    assert_eq!(hub_of(Mode::FloorOrders), Some((Hub::Base, 1, 3)));
    assert_eq!(hub_of(Mode::WorkOrderPick), None);
    assert_eq!(hub_of(Mode::WorkOrderQuantity), None);
    assert_eq!(hub_of(Mode::StructureAssign), None);
    assert_eq!(hub_of(Mode::StaffDiscontent), Some((Hub::Base, 4, 2)));
    assert_eq!(hub_of(Mode::Playing), None);
}

#[test]
fn b_opens_overview_and_esc_returns_to_the_map() {
    let mut app = test_app(30001);
    app.handle_key(GameKey::Char('b'));
    assert_eq!(app.mode, Mode::BaseOverview);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn right_bracket_wraps_last_to_first_and_left_bracket_first_to_last() {
    let mut app = test_app(30002);
    app.handle_key(GameKey::Char('b'));
    app.handle_key(GameKey::Char('['));
    assert_eq!(app.mode, last_page_mode(&app), "[ wraps first -> last");
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::BaseOverview, "] wraps last -> first");
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::Build);
}

#[test]
fn esc_from_a_later_page_leaves_the_hub_rather_than_stepping_back() {
    let mut app = test_app(30003);
    app.handle_key(GameKey::Char('b'));
    for _ in 0..rail_modes(&app).len() - 1 {
        app.handle_key(GameKey::Char(']'));
    }
    assert_eq!(app.mode, last_page_mode(&app));
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn esc_from_the_rail_leaves_the_hub_opened_from_the_map() {
    let mut app = test_app(30004);
    app.handle_key(GameKey::Char('b'));
    assert_eq!(app.mode, Mode::BaseOverview);
    app.handle_key(GameKey::Char(']'));
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn tab_is_consumed_by_the_hub_and_keeps_a_single_screen_page() {
    let mut app = test_app(30005);
    app.handle_key(GameKey::Char('b'));
    app.handle_key(GameKey::Tab);
    assert_eq!(app.mode, Mode::BaseOverview);
    assert!(!app.log_expanded, "Tab must not reach the map's log toggle");
}

#[test]
fn a_digit_on_alerts_stays_on_alerts() {
    let alerts = (0..5)
        .map(|i| {
            alert(
                AlertKind::DepotsFull,
                &format!("s{i}"),
                &format!("alert-{i}"),
            )
        })
        .collect();
    let mut app = seeded(30006, alerts);
    app.handle_key(GameKey::Char('N'));
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Char('7'));
    assert_eq!(app.mode, Mode::Alerts);
    assert_eq!(
        app.menu_selected, 1,
        "the highlight is Down's; a digit does nothing"
    );
}

#[test]
fn b_reopens_the_last_page() {
    let mut app = test_app(30007);
    app.handle_key(GameKey::Char('b'));
    app.handle_key(GameKey::Char(']'));
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    app.handle_key(GameKey::Char('b'));
    assert_eq!(app.mode, Mode::Build);
}

#[test]
fn n_lands_on_alerts_with_everything_marked_read() {
    let mut app = seeded(
        30009,
        vec![alert(
            AlertKind::DepotsFull,
            "depots",
            "Every Depot is full.",
        )],
    );
    assert_eq!(app.game.as_ref().unwrap().unread_alerts(), 1);
    app.handle_key(GameKey::Char('b'));
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Char('N'));
    assert_eq!(app.mode, Mode::Alerts);
    assert_eq!(app.game.as_ref().unwrap().unread_alerts(), 0);
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn paging_onto_alerts_marks_everything_read() {
    let mut app = seeded(
        30015,
        vec![alert(
            AlertKind::DepotsFull,
            "depots",
            "Every Depot is full.",
        )],
    );
    app.handle_key(GameKey::Char('b'));
    assert_eq!(app.game.as_ref().unwrap().unread_alerts(), 1);
    back_to(&mut app, Mode::Alerts);
    assert_eq!(app.game.as_ref().unwrap().unread_alerts(), 0);
}

#[test]
fn no_command_row_is_called_base_output_or_base_report() {
    let mut app = test_app(30010);
    let labels: Vec<_> = party_rows(&mut app).iter().map(|r| r.label).collect();
    assert!(!labels.contains(&"Base output"), "{labels:?}");
    assert!(!labels.contains(&"Base report"), "{labels:?}");
}

#[test]
fn enter_on_an_overview_row_jumps_to_that_pages_first_screen() {
    let mut app = test_app(30011);
    app.handle_key(GameKey::Char('b'));
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::Build, "row 0 is the page after Overview");

    let mut app = test_app(30013);
    app.handle_key(GameKey::Char('b'));
    app.handle_key(GameKey::Right);
    let alerts = BASE_REPORT_PAGES
        .iter()
        .position(|p| p.title == "Alerts")
        .unwrap();
    for _ in 1..alerts {
        app.handle_key(GameKey::Down);
    }
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::Alerts);
}

/// A digit names no overview row: the page is left by Enter on the
/// highlight, so a stray number neither jumps nor moves it.
#[test]
fn a_digit_on_the_overview_picks_no_row() {
    let mut app = test_app(30014);
    app.handle_key(GameKey::Char('b'));
    app.handle_key(GameKey::Right);
    let (mode, row) = (app.mode, app.menu_selected);
    for c in '0'..='9' {
        app.handle_key(GameKey::Char(c));
        assert_eq!((app.mode, app.menu_selected), (mode, row), "[{c}]");
    }
}

#[test]
fn the_screens_inside_the_hub_cost_no_game_time() {
    let mut app = test_app(30012);
    let before = app.game.as_ref().unwrap().current_tick();
    app.handle_key(GameKey::Char('b'));
    for _ in 0..4 {
        app.handle_key(GameKey::Char(']'));
        app.handle_key(GameKey::Tab);
    }
    app.handle_key(GameKey::Esc);
    assert_eq!(app.game.as_ref().unwrap().current_tick(), before);
}

#[test]
fn tab_walks_the_five_staff_tabs_and_left_goes_back_to_the_rail() {
    let mut app = test_app(30015);
    open_hub_screen(&mut app, Mode::StaffWellbeing);
    let walk = |app: &mut App, key| -> Vec<Mode> {
        (0..5)
            .map(|_| {
                app.handle_key(key);
                app.mode
            })
            .collect()
    };
    assert_eq!(
        walk(&mut app, GameKey::Tab),
        vec![
            Mode::StaffCoverage,
            Mode::StaffDiscontent,
            Mode::BaseStaff,
            Mode::Structures,
            Mode::StaffWellbeing
        ],
        "Tab wraps past the last tab"
    );
    assert!(!app.hub_on_rail);
    assert_eq!(
        walk(&mut app, GameKey::ShiftTab),
        vec![
            Mode::Structures,
            Mode::BaseStaff,
            Mode::StaffDiscontent,
            Mode::StaffCoverage,
            Mode::StaffWellbeing
        ]
    );
    app.handle_key(GameKey::Left);
    assert!(app.hub_on_rail, "Left from any tab reaches the rail");
}

#[test]
fn the_staff_screens_scroll_by_cursor_and_start_each_screen_at_the_top() {
    let mut app = test_app(30016);
    open_hub_screen(&mut app, Mode::StaffWellbeing);
    let rows = app.game.as_ref().unwrap().need_coverage().len();
    assert!(rows >= 2, "the shipped catalogue has two needs");
    app.handle_key(GameKey::Tab);
    assert_eq!(app.mode, Mode::StaffCoverage);
    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, 1);
    app.handle_key(GameKey::Tab);
    assert_eq!(app.menu_selected, 0, "a new screen starts at the top");
}

/// Popup children are not in `BASE_REPORT_PAGES`, so the nav must hand every
/// key — page keys, `Tab`, `Esc` — to the popup's own handler.
#[test]
fn page_keys_inside_a_work_order_popup_do_not_change_page() {
    let mut app = app_inside_a_small_base(30020, false);
    open_hub_screen(&mut app, Mode::WorkOrders);
    let last = app.work_order_rows().len() - 1;
    pick_row(&mut app, last);
    assert_eq!(app.mode, Mode::WorkOrderPick);

    for key in [GameKey::Char('['), GameKey::Char(']'), GameKey::Tab] {
        app.handle_key(key);
        assert_eq!(
            app.mode,
            Mode::WorkOrderPick,
            "{key:?} must not turn a page"
        );
    }

    pick_row(&mut app, 0);
    assert_eq!(app.mode, Mode::WorkOrderQuantity);
    for key in [GameKey::Char('['), GameKey::Char(']'), GameKey::Tab] {
        app.handle_key(key);
        assert_eq!(
            app.mode,
            Mode::WorkOrderQuantity,
            "{key:?} must not turn a page"
        );
    }
    app.handle_key(GameKey::Char('7'));
    assert_eq!(app.order_quantity_input, "7", "text entry still lands");

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::WorkOrderPick, "Esc steps back one popup");
    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.mode,
        Mode::WorkOrders,
        "and then onto the parent screen"
    );
    app.handle_key(GameKey::Esc);
    assert!(app.hub_on_rail, "whose Esc goes to the rail");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing, "and the rail's leaves the hub");
}

#[test]
fn page_keys_inside_the_staffing_popup_do_not_change_page() {
    let mut app = app_inside_a_small_base_with_two_extractors(30021);
    open_hub_screen(&mut app, Mode::Structures);
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::StructureAssign);

    for key in [GameKey::Char('['), GameKey::Char(']'), GameKey::Tab] {
        app.handle_key(key);
        assert_eq!(
            app.mode,
            Mode::StructureAssign,
            "{key:?} must not turn a page"
        );
    }
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Structures, "Esc returns to the roster");
    assert_eq!(app.menu_selected, 1, "on the row that was staffed");
}

#[test]
fn each_popup_child_names_the_hub_screen_it_draws_over() {
    assert_eq!(
        base_popup_parent(Mode::WorkOrderPick),
        Some(Mode::WorkOrders)
    );
    assert_eq!(
        base_popup_parent(Mode::WorkOrderQuantity),
        Some(Mode::WorkOrders)
    );
    assert_eq!(
        base_popup_parent(Mode::StructureAssign),
        Some(Mode::Structures)
    );
    assert_eq!(
        base_popup_parent(Mode::MachineRecipeConfirm),
        Some(Mode::MachineRecipe)
    );
    assert_eq!(base_popup_parent(Mode::WorkOrders), None);
    assert_eq!(base_popup_parent(Mode::Playing), None);
}

/// The old command rows were `Locality::Field`; the screens now open from
/// anywhere, so the keys that change something carry the refusal.
#[test]
fn work_orders_refuse_edits_underground() {
    let mut app = app_inside_a_small_base(30022, true);
    open_hub_screen(&mut app, Mode::WorkOrders);
    let last = app.work_order_rows().len() - 1;

    pick_row(&mut app, last);

    assert_eq!(app.mode, Mode::WorkOrders, "the picker must not open");
    assert_eq!(
        app.status_line.as_deref(),
        Some(crate::app::building::OUT_OF_BASE)
    );
}

#[test]
fn queued_orders_cannot_be_cancelled_or_reordered_underground() {
    let mut app = app_inside_a_small_base(30026, false);
    let item = app.orderable_items()[0].0.clone();
    let game = app.game.as_mut().unwrap();
    for qty in [1, 2] {
        game.queue_work_order(WorkOrder::batch(item.clone(), qty))
            .unwrap();
    }
    go_underground(&mut app);
    open_hub_screen(&mut app, Mode::WorkOrders);
    let quantities = |app: &App| -> Vec<u32> {
        app.game
            .as_ref()
            .unwrap()
            .work_orders()
            .iter()
            .map(|o| o.qty)
            .collect()
    };

    for key in [GameKey::Char('>'), GameKey::Char('<'), GameKey::Backspace] {
        app.status_line = None;
        app.handle_key(key);
        assert_eq!(
            app.status_line.as_deref(),
            Some(crate::app::building::OUT_OF_BASE),
            "{key:?}"
        );
        assert_eq!(quantities(&app), vec![1, 2], "{key:?} must not edit");
    }
}

#[test]
fn the_duty_table_refuses_edits_underground() {
    let mut app = app_owning_distant_programs(30023, 1);
    go_underground(&mut app);
    open_hub_screen(&mut app, Mode::BaseStaff);
    let before = app.base_staff_table().rows[0].cells.clone();

    for key in [
        GameKey::Char(' '),
        GameKey::Char('A'),
        GameKey::Char('<'),
        GameKey::Char('>'),
    ] {
        app.status_line = None;
        app.handle_key(key);
        assert_eq!(
            app.status_line.as_deref(),
            Some(crate::app::building::OUT_OF_BASE),
            "{key:?}"
        );
    }

    assert_eq!(app.base_staff_table().rows[0].cells, before);
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Right);
    assert_eq!(app.work_column, 1, "moving the cursors is still reading");
}

/// `work_column` is reset by the mode change, not by which key made it, so
/// stepping back into the duty table from the roster must clear it too.
#[test]
fn stepping_back_into_the_duty_table_resets_its_column_cursor() {
    let mut app = app_owning_distant_programs(30024, 1);
    stand_in_base(&mut app);
    open_hub_screen(&mut app, Mode::BaseStaff);
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Right);
    assert_eq!(app.work_column, 2);

    let columns = app.base_staff_table().columns.len();
    for _ in 3..columns {
        app.handle_key(GameKey::Right);
    }
    assert_eq!(
        app.work_column,
        columns - 1,
        "Right stops on the last column"
    );
    assert_eq!(app.mode, Mode::BaseStaff);
    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::BaseStaff, "the duty table is its own tab");
    app.handle_key(GameKey::Tab);
    assert_eq!(app.mode, Mode::Structures, "the roster is the next tab");
    app.handle_key(GameKey::ShiftTab);

    assert_eq!(app.mode, Mode::BaseStaff);
    assert_eq!(
        app.work_column, 0,
        "a fresh entry starts at the first column"
    );
}

/// The roster's `keeps_highlight` pair: the staffing popup's Esc lands on
/// the row it was opened from, not wherever the popup's own cursor was left.
#[test]
fn the_roster_keeps_its_highlight_across_the_staffing_popup() {
    let mut app = app_inside_a_small_base_with_two_extractors(30025);
    open_hub_screen(&mut app, Mode::Structures);
    assert_eq!(app.menu_selected, 0, "entered at the top");
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::StructureAssign);
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Down);

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Structures);
    assert_eq!(app.menu_selected, 1, "back on the second Extractor's row");
}

/// `]` from Overview walks every page once and wraps; `[` walks it back.
#[test]
fn the_full_rail_walks_forward_and_back() {
    let mut app = test_app(30017);
    app.handle_key(GameKey::Char('b'));
    let rail = rail_modes(&app);
    let mut seen = vec![app.mode];
    for _ in 1..rail.len() {
        app.handle_key(GameKey::Char(']'));
        seen.push(app.mode);
    }
    assert_eq!(seen, rail);
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::BaseOverview);
    for page in rail.into_iter().rev() {
        app.handle_key(GameKey::Char('['));
        assert_eq!(app.mode, page);
    }
}

#[test]
fn tab_walks_production_and_build_tabs() {
    for screens in [
        &[
            Mode::BaseOutput,
            Mode::BaseMachines,
            Mode::BasePower,
            Mode::Recipes,
            Mode::Craft,
        ][..],
        &[
            Mode::Build,
            Mode::BuildOrders,
            Mode::DigOrders,
            Mode::FloorOrders,
        ],
    ] {
        let mut app = test_app(30018);
        open_hub_screen(&mut app, screens[0]);
        for &next in &screens[1..] {
            app.handle_key(GameKey::Tab);
            assert_eq!(app.mode, next);
        }
        app.handle_key(GameKey::Tab);
        assert_eq!(app.mode, screens[0], "Tab wraps past the last tab");
    }
}

/// The cursor walks the report each screen draws and wraps at its ends, and
/// no key costs game time. Digging and Builds are filled to different
/// lengths so an arm reading the other's list wraps on the wrong row.
#[test]
fn the_list_cursor_wraps_over_each_screens_own_report_and_costs_no_time() {
    let mut app = app_inside_a_small_base(30020, false);
    {
        let game = app.game.as_mut().unwrap();
        let r = TEST_POCKET_HALF_HEIGHT + 1;
        // Four cells in a row: four dig marks.
        game.toggle_mark_box((0, r), (3, r), None);
        // Three requests that cost no program.
        for dy in [1, 2, 3] {
            game.place_structure("depot", 0, dy, None).unwrap();
        }
        assert_eq!(game.dig_marks().len(), 4, "precondition");
        assert_eq!(game.build_order_report().len(), 3, "precondition");
    }
    let before = app.game.as_ref().unwrap().current_tick();
    for mode in [
        Mode::BaseMachines,
        Mode::BasePower,
        Mode::BaseStock,
        Mode::BuildOrders,
        Mode::DigOrders,
        Mode::FloorOrders,
    ] {
        app.mode = mode;
        app.menu_selected = 0;
        app.hub_on_rail = false;
        let rows = {
            let game = app.game.as_ref().unwrap();
            match mode {
                Mode::BaseMachines => game.machine_report().len(),
                Mode::BasePower => game.power_report().lines.len(),
                Mode::BaseStock => game.stock_report().len(),
                Mode::BuildOrders => game.build_order_report().len(),
                Mode::DigOrders => game.dig_marks().len(),
                _ => game.floor_marks().len(),
            }
        };
        if rows == 0 {
            app.handle_key(GameKey::Down);
            assert_eq!(app.menu_selected, 0, "{mode:?} has nowhere to go");
            continue;
        }
        for _ in 1..rows {
            app.handle_key(GameKey::Down);
        }
        assert_eq!(app.menu_selected, rows - 1, "{mode:?} reaches its last row");
        app.handle_key(GameKey::Down);
        assert_eq!(app.menu_selected, 0, "{mode:?} wraps to the top");
        app.handle_key(GameKey::Up);
        assert_eq!(app.menu_selected, rows - 1, "{mode:?} wraps to the bottom");
        assert_eq!(app.mode, mode);
    }
    assert_eq!(app.game.as_ref().unwrap().current_tick(), before);
}

#[test]
fn b_opens_with_the_cursor_on_the_rail_and_down_walks_the_pages() {
    let mut app = test_app(30030);
    app.handle_key(GameKey::Char('b'));
    assert!(app.hub_on_rail, "b opens on the rail");
    app.handle_key(GameKey::Down);
    assert_eq!(app.mode, Mode::Build);
    app.handle_key(GameKey::Up);
    app.handle_key(GameKey::Up);
    assert_eq!(app.mode, last_page_mode(&app), "Up wraps first -> last");
    assert!(app.hub_on_rail, "walking the rail keeps the cursor there");
}

#[test]
fn right_moves_the_cursor_into_the_page_and_left_brings_it_back() {
    let mut app = test_app(30031);
    app.handle_key(GameKey::Char('b'));
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Right);
    assert!(!app.hub_on_rail);
    app.handle_key(GameKey::Down);
    assert_eq!(app.mode, Mode::Build, "Down in the page moves its rows");
    app.handle_key(GameKey::Left);
    assert!(app.hub_on_rail);
    assert_eq!(app.mode, Mode::Build);
}

#[test]
fn enter_on_the_rail_moves_into_the_page_without_acting_on_it() {
    let mut app = test_app(30032);
    app.handle_key(GameKey::Char('b'));
    app.handle_key(GameKey::Enter);
    assert!(!app.hub_on_rail);
    assert_eq!(
        app.mode,
        Mode::BaseOverview,
        "Enter did not open an Overview row"
    );
}

#[test]
fn the_rail_keeps_page_keys_from_reaching_the_screen() {
    let mut app = test_app(30033);
    app.handle_key(GameKey::Char('b'));
    app.handle_key(GameKey::Char('1'));
    assert_eq!(
        app.mode,
        Mode::BaseOverview,
        "a row jump waits for the page"
    );
}

#[test]
fn esc_in_the_page_goes_to_the_rail_and_esc_on_the_rail_closes() {
    let mut app = test_app(30034);
    app.handle_key(GameKey::Char('b'));
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BaseOverview);
    assert!(app.hub_on_rail);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn n_opens_alerts_with_the_cursor_in_the_page() {
    let mut app = test_app(30035);
    app.handle_key(GameKey::Char('N'));
    assert_eq!(app.mode, Mode::Alerts);
    assert!(
        !app.hub_on_rail,
        "N is for reading the board, so it lands on it"
    );
}

#[test]
fn left_walks_the_duty_table_columns_before_it_reaches_the_rail() {
    let mut app = test_app(30036);
    open_hub_screen(&mut app, Mode::BaseStaff);
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Right);
    assert_eq!(app.work_column, 2);
    app.handle_key(GameKey::Left);
    app.handle_key(GameKey::Left);
    assert_eq!(app.work_column, 0);
    assert!(!app.hub_on_rail, "a column step is not a trip to the rail");
    app.handle_key(GameKey::Left);
    assert!(app.hub_on_rail);
}

#[test]
fn b_opens_the_hub_at_overview_then_where_it_was_left() {
    let mut app = test_app(30100);
    app.handle_key(GameKey::Char('b'));
    assert_eq!(app.mode, Mode::BaseOverview);
    assert!(app.hub_on_rail);
    app.handle_key(GameKey::Char('['));
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    app.handle_key(GameKey::Char('b'));
    assert_eq!(app.mode, last_page_mode(&app), "the last hub screen");
    assert!(app.hub_on_rail);
}

#[test]
fn capital_b_no_longer_opens_anything() {
    let mut app = test_app(30101);
    app.handle_key(GameKey::Char('B'));
    assert_eq!(app.mode, Mode::Playing);
}

/// A page is never hidden for where the party stands (hub merge): the
/// Deploy tab opens underground, and the engine's own refusal is what a
/// placement there reads.
#[test]
fn b_underground_keeps_the_deploy_tab() {
    let mut app = app_underground(30102);
    app.handle_key(GameKey::Char('b'));
    assert_eq!(app.mode, Mode::BaseOverview);
    app.handle_key(GameKey::Esc);
    open_hub_screen(&mut app, Mode::Build);
}

#[test]
fn no_mode_is_in_two_hubs() {
    let app = app_with_a_breeding_bay(30090, 1, false);
    let mut seen = Vec::new();
    for hub in Hub::ALL {
        let mut in_hub = Vec::new();
        // A structure's entries repeat their tabs by design (two depots), and
        // are entered by their own door.
        for mode in hub
            .kind()
            .pages(&app)
            .iter()
            .filter(|p| !matches!(p.key, HubPageKey::Structure(_)))
            .flat_map(|p| p.modes().collect::<Vec<_>>())
        {
            assert!(!seen.contains(&mode), "{mode:?} is in two hubs");
            in_hub.push(mode);
        }
        seen.extend(in_hub);
    }
}

#[test]
fn the_deploy_tab_opens_on_its_category_list() {
    let mut app = test_app(30107);
    app.build_category = Some(feral_processes_engine::structures::StructureCategory::Utility);
    open_hub_screen(&mut app, Mode::Build);
    assert_eq!(app.mode, Mode::Build);
    assert_eq!(app.build_category, None);
}

#[test]
fn a_non_command_mode_has_no_command_rows() {
    let mut app = test_app(30108);
    assert!(app.command_rows(Mode::BaseStock).is_none());
    assert!(app.command_rows(Mode::Playing).is_none());
}

/// A merged page keeps every mark its old pages had: the engine's flag for
/// any of its tabs marks it.
#[test]
fn a_merged_page_is_marked_by_any_of_its_tabs() {
    let page = |title: &str| {
        BASE_REPORT_PAGES
            .iter()
            .position(|p| p.title == title)
            .unwrap()
    };
    let flags = feral_processes_engine::BaseReportFlags {
        work_orders: Some("an order is stalled".into()),
        staffing: Some("a duty is unstaffed".into()),
        build_orders: Some("a request is dry".into()),
        ..Default::default()
    };
    assert_eq!(
        base_page_flag(&flags, page("Stock")),
        Some("an order is stalled")
    );
    assert_eq!(
        base_page_flag(&flags, page("Staff")),
        Some("a duty is unstaffed")
    );
    assert_eq!(
        base_page_flag(&flags, page("Build")),
        Some("a request is dry")
    );
    assert_eq!(base_page_flag(&flags, page("Research")), None);
}

/// Inside the Deploy tab Esc steps out of an open category first; only from
/// the category list does it go to the rail.
#[test]
fn esc_in_a_deploy_category_closes_it_before_the_rail() {
    let mut app = test_app(30112);
    stand_in_base(&mut app);
    open_hub_screen(&mut app, Mode::Build);
    let category = app
        .build_menu()
        .unwrap()
        .rows
        .iter()
        .position(|r| matches!(r, crate::app::building::BuildRow::Category(_)))
        .expect("a category row");
    pick_row(&mut app, category);
    assert!(app.build_category.is_some());
    app.handle_key(GameKey::Esc);
    assert_eq!(app.build_category, None);
    assert_eq!(app.mode, Mode::Build);
    assert!(!app.hub_on_rail);
    app.handle_key(GameKey::Esc);
    assert!(app.hub_on_rail);
}

/// The tree view walks its nodes with every arrow, so Left is the page's.
#[test]
fn left_in_the_research_tree_view_stays_in_the_page() {
    let mut app = test_app(30113);
    open_hub_screen(&mut app, Mode::Research);
    if !app.research_graph_view {
        app.handle_key(GameKey::Char('G'));
    }
    assert!(app.research_graph_view);
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Left);
    assert!(!app.hub_on_rail, "Left moved the tree cursor");
    app.handle_key(GameKey::Left);
    assert!(app.hub_on_rail, "off the first column it is the rail's");
    app.hub_on_rail = false;
    app.handle_key(GameKey::Char('G'));
    app.handle_key(GameKey::Left);
    assert!(app.hub_on_rail, "the list view's Left goes to the rail");
}

/// The tree view draws over the whole hub, so with the cursor in it the
/// page keys do nothing under it, Tab switches trees in the view, and Esc
/// steps back to the list view before the next one reaches the rail.
#[test]
fn the_research_tree_view_keeps_the_hubs_keys_until_esc() {
    let mut app = test_app(30115);
    open_hub_screen(&mut app, Mode::Research);
    if !app.research_graph_view {
        app.handle_key(GameKey::Char('G'));
    }
    let page = app.hub_page().cloned();
    app.handle_key(GameKey::Tab);
    assert_eq!(app.mode, Mode::RoutineResearch, "Tab switches trees");
    assert!(app.research_graph_view, "and the tree view stays shown");
    assert!(!app.hub_on_rail);
    app.handle_key(GameKey::ShiftTab);
    assert_eq!(app.mode, Mode::Research, "Shift+Tab switches back");
    for key in [
        GameKey::Char(']'),
        GameKey::Char('['),
        GameKey::Char('}'),
        GameKey::Char('{'),
    ] {
        app.handle_key(key);
        assert_eq!(app.mode, Mode::Research, "{key:?} changed the tab");
        assert_eq!(app.hub_page().cloned(), page, "{key:?} turned the page");
        assert!(!app.hub_on_rail, "{key:?} went to the rail");
        assert!(app.research_graph_view, "{key:?} left the tree view");
    }
    app.handle_key(GameKey::Esc);
    assert!(!app.research_graph_view, "Esc leaves the tree view first");
    assert_eq!(app.mode, Mode::Research);
    assert!(!app.hub_on_rail);
    app.handle_key(GameKey::Esc);
    assert!(app.hub_on_rail, "the list view's Esc goes to the rail");
    assert_eq!(app.mode, Mode::Research);
}

/// From the rail the tree view claims nothing, so `]` walks past the
/// Research page rather than stopping on it.
#[test]
fn the_rail_walks_past_the_research_tree_view() {
    let mut app = test_app(30116);
    open_hub_screen(&mut app, Mode::Research);
    if !app.research_graph_view {
        app.handle_key(GameKey::Char('G'));
    }
    app.handle_key(GameKey::Left);
    app.hub_on_rail = true;
    app.handle_key(GameKey::Char(']'));
    assert_ne!(app.mode, Mode::Research, "the rail stepped on");
    app.handle_key(GameKey::Char('['));
    app.handle_key(GameKey::Tab);
    assert_eq!(
        app.mode,
        Mode::RoutineResearch,
        "Tab from the rail switches tabs"
    );
}

/// The Study tab stands whether or not a station does (a page is never
/// hidden for the base's state); with no station there is no pen, so the
/// staff are not rows a key could pin.
#[test]
fn the_study_tab_with_no_station_has_no_rows() {
    let mut app = app_owning_distant_programs(30114, 2);
    stand_in_base(&mut app);
    let game = app.game.as_ref().unwrap();
    assert!(game.study_station().is_none(), "precondition");
    assert!(game.base_staff().len() >= 2, "precondition");
    open_hub_screen(&mut app, Mode::PinSubject);
    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, 0);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::PinSubject);
    assert!(app.game.as_ref().unwrap().pinned_subject().is_none());
}

/// Resting on Research with the tree view up, the rail has focus: every
/// arrow is the rail's, Left included, and Esc closes the hub.
#[test]
fn resting_on_research_leaves_the_rail_keys_to_the_rail() {
    let mut app = test_app(30117);
    open_hub_screen(&mut app, Mode::Research);
    if !app.research_graph_view {
        app.handle_key(GameKey::Char('G'));
    }
    app.hub_on_rail = true;
    app.handle_key(GameKey::Down);
    assert!(app.hub_on_rail);
    assert!(!is_research_mode(app.mode), "Down moved the rail on");
    app.handle_key(GameKey::Up);
    assert_eq!(app.mode, Mode::Research);
    assert!(app.hub_on_rail);
    app.handle_key(GameKey::Left);
    assert!(app.hub_on_rail, "Left on the rail stays on the rail");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing, "Esc on the rail closes the hub");
}

#[test]
fn right_from_the_rail_enters_the_tree() {
    let mut app = test_app(30118);
    open_hub_screen(&mut app, Mode::Research);
    if !app.research_graph_view {
        app.handle_key(GameKey::Char('G'));
    }
    app.hub_on_rail = true;
    app.handle_key(GameKey::Right);
    assert!(!app.hub_on_rail, "Right enters the tree");
    assert_eq!(app.mode, Mode::Research);
}

/// Left from the tree's left-most column is the rail's again (ADR 0045).
#[test]
fn left_from_the_trees_left_edge_returns_to_the_rail() {
    let mut app = test_app(30119);
    app.handle_key(GameKey::Char('b'));
    // The rail rests on Research the way a player reaches it.
    for _ in 0..40 {
        if app.mode == Mode::Research {
            break;
        }
        app.handle_key(GameKey::Down);
    }
    assert_eq!(app.mode, Mode::Research);
    assert!(app.research_graph_view, "the tree is the default view");
    assert!(app.hub_on_rail);
    app.handle_key(GameKey::Right);
    assert!(!app.hub_on_rail, "Right enters the tree");
    for _ in 0..40 {
        app.handle_key(GameKey::Left);
    }
    assert!(app.hub_on_rail, "Left off the first column is the rail's");
    assert_eq!(app.mode, Mode::Research);
}

/// The guard on the Left claim: on the rail the tree claims no arrow, so
/// Left cannot reach its handler and step the cursor behind the rail's back.
#[test]
fn the_tree_claims_no_arrow_while_the_rail_has_focus() {
    let mut app = test_app(30120);
    open_hub_screen(&mut app, Mode::Research);
    if !app.research_graph_view {
        app.handle_key(GameKey::Char('G'));
    }
    app.hub_on_rail = true;
    for key in [GameKey::Left, GameKey::Right, GameKey::Up, GameKey::Down] {
        assert!(!app.screen_claims(key), "{key:?} claimed from the rail");
    }
    app.hub_on_rail = false;
    assert!(app.screen_claims(GameKey::Left), "in the tree Left is its");
}

fn is_research_mode(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::Research | Mode::RoutineResearch | Mode::AffixResearch | Mode::WeaponResearch
    )
}
