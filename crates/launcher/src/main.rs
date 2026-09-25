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
//! cargo run -- --template stack --keys "Right Right M" --screenshot out.png
//! ```

use feral_processes::{dev_template, paths};
use feral_processes_app_core::{App, DevTemplates, GameKey};
use feral_processes_gui::Capture;

const USAGE: &str = "\
usage:
  feral-processes                     play
  feral-processes --template <name>   regenerate a dev-saves/ world and play it
  ... --keys \"<key> <key> ...\"       press these first: GameKey names
                                      (Right, Enter, Esc...), Space, or one
                                      character
  ... --screenshot <file.png>         write the screen at 1280x720 and exit";

/// What the command line asked for. Each flag at most once, in any order.
#[derive(Debug, Default, PartialEq)]
struct Args {
    template: Option<String>,
    keys: Vec<GameKey>,
    screenshot: Option<std::path::PathBuf>,
}

/// `Err` carries the message to print; `None` inside it means "print the
/// usage".
fn parse_args(args: &[String]) -> Result<Args, Option<String>> {
    let mut parsed = Args::default();
    let mut rest = args.iter().map(String::as_str);
    while let Some(flag) = rest.next() {
        let value = rest.next();
        match (flag, value) {
            ("--template", Some(name)) if parsed.template.is_none() => {
                parsed.template = Some(name.to_string());
            }
            // A bare `--template` is a likely typo rather than a request to
            // list, so it answers with the names it would have accepted.
            ("--template", None) => {
                return Err(Some(format!(
                    "--template needs a name\n{}",
                    dev_template::known()
                )));
            }
            ("--keys", Some(list)) if parsed.keys.is_empty() => {
                parsed.keys = GameKey::parse_list(list).map_err(Some)?;
            }
            ("--screenshot", Some(path)) if parsed.screenshot.is_none() => {
                parsed.screenshot = Some(path.into());
            }
            _ => return Err(None),
        }
    }
    Ok(parsed)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Args {
        template,
        keys,
        screenshot,
    } = match parse_args(&args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{}", message.as_deref().unwrap_or(USAGE));
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
    // Through the one door the keyboard uses, before the first frame — so a
    // scripted screen is reached exactly as a player would reach it.
    for key in keys {
        app.handle_key(key);
    }
    let capture = screenshot.map(|path| Capture { path });
    if feral_processes_gui::run(app, capture).is_error() {
        std::process::exit(1);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &[&str]) -> Result<Args, Option<String>> {
        parse_args(&line.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn no_arguments_is_play() {
        assert_eq!(parse(&[]), Ok(Args::default()));
    }

    #[test]
    fn the_three_flags_parse_in_any_order() {
        let parsed = parse(&[
            "--screenshot",
            "out.png",
            "--keys",
            "Right M",
            "--template",
            "stack",
        ])
        .unwrap();
        assert_eq!(parsed.template.as_deref(), Some("stack"));
        assert_eq!(parsed.keys, vec![GameKey::Right, GameKey::Char('M')]);
        assert_eq!(parsed.screenshot, Some("out.png".into()));
    }

    #[test]
    fn a_bad_key_names_itself() {
        let err = parse(&["--keys", "Rigth"]).unwrap_err().unwrap();
        assert!(err.contains("Rigth"), "{err}");
    }

    #[test]
    fn a_repeated_or_unknown_flag_prints_the_usage() {
        assert_eq!(parse(&["--template", "a", "--template", "b"]), Err(None));
        assert_eq!(parse(&["--fullscreen"]), Err(None));
        assert_eq!(parse(&["--screenshot"]), Err(None));
    }

    #[test]
    fn a_bare_template_lists_the_names() {
        assert!(matches!(parse(&["--template"]), Err(Some(_))));
    }
}
