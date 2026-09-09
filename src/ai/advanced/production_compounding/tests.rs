use super::*;
use crate::ai::advanced::{StrategicPlan, VictoryTarget};
use crate::game::{install_test_district, Action};

fn workshop_fixture() -> (Game, u32, Item) {
    let mut game = Game::new(2, 32, 24, 5_417, 250, 0);
    game.game_speed = crate::setup::GameSpeed::Online;
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = game.player_city_ids(0)[0];
    game.players[0].techs.insert(crate::name!("apprenticeship"));
    game.cities.get_mut(&cid).unwrap().pop = 6;
    install_test_district(&mut game, cid, "industrial_zone");
    let item = Item::Building {
        building: crate::name!("workshop"),
    };
    assert!(game.can_produce(0, cid, &item));
    (game, cid, item)
}

fn horizon(ai: &AdvancedAi, game: &Game, cid: u32, item: &Item, regional: f64) -> f64 {
    ai.industrial_investment_horizon(
        game,
        0,
        cid,
        item,
        &game.rules.buildings["workshop"],
        regional,
        GrandStrategy::Science,
    )
}

#[test]
fn profitable_workshop_keeps_full_priority_in_every_named_lane() {
    let (mut game, cid, item) = workshop_fixture();
    game.turn = 180;
    let template = AdvancedAi::targeting(VictoryTarget::Science);
    let build = template.production_build_turns(&game, 0, cid, &item);
    let payback = game.item_remaining_cost_for_city(0, cid, &item) / 3.0;
    game.max_turns = game.turn + (build + payback).ceil() as u32 + 1;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: game.turn,
        rush: false,
    };
    for target in VictoryTarget::ALL {
        let mut named = AdvancedAi::targeting(target);
        named.chain_payback_window = false;
        named.chain_payback_window_2 = false;
        let mut adaptive = named.clone();
        adaptive.victory_target = None;
        adaptive.enable_industrial_chain_debt();
        assert_eq!(horizon(&named, &game, cid, &item, 0.0), 1.0);
        let counts = named.counts(&game, 0);
        assert!(
            named.production_value(&game, 0, cid, &item, &plan, &counts)
                > adaptive.production_value(&game, 0, cid, &item, &plan, &counts),
            "{} must retain its profitable production premium",
            target.as_str()
        );
    }
}

#[test]
fn completion_time_and_remaining_clock_limit_the_investment() {
    let (mut game, cid, item) = workshop_fixture();
    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    let build = ai.production_build_turns(&game, 0, cid, &item);
    game.turn = 200;
    game.max_turns = game.turn + build.floor() as u32;
    assert_eq!(horizon(&ai, &game, cid, &item, 0.0), 0.0);
    game.max_turns += 5;
    let partial = horizon(&ai, &game, cid, &item, 0.0);
    assert!(partial > 0.0 && partial < 1.0, "{partial}");
    game.turn = game.max_turns;
    assert_eq!(horizon(&ai, &game, cid, &item, 0.0), 0.0);
}

#[test]
fn regional_production_shortens_payback() {
    let (mut game, cid, item) = workshop_fixture();
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    let build = ai.production_build_turns(&game, 0, cid, &item);
    game.turn = 200;
    game.max_turns = game.turn + build.ceil() as u32 + 8;
    let local = horizon(&ai, &game, cid, &item, 0.0);
    // Existing reach already discounts overlap and uncertain remote yields.
    let regional = ai.yield_value(
        Yields {
            production: 3.0,
            ..Yields::default()
        },
        GrandStrategy::Science,
    ) * 42.0;
    assert!(horizon(&ai, &game, cid, &item, regional) > local);
}

#[test]
fn unlimited_games_keep_investing_after_the_stock_turn_limit() {
    let (mut game, cid, item) = workshop_fixture();
    game.max_turns = 0;
    game.turn = 900;
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    assert!(horizon(&ai, &game, cid, &item, 0.0) > 0.0);
}

#[test]
fn science_reservation_actually_queues_the_winning_production_investment() {
    let (mut game, cid, workshop) = workshop_fixture();
    install_test_district(&mut game, cid, "campus");
    game.players[0].techs.insert(crate::name!("writing"));
    game.turn = 100;
    let library = Item::Building {
        building: crate::name!("library"),
    };
    assert!(game.can_produce(0, cid, &library));
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: game.turn,
        rush: false,
    };
    let investment = game.item_cost_for_city(0, cid, &workshop) - 1.0;
    game.cities
        .get_mut(&cid)
        .unwrap()
        .production_progress
        .insert("building:workshop".to_string(), investment);
    assert_eq!(
        ai.research_or_industrial_foundation(&game, 0, cid, library.clone(), &plan),
        workshop
    );
    ai.reserve_targeted_research_buildings(&mut game, 0, &plan);
    assert_eq!(
        game.cities[&cid].queue.first(),
        Some(&workshop),
        "the early reservation must execute the production scorer's decision"
    );

    // Once the industrial debt is paid, the same reservation returns to the
    // Library. It never substitutes an unrelated district or repeatable project.
    let city = game.cities.get_mut(&cid).unwrap();
    city.queue.clear();
    city.buildings.push(crate::name!("workshop"));
    city.production_progress.clear();
    ai.reserve_targeted_research_buildings(&mut game, 0, &plan);
    assert_eq!(game.cities[&cid].queue.first(), Some(&library));
}

#[test]
fn adaptive_research_reservation_retains_its_screened_policy() {
    let (mut game, cid, workshop) = workshop_fixture();
    install_test_district(&mut game, cid, "campus");
    game.players[0].techs.insert(crate::name!("writing"));
    let library = Item::Building {
        building: crate::name!("library"),
    };
    let investment = game.item_cost_for_city(0, cid, &workshop) - 1.0;
    game.cities
        .get_mut(&cid)
        .unwrap()
        .production_progress
        .insert("building:workshop".to_string(), investment);
    let ai = AdvancedAi::new();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: game.turn,
        rush: false,
    };
    assert_eq!(
        ai.research_or_industrial_foundation(&game, 0, cid, library.clone(), &plan),
        library
    );
}

#[test]
fn repayable_workshop_precedes_discretionary_spy_in_the_real_queue() {
    let (mut game, cid, workshop) = workshop_fixture();
    game.turn = 100;
    game.players[0]
        .civics
        .insert(crate::name!("diplomatic_service"));
    let spy = Item::Unit {
        unit: crate::name!("spy"),
    };
    assert!(game.can_produce(0, cid, &spy));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Culture,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    };
    let counts = ai.counts(&game, 0);
    assert!(
        ai.production_value(&game, 0, cid, &spy, &plan, &counts)
            > ai.production_value(&game, 0, cid, &workshop, &plan, &counts),
        "reproduce the old discretionary score winning"
    );
    ai.advanced_production(&mut game, 0, &plan, false);
    assert_eq!(
        game.cities[&cid].queue.first(),
        Some(&workshop),
        "the actual queue must invest instead of merely raising a score"
    );
}

#[test]
fn every_named_lane_reserves_only_a_safe_repayable_idle_investment() {
    let (mut game, cid, workshop) = workshop_fixture();
    game.turn = 100;
    for target in VictoryTarget::ALL {
        let ai = AdvancedAi::targeting(target);
        let mut plan = StrategicPlan {
            strategy: target.strategy(),
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 1,
            assessed_turn: game.turn,
            rush: false,
        };
        assert_eq!(
            ai.profitable_industrial_foundation(&game, 0, cid, &plan),
            Some(workshop.clone())
        );
        plan.threatened_city = Some(cid);
        assert!(ai
            .profitable_industrial_foundation(&game, 0, cid, &plan)
            .is_none());
        plan.threatened_city = None;
        game.cities.get_mut(&cid).unwrap().last_attacked = game.turn;
        assert!(ai
            .profitable_industrial_foundation(&game, 0, cid, &plan)
            .is_none());
        game.cities.get_mut(&cid).unwrap().last_attacked = 0;
        game.cities.get_mut(&cid).unwrap().queue.push(Item::Unit {
            unit: crate::name!("warrior"),
        });
        assert!(ai
            .profitable_industrial_foundation(&game, 0, cid, &plan)
            .is_none());
        game.cities.get_mut(&cid).unwrap().queue.clear();
        game.max_turns = game.turn + 1;
        assert!(ai
            .profitable_industrial_foundation(&game, 0, cid, &plan)
            .is_none());
        game.max_turns = 250;
    }
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    install_test_district(&mut game, cid, "spaceport");
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    };
    assert!(
        ai.profitable_industrial_foundation(&game, 0, cid, &plan)
            .is_none(),
        "a launch city's project priority is preserved"
    );
}
