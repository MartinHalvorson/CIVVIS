use super::*;

fn threatened_culture() -> (Game, StrategicPlan) {
    let mut g = Game::new(2, 24, 16, 774_4071, 250, 0);
    g.players[0].civ = "Greece".to_string();
    let home = g.units[&g.player_unit_ids(0)[0]].pos;
    g.found_city_for(0, home, None);
    for uid in g.player_unit_ids(0) {
        g.remove_unit(uid);
    }
    g.spawn_test_unit("hoplite", 0, home);
    g.spawn_test_unit("hoplite", 0, home);
    g.players[0]
        .techs
        .extend([crate::name!("mining"), crate::name!("bronze_working")]);
    g.at_war.insert((0, 1));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        target_player: None,
        target_city: None,
        threatened_city: Some(g.player_city_ids(0)[0]),
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, plan)
}

#[test]
fn besieged_culture_unlocks_walls_before_pikeman_prerequisites() {
    let (mut g, plan) = threatened_culture();
    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    assert_eq!(
        ai.wartime_modernization_tech(&g, 0).as_deref(),
        Some("military_tactics")
    );
    g.players[0].research = None;
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("masonry"));
    g.players[0].techs.insert(crate::name!("masonry"));
    assert!(g.can_produce(
        0,
        plan.threatened_city.unwrap(),
        &crate::game::Item::Building {
            building: crate::name!("walls"),
        }
    ));
    g.players[0].research = None;
    ai.advanced_research(&mut g, 0, &plan);
    assert_ne!(g.players[0].research.as_deref(), Some("masonry"));
}

#[test]
fn wall_unlock_requires_an_unwalled_culture_city_in_a_defensive_war() {
    let (mut g, mut plan) = threatened_culture();
    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    assert!(ai.culture_defensive_walls_goal(&g, 0, &plan).is_some());
    assert!(AdvancedAi::targeting(VictoryTarget::Science)
        .culture_defensive_walls_goal(&g, 0, &plan)
        .is_none());
    plan.strategy = GrandStrategy::Expansion;
    assert!(ai.culture_defensive_walls_goal(&g, 0, &plan).is_none());
    plan.strategy = GrandStrategy::Recovery;
    let city = plan.threatened_city.take().unwrap();
    // A power-deficit recovery is the warning; the city need not already
    // be in the attacker's one-turn capture radius.
    assert!(ai.culture_defensive_walls_goal(&g, 0, &plan).is_some());
    plan.threatened_city = Some(city);
    g.at_war.clear();
    assert!(ai.culture_defensive_walls_goal(&g, 0, &plan).is_none());
    g.at_war.insert((0, 1));
    g.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    assert!(ai.culture_defensive_walls_goal(&g, 0, &plan).is_none());
}
