use super::*;

/// What `render::draw` paints for `app`, as plain text runs.
fn drawn_text(app: &mut feral_processes_app_core::App) -> Vec<String> {
    let mut fx = Fx::new();
    let (_, shapes) = with_painter(|p| crate::render::draw(app, &mut fx, p, false, false));
    painted_text(&shapes)
}

/// Through the real `render::draw` rather than `draw_stock_block`, which
/// would pass with the call site deleted — and with the item's *name*,
/// the half the status bar's two-letter tag never printed.
///
/// The downed store is checked the same way and for the same reason:
/// `draw_stock_block`'s own tests are handed a count, so only a draw
/// through the real path proves the call site asks `Game::downed_store`
/// for it rather than passing an empty one.
#[test]
fn the_base_stock_is_listed_over_the_map_inside_the_base() {
    use super::test_support::{HELD_PROGRAMS, STOCKED_NAME, StandingIn, game_with_base_stock};
    let mut app =
        super::test_support::playing_app_around(game_with_base_stock(5101, StandingIn::Base));
    let text = drawn_text(&mut app);
    assert!(
        text.iter().any(|t| t == "BASE STOCK"),
        "no stock block over the map: {text:?}"
    );
    assert!(
        text.iter().any(|t| t == STOCKED_NAME),
        "the block does not name what the base holds: {text:?}"
    );
    let heading = |h: &str| text.iter().position(|t| t == h);
    assert!(
        heading("RESEARCHING") < heading("BASE STOCK") && heading("RESEARCHING").is_some(),
        "the idle Research Node is not read out above the stock: {text:?}"
    );
    assert!(
        heading("BASE STOCK") < heading("DOWNED PROGRAMS") && heading("DOWNED PROGRAMS").is_some(),
        "the downed store is not read out under the stock: {text:?}"
    );
    let cap = feral_processes_engine::tuning::MAX_DOWNED_PROGRAMS;
    assert!(
        text.iter().any(|t| t == &format!("{HELD_PROGRAMS}/{cap}")),
        "the store's figure is not what the player is actually holding: {text:?}"
    );
}

/// The Stack's frame map owns that corner. Same stock as the base-space
/// test, so this cannot pass on a base that simply holds nothing.
#[test]
fn the_base_stock_is_not_listed_underground() {
    use super::test_support::{STOCKED_NAME, StandingIn, game_with_base_stock};
    let game = game_with_base_stock(5102, StandingIn::Stack);
    assert!(
        game.stack_view().is_some(),
        "precondition: the party is in the Stack"
    );
    assert!(
        game.base_stock().iter().any(|r| r.name == STOCKED_NAME),
        "precondition: the base holds something to hide"
    );
    let mut app = super::test_support::playing_app_around(game);
    let text = drawn_text(&mut app);
    assert!(
        !text.iter().any(|t| t == "BASE STOCK"
            || t == "RESEARCHING"
            || t == "DOWNED PROGRAMS"
            || t == STOCKED_NAME),
        "the stock block drew over the Stack view: {text:?}"
    );
}

/// A tactical board is the fight.
#[test]
fn the_base_stock_is_not_listed_on_a_battle_map() {
    use super::test_support::{STOCKED_NAME, StandingIn, game_with_base_stock};
    let game = (0..200u32)
        .find_map(|seed| {
            let mut game = game_with_base_stock(seed, StandingIn::Surface);
            let mut profile = game.profile().clone();
            profile.tactical_battles = true;
            game.install_profile(profile);
            let at = game.player_status().position;
            let target = game
                .view_entities(12, 12)
                .into_iter()
                .filter(|e| e.is_hostile && !e.is_tamed && !e.is_structure)
                .find(|e| (e.pos.0 - at.0).abs() + (e.pos.1 - at.1).abs() == 1)?;
            game.move_player(target.pos.0 - at.0, target.pos.1 - at.1);
            game.in_tactical_battle().then_some(game)
        })
        .expect("no seed under 200 put a lone wild program next to the player");
    assert!(
        game.base_stock().iter().any(|r| r.name == STOCKED_NAME),
        "precondition: the base holds something to hide"
    );
    let mut app = super::test_support::playing_app_around(game);
    let text = drawn_text(&mut app);
    assert!(
        !text
            .iter()
            .any(|t| t == "BASE STOCK" || t == "RESEARCHING" || t == STOCKED_NAME),
        "the stock block drew over the battle map: {text:?}"
    );
}

/// **The trap Task 4 exists to close.** `border_strip` paints its own
/// background quad, so calling `draw_map_frame` before the pane's
/// contents lets the map's own fill paint straight over the frame's
/// strips — the design handoff's own recorded failure.
/// `a_strip_paints_its_background_before_its_glyphs` in `hud::strip`
/// only proves the order *inside* `border_strip`; this is the half only
/// visible a level up, where the map pane's own background rect and the
/// frame's title text are two separate calls inside `draw_playing_base`.
/// The `@` is the one glyph on the map whose colour is a **role** and not
/// the hue its entity authored. `GlyphColor::Cyan` is what the player
/// spawns with and what a Backplane-blue structure may author too; br
/// cyan is the player's alone, and reading it off `is_player` is what
/// keeps it that way.
///
/// Distance rather than equality, because the map multiplies everything
/// it draws by a vignette and a per-tile shade — the player stands at the
/// centre of its own view, where both are all but 1.0, but "all but" is
/// what rules equality out. The second assertion is the load-bearing one:
/// the authored cyan is what this used to paint.
#[test]
fn the_players_glyph_wears_the_player_role() {
    let mut app = playing_app();
    let mut fx = Fx::new();
    let m = ui_metrics(900.0);
    let (_, shapes) = with_painter(|p| {
        draw_playing_base(&mut app, &mut fx, None, p, &m, false);
    });
    let dist = |a: Color, b: Color| (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs();
    let at = crate::paint::painted_map_glyphs(&shapes);
    let (_, drawn) = at
        .iter()
        .find(|(g, _)| g == "@")
        .expect("the map draws the player");
    assert!(
        dist(*drawn, hud::palette::PLAYER) < 0.06,
        "the player's @ painted {drawn:?}, which is {:.3} from PLAYER",
        dist(*drawn, hud::palette::PLAYER)
    );
    assert!(
        dist(*drawn, hud::palette::PLAYER) < dist(*drawn, glyph_color(GlyphColor::Cyan)),
        "the @ is nearer its authored hue than the player role — it is \
         still being drawn through `glyph_color`"
    );
}

/// The base map drawn with one `factory` beside the party, set to
/// `recipe` or left unset, and how many `?` glyphs came out in
/// `ATTENTION`. Staged through the save round trip, the way a renderer
/// test reaches game state without a `world` accessor.
fn attention_question_marks(recipe: Option<&str>) -> usize {
    use feral_processes_engine::items::ItemId;
    use feral_processes_engine::save;
    let mut app = playing_app();
    let path = std::env::temp_dir().join(format!(
        "fp_gui_unconfigured_{}_{}.sav",
        std::process::id(),
        recipe.unwrap_or("unset")
    ));
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.structures.push(save::StructureSave {
        recipe: recipe.map(ItemId::from),
        kind: "factory".to_string(),
        position: (1, 0),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: Vec::new(),
        standing_work: false,
        standing_guard: false,
        denied_items: Vec::new(),
        power_fuel: feral_processes_engine::tuning::POWER_UPKEEP_TICKS,
        build_quality: 1.0,
        racked: Vec::new(),
        hopper: Vec::new(),
        hopper_progress: 0,
        standing_tool: None,
        pod_charged: None,
        battery_charge: 0,
        incubating: Vec::new(),
        study_yield: None,
    });
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &test_assets()).unwrap());
    let _ = std::fs::remove_file(&path);

    let mut fx = Fx::new();
    let m = ui_metrics(900.0);
    let (_, shapes) = with_painter(|p| {
        draw_playing_base(&mut app, &mut fx, None, p, &m, false);
    });
    let want = hud::palette::ATTENTION;
    crate::paint::painted_map_glyphs(&shapes)
        .iter()
        .filter(|(g, c)| {
            g == "?" && (c.r - want.r).abs() + (c.g - want.g).abs() + (c.b - want.b).abs() < 0.05
        })
        .count()
}

/// A machine with nothing set to make wears a `?`, and one that has a
/// recipe does not. Drawn through the whole map so the call site's gate
/// on `EntityView::unconfigured` is what is tested, not the helper alone.
#[test]
fn an_unset_machine_wears_a_question_mark() {
    assert_eq!(attention_question_marks(None), 1);
}

#[test]
fn a_set_machine_wears_none() {
    assert_eq!(attention_question_marks(Some("bytecode_block")), 0);
}

/// The palette's two reservations, asserted where they are easiest to
/// break: br red means hostility and inbound harm, so a clogged machine —
/// an ordinary, self-inflicted, entirely fixable state — may not wear it,
/// and br yellow means the player must act, which is exactly what the
/// three states that will not resolve themselves are.
///
/// The split the old red/yellow pair carried is kept, not flattened:
/// waiting fixes a starved machine and does not fix a clogged one, so
/// they stay two colours.
#[test]
fn a_machine_asks_for_attention_and_never_reads_as_a_threat() {
    for status in [
        MachineStatus::Clogged,
        MachineStatus::Stranded,
        MachineStatus::Unpowered,
    ] {
        assert_eq!(
            machine_color(status),
            hud::palette::ATTENTION,
            "{status:?} will not fix itself and is what ATTENTION means"
        );
    }
    for status in [MachineStatus::Starved, MachineStatus::Unstaffed] {
        assert_eq!(
            machine_color(status),
            hud::palette::WARN,
            "{status:?} resolves itself, so it is the dimmer yellow"
        );
    }
    assert_eq!(machine_color(MachineStatus::Running), hud::palette::HEALTHY);
    // The one machine state that wears a red, and it is the *other* red.
    // A dry supplier has taken its whole supply off the grid and is
    // darkening machines elsewhere on the map, which is past what the
    // amber says — but it is still not hostility.
    assert_eq!(machine_color(MachineStatus::Dry), hud::palette::OFFLINE);
    // Walked rather than listed: a status added without a colour of its
    // own is exactly the one that would inherit red by accident.
    for status in MachineStatus::ALL {
        assert_ne!(
            machine_color(status),
            hud::palette::THREAT,
            "{status:?} is not hostility, and THREAT is reserved for it"
        );
    }
}

/// Two reds on one map is only worth having if they are two reds. This
/// is the `a_plan_cannot_be_read_as_a_machine_asking_for_you` measurement
/// pointed at the pair it was written for: a retune that drifts OFFLINE
/// toward THREAT turns "this node stopped" and "this thing will kill
/// you" into one piece of news.
#[test]
fn the_two_reds_do_not_read_as_one() {
    let dist = |a: Color, b: Color| (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs();
    let apart = dist(hud::palette::OFFLINE, hud::palette::THREAT);
    assert!(
        apart > 0.30,
        "OFFLINE and THREAT sit {apart} apart; they read as one colour"
    );
}

/// A plan is the player having acted; the attention yellow is the base
/// asking them to. Drawn in the same family they were two different
/// pieces of news in one colour — the washes sat 0.11 from `ATTENTION`,
/// which is nothing at 18% alpha over tinted rock.
///
/// All four are checked against the same rule, because the plan is one
/// hue at four alphas and a retune that moves only the one being read
/// here is exactly what this is for.
#[test]
fn a_plan_cannot_be_read_as_a_machine_asking_for_you() {
    let dist = |a: Color, b: Color| (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs();
    for (name, c) in [
        ("MARK_FILL", MARK_FILL),
        ("MARK_EDGE", MARK_EDGE),
        ("PREVIEW_FILL", PREVIEW_FILL),
        ("CUTTING_OUTLINE", CUTTING_OUTLINE),
    ] {
        assert!(
            dist(c, hud::palette::ATTENTION) > 0.25,
            "{name} is only {:.2} from ATTENTION",
            dist(c, hud::palette::ATTENTION)
        );
        assert!(
            dist(c, hud::palette::WARN) > 0.25,
            "{name} is only {:.2} from WARN",
            dist(c, hud::palette::WARN)
        );
        assert_eq!(
            (c.r, c.g, c.b),
            (
                hud::palette::PLAN.r,
                hud::palette::PLAN.g,
                hud::palette::PLAN.b
            ),
            "{name} is not the plan's hue — a plan reads as one thing or \
             it reads as several"
        );
    }
}

#[test]
fn the_map_frame_draws_after_the_map() {
    let mut app = playing_app();
    let label = ground_label(&mut app);
    let mut fx = Fx::new();
    let m = ui_metrics(900.0);
    let (regions, shapes) = with_painter(|p| {
        let char_w = p.measure_ui_advance("M", m.font_size);
        let regions =
            hud::layout::regions(p.screen_w(), p.screen_h(), char_w, &m, app.log_expanded);
        draw_playing_base(&mut app, &mut fx, None, p, &m, false);
        regions
    });

    let pane = regions.map_pane;
    let map_bg = shapes
        .iter()
        .position(|cs| match &cs.shape {
            // `fill.a() > 0` is load-bearing: `draw_map_frame` also
            // paints a rect the exact size of the pane — its border, via
            // `rect_lines`, which is a stroke with a *transparent* fill.
            // Matching on geometry alone would find that border instead
            // of the map's own `painter.rect(...)` fill and pass by
            // accident whichever one drew first.
            bevy_egui::egui::Shape::Rect(r) => {
                r.fill.a() > 0
                    && (r.rect.min.x - pane.x).abs() < 0.5
                    && (r.rect.min.y - pane.y).abs() < 0.5
                    && (r.rect.width() - pane.w).abs() < 0.5
                    && (r.rect.height() - pane.h).abs() < 0.5
            }
            _ => false,
        })
        .expect("the map pane paints a background rect the size of the pane");
    let title = shapes
        .iter()
        .position(|cs| {
            matches!(&cs.shape, bevy_egui::egui::Shape::Text(t) if t.galley.text().contains(&label))
        })
        .unwrap_or_else(|| panic!("the map frame never painted its ground readout {label:?}"));

    assert!(
        title > map_bg,
        "the map's background painted at {map_bg}, the {label:?} readout at {title} — \
         the frame drew before the map, so its own fill painted over the label"
    );
}

/// **Bug A.** `border_strip` centres its background quad *on* the border
/// line it mounts to, so a strip riding the map pane's top border
/// reaches upward past `map_pane.y` by `size/2 + pad/2` — and
/// `map_pane.y` used to sit exactly on the status bar's own bottom edge.
/// `draw_status_bar` paints its opaque fill *after* `draw_map_frame` has
/// already run, so the top of that quad — and the top of the ground
/// readout's and the THREAT readout's glyph caps riding on it — painted
/// straight through the bar.
///
/// Asserted against the real painted quad rather than a region number:
/// `the_playing_screen_draws_inside_its_regions` already checks
/// `map_pane.y >= status_bar.y + status_bar.h`, and that alone passes
/// against this bug, since the quad reaches *above* `map_pane.y` by a
/// margin that number says nothing about.
#[test]
fn the_map_frames_top_strips_clear_the_status_bar() {
    let mut app = playing_app();
    let ground = ground_label(&mut app);
    let mut fx = Fx::new();
    let m = ui_metrics(900.0);
    let (regions, shapes) = with_painter(|p| {
        let char_w = p.measure_ui_advance("M", m.font_size);
        let regions =
            hud::layout::regions(p.screen_w(), p.screen_h(), char_w, &m, app.log_expanded);
        draw_playing_base(&mut app, &mut fx, None, p, &m, false);
        regions
    });

    let status_bottom = regions.status_bar.y + regions.status_bar.h;

    // The ground readout (Mount::TopLeft) and the THREAT readout
    // (Mount::TopRight) both ride the map pane's top border. Each paints
    // its background quad immediately before its own glyphs
    // (`strip::border_strip`'s ordering rule), so the nearest preceding
    // filled rect *is* that quad.
    for label in [ground.as_str(), "THREAT"] {
        let text_idx = shapes
            .iter()
            .position(|cs| {
                matches!(&cs.shape, bevy_egui::egui::Shape::Text(t) if t.galley.text().contains(label))
            })
            .unwrap_or_else(|| panic!("the map frame never painted {label:?}"));
        let quad = shapes[..text_idx]
            .iter()
            .rev()
            .find_map(|cs| match &cs.shape {
                bevy_egui::egui::Shape::Rect(r) if r.fill.a() > 0 => Some(r.rect),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{label} has no background quad ahead of it"));
        assert!(
            quad.min.y >= status_bottom - 0.01,
            "{label}'s background quad starts at y={} against the status \
             bar's {status_bottom}px bottom edge — it paints into the bar",
            quad.min.y
        );
    }
}

/// **Bug C's other half.** The expanded log pane is an *overlay*: it
/// keeps the collapsed pane's bottom edge and grows upward over the
/// bottom of the map, which is what stops TAB re-laying the grid out
/// under the player. That costs the layout nothing only because the log
/// is drawn **after** the map and fills opaquely — move
/// `draw_log_pane` above the map and the expanded rows are painted over
/// by the very pane they are supposed to cover, with nothing failing to
/// compile.
#[test]
fn the_expanded_log_pane_draws_over_the_map() {
    let mut app = playing_app();
    app.log_expanded = true;
    let mut fx = Fx::new();
    let m = ui_metrics(900.0);
    let (regions, shapes) = with_painter(|p| {
        let char_w = p.measure_ui_advance("M", m.font_size);
        let regions =
            hud::layout::regions(p.screen_w(), p.screen_h(), char_w, &m, app.log_expanded);
        draw_playing_base(&mut app, &mut fx, None, p, &m, false);
        regions
    });

    assert!(
        regions.log_pane.y < regions.map_pane.y + regions.map_pane.h,
        "the expanded log does not reach the map, so nothing is overlaid"
    );

    // `fill.a() > 0` for `the_map_frame_draws_after_the_map`'s reason:
    // both panes also paint a `rect_lines` border of the same geometry
    // with a transparent fill.
    let fill_of = |pane: crate::paint::Rect| {
        shapes
            .iter()
            .position(|cs| match &cs.shape {
                bevy_egui::egui::Shape::Rect(r) => {
                    r.fill.a() > 0
                        && (r.rect.min.x - pane.x).abs() < 0.5
                        && (r.rect.min.y - pane.y).abs() < 0.5
                        && (r.rect.width() - pane.w).abs() < 0.5
                        && (r.rect.height() - pane.h).abs() < 0.5
                }
                _ => false,
            })
            .expect("both panes paint a background fill the size of the pane")
    };
    let map_bg = fill_of(regions.map_pane);
    let log_bg = fill_of(regions.log_pane);
    assert!(
        log_bg > map_bg,
        "the map's fill painted at {map_bg} and the log's at {log_bg} — \
         the overlay is under the pane it overlays"
    );
}

/// The first filled rect painted after `index` that lands on `quad`.
///
/// A strip's own background quad is painted immediately before its
/// glyphs (`strip::border_strip`'s ordering rule), so it is *not* a
/// candidate when `index` is that strip's text — which is what makes
/// "nothing opaque lands on top of it afterwards" the question this
/// answers.
pub(super) fn covering_rect_after(
    shapes: &[bevy_egui::egui::epaint::ClippedShape],
    index: usize,
    quad: bevy_egui::egui::Rect,
) -> Option<bevy_egui::egui::Rect> {
    // A pane whose edge lands exactly on the quad's is the clearance
    // holding, not a rect painted over it, so the overlap has to be an
    // area rather than a touch — the same 0.01 slack every other
    // geometry assertion on this screen carries.
    let covers = |r: &bevy_egui::egui::Rect| {
        r.min.x < quad.max.x - EDGE_SLACK
            && r.max.x > quad.min.x + EDGE_SLACK
            && r.min.y < quad.max.y - EDGE_SLACK
            && r.max.y > quad.min.y + EDGE_SLACK
    };
    shapes[index + 1..].iter().find_map(|cs| match &cs.shape {
        bevy_egui::egui::Shape::Rect(r) if r.fill.a() > 0 && covers(&r.rect) => Some(r.rect),
        _ => None,
    })
}

/// Two edges this close are the same edge.
const EDGE_SLACK: f32 = 0.01;

/// The vitals strip's glyphs and the background quad they sit on.
///
/// The whole strip is one galley (`strip::draw_pieces` lays its pieces
/// out together), so any segment identifies it; MIT is the one the
/// report named.
fn vitals_strip(
    shapes: &[bevy_egui::egui::epaint::ClippedShape],
) -> (usize, bevy_egui::egui::Rect) {
    let index = shapes
        .iter()
        .position(|cs| {
            matches!(&cs.shape, bevy_egui::egui::Shape::Text(t) if t.galley.text().contains("MIT "))
        })
        .expect("the vitals strip was never painted");
    let quad = shapes[..index]
        .iter()
        .rev()
        .find_map(|cs| match &cs.shape {
            bevy_egui::egui::Shape::Rect(r) if r.fill.a() > 0 => Some(r.rect),
            _ => None,
        })
        .expect("the vitals strip has no background quad ahead of it");
    (index, quad)
}

/// Draws the map screen and returns the shapes, with the log collapsed
/// or expanded.
fn playing_shapes(expanded: bool) -> Vec<bevy_egui::egui::epaint::ClippedShape> {
    let mut app = playing_app();
    app.log_expanded = expanded;
    let mut fx = Fx::new();
    let m = ui_metrics(900.0);
    let (_, shapes) = with_painter(|p| {
        draw_playing_base(&mut app, &mut fx, None, p, &m, false);
    });
    shapes
}

/// **The vitals strip is the last thing painted over its own line, and
/// this asserts that against *every* quad that follows it.**
///
/// The strip rides a pane border, and `border_strip` centres its
/// background quad *on* that line — so it reaches `size/2 + pad/2` past
/// the border on both sides, into whatever the next pane fills.
///
/// **The predecessor of this test named one such quad and was
/// effectively vacuous.** It compared the strip's own quad against
/// `log_pane.y`, the log pane's *body fill*, and knew nothing about the
/// filter strip that used to ride the log pane's top border — whose
/// quad reached `size/2 + pad/2` *above* that line and covered the
/// lower half of the vitals glyphs, baseline included, while the
/// arithmetic it asserted still held. So the question is not "does one
/// named rect clear it" but "does anything painted after it land on
/// it", which is what this walks.
#[test]
fn nothing_paints_over_the_vitals_strip() {
    let shapes = playing_shapes(false);
    let (index, quad) = vitals_strip(&shapes);
    let covered = covering_rect_after(&shapes, index, quad);
    assert!(
        covered.is_none(),
        "an opaque rect at {covered:?} painted over the vitals strip's \
         {quad:?} — the strip is cut by whatever fills that box"
    );
}

/// **The second complaint, and the reason the strip moved.** With the
/// log expanded (TAB, `App::log_expanded`) the pane grows upward over
/// the bottom of the map as an overlay — so a strip riding the *map*
/// pane's bottom border vanished entirely for as long as the log was
/// open. Riding the *log* pane's top border instead, it travels with
/// the pane and stays on screen in both states.
///
/// Asserted as "painted, and nothing painted over it": the text alone
/// is drawn either way, so a test that only looked for it passes
/// against the bug it exists to catch.
#[test]
fn the_vitals_strip_survives_an_expanded_log() {
    let shapes = playing_shapes(true);
    let text = painted_text(&shapes).join(" ");
    assert!(
        text.contains("MIT "),
        "the vitals strip is not on screen with the log expanded: {text:?}"
    );
    let (index, quad) = vitals_strip(&shapes);
    let covered = covering_rect_after(&shapes, index, quad);
    assert!(
        covered.is_none(),
        "with the log expanded, an opaque rect at {covered:?} painted over \
         the vitals strip's {quad:?}"
    );
}

/// **The census the design names.** The badge, the tab marker and the
/// collapsed bar are three readouts of one call, so they cannot
/// disagree — and both halves are asserted, because either alone passes
/// against a surface drawing a constant.
///
/// The nagging half is reached through the save round-trip rather than
/// by writing a component: `Game`'s `world` is private from out here and
/// deliberately stays that way (`CLAUDE.md`'s architectural rule), so a
/// renderer test reaches game state the way a player would — through a
/// save.
#[test]
fn attention_drives_all_three_markers() {
    let m = ui_metrics(900.0);
    let mut fx = Fx::new();

    let mut calm = playing_app();
    assert!(
        calm.game.as_mut().unwrap().attention().is_empty(),
        "the calm fixture has something to say"
    );
    let (_, shapes) = with_painter(|p| {
        draw_playing_base(&mut calm, &mut fx, None, p, &m, false);
    });
    let text = painted_text(&shapes).join(" ");
    assert!(text.contains("ALL NOMINAL"), "no calm badge: {text:?}");
    assert!(
        crate::paint::painted_runs_in(&shapes, hud::palette::ATTENTION, true).is_empty(),
        "a calm base wears a mark with nothing to mark"
    );

    let path = std::env::temp_dir().join(format!(
        "fp_gui_attention_census_{}.sav",
        std::process::id()
    ));
    calm.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = feral_processes_engine::save::load_from_file(&path).unwrap();
    data.player.perk_points = 4;
    feral_processes_engine::save::save_to_file(&path, &data).unwrap();
    let mut nagged = playing_app();
    nagged.game = Some(Game::load(&path, &test_assets()).unwrap());
    let _ = std::fs::remove_file(&path);

    let row = nagged
        .game
        .as_mut()
        .unwrap()
        .attention()
        .into_iter()
        .next()
        .expect("four unspent perk points is something to say");
    // A perk point lands in CREW, so open BASE and read the *closed*
    // pane's marker and bar — the state the whole design is about.
    nagged.info_tab = feral_processes_app_core::InfoTab::Base;

    let (_, shapes) = with_painter(|p| {
        draw_playing_base(&mut nagged, &mut fx, None, p, &m, false);
    });
    let text = painted_text(&shapes).join(" ");
    assert!(
        text.contains(&row.text.to_uppercase()),
        "the badge is silent: {text:?}"
    );
    assert!(
        text.contains(&row.text),
        "the closed pane hid the condition: {text:?}"
    );
    assert!(
        !crate::paint::painted_runs_in(&shapes, hud::palette::ATTENTION, true).is_empty(),
        "no tab wears a mark"
    );
    assert!(
        !text.contains("ALL NOMINAL"),
        "the bar claims calm while nagging: {text:?}"
    );
}

/// The column does not scroll, so a row past the bottom is dropped in
/// silence — `the_tallest_gear_page_fits_its_popup`'s trap in a taller
/// box. Measured at the smallest supported window, against the fixed
/// head of the panel: the bars, the stat block, the party and pet
/// headings and every party row. The inventory list below them clips
/// itself against the same floor and is not part of this.
#[test]
fn the_tallest_column_pane_fits_its_column() {
    let m = ui_metrics(720.0);
    with_painter(|p| {
        let char_w = p.measure_ui_advance("M", m.font_size);
        let r = hud::layout::regions(1280.0, 720.0, char_w, &m, false);
        let body = hud::column::regions(r.info_column, &m).body;

        // Two bars, the level/zone/position block, four stat rows, the
        // mining row, the two headings, and a full party under them.
        let head =
            2.0 + 3.0 + 4.0 + 1.0 + 2.0 + feral_processes_engine::tuning::MAX_PARTY_SIZE as f32;
        let rows = (body.h - m.inset) / m.line_height;
        assert!(
            rows >= head,
            "the column fits {rows:.1} rows and the panel's head is {head} —                  the overflow is dropped in silence"
        );
    });
}

/// `draw_playing_base` no longer computes its own rects — it reads them
/// from `hud::layout::regions` once, at the top. This is the trap named
/// in the module's own doc comment: a literal `0.0` for a y-origin draws
/// under the status bar and no test sees it, so the regions are asserted
/// against each other here rather than trusted by eye.
#[test]
fn the_playing_screen_draws_inside_its_regions() {
    with_painter(|p| {
        let m = ui_metrics(900.0);
        let char_w = p.measure_ui_advance("M", m.font_size);
        let r = hud::layout::regions(1440.0, 900.0, char_w, &m, false);
        assert!(
            r.map_pane.x + r.map_pane.w <= r.info_column.x,
            "the map pane runs into the info column"
        );
        assert!(
            r.map_pane.y >= r.status_bar.y + r.status_bar.h,
            "the map pane starts above the status bar's bottom edge"
        );
    });
}
