//! What a level-up announces. The three sites that can grant one — the
//! player, a party member and a posted worker — each write a header line
//! and a block of stat lines under it, and they have to agree on the shape
//! since `progression::stat_block` is the one thing that renders it.

use super::support::*;
use crate::balance_sim::{
    best_gear_stats, median_ordinary_species, min_level_to_clear_zone, toughest_ordinary_species,
};
use crate::components::{Decompiler, Experience, Stats};
use crate::progression::{StatRow, stat_block};
use crate::resources::{CONDENSE_LOOKBACK, LogLine, MessageSource, condense};
use crate::species::SpeciesDb;
use crate::stack::Dir;
use crate::tuning::{
    BASE_PET_CAPACITY, DECOMPILER_SKILL_PER_LEVEL, KERNEL_RING_MAX, LEVELS_PER_RING,
    PERK_POINTS_PER_LEVEL, TALENT_START_LEVEL, ZONE_LEVEL_CAP_FLOOR, arena_level_ceiling,
};
use crate::*;

/// The indented stat lines out of a log, in order. Reaching for the two
/// leading spaces rather than for a stat name keeps the assertion about
/// "this is a block under a header" rather than about any one stat.
fn stat_lines(game: &Game) -> Vec<String> {
    game.message_log(MESSAGE_LOG_CAP)
        .into_iter()
        .filter(|e| e.text.starts_with("  "))
        .map(|e| e.text)
        .collect()
}

#[test]
fn a_player_level_up_lists_what_each_stat_grew_to() {
    let mut game = Game::new(39, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Experience>(player).unwrap().xp_to_next = 5;
    let before = *game.world.get::<Stats>(player).unwrap();

    game.award_player_xp(player, 5);

    let after = *game.world.get::<Stats>(player).unwrap();
    assert_eq!(
        stat_lines(&game),
        vec![
            format!("  Max HP {} → {}", before.max_hp, after.max_hp),
            format!("  ATK {} → {}", before.atk, after.atk),
            // No mitigation row: levelling never raises it, so there is
            // nothing to draw — see `components::Stats::mitigation`.
            format!("  Perk Points 0 → {PERK_POINTS_PER_LEVEL}"),
            format!(
                "  Decompiler {} → {}",
                game.world.get::<Decompiler>(player).unwrap().skill - DECOMPILER_SKILL_PER_LEVEL,
                game.world.get::<Decompiler>(player).unwrap().skill
            ),
        ],
        "the player's block also reports the Perk Point and Decompiler skill a level pays"
    );
}

#[test]
fn every_line_of_a_level_up_block_is_tagged_level_up() {
    let mut game = Game::new(39, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Experience>(player).unwrap().xp_to_next = 5;

    game.award_player_xp(player, 5);

    // `retain_outcomes_since_battle` keeps only Outcome/Loot/LevelUp/Raid, so
    // a stat line tagged anything else would be dropped on the way out of a
    // fight and the block would arrive on the map with its header alone.
    let block: Vec<_> = game
        .message_log(MESSAGE_LOG_CAP)
        .into_iter()
        .filter(|e| e.text.starts_with("  "))
        .collect();
    // Without this the filter below is satisfied by a log with no block in it
    // at all, and the test would keep passing with the feature torn out.
    assert!(!block.is_empty(), "there should be a block to check");
    let untagged: Vec<_> = block
        .iter()
        .filter(|e| e.kind != MessageKind::LevelUp)
        .collect();
    assert!(
        untagged.is_empty(),
        "every stat line has to survive the battle prune: {untagged:?}"
    );
}

#[test]
fn a_companion_level_up_lists_its_stat_growth() {
    let mut game = Game::new(38, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, companion);
    game.world
        .get_mut::<Experience>(companion)
        .unwrap()
        .xp_to_next = 1;
    let before = *game.world.get::<Stats>(companion).unwrap();

    // The party earns half, so this has to clear 1 XP after halving.
    game.award_player_xp(player, 4);

    let after = *game.world.get::<Stats>(companion).unwrap();
    let name = game.creature_label(companion);
    let log = game.message_log(MESSAGE_LOG_CAP);
    let header = log
        .iter()
        .position(|e| e.text.starts_with(&format!("{name} gains")))
        .expect("the companion should have announced a level-up");
    assert_eq!(
        log[header + 1].text,
        format!("  Max HP {} → {}", before.max_hp, after.max_hp),
        "the block has to follow its own header: {log:?}"
    );
    assert!(
        !log.iter().any(|e| e.text.contains("Perk Points")),
        "Perk Points are the player's alone: {log:?}"
    );
}

#[test]
fn a_posted_worker_levels_up_in_the_base_log_beside_its_machine() {
    let mut game = Game::new(4210, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    let worker = spawn_tamed(&mut game, 10, 3);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    game.world.get_mut::<Experience>(worker).unwrap().xp_to_next = 1;

    for _ in 0..60 {
        game.wait();
    }

    let log = game.message_log(MESSAGE_LOG_CAP);
    let header = log
        .iter()
        .find(|e| e.text.contains("levels up to"))
        .unwrap_or_else(|| panic!("the worker should have levelled at the node: {log:?}"));
    assert_eq!(header.source, MessageSource::Base, "base news, not field");
    assert_eq!(header.kind, MessageKind::LevelUp);
    assert!(
        header.text.starts_with("Your subroutine at the "),
        "the base log names a worker by the machine it is posted to: {:?}",
        header.text
    );
    assert!(
        !log.iter()
            .any(|e| e.text.starts_with("Your subroutine extracted") && e.text.contains("levels")),
        "the level-up is its own line now, not a tail on the payout: {log:?}"
    );
    assert!(
        stat_lines(&game).iter().any(|l| l.starts_with("  Max HP ")),
        "the base block reports growth like the two field ones: {log:?}"
    );
}

/// The reason the stat lines carry `before → after` rather than a bare
/// `+1 DEF`. `condense` folds an identical `(kind, source, text)` into the
/// nearest match within `CONDENSE_LOOKBACK`, so two entities gaining the same
/// amount of the same stat in one fight would collapse into one history row
/// under whichever announced first — deleting the other's line outright.
#[test]
fn two_entities_gaining_the_same_stat_stay_two_history_rows() {
    let level_up = |text: &str| LogLine {
        kind: MessageKind::LevelUp,
        source: MessageSource::Field,
        text: text.to_string(),
        outcome: None,
    };
    // Built through the shipping formatter, not from hand-written strings: a
    // format that went back to a bare "+1 DEF" has to fail here, and it only
    // can if these lines are the ones the game would really push. Both
    // entities gain exactly one point of DEF — the collision case.
    let player = stat_block(&[
        StatRow::new("Max HP", 108, 120),
        StatRow::new("DEF", 11, 12),
    ]);
    let companion = stat_block(&[StatRow::new("Max HP", 46, 57), StatRow::new("DEF", 8, 9)]);
    let mut lines = vec![level_up("You gain 40 XP and reach level 5!")];
    lines.extend(player.iter().map(|l| level_up(l)));
    lines.push(level_up("Scrapper gains 20 XP and levels up to 3!"));
    lines.extend(companion.iter().map(|l| level_up(l)));
    // Both DEF lines sit inside the lookback window of each other, which is
    // what a bare "+1 DEF" pair would fold across.
    assert!(lines.len() <= CONDENSE_LOOKBACK * 2);

    let entries = condense(&lines);
    assert_eq!(
        entries.len(),
        6,
        "each entity's block must stay its own: {entries:?}"
    );
    assert!(
        entries.iter().all(|e| e.repeats == 1),
        "nothing here is a repeat: {entries:?}"
    );
}

/// A level's threshold is `xp_for_level(level)` and always has been, so the
/// copy in the save is redundant — and a save written under a different
/// `XP_PER_LEVEL_STEP` carries one that disagrees. Deriving it on load is
/// what stops such a save handing out a cheap level, at no
/// `SAVE_FORMAT_VERSION` cost.
///
/// Written by corrupting the field rather than by checking in an old save,
/// because the property is "the file's copy is not trusted", which any
/// disagreeing value demonstrates.
#[test]
fn a_saves_stale_xp_threshold_is_rederived_from_its_level_on_load() {
    let mut game = Game::new(77, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, companion);
    for e in [player, companion] {
        let mut exp = game.world.get_mut::<Experience>(e).unwrap();
        exp.level = 4;
        exp.xp_to_next = 7; // a threshold no level has ever had
    }

    let path = std::env::temp_dir().join(format!(
        "feral_processes_xp_threshold_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let expected = crate::progression::xp_for_level(4);
    let reloaded_player = loaded.player_entity();
    assert_eq!(
        loaded
            .world
            .get::<Experience>(reloaded_player)
            .unwrap()
            .xp_to_next,
        expected,
        "the player's threshold comes from their level, not from the file"
    );
    let companion_threshold = loaded
        .world
        .iter_entities()
        .filter(|e| e.contains::<Tamed>())
        .filter_map(|e| e.get::<Experience>())
        .map(|e| e.xp_to_next)
        .next()
        .expect("the companion came back");
    assert_eq!(
        companion_threshold, expected,
        "and so does a companion's — both load paths, or one of them keeps the stale value"
    );
}

/// Enough XP to blow through every level a companion could possibly reach,
/// so a test asserting where it stopped is asserting about the *cap* rather
/// than about how much it was fed.
fn xp_past_every_cap() -> u32 {
    (1..=arena_level_ceiling() + 4)
        .map(crate::progression::xp_for_level)
        .sum::<u32>()
        * 2
}

fn a_party_member(game: &mut Game) -> Entity {
    let companion = spawn_tamed(game, 10, 3);
    game.world.resource_mut::<Party>().0.push(companion);
    companion
}

/// A companion's ceiling is the zone's, and a Kernel Ring is not part of the
/// answer any more — this and the two tests below are the three that used to
/// say the opposite.
#[test]
fn a_companion_with_no_ring_stops_at_the_zone_cap() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(ZoneLevel(3));
    let cap = game.level_cap();
    let companion = a_party_member(&mut game);

    game.award_party_xp(xp_past_every_cap());

    assert_eq!(
        game.world.get::<Experience>(companion).unwrap().level,
        cap,
        "a companion with no Kernel Ring stops where everyone stops"
    );
}

/// **A ring buys no levels.** It used to buy `LEVELS_PER_RING` of ceiling;
/// the zone is the ceiling now, and what a ring buys is the right to spend
/// levels already earned on a talent tree.
#[test]
fn a_kernel_ring_does_not_lift_the_level_ceiling() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(ZoneLevel(3));
    let cap = game.level_cap();
    let ringed = a_party_member(&mut game);
    game.world.entity_mut(ringed).insert(KernelRing(1));
    let plain = a_party_member(&mut game);

    game.award_party_xp(xp_past_every_cap());

    assert_eq!(
        game.world.get::<Experience>(ringed).unwrap().level,
        cap,
        "a ring must not carry a companion past the zone's cap"
    );
    assert_eq!(
        game.world.get::<Experience>(plain).unwrap().level,
        game.world.get::<Experience>(ringed).unwrap().level,
        "and a ringed companion and a plain one now stop at the same level"
    );
}

#[test]
fn every_ring_open_still_stops_at_the_zone_cap() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(ZoneLevel(2));
    let cap = game.level_cap();
    let companion = a_party_member(&mut game);
    game.world
        .entity_mut(companion)
        .insert(KernelRing(KERNEL_RING_MAX));

    game.award_party_xp(xp_past_every_cap());

    assert_eq!(
        game.world.get::<Experience>(companion).unwrap().level,
        cap,
        "three rings buy no more ceiling than none does"
    );
}

/// The whole of how a ring stays inside "progression is earned by fighting":
/// `systems.rs`'s cronjob payout keeps passing the base cap, and its own
/// `WORK_XP_LEVEL_CAP` guard stops a posted worker well below even that. A
/// developed program cannot be ground up at a Mining Node.
#[test]
fn a_ringed_cronjob_worker_still_stops_at_the_work_cap() {
    let mut game = Game::new(301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world
        .entity_mut(worker)
        .insert(KernelRing(KERNEL_RING_MAX));
    game.world.get_mut::<Experience>(worker).unwrap().level = crate::tuning::WORK_XP_LEVEL_CAP;
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 3, y: 4 },
            ResourceNode {
                resource: ItemId::from(ids::CORE_FRAGMENT),
                level: None,
            },
            work_node_parts(),
        ))
        .id();
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 0,
        required: 1,
    });

    for _ in 0..3 {
        game.tick();
    }

    let exp = game.world.get::<Experience>(worker).unwrap();
    assert_eq!(
        exp.level,
        crate::tuning::WORK_XP_LEVEL_CAP,
        "a ring must not open the cronjob grind"
    );
    assert_eq!(exp.xp, 0, "a capped worker earns no work XP at all");
}

/// A **save → load → assert**, not a RON round trip: a round trip cannot tell
/// a field that fails to travel from one that does, which is exactly what
/// `#[serde(skip)]` looks like from its side.
///
/// It used to assert the ceiling the ring bought was still lifted on the
/// loaded game. A ring buys no ceiling now, so what is left to hold is that
/// the count itself travels — it is what `Game::talent_points` reads.
#[test]
fn a_kernel_ring_survives_a_save() {
    let dir = scratch_assets_dir("ring_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");

    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = a_party_member(&mut game);
    game.world.entity_mut(companion).insert(KernelRing(2));
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let restored = loaded.world.resource::<Party>().0[0];
    assert_eq!(
        loaded.world.get::<KernelRing>(restored).map(|r| r.0),
        Some(2),
        "the ring count must travel"
    );

    loaded.award_party_xp(xp_past_every_cap());
    assert_eq!(
        loaded.world.get::<Experience>(restored).unwrap().level,
        loaded.level_cap(),
        "and the loaded companion stops at the zone's cap like everyone else"
    );
}

/// A percentage that grows per level approaches immunity, so levelling buys
/// HP, attack, accuracy and evasion — never mitigation. Delete the fix (put
/// a per-level growth term back on the `mitigation` field) and this fails.
#[test]
fn levelling_never_raises_mitigation() {
    use crate::components::Stats;
    use crate::progression::stats_after_levels;
    let base = Stats {
        hp: 90,
        max_hp: 90,
        atk: 6,
        mitigation: 12,
    };
    for levels in [1, 5, 20, 60] {
        let grown = stats_after_levels(base, levels, 1.5);
        assert_eq!(
            grown.mitigation, base.mitigation,
            "mitigation moved at {levels} levels"
        );
        assert!(grown.max_hp > base.max_hp);
        assert!(grown.atk > base.atk);
    }
}

/// The band the cap is fitted into, and the one number in these tests that
/// is not a call.
///
/// A linear cap cannot sit strictly under the gear-free requirement at every
/// zone: the two clear curves both pass near the origin and then diverge
/// (gear-free climbs about half again as fast), so a slope steep enough to
/// keep zone 16 clearable necessarily overshoots the gear-free requirement in
/// the low zones. This is the largest overshoot the fitted constants actually
/// produce, measured rather than chosen — see
/// `docs/measurements/2026-08-27-zone-level-cap.md`. Tightening the fit
/// lowers it; raising it to make a failing fit pass is writing the test to
/// agree with the code.
const GRIND_TOLERANCE_LEVELS: u32 = 6;

/// The shipped species db, for the tests that measure against the real
/// clear curves rather than against a fixture.
fn shipped_species_db() -> crate::species::SpeciesDb {
    let dir = test_assets_dir();
    let (abilities, _) = crate::abilities::AbilityDb::load_dir(&dir.join("abilities")).unwrap();
    SpeciesDb::load_dir(&dir.join("species"), &abilities)
        .unwrap()
        .0
}

fn cap_at_zone(zone: u32) -> u32 {
    let mut game = Game::new(39, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(ZoneLevel(zone));
    game.level_cap()
}

/// The cap is linear in the zone, which is the property every difficulty
/// curve in this game holds and the one `ZoneLevel::stat_multiplier`'s doc
/// comment argues at length: a cap that compounds races the enemy curve in
/// the player's favour, and a race between two curves of different order has
/// an end wherever the coefficients are put.
///
/// Swept above the floor, since the floor is deliberately flat.
#[test]
fn the_zone_level_cap_rises_linearly() {
    let caps: Vec<u32> = (2..=16).map(cap_at_zone).collect();
    let steps: Vec<u32> = caps.windows(2).map(|w| w[1] - w[0]).collect();
    let first = steps[0];
    assert!(
        steps.iter().all(|&s| s == first),
        "the cap's per-zone step must be constant: caps {caps:?}, steps {steps:?}"
    );
    assert!(
        first > 0,
        "a flat cap above the floor never lifts on a breach"
    );
}

/// Zone 1's cap is the floor, and the floor is above what zone 1 asks for —
/// a cap that bites in the opening zone is a cap the player meets before
/// they have met the game.
#[test]
fn zone_one_is_capped_at_the_floor() {
    assert_eq!(cap_at_zone(1), ZONE_LEVEL_CAP_FLOOR);
    let db = shipped_species_db();
    let (weapon, armor) = best_gear_stats();
    let (needed, _) = min_level_to_clear_zone(
        toughest_ordinary_species(&db),
        median_ordinary_species(&db),
        1,
        200,
        BASE_PET_CAPACITY,
        false,
        (weapon, armor),
    )
    .expect("zone 1 is clearable");
    assert!(
        cap_at_zone(1) > needed,
        "zone 1 needs level {needed} and caps at {} — the opening zone must \
         leave room above what it asks for",
        cap_at_zone(1)
    );
}

/// **The bound this feature lives or dies on.** Every figure here is a call
/// into `balance_sim`; none is transcribed, because a number copied out of a
/// doc comment is how this repo has been bitten four times.
///
/// Two halves, and they are not symmetrical:
///
/// - The cap must be **at or above the geared requirement** at every zone.
///   Below it, a fully-equipped party cannot clear the zone at any level it
///   is allowed to reach, which is not difficulty — it is a dead run. No
///   tolerance.
/// - The cap should sit **under the gear-free requirement**, so a zone
///   cannot be cleared by levelling alone. That is the design goal rather
///   than a correctness bound, and `GRIND_TOLERANCE_LEVELS` is how far the
///   fitted line misses it in the low zones.
#[test]
fn the_zone_level_cap_is_bounded_by_both_clear_curves() {
    let db = shipped_species_db();
    let (toughest, party) = (toughest_ordinary_species(&db), median_ordinary_species(&db));
    let (weapon, armor) = best_gear_stats();
    let required = |zone: u32, with_gear: bool| {
        min_level_to_clear_zone(
            toughest,
            party,
            zone,
            400,
            BASE_PET_CAPACITY,
            with_gear,
            (weapon, armor),
        )
        .map(|(level, _)| level)
    };

    for zone in 1..=16 {
        let cap = cap_at_zone(zone);
        let geared = required(zone, true).unwrap_or_else(|| {
            panic!(
                "zone {zone} is not clearable at all, geared — that is a sim fault, not a cap one"
            )
        });
        assert!(
            cap >= geared,
            "zone {zone} needs level {geared} fully geared but caps at {cap} — \
             a cap under the geared requirement is a run that cannot continue"
        );
        if let Some(gear_free) = required(zone, false) {
            assert!(
                cap <= gear_free + GRIND_TOLERANCE_LEVELS,
                "zone {zone} caps at {cap} against a gear-free requirement of \
                 {gear_free} — the cap is meant to sit under that, so gear and \
                 not grinding is what opens a zone"
            );
        }
    }
}

/// The cap reads the zone and nothing else. Structural today — the formula
/// names only `ZoneLevel` — but it is a stated design property, and the
/// thing that would quietly break it is a depth term added to "help" a party
/// four frames down. A deep stack is harder because the programs in it are
/// scaled, not because the party is allowed to outgrow it.
#[test]
fn depth_does_not_lift_the_zone_level_cap() {
    let mut game = Game::new(39, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(ZoneLevel(4));
    let on_the_surface = game.level_cap();
    game.world.insert_resource(Locale::Stack {
        depth: 4,
        frames: 6,
        x: 0,
        y: 0,
        facing: Dir::North,
        entrance: (0, 0),
    });
    assert_eq!(
        game.level_cap(),
        on_the_surface,
        "the cap must not move with depth"
    );
}

/// The player is capped at all now, which they never were before, and the
/// number is the zone's.
#[test]
fn the_player_stops_levelling_at_the_zone_cap() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(ZoneLevel(2));
    let cap = game.level_cap();
    let player = game.player_entity();

    game.award_player_xp(player, xp_past_every_cap());

    assert_eq!(
        game.world.get::<Experience>(player).unwrap().level,
        cap,
        "the player must stop at the zone cap"
    );
}

/// **One ceiling over the whole party.** A companion used to stop six levels
/// under the player and buy the difference back a Kernel Ring at a time;
/// now the two numbers are the same number, which is what makes developing
/// a companion worth the XP.
#[test]
fn a_companion_stops_at_the_same_level_as_the_player() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(ZoneLevel(3));
    let cap = game.level_cap();
    let player = game.player_entity();
    let companion = a_party_member(&mut game);

    game.award_player_xp(player, xp_past_every_cap());
    game.award_party_xp(xp_past_every_cap());

    let player_level = game.world.get::<Experience>(player).unwrap().level;
    let companion_level = game.world.get::<Experience>(companion).unwrap().level;
    assert_eq!(player_level, cap);
    assert_eq!(
        companion_level, player_level,
        "player and companion share one ceiling"
    );
}

/// The cap is what a breach is *for*: the zone is the dial.
#[test]
fn a_breach_lifts_the_cap_for_both() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(ZoneLevel(2));
    let before = game.level_cap();
    let player = game.player_entity();
    let companion = a_party_member(&mut game);
    game.award_player_xp(player, xp_past_every_cap());
    game.award_party_xp(xp_past_every_cap());

    game.world.insert_resource(ZoneLevel(3));
    let after = game.level_cap();
    assert!(after > before, "a breach must lift the cap");

    game.award_player_xp(player, xp_past_every_cap());
    game.award_party_xp(xp_past_every_cap());
    assert_eq!(game.world.get::<Experience>(player).unwrap().level, after);
    assert_eq!(
        game.world.get::<Experience>(companion).unwrap().level,
        after
    );
}

/// **Growth already spent is never clawed back**, `EquippedItem::fusion_tier`'s
/// rule: a receipt for something already paid out must not be re-read as a
/// live ceiling. Reachable by any save developed in a deeper zone than the
/// one it is loaded in, and by every hand-edited save.
///
/// The subject is the player, because `arena::set_level` clamps a *creature*
/// at `arena_level_ceiling()` and so cannot build the fixture — which is
/// itself the second rename doing its job.
#[test]
fn an_entity_above_the_cap_keeps_its_level_and_stats() {
    let dir = scratch_assets_dir("over_cap_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");

    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(ZoneLevel(5));
    let player = game.player_entity();
    set_level(&mut game, player, 30);
    let developed = *game.world.get::<Stats>(player).unwrap();
    game.world.insert_resource(ZoneLevel(1));
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let restored = loaded.player_entity();
    assert!(
        loaded.world.get::<Experience>(restored).unwrap().level > loaded.level_cap(),
        "the fixture must actually stand above the cap or this proves nothing"
    );
    assert_eq!(
        loaded.world.get::<Experience>(restored).unwrap().level,
        30,
        "a level already earned is not taken back by a lower cap"
    );
    let kept = *loaded.world.get::<Stats>(restored).unwrap();
    assert_eq!(
        (kept.max_hp, kept.atk, kept.mitigation),
        (developed.max_hp, developed.atk, developed.mitigation),
        "nor are the stats it bought"
    );

    // And it simply earns nothing further until the cap catches up.
    loaded.award_player_xp(restored, xp_past_every_cap());
    assert_eq!(loaded.world.get::<Experience>(restored).unwrap().level, 30);
}

/// The five shipped `dev-arenas/` scenarios author `level: 12`, and
/// `arena::set_level` must keep staging exactly that. Pointed at the zone
/// cap instead they would silently clamp — a failure this repo has already
/// had once, where old reports stopped being comparable and nothing said so.
#[test]
fn an_arena_scenario_still_stages_a_level_twelve_companion() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(ZoneLevel(1));
    assert!(
        game.level_cap() < 12,
        "the fixture needs a zone whose cap is under 12, or it proves nothing"
    );
    let companion = a_party_member(&mut game);

    set_level(&mut game, companion, 12);

    assert_eq!(
        game.world.get::<Experience>(companion).unwrap().level,
        12,
        "an arena scenario stages the level it authors, not the zone's cap"
    );
    assert_eq!(
        arena_level_ceiling(),
        TALENT_START_LEVEL + KERNEL_RING_MAX * LEVELS_PER_RING,
        "and the arena's own ceiling is unchanged by the zone cap"
    );
}
