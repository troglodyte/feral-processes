use super::*;

pub(super) fn assets() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

pub(super) fn fighting() -> Game {
    for seed in 0..200u32 {
        let mut game =
            Game::new(seed, DifficultyMode::Forgiving, &assets()).expect("the assets parse");
        let mut profile = game.profile().clone();
        profile.tactical_battles = true;
        game.install_profile(profile);
        let at = game.player_status().position;
        let target = game
            .view_entities(12, 12)
            .into_iter()
            .filter(|e| e.is_hostile && !e.is_tamed && !e.is_structure)
            .find(|e| (e.pos.0 - at.0).abs() + (e.pos.1 - at.1).abs() == 1);
        let Some(target) = target else { continue };
        game.move_player(target.pos.0 - at.0, target.pos.1 - at.1);
        if game.in_tactical_battle() {
            return game;
        }
    }
    panic!("no seed under 200 put a lone wild program next to the player");
}

pub(super) fn pane() -> Rect {
    Rect::new(0.0, 0.0, 800.0, 600.0)
}

/// The downward-pointing, horizontally symmetric triangles among a set
/// of polygons, as `(apex, width, height)`.
///
/// The discriminator is the *apex being centred between the other two
/// points*, which is what tells this arrow from the con earmark — the
/// only other polygon on this grid, a right-angled wedge folded into a
/// corner whose third point sits directly under one of its neighbours.
/// It has to be told apart by shape and not by colour: `palette::glyph`
/// resolves `GlyphColor::Red` to `THREAT` and `Blue` to `PLAN`, so an
/// earmark can be painted in either of the two colours this arrow uses.
pub(super) fn arrows(polys: &[Vec<(f32, f32)>]) -> Vec<((f32, f32), f32, f32)> {
    polys
        .iter()
        .filter(|p| p.len() == 3)
        .filter_map(|p| {
            let (a, b, c) = (p[0], p[1], p[2]);
            let centred = ((a.0 + b.0) / 2.0 - c.0).abs() < 0.01;
            (a.1 == b.1 && c.1 > a.1 && centred).then(|| (c, (b.0 - a.0).abs(), c.1 - a.1))
        })
        .collect()
}
