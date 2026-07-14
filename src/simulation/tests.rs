//! Headless simulation tests, grouped by what they pin down. This file is
//! the shared harness; each submodule is one suite.

mod campaign;
mod crafting;
mod determinism;
mod economy;
mod mining;
mod wild;

use super::*;
use crate::state::creatures::Job;

pub(super) fn boot(seed: u64) -> (GameData, GameSession) {
    let data = GameData::load().unwrap();
    let session = GameSession::new(&data, seed);
    (data, session)
}

/// Boot on the seed the shipped game uses — the one the balance probes
/// are tuned against.
pub(super) fn boot_on_config_seed() -> (GameData, GameSession) {
    let data = GameData::load().unwrap();
    let session = GameSession::new(&data, data.config.world_seed);
    (data, session)
}

/// Tick until `stop`, running `policy` (the simulated player) each tick.
/// Returns the sim-seconds elapsed.
pub(super) fn run_until(
    session: &mut GameSession,
    data: &GameData,
    max_minutes: f32,
    mut policy: impl FnMut(&mut GameSession, f32),
    mut stop: impl FnMut(&GameSession) -> bool,
) -> f32 {
    let max_ticks = (max_minutes * 60.0 / SIM_DT) as u64;
    for _ in 0..max_ticks {
        tick(session, data);
        let t = sim_seconds(session);
        policy(session, t);
        if stop(session) {
            return t;
        }
    }
    sim_seconds(session)
}

pub(super) fn reassign(session: &mut GameSession, data: &GameData, from: Job, to: Job) -> bool {
    session.reassign(from, to, |s| {
        data.species.get(s).map(|d| d.reassignable).unwrap_or(false)
    })
}
