//! The turn loop: ticking, resting, waiting, and consuming items.

use super::support::*;
use crate::*;

#[test]
fn player_status_power_matches_max_hp_plus_atk_plus_def() {
    let game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let status = game.player_status();
    assert_eq!(
        status.strength,
        status.max_hp + status.atk + status.mitigation
    );
}

/// The map's Integrity gauge and the battle screen's "You" bar are two
/// readouts of one number. Nothing may fork them — not the entity they
/// resolve, not a buff, not a stale view.
#[test]
fn battle_view_integrity_matches_the_map_status_panel() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = start_battle_with_a_wild_program(&mut game);
    let player = game.player_entity();
    assert_eq!(
        game.world.resource::<BattleState>().player,
        player,
        "the battle must be fought by the entity the map panel reads"
    );

    // Outlast the pack without killing it: a fight that ends mid-loop
    // would drop the battle view and stop comparing.
    {
        let mut w = game.world.get_mut::<Stats>(wild).unwrap();
        w.hp = 10_000;
        w.max_hp = 10_000;
        w.atk = 50;
    }
    {
        let mut p = game.world.get_mut::<Stats>(player).unwrap();
        p.hp = 5_000;
        p.max_hp = 5_000;
    }

    let start_hp = game.player_status().hp;
    for round in 0..10 {
        player_attacks(&mut game);
        let status = game.player_status();
        let view = game
            .battle_view()
            .unwrap_or_else(|| panic!("battle ended early at round {round}"));
        let player_row = &view.party[0];
        assert_eq!(player_row.hp, status.hp, "hp diverged at round {round}");
        assert_eq!(
            player_row.max_hp, status.max_hp,
            "max_hp diverged at round {round}"
        );
    }
    assert!(
        game.player_status().hp < start_hp,
        "the wild program never landed a hit, so the comparison proved nothing"
    );
}

#[test]
fn wait_advances_one_tick_without_moving() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let pos_before = *game.world.get::<Position>(player).unwrap();
    let tick_before = game.world.resource::<GameClock>().tick;

    game.wait();

    let pos_after = *game.world.get::<Position>(player).unwrap();
    let tick_after = game.world.resource::<GameClock>().tick;
    assert_eq!(pos_after, pos_before, "waiting shouldn't move the player");
    assert_eq!(
        tick_after,
        tick_before + 1,
        "waiting should advance exactly one tick"
    );
}

#[test]
fn current_tick_matches_the_internal_game_clock() {
    let mut game = Game::new(35, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(
        game.current_tick(),
        0,
        "a fresh game should start at tick 0"
    );

    game.wait();
    game.wait();

    assert_eq!(
        game.current_tick(),
        2,
        "current_tick should track GameClock exactly"
    );
}

#[test]
fn idle_tick_advances_the_clock_outside_battle_but_not_during_one() {
    let mut game = Game::new(35, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    game.idle_tick();
    assert_eq!(
        game.current_tick(),
        1,
        "idle_tick should advance the clock with no battle active"
    );

    let player = game.player_entity();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    insert_battle(&mut game, player, vec![wild]);
    game.idle_tick();
    assert_eq!(
        game.current_tick(),
        1,
        "idle_tick should be a no-op while a battle is active"
    );
}

/// With Fatigue gone, rest's meter refill lands on Power. That is a real
/// behaviour change — rest used to leave a drained player drained — and it is
/// what makes the base the place a running budget is bought back.
#[test]
fn rest_fully_heals_and_restores_power() {
    let mut game = Game::new(18, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.hp = 1;
    }
    {
        let mut needs = game.world.get_mut::<PowerReserve>(player).unwrap();
        *needs = PowerReserve::new(10.0);
    }
    stand_in_base_beside_home(&mut game);

    game.rest().unwrap();

    let stats = *game.world.get::<Stats>(player).unwrap();
    let needs = *game.world.get::<PowerReserve>(player).unwrap();
    assert_eq!(stats.hp, stats.max_hp, "rest should fully heal Integrity");
    assert_eq!(needs.get(), 100.0, "rest should fully restore Power");
}

/// No rest can be interrupted, because no rest advances the clock — there
/// are no ticks for `nest_aggro_tick` to open a battle on.
///
/// Kept pointed at the hardest case rather than retired with the tick loop:
/// a provoked guardian standing on the party's own pinned tile, which is
/// where an interruption would have to come from if one could. Resting was
/// interruptible on the slab's outer ring once, and then unreachable-by-
/// construction while the base was out of phase; it is now neither, and this
/// says so rather than leaving it to be rediscovered.
#[test]
fn a_pursuer_beside_the_anchor_cannot_interrupt_a_rest() {
    let mut game = Game::new(741, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ppos = *game.world.get::<Position>(player).unwrap();
    stand_in_base_beside_home(&mut game);

    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.hp = 1;
    }
    {
        let mut needs = game.world.get_mut::<PowerReserve>(player).unwrap();
        *needs = PowerReserve::new(10.0);
    }

    // Standing on the anchor's own doorstep, which is where the player's
    // `Position` is pinned the whole time they are inside.
    let nest = spawn_bare_nest(&mut game, ppos.x + 1, ppos.y);
    spawn_pursuing_guardian(&mut game, nest, "scrapper", ppos.x + 1, ppos.y);

    game.rest().unwrap();

    assert!(
        !game.has_active_battle(),
        "a pursuer on the anchor tile must not reach a party out of phase"
    );
    let stats = *game.world.get::<Stats>(player).unwrap();
    assert_eq!(
        stats.hp, stats.max_hp,
        "and the rest runs to its heal, since nothing interrupted it"
    );
    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        100.0,
        "Power too"
    );
}

#[test]
fn rest_also_fully_heals_the_active_companion() {
    let mut game = Game::new(29, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, companion);
    {
        let mut stats = game.world.get_mut::<Stats>(companion).unwrap();
        stats.hp = 1;
    }
    stand_in_base_beside_home(&mut game);

    game.rest().unwrap();

    let stats = *game.world.get::<Stats>(companion).unwrap();
    assert_eq!(
        stats.hp, stats.max_hp,
        "rest should fully heal the active companion too"
    );
}

#[test]
fn rest_heals_every_party_member() {
    let mut game = Game::new(74, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = spawn_tamed(&mut game, 10, 3);
    let b = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, a);
    enlist(&mut game, b);
    for e in [a, b] {
        game.world.get_mut::<Stats>(e).unwrap().hp = 1;
    }
    stand_in_base_beside_home(&mut game);

    game.rest().unwrap();

    assert_eq!(game.world.get::<Stats>(a).unwrap().hp, 10);
    assert_eq!(game.world.get::<Stats>(b).unwrap().hp, 10);
}

/// A rest repairs the player and the programs *with* the player. It does not
/// repair the base's labour pool: staff mend at a Repair Bay, which is what
/// that building is for, and a rest that healed them made the Bay optional.
///
/// The role assertion is not decoration. `Staff` is what `program_role`
/// leaves over, so a fixture that quietly enlisted the third program would
/// make the interesting half of this test pass for the wrong reason.
#[test]
fn resting_repairs_the_party_and_leaves_base_staff_to_the_repair_bay() {
    let mut game = Game::new(75, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    let staff = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, companion);
    game.world.get_mut::<Stats>(player).unwrap().hp = 1;
    for e in [companion, staff] {
        game.world.get_mut::<Stats>(e).unwrap().hp = 1;
    }
    *game.world.get_mut::<PowerReserve>(staff).unwrap() = PowerReserve::new(3.0);
    stand_in_base_beside_home(&mut game);
    assert_eq!(
        game.program_role(staff),
        Some(ProgramRole::Staff),
        "the unenlisted program has to actually be staff, or this is vacuous"
    );

    game.rest().unwrap();

    let stats = *game.world.get::<Stats>(player).unwrap();
    assert_eq!(stats.hp, stats.max_hp, "the player is repaired as before");
    assert_eq!(
        game.world.get::<Stats>(companion).unwrap().hp,
        10,
        "and so is a program fighting beside them"
    );
    assert_eq!(
        game.world.get::<Stats>(staff).unwrap().hp,
        1,
        "but the base's labour pool is left for a Repair Bay"
    );
    // Deliberately still refilled: a Bay restores Integrity and nothing
    // else, and nothing refills a reserve passively, so withholding this
    // would strand a staff program that spent Power defending a sweep.
    assert_eq!(
        game.world.get::<PowerReserve>(staff).unwrap().get(),
        100.0,
        "a staff program's Power is still refilled"
    );
}

/// The exclusion is by **role**, not by locale. Before this, the walk was
/// over every `Tamed` program the player owned wherever anybody stood, so a
/// rest four frames underground reached back and repaired a base the party
/// had not seen in an hour.
#[test]
fn a_field_rest_does_not_reach_back_and_repair_base_staff() {
    let mut game = Game::new(76, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let staff = spawn_tamed(&mut game, 10, 3);
    game.world.get_mut::<Stats>(staff).unwrap().hp = 1;
    game.world.get_mut::<Stats>(player).unwrap().hp = 1;
    descend(&mut game);

    game.rest().unwrap();

    let stats = *game.world.get::<Stats>(player).unwrap();
    assert_eq!(
        stats.hp,
        stats.max_hp,
        "the charge repairs the player underground: {:?}",
        game.message_log(5)
    );
    assert_eq!(
        game.world.get::<Stats>(staff).unwrap().hp,
        1,
        "and reaches no further than the party it was spent by"
    );
}

/// A wielded program is repaired. It is outside `Party` but it is carried
/// wherever the player goes, it is never posted at the base, and no Repair
/// Bay can reach it — so excluding it would be the one role with no
/// recovery route at all rather than a role told to use a different one.
#[test]
fn a_wielded_program_is_repaired_by_a_rest() {
    let mut game = Game::new(77, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let weapon = spawn_tamed(&mut game, 10, 3);
    stand_in_base_beside_home(&mut game);
    game.wield_program(weapon).expect("wielding a program");
    game.world.get_mut::<Stats>(weapon).unwrap().hp = 1;
    assert_eq!(
        game.program_role(weapon),
        Some(ProgramRole::Wielded),
        "the fixture has to leave it wielded, or this reads as the party rule"
    );

    game.rest().unwrap();

    assert_eq!(
        game.world.get::<Stats>(weapon).unwrap().hp,
        10,
        "the program in your hands comes back with you"
    );
}

#[test]
fn a_new_game_starts_with_two_power_outlets() {
    let game = Game::new(701, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let held = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::OUTLET));
    assert_eq!(
        held, 2,
        "the bounded-income opening softener is two outlets, beside the \
         3 ICE Breakers / 3 Power Cells / 5 Core Fragments"
    );
}

#[test]
fn rest_refused_by_game_over_consumes_no_outlet() {
    let mut game = Game::new(704, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    stand_in_base_beside_home(&mut game);
    game.world.resource_mut::<GameOver>().reason = Some("test".to_string());

    let refusal = game.rest();
    assert!(
        refusal.is_err(),
        "a rest that does not happen must say why, not fall silent"
    );

    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::OUTLET)),
        2,
        "a rest refused by the game-over gate must spend nothing"
    );
}

#[test]
fn rest_refused_by_active_battle_consumes_no_outlet() {
    let mut game = Game::new(705, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    stand_in_base_beside_home(&mut game);
    start_battle_with_a_wild_program(&mut game);

    let refusal = game.rest();
    assert!(
        refusal.is_err(),
        "a rest that does not happen must say why, not fall silent"
    );

    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::OUTLET)),
        2,
        "a rest refused by the active-battle gate must spend nothing"
    );
}

/// `Regen` rather than a stat kind on purpose: only the two over-time kinds
/// still carry a turn count when a routine armed them
/// (`FieldBuffKind::runs_until_rest`), so a `Def` buff here would test the
/// countdown against a buff that no longer has one and pass vacuously.
#[test]
fn tick_field_buffs_decrements_and_expires_after_the_exact_tick_count() {
    let mut game = Game::new(600, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Regen,
            name: "Test Shield".to_string(),
            power: 2,
            remaining: 5,
            interval: 1,
            source: BuffSource::Routine,
        },
    );

    for _ in 0..4 {
        game.tick_field_buffs();
    }
    assert_eq!(
        game.world.get::<FieldBuff>(player).unwrap().active.len(),
        1,
        "a 5-tick buff should still be running after only 4 ticks"
    );

    game.tick_field_buffs();
    assert!(
        game.world
            .get::<FieldBuff>(player)
            .unwrap()
            .active
            .is_empty(),
        "a 5-tick buff should be gone after the 5th tick"
    );
}

#[test]
fn tick_field_buffs_logs_the_armed_name_not_the_kind_on_expiry() {
    let mut game = Game::new(601, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::CaptureBoost,
            name: "Snare Protocol".to_string(),
            power: 10,
            remaining: 1,
            interval: 1,
            source: BuffSource::Consumable,
        },
    );

    game.tick_field_buffs();

    let log = game.message_log(10);
    assert!(
        log.iter().any(|e| e.text.contains("Snare Protocol")),
        "the expiry line should name the armed buff, not its kind: {log:?}"
    );
}

/// `Regen` for `tick_field_buffs_decrements_and_expires_after_the_exact_tick_count`'s
/// reason: a routine-armed `Atk` buff has no count left to age, so the
/// property this test exists for — that the walk reaches a companion and not
/// just the player — would go untested.
#[test]
fn tick_field_buffs_ages_buffs_on_party_members_too() {
    let mut game = Game::new(602, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, companion);
    game.arm_field_buff(
        companion,
        ActiveFieldBuff {
            kind: FieldBuffKind::Regen,
            name: "Overclock".to_string(),
            power: 3,
            remaining: 2,
            interval: 1,
            source: BuffSource::Routine,
        },
    );

    game.tick_field_buffs();

    let remaining = game
        .world
        .get::<FieldBuff>(companion)
        .unwrap()
        .active
        .first()
        .unwrap()
        .remaining;
    assert_eq!(remaining, 1, "a companion's field buff should tick too");
}

/// The tick half of the until-rest rule. Not "ages slowly" and not "expires
/// late" — a routine-armed buff of a read-on-demand kind is not aged at all,
/// so its `remaining` is still whatever it was armed with after a run's worth
/// of turns.
#[test]
fn an_until_rest_routine_buff_is_never_aged() {
    let mut game = Game::new(620, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Atk,
            name: "Overclock Single".to_string(),
            power: 4,
            remaining: 1,
            interval: 1,
            source: BuffSource::Routine,
        },
    );

    for _ in 0..500 {
        game.tick_field_buffs();
    }

    let buff = game
        .world
        .get::<FieldBuff>(player)
        .unwrap()
        .active
        .first()
        .cloned()
        .expect("an until-rest buff outlasts any number of turns");
    assert_eq!(
        buff.remaining, 1,
        "nothing may decrement an until-rest buff's count — it is not a lifetime"
    );
}

/// The `source` half of `ActiveFieldBuff::runs_until_rest`, which is the half
/// a rule keyed on `kind` alone would have got wrong. `patch_routine` arms
/// Mitigation from a one-shot item for 120 ticks; that item must still
/// expire, or its 10% would sit under the routine's forever —
/// `field_buff_power_of` sums a `Consumable` and a `Routine` entry of one
/// kind rather than choosing between them.
#[test]
fn a_consumable_buff_of_an_until_rest_kind_still_expires() {
    let mut game = Game::new(621, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Mitigation,
            name: "Patch Routine".to_string(),
            power: 10,
            remaining: 2,
            interval: 1,
            source: BuffSource::Consumable,
        },
    );

    game.tick_field_buffs();
    assert_eq!(
        game.world.get::<FieldBuff>(player).unwrap().active.len(),
        1,
        "a 2-tick consumable buff is still running after one tick"
    );

    game.tick_field_buffs();
    assert!(
        game.world
            .get::<FieldBuff>(player)
            .unwrap()
            .active
            .is_empty(),
        "an item's buff keeps its own clock however the kind behaves for a routine"
    );
}

/// Rest is the expiry event for the until-rest ones, and only for those. A
/// counted buff comes out of a rest **unaged** now that no rest advances the
/// clock — the drop is a thing `rest` does on purpose, not a side effect of
/// time passing, and that distinction is the whole of why the counted one
/// survives.
#[test]
fn resting_drops_until_rest_buffs_and_leaves_counted_ones_aged() {
    let mut game = Game::new(622, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    stand_in_base_beside_home(&mut game);
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Atk,
            name: "Overclock Single".to_string(),
            power: 4,
            remaining: 0,
            interval: 1,
            source: BuffSource::Routine,
        },
    );
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Regen,
            name: "Repair Loop Single".to_string(),
            power: 2,
            remaining: 12,
            interval: 1,
            source: BuffSource::Routine,
        },
    );

    game.rest().unwrap();

    let active = game.world.get::<FieldBuff>(player).unwrap().active.clone();
    assert_eq!(
        active.len(),
        1,
        "only the until-rest buff should go: {active:?}"
    );
    assert_eq!(active[0].kind, FieldBuffKind::Regen);
    assert_eq!(
        active[0].remaining, 12,
        "a counted buff is untouched: a rest passes no time to age it with"
    );
    assert!(
        game.message_log(20)
            .iter()
            .any(|e| e.text.contains("Overclock Single")),
        "the drop is announced by the name that armed it, as an expiry is"
    );
}

/// The drop walks the player *and* the party — the same set
/// `tick_field_buffs` ages, which is the only set an invocation can arm one on.
#[test]
fn resting_drops_a_companions_until_rest_buffs_too() {
    let mut game = Game::new(623, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, companion);
    stand_in_base_beside_home(&mut game);
    game.arm_field_buff(
        companion,
        ActiveFieldBuff {
            kind: FieldBuffKind::Mitigation,
            name: "Hardened Shell Single".to_string(),
            power: 4,
            remaining: 0,
            interval: 1,
            source: BuffSource::Routine,
        },
    );

    game.rest().unwrap();

    assert!(
        game.world
            .get::<FieldBuff>(companion)
            .unwrap()
            .active
            .is_empty(),
        "a companion's loadout ends with the player's"
    );
}

/// The drop sits with the heal and the refill, past every bail — so a rest
/// that never happened costs nothing. A player who walked to the wrong tile
/// has not lost what they run.
#[test]
fn a_refused_rest_keeps_until_rest_buffs() {
    let mut game = Game::new(624, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::XpBoost,
            name: "Trace Analysis Party".to_string(),
            power: 20,
            remaining: 0,
            interval: 1,
            source: BuffSource::Routine,
        },
    );

    // Out on the grid with an empty pack: the one refusal a rest still has.
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        let held = inv.count(&ItemId::from(ids::OUTLET));
        inv.take(ItemId::from(ids::OUTLET), held);
    }
    let refusal = game.rest();
    assert!(
        refusal.is_err(),
        "a rest that does not happen must say why, not fall silent"
    );

    assert_eq!(
        game.world.get::<FieldBuff>(player).unwrap().active.len(),
        1,
        "a refused rest must clear nothing"
    );
}

/// The row a buff list draws says which of the two lifetimes it has, and the
/// engine is what words it — `render/field.rs` puts `remaining` straight into
/// the row's suffix now rather than formatting a count itself.
#[test]
fn an_until_rest_buff_row_says_so_where_a_counted_one_shows_turns() {
    let mut game = Game::new(625, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Atk,
            name: "Overclock Single".to_string(),
            power: 4,
            remaining: 0,
            interval: 1,
            source: BuffSource::Routine,
        },
    );
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Regen,
            name: "Repair Loop Single".to_string(),
            power: 2,
            remaining: 40,
            interval: 1,
            source: BuffSource::Routine,
        },
    );

    let rows = game.active_buffs();

    let overclock = rows.iter().find(|b| b.name == "Overclock Single").unwrap();
    let repair = rows
        .iter()
        .find(|b| b.name == "Repair Loop Single")
        .unwrap();
    assert_eq!(overclock.remaining, "rest");
    assert_eq!(repair.remaining, "40t");
}

#[test]
fn tick_field_buffs_regen_heals_the_carrier_and_caps_at_max_hp() {
    let mut game = Game::new(610, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let max_hp = game.world.get::<Stats>(player).unwrap().max_hp;
    game.world.get_mut::<Stats>(player).unwrap().hp = max_hp - 10;
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Regen,
            name: "Nanite Patch".to_string(),
            power: 4,
            remaining: 5,
            interval: 1,
            source: BuffSource::Routine,
        },
    );

    game.tick_field_buffs();
    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        max_hp - 6,
        "Regen should heal by exactly its power on a tick that doesn't hit the cap"
    );

    // Two more ticks at +4 each would land at max_hp - 6 + 8 = max_hp + 2
    // if uncapped — proves the clamp, not just that healing happened.
    game.tick_field_buffs();
    game.tick_field_buffs();
    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        max_hp,
        "Regen must not heal past max_hp"
    );
}

/// A long trickle heals on a cadence rather than every turn — `interval` is
/// the whole difference between "+2 for 300 turns" and "+2 every fourth turn
/// for 300". The cadence is phased off `remaining`, so a duration that is a
/// multiple of its interval (as every shipped one is) fires on the first tick
/// and every interval-th tick after.
#[test]
fn an_interval_makes_a_field_buff_fire_on_a_cadence_not_every_tick() {
    let mut game = Game::new(612, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let max_hp = game.world.get::<Stats>(player).unwrap().max_hp;
    let start = max_hp - 40;
    game.world.get_mut::<Stats>(player).unwrap().hp = start;
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Regen,
            name: "Repair Loop Single".to_string(),
            power: 2,
            remaining: 8,
            interval: 4,
            source: BuffSource::Routine,
        },
    );

    game.tick_field_buffs();
    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        start + 2,
        "the first tick is on the cadence, so it heals"
    );
    for _ in 0..3 {
        game.tick_field_buffs();
    }
    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        start + 2,
        "the three turns after it are not, and must heal nothing"
    );
    for _ in 0..4 {
        game.tick_field_buffs();
    }
    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        start + 4,
        "eight turns at one heal every fourth is two heals, not eight"
    );
}

#[test]
fn tick_field_buffs_regen_heals_a_companion_not_the_player() {
    let mut game = Game::new(611, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, companion);
    game.world.get_mut::<Stats>(companion).unwrap().hp = 4;
    let player_hp_before = game.world.get::<Stats>(game.player_entity()).unwrap().hp;
    game.arm_field_buff(
        companion,
        ActiveFieldBuff {
            kind: FieldBuffKind::Regen,
            name: "Self Repair".to_string(),
            power: 3,
            remaining: 5,
            interval: 1,
            source: BuffSource::Routine,
        },
    );

    game.tick_field_buffs();

    assert_eq!(
        game.world.get::<Stats>(companion).unwrap().hp,
        7,
        "the companion carrying Regen should heal itself"
    );
    assert_eq!(
        game.world.get::<Stats>(game.player_entity()).unwrap().hp,
        player_hp_before,
        "Regen on a companion must not heal the player"
    );
}

/// `tick_field_buffs` runs on every `tick()`, including every battle round,
/// and `Party` deliberately keeps a dead member around until `end_battle`
/// reaps it — so a `Regen` with no floor check would heal a companion
/// killed mid-battle back to positive HP on the very next tick. This repo
/// shipped permadeath; an accidental auto-revive would silently undo it.
#[test]
fn tick_field_buffs_regen_does_not_revive_a_dead_companion() {
    let mut game = Game::new(614, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, companion);
    game.arm_field_buff(
        companion,
        ActiveFieldBuff {
            kind: FieldBuffKind::Regen,
            name: "Self Repair".to_string(),
            power: 5,
            remaining: 10,
            interval: 1,
            source: BuffSource::Routine,
        },
    );
    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);
    game.world.get_mut::<Stats>(companion).unwrap().hp = -3;

    game.tick_field_buffs();

    assert_eq!(
        game.world.get::<Stats>(companion).unwrap().hp,
        -3,
        "a dead companion's HP must not move on a Regen tick"
    );
    assert!(
        !game.creature_alive(companion),
        "the companion must still read as dead"
    );

    game.end_battle(player, None);
    // Forgiving benches rather than reaps, so what says "the teardown still
    // read this program as dead" is the marker, not an absent entity.
    assert!(
        game.world
            .get::<crate::components::Downed>(companion)
            .is_some(),
        "end_battle must still treat the companion as dead; a running Regen must not save it"
    );
}

#[test]
fn a_full_tick_applies_trickle_on_top_of_that_ticks_decay() {
    let mut game = Game::new(612, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let hunger_before = {
        let mut needs = game.world.get_mut::<PowerReserve>(player).unwrap();
        *needs = PowerReserve::new(90.0);
        needs.get()
    };
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Trickle,
            name: "Power Tap".to_string(),
            power: 15,
            remaining: 5,
            interval: 1,
            source: BuffSource::Routine,
        },
    );

    game.wait();

    // `needs_tick_system` runs inside the same tick's schedule, ahead of
    // `tick_field_buffs` (see `tick_inner`), so the restore lands on top of
    // that tick's own drain. Read through the live formula instead of
    // restating its constants.
    let ticked_hunger = hunger_before - crate::systems::power_drain_per_tick(1.0);
    let needs = *game.world.get::<PowerReserve>(player).unwrap();
    assert_eq!(
        needs.get(),
        (ticked_hunger + 15.0).min(POWER_MAX),
        "Trickle should restore Power on top of the tick's own drain"
    );
    assert_eq!(
        needs.get(),
        POWER_MAX,
        "90 + 15 minus a hair of decay should still clamp to the cap"
    );
}

#[test]
fn tick_field_buffs_applies_a_buffs_last_tick_before_it_expires() {
    let mut game = Game::new(613, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let max_hp = game.world.get::<Stats>(player).unwrap().max_hp;
    game.world.get_mut::<Stats>(player).unwrap().hp = max_hp - 5;
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Regen,
            name: "Last Gasp".to_string(),
            power: 3,
            remaining: 1,
            interval: 1,
            source: BuffSource::Routine,
        },
    );

    game.tick_field_buffs();

    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        max_hp - 2,
        "a buff with 1 tick left must still apply its effect before it expires"
    );
    assert!(
        game.world
            .get::<FieldBuff>(player)
            .unwrap()
            .active
            .is_empty(),
        "the buff should be gone after its last tick"
    );
}

/// `ActiveFieldBuff`/`FieldBuffKind`/`BuffSource` all need to survive a
/// save/load round trip intact, on both the player and a party member — not
/// just the count, but every field, since a save that silently dropped
/// `power` or `source` would still pass a length check.
#[test]
fn field_buffs_survive_a_save_load_round_trip() {
    let mut game = Game::new(604, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, companion);

    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::CaptureBoost,
            name: "Snare Protocol".to_string(),
            power: 15,
            remaining: 7,
            interval: 1,
            source: BuffSource::Consumable,
        },
    );
    game.arm_field_buff(
        companion,
        ActiveFieldBuff {
            kind: FieldBuffKind::Atk,
            name: "Overclock".to_string(),
            power: 4,
            remaining: 3,
            interval: 1,
            source: BuffSource::Routine,
        },
    );

    let path = std::env::temp_dir().join(format!(
        "feral_field_buffs_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let player_buff = loaded
        .world
        .get::<FieldBuff>(loaded.player_entity())
        .unwrap()
        .active
        .first()
        .cloned()
        .expect("the player's field buff should survive the round trip");
    assert_eq!(player_buff.kind, FieldBuffKind::CaptureBoost);
    assert_eq!(player_buff.name, "Snare Protocol");
    assert_eq!(player_buff.power, 15);
    assert_eq!(player_buff.remaining, 7);
    assert_eq!(player_buff.source, BuffSource::Consumable);

    let companion_buff = loaded
        .owned_pets()
        .first()
        .and_then(|p| loaded.world.get::<FieldBuff>(p.entity).cloned())
        .and_then(|f| f.active.first().cloned())
        .expect("the companion's field buff should survive the round trip");
    assert_eq!(companion_buff.kind, FieldBuffKind::Atk);
    assert_eq!(companion_buff.name, "Overclock");
    assert_eq!(companion_buff.power, 4);
    assert_eq!(companion_buff.remaining, 3);
    assert_eq!(companion_buff.source, BuffSource::Routine);
}

#[test]
fn use_item_applies_a_power_restore_and_consumes_one() {
    let mut game = Game::new(500, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(50.0);
    // The player already starts holding Power Cells (see `Game::new`);
    // drain the default stock first so the stack is exactly 2 below.
    let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
    let held = inv.count(&ItemId::from(ids::POWER_CELL));
    inv.take(ItemId::from(ids::POWER_CELL), held);
    inv.add(ItemId::from(ids::POWER_CELL), 2);

    game.use_item(&ItemId::from(ids::POWER_CELL));

    // `use_item` ends with `self.tick()` like every other player action,
    // so `needs_tick_system` also shaves off one tick's worth of hunger
    // (see `HUNGER_DECAY_PER_TICK` in systems.rs) on top of the +25
    // restore — same shared-decay caveat documented on
    // `commanding_a_companion_in_battle_costs_the_player_no_power`.
    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        75.0 - 0.15
    );
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::POWER_CELL)),
        1
    );
}

#[test]
fn use_item_clamps_power_at_full() {
    let mut game = Game::new(501, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(90.0);
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::POWER_CELL), 1);

    game.use_item(&ItemId::from(ids::POWER_CELL));

    // 90 + 25 clamps to 100 before the trailing tick's decay shaves off
    // 0.15 (see the comment in the test above) — had the clamp not
    // engaged, this would read 114.85 instead.
    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        100.0 - 0.15
    );
}

#[test]
fn use_item_rejects_a_non_consumable() {
    let mut game = Game::new(502, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // The player already starts holding Core Fragments (see
    // `Game::new`), so compare against a captured baseline rather than
    // an absolute count.
    let before = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::CORE_FRAGMENT));
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 3);

    game.use_item(&ItemId::from(ids::CORE_FRAGMENT));

    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT)),
        before + 3,
        "a non-consumable must not be consumed"
    );
}

#[test]
fn use_item_on_an_empty_stack_is_a_no_op() {
    let mut game = Game::new(503, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // The player starts holding Power Cells (see `Game::new`), so drain
    // the stack to actually exercise the empty-stack path.
    let held = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::POWER_CELL));
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .take(ItemId::from(ids::POWER_CELL), held);
    let before = game.world.get::<PowerReserve>(player).unwrap().get();

    game.use_item(&ItemId::from(ids::POWER_CELL));

    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        before
    );
}

#[test]
fn a_prebattle_buff_armed_on_the_map_is_live_at_the_next_intrusion() {
    let mut game = Game::new(504, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // Arm an Atk buff directly — models a companion's Rally/Shield left
    // active going into a fight, `CombatBuff`'s own reason to exist. A
    // pre-battle consumable no longer arms this component; see
    // `arm_field_buff` below for what it arms instead.
    game.world.get_mut::<CombatBuff>(player).unwrap().active = Some(ActiveBuff {
        kind: BuffKind::Atk,
        remaining: 3,
        power: 5,
    });
    // And a field buff, modeling what a prebattle_buff consumable arms
    // now (see `Game::arm_field_buff`) — it must carry into the fight the
    // same way a `CombatBuff` does.
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Atk,
            name: "Test Stim".to_string(),
            power: 5,
            remaining: 5,
            interval: 1,
            source: BuffSource::Consumable,
        },
    );

    let wild = spawn_wild_on_player_tile(&mut game);
    game.start_battle(vec![wild]);

    let buff = game.world.get::<CombatBuff>(player).unwrap().active;
    assert!(
        matches!(
            buff,
            Some(ActiveBuff {
                kind: BuffKind::Atk,
                power: 5,
                ..
            })
        ),
        "a buff armed before the fight must still be active when it starts"
    );
    assert_eq!(
        game.field_buff_power(player, FieldBuffKind::Atk),
        5,
        "a field buff armed before the fight must also still be active when it starts"
    );
}

/// One `.ron` item, shared by the two reproducers below, declaring a
/// `prebattle_buff` — no shipped item declares one, so a fixture is the
/// only way to drive `use_item`'s real code path.
const TEST_STIM_ITEM: &str = r#"(
    id: "test_stim",
    name: "Test Stim",
    consume: Some((
        prebattle_buff: Some((kind: Atk, power: 5, ticks: 5)),
    )),
)"#;

/// Bug 1: `clear_battle_status_effects` used to null the player's
/// `CombatBuff` unconditionally whenever a battle ended — correct for a
/// companion's Rally, but the pre-battle item buff was living in that same
/// component, so a 5-round stim was destroyed by a battle that ended after
/// 1 round, 4 rounds still on the clock. It now arms `FieldBuff`, which
/// battle end never touches.
#[test]
fn a_prebattle_buff_survives_the_battle_it_was_armed_for() {
    let dir = modded_assets_dir(
        "prebattle_stim_survives_battle",
        &[],
        &[("test_stim.ron", TEST_STIM_ITEM)],
        &[],
        &[],
        &[],
    );
    let mut game = Game::new(9101, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let player = game.player_entity();
    let stim = ItemId::from("test_stim");
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(stim.clone(), 1);

    game.use_item(&stim);
    assert!(
        game.world
            .get::<FieldBuff>(player)
            .unwrap()
            .active
            .iter()
            .any(|b| b.kind == FieldBuffKind::Atk && b.power == 5),
        "arming a prebattle_buff item should land it on FieldBuff, not the \
         battle-scoped CombatBuff"
    );

    start_battle_with_a_wild_program(&mut game);
    game.end_battle(player, None);

    assert!(
        game.world
            .get::<FieldBuff>(player)
            .unwrap()
            .active
            .iter()
            .any(|b| b.kind == FieldBuffKind::Atk && b.power == 5),
        "a field buff must still be running once the battle it was armed for ends"
    );
}

/// Bug 2: before `FieldBuff` existed there was nowhere to put a map-armed
/// item buff in `PlayerSave`, so it vanished on the round trip. Arming it
/// now writes `FieldBuff`, which `PlayerSave` already persists.
#[test]
fn a_prebattle_buff_survives_a_save_load_round_trip() {
    let dir = modded_assets_dir(
        "prebattle_stim_survives_save",
        &[],
        &[("test_stim.ron", TEST_STIM_ITEM)],
        &[],
        &[],
        &[],
    );
    let mut game = Game::new(9102, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    let stim = ItemId::from("test_stim");
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(stim.clone(), 1);
    game.use_item(&stim);

    let path = std::env::temp_dir().join(format!(
        "feral_prebattle_buff_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &dir).unwrap();
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir_all(&dir);

    let buff = loaded
        .world
        .get::<FieldBuff>(loaded.player_entity())
        .unwrap()
        .active
        .first()
        .cloned();
    assert!(
        matches!(
            buff,
            Some(ActiveFieldBuff {
                kind: FieldBuffKind::Atk,
                power: 5,
                ..
            })
        ),
        "a prebattle buff must survive a save/load round trip: {buff:?}"
    );
}

#[test]
fn use_power_source_restores_power_and_consumes_one() {
    let mut game = Game::new(504, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(50.0);
    // The player already starts holding Power Cells (see `Game::new`);
    // drain the default stock first so the stack is exactly 2 below.
    let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
    let held = inv.count(&ItemId::from(ids::POWER_CELL));
    inv.take(ItemId::from(ids::POWER_CELL), held);
    inv.add(ItemId::from(ids::POWER_CELL), 2);

    game.use_power_source();

    // `use_power_source` dispatches to `use_item`, which ends with
    // `self.tick()` like every other player action, so
    // `needs_tick_system` also shaves off one tick's worth of hunger
    // (see `HUNGER_DECAY_PER_TICK` in systems.rs) on top of the +25
    // restore — same shared-decay caveat as `use_item_applies_a_power_
    // restore_and_consumes_one` above.
    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        75.0 - 0.15
    );
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::POWER_CELL)),
        1
    );
}

#[test]
fn use_power_source_with_nothing_to_recharge_from_is_a_no_op() {
    let mut game = Game::new(505, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // Drain the default Power Cell stock (see `Game::new`) so no
    // power-restoring item remains; the Core Fragments the player also
    // starts with have no `consume` effect at all.
    let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
    let held = inv.count(&ItemId::from(ids::POWER_CELL));
    inv.take(ItemId::from(ids::POWER_CELL), held);
    let fragments_before = inv.count(&ItemId::from(ids::CORE_FRAGMENT));
    let hunger_before = game.world.get::<PowerReserve>(player).unwrap().get();

    game.use_power_source();

    // No candidate item means no `use_item` dispatch, so unlike the
    // success path above there's no trailing `tick()` and hunger must
    // be untouched, not merely undecayed.
    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        hunger_before,
        "a failed recharge must not tick the game or touch PowerReserve"
    );
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT)),
        fragments_before,
        "a failed recharge must not consume an unrelated item"
    );
    assert!(
        game.message_log(10)
            .iter()
            .any(|e| e.text == "You have nothing to recharge from."),
        "expected the no-power-source message, got: {:?}",
        game.message_log(10)
    );
}

#[test]
fn use_power_source_picks_the_power_item_over_an_earlier_non_power_item() {
    let mut game = Game::new(506, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(50.0);
    // Drain all four starting stacks (see `Game::new`: Ice Breaker, Power
    // Cell, Core Fragment, Power Outlet) and rebuild the inventory with the
    // non-power item (Core Fragment) added *first*, so it's ahead of
    // the Power Cell in `Inventory::items`. This pins selection to the
    // `ConsumeDef.power > 0.0` predicate rather than to iteration
    // order or to which `ItemId` happens to be checked first.
    let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
    let ice_breaker_held = inv.count(&ItemId::from(ids::ICE_BREAKER));
    inv.take(ItemId::from(ids::ICE_BREAKER), ice_breaker_held);
    let power_held = inv.count(&ItemId::from(ids::POWER_CELL));
    inv.take(ItemId::from(ids::POWER_CELL), power_held);
    let fragments_held = inv.count(&ItemId::from(ids::CORE_FRAGMENT));
    inv.take(ItemId::from(ids::CORE_FRAGMENT), fragments_held);
    let outlets_held = inv.count(&ItemId::from(ids::OUTLET));
    inv.take(ItemId::from(ids::OUTLET), outlets_held);
    inv.add(ItemId::from(ids::CORE_FRAGMENT), 5);
    inv.add(ItemId::from(ids::POWER_CELL), 2);
    assert_eq!(
        inv.items[0].0,
        ItemId::from(ids::CORE_FRAGMENT),
        "test setup: the non-power item must be first in iteration order"
    );

    game.use_power_source();

    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::POWER_CELL)),
        1,
        "the power-restoring item should have been the one consumed"
    );
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT)),
        5,
        "the earlier non-power item must be left untouched"
    );
    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        75.0 - 0.15
    );
}

/// Wipes every wild program off the map. The ambush tests use this to make
/// a battle unambiguous: with nothing left to walk into, the bump path in
/// `move_player` cannot fire, so any fight that opens came from the ambush
/// roll.
fn despawn_every_hostile(game: &mut Game) {
    let hostiles: Vec<Entity> = {
        let mut query = game.world.query_filtered::<Entity, With<Hostile>>();
        query.iter(&game.world).collect()
    };
    for entity in hostiles {
        game.world.despawn(entity);
    }
}

#[test]
fn an_ambush_engages_a_pack_immediately() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    despawn_every_hostile(&mut game);

    for _ in 0..2000 {
        game.maybe_ambush();
        if game.has_active_battle() {
            let enemies = game.all_living_enemies();
            assert!(
                !enemies.is_empty(),
                "an ambush that opens a battle must put something in it"
            );
            return;
        }
    }
    panic!("2000 ambush rolls never fired — RANDOM_ENCOUNTER_CHANCE may be broken");
}

/// Bosses are something you find and choose to fight. One that jumps you
/// with no chance to decline is a death sentence you never opted into.
#[test]
fn an_ambush_never_fields_a_boss() {
    let mut ambushes = 0;
    for seed in 0..40 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        despawn_every_hostile(&mut game);
        for _ in 0..400 {
            game.maybe_ambush();
            if !game.has_active_battle() {
                continue;
            }
            ambushes += 1;
            for enemy in game.all_living_enemies() {
                let species = game.world.get::<Creature>(enemy).unwrap().species.clone();
                let def = game.world.resource::<SpeciesDb>().get(&species).cloned();
                assert!(
                    !def.expect("an ambushed program's species must be loaded")
                        .is_boss,
                    "an ambush must never field a boss"
                );
            }
            despawn_every_hostile(&mut game);
            game.world.remove_resource::<BattleState>();
        }
    }
    // Without this the whole sweep passes vacuously if ambushes stop
    // firing — the assertion above only runs inside a battle.
    assert!(
        ambushes > 100,
        "the sweep only fired {ambushes} ambushes; it is not exercising the boss check"
    );
}

/// The base is out of phase and nothing out on the zone surface can reach
/// it — it is the one safe ground, and walking around inside it must never
/// draw an ambush.
///
/// Driven through `move_player` rather than `maybe_ambush` directly, because
/// what keeps the base safe is that `move_player` dispatches on locale and
/// the base-space branch never rolls at all. Calling the roll by hand would
/// be testing a path nothing in base space takes.
#[test]
fn no_ambush_fires_while_walking_the_base() {
    let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    despawn_every_hostile(&mut game);
    stand_in_base(&mut game);

    let before = game.current_tick();
    for step in 0..2000 {
        let (dx, dy) = if step % 2 == 0 { (1, 0) } else { (-1, 0) };
        game.move_player(dx, dy);
        assert!(
            !game.has_active_battle(),
            "walking your own base must never be ambushed"
        );
    }
    // The clock, not the position. A sweep that ended back where it started
    // proves nothing on its own — that is equally true of one whose every
    // step was *refused*, which is exactly the failure this guard is for. A
    // step in base space costs a turn and a refused one costs nothing, so
    // the tick count is what says the party actually walked.
    assert_eq!(
        game.current_tick() - before,
        2000,
        "every step has to have landed, or the sweep never walked at all"
    );
}

/// The integration the ambush actually ships as: a walked step can open a
/// fight. Every hostile is cleared immediately before each move, so the
/// bump path in `move_player` has nothing to trigger on and any battle that
/// appears is attributable to the ambush roll alone.
#[test]
fn walking_open_ground_can_be_ambushed() {
    let mut game = Game::new(3, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    for step in 0..1000 {
        despawn_every_hostile(&mut game);
        let dx = if step % 2 == 0 { 1 } else { -1 };
        game.move_player(dx, 0);
        if game.has_active_battle() {
            return;
        }
        if game.is_game_over().is_some() {
            panic!("the player died before any ambush fired");
        }
    }
    panic!("1000 walked steps never produced an ambush");
}

/// Stands the player on a tile of `from` with a tile of `to` one step east,
/// both written through the override overlay. Deliberately not hunting the
/// generated map for a boundary: `tile_overrides` is what the overlay
/// exists for, and it keeps the test off world-seed luck.
///
/// Anything standing on the destination is despawned first — walking into a
/// program, a nest or a structure is a fight or a door, not travel, and
/// `move_player` returns before the step in every one of those branches.
fn ground_step(game: &mut Game, from: Biome, to: Biome, to_walkable: bool) -> (i32, i32) {
    let player = game.player_entity();
    let pos = *game.world.get::<Position>(player).unwrap();
    let (nx, ny) = (pos.x + 1, pos.y);
    let squatters: Vec<Entity> = {
        let mut q = game.world.query::<(Entity, &Position)>();
        q.iter(&game.world)
            .filter(|(e, p)| *e != player && p.x == nx && p.y == ny)
            .map(|(e, _)| e)
            .collect()
    };
    for e in squatters {
        game.world.despawn(e);
    }
    let mut map = game.world.resource_mut::<WorldMap>();
    map.set_override(
        pos.x,
        pos.y,
        Tile {
            biome: from,
            walkable: true,
            rock_shade: None,
        },
    );
    map.set_override(
        nx,
        ny,
        Tile {
            biome: to,
            walkable: to_walkable,
            rock_shade: None,
        },
    );
    (nx, ny)
}

fn log_names(game: &Game, biome: Biome) -> usize {
    game.world
        .resource::<MessageLog>()
        .lines
        .iter()
        .filter(|l| l.text.contains(biome.name()))
        .count()
}

#[test]
fn stepping_into_a_different_biome_names_the_ground_you_reached() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (nx, ny) = ground_step(&mut game, Biome::OpenGrid, Biome::Deadlock, true);

    game.move_player(1, 0);

    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    assert_eq!((pos.x, pos.y), (nx, ny), "the fixture never took the step");
    assert_eq!(
        log_names(&game, Biome::Deadlock),
        1,
        "crossing a boundary is the first time the ground has ever been named"
    );
}

#[test]
fn stepping_within_one_biome_names_nothing() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    ground_step(&mut game, Biome::Deadlock, Biome::Deadlock, true);

    game.move_player(1, 0);

    assert_eq!(
        log_names(&game, Biome::Deadlock),
        0,
        "the line belongs to the boundary, not to every step across a sector"
    );
}

#[test]
fn bouncing_off_an_unwalkable_tile_names_no_biome() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (nx, ny) = ground_step(&mut game, Biome::OpenGrid, Biome::BlackIce, false);

    game.move_player(1, 0);

    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    assert_ne!(
        (pos.x, pos.y),
        (nx, ny),
        "an unwalkable tile is not enterable"
    );
    assert_eq!(
        log_names(&game, Biome::BlackIce),
        0,
        "shoving at a wall is not travel — the same rule `maybe_ambush` follows"
    );
}

/// Zone 1 is neutral ground and takes no environment *effects*. The name is
/// deliberately outside that gate: a player who has never left the first
/// sector should still learn what they are walking on. This test is what
/// stops the effect gate being wrapped around the log line later.
#[test]
fn the_ground_is_named_at_zone_one() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(game.world.resource::<ZoneLevel>().0, 1);
    ground_step(&mut game, Biome::OpenGrid, Biome::Backplane, true);

    game.move_player(1, 0);

    assert_eq!(log_names(&game, Biome::Backplane), 1);
}

// ---------------------------------------------------------------------
// Rest: free inside the base, an outlet outside it, instant everywhere
// ---------------------------------------------------------------------

/// The base half of the flip. Standing inside base space is the whole
/// price — no structure in reach, no item spent — because the walk home is
/// what the rest costs.
#[test]
fn resting_in_base_space_spends_nothing() {
    let mut game = Game::new(3210, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        let held = inv.count(&ItemId::from(ids::OUTLET));
        inv.take(ItemId::from(ids::OUTLET), held);
    }
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(10.0);
    stand_in_base(&mut game);

    game.rest().unwrap();

    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        100.0,
        "a rest in base space runs with an empty pack: {:?}",
        game.message_log(5)
    );
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::OUTLET)),
        0,
        "and spends no outlet"
    );
}

/// The anti-abuse half, and the reason a free rest is safe to give away:
/// no rest advances the clock, so a base rest cannot be spammed to farm
/// production, raid pressure or need decay.
#[test]
fn a_rest_in_base_space_does_not_advance_the_clock() {
    let mut game = Game::new(3211, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(10.0);
    stand_in_base(&mut game);
    let before = game.current_tick();

    game.rest().unwrap();

    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        100.0,
        "the rest has to run, or the clock assertion below is vacuous"
    );
    assert_eq!(
        before,
        game.current_tick(),
        "a rest passes no time anywhere any more"
    );
}

/// The field half. Out on the open grid there is no base to stand in, so
/// the rest is bought with a carried charge instead.
#[test]
fn resting_on_the_surface_spends_one_outlet() {
    let mut game = Game::new(3212, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(10.0);
    let before = game.current_tick();

    game.rest().unwrap();

    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        100.0,
        "an outlet rests you anywhere: {:?}",
        game.message_log(5)
    );
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::OUTLET)),
        1,
        "a fresh game starts with 2 outlets; one rest should leave exactly 1"
    );
    assert_eq!(
        before,
        game.current_tick(),
        "and the field rest passes no time either"
    );
}

/// Underground is field too. An outlet is the Stack's rest, which is what
/// makes a deep run bounded by outlet stock rather than by Power reserve.
#[test]
fn resting_underground_spends_one_outlet() {
    let mut game = Game::new(3213, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    descend(&mut game);
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(10.0);

    game.rest().unwrap();

    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        100.0,
        "the Stack no longer refuses a rest outright: {:?}",
        game.message_log(5)
    );
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::OUTLET)),
        1,
        "and it costs the same one outlet the surface does"
    );
}

/// The refusal, and the only one the field path has.
#[test]
fn resting_outside_the_base_with_no_outlet_is_refused() {
    let mut game = Game::new(3214, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        let held = inv.count(&ItemId::from(ids::OUTLET));
        inv.take(ItemId::from(ids::OUTLET), held);
    }
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(10.0);

    let refusal = game
        .rest()
        .expect_err("a rest that does not happen must say why, not fall silent");

    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        10.0,
        "a refused rest restores nothing"
    );
    // Read off the returned sentence rather than the log: the caller is what
    // puts a refusal on the map's status banner, and a rest whose only
    // account of itself was an `Info` line the log filter can hide is the
    // hole this whole path was widened to close.
    assert!(
        refusal.to_lowercase().contains("outlet"),
        "and says why: {refusal:?}"
    );
}

/// What a rest costs in the field is a property of the *item*, not a
/// hardcoded id — the price used to live on `RestDef::cost` precisely so a
/// mod could change it, and it must stay data now that no structure is
/// involved. A modded charge is spent in the shipped outlet's place.
#[test]
fn any_item_flagged_enables_rest_can_buy_a_field_rest() {
    let dir = assets_dir_with_extra_item(
        "spare_battery",
        "spare_battery.ron",
        r#"(
    id: "spare_battery",
    name: "Spare Battery",
    description: "Test fixture: a modded rest charge.",
    value: Some(5),
    enables_rest: true,
)"#,
    );
    let mut game = Game::new(3215, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        let held = inv.count(&ItemId::from(ids::OUTLET));
        inv.take(ItemId::from(ids::OUTLET), held);
        inv.add(ItemId::from("spare_battery"), 1);
    }
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(10.0);

    game.rest().unwrap();

    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        100.0,
        "a modded rest charge should rest you: {:?}",
        game.message_log(5)
    );
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from("spare_battery")),
        0,
        "and be spent doing it"
    );
}

/// The shipped half of the rule above: the Power Outlet is what carries the
/// flag, so deleting the field from `outlet.ron` is a content change rather
/// than something the engine papers over.
#[test]
fn the_power_outlet_is_a_rest_charge() {
    let game = Game::new(3216, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(
        game.item_def(&ItemId::from(ids::OUTLET))
            .expect("outlet.ron should load")
            .enables_rest,
        "the Power Outlet is the shipped rest charge"
    );
}

/// The reported bug, and the one shape of it that reproduces: `r` in the
/// Stack doing *literally nothing* — no heal, no charge spent, and not one
/// word about why.
///
/// `Inventory::add(item, 0)` pushes a `(item, 0)` slot when no slot exists,
/// and `rest_charge_in_pack` matched on `ItemDef::enables_rest` alone
/// without ever asking how many were in the stack. So an empty outlet slot
/// *was* a charge as far as the gate was concerned: the "you have none"
/// refusal was skipped, `Inventory::take` came back with 0, and `rest` took
/// its silent `return`. Nothing on screen, nothing in the log, nothing to
/// diagnose from — which is what a bug report saying outlets don't work in
/// the Stack looks like from the inside.
///
/// A charge is a *unit you can spend*, not a key that happens to be in the
/// map, so the quantity belongs in the predicate rather than in a second
/// check downstream of it.
#[test]
fn an_empty_rest_charge_slot_is_not_a_charge() {
    let mut game = Game::new(3216, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    descend(&mut game);
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        let held = inv.count(&ItemId::from(ids::OUTLET));
        inv.take(ItemId::from(ids::OUTLET), held);
        // The state the silent return needs: the slot is present and empty.
        inv.add(ItemId::from(ids::OUTLET), 0);
    }
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(10.0);

    let refusal = game.rest().expect_err("an empty slot cannot buy a rest");

    assert!(
        refusal.to_lowercase().contains("outlet"),
        "and it must name the charge it wanted: {refusal:?}"
    );
    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        10.0,
        "a refused rest restores nothing"
    );
}

/// The other two silent exits. Both are unreachable from the map screen
/// today, which is exactly why they were written as bare `return`s — and
/// exactly why a report of "`r` does nothing" could not be told apart from
/// them. A refusal that is never spoken is indistinguishable from a dead
/// key, so `rest` now has no exit that says nothing.
#[test]
fn resting_mid_intrusion_says_why_instead_of_nothing() {
    let mut game = Game::new(3217, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    start_battle_with_a_wild_program(&mut game);
    assert!(game.has_active_battle(), "fixture: a fight must be open");

    let refusal = game.rest().expect_err("a rest mid-fight is refused");

    assert!(!refusal.is_empty(), "and it is not silence");
}

// --- Powering down in the field can be interrupted -------------------------
//
// The roll sits below the payment and above the restore. Every test here is
// a consequence of that one placement, so they are written as a group.

/// The charge is spent, the fight is open, and *nothing* was restored. No
/// refund is the mechanic rather than an oversight — a refund makes the risk
/// free and `REST_AMBUSH_CHANCE` meaningless.
#[test]
fn an_interrupted_field_rest_burns_the_charge_and_restores_nothing() {
    let game = first_rng_seed_where(
        |seed| {
            let mut game = a_hurt_player_holding_one_outlet();
            reseed_rng(&mut game, seed);
            game.rest().unwrap();
            game
        },
        |game| game.has_active_battle(),
    );

    let player = game.player_entity();
    let stats = *game.world.get::<Stats>(player).unwrap();
    let reserve = *game.world.get::<PowerReserve>(player).unwrap();
    assert_eq!(stats.hp, 1, "an interrupted rest must restore no Integrity");
    assert_eq!(
        reserve.get(),
        10.0,
        "an interrupted rest must restore no Power"
    );
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::OUTLET)),
        0,
        "the charge is spent even though the rest was jumped — there is no refund"
    );
}

/// The other side of the same roll: a charged rest that is not interrupted
/// behaves exactly as it did before this feature existed.
#[test]
fn an_uninterrupted_field_rest_still_fully_restores() {
    let game = first_rng_seed_where(
        |seed| {
            let mut game = a_hurt_player_holding_one_outlet();
            reseed_rng(&mut game, seed);
            game.rest().unwrap();
            game
        },
        |game| !game.has_active_battle(),
    );

    let player = game.player_entity();
    let stats = *game.world.get::<Stats>(player).unwrap();
    let reserve = *game.world.get::<PowerReserve>(player).unwrap();
    assert_eq!(stats.hp, stats.max_hp, "a rest that was not jumped heals");
    assert_eq!(reserve.get(), 100.0, "a rest that was not jumped recharges");
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::OUTLET)),
        0,
        "the charge is spent either way"
    );
}

/// Base space stays the one safe ground, and it is safe *by placement* — the
/// roll rides the branch that takes a charge, and a free rest never enters
/// it. Asserted on the RNG stream rather than on the outcome, because a 0.15
/// chance passes an outcome test five times in six by luck.
#[test]
fn a_free_base_rest_never_rolls_for_an_interrupt() {
    use rand::SeedableRng;

    let mut game = Game::new(18, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.hp = 1;
    }
    stand_in_base_beside_home(&mut game);

    reseed_rng(&mut game, 4242);
    game.rest().unwrap();

    let next: u64 = game.world.resource_mut::<GameRng>().0.random();
    let expected: u64 = rand::rngs::StdRng::seed_from_u64(4242).random();
    assert_eq!(
        next, expected,
        "a free base rest must not draw from GameRng at all"
    );
}

/// Underground and above ground both roll, and each draws its own kind of
/// pack. `StackSpawn` is the tag `end_battle` sweeps by, so a Stack interrupt
/// that fielded a surface pack would leave bodies behind on the way out.
#[test]
fn each_locale_draws_its_own_pack_kind_when_a_rest_is_jumped() {
    let surface = first_rng_seed_where(
        |seed| {
            let mut game = a_hurt_player_holding_one_outlet();
            reseed_rng(&mut game, seed);
            game.rest().unwrap();
            game
        },
        |game| game.has_active_battle(),
    );
    assert!(
        !any_combatant_is_a_stack_spawn(&surface),
        "a surface rest must field a surface pack"
    );

    let underground = first_rng_seed_where(
        |seed| {
            let mut game = a_hurt_player_holding_one_outlet();
            descend(&mut game);
            reseed_rng(&mut game, seed);
            game.rest().unwrap();
            game
        },
        |game| game.has_active_battle(),
    );
    assert!(
        any_combatant_is_a_stack_spawn(&underground),
        "a Stack rest must field a Stack pack, tagged for end_battle to sweep"
    );
}

/// A hurt, drained player standing in the open with exactly one rest charge —
/// the fixture every interrupt test above shares.
fn a_hurt_player_holding_one_outlet() -> Game {
    let mut game = Game::new(21, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // Exactly one, not one *more*: the starting kit already ships outlets,
    // so "the charge is spent" is only readable as a count of zero if the
    // pack held precisely the charge the rest is about to take. Through
    // `take`/`add` rather than pushing a row, since `Inventory::count` reads
    // the first matching slot and a second one would be invisible.
    let outlet = ItemId::from(ids::OUTLET);
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        let held = inv.count(&outlet);
        inv.take(outlet.clone(), held);
        inv.add(outlet.clone(), 1);
    }
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.hp = 1;
    }
    {
        let mut reserve = game.world.get_mut::<PowerReserve>(player).unwrap();
        *reserve = PowerReserve::new(10.0);
    }
    game
}

fn any_combatant_is_a_stack_spawn(game: &Game) -> bool {
    let state = game.world.resource::<BattleState>();
    state
        .groups
        .iter()
        .flat_map(|g| g.members.iter())
        .any(|&e| game.world.get::<StackSpawn>(e).is_some())
}
