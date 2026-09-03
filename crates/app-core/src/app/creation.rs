//! The character-creation wizard: nine steps, one mode, one back button.
//!
//! `Mode::CreateCharacter` carries a [`CreationStep`] cursor rather than
//! being nine modes — `Mode::Transfer`'s reason, written out on that
//! variant. What lives here is the key table for each step, the rows each
//! draws, and the roll `[R]` performs.
//!
//! Three things are subtle enough to say twice:
//!
//! **`CharacterChoice::stats[i]` is units *bought* on that axis, never
//! points spent.** `CharacterChoice::cost()` prices them, per axis, at
//! `tuning::CREATION_COST_*`. All four axes cost one point today, so the
//! two readings happen to agree — which is exactly why the distinction has
//! to be held here rather than left to the numbers: repricing any axis is
//! a `tuning.rs` edit, and every reader that treats the array as a point
//! tally would silently start handing that axis out free.
//!
//! **`[r]` rerolls the kit, and only the kit.** It used to roll every
//! choice the player had not made by hand and jump to the summary, which
//! is a way to skip the wizard rather than a way to use a screen of it —
//! and it cost a [`Decided`] flag per step to keep it from destroying a
//! character someone had walked eight steps to build. On the one screen
//! whose choice is a *basket*, the player can see what changes and press
//! it again for free, so none of that applies.
//!
//! **No step can be left with its allowance unspent.** The two screens
//! that hand out a budget — Kit and Points — refuse to advance while
//! anything on them is still affordable, which is the same halt condition
//! [`roll_kit_basket`] reaches for, read as a question. Nothing is
//! stranded by it: an axis or a row priced above the remainder is not
//! affordable, so the screen lets go.
//!
//! **Difficulty is never rolled, and neither is anything else now.** The
//! wizard is walked; the only key that draws a random anything is `[r]` on
//! the Kit step.
//!
//! **The Points step opens on a roll, not a blank form** —
//! `enter_creation_step` seeds it the moment the cursor lands there, once,
//! never inside [`App::creation_rows`] (rebuilt every frame; rolling there
//! would reroll every frame too). The seed does not set `Decided::stats`:
//! it is not a hand-made choice, so `[R]` stays free to replace it and
//! re-entering the step after it *has* been touched by hand does not
//! stomp the player's own spread.

use crate::app::icon_editor::{IconEditor, IconEditorOutcome};
use crate::*;
use feral_processes_engine::PlayerIcon;
use feral_processes_engine::items::ItemId;
use feral_processes_engine::tuning::{
    CREATION_COST_ATK, CREATION_COST_DECOMPILER, CREATION_COST_DEF, CREATION_COST_INTEGRITY,
    CREATION_CREDITS, CREATION_GAIN_INTEGRITY, CREATION_STAT_POINTS, PLAYER_BASE_STATS,
};

/// The (glyph, sprite name) pairs the Icon step offers.
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

/// How many swatches the Colour step offers — the length of the renderer's
/// `palette::PLAYER_CHOICES`, which app-core cannot see.
/// `the_wizard_offers_every_shipped_swatch` in `crates/gui` is what holds
/// the two in step.
pub const CREATION_COLOURS: u8 = 6;

/// Which of the wizard's choices the player has settled **by hand**, and
/// so which `[R]` must leave alone.
///
/// A separate record rather than reading the choice back for a sentinel,
/// because three of the six have no unset value to read: the default look
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
    stats: bool,
}

impl Decided {
    /// Test-only window onto `stats`, because two random draws are not
    /// guaranteed to differ and so cannot black-box-prove the entry seed
    /// left this flag alone — the field is private to this module, so a
    /// black-box test in `tests/creation.rs` has no other way to ask.
    #[cfg(test)]
    pub(crate) fn stats_decided(&self) -> bool {
        self.stats
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

/// A random spread that spends **exactly** `CREATION_STAT_POINTS`: one unit
/// bought at a time from whichever axes are still affordable, so the loop
/// can only halt once nothing affordable is left. That makes the pool
/// invariant (`cost() == Some(CREATION_STAT_POINTS)`) a consequence of the
/// construction rather than something checked after the fact — there is no
/// path through this loop that can under- or over-spend. Shared by the
/// step's own entry seed and by `roll_the_rest`'s `[R]`, so the two can
/// never quote different odds for the same pool.
fn roll_points_spread(roll: &mut Roll) -> [u32; 4] {
    let mut stats = [0u32; 4];
    let costs: Vec<u32> = MainStat::all().iter().map(|s| stat_cost(*s)).collect();
    let mut left = CREATION_STAT_POINTS;
    loop {
        let affordable: Vec<usize> = (0..costs.len()).filter(|i| costs[*i] <= left).collect();
        if affordable.is_empty() {
            break;
        }
        let axis = affordable[roll.below(affordable.len())];
        stats[axis] += 1;
        left -= costs[axis];
    }
    stats
}

/// A random basket that spends **as much of `CREATION_CREDITS` as the
/// shelf allows**: one unit bought at a time from whichever rows are still
/// affordable, so the loop can only halt once nothing affordable is left.
/// `roll_points_spread`'s construction exactly, and for its reason — the
/// spend is a consequence of the loop rather than something checked after
/// it, so `[R]` can never hand out a basket the commit would refuse.
///
/// It does not always land on zero remaining, unlike the stat pool: the
/// cheapest shipped row is 1 Credit, so it does, but a modded shelf whose
/// cheapest item costs 3 would stop with 1 or 2 left over. That is
/// affordable slack, not an overspend.
fn roll_kit_basket(
    roll: &mut Roll,
    shelf: &[feral_processes_engine::StartingItemRow],
) -> Vec<(ItemId, u32)> {
    let mut basket: Vec<(ItemId, u32)> = Vec::new();
    let mut left = CREATION_CREDITS;
    loop {
        let affordable: Vec<usize> = (0..shelf.len())
            .filter(|i| shelf[*i].price > 0 && shelf[*i].price <= left)
            .collect();
        if affordable.is_empty() {
            break;
        }
        let pick = &shelf[affordable[roll.below(affordable.len())]];
        left -= pick.price;
        match basket.iter_mut().find(|(id, _)| *id == pick.id) {
            Some(slot) => slot.1 += 1,
            None => basket.push((pick.id.clone(), 1)),
        }
    }
    basket
}

impl App {
    /// Opens the wizard from the main menu, on its first step, with every
    /// choice back at its default — an abandoned wizard must not leak into
    /// the next one.
    pub(crate) fn open_creation(&mut self) {
        self.status_line = None;
        self.creation_step = CreationStep::Difficulty;
        // `at_creation`, not `default` — the Perk Point allowance rides
        // the choice, and `default()` is the classless, allowance-less
        // player every `Game::new` builds.
        self.creation_choice = CharacterChoice::at_creation();
        self.creation_decided = Decided::default();
        self.creation_difficulty = None;
        self.creation_icon_editor = None;
        self.creation_icon_seeded = false;
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
            CreationStep::Profile => {
                let rows = self.profile_preview_rows();
                match rows.is_empty() {
                    // A first run earns the sentence rather than a blank
                    // box — the step is where the ladder is explained, and
                    // "nothing yet" is the most useful thing it can say to
                    // the player who has earned nothing yet.
                    true => vec![CreationRow::Earned(
                        "Nothing yet — what you achieve carries into every run after it."
                            .to_string(),
                    )],
                    false => rows.into_iter().map(CreationRow::Earned).collect(),
                }
            }
            CreationStep::Class => self
                .creation_catalogue
                .class_rows()
                .into_iter()
                .map(CreationRow::Class)
                .collect(),
            CreationStep::Kit => self
                .creation_catalogue
                .shelf_rows()
                .into_iter()
                .map(|row| {
                    let taken = self.kit_taken(&row.id);
                    CreationRow::Item { row, taken }
                })
                .collect(),
            // The sixth row is the player's own drawing rather than one of
            // the five presets — `drawn` is `CharacterChoice::icon` read
            // back, so the renderer draws "Draw your own…" or "Your
            // drawing" off app-core's own answer rather than re-deriving it.
            CreationStep::Icon => CREATION_ICONS
                .iter()
                .map(|(glyph, sprite)| CreationRow::Icon {
                    glyph: *glyph,
                    sprite: sprite.to_string(),
                })
                .chain(std::iter::once(CreationRow::DrawnIcon {
                    drawn: self.creation_choice.icon.is_some(),
                }))
                .collect(),
            CreationStep::Colour => (0..CREATION_COLOURS)
                .map(|index| CreationRow::Colour { index })
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
            CreationStep::Perks => self
                .creation_catalogue
                .perk_rows()
                .into_iter()
                .map(|row| {
                    let taken = self.perk_taken(row.id);
                    CreationRow::Perk { row, taken }
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
        // No Name row: the name is asked for on the step *after* this one,
        // so a row here could only ever read back a dash.
        let mut rows = vec![
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
                    .map(|row| format!("{} ({})", row.name, row.trade))
                    .unwrap_or_else(|| "—".to_string()),
            ),
        ];
        rows.push(CreationRow::Look {
            label: "Icon".to_string(),
            glyph: choice.glyph,
            colour: choice.colour,
        });
        for (stat, units) in MainStat::all().iter().zip(choice.stats.iter()) {
            rows.push(line(stat.label(), format!("{}", stat_value(*stat, *units))));
        }
        rows.push(line(
            "Kit",
            match choice.items.is_empty() {
                // The kit step's own fallback, said out loud rather than
                // shown as a dash — "you kept your class kit" is a
                // different fact from "you chose nothing".
                true => "class kit".to_string(),
                false => choice
                    .items
                    .iter()
                    .map(|(id, qty)| {
                        let name = self
                            .creation_catalogue
                            .shelf_rows()
                            .into_iter()
                            .find(|row| &row.id == id)
                            .map(|row| row.name)
                            .unwrap_or_else(|| id.as_str().to_string());
                        format!("{qty}x {name}")
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
            },
        ));
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
    ///
    /// **The icon editor takes every key first, whole, while it is open.**
    /// It hangs off the wizard rather than owning a `Mode` of its own (see
    /// `app::icon_editor`), so this is the one door it can intercept
    /// through — Esc included, which would otherwise walk the wizard back a
    /// step instead of reaching `IconEditor::handle_key`'s own Esc.
    ///
    /// **An all-transparent canvas is not a drawing**, and that is decided
    /// here rather than at each surface that asks. `CharacterChoice::icon`
    /// being `Some` is the whole answer to "does this player have a
    /// drawing?" — the row's words, the Colour step's note, the save, the
    /// profile and the texture upload all read it — so a blank kept as
    /// `Some` makes four of those five lie while the fifth (the upload,
    /// which filters blanks itself) quietly disagrees.
    ///
    /// **Both endings return to the Icon step**, the spec's key table:
    /// `Enter` keeps and `Esc` discards, and neither advances. The row is
    /// still selected because opening the editor put the cursor on it.
    pub(crate) fn handle_creation_key(&mut self, key: GameKey) {
        if let Some(editor) = self.creation_icon_editor.as_mut() {
            match editor.handle_key(key) {
                IconEditorOutcome::Open => {}
                IconEditorOutcome::Keep => {
                    let drawn = editor.icon().clone();
                    self.creation_icon_editor = None;
                    self.creation_choice.icon = (!drawn.is_blank()).then_some(drawn);
                }
                IconEditorOutcome::Discard => self.creation_icon_editor = None,
            }
            return;
        }
        if key == GameKey::Esc {
            // The Name step types text, so Esc is the only way out of it —
            // and a typed `R` there must not be a reroll.
            match self.creation_step.prev() {
                Some(prev) => self.enter_creation_step(prev),
                None => {
                    self.status_line = None;
                    self.mode = Mode::MainMenu;
                }
            }
            return;
        }
        // Left and Right page the wizard — but only on the steps that do
        // not spend, where the two keys already mean "take one" and "put
        // one back" (`Mode::Transfer`'s rule, which those screens share).
        // Those are the steps a player cannot leave early anyway, so what
        // pages them is Enter and Esc.
        if !self.creation_step.spends() {
            match key {
                GameKey::Left => {
                    if let Some(prev) = self.creation_step.prev() {
                        self.enter_creation_step(prev);
                    }
                    return;
                }
                GameKey::Right => {
                    self.try_advance_creation();
                    return;
                }
                _ => {}
            }
        }
        match self.creation_step {
            CreationStep::Difficulty => self.handle_creation_difficulty_key(key),
            // Nothing to decide: Up/Down walks the list, and Enter is the
            // same "move on" the paging keys already give every step that
            // does not spend.
            CreationStep::Profile => match key {
                GameKey::Enter => self.advance_creation(),
                _ => self.scroll(key, self.creation_rows().len()),
            },
            CreationStep::Class => self.handle_creation_class_key(key),
            CreationStep::Kit => self.handle_creation_kit_key(key),
            CreationStep::Icon => self.handle_creation_icon_key(key),
            CreationStep::Colour => self.handle_creation_colour_key(key),
            CreationStep::Points => self.handle_creation_points_key(key),
            CreationStep::Perks => self.handle_creation_perks_key(key),
            CreationStep::Routine => self.handle_creation_routine_key(key),
            CreationStep::Name => self.handle_creation_name_key(key),
            CreationStep::Summary => self.handle_creation_summary_key(key),
        }
    }

    /// Advances only if the step will let go of the player, and says why
    /// if it will not. The one door for `Right` and for the two spending
    /// steps' Enter — a step that refuses on one key and not the other is
    /// a screen the player can leave by accident.
    fn try_advance_creation(&mut self) {
        match self.leave_refusal() {
            Some(why) => self.refuse(why),
            None => self.advance_creation(),
        }
    }

    /// Why the current step will not be left as it stands, or `None`.
    ///
    /// **An allowance you can still spend is not a decision you have
    /// made.** The two spending steps hand out a budget and the player
    /// walking past it with points in hand is the mistake this closes; the
    /// test is "nothing on this screen is still affordable" rather than
    /// "the budget is empty", so an axis or a shelf row priced above the
    /// remainder cannot strand the wizard on a screen it will not let go
    /// of. That is `roll_kit_basket`'s halt condition, read as a question.
    fn leave_refusal(&self) -> Option<String> {
        match self.creation_step {
            CreationStep::Difficulty => self
                .creation_difficulty
                .is_none()
                .then(|| "Choose a difficulty first.".to_string()),
            CreationStep::Class => (self.creation_choice.class.is_none()
                && !self.creation_catalogue.class_rows().is_empty())
            .then(|| "Choose a class first.".to_string()),
            CreationStep::Points => {
                let left = self.creation_points_left();
                (MainStat::all().iter().any(|s| stat_cost(*s) <= left))
                    .then(|| format!("{left} points still to spend."))
            }
            CreationStep::Kit => {
                let left = self.creation_credits_left();
                self.creation_catalogue
                    .shelf_rows()
                    .iter()
                    .any(|row| row.price <= left)
                    .then(|| format!("{left} Credits still to spend."))
            }
            _ => None,
        }
    }

    /// Moves to the next step, or commits on the last one.
    fn advance_creation(&mut self) {
        match self.creation_step.next() {
            Some(next) => self.enter_creation_step(next),
            None => self.commit_creation(),
        }
    }

    /// Lands the cursor on `step` — Esc and every forward advance's shared
    /// door. Seeds the Points step's roll on the way in, and only there:
    /// **once**, on arrival, never inside [`App::creation_rows`], which is
    /// rebuilt every frame and would reroll on every one of them. Guarded
    /// on `!creation_decided.stats` so a spread the player has already
    /// redistributed by hand survives walking away and back — the seed
    /// itself never sets that flag, since it is not the hand-made decision
    /// the flag records.
    ///
    /// **The Icon step seeds the same way, latched on `creation_icon_seeded`
    /// rather than `creation_choice.icon.is_none()`.** `None` is also what
    /// taking a preset row produces on purpose — guarding on the value
    /// instead of a one-shot flag would silently un-pick a preset the
    /// moment the player walked back to the Icon step and returned, since
    /// the profile's saved drawing would win the field back every time.
    /// This is not the `Decided` flag the doc comment above says the wizard
    /// has none of: that rule is about protecting a hand-made choice from
    /// `[r]`'s reroll, which nothing here does — the latch only decides
    /// whether the entry seed still has something to do, exactly once.
    fn enter_creation_step(&mut self, step: CreationStep) {
        self.creation_step = step;
        self.menu_selected = 0;
        self.status_line = None;
        if step == CreationStep::Points && !self.creation_decided.stats {
            self.creation_choice.stats = roll_points_spread(&mut Roll::new());
        }
        if step == CreationStep::Icon && !self.creation_icon_seeded {
            self.creation_icon_seeded = true;
            self.creation_choice.icon = self
                .profile
                .player_icon
                .as_deref()
                .and_then(PlayerIcon::decode);
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
        self.advance_creation();
    }

    /// Credits still unspent on the Kit step. Always the whole allowance
    /// outside it, which is the only step that spends any.
    pub fn creation_credits_left(&self) -> u32 {
        CREATION_CREDITS.saturating_sub(self.kit_spend())
    }

    /// What the basket costs, priced off the shelf. An item the shelf does
    /// not offer contributes nothing — the basket can only be built by
    /// taking rows, so that case is unreachable rather than clamped, and
    /// `Game::apply_creation_kit` refuses it outright if it ever is not.
    fn kit_spend(&self) -> u32 {
        let shelf = self.creation_catalogue.shelf_rows();
        self.creation_choice
            .items
            .iter()
            .map(|(id, qty)| {
                shelf
                    .iter()
                    .find(|r| &r.id == id)
                    .map_or(0, |r| r.price.saturating_mul(*qty))
            })
            .sum()
    }

    /// How many units of `id` the basket holds.
    fn kit_taken(&self, id: &ItemId) -> u32 {
        self.creation_choice
            .items
            .iter()
            .find(|(item, _)| item == id)
            .map_or(0, |(_, qty)| *qty)
    }

    /// Sets `id` to `qty`, dropping the row entirely at zero.
    ///
    /// `CharacterChoice::items` is the basket's **only** store — there is no
    /// parallel amount vector as `Mode::Transfer` keeps, because the shelf
    /// is re-derived every frame and a second list would have to be kept in
    /// step with it. Walking back to this step with Esc therefore preserves
    /// the picks for free.
    fn set_kit_taken(&mut self, id: &ItemId, qty: u32) {
        let items = &mut self.creation_choice.items;
        match items.iter_mut().find(|(item, _)| item == id) {
            Some(slot) => slot.1 = qty,
            None if qty > 0 => items.push((id.clone(), qty)),
            None => {}
        }
        items.retain(|(_, qty)| *qty > 0);
    }

    /// The Points step's key table, in Credits: the cursor moves on
    /// Up/Down, Left/Right adjusts the highlighted row, `ShiftLeft`/
    /// `ShiftRight` targets an end of it, `CtrlLeft`/`CtrlRight` halves the
    /// gap and `[r]` rerolls the whole basket. Enter takes it as it stands
    /// — but only once nothing on the shelf is still affordable, which is
    /// `leave_refusal`'s rule.
    ///
    /// **That gate costs the class-kit fallback.** An empty basket still
    /// means "keep the kit my class authored" everywhere downstream (see
    /// `CharacterChoice::items`, and `a_picked_kit_reaches_the_started_run`
    /// for the other branch), and the engine has no idea the wizard exists
    /// — but the player can no longer *reach* it, because the shelf's
    /// cheapest row is affordable at a full allowance. The fallback is kept
    /// rather than deleted: it is what a `CharacterChoice` built anywhere
    /// but this screen still gets, and re-opening the gate is a one-line
    /// change here rather than a feature to rebuild.
    ///
    /// **Enter is the only way forward, and there is deliberately no `[n]`
    /// here.** On the Icon, Colour and Routine steps `[n]` means "I am not
    /// picking on this screen", which those steps have no other way to say.
    /// This one has nothing to say it about any more — the allowance must
    /// be spent — and an `[n]` would have had to mean "empty the basket and
    /// move on", a destructive key wearing a skip key's name.
    ///
    /// A digit is never a row pick here, `Mode::Transfer`'s rule: every row
    /// is a quantity, and there are more rows than there are digits.
    fn handle_creation_kit_key(&mut self, key: GameKey) {
        let len = self.creation_rows().len();
        match key {
            GameKey::Enter => self.try_advance_creation(),
            GameKey::Char('r') => self.reroll_the_kit(),
            GameKey::Up | GameKey::Down => self.scroll(key, len),
            GameKey::Left => self.spend_on_item(|taken, _| taken.saturating_sub(1)),
            GameKey::Right => self.spend_on_item(|taken, max| (taken + 1).min(max)),
            GameKey::ShiftLeft => self.spend_on_item(|_, _| 0),
            GameKey::ShiftRight => self.spend_on_item(|_, max| max),
            GameKey::CtrlLeft => self.spend_on_item(|taken, _| super::basket::halve(taken, 0)),
            GameKey::CtrlRight => self.spend_on_item(super::basket::halve),
            _ => {}
        }
    }

    /// Applies `f` to the highlighted row's unit count, where `max` is the
    /// most that row could hold given what the **other** rows have already
    /// spent — `App::put_available`'s rule, and for its reason: counting the
    /// row's own units against its own ceiling would make it unlowerable
    /// once the allowance ran out.
    ///
    /// The refusal is `spend_on_row`'s: the arrows clamp by construction, so
    /// reaching it means the allowance is gone and the player pressed Right
    /// anyway, which is worth saying.
    fn spend_on_item(&mut self, f: impl FnOnce(u32, u32) -> u32) {
        let shelf = self.creation_catalogue.shelf_rows();
        let Some(row) = shelf.get(self.menu_selected).cloned() else {
            return;
        };
        let before = self.kit_taken(&row.id);
        let others = self.kit_spend() - row.price.saturating_mul(before);
        let max = CREATION_CREDITS.saturating_sub(others) / row.price.max(1);
        let after = f(before, max).min(max);
        if after == before && before == max {
            self.refuse(format!(
                "No Credits left — {} costs {} each.",
                row.name, row.price
            ));
            return;
        }
        self.set_kit_taken(&row.id, after);
        self.status_line = None;
    }

    /// The glyph list, plus the sixth row that opens the icon editor.
    /// `Mode::CreateCharacter`'s Class and Routine key table: taking a
    /// preset row decides the choice and moves on, `[n]` skips the step
    /// with the default look left in place.
    ///
    /// **Advancing on a pick is what splitting the old `Look` step bought.**
    /// While the icons and the swatches shared one screen a pick could not
    /// advance — the other half of the decision was still below the cursor
    /// — so this was the one list in the wizard where Enter left you where
    /// you were.
    ///
    /// **A preset clears `CharacterChoice::icon`.** The two cannot both be
    /// live and the drawn icon wins at the draw site, so a preset that left
    /// a drawing in place would look like the row doing nothing.
    ///
    /// **The sixth row opens the editor instead of advancing.** Taking it
    /// is not itself the decision — `handle_creation_key`'s editor
    /// interception is what turns `Enter`/`Esc` inside it into the actual
    /// keep-or-discard, and neither of those advances either: the spec's
    /// key table has both endings return here. Opening moves the cursor
    /// onto the row so that "with that row selected" holds even when the
    /// editor was opened by its number key, which `selected_index` does not
    /// move the cursor for.
    fn handle_creation_icon_key(&mut self, key: GameKey) {
        if key == GameKey::Char('n') {
            self.advance_creation();
            return;
        }
        let Some(idx) = self.selected_index(key, CREATION_ICONS.len() + 1) else {
            return;
        };
        if idx == CREATION_ICONS.len() {
            self.menu_selected = idx;
            self.creation_icon_editor = Some(IconEditor::open(
                self.creation_choice.icon.clone().unwrap_or_default(),
            ));
            return;
        }
        let (glyph, sprite) = CREATION_ICONS[idx];
        self.creation_choice.glyph = glyph;
        self.creation_choice.sprite = sprite.to_string();
        self.creation_choice.icon = None;
        self.advance_creation();
    }

    /// The swatch list, `handle_creation_icon_key`'s table exactly — the
    /// second half of one look, on its own screen and against the same
    /// live preview cell.
    fn handle_creation_colour_key(&mut self, key: GameKey) {
        if key == GameKey::Char('n') {
            self.advance_creation();
            return;
        }
        let Some(idx) = self.selected_index(key, CREATION_COLOURS as usize) else {
            return;
        };
        self.creation_choice.colour = Some(idx as u8);
        self.advance_creation();
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
            GameKey::Enter => self.try_advance_creation(),
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

    /// The Kit step's key table in Perk Points: Up/Down moves the cursor,
    /// Left/Right buys and refunds a level of the highlighted perk, and
    /// Enter moves on **whether or not the allowance is spent**.
    ///
    /// **This is the one budget that is not a gate**, because it is the one
    /// that is not lost: a Perk Point is the same point after the run
    /// starts and buys the same perk at the same price, so what the screen
    /// does not spend arrives with the run. The stat pool has no such
    /// afterlife, and the Kit allowance turns into goods.
    ///
    /// There is no `[r]` here. The kit's reroll exists because a basket of
    /// two dozen consumables is tedious to assemble and meaningless to
    /// compare; four points across nineteen perks is the decision itself.
    fn handle_creation_perks_key(&mut self, key: GameKey) {
        let len = self.creation_rows().len();
        match key {
            GameKey::Enter => self.advance_creation(),
            GameKey::Up | GameKey::Down => self.scroll(key, len),
            GameKey::Left => self.buy_perk_level(|taken, _| taken.saturating_sub(1)),
            GameKey::Right => self.buy_perk_level(|taken, max| (taken + 1).min(max)),
            GameKey::ShiftLeft => self.buy_perk_level(|_, _| 0),
            GameKey::ShiftRight => self.buy_perk_level(|_, max| max),
            GameKey::CtrlLeft => self.buy_perk_level(|taken, _| super::basket::halve(taken, 0)),
            GameKey::CtrlRight => self.buy_perk_level(super::basket::halve),
            _ => {}
        }
    }

    /// `spend_on_item` for perk levels — `App::put_available`'s rule again:
    /// `max` is what the highlighted row could hold given what the *other*
    /// rows have already bought, so a row stays lowerable once the
    /// allowance runs out.
    fn buy_perk_level(&mut self, f: impl FnOnce(u32, u32) -> u32) {
        let rows = self.creation_catalogue.perk_rows();
        let Some(row) = rows.get(self.menu_selected).cloned() else {
            return;
        };
        let others: u32 = self
            .creation_choice
            .perks
            .iter()
            .filter(|(perk, _)| *perk != row.id)
            .filter_map(|(perk, levels)| {
                rows.iter().find(|r| r.id == *perk).map(|r| r.cost * levels)
            })
            .sum();
        let allowance = self.creation_choice.perk_points;
        let max = allowance.saturating_sub(others) / row.cost.max(1);
        let before = self.perk_taken(row.id);
        let after = f(before, max).min(max);
        if after == before && before == max {
            self.refuse(format!(
                "Not enough Perk Points — {} costs {}.",
                row.name, row.cost
            ));
            return;
        }
        self.creation_choice
            .perks
            .retain(|(perk, _)| *perk != row.id);
        if after > 0 {
            self.creation_choice.perks.push((row.id, after));
        }
        self.status_line = None;
    }

    /// Levels of `perk` the basket holds.
    fn perk_taken(&self, perk: feral_processes_engine::perks::Perk) -> u32 {
        self.creation_choice
            .perks
            .iter()
            .find(|(p, _)| *p == perk)
            .map(|(_, levels)| *levels)
            .unwrap_or(0)
    }

    /// Perk Points still unspent — the Perks step's own figure, for its
    /// footer. `App::creation_credits_left`'s shape on the third budget.
    pub fn creation_perk_points_left(&self) -> u32 {
        let allowance = self.creation_choice.perk_points;
        allowance.saturating_sub(
            self.creation_catalogue
                .perk_cost(&self.creation_choice)
                .unwrap_or(allowance),
        )
    }

    /// The Colour step's one line of help text once a drawing has been
    /// kept, `None` otherwise — the swatch chosen here still colours the
    /// glyph everywhere else, but the map tile draws over it with the
    /// icon, and the step says so rather than quietly deciding nothing.
    ///
    /// A query rather than a `CreationRow`: the note is not a pickable row
    /// and does not change the step's row count, which the height census in
    /// `crates/gui` holds to a fixed ceiling.
    pub fn creation_colour_note(&self) -> Option<String> {
        self.creation_choice.icon.is_some().then(|| {
            "You've drawn a map icon — the map tile shows it instead of this swatch, \
             though the swatch still colours your glyph everywhere else."
                .to_string()
        })
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
        self.advance_creation();
    }

    /// Text entry, `Mode::FuseName`'s table: printable characters up to
    /// `MAX_CUSTOM_NAME_LEN`, Backspace, Enter to start the run — this is
    /// the last step, so `advance_creation` commits from here. A blank
    /// name is allowed and installs no `CustomName`, exactly as today's
    /// nameless player has none.
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

    /// Reads back, and accepts on Enter — which asks for a name rather
    /// than starting the run. `[R]` is live here (`handle_creation_key`
    /// takes it before this), so this is the screen a reroll is read on
    /// *and* rerolled from.
    fn handle_creation_summary_key(&mut self, key: GameKey) {
        match key {
            GameKey::Enter => self.advance_creation(),
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

    /// `[r]` on the Kit step: a fresh basket, in place.
    ///
    /// **It rerolls the kit and nothing else.** The key used to roll every
    /// choice the player had not made by hand and jump to the summary,
    /// which made it a way to skip the wizard rather than a way to use one
    /// screen of it — and it needed a `Decided` flag per step to avoid
    /// destroying a character someone had walked eight steps to build.
    /// Narrowed to the one screen whose choice is a *basket* rather than a
    /// row, none of that applies: the player is looking at what changes,
    /// it changes nothing else, and pressing it again is free.
    ///
    /// A hand-made basket **is** replaced, deliberately. On the one screen
    /// the key lives on, asking for a reroll is the decision.
    fn reroll_the_kit(&mut self) {
        self.creation_choice.items =
            roll_kit_basket(&mut Roll::new(), &self.creation_catalogue.shelf_rows());
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
    /// Perk Points the cross-run profile will grant **after** creation —
    /// `Game::new` pays the achievement ladder once the character is
    /// applied, so these are not spendable on the Perks step and are not
    /// part of its allowance.
    ///
    /// The step says so out loud because the arithmetic is otherwise a
    /// surprise: a screen reading "4 of 4 Perk Points left" that lands the
    /// player on 6 looks like a defect, and the Summary's own profile rows
    /// say `+1 Perk Point` twice, on a different screen, among every other
    /// reward.
    pub fn profile_perk_points(&self) -> u32 {
        feral_processes_engine::achievements::profile_rewards(&self.profile, &self.achievement_db)
            .into_iter()
            .map(|(reward, _)| match reward {
                feral_processes_engine::achievements::Reward::PerkPoints(n) => n,
                _ => 0,
            })
            .sum()
    }

    /// What the cross-run profile is about to grant this run, **folded to
    /// one line per thing** — `achievements::profile_summary`.
    ///
    /// One derivation, two screens: the `Profile` step opens on it and the
    /// Summary reads it back. A receipt (one line per rung) was what this
    /// returned, and it showed a player with two Perk Point achievements
    /// `+1 Perk Point` twice, where what they need to know is that they
    /// open holding two.
    pub fn profile_preview_rows(&self) -> Vec<String> {
        feral_processes_engine::achievements::profile_summary(&self.profile, &self.achievement_db)
    }
}
