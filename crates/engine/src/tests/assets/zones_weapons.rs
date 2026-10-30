use crate::tests::support::*;
use crate::*;

/// The materials a breach unlocks, in the order the zones hand them over.
/// A list here rather than a derivation, because "this item is what zone N
/// pays you" is a content decision and there is nothing in `ItemDef` that
/// could be read to recover it — see the two censuses below, which are what
/// stop the decision being reverted one `.ron` file at a time.
pub(super) const ZONE_MATERIALS: &[&str] = &["cache_grain"];

/// A breach has to change *what* you can make, not only how fast. Every
/// gear recipe a zone-gated research node hands over is denominated in the
/// material of the zone that gates it, so the Cache Tap is on the path to
/// the gear rather than beside it.
///
/// Asserted over the recipes rather than the item files because that is
/// where the zone gate already lives: `ResearchDef::min_zone` and
/// `unlocks_recipes` are one file, so a node that keeps its gate and loses
/// its material is a single-line edit this catches.
#[test]
fn every_zone_gated_gear_recipe_asks_for_a_zone_material() {
    let game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut checked = 0;
    for def in game.world.resource::<crate::research::ResearchDb>().all() {
        if def.min_zone < 2 {
            continue;
        }
        for recipe in &def.unlocks_recipes {
            assert!(
                recipe
                    .cost
                    .iter()
                    .any(|(id, _)| ZONE_MATERIALS.contains(&id.as_str())),
                "{} is gated on zone {} but its recipe for {} names no zone material, so breaching buys the blueprint and nothing has to be mined for it",
                def.id.as_str(),
                def.min_zone,
                recipe.result.as_str()
            );
            checked += 1;
        }
    }
    assert_eq!(
        checked, 18,
        "expected every zone-gated recipe the tree unlocks — eight of gear, \
         four of Power cells and six implants; one that lost its recipe would drop out of this \
         scan unnoticed"
    );
}

/// The upgrade half of the same payoff. `upgrade_ceiling` is already
/// `min(max_tier, zone)`, so tier 2 is unreachable before the second zone —
/// which is what makes naming the zone-2 material in an upgrade cost free of
/// any effect on zone 1, and what makes it the natural place to spend one.
#[test]
fn every_upgrade_path_asks_for_a_zone_material() {
    let game = Game::new(82, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut checked = 0;
    for def in game.structure_defs() {
        let Some(upgrade) = &def.upgrade else {
            continue;
        };
        assert!(
            upgrade
                .cost
                .iter()
                .any(|(id, _)| ZONE_MATERIALS.contains(&id.as_str())),
            "{} upgrades on fragments alone, so every tier past the first is reachable the moment the zone number allows it",
            def.id.as_str()
        );
        checked += 1;
    }
    assert_eq!(
        checked, 6,
        "expected the six upgradeable structures — the Workbench, the Extractor, the Research Node, the Patch Node, the Teardown Rig and the Quarantine Rack; one that lost its path would drop out of this scan unnoticed"
    );
}

/// Every line the Zone Portal demands at sector N names something the base
/// can actually be making by sector N.
///
/// `ZONE_MATERIALS`' shape, and its reason: nothing in `StructureDef` says the
/// portal's bill has to be *reachable*, and the constraint is entirely a
/// content decision. `zone_build_cost` exists precisely because two of the
/// lines are not legal in sector 1 — Cache Grain behind `cache_coherence` and
/// the Recompile Kernel behind `program_refactoring`, both `min_zone: 2` — and
/// that argument is written down in
/// `seam:a-zone-portal-line-is-ramped-from-the-zone-it-was` and asserted
/// nowhere. Moving `("cache_grain", 10)` up into `build_cost` makes sector 1
/// unbreachable, and would have failed no test.
///
/// A line naming something **no** structure makes and no research unlocks is
/// not gated by research at all: it is a drop, and `PORTAL_DROPPED_LINES`
/// names the ones that are so a new uncraftable line cannot slip through as
/// "nothing gates it".
const PORTAL_DROPPED_LINES: &[&str] = &["portal_fragment"];

/// The earliest zone each research node can be bought in: its own `min_zone`
/// and every node it waits on, whichever is deepest. Zone numbering starts
/// at 1, and `min_zone: 0` — the default, meaning ungated — reads as 1.
fn research_reachable_zone(
    db: &crate::research::ResearchDb,
) -> std::collections::HashMap<String, u32> {
    let mut zones: std::collections::HashMap<String, u32> = db
        .all()
        .map(|d| (d.id.to_string(), d.min_zone.max(1)))
        .collect();
    // Fixed point rather than recursion: the tree is two dozen nodes and a
    // cycle in a mod's `requires` would otherwise be a stack overflow rather
    // than a census failure.
    loop {
        let mut moved = false;
        for def in db.all() {
            let deepest = def
                .requires
                .iter()
                .filter_map(|r| zones.get(r.as_str()).copied())
                .max()
                .unwrap_or(1)
                .max(def.min_zone.max(1));
            let entry = zones.get_mut(def.id.as_str()).expect("seeded above");
            if *entry < deepest {
                *entry = deepest;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    zones
}

#[test]
fn every_zone_portal_line_is_makeable_by_the_sector_that_demands_it() {
    let game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let research = game.world.resource::<crate::research::ResearchDb>();
    let structures = game.world.resource::<StructureDb>();
    let items = game.world.resource::<crate::items_db::ItemDb>();
    let node_zone = research_reachable_zone(research);

    // The gated structures first, each at the shallowest node that unlocks
    // it. Seeding every structure at 1 up front and then taking a `min`
    // reads the same and is wrong: the seed wins every comparison, and the
    // whole census passes on a bill that cannot be paid.
    let mut structure_zone: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    for def in research.all() {
        let zone = node_zone[def.id.as_str()];
        for kind in &def.unlocks_structures {
            let entry = structure_zone.entry(kind.to_string()).or_insert(u32::MAX);
            *entry = (*entry).min(zone);
        }
    }
    // A structure named by no research file is buildable from turn one —
    // `assets/research/README.md` states that rule, and it is what keeps the
    // Assembly Bay in sector 1's bill.
    for def in structures.all() {
        structure_zone.entry(def.id.to_string()).or_insert(1);
    }

    // The earliest zone anything can put a unit of an item on a shelf: the
    // machine that makes it, the bench its own recipe names, or the research
    // node that hands the recipe over. `None` means nothing makes it.
    let item_zone = |id: &ItemId| -> Option<u32> {
        let mut best: Option<u32> = None;
        let mut consider = |zone: u32| {
            best = Some(best.map_or(zone, |b: u32| b.min(zone)));
        };
        for def in structures.all() {
            // The machine is buildable from the start, so what gates a
            // listed recipe is the research it names.
            for production in def.productions().filter(|p| &p.recipe.produces == id) {
                let gate = production
                    .recipe
                    .research
                    .as_ref()
                    .map_or(1, |r| node_zone[r.as_str()]);
                consider(structure_zone[def.id.as_str()].max(gate));
            }
        }
        if let Some(craftable) = items.get(id.as_str()).and_then(|d| d.craftable.as_ref()) {
            let bench = craftable
                .requires_structure
                .as_ref()
                .map_or(1, |k| structure_zone[k.as_str()]);
            let gate = craftable.research.as_deref().map_or(1, |r| node_zone[r]);
            consider(bench.max(gate));
        }
        for def in research.all() {
            for recipe in &def.unlocks_recipes {
                if &recipe.result != id {
                    continue;
                }
                let bench = recipe
                    .requires_structure
                    .as_ref()
                    .map_or(1, |k| structure_zone[k.as_str()]);
                consider(node_zone[def.id.as_str()].max(bench));
            }
        }
        best
    };

    let portal = structures
        .all()
        .find(|d| d.zone_portal)
        .expect("one shipped structure breaches");
    let lines: Vec<(u32, &ItemId)> = portal
        .build_cost
        .iter()
        .map(|(item, _)| (1, item))
        .chain(
            portal
                .zone_build_cost
                .iter()
                .map(|(min_zone, item, _)| (*min_zone, item)),
        )
        .collect();
    assert!(
        lines.len() >= 4,
        "the portal's bill lost its base lines, and this scan would pass on an empty one"
    );

    let mut dropped = 0;
    for (demanded_at, item) in lines {
        let Some(makeable_at) = item_zone(item) else {
            assert!(
                PORTAL_DROPPED_LINES.contains(&item.as_str()),
                "the portal demands {} at sector {demanded_at} and nothing in the game \
                 makes one — add it to PORTAL_DROPPED_LINES if that is deliberate",
                item.as_str()
            );
            dropped += 1;
            continue;
        };
        assert!(
            makeable_at <= demanded_at,
            "the portal demands {} at sector {demanded_at}, but nothing can make one \
             before sector {makeable_at} — that sector cannot be breached",
            item.as_str()
        );
    }
    assert_eq!(
        dropped,
        PORTAL_DROPPED_LINES.len(),
        "a line in PORTAL_DROPPED_LINES stopped being demanded, or became craftable"
    );
}

/// The talent-tree censuses, over the **real** `assets/talents/` rather than a
/// fixture. Nothing in `TalentDb::load_dir` enforces any of this beyond the
/// shape — a mod's tree is never refused for being thin — so these are what
/// hold the shipped content to what the design says it is.
mod talents {
    use super::*;
    use crate::species::AffinityClass;
    use crate::talents::{CHOICES_PER_TIER, TalentDb, TalentNode, tiers_per_tree};
    use crate::tuning::{MAX_TALENT_ACCURACY_POINTS, MAX_TALENT_STAT_PERCENT};

    fn shipped() -> TalentDb {
        let (db, warnings) = TalentDb::load_dir(&test_assets_dir().join("talents")).unwrap();
        assert!(
            warnings.is_empty(),
            "shipped trees must load clean: {warnings:?}"
        );
        db
    }

    /// Driven by the enum rather than by naming five files, so a sixth class
    /// fails this test the day it is added rather than silently inheriting the
    /// generic tree.
    #[test]
    fn every_class_has_a_tree_and_so_does_the_generic_fallback() {
        let db = shipped();
        for class in AffinityClass::ALL {
            assert!(
                db.get(Some(class)).is_some_and(|t| t.class == Some(class)),
                "{class:?} has no tree of its own"
            );
        }
        assert!(
            db.get(None).is_some_and(|t| t.class.is_none()),
            "a program with no readable class still spends its points somewhere"
        );
    }

    #[test]
    fn every_tree_is_the_full_depth_and_offers_two_choices_a_tier() {
        let db = shipped();
        for tree in db.trees() {
            assert_eq!(
                tree.tiers.len(),
                tiers_per_tree(),
                "{:?}'s tree is not one tier per level a ringed companion earns",
                tree.class
            );
            for (i, tier) in tree.tiers.iter().enumerate() {
                assert_eq!(
                    tier.0.len(),
                    CHOICES_PER_TIER,
                    "{:?} tier {} is not a decision",
                    tree.class,
                    i + 1
                );
            }
        }
    }

    /// Both halves: the id resolves, and it is a **battle** ability.
    /// `AffinityKind` is blind to the distinction — a `FieldBuff(kind: Mitigation)`
    /// reports `Buff` like any other buff while never appearing in the Special
    /// picker, which is the one place a granted routine is spent.
    #[test]
    fn every_ability_node_names_a_battle_routine_that_exists() {
        let db = shipped();
        let (abilities, _) =
            crate::abilities::AbilityDb::load_dir(&test_assets_dir().join("abilities")).unwrap();
        for choice in db.all_nodes() {
            let TalentNode::Ability { id } = &choice.node else {
                continue;
            };
            let def = abilities
                .get(id)
                .unwrap_or_else(|| panic!("talent {} names no such ability {id:?}", choice.id));
            assert!(
                !def.effect.field_only(),
                "talent {} grants {id:?}, which never appears in the Special picker",
                choice.id
            );
        }
    }

    /// A developed companion already carries four multiplicative axes, and
    /// options compound far less dangerously than numbers — so a `Stat` node's
    /// percentage is bounded, and the trees are weighted away from them.
    #[test]
    fn no_stat_node_is_worth_more_than_its_ceiling() {
        let db = shipped();
        for choice in db.all_nodes() {
            if let TalentNode::Stat { percent, .. } = choice.node {
                assert!(
                    percent > 0.0 && percent <= MAX_TALENT_STAT_PERCENT,
                    "talent {} raises a stat by {percent}%, past MAX_TALENT_STAT_PERCENT",
                    choice.id
                );
            }
        }
    }

    /// The same bound on the axis that has no `Stats` field to hold it.
    /// Accuracy feeds a ratio, so an unbounded node would walk a companion to
    /// `HIT_CHANCE_MAX` on its own and take every later decision in its tree
    /// with it.
    #[test]
    fn no_accuracy_node_is_worth_more_than_its_ceiling() {
        let db = shipped();
        let mut seen = 0;
        for choice in db.all_nodes() {
            if let TalentNode::Accuracy { points } = choice.node {
                seen += 1;
                assert!(
                    points > 0 && points <= MAX_TALENT_ACCURACY_POINTS,
                    "talent {} raises Accuracy by {points}, outside \
                     1..={MAX_TALENT_ACCURACY_POINTS}",
                    choice.id
                );
            }
        }
        assert!(
            seen > 0,
            "no tree offers the accuracy axis at all, so the node ships dark"
        );
    }

    /// `Game::take_talent` resolves a node by id against the whole tree, so two
    /// nodes sharing one id would make which of them a player bought depend on
    /// tier order.
    #[test]
    fn talent_ids_are_unique_across_every_tree() {
        let db = shipped();
        let mut seen = std::collections::HashSet::new();
        for choice in db.all_nodes() {
            assert!(
                seen.insert(choice.id.clone()),
                "{} appears in more than one tree",
                choice.id
            );
        }
    }

    /// The weighting rule, asserted rather than left to authorial memory: a
    /// tree of nothing but percentages is four more multiplicative axes on a
    /// companion that already has four.
    #[test]
    fn every_tree_spends_most_of_itself_on_options_rather_than_numbers() {
        let db = shipped();
        for tree in db.trees() {
            let nodes: Vec<_> = tree.tiers.iter().flat_map(|t| t.0.iter()).collect();
            // `Accuracy` counts as a number even though it never reaches
            // `Stats`: what this rule is about is how much of a tree is spent
            // on a figure going up rather than on a decision, and a flat
            // Accuracy node is as much a figure as a percentage is.
            let numbers = nodes
                .iter()
                .filter(|c| {
                    matches!(
                        c.node,
                        TalentNode::Stat { .. } | TalentNode::Accuracy { .. }
                    )
                })
                .count();
            assert!(
                numbers * 2 <= nodes.len(),
                "{:?}'s tree is {numbers} number nodes of {} — options compound less \
                 dangerously than numbers",
                tree.class,
                nodes.len()
            );
        }
    }
}

/// Every shipped species authors a mitigation percentage inside the band the
/// cap allows. A species at or past `MAX_MITIGATION_PERCENT` is immune
/// before gear or a buff is counted, which the cap would silently swallow.
#[test]
fn every_species_mitigation_leaves_room_under_the_cap() {
    let game = Game::new(3311, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for species in game.species_defs() {
        assert!(
            (0..crate::tuning::MAX_MITIGATION_PERCENT).contains(&species.base_mitigation),
            "{} authors base_mitigation {}, outside 0..{}",
            species.id,
            species.base_mitigation,
            crate::tuning::MAX_MITIGATION_PERCENT
        );
    }
}

/// Every shipped move authors a range that can actually vary, and none can
/// roll negative. A roster of degenerate ranges would ship the feature dark.
#[test]
fn every_shipped_move_authors_a_real_damage_range() {
    let game = Game::new(3312, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for species in game.species_defs() {
        for mv in &species.moves {
            let range = mv.range();
            assert!(
                range.min >= 0,
                "{} / {} can roll negative",
                species.id,
                mv.name
            );
            assert!(
                range.max > range.min,
                "{} / {} is a degenerate range — a shipped move must vary",
                species.id,
                mv.name
            );
        }
    }
}

/// The two defensive axes must be a real choice rather than one stat with a
/// second name: some shipped armour has to buy evasion instead of
/// mitigation, and some shipped weapons accuracy instead of damage. A field
/// nothing authors is an unused feature flag.
#[test]
fn the_shipped_gear_actually_authors_both_defensive_and_both_offensive_axes() {
    let game = Game::new(3313, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let equipment: Vec<_> = game
        .item_defs()
        .into_iter()
        .filter_map(|i| i.equipment)
        .collect();
    assert!(
        equipment.iter().any(|(_, stats)| stats.evasion > 0),
        "no shipped armour buys evasion"
    );
    assert!(
        equipment.iter().any(|(_, stats)| stats.mitigation > 0),
        "no shipped armour buys mitigation"
    );
    assert!(
        equipment.iter().any(|(_, stats)| stats.accuracy > 0),
        "no shipped weapon buys accuracy"
    );
    assert!(
        equipment
            .iter()
            .any(|(_, stats)| stats.damage != crate::battle::DamageRange::default()),
        "no shipped weapon authors a damage range"
    );
}

/// Every shipped weapon carries a damage range, and nothing else does. A
/// weapon **overrides** a natural attack rather than adding to it, so a
/// weapon with no range would silently disarm whoever equipped it.
#[test]
fn every_weapon_authors_a_range_and_nothing_else_does() {
    let game = Game::new(3314, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for item in game.item_defs() {
        let Some((slot, stats)) = item.equipment.as_ref() else {
            continue;
        };
        let has_range = stats.damage != crate::battle::DamageRange::default();
        assert_eq!(
            has_range,
            *slot == EquipmentSlot::Weapon,
            "{} is a {slot:?} and {} a damage range",
            item.id,
            if has_range { "has" } else { "lacks" }
        );
    }
}

/// Every shipped reach weapon authors a recharge above zero.
///
/// A wide swing on **every** swing is a straight throughput multiplier that
/// nothing in the balance gate would see — `balance_sim` models no reach at
/// all — so the recharge is what pays for the breadth. `serde`'s `u32`
/// default is 0, and `ItemDb::load_dir` deliberately does not refuse one
/// (`recharge: 0` is inert rather than special-cased, so a mod may author
/// it), which means nothing but this census catches a forgotten field on a
/// shipped file.
#[test]
fn every_shipped_reach_weapon_authors_a_recharge() {
    let game = Game::new(3315, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut checked = 0;
    for item in game.item_defs() {
        let Some(reach) = item.reach else {
            continue;
        };
        assert!(
            reach.recharge > 0,
            "{} swings wide on every swing, which no balance instrument can see",
            item.id
        );
        checked += 1;
    }
    assert!(
        checked >= 2,
        "the shipped set should carry both reach weapons, not {checked}"
    );
}

/// A range outside the window is refused at load, so a shipped weapon
/// sitting outside it would be a weapon that silently is not in the game at
/// all — one warning nobody reads, and a recipe that builds nothing.
#[test]
fn every_authored_weapon_range_is_inside_its_window() {
    let game = Game::new(3317, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut authored = 0;
    for def in game.item_defs() {
        let Some(range) = def.range else { continue };
        authored += 1;
        assert!(
            (1..=crate::tuning::TACTICAL_WEAPON_RANGE_MAX).contains(&range),
            "{} authors range {range}, outside 1..={}",
            def.id.as_str(),
            crate::tuning::TACTICAL_WEAPON_RANGE_MAX
        );
        assert!(
            matches!(
                def.equipment,
                Some((crate::items::EquipmentSlot::Weapon, _))
            ),
            "{} authors a range and is not a weapon",
            def.id.as_str()
        );
    }
    assert!(
        authored >= 7,
        "the shipped reaching weapons stopped authoring a range: {authored}"
    );
}

/// Belt and braces over `ItemDef::unreachable_reach`, asserted against the
/// shipped tree rather than a temp file: the loader's refusals are what
/// stop a *mod* shipping an incoherent reach, and this is what stops one
/// being edited into this repo's own content and silently skipped at
/// startup — a shipped weapon that fails to load is a weapon that is simply
/// not in the game, with one warning nobody reads.
#[test]
fn every_shipped_reach_weapon_is_enemy_facing() {
    let game = Game::new(3316, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for item in game.item_defs() {
        let Some(reach) = item.reach else {
            continue;
        };
        assert!(
            matches!(
                reach.target,
                crate::abilities::AbilityTarget::WholeEnemyGroup
                    | crate::abilities::AbilityTarget::AllEnemies
            ),
            "{} reaches {:?}, which is either the party's own side or one body",
            item.id,
            reach.target
        );
        assert_ne!(
            reach.tactical_shape(),
            crate::abilities::AbilityShape::Single,
            "{} resolves to a single cell on a battle map and so reaches nobody",
            item.id
        );
    }
}
