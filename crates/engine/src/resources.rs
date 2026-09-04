use bevy_ecs::prelude::{Entity, Resource};
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::battle::{AttackOutcome, BattleAction, EnemyGroup};
use crate::components::GlyphColor;
use crate::items::GearCopy;
use crate::stack::{Dir, Frame};
use crate::structures::StructureId;

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum DifficultyMode {
    Permadeath,
    Forgiving,
}

#[derive(Resource, Default, Clone, Copy, Serialize, Deserialize)]
pub struct GameClock {
    pub tick: u64,
}

/// Whether the player's own step into solid base-space rock is a swing or a
/// refusal.
///
/// **Off when a run starts.** A step into rock used to be a swing
/// unconditionally, which meant a developed player navigating their own base
/// demolished it a corner at a time — you clip a wall on the way past and the
/// wall is gone. The excavation plan (`m`) already exists for deliberate
/// digging, so this is the player's own bump asking to be armed first.
///
/// **It governs the player's bump and nothing else.** `Game::run_dig_crew`
/// never reads it: a marked cell is dug by whoever is posted to it whether
/// the player's mining is armed or not, because the mark *is* the
/// instruction and disarming a tool you are holding says nothing about a job
/// the base was already given.
///
/// Saved behind a `#[serde(default)]`, so an existing save loads with mining
/// off — which is both the new default and the safe reading of a save that
/// never expressed a preference. Additive, so it costs no
/// `SAVE_FORMAT_VERSION` bump.
#[derive(Resource, Default, Clone, Copy, Serialize, Deserialize)]
pub struct MiningMode(pub bool);

/// Which `StructureDef::first_free` structures this run has already had for
/// nothing. Written by `Game::spawn_structure` — the one place a structure's
/// component list is written, and so the one moment a freebie is genuinely
/// spent rather than merely asked for. A filed request that is cancelled, or
/// wiped with the cell it stood on, costs the run nothing.
///
/// Not wiped in `Game::enter_next_zone`: the base travels through a breach
/// and the Broker standing in it travels too, so a freebie that reset per
/// zone would be one per breach rather than one per run.
///
/// A `BTreeSet` for `KnownRoutines`' reason — the save writes it out, and a
/// `HashSet`'s iteration order would make the encoded bytes differ run to
/// run. Additive behind a `#[serde(default)]`, so it costs no
/// `SAVE_FORMAT_VERSION` bump and a save written before it existed loads
/// with its freebies unspent.
#[derive(Resource, Default, Clone, Serialize, Deserialize)]
pub struct FreeBuilds(pub std::collections::BTreeSet<crate::structures::StructureId>);

#[derive(Resource)]
pub struct GameRng(pub StdRng);

/// Which research nodes the player has unlocked (see `research::ResearchDb`).
/// Empty at the start of a run — every node in the tree begins locked.
#[derive(Resource, Default)]
pub struct Research(pub std::collections::HashSet<crate::research::ResearchId>);

/// Which routines the player has learned and may install, given a blank
/// Routine Disk to burn one onto. Written by exactly two things:
/// `Game::unlock_research` (a node's `unlocks_abilities`) and
/// `Game::extract_routine`. Knowledge is permanent — installing spends a
/// disk, never the knowledge.
///
/// A `BTreeSet` rather than a `HashSet` for the reason `components::Stock`
/// keys by a `BTreeMap`: the save writes this set out, and a `HashSet`'s
/// iteration order would make the encoded bytes differ run to run.
#[derive(Resource, Default)]
pub struct KnownRoutines(pub BTreeSet<crate::abilities::AbilityId>);

/// How many lines the log holds before dropping its oldest.
///
/// Public because it is the whole of the history screen's reach: asking
/// `Game::message_log` for this many is asking for everything still kept,
/// where any other number would be a frontend guessing at the engine's
/// bound.
pub const MESSAGE_LOG_CAP: usize = 100;

/// How a log line should be presented — a display hint set by whatever
/// engine code produced the line, not derived from the text itself, so
/// frontends don't need to pattern-match message strings to style them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MessageKind {
    #[default]
    Info,
    Loot,
    LevelUp,
    Raid,
    /// Marks where one resolved battle round ends and the next begins. The
    /// battle screen's log pane is continuous, so without this the narration
    /// of six rounds reads as one undifferentiated block.
    Round,
    /// A line that is a *result* of a battle rather than narration of it —
    /// the kill, the XP, the decompile verdict, the jack-out.
    /// `MessageLog::retain_outcomes_since_battle` keeps these alongside
    /// `Loot`, `LevelUp`, `Raid` and `Complete` when a battle ends, which is
    /// what stops the blow-by-blow following the player onto the map.
    Outcome,
    /// A party member landing damage on a hostile group.
    PartyDamage,
    /// A hostile program landing a plain damage-only move.
    EnemyAttack,
    /// A hostile program doing something other than a plain swing: a move
    /// that also inflicts a status condition, the line naming that
    /// condition, or a carrier spending its round on an installed routine
    /// (see `Game::wild_routine_ready`).
    EnemySpecial,
    /// Integrity restored to the *party* — deliberately not to a hostile,
    /// which mends itself under `EnemySpecial` like its other routines,
    /// since a kind is read as whose news a line is and not as what
    /// mechanically happened. Narration rather than a result, so
    /// `retain_outcomes_since_battle` drops it with the rest of the
    /// blow-by-blow when the fight ends.
    Heal,
    /// A job the base set itself has finished — currently a work order
    /// reaching its target. Its own kind rather than `Outcome` or `Loot`
    /// because the colour table is the only thing distinguishing it from
    /// the filing and cancellation lines that share its wording, and
    /// neither of those is green. Kept by
    /// `retain_outcomes_since_battle` for the reason `Raid` is: an order
    /// that lands mid-fight has to survive the prune or the player never
    /// learns it finished.
    Complete,
    /// Why an action the player just asked for was refused — the line
    /// `Game::note_refusal` pushes and `App::refuse` is the only caller of.
    ///
    /// Its own kind rather than `Info` so the colour table can say what it
    /// is, and deliberately *absent* from
    /// `MessageLog::retain_outcomes_since_battle`'s keep-set: a refusal is
    /// news about a keypress, not a result, and has no business following
    /// the player out of a fight.
    Refusal,
}

/// Which of the two things the player is doing produced a line: running the
/// base, or being out in the world. Deliberately a second axis rather than
/// more `MessageKind` variants — kind is read by three consumers that mean
/// different things by it (the colour table, `retain_outcomes_since_battle`'s
/// prune, and `condense`'s notion of line identity), and a raid alert has to
/// stay `MessageKind::Raid` for the first two while still being base news.
///
/// `Field` is the default so that a `log` call which predates the split keeps
/// meaning what it did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MessageSource {
    #[default]
    Field,
    /// Production, construction and the base coming under attack. Keeps
    /// accumulating while the party is underground — the base runs whether
    /// or not anyone is standing in it.
    Base,
}

/// Which of `battle::resolve_attack`'s four bands landed, for the one line
/// that narrates a swing. Stripped down to the band alone — no damage
/// figure, no `battle::FumbleRung` — because the only reader is a per-swing
/// sound cue picking one of a handful of clips; the damage and the rung are
/// already in the line's own `text` for anyone reading that.
///
/// A third axis on `LogLine`, beside `kind` and `source`, rather than a
/// parallel record keyed by line index (a second `BattleTimeline`, say):
/// `Game::battle_log` already returns lines in the raw order
/// `App::advance_reveal` counts through to pace the reveal, so keying the
/// cue to the line it describes costs nothing extra to keep in sync — a
/// parallel structure would have to agree with `MessageLog`'s own drops and
/// drains on every index by hand. `None` for every line that isn't swing
/// narration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwingOutcome {
    Crit,
    Hit,
    Miss,
    Fumble,
}

impl From<AttackOutcome> for SwingOutcome {
    fn from(outcome: AttackOutcome) -> Self {
        match outcome {
            AttackOutcome::Crit { .. } => SwingOutcome::Crit,
            AttackOutcome::Hit { .. } => SwingOutcome::Hit,
            AttackOutcome::Miss => SwingOutcome::Miss,
            AttackOutcome::Fumble(_) => SwingOutcome::Fumble,
        }
    }
}

/// One line as stored. A struct rather than the `(kind, text)` tuple it grew
/// from: with a third field the positional form stops reading, and every
/// consumer already wanted to name what it was reaching for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogLine {
    pub kind: MessageKind,
    pub source: MessageSource,
    pub text: String,
    pub outcome: Option<SwingOutcome>,
}

/// Where a battle's narration begins, as a count of lines ever pushed.
///
/// Deliberately not an index into `MessageLog::lines`: the log drains its
/// oldest entries once past `MESSAGE_LOG_CAP`, so an index would come to
/// point at the wrong line in any battle long enough to overflow it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MessageMark(u64);

#[derive(Resource, Default)]
pub struct MessageLog {
    pub lines: Vec<LogLine>,
    /// Suppresses `retain_outcomes_since_battle` entirely. Set only by
    /// `arena`, and only because the prune exists to keep a *map pane*
    /// readable — an arena run has no pane and no player scrolling it, and
    /// the blow-by-blow the prune drops is the whole point of a report.
    ///
    /// It has to be a flag rather than the arena reading the log more
    /// carefully: the prune deletes the lines outright, and it runs inside
    /// `battle_resolve_round`, so the round that ends a fight is
    /// unreachable from outside no matter when the caller looks.
    pub(crate) keep_battle_narration: bool,
    /// Lines ever pushed, including those since dropped. Marks are minted
    /// from this, and `pushed - dropped == lines.len()` is the invariant
    /// that converts a mark back into an index — every mutation of `lines`
    /// has to keep it.
    pushed: u64,
    /// Lines dropped off the front by the cap. Counted rather than derived
    /// from `pushed - lines.len()`: `retain_outcomes_since_battle` removes
    /// lines from the middle too, which a derived figure would mistake for
    /// front-drops and slide every mark backwards past its own range.
    dropped: u64,
    /// Where the current — or most recently ended — battle's narration
    /// begins. `None` until the first battle: a run that has never fought
    /// has no narration, and defaulting to mark 0 would instead make the
    /// whole log read as one endless battle.
    ///
    /// Deliberately not cleared when a battle ends: the frontend is still
    /// scrolling that battle's results in after the fact and needs the range
    /// to slice. The next `open_battle` replaces it.
    battle_start: Option<MessageMark>,
    /// Where the current round's narration begins. The pane shows one round
    /// at a time, so a resolved round replaces the last rather than piling
    /// on top of it.
    round_start: Option<MessageMark>,
    /// Bumped every time the pane's range resets — a new round or a new
    /// battle — so a frontend pacing the narration can tell it has a fresh
    /// range to scroll rather than comparing text. A per-*battle* counter
    /// is not enough: consecutive rounds are much the same length, so a
    /// reveal that carried its count across one would find the new range
    /// already covered and show it whole.
    generation: u64,
}

impl MessageLog {
    pub fn push(&mut self, line: impl Into<String>) {
        self.push_kind(MessageKind::Info, line);
    }

    pub fn push_kind(&mut self, kind: MessageKind, line: impl Into<String>) {
        self.push_line(kind, MessageSource::Field, None, line);
    }

    /// News from the base rather than from wherever the party is standing.
    pub fn push_base(&mut self, line: impl Into<String>) {
        self.push_line(MessageKind::Info, MessageSource::Base, None, line);
    }

    pub fn push_base_kind(&mut self, kind: MessageKind, line: impl Into<String>) {
        self.push_line(kind, MessageSource::Base, None, line);
    }

    /// Narrates one resolved swing, tagging the line with the band it
    /// landed on — see `SwingOutcome`. Always `MessageSource::Field`: base
    /// production never swings at anything.
    pub fn push_swing(
        &mut self,
        kind: MessageKind,
        outcome: SwingOutcome,
        line: impl Into<String>,
    ) {
        self.push_line(kind, MessageSource::Field, Some(outcome), line);
    }

    fn push_line(
        &mut self,
        kind: MessageKind,
        source: MessageSource,
        outcome: Option<SwingOutcome>,
        text: impl Into<String>,
    ) {
        self.lines.push(LogLine {
            kind,
            source,
            text: text.into(),
            outcome,
        });
        self.pushed += 1;
        if self.lines.len() > MESSAGE_LOG_CAP {
            let excess = self.lines.len() - MESSAGE_LOG_CAP;
            self.lines.drain(0..excess);
            self.dropped += excess as u64;
        }
    }

    pub fn recent(&self, n: usize) -> &[LogLine] {
        let start = self.lines.len().saturating_sub(n);
        &self.lines[start..]
    }

    /// Opens a new battle's narration range at the next line pushed, and
    /// its first round with it.
    pub fn open_battle(&mut self) {
        self.battle_start = Some(MessageMark(self.pushed));
        self.open_round();
    }

    /// Opens a new round's range at the next line pushed. The pane shows one
    /// round at a time, so this is what clears it between them.
    pub fn open_round(&mut self) {
        self.round_start = Some(MessageMark(self.pushed));
        self.generation += 1;
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Where `mark` sits in `lines` right now. Clamped: once the mark has
    /// been dropped past, every line still held is younger than it, so all
    /// of them belong to the range.
    fn index_of(&self, mark: Option<MessageMark>) -> Option<usize> {
        let mark = mark?;
        let start = mark.0.saturating_sub(self.dropped) as usize;
        Some(start.min(self.lines.len()))
    }

    /// The current round's lines, oldest first — what the battle pane shows.
    /// Empty before the run's first battle.
    pub fn since_round(&self) -> &[LogLine] {
        match self.index_of(self.round_start) {
            Some(start) => &self.lines[start..],
            None => &[],
        }
    }

    /// Drops the blow-by-blow from the battle range, keeping what the player
    /// should still be reading once the map is back: the battle's results,
    /// and any world news that landed mid-fight. `Raid` is kept because the
    /// background systems in `systems.rs` and `difficulty.rs` write to this
    /// log directly, so a raid alert can arrive inside a battle's range
    /// without being any part of that battle.
    pub fn retain_outcomes_since_battle(&mut self) {
        if self.keep_battle_narration {
            return;
        }
        let Some(start) = self.index_of(self.battle_start) else {
            return;
        };
        let mut index = 0;
        self.lines.retain(|line| {
            let keep = index < start
                || matches!(
                    line.kind,
                    MessageKind::Outcome
                        | MessageKind::Loot
                        | MessageKind::LevelUp
                        | MessageKind::Raid
                        | MessageKind::Complete
                );
            index += 1;
            keep
        });
        // Restores `pushed - dropped == lines.len()`. Without this the lines
        // just removed would read as front-drops and drag every mark back
        // past its own range.
        self.pushed = self.dropped + self.lines.len() as u64;
        // What survived is the results, and they are what the map's pane
        // scrolls in — so the round range has to cover them, not the point
        // the final round happened to start at.
        self.round_start = Some(MessageMark(self.dropped + start as u64));
    }
}

/// One row of the history screen: a log line, and how many identical lines
/// it stands for. `repeats` is 1 for a line that stands alone, so a consumer
/// never has an uncollapsed path to special-case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogEntry {
    pub kind: MessageKind,
    pub source: MessageSource,
    pub text: String,
    pub repeats: usize,
}

/// How far back a repeated line looks for an entry to fold into, counted in
/// already-emitted entries rather than raw lines. That unit is the whole
/// design: an unbroken run collapses however long it gets, because each copy
/// finds its anchor as the newest entry; two or three cronjobs interleaving
/// their yields each cycle still collapse, because their anchors stay inside
/// the window; and the same warning again three hundred ticks later falls
/// outside it, so two starvation events read as two events.
///
/// Not in `tuning.rs`: that file is how hard the game is, and this is how the
/// screen reads.
pub const CONDENSE_LOOKBACK: usize = 4;

/// Folds repeated lines together for every screen that draws the log: the
/// history screen (`L`), the map's pane and the battle pane.
///
/// Deliberately a view over the stored log rather than a collapse in
/// `MessageLog::push_kind`: storage carries the mark arithmetic that
/// `since_round` and `retain_outcomes_since_battle` slice with, and merging a
/// new round's first line backwards across `open_round` would drop it out of
/// the battle pane's range entirely.
///
/// A view is also what keeps the battle pane's reveal intact. Pacing counts
/// *raw* lines — `App::hidden_log_lines` chops the same figure off the map
/// pane's tail, and `Game::battle_view_at` replays the timeline by it — so
/// the fold has to land after that arithmetic, on the rows about to be
/// drawn. A round that kills seven programs still takes seven beats to
/// scroll in; what it no longer does is say the same sentence seven times.
///
/// Lives here rather than in a frontend because both consumers have to agree
/// on the row count — app-core scrolls the screen (`App::handle_history_key`)
/// while the renderer draws it, so a fold applied only while drawing would
/// leave the highlight indexing rows that no longer exist.
///
/// `kind` is part of the match: the same sentence pushed as `Info` and as
/// `Outcome` is styled differently and means something different. `source` is
/// part of it for the same reason — the two channels are read separately, so
/// identical text from the base and from the field is two events.
pub fn condense(lines: &[LogLine]) -> Vec<LogEntry> {
    let mut entries: Vec<LogEntry> = Vec::new();
    for line in lines {
        let window = entries.len().saturating_sub(CONDENSE_LOOKBACK);
        match entries[window..]
            .iter_mut()
            .find(|e| e.kind == line.kind && e.source == line.source && e.text == line.text)
        {
            Some(entry) => entry.repeats += 1,
            None => entries.push(LogEntry {
                kind: line.kind,
                source: line.source,
                text: line.text.clone(),
                repeats: 1,
            }),
        }
    }
    entries
}

/// How many effects the queue holds before dropping its oldest — a
/// backstop for a frontend that never calls `Game::take_effects`, matching
/// the cap `MessageLog` puts on lines.
pub const EFFECT_QUEUE_CAP: usize = 32;

/// What happened to a structure a raid picked, for frontends that want to
/// show it on the map. `Deflected` covers both no-damage outcomes — the
/// shield network zeroing the damage out, and a cronjob worker fully
/// mitigating it — since neither changes any state a renderer could
/// otherwise observe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectKind {
    Hit,
    Deflected,
    Destroyed,
}

/// A transient "something happened here" cue, in world coordinates so a
/// frontend can keep it pinned to its tile as the player moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisualEffect {
    pub pos: (i32, i32),
    pub kind: EffectKind,
}

/// Effects queued since the last `Game::take_effects`. Deliberately not
/// serialized — a cue mid-flight has nothing to say to a reloaded save.
#[derive(Resource, Default)]
pub struct EffectQueue {
    effects: Vec<VisualEffect>,
}

impl EffectQueue {
    pub fn push(&mut self, pos: (i32, i32), kind: EffectKind) {
        self.effects.push(VisualEffect { pos, kind });
        if self.effects.len() > EFFECT_QUEUE_CAP {
            let excess = self.effects.len() - EFFECT_QUEUE_CAP;
            self.effects.drain(0..excess);
        }
    }

    pub fn take(&mut self) -> Vec<VisualEffect> {
        std::mem::take(&mut self.effects)
    }
}

/// A program walking from one base-space cell to another, for a frontend to
/// animate.
///
/// **Purely cosmetic.** Nothing in the simulation is standing at any of
/// these cells: by the time this is queued the walk has already happened as
/// far as the engine is concerned, and the body is either away on a sortie
/// or home and idle. A frontend that ignores these draws the game exactly as
/// it did before they existed.
///
/// A separate value from `VisualEffect` rather than a fourth `EffectKind`,
/// because that one's whole shape is a *tile* — "something happened here",
/// pinned to one cell — and this is a path over time carrying the glyph to
/// walk along it. The glyph is carried because the walker cannot be looked
/// up: a departing program is `ProgramRole::Sortie` the moment the record is
/// pushed, and every view drops it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitCue {
    pub glyph: char,
    pub color: GlyphColor,
    /// Cell by cell, origin first and destination last. Never empty, and
    /// every consecutive pair are neighbours — a frontend interpolates
    /// between them and would slide across the map if they were not.
    pub path: Vec<(i32, i32)>,
}

/// Walks queued since the last `Game::take_transits` — `EffectQueue`'s
/// counterpart, capped and drained the same way, and deliberately not
/// serialized for the same reason: a body mid-walk has nothing to say to a
/// reloaded save.
#[derive(Resource, Default)]
pub struct TransitQueue {
    cues: Vec<TransitCue>,
}

impl TransitQueue {
    pub(crate) fn push(&mut self, cue: TransitCue) {
        self.cues.push(cue);
        if self.cues.len() > EFFECT_QUEUE_CAP {
            let excess = self.cues.len() - EFFECT_QUEUE_CAP;
            self.cues.drain(0..excess);
        }
    }

    pub fn take(&mut self) -> Vec<TransitCue> {
        std::mem::take(&mut self.cues)
    }
}

#[derive(Resource, Default)]
pub struct GameOver {
    pub reason: Option<String>,
}

/// Feats this run has performed that an achievement might care about but no
/// counter can be polled for. Today that is exactly one thing: bosses that
/// actually died.
///
/// A **per-tick drain queue**, not an accumulator. `Game::award_loot` pushes
/// a species id and does nothing else — no achievement lookup, no reward, no
/// profile write — and `game::achievements::achievement_system` drains it in
/// the same tick, unconditionally, earned or not. So there is still exactly
/// one place that decides what has been earned, and the kill site cannot
/// drift from it. Forget the drain and one kill re-earns forever.
///
/// **Not saved**, and that is only sound because every authored boss trigger
/// names a single species: the trigger is satisfied by the kill itself, and
/// the thing that accumulates is `achievements::Profile`, which is written to
/// disk the moment a rung is earned. A "kill N bosses in one run" trigger
/// would need real saved run state and a `SAVE_FORMAT_VERSION` bump — it is
/// not the small addition it looks like.
#[derive(Resource, Default)]
pub struct RunFeats {
    pub bosses_defeated: Vec<String>,
    /// The species id of every creature killed this tick, for a contract's
    /// `Objective::Terminate`. Written beside `bosses_defeated` in `award_loot`,
    /// so the two records cannot drift about what counts as a kill.
    ///
    /// A **separate field**, drained by `game::contracts::contract_system`
    /// and by nothing else. Each field having exactly one drainer is what
    /// removes any ordering dependency between the two systems: both are
    /// registered unchained, and a shared queue would silently make that
    /// unsound the moment one ate the other's events.
    pub kills: Vec<String>,
    /// Every `contracts::Deed` the player performed this tick, for a
    /// contract's `Objective::Perform`.
    ///
    /// A **third field**, not a widening of `kills`, for the reason the
    /// second one exists: each field having exactly one drainer is what
    /// removes any ordering dependency between the systems that read them,
    /// and a shared queue would make that unsound the moment one ate the
    /// other's events. This one's single drainer is
    /// `game::contracts::contract_system`.
    pub deeds: Vec<crate::contracts::Deed>,
}

/// The hand-compile the player is currently standing over — the item, how
/// much of the batch is left, and how far into the unit in flight they are.
///
/// **Not saved**, on `RunFeats`' precedent above, and sound for a different
/// reason: `Mode::Compiling` is a blocking screen with exactly two exits and
/// no save inside it, so a resource that only exists between them cannot
/// outlive a session. It is inserted by `Game::begin_hand_craft` and removed
/// by `Game::abort_hand_craft`, rather than living at both constructors, so
/// a run that never compiles by hand carries nothing.
#[derive(Resource)]
pub struct HandCraft {
    pub item: crate::items::ItemId,
    /// Batch size, for the screen's `unit / units`.
    pub units: u32,
    /// Units still owed, the one in flight included.
    pub remaining: u32,
    /// Ticks spent inside the unit in flight.
    pub ticks_done: u32,
    /// Whether the unit in flight has had its ingredients taken — what an
    /// abort refunds. Set at the unit's start, cleared when it is granted.
    pub spent: bool,
    /// Units already granted, so the batch is announced once with the
    /// number that actually came out of it.
    pub completed: u32,
    pub careful: bool,
    /// The floor a gear copy off this batch rolls from, or `None` for
    /// anything that stacks in `Inventory` and so spends no quality draw.
    ///
    /// Resolved once, in `Game::begin_hand_craft`, because a `CraftOrder`
    /// is the batch's and not the unit's — `careful` already is — and
    /// because that is the one call that has the recipe in hand.
    pub quality_floor: Option<u8>,
}

/// What `Game::advance_hand_craft` reports back: enough to draw the bar
/// without the screen knowing anything about how the loop is driven.
///
/// `ticks_total` is `Game::hand_craft_ticks`' own answer rather than a
/// second multiplication of the constant, so the figure on screen and the
/// time the loop spends cannot disagree.
pub struct HandCraftProgress {
    pub item: crate::items::ItemId,
    /// 1-based: which unit of the batch is in flight, or the last one once
    /// `finished`.
    pub unit: u32,
    pub units: u32,
    pub ticks_done: u32,
    pub ticks_total: u32,
    /// The batch is over — completed, aborted or interrupted — and the
    /// resource is gone.
    pub finished: bool,
}

/// Which ground conditions this **session** has already described.
///
/// Not saved, on `RunFeats`' precedent above: a reload re-announces a
/// condition's description, and that is cheaper than a save field for
/// flavour text nothing else depends on.
#[derive(Resource, Default)]
pub struct SeenConditions(pub Vec<crate::environment::GroundCondition>);

/// This tick's answer to "what does the base supply, what do its machines
/// draw, and which of them lost the cut" — `game::base::power::ledger`'s
/// result, parked where the systems downstream of it can read it.
///
/// A **per-tick derived cache**, rewritten from scratch by
/// `systems::power_grid_system` at the head of the base chain and read by the
/// three systems behind it. Nothing else recomputes the rule: `ledger` is the
/// single expression of "is this machine dark", and this resource is how the
/// one call it gets per tick reaches its readers.
///
/// **Not saved**, and that is sound because there is nothing in it a load
/// could get wrong: every field is a pure function of the structures standing
/// on the map, which the save does restore, so the first tick after a load
/// recomputes it exactly. Saving it would be storing an answer next to its
/// own question. `MachineStatus` is left out of the save for the same reason
/// and lands back on its `Running` default, so a base that loads over
/// capacity announces itself dark once — which is information the player
/// wants.
///
/// Inserted at both `Game` constructors anyway, the way `RunFeats` is, so a
/// reader that runs before the first tick — the base pane's grid header
/// (Task 5) draws on the frame a load finishes — sees an empty grid rather
/// than a missing resource.
#[derive(Resource, Default)]
pub struct PowerGrid {
    pub supply: u32,
    pub draw: u32,
    pub dark: std::collections::HashSet<Entity>,
}

impl PowerGrid {
    /// Whether `machine` lost this tick's cut. The one question the three
    /// base systems ask of the grid — asked through a method rather than by
    /// reaching into `dark` so no caller is tempted to re-derive the rule
    /// from `supply` and `draw`, which are the base-wide totals and cannot
    /// answer it.
    pub fn is_dark(&self, machine: Entity) -> bool {
        self.dark.contains(&machine)
    }
}

/// One contract the run has taken on, and how far along it is.
///
/// Holds the **whole resolved `ContractDef`**, not an id plus parameters, for
/// the argument `EquippedItem` stores an entire `GearCopy`: forgetting a
/// property must not be expressible, and a contract file edited or deleted
/// mid-run must not strand or silently rewrite one already accepted. A save
/// naming a contract whose file is gone still finishes and still pays.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ActiveContract {
    pub def: crate::contracts::ContractDef,
    /// Counted against `Objective::target()`. Written by
    /// `game::contracts::contract_system` and, for a `Deliver`, by
    /// `Game::deliver_to_contract` — nothing else raises it.
    pub progress: u32,
    pub accepted_tick: u64,
}

/// What the run is holding and what it has finished.
///
/// Saved, unlike `RunFeats` — this *is* the accumulator that a per-tick drain
/// queue feeds. `done` is what keeps a finished non-repeatable contract off
/// the board for the rest of the run.
#[derive(Resource, Default)]
pub struct ActiveContracts {
    pub active: Vec<ActiveContract>,
    pub done: Vec<crate::contracts::ContractId>,
}

/// What the base has been told to hold, front-first.
///
/// One order at a time, worked to completion, then the next — splitting
/// staff across several live orders makes it impossible to read why a
/// machine is idle. An order that has become unfillable is *skipped*
/// rather than blocking the queue behind it, and stays listed so the
/// status screen can say which machine went missing.
#[derive(Resource, Default)]
pub struct WorkOrders(pub Vec<crate::game::base::work_orders::WorkOrder>);

/// How many posts the queue asked for last tick, against how many staff the
/// base had to fill them.
///
/// **Cached rather than derived on demand**, for `Platform`'s radius reason:
/// both figures live inside `schedule_base_labour`, which is `&mut self` and
/// has side effects — `settle_orders` drops a completed order and announces
/// a stall — so a screen cannot ask for them by calling it. It is written
/// once a tick, before the cut to `staff.len()`, and read from `&self`.
///
/// Not saved: it is rewritten on the next tick either way, and a figure
/// restored from a save would describe a base that has since changed.
#[derive(Resource, Default, Clone, Copy)]
pub struct LabourDemand {
    /// Posts the queue, the standing jobs and the dig plan asked for
    /// together — what the scheduler wanted *before* it cut the list to the
    /// bodies it had.
    pub wanted: usize,
    pub staff: usize,
}

impl LabourDemand {
    /// How many bodies short the base is, and the one definition of it — the
    /// screen draws its header off this rather than subtracting the two
    /// fields itself.
    pub fn shortfall(&self) -> usize {
        self.wanted.saturating_sub(self.staff)
    }
}

/// Achievements earned since the last time anyone wrote `profile.ron`.
///
/// The engine decides what has been earned and app-core owns the path, so
/// this is the handoff between them: `achievement_system` pushes, app-core
/// drains after each tick and writes. It accumulates rather than being
/// per-tick like `RunFeats`, because a failed or skipped drain must not lose
/// the earn.
///
/// **Two reasons the profile can be dirty, one resource.** A notification
/// with `Repeat::OnceEver` latches in the profile too, so `seen` accumulates
/// beside `earned` rather than getting a second resource of its own — two
/// resources meaning "the profile changed" is two things app-core has to
/// remember to drain, and the one that drifts is the one nobody runs.
#[derive(Resource, Default)]
pub struct PendingProfileWrites {
    pub earned: Vec<crate::achievements::AchievementId>,
    pub seen: Vec<crate::notifications::NotificationKind>,
}

impl PendingProfileWrites {
    pub fn is_empty(&self) -> bool {
        self.earned.is_empty() && self.seen.is_empty()
    }
}

/// Notifications waiting to take the screen, oldest first.
///
/// A **session-only queue**, `RunFeats`' precedent and for a stronger
/// reason: a notification is news about a moment, and a player reloading has
/// already left the moment. Nothing here is saved, so `SAVE_FORMAT_VERSION`
/// is untouched.
///
/// It holds **resolved** `Notification` values rather than ids —
/// `ActiveContract`'s rule and `Sortie`'s — so a `.ron` file edited or
/// deleted between the push and the draw cannot strand or silently rewrite
/// something already queued.
///
/// `Game::notify` is the only thing that pushes and `Game::take_notification`
/// the only thing that pops. A frontend that draws none must still call the
/// drain or the queue grows for the life of the run, which is
/// `take_effects`' contract.
#[derive(Resource, Default)]
pub struct Notifications {
    pending: std::collections::VecDeque<crate::notifications::Notification>,
}

impl Notifications {
    pub fn push(&mut self, notification: crate::notifications::Notification) {
        self.pending.push_back(notification);
    }

    pub fn pop(&mut self) -> Option<crate::notifications::Notification> {
        self.pending.pop_front()
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

/// Battle records waiting for app-core to write them out, and the one bool
/// that decides whether any are built at all.
///
/// The `PendingProfileWrites` shape: the engine knows what happened, app-core
/// owns the path and does the IO. Deliberately **not saved** — this is dev
/// output, not run state, so `SAVE_FORMAT_VERSION` is untouched.
///
/// `on` is set once at startup from `FERAL_DEV_LOG` and never read from the
/// environment again; `Game::record` tests it before doing anything else.
#[derive(Resource, Default)]
pub struct BattleTelemetry {
    pub on: bool,
    pub(crate) records: Vec<crate::telemetry::Record>,
    /// The fight the emission seams are currently tagging records with.
    pub(crate) fight: u64,
    /// Counts up within the process, so many fights in one session separate
    /// cleanly in the file.
    pub(crate) next_fight: u64,
}

/// The single player-controlled entity. Kept as a resource (rather than
/// re-queried with a `With<Player>` filter each time) since lookups happen
/// on almost every action.
#[derive(Resource, Clone, Copy)]
pub struct PlayerEntity(pub Entity);

/// The single `components::BaseAnchor` entity. Kept as a resource for the
/// same reason `PlayerEntity` is: `Game::anchor_position` is `&self` and
/// reads far more often than the anchor moves, so a cached handle is what
/// lets it look the position up without a `World`-mutating query.
#[derive(Resource, Clone, Copy)]
pub struct AnchorEntity(pub Entity);

/// The stack lair a fight was roused from: which frame it is, and the
/// program the lair was built around.
///
/// Recorded by `Game::rouse_lair` as the fight opens rather than derived at
/// teardown, because the two moments are not the same one and a Forgiving
/// reboot can land between them: the guardian falls, its escort flatlines
/// the player, and `difficulty::death_handling_system` surfaces the party
/// inside the trailing `tick` — so by the time the fight ends there is no
/// `Locale::Stack` left to read a frame off, and the stack would survive
/// with a cleared lair as a dud the player could re-enter forever.
///
/// **Naming the guardian is what keeps an escort's death out of it.**
/// `Game::award_loot` fires for every hostile in the game that goes down,
/// and past the first frame most of a lair's pack is escort rather than
/// guardian (`spawn_pack`). A lair spent by "something died while the party
/// stood on it" was spent by cutting one escort down and jacking out, which
/// collapsed the whole stack with the guardian still standing in it.
#[derive(Clone, Copy)]
pub(crate) struct LairFight {
    /// The frame the lair is in — `Game::collapse_stack` needs its entrance,
    /// and `FrameMemory` is keyed by that and the depth.
    pub(crate) pos: crate::game::stack::StackPos,
    /// The program the lair was built around: the biome's boss, or the
    /// toughest ordinary program where it fields none (`pick_lair_species`).
    pub(crate) guardian: Entity,
}

/// Active turn-based encounter between the player's party and one or more
/// wild species groups (see `battle::EnemyGroup`), partitioned out of the
/// pack `Game::gather_pack` collected. Groups 0 and 1 are engaged and can
/// melee; anything further back needs a move flagged `ranged` to reach the
/// party. Removing this resource ends the battle.
#[derive(Resource)]
pub struct BattleState {
    pub player: Entity,
    pub groups: Vec<EnemyGroup>,
    pub round: u32,
    /// This round's chosen action per party slot — index 0 is the player,
    /// 1.. are party members in roster order. `None` means "not yet
    /// chosen"; a round resolves only once every slot is `Some`.
    pub planned: Vec<Option<BattleAction>>,
    pub finished: bool,
    pub player_won: bool,
    /// How many decompiles have already been attempted against each target,
    /// which `taming::capture_chance` reads to make each attempt raise the
    /// odds of the next one against that same program.
    ///
    /// It lives here, on the battle, rather than as a component on the
    /// creature or a saved resource, because that is exactly the lifetime
    /// the mechanic wants: removing this resource ends the battle and takes
    /// the counter with it, so a program the party fled from is met fresh
    /// and no teardown code has to remember to clear anything. Battles are
    /// never serialised, so this needs no `SAVE_FORMAT_VERSION` bump.
    ///
    /// Keying by `Entity` is safe against `finish_member`'s mid-round
    /// despawns: an `Entity` carries a generation, so a recycled index is a
    /// different `Entity` and a stale key can never alias a live target.
    pub decompile_attempts: HashMap<Entity, u32>,
    /// What the fight has paid out so far, held back until it ends — see
    /// `BattleRewards`.
    pub rewards: BattleRewards,
    /// The stack lair this fight was roused from, if it was one — see
    /// `LairFight`. `Game::end_battle` reads it back to decide whether the
    /// stack comes down on the way out.
    ///
    /// Same lifetime argument as `decompile_attempts` and `rewards`, and the
    /// same payoff: battles are never serialised, so this needs no
    /// `SAVE_FORMAT_VERSION` bump.
    pub(crate) lair: Option<LairFight>,
    /// Each group's members as they stood when this round's plan was made,
    /// in the order the plan indexes them.
    ///
    /// A `BattleAction::Attack { group }` stores a plain index, but an
    /// emptied group is dropped from `groups` mid-round (`remove_member`),
    /// re-lettering everything behind it — so by the time a slower party
    /// member acts, the index it planned against may name a *different*
    /// group. This is what lets `Game::retarget` answer the question the
    /// plan actually asked: it looks up the member set the player aimed at
    /// and finds where that group is standing now, or `None` if it has
    /// fallen. Members only ever leave a group, so any survivor identifies
    /// it.
    ///
    /// Refreshed at the top of every round rather than tracked
    /// incrementally: derived from the live groups, it cannot drift out of
    /// step with them. Same lifetime argument as `decompile_attempts` —
    /// battles are never serialised, so this needs no `SAVE_FORMAT_VERSION`
    /// bump.
    pub round_targets: Vec<Vec<Entity>>,
    /// Whether the hostiles outweighed the party at the opening round, by
    /// summed `Stats::power()`.
    ///
    /// **A snapshot, because the question is unanswerable later.** It is what
    /// `Game::form_victory_memories` reads to decide whether a win was
    /// `hard_won`, and by the time a fight is won its hostiles are dead by
    /// definition — there is nothing left at teardown to weigh. Taken in
    /// `begin_battle`, so `arena`'s authored compositions are judged by the
    /// same rule the world's packs are.
    ///
    /// It stores the **verdict** rather than the two sums: nothing else has a
    /// use for the figures, and a stored pair invites a second reader deriving
    /// its own threshold from them. Same lifetime argument as
    /// `decompile_attempts` and `lair` — battles are never serialised, so this
    /// needs no `SAVE_FORMAT_VERSION` bump.
    pub(crate) outmatched: bool,
}

/// A fight's payout, accumulated per kill and announced once, by
/// `Game::settle_rewards` at the top of `end_battle`.
///
/// **Only the announcement waits.** Every reward here has already been
/// granted by the time it is recorded: a level-up full-heals inside
/// `progression::add_xp` and the killing blow is usually the level, so
/// deferring the award itself would move fight outcomes and every arena and
/// `balance_sim` number with them. What this removes is a loot line and an
/// XP line landing between every pair of blows.
///
/// It lives on `BattleState` for the lifetime `decompile_attempts` wants and
/// for the same two payoffs: battles are never serialised, so it costs no
/// `SAVE_FORMAT_VERSION` bump, and it is not a `Resource` of its own, so it
/// cannot shift bevy's query iteration order under an unrelated test.
/// `Game::end_battle` takes it out before dropping the resource, which is
/// what makes a win and a jack-out pay through one path.
#[derive(Default)]
pub struct BattleRewards {
    /// Every copy that dropped, merged by copy. Held in the order things
    /// fell and sorted by `settle_rewards` on the way out, so the same haul
    /// reads the same way however the kills happened to order it.
    ///
    /// `(GearCopy, u32)` is `BuybackLedger`'s shape and carries its argument:
    /// two copies that differ are not two of a thing, so an Overclocked
    /// weapon is tallied apart from a plain one rather than summed into it.
    /// A material is a `GearCopy::plain`, which is what lets salvage and gear
    /// share one row format.
    pub drops: Vec<(crate::items::GearCopy, u32)>,
    pub player: XpTally,
    /// Keyed by entity rather than by name: a companion that died winning the
    /// fight is still in `Party` when `settle_rewards` runs and is gone by
    /// the time `end_battle` returns, so the name is resolved at flush.
    pub companions: Vec<(Entity, XpTally)>,
    /// The most recent failed decompile, as the line the pane already showed.
    ///
    /// The odd one out here — nothing was granted, and what waits is the
    /// *only* copy rather than a tally of copies already announced. It is
    /// here anyway because the shape is the same one: a refusal repeats once
    /// per catalyst spent and is narration while the fight is on (the pane
    /// shows every kind), but `retain_outcomes_since_battle` keeps whole
    /// kinds, so six attempts left six near-identical refusals on the
    /// results screen. Only the newest still says anything — it is the one
    /// that knows whether the fraying has hit `DECOMPILE_ATTEMPT_BONUS_CAP`.
    ///
    /// The finished string rather than the attempt count, so the live line
    /// and the summary cannot word the same verdict differently. Cleared by
    /// a capture: the breach line is pushed live and survives the prune in
    /// place, so a refusal flushed afterwards would sit *below* it and read
    /// as a failure that came after the program was already yours.
    pub decompile_verdict: Option<String>,
}

/// One fighter's experience over a whole fight.
///
/// The deltas sum because `progression::LevelGain::stat_rows` recovers each
/// row's "before" by subtracting the delta from the stats as they stand *now*
/// — hand it the whole fight's delta and it reports the whole fight's range,
/// with nothing snapshotted at the start of the battle.
#[derive(Clone, Default)]
pub struct XpTally {
    pub xp: u32,
    pub gain: crate::progression::LevelGain,
    /// Perk Points and Decompiler skill are the player's alone, and neither
    /// is anything `add_xp` computes — see `Game::award_player_xp`.
    pub perk_points: u32,
    pub decompiler: i32,
}

impl XpTally {
    /// Folds one kill's experience into a fight's running total.
    pub fn absorb(&mut self, other: &XpTally) {
        self.xp += other.xp;
        self.gain.absorb(other.gain);
        self.perk_points += other.perk_points;
        self.decompiler += other.decompiler;
    }

    pub fn is_empty(&self) -> bool {
        self.xp == 0 && self.gain.levels == 0
    }
}

/// What the battle roster looked like at one point in the current round's
/// narration — see `BattleTimeline`.
pub struct RosterFrame {
    /// `Game::battle_log().len()` when this was taken. A count rather than
    /// an index, because the trailing `tick` in `battle_resolve_round` lets
    /// background systems push lines nothing took a frame for; the lookup
    /// takes the last frame at or under the revealed count, so an unframed
    /// line simply holds the previous frame on screen.
    pub lines: usize,
    pub groups: Vec<crate::views::EnemyGroupView>,
    pub party: Vec<crate::views::PartySlotView>,
}

/// The current round's roster, recorded once per narrated line.
///
/// `battle_resolve_round` resolves the whole round in one call while a
/// frontend scrolls the narration in over a second or two, so without this
/// every HP bar has already dropped to its end-of-round value before the
/// first line is legible. `Game::battle_view_at` reads it to answer what
/// the roster looked like when a given line landed.
///
/// It stores *rendered rows* rather than entities and HP numbers, because
/// two things about a round are not recoverable from a dead entity:
/// `finish_member` despawns a victim mid-round, and a group emptied
/// mid-round is dropped from `BattleState::groups`, which re-letters every
/// group behind it. Rows make deaths, counts, letters and decompile odds
/// all rewind together for free.
///
/// Transient presentation state, deliberately not saved — the same
/// category as app-core's reveal counter. A loaded game resumes with an
/// empty timeline, which reads as "nothing pending" rather than as a
/// rewind to somewhere the round never was.
#[derive(Resource, Default)]
pub struct BattleTimeline {
    pub frames: Vec<RosterFrame>,
    /// The whole roster as the fight ended, so the battle screen can keep
    /// drawing itself while the results scroll into its own log pane.
    /// `None` before the first fight of a run.
    ///
    /// Captured at the top of `end_battle`, before `dissolve_tamed_program`
    /// drops the dead out of `Party` and despawns them — a companion that
    /// died winning the fight is the single thing the screen most needs to
    /// say, and one line later there is nothing left to read it off.
    ///
    /// The hostile half comes out **empty on a win**, because
    /// `finish_member` only calls `end_battle` once `remove_member` has
    /// emptied the last group. That is the intended reading: the pane
    /// clearing is what winning looks like. A jack-out leaves it populated,
    /// which is equally the point — you can see what you ran from.
    pub closing: Option<ClosingRoster>,
}

/// The battle screen's state at the moment a fight ended — see
/// `BattleTimeline::closing`.
pub struct ClosingRoster {
    pub groups: Vec<crate::views::EnemyGroupView>,
    pub party: Vec<crate::views::PartySlotView>,
    pub round: u32,
    pub player_decompiler: i32,
}

impl BattleTimeline {
    /// The roster as of `revealed` narrated lines, or `None` when no frame
    /// covers that far back — an empty timeline, or a count of zero, which
    /// is the frame before the round header itself.
    pub fn frame_at(&self, revealed: usize) -> Option<&RosterFrame> {
        self.frames.iter().rev().find(|f| f.lines <= revealed)
    }
}

/// The player's active battle party: up to `MAX_PARTY_SIZE` tamed programs
/// that fight alongside them and can be commanded to attack during an
/// intrusion. Membership is mutually exclusive with an active cronjob
/// `Task` on the same entity — a program is either working a structure or
/// fighting beside the player, never both at once.
///
/// The order is the battle line and is mechanically meaningful: the player
/// is aggro slot 0 and members follow in this order, so a front member draws
/// more fire (see `battle::slot_aggro_weight`). `Game::move_party_member` is
/// the only thing that reorders it.
#[derive(Resource, Default, Clone)]
pub struct Party(pub Vec<Entity>);

/// The tamed program currently equipped as the player's weapon, if any.
///
/// Deliberately not a field on `components::Equipment`: that slot holds an
/// `EquippedItem` — an `ItemId` plus the level and fusion tier captured at
/// equip time — and an entity is none of those things. The bonus it lends
/// is computed live by `Game::wielded_stat_bonus` rather than baked into
/// the player's `Stats`, because a program can be sold, extracted, fused
/// away or killed, and a bonus welded in by an equip that can never be
/// matched by an unequip is permanent free stats with no record of where
/// they came from.
///
/// Read through `Game::wielded_program`, never directly: that accessor
/// drops an entity that no longer exists, which is what makes every
/// destruction path correct without knowing this feature exists.
#[derive(Resource, Default, Clone)]
pub struct WieldedProgram(pub Option<Entity>);

/// Which way along the battle line `Game::move_party_member` shifts a
/// member — toward the player (`Forward`, drawing more fire) or away from
/// them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlotShift {
    Forward,
    Back,
}

/// What each trading post has bought off the player and will sell back to
/// them — see `Game::buyback_options` and `Game::buy_back`.
///
/// Keyed by `(structure kind, tile)` rather than by `Entity` because a shelf
/// has to outlive the building standing on it: a raid that levels the Market
/// must not erase what the player sold it, and the tile is the only identity
/// a rebuilt structure shares with the one it replaced. Rebuilding on the
/// same footprint therefore reopens the same store, and two Markets in one
/// zone keep separate shelves. The kind is part of the key so a different
/// structure raised on a dead trader's tile inherits nothing.
///
/// Entries for tiles that no longer hold a trader are kept, not pruned —
/// that is the whole point — and are bounded by the tiles built on in one
/// zone before `Game::enter_next_zone` clears the lot.
///
/// `BTreeMap` rather than `HashMap` so save bytes don't depend on hash
/// order; the inner `Vec` stays in insertion order, which is player-driven
/// and gives the trade screen a stable row order.
///
/// A shelf row is `(copy, qty)`, keyed on the whole `items::GearCopy` and
/// not on the item. That is not decoration: a shelf keyed on the item alone
/// would hand a mis-sold T3 back as an ordinary copy and silently delete
/// eight base copies of work — and now that a copy also carries a rare tier,
/// it would hand back an ordinary Arc Lance for the Bare-Metal one that was
/// just sold by mistake. The unit price is deliberately the same at every
/// tier (`Game::item_value` is untouched), so being able to buy the *same
/// copy* back is the only thing that makes a mis-sale recoverable.
/// See `components::GearCopies`.
#[derive(Resource, Default, Clone)]
pub struct BuybackLedger(pub BTreeMap<ShelfKey, Vec<(GearCopy, u32)>>);

/// Which of the visiting caravan's own rows it has already sold.
///
/// **Keyed by visit index, which is what makes it self-clearing.** When the
/// schedule's index moves on, `visit` no longer matches and the set reads as
/// empty — so there is no reset call anywhere and no way for a stale entry to
/// make next month's trader arrive already sold out.
///
/// Deliberately **not** a `BuybackLedger`: what the player sells a caravan is
/// gone, exactly as at a Stack market. A trader that would sell it back is a
/// shop the party can walk to, and a caravan's whole shape is that it cannot
/// be walked back to.
///
/// Survives a breach, along with the wagon it remembers. It was wiped by
/// name in `Game::enter_next_zone` while a breach rebuilt the world, since
/// a caravan's journey is defined against an anchor tile in a sector that
/// was about to stop existing — the sector does not stop existing now.
#[derive(Resource, Default, Clone, Debug, PartialEq, Eq)]
pub struct CaravanMemory {
    /// The visit this memory is about, or `None` before any trader has been
    /// walked in this sector.
    ///
    /// An `Option` rather than a bare `u64` because it carries **two** facts
    /// and zero is a real visit index: which visit `bought` belongs to, and
    /// that this visit has already been walked at all. The second is what
    /// stops a trader that gave up on the way in from being spawned again on
    /// the next tick, and again for the rest of its window.
    pub visit: Option<u64>,
    pub bought: BTreeSet<usize>,
}

/// Which shelf: the trader's kind and the tile it stands on — see
/// `BuybackLedger` for why those two and not an `Entity`.
pub type ShelfKey = (StructureId, (i32, i32));

/// Which zone sector the player is currently breached into. Starts at 1
/// (the sector the run begins in); breaching a zone portal increments it.
/// Deeper zones regenerate their terrain from a different seed and spawn
/// wild programs with stats scaled by `stat_multiplier` — there's no way
/// back down once you've breached forward.
#[derive(Resource, Clone, Copy, Serialize, Deserialize)]
pub struct ZoneLevel(pub u32);

impl Default for ZoneLevel {
    fn default() -> Self {
        ZoneLevel(1)
    }
}

impl ZoneLevel {
    /// Flat stat multiplier applied to wild programs spawned in this zone:
    /// rises by `ZONE_STAT_STEP` per zone level (level 1 = x1, level 2 = x2,
    /// level 3 = x3, ...).
    ///
    /// **Linear, and that is the whole of what keeps a deep zone reachable.**
    /// It used to double. The player's own offence rises by `ATK_PER_LEVEL`
    /// per level and a flat point or two per item, so a doubling enemy curve
    /// is a geometric quantity racing a linear one — a race the geometric
    /// side always eventually wins, whatever the coefficients. Damage is
    /// a positive expected value against any mitigation, so "eventually wins"
    /// means every swing lands on the floor and no amount of levelling, gear
    /// or roster moves it. Measured before the change: a zone-3 Stack
    /// guardian was unbeatable at level 90 in the best gear in the game.
    ///
    /// Linear does not remove difficulty, it makes it *fundable*: the levels
    /// needed to keep pace then grow by a roughly constant amount per zone
    /// forever, instead of doubling per zone until no reachable level is
    /// enough. `GEAR_LEVEL_STEP` is matched to this for the same reason it
    /// was matched to the old base.
    pub fn stat_multiplier(self) -> i32 {
        1 + crate::tuning::ZONE_STAT_STEP * (self.0 as i32 - 1)
    }

    /// `stat` moved from tier `from` to `from + 1` — the step
    /// `Game::refactor_companion`'s zone bump applies.
    ///
    /// Applies the step rather than returning it, because on a linear curve
    /// the step is a *ratio* (tier 2 to 3 is 3/2) and an `i32` multiplier
    /// cannot hold one: it truncated to 1, silently making a Recompile
    /// Kernel's zone bump a no-op from tier 2 up. Multiplying before
    /// dividing keeps the arithmetic exact without reaching for a float.
    ///
    /// Derived from `stat_multiplier` rather than reaching for
    /// `ZONE_STAT_STEP` directly, because "one zone tier" has to keep
    /// meaning the same thing on both sides. A Recompile Kernel is sold to
    /// the player as catching a companion up with the ground the spawner
    /// scales to, so a bump that kept its own copy of the curve would be
    /// paying for a tier the spawner no longer grants.
    pub fn raised_a_tier(stat: i32, from: u32) -> i32 {
        stat * ZoneLevel(from + 1).stat_multiplier() / ZoneLevel(from).stat_multiplier()
    }
}

/// Which of the three spaces the player is standing in — the zone surface,
/// down inside the Stack, or out of phase in base space — and where in it.
///
/// The Stack coordinates live *here* rather than on the player's
/// `Position` component, and that is the load-bearing decision of the whole
/// Stack layer. `Position` is the shared coordinate space that structures,
/// wild programs, nests, cronjob targets, raid pathing, the build radius and
/// `Game::view_entities` all live in. Moving the player into Stack
/// coordinates through it would put them on a surface tile that means
/// something else entirely, and every one of those systems would quietly
/// misbehave.
///
/// So while underground the player's `Position` stays pinned to `entrance`
/// — the surface tile they walked in through. Nothing on the surface has to
/// know the Stack exists, and the consequences are the right ones for
/// free: the base is where it was left, cronjobs keep paying out, and a raid
/// can land while the player is four frames down.
///
/// `Base` is the same arrangement pointed the other way, and the reason
/// `Game::require_surface` had to be split: "not in the Stack" and "on the
/// surface proper" were one condition while there were two locales, and
/// eleven guarded actions had never had to say which they meant. See
/// `docs/seams.md`.
#[derive(Resource, Clone, Copy, Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Locale {
    #[default]
    Surface,
    Stack {
        /// 1 at the first frame below the surface, counting up as you
        /// descend. Part of the `stack::FrameSpec` the frame regenerates
        /// from.
        depth: u32,
        /// How many frames this stack runs before it bottoms out. Carried
        /// rather than recomputed from `entrance` because it is also part
        /// of the frame spec — the bottom frame is generated without a way
        /// down — and a stack that changed length underneath the party
        /// would strand them.
        frames: u32,
        x: i32,
        y: i32,
        facing: Dir,
        /// The surface tile of the entrance walked in through — where the
        /// player's `Position` is pinned, and where they pop out on
        /// climbing back up from depth 1.
        entrance: (i32, i32),
    },
    /// Out of phase, inside the base's own pocket dimension, and where in
    /// it.
    ///
    /// The same trick as `Stack`, for the same reason: the coordinates live
    /// here and the player's `Position` stays pinned to the anchor tile
    /// they stepped through, so nothing on the zone surface has to know
    /// base space exists.
    ///
    /// No `entrance` beside them, unlike `Stack`. There is exactly one door
    /// per zone, auto-placed at the zone's spawn point, so the way back out
    /// is a query for it rather than a field that could disagree with one.
    Base { x: i32, y: i32 },
}

/// How loud the party has been in the stack they are currently in.
///
/// A resource rather than a field on the `Locale::Stack` variant, which is
/// where it looks like it belongs. `Game::descend_to` and `Game::ascend_to`
/// each *construct* a fresh variant rather than mutating the live one, so a
/// field there would be silently zeroed on every frame change — exactly when
/// Trace is supposed to be accumulating. As a resource it survives frame
/// changes for free and resets in one place, `Game::clear_stack`, which is
/// already the single door out of the Stack that even `use_symlink` goes
/// through rather than around.
///
/// `u32` rather than a float: bands compare exactly, save bytes are exact,
/// and a long dive accumulates no rounding error.
///
/// Saved. Without persistence, saving mid-dive would be a free Trace reset.
#[derive(Resource, Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trace(pub u32);

/// The next `components::ProgramId` to hand out. Advanced by
/// `Game::roster_parts`, which is the only thing that mints one.
///
/// **No `Default`.** A derived default of `0` would hand the first program
/// the unassigned sentinel, which is the one value that must never belong to
/// a real program — `START` is 1 and `Game::new` inserts it explicitly.
///
/// Saved, because an id that is reused after a reload names two different
/// programs. `Game::load` sets it past the highest id in the file rather
/// than trusting the saved counter alone: a hand-edited or savetool-packed
/// save can carry ids the counter has never seen.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct NextProgramId(pub u32);

impl NextProgramId {
    pub const START: NextProgramId = NextProgramId(1);
}

/// How many times the party has been goaded into speech (`Game::taunt`),
/// which is what picks the next line so repeated presses cycle rather than
/// repeat.
///
/// **A counter and not a `GameRng` draw**, deliberately: a cosmetic key a
/// player might press twenty times in a fight is the worst possible place
/// to advance the shared stream, since every later roll in the run would
/// shift with it.
///
/// **Not saved, and not inserted by either constructor** — `Game::taunt`
/// defaults it in place, the way `ProfileRewardsPaid` exists only once
/// something has happened. A resource is persisted by being an explicit
/// field in `save.rs`, so leaving it out of that struct is the whole of
/// what keeps it transient, and there is nothing here for a
/// `SAVE_FORMAT_VERSION` bump to be about.
#[derive(Resource, Clone, Copy, Default, Debug)]
pub struct TauntCount(pub u32);

/// The four named readings of `Trace`, and the only form the player ever
/// sees it in — a threat readout rather than a progress bar, since a visible
/// integer invites playing to the threshold instead of to the risk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceBand {
    Quiet,
    Noticed,
    Traced,
    Hunted,
}

impl TraceBand {
    pub fn label(self) -> &'static str {
        match self {
            TraceBand::Quiet => "Quiet",
            TraceBand::Noticed => "Noticed",
            TraceBand::Traced => "Traced",
            TraceBand::Hunted => "Hunted",
        }
    }

    /// Indexes the per-band multiplier tables in `tuning` — `TRACE_*_MULT`
    /// are all `[_; 4]` in this order, so a band added here without a column
    /// added there is a compile error rather than a silent wrong lookup.
    pub(crate) fn index(self) -> usize {
        match self {
            TraceBand::Quiet => 0,
            TraceBand::Noticed => 1,
            TraceBand::Traced => 2,
            TraceBand::Hunted => 3,
        }
    }
}

/// Which frame of which stack a `FrameMemory` belongs to.
///
/// Keyed by the link's surface tile rather than by anything about the
/// frame, because that tile is what makes a stack itself — it is already
/// half of `stack::FrameSpec`. Two links in a sector therefore keep
/// separate maps of their separate depth-3s.
pub type FrameKey = ((i32, i32), u32);

/// What the party learned about one Stack frame by walking it.
///
/// This is the only Stack state that is saved rather than regenerated.
/// The frame itself is a pure function of its `stack::FrameSpec`, but what
/// the player has *seen* of it is not — that is the run's history, and
/// losing it on load would hand back a blank map of a frame already walked.
///
/// `BTreeSet` rather than `HashSet` so the encoded save bytes don't depend
/// on hash order.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct FrameMemory {
    /// Every cell the party has had in view — see `view_cone`, which
    /// both this and the first-person view are filled from.
    pub seen: BTreeSet<(i32, i32)>,
    /// Cells whose cache has been emptied.
    pub looted: BTreeSet<(i32, i32)>,
    /// Sealed doors that have been opened, which stay open.
    pub opened: BTreeSet<(i32, i32)>,
    /// Whether this frame's lair has been cleared. Only ever true on the
    /// bottom frame of a stack, which is the only frame that has one.
    pub cleared: bool,
    /// Where the party was jumped. Kept for the map alone: a corridor that
    /// has cost you something is worth marking, and it is the one landmark
    /// the frame's own layout can't tell you about.
    pub fights: BTreeSet<(i32, i32)>,
    /// Breakpoints already jacked into. A used one has nothing left to show
    /// — the frame it maps is already mapped — so both views drop it back to
    /// plain floor, exactly as an emptied cache stops being advertised.
    ///
    /// A set rather than the `bool` `cleared` uses, even though
    /// `STACK_BREAKPOINTS_PER_FRAME` is 1: raising that constant is a
    /// plausible outcome of playtest, and a `bool` would silently spend every
    /// breakpoint in the frame the moment one was used.
    ///
    /// `#[serde(default)]` so the field-named RON that `dev-saves/`
    /// templates are written in keeps parsing without re-capture — the
    /// positional bincode save is what the version bump is for.
    #[serde(default)]
    pub jacked: BTreeSet<(i32, i32)>,
    /// Orphaned processes already adopted. The dead end they were sitting
    /// in has nothing left in it, so both views drop it back to plain
    /// floor — the same argument as `jacked` and `looted`.
    ///
    /// A set for the same reason `jacked` is one, and `#[serde(default)]`
    /// for the same reason too: the field-named RON that `dev-saves/`
    /// templates are written in keeps parsing without re-capture, and the
    /// positional bincode save is what the version bump is for.
    #[serde(default)]
    pub adopted: BTreeSet<(i32, i32)>,
    /// Rows of this frame's market that have already been bought, by their
    /// index in `Game::market_offers` — which is derived from the frame
    /// spec and so hands back the same shelf in the same order after a
    /// save and load.
    ///
    /// Keyed by index rather than by cell, unlike every set above, because
    /// a market is spent a row at a time: the frame has one stall and the
    /// question is which of its seven rows are gone. A market with every
    /// row bought reads as plain floor in both views, exactly as an emptied
    /// cache does, and that is the whole of what makes a trader down here
    /// ephemeral — see `Game::market_live`.
    ///
    /// `#[serde(default)]` so a save written before markets existed loads
    /// with an empty shelf history rather than needing a
    /// `SAVE_FORMAT_VERSION` bump: the payload is field-named RON, so an
    /// added field costs nothing.
    #[serde(default)]
    pub bought: BTreeSet<usize>,
}

/// Everything the party has learned about every Stack frame at this tier.
///
/// **Not** self-clearing: it is wiped by name in `Game::enter_next_zone`,
/// and the reason changed when the world became persistent. An entrance
/// used to be despawned at a breach, so a stale entry would draw the last
/// sector's walked corridors onto a fresh link that happened to land on a
/// matching tile. Entrances survive now — but `FrameSpec` folds in the
/// tier, so the frame behind a surviving entrance is re-carved, and every
/// record here describes a frame that no longer exists.
#[derive(Resource, Clone, Default, Debug, Serialize, Deserialize)]
pub struct StackMemory(pub BTreeMap<FrameKey, FrameMemory>);

/// Which world chunks have had their wild population placed — see
/// `Game::ensure_local_population`. A chunk in here is one the sector has
/// already stocked; a chunk absent from it is ground nothing has ever lived
/// on, and will be stocked the first time the player comes within a chunk of
/// it.
///
/// This is what makes the wild population a property of *place* rather than
/// a record of where the player has stood. The map is unbounded and
/// generated a chunk at a time (see `world::WorldMap::ensure_chunk`), so
/// there is no finite area a one-time seed could cover; population has to
/// follow terrain and arrive on demand.
///
/// A `BTreeSet` rather than a `HashSet` for the reason `Stock` keys by
/// `BTreeMap`: this is serialized, and a hash set would make the save
/// encoding differ between runs holding identical state.
///
/// Wiped **by name** in `Game::enter_next_zone`, and that line is the
/// mechanism rather than a leftover: clearing the marks sends
/// `Game::ensure_local_population` back over ground it has already covered
/// to re-stock it at the new tier, which is the whole visible effect of a
/// breach now that the map is never rebuilt.
#[derive(Resource, Default, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PopulatedChunks(pub BTreeSet<(i32, i32)>);

/// Every settlement the party has reached, keyed by its region.
///
/// **Keyed by `SettlementKey`, not by `Entity`** — `party_slot`'s reason,
/// one level out: entity ids are not stable across a save, and a region's
/// coordinates are the one name for this place that cannot drift.
///
/// **Stores the whole resolved `SettlementDef`**, following `ActiveContract`
/// and `SortieSave`. A catalogue file edited or deleted mid-run must not
/// strand or rewrite a town the party has already walked to and, from Phase
/// 4, earned standing with — the derivation says which entry *would* stand
/// in a region, and this says which one does.
///
/// A `BTreeMap` rather than a `HashMap` for the reason `Stock` keys by one:
/// this is serialized, and a hash map would make the save encoding differ
/// between runs holding identical state.
#[derive(Resource, Default, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Settlements(pub BTreeMap<crate::settlements::SettlementKey, KnownSettlement>);

/// A settlement that has been materialized onto the map.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct KnownSettlement {
    /// The tile it actually stands on.
    ///
    /// Recorded rather than re-derived. `settlement_at` answers with a
    /// *candidate* cell — the derivation cannot see the ground — and
    /// materialization walks out from it for somewhere standable. That walk
    /// is deterministic, since the map is permanent and itself a pure
    /// function of the seed, but recording the answer means a later change
    /// to how the walk breaks ties cannot move a town the party already
    /// knows.
    pub tile: (i32, i32),
    pub def: crate::settlements::SettlementDef,
}

/// The settlement the player's last step bumped into, waiting for a
/// frontend to open its screen.
///
/// Deliberately not serialized, `CurrentStack`'s reason: this is a cue about
/// *this instant*, not a fact about the world, and a save that restored one
/// would reopen a screen the moment the file loaded rather than on the step
/// that actually asked for it. `Game::take_settlement_visit` is the one
/// door that reads it, and reading it clears it — `EffectQueue`/
/// `TransitQueue`'s shape — so a keypress the player spends walking away
/// from the tile does not find the screen reopening under it.
#[derive(Resource, Default)]
pub struct PendingVisit(pub Option<crate::settlements::SettlementKey>);

/// The frame the player is currently standing in, or `None` on the surface.
///
/// Deliberately not serialized: it regenerates from `(WorldMap::seed,
/// Locale::depth)` on load, exactly as terrain regenerates from the world
/// seed. See `stack::generate`.
#[derive(Resource, Default)]
pub struct CurrentStack(pub Option<Frame>);

/// Where the player materialized on breaching into the current zone sector
/// (set alongside `ZoneLevel` in `Game::new`/`Game::enter_next_zone`) — the
/// origin wild spawns measure distance from to scale stats further out, on
/// top of `ZoneLevel::stat_multiplier` — see `Game::distance_stat_multiplier`.
#[derive(Resource, Clone, Copy, Serialize, Deserialize)]
pub struct ZoneSpawnPoint {
    pub x: i32,
    pub y: i32,
}

/// The trained enemy battle policy, if `assets/policies/enemy_battle.ron`
/// is installed. `None` is an ordinary state, not a failure: it is what
/// every save and every mod that ships no policy plays with, and it means
/// the uniform move roll and slot-weighted target roll the game had before
/// this existed.
///
/// Not saved. Weights are an asset, like a species file — a run picks up
/// whatever is installed when it loads, so retraining reaches saves in
/// flight without a `SAVE_FORMAT_VERSION` bump.
#[derive(Resource, Default)]
pub struct EnemyPolicy(pub Option<crate::policy::PolicyWeights>);

/// A squad of base staff currently away from the base.
///
/// **The site is stored resolved, never as an id or a board index.** A
/// board that rotates while the squad is out, or an `assets/sorties/` file
/// edited between sessions, must not be able to rewrite or strand a trip
/// already in flight — `ActiveContract` stores a whole `ContractDef` for
/// exactly this reason.
///
/// Not `Serialize`: the save form is `save::SortieSave`, which carries no
/// member list at all. Entity ids are not stable across a save, so
/// membership rides `CreatureSave::sortie_index` from the creature side,
/// which is `party_slot`'s precedent.
#[derive(Clone, Debug)]
pub struct Sortie {
    pub site: crate::sorties::SortieDef,
    /// Steps above the zone baseline this site was offered at. Kept beside
    /// the def because the board is what applies it, and because a def
    /// reloaded from an edited file must not be able to change what a trip
    /// already in flight is fighting.
    pub risk: u32,
    pub members: Vec<Entity>,
    pub ticks_total: u64,
    pub ticks_elapsed: u64,
    pub battles_total: u32,
    pub battles_done: u32,
    /// Set when a member goes down. Remaining battles are skipped; the
    /// return travel still runs, because the countdown was always going to
    /// take that long and there is no teleport home.
    pub aborted: bool,
    pub loot: Vec<(crate::items::ItemId, u32)>,
    pub xp: u32,
    pub kills: u32,
    /// Who did not come back, named at the moment they fell — a Permadeath
    /// casualty's entity is gone by the time the report is drawn.
    pub casualties: Vec<String>,
}

/// Every sortie currently away. Saved; see `save::SortieSave`.
#[derive(Resource, Default, Clone, Debug)]
pub struct Sorties(pub Vec<Sortie>);

impl Sorties {
    /// Whether `creature` is away. The one accessor the feature needs:
    /// `party::role_of` asks it, and everything else about an away program
    /// falls out of the role by omission.
    pub(crate) fn contains(&self, creature: Entity) -> bool {
        self.0.iter().any(|s| s.members.contains(&creature))
    }
}

#[cfg(test)]
impl Sortie {
    /// A minimal in-flight record for tests that only care about
    /// membership. Goes through the real struct so a new field is a compile
    /// error here rather than a silently unset default — `work_node_parts`'
    /// rule.
    pub(crate) fn test_stub(members: Vec<Entity>) -> Self {
        Self {
            site: crate::sorties::SortieDef::test_stub(),
            risk: 0,
            members,
            ticks_total: 100,
            ticks_elapsed: 0,
            battles_total: 1,
            battles_done: 0,
            aborted: false,
            loot: Vec::new(),
            xp: 0,
            kills: 0,
            casualties: Vec::new(),
        }
    }
}
