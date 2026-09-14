use super::*;

fn field() -> (Game, AdvancedAi, Pos) {
    let mut g = Game::new(2, 24, 16, 914_356_600, 250, 0);
    g.units.clear();
    g.cities.clear();
    g.at_war.insert((0, 1));
    g.turn = 20;
    let center = (10, 8);
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    let mut ai = AdvancedAi::new();
    ai.enable_hostile_memory_3();
    ai.hostile_last_seen.insert(
        999_999,
        RememberedHostile {
            pos: center,
            when: g.turn - 1,
            owner: 1,
            kind: crate::name!("warrior"),
        },
    );
    (g, ai, center)
}

#[test]
fn hostile_memory_v3_releases_cleared_ground_but_v2_keeps_the_old_threat() {
    let (mut g, mut ai, center) = field();
    g.host_observed = Arc::new(g.wdisk(center, 3).into_iter().collect());
    let mut control = ai.clone();
    control.enable_hostile_memory_2();
    control.observe_turn_start_hostiles(&g, 0);
    ai.observe_turn_start_hostiles(&g, 0);
    assert!(control
        .barbarian_reach(&g, 0, center, 10)
        .covers(&g, center));
    assert!(!ai.barbarian_reach(&g, 0, center, 10).covers(&g, center));
    assert!(ai.hostile_last_seen.is_empty());
}

#[test]
fn hostile_memory_v3_keeps_a_sighting_when_any_forecast_tile_is_still_fogged() {
    let (mut g, mut ai, center) = field();
    let mut visible: BTreeSet<Pos> = g.wdisk(center, 3).into_iter().collect();
    let fog = g
        .wdisk(center, 3)
        .into_iter()
        .find(|pos| g.wdist(center, *pos) == 3)
        .unwrap();
    visible.remove(&fog);
    g.host_observed = Arc::new(visible);
    ai.observe_turn_start_hostiles(&g, 0);
    assert!(ai.hostile_last_seen.contains_key(&999_999));
    assert!(ai.barbarian_reach(&g, 0, center, 10).covers(&g, center));
}

#[test]
fn hostile_memory_v3_does_not_clear_a_current_turn_sighting_after_a_planning_kill() {
    let (mut g, mut ai, center) = field();
    g.host_observed = Arc::new(g.wdisk(center, 3).into_iter().collect());
    ai.hostile_last_seen.get_mut(&999_999).unwrap().when = g.turn;
    ai.observe_turn_start_hostiles(&g, 0);
    assert!(ai.barbarian_reach(&g, 0, center, 10).covers(&g, center));
}

#[test]
fn hostile_memory_v3_preserves_fresh_observations_and_possible_camouflage() {
    for kind in ["scout", "privateer", "warrior"] {
        let (mut g, mut ai, center) = field();
        g.host_observed = Arc::new(g.map.tiles.keys().copied().collect());
        ai.hostile_last_seen.get_mut(&999_999).unwrap().kind = kind.into();
        if kind == "warrior" {
            let uid = g.spawn_test_unit(kind, 1, center);
            Arc::make_mut(&mut g.host_unit_facts)
                .entry(uid)
                .or_default()
                .civ6_id = Some(999_999);
        }
        ai.observe_turn_start_hostiles(&g, 0);
        assert!(ai.hostile_last_seen.contains_key(&999_999), "{kind}");
    }
}

#[test]
fn hostile_memory_v3_forecast_is_identical_with_a_hidden_native_unit_or_an_absent_export() {
    let (mut g, mut ai, center) = field();
    g.host_observed = Arc::new([center].into_iter().collect());
    let hidden = (20, 12);
    let uid = g.spawn_test_unit("modern_armor", 1, hidden);
    Arc::make_mut(&mut g.host_unit_facts)
        .entry(uid)
        .or_default()
        .civ6_id = Some(999_999);
    let record = ai.hostile_last_seen[&999_999].clone();
    ai.observe_turn_start_hostiles(&g, 0);
    assert_eq!(ai.hostile_last_seen[&999_999], record);
    let native = ai.barbarian_reach(&g, 0, center, 10);
    let mut exported = g.clone();
    exported.remove_unit(uid);
    let mirror = ai.barbarian_reach(&exported, 0, center, 10);
    for pos in g.wdisk(center, 8) {
        assert_eq!(
            native.covers(&g, pos),
            mirror.covers(&exported, pos),
            "{pos:?}"
        );
    }
    let edge = g
        .wdisk(center, 4)
        .into_iter()
        .find(|pos| g.wdist(center, *pos) == 4)
        .unwrap();
    assert!(
        !native.covers(&g, edge),
        "the hidden upgrade cannot expand the observed Warrior's reach"
    );
}

#[test]
fn hostile_memory_v3_is_an_exclusive_opt_in() {
    test_support::opt_in_off_in_both_controllers("hostile-memory-3", |ai| ai.hostile_memory_3);
    let mut ai = AdvancedAi::new();
    for enable in [
        AdvancedAi::enable_hostile_memory,
        AdvancedAi::enable_hostile_memory_2,
    ] {
        enable(&mut ai);
        ai.enable_hostile_memory_3();
        assert!(!ai.hostile_memory && !ai.hostile_memory_2 && ai.hostile_memory_3);
        enable(&mut ai);
        assert!(!ai.hostile_memory_3);
    }
}
