use super::super::*;

fn fixture() -> (Game, AdvancedAi, u32, u32, u32) {
    let mut g = Game::new_full(3, 32, 24, 366_100, 300, 0, false);
    g.units.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    let mut home = Vec::new();
    for pos in [(4, 4), (10, 4), (4, 10), (10, 10)] {
        home.push(g.found_city_for(0, pos, None));
    }
    let enemy = g.found_city_for(1, (22, 4), None);
    let ally = g.found_city_for(2, (22, 12), None);
    g.players[1].religion = Some("Buddhism".into());
    g.players[2].religion = Some("Orthodoxy".into());
    for cid in home.iter().copied().chain([enemy, ally]) {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pop = 4;
        city.atheist_pressure = 0.0;
        city.pressure.clear();
        city.pressure.insert(
            if cid == ally { "Orthodoxy" } else { "Buddhism" }.into(),
            100.0,
        );
    }
    let source = home[0];
    let large = home[1];
    g.cities.get_mut(&large).unwrap().pop = 20;
    crate::game::install_test_district(&mut g, source, "holy_site");
    g.cities
        .get_mut(&source)
        .unwrap()
        .buildings
        .push(crate::name!("shrine"));
    g.current = 0;
    g.turn = 168;
    g.players[0].faith = 300.0;
    g.players[0].techs.insert(crate::name!("astrology"));
    let uid = g.spawn_unit("missionary", 0, (4, 5));
    g.units.get_mut(&uid).unwrap().religion = Some("Orthodoxy".into());
    (
        g,
        AdvancedAi::targeting(VictoryTarget::Domination),
        source,
        large,
        uid,
    )
}

#[test]
fn recover_supplier_before_larger_city_and_reopen_recruitment() {
    let (mut g, mut ai, source, large, uid) = fixture();
    let mut ordinary = g.clone();
    assert!(
        AdvancedAi::targeting(VictoryTarget::Science).advanced_missionary_step(
            &mut ordinary,
            0,
            uid,
            false
        )
    );
    assert_eq!(
        ordinary.units[&uid].charges, 3,
        "ordinary population target sends it away"
    );
    assert!(ai.advanced_missionary_step(&mut g, 0, uid, false));
    assert_eq!(g.units[&uid].charges, 2);
    assert_eq!(g.city_religion(&g.cities[&source]), Some("Orthodoxy"));
    assert_eq!(g.city_religion(&g.cities[&large]), Some("Buddhism"));
    assert!(ai
        .counterfaith_recruitment_targets(&g, 0, "Orthodoxy")
        .is_empty());
    ai.religious_defense(&mut g, 0, "Buddhism");
    assert!(g.units.values().any(|u| u.id != uid
        && u.owner == 0
        && u.kind == "missionary"
        && u.religion.as_deref() == Some("Orthodoxy")));
}

#[test]
fn last_charge_recovers_source_instead_of_exploring() {
    let (mut g, mut ai, source, _, uid) = fixture();
    ai.missionary_last_charge_explores = true;
    ai.missionary_last_charge_explores_2 = true;
    g.units.get_mut(&uid).unwrap().charges = 1;
    assert!(ai.advanced_missionary_step(&mut g, 0, uid, false));
    assert!(!g.units.contains_key(&uid));
    assert_eq!(g.city_religion(&g.cities[&source]), Some("Orthodoxy"));
}

#[test]
fn existing_counterfaith_supplier_releases_other_equipped_cities() {
    let (mut g, ai, source, other, _) = fixture();
    crate::game::install_test_district(&mut g, other, "holy_site");
    let c = g.cities.get_mut(&other).unwrap();
    c.buildings.push(crate::name!("shrine"));
    c.pressure.clear();
    c.pressure.insert("Orthodoxy".into(), 1000.0);
    assert_eq!(g.city_religion(&g.cities[&source]), Some("Buddhism"));
    assert!(ai
        .counterfaith_recruitment_targets(&g, 0, "Orthodoxy")
        .is_empty());
}

#[test]
fn pillaged_or_incomplete_infrastructure_does_not_earn_priority() {
    let (mut g, ai, source, _, _) = fixture();
    assert_eq!(
        ai.counterfaith_recruitment_targets(&g, 0, "Orthodoxy"),
        vec![source]
    );
    g.cities
        .get_mut(&source)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("shrine"));
    assert!(ai
        .counterfaith_recruitment_targets(&g, 0, "Orthodoxy")
        .is_empty());
    g.cities
        .get_mut(&source)
        .unwrap()
        .pillaged_buildings
        .clear();
    let pos = g.cities[&source].districts[&crate::name!("holy_site")];
    g.map.get_mut(pos).unwrap().pillaged = true;
    assert!(ai
        .counterfaith_recruitment_targets(&g, 0, "Orthodoxy")
        .is_empty());
    g.map.get_mut(pos).unwrap().pillaged = false;
    g.cities.get_mut(&source).unwrap().buildings.clear();
    assert!(ai
        .counterfaith_recruitment_targets(&g, 0, "Orthodoxy")
        .is_empty());
}

#[test]
fn founder_other_lane_disabled_victory_and_invading_faith_stand_aside() {
    let (mut g, ai, _, _, _) = fixture();
    assert!(AdvancedAi::targeting(VictoryTarget::Science)
        .counterfaith_recruitment_targets(&g, 0, "Orthodoxy")
        .is_empty());
    assert!(ai
        .counterfaith_recruitment_targets(&g, 0, "Buddhism")
        .is_empty());
    g.players[0].religion = Some("Orthodoxy".into());
    assert!(ai
        .counterfaith_recruitment_targets(&g, 0, "Orthodoxy")
        .is_empty());
    g.players[0].religion = None;
    g.victory_conditions.religious = false;
    assert!(ai
        .counterfaith_recruitment_targets(&g, 0, "Orthodoxy")
        .is_empty());
}

#[test]
fn recovery_cannot_finish_another_founders_religious_victory() {
    let (mut g, ai, _, _, _) = fixture();
    for cid in g.player_city_ids(1) {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pressure.clear();
        city.pressure.insert("Orthodoxy".into(), 1000.0);
    }
    assert!(!AdvancedAi::safe_adopted_counterfaith(&g, 0, "Orthodoxy"));
    assert!(ai
        .counterfaith_recruitment_targets(&g, 0, "Orthodoxy")
        .is_empty());
}
