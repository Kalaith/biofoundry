//! Creature instances: job, task, hunger, position, and path.

use macroquad_toolkit::grid::TilePos;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Job {
    Miner,
    Carrier,
    Cook,
    Idle,
}

impl Job {
    pub fn label(self) -> &'static str {
        match self {
            Job::Miner => "Miner",
            Job::Carrier => "Carrier",
            Job::Cook => "Cook",
            Job::Idle => "Idle",
        }
    }
}

/// What a creature is currently doing. Movement is generic: while `path` is
/// non-empty the creature walks it; task logic runs on arrival.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Task {
    Idle,
    /// Walking to stand next to this ore vein, then mine it.
    GoMine(TilePos),
    Mining {
        vein: TilePos,
        remaining: f32,
    },
    DeliverOre,
    /// Walking to a mushroom source (farm tile or wild patch).
    GoFetch(TilePos),
    /// Gathering a load at a source.
    Fetching {
        source: TilePos,
        remaining: f32,
    },
    DeliverMushrooms,
    /// Walking to the cook pot.
    GoCook,
    Cooking {
        remaining: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Creature {
    pub id: u32,
    /// Species id in `species.json` ("goblin", "beetle").
    pub species: String,
    pub job: Job,
    /// World position in tile units (tile center = tile + 0.5).
    pub x: f32,
    pub y: f32,
    /// Remaining waypoints, front first.
    pub path: Vec<TilePos>,
    pub task: Task,
    /// Mushrooms (carriers) or ore (miners) currently carried.
    pub carrying: u32,
    /// 1.0 = fed, 0.0 = starving. Drives work speed (brownout).
    pub satiation: f32,
    /// Seconds spent at zero satiation (blackout → desertion).
    pub starving_for: f32,
}

impl Creature {
    pub fn new(id: u32, species: &str, job: Job, tile: TilePos) -> Self {
        Self {
            id,
            species: species.to_owned(),
            job,
            x: tile.x as f32 + 0.5,
            y: tile.y as f32 + 0.5,
            path: Vec::new(),
            task: Task::Idle,
            carrying: 0,
            satiation: 1.0,
            starving_for: 0.0,
        }
    }

    pub fn tile(&self) -> TilePos {
        TilePos::new(self.x.floor() as i32, self.y.floor() as i32)
    }

    /// Brownout curve: fed 100%, hungry 50%, starving 25%.
    pub fn work_speed(&self) -> f32 {
        if self.satiation > 0.66 {
            1.0
        } else if self.satiation > 0.33 {
            0.5
        } else {
            0.25
        }
    }

    /// Reset activity when reassigned or interrupted; drops carried goods.
    pub fn clear_task(&mut self) {
        self.task = Task::Idle;
        self.path.clear();
        self.carrying = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_speed_follows_brownout_tiers() {
        let mut c = Creature::new(1, "goblin", Job::Miner, TilePos::new(0, 0));

        c.satiation = 1.0;
        assert_eq!(c.work_speed(), 1.0);
        c.satiation = 0.5;
        assert_eq!(c.work_speed(), 0.5);
        c.satiation = 0.1;
        assert_eq!(c.work_speed(), 0.25);
        c.satiation = 0.0;
        assert_eq!(c.work_speed(), 0.25);
    }
}
