//! Salamanders: the living furnace. A batch claims ore + charcoal from
//! the den; the charcoal is also the salamander's meal (diet chain).

use crate::data::GameData;
use crate::simulation::jobs::routing::{nearest_building, nearest_building_where, send_to};
use crate::state::creatures::{Creature, Good, Task};
use crate::state::GameSession;

pub(super) fn tick_smelter(
    creature: &mut Creature,
    session: &mut GameSession,
    data: &GameData,
    dt: f32,
    work_boost: f32,
) {
    let b = &data.balance;
    match creature.task.clone() {
        Task::Idle => {
            let ready = nearest_building_where(creature, session, "smelter", |den| {
                den.stock(Good::Ore) >= b.smelt_batch_ore as f32
                    && den.stock(Good::Charcoal) >= b.smelt_batch_charcoal
            });
            if let Some(den) = ready {
                if creature.tile() == den {
                    if let Some(building) = session.building_at_mut(den) {
                        building.take_stock(Good::Ore, b.smelt_batch_ore as f32);
                        building.take_stock(Good::Charcoal, b.smelt_batch_charcoal);
                        // Eating the charcoal is what feeds a salamander.
                        creature.satiation = 1.0;
                        creature.starving_for = 0.0;
                        creature.task = Task::Smelting {
                            den,
                            remaining: b.smelt_batch_time_sec,
                        };
                    }
                } else {
                    send_to(creature, session, den, Task::GoSmelt(den));
                }
                return;
            }
            // No batch ready: wait at the nearest den, and keep the furnace
            // warm on the den's own charcoal so a passing ore drought never
            // starves the living smelter (there's always spare fuel piling
            // up from the kiln — the ore is the scarce input, not the meal).
            if let Some(den) = nearest_building(creature, session, "smelter") {
                if creature.tile() != den {
                    send_to(creature, session, den, Task::GoSmelt(den));
                } else if creature.satiation < 1.0 {
                    let drain = b.salamander_hunger_drain_sec;
                    if let Some(building) = session.building_at_mut(den) {
                        if building.stock(Good::Charcoal) > 0.0 {
                            let bite = (dt / drain).min(building.stock(Good::Charcoal));
                            building.take_stock(Good::Charcoal, bite);
                            creature.satiation = (creature.satiation + dt / drain).min(1.0);
                            creature.starving_for = 0.0;
                        }
                    }
                }
            }
        }
        Task::GoSmelt(_) => creature.task = Task::Idle,
        Task::Smelting { den, remaining } => {
            let left = remaining - dt * creature.work_speed() * work_boost;
            if left > 0.0 {
                creature.task = Task::Smelting {
                    den,
                    remaining: left,
                };
            } else {
                // A forged ingot lands in the den's output buffer; carriers
                // bank it. The lifetime counter drives win 2 / the shrine.
                if let Some(building) = session.building_at_mut(den) {
                    building.add_stock(Good::Ingot, 1.0);
                }
                session.economy.ingots_forged += 1;
                creature.task = Task::Idle;
            }
        }
        _ => creature.task = Task::Idle,
    }
}
