use super::*;

fn fixture() -> (Game, AdvancedAi) {
    let mut g = Game::new_full(4, 40, 24, 936029, 650, 0, false);
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
    g.found_city_for(0, (12, 12), None);
    let target = g.found_city_for(1, (18, 12), None);
    g.found_city_for(2, (12, 19), None);
    g.found_city_for(3, (30, 4), None);
    for other in 1..4 {
        g.record_contact(0, other);
    }
    g.players[0].techs.extend(
        g.rules.tech_ancestors[AIR_SURGE_GOAL_TECH]
            .iter()
            .map(|tech| Name::new(tech)),
    );
    g.players[0].gold = 10000.0;
    g.players[0]
        .strategic_resources
        .insert(crate::name!("aluminum"), 400.0);
    g.at_war.clear();
    g.current = 0;
    g.turn = 150;
    for pos in [(12, 13), (13, 12)] {
        g.spawn_test_unit("cuirassier", 0, pos);
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    ai.air_surge_plan = Some(AirSurge {
        target_player: 1,
        objective_city: target,
        objective_pos: g.cities[&target].pos,
        body_unit: crate::name!("knight"),
        body_is_cavalry: true,
        opened_at_war: false,
        phase: AirSurgePhase::Beeline,
        appointed_turn: 140,
        tech_turn: None,
        declared_turn: None,
        last_reviewed_turn: 150,
        recovery_assessments: 0,
    });
    (g, ai)
}

fn ready_wing(g: &mut Game) {
    g.players[0].techs.insert(Name::new(AIR_SURGE_GOAL_TECH));
    for pos in [(12, 12), (11, 12)] {
        g.spawn_test_unit("bomber", 0, pos);
    }
}

#[test]
fn a_new_major_war_redirects_the_existing_wing_without_restarting_research() {
    let (mut g, mut ai) = fixture();
    g.at_war.insert((0, 2));
    assert_eq!(ai.choose_air_surge(&g, 0).unwrap().target_player, 2);
    ai.maintain_air_surge(&g, 0);
    let surge = ai.air_surge_plan.as_ref().unwrap();
    assert_eq!(surge.target_player, 2);
    assert_eq!(g.cities[&surge.objective_city].owner, 2);
    assert!(surge.opened_at_war);
    assert_eq!(surge.phase, AirSurgePhase::Exploit);
    assert_eq!(surge.appointed_turn, 140);
    assert_eq!(ai.air_surge_research_goal(&g, 0), Some(AIR_SURGE_GOAL_TECH));
    assert_eq!(ai.air_surge_cooldown_until, 0);
}

#[test]
fn retargeting_preserves_the_breakthrough_clock() {
    let (mut g, mut ai) = fixture();
    ready_wing(&mut g);
    ai.air_surge_plan.as_mut().unwrap().tech_turn = Some(149);
    g.at_war.insert((0, 2));
    ai.maintain_air_surge(&g, 0);
    let surge = ai.air_surge_plan.as_ref().unwrap();
    assert_eq!(surge.target_player, 2);
    assert_eq!(surge.tech_turn, Some(149));
    assert_eq!(surge.appointed_turn, 140);
}

#[test]
fn an_unreachable_counter_does_not_let_the_peacetime_wing_override_the_war() {
    let (mut g, mut ai) = fixture();
    ready_wing(&mut g);
    g.at_war.insert((0, 3));
    assert!(ai.choose_air_surge(&g, 0).is_none());
    ai.maintain_air_surge(&g, 0);
    let surge = ai.air_surge_plan.as_ref().unwrap();
    assert_eq!(surge.target_player, 1);
    assert!(ai.air_surge_status.wing_ready());
    assert!(ai.air_surge_status.escort_ready());
    assert_eq!(surge.phase, AirSurgePhase::Arm);
    let mut strategy = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(3),
        target_city: Some(g.player_city_ids(3)[0]),
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    };
    ai.apply_air_surge_to_strategy(&mut strategy);
    assert_eq!(strategy.target_player, Some(3));
}

#[test]
fn a_wing_already_fighting_its_target_keeps_that_target_on_a_second_front() {
    let (mut g, mut ai) = fixture();
    g.at_war.extend([(0, 1), (0, 2)]);
    ai.air_surge_plan.as_mut().unwrap().opened_at_war = true;
    ai.maintain_air_surge(&g, 0);
    assert_eq!(ai.air_surge_plan.as_ref().unwrap().target_player, 1);
    assert_eq!(
        ai.air_surge_plan.as_ref().unwrap().phase,
        AirSurgePhase::Exploit
    );
}

#[test]
fn two_new_fronts_hold_the_elective_strike_without_choosing_an_arbitrary_war() {
    let (mut g, mut ai) = fixture();
    ready_wing(&mut g);
    g.at_war.extend([(0, 2), (0, 3)]);
    ai.maintain_air_surge(&g, 0);
    let surge = ai.air_surge_plan.as_ref().unwrap();
    assert_eq!(surge.target_player, 1);
    assert_eq!(surge.phase, AirSurgePhase::Arm);
}

#[test]
fn peace_still_allows_the_ready_elective_strike() {
    let (mut g, mut ai) = fixture();
    ready_wing(&mut g);
    ai.maintain_air_surge(&g, 0);
    let surge = ai.air_surge_plan.as_ref().unwrap();
    assert_eq!(surge.target_player, 1);
    assert_eq!(surge.phase, AirSurgePhase::Strike);
}

#[test]
fn a_minor_war_does_not_redirect_the_major_air_campaign() {
    let (mut g, mut ai) = fixture();
    ready_wing(&mut g);
    g.players[2].is_minor = true;
    g.at_war.insert((0, 2));
    ai.maintain_air_surge(&g, 0);
    let surge = ai.air_surge_plan.as_ref().unwrap();
    assert_eq!(surge.target_player, 1);
    assert_eq!(surge.phase, AirSurgePhase::Strike);
}

#[test]
fn a_city_leaving_the_enemy_keeps_the_wing_on_another_city_of_that_enemy() {
    let (mut g, mut ai) = fixture();
    let old_city = ai.air_surge_plan.as_ref().unwrap().objective_city;
    let replacement = g.found_city_for(1, (16, 17), None);
    g.cities.get_mut(&old_city).unwrap().owner = 3;
    g.at_war.insert((0, 1));
    ai.air_surge_plan.as_mut().unwrap().opened_at_war = true;
    ai.maintain_air_surge(&g, 0);
    let surge = ai.air_surge_plan.as_ref().unwrap();
    assert_eq!(surge.target_player, 1);
    assert_eq!(surge.objective_city, replacement);
    assert_eq!(surge.phase, AirSurgePhase::Exploit);
    assert_eq!(surge.appointed_turn, 140);
    assert_eq!(ai.air_surge_research_goal(&g, 0), Some(AIR_SURGE_GOAL_TECH));
    assert_eq!(ai.air_surge_cooldown_until, 0);
    assert_eq!(ai.air_surge_census.objectives_captured, 0);
}

#[test]
fn a_captured_counter_objective_is_counted_once_before_the_wing_moves_on() {
    let (mut g, mut ai) = fixture();
    let old_city = ai.air_surge_plan.as_ref().unwrap().objective_city;
    let replacement = g.found_city_for(1, (16, 17), None);
    g.cities.get_mut(&old_city).unwrap().owner = 0;
    g.at_war.insert((0, 1));
    ai.air_surge_plan.as_mut().unwrap().opened_at_war = true;
    ai.maintain_air_surge(&g, 0);
    ai.maintain_air_surge(&g, 0);
    assert_eq!(
        ai.air_surge_plan.as_ref().unwrap().objective_city,
        replacement
    );
    assert_eq!(ai.air_surge_census.objectives_captured, 1);
}

#[test]
fn a_changed_owner_still_ends_the_plan_when_no_counter_objective_is_available() {
    let (mut g, mut ai) = fixture();
    let old_city = ai.air_surge_plan.as_ref().unwrap().objective_city;
    g.cities.get_mut(&old_city).unwrap().owner = 3;
    g.at_war.insert((0, 1));
    ai.air_surge_plan.as_mut().unwrap().opened_at_war = true;
    ai.maintain_air_surge(&g, 0);
    assert!(ai.air_surge_plan.is_none());
    assert_eq!(ai.air_surge_census.aborts["objective changed owner"], 1);
}
