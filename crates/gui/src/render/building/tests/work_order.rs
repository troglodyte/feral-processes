use super::*;
use feral_processes_engine::items::ItemId;
use feral_processes_engine::{WorkOrderMachine, WorkOrderReport};

/// The widest row this screen can draw is a machine line carrying a
/// machine name, a worker name and a shortfall — and the runner-up is a
/// stalled order's whole sentence. `draw_row` clamps a row vertically
/// and **never horizontally**, so a row past the popup body simply runs
/// off it; two shipped screens already do that because nobody measured.
///
/// **Every state is measured, not just the one that used to carry a
/// tag.** Before the four states only a stalled order put anything after
/// the count, so a head line was the shortest row on the screen and
/// nothing here had to think about it; now all four do. The head is
/// also the only row on this screen that is *not* wrapped —
/// `continuation_lines` bounds every other one — so it is the row a tag
/// can actually run off, and it is measured here against the longest
/// shipped item name and four-digit counts rather than a convenient
/// short one.
/// A stalled order wears a red `!` and shows the reason it is stalled; a
/// queued one wears neither.
#[test]
fn a_stalled_work_order_wears_a_red_bang_and_its_reason() {
    let order = |state, blocked_by: Option<&str>| WorkOrderRow {
        order: Some(WorkOrderReport {
            item: ItemId::from("bytecode_block"),
            label: "Bytecode Block".to_string(),
            have: 0,
            target: 5,
            state,
            blocked_by: blocked_by.map(str::to_string),
            machines: Vec::new(),
        }),
    };
    let stalled = order(
        OrderState::Stalled,
        Some("No way to make this \u{2014} set a Factory to Bytecode Block."),
    );
    let queued = order(OrderState::Queued, None);
    let rows = work_order_screen_rows(&[stalled, queued], LabourDemand::default(), 0);
    let icons: Vec<Option<(char, Color)>> = rows
        .iter()
        .filter_map(|r| match r {
            Row::Item { icon, .. } => Some(*icon),
            _ => None,
        })
        .collect();
    assert_eq!(icons, vec![Some(('!', hud::palette::OFFLINE)), None]);
    let text: Vec<String> = rows.iter().map(row_label_text).collect();
    assert!(
        text.iter()
            .any(|t| t.contains("No way to make this") && t.contains("set a Factory")),
        "the reason is on the screen: {text:?}"
    );
}

#[test]
fn no_work_order_row_runs_past_the_popup_body() {
    let report = WorkOrderReport {
        item: ItemId::from("singularity_matrix"),
        label: "Singularity Matrix".to_string(),
        have: 9999,
        target: 9999,
        state: OrderState::Working,
        blocked_by: None,
        machines: vec![WorkOrderMachine {
            entity: Entity::PLACEHOLDER,
            label: "Annealing Node".to_string(),
            worker: Some("Sub-Process Lv12".to_string()),
            short_of: Some("Blank Substrate".to_string()),
            depth: 2,
        }],
    };
    let stalled = WorkOrderReport {
        state: OrderState::Stalled,
        blocked_by: Some(
            "Nothing beside the Annealing Node is making Blank Substrate — a machine can \
             only take what a neighbour has finished."
                .to_string(),
        ),
        machines: Vec::new(),
        ..report.clone()
    };
    // The two states that draw a tag over an empty chain, which is the
    // combination the sentence under the head has to fit beside.
    let dormant = WorkOrderReport {
        state: OrderState::Dormant,
        blocked_by: None,
        machines: Vec::new(),
        ..report.clone()
    };
    let queued = WorkOrderReport {
        state: OrderState::Queued,
        ..dormant.clone()
    };
    let rows = [
        WorkOrderRow {
            order: Some(report),
        },
        WorkOrderRow {
            order: Some(stalled),
        },
        WorkOrderRow {
            order: Some(dormant),
        },
        WorkOrderRow {
            order: Some(queued),
        },
        WorkOrderRow { order: None },
    ];

    for row in &rows {
        for line in work_order_lines(row) {
            assert!(
                line.chars().count() <= ROW_WRAP_COLUMNS,
                "a {} char row runs past the {ROW_WRAP_COLUMNS} column body: {line:?}",
                line.chars().count()
            );
        }
    }

    // The hub's content rect is narrower than the popup these lines were
    // wrapped for, so the columns budget is not the whole answer.
    crate::paint::with_painter(|p| {
        for (w, h) in [(1280.0, 720.0), (1440.0, 900.0), (1920.0, 1080.0)] {
            let m = crate::text::ui_metrics(h);
            let char_w = p.measure_ui_advance("M", m.font_size);
            let content = crate::render::base_report::content_rect_at(w, h, char_w, &m);
            let room = content.w - m.pad * 2.0;
            for row in &rows {
                for line in work_order_lines(row) {
                    let drawn = p.measure_ui_advance(format!("     {line}"), m.font_size);
                    assert!(
                        drawn <= room,
                        "a work order row overflows the content rect by {:.0}px at \
                         {w}x{h}:\n{line}",
                        drawn - room
                    );
                }
            }
        }
    });

    assert!(
        WORK_ORDERS_NOTE.chars().count() <= ROW_WRAP_COLUMNS,
        "the note line runs past the {ROW_WRAP_COLUMNS} column body"
    );

    // The shortfall header, measured here rather than in its own test
    // because it is drawn into this same popup and is unwrapped for the
    // same reason a head line is. Three digits everywhere is past any
    // base a `max_deployed` roster can field, which is the point: the
    // row has to fit before anyone finds out where the real ceiling is.
    let header = labour_header(LabourDemand {
        wanted: 999,
        staff: 998,
        ..Default::default()
    })
    .expect("a base one body short has something to say");
    assert!(
        header.chars().count() <= ROW_WRAP_COLUMNS,
        "a {} char header runs past the {ROW_WRAP_COLUMNS} column body: {header:?}",
        header.chars().count()
    );
    let wide = labour_header(LabourDemand {
        wanted: 999,
        staff: 0,
        ..Default::default()
    })
    .expect("a base with nobody in it has something to say");
    assert!(
        wide.chars().count() <= ROW_WRAP_COLUMNS,
        "a {} char header runs past the {ROW_WRAP_COLUMNS} column body: {wide:?}",
        wide.chars().count()
    );
}

/// **Silence is the whole design of the header**: it says nothing when
/// the base has a body for every post, so a player who never runs short
/// never learns to skip a line at the top of this screen. A base with
/// no orders and no staff at all is the same answer for the same
/// reason — nothing was asked for, so nothing went unfilled.
#[test]
fn a_base_with_bodies_to_spare_draws_no_shortfall_header() {
    assert_eq!(
        labour_header(LabourDemand {
            wanted: 2,
            staff: 3,
            ..Default::default()
        }),
        None
    );
    assert_eq!(
        labour_header(LabourDemand {
            wanted: 3,
            staff: 3,
            ..Default::default()
        }),
        None
    );
    assert_eq!(labour_header(LabourDemand::default()), None);
}

/// The quantity page is prose in a popup with no scroll, and its widest
/// sentence had already outgrown the small box it used to be drawn in.
/// Measured in pixels rather than columns because that is the failure —
/// a column count is a proxy that was never checked here.
#[test]
fn no_work_order_quantity_row_runs_past_the_popup_body() {
    let m = crate::text::ui_metrics(900.0);
    // Longer than any name the shipped items carry, so a mod naming
    // something unreasonably has room too.
    let name = "Recompiled Kernel Substrate Blank";
    crate::paint::with_painter(|p| {
        let box_w = p.screen_w() * 0.88;
        for standing in [true, false] {
            for line in work_order_quantity_lines(name, "9999", standing) {
                let text_w = p.measure_ui_advance(&line, m.font_size);
                assert!(
                    text_w + 2.0 * m.pad < box_w,
                    "a {text_w}px row runs past the {box_w}px body: {line:?}"
                );
            }
        }
    });
}
