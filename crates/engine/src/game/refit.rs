//! The Refit Bench: spend an Annealed Core from its stock, and a worn copy's
//! condition returns to full. Player-operated and instant, the Mod Bench's pattern — the bench
//! only has to be standing somewhere in the base.

use crate::base_ledger::ConsumeSource;
use crate::game::wear::is_nearly_worn;
use crate::items::{GearCopy, ItemId, ids};
use crate::resources::RefitStand;
use crate::tuning::{REFIT_CORES, REFIT_IDLE_TICKS, REFIT_REACH};
use crate::*;

/// Where a copy to be mended sits: on a wearer in a slot, or in the pack.
pub(crate) enum WornAt {
    Slot(Entity, EquipmentSlot, EquippedItem),
    Pack(GearCopy),
}

impl WornAt {
    fn wear(&self) -> u8 {
        match self {
            WornAt::Slot(_, _, item) => item.copy.wear,
            WornAt::Pack(copy) => copy.wear,
        }
    }
}

/// The structure a refit requires. Only its presence matters, as with
/// `affix_mods::MOD_BENCH`.
const REFIT_BENCH: &str = "refit_bench";

impl Game {
    /// Whether a Refit Bench stands anywhere in the base, so a screen can say
    /// so before the player pays attention to a bill they cannot pay.
    pub fn refit_bench_standing(&self) -> bool {
        self.has_structure(REFIT_BENCH)
    }

    /// What refitting `copy` costs: the same single `REFIT_CORES` Annealed
    /// Core for any worn gear copy, craftable or found, whatever its rarity.
    /// Empty for anything that is not gear.
    pub fn refit_cost(&self, copy: &GearCopy) -> Vec<(ItemId, u32)> {
        let is_gear = self
            .world
            .resource::<ItemDb>()
            .get(copy.item.as_str())
            .is_some_and(|def| def.equipment.is_some());
        if is_gear {
            vec![(ItemId::from(ids::ANNEALED_CORE), REFIT_CORES)]
        } else {
            Vec::new()
        }
    }

    /// Every Refit Bench with the cores it holds, in `(x, y)` order: bevy's
    /// iteration order is not stable, and `stocked_refit_bench` takes the
    /// first.
    fn refit_benches(&self) -> Vec<(Entity, u32)> {
        let core = ItemId::from(ids::ANNEALED_CORE);
        let mut found: Vec<((i32, i32), Entity, u32)> = self
            .world
            .iter_entities()
            .filter(|e| e.get::<Structure>().is_some_and(|s| s.kind == REFIT_BENCH))
            .filter_map(|e| {
                let at = e.get::<Position>()?;
                let held = e
                    .get::<Stock>()
                    .and_then(|s| s.input.get(&core).copied())
                    .unwrap_or(0);
                Some(((at.x, at.y), e.id(), held))
            })
            .collect();
        found.sort_unstable_by_key(|(at, ..)| *at);
        found.into_iter().map(|(_, e, held)| (e, held)).collect()
    }

    /// The first bench, in `(x, y)` order, holding enough Annealed Cores for
    /// one refit.
    pub(crate) fn stocked_refit_bench(&self) -> Option<Entity> {
        self.refit_benches()
            .into_iter()
            .find(|(_, held)| *held >= REFIT_CORES)
            .map(|(e, _)| e)
    }

    /// How many Annealed Cores the bench a refit would draw on holds, for the
    /// refit screen to quote beside the bill. Zero when none is stocked.
    pub fn refit_bench_cores(&self) -> u32 {
        self.stocked_refit_bench()
            .and_then(|bench| self.world.get::<Stock>(bench))
            .and_then(|s| s.input.get(&ItemId::from(ids::ANNEALED_CORE)).copied())
            .unwrap_or(0)
    }

    /// Restores a located copy to full condition and spends its core from
    /// `bench`: the one mend, shared by the menu action and the bench's own
    /// pass. A worn copy is swapped in its slot by `swap_worn_copy`; a carried
    /// one goes through the pack helpers because a mended copy may be plain
    /// again and so change store. Returns the new copy. Logs nothing and
    /// ticks nothing — both belong to the caller.
    pub(crate) fn mend_located(&mut self, at: WornAt, bench: Entity) -> GearCopy {
        let paid = (0..REFIT_CORES)
            .all(|_| crate::game::base::stock::spend_supply(self, bench, ConsumeSource::Refit));
        debug_assert!(paid, "the caller chose a stocked bench");
        match at {
            WornAt::Slot(wearer, slot, old) => {
                let mended = GearCopy {
                    wear: 0,
                    ..old.copy.clone()
                };
                self.swap_worn_copy(wearer, slot, &old, mended.clone());
                mended
            }
            WornAt::Pack(copy) => {
                let mended = GearCopy {
                    wear: 0,
                    ..copy.clone()
                };
                self.take_copies(&copy, 1);
                self.add_copies(&mended, 1);
                mended
            }
        }
    }

    /// Every copy the bench would mend now, in the order it serves them: the
    /// player's slots, then each party member's in `Party` order, most worn
    /// first within a wearer, then pack copies the same way. Only copies at
    /// the `GearWorn` alert line or past it: the bench fixes what the HUD
    /// warns about, since a core a scratch can spend is a core the next
    /// fight's real damage cannot.
    fn worn_for_auto_refit(&self) -> Vec<WornAt> {
        let needs_it = |copy: &GearCopy| is_nearly_worn(self.condition_of(copy));
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
            worn.sort_by_key(|at| std::cmp::Reverse(at.wear()));
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
    /// stocked bench within `REFIT_REACH`, and a copy that needs it. A pure
    /// read, so the pass and the map's mark ask the one question.
    pub(crate) fn refit_bench_serving(&self) -> Option<Entity> {
        let (px, py) = self.base_pos()?;
        if self.has_active_battle() || self.world.resource::<RefitStand>().ticks <= REFIT_IDLE_TICKS
        {
            return None;
        }
        let bench = self.refit_benches().into_iter().find_map(|(e, held)| {
            let at = self.world.get::<Position>(e)?;
            let near = (at.x - px).abs().max((at.y - py).abs()) <= REFIT_REACH;
            (near && held >= REFIT_CORES).then_some(e)
        })?;
        (!self.worn_for_auto_refit().is_empty()).then_some(bench)
    }

    /// One tick of the bench's own service: counts the player's stand, and
    /// once it is long enough mends one copy through `mend_located`. Never
    /// `refit_copy`, which ticks and would recurse.
    pub(crate) fn run_refit_bench(&mut self) {
        self.count_refit_stand();
        match self.refit_bench_serving() {
            Some(bench) => self.mend_next_worn(bench),
            None => self.warn_refit_bench_empty(),
        }
    }

    fn count_refit_stand(&mut self) {
        let here = self.base_pos();
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

    fn mend_next_worn(&mut self, bench: Entity) {
        let Some(at) = self.worn_for_auto_refit().into_iter().next() else {
            return;
        };
        let owner = match &at {
            WornAt::Slot(wearer, ..) if *wearer != self.player_entity() => {
                Some(self.creature_label(*wearer))
            }
            _ => None,
        };
        let mended = self.mend_located(at, bench);
        let name = self.copy_name(&mended);
        self.log(match owner {
            Some(label) => format!("The Refit Bench mended {label}'s {name}."),
            None => format!("The Refit Bench mended {name}."),
        });
    }

    /// Says so once per stand, on the first tick the wait is over, when work
    /// is waiting and no bench holds a core.
    fn warn_refit_bench_empty(&mut self) {
        let first_ready_tick = self.world.resource::<RefitStand>().ticks == REFIT_IDLE_TICKS + 1;
        if first_ready_tick
            && !self.has_active_battle()
            && self.refit_bench_standing()
            && self.stocked_refit_bench().is_none()
            && !self.worn_for_auto_refit().is_empty()
        {
            self.log("The Refit Bench is out of Annealed Cores.");
        }
    }

    /// Restores `copy` to full condition, paying one Annealed Core from a
    /// stocked Refit Bench. Returns the copy's new key — it is plain again,
    /// so it may live in a different store — and the log line.
    ///
    /// Every refusal sits above the payment, so a refused refit spends
    /// nothing.
    pub fn refit_copy(&mut self, copy: &GearCopy) -> Result<(GearCopy, String), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if !self.refit_bench_standing() {
            return Err("Build a Refit Bench first.".into());
        }
        let Some((slot, _)) = self.equipment_of(&copy.item) else {
            return Err(format!("{} can't be refitted.", self.item_name(&copy.item)));
        };
        let (slot, worn) = self.locate_in_slot(copy, slot)?;
        if copy.wear == 0 {
            return Err(format!(
                "{} is already in full condition.",
                self.copy_name(copy)
            ));
        }
        let Some(bench) = self.stocked_refit_bench() else {
            return Err("Load the Refit Bench with Annealed Cores first.".into());
        };
        let at = match worn {
            Some(item) => WornAt::Slot(self.player_entity(), slot, item),
            None => WornAt::Pack(copy.clone()),
        };
        let renewed = self.mend_located(at, bench);
        let msg = format!("You refit {}.", self.copy_name(&renewed));
        self.log(msg.clone());
        self.tick();
        Ok((renewed, msg))
    }
}
