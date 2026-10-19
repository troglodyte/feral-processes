use super::*;

/// What one owned program's reserves stand at. Minted empty at
/// `Game::roster_parts` beside `Memories`, so the absence of this component
/// means "not on the roster" rather than "needs nothing".
///
/// **Empty is not the same as full.** Seeding lives in one place,
/// `seed_missing`, called at the top of the drain — one code path covers a
/// freshly spawned program, a program that predates a new def, and a save
/// written before this feature. Do not seed anywhere else.
///
/// Keyed by `NeedId` in a `BTreeMap` for `Stock`'s reason: iteration order
/// feeds the readouts and a `HashMap` would make the save encoding differ run
/// to run.
#[derive(Component, Clone, Debug, Default)]
pub struct Needs {
    reserves: std::collections::BTreeMap<NeedId, f32>,
    /// Which needs have already had their "nothing services this" complaint
    /// said. **Never saved** — a reload should say it again, `DigSite`'s
    /// `announced_stuck` rule.
    stalled_announced: std::collections::BTreeSet<NeedId>,
    /// Which amenity tile holds this program's place, per need. **Never
    /// saved** and re-claimed on the first tick after a load, the latch's
    /// rule. Lives here rather than in a component of its own because
    /// inserting one moves every staff program to a new archetype, which
    /// shifts query iteration order across the whole engine.
    serviced_at: std::collections::BTreeMap<NeedId, (i32, i32)>,
}

impl Needs {
    /// The amenity tile holding this program's place for `id`, if any.
    pub(crate) fn serviced_at(&self, id: &NeedId) -> Option<(i32, i32)> {
        self.serviced_at.get(id).copied()
    }

    /// Every `(need, amenity tile)` this program holds a place at.
    pub(crate) fn serviced(&self) -> impl Iterator<Item = (&NeedId, (i32, i32))> {
        self.serviced_at.iter().map(|(id, tile)| (id, *tile))
    }

    /// Replaces every held place at once: the allocation is recomputed whole
    /// each tick, so a place not re-granted is released.
    pub(crate) fn set_serviced_at(
        &mut self,
        places: std::collections::BTreeMap<NeedId, (i32, i32)>,
    ) {
        self.serviced_at = places;
    }

    pub fn get(&self, id: &NeedId) -> Option<f32> {
        self.reserves.get(id).copied()
    }

    /// Clamps to `NEED_MIN..=NEED_MAX`. **The clamp is the type's**, exactly
    /// as `PowerReserve`'s is: a mod's `per_tick` and a save file's number are
    /// equally outside this crate's control, and no caller clamps.
    pub fn set(&mut self, id: &NeedId, value: f32) {
        self.reserves
            .insert(id.clone(), value.clamp(NEED_MIN, NEED_MAX));
    }

    pub fn iter(&self) -> impl Iterator<Item = (&NeedId, f32)> {
        self.reserves.iter().map(|(id, v)| (id, *v))
    }

    /// Any def in `db` with no entry here starts full. The one seeding site —
    /// see the type doc.
    pub fn seed_missing(&mut self, db: &crate::needs::NeedDb) {
        for def in db.iter() {
            self.reserves.entry(def.id.clone()).or_insert(NEED_MAX);
        }
    }

    /// Whether this need's stall has already been announced.
    pub(crate) fn is_latched(&self, id: &NeedId) -> bool {
        self.stalled_announced.contains(id)
    }

    /// Latches the stall, and reports whether this was the **edge**. The
    /// announcement and the grudge both hang off a `true`, which is what
    /// makes them once each however many ways the need can fail to be met.
    pub(crate) fn latch(&mut self, id: &NeedId) -> bool {
        self.stalled_announced.insert(id.clone())
    }

    /// Clears the latch, so a need that recovers and runs down again
    /// complains a second time — `set_machine_status`' rule, that entering a
    /// state is news and staying in it is not.
    pub(crate) fn unlatch(&mut self, id: &NeedId) {
        self.stalled_announced.remove(id);
    }
}

/// The one thing this feature stores about a program that is not a reserve:
/// **which need has taken it off its post.**
///
/// Everything else is derived — which amenity is nearest, whether the program
/// is being serviced, what it is doing — for `hauling::Errand`'s reason. This
/// cannot be, because it is **hysteresis**: a rule read off the current value
/// alone flickers every tick at the boundary, pulled off at 20, returned at
/// 20.1, drained to 20 again. Inserted below the need's `critical`, removed at
/// its `content`, and the gap between the two is the whole point.
#[derive(Component, Clone, Debug)]
pub struct OffShift {
    pub need: NeedId,
}

/// A program whose morale has run far enough below zero that it has stopped
/// working — the `OffShift` shape on a different meter.
///
/// **Hysteresis is why this is stored and not derived**, exactly as
/// `OffShift` is: read off the current morale alone a body downs tools and
/// picks them up again every tick at the boundary. It is inserted below
/// `MORALE_DOWNS_TOOLS_AT` and removed at `MORALE_RECOVERED_AT`, and the gap
/// between the two is the feature.
///
/// It carries how far past the line the program went, because the ladder has
/// two rungs and the boundary between them needs the same protection as the
/// boundary at the bottom. Severity **ratchets** while the marker is held: a
/// body that has downed tools does not go back to merely sulking as morale
/// wobbles, it goes back to work or it does not. One hysteresis gap, not two.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Disgruntled {
    pub grievance: Grievance,
    /// Whether the respite errand has been given up as unwalkable — the
    /// no-route latch, and `Needs::stalled_announced`'s job on this meter.
    ///
    /// **Reachability is never asked as its own question**, exactly as it is
    /// not for `OffShift`: `Game::step_respite` discovers it and sets this,
    /// which is what stops the leave-pool → failed step → rejoin-pool → leave
    /// flicker on every beat. One Dijkstra per newly disgruntled body, then
    /// nothing until `RESPITE_RETRY_TICKS` elapses or the mood recovers and
    /// takes the marker with it.
    ///
    /// A latched body is back in the posting pool, which is where
    /// `Game::refuses_post` governs it — so a base with nowhere to unwind
    /// still has the grudge-against-a-machine rung, rather than that rung
    /// quietly becoming unreachable.
    pub stranded: bool,
    /// Whether `Game::strand_respite`'s line has already been said for this
    /// marker. **Never saved**, `Needs::stalled_announced`'s rule — a reload
    /// re-announcing a stranding already in progress is acceptable, and
    /// `CreatureSave` carries no field for this one. Survives the retry that
    /// clears `stranded` (only recovery clears this, by removing the whole
    /// marker), which is what keeps a body stuck for several retries from
    /// repeating the line once a period instead of once.
    pub(crate) told: bool,
}

/// How far a program has gone. Ordered least to worst; see `Disgruntled`.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Grievance {
    /// Still works, but not at a machine it holds a grudge against — the
    /// posting-time counterpart of `drift_idle_staff`'s tile avoidance,
    /// against the same `MEMORY_AVOIDANCE_THRESHOLD`.
    Sulking,
    /// Takes no posting at all. Leaves the `on_shift` filter, which covers
    /// the posting, the standdown and the `LabourDemand` shortfall at once.
    DownedTools,
    /// Rounds on whoever is in reach and hits them — `game::base::tantrum`.
    ///
    /// **Appended, never inserted.** `Ord` derives from declaration order and
    /// is what `update_disgruntled`'s ratchet compares, and
    /// `SaveData::disgruntled` is field-named RON encoding the variant
    /// *name*. Inserting reorders the ladder silently and costs a save
    /// format bump for nothing.
    ///
    /// It is strictly worse than `DownedTools` and not a separate axis, so
    /// `Game::has_downed_tools` reads `>=` rather than `==`: a program on
    /// this rung has already stopped working.
    LashingOut,
}

impl Grievance {
    /// Every rung, lowest first.
    pub const ALL: [Grievance; 3] = [
        Grievance::Sulking,
        Grievance::DownedTools,
        Grievance::LashingOut,
    ];

    /// The wire name a bench or analysis reads the rung by.
    pub fn as_str(self) -> &'static str {
        match self {
            Grievance::Sulking => "sulking",
            Grievance::DownedTools => "downed_tools",
            Grievance::LashingOut => "lashing_out",
        }
    }
}

#[cfg(test)]
mod grievance_all_tests {
    use super::Grievance;

    /// The match has no wildcard, so a new rung stops this compiling until it
    /// is placed here, and the assertion below until it is in `ALL`.
    #[test]
    fn every_rung_is_in_all() {
        let mut seen = 0;
        for rung in [
            Grievance::Sulking,
            Grievance::DownedTools,
            Grievance::LashingOut,
        ] {
            match rung {
                Grievance::Sulking | Grievance::DownedTools | Grievance::LashingOut => {
                    assert!(Grievance::ALL.contains(&rung), "{rung:?}");
                    seen += 1;
                }
            }
        }
        assert_eq!(seen, Grievance::ALL.len());
    }
}
