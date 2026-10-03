use super::*;
use crate::game::HostMenuEntry;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

fn board() -> (Game, AdvancedAi, [u32; 2], Item) {
    let mut g = Game::new_full(2, 32, 20, 10_033_877, 300, 0, false);
    g.units.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
    }
    let cities = [(5, 5), (17, 5)].map(|pos| g.found_city_for(0, pos, None));
    for city in cities {
        let c = g.cities.get_mut(&city).unwrap();
        c.queue.clear();
        c.production = 0.0;
    }
    let item = Item::Unit {
        unit: crate::name!("bomber"),
    };
    (g, AdvancedAi::new(), cities, item)
}

fn quote(g: &mut Game, city: u32, item: &Item, turns: Option<f64>) {
    Arc::make_mut(&mut g.host_buildable)
        .entry(city)
        .or_default()
        .insert(
            Game::production_block_key(item),
            HostMenuEntry {
                cost: Some(280.0),
                turns,
            },
        );
}

fn fallback(g: &Game, ai: &AdvancedAi, city: u32, item: &Item) -> f64 {
    let production = g.city_yields(city).production.max(1.0);
    let rate = if ai.victory_planning {
        (production * g.item_prod_mult(0, city, Some(item))).max(1.0)
    } else {
        production
    };
    g.item_remaining_cost_for_city(0, city, item) / rate
}

#[test]
fn native_bomber_quote_is_the_live_build_time_not_a_recomputed_ratio() {
    let (mut g, ai, cities, item) = board();
    g.game_speed = GameSpeed::Online;
    Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        cities[0],
        Yields {
            production: 40.0,
            ..Default::default()
        },
    );
    quote(&mut g, cities[0], &item, Some(5.0));
    assert_ne!(fallback(&g, &ai, cities[0], &item), 5.0);
    assert_eq!(ai.production_build_turns(&g, 0, cities[0], &item), 5.0);
}

#[test]
fn native_turn_quote_is_not_scaled_again_by_game_speed() {
    let (mut g, ai, cities, item) = board();
    quote(&mut g, cities[0], &item, Some(5.0));
    for speed in [GameSpeed::Online, GameSpeed::Standard, GameSpeed::Marathon] {
        g.game_speed = speed;
        assert_eq!(ai.production_build_turns(&g, 0, cities[0], &item), 5.0);
    }
}

#[test]
fn native_quote_stays_specific_to_the_city_item_and_formation() {
    let (mut g, ai, cities, item) = board();
    quote(&mut g, cities[0], &item, Some(5.0));
    quote(&mut g, cities[1], &item, Some(12.0));
    assert_eq!(ai.production_build_turns(&g, 0, cities[0], &item), 5.0);
    assert_eq!(ai.production_build_turns(&g, 0, cities[1], &item), 12.0);
    for other in [
        Item::Unit {
            unit: crate::name!("warrior"),
        },
        Item::Formation {
            unit: crate::name!("warrior"),
            formation: 1,
        },
    ] {
        assert_eq!(
            ai.production_build_turns(&g, 0, cities[0], &other),
            fallback(&g, &ai, cities[0], &other)
        );
    }
}

#[test]
fn invalid_or_missing_native_quotes_preserve_the_existing_fallback() {
    let (mut g, ai, cities, item) = board();
    for turns in [None, Some(-1.0), Some(f64::NAN), Some(f64::INFINITY)] {
        quote(&mut g, cities[0], &item, turns);
        assert_eq!(
            ai.production_build_turns(&g, 0, cities[0], &item),
            fallback(&g, &ai, cities[0], &item)
        );
    }
}

#[test]
fn native_zero_turn_quote_is_not_replaced_with_a_positive_guess() {
    let (mut g, ai, cities, item) = board();
    quote(&mut g, cities[0], &item, Some(0.0));
    assert_eq!(ai.production_build_turns(&g, 0, cities[0], &item), 0.0);
}

#[test]
fn unquoted_simulator_boards_keep_the_original_calculation() {
    let (mut g, mut ai, cities, item) = board();
    for victory_planning in [false, true] {
        ai.victory_planning = victory_planning;
        for speed in [GameSpeed::Online, GameSpeed::Standard] {
            g.game_speed = speed;
            assert_eq!(
                ai.production_build_turns(&g, 0, cities[0], &item),
                fallback(&g, &ai, cities[0], &item)
            );
        }
    }
}

#[test]
fn native_quotes_choose_the_faster_city_for_research_investment() {
    let (mut g, mut ai, cities, _) = board();
    g.turn = 60;
    g.record_contact(0, 1);
    g.players[0].techs.insert(crate::name!("writing"));
    g.players[0].gold = 1000.0;
    g.players[0].gold_per_turn = 10.0;
    Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        1,
        Yields {
            science: 1000.0,
            ..Default::default()
        },
    );
    let library = Item::Building {
        building: crate::name!("library"),
    };
    for (city, turns) in [(cities[0], 8.0), (cities[1], 1.0)] {
        let pos = g.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|pos| *pos != g.cities[&city].pos)
            .unwrap();
        g.map.tiles.get_mut(&pos).unwrap().district = Some(crate::name!("campus"));
        g.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(crate::name!("campus"), pos);
        quote(&mut g, city, &library, Some(turns));
        Arc::make_mut(&mut g.host_buildable)
            .get_mut(&city)
            .unwrap()
            .get_mut(&Game::production_block_key(&library))
            .unwrap()
            .cost = Some(90.0);
    }
    ai.enable_research_building_catchup_2();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    assert_eq!(g.cities[&cities[1]].queue.first(), Some(&library));
    assert!(g.cities[&cities[0]].queue.is_empty());
}
