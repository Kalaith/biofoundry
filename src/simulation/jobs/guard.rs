//! Guards: chase raiders and fight them; otherwise stand watch near the
//! stockpile (the raid target).

use crate::data::GameData;
use crate::simulation::jobs::equipment::equip_effect;
use crate::simulation::jobs::routing::{send_to, set_path};
use crate::simulation::{nav, wildlife};
use crate::state::creatures::{Creature, Task};
use crate::state::GameSession;

pub(super) fn tick_guard(
    creature: &mut Creature,
    session: &mut GameSession,
    data: &GameData,
    dt: f32,
    work_boost: f32,
) {
    match creature.task.clone() {
        Task::Idle => {
            if let Some(target) = nearest_raider(creature, session) {
                let tile = session
                    .wilds
                    .iter()
                    .find(|w| w.id == target)
                    .map(|w| w.tile());
                if let Some(tile) = tile {
                    if set_path(creature, session, tile) || creature.tile() == tile {
                        creature.task = Task::Hunt { target };
                        return;
                    }
                }
            }
            // Stand watch by the larder.
            let post = session.stockpile_pos();
            if creature.tile().manhattan_distance(&post) > 2 {
                send_to(creature, session, post, Task::Idle);
            }
        }
        Task::Hunt { target } => {
            // A Guard Blade multiplies DPS, stacking with Hardened Guards;
            // species strength and any Overseer aura fold in via work_boost.
            let blade = equip_effect(creature, data, "guard_dps_mult").unwrap_or(1.0);
            let dps =
                wildlife::guard_dps(session, data) * creature.work_speed() * work_boost * blade;
            let Some(wild) = session.wilds.iter_mut().find(|w| w.id == target) else {
                creature.task = Task::Idle;
                return;
            };
            if nav::dist_sq(creature.x, creature.y, wild.x, wild.y) <= 2.0 {
                wild.hp -= dps * dt;
                if wild.hp <= 0.0 {
                    creature.task = Task::Idle;
                }
            } else {
                // Chase: re-path to the raider's current tile.
                let tile = wild.tile();
                if !set_path(creature, session, tile) {
                    creature.task = Task::Idle;
                }
            }
        }
        _ => creature.task = Task::Idle,
    }
}

/// Nearest living raider's wild id.
fn nearest_raider(creature: &Creature, session: &GameSession) -> Option<u32> {
    let from = creature.tile();
    session
        .wilds
        .iter()
        .filter(|w| w.is_raider() && w.hp > 0.0)
        .min_by_key(|w| {
            let t = w.tile();
            (t.manhattan_distance(&from), t.x, t.y)
        })
        .map(|w| w.id)
}
