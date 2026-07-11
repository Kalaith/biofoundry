//! Stateless simulation services.
//!
//! The sim advances on a fixed timestep decoupled from the render loop:
//! `Game` accumulates real frame time and calls `tick` zero or more times
//! per frame. All services take state in and mutate it explicitly — no
//! globals — so integration tests can run headless for thousands of ticks.

pub mod food;
pub mod jobs;
pub mod nav;
pub mod wildlife;

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
    pub factory_this_tick: bool,
    pub worm_this_tick: bool,
    pub wild: wildlife::WildReport,
}

/// Advance the simulation by one fixed step.
pub fn tick(session: &mut GameSession, data: &GameData) -> TickReport {
    session.tick += 1;
    let dt = SIM_DT;
    let balance = &data.balance;

    // Generators: farms grow, kilns smoulder, wild flora regrows.
    use crate::state::creatures::Good;
    let farm_cap = wildlife::farm_cap(session, data);
    for building in session.buildings.iter_mut() {
        match building.kind.as_str() {
            "farm" => {
                let grown = (building.stock(Good::Mushroom)
                    + balance.farm_mushrooms_per_min / 60.0 * dt)
                    .min(farm_cap);
                building.stocks.insert(Good::Mushroom, grown);
            }
            "kiln" => {
                let converted =
                    (balance.kiln_charcoal_per_min / 60.0 * dt).min(building.stock(Good::Wood));
                if converted > 0.0 {
                    building.take_stock(Good::Wood, converted);
                    building.add_stock(Good::Charcoal, converted);
                }
            }
            _ => {}
        }
    }
    for regrow in session.patch_regrow.values_mut() {
        *regrow = (*regrow - dt).max(0.0);
    }
    for regrow in session.sporewood_regrow.values_mut() {
        *regrow = (*regrow - dt).max(0.0);
    }

    // Workers, then the calorie ledger they drain. Jobs only ever add
    // food (cook batches), so the delta is this tick's production. Ore
    // banked and ingots forged are tracked the same way for the factory
    // dashboard (the food grid generalized to every chain).
    let food_before = session.economy.food;
    let ore_before = session.economy.ore_delivered_total;
    let ingots_before = session.economy.ingots_forged;
    jobs::tick_creatures(session, data, dt);
    let smoothing = dt / 15.0; // ~15s time constant
    let produced_per_min = ((session.economy.food - food_before) / dt * 60.0).max(0.0);
    session.economy.production_ema_per_min +=
        (produced_per_min - session.economy.production_ema_per_min) * smoothing;
    let ore_per_min = (session.economy.ore_delivered_total - ore_before) as f32 / dt * 60.0;
    session.economy.ore_ema_per_min += (ore_per_min - session.economy.ore_ema_per_min) * smoothing;
    let ingot_per_min = (session.economy.ingots_forged - ingots_before) as f32 / dt * 60.0;
    session.economy.ingot_ema_per_min +=
        (ingot_per_min - session.economy.ingot_ema_per_min) * smoothing;
    let wild = wildlife::tick_wildlife(session, data, dt);
    let deserters = food::tick_hunger(session, data, dt);

    let mut won_this_tick = false;
    if !session.won
        && session.economy.ore_delivered_total >= balance.win_ore_delivered
        && session.economy.food >= balance.win_food_surplus
    {
        session.won = true;
        won_this_tick = true;
    }

    let mut factory_this_tick = false;
    if !session.factory_complete && session.economy.ingots_forged >= balance.win2_ingots {
        session.factory_complete = true;
        factory_this_tick = true;
    }

    // The Worm Shrine: offerings drain the larder (the worm is the final
    // power draw), pausing below the reserve so feeding can't blackout
    // the warren on its own.
    let mut worm_this_tick = false;
    if !session.worm_awake && session.buildings_of("worm_shrine").next().is_some() {
        let headroom = (session.economy.food - balance.worm_feed_reserve).max(0.0);
        let bite = (balance.worm_food_per_min / 60.0 * dt).min(headroom);
        if bite > 0.0 {
            session.economy.food -= bite;
            session.worm_fed += bite;
        }
        if session.worm_fed >= balance.worm_awaken_at {
            session.worm_awake = true;
            worm_this_tick = true;
        }
    }

    TickReport {
        deserters,
        won_this_tick,
        factory_this_tick,
        worm_this_tick,
        wild,
    }
}

/// Seconds of simulated time elapsed.
pub fn sim_seconds(session: &GameSession) -> f32 {
    session.tick as f32 * SIM_DT
}

/// Spend banked ore to attract a beetle hauler (the Phase 1 upgrade
/// decision). Returns false when the stockpile can't afford it.
pub fn try_attract_beetle(session: &mut GameSession, data: &GameData) -> bool {
    let cost = data.balance.beetle_ore_cost;
    if session.economy.ore_stock < cost {
        return false;
    }
    session.economy.ore_stock -= cost;
    session.spawn_creature(data, "beetle", crate::state::creatures::Job::Carrier);
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
    session.spawn_creature(data, "salamander", crate::state::creatures::Job::Smelter);
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
    session.spawn_creature(data, "hobgoblin", crate::state::creatures::Job::Idle);
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
    session.spawn_creature(data, "overseer", crate::state::creatures::Job::Idle);
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
        assert_eq!(a.economy.ore_delivered_total, b.economy.ore_delivered_total);
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
            session.economy.ore_delivered_total, session.economy.food
        );
        assert!(
            minutes <= 35.0,
            "win took {minutes:.1} min; the slice should fit one sitting"
        );
    }

    /// Buying the beetle trades banked ore for hauling capacity.
    #[test]
    fn beetle_purchase_spends_ore_and_spawns_hauler() {
        let (data, mut session) = boot(3);
        session.economy.ore_stock = data.balance.beetle_ore_cost + 5;

        let before = session.creatures.len();
        assert!(try_attract_beetle(&mut session, &data));
        assert_eq!(session.creatures.len(), before + 1);
        assert_eq!(session.economy.ore_stock, 5);
        assert!(session.creatures.iter().any(|c| c.species == "beetle"));

        // Too poor now.
        assert!(!try_attract_beetle(&mut session, &data));
    }

    /// Phase 6 exit gate: the prebuilt Mine is staffed by a goblin and its
    /// ore reaches the stockpile with zero player input.
    #[test]
    fn prebuilt_mine_feeds_the_stockpile_hands_off() {
        use crate::state::creatures::Task;
        let (data, mut session) = boot_on_config_seed();
        session.economy.food = 400.0; // keep everyone fed; isolate the ore loop

        run_until(
            &mut session,
            &data,
            4.0,
            |_, _| {},
            |s| s.economy.ore_delivered_total >= 5,
        );

        assert!(
            session.economy.ore_delivered_total >= 5,
            "ore should flow mine → carrier → stockpile with no input, got {}",
            session.economy.ore_delivered_total
        );
        assert!(
            session
                .creatures
                .iter()
                .any(|c| matches!(c.task, Task::WorkMine(_))),
            "a goblin should claim the mine post"
        );
    }

    /// Extraction fills the local buffer and draws down the finite deposit;
    /// an exhausted mine sheds its miner.
    #[test]
    fn mine_extracts_into_buffer_and_depletes_reserve() {
        use crate::state::creatures::{Good, Task};
        let (data, mut session) = boot_on_config_seed();
        session.economy.food = 500.0;
        session.creatures.clear(); // isolate a single miner + no carriers

        let mine_pos = session.buildings_of("mine").next().unwrap().pos;
        session.building_at_mut(mine_pos).unwrap().reserve = 5.0;
        session.spawn_creature(&data, "goblin", Job::Miner);

        run_until(
            &mut session,
            &data,
            3.0,
            |_, _| {},
            |s| s.building_at(mine_pos).unwrap().reserve <= 0.0,
        );

        let mine = session.building_at(mine_pos).unwrap();
        assert!(mine.reserve <= 0.0, "reserve should deplete");
        assert!(
            mine.stock(Good::Ore) >= 4.9,
            "extracted ore banks in the buffer (no carriers here), got {}",
            mine.stock(Good::Ore)
        );

        for _ in 0..5 {
            tick(&mut session, &data);
        }
        assert!(
            !session
                .creatures
                .iter()
                .any(|c| matches!(c.task, Task::WorkMine(_))),
            "a miner leaves an exhausted mine"
        );
    }

    /// The workstation's slot count caps how many miners staff one mine;
    /// surplus miners idle rather than pile on.
    #[test]
    fn mine_slots_cap_staffing() {
        use crate::state::creatures::Task;
        let (data, mut session) = boot_on_config_seed();
        session.economy.food = 800.0;
        session.creatures.clear();
        for _ in 0..5 {
            session.spawn_creature(&data, "goblin", Job::Miner);
        }

        run_until(
            &mut session,
            &data,
            2.0,
            |_, _| {},
            |s| {
                s.creatures
                    .iter()
                    .filter(|c| matches!(c.task, Task::WorkMine(_)))
                    .count()
                    >= 3
            },
        );

        let slots = data
            .buildings
            .get("mine")
            .unwrap()
            .workstation
            .as_ref()
            .unwrap()
            .slots as usize;
        let stationed = session
            .creatures
            .iter()
            .filter(|c| matches!(c.task, Task::WorkMine(_)))
            .count();
        assert_eq!(
            stationed, slots,
            "exactly {slots} miners staff the one mine, saw {stationed}"
        );
    }

    /// A Blacksmith hammers ore into ingots at the batch rate, banking the
    /// output in its buffer and the lifetime counter.
    #[test]
    fn blacksmith_forges_ingots_from_ore() {
        use crate::state::creatures::Good;
        use crate::state::structures::Building;
        let (data, mut session) = boot_on_config_seed();
        session.economy.food = 500.0;
        session.creatures.clear(); // isolate one smith, no carriers

        let spawn = session.spawn_tile();
        let spot = session
            .world
            .tiles
            .iter_with_pos()
            .filter(|(pos, _)| session.can_place_building(*pos))
            .map(|(pos, _)| pos)
            .min_by_key(|p| (p.manhattan_distance(&spawn), p.x, p.y))
            .unwrap();
        session.buildings.push(Building::new("blacksmith", spot));
        let batches = 3;
        let ore = data.balance.smith_batch_ore * batches;
        session
            .building_at_mut(spot)
            .unwrap()
            .add_stock(Good::Ore, ore as f32);
        session.spawn_creature(&data, "goblin", Job::Smith);

        run_until(
            &mut session,
            &data,
            3.0,
            |_, _| {},
            |s| s.economy.ingots_forged >= batches,
        );

        assert_eq!(
            session.economy.ingots_forged, batches,
            "{batches} batches of {} ore each → {batches} ingots",
            data.balance.smith_batch_ore
        );
        let shop = session.building_at(spot).unwrap();
        assert!(
            shop.stock(Good::Ingot) >= batches as f32 - 0.1,
            "forged ingots sit in the output buffer (no carriers here)"
        );
        assert!(shop.stock(Good::Ore) < 1.0, "all input ore was consumed");
    }

    /// Phase 7 exit gate: on a fresh warren, a Blacksmith + one Smith turns
    /// the Mine's ore into a banked ingot with only carriers in between.
    #[test]
    fn mine_blacksmith_ingot_chain_banks_an_ingot() {
        use crate::state::structures::Building;
        let (data, mut session) = boot_on_config_seed();
        session.economy.food = 400.0; // keep everyone fed; isolate the loop

        let spawn = session.spawn_tile();
        let spot = session
            .world
            .tiles
            .iter_with_pos()
            .filter(|(pos, _)| session.can_place_building(*pos))
            .map(|(pos, _)| pos)
            .min_by_key(|p| (p.manhattan_distance(&spawn), p.x, p.y))
            .unwrap();
        session.buildings.push(Building::new("blacksmith", spot));
        // Two player moves: put a goblin on the anvil and free a hauler.
        assert!(reassign(&mut session, &data, Job::Miner, Job::Smith));
        assert!(reassign(&mut session, &data, Job::Miner, Job::Carrier));

        run_until(
            &mut session,
            &data,
            8.0,
            |_, _| {},
            |s| s.economy.ingots_stock >= 1,
        );

        assert!(
            session.economy.ingots_stock >= 1,
            "an ingot should reach the stockpile via mine → carrier → blacksmith → smith → carrier, banked {}",
            session.economy.ingots_stock
        );
    }

    /// Modifier math: an Iron Pickaxe multiplies a miner's extraction rate.
    #[test]
    fn iron_pickaxe_speeds_mine_extraction() {
        use crate::state::creatures::Good;
        let (data, mut base) = boot_on_config_seed();
        base.economy.food = 500.0;
        base.creatures.clear();
        base.spawn_creature(&data, "goblin", Job::Miner);
        let mine = base.buildings_of("mine").next().unwrap().pos;

        let mut geared = base.clone();
        geared.creatures[0].equipment = Some("iron_pickaxe".to_owned());

        for _ in 0..600 {
            tick(&mut base, &data);
            tick(&mut geared, &data);
        }

        let plain = base.building_at(mine).unwrap().stock(Good::Ore);
        let boosted = geared.building_at(mine).unwrap().stock(Good::Ore);
        assert!(plain > 1.0, "the plain miner should extract something");
        assert!(
            boosted > plain * 1.3,
            "the pickaxe should visibly raise ore/min: {boosted} vs {plain}"
        );
    }

    /// Phase 8 go/no-go loop: queue a pickaxe at the Blacksmith, a Smith
    /// crafts it from banked ingots, and a miner picks it up on its own.
    #[test]
    fn blacksmith_order_crafts_gear_a_miner_equips() {
        use crate::state::creatures::Good;
        use crate::state::structures::Building;
        let (data, mut session) = boot_on_config_seed();
        session.economy.food = 500.0;

        let spawn = session.spawn_tile();
        let spot = session
            .world
            .tiles
            .iter_with_pos()
            .filter(|(pos, _)| session.can_place_building(*pos))
            .map(|(pos, _)| pos)
            .min_by_key(|p| (p.manhattan_distance(&spawn), p.x, p.y))
            .unwrap();
        let mut shop = Building::new("blacksmith", spot);
        shop.add_stock(
            Good::Ingot,
            data.equipment_def("iron_pickaxe").unwrap().cost_ingots as f32,
        );
        shop.orders.push("iron_pickaxe".to_owned());
        session.buildings.push(shop);
        assert!(reassign(&mut session, &data, Job::Miner, Job::Smith));

        run_until(
            &mut session,
            &data,
            6.0,
            |_, _| {},
            |s| {
                s.creatures
                    .iter()
                    .any(|c| c.equipment.as_deref() == Some("iron_pickaxe"))
            },
        );

        let equipped = session
            .creatures
            .iter()
            .filter(|c| c.equipment.as_deref() == Some("iron_pickaxe"))
            .count();
        assert_eq!(
            equipped, 1,
            "exactly one miner should wear the crafted pickaxe"
        );
        assert!(
            session
                .creatures
                .iter()
                .find(|c| c.equipment.as_deref() == Some("iron_pickaxe"))
                .map(|c| c.job == Job::Miner)
                .unwrap_or(false),
            "the pickaxe belongs to a miner"
        );
    }

    /// A reassigned goblin drops job-mismatched gear back to the pool.
    #[test]
    fn reassigned_worker_drops_mismatched_gear() {
        let (data, mut session) = boot_on_config_seed();
        session.economy.food = 500.0;
        session.creatures.clear();
        session.spawn_creature(&data, "goblin", Job::Miner);
        session.creatures[0].equipment = Some("iron_pickaxe".to_owned());

        assert!(reassign(&mut session, &data, Job::Miner, Job::Carrier));
        tick(&mut session, &data); // tick_gear drops the mismatched pickaxe

        assert!(session.creatures[0].equipment.is_none());
        assert_eq!(
            session
                .economy
                .gear_stock
                .get("iron_pickaxe")
                .copied()
                .unwrap_or(0),
            1,
            "the pickaxe returns to the stockpile pool"
        );
    }

    /// Equipment survives a save/load roundtrip (on creatures and in the
    /// stockpile pool).
    #[test]
    fn save_roundtrip_preserves_equipment() {
        let (_data, mut session) = boot_on_config_seed();
        session.creatures[0].equipment = Some("iron_pickaxe".to_owned());
        session
            .economy
            .gear_stock
            .insert("guard_blade".to_owned(), 2);

        let json = serde_json::to_string(&session).expect("serialize");
        let restored: GameSession = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(
            restored.creatures[0].equipment.as_deref(),
            Some("iron_pickaxe")
        );
        assert_eq!(
            restored.economy.gear_stock.get("guard_blade").copied(),
            Some(2)
        );
    }

    /// A Hobgoblin's ×2 work multiplier makes it out-mine a goblin.
    #[test]
    fn hobgoblin_out_mines_a_goblin() {
        use crate::state::creatures::Good;
        let (data, mut a) = boot_on_config_seed();
        a.economy.food = 500.0;
        a.creatures.clear();
        a.spawn_creature(&data, "goblin", Job::Miner);
        let mine = a.buildings_of("mine").next().unwrap().pos;

        let mut b = boot_on_config_seed().1;
        b.economy.food = 500.0;
        b.creatures.clear();
        b.spawn_creature(&data, "hobgoblin", Job::Miner);

        for _ in 0..600 {
            tick(&mut a, &data);
            tick(&mut b, &data);
        }
        let goblin_ore = a.building_at(mine).unwrap().stock(Good::Ore);
        let hob_ore = b.building_at(mine).unwrap().stock(Good::Ore);
        assert!(goblin_ore > 1.0);
        assert!(
            hob_ore > goblin_ore * 1.7,
            "hobgoblin (×2) should far out-mine a goblin: {hob_ore} vs {goblin_ore}"
        );
    }

    /// A Goblin Overseer's aura speeds nearby workers.
    #[test]
    fn overseer_aura_speeds_nearby_workers() {
        use crate::state::creatures::Good;
        let (data, mut a) = boot_on_config_seed();
        a.economy.food = 800.0;
        a.creatures.clear();
        a.spawn_creature(&data, "goblin", Job::Miner);
        let mine = a.buildings_of("mine").next().unwrap().pos;

        let mut b = a.clone();
        // An overseer stands at the warren centre, its aura over the mine.
        b.spawn_creature(&data, "overseer", Job::Idle);

        for _ in 0..600 {
            tick(&mut a, &data);
            tick(&mut b, &data);
        }
        let plain = a.building_at(mine).unwrap().stock(Good::Ore);
        let auraed = b.building_at(mine).unwrap().stock(Good::Ore);
        assert!(
            auraed > plain * 1.2,
            "the aura should visibly speed the miner: {auraed} vs {plain}"
        );
    }

    /// Ledger: a Hobgoblin eats ~2.5× a goblin (specialist vs generalist).
    #[test]
    fn hobgoblin_upkeep_is_heavier() {
        let (data, _) = boot_on_config_seed();
        let gob = data.species.get("goblin").unwrap().food_per_min;
        let hob = data.species.get("hobgoblin").unwrap().food_per_min;
        assert!(
            (hob / gob - 2.5).abs() < 0.01,
            "hobgoblin should draw 2.5× a goblin, got {}×",
            hob / gob
        );
    }

    /// Breeding is gated on the ingot unlock, a breeding pit, and banked
    /// ingots; only one Overseer at a time.
    #[test]
    fn breeding_is_gated_and_capped() {
        use crate::state::structures::Building;
        let (data, mut session) = boot_on_config_seed();
        session.economy.ingots_stock = 20;

        // No unlock yet.
        assert!(!try_breed_hobgoblin(&mut session, &data));
        session.unlocked.insert("hobgoblin".to_owned());
        // Unlocked, but no breeding pit.
        assert!(!try_breed_hobgoblin(&mut session, &data));

        let spot = session
            .world
            .tiles
            .iter_with_pos()
            .find(|(pos, _)| session.can_place_building(*pos))
            .map(|(pos, _)| pos)
            .unwrap();
        session.buildings.push(Building::new("breeding_pit", spot));
        assert!(try_breed_hobgoblin(&mut session, &data));
        assert!(session.creatures.iter().any(|c| c.species == "hobgoblin"));
        assert_eq!(
            session.economy.ingots_stock,
            20 - data.balance.hobgoblin_ingot_cost
        );

        // One overseer per district.
        session.unlocked.insert("overseer".to_owned());
        assert!(try_breed_overseer(&mut session, &data));
        assert!(!try_breed_overseer(&mut session, &data));
    }

    /// Designated rock gets carved into floor by miners.
    #[test]
    fn dig_designations_get_carved_by_miners() {
        let (data, mut session) = boot_on_config_seed();

        // Mark a rock tile adjacent to reachable floor near spawn.
        let spawn = session.spawn_tile();
        let mark = session
            .world
            .tiles
            .iter_with_pos()
            .filter(|(pos, t)| {
                **t == crate::state::world::Tile::Rock
                    && pos
                        .neighbors_4way()
                        .iter()
                        .any(|n| session.world.tiles.get(*n).is_some_and(|t| t.walkable()))
            })
            .map(|(pos, _)| pos)
            .min_by_key(|p| (p.manhattan_distance(&spawn), p.x, p.y))
            .unwrap();
        assert!(session.toggle_dig_mark(mark));

        run_until(
            &mut session,
            &data,
            3.0,
            |_, _| {},
            |s| s.dig_marks.is_empty(),
        );

        assert!(session.dig_marks.is_empty(), "mark should be dug");
        assert!(session.world.tiles.get(mark).unwrap().walkable());
    }

    /// A placed ghost gets its ore hauled by carriers and becomes a
    /// working building (a second farm that then grows mushrooms).
    #[test]
    fn build_site_completes_from_hauled_ore() {
        let (data, mut session) = boot_on_config_seed();
        session.economy.ore_stock = 40;
        session.economy.food = 200.0; // keep everyone fed for the test

        let spawn = session.spawn_tile();
        let spot = session
            .world
            .tiles
            .iter_with_pos()
            .filter(|(pos, t)| {
                **t == crate::state::world::Tile::Floor
                    && session.can_place_building(*pos)
                    && pos.manhattan_distance(&spawn) >= 2
            })
            .map(|(pos, _)| pos)
            .min_by_key(|p| (p.manhattan_distance(&spawn), p.x, p.y))
            .unwrap();
        assert!(try_place_build_site(&mut session, &data, "farm", spot));
        assert!(!try_place_build_site(&mut session, &data, "farm", spot));

        run_until(
            &mut session,
            &data,
            6.0,
            |_, _| {},
            |s| s.build_sites.is_empty(),
        );

        assert!(session.build_sites.is_empty(), "site should complete");
        let new_farm = session.building_at(spot).expect("farm built");
        assert_eq!(new_farm.kind, "farm");
        assert_eq!(session.buildings_of("farm").count(), 2);
    }

    /// The charcoal chain end-to-end: kiln converts wood, salamander
    /// claims ore + charcoal and forges ingots (and eats the charcoal).
    #[test]
    fn kiln_and_salamander_forge_ingots() {
        use crate::state::creatures::Good;
        let (data, mut session) = boot_on_config_seed();
        session.economy.food = 500.0; // not under test here

        // Drop a kiln and smelter directly next to spawn.
        let spawn = session.spawn_tile();
        let mut spots = session
            .world
            .tiles
            .iter_with_pos()
            .filter(|(pos, _)| session.can_place_building(*pos))
            .map(|(pos, _)| pos)
            .collect::<Vec<_>>();
        spots.sort_by_key(|p| (p.manhattan_distance(&spawn), p.x, p.y));
        let kiln_pos = spots[0];
        let den_pos = spots[1];
        session
            .buildings
            .push(crate::state::structures::Building::new("kiln", kiln_pos));
        session
            .buildings
            .push(crate::state::structures::Building::new("smelter", den_pos));
        session
            .building_at_mut(kiln_pos)
            .unwrap()
            .add_stock(Good::Wood, 6.0);
        session
            .building_at_mut(den_pos)
            .unwrap()
            .add_stock(Good::Ore, 6.0);
        session.spawn_creature(&data, "salamander", Job::Smelter);

        run_until(
            &mut session,
            &data,
            8.0,
            |_, _| {},
            |s| s.economy.ingots_forged >= 2,
        );

        assert!(
            session.economy.ingots_forged >= 2,
            "salamander should forge ingots"
        );
        let salamander = session
            .creatures
            .iter()
            .find(|c| c.species == "salamander")
            .expect("salamander survives");
        assert!(salamander.satiation > 0.5, "smelting feeds the salamander");
    }

    /// The whole campaign on the fixed seed: famine → recover → first
    /// victory → the Blacksmith forges the factory goal → the Colossal Worm.
    /// The "one sitting" length probe, and the contract for the arc (plan
    /// §Phase 11): famine ~5 min, win 1 ~12–15, win 2 ~22–26, worm ~45–50.
    ///
    /// A competent campaign leans on the automation loop: beetle haulers for
    /// capacity, a Blacksmith hammering ore into ingots, expansion (a second
    /// Mine) and food scaling (more farms/cooks) to feed the growing warren
    /// and the worm's appetite.
    #[test]
    fn sim_to_factory_complete_on_fixed_seed() {
        use crate::state::creatures::Good;
        let (data, mut session) = boot_on_config_seed();
        let mut reacted = false;
        let mut farms = 1;
        let mut beetled = false;
        let mut smith_placed = false;
        let mut smith = false;
        let mut geared = false;
        let mut shrined = false;

        // Place `kind` on the nearest buildable floor to spawn.
        fn build_near(s: &mut GameSession, data: &GameData, kind: &str) -> bool {
            let spawn = s.spawn_tile();
            let spot = s
                .world
                .tiles
                .iter_with_pos()
                .filter(|(pos, _)| s.can_place_building(*pos))
                .map(|(pos, _)| pos)
                .min_by_key(|p| (p.manhattan_distance(&spawn), p.x, p.y));
            spot.map(|spot| try_place_build_site(s, data, kind, spot))
                .unwrap_or(false)
        }

        let done_at = run_until(
            &mut session,
            &data,
            80.0,
            |s, t| {
                if (t as u64).is_multiple_of(300) && t.fract() < 0.05 {
                    let bs: Vec<(f32, f32)> = s
                        .buildings_of("blacksmith")
                        .map(|b| (b.stock(Good::Ore), b.stock(Good::Ingot)))
                        .collect();
                    eprintln!(
                        "[t={:.0}m] food={:.0} ore={} ingots={}/{} won={} fac={} worm={} M{} C{} S{} pop={} bs={:?}",
                        t / 60.0, s.economy.food, s.economy.ore_stock, s.economy.ingots_forged,
                        data.balance.win2_ingots, s.won, s.factory_complete, s.worm_awake,
                        s.job_count(Job::Miner), s.job_count(Job::Carrier), s.job_count(Job::Smith),
                        s.creatures.len(), bs
                    );
                }
                // 1. Famine: shift the surplus miners onto hauling. The
                //    lean steady-state warren is 1 miner + 3 carriers + cook.
                if !reacted && s.economy.food < 15.0 {
                    let _ = reassign(s, &data, Job::Miner, Job::Carrier);
                    let _ = reassign(s, &data, Job::Miner, Job::Carrier);
                    reacted = true;
                }
                // 2. Scale food: a second farm (then a third for the worm's
                //    appetite) lifts the kitchen above break-even so the
                //    larder builds a surplus that frees carriers for industry.
                let want_farms = if s.factory_complete { 3 } else { 2 };
                if s.won
                    && farms < want_farms
                    && s.economy.ore_stock >= 10
                    && build_near(s, &data, "farm")
                {
                    farms += 1;
                }
                // 3. A single beetle hauler (5× a goblin's load) once the
                //    first win banks ore — the hauling backbone that lets the
                //    warren feed itself *and* supply the Blacksmith.
                if s.won
                    && farms >= 2
                    && !beetled
                    && s.economy.ore_stock >= data.balance.beetle_ore_cost
                {
                    beetled = try_attract_beetle(s, &data);
                }
                // 4. After win 1, place a Blacksmith (once) and, when it's
                //    built, put a goblin on the anvil to hammer ore into the
                //    20 ingots of win 2.
                if s.won && beetled && !smith_placed && s.economy.ore_stock >= 8 {
                    smith_placed = build_near(s, &data, "blacksmith");
                }
                if smith_placed
                    && !smith
                    && s.buildings_of("blacksmith").next().is_some()
                    && reassign(s, &data, Job::Carrier, Job::Smith)
                {
                    smith = true;
                }
                // 3. Craft equipment — a pickaxe (miner ×1.5) and a hauling
                //    frame (carrier +1) — the cheap, upkeep-free way to lift
                //    throughput for the endgame push (the feedback loop).
                if smith && !geared {
                    let shop = s.buildings_of("blacksmith").next().map(|b| b.pos);
                    if let Some(shop) = shop {
                        let b = s.building_at_mut(shop).unwrap();
                        b.orders.push("iron_pickaxe".to_owned());
                        b.orders.push("hauling_frame".to_owned());
                        geared = true;
                    }
                }
                // 4. The endgame monument once the factory goal completes.
                if s.factory_complete
                    && !shrined
                    && s.unlocked.contains("worm_shrine")
                    && s.economy.ore_stock >= 20
                    && build_near(s, &data, "worm_shrine")
                {
                    shrined = true;
                }
            },
            |s| s.worm_awake,
        );

        assert!(session.won, "first victory should land on the way");
        assert!(
            session.factory_complete,
            "the Blacksmith should forge the {} ingots of win 2",
            data.balance.win2_ingots
        );
        assert!(
            session.worm_awake,
            "expected the Colossal Worm within 80 sim-minutes"
        );
        let minutes = done_at / 60.0;
        eprintln!(
            "[balance probe] worm awakened at {minutes:.1} sim-min ({} ingots, {} deserted, {} raids survived)",
            session.economy.ingots_forged, session.economy.deserted, session.progress.raids_survived
        );
        assert!(
            (30.0..=60.0).contains(&minutes),
            "campaign took {minutes:.1} min; want a ~45-min sitting"
        );
        assert!(
            session.economy.deserted <= 2,
            "the campaign should cost at most two workers, lost {}",
            session.economy.deserted
        );
    }

    /// Worm offerings drain the larder but never below the reserve.
    #[test]
    fn worm_feeding_respects_the_reserve() {
        use crate::state::structures::Building;
        let (data, mut session) = boot(11);
        let spot = session
            .world
            .tiles
            .iter_with_pos()
            .find(|(pos, _)| session.can_place_building(*pos))
            .map(|(pos, _)| pos)
            .unwrap();
        session.buildings.push(Building::new("worm_shrine", spot));
        session.creatures.clear(); // isolate the worm's draw
        session.economy.food = data.balance.worm_feed_reserve + 3.0;

        for _ in 0..1200 {
            tick(&mut session, &data);
        }

        assert!(session.worm_fed > 0.0, "offerings should accumulate");
        assert!(
            session.economy.food >= data.balance.worm_feed_reserve - 1e-3,
            "feeding must pause at the reserve, food = {}",
            session.economy.food
        );
        assert!(!session.worm_awake);
    }

    /// Capture → study → adapt: snared wild beetles advance the counter,
    /// unlock the breeding pit, and the pit hatches new haulers.
    #[test]
    fn traps_capture_and_breeding_pit_hatches() {
        use crate::state::structures::Building;
        use crate::state::wildlife::{WildBehavior, WildCreature};
        let (data, mut session) = boot_on_config_seed();
        session.economy.food = 400.0;

        // Two traps near spawn, and wild beetles sitting on them (each
        // trap is single-use).
        let spawn = session.spawn_tile();
        let mut spots: Vec<_> = session
            .world
            .tiles
            .iter_with_pos()
            .filter(|(pos, _)| session.can_place_building(*pos))
            .map(|(pos, _)| pos)
            .collect();
        spots.sort_by_key(|p| (p.manhattan_distance(&spawn), p.x, p.y));
        for (i, spot) in spots.iter().take(2).enumerate() {
            session.buildings.push(Building::new("trap", *spot));
            session.wilds.push(WildCreature::new(
                900 + i as u32,
                "wild_beetle",
                *spot,
                30.0,
                WildBehavior::Wander { next_move_in: 5.0 },
            ));
        }

        run_until(
            &mut session,
            &data,
            1.0,
            |_, _| {},
            |s| s.progress.beetles_captured >= 2,
        );

        assert_eq!(session.progress.beetles_captured, 2);
        assert!(
            session.buildings_of("trap").next().is_none(),
            "traps are single-use"
        );
        assert!(
            session.unlocked.contains("breeding_pit"),
            "capturing 2 beetles should unlock the breeding pit"
        );

        // Breeding: pit + 2 specimens hatch a beetle when the timer laps.
        let pit_spot = session
            .world
            .tiles
            .iter_with_pos()
            .filter(|(pos, _)| session.can_place_building(*pos))
            .map(|(pos, _)| pos)
            .min_by_key(|p| (p.manhattan_distance(&spawn), p.x, p.y))
            .unwrap();
        session
            .buildings
            .push(Building::new("breeding_pit", pit_spot));
        let beetles_before = session
            .creatures
            .iter()
            .filter(|c| c.species == "beetle")
            .count();
        session.breed_in = 1.0;
        run_until(
            &mut session,
            &data,
            1.0,
            |_, _| {},
            |s| s.creatures.iter().filter(|c| c.species == "beetle").count() > beetles_before,
        );
        assert!(
            session
                .creatures
                .iter()
                .filter(|c| c.species == "beetle")
                .count()
                > beetles_before
        );
    }

    /// Guards kill a staged raid; surviving it unlocks hardened guards.
    /// Without guards, raiders eat the stockpile instead.
    #[test]
    fn guards_repel_raids_and_undefended_raids_drain_food() {
        // Defended warren.
        let (data, mut session) = boot_on_config_seed();
        session.economy.food = 300.0;
        for _ in 0..2 {
            assert!(reassign(&mut session, &data, Job::Miner, Job::Guard));
        }
        session.raid_in = 1.0;
        run_until(
            &mut session,
            &data,
            8.0,
            |_, _| {},
            |s| s.progress.raids_survived >= 1,
        );
        assert_eq!(session.progress.raids_survived, 1);
        assert!(session.unlocked.contains("hardened_guards"));
        assert!(!session.raid_active);

        // Undefended warren: raiders feast, then leave on their own.
        let (data2, mut open_house) = boot_on_config_seed();
        open_house.economy.food = 300.0;
        open_house.raid_in = 1.0;
        let food_before_raid = open_house.economy.food;
        run_until(
            &mut open_house,
            &data2,
            10.0,
            |_, _| {},
            |s| s.progress.raids_survived >= 1,
        );
        assert!(
            open_house.economy.food
                < food_before_raid - data2.balance.raider_flee_after_eaten * 0.9,
            "an undefended raid should eat a meaningful chunk of the larder"
        );
    }

    /// Surviving a famine grants Preservation Techniques (bigger farms).
    #[test]
    fn famine_survival_unlocks_preservation() {
        let (data, mut session) = boot_on_config_seed();
        session.economy.food = 0.0;
        assert!((wildlife::farm_cap(&session, &data) - data.balance.farm_storage_cap).abs() < 1e-4);

        run_until(&mut session, &data, 1.0, |_, _| {}, |s| s.famine_active);
        session.economy.food = data.balance.famine_recover_food + 5.0;
        run_until(
            &mut session,
            &data,
            0.5,
            |_, _| {},
            |s| s.progress.famines_survived >= 1,
        );

        assert_eq!(session.progress.famines_survived, 1);
        assert!(session.unlocked.contains("preservation"));
        assert!(wildlife::farm_cap(&session, &data) > data.balance.farm_storage_cap);
    }

    /// Full-session serde roundtrip: a loaded save simulates identically
    /// to the original.
    #[test]
    fn save_roundtrip_preserves_simulation() {
        let (data, mut original) = boot_on_config_seed();
        // Make the state interesting first.
        for _ in 0..1200 {
            tick(&mut original, &data);
        }

        let json = serde_json::to_string(&original).expect("serialize");
        let mut restored: GameSession = serde_json::from_str(&json).expect("deserialize");

        for _ in 0..1200 {
            tick(&mut original, &data);
            tick(&mut restored, &data);
        }

        assert_eq!(
            original.economy.ore_delivered_total,
            restored.economy.ore_delivered_total
        );
        assert!((original.economy.food - restored.economy.food).abs() < 1e-3);
        assert_eq!(original.creatures.len(), restored.creatures.len());
        for (a, b) in original.creatures.iter().zip(&restored.creatures) {
            assert_eq!(a.task, b.task);
            assert!((a.x - b.x).abs() < 1e-4);
            assert!((a.y - b.y).abs() < 1e-4);
        }
    }
}
