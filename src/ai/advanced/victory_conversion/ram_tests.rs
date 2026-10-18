//! `siege-budget-counts-the-ram`: the siege damage budget prices a melee
//! member's swing beside the train's Battering Ram or Siege Tower the way
//! the host resolves it.

use super::*;

/// A city behind Ancient Walls (100/100) at war with player 0, and a train
/// of `melee` Spearmen plus a `support` unit two tiles out, none of them
/// beside the city yet.
fn walled_train(support: &str, melee: usize) -> (Game, AdvancedAi, u32, Vec<u32>) {
    let mut g = Game::new_full(2, 24, 16, 91_119, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
    }
    g.found_city_for(0, (4, 4), None);
    let enemy = g.found_city_for(1, (16, 8), None);
    g.record_contact(0, 1);
    g.current = 0;
    g.turn = 60;
    g.at_war.insert((0, 1));
    g.cities
        .get_mut(&enemy)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    let full_wall = g.city_max_wall_hp(&g.cities[&enemy]);
    assert_eq!(full_wall, 100, "Ancient Walls");
    g.cities.get_mut(&enemy).unwrap().wall_hp = full_wall;
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(enemy, full_wall);
    let city = g.cities[&enemy].pos;
    let mut force = Vec::new();
    for dy in 0..melee as i32 {
        force.push(g.spawn_test_unit("spearman", 0, (city.0 - 2, city.1 + dy - 1)));
    }
    force.push(g.spawn_test_unit(support, 0, (city.0 - 2, city.1 + melee as i32)));
    for uid in &force {
        assert!(
            g.wdist(g.units[uid].pos, city) >= 2,
            "nobody stands beside the city"
        );
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_siege_positive_damage_budget();
    ai.enable_siege_budget_counts_what_fires();
    ai.enable_unwalled_city_takes_the_swarm();
    (g, ai, enemy, force)
}

#[test]
fn a_train_held_ram_turns_an_infinite_wall_budget_finite() {
    let (g, mut ai, city, force) = walled_train("battering_ram", 4);
    assert!(g.city_allows_siege_support(city, "battering_ram"));
    let (off, _) = ai
        .conversion_siege_entry_budget(&g, 0, city, &force, 5)
        .unwrap();
    assert!(
        off.is_infinite(),
        "without the gene the melee price no wall damage: {off}"
    );

    ai.enable_siege_budget_counts_the_ram();
    let (on, endurance) = ai
        .conversion_siege_entry_budget(&g, 0, city, &force, 5)
        .unwrap();
    assert!(
        on.is_finite(),
        "four Spearmen beside a Ram open Ancient Walls: {on}"
    );
    assert!(endurance > 0.0);
    // The walls fall to the Ram-supported swings at whole damage, then the
    // fit Spearmen swarm the open city.
    let swing = expected_damage(
        g.unit_strength(&g.units[&force[0]], true),
        g.city_strength(city),
    );
    let walls = 100.0 / (4.0 * swing);
    let open_city = 200.0 / (4.0 * swing - 20.0);
    assert!(
        (on - (walls + open_city + 1.0)).abs() < 1e-9,
        "{on} against {walls} + {open_city} + 1"
    );
}

#[test]
fn a_ram_prices_the_reply_its_swingers_take() {
    let (g, mut ai, city, force) = walled_train("battering_ram", 4);
    let (_, endurance_off) = ai
        .conversion_siege_entry_budget(&g, 0, city, &force, 5)
        .unwrap();
    ai.enable_siege_budget_counts_the_ram();
    let (_, endurance_on) = ai
        .conversion_siege_entry_budget(&g, 0, city, &force, 5)
        .unwrap();
    assert!(
        endurance_on < endurance_off,
        "a Spearman swinging every turn takes the city's reply: {endurance_on} vs {endurance_off}"
    );
}

#[test]
fn a_train_held_tower_carries_the_swings_past_the_walls() {
    let (g, mut ai, city, force) = walled_train("siege_tower", 4);
    assert!(g.city_allows_siege_support(city, "siege_tower"));
    let (off, _) = ai
        .conversion_siege_entry_budget(&g, 0, city, &force, 5)
        .unwrap();
    assert!(off.is_infinite());
    ai.enable_siege_budget_counts_the_ram();
    let (on, _) = ai
        .conversion_siege_entry_budget(&g, 0, city, &force, 5)
        .unwrap();
    let swing = expected_damage(
        g.unit_strength(&g.units[&force[0]], true),
        g.city_strength(city),
    );
    let expected = 200.0 / (4.0 * swing - 20.0) + 1.0;
    assert!((on - expected).abs() < 1e-9, "{on} against {expected}");
}

#[test]
fn a_ram_the_walls_are_immune_to_changes_nothing() {
    let (mut g, mut ai, city, force) = walled_train("battering_ram", 4);
    // A pool of 200 stands behind Medieval Walls, which a Ram cannot work on.
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(city, 200);
    g.observed_wall_tier_rules = true;
    assert!(!g.city_allows_siege_support(city, "battering_ram"));
    let off = ai.conversion_siege_entry_budget(&g, 0, city, &force, 5);
    ai.enable_siege_budget_counts_the_ram();
    let on = ai.conversion_siege_entry_budget(&g, 0, city, &force, 5);
    assert_eq!(off, on);
}

#[test]
fn a_ram_beyond_the_reading_radius_changes_nothing() {
    let (mut g, mut ai, city, force) = walled_train("battering_ram", 4);
    let ram = *force.last().unwrap();
    let far = (g.cities[&city].pos.0 - 8, g.cities[&city].pos.1);
    g.units.get_mut(&ram).unwrap().pos = far;
    let off = ai.conversion_siege_entry_budget(&g, 0, city, &force, 5);
    ai.enable_siege_budget_counts_the_ram();
    let on = ai.conversion_siege_entry_budget(&g, 0, city, &force, 5);
    assert_eq!(off, on);
}
