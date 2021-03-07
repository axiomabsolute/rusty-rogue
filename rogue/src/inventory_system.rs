use super::{
    GameLog, InBackpack, InSpellBook, Name, Position, WantsToPickupItem, WantsToEquipItem,
    WantsToRemoveItem, Equipped, Castable
};
use specs::prelude::*;


pub struct ItemCollectionSystem {}

// Looks for WantsToPickUp components, then tries to place the requested item it
// the owner's backpack by attaching the InBackpack component to the item.
impl<'a> System<'a> for ItemCollectionSystem {
    type SystemData = (
        ReadExpect<'a, Entity>,
        WriteExpect<'a, GameLog>,
        WriteStorage<'a, WantsToPickupItem>,
        WriteStorage<'a, Position>,
        ReadStorage<'a, Name>,
        ReadStorage<'a, Castable>,
        WriteStorage<'a, InBackpack>,
        WriteStorage<'a, InSpellBook>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (player, mut log, mut pickups, mut positions, names, castables, mut backpacks, mut spellbooks) = data;
        for pickup in pickups.join() {
            positions.remove(pickup.item);
            // Spells go in the spellbook, everything else goes in a backpack.
            let is_castable = castables.get(pickup.item).is_some();
            if is_castable {
                spellbooks
                    .insert(pickup.item, InSpellBook {owner: pickup.by})
                    .expect("Unable to insert spell into spellbook.");
            } else {
                backpacks
                    .insert(pickup.item, InBackpack {owner: pickup.by})
                    .expect("Unable to insert item in backpack.");
            }
            if pickup.by == *player {
                let name = &names.get(pickup.item);
                if let Some(name) = name {
                    log.entries.push(format!("You pickup the {}", name.name))
                }
            }
        }
        pickups.clear();
    }
}


pub struct ItemEquipSystem {}

// Searches for WantsToEquipItem compoents and then processes the results
// by attaching an Equipped component to the item. This component contains a
// reference to the equipper entity.
impl<'a> System<'a> for ItemEquipSystem {
    #[allow(clippy::type_complexity)]
    type SystemData = (
        Entities<'a>,
        WriteExpect<'a, GameLog>,
        ReadStorage<'a, Name>,
        WriteStorage<'a, WantsToEquipItem>,
        WriteStorage<'a, Equipped>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (
            entities,
            mut log,
            names,
            mut wants_equip,
            mut equipped,
        ) = data;
        // Remove any already equipped items.
        let mut already_equipped: Vec<Entity> = Vec::new();
        for (equipper, do_equip) in (&entities, &wants_equip).join() {
            already_equipped.extend(
                (&entities, &equipped)
                    .join()
                    .filter(|(_item, eqp)| eqp.owner == equipper && eqp.slot == do_equip.slot)
                    .map(|(item, _eqp)| item)
            )
        }
        for item in already_equipped {
            equipped.remove(item);
        }
        // Weild the equipment.
        for (equipper, do_equip, name) in (&entities, &wants_equip, &names).join() {
            equipped.
                insert(do_equip.item, Equipped {owner: equipper, slot: do_equip.slot})
                .expect("Failed to equip item.");
            let item_name = names.get(do_equip.item);
            if let Some(item_name) = item_name {
                log.entries.push(format!("{} equipped the {}.", name.name, item_name.name));
            }
        }
        wants_equip.clear();
    }
}


pub struct ItemRemoveSystem {}

// Searches for WantsToEquipItem compoents and then processes the results
// by attaching an Equipped component to the item. This component contains a
// reference to the equipper entity.
impl<'a> System<'a> for ItemRemoveSystem {
    #[allow(clippy::type_complexity)]
    type SystemData = (
        Entities<'a>,
        WriteExpect<'a, GameLog>,
        ReadStorage<'a, Name>,
        WriteStorage<'a, WantsToRemoveItem>,
        WriteStorage<'a, Equipped>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (
            entities,
            mut log,
            names,
            mut wants_remove,
            mut equipped,
        ) = data;

        for (remover, do_remove) in (&entities, &wants_remove).join() {
            equipped.remove(do_remove.item);
            let item_name = names.get(do_remove.item);
            let remover_name = names.get(remover);
            if let (Some(item_name), Some(remover_name)) = (item_name, remover_name) {
                log.entries.push(format!(
                    "{} reomves {}.", remover_name.name, item_name.name
                ))
            }
        }
        wants_remove.clear();
    }
}