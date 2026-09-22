//! The tactical battle model: a disposable grid a surface fight is fought
//! on.
//!
//! Opt-in, off by default, and the second of the game's two combat models —
//! see
//! `docs/superpowers/archive/specs/2026-09-09-tactical-surface-battles-design.md`.
//! `Game::start_battle` is the router that chooses between the two models,
//! by inspecting the pack; `arena::stage` is the second chooser, and takes
//! the model its caller can drive.
//!
//! **A battle coordinate lives here and nowhere else.** No world `Position`
//! is ever written for a body standing on a battle map, the same way the
//! Stack keeps its coordinates in `resources::Locale` and base space keeps
//! its own in `Locale::Base`. This module does not import `Position`.

pub mod ai;
pub mod deploy;
pub mod map;
pub mod reach;
pub mod squads;
pub mod turn;
pub mod view;

use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::{Entity, Resource};

use crate::components::GlyphColor;
use crate::resources::BattleRewards;
use crate::tactical::map::{BattleSpec, Board};

/// A cell a Hallucination has told the other side a body stands on.
///
/// **Not an entity and not part of any `Tampered` entry.** A decoy outlives
/// being struck at by one body and is shared by every hallucinating body on
/// the other side, so it belongs to the fight rather than to anyone in it —
/// and it is nothing `reach::movement_field`, `line_of_sight` or
/// `recipients` can see, because none of them read this list.
///
/// `owner_hostile` is the invoker's literal `Hostile`-ness, and a body sees a
/// decoy only when it is hallucinating and stands on the other side of that
/// line — `Game::sees_decoy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Decoy {
    pub cell: (i32, i32),
    pub owner_hostile: bool,
    /// The invoker's own glyph and hue, so the fake reads as the body that
    /// ran it.
    pub glyph: char,
    pub color: GlyphColor,
    /// The player's `@` is drawn in the `PLAYER` role rather than its
    /// `GlyphColor`, so a decoy of the player has to say so or it draws in the
    /// hue the player merely spawned with.
    pub of_player: bool,
}

/// Whether a body on the `hostile` side stands opposite something placed by
/// the `owner_hostile` side.
///
/// **The one expression of "opposing".** `Decoy::opposes` asks it of a decoy
/// and its watcher; `Game::hallucinate` asks it of an invoker and a
/// recipient, and `Game::tactical_use_routine` asks the same question again
/// before spending anything on the invocation. A free function rather than
/// three inline `!=`s, because every one of them is a place a party body
/// could come to be handed its own side's decoy.
pub(crate) fn opposes(owner_hostile: bool, hostile: bool) -> bool {
    owner_hostile != hostile
}

impl Decoy {
    /// Whether a hallucinating body on the `hostile` side sees this decoy —
    /// the other side's, never its own. A call into [`opposes`], which is
    /// where that rule lives.
    pub fn opposes(&self, hostile: bool) -> bool {
        opposes(self.owner_hostile, hostile)
    }
}

/// A tactical fight's spatial state: the map it is fought on and where
/// every body stands.
///
/// **The one place a battle coordinate lives.** No world `Position` is
/// written for a body on a battle map, the same way the Stack keeps its
/// coordinates in `resources::Locale` and base space keeps its own there
/// too. A body's `Position` stays exactly where the fight opened.
///
/// Bare `#[derive(Resource)]` — no `Default`, no `Serialize` — matching
/// `BattleState` for the same reason: a fight is never saved, so nothing
/// here appears in `save.rs` and `SAVE_FORMAT_VERSION` does not move.
#[derive(Resource)]
pub struct TacticalBattle {
    pub spec: BattleSpec,
    pub board: Board,
    /// A `Vec` and not a `HashMap`. Iteration order has to be stable —
    /// bevy's own query order is not, and anything that walks a fight's
    /// bodies must not resolve differently between runs, which is `Stock`'s
    /// `BTreeMap` rule again — and a fight holds at most thirteen bodies,
    /// so a linear scan is the simpler thing and also the faster one.
    bodies: Vec<(Entity, (i32, i32))>,
    /// What this fight has paid out so far, held back until it ends.
    ///
    /// The second arm of `Game::fight_rewards_mut`, and a field rather than
    /// a resource of its own for the reason `BattleRewards`' own doc gives:
    /// a new `Resource` shifts bevy's query iteration order under unrelated
    /// tests, and a fight's payout has no business doing that.
    pub(crate) rewards: BattleRewards,
    /// Every body still in the fight, fastest first — rolled once when the
    /// fight opens rather than fresh each round, so a turn-order strip is
    /// stable enough to plan against.
    ///
    /// **Kept in step by deletion, not by re-sorting.** A body that dies or
    /// walks off the edge leaves this list in `remove`, which is the whole
    /// of "re-sorted as bodies die": the survivors' relative order was
    /// settled at the bell and nothing later may disturb it.
    initiative: Vec<Entity>,
    /// Which entry of `initiative` is acting. Never points past the end
    /// while any body remains — `wrap` is what holds that.
    turn: usize,
    /// What the acting body has spent on movement this turn, against
    /// `Game::movement_allowance`.
    spent: u32,
    /// How many of the acting body's actions this turn are unspent — one
    /// without a `Squad`. The turn ends once this reaches zero, so it is
    /// only ever read between an action landing and the turn being handed
    /// on. Set from `shapes` in `begin_turn`.
    actions_left: u8,
    /// The cells the acting body has committed to walking and has not walked
    /// yet, in the order it will enter them.
    ///
    /// **`None` is "has not chosen yet" and `Some(vec![])` is "has chosen to
    /// stop here"**, which is the whole reason this is not a bare `Vec`: a
    /// hostile's walk is spent one cell a beat, and a beat that found the
    /// list empty must tell "the walk is done, act now" from "no walk has
    /// been planned" — read as one, the AI plans a fresh walk every beat and
    /// draws `GameRng` once per cell instead of once per turn.
    walk: Option<Vec<(i32, i32)>>,
    /// How many times the order has come round, from 1. The results
    /// header's figure and the telemetry's alike.
    pub round: u32,
    /// How many decompiles this fight has thrown at each program, so a
    /// program's defences fray across a fight and no longer.
    ///
    /// `BattleState::decompile_attempts`' counterpart, and a field here for
    /// `rewards`' reason: a counter of its own would be a `Resource`, and a
    /// new one shifts bevy's query iteration order under unrelated tests.
    /// Read and written through `Game::decompile_attempts`/`_mut`, never
    /// directly, so a capture rolls against the same count on a battle map
    /// as it does in front of a group.
    pub(crate) decompile_attempts: HashMap<Entity, u32>,
    /// Whether the hostiles outweighed the party at the bell, by summed
    /// `Stats::power()` — `BattleState::outmatched`'s counterpart, and a
    /// snapshot for its reason: by the time a fight is won the question is
    /// unanswerable.
    pub(crate) outmatched: bool,
    /// Every decoy a Hallucination has placed and nobody has struck through
    /// yet, in placement order — `bodies`' reason for a `Vec`.
    decoys: Vec<Decoy>,
    /// Who has spent their reaction and not yet had it back — `bodies`'
    /// reason for a `Vec` again, and a fight holds at most thirteen.
    ///
    /// **Refunded at a body's own turn, which is `begin_turn` and not the
    /// hand-on.** The cursor moves in two places and only one of them is
    /// `end_turn`: a body that dies on the last rung starts the next round
    /// from inside `remove`, and a refund hung off the hand-on would miss
    /// whoever the wrap landed on — one body unable to react for a round,
    /// for a reason nothing on screen could explain.
    reacted: Vec<Entity>,
    /// A body's shape on the board — footprint and actions per turn —
    /// written once, when it is seated (`set_shape`), and absent for an
    /// ordinary body.
    ///
    /// `TacticalBattle` holds no `World`, so `footprint_of` and `begin_turn`
    /// cannot resolve a `components::Squad` themselves; they read whatever
    /// the caller who *does* have one (`Game::open_tactical_battle_at`)
    /// told them at seat time instead. A `HashMap` rather than a fourth
    /// parallel `Vec`, for `decompile_attempts`' reason: both are keyed
    /// lookups a body's own turn reads, never walked in fight order.
    shapes: HashMap<Entity, BodyShape>,
    /// How many `components::Besieger` bodies a siege's pack opened
    /// with — `0` for every fight that is not one, `Game::open_siege`'s own
    /// count of what it actually seated rather than `siege::pack_size(zone)`
    /// restated, since a raider that could not be seated at all (no free
    /// cell near the door) must not count toward a morale break it never
    /// joined.
    ///
    /// `game::siege::raiders::siege_morale_broken`'s one reader: deriving
    /// this from `pack_size(zone)` instead would read any battle fixture
    /// seating fewer than half a real pack as already broken, which is
    /// every hand-built unit test in this file's siege coverage — a field
    /// set once, at seat time, is what keeps "a siege's own pack" and "a
    /// test's one besieger" from being the same question.
    pub(crate) siege_pack: u32,
    /// The base-space cell the board's `(0, 0)` maps to, for a siege —
    /// `(0, 0)` for every other fight, `siege_pack`'s own "not a siege"
    /// answer. `game::siege::board::SiegeBoard::origin`'s own copy, kept
    /// here because a `SiegeBoard` is a construction-time-only wrapper and
    /// `game::siege::persist::assemble` has only this resource to read a
    /// save back out of.
    pub(crate) siege_origin: (i32, i32),
    /// `BASE_EXIT_CELL`, in board coordinates — `siege_origin`'s own
    /// reason, and `game::siege::board::SiegeBoard::door`'s copy.
    pub(crate) siege_door: (i32, i32),
}

/// One entry of `TacticalBattle::shapes` — see that field's doc.
#[derive(Clone, Copy, Debug)]
struct BodyShape {
    footprint: u8,
    actions: u8,
}

impl TacticalBattle {
    pub fn open(spec: BattleSpec, board: Board) -> Self {
        TacticalBattle {
            spec,
            board,
            bodies: Vec::new(),
            rewards: BattleRewards::default(),
            decompile_attempts: HashMap::new(),
            initiative: Vec::new(),
            turn: 0,
            spent: 0,
            actions_left: 1,
            walk: None,
            round: 1,
            outmatched: false,
            decoys: Vec::new(),
            reacted: Vec::new(),
            shapes: HashMap::new(),
            siege_pack: 0,
            siege_origin: (0, 0),
            siege_door: (0, 0),
        }
    }

    /// Whether `body` has already spent its reaction this round.
    pub fn reaction_spent(&self, body: Entity) -> bool {
        self.reacted.contains(&body)
    }

    /// Charges `body`'s reaction. Idempotent, so a caller that swings twice
    /// in one provocation cannot hand a body two budgets back.
    pub(crate) fn spend_reaction(&mut self, body: Entity) {
        if !self.reacted.contains(&body) {
            self.reacted.push(body);
        }
    }

    /// Puts a body on a cell, or refuses.
    ///
    /// Refused when any cell of the footprint anchored there cannot be stood
    /// on, when somebody already holds one, or when this body is already on
    /// the board — the last so a double placement is a refusal rather than a
    /// second entry that `cell_of` would answer from and `occupant` would
    /// not.
    pub fn place(&mut self, body: Entity, cell: (i32, i32)) -> bool {
        if self.cell_of(body).is_some() {
            return false;
        }
        let footprint = self.footprint_cells(body, cell);
        let blocked: HashSet<(i32, i32)> = self
            .bodies()
            .flat_map(|(other, _)| self.cells_of(other))
            .collect();
        if !footprint_clear(&self.board, &footprint, &blocked) {
            return false;
        }
        self.bodies.push((body, cell));
        true
    }

    pub fn cell_of(&self, body: Entity) -> Option<(i32, i32)> {
        self.bodies
            .iter()
            .find(|(e, _)| *e == body)
            .map(|(_, cell)| *cell)
    }

    /// The body whose footprint contains `cell`, if any.
    pub fn occupant(&self, cell: (i32, i32)) -> Option<Entity> {
        self.bodies
            .iter()
            .map(|&(e, _)| e)
            .find(|&e| self.cells_of(e).contains(&cell))
    }

    /// How many cells wide (and tall) `body`'s footprint is — one without a
    /// `Squad`, read out of `shapes` — see that field's doc for why this
    /// cannot ask a `Squad` component itself.
    ///
    /// Every reader of a body's footprint goes through this or
    /// [`cells_of`](Self::cells_of) rather than assuming one cell, so
    /// widening a formation's footprint is the only place that has to
    /// change.
    pub fn footprint_of(&self, body: Entity) -> u8 {
        self.shapes.get(&body).map_or(1, |s| s.footprint)
    }

    /// Records `body`'s shape for the rest of the fight — its footprint and
    /// how many actions it gets a turn. Called once, when it is seated
    /// (`Game::open_tactical_battle_at`), never again: a squad keeps its
    /// formation to the end whatever its Integrity.
    ///
    /// **Shaping comes before seating**, and the assert is what holds it:
    /// `place` validates the footprint it knows about, so a body seated
    /// while still reading as one cell has its block checked against
    /// nothing, and a widening `set_shape` afterwards can leave two bodies
    /// overlapping with no refusal anywhere. Today the one caller is
    /// correct only because `deploy::plan` reserved a clear block two files
    /// away; this makes the rule local to the type, so a second seating
    /// site cannot get it wrong quietly.
    pub(crate) fn set_shape(&mut self, body: Entity, footprint: u8, actions: u8) {
        debug_assert!(
            self.cell_of(body).is_none(),
            "a body's shape must be set before it is seated, or `place` \
             checked a footprint it did not yet know about"
        );
        self.shapes.insert(body, BodyShape { footprint, actions });
    }

    /// The cells `body`'s footprint would cover, anchored top-left at
    /// `anchor` — [`cells_of`](Self::cells_of)'s general form, usable before
    /// a body is actually standing there. `place`'s own refusal needs to ask
    /// about a cell nobody occupies yet.
    fn footprint_cells(&self, body: Entity, anchor: (i32, i32)) -> Vec<(i32, i32)> {
        footprint_cells_at(anchor, self.footprint_of(body))
    }

    /// Every cell `body`'s footprint covers right now, anchored at
    /// [`cell_of`](Self::cell_of) — empty for a body not on the board, the
    /// same "off the board reaches nothing" answer every other reader here
    /// gives.
    pub fn cells_of(&self, body: Entity) -> Vec<(i32, i32)> {
        match self.cell_of(body) {
            Some(anchor) => self.footprint_cells(body, anchor),
            None => Vec::new(),
        }
    }

    /// Whether [`move_to`](Self::move_to) would take `body` to `cell` —
    /// every cell of the footprint anchored there walkable, and held by
    /// nobody but this body.
    ///
    /// **Extracted so a caller can ask before it spends.** `move_to` is the
    /// one writer and calls this rather than restating the rule, because a
    /// refusal that quotes a different answer from the move it guards is
    /// exactly the drift `routine_power_cost` records: `Game::
    /// tactical_teleport` has to refuse an illegal destination *above* its
    /// charge, and `Game::teleport_destinations` outlines the cells the
    /// player may aim at with the same question.
    pub fn can_move_to(&self, body: Entity, cell: (i32, i32)) -> bool {
        if self.cell_of(body).is_none() {
            return false;
        }
        let footprint = self.footprint_cells(body, cell);
        let blocked: HashSet<(i32, i32)> = self
            .bodies()
            .filter(|&(other, _)| other != body)
            .flat_map(|(other, _)| self.cells_of(other))
            .collect();
        footprint_clear(&self.board, &footprint, &blocked)
    }

    /// Moves a placed body, or refuses. Standing still is allowed.
    ///
    /// Refused when any cell of the footprint anchored at `cell` cannot be
    /// stood on, or is held by a body other than this one.
    pub fn move_to(&mut self, body: Entity, cell: (i32, i32)) -> bool {
        if !self.can_move_to(body, cell) {
            return false;
        }
        let slot = self
            .bodies
            .iter_mut()
            .find(|(e, _)| *e == body)
            .expect("checked above: cell_of(body) answered Some");
        slot.1 = cell;
        true
    }

    /// Takes a body off the board — killed, or walked off the edge — and out
    /// of the turn order with it.
    ///
    /// **The cursor names a body, not a position.** Removing an entry ahead
    /// of the cursor shifts everything behind it down one, so the cursor
    /// follows; removing the *acting* body leaves the cursor already naming
    /// whoever stood behind it, which is a fresh turn and is reset as one.
    /// A body that dies on somebody else's turn costs the order nothing.
    pub fn remove(&mut self, body: Entity) {
        self.bodies.retain(|(e, _)| *e != body);
        self.reacted.retain(|e| *e != body);
        // `shapes` goes with the body, like every other per-body record
        // here. A stale entry answers `footprint_of` for a body that has
        // left the fight, and entity ids are reused: the next body bevy
        // hands out that id would be seated as whatever shape the last one
        // was.
        self.shapes.remove(&body);
        let Some(idx) = self.initiative.iter().position(|&e| e == body) else {
            return;
        };
        self.initiative.remove(idx);
        let was_acting = idx == self.turn;
        if idx < self.turn {
            self.turn -= 1;
        }
        // **The wrap comes first, and `begin_turn` after it.** `begin_turn`
        // refunds the reaction of whoever is now acting, so it has to run
        // with the cursor already inside the order — taking the *last* body
        // out leaves `turn == len` until the wrap, and a refund read from
        // there names nobody. The three cursor fields it clears do not care
        // which order they are reset in, which is why this was free.
        self.wrap();
        if was_acting {
            self.begin_turn();
        }
    }

    /// Splices `body` into the turn order immediately behind the cursor.
    ///
    /// **The cursor names a body, not a position** — which is why `remove`
    /// above decrements `turn` when it takes something out ahead of it.
    /// Insertion carries the same trap mirrored: inserting *ahead* of the
    /// cursor shifts every later entry down one and somebody acts twice.
    ///
    /// `turn + 1` is the only index that is safe wherever the cursor sits,
    /// and it reads right at the keyboard: you call it, it acts next. It is
    /// also why this takes no index — there is exactly one correct answer
    /// and a caller choosing would be a caller getting it wrong.
    pub fn insert_after_cursor(&mut self, body: Entity) {
        let at = (self.turn + 1).min(self.initiative.len());
        self.initiative.insert(at, body);
    }

    /// Seats the turn order, fastest first. Called once, when the fight
    /// opens.
    pub fn set_initiative(&mut self, order: Vec<Entity>) {
        self.initiative = order;
        self.turn = 0;
        self.begin_turn();
    }

    /// Picks a fight back up mid-round after a save/load round trip —
    /// `game::siege::persist::restore`'s one caller. Distinct from
    /// `set_initiative`, which starts the order at the front and
    /// recomputes a fresh action budget through `begin_turn`; both are
    /// wrong here; `turn` and `actions_left` are exactly what the save
    /// said, not what a fresh bell would ring.
    ///
    /// `turn` is clamped into `order`, the one concession to a save whose
    /// acting body failed to reload — see `save::SiegeSave::turn`'s own
    /// doc for why that is a raw position here and not an order value:
    /// the caller has already resolved which surviving member it names.
    pub(crate) fn resume(&mut self, order: Vec<Entity>, round: u32, turn: usize, actions_left: u8) {
        let turn = turn.min(order.len().saturating_sub(1));
        self.initiative = order;
        self.turn = turn;
        self.round = round;
        self.actions_left = actions_left;
        self.spent = 0;
        self.walk = None;
    }

    /// The turn order as it stands, fastest first.
    pub fn initiative(&self) -> &[Entity] {
        &self.initiative
    }

    /// Whose turn it is, or `None` when nobody is left to take one.
    pub fn actor(&self) -> Option<Entity> {
        self.initiative.get(self.turn).copied()
    }

    /// What the acting body has spent on movement so far this turn.
    pub fn spent(&self) -> u32 {
        self.spent
    }

    /// How many actions the acting body has left to spend this turn.
    pub fn actions_left(&self) -> u8 {
        self.actions_left
    }

    /// Charges `cost` against the acting body's movement.
    pub fn spend(&mut self, cost: u32) {
        self.spent += cost;
    }

    /// Commits the acting body to a walk, to be spent one cell at a time.
    ///
    /// Committing an empty path is how a body says it is staying where it
    /// is: the walk is then planned and finished at once.
    pub fn commit_walk(&mut self, path: Vec<(i32, i32)>) {
        self.walk = Some(path);
    }

    /// Whether the acting body has already chosen where it is walking.
    pub fn walk_planned(&self) -> bool {
        self.walk.is_some()
    }

    /// The next cell of the committed walk, taken off it.
    pub fn take_walk_step(&mut self) -> Option<(i32, i32)> {
        let walk = self.walk.as_mut()?;
        match walk.is_empty() {
            true => None,
            false => Some(walk.remove(0)),
        }
    }

    /// Whether the acting body is part-way through a walk it has committed
    /// to. What a driver pacing the fight reads to know it owes a step
    /// rather than a turn.
    pub fn walking(&self) -> bool {
        self.walk.as_ref().is_some_and(|walk| !walk.is_empty())
    }

    /// Spends one of the acting body's actions. The caller ends the turn
    /// once none are left — this only says one landed, because a body
    /// killed by its own fumble or its own blast leaves the order instead.
    pub fn spend_action(&mut self) {
        self.actions_left = self.actions_left.saturating_sub(1);
    }

    /// Clears whatever the acting body has committed to walking, leaving
    /// everything else about its turn alone — `begin_turn`'s reset with the
    /// movement allowance and the action budget left untouched.
    ///
    /// **`hand_on_turn`'s own door, for a body with another action still to
    /// spend.** A turn's movement is spent once for the whole turn however
    /// many actions it buys, so the walk is the only thing stale between one
    /// action and the next — the next action plans a fresh one from
    /// wherever this one left the body standing.
    pub(crate) fn clear_walk(&mut self) {
        self.walk = None;
    }

    /// Empties the acting body's action budget without spending anything.
    ///
    /// `tactical_end_turn`'s own door: passing a turn with actions still
    /// unspent forfeits all of them, not just one, so `hand_on_turn`'s
    /// "another action is coming" gate cannot read a pass as anything but
    /// the end of the turn.
    pub(crate) fn forfeit_actions(&mut self) {
        self.actions_left = 0;
    }

    /// Test hook: overrides the acting body's action budget for the turn,
    /// so a rule can be pinned without seating a real `Squad` to carry the
    /// second action. Nothing outside a test calls it — in a real fight the
    /// budget comes from `begin_turn`, off `shapes`.
    #[cfg(test)]
    pub(crate) fn set_actions_left(&mut self, n: u8) {
        self.actions_left = n;
    }

    /// Hands the turn to the next body in the order, starting a new round
    /// when it comes back round to the front.
    pub fn end_turn(&mut self) {
        self.turn += 1;
        self.wrap();
        self.begin_turn();
    }

    /// The one place a body's turn starts — `end_turn`, `remove` and
    /// `set_initiative` all land here, which is what makes it the honest
    /// home for the reaction refund.
    ///
    /// **`actions_left` is read out of `shapes`, `footprint_of`'s door
    /// again** — one for a body `set_shape` never touched.
    fn begin_turn(&mut self) {
        self.spent = 0;
        self.actions_left = self
            .actor()
            .map_or(1, |a| self.shapes.get(&a).map_or(1, |s| s.actions));
        self.walk = None;
        // **Refunded at the start of its own turn, not at the round.** A
        // body that reacts early in a round gets its budget back when its
        // own turn comes round and may react again later in the same one —
        // standing next to an enemy costs the mover, not the clock.
        if let Some(actor) = self.actor() {
            self.reacted.retain(|&e| e != actor);
        }
    }

    /// Brings the cursor back inside the order, counting a round each time
    /// it comes round. An empty order parks it at zero rather than counting
    /// rounds against a fight nobody is left in.
    fn wrap(&mut self) {
        if self.initiative.is_empty() {
            self.turn = 0;
            return;
        }
        if self.turn >= self.initiative.len() {
            self.turn = 0;
            self.round += 1;
        }
    }

    /// Every body and where it stands, in placement order.
    pub fn bodies(&self) -> impl Iterator<Item = (Entity, (i32, i32))> + '_ {
        self.bodies.iter().copied()
    }

    /// Adds a decoy. The caller has already chosen a free cell — see
    /// `Game::apply_tamper`.
    pub(crate) fn place_decoy(&mut self, decoy: Decoy) {
        self.decoys.push(decoy);
    }

    /// Every decoy standing, in placement order.
    pub(crate) fn decoys(&self) -> &[Decoy] {
        &self.decoys
    }

    /// Takes the decoy on `cell` that a body on the `striker_hostile` side
    /// can see off the board, if there is one.
    ///
    /// Takes the **striker's** own side and asks [`Decoy::opposes`], rather
    /// than taking the owner's side and comparing it: the caller already
    /// established through `Game::sees_decoy_at` that this body sees a decoy
    /// here, and that predicate reads `opposes` too — so the decoy removed is
    /// the decoy seen by construction.
    pub(crate) fn take_decoy_at(
        &mut self,
        cell: (i32, i32),
        striker_hostile: bool,
    ) -> Option<Decoy> {
        let idx = self
            .decoys
            .iter()
            .position(|d| d.cell == cell && d.opposes(striker_hostile))?;
        Some(self.decoys.remove(idx))
    }

    /// Keeps only the decoys `keep` answers `true` for.
    pub(crate) fn retain_decoys(&mut self, keep: impl FnMut(&Decoy) -> bool) {
        self.decoys.retain(keep);
    }
}

/// The cells an anchor-top-left footprint of `side` cells covers —
/// [`TacticalBattle::footprint_cells`](TacticalBattle::footprint_cells)'s
/// pure grid math, pulled out to a free function so `deploy` (which never
/// sees an `Entity` at all) can seat a squad's clear NxN block without
/// reaching back into a body's own shape lookup.
pub(crate) fn footprint_cells_at(anchor: (i32, i32), side: u8) -> Vec<(i32, i32)> {
    let side = i32::from(side);
    (0..side)
        .flat_map(|dy| (0..side).map(move |dx| (anchor.0 + dx, anchor.1 + dy)))
        .collect()
}

/// Whether every cell of `footprint` can be stood on: walkable, and none of
/// them in `blocked` — `place`/`move_to`'s shared refusal, and
/// `reach::movement_field`'s destination test, so the anchors a walk is
/// offered are exactly the anchors `move_to` will accept.
///
/// A free function rather than inlined at each call site, so its own test
/// can hand it a hand-built multi-cell footprint with no `Squad` behind it.
pub(crate) fn footprint_clear(
    board: &Board,
    footprint: &[(i32, i32)],
    blocked: &HashSet<(i32, i32)>,
) -> bool {
    footprint
        .iter()
        .all(|&(x, y)| board.walkable(x, y) && !blocked.contains(&(x, y)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tactical::map::{BattleSpec, Board, generate};
    use crate::world::Biome;
    use bevy_ecs::world::World;

    fn fight() -> (TacticalBattle, Vec<Entity>) {
        let spec = BattleSpec {
            world_seed: 5,
            site: (0, 0),
            tick: 10,
            zone: 1,
            biome: Biome::OpenGrid,
            bodies: 4,
        };
        let board = generate(spec);
        let mut world = World::new();
        let bodies = (0..3).map(|_| world.spawn_empty().id()).collect();
        (TacticalBattle::open(spec, board), bodies)
    }

    /// The one place a battle coordinate lives.
    #[test]
    fn a_body_stands_where_it_was_placed() {
        let (mut battle, bodies) = fight();
        let cell = first_open(&battle);
        assert!(battle.place(bodies[0], cell));
        assert_eq!(battle.cell_of(bodies[0]), Some(cell));
        assert_eq!(battle.occupant(cell), Some(bodies[0]));
    }

    fn first_open(battle: &TacticalBattle) -> (i32, i32) {
        battle
            .board
            .cells()
            .find(|(_, k)| k.walkable())
            .map(|(c, _)| c)
            .expect("a board with no ground")
    }

    /// The first anchor a whole `footprint`x`footprint` block stands on —
    /// [`first_open`](first_open) answers for one cell, which is not the
    /// same question once a body is wider than that.
    fn first_open_block(battle: &TacticalBattle, footprint: u8) -> (i32, i32) {
        battle
            .board
            .cells()
            .map(|(c, _)| c)
            .find(|&c| {
                footprint_cells_at(c, footprint)
                    .iter()
                    .all(|&(x, y)| battle.board.walkable(x, y))
            })
            .expect("a board with no room for a block")
    }

    #[test]
    fn two_bodies_never_share_a_cell() {
        let (mut battle, bodies) = fight();
        let cell = first_open(&battle);
        assert!(battle.place(bodies[0], cell));
        assert!(!battle.place(bodies[1], cell), "the cell was taken twice");
        assert_eq!(battle.occupant(cell), Some(bodies[0]));
    }

    #[test]
    fn nobody_stands_on_ground_they_cannot_stand_on() {
        let (mut battle, bodies) = fight();
        let blocked = battle
            .board
            .cells()
            .find(|(_, k)| !k.walkable())
            .map(|(c, _)| c)
            .expect("a board with nothing on it");
        assert!(!battle.place(bodies[0], blocked));
        assert_eq!(battle.cell_of(bodies[0]), None);
    }

    #[test]
    fn a_body_placed_twice_is_refused_rather_than_duplicated() {
        let (mut battle, bodies) = fight();
        let first = first_open(&battle);
        assert!(battle.place(bodies[0], first));
        let elsewhere = battle
            .board
            .cells()
            .filter(|(c, k)| k.walkable() && *c != first)
            .map(|(c, _)| c)
            .next()
            .expect("a board with one cell");
        assert!(
            !battle.place(bodies[0], elsewhere),
            "the body was placed twice"
        );
        assert_eq!(battle.bodies().count(), 1);
    }

    #[test]
    fn a_body_moves_and_leaves_its_cell_behind() {
        let (mut battle, bodies) = fight();
        let from = first_open(&battle);
        let to = battle
            .board
            .cells()
            .filter(|(c, k)| k.walkable() && *c != from)
            .map(|(c, _)| c)
            .next()
            .expect("a board with one cell");
        battle.place(bodies[0], from);
        assert!(battle.move_to(bodies[0], to));
        assert_eq!(battle.cell_of(bodies[0]), Some(to));
        assert_eq!(battle.occupant(from), None);
    }

    #[test]
    fn a_body_cannot_move_onto_somebody_else() {
        let (mut battle, bodies) = fight();
        let a = first_open(&battle);
        let b = battle
            .board
            .cells()
            .filter(|(c, k)| k.walkable() && *c != a)
            .map(|(c, _)| c)
            .next()
            .expect("a board with one cell");
        battle.place(bodies[0], a);
        battle.place(bodies[1], b);
        assert!(!battle.move_to(bodies[0], b));
        assert_eq!(battle.cell_of(bodies[0]), Some(a));
    }

    /// A body's shape leaves with it. Entity ids are reused, so a stale
    /// entry is not merely untidy — the next body handed that id would seat
    /// at the dead one's footprint.
    #[test]
    fn a_removed_body_takes_its_shape_with_it() {
        let (mut battle, bodies) = fight();
        let cell = first_open_block(&battle, 2);
        battle.set_shape(bodies[0], 2, 2);
        assert!(battle.place(bodies[0], cell), "the 2x2 block had no room");
        assert_eq!(battle.footprint_of(bodies[0]), 2);

        battle.remove(bodies[0]);
        assert_eq!(
            battle.footprint_of(bodies[0]),
            1,
            "a departed body kept its footprint"
        );
    }

    /// A body that dies or walks off the edge leaves, and takes its cell
    /// with it.
    #[test]
    fn a_removed_body_frees_its_cell() {
        let (mut battle, bodies) = fight();
        let cell = first_open(&battle);
        battle.place(bodies[0], cell);
        battle.remove(bodies[0]);
        assert_eq!(battle.cell_of(bodies[0]), None);
        assert_eq!(battle.occupant(cell), None);
        assert_eq!(battle.bodies().count(), 0);
    }

    /// A fixture with all three bodies placed and seated in the order they
    /// were spawned.
    fn seated() -> (TacticalBattle, Vec<Entity>) {
        let (mut battle, bodies) = fight();
        let cells: Vec<(i32, i32)> = battle
            .board
            .cells()
            .filter(|(_, k)| k.walkable())
            .map(|(c, _)| c)
            .take(3)
            .collect();
        for (body, cell) in bodies.iter().zip(&cells) {
            battle.place(*body, *cell);
        }
        battle.set_initiative(bodies.clone());
        (battle, bodies)
    }

    #[test]
    fn the_order_is_walked_in_order_and_wraps_into_a_new_round() {
        let (mut battle, bodies) = seated();
        assert_eq!(battle.actor(), Some(bodies[0]));
        assert_eq!(battle.round, 1);
        battle.end_turn();
        assert_eq!(battle.actor(), Some(bodies[1]));
        assert_eq!(battle.round, 1, "a hand-off is not a round");
        battle.end_turn();
        battle.end_turn();
        assert_eq!(battle.actor(), Some(bodies[0]));
        assert_eq!(battle.round, 2);
    }

    /// The turn economy: a fresh turn owes nothing and has acted on nothing.
    #[test]
    fn a_new_turn_starts_with_nothing_spent_and_nothing_done() {
        let (mut battle, _) = seated();
        battle.spend(3);
        battle.spend_action();
        assert_eq!(battle.spent(), 3);
        assert_eq!(battle.actions_left(), 0);
        battle.end_turn();
        assert_eq!(battle.spent(), 0);
        assert_eq!(battle.actions_left(), 1);
    }

    /// `None` is "has not chosen yet" and `Some(vec![])` is "has arrived",
    /// and the whole walk-a-cell-a-beat pacing rests on the difference.
    #[test]
    fn a_planned_walk_and_an_unplanned_one_are_not_the_same_answer() {
        let (mut battle, _) = seated();
        assert!(!battle.walk_planned(), "a fresh turn has chosen nothing");
        assert!(!battle.walking());

        battle.commit_walk(vec![(1, 1), (2, 2)]);
        assert!(battle.walk_planned());
        assert!(battle.walking());
        assert_eq!(battle.take_walk_step(), Some((1, 1)));
        assert_eq!(battle.take_walk_step(), Some((2, 2)));

        assert_eq!(battle.take_walk_step(), None, "the walk is spent");
        assert!(battle.walk_planned(), "a spent walk is still a plan");
        assert!(!battle.walking(), "a body that has arrived is not walking");
    }

    /// A walk belongs to the turn that committed it, so the next body starts
    /// having chosen nothing.
    #[test]
    fn handing_the_turn_on_clears_the_committed_walk() {
        let (mut battle, _) = seated();
        battle.commit_walk(vec![(1, 1)]);
        battle.end_turn();

        assert!(!battle.walk_planned());
        assert_eq!(battle.take_walk_step(), None);
    }

    /// A body dying on somebody else's turn costs the order nothing: the
    /// cursor names a body, not a position, so everyone behind the gap keeps
    /// their place.
    #[test]
    fn a_body_removed_ahead_of_the_cursor_leaves_the_acting_body_acting() {
        let (mut battle, bodies) = seated();
        battle.end_turn();
        battle.end_turn();
        assert_eq!(battle.actor(), Some(bodies[2]));
        battle.remove(bodies[0]);
        assert_eq!(battle.actor(), Some(bodies[2]), "the cursor slipped");
        assert_eq!(battle.initiative(), &[bodies[1], bodies[2]]);
    }

    #[test]
    fn removing_the_acting_body_hands_the_turn_to_whoever_stood_behind_it() {
        let (mut battle, bodies) = seated();
        battle.end_turn();
        battle.spend(2);
        assert_eq!(battle.actor(), Some(bodies[1]));
        battle.remove(bodies[1]);
        assert_eq!(battle.actor(), Some(bodies[2]));
        assert_eq!(battle.spent(), 0, "the dead body's spend carried over");
    }

    /// The last body in the order leaving wraps the cursor rather than
    /// stranding it past the end.
    #[test]
    fn removing_the_last_body_in_the_order_wraps_the_cursor() {
        let (mut battle, bodies) = seated();
        battle.end_turn();
        battle.end_turn();
        battle.remove(bodies[2]);
        assert_eq!(battle.actor(), Some(bodies[0]));
        assert_eq!(battle.round, 2);
    }

    #[test]
    fn an_emptied_order_has_nobody_acting() {
        let (mut battle, bodies) = seated();
        for body in bodies {
            battle.remove(body);
        }
        assert_eq!(battle.actor(), None);
    }

    /// Placement order is what `bodies` reports, every time. Bevy's own
    /// query order is not stable, so anything that walks a fight's bodies
    /// walks this instead.
    #[test]
    fn bodies_come_back_in_the_order_they_were_placed() {
        let (mut battle, bodies) = fight();
        let cells: Vec<(i32, i32)> = battle
            .board
            .cells()
            .filter(|(_, k)| k.walkable())
            .map(|(c, _)| c)
            .take(3)
            .collect();
        for (body, cell) in bodies.iter().zip(&cells) {
            battle.place(*body, *cell);
        }
        let order: Vec<Entity> = battle.bodies().map(|(e, _)| e).collect();
        assert_eq!(order, bodies);
    }

    /// A footprint of one is exactly the anchor `cell_of` already answers —
    /// the whole of `footprint_of`'s answer for a body nothing ever called
    /// `set_shape` for, which is every body but a squad.
    #[test]
    fn a_bodys_cells_are_its_anchor_alone_without_a_squad() {
        let (mut battle, bodies) = fight();
        let cell = first_open(&battle);
        battle.place(bodies[0], cell);
        assert_eq!(battle.footprint_of(bodies[0]), 1);
        assert_eq!(battle.cells_of(bodies[0]), vec![cell]);
    }

    #[test]
    fn a_body_not_on_the_board_has_no_cells() {
        let (battle, bodies) = fight();
        assert!(battle.cells_of(bodies[0]).is_empty());
    }

    /// Two bodies cannot be placed overlapping — `footprint_clear`'s own
    /// rule, pinned on a hand-built two-cell footprint since no `Squad`
    /// exists yet to seat a real one. Deleting the rule (checking only the
    /// footprint's first cell) would still pass a footprint whose *first*
    /// cell is clear, so the blocked cell here is the footprint's second.
    #[test]
    fn a_footprint_refuses_a_cell_any_other_body_already_holds() {
        let board = Board::from_rows(&["...", "...", "..."]);
        let blocked: HashSet<(i32, i32)> = HashSet::from([(1, 1)]);
        assert!(
            !footprint_clear(&board, &[(0, 0), (1, 1)], &blocked),
            "a footprint overlapping an occupied cell was accepted"
        );
        assert!(footprint_clear(&board, &[(0, 0), (0, 1)], &blocked));
    }

    #[test]
    fn a_footprint_refuses_ground_any_of_its_cells_cannot_stand_on() {
        let board = Board::from_rows(&["..X", "...", "..."]);
        let clear = HashSet::new();
        assert!(!footprint_clear(&board, &[(0, 0), (2, 0)], &clear));
        assert!(footprint_clear(&board, &[(0, 0), (1, 0)], &clear));
    }

    /// `set_shape` is what `footprint_of` and `begin_turn` read — the
    /// `Squad` lookup lands at the caller instead, since `TacticalBattle`
    /// holds no `World` to ask a component itself.
    #[test]
    fn a_seated_shape_is_what_footprint_of_and_begin_turn_read() {
        let (mut battle, bodies) = fight();
        let cell = first_open_block(&battle, 2);
        battle.set_shape(bodies[0], 2, 2);
        assert!(battle.place(bodies[0], cell), "the 2x2 block had no room");
        assert_eq!(battle.footprint_of(bodies[0]), 2);
        assert_eq!(
            battle.cells_of(bodies[0]).len(),
            4,
            "a footprint of 2 covers a 2x2 block"
        );

        battle.set_initiative(vec![bodies[0]]);
        assert_eq!(
            battle.actions_left(),
            2,
            "begin_turn (run by set_initiative) must read the seated shape"
        );
    }

    /// Seating checks the whole block, and `TacticalBattle` is where that is
    /// enforced — `deploy::plan` reserving a clear block is what makes the
    /// one production caller safe today, so this pins the rule without it.
    ///
    /// The overlap is on the footprint's *second* cell and its anchor is
    /// free, so a check that reads the anchor alone accepts it.
    #[test]
    fn seating_a_shaped_body_refuses_a_block_another_body_is_standing_in() {
        let spec = BattleSpec {
            world_seed: 5,
            site: (0, 0),
            tick: 10,
            zone: 1,
            biome: Biome::OpenGrid,
            bodies: 4,
        };
        let mut world = World::new();
        let (sitting, squad) = (world.spawn_empty().id(), world.spawn_empty().id());
        let mut battle = TacticalBattle::open(spec, Board::from_rows(&["....."; 5]));
        assert!(battle.place(sitting, (1, 1)));

        battle.set_shape(squad, 2, 2);
        assert!(
            !battle.place(squad, (0, 0)),
            "a 2x2 block was seated over a body standing in its far corner"
        );

        // The same anchor at the ordinary footprint is free, so the refusal
        // above is the block's doing and not the anchor's.
        let ordinary = world.spawn_empty().id();
        assert!(battle.place(ordinary, (0, 0)));
    }

    /// A body `set_shape` never touched still reads footprint 1, action 1 —
    /// the default this whole feature must leave alone.
    #[test]
    fn an_unshaped_body_keeps_the_ordinary_defaults() {
        let (mut battle, bodies) = fight();
        let cell = first_open(&battle);
        battle.place(bodies[0], cell);
        battle.set_initiative(vec![bodies[0]]);
        assert_eq!(battle.footprint_of(bodies[0]), 1);
        assert_eq!(battle.actions_left(), 1);
    }
}
