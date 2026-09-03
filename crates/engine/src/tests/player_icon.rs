//! Task 2 of the player icon editor: the drawn `PlayerIcon` persists on the
//! live `PlayerIdentity` component, through a real save/load round trip,
//! and — as the encoded string, never the struct — in the cross-run
//! `achievements::Profile`. The codec itself is unit-tested in `icon::tests`;
//! what is here is everything that touches a real `World`, a real save file
//! and a real `profile.ron`.

use super::support::*;
use crate::achievements::{AchievementId, Profile};
use crate::*;

fn save_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "feral_player_icon_{name}_{}.sav",
        std::process::id()
    ))
}

fn painted_icon() -> PlayerIcon {
    let mut icon = PlayerIcon::default();
    icon.set(0, 0, 1);
    icon.set(15, 15, 15);
    icon.set(7, 3, 9);
    icon
}

/// **Through a real save/load round trip, not a RON string round trip.**
/// `ron::from_str(&ron::to_string(x))` cannot catch a field `Game::save`
/// simply never writes — `tests::creation`'s
/// `a_created_player_round_trips_through_a_real_save` is the precedent this
/// follows, for `PlayerIdentity::icon` instead of its `sprite`/`colour`.
#[test]
fn a_drawn_icon_survives_a_real_save_and_load_round_trip() {
    let mut game = Game::new(70_201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let icon = painted_icon();
    game.world.get_mut::<PlayerIdentity>(player).unwrap().icon = Some(icon.clone());

    let path = save_path("round_trip");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let identity = loaded
        .world
        .get::<PlayerIdentity>(loaded.player_entity())
        .unwrap();
    assert_eq!(
        identity.icon,
        Some(icon),
        "the drawn icon must come back exactly as drawn"
    );
}

/// The field is additive behind `#[serde(default)]`, so it costs no
/// `SAVE_FORMAT_VERSION` bump — a save written before this feature existed
/// has no `icon` key at all, and must load with no icon rather than being
/// refused. Derived from a real save with the key stripped out, the way
/// `save.rs`'s `a_save_written_before_dig_sites_existed_still_loads` and
/// `a_save_written_before_caravans_existed_still_loads` both do, so the
/// fixture cannot drift from what `Game::save` actually writes.
#[test]
fn a_save_written_before_icons_existed_still_loads_with_no_icon() {
    let mut game = Game::new(70_202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let path = save_path("legacy");
    game.save(&path).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    let older: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("icon:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !older.contains("icon:"),
        "the key has to actually be gone for this to prove anything"
    );
    std::fs::write(&path, &older).unwrap();

    let loaded = match Game::load(&path, &test_assets_dir()) {
        Ok(loaded) => loaded,
        Err(e) => panic!("a save written before icons existed must still load: {e}"),
    };
    let _ = std::fs::remove_file(&path);
    let identity = loaded
        .world
        .get::<PlayerIdentity>(loaded.player_entity())
        .unwrap();
    assert!(
        identity.icon.is_none(),
        "an absent key must load with no icon, not a default guess"
    );
}

/// **The whole reason `Profile::player_icon` is a plain string and not the
/// struct.** Asserting the icon itself comes back `None` here would also
/// pass with the feature deleted outright — an absent field gives `None`
/// too — so the assertion that actually distinguishes "handled gracefully"
/// from "never built" is that the achievements survive at all:
/// `Profile::load` discards the *whole* profile, achievements included,
/// when it cannot parse, so a typed field that let a bad icon fail the
/// whole document would cost this earn.
#[test]
fn a_profile_with_a_garbage_icon_still_loads_its_achievements() {
    let dir = scratch_assets_dir("player_icon_garbage_profile");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("profile.ron");
    std::fs::write(
        &path,
        r#"(earned: [(id: "did_a_thing", first_tick: 1)], player_icon: Some("not-a-real-icon"))"#,
    )
    .unwrap();

    let (profile, warning) = Profile::load(&path);
    assert!(warning.is_none(), "{warning:?}");
    assert!(
        profile.contains(&AchievementId::from("did_a_thing")),
        "a garbage icon must not cost the player an achievement already earned"
    );
}
