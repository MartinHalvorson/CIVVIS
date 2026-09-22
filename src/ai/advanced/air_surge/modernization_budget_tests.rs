use super::*;

fn fixture() -> (Game, AdvancedAi, StrategicPlan) {
    let mut g = Game::new_full(2, 40, 24, 372200, 2000, 0, false);
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
    }
    g.found_city_for(0, (6, 12), None);
    g.found_city_for(0, (12, 12), None);
    let target = g.found_city_for(1, (24, 12), None);
    g.record_contact(0, 1);
    for city in g.cities.values_mut().filter(|city| city.owner == 0) {
        city.pop = 10;
    }
    for goal in ["education", "ballistics"] {
        for tech in g.rules.tech_ancestors[goal].clone() {
            g.players[0].techs.insert(Name::new(&tech));
        }
        g.players[0].techs.insert(Name::new(goal));
    }
    for pos in [(6, 13), (7, 12)] {
        g.spawn_test_unit("field_cannon", 0, pos);
    }
    g.at_war.clear();
    g.at_war.insert((0, 1));
    g.current = 0;
    g.turn = 130;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: 130,
        rush: false,
    };
    (g, ai, plan)
}

#[test]
fn safe_modernized_army_finishes_the_industrial_branch_without_a_war_appointment() {
    let (mut g, ai, plan) = fixture();
    assert_eq!(
        ai.wartime_modernization_tech(&g, 0),
        Some(crate::name!("advanced_ballistics"))
    );
    assert!(ai.air_surge_plan.is_none());
    assert!(!ai.domination_air_readiness_active(&g, 0));
    assert_eq!(ai.air_surge_research_goal(&g, 0), Some("industrialization"));
    ai.advanced_research(&mut g, 0, &plan);
    let pick = g.players[0].research.as_deref().unwrap();
    assert!(
        ai.tech_leads_to(&g, &Name::new(pick), "industrialization"),
        "{pick}"
    );
    assert!(ai.air_surge_plan.is_none());
}

#[test]
fn industrial_bridge_keeps_opening_defence_lane_and_deadline_guards() {
    for case in [
        "education",
        "ballistics",
        "science",
        "disabled",
        "one_city",
        "home_threat",
        "too_late",
        "air_already_known",
        "fuel",
    ] {
        let (mut g, mut ai, _) = fixture();
        match case {
            "education" | "ballistics" => {
                g.players[0].techs.remove(&Name::new(case));
            }
            "science" => ai.victory_target = Some(VictoryTarget::Science),
            "disabled" => ai.disable_air_surge_2(),
            "one_city" => {
                let cid = g.player_city_ids(0)[1];
                g.cities.get_mut(&cid).unwrap().owner = 1;
            }
            "home_threat" => {
                for pos in [(5, 12), (6, 11), (5, 13)] {
                    g.spawn_test_unit("tank", 1, pos);
                }
                assert!(ai.threatened_city(&g, 0).is_some());
            }
            "fuel" => {
                g.spawn_test_unit("infantry", 0, (7, 13));
                assert_eq!(
                    ai.standing_army_fuel_goal(&g, 0),
                    Some(crate::name!("refining"))
                );
            }
            "too_late" => g.max_turns = g.turn + 1,
            "air_already_known" => {
                g.players[0].techs.insert(Name::new(AIR_SURGE_GOAL_TECH));
            }
            _ => unreachable!(),
        }
        assert_eq!(ai.air_surge_research_goal(&g, 0), None, "{case}");
    }
}

#[test]
fn industrial_milestone_hands_back_to_existing_air_readiness() {
    let (mut g, ai, _) = fixture();
    g.at_war.clear();
    assert_eq!(ai.air_surge_research_goal(&g, 0), Some("industrialization"));
    for tech in g.rules.tech_ancestors["industrialization"].clone() {
        g.players[0].techs.insert(Name::new(&tech));
    }
    g.players[0].techs.insert(crate::name!("industrialization"));
    assert_eq!(ai.air_surge_research_goal(&g, 0), Some(AIR_SURGE_GOAL_TECH));
}
