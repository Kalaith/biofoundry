//! Gear: which item a job can wear, what an equipped item does, and the
//! indirect auto-equip loop that walks a worker to the stockpile.

use crate::data::{GameData, SpeciesDef};
use crate::simulation::jobs::routing::send_to;
use crate::state::creatures::{Creature, Job, Task};
use crate::state::GameSession;

/// The equipment job-affinity key for a job, if it can wear gear.
fn job_key(job: Job) -> Option<&'static str> {
    match job {
        Job::Miner => Some("miner"),
        Job::Carrier => Some("carrier"),
        Job::Smith => Some("smith"),
        Job::Guard => Some("guard"),
        _ => None,
    }
}

/// The value of the equipped item's `effect`, if this creature wears gear
/// with that effect. Callers fold in the identity (1.0 or 0.0).
pub(super) fn equip_effect(creature: &Creature, data: &GameData, effect: &str) -> Option<f32> {
    let def = data.equipment_def(creature.equipment.as_deref()?)?;
    (def.effect == effect).then_some(def.value)
}

/// A creature's carry capacity, including a Hauling Frame's bonus.
pub(super) fn carry_capacity(creature: &Creature, species: &SpeciesDef, data: &GameData) -> u32 {
    species.carry_capacity + equip_effect(creature, data, "carry_bonus").unwrap_or(0.0) as u32
}

/// The gear item this creature should be wearing but isn't — a banked item
/// matching its job. None if already equipped or nothing is waiting.
fn wants_gear(creature: &Creature, session: &GameSession, data: &GameData) -> Option<String> {
    if creature.equipment.is_some() {
        return None;
    }
    let key = job_key(creature.job)?;
    data.equipment
        .iter()
        .filter(|e| e.job == key)
        .find(|e| session.economy.gear_stock.get(&e.id).copied().unwrap_or(0) > 0)
        .map(|e| e.id.clone())
}

/// Indirect auto-equip: drop gear that no longer matches this creature's
/// job back to the pool, then fetch matching gear from the stockpile.
/// Returns true when it took over the tick (walking there to equip).
pub(super) fn tick_gear(
    creature: &mut Creature,
    session: &mut GameSession,
    data: &GameData,
) -> bool {
    // Drop job-mismatched gear (e.g. a reassigned goblin).
    if let Some(id) = creature.equipment.clone() {
        let job_ok = data
            .equipment_def(&id)
            .is_some_and(|e| job_key(creature.job) == Some(e.job.as_str()));
        if !job_ok {
            *session.economy.gear_stock.entry(id).or_insert(0) += 1;
            creature.equipment = None;
        }
    }
    // Fetch matching gear waiting at the stockpile.
    if let Some(item) = wants_gear(creature, session, data) {
        let stockpile = session.stockpile_pos();
        if creature.tile() == stockpile {
            if let Some(count) = session.economy.gear_stock.get_mut(&item) {
                if *count > 0 {
                    *count -= 1;
                    creature.equipment = Some(item);
                }
            }
            if creature.task == Task::GoEquip {
                creature.task = Task::Idle;
            }
        } else {
            send_to(creature, session, stockpile, Task::GoEquip);
            return true;
        }
    } else if creature.task == Task::GoEquip {
        creature.task = Task::Idle;
    }
    false
}
