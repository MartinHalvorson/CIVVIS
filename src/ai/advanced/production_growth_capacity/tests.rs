use super::*;
use crate::game::{Action, GameOptions};
use std::collections::BTreeSet;
use std::sync::Arc;

fn board() -> (Game, u32, StrategicPlan) {
    let mut options = GameOptions::new(2, 32, 20, 61006499, 250, 0);
    options.speed = "online".into();
    options.barbarians = false;
    options.disaster_intensity = 0;
    options.handicap_exempt = BTreeSet::from([0]);
    let mut g = Game::new_with(options);
    for pid in 0..2 {
        g.current = pid;
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|u| g.units[u].kind == "settler")
            .unwrap();
        g.apply(pid, &Action::FoundCity { unit: settler }).unwrap();
    }
    g.current = 0;
    let cid = g.player_city_ids(0)[0];
    let center = g.cities[&cid].pos;
    let second = g
        .map
        .tiles
        .values()
        .find(|t| {
            !g.rules.is_water(t)
                && t.terrain != "mountain"
                && t.owner_city.is_none()
                && g.map.distance(t.pos, center) >= 7
        })
        .unwrap()
        .pos;
    g.found_city_for(0, second, None);
    g.players[0].techs.extend([
        crate::name!("pottery"),
        crate::name!("mining"),
        crate::name!("apprenticeship"),
    ]);
    for city in g.player_city_ids(0) {
        let center = g.cities[&city].pos;
        let positions: Vec<_> = g
            .map
            .tiles
            .keys()
            .copied()
            .filter(|p| g.map.distance(*p, center) <= 3)
            .collect();
        for pos in positions {
            let tile = g.map.tiles.get_mut(&pos).unwrap();
            tile.owner_city = Some(city);
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = pos != center;
            tile.resource = None;
            tile.improvement = (pos != center).then_some(crate::name!("mine"));
            tile.pillaged = false;
        }
        let c = g.cities.get_mut(&city).unwrap();
        c.pop = 4;
        c.food = 0.0;
        c.loyalty = 100.0;
        let current = g.city_housing(&g.cities[&city]);
        Arc::make_mut(&mut g.observed_city_housing_adjustments).insert(city, 4.0 - current);
        let current = g.city_amenity_surplus(&g.cities[&city]);
        Arc::make_mut(&mut g.observed_city_amenity_adjustments).insert(city, 10 - current);
    }
    for kind in ["warrior", "warrior", "builder", "trader"] {
        g.spawn_test_unit(kind, 0, center);
    }
    g.players[0].gold = 100.0;
    g.players[0].gold_per_turn = 20.0;
    g.turn = 35;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, cid, plan)
}

fn granary() -> Item {
    Item::Building {
        building: crate::name!("granary"),
    }
}

#[test]
fn growth_capacity_projection_matches_actual_growth_and_completion() {
    let (mut original, cid, _) = board();
    // This comparison isolates the target city's recurrence; the forecast
    // deliberately holds other populations fixed. Keep the other city at
    // zero housing so actual EndTurn also leaves its population unchanged.
    let other = original
        .player_city_ids(0)
        .into_iter()
        .find(|id| *id != cid)
        .unwrap();
    let adjustment = original.observed_city_housing_adjustments[&other];
    let housing = original.city_housing(&original.cities[&other]);
    Arc::make_mut(&mut original.observed_city_housing_adjustments)
        .insert(other, adjustment - housing);
    for investment in [None, Some(granary())] {
        let predicted = capacity_projection(&original, 0, cid, investment.as_ref(), 16).unwrap();
        let mut actual = original.clone();
        if let Some(item) = &investment {
            actual
                .apply(
                    0,
                    &Action::Produce {
                        city: cid,
                        item: item.clone(),
                    },
                )
                .unwrap();
        }
        let mut production = 0.0;
        for _ in 0..16 {
            production += actual
                .player_city_ids(0)
                .iter()
                .map(|id| actual.city_yields(*id).production)
                .sum::<f64>();
            actual.apply(0, &Action::EndTurn).unwrap();
            actual.apply(1, &Action::EndTurn).unwrap();
        }
        assert_eq!(predicted.population, actual.cities[&cid].pop);
        assert!(
            (predicted.production - production).abs() < 1e-6,
            "{} != {production}",
            predicted.production
        );
        if investment.is_some() {
            assert_eq!(
                predicted.completed,
                actual.cities[&cid]
                    .buildings
                    .contains(&crate::name!("granary"))
            );
        }
    }
}

#[test]
fn growth_capacity_reserves_real_return_and_reaches_the_governor() {
    let (mut g, cid, plan) = board();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_live_bridge();
    assert!(ai.wide_map_capacity);
    let before = g.cities[&cid].clone();
    let investment = ai
        .early_growth_capacity(&g, 0, cid, &plan)
        .expect("productive future citizens repay the build");
    assert_eq!(investment.item, granary());
    assert!(investment.returned >= investment.cost * CAPACITY_RETURN_MARGIN);
    assert_eq!(g.cities[&cid].pop, before.pop);
    assert_eq!(g.cities[&cid].food, before.food);
    assert_eq!(g.cities[&cid].queue, before.queue);
    let mut ai = ai;
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&cid].queue.first(), Some(&granary()));
}

#[test]
fn growth_capacity_does_not_buy_population_without_production() {
    let (mut g, cid, plan) = board();
    let center = g.cities[&cid].pos;
    let positions: Vec<_> = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|p| g.map.distance(*p, center) <= 3)
        .collect();
    for pos in positions {
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.hills = false;
        tile.improvement = None;
    }
    let a = capacity_projection(&g, 0, cid, None, 30).unwrap();
    let b = capacity_projection(&g, 0, cid, Some(&granary()), 30).unwrap();
    assert!(b.population > a.population);
    assert!(b.production <= a.production + 1e-6);
    assert!(AdvancedAi::targeting(VictoryTarget::Domination)
        .early_growth_capacity(&g, 0, cid, &plan)
        .is_none());
}

#[test]
fn growth_capacity_preserves_threats_queues_and_expansion() {
    let (base, cid, plan) = board();
    for reason in [
        "threat",
        "recent",
        "queue",
        "other_granary",
        "expansion",
        "income",
        "late",
        "army",
    ] {
        let mut g = base.clone();
        let mut plan = plan.clone();
        match reason {
            "threat" => plan.threatened_city = Some(cid),
            "recent" => g.cities.get_mut(&cid).unwrap().last_attacked = g.turn - 1,
            "queue" => g.cities.get_mut(&cid).unwrap().queue.push(Item::Unit {
                unit: crate::name!("warrior"),
            }),
            "other_granary" => {
                let other = g
                    .player_city_ids(0)
                    .into_iter()
                    .find(|id| *id != cid)
                    .unwrap();
                g.cities.get_mut(&other).unwrap().queue.push(granary());
            }
            "expansion" => {
                plan.strategy = GrandStrategy::Expansion;
                plan.desired_cities = 4;
            }
            "income" => g.players[0].gold_per_turn = -10.0,
            "late" => g.turn = g.max_turns - 1,
            "army" => {
                let ids: Vec<_> = g
                    .player_unit_ids(0)
                    .into_iter()
                    .filter(|id| g.rules.units[g.units[id].kind.as_str()].class == "military")
                    .collect();
                for id in ids {
                    g.remove_unit(id);
                }
                let other = g
                    .player_city_ids(0)
                    .into_iter()
                    .find(|id| *id != cid)
                    .unwrap();
                g.cities.get_mut(&other).unwrap().queue.push(Item::Unit {
                    unit: crate::name!("warrior"),
                });
            }
            _ => unreachable!(),
        }
        assert!(
            AdvancedAi::targeting(VictoryTarget::Domination)
                .early_growth_capacity(&g, 0, cid, &plan)
                .is_none(),
            "{reason}"
        );
    }
    for target in [
        VictoryTarget::Science,
        VictoryTarget::Culture,
        VictoryTarget::Score,
    ] {
        assert!(AdvancedAi::targeting(target)
            .early_growth_capacity(&base, 0, cid, &plan)
            .is_none());
    }
    assert!(AdvancedAi::new()
        .early_growth_capacity(&base, 0, cid, &plan)
        .is_none());
}

#[test]
fn growth_capacity_prices_mild_amenity_deficits_and_maintenance_runway() {
    let (mut g, cid, plan) = board();
    for city in g.player_city_ids(0) {
        Arc::make_mut(&mut g.observed_city_amenity_adjustments).remove(&city);
        let current = g.city_amenity_surplus(&g.cities[&city]);
        Arc::make_mut(&mut g.observed_city_amenity_adjustments).insert(city, -1 - current);
    }
    let upkeep = g.rules.buildings[crate::name!("granary")].maintenance;
    // Price the loaded rule, including a zero-maintenance Granary.
    // A half-Gold deficit must still consume the forecast's cash runway.
    g.players[0].gold_per_turn = upkeep - 0.5;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_live_bridge();
    assert!(ai.early_growth_capacity(&g, 0, cid, &plan).is_some());
    g.players[0].gold = CAPACITY_GOLD_BUFFER;
    assert!(ai.early_growth_capacity(&g, 0, cid, &plan).is_none());
    g.players[0].gold_per_turn = upkeep;
    assert!(ai.early_growth_capacity(&g, 0, cid, &plan).is_some());
    g.players[0].bankruptcy_amenity_penalty = 1;
    assert!(ai.early_growth_capacity(&g, 0, cid, &plan).is_none());
}

#[test]
fn growth_capacity_forecasts_future_workers_without_changing_host_observations() {
    let (mut mirror, cid, _) = board();
    let native = capacity_projection(&mirror, 0, cid, Some(&granary()), 30).unwrap();
    let worked = mirror.city_citizen_plan(cid).worked_tiles;
    assert_eq!(worked.len(), mirror.cities[&cid].pop as usize);
    Arc::make_mut(&mut mirror.observed_city_worked_tiles).insert(cid, worked.clone());
    let before = serde_json::to_vec(&mirror).unwrap();
    let modeled = capacity_projection(&mirror, 0, cid, Some(&granary()), 30).unwrap();
    assert!(modeled.population > mirror.cities[&cid].pop);
    assert_eq!(modeled.population, native.population);
    assert!((modeled.production - native.production).abs() < 1e-6);
    assert_eq!(mirror.observed_city_worked_tiles[&cid], worked);
    assert_eq!(serde_json::to_vec(&mirror).unwrap(), before);
}
