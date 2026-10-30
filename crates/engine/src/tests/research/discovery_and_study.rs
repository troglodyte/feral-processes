use super::*;

/// A RON round-trip alone is not enough: `#[serde(skip)]` on a new field
/// leaves that test green while the field never reaches disk. This goes
/// through the real `Game::save`/`Game::load` pair.
#[test]
fn a_save_round_trips_what_the_base_has_discovered_and_banked() {
    let mut game = Game::new(4420, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world
        .resource_mut::<crate::resources::DiscoveredResearch>()
        .0
        .insert("paging".to_string());
    game.world
        .resource_mut::<crate::resources::ActiveResearch>()
        .study = 3;

    let path = std::env::temp_dir().join(format!(
        "feral_research_discovery_save_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(
        loaded
            .world
            .resource::<crate::resources::DiscoveredResearch>()
            .0
            .contains("paging"),
        "a discovery is permanent and must survive a reload"
    );
    assert_eq!(
        loaded
            .world
            .resource::<crate::resources::ActiveResearch>()
            .study,
        3,
        "a part-banked study attempt must survive a reload"
    );
}

/// The one door: one id written, one base line, one notification — and a
/// second call on the same node is inert, which is what lets any caller fire
/// it without first asking whether the node is already known.
#[test]
fn discovering_a_node_writes_one_id_one_line_and_one_notification() {
    let mut game = Game::new(4430, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let name = game
        .world
        .resource::<ResearchDb>()
        .get("paging")
        .expect("paging ships")
        .name
        .clone();
    while game.take_notification().is_some() {}

    assert!(game.discover_research("paging"), "a fresh node is news");

    assert_eq!(
        game.world
            .resource::<crate::resources::DiscoveredResearch>()
            .0
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["paging".to_string()],
        "exactly one id, and only the one asked for"
    );
    let said: Vec<String> = game
        .message_history(500)
        .iter()
        .filter(|l| l.text.contains("uncovers"))
        .map(|l| l.text.clone())
        .collect();
    assert_eq!(said, vec![format!("The study uncovers {name}.")]);
    assert_eq!(game.notifications_pending(), 1);

    // Inert the second time: no second id, no second line, no second popup.
    while game.take_notification().is_some() {}
    assert!(!game.discover_research("paging"), "already found");
    assert_eq!(game.notifications_pending(), 0);
    assert_eq!(
        game.message_history(500)
            .iter()
            .filter(|l| l.text.contains("uncovers"))
            .count(),
        1
    );

    // An id nothing defines is refused rather than written — a save editor's
    // typo must not mint a node the tree has never heard of.
    assert!(!game.discover_research("not_a_node"));
    assert_eq!(
        game.world
            .resource::<crate::resources::DiscoveredResearch>()
            .0
            .len(),
        1
    );
}

/// A discoverable node is off **both** surfaces until it is found, and on
/// both after. One predicate feeds `listed_research`, and `research_nodes`
/// and `research_graph` are both built from it, so the menu and the flow
/// chart cannot disagree about what exists.
#[test]
fn a_discoverable_node_is_absent_from_both_research_surfaces_until_discovered() {
    let mut game = Game::new(4440, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let listed = |g: &Game| {
        g.research_nodes(ResearchTree::Base)
            .iter()
            .any(|n| n.id == "paging")
    };
    let drawn = |g: &Game| {
        g.research_graph(ResearchTree::Base)
            .cell("paging")
            .is_some()
    };

    assert!(!listed(&game), "an undiscovered node is not a menu row");
    assert!(!drawn(&game), "and not a box on the chart either");

    game.discover_research("paging");

    assert!(listed(&game));
    assert!(drawn(&game));
}

/// Leaving a node off the menu is not enough: an id typed into a save editor
/// or a stale UI row must be refused too — and refused **before anything is
/// written**, `commit_caravan_basket`'s rule.
#[test]
fn select_research_refuses_an_undiscovered_node_before_anything_is_written() {
    let mut game = Game::new(4441, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let station = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, station);

    let err = game.select_research("paging").unwrap_err();

    assert_eq!(
        err, "Unknown research.",
        "a hidden node must not be told apart from one that does not exist"
    );
    assert_eq!(active_research(&game), None, "and no project is started");
    assert!(game.work_orders().is_empty(), "and no bill is filed");

    // The reachability half: discovering it makes the identical call succeed,
    // so the refusal cannot ship permanent.
    game.discover_research("paging");
    game.select_research("paging")
        .expect("a discovered node is an ordinary node");
}

/// The regression gate on the visible spine. A node that is not
/// `discoverable` is listed and selectable exactly as it was — including the
/// two that cost a subject and stay visible on purpose.
#[test]
fn a_node_that_is_not_discoverable_is_untouched_by_the_hiding_rule() {
    let mut game = Game::new(4442, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    let ids: Vec<String> = game
        .research_nodes(ResearchTree::Base)
        .iter()
        .map(|n| n.id.clone())
        .collect();
    for id in [
        "automation",
        "power_grid",
        "commerce",
        "teardown",
        "fortification",
        "routine_fabrication",
        "program_refactoring",
    ] {
        assert!(ids.contains(&id.to_string()), "{id} must stay visible");
    }

    game.select_research("automation")
        .expect("the visible spine is bought exactly as before");
}

/// "Researched means known" reaches across this gate too: a node already
/// bought is never hidden by a later change to what gates it — a save from
/// before this feature, or a `.ron` edit that adds `discoverable` to
/// something the player already owns.
#[test]
fn an_already_researched_node_stays_listed_even_when_it_is_discoverable() {
    let mut game = Game::new(4443, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "armor_bench");
    assert!(
        game.world
            .resource::<crate::resources::DiscoveredResearch>()
            .0
            .is_empty(),
        "the fixture is vacuous if the chain discovered it on the way"
    );

    assert!(
        game.research_nodes(ResearchTree::Base)
            .iter()
            .any(|n| n.id == "armor_bench"),
        "a node you already own cannot become invisible"
    );
}

/// One payout lands in exactly one place, and the branch is `id.is_none()`
/// — never both, asserted on the same payout.
#[test]
fn a_research_payout_feeds_the_study_or_the_project_and_never_both() {
    let mut game = Game::new(4450, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    let worker = spawn_tamed(&mut game, 10, 3);
    park_at_post(&mut game, worker, node);
    game.set_standing_job(node, true, false).unwrap();

    for _ in 0..400 {
        game.tick();
    }

    let banked = game
        .world
        .resource::<crate::resources::ActiveResearch>()
        .study;
    assert!(
        banked > 0,
        "an idle Station's cycles must bank toward a study attempt, not land nowhere"
    );
    assert!(
        game.world
            .resource::<crate::resources::ActiveResearch>()
            .progress
            .is_empty(),
        "with nothing selected, nothing may reach a project's progress"
    );

    game.select_research("automation").unwrap();
    let before = game
        .world
        .resource::<crate::resources::ActiveResearch>()
        .study;
    for _ in 0..400 {
        game.tick();
    }

    assert_eq!(
        game.world
            .resource::<crate::resources::ActiveResearch>()
            .study,
        before,
        "with a project selected the study banks nothing — one or the other"
    );
    assert!(
        game.world
            .resource::<crate::resources::ActiveResearch>()
            .progress
            .values()
            .sum::<u32>()
            > 0,
        "and the project is the one that moves"
    );
}

/// The gate is `id.is_none()` and **not** `landed == 0`. A project sitting
/// at its cost also lands nothing and will settle next tick; feeding the
/// study from it would make "study or advance" a lie for one tick an
/// attentive player could farm.
#[test]
fn a_project_sitting_at_its_cost_banks_nothing_toward_a_study() {
    let mut game = Game::new(4451, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    let worker = spawn_tamed(&mut game, 10, 3);
    park_at_post(&mut game, worker, node);
    let cost = game
        .world
        .resource::<ResearchDb>()
        .get("cold_archive")
        .expect("a deep, expensive node nothing in this fixture will finish")
        .cost;
    game.discover_research("cold_archive");
    {
        let mut research = game
            .world
            .resource_mut::<crate::resources::ActiveResearch>();
        research.id = Some("cold_archive".to_string());
        research.progress.insert("cold_archive".to_string(), cost);
    }

    for _ in 0..400 {
        game.tick();
    }

    assert_eq!(
        game.world
            .resource::<crate::resources::ActiveResearch>()
            .study,
        0,
        "a full project lands nothing and must bank nothing"
    );
    assert_eq!(
        active_research(&game).as_deref(),
        Some("cold_archive"),
        "the fixture is vacuous if the project completed on its own"
    );
}

/// **Three early returns hold the bar; only a real attempt spends it.** A
/// full bar with an empty pen, or with nothing left to find, is banked and
/// fires the instant the condition clears — which is what makes a discovery
/// legible as a consequence of the player's action rather than of a timer
/// they cannot see. A spend-on-hold is the bug that makes it feel arbitrary.
#[test]
fn a_study_attempt_is_held_until_a_subject_and_a_pool_are_both_there() {
    let mut game = Game::new(4460, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    let full = crate::tuning::STUDY_ATTEMPT_DATA;

    // Held: a full bar, an empty pen.
    game.world
        .resource_mut::<crate::resources::ActiveResearch>()
        .study = full;
    game.settle_study();
    assert_eq!(
        game.world
            .resource::<crate::resources::ActiveResearch>()
            .study,
        full,
        "nothing is pinned, so the attempt waits rather than being spent"
    );

    // Held: a subject, but nothing eligible to find. Zone 1 with nothing
    // researched is exactly that state — every discoverable node either
    // waits on `automation` or is gated at `min_zone >= 2`.
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    assert!(
        !game.is_researched("automation"),
        "the fixture is vacuous if the pool is already open"
    );
    game.settle_study();
    assert_eq!(
        game.world
            .resource::<crate::resources::ActiveResearch>()
            .study,
        full,
        "nothing to find, so the attempt waits"
    );

    // Spent: both conditions clear.
    unlock_research_chain(&mut game, "automation");
    game.settle_study();
    assert_eq!(
        game.world
            .resource::<crate::resources::ActiveResearch>()
            .study,
        0,
        "with a subject and a pool, the attempt is spent whether or not it hits"
    );
}

/// The pool is the whole of what may be found: a prereq-unsatisfied node and
/// a zone-gated one are never picked, over enough forced attempts that the
/// absence means something. A discovery is therefore always immediately
/// researchable.
#[test]
fn a_study_never_discovers_outside_the_eligible_pool() {
    let mut game = Game::new(4461, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    unlock_research_chain(&mut game, "automation");
    set_zone(&mut game, 1);

    for _ in 0..500 {
        game.world
            .resource_mut::<crate::resources::ActiveResearch>()
            .study = crate::tuning::STUDY_ATTEMPT_DATA;
        game.settle_study();
        repin_after_a_find(&mut game, node);
    }

    let found: Vec<String> = game
        .world
        .resource::<crate::resources::DiscoveredResearch>()
        .0
        .iter()
        .cloned()
        .collect();
    assert!(
        !found.is_empty(),
        "500 forced attempts must find something, or this test proves nothing"
    );
    for id in &found {
        let def = game.world.resource::<ResearchDb>().get(id).unwrap().clone();
        assert!(
            def.min_zone <= 1,
            "{id} is gated above zone 1 and must not have been found here"
        );
        for req in &def.requires {
            assert!(
                game.is_researched(req),
                "{id} was found with {req} unresearched"
            );
        }
    }
}

/// A find spends the subject: clear the slot and pin a fresh one, so a
/// forced-attempt loop keeps rolling.
fn repin_after_a_find(game: &mut Game, node: Entity) {
    if game.pinned_subject().is_some() {
        return;
    }
    if let Some(mut slot) = game.world.get_mut::<crate::components::StudyYield>(node) {
        slot.0 = None;
    }
    let fresh = spawn_tamed(game, 10, 3);
    pin_subject_at_pen(game, fresh, node);
}

/// `settle_study` draws **no** `GameRng` on a tick where no attempt is made
/// — the arena's own rule, so a base that is not studying cannot shift the
/// seeded stream out from under everything else in the run.
#[test]
fn settle_study_draws_no_rng_when_no_attempt_is_made() {
    let mut game = Game::new(4462, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    assert_eq!(
        game.world
            .resource::<crate::resources::ActiveResearch>()
            .study,
        0,
        "no bar, so no attempt"
    );

    fn peek(g: &mut Game) -> u64 {
        g.world
            .resource_mut::<crate::resources::GameRng>()
            .0
            .random()
    }

    reseed_rng(&mut game, 55);
    let without = peek(&mut game);

    reseed_rng(&mut game, 55);
    game.settle_study();
    let with = peek(&mut game);

    assert_eq!(
        without, with,
        "an idle study must not touch the shared GameRng stream"
    );
}

/// The miss line: an attempt that is spent and finds nothing says so, and
/// says it as base news. `resources::condense` already folds repeats on all
/// three log surfaces, so this needs no rate limit of its own.
#[test]
fn a_failed_study_attempt_says_so() {
    let mut game = Game::new(4463, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    unlock_research_chain(&mut game, "automation");

    for _ in 0..200 {
        game.world
            .resource_mut::<crate::resources::ActiveResearch>()
            .study = crate::tuning::STUDY_ATTEMPT_DATA;
        game.settle_study();
        repin_after_a_find(&mut game, node);
    }

    assert!(
        game.message_history(500)
            .iter()
            .any(|l| l.text.contains("The study turns up nothing.")),
        "200 attempts against a sub-1.0 chance must miss at least once, and a miss is news"
    );
}

/// **The feature, end to end, through nothing but ticks.** Every test above
/// reaches into `ActiveResearch::study` or calls `settle_study` directly;
/// this one posts a program, pins a subject and waits, so a seam left
/// unjoined — the accrual not wired, `settle_study` not called from the tick
/// — fails here and nowhere else.
#[test]
fn an_idle_station_with_a_subject_pinned_uncovers_a_node_on_its_own() {
    let mut game = Game::new(4470, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    unlock_research_chain(&mut game, "automation");
    let worker = spawn_tamed(&mut game, 10, 3);
    park_at_post(&mut game, worker, node);
    game.set_standing_job(node, true, false).unwrap();
    let subject = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, subject, node);
    assert_eq!(
        active_research(&game),
        None,
        "the Station studies, not works"
    );

    for _ in 0..4000 {
        game.tick();
    }

    let found: Vec<String> = game
        .world
        .resource::<crate::resources::DiscoveredResearch>()
        .0
        .iter()
        .cloned()
        .collect();
    assert_eq!(
        found,
        vec!["armor_bench".to_string()],
        "zone 1 with only automation researched has exactly one eligible node"
    );
    for id in &found {
        assert!(
            game.research_nodes(ResearchTree::Base)
                .iter()
                .any(|n| &n.id == id),
            "{id} was found and must arrive on the menu"
        );
    }
    assert!(
        game.world.get_entity(subject).is_err(),
        "the discovery spent the subject"
    );
}

#[test]
fn decompile_reach_is_empty_with_nothing_researched() {
    let game = Game::new(5, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(
        game.decompile_reach(),
        DecompileReach {
            range: None,
            radius: 0
        }
    );
}

#[test]
fn decompile_reach_folds_researched_nodes_by_max_per_field() {
    use crate::research::DecompilerUpgrade as U;
    let mut game = Game::new(5, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    research_decompiler(
        &mut game,
        "t_a",
        U {
            range: Some(4),
            radius: None,
        },
    );
    research_decompiler(
        &mut game,
        "t_b",
        U {
            range: Some(6),
            radius: Some(1),
        },
    );
    research_decompiler(
        &mut game,
        "t_c",
        U {
            range: Some(2),
            radius: Some(2),
        },
    );
    assert_eq!(
        game.decompile_reach(),
        DecompileReach {
            range: Some(6),
            radius: 2
        }
    );
}

#[test]
fn decompile_reach_ignores_an_unresearched_node() {
    use crate::research::DecompilerUpgrade as U;
    let mut game = Game::new(5, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    research_decompiler(
        &mut game,
        "t_a",
        U {
            range: Some(4),
            radius: None,
        },
    );
    game.world
        .resource_mut::<crate::resources::Research>()
        .0
        .remove("t_a");
    assert_eq!(game.decompile_reach().range, None);
}

/// A decompiler node says what it grants from its `decompiler` field, not from
/// the prose, so a retuned range cannot leave the menu quoting the old one.
#[test]
fn a_decompiler_node_reports_its_reach_or_area() {
    let mut game = Game::new(718, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    discover_all_research(&mut game);

    assert_eq!(
        research_node(&game, "remote_decompile").unlocks.as_deref(),
        Some("Decompile reach: 4")
    );
    assert_eq!(
        research_node(&game, "broadcast_decompile")
            .unlocks
            .as_deref(),
        Some("Decompile area: radius 1")
    );
}

/// The research screen's study line walks the four states a player can be
/// in, and goes quiet when a study has nothing to find.
#[test]
fn the_study_hint_names_what_stands_between_the_player_and_a_find() {
    let mut bare = Game::new(4470, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut bare);
    unlock_research_chain(&mut bare, "automation");
    assert_eq!(
        bare.study_hint().as_deref(),
        Some(
            "Hidden research is found by study: build a Research Station and pin a program in its pen."
        )
    );

    let mut game = Game::new(4471, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    assert!(
        game.eligible_discoveries().is_empty(),
        "the fixture is vacuous if the pool is already open"
    );
    assert_eq!(game.study_hint(), None, "nothing to find, nothing to say");

    unlock_research_chain(&mut game, "automation");
    assert_eq!(
        game.study_hint().as_deref(),
        Some("Pin a program in the Research Station's pen to study it for hidden research.")
    );

    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    game.world
        .resource_mut::<crate::resources::ActiveResearch>()
        .study = 2;
    assert_eq!(
        game.study_hint(),
        Some(format!(
            "Studying for hidden research: 2/{} Research Data to the next attempt.",
            crate::tuning::STUDY_ATTEMPT_DATA
        ))
    );

    game.world
        .resource_mut::<crate::resources::ActiveResearch>()
        .id = Some("commerce".to_string());
    assert_eq!(
        game.study_hint().as_deref(),
        Some(
            "Study is paused while a project runs. Abandon it (A) to look for hidden research; its progress is kept."
        )
    );
}
