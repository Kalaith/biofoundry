//! The ingot chain and the feedback loop it powers: the Blacksmith, the
//! gear it crafts, and the bred workers those ingots buy.

use super::*;

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
