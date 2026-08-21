//! The one door a memory is written through.
//!
//! `Game::remember` is to `components::Memories` what `Game::apply_damage` is
//! to `Stats::hp`: the single path, so a rule that must see *every* memory has
//! one place to go. Nothing else in the engine pushes a `Memory`.

// Nothing in play calls `remember` yet — the four triggers that will are the
// next phase's, and each gets its own reviewer boundary. `expect` rather than
// `allow` on purpose: the moment a real caller lands this becomes an
// unfulfilled expectation and says so, which is what gets the attribute
// deleted instead of quietly outliving its reason. `pub` was considered and
// refused for `DescriptionDb::subjects`' stated reason — widening visibility
// to suppress a warning states something untrue about who may call this, and
// the renderer must never write a memory.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "phase 3 wires the first caller; delete this attribute with it"
    )
)]

use crate::components::{Memories, Memory, MemorySubject, ProgramId};
use crate::memories::{MemoryDb, MemoryId};
use crate::resources::GameClock;
use crate::tuning::{MEMORY_CAP_PER_PROGRAM, MEMORY_FORGET_THRESHOLD};
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
    pub(crate) fn morale(&self, who: Entity) -> f32 {
        self.memory_sum(who, |_| true)
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
        let db = self.world.resource::<MemoryDb>();
        let now = self.world.resource::<GameClock>().tick;
        store
            .0
            .iter()
            .filter(|m| keep(m))
            .filter_map(|m| Some(m.intensity(db.get(&m.def)?, now)))
            .sum()
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
