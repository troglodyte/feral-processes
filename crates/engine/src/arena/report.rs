//! What a run of the arena produces.

use serde::Serialize;

use super::scenario::Scenario;

/// One fight, start to finish.
///
/// `transcript` is `Vec<String>` rather than `Vec<LogLine>` on purpose: the
/// report is for reading and post-processing, and `MessageKind` /
/// `MessageSource` are the log's internal vocabulary, not a file format.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RepRecord {
    pub seed: u64,
    pub won: bool,
    pub rounds: u32,
    pub player_hp_fraction: f32,
    pub companions_downed: u32,
    /// What this rep actually fought — one entry per group, in formation
    /// order, `(species id, member count)`. A rolled encounter fields
    /// something different every rep and the transcript names only the front
    /// group, so without this a rolled report cannot be read at all.
    pub composition: Vec<(String, u32)>,
    pub transcript: Vec<String>,
}

/// Every rep, plus the scenario that produced them — a report is meant to
/// be readable a month later without the file that made it.
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub scenario: Scenario,
    pub warnings: Vec<String>,
    pub reps: Vec<RepRecord>,
}

/// What a run of many reps says, once. One fight is an anecdote; a win rate
/// is what a difficulty knob moves.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Summary {
    pub reps: u32,
    pub wins: u32,
    pub win_rate: f32,
    pub mean_rounds: f32,
    pub median_rounds: u32,
    pub mean_player_hp_fraction: f32,
    pub mean_companions_downed: f32,
    /// In rep order, so a bad fight found at rep 37 can be replayed alone by
    /// pinning its seed. The one thing in here that is actionable rather
    /// than merely descriptive.
    pub loss_seeds: Vec<u64>,
}

impl Report {
    pub fn summary(&self) -> Summary {
        let n = self.reps.len();
        let mean = |total: f32| if n == 0 { 0.0 } else { total / n as f32 };
        let wins = self.reps.iter().filter(|r| r.won).count() as u32;

        let mut rounds: Vec<u32> = self.reps.iter().map(|r| r.rounds).collect();
        rounds.sort_unstable();
        // The lower of the two middle values on an even-length set —
        // stated here because an unstated median convention is exactly what
        // changes quietly between refactors.
        let median_rounds = if n == 0 { 0 } else { rounds[(n - 1) / 2] };

        Summary {
            reps: n as u32,
            wins,
            win_rate: mean(wins as f32),
            mean_rounds: mean(self.reps.iter().map(|r| r.rounds as f32).sum()),
            median_rounds,
            mean_player_hp_fraction: mean(self.reps.iter().map(|r| r.player_hp_fraction).sum()),
            mean_companions_downed: mean(
                self.reps.iter().map(|r| r.companions_downed as f32).sum(),
            ),
            loss_seeds: self
                .reps
                .iter()
                .filter(|r| !r.won)
                .map(|r| r.seed)
                .collect(),
        }
    }

    pub fn to_ron(&self) -> Result<String, String> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(seed: u64, won: bool, rounds: u32, hp: f32, downed: u32) -> RepRecord {
        RepRecord {
            seed,
            won,
            rounds,
            player_hp_fraction: hp,
            companions_downed: downed,
            composition: Vec::new(),
            transcript: Vec::new(),
        }
    }

    fn report(reps: Vec<RepRecord>) -> Report {
        Report {
            scenario: Scenario::default(),
            warnings: Vec::new(),
            reps,
        }
    }

    #[test]
    fn win_rate_and_means_over_a_mixed_set() {
        let s = report(vec![
            record(1, true, 4, 1.0, 0),
            record(2, true, 6, 0.5, 1),
            record(3, false, 10, 0.0, 2),
            record(4, true, 8, 0.5, 1),
            record(5, false, 12, 0.0, 3),
        ])
        .summary();

        assert_eq!(s.reps, 5);
        assert_eq!(s.wins, 3);
        assert_eq!(s.win_rate, 0.6);
        assert_eq!(s.mean_rounds, 8.0);
        assert_eq!(s.mean_player_hp_fraction, 0.4);
        assert_eq!(s.mean_companions_downed, 1.4);
    }

    #[test]
    fn an_even_length_set_takes_the_lower_of_the_two_middle_rounds() {
        let s = report(vec![
            record(1, true, 2, 1.0, 0),
            record(2, true, 4, 1.0, 0),
            record(3, true, 6, 1.0, 0),
            record(4, true, 8, 1.0, 0),
        ])
        .summary();
        assert_eq!(s.median_rounds, 4, "the lower middle, not 6 and not 5");
    }

    #[test]
    fn loss_seeds_holds_exactly_the_losing_seeds_in_rep_order() {
        let s = report(vec![
            record(70, true, 1, 1.0, 0),
            record(71, false, 1, 0.0, 0),
            record(72, true, 1, 1.0, 0),
            record(73, false, 1, 0.0, 0),
        ])
        .summary();
        assert_eq!(s.loss_seeds, vec![71, 73]);
    }

    #[test]
    fn an_empty_run_divides_by_nothing() {
        let s = report(Vec::new()).summary();
        assert_eq!(s.reps, 0);
        assert_eq!(s.win_rate, 0.0);
        assert_eq!(s.mean_rounds, 0.0);
        assert_eq!(s.median_rounds, 0);
        assert!(s.loss_seeds.is_empty());
    }
}
