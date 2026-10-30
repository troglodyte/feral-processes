use super::*;

/// A hijacked companion's own rung wears `WARN`, not `PLAYER` — its side
/// of the initiative order (`is_hostile`) does not change, but its turn
/// is the AI's for as long as the entry lasts.
#[test]
fn a_hijacked_companion_s_rung_is_drawn_in_warn() {
    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    let idx = view
        .order
        .iter()
        .position(|r| !r.is_hostile)
        .expect("a party rung stands in the initiative order");
    view.order[idx].taken_over = true;
    let glyph = view.order[idx].glyph;
    assert_eq!(
        view.order.iter().filter(|r| r.glyph == glyph).count(),
        1,
        "two rungs wear {glyph:?}, so a glyph box cannot name one of them"
    );

    let m = ui_metrics(720.0);
    let (_, shapes) = with_painter(|p| draw_turn_strip(&view, pane(), p, &m));
    let (_, color) = crate::paint::painted_map_glyphs(&shapes)
        .into_iter()
        .find(|(text, _)| text == &glyph.to_string())
        .expect("the hijacked rung was not drawn");
    let close = |a: Color, b: Color| {
        (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01
    };
    assert!(
        close(color, palette::WARN),
        "a taken-over companion's rung drew {color:?}, not WARN"
    );
}

/// The width census: the widest line the tamper block can ever draw —
/// every tag, `HIJACK`, and the longest shipped routine's own display
/// name — leaves `draw_tamper_block`'s own guard satisfied.
///
/// **Measured against what the guard measures**, not against the pane.
/// The block is right-aligned on the strip, so it is dropped whole when
/// `x < pane.x + strip_inset` — that is, when its `pad * 2 + widest`
/// exceeds `strip_right - (pane.x + strip_inset)`, and `strip_right` is
/// `pane.x + pane.w - m.inset` whatever the strip's own width. A census
/// that measured the bare line against the whole pane could pass while
/// the block vanished in silence, which is the failure this exists to
/// catch.
///
/// **(M)** The shipped worst case measures 496.2px into a 766.5px
/// budget, so the headroom is real and a sixth tag does not spend it —
/// the mutation that fails this is padding the fixture's tag list out
/// until the line is wider than the budget, which it does at around
/// forty extra rungs. Verified and restored.
#[test]
fn the_widest_tamper_line_fits_the_map_pane() {
    use crate::render::hud::layout;
    use feral_processes_engine::abilities::AbilityDb;

    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/abilities");
    let (db, warnings) = AbilityDb::load_dir(&dir).expect("the abilities load");
    assert!(warnings.is_empty(), "{warnings:?}");
    let longest = db
        .all()
        .map(|d| d.name.as_str())
        .max_by_key(|name| name.len())
        .expect("at least one routine ships");

    let mut game = fighting();
    let entity = game.tactical_view().expect("the fight is open").order[0].entity;
    let row = TurnRow {
        entity,
        glyph: 'W',
        color: feral_processes_engine::components::GlyphColor::White,
        label: String::new(),
        is_hostile: false,
        hp_fraction: None,
        tags: vec![
            TamperTag::Hot,
            TamperTag::Cold,
            TamperTag::Profiled,
            TamperTag::Injected,
            TamperTag::Hallucinating,
        ],
        forecast: Some(feral_processes_engine::tactical::view::ForecastView {
            action: longest.to_string(),
            walk: Vec::new(),
            target: None,
        }),
        taken_over: true,
    };
    let line = tamper_line(&row).expect("a row wearing every tag says something");

    let m = ui_metrics(900.0);
    with_painter(|p| {
        let char_w = p.measure_ui_advance("M", m.font_size);
        let map_pane = layout::regions(1280.0, 720.0, char_w, &m, false).map_pane;
        // `draw_turn_strip`'s own right edge, which the block hangs off
        // and which is independent of how wide the strip itself is.
        let strip_right = map_pane.x + map_pane.w - m.inset;
        let budget = strip_right - (map_pane.x + strip_inset(&m));
        // `draw_tamper_block`'s own two figures, in its own spelling.
        let pad = m.line_height * 0.35;
        let width = pad * 2.0 + p.measure_ui_advance(&line, m.small());
        assert!(
            width <= budget,
            "the widest tamper block would be dropped by its own guard, \
             overflowing by {:.1}px ({width:.1}px into a {budget:.1}px budget): {line:?}",
            width - budget
        );
    });
}

/// The action bar says what is left to spend, and drops the two keys
/// that would be refused once the action is gone.
#[test]
fn the_action_bar_follows_what_is_left_of_the_turn() {
    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    view.player_turn = true;
    view.acted = false;
    let open = action_bar(Mode::TacticalBattle, &view, false);
    assert!(open.iter().any(|(k, _)| k == "a"));
    assert!(open.iter().any(|(k, _)| k == "E"));
    assert!(
        open.iter().any(|(k, l)| k == "s" && l == "special"),
        "the special key must read the way the abstract fight's does: {open:?}"
    );
    assert!(
        !open.iter().any(|(k, _)| k == "r"),
        "`r` is not a tactical key any more: {open:?}"
    );

    view.acted = true;
    let spent = action_bar(Mode::TacticalBattle, &view, false);
    // `R` and `A` both stay: arming auto-attack or driving the rest of
    // the fight are not actions and are not refused on a spent turn —
    // and a turn with nothing left to spend is exactly when a player
    // decides they would rather watch the rest of the fight.
    assert_eq!(
        spent.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
        vec!["E", "R", "A"],
        "a spent turn still offered an action"
    );
}

/// **The bar is one row on a border with no wrap and no clip**, so
/// widening the movement cell to name the numpad is a measured question
/// and not a guess — `the_keybar_fits_the_log_pane`'s rule, asked of
/// the content that replaces that bar in a fight. Measured through
/// `keybar_segments` and `fitting` rather than against a hand-built
/// list, so the thing measured is the thing drawn.
#[test]
fn the_action_bar_fits_the_log_pane() {
    use crate::render::hud::{layout, strip::fitting};

    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    // The fixture may open on the wild side, whose bar is one short
    // sentence — measuring that would pass against any width at all.
    view.player_turn = true;
    view.acted = false;
    // Three bars: the cursor's is its own content at its own width, a
    // census over one of them passes against the other overflowing, and
    // the board's bar is one row longer while auto-attack is on offer —
    // which is the row that has to be measured rather than assumed to fit.
    // And the aim bar with the worst readout a modded piece id can make:
    // it leads the bar, so one that does not fit blanks every key behind.
    let cell = (
        view.bodies.iter().map(|b| b.cell.0).max().unwrap_or(0) + 50,
        0,
    );
    let mut long = view.clone();
    long.props
        .push(feral_processes_engine::tactical::view::PropView {
            cell,
            name: "an_extraordinarily_long_modded_piece_identifier".repeat(4),
            sprite: String::new(),
            hp: Some(88),
            max_hp: Some(88),
            volatile: true,
            decoration: false,
        });
    let cases = [
        (
            Mode::TacticalBattle,
            false,
            keybar_rows(Mode::TacticalBattle, &view, false, None),
        ),
        (
            Mode::TacticalBattle,
            true,
            keybar_rows(Mode::TacticalBattle, &view, true, None),
        ),
        (
            Mode::TacticalAim,
            false,
            keybar_rows(Mode::TacticalAim, &view, false, None),
        ),
        (
            Mode::TacticalAim,
            false,
            keybar_rows(Mode::TacticalAim, &long, false, Some(cell)),
        ),
    ];
    for (mode, auto, actions) in cases {
        assert!(
            auto || actions.len() >= 3,
            "not a full bar for {mode:?}: {actions:?}"
        );
        for (w, h) in [(1280.0, 720.0), (1920.0, 1080.0)] {
            let m = ui_metrics(h);
            with_painter(|p| {
                let char_w = p.measure_ui_advance("M", m.font_size);
                let pane = layout::regions(w, h, char_w, &m, false).log_pane;
                let avail = pane.w - m.inset * 2.0;
                let segments =
                    crate::render::hud::log_frame::keybar_segments(std::slice::from_ref(&actions));
                let taken = fitting(&segments, avail, p, &m);
                let drawn: String = taken.iter().map(|(t, _, _)| t.as_str()).collect();
                let slack = avail - p.measure_ui_advance(&drawn, m.small());

                // Against the bar as it would be with unlimited room, so
                // this cannot drift as segments are added: `fitting`
                // drops from the end, so anything short of the whole is
                // a key the player cannot see.
                let whole = fitting(&segments, f32::INFINITY, p, &m);
                assert_eq!(
                    taken.len(),
                    whole.len(),
                    "{mode:?}'s bar (auto {auto}) dropped a key at {w}x{h} — slack {slack:.1}px: {drawn:?}"
                );
                assert!(
                    slack >= 0.0,
                    "{mode:?}'s bar (auto {auto}) overhangs its pane by {slack:.1}px"
                );
            });
        }
    }
}

/// Every key a battle-map bar names is on the `?` sheet, so the popup
/// never omits a key the bar advertises. The bar states are the whole of
/// `keybar_rows`: the aim cursor, a charger's decision, auto-attack, the
/// wild side's turn, and the board open and spent.
#[test]
fn every_key_on_a_tactical_bar_is_on_the_sheet() {
    use feral_processes_app_core::{ShortcutArea, shortcut_sheet_for};
    use feral_processes_engine::views::FormLook;

    let sheet = shortcut_sheet_for(ShortcutArea::TacticalMap);
    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    view.player_turn = true;
    view.acted = false;
    let acting = acting_body(&view).expect("someone acts").entity;

    let mut states: Vec<(&str, Vec<(String, String)>)> = Vec::new();
    states.push((
        "aim",
        keybar_rows(Mode::TacticalAim, &view, false, Some((0, 0))),
    ));
    states.push(("auto", keybar_rows(Mode::TacticalBattle, &view, true, None)));
    states.push((
        "plain",
        keybar_rows(Mode::TacticalBattle, &view, false, None),
    ));
    let mut form = view.clone();
    for b in form.bodies.iter_mut().filter(|b| b.entity == acting) {
        b.form = Some(FormLook {
            glyph: 'x',
            sprite: None,
        });
    }
    states.push((
        "emulating",
        keybar_rows(Mode::TacticalBattle, &form, false, None),
    ));
    let mut spent = view.clone();
    spent.acted = true;
    states.push((
        "spent",
        keybar_rows(Mode::TacticalBattle, &spent, false, None),
    ));
    let mut charging = view.clone();
    for b in charging.bodies.iter_mut().filter(|b| b.entity == acting) {
        b.charge = Some((1, 3));
    }
    states.push((
        "charging",
        keybar_rows(Mode::TacticalBattle, &charging, false, None),
    ));
    let mut wild = view.clone();
    wild.player_turn = false;
    states.push((
        "wild",
        keybar_rows(Mode::TacticalBattle, &wild, false, None),
    ));

    for (state, rows) in states {
        // A leading readout row has no key; `?` leads the keyed rows.
        let first = rows.iter().find(|(k, _)| !k.is_empty());
        assert_eq!(
            first.map(|(k, _)| k.as_str()),
            Some("?"),
            "`?` leads the {state} bar: {rows:?}"
        );
        for (key, _) in rows.iter().filter(|(k, _)| !k.is_empty()) {
            assert!(
                sheet.lists(key),
                "the {state} bar names {key:?}, which the sheet does not list"
            );
        }
    }
}

/// The bar is the only place auto-attack is discoverable, and while it is
/// running it is the only thing the bar has to say: every action key on it
/// would stop it rather than do what it names.
#[test]
fn the_action_bar_offers_auto_attack_and_says_when_it_is_running() {
    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    view.player_turn = true;
    view.acted = false;

    let offered = action_bar(Mode::TacticalBattle, &view, false);
    assert!(
        offered.iter().any(|(k, _)| k == "A"),
        "auto-attack is bound to nothing the player can see: {offered:?}"
    );

    let running = action_bar(Mode::TacticalBattle, &view, true);
    for action in ["a", "d", "s", "E"] {
        assert!(
            !running.iter().any(|(k, _)| k == action),
            "{action:?} would stop auto-attack rather than act, so the bar must not offer it: {running:?}"
        );
    }
    assert!(
        running.iter().any(|(_, l)| l.contains("auto-attack")),
        "a running auto-attack went unsaid: {running:?}"
    );
}

/// The readout names a prop under the cursor, tags a volatile one, and
/// stays silent over a cell with nothing on it.
#[test]
fn the_aim_readout_names_the_prop_under_the_cursor() {
    use feral_processes_engine::tactical::view::PropView;
    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    let cell = (
        view.bodies.iter().map(|b| b.cell.0).max().unwrap_or(0) + 50,
        0,
    );
    view.props.push(PropView {
        cell,
        name: "coolant tank".into(),
        sprite: "prop_coolant_tank".into(),
        hp: Some(6),
        max_hp: Some(8),
        volatile: true,
        decoration: false,
    });

    let (_, text) = aim_readout(&view, Some(cell)).expect("a prop is under the cursor");

    assert_eq!(text, "coolant tank 6/8 explosive");
    assert!(aim_readout(&view, Some((cell.0 + 1, 0))).is_none());
    assert!(aim_readout(&view, None).is_none());
}

/// The cursor is a different screen from the board, and offering the
/// board's keys on it names three that do nothing and hides the two
/// that commit and cancel.
#[test]
fn the_aim_cursor_gets_its_own_keys() {
    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    view.player_turn = true;
    view.acted = false;

    let rows = action_bar(Mode::TacticalAim, &view, false);

    assert!(
        rows.iter().any(|(k, _)| k == "Enter"),
        "the cursor's commit key is not on the bar: {rows:?}"
    );
    assert!(
        rows.iter().any(|(k, _)| k == "Esc"),
        "the cursor's cancel key is not on the bar: {rows:?}"
    );
    for dead in ["a", "s", "E"] {
        assert!(
            !rows.iter().any(|(k, _)| k == dead),
            "{dead:?} does nothing while aiming, so the bar must not offer it: {rows:?}"
        );
    }
}

/// The wild side's turn says so rather than offering keys that would be
/// swallowed.
#[test]
fn the_action_bar_waits_out_the_wild_side() {
    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    view.player_turn = false;
    let rows = action_bar(Mode::TacticalBattle, &view, false);
    assert!(rows.iter().all(|(k, _)| k.is_empty()));
}
