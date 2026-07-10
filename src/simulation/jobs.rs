//! Creature job AI: miners, carriers, and cooks as small state machines.
//!
//! Movement is generic (walk the cached path), work runs on arrival. Paths
//! are computed once per target (toolkit BFS) and only recomputed when a
//! new target is chosen — the performance guardrail from the plan.

use crate::data::{GameData, SpeciesDef};
use crate::state::creatures::{Creature, Job, Task};
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

fn tick_miner(creature: &mut Creature, session: &mut GameSession, data: &GameData, dt: f32) {
    match creature.task.clone() {
        Task::Idle => {
            if creature.carrying > 0 {
                send_to(
                    creature,
                    session,
                    session.buildings.stockpile,
                    Task::DeliverOre,
                );
                return;
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
                creature.carrying += 1;
                if *ore == 0 {
                    // Mined-out veins open into floor, expanding the cave.
                    session.vein_ore.remove(&vein);
                    session.world.tiles.set(vein, Tile::Floor);
                }
            }
            send_to(
                creature,
                session,
                session.buildings.stockpile,
                Task::DeliverOre,
            );
        }
        Task::DeliverOre => {
            if creature.tile() == session.buildings.stockpile {
                session.economy.ore_delivered += creature.carrying;
                creature.carrying = 0;
            }
            creature.task = Task::Idle;
        }
        _ => creature.task = Task::Idle,
    }
}

fn tick_carrier(
    creature: &mut Creature,
    session: &mut GameSession,
    data: &GameData,
    species: &SpeciesDef,
    dt: f32,
) {
    match creature.task.clone() {
        Task::Idle => {
            if creature.carrying >= species.carry_capacity {
                send_to(
                    creature,
                    session,
                    session.buildings.cook_pot,
                    Task::DeliverMushrooms,
                );
                return;
            }
            for source in mushroom_sources_nearest_first(creature, session) {
                if set_path(creature, session, source) {
                    creature.task = Task::GoFetch(source);
                    return;
                }
            }
            if creature.carrying > 0 {
                send_to(
                    creature,
                    session,
                    session.buildings.cook_pot,
                    Task::DeliverMushrooms,
                );
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
            if source == session.buildings.farm {
                let space = species.carry_capacity - creature.carrying;
                let take = (session.economy.farm_mushrooms.floor() as u32).min(space);
                session.economy.farm_mushrooms -= take as f32;
                creature.carrying += take;
            } else if session.patch_regrow.get(&source).is_some_and(|t| *t <= 0.0) {
                session
                    .patch_regrow
                    .insert(source, data.balance.patch_regrow_sec);
                creature.carrying += 1;
            }
            creature.task = Task::Idle;
        }
        Task::DeliverMushrooms => {
            if creature.tile() == session.buildings.cook_pot {
                session.economy.pot_mushrooms += creature.carrying;
                creature.carrying = 0;
            }
            creature.task = Task::Idle;
        }
        _ => creature.task = Task::Idle,
    }
}

fn tick_cook(creature: &mut Creature, session: &mut GameSession, data: &GameData, dt: f32) {
    let pot = session.buildings.cook_pot;
    match creature.task.clone() {
        Task::Idle => {
            if creature.tile() != pot {
                send_to(creature, session, pot, Task::GoCook);
                return;
            }
            let batch = data.balance.cook_batch_mushrooms;
            if session.economy.pot_mushrooms >= batch {
                // Ingredients are claimed up front so two cooks can't share
                // one batch.
                session.economy.pot_mushrooms -= batch;
                creature.task = Task::Cooking {
                    remaining: data.balance.cook_batch_time_sec,
                };
            }
        }
        Task::GoCook => creature.task = Task::Idle,
        Task::Cooking { remaining } => {
            let left = remaining - dt * creature.work_speed();
            if left > 0.0 {
                creature.task = Task::Cooking { remaining: left };
            } else {
                session.economy.food += data.balance.cook_batch_food;
                creature.task = Task::Idle;
            }
        }
        _ => creature.task = Task::Idle,
    }
}

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

/// Nearest vein with ore left, together with a reachable stand-adjacent
/// tile. Checks candidates in deterministic nearest-first order.
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
        let mut stands: Vec<TilePos> = vein
            .neighbors_4way()
            .into_iter()
            .filter(|n| session.world.tiles.get(*n).is_some_and(|t| t.walkable()))
            .collect();
        stands.sort_by_key(|p| (p.manhattan_distance(&from), p.x, p.y));
        for stand in stands {
            if session
                .world
                .tiles
                .bfs_path(from, stand, false, |_, t| t.walkable())
                .is_some()
            {
                return Some((vein, stand));
            }
        }
    }
    None
}

/// Sources currently holding mushrooms — the farm (if stocked) and grown
/// wild patches — nearest first, deterministic tie-break.
fn mushroom_sources_nearest_first(creature: &Creature, session: &GameSession) -> Vec<TilePos> {
    let from = creature.tile();
    let mut sources: Vec<TilePos> = session
        .patch_regrow
        .iter()
        .filter(|(_, regrow)| **regrow <= 0.0)
        .map(|(pos, _)| *pos)
        .collect();
    if session.economy.farm_mushrooms >= 1.0 {
        sources.push(session.buildings.farm);
    }
    sources.sort_by_key(|p| (p.manhattan_distance(&from), p.x, p.y));
    sources
}

fn source_has_mushrooms(session: &GameSession, source: TilePos) -> bool {
    if source == session.buildings.farm {
        session.economy.farm_mushrooms >= 1.0
    } else {
        session.patch_regrow.get(&source).is_some_and(|t| *t <= 0.0)
    }
}
