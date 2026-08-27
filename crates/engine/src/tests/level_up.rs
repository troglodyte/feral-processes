//! What a level-up announces. The three sites that can grant one — the
//! player, a party member and a posted worker — each write a header line
//! and a block of stat lines under it, and they have to agree on the shape
//! since `progression::stat_block` is the one thing that renders it.

use super::support::*;
use crate::components::{Decompiler, Experience, Stats};
use crate::progression::{StatRow, stat_block};
use crate::resources::{CONDENSE_LOOKBACK, LogLine, MessageSource, condense};
use crate::tuning::{
    CREATURE_MAX_LEVEL, DECOMPILER_SKILL_PER_LEVEL, KERNEL_RING_MAX, LEVELS_PER_RING,
    PERK_POINTS_PER_LEVEL, absolute_companion_level_cap,
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
    (1..=absolute_companion_level_cap() + 4)
        .map(crate::progression::xp_for_level)
        .sum::<u32>()
        * 2
}

fn a_party_member(game: &mut Game) -> Entity {
    let companion = spawn_tamed(game, 10, 3);
    game.world.resource_mut::<Party>().0.push(companion);
    companion
}

#[test]
fn a_companion_with_no_ring_still_stops_at_the_base_cap() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = a_party_member(&mut game);

    game.award_party_xp(xp_past_every_cap());

    assert_eq!(
        game.world.get::<Experience>(companion).unwrap().level,
        CREATURE_MAX_LEVEL,
        "a companion with no Kernel Ring must still stop where it always did"
    );
}

#[test]
fn one_kernel_ring_lifts_a_companions_ceiling_by_its_levels() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = a_party_member(&mut game);
    game.world.entity_mut(companion).insert(KernelRing(1));

    game.award_party_xp(xp_past_every_cap());

    assert_eq!(
        game.world.get::<Experience>(companion).unwrap().level,
        CREATURE_MAX_LEVEL + LEVELS_PER_RING,
        "one ring buys exactly LEVELS_PER_RING levels, and no more"
    );
    assert_eq!(
        game.companion_level_cap(companion),
        CREATURE_MAX_LEVEL + LEVELS_PER_RING
    );
}

#[test]
fn every_ring_open_stops_at_the_absolute_cap() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = a_party_member(&mut game);
    game.world
        .entity_mut(companion)
        .insert(KernelRing(KERNEL_RING_MAX));

    game.award_party_xp(xp_past_every_cap());

    assert_eq!(
        game.world.get::<Experience>(companion).unwrap().level,
        absolute_companion_level_cap(),
        "the last ring is still a ceiling"
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
/// `#[serde(skip)]` looks like from its side. Both halves matter — the count
/// survives, *and* the ceiling it bought is still lifted on the loaded game.
#[test]
fn a_kernel_ring_survives_a_save_and_still_lifts_the_ceiling() {
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
    assert_eq!(
        loaded.companion_level_cap(restored),
        CREATURE_MAX_LEVEL + 2 * LEVELS_PER_RING,
        "and the ceiling it bought must still be lifted"
    );

    loaded.award_party_xp(xp_past_every_cap());
    assert_eq!(
        loaded.world.get::<Experience>(restored).unwrap().level,
        CREATURE_MAX_LEVEL + 2 * LEVELS_PER_RING
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
