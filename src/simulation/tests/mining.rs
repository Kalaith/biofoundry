//! The ore loop and the ways the player extends it: staffing a Mine,
//! carving rock, and hauling ore into new buildings.

use super::*;

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
