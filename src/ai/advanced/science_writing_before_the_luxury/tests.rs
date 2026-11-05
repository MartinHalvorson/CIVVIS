use super::*;
use crate::ai::advanced::{GrandStrategy, StrategicPlan};
use crate::name::Name;

fn opening() -> (Game, AdvancedAi, StrategicPlan) {
    let mut g = Game::new_full(2, 40, 24, 936012, 500, 0, false);
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
    g.players[0].civ = "Korea".to_string();
    let capital = g.found_city_for(0, (6, 12), None);
    g.found_city_for(0, (12, 12), None);
    g.found_city_for(1, (25, 12), None);
    g.spawn_test_unit("warrior", 0, (6, 12));
    g.spawn_test_unit("archer", 0, (12, 12));
    g.spawn_test_unit("builder", 0, (6, 12));
    g.players[0]
        .techs
        .extend(["mining", "animal_husbandry", "archery", "pottery"].map(Name::new));
    let coffee = g.cities[&capital]
        .owned_tiles
        .iter()
        .copied()
        .find(|p| *p != g.cities[&capital].pos)
        .unwrap();
    g.map.tiles.get_mut(&coffee).unwrap().resource = Some(crate::name!("coffee"));
    g.at_war.clear();
    g.current = 0;
    g.turn = 24;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_connect_the_luxury();
    ai.enable_campus_unlock_every_lane();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 8,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan)
}

#[test]
fn science_finishes_writing_before_an_owned_luxury_connection() {
    let pick = |on: bool| {
        let (mut g, mut ai, plan) = opening();
        assert_eq!(ai.unconnected_luxury_tech(&g, 0), Some("irrigation"));
        assert_eq!(ai.every_lane_campus_unlock_goal(&g, 0), Some("writing"));
        if on {
            ai.enable_science_writing_before_the_luxury();
        }
        ai.advanced_research(&mut g, 0, &plan);
        g.players[0].research.clone()
    };
    assert_eq!(pick(false).as_deref(), Some("irrigation"));
    assert_eq!(pick(true).as_deref(), Some("writing"));
}

#[test]
fn the_goal_is_bounded_to_an_enabled_science_seat_with_two_cities_and_writing_debt() {
    let (mut g, mut ai, _) = opening();
    assert_eq!(ai.science_writing_before_the_luxury_goal(&g, 0), None);
    ai.enable_science_writing_before_the_luxury();
    assert_eq!(
        ai.science_writing_before_the_luxury_goal(&g, 0),
        Some("writing")
    );
    let second = g.player_city_ids(0)[1];
    g.cities.get_mut(&second).unwrap().owner = 1;
    assert_eq!(ai.science_writing_before_the_luxury_goal(&g, 0), None);
    g.cities.get_mut(&second).unwrap().owner = 0;
    g.players[0].techs.insert(crate::name!("writing"));
    assert_eq!(ai.science_writing_before_the_luxury_goal(&g, 0), None);
    g.players[0].techs.remove(&crate::name!("writing"));
    g.victory_conditions.science = false;
    assert_eq!(ai.science_writing_before_the_luxury_goal(&g, 0), None);
}

#[test]
fn other_victory_lanes_keep_their_research_choices() {
    for target in [
        VictoryTarget::Domination,
        VictoryTarget::Culture,
        VictoryTarget::Religion,
        VictoryTarget::Diplomacy,
    ] {
        let pick = |on: bool| {
            let (mut g, mut ai, plan) = opening();
            ai.victory_target = Some(target);
            if on {
                ai.enable_science_writing_before_the_luxury();
            }
            assert_eq!(ai.science_writing_before_the_luxury_goal(&g, 0), None);
            ai.advanced_research(&mut g, 0, &plan);
            g.players[0].research.clone()
        };
        assert_eq!(pick(false), pick(true), "{target:?}");
    }
}

#[test]
fn a_missing_pottery_prerequisite_is_researched_before_writing() {
    let (mut g, mut ai, plan) = opening();
    g.players[0].techs.remove(&crate::name!("pottery"));
    ai.enable_science_writing_before_the_luxury();
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("pottery"));
}

#[test]
fn the_first_ranged_defender_keeps_priority() {
    let (mut g, mut ai, plan) = opening();
    ai.enable_science_writing_before_the_luxury();
    g.players[0].techs.remove(&crate::name!("archery"));
    for uid in g
        .units
        .values()
        .filter(|u| u.owner == 0 && u.kind == "archer")
        .map(|u| u.id)
        .collect::<Vec<_>>()
    {
        g.remove_unit(uid);
    }
    g.spawn_test_unit("slinger", 0, (6, 12));
    ai.base.garrison_under_fire = true;
    assert_eq!(ai.opening_archery_goal(&g, 0).as_deref(), Some("archery"));
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("archery"));
}

#[test]
fn a_rush_keeps_its_military_unlock_first() {
    let (mut g, mut ai, mut plan) = opening();
    ai.enable_science_writing_before_the_luxury();
    plan.rush = true;
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("horseback_riding"));
}

#[test]
fn ongoing_research_is_not_cancelled() {
    let (mut g, mut ai, plan) = opening();
    ai.enable_science_writing_before_the_luxury();
    g.players[0].research = Some("bronze_working".to_string());
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("bronze_working"));
}

#[test]
fn the_native_opt_in_ships_off_in_both_controllers() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "science-writing-before-the-luxury",
        |ai| ai.science_writing_before_the_luxury,
    );
}
