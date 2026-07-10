//! Creature instances: job, task, hunger, position, and path.

use macroquad_toolkit::grid::TilePos;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Job {
    Miner,
    Carrier,
    Cook,
    /// Salamanders only: the living furnace at a smelter den.
    Smelter,
    Idle,
}

impl Job {
    pub fn label(self) -> &'static str {
        match self {
            Job::Miner => "Miner",
            Job::Carrier => "Carrier",
            Job::Cook => "Cook",
            Job::Smelter => "Smelter",
            Job::Idle => "Idle",
        }
    }
}

/// What a creature can hold or a building can stock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Good {
    Mushroom,
    Ore,
    Wood,
    Charcoal,
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
    /// Walking to stand next to a designated rock tile, then carve it.
    GoDig(TilePos),
    Digging {
        mark: TilePos,
        remaining: f32,
    },
    /// Carry ore home to the stockpile.
    DeliverOre,
    /// Walking to a mushroom source (farm tile or wild patch).
    GoFetch(TilePos),
    /// Gathering a load at a source.
    Fetching {
        source: TilePos,
        remaining: f32,
    },
    /// Carry the load to this building or build site.
    DeliverTo(TilePos),
    /// Walking to the stockpile to load construction/smelting ore.
    GoPickupOre,
    PickingUpOre {
        remaining: f32,
    },
    /// Walking to this cook pot to work it.
    GoCook(TilePos),
    Cooking {
        pot: TilePos,
        remaining: f32,
    },
    /// Walking to this smelter den to work it.
    GoSmelt(TilePos),
    Smelting {
        den: TilePos,
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
    /// What's on this creature's back, if anything.
    pub carrying: Option<(Good, u32)>,
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
            carrying: None,
            satiation: 1.0,
            starving_for: 0.0,
        }
    }

    pub fn tile(&self) -> TilePos {
        TilePos::new(self.x.floor() as i32, self.y.floor() as i32)
    }

    pub fn carried(&self, good: Good) -> u32 {
        match self.carrying {
            Some((g, n)) if g == good => n,
            _ => 0,
        }
    }

    pub fn add_carried(&mut self, good: Good, amount: u32) {
        if amount == 0 {
            return;
        }
        match &mut self.carrying {
            Some((g, n)) if *g == good => *n += amount,
            _ => self.carrying = Some((good, amount)),
        }
    }

    /// Remove up to `amount` of `good`; returns how much came off.
    pub fn take_carried(&mut self, good: Good, amount: u32) -> u32 {
        let Some((g, n)) = &mut self.carrying else {
            return 0;
        };
        if *g != good {
            return 0;
        }
        let taken = amount.min(*n);
        *n -= taken;
        if *n == 0 {
            self.carrying = None;
        }
        taken
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
        self.carrying = None;
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

    #[test]
    fn carried_goods_accounting() {
        let mut c = Creature::new(1, "goblin", Job::Carrier, TilePos::new(0, 0));

        c.add_carried(Good::Mushroom, 2);
        assert_eq!(c.carried(Good::Mushroom), 2);
        assert_eq!(c.carried(Good::Ore), 0);

        assert_eq!(c.take_carried(Good::Ore, 5), 0);
        assert_eq!(c.take_carried(Good::Mushroom, 1), 1);
        assert_eq!(c.take_carried(Good::Mushroom, 5), 1);
        assert!(c.carrying.is_none());
    }
}
