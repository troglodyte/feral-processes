//! The `feral-processes` binary. Resolves the game's on-disk paths and hands
//! off to the graphical frontend (`feral-processes-gui`) — this crate itself
//! draws nothing and knows nothing about game rules.
//!
//! ```sh
//! cargo run                          # the game
//! cargo run -- --template extraction # ...starting from a known world
//! ```

use std::io;

use feral_processes::dev_template;
use feral_processes_app_core::App;

const USAGE: &str = "\
usage:
  feral-processes                   play
  feral-processes --template <name> regenerate a dev-saves/ world and play it";

fn main() -> io::Result<()> {
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

    let repo_root = dev_template::repo_root();
    let assets_dir = repo_root.join("assets");
    let saves_dir = repo_root.join("saves");
    std::fs::create_dir_all(&saves_dir)?;
    // One-time migration: earlier builds kept a single save at
    // `save.bin`. Move it into the new saves directory (under its old
    // name) so it still shows up in the load list instead of silently
    // disappearing — even if it turns out to be from an incompatible
    // save version, it's still visible there and deletable.
    let legacy_save = repo_root.join("save.bin");
    if legacy_save.exists() {
        let _ = std::fs::rename(&legacy_save, saves_dir.join("save.bin"));
    }
    let history_path = repo_root.join("run_history.log");

    if !graphics_available() {
        eprintln!("No display detected; feral-processes needs a graphical display.");
        std::process::exit(1);
    }
    let mut app = App::new(assets_dir, saves_dir, history_path);
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
    Ok(())
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
