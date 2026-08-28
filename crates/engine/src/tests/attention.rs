//! What needs the player right now — `Game::attention` and the four
//! conditions it reports.
//!
//! The HUD's badge, its tab markers and its collapsed bars are all readouts
//! of this one call, which is what makes "a closed pane cannot hide an
//! actionable state" a construction rather than three sites agreeing. The
//! renderer-side half of that is `hud::column`'s census.

use super::support::*;
use crate::components::{Durability, Perks};
use crate::*;

fn fresh(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn kinds(game: &mut Game) -> Vec<AttentionKind> {
    game.attention().into_iter().map(|r| r.kind).collect()
}

fn row(game: &mut Game, kind: AttentionKind) -> AttentionRow {
    game.attention()
        .into_iter()
        .find(|r| r.kind == kind)
        .unwrap_or_else(|| panic!("no {kind:?} row in {:?}", kinds(game)))
}

/// The calm state is a real state and the HUD draws it — `ALL NOMINAL` is
/// read off this being empty, so it has to be reachable.
#[test]
fn a_calm_base_needs_nothing() {
    let mut game = fresh(1);
    assert_eq!(
        kinds(&mut game),
        Vec::new(),
        "a fresh run wants nothing yet"
    );
}

#[test]
fn an_idle_node_asks_for_a_program() {
    let mut game = fresh(2);
    deploy_upgradeable_node(&mut game);

    let idle = row(&mut game, AttentionKind::IdleStructures);
    assert_eq!(idle.text, "1 node without a program");
    assert_eq!(idle.key, 'b', "the base menu is where a node is staffed");
    assert!(!idle.threat, "an idle node is work, not harm");
}

/// Both halves in one test: the row at zero is an omission, and an omission
/// is what regresses in silence.
#[test]
fn unspent_perk_points_ask_to_be_spent() {
    let mut game = fresh(3);
    assert!(
        !kinds(&mut game).contains(&AttentionKind::PerkPoints),
        "a player with nothing to spend is not being asked to spend it"
    );

    let player = game.player_entity();
    game.world.get_mut::<Perks>(player).unwrap().points = 3;

    let perks = row(&mut game, AttentionKind::PerkPoints);
    assert_eq!(perks.text, "3 perk points unspent");
    assert_eq!(perks.key, 'p', "perks live behind the party menu");
    assert!(!perks.threat);
}

#[test]
fn one_unspent_point_is_singular() {
    let mut game = fresh(4);
    let player = game.player_entity();
    game.world.get_mut::<Perks>(player).unwrap().points = 1;

    assert_eq!(
        row(&mut game, AttentionKind::PerkPoints).text,
        "1 perk point unspent"
    );
}

#[test]
fn a_full_roster_says_so() {
    let mut game = fresh(5);
    assert!(
        !kinds(&mut game).contains(&AttentionKind::RosterFull),
        "a fresh roster has room"
    );

    while game.pet_count() < game.pet_capacity() {
        spawn_tamed(&mut game, 10, 3);
    }

    let full = row(&mut game, AttentionKind::RosterFull);
    let (count, capacity) = (game.pet_count(), game.pet_capacity());
    assert_eq!(full.text, format!("roster full ({count}/{capacity})"));
    assert_eq!(full.key, 'p');
    assert!(!full.threat);
}

#[test]
fn a_damaged_structure_is_the_threat_row() {
    let mut game = fresh(6);
    let node = deploy_upgradeable_node(&mut game);
    let label = game
        .structure_report()
        .into_iter()
        .find(|s| s.entity == node)
        .expect("the node is on the roster")
        .label;

    let mut hp = game.world.get_mut::<Durability>(node).unwrap();
    hp.hp = hp.max_hp - 1;

    let damaged = row(&mut game, AttentionKind::StructureDamaged);
    assert_eq!(damaged.text, format!("{label} damaged"));
    assert_eq!(damaged.key, 'b');
    assert!(damaged.threat, "a structure coming down is inbound harm");
}

/// One row however many are damaged: the badge names a condition, not a
/// casualty list, and four rows for four hits would push everything else off
/// the column.
#[test]
fn one_row_however_many_are_damaged() {
    let mut game = fresh(7);
    // `deploy_upgradeable_node` stands the Home up itself; a second
    // `place_home` is refused.
    let node = deploy_upgradeable_node(&mut game);
    let home = find_structure_by_kind(&mut game, "home").expect("the Home is standing");

    for e in [home, node] {
        if let Some(mut hp) = game.world.get_mut::<Durability>(e) {
            hp.hp = 1;
        }
    }

    assert_eq!(
        kinds(&mut game)
            .iter()
            .filter(|k| **k == AttentionKind::StructureDamaged)
            .count(),
        1
    );
}

/// The one thing about the ordering worth pinning, and the deviation from
/// the design's own table: the badge shows the most urgent row, and a raid
/// eating the base reading second to "3 perk points unspent" is wrong on a
/// HUD.
#[test]
fn a_threat_sorts_above_everything_else() {
    let mut game = fresh(8);
    let node = deploy_upgradeable_node(&mut game);
    let player = game.player_entity();
    game.world.get_mut::<Perks>(player).unwrap().points = 2;
    let mut hp = game.world.get_mut::<Durability>(node).unwrap();
    hp.hp = 1;

    let all = kinds(&mut game);
    assert_eq!(
        all.first(),
        Some(&AttentionKind::StructureDamaged),
        "the threat row leads: {all:?}"
    );
    assert!(all.contains(&AttentionKind::IdleStructures));
    assert!(all.contains(&AttentionKind::PerkPoints));
}
