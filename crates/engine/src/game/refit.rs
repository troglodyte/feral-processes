//! The Refit Bench: mends worn gear a repair point a tick while the player
//! stands by it. A spent Annealed Core pours `REFIT_POINTS_PER_CORE` points
//! into the bench's `RefitCharge`, and a point is one unit of `wear`, so no
//! core is wasted on light wear.

use crate::base_ledger::ConsumeSource;
use crate::components::RefitCharge;
use crate::items::{GearCopy, ItemId, ids};
use crate::resources::RefitStand;
use crate::tuning::{REFIT_IDLE_TICKS, REFIT_POINTS_PER_CORE, REFIT_POINTS_PER_TICK, REFIT_REACH};
use crate::*;

/// Where a copy to be mended sits: on a wearer in a slot, or in the pack.
pub(crate) enum WornAt {
    Slot(Entity, EquipmentSlot, EquippedItem),
    Pack(GearCopy),
}

impl WornAt {
    fn copy(&self) -> &GearCopy {
        match self {
            WornAt::Slot(_, _, item) => &item.copy,
            WornAt::Pack(copy) => copy,
        }
    }
}

/// The structure that mends. Only its presence and stock matter, as with
/// `affix_mods::MOD_BENCH`.
const REFIT_BENCH: &str = "refit_bench";

/// A bench, its poured charge, and the cores in its stock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bench {
    entity: Entity,
    charge: u32,
    cores: u32,
}

impl Bench {
    fn serviceable(&self) -> bool {
        self.charge > 0 || self.cores > 0
    }
}

/// One tick of mending, decided from the bench's charge and stock and the
/// served copy's wear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RefitStep {
    pub spend_core: bool,
    pub restored: u8,
    pub charge_after: u32,
}

/// A core is spent only when the charge is empty and there is wear to take
/// it; the tick then restores up to `REFIT_POINTS_PER_TICK`, never more than
/// the wear or the charge.
pub(crate) fn refit_step(charge: u32, cores: u32, wear: u8) -> RefitStep {
    let spend_core = charge == 0 && cores > 0 && wear > 0;
    let charge = if spend_core {
        REFIT_POINTS_PER_CORE
    } else {
        charge
    };
    let restored = REFIT_POINTS_PER_TICK
        .min(wear)
        .min(u8::try_from(charge).unwrap_or(u8::MAX));
    RefitStep {
        spend_core,
        restored,
        charge_after: charge - u32::from(restored),
    }
}

impl Game {
    /// Whether a Refit Bench stands anywhere in the base, so a screen can say
    /// so before the player goes looking for one.
    pub fn refit_bench_standing(&self) -> bool {
        self.has_structure(REFIT_BENCH)
    }

    /// The `(charge, cores)` of the bench that would serve: the first
    /// serviceable one in reach, else the first in reach, else the first in
    /// the base. In reach wins so the view never quotes a stocked bench
    /// elsewhere beside the log's "out of Annealed Cores". `None` when no
    /// bench stands.
    pub fn refit_bench_reserve(&self) -> Option<(u32, u32)> {
        let in_reach = self.refit_benches_in_reach();
        in_reach
            .iter()
            .copied()
            .find(Bench::serviceable)
            .or_else(|| in_reach.first().copied())
            .or_else(|| self.refit_benches().into_iter().next())
            .map(|b| (b.charge, b.cores))
    }

    /// Every Refit Bench with its charge and cores, in `(x, y)` order: bevy's
    /// iteration order is not stable, and serving takes the first.
    fn refit_benches(&self) -> Vec<Bench> {
        let core = ItemId::from(ids::ANNEALED_CORE);
        let mut found: Vec<((i32, i32), Bench)> = self
            .world
            .iter_entities()
            .filter(|e| e.get::<Structure>().is_some_and(|s| s.kind == REFIT_BENCH))
            .filter_map(|e| {
                let at = e.get::<Position>()?;
                let cores = e
                    .get::<Stock>()
                    .and_then(|s| s.input.get(&core).copied())
                    .unwrap_or(0);
                let charge = e.get::<RefitCharge>().map_or(0, |c| c.0);
                Some((
                    (at.x, at.y),
                    Bench {
                        entity: e.id(),
                        charge,
                        cores,
                    },
                ))
            })
            .collect();
        found.sort_unstable_by_key(|(at, _)| *at);
        found.into_iter().map(|(_, bench)| bench).collect()
    }

    /// The benches within `REFIT_REACH` of the player's base cell, in `(x, y)`
    /// order: the one set both the mending and its empty-stock warning ask
    /// about, so neither speaks for a bench the player is not standing at.
    fn refit_benches_in_reach(&self) -> Vec<Bench> {
        let Some((px, py)) = self.field_pos() else {
            return Vec::new();
        };
        self.refit_benches()
            .into_iter()
            .filter(|b| {
                self.world
                    .get::<Position>(b.entity)
                    .is_some_and(|at| (at.x - px).abs().max((at.y - py).abs()) <= REFIT_REACH)
            })
            .collect()
    }

    /// Every copy the bench would mend now, in the order it serves them: the
    /// player's slots, then each party member's in `Party` order, most worn
    /// first within a wearer, then pack copies the same way. Any wear at all:
    /// a point costs a point, so a scratch wastes nothing.
    fn worn_for_auto_refit(&self) -> Vec<WornAt> {
        let needs_it = |copy: &GearCopy| copy.wear > 0;
        let mut wearers = vec![self.player_entity()];
        wearers.extend(self.world.resource::<Party>().0.iter().copied());
        let mut out = Vec::new();
        for wearer in wearers {
            let Some(equipment) = self.world.get::<Equipment>(wearer) else {
                continue;
            };
            let mut worn: Vec<WornAt> = EquipmentSlot::ALL
                .into_iter()
                .filter_map(|slot| Some((slot, equipment.get(slot)?)))
                .filter(|(_, item)| needs_it(&item.copy))
                .map(|(slot, item)| WornAt::Slot(wearer, slot, item))
                .collect();
            worn.sort_by_key(|at| std::cmp::Reverse(at.copy().wear));
            out.extend(worn);
        }
        let mut carried: Vec<GearCopy> = self
            .world
            .get::<GearCopies>(self.player_entity())
            .map(|rows| {
                rows.copies
                    .iter()
                    .map(|(copy, _)| copy.clone())
                    .filter(needs_it)
                    .collect()
            })
            .unwrap_or_default();
        carried.sort_by_key(|copy| std::cmp::Reverse(copy.wear));
        out.extend(carried.into_iter().map(WornAt::Pack));
        out
    }

    /// The bench that is mending the player this tick, if any: in base
    /// space, no battle, stood still for more than `REFIT_IDLE_TICKS`, a
    /// serviceable bench within `REFIT_REACH`, and a copy that needs it. A
    /// pure read, so the pass and the map's mark ask the one question.
    pub(crate) fn refit_bench_serving(&self) -> Option<Entity> {
        if self.field_pos().is_none()
            || self.has_active_battle()
            || self.world.resource::<RefitStand>().ticks <= REFIT_IDLE_TICKS
        {
            return None;
        }
        let bench = self
            .refit_benches_in_reach()
            .into_iter()
            .find(Bench::serviceable)?;
        (!self.worn_for_auto_refit().is_empty()).then_some(bench.entity)
    }

    /// One tick of the bench's own service: counts the player's stand, and
    /// once it is long enough restores a point to the most urgent copy.
    pub(crate) fn run_refit_bench(&mut self) {
        self.count_refit_stand();
        let serving = self.refit_bench_serving();
        let was_mending = std::mem::replace(
            &mut self.world.resource_mut::<RefitStand>().mended,
            serving.is_some(),
        );
        match serving {
            Some(bench) => self.mend_next_worn(bench),
            None => self.warn_refit_bench_empty(was_mending),
        }
    }

    fn count_refit_stand(&mut self) {
        let here = self.field_pos();
        let mut stand = self.world.resource_mut::<RefitStand>();
        match here {
            Some(cell) if stand.cell == Some(cell) => stand.ticks += 1,
            Some(cell) => {
                stand.cell = Some(cell);
                stand.ticks = 1;
            }
            None => *stand = RefitStand::default(),
        }
    }

    /// Gathers the bench and the head of the queue, decides through
    /// `refit_step`, and applies: a core into the charge, points off the
    /// copy. A worn copy is swapped in its slot; a carried one changes key in
    /// the pack, since a mended copy may be plain again and so change store.
    /// Logs only when the copy reaches full condition.
    fn mend_next_worn(&mut self, bench: Entity) {
        let Some(at) = self.worn_for_auto_refit().into_iter().next() else {
            return;
        };
        let Some(state) = self.refit_benches().into_iter().find(|b| b.entity == bench) else {
            return;
        };
        let copy = at.copy().clone();
        let step = refit_step(state.charge, state.cores, copy.wear);
        if step.spend_core
            && !crate::game::base::stock::spend_supply(self, bench, ConsumeSource::Refit)
        {
            debug_assert!(false, "the bench was read as holding a core");
            return;
        }
        self.world
            .entity_mut(bench)
            .insert(RefitCharge(step.charge_after));
        if step.restored == 0 {
            return;
        }
        let owner = match &at {
            WornAt::Slot(wearer, ..) if *wearer != self.player_entity() => {
                Some(self.creature_label(*wearer))
            }
            _ => None,
        };
        let worn = match at {
            WornAt::Slot(wearer, slot, old) => Some((wearer, slot, old)),
            WornAt::Pack(_) => None,
        };
        let mended = GearCopy {
            wear: copy.wear - step.restored,
            ..copy.clone()
        };
        let mended = self.rekey_copy(&copy, mended, worn);
        if mended.wear > 0 {
            return;
        }
        let name = self.copy_name(&mended);
        self.log(match owner {
            Some(label) => format!("The Refit Bench mended {label}'s {name}."),
            None => format!("The Refit Bench mended {name}."),
        });
    }

    /// Says so once per stand, when work is waiting and every bench within
    /// reach has neither charge nor cores: on the first tick the wait is
    /// over, or on the tick the charge ran dry mid-stand (`was_mending`).
    fn warn_refit_bench_empty(&mut self, was_mending: bool) {
        let first_ready_tick = self.world.resource::<RefitStand>().ticks == REFIT_IDLE_TICKS + 1;
        if !(first_ready_tick || was_mending) || self.has_active_battle() {
            return;
        }
        let in_reach = self.refit_benches_in_reach();
        if !in_reach.is_empty()
            && !in_reach.iter().any(Bench::serviceable)
            && !self.worn_for_auto_refit().is_empty()
        {
            self.log("The Refit Bench is out of Annealed Cores.");
        }
    }
}
