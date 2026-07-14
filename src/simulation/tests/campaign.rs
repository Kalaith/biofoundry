//! The whole arc on the fixed seed, played by a scripted competent player.
//! This is the "one sitting" length probe.

use super::*;

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
            // 5. Craft equipment — a pickaxe (miner ×1.5) and a hauling
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
            // 6. The endgame monument once the factory goal completes.
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

/// Place `kind` on the nearest buildable floor to spawn.
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
