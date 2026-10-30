use super::*;

/// The `Squad` arm inside the existing `Game::effective_atk` door. A bare
/// fixture rather than a real `Game::spawn_squad`, since this arm only ever
/// reads the entity's own `Squad`-presence and `Stats`.
mod squad_effective_atk {
    use super::*;

    fn squad_body(game: &mut Game, hp: i32, max_hp: i32, atk: i32) -> Entity {
        game.world
            .spawn((
                Creature {
                    species: generic_species().id,
                },
                Hostile,
                Stats {
                    hp,
                    max_hp,
                    atk,
                    mitigation: 0,
                },
                Squad {
                    members: Vec::new(),
                    formation: 0,
                },
            ))
            .id()
    }

    #[test]
    fn a_squads_attack_falls_with_its_own_integrity() {
        let mut game = game();
        let full = squad_body(&mut game, 100, 100, 40);
        assert_eq!(
            game.effective_atk(full),
            40,
            "a squad at full Integrity should hit for its raw atk"
        );

        let half = squad_body(&mut game, 50, 100, 40);
        assert_eq!(
            game.effective_atk(half),
            20,
            "a squad at half Integrity should hit for half its raw atk"
        );

        let empty = squad_body(&mut game, 0, 100, 40);
        assert_eq!(
            game.effective_atk(empty),
            0,
            "a squad at zero Integrity should hit for nothing"
        );
    }

    /// The door's existing behaviour for anything without a `Squad` — a
    /// wild body's `effective_atk` is untouched by this arm.
    #[test]
    fn a_lone_body_is_untouched_by_the_squad_scale() {
        let mut game = game();
        let lone = body(&mut game, &generic_species().id);
        assert_eq!(game.effective_atk(lone), 3);
    }
}

/// Squads forming and fighting, on a real board.
mod squads {
    use super::*;

    /// Nine of a species (`tactical_pack`'s own baseline: one shared
    /// species, `atk: 1`, `mitigation: 0`) fold into one squad body and
    /// four singles, with the stat block the spec's table describes.
    #[test]
    fn nine_of_a_species_fold_into_a_squad_with_the_summed_stat_block() {
        let mut game = game();
        let pack = tactical_fight(&mut game, 9, 10);

        let board_bodies: Vec<Entity> = {
            let battle = game.world.resource::<TacticalBattle>();
            battle.bodies().map(|(e, _)| e).collect()
        };
        let squads: Vec<Entity> = board_bodies
            .iter()
            .copied()
            .filter(|&e| game.world.get::<Squad>(e).is_some())
            .collect();
        assert_eq!(squads.len(), 1, "9 of a kind should seat exactly one squad");
        let squad = squads[0];

        let stats = *game.world.get::<Stats>(squad).unwrap();
        assert_eq!(stats.max_hp, 50, "summed max_hp over 5 members at 10 each");
        assert_eq!(stats.hp, 50);
        let formation = &crate::tuning::FORMATIONS[0];
        // 5 members at `atk: 1` each (`tactical_pack`'s baseline).
        let expected_atk = (5_f32 * formation.swing_share).round() as i32;
        assert_eq!(stats.atk, expected_atk);
        assert_eq!(stats.mitigation, 0, "the members' highest, all zero here");

        {
            let battle = game.world.resource::<TacticalBattle>();
            assert_eq!(battle.footprint_of(squad), formation.footprint);
            assert_eq!(
                battle.cells_of(squad).len(),
                (formation.footprint as usize).pow(2)
            );
        }
        assert_eq!(game.actions_per_turn(squad), formation.actions);

        // The board holds the player, one squad and four leftover singles —
        // nine wild bodies never became six board occupants by accident.
        assert_eq!(board_bodies.len(), 1 + 1 + 4);
        let squad_members = game.world.get::<Squad>(squad).unwrap().members.clone();
        assert_eq!(squad_members.len(), 5);
        for &member in &squad_members {
            assert!(
                pack.contains(&member),
                "a squad's members must come from the pack it formed out of"
            );
            assert!(
                !board_bodies.contains(&member),
                "a squad's members must not also be placed on the board"
            );
        }
    }

    /// Four of a species is under the formation's threshold, so nothing
    /// folds — the ordinary one-cell, one-action case.
    #[test]
    fn four_of_a_species_never_folds_on_a_real_board() {
        let mut game = game();
        tactical_fight(&mut game, 4, 10);
        let battle = game.world.resource::<TacticalBattle>();
        assert!(
            battle
                .bodies()
                .all(|(e, _)| game.world.get::<Squad>(e).is_none())
        );
    }

    /// A squad gets its formation's two actions before the turn is handed
    /// on; an ordinary body still gets exactly one. `tactical_defend`
    /// rather than a swing, since it needs no range or target — only
    /// whether an action was spent.
    #[test]
    fn a_squad_spends_two_actions_before_the_turn_moves_on() {
        let mut game = game();
        let pack = tactical_pack(&mut game, 9, 40);
        game.open_tactical_battle(pack);
        let squad = {
            let battle = game.world.resource::<TacticalBattle>();
            battle
                .bodies()
                .map(|(e, _)| e)
                .find(|&e| game.world.get::<Squad>(e).is_some())
                .expect("9 of a kind must seat a squad")
        };
        assert!(
            wait_for_turn(&mut game, squad),
            "the squad never got a turn"
        );
        assert_eq!(game.world.resource::<TacticalBattle>().actions_left(), 2);

        assert!(game.tactical_defend(), "the first brace was refused");
        assert_eq!(
            game.tactical_actor(),
            Some(squad),
            "one of two actions spent must not hand the turn on"
        );

        assert!(game.tactical_defend(), "the second brace was refused");
        assert_ne!(
            game.tactical_actor(),
            Some(squad),
            "both actions spent must hand the turn on"
        );
    }

    /// A single body still gets exactly one action — the pre-squad
    /// behaviour must survive squads existing at all.
    #[test]
    fn a_single_body_still_gets_one_action() {
        let mut game = game();
        let pack = tactical_fight(&mut game, 1, 40);
        assert!(
            wait_for_turn(&mut game, pack[0]),
            "the lone body never got a turn"
        );
        assert_eq!(game.world.resource::<TacticalBattle>().actions_left(), 1);
    }

    /// **Nothing is seated on top of anything else, or on ground it cannot
    /// stand on.** `TacticalBattle::place` is the refusal that holds that,
    /// and it can only apply it to a body whose shape it already knows — so
    /// `open_tactical_battle_at` has to call `set_shape` *before* `place`,
    /// not after. Seated first and widened afterwards a squad's block is
    /// never checked at all, and the fight is only well-formed because
    /// `deploy::plan` reserves a clear block two files away.
    #[test]
    fn no_two_seated_footprints_overlap_and_all_of_them_stand_on_ground() {
        let mut game = game();
        let _pack = tactical_fight(&mut game, 9, 10);
        let battle = game.world.resource::<TacticalBattle>();

        let mut held: Vec<((i32, i32), Entity)> = Vec::new();
        for (body, _) in battle.bodies() {
            for cell in battle.cells_of(body) {
                assert!(
                    battle.board.walkable(cell.0, cell.1),
                    "{body:?} was seated on {cell:?}, which nothing can stand on"
                );
                if let Some((_, other)) = held.iter().find(|(at, _)| *at == cell) {
                    panic!("{body:?} and {other:?} both hold {cell:?}");
                }
                held.push((cell, body));
            }
        }
        assert!(
            battle.bodies().any(|(e, _)| battle.footprint_of(e) > 1),
            "fixture: a fight with no squad in it says nothing about blocks"
        );
    }

    /// A board is sized by the cells the fight puts on it, so a squad counts
    /// for its whole block rather than for one body — the plan's reader
    /// table for `BattleSpec::bodies`.
    #[test]
    fn a_squad_counts_for_its_whole_block_when_the_board_is_sized() {
        let mut game = game();
        let _pack = tactical_fight(&mut game, 9, 10);
        let battle = game.world.resource::<TacticalBattle>();
        let seated: u32 = battle
            .bodies()
            .map(|(e, _)| u32::from(battle.footprint_of(e)).pow(2))
            .sum();
        assert_eq!(
            battle.spec.bodies, seated,
            "the spec counted bodies where the board holds cells"
        );
    }

    /// **The opening bearing is unchanged by folding.** `squads::plan` runs
    /// inside `open_tactical_battle_at`, after the bearing is already in
    /// hand, and that placement is the whole of what keeps a squad away from
    /// the two sites it would degrade silently: `Game::gather_pack` answers
    /// a pack of one for an anchor with no `Position`, and
    /// `open_tactical_battle` derives its bearing from `pack[0]`'s tile,
    /// which a squad does not have — so a pack folded one call earlier is
    /// seated on a degenerate zero vector, with both ranks on the board's
    /// centre.
    ///
    /// Through the *deriving* door, and measured against a pack that cannot
    /// fold: four of a kind never make a set, so the two fights differ in
    /// whether anything folded and in nothing else.
    #[test]
    fn folding_a_pack_does_not_move_the_bearing_it_is_seated_on() {
        /// Mean x of the player's rank, then of the wild one.
        /// `tactical_pack` stands east of the player, so a bearing derived
        /// from the pack's own tiles seats the wild rank at the greater x.
        fn ranks(game: &Game) -> (f32, f32) {
            let battle = game.world.resource::<TacticalBattle>();
            let player = game.player_entity();
            let party: Vec<i32> = battle
                .bodies()
                .filter(|&(e, _)| e == player)
                .map(|(_, cell)| cell.0)
                .collect();
            let wild: Vec<i32> = battle
                .bodies()
                .filter(|&(e, _)| game.world.get::<Hostile>(e).is_some())
                .map(|(_, cell)| cell.0)
                .collect();
            assert!(!party.is_empty() && !wild.is_empty());
            (
                party.iter().sum::<i32>() as f32 / party.len() as f32,
                wild.iter().sum::<i32>() as f32 / wild.len() as f32,
            )
        }

        let mut folded = game();
        let pack = tactical_pack(&mut folded, 5, 10);
        folded.open_tactical_battle(pack);
        assert!(
            folded
                .world
                .resource::<TacticalBattle>()
                .bodies()
                .any(|(e, _)| folded.world.get::<Squad>(e).is_some()),
            "fixture: five of a kind must fold"
        );

        let mut unfolded = game();
        let pack = tactical_pack(&mut unfolded, 4, 10);
        unfolded.open_tactical_battle(pack);
        assert!(
            unfolded
                .world
                .resource::<TacticalBattle>()
                .bodies()
                .all(|(e, _)| unfolded.world.get::<Squad>(e).is_none()),
            "fixture: four of a kind must not fold"
        );

        let (folded_party, folded_wild) = ranks(&folded);
        let (plain_party, plain_wild) = ranks(&unfolded);
        assert_eq!(
            (folded_party, folded_wild),
            (plain_party, plain_wild),
            "folding moved the ranks the bearing seats"
        );
        assert!(
            plain_wild > plain_party,
            "fixture: the eastward bearing must seat the wild rank east"
        );
    }

    /// A blast covering two cells of one footprint hits it **once** —
    /// `reach::recipients`' `footprint_hit`, asked per body rather than per
    /// covered cell. Against a real squad, because task 1's own test for
    /// this used a hand-built cell list and stayed green with every
    /// footprint at one.
    #[test]
    fn a_blast_over_two_cells_of_a_squads_block_catches_it_once() {
        let mut game = game();
        let pack = tactical_pack(&mut game, 5, 40);
        game.open_tactical_battle(pack);
        let squad = seated_squad(&game);
        let player = game.player_entity();

        let anchor = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(squad)
            .expect("the squad is seated");
        let cells = crate::tactical::footprint_cells_at(anchor, 2);
        let shape = crate::abilities::AbilityShape::Radius { radius: 1 };
        let covered = crate::tactical::reach::shape_cells(
            &game.world.resource::<TacticalBattle>().board,
            anchor,
            anchor,
            shape,
        );
        assert!(
            cells.iter().filter(|c| covered.contains(c)).count() >= 2,
            "fixture: the blast must cover more than one cell of the block"
        );

        let caught = crate::tactical::reach::recipients(
            game.world.resource::<TacticalBattle>(),
            player,
            anchor,
            shape,
        );
        assert_eq!(
            caught.iter().filter(|&&e| e == squad).count(),
            1,
            "the squad was caught once per covered cell rather than once"
        );
    }

    /// The one squad a 9-of-a-kind pack seats.
    fn seated_squad(game: &Game) -> Entity {
        let battle = game.world.resource::<TacticalBattle>();
        battle
            .bodies()
            .map(|(e, _)| e)
            .find(|&e| game.world.get::<Squad>(e).is_some())
            .expect("9 of a kind must seat a squad")
    }

    /// **Every anchor the field offers is one `move_to` accepts.** That is
    /// the property `step_along_walk` rests on when it calls `Struck` and a
    /// refusal unreachable from a committed path — offered an anchor the
    /// board then refuses, a squad abandons the rest of its walk mid-path,
    /// silently, and the turn is owed to nobody.
    ///
    /// Against a *real* squad, because task 1's footprint unit tests used
    /// hand-built cell lists and stayed green with every footprint at one.
    #[test]
    fn every_anchor_a_squads_field_offers_is_one_move_to_accepts() {
        let mut game = game();
        let _pack = tactical_fight(&mut game, 9, 10);
        let squad = seated_squad(&game);
        let allowance = game.movement_allowance(squad);

        let (home, field) = {
            let battle = game.world.resource::<TacticalBattle>();
            let home = battle.cell_of(squad).expect("the squad is seated");
            let field: Vec<(i32, i32)> =
                crate::tactical::reach::movement_field(battle, squad, allowance)
                    .into_keys()
                    .collect();
            (home, field)
        };
        assert!(
            field.len() > 1,
            "a squad that can reach nowhere but the cell it stands on proves nothing"
        );

        for anchor in field {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            assert!(
                battle.move_to(squad, anchor),
                "the field offered {anchor:?}, which move_to refuses"
            );
            assert!(battle.move_to(squad, home), "the squad could not step back");
        }
    }

    /// A squad stands somewhere only if its *whole* footprint does:
    /// `move_to` accepts an anchor exactly when every cell of the block
    /// anchored there is walkable and free. Swept over the whole board, so
    /// both halves — ground nothing can stand on, and another body — are
    /// asserted against rather than assumed reachable, and the two counters
    /// below keep the sweep from passing vacuously.
    #[test]
    fn a_squad_stands_only_where_its_whole_footprint_does() {
        let mut game = game();
        let _pack = tactical_fight(&mut game, 9, 10);
        let squad = seated_squad(&game);

        let (home, side, others) = {
            let battle = game.world.resource::<TacticalBattle>();
            let others: Vec<(i32, i32)> = battle
                .bodies()
                .map(|(e, _)| e)
                .filter(|&e| e != squad)
                .flat_map(|e| battle.cells_of(e))
                .collect();
            (
                battle.cell_of(squad).expect("the squad is seated"),
                battle.board.side,
                others,
            )
        };
        assert_eq!(
            crate::tactical::footprint_cells_at(home, 2).len(),
            4,
            "fixture: the shipped formation is a 2x2"
        );

        let mut refused_for_ground = 0;
        let mut refused_for_a_body = 0;
        for y in 0..side {
            for x in 0..side {
                let anchor = (x, y);
                let cells = crate::tactical::footprint_cells_at(anchor, 2);
                let (on_ground, clear_of_bodies) = {
                    let battle = game.world.resource::<TacticalBattle>();
                    (
                        cells.iter().all(|&(cx, cy)| battle.board.walkable(cx, cy)),
                        cells.iter().all(|c| !others.contains(c)),
                    )
                };
                let legal = on_ground && clear_of_bodies;
                if !on_ground {
                    refused_for_ground += 1;
                }
                if on_ground && !clear_of_bodies {
                    refused_for_a_body += 1;
                }
                let mut battle = game.world.resource_mut::<TacticalBattle>();
                assert_eq!(
                    battle.move_to(squad, anchor),
                    legal,
                    "anchor {anchor:?}: footprint {cells:?}, on_ground {on_ground}, \
                     clear {clear_of_bodies}"
                );
                assert!(battle.move_to(squad, home), "the squad could not step back");
            }
        }
        assert!(
            refused_for_ground > 0,
            "the sweep never met ground a footprint could not stand on"
        );
        assert!(
            refused_for_a_body > 0,
            "the sweep never met another body to overlap"
        );
    }

    /// **The AI measures reach the way the door that honours it does.**
    /// `Game::tactical_attack` reads `reach::gap` over both footprints; a
    /// planner reading anchor-to-anchor declines the very swing that door
    /// would take. For a 2x2 squad anchored at A with the player at
    /// (A.x+2, A.y+1) the gap is 1 and the anchor distance is 2, so at
    /// melee reach the squad holds its ground and swings — a body that
    /// walks instead, or that finds nothing to swing at and hands the turn
    /// on, is the disagreement.
    #[test]
    fn a_squad_swings_from_a_cell_its_footprint_reaches_and_its_anchor_does_not() {
        let mut game = game();
        // `generic_species` and not the shipped roster's first entry: the
        // swing has to be a *melee* one for anchor distance 2 to be out of
        // reach at all, and a species whose basic attack is `ranged` would
        // make the whole fixture vacuous.
        game.world
            .resource_mut::<SpeciesDb>()
            .insert(generic_species());
        // Five of a kind: one squad and no leftover singles, so nothing
        // else on the wild side can spend a turn between the fight opening
        // and the squad's own.
        let at = *game.world.get::<Position>(game.player_entity()).unwrap();
        let pack: Vec<Entity> = (0..5)
            .map(|i| {
                game.world
                    .spawn((
                        Creature {
                            species: crate::tests::support::GENERIC_SPECIES_ID.to_string(),
                        },
                        Hostile,
                        Position {
                            x: at.x + 1 + i,
                            y: at.y,
                        },
                        Stats {
                            hp: 40,
                            max_hp: 40,
                            atk: 1,
                            mitigation: 0,
                        },
                        StatusEffects::default(),
                    ))
                    .id()
            })
            .collect();
        game.open_tactical_battle(pack);
        let squad = seated_squad(&game);
        let player = game.player_entity();
        assert_eq!(
            game.swing_range(squad),
            crate::tuning::TACTICAL_MELEE_RANGE,
            "fixture: a squad that swings further than one cell reaches the anchor distance anyway"
        );
        assert!(
            wait_for_turn(&mut game, squad),
            "the squad never got a turn"
        );

        let anchor = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(squad)
            .expect("the squad is seated");
        // Every cell two anchor-steps away whose nearest footprint cell is
        // one — the first that is free, since a generated board decides
        // which of them exists.
        let spot = [
            (anchor.0 + 2, anchor.1 + 1),
            (anchor.0 + 1, anchor.1 + 2),
            (anchor.0 + 2, anchor.1 + 2),
            (anchor.0 + 2, anchor.1 - 1),
            (anchor.0 - 1, anchor.1 + 2),
        ]
        .into_iter()
        .find(|&cell| {
            let battle = game.world.resource::<TacticalBattle>();
            battle.board.walkable(cell.0, cell.1) && battle.occupant(cell).is_none()
        })
        .expect("no free cell at gap 1 and anchor distance 2");
        assert_eq!(
            crate::tactical::reach::gap(&crate::tactical::footprint_cells_at(anchor, 2), &[spot]),
            1,
            "fixture: the player must stand one cell off the squad's block"
        );
        assert_eq!(
            crate::tactical::reach::distance(anchor, spot),
            2,
            "fixture: and two cells off its anchor"
        );
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(player, spot)
        );

        let beat = game.tactical_ai_beat();
        let battle = game.world.resource::<TacticalBattle>();
        assert_eq!(
            battle.cell_of(squad),
            Some(anchor),
            "the squad walked away from a swing it could already take: {beat:?}"
        );
        assert_eq!(
            battle.actions_left(),
            1,
            "the squad spent no action on a target its footprint reaches: {beat:?}"
        );
        assert_eq!(
            game.tactical_actor(),
            Some(squad),
            "the squad has a second action owed and should still be acting"
        );
    }

    /// **A squad is a combatant, so a condition has somewhere to live on
    /// it.** `Game::arm_status` is a documented silent no-op on a body with
    /// no `StatusEffects`, while `use_ability`'s `Debuff` arm logs
    /// unconditionally — so without the component the player reads that a
    /// squad's validation was stripped and nothing at all happened. The
    /// second assertion is `Exposed`, the rung with a live effect in every
    /// fight: read through `combatant_profile` it lowers evasion, and a
    /// squad that cannot hold the condition is simply immune to it.
    #[test]
    fn a_condition_lands_on_a_squad_and_costs_it_its_evasion() {
        let mut game = game();
        let pack = tactical_pack(&mut game, 9, 40);
        game.open_tactical_battle(pack);
        let squad = seated_squad(&game);
        let player = game.player_entity();

        let plain = game
            .defender_profile_against(
                player,
                squad,
                crate::battle::Swing::plain(crate::battle::DamageRange::centred(10, 0)),
            )
            .evasion;
        game.arm_status(squad, &crate::statuses::StatusId::from("exposed"), 2, 0);

        assert_eq!(
            game.world
                .get::<StatusEffects>(squad)
                .and_then(|s| s.active.first().map(|a| a.id.clone())),
            Some(crate::statuses::StatusId::from("exposed")),
            "the condition had nowhere to live on the squad"
        );
        let exposed = game
            .defender_profile_against(
                player,
                squad,
                crate::battle::Swing::plain(crate::battle::DamageRange::centred(10, 0)),
            )
            .evasion;
        assert!(
            exposed < plain,
            "Exposed cost the squad nothing: {exposed} against {plain}"
        );
    }

    /// **A squad's capture is priced as the lead, not as the summed block.**
    /// The spec and `decompile_squad`'s own doc both say the roll is taken
    /// as though for the lead at the squad's Integrity fraction, and
    /// `TargetResistance::threat_ratio` is the term that was reading five
    /// bodies' `Stats::power` instead of one — enough to bury every attempt
    /// at `CAPTURE_CHANCE_MIN`.
    ///
    /// Read off the odds rather than off a roll: at full Integrity the two
    /// differ in `threat_ratio` alone, so the quoted chances must be equal to
    /// the float.
    #[test]
    fn a_squads_capture_is_priced_as_its_lead_and_not_as_the_summed_block() {
        let mut game = game();
        let pack = tactical_pack(&mut game, 5, 40);
        game.open_tactical_battle(pack);
        let squad = seated_squad(&game);
        let lead = game.world.get::<Squad>(squad).unwrap().members[0];
        crate::tests::support::set_inventory(&mut game, &[(crate::items::ids::ICE_BREAKER, 50)]);

        let squad_power = game.world.get::<Stats>(squad).unwrap().power();
        let lead_power = game.world.get::<Stats>(lead).unwrap().power();
        assert!(
            squad_power > lead_power,
            "fixture: a squad's block must outweigh one member's, or this says nothing"
        );

        let squad_at = game.target_resistance(squad).unwrap();
        let lead_at = game.target_resistance(lead).unwrap();
        assert_eq!(
            squad_at.threat_ratio, lead_at.threat_ratio,
            "the squad's roll is priced against its whole summed block"
        );
        assert_eq!(
            squad_at.hp_fraction, 1.0,
            "fixture: the squad is at full Integrity, so only threat_ratio can differ"
        );

        let (_, potency) = game
            .taming_catalyst()
            .expect("the fixture stocked a catalyst");
        let bonuses = game.player_decompiler_bonuses();
        let as_a_squad = crate::taming::capture_chance(potency, squad_at, bonuses);
        let as_the_lead = crate::taming::capture_chance(potency, lead_at, bonuses);
        assert_eq!(as_a_squad, as_the_lead);
        assert!(
            as_a_squad > crate::tuning::CAPTURE_CHANCE_MIN,
            "a squad priced off its block sits on the floor at {as_a_squad}"
        );
    }

    /// A squad's death pays each remaining member's own kill — the same XP
    /// five separate kills would pay, not one kill priced off the squad's
    /// inflated combined `Stats`. The player's `atk` is boosted to a
    /// one-hit kill so `kill_xp`'s `threat_ratio` denominator (the player's
    /// own power) is identical whether read before the swing or at the
    /// moment of death.
    #[test]
    fn a_squads_death_pays_five_kills_worth_of_xp_and_loot() {
        let mut game = game();
        tactical_fight(&mut game, 9, 1);
        let squad = {
            let battle = game.world.resource::<TacticalBattle>();
            battle
                .bodies()
                .map(|(e, _)| e)
                .find(|&e| game.world.get::<Squad>(e).is_some())
                .expect("9 of a kind must seat a squad")
        };
        let player = game.player_entity();
        game.world.get_mut::<Stats>(player).unwrap().atk = 9999;

        let members = game.world.get::<Squad>(squad).unwrap().members.clone();
        assert_eq!(members.len(), 5);
        let expected_xp: u32 = members.iter().map(|&m| game.kill_xp(m)).sum();
        let xp_before = game.world.get::<Experience>(player).unwrap().xp;
        let downed_before = game
            .world
            .get::<crate::components::DownedPrograms>(player)
            .map_or(0, |d| d.0.len());

        for _ in 0..64 {
            if game.world.get_resource::<TacticalBattle>().is_none() {
                break;
            }
            if game
                .world
                .get_resource::<TacticalBattle>()
                .unwrap()
                .cell_of(squad)
                .is_none()
            {
                break;
            }
            if !wait_for_turn(&mut game, player) {
                break;
            }
            let at = game
                .world
                .resource::<TacticalBattle>()
                .cell_of(squad)
                .unwrap();
            if let Some(spot) = beside(&game, at) {
                game.world
                    .resource_mut::<TacticalBattle>()
                    .move_to(player, spot);
            }
            game.tactical_attack(squad);
        }

        assert!(
            game.world
                .get_resource::<TacticalBattle>()
                .is_none_or(|b| b.cell_of(squad).is_none()),
            "the squad never died"
        );
        let xp_after = game.world.get::<Experience>(player).unwrap().xp;
        assert_eq!(
            xp_after - xp_before,
            expected_xp,
            "a squad's death must pay exactly what killing its five members individually would"
        );
        let downed_after = game
            .world
            .get::<crate::components::DownedPrograms>(player)
            .map_or(0, |d| d.0.len());
        assert_eq!(
            downed_after - downed_before,
            5,
            "a squad's death must leave five downed programs behind, one per member"
        );
    }
}

/// A surviving squad hands its own Integrity fraction back to its members
/// and disbands, whichever way the fight ends alive under it: it walks off
/// the board itself, or the fight ends around it because the player jacked
/// out or went down.
mod disbanding {
    use super::*;

    /// A 9-of-a-kind squad at exactly half Integrity, still on the board.
    fn squad_at_half(game: &mut Game) -> (Entity, Vec<Entity>) {
        let squad = squad_on_the_board(game);
        {
            let mut stats = game.world.get_mut::<Stats>(squad).unwrap();
            stats.hp = stats.max_hp / 2;
        }
        let members = game.world.get::<Squad>(squad).unwrap().members.clone();
        (squad, members)
    }

    /// The same squad, at `hp` Integrity out of its own summed block.
    fn squad_at(game: &mut Game, hp: i32) -> (Entity, Vec<Entity>) {
        let squad = squad_on_the_board(game);
        game.world.get_mut::<Stats>(squad).unwrap().hp = hp;
        let members = game.world.get::<Squad>(squad).unwrap().members.clone();
        (squad, members)
    }

    fn squad_on_the_board(game: &mut Game) -> Entity {
        let pack = tactical_pack(game, 9, 10);
        game.open_tactical_battle(pack);
        let battle = game.world.resource::<TacticalBattle>();
        battle
            .bodies()
            .map(|(e, _)| e)
            .find(|&e| game.world.get::<Squad>(e).is_some())
            .expect("9 of a kind must seat a squad")
    }

    /// A walkable block on the western edge wide enough for `footprint` —
    /// `western_edge`'s general form, since a single free cell there is not
    /// enough room for a squad's whole footprint.
    fn footprint_western_edge(game: &Game, footprint: u8) -> (i32, i32) {
        let battle = game.world.resource::<TacticalBattle>();
        (0..battle.board.side)
            .map(|y| (0, y))
            .find(|&at| {
                crate::tactical::footprint_cells_at(at, footprint)
                    .iter()
                    .all(|&(x, y)| battle.board.walkable(x, y) && battle.occupant((x, y)).is_none())
            })
            .expect("a walkable block on the western edge")
    }

    /// A squad that walks off the board edge disbands into its members at
    /// its own Integrity fraction, rather than vanishing along with it.
    #[test]
    fn a_squad_that_departs_the_board_disbands_its_members() {
        let mut game = game();
        let (squad, members) = squad_at_half(&mut game);
        assert!(
            wait_for_turn(&mut game, squad),
            "the squad never got a turn"
        );

        let footprint = game.world.resource::<TacticalBattle>().footprint_of(squad);
        let edge = footprint_western_edge(&game, footprint);
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(squad, edge),
            "the squad could not be seated on the edge"
        );

        assert_eq!(game.tactical_step((-1, 0)), StepOutcome::Departed);

        assert!(
            game.world.get::<Squad>(squad).is_none(),
            "a squad must not survive its own departure"
        );
        for &member in &members {
            let stats = game.world.get::<Stats>(member).unwrap();
            assert_eq!(
                stats.hp,
                stats.max_hp / 2,
                "member {member:?} did not land at the squad's own Integrity fraction"
            );
        }
    }

    /// **No member is ever handed back at zero Integrity.** A squad on its
    /// last point hands out `max_hp * fraction` rounded, which for a member
    /// of 10 at a fraction of 0.02 is nothing at all — and a member at zero
    /// is not a corpse: it keeps its world `Position` and its `Hostile`, and
    /// `Game::gather_pack` does not filter on `creature_alive`, so walking
    /// into one opens a fight that pays five kills for free.
    /// `decompile_squad` already floors its captured lead at one; this is
    /// its sibling, which did not.
    #[test]
    fn a_squad_disbanding_on_its_last_point_leaves_no_member_at_zero() {
        let mut game = game();
        let (squad, members) = squad_at(&mut game, 1);
        let max_hp = game.world.get::<Stats>(squad).unwrap().max_hp;
        assert!(
            (members[0..1].iter())
                .all(|&m| game.world.get::<Stats>(m).unwrap().max_hp / max_hp == 0),
            "fixture: a member's share of one point must round to nothing"
        );
        assert!(
            wait_for_turn(&mut game, squad),
            "the squad never got a turn"
        );

        let footprint = game.world.resource::<TacticalBattle>().footprint_of(squad);
        let edge = footprint_western_edge(&game, footprint);
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(squad, edge)
        );
        assert_eq!(game.tactical_step((-1, 0)), StepOutcome::Departed);

        for &member in &members {
            assert!(
                game.world.get::<Stats>(member).unwrap().hp >= 1,
                "member {member:?} was handed back dead"
            );
            assert!(
                game.creature_alive(member),
                "member {member:?} stands on the zone map as a free kill"
            );
        }
    }

    /// A squad left standing when the player jacks out disbands too, even
    /// though nothing happened to the squad itself — `settle_tactical`'s own
    /// sweep, not `depart_tactical`'s direct call for its own departure.
    #[test]
    fn a_squad_left_standing_when_the_player_jacks_out_disbands() {
        let mut game = game();
        let (squad, members) = squad_at_half(&mut game);
        let player = game.player_entity();
        assert!(
            wait_for_turn(&mut game, player),
            "the player never got a turn"
        );
        let edge = western_edge(&game);
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(player, edge)
        );

        assert_eq!(game.tactical_step((-1, 0)), StepOutcome::Departed);

        assert!(
            game.world.get_resource::<TacticalBattle>().is_none(),
            "the fight must have closed behind the jack-out"
        );
        assert!(
            game.world.get::<Squad>(squad).is_none(),
            "the world outside a fight must never contain a squad"
        );
        for &member in &members {
            let stats = game.world.get::<Stats>(member).unwrap();
            assert_eq!(stats.hp, stats.max_hp / 2);
        }
    }
}

/// A squad's capture: the roll is taken against the squad's own Integrity,
/// only the lead leaves the fight, and a squad supplies at most five.
mod squad_capture {
    use super::*;

    /// A 9-of-a-kind squad and a pack of catalysts.
    ///
    /// **The decompiler is left at whatever the player starts with**, so the
    /// odds this rolls against are the real ones — around 0.19 at full
    /// Integrity, rising as the squad is worn down. It used to force
    /// `skill = 2000` to keep the retry loop short, and that clamped every
    /// attempt at `CAPTURE_CHANCE_MAX`: a squad priced against its whole
    /// summed block rolls `CAPTURE_CHANCE_MIN` instead, and the clamp made
    /// the two indistinguishable, so this test read as covering the odds and
    /// covered nothing about them. At the real rate the loop below spends 15
    /// of its 40 attempts.
    fn squad_ready_to_capture(game: &mut Game) -> Entity {
        // Capturing all five members would otherwise run into
        // `BASE_PET_CAPACITY` (3) long before the squad runs out of
        // members to give — a real gate this fixture must clear rather
        // than a squad-specific limit.
        crate::tests::support::spawn_data_cache(game, 1);
        let pack = tactical_pack(game, 9, 40);
        game.open_tactical_battle(pack);
        let squad = {
            let battle = game.world.resource::<TacticalBattle>();
            battle
                .bodies()
                .map(|(e, _)| e)
                .find(|&e| game.world.get::<Squad>(e).is_some())
                .expect("9 of a kind must seat a squad")
        };
        let player = game.player_entity();
        only_routine(game, player, "decompile");
        crate::tests::support::set_inventory(game, &[(crate::items::ids::ICE_BREAKER, 50)]);
        squad
    }

    /// One decompile attempt against `squad`, whichever cell it currently
    /// anchors at. Returns whether a member was pulled out of it this
    /// attempt (the squad may also have been killed by the capture's own
    /// damage in the same swing, which reads as a member pulled too).
    fn attempt(game: &mut Game, squad: Entity) -> bool {
        let player = game.player_entity();
        assert!(wait_for_turn(game, player), "the player never got a turn");
        let Some(at) = game.world.resource::<TacticalBattle>().cell_of(squad) else {
            return false;
        };
        let before = game.world.get::<Squad>(squad).map(|s| s.members.len());
        if let Some(spot) = beside(game, at) {
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(player, spot);
        }
        game.tactical_use_routine(0, at);
        game.world.get::<Squad>(squad).map(|s| s.members.len()) != before
    }

    /// A capture yields one member, at a fifth of `max_hp` off the squad —
    /// exact here because `tactical_pack`'s `hp: 40` makes `max_hp / 5`
    /// divide evenly, so five captures spend exactly the squad's whole
    /// Integrity and the sixth attempt finds nothing left to aim at.
    #[test]
    fn a_squad_supplies_at_most_five_captures() {
        let mut game = game();
        let squad = squad_ready_to_capture(&mut game);
        let max_hp = game.world.get::<Stats>(squad).unwrap().max_hp;
        assert_eq!(max_hp, 200, "fixture: 5 members at 40 max_hp each");

        let mut captures = 0;
        for _ in 0..40 {
            if game.world.get::<Squad>(squad).is_none() {
                break;
            }
            let hp_before = game.world.get::<Stats>(squad).unwrap().hp;
            if !attempt(&mut game, squad) {
                continue;
            }
            captures += 1;
            // The squad may have been despawned by this same capture's
            // damage (the fifth), so a live `Stats` is required rather than
            // assumed.
            if let Some(hp_after) = game.world.get::<Stats>(squad).map(|s| s.hp) {
                assert_eq!(
                    hp_before - hp_after,
                    max_hp / 5,
                    "capture {captures} did not remove a fifth of max_hp"
                );
            }
        }

        assert_eq!(
            captures, 5,
            "a squad of five must supply exactly five captures"
        );
        assert!(
            game.world.get::<Squad>(squad).is_none(),
            "the squad must be gone after supplying every capture it can"
        );
        // The sixth attempt: nothing to aim at any more, so a further
        // decompile against the squad's old cell finds no target and does
        // nothing.
        if let Some(battle) = game.world.get_resource::<TacticalBattle>() {
            assert!(battle.cell_of(squad).is_none());
        }
    }

    /// Each capture pulls exactly the lead out and grants it a real place
    /// on the roster — a new `ProgramId`, `Experience`, and no longer
    /// `Hostile`.
    #[test]
    fn a_capture_grants_the_lead_a_place_on_the_roster() {
        let mut game = game();
        let squad = squad_ready_to_capture(&mut game);
        let lead = game.world.get::<Squad>(squad).unwrap().members[0];

        // Forty attempts, matching the sibling test above, because the
        // fixture deliberately rolls the real ~0.19 capture chance: ten was
        // a one-in-eight failure on any stream, and the raid clock's removal
        // of `raid_check`'s per-tick draw is simply the stream that found it.
        let landed = (0..40).any(|_| attempt(&mut game, squad));
        assert!(landed, "the capture never landed in 40 attempts");

        assert!(
            game.world.get::<Hostile>(lead).is_none(),
            "the captured lead is still hostile"
        );
        assert!(
            game.world
                .get::<crate::components::ProgramId>(lead)
                .is_some(),
            "the captured lead was not given a roster identity"
        );
        assert!(
            game.world.get::<Experience>(lead).is_some(),
            "the captured lead was not given Experience"
        );
        assert!(
            !game
                .world
                .get::<Squad>(squad)
                .is_some_and(|s| s.members.contains(&lead)),
            "the captured lead is still listed as a squad member"
        );
    }

    /// `Squad::members` running out is a *second* way a squad dies,
    /// independent of `Stats::hp` — the fifth capture's own damage need not
    /// zero the squad exactly. Calling `decompile_squad` directly (rather
    /// than through `tactical_use_routine`) needs no range or positioning
    /// at all, since aiming is that door's own concern and not this one's.
    #[test]
    fn an_emptied_squad_dies_even_with_integrity_left_over() {
        let mut game = game();
        crate::tests::support::spawn_data_cache(&mut game, 1);
        let species = game
            .species_defs()
            .into_iter()
            .find(|d| !d.is_hybrid())
            .expect("at least one species ships")
            .id;
        let at = *game.world.get::<Position>(game.player_entity()).unwrap();
        // One member one HP heavier than the rest, so `max_hp` (201) does
        // not divide evenly by the formation's five members (share: 40,
        // remainder 1) — the corner `kill_outright` exists for.
        let pack: Vec<Entity> = (0..5)
            .map(|i| {
                let hp = if i == 0 { 41 } else { 40 };
                game.world
                    .spawn((
                        Creature {
                            species: species.clone(),
                        },
                        Hostile,
                        Position { x: at.x, y: at.y },
                        Stats {
                            hp,
                            max_hp: hp,
                            atk: 1,
                            mitigation: 0,
                        },
                        StatusEffects::default(),
                    ))
                    .id()
            })
            .collect();
        game.open_tactical_battle(pack);
        let squad = {
            let battle = game.world.resource::<TacticalBattle>();
            battle
                .bodies()
                .map(|(e, _)| e)
                .find(|&e| game.world.get::<Squad>(e).is_some())
                .expect("5 of a kind must seat a squad")
        };
        let player = game.player_entity();
        let max_hp = game.world.get::<Stats>(squad).unwrap().max_hp;
        assert_eq!(max_hp, 201, "fixture: 4 members at 40 plus one at 41");
        assert_ne!(
            max_hp % 5,
            0,
            "fixture: max_hp must not divide evenly by the formation size"
        );

        crate::tests::support::set_inventory(&mut game, &[(crate::items::ids::ICE_BREAKER, 50)]);
        only_routine(&mut game, player, "decompile");
        game.world
            .get_mut::<crate::components::Decompiler>(player)
            .unwrap()
            .skill = 2000;

        let mut captures = 0;
        for _ in 0..40 {
            if game.world.get::<Squad>(squad).is_none() {
                break;
            }
            if game.decompile_squad(squad, player) {
                captures += 1;
            }
            if captures >= 5 {
                break;
            }
        }

        assert_eq!(
            captures, 5,
            "a squad of five must supply exactly five captures"
        );
        // `decompile_squad` is called directly here rather than through
        // `tactical_use_routine`, so the reap that would despawn a dead
        // squad on a real turn never runs — what this pins is the
        // precondition that reap acts on: with `Squad::members` emptied,
        // `kill_outright` must have zeroed the one Integrity point real
        // damage alone would have left standing.
        assert!(
            game.world
                .get::<Squad>(squad)
                .is_some_and(|s| s.members.is_empty()),
            "the squad must have given up its last member"
        );
        assert_eq!(
            game.world.get::<Stats>(squad).map(|s| s.hp),
            Some(0),
            "an emptied squad must have zero Integrity even though 1 point was left over"
        );
    }
}

/// A squad is never saved, and its members are unchanged by a save made
/// mid-fight — `#[serde(skip)]` on `Squad` would leave this green against
/// nothing, since `Squad` derives no `Serialize` at all and there is no RON
/// round trip to catch that omission; this needs a real save/load.
#[test]
fn a_squad_is_not_saved_and_its_members_are_unchanged() {
    let mut game = game();
    let pack = tactical_pack(&mut game, 9, 40);
    game.open_tactical_battle(pack.clone());

    // `pack`'s own tiles, not a query over every `Hostile` in the world —
    // ambient habitat spawns near the player would otherwise swamp the
    // count, and entity ids are not stable across a save, so the tile each
    // member stood on is what ties a pre-save row to its post-load one.
    let tiles: std::collections::BTreeSet<(i32, i32)> = pack
        .iter()
        .map(|&e| {
            let p = game.world.get::<Position>(e).unwrap();
            (p.x, p.y)
        })
        .collect();
    assert_eq!(tiles.len(), 9, "fixture: nine distinct member tiles");

    let before: Vec<(Position, Stats)> = {
        let mut query = game
            .world
            .query_filtered::<(&Position, &Stats), With<Hostile>>();
        let mut rows: Vec<(Position, Stats)> = query
            .iter(&game.world)
            .filter(|(p, _)| tiles.contains(&(p.x, p.y)))
            .map(|(p, s)| (*p, *s))
            .collect();
        rows.sort_by_key(|(p, _)| (p.x, p.y));
        rows
    };
    // The squad shell has no `Position`, so it never appears in `before`
    // above — nine members, not nine plus a squad.
    assert_eq!(
        before.len(),
        9,
        "fixture: the squad's members alone carry Position"
    );

    let path = std::env::temp_dir().join(format!(
        "feral_processes_squad_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let after: Vec<(Position, Stats)> = {
        let mut query = loaded
            .world
            .query_filtered::<(&Position, &Stats), With<Hostile>>();
        let mut rows: Vec<(Position, Stats)> = query
            .iter(&loaded.world)
            .filter(|(p, _)| tiles.contains(&(p.x, p.y)))
            .map(|(p, s)| (*p, *s))
            .collect();
        rows.sort_by_key(|(p, _)| (p.x, p.y));
        rows
    };
    assert_eq!(
        after.len(),
        9,
        "a save made mid-fight must restore exactly the nine members, no squad shell"
    );
    for ((before_pos, before_stats), (after_pos, after_stats)) in before.iter().zip(&after) {
        assert_eq!(
            before_pos, after_pos,
            "a member's own Position must survive the round trip"
        );
        // `Stats` derives no `PartialEq`, so its fields are compared by hand.
        assert_eq!(before_stats.hp, after_stats.hp);
        assert_eq!(before_stats.max_hp, after_stats.max_hp);
        assert_eq!(before_stats.atk, after_stats.atk);
        assert_eq!(before_stats.mitigation, after_stats.mitigation);
    }

    // And no entity anywhere in the loaded world carries `components::Squad`
    // — the component itself never reaches the save format at all.
    let mut squads = loaded.world.query::<&Squad>();
    assert_eq!(
        squads.iter(&loaded.world).count(),
        0,
        "a loaded save must never contain a Squad"
    );

    // A broader net than the tile match above: whatever tile a squad's
    // combined stat block might land on if it ever gained a Position by
    // mistake, its distinctive summed `max_hp` (5 members at 40 each) must
    // not appear anywhere in the loaded world's Hostile roster.
    let mut all_hostiles = loaded
        .world
        .query_filtered::<&Stats, (With<Hostile>, With<Position>)>();
    assert!(
        all_hostiles.iter(&loaded.world).all(|s| s.max_hp != 200),
        "a squad's combined stat block reached the save under some other tile"
    );
}

/// What a fight's drawing needs: `TacticalBody`'s two squad fields, and the
/// name `Game::entity_label` builds for a `Squad`.
mod squad_drawing {
    use super::*;

    /// A folded squad's own `TacticalBody` carries the formation's
    /// footprint and a `SquadView`; a leftover single carries neither.
    #[test]
    fn a_squads_tacticalbody_carries_its_footprint_and_squadview() {
        let mut game = game();
        let pack = tactical_pack(&mut game, 9, 10);
        game.open_tactical_battle(pack);
        let view = game.tactical_view().expect("a fight is open");
        let formation = &crate::tuning::FORMATIONS[0];

        let squad_body = view
            .bodies
            .iter()
            .find(|b| b.squad.is_some())
            .expect("9 of a kind must seat a squad");
        assert_eq!(squad_body.footprint, formation.footprint);
        let squad_view = squad_body.squad.as_ref().unwrap();
        assert_eq!(squad_view.members, 5);
        assert_eq!(squad_view.mark, formation.mark);
        assert_eq!(squad_view.noun, formation.noun);

        let lone = view
            .bodies
            .iter()
            .find(|b| b.squad.is_none() && !b.is_player)
            .expect("a leftover single stands on the board");
        assert_eq!(
            lone.footprint, 1,
            "a body with no Squad must read as footprint 1"
        );
    }

    /// `"<species> squad (5)"`, built once in the engine — and it shrinks
    /// live off `Squad::members`, never off a count stashed at formation, so
    /// a turn strip and an examine line built a tick apart after a capture
    /// cannot disagree.
    #[test]
    fn a_squads_name_is_built_in_the_engine_and_tracks_its_own_membership() {
        let mut game = game();
        let pack = tactical_pack(&mut game, 9, 10);
        game.open_tactical_battle(pack);
        let squad = {
            let battle = game.world.resource::<TacticalBattle>();
            battle
                .bodies()
                .map(|(e, _)| e)
                .find(|&e| game.world.get::<Squad>(e).is_some())
                .expect("9 of a kind must seat a squad")
        };
        let creature = game.world.get::<Creature>(squad).unwrap().clone();
        let species_name = game
            .world
            .resource::<SpeciesDb>()
            .get(&creature.species)
            .unwrap()
            .name
            .clone();
        assert_eq!(
            game.entity_label(squad),
            format!("{species_name} squad (5)")
        );

        game.world.get_mut::<Squad>(squad).unwrap().members.pop();
        assert_eq!(
            game.entity_label(squad),
            format!("{species_name} squad (4)"),
            "the name must read the squad's own membership, not a cached count"
        );
    }
}
