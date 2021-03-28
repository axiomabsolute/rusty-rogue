use specs::prelude::*;
use super::{
    Viewshed, Monster, CombatStats, CanAct, MonsterBasicAI,
    MonsterAttackSpellcasterAI, MonsterClericAI, Position, Map, RoutingMap,
    WantsToMeleeAttack, WantsToUseTargeted, StatusIsFrozen, InSpellBook,
    Castable, SpellCharges, MovementRoutingOptions
};
use rltk::{Point, RandomNumberGenerator};

//----------------------------------------------------------------------------
// System for determining if a Monster can take an action this turn.
//----------------------------------------------------------------------------
pub struct MonsterCanActSystem {}

#[derive(SystemData)]
pub struct MonsterCanActSystemData<'a> {
    entities: Entities<'a>,
    monsters: ReadStorage<'a, Monster>,
    status_is_frozen: ReadStorage<'a, StatusIsFrozen>,
    can_acts: WriteStorage<'a, CanAct>
}

impl<'a> System<'a> for MonsterCanActSystem {
    type SystemData = MonsterCanActSystemData<'a>;

    fn run(&mut self, data: Self::SystemData) {

        let MonsterCanActSystemData {entities, monsters, status_is_frozen, mut can_acts} = data;

        for (entity, _monster) in (&entities, &monsters).join() {

            // Guard for frozen monsters: they cannot act.
            if let Some(_) = status_is_frozen.get(entity) {
                continue;
            }
            can_acts.insert(entity, CanAct {})
                .expect("Failed to insert CanAct component.");
        }
    }
}


//----------------------------------------------------------------------------
// System for the most basic monster AI.
//
// Monsters with this AI type are simple Melee attaackers. They attempt to chase
// down the player and will Melee attack until someone is dead.
//----------------------------------------------------------------------------
pub struct MonsterBasicAISystem {}

#[derive(SystemData)]
pub struct MonsterBasicAISystemData<'a> {
    entities: Entities<'a>,
    map: WriteExpect<'a, Map>,
    ppos: ReadExpect<'a, Point>,
    player: ReadExpect<'a, Entity>,
    monsters: ReadStorage<'a, Monster>,
    viewsheds: WriteStorage<'a, Viewshed>,
    basic_ais: WriteStorage<'a, MonsterBasicAI>,
    can_acts: WriteStorage<'a, CanAct>,
    positions: WriteStorage<'a, Position>,
    wants_melee_attack: WriteStorage<'a, WantsToMeleeAttack>,
}

impl<'a> System<'a> for MonsterBasicAISystem {

    type SystemData = MonsterBasicAISystemData<'a>;

    fn run(&mut self, data: Self::SystemData) {
        let MonsterBasicAISystemData {
            entities,
            mut map,
            ppos,
            player,
            monsters,
            mut viewsheds,
            mut basic_ais,
            mut can_acts,
            mut positions,
            mut wants_melee_attack,
        } = data;

        let iter = (
            &entities,
            &monsters,
            &mut viewsheds,
            &mut basic_ais,
            &mut positions).join();

        for (entity, _m, mut viewshed, ai, mut pos) in iter {

            // If the entity cannot act, bail out.
            if can_acts.get(entity).is_none() {
                continue
            }

            // Our decision for what to do is conditional on this data.
            let in_viewshed = viewshed.visible_tiles.contains(&*ppos);
            let keep_following = ai.do_keep_following();
            let next_to_player = rltk::DistanceAlg::Pythagoras.distance2d(
                Point::new(pos.x, pos.y),
                *ppos
            ) < 1.5;

            // Monster next to player branch:
            //   If we're already next to player, we enter into melee combat.
            if next_to_player {
                wants_melee_attack
                    .insert(entity, WantsToMeleeAttack {target: *player})
                    .expect("Failed to insert player as melee target.");
            // Monster seeking player branch:
            //   This branch is taken if the monster is currently seeking the
            //   player, i.e., the monster is currently attempting to move towards
            //   the player until they are adjacent.
            } else if in_viewshed || keep_following {
                let path = rltk::a_star_search(
                    map.xy_idx(pos.x, pos.y) as i32,
                    map.xy_idx(ppos.x, ppos.y) as i32,
                    &RoutingMap::from_map(&*map, &ai.routing_options)
                );
                if path.success && path.steps.len() > 1 {
                    let new_x = path.steps[1] as i32 % map.width;
                    let new_y = path.steps[1] as i32 / map.width;
                    move_monster(&mut map, &mut pos, new_x, new_y, &mut viewshed);
                }
                // Update our monster's propensity to keep following the player
                // when they lose visual contact. After a specified amount of
                // time, the monster will switch to idling.
                if in_viewshed {
                    ai.reset_keep_following();
                } else {
                    ai.decrement_keep_following();
                }
            // Monster idling branch.
            //   This branch is taken if the monster can not currently see the
            //   player, and are flagged to wander when the player is out of
            //   visible range.
            } else if !in_viewshed && ai.no_visibility_wander {
                let new_pos = random_adjacent_position(&map, pos);
                move_monster(&mut map, &mut pos, new_pos.0, new_pos.1, &mut viewshed)
            }
            // We're done acting, so we've used up our action for the turn.
            can_acts.remove(entity).expect("Unable to remove CanAct component.");
        }
    }
}


//----------------------------------------------------------------------------
// System for a spellcasting monster.
//
// Monsters with this AI type are attack spellcasters, i.e., they have spells
// that they will attempt to target at the player. Otherwise, they attempt to
// keep a ranged distance to the player and wait for their spells to recharge.
//----------------------------------------------------------------------------
pub struct MonsterAttackSpellcasterAISystem {}

#[derive(SystemData)]
pub struct MonsterAttackSpellcasterAISystemData<'a> {
    entities: Entities<'a>,
    map: WriteExpect<'a, Map>,
    ppos: ReadExpect<'a, Point>,
    player: ReadExpect<'a, Entity>,
    monsters: ReadStorage<'a, Monster>,
    viewsheds: WriteStorage<'a, Viewshed>,
    attack_spellcaster_ais: WriteStorage<'a, MonsterAttackSpellcasterAI>,
    can_acts: WriteStorage<'a, CanAct>,
    positions: WriteStorage<'a, Position>,
    wants_to_target: WriteStorage<'a, WantsToUseTargeted>,
    wants_to_melee: WriteStorage<'a, WantsToMeleeAttack>,
    in_spellbooks: ReadStorage<'a, InSpellBook>,
    castables: ReadStorage<'a, Castable>,
    charges: ReadStorage<'a, SpellCharges>,
}

impl<'a> System<'a> for MonsterAttackSpellcasterAISystem {

    type SystemData = MonsterAttackSpellcasterAISystemData<'a>;

    fn run(&mut self, data: Self::SystemData) {
        let MonsterAttackSpellcasterAISystemData {
            entities,
            mut map,
            ppos,
            player,
            monsters,
            mut viewsheds,
            mut attack_spellcaster_ais,
            mut can_acts,
            mut positions,
            mut wants_to_target,
            mut wants_to_melee,
            in_spellbooks,
            castables,
            charges
        } = data;

        let iter = (
            &entities,
            &monsters,
            &mut viewsheds,
            &mut attack_spellcaster_ais,
            &mut positions).join();

        for (entity, _m, mut viewshed, ai, mut pos) in iter {

            // If the entity cannot act, bail out.
            if can_acts.get(entity).is_none() {
                continue
            }

            // Our decision for what to do is conditional on this data.
            let in_viewshed = viewshed.visible_tiles.contains(&*ppos);
            let next_to_player = rltk::DistanceAlg::Pythagoras.distance2d(
                Point::new(pos.x, pos.y),
                *ppos
            ) < 1.5;
            let l_infinity_distance_to_player = i32::max(
                i32::abs(pos.x - ppos.x), i32::abs(pos.y - ppos.y)
            );
            let mut spells = (&entities, &in_spellbooks, &castables, &charges)
                .join()
                .filter(|(_spell, book, _cast, charge)|
                    book.owner == entity && charge.charges > 0
                )
                .map(|(spell, _book, _cast, _charge)| spell);
            let spell_to_cast = spells.next();
            let has_spell_to_cast = spell_to_cast.is_some();

            // Monster can cast spell branch.
            // The monster can see the player and has a spell charge to expend,
            // so they will cast the spell on the player.
            if in_viewshed && has_spell_to_cast && l_infinity_distance_to_player <= ai.distance_to_keep_away{
                if let Some(spell) = spell_to_cast {
                    wants_to_target
                        .insert(entity, WantsToUseTargeted {thing: spell, target: *ppos})
                        .expect("Could not insert WantsToUseTargeted from Monster Spellcaster AI.");
                }
            // Monster next to player branch.
            // If we're next to the player, and have no spell to cast, we'll
            // resort to melee attacks.
            } else if next_to_player {
                wants_to_melee
                    .insert(entity, WantsToMeleeAttack {target: *player})
                    .expect("Failed to insert player as melee target.");
            // Monster can see player but has no spell to cast.
            // The monster will try to keep a fixed distance from the player
            // (within spell range) until their spell recharges.
            } else if in_viewshed {
                let target_idx = get_position_at_range_from_player (
                    &*map,
                    &*ppos,
                    pos,
                    &ai.routing_options,
                    ai.distance_to_keep_away
                );
                if let Some(target_idx) = target_idx {
                    let target_pos = map.idx_xy(target_idx);
                    move_monster(&mut map, &mut pos, target_pos.0, target_pos.1, &mut viewshed);
                }
            }
            // We're done acting, so we've used up our action for the turn.
            can_acts.remove(entity).expect("Unable to remove CanAct component.");
        }
    }
}

//----------------------------------------------------------------------------
// System for a cleric.
//
// Monsters with this AI type are healers, i.e., they have spells
// that they will attempt to target at allied monsters to keep them at full health.
// They try to position themselves near other monsters (but as far away from the
// player within that constraint), then will cast a healing spell if they see a
// monster at less than half health.
//----------------------------------------------------------------------------
pub struct MonsterClericAISystem {}

#[derive(SystemData)]
pub struct MonsterClericAISystemData<'a> {
    entities: Entities<'a>,
    map: WriteExpect<'a, Map>,
    ppos: ReadExpect<'a, Point>,
    player: ReadExpect<'a, Entity>,
    monsters: ReadStorage<'a, Monster>,
    stats: ReadStorage<'a, CombatStats>,
    viewsheds: WriteStorage<'a, Viewshed>,
    cleric_ais: WriteStorage<'a, MonsterClericAI>,
    can_acts: WriteStorage<'a, CanAct>,
    positions: WriteStorage<'a, Position>,
    wants_to_target: WriteStorage<'a, WantsToUseTargeted>,
    wants_to_melee: WriteStorage<'a, WantsToMeleeAttack>,
    in_spellbooks: ReadStorage<'a, InSpellBook>,
    castables: ReadStorage<'a, Castable>,
    charges: ReadStorage<'a, SpellCharges>,
}

impl<'a> System<'a> for MonsterClericAISystem {

    type SystemData = MonsterClericAISystemData<'a>;

    fn run(&mut self, data: Self::SystemData) {
        let MonsterClericAISystemData {
            entities,
            mut map,
            ppos,
            player,
            monsters,
            stats,
            mut viewsheds,
            mut cleric_ais,
            mut can_acts,
            mut positions,
            mut wants_to_target,
            mut wants_to_melee,
            in_spellbooks,
            castables,
            charges
        } = data;

        // A data structure to buffer movement results for each monster with
        // this AI behaviour. We need this to satisfy the borrow checker, as we
        // need to keep our mutable reference to Positions seperate from our
        // immuntable ones used during the computations below.
        let mut movement_buffer: Vec<(Entity, (i32, i32))> = Vec::new();

        let iter = (
            &entities,
            &monsters,
            &mut viewsheds,
            &mut cleric_ais,
            &positions).join();

        for (entity, _m, viewshed, ai, pos) in iter {

            // If the entity cannot act, bail out.
            if can_acts.get(entity).is_none() {
                continue
            }

            let in_viewshed = viewshed.visible_tiles.contains(&*ppos);
            let next_to_player = rltk::DistanceAlg::Pythagoras.distance2d(
                Point::new(pos.x, pos.y),
                *ppos
            ) < 1.5;
            let mut spells = (&entities, &in_spellbooks, &castables, &charges)
                .join()
                .filter(|(_spell, book, _cast, charge)|
                    book.owner == entity && charge.charges > 0
                )
                .map(|(spell, _book, _cast, _charge)| spell);
            let spell_to_cast = spells.next();

            let mut monsters_within_viewshed = (&entities, &monsters, &positions).join()
                .filter(|(_e, _m, pos)| viewshed.visible_tiles.contains(&pos.to_point()))
                .map(|(e, _m, _p)| e)
                .filter(|e| *e != entity);
            let any_monsters_within_viewshed = monsters_within_viewshed.next().is_some();

            // We want to heal any monsters we can see that are below half health.
            let mut monsters_to_heal_within_viewshed = (&entities, &monsters, &stats, &positions).join()
                .filter(|(_e, _m, _s, pos)| viewshed.visible_tiles.contains(&pos.to_point()))
                .filter(|(_e, _m, stat, _p)| stat.hp < stat.max_hp / 2)
                .map(|(e, _m, _s, _p)| e)
                .filter(|e| *e != entity);
            // TODO: Take the closest monster, smrt.
            let monster_to_heal = monsters_to_heal_within_viewshed.next();

            // Monster can cast spell branch.
            // The monster can see a valid target (in this case another monster
            // with below half hp) and has a spell charge to expend, so they
            // will cast the spell on that target.
            // TODO: We should probably check that the spell will actually hit
            // the target here, it's very possible that the monster casts the
            // spell, but the path is blocked by a wall.
            if let (Some(spell), Some(monster)) = (spell_to_cast, monster_to_heal) {
                let mpos = positions.get(monster)
                    .expect("Monster to heal has no position.");
                wants_to_target
                    .insert(entity, WantsToUseTargeted {thing: spell, target: mpos.to_point()})
                    .expect("Could not insert WantsToUseTargeted from Monster Spellcaster AI.");
            // Monster next to player branch.
            // If we're next to the player, and have no spell to cast, we'll
            // resort to melee attacks.
            } else if next_to_player {
                wants_to_melee
                    .insert(entity, WantsToMeleeAttack {target: *player})
                    .expect("Failed to insert player as melee target.");
            // Monster can see other potential targets branch.
            // The monster can see potential targets, but they are not in a
            // state where it is benificial to cast the spell (so in this case,
            // can see other monsters, but they are not below half hp). The
            // monster then moves to a positon at a range to the potential
            // targets, and chooses amongst these by moving to the possition at
            // a given range from a target that is furthest from the player.
            } else if any_monsters_within_viewshed {
                let target_idx = get_position_at_range_from_other_monsters(
                    &*map,
                    &entity,
                    &*ppos,
                    pos,
                    &entities,
                    &monsters,
                    &positions,
                    viewshed,
                    &ai.routing_options,
                    ai.distance_to_keep_away_from_monsters
                );
                if let Some(target_idx) = target_idx {
                    let flee_target_pos = map.idx_xy(target_idx);
                    movement_buffer.push((entity, flee_target_pos))
                }
            // Monster can see player but no monsters.
            // The monster will try to keep a fixed distance from the player
            // (within spell range) until their spell recharges.
            // TODO: The monster should flee here.
            } else if in_viewshed {
                let target_idx = get_position_at_range_from_player (
                    &*map,
                    &*ppos,
                    pos,
                    &ai.routing_options,
                    ai.distance_to_keep_away_from_player
                );
                if let Some(target_idx) = target_idx {
                    let target_pos = map.idx_xy(target_idx);
                    movement_buffer.push((entity, target_pos))
                }
            }
            // We're done acting, so we've used up our action for the turn.
            can_acts.remove(entity).expect("Unable to remove CanAct component.");
        }

        for (monster, (x, y)) in movement_buffer {
            let pos = positions.get_mut(monster);
            let viewshed = viewsheds.get_mut(monster);
            if let(Some(mut pos), Some(mut viewshed)) = (pos, viewshed) {
                move_monster(&mut map, &mut pos, x, y, &mut viewshed);
            }
        }
    }
}


fn get_position_at_range_from_player (
    map: &Map,
    ppos: &Point,
    pos: &Position,
    routing_options: &MovementRoutingOptions,
    distance_to_keep_away: i32,
) -> Option<usize> {
    let zero_indicies: Vec<usize> = map
        .get_l_infinity_circle_around(*ppos, distance_to_keep_away)
        .iter()
        .map(|pt| map.xy_idx(pt.x, pt.y))
        .collect();
    let routing_map = &RoutingMap::from_map(&*map, routing_options);
    let dmap = rltk::DijkstraMap::new(
        map.width,
        map.height,
        &zero_indicies,
        routing_map,
        100.0
    );
    rltk::DijkstraMap::find_lowest_exit(
        &dmap, map.xy_idx(pos.x, pos.y), routing_map
    )
}

fn get_position_at_range_from_other_monsters(
    map: &Map,
    entity: &Entity,
    ppos: &Point,
    pos: &Position,
    entities: &Entities,
    monsters: &ReadStorage<Monster>,
    positions: &WriteStorage<Position>,
    viewshed: &Viewshed,
    routing_options: &MovementRoutingOptions,
    distance_to_keep_away: i32,
) -> Option<usize> {
    let zero_indicies: Vec<usize> = (entities, monsters, positions).join()
        .filter(|(_e, _m, pos)| viewshed.visible_tiles.contains(&pos.to_point()))
        .filter(|(e, _m, _pos)| *e != *entity)
        .map(|(_e, _m, pos)| map.get_l_infinity_circle_around(
            pos.to_point(), distance_to_keep_away
        ))
        .map(|circle| {
            let mut furthest_point = Point {x: 0, y: 0};
            let mut largest_distance = 0.0;
            for pt in circle {
                let dist = rltk::DistanceAlg::Pythagoras.distance2d(pt, *ppos);
                if dist > largest_distance {
                    largest_distance = dist;
                    furthest_point = pt;
                }
            }
            furthest_point
        })
        .map(|pt| map.xy_idx(pt.x, pt.y))
        .collect();
    let routing_map = &RoutingMap::from_map(&*map, routing_options);
    let dmap = rltk::DijkstraMap::new(
        map.width,
        map.height,
        &zero_indicies,
        routing_map,
        100.0
    );
    rltk::DijkstraMap::find_lowest_exit(
        &dmap, map.xy_idx(pos.x, pos.y), routing_map
    )
}

// Move a monster to a new postions.
fn move_monster(map: &mut Map, pos: &mut Position, newposx: i32, newposy: i32, viewshed: &mut Viewshed) {
    let new_idx = map.xy_idx(newposx, newposy);
    let old_idx = map.xy_idx(pos.x, pos.y);
    if map.blocked[new_idx] {
        return
    }
    // We need to update the blocking information *now*, since we do
    // not want later monsters in the move queue to move into the
    // same position as this monster.
    map.blocked[old_idx] = false;
    map.blocked[new_idx] = true;
    pos.x = newposx;
    pos.y = newposy;
    viewshed.dirty = true;
}

// Return a random adjcaent position to pos that is not currently blocked.
// TODO: This should use the general functions we introduced in Map.
fn random_adjacent_position(map: &Map, pos: &Position) -> (i32, i32) {
    // TODO: This should use the game's internal RNG and probably belongs in
    // Map, not here.
    let mut rng = RandomNumberGenerator::new();
    let dx = rng.range(-1, 2);
    let dy = rng.range(-1, 2);
    let idx = map.xy_idx(pos.x + dx, pos.y + dy);
    if !map.blocked[idx] {
        return (pos.x + dx, pos.y + dy)
    } else {
        return (pos.x, pos.y)
    }
}