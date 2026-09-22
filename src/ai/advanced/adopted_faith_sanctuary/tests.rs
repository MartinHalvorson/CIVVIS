use super::super::*;

fn fixture_with_players(players: usize) -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut g = Game::new_full(players, 32, 24, 365_400, 300, 0, false);
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
    let invader = g.found_city_for(1, (22, 4), None);
    let ally = g.found_city_for(2, (22, 12), None);
    g.players[1].religion = Some("Buddhism".into());
    g.players[2].religion = Some("Orthodoxy".into());
    for (cid, faith) in home
        .iter()
        .copied()
        .map(|cid| {
            (
                cid,
                if cid == home[0] {
                    "Orthodoxy"
                } else {
                    "Buddhism"
                },
            )
        })
        .chain([(invader, "Buddhism"), (ally, "Orthodoxy")])
    {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pop = 4;
        city.atheist_pressure = 0.0;
        city.pressure.clear();
        city.pressure.insert(faith.into(), 1000.0);
    }
    g.current = 0;
    g.turn = 98;
    g.players[0].faith = 300.0;
    g.players[0].gold_per_turn = 20.0;
    g.players[0].techs.insert(crate::name!("astrology"));
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(invader),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, home[0])
}

#[test]
fn minority_counterfaith_in_first_city_is_not_the_invasion() {
    let (g, ai, _, _) = fixture();
    assert_eq!(
        ai.home_conversion_threat(&g, 0).as_deref(),
        Some("Buddhism")
    );
}

#[test]
fn legal_holy_site_reservation_survives_production_review() {
    let (mut g, mut ai, plan, home) = fixture();
    ai.reserve_adopted_faith_sanctuary(&mut g, 0, &plan);
    let item = g.cities[&home]
        .queue
        .first()
        .cloned()
        .expect("defensive source reserved");
    assert!(matches!(item, Item::District { district, .. } if district == "holy_site"));
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&home].queue.first(), Some(&item));
}

#[test]
fn completed_holy_site_gets_shrine_then_existing_defense_buys_counterfaith() {
    let (mut g, mut ai, plan, home) = fixture();
    crate::game::install_test_district(&mut g, home, "holy_site");
    ai.reserve_adopted_faith_sanctuary(&mut g, 0, &plan);
    assert_eq!(
        g.cities[&home].queue.first(),
        Some(&Item::Building {
            building: crate::name!("shrine")
        })
    );
    g.cities.get_mut(&home).unwrap().queue.clear();
    g.cities
        .get_mut(&home)
        .unwrap()
        .buildings
        .push(crate::name!("shrine"));
    assert!(ai.adopted_faith_sanctuary_choice(&g, 0, None).is_none());
    ai.religious_defense(&mut g, 0, "Buddhism");
    assert!(g.units.values().any(|u| u.owner == 0
        && u.kind == "missionary"
        && u.religion.as_deref() == Some("Orthodoxy")));
}

#[test]
fn converted_or_threatened_supplier_is_not_reserved() {
    let (mut g, ai, _, home) = fixture();
    assert!(ai
        .adopted_faith_sanctuary_choice(&g, 0, Some(home))
        .is_none());
    let city = g.cities.get_mut(&home).unwrap();
    city.pressure.clear();
    city.pressure.insert("Buddhism".into(), 1000.0);
    assert!(ai.adopted_faith_sanctuary_choice(&g, 0, None).is_none());
}

#[test]
fn founder_other_lane_disabled_victory_and_unfunded_cases_stand_aside() {
    let (mut g, ai, _, _) = fixture();
    let science = AdvancedAi::targeting(VictoryTarget::Science);
    assert!(science
        .adopted_faith_sanctuary_choice(&g, 0, None)
        .is_none());
    g.players[0].religion = Some("Our faith".into());
    assert!(ai.adopted_faith_sanctuary_choice(&g, 0, None).is_none());
    g.players[0].religion = None;
    g.victory_conditions.religious = false;
    assert!(ai.adopted_faith_sanctuary_choice(&g, 0, None).is_none());
    g.victory_conditions.religious = true;
    g.players[0].faith = 0.0;
    assert!(ai.adopted_faith_sanctuary_choice(&g, 0, None).is_none());
}

#[test]
fn counterfaith_cannot_finish_another_founders_victory() {
    let (mut g, ai, _, _) = fixture();
    for cid in g.player_city_ids(1) {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pressure.clear();
        city.pressure.insert("Orthodoxy".into(), 1000.0);
    }
    assert!(!AdvancedAi::safe_adopted_counterfaith(&g, 0, "Orthodoxy"));
    assert!(ai.adopted_faith_sanctuary_choice(&g, 0, None).is_none());
}

#[test]
fn repeated_reservation_does_not_start_a_second_supplier() {
    let (mut g, ai, plan, home) = fixture();
    let other = g
        .player_city_ids(0)
        .into_iter()
        .find(|cid| *cid != home)
        .unwrap();
    let city = g.cities.get_mut(&other).unwrap();
    city.pressure.clear();
    city.pressure.insert("Orthodoxy".into(), 1000.0);
    ai.reserve_adopted_faith_sanctuary(&mut g, 0, &plan);
    ai.reserve_adopted_faith_sanctuary(&mut g, 0, &plan);
    assert_eq!(
        g.player_city_ids(0)
            .into_iter()
            .filter(|cid| g.cities[cid].queue.first().is_some_and(
                |item| matches!(item, Item::District { district, .. } if district == "holy_site")
            ))
            .count(),
        1
    );
}

#[test]
fn adopted_missionaries_stop_when_their_counterfaith_becomes_the_threat() {
    let (mut g, ai, _, home) = fixture();
    let target = g
        .player_city_ids(0)
        .into_iter()
        .find(|cid| *cid != home)
        .unwrap();
    let unit = g.spawn_test_unit("missionary", 0, g.cities[&target].pos);
    g.units.get_mut(&unit).unwrap().religion = Some("Orthodoxy".into());
    assert!(ai.advanced_missionary_step(&mut g, 0, unit, false));
    assert_eq!(
        g.units[&unit].charges, 2,
        "safe adopted faith still defends"
    );

    // The same purchased unit remains alive while the other cities convert.
    for cid in g.player_city_ids(0) {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pressure.clear();
        city.pressure.insert(
            if cid == target {
                "Buddhism"
            } else {
                "Orthodoxy"
            }
            .into(),
            1000.0,
        );
    }
    assert_eq!(ai.adopted_faith_threat(&g, 0).as_deref(), Some("Orthodoxy"));
    g.units.get_mut(&unit).unwrap().moves_left = 4.0;
    let before = g.cities[&target].pressure.clone();
    for offensive in [false, true] {
        assert!(!ai.advanced_missionary_step(&mut g, 0, unit, offensive));
        assert_eq!(g.units[&unit].charges, 2);
        assert_eq!(g.cities[&target].pressure, before);
    }

    // Re-evaluate the current board, rather than permanently disabling a unit.
    for cid in g.player_city_ids(0).into_iter().filter(|cid| *cid != home) {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pressure.clear();
        city.pressure.insert("Buddhism".into(), 1000.0);
    }
    assert!(ai.advanced_missionary_step(&mut g, 0, unit, false));
    assert_eq!(g.units[&unit].charges, 1);
}

#[test]
fn adopted_spread_cannot_help_a_faith_that_holds_every_other_major() {
    let (mut g, ai, _, home) = fixture();
    let target = g
        .player_city_ids(0)
        .into_iter()
        .find(|cid| *cid != home)
        .unwrap();
    for cid in g.player_city_ids(1) {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pressure.clear();
        city.pressure.insert("Orthodoxy".into(), 1000.0);
    }
    let unit = g.spawn_test_unit("missionary", 0, g.cities[&target].pos);
    g.units.get_mut(&unit).unwrap().religion = Some("Orthodoxy".into());
    assert!(!ai.advanced_missionary_step(&mut g, 0, unit, false));
    assert_eq!(g.units[&unit].charges, 3);
}

#[test]
fn adopted_spread_restraint_preserves_founder_other_lane_and_disabled_victory() {
    for mode in ["founder", "science", "disabled"] {
        let (mut g, mut ai, _, home) = fixture();
        let unit = g.spawn_test_unit("missionary", 0, g.cities[&home].pos);
        g.units.get_mut(&unit).unwrap().religion = Some("Buddhism".into());
        match mode {
            "founder" => {
                g.players[1].religion = None;
                g.players[0].religion = Some("Buddhism".into());
            }
            "science" => ai = AdvancedAi::targeting(VictoryTarget::Science),
            _ => g.victory_conditions.religious = false,
        }
        assert!(
            ai.advanced_missionary_step(&mut g, 0, unit, false),
            "{mode}"
        );
        assert_eq!(g.units[&unit].charges, 2, "{mode}");
    }
}

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32) {
    fixture_with_players(3)
}

fn early_warning_fixture() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let (mut g, ai, plan, supplier) = fixture_with_players(4);
    let home = g.player_city_ids(0);
    for cid in home.iter().skip(2) {
        let city = g.cities.get_mut(cid).unwrap();
        city.pressure.clear();
        city.atheist_pressure = 1000.0;
    }
    let converted = g.found_city_for(3, (20, 18), None);
    let city = g.cities.get_mut(&converted).unwrap();
    city.pop = 4;
    city.atheist_pressure = 0.0;
    city.pressure.clear();
    city.pressure.insert("Buddhism".into(), 1000.0);
    (g, ai, plan, supplier, converted)
}

#[test]
fn global_conversion_lead_reserves_supplier_before_home_majority() {
    let (mut g, mut ai, plan, supplier, _) = early_warning_fixture();
    assert!(ai.adopted_faith_threat(&g, 0).is_none());
    assert_eq!(
        g.player_city_ids(0)
            .iter()
            .filter(|cid| g.city_religion(&g.cities[cid]) == Some("Buddhism"))
            .count(),
        1
    );
    ai.reserve_adopted_faith_sanctuary(&mut g, 0, &plan);
    let item = g.cities[&supplier]
        .queue
        .first()
        .cloned()
        .expect("start the defensive chain while the minority source survives");
    assert!(matches!(item, Item::District { district, .. } if district == "holy_site"));
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&supplier].queue.first(), Some(&item));
    assert!(
        ai.adopted_faith_threat(&g, 0).is_none(),
        "construction warning does not lower the ordinary spread/purchase alarm"
    );
}

#[test]
fn construction_warning_requires_a_foreign_conversion_and_home_arrival() {
    let (mut g, ai, _, _, converted) = early_warning_fixture();
    let city = g.cities.get_mut(&converted).unwrap();
    city.pressure.clear();
    city.atheist_pressure = 1000.0;
    assert!(ai.adopted_faith_sanctuary_choice(&g, 0, None).is_none());
    let (mut g, ai, _, _, _) = early_warning_fixture();
    for cid in g.player_city_ids(0) {
        let city = g.cities.get_mut(&cid).unwrap();
        if city.pressure.contains_key("Buddhism") {
            city.pressure.clear();
            city.atheist_pressure = 1000.0;
        }
    }
    assert!(ai.adopted_faith_sanctuary_choice(&g, 0, None).is_none());
}

#[test]
fn one_converted_foreign_civilization_below_half_is_not_a_construction_alarm() {
    let (mut g, ai, _, _, _) = early_warning_fixture();
    let founder_city = g.player_city_ids(1)[0];
    let city = g.cities.get_mut(&founder_city).unwrap();
    city.pressure.clear();
    city.atheist_pressure = 1000.0;
    assert!(g.civ_follows_religion(3, "Buddhism"));
    assert!(!g.civ_follows_religion(1, "Buddhism"));
    assert!(ai.adopted_faith_sanctuary_choice(&g, 0, None).is_none());
}

fn founder_supplier_fixture() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let (mut g, ai, plan, home) = fixture();
    g.players[0].religion = Some("Our faith".into());
    let city = g.cities.get_mut(&home).unwrap();
    city.pressure.clear();
    city.pressure.insert("Our faith".into(), 1000.0);
    crate::game::install_test_district(&mut g, home, "holy_site");
    (g, ai, plan, home)
}

#[test]
fn founder_supplier_keeps_shrine_through_review_and_recruits_own_defender() {
    let (mut g, mut ai, plan, home) = founder_supplier_fixture();
    let shrine = Item::Building {
        building: crate::name!("shrine"),
    };
    ai.reserve_adopted_faith_sanctuary(&mut g, 0, &plan);
    assert_eq!(g.cities[&home].queue.first(), Some(&shrine));
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&home].queue.first(), Some(&shrine));
    let city = g.cities.get_mut(&home).unwrap();
    city.queue.clear();
    city.buildings.push(crate::name!("shrine"));
    g = g.speculative_clone();
    assert!(ai.adopted_faith_sanctuary_choice(&g, 0, None).is_none());
    ai.religious_defense(&mut g, 0, "Buddhism");
    assert!(g.units.values().any(|u| u.owner == 0
        && u.kind == "missionary"
        && u.religion.as_deref() == Some("Our faith")));
}

#[test]
fn founder_supplier_requires_live_pressure_and_a_surviving_own_majority() {
    let (mut g, ai, _, home) = founder_supplier_fixture();
    for cid in g.player_city_ids(0).into_iter().filter(|cid| *cid != home) {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pressure.clear();
        city.atheist_pressure = 1000.0;
    }
    assert!(ai.adopted_faith_sanctuary_choice(&g, 0, None).is_none());
    let (mut g, ai, _, home) = founder_supplier_fixture();
    let city = g.cities.get_mut(&home).unwrap();
    city.pressure.clear();
    city.pressure.insert("Orthodoxy".into(), 1000.0);
    assert!(ai.adopted_faith_sanctuary_choice(&g, 0, None).is_none());
}

#[test]
fn founder_supplier_preserves_lane_funding_and_local_defense_guards() {
    let (mut g, ai, _, home) = founder_supplier_fixture();
    assert!(ai
        .adopted_faith_sanctuary_choice(&g, 0, Some(home))
        .is_none());
    assert!(AdvancedAi::targeting(VictoryTarget::Science)
        .adopted_faith_sanctuary_choice(&g, 0, None)
        .is_none());
    g.victory_conditions.religious = false;
    assert!(ai.adopted_faith_sanctuary_choice(&g, 0, None).is_none());
    g.victory_conditions.religious = true;
    g.players[0].faith = 0.0;
    assert!(ai.adopted_faith_sanctuary_choice(&g, 0, None).is_none());
}
