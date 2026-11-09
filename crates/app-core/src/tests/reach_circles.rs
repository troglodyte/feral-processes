//! The Shield and Patch Node circles `App::reach_circles` hands the renderer.

use super::support::*;
use crate::*;

/// A base at zone 2 with a Mk1 Shield one tile east of the party.
fn app_beside_a_shield(seed: u32) -> App {
    let mut app = app_in_base_with_programs(seed, 2);
    let game = app.game.as_mut().unwrap();
    game.dev_unlock_research(&"fortification".into())
        .expect("Fortification unlocks the Shield");
    game.dev_set_free_build(true);
    game.place_structure("shield", 1, 0, None)
        .expect("a Shield deploys beside the party");
    game.dev_set_free_build(false);
    game.warp_to_zone(2).expect("the breach opens zone 2");
    drain_notifications(&mut app);
    app
}

fn shield_anchor(app: &mut App) -> (i32, i32) {
    let shields = app.game.as_mut().unwrap().view_shields();
    assert_eq!(shields.len(), 1, "one Shield stands");
    shields[0].anchor
}

#[test]
fn hovering_a_shield_returns_its_circle() {
    let mut app = app_beside_a_shield(6101);
    let anchor = shield_anchor(&mut app);

    app.set_map_hover(Some(anchor));
    assert_eq!(
        app.reach_circles(),
        vec![ReachCircle {
            anchor,
            radius: 4,
            style: ReachCircleStyle::Hover,
            kind: ReachKind::Shield
        }]
    );

    app.set_map_hover(Some((anchor.0 + 1, anchor.1)));
    assert!(app.reach_circles().is_empty());
}

#[test]
fn deploying_a_shield_previews_four_circles() {
    let mut app = app_beside_a_shield(6102);
    let (px, py) = app.game.as_ref().unwrap().field_pos().unwrap();
    app.mode = Mode::BuildDirection;
    app.pending_structure = Some("shield".into());

    let mut anchors: Vec<_> = app
        .reach_circles()
        .into_iter()
        .inspect(|c| {
            assert_eq!((c.radius, c.style), (4, ReachCircleStyle::Preview));
        })
        .map(|c| c.anchor)
        .collect();
    anchors.sort();
    let mut want = vec![(px, py - 1), (px, py + 1), (px - 1, py), (px + 1, py)];
    want.sort();
    assert_eq!(anchors, want);

    app.pending_structure = Some("depot".into());
    assert!(app.reach_circles().is_empty(), "a depot has no circle");
}

#[test]
fn upgrading_previews_the_next_radius() {
    let mut app = app_beside_a_shield(6103);
    let anchor = shield_anchor(&mut app);
    app.mode = Mode::UpgradeDirection;

    assert_eq!(
        app.reach_circles(),
        vec![ReachCircle {
            anchor,
            radius: 6,
            style: ReachCircleStyle::Preview,
            kind: ReachKind::Shield
        }]
    );
}

#[test]
fn a_shield_upgrade_skips_the_program_picker() {
    let mut app = app_beside_a_shield(6104);
    app.mode = Mode::UpgradeDirection;

    app.handle_key(GameKey::Right);

    assert_ne!(app.mode, Mode::BuildProgram);
    assert!(
        app.game
            .as_mut()
            .unwrap()
            .adjacent_build_site(1, 0)
            .is_some(),
        "the upgrade is filed by the direction press alone"
    );
}

#[test]
fn board_hover_circle_is_in_board_cells() {
    let mut app = app_beside_a_shield(6105);
    let anchor = shield_anchor(&mut app);
    let game = app.game.as_mut().unwrap();
    game.dev_set_sieges(true);
    game.dev_force_siege();
    let origin = game.tactical_view().expect("the siege opened").siege_origin;
    app.mode = Mode::TacticalBattle;
    let cell = (anchor.0 - origin.0, anchor.1 - origin.1);

    app.set_map_hover(Some(cell));
    let circles = app.reach_circles();

    assert_eq!(circles.len(), 1);
    assert_eq!(circles[0].anchor, cell);
    assert_eq!(circles[0].radius, 4);
}

/// A base at zone 2 with a Patch Node one tile east of the party.
fn app_beside_a_patch_node(seed: u32) -> App {
    let mut app = app_in_base_with_programs(seed, 2);
    let game = app.game.as_mut().unwrap();
    game.dev_unlock_research(&"fortification".into())
        .expect("Fortification unlocks the Patch Node");
    game.dev_set_free_build(true);
    game.place_structure("patch_node", 1, 0, None)
        .expect("a Patch Node deploys beside the party");
    game.dev_set_free_build(false);
    game.warp_to_zone(2).expect("the breach opens zone 2");
    drain_notifications(&mut app);
    app
}

#[test]
fn hovering_a_patch_node_shows_a_patch_circle() {
    let mut app = app_beside_a_patch_node(6111);
    let anchor = app.game.as_mut().unwrap().view_patch_nodes()[0].anchor;

    app.set_map_hover(Some(anchor));

    assert_eq!(
        app.reach_circles(),
        vec![ReachCircle {
            anchor,
            radius: 4,
            style: ReachCircleStyle::Hover,
            kind: ReachKind::Patch
        }]
    );
}

#[test]
fn deploy_preview_of_a_patch_node_shows_radius_4() {
    let mut app = app_beside_a_patch_node(6112);
    app.mode = Mode::BuildDirection;
    app.pending_structure = Some("patch_node".into());

    let circles = app.reach_circles();

    assert_eq!(circles.len(), 4);
    assert!(circles.iter().all(|c| {
        (c.radius, c.style, c.kind) == (4, ReachCircleStyle::Preview, ReachKind::Patch)
    }));
}

#[test]
fn upgrade_preview_of_a_tier_1_patch_node_shows_radius_6() {
    let mut app = app_beside_a_patch_node(6113);
    let anchor = app.game.as_mut().unwrap().view_patch_nodes()[0].anchor;
    app.mode = Mode::UpgradeDirection;

    assert_eq!(
        app.reach_circles(),
        vec![ReachCircle {
            anchor,
            radius: 6,
            style: ReachCircleStyle::Preview,
            kind: ReachKind::Patch
        }]
    );
}

#[test]
fn a_shield_circle_is_still_kind_shield() {
    let mut app = app_beside_a_shield(6114);
    let anchor = shield_anchor(&mut app);
    app.set_map_hover(Some(anchor));

    assert_eq!(app.reach_circles()[0].kind, ReachKind::Shield);
}
