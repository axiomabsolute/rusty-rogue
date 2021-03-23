use super::{
    Map, TileType, EntitySpawnKind, BlocksTile, CombatStats, HungerClock,
    HungerState, Monster, Hazard, IsEntityKind, MonsterBasicAI, MonsterAttackSpellcasterAI,
    MovementRoutingOptions, Name, Player, Position, Renderable, Viewshed,
    PickUpable, Useable, Castable, SpellCharges, Equippable, EquipmentSlot,
    Throwable, Targeted, TargetingKind, Untargeted, Consumable,
    ProvidesFullHealing, ProvidesFullFood, IncreasesMaxHpWhenUsed,
    InflictsDamageWhenTargeted, InflictsDamageWhenEncroachedUpon,
    InflictsFreezingWhenTargeted, InflictsBurningWhenTargeted,
    InflictsBurningWhenEncroachedUpon, InflictsFreezingWhenEncroachedUpon,
    AreaOfEffectAnimationWhenTargeted, AlongRayAnimationWhenTargeted,
    MovesToRandomPosition, SpawnsEntityInAreaWhenTargeted,
    ChanceToSpawnAdjacentEntity, ChanceToDissipate, GrantsMeleeAttackBonus,
    GrantsMeleeDefenseBonus, ProvidesFireImmunityWhenUsed,
    ProvidesChillImmunityWhenUsed, SimpleMarker, SerializeMe, MarkedBuilder,
    ElementalDamageKind, InSpellBook,
    MAP_WIDTH, random_table
};
use rltk::{RandomNumberGenerator, RGB};
use specs::prelude::*;
mod potions;
mod equipment;
mod spells;
mod monsters;

const MAX_MONSTERS_IN_ROOM: i32 = 4;
const MAX_ITEMS_IN_ROOM: i32 = 2;

// Create the player entity in a specified position. Called only once a game.
pub fn spawn_player(ecs: &mut World, px: i32, py: i32) -> Entity {
    ecs.create_entity()
        .with(Position {x: px, y: py})
        .with(Renderable {
            glyph: rltk::to_cp437('@'),
            fg: RGB::named(rltk::YELLOW),
            bg: RGB::named(rltk::BLACK),
            order: 0
        })
        .with(Player {})
        .with(Viewshed {
            visible_tiles: Vec::new(),
            range: 8,
            dirty: true,
        })
        .with(Name {
            name: "Player".to_string(),
        })
        .with(CombatStats {
            max_hp: 50,
            hp: 50,
            defense: 2,
            power: 5,
        })
        .with(HungerClock {
            state: HungerState::Normal,
            state_duration: 200,
            time: 200,
            tick_damage: 1
        })
        .marked::<SimpleMarker<SerializeMe>>()
        .build()
}

// Populate a reigon (defined by a container of map indexes) with monsters and
// items.
pub fn spawn_region(ecs: &mut World, region: &[usize], depth: i32) {
    let mut areas : Vec<usize> = Vec::from(region);
    if areas.is_empty() {return;}

    let mut monster_spawn_points: Vec<usize> = Vec::new();
    let mut item_spawn_points: Vec<usize> = Vec::new();
    {
        let mut rng = ecs.write_resource::<RandomNumberGenerator>();
        let num_monsters = rng.roll_dice(1, MAX_MONSTERS_IN_ROOM + 1) + depth;
        let num_items = rng.roll_dice(1, MAX_ITEMS_IN_ROOM + 1) + depth / 2;
        for i in 0..(num_monsters + num_items) {
            if areas.is_empty() {break;}
            let array_index = if areas.len() == 1 {
                0usize
            } else {
                (rng.roll_dice(1, areas.len() as i32) - 1) as usize
            };
            let map_idx = areas[array_index];
            if i < num_monsters {
                monster_spawn_points.push(map_idx);
            } else {
                item_spawn_points.push(map_idx)
            }
            areas.remove(array_index);
        }
    }

    for idx in monster_spawn_points.iter() {
        let (x, y) = (*idx as i32 % MAP_WIDTH, *idx as i32 / MAP_WIDTH);
        spawn_random_monster(ecs, x, y, depth);
    }
    for idx in item_spawn_points.iter() {
        let (x, y) = (*idx as i32 % MAP_WIDTH, *idx as i32 / MAP_WIDTH);
        spawn_random_item(ecs, x, y, depth);
    }
}

// Spawns a randomly chosen monster at a specified location.
#[derive(Clone, Copy)]
enum MonsterType {
    None,
    GoblinBasic,
    GoblinFirecaster,
    GoblinChillcaster,
    Orc
}
fn spawn_random_monster(ecs: &mut World, x: i32, y: i32, depth: i32) {
    let monster: Option<MonsterType>;
    {
        let mut rng = ecs.write_resource::<RandomNumberGenerator>();
        // TODO: Make this table in a less stupid place.
        monster = random_table::RandomTable::new()
            .insert(MonsterType::GoblinBasic, 20)
            .insert(MonsterType::GoblinFirecaster, 1 + depth)
            .insert(MonsterType::GoblinChillcaster, 1 + depth)
            .insert(MonsterType::Orc, 3 + 3 * (depth-1))
            .insert(MonsterType::None, 70 - depth)
            .roll(&mut rng);
    }
    match monster {
        Some(MonsterType::GoblinBasic) => monsters::goblin_basic(ecs, x, y),
        Some(MonsterType::GoblinFirecaster) => monsters::goblin_firecaster(ecs, x, y),
        Some(MonsterType::GoblinChillcaster) => monsters::goblin_chillcaster(ecs, x, y),
        Some(MonsterType::Orc) => monsters::orc_basic(ecs, x, y),
        _ => {None}
    };
}

// Spawns a randomly chosen item at a specified location.
#[derive(Clone, Copy)]
enum ItemType {
    None,
    Turnip,
    Pomegranate,
    HealthPotion,
    TeleportationPotion,
    FirePotion,
    FreezingPotion,
    Dagger,
    LeatherArmor,
    FireblastScroll,
    FireballScroll,
    IceblastScroll,
    IcespikeScroll,
}
fn spawn_random_item(ecs: &mut World, x: i32, y: i32, depth: i32) {
    let item: Option<ItemType>;
    {
        let mut rng = ecs.write_resource::<RandomNumberGenerator>();
        // TODO: Make this table in a less stupid place.
        item = random_table::RandomTable::new()
            .insert(ItemType::Turnip, 3 + depth)
            .insert(ItemType::Pomegranate, depth)
            .insert(ItemType::HealthPotion, 2 + depth)
            .insert(ItemType::TeleportationPotion, 2 + depth)
            .insert(ItemType::FirePotion, 2 + depth)
            .insert(ItemType::FreezingPotion, 2 + depth)
            .insert(ItemType::Dagger, depth)
            .insert(ItemType::LeatherArmor, depth)
            .insert(ItemType::FireblastScroll, depth)
            .insert(ItemType::FireballScroll, 1 + depth)
            .insert(ItemType::IceblastScroll, depth)
            .insert(ItemType::IcespikeScroll, 1 + depth)
            .insert(ItemType::None, 100)
            .roll(&mut rng);
    }
    match item {
        Some(ItemType::Turnip) => turnip(ecs, x, y),
        Some(ItemType::Pomegranate) => pomegranate(ecs, x, y),
        Some(ItemType::HealthPotion) => potions::health(ecs, x, y),
        Some(ItemType::TeleportationPotion) => potions::teleportation(ecs, x, y),
        Some(ItemType::FirePotion) => potions::fire(ecs, x, y),
        Some(ItemType::FreezingPotion) => potions::freezing(ecs, x, y),
        Some(ItemType::Dagger) => equipment::dagger(ecs, x, y),
        Some(ItemType::LeatherArmor) => equipment::leather_armor(ecs, x, y),
        Some(ItemType::FireblastScroll) => spells::fireblast(ecs, x, y),
        Some(ItemType::FireballScroll) => spells::fireball(ecs, x, y, 5, 2),
        Some(ItemType::IceblastScroll) => spells::iceblast(ecs, x, y),
        Some(ItemType::IcespikeScroll) => spells::icespike(ecs, x, y, 5, 2),
        Some(ItemType::None) => {None},
        None => {None}
    };
}


//----------------------------------------------------------------------------
// Hazards
//----------------------------------------------------------------------------
// Spawn a fire entity in the ecs, representing a burning tile that can spread
// and dissipate. All spawning of fire MUST use this function, since it handles
// syncronizing the map.fire array.
pub fn fire(ecs: &mut World, x: i32, y: i32, spread_chance: i32, dissipate_chance: i32) -> Option<Entity> {
    let can_spawn: bool;
    let idx: usize;
    {
        let map = ecs.fetch::<Map>();
        idx = map.xy_idx(x, y);
        can_spawn = !map.fire[idx] && map.tiles[idx] != TileType::Wall;
    }
    let entity;
    if can_spawn {
        entity = ecs.create_entity()
            .with(Position {x, y})
            .with(Renderable {
                glyph: rltk::to_cp437('^'),
                fg: RGB::named(rltk::RED),
                bg: RGB::named(rltk::ORANGE),
                order: 2,
            })
            .with(Name {name: "Fire".to_string()})
            .with(Hazard {})
            .with(IsEntityKind {
                kind: EntitySpawnKind::Fire {
                    spread_chance, dissipate_chance
                }
            })
            .with(ChanceToSpawnAdjacentEntity {
                chance: spread_chance,
                kind: EntitySpawnKind::Fire {
                    spread_chance: i32::max(0, spread_chance - 20),
                    dissipate_chance: i32::max(0, dissipate_chance + 20),
                }
            })
            .with(ChanceToDissipate {
                chance: dissipate_chance
            })
            .with(InflictsDamageWhenEncroachedUpon {
                damage: 2,
                kind: ElementalDamageKind::Fire
            })
            .with(InflictsBurningWhenEncroachedUpon {
                turns: 4, tick_damage: 2
            })
            .marked::<SimpleMarker<SerializeMe>>()
            .build();
        let mut map = ecs.fetch_mut::<Map>();
        map.fire[idx] = true;
        Some(entity)
    } else {
        None
    }
}
pub fn destroy_fire(ecs: &mut World, entity: &Entity) {
    let idx;
    { // Contain first borrow of ECS.
        let positions = ecs.read_storage::<Position>();
        let map = ecs.fetch::<Map>();
        let pos = positions.get(*entity);
        match pos {
            Some(pos) => {
                idx = map.xy_idx(pos.x, pos.y);
                if !map.fire[idx] {
                    panic!(format!(
                        "Attempted to delete fire but no fire in position {} {}.",
                        pos.x, pos.y
                    ))
                }
            }
            None => panic!("Attempted to delete fire, but fire has no position.")
        }
    }
    { // Contain second borrow of ECS.
        let mut map = ecs.fetch_mut::<Map>();
        map.fire[idx] = false;
    }
    ecs.delete_entity(*entity).expect("Unable to remove fire entity.");
}

// Spawn a chill entity in the ecs, representing freezing air that can spread
// and dissipate. All spawning of chill MUST use this function, since it handles
// syncronizing the map.chill array.
pub fn chill(ecs: &mut World, x: i32, y: i32, spread_chance: i32, dissipate_chance: i32) -> Option<Entity> {
    let can_spawn: bool;
    let idx: usize;
    {
        let map = ecs.fetch::<Map>();
        idx = map.xy_idx(x, y);
        can_spawn = !map.chill[idx] && map.tiles[idx] != TileType::Wall;
    }
    let entity;
    if can_spawn {
        entity = ecs.create_entity()
            .with(Position {x, y})
            .with(Renderable {
                fg: RGB::named(rltk::WHITE),
                bg: RGB::named(rltk::LIGHT_BLUE),
                glyph: rltk::to_cp437('*'),
                order: 2,
            })
            .with(Name {name: "Chill".to_string()})
            .with(Hazard {})
            .with(IsEntityKind {
                kind: EntitySpawnKind::Chill {
                    spread_chance, dissipate_chance
                }
            })
            .with(ChanceToSpawnAdjacentEntity {
                chance: spread_chance,
                kind: EntitySpawnKind::Chill {
                    spread_chance: i32::max(0, spread_chance - 10),
                    dissipate_chance: i32::max(0, dissipate_chance + 40),
                }
            })
            .with(ChanceToDissipate {
                chance: dissipate_chance
            })
            .with(InflictsDamageWhenEncroachedUpon {
                damage: 2,
                kind: ElementalDamageKind::Chill
            })
            .with(InflictsFreezingWhenEncroachedUpon {
                turns: 2
            })
            .marked::<SimpleMarker<SerializeMe>>()
            .build();
        let mut map = ecs.fetch_mut::<Map>();
        map.chill[idx] = true;
        Some(entity)
    } else {
        None
    }
}
pub fn destroy_chill(ecs: &mut World, entity: &Entity) {
    let idx;
    { // Contain first borrow of ECS.
        let positions = ecs.read_storage::<Position>();
        let map = ecs.fetch::<Map>();
        let pos = positions.get(*entity);
        match pos {
            Some(pos) => {
                idx = map.xy_idx(pos.x, pos.y);
                if !map.chill[idx] {
                    panic!(format!(
                        "Attempted to delete chill but no chill in position {} {}.",
                        pos.x, pos.y
                    ))
                }
            }
            None => panic!("Attempted to delete chill, but chill has no position.")
        }
    }
    { // Contain second borrow of ECS.
        let mut map = ecs.fetch_mut::<Map>();
        map.chill[idx] = false;
    }
    ecs.delete_entity(*entity).expect("Unable to remove chill entity.");
}


//----------------------------------------------------------------------------
// Food
//----------------------------------------------------------------------------
fn turnip(ecs: &mut World, x: i32, y: i32) -> Option<Entity> {
    let entity = ecs.create_entity()
        .with(Position {x, y})
        .with(Renderable {
            glyph: rltk::to_cp437(';'),
            fg: RGB::named(rltk::WHITE),
            bg: RGB::named(rltk::BLACK),
            order: 2,
        })
        .with(Name {name: "Turnip".to_string()})
        .with(PickUpable {})
        .with(Useable {})
        .with(Consumable {})
        .with(Untargeted {verb: "eats".to_string()})
        // TODO: We probably want a component for triggering the healing animation.
        .with(ProvidesFullFood {})
        .marked::<SimpleMarker<SerializeMe>>()
        .build();
        Some(entity)
}

fn pomegranate(ecs: &mut World, x: i32, y: i32) -> Option<Entity> {
    let entity = ecs.create_entity()
        .with(Position {x, y})
        .with(Renderable {
            glyph: rltk::to_cp437(';'),
            fg: RGB::named(rltk::RED),
            bg: RGB::named(rltk::BLACK),
            order: 2,
        })
        .with(Name {name: "Pomegranate".to_string()})
        .with(PickUpable {})
        .with(Useable {})
        .with(Consumable {})
        .with(Untargeted {verb: "eats".to_string()})
        // TODO: We probably want a component for triggering the healing animation.
        .with(ProvidesFullFood {})
        .with(ProvidesFullHealing {})
        .with(IncreasesMaxHpWhenUsed {amount: 5})
        .marked::<SimpleMarker<SerializeMe>>()
        .build();
        Some(entity)
}

