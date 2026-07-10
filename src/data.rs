//! Embedded, data-driven game configuration and content tables.
//!
//! All balance/content lives in `assets/data/*.json` and is embedded with
//! `include_str!` so WASM builds work without filesystem access. Tune the
//! JSON, not Rust constants.

use macroquad_toolkit::data_loader::{load_embedded_json_labeled, DataRegistry};
use serde::{Deserialize, Serialize};

const GAME_CONFIG_JSON: &str = include_str!("../assets/data/game_config.json");
const SPECIES_JSON: &str = include_str!("../assets/data/species.json");
const BALANCE_JSON: &str = include_str!("../assets/data/balance.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub game_name: String,
    pub display_name: String,
    pub save_slot: String,
    pub version: String,
    pub world_width: usize,
    pub world_height: usize,
    pub world_seed: u64,
    pub tile_size: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeciesDef {
    pub id: String,
    pub name: String,
    /// Base upkeep draw while working a normal job (food per minute).
    pub food_per_min: f32,
    pub move_tiles_per_sec: f32,
    pub carry_capacity: u32,
    /// Whether the player can move this creature between jobs.
    pub reassignable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub start_food: f32,
    pub start_miners: u32,
    pub start_carriers: u32,
    pub start_cooks: u32,
    /// Cooks draw more than field goblins (plan: cook eats 2/min).
    pub cook_upkeep_factor: f32,
    /// Idle creatures draw a reduced rate.
    pub idle_upkeep_factor: f32,
    pub farm_mushrooms_per_min: f32,
    pub farm_storage_cap: f32,
    /// Preferred manhattan distance from spawn to the farm — the haul is
    /// the labor cost that makes carrier throughput matter.
    pub farm_min_distance: i32,
    pub patch_regrow_sec: f32,
    pub vein_ore_yield: u32,
    pub mine_time_sec: f32,
    /// Time to gather a load at the farm or a wild patch.
    pub haul_pickup_sec: f32,
    pub cook_batch_mushrooms: u32,
    pub cook_batch_food: f32,
    pub cook_batch_time_sec: f32,
    /// Seconds for satiation to refill from 0 to 1 while food is stocked.
    pub satiation_recover_sec: f32,
    /// Seconds for satiation to drain from 1 to 0 on an empty stockpile.
    pub satiation_drain_sec: f32,
    pub desert_after_starving_sec: f32,
    pub beetle_ore_cost: u32,
    pub win_food_surplus: f32,
    pub win_ore_delivered: u32,
}

#[derive(Debug, Clone)]
pub struct GameData {
    pub config: GameConfig,
    pub species: DataRegistry<SpeciesDef>,
    pub balance: Balance,
}

impl GameData {
    pub fn load() -> Result<Self, String> {
        let config = load_embedded_json_labeled("game_config", GAME_CONFIG_JSON)?;
        let species = DataRegistry::from_embedded_json(SPECIES_JSON, "id")?;
        let balance = load_embedded_json_labeled("balance", BALANCE_JSON)?;

        Ok(Self {
            config,
            species,
            balance,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_data_loads() {
        let data = GameData::load().unwrap();

        assert_eq!(data.config.game_name, "biofoundry");
        assert!(data.config.world_width >= 16);
        assert!(data.config.tile_size > 0.0);
    }

    #[test]
    fn species_cross_references_hold() {
        let data = GameData::load().unwrap();

        // The two Phase 1 species must exist and be sane.
        let goblin = data.species.get("goblin").expect("goblin species");
        assert!(goblin.reassignable);
        assert!(goblin.food_per_min > 0.0);
        assert!(goblin.carry_capacity > 0);

        let beetle = data.species.get("beetle").expect("beetle species");
        assert!(!beetle.reassignable);
        assert!(
            beetle.carry_capacity >= goblin.carry_capacity * 5,
            "beetle must haul at least 5x a goblin (plan)"
        );
        assert!(beetle.food_per_min > goblin.food_per_min);
    }

    #[test]
    fn balance_values_are_playable() {
        let data = GameData::load().unwrap();
        let b = &data.balance;

        assert!(b.start_food > 0.0);
        assert!(b.start_miners + b.start_carriers + b.start_cooks >= 3);
        assert!(b.cook_batch_mushrooms > 0);
        assert!(b.cook_batch_food > 0.0);
        assert!(b.win_ore_delivered > 0);
        assert!(b.win_food_surplus > b.start_food);
        // Cooking must multiply calories, or the loop can never go positive.
        assert!(b.cook_batch_food / b.cook_batch_mushrooms as f32 > 1.0);
    }
}
