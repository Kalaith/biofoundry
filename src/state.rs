//! Game state machine and the live warren session.
//!
//! Only one `GameState` is active at a time; states signal changes by
//! returning a `StateTransition`, which `Game::transition` applies
//! explicitly. Simulation state lives in `GameSession` and is only mutated
//! by `simulation` services and dispatched `UiAction`s.

pub mod world;

use crate::data::GameConfig;
use macroquad_toolkit::rng::SeededRng;
use serde::{Deserialize, Serialize};
use world::WorldMap;

/// Which screen is running. Session data is owned by the active state.
pub enum GameState {
    Menu,
    Warren(GameSession),
}

/// Explicit state changes returned by state updates / UI dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateTransition {
    StartWarren,
    BackToMenu,
}

/// The live simulation: world map plus everything that ticks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSession {
    pub world: WorldMap,
    pub rng: SeededRng,
    /// Completed fixed-timestep simulation ticks.
    pub tick: u64,
}

impl GameSession {
    pub fn new(config: &GameConfig, seed: u64) -> Self {
        let mut rng = SeededRng::new(seed);
        let world = WorldMap::generate(config.world_width, config.world_height, &mut rng);
        Self {
            world,
            rng,
            tick: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;

    #[test]
    fn session_boots_from_config() {
        let data = GameData::load().unwrap();
        let session = GameSession::new(&data.config, data.config.world_seed);

        assert_eq!(session.tick, 0);
        assert_eq!(session.world.tiles.width, data.config.world_width);
        assert_eq!(session.world.tiles.height, data.config.world_height);
    }
}
