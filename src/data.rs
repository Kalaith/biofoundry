//! Embedded, data-driven game configuration and content tables.
//!
//! All balance/content lives in `assets/data/*.json` and is embedded with
//! `include_str!` so WASM builds work without filesystem access.

use macroquad_toolkit::data_loader::load_embedded_json_labeled;
use serde::{Deserialize, Serialize};

const GAME_CONFIG_JSON: &str = include_str!("../assets/data/game_config.json");

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

#[derive(Debug, Clone)]
pub struct GameData {
    pub config: GameConfig,
}

impl GameData {
    pub fn load() -> Result<Self, String> {
        let config = load_embedded_json_labeled("game_config", GAME_CONFIG_JSON)?;
        Ok(Self { config })
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
        assert!(data.config.world_height >= 16);
        assert!(data.config.tile_size > 0.0);
    }
}
