//! Placed buildings and in-progress build sites.

use crate::state::creatures::Good;
use macroquad_toolkit::grid::TilePos;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A finished building with per-good stocks: farms grow Mushroom, pots
/// buffer Mushroom for cooking, kilns hold Wood in and Charcoal out,
/// smelters hold Ore and Charcoal for the salamander.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Building {
    /// Id into `buildings.json` ("farm", "cook_pot", "mine", "kiln",
    /// "smelter", "stockpile").
    pub kind: String,
    pub pos: TilePos,
    pub stocks: HashMap<Good, f32>,
    /// Extractable deposit remaining, for reserve-bearing workstations
    /// (the Mine). Zero for everything else. `#[serde(default)]` keeps
    /// pre-Phase-6 saves loading.
    #[serde(default)]
    pub reserve: f32,
    /// Equipment craft queue (item ids), for the Blacksmith. The smith
    /// works the front order once it has banked enough ingots.
    #[serde(default)]
    pub orders: Vec<String>,
}

impl Building {
    pub fn new(kind: &str, pos: TilePos) -> Self {
        Self {
            kind: kind.to_owned(),
            pos,
            stocks: HashMap::new(),
            reserve: 0.0,
            orders: Vec::new(),
        }
    }

    /// A Mine with a starting deposit; carriers drain its ore buffer.
    pub fn mine(pos: TilePos, reserve: f32) -> Self {
        let mut b = Self::new("mine", pos);
        b.reserve = reserve;
        b
    }

    pub fn stock(&self, good: Good) -> f32 {
        self.stocks.get(&good).copied().unwrap_or(0.0)
    }

    pub fn add_stock(&mut self, good: Good, amount: f32) {
        *self.stocks.entry(good).or_insert(0.0) += amount;
    }

    /// Remove up to `amount`; returns how much actually came out.
    pub fn take_stock(&mut self, good: Good, amount: f32) -> f32 {
        let Some(stock) = self.stocks.get_mut(&good) else {
            return 0.0;
        };
        let taken = amount.min(*stock);
        *stock -= taken;
        taken
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_accounting() {
        let mut b = Building::new("kiln", TilePos::new(1, 1));
        assert_eq!(b.stock(Good::Wood), 0.0);

        b.add_stock(Good::Wood, 3.0);
        assert_eq!(b.take_stock(Good::Wood, 1.5), 1.5);
        assert_eq!(b.take_stock(Good::Wood, 5.0), 1.5);
        assert_eq!(b.take_stock(Good::Wood, 1.0), 0.0);
    }
}
