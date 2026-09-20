use super::super::air_surge::AirSurgePhase;
use super::super::*;
use crate::name;

fn board() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut g = Game::new_full(2, 40, 24, 936031, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (6, 12), None);
    let target = g.found_city_for(1, (25, 12), None);
    g.players[0].techs.extend(
        g.rules
            .techs
            .iter()
            .filter(|(_, spec)| spec.era < 3)
            .map(|(tech, _)| *tech),
    );
    g.players[0].techs.extend(
        g.rules.tech_ancestors["combined_arms"]
            .iter()
            .map(|tech| Name::new(tech)),
    );
    g.current = 0;
    g.turn = 190;
    g.at_war.clear();
    g.at_war.insert((0, 1));
    g.record_contact(0, 1);
    let uid = g.spawn_test_unit("giant_death_robot", 0, (6, 12));
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, uid)
}

#[test]
fn a_single_granted_robot_reveals_its_fuel_despite_older_era_backlog() {
    let (mut g, ai, plan, _) = board();
    assert!(g.available_techs(0).contains(&name!("combined_arms")));
    assert!(!BasicAi::era_window_techs(&g, 0).contains(&name!("combined_arms")));
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("combined_arms"));
}

#[test]
fn fuel_research_walks_legal_prerequisites_instead_of_skipping_them() {
    let (mut g, ai, plan, _) = board();
    g.players[0].techs.remove(&name!("combustion"));
    assert!(!g.available_techs(0).contains(&name!("combined_arms")));
    assert!(g.available_techs(0).contains(&name!("combustion")));
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("combustion"));
}

#[test]
fn fuel_demand_does_not_cancel_research_already_in_progress() {
    let (mut g, ai, plan, _) = board();
    g.players[0].research = Some("printing".into());
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("printing"));
}

#[test]
fn fuel_priority_requires_exhausted_unrevealed_demand_in_a_domination_war() {
    for case in 0..9 {
        let (mut g, mut ai, _, uid) = board();
        assert_eq!(
            ai.standing_army_fuel_goal(&g, 0),
            Some(name!("combined_arms"))
        );
        match case {
            0 => {
                g.remove_unit(uid);
            }
            1 => {
                g.units.get_mut(&uid).unwrap().free_upkeep = true;
            }
            2 => {
                g.players[0]
                    .strategic_resources
                    .insert(name!("uranium"), 1.0);
            }
            3 => {
                g.players[0].techs.insert(name!("combined_arms"));
            }
            4 => {
                ai.victory_target = Some(VictoryTarget::Science);
            }
            5 => {
                g.at_war.clear();
            }
            6 => {
                ai.victory_planning = false;
            }
            7 => {
                g.victory_conditions.domination = false;
            }
            _ => {
                g.units.get_mut(&uid).unwrap().owner = 1;
            }
        }
        assert_eq!(ai.standing_army_fuel_goal(&g, 0), None, "case={case}");
    }
}

#[test]
fn construction_resources_and_queued_units_are_not_standing_fuel_demand() {
    let (mut g, ai, _, uid) = board();
    g.remove_unit(uid);
    g.players[0].techs.remove(&name!("bronze_working"));
    g.spawn_test_unit("swordsman", 0, (6, 12));
    let home = g.player_city_ids(0)[0];
    g.cities.get_mut(&home).unwrap().queue.push(Item::Unit {
        unit: name!("giant_death_robot"),
    });
    assert_eq!(ai.standing_army_fuel_goal(&g, 0), None);
}

#[test]
fn fuel_priority_aggregates_the_fielded_army_and_stops_when_it_is_supplied() {
    let (mut g, ai, _, _) = board();
    g.players[0].techs.remove(&name!("refining"));
    g.spawn_test_unit("infantry", 0, (7, 12));
    assert_eq!(
        ai.standing_army_fuel_goal(&g, 0),
        Some(name!("combined_arms"))
    );
    g.spawn_test_unit("infantry", 0, (8, 12));
    assert_eq!(ai.standing_army_fuel_goal(&g, 0), Some(name!("refining")));
    g.players[0].strategic_resources.insert(name!("oil"), 1.0);
    assert_eq!(
        ai.standing_army_fuel_goal(&g, 0),
        Some(name!("combined_arms"))
    );
}

#[test]
fn an_appointed_land_breakthrough_keeps_priority_over_fuel_revelation() {
    let (mut g, mut ai, plan, _) = board();
    g.players[0].techs.remove(&name!("metal_casting"));
    ai.war_plan = Some(WarPlan {
        target_player: 1,
        objective_city: plan.target_city.unwrap(),
        breakthrough_tech: name!("metal_casting"),
        assault_unit: name!("bombard"),
        predecessor: None,
        breach_unit: None,
        estimated_research_turns: 1,
        estimated_production_turns: 3,
        estimated_upgrade_gold: 0.0,
        estimated_march_turns: 2,
        phase: WarPhase::Research,
        appointed_turn: 130,
        tech_turn: None,
        declared_turn: None,
        last_reviewed_turn: 190,
        recovery_assessments: 0,
    });
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("metal_casting"));
}

#[test]
fn an_appointed_air_breakthrough_keeps_priority_over_fuel_revelation() {
    let (mut g, mut ai, plan, _) = board();
    g.players[0].techs.extend(
        g.rules.tech_ancestors["advanced_flight"]
            .iter()
            .map(|tech| Name::new(tech)),
    );
    ai.air_surge_plan = Some(AirSurge {
        target_player: 1,
        objective_city: plan.target_city.unwrap(),
        objective_pos: (25, 12),
        body_unit: name!("knight"),
        body_is_cavalry: true,
        opened_at_war: true,
        phase: AirSurgePhase::Beeline,
        appointed_turn: 180,
        tech_turn: None,
        declared_turn: None,
        last_reviewed_turn: 190,
        recovery_assessments: 0,
    });
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("advanced_flight"));
}
