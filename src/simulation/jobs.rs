//! Creature job AI: miners, carriers, cooks, and salamander smelters as
//! small state machines.
//!
//! Movement is generic (walk the cached path), work runs on arrival. Paths
//! are computed once per target (toolkit BFS) and only recomputed when a
//! new target is chosen — the performance guardrail from the plan.

use crate::data::{GameData, SpeciesDef};
use crate::simulation::{nav, wildlife};
use crate::state::creatures::{Creature, Good, Job, Task};
use crate::state::structures::Building;
use crate::state::world::Tile;
use crate::state::GameSession;
use macroquad_toolkit::grid::TilePos;
use std::collections::HashMap;

/// Live count of miners claiming each Mine (walking to or working it), so
/// staffing respects the workstation's slot limit even though the creature
/// list is taken out of the session during the tick.
type MineClaims = HashMap<TilePos, u32>;

pub fn tick_creatures(session: &mut GameSession, data: &GameData, dt: f32) {
    // Take the creature list out so each creature can mutate the rest of
    // the session (economy, veins, patches, tiles) without aliasing.
    let mut creatures = std::mem::take(&mut session.creatures);

    // Snapshot Mine slot claims from current tasks; miners deciding this
    // tick read and update it so they spread across open slots.
    let mut claims: MineClaims = HashMap::new();
    for c in &creatures {
        if let Task::GoMine(p) | Task::WorkMine(p) = &c.task {
            *claims.entry(*p).or_insert(0) += 1;
        }
    }

    for creature in &mut creatures {
        tick_creature(creature, session, data, dt, &mut claims);
    }
    session.creatures = creatures;
}

fn tick_creature(
    creature: &mut Creature,
    session: &mut GameSession,
    data: &GameData,
    dt: f32,
    claims: &mut MineClaims,
) {
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
        Job::Miner => tick_miner(creature, session, data, dt, claims),
        Job::Carrier => tick_carrier(creature, session, data, &species, dt),
        Job::Cook => tick_cook(creature, session, data, dt),
        Job::Smith => tick_smith(creature, session, data, dt),
        Job::Guard => tick_guard(creature, session, data, dt),
        Job::Smelter => tick_smelter(creature, session, data, dt),
    }
}

/// Advance along the path at species speed scaled by the brownout curve.
fn walk(creature: &mut Creature, species: &SpeciesDef, dt: f32) {
    let speed = species.move_tiles_per_sec * creature.work_speed();
    nav::walk(
        &mut creature.x,
        &mut creature.y,
        &mut creature.path,
        speed,
        dt,
    );
}

// ---------------------------------------------------------------- miners

fn tick_miner(
    creature: &mut Creature,
    session: &mut GameSession,
    data: &GameData,
    dt: f32,
    claims: &mut MineClaims,
) {
    match creature.task.clone() {
        Task::Idle => decide_miner(creature, session, data, claims),
        Task::GoMine(mine) => {
            // Arrived at the Mine tile: take the post if it's still valid.
            let ready = creature.tile() == mine
                && session
                    .building_at(mine)
                    .is_some_and(|b| b.kind == "mine" && b.reserve > 0.0);
            if ready {
                creature.task = Task::WorkMine(mine);
            } else {
                release_mine(claims, mine);
                creature.task = Task::Idle;
            }
        }
        Task::WorkMine(mine) => {
            // Yield the post to expansion digging (finite, player-directed).
            if nearest_dig_mark(creature, session).is_some() {
                release_mine(claims, mine);
                creature.task = Task::Idle;
                return;
            }
            let cap = data.balance.mine_buffer_cap;
            let rate = data.balance.mine_ore_per_min;
            let Some(b) = session.building_at_mut(mine) else {
                release_mine(claims, mine);
                creature.task = Task::Idle;
                return;
            };
            if b.kind != "mine" || b.reserve <= 0.0 {
                release_mine(claims, mine);
                creature.task = Task::Idle;
                return;
            }
            // Extract into the local buffer, drawing down the deposit. A
            // full buffer stalls the miner in place (output backed up) —
            // it resumes when carriers drain it.
            let headroom = (cap - b.stock(Good::Ore)).max(0.0);
            let amount = (rate / 60.0 * dt * creature.work_speed())
                .min(b.reserve)
                .min(headroom);
            if amount > 0.0 {
                b.reserve -= amount;
                b.add_stock(Good::Ore, amount);
            }
            // Stay stationed (task unchanged).
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

/// An idle miner banks any salvaged ore, answers dig designations, then
/// claims the nearest open Mine slot.
fn decide_miner(
    creature: &mut Creature,
    session: &mut GameSession,
    data: &GameData,
    claims: &mut MineClaims,
) {
    if creature.carried(Good::Ore) > 0 {
        send_to(creature, session, session.stockpile_pos(), Task::DeliverOre);
        return;
    }
    // Expansion digging first: it's finite and player-directed.
    if let Some((mark, stand)) = nearest_dig_mark(creature, session) {
        if set_path(creature, session, stand) {
            creature.task = Task::GoDig(mark);
            return;
        }
    }
    if let Some(mine) = nearest_open_mine(creature, session, data, claims) {
        if creature.tile() == mine || set_path(creature, session, mine) {
            *claims.entry(mine).or_insert(0) += 1;
            creature.task = Task::GoMine(mine);
            return;
        }
    }
    creature.task = Task::Idle;
}

/// Release one slot claim on a Mine (miner left the post).
fn release_mine(claims: &mut MineClaims, mine: TilePos) {
    if let Some(count) = claims.get_mut(&mine) {
        *count = count.saturating_sub(1);
    }
}

/// Nearest Mine with a free slot and ore left in the deposit, reachable
/// from the miner's tile.
fn nearest_open_mine(
    creature: &Creature,
    session: &GameSession,
    data: &GameData,
    claims: &MineClaims,
) -> Option<TilePos> {
    let slots = data
        .buildings
        .get("mine")
        .and_then(|d| d.workstation.as_ref())
        .map(|w| w.slots)
        .unwrap_or(0);
    let from = creature.tile();
    let mut mines: Vec<TilePos> = session
        .buildings_of("mine")
        .filter(|b| b.reserve > 0.0)
        .filter(|b| claims.get(&b.pos).copied().unwrap_or(0) < slots)
        .map(|b| b.pos)
        .collect();
    mines.sort_by_key(|p| (p.manhattan_distance(&from), p.x, p.y));
    mines
        .into_iter()
        .take(6)
        .find(|p| from == *p || nav::find_path(session, from, *p).is_some())
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
            let left = remaining - dt * creature.work_speed();
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
            let left = remaining - dt * creature.work_speed();
            if left > 0.0 {
                creature.task = Task::PickingUpOre { remaining: left };
                return;
            }
            let wanted = ore_wanted(session, data);
            let take = species
                .carry_capacity
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
    if creature.carried(Good::Mushroom) >= species.carry_capacity {
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

    // Ingot runs: forge output back to the stockpile.
    if let Some(forge) = nearest_building_where(creature, session, "blacksmith", |b| {
        b.stock(Good::Ingot) >= 1.0
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

/// Ore target for an ore-consuming forge, or None if it isn't one.
fn forge_ore_target(kind: &str, data: &GameData) -> Option<u32> {
    match kind {
        "blacksmith" => Some(data.balance.blacksmith_ore_target),
        "smelter" => Some(data.balance.smelter_ore_target),
        _ => None,
    }
}

/// Where should carried ore go: an unfinished site, else the nearest forge
/// (blacksmith or smelter) under its ore target.
fn ore_destination(creature: &Creature, session: &GameSession, data: &GameData) -> Option<TilePos> {
    let from = creature.tile();
    if let Some(site) = session
        .build_sites
        .iter()
        .filter(|s| !s.complete() && s.remaining() > 0)
        .map(|s| s.pos)
        .min_by_key(|p| (p.manhattan_distance(&from), p.x, p.y))
    {
        return Some(site);
    }
    session
        .buildings
        .iter()
        .filter(|b| {
            forge_ore_target(&b.kind, data).is_some_and(|t| (b.stock(Good::Ore) as u32) < t)
        })
        .map(|b| b.pos)
        .min_by_key(|p| (p.manhattan_distance(&from), p.x, p.y))
}

/// Total ore the warren wants moved: site remainders, blacksmith top-ups,
/// and smelter top-ups. The blacksmith is the primary early forge so it's
/// fed freely; smelters only draw above the bank reserve (with an emergency
/// trickle), so endless salamander smelting can't starve construction.
fn ore_wanted(session: &GameSession, data: &GameData) -> u32 {
    let sites: u32 = session.build_sites.iter().map(|s| s.remaining()).sum();
    let blacksmiths: u32 = session
        .buildings_of("blacksmith")
        .map(|b| {
            data.balance
                .blacksmith_ore_target
                .saturating_sub(b.stock(Good::Ore) as u32)
        })
        .sum();
    let smelters: u32 = if session.economy.ore_stock > data.balance.smelter_bank_reserve {
        session
            .buildings_of("smelter")
            .map(|b| {
                data.balance
                    .smelter_ore_target
                    .saturating_sub(b.stock(Good::Ore) as u32)
            })
            .sum()
    } else {
        // Below the reserve, dens still get an emergency trickle so the
        // salamander never starves while the bank saves up.
        session
            .buildings_of("smelter")
            .map(|b| 2u32.saturating_sub(b.stock(Good::Ore) as u32))
            .sum()
    };
    sites + blacksmiths + smelters
}

/// What a fetch at this tile would yield right now.
fn fetchable_good(session: &GameSession, source: TilePos) -> Option<Good> {
    if let Some(building) = session.building_at(source) {
        return match building.kind.as_str() {
            "farm" if building.stock(Good::Mushroom) >= 1.0 => Some(Good::Mushroom),
            "mine" if building.stock(Good::Ore) >= 1.0 => Some(Good::Ore),
            "kiln" if building.stock(Good::Charcoal) >= 1.0 => Some(Good::Charcoal),
            // The forges emit ingots into their output buffer.
            "blacksmith" | "smelter" if building.stock(Good::Ingot) >= 1.0 => Some(Good::Ingot),
            _ => None,
        };
    }
    if session.patch_regrow.get(&source).is_some_and(|t| *t <= 0.0) {
        return Some(Good::Mushroom);
    }
    if session
        .sporewood_regrow
        .get(&source)
        .is_some_and(|t| *t <= 0.0)
    {
        return Some(Good::Wood);
    }
    None
}

fn harvest_source(
    creature: &mut Creature,
    session: &mut GameSession,
    data: &GameData,
    species: &SpeciesDef,
    source: TilePos,
) {
    let Some(good) = fetchable_good(session, source) else {
        return;
    };
    let space = species.carry_capacity - creature.carried(good);
    if space == 0 {
        return;
    }
    if let Some(building) = session.building_at_mut(source) {
        let take = building.take_stock(good, space as f32).floor() as u32;
        // take_stock floors can strand fractions; put any remainder back.
        creature.add_carried(good, take);
        return;
    }
    match good {
        Good::Mushroom => {
            session
                .patch_regrow
                .insert(source, data.balance.patch_regrow_sec);
            creature.add_carried(Good::Mushroom, 1);
        }
        Good::Wood => {
            session
                .sporewood_regrow
                .insert(source, data.balance.sporewood_regrow_sec);
            creature.add_carried(Good::Wood, 1);
        }
        _ => {}
    }
}

/// Deposit the load at a building or build site.
fn drop_load(creature: &mut Creature, session: &mut GameSession, data: &GameData, pos: TilePos) {
    // Build sites take ore.
    if let Some(site) = session.build_sites.iter_mut().find(|s| s.pos == pos) {
        let deliver = creature.take_carried(Good::Ore, site.remaining());
        site.ore_delivered += deliver;
        complete_finished_sites(session, data);
        return;
    }
    let Some(building) = session.buildings.iter_mut().find(|b| b.pos == pos) else {
        return;
    };
    match building.kind.as_str() {
        "cook_pot" => {
            let n = creature.take_carried(Good::Mushroom, u32::MAX);
            building.add_stock(Good::Mushroom, n as f32);
        }
        "kiln" => {
            let n = creature.take_carried(Good::Wood, u32::MAX);
            building.add_stock(Good::Wood, n as f32);
        }
        "blacksmith" => {
            let ore = creature.take_carried(Good::Ore, u32::MAX);
            building.add_stock(Good::Ore, ore as f32);
        }
        "smelter" => {
            let ore = creature.take_carried(Good::Ore, u32::MAX);
            building.add_stock(Good::Ore, ore as f32);
            let charcoal = creature.take_carried(Good::Charcoal, u32::MAX);
            building.add_stock(Good::Charcoal, charcoal as f32);
        }
        _ => {}
    }
}

/// Turn fully-supplied ghosts into working buildings.
fn complete_finished_sites(session: &mut GameSession, data: &GameData) {
    let mut i = 0;
    while i < session.build_sites.len() {
        if session.build_sites[i].complete() {
            let site = session.build_sites.remove(i);
            let building = if site.kind == "mine" {
                Building::mine(site.pos, data.balance.mine_reserve)
            } else {
                Building::new(&site.kind, site.pos)
            };
            session.buildings.push(building);
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

// --------------------------------------------------------------- guards

/// Guards chase raiders and fight them; otherwise they stand watch near
/// the stockpile (the raid target).
fn tick_guard(creature: &mut Creature, session: &mut GameSession, data: &GameData, dt: f32) {
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
            let dps = wildlife::guard_dps(session, data) * creature.work_speed();
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

// ------------------------------------------------------------- smelters

/// Salamanders: the living furnace. A batch claims ore + charcoal from
/// the den; the charcoal is also the salamander's meal (diet chain).
fn tick_smelter(creature: &mut Creature, session: &mut GameSession, data: &GameData, dt: f32) {
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
            let left = remaining - dt * creature.work_speed();
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

// ---------------------------------------------------------------- smiths

/// Goblin smiths: a Blacksmith workstation hammers ore into ingots — the
/// first processing node, live from the early minutes. Deliberately worse
/// per-ore than the salamander smelter (labour-only, no charcoal), so the
/// charcoal chain stays the bulk upgrade.
fn tick_smith(creature: &mut Creature, session: &mut GameSession, data: &GameData, dt: f32) {
    let b = &data.balance;
    match creature.task.clone() {
        Task::Idle => {
            let ready = nearest_building_where(creature, session, "blacksmith", |shop| {
                shop.stock(Good::Ore) >= b.smith_batch_ore as f32
            });
            if let Some(shop) = ready {
                if creature.tile() == shop {
                    if let Some(building) = session.building_at_mut(shop) {
                        building.take_stock(Good::Ore, b.smith_batch_ore as f32);
                        creature.task = Task::Smithing {
                            shop,
                            remaining: b.smith_batch_time_sec,
                        };
                    }
                } else {
                    send_to(creature, session, shop, Task::GoSmith(shop));
                }
                return;
            }
            // No ore to work: wait at the nearest blacksmith.
            if let Some(shop) = nearest_building(creature, session, "blacksmith") {
                if creature.tile() != shop {
                    send_to(creature, session, shop, Task::GoSmith(shop));
                }
            }
        }
        Task::GoSmith(_) => creature.task = Task::Idle,
        Task::Smithing { shop, remaining } => {
            let left = remaining - dt * creature.work_speed();
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

/// Grown sporewood tiles, nearest first.
fn sporewood_sources_nearest_first(creature: &Creature, session: &GameSession) -> Vec<TilePos> {
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
