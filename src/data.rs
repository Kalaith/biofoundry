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
const BUILDINGS_JSON: &str = include_str!("../assets/data/buildings.json");
const UNLOCKS_JSON: &str = include_str!("../assets/data/unlocks.json");

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
    /// What this species eats: "food" (the cooked stockpile) or
    /// "charcoal" (drawn from its workplace) — each new diet is a supply
    /// chain, not just a stat (plan §3).
    pub diet: String,
    /// Base upkeep draw while working a normal job (food per minute).
    pub food_per_min: f32,
    pub move_tiles_per_sec: f32,
    pub carry_capacity: u32,
    pub max_hp: f32,
    /// Innate damage per second (wild predators; worker jobs use balance
    /// values like `guard_dps` instead).
    pub attack_dps: f32,
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
    /// Time for a miner to carve one designated rock tile into floor.
    pub dig_time_sec: f32,
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
    pub salamander_ore_cost: u32,
    pub sporewood_regrow_sec: f32,
    /// Kiln wood→charcoal conversion rate (no worker needed; it smoulders).
    pub kiln_charcoal_per_min: f32,
    pub kiln_wood_cap: f32,
    pub smelt_batch_ore: u32,
    pub smelt_batch_charcoal: f32,
    pub smelt_batch_time_sec: f32,
    /// Carriers keep each smelter's ore stock topped up to this level.
    pub smelter_ore_target: u32,
    /// Seconds for a charcoal-eater to go from fed to starving without
    /// charcoal at its den.
    pub salamander_hunger_drain_sec: f32,
    /// Below this food level carriers drop industry hauling and feed the
    /// kitchen first — the load-shedding rule of the food grid.
    pub carrier_food_reserve: f32,
    /// Guards eat more, like cooks (tier-0 table).
    pub guard_upkeep_factor: f32,
    pub guard_dps: f32,
    /// Well-fed creatures knit wounds between fights.
    pub hp_regen_per_sec: f32,
    pub wild_beetle_spawn_sec: f32,
    pub wild_beetle_max: usize,
    /// First raid lands after this long; later raids grow to `raid_size_max`.
    pub raid_first_sec: f32,
    pub raid_interval_sec: f32,
    pub raid_size_max: usize,
    /// Raiders drain the food stockpile at this rate while feeding.
    pub raider_food_eat_per_min: f32,
    /// A raider that has eaten this much slinks away satisfied.
    pub raider_flee_after_eaten: f32,
    pub study_knowledge_per_specimen_min: f32,
    pub breed_interval_sec: f32,
    /// The breeding pit stops at this many living beetles.
    pub bred_beetle_cap: u32,
    /// Food must recover above this after a blackout to count the famine
    /// as survived.
    pub famine_recover_food: f32,
    pub win_food_surplus: f32,
    pub win_ore_delivered: u32,
    /// Metal to forge for the extended "Factory Complete" goal.
    pub win2_metal: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildingDef {
    pub id: String,
    pub name: String,
    /// Ore that carriers must deliver to the build site.
    pub cost_ore: u32,
    /// Whether it appears in the player's build menu.
    pub buildable: bool,
    /// Unlock id (from `unlocks.json`) gating this building, if any.
    pub requires_unlock: Option<String>,
}

/// A progression unlock: an event counter the player naturally advances,
/// and what completing it grants (plan §5 — no abstract tech tree).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnlockDef {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Which session counter drives this ("beetles_captured",
    /// "raids_survived", "famines_survived").
    pub counter: String,
    pub threshold: u32,
    /// "unlock_building", "guard_dps_mult", or "farm_cap_mult".
    pub effect: String,
    pub value: f32,
    pub building: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GameData {
    pub config: GameConfig,
    pub species: DataRegistry<SpeciesDef>,
    pub buildings: DataRegistry<BuildingDef>,
    pub unlocks: Vec<UnlockDef>,
    pub balance: Balance,
}

impl GameData {
    pub fn load() -> Result<Self, String> {
        let config = load_embedded_json_labeled("game_config", GAME_CONFIG_JSON)?;
        let species = DataRegistry::from_embedded_json(SPECIES_JSON, "id")?;
        let buildings = DataRegistry::from_embedded_json(BUILDINGS_JSON, "id")?;
        let unlocks: Vec<UnlockDef> = load_embedded_json_labeled("unlocks", UNLOCKS_JSON)?;
        let balance = load_embedded_json_labeled("balance", BALANCE_JSON)?;

        Ok(Self {
            config,
            species,
            buildings,
            unlocks,
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
