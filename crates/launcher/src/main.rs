// A release build on Windows opens without a console window behind the game.
// Debug keeps stderr, which is what a developer on Windows would want — the
// cost is that the `eprintln!`-then-exit paths go silent in release, which
// is why the two a player can actually reach also write `startup-error.txt`.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

//! The `feral-processes` binary. Resolves the game's on-disk paths and hands
//! off to the graphical frontend (`feral-processes-gui`) — this crate itself
//! draws nothing and knows nothing about game rules.
//!
//! ```sh
//! cargo run                          # the game
//! cargo run -- --template extraction # ...starting from a known world
//! ```

use feral_processes::{dev_template, paths};
use feral_processes_app_core::{App, DevTemplates};

const USAGE: &str = "\
usage:
  feral-processes                   play
  feral-processes --template <name> regenerate a dev-saves/ world and play it";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let template = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        [] => None,
        ["--template", name] => Some(name.to_string()),
        // A bare `--template` is a likely typo rather than a request to
        // list, so it answers with the names it would have accepted.
        ["--template"] => {
            eprintln!("--template needs a name\n{}", dev_template::known());
            std::process::exit(1);
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(1);
        }
    };

    let paths = paths::resolve();
    if !paths.assets.is_dir() {
        fatal(&format!(
            "no assets directory at {}\n\
             The game needs its `assets` folder beside the executable, \
             or FERAL_ASSETS_DIR pointing at one.",
            paths.assets.display()
        ));
    }
    let saves_dir = paths.data.join("saves");
    if let Err(e) = std::fs::create_dir_all(&saves_dir) {
        fatal(&format!("cannot create {}: {e}", saves_dir.display()));
    }
    paths::migrate_from_repo(&paths::repo_root(), &paths.data);
    let history_path = paths.data.join("run_history.log");
    // Beside the history log, not in `saves/`: both span runs rather than
    // belonging to one, and `App::list_saves` filters on `.bin` so neither
    // can turn up in the save picker.
    let profile_path = paths.data.join("profile.ron");
    // An installed build has no repo to find dev material in, so these two
    // point at directories that will never exist. That is not a special
    // case: the arena and template rows are gated behind `FERAL_DEV_ARENA`,
    // and both `dev_template::list` and the arena catalog already read a
    // missing directory as nothing to offer.
    let (arena_dir, battle_log) = match &paths.dev {
        Some(dev) => (dev.arenas.clone(), dev.battle_log.clone()),
        None => (
            paths.data.join("dev-arenas"),
            paths.data.join("dev-logs").join("battles.jsonl"),
        ),
    };

    if !graphics_available() {
        eprintln!("No display detected; feral-processes needs a graphical display.");
        std::process::exit(1);
    }
    let mut app = App::new(
        paths.assets,
        saves_dir,
        history_path,
        profile_path,
        arena_dir,
        // Beside `dev-saves/`, `dev-arenas/` and `dev-training/`, and
        // gitignored: written only when `FERAL_DEV_LOG` is set, and never
        // reachable in a player's build. See `dev-logs/README.md`.
        battle_log,
    );
    // Unconditionally, not behind `FERAL_DEV_ARENA`: the gate decides
    // whether the arena is *visible*, and a launcher that installed only
    // when gated would make one flag mean two things.
    app.install_dev_templates(DevTemplates {
        names: dev_template::list(),
        resolve: dev_template::resolve,
    });
    // Only inside a checkout — `paths.dev` is `None` in an installed build,
    // which is the other half of `App::sprite_forge_enabled`'s gate:
    // `FERAL_DEV_SPRITES` alone must not be enough to offer a screen whose
    // whole purpose is writing into a source tree that build does not have.
    if paths.dev.is_some() {
        app.install_sprite_dir();
    }
    // Generated into an expendable copy under `saves/`, never opened on the
    // `dev-saves/` source — the game autosaves, so playing the fixture
    // directly would rewrite it into a record of this session.
    if let Some(name) = template {
        let working_copy = dev_template::working_copy(&name);
        if let Err(e) = dev_template::generate(&name, &working_copy) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        eprintln!("playing template `{name}` at {}", working_copy.display());
        app.load_game(working_copy);
    }
    feral_processes_gui::run(app);
}

/// The two startup failures a player can actually reach, said in both
/// places at once: stderr for a developer watching a terminal, and
/// `startup-error.txt` beside the executable for a player whose release
/// build has no console. Both unconditionally — branching on which is which
/// is a `cfg` nobody would maintain, and a message box is a dependency
/// bought for two error strings.
fn fatal(message: &str) -> ! {
    eprintln!("{message}");
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let _ = std::fs::write(dir.join("startup-error.txt"), format!("{message}\n"));
    }
    std::process::exit(1);
}

/// Best-effort preflight check: on Linux there's no windowing system to
/// open a window on at all without an X11/Wayland display, and the winit
/// layer under Bevy panics out of `App::run` rather than returning an error
/// a caller could act on — so this check is what turns the common "no
/// display" case (e.g. an SSH session or a CI box) into a readable error
/// instead of a backtrace. macOS/Windows always have a compositor available
/// to the active session, so there's nothing analogous to check.
fn graphics_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        // An empty (but present) value is what a shell like `DISPLAY= cmd`
        // produces, and behaves the same as unset here — `var_os` alone
        // would treat it as "present" and skip straight to a doomed
        // XOpenDisplay() call.
        let has = |name: &str| std::env::var(name).is_ok_and(|v| !v.is_empty());
        has("DISPLAY") || has("WAYLAND_DISPLAY")
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}
