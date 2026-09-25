# Can an agent see the game? — `--screenshot`

**Question:** can a session with no person at the desktop get a real
rendered frame to look at?

**Run** (2026-09-25, RTX 5070 Ti, NVIDIA 595.91.07, Vulkan, GNOME on
Wayland, `WAYLAND_DISPLAY=wayland-0` in the agent's environment):

```sh
cargo run -- --template stack --screenshot out.png
```

**Result:** yes. A 1280x720 PNG (145 KB) of the Stack template's corridor
view, HUD and all, written after 30 settle frames. No Xvfb, no screenshot
tool and no portal consent. Bevy's own `Screenshot::primary_window()`
reads the frame back.

**Blind spots / found along the way:**

- The window really opens on the user's desktop for about a second. There's
  no offscreen or hidden-window mode yet.
- **The process exits with SIGSEGV (139) after the PNG is written.** A plain
  quit (`--keys "q y"` from the main menu, no capture) does the same, so the
  crash is in teardown and predates this mode. A script should treat the file's
  existence as success, not the exit code.
- The world clock runs during the settle frames, so a capture is about half
  a second of real time past the scripted keys.
