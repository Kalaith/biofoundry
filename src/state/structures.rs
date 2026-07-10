//! Placed buildings and in-progress build sites.

use macroquad_toolkit::grid::TilePos;
use serde::{Deserialize, Serialize};

/// A finished building. `stock` means mushrooms grown (farms) or
/// mushrooms awaiting cooking (cook pots); unused for stockpiles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Building {
    /// Id into `buildings.json` ("farm", "cook_pot", "stockpile").
    pub kind: String,
    pub pos: TilePos,
    pub stock: f32,
}

impl Building {
    pub fn new(kind: &str, pos: TilePos) -> Self {
        Self {
            kind: kind.to_owned(),
            pos,
            stock: 0.0,
        }
    }
}

/// A ghost the player placed: carriers deliver ore until it completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildSite {
    pub kind: String,
    pub pos: TilePos,
    pub ore_needed: u32,
    pub ore_delivered: u32,
}

impl BuildSite {
    pub fn remaining(&self) -> u32 {
        self.ore_needed.saturating_sub(self.ore_delivered)
    }

    pub fn complete(&self) -> bool {
        self.ore_delivered >= self.ore_needed
    }
}
