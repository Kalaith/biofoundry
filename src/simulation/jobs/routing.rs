//! Shared target-finding and path-setting used by every job: pick a tile
//! worth walking to, cache the path there, and set the follow-up task.

use crate::simulation::nav;
use crate::state::creatures::{Creature, Task};
use crate::state::structures::Building;
use crate::state::GameSession;
use macroquad_toolkit::grid::TilePos;

/// Path to `target` and set the follow-up task, or fall back to idle.
pub(super) fn send_to(creature: &mut Creature, session: &GameSession, target: TilePos, task: Task) {
    if creature.tile() == target || set_path(creature, session, target) {
        creature.task = task;
    } else {
        creature.task = Task::Idle;
    }
}

/// Compute and cache a walkable path. Returns false when unreachable.
pub(super) fn set_path(creature: &mut Creature, session: &GameSession, target: TilePos) -> bool {
    let Some(path) = nav::find_path(session, creature.tile(), target) else {
        return false;
    };
    creature.path = path;
    true
}

/// Nearest walkable stand tile adjacent to `target` rock, with a path.
fn reachable_stand(creature: &Creature, session: &GameSession, target: TilePos) -> Option<TilePos> {
    nav::reachable_stand(session, creature.tile(), target)
}

/// Nearest player dig designation with a reachable stand tile.
pub(super) fn nearest_dig_mark(
    creature: &Creature,
    session: &GameSession,
) -> Option<(TilePos, TilePos)> {
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

/// Grown sporewood tiles, nearest first.
pub(super) fn sporewood_sources_nearest_first(
    creature: &Creature,
    session: &GameSession,
) -> Vec<TilePos> {
    let from = creature.tile();
    let mut sources: Vec<TilePos> = session
        .sporewood_regrow
        .iter()
        .filter(|(_, regrow)| **regrow <= 0.0)
        .map(|(pos, _)| *pos)
        .collect();
    sources.sort_by_key(|p| (p.manhattan_distance(&from), p.x, p.y));
    sources
}

pub(super) fn nearest_building(
    creature: &Creature,
    session: &GameSession,
    kind: &str,
) -> Option<TilePos> {
    nearest_building_where(creature, session, kind, |_| true)
}

pub(super) fn nearest_building_where(
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
