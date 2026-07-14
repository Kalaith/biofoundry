//! Goblin smiths: a Blacksmith workstation hammers ore into ingots — the
//! first processing node, live from the early minutes. Deliberately worse
//! per-ore than the salamander smelter (labour-only, no charcoal), so the
//! charcoal chain stays the bulk upgrade. When the shop has a queued
//! production order and enough banked ingots, the smith crafts equipment
//! instead — the player's first explicit production verb.

use crate::data::GameData;
use crate::simulation::jobs::equipment::equip_effect;
use crate::simulation::jobs::routing::{nearest_building, send_to};
use crate::state::creatures::{Creature, Good, Task};
use crate::state::GameSession;

pub(super) fn tick_smith(
    creature: &mut Creature,
    session: &mut GameSession,
    data: &GameData,
    dt: f32,
    work_boost: f32,
) {
    let b = &data.balance;
    // A Smith's Hammer speeds the smith's work.
    let hammer = equip_effect(creature, data, "smith_time_mult").unwrap_or(1.0);
    match creature.task.clone() {
        Task::Idle => {
            let Some(shop) = nearest_building(creature, session, "blacksmith") else {
                return;
            };
            if creature.tile() != shop {
                send_to(creature, session, shop, Task::GoSmith(shop));
                return;
            }
            // At the anvil. Work a production order if one's queued and paid
            // for; otherwise forge ingots (which accumulate toward the order
            // or get banked when the queue is empty).
            let front = session
                .building_at(shop)
                .and_then(|s| s.orders.first().cloned());
            if let Some(item) = front {
                let cost = data
                    .equipment_def(&item)
                    .map(|e| e.cost_ingots)
                    .unwrap_or(0);
                let have = session.building_at(shop).map(|s| s.stock(Good::Ingot));
                if have.unwrap_or(0.0) >= cost as f32 {
                    if let Some(building) = session.building_at_mut(shop) {
                        building.take_stock(Good::Ingot, cost as f32);
                        building.orders.remove(0);
                    }
                    creature.task = Task::Crafting {
                        shop,
                        item,
                        remaining: b.gear_craft_time_sec * hammer,
                    };
                    return;
                }
            }
            // Forge an ingot batch when ore is on hand.
            let has_ore = session
                .building_at(shop)
                .is_some_and(|s| s.stock(Good::Ore) >= b.smith_batch_ore as f32);
            if has_ore {
                if let Some(building) = session.building_at_mut(shop) {
                    building.take_stock(Good::Ore, b.smith_batch_ore as f32);
                }
                creature.task = Task::Smithing {
                    shop,
                    remaining: b.smith_batch_time_sec * hammer,
                };
            }
            // else: nothing to do — wait at the anvil (Idle).
        }
        Task::GoSmith(_) => creature.task = Task::Idle,
        Task::Smithing { shop, remaining } => {
            let left = remaining - dt * creature.work_speed() * work_boost;
            if left > 0.0 {
                creature.task = Task::Smithing {
                    shop,
                    remaining: left,
                };
            } else {
                if let Some(building) = session.building_at_mut(shop) {
                    building.add_stock(Good::Ingot, 1.0);
                }
                session.economy.ingots_forged += 1;
                creature.task = Task::Idle;
            }
        }
        Task::Crafting {
            shop,
            item,
            remaining,
        } => {
            let left = remaining - dt * creature.work_speed() * work_boost;
            if left > 0.0 {
                creature.task = Task::Crafting {
                    shop,
                    item,
                    remaining: left,
                };
            } else {
                // Finished gear waits at the stockpile for a matching worker.
                *session.economy.gear_stock.entry(item).or_insert(0) += 1;
                creature.task = Task::Idle;
            }
        }
        _ => creature.task = Task::Idle,
    }
}
