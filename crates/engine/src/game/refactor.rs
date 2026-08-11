//! Permanent, player-driven upgrades to a tamed program.
//!
//! Two tracks, one entry point. A **Recompile Kernel** raises a program one
//! zone tier and is refused once it has caught up with the player, which is
//! what bounds it. The **percentage buffs** raise one stat apiece and are
//! bounded instead by `MAX_COMPANION_REFACTORS`, because they come off a
//! chain rooted in a Mining Node that produces forever.

use crate::items_db::CompanionUpgradeDef;
use crate::resources::ZoneLevel;
use crate::tuning::MAX_COMPANION_REFACTORS;
use crate::*;

/// A stat raised by `pct` percent, rounded, never gaining less than a point.
///
/// The floor is load-bearing rather than defensive: `+5%` of a Drone's 3 ATK
/// rounds straight back to 3, so without it a percentage buff would do
/// nothing at all to exactly the weak programs it exists to rescue — and
/// would charge them a permanent slot for the privilege.
fn raised(old: i32, pct: f32) -> i32 {
    if pct == 0.0 {
        return old;
    }
    let scaled = (old as f32 * (1.0 + pct / 100.0)).round() as i32;
    scaled.max(old + 1)
}

/// `stats` after `upgrade` is applied — the one place a refactor's
/// arithmetic lives, so the two tracks cannot drift apart. An item may
/// legally declare both (no shipped one does; a mod could), which is the
/// other reason this is one function rather than two.
///
/// The zone bump goes first because the tracks mean different things in that
/// order: the bump is catching up with the ground you are standing on, and
/// the percentages are specialisation on top of wherever that left you.
///
/// Current HP rises by exactly the delta the maximum rose by, rather than
/// refilling. A level-up full-heals; if a refactor did too, a Recompile
/// Kernel would be the strongest healing item in the game and would be
/// carried into fights for that instead of for what it is.
///
/// `tier` is the program's tier *before* the bump, because the step from one
/// tier to the next comes from `ZoneLevel::tier_step` rather than from a
/// constant restated here — the spawner's curve is what a bump is catching
/// the program up with, so the two have to be one formula.
fn refactored(stats: &Stats, upgrade: &CompanionUpgradeDef, tier: u32) -> Stats {
    let mut max_hp = stats.max_hp;
    let (mut atk, mut def) = (stats.atk, stats.def);
    if upgrade.zone_bump {
        let step = ZoneLevel::tier_step(tier);
        max_hp *= step;
        atk *= step;
        def *= step;
    }
    max_hp = raised(max_hp, upgrade.hp_percent);
    atk = raised(atk, upgrade.atk_percent);
    def = raised(def, upgrade.def_percent);
    Stats {
        hp: stats.hp + (max_hp - stats.max_hp),
        max_hp,
        atk,
        def,
    }
}

impl Game {
    /// Every upgrade item the player is carrying, id-sorted so the menu
    /// numbering is stable across sessions the way the research tree's is.
    ///
    /// Cargo rather than the whole item set: an upgrade the player has not
    /// found yet is not a choice, and listing it would put the one refusal
    /// the screen can prevent in front of them as a row to press.
    pub fn companion_upgrades(&self) -> Vec<UpgradeOption> {
        let player = self.player_entity();
        let inventory = self.world.get::<Inventory>(player).unwrap();
        let db = self.world.resource::<ItemDb>();
        let mut rows: Vec<UpgradeOption> = inventory
            .items
            .iter()
            .filter(|(_, qty)| *qty > 0)
            .filter_map(|(item, qty)| {
                let def = db.get(item.as_str())?;
                let upgrade = def.upgrade?;
                Some(UpgradeOption {
                    item: item.clone(),
                    name: def.name.clone(),
                    description: def.description.clone(),
                    qty: *qty,
                    zone_bump: upgrade.zone_bump,
                })
            })
            .collect();
        rows.sort_by(|a, b| a.item.as_str().cmp(b.item.as_str()));
        rows
    }

    /// Spends one `item` to permanently upgrade `target`.
    ///
    /// The item is taken **last**, once every refusal has had its chance —
    /// the same ordering `install_routine` states about its Routine Disk and
    /// `use_symlink` about dropping the locale. Nothing here may consume the
    /// item on a path that then fails.
    ///
    /// The two tracks deliberately do not share a pool: a program that has
    /// spent all five upgrade slots can still be bumped, because a player
    /// forced to burn permanent slots merely staying level with their zone
    /// has had the feature taken away at the point it was meant to help.
    pub fn refactor_companion(&mut self, target: Entity, item: &ItemId) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if self
            .world
            .get::<Tamed>(target)
            .is_none_or(|t| t.owner != self.player_entity())
        {
            return Err("You don't control that program.".into());
        }
        let def = self
            .item_def(item)
            .ok_or_else(|| "No such item.".to_string())?;
        let Some(upgrade) = def.upgrade else {
            return Err(format!("{} can't refactor a program.", def.name));
        };

        let tier = self.zone_tier(target);
        if upgrade.zone_bump {
            let zone = self.world.resource::<ZoneLevel>().0;
            if tier >= zone {
                return Err(format!(
                    "{} is already current for this zone.",
                    self.entity_label(target)
                ));
            }
        }
        let spent = self
            .world
            .get::<Refactors>(target)
            .copied()
            .unwrap_or_default()
            .0;
        if upgrade.spends_a_slot() && spent >= MAX_COMPANION_REFACTORS {
            return Err(format!(
                "{} has no upgrade slots left ({MAX_COMPANION_REFACTORS} of {MAX_COMPANION_REFACTORS} spent).",
                self.entity_label(target)
            ));
        }

        let player = self.player_entity();
        let mut inventory = self.world.get_mut::<Inventory>(player).unwrap();
        if inventory.count(item) == 0 {
            return Err(format!("You have no {}.", def.name));
        }
        inventory.take(item.clone(), 1);

        let stats = *self.world.get::<Stats>(target).unwrap();
        let after = refactored(&stats, &upgrade, tier);
        *self.world.get_mut::<Stats>(target).unwrap() = after;
        if upgrade.zone_bump {
            // Recorded, not merely applied: `program_payout` divides bought
            // tiers back out, or twelve printable Core Fragments would buy a
            // permanent rise in what a trader pays. See `PurchasedTiers`.
            let bought = self.purchased_tiers(target);
            self.world
                .entity_mut(target)
                .insert((ZonePortal(tier + 1), PurchasedTiers(bought + 1)));
        }
        if upgrade.spends_a_slot() {
            self.world.entity_mut(target).insert(Refactors(spent + 1));
        }

        let name = self.entity_label(target);
        self.log(format!(
            "{name} recompiled with {} — now {} HP, {} ATK, {} DEF.",
            def.name, after.max_hp, after.atk, after.def
        ));
        Ok(())
    }
}
