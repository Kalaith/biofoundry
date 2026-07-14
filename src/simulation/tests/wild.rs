//! The world pushing back: trapping wild stock to unlock breeding, and
//! raids with and without guards to meet them.

use super::*;

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
        open_house.economy.food < food_before_raid - data2.balance.raider_flee_after_eaten * 0.9,
        "an undefended raid should eat a meaningful chunk of the larder"
    );
}
