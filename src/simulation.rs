//! Stateless simulation services.
//!
//! The sim advances on a fixed timestep decoupled from the render loop:
//! `Game` accumulates real frame time and calls `tick` zero or more times
//! per frame. All services take state in and mutate it explicitly — no
//! globals — so integration tests can run headless for thousands of ticks.

pub mod food;
pub mod jobs;

use crate::data::GameData;
use crate::state::creatures::Creature;
use crate::state::GameSession;

/// Fixed simulation timestep in seconds (10 ticks per second).
pub const SIM_DT: f32 = 0.1;

/// Cap on ticks consumed in one frame so a long hitch can't spiral.
pub const MAX_TICKS_PER_FRAME: u32 = 10;

/// Anything the UI should announce after a tick.
#[derive(Debug, Default)]
pub struct TickReport {
    pub deserters: Vec<Creature>,
    pub won_this_tick: bool,
}

/// Advance the simulation by one fixed step.
pub fn tick(session: &mut GameSession, data: &GameData) -> TickReport {
    session.tick += 1;
    let dt = SIM_DT;
    let balance = &data.balance;

    // Generators: the farm grows, wild patches regrow.
    session.economy.farm_mushrooms = (session.economy.farm_mushrooms
        + balance.farm_mushrooms_per_min / 60.0 * dt)
        .min(balance.farm_storage_cap);
    for regrow in session.patch_regrow.values_mut() {
        *regrow = (*regrow - dt).max(0.0);
    }

    // Workers, then the calorie ledger they drain. Jobs only ever add
    // food (cook batches), so the delta is this tick's production.
    let food_before = session.economy.food;
    jobs::tick_creatures(session, data, dt);
    let produced_per_min = (session.economy.food - food_before) / dt * 60.0;
    let smoothing = dt / 15.0; // ~15s time constant
    session.economy.production_ema_per_min +=
        (produced_per_min - session.economy.production_ema_per_min) * smoothing;
    let deserters = food::tick_hunger(session, data, dt);

    let mut won_this_tick = false;
    if !session.won
        && session.economy.ore_delivered >= balance.win_ore_delivered
        && session.economy.food >= balance.win_food_surplus
    {
        session.won = true;
        won_this_tick = true;
    }

    TickReport {
        deserters,
        won_this_tick,
    }
}

/// Seconds of simulated time elapsed.
pub fn sim_seconds(session: &GameSession) -> f32 {
    session.tick as f32 * SIM_DT
}

/// Spend ore to attract a beetle hauler (the Phase 1 upgrade decision).
/// Returns false when the stockpile can't afford it.
pub fn try_attract_beetle(session: &mut GameSession, data: &GameData) -> bool {
    let cost = data.balance.beetle_ore_cost;
    if session.economy.ore_delivered < cost {
        return false;
    }
    session.economy.ore_delivered -= cost;
    session.spawn_creature("beetle", crate::state::creatures::Job::Carrier);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::creatures::Job;

    fn boot(seed: u64) -> (GameData, GameSession) {
        let data = GameData::load().unwrap();
        let session = GameSession::new(&data, seed);
        (data, session)
    }

    fn boot_on_config_seed() -> (GameData, GameSession) {
        let data = GameData::load().unwrap();
        let session = GameSession::new(&data, data.config.world_seed);
        (data, session)
    }

    fn run_until(
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

    fn reassign(session: &mut GameSession, data: &GameData, from: Job, to: Job) -> bool {
        session.reassign(from, to, |s| {
            data.species.get(s).map(|d| d.reassignable).unwrap_or(false)
        })
    }

    #[test]
    fn ticks_accumulate_deterministically() {
        let (data, mut session) = boot(42);
        for _ in 0..600 {
            tick(&mut session, &data);
        }
        assert_eq!(session.tick, 600);
        assert!((sim_seconds(&session) - 60.0).abs() < 1e-3);
    }

    #[test]
    fn same_seed_same_outcome() {
        let (data, mut a) = boot(7);
        let (_, mut b) = boot(7);
        for _ in 0..3000 {
            tick(&mut a, &data);
            tick(&mut b, &data);
        }
        assert_eq!(a.economy.ore_delivered, b.economy.ore_delivered);
        assert!((a.economy.food - b.economy.food).abs() < 1e-3);
        assert_eq!(a.creatures.len(), b.creatures.len());
        for (ca, cb) in a.creatures.iter().zip(&b.creatures) {
            assert_eq!(ca.task, cb.task);
            assert!((ca.x - cb.x).abs() < 1e-4);
        }
    }

    /// The plan's scripted-by-balance first crisis: with the default job
    /// allocation the warren must run out of food roughly five minutes in
    /// (window 2–10 sim-minutes), telegraphed, not instant.
    #[test]
    fn default_allocation_hits_famine_around_five_minutes() {
        let (data, mut session) = boot_on_config_seed();

        let famine_at = run_until(
            &mut session,
            &data,
            12.0,
            |_, _| {},
            |s| s.economy.food <= 0.0,
        );

        let minutes = famine_at / 60.0;
        eprintln!("[balance probe] famine hits at {minutes:.1} sim-min");
        assert!(
            (2.0..=10.0).contains(&minutes),
            "famine should land ~5 min in, hit at {minutes:.1} min"
        );
        // It must be a brownout first, not an instant wipe.
        assert_eq!(session.economy.deserted, 0);
        assert!(!session.creatures.is_empty());
    }

    /// The famine is survivable by reassigning workers, and the game is
    /// winnable in one sitting on a fixed seed: react to the meter by
    /// moving miners onto the food economy, then win 50 ore + 100 food.
    #[test]
    fn sim_to_win_on_fixed_seed() {
        let (data, mut session) = boot_on_config_seed();
        let mut reacted = false;

        let won_at = run_until(
            &mut session,
            &data,
            40.0,
            |s, _| {
                // Player reads "time to empty" on the calorie meter and
                // shifts two miners into hauling before the blackout.
                if !reacted && s.economy.food < 15.0 {
                    let _ = reassign(s, &data, Job::Miner, Job::Carrier);
                    let _ = reassign(s, &data, Job::Miner, Job::Carrier);
                    reacted = true;
                }
            },
            |s| s.won,
        );

        assert!(reacted, "the famine never forced a reaction");
        assert!(session.won, "expected a win within 40 sim-minutes");
        assert_eq!(session.economy.deserted, 0, "no one should starve out");
        let minutes = won_at / 60.0;
        eprintln!(
            "[balance probe] won at {minutes:.1} sim-min with {} ore and {:.0} food",
            session.economy.ore_delivered, session.economy.food
        );
        assert!(
            minutes <= 35.0,
            "win took {minutes:.1} min; the slice should fit one sitting"
        );
    }

    /// Buying the beetle trades ore for hauling capacity.
    #[test]
    fn beetle_purchase_spends_ore_and_spawns_hauler() {
        let (data, mut session) = boot(3);
        session.economy.ore_delivered = data.balance.beetle_ore_cost + 5;

        let before = session.creatures.len();
        assert!(try_attract_beetle(&mut session, &data));
        assert_eq!(session.creatures.len(), before + 1);
        assert_eq!(session.economy.ore_delivered, 5);
        assert!(session.creatures.iter().any(|c| c.species == "beetle"));

        // Too poor now.
        assert!(!try_attract_beetle(&mut session, &data));
    }
}
