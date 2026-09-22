use super::*;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut g = Game::new_full(2, 40, 24, 370500, 300, 0, false);
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
    let city = g.found_city_for(0, (10, 10), None);
    let target = g.found_city_for(1, (25, 10), None);
    g.current = 0;
    g.players[0].techs.extend([
        crate::name!("mining"),
        crate::name!("military_engineering"),
        crate::name!("metal_casting"),
    ]);
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    g.players[0].gold = 180.0;
    g.players[0].gold_per_turn = 10.0;
    g.map.tiles.get_mut(&(12, 10)).unwrap().resource = Some(crate::name!("niter"));
    g.spawn_test_unit("trebuchet", 0, (10, 11));
    let builder = g.spawn_test_unit("builder", 0, (11, 10));
    g.units.get_mut(&builder).unwrap().charges = 3;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: 1,
        rush: false,
    };
    (
        g,
        AdvancedAi::targeting(VictoryTarget::Domination),
        plan,
        city,
    )
}

#[test]
fn buys_the_siege_supply_before_surplus_shopping() {
    let (mut g, ai, plan, city) = fixture();
    let cost = g.plot_purchase_cost(0, city, (12, 10)).unwrap();
    let before = g.players[0].gold;
    assert!(ai.advanced_gold_spending(&mut g, 0, &plan));
    assert_eq!(g.map.tiles[&(12, 10)].owner_city, Some(city));
    assert_eq!(g.players[0].gold, before - cost);
}

#[test]
fn refuses_without_a_builder_researched_upgrade_or_safe_treasury() {
    for case in ["builder", "tech", "treasury", "home", "lane"] {
        let (mut g, mut ai, mut plan, _) = fixture();
        match case {
            "builder" => {
                for u in g.units.values_mut().filter(|u| u.kind == "builder") {
                    u.charges = 0;
                }
            }
            "tech" => {
                g.players[0].techs.remove(&crate::name!("metal_casting"));
            }
            "treasury" => g.players[0].gold = 40.0,
            "home" => plan.threatened_city = Some(g.player_city_ids(0)[0]),
            "lane" => ai = AdvancedAi::targeting(VictoryTarget::Science),
            _ => unreachable!(),
        }
        assert!(!ai.siege_resource_purchase(&mut g, 0, &plan), "{case}");
        assert_eq!(g.map.tiles[&(12, 10)].owner_city, None);
    }
}
