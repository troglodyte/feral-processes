//! The base report hub: `BASE_REPORT_PAGES`, the arrow/`[`/`]`/`Esc` nav
//! that runs ahead of every hub screen's own handler, and the doors in
//! (`b`, `N`).

use feral_processes_engine::alerts::AlertKind;

use crate::tests::alerts::{alert, seeded};
use crate::tests::support::*;
use crate::*;

fn page_modes() -> Vec<Mode> {
    BASE_REPORT_PAGES
        .iter()
        .map(|p| p.screens[0])
        .collect::<Vec<_>>()
}

#[test]
fn the_rail_runs_all_twelve_pages_in_order() {
    assert_eq!(
        page_modes(),
        vec![
            Mode::BaseOverview,
            Mode::BaseBuild,
            Mode::BaseResearch,
            Mode::BaseTrade,
            Mode::BaseCompile,
            Mode::StaffWellbeing,
            Mode::BaseOutput,
            Mode::BaseStock,
            Mode::WorkOrders,
            Mode::BaseStaff,
            Mode::BuildOrders,
            Mode::Alerts
        ]
    );
    assert_eq!(hub_of(Mode::BaseOutput), Some((Hub::Base, 6, 0)));
    assert_eq!(hub_of(Mode::BasePower), Some((Hub::Base, 6, 2)));
    assert_eq!(hub_of(Mode::Structures), Some((Hub::Base, 9, 1)));
    assert_eq!(hub_of(Mode::FloorOrders), Some((Hub::Base, 10, 2)));
    assert_eq!(hub_of(Mode::WorkOrderPick), None);
    assert_eq!(hub_of(Mode::WorkOrderQuantity), None);
    assert_eq!(hub_of(Mode::StructureAssign), None);
    assert_eq!(hub_of(Mode::StaffDiscontent), Some((Hub::Base, 5, 2)));
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
    assert_eq!(app.mode, Mode::Alerts, "[ wraps first -> last");
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::BaseOverview, "] wraps last -> first");
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::BaseBuild);
}

#[test]
fn esc_from_a_later_page_leaves_the_hub_rather_than_stepping_back() {
    let mut app = test_app(30003);
    app.handle_key(GameKey::Char('b'));
    for _ in 0..BASE_REPORT_PAGES.len() - 1 {
        app.handle_key(GameKey::Char(']'));
    }
    assert_eq!(app.mode, Mode::Alerts);
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
    assert_eq!(app.mode, Mode::BaseBuild);
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
    app.handle_key(GameKey::Char('['));
    assert_eq!(app.mode, Mode::Alerts);
    assert_eq!(app.game.as_ref().unwrap().unread_alerts(), 0);
}

#[test]
fn no_command_row_is_called_base_output_or_base_report() {
    let mut app = test_app(30010);
    let labels = command_labels_all(&mut app);
    assert!(!labels.contains(&"Base output"), "{labels:?}");
    assert!(!labels.contains(&"Base report"), "{labels:?}");
}

#[test]
fn enter_on_an_overview_row_jumps_to_that_pages_first_screen() {
    let mut app = test_app(30011);
    app.handle_key(GameKey::Char('b'));
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Enter);
    assert_eq!(
        app.mode,
        Mode::BaseBuild,
        "row 0 is the page after Overview"
    );

    let mut app = test_app(30013);
    app.handle_key(GameKey::Char('b'));
    app.handle_key(GameKey::Right);
    for _ in 0..BASE_REPORT_PAGES.len() - 2 {
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
fn right_walks_the_three_staff_screens_and_left_walks_back_to_the_rail() {
    let mut app = test_app(30015);
    open_hub_screen(&mut app, Mode::StaffWellbeing);
    let walk = |app: &mut App, key| -> Vec<Mode> {
        (0..3)
            .map(|_| {
                app.handle_key(key);
                app.mode
            })
            .collect()
    };
    assert_eq!(
        walk(&mut app, GameKey::Right),
        vec![
            Mode::StaffCoverage,
            Mode::StaffDiscontent,
            Mode::StaffDiscontent
        ],
        "Right stops at the last screen"
    );
    assert!(!app.hub_on_rail);
    assert_eq!(
        walk(&mut app, GameKey::Left),
        vec![
            Mode::StaffCoverage,
            Mode::StaffWellbeing,
            Mode::StaffWellbeing
        ]
    );
    assert!(
        app.hub_on_rail,
        "Left off the first screen reaches the rail"
    );
}

#[test]
fn the_staff_screens_scroll_by_cursor_and_start_each_screen_at_the_top() {
    let mut app = test_app(30016);
    open_hub_screen(&mut app, Mode::StaffWellbeing);
    let rows = app.game.as_ref().unwrap().need_coverage().len();
    assert!(rows >= 2, "the shipped catalogue has two needs");
    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::StaffCoverage);
    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, 1);
    app.handle_key(GameKey::Right);
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
    let mut app = app_inside_a_small_base_with_programs(30021, false, 1);
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

/// The old command rows were `Locality::Base`; the screens now open from
/// anywhere, so the keys that change something carry the refusal.
#[test]
fn work_orders_refuse_edits_outside_base_space() {
    let mut app = app_inside_a_small_base(30022, false);
    app.game.as_mut().unwrap().leave_base().unwrap();
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
fn queued_orders_cannot_be_cancelled_or_reordered_outside_base_space() {
    let mut app = app_inside_a_small_base(30026, false);
    let item = app.orderable_items()[0].0.clone();
    let game = app.game.as_mut().unwrap();
    for qty in [1, 2] {
        game.queue_work_order(WorkOrder::batch(item.clone(), qty))
            .unwrap();
    }
    game.leave_base().unwrap();
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
fn the_duty_table_refuses_edits_outside_base_space() {
    let mut app = app_owning_distant_programs(30023, 1);
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
    assert_eq!(app.mode, Mode::Structures, "and then steps to the roster");
    app.handle_key(GameKey::Left);

    assert_eq!(app.mode, Mode::BaseStaff);
    assert_eq!(
        app.work_column, 0,
        "a fresh entry starts at the first column"
    );
}

/// The roster's `keeps_highlight` pair: the staffing popup's Esc lands on
/// the row it was opened from even though the roster was stepped onto from
/// the duty table.
#[test]
fn the_roster_keeps_its_highlight_across_the_staffing_popup() {
    let mut app = app_inside_a_small_base_with_programs(30025, false, 1);
    open_hub_screen(&mut app, Mode::Structures);
    assert_eq!(app.menu_selected, 0, "entered at the top");
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::StructureAssign);

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Structures);
    assert_eq!(app.menu_selected, 1);
}

/// `]` from Overview walks every page once and wraps; `[` walks it back.
#[test]
fn the_full_rail_walks_forward_and_back() {
    let mut app = test_app(30017);
    app.handle_key(GameKey::Char('b'));
    let mut seen = vec![app.mode];
    for _ in 1..BASE_REPORT_PAGES.len() {
        app.handle_key(GameKey::Char(']'));
        seen.push(app.mode);
    }
    assert_eq!(seen, page_modes());
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::BaseOverview);
    for page in page_modes().into_iter().rev() {
        app.handle_key(GameKey::Char('['));
        assert_eq!(app.mode, page);
    }
}

#[test]
fn right_walks_production_and_build_orders_screens() {
    let mut app = test_app(30018);
    app.hub_on_rail = false;
    for screens in [
        [Mode::BaseOutput, Mode::BaseMachines, Mode::BasePower],
        [Mode::BuildOrders, Mode::DigOrders, Mode::FloorOrders],
    ] {
        app.mode = screens[0];
        for &next in &screens[1..] {
            app.handle_key(GameKey::Right);
            assert_eq!(app.mode, next);
        }
        app.handle_key(GameKey::Right);
        assert_eq!(app.mode, screens[2], "Right stops at the last screen");
    }
}

#[test]
fn tab_no_longer_turns_a_pages_screens() {
    let mut app = test_app(30019);
    app.mode = Mode::BaseOutput;
    app.hub_on_rail = false;
    app.handle_key(GameKey::Tab);
    assert_eq!(app.mode, Mode::BaseOutput);
}

/// The cursor walks the report each screen draws and wraps at its ends, and
/// no key costs game time. Digging and Builds are filled to different
/// lengths so an arm reading the other's list wraps on the wrong row.
#[test]
fn the_list_cursor_wraps_over_each_screens_own_report_and_costs_no_time() {
    let mut app = app_inside_a_small_base(30020, false);
    {
        let game = app.game.as_mut().unwrap();
        let r = feral_processes_engine::tuning::STARTING_POCKET_HALF_HEIGHT + 1;
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
    assert_eq!(app.mode, Mode::BaseBuild);
    app.handle_key(GameKey::Up);
    app.handle_key(GameKey::Up);
    assert_eq!(app.mode, Mode::Alerts, "Up wraps first -> last");
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
    assert_eq!(app.mode, Mode::BaseBuild, "Down in the page moves its rows");
    app.handle_key(GameKey::Left);
    assert!(app.hub_on_rail);
    assert_eq!(app.mode, Mode::BaseBuild);
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
    app.handle_key(GameKey::Char('b'));
    while app.mode != Mode::BaseStaff {
        app.handle_key(GameKey::Down);
    }
    app.handle_key(GameKey::Right);
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

fn command_labels(app: &mut App, page: Mode) -> Vec<&'static str> {
    app.command_rows(page)
        .expect("a command page")
        .iter()
        .map(|r| r.label)
        .collect()
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
    assert_eq!(app.mode, Mode::Alerts, "the last hub screen");
    assert!(app.hub_on_rail);
}

#[test]
fn capital_b_no_longer_opens_anything() {
    let mut app = test_app(30101);
    app.handle_key(GameKey::Char('B'));
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn b_underground_opens_the_hub_and_build_has_no_base_rows() {
    let mut app = app_underground(30102);
    app.handle_key(GameKey::Char('b'));
    assert_eq!(app.mode, Mode::BaseOverview);
    let build = command_labels(&mut app, Mode::BaseBuild);
    for absent in [
        "Work a structure yourself",
        "Upgrade a structure",
        "Demolish a structure",
        "Deploy a structure",
    ] {
        assert!(!build.contains(&absent), "{absent:?} in {build:?}");
    }
}

#[test]
fn no_mode_is_in_two_hubs() {
    let mut seen = Vec::new();
    for hub in Hub::ALL {
        for mode in hub.kind().table().iter().flat_map(|p| p.screens) {
            assert!(!seen.contains(mode), "{mode:?} is in two hubs");
            seen.push(*mode);
        }
    }
}

#[test]
fn the_command_pages_sit_after_overview_with_their_titles() {
    let titles: Vec<_> = BASE_REPORT_PAGES.iter().map(|p| p.title).collect();
    assert_eq!(
        titles[..5],
        ["Overview", "Build", "Research", "Trade", "Compile"]
    );
    assert_eq!(hub_of(Mode::BaseBuild), Some((Hub::Base, 1, 0)));
    assert_eq!(hub_of(Mode::BaseCompile), Some((Hub::Base, 4, 0)));
}

#[test]
fn each_command_page_lists_its_table_in_base_space() {
    let mut app = app_inside_a_small_base_with_programs(30103, false, 1);
    assert_eq!(
        command_labels(&mut app, Mode::BaseBuild),
        [
            "Deploy a structure",
            "Work a structure yourself",
            "Upgrade a structure",
            "Demolish a structure"
        ]
    );
    assert_eq!(
        command_labels(&mut app, Mode::BaseResearch),
        ["Research", "Routine research", "Affix research"]
    );
    assert_eq!(
        command_labels(&mut app, Mode::BaseTrade),
        ["Contracts"],
        "the onboarding mission is in hand"
    );
    assert_eq!(
        command_labels(&mut app, Mode::BaseCompile),
        ["Compile an item", "Recipes"]
    );

    let mut station = app_owning_a_program_and_a_research_station(30104);
    stand_in_base(&mut station);
    assert!(command_labels(&mut station, Mode::BaseResearch).contains(&"Study a program"));
    let mut cell = app_beside_a_holding_cell(30105, Vec::new(), 0);
    stand_in_base(&mut cell);
    let research = command_labels(&mut cell, Mode::BaseResearch);
    assert_eq!(research.last(), Some(&"Hold a program"), "{research:?}");

    let mut broker = app_at_a_contract_broker(30106, false);
    assert_eq!(command_labels(&mut broker, Mode::BaseTrade), ["Contracts"]);
}

#[test]
fn founding_offers_deploy_alone_and_it_opens_the_category_list() {
    let mut app = test_app(30107);
    assert_eq!(
        command_labels(&mut app, Mode::BaseBuild),
        ["Deploy a structure"]
    );
    app.build_category = Some(feral_processes_engine::structures::StructureCategory::Utility);
    open_command(&mut app, "Deploy a structure");
    assert_eq!(app.mode, Mode::Build);
    assert_eq!(app.build_category, None);
}

#[test]
fn a_non_command_mode_has_no_command_rows() {
    let mut app = test_app(30108);
    assert!(app.command_rows(Mode::BaseStock).is_none());
    assert!(app.command_rows(Mode::Playing).is_none());
}

#[test]
fn esc_from_a_command_returns_to_its_row_and_a_page_turn_forgets_it() {
    let mut app = app_inside_a_small_base_with_programs(30109, false, 1);
    open_hub_screen(&mut app, Mode::BaseBuild);
    let demolish = app
        .command_rows(Mode::BaseBuild)
        .unwrap()
        .iter()
        .position(|r| r.label == "Demolish a structure")
        .unwrap();
    assert!(demolish > 0);
    pick_row(&mut app, demolish);
    assert_eq!(app.mode, Mode::Remove);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BaseBuild);
    assert!(!app.hub_on_rail, "the highlight is on the row just used");
    assert_eq!(app.menu_selected, demolish);

    app.handle_key(GameKey::Char(']'));
    app.handle_key(GameKey::Char('['));
    assert_eq!(app.mode, Mode::BaseBuild);
    assert_eq!(app.menu_selected, 0, "a page turn starts at the top");
}

#[test]
fn a_row_letter_on_the_rail_of_a_command_page_does_nothing() {
    let mut app = app_inside_a_small_base_with_programs(30110, false, 1);
    app.handle_key(GameKey::Char('b'));
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::BaseBuild);
    assert!(app.hub_on_rail);
    app.handle_key(GameKey::Char('a'));
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::BaseBuild);
}

#[test]
fn an_all_hidden_page_is_still_on_the_rail() {
    let mut app = test_app(30111);
    skip_tutorial(&mut app);
    app.handle_key(GameKey::Char('b'));
    for _ in 0..3 {
        app.handle_key(GameKey::Char(']'));
    }
    assert_eq!(app.mode, Mode::BaseTrade);
    assert_eq!(app.command_rows(Mode::BaseTrade).map(|r| r.len()), Some(0));
}
