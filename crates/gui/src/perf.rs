//! The frame-timing readout's rolling window.
//!
//! Kept out of `lib.rs`'s frame system for one reason: the frame system reads
//! the clock and this must not. Every figure here is derived from timestamps
//! the caller passes in, so the whole window is exercised by synthetic
//! samples and no test depends on wall-clock time.
//!
//! What the four figures answer, and why each is here rather than a plain
//! frames-per-second number:
//!
//! - `fps` is a **count** of frames in the window divided by the window's own
//!   elapsed time — not the reciprocal of a smoothed delta, which can be
//!   dragged around by the smoothing rather than by the frames.
//! - `mean_ms` pairs with it. Showing the *instantaneous* frame instead
//!   changes sixty times a second and cannot be read.
//! - `peak_ms` is the whole point. A mean hides a hitch completely: sixty
//!   frames a second with one of them at 90 ms still averages under 18, and
//!   that shape is exactly what the map's jerkiness was reported as.
//! - `draw_ms` is the renderer's own shape-building pass, so the gap between
//!   it and `mean_ms` is bevy's schedule, egui's tessellator, the buffer
//!   upload and the GPU — the part of a frame this game has never measured.

/// How long a window accumulates before its figures are published.
///
/// The figures update once per window rather than per frame: four numbers
/// changing every frame are unreadable, and a peak that decays continuously
/// cannot be caught by eye at all.
const WINDOW_SECONDS: f64 = 1.0;

/// One window's finished figures.
pub(crate) struct PerfReadout {
    pub fps: u32,
    pub mean_ms: f64,
    pub peak_ms: f64,
    pub draw_ms: f64,
}

impl PerfReadout {
    /// The one line the overlay paints.
    ///
    /// Built here rather than in the renderer so the format is unit-testable
    /// without a painter.
    pub fn line(&self) -> String {
        format!(
            "{} fps   {:.1} ms   peak {:.1}   draw {:.1}",
            self.fps, self.mean_ms, self.peak_ms, self.draw_ms
        )
    }
}

/// Accumulates frames until a window closes, then publishes and starts over.
pub(crate) struct PerfMeter {
    window_start: Option<f64>,
    frames: u32,
    frame_total: f64,
    frame_peak: f64,
    draw_total: f64,
    published: Option<PerfReadout>,
}

impl PerfMeter {
    pub fn new() -> Self {
        Self {
            window_start: None,
            frames: 0,
            frame_total: 0.0,
            frame_peak: 0.0,
            draw_total: 0.0,
            published: None,
        }
    }

    /// Records one frame. `now` is the timestamp the frame was drawn at;
    /// `frame_secs` is the whole frame's delta and `draw_secs` the part of it
    /// spent building shapes.
    pub fn sample(&mut self, now: f64, frame_secs: f64, draw_secs: f64) {
        // A frame's delta covers the interval *ending* at `now`, so the first
        // window opens one frame back. Anchored at `now` instead, every
        // window is one frame short of the second it claims to measure, and
        // `fps` and `mean_ms` disagree by that frame.
        let start = *self.window_start.get_or_insert(now - frame_secs);
        self.frames += 1;
        self.frame_total += frame_secs;
        self.frame_peak = self.frame_peak.max(frame_secs);
        self.draw_total += draw_secs;

        let elapsed = now - start;
        if elapsed >= WINDOW_SECONDS {
            let frames = f64::from(self.frames);
            self.published = Some(PerfReadout {
                // Against the window's *own* elapsed time rather than against
                // one second: a window that overshoots by a frame would
                // otherwise report that frame as extra speed.
                fps: (frames / elapsed).round() as u32,
                mean_ms: self.frame_total / frames * 1000.0,
                peak_ms: self.frame_peak * 1000.0,
                draw_ms: self.draw_total / frames * 1000.0,
            });
            self.window_start = Some(now);
            self.frames = 0;
            self.frame_total = 0.0;
            self.frame_peak = 0.0;
            self.draw_total = 0.0;
        }
    }

    /// The last completed window's figures, or `None` before the first one
    /// closes — there is nothing honest to draw in the first second.
    pub fn readout(&self) -> Option<&PerfReadout> {
        self.published.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds `count` frames of `frame_secs` each, advancing the clock by the
    /// same amount, and returns the timestamp it stopped at.
    fn feed(meter: &mut PerfMeter, from: f64, count: u32, frame_secs: f64, draw_secs: f64) -> f64 {
        let mut now = from;
        for _ in 0..count {
            now += frame_secs;
            meter.sample(now, frame_secs, draw_secs);
        }
        now
    }

    /// The figure is a count of frames, so it cannot be dragged off by the
    /// smoothing that a reciprocal-of-delta reading would carry.
    #[test]
    fn the_frame_rate_is_the_frames_that_were_drawn() {
        let mut meter = PerfMeter::new();
        feed(&mut meter, 0.0, 60, 1.0 / 60.0, 0.002);
        let r = meter
            .readout()
            .expect("a full second of frames closes a window");
        assert_eq!(r.fps, 60);
        assert!(
            (r.mean_ms - 16.7).abs() < 0.1,
            "mean should be the frame time: {}",
            r.mean_ms
        );
        assert!(
            (r.draw_ms - 2.0).abs() < 0.01,
            "draw should be the pass time: {}",
            r.draw_ms
        );
    }

    /// The reason `peak_ms` is on the readout at all. One 90 ms frame is a
    /// visible hitch and moves the mean by barely a millisecond, so a readout
    /// carrying the mean alone reports a stuttering game as a smooth one.
    #[test]
    fn a_single_hitch_shows_in_the_peak_and_hides_in_the_mean() {
        let mut meter = PerfMeter::new();
        let now = feed(&mut meter, 0.0, 59, 1.0 / 60.0, 0.002);
        meter.sample(now + 0.090, 0.090, 0.002);
        let r = meter.readout().expect("sixty frames close a window");
        assert!(
            (r.peak_ms - 90.0).abs() < 0.01,
            "the hitch must be the peak: {}",
            r.peak_ms
        );
        assert!(
            r.mean_ms < 18.0,
            "the mean is exactly what fails to show it: {}",
            r.mean_ms
        );
    }

    /// There is no honest reading before a window has closed, and a figure
    /// extrapolated from three frames of startup is the least honest of all.
    #[test]
    fn nothing_is_published_until_the_first_window_closes() {
        let mut meter = PerfMeter::new();
        feed(&mut meter, 0.0, 10, 1.0 / 60.0, 0.002);
        assert!(meter.readout().is_none());
    }

    /// A window that accumulated instead of resetting would average the whole
    /// session, so a game that started slowly would keep reporting slow long
    /// after it recovered — and the peak would never come down at all.
    #[test]
    fn a_new_window_forgets_the_one_before_it() {
        let mut meter = PerfMeter::new();
        let now = feed(&mut meter, 0.0, 59, 1.0 / 60.0, 0.002);
        let now = {
            let t = now + 0.090;
            meter.sample(t, 0.090, 0.002);
            t
        };
        assert!(meter.readout().unwrap().peak_ms > 89.0);

        feed(&mut meter, now, 30, 1.0 / 30.0, 0.004);
        let r = meter.readout().expect("a second window closes too");
        assert_eq!(r.fps, 30);
        assert!(
            r.peak_ms < 40.0,
            "the previous window's hitch is still being reported: {}",
            r.peak_ms
        );
        assert!(
            (r.draw_ms - 4.0).abs() < 0.01,
            "the draw figure is stale too: {}",
            r.draw_ms
        );
    }

    /// A window closes on the first frame *past* a second, so it always
    /// measures a little over the second it is named for — and the overshoot
    /// is one whole frame, which at 60 fps is 1.7% and at 1 fps is 90%.
    ///
    /// So the count has to be divided by the window's real elapsed time.
    /// Taken as a bare frame count it is reported as speed the game never
    /// had, and it invents *more* of it the worse the game is running, which
    /// is exactly backwards from what a readout is for. Two 900 ms frames is
    /// the honest 1 fps; the bare count calls it 2.
    #[test]
    fn a_window_that_overshoots_reports_the_rate_it_actually_saw() {
        let mut meter = PerfMeter::new();
        feed(&mut meter, 0.0, 2, 0.900, 0.010);
        let r = meter.readout().expect("1.8 s of frames closes a window");
        assert_eq!(r.fps, 1, "2 frames over 1.8 s is 1.1/s, not 2");
        assert!(
            (r.mean_ms - 900.0).abs() < 0.01,
            "the mean must agree with the rate: {}",
            r.mean_ms
        );
    }

    /// The overlay paints this string and nothing else, so its shape is
    /// pinned here rather than by reading pixels back.
    #[test]
    fn the_line_names_all_four_figures() {
        let line = PerfReadout {
            fps: 58,
            mean_ms: 17.24,
            peak_ms: 41.3,
            draw_ms: 2.11,
        }
        .line();
        assert_eq!(line, "58 fps   17.2 ms   peak 41.3   draw 2.1");
    }
}
