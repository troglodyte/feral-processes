use super::*;
use crate::components::ProgramId;
use crate::tuning::{
    BASE_AI_FUGUE_TICKS, BASE_AI_PRACTICE_BASE, BASE_AI_PRACTICE_PER_TICK, BASE_AI_START_SPEED,
};

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
        fixture_in(&test_assets_dir())
    }

    fn fixture_in(assets: &std::path::Path) -> Fixture {
        let mut game = Game::new(5200, DifficultyMode::Forgiving, assets).unwrap();
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
    fn a_held_storyline_stays_dormant_past_the_trigger() {
        let mut f = fixture();
        f.game.dev_set_storyline(false);
        set_researched(&mut f.game, 25);
        f.game.tick();
        assert!(matches!(f.game.storyline().base_ai, BaseAi::Dormant { .. }));
        f.game.dev_set_storyline(true);
        f.game.tick();
        assert!(matches!(
            f.game.storyline().base_ai,
            BaseAi::Claiming { .. }
        ));
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
        // The fixture studies at a temporary station of its own, which a
        // fused arc would no longer treat as the study station.
        let fused = std::mem::take(&mut f.game.storyline.base_ai);
        unlock_research_chain(&mut f.game, "program_refactoring");
        f.game.storyline.base_ai = fused;
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

    /// The fused fixture with the arc's figures replaced, so a test reads
    /// the shell's arithmetic and not the formula behind `pace_at_fusion`.
    fn fused_with(pace: f64, practice: u32) -> (Fixture, Entity, String) {
        let (mut f, host) = fused_fixture();
        let (id, _) = with_progress_to_lose(&mut f);
        f.game
            .world
            .resource_mut::<ActiveResearch>()
            .progress
            .insert(id.clone(), 0);
        let BaseAi::Fused(fused) = &mut f.game.storyline.base_ai else {
            unreachable!()
        };
        fused.pace_at_fusion = pace;
        fused.practice = practice;
        fused.carry = 0.0;
        (f, host, id)
    }

    fn progress_of(game: &Game, id: &str) -> u32 {
        game.world
            .resource::<ActiveResearch>()
            .progress
            .get(id)
            .copied()
            .unwrap_or(0)
    }

    fn analysis_of(game: &Game, host: Entity) -> i32 {
        game.analysis_of_program(host)
    }

    fn waiting_alerts(game: &Game) -> usize {
        game.alerts()
            .iter()
            .filter(|a| a.kind == crate::alerts::AlertKind::BaseAiWaiting)
            .count()
    }

    #[test]
    fn the_ai_researches_at_a_quarter_of_its_old_pace_at_fusion() {
        let (mut f, host, id) = fused_with(0.4, 0);
        let start = analysis_of(&f.game, host);
        let BaseAi::Fused(fused) = f.game.storyline().base_ai.clone() else {
            unreachable!()
        };
        assert_eq!(start, fused.analysis_at_fusion);
        for _ in 0..40 {
            f.game.tick();
        }
        // 40 ticks at 0.4 * 0.25, with no Analysis gained yet.
        let landed = progress_of(&f.game, &id);
        assert!((3..=5).contains(&landed), "{landed}");
    }

    #[test]
    fn the_ai_banks_study_when_nothing_is_selected() {
        let (mut f, _host, _id) = fused_with(4.0, 0);
        f.game.world.resource_mut::<ActiveResearch>().id = None;
        for _ in 0..8 {
            f.game.tick();
        }
        let study = f.game.world.resource::<ActiveResearch>().study;
        assert!(study > 0, "a study attempt is banking");
    }

    #[test]
    fn analysis_grows_while_it_researches_and_shows_on_its_attributes() {
        let (mut f, host, _id) = fused_with(0.0, 0);
        let start = analysis_of(&f.game, host);
        // The first point costs `BASE_AI_PRACTICE_BASE` at `BASE_AI_PRACTICE_PER_TICK` a tick.
        for _ in 0..(BASE_AI_PRACTICE_BASE / BASE_AI_PRACTICE_PER_TICK) {
            f.game.tick();
        }
        assert_eq!(analysis_of(&f.game, host), start + 1);
        let stored = f
            .game
            .world
            .get::<components::Attributes>(host)
            .unwrap()
            .get(&crate::attributes::AttributeId::from("analysis"));
        assert_eq!(stored, Some(start + 1));
    }

    #[test]
    fn analysis_stops_at_the_threshold_and_the_waiting_alert_posts() {
        let (mut f, host, _id) = fused_with(0.0, 49);
        let BaseAi::Fused(fused) = f.game.storyline().base_ai.clone() else {
            unreachable!()
        };
        let cap = crate::storyline::analysis_cap(&fused);
        f.game
            .world
            .get_mut::<components::Attributes>(host)
            .unwrap()
            .set(&crate::attributes::AttributeId::from("analysis"), cap - 1);
        // One point short of the cap, with the next one a tick away.
        let BaseAi::Fused(fused) = &mut f.game.storyline.base_ai else {
            unreachable!()
        };
        fused.practice = crate::storyline::practice_cost(cap - 1 - fused.analysis_at_fusion) - 1;
        assert_eq!(waiting_alerts(&f.game), 0);
        f.game.tick();
        assert_eq!(analysis_of(&f.game, host), cap);
        assert_eq!(waiting_alerts(&f.game), 1);
        for _ in 0..30 {
            f.game.tick();
        }
        assert_eq!(analysis_of(&f.game, host), cap, "no overshoot");
        assert_eq!(waiting_alerts(&f.game), 1, "posted once, on the stall");
    }

    #[test]
    fn a_stall_does_not_stop_the_running_project() {
        let (mut f, host, id) = fused_with(4.0, 0);
        let BaseAi::Fused(fused) = f.game.storyline().base_ai.clone() else {
            unreachable!()
        };
        f.game
            .world
            .get_mut::<components::Attributes>(host)
            .unwrap()
            .set(
                &crate::attributes::AttributeId::from("analysis"),
                crate::storyline::analysis_cap(&fused),
            );
        let before = progress_of(&f.game, &id);
        for _ in 0..4 {
            f.game.tick();
        }
        assert!(progress_of(&f.game, &id) > before);
    }

    #[test]
    fn other_stations_produce_nothing_after_fusion() {
        let (mut f, _host, id) = fused_with(0.0, 0);
        // The project is selected, so the scheduler wants the Station worked
        // and would post a staff program to it were it not the AI's.
        for _ in 0..40 {
            f.game.tick();
            let posted = f
                .game
                .world
                .query::<&Task>()
                .iter(&f.game.world)
                .any(|t| t.target == f.station);
            assert!(!posted, "nobody is posted to the Station");
        }
        assert_eq!(progress_of(&f.game, &id), 0);
    }

    /// Stalls the host at its cap, so "Train your AI" is on offer.
    fn stall(game: &mut Game, host: Entity) {
        let BaseAi::Fused(fused) = game.storyline().base_ai.clone() else {
            unreachable!()
        };
        game.world
            .get_mut::<components::Attributes>(host)
            .unwrap()
            .set(
                &crate::attributes::AttributeId::from("analysis"),
                crate::storyline::analysis_cap(&fused),
            );
    }

    fn fused_state(game: &Game) -> FusedAi {
        let BaseAi::Fused(fused) = game.storyline().base_ai.clone() else {
            unreachable!()
        };
        fused
    }

    #[test]
    fn training_is_refused_until_the_ai_stalls() {
        let (mut f, host, _id) = fused_with(0.0, 0);
        assert!(f.game.select_ai_training().is_err());
        assert!(!fused_state(&f.game).training.selected);
        assert!(f.game.base_ai_view().is_some_and(|v| !v.training_available));
        stall(&mut f.game, host);
        assert!(f.game.base_ai_view().is_some_and(|v| v.training_available));
        f.game.select_ai_training().unwrap();
        assert!(fused_state(&f.game).training.selected);
        assert_eq!(f.game.world.resource::<ActiveResearch>().id, None);
        assert!(f.game.ai_training_wants_subject());
    }

    #[test]
    fn training_wants_a_subject_and_consumes_it_on_completion_only() {
        let (mut f, host, _id) = fused_with(4.0, 0);
        stall(&mut f.game, host);
        f.game.select_ai_training().unwrap();
        let cost = crate::storyline::training_cost(0);
        for _ in 0..(cost * 5) {
            f.game.tick();
        }
        assert_eq!(fused_state(&f.game).training.progress, cost);
        assert_eq!(fused_state(&f.game).level, 0, "no subject, no level");
        let subject = f.staff[0];
        pin_subject_at_pen(&mut f.game, subject, f.station);
        assert!(f.game.world.get::<Stats>(subject).is_some());
        f.game.tick();
        assert_eq!(fused_state(&f.game).level, 1);
        assert!(f.game.world.get::<Stats>(subject).is_none(), "spent");
    }

    #[test]
    fn unpinning_before_completion_costs_nothing() {
        let (mut f, host, _id) = fused_with(0.0, 0);
        stall(&mut f.game, host);
        f.game.select_ai_training().unwrap();
        let subject = f.staff[0];
        pin_subject_at_pen(&mut f.game, subject, f.station);
        f.game.tick();
        f.game.unpin_subject(subject).unwrap();
        f.game.tick();
        assert!(f.game.world.get::<Stats>(subject).is_some());
        assert_eq!(fused_state(&f.game).level, 0);
    }

    #[test]
    fn switching_away_from_training_keeps_its_progress() {
        let (mut f, host, id) = fused_with(0.0, 0);
        stall(&mut f.game, host);
        f.game.select_ai_training().unwrap();
        let BaseAi::Fused(fused) = &mut f.game.storyline.base_ai else {
            unreachable!()
        };
        fused.training.progress = 7;
        f.game.select_research(&id).unwrap();
        let after = fused_state(&f.game);
        assert!(!after.training.selected);
        assert_eq!(after.training.progress, 7);
        assert_eq!(
            f.game.world.resource::<ActiveResearch>().id.as_deref(),
            Some(id.as_str())
        );
    }

    #[test]
    fn completing_training_raises_the_level_and_practice_resumes() {
        let (mut f, host, _id) = fused_with(0.0, 0);
        stall(&mut f.game, host);
        f.game.select_ai_training().unwrap();
        let BaseAi::Fused(fused) = &mut f.game.storyline.base_ai else {
            unreachable!()
        };
        fused.training.progress = crate::storyline::training_cost(0);
        let subject = f.staff[0];
        pin_subject_at_pen(&mut f.game, subject, f.station);
        f.game.tick();
        let after = fused_state(&f.game);
        assert_eq!(after.level, 1);
        assert!(!after.training.selected);
        assert_eq!(after.training.progress, 0);
        assert!(
            analysis_of(&f.game, host) < crate::storyline::analysis_cap(&after),
            "the next threshold is further off"
        );
        // Nothing is selected, so the study bank is where credit lands, and
        // the AI is working again: practice accrues.
        f.game.tick();
        assert!(fused_state(&f.game).practice > 0);
    }

    #[test]
    fn the_level_up_notification_names_the_level_and_clears_the_alert() {
        let (mut f, host, _id) = fused_with(0.0, 49);
        let fused = fused_state(&f.game);
        let cap = crate::storyline::analysis_cap(&fused);
        f.game
            .world
            .get_mut::<components::Attributes>(host)
            .unwrap()
            .set(&crate::attributes::AttributeId::from("analysis"), cap - 1);
        let BaseAi::Fused(fused) = &mut f.game.storyline.base_ai else {
            unreachable!()
        };
        fused.practice = crate::storyline::practice_cost(cap - 1 - fused.analysis_at_fusion) - 1;
        f.game.tick();
        f.game.tick();
        assert_eq!(waiting_alerts(&f.game), 1);
        f.game.select_ai_training().unwrap();
        let BaseAi::Fused(fused) = &mut f.game.storyline.base_ai else {
            unreachable!()
        };
        fused.training.progress = crate::storyline::training_cost(0);
        let subject = f.staff[0];
        pin_subject_at_pen(&mut f.game, subject, f.station);
        f.game.tick();
        assert_eq!(waiting_alerts(&f.game), 0);
        let told = storyline_notifications(&mut f.game);
        let body = told.last().map(|n| n.body.clone()).unwrap_or_default();
        assert!(body.contains("level 1"), "{body}");
    }

    /// A project whose bill is a handful of one plain item and whose data
    /// cost is small, so a test can watch the bill go in and the project end.
    const BILLED_NODE: &str = r#"(
    id: "errand_billed",
    name: "Errand Billed Node",
    description: "A node with a bill.",
    cost: 4,
    materials: [("core_fragment", 6)],
)"#;
    const BILL: u32 = 6;

    /// A fused base with the billed project selected and `BILL` units on its
    /// Depot's shelf and nowhere else; the two non-host staff are idle.
    fn fused_with_a_bill(tag: &str, pace: f64) -> (Fixture, Entity, ScratchAssets) {
        let assets = assets_dir_with_extra_research(tag, "errand_billed.ron", BILLED_NODE);
        let mut f = fixture_in(&assets);
        set_researched(&mut f.game, 25);
        tick_until(&mut f.game, 400, is_fused);
        let shelves: Vec<Entity> = f
            .game
            .world
            .query_filtered::<Entity, With<Stock>>()
            .iter(&f.game.world)
            .collect();
        let mut depot = None;
        for shelf in shelves {
            let is_depot = f
                .game
                .world
                .get::<Structure>(shelf)
                .is_some_and(|s| s.kind == "depot");
            let mut stock = f.game.world.get_mut::<Stock>(shelf).unwrap();
            stock.output.remove(&ItemId::from(ids::CORE_FRAGMENT));
            if is_depot {
                depot = Some(shelf);
            }
        }
        let depot = depot.expect("the research fixture stands a Depot");
        f.game
            .world
            .get_mut::<Stock>(depot)
            .unwrap()
            .output
            .insert(ItemId::from(ids::CORE_FRAGMENT), BILL);
        f.game.select_research("errand_billed").unwrap();
        let BaseAi::Fused(fused) = &mut f.game.storyline.base_ai else {
            unreachable!()
        };
        fused.pace_at_fusion = pace;
        fused.practice = 0;
        fused.carry = 0.0;
        f.game
            .world
            .resource_mut::<ActiveResearch>()
            .progress
            .remove("errand_billed");
        (f, depot, assets)
    }

    fn delivered(game: &Game) -> u32 {
        game.world
            .resource::<ActiveResearch>()
            .delivered
            .get("errand_billed")
            .and_then(|d| d.get(&ItemId::from(ids::CORE_FRAGMENT)))
            .copied()
            .unwrap_or(0)
    }

    #[test]
    fn idle_staff_carry_the_bill_to_the_ai_station_and_the_project_completes() {
        let (mut f, depot, _assets) = fused_with_a_bill("errand_completes", 4.0);
        tick_until(&mut f.game, 1500, |g| {
            g.world.resource::<Research>().0.contains("errand_billed")
        });
        // Completion clears the delivered tally, so the shelf is the witness.
        let left = f
            .game
            .world
            .get::<Stock>(depot)
            .unwrap()
            .output
            .get(&ItemId::from(ids::CORE_FRAGMENT))
            .copied()
            .unwrap_or(0);
        assert_eq!(left, 0, "the bill left the shelf");
    }

    #[test]
    fn the_errand_credits_no_research_itself() {
        let (mut f, _depot, _assets) = fused_with_a_bill("errand_no_credit", 0.0);
        tick_until(&mut f.game, 1500, |g| delivered(g) == BILL);
        let progress = f
            .game
            .world
            .resource::<ActiveResearch>()
            .progress
            .get("errand_billed")
            .copied()
            .unwrap_or(0);
        assert_eq!(progress, 0, "carrying the bill earns no data");
    }

    #[test]
    fn no_errand_runs_while_the_arc_is_dormant() {
        let mut f = fixture();
        assert!(f.game.provision_board().is_none());
    }

    #[test]
    fn the_storyline_round_trips_in_every_stage() {
        let mut f = fixture();
        let host = program_id(&f.game, f.staff[0]);
        let fused = FusedAi {
            host,
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
                    host,
                    station: (2, 2),
                },
            ),
            (
                "fugue",
                BaseAi::Fugue {
                    host,
                    station: (2, 2),
                    ticks_left: 57,
                },
            ),
            ("fused", BaseAi::Fused(fused.clone())),
            (
                "stalled",
                BaseAi::Fused(FusedAi {
                    practice: 0,
                    carry: 0.375,
                    ..fused
                }),
            ),
        ];
        for (tag, stage) in stages {
            f.game.storyline.base_ai = stage.clone();
            let loaded = round_trip(&mut f.game, tag);
            assert_eq!(loaded.storyline().base_ai, stage, "{tag}");
            if !matches!(stage, BaseAi::Dormant { .. }) {
                let entity = loaded.program_entity(host).expect("the host loads");
                assert_eq!(
                    loaded.program_role(entity),
                    Some(ProgramRole::BaseAi),
                    "{tag}"
                );
            }
        }
    }
}
