//! A seeded keypress soak over the `dev-saves/` templates.
//!
//! The suite asserts what someone thought to assert. This walks a real `App`
//! through a pseudo-random stream of keys and asserts only that nothing
//! panics, which is the one property no hand-written test can cover: every
//! long loop elsewhere ticks *inside* one screen, and nothing crosses modes.
//!
//! **The walk is deliberately ignorant of what each mode reads.** Asking the
//! mode which keys are live would be a second copy of `App::handle_key`'s
//! dispatch, and the arm worth finding is the one nobody wrote down. So the
//! alphabet is every key a frontend can physically send, uniform.
//!
//! It lives in the launcher because the templates do — `dev_template` is
//! this crate's, and a template is the only cheap route to the mid-run
//! states (a deep zone, a full party, a town, a fight one step away) where
//! the untrodden branches are. A walk from `Game::new` only ever tests the
//! opening minutes.
//!
//! `App::update_realtime` is deliberately never called: it reads
//! `Instant::now()`, so a walk that depended on it would be wall-clock
//! dependent. Keys spend the world's ticks through `handle_key`'s own tail,
//! which is where the movement in this walk comes from.

use std::path::Path;

use feral_processes_app_core::{App, GameKey, Mode};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

/// The lowest and highest printable ASCII a keyboard can produce.
const FIRST_PRINTABLE: u8 = 0x20;
const LAST_PRINTABLE: u8 = 0x7e;

/// One step in this many is an `Esc` while the walk is off `Mode::Playing`.
///
/// Without it a uniform walk drowns: returning to the map needs `Esc`
/// specifically, at one key in 111, and a nested popup needs several in a
/// row — measured, ten templates spent 200 keys each and banked 1 to 5 ticks
/// between them, never reaching the mid-run state the template was generated
/// for. This is a watchdog and deliberately not a weighting of the alphabet:
/// key *choice* stays uniform, so an unlisted key is as likely as a bound
/// one.
///
/// A *nudge* rather than an override, which is the whole of the constant.
/// Sending `Esc` on every stuck step instead meant a walk that wandered into
/// a fight — which `Esc` cannot leave — spent its entire remaining budget
/// pressing `Esc` and never exercised the battle screen at all: nine of ten
/// templates ended in `Mode::Battle`. One step in twelve unwinds menus while
/// the other eleven still press what they drew.
const ESCAPE_AFTER: usize = 12;

/// Every key a frontend can send, in a fixed order so a seed names the same
/// stream across runs.
///
/// Uniform over the whole space rather than weighted towards the keys the
/// game documents: a no-op press costs a step and steps are cheap, while an
/// unlisted key is precisely the one most likely to reach an arm with an
/// unguarded index in it.
pub fn key_alphabet() -> Vec<GameKey> {
    let mut keys = vec![
        GameKey::Up,
        GameKey::Down,
        GameKey::Left,
        GameKey::Right,
        GameKey::ShiftLeft,
        GameKey::ShiftRight,
        GameKey::CtrlLeft,
        GameKey::CtrlRight,
        GameKey::UpLeft,
        GameKey::UpRight,
        GameKey::DownLeft,
        GameKey::DownRight,
        GameKey::Enter,
        GameKey::Esc,
        GameKey::Backspace,
        GameKey::Tab,
    ];
    keys.extend((FIRST_PRINTABLE..=LAST_PRINTABLE).map(|c| GameKey::Char(c as char)));
    keys
}

/// Holds `key_alphabet` to `GameKey`'s actual shape.
///
/// Exhaustive with no wildcard, `render/mod.rs::cell_mark`'s rule: a new
/// `GameKey` variant fails to compile here rather than silently never being
/// pressed, which is the failure a census over a hand-written list cannot
/// see.
#[allow(dead_code)]
fn every_variant_is_spoken_for(key: GameKey) {
    match key {
        GameKey::Up
        | GameKey::Down
        | GameKey::Left
        | GameKey::Right
        | GameKey::ShiftLeft
        | GameKey::ShiftRight
        | GameKey::CtrlLeft
        | GameKey::CtrlRight
        | GameKey::UpLeft
        | GameKey::UpRight
        | GameKey::DownLeft
        | GameKey::DownRight
        | GameKey::Enter
        | GameKey::Esc
        | GameKey::Backspace
        | GameKey::Tab
        | GameKey::Char(_) => {}
    }
}

/// The keys seed `seed` presses, as a pure function of `(seed, steps)`.
///
/// Split from `walk` so the determinism the whole reproducer-by-seed claim
/// rests on is testable in microseconds instead of by replaying a game, and
/// so a failing walk can be shortened by hand.
pub fn key_sequence(seed: u64, steps: usize) -> Vec<GameKey> {
    let alphabet = key_alphabet();
    let mut rng = StdRng::seed_from_u64(seed);
    (0..steps)
        .map(|_| alphabet[rng.random_range(0..alphabet.len())])
        .collect()
}

/// What a walk did, for a caller that wants to know it did anything.
///
/// `keys` is what was actually pressed rather than what was asked for: a
/// walk that cannot get back into the run stops, and the gap between the two
/// is how a vacuous soak tells on itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoakReport {
    pub keys: usize,
    pub reloads: usize,
    pub ticks: u64,
}

/// Presses `keys` into `app`, reloading `save` whenever the walk leaves the
/// run, and reports what it spent.
///
/// Leaving the run is a restart and not the end: reaching the mid-run state
/// is the whole reason this walks a template, so draining the remaining
/// budget on the main menu would throw that away. Both exits are treated the
/// same way — `App::quit` is cleared and the template reloaded — because a
/// quit-app confirm and a quit-run confirm are the same loss of the state
/// under test.
///
/// A reload that does not land stops the walk instead of retrying once per
/// remaining key: a run sealed `game_over` is refused by `Game::load` for
/// good, so continuing would report a full budget spent on nothing.
pub fn walk(app: &mut App, save: &Path, keys: &[GameKey]) -> SoakReport {
    let tick = |app: &App| app.game.as_ref().map_or(0, |g| g.current_tick());

    // Banked per segment rather than measured end to end, because a reload
    // rewinds the clock to the template's own tick — an end-to-end
    // subtraction reads a walk that reloaded as a walk that did nothing.
    let mut ticks = 0;
    let mut segment_start = tick(app);
    let mut reloads = 0;
    let mut pressed = 0;

    // Consecutive steps spent off `Mode::Playing`, which is what
    // `ESCAPE_AFTER` counts against.
    let mut stuck = 0;
    for &key in keys {
        if app.mode == Mode::Playing {
            stuck = 0;
        } else {
            stuck += 1;
        }
        let key = if stuck > 0 && stuck % ESCAPE_AFTER == 0 {
            GameKey::Esc
        } else {
            key
        };
        app.handle_key(key);
        pressed += 1;

        if app.quit || app.mode == Mode::MainMenu {
            ticks += tick(app).saturating_sub(segment_start);
            app.quit = false;
            app.load_game(save.to_path_buf());
            reloads += 1;
            if app.mode != Mode::Playing {
                break;
            }
            segment_start = tick(app);
        }
    }

    ticks += tick(app).saturating_sub(segment_start);
    SoakReport {
        keys: pressed,
        reloads,
        ticks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev_template;
    use std::path::PathBuf;

    /// How many keys each template is walked through in the suite.
    ///
    /// A budget rather than a target: the long run belongs in a bin, and this
    /// has to share `cargo test --workspace` with 6,000 other tests.
    const SUITE_STEPS: usize = 1000;

    /// An `App` whose every writable path lands in a scratch directory.
    ///
    /// The point is that no key needs excluding for safety: the walk is free
    /// to press save, quit-and-save and anything that writes a profile or a
    /// run history, and none of it reaches the player's real data. `App::new`
    /// taking all six paths as arguments is what makes that true.
    fn soak_app(tag: &str) -> (App, PathBuf) {
        let scratch = std::env::temp_dir().join(format!("feral_processes_soak_{tag}"));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).expect("a scratch directory for the soak");
        let app = App::new(
            dev_template::assets_dir(),
            scratch.join("saves"),
            scratch.join("history.ron"),
            scratch.join("profile.ron"),
            dev_template::repo_root().join("dev-arenas"),
            scratch.join("telemetry.ron"),
        );
        (app, scratch)
    }

    /// The property every failure report rests on: a seed names a stream, so
    /// a panic found here replays from the seed alone.
    #[test]
    fn a_seed_names_the_same_stream_every_time() {
        assert_eq!(key_sequence(7, 64), key_sequence(7, 64));
        assert_ne!(
            key_sequence(7, 64),
            key_sequence(8, 64),
            "two seeds walked the same keys, so the seed is not the stream"
        );
    }

    /// A prefix and not a fresh draw, so shortening a failing walk by hand
    /// keeps the keys it failed on.
    #[test]
    fn a_shorter_walk_is_a_prefix_of_a_longer_one() {
        let long = key_sequence(11, 64);
        assert_eq!(key_sequence(11, 16), long[..16]);
    }

    /// The alphabet is the whole physical keyboard. `every_variant_is_spoken_for`
    /// is what holds the non-`Char` half to `GameKey` itself; this holds the
    /// `Char` half, where a range is easy to narrow by accident.
    #[test]
    fn the_alphabet_covers_every_printable_character() {
        let alphabet = key_alphabet();
        for c in FIRST_PRINTABLE..=LAST_PRINTABLE {
            assert!(
                alphabet.contains(&GameKey::Char(c as char)),
                "{:?} is not in the alphabet",
                c as char
            );
        }
        assert_eq!(
            alphabet.len(),
            16 + (LAST_PRINTABLE - FIRST_PRINTABLE + 1) as usize
        );
    }

    /// **The soak.** Every checked-in template, walked through a stream of
    /// keys nobody chose for it.
    ///
    /// The only assertion about the game is that nothing panicked — reaching
    /// this line is the pass. The two assertions that *are* here guard against
    /// the walk being vacuous, which is the way a soak fails silently: `keys`
    /// short of the budget means it could not get back into the run, and no
    /// ticks at all means it never advanced the world it was meant to stress.
    #[test]
    fn every_template_survives_a_walk_nobody_chose_for_it() {
        let mut total = 0u64;
        let mut vacuous = Vec::new();
        let mut short = Vec::new();

        for (n, name) in dev_template::list().into_iter().enumerate() {
            let save = std::env::temp_dir().join(format!("feral_processes_soak_{name}.bin"));
            dev_template::generate(&name, &save).expect("a checked-in template generates");

            let (mut app, scratch) = soak_app(&name);
            app.load_game(save.clone());
            assert_eq!(
                app.mode,
                Mode::Playing,
                "the {name} template did not load: {:?}",
                app.status_line
            );

            // Seeded off the index so each template walks a different stream
            // while the whole suite stays reproducible.
            let report = walk(&mut app, &save, &key_sequence(n as u64, SUITE_STEPS));
            let ended_in = app.mode;

            let _ = std::fs::remove_file(&save);
            let _ = std::fs::remove_dir_all(&scratch);

            // Reported rather than asserted on, `species::stat_shape_faults`'
            // call: which template banks how many ticks moves with the
            // alphabet, with `ESCAPE_AFTER` and with any content change, so a
            // per-template floor would fail the build for a reshuffle rather
            // than a defect. The figures are here to be read when one does
            // fail.
            eprintln!(
                "{name}: {} keys, {} reloads, {} ticks, ended in {ended_in:?}",
                report.keys, report.reloads, report.ticks
            );
            if report.keys < SUITE_STEPS {
                short.push(format!("{name} ({} keys)", report.keys));
            }
            total += report.ticks;
            if report.ticks == 0 {
                vacuous.push(format!("{name} (ended in {ended_in:?})"));
            }
        }

        assert!(
            short.is_empty(),
            "these walks could not get back into the run: {short:?}"
        );
        eprintln!("total ticks: {total}, templates that never ticked: {vacuous:?}");
        assert!(
            total > 0,
            "the whole soak advanced the world by nothing, so it tested no world at all"
        );
    }
}
