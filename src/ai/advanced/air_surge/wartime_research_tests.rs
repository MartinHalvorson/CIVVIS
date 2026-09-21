use super::*;

fn fixture() -> (Game, AdvancedAi, StrategicPlan) {
    let mut g = Game::new_full(2, 40, 24, 936031, 500, 0, false);
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
    g.record_contact(0, 1);
    g.players[0].techs.extend(
        g.rules
            .techs
            .iter()
            .filter(|(_, spec)| spec.era < 3)
            .map(|(tech, _)| *tech),
    );
    g.players[0].techs.extend(
        g.rules.tech_ancestors[AIR_SURGE_GOAL_TECH]
            .iter()
            .map(|tech| Name::new(tech)),
    );
    for tech in ["flight", "radio", "steam_power"] {
        g.players[0].techs.remove(&Name::new(tech));
    }
    g.current = 0;
    g.turn = 121;
    g.at_war.clear();
    g.at_war.insert((0, 1));
    for pos in [(12, 13), (13, 12)] {
        g.spawn_test_unit("crossbowman", 0, pos);
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    ai.air_surge_plan = Some(AirSurge {
        target_player: 1,
        objective_city: target,
        objective_pos: (18, 12),
        body_unit: crate::name!("knight"),
        body_is_cavalry: true,
        opened_at_war: true,
        phase: AirSurgePhase::Exploit,
        appointed_turn: 102,
        tech_turn: None,
        declared_turn: None,
        last_reviewed_turn: 121,
        recovery_assessments: 0,
    });
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: 121,
        rush: false,
    };
    (g, ai, plan)
}

#[test]
fn cheaper_wartime_upgrade_interrupts_distant_air_research_and_then_releases_it() {
    let (mut g, ai, plan) = fixture();
    assert_eq!(
        ai.wartime_modernization_tech(&g, 0),
        Some(crate::name!("ballistics"))
    );
    assert!(
        AdvancedAi::war_remaining_research_cost(&g, 0, crate::name!("ballistics"))
            < AdvancedAi::war_remaining_research_cost(&g, 0, Name::new(AIR_SURGE_GOAL_TECH))
    );
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("gunpowder"));
    assert_eq!(ai.air_surge_plan.as_ref().unwrap().appointed_turn, 102);

    // Unlocking the immediate successor releases the interruption even if
    // production or Gold has not yet upgraded either Crossbowman.
    g.players[0].techs.extend([
        crate::name!("gunpowder"),
        crate::name!("metal_casting"),
        crate::name!("ballistics"),
    ]);
    g.players[0].research = None;
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("steam_power"));
}

#[test]
fn air_research_keeps_priority_without_a_nearer_major_war_upgrade() {
    for case in ["peace", "minor", "single", "final_air_tech", "legacy"] {
        let (mut g, mut ai, plan) = fixture();
        match case {
            "peace" => g.at_war.clear(),
            "minor" => g.players[1].is_minor = true,
            "single" => {
                let uid = g.player_unit_ids(0)[0];
                g.remove_unit(uid);
            }
            "final_air_tech" => {
                g.players[0].techs.extend([
                    crate::name!("steam_power"),
                    crate::name!("flight"),
                    crate::name!("radio"),
                ]);
            }
            "legacy" => ai.disable_air_surge_2(),
            _ => unreachable!(),
        }
        ai.advanced_research(&mut g, 0, &plan);
        let expected = if case == "final_air_tech" {
            "advanced_flight"
        } else {
            "steam_power"
        };
        assert_eq!(g.players[0].research.as_deref(), Some(expected), "{case}");
    }
}

#[test]
fn a_more_expensive_ground_upgrade_does_not_delay_the_air_campaign() {
    let (mut g, ai, plan) = fixture();
    for uid in g.player_unit_ids(0) {
        g.remove_unit(uid);
    }
    for pos in [(12, 13), (13, 12)] {
        g.spawn_test_unit("infantry", 0, pos);
    }
    let upgrade = ai.wartime_modernization_tech(&g, 0).unwrap();
    assert!(
        AdvancedAi::war_remaining_research_cost(&g, 0, upgrade)
            > AdvancedAi::war_remaining_research_cost(&g, 0, Name::new(AIR_SURGE_GOAL_TECH))
    );
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("steam_power"));
}
