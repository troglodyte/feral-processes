use bevy_ecs::prelude::{Entity, Resource};
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::battle::{BattleAction, EnemyGroup};
use crate::items::ItemId;
use crate::stack::{Dir, Frame};
use crate::structures::StructureId;
use crate::tuning::{MAX_BUILD_DISTANCE_FROM_HOME, PLATFORM_CORNER_CUT};

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum DifficultyMode {
    Permadeath,
    Forgiving,
}

#[derive(Resource, Default, Clone, Copy, Serialize, Deserialize)]
pub struct GameClock {
    pub tick: u64,
}

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
    /// the kill, the XP, the decompile verdict, the jack-out. `MessageLog::
    /// retain_outcomes_since_battle` keeps exactly these (plus `Loot` and
    /// `LevelUp`, which already tag themselves) when a battle ends, which is
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

/// One line as stored. A struct rather than the `(kind, text)` tuple it grew
/// from: with a third field the positional form stops reading, and every
/// consumer already wanted to name what it was reaching for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogLine {
    pub kind: MessageKind,
    pub source: MessageSource,
    pub text: String,
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
        self.push_line(kind, MessageSource::Field, line);
    }

    /// News from the base rather than from wherever the party is standing.
    pub fn push_base(&mut self, line: impl Into<String>) {
        self.push_line(MessageKind::Info, MessageSource::Base, line);
    }

    pub fn push_base_kind(&mut self, kind: MessageKind, line: impl Into<String>) {
        self.push_line(kind, MessageSource::Base, line);
    }

    fn push_line(&mut self, kind: MessageKind, source: MessageSource, text: impl Into<String>) {
        self.lines.push(LogLine {
            kind,
            source,
            text: text.into(),
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
pub(crate) const CONDENSE_LOOKBACK: usize = 4;

/// Folds repeated lines together for the history screen — the `L` log,
/// nowhere else. The map's pane and the battle pane show every line, since
/// the battle narration's reveal pacing counts rows.
///
/// Deliberately a view over the stored log rather than a collapse in
/// `MessageLog::push_kind`: storage carries the mark arithmetic that
/// `since_round` and `retain_outcomes_since_battle` slice with, and merging a
/// new round's first line backwards across `open_round` would drop it out of
/// the battle pane's range entirely.
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
pub(crate) fn condense(lines: &[LogLine]) -> Vec<LogEntry> {
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
}

/// Achievements earned since the last time anyone wrote `profile.ron`.
///
/// The engine decides what has been earned and app-core owns the path, so
/// this is the handoff between them: `achievement_system` pushes, app-core
/// drains after each tick and writes. It accumulates rather than being
/// per-tick like `RunFeats`, because a failed or skipped drain must not lose
/// the earn.
#[derive(Resource, Default)]
pub struct PendingProfileWrites(pub Vec<crate::achievements::AchievementId>);

/// The single player-controlled entity. Kept as a resource (rather than
/// re-queried with a `With<Player>` filter each time) since lookups happen
/// on almost every action.
#[derive(Resource, Clone, Copy)]
pub struct PlayerEntity(pub Entity);

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

/// Center of the player's base platform — the slab of `Biome::Platform`
/// stamped across `MAX_BUILD_DISTANCE_FROM_HOME` when a Home is deployed.
/// `None` until the run's first Home goes down, which is why the opening
/// minutes of a run scale danger exactly as they did before platforms
/// existed.
///
/// Exists as a resource rather than being looked up from the Home entity
/// because `Game::distance_stat_multiplier` and `Game::max_group_size` take
/// `&self`, while querying for the Home needs `&mut self`.
///
/// Deliberately not serialized: it's reconstructed on load from the Home's
/// own position, which `save::SaveData::structures` already carries.
#[derive(Resource, Default, Clone, Copy)]
pub struct Platform {
    pub center: Option<(i32, i32)>,
}

impl Platform {
    /// Whether a tile `(dx, dy)` from the Home is part of the slab: the
    /// build box with `PLATFORM_CORNER_CUT` diagonal steps trimmed off each
    /// corner.
    ///
    /// The one statement of the base's footprint, and deliberately a
    /// function of the offset rather than of the resource — `stamp_platform`
    /// lays the floor, `clear_platform` takes it up, and `place_structure`
    /// decides what may stand on it, and a shape one of the three disagreed
    /// about would put a machine on wild ground at a cut corner or leave
    /// orphan floor behind a demolished Home.
    pub(crate) fn covers(dx: i32, dy: i32) -> bool {
        dx.abs() <= MAX_BUILD_DISTANCE_FROM_HOME
            && dy.abs() <= MAX_BUILD_DISTANCE_FROM_HOME
            && dx.abs() + dy.abs() <= 2 * MAX_BUILD_DISTANCE_FROM_HOME - PLATFORM_CORNER_CUT
    }
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
/// A shelf row is `(item, fusion tier, qty)`. The tier is not decoration:
/// a shelf keyed on the item alone would hand a mis-sold T3 back as an
/// ordinary copy and silently delete eight base copies of work — see
/// `components::FusedGear`.
#[derive(Resource, Default, Clone)]
pub struct BuybackLedger(pub BTreeMap<ShelfKey, Vec<(ItemId, u32, u32)>>);

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
    /// doubles with each zone level (level 1 = x1, level 2 = x2, level 3 =
    /// x4, ...).
    pub fn stat_multiplier(self) -> i32 {
        crate::tuning::ZONE_STAT_GROWTH.pow(self.0 - 1)
    }
}

/// Whether the player is walking the zone map or is down inside the Stack,
/// and where in it.
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
}

/// Everything the party has learned about every Stack frame in this zone.
///
/// Zone-local, and **not** self-clearing: like `BuybackLedger` this has to
/// be wiped by name in `Game::enter_next_zone`, because breaching does not
/// despawn what a zone accumulated. Left behind, a stale entry would draw
/// the previous sector's walked corridors onto a new sector's map at
/// whatever tile happened to collide.
#[derive(Resource, Clone, Default, Debug, Serialize, Deserialize)]
pub struct StackMemory(pub BTreeMap<FrameKey, FrameMemory>);

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
