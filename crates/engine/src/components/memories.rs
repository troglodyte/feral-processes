use super::*;

/// What one owned program remembers. Minted empty at `Game::roster_parts`
/// beside `ProgramId`, so the absence of this component means "not on the
/// roster" rather than "remembers nothing".
#[derive(Component, Clone, Debug, Default)]
pub struct Memories(pub Vec<Memory>);

/// What one owned program has said and heard, newest first, capped at
/// `CONVERSATION_RING`. Inserted by the first conversation rather than minted
/// with the roster, so a program that has never spoken carries none; readers
/// treat absent as empty.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct Conversations(pub std::collections::VecDeque<crate::interactions::ConversationRecord>);

impl Conversations {
    pub fn push(&mut self, record: crate::interactions::ConversationRecord) {
        self.0.push_front(record);
        self.0.truncate(crate::tuning::CONVERSATION_RING);
    }
}

/// One remembered thing: which kind it is, what it was about, when it was
/// last reinforced, and how many times.
///
/// **What it is worth is not here.** Intensity is derived from the game clock
/// at read time — see `Memory::intensity` — for the reason `Platform`'s
/// radius, a program's role and a Broker's board are derived: nothing ticks,
/// nothing oscillates, reinforcement is a single field write, and a stored
/// weight cannot drift out of step with the clock the way a per-tick
/// decrement can.
#[derive(Clone, Debug)]
pub struct Memory {
    /// Which `memories::MemoryDef` this is a record of. An id no file
    /// defines is **kept**, not dropped: restoring a removed mod file
    /// restores the memories that named it, and every reader already skips
    /// what it cannot resolve.
    pub def: crate::memories::MemoryId,
    pub subject: MemorySubject,
    /// The subject's display name as of the last reinforcement, captured at
    /// the write rather than resolved at the read — a program can be
    /// destroyed, and the screen still has to say who the memory is about.
    pub subject_name: Option<String>,
    /// The `resources::GameClock` tick this last landed on. The zero point
    /// of the decay, and the only field reinforcement writes besides
    /// `strikes`.
    pub reinforced: u64,
    /// How many times it has landed, clamped at read time by the def's
    /// `strike_cap`.
    pub strikes: u32,
}

/// What a memory is *about*.
///
/// Serde lives on this enum directly rather than on a `save::` mirror of it,
/// which is the opposite call from `save::CronjobKind`. A mirror here would
/// be a second copy of a six-variant enum that a new variant must be added to
/// twice with nothing failing to compile if it isn't — and the whole point of
/// `kind()`'s exhaustive match is that a new variant *must* fail to compile.
/// The on-disk form is field-named RON, so variants encode by name and
/// reordering them is not a save-format change (unlike `perks::Perk`, which
/// bincode encodes positionally).
///
/// **`BaseTile`, not `Place`.** Base space and surface space are the same two
/// integers meaning different things, and reading one as the other is what
/// put the base's roster on the open grid. Naming the space in the type is
/// what stops that recurring. A *surface* variant, when content asks for one,
/// needs no wipe either: the world is persistent, so a surface tile is as
/// permanent as a base tile and a memory keyed to one stays true.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MemorySubject {
    /// About nothing in particular — a memory of an event, not of a thing.
    Nothing,
    Program(ProgramId),
    Species(SpeciesId),
    Structure(StructureId),
    /// **Base-space** coordinates, never a tile on the zone surface.
    BaseTile {
        x: i32,
        y: i32,
    },
    Activity(TaskKind),
}

impl MemorySubject {
    /// Which `MemorySubjectKind` this payload satisfies, so a write can be
    /// checked against the def that declared one.
    ///
    /// Exhaustive, and it stays exhaustive — `render/stack.rs`'s `cell_mark`
    /// rule. Under a `_ =>` arm a seventh variant would ship answering the
    /// wrong kind, and the symptom is a trigger silently refused rather than
    /// a compiler error.
    pub fn kind(&self) -> crate::memories::MemorySubjectKind {
        use crate::memories::MemorySubjectKind as K;
        match self {
            MemorySubject::Nothing => K::Nothing,
            MemorySubject::Program(_) => K::Program,
            MemorySubject::Species(_) => K::Species,
            MemorySubject::Structure(_) => K::Structure,
            MemorySubject::BaseTile { .. } => K::BaseTile,
            MemorySubject::Activity(_) => K::Activity,
        }
    }
}

impl Memory {
    /// What this memory is worth on tick `now`: signed, decayed, and derived
    /// on every read.
    ///
    /// ```text
    /// valence * (1 + r + r^2 + … + r^(n-1)) * 2^-(elapsed / (half_life * MULT))
    /// ```
    ///
    /// for `n = min(strikes, strike_cap)` and `r = def.stack_decay`. `r ==
    /// 1.0` (the shipped default) collapses the geometric sum to plain `n`,
    /// today's linear stacking.
    ///
    /// The decay is a magnitude scale and never a sign flip, because `morale`
    /// is a signed sum over this figure — a grudge that decayed into a
    /// fondness would read as a program cheering up because it was hurt a
    /// while ago.
    pub fn intensity(&self, def: &crate::memories::MemoryDef, now: u64) -> f32 {
        self.intensity_with(def, now, crate::tuning::MEMORY_HALF_LIFE_MULTIPLIER)
    }

    /// `intensity` with the global stickiness dial supplied rather than read.
    ///
    /// The dial is a parameter here for `walk_field`'s reason: at its shipped
    /// neutral value a test cannot tell a formula that honours it from one
    /// that ignores it, so the only way to *prove* the dial reaches the
    /// denominator is to vary it.
    pub(crate) fn intensity_with(
        &self,
        def: &crate::memories::MemoryDef,
        now: u64,
        stickiness: f32,
    ) -> f32 {
        // A memory reinforced later than `now` is unreachable through
        // `Game::remember`, but a hand-edited save can hold one and an
        // underflow here is a panic in release arithmetic, not a wrong
        // number.
        let elapsed = now.saturating_sub(self.reinforced) as f32;
        let n = self.strikes.min(def.strike_cap);
        let r = def.stack_decay;
        // `r == 1.0` is the shipped default and must stay bit-identical to the
        // linear term; the closed form would also divide 0 by 0 there.
        let stacked = if r == 1.0 {
            n as f32
        } else {
            (1.0 - r.powi(n as i32)) / (1.0 - r)
        };
        def.valence * stacked * 2f32.powf(-elapsed / (def.half_life as f32 * stickiness))
    }
}
