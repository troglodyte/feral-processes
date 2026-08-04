//! The three-page program fusion flow.

use super::support::*;
use crate::*;

/// `Game::fuse_companions` asks only that you own both programs — there is
/// no distance requirement anywhere in it — so the picker must offer the
/// whole roster, exactly as the party screen does. It used to scan
/// `MENU_SCAN_RADIUS` tiles instead, which quietly hid every program parked
/// away from the player and reported "no compiled programs nearby".
#[test]
fn the_fuse_picker_offers_owned_programs_parked_far_from_the_player() {
    let mut app = app_owning_distant_programs(740, 2);
    assert_eq!(
        app.game.as_mut().unwrap().owned_pets().len(),
        2,
        "fixture should hand the player exactly two programs to fuse"
    );

    open_via_menu(&mut app, 'p', "Fuse two programs");
    assert_eq!(app.mode, Mode::Fuse, "'f' should open the fuse picker");

    app.handle_key(GameKey::Char('1'));
    assert_eq!(
        app.mode,
        Mode::FuseSecond,
        "picking a distant program should advance to the second page"
    );

    app.handle_key(GameKey::Char('1'));
    assert_eq!(
        app.mode,
        Mode::FuseName,
        "the second page should offer the other distant program"
    );

    app.handle_key(GameKey::Enter);
    assert_eq!(app.status_line, None, "the fusion should not be refused");
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(
        app.game.as_mut().unwrap().owned_pets().len(),
        1,
        "both inputs are consumed and one fused program remains"
    );
}
