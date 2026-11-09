//! The screens the hub merge made into `b` tabs (Deploy, the research
//! lists, Study) draw into the hub's content rect, which is not the popup
//! they were sized for: every row fits it at the smallest window.

use super::*;
use crate::render::base_report::assert_rows_fit;
use crate::render::building::{build_menu_page_rows, pin_subject_page_rows};
use crate::render::progression::research_page_rows;
use feral_processes_app_core::{BuildMenu, BuildRow};
use feral_processes_engine::ResearchTree;

#[test]
fn the_deploy_tab_fits_the_content_rect() {
    let mut app = census_app();
    let game = app.game.as_mut().unwrap();
    assert_rows_fit(&build_menu_page_rows(game, None, 0), "Deploy");
    let categories: Vec<_> = BuildMenu::of(game, None)
        .rows
        .into_iter()
        .filter_map(|r| match r {
            BuildRow::Category(c) => Some(c),
            BuildRow::Structure(_) => None,
        })
        .collect();
    assert!(!categories.is_empty(), "the census needs a category");
    for c in categories {
        assert_rows_fit(&build_menu_page_rows(game, Some(c), 0), "Deploy category");
    }
}

#[test]
fn the_research_tabs_fit_the_content_rect() {
    let app = census_app();
    let game = app.game.as_ref().unwrap();
    for tree in [
        ResearchTree::Base,
        ResearchTree::Routines,
        ResearchTree::Affixes,
        ResearchTree::Weapons,
    ] {
        let (title, rows) = research_page_rows(game, tree, 0, true);
        assert_rows_fit(&rows, title);
    }
}

#[test]
fn the_study_tab_fits_the_content_rect() {
    let app = census_app();
    let game = app.game.as_ref().unwrap();
    assert_rows_fit(&pin_subject_page_rows(game, 0), "Study");
}
