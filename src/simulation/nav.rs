//! Shared movement and pathfinding helpers used by workers and wildlife.

use crate::state::world::Tile;
use crate::state::GameSession;
use macroquad_toolkit::grid::TilePos;

/// Advance a position along a waypoint path at `tiles_per_sec`.
pub fn walk(x: &mut f32, y: &mut f32, path: &mut Vec<TilePos>, tiles_per_sec: f32, dt: f32) {
    let mut budget = tiles_per_sec * dt;
    while budget > 0.0 {
        let Some(&next) = path.first() else {
            return;
        };
        let target = (next.x as f32 + 0.5, next.y as f32 + 0.5);
        let dx = target.0 - *x;
        let dy = target.1 - *y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist <= budget {
            *x = target.0;
            *y = target.1;
            path.remove(0);
            budget -= dist;
        } else {
            *x += dx / dist * budget;
            *y += dy / dist * budget;
            return;
        }
    }
}

/// BFS path across walkable tiles, with the starting tile trimmed off.
pub fn find_path(session: &GameSession, from: TilePos, to: TilePos) -> Option<Vec<TilePos>> {
    let mut path = session
        .world
        .tiles
        .bfs_path(from, to, false, |_, t| t.walkable())?;
    if path.first() == Some(&from) {
        path.remove(0);
    }
    Some(path)
}

/// Nearest walkable stand tile adjacent to `target` rock, with a path
/// from `from`.
pub fn reachable_stand(session: &GameSession, from: TilePos, target: TilePos) -> Option<TilePos> {
    let mut stands: Vec<TilePos> = target
        .neighbors_4way()
        .into_iter()
        .filter(|n| session.world.tiles.get(*n).is_some_and(|t| t.walkable()))
        .collect();
    stands.sort_by_key(|p| (p.manhattan_distance(&from), p.x, p.y));
    stands
        .into_iter()
        .find(|stand| find_path(session, from, *stand).is_some())
}

/// Squared distance between two world positions (tile units).
pub fn dist_sq(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = ax - bx;
    let dy = ay - by;
    dx * dx + dy * dy
}

/// Deterministic pick of a random reachable walkable tile at least
/// `min_dist` from the spawn chamber (wild spawn points).
pub fn far_walkable_tile(session: &mut GameSession, min_dist: i32) -> Option<TilePos> {
    let spawn = session.spawn_tile();
    let mut candidates: Vec<TilePos> = session
        .world
        .tiles
        .iter_with_pos()
        .filter(|(pos, t)| t.walkable() && pos.manhattan_distance(&spawn) >= min_dist)
        .map(|(pos, _)| pos)
        .filter(|pos| find_path(session, spawn, *pos).is_some())
        .collect();
    candidates.sort_by_key(|p| (p.x, p.y));
    if candidates.is_empty() {
        return None;
    }
    let index = session.rng.below(candidates.len());
    Some(candidates[index])
}

/// A random walkable neighbor for wander behavior; None when boxed in.
pub fn random_step(session: &mut GameSession, from: TilePos) -> Option<TilePos> {
    let mut options: Vec<TilePos> = from
        .neighbors_4way()
        .into_iter()
        .filter(|n| {
            session
                .world
                .tiles
                .get(*n)
                .is_some_and(|t: &Tile| t.walkable())
        })
        .collect();
    options.sort_by_key(|p| (p.x, p.y));
    if options.is_empty() {
        return None;
    }
    let index = session.rng.below(options.len());
    Some(options[index])
}
