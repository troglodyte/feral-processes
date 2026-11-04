use super::*;
use crate::components::ProgramId;
use crate::tuning::{BASE_AI_FUGUE_TICKS, BASE_AI_START_SPEED};

fn cand(id: u32, analysis: i32, free: bool) -> HostCandidate {
    HostCandidate {
        id: ProgramId(id),
        analysis,
        free,
    }
}

fn spot(anchor: (i32, i32), distance: u32) -> StationSpot {
    StationSpot { anchor, distance }
}

fn input<'a>(
    researched: u32,
    candidates: &'a [HostCandidate],
    stations: &'a [StationSpot],
) -> StepInput<'a> {
    StepInput {
        researched,
        trigger: 7,
        candidates,
        stations,
        host_free: true,
        host_arrived: false,
        host_analysis: 20,
        host_pace: 0.4,
    }
}

fn fused() -> FusedAi {
    FusedAi {
        host: ProgramId(1),
        station: (0, 0),
        level: 0,
        analysis_at_fusion: 20,
        pace_at_fusion: 0.4,
        practice: 0,
        carry: 0.0,
        training: TrainingTarget::default(),
    }
}

#[test]
fn trigger_count_is_between_five_and_ten_for_every_seed() {
    for seed in 0..10_000 {
        assert!((5..=10).contains(&trigger_count(seed)), "seed {seed}");
    }
}

#[test]
fn trigger_count_spreads_across_the_range() {
    let seen: std::collections::BTreeSet<u32> = (0..10_000).map(trigger_count).collect();
    assert_eq!(
        seen.into_iter().collect::<Vec<_>>(),
        vec![5, 6, 7, 8, 9, 10]
    );
}

#[test]
fn pick_host_takes_the_highest_free_analysis_and_ties_go_to_the_lower_id() {
    let c = [cand(4, 30, true), cand(2, 30, true), cand(1, 20, true)];
    assert_eq!(pick_host(&c), Some(ProgramId(2)));
    let c = [cand(3, 10, true), cand(5, 25, true)];
    assert_eq!(pick_host(&c), Some(ProgramId(5)));
}

#[test]
fn pick_host_skips_unfree_programs() {
    let c = [cand(1, 99, false), cand(2, 10, true)];
    assert_eq!(pick_host(&c), Some(ProgramId(2)));
}

#[test]
fn pick_host_with_no_free_program_is_none() {
    assert_eq!(pick_host(&[]), None);
    assert_eq!(pick_host(&[cand(1, 50, false)]), None);
}

#[test]
fn pick_station_takes_the_nearest_and_ties_go_to_the_lower_anchor() {
    let s = [spot((9, 9), 3), spot((5, 1), 3), spot((0, 0), 8)];
    assert_eq!(pick_station(&s), Some((5, 1)));
    assert_eq!(pick_station(&[]), None);
}

#[test]
fn step_stays_dormant_below_the_trigger() {
    let c = [cand(1, 20, true)];
    let s = [spot((0, 0), 1)];
    let out = step(&BaseAi::default(), &input(6, &c, &s));
    assert_eq!(out, StepOutcome::None);
}

#[test]
fn step_begins_the_claim_at_the_trigger_even_with_no_research_selected() {
    let c = [cand(1, 20, true)];
    let s = [spot((2, 3), 1)];
    let out = step(&BaseAi::default(), &input(7, &c, &s));
    assert_eq!(
        out,
        StepOutcome::BeginClaim {
            host: ProgramId(1),
            station: (2, 3),
            announce: true
        }
    );
}

#[test]
fn step_does_not_announce_a_second_claim() {
    let c = [cand(1, 20, true)];
    let s = [spot((2, 3), 1)];
    let out = step(&BaseAi::Dormant { announced: true }, &input(12, &c, &s));
    assert!(matches!(
        out,
        StepOutcome::BeginClaim {
            announce: false,
            ..
        }
    ));
}

#[test]
fn step_waits_when_no_candidate_or_no_station() {
    let c = [cand(1, 20, true)];
    let s = [spot((2, 3), 1)];
    let none = step(&BaseAi::default(), &input(9, &[], &s));
    assert_eq!(none, StepOutcome::None);
    let none = step(&BaseAi::default(), &input(9, &c, &[]));
    assert_eq!(none, StepOutcome::None);
}

#[test]
fn step_abandons_the_claim_when_the_host_stops_being_free() {
    let arc = BaseAi::Claiming {
        host: ProgramId(1),
        station: (0, 0),
    };
    let mut i = input(9, &[], &[]);
    i.host_free = false;
    assert_eq!(step(&arc, &i), StepOutcome::AbandonClaim);
}

#[test]
fn step_starts_the_fugue_on_arrival_and_fuses_after_120_ticks() {
    let mut arc = BaseAi::Claiming {
        host: ProgramId(1),
        station: (4, 4),
    };
    let mut i = input(9, &[], &[]);
    assert_eq!(step(&arc, &i), StepOutcome::None);
    i.host_arrived = true;
    assert_eq!(step(&arc, &i), StepOutcome::StartFugue);

    arc = BaseAi::Fugue {
        host: ProgramId(1),
        station: (4, 4),
        ticks_left: BASE_AI_FUGUE_TICKS,
    };
    assert_eq!(BASE_AI_FUGUE_TICKS, 120);
    let mut ticks = 0;
    let fused = loop {
        match step(&arc, &i) {
            StepOutcome::TickFugue => {
                ticks += 1;
                let BaseAi::Fugue { ticks_left, .. } = &mut arc else {
                    unreachable!()
                };
                *ticks_left -= 1;
            }
            StepOutcome::Fuse(f) => break f,
            other => panic!("unexpected {other:?}"),
        }
    };
    assert_eq!(ticks, 120);
    assert_eq!(fused.level, 0);
    assert_eq!(fused.analysis_at_fusion, i.host_analysis);
    assert_eq!(fused.pace_at_fusion, i.host_pace);
    assert_eq!((fused.host, fused.station), (ProgramId(1), (4, 4)));
}

#[test]
fn pace_starts_at_a_quarter_and_rises_with_analysis_without_a_ceiling() {
    let f = fused();
    let (a0, p0) = (f.analysis_at_fusion, f.pace_at_fusion);
    let near = |x: f64, y: f64| (x - y).abs() < 1e-9;
    assert_eq!(BASE_AI_START_SPEED, 0.25);
    assert!(near(pace(&f, a0), 0.25 * p0));
    assert!(near(pace(&f, 4 * a0), p0));
    assert!(near(pace(&f, 40 * a0), 10.0 * p0));
}

#[test]
fn practise_raises_analysis_and_each_point_costs_more() {
    let f = fused();
    let first = practise(&f, 20, practice_cost(0));
    assert_eq!(first.analysis, 21);
    // One tick short of the second point's price buys nothing more.
    let second = practise(&f, 21, practice_cost(1) - 1);
    assert_eq!(second.analysis, 21);
    assert!(practice_cost(1) > practice_cost(0));
    assert!(practice_cost(5) > practice_cost(1));
}

#[test]
fn practise_stalls_exactly_at_the_cap_and_drops_overflow() {
    let f = fused();
    let cap = analysis_cap(&f);
    let out = practise(&f, 20, 1_000_000);
    assert_eq!(out.analysis, cap);
    assert!(out.stalled);
    assert_eq!(out.practice, 0);
    // Already stalled: nothing moves.
    let again = practise(&f, cap, 500);
    assert_eq!(again, out);
}

#[test]
fn practise_after_the_last_offset_never_stalls() {
    let mut f = fused();
    f.level = 5;
    assert_eq!(analysis_cap(&f), i32::MAX);
    let out = practise(&f, 125, 100_000);
    assert!(!out.stalled);
    assert!(out.analysis > 125);
}

/// The arc driven through a real `Game`: a floored base with a Research
/// Station and three staff programs of distinct Analysis.
mod in_a_game {
    use super::*;
    use crate::resources::ActiveResearch;
    use crate::tests::support::*;
    use crate::*;

    const STAFF_ANALYSIS: [i32; 3] = [18, 31, 24];

    struct Fixture {
        game: Game,
        station: Entity,
        staff: Vec<Entity>,
    }

    fn fixture() -> Fixture {
        let mut game = Game::new(5200, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game);
        lay_floor_box(&mut game, (-12, -12), (12, 12));
        let station = base_with_a_research_node(&mut game);
        let staff = STAFF_ANALYSIS
            .iter()
            .enumerate()
            .map(|(i, &analysis)| {
                let program = spawn_tamed(&mut game, 20, 5);
                *game.world.get_mut::<Position>(program).unwrap() = Position {
                    x: -3 - i as i32,
                    y: -3,
                };
                game.world
                    .get_mut::<components::Attributes>(program)
                    .unwrap()
                    .set(&crate::attributes::AttributeId::from("analysis"), analysis);
                program
            })
            .collect();
        Fixture {
            game,
            station,
            staff,
        }
    }

    fn trigger(game: &Game) -> u32 {
        trigger_count(game.world.resource::<WorldMap>().seed())
    }

    fn set_researched(game: &mut Game, n: u32) {
        let mut research = game.world.resource_mut::<Research>();
        research.0.clear();
        research
            .0
            .extend((0..n).map(|i| format!("storyline_test_{i}")));
    }

    fn program_id(game: &Game, e: Entity) -> ProgramId {
        *game.world.get::<ProgramId>(e).unwrap()
    }

    fn tick_until(game: &mut Game, limit: u32, done: impl Fn(&Game) -> bool) {
        for _ in 0..limit {
            if done(game) {
                return;
            }
            game.tick();
        }
        assert!(done(game), "not reached in {limit} ticks");
    }

    fn is_fused(game: &Game) -> bool {
        matches!(game.storyline().base_ai, BaseAi::Fused(_))
    }

    #[test]
    fn the_awakening_fires_once_research_reaches_the_trigger() {
        let mut f = fixture();
        let n = trigger(&f.game);
        set_researched(&mut f.game, n - 1);
        f.game.tick();
        assert!(matches!(f.game.storyline().base_ai, BaseAi::Dormant { .. }));
        set_researched(&mut f.game, n);
        f.game.tick();
        let strongest = program_id(&f.game, f.staff[1]);
        assert!(matches!(
            f.game.storyline().base_ai,
            BaseAi::Claiming { host, .. } if host == strongest
        ));
        assert!(
            f.game
                .world
                .get::<components::BaseAiHost>(f.staff[1])
                .is_some()
        );
    }

    #[test]
    fn a_save_already_past_ten_claims_on_the_first_tick() {
        let mut f = fixture();
        set_researched(&mut f.game, 25);
        f.game.tick();
        assert!(matches!(
            f.game.storyline().base_ai,
            BaseAi::Claiming { .. }
        ));
    }

    /// Gives the base a project with progress to lose.
    fn with_progress_to_lose(f: &mut Fixture) -> (String, u32) {
        let id = f
            .game
            .world
            .resource::<ResearchDb>()
            .all()
            .find(|d| d.requires.is_empty() && d.cost > 5)
            .map(|d| d.id.clone())
            .expect("a cheap root project ships");
        let mut active = f.game.world.resource_mut::<ActiveResearch>();
        active.id = Some(id.clone());
        active.progress.insert(id.clone(), 3);
        (id, 3)
    }

    #[test]
    fn progress_on_the_active_project_survives_the_fugue() {
        let mut f = fixture();
        let (id, before) = with_progress_to_lose(&mut f);
        set_researched(&mut f.game, 25);
        tick_until(&mut f.game, 400, is_fused);
        let active = f.game.world.resource::<ActiveResearch>();
        assert_eq!(active.id.as_deref(), Some(id.as_str()));
        assert_eq!(active.progress.get(&id), Some(&before));
    }

    #[test]
    fn no_research_lands_during_the_claim_or_the_fugue() {
        let mut f = fixture();
        with_progress_to_lose(&mut f);
        // A program already working the Station when the claim begins.
        let worker = f.staff[0];
        f.game.post_worker(worker, f.station);
        park_at_post(&mut f.game, worker, f.station);
        set_researched(&mut f.game, 25);
        let snapshot = |g: &Game| {
            let a = g.world.resource::<ActiveResearch>();
            (a.study, a.progress.clone())
        };
        let before = snapshot(&f.game);
        f.game.tick();
        assert!(
            matches!(f.game.storyline().base_ai, BaseAi::Claiming { .. }),
            "the claim begins on the first tick"
        );
        while !is_fused(&f.game) {
            f.game.tick();
            assert_eq!(snapshot(&f.game), before);
        }
    }

    #[test]
    fn the_host_fuses_after_the_fugue_and_has_no_task() {
        let mut f = fixture();
        set_researched(&mut f.game, 25);
        tick_until(&mut f.game, 400, is_fused);
        let BaseAi::Fused(fused) = f.game.storyline().base_ai.clone() else {
            unreachable!()
        };
        let host = f.game.base_ai_host().expect("the host is still there");
        assert_eq!(host, f.staff[1]);
        assert!(f.game.world.get::<Task>(host).is_none());
        assert_eq!(fused.level, 0);
        assert_eq!(fused.analysis_at_fusion, STAFF_ANALYSIS[1]);
        assert!(
            fused.pace_at_fusion > 0.0,
            "the frozen pace is a real figure"
        );
        assert_eq!(f.game.base_ai_station(), Some(f.station));
        // And the scheduler leaves it alone from then on.
        for _ in 0..20 {
            f.game.tick();
            assert!(f.game.world.get::<Task>(host).is_none());
        }
        assert!(!f.game.base_staff().contains(&host));
    }

    #[test]
    fn a_host_downed_mid_walk_abandons_and_the_next_claim_does_not_re_announce() {
        let mut f = fixture();
        set_researched(&mut f.game, 25);
        f.game.tick();
        let first = f.staff[1];
        assert!(f.game.world.get::<components::BaseAiHost>(first).is_some());
        f.game.world.entity_mut(first).insert(components::Downed);
        f.game.tick();
        assert!(f.game.world.get::<components::BaseAiHost>(first).is_none());
        assert!(matches!(
            f.game.storyline().base_ai,
            BaseAi::Dormant { announced: true } | BaseAi::Claiming { .. }
        ));
        f.game.tick();
        let next = program_id(&f.game, f.staff[2]);
        assert!(matches!(
            f.game.storyline().base_ai,
            BaseAi::Claiming { host, .. } if host == next
        ));
        let told = storyline_notifications(&mut f.game).len();
        assert_eq!(told, 1, "the awakening is told once");
    }

    /// Drains the queue and keeps what carries the Storyline category.
    fn storyline_notifications(game: &mut Game) -> Vec<crate::notifications::Notification> {
        std::iter::from_fn(|| game.take_notification())
            .filter(|n| n.category == Some(crate::notifications::NotificationCategory::Storyline))
            .collect()
    }

    #[test]
    fn the_awakening_notification_names_the_host() {
        let mut f = fixture();
        let n = trigger(&f.game);
        set_researched(&mut f.game, n);
        f.game.tick();
        let name = f.game.creature_label(f.staff[1]);
        let told = storyline_notifications(&mut f.game);
        assert_eq!(told.len(), 1, "{told:?}");
        assert!(told[0].body.contains(&name), "{}", told[0].body);
        assert!(!told[0].body.contains('{'), "an unfilled hole");
    }

    /// A fixture driven to Fused: the host is `staff[1]`.
    fn fused_fixture() -> (Fixture, Entity) {
        let mut f = fixture();
        set_researched(&mut f.game, 25);
        tick_until(&mut f.game, 400, is_fused);
        let host = f.staff[1];
        (f, host)
    }

    fn home_entity(game: &mut Game) -> Entity {
        game.world
            .query::<(Entity, &Structure)>()
            .iter(&game.world)
            .find(|(_, s)| s.kind == "home")
            .map(|(e, _)| e)
            .expect("the fixture stands a Home")
    }

    #[test]
    fn the_base_ai_is_never_base_staff() {
        let (f, host) = fused_fixture();
        assert_eq!(f.game.program_role(host), Some(ProgramRole::BaseAi));
        assert!(!f.game.base_staff().contains(&host));
    }

    #[test]
    fn the_base_ai_cannot_join_the_party() {
        let (mut f, host) = fused_fixture();
        let err = f.game.add_companion(host).expect_err("refused");
        assert!(err.contains("Research Station"), "{err}");
        assert!(!f.game.world.resource::<Party>().0.contains(&host));
    }

    #[test]
    fn the_base_ai_cannot_be_wielded() {
        let (mut f, host) = fused_fixture();
        let err = f.game.wield_program(host).expect_err("refused");
        assert!(err.contains("Research Station"), "{err}");
    }

    #[test]
    fn the_base_ai_cannot_be_fused() {
        let (mut f, host) = fused_fixture();
        unlock_research_chain(&mut f.game, "program_refactoring");
        let other = f.staff[0];
        let err = f
            .game
            .fuse_companions(host, other, None)
            .expect_err("refused");
        assert!(err.contains("Research Station"), "{err}");
        assert!(f.game.world.get::<Creature>(host).is_some());
    }

    #[test]
    fn the_base_ai_cannot_be_sold() {
        let (mut f, host) = fused_fixture();
        let market = spawn_market(&mut f.game);
        let err = f.game.sell_companion(market, host).expect_err("refused");
        assert!(err.contains("Research Station"), "{err}");
        assert!(f.game.world.get::<Creature>(host).is_some());
    }

    #[test]
    fn the_base_ai_cannot_go_on_a_sortie() {
        let (f, host) = fused_fixture();
        // `dispatch_sortie` admits only `ProgramRole::Staff`.
        assert_ne!(f.game.program_role(host), Some(ProgramRole::Staff));
    }

    #[test]
    fn the_base_ai_cannot_be_spent_on_a_build() {
        let (mut f, host) = fused_fixture();
        assert!(
            f.game
                .programs_for_build(0)
                .iter()
                .all(|p| p.entity != host)
        );
        assert!(f.game.commit_program(host).is_none());
        assert!(f.game.world.get::<Creature>(host).is_some());
    }

    #[test]
    fn a_second_research_station_is_refused_after_fusion() {
        let (mut f, _) = fused_fixture();
        let err = f
            .game
            .place_structure("research_node", 5, 5, None)
            .expect_err("refused");
        assert!(err.contains("Research Station"), "{err}");
    }

    #[test]
    fn the_ai_station_cannot_be_removed() {
        let (mut f, _) = fused_fixture();
        let err = f.game.remove_structure(f.station).expect_err("refused");
        assert!(err.contains("Research Station"), "{err}");
        assert!(f.game.world.get::<Structure>(f.station).is_some());
    }

    #[test]
    fn removing_the_home_leaves_the_ai_station() {
        let (mut f, _) = fused_fixture();
        let home = home_entity(&mut f.game);
        f.game.remove_structure(home).expect("the Home comes down");
        assert!(f.game.world.get::<Structure>(f.station).is_some());
        assert_eq!(f.game.base_ai_station(), Some(f.station));
    }

    #[test]
    fn a_subject_pinned_elsewhere_is_released_at_fusion_and_cannot_be_re_pinned_there() {
        let mut f = fixture();
        let other = spawn_machine_at(&mut f.game, "research_node", -8, 4);
        let subject = f.staff[0];
        f.game
            .pin_subject(subject, other)
            .expect("pins before the arc");
        assert!(
            f.game
                .world
                .get::<components::UnderStudy>(subject)
                .is_some()
        );
        set_researched(&mut f.game, 25);
        tick_until(&mut f.game, 400, is_fused);
        assert!(
            f.game
                .world
                .get::<components::UnderStudy>(subject)
                .is_none(),
            "released at fusion"
        );
        let err = f.game.pin_subject(subject, other).expect_err("refused");
        assert!(err.contains("base AI"), "{err}");
    }

    #[test]
    fn the_manifest_marks_the_base_ai_and_shows_its_true_analysis() {
        let (mut f, host) = fused_fixture();
        f.game
            .world
            .get_mut::<components::Attributes>(host)
            .unwrap()
            .set(&crate::attributes::AttributeId::from("analysis"), 120);
        let row = f
            .game
            .manifest_roster()
            .into_iter()
            .find(|r| r.entity == host)
            .expect("the host stays on the manifest");
        assert!(row.base_ai);
        assert!(
            f.game
                .manifest_roster()
                .iter()
                .filter(|r| r.entity != host)
                .all(|r| !r.base_ai)
        );
        let view = f.game.manifest(host).expect("a full stat sheet");
        assert_eq!(f.game.analysis_of_program(host), 120);
        let crate::views::ManifestSubject::Program(program) = view.subject else {
            panic!("a program's sheet");
        };
        assert_eq!(program.analysis, 120);
    }

    fn round_trip(game: &mut Game, tag: &str) -> Game {
        let path =
            std::env::temp_dir().join(format!("feral_storyline_{tag}_{}.sav", std::process::id()));
        game.save(&path).unwrap();
        let loaded = Game::load(&path, &test_assets_dir()).unwrap();
        let _ = std::fs::remove_file(&path);
        loaded
    }

    #[test]
    fn the_storyline_round_trips_in_every_stage() {
        let mut f = fixture();
        let fused = FusedAi {
            host: ProgramId(7),
            station: (2, 2),
            level: 2,
            analysis_at_fusion: 31,
            pace_at_fusion: 0.4375,
            practice: 12,
            carry: 0.625,
            training: TrainingTarget {
                selected: true,
                progress: 9,
            },
        };
        let stages = [
            ("dormant", BaseAi::Dormant { announced: true }),
            (
                "claiming",
                BaseAi::Claiming {
                    host: ProgramId(7),
                    station: (2, 2),
                },
            ),
            (
                "fugue",
                BaseAi::Fugue {
                    host: ProgramId(7),
                    station: (2, 2),
                    ticks_left: 57,
                },
            ),
            ("fused", BaseAi::Fused(fused)),
        ];
        for (tag, stage) in stages {
            f.game.storyline.base_ai = stage.clone();
            let loaded = round_trip(&mut f.game, tag);
            assert_eq!(loaded.storyline().base_ai, stage, "{tag}");
        }
    }
}
