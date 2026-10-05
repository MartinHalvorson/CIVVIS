use super::*;
use crate::ai::advanced::GrandStrategy;
use crate::rules::Yields;
use std::sync::Arc;

fn board() -> (Game, AdvancedAi, StrategicPlan, [u32; 2]) {
    let mut g = Game::new(2, 32, 22, 914_357_001, 250, 0);
    g.units.clear();
    g.cities.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
    }
    let cities = [(5, 5), (15, 5)].map(|pos| g.found_city_for(0, pos, None));
    for cid in cities {
        let site = g.nbrs(g.cities[&cid].pos)[0];
        g.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("theater_square"));
        let city = g.cities.get_mut(&cid).unwrap();
        city.pop = 7;
        city.queue.clear();
        city.buildings.clear();
        city.districts.insert(crate::name!("theater_square"), site);
        Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
            cid,
            Yields {
                production: 20.0,
                ..Default::default()
            },
        );
    }
    g.turn = 100;
    g.record_contact(0, 1);
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 10.0;
    g.players[0]
        .civics
        .extend([crate::name!("drama_poetry"), crate::name!("humanism")]);
    Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        1,
        Yields {
            culture: 100.0,
            ..Default::default()
        },
    );
    let mut ai = AdvancedAi::new();
    ai.enable_culture_defense_finishes_theater();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, cities)
}

fn building(name: &str) -> Item {
    Item::Building {
        building: name.into(),
    }
}

fn reserved(g: &Game) -> Vec<(u32, Item)> {
    g.player_city_ids(0)
        .into_iter()
        .filter_map(|cid| {
            g.cities[&cid]
                .queue
                .first()
                .cloned()
                .map(|item| (cid, item))
        })
        .collect()
}

#[test]
fn reserves_an_amphitheater_during_conquest_only_when_enabled() {
    let (mut g, mut ai, plan, _) = board();
    g.at_war.insert((0, 1));
    ai.disable_culture_defense_finishes_theater();
    ai.reserve_culture_defense_building(&mut g, 0, &plan);
    assert!(reserved(&g).is_empty());
    ai.enable_culture_defense_finishes_theater();
    ai.reserve_culture_defense_building(&mut g, 0, &plan);
    let claims = reserved(&g);
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].1, building("amphitheater"));
}

#[test]
fn an_active_building_services_the_empire_but_a_future_promise_does_not() {
    let (g, ai, plan, cities) = board();
    let mut active = g.clone();
    active
        .cities
        .get_mut(&cities[0])
        .unwrap()
        .queue
        .push(building("amphitheater"));
    ai.reserve_culture_defense_building(&mut active, 0, &plan);
    assert!(active.cities[&cities[1]].queue.is_empty());

    let mut promised = g;
    let city = promised.cities.get_mut(&cities[0]).unwrap();
    city.queue.push(Item::Unit {
        unit: crate::name!("warrior"),
    });
    city.queue.push(building("amphitheater"));
    let prior = city.queue.clone();
    ai.reserve_culture_defense_building(&mut promised, 0, &plan);
    assert_eq!(
        promised.cities[&cities[0]].queue, prior,
        "existing work stays"
    );
    assert_eq!(
        promised.cities[&cities[1]].queue,
        vec![building("amphitheater")]
    );
}

#[test]
fn prefers_the_city_that_finishes_first_and_does_not_claim_more_on_reentry() {
    let (mut g, ai, plan, cities) = board();
    Arc::make_mut(&mut g.observed_city_yield_adjustments)
        .get_mut(&cities[1])
        .unwrap()
        .production = 100.0;
    ai.reserve_culture_defense_building(&mut g, 0, &plan);
    assert_eq!(reserved(&g), vec![(cities[1], building("amphitheater"))]);
    ai.reserve_culture_defense_building(&mut g, 0, &plan);
    assert!(g.cities[&cities[0]].queue.is_empty());
}

#[test]
fn progresses_through_art_museum_and_broadcast_after_either_museum() {
    let (mut g, ai, plan, cities) = board();
    for cid in cities {
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .push(crate::name!("amphitheater"));
    }
    ai.reserve_culture_defense_building(&mut g, 0, &plan);
    assert_eq!(reserved(&g)[0].1, building("art_museum"));

    for cid in cities {
        let city = g.cities.get_mut(&cid).unwrap();
        city.queue.clear();
        city.buildings.push(crate::name!("archaeological_museum"));
    }
    g.players[0].techs.insert(crate::name!("radio"));
    ai.reserve_culture_defense_building(&mut g, 0, &plan);
    assert_eq!(reserved(&g)[0].1, building("broadcast_center"));
}

#[test]
fn protects_threatened_and_recently_attacked_cities() {
    let (mut g, ai, mut plan, cities) = board();
    plan.threatened_city = Some(cities[0]);
    g.cities.get_mut(&cities[1]).unwrap().last_attacked = g.turn - 2;
    ai.reserve_culture_defense_building(&mut g, 0, &plan);
    assert!(reserved(&g).is_empty());
    g.cities.get_mut(&cities[1]).unwrap().last_attacked = g.turn - 5;
    ai.reserve_culture_defense_building(&mut g, 0, &plan);
    assert_eq!(reserved(&g), vec![(cities[1], building("amphitheater"))]);
}

#[test]
fn recovery_and_a_culture_lead_leave_the_queues_to_the_governor() {
    let (g, ai, plan, _) = board();
    let mut broke = g.clone();
    broke.players[0].gold = 10.0;
    broke.players[0].gold_per_turn = -10.0;
    ai.reserve_culture_defense_building(&mut broke, 0, &plan);
    assert!(reserved(&broke).is_empty());
    let mut ahead = g;
    Arc::make_mut(&mut ahead.observed_yield_adjustments)
        .get_mut(&1)
        .unwrap()
        .culture = 0.0;
    ai.reserve_culture_defense_building(&mut ahead, 0, &plan);
    assert!(reserved(&ahead).is_empty());
}

#[test]
fn does_not_open_a_theater_or_commit_beyond_the_useful_clock() {
    let (g, ai, plan, cities) = board();
    let mut no_theater = g.clone();
    for cid in cities {
        no_theater.cities.get_mut(&cid).unwrap().districts.clear();
    }
    ai.reserve_culture_defense_building(&mut no_theater, 0, &plan);
    assert!(reserved(&no_theater).is_empty());
    let mut late = g;
    late.turn = 249;
    ai.reserve_culture_defense_building(&mut late, 0, &plan);
    assert!(reserved(&late).is_empty());
}
