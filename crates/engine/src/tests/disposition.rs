//! A program's hidden temperament: the two axes it moves, and what survives
//! a save.
//!
//! The unit tests for the table itself live beside it in
//! `crate::disposition`. What is asserted here is that the table actually
//! *reaches* the two seams — a multiplier nothing multiplies by is the
//! failure mode this whole file exists for, and it is invisible to a test
//! that only reads `Disposition::need_drain()`.

use super::support::*;
use crate::components::{Memories, MemorySubject, Needs, ProgramId};
use crate::disposition::Disposition;
use crate::needs::{NEED_MAX, NeedId};
use crate::*;

fn coherence() -> NeedId {
    NeedId::from("coherence")
}

fn reserve(game: &Game, who: Entity) -> f32 {
    game.world
        .get::<Needs>(who)
        .expect("a roster program carries a store")
        .get(&coherence())
        .expect("seeded on the first drain")
}

fn set(game: &mut Game, who: Entity, d: Disposition) {
    game.world.entity_mut(who).insert(d);
}

/// The drain axis, through the system rather than through the table. Remove
/// the `* temperament` in `needs_drain_system` and this is what goes red.
#[test]
fn a_languid_program_runs_down_faster_than_a_dogged_one() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let languid = spawn_tamed(&mut game, 10, 3);
    let dogged = spawn_tamed(&mut game, 10, 3);
    set(&mut game, languid, Disposition::Languid);
    set(&mut game, dogged, Disposition::Dogged);

    game.tick();

    let (fast, slow) = (reserve(&game, languid), reserve(&game, dogged));
    assert!(
        fast < slow,
        "Languid must spend more of its reserve than Dogged in the same \
         tick, got {fast} against {slow}"
    );
    assert!(fast < NEED_MAX && slow < NEED_MAX, "both still drain");
}

/// A program with no disposition at all — every fixture written before this
/// feature — must drain at exactly the authored rate. This is the property
/// that keeps `Disposition` from being a difficulty change nobody asked for.
#[test]
fn a_program_with_no_disposition_drains_at_the_authored_rate() {
    let mut game = Game::new(42, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world
        .entity_mut(worker)
        .remove::<Disposition>()
        .insert(Disposition::Steady);
    let neutral = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(neutral).remove::<Disposition>();

    game.tick();

    assert!(
        (reserve(&game, worker) - reserve(&game, neutral)).abs() < 1e-6,
        "an absent disposition must read exactly as Steady"
    );
}

/// The memory axis, through `Game::morale` rather than through the table.
/// Both programs are handed the *same* memory; only how hard it lands
/// differs.
#[test]
fn an_abrasive_program_feels_a_grudge_harder_than_an_amiable_one() {
    let mut game = Game::new(43, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let abrasive = spawn_tamed(&mut game, 10, 3);
    let amiable = spawn_tamed(&mut game, 10, 3);
    set(&mut game, abrasive, Disposition::Abrasive);
    set(&mut game, amiable, Disposition::Amiable);

    let where_ = MemorySubject::BaseTile { x: 2, y: 2 };
    game.remember(abrasive, "frayed_here", where_.clone());
    game.remember(amiable, "frayed_here", where_);

    let (sour, sunny) = (game.morale(abrasive), game.morale(amiable));
    assert!(
        sour < 0.0 && sunny < 0.0,
        "a grudge stays a grudge for both"
    );
    assert!(
        sour < sunny,
        "the same grudge must weigh more on Abrasive than on Amiable, got \
         {sour} against {sunny}"
    );
}

/// The other pole of the same axis, so a one-sided implementation that only
/// amplifies cannot pass. A fondness must land harder on `Amiable`.
#[test]
fn an_amiable_program_feels_a_fondness_harder_than_an_abrasive_one() {
    let mut game = Game::new(44, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let amiable = spawn_tamed(&mut game, 10, 3);
    let abrasive = spawn_tamed(&mut game, 10, 3);
    set(&mut game, amiable, Disposition::Amiable);
    set(&mut game, abrasive, Disposition::Abrasive);

    let kind = MemorySubject::Structure("mining_node".into());
    game.remember(amiable, "settled_in", kind.clone());
    game.remember(abrasive, "settled_in", kind);

    let (sunny, sour) = (game.morale(amiable), game.morale(abrasive));
    assert!(sunny > 0.0 && sour > 0.0, "a fondness stays a fondness");
    assert!(
        sunny > sour,
        "the same fondness must weigh more on Amiable, got {sunny} against {sour}"
    );
}

/// Every door into the roster passes through `roster_parts`, so a program
/// cannot arrive without one, and the one it arrives with is the one its id
/// derives.
///
/// Asserted against `adopt_program` and against the barrier itself rather
/// than against `spawn_tamed`, which pins `Steady` deliberately — see the
/// comment there. A fixture that pinned nothing would make every other test
/// in the suite seed-dependent.
#[test]
fn every_program_on_the_roster_is_born_with_a_disposition() {
    let mut game = Game::new(45, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let adopted = game.adopt_program("scrapper", 4, 4, 1.0).unwrap();
    let id = game.world.get::<ProgramId>(adopted).expect("an id").0;
    assert_eq!(
        game.world.get::<Disposition>(adopted).copied(),
        Some(Disposition::seed(id)),
        "an adopted program's disposition is the one its id derives"
    );

    // The barrier itself, so a door added later that assembles its own
    // component list still cannot ship a program without one.
    let (.., minted) = game.roster_parts();
    assert!(
        Disposition::ALL.contains(&minted),
        "roster_parts mints a real disposition, got {minted:?}"
    );
}

/// A field-named RON round trip cannot catch a skipped field, so this goes
/// through a **real** save and load.
#[test]
fn a_disposition_survives_a_save_and_load() {
    let dir = scratch_assets_dir("disposition_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = Game::new(46, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    // Written by hand to something the id would not have derived, so a load
    // path that re-seeds instead of reading the file fails here.
    let planted = Disposition::ALL
        .into_iter()
        .find(|d| *d != Disposition::seed(game.world.get::<ProgramId>(worker).unwrap().0))
        .expect("five variants, so one differs");
    set(&mut game, worker, planted);
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let restored: Vec<Disposition> = loaded
        .world
        .query::<&Disposition>()
        .iter(&loaded.world)
        .copied()
        .filter(|d| *d == planted)
        .collect();
    assert!(
        !restored.is_empty(),
        "the stored disposition survives the round trip rather than being re-seeded"
    );
}

/// A save written before dispositions existed carries no key. Every program
/// in it must come up with the disposition its `ProgramId` derives — not
/// `Steady`, which would leave an existing base full of neutral programs and
/// read as the feature not working.
#[test]
fn a_save_written_before_dispositions_seeds_from_the_program_id() {
    let dir = scratch_assets_dir("disposition_legacy_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = Game::new(47, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    spawn_tamed(&mut game, 10, 3);
    spawn_tamed(&mut game, 10, 3);
    game.save(&path).unwrap();
    let mut data = crate::save::load_from_file(&path).unwrap();
    for creature in &mut data.creatures {
        creature.disposition = None;
    }
    crate::save::save_to_file(&path, &data).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let pairs: Vec<(u32, Disposition)> = loaded
        .world
        .query::<(&ProgramId, &Disposition)>()
        .iter(&loaded.world)
        .map(|(id, d)| (id.0, *d))
        .collect();
    assert!(!pairs.is_empty(), "the roster survived the load");
    for (id, d) in pairs {
        assert_eq!(d, Disposition::seed(id), "program {id} seeded off its id");
    }
}

/// The documented exception: a disposition scales what a program *feels*,
/// never which memories it *keeps*. Scaling the eviction weight too would
/// make an Abrasive program drop its fondnesses first and fill its store
/// with grudges — a compounding loop on an axis that already compounds
/// through morale.
#[test]
fn eviction_does_not_read_a_disposition() {
    let cap = crate::tuning::MEMORY_CAP_PER_PROGRAM;
    let mut game = Game::new(48, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let abrasive = spawn_tamed(&mut game, 10, 3);
    let steady = spawn_tamed(&mut game, 10, 3);
    set(&mut game, abrasive, Disposition::Abrasive);
    set(&mut game, steady, Disposition::Steady);

    // Alternating poles, past the cap, so eviction has to choose between a
    // fondness and a grudge on every write after the first `cap`.
    for i in 0..(cap as i32 + 4) {
        let (def, subject) = if i % 2 == 0 {
            ("frayed_here", MemorySubject::BaseTile { x: i, y: 0 })
        } else {
            (
                "settled_in",
                MemorySubject::Structure(format!("m{i}").into()),
            )
        };
        game.remember(abrasive, def, subject.clone());
        game.remember(steady, def, subject);
    }

    let kept = |g: &Game, who: Entity| -> Vec<String> {
        g.world
            .get::<Memories>(who)
            .expect("a store")
            .0
            .iter()
            .map(|m| format!("{}:{:?}", m.def.as_str(), m.subject))
            .collect()
    };
    assert_eq!(
        kept(&game, abrasive),
        kept(&game, steady),
        "the store's contents must not depend on the disposition reading them"
    );
}

// ---------------------------------------------------------------------------
// Acting out: the morale gate, and the hysteresis it exists for.
// ---------------------------------------------------------------------------

use crate::components::{Disgruntled, Memory};
use crate::memories::MemoryId;
use crate::tuning::{MORALE_DOWNS_TOOLS_AT, MORALE_RECOVERED_AT};

/// Drives morale to `target` by stacking maxed grudges, which is the only
/// lever a test has on a figure that is otherwise a sum of decayed history.
///
/// Written straight into the store rather than through `Game::remember`,
/// which caps strikes per def and would need a dozen distinct subjects to
/// reach the threshold.
fn sour_to(game: &mut Game, who: Entity, target: f32) {
    let now = game.current_tick();
    let mut n = 0;
    while game.morale(who) > target {
        game.world
            .get_mut::<Memories>(who)
            .expect("a roster program holds a store")
            .0
            .push(Memory {
                def: MemoryId::from("frayed_here"),
                subject: MemorySubject::BaseTile { x: n, y: 900 },
                subject_name: None,
                reinforced: now,
                strikes: 3,
            });
        n += 1;
        assert!(n < 400, "morale never reached {target}");
    }
}

/// The gap between the two thresholds *is* the feature. Equal, the marker
/// flickers every tick at the boundary, which is the whole reason
/// `Disgruntled` is stored rather than derived.
#[test]
fn the_recovery_threshold_leaves_a_hysteresis_gap() {
    assert!(
        MORALE_RECOVERED_AT > MORALE_DOWNS_TOOLS_AT,
        "recovery must sit strictly above the downing-tools line, got \
         {MORALE_RECOVERED_AT} against {MORALE_DOWNS_TOOLS_AT}"
    );
}

/// A program run far enough into the hole stops taking postings.
#[test]
fn a_program_deep_in_the_hole_downs_tools() {
    let mut game = Game::new(51, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    assert!(
        game.world.get::<Disgruntled>(worker).is_none(),
        "a fresh program is not disgruntled"
    );

    sour_to(&mut game, worker, MORALE_DOWNS_TOOLS_AT);
    game.update_disgruntled(&[worker]);

    assert!(
        game.world.get::<Disgruntled>(worker).is_some(),
        "morale {} is at or past the line",
        game.morale(worker)
    );
}

/// The hysteresis, asserted where it actually bites: morale recovered to
/// **between** the two thresholds must leave the marker in place. Read off
/// one number, this is where a body picks its tools back up a tick after
/// dropping them.
#[test]
fn a_disgruntled_program_stays_disgruntled_between_the_thresholds() {
    let mut game = Game::new(52, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    sour_to(&mut game, worker, MORALE_DOWNS_TOOLS_AT);
    game.update_disgruntled(&[worker]);
    assert!(
        game.world.get::<Disgruntled>(worker).is_some(),
        "down first"
    );

    // Back above the entry line but not yet to recovery — the gap.
    game.world.get_mut::<Memories>(worker).unwrap().0.clear();
    let now = game.current_tick();
    game.world
        .get_mut::<Memories>(worker)
        .unwrap()
        .0
        .push(Memory {
            def: MemoryId::from("frayed_here"),
            subject: MemorySubject::BaseTile { x: 0, y: 900 },
            subject_name: None,
            reinforced: now,
            strikes: 2,
        });
    let between = game.morale(worker);
    assert!(
        between > MORALE_DOWNS_TOOLS_AT && between < MORALE_RECOVERED_AT,
        "the fixture must land inside the gap, got {between}"
    );

    game.update_disgruntled(&[worker]);

    assert!(
        game.world.get::<Disgruntled>(worker).is_some(),
        "inside the gap the marker is kept — this is the hysteresis"
    );
}

/// And it clears once morale is genuinely back.
#[test]
fn a_recovered_program_picks_its_tools_back_up() {
    let mut game = Game::new(53, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    sour_to(&mut game, worker, MORALE_DOWNS_TOOLS_AT);
    game.update_disgruntled(&[worker]);
    assert!(
        game.world.get::<Disgruntled>(worker).is_some(),
        "down first"
    );

    game.world.get_mut::<Memories>(worker).unwrap().0.clear();
    assert!(game.morale(worker) >= MORALE_RECOVERED_AT, "back to zero");
    game.update_disgruntled(&[worker]);

    assert!(
        game.world.get::<Disgruntled>(worker).is_none(),
        "a program whose grudges have gone goes back to work"
    );
}

/// An `Abrasive` program reaches the line on strictly less history than an
/// `Amiable` one, which is the whole of why the two features were built in
/// this order — the hidden disposition becomes visible through *when*
/// somebody breaks.
#[test]
fn an_abrasive_program_downs_tools_on_less_than_an_amiable_one() {
    let mut game = Game::new(54, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let abrasive = spawn_tamed(&mut game, 10, 3);
    let amiable = spawn_tamed(&mut game, 10, 3);
    set(&mut game, abrasive, Disposition::Abrasive);
    set(&mut game, amiable, Disposition::Amiable);

    // The *same* history on both, sized so it takes the amplifying one past
    // the line and leaves the damping one short.
    let now = game.current_tick();
    for who in [abrasive, amiable] {
        game.world.get_mut::<Memories>(who).unwrap().0.push(Memory {
            def: MemoryId::from("frayed_here"),
            subject: MemorySubject::BaseTile { x: 0, y: 900 },
            subject_name: None,
            reinforced: now,
            strikes: 3,
        });
    }
    // The window this test lives in, stated rather than assumed: one maxed
    // grudge has to straddle the line once the two dispositions have scaled
    // it. A threshold retune that closes the window fails here with the
    // figures rather than silently proving nothing.
    let (sour, sunny) = (game.morale(abrasive), game.morale(amiable));
    assert!(
        sour <= MORALE_DOWNS_TOOLS_AT && sunny > MORALE_DOWNS_TOOLS_AT,
        "the fixture must straddle the line, got {sour} and {sunny} against          {MORALE_DOWNS_TOOLS_AT}"
    );
    game.update_disgruntled(&[abrasive, amiable]);

    assert!(
        game.morale(abrasive) < game.morale(amiable),
        "the same history weighs more on Abrasive"
    );
    assert!(
        game.world.get::<Disgruntled>(abrasive).is_some(),
        "Abrasive is past the line at morale {}",
        game.morale(abrasive)
    );
    assert!(
        game.world.get::<Disgruntled>(amiable).is_none(),
        "Amiable is still short of it at morale {}",
        game.morale(amiable)
    );
}

/// The marker is the hysteresis, so it has to survive a reload — dropped, a
/// program goes back to work the moment the player looks away, at a morale
/// that has not moved.
#[test]
fn downing_tools_survives_a_save_and_load() {
    let dir = scratch_assets_dir("disgruntled_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = Game::new(55, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    sour_to(&mut game, worker, MORALE_DOWNS_TOOLS_AT);
    game.update_disgruntled(&[worker]);
    assert!(game.world.get::<Disgruntled>(worker).is_some());
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let down = loaded
        .world
        .query::<&Disgruntled>()
        .iter(&loaded.world)
        .count();
    assert_eq!(down, 1, "the marker travels with the program");
}

// ---------------------------------------------------------------------------
// The standdown itself: what downing tools actually costs the base.
// ---------------------------------------------------------------------------

use crate::components::{Carrying, Task};
use crate::game::base::work_orders::WorkOrder;
use crate::items::ids;

/// A Home, a mining node and one body, with an order standing that wants
/// that body on the node.
fn a_base_with_an_order(game: &mut Game) -> (Entity, Entity) {
    stand_in_base(game);
    place_home(game);
    give(game, &ItemId::from(ids::CORE_FRAGMENT), 200);
    let node = spawn_machine_at(game, "mining_node", 2, 0);
    let worker = spawn_tamed(game, 10, 3);
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 50))
        .unwrap();
    (node, worker)
}

/// The point of the rung: a program that has stopped caring is not handed a
/// job. `an_off_shift_program_is_not_posted`'s shape on the other meter.
#[test]
fn a_disgruntled_program_is_not_posted() {
    let mut game = Game::new(72, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (node, worker) = a_base_with_an_order(&mut game);
    game.tick();
    assert_eq!(
        game.world.get::<Task>(worker).map(|t| t.target),
        Some(node),
        "content, it takes the post"
    );

    sour_to(&mut game, worker, MORALE_DOWNS_TOOLS_AT);
    game.tick();

    assert!(
        game.world.get::<Disgruntled>(worker).is_some(),
        "the gate fired inside the tick"
    );
    assert!(
        game.world.get::<Task>(worker).is_none(),
        "and it is off the node while it will not work"
    );
}

/// **The one exception, kept**: the never-free-a-`Carrying`-holder rule. A
/// body that has stopped caring is still standing in the base holding
/// something the line is waiting on, and freeing it destroys the goods.
/// Only `Downed` overrides this.
#[test]
fn a_disgruntled_program_holding_a_load_keeps_its_post() {
    let mut game = Game::new(73, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (_, worker) = a_base_with_an_order(&mut game);
    game.tick();
    game.world.entity_mut(worker).insert(Carrying {
        item: ItemId::from(ids::CORE_FRAGMENT),
        qty: 3,
    });

    sour_to(&mut game, worker, MORALE_DOWNS_TOOLS_AT);
    game.tick();

    assert!(
        game.world.get::<Task>(worker).is_some(),
        "a loaded body keeps its post even disgruntled"
    );
    assert_eq!(
        game.world.get::<Carrying>(worker).map(|c| c.qty),
        Some(3),
        "and its load is not destroyed"
    );
}

/// The other half of downing tools, and the half the `Task` removal cannot
/// prove: a disgruntled body leaves the **pool**, so the base stops planning
/// work for hands it does not have. Without it `wanted` is still cut to a
/// count that includes the body refusing to work, and the shortfall the work
/// order header shows reads one short of the truth.
#[test]
fn labour_demand_counts_only_the_bodies_still_willing() {
    let mut game = Game::new(74, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (_, worker) = a_base_with_an_order(&mut game);
    game.tick();
    let before = game.labour_demand().staff;
    assert!(before > 0, "somebody was counted to begin with");

    sour_to(&mut game, worker, MORALE_DOWNS_TOOLS_AT);
    game.tick();

    assert_eq!(
        game.labour_demand().staff,
        before - 1,
        "the one body that has downed tools is one body the base does not have"
    );
}

/// The third thing downing tools has to do, and the one a single-body base
/// cannot show: **a disgruntled body already standing at a post is freed
/// from it**, rather than being left there because its post happened to
/// survive the truncation.
///
/// With one worker the pool filter alone looks sufficient — `wanted` is cut
/// to zero, the held post falls out of `remaining`, and the body is freed as
/// a side effect. Add a second, willing body and that stops being true: the
/// post survives the cut, the disgruntled body matches it, and without the
/// explicit free it keeps working a machine it has refused to work. The
/// willing body then gets nothing, which is a base with an idle hand and a
/// sulking one on the node.
#[test]
fn a_disgruntled_body_is_freed_so_a_willing_one_can_take_the_post() {
    let mut game = Game::new(75, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (node, first) = a_base_with_an_order(&mut game);
    let second = spawn_tamed(&mut game, 10, 3);
    game.tick();
    // Which body the scheduler picked is not something this test may assume
    // — `idle` is walked deepest-first and the order is its business. Sour
    // whichever one actually holds the post.
    let holds = |g: &Game, w: Entity| g.world.get::<Task>(w).map(|t| t.target) == Some(node);
    let (sour, willing) = if holds(&game, first) {
        (first, second)
    } else {
        assert!(holds(&game, second), "somebody took the post");
        (second, first)
    };

    sour_to(&mut game, sour, MORALE_DOWNS_TOOLS_AT);
    game.tick();

    assert!(
        game.world.get::<Task>(sour).is_none(),
        "the disgruntled body is off the node even though the post survived \
         the cut"
    );
    assert_eq!(
        game.world.get::<Task>(willing).map(|t| t.target),
        Some(node),
        "and the willing body has it instead"
    );
}
