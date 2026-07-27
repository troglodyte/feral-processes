//! The `feral-processes` binary. Resolves the game's on-disk paths and hands
//! off to the graphical frontend (`feral-processes-gui`) — this crate itself
//! draws nothing and knows nothing about game rules.

use std::io;
use std::path::PathBuf;

use feral_processes_app_core::App;

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

    if !graphics_available() {
        eprintln!("No display detected; feral-processes needs a graphical display.");
        std::process::exit(1);
    }
    feral_processes_gui::run(App::new(assets_dir, saves_dir, history_path));
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
