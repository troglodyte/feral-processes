//! The base report hub: `BASE_REPORT_PAGES`, the `[`/`]`/`Tab`/`Esc` nav
//! that runs ahead of every hub screen's own handler, and the three doors
//! in (`B`, `N`, the base-menu row).

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
fn the_rail_runs_all_eight_pages_in_order() {
    assert_eq!(
        page_modes(),
        vec![
            Mode::BaseOverview,
            Mode::StaffWellbeing,
            Mode::BaseOutput,
            Mode::BaseStock,
            Mode::WorkOrders,
            Mode::BaseStaff,
            Mode::BuildOrders,
            Mode::Alerts
        ]
    );
    assert_eq!(base_page_of(Mode::BaseOutput), Some((2, 0)));
    assert_eq!(base_page_of(Mode::BasePower), Some((2, 2)));
    assert_eq!(base_page_of(Mode::Structures), Some((5, 1)));
    assert_eq!(base_page_of(Mode::FloorOrders), Some((6, 2)));
    assert_eq!(base_page_of(Mode::WorkOrderPick), None);
    assert_eq!(base_page_of(Mode::WorkOrderQuantity), None);
    assert_eq!(base_page_of(Mode::StructureAssign), None);
    assert_eq!(base_page_of(Mode::StaffDiscontent), Some((1, 2)));
    assert_eq!(base_page_of(Mode::Playing), None);
}

#[test]
fn b_opens_overview_and_esc_returns_to_the_map() {
    let mut app = test_app(30001);
    app.handle_key(GameKey::Char('B'));
    assert_eq!(app.mode, Mode::BaseOverview);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn right_bracket_wraps_last_to_first_and_left_bracket_first_to_last() {
    let mut app = test_app(30002);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Char('['));
    assert_eq!(app.mode, Mode::Alerts, "[ wraps first -> last");
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::BaseOverview, "] wraps last -> first");
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::StaffWellbeing);
}

#[test]
fn esc_from_a_later_page_leaves_the_hub_rather_than_stepping_back() {
    let mut app = test_app(30003);
    app.handle_key(GameKey::Char('B'));
    for _ in 0..BASE_REPORT_PAGES.len() - 1 {
        app.handle_key(GameKey::Char(']'));
    }
    assert_eq!(app.mode, Mode::Alerts);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn esc_returns_to_the_base_menu_when_the_row_opened_the_hub() {
    let mut app = test_app(30004);
    open_via_menu(&mut app, 'b', "Base report");
    assert_eq!(app.mode, Mode::BaseOverview);
    app.handle_key(GameKey::Char(']'));
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BaseMenu);
}

#[test]
fn tab_is_consumed_by_the_hub_and_keeps_a_single_screen_page() {
    let mut app = test_app(30005);
    app.handle_key(GameKey::Char('B'));
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
        "the highlight is Down's, not the digit's page jump"
    );
}

#[test]
fn b_reopens_the_last_page() {
    let mut app = test_app(30007);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Char(']'));
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    app.handle_key(GameKey::Char('B'));
    assert_eq!(app.mode, Mode::StaffWellbeing);
}

#[test]
fn the_base_menu_row_reopens_the_last_page_too() {
    let mut app = test_app(30008);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Char('['));
    app.handle_key(GameKey::Esc);
    open_via_menu(&mut app, 'b', "Base report");
    assert_eq!(app.mode, Mode::Alerts);
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
    app.handle_key(GameKey::Char('B'));
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
    app.handle_key(GameKey::Char('B'));
    assert_eq!(app.game.as_ref().unwrap().unread_alerts(), 1);
    app.handle_key(GameKey::Char('['));
    assert_eq!(app.mode, Mode::Alerts);
    assert_eq!(app.game.as_ref().unwrap().unread_alerts(), 0);
}

#[test]
fn the_base_output_row_is_gone_and_the_report_row_is_offered() {
    let mut app = test_app(30010);
    app.handle_key(GameKey::Char('b'));
    let labels: Vec<_> = app.base_menu_rows().iter().map(|r| r.label).collect();
    assert!(!labels.contains(&"Base output"), "{labels:?}");
    assert!(labels.contains(&"Base report"), "{labels:?}");
}

#[test]
fn enter_on_an_overview_row_jumps_to_that_pages_first_screen() {
    let mut app = test_app(30011);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Enter);
    assert_eq!(
        app.mode,
        Mode::StaffWellbeing,
        "row 0 is the page after Overview"
    );

    let mut app = test_app(30013);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Right);
    for _ in 0..BASE_REPORT_PAGES.len() - 2 {
        app.handle_key(GameKey::Down);
    }
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::Alerts);
}

/// The nav layer must let a digit through to the screen under it — Alerts
/// discards its row pick, so only a screen that acts on one can catch a nav
/// that swallows digits.
#[test]
fn a_digit_on_the_overview_reaches_its_handler() {
    let mut app = test_app(30014);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Char('7'));
    assert_eq!(app.mode, Mode::Alerts);
}

#[test]
fn the_screens_inside_the_hub_cost_no_game_time() {
    let mut app = test_app(30012);
    let before = app.game.as_ref().unwrap().current_tick();
    app.handle_key(GameKey::Char('B'));
    for _ in 0..4 {
        app.handle_key(GameKey::Char(']'));
        app.handle_key(GameKey::Tab);
    }
    app.handle_key(GameKey::Esc);
    assert_eq!(app.game.as_ref().unwrap().current_tick(), before);
}

#[test]
fn tab_cycles_the_three_staff_screens_and_wraps() {
    let mut app = test_app(30015);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Char(']'));
    let seen: Vec<Mode> = (0..4)
        .map(|_| {
            let mode = app.mode;
            app.handle_key(GameKey::Tab);
            mode
        })
        .collect();
    assert_eq!(
        seen,
        vec![
            Mode::StaffWellbeing,
            Mode::StaffCoverage,
            Mode::StaffDiscontent,
            Mode::StaffWellbeing
        ]
    );
}

#[test]
fn the_staff_screens_scroll_by_cursor_and_start_each_screen_at_the_top() {
    let mut app = test_app(30016);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Char(']'));
    app.handle_key(GameKey::Right);
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
    app.handle_key(GameKey::Char(menu_shortcut(last)));
    assert_eq!(app.mode, Mode::WorkOrderPick);

    for key in [GameKey::Char('['), GameKey::Char(']'), GameKey::Tab] {
        app.handle_key(key);
        assert_eq!(
            app.mode,
            Mode::WorkOrderPick,
            "{key:?} must not turn a page"
        );
    }

    app.handle_key(GameKey::Char('1'));
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
    assert_eq!(base_popup_parent(Mode::WorkOrders), None);
    assert_eq!(base_popup_parent(Mode::Playing), None);
}

/// The base-menu rows were `Locality::Base`; the screens now open from
/// anywhere, so the keys that change something carry the refusal.
#[test]
fn work_orders_refuse_edits_outside_base_space() {
    let mut app = app_inside_a_small_base(30022, false);
    app.game.as_mut().unwrap().leave_base().unwrap();
    open_hub_screen(&mut app, Mode::WorkOrders);
    let last = app.work_order_rows().len() - 1;

    app.handle_key(GameKey::Char(menu_shortcut(last)));

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
/// entering the duty table through `Tab` must clear it too.
#[test]
fn tabbing_into_the_duty_table_resets_its_column_cursor() {
    let mut app = app_owning_distant_programs(30024, 1);
    stand_in_base(&mut app);
    open_hub_screen(&mut app, Mode::BaseStaff);
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Right);
    assert_eq!(app.work_column, 2);

    app.handle_key(GameKey::Tab);
    assert_eq!(app.mode, Mode::Structures);
    app.handle_key(GameKey::Tab);

    assert_eq!(app.mode, Mode::BaseStaff);
    assert_eq!(
        app.work_column, 0,
        "a fresh entry starts at the first column"
    );
}

/// The roster's `keeps_highlight` pair: the staffing popup's Esc lands on
/// the row it was opened from even though the roster was entered by `Tab`.
#[test]
fn the_roster_keeps_its_highlight_across_the_staffing_popup_after_a_tab() {
    let mut app = app_inside_a_small_base_with_programs(30025, false, 1);
    open_hub_screen(&mut app, Mode::BaseStaff);
    app.handle_key(GameKey::Tab);
    assert_eq!(app.mode, Mode::Structures);
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
    app.handle_key(GameKey::Char('B'));
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
fn tab_cycles_production_and_build_orders_screens() {
    let mut app = test_app(30018);
    app.mode = Mode::BaseOutput;
    let production: Vec<Mode> = (0..4)
        .map(|_| {
            let mode = app.mode;
            app.handle_key(GameKey::Tab);
            mode
        })
        .collect();
    assert_eq!(
        production,
        vec![
            Mode::BaseOutput,
            Mode::BaseMachines,
            Mode::BasePower,
            Mode::BaseOutput
        ]
    );
    app.mode = Mode::BuildOrders;
    app.handle_key(GameKey::Tab);
    assert_eq!(app.mode, Mode::DigOrders);
    app.handle_key(GameKey::Tab);
    assert_eq!(app.mode, Mode::FloorOrders);
    app.handle_key(GameKey::Tab);
    assert_eq!(app.mode, Mode::BuildOrders);
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
    app.handle_key(GameKey::Char('B'));
    assert!(app.hub_on_rail, "B opens on the rail");
    app.handle_key(GameKey::Down);
    assert_eq!(app.mode, Mode::StaffWellbeing);
    app.handle_key(GameKey::Up);
    app.handle_key(GameKey::Up);
    assert_eq!(app.mode, Mode::Alerts, "Up wraps first -> last");
    assert!(app.hub_on_rail, "walking the rail keeps the cursor there");
}

#[test]
fn right_moves_the_cursor_into_the_page_and_left_brings_it_back() {
    let mut app = test_app(30031);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Right);
    assert!(!app.hub_on_rail);
    app.handle_key(GameKey::Down);
    assert_eq!(
        app.mode,
        Mode::StaffWellbeing,
        "Down in the page moves its rows"
    );
    app.handle_key(GameKey::Left);
    assert!(app.hub_on_rail);
    assert_eq!(app.mode, Mode::StaffWellbeing);
}

#[test]
fn enter_on_the_rail_moves_into_the_page_without_acting_on_it() {
    let mut app = test_app(30032);
    app.handle_key(GameKey::Char('B'));
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
    app.handle_key(GameKey::Char('B'));
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
    app.handle_key(GameKey::Char('B'));
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
    app.handle_key(GameKey::Char('B'));
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
