//! Goods logistics, independent of who does the hauling: what a tile
//! yields, where a load belongs, how much ore the warren still wants, and
//! what happens when a load lands.

use crate::data::{GameData, SpeciesDef};
use crate::simulation::jobs::equipment::carry_capacity;
use crate::state::creatures::{Creature, Good};
use crate::state::structures::Building;
use crate::state::GameSession;
use macroquad_toolkit::grid::TilePos;

/// Ore target for an ore-consuming forge, or None if it isn't one.
fn forge_ore_target(kind: &str, data: &GameData) -> Option<u32> {
    match kind {
        "blacksmith" => Some(data.balance.blacksmith_ore_target),
        "smelter" => Some(data.balance.smelter_ore_target),
        _ => None,
    }
}

/// Where should carried ore go: an unfinished site, else the nearest forge
/// (blacksmith or smelter) under its ore target.
pub(super) fn ore_destination(
    creature: &Creature,
    session: &GameSession,
    data: &GameData,
) -> Option<TilePos> {
    let from = creature.tile();
    if let Some(site) = session
        .build_sites
        .iter()
        .filter(|s| !s.complete() && s.remaining() > 0)
        .map(|s| s.pos)
        .min_by_key(|p| (p.manhattan_distance(&from), p.x, p.y))
    {
        return Some(site);
    }
    session
        .buildings
        .iter()
        .filter(|b| {
            forge_ore_target(&b.kind, data).is_some_and(|t| (b.stock(Good::Ore) as u32) < t)
        })
        .map(|b| b.pos)
        .min_by_key(|p| (p.manhattan_distance(&from), p.x, p.y))
}

/// Total ore the warren wants moved: site remainders, blacksmith top-ups,
/// and smelter top-ups. The blacksmith is the primary early forge so it's
/// fed freely; smelters only draw above the bank reserve (with an emergency
/// trickle), so endless salamander smelting can't starve construction.
pub(super) fn ore_wanted(session: &GameSession, data: &GameData) -> u32 {
    let sites: u32 = session.build_sites.iter().map(|s| s.remaining()).sum();
    let blacksmiths: u32 = session
        .buildings_of("blacksmith")
        .map(|b| {
            data.balance
                .blacksmith_ore_target
                .saturating_sub(b.stock(Good::Ore) as u32)
        })
        .sum();
    let smelters: u32 = if session.economy.ore_stock > data.balance.smelter_bank_reserve {
        session
            .buildings_of("smelter")
            .map(|b| {
                data.balance
                    .smelter_ore_target
                    .saturating_sub(b.stock(Good::Ore) as u32)
            })
            .sum()
    } else {
        // Below the reserve, dens still get an emergency trickle so the
        // salamander never starves while the bank saves up.
        session
            .buildings_of("smelter")
            .map(|b| 2u32.saturating_sub(b.stock(Good::Ore) as u32))
            .sum()
    };
    sites + blacksmiths + smelters
}

/// What a fetch at this tile would yield right now.
pub(super) fn fetchable_good(session: &GameSession, source: TilePos) -> Option<Good> {
    if let Some(building) = session.building_at(source) {
        return match building.kind.as_str() {
            "farm" if building.stock(Good::Mushroom) >= 1.0 => Some(Good::Mushroom),
            "mine" if building.stock(Good::Ore) >= 1.0 => Some(Good::Ore),
            "kiln" if building.stock(Good::Charcoal) >= 1.0 => Some(Good::Charcoal),
            // The forges emit ingots into their output buffer.
            "blacksmith" | "smelter" if building.stock(Good::Ingot) >= 1.0 => Some(Good::Ingot),
            _ => None,
        };
    }
    if session.patch_regrow.get(&source).is_some_and(|t| *t <= 0.0) {
        return Some(Good::Mushroom);
    }
    if session
        .sporewood_regrow
        .get(&source)
        .is_some_and(|t| *t <= 0.0)
    {
        return Some(Good::Wood);
    }
    None
}

pub(super) fn harvest_source(
    creature: &mut Creature,
    session: &mut GameSession,
    data: &GameData,
    species: &SpeciesDef,
    source: TilePos,
) {
    let Some(good) = fetchable_good(session, source) else {
        return;
    };
    let space = carry_capacity(creature, species, data).saturating_sub(creature.carried(good));
    if space == 0 {
        return;
    }
    if let Some(building) = session.building_at_mut(source) {
        let take = building.take_stock(good, space as f32).floor() as u32;
        // take_stock floors can strand fractions; put any remainder back.
        creature.add_carried(good, take);
        return;
    }
    match good {
        Good::Mushroom => {
            session
                .patch_regrow
                .insert(source, data.balance.patch_regrow_sec);
            creature.add_carried(Good::Mushroom, 1);
        }
        Good::Wood => {
            session
                .sporewood_regrow
                .insert(source, data.balance.sporewood_regrow_sec);
            creature.add_carried(Good::Wood, 1);
        }
        _ => {}
    }
}

/// Deposit the load at a building or build site.
pub(super) fn drop_load(
    creature: &mut Creature,
    session: &mut GameSession,
    data: &GameData,
    pos: TilePos,
) {
    // Build sites take ore.
    if let Some(site) = session.build_sites.iter_mut().find(|s| s.pos == pos) {
        let deliver = creature.take_carried(Good::Ore, site.remaining());
        site.ore_delivered += deliver;
        complete_finished_sites(session, data);
        return;
    }
    let Some(building) = session.buildings.iter_mut().find(|b| b.pos == pos) else {
        return;
    };
    match building.kind.as_str() {
        "cook_pot" => {
            let n = creature.take_carried(Good::Mushroom, u32::MAX);
            building.add_stock(Good::Mushroom, n as f32);
        }
        "kiln" => {
            let n = creature.take_carried(Good::Wood, u32::MAX);
            building.add_stock(Good::Wood, n as f32);
        }
        "blacksmith" => {
            let ore = creature.take_carried(Good::Ore, u32::MAX);
            building.add_stock(Good::Ore, ore as f32);
        }
        "smelter" => {
            let ore = creature.take_carried(Good::Ore, u32::MAX);
            building.add_stock(Good::Ore, ore as f32);
            let charcoal = creature.take_carried(Good::Charcoal, u32::MAX);
            building.add_stock(Good::Charcoal, charcoal as f32);
        }
        _ => {}
    }
}

/// Turn fully-supplied ghosts into working buildings.
fn complete_finished_sites(session: &mut GameSession, data: &GameData) {
    let mut i = 0;
    while i < session.build_sites.len() {
        if session.build_sites[i].complete() {
            let site = session.build_sites.remove(i);
            let building = if site.kind == "mine" {
                Building::mine(site.pos, data.balance.mine_reserve)
            } else {
                Building::new(&site.kind, site.pos)
            };
            session.buildings.push(building);
        } else {
            i += 1;
        }
    }
}
