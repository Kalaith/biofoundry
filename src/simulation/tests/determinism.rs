//! The sim is a pure function of (seed, inputs): same seed → same run, and
//! a save/load roundtrip must not perturb it.

use super::*;

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
