//! Creature job AI: miners, carriers, and cooks as small state machines.
//!
//! Movement is generic (walk the cached path), work runs on arrival. Paths
//! are computed once per target (toolkit BFS) and only recomputed when a
//! new target is chosen — the performance guardrail from the plan.

use crate::data::{GameData, SpeciesDef};
use crate::state::creatures::{Creature, Good, Job, Task};
use crate::state::structures::Building;
use crate::state::world::Tile;
use crate::state::GameSession;
use macroquad_toolkit::grid::TilePos;

pub fn tick_creatures(session: &mut GameSession, data: &GameData, dt: f32) {
    // Take the creature list out so each creature can mutate the rest of
    // the session (economy, veins, patches, tiles) without aliasing.
    let mut creatures = std::mem::take(&mut session.creatures);
    for creature in &mut creatures {
        tick_creature(creature, session, data, dt);
    }
    session.creatures = creatures;
}

fn tick_creature(creature: &mut Creature, session: &mut GameSession, data: &GameData, dt: f32) {
    let Some(species) = data.species.get(&creature.species).cloned() else {
        return;
    };

    if !creature.path.is_empty() {
        walk(creature, &species, dt);
        return;
    }

    match creature.job {
        Job::Idle => {
            if creature.task != Task::Idle {
                creature.clear_task();
            }
        }
        Job::Miner => tick_miner(creature, session, data, dt),
        Job::Carrier => tick_carrier(creature, session, data, &species, dt),
        Job::Cook => tick_cook(creature, session, data, dt),
    }
}

/// Advance along the path at species speed scaled by the brownout curve.
fn walk(creature: &mut Creature, species: &SpeciesDef, dt: f32) {
    let mut budget = species.move_tiles_per_sec * creature.work_speed() * dt;
    while budget > 0.0 {
        let Some(&next) = creature.path.first() else {
            return;
        };
        let target = (next.x as f32 + 0.5, next.y as f32 + 0.5);
        let dx = target.0 - creature.x;
        let dy = target.1 - creature.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist <= budget {
            creature.x = target.0;
            creature.y = target.1;
            creature.path.remove(0);
            budget -= dist;
        } else {
            creature.x += dx / dist * budget;
            creature.y += dy / dist * budget;
            return;
        }
    }
}

// ---------------------------------------------------------------- miners

fn tick_miner(creature: &mut Creature, session: &mut GameSession, data: &GameData, dt: f32) {
    match creature.task.clone() {
        Task::Idle => {
            if creature.carried(Good::Ore) > 0 {
                send_to(creature, session, session.stockpile_pos(), Task::DeliverOre);
                return;
            }
            // Player dig designations first, then ore veins.
            if let Some((mark, stand)) = nearest_dig_mark(creature, session) {
                if set_path(creature, session, stand) {
                    creature.task = Task::GoDig(mark);
                    return;
                }
            }
            if let Some((vein, stand)) = nearest_minable_vein(creature, session) {
                if set_path(creature, session, stand) {
                    creature.task = Task::GoMine(vein);
                }
            }
        }
        Task::GoMine(vein) => {
            let adjacent = creature.tile().manhattan_distance(&vein) == 1;
            if adjacent && session.vein_ore.get(&vein).is_some_and(|ore| *ore > 0) {
                creature.task = Task::Mining {
                    vein,
                    remaining: data.balance.mine_time_sec,
                };
            } else {
                creature.task = Task::Idle;
            }
        }
        Task::Mining { vein, remaining } => {
            let left = remaining - dt * creature.work_speed();
            if left > 0.0 {
                creature.task = Task::Mining {
                    vein,
                    remaining: left,
                };
                return;
            }
            if let Some(ore) = session.vein_ore.get_mut(&vein) {
                *ore = ore.saturating_sub(1);
                creature.add_carried(Good::Ore, 1);
                if *ore == 0 {
                    // Mined-out veins open into floor, expanding the cave.
                    session.vein_ore.remove(&vein);
                    session.world.tiles.set(vein, Tile::Floor);
                }
            }
            send_to(creature, session, session.stockpile_pos(), Task::DeliverOre);
        }
        Task::GoDig(mark) => {
            let adjacent = creature.tile().manhattan_distance(&mark) == 1;
            if adjacent && session.dig_marks.contains(&mark) {
                creature.task = Task::Digging {
                    mark,
                    remaining: data.balance.dig_time_sec,
                };
            } else {
                creature.task = Task::Idle;
            }
        }
        Task::Digging { mark, remaining } => {
            let left = remaining - dt * creature.work_speed();
            if left > 0.0 {
                creature.task = Task::Digging {
                    mark,
                    remaining: left,
                };
                return;
            }
            session.dig_marks.remove(&mark);
            // Carving through a vein salvages one ore.
            if session.vein_ore.remove(&mark).is_some() {
                creature.add_carried(Good::Ore, 1);
            }
            session.world.tiles.set(mark, Tile::Floor);
            creature.task = Task::Idle;
        }
        Task::DeliverOre => {
            if creature.tile() == session.stockpile_pos() {
                let n = creature.take_carried(Good::Ore, u32::MAX);
                session.economy.ore_stock += n;
                session.economy.ore_delivered_total += n;
            }
            creature.task = Task::Idle;
        }
        _ => creature.task = Task::Idle,
    }
}

// -------------------------------------------------------------- carriers

fn tick_carrier(
    creature: &mut Creature,
    session: &mut GameSession,
    data: &GameData,
    species: &SpeciesDef,
    dt: f32,
) {
    match creature.task.clone() {
        Task::Idle => {
            // Construction ore on the back goes to a site (or back home).
            if creature.carried(Good::Ore) > 0 {
                if let Some(site) = nearest_hungry_site(creature, session) {
                    send_to(creature, session, site, Task::DeliverBuildMaterial(site));
                } else {
                    send_to(creature, session, session.stockpile_pos(), Task::DeliverOre);
                }
                return;
            }
            // A full mushroom load goes to the nearest pot.
            if creature.carried(Good::Mushroom) >= species.carry_capacity {
                deliver_mushrooms(creature, session);
                return;
            }
            // Construction beats hauling food when ore is banked.
            if session.economy.ore_stock > 0 && nearest_hungry_site(creature, session).is_some() {
                send_to(
                    creature,
                    session,
                    session.stockpile_pos(),
                    Task::GoPickupOre,
                );
                return;
            }
            // Otherwise gather mushrooms, topping up a partial load.
            for source in mushroom_sources_nearest_first(creature, session) {
                if set_path(creature, session, source) {
                    creature.task = Task::GoFetch(source);
                    return;
                }
            }
            if creature.carried(Good::Mushroom) > 0 {
                deliver_mushrooms(creature, session);
            }
        }
        Task::GoFetch(source) => {
            if creature.tile() == source && source_has_mushrooms(session, source) {
                creature.task = Task::Fetching {
                    source,
                    remaining: data.balance.haul_pickup_sec,
                };
            } else {
                creature.task = Task::Idle;
            }
        }
        Task::Fetching { source, remaining } => {
            let left = remaining - dt * creature.work_speed();
            if left > 0.0 {
                creature.task = Task::Fetching {
                    source,
                    remaining: left,
                };
                return;
            }
            let space = species.carry_capacity - creature.carried(Good::Mushroom);
            if let Some(farm) = session.building_at_mut(source).filter(|b| b.kind == "farm") {
                let take = (farm.stock.floor() as u32).min(space);
                farm.stock -= take as f32;
                creature.add_carried(Good::Mushroom, take);
            } else if session.patch_regrow.get(&source).is_some_and(|t| *t <= 0.0) {
                session
                    .patch_regrow
                    .insert(source, data.balance.patch_regrow_sec);
                creature.add_carried(Good::Mushroom, 1);
            }
            creature.task = Task::Idle;
        }
        Task::DeliverMushrooms(pot) => {
            if creature.tile() == pot {
                let n = creature.take_carried(Good::Mushroom, u32::MAX);
                if let Some(building) = session.building_at_mut(pot) {
                    building.stock += n as f32;
                }
            }
            creature.task = Task::Idle;
        }
        Task::GoPickupOre => {
            if creature.tile() == session.stockpile_pos() && session.economy.ore_stock > 0 {
                creature.task = Task::PickingUpOre {
                    remaining: data.balance.haul_pickup_sec,
                };
            } else {
                creature.task = Task::Idle;
            }
        }
        Task::PickingUpOre { remaining } => {
            let left = remaining - dt * creature.work_speed();
            if left > 0.0 {
                creature.task = Task::PickingUpOre { remaining: left };
                return;
            }
            let needed: u32 = session.build_sites.iter().map(|s| s.remaining()).sum();
            let take = species
                .carry_capacity
                .min(session.economy.ore_stock)
                .min(needed);
            session.economy.ore_stock -= take;
            creature.add_carried(Good::Ore, take);
            creature.task = Task::Idle;
        }
        Task::DeliverBuildMaterial(pos) => {
            if creature.tile() == pos {
                if let Some(site) = session.build_sites.iter_mut().find(|s| s.pos == pos) {
                    let deliver = creature.take_carried(Good::Ore, site.remaining());
                    site.ore_delivered += deliver;
                }
                complete_finished_sites(session);
            }
            creature.task = Task::Idle;
        }
        _ => creature.task = Task::Idle,
    }
}

fn deliver_mushrooms(creature: &mut Creature, session: &mut GameSession) {
    if let Some(pot) = nearest_building(creature, session, "cook_pot") {
        send_to(creature, session, pot, Task::DeliverMushrooms(pot));
    }
}

/// Turn fully-supplied ghosts into working buildings.
fn complete_finished_sites(session: &mut GameSession) {
    let mut i = 0;
    while i < session.build_sites.len() {
        if session.build_sites[i].complete() {
            let site = session.build_sites.remove(i);
            session.buildings.push(Building::new(&site.kind, site.pos));
        } else {
            i += 1;
        }
    }
}

// ----------------------------------------------------------------- cooks

fn tick_cook(creature: &mut Creature, session: &mut GameSession, data: &GameData, dt: f32) {
    let batch = data.balance.cook_batch_mushrooms;
    match creature.task.clone() {
        Task::Idle => {
            // Work the nearest pot with a full batch waiting.
            let stocked =
                nearest_building_where(creature, session, "cook_pot", |b| b.stock >= batch as f32);
            if let Some(pot) = stocked {
                if creature.tile() == pot {
                    if let Some(building) = session.building_at_mut(pot) {
                        // Ingredients are claimed up front so two cooks
                        // can't share one batch.
                        building.stock -= batch as f32;
                        creature.task = Task::Cooking {
                            pot,
                            remaining: data.balance.cook_batch_time_sec,
                        };
                    }
                } else {
                    send_to(creature, session, pot, Task::GoCook(pot));
                }
                return;
            }
            // Nothing to cook: wait at the nearest pot.
            if let Some(pot) = nearest_building(creature, session, "cook_pot") {
                if creature.tile() != pot {
                    send_to(creature, session, pot, Task::GoCook(pot));
                }
            }
        }
        Task::GoCook(_) => creature.task = Task::Idle,
        Task::Cooking { pot, remaining } => {
            let left = remaining - dt * creature.work_speed();
            if left > 0.0 {
                creature.task = Task::Cooking {
                    pot,
                    remaining: left,
                };
            } else {
                session.economy.food += data.balance.cook_batch_food;
                creature.task = Task::Idle;
            }
        }
        _ => creature.task = Task::Idle,
    }
}

// --------------------------------------------------------------- helpers

/// Path to `target` and set the follow-up task, or fall back to idle.
fn send_to(creature: &mut Creature, session: &GameSession, target: TilePos, task: Task) {
    if creature.tile() == target || set_path(creature, session, target) {
        creature.task = task;
    } else {
        creature.task = Task::Idle;
    }
}

/// Compute and cache a walkable path. Returns false when unreachable.
fn set_path(creature: &mut Creature, session: &GameSession, target: TilePos) -> bool {
    let from = creature.tile();
    let Some(mut path) = session
        .world
        .tiles
        .bfs_path(from, target, false, |_, t| t.walkable())
    else {
        return false;
    };
    if path.first() == Some(&from) {
        path.remove(0);
    }
    creature.path = path;
    true
}

/// Nearest walkable stand tile adjacent to `target` rock, with a path.
fn reachable_stand(creature: &Creature, session: &GameSession, target: TilePos) -> Option<TilePos> {
    let from = creature.tile();
    let mut stands: Vec<TilePos> = target
        .neighbors_4way()
        .into_iter()
        .filter(|n| session.world.tiles.get(*n).is_some_and(|t| t.walkable()))
        .collect();
    stands.sort_by_key(|p| (p.manhattan_distance(&from), p.x, p.y));
    stands.into_iter().find(|stand| {
        session
            .world
            .tiles
            .bfs_path(from, *stand, false, |_, t| t.walkable())
            .is_some()
    })
}

/// Nearest vein with ore left, together with a reachable stand tile.
fn nearest_minable_vein(creature: &Creature, session: &GameSession) -> Option<(TilePos, TilePos)> {
    let from = creature.tile();
    let mut veins: Vec<TilePos> = session
        .vein_ore
        .iter()
        .filter(|(_, ore)| **ore > 0)
        .map(|(pos, _)| *pos)
        .collect();
    veins.sort_by_key(|p| (p.manhattan_distance(&from), p.x, p.y));

    for vein in veins.into_iter().take(8) {
        if let Some(stand) = reachable_stand(creature, session, vein) {
            return Some((vein, stand));
        }
    }
    None
}

/// Nearest player dig designation with a reachable stand tile.
fn nearest_dig_mark(creature: &Creature, session: &GameSession) -> Option<(TilePos, TilePos)> {
    let from = creature.tile();
    let mut marks: Vec<TilePos> = session.dig_marks.iter().copied().collect();
    marks.sort_by_key(|p| (p.manhattan_distance(&from), p.x, p.y));

    for mark in marks.into_iter().take(8) {
        if let Some(stand) = reachable_stand(creature, session, mark) {
            return Some((mark, stand));
        }
    }
    None
}

/// Sources currently holding mushrooms — stocked farms and grown wild
/// patches — nearest first, deterministic tie-break.
fn mushroom_sources_nearest_first(creature: &Creature, session: &GameSession) -> Vec<TilePos> {
    let from = creature.tile();
    let mut sources: Vec<TilePos> = session
        .patch_regrow
        .iter()
        .filter(|(_, regrow)| **regrow <= 0.0)
        .map(|(pos, _)| *pos)
        .collect();
    sources.extend(
        session
            .buildings_of("farm")
            .filter(|b| b.stock >= 1.0)
            .map(|b| b.pos),
    );
    sources.sort_by_key(|p| (p.manhattan_distance(&from), p.x, p.y));
    sources
}

fn source_has_mushrooms(session: &GameSession, source: TilePos) -> bool {
    if let Some(building) = session.building_at(source) {
        return building.kind == "farm" && building.stock >= 1.0;
    }
    session.patch_regrow.get(&source).is_some_and(|t| *t <= 0.0)
}

fn nearest_building(creature: &Creature, session: &GameSession, kind: &str) -> Option<TilePos> {
    nearest_building_where(creature, session, kind, |_| true)
}

fn nearest_building_where(
    creature: &Creature,
    session: &GameSession,
    kind: &str,
    predicate: impl Fn(&Building) -> bool,
) -> Option<TilePos> {
    let from = creature.tile();
    session
        .buildings_of(kind)
        .filter(|b| predicate(b))
        .map(|b| b.pos)
        .min_by_key(|p| (p.manhattan_distance(&from), p.x, p.y))
}

/// Nearest build site still missing material.
fn nearest_hungry_site(creature: &Creature, session: &GameSession) -> Option<TilePos> {
    let from = creature.tile();
    session
        .build_sites
        .iter()
        .filter(|s| !s.complete() && s.remaining() > 0)
        .map(|s| s.pos)
        .min_by_key(|p| (p.manhattan_distance(&from), p.x, p.y))
}
