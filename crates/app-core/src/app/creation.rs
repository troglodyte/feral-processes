//! The character-creation wizard: seven steps, one mode, one back button.
//!
//! `Mode::CreateCharacter` carries a [`CreationStep`] cursor rather than
//! being seven modes — `Mode::Transfer`'s reason, written out on that
//! variant. What lives here is the key table for each step, the rows each
//! draws, and the roll `[R]` performs.
//!
//! Three things are subtle enough to say twice:
//!
//! **`CharacterChoice::stats[i]` is units *bought* on that axis, never
//! points spent.** `CharacterChoice::cost()` prices them, per axis, at
//! `tuning::CREATION_COST_*`. Treating the array as a point tally makes
//! mitigation — the one axis that costs more than one — silently free.
//!
//! **The roll spends exactly the pool**, so it can never beat point-buy and
//! there is no reason to reroll for size. `[R]` rerolls for *shape* — and
//! only the shape of what the player has not settled by hand. [`Decided`]
//! is the record of that, written at the five sites that take a row;
//! pressing `[R]` again rerolls what `[R]` itself chose and nothing else,
//! so the key can never destroy a character the player walked the wizard
//! to build.
//!
//! **Difficulty is never rolled.** It is the one choice on the board that
//! is a commitment rather than a shape, and a roll that could hand a player
//! permadeath they never picked is not a convenience. `[R]` on the first
//! step is refused instead.

use crate::*;
use feral_processes_engine::abilities::AbilityId;
use feral_processes_engine::species::AffinityClass;
use feral_processes_engine::tuning::{
    CREATION_COST_ATK, CREATION_COST_DECOMPILER, CREATION_COST_DEF, CREATION_COST_INTEGRITY,
    CREATION_GAIN_INTEGRITY, CREATION_STAT_POINTS, PLAYER_BASE_STATS,
};

/// The (glyph, sprite name) pairs the Look step offers.
///
/// A Rust table rather than a content directory, `palette::PLAYER_CHOICES`'
/// reason: these are the *player's* options on one screen, not a catalogue
/// anything else in the game reads. Each pair works today on its glyph
/// alone — a sprite name the table has nothing under draws the glyph, so
/// art blocks nothing and upgrades each option in place as it arrives.
///
/// Every glyph here is absent from `assets/species/` and
/// `assets/structures/`, so the player can never be mistaken for something
/// standing next to them.
///
/// The first pair is `CharacterChoice::default()`'s own look, named through
/// `DEFAULT_PLAYER_SPRITE` so the two cannot drift: a wizard whose first
/// icon differed from the default would draw two different players off one
/// keystroke that decided nothing.
pub const CREATION_ICONS: [(char, &str); 5] = [
    ('@', feral_processes_engine::DEFAULT_PLAYER_SPRITE),
    ('&', "operator"),
    ('*', "weaver"),
    ('!', "spike"),
    ('?', "drifter"),
];

/// How many swatches the Look step offers — the length of the renderer's
/// `palette::PLAYER_CHOICES`, which app-core cannot see.
/// `the_wizard_offers_every_shipped_swatch` in `crates/gui` is what holds
/// the two in step.
pub const CREATION_COLOURS: u8 = 6;

/// Which of the wizard's choices the player has settled **by hand**, and
/// so which `[R]` must leave alone.
///
/// A separate record rather than reading the choice back for a sentinel,
/// because three of the five have no unset value to read: the default look
/// *is* `CREATION_ICONS[0]` in its default colour, and a declined routine
/// and an untouched one are both `None`. Sentinels would have to be
/// reintroduced for the roll's benefit alone, and each would be a second
/// meaning on a field the engine already reads one way.
///
/// The rule at every site is the same one: **taking a row decides that
/// choice, `[n]` skips the step.** A step walked past without a pick stays
/// open to the roll.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Decided {
    class: bool,
    icon: bool,
    colour: bool,
    stats: bool,
    routine: bool,
}

impl Decided {
    /// Whether every choice `[R]` is allowed to touch has been made — the
    /// case where a roll would have nothing to do and would, before this
    /// was tracked, silently replace a finished character instead.
    fn all(&self) -> bool {
        self.class && self.icon && self.colour && self.stats && self.routine
    }
}

/// What one unit of `stat` costs out of the pool.
fn stat_cost(stat: MainStat) -> u32 {
    match stat {
        MainStat::Atk => CREATION_COST_ATK,
        MainStat::Def => CREATION_COST_DEF,
        MainStat::Integrity => CREATION_COST_INTEGRITY,
        MainStat::Decompiler => CREATION_COST_DECOMPILER,
    }
}

/// What the player opens on for `stat` after buying `units` of it —
/// `PLAYER_BASE_STATS` plus the spend, never a redistribution of it.
/// Mirrors `Game::apply_creation_stats`, which is the one that actually
/// writes the numbers.
fn stat_value(stat: MainStat, units: u32) -> i32 {
    let units = units as i32;
    match stat {
        MainStat::Atk => PLAYER_BASE_STATS.atk + units,
        MainStat::Def => PLAYER_BASE_STATS.mitigation + units,
        MainStat::Integrity => PLAYER_BASE_STATS.max_hp + units * CREATION_GAIN_INTEGRITY as i32,
        // `Decompiler::default()` is 0, so the base is the spend.
        MainStat::Decompiler => units,
    }
}

/// A xorshift64* stream for `[R]`.
///
/// Its own rather than the engine's `resources::GameRng`: there is no
/// `Game` yet when the wizard runs, and world generation's own rule —
/// never draw from `GameRng` — is the same instinct. Seeded off the clock,
/// because a rerolled character that came back identical would read as the
/// key doing nothing.
struct Roll(u64);

impl Roll {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64 ^ d.as_secs())
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        // A zero state is a fixed point of xorshift, so never seed one.
        Self(nanos | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A uniform draw below `n`. `n == 0` is never asked of this.
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

impl App {
    /// Opens the wizard from the main menu, on its first step, with every
    /// choice back at its default — an abandoned wizard must not leak into
    /// the next one.
    pub(crate) fn open_creation(&mut self) {
        self.status_line = None;
        self.creation_step = CreationStep::Difficulty;
        self.creation_choice = CharacterChoice::default();
        self.creation_decided = Decided::default();
        self.creation_difficulty = None;
        self.menu_selected = 0;
        self.mode = Mode::CreateCharacter;
    }

    /// Which step is showing.
    pub fn creation_step(&self) -> CreationStep {
        self.creation_step
    }

    /// The character as it stands.
    pub fn creation_choice(&self) -> &CharacterChoice {
        &self.creation_choice
    }

    /// The difficulty picked on the first step, if it has been.
    pub fn creation_difficulty(&self) -> Option<DifficultyMode> {
        self.creation_difficulty
    }

    /// Pool points still unspent. Always the whole pool outside the Points
    /// step, which is the only thing that spends any.
    pub fn creation_points_left(&self) -> u32 {
        CREATION_STAT_POINTS.saturating_sub(self.creation_spend())
    }

    /// What the current spend costs, unclamped by the pool — `cost()`
    /// refuses above it, and nothing here is ever allowed to get there.
    fn creation_spend(&self) -> u32 {
        MainStat::all()
            .iter()
            .zip(self.creation_choice.stats.iter())
            .map(|(stat, units)| units.saturating_mul(stat_cost(*stat)))
            .sum()
    }

    /// The rows of whichever step is showing, in draw order.
    ///
    /// **Exhaustive on `CreationStep`**, the rule `render/stack.rs`'s
    /// `cell_mark` records: as a `_ =>` arm an eighth step would ship with
    /// a blank screen and no failing test.
    pub fn creation_rows(&self) -> Vec<CreationRow> {
        match self.creation_step {
            CreationStep::Difficulty => vec![
                CreationRow::Difficulty {
                    mode: DifficultyMode::Permadeath,
                    label: "Permadeath".to_string(),
                    detail: "flatlining is final; the session is archived to a log".to_string(),
                },
                CreationRow::Difficulty {
                    mode: DifficultyMode::Forgiving,
                    label: "Forgiving".to_string(),
                    detail: "flatlining costs you, but you reboot and keep going".to_string(),
                },
            ],
            CreationStep::Class => self
                .creation_catalogue
                .class_rows()
                .into_iter()
                .map(CreationRow::Class)
                .collect(),
            CreationStep::Look => CREATION_ICONS
                .iter()
                .map(|(glyph, sprite)| CreationRow::Icon {
                    glyph: *glyph,
                    sprite: sprite.to_string(),
                })
                .chain((0..CREATION_COLOURS).map(|index| CreationRow::Colour { index }))
                .collect(),
            CreationStep::Points => MainStat::all()
                .iter()
                .zip(self.creation_choice.stats.iter())
                .map(|(stat, units)| CreationRow::Stat {
                    stat: *stat,
                    spent: *units,
                    value: stat_value(*stat, *units),
                    cost: stat_cost(*stat),
                })
                .collect(),
            CreationStep::Routine => self
                .creation_catalogue
                .starter_rows(self.creation_choice.class)
                .into_iter()
                .map(CreationRow::Routine)
                .collect(),
            CreationStep::Name => vec![CreationRow::Name {
                typed: self.creation_choice.name.clone(),
            }],
            CreationStep::Summary => self.creation_summary_rows(),
        }
    }

    /// The finished character, one line per decision, plus whatever the
    /// cross-run profile is about to grant — see `profile_preview_rows`.
    fn creation_summary_rows(&self) -> Vec<CreationRow> {
        let choice = &self.creation_choice;
        let line = |label: &str, value: String| CreationRow::Summary {
            label: label.to_string(),
            value,
        };
        let mut rows = vec![
            line(
                "Name",
                match choice.name.is_empty() {
                    true => "—".to_string(),
                    false => choice.name.clone(),
                },
            ),
            line(
                "Difficulty",
                match self.creation_difficulty {
                    Some(DifficultyMode::Permadeath) => "Permadeath".to_string(),
                    Some(DifficultyMode::Forgiving) => "Forgiving".to_string(),
                    None => "—".to_string(),
                },
            ),
            line(
                "Class",
                self.creation_catalogue
                    .class_rows()
                    .into_iter()
                    .find(|row| Some(row.class) == choice.class)
                    .map(|row| format!("{} ({})", row.name, row.axes))
                    .unwrap_or_else(|| "—".to_string()),
            ),
            line("Icon", choice.glyph.to_string()),
        ];
        for (stat, units) in MainStat::all().iter().zip(choice.stats.iter()) {
            rows.push(line(stat.label(), format!("{}", stat_value(*stat, *units))));
        }
        rows.push(line(
            "Routine",
            self.creation_catalogue
                .starter_rows(choice.class)
                .into_iter()
                .find(|row| Some(&row.id) == choice.routine.as_ref())
                .map(|row| row.name)
                .unwrap_or_else(|| "—".to_string()),
        ));
        for preview in self.profile_preview_rows() {
            rows.push(line("Profile", preview));
        }
        rows
    }

    /// The key table. Esc walks back one step and off the first one leaves
    /// for the main menu; `[R]` rolls whatever is still undecided and jumps
    /// to the Summary; everything else is the showing step's own.
    pub(crate) fn handle_creation_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            // The Name step types text, so Esc is the only way out of it —
            // and a typed `R` there must not be a reroll.
            match self.creation_step.prev() {
                Some(prev) => {
                    self.creation_step = prev;
                    self.menu_selected = 0;
                    self.status_line = None;
                }
                None => {
                    self.status_line = None;
                    self.mode = Mode::MainMenu;
                }
            }
            return;
        }
        if key == GameKey::Char('R') && self.creation_step != CreationStep::Name {
            self.roll_the_rest();
            return;
        }
        match self.creation_step {
            CreationStep::Difficulty => self.handle_creation_difficulty_key(key),
            CreationStep::Class => self.handle_creation_class_key(key),
            CreationStep::Look => self.handle_creation_look_key(key),
            CreationStep::Points => self.handle_creation_points_key(key),
            CreationStep::Routine => self.handle_creation_routine_key(key),
            CreationStep::Name => self.handle_creation_name_key(key),
            CreationStep::Summary => self.handle_creation_summary_key(key),
        }
    }

    /// Moves to the next step, or commits on the last one.
    fn advance_creation(&mut self) {
        match self.creation_step.next() {
            Some(next) => {
                self.creation_step = next;
                self.menu_selected = 0;
                self.status_line = None;
            }
            None => self.commit_creation(),
        }
    }

    /// The two difficulties, keyed `[p]`/`[f]` exactly as the screen this
    /// step replaces was — a player's muscle memory for starting a run is
    /// the one thing folding it into the wizard should not cost.
    fn handle_creation_difficulty_key(&mut self, key: GameKey) {
        let options = ['p', 'f'];
        let idx = self
            .selected_index(key, options.len())
            .or_else(|| match key {
                GameKey::Char(c) => options.iter().position(|&o| o == c.to_ascii_lowercase()),
                _ => None,
            });
        match idx.map(|i| options[i]) {
            Some('p') => {
                self.creation_difficulty = Some(DifficultyMode::Permadeath);
                self.advance_creation();
            }
            Some('f') => {
                self.creation_difficulty = Some(DifficultyMode::Forgiving);
                self.advance_creation();
            }
            _ => {}
        }
    }

    /// **There is no Unaligned option**, so this step advances only once a
    /// class is picked — every class damps an axis, and a run that skipped
    /// the step would be strictly better than every one that took it.
    ///
    /// An empty `assets/classes/` is still a supported install: with no
    /// rows there is nothing to pick and nothing to refuse, and the step
    /// falls through to the next one so the run is today's game.
    fn handle_creation_class_key(&mut self, key: GameKey) {
        let rows = self.creation_catalogue.class_rows();
        if rows.is_empty() {
            if matches!(key, GameKey::Enter) {
                self.advance_creation();
            }
            return;
        }
        let Some(idx) = self.selected_index(key, rows.len()) else {
            return;
        };
        self.creation_choice.class = Some(rows[idx].class);
        self.creation_decided.class = true;
        self.advance_creation();
    }

    /// The icons and the swatches on one screen, one list: the cursor walks
    /// both, Enter takes whichever kind it is standing on, and `[n]` moves
    /// to the next step. Two lists rather than two steps because an icon
    /// and a colour are one decision — what you look like.
    fn handle_creation_look_key(&mut self, key: GameKey) {
        let rows = self.creation_rows();
        if key == GameKey::Char('n') {
            self.advance_creation();
            return;
        }
        let Some(idx) = self.selected_index(key, rows.len()) else {
            return;
        };
        match &rows[idx] {
            CreationRow::Icon { glyph, sprite } => {
                self.creation_choice.glyph = *glyph;
                self.creation_choice.sprite = sprite.clone();
                self.creation_decided.icon = true;
            }
            CreationRow::Colour { index } => {
                self.creation_choice.colour = Some(*index);
                self.creation_decided.colour = true;
            }
            _ => {}
        }
    }

    /// `Mode::Transfer`'s key idiom exactly: the cursor moves on Up/Down
    /// through `App::scroll` — a digit is never a row pick here, since
    /// there are only four rows and every one of them is a quantity —
    /// Left/Right adjusts the highlighted row, `ShiftLeft`/`ShiftRight`
    /// targets an end of it and `CtrlLeft`/`CtrlRight` halves the gap to
    /// that end. Enter takes the spend as it stands, whether or not the
    /// pool is empty.
    fn handle_creation_points_key(&mut self, key: GameKey) {
        let len = MainStat::all().len();
        match key {
            GameKey::Enter => self.advance_creation(),
            GameKey::Up | GameKey::Down => self.scroll(key, len),
            GameKey::Left => self.spend_on_row(|units, _| units.saturating_sub(1)),
            GameKey::Right => self.spend_on_row(|units, max| (units + 1).min(max)),
            GameKey::ShiftLeft => self.spend_on_row(|_, _| 0),
            GameKey::ShiftRight => self.spend_on_row(|_, max| max),
            GameKey::CtrlLeft => self.spend_on_row(|units, _| super::basket::halve(units, 0)),
            GameKey::CtrlRight => self.spend_on_row(super::basket::halve),
            _ => {}
        }
    }

    /// Applies `f` to the highlighted axis's unit count, where `max` is the
    /// most that axis could hold given what the *other* rows have already
    /// spent — `App::put_available`'s rule, and for its reason: counting
    /// the row's own units against its own ceiling would make it
    /// unloweranble once the pool ran out.
    ///
    /// A request that would overspend is not silently clamped — the arrow
    /// keys clamp by construction, so reaching this refusal means the pool
    /// is empty and the player pressed Right anyway, which is worth
    /// saying.
    fn spend_on_row(&mut self, f: impl FnOnce(u32, u32) -> u32) {
        let stats = MainStat::all();
        let Some(stat) = stats.get(self.menu_selected).copied() else {
            return;
        };
        let cost = stat_cost(stat);
        let others: u32 = stats
            .iter()
            .zip(self.creation_choice.stats.iter())
            .enumerate()
            .filter(|(i, _)| *i != self.menu_selected)
            .map(|(_, (s, units))| units.saturating_mul(stat_cost(*s)))
            .sum();
        let max = CREATION_STAT_POINTS.saturating_sub(others) / cost;
        let before = self.creation_choice.stats[self.menu_selected];
        let after = f(before, max).min(max);
        if after == before && before == max {
            self.refuse(format!(
                "No points left — {} costs {cost} a point.",
                stat.label()
            ));
            return;
        }
        self.creation_choice.stats[self.menu_selected] = after;
        self.creation_decided.stats = true;
        self.status_line = None;
    }

    /// The starter pool, priced through the class already picked. Enter on
    /// a row takes it; `[n]` moves on with the slot left empty, which is
    /// what `CharacterChoice::default()` is and so still a supported run.
    fn handle_creation_routine_key(&mut self, key: GameKey) {
        let rows = self
            .creation_catalogue
            .starter_rows(self.creation_choice.class);
        if key == GameKey::Char('n') || rows.is_empty() && key == GameKey::Enter {
            self.creation_choice.routine = None;
            self.advance_creation();
            return;
        }
        let Some(idx) = self.selected_index(key, rows.len()) else {
            return;
        };
        self.creation_choice.routine = Some(rows[idx].id.clone());
        self.creation_decided.routine = true;
        self.advance_creation();
    }

    /// Text entry, `Mode::FuseName`'s table: printable characters up to
    /// `MAX_CUSTOM_NAME_LEN`, Backspace, Enter to move on. A blank name is
    /// allowed and installs no `CustomName`, exactly as today's nameless
    /// player has none.
    fn handle_creation_name_key(&mut self, key: GameKey) {
        match key {
            GameKey::Backspace => {
                self.creation_choice.name.pop();
            }
            GameKey::Char(c)
                if !c.is_control()
                    && self.creation_choice.name.chars().count()
                        < feral_processes_engine::MAX_CUSTOM_NAME_LEN =>
            {
                self.creation_choice.name.push(c);
            }
            GameKey::Enter => self.advance_creation(),
            _ => {}
        }
    }

    /// Reads back, and commits on Enter.
    fn handle_creation_summary_key(&mut self, key: GameKey) {
        match key {
            GameKey::Enter => self.commit_creation(),
            _ => self.scroll(key, self.creation_rows().len()),
        }
    }

    /// Starts the run. The difficulty is always set by here — the first
    /// step is the only way past itself — but a missing one drops back to
    /// that step rather than guessing, since guessing would be picking
    /// permadeath for someone.
    fn commit_creation(&mut self) {
        let Some(difficulty) = self.creation_difficulty else {
            self.creation_step = CreationStep::Difficulty;
            self.menu_selected = 0;
            self.refuse("Choose a difficulty first.");
            return;
        };
        let choice = self.creation_choice.clone();
        self.start_new_game(difficulty, &choice);
    }

    /// `[R]`: rolls every choice the player has **not** made by hand and
    /// jumps to the Summary. What has been made is left exactly as it is —
    /// `Decided` is the record, and its doc comment is the rule.
    ///
    /// **The spend is exactly the pool**, by construction rather than by a
    /// check: units are bought one at a time from whichever axes are still
    /// affordable, and three of the four cost one point, so the loop can
    /// only stop at zero remaining. It therefore can never beat point-buy,
    /// which is what makes rerolling a question of shape and not of size.
    /// A hand-made spend is not rerolled and not topped up either — the
    /// player may leave points on the table, which the Points step already
    /// allows.
    ///
    /// Neither difficulty nor the name is among what it rolls. Difficulty
    /// is a commitment rather than a shape — see the module doc comment —
    /// and there is no name bank to draw one from.
    fn roll_the_rest(&mut self) {
        let Some(_) = self.creation_difficulty else {
            self.refuse("Choose a difficulty first.");
            return;
        };
        if self.creation_decided.all() {
            self.refuse("Nothing left to roll — every choice is made.");
            return;
        }
        let mut roll = Roll::new();

        if !self.creation_decided.class {
            let classes: Vec<AffinityClass> = self
                .creation_catalogue
                .class_rows()
                .iter()
                .map(|row| row.class)
                .collect();
            if !classes.is_empty() {
                self.creation_choice.class = Some(classes[roll.below(classes.len())]);
            }
        }

        if !self.creation_decided.icon {
            let (glyph, sprite) = CREATION_ICONS[roll.below(CREATION_ICONS.len())];
            self.creation_choice.glyph = glyph;
            self.creation_choice.sprite = sprite.to_string();
        }
        if !self.creation_decided.colour {
            self.creation_choice.colour = Some(roll.below(CREATION_COLOURS as usize) as u8);
        }

        if !self.creation_decided.stats {
            self.creation_choice.stats = [0; 4];
            let costs: Vec<u32> = MainStat::all().iter().map(|s| stat_cost(*s)).collect();
            let mut left = CREATION_STAT_POINTS;
            loop {
                let affordable: Vec<usize> =
                    (0..costs.len()).filter(|i| costs[*i] <= left).collect();
                if affordable.is_empty() {
                    break;
                }
                let axis = affordable[roll.below(affordable.len())];
                self.creation_choice.stats[axis] += 1;
                left -= costs[axis];
            }
        }

        if !self.creation_decided.routine {
            let routines: Vec<AbilityId> = self
                .creation_catalogue
                .starter_rows(self.creation_choice.class)
                .into_iter()
                .map(|row| row.id)
                .collect();
            self.creation_choice.routine = match routines.is_empty() {
                true => None,
                false => Some(routines[roll.below(routines.len())].clone()),
            };
        }

        self.creation_step = CreationStep::Summary;
        self.menu_selected = 0;
        self.status_line = None;
    }

    /// What the cross-run profile is about to grant this run, one line per
    /// reward, for the Summary step to show before the run starts.
    ///
    /// Reads `feral_processes_engine::achievements::profile_rewards`, the
    /// same derivation `Game::grant_profile_rewards` pays from — a preview
    /// that disagreed with what is actually paid would be worse than no
    /// preview, since the player is deciding who to be on the strength of
    /// it.
    pub fn profile_preview_rows(&self) -> Vec<String> {
        feral_processes_engine::achievements::profile_rewards(&self.profile, &self.achievement_db)
            .into_iter()
            .map(|(reward, rolled)| {
                feral_processes_engine::achievements::preview_line(&reward, rolled)
            })
            .collect()
    }
}
