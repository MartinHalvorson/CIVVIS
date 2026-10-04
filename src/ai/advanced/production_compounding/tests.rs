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

fn district_fixture() -> (Game, u32, u32, AdvancedAi, StrategicPlan) {
    let (mut game, cid, _) = workshop_fixture();
    game.turn = 50;
    game.max_turns = 150;
    let old = game
        .cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .remove(crate::name!("industrial_zone"))
        .unwrap()[0];
    game.map.tiles.get_mut(&old).unwrap().district = None;
    game.players[0].techs.insert(crate::name!("mining"));
    game.players[0].gold_per_turn = 20.0;
    std::sync::Arc::make_mut(&mut game.observed_city_amenity_adjustments).insert(cid, 10);
    let center = game.cities[&cid].pos;
    let site = game.district_sites(cid, crate::name!("industrial_zone"))[0];
    let tile = game.map.tiles.get_mut(&site).unwrap();
    tile.terrain = crate::name!("grassland");
    tile.hills = false;
    tile.feature = None;
    tile.resource = None;
    tile.improvement = None;
    for pos in game.nbrs(site) {
        if pos == center {
            continue;
        }
        let tile = game.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = None;
        tile.resource = Some(crate::name!("stone"));
        tile.improvement = Some(crate::name!("quarry"));
    }
    let second_pos = game
        .map
        .tiles
        .values()
        .find(|tile| {
            !game.rules.is_water(tile)
                && tile.owner_city.is_none()
                && game.wdist(center, tile.pos) >= 6
                && game.unit_ids_at(tile.pos).is_empty()
        })
        .unwrap()
        .pos;
    let settler = game.spawn_unit("settler", 0, second_pos);
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let second = game
        .player_city_ids(0)
        .into_iter()
        .find(|id| *id != cid)
        .unwrap();
    game.spawn_unit("warrior", 0, second_pos);
    game.spawn_unit("builder", 0, center);
    game.spawn_unit("trader", 0, center);
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    assert!(ai
        .profitable_industrial_foundation(&game, 0, cid, &plan)
        .is_some());
    (game, cid, second, ai, plan)
}

#[test]
fn domination_opens_repayable_industry_in_the_real_queue() {
    let (mut game, cid, _, mut ai, plan) = district_fixture();
    ai.advanced_production(&mut game, 0, &plan, false);
    assert!(matches!(game.cities[&cid].queue.first(),
        Some(Item::District { district, .. }) if game.district_family(*district) == "industrial_zone"));
}

#[test]
fn industrial_district_prices_the_workshop_and_protects_other_priorities() {
    let (mut game, cid, _, ai, mut plan) = district_fixture();
    let item = ai
        .profitable_industrial_foundation(&game, 0, cid, &plan)
        .unwrap();
    let Item::District { district, pos } = item else {
        panic!("fixture must open a district");
    };
    let adjacency = game
        .district_adjacency_assuming(district, pos, None, None)
        .production;
    let worked = game.city_citizen_plan(cid).worked_tiles.contains(&pos);
    let displaced = if worked {
        game.workable_tile_yields(pos).production
    } else {
        0.0
    };
    let gain = adjacency - displaced;
    let district_cost = game.item_remaining_cost_for_city(0, cid, &item);
    let build = ai.production_build_turns(&game, 0, cid, &item);
    game.max_turns = game.turn + (build + district_cost / gain).ceil() as u32 + 1;
    assert!((game.max_turns - game.turn) as f64 >= build + district_cost / gain);
    assert!(
        ai.profitable_industrial_foundation(&game, 0, cid, &plan)
            .is_none(),
        "district-only payback must not hide the cost of the Workshop"
    );
    game.max_turns = 150;
    plan.threatened_city = Some(cid);
    assert!(ai
        .profitable_industrial_foundation(&game, 0, cid, &plan)
        .is_none());
    plan.threatened_city = None;
    for target in [VictoryTarget::Science, VictoryTarget::Culture] {
        assert!(AdvancedAi::targeting(target)
            .profitable_industrial_foundation(&game, 0, cid, &plan)
            .is_none());
    }
    game.players[0].gold_per_turn = 0.0;
    assert!(ai
        .profitable_industrial_foundation(&game, 0, cid, &plan)
        .is_none());
}

#[test]
fn pending_workshop_closes_new_chain_but_owed_building_still_reserves() {
    let (mut game, cid, second, ai, plan) = district_fixture();
    install_test_district(&mut game, second, "industrial_zone");
    let workshop = Item::Building {
        building: crate::name!("workshop"),
    };
    assert!(game.can_produce(0, second, &workshop));
    game.apply(
        0,
        &Action::Produce {
            city: second,
            item: workshop.clone(),
        },
    )
    .unwrap();
    assert!(ai
        .profitable_industrial_foundation(&game, 0, cid, &plan)
        .is_none());
    game.cities.get_mut(&second).unwrap().queue.clear();
    assert_eq!(
        ai.profitable_industrial_foundation(&game, 0, second, &plan),
        Some(workshop)
    );
}
