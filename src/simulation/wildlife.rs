//! Wildlife simulation: wild beetle wandering + trapping, gnarl raids on
//! the food stores, raider combat against guards, and the capture /
//! study / adapt progression (unlocks, study pens, breeding pits).

use crate::data::GameData;
use crate::simulation::nav;
use crate::state::creatures::Job;
use crate::state::wildlife::{WildBehavior, WildCreature};
use crate::state::GameSession;

/// Wildlife events the UI should announce.
#[derive(Debug, Default)]
pub struct WildReport {
    pub raid_started: bool,
    pub raid_survived: bool,
    pub captured: u32,
    pub guards_killed: u32,
    /// Names of unlocks granted this tick.
    pub unlocked: Vec<String>,
    pub bred_beetle: bool,
}

pub fn tick_wildlife(session: &mut GameSession, data: &GameData, dt: f32) -> WildReport {
    let mut report = WildReport::default();

    spawn_wild_beetles(session, data, dt);
    report.raid_started = spawn_raids(session, data, dt);
    behave(session, data, dt, &mut report);
    capture_at_traps(session, &mut report);
    settle_raid(session, &mut report);
    study_and_breed(session, data, dt, &mut report);
    track_famine(session, data);
    grant_unlocks(session, data, &mut report);

    report
}

fn spawn_wild_beetles(session: &mut GameSession, data: &GameData, dt: f32) {
    session.wild_spawn_in -= dt;
    if session.wild_spawn_in > 0.0 {
        return;
    }
    session.wild_spawn_in = data.balance.wild_beetle_spawn_sec;

    let wild_beetles = session
        .wilds
        .iter()
        .filter(|w| w.species == "wild_beetle")
        .count();
    if wild_beetles >= data.balance.wild_beetle_max {
        return;
    }
    let Some(tile) = nav::far_walkable_tile(session, 10) else {
        return;
    };
    let hp = data
        .species
        .get("wild_beetle")
        .map(|s| s.max_hp)
        .unwrap_or(30.0);
    let id = session.next_wild_id;
    session.next_wild_id += 1;
    session.wilds.push(WildCreature::new(
        id,
        "wild_beetle",
        tile,
        hp,
        WildBehavior::Wander { next_move_in: 1.0 },
    ));
}

fn spawn_raids(session: &mut GameSession, data: &GameData, dt: f32) -> bool {
    if session.raid_active {
        return false;
    }
    session.raid_in -= dt;
    if session.raid_in > 0.0 {
        return false;
    }
    session.raid_in = data.balance.raid_interval_sec;

    let size = ((session.raids_launched as usize) + 1).min(data.balance.raid_size_max);
    let hp = data.species.get("gnarl").map(|s| s.max_hp).unwrap_or(26.0);
    let mut spawned = false;
    for _ in 0..size {
        let Some(tile) = nav::far_walkable_tile(session, 12) else {
            continue;
        };
        let id = session.next_wild_id;
        session.next_wild_id += 1;
        session.wilds.push(WildCreature::new(
            id,
            "gnarl",
            tile,
            hp,
            WildBehavior::Raid {
                origin: tile,
                eaten: 0.0,
                fleeing: false,
            },
        ));
        spawned = true;
    }
    if spawned {
        session.raids_launched += 1;
        session.raid_active = true;
    }
    spawned
}

/// Movement + per-behavior logic for every wild creature.
fn behave(session: &mut GameSession, data: &GameData, dt: f32, report: &mut WildReport) {
    let larder = session.stockpile_pos();
    let mut wilds = std::mem::take(&mut session.wilds);

    for wild in &mut wilds {
        let speed = data
            .species
            .get(&wild.species)
            .map(|s| s.move_tiles_per_sec)
            .unwrap_or(2.0);

        if !wild.path.is_empty() {
            nav::walk(&mut wild.x, &mut wild.y, &mut wild.path, speed, dt);
            continue;
        }

        let here = wild.tile();
        match wild.behavior.clone() {
            WildBehavior::Wander { next_move_in } => {
                let mut timer = next_move_in - dt;
                if timer <= 0.0 {
                    timer = 1.5;
                    if let Some(step) = nav::random_step(session, here) {
                        wild.path = vec![step];
                    }
                }
                wild.behavior = WildBehavior::Wander {
                    next_move_in: timer,
                };
            }
            WildBehavior::Raid {
                origin,
                mut eaten,
                mut fleeing,
            } => {
                if fleeing {
                    // Reached home (or boxed in): vanish into the dark.
                    wild.hp = -1.0;
                    continue;
                }
                if here.manhattan_distance(&larder) <= 1 {
                    // Feast on the stockpile — the brownout raiders cause
                    // is the whole point of defending.
                    let bite = data.balance.raider_food_eat_per_min / 60.0 * dt;
                    let taken = bite.min(session.economy.food);
                    session.economy.food -= taken;
                    eaten += taken;
                    if eaten >= data.balance.raider_flee_after_eaten {
                        fleeing = true;
                        if let Some(path) = nav::find_path(session, here, origin) {
                            wild.path = path;
                        }
                    }
                } else if let Some(path) = nav::find_path(session, here, larder) {
                    wild.path = path;
                } else {
                    // Larder unreachable (walled off): give up.
                    fleeing = true;
                }
                wild.behavior = WildBehavior::Raid {
                    origin,
                    eaten,
                    fleeing,
                };
            }
        }

        // Raiders bite back at adjacent guards.
        if wild.is_raider() {
            let dps = data
                .species
                .get(&wild.species)
                .map(|s| s.attack_dps)
                .unwrap_or(2.0);
            let mut i = 0;
            while i < session.creatures.len() {
                let guard = &mut session.creatures[i];
                if guard.job == Job::Guard && nav::dist_sq(wild.x, wild.y, guard.x, guard.y) <= 1.7
                {
                    guard.hp -= dps * dt;
                    if guard.hp <= 0.0 {
                        session.creatures.remove(i);
                        session.economy.killed += 1;
                        report.guards_killed += 1;
                        continue;
                    }
                }
                i += 1;
            }
        }
    }

    wilds.retain(|w| w.hp > 0.0);
    session.wilds = wilds;
}

/// Wild beetles adjacent to a snare trap get captured; the trap is spent.
fn capture_at_traps(session: &mut GameSession, report: &mut WildReport) {
    let traps: Vec<_> = session.buildings_of("trap").map(|b| b.pos).collect();
    if traps.is_empty() {
        return;
    }

    let mut sprung: Vec<macroquad_toolkit::grid::TilePos> = Vec::new();
    session.wilds.retain(|wild| {
        if wild.species != "wild_beetle" {
            return true;
        }
        let near = traps
            .iter()
            .find(|t| wild.tile().manhattan_distance(t) <= 1 && !sprung.contains(t));
        if let Some(&trap) = near {
            sprung.push(trap);
            false
        } else {
            true
        }
    });

    let captures = sprung.len() as u32;
    if captures > 0 {
        session.progress.beetles_captured += captures;
        session.progress.specimens += captures;
        report.captured += captures;
        session.buildings.retain(|b| !sprung.contains(&b.pos));
    }
}

/// A raid ends when no raiders remain; the warren survived it.
fn settle_raid(session: &mut GameSession, report: &mut WildReport) {
    if session.raid_active && !session.wilds.iter().any(|w| w.is_raider()) {
        session.raid_active = false;
        session.progress.raids_survived += 1;
        report.raid_survived = true;
    }
}

/// Study pens turn housed specimens into knowledge; a breeding pit slowly
/// domesticates new beetle haulers from the brood stock.
fn study_and_breed(session: &mut GameSession, data: &GameData, dt: f32, report: &mut WildReport) {
    if session.buildings_of("study_pen").next().is_some() {
        session.progress.knowledge += session.progress.specimens as f32
            * data.balance.study_knowledge_per_specimen_min
            / 60.0
            * dt;
    }

    if session.buildings_of("breeding_pit").next().is_some() && session.progress.specimens >= 2 {
        session.breed_in -= dt;
        if session.breed_in <= 0.0 {
            session.breed_in = data.balance.breed_interval_sec;
            let beetles = session
                .creatures
                .iter()
                .filter(|c| c.species == "beetle")
                .count() as u32;
            if beetles < data.balance.bred_beetle_cap {
                session.spawn_creature(data, "beetle", Job::Carrier);
                report.bred_beetle = true;
            }
        }
    }
}

/// Blackout episodes count as survived once food recovers.
fn track_famine(session: &mut GameSession, data: &GameData) {
    if !session.famine_active && session.economy.food <= 0.0 && !session.creatures.is_empty() {
        session.famine_active = true;
    } else if session.famine_active && session.economy.food >= data.balance.famine_recover_food {
        session.famine_active = false;
        session.progress.famines_survived += 1;
    }
}

/// Data-driven unlock grants: counters cross thresholds as a side effect
/// of play.
fn grant_unlocks(session: &mut GameSession, data: &GameData, report: &mut WildReport) {
    for unlock in &data.unlocks {
        if session.unlocked.contains(&unlock.id) {
            continue;
        }
        if counter_value(session, &unlock.counter) >= unlock.threshold {
            session.unlocked.insert(unlock.id.clone());
            report.unlocked.push(unlock.name.clone());
        }
    }
}

/// Session-wide counter lookup: progression counters plus economy stats.
fn counter_value(session: &GameSession, name: &str) -> u32 {
    match name {
        "ingots_forged" => session.economy.ingots_forged,
        other => session.progress.counter(other),
    }
}

/// Effect helpers: unlock-modified values.
pub fn guard_dps(session: &GameSession, data: &GameData) -> f32 {
    let mut dps = data.balance.guard_dps;
    for unlock in &data.unlocks {
        if unlock.effect == "guard_dps_mult" && session.unlocked.contains(&unlock.id) {
            dps *= unlock.value;
        }
    }
    dps
}

pub fn farm_cap(session: &GameSession, data: &GameData) -> f32 {
    let mut cap = data.balance.farm_storage_cap;
    for unlock in &data.unlocks {
        if unlock.effect == "farm_cap_mult" && session.unlocked.contains(&unlock.id) {
            cap *= unlock.value;
        }
    }
    cap
}
