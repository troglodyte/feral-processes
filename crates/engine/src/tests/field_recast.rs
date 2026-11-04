//! Auto-recast: the per-routine flag, and the rerun when a timed field buff
//! runs out.

use super::support::*;
use crate::abilities::AbilityDb;
use crate::components::{AutoRecast, Routines};
use crate::routine_craft::Core;
use crate::*;

fn game() -> Game {
    Game::new(41_001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// Gives `entity` a freshly minted routine built from `core` and returns its
/// id. Built, not hand-written, so the def is the one the game compiles.
fn give_core(game: &mut Game, entity: Entity, core: Core) -> String {
    make_room_for_routine(game, entity);
    let id = built_core(game, core);
    let mut installed = game.world.get::<Routines>(entity).unwrap().0.clone();
    installed.push(id.clone());
    game.world.entity_mut(entity).insert(Routines(installed));
    id
}

fn eligible(game: &Game, id: &str) -> bool {
    game.world
        .resource::<AbilityDb>()
        .get(id)
        .unwrap()
        .auto_recastable()
}

#[test]
fn only_a_timed_field_buff_is_eligible() {
    let mut game = game();
    let regen = built_core(&mut game, Core::FieldRegen);
    let recharge = built_core(&mut game, Core::Recharge);
    let shell = built_core(&mut game, Core::FieldMitigation);
    let heal = built_core(&mut game, Core::Heal);
    let jump = built_core(&mut game, Core::Jump);

    assert!(eligible(&game, &regen));
    assert!(eligible(&game, &recharge));
    assert!(!eligible(&game, &shell), "it runs until rest");
    assert!(!eligible(&game, &heal));
    assert!(!eligible(&game, &jump));
}

#[test]
fn flags_survive_a_save_and_load_for_the_player_and_a_companion() {
    let mut game = game();
    let player = game.player_entity();
    let pet = spawn_tamed(&mut game, 10, 3);
    set_level(&mut game, pet, 10);
    let mine = give_core(&mut game, player, Core::FieldRegen);
    let theirs = give_core(&mut game, pet, Core::Recharge);
    game.set_auto_recast(player, &mine, true);
    game.set_auto_recast(pet, &theirs, true);

    let dir = scratch_assets_dir("auto_recast_roundtrip");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("s.ron");
    game.save(&path).expect("save");
    let mut loaded = Game::load(&path, &test_assets_dir()).expect("load");

    assert_eq!(loaded.auto_recast_of(loaded.player_entity()), vec![mine]);
    let loaded_pet = loaded
        .owned_pets()
        .first()
        .map(|p| p.entity)
        .expect("the companion came back");
    assert_eq!(loaded.auto_recast_of(loaded_pet), vec![theirs]);
}

#[test]
fn an_empty_flag_set_writes_nothing() {
    let mut game = game();
    let dir = scratch_assets_dir("auto_recast_empty");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("s.ron");
    game.save(&path).expect("save");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(!text.contains("auto_recast"));
}

#[test]
fn uninstalling_a_routine_clears_its_flag() {
    let mut game = game();
    let player = game.player_entity();
    let id = give_core(&mut game, player, Core::FieldRegen);
    game.set_auto_recast(player, &id, true);
    let slot = game
        .world
        .get::<Routines>(player)
        .unwrap()
        .0
        .iter()
        .position(|r| *r == id)
        .unwrap();

    game.uninstall_routine(player, slot).unwrap();

    assert!(game.auto_recast_of(player).is_empty());
}

#[test]
fn fusion_keeps_a_flag_only_for_a_routine_the_child_holds() {
    let mut game = game();
    unlock_research_chain(&mut game, "program_refactoring");
    let dominant = spawn_tamed(&mut game, 30, 6);
    set_level(&mut game, dominant, 10);
    let other = spawn_tamed(&mut game, 10, 3);
    let built = give_core(&mut game, dominant, Core::FieldRegen);
    let innate = crate::abilities::FALLBACK_ABILITY_ID.to_string();
    game.world
        .entity_mut(dominant)
        .insert(AutoRecast(vec![built.clone(), innate.clone()]));

    game.fuse_companions(dominant, other, None).unwrap();

    let child = game.owned_pets().first().map(|p| p.entity).unwrap();
    let kit = game.world.get::<Routines>(child).unwrap().0.clone();
    assert!(!kit.contains(&built), "a built routine does not survive");
    let flags = game.auto_recast_of(child);
    assert!(!flags.contains(&built));
    assert_eq!(flags.contains(&innate), kit.contains(&innate));
}

#[test]
fn a_manual_run_records_the_routine_on_the_buff_and_keeps_it_across_a_save() {
    let mut game = game();
    pin_player_combat(&mut game);
    let player = game.player_entity();
    let id = give_core(&mut game, player, Core::FieldRegen);
    let index = game
        .field_routines()
        .iter()
        .position(|r| r.ability == id)
        .unwrap();

    game.run_field_routine(index, FieldRoutineTarget::Ally(player))
        .unwrap();

    let armed = |game: &Game, who: Entity| {
        game.world
            .get::<crate::components::FieldBuff>(who)
            .unwrap()
            .active
            .iter()
            .map(|b| b.ability.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(armed(&game, player), vec![Some(id.clone())]);

    let dir = scratch_assets_dir("auto_recast_buff_ability");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("s.ron");
    game.save(&path).expect("save");
    let loaded = Game::load(&path, &test_assets_dir()).expect("load");
    assert_eq!(armed(&loaded, loaded.player_entity()), vec![Some(id)]);
}

fn row_for<'a>(rows: &'a [FieldRoutineView], holder: Entity, id: &str) -> &'a FieldRoutineView {
    rows.iter()
        .find(|r| r.holder == holder && r.ability == id)
        .expect("the row is listed")
}

#[test]
fn the_toggle_flips_the_flag_and_the_view_follows() {
    let mut game = game();
    let player = game.player_entity();
    let id = give_core(&mut game, player, Core::FieldRegen);
    let index = |game: &mut Game| {
        game.field_routines()
            .iter()
            .position(|r| r.ability == id)
            .unwrap()
    };

    let rows = game.field_routines();
    assert_eq!(row_for(&rows, player, &id).auto_recast, Some(false));

    let i = index(&mut game);
    assert_eq!(game.toggle_field_auto_recast(i), Ok(true));
    let rows = game.field_routines();
    assert_eq!(row_for(&rows, player, &id).auto_recast, Some(true));

    let i = index(&mut game);
    assert_eq!(game.toggle_field_auto_recast(i), Ok(false));
    let rows = game.field_routines();
    assert_eq!(row_for(&rows, player, &id).auto_recast, Some(false));
}

#[test]
fn two_holders_of_one_routine_toggle_independently() {
    let mut game = game();
    let player = game.player_entity();
    let pet = spawn_tamed(&mut game, 10, 3);
    set_level(&mut game, pet, 10);
    let id = give_core(&mut game, player, Core::FieldRegen);
    make_room_for_routine(&mut game, pet);
    let mut kit = game.world.get::<Routines>(pet).unwrap().0.clone();
    kit.push(id.clone());
    game.world.entity_mut(pet).insert(Routines(kit));

    let rows = game.field_routines();
    let pets_row = rows
        .iter()
        .position(|r| r.holder == pet && r.ability == id)
        .unwrap();
    game.toggle_field_auto_recast(pets_row).unwrap();

    let rows = game.field_routines();
    assert_eq!(row_for(&rows, pet, &id).auto_recast, Some(true));
    assert_eq!(row_for(&rows, player, &id).auto_recast, Some(false));
}

#[test]
fn an_ineligible_row_and_a_bad_index_refuse() {
    let mut game = game();
    let player = game.player_entity();
    let shell = give_core(&mut game, player, Core::FieldMitigation);
    let rows = game.field_routines();
    assert_eq!(row_for(&rows, player, &shell).auto_recast, None);
    let i = rows.iter().position(|r| r.ability == shell).unwrap();
    let name = rows[i].name.clone();

    assert_eq!(
        game.toggle_field_auto_recast(i),
        Err(format!("{name} doesn't run out on its own."))
    );
    assert_eq!(
        game.toggle_field_auto_recast(999),
        Err("No such routine.".to_string())
    );
}

// ---------------------------------------------------------------------------
// The recast itself
// ---------------------------------------------------------------------------

use crate::components::{FieldBuff, POWER_MAX, PowerReserve};
use crate::game::field_recast::{Caster, Expiry, Plan, plan_recasts};
use crate::resources::{MessageLog, Party};
use crate::routine_craft::{AreaKind, Subroutine};

fn reserve(game: &Game, e: Entity) -> f32 {
    game.world.get::<PowerReserve>(e).unwrap().get()
}

fn fill(game: &mut Game, e: Entity, amount: f32) {
    *game.world.get_mut::<PowerReserve>(e).unwrap() = PowerReserve::new(amount, POWER_MAX);
}

fn log_has(game: &Game, needle: &str) -> bool {
    game.world
        .resource::<MessageLog>()
        .lines
        .iter()
        .any(|l| l.text.contains(needle))
}

fn buffs(game: &Game, e: Entity) -> Vec<crate::components::ActiveFieldBuff> {
    game.world
        .get::<FieldBuff>(e)
        .map(|f| f.active.clone())
        .unwrap_or_default()
}

/// Runs `id` (a row on `holder`) with `pick`, then leaves every routine buff
/// on the party one tick from expiring.
fn run_and_age(game: &mut Game, holder: Entity, id: &str, pick: FieldRoutineTarget) {
    let index = game
        .field_routines()
        .iter()
        .position(|r| r.holder == holder && r.ability == id)
        .unwrap();
    game.run_field_routine(index, pick).unwrap();
    let mut bodies = vec![game.player_entity()];
    bodies.extend(game.world.resource::<Party>().0.clone());
    for e in bodies {
        if let Some(mut f) = game.world.get_mut::<FieldBuff>(e) {
            for b in &mut f.active {
                b.remaining = 1;
            }
        }
    }
}

fn cost_of(game: &Game, holder: Entity, id: &str) -> f32 {
    let def = game.world.resource::<AbilityDb>().get(id).unwrap().clone();
    game.routine_cost(holder, &def)
}

#[test]
fn a_flagged_one_ally_routine_reruns_on_expiry_and_pays_once() {
    let mut game = game();
    pin_player_combat(&mut game);
    let player = game.player_entity();
    let id = give_core(&mut game, player, Core::FieldRegen);
    game.set_auto_recast(player, &id, true);
    fill(&mut game, player, 100.0);
    run_and_age(&mut game, player, &id, FieldRoutineTarget::Ally(player));
    let before = reserve(&game, player);

    game.tick_field_buffs();

    let after = buffs(&game, player);
    assert_eq!(after.len(), 1, "the buff was replaced, not lost");
    assert!(after[0].remaining > 1);
    assert_eq!(after[0].ability.as_deref(), Some(id.as_str()));
    assert_eq!(before - reserve(&game, player), cost_of(&game, player, &id));
    assert!(log_has(&game, "reruns"));
    assert!(!log_has(&game, "fades"));
}

#[test]
fn a_whole_party_routine_reruns_once_and_pays_once() {
    let mut game = game();
    pin_player_combat(&mut game);
    let player = game.player_entity();
    let pet = spawn_tamed(&mut game, 10, 3);
    game.world.resource_mut::<Party>().0.push(pet);
    let id = mint_parts(
        &mut game,
        &[
            Subroutine::Core(Core::FieldRegen, crate::routine_craft::Version::V1),
            Subroutine::Area(AreaKind::Group),
        ],
    );
    make_room_for_routine(&mut game, player);
    let mut kit = game.world.get::<Routines>(player).unwrap().0.clone();
    kit.push(id.clone());
    game.world.entity_mut(player).insert(Routines(kit));
    game.set_auto_recast(player, &id, true);
    fill(&mut game, player, 100.0);
    run_and_age(&mut game, player, &id, FieldRoutineTarget::None);
    let before = reserve(&game, player);

    game.tick_field_buffs();

    assert_eq!(buffs(&game, player).len(), 1);
    assert_eq!(buffs(&game, pet).len(), 1);
    assert_eq!(
        before - reserve(&game, player),
        cost_of(&game, player, &id),
        "one payment for the whole party"
    );
    let reruns = game
        .world
        .resource::<MessageLog>()
        .lines
        .iter()
        .filter(|l| l.text.contains("reruns"))
        .count();
    assert_eq!(reruns, 1);
}

#[test]
fn short_of_power_it_fades_and_turns_the_flag_off() {
    let mut game = game();
    pin_player_combat(&mut game);
    let player = game.player_entity();
    let id = give_core(&mut game, player, Core::FieldRegen);
    game.set_auto_recast(player, &id, true);
    fill(&mut game, player, 100.0);
    run_and_age(&mut game, player, &id, FieldRoutineTarget::Ally(player));
    fill(&mut game, player, 0.0);

    game.tick_field_buffs();

    assert!(buffs(&game, player).is_empty());
    assert!(log_has(&game, "couldn't rerun it: Can't run"));
    assert!(log_has(&game, "Auto-recast is off."));
    assert!(game.auto_recast_of(player).is_empty());
}

#[test]
fn a_downed_ally_fades_and_the_flag_stays_on() {
    let mut game = game();
    pin_player_combat(&mut game);
    let player = game.player_entity();
    let pet = spawn_tamed(&mut game, 10, 3);
    game.world.resource_mut::<Party>().0.push(pet);
    let id = give_core(&mut game, player, Core::FieldRegen);
    game.set_auto_recast(player, &id, true);
    fill(&mut game, player, 100.0);
    run_and_age(&mut game, player, &id, FieldRoutineTarget::Ally(pet));
    game.world.get_mut::<Stats>(pet).unwrap().hp = 0;

    game.tick_field_buffs();

    assert!(buffs(&game, pet).is_empty());
    assert!(log_has(&game, "couldn't rerun it"));
    assert!(!log_has(&game, "Auto-recast is off."));
    assert_eq!(game.auto_recast_of(player), vec![id]);
}

#[test]
fn an_unflagged_routine_fades_as_before() {
    let mut game = game();
    pin_player_combat(&mut game);
    let player = game.player_entity();
    let id = give_core(&mut game, player, Core::FieldRegen);
    fill(&mut game, player, 100.0);
    run_and_age(&mut game, player, &id, FieldRoutineTarget::Ally(player));
    let before = reserve(&game, player);

    game.tick_field_buffs();

    assert!(buffs(&game, player).is_empty());
    assert_eq!(reserve(&game, player), before);
    assert!(log_has(&game, "fades."));
    assert!(!log_has(&game, "reruns"));
}

// The decide step on its own.

fn bodies() -> (Entity, Entity, Entity) {
    let mut world = World::new();
    (
        world.spawn_empty().id(),
        world.spawn_empty().id(),
        world.spawn_empty().id(),
    )
}

fn caster(holder: Entity, target: AbilityTarget) -> Option<Caster> {
    Some(Caster {
        holder,
        ability: "r".to_string(),
        target,
    })
}

fn expiry(recipient: Entity, caster: Option<Caster>) -> Expiry {
    Expiry {
        recipient,
        name: "R".to_string(),
        caster,
    }
}

#[test]
fn plan_groups_a_whole_party_expiry_into_one_recast_with_no_pick() {
    let (holder, a, b) = bodies();
    let plans = plan_recasts(&[
        expiry(a, caster(holder, AbilityTarget::WholeParty)),
        expiry(b, caster(holder, AbilityTarget::WholeParty)),
    ]);
    assert_eq!(
        plans,
        vec![Plan::Recast {
            holder,
            ability: "r".to_string(),
            name: "R".to_string(),
            pick: FieldRoutineTarget::None,
        }]
    );
}

#[test]
fn plan_recasts_each_one_ally_recipient_on_that_ally() {
    let (holder, a, b) = bodies();
    let plans = plan_recasts(&[
        expiry(a, caster(holder, AbilityTarget::OneAlly)),
        expiry(b, caster(holder, AbilityTarget::OneAlly)),
    ]);
    let picks: Vec<_> = plans
        .iter()
        .map(|p| match p {
            Plan::Recast { pick, .. } => *pick,
            Plan::Fade(_) => panic!("both have a caster"),
        })
        .collect();
    assert_eq!(
        picks,
        vec![FieldRoutineTarget::Ally(a), FieldRoutineTarget::Ally(b)]
    );
}

#[test]
fn plan_fades_an_expiry_with_no_caster() {
    let (_, a, _) = bodies();
    assert_eq!(
        plan_recasts(&[expiry(a, None)]),
        vec![Plan::Fade("R".to_string())]
    );
}

#[test]
fn a_rest_clears_a_flagged_timed_buff_and_nothing_reruns_it() {
    let mut game = game();
    pin_player_combat(&mut game);
    stand_in_base_beside_home(&mut game);
    let player = game.player_entity();
    let id = give_core(&mut game, player, Core::FieldRegen);
    game.set_auto_recast(player, &id, true);
    fill(&mut game, player, 100.0);
    run_and_age(&mut game, player, &id, FieldRoutineTarget::Ally(player));

    game.rest().unwrap();
    assert!(
        buffs(&game, player).is_empty(),
        "rest ends a timed buff too"
    );
    assert!(log_has(&game, "fades"));

    game.tick_field_buffs();
    assert!(buffs(&game, player).is_empty());
    assert!(!log_has(&game, "reruns"));
    assert_eq!(game.auto_recast_of(player), vec![id], "the flag stays on");
}
