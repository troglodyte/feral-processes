use super::*;

fn assignee(kind: TaskKind) -> Assignee {
    Assignee {
        entity: Entity::PLACEHOLDER,
        label: "Sub-Process (Z9)".into(),
        kind,
        progress: 999,
        required: 999,
        level: Some(99),
        hp: Some((999, 999)),
    }
}

#[test]
fn an_assignee_row_reads_the_way_the_party_roster_does() {
    assert_eq!(
        assignee_line(&assignee(TaskKind::GatherResource)),
        "Sub-Process (Z9) Lv99 HP 999/999 — cronjob 999/999"
    );
    assert_eq!(
        assignee_line(&assignee(TaskKind::Guard)),
        "Sub-Process (Z9) Lv99 HP 999/999 — guarding"
    );
}

/// A program without the components still gets a row rather than a line
/// full of placeholder figures.
#[test]
fn an_assignee_with_no_stats_reads_as_it_did_before_the_vitals() {
    let bare = Assignee {
        level: None,
        hp: None,
        ..assignee(TaskKind::Guard)
    };
    assert_eq!(assignee_line(&bare), "Sub-Process (Z9) — guarding");
}

/// The structure sheet is a `PopupSize::Small`, which is half the window
/// wide — and `draw_row` clamps rows vertically but never horizontally,
/// so a line too long for the box runs off its edge rather than wrapping
/// or being cut. Adding the vitals lengthened every assignee row, so the
/// worst case is measured against the real box rather than eyeballed.
#[test]
fn the_longest_assignee_row_fits_the_structure_sheet() {
    let m = crate::text::ui_metrics(900.0);
    let line = format!("  {}", assignee_line(&assignee(TaskKind::GatherResource)));
    crate::paint::with_painter(|p| {
        let box_w = p.screen_w() * 0.5;
        let text_w = p.measure_ui_advance(&line, m.font_size);
        assert!(
            text_w + 2.0 * m.pad < box_w,
            "an assignee row is {text_w}px inside a {box_w}px sheet: {line:?}"
        );
    });
}

/// The widest headline and the widest room row each fit the half-window
/// sheet. The widest of each part comes from the shipped assets, so a
/// longer room or structure name fails here rather than running off the
/// box. The room is its own row because the two together overflow.
#[test]
fn the_longest_structure_headline_fits_the_structure_sheet() {
    use feral_processes_engine::rooms::{RoomBand, RoomDb};
    use feral_processes_engine::structures::StructureDb;

    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let (rooms, _) = RoomDb::load_dir(&assets.join("rooms")).unwrap();
    let (structures, _) = StructureDb::load_dir(&assets.join("structures")).unwrap();
    let m = crate::text::ui_metrics(900.0);
    crate::paint::with_painter(|p| {
        let width = |text: &str| p.measure_ui_advance(text, m.font_size);
        let widest = |names: Vec<String>| {
            names
                .into_iter()
                .max_by(|a, b| width(a).total_cmp(&width(b)))
                .expect("the shipped assets define at least one")
        };
        let band = widest(
            [
                RoomBand::Cramped,
                RoomBand::Plain,
                RoomBand::Fine,
                RoomBand::Superb,
            ]
            .map(|b| b.label().to_string())
            .to_vec(),
        );
        let role = widest(rooms.iter().map(|r| r.name.clone()).collect());
        let label = widest(structures.all().map(|d| d.name.clone()).collect());

        let mut s = structure_report(MachineStatus::Running);
        s.label = label;
        s.pos = (-99, -99);
        s.distance = 999;
        s.tier = Some(9);
        s.durability = Some((999, 999));
        s.room = Some(format!("{band} {role}"));
        let box_w = p.screen_w() * 0.5;
        let headline = structure_headline(&s);
        let room_row = structure_detail_lines(&s, None, false)
            .into_iter()
            .map(|(text, _)| text)
            .find(|text| text.contains(&role))
            .expect("a structure in a room says which");
        for text in [headline, room_row] {
            let text_w = width(&text);
            assert!(
                text_w + 2.0 * m.pad < box_w,
                "a row is {text_w}px inside a {box_w}px sheet: {text:?}"
            );
        }
    });
}

/// The roster's second header row: what it says, and — the actual point
/// of the row — that it goes red exactly when the grid can't cover its
/// machines. A flush grid stays a plain row so red keeps meaning "short".
#[test]
fn the_roster_header_reports_the_grid() {
    let short = grid_header_row(15, 12);
    assert_eq!(row_text(&short), "Grid  15 / 12");
    match short {
        Row::TextColored(_, color) => {
            assert_eq!(color, RED, "a grid short of supply should read red")
        }
        Row::Text(_) | Row::Item { .. } => panic!("a grid short of supply should read red"),
    }

    let flush = grid_header_row(12, 15);
    assert_eq!(row_text(&flush), "Grid  12 / 15");
    assert!(
        matches!(flush, Row::Text(_)),
        "a grid with supply to spare should not read red"
    );
}

fn structure_report(status: MachineStatus) -> StructureReport {
    StructureReport {
        recipe: Some("core_fragment".into()),
        entity: Entity::PLACEHOLDER,
        kind: "extractor".to_string(),
        label: "Extractor (Core Fragment)".to_string(),
        pos: (0, 0),
        distance: 0,
        tier: None,
        durability: None,
        is_home: false,
        workable: true,
        player_adjacent: false,
        room: None,
        input: Vec::new(),
        output: Vec::new(),
        output_capacity: 0,
        status: Some(status),
        assignees: Vec::new(),
        standing_tool: None,
        supply: None,
        pod: None,
        prison: None,
        line: None,
    }
}

/// A defence names what charges it even while it holds some: the
/// "uncharged" stall says the buffer is empty, and nothing else on the
/// sheet says what fills it.
#[test]
fn the_detail_lines_name_what_a_structure_is_fed_with() {
    let mut s = structure_report(MachineStatus::Uncharged);
    s.supply = Some(("Charge Coil".to_string(), 6));
    let lines = structure_detail_lines(&s, None, false);
    let texts: Vec<&str> = lines.iter().map(|(l, _)| l.as_str()).collect();
    assert!(
        texts.contains(&"  takes Charge Coil, holds up to 6"),
        "{texts:?}"
    );
    let stall = texts.iter().position(|l| l.starts_with("  uncharged"));
    let takes = texts.iter().position(|l| l.starts_with("  takes"));
    assert!(
        stall < takes,
        "the stall is explained by the line under it: {texts:?}"
    );

    s.supply = None;
    let lines = structure_detail_lines(&s, None, false);
    assert!(
        lines.iter().all(|(l, _)| !l.starts_with("  takes")),
        "{lines:?}"
    );
}

fn line_report(active: Option<usize>, missing_text: Option<&str>) -> LineReport {
    let members: Vec<Entity> = (1..=3)
        .map(Entity::from_raw_u32)
        .map(Option::unwrap)
        .collect();
    LineReport {
        key: feral_processes_engine::LineKey((0, 0)),
        active: active.map(|i| members[i]),
        members,
        names: ["Mining Node", "Lathe", "Disk Press"]
            .map(String::from)
            .to_vec(),
        status: MachineStatus::Starved,
        missing: None,
        missing_text: missing_text.map(String::from),
    }
}

#[test]
fn a_cell_quotes_its_prisoner_its_clock_and_the_missing_warden() {
    use feral_processes_engine::views::{PrisonState, PrisonerState};
    let mut s = structure_report(MachineStatus::Idle);
    s.prison = Some(PrisonState {
        warded: false,
        prisoner: Some(PrisonerState {
            name: "Crawler 1".to_string(),
            attempts: 2,
            max_attempts: 5,
            progress: 7,
            attempt_ticks: 40,
            odds_percent: 18,
        }),
    });
    let text: Vec<String> = structure_detail_lines(&s, None, false)
        .into_iter()
        .map(|(t, _)| t)
        .collect();
    let joined = text.join("\n");
    assert!(joined.contains("holds Crawler 1"), "{joined}");
    assert!(joined.contains("attempts 2/5"), "{joined}");
    assert!(joined.contains("beats 7/40"), "{joined}");
    assert!(joined.contains("next roll 18%"), "{joined}");
    assert!(joined.contains("no warden"), "{joined}");
}

#[test]
fn a_line_row_names_the_machines_and_what_the_line_is_doing() {
    assert_eq!(
        line_headline(&line_report(Some(1), None)),
        "Mining Node → Lathe → Disk Press · working Lathe"
    );
    assert_eq!(
        line_headline(&line_report(
            Some(1),
            Some("Disk Press needs Core Fragment")
        )),
        "Mining Node → Lathe → Disk Press · Disk Press needs Core Fragment"
    );
    assert_eq!(
        line_headline(&line_report(None, None)),
        "Mining Node → Lathe → Disk Press · starved"
    );
}

#[test]
fn a_machine_in_a_line_says_which() {
    let line = line_report(None, None);
    let mut s = structure_report(MachineStatus::Running);
    s.line = Some(line.key);
    let lines: Vec<String> = structure_detail_lines(&s, Some(&line), false)
        .into_iter()
        .map(|(l, _)| l)
        .collect();
    assert!(lines.contains(&"  Part of a line: Mining Node → Lathe → Disk Press.".to_string()));
    assert!(
        structure_detail_lines(&structure_report(MachineStatus::Running), None, false)
            .iter()
            .all(|(l, _)| !l.contains("Part of a line"))
    );
}

#[test]
fn a_member_of_a_staffed_line_is_not_idle() {
    let line = line_report(None, None);
    let mut a = structure_report(MachineStatus::Running);
    a.line = Some(line.key);
    let mut b = a.clone();
    b.assignees = vec![assignee(TaskKind::GatherResource)];
    let members = [&a, &b];
    assert!(!structure_is_idle(&a, &members));
    assert!(
        structure_detail_lines(&a, Some(&line), structure_is_idle(&a, &members))
            .iter()
            .all(|(l, _)| !l.contains("idle"))
    );
    b.assignees.clear();
    let members = [&a, &b];
    assert!(structure_is_idle(&a, &members));
    assert_eq!(StructureReport::idle_count(&[a, b]), 1, "once per line");
}

/// A terminal says whether it can fire, and a structure that is not one
/// says nothing about pods at all.
#[test]
fn a_drop_pod_terminal_says_whether_it_is_charged() {
    let pod_lines = |pod| {
        let mut s = structure_report(MachineStatus::Running);
        s.pod = pod;
        structure_detail_lines(&s, None, false)
            .into_iter()
            .map(|(line, _)| line)
            .filter(|line| line.contains("charged") || line.contains("recharging"))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        pod_lines(Some(PodState::Charged)),
        ["  charged — ready to drop"]
    );
    assert_eq!(
        pod_lines(Some(PodState::Recharging { percent: 40 })),
        ["  recharging — 40% reloaded"]
    );
    assert!(pod_lines(None).is_empty());
}

/// The two grid stalls, and the difference between them is the whole
/// point of the pair.
///
/// This line used to end "build a Recharger Node", on the reading that
/// `Unpowered` is the one status whose fix is a build. That advice is
/// wrong on the base most likely to be reading it: one whose Rechargers
/// are all standing there `Dry`, where a fifth would go dry beside them.
/// The line now states the fact and stops, and the *cause* says what to
/// do about itself.
///
/// Asserted as an absence as well as an equality, because the equality
/// alone passes against any rewrite at all — including one that puts the
/// build advice back in different words.
#[test]
fn a_dark_machine_says_the_grid_is_short_without_prescribing_a_build() {
    let dark = stall_line(&structure_report(MachineStatus::Unpowered));
    assert_eq!(dark, Some("dark — the grid is short"));
    assert!(
        !dark.unwrap().contains("build"),
        "a base whose Rechargers are all dry does not need a fifth one"
    );
}

/// The cause, and the line the inspector's sheet draws when `i` finds a
/// dry Recharger Node — `draw_structure_manifest` calls this builder
/// rather than restating it, so the roster and the sheet cannot drift.
///
/// It names the fix, which is what splitting `Dry` off `Starved` bought:
/// a supplier has no input buffer and no program, so "nothing is feeding
/// it" sent the player looking for two things that do not exist on it.
#[test]
fn a_dry_supplier_says_it_is_out_of_fuel_and_where_fuel_goes() {
    let dry = stall_line(&structure_report(MachineStatus::Dry));
    assert_eq!(dry, Some("out of fuel — no Power Cells beside it"));
    assert_ne!(
        dry,
        stall_line(&structure_report(MachineStatus::Starved)),
        "sharing the sentence is what made this unreadable"
    );
}

/// The roster is drawn into the base report's content rect, and
/// `draw_row` never clips a row horizontally — an overlong row just runs
/// off the edge. Every row a structure can draw at its widest (tier,
/// durability, a stall sentence, a charged pod, a four-digit grid
/// header) is held against the room the hub gives it at three window
/// sizes.
#[test]
fn no_roster_row_overflows_the_content_rect() {
    let mut worst = structure_report(MachineStatus::Unpowered);
    worst.label = "Reconstruction Foundry".to_string();
    worst.pos = (-9999, -9999);
    worst.tier = Some(9);
    worst.durability = Some((9999, 9999));
    worst.pod = Some(PodState::Recharging { percent: 99 });
    let mut lines = vec![
        row_text(&grid_header_row(9999, 9999)).to_string(),
        structure_headline(&worst),
    ];
    lines.extend(
        structure_detail_lines(&worst, None, true)
            .into_iter()
            .map(|(line, _)| line),
    );
    crate::paint::with_painter(|p| {
        for (w, h) in [(1280.0, 720.0), (1440.0, 900.0), (1920.0, 1080.0)] {
            let m = crate::text::ui_metrics(h);
            let char_w = p.measure_ui_advance("M", m.font_size);
            let content = crate::render::base_report::content_rect_at(w, h, char_w, &m);
            let room = content.w - m.pad * 2.0;
            for line in &lines {
                // The selection prefix and icon slot an item row carries.
                let drawn = p.measure_ui_advance(format!("     {line}"), m.font_size);
                assert!(
                    drawn <= room,
                    "a roster row overflows by {:.0}px at {w}x{h}:\n{line}",
                    drawn - room
                );
            }
        }
    });
}
