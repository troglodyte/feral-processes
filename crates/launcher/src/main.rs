//! The `feral-processes` binary. Resolves the game's on-disk paths, picks a
//! frontend (graphics or text), and hands off to whichever renderer crate
//! (`feral-processes-gui` or `feral-processes-tui`) runs it — this crate
//! itself draws nothing and knows nothing about game rules, it's purely the
//! backend-selection glue.

use std::io::{self, Write};
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;

use feral_processes_app_core::App;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Backend {
    Gui,
    Tui,
}

fn main() -> io::Result<()> {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_dir
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(&crate_dir)
        .to_path_buf();
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

    let backend = pick_backend();

    match backend {
        Backend::Tui => {
            let mut app = App::new(assets_dir, saves_dir, history_path);
            feral_processes_tui::run(&mut app)
        }
        Backend::Gui => {
            if !graphics_available() {
                eprintln!("No display detected; falling back to text mode.");
                let mut app = App::new(assets_dir, saves_dir, history_path);
                return feral_processes_tui::run(&mut app);
            }
            let app = App::new(assets_dir.clone(), saves_dir.clone(), history_path.clone());
            let result = panic::catch_unwind(AssertUnwindSafe(|| feral_processes_gui::run(app)));
            if result.is_err() {
                // The in-progress session is lost with the unwound stack
                // frame (see feral-processes-gui::run's docs) — autosaves
                // mean at most a few ticks of progress, recoverable from
                // the load-game menu, not a fresh save every time.
                eprintln!("Graphics frontend crashed; falling back to text mode.");
                let mut app = App::new(assets_dir, saves_dir, history_path);
                return feral_processes_tui::run(&mut app);
            }
            Ok(())
        }
    }
}

/// `--gui`/`--tui`/`--ascii` skip the prompt; otherwise ask interactively,
/// before either backend has touched a terminal or opened a window, so the
/// choice happens once, up front.
fn pick_backend() -> Backend {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--gui") {
        return Backend::Gui;
    }
    if args.iter().any(|a| a == "--tui" || a == "--ascii") {
        return Backend::Tui;
    }

    print!("feral-processes — Graphics or Text interface? [g/t]: ");
    let _ = io::stdout().flush();
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() && input.trim().eq_ignore_ascii_case("g") {
        Backend::Gui
    } else {
        Backend::Tui
    }
}

/// Best-effort preflight check: on Linux there's no windowing system to
/// open a window on at all without an X11/Wayland display, and macroquad's
/// underlying platform layer aborts the process rather than returning a
/// catchable error in that case — so this check, not `catch_unwind`, is the
/// fallback path that actually fires in the common "no display" case (e.g.
/// an SSH session or a CI box). macOS/Windows always have a compositor
/// available to the active session, so there's nothing analogous to check.
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
