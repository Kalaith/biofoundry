//! Game state machine and the live warren session.
//!
//! Only one `GameState` is active at a time; states signal changes by
//! returning a `StateTransition`, which `Game::transition` applies
//! explicitly. Simulation state lives in `GameSession` and is only mutated
//! by `simulation` services and dispatched `UiAction`s.

pub mod creatures;
pub mod world;

use crate::data::{Balance, GameData};
use creatures::{Creature, Job};
use macroquad_toolkit::grid::TilePos;
use macroquad_toolkit::rng::SeededRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use world::{Tile, WorldMap};

/// Which screen is running. Session data is owned by the active state.
pub enum GameState {
    Menu,
    Warren(Box<GameSession>),
}

/// Explicit state changes returned by state updates / UI dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateTransition {
    StartWarren,
    BackToMenu,
}

/// Fixed structures in the warren (pre-placed in Phase 1; player-built in
/// Phase 2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Buildings {
    /// Mushroom Farm: grows mushrooms continuously (the "generator").
    pub farm: TilePos,
    /// Cook Pot: mushrooms are cooked into food here (the "power plant").
    pub cook_pot: TilePos,
    /// Stockpile: miners deliver ore here.
    pub stockpile: TilePos,
}

/// Global resource counters (the "battery" side of the food grid).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Economy {
    /// Cooked food stockpile — the electricity of the warren.
    pub food: f32,
    /// Mushrooms grown and waiting at the farm.
    pub farm_mushrooms: f32,
    /// Mushrooms delivered to the cook pot, awaiting cooking.
    pub pot_mushrooms: u32,
    /// Total ore delivered to the stockpile (win condition counter).
    pub ore_delivered: u32,
    /// Creatures lost to starvation desertion (blackout consequence).
    pub deserted: u32,
    /// Smoothed food production rate (per minute) for the calorie meter —
    /// cooking lands in bursts, so the HUD shows a moving average.
    pub production_ema_per_min: f32,
}

/// The live simulation: world map plus everything that ticks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSession {
    pub world: WorldMap,
    pub rng: SeededRng,
    /// Completed fixed-timestep simulation ticks.
    pub tick: u64,
    pub buildings: Buildings,
    pub economy: Economy,
    pub creatures: Vec<Creature>,
    pub next_creature_id: u32,
    /// Wild mushroom patches: seconds until regrown (0 = harvestable).
    pub patch_regrow: HashMap<TilePos, f32>,
    /// Ore remaining per vein tile; mined-out veins open into floor.
    pub vein_ore: HashMap<TilePos, u32>,
    pub won: bool,
    pub victory_shown: bool,
}

impl GameSession {
    pub fn new(data: &GameData, seed: u64) -> Self {
        let config = &data.config;
        let balance = &data.balance;
        let mut rng = SeededRng::new(seed);
        let world = WorldMap::generate(config.world_width, config.world_height, &mut rng);

        let buildings = place_buildings(&world, balance.farm_min_distance);
        let patch_regrow = world
            .tiles
            .iter_with_pos()
            .filter(|(_, t)| **t == Tile::MushroomPatch)
            .map(|(pos, _)| (pos, 0.0))
            .collect();
        let vein_ore = world
            .tiles
            .iter_with_pos()
            .filter(|(_, t)| **t == Tile::OreVein)
            .map(|(pos, _)| (pos, balance.vein_ore_yield))
            .collect();

        let mut session = Self {
            world,
            rng,
            tick: 0,
            buildings,
            economy: Economy {
                food: balance.start_food,
                farm_mushrooms: 0.0,
                pot_mushrooms: 0,
                ore_delivered: 0,
                deserted: 0,
                production_ema_per_min: 0.0,
            },
            creatures: Vec::new(),
            next_creature_id: 1,
            patch_regrow,
            vein_ore,
            won: false,
            victory_shown: false,
        };

        for _ in 0..balance.start_miners {
            session.spawn_creature("goblin", Job::Miner);
        }
        for _ in 0..balance.start_carriers {
            session.spawn_creature("goblin", Job::Carrier);
        }
        for _ in 0..balance.start_cooks {
            session.spawn_creature("goblin", Job::Cook);
        }

        session
    }

    pub fn spawn_creature(&mut self, species: &str, job: Job) {
        let id = self.next_creature_id;
        self.next_creature_id += 1;
        self.creatures
            .push(Creature::new(id, species, job, self.world.spawn));
    }

    pub fn job_count(&self, job: Job) -> usize {
        self.creatures.iter().filter(|c| c.job == job).count()
    }

    /// Move one reassignable creature from `from` to `to`. Returns success.
    pub fn reassign(
        &mut self,
        from: Job,
        to: Job,
        species_reassignable: impl Fn(&str) -> bool,
    ) -> bool {
        if from == to {
            return false;
        }
        let Some(creature) = self
            .creatures
            .iter_mut()
            .find(|c| c.job == from && species_reassignable(&c.species))
        else {
            return false;
        };
        creature.job = to;
        creature.clear_task();
        true
    }

    /// Per-minute upkeep draw for one creature (idle draws reduced rate,
    /// cooks draw more — the plan's tier-0 table).
    pub fn upkeep_per_min(creature: &Creature, base: f32, balance: &Balance) -> f32 {
        match creature.job {
            Job::Idle => base * balance.idle_upkeep_factor,
            Job::Cook => base * balance.cook_upkeep_factor,
            _ => base,
        }
    }
}

/// Pre-place the farm, cook pot, and stockpile on reachable open floor.
/// The pot sits next to spawn; the farm sits a real haul away (that walk is
/// what makes carrier throughput a meaningful number).
fn place_buildings(world: &WorldMap, farm_min_distance: i32) -> Buildings {
    let spawn = world.spawn;
    let reachable = world
        .tiles
        .flood_fill(spawn, false, |_, t: &Tile| t.walkable());
    let mut floors_by_distance: Vec<TilePos> = world
        .tiles
        .iter_with_pos()
        .filter(|(pos, t)| **t == Tile::Floor && *pos != spawn && reachable.contains(pos))
        .map(|(pos, _)| pos)
        .collect();
    floors_by_distance.sort_by_key(|p| (p.manhattan_distance(&spawn), p.x, p.y));

    let cook_pot = floors_by_distance.first().copied().unwrap_or(spawn);
    let farm = floors_by_distance
        .iter()
        .find(|p| p.manhattan_distance(&spawn) >= farm_min_distance && **p != cook_pot)
        .copied()
        // Fall back to the farthest reachable floor on cramped maps.
        .or_else(|| floors_by_distance.last().copied())
        .unwrap_or(spawn);

    Buildings {
        farm,
        cook_pot,
        stockpile: spawn,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;

    #[test]
    fn session_boots_from_config() {
        let data = GameData::load().unwrap();
        let session = GameSession::new(&data, data.config.world_seed);

        assert_eq!(session.tick, 0);
        assert_eq!(session.world.tiles.width, data.config.world_width);
        assert_eq!(
            session.creatures.len() as u32,
            data.balance.start_miners + data.balance.start_carriers + data.balance.start_cooks
        );
        assert!(!session.patch_regrow.is_empty());
        assert!(!session.vein_ore.is_empty());
        assert!(session.economy.food > 0.0);
    }

    #[test]
    fn buildings_land_on_walkable_floor() {
        let data = GameData::load().unwrap();
        let session = GameSession::new(&data, data.config.world_seed);
        let b = &session.buildings;

        for pos in [b.farm, b.cook_pot, b.stockpile] {
            assert!(
                session.world.tiles.get(pos).unwrap().walkable(),
                "building at {pos:?} must be walkable"
            );
        }
        assert_ne!(b.farm, b.cook_pot);
    }

    #[test]
    fn reassignment_moves_one_goblin_and_resets_its_task() {
        let data = GameData::load().unwrap();
        let mut session = GameSession::new(&data, 5);
        let miners_before = session.job_count(Job::Miner);
        let carriers_before = session.job_count(Job::Carrier);

        let moved = session.reassign(Job::Miner, Job::Carrier, |s| {
            data.species.get(s).map(|d| d.reassignable).unwrap_or(false)
        });

        assert!(moved);
        assert_eq!(session.job_count(Job::Miner), miners_before - 1);
        assert_eq!(session.job_count(Job::Carrier), carriers_before + 1);
    }

    #[test]
    fn beetles_cannot_be_reassigned() {
        let data = GameData::load().unwrap();
        let mut session = GameSession::new(&data, 5);
        // Make everyone a beetle-only pool for the source job.
        session.creatures.clear();
        session.spawn_creature("beetle", Job::Carrier);

        let moved = session.reassign(Job::Carrier, Job::Miner, |s| {
            data.species.get(s).map(|d| d.reassignable).unwrap_or(false)
        });

        assert!(!moved);
    }
}
