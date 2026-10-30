use super::*;

/// A board with one cell of cover between the two halves, and the two bodies
/// placed either side of it.
///
/// Hand-written rather than generated: where the cover falls is what is being
/// tested, and `map::generate` puts it wherever the seed says.
fn across_cover(game: &mut Game, wild: Entity) -> ((i32, i32), (i32, i32)) {
    use crate::tactical::map::Board;

    let player = game.player_entity();
    let mut battle = game.world.resource_mut::<TacticalBattle>();
    battle.board = Board::from_rows(&[
        ".......", ".......", ".......", "...#...", ".......", ".......", ".......",
    ]);
    assert!(battle.move_to(player, (3, 1)), "the player would not stand");
    assert!(battle.move_to(wild, (3, 5)), "the hostile would not stand");
    ((3, 1), (3, 5))
}

/// **A routine may not be thrown at what the thrower cannot see.** The swing
/// has always checked sight; a routine checked only range, and since every
/// shipped area routine derives a six-cell throw, that meant every blast in
/// the game landed in full through a solid wall.
///
/// The same routine at a visible cell is run immediately afterwards, from the
/// same fight and the same turn — so the refusal cannot be passing on Power,
/// a cooldown or a range the fixture got wrong.
#[test]
fn a_blast_may_not_be_thrown_through_cover() {
    use crate::components::AbilityCooldowns;
    use crate::tests::support::HOSTILE_SWEEP;

    let mut game = game();
    let wild = tactical_fight(&mut game, 1, 200)[0];
    let (_, behind) = across_cover(&mut game, wild);
    let player = game.player_entity();
    only_routine(&mut game, player, HOSTILE_SWEEP);
    assert!(wait_for_turn(&mut game, player), "the fight ended early");
    let hp = hp_of(&game, wild);

    assert!(
        !game.tactical_use_routine(0, behind),
        "a blast was thrown through cover"
    );

    assert_eq!(hp_of(&game, wild), hp, "the refused blast still landed");
    assert!(
        game.world
            .get::<AbilityCooldowns>(player)
            .is_none_or(|c| !c.0.contains_key(HOSTILE_SWEEP)),
        "the refused blast armed its cooldown"
    );
    assert_eq!(
        game.tactical_actor(),
        Some(player),
        "the refused blast spent the turn"
    );
    assert!(
        game.tactical_use_routine(0, (3, 2)),
        "the same blast was refused at a cell in plain view, so the fixture \
         proves nothing about sight"
    );
}

/// The hostile side is held to the same rule, and the AI's aim is where that
/// lands: `best_aim` walks every cell in range and scores who the shape would
/// cover, so without the gate it picks the party's own cell through a wall.
///
/// A `Single` shape at range, because the blast half is already closed by
/// `shape_cells` — a `Radius` thrown through cover now covers nobody, so it
/// scores zero and is skipped whether or not the aim itself is gated. A shot
/// is the case that needs the gate.
///
/// The instrument is the **cooldown** and not the player's Integrity: a shot
/// that fired and missed leaves Integrity untouched too, so a test reading
/// damage would pass against a hostile firing through walls all day. The
/// hostile is then moved into plain sight and driven again, so a refusal that
/// was really the fixture failing to arm anything cannot pass either.
#[test]
fn a_hostile_will_not_shoot_through_cover() {
    use crate::abilities::{AbilityDb, AbilityRange, AbilityShape};
    use crate::components::AbilityCooldowns;

    let mut game = game();
    let wild = tactical_fight(&mut game, 1, 200)[0];
    across_cover(&mut game, wild);

    // A shipped single-target attack given the reach of a thrown one, and
    // stripped of its trigger: the shipped file authors no `range:` and so
    // derives arm's length, where no cell can lie between the two bodies and
    // sight can never be blocked at all.
    let mut def = game
        .world
        .resource::<AbilityDb>()
        .get("siphon_cycles")
        .expect("the shipped routine is loaded")
        .clone();
    def.id = "reaching_shot".to_string();
    def.shape = Some(AbilityShape::Single);
    def.range = Some(AbilityRange { min: 0, max: 6 });
    def.triggers = None;
    game.world.resource_mut::<AbilityDb>().insert(def);
    only_routine(&mut game, wild, "reaching_shot");

    let fired = |game: &Game| {
        game.world
            .get::<AbilityCooldowns>(wild)
            .is_some_and(|c| c.0.contains_key("reaching_shot"))
    };
    let drive = |game: &mut Game| {
        assert!(wait_for_turn(game, wild), "the fight ended early");
        // Pinned where it stands: left to walk it would step around the cover
        // and shoot from somewhere it can see, which is the rule working
        // rather than being skipped — and not what this measures.
        game.world
            .resource_mut::<TacticalBattle>()
            .commit_walk(Vec::new());
        game.tactical_ai_beat()
    };

    drive(&mut game);
    assert!(!fired(&game), "the hostile shot the player through a wall");

    // In plain view of the player, two cells short of the cover.
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(wild, (3, 2)),
        "the hostile would not stand in the open"
    );
    drive(&mut game);
    assert!(
        fired(&game),
        "the hostile would not shoot from a cell in plain view either, so the \
         refusal above says nothing about sight"
    );
}

/// **Staying put is the default.** A hostile that can already swing at the
/// player from where it stands spends its turn swinging, not shuffling to
/// another cell that swings exactly as well — which is what it did while
/// every cell beside the player tied and the softmax drew among all of them.
///
/// A packmate stands beside it on purpose: crowding is the term that would
/// otherwise talk it into a sidestep, and crowding alone is not a reason to
/// move. Several turns at the shipped temperature, because one draw landing
/// back on its own cell would pass a single turn.
#[test]
fn a_hostile_already_in_reach_holds_its_cell() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 2, 10_000);
    let wild = pack[0];
    open_ground(&mut game, &pack, &[(3, 4), (3, 3)]);

    for turn in 0..6 {
        assert!(wait_for_turn(&mut game, wild), "the fight ended early");
        assert!(game.tactical_ai_turn(), "the hostile's turn was not run");
        assert_eq!(
            cell_of(&game, wild),
            Some((3, 4)),
            "turn {turn}: a hostile already in reach walked anyway"
        );
    }
}

/// Holding is not freezing: a hostile out of reach still closes.
#[test]
fn a_hostile_out_of_reach_still_closes() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 10_000);
    let wild = pack[0];
    let player = open_ground(&mut game, &pack, &[(0, 4)]);
    assert!(wait_for_turn(&mut game, wild), "the fight ended early");
    let before = crate::tactical::reach::distance(player, (0, 4));

    assert!(game.tactical_ai_turn(), "the hostile's turn was not run");

    let after = crate::tactical::reach::distance(
        player,
        cell_of(&game, wild).expect("the hostile left the board"),
    );
    assert!(
        after < before,
        "the hostile stood {before} cells off and stayed at {after}"
    );
}

/// In range is not the same as able to act: a reaching hostile in band but
/// behind cover has a reason to move, and moves to a cell it can see from.
#[test]
fn a_hostile_in_range_but_blind_steps_into_sight() {
    let mut game = game();
    let wild = fight_against(&mut game, "drone");
    let player = open_ground(&mut game, &[wild], &[(4, 2)]);
    block_cell(&mut game, (4, 3));
    assert_eq!(
        game.swing_range(wild),
        2,
        "the fixture stands the drone at exactly its reach"
    );
    assert!(wait_for_turn(&mut game, wild), "the fight ended early");

    assert!(game.tactical_ai_turn(), "the hostile's turn was not run");

    let stood = cell_of(&game, wild).expect("the hostile left the board");
    let battle = game.world.resource::<TacticalBattle>();
    assert_ne!(
        stood,
        (4, 2),
        "a hostile that could not see its target held"
    );
    assert!(
        crate::tactical::reach::line_of_sight(&battle.board, stood, player),
        "the hostile moved to {stood:?}, which cannot see the player either"
    );
}

/// The outline this feeds `render/tactical.rs` cannot disagree with the
/// refusal `tactical_use_routine` gives at the keyboard: a cell out of
/// range and a cell out of sight are both excluded, and a cell the action
/// accepts is included.
///
/// `bus_fault` (`AllEnemies`) derives a six-cell range and a two-cell
/// radius — wide enough that a hand-written board can hold an out-of-range
/// corner, a cell behind cover, and a cell in plain view all inside one
/// fixture.
#[test]
fn placeable_cells_match_what_the_routine_accepts_and_refuses() {
    use crate::tactical::map::Board;
    use crate::tests::support::HOSTILE_SWEEP;

    let mut game = game();
    let wild = tactical_fight(&mut game, 1, 200)[0];
    let player = game.player_entity();
    only_routine(&mut game, player, HOSTILE_SWEEP);

    {
        let mut battle = game.world.resource_mut::<TacticalBattle>();
        battle.board = Board::from_rows(&[
            ".........",
            ".........",
            ".........",
            ".#.......",
            ".........",
            ".........",
            ".........",
            ".........",
            ".........",
        ]);
        assert!(battle.move_to(player, (1, 1)), "the player would not stand");
        assert!(battle.move_to(wild, (1, 6)), "the hostile would not stand");
    }
    assert!(wait_for_turn(&mut game, player), "the fight ended early");

    let out_of_range = (8, 8); // Chebyshev 7 from (1,1); the range caps at 6.
    let behind_cover = (1, 6); // straight down column 1, past the `#` at (1,3).
    let in_the_open = (1, 2); // one cell down, nothing between.

    let placeable = game.tactical_placeable_cells(0);
    assert!(
        !placeable.is_empty(),
        "an empty set would prove nothing about the ones excluded below"
    );

    assert!(
        !placeable.contains(&out_of_range),
        "an out-of-range cell was offered as a legal centre"
    );
    assert!(
        !game.tactical_use_routine(0, out_of_range),
        "the action accepted a cell the outline would have refused"
    );

    assert!(
        !placeable.contains(&behind_cover),
        "a cell behind cover was offered as a legal centre"
    );
    assert!(
        !game.tactical_use_routine(0, behind_cover),
        "the action accepted a cell the outline would have refused"
    );

    assert!(
        placeable.contains(&in_the_open),
        "a cell in range and in plain view was left off the outline"
    );
    assert!(
        game.tactical_use_routine(0, in_the_open),
        "the outline offered a cell the action then refused"
    );
}

/// A `Single` shape resolves onto whoever is standing at the aimed cell
/// rather than a centre the player chooses freely, and a `Line`/`Cone` is
/// aimed as a direction — neither has a "legal centre" to outline, so both
/// are left alone rather than forced into a cell set that would read as
/// every cell in range.
#[test]
fn placeable_cells_are_empty_for_a_single_target_routine() {
    let mut game = game();
    tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    only_routine(&mut game, player, "priority_boost");
    assert!(wait_for_turn(&mut game, player), "the fight ended early");

    assert!(
        game.tactical_placeable_cells(0).is_empty(),
        "a Single shape has no centre to outline"
    );
}

// --- Partial cover -----------------------------------------------------
//
// `reach::cover_between` needs no `Game` at all: it is a pure function of a
// board and two cells, so these are bare `Board`s and direct calls.

/// Attacker at (3,1), defender at (3,5), and whatever `rows` says between
/// them.
fn cover_board(rows: &[&str]) -> crate::tactical::map::Board {
    crate::tactical::map::Board::from_rows(rows)
}

#[test]
fn a_boulder_in_the_arc_gives_cover() {
    let board = cover_board(&[
        ".......", ".......", ".......", ".......", "..#....", ".......", ".......",
    ]);
    assert!(
        crate::tactical::reach::cover_between(&board, (3, 1), (3, 5)),
        "a boulder on the attacker's side of the defender is cover"
    );
}

#[test]
fn a_boulder_behind_the_defender_is_not_cover() {
    let board = cover_board(&[
        ".......", ".......", ".......", ".......", ".......", ".......", "..#....",
    ]);
    assert!(
        !crate::tactical::reach::cover_between(&board, (3, 1), (3, 5)),
        "a boulder on the far side shields nothing"
    );
}

/// The dot product is **strictly** positive, and this is the test that says
/// so: a boulder exactly abeam of the defender scores zero, and a `>= 0`
/// comparison would hand out cover for standing next to a rock.
#[test]
fn a_boulder_exactly_abeam_is_not_cover() {
    let board = cover_board(&[
        ".......", ".......", ".......", ".......", ".......", "..#....", ".......",
    ]);
    assert!(
        !crate::tactical::reach::cover_between(&board, (3, 1), (3, 5)),
        "ninety degrees off the bearing is beside you, not between you and the shot"
    );
}

#[test]
fn cover_does_nothing_at_melee_range() {
    let board = cover_board(&[
        ".......", ".......", ".......", ".......", "..#....", ".......", ".......",
    ]);
    assert_eq!(
        crate::tactical::reach::distance((3, 4), (3, 5)),
        crate::tuning::TACTICAL_MELEE_RANGE,
        "the fixture is meant to sit exactly on the melee band"
    );
    assert!(
        !crate::tactical::reach::cover_between(&board, (3, 4), (3, 5)),
        "a boulder is no help against someone standing on top of you"
    );
}

#[test]
fn a_defender_out_of_sight_has_no_cover() {
    let board = cover_board(&[
        ".......", ".......", ".......", "...#...", "..#....", ".......", ".......",
    ]);
    assert!(
        !crate::tactical::reach::cover_between(&board, (3, 1), (3, 5)),
        "a shot that cannot be taken needs no modifier"
    );
}

/// **Cover that no generated board produces is a feature that ships green
/// and dead**, which this repo has shipped before. Measured over full
/// enumeration on 2026-09-17, as the share of ordered walkable pairs beyond
/// melee range *with line of sight* whose defender has cover:
///
/// | biome | `Cover` weight | share |
/// |---|---:|---:|
/// | OpenGrid | 4 | 12.7% |
/// | Deadlock | 7 | 22.3% |
/// | NullSector | 8 | 22.8% |
/// | Backplane | 20 | 43.2% |
///
/// Sighted pairs is the right denominator: a pair with no line of sight has
/// no attack to modify. Note Backplane has the *lowest* share of all pairs
/// and the highest of sighted ones — dense cover blocks most long sightlines
/// outright, so the shots that remain are mostly covered ones.
///
/// The sweep here samples attackers rather than exhausting them, to stay
/// cheap; the floor is well below every measured figure.
#[test]
fn cover_is_reachable_on_every_biome_a_fight_opens_on() {
    use crate::tactical::map::{BattleSpec, generate};
    use crate::tactical::reach::{cover_between, distance, line_of_sight};
    use crate::tuning::TACTICAL_MELEE_RANGE;
    use crate::world::Biome;

    let (props, _) = crate::tactical::props::PropDb::load_dir(
        &crate::tests::support::test_assets_dir().join("battle-props"),
    )
    .unwrap();
    for biome in [
        Biome::OpenGrid,
        Biome::Deadlock,
        Biome::NullSector,
        Biome::Backplane,
    ] {
        let (mut sighted, mut covered) = (0u32, 0u32);
        for seed in 1..=3u32 {
            for bodies in [2u32, 5, 8] {
                let board = generate(
                    BattleSpec {
                        world_seed: seed,
                        site: (seed as i32, 0),
                        tick: u64::from(seed) * 17,
                        zone: 1,
                        biome,
                        bodies,
                    },
                    &props,
                );
                let walkable: Vec<(i32, i32)> = board
                    .cells()
                    .map(|(cell, _)| cell)
                    .filter(|&(x, y)| board.walkable(x, y))
                    .collect();
                let step = (walkable.len() / 60).max(1);
                for &attacker in walkable.iter().step_by(step) {
                    for &defender in &walkable {
                        if distance(attacker, defender) <= TACTICAL_MELEE_RANGE
                            || !line_of_sight(&board, attacker, defender)
                        {
                            continue;
                        }
                        sighted += 1;
                        if cover_between(&board, attacker, defender) {
                            covered += 1;
                        }
                    }
                }
            }
        }
        assert!(sighted > 0, "{biome:?} produced no shots at all");
        let share = f64::from(covered) / f64::from(sighted);
        assert!(
            share >= 0.10,
            "{biome:?}: only {:.1}% of takeable shots are at a defender in cover, \
             so the feature is close to unreachable there",
            share * 100.0
        );
    }
}

/// Which attacks cover applies to. The fixture is `combat_status`'s, since
/// these are questions about a swing rather than about a board.
mod cover_by_shape {
    use super::*;
    use crate::abilities::AbilityShape;
    use crate::battle;

    fn shaped_swing(game: &Game, body: Entity, shape: AbilityShape) -> battle::Swing {
        battle::Swing {
            cover_ignored: shape.ignores_cover(),
            ..battle::Swing::plain(game.natural_range_of(body))
        }
    }

    fn covered(game: &Game, attacker: Entity, defender: Entity, shape: AbilityShape) -> bool {
        let swing = shaped_swing(game, defender, shape);
        game.defender_profile_against(attacker, defender, swing)
            .evasion
            > game.combatant_profile(defender, swing).evasion
    }

    /// A blast flushes a body out from behind its boulder.
    #[test]
    fn a_blast_ignores_cover() {
        let (game, player, wild) = crate::tests::combat_status::cover_fight(
            904,
            &crate::tests::combat_status::COVERED_BOARD,
            (4, 1),
            (4, 4),
        );
        assert!(!covered(
            &game,
            player,
            wild,
            AbilityShape::Radius { radius: 1 }
        ));
    }

    /// Everything that is a shot rather than a blast is refused by cover.
    #[test]
    fn every_other_shape_is_penalised_by_cover() {
        let (game, player, wild) = crate::tests::combat_status::cover_fight(
            904,
            &crate::tests::combat_status::COVERED_BOARD,
            (4, 1),
            (4, 4),
        );
        for shape in [
            AbilityShape::Single,
            AbilityShape::Line { length: 4 },
            AbilityShape::Cone {
                length: 4,
                degrees: 90,
            },
        ] {
            assert!(
                covered(&game, player, wild, shape),
                "{shape:?} should be refused by cover"
            );
        }
    }

    /// **The polarity regression.** `false` is "cover applies", so a
    /// `Swing::default()` written later cannot switch the feature off.
    #[test]
    fn a_default_swing_still_honours_cover() {
        assert!(!battle::Swing::default().cover_ignored);
    }
}

/// What the screen is told about cover.
mod cover_telegraph {
    use super::*;
    use crate::tests::combat_status::{COVERED_BOARD, cover_fight};

    /// **The test the spec asks for by name, and the one that must not be
    /// weakened.** The mark and the roll are two calls into one rule; over
    /// every ordered pair on a board, they answer the same thing.
    #[test]
    fn the_telegraph_agrees_with_the_roll() {
        let (game, player, wild) = cover_fight(905, &COVERED_BOARD, (4, 1), (4, 4));
        let mut agreed = 0;
        for (attacker, defender) in [(player, wild), (wild, player)] {
            let swing = crate::battle::Swing::plain(game.natural_range_of(defender));
            let raised = game
                .defender_profile_against(attacker, defender, swing)
                .evasion
                > game.combatant_profile(defender, swing).evasion;
            assert_eq!(
                game.body_in_cover(attacker, defender),
                raised,
                "the mark and the roll disagree"
            );
            agreed += u32::from(raised);
        }
        assert_eq!(
            agreed, 1,
            "the fixture should shelter exactly one of the two, or it proves nothing"
        );
    }

    /// The standing mark is a hostile's turn only — on the player's own,
    /// nothing has been aimed yet.
    #[test]
    fn the_standing_mark_is_a_hostiles_turn_only() {
        let (mut game, player, wild) = cover_fight(906, &COVERED_BOARD, (4, 4), (4, 1));
        assert!(wait_for_turn(&mut game, player), "the fight ended early");
        let view = game.tactical_view().expect("a fight is open");
        assert!(
            view.bodies.iter().all(|b| !b.in_cover),
            "the player's own turn lit a standing mark"
        );
        assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
        let view = game.tactical_view().expect("a fight is open");
        let marked = view
            .bodies
            .iter()
            .find(|b| b.entity == player)
            .expect("the player is on the board");
        assert!(
            marked.in_cover,
            "the boulder shelters the player from the hostile whose turn it is"
        );
        assert_eq!(
            marked.in_cover,
            game.body_in_cover(wild, player),
            "the mark disagrees with the door it is a call into"
        );
    }

    /// A reachable cell that would shelter the acting body is washed, and
    /// one that would not is left alone.
    #[test]
    fn a_covered_destination_is_marked() {
        let (mut game, _, wild) = cover_fight(907, &COVERED_BOARD, (4, 1), (4, 6));
        assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
        let view = game.tactical_view().expect("a fight is open");
        assert!(
            !view.covered.is_empty(),
            "nothing on this board sheltered the hostile from the player"
        );
        for cell in &view.covered {
            assert!(
                view.reachable.contains(cell),
                "a covered cell that cannot be walked to was marked"
            );
        }
    }

    /// A finished fight is a result screen, not a resumed one.
    #[test]
    fn a_finished_fight_marks_nothing() {
        let (mut game, _, wild) = cover_fight(908, &COVERED_BOARD, (4, 1), (4, 6));
        assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
        let frozen = game.tactical_view().expect("a fight is open").frozen();
        assert!(frozen.covered.is_empty());
        assert!(frozen.bodies.iter().all(|b| !b.in_cover));
    }
}
