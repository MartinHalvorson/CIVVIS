//! Research priority and context regressions for the first-capital army.
use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

fn fixture(on: bool) -> (Game, AdvancedAi, StrategicPlan) {
    let mut g = Game::new_full(2, 48, 30, 50_580, 250, 0, false);
    g.players[0].civ = "Macedon".to_string();
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
    }
    for pos in [(8, 8), (8, 16), (14, 16)] {
        g.found_city_for(0, pos, None);
    }
    g.found_city_for(1, (22, 8), None);
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    g.players[0].techs.extend([
        crate::name!("mining"),
        crate::name!("masonry"),
        crate::name!("animal_husbandry"),
        crate::name!("archery"),
    ]);
    g.players[0].research = None;
    g.game_speed = crate::setup::GameSpeed::Online;
    g.turn = 25;
    g.current = 0;
    g.at_war.clear();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.skip_opening_book();
    ai.enable_early_conquest_opening();
    ai.enable_first_capital_before_the_walls();
    ai.enable_city_state_before_the_first_capital();
    // The prior stepping stone has ended, as Nalanda did when its suzerain appeared.
    ai.city_state_stone_named = true;
    if on {
        ai.enable_research_follows_the_first_capital();
    }
    ai.maintain_conquest_opening(&mut g, 0);
    assert!(ai.conquest_opening.is_some());
    assert!(ai.city_state_stone_research_goal(&g, 0).is_none());
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan)
}

#[test]
fn research_follows_the_first_capital_defaults_off_in_both_controllers() {
    opt_in_off_in_both_controllers("research-follows-the-first-capital", |ai| {
        ai.research_follows_the_first_capital
    });
}

#[test]
fn the_named_capital_keeps_the_unique_melee_beeline_after_the_stone_ends() {
    for on in [false, true] {
        let (mut g, ai, plan) = fixture(on);
        ai.advanced_research(&mut g, 0, &plan);
        assert_eq!(
            g.players[0].research,
            Some(if on {
                crate::name!("bronze_working")
            } else {
                crate::name!("pottery")
            })
        );
    }
}

#[test]
fn bronze_working_then_iron_working_then_the_economic_goal() {
    let (mut g, ai, plan) = fixture(true);
    g.players[0].techs.insert(crate::name!("bronze_working"));
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research, Some(crate::name!("iron_working")));
    g.players[0].techs.insert(crate::name!("iron_working"));
    g.players[0].research = None;
    assert_eq!(ai.first_capital_army_research_goal(&g, 0), None);
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research, Some(crate::name!("pottery")));
}

#[test]
fn the_breakers_technology_still_precedes_the_melee_line() {
    let (mut g, ai, plan) = fixture(true);
    g.players[0].techs.remove(&crate::name!("masonry"));
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research, Some(crate::name!("masonry")));
}

#[test]
fn a_technology_already_in_flight_finishes() {
    let (mut g, ai, plan) = fixture(true);
    g.players[0].research = Some(crate::name!("sailing"));
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research, Some(crate::name!("sailing")));
}

#[test]
fn the_goal_requires_the_named_prewar_major_capital_context() {
    let (g, ai, _) = fixture(true);
    assert_eq!(
        ai.first_capital_army_research_goal(&g, 0),
        Some(crate::name!("iron_working"))
    );
    for fence in [
        "culture",
        "no parent",
        "no opening",
        "declared",
        "closed",
        "minor",
        "taken",
    ] {
        let (mut g, mut ai, _) = fixture(true);
        match fence {
            "culture" => ai.victory_target = Some(VictoryTarget::Culture),
            "no parent" => ai.disable_first_capital_before_the_walls(),
            "no opening" => ai.conquest_opening = None,
            "declared" => ai.conquest_opening.as_mut().unwrap().declared = Some(g.turn),
            "closed" => ai.conquest_closed = true,
            "minor" => g.players[1].is_minor = true,
            "taken" => {
                let city = ai.conquest_opening.as_ref().unwrap().city;
                g.cities.get_mut(&city).unwrap().owner = 0;
            }
            _ => unreachable!(),
        }
        assert_eq!(ai.first_capital_army_research_goal(&g, 0), None, "{fence}");
    }
}

#[test]
fn the_immediate_archery_defender_keeps_its_priority() {
    let (mut g, mut ai, plan) = fixture(true);
    g.players[0].techs.remove(&crate::name!("archery"));
    ai.base.garrison_under_fire = true;
    ai.enable_early_archers();
    let barb = g
        .barb_pid
        .expect("the native-style board has a barbarian seat");
    g.spawn_test_unit("warrior", barb, (9, 8));
    assert_eq!(
        ai.first_capital_army_research_goal(&g, 0),
        Some(crate::name!("iron_working"))
    );
    assert_eq!(
        ai.opening_archery_goal(&g, 0),
        Some(crate::name!("archery"))
    );
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research, Some(crate::name!("archery")));
}
