use super::*;

fn region() -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(2, 28, 20, 365100, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let a = g.found_city_for(0, (6, 8), None);
    let b = g.found_city_for(0, (10, 8), None);
    g.found_city_for(0, (8, 12), None);
    g.players[0].techs.insert(crate::name!("industrialization"));
    for (id, pos) in [(a, (7, 8)), (b, (9, 8))] {
        let city = g.cities.get_mut(&id).unwrap();
        city.districts.insert(crate::name!("industrial_zone"), pos);
        city.buildings.push(crate::name!("workshop"));
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.district = Some(crate::name!("industrial_zone"));
        tile.owner_city = Some(id);
        tile.pillaged = false;
    }
    (g, AdvancedAi::targeting(VictoryTarget::Domination), a, b)
}

fn factory_value(g: &Game, ai: &AdvancedAi, city: u32) -> f64 {
    ai.regional_production_reach(
        g,
        0,
        &g.cities[&city],
        &crate::name!("factory"),
        &g.rules.buildings["factory"],
        GrandStrategy::Conquest,
    )
}

fn reserve(g: &mut Game, city: u32) {
    let item = Item::Building {
        building: crate::name!("factory"),
    };
    assert!(g.can_produce(0, city, &item));
    g.cities.get_mut(&city).unwrap().queue.push(item);
}

#[test]
fn overlapping_factory_queues_share_one_regional_credit() {
    let (mut g, ai, a, b) = region();
    reserve(&mut g, a);
    reserve(&mut g, b);
    g.cities.get_mut(&a).unwrap().production = 300.0;
    assert!(factory_value(&g, &ai, a) > 0.0);
    assert_eq!(factory_value(&g, &ai, b), 0.0);
    assert!(
        !g.cities[&a].buildings.contains(&crate::name!("factory")),
        "projection does not build on the observed board"
    );
}

#[test]
fn removing_a_reservation_releases_its_regional_credit() {
    let (mut g, ai, a, b) = region();
    reserve(&mut g, a);
    assert_eq!(factory_value(&g, &ai, b), 0.0);
    g.cities.get_mut(&a).unwrap().queue.clear();
    assert!(factory_value(&g, &ai, b) > 0.0);
}

#[test]
fn a_pillaged_factory_does_not_hide_missing_production() {
    let (mut g, ai, a, b) = region();
    g.cities
        .get_mut(&a)
        .unwrap()
        .buildings
        .push(crate::name!("factory"));
    assert_eq!(factory_value(&g, &ai, b), 0.0);
    g.cities
        .get_mut(&a)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("factory"));
    assert!(factory_value(&g, &ai, b) > 0.0);
}

#[test]
fn a_captured_unique_factory_covers_the_same_regional_group() {
    let (mut g, ai, a, b) = region();
    g.cities
        .get_mut(&a)
        .unwrap()
        .buildings
        .push(crate::name!("electronics_factory"));
    assert_eq!(factory_value(&g, &ai, b), 0.0);
}

#[test]
fn a_factory_can_still_reach_an_uncovered_city() {
    let (mut g, ai, a, b) = region();
    reserve(&mut g, a);
    g.found_city_for(0, (15, 8), None);
    assert!(factory_value(&g, &ai, b) > 0.0);
}

#[test]
fn other_victory_lanes_keep_their_existing_regional_score() {
    let (mut g, _, a, b) = region();
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    let before = factory_value(&g, &ai, b);
    reserve(&mut g, a);
    assert_eq!(factory_value(&g, &ai, b), before);
}

#[test]
fn an_illegal_queue_does_not_reserve_regional_production() {
    let (mut g, ai, a, b) = region();
    reserve(&mut g, a);
    g.cities.get_mut(&a).unwrap().buildings.clear();
    assert!(factory_value(&g, &ai, b) > 0.0);
}

#[test]
fn vertical_integration_still_values_another_reaching_factory() {
    let (mut g, ai, a, b) = region();
    g.cities
        .get_mut(&a)
        .unwrap()
        .buildings
        .push(crate::name!("factory"));
    assert_eq!(factory_value(&g, &ai, b), 0.0);
    g.turn = 10;
    g.players[0].governor_roster.insert(
        "magnus".into(),
        crate::game::GovernorState {
            city: Some(a),
            assigned_turn: 0,
            disabled_until: 0,
            promotions: ["vertical_integration".to_string()].into_iter().collect(),
        },
    );
    assert!(factory_value(&g, &ai, b) > 0.0);
}
