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

fn coastal_fixture() -> (Game, AdvancedAi, StrategicPlan, u32, crate::Pos) {
    let (mut g, ai, plan, city, source) = fixture();
    for (pos, tile) in &mut g.map.tiles {
        if pos.0 > source.0 {
            tile.terrain = crate::name!("coast");
        }
    }
    g.players[0].techs.extend(
        ["sailing", "shipbuilding", "cartography"]
            .into_iter()
            .map(Name::new),
    );
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

#[test]
fn a_healthy_connected_source_does_not_hide_insufficient_bomber_income() {
    let (mut g, ai, _, city, source) = fixture();
    let home = g.cities[&city].pos;
    g.map.tiles.get_mut(&home).unwrap().resource = Some(crate::name!("aluminum"));
    for _ in 0..3 {
        g.spawn_test_unit("bomber", 0, home);
    }
    assert_eq!(g.strategic_resource_rate(0, "aluminum"), 2.0);
    assert_eq!(
        ai.air_resource_shortfall(&g, 0),
        Some(crate::name!("aluminum"))
    );
    assert_eq!(ai.air_resource_frontier(&g, 0), Some(source));
    g.map.tiles.get_mut(&home).unwrap().pillaged = true;
    assert_eq!(ai.air_resource_frontier(&g, 0), None);
}

#[test]
fn free_resource_scout_actually_advances_before_routine_military_assignments() {
    let (mut g, mut ai, plan, city, source) = fixture();
    let home = g.cities[&city].pos;
    let scout = g.spawn_test_unit("scout", 0, home);
    let before = g.wdist(home, source);
    assert!(ai.advanced_military_step_with_decline(&mut g, 0, scout, &plan, true));
    assert!(g.wdist(g.units[&scout].pos, source) < before);
    let (goal, _) = ai.base.explore_goal.borrow().get(&scout).copied().unwrap();
    assert!(g.wdist(goal, source) <= 9);
    assert!(ai
        .settle_site_frontier_loyalty_verdict(&g, 0, source)
        .is_some());
}

#[test]
fn resource_scout_can_clear_the_remote_land_frontier_without_relaxing_its_veto() {
    let (mut g, mut ai, _, city, source) = fixture();
    let scout = g.spawn_test_unit("scout", 0, g.cities[&city].pos);
    assert!(AdvancedAi::beyond_loyalty_reach(&g, 0, source));
    for _ in 0..60 {
        g.turn += 1;
        let moves = g.unit_max_moves(scout);
        let u = g.units.get_mut(&scout).unwrap();
        u.moves_left = moves;
        u.moved = false;
        u.acted = false;
        u.attacks_left = 1;
        for _ in 0..8 {
            if ai.distance_scout_step(&mut g, 0, scout) != Some(true) {
                break;
            }
        }
        if !AdvancedAi::beyond_loyalty_reach(&g, 0, source) {
            break;
        }
    }
    assert!(!AdvancedAi::beyond_loyalty_reach(&g, 0, source));
    assert!(ai
        .settle_site_frontier_loyalty_verdict(&g, 0, source)
        .is_none());
}

#[test]
fn resource_scout_can_clear_a_coastal_supply_neighborhood_without_relaxing_loyalty() {
    let (mut g, mut ai, plan, city, source) = coastal_fixture();
    let scout = g.spawn_test_unit("scout", 0, g.cities[&city].pos);
    assert!(AdvancedAi::beyond_loyalty_reach(&g, 0, source));
    assert!(ai
        .settle_site_frontier_loyalty_verdict(&g, 0, source)
        .is_some());
    let mut embarked = false;
    for _ in 0..100 {
        g.turn += 1;
        let moves = g.unit_max_moves(scout);
        let u = g.units.get_mut(&scout).unwrap();
        u.moves_left = moves;
        u.moved = false;
        u.acted = false;
        u.attacks_left = 1;
        for _ in 0..8 {
            if !ai.advanced_military_step_with_decline(&mut g, 0, scout, &plan, true) {
                break;
            }
            embarked |= g.is_embarked(&g.units[&scout]);
        }
        if !AdvancedAi::beyond_loyalty_reach(&g, 0, source) {
            break;
        }
    }
    assert!(
        embarked,
        "fixture must exercise the water survey, not land-only sight"
    );
    assert!(!AdvancedAi::beyond_loyalty_reach(&g, 0, source));
    assert!(ai
        .settle_site_frontier_loyalty_verdict(&g, 0, source)
        .is_none());
}

#[test]
fn ordinary_land_exploration_keeps_come_ashore_outside_the_supply_survey() {
    for kind in ["scout", "warrior"] {
        let (mut g, mut ai, _, city, shore) = coastal_fixture();
        if kind == "scout" {
            g.cities.get_mut(&city).unwrap().districts.clear();
        }
        g.players[0].explored.extend(
            g.map
                .tiles
                .iter()
                .filter(|(_, tile)| !g.rules.is_water(tile))
                .map(|(pos, _)| *pos),
        );
        let unit = g.spawn_test_unit(kind, 0, shore);
        if kind == "warrior" {
            let survey = g.spawn_test_unit("scout", 0, g.cities[&city].pos);
            assert!(ai.air_resource_scout_goal(&g, 0, survey).is_some());
        }
        let water = g
            .nbrs(shore)
            .into_iter()
            .find(|p| g.map.get(*p).is_some_and(|tile| g.rules.is_water(tile)))
            .unwrap();
        assert!(g.unit_can_traverse(unit, water));
        assert!(ai.base.come_ashore);
        assert_eq!(ai.air_resource_scout_goal(&g, 0, unit), None);
        assert!(!ai.base.explore_step(&mut g, 0, unit), "{kind}");
        assert_eq!(ai.distance_scout_step(&mut g, 0, unit), None, "{kind}");
        assert_eq!(g.units[&unit].pos, shore, "{kind}");
    }
}

#[test]
fn deeper_coastal_survey_needs_a_charted_water_route_the_scout_can_use() {
    let (mut g, ai, _, _, source) = coastal_fixture();
    g.players[0].explored.extend(
        g.map
            .tiles
            .iter()
            .filter(|(pos, _)| pos.0 <= source.0 + 2)
            .map(|(pos, _)| *pos),
    );
    let scout = g.spawn_test_unit("scout", 0, source);
    g.players[0].techs.remove(&crate::name!("shipbuilding"));
    g.players[0].techs.remove(&crate::name!("cartography"));
    assert!(AdvancedAi::beyond_loyalty_reach(&g, 0, source));
    assert_eq!(ai.air_resource_scout_goal(&g, 0, scout), None);
    g.players[0].techs.insert(crate::name!("shipbuilding"));
    g.players[0].techs.insert(crate::name!("cartography"));
    assert!(ai.air_resource_scout_goal(&g, 0, scout).is_some());
}

#[test]
fn a_coastal_survey_does_not_reuse_the_scouts_host_retired_goal() {
    let (mut g, ai, _, _, source) = coastal_fixture();
    g.players[0].explored.extend(
        g.map
            .tiles
            .iter()
            .filter(|(pos, _)| pos.0 <= source.0 + 2)
            .map(|(pos, _)| *pos),
    );
    let scout = g.spawn_test_unit("scout", 0, source);
    let goal = ai.air_resource_scout_goal(&g, 0, scout).unwrap();
    assert!(g.rules.is_water(&g.map.tiles[&goal]));
    ai.base.retire_exploration_target(&g, scout, goal);
    assert_ne!(ai.air_resource_scout_goal(&g, 0, scout), Some(goal));
}
