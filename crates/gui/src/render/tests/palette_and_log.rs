use super::*;

#[test]
fn hp_critical_triggers_at_exactly_a_third_and_not_a_point_above() {
    assert!(hp_critical(10, 30), "exactly a third is already critical");
    assert!(!hp_critical(11, 30), "a point above a third is not");
    assert!(hp_critical(0, 30), "a dead program reads as critical");
    assert!(!hp_critical(30, 30), "full health is never critical");
    assert!(
        !hp_critical(0, 0),
        "a program with no max HP is a malformed fixture, not a warning"
    );
}

/// Programs and gear read the same, because they share `MAX_FUSIONS` —
/// cyan means fused and still usable as an input, magenta means at the
/// ceiling. `None` rather than a default colour so a caller with a
/// colour rule of its own (the party screen's CRITICAL red) composes
/// with this instead of being overwritten by it.
#[test]
fn fusion_color_separates_a_fused_thing_from_a_maxed_one() {
    assert_eq!(fusion_color(0), None, "an unfused row is left plain");
    assert_eq!(fusion_color(1), Some(CYAN));
    assert_eq!(fusion_color(MAX_FUSIONS - 1), Some(CYAN));
    assert_eq!(fusion_color(MAX_FUSIONS), Some(MAGENTA));
    assert_eq!(
        fusion_color(MAX_FUSIONS + 1),
        Some(MAGENTA),
        "a legacy over-ceiling gear tier still reads as maxed"
    );
}

/// The ramp is emphasis and has to be monotone: a band that gave up the
/// weight the one below it earned would read as *less* emphatic at the
/// top of its own ladder. Only the two extremes spend a colour, and the
/// as-designed rung is literally the default treatment — every copy in
/// every existing save sits at `QUALITY_DEFAULT`, so this repaints
/// nothing already on screen.
#[test]
fn the_quality_tag_ramp_gains_emphasis_and_never_gives_it_back() {
    use feral_processes_engine::tuning::{QUALITY_DEFAULT, QUALITY_MAX, QUALITY_MIN};

    let plain = quality_tag_style(None);
    assert_eq!(
        plain,
        (TEXT, false),
        "an untagged copy draws as it always did"
    );
    assert_eq!(
        quality_tag_style(Some(QUALITY_DEFAULT)),
        plain,
        "the as-designed band is no change at all"
    );

    let mut last = (GRAY, false);
    for quality in QUALITY_MIN..=QUALITY_MAX {
        let style = quality_tag_style(Some(quality));
        assert!(
            style.1 >= last.1,
            "emphasis went backwards at {quality}: {last:?} -> {style:?}"
        );
        last = style;
    }
    assert_eq!(quality_tag_style(Some(QUALITY_MIN)), (GRAY, false));
    assert_eq!(quality_tag_style(Some(QUALITY_MAX)), (GOLD, true));
    // The two middle bands differ by weight alone, which is the whole of
    // why an ordinary copy is never painted an alarming colour.
    assert_eq!(quality_tag_style(Some(110)).0, TEXT);
    assert!(quality_tag_style(Some(110)).1);
}

/// Two permanent properties, one channel. Fusion wins because it is the
/// one that gates an action available now (can this still be a fusion
/// input); a rare tier is only ever worth knowing.
#[test]
fn fusion_outranks_rarity_in_a_menu_row() {
    assert_eq!(
        tier_color(0, Rarity::Gold),
        Some(GOLD),
        "an unfused program shows its tier"
    );
    assert_eq!(
        tier_color(1, Rarity::Gold),
        Some(CYAN),
        "fusion depth is the actionable read and takes the channel"
    );
    assert_eq!(
        tier_color(MAX_FUSIONS, Rarity::Silver),
        Some(MAGENTA),
        "and still does at the ceiling"
    );
    assert_eq!(
        tier_color(0, Rarity::Ordinary),
        None,
        "an ordinary unfused program keeps the plain row colour"
    );
}

/// The tier bar is two pixels tall and is drawn directly above a glyph
/// that `difficulty_color` may have painted green, yellow, orange, red
/// or magenta — so each tier has to stay clear of all of them, of the
/// neutrals it could read as a dimmed version of, and of the other tier.
///
/// Asserting the separation rather than the literals, so a palette
/// retune is free to move any of them. This caught both first drafts:
/// gold was 0.23 from `YELLOW` and silver 0.22 from `TEXT`.
#[test]
fn the_tier_colours_are_separable_from_their_neighbours() {
    let dist = |a: Color, b: Color| (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs();
    // Everything a tier bar can end up sitting against: every hue the
    // glyph under it can be drawn in, the player's own role colour, and
    // the neutrals a tier could read as a dimmed version of.
    //
    // Walked through `glyph_color` rather than naming the constants a
    // creature *used* to be painted in — named, this went on measuring
    // `render/mod.rs`'s old palette after the map had moved to
    // `hud::palette`, which is the same failure it already caught once
    // in the tier list below.
    let neighbours: Vec<(String, Color)> = GlyphColor::ALL
        .into_iter()
        .map(|c| (format!("{c:?}"), glyph_color(c)))
        .chain([
            ("PLAYER".to_string(), hud::palette::PLAYER),
            ("TEXT".to_string(), TEXT),
            ("TEXT_DIM".to_string(), TEXT_DIM),
        ])
        .collect();
    // Walks `Rarity::ALL` through `rarity_color` rather than naming the
    // colour constants: named, this checked exactly the two rungs that
    // existed when it was written and went on passing when two more were
    // added, which is the whole failure it is supposed to prevent.
    let tiers: Vec<(Rarity, Color)> = Rarity::ALL
        .into_iter()
        .filter_map(|r| Some((r, rarity_color(r)?)))
        .collect();
    assert_eq!(
        tiers.len(),
        Rarity::ALL.len() - 1,
        "every rung above Ordinary needs a colour"
    );
    for (tier, colour) in &tiers {
        for (other_name, other) in &neighbours {
            assert!(
                dist(*colour, *other) > 0.25,
                "{tier:?} is only {:.2} from {other_name} — it would read \
                 as that colour in a two-pixel bar",
                dist(*colour, *other)
            );
        }
    }
    for (i, (tier, colour)) in tiers.iter().enumerate() {
        for (other, other_colour) in &tiers[i + 1..] {
            assert!(
                dist(*colour, *other_colour) > 0.25,
                "{tier:?} and {other:?} are only {:.2} apart — two tiers \
                 that read alike are no ladder at all",
                dist(*colour, *other_colour)
            );
        }
    }
}

#[test]
fn only_a_rare_tier_gets_a_roster_tag() {
    assert_eq!(rarity_tag(Rarity::Ordinary), "");
    assert!(rarity_tag(Rarity::Gold).contains("OVERCLOCKED"));
    assert!(
        rarity_tag(Rarity::Silver).starts_with(' '),
        "the tag is appended after a name and separates itself"
    );
}

/// The strip is the fallback for the screens that are not popups and
/// have no log pane of their own. `Battle` is the load-bearing one: a
/// refusal raised in a fight is deliberately never logged, so the strip
/// is the whole of how it reaches the player.
#[test]
fn only_the_screens_with_nowhere_else_to_put_it_use_the_banner() {
    for mode in [
        Mode::Battle,
        Mode::BattleResult,
        Mode::FrameMap,
        Mode::WorldMap,
        Mode::FieldRoutineCell,
        Mode::SpritePicker,
        Mode::SpriteEditor,
    ] {
        assert!(
            needs_status_banner(mode),
            "{mode:?} draws no popup, so its refusals need the banner"
        );
    }
}

/// The kinds a player is meant to tell apart at a glance mid-fight: what
/// they gained, what the enemy did, which of the enemy's blows carried a
/// condition, their own hit, Integrity coming back, and how the fight
/// came out. Sharing a colour between any two of them defeats the point
/// of the log being coloured at all — and all six can sit in the battle
/// pane at once.
#[test]
fn the_log_colours_a_player_reads_mid_fight_are_all_distinct() {
    let kinds = [
        MessageKind::Loot,
        MessageKind::EnemyAttack,
        MessageKind::EnemySpecial,
        MessageKind::PartyDamage,
        MessageKind::Heal,
        MessageKind::Outcome,
    ];
    for (i, a) in kinds.iter().enumerate() {
        for b in &kinds[i + 1..] {
            assert_ne!(
                message_color(*a),
                message_color(*b),
                "{a:?} and {b:?} narrate in the same colour"
            );
        }
    }
}

/// The party's own drain narrates under `Heal` — the Integrity coming
/// back is the half a plain `Attack` cannot also produce — and that line
/// carries a figure on either side of it. Losing the emphasis in the move
/// would have taken the number-first reading off the one line that has
/// two, so the styling follows the kind rather than staying behind on
/// `PartyDamage` alone.
#[test]
fn the_lines_read_for_a_number_are_the_partys_own_blows_and_its_own_mending() {
    for kind in [MessageKind::PartyDamage, MessageKind::Heal] {
        assert!(
            emphasizes_numbers(kind),
            "{kind:?} is read for its figure, so its digits take the emphasis"
        );
    }
    for kind in [
        MessageKind::Info,
        MessageKind::Loot,
        MessageKind::LevelUp,
        MessageKind::Raid,
        MessageKind::Round,
        MessageKind::Outcome,
        MessageKind::EnemyAttack,
        MessageKind::EnemySpecial,
        MessageKind::Complete,
    ] {
        assert!(
            !emphasizes_numbers(kind),
            "{kind:?} is read for its wording; picking a digit out of it says nothing"
        );
    }
}

/// A message-log line always draws in full, whichever style it takes: the
/// run split is a re-styling of the text, not a filter on it.
#[test]
fn splitting_a_line_into_runs_preserves_it_exactly() {
    for line in [
        "You unleash a data strike for 7 damage.",
        "Sparkgrub 12 executes Arc Bite for 103 damage.",
        "42",
        "no numbers at all",
        "",
        "Ünïcödé hits you for 5 damage.",
    ] {
        let joined: String = emphasize_numbers(line, TEXT_DIM, WHITE)
            .iter()
            .map(|r| r.text)
            .collect();
        assert_eq!(joined, line);
    }
}

/// Only the digits take the emphasis. Bolding the whole sentence is what
/// this replaced, and bolding nothing would leave the number no easier to
/// pick out than the flavour text around it.
#[test]
fn only_the_digits_of_a_damage_line_are_emphasized() {
    let runs = emphasize_numbers("You unleash a data strike for 7 damage.", TEXT_DIM, WHITE);
    for run in &runs {
        let all_digits = run.text.chars().all(|c| c.is_ascii_digit());
        assert_eq!(
            run.bold, all_digits,
            "{:?} came out bold={} — a run is either all digits or none",
            run.text, run.bold
        );
        assert_eq!(run.color, if run.bold { WHITE } else { TEXT_DIM });
    }
    let emphasized: String = runs
        .iter()
        .filter(|r| r.bold)
        .map(|r| r.text)
        .collect::<Vec<_>>()
        .join("|");
    assert_eq!(emphasized, "7");
}

/// Multi-digit numbers stay whole. Splitting per character would still
/// look right, but only by accident of the two faces sharing an advance.
#[test]
fn a_multi_digit_number_is_one_run() {
    let runs = emphasize_numbers("Sparkgrub hits you for 103 damage.", TEXT_DIM, WHITE);
    let bold: Vec<&str> = runs.iter().filter(|r| r.bold).map(|r| r.text).collect();
    assert_eq!(bold, ["103"]);
}

#[test]
fn modes_that_already_show_the_status_line_dont_double_up() {
    for mode in [
        Mode::Playing,
        Mode::MainMenu,
        Mode::SaveAction,
        Mode::QuitAppConfirm,
        Mode::Research,
        Mode::Inventory,
        Mode::Trade,
    ] {
        assert!(
            !needs_status_banner(mode),
            "{mode:?} already surfaces status_line itself"
        );
    }
}

/// The row draws its setting's current value. The menu row that opens
/// this screen is asserted in `meta.rs`'s own tests, beside the private
/// `main_menu_options` those two already walk.
#[test]
fn the_options_screen_draws_its_toggle() {
    let mut app = census_app();
    app.mode = Mode::GameplayOptions;
    let m = ui_metrics(900.0);
    let (_, shapes) = crate::paint::with_painter(|p| draw_game_menu_page(&app, None, p, &m));
    let drawn = crate::paint::painted_text(&shapes);
    assert!(
        drawn
            .iter()
            .any(|t| t.contains("Tactical surface battles") && t.contains("On")),
        "the toggle row did not draw its value: {drawn:#?}"
    );
}

/// The width census: every game-menu page's rows, the Save & quit
/// confirm included, against the hub content rect they draw into at the
/// smallest window. A row too long draws past the frame in silence.
#[test]
fn no_game_menu_row_overflows_the_hub_content_at_1280x720() {
    let m = ui_metrics(720.0);
    let mut pages: Vec<(Mode, bool)> = [
        Mode::GameplayOptions,
        Mode::AudioOptions,
        Mode::GraphicsOptions,
        Mode::SpriteOptions,
        Mode::ManualOptions,
        Mode::QuitRunConfirm,
    ]
    .into_iter()
    .map(|mode| (mode, false))
    .collect();
    pages.push((Mode::QuitRunConfirm, true));
    crate::paint::with_painter(|p| {
        let char_w = p.measure_ui_advance("M", m.font_size);
        let content = base_report::content_rect_at(1280.0, 720.0, char_w, &m);
        for (mode, pending) in pages {
            let mut app = census_app();
            app.mode = mode;
            if pending {
                app.handle_key(feral_processes_app_core::GameKey::Char('3'));
            }
            let rows = if mode == Mode::QuitRunConfirm {
                meta::save_and_quit_rows(&app)
            } else {
                meta::options_page_rows(&app)
            };
            assert!(!rows.is_empty(), "{mode:?} drew no rows");
            for row in &rows {
                let text = match row {
                    Row::Text(t) | Row::TextColored(t, _) => t,
                    Row::Item { text, .. } => text,
                };
                let width = p.measure_ui_advance(text, m.font_size);
                assert!(
                    width <= content.w,
                    "{mode:?} draws {width}px into {}px: {text:?}",
                    content.w
                );
            }
        }
    });
}
