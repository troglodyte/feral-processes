//! The dev screenshot mode: run the real frontend for a few frames, write
//! the primary window to a PNG, and exit — so an agent with no eyes on the
//! desktop can still look at a screen, with `Read` on the file.
//!
//! Reached only from the launcher's `--screenshot`; a normal run never
//! installs any of this.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::window::WindowResolution;

/// Frames drawn before the capture is taken. The first few are spent on
/// setup — the egui context appears once it has seen the camera, and the
/// fonts bind a pass after that — so frame one is not the screen.
const SETTLE_FRAMES: u32 = 30;

/// The size every capture is taken at, in physical pixels at scale 1.0, so
/// two captures compare pixel for pixel whatever monitor ran them. It is
/// the layout floor the HUD is already sized against.
pub const CAPTURE_WIDTH: u32 = 1280;
pub const CAPTURE_HEIGHT: u32 = 720;

/// Where to write the capture. Its presence is the whole mode.
#[derive(Resource, Clone, Debug)]
pub struct Capture {
    pub path: PathBuf,
}

/// `FERAL_DEV_CAPTURE_SIZE=1440x900` takes the capture at another size, for
/// checking a layout above the 1280x720 floor. Unset, or anything that is
/// not `WxH`, is the floor itself.
pub(crate) fn resolution() -> WindowResolution {
    let (w, h) = std::env::var("FERAL_DEV_CAPTURE_SIZE")
        .ok()
        .and_then(|v| parse_size(&v))
        .unwrap_or((CAPTURE_WIDTH, CAPTURE_HEIGHT));
    WindowResolution::new(w, h).with_scale_factor_override(1.0)
}

fn parse_size(text: &str) -> Option<(u32, u32)> {
    let (w, h) = text.split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?))
}

pub(crate) fn install(app: &mut bevy::app::App, capture: Capture) {
    app.insert_resource(capture)
        .add_systems(Last, take_after_settling);
}

/// Spawns the one screenshot once the screen has settled. The exit waits for
/// the save rather than riding this frame: the readback lands frames later,
/// and exiting here would end the process with no file written.
fn take_after_settling(mut commands: Commands, capture: Res<Capture>, mut frames: Local<u32>) {
    *frames += 1;
    if *frames != SETTLE_FRAMES {
        return;
    }
    let path = capture.path.clone();
    commands.spawn(Screenshot::primary_window()).observe(
        move |captured: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
            let saved = captured
                .image
                .clone()
                .try_into_dynamic()
                .map_err(|e| e.to_string())
                .and_then(|img| img.to_rgb8().save(&path).map_err(|e| e.to_string()));
            match saved {
                Ok(()) => {
                    eprintln!("screenshot written to {}", path.display());
                    exit.write(AppExit::Success);
                }
                Err(e) => {
                    eprintln!("cannot write screenshot {}: {e}", path.display());
                    exit.write(AppExit::error());
                }
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::parse_size;

    #[test]
    fn a_capture_size_is_width_x_height_or_nothing() {
        assert_eq!(parse_size("1440x900"), Some((1440, 900)));
        assert_eq!(parse_size("1440"), None);
        assert_eq!(parse_size("wide x tall"), None);
    }
}
