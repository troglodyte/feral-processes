//! The off-screen siege: what happens when the clock decides one fires and
//! the player is not home to fight it.
//!
//! Not `run_raid`'s body — a sweep is one machine taking one hit, while an
//! abstract siege has to price all four stakes at once, because a player
//! who missed one should find stores gone, machines broken and possibly
//! staff dead.

use crate::tuning::{
    SIEGE_DAMAGE_PER_POINT, SIEGE_PACK_BASE, SIEGE_PACK_MAX, SIEGE_PACK_PER_ZONE,
    SIEGE_POINTS_PER_CASUALTY, SIEGE_STAFF_DEFENSE, SIEGE_STEAL_PER_POINT,
};
use crate::*;

use super::turrets::turret_defense;

/// The size of a besieging pack at `zone` — `SIEGE_PACK_BASE` plus
/// `SIEGE_PACK_PER_ZONE` a sector, capped at `SIEGE_PACK_MAX`.
///
/// The off-screen resolution and a fought siege must field the same pack,
/// so this is the one place that decides it — Task 8 calls it rather than
/// restating the formula, or the siege you fight and the siege you miss
/// would be different sizes.
pub(crate) fn pack_size(zone: u32) -> u32 {
    (SIEGE_PACK_BASE + SIEGE_PACK_PER_ZONE * zone).min(SIEGE_PACK_MAX)
}

impl Game {
    /// Resolves a siege the player was not home for: one shortfall figure,
    /// three payouts, in this order — stores stolen, structures damaged
    /// (through `Game::damage_structure`, which is what makes one destroyed
    /// this way destroyed like any other), staff benched.
    ///
    /// Always `true` — by the time `Game::siege_check` calls this, its own
    /// "nothing to besiege" hold has already refused a tick with nothing
    /// standing, so there is always something for a shortfall to spend
    /// itself against, even if the shortfall itself turns out to be zero.
    pub(crate) fn resolve_siege_offscreen(&mut self) -> bool {
        // Posted unconditionally, even when the shortfall below turns out to
        // be zero: a siege fully held off while the player was away is still
        // worth seeing on the board (correction 2).
        self.post_alert(
            crate::alerts::AlertKind::SiegeBegun,
            "siege",
            "A siege hits the base while you're away.",
        );
        let zone = self.world.resource::<ZoneLevel>().0;
        let strength = pack_size(zone);
        let staff = self.defending_base_staff();
        let defence = self.total_raid_defense()
            + turret_defense(self)
            + staff.len() as u32 * SIEGE_STAFF_DEFENSE;
        let shortfall = strength.saturating_sub(defence);
        if shortfall == 0 {
            return true;
        }

        let stolen = self.siege_steal_from_shelves(shortfall * SIEGE_STEAL_PER_POINT);
        if stolen > 0 {
            self.log_base_kind(
                MessageKind::Raid,
                format!("Besiegers carry off {stolen} units from your stores."),
            );
        }

        // Structure damage logs itself, per structure, inside
        // `damage_structure` — nothing to add here.
        self.siege_damage_structures(shortfall * SIEGE_DAMAGE_PER_POINT);

        let casualties = (shortfall / SIEGE_POINTS_PER_CASUALTY) as usize;
        for &body in staff.iter().take(casualties) {
            let name = self.bench_or_dissolve(body);
            self.log_base_kind(
                MessageKind::Raid,
                format!("{name} falls defending the base against the siege."),
            );
        }

        true
    }

    /// Lifts up to `want` units off deployed output buffers — `Stock`'s
    /// asymmetry, so a hopper already committed to a recipe is untouched —
    /// walked in `(x, y, entity)` order for the same reason
    /// `assembler_system` sorts before it pulls: a `HashMap`-free order is
    /// what keeps this deterministic and out of `GameRng`'s way entirely.
    /// Returns how many units actually moved.
    fn siege_steal_from_shelves(&mut self, want: u32) -> u32 {
        if want == 0 {
            return 0;
        }
        let mut remaining = want;
        let mut taken = 0;
        let structures: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Position), (With<Structure>, With<Stock>)>();
            let mut ordered: Vec<(Entity, (i32, i32))> = query
                .iter(&self.world)
                .map(|(e, p)| (e, (p.x, p.y)))
                .collect();
            ordered.sort_by_key(|&(e, pos)| (pos, e));
            ordered.into_iter().map(|(e, _)| e).collect()
        };
        for structure in structures {
            if remaining == 0 {
                break;
            }
            let Some(mut stock) = self.world.get_mut::<Stock>(structure) else {
                continue;
            };
            let items: Vec<ItemId> = stock.output.keys().cloned().collect();
            for item in items {
                if remaining == 0 {
                    break;
                }
                let held = stock.output.get(&item).copied().unwrap_or(0);
                let take = held.min(remaining);
                if take == 0 {
                    continue;
                }
                if take >= held {
                    stock.output.remove(&item);
                } else {
                    *stock.output.get_mut(&item).unwrap() -= take;
                }
                remaining -= take;
                taken += take;
            }
        }
        taken
    }

    /// Spends `want` `Durability` against deployed, raidable structures —
    /// `run_raid`'s own target pool (`With<Durability>, With<Structure>`),
    /// walked in the same stable order `siege_steal_from_shelves` uses. A
    /// structure that cannot absorb the whole remaining pool is destroyed
    /// and the rest carries to the next one, which is the "spread" this
    /// pays out as.
    fn siege_damage_structures(&mut self, want: u32) {
        if want == 0 {
            return;
        }
        let mut remaining = want;
        let structures: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Position), (With<Structure>, With<Durability>)>();
            let mut ordered: Vec<(Entity, (i32, i32))> = query
                .iter(&self.world)
                .map(|(e, p)| (e, (p.x, p.y)))
                .collect();
            ordered.sort_by_key(|&(e, pos)| (pos, e));
            ordered.into_iter().map(|(e, _)| e).collect()
        };
        for structure in structures {
            if remaining == 0 {
                break;
            }
            let Some(hp) = self.world.get::<Durability>(structure).map(|d| d.hp) else {
                continue;
            };
            if hp == 0 {
                continue;
            }
            let dmg = hp.min(remaining);
            let label = self.entity_label(structure);
            self.damage_structure(structure, dmg, &label, "a siege");
            remaining -= dmg;
        }
    }
}
