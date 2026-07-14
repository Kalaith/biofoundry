//! Cooks: the kitchen node. Mushrooms in, calories on the grid out.

use crate::data::GameData;
use crate::simulation::jobs::routing::{nearest_building, nearest_building_where, send_to};
use crate::state::creatures::{Creature, Good, Task};
use crate::state::GameSession;

pub(super) fn tick_cook(
    creature: &mut Creature,
    session: &mut GameSession,
    data: &GameData,
    dt: f32,
    work_boost: f32,
) {
    let batch = data.balance.cook_batch_mushrooms;
    match creature.task.clone() {
        Task::Idle => {
            // Work the nearest pot with a full batch waiting.
            let stocked = nearest_building_where(creature, session, "cook_pot", |b| {
                b.stock(Good::Mushroom) >= batch as f32
            });
            if let Some(pot) = stocked {
                if creature.tile() == pot {
                    if let Some(building) = session.building_at_mut(pot) {
                        // Ingredients are claimed up front so two cooks
                        // can't share one batch.
                        building.take_stock(Good::Mushroom, batch as f32);
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
            let left = remaining - dt * creature.work_speed() * work_boost;
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
