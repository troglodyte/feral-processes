use super::*;

/// The round's upkeep can kill, and what it killed has to be answered for
/// *before* the world tick — the order `battle_resolve_round` keeps. Ticked
/// first, `difficulty::death_handling_system` reboots a Forgiving player
/// inside a fight that is still open: they read as alive again, the fight
/// never ends, and their world `Position` has been warped to the anchor
/// while they stand on the board.
#[test]
fn a_bleed_that_kills_the_player_at_a_round_boundary_ends_the_fight() {
    let mut game = game();
    tactical_fight(&mut game, 1, 400);
    let player = game.player_entity();
    game.world.get_mut::<Stats>(player).unwrap().hp = 1;
    game.world
        .get_mut::<crate::components::StatusEffects>(player)
        .unwrap()
        .active = vec![crate::components::ActiveStatus {
        id: crate::statuses::StatusId::from("bleed"),
        remaining: 4,
        power: 20,
        stacks: 1,
        landed_this_round: false,
    }];

    let round = game.tactical_view().expect("the fight closed").round;
    for _ in 0..64 {
        if game.tactical_actor().is_none() {
            break;
        }
        if game.tactical_view().is_some_and(|v| v.round > round) {
            break;
        }
        game.tactical_end_turn();
    }

    assert!(
        game.world.get_resource::<TacticalBattle>().is_none(),
        "the bleed took the player down and the fight stayed open around them"
    );
}

/// `TacticalBattle::remove` wraps the order itself, so a body that dies on
/// the *last* rung starts the next round without `end_turn` ever being
/// called. Read off `end_turn` alone, that round's upkeep is skipped: no
/// cooldowns come down, no status ticks, and the world clock stands still
/// for a round nobody can see went by.
#[test]
fn a_round_begun_by_a_death_on_the_last_rung_still_costs_its_upkeep() {
    let mut game = game();
    let actor = crate::tests::support::spawn_tamed(&mut game, 40, 3);
    crate::tests::support::enlist(&mut game, actor);
    let pack = tactical_fight(&mut game, 1, 400);
    let player = game.player_entity();
    // Seated by hand: the property is about the *last* rung, and initiative
    // is rolled off speed.
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, pack[0], actor]);
    assert!(wait_for_turn(&mut game, actor));
    only_routine(&mut game, actor, "cascade_overflow");

    let alone = lonely_cell(&game, 3);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(actor, alone)
    );
    game.world.get_mut::<Stats>(actor).unwrap().hp = 1;
    crate::tests::support::force_the_next_attack_to_land(&mut game);
    let tick = game.current_tick();

    assert!(game.tactical_use_routine(0, alone));
    assert!(
        !game.creature_alive(actor),
        "the blast spared its own invoker — this fixture needs a lethal roll"
    );
    assert!(
        game.current_tick() > tick,
        "the death started a new round and its upkeep was never spent"
    );
}

/// A brace has to actually cost the next swing something, or it is a spent
/// turn dressed up as a choice.
///
/// The swing is driven by hand rather than through `tactical_ai_turn`,
/// because the AI's cell pick draws and what is being measured is the
/// mitigation, not the AI's aim. The stream is reseeded *after* the brace
/// and immediately before the swing, so the two runs meet the swing on an
/// identical stream and the only difference between them is the buff —
/// `force_the_next_attack_to_land` cannot do that job here, since
/// `swing_move` rolls the move first and eats the forced roll. The seed is
/// searched for rather than pinned, `first_rng_seed_where`'s rule.
///
/// Seated by hand for the reason the last-rung test is: initiative is
/// rolled off speed, and a player landing on the *last* rung has their
/// brace ticked off by the wrap before anybody swings.
#[test]
fn bracing_reduces_what_the_next_swing_lands() {
    let damage_taken = |brace: bool, stream: u64| {
        let mut game = game();
        let pack = tactical_fight(&mut game, 1, 400);
        let player = game.player_entity();
        let wild = pack[0];
        game.world
            .resource_mut::<TacticalBattle>()
            .set_initiative(vec![player, wild]);

        // Hard enough that a fifth off it is visible in whole points, and
        // survivable so the fight is still open when the HP is read back.
        game.world.get_mut::<Stats>(wild).unwrap().atk = 60;
        {
            let mut stats = game.world.get_mut::<Stats>(player).unwrap();
            stats.max_hp = 4000;
            stats.hp = 4000;
        }
        let beside = {
            let battle = game.world.resource::<TacticalBattle>();
            let (px, py) = battle.cell_of(player).expect("the player is seated");
            (px + 1, py)
        };
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(wild, beside),
            "fixture: the hostile must stand within reach"
        );

        if brace {
            assert!(game.tactical_defend(), "the player may brace on its turn");
        } else {
            game.tactical_end_turn();
        }
        assert_eq!(
            game.tactical_actor(),
            Some(wild),
            "fixture: the hostile must be the one swinging"
        );

        crate::tests::support::reseed_rng(&mut game, stream);
        let before = game.world.get::<Stats>(player).unwrap().hp;
        assert!(game.tactical_attack(player), "the hostile must reach");
        before - game.world.get::<Stats>(player).unwrap().hp
    };

    // A stream on which the swing lands enough for a fifth of it to be a
    // whole point at all — every matchup has a miss chance by design.
    let stream = (0..512u64)
        .find(|&s| damage_taken(false, s) >= 5)
        .expect("no stream in 0..512 landed the swing");
    let open = damage_taken(false, stream);
    let braced = damage_taken(true, stream);
    assert!(
        braced < open,
        "bracing cost the swing nothing: {braced} against {open}"
    );
}

/// Bracing is an action, and an action ends the turn — the same rule
/// `tactical_attack` and `tactical_use_routine` hold.
#[test]
fn bracing_ends_the_turn() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, pack[0]]);

    assert!(game.tactical_defend());
    assert_eq!(
        game.tactical_actor(),
        Some(pack[0]),
        "a brace that does not hand the turn on lets a body brace for ever"
    );
}

/// A body that has already spent its action may not brace on top of it.
#[test]
fn a_body_that_has_acted_may_not_brace() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, pack[0]]);
    let raw = game.effective_mitigation(player);
    game.world.resource_mut::<TacticalBattle>().spend_action();

    assert!(!game.tactical_defend(), "an acted body braced anyway");
    assert_eq!(
        game.effective_mitigation(player),
        raw,
        "the refusal armed the buff on its way out"
    );
}

/// The brace lasts the rest of the round and no longer: it is armed for one
/// round and the order coming back round is what spends that.
#[test]
fn a_brace_is_gone_once_the_order_comes_round() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 400);
    let player = game.player_entity();
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, pack[0]]);
    let raw = game.effective_mitigation(player);

    assert!(game.tactical_defend());
    assert_eq!(
        game.effective_mitigation(player),
        raw + crate::tuning::DEFEND_MITIGATION_BONUS,
        "the brace never took"
    );

    // The last rung's turn ending is what wraps the round, and the wrap is
    // what spends the upkeep the buff ages under.
    game.tactical_end_turn();
    assert_eq!(
        game.effective_mitigation(player),
        raw,
        "the brace outlived the round it was armed for"
    );
}

/// The brace's one accepted cost, pinned rather than reasoned about: a body
/// on the **last** rung braces against nobody, because `hand_on_turn`'s wrap
/// fires the moment it hands the turn on and the wrap is what ages the buff.
///
/// Stated in `CLAUDE.md`, in the `seams` skill and in
/// `seam:a-brace-on-a-board-is-the-group-models-and-only-the`, so it is a
/// claim that has to be checked rather than remembered — and it is the thing
/// that would silently become false if the brace were ever rearmed for longer.
#[test]
fn a_body_on_the_last_rung_braces_against_nobody() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 400);
    let player = game.player_entity();
    // The player last, where the wrap lands directly behind its turn.
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![pack[0], player]);
    assert!(wait_for_turn(&mut game, player));
    let raw = game.effective_mitigation(player);

    assert!(game.tactical_defend());

    assert_eq!(
        game.effective_mitigation(player),
        raw,
        "the last rung's brace outlived the wrap that follows it"
    );
}

/// A hostile's approach is walked, not teleported: every beat that steps
/// moves it exactly one cell, and it takes as many beats as the walk is
/// long.
///
/// **The regression this whole seam exists for.** Committed as one
/// placement, a hostile crossed its entire allowance between two frames and
/// read as a teleport.
#[test]
fn a_hostiles_walk_is_spent_one_cell_a_beat() {
    use crate::tactical::ai::AiBeat;
    use crate::tactical::reach::distance;

    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 200);
    let wild = pack[0];
    assert!(wait_for_turn(&mut game, wild), "the hostile never acted");

    let mut at = cell_of(&game, wild).expect("the hostile is on the board");
    let mut steps = 0;
    loop {
        match game.tactical_ai_beat() {
            AiBeat::Stepped => {
                let now = cell_of(&game, wild).expect("the walker left the board");
                assert_eq!(distance(at, now), 1, "a beat crossed more than one cell");
                at = now;
                steps += 1;
                assert!(steps <= TACTICAL_MOVE_MAX, "the walk never ended");
            }
            AiBeat::Acted => break,
            AiBeat::Idle => panic!("the hostile's turn was not the AI's to drive"),
        }
    }
    assert!(
        steps > 1,
        "the fixture never made the hostile walk, so nothing was tested"
    );
}

/// The walk is planned on the beat that takes its first step and read back
/// by every beat after it — **one `GameRng` draw a turn, not one a cell**.
///
/// The case that holds it is a walk already spent: `Some(vec![])` is a body
/// that has arrived and `None` is one that has not chosen yet, and a beat
/// that read the two as one would plan a fresh approach — and draw again —
/// every beat for the rest of the turn instead of acting.
#[test]
fn a_spent_walk_acts_rather_than_planning_a_fresh_one() {
    use crate::tactical::ai::AiBeat;

    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 200);
    let wild = pack[0];
    assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
    assert_eq!(
        game.tactical_ai_beat(),
        AiBeat::Stepped,
        "the hostile had no approach to walk"
    );

    // Arrived: the plan is made and there is nothing left of it.
    game.world
        .resource_mut::<TacticalBattle>()
        .commit_walk(Vec::new());
    let stood = cell_of(&game, wild).expect("the walker left the board");

    assert_eq!(
        game.tactical_ai_beat(),
        AiBeat::Acted,
        "a spent walk was re-planned instead of acted on"
    );
    assert_eq!(
        cell_of(&game, wild).or(Some(stood)),
        Some(stood),
        "the body walked again on a turn it had already finished walking"
    );
}

/// The two granularities are one turn. A fight watched a beat at a time and
/// the same fight resolved in a single call have to reach the same board, or
/// the arena is measuring a different game from the one being played.
#[test]
fn a_turn_and_the_beats_it_is_made_of_reach_the_same_board() {
    use crate::tactical::ai::AiBeat;

    let mut whole = game();
    let mut paced = game();
    let wild_whole = tactical_fight(&mut whole, 1, 200)[0];
    let wild_paced = tactical_fight(&mut paced, 1, 200)[0];
    assert!(wait_for_turn(&mut whole, wild_whole));
    assert!(wait_for_turn(&mut paced, wild_paced));

    assert!(whole.tactical_ai_turn(), "the whole turn was not driven");
    for _ in 0..=TACTICAL_MOVE_MAX {
        if paced.tactical_ai_beat() == AiBeat::Acted {
            break;
        }
    }

    assert_eq!(
        cell_of(&whole, wild_whole),
        cell_of(&paced, wild_paced),
        "the paced fight ended the turn somewhere else"
    );
    let player = whole.player_entity();
    assert_eq!(
        hp_of(&whole, player),
        hp_of(&paced, paced.player_entity()),
        "the paced fight landed a different blow"
    );
}

/// What a driver reads to know it owes a step rather than a turn. A body
/// that has not set off yet is not walking — which is what buys the beat of
/// anticipation before it does — and one that has arrived is not either.
#[test]
fn a_body_is_walking_only_between_its_first_step_and_its_last() {
    use crate::tactical::ai::AiBeat;

    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 200);
    let wild = pack[0];
    assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
    assert!(
        !game.tactical_walking(),
        "a body that has chosen nothing yet was called mid-walk"
    );

    assert_eq!(game.tactical_ai_beat(), AiBeat::Stepped);
    assert!(
        game.tactical_walking(),
        "a body mid-approach reads as still"
    );

    for _ in 0..=TACTICAL_MOVE_MAX {
        if game.tactical_ai_beat() == AiBeat::Acted {
            break;
        }
    }
    assert!(
        !game.tactical_walking(),
        "the turn ended with a walk still owed"
    );
}

/// The player unarmed swings at arm's length, and nothing else.
#[test]
fn an_unarmed_body_swings_at_arms_length() {
    let game = game();
    let player = game.player_entity();
    assert_eq!(
        game.swing_range(player),
        crate::tuning::TACTICAL_MELEE_RANGE
    );
}

/// A species carrying a `ranged` move reaches past arm's length.
#[test]
fn a_ranged_species_reaches_past_arms_length() {
    let mut game = game();
    let shooter = body(&mut game, "drone");
    assert_eq!(
        game.swing_range(shooter),
        crate::tuning::TACTICAL_RANGED_MOVE_RANGE,
        "Drone's Recon Ping is `ranged: true`"
    );
}

/// A species with no ranged move stays at arm's length.
#[test]
fn a_melee_species_stays_at_arms_length() {
    let mut game = game();
    let bruiser = body(&mut game, "construct");
    assert_eq!(
        game.swing_range(bruiser),
        crate::tuning::TACTICAL_MELEE_RANGE,
        "Construct authors no ranged move"
    );
}

/// A worn weapon **replaces** the species figure rather than being maxed
/// against it — `Game::attack_range`'s precedent, where worn damage replaces
/// a natural band outright. A ranged program holding a melee blade swings at
/// arm's length, because the weapon is what it is swinging.
#[test]
fn a_worn_weapon_replaces_the_species_range() {
    let mut game = game();
    let shooter = body(&mut game, "drone");
    equip_weapon(&mut game, shooter, "shim_blade");
    assert_eq!(
        game.swing_range(shooter),
        crate::tuning::TACTICAL_MELEE_RANGE
    );

    equip_weapon(&mut game, shooter, "plasma_router");
    assert_eq!(game.swing_range(shooter), 3);
}

/// The range number the board floats over a body is `Game::swing_range`'s
/// own answer, armed and unarmed.
#[test]
fn a_bodys_view_carries_its_swing_range() {
    let mut game = game();
    let wild = tactical_fight(&mut game, 1, 200)[0];
    let player = game.player_entity();
    equip_weapon(&mut game, player, "plasma_router");
    let view = game.tactical_view().expect("the fight is open");
    let range_of = |entity| {
        view.bodies
            .iter()
            .find(|b| b.entity == entity)
            .expect("the body is on the board")
            .swing_range
    };
    assert_eq!(range_of(player), 3, "the worn router reaches three");
    assert_eq!(range_of(wild), game.swing_range(wild));
}

/// A swing at exactly the weapon's range lands. Its other half is below, and
/// the two are one pair — at the bound and one past it.
#[test]
fn a_swing_lands_at_exactly_its_range() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "plasma_router"); // range 3
    place_bodies(&mut game, player, (0, 0), pack[0], (3, 0));
    assert!(
        game.tactical_attack(pack[0]),
        "three cells is inside range 3"
    );
}

/// One cell past the range is refused.
#[test]
fn a_swing_one_cell_past_its_range_is_refused() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "plasma_router"); // range 3
    place_bodies(&mut game, player, (0, 0), pack[0], (4, 0));
    assert!(!game.tactical_attack(pack[0]), "four cells is past range 3");
}

/// Cover blocks a swing. Delete the `line_of_sight` check and this must
/// fail — a test that passes with the fix removed is not coverage.
#[test]
fn a_ranged_swing_is_blocked_by_cover() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "plasma_router");
    place_bodies(&mut game, player, (0, 0), pack[0], (2, 0));
    block_cell(&mut game, (1, 0));
    assert!(
        !game.tactical_attack(pack[0]),
        "cover did not stop the swing"
    );
}

/// The same swing with the cover gone lands — so the test above is measuring
/// the cover and not the placement.
#[test]
fn the_same_swing_lands_once_the_cover_is_gone() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "plasma_router");
    place_bodies(&mut game, player, (0, 0), pack[0], (2, 0));
    assert!(game.tactical_attack(pack[0]));
}

/// An adjacent swing is untouched by the sight check — `line_of_sight`
/// excludes its endpoints, so for neighbours its loop is empty.
#[test]
fn an_adjacent_swing_is_unaffected_by_the_sight_check() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    place_bodies(&mut game, player, (0, 0), pack[0], (1, 0));
    assert!(game.tactical_attack(pack[0]));
}

/// A sweep fired from range still sweeps. The shape is cast from the actor
/// toward the aim whatever the distance, so a reach weapon that quietly went
/// single-target at range would read as a nerf rather than a bug.
#[test]
fn a_reach_weapon_still_sweeps_when_fired_from_range() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 2);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "scatter_lance"); // range 2, WholeEnemyGroup
    place_bodies(&mut game, player, (0, 0), pack[0], (2, 0));
    place_one(&mut game, pack[1], (2, 1));
    let before = hp_of(&game, pack[1]);
    crate::tests::support::force_the_next_attack_to_land(&mut game);
    assert!(game.tactical_attack(pack[0]));
    assert!(
        hp_of(&game, pack[1]) < before,
        "the neighbour was not caught by a sweep fired from two cells"
    );
}

/// A body standing at two cells fires the ranged half of its pair, never the
/// melee half — the AI decides its intent before it walks, and a roll that
/// could draw a move it cannot fire makes a planned standoff a coin flip.
#[test]
fn a_body_at_range_rolls_only_a_move_that_reaches() {
    let mut game = game();
    let shooter = body(&mut game, "drone");
    for _ in 0..20 {
        let rolled = game
            .roll_species_move_in_range(shooter, Some(2))
            .expect("Drone has moves");
        assert!(rolled.ranged, "rolled {} at two cells", rolled.name);
    }
}

/// Adjacent, either half of the pair is fair game — which is the variety
/// `swing_move` exists for.
#[test]
fn a_body_adjacent_may_roll_either_move() {
    let mut game = game();
    let shooter = body(&mut game, "drone");
    let mut saw_melee = false;
    for _ in 0..40 {
        let rolled = game
            .roll_species_move_in_range(shooter, Some(1))
            .expect("Drone has moves");
        saw_melee |= !rolled.ranged;
    }
    assert!(saw_melee, "the melee half was never drawn in forty rolls");
}

/// The group model passes `None` and is untouched.
#[test]
fn the_group_model_rolls_over_every_move() {
    let mut game = game();
    let shooter = body(&mut game, "drone");
    let mut saw_melee = false;
    let mut saw_ranged = false;
    for _ in 0..40 {
        let rolled = game.roll_species_move(shooter).expect("Drone has moves");
        saw_melee |= !rolled.ranged;
        saw_ranged |= rolled.ranged;
    }
    assert!(saw_melee && saw_ranged, "the unconstrained roll narrowed");
}

/// A hostile of `species` standing beside the player, with a fight opened
/// around it. `tactical_pack` takes the *first* species in the db, so a test
/// that needs a particular one spawns its own.
pub(super) fn fight_against(game: &mut Game, species: &str) -> Entity {
    let player = game.player_entity();
    let at = *game
        .world
        .get::<Position>(player)
        .expect("the player stands somewhere");
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.to_string(),
            },
            Hostile,
            Position {
                x: at.x + 1,
                y: at.y,
            },
            Stats {
                hp: 20,
                max_hp: 20,
                atk: 4,
                mitigation: 0,
            },
            StatusEffects::default(),
        ))
        .id();
    game.open_tactical_battle(vec![wild]);
    // Pinned: these tests are not about the pack, and the roll is seed luck.
    game.set_pack_tactic(crate::tactical::pack::PackTactic::Rush);
    wild
}

/// A reaching hostile swings from where it stands instead of closing, and
/// the move it swings with is the reaching half of its pair.
#[test]
fn a_reaching_hostile_swings_without_closing() {
    let mut game = game();
    let wild = fight_against(&mut game, "drone");
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
    place_bodies(&mut game, player, (0, 0), wild, (2, 0));
    {
        let battle = &mut *game.world.resource_mut::<TacticalBattle>();
        for x in 0..6 {
            for y in 0..4 {
                battle
                    .board
                    .put(x, y, crate::tactical::map::BattleCell::Open);
            }
        }
    }

    assert!(game.tactical_ai_turn(), "the hostile's turn was not run");
    // The *distance* and not the cell: holding the band is what is being
    // asserted, and a body that sidesteps to another cell two out has held
    // it exactly as well as one that stood still.
    let held = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(wild)
        .expect("the hostile left the board");
    assert_eq!(
        crate::tactical::reach::distance((0, 0), held),
        crate::tuning::TACTICAL_RANGED_MOVE_RANGE,
        "the reaching hostile closed instead of holding its band, ending at {held:?}"
    );
    assert!(
        log_texts(&game).iter().any(|l| l.contains("Recon Ping")),
        "the reaching hostile never swung its reaching move: {:?}",
        log_texts(&game)
    );
}

/// One streak per body actually swung at, from the swinger's cell to each
/// recipient's — so a reach weapon's sweep fires one at every body its shape
/// caught.
#[test]
fn a_swing_queues_one_bolt_per_body_it_lands_on() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "plasma_router");
    place_bodies(&mut game, player, (0, 0), pack[0], (3, 0));
    assert!(game.tactical_attack(pack[0]));

    let bolts = game.take_bolts();
    assert_eq!(bolts.len(), 1, "one body swung at, one streak");
    assert_eq!(bolts[0].from, (0, 0));
    assert_eq!(bolts[0].to, (3, 0));
}

/// A swing's cue names the wielded weapon's effect, and an unarmed swing
/// names none — the renderer's default is the renderer's to pick.
#[test]
fn a_swing_cue_carries_the_wielded_weapons_fx() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    let mut def = game
        .world
        .resource::<crate::items_db::ItemDb>()
        .get("plasma_router")
        .expect("the shipped weapon is loaded")
        .clone();
    def.fx = Some("x".to_string());
    game.world
        .resource_mut::<crate::items_db::ItemDb>()
        .insert(def);
    equip_weapon(&mut game, player, "plasma_router");
    place_bodies(&mut game, player, (0, 0), pack[0], (3, 0));
    assert!(game.tactical_attack(pack[0]));
    let bolts = game.take_bolts();
    assert_eq!(bolts.len(), 1);
    assert_eq!(bolts[0].fx.as_deref(), Some("x"));

    // Unarmed: a fresh fight with nothing in the weapon slot.
    let mut game = self::game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    place_bodies(&mut game, player, (0, 0), pack[0], (1, 0));
    assert!(game.tactical_attack(pack[0]));
    let bolts = game.take_bolts();
    assert_eq!(bolts.len(), 1);
    assert_eq!(bolts[0].fx, None);
}

/// A routine on a battle map queues exactly one cue, carrying the shape's
/// cells and the routine's own effect, before it resolves.
#[test]
fn a_tactical_routine_queues_one_cue_with_its_shape_cells() {
    use crate::abilities::{AbilityDb, AbilityRange, AbilityShape};

    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    let shape = AbilityShape::Radius { radius: 1 };
    let id = shaped_sweep(&mut game, player, shape, AbilityRange { min: 0, max: 6 });
    let mut def = game.world.resource::<AbilityDb>().get(id).unwrap().clone();
    def.fx = Some("boom".to_string());
    game.world.resource_mut::<AbilityDb>().insert(def);
    place_bodies(&mut game, player, (0, 0), pack[0], (3, 0));
    game.take_routine_cues();
    let expected = crate::tactical::reach::shape_cells(
        &game.world.resource::<TacticalBattle>().board,
        (0, 0),
        (3, 0),
        shape,
    );

    assert!(game.tactical_use_routine(0, (3, 0)));

    let cues = game.take_routine_cues();
    assert_eq!(cues.len(), 1, "one cast, one cue: {cues:?}");
    assert_eq!(cues[0].cells, expected);
    assert_eq!(cues[0].from, (0, 0));
    assert_eq!(cues[0].aim, (3, 0));
    assert_eq!(cues[0].fx.as_deref(), Some("boom"));
}

/// A sweep fires one at every body its shape caught, not one at the aim.
#[test]
fn a_sweep_queues_a_bolt_per_body_it_swept() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 2);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "scatter_lance");
    place_bodies(&mut game, player, (0, 0), pack[0], (2, 0));
    place_one(&mut game, pack[1], (2, 1));
    assert!(game.tactical_attack(pack[0]));

    let bolts = game.take_bolts();
    assert_eq!(bolts.len(), 2, "two bodies swept, two streaks: {bolts:?}");
    assert!(bolts.iter().all(|b| b.from == (0, 0)));
}

/// A swing on a battle map is heard off its own queue, so a fight long enough
/// to push the log past `MESSAGE_LOG_CAP` still sounds. The log is filled
/// first because that is the fight that went silent: sound read the round's
/// range by position, and a range pinned at the cap never grew.
#[test]
fn a_swing_is_heard_however_full_the_log_is() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    place_bodies(&mut game, player, (0, 0), pack[0], (1, 0));
    for i in 0..2 * crate::MESSAGE_LOG_CAP {
        game.log(format!("filler {i}"));
    }
    game.take_swing_cues();

    assert!(game.tactical_attack(pack[0]));

    let swung = game
        .message_log(crate::MESSAGE_LOG_CAP)
        .iter()
        .filter_map(|line| line.outcome)
        .collect::<Vec<_>>();
    assert_eq!(swung.len(), 1, "one swing logged");
    assert_eq!(game.take_swing_cues(), swung);
    assert!(game.take_swing_cues().is_empty(), "a drain is a drain");
}

/// The group model is heard through the reveal, which paces its cues to the
/// narration, so a swing off the board must not queue a second copy.
#[test]
fn a_swing_off_the_board_queues_no_cue() {
    let mut game = game();
    let player = game.player_entity();
    game.log_swing(
        crate::resources::MessageKind::PartyDamage,
        crate::battle::AttackOutcome::Crit {
            dmg: 12,
            resisted: None,
        },
        player,
        "you tear it clean through",
    );
    assert!(game.take_swing_cues().is_empty());
}

/// Draining is a drain — a second call comes back empty, so a frontend
/// cannot draw one streak twice.
#[test]
fn taking_the_bolts_empties_the_queue() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    place_bodies(&mut game, player, (0, 0), pack[0], (1, 0));
    assert!(game.tactical_attack(pack[0]));
    assert_eq!(game.take_bolts().len(), 1);
    assert!(game.take_bolts().is_empty());
}

/// An adjacent swing queues one too — the streak is one rule at every
/// distance, and at one cell it is the melee feedback.
#[test]
fn an_adjacent_swing_queues_a_bolt_too() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    place_bodies(&mut game, player, (0, 0), pack[0], (1, 0));
    assert!(game.tactical_attack(pack[0]));
    assert_eq!(game.take_bolts().len(), 1);
}

// --- A body's own hit or heal on a battle map --------------------------
//
// `Game::apply_damage` and `Game::restore_hp` are the two doors, so the
// engine side of the whole feature is a queue push inside each — see
// `TacticalFxQueue`. The stream is searched for rather than pinned,
// `bracing_reduces_what_the_next_swing_lands`'s rule: a swing has a miss
// chance by design, so a fixed seed would read as flaky the day the combat
// tables move.

/// A landed blow cues a hit at the defender's own board cell — the same
/// door `apply_damage` always damages a creature through, reached here by
/// an ordinary melee swing.
#[test]
fn a_landed_swing_queues_a_hit_cue_at_the_defenders_cell() {
    fn attempt(seed: u64) -> (Game, Entity) {
        let mut game = game();
        let pack = ranged_fight(&mut game, 1);
        let player = game.player_entity();
        let target = pack[0];
        place_bodies(&mut game, player, (0, 0), target, (1, 0));
        crate::tests::support::reseed_rng(&mut game, seed);
        game.tactical_attack(target);
        (game, target)
    }

    let (mut game, _target) = (0..512u64)
        .map(attempt)
        .find(|(game, target)| hp_of(game, *target) < 20)
        .expect("no stream in 0..512 landed the swing");

    // The cell `place_bodies` seated the defender on, read back rather than
    // re-derived from `TacticalBattle::cell_of` after the swing: a hard
    // enough blow reaps the defender off the board entirely, and the cue's
    // own cell was captured before that could happen.
    let cues = game.take_tactical_fx();
    assert!(
        matches!(
            cues.as_slice(),
            [crate::resources::TacticalFxCue {
                pos: (1, 0),
                kind: crate::resources::TacticalFxKind::Hit(dealt),
            }] if *dealt > 0
        ),
        "a landed blow must cue exactly one hit at the defender's cell: {cues:?}"
    );
}

/// A miss or a zero-damage call cues nothing — `apply_damage`'s own gate on
/// `dealt > 0`, exercised directly so the case does not depend on finding an
/// unlucky stream.
#[test]
fn a_hit_with_nothing_dealt_cues_no_tactical_fx() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 20);
    let target = pack[0];
    assert_eq!(
        game.apply_damage(target, 0),
        0,
        "fixture: zero damage must land as zero"
    );
    assert!(
        game.take_tactical_fx().is_empty(),
        "zero damage must not have cued a hit"
    );
}

/// A heal on a body standing on the board cues a `+` at its cell — full
/// friendly fire's own fixture, since `mirror_restore` is what proved
/// `restore_hp` reaches a hostile standing inside a player-centred patch.
#[test]
fn a_heal_queues_a_heal_cue_at_the_recipients_cell() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    only_routine(&mut game, player, "mirror_restore");
    assert!(wait_for_turn(&mut game, player));

    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .expect("the player was not seated");
    let beside = free_neighbour(&game, at);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(pack[0], beside)
    );
    game.world.get_mut::<Stats>(pack[0]).unwrap().hp = 10;

    assert!(
        game.tactical_use_routine(0, at),
        "a patch aimed at the caster's own cell was refused"
    );

    let cues = game.take_tactical_fx();
    assert!(
        cues.iter().any(|c| c.pos == beside
            && matches!(c.kind, crate::resources::TacticalFxKind::Heal(n) if n > 0)),
        "the healed hostile's cell must carry a heal cue: {cues:?}"
    );
}

/// A heal that restores nothing — a full-health recipient — cues nothing,
/// `restore_hp`'s own gate on `restored > 0`.
#[test]
fn a_heal_that_restores_nothing_cues_no_tactical_fx() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let target = pack[0];
    assert_eq!(
        game.restore_hp(target, 10),
        0,
        "fixture: a full-health target must have nothing to restore"
    );
    assert!(
        game.take_tactical_fx().is_empty(),
        "a no-op heal must not have cued anything"
    );
}

/// A hit cue carries what came off after mitigation, and a heal cue what
/// landed under the cap — the figures the board floats over the body, so a
/// number printing the *requested* amount would claim damage never taken.
#[test]
fn a_cue_carries_the_landed_amount_not_the_requested_one() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let target = pack[0];
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(target)
        .expect("the hostile was not seated");
    game.world.get_mut::<Stats>(target).unwrap().mitigation = 50;

    let dealt = game.apply_damage(target, 10);
    assert!(
        (1..10).contains(&dealt),
        "fixture: mitigation must cut the blow without zeroing it, got {dealt}"
    );
    let max = game.world.get::<Stats>(target).unwrap().max_hp;
    game.world.get_mut::<Stats>(target).unwrap().hp = max - 3;
    assert_eq!(game.restore_hp(target, 10), 3, "fixture: the cap must bite");

    assert_eq!(
        game.take_tactical_fx(),
        vec![
            crate::resources::TacticalFxCue {
                pos: at,
                kind: crate::resources::TacticalFxKind::Hit(dealt),
            },
            crate::resources::TacticalFxCue {
                pos: at,
                kind: crate::resources::TacticalFxKind::Heal(3),
            },
        ]
    );
}
