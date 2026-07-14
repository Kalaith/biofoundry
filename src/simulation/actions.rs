//! Player-initiated simulation actions: the decisions the HUD can take on
//! the warren's behalf. Each one checks its own gates and returns false
//! when the warren can't afford or hasn't unlocked it.

use crate::data::GameData;
use crate::state::creatures::Job;
use crate::state::GameSession;

/// Spend banked ore to attract a beetle hauler (the Phase 1 upgrade
/// decision). Returns false when the stockpile can't afford it.
pub fn try_attract_beetle(session: &mut GameSession, data: &GameData) -> bool {
    let cost = data.balance.beetle_ore_cost;
    if session.economy.ore_stock < cost {
        return false;
    }
    session.economy.ore_stock -= cost;
    session.spawn_creature(data, "beetle", Job::Carrier);
    true
}

/// Spend banked ore to attract a salamander smelter. Requires a smelter
/// den so the creature has somewhere to live and eat.
pub fn try_attract_salamander(session: &mut GameSession, data: &GameData) -> bool {
    let cost = data.balance.salamander_ore_cost;
    if session.economy.ore_stock < cost || session.buildings_of("smelter").next().is_none() {
        return false;
    }
    session.economy.ore_stock -= cost;
    session.spawn_creature(data, "salamander", Job::Smelter);
    true
}

/// Breed a Hobgoblin at the Breeding Pit — a heavyweight worker (×2 work
/// speed, ×2.5 upkeep). Gated on the ingot unlock and paid in banked ingots.
pub fn try_breed_hobgoblin(session: &mut GameSession, data: &GameData) -> bool {
    if !session.unlocked.contains("hobgoblin")
        || session.buildings_of("breeding_pit").next().is_none()
        || session.economy.ingots_stock < data.balance.hobgoblin_ingot_cost
    {
        return false;
    }
    session.economy.ingots_stock -= data.balance.hobgoblin_ingot_cost;
    session.spawn_creature(data, "hobgoblin", Job::Idle);
    true
}

/// Breed a Goblin Overseer — the living beacon that speeds every worker in
/// its aura. One at a time (one per district). Paid in banked ingots.
pub fn try_breed_overseer(session: &mut GameSession, data: &GameData) -> bool {
    if !session.unlocked.contains("overseer")
        || session.buildings_of("breeding_pit").next().is_none()
        || session.creatures.iter().any(|c| c.species == "overseer")
        || session.economy.ingots_stock < data.balance.overseer_ingot_cost
    {
        return false;
    }
    session.economy.ingots_stock -= data.balance.overseer_ingot_cost;
    session.spawn_creature(data, "overseer", Job::Idle);
    true
}

/// Place a construction ghost. Returns false when the spot or kind is
/// invalid; carriers deliver the ore and finish it.
pub fn try_place_build_site(
    session: &mut GameSession,
    data: &GameData,
    kind: &str,
    pos: macroquad_toolkit::grid::TilePos,
) -> bool {
    let Some(def) = data.buildings.get(kind) else {
        return false;
    };
    if !def.buildable || !session.building_unlocked(def) || !session.can_place_kind(kind, pos) {
        return false;
    }
    session
        .build_sites
        .push(crate::state::structures::BuildSite {
            kind: kind.to_owned(),
            pos,
            ore_needed: def.cost_ore,
            ore_delivered: 0,
        });
    session.tutorial_built = true;
    true
}
