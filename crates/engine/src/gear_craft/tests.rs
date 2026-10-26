use super::weapon::{
    Category, FitError, Frame, Proc, Tilt, WeaponPart, all_designs_up_to_cap, check, decompose,
    frame_of,
};
use super::*;
use crate::game::gear_power::ReferenceFoe;
use crate::items::EquipmentSlot;
use crate::routine_craft::{AreaKind, StatusKind, Version};
use crate::tests::support::{shipped_affix_db, test_assets_dir};
use crate::tuning as t;

fn abilities() -> crate::abilities::AbilityDb {
    crate::abilities::AbilityDb::load_dir(&test_assets_dir().join("abilities"))
        .unwrap()
        .0
}

fn items() -> crate::items_db::ItemDb {
    crate::items_db::ItemDb::load_dir(&test_assets_dir().join("items"), &abilities())
        .unwrap()
        .0
}

fn foe() -> ReferenceFoe {
    let (species, _) =
        crate::species::SpeciesDb::load_dir(&test_assets_dir().join("species"), &abilities())
            .unwrap();
    ReferenceFoe::median(&species)
}

fn design(parts: &[WeaponPart]) -> GearDesign {
    GearDesign {
        id: 1,
        name: "Test".into(),
        slot: EquipmentSlot::Weapon,
        parts: parts.iter().map(|p| GearPart::Weapon(*p)).collect(),
    }
}

fn build(parts: &[WeaponPart]) -> crate::items_db::ItemDef {
    compile(&design(parts), &shipped_affix_db()).expect("design compiles")
}

fn edge() -> WeaponPart {
    WeaponPart::Frame(Frame::Edge, Version::V1)
}

fn band(def: &crate::items_db::ItemDef) -> (i32, i32) {
    let (_, s) = def.equipment.unwrap();
    (s.damage.min, s.damage.max)
}

// Task 1 ---------------------------------------------------------------

#[test]
fn the_catalogue_represents_every_category_and_omits_the_stun_rider() {
    let all = WeaponPart::all();
    for category in std::iter::once(Category::Frame).chain(Category::MODIFIERS) {
        assert!(
            all.iter().any(|p| p.category() == category),
            "{category:?} has no part"
        );
    }
    assert!(!all.contains(&WeaponPart::Rider(StatusKind::Stun)));
    assert!(all.contains(&WeaponPart::Exploit(StatusKind::Stun)));
    assert_eq!(
        all.iter()
            .filter(|p| p.category() == Category::Frame)
            .count(),
        Frame::ALL.len() * Version::ALL.len()
    );
}

#[test]
fn the_shelf_leaves_payoffs_and_procs_off_and_is_a_subset() {
    let all = WeaponPart::all();
    for part in WeaponPart::shelf() {
        assert!(all.contains(&part));
        assert!(!matches!(
            part,
            WeaponPart::Exploit(_) | WeaponPart::Proc(_)
        ));
    }
}

// Task 2 ---------------------------------------------------------------

#[test]
fn fit_refuses_with_a_typed_reason() {
    let cap = t::GEAR_PART_CAP_START;
    assert_eq!(check(&[], 0, cap), Err(FitError::NoFrame));
    assert_eq!(
        check(
            &[edge(), WeaponPart::Frame(Frame::Lance, Version::V1)],
            0,
            cap
        ),
        Err(FitError::TwoFrames)
    );
    assert_eq!(
        check(&[edge(), WeaponPart::Capture, WeaponPart::Capture], 0, cap),
        Err(FitError::CategoryTaken(Category::Capture))
    );
    assert_eq!(
        check(&[edge(), WeaponPart::Rider(StatusKind::Stun)], 0, cap),
        Err(FitError::StunRider)
    );
    let launcher = WeaponPart::Frame(Frame::Launcher, Version::V1);
    assert_eq!(
        check(&[launcher, WeaponPart::Extend], 0, cap),
        Err(FitError::RangeAtMax(Frame::Launcher))
    );
}

#[test]
fn the_cap_counts_modifiers_and_affixes_but_not_the_frame() {
    let two = [edge(), WeaponPart::Capture, WeaponPart::Extend];
    assert_eq!(check(&two, 0, 2), Ok(()));
    assert_eq!(check(&two, 1, 2), Err(FitError::OverCap { cap: 2 }));
    assert_eq!(check(&[edge()], 2, 2), Ok(()));
}

#[test]
fn rider_and_exploit_may_name_the_same_status() {
    let parts = [
        edge(),
        WeaponPart::Rider(StatusKind::Bleed),
        WeaponPart::Exploit(StatusKind::Bleed),
    ];
    assert_eq!(check(&parts, 0, 2), Ok(()));
}

// Task 3: one test per family -------------------------------------------

#[test]
fn a_frame_sets_range_band_and_grows_with_version() {
    let fig = t::GEAR_FRAME[Frame::Lance.index()];
    let v1 = build(&[WeaponPart::Frame(Frame::Lance, Version::V1)]);
    assert_eq!(v1.range, Some(fig.range));
    assert_eq!(band(&v1), fig.band);
    assert_eq!(v1.id.as_str(), "crafted_gear/1");
    let v3 = build(&[WeaponPart::Frame(Frame::Lance, Version::V3)]);
    assert!(band(&v3).1 > band(&v1).1);
    assert!(build(&[edge()]).range.is_none(), "melee authors no range");
}

#[test]
fn extend_adds_a_cell_and_costs_accuracy() {
    let plain = build(&[edge()]);
    let long = build(&[edge(), WeaponPart::Extend]);
    assert_eq!(long.range, Some(2));
    assert_eq!(
        long.equipment.unwrap().1.accuracy,
        plain.equipment.unwrap().1.accuracy + t::GEAR_EXTEND_ACCURACY
    );
}

#[test]
fn reach_sets_an_enemy_facing_plural_target_and_a_recharge_and_costs_band() {
    let plain = build(&[edge()]);
    let group = build(&[edge(), WeaponPart::Reach(AreaKind::Group)]);
    let all = build(&[edge(), WeaponPart::Reach(AreaKind::Everyone)]);
    let reach = group.reach.as_ref().unwrap();
    assert_eq!(reach.recharge, t::GEAR_REACH_RECHARGE[0]);
    assert_eq!(
        all.reach.as_ref().unwrap().target,
        crate::abilities::AbilityTarget::AllEnemies
    );
    assert!(all.reach.as_ref().unwrap().recharge > reach.recharge);
    assert!(band(&group).1 <= band(&plain).1);
    assert!(band(&all).1 < band(&plain).1 || band(&all).0 < band(&plain).0);
}

#[test]
fn a_damage_type_sets_the_type() {
    let def = build(&[
        edge(),
        WeaponPart::DamageType(crate::damage::DamageType::Noise),
    ]);
    assert_eq!(
        def.equipment.unwrap().1.damage_type,
        Some(crate::damage::DamageType::Noise)
    );
}

#[test]
fn tilt_trades_accuracy_against_band() {
    let plain = build(&[edge()]);
    let precise = build(&[edge(), WeaponPart::Tilt(Tilt::Precise)]);
    let heavy = build(&[edge(), WeaponPart::Tilt(Tilt::Heavy)]);
    let acc = |d: &crate::items_db::ItemDef| d.equipment.unwrap().1.accuracy;
    assert!(acc(&precise) > acc(&plain) && band(&precise).1 <= band(&plain).1);
    assert!(acc(&heavy) < acc(&plain) && band(&heavy).1 >= band(&plain).1);
}

#[test]
fn capture_adds_decompiler() {
    let def = build(&[edge(), WeaponPart::Capture]);
    assert_eq!(
        def.equipment.unwrap().1.decompiler,
        t::GEAR_CAPTURE_DECOMPILER
    );
}

#[test]
fn a_proc_grants_a_routine_that_can_fire() {
    let abilities = abilities();
    for proc in Proc::ALL {
        let def = build(&[edge(), WeaponPart::Proc(proc)]);
        assert_eq!(def.grants, Some(proc.ability()));
        assert_eq!(def.load_fault(&abilities), None);
    }
}

#[test]
fn a_rider_sets_the_runtime_only_field_and_the_exploit_likewise() {
    let def = build(&[
        edge(),
        WeaponPart::Rider(StatusKind::Bleed),
        WeaponPart::Exploit(StatusKind::Stun),
    ]);
    let rider = def.on_hit_rider.as_ref().unwrap();
    assert_eq!(rider.kind, StatusKind::Bleed.id());
    assert_eq!(rider.chance, t::GEAR_RIDER[0].0);
    let exploit = def.exploit.as_ref().unwrap();
    assert_eq!(exploit.status, StatusKind::Stun.id());
    assert_eq!(exploit.percent, t::GEAR_EXPLOIT_PERCENT[1]);
    assert!(build(&[edge()]).on_hit_rider.is_none());
}

#[test]
fn an_affix_folds_into_the_stats_before_scaling() {
    let affixes = shipped_affix_db();
    let id = affixes
        .pool_for(EquipmentSlot::Weapon)
        .into_iter()
        .find(|a| a.stats.atk > 0)
        .expect("a weapon affix with attack ships")
        .id
        .clone();
    let mut d = design(&[edge()]);
    d.parts.push(GearPart::Affix(id.clone()));
    let def = compile(&d, &affixes).unwrap();
    let plain = build(&[edge()]);
    assert!(def.equipment.unwrap().1.atk > plain.equipment.unwrap().1.atk);

    d.parts = vec![GearPart::Affix("no_such_affix".into())];
    d.parts.insert(0, GearPart::Weapon(edge()));
    assert!(matches!(
        compile(&d, &affixes),
        Err(CompileError::UnknownAffix(_))
    ));
}

#[test]
fn only_weapons_compile() {
    let mut d = design(&[edge()]);
    d.slot = EquipmentSlot::Armor;
    assert!(matches!(
        compile(&d, &shipped_affix_db()),
        Err(CompileError::UnsupportedSlot(EquipmentSlot::Armor))
    ));
}

// Task 4 ---------------------------------------------------------------

fn every_design() -> Vec<(Vec<WeaponPart>, crate::items_db::ItemDef)> {
    let affixes = shipped_affix_db();
    all_designs_up_to_cap(t::GEAR_PART_CAP_MAX)
        .into_iter()
        .map(|parts| {
            let def = compile(&design(&parts), &affixes)
                .unwrap_or_else(|e| panic!("{parts:?} does not compile: {e:?}"));
            (parts, def)
        })
        .collect()
}

#[test]
fn every_design_passes_the_authored_weapon_validators() {
    let abilities = abilities();
    let designs = every_design();
    assert!(designs.len() > 100_000, "census saw {}", designs.len());
    for (parts, def) in &designs {
        assert_eq!(def.load_fault(&abilities), None, "{parts:?}");
        assert!(matches!(def.equipment, Some((EquipmentSlot::Weapon, _))));
        let (_, stats) = def.equipment.unwrap();
        assert!(stats.damage.min >= 1 && stats.damage.max >= stats.damage.min);
        assert!(!def.description.is_empty());
    }
}

#[test]
fn a_typed_crafted_weapon_has_an_armour_resisting_its_type() {
    let items = items();
    for ty in crate::damage::DamageType::ALL {
        assert!(
            items.all().any(|def| matches!(def.worn_stats(),
                Some((EquipmentSlot::Armor, s)) if s.resist.get(ty) > 0)),
            "a weapon can deal {} but no shipped armour resists it",
            ty.name()
        );
    }
}

// Task 5 ---------------------------------------------------------------

fn authored_weapons() -> Vec<crate::items_db::ItemDef> {
    items()
        .all()
        .filter(|d| matches!(d.equipment, Some((EquipmentSlot::Weapon, _))))
        .cloned()
        .collect()
}

#[test]
fn no_design_passes_its_frame_ceiling() {
    let foe = foe();
    let mut over = Vec::new();
    for (parts, def) in every_design() {
        let (frame, version) = frame_of(&parts).unwrap();
        let power = design_power(&def, &foe).expect("a weapon rates");
        if power > t::GEAR_FRAME_CEILING[frame.index()][version.index()] {
            over.push((parts, power));
        }
    }
    assert!(
        over.is_empty(),
        "{} over the ceiling, e.g. {:?}",
        over.len(),
        &over[..over.len().min(3)]
    );
}

/// A ceiling that passed an authored weapon would make the drops pointless.
/// Weapons with mitigation are left out: `copy_power` prices the armour on
/// them as survivability, which is not what a frame competes on.
#[test]
fn a_ceiling_never_passes_the_authored_weapon_of_its_range() {
    let foe = foe();
    let authored = authored_weapons();
    for frame in Frame::ALL {
        let range = t::GEAR_FRAME[frame.index()].range;
        let best = authored
            .iter()
            .filter(|d| d.range.unwrap_or(1) == range)
            .filter(|d| d.equipment.unwrap().1.mitigation == 0)
            .filter_map(|d| design_power(d, &foe))
            .max()
            .expect("an authored weapon at every frame's range");
        let top = t::GEAR_FRAME_CEILING[frame.index()][2];
        assert!(
            top <= best,
            "{frame:?} v3 ceiling {top} passes the authored {best}"
        );
    }
}

/// A ceiling well above every design is no ceiling: the tallest design of
/// each frame and version sits within a point of it.
#[test]
fn each_ceiling_is_tight() {
    let foe = foe();
    let mut best = [[i32::MIN; 3]; 4];
    for (parts, def) in every_design() {
        let (f, v) = frame_of(&parts).unwrap();
        let cell = &mut best[f.index()][v.index()];
        *cell = (*cell).max(design_power(&def, &foe).unwrap());
    }
    for (f, row) in best.iter().enumerate() {
        for (v, tallest) in row.iter().enumerate() {
            let ceiling = t::GEAR_FRAME_CEILING[f][v];
            assert!(
                (0..=1).contains(&(ceiling - tallest)),
                "frame {f} v{}: ceiling {ceiling}, tallest design {tallest}",
                v + 1,
            );
        }
    }
}

#[test]
fn ceilings_rise_with_version() {
    for row in t::GEAR_FRAME_CEILING {
        assert!(row[0] < row[1] && row[1] < row[2]);
    }
}

type Axes = [i32; 10];

/// An axis vector, higher better on every entry: what a weapon pays on its
/// own axes (band, accuracy, range, how many it reaches and how soon it swings
/// again, decompiler, a type) and what each flat term buys, kept apart so a
/// rider is not weighed against a proc on one shared number.
fn axes(def: &crate::items_db::ItemDef, parts: &[WeaponPart]) -> Axes {
    let reach = def.reach.as_ref();
    let stats = def.equipment.unwrap().1;
    [
        (super::weapon::exact_band_of(parts) * 1000.0) as i32,
        stats.accuracy,
        def.on_hit_rider.as_ref().map_or(0, |r| {
            t::GEAR_RIDER_POWER[StatusKind::from_id(&r.kind).unwrap().index()]
        }),
        def.exploit.as_ref().map_or(0, |e| {
            t::GEAR_EXPLOIT_POWER[StatusKind::from_id(&e.status).unwrap().index()]
        }),
        def.grants
            .as_ref()
            .and_then(Proc::from_ability)
            .map_or(0, |p| t::GEAR_PROC_POWER[p.index()]),
        def.range.unwrap_or(1) as i32,
        match reach.map(|r| r.target) {
            None => 0,
            Some(crate::abilities::AbilityTarget::WholeEnemyGroup) => 1,
            Some(_) => 2,
        },
        -(reach.map_or(0, |r| r.recharge) as i32),
        def.equipment.unwrap().1.decompiler,
        i32::from(def.equipment.unwrap().1.damage_type.is_some()),
    ]
}

fn dominates(a: &Axes, b: &Axes) -> bool {
    a.iter().zip(b).all(|(x, y)| x >= y) && a != b
}

#[test]
fn no_set_of_modifiers_beats_another_of_the_same_frame_and_size() {
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<(Frame, Version, usize), BTreeMap<Axes, Vec<WeaponPart>>> =
        BTreeMap::new();
    for (parts, def) in every_design() {
        let (frame, version) = frame_of(&parts).unwrap();
        groups
            .entry((frame, version, parts.len()))
            .or_default()
            .entry(axes(&def, &parts))
            .or_insert(parts);
    }
    let mut found = Vec::new();
    for (key, vectors) in &groups {
        let all: Vec<_> = vectors.iter().collect();
        for (a, pa) in &all {
            for (b, pb) in &all {
                if dominates(a, b) {
                    found.push((key, (*pa).clone(), (*pb).clone()));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "{} dominating pairs, e.g. {:?}",
        found.len(),
        &found[..found.len().min(12)]
    );
}

#[test]
fn a_weapon_rider_is_weaker_than_a_routines() {
    for status in StatusKind::ALL {
        if status == StatusKind::Stun {
            continue;
        }
        let (chance, rounds, power) = t::GEAR_RIDER[status.index()];
        let (r_chance, r_rounds, r_power) = t::CRAFT_RIDER[status.index()];
        assert!(chance < r_chance, "{status:?} chance");
        assert!(
            rounds <= r_rounds && power <= r_power,
            "{status:?} strength"
        );
    }
}

// Task 6 ---------------------------------------------------------------

#[test]
fn every_authored_weapon_maps_to_a_frame() {
    let abilities = abilities();
    let affixes = shipped_affix_db();
    let weapons = authored_weapons();
    assert_eq!(weapons.len(), 15);
    for def in &weapons {
        let parts = decompose(def).unwrap_or_else(|| panic!("{} has no frame", def.id.as_str()));
        assert!(matches!(parts[0], WeaponPart::Frame(..)));
        assert_eq!(
            check(&parts, 0, t::GEAR_PART_CAP_MAX),
            Ok(()),
            "{}",
            def.id.as_str()
        );
        let built = compile(&design(&parts), &affixes).unwrap();
        assert_eq!(built.load_fault(&abilities), None);
        if let Some(ty) = def.equipment.unwrap().1.damage_type {
            assert!(parts.contains(&WeaponPart::DamageType(ty)));
        }
    }
}

#[test]
fn decompose_maps_decompiler_to_capture_grants_to_proc_and_drops_mitigation() {
    let items = items();
    let parts = |id: &str| decompose(items.get(id).unwrap()).unwrap();
    assert!(parts("shim_blade").contains(&WeaponPart::Capture));
    assert!(parts("black_ice_pick").contains(&WeaponPart::Capture));
    assert!(parts("ragged_edge").contains(&WeaponPart::Proc(Proc::ClockSkew)));
    assert!(parts("crash_handler").contains(&WeaponPart::Proc(Proc::CoreDump)));
    assert!(parts("interrupt_coil").contains(&WeaponPart::Proc(Proc::InterruptRequest)));
    // Mitigation maps to nothing: nothing in the result could carry it.
    let recursion = parts("recursion_blade");
    assert_eq!(recursion.len(), 1);
    let compiled = compile(&design(&recursion), &shipped_affix_db()).unwrap();
    assert_eq!(compiled.equipment.unwrap().1.mitigation, 0);
    assert!(parts("broadcast_storm").contains(&WeaponPart::Reach(AreaKind::Everyone)));
    assert!(parts("scatter_lance").contains(&WeaponPart::Reach(AreaKind::Group)));
}
