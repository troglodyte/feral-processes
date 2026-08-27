//! The one door a memory is written through.
//!
//! `Game::remember` is to `components::Memories` what `Game::apply_damage` is
//! to `Stats::hp`: the single path, so a rule that must see *every* memory has
//! one place to go. Nothing else in the engine pushes a `Memory`.

use crate::components::{
    MachineStatus, Memories, Memory, MemorySubject, Position, ProgramId, Stats, Stranded,
    Structure, Task, TaskKind,
};
use crate::memories::{MemoryDb, MemoryId};
use crate::resources::{BattleState, GameClock, Party};
use crate::tuning::{MEMORY_CAP_PER_PROGRAM, MEMORY_FORGET_THRESHOLD, MEMORY_POSTING_PERIOD};
use bevy_ecs::prelude::{Entity, Mut};

/// What one `Game::remember` did.
///
/// The spec asks for a subject-kind mismatch to be "refused with a warning",
/// and the engine has no runtime warning channel: `load_dir` warnings are
/// returned `String`s surfaced once at startup, and the message log is
/// player-facing text this feature is forbidden from writing. So the refusal
/// is *returned* instead — four observable outcomes, one per no-op, which is
/// what makes the no-op rule testable without a `debug_assert!` panic sitting
/// in the middle of the test that asserts it.
///
/// Deliberately not `#[must_use]`: a trigger that fires on a body which may or
/// may not be on the roster is the normal case, and `NoStore` is the answer it
/// is entitled to ignore.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Remembered {
    Written,
    /// `who` carries no `Memories` — a hostile, a structure, the player.
    NoStore,
    /// No loaded def claims that id. The deleted-mod-file case.
    UnknownDef,
    /// The subject's kind is not the one the def declares it is about.
    WrongSubject,
}

impl crate::Game {
    /// Forms or reinforces `who`'s memory of `subject`, and evicts what has
    /// faded. **The one door**; every later trigger writes through it.
    ///
    /// **A `who` with no `Memories` is a no-op**, the same deliberate
    /// asymmetry `Game::spend_power` uses for a missing `PowerReserve`. The
    /// store is minted at `Game::roster_parts` and nowhere else, so its
    /// absence *is* "not on the roster" — which keeps hostiles, structures and
    /// the player safe here without a branch at any call site.
    ///
    /// **It draws no RNG at all** — no `GameRng`, no local `StdRng`. That is
    /// what keeps every seeded test and every `dev-arenas/` report where they
    /// are, and it is why none of the RNG-stream-shift diagnostics apply to
    /// anything this feature breaks.
    ///
    /// **It writes no log line.** The screen is the surface. A line every time
    /// a machine strands a body would flood the map's log pane and drag the
    /// fold, filter and reveal seams into a feature that does not need them;
    /// announcing memories is a `MessageKind`/`MessageSource` decision to make
    /// deliberately, not to acquire by default.
    ///
    /// The order below is load-bearing: the def resolves before the store is
    /// touched, so a deleted `assets/memories/` is *inert* rather than merely
    /// quiet.
    pub(crate) fn remember(
        &mut self,
        who: Entity,
        def_id: &str,
        subject: MemorySubject,
    ) -> Remembered {
        let id = MemoryId::from(def_id);
        let Some(def) = self.world.resource::<MemoryDb>().get(&id) else {
            return Remembered::UnknownDef;
        };
        if subject.kind() != def.subject {
            return Remembered::WrongSubject;
        }
        let cap = def.strike_cap;
        if self.world.get::<Memories>(who).is_none() {
            return Remembered::NoStore;
        }
        // Resolved before the component is borrowed mutably: naming a program
        // is a read of the whole world, which cannot coexist with that borrow.
        let subject_name = self.remembered_name(&subject);
        let now = self.world.resource::<GameClock>().tick;

        // The store is looked up a second time rather than held across the
        // name resolution above, which needs `&self`.
        let Some(mut store) = self.world.get_mut::<Memories>(who) else {
            return Remembered::NoStore;
        };
        match store
            .0
            .iter_mut()
            .find(|m| m.def == id && m.subject == subject)
        {
            Some(held) => {
                held.strikes = (held.strikes + 1).min(cap);
                held.reinforced = now;
                held.subject_name = subject_name;
            }
            None => store.0.push(Memory {
                def: id,
                subject,
                subject_name,
                reinforced: now,
                strikes: 1,
            }),
        }

        // Eviction is lazy and this is the only place it happens — nothing
        // sweeps. It needs the catalogue to score an entry, which the
        // component borrow above cannot coexist with.
        self.world.resource_scope(|world, db: Mut<MemoryDb>| {
            if let Some(mut store) = world.get_mut::<Memories>(who) {
                evict(&mut store.0, &db, now);
            }
        });
        Remembered::Written
    }

    /// Remembers what nearly ended `defender`, by the species of whatever
    /// swung it.
    ///
    /// **Hooked at `resolve_and_apply_attack` rather than at `apply_damage`.**
    /// `apply_damage` is the one door that *damages* a creature and is the
    /// obvious home — except that it does not know who is swinging, and the
    /// subject here is the attacker's species. Every creature-versus-creature
    /// attack in the game already funnels through the one function that holds
    /// both entities and the figure that actually landed. Its five other
    /// callers are terrain, a thrown item, a raid, a status tick and a fumble
    /// rung: none has an attacker whose *species* a program could hold a
    /// grudge against, and a fumble's recoil damages the swinger, which from
    /// inside `apply_damage` would read as being mauled by one's own kind.
    ///
    /// `landed` is what came off after mitigation, never what was rolled —
    /// armour that absorbed the blow is armour that stopped the scar. It is
    /// measured against `max_hp` rather than against the HP that was left, so
    /// a mauling is a property of the blow rather than of how worn down the
    /// body already was.
    ///
    /// An attacker carrying no `Creature` — the player — has no species to be
    /// remembered by, and a `defender` with no store is `remember`'s own
    /// no-op. Neither needs a branch at the call site.
    pub(crate) fn note_maul(&mut self, attacker: Entity, defender: Entity, landed: i32) {
        let Some(max_hp) = self.world.get::<Stats>(defender).map(|s| s.max_hp) else {
            return;
        };
        if (landed as f32) <= max_hp as f32 * crate::tuning::MEMORY_MAUL_FRACTION {
            return;
        }
        let Some(species) = self
            .world
            .get::<crate::components::Creature>(attacker)
            .map(|c| c.species.clone())
        else {
            return;
        };
        self.remember(defender, "mauled_by", MemorySubject::Species(species));
    }

    /// What a fight the party **won** leaves in its survivors: a bond with
    /// everyone who came out of it beside them, and — if the fight was uphill
    /// at the bell — the fight itself.
    ///
    /// The twin of `mark_nemeses`, which is what a fight the party *lost*
    /// leaves behind, and it sits beside it in `end_battle` for that reason.
    ///
    /// **"Won" is an empty `BattleState::groups`**, the same read
    /// `settle_rewards` and the `FightEnd` telemetry record make, so the three
    /// cannot come to different conclusions about one fight. A jack-out leaves
    /// the groups standing and is the whole of what this gate refuses.
    ///
    /// The player is neither a holder nor a subject, and that falls out of
    /// `ProgramId`'s absence rather than out of a `Player` check: the id is
    /// minted at the roster barrier, and the player does not pass through it.
    /// The dead are already reaped by the time this runs, which is harmless —
    /// what a bond is worth is having *survived* together.
    pub(crate) fn form_victory_memories(&mut self) {
        let won = self
            .world
            .get_resource::<BattleState>()
            .is_some_and(|b| b.groups.is_empty());
        if !won {
            return;
        }
        let outmatched = self.world.resource::<BattleState>().outmatched;
        // Collected before anything is written: `remember` takes `&mut self`,
        // and the roster it walks must be the one that came out of the fight
        // rather than one shifting under the loop.
        let survivors: Vec<(Entity, ProgramId)> = self
            .world
            .resource::<Party>()
            .0
            .iter()
            .copied()
            .filter(|&e| self.creature_alive(e))
            .filter_map(|e| self.world.get::<ProgramId>(e).map(|id| (e, *id)))
            .collect();
        for &(who, _) in &survivors {
            for &(other, id) in &survivors {
                if other == who {
                    continue;
                }
                self.remember(who, "bonded_in_battle", MemorySubject::Program(id));
            }
            if outmatched {
                self.remember(who, "hard_won", MemorySubject::Nothing);
            }
        }
    }

    /// Remembers, for every worker whose stranding **began this tick**, the
    /// corner of the base it was left standing in.
    ///
    /// The tile is the worker's own `Position` and not its post: a stranded
    /// body is stranded precisely because it is *not* at its post, "left
    /// stranded here" is a claim about where it is standing, and a memory
    /// keyed to the machine's tile could never be read by the drift hook it
    /// exists for — `drift_idle_staff` already refuses a tile a `Structure`
    /// stands on. A posted program's `Position` is base space, which is the
    /// space `MemorySubject::BaseTile` names.
    ///
    /// **Edge-triggered, off `Stranded::since`.** `haul_step_system` writes
    /// the marker on entry only, so this pass runs immediately after the
    /// schedule — where those commands have just flushed and before the clock
    /// moves on at the bottom of the tick. One tick later the edge is gone
    /// and there is nothing left to tell a fresh stranding from a standing
    /// one; a per-tick write instead would saturate `strike_cap` in three
    /// ticks and hold the grudge at full intensity for as long as the route
    /// stayed broken, which makes `strikes` mean nothing.
    pub(crate) fn note_strandings(&mut self) {
        let now = self.world.resource::<GameClock>().tick;
        // Collected before anything is written, for `form_victory_memories`'
        // reason: `remember` takes `&mut self`.
        let fresh: Vec<(Entity, Position)> = self
            .world
            .query::<(Entity, &Position, &Stranded)>()
            .iter(&self.world)
            .filter(|(_, _, stranded)| stranded.since == now)
            .map(|(e, pos, _)| (e, *pos))
            .collect();
        for (worker, pos) in fresh {
            self.remember(
                worker,
                "stranded_at",
                MemorySubject::BaseTile { x: pos.x, y: pos.y },
            );
        }
    }

    /// Remembers, every `MEMORY_POSTING_PERIOD` ticks, what each posted
    /// program is doing and where.
    ///
    /// Runs from `tick_inner` immediately after the schedule, beside
    /// `note_strandings` and for that call's stated reason: the base systems'
    /// commands have just flushed and the clock has not moved on yet.
    ///
    /// **On a period rather than every tick, and rather than on an edge.**
    /// `note_strandings` is edge-triggered because `Stranded::since` gives it
    /// an edge to read; a posting gives none — nothing tells the first tick at
    /// a machine from the thousandth. See `MEMORY_POSTING_PERIOD` for what a
    /// per-tick write would do to `strikes` and to eviction. It is also why
    /// this does not fire on a completed cycle instead: that would make
    /// `strikes` a cycle count, so the same length of service would mean
    /// different things at a fast machine and a slow one.
    ///
    /// **Two memories, on two axes**, because a posting is both a place and a
    /// kind of work:
    ///
    /// - the machine, as a `Structure` — `settled_in` normally, `jammed_here`
    ///   while it is `Clogged`. Same subject, opposite signs, so a machine
    ///   kind that mostly runs nets out to a mild fondness and one that spends
    ///   its life backed up nets out to a grudge.
    /// - the work, as an `Activity` — `cutting_rock` for a digger, which is
    ///   the one posting with no machine to remember: a `DigSite` is the one
    ///   `Task` target that is not a `Structure`, so that arm is skipped for
    ///   free rather than by a check.
    ///
    /// **The player needs no exclusion here** even though the player can hold
    /// a `Task`. `remember` is a no-op on a body with no `Memories`, and the
    /// store is minted at `roster_parts` and nowhere else — so "not on the
    /// roster" refuses this without a branch, exactly as it does at the four
    /// triggers that came before.
    pub(crate) fn note_postings(&mut self) {
        let now = self.world.resource::<GameClock>().tick;
        if !now.is_multiple_of(MEMORY_POSTING_PERIOD) {
            return;
        }
        // Collected before anything is written, for `note_strandings`' and
        // `form_victory_memories`' reason: `remember` takes `&mut self`.
        let posted: Vec<(Entity, TaskKind, Entity)> = self
            .world
            .query::<(Entity, &Task)>()
            .iter(&self.world)
            .map(|(e, t)| (e, t.kind, t.target))
            .collect();
        for (worker, kind, target) in posted {
            if kind == TaskKind::Excavate {
                self.remember(
                    worker,
                    "cutting_rock",
                    MemorySubject::Activity(TaskKind::Excavate),
                );
            }
            let Some(machine) = self.world.get::<Structure>(target).map(|s| s.kind.clone()) else {
                continue;
            };
            let clogged = self.world.get::<MachineStatus>(target) == Some(&MachineStatus::Clogged);
            self.remember(
                worker,
                if clogged { "jammed_here" } else { "settled_in" },
                MemorySubject::Structure(machine),
            );
        }
    }

    /// The signed sum of every memory `who` currently holds — the one figure
    /// the screen heads its page with, and the closest thing the roster has to
    /// a mood.
    ///
    /// `&self`: it derives. Nothing here evicts, because a read-only screen
    /// that rewrote the roster it is drawing would make what a program
    /// remembers depend on whether anyone looked.
    ///
    /// A body carrying no `Memories` reads zero rather than panicking, the
    /// same asymmetry `remember` makes on the write side: hostiles, structures
    /// and the player are safe here without a branch at the call site.
    pub fn morale(&self, who: Entity) -> f32 {
        self.memory_sum(who, |_| true)
    }

    /// Every memory `who` currently holds, as the page draws them: strongest
    /// first, each already named and already weighed.
    ///
    /// **One derivation, `Game::gear_detail`'s rule.** Every figure on the
    /// screen is a call rather than a formula the renderer keeps a copy of —
    /// `intensity` is `Memory::intensity` projected, not a second expression
    /// of the decay, which is the shape that has drifted in this repo four
    /// times.
    ///
    /// **`&self`, and it evicts nothing.** A read-only screen that rewrote
    /// the roster it is drawing would make what a program remembers depend on
    /// whether anybody looked; a faded entry is still one the program holds,
    /// and it is `Game::remember` that drops it at the next formation.
    ///
    /// **An entry whose def no file defines is skipped**, contributing no
    /// row — `memory_sum`'s rule, and where the empty-database property comes
    /// from at this end: with `assets/memories/` deleted the store is intact
    /// and the page is empty.
    ///
    /// The order is by **magnitude**, `evict`'s rule mirrored rather than
    /// described: a signed sort files every grudge below every fondness, so
    /// the deepest scar a program carries would sit at the bottom of the page
    /// most often opened to read it.
    ///
    /// `sort_by` and not `sort_unstable_by`, so a tie keeps insertion order —
    /// but **there is deliberately no test for that**, because none can
    /// exist: measured, the unstable sort returns equal keys in insertion
    /// order too at every length a store can reach, `MEMORY_CAP_PER_PROGRAM`
    /// being 12 and the unstable sort running insertion sort under 20. A test
    /// that cannot tell the two calls apart is coverage-shaped, so the
    /// guarantee is taken from the standard library's contract and stated
    /// here instead.
    pub fn memory_report(&self, who: Entity) -> Vec<crate::views::MemoryRow> {
        let Some(store) = self.world.get::<Memories>(who) else {
            return Vec::new();
        };
        let db = self.world.resource::<MemoryDb>();
        let now = self.world.resource::<GameClock>().tick;
        let mut rows: Vec<crate::views::MemoryRow> = store
            .0
            .iter()
            .filter_map(|m| {
                let def = db.get(&m.def)?;
                Some(crate::views::MemoryRow {
                    name: def.name.clone(),
                    blurb: def.blurb.clone(),
                    subject: self.subject_name(m),
                    intensity: m.intensity(def, now),
                    age: age_phrase(now.saturating_sub(m.reinforced), def.half_life),
                })
            })
            .collect();
        rows.sort_by(|a, b| {
            b.intensity
                .abs()
                .partial_cmp(&a.intensity.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        rows
    }

    /// What a row calls the thing a memory is about.
    ///
    /// Exhaustive, and it stays exhaustive — `cell_mark`'s rule. Under a
    /// `_ =>` arm a seventh `MemorySubject` variant would ship drawing a row
    /// that names nothing, and the symptom is a blank column rather than a
    /// compiler error.
    ///
    /// A `Program` is named off the **record** rather than by a live lookup.
    /// The name is captured at the write for exactly this case: the program a
    /// memory is about can be destroyed, and the screen still has to say who
    /// it was. A species or a structure a mod has since removed falls back to
    /// its id, which is at least a thing the player can search a file for.
    fn subject_name(&self, memory: &Memory) -> Option<String> {
        match &memory.subject {
            MemorySubject::Nothing => None,
            MemorySubject::Program(_) => Some(
                memory
                    .subject_name
                    .clone()
                    .unwrap_or_else(|| "a program that is gone".to_string()),
            ),
            MemorySubject::Species(id) => Some(
                self.world
                    .resource::<crate::species::SpeciesDb>()
                    .get(id)
                    .map_or_else(|| id.clone(), |def| def.name.clone()),
            ),
            MemorySubject::Structure(id) => Some(
                self.world
                    .resource::<crate::structures::StructureDb>()
                    .get(id)
                    .map_or_else(|| id.clone(), |def| def.name.clone()),
            ),
            // Named as *base* space, never as a tile on the zone surface:
            // the two are the same pair of integers meaning different
            // things, and reading one as the other put the base's roster on
            // the open grid once already.
            MemorySubject::BaseTile { x, y } => Some(format!("the base at ({x}, {y})")),
            MemorySubject::Activity(kind) => Some(
                match kind {
                    crate::components::TaskKind::GatherResource => "working a machine",
                    crate::components::TaskKind::Guard => "standing guard",
                    crate::components::TaskKind::Excavate => "cutting rock",
                }
                .to_string(),
            ),
        }
    }

    /// What `who` thinks of one thing: `morale` restricted to the memories
    /// about `subject`.
    ///
    /// A subject nothing has happened about sums an empty set and answers
    /// zero, which is a real answer and not a missing one.
    pub(crate) fn opinion_of(&self, who: Entity, subject: &MemorySubject) -> f32 {
        self.memory_sum(who, |m| &m.subject == subject)
    }

    /// The one fold both readers are, with the restriction handed in.
    ///
    /// `opinion_of` is a *restriction* of `morale` structurally rather than by
    /// description: two folds could disagree about whether an unresolvable def
    /// counts, and a comment claiming one mirrors the other is the shape that
    /// has drifted in this repo four times.
    ///
    /// **An entry whose def no file defines is skipped**, contributing
    /// nothing. That is where the empty-database property comes from — with
    /// `assets/memories/` deleted every entry is unresolvable and every reader
    /// answers zero, without a load-time purge and without the entries being
    /// lost if the directory comes back.
    fn memory_sum(&self, who: Entity, keep: impl Fn(&Memory) -> bool) -> f32 {
        let Some(store) = self.world.get::<Memories>(who) else {
            return 0.0;
        };
        crate::memories::sum_intensity(
            store,
            self.world.resource::<MemoryDb>(),
            self.world.resource::<GameClock>().tick,
            keep,
        )
    }

    /// The display name to stamp on a memory of `subject`, resolved at the
    /// write rather than at the read: the program a memory is about can be
    /// destroyed, and the screen still has to say who it was.
    ///
    /// `None` for every non-`Program` subject and for a program already gone —
    /// a subject that has no name is not a failure, and the two are the same
    /// answer as far as a row is concerned.
    fn remembered_name(&self, subject: &MemorySubject) -> Option<String> {
        let MemorySubject::Program(id) = subject else {
            return None;
        };
        let entity = self
            .world
            .iter_entities()
            .find(|e| e.get::<ProgramId>() == Some(id))?
            .id();
        Some(self.creature_label(entity))
    }
}

/// Drops what has faded, then the weakest while the store is over its cap.
///
/// **Magnitude, never signed value, at both.** A signed comparison evicts
/// every grudge and keeps every fondness, which is not a memory system: the
/// deepest scar a program carries is the smallest number in its store.
///
/// The entry `remember` just wrote survives this by construction — it is at
/// full undecayed intensity, so its magnitude is `|valence| * 1`, and the
/// shipped-catalogue census refuses a zero valence. Say that out loud rather
/// than leaving it implicit: a later `MEMORY_FORGET_THRESHOLD` raised past the
/// weakest authored valence would make formation silently write nothing.
///
/// An entry naming a def no file defines is **kept** by the threshold sweep,
/// per the standing rule that restoring a removed mod file restores the
/// memories that named it. It cannot be scored, so when the cap forces a
/// choice it is the first thing dropped — a memory the game cannot weigh must
/// not hold a slot against one it can.
fn evict(store: &mut Vec<Memory>, db: &MemoryDb, now: u64) {
    let weight = |m: &Memory| db.get(&m.def).map(|def| m.intensity(def, now).abs());
    store.retain(|m| weight(m).is_none_or(|w| w >= MEMORY_FORGET_THRESHOLD));
    while store.len() > MEMORY_CAP_PER_PROGRAM {
        // The loop condition guarantees a non-empty store, so the weakest is
        // folded from index 0 with no empty case to absorb. Strictly `<`, so
        // ties break by insertion order and eviction is reproducible run to
        // run; an unscoreable entry weighs 0.0 and so goes first.
        let mut weakest = 0;
        for i in 1..store.len() {
            if weight(&store[i]).unwrap_or(0.0) < weight(&store[weakest]).unwrap_or(0.0) {
                weakest = i;
            }
        }
        store.remove(weakest);
    }
}

/// How long ago a memory last landed, in the player's words.
///
/// **Against the def's own half-life**, never against an absolute tick
/// count: the game has never shown the player a tick, and a memory whose
/// half-life is 3,000 is old at an age another's is barely into. Banding it
/// this way is what makes "recently" mean the same thing on every row —
/// early in its own decay — rather than the same number of ticks.
///
/// Four bands and no more. A fifth would be a distinction the decay curve
/// cannot support: past two half-lives a memory is under a quarter of what
/// it was and is close to what `evict` drops, so everything out there is
/// simply long ago.
fn age_phrase(elapsed: u64, half_life: u64) -> String {
    let ratio = elapsed as f64 / half_life.max(1) as f64;
    match ratio {
        _ if elapsed == 0 => "just now",
        _ if ratio < 0.5 => "recently",
        _ if ratio < 2.0 => "a while ago",
        _ => "long ago",
    }
    .to_string()
}
