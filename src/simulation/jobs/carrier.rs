//! Carriers: the warren's belts. They pick a chain by the state of the
//! larder (the load-shedding rule of the food grid) and shuttle goods.

use crate::data::{GameData, SpeciesDef};
use crate::simulation::jobs::equipment::carry_capacity;
use crate::simulation::jobs::hauling::{
    drop_load, fetchable_good, harvest_source, ore_destination, ore_wanted,
};
use crate::simulation::jobs::routing::{
    nearest_building, nearest_building_where, send_to, set_path, sporewood_sources_nearest_first,
};
use crate::state::creatures::{Creature, Good, Task};
use crate::state::GameSession;
use macroquad_toolkit::grid::TilePos;

pub(super) fn tick_carrier(
    creature: &mut Creature,
    session: &mut GameSession,
    data: &GameData,
    species: &SpeciesDef,
    dt: f32,
    work_boost: f32,
) {
    match creature.task.clone() {
        Task::Idle => choose_carrier_work(creature, session, data, species),
        Task::GoFetch(source) => {
            if creature.tile() == source && fetchable_good(session, source).is_some() {
                creature.task = Task::Fetching {
                    source,
                    remaining: data.balance.haul_pickup_sec,
                };
            } else {
                creature.task = Task::Idle;
            }
        }
        Task::Fetching { source, remaining } => {
            let left = remaining - dt * creature.work_speed() * work_boost;
            if left > 0.0 {
                creature.task = Task::Fetching {
                    source,
                    remaining: left,
                };
                return;
            }
            harvest_source(creature, session, data, species, source);
            // Ore (from a Mine) and ingots (from a forge) always bank at the
            // stockpile; other goods route through the normal chain.
            if creature.carried(Good::Ore) > 0 {
                send_to(creature, session, session.stockpile_pos(), Task::DeliverOre);
            } else if creature.carried(Good::Ingot) > 0 {
                send_to(
                    creature,
                    session,
                    session.stockpile_pos(),
                    Task::DeliverIngot,
                );
            } else {
                creature.task = Task::Idle;
            }
        }
        Task::DeliverTo(pos) => {
            if creature.tile() == pos {
                drop_load(creature, session, data, pos);
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
            let left = remaining - dt * creature.work_speed() * work_boost;
            if left > 0.0 {
                creature.task = Task::PickingUpOre { remaining: left };
                return;
            }
            let wanted = ore_wanted(session, data);
            let take = carry_capacity(creature, species, data)
                .min(session.economy.ore_stock)
                .min(wanted);
            session.economy.ore_stock -= take;
            creature.add_carried(Good::Ore, take);
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
        Task::DeliverIngot => {
            if creature.tile() == session.stockpile_pos() {
                let n = creature.take_carried(Good::Ingot, u32::MAX);
                session.economy.ingots_stock += n;
            }
            creature.task = Task::Idle;
        }
        _ => creature.task = Task::Idle,
    }
}

/// Carrier priorities: deliver what's on the back first; then choose a
/// chain by need — when food dips below the reserve, the kitchen outranks
/// industry (load shedding), otherwise industry (construction/smelting)
/// runs first and the food economy coasts on its buffer.
fn choose_carrier_work(
    creature: &mut Creature,
    session: &mut GameSession,
    data: &GameData,
    species: &SpeciesDef,
) {
    // 1. Deliver whatever is already carried.
    if creature.carried(Good::Ore) > 0 {
        if let Some(target) = ore_destination(creature, session, data) {
            send_to(creature, session, target, Task::DeliverTo(target));
        } else {
            send_to(creature, session, session.stockpile_pos(), Task::DeliverOre);
        }
        return;
    }
    if creature.carried(Good::Ingot) > 0 {
        send_to(
            creature,
            session,
            session.stockpile_pos(),
            Task::DeliverIngot,
        );
        return;
    }
    if creature.carried(Good::Wood) > 0 {
        if let Some(kiln) = nearest_building(creature, session, "kiln") {
            send_to(creature, session, kiln, Task::DeliverTo(kiln));
            return;
        }
    }
    if creature.carried(Good::Charcoal) > 0 {
        if let Some(den) = nearest_building(creature, session, "smelter") {
            send_to(creature, session, den, Task::DeliverTo(den));
            return;
        }
    }
    if creature.carried(Good::Mushroom) >= carry_capacity(creature, species, data) {
        if let Some(pot) = nearest_building(creature, session, "cook_pot") {
            send_to(creature, session, pot, Task::DeliverTo(pot));
        }
        return;
    }

    // 2. Priorities by the state of the larder (the load-shedding rule of
    //    the food grid, in three tiers):
    //    - crisis (below the reserve): forage everything for food, shed
    //      industry entirely;
    //    - building (reserve→comfortable): push food up from the near farm
    //      first, run finite industry, only trickle-drain the mine;
    //    - surplus (comfortable and up): the larder is safe, so bank mine
    //      ore ahead of hauling still more food.
    //    Wild patches are scattered and slow — always the last resort.
    let food = session.economy.food;
    let bal = &data.balance;
    if food < bal.carrier_food_reserve {
        if try_farm_haul(creature, session)
            || try_patch_forage(creature, session)
            || try_industry_chain(creature, session, data)
        {
            return;
        }
    } else if food < bal.carrier_food_comfortable {
        if try_industry_chain(creature, session, data)
            || try_farm_haul(creature, session)
            || try_mine_drain(creature, session)
            || try_patch_forage(creature, session)
        {
            return;
        }
    } else if try_industry_chain(creature, session, data)
        || try_mine_drain(creature, session)
        || try_farm_haul(creature, session)
        || try_patch_forage(creature, session)
    {
        return;
    }

    // 3. Nothing to start: at least finish a partial mushroom load.
    if creature.carried(Good::Mushroom) > 0 {
        if let Some(pot) = nearest_building(creature, session, "cook_pot") {
            send_to(creature, session, pot, Task::DeliverTo(pot));
        }
    }
}

/// Haul from a stocked Farm to a pot. The farm is the reliable near source;
/// on its own it out-produces upkeep, so this is the backbone of the food
/// chain. Returns true when a task was started.
fn try_farm_haul(creature: &mut Creature, session: &mut GameSession) -> bool {
    let from = creature.tile();
    let mut farms: Vec<TilePos> = session
        .buildings_of("farm")
        .filter(|b| b.stock(Good::Mushroom) >= 1.0)
        .map(|b| b.pos)
        .collect();
    farms.sort_by_key(|p| (p.manhattan_distance(&from), p.x, p.y));
    for source in farms {
        if set_path(creature, session, source) {
            creature.task = Task::GoFetch(source);
            return true;
        }
    }
    false
}

/// Forage a grown wild mushroom patch. Scattered and slow — the crisis
/// reserve the warren leans on only when the larder dips. Returns true
/// when a task was started.
fn try_patch_forage(creature: &mut Creature, session: &mut GameSession) -> bool {
    let from = creature.tile();
    let mut patches: Vec<TilePos> = session
        .patch_regrow
        .iter()
        .filter(|(_, regrow)| **regrow <= 0.0)
        .map(|(pos, _)| *pos)
        .collect();
    patches.sort_by_key(|p| (p.manhattan_distance(&from), p.x, p.y));
    for source in patches {
        if set_path(creature, session, source) {
            creature.task = Task::GoFetch(source);
            return true;
        }
    }
    false
}

/// Drain a Mine's ore buffer back to the stockpile. Returns true when a
/// task was started.
fn try_mine_drain(creature: &mut Creature, session: &mut GameSession) -> bool {
    if let Some(mine) =
        nearest_building_where(creature, session, "mine", |b| b.stock(Good::Ore) >= 1.0)
    {
        if set_path(creature, session, mine) {
            creature.task = Task::GoFetch(mine);
            return true;
        }
    }
    false
}

/// Construction ore, smelter supply, charcoal, and wood runs — the
/// *finite* industry sinks. Returns true when a task was started.
fn try_industry_chain(creature: &mut Creature, session: &mut GameSession, data: &GameData) -> bool {
    // Ore runs: build sites, blacksmiths, and hungry smelters, from the bank.
    if session.economy.ore_stock > 0 && ore_wanted(session, data) > 0 {
        send_to(
            creature,
            session,
            session.stockpile_pos(),
            Task::GoPickupOre,
        );
        return true;
    }

    // Ingot runs: forge output back to the stockpile. A blacksmith working
    // a production order keeps its ingots (they feed the craft), so only
    // drain it when its queue is empty.
    if let Some(forge) = nearest_building_where(creature, session, "blacksmith", |b| {
        b.stock(Good::Ingot) >= 1.0 && b.orders.is_empty()
    })
    .or_else(|| {
        nearest_building_where(creature, session, "smelter", |b| {
            b.stock(Good::Ingot) >= 1.0
        })
    }) {
        if set_path(creature, session, forge) {
            creature.task = Task::GoFetch(forge);
            return true;
        }
    }

    // Charcoal runs: kiln output to smelters.
    if nearest_building(creature, session, "smelter").is_some() {
        if let Some(kiln) = nearest_building_where(creature, session, "kiln", |b| {
            b.stock(Good::Charcoal) >= 1.0
        }) {
            if set_path(creature, session, kiln) {
                creature.task = Task::GoFetch(kiln);
                return true;
            }
        }
    }

    // Wood runs: groves to kilns with spare capacity.
    if nearest_building_where(creature, session, "kiln", |b| {
        b.stock(Good::Wood) < data.balance.kiln_wood_cap
    })
    .is_some()
    {
        for source in sporewood_sources_nearest_first(creature, session) {
            if set_path(creature, session, source) {
                creature.task = Task::GoFetch(source);
                return true;
            }
        }
    }
    false
}
