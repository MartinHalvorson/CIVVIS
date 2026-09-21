use super::super::*;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut g = Game::new_full(3, 32, 24, 365_400, 300, 0, false);
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
