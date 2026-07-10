//! Stateless simulation services.
//!
//! The sim advances on a fixed timestep decoupled from the render loop:
//! `Game` accumulates real frame time and calls `tick` zero or more times
//! per frame. All services take state in and mutate it explicitly — no
//! globals — so integration tests can run headless for thousands of ticks.

use crate::state::GameSession;

/// Fixed simulation timestep in seconds (10 ticks per second).
pub const SIM_DT: f32 = 0.1;

/// Cap on ticks consumed in one frame so a long hitch can't spiral.
pub const MAX_TICKS_PER_FRAME: u32 = 10;

/// Advance the simulation by one fixed step.
///
/// Phase 0 only counts ticks; Phase 1 adds creature jobs, hunger, and the
/// calorie ledger here.
pub fn tick(session: &mut GameSession) {
    session.tick += 1;
}

/// Seconds of simulated time elapsed.
pub fn sim_seconds(session: &GameSession) -> f32 {
    session.tick as f32 * SIM_DT
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::state::GameSession;

    #[test]
    fn ticks_accumulate_deterministically() {
        let data = GameData::load().unwrap();
        let mut session = GameSession::new(&data.config, 42);

        for _ in 0..600 {
            tick(&mut session);
        }

        assert_eq!(session.tick, 600);
        assert!((sim_seconds(&session) - 60.0).abs() < f32::EPSILON);
    }
}
