//! Miners: claim a Mine slot, extract into its buffer, and answer the
//! player's dig designations (which outrank the post — they're finite).

use crate::data::GameData;
use crate::simulation::jobs::equipment::equip_effect;
use crate::simulation::jobs::routing::{nearest_dig_mark, send_to, set_path};
use crate::simulation::nav;
use crate::state::creatures::{Creature, Good, Task};
use crate::state::world::Tile;
use crate::state::GameSession;
use macroquad_toolkit::grid::TilePos;
use std::collections::HashMap;

/// Live count of miners claiming each Mine (walking to or working it), so
/// staffing respects the workstation's slot limit even though the creature
/// list is taken out of the session during the tick.
pub(super) type MineClaims = HashMap<TilePos, u32>;

pub(super) fn tick_miner(
    creature: &mut Creature,
    session: &mut GameSession,
    data: &GameData,
    dt: f32,
    claims: &mut MineClaims,
    work_boost: f32,
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
            // it resumes when carriers drain it. An Iron Pickaxe multiplies
            // the extraction rate — the feedback loop the factory runs on.
            let pickaxe = equip_effect(creature, data, "mine_speed_mult").unwrap_or(1.0);
            let headroom = (cap - b.stock(Good::Ore)).max(0.0);
            let amount = (rate / 60.0 * dt * creature.work_speed() * work_boost * pickaxe)
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
            let left = remaining - dt * creature.work_speed() * work_boost;
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
