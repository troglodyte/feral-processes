//! A visit to an amenity is one `OffShift` episode, tallied into the ledger
//! as served, waited or stranded when it ends.

use super::needs::{a_base_with_an_amenity, coherence, drift, set_reserve, threshold};
use super::support::*;
use crate::base_ledger::{BaseLedger, VisitTally};
use crate::components::{OffShift, Position};
use crate::*;

/// The bay `a_base_with_an_amenity` builds, with `n` staff just under
/// critical and each standing in reach of it.
fn crowded_bay(seed: u32, n: usize) -> (Game, Vec<Entity>) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, n);
    let bay = find_structure_by_kind(&mut game, "defrag_bay").unwrap();
    let site = *game.world.get::<Position>(bay).unwrap();
    let (critical, _) = threshold(&game, &coherence());
    let around = [(0, 1), (-1, 0), (0, -1), (1, 1), (-1, 1)];
    for (i, &who) in staff.iter().enumerate() {
        let (dx, dy) = around[i % around.len()];
        let mut pos = game.world.get_mut::<Position>(who).unwrap();
        pos.x = site.x + dx;
        pos.y = site.y + dy;
        set_reserve(&mut game, who, &coherence(), critical - 1.0);
    }
    (game, staff)
}

fn visits(game: &Game) -> VisitTally {
    let tick = game.current_tick();
    game.world
        .resource::<BaseLedger>()
        .recent_visits(&coherence(), tick)
}

/// Ticks until `n` visits have ended, or fails.
fn run_until_visits(game: &mut Game, n: u32) {
    for _ in 0..3000 {
        if visits(game).total() >= n {
            return;
        }
        game.tick();
    }
    panic!("only {:?} visits ended", visits(game));
}

#[test]
fn a_program_that_gets_a_place_is_one_served_visit() {
    let (mut game, _) = crowded_bay(80, 1);
    run_until_visits(&mut game, 1);
    let tally = visits(&game);
    assert_eq!((tally.served, tally.waited, tally.stranded), (1, 0, 0));
}

#[test]
fn a_third_program_at_a_two_place_bay_waits() {
    let (mut game, _) = crowded_bay(81, 3);
    run_until_visits(&mut game, 3);
    let tally = visits(&game);
    assert!(tally.waited >= 1, "capacity is two, got {tally:?}");
    assert_eq!(tally.stranded, 0);
}

/// The third program stands in reach of the full bay on its way to the free
/// one `Amenities::route` sent it to; it is passing through, not waiting.
#[test]
fn a_program_walking_past_a_full_bay_to_a_free_one_does_not_wait() {
    let (mut game, staff) = crowded_bay(88, 3);
    place_now(&mut game, "defrag_bay", -5, 0).expect("a second bay is buildable");
    // Placing the bay ran ticks with the staff already critical and only the
    // first bay counted; start the visits afresh once both are.
    let (critical, _) = threshold(&game, &coherence());
    for &who in &staff {
        set_reserve(&mut game, who, &coherence(), critical + 5.0);
        game.world.entity_mut(who).remove::<OffShift>();
    }
    for _ in 0..200 {
        if game.amenities().servicing(&coherence()).len() == 2 {
            break;
        }
        game.tick();
    }
    assert_eq!(game.amenities().servicing(&coherence()).len(), 2);
    for &who in &staff {
        set_reserve(&mut game, who, &coherence(), critical - 1.0);
    }
    run_until_visits(&mut game, 3);
    let tally = visits(&game);
    assert_eq!(tally.waited, 0, "a free bay existed, got {tally:?}");
}

#[test]
fn an_unreachable_amenity_is_a_stranded_visit() {
    let (mut game, staff) = crowded_bay(82, 1);
    {
        let mut pos = game.world.get_mut::<Position>(staff[0]).unwrap();
        pos.x = 900;
        pos.y = 900;
    }
    game.world.entity_mut(staff[0]).insert(OffShift {
        need: coherence(),
        waited: false,
    });
    drift(&mut game, &staff);
    let tally = visits(&game);
    assert_eq!((tally.served, tally.waited, tally.stranded), (0, 0, 1));
}

/// Re-inserting `OffShift` for the same need would reset the flag, and the
/// visit would end as served.
#[test]
fn a_repeated_beat_for_the_same_need_keeps_the_waited_flag() {
    let (mut game, staff) = crowded_bay(83, 1);
    game.world.entity_mut(staff[0]).insert(OffShift {
        need: coherence(),
        waited: true,
    });
    for _ in 0..3 {
        let amenities = game.amenities();
        game.update_off_shift(&staff, &amenities);
    }
    assert!(game.world.get::<OffShift>(staff[0]).unwrap().waited);
}

// ---------------------------------------------------------------------------
// The alert: posted on the first blocked visit, latched until the window
// holds no blocked visit for that need.
// ---------------------------------------------------------------------------

mod alert {
    use super::*;
    use crate::alerts::AlertKind;
    use crate::base_ledger::{BUCKET_TICKS, VisitOutcome};

    fn bay_site(game: &mut Game) -> Position {
        let bay = find_structure_by_kind(game, "defrag_bay").unwrap();
        *game.world.get::<Position>(bay).unwrap()
    }

    fn blocked_rows(game: &Game) -> Vec<(u32, String)> {
        game.alerts()
            .into_iter()
            .filter(|a| a.kind == AlertKind::AmenityBlocked)
            .map(|a| (a.count, a.text))
            .collect()
    }

    #[test]
    fn the_first_blocked_visit_posts_naming_the_amenity() {
        let (mut game, _) = crowded_bay(84, 1);
        let site = bay_site(&mut game);

        game.note_amenity_visit(&coherence(), VisitOutcome::Waited, Some(site));
        let rows = blocked_rows(&game);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].1,
            "Staff waited for a place at the Log Analyzer Bay"
        );

        let (mut game, _) = crowded_bay(85, 1);
        let site = bay_site(&mut game);
        game.note_amenity_visit(&coherence(), VisitOutcome::Stranded, Some(site));
        assert_eq!(
            blocked_rows(&game)[0].1,
            "Staff can't reach the Log Analyzer Bay"
        );
    }

    #[test]
    fn a_second_block_while_latched_adds_nothing() {
        let (mut game, _) = crowded_bay(86, 1);
        let site = bay_site(&mut game);
        game.note_amenity_visit(&coherence(), VisitOutcome::Waited, Some(site));
        game.note_amenity_visit(&coherence(), VisitOutcome::Waited, Some(site));
        game.note_amenity_visit(&coherence(), VisitOutcome::Stranded, Some(site));

        let rows = blocked_rows(&game);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, 1, "no repeat post while latched");
    }

    #[test]
    fn a_clean_visit_after_the_window_empties_clears_the_latch() {
        let (mut game, _) = crowded_bay(87, 1);
        let site = bay_site(&mut game);
        game.note_amenity_visit(&coherence(), VisitOutcome::Waited, Some(site));

        // A clean visit while the block is still inside the window leaves
        // the latch set.
        game.note_amenity_visit(&coherence(), VisitOutcome::Served, Some(site));
        game.note_amenity_visit(&coherence(), VisitOutcome::Waited, Some(site));
        assert_eq!(blocked_rows(&game)[0].0, 1);

        let tick = game.current_tick();
        game.world.resource_mut::<GameClock>().tick = tick + BUCKET_TICKS * 3;
        game.note_amenity_visit(&coherence(), VisitOutcome::Served, Some(site));
        game.note_amenity_visit(&coherence(), VisitOutcome::Waited, Some(site));
        assert_eq!(blocked_rows(&game)[0].0, 2, "posted again after the clear");
    }
}
