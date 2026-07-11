//! Factory legibility: derive the at-a-glance status of a workstation and
//! the warren's pending-haul pressure, so a stalled chain link is
//! diagnosable without clicking anything (plan §Phase 9).

use crate::data::GameData;
use crate::simulation::wildlife;
use crate::state::creatures::{Good, Job, Task};
use crate::state::structures::Building;
use crate::state::GameSession;
use macroquad_toolkit::grid::TilePos;

/// What's wrong with a workstation right now — the in-world status icon.
/// `None` (from [`building_status`]) means the node is running nominally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingStatus {
    /// A workstation with no creature working it (stopped node).
    NoWorker,
    /// Waiting on input goods it can't get (starved).
    InputStarved,
    /// Output buffer is full and backing up — needs a carrier.
    OutputFull,
    /// A Mine whose deposit has run dry.
    Exhausted,
    /// A Farm sitting at its storage cap, idle until a carrier drains it.
    AwaitingHaul,
}

impl BuildingStatus {
    /// A short human label (also the legend text).
    pub fn label(self) -> &'static str {
        match self {
            BuildingStatus::NoWorker => "No worker",
            BuildingStatus::InputStarved => "Starved",
            BuildingStatus::OutputFull => "Backed up",
            BuildingStatus::Exhausted => "Exhausted",
            BuildingStatus::AwaitingHaul => "Awaiting haul",
        }
    }
}

/// Is any creature of `job` currently working (or waiting) at `pos`?
fn staffed_at(session: &GameSession, pos: TilePos, job: Job) -> bool {
    session.creatures.iter().any(|c| {
        c.job == job
            && match &c.task {
                Task::WorkMine(p) | Task::GoMine(p) => *p == pos,
                Task::Smithing { shop, .. } | Task::Crafting { shop, .. } | Task::GoSmith(shop) => {
                    *shop == pos
                }
                Task::Smelting { den, .. } | Task::GoSmelt(den) => *den == pos,
                // A creature idling on the tile also counts as manning it.
                _ => c.tile() == pos,
            }
    })
}

/// The status icon to show over `building`, or `None` when it's nominal.
pub fn building_status(
    session: &GameSession,
    data: &GameData,
    building: &Building,
) -> Option<BuildingStatus> {
    let pos = building.pos;
    match building.kind.as_str() {
        "mine" => {
            if building.reserve <= 0.0 {
                return Some(BuildingStatus::Exhausted);
            }
            if !staffed_at(session, pos, Job::Miner) {
                return Some(BuildingStatus::NoWorker);
            }
            if building.stock(Good::Ore) >= data.balance.mine_buffer_cap - 0.5 {
                return Some(BuildingStatus::OutputFull);
            }
            None
        }
        "blacksmith" => {
            if !staffed_at(session, pos, Job::Smith) {
                return Some(BuildingStatus::NoWorker);
            }
            // Idle for lack of ore, and nothing queued to justify waiting.
            if building.stock(Good::Ore) < data.balance.smith_batch_ore as f32
                && building.orders.is_empty()
            {
                return Some(BuildingStatus::InputStarved);
            }
            None
        }
        "smelter" => {
            if !staffed_at(session, pos, Job::Smelter) {
                return Some(BuildingStatus::NoWorker);
            }
            if building.stock(Good::Ore) < data.balance.smelt_batch_ore as f32
                || building.stock(Good::Charcoal) < data.balance.smelt_batch_charcoal
            {
                return Some(BuildingStatus::InputStarved);
            }
            None
        }
        "farm" => {
            let cap = wildlife::farm_cap(session, data);
            if building.stock(Good::Mushroom) >= cap - 0.5 {
                return Some(BuildingStatus::AwaitingHaul);
            }
            None
        }
        "cook_pot" => {
            if building.stock(Good::Mushroom) < data.balance.cook_batch_mushrooms as f32 {
                return Some(BuildingStatus::InputStarved);
            }
            None
        }
        "kiln" => {
            if building.stock(Good::Wood) <= 0.0 {
                return Some(BuildingStatus::InputStarved);
            }
            None
        }
        _ => None,
    }
}

/// Rough count of pending haul jobs — pickup points holding goods that want
/// moving, plus open construction. Turns "should I add a carrier?" into a
/// read instead of a guess.
pub fn pending_hauls(session: &GameSession) -> usize {
    let mut n = 0;
    for b in &session.buildings {
        match b.kind.as_str() {
            "mine" if b.stock(Good::Ore) >= 1.0 => n += 1,
            "farm" if b.stock(Good::Mushroom) >= 1.0 => n += 1,
            "smelter" if b.stock(Good::Ingot) >= 1.0 => n += 1,
            "blacksmith" if b.stock(Good::Ingot) >= 1.0 && b.orders.is_empty() => n += 1,
            _ => {}
        }
    }
    // Open construction is haul demand too.
    n += session
        .build_sites
        .iter()
        .filter(|s| s.remaining() > 0)
        .count();
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::structures::Building;

    fn boot() -> (GameData, GameSession) {
        let data = GameData::load().unwrap();
        let session = GameSession::new(&data, data.config.world_seed);
        (data, session)
    }

    #[test]
    fn unstaffed_mine_reads_no_worker_then_exhausted() {
        let (data, mut session) = boot();
        session.creatures.clear();
        let mine = session.buildings_of("mine").next().unwrap().pos;

        // Nobody on it → no worker.
        let b = session.building_at(mine).unwrap();
        assert_eq!(
            building_status(&session, &data, b),
            Some(BuildingStatus::NoWorker)
        );

        // Dry deposit dominates.
        session.building_at_mut(mine).unwrap().reserve = 0.0;
        let b = session.building_at(mine).unwrap();
        assert_eq!(
            building_status(&session, &data, b),
            Some(BuildingStatus::Exhausted)
        );
    }

    #[test]
    fn full_mine_buffer_reads_backed_up() {
        let (data, mut session) = boot();
        let mine = session.buildings_of("mine").next().unwrap().pos;
        // Staff it and cap the buffer.
        session.spawn_creature(&data, "goblin", Job::Miner);
        {
            let m = session.building_at_mut(mine).unwrap();
            m.add_stock(Good::Ore, data.balance.mine_buffer_cap);
        }
        session.creatures.last_mut().unwrap().task = Task::WorkMine(mine);
        let b = session.building_at(mine).unwrap();
        assert_eq!(
            building_status(&session, &data, b),
            Some(BuildingStatus::OutputFull)
        );
    }

    #[test]
    fn starved_blacksmith_and_kiln_read_starved() {
        let (data, mut session) = boot();
        let spot = session
            .world
            .tiles
            .iter_with_pos()
            .find(|(pos, _)| session.can_place_building(*pos))
            .map(|(pos, _)| pos)
            .unwrap();
        session.buildings.push(Building::new("blacksmith", spot));
        session.spawn_creature(&data, "goblin", Job::Smith);
        session.creatures.last_mut().unwrap().task = Task::GoSmith(spot);
        let b = session.building_at(spot).unwrap();
        // No ore, no orders → starved.
        assert_eq!(
            building_status(&session, &data, b),
            Some(BuildingStatus::InputStarved)
        );
    }

    #[test]
    fn pending_hauls_counts_waiting_goods() {
        let (_data, mut session) = boot();
        let mine = session.buildings_of("mine").next().unwrap().pos;
        let before = pending_hauls(&session);
        session
            .building_at_mut(mine)
            .unwrap()
            .add_stock(Good::Ore, 5.0);
        assert!(pending_hauls(&session) > before);
    }
}
