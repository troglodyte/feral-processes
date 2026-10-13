//! When the base's ambient blips play. The clips themselves are in
//! `sounds.rs`; this only decides which one, and when.
//!
//! Atmosphere, not feedback: nothing here reads the sim, so it has no way
//! to disagree with it.

/// Shortest and longest quiet stretch between two blips, in seconds.
const MIN_GAP: f64 = 2.0;
const MAX_GAP: f64 = 8.0;

/// How loud ambience plays against the master volume. The clips peak at the
/// same level as the action cues, and ambience has to sit under them.
pub const DRONE_MIX: f32 = 0.35;
pub const BLIP_MIX: f32 = 0.5;

pub struct Ambience {
    rng: u64,
    blips: usize,
    next_at: Option<f64>,
    last: Option<usize>,
    active: bool,
}

impl Ambience {
    /// `blips` is how many clips there are to choose between.
    pub fn new(seed: u64, blips: usize) -> Self {
        Self {
            rng: seed,
            blips,
            next_at: None,
            last: None,
            active: false,
        }
    }

    /// The blip due this frame, if any. Leaving the base forgets the pending
    /// one, so coming back starts a fresh gap rather than playing whatever
    /// fell due while away the instant you arrive.
    pub fn tick(&mut self, now: f64, active: bool) -> Option<usize> {
        self.active = active;
        if !active {
            self.next_at = None;
            return None;
        }
        let Some(due) = self.next_at else {
            self.next_at = Some(now + self.gap());
            return None;
        };
        if now < due {
            return None;
        }
        self.next_at = Some(now + self.gap());
        let blip = self.pick();
        self.last = Some(blip);
        Some(blip)
    }

    /// Whether the last `tick` was in the base — the drone's on-switch.
    pub fn is_active(&self) -> bool {
        self.active
    }

    fn gap(&mut self) -> f64 {
        MIN_GAP + (MAX_GAP - MIN_GAP) * self.unit()
    }

    /// Never the clip that played last: the same beep twice running reads
    /// as a repeat of one event rather than a room full of machines.
    fn pick(&mut self) -> usize {
        let choices = match self.last {
            Some(_) if self.blips > 1 => self.blips - 1,
            _ => self.blips,
        };
        let k = (self.next() % choices as u64) as usize;
        match self.last {
            Some(last) if self.blips > 1 && k >= last => k + 1,
            _ => k,
        }
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// splitmix64: no `rand` dependency for one jittered timer.
    fn next(&mut self) -> u64 {
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE5_E9B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEP: f64 = 1.0 / 60.0;

    /// Every blip `amb` plays over `seconds` at 60 fps, with its time.
    fn run(amb: &mut Ambience, from: f64, seconds: f64, active: bool) -> Vec<(f64, usize)> {
        let frames = (seconds / STEP) as usize;
        (0..frames)
            .filter_map(|f| {
                let now = from + f as f64 * STEP;
                amb.tick(now, active).map(|b| (now, b))
            })
            .collect()
    }

    #[test]
    fn nothing_plays_outside_the_base() {
        let mut amb = Ambience::new(1, 6);
        assert!(run(&mut amb, 0.0, 120.0, false).is_empty());
    }

    #[test]
    fn gaps_stay_in_range_and_no_clip_repeats() {
        for seed in 0..20 {
            let mut amb = Ambience::new(seed, 6);
            let played = run(&mut amb, 0.0, 600.0, true);
            assert!(
                played.len() > 60,
                "seed {seed}: only {} blips",
                played.len()
            );
            assert!(played[0].0 >= MIN_GAP && played[0].0 <= MAX_GAP + STEP);
            for pair in played.windows(2) {
                let gap = pair[1].0 - pair[0].0;
                assert!(
                    (MIN_GAP - STEP..=MAX_GAP + STEP).contains(&gap),
                    "seed {seed}: gap {gap}"
                );
                assert_ne!(pair[0].1, pair[1].1, "seed {seed}: repeat");
                assert!(pair[1].1 < 6);
            }
        }
    }

    #[test]
    fn every_clip_gets_played() {
        let mut amb = Ambience::new(3, 6);
        let played = run(&mut amb, 0.0, 600.0, true);
        for clip in 0..6 {
            assert!(
                played.iter().any(|&(_, b)| b == clip),
                "clip {clip} never played"
            );
        }
    }

    #[test]
    fn returning_to_the_base_waits_a_fresh_gap() {
        let mut amb = Ambience::new(5, 6);
        // Arm a pending blip, then leave long past when it was due.
        amb.tick(0.0, true);
        run(&mut amb, STEP, 60.0, false);
        let back = 100.0;
        assert_eq!(amb.tick(back, true), None);
        let played = run(&mut amb, back + STEP, 10.0, true);
        assert!(played[0].0 - back >= MIN_GAP - STEP);
    }

    #[test]
    fn a_single_clip_still_plays() {
        let mut amb = Ambience::new(9, 1);
        let played = run(&mut amb, 0.0, 60.0, true);
        assert!(played.len() > 3 && played.iter().all(|&(_, b)| b == 0));
    }
}
