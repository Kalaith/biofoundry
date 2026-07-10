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
    // food (cook batches), so the delta is this tick's production.
    let food_before = session.economy.food;
    jobs::tick_creatures(session, data, dt);
    let produced_per_min = ((session.economy.food - food_before) / dt * 60.0).max(0.0);
    let smoothing = dt / 15.0; // ~15s time constant
    session.economy.production_ema_per_min +=
        (produced_per_min - session.economy.production_ema_per_min) * smoothing;
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
    if !session.factory_complete && session.economy.metal >= balance.win2_metal {
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
    if !def.buildable || !session.building_unlocked(def) || !session.can_place_building(pos) {
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
    /// claims ore + charcoal and forges metal (and eats the charcoal).
    #[test]
    fn kiln_and_salamander_forge_metal() {
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
            |s| s.economy.metal >= 2,
        );

        assert!(session.economy.metal >= 2, "salamander should forge metal");
        let salamander = session
            .creatures
            .iter()
            .find(|c| c.species == "salamander")
            .expect("salamander survives");
        assert!(salamander.satiation > 0.5, "smelting feeds the salamander");
    }

    /// The whole prototype on one fixed seed: famine → recover → first
    /// victory → build the charcoal chain → salamander forges the factory
    /// goal. This is the "one sitting" length probe.
    #[test]
    fn sim_to_factory_complete_on_fixed_seed() {
        let (data, mut session) = boot_on_config_seed();
        let mut reacted = false;
        let mut farm2 = false;
        let mut placed = false;
        let mut attracted = false;
        let mut shrined = false;

        let mut guarded = 0;
        let done_at = run_until(
            &mut session,
            &data,
            70.0,
            |s, t| {
                if (t as u64).is_multiple_of(300) && t.fract() < 0.05 {
                    use crate::state::creatures::Good;
                    let kiln: Vec<(f32, f32)> = s
                        .buildings_of("kiln")
                        .map(|b| (b.stock(Good::Wood), b.stock(Good::Charcoal)))
                        .collect();
                    let den: Vec<(f32, f32)> = s
                        .buildings_of("smelter")
                        .map(|b| (b.stock(Good::Ore), b.stock(Good::Charcoal)))
                        .collect();
                    eprintln!(
                        "[t={:.0}m] food={:.0} ore_bank={} metal={} won={} sites={} kiln={:?} den={:?} pop={} deserted={}",
                        t / 60.0, s.economy.food, s.economy.ore_stock, s.economy.metal,
                        s.won, s.build_sites.len(), kiln, den, s.creatures.len(), s.economy.deserted
                    );
                }
                if !reacted && s.economy.food < 15.0 {
                    let _ = reassign(s, &data, Job::Miner, Job::Carrier);
                    let _ = reassign(s, &data, Job::Miner, Job::Carrier);
                    reacted = true;
                }
                // Post a guard shortly before the first raid lands. The
                // remaining 1 miner + 2 carriers + cook + guard is the
                // steady-state warren.
                if guarded == 0
                    && t > data.balance.raid_first_sec - 60.0
                    && (reassign(s, &data, Job::Carrier, Job::Guard)
                        || reassign(s, &data, Job::Miner, Job::Guard))
                {
                    guarded = 1;
                }
                // A second farm near the warren shortens haul trips — the
                // "build more generators" move once ore allows it.
                if !farm2 && guarded == 1 && s.economy.ore_stock >= 12 {
                    let spawn = s.spawn_tile();
                    let spot = s
                        .world
                        .tiles
                        .iter_with_pos()
                        .filter(|(pos, _)| s.can_place_building(*pos))
                        .map(|(pos, _)| pos)
                        .min_by_key(|p| (p.manhattan_distance(&spawn), p.x, p.y));
                    if let Some(spot) = spot {
                        farm2 = try_place_build_site(s, &data, "farm", spot);
                    }
                }
                // After the first win, invest in the smelting chain.
                if s.won && !placed && s.economy.ore_stock >= 27 {
                    let spawn = s.spawn_tile();
                    let mut spots = s
                        .world
                        .tiles
                        .iter_with_pos()
                        .filter(|(pos, _)| s.can_place_building(*pos))
                        .map(|(pos, _)| pos)
                        .collect::<Vec<_>>();
                    spots.sort_by_key(|p| (p.manhattan_distance(&spawn), p.x, p.y));
                    assert!(try_place_build_site(s, &data, "kiln", spots[0]));
                    assert!(try_place_build_site(s, &data, "smelter", spots[1]));
                    placed = true;
                }
                if placed
                    && !attracted
                    && s.buildings_of("smelter").next().is_some()
                    && s.economy.ore_stock >= data.balance.salamander_ore_cost
                {
                    attracted = try_attract_salamander(s, &data);
                }
                // The endgame monument: shrine up, offerings flow, and the
                // food grid scales up to feed the worm (beetle + 3rd farm).
                if s.factory_complete
                    && !shrined
                    && s.unlocked.contains("worm_shrine")
                    && s.economy.ore_stock >= 20
                {
                    let spawn = s.spawn_tile();
                    let spot = s
                        .world
                        .tiles
                        .iter_with_pos()
                        .filter(|(pos, _)| s.can_place_building(*pos))
                        .map(|(pos, _)| pos)
                        .min_by_key(|p| (p.manhattan_distance(&spawn), p.x, p.y));
                    if let Some(spot) = spot {
                        shrined = try_place_build_site(s, &data, "worm_shrine", spot);
                    }
                }
                // Scale the food grid for the worm's appetite: a beetle
                // hauler and a third farm as soon as the factory pays out.
                if s.factory_complete
                    && !s.creatures.iter().any(|c| c.species == "beetle")
                    && s.economy.ore_stock >= data.balance.beetle_ore_cost + 10
                {
                    let _ = try_attract_beetle(s, &data);
                    let spawn = s.spawn_tile();
                    let spot = s
                        .world
                        .tiles
                        .iter_with_pos()
                        .filter(|(pos, _)| s.can_place_building(*pos))
                        .map(|(pos, _)| pos)
                        .min_by_key(|p| (p.manhattan_distance(&spawn), p.x, p.y));
                    if let Some(spot) = spot {
                        let _ = try_place_build_site(s, &data, "farm", spot);
                    }
                }
            },
            |s| s.worm_awake,
        );

        assert!(session.won, "first victory should land on the way");
        assert!(attracted, "the salamander never arrived");
        assert!(session.factory_complete, "factory goal should complete");
        assert!(
            session.worm_awake,
            "expected the Colossal Worm within 70 sim-minutes"
        );
        let minutes = done_at / 60.0;
        eprintln!(
            "[balance probe] worm awakened at {minutes:.1} sim-min ({} metal, {} deserted, {} raids survived)",
            session.economy.metal, session.economy.deserted, session.progress.raids_survived
        );
        assert!(
            (25.0..=65.0).contains(&minutes),
            "campaign took {minutes:.1} min; want a 30-50 minute sitting"
        );
        assert!(
            session.economy.deserted <= 1,
            "the worm's appetite should cost at most one worker, lost {}",
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
