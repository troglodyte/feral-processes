use super::*;
use crate::*;

/// Replaces the player's whole inventory with `stock`, so a taming test
/// states exactly which catalysts are on hand instead of inheriting
/// whatever `Game::new`'s starting kit holds.
/// Tops the player's reserve back up to `POWER_MAX`.
///
/// A hand-compile's ticks drain Power like any others and
/// `tuning::HAND_CRAFT_POWER_FLOOR` refuses a batch that would run it out,
/// so a fixture that compiles two batches in a row has to restock the
/// reserve exactly as it restocks the pack — the second batch is otherwise
/// refused for a reason the test is not about.
pub(crate) fn fill_power(game: &mut Game) {
    let player = game.player_entity();
    game.world
        .get_mut::<PowerReserve>(player)
        .unwrap()
        .fill(crate::components::POWER_MAX);
}

pub(crate) fn set_inventory(game: &mut Game, stock: &[(&str, u32)]) {
    let player = game.player_entity();
    let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
    inv.items.clear();
    for (id, qty) in stock {
        inv.add(ItemId::from(*id), *qty);
    }
}

/// Retries `battle_flee` until the escape roll lands, for the tests that
/// are about what a *successful* jack-out does rather than about the roll.
/// Jacking out stopped being guaranteed (see `battle::jack_out_chance`), so
/// a single call is no longer enough to end a battle.
///
/// Bounded rather than looping forever: the chance floors at
/// `JACK_OUT_CHANCE_MIN`, so 200 straight failures is a ~1e-9 event and
/// means something is actually broken. Bails out if the battle ends under
/// it — a failed attempt draws a full volley, which can flatline the party.
pub(crate) fn flee_until_clear(game: &mut Game) {
    for _ in 0..200 {
        if game.battle_flee() {
            return;
        }
        if !game.has_active_battle() {
            return;
        }
    }
    panic!("200 jack-out attempts all failed — the escape roll is broken");
}

/// Dives the party from the surface to `depth` through the real descent
/// path — an entrance under the player's feet, then one `Game::descend` per
/// frame — teleporting only *within* a frame to stand on its way down, which
/// is the one thing a test can't be asked to walk a maze for.
///
/// `Locale` is never hand-written into a different depth: `enter_frame` is
/// the one way into a frame and a test that skipped it would be asserting
/// against a world no player can reach.
/// The entrance chosen is far enough from the zone's spawn point that its
/// stack actually runs `depth` frames deep — `stack::frames_for` grows the
/// count with distance, and the shallow stacks beside the spawn have no
/// bottom to reach.
pub(crate) fn dive_to_depth(game: &mut Game, depth: u32) {
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let entrance = (0..500)
        .map(|step| (pos.x + step, pos.y))
        .find(|&(x, y)| {
            game.frames_at((x, y)) >= depth
                && game.world.resource_mut::<WorldMap>().tile(x, y).walkable
        })
        .expect("somewhere along this row there should be a stack deep enough");
    game.enter_stack(entrance.0, entrance.1);
    for _ in 1..depth {
        stand_on_link_down(game);
        game.descend();
    }
    let Locale::Stack { depth: reached, .. } = game.locale() else {
        panic!("the dive should have left the party underground");
    };
    assert_eq!(reached, depth, "the dive did not reach the asked-for depth");
}

/// Teleports the party onto the current frame's way down, so a test about
/// descending doesn't have to walk the maze to find it.
fn stand_on_link_down(game: &mut Game) {
    let down = game
        .world
        .resource::<CurrentStack>()
        .0
        .as_ref()
        .unwrap()
        .link_down
        .expect("every frame a test dives through should have a way down");
    let Locale::Stack {
        depth,
        frames,
        facing,
        entrance,
        ..
    } = game.locale()
    else {
        unreachable!("not underground")
    };
    game.world.insert_resource(Locale::Stack {
        depth,
        frames,
        x: down.0,
        y: down.1,
        facing,
        entrance,
    });
}

/// Spawns a wild program on the player's tile and opens an intrusion on
/// it — the state `battle_decompile` needs.
pub(crate) fn start_battle_with_a_wild_program(game: &mut Game) -> Entity {
    let wild = spawn_wild_on_player_tile(game);
    game.start_battle(vec![wild]);
    wild
}

/// `Game::spawn_wild_creature` through `species`, with whatever routine the
/// `WILD_ROUTINE_CHANCE` roll gave it immediately cleared — for a fixture
/// that asserts on enemy *move* behaviour and can't tolerate the roll
/// occasionally handing it a carrier instead. A carrier's routine acting
/// differently from a move is loud, not silent (a kill or damage assertion
/// fails rather than passes), but loud is still a mystery failure in an
/// unrelated test the day the RNG stream shifts.
pub(crate) fn spawn_wild_without_routine(game: &mut Game, species: &str, x: i32, y: i32) -> Entity {
    let entity = game
        .spawn_wild_creature(species, x, y)
        .unwrap_or_else(|| panic!("{species} ships with the game"));
    game.world.entity_mut(entity).insert(Routines::default());
    entity
}

pub(crate) fn spawn_tamed(game: &mut Game, hp: i32, atk: i32) -> Entity {
    let species = generic_species();
    // Through `roster_parts` rather than a hand-written tuple, for the reason
    // `work_node_parts` exists: a fixture short a component reads as the
    // feature being broken rather than as the fixture being wrong.
    let parts = game.roster_parts();
    let entity = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Position { x: 3, y: 3 },
            Glyph {
                ch: species.glyph,
                color: species.color,
            },
            Stats {
                hp,
                max_hp: hp,
                atk,
                mitigation: 1,
            },
            parts,
        ))
        .id();
    // Pinned neutral, `Potential::NEUTRAL`'s reason. `roster_parts` mints a
    // disposition derived from the program's id, so four fixtures in a row
    // take four different temperaments — and every test asserting an
    // authored need-drain rate or an exact morale figure would then be
    // measuring a multiplier it never asked for. A test about a disposition
    // says so by inserting one; every other test wants the base formula.
    game.world
        .entity_mut(entity)
        .insert(crate::disposition::Disposition::Steady);
    // The one component `roster_parts` deliberately does *not* mint — every
    // real door into the roster goes through `spawn_wild_creature_scaled`
    // first, which is where a body's attributes come from, and this fixture
    // skips it. Without this a fixture body saves an empty map and mints
    // five on the way back in, which is the old-save arm firing on a save
    // written today.
    let attrs = crate::attributes::mint(
        game.world.resource::<crate::attributes::AttributeDb>(),
        crate::attributes::body_seed(
            game.world.resource::<crate::world::WorldMap>().seed(),
            3,
            3,
            species.id.as_str(),
            1,
        ),
        &species.attributes,
    );
    game.world.entity_mut(entity).insert(attrs);
    game.install_innate_routines(entity);
    // Last, like every real door: a seated program's figures derive from its
    // base, so a fixture that seated early would have its setup recomputed.
    game.seat_derived(entity);
    entity
}

/// Gives a seated program exactly these figures: unseats it, writes `Stats`
/// and seats it again, so the base is derived from the figures asked for. A
/// write to `Stats` alone is discarded by the next recompute.
pub(crate) fn reseat_with_stats(game: &mut Game, entity: Entity, max_hp: i32, atk: i32, def: i32) {
    game.world
        .entity_mut(entity)
        .remove::<crate::components::ProgramBase>();
    let mut s = game.world.get_mut::<Stats>(entity).unwrap();
    s.hp = max_hp;
    s.max_hp = max_hp;
    s.atk = atk;
    s.mitigation = def;
    game.seat_derived(entity);
}

/// `spawn_tamed` with a `ZonePortal` set, so a test can pin the depth a
/// program was caught at without reaching into the ECS itself — e.g. which
/// side of `programs_for_build`'s `>=` floor it lands on.
pub(crate) fn tame_at_zone(game: &mut Game, zone: u32) -> Entity {
    let entity = spawn_tamed(game, 10, 3);
    game.world.entity_mut(entity).insert(ZonePortal(zone));
    entity
}

/// `spawn_tamed` at a chosen tile and carrying a `Glyph`, so `view_entities`
/// can see it.
///
/// That query is `(Entity, &Position, &Glyph)`, and `spawn_tamed` grants no
/// glyph — a program spawned by it is invisible to every map-facing view,
/// which reads as the view filtering it out rather than as the fixture
/// being short a component.
pub(crate) fn spawn_tamed_on_map(game: &mut Game, x: i32, y: i32) -> Entity {
    let entity = spawn_tamed(game, 10, 3);
    game.world.entity_mut(entity).insert(Glyph {
        ch: 'd',
        color: GlyphColor::Cyan,
    });
    let mut pos = game.world.get_mut::<Position>(entity).unwrap();
    pos.x = x;
    pos.y = y;
    entity
}

/// Spawns a minimal wild (untamed, `Hostile`) `Creature` on the
/// player's own tile, suitable to pass straight into `start_battle` —
/// mirrors `spawn_tamed`'s pattern but without `Tamed`/`Experience`,
/// since a wild pack member has neither.
pub(crate) fn spawn_wild_on_player_tile(game: &mut Game) -> Entity {
    let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .expect("at least one species");
    let entity = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position {
                x: player_pos.x,
                y: player_pos.y,
            },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 0,
                mitigation: 1,
            },
            // The real spawner (`spawning.rs`) always gives a hostile one,
            // and `Game::arm_status` is a silent no-op without it — so a
            // fixture short this component makes every Debuff landing on
            // this creature disappear, and reads as the debuff not working.
            StatusEffects::default(),
        ))
        .id();
    // `spawn_tamed`'s reason, on the wild side: the real spawner mints
    // these, and a fixture body that saves an empty map and mints five on
    // the way back in fires the old-save arm on a save written today.
    let attrs = crate::attributes::mint(
        game.world.resource::<crate::attributes::AttributeDb>(),
        crate::attributes::body_seed(
            game.world.resource::<crate::world::WorldMap>().seed(),
            player_pos.x,
            player_pos.y,
            species.id.as_str(),
            1,
        ),
        &species.attributes,
    );
    game.world.entity_mut(entity).insert(attrs);
    entity
}

/// The wild-boss counterpart of `spawn_wild_on_player_tile`, for the tests
/// that assert a boss can never reach the roster. Its stats come from the
/// real species rather than the token 10 HP above, because `atk: 0` is
/// exactly what stops those tests noticing if a refused round resolves
/// anyway.
pub(crate) fn spawn_boss_on_player_tile(game: &mut Game) -> Entity {
    let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let boss = game
        .species_defs()
        .into_iter()
        .find(|s| s.is_boss)
        .expect("at least one boss species should ship in assets/species");
    game.spawn_wild_creature(&boss.id, player_pos.x, player_pos.y)
        .expect("a shipped boss species should spawn")
}

/// Opens a lair fight in the frame the party is standing in, against a
/// guardian the game will actually let the player decompile.
///
/// A real `rouse_lair` can no longer produce one at all: a guardian is a
/// boss whatever it was drawn from — `pick_lair_species` marks its ordinary
/// fallback as one too, since a guardian that is not a boss pays no Portal
/// Fragments — and a boss is refused as a decompile target before the roll.
/// So this installs the state by hand rather than reaching for a case the
/// game still has, naming exactly what `rouse_lair` writes: the pack in the
/// fight, `StackSpawn` on it, and `BattleState::lair` pointing at the
/// guardian.
///
/// Do not "fix" this by making some guardians tameable.
/// `Game::is_boss_creature` refusing a decompile is the rule; this fixture
/// reaching around it is a fixture.
///
/// Softened to 1 HP, so a test about what a capture *does* is not also a
/// test of whether the roll lands.
pub(crate) fn rouse_a_tameable_guardian(game: &mut Game) -> Entity {
    let player = game.player_entity();
    let pos = game
        .stack_pos()
        .expect("a lair fight belongs to a frame — descend first");
    let tile = *game.world.get::<Position>(player).unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss)
        .expect("the shipped roster is not all bosses");
    let guardian = game
        .spawn_wild_creature(&species.id, tile.x, tile.y)
        .expect("a shipped species should spawn");
    game.world.entity_mut(guardian).insert(StackSpawn);
    game.world.get_mut::<Stats>(guardian).unwrap().hp = 1;
    insert_battle(game, player, vec![guardian]);
    game.world.resource_mut::<BattleState>().lair =
        Some(crate::resources::LairFight { pos, guardian });
    guardian
}

/// Stands a Home on base space's exit cell and puts the party on it —
/// `Game::rest` is a base action and requires a rest-enabling structure in
/// reach, so tests exercising `rest` need both halves.
///
/// Both, and not just the structure, because either alone leaves `rest`
/// refused: the locale guard and the reach check are two separate things a
/// fixture has to satisfy, and a helper that supplied one of them would make
/// every caller restate the other. A test *about* the locale overwrites it
/// afterwards.
///
/// Spawned directly rather than through `place_structure` to sidestep its
/// cost and one-Home-only requirements, which aren't what these tests are
/// about.
pub(crate) fn stand_in_base_beside_home(game: &mut Game) {
    game.world.spawn((
        Structure {
            kind: "home".to_string(),
        },
        Position {
            x: TEST_HOME_CELL.0,
            y: TEST_HOME_CELL.1,
        },
    ));
    stand_in_base(game);
}

/// Sets up a single-round battle with one companion (stunned or not)
/// and returns how much the player's Power dropped from commanding
/// it. Shared by the two cost tests below.
pub(crate) fn power_spent_commanding_companion(seed: u32, stunned: bool) -> f32 {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 20);
    enlist(&mut game, companion);
    if stunned {
        game.world.entity_mut(companion).insert(StatusEffects {
            active: vec![ActiveStatus {
                id: crate::statuses::StatusId::from("stun"),
                remaining: 1,
                power: 0,
                stacks: 1,
                landed_this_round: false,
            }],
        });
    }

    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .expect("at least one species");
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 5, y: 5 },
            Stats {
                hp: 100,
                max_hp: 100,
                atk: 0,
                mitigation: 0,
            },
            StatusEffects::default(),
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);

    // Start off the cap. Power drains per tick, and both arms
    // of the comparison are supposed to absorb one tick's worth identically
    // — which they only do if neither is clamped at either end.
    *game.world.get_mut::<PowerReserve>(player).unwrap() =
        PowerReserve::new(50.0, crate::components::POWER_MAX);
    let power_before = game.world.get::<PowerReserve>(player).unwrap().get();
    companion_uses_special(
        &mut game,
        companion,
        0,
        battle::SpecialTarget::Ally { slot: 0 },
    );
    let power_after = game.world.get::<PowerReserve>(player).unwrap().get();
    power_before - power_after
}
