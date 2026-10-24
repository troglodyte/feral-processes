use std::collections::BTreeSet;

use super::*;
use crate::abilities::{AbilityDb, AbilityDef, AbilityEffect, AbilityTarget, validate};
use crate::damage::DamageType;
use crate::tests::support::test_assets_dir;
use crate::tuning::{CRAFT_PART_CAP_MAX, CRAFT_PART_CAP_START};

fn abilities() -> AbilityDb {
    AbilityDb::load_dir(&test_assets_dir().join("abilities"))
        .unwrap()
        .0
}

fn routine(parts: &[Subroutine]) -> CraftedRoutine {
    CraftedRoutine {
        id: "crafted/1".into(),
        name: "Test".into(),
        parts: parts.to_vec(),
    }
}

fn core(c: Core) -> Subroutine {
    Subroutine::Core(c, Version::V1)
}

// ---------------------------------------------------------------------------
// Task 1 and 6: the census over what a wild program, species kit, talent,
// item or boss can carry.
// ---------------------------------------------------------------------------

/// Every authored routine a player-facing source can reach, by id.
fn reachable(db: &AbilityDb) -> BTreeSet<String> {
    let assets = test_assets_dir();
    let mut ids: BTreeSet<String> = BTreeSet::new();
    ids.extend(db.wild_pool().into_iter().map(|(d, _)| d.id.clone()));
    ids.extend(db.exclusive_pool().into_iter().map(|d| d.id.clone()));
    let (species, _) = crate::species::SpeciesDb::load_dir(&assets.join("species"), db).unwrap();
    for def in species.all() {
        ids.extend(def.abilities.iter().map(|a| a.id.clone()));
    }
    let (talents, _) = crate::talents::TalentDb::load_dir(&assets.join("talents")).unwrap();
    for choice in talents.all_nodes() {
        if let crate::talents::TalentNode::Ability { id } = &choice.node {
            ids.insert(id.clone());
        }
    }
    let (items, _) = crate::items_db::ItemDb::load_dir(&assets.join("items"), db).unwrap();
    ids.extend(items.all().filter_map(|i| i.grants.clone()));
    ids
}

fn assert_builds(def: &AbilityDef, parts: &[Subroutine]) {
    assert_eq!(
        validate_build(parts, CRAFT_PART_CAP_MAX),
        Ok(()),
        "{} decomposed to an invalid build {parts:?}",
        def.id
    );
    let built = compile(&routine(parts));
    assert_eq!(validate(&built), Ok(()), "{} recompiled invalid", def.id);
}

#[test]
fn every_reachable_routine_decomposes_except_the_known_misfits() {
    let db = abilities();
    let ids = reachable(&db);
    assert!(ids.len() > 40, "the census walked {} routines", ids.len());
    let mut misfits = Vec::new();
    for id in &ids {
        let def = db
            .get(id)
            .unwrap_or_else(|| panic!("{id} names no routine"));
        match decompose(def) {
            Some(parts) => assert_builds(def, &parts),
            None => misfits.push(id.as_str()),
        }
    }
    assert!(misfits.is_empty(), "no Core stands for {misfits:?}");
}

#[test]
fn every_species_basic_attack_decomposes() {
    let assets = test_assets_dir();
    let db = abilities();
    let (species, _) = crate::species::SpeciesDb::load_dir(&assets.join("species"), &db).unwrap();
    let mut seen = 0;
    for def in species.all() {
        for attack in def.basic_attacks() {
            let parts = decompose(&attack)
                .unwrap_or_else(|| panic!("{} has a basic attack no Core stands for", def.id));
            assert_builds(&attack, &parts);
            seen += 1;
        }
    }
    assert!(seen > 20);
}

#[test]
fn the_seven_boss_exclusives_each_decompose_to_a_core() {
    let db = abilities();
    for id in [
        "bus_snoop",
        "deadman",
        "hard_fault",
        "kernel_shear",
        "long_winter",
        "null_cache",
        "watchdog",
    ] {
        let def = db.get(id).unwrap();
        assert!(def.exclusive, "{id}");
        let parts = decompose(def).unwrap_or_else(|| panic!("{id} fits no Core"));
        assert!(matches!(parts[0], Subroutine::Core(..)));
        assert_builds(def, &parts);
    }
}

#[test]
fn decompose_reads_the_modifiers_a_routine_implies() {
    let db = abilities();
    let parts = |id: &str| decompose(db.get(id).unwrap()).unwrap();
    assert_eq!(
        parts("hard_fault"),
        vec![
            core(Core::Debuff),
            Subroutine::Area(AreaKind::Everyone),
            Subroutine::Trickle,
            Subroutine::Status(StatusKind::Stun),
        ]
    );
    assert_eq!(
        parts("kernel_shear"),
        vec![
            core(Core::Damage),
            Subroutine::Area(AreaKind::Group),
            Subroutine::Status(StatusKind::Bleed),
        ]
    );
    assert_eq!(
        parts("null_cache"),
        vec![
            core(Core::Drain),
            Subroutine::Area(AreaKind::Group),
            Subroutine::DamageType(DamageType::Magnetic),
        ]
    );
    assert_eq!(
        parts("cold_boot"),
        vec![Subroutine::Core(Core::Heal, Version::V3)]
    );
    assert_eq!(parts("haymaker"), vec![core(Core::Damage)]);
    // A Field Mitigation takes no Area, so the party scope is lost; the
    // routine is the nearest the catalogue has.
    assert_eq!(parts("long_winter"), vec![core(Core::FieldMitigation)]);
}

// ---------------------------------------------------------------------------
// Task 2: the fit matrix.
// ---------------------------------------------------------------------------

/// The spec's "What fits which Core", one row per Core:
/// range, area, trickle, damage type, status.
const MATRIX: [(Core, [bool; 5]); 31] = [
    (Core::SapMitigation, [true, true, true, false, false]),
    (Core::Damage, [true, true, false, true, true]),
    (Core::Heal, [true, true, true, false, false]),
    (Core::Drain, [true, true, false, true, false]),
    (Core::BoostAttack, [true, true, true, false, false]),
    (Core::BoostMitigation, [true, true, true, false, false]),
    (Core::SapAttack, [true, true, true, false, false]),
    (Core::Debuff, [true, true, true, false, true]),
    (Core::Cleanse, [true, true, false, false, false]),
    (Core::Cloak, [false, true, true, false, false]),
    (Core::ColdSample, [true, false, true, false, false]),
    (Core::HeatInjection, [true, false, true, false, false]),
    (Core::Profile, [true, false, true, false, false]),
    (Core::Inject, [true, false, true, false, false]),
    (Core::Hallucinate, [true, true, true, false, false]),
    (Core::FieldRegen, [false, true, true, false, false]),
    (Core::Recharge, [false, true, true, false, false]),
    (Core::Fork, [false; 5]),
    (Core::Respawn, [false; 5]),
    (Core::Teleport, [false; 5]),
    (Core::Reinforce, [false; 5]),
    (Core::Emulate, [false; 5]),
    (Core::FieldAttack, [false; 5]),
    (Core::FieldMitigation, [false; 5]),
    (Core::CaptureBoost, [false; 5]),
    (Core::XpBoost, [false; 5]),
    (Core::EncounterDamp, [false; 5]),
    (Core::DropBoost, [false; 5]),
    (Core::Phase, [false; 5]),
    (Core::Jump, [false; 5]),
    (Core::Symlink, [false; 5]),
];

#[test]
fn the_fit_matrix_is_the_specs() {
    let covered: BTreeSet<Core> = MATRIX.iter().map(|(c, _)| *c).collect();
    assert_eq!(covered.len(), Core::ALL.len());
    for (core, row) in MATRIX {
        let area = core
            .areas()
            .first()
            .map_or(Subroutine::Area(AreaKind::Group), |k| Subroutine::Area(*k));
        let cells = [
            Subroutine::Range(RangeKind::Near),
            area,
            Subroutine::Trickle,
            Subroutine::DamageType(DamageType::Thermal),
            Subroutine::Status(StatusKind::Bleed),
        ];
        for (cell, expected) in cells.iter().zip(row) {
            assert_eq!(
                fits(core, cell).is_ok(),
                expected,
                "{} with {}: {:?}",
                core.name(),
                cell.label(),
                fits(core, cell)
            );
        }
    }
}

#[test]
fn a_refusal_carries_a_reason_a_screen_can_show() {
    for core in Core::ALL {
        for part in [
            Subroutine::Range(RangeKind::Far),
            Subroutine::Area(AreaKind::Everyone),
            Subroutine::Trickle,
            Subroutine::DamageType(DamageType::Noise),
            Subroutine::Status(StatusKind::Stun),
        ] {
            if let Err(reason) = fits(core, &part) {
                assert!(reason.len() > 10, "{core:?} {part:?}");
            }
        }
    }
    assert!(fits(Core::Heal, &Subroutine::Area(AreaKind::Everyone)).is_err());
    assert!(fits(Core::Damage, &Subroutine::Area(AreaKind::Everyone)).is_ok());
}

#[test]
fn only_the_cores_with_a_magnitude_are_versioned() {
    for core in Core::ALL {
        let expected = !matches!(
            core,
            Core::Debuff
                | Core::Cleanse
                | Core::Cloak
                | Core::ColdSample
                | Core::HeatInjection
                | Core::Profile
                | Core::Inject
                | Core::Hallucinate
                | Core::Teleport
                | Core::Reinforce
                | Core::Emulate
                | Core::Phase
                | Core::Jump
                | Core::Symlink
        );
        assert_eq!(core.versioned(), expected, "{core:?}");
    }
}

#[test]
fn a_build_is_checked_against_the_cap_the_slots_and_the_required_parts() {
    let ok = |parts: &[Subroutine], cap| validate_build(parts, cap);
    let heal = core(Core::Heal);
    assert_eq!(ok(&[heal], CRAFT_PART_CAP_START), Ok(()));
    assert_eq!(
        ok(&[heal, Subroutine::Trickle, Subroutine::Trickle], 2),
        Err(BuildError::TooManyParts { cap: 2 })
    );
    assert_eq!(
        ok(&[heal, Subroutine::Trickle, Subroutine::Trickle], 3),
        Ok(())
    );
    assert_eq!(ok(&[], 6), Err(BuildError::NoCore));
    assert_eq!(ok(&[Subroutine::Trickle], 6), Err(BuildError::NoCore));
    assert_eq!(
        ok(&[heal, core(Core::Damage)], 6),
        Err(BuildError::TwoCores)
    );
    assert_eq!(
        ok(
            &[
                core(Core::Damage),
                Subroutine::Range(RangeKind::Near),
                Subroutine::Range(RangeKind::Far)
            ],
            6
        ),
        Err(BuildError::Duplicate("Range"))
    );
    assert!(matches!(
        ok(&[heal, Subroutine::DamageType(DamageType::Noise)], 6),
        Err(BuildError::DoesNotFit(_))
    ));
    assert_eq!(
        ok(&[core(Core::Debuff)], 6),
        Err(BuildError::Missing("a Status"))
    );
    assert_eq!(
        ok(&[core(Core::Hallucinate)], 6),
        Err(BuildError::Missing("an Area"))
    );
    assert_eq!(
        ok(&[Subroutine::Core(Core::Cleanse, Version::V2)], 6),
        Err(BuildError::NotVersioned)
    );
    assert_eq!(
        ok(&[Subroutine::Core(Core::Damage, Version::V3)], 6),
        Ok(())
    );
}

#[test]
fn a_part_costs_one_disk_plus_its_version_of_a_material() {
    let (item, n) = material(&Subroutine::Core(Core::Heal, Version::V3));
    assert_eq!((item.0.as_str(), n), ("bytecode_block", 3));
    let (item, n) = material(&Subroutine::Core(Core::Damage, Version::V2));
    assert_eq!((item.0.as_str(), n), ("core_fragment", 2));
    let (item, n) = material(&core(Core::Teleport));
    assert_eq!((item.0.as_str(), n), ("power_cell", 1));
    assert_eq!(material(&Subroutine::Trickle).0.0, "bytecode_block");
    assert_eq!(
        material(&Subroutine::Area(AreaKind::Group)).0.0,
        "core_fragment"
    );
    assert_eq!(
        material(&Subroutine::Range(RangeKind::Far)).0.0,
        "power_cell"
    );
}

// ---------------------------------------------------------------------------
// Task 4: compile.
// ---------------------------------------------------------------------------

#[test]
fn patch_is_a_v1_heal_with_the_specs_numbers() {
    let def = compile(&routine(&[core(Core::Heal)]));
    assert_eq!(def.target, AbilityTarget::OneAlly);
    assert!(matches!(
        def.effect,
        AbilityEffect::Heal {
            power: 10,
            spread: 2
        }
    ));
    assert_eq!((def.cooldown, def.power_cost), (2, 6.0));
    assert_eq!(def.name, "Test");
    assert!(!def.description.is_empty());
}

#[test]
fn a_v1_damage_matches_segfault_v1() {
    let shipped = abilities();
    let segfault = shipped.get("segfault_v1").unwrap();
    let def = compile(&routine(&[core(Core::Damage)]));
    assert_eq!(
        format!("{:?}", def.effect),
        format!("{:?}", segfault.effect)
    );
    assert_eq!(
        (def.cooldown, def.power_cost, def.accuracy, def.target),
        (
            segfault.cooldown,
            segfault.power_cost,
            segfault.accuracy,
            segfault.target
        )
    );
}

#[test]
fn versions_multiply_the_magnitude_and_the_price() {
    let heal = |v| compile(&routine(&[Subroutine::Core(Core::Heal, v)]));
    let AbilityEffect::Heal { power, spread } = heal(Version::V2).effect else {
        panic!()
    };
    assert_eq!((power, spread), (18, 4));
    let AbilityEffect::Heal { power, .. } = heal(Version::V3).effect else {
        panic!()
    };
    assert_eq!(power, 28);
    assert_eq!(heal(Version::V3).power_cost, 10.0);
    assert_eq!(heal(Version::V3).cooldown, 4);
}

#[test]
fn an_area_lowers_the_per_target_figure_and_raises_price_and_wait() {
    let single = compile(&routine(&[core(Core::Damage)]));
    let group = compile(&routine(&[
        core(Core::Damage),
        Subroutine::Area(AreaKind::Group),
    ]));
    let everyone = compile(&routine(&[
        core(Core::Damage),
        Subroutine::Area(AreaKind::Everyone),
    ]));
    assert_eq!(group.target, AbilityTarget::WholeEnemyGroup);
    assert_eq!(everyone.target, AbilityTarget::AllEnemies);
    assert!(group.shape.is_some() && everyone.shape.is_some());
    let power = |d: &AbilityDef| match d.effect {
        AbilityEffect::Damage { power, .. } => power,
        _ => panic!(),
    };
    assert!(power(&group) < power(&single) && power(&everyone) < power(&group));
    assert!(group.power_cost > single.power_cost && everyone.power_cost > group.power_cost);
    assert_eq!(group.cooldown, single.cooldown + 1);
    assert!(group.accuracy < single.accuracy);
}

#[test]
fn range_only_centres_the_area_and_never_resizes_it() {
    let def = compile(&routine(&[
        core(Core::Damage),
        Subroutine::Area(AreaKind::Group),
        Subroutine::Range(RangeKind::Near),
    ]));
    assert_eq!(
        def.shape,
        Some(crate::abilities::AbilityShape::Radius {
            radius: crate::tuning::TACTICAL_GROUP_RADIUS
        })
    );
    assert_eq!(def.range.map(|r| r.max), Some(3));
    let unranged = compile(&routine(&[
        core(Core::Damage),
        Subroutine::Area(AreaKind::Group),
    ]));
    assert_eq!(def.shape, unranged.shape);
    assert_eq!(def.tactical_shape(), unranged.tactical_shape());
}

#[test]
fn sap_mitigation_matches_the_three_shipped_mitigation_saps() {
    let shipped = abilities();
    let magnitude = |d: &AbilityDef| match d.effect {
        AbilityEffect::Buff {
            power, duration, ..
        } => (power, duration),
        _ => panic!(),
    };
    for (id, area) in [
        ("acid_wash", None),
        ("etch", Some(AreaKind::Group)),
        ("oxide_strip", Some(AreaKind::Everyone)),
    ] {
        let mut parts = vec![core(Core::SapMitigation)];
        parts.extend(area.map(Subroutine::Area));
        let built = compile(&routine(&parts));
        let authored = shipped.get(id).unwrap();
        assert_eq!(magnitude(&built), magnitude(authored), "{id}");
        assert_eq!(built.target, authored.target, "{id}");
    }
}

#[test]
fn range_sets_the_reach_and_lets_a_back_rank_use_it() {
    let def = compile(&routine(&[
        core(Core::Damage),
        Subroutine::Range(RangeKind::Far),
    ]));
    assert!(def.ranged);
    assert_eq!(def.range.map(|r| r.max), Some(6));
    assert!(!compile(&routine(&[core(Core::Damage)])).ranged);
}

#[test]
fn trickle_lengthens_a_buff_and_costs_power_and_wait() {
    let plain = compile(&routine(&[core(Core::BoostAttack)]));
    let long = compile(&routine(&[
        core(Core::BoostAttack),
        Subroutine::Trickle,
        Subroutine::Trickle,
    ]));
    let rounds = |d: &AbilityDef| match d.effect {
        AbilityEffect::Buff { duration, .. } => duration,
        _ => panic!(),
    };
    assert_eq!(rounds(&long), rounds(&plain) + 2);
    assert_eq!(long.cooldown, plain.cooldown + 2);
    assert!(long.power_cost > plain.power_cost);
}

#[test]
fn heal_plus_trickle_is_a_heal_over_time_worth_more_in_total() {
    let hot = compile(&routine(&[core(Core::Heal), Subroutine::Trickle]));
    let AbilityEffect::HealOverTime {
        power,
        spread,
        rounds,
    } = hot.effect
    else {
        panic!("{:?}", hot.effect)
    };
    assert_eq!((power, spread, rounds), (6, 1, 2));
    assert!(power * rounds as i32 > 10);
    assert_eq!(validate(&hot), Ok(()));
}

#[test]
fn a_status_rider_and_a_damage_type_ride_on_damage() {
    let def = compile(&routine(&[
        core(Core::Damage),
        Subroutine::DamageType(DamageType::Noise),
        Subroutine::Status(StatusKind::Stun),
    ]));
    let AbilityEffect::Damage {
        status,
        damage_type,
        ..
    } = def.effect
    else {
        panic!()
    };
    assert_eq!(damage_type, Some(DamageType::Noise));
    assert_eq!(status.unwrap().kind, StatusKind::Stun.id());
}

#[test]
fn a_field_core_has_no_cooldown_and_a_timed_one_has_a_duration() {
    let regen = compile(&routine(&[core(Core::FieldRegen), Subroutine::Trickle]));
    assert_eq!(regen.cooldown, 0);
    let AbilityEffect::FieldBuff { duration, .. } = regen.effect else {
        panic!()
    };
    assert_eq!(duration, 900);
    let shell = compile(&routine(&[core(Core::FieldMitigation)]));
    let AbilityEffect::FieldBuff { duration, .. } = shell.effect else {
        panic!()
    };
    assert_eq!(duration, 0);
    assert_eq!(shell.target, AbilityTarget::OneAlly);
    assert_eq!(
        compile(&routine(&[core(Core::XpBoost)])).target,
        AbilityTarget::WholeParty
    );
}

/// Every Core under every modifier multiset the cap allows must pass the
/// validators `AbilityDb::load_dir` applies, and name only statuses that
/// exist.
#[test]
fn every_buildable_routine_compiles_to_one_the_loader_would_accept() {
    let (statuses, _) =
        crate::statuses::StatusDb::load_dir(&test_assets_dir().join("statuses")).unwrap();
    let (statuses, _) = statuses.with_builtins();
    assert!(statuses.contains(&crate::statuses::StatusId::from(
        crate::tuning::REGEN_STATUS
    )));

    let mut built = 0;
    for core_kind in Core::ALL {
        let versions: &[Version] = if core_kind.versioned() {
            &Version::ALL
        } else {
            &[Version::V1]
        };
        let ranges = [None, Some(RangeKind::Near), Some(RangeKind::Far)];
        let areas = std::iter::once(None).chain(core_kind.areas().iter().map(|a| Some(*a)));
        let areas: Vec<Option<AreaKind>> = areas.collect();
        let types = std::iter::once(None)
            .chain(DamageType::ALL.into_iter().map(Some))
            .collect::<Vec<_>>();
        let stats = std::iter::once(None)
            .chain(StatusKind::ALL.into_iter().map(Some))
            .collect::<Vec<_>>();
        for &version in versions {
            for range in ranges {
                for &area in &areas {
                    for &ty in &types {
                        for &status in &stats {
                            for trickles in 0..CRAFT_PART_CAP_MAX {
                                let mut parts = vec![Subroutine::Core(core_kind, version)];
                                parts.extend(range.map(Subroutine::Range));
                                parts.extend(area.map(Subroutine::Area));
                                parts.extend(ty.map(Subroutine::DamageType));
                                parts.extend(status.map(Subroutine::Status));
                                parts.extend(std::iter::repeat_n(Subroutine::Trickle, trickles));
                                if validate_build(&parts, CRAFT_PART_CAP_MAX).is_err() {
                                    continue;
                                }
                                let def = compile(&routine(&parts));
                                assert_eq!(
                                    validate(&def),
                                    Ok(()),
                                    "{parts:?} compiled to a def the loader refuses"
                                );
                                assert!(def.power_cost > 0.0, "{parts:?} is free");
                                assert!(!def.description.is_empty(), "{parts:?}");
                                assert_eq!(describe(&parts), def.description);
                                if def.effect.field_only() {
                                    assert_eq!(def.cooldown, 0, "{parts:?}");
                                }
                                match &def.effect {
                                    AbilityEffect::Debuff { kind, .. } => {
                                        assert!(statuses.contains(kind), "{parts:?}")
                                    }
                                    AbilityEffect::Damage {
                                        status: Some(rider),
                                        ..
                                    } => assert!(statuses.contains(&rider.kind), "{parts:?}"),
                                    _ => {}
                                }
                                built += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(built > 2000, "only {built} builds were tried");
}
