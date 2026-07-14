//! Stateless simulation services.
//!
//! The sim advances on a fixed timestep decoupled from the render loop:
//! `Game` accumulates real frame time and calls `tick` zero or more times
//! per frame. All services take state in and mutate it explicitly — no
//! globals — so integration tests can run headless for thousands of ticks.

pub mod actions;
pub mod food;
pub mod jobs;
pub mod nav;
pub mod wildlife;

#[cfg(test)]
mod tests;

use crate::data::GameData;
use crate::state::creatures::Creature;
use crate::state::GameSession;

pub use actions::{
    try_attract_beetle, try_attract_salamander, try_breed_hobgoblin, try_breed_overseer,
    try_place_build_site,
};

/// Fixed simulation timestep in seconds (10 ticks per second).
pub const SIM_DT: f32 = 0.1;

/// Cap on ticks consumed in one frame so a long hitch can't spiral.
pub const MAX_TICKS_PER_FRAME: u32 = 10;

/// Anything the UI should announce after a tick.
#[derive(Debug, Default)]
pub struct TickReport {
    pub deserters: Vec<Creature>,
    pub won_this_tick: bool,
    pub factory_this_tick: bool,
    pub worm_this_tick: bool,
    pub wild: wildlife::WildReport,
}

/// Advance the simulation by one fixed step.
pub fn tick(session: &mut GameSession, data: &GameData) -> TickReport {
    session.tick += 1;
    let dt = SIM_DT;
    let balance = &data.balance;

    // Generators: farms grow, kilns smoulder, wild flora regrows.
    use crate::state::creatures::Good;
    let farm_cap = wildlife::farm_cap(session, data);
    for building in session.buildings.iter_mut() {
        match building.kind.as_str() {
            "farm" => {
                let grown = (building.stock(Good::Mushroom)
                    + balance.farm_mushrooms_per_min / 60.0 * dt)
                    .min(farm_cap);
                building.stocks.insert(Good::Mushroom, grown);
            }
            "kiln" => {
                let converted =
                    (balance.kiln_charcoal_per_min / 60.0 * dt).min(building.stock(Good::Wood));
                if converted > 0.0 {
                    building.take_stock(Good::Wood, converted);
                    building.add_stock(Good::Charcoal, converted);
                }
            }
            _ => {}
        }
    }
    for regrow in session.patch_regrow.values_mut() {
        *regrow = (*regrow - dt).max(0.0);
    }
    for regrow in session.sporewood_regrow.values_mut() {
        *regrow = (*regrow - dt).max(0.0);
    }

    // Workers, then the calorie ledger they drain. Jobs only ever add
    // food (cook batches), so the delta is this tick's production. Ore
    // banked and ingots forged are tracked the same way for the factory
    // dashboard (the food grid generalized to every chain).
    let food_before = session.economy.food;
    let ore_before = session.economy.ore_delivered_total;
    let ingots_before = session.economy.ingots_forged;
    jobs::tick_creatures(session, data, dt);
    let smoothing = dt / 15.0; // ~15s time constant
    let produced_per_min = ((session.economy.food - food_before) / dt * 60.0).max(0.0);
    session.economy.production_ema_per_min +=
        (produced_per_min - session.economy.production_ema_per_min) * smoothing;
    let ore_per_min = (session.economy.ore_delivered_total - ore_before) as f32 / dt * 60.0;
    session.economy.ore_ema_per_min += (ore_per_min - session.economy.ore_ema_per_min) * smoothing;
    let ingot_per_min = (session.economy.ingots_forged - ingots_before) as f32 / dt * 60.0;
    session.economy.ingot_ema_per_min +=
        (ingot_per_min - session.economy.ingot_ema_per_min) * smoothing;
    let wild = wildlife::tick_wildlife(session, data, dt);
    let deserters = food::tick_hunger(session, data, dt);

    let mut won_this_tick = false;
    if !session.won
        && session.economy.ore_delivered_total >= balance.win_ore_delivered
        && session.economy.food >= balance.win_food_surplus
    {
        session.won = true;
        won_this_tick = true;
    }

    let mut factory_this_tick = false;
    if !session.factory_complete && session.economy.ingots_forged >= balance.win2_ingots {
        session.factory_complete = true;
        factory_this_tick = true;
    }

    // The Worm Shrine: offerings drain the larder (the worm is the final
    // power draw), pausing below the reserve so feeding can't blackout
    // the warren on its own.
    let mut worm_this_tick = false;
    if !session.worm_awake && session.buildings_of("worm_shrine").next().is_some() {
        let headroom = (session.economy.food - balance.worm_feed_reserve).max(0.0);
        let bite = (balance.worm_food_per_min / 60.0 * dt).min(headroom);
        if bite > 0.0 {
            session.economy.food -= bite;
            session.worm_fed += bite;
        }
        if session.worm_fed >= balance.worm_awaken_at {
            session.worm_awake = true;
            worm_this_tick = true;
        }
    }

    TickReport {
        deserters,
        won_this_tick,
        factory_this_tick,
        worm_this_tick,
        wild,
    }
}

/// Seconds of simulated time elapsed.
pub fn sim_seconds(session: &GameSession) -> f32 {
    session.tick as f32 * SIM_DT
}
