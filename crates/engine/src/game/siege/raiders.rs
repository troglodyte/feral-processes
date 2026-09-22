//! A besieger's turn: steal, wreck, withdraw — `components::Besieger`'s
//! behaviour, one hook ahead of the generic tactical AI.
//!
//! **The hook sits inside `tactical::turn::run_tactical_beat`, not
//! `Game::tactical_ai_turn`.** The plan named `tactical_ai_turn`, but real
//! play never calls it — `app-core`'s per-frame pacing
//! (`App::advance_tactical`) drives `Game::tactical_ai_beat`, and
//! `run_tactical_beat` is documented as the one implementation every driver
//! shares "so a fight watched a beat at a time and the same fight resolved
//! in one call have to reach the same board." Hooking only the coarse door
//! would have made a besieger fight normally in real play and steal only
//! under a full-turn test resolution. `Game::besieger_turn` is still exactly
//! the interface the plan specifies — `true` when it spent the beat on siege
//! behaviour, `false` to fall through to the generic AI (which is how a
//! besieger with nothing to take, wreck or withdraw toward still fights
//! back).

use bevy_ecs::prelude::Entity;

use crate::Game;
use crate::components::{Besieger, Carrying, Durability, Stock, StolenFrom, Structure};
use crate::resources::MessageKind;
use crate::tactical::TacticalBattle;
use crate::tactical::reach;
use crate::tactical::turn::StepOutcome;
use crate::tuning::{HAUL_CARRY_CAPACITY, SIEGE_MORALE_BREAK_PERCENT, TACTICAL_MELEE_RANGE};

impl Game {
    /// `body`'s whole beat, when it is a besieger — the hook
    /// `tactical::turn::run_tactical_beat` calls before falling through to
    /// the generic tactical AI.
    ///
    /// Steal, then wreck: adjacent to a stocked structure, take what it
    /// holds and make for the door; adjacent to a structure with nothing to
    /// take, swing at it instead — **steal before wreck**, because a raider
    /// that wrecks the shelf it could have emptied makes interception
    /// pointless. Carrying already, walk toward the door; at the door
    /// carrying, leave.
    ///
    /// **A morale break makes the door arm unconditional** (Task 14): once
    /// `siege_morale_broken` answers `true`, steal and wreck are skipped
    /// entirely and every besieger, carrying or not, makes for the door —
    /// no quota and no round limit, a withdrawal is not a rout to chase.
    pub(crate) fn besieger_turn(&mut self, body: Entity) -> bool {
        if self.world.get::<Besieger>(body).is_none() {
            return false;
        }
        // `battle.siege_door`, set once by `Game::open_siege`, and not a
        // fresh `board::build` every beat — the board it would rebuild
        // reads the *current* `BaseGrid`, and `Game::tick` (run from the
        // round upkeep this same fight owns) runs the base's own systems,
        // so a dig finishing mid-siege could move where this door is read
        // as standing without moving where it actually is.
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return false;
        };
        let door = battle.siege_door;
        let Some(from) = battle.cell_of(body) else {
            return false;
        };

        let withdrawing = self.siege_morale_broken();
        let carrying = self.world.get::<Carrying>(body).is_some();

        // **Adjacent, not exactly on it.** The Home always stands on the
        // door (`board::seat_structures` seats it as a body there, and one
        // body to a cell keeps every other body off that exact cell), so
        // `from == door` can never hold in real play — a carrier or a
        // withdrawing besieger would fall through to the generic AI and
        // fight forever short of the threshold this checks against.
        if reach::gap(&[from], &[door]) <= TACTICAL_MELEE_RANGE && (carrying || withdrawing) {
            return self.besieger_leaves(body, carrying);
        }

        if !withdrawing {
            // **Gated on `!carrying`.** Without this, a carrier that is
            // still adjacent to the shelf it just emptied on its next beat
            // (the walk toward the door has not moved it off that cell yet)
            // steals again — `Carrying`/`StolenFrom` are a single pair, so
            // the second `insert` silently replaces the first load rather
            // than adding to it, and the units already lifted are gone for
            // good.
            if !carrying && let Some(structure) = self.adjacent_stocked_structure(body) {
                return self.besieger_steal(body, structure);
            }
            if let Some(structure) = self.adjacent_wreckable_structure(body) {
                return self.besieger_wreck(body, structure);
            }
            if !carrying {
                // Nothing to take or wreck in reach, and no reason yet to
                // make for the door — the generic AI's to fight, opportunist
                // or not.
                return false;
            }
        }

        self.besieger_walk_toward(body, door)
    }

    /// `SIEGE_MORALE_BREAK_PERCENT` of the besieging pack down — killed, or
    /// gone through the door with its plunder, both counting the same way
    /// as "no longer fighting." The original pack size is
    /// `TacticalBattle::siege_pack`, set once by `Game::open_siege` at the
    /// count it actually seated — `0` for a fight this is not a siege
    /// board's, which is what keeps every non-siege tactical fixture (and
    /// every siege one that only ever seats a handful of bodies to test one
    /// besieger's own behaviour) from reading itself as already broken.
    pub(crate) fn siege_morale_broken(&self) -> bool {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return false;
        };
        let original = battle.siege_pack;
        if original == 0 {
            return false;
        }
        let remaining = battle
            .bodies()
            .filter(|&(e, _)| self.world.get::<Besieger>(e).is_some())
            .count() as u32;
        let down = original.saturating_sub(remaining);
        down * 100 >= original * SIEGE_MORALE_BREAK_PERCENT
    }

    /// A structure in melee reach of `body` with something in its output —
    /// `Stock`'s output half, `siege::offscreen::siege_steal_from_shelves`'s
    /// own target. `None` when nothing adjacent has anything to take.
    fn adjacent_stocked_structure(&self, body: Entity) -> Option<Entity> {
        let battle = self.world.get_resource::<TacticalBattle>()?;
        let cells = battle.cells_of(body);
        battle
            .bodies()
            .filter(|&(e, _)| e != body)
            .filter(|&(e, _)| self.world.get::<Structure>(e).is_some())
            .filter(|&(e, _)| {
                self.world
                    .get::<Stock>(e)
                    .is_some_and(|s| s.output_used() > 0)
            })
            .find(|&(e, _)| {
                let scells = battle.cells_of(e);
                reach::gap(&cells, &scells) <= TACTICAL_MELEE_RANGE
            })
            .map(|(e, _)| e)
    }

    /// Takes what `structure` has and makes for the door — one action,
    /// capped at `HAUL_CARRY_CAPACITY`, `Game::siege_steal_from_shelves`'s
    /// output-only rule applied to a single structure rather than a spread
    /// over every shelf in the base.
    fn besieger_steal(&mut self, body: Entity, structure: Entity) -> bool {
        let Some(round_before) = self.world.get_resource::<TacticalBattle>().map(|b| b.round)
        else {
            return false;
        };
        let Some((item, take)) = self
            .world
            .get_mut::<Stock>(structure)
            .and_then(|mut stock| {
                let (item, held) = stock.output.iter().next().map(|(i, &q)| (i.clone(), q))?;
                let take = held.min(HAUL_CARRY_CAPACITY);
                if take == 0 {
                    return None;
                }
                if take >= held {
                    stock.output.remove(&item);
                } else {
                    *stock.output.get_mut(&item).unwrap() -= take;
                }
                Some((item, take))
            })
        else {
            return false;
        };
        self.world
            .entity_mut(body)
            .insert((Carrying { item, qty: take }, StolenFrom(structure)));
        self.world.resource_mut::<TacticalBattle>().spend_action();
        self.hand_on_turn(body, round_before);
        true
    }

    /// A structure in melee reach of `body` with `Durability` and nothing
    /// worth taking — asked only once `adjacent_stocked_structure` has
    /// already come up empty, which is the whole of "steal before wreck."
    fn adjacent_wreckable_structure(&self, body: Entity) -> Option<Entity> {
        let battle = self.world.get_resource::<TacticalBattle>()?;
        let cells = battle.cells_of(body);
        battle
            .bodies()
            .filter(|&(e, _)| e != body)
            .filter(|&(e, _)| self.world.get::<Durability>(e).is_some())
            .find(|&(e, _)| {
                let scells = battle.cells_of(e);
                reach::gap(&cells, &scells) <= TACTICAL_MELEE_RANGE
            })
            .map(|(e, _)| e)
    }

    /// Swings at `structure` — Task 10's own structure branch, reused
    /// rather than restated. Nothing new about the damage; the whole of
    /// this task is the priority order above it.
    fn besieger_wreck(&mut self, body: Entity, structure: Entity) -> bool {
        let Some(round_before) = self.world.get_resource::<TacticalBattle>().map(|b| b.round)
        else {
            return false;
        };
        let dmg = self.swing_damage(body);
        let label = self.entity_label(structure);
        self.damage_structure(structure, dmg, &label, "a siege");
        // **The entity is gone, not merely its `Durability`.** A
        // `Durability`-less structure (the Home, `raidable: false`) makes
        // `damage_structure` return before ever touching that component, so
        // reading its absence as "destroyed" removes an intact structure
        // from the board on the very first swing — `Game::tactical_attack`'s
        // own copy of this check, and the same fix.
        if self.world.get::<Structure>(structure).is_none() {
            self.world
                .resource_mut::<TacticalBattle>()
                .remove(structure);
        }
        self.world.resource_mut::<TacticalBattle>().spend_action();
        self.hand_on_turn(body, round_before);
        true
    }

    /// Walks one cell toward `target` — the door, whether `body` is
    /// carrying or the pack has broken — through `Game::tactical_step`, the
    /// same door the generic AI's own walk goes through. `false` when no
    /// path exists at all, which falls through to the generic AI rather
    /// than standing still.
    ///
    /// **`target` is never itself in the field.** The Home always stands on
    /// the door — `besieger_turn`'s own reason for testing melee range
    /// rather than equality — and one body to a cell excludes an occupied
    /// cell from `reach::movement_field` entirely, so `reach::path_to`
    /// aimed at the door directly always answers empty. This walks toward
    /// whichever reachable cell this turn is strictly closer to the door
    /// than the cell already stood on — `tactical::ai::scored_cells`'
    /// closing approximation for a target too far to path to exactly,
    /// applied to a single point instead of a band — and falls through to
    /// the generic AI when nothing reachable this turn is any closer.
    fn besieger_walk_toward(&mut self, body: Entity, target: (i32, i32)) -> bool {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return false;
        };
        let Some(from) = battle.cell_of(body) else {
            return false;
        };
        if !battle.walk_planned() {
            let allowance = self.movement_allowance(body);
            let battle = self.world.resource::<TacticalBattle>();
            let field = reach::movement_field(battle, body, allowance);
            let standing = reach::distance(from, target);
            let approach = field
                .keys()
                .copied()
                .filter(|&cell| reach::distance(cell, target) < standing)
                .min_by_key(|&cell| (reach::distance(cell, target), cell.1, cell.0));
            let Some(approach) = approach else {
                return false;
            };
            let path = reach::path_to(&battle.board, &field, from, approach);
            if path.is_empty() {
                return false;
            }
            self.world
                .resource_mut::<TacticalBattle>()
                .commit_walk(path);
        }
        let battle = self.world.resource::<TacticalBattle>();
        let Some(from) = battle.cell_of(body) else {
            return false;
        };
        let Some(next) = self.world.resource_mut::<TacticalBattle>().take_walk_step() else {
            return false;
        };
        let dir = (next.0 - from.0, next.1 - from.1);
        match self.tactical_step(dir) {
            StepOutcome::Moved | StepOutcome::Departed => true,
            StepOutcome::Refused | StepOutcome::Struck => {
                if let Some(mut battle) = self.world.get_resource_mut::<TacticalBattle>() {
                    battle.commit_walk(Vec::new());
                }
                false
            }
        }
    }

    /// `body` reaches the door and leaves — `Game::tactical_step`'s own
    /// off-board departure exists for a body disengaging on foot, but a
    /// besieger's door is not reliably at the board's own edge (the flood
    /// fill's bounding box need not put `BASE_EXIT_CELL` on its boundary),
    /// so this calls the same removal the edge-step takes directly rather
    /// than depending on that geometry. **What it carried is gone** — no
    /// further bookkeeping, which is what "stolen" means once it is through
    /// the door.
    fn besieger_leaves(&mut self, body: Entity, carrying: bool) -> bool {
        let label = self.entity_label(body);
        let line = if carrying {
            format!("{label} slips out through the door with its plunder.")
        } else {
            format!("{label} withdraws.")
        };
        self.log_base_kind(MessageKind::Raid, line);
        // **`round_before` read ahead of `remove`, not through `hand_on_
        // turn`.** `body` is the acting besieger, so `TacticalBattle::
        // remove` wraps the round itself when `body` held the last rung —
        // its own doc — and nothing else on this path would otherwise ask
        // whether that happened. Skipped, a besieger leaving on the last
        // rung of a round costs that round's turret fire and its upkeep
        // entirely, with no `end_turn` anyone can see having been reached.
        let round_before = self.world.resource::<TacticalBattle>().round;
        self.world.resource_mut::<TacticalBattle>().remove(body);
        self.world.despawn(body);
        self.settle_tactical(None);
        if let Some(battle) = self.world.get_resource::<TacticalBattle>()
            && battle.round > round_before
        {
            self.skip_disengaged_turns();
            self.tactical_round_upkeep();
        }
        true
    }
}

/// The goods a besieger was carrying when it died, put back — into the
/// structure they came from if that still stands, else the nearest standing
/// Depot to take them. Called from `tactical::turn::reap_tactical_dead`,
/// gated on `Besieger`, before the body is swept off the board.
///
/// **No ground-item mechanic exists in this codebase to drop the goods "on
/// the floor" the literal way the design phrase reads**, so the fallback
/// (source destroyed) reads them into the nearest Depot rather than losing
/// them — conservation of the base's total stock is the property the
/// design's own test asks for, not the exact shelf. **Restricted to a
/// Depot and not any `Stock`-bearing structure**: every structure carries
/// one (`Game::spawn_structure` inserts it unconditionally), so a fallback
/// keyed only on its presence could hand a raider's plunder to the Home or
/// a turret — `Game::is_depot`, `StructureDef::stores`'s own door, is the
/// line between a shelf and a machine's working buffer. The *source* check
/// above needs no such restriction: it puts the exact units back exactly
/// where they were taken from, a shelf or not.
pub(crate) fn drop_besieger_cargo(game: &mut Game, body: Entity) {
    let Some(carrying) = game.world.get::<Carrying>(body).cloned() else {
        return;
    };
    let source = game.world.get::<StolenFrom>(body).map(|s| s.0);
    let target = source
        .filter(|&s| game.world.get::<Stock>(s).is_some())
        .or_else(|| nearest_stock_structure(game, body));
    let Some(target) = target else {
        return;
    };
    let mut stock = game.world.get_mut::<Stock>(target).unwrap();
    *stock.output.entry(carrying.item.clone()).or_insert(0) += carrying.qty;
    let label = game.entity_label(body);
    game.log_base_kind(MessageKind::Raid, format!("{label} drops what it stole."));
}

/// The nearest standing Depot — the fallback `drop_besieger_cargo` reaches
/// for once the source structure is gone, or was never a storage structure
/// to begin with.
fn nearest_stock_structure(game: &Game, body: Entity) -> Option<Entity> {
    let battle = game.world.get_resource::<TacticalBattle>()?;
    let cells = battle.cells_of(body);
    battle
        .bodies()
        .filter(|&(e, _)| game.is_depot(e))
        .min_by_key(|&(e, _)| {
            let scells = battle.cells_of(e);
            reach::gap(&cells, &scells)
        })
        .map(|(e, _)| e)
}
