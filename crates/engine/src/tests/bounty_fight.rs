//! The bounty fight: the bump, the roster, the verdict and the fail door
//! (ADR 0044), in both combat models.

use super::bounty::{accepted, anchor_of, board_bounty, broker_game, quarries};
use super::support::*;
use crate::components::{BountyMark, Hostile};
use crate::contracts::{ContractDef, ContractId, Objective, Reward};
use crate::game::bounty::BountyFight;
use crate::resources::{ActiveContract, ActiveContracts, BattleState, CompassBearing};
use crate::settlements::CompassTarget;
use crate::tactical::TacticalBattle;
use crate::*;

fn bump_the_quarry(game: &mut Game) {
    let anchor = anchor_of(game);
    stand_in_base_at(game, anchor.0, anchor.1 + 1);
    game.move_player(0, -1);
}

fn roster(game: &Game) -> (Entity, Vec<Entity>) {
    let fight = game.world.resource::<BountyFight>();
    (fight.quarry, fight.escort.clone())
}

fn announced(game: &Game, needle: &str) -> usize {
    game.message_log(usize::MAX)
        .iter()
        .filter(|l| l.text.contains(needle))
        .count()
}

/// Ends the fight the way jacking out does, in whichever model is open.
fn flee(game: &mut Game) {
    if game.world.get_resource::<TacticalBattle>().is_some() {
        let player = game.player_entity();
        game.world.resource_mut::<TacticalBattle>().remove(player);
        game.end_tactical_battle(None);
    } else {
        let player = game.player_entity();
        game.end_battle(player, None);
    }
}

/// Puts `victim` down through the model's own kill path.
fn kill(game: &mut Game, victim: Entity) {
    let player = game.player_entity();
    if game.world.get_resource::<TacticalBattle>().is_some() {
        game.world.get_mut::<Stats>(victim).unwrap().hp = 0;
        game.reap_tactical_dead(None);
        return;
    }
    let (group, index) = {
        let battle = game.world.resource::<BattleState>();
        battle
            .groups
            .iter()
            .enumerate()
            .find_map(|(g, grp)| {
                grp.members
                    .iter()
                    .position(|&m| m == victim)
                    .map(|i| (g, i))
            })
            .expect("the victim is in the fight")
    };
    game.finish_member(group, index, player);
}

fn strong_party(game: &mut Game) {
    let player = game.player_entity();
    let mut stats = game.world.get_mut::<Stats>(player).unwrap();
    stats.max_hp = 100_000;
    stats.hp = 100_000;
}

fn game_in(tactical: bool) -> (Game, ContractId) {
    let (mut game, id) = accepted();
    game.set_tactical_battles(tactical);
    strong_party(&mut game);
    (game, id)
}

fn job(id: &str, objective: Objective, credits: u32) -> ActiveContract {
    ActiveContract {
        def: ContractDef {
            id: ContractId::from(id.to_string()),
            name: id.to_string(),
            description: String::new(),
            objective,
            reward: vec![Reward::Credits(credits)],
            min_zone: 0,
            repeatable: false,
            starter: false,
            tutorial: None,
        },
        progress: 0,
        accepted_tick: 0,
        issuer: None,
    }
}

#[test]
fn bumping_the_quarry_opens_an_abstract_fight_holding_the_whole_roster() {
    let (mut game, _) = game_in(false);
    bump_the_quarry(&mut game);
    let (quarry, escort) = roster(&game);
    assert_eq!(
        escort.len(),
        game.held_bounty().unwrap().1.escort_size() as usize
    );
    let battle = game.world.resource::<BattleState>();
    let held: Vec<Entity> = battle
        .groups
        .iter()
        .flat_map(|g| g.members.iter().copied())
        .collect();
    assert_eq!(held.len(), 1 + escort.len(), "no ceiling trims the roster");
    assert_eq!(battle.groups[0].members[0], quarry, "the quarry leads");
    assert!(escort.iter().all(|e| held.contains(e)));
}

#[test]
fn bumping_the_quarry_opens_a_tactical_fight_holding_the_whole_roster() {
    let (mut game, _) = game_in(true);
    bump_the_quarry(&mut game);
    assert!(
        game.world.get_resource::<TacticalBattle>().is_some(),
        "the tactical model took it"
    );
    let (quarry, escort) = roster(&game);
    assert_eq!(
        escort.len(),
        game.held_bounty().unwrap().1.escort_size() as usize
    );
    let battle = game.world.resource::<TacticalBattle>();
    let bodies: Vec<Entity> = battle.bodies().map(|(e, _)| e).collect();
    assert!(bodies.contains(&quarry));
    let hostiles = bodies
        .iter()
        .filter(|&&e| game.world.get::<Hostile>(e).is_some())
        .count();
    assert!(hostiles >= 2, "the quarry and the escort stand");
}

#[test]
fn the_quarry_leads_a_group_of_its_own_and_each_escort_species_gets_one() {
    for seed in 1..200 {
        let mut game = broker_game(seed, 2);
        let Some(def) = board_bounty(&mut game) else {
            continue;
        };
        game.accept_contract(&def.id, None).unwrap();
        game.set_tactical_battles(false);
        strong_party(&mut game);
        bump_the_quarry(&mut game);
        let groups = game.world.resource::<BattleState>().groups.len();
        let quarry = game.held_bounty().unwrap().1;
        let kinds = quarry.escort.len();
        assert_eq!(
            groups,
            1 + kinds,
            "the quarry alone, then one group per escort species, none cut"
        );
        let first = &game.world.resource::<BattleState>().groups[0];
        assert_eq!(
            first.members.len(),
            1,
            "the quarry is not folded into a pack"
        );
        if kinds >= 2 {
            return;
        }
    }
    panic!("no sector 2 bounty with two species");
}

#[test]
fn the_quarry_stays_out_of_every_other_pack() {
    let (mut game, _) = accepted();
    let quarry = quarries(&mut game)[0].0;
    let anchor = anchor_of(&game);
    let neighbour = game
        .spawn_wild_creature_scaled("drone", anchor.0 + 1, anchor.1, 1.0, false)
        .unwrap();
    let pack = game.gather_pack(neighbour);
    assert!(!pack.contains(&quarry));
    assert_eq!(pack[0], neighbour);
}

#[test]
fn killing_the_quarry_then_fleeing_completes_the_contract_once() {
    let (mut game, id) = game_in(false);
    let before = credits(&game);
    let Some(Reward::Credits(paid)) = game.world.resource::<ActiveContracts>().active[0]
        .def
        .reward
        .first()
        .cloned()
    else {
        panic!("credits come first")
    };
    bump_the_quarry(&mut game);
    let (quarry, _) = roster(&game);
    kill(&mut game, quarry);
    flee(&mut game);
    assert!(!game.world.contains_resource::<BountyFight>());
    let held = game.world.resource::<ActiveContracts>();
    assert!(held.active.is_empty());
    assert_eq!(held.done.iter().filter(|d| **d == id).count(), 1);
    assert!(held.failed.is_empty());
    assert_eq!(credits(&game), before + paid, "paid exactly once");
    assert_eq!(announced(&game, "got away"), 0);
}

#[test]
fn a_quarry_settled_mid_fight_is_not_paid_again_at_teardown() {
    let (mut game, id) = game_in(false);
    // A second held contract, ahead of the bounty: a teardown that completes
    // by position instead of by id would pay it.
    game.world.resource_mut::<ActiveContracts>().active.insert(
        0,
        job(
            "extra_job",
            Objective::Terminate {
                species: None,
                count: 99,
            },
            777,
        ),
    );
    bump_the_quarry(&mut game);
    let (quarry, _) = roster(&game);
    kill(&mut game, quarry);
    // The normal door settles it mid-fight; the teardown must find it gone.
    game.settle_contracts();
    assert!(game.world.resource::<ActiveContracts>().done.contains(&id));
    let mid = credits(&game);
    flee(&mut game);
    let held = game.world.resource::<ActiveContracts>();
    assert_eq!(held.done.iter().filter(|d| **d == id).count(), 1);
    assert_eq!(held.active.len(), 1, "the other contract is untouched");
    assert_eq!(credits(&game), mid, "the teardown paid nothing more");
}

fn assert_failed_cleanly(game: &mut Game, id: &ContractId) {
    let held = game.world.resource::<ActiveContracts>();
    assert!(held.active.is_empty(), "the contract is gone");
    assert!(!held.done.contains(id));
    assert!(held.failed.contains(id));
    assert!(quarries(game).is_empty(), "the quarry is despawned");
    assert!(!game.world.contains_resource::<BountyFight>());
    assert_eq!(game.world.resource::<CompassBearing>().0, None);
    assert_eq!(announced(game, "Bounty failed."), 1, "announced once");
    assert!(
        game.compass_targets()
            .iter()
            .all(|r| !matches!(r.target, CompassTarget::Bounty(_)))
    );
    assert!(board_bounty(game).is_none(), "not re-offered this epoch");
    let mut q = game.world.query_filtered::<Entity, With<Hostile>>();
    assert!(
        q.iter(&game.world)
            .all(|e| game.world.get::<BountyMark>(e).is_none()),
        "no marked body left"
    );
}

#[test]
fn fleeing_with_the_quarry_alive_fails_the_contract_abstract() {
    let (mut game, id) = game_in(false);
    game.set_compass_bearing(Some(CompassTarget::Bounty(id.clone())));
    bump_the_quarry(&mut game);
    let (_, escort) = roster(&game);
    flee(&mut game);
    assert_failed_cleanly(&mut game, &id);
    assert!(
        escort.iter().all(|&e| game.world.get_entity(e).is_err()),
        "surviving escort is gone"
    );
}

#[test]
fn fleeing_with_the_quarry_alive_fails_the_contract_tactical() {
    let (mut game, id) = game_in(true);
    game.set_compass_bearing(Some(CompassTarget::Bounty(id.clone())));
    bump_the_quarry(&mut game);
    assert!(game.world.get_resource::<TacticalBattle>().is_some());
    let (_, escort) = roster(&game);
    flee(&mut game);
    assert_failed_cleanly(&mut game, &id);
    assert!(escort.iter().all(|&e| game.world.get_entity(e).is_err()));
}

#[test]
fn clearing_the_escort_while_the_quarry_lives_still_fails() {
    let (mut game, id) = game_in(false);
    bump_the_quarry(&mut game);
    let (_, escort) = roster(&game);
    for e in escort {
        kill(&mut game, e);
    }
    assert!(game.world.contains_resource::<BattleState>());
    flee(&mut game);
    assert_failed_cleanly(&mut game, &id);
}

#[test]
fn a_forgiving_loss_with_the_quarry_alive_fails_the_contract() {
    let (mut game, id) = game_in(false);
    bump_the_quarry(&mut game);
    let player = game.player_entity();
    game.world.get_mut::<Stats>(player).unwrap().hp = 0;
    game.end_battle(player, None);
    game.tick();
    assert_failed_cleanly(&mut game, &id);
}

#[test]
fn winning_by_clearing_everything_completes_the_contract() {
    let (mut game, id) = game_in(false);
    bump_the_quarry(&mut game);
    let (quarry, escort) = roster(&game);
    for e in escort {
        kill(&mut game, e);
    }
    kill(&mut game, quarry);
    assert!(!game.world.contains_resource::<BattleState>(), "won");
    let held = game.world.resource::<ActiveContracts>();
    assert!(held.done.contains(&id));
    assert!(held.failed.is_empty());
}

#[test]
fn winning_a_tactical_fight_completes_the_contract() {
    let (mut game, id) = game_in(true);
    bump_the_quarry(&mut game);
    let (quarry, escort) = roster(&game);
    for e in escort.iter().copied().chain([quarry]) {
        if let Some(mut s) = game.world.get_mut::<Stats>(e) {
            s.hp = 0;
        }
    }
    game.reap_tactical_dead(None);
    assert!(!game.world.contains_resource::<TacticalBattle>());
    let held = game.world.resource::<ActiveContracts>();
    assert!(held.done.contains(&id));
}

#[test]
fn escort_kills_advance_a_held_terminate_contract() {
    let (mut game, _) = game_in(false);
    let want = game.held_bounty().unwrap().1.escort[0].0.clone();
    game.world
        .resource_mut::<ActiveContracts>()
        .active
        .push(job(
            "hunt_job",
            Objective::Terminate {
                species: Some(want.clone()),
                count: 1,
            },
            5,
        ));
    bump_the_quarry(&mut game);
    let (_, escort) = roster(&game);
    let victim = escort
        .iter()
        .copied()
        .find(|&e| game.world.get::<Creature>(e).unwrap().species == want)
        .unwrap();
    kill(&mut game, victim);
    game.tick();
    game.tick();
    assert!(
        game.world
            .resource::<ActiveContracts>()
            .done
            .iter()
            .any(|d| d.as_str() == "hunt_job")
    );
}

#[test]
fn the_abstract_roster_and_intercept_line_name_the_quarry() {
    let (mut game, _) = game_in(false);
    let name = game.held_bounty().unwrap().1.name;
    bump_the_quarry(&mut game);
    let view = game.battle_view().unwrap();
    assert_eq!(view.groups[0].species_name, name);
    assert!(
        view.groups.iter().skip(1).all(|g| g.species_name != name),
        "only the quarry's row carries the name"
    );
    assert_eq!(
        announced(&game, &format!("{name} intercepts your signal")),
        1
    );
    assert_eq!(announced(&game, "rogue"), 0);
}

#[test]
fn the_tactical_roster_and_intercept_line_name_the_quarry() {
    let (mut game, _) = game_in(true);
    let name = game.held_bounty().unwrap().1.name;
    bump_the_quarry(&mut game);
    let view = game.tactical_view().unwrap();
    assert_eq!(
        view.bodies.iter().filter(|b| b.label == name).count(),
        1,
        "the quarry's body is labelled by name"
    );
    assert_eq!(
        announced(&game, &format!("{name} intercepts your signal")),
        1
    );
}
