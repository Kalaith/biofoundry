//! The food grid: the scripted famine, the recovery that follows, and the
//! other draws on the larder (the charcoal chain, the worm).

use super::*;

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
