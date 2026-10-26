//! The Workbench's weapon-crafting pages: Forge, New design, Weapon parts
//! and Salvage, and the Weapons research tab.

use super::support::*;
use crate::*;
use feral_processes_engine::gear_craft::weapon::{Category, Frame, WeaponPart};
use feral_processes_engine::gear_craft::{DesignHolder, FitError, GearCraftRefusal, GearPart};
use feral_processes_engine::items::ItemId;
use feral_processes_engine::routine_craft::{StatusKind, Version};

fn edge() -> WeaponPart {
    WeaponPart::Frame(Frame::Edge, Version::V1)
}

fn bleed() -> WeaponPart {
    WeaponPart::Rider(StatusKind::Bleed)
}

fn forge_app(seed: u32) -> App {
    let mut app = app_at_a_workbench(seed);
    let game = app.game.as_mut().unwrap();
    for id in [
        "weapon_bench".to_string(),
        edge().node_id(),
        bleed().node_id(),
    ] {
        let _ = game.dev_unlock_research(&id);
    }
    assert!(game.weapon_tree_open());
    app
}

fn open_page(app: &mut App, target: Mode) {
    app.handle_key(GameKey::Char('c'));
    for _ in 0..16 {
        if app.mode == target {
            break;
        }
        app.handle_key(GameKey::Down);
    }
    assert_eq!(app.mode, target, "the rail reaches {target:?}");
    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, target);
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        app.handle_key(GameKey::Char(c));
    }
}

fn row_of(category: Category) -> usize {
    if category == Category::Frame {
        return 0;
    }
    1 + Category::MODIFIERS
        .iter()
        .position(|c| *c == category)
        .unwrap()
}

/// Opens the part list for the row and picks `part` from it.
fn choose(app: &mut App, row: usize, part: WeaponPart) {
    pick_row(app, row);
    assert_eq!(app.mode, Mode::DesignPick);
    let game = app.game.as_ref().unwrap();
    let options = game.design_part_options(
        part.category(),
        &app.design_draft.parts,
        app.design_draft.affixes.len(),
    );
    let at = options.iter().position(|o| o.part == part).unwrap();
    pick_row(app, at);
    assert_eq!(app.mode, Mode::NewDesign);
}

#[test]
fn a_workbench_hub_has_the_forge_pages_only_once_weapon_bench_is_researched() {
    let mut app = app_at_a_workbench(81_001);
    app.handle_key(GameKey::Char('c'));
    let hub = app.structure_hub.as_ref().unwrap();
    assert!(!hub.current().pages.contains(&StructurePage::Forge));
    let mut app = forge_app(81_002);
    app.handle_key(GameKey::Char('c'));
    let pages = &app.structure_hub.as_ref().unwrap().current().pages;
    for page in [
        StructurePage::Forge,
        StructurePage::NewDesign,
        StructurePage::WeaponParts,
        StructurePage::Salvage,
    ] {
        assert!(pages.contains(&page), "{page:?} in {pages:?}");
    }
}

#[test]
fn designing_a_weapon_saves_it_and_the_forge_lists_it() {
    let mut app = forge_app(81_003);
    open_page(&mut app, Mode::NewDesign);
    choose(&mut app, row_of(Category::Frame), edge());
    choose(&mut app, row_of(Category::Rider), bleed());
    assert_eq!(app.design_draft.parts, vec![edge(), bleed()]);

    pick_row(&mut app, DESIGN_NAME_ROW);
    assert_eq!(app.mode, Mode::DesignName);
    type_text(&mut app, "Leaker");
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::NewDesign);

    pick_row(&mut app, DESIGN_SAVE_ROW);
    assert!(app.design_draft.parts.is_empty(), "the draft resets");
    let designs = app.game.as_ref().unwrap().design_rows();
    assert_eq!(designs.len(), 1);
    assert_eq!(designs[0].name, "Leaker");
}

#[test]
fn saving_an_unfinished_design_is_refused_in_words_and_keeps_the_draft() {
    let mut app = forge_app(81_004);
    open_page(&mut app, Mode::NewDesign);
    app.design_draft.parts = vec![edge()];
    pick_row(&mut app, DESIGN_SAVE_ROW);
    assert_eq!(app.status_line.as_deref(), Some("Give the design a name."));
    assert_eq!(app.design_draft.parts, vec![edge()]);
}

#[test]
fn a_greyed_part_refuses_with_its_reason_and_the_list_stays_open() {
    let mut app = forge_app(81_005);
    open_page(&mut app, Mode::NewDesign);
    app.design_draft.parts = vec![edge()];
    pick_row(&mut app, row_of(Category::Proc));
    assert_eq!(app.mode, Mode::DesignPick);
    pick_row(&mut app, 0);
    assert_eq!(app.mode, Mode::DesignPick);
    assert!(app.status_line.as_deref().unwrap().contains("research"));
}

#[test]
fn the_part_picker_filters_by_typing_and_esc_clears_before_leaving() {
    let mut app = forge_app(81_006);
    open_page(&mut app, Mode::NewDesign);
    pick_row(&mut app, row_of(Category::Frame));
    let all = design_pick_visible(
        app.game.as_ref().unwrap(),
        &app.design_draft,
        app.filter_text(),
    )
    .len();
    type_text(&mut app, "edge");
    let visible = design_pick_visible(
        app.game.as_ref().unwrap(),
        &app.design_draft,
        app.filter_text(),
    );
    assert!(!visible.is_empty() && visible.len() < all, "{visible:?}");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::DesignPick, "first Esc clears the filter");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::NewDesign);
}

#[test]
fn changing_the_frame_drops_a_modifier_the_new_frame_refuses() {
    let mut app = forge_app(81_007);
    let whip = WeaponPart::Frame(Frame::Whip, Version::V1);
    let _ = app
        .game
        .as_mut()
        .unwrap()
        .dev_unlock_research(&whip.node_id());
    app.design_draft.parts = vec![edge(), WeaponPart::Extend];
    let _ = app
        .game
        .as_mut()
        .unwrap()
        .dev_unlock_research(&WeaponPart::Extend.node_id());
    // Whip already swings at the longest range, so Extend cannot stay.
    open_page(&mut app, Mode::NewDesign);
    choose(&mut app, row_of(Category::Frame), whip);
    assert_eq!(app.design_draft.parts, vec![whip]);
}

#[test]
fn the_parts_page_crafts_a_known_part_and_refuses_an_unresearched_one() {
    let mut app = forge_app(81_008);
    open_page(&mut app, Mode::WeaponParts);
    let rows = app.game.as_ref().unwrap().weapon_part_rows();
    let locked = rows.iter().position(|r| !r.known).unwrap();
    pick_row(&mut app, locked);
    let line = app.status_line.clone().unwrap();
    assert!(line.ends_with("isn't researched yet."), "{line}");

    let at = rows.iter().position(|r| r.part == edge()).unwrap();
    assert!(rows[at].affordable, "{:?}", rows[at]);
    pick_row(&mut app, at);
    assert!(app.status_line.is_none(), "{:?}", app.status_line);
    assert_eq!(app.game.as_ref().unwrap().weapon_parts_held(edge()), 1);
}

#[test]
fn the_forge_builds_a_design_and_refuses_deleting_one_with_a_copy_out() {
    let mut app = forge_app(81_009);
    let game = app.game.as_mut().unwrap();
    let id = game
        .create_design("Plain", vec![GearPart::Weapon(edge())])
        .unwrap();
    open_page(&mut app, Mode::Forge);
    pick_row(&mut app, 0);
    let line = app.status_line.clone().unwrap();
    assert!(line.starts_with("Missing parts"), "{line}");

    let game = app.game.as_mut().unwrap();
    game.dev_give_item(&ItemId::weapon_part(edge()), 1).unwrap();
    let (frame, version) = game.gear_designs().remove(0).frame().unwrap();
    let (item, count) = feral_processes_engine::gear_craft::weapon::frame_bill(frame, version);
    game.dev_give_item(&item, count).unwrap();
    pick_row(&mut app, 0);
    assert_eq!(app.status_line, None);
    assert_eq!(
        app.game.as_ref().unwrap().design_holder(id),
        Some(DesignHolder::Pack)
    );

    highlight_row(&mut app, 0);
    app.handle_key(GameKey::Char('D'));
    let line = app.status_line.clone().unwrap();
    assert!(line.contains("pack"), "{line}");
    assert_eq!(app.game.as_ref().unwrap().design_rows().len(), 1);
}

#[test]
fn the_forge_deletes_a_design_nothing_holds() {
    let mut app = forge_app(81_010);
    app.game
        .as_mut()
        .unwrap()
        .create_design("Plain", vec![GearPart::Weapon(edge())])
        .unwrap();
    open_page(&mut app, Mode::Forge);
    app.handle_key(GameKey::Char('D'));
    assert!(app.game.as_ref().unwrap().design_rows().is_empty());
}

#[test]
fn every_refusal_has_a_plain_sentence() {
    let app = forge_app(81_011);
    let game = app.game.as_ref().unwrap();
    let refusals = vec![
        GearCraftRefusal::Busy,
        GearCraftRefusal::NoWorkbench,
        GearCraftRefusal::PartNotResearched(edge()),
        GearCraftRefusal::ShortOf {
            item: ItemId::from("core_fragment"),
            need: 3,
            held: 1,
        },
        GearCraftRefusal::MissingParts(vec![(edge(), 2)]),
        GearCraftRefusal::NameEmpty,
        GearCraftRefusal::NameTooLong { max: 24 },
        GearCraftRefusal::Fit(FitError::NoFrame),
        GearCraftRefusal::Fit(FitError::TwoFrames),
        GearCraftRefusal::Fit(FitError::CategoryTaken(Category::Rider)),
        GearCraftRefusal::Fit(FitError::OverCap { cap: 2 }),
        GearCraftRefusal::Fit(FitError::RangeAtMax(Frame::Whip)),
        GearCraftRefusal::Fit(FitError::StunRider),
        GearCraftRefusal::AffixNotKnown("honed".into()),
        GearCraftRefusal::NoSuchDesign(3),
        GearCraftRefusal::InUse(DesignHolder::Worn),
        GearCraftRefusal::WeaponTreeClosed,
        GearCraftRefusal::NoCopyHeld,
        GearCraftRefusal::NotSalvageable,
        GearCraftRefusal::SalvageOnly(edge()),
    ];
    for r in refusals {
        let line = gear_craft_refusal_line(game, &r);
        assert!(line.ends_with('.'), "{r:?} -> {line}");
        assert!(!line.to_lowercase().contains("tick"), "{line}");
    }
}

#[test]
fn the_salvage_page_takes_a_pack_weapon_apart() {
    let mut app = forge_app(81_012);
    open_page(&mut app, Mode::WeaponSalvage);
    assert!(app.game.as_ref().unwrap().salvage_rows().is_empty());
    put_in_pack(&mut app, "ragged_edge", 1);
    assert_eq!(app.game.as_ref().unwrap().salvage_rows().len(), 1);
    pick_row(&mut app, 0);
    assert_eq!(app.status_line, None);
    assert!(app.game.as_ref().unwrap().salvage_rows().is_empty());
}

#[test]
fn the_weapons_research_tab_opens_from_the_research_page() {
    let mut app = forge_app(81_013);
    open_command(&mut app, "Weapon research");
    assert_eq!(app.mode, Mode::WeaponResearch);
    app.handle_key(GameKey::Esc);
    assert_ne!(app.mode, Mode::WeaponResearch);
}
