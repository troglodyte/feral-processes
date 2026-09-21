//! A squad's whole lifecycle outside forming it (`tactical::squads::plan`,
//! pure) and fighting as it (`Game::effective_atk`'s `Squad` arm): spawning
//! the one entity a formed set becomes, and unwinding it again once the
//! fight that formed it is over.

use crate::tactical::TacticalBattle;
use crate::*;

impl Game {
    /// The one place a squad's component list is written —
    /// `spawn_structure`'s rule. Builds a single wild `Hostile` entity out
    /// of `members`' combined stat block (the spec's table): summed
    /// `max_hp`/`hp`, summed `atk` times the formation's `swing_share`, the
    /// members' highest `mitigation` and `Rarity`, and the lead's (`members[0]`'s)
    /// routines and cooldowns — members share a species, so they know the
    /// same ones.
    ///
    /// **No world `Position` and no `Tamed`** — the same omission that
    /// keeps a `Summoned` fork out of the save: `creature_save_for` needs a
    /// `Position` and skips whatever lacks one. `Creature` is not an
    /// omission, though — `group_pack` silently drops a member with no
    /// `Creature`, and while a squad should never reach the group model, an
    /// entity that would vanish there is a trap for whoever later wires one
    /// up.
    ///
    /// **The census against `spawn_wild_creature_scaled`'s own tuple**, which
    /// is what a body on a battle map otherwise is. Four of its components
    /// are deliberately absent and one had to come across:
    ///
    /// - `StatusEffects` — **here, and not an omission.** `Game::arm_status`
    ///   does nothing at all on a body without it while `use_ability`'s
    ///   `Debuff` arm logs unconditionally, so a squad missing it reads the
    ///   line and takes the condition anyway; `Exposed` has a live effect in
    ///   `combatant_profile`, so a squad would simply have been immune to it.
    /// - `Position` and `WanderAi` — a squad exists only inside a fight, is
    ///   never on the zone map and never wanders. `Position`'s absence is
    ///   load-bearing (above); `WanderAi`'s is merely unused.
    /// - `ZonePortal` — display only (`zone_tagged_name`), and a squad's own
    ///   name is `Game::entity_label`'s.
    /// - `Potential` — both readers that could see a hostile are nemesis
    ///   naming, which defaults to `Potential::NEUTRAL` and which no squad
    ///   reaches: a nemesis is promoted from a body that survived, and a
    ///   squad never leaves the fight it formed in.
    /// - `Boss` — a boss is its own group and never folds.
    /// - `Attributes` — a squad is a stand-in for its members inside one
    ///   fight, is never inspected on a dossier and is never saved, and its
    ///   members keep their own. Nothing reads an attribute, so this is an
    ///   omission with no live consequence — but the census above claims to
    ///   enumerate the tuple, and a claim like that is only worth anything
    ///   while it is complete.
    ///
    /// `members` themselves are never touched: they keep their own
    /// `Position` and `Stats` and are never placed on the board, which is
    /// what keeps them from being targeted, drawn or given a turn while the
    /// squad stands in for them.
    pub(crate) fn spawn_squad(&mut self, members: &[Entity], formation: usize) -> Entity {
        let lead = members[0];
        let species = self
            .world
            .get::<Creature>(lead)
            .map(|c| c.species.clone())
            .unwrap_or_default();
        let glyph = self.world.get::<Glyph>(lead).copied();
        let stats: Vec<Stats> = members
            .iter()
            .filter_map(|&e| self.world.get::<Stats>(e).copied())
            .collect();
        let max_hp: i32 = stats.iter().map(|s| s.max_hp).sum();
        let hp: i32 = stats.iter().map(|s| s.hp).sum();
        let atk_sum: i32 = stats.iter().map(|s| s.atk).sum();
        let mitigation = stats.iter().map(|s| s.mitigation).max().unwrap_or(0);
        let swing_share = crate::tuning::FORMATIONS
            .get(formation)
            .map(|f| f.swing_share)
            .unwrap_or(1.0);
        let atk = ((atk_sum as f32) * swing_share).round() as i32;
        let rarity = members
            .iter()
            .filter_map(|&e| self.world.get::<Rarity>(e).copied())
            .max()
            .unwrap_or_default();
        let routines = self
            .world
            .get::<Routines>(lead)
            .cloned()
            .unwrap_or_default();
        let cooldowns = self
            .world
            .get::<AbilityCooldowns>(lead)
            .map(|c| AbilityCooldowns(c.0.clone()))
            .unwrap_or_default();

        let mut entity = self.world.spawn((
            Creature { species },
            Hostile,
            Stats {
                hp,
                max_hp,
                atk,
                mitigation,
            },
            rarity,
            routines,
            cooldowns,
            StatusEffects::default(),
            Squad {
                members: members.to_vec(),
                formation,
            },
        ));
        if let Some(glyph) = glyph {
            entity.insert(glyph);
        }
        entity.id()
    }

    /// A surviving squad leaves no trace outside the fight it formed in —
    /// the world never contains a `Squad` once a fight is over. Each
    /// remaining member's `hp` is set to its own `max_hp` times the
    /// squad's own Integrity fraction, and the squad entity is despawned.
    ///
    /// **Not damage.** No member is being hit — the squad's own remaining
    /// Integrity is being handed back out to the bodies it was drawn
    /// from — so this does not go through `Game::apply_damage`, which
    /// prices mitigation and the fumble ladder that has nothing here to
    /// price.
    ///
    /// **Floored at one, `decompile_squad`'s captured lead's rule.** A squad
    /// on its last point hands out a fraction that rounds to nothing, and a
    /// member at zero Integrity is not a corpse — nothing killed it, so it
    /// keeps its `Position` and its `Hostile` and stands on the zone map.
    /// `Game::gather_pack` does not ask `creature_alive`, so bumping one
    /// would open a fight that pays five kills for free.
    pub(crate) fn disband_squad(&mut self, squad: Entity) {
        let Some(members) = self.world.get::<Squad>(squad).map(|s| s.members.clone()) else {
            return;
        };
        let frac = self
            .world
            .get::<Stats>(squad)
            .map(|s| s.hp_fraction())
            .unwrap_or(1.0);
        for member in members {
            if let Some(mut stats) = self.world.get_mut::<Stats>(member) {
                stats.hp = ((stats.max_hp as f32) * frac).round().max(1.0) as i32;
            }
        }
        self.world.despawn(squad);
    }

    /// Disbands every squad still standing on the board — the second of the
    /// three ways one survives a fight, alongside `depart_tactical`'s own
    /// direct call for a squad that walks off the edge itself. Read right
    /// before `Game::settle_tactical` decides the fight is over for any
    /// other reason (the player down, or gone), because once that runs the
    /// `TacticalBattle` this reads is on its way out.
    pub(crate) fn disband_surviving_squads(&mut self) {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return;
        };
        let squads: Vec<Entity> = battle
            .bodies()
            .map(|(e, _)| e)
            .filter(|&e| self.world.get::<Squad>(e).is_some())
            .collect();
        for squad in squads {
            self.world.resource_mut::<TacticalBattle>().remove(squad);
            self.disband_squad(squad);
        }
    }
}
