//! Reads, edits and re-packs a save file, for testing.
//!
//! Saves are bincode (see `save::SAVE_FORMAT_VERSION`), which has no field
//! names on disk and is not editable by hand. `dump` renders one as RON,
//! `pack` puts an edited one back, and `warp` advances a save to a later
//! zone by running the real breach rather than by editing a number — see
//! `Game::warp_to_zone` for why that distinction matters.
//!
//! ```sh
//! cargo run -p feral-processes-engine --bin savetool -- dump saves/save.bin s.ron
//! cargo run -p feral-processes-engine --bin savetool -- pack s.ron saves/save.bin
//! cargo run -p feral-processes-engine --bin savetool -- warp saves/save.bin 6
//! ```
//!
//! `pack` always stamps the *current* `SAVE_FORMAT_VERSION`, so dumping a
//! save before a format bump and packing it after is the one way to carry a
//! save across a version change. There is no other migration path, by
//! design.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use feral_processes_engine::{Game, save};

const USAGE: &str = "\
usage:
  savetool dump <save.bin> [out.ron]   read a save out as editable RON
  savetool pack <in.ron> <save.bin>    write edited RON back as a save
  savetool warp <save.bin> <zone>      breach the save forward to <zone>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    let result = match args.as_slice() {
        ["dump", save] => dump(Path::new(save), None),
        ["dump", save, out] => dump(Path::new(save), Some(Path::new(out))),
        ["pack", text, save] => pack(Path::new(text), Path::new(save)),
        ["warp", save, zone] => warp(Path::new(save), zone),
        _ => Err(USAGE.to_string()),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn dump(save_path: &Path, out: Option<&Path>) -> Result<(), String> {
    let data =
        save::load_from_file(save_path).map_err(|e| format!("{}: {e}", save_path.display()))?;
    let text = save::to_ron(&data).map_err(|e| e.to_string())?;
    match out {
        Some(path) => {
            std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
            eprintln!("wrote {}", path.display());
        }
        None => print!("{text}"),
    }
    Ok(())
}

fn pack(text_path: &Path, save_path: &Path) -> Result<(), String> {
    let text =
        std::fs::read_to_string(text_path).map_err(|e| format!("{}: {e}", text_path.display()))?;
    let data = save::from_ron(&text).map_err(|e| format!("{}: {e}", text_path.display()))?;
    save::save_to_file(save_path, &data).map_err(|e| format!("{}: {e}", save_path.display()))?;
    eprintln!(
        "wrote {} at save format v{}",
        save_path.display(),
        save::SAVE_FORMAT_VERSION
    );
    Ok(())
}

fn warp(save_path: &Path, zone: &str) -> Result<(), String> {
    let zone: u32 = zone
        .parse()
        .map_err(|_| format!("`{zone}` is not a zone number"))?;
    let mut game = Game::load(save_path, &assets_dir())
        .map_err(|e| format!("{}: {e}", save_path.display()))?;
    game.warp_to_zone(zone)?;
    game.save(save_path)
        .map_err(|e| format!("{}: {e}", save_path.display()))?;
    eprintln!("{} is now in zone {zone}", save_path.display());
    Ok(())
}

/// Warping runs the real sim, so it needs the game's assets. Resolved from
/// the crate's own location the same way the launcher resolves them, since
/// this tool is only ever run out of the repo.
fn assets_dir() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(&crate_dir)
        .join("assets")
}
