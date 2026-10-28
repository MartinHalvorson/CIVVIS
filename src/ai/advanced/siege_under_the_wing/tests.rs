use super::*;

/// Two cities of ours at (12, 10) and (6, 16); the rival's `far` city 18
/// tiles out at (30, 10), behind 300 walls our Catapult cannot breach (city
/// strength 100), and its `near` city at (20, 10), eight tiles from home.
/// Two Bombers stand at home, a Catapult and a Swordsman beside them.
fn wing_case() -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(2, 48, 24, 4062, 650, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    g.clear_mirror_cities();
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (12, 10), None);
    g.found_city_for(0, (6, 16), None);
    let near = g.found_city_for(1, (20, 10), None);
    let far = g.found_city_for(1, (30, 10), None);
    let city = g.cities.get_mut(&far).unwrap();
    for walls in ["walls", "medieval_walls", "renaissance_walls"] {
        city.buildings.push(Name::new(walls));
    }
    let max = g.city_max_wall_hp(&g.cities[&far]);
    assert!(max > 0);
    g.cities.get_mut(&far).unwrap().wall_hp = max;
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(far, 100.0);
    g.current = 0;
    g.turn = 170;
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    for kind in ["bomber", "bomber", "catapult", "swordsman"] {
        g.spawn_test_unit(kind, 0, (12, 10));
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_breakers_match_the_walls();
    (g, ai, far, near)
}

/// Gene off, the unbreachable city out of reach keeps the siege; gene on,
/// it moves to the city the wing can hit, and that city keeps it.
#[test]
fn an_unbreachable_city_out_of_reach_yields_to_one_under_the_wing() {
    let (g, mut ai, far, near) = wing_case();
    assert!(matches!(
        ai.breakers_matched_to_walls(&g, 0, far),
        Some(siege_production::BreakerMatch { ask: None, .. })
    ));
    assert!(!AdvancedAi::air_surge_in_range(&g, 0, g.cities[&far].pos));
    assert!(AdvancedAi::air_surge_in_range(&g, 0, g.cities[&near].pos));
    let conquest = GrandStrategy::Conquest;
    assert_eq!(
        ai.siege_under_the_wing_city(&g, 0, far, conquest),
        None,
        "off"
    );
    ai.enable_siege_moves_under_the_wing();
    assert_eq!(
        ai.siege_under_the_wing_city(&g, 0, far, conquest),
        Some(near)
    );
    assert_eq!(
        ai.siege_under_the_wing_city(&g, 0, near, conquest),
        None,
        "in reach"
    );
    assert_eq!(
        ai.siege_under_the_wing_city(&g, 0, far, GrandStrategy::Expansion),
        None,
        "Conquest only"
    );
}

/// Each gate holds the siege: a wall the land guns can breach, a siege that
/// has opened its city, a single Bomber, or peace.
#[test]
fn the_siege_stays_while_any_gate_fails() {
    let conquest = GrandStrategy::Conquest;
    for case in ["breachable", "damaged", "one_bomber", "peace"] {
        let (mut g, mut ai, far, _) = wing_case();
        ai.enable_siege_moves_under_the_wing();
        match case {
            "breachable" => {
                std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(far, 40.0);
                assert!(matches!(
                    ai.breakers_matched_to_walls(&g, 0, far),
                    Some(siege_production::BreakerMatch { ask: Some(_), .. })
                ));
            }
            "damaged" => g.cities.get_mut(&far).unwrap().wall_hp -= 50,
            "one_bomber" => {
                let bomber = g
                    .player_unit_ids(0)
                    .into_iter()
                    .find(|uid| g.units[uid].kind == "bomber")
                    .unwrap();
                g.remove_unit(bomber);
            }
            "peace" => {
                g.at_war.clear();
            }
            _ => unreachable!(),
        }
        assert_eq!(
            ai.siege_under_the_wing_city(&g, 0, far, conquest),
            None,
            "{case}"
        );
    }
}

/// Through `assess`: a plan committed to the far city keeps it with the gene
/// off and moves under the wing with it on.
#[test]
fn assess_aims_the_campaign_under_the_wing() {
    let aimed = |gene: bool| {
        let (g, mut ai, far, near) = wing_case();
        ai.enable_siege_commitment();
        if gene {
            ai.enable_siege_moves_under_the_wing();
        }
        ai.plan = Some(StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(far),
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: g.turn - 1,
            rush: false,
        });
        let plan = ai.assess(&g, 0);
        (plan.target_player, plan.target_city, far, near)
    };
    let (player, city, far, _) = aimed(false);
    assert_eq!((player, city), (Some(1), Some(far)), "off");
    let (player, city, _, near) = aimed(true);
    assert_eq!((player, city), (Some(1), Some(near)), "on");
}
