use super::zones_weapons::ZONE_MATERIALS;
use crate::tests::support::*;
use crate::*;

/// The game has one word for the defensive stat, and it is not `Defense`.
///
/// `Stats::def` became `Stats::mitigation` in the combat model rewrite and
/// the authored prose did not follow it: ten shipped ability descriptions
/// still said `DEF`, and nine of those quoted a magnitude the very same
/// commit had tripled — `bastion` read "+4 DEF" against an authored power
/// of 12 for months. A screen's wording is held by the test that renders
/// it; nothing at all compiles a `.ron` description, so this is the half a
/// rename sweep forgets.
///
/// Scoped to authored player text — `description:` lines in every shipped
/// `.ron`, and the manual's pages, which are prose end to end. A `//`
/// comment naming the retired field is history and is deliberately left
/// alone, as is `raid_defense`, which is a structure's own separate stat
/// and a mod-facing field name besides. So is lower-case "defense": a
/// research node describing "automated perimeter defense" is using the
/// ordinary word and is not making a claim about anybody's stat block.
#[test]
fn no_shipped_description_calls_mitigation_defense() {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("the shipped asset tree is readable") {
            let path = entry.expect("a readable directory entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(&test_assets_dir(), &mut files);
    assert!(!files.is_empty(), "the shipped asset tree is not empty");

    // The capitalised word and the bare token only. Lower-case "defense" is
    // ordinary English about a base holding a perimeter — Fortification's
    // description is exactly that — and reads as the stat to nobody.
    let stale = |line: &str| {
        line.contains("Defense")
            || line.match_indices("DEF").any(|(i, _)| {
                let before = line[..i].chars().next_back();
                let after = line[i + 3..].chars().next();
                let boundary =
                    |c: Option<char>| !c.is_some_and(|c| c.is_alphanumeric() || c == '_');
                boundary(before) && boundary(after)
            })
    };
    let mut offenders = Vec::new();
    for path in files {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let is_help = ext == "md" && path.parent().is_some_and(|p| p.ends_with("help"));
        if ext != "ron" && !is_help {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (i, line) in text.lines().enumerate() {
            let authored = is_help || line.trim_start().starts_with("description:");
            // `raid_defense` is a structure's own stat and a field name, so a
            // line carrying one says nothing about this rename.
            if authored && stale(line) && !line.contains("raid_defense") {
                offenders.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "shipped player text still calls mitigation Defense/DEF:\n{}",
        offenders.join("\n")
    );
}

/// Every shipped equipment item must rate `Some`.
///
/// A weapon, armour or module that rates `None` is one whose whole bonus
/// sits on axes `Game::copy_power` does not price — a formula gap, and it
/// must fail the build rather than draw an em dash in the shipping game
/// where the player expects a number.
///
/// The one legitimate `None` is a piece paying only in `decompiler`, which
/// buys taming rather than combat. That is a real category, so the census
/// names those explicitly rather than exempting a whole slot: a *new* one
/// has to be added here deliberately.
#[test]
fn every_shipped_equipment_item_rates_a_power() {
    let game = Game::new(931, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ids: Vec<ItemId> = game
        .world
        .resource::<crate::items_db::ItemDb>()
        .all()
        .filter(|def| def.equipment.is_some())
        .map(|def| def.id.clone())
        .collect();
    assert!(
        !ids.is_empty(),
        "the shipped set defines no equipment at all"
    );
    let mut unrated = Vec::new();
    for id in ids {
        let (_, base) = game.equipment_of(&id).unwrap();
        // A copy whose only authored bonus is `decompiler` has no combat
        // axis by construction, and `copy_power` says so on purpose.
        let combat_only_decompiler = base.atk == 0
            && base.mitigation == 0
            && base.accuracy == 0
            && base.evasion == 0
            && base.damage == crate::battle::DamageRange::default();
        if game.copy_power(&gear(&id, 0)).is_none() && !combat_only_decompiler {
            unrated.push(id);
        }
    }
    assert!(
        unrated.is_empty(),
        "these shipped equipment items rate no power at all — the formula is missing an axis \
         they pay on: {unrated:?}"
    );
}

/// ...and nothing that is not equipment may rate at all. A consumable, a
/// material or a currency has no combat axis, so a `Some` here means the
/// formula is reading something it should not.
#[test]
fn no_shipped_non_equipment_item_rates_a_power() {
    let game = Game::new(932, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ids: Vec<ItemId> = game
        .world
        .resource::<crate::items_db::ItemDb>()
        .all()
        .filter(|def| def.equipment.is_none())
        .map(|def| def.id.clone())
        .collect();
    assert!(!ids.is_empty());
    let rated: Vec<&ItemId> = ids
        .iter()
        .filter(|id| game.copy_power(&gear(id, 0)).is_some())
        .collect();
    assert!(
        rated.is_empty(),
        "these shipped items are not equipment and yet rate a combat power: {rated:?}"
    );
}

/// The Repair Bay is the one way a downed program comes back, so it has to
/// be affordable in the zone where a wipe first happens — a gate you cannot
/// pay for in zone 1 is a dead run, not pressure. It is also passive: no
/// `work` block, so nothing has to be posted to it, and no research node
/// unlocks it.
///
/// A census rather than a rule in Rust, `every_shipped_integrity_routine`'s
/// shape: `StructureDef::recovery` is `#[serde(default)]` so a mod can ship
/// a second Bay, and what this holds is the *shipped* content to the design.
#[test]
fn the_shipped_repair_bay_is_passive_and_affordable_in_zone_one() {
    let game = Game::new(3401, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let bays: Vec<_> = game
        .world
        .resource::<crate::structures::StructureDb>()
        .all()
        .filter(|d| d.recovery.is_some())
        .collect();
    assert_eq!(
        bays.len(),
        1,
        "expected exactly one shipped structure to declare `recovery`"
    );
    let bay = bays[0];
    let recovery = bay.recovery.as_ref().unwrap();
    assert!(
        recovery.per_tick > 0,
        "a Bay that repairs nothing per tick is a Bay a downed program never leaves"
    );
    assert!(
        bay.work.is_none(),
        "the Bay is passive — a `work` block would make it a machine wanting a body"
    );
    for (item, _) in &bay.build_cost {
        assert!(
            !ZONE_MATERIALS.contains(&item.as_str()),
            "{} is built from {}, a zone material, so it cannot be raised in the \
             zone where the first wipe happens",
            bay.id.as_str(),
            item.as_str()
        );
    }
    for def in game.world.resource::<crate::research::ResearchDb>().all() {
        assert!(
            !def.unlocks_structures.contains(&bay.id),
            "{} gates the Bay behind research; recovering from a wipe is not a \
             blueprint you have to earn",
            def.id.as_str()
        );
    }
}

// ---------------------------------------------------------------------------
// The onboarding chain
// ---------------------------------------------------------------------------

/// **The shipped chain cannot stall on economy.**
///
/// An unbreakable chain fails in a way an optional contract does not: a
/// mission the player cannot afford to finish ends onboarding for the rest
/// of the run, with no key to press. This walks the chain in step order
/// carrying a Core Fragment balance — starting from what a run starts with,
/// crediting each mission's payout and debiting each `Build`'s cost — and
/// asserts it never goes negative.
///
/// It is deliberately blind to what the player spends on anything the chain
/// does not know about, and to everything a run *earns* by playing, which is
/// why the shipped payouts carry headroom rather than exactly clearing.
#[test]
fn the_tutorial_chain_can_always_afford_its_next_step() {
    let assets = test_assets_dir();
    let (contracts, _) = crate::contracts::ContractDb::load_dir(&assets.join("contracts")).unwrap();
    let (structures, _) = crate::structures::StructureDb::load_dir(&assets.join("structures"))
        .expect("the shipped structures load");
    let (abilities, _) = crate::abilities::AbilityDb::load_dir(&assets.join("abilities")).unwrap();
    let (items, _) = crate::items_db::ItemDb::load_dir(&assets.join("items"), &abilities).unwrap();
    let fragment = ItemId::from(crate::items::ids::CORE_FRAGMENT);

    let chain = contracts.tutorial_chain();
    assert!(!chain.is_empty(), "the census must walk a real chain");

    // What `Game::new` puts in the pack — see `game/lifecycle.rs`.
    let mut balance: i64 = 5;
    for def in chain {
        match &def.objective {
            crate::contracts::Objective::Build { structure } => {
                let cost = structures
                    .get(structure)
                    .unwrap_or_else(|| panic!("{} names a structure that does not exist", def.id))
                    .build_cost
                    .iter()
                    .find(|(item, _)| *item == fragment)
                    .map(|(_, n)| *n as i64)
                    .unwrap_or(0);
                balance -= cost;
            }
            // A `Hold` on fragments is satisfied by *play* — fighting and
            // mining — not by the chain's own payouts, so there is no balance
            // claim to make here beyond the one below.
            crate::contracts::Objective::Hold { item, .. } if *item == fragment => {}
            // Anything else held is compiled, and paid for in fragments.
            crate::contracts::Objective::Hold { item, count } => {
                let recipe = items
                    .get(item.as_str())
                    .and_then(|d| d.craftable.as_ref())
                    .unwrap_or_else(|| panic!("{} holds {item}, which nothing compiles", def.id));
                for (input, n) in &recipe.cost {
                    assert!(
                        *input == fragment && recipe.requires_structure.is_none(),
                        "{} holds {item}, which the balance cannot price",
                        def.id
                    );
                    balance -= (*n * *count) as i64;
                }
            }
            _ => {}
        }
        assert!(
            balance >= 0,
            "the chain runs dry at {}: {balance} Core Fragments",
            def.id
        );
        for reward in &def.reward {
            if let crate::contracts::Reward::Item(item, n) = reward
                && *item == fragment
            {
                balance += *n as i64;
            }
        }
    }
}

/// **Every `Deed` has an emit site.** Exhaustive over the enum, `cell_mark`'s
/// rule: a variant with no writer ships a mission that can never complete,
/// and this fails the build instead.
///
/// A source census rather than a runtime check, because there is no runtime
/// at which "has anything ever written this" is answerable.
#[test]
fn every_deed_has_an_emit_site() {
    use crate::contracts::Deed;

    fn rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                rust_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut paths = Vec::new();
    rust_files(&src, &mut paths);
    let mut body = String::new();
    for path in &paths {
        // The tests are not emit sites; a deed only written by a fixture is
        // a deed the game never records.
        if path.components().any(|c| c.as_os_str() == "tests") {
            continue;
        }
        // Comment lines stripped: `Deed`'s own doc names each emit site, and
        // a doc comment quoting the call form would answer this census for a
        // deed that has no writer at all.
        for line in std::fs::read_to_string(path).unwrap().lines() {
            if !line.trim_start().starts_with("//") {
                body.push_str(line);
                body.push('\n');
            }
        }
    }
    assert!(
        body.len() > 10_000,
        "the census must actually read the engine's source, or it passes vacuously"
    );

    let every = [
        Deed::Examined,
        Deed::Tamed,
        Deed::TookFromContainer,
        Deed::QueuedStandingOrder,
        Deed::UnlockedPerk,
        Deed::PostedStaff,
        Deed::ClearedNest,
        Deed::RepelledRaid,
        Deed::ReturnedSortie,
        Deed::TradedWithTown,
        Deed::ExtractedProgram,
        Deed::CollapsedStack,
        Deed::FinishedResearch,
    ];
    // `cell_mark`'s rule, applied to the census itself: the list above is
    // hand-written, so a new variant would otherwise be checked by nothing
    // and ship with no emit site against a green suite. This match is what
    // fails to compile instead, and the arm it forces is the reminder.
    for deed in every {
        match deed {
            Deed::Examined
            | Deed::Tamed
            | Deed::TookFromContainer
            | Deed::QueuedStandingOrder
            | Deed::UnlockedPerk
            | Deed::PostedStaff
            | Deed::ClearedNest
            | Deed::RepelledRaid
            | Deed::ReturnedSortie
            | Deed::TradedWithTown
            | Deed::ExtractedProgram
            | Deed::CollapsedStack
            | Deed::FinishedResearch => {}
        }
    }

    for deed in every {
        let call = format!("note_deed(crate::contracts::Deed::{deed:?})");
        let short = format!("note_deed(Deed::{deed:?})");
        assert!(
            body.contains(&call) || body.contains(&short),
            "{deed:?} has no `Game::note_deed` caller outside the tests. A deed \
             with no emit site is a mission that can never complete."
        );
    }
}

/// The shipped chain's own well-formedness: unique steps, ids that resolve,
/// and nothing carrying a flag its own load refuses.
#[test]
fn the_shipped_tutorial_chain_is_well_formed() {
    let assets = test_assets_dir();
    let (contracts, warnings) =
        crate::contracts::ContractDb::load_dir(&assets.join("contracts")).unwrap();
    assert!(warnings.is_empty(), "the shipped set parses: {warnings:?}");
    let chain = contracts.tutorial_chain();
    // A floor, not a count — the chain is content and grows. It went from
    // eleven to ten when `tutorial_spend_it` was retired: perks are bought
    // on their own creation step now, before the run starts, so a mission
    // telling the player to go and spend a point was teaching a screen
    // they had already used.
    assert!(
        chain.len() >= 10,
        "the shipped chain is authored, not empty: {}",
        chain.len()
    );

    let (structures, _) =
        crate::structures::StructureDb::load_dir(&assets.join("structures")).unwrap();
    let (abilities, _) = crate::abilities::AbilityDb::load_dir(&assets.join("abilities")).unwrap();
    let (items, _) = crate::items_db::ItemDb::load_dir(&assets.join("items"), &abilities).unwrap();

    let mut steps: Vec<u32> = Vec::new();
    for def in &chain {
        let step = def.tutorial.expect("in the chain");
        assert!(!steps.contains(&step), "{} repeats step {step}", def.id);
        steps.push(step);
        assert!(!def.starter, "{} is a mission, not a starter", def.id);
        assert!(!def.repeatable, "{} must not be repeatable", def.id);
        assert!(
            !def.description.is_empty(),
            "{} needs a description — it is the only place the player is told what to do",
            def.id
        );
        match &def.objective {
            crate::contracts::Objective::Build { structure } => assert!(
                structures.get(structure).is_some(),
                "{} names a structure that does not exist: {structure}",
                def.id
            ),
            crate::contracts::Objective::Hold { item, count } => {
                assert!(
                    items.get(item.as_str()).is_some(),
                    "{} asks for an item that does not exist: {item}",
                    def.id
                );
                assert!(*count > 0, "{} holds nothing", def.id);
            }
            _ => {}
        }
    }
}

/// The class censuses, over the **real** `assets/classes/` rather than a
/// fixture — see `crates/engine/src/classes.rs`'s own doc comment for the
/// contract these hold to.
mod classes {
    use super::*;
    use crate::classes::{ClassDb, PlayerClass};

    fn shipped() -> ClassDb {
        let (db, warnings) = ClassDb::load_dir(&test_assets_dir().join("classes")).unwrap();
        assert!(
            warnings.is_empty(),
            "shipped classes must load clean: {warnings:?}"
        );
        db
    }

    /// Driven by the enum rather than by naming the files, so a ninth
    /// class fails this test the day it is added rather than shipping
    /// unpickable — `cell_mark`'s rule.
    #[test]
    fn every_class_has_a_file() {
        let db = shipped();
        for class in PlayerClass::ALL {
            assert!(db.get(class).is_some(), "{class:?} has no class file");
        }
    }

    /// A kit naming an item that does not exist must fail the build, not
    /// silently hand the player an empty pack.
    #[test]
    fn every_class_kit_item_exists() {
        let db = shipped();
        let (abilities, _) =
            crate::abilities::AbilityDb::load_dir(&test_assets_dir().join("abilities")).unwrap();
        let (items, _) =
            crate::items_db::ItemDb::load_dir(&test_assets_dir().join("items"), &abilities)
                .unwrap();
        for def in db.iter() {
            for (item, _) in &def.kit {
                assert!(
                    items.get(item.as_str()).is_some(),
                    "{:?}'s kit names an item that does not exist: {item}",
                    def.class
                );
            }
        }
    }
}

/// **`tuning::CREATION_SHELF_ROWS` is a mod safety net, not the shipped
/// bound.** Reaching it means the shipped item set is being silently
/// truncated — rows the wizard would draw, dropped with no failure — so
/// the census is that the shipped shelf sits **under** the cap with room,
/// and the fix when it stops doing so is `CREATION_SHELF_MAX_VALUE`, never
/// this cap.
///
/// The other half of the promise — that those rows actually fit a 1280x720
/// popup with no scroll — is `the_tallest_creation_step_fits_its_screen`
/// in `crates/gui`, which is the only side that can measure a screen.
#[test]
fn the_shipped_shelf_fits_the_wizard() {
    let game = Game::new(3411, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let rows = game.creation_shelf_rows();
    assert!(
        !rows.is_empty(),
        "the shipped item set stocks no creation shelf at all"
    );
    assert!(
        rows.len() < crate::tuning::CREATION_SHELF_ROWS,
        "the shipped shelf is {} rows against a {} truncation cap — items are being \
         dropped in silence; lower CREATION_SHELF_MAX_VALUE rather than raising the cap",
        rows.len(),
        crate::tuning::CREATION_SHELF_ROWS
    );
}

/// **`CREATION_CREDITS` is sized against the shipped class kits, and this
/// is what says so.** The kit step *replaces* the class kit, so the
/// allowance has to buy something comparable: far above the band the kits
/// occupy and picking is a free upgrade over every class, far below and it
/// is a punishment for engaging with the screen. Nothing in
/// `assets/classes/` states its own worth, so this census is the whole of
/// the rule — `ZONE_MATERIALS`' precedent, one directory over.
///
/// The claim is that the allowance sits **inside the span the shipped kits
/// already occupy**, not that every kit is near it: the five are spread
/// 11..35 by design, and equalising them is the Class step's business, not
/// this one's. Priced through `ItemDb::value_of`, the same door the shelf
/// prices a row through, so a retuned item value moves both sides together.
#[test]
fn the_creation_allowance_sits_inside_the_class_kit_band() {
    let (abilities, _) =
        crate::abilities::AbilityDb::load_dir(&test_assets_dir().join("abilities")).unwrap();
    let (items, _) =
        crate::items_db::ItemDb::load_dir(&test_assets_dir().join("items"), &abilities).unwrap();
    let (classes, _) =
        crate::classes::ClassDb::load_dir(&test_assets_dir().join("classes")).unwrap();

    let worths: Vec<(String, u32)> = classes
        .iter()
        .map(|def| {
            let worth = def
                .kit
                .iter()
                .map(|(item, qty)| {
                    let price = items
                        .get(item.as_str())
                        .map(crate::items_db::ItemDb::value_of)
                        .unwrap_or_else(|| {
                            panic!("{} kits an item that does not exist: {item}", def.name)
                        });
                    price * qty
                })
                .sum();
            (def.name.clone(), worth)
        })
        .collect();
    assert!(!worths.is_empty(), "the census walked no classes at all");

    let allowance = crate::tuning::CREATION_CREDITS;
    let cheapest = worths.iter().min_by_key(|(_, w)| *w).unwrap();
    let dearest = worths.iter().max_by_key(|(_, w)| *w).unwrap();
    assert!(
        (cheapest.1..=dearest.1).contains(&allowance),
        "CREATION_CREDITS is {allowance}, outside the {}..={} the shipped class kits span \
         ({} to {}) — the kit step replaces one of these, so an allowance outside the band \
         is strictly better or strictly worse than picking a class",
        cheapest.1,
        dearest.1,
        cheapest.0,
        dearest.0
    );
}
