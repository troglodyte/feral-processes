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
    assert_eq!(idle.key, 'b', "the base report is where a node is staffed");
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
    assert_eq!(perks.key, 'p', "perks live behind the Perks page");
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

/// A roster exactly filling its slots is fine now — slots stopped being a
/// door — so the row waits for the first program with no slot, which is
/// the first one earning `unslotted`.
#[test]
fn a_roster_past_its_slots_says_so() {
    let mut game = fresh(5);
    while game.pet_count() < game.pet_capacity() {
        spawn_tamed(&mut game, 10, 3);
    }
    assert!(
        !kinds(&mut game).contains(&AttentionKind::Unslotted),
        "every program has a slot"
    );

    spawn_tamed(&mut game, 10, 3);

    let over = row(&mut game, AttentionKind::Unslotted);
    let (count, capacity) = (game.pet_count(), game.pet_capacity());
    assert_eq!(over.text, format!("1 unslotted ({count}/{capacity})"));
    assert_eq!(over.key, 'p');
    assert!(!over.threat);
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

/// Staff standing in a base, each holding `grievance` (or nothing) — no tick,
/// so `update_disgruntled` cannot clear what the test inserted.
fn soured_staff(game: &mut Game, grievances: &[Option<crate::components::Grievance>]) {
    stand_in_base(game);
    for (i, g) in grievances.iter().enumerate() {
        let body = spawn_tamed_on_map(game, -2 - i as i32, 0);
        if let Some(grievance) = *g {
            game.world
                .entity_mut(body)
                .insert(crate::components::Disgruntled {
                    grievance,
                    stranded: false,
                    told: false,
                });
        }
    }
}

/// The two quiet steps are the map's and the CREW bar's to show; raising
/// them here would nag about a body that is still working.
#[test]
fn a_sulking_staff_raises_no_attention() {
    use crate::components::Grievance;
    let mut game = fresh(7);
    soured_staff(&mut game, &[Some(Grievance::Sulking), None]);
    assert!(!kinds(&mut game).contains(&AttentionKind::StaffUnrest));
}

/// One row per step and never one per body, lashing first.
#[test]
fn downed_and_lashing_staff_raise_one_row_each() {
    use crate::components::Grievance;
    let mut game = fresh(7);
    soured_staff(
        &mut game,
        &[
            Some(Grievance::DownedTools),
            Some(Grievance::LashingOut),
            Some(Grievance::DownedTools),
        ],
    );
    let rows: Vec<AttentionRow> = game
        .attention()
        .into_iter()
        .filter(|r| r.kind == AttentionKind::StaffUnrest)
        .collect();
    let texts: Vec<&str> = rows.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(texts, ["1 lashing out (Base)", "2 downed tools (Base)"]);
    assert!(rows.iter().all(|r| r.key == 'b' && r.threat));
}

/// The CREW bar shows the first CREW row, so a program hitting people must
/// come before an unspent point — the screenshot that found this read
/// "11 perk points unspent" over a tantrum.
#[test]
fn staff_unrest_outranks_unspent_points() {
    use crate::components::Grievance;
    let mut game = fresh(7);
    soured_staff(&mut game, &[Some(Grievance::LashingOut)]);
    let player = game.player_entity();
    game.world.get_mut::<Perks>(player).unwrap().points = 3;
    let k = kinds(&mut game);
    let at = |kind| {
        k.iter()
            .position(|&x| x == kind)
            .unwrap_or_else(|| panic!("{kind:?} in {k:?}"))
    };
    assert!(
        at(AttentionKind::StaffUnrest) < at(AttentionKind::PerkPoints),
        "{k:?}"
    );
}
