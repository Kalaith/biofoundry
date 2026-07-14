//! Creature job AI: each job is a small state machine in its own module
//! (`miner`, `carrier`, `cook`, `smith`, `guard`, `smelter`), and this file
//! is the per-tick dispatcher that picks one for each creature.
//!
//! Movement is generic (walk the cached path), work runs on arrival. Paths
//! are computed once per target (toolkit BFS) and only recomputed when a
//! new target is chosen — the performance guardrail from the plan.

mod carrier;
mod cook;
mod equipment;
mod guard;
mod hauling;
mod miner;
mod routing;
mod smelter;
mod smith;

use crate::data::{GameData, SpeciesDef};
use crate::simulation::nav;
use crate::state::creatures::{Creature, Job, Task};
use crate::state::GameSession;
use miner::MineClaims;

pub fn tick_creatures(session: &mut GameSession, data: &GameData, dt: f32) {
    // Take the creature list out so each creature can mutate the rest of
    // the session (economy, veins, patches, tiles) without aliasing.
    let mut creatures = std::mem::take(&mut session.creatures);

    // Snapshot Mine slot claims from current tasks; miners deciding this
    // tick read and update it so they spread across open slots.
    let mut claims: MineClaims = MineClaims::new();
    for c in &creatures {
        if let Task::GoMine(p) | Task::WorkMine(p) = &c.task {
            *claims.entry(*p).or_insert(0) += 1;
        }
    }

    // Overseer posts: workers within an aura radius labour faster. Snapshot
    // positions before the loop (creatures are taken out of the session).
    let overseers: Vec<(f32, f32)> = creatures
        .iter()
        .filter(|c| c.species == "overseer")
        .map(|c| (c.x, c.y))
        .collect();

    for creature in &mut creatures {
        tick_creature(creature, session, data, dt, &mut claims, &overseers);
    }
    session.creatures = creatures;
}

fn tick_creature(
    creature: &mut Creature,
    session: &mut GameSession,
    data: &GameData,
    dt: f32,
    claims: &mut MineClaims,
    overseers: &[(f32, f32)],
) {
    let Some(species) = data.species.get(&creature.species).cloned() else {
        return;
    };

    if !creature.path.is_empty() {
        walk(creature, &species, dt);
        return;
    }

    // Auto-equip: drop job-mismatched gear, fetch matching gear waiting at
    // the stockpile. Takes over the tick while walking there.
    if equipment::tick_gear(creature, session, data) {
        return;
    }

    // Task work speed = brownout × species multiplier (a Hobgoblin works
    // ×2) × any Overseer aura the worker stands in. These stack with
    // per-job equipment multipliers applied at each work site.
    let work_boost = species.work_mult * overseer_aura(creature, overseers, data);

    match creature.job {
        Job::Idle => {
            if creature.task != Task::Idle {
                creature.clear_task();
            }
        }
        Job::Miner => miner::tick_miner(creature, session, data, dt, claims, work_boost),
        Job::Carrier => carrier::tick_carrier(creature, session, data, &species, dt, work_boost),
        Job::Cook => cook::tick_cook(creature, session, data, dt, work_boost),
        Job::Smith => smith::tick_smith(creature, session, data, dt, work_boost),
        Job::Guard => guard::tick_guard(creature, session, data, dt, work_boost),
        Job::Smelter => smelter::tick_smelter(creature, session, data, dt, work_boost),
    }
}

/// The Overseer aura multiplier for a worker: ×`overseer_aura_mult` when it
/// stands within `overseer_aura_radius` of any Overseer, else 1.0.
fn overseer_aura(creature: &Creature, overseers: &[(f32, f32)], data: &GameData) -> f32 {
    if overseers.is_empty() {
        return 1.0;
    }
    let r2 = data.balance.overseer_aura_radius * data.balance.overseer_aura_radius;
    let in_range = overseers.iter().any(|(ox, oy)| {
        let dx = ox - creature.x;
        let dy = oy - creature.y;
        dx * dx + dy * dy <= r2
    });
    if in_range {
        data.balance.overseer_aura_mult
    } else {
        1.0
    }
}

/// Advance along the path at species speed scaled by the brownout curve.
fn walk(creature: &mut Creature, species: &SpeciesDef, dt: f32) {
    let speed = species.move_tiles_per_sec * creature.work_speed();
    nav::walk(
        &mut creature.x,
        &mut creature.y,
        &mut creature.path,
        speed,
        dt,
    );
}
