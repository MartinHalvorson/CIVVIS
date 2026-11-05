use super::*;
use crate::ai::industrial_zones::tests::producer_empire;
use crate::game::install_test_district;
use crate::setup::GameSpeed;
use std::sync::Arc;

fn building(name: &str) -> Item {
    Item::Building {
        building: Name::new(name),
    }
}

fn fixture() -> (Game, u32, AdvancedAi, StrategicPlan) {
    let (mut game, ranked) = producer_empire("UNIVERSITYFORGE", 94_301, 1);
    game.players[0].techs.insert(crate::name!("education"));
    game.players[0].techs.insert(crate::name!("mathematics"));
    let city = ranked[0];
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_universities_before_the_forge();
    ai.enable_industrial_zone_claims_the_producer_queue();
    ai.enable_industrial_zone_claims_the_factory();
    let mut plan = ai.assess(&game, 0);
    plan.strategy = GrandStrategy::Science;
    plan.threatened_city = None;
    (game, city, ai, plan)
}

#[test]
fn opt_in_is_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "universities-before-the-forge",
        |ai| ai.universities_before_the_forge,
    );
}

#[test]
fn idle_science_city_releases_zone_claim_but_flag_off_keeps_it() {
    let (game, city, mut ai, plan) = fixture();
    assert!(ai.university_precedes_industry(&game, 0, city, &plan));
    assert!(ai.producer_zone_claim_item(&game, 0, city, &plan).is_none());
    ai.disable_universities_before_the_forge();
    assert!(!ai.university_precedes_industry(&game, 0, city, &plan));
    assert!(ai.producer_zone_claim_item(&game, 0, city, &plan).is_some());
}

#[test]
fn an_existing_workshop_claim_can_yield_without_disabling_industry() {
    let (mut game, city, ai, plan) = fixture();
    install_test_district(&mut game, city, "industrial_zone");
    let workshop = building("workshop");
    game.cities.get_mut(&city).unwrap().queue = vec![workshop.clone()];
    assert!(ai.university_precedes_industry(&game, 0, city, &plan));
    assert!(!ai.producer_zone_claim_holds(&game, 0, city, &plan, &workshop));
}

#[test]
fn factory_claim_keeps_its_existing_priority() {
    let (mut game, city, mut ai, plan) = fixture();
    game.players[0]
        .techs
        .insert(crate::name!("industrialization"));
    install_test_district(&mut game, city, "industrial_zone");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("workshop"));
    assert!(ai.factory_claim_item(&game, 0, city, &plan).is_some());
    let factory = building("factory");
    game.cities.get_mut(&city).unwrap().queue = vec![factory.clone()];
    assert!(ai.factory_claim_holds(&game, 0, city, &plan, &factory));
    ai.disable_universities_before_the_forge();
    assert!(ai.factory_claim_holds(&game, 0, city, &plan, &factory));
}

#[test]
fn threat_recent_attack_and_recovery_leave_old_claim_rules_unchanged() {
    let (mut game, city, ai, mut plan) = fixture();
    plan.threatened_city = Some(city);
    assert!(!ai.university_precedes_industry(&game, 0, city, &plan));
    plan.threatened_city = None;
    plan.strategy = GrandStrategy::Recovery;
    assert!(!ai.university_precedes_industry(&game, 0, city, &plan));
    plan.strategy = GrandStrategy::Science;
    game.cities.get_mut(&city).unwrap().last_attacked = game.turn - 1;
    assert!(!ai.university_precedes_industry(&game, 0, city, &plan));
}

#[test]
fn rocket_age_and_other_victory_targets_keep_industrial_priority() {
    let (mut game, city, ai, plan) = fixture();
    game.players[0].techs.insert(crate::name!("rocketry"));
    assert!(!ai.university_precedes_industry(&game, 0, city, &plan));
    game.players[0].techs.remove(&crate::name!("rocketry"));
    let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
    culture.enable_universities_before_the_forge();
    assert!(!culture.university_precedes_industry(&game, 0, city, &plan));
}

#[test]
fn completed_or_illegal_university_does_not_release_industry() {
    let (mut game, city, ai, plan) = fixture();
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("university"));
    assert!(!ai.university_precedes_industry(&game, 0, city, &plan));
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .retain(|b| *b != "university");
    game.players[0].techs.remove(&crate::name!("education"));
    assert!(!ai.university_precedes_industry(&game, 0, city, &plan));
}

#[test]
fn military_defense_project_and_science_heads_are_not_reconsidered() {
    let (mut game, city, ai, plan) = fixture();
    for item in [
        Item::Unit {
            unit: crate::name!("archer"),
        },
        building("walls"),
        building("library"),
        Item::Project {
            project: crate::name!("campus_research_grants"),
        },
    ] {
        game.cities.get_mut(&city).unwrap().queue = vec![item];
        assert!(!ai.university_precedes_industry(&game, 0, city, &plan));
    }
}

#[test]
fn a_one_turn_industrial_head_finishes_first() {
    let (mut game, city, ai, plan) = fixture();
    install_test_district(&mut game, city, "industrial_zone");
    let workshop = building("workshop");
    let cost = game.item_cost_for_city(0, city, &workshop);
    game.cities.get_mut(&city).unwrap().queue = vec![workshop.clone()];
    game.cities.get_mut(&city).unwrap().production = cost - 1.0;
    assert!(ai.production_build_turns(&game, 0, city, &workshop) <= 1.0);
    assert!(!ai.university_precedes_industry(&game, 0, city, &plan));
}

#[test]
fn host_quote_controls_the_limit_and_game_speed_scales_it() {
    let (mut game, city, ai, plan) = fixture();
    let university = building("university");
    for (speed, turns, expected) in [
        (GameSpeed::Online, 12.0, true),
        (GameSpeed::Online, 13.0, false),
        (GameSpeed::Standard, 24.0, true),
        (GameSpeed::Standard, 25.0, false),
        (GameSpeed::Online, f64::NAN, false),
    ] {
        game.game_speed = speed;
        Arc::make_mut(&mut game.host_buildable)
            .entry(city)
            .or_default()
            .insert(
                Game::production_block_key(&university),
                crate::game::HostMenuEntry {
                    cost: Some(125.0),
                    turns: Some(turns),
                },
            );
        assert_eq!(
            ai.university_precedes_industry(&game, 0, city, &plan),
            expected
        );
    }
}
