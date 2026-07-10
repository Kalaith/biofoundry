//! Wild creatures: fauna the player didn't hire.
//!
//! Wild beetles wander until trapped (capture → study → adapt); gnarl
//! raiders head for the larder, eat, and slink home — they're hungry too.

use macroquad_toolkit::grid::TilePos;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WildBehavior {
    /// Amble between nearby tiles until trapped.
    Wander { next_move_in: f32 },
    /// Head to the larder, eat, then flee back out.
    Raid {
        origin: TilePos,
        eaten: f32,
        fleeing: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WildCreature {
    pub id: u32,
    /// Species id in `species.json` ("wild_beetle", "gnarl").
    pub species: String,
    pub x: f32,
    pub y: f32,
    pub path: Vec<TilePos>,
    pub hp: f32,
    pub behavior: WildBehavior,
}

impl WildCreature {
    pub fn new(id: u32, species: &str, tile: TilePos, hp: f32, behavior: WildBehavior) -> Self {
        Self {
            id,
            species: species.to_owned(),
            x: tile.x as f32 + 0.5,
            y: tile.y as f32 + 0.5,
            path: Vec::new(),
            hp,
            behavior,
        }
    }

    pub fn tile(&self) -> TilePos {
        TilePos::new(self.x.floor() as i32, self.y.floor() as i32)
    }

    pub fn is_raider(&self) -> bool {
        matches!(self.behavior, WildBehavior::Raid { .. })
    }
}

/// Event counters that drive `unlocks.json` progression — advancing them
/// is a side effect of playing (plan §5).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Progress {
    pub beetles_captured: u32,
    pub raids_survived: u32,
    pub famines_survived: u32,
    /// Captured specimens housed for study/breeding.
    pub specimens: u32,
    /// Accumulated observation at study pens (flavor + future unlocks).
    pub knowledge: f32,
}

impl Progress {
    /// Named counter lookup for data-driven unlock definitions.
    pub fn counter(&self, name: &str) -> u32 {
        match name {
            "beetles_captured" => self.beetles_captured,
            "raids_survived" => self.raids_survived,
            "famines_survived" => self.famines_survived,
            _ => 0,
        }
    }
}
