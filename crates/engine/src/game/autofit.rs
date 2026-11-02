//! Autofit: puts the best gear in the pack on one wearer, one slot at a time.
//!
//! Gather rates every pack candidate at the level `equip` would give it and
//! the worn copy at the level it went on at; `pick_autofit` decides from
//! those numbers alone; `Game::autofit` equips the picks and ticks once.

use crate::components::Equipment;
use crate::game::gear_power::{ReferenceFoe, rate_against};
use crate::items::{EquipmentSlot, EquipmentStats, GearCopy};
use crate::*;

/// What one autofit put on, and the line it logged.
pub struct AutofitReport {
    pub equipped: Vec<(EquipmentSlot, GearCopy)>,
    pub message: String,
}

/// A copy's combat rating (`None` when it has no combat axis) and its
/// decompile chance, both from the same `copy_bonus` figure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rating {
    pub(crate) power: Option<i32>,
    pub(crate) decompiler: i32,
}

#[derive(Clone, Debug)]
pub(crate) struct Candidate {
    pub(crate) copy: GearCopy,
    pub(crate) rating: Rating,
}

/// One slot as gather saw it. `candidates` arrive in a stable order, and the
/// first of equal-ranked candidates wins.
#[derive(Clone, Debug)]
pub(crate) struct SlotInput {
    pub(crate) slot: EquipmentSlot,
    pub(crate) worn: Option<Rating>,
    pub(crate) candidates: Vec<Candidate>,
}

/// The rank a rating has in `slot`, `None` for unrated. Decompile chance only
/// does anything on the player, so only the player's module ranks by it
/// first; there an empty slot ranks as no decompile and no rating.
fn rank(rating: Option<Rating>, slot: EquipmentSlot, is_player: bool) -> Option<(i32, i32)> {
    if is_player && slot == EquipmentSlot::Module {
        let r = rating.unwrap_or(Rating {
            power: None,
            decompiler: 0,
        });
        return Some((r.decompiler, r.power.unwrap_or(i32::MIN)));
    }
    rating?.power.map(|p| (0, p))
}

/// The copies to put on, in slot order. A slot changes only when its best
/// candidate strictly outranks what is worn, so a tie never spends a turn.
pub(crate) fn pick_autofit(slots: &[SlotInput], is_player: bool) -> Vec<(EquipmentSlot, GearCopy)> {
    slots
        .iter()
        .filter_map(|input| {
            let mut best: Option<((i32, i32), &Candidate)> = None;
            for candidate in &input.candidates {
                let Some(key) = rank(Some(candidate.rating), input.slot, is_player) else {
                    continue;
                };
                if best.is_none_or(|(top, _)| key > top) {
                    best = Some((key, candidate));
                }
            }
            let (key, candidate) = best?;
            (Some(key) > rank(input.worn, input.slot, is_player))
                .then(|| (input.slot, candidate.copy.clone()))
        })
        .collect()
}

impl Game {
    /// Fits the best gear in the player's pack onto `wearer`: the player or
    /// a program they own. Refused exactly when `equip` would be. One tick
    /// and one log line however many slots change; nothing changed costs
    /// no tick.
    pub fn autofit(&mut self, wearer: Entity) -> Result<AutofitReport, String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        self.check_wearer(wearer)?;
        let is_player = wearer == self.player_entity();
        let picks = pick_autofit(&self.gather_autofit(wearer), is_player);
        if picks.is_empty() {
            let message = "Autofit: already wearing the best gear.".to_string();
            self.log(message.clone());
            return Ok(AutofitReport {
                equipped: picks,
                message,
            });
        }
        // Gather left out every slot `equip_quietly` could refuse (a worn
        // item missing from the item set) and offered only copies held in
        // the pack for that slot, so this `?` cannot fire after a slot moved.
        for (_, copy) in &picks {
            self.equip_quietly(wearer, copy)?;
        }
        let names = picks
            .iter()
            .map(|(_, copy)| self.item_name(&copy.item))
            .collect::<Vec<_>>()
            .join(", ");
        let message = if is_player {
            format!("Autofit: you equip {names}.")
        } else {
            format!("Autofit: {} equips {names}.", self.entity_label(wearer))
        };
        self.log(message.clone());
        self.tick();
        Ok(AutofitReport {
            equipped: picks,
            message,
        })
    }

    fn gather_autofit(&self, wearer: Entity) -> Vec<SlotInput> {
        let level = self.world.resource::<ZoneLevel>().0;
        let foe = ReferenceFoe::median(self.world.resource::<SpeciesDb>());
        let rate = |mods: EquipmentStats| Rating {
            power: rate_against(mods, &foe).map(|p| p.total),
            decompiler: mods.decompiler,
        };
        let equipment = self
            .world
            .get::<Equipment>(wearer)
            .cloned()
            .unwrap_or_default();
        let rows = self.player_status().inventory;
        EquipmentSlot::ALL
            .into_iter()
            .filter_map(|slot| {
                let worn = match equipment.get(slot) {
                    // A worn item missing from the item set can't be moved,
                    // so its slot is left alone rather than refused halfway.
                    Some(worn) => Some(rate(self.worn_bonus(&worn)?)),
                    None => None,
                };
                let mut candidates: Vec<Candidate> = rows
                    .iter()
                    .filter(|row| {
                        self.equipment_of(&row.copy.item).map(|(s, _)| s) == Some(slot)
                            && !self.is_broken(&row.copy)
                    })
                    .filter_map(|row| {
                        Some(Candidate {
                            copy: row.copy.clone(),
                            rating: rate(self.copy_bonus(&row.copy, level)?),
                        })
                    })
                    .collect();
                candidates.sort_by_cached_key(|c| (self.copy_name(&c.copy), c.copy.clone()));
                Some(SlotInput {
                    slot,
                    worn,
                    candidates,
                })
            })
            .collect()
    }
}
