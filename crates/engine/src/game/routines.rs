//! Installing, removing and inspecting routines — the abilities that occupy
//! a party member's slots. Extraction lives here too.

use crate::components::Routines;
use crate::*;

impl Game {
    /// Whether `item` is a loose routine rather than ordinary cargo.
    pub fn is_routine(&self, item: &ItemId) -> bool {
        self.world
            .resource::<ItemDb>()
            .get(item.as_str())
            .is_some_and(|d| d.routine.is_some())
    }

    /// `entity`'s slots in menu order, filled and empty alike.
    pub fn routine_view(&self, entity: Entity) -> Vec<RoutineSlotView> {
        let db = self.world.resource::<AbilityDb>();
        let installed = self
            .world
            .get::<Routines>(entity)
            .map(|r| r.0.clone())
            .unwrap_or_default();
        (0..self.routine_slots(entity))
            .map(
                |index| match installed.get(index).and_then(|id| db.get(id)) {
                    Some(def) => RoutineSlotView {
                        index,
                        ability: Some(def.id.clone()),
                        name: def.name.clone(),
                        description: def.description.clone(),
                    },
                    None => RoutineSlotView {
                        index,
                        ability: None,
                        name: "(empty)".to_string(),
                        description: String::new(),
                    },
                },
            )
            .collect()
    }

    /// You, then every program you own — everyone who has routine slots.
    pub fn routine_holders(&mut self) -> Vec<RoutineHolderView> {
        let player = self.player_entity();
        let level = self
            .world
            .get::<Experience>(player)
            .map(|e| e.level)
            .unwrap_or(1);
        let mut holders = vec![RoutineHolderView {
            entity: player,
            name: "You".to_string(),
            level,
            filled: self
                .world
                .get::<Routines>(player)
                .map(|r| r.0.len())
                .unwrap_or(0),
            slots: self.routine_slots(player),
        }];
        for pet in self.owned_pets() {
            holders.push(RoutineHolderView {
                entity: pet.entity,
                name: pet.name.clone(),
                level: pet.level,
                filled: self
                    .world
                    .get::<Routines>(pet.entity)
                    .map(|r| r.0.len())
                    .unwrap_or(0),
                slots: self.routine_slots(pet.entity),
            });
        }
        holders
    }

    /// Loose routines held in inventory, id-sorted so the picker's numbering
    /// is stable between sessions.
    pub fn loose_routines(&self) -> Vec<RoutineItemView> {
        let db = self.world.resource::<ItemDb>();
        let Some(inv) = self.world.get::<Inventory>(self.player_entity()) else {
            return Vec::new();
        };
        let mut rows: Vec<RoutineItemView> = inv
            .items
            .iter()
            .filter(|(_, count)| *count > 0)
            .filter_map(|(item, count)| {
                let def = db.get(item.as_str())?;
                def.routine.as_ref()?;
                Some(RoutineItemView {
                    item: item.clone(),
                    name: def.name.clone(),
                    description: def.description.clone(),
                    count: *count,
                })
            })
            .collect();
        rows.sort_by(|a, b| a.item.as_str().cmp(b.item.as_str()));
        rows
    }

    /// Spends one loose `item` and fills `entity`'s first free slot with the
    /// routine it carries. Free and unrestricted outside battle.
    pub fn install_routine(&mut self, entity: Entity, item: &ItemId) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let ability = self
            .world
            .resource::<ItemDb>()
            .get(item.as_str())
            .and_then(|d| d.routine.clone())
            .ok_or_else(|| "That isn't a routine.".to_string())?;
        let player = self.player_entity();
        if self
            .world
            .get::<Inventory>(player)
            .map(|i| i.count(item))
            .unwrap_or(0)
            == 0
        {
            return Err("You don't have that routine.".into());
        }
        let mut installed = self
            .world
            .get::<Routines>(entity)
            .map(|r| r.0.clone())
            .ok_or_else(|| "That can't hold routines.".to_string())?;
        if installed.len() >= self.routine_slots(entity) {
            return Err("There's no free routine slot — pop one out first.".into());
        }
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(item.clone(), 1);
        installed.push(ability.clone());
        self.world.entity_mut(entity).insert(Routines(installed));
        let name = self.routine_holder_label(entity);
        let ability_name = self
            .world
            .resource::<AbilityDb>()
            .get(&ability)
            .map(|a| a.name.clone())
            .unwrap_or(ability);
        self.log(format!("{name} now runs {ability_name}."));
        Ok(())
    }

    /// Frees `slot` and returns its routine to inventory as an item.
    ///
    /// Checked for cargo room *before* the slot is cleared, for the reason
    /// `sell_item` documents about its own ordering: discovering there was
    /// no room afterwards would eat the routine.
    pub fn uninstall_routine(&mut self, entity: Entity, slot: usize) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let installed = self
            .world
            .get::<Routines>(entity)
            .map(|r| r.0.clone())
            .ok_or_else(|| "That can't hold routines.".to_string())?;
        let ability = installed
            .get(slot)
            .cloned()
            .ok_or_else(|| "That slot is empty.".to_string())?;
        let item = abilities::routine_item_id(&ability);
        self.check_room(&item, 1)?;

        let mut installed = installed;
        installed.remove(slot);
        self.world.entity_mut(entity).insert(Routines(installed));
        self.world
            .get_mut::<Inventory>(self.player_entity())
            .unwrap()
            .add(item, 1);
        let name = self.routine_holder_label(entity);
        self.log(format!("{name} stops running that routine."));
        Ok(())
    }

    /// "You" for the player, the program's display name otherwise — the one
    /// place that distinction is worded, so every routine log line reads the
    /// same.
    pub(crate) fn routine_holder_label(&self, entity: Entity) -> String {
        if entity == self.player_entity() {
            "You".to_string()
        } else {
            self.creature_label(entity)
        }
    }
}
