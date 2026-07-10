//! The food grid: calorie ledger math, hunger (brownout), and starvation
//! (blackout → desertion). Food is treated exactly like electricity.

use crate::data::GameData;
use crate::state::creatures::Creature;
use crate::state::GameSession;

/// Total upkeep draw across all creatures, in food per minute.
pub fn consumption_per_min(session: &GameSession, data: &GameData) -> f32 {
    session
        .creatures
        .iter()
        .map(|c| {
            let base = data
                .species
                .get(&c.species)
                .map(|s| s.food_per_min)
                .unwrap_or(0.0);
            GameSession::upkeep_per_min(c, base, &data.balance)
        })
        .sum()
}

/// Seconds until the stockpile empties at current net drain; `None` when
/// the balance is non-negative.
pub fn time_to_empty_sec(food: f32, production: f32, consumption: f32) -> Option<f32> {
    let net = consumption - production;
    if net <= 0.0 {
        None
    } else {
        Some(food / net * 60.0)
    }
}

/// Drain the stockpile by total upkeep and update every creature's
/// satiation. Returns creatures that deserted this tick (already removed).
///
/// Diets: "food" eaters draw the shared stockpile; "charcoal" eaters
/// (salamanders) refill by consuming charcoal at their den when they
/// smelt, and only hunger slowly between meals.
pub fn tick_hunger(session: &mut GameSession, data: &GameData, dt: f32) -> Vec<Creature> {
    let consumption = consumption_per_min(session, data);
    let fed = session.economy.food > 0.0;
    session.economy.food = (session.economy.food - consumption / 60.0 * dt).max(0.0);

    let b = &data.balance;
    for creature in &mut session.creatures {
        let species = data.species.get(&creature.species);
        let eats_food = species.map(|s| s.diet == "food").unwrap_or(true);
        // Fed creatures knit wounds between fights.
        if creature.satiation > 0.66 {
            let max_hp = species.map(|s| s.max_hp).unwrap_or(20.0);
            creature.hp = (creature.hp + b.hp_regen_per_sec * dt).min(max_hp);
        }
        if eats_food {
            if fed {
                creature.satiation = (creature.satiation + dt / b.satiation_recover_sec).min(1.0);
                creature.starving_for = 0.0;
            } else {
                creature.satiation = (creature.satiation - dt / b.satiation_drain_sec).max(0.0);
                if creature.satiation <= 0.0 {
                    creature.starving_for += dt;
                }
            }
        } else {
            // Charcoal eaters: meals happen at the den (jobs.rs); here
            // they just get slowly hungrier.
            creature.satiation = (creature.satiation - dt / b.salamander_hunger_drain_sec).max(0.0);
            if creature.satiation <= 0.0 {
                creature.starving_for += dt;
            }
        }
    }

    let desert_after = b.desert_after_starving_sec;
    let mut deserters = Vec::new();
    let mut i = 0;
    while i < session.creatures.len() {
        if session.creatures[i].starving_for >= desert_after {
            deserters.push(session.creatures.remove(i));
        } else {
            i += 1;
        }
    }
    session.economy.deserted += deserters.len() as u32;
    deserters
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::state::creatures::Job;

    fn boot() -> (GameData, GameSession) {
        let data = GameData::load().unwrap();
        let session = GameSession::new(&data, 42);
        (data, session)
    }

    #[test]
    fn ledger_matches_tier0_table() {
        let (data, session) = boot();
        let b = &data.balance;

        // Miners and carriers draw base upkeep; cooks draw the cook factor.
        let base = data.species.get("goblin").unwrap().food_per_min;
        let expected = (b.start_miners + b.start_carriers) as f32 * base
            + b.start_cooks as f32 * base * b.cook_upkeep_factor;
        let consumption = consumption_per_min(&session, &data);
        assert!((consumption - expected).abs() < 1e-4);
    }

    #[test]
    fn idle_creatures_draw_reduced_rate() {
        let (data, mut session) = boot();
        let before = consumption_per_min(&session, &data);

        for c in &mut session.creatures {
            if c.job == Job::Miner {
                c.job = Job::Idle;
            }
        }
        let after = consumption_per_min(&session, &data);
        assert!(after < before);
    }

    #[test]
    fn stockpile_drains_and_satiation_recovers_while_fed() {
        let (data, mut session) = boot();
        let food_before = session.economy.food;
        for c in &mut session.creatures {
            c.satiation = 0.5;
        }

        let deserters = tick_hunger(&mut session, &data, 1.0);

        assert!(deserters.is_empty());
        assert!(session.economy.food < food_before);
        assert!(session.creatures.iter().all(|c| c.satiation > 0.5));
    }

    #[test]
    fn empty_stockpile_causes_brownout_then_desertion() {
        let (data, mut session) = boot();
        session.economy.food = 0.0;

        // Brownout: satiation decays, work speed drops.
        let drain = data.balance.satiation_drain_sec;
        let steps = (drain * 0.75 / 0.1) as usize;
        for _ in 0..steps {
            tick_hunger(&mut session, &data, 0.1);
        }
        assert!(session.creatures.iter().all(|c| c.work_speed() < 1.0));

        // Blackout: sustained starvation deserts everyone eventually.
        let total = session.creatures.len();
        let more = ((drain + data.balance.desert_after_starving_sec + 2.0) / 0.1) as usize;
        for _ in 0..more {
            tick_hunger(&mut session, &data, 0.1);
        }
        assert!(session.creatures.is_empty());
        assert_eq!(session.economy.deserted as usize, total);
    }

    #[test]
    fn refeeding_recovers_a_brownout() {
        let (data, mut session) = boot();
        session.economy.food = 0.0;
        for _ in 0..300 {
            tick_hunger(&mut session, &data, 0.1);
        }
        assert!(session.creatures.iter().all(|c| c.work_speed() < 1.0));

        session.economy.food = 50.0;
        for _ in 0..((data.balance.satiation_recover_sec / 0.1) as usize + 10) {
            tick_hunger(&mut session, &data, 0.1);
        }
        assert!(session.creatures.iter().all(|c| c.work_speed() == 1.0));
        assert!(session.creatures.iter().all(|c| c.starving_for == 0.0));
    }

    #[test]
    fn time_to_empty_matches_net_drain() {
        assert_eq!(time_to_empty_sec(10.0, 5.0, 5.0), None);
        assert_eq!(time_to_empty_sec(10.0, 6.0, 5.0), None);
        let t = time_to_empty_sec(10.0, 0.0, 6.0).unwrap();
        assert!((t - 100.0).abs() < 1e-4);
    }
}
