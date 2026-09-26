use super::*;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, crate::Pos) {
    let mut g = Game::new_full(2, 40, 24, 379300, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.owner_city = None;
        tile.district = None;
    }
    let home = (10, 10);
    let city = g.found_city_for(0, home, None);
    crate::game::install_test_district(&mut g, city, "aerodrome");
    let source = (24, 10);
    g.map.tiles.get_mut(&source).unwrap().resource = Some(crate::name!("aluminum"));
    g.players[0].techs.extend(
        ["flight", "radio", "advanced_flight"]
            .into_iter()
            .map(Name::new),
    );
    g.players[0].explored = g.wdisk(home, 4).into_iter().collect();
    let source_ring = g.wdisk(source, 1);
    g.players[0].explored.extend(source_ring);
    g.players[0].gold = 1000.0;
    g.players[0].gold_per_turn = 40.0;
    g.turn = 162;
    g.current = 0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.base.recon_replacement = false;
    ai.frontier_loyalty = true;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: 162,
        rush: false,
    };
    (g, ai, plan, city, source)
}

#[test]
fn known_air_supply_frontier_gets_one_scout_even_with_generic_recon_withheld() {
    let (mut g, ai, plan, _, _) = fixture();
    let bank = g.players[0].gold;
    assert!(ai.siege_resource_purchase(&mut g, 0, &plan));
    let recon = || {
        g.units
            .values()
            .filter(|u| u.owner == 0 && g.rules.units[u.kind].promotion_class == "recon")
            .count()
    };
    assert_eq!(recon(), 1);
    assert!(g.players[0].gold < bank);
    assert!(!ai.siege_resource_purchase(&mut g, 0, &plan));
}

#[test]
fn expedition_requires_known_revealed_supply_need_and_a_real_frontier() {
    for case in [
        "reveal", "hidden", "owned", "charted", "field", "flooded", "lane", "frozen", "home",
    ] {
        let (mut g, mut ai, mut plan, city, source) = fixture();
        match case {
            "reveal" => {
                g.players[0].techs.remove(&crate::name!("radio"));
            }
            "hidden" => {
                g.players[0].explored.remove(&source);
            }
            "owned" => {
                g.map.tiles.get_mut(&source).unwrap().owner_city = Some(city);
            }
            "charted" => {
                g.players[0].explored.extend(g.map.tiles.keys().copied());
            }
            "field" => {
                g.cities.get_mut(&city).unwrap().districts.clear();
            }
            "flooded" => {
                g.map.tiles.get_mut(&source).unwrap().flooded = true;
            }
            "lane" => {
                ai = AdvancedAi::targeting(VictoryTarget::Science);
            }
            "frozen" => {
                ai.base.explore_commit = false;
            }
            "home" => {
                plan.threatened_city = Some(city);
            }
            _ => unreachable!(),
        }
        let bank = g.players[0].gold;
        assert!(!ai.siege_resource_purchase(&mut g, 0, &plan), "{case}");
        assert_eq!(g.players[0].gold, bank, "{case}");
    }
}

#[test]
fn unaffordable_expedition_reserves_one_idle_queue_without_replacing_a_project() {
    let (mut g, ai, plan, city, _) = fixture();
    g.players[0].gold = 0.0;
    assert!(ai.siege_resource_purchase(&mut g, 0, &plan));
    assert!(
        matches!(g.cities[&city].queue.first(), Some(Item::Unit { unit }) if g.rules.units[*unit].promotion_class == "recon")
    );
    assert!(!ai.siege_resource_purchase(&mut g, 0, &plan));
    g.cities.get_mut(&city).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("builder"),
    }];
    assert!(!ai.siege_resource_purchase(&mut g, 0, &plan));
    assert!(
        matches!(g.cities[&city].queue.first(), Some(Item::Unit { unit }) if *unit == "builder")
    );
}

#[test]
fn resource_goal_uses_only_one_unbound_healthy_scout_and_keeps_frontier_veto() {
    let (mut g, mut ai, _, city, source) = fixture();
    let home = g.cities[&city].pos;
    let scout = g.spawn_test_unit("scout", 0, home);
    let spare = g.spawn_test_unit("scout", 0, g.nbrs(home)[0]);
    let chosen = [scout, spare]
        .into_iter()
        .find(|uid| ai.air_resource_scout_goal(&g, 0, *uid).is_some())
        .unwrap();
    let other = if chosen == scout { spare } else { scout };
    assert_eq!(ai.air_resource_scout_goal(&g, 0, other), None);
    let goal = ai.air_resource_scout_goal(&g, 0, chosen).unwrap();
    assert!(g.wdist(goal, source) <= 9);
    assert!(!g.players[0].explored.contains(&goal));
    assert!(ai
        .settle_site_frontier_loyalty_verdict(&g, 0, source)
        .is_some());
    let settler = g.spawn_test_unit("settler", 0, home);
    ai.settler_guards.insert(settler, chosen);
    assert_eq!(ai.air_resource_scout_goal(&g, 0, chosen), None);
    g.units.get_mut(&other).unwrap().hp = 20;
    assert_eq!(ai.air_resource_scout_goal(&g, 0, other), None);
}

#[test]
fn hidden_facts_do_not_select_a_resource_or_change_its_exploration_goal() {
    let (mut g, ai, _, city, source) = fixture();
    let scout = g.spawn_test_unit("scout", 0, g.cities[&city].pos);
    let frontier = ai.air_resource_frontier(&g, 0);
    let goal = ai.air_resource_scout_goal(&g, 0, scout);
    assert_eq!(frontier, Some(source));
    assert!(goal.is_some());
    for hidden in g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|p| !g.players[0].explored.contains(p))
        .collect::<Vec<_>>()
    {
        let tile = g.map.tiles.get_mut(&hidden).unwrap();
        tile.resource = Some(crate::name!("aluminum"));
        tile.terrain = crate::name!("mountain");
    }
    assert_eq!(ai.air_resource_frontier(&g, 0), frontier);
    assert_eq!(ai.air_resource_scout_goal(&g, 0, scout), goal);
    g.players[0].explored.remove(&source);
    assert_eq!(ai.air_resource_frontier(&g, 0), None);
}

#[test]
fn resource_scout_keeps_valid_commitment_and_honors_retired_targets() {
    let (mut g, ai, _, city, _) = fixture();
    let scout = g.spawn_test_unit("scout", 0, g.cities[&city].pos);
    let goal = ai.air_resource_scout_goal(&g, 0, scout).unwrap();
    ai.base
        .explore_goal
        .borrow_mut()
        .insert(scout, (goal, g.turn));
    g.turn += 1;
    assert_eq!(ai.air_resource_scout_goal(&g, 0, scout), Some(goal));
    ai.base.retire_exploration_target(&g, scout, goal);
    assert_ne!(ai.air_resource_scout_goal(&g, 0, scout), Some(goal));
}
