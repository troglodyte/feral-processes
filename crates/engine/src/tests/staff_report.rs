//! The Staff page's reports: wellbeing, coverage, discontent and the flag
//! that is built from them.

use super::needs::{a_base_with_an_amenity, coherence, set_reserve, threshold};
use super::support::*;
use crate::components::{Disgruntled, Grievance, MemorySubject, Position, ProgramId};
use crate::views::StaffActivity;
use crate::*;

fn game_with_staff(seed: u32, n: usize) -> (Game, Vec<Entity>) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, n);
    place_now(&mut game, "sandbox", 2, 4).expect("a Sandbox beside the bay");
    // Hot-start every reserve so nothing is critical until a test says so.
    for &who in &staff {
        set_reserve(&mut game, who, &coherence(), 90.0);
    }
    (game, staff)
}

fn sulk(game: &mut Game, who: Entity, grievance: Grievance) {
    game.world.entity_mut(who).insert(Disgruntled {
        grievance,
        stranded: false,
        told: false,
    });
}

fn grudge_against(game: &mut Game, who: Entity, about: Entity, def: &str) {
    let id = *game.world.get::<ProgramId>(about).unwrap();
    game.remember(who, def, MemorySubject::Program(id));
}

#[test]
fn wellbeing_lists_worst_morale_first_then_the_emptiest_need() {
    let (mut game, staff) = game_with_staff(90, 4);
    // staff[3] holds a grudge; staff[1] is emptier than staff[2].
    grudge_against(&mut game, staff[3], staff[0], "turned_on_me");
    set_reserve(&mut game, staff[1], &coherence(), 40.0);
    set_reserve(&mut game, staff[2], &coherence(), 70.0);

    let order: Vec<Entity> = game.staff_wellbeing().iter().map(|r| r.entity).collect();

    assert_eq!(order[0], staff[3], "worst morale first");
    assert_eq!(order[1], staff[1], "then the emptiest need among equals");
    assert_eq!(order[2], staff[2]);
    assert_eq!(order[3], staff[0]);
}

#[test]
fn wellbeing_gap_case_a_program_below_critical_is_flagged() {
    let (mut game, staff) = game_with_staff(91, 2);
    assert_eq!(game.base_report_flags().staff, None, "a healthy staff");
    let (critical, _) = threshold(&game, &coherence());
    set_reserve(&mut game, staff[0], &coherence(), critical - 1.0);

    let rows = game.staff_wellbeing();

    let row = rows.iter().find(|r| r.entity == staff[0]).unwrap();
    assert!(row.is_critical());
    assert!(game.base_report_flags().staff.is_some());
}

#[test]
fn an_off_shift_program_in_reach_of_a_full_bay_is_waiting() {
    let (mut game, staff) = game_with_staff(92, 3);
    let bay = find_structure_by_kind(&mut game, "defrag_bay").unwrap();
    let site = *game.world.get::<Position>(bay).unwrap();
    let (critical, _) = threshold(&game, &coherence());
    let around = [(0, 1), (-1, 0), (0, -1)];
    for (&who, (dx, dy)) in staff.iter().zip(around) {
        let mut pos = game.world.get_mut::<Position>(who).unwrap();
        (pos.x, pos.y) = (site.x + dx, site.y + dy);
        set_reserve(&mut game, who, &coherence(), critical - 1.0);
    }
    game.tick();
    for &who in &staff {
        game.world
            .entity_mut(who)
            .insert(crate::components::OffShift { need: coherence() });
    }

    let waiting = game
        .staff_wellbeing()
        .into_iter()
        .filter(|r| matches!(r.activity, StaffActivity::Waiting))
        .count();

    assert_eq!(waiting, 1, "two hold places, one waits");
}

#[test]
fn coverage_gap_case_no_amenity_built_is_short() {
    let mut game = Game::new(93, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    spawn_tamed(&mut game, 10, 3);

    let rows = game.need_coverage();

    let row = rows.iter().find(|r| r.name == "Coherence").unwrap();
    assert_eq!((row.built, row.places), (0, 0));
    assert!(row.needed >= 1 && row.is_short());
    assert!(game.base_report_flags().staff.is_some());
}

#[test]
fn coverage_counts_a_built_bay_and_the_share_comes_from_the_pure_formula() {
    let (game, staff) = game_with_staff(94, 3);

    let rows = game.need_coverage();

    let row = rows.iter().find(|r| r.name == "Coherence").unwrap();
    assert_eq!((row.built, row.places, row.capacity), (1, 2, 2));
    assert_eq!(row.needed, 1, "{} staff need one bay", staff.len());
    let def = game
        .world
        .resource::<crate::needs::NeedDb>()
        .get(&coherence())
        .unwrap()
        .clone();
    let expected = crate::needs::off_shift_share(&def, 0.6, 0.5);
    assert!((row.lost_share - expected).abs() < 1e-6);
    assert_eq!(row.below_critical, 0);
}

#[test]
fn coverage_lists_a_boxed_in_bay_and_counts_those_below_critical() {
    let (mut game, staff) = game_with_staff(95, 2);
    let bay = find_structure_by_kind(&mut game, "defrag_bay").unwrap();
    let site = *game.world.get::<Position>(bay).unwrap();
    let r = game.amenities().servicing(&coherence())[0].2;
    let mut kept = false;
    for dy in -r..=r {
        for dx in -r..=r {
            let (x, y) = (site.x + dx, site.y + dy);
            if (dx, dy) == (0, 0) || !game.world.resource::<base_grid::BaseGrid>().walkable(x, y) {
                continue;
            }
            if kept {
                spawn_structure_at(&mut game, "depot", x, y);
            }
            kept = true;
        }
    }
    let (critical, _) = threshold(&game, &coherence());
    set_reserve(&mut game, staff[0], &coherence(), critical - 1.0);

    let rows = game.need_coverage();

    let row = rows.iter().find(|r| r.name == "Coherence").unwrap();
    assert_eq!(row.places, 1);
    assert_eq!(row.boxed_in.len(), 1);
    assert_eq!(row.boxed_in[0].open_tiles, 1);
    assert_eq!(row.below_critical, 1);
}

#[test]
fn discontent_lists_only_the_aggrieved_worst_rung_first() {
    let (mut game, staff) = game_with_staff(96, 3);
    for def in ["turned_on_me", "mauled_by", "frayed_here", "lost_in_battle"] {
        grudge_against(&mut game, staff[1], staff[0], def);
    }
    sulk(&mut game, staff[1], Grievance::Sulking);
    sulk(&mut game, staff[2], Grievance::DownedTools);

    let rows = game.discontent();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].entity, staff[2]);
    assert!(rows[0].stopped_work);
    assert_eq!(rows[1].entity, staff[1]);
    assert!(!rows[1].stopped_work);
    assert!(rows[1].grudges.len() <= 3 && !rows[1].grudges.is_empty());
    assert!(rows[1].grudges.iter().all(|g| g.intensity < 0.0));
    assert!(
        rows[1]
            .grudges
            .windows(2)
            .all(|w| w[0].intensity.abs() >= w[1].intensity.abs())
    );
}

#[test]
fn discontent_gap_case_a_sulker_alone_does_not_flag_but_downed_tools_does() {
    let (mut game, staff) = game_with_staff(97, 2);
    sulk(&mut game, staff[0], Grievance::Sulking);
    assert_eq!(game.base_report_flags().staff, None);

    sulk(&mut game, staff[0], Grievance::DownedTools);

    assert!(game.base_report_flags().staff.is_some());
}
