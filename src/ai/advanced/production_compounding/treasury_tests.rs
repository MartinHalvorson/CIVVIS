use super::*;
use crate::setup::GameSpeed;
use std::sync::Arc;

fn board() -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(2, 32, 22, 610_068_499, 250, 0, false);
    g.game_speed = GameSpeed::Online;
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.hills = true;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.pillaged = false;
    }
    let first = g.found_city_for(0, (5, 5), None);
    let second = g.found_city_for(0, (19, 5), None);
    let rival = g.found_city_for(1, (22, 16), None);
    g.spawn_test_unit("warrior", 1, g.cities[&rival].pos);
    for cid in [first, second] {
        let city = g.cities.get_mut(&cid).unwrap();
        city.queue.clear();
        city.pop = 3;
        let worked = city
            .owned_tiles
            .iter()
            .copied()
            .filter(|pos| *pos != city.pos)
            .take(3)
            .collect();
        Arc::make_mut(&mut g.observed_city_worked_tiles).insert(cid, worked);
        Arc::make_mut(&mut g.observed_city_amenity_adjustments).insert(cid, 2);
        g.spawn_test_unit("warrior", 0, g.cities[&cid].pos);
    }
    let builder = g.spawn_test_unit("builder", 0, g.cities[&first].pos);
    g.players[0]
        .techs
        .extend([crate::name!("mining"), crate::name!("apprenticeship")]);
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 60.0;
    g.turn = 40;
    g.current = 0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: 40,
        rush: false,
    });
    assert!(g.city_amenity_surplus(&g.cities[&second]) >= 0);
    assert!(
        ai.productive_builder_purchase_candidate(&g, 0, 120.0)
            .is_some(),
        "positive fixture: cities={:?}, price={:?}, worked={:?}",
        g.player_city_ids(0),
        g.unit_purchase_cost(0, second, "builder", "gold"),
        g.city_citizen_plan(second).worked_tiles
    );
    (g, ai, second, builder)
}

#[test]
fn actual_purchase_services_distant_work_without_spending_queue_progress_or_reserve() {
    let (mut g, ai, city, _) = board();
    g.cities.get_mut(&city).unwrap().queue = vec![Item::Building {
        building: crate::name!("granary"),
    }];
    g.cities.get_mut(&city).unwrap().production = 5.0;
    let queue = g.cities[&city].queue.clone();
    let price = g.unit_purchase_cost(0, city, "builder", "gold").unwrap();
    let before = serde_json::to_value(&g).unwrap();
    let choice = ai
        .productive_builder_purchase_candidate(&g, 0, 120.0)
        .unwrap();
    assert_eq!(choice.0, city);
    assert!(choice.2 >= 6.0);
    assert_eq!(
        serde_json::to_value(&g).unwrap(),
        before,
        "preflight is read-only"
    );
    assert!(ai.young_empire_purchase(&mut g, 0, 120.0));
    assert_eq!(g.players[0].gold, 500.0 - price);
    assert!(g.players[0].gold >= 120.0);
    assert_eq!(g.cities[&city].queue, queue);
    assert_eq!(g.cities[&city].production, 5.0);
    assert_eq!(
        g.player_unit_ids(0)
            .iter()
            .filter(|uid| g.units[uid].kind == "builder")
            .count(),
        2
    );
    assert!(
        !ai.productive_builder_purchase(&mut g, 0, 120.0),
        "new local charges cover the existing jobs"
    );
}

#[test]
fn coverage_credits_field_charges_and_a_queued_builder_that_arrives_soon() {
    let (mut g, ai, city, _) = board();
    g.spawn_test_unit("builder", 0, g.cities[&city].pos);
    assert!(ai
        .productive_builder_purchase_candidate(&g, 0, 120.0)
        .is_none());
    let (mut g, ai, city, _) = board();
    let item = Item::Unit {
        unit: crate::name!("builder"),
    };
    let cost = g.item_cost_for_city(0, city, &item);
    g.cities.get_mut(&city).unwrap().queue = vec![item];
    g.cities.get_mut(&city).unwrap().production = cost - 1.0;
    assert!(ai
        .productive_builder_purchase_candidate(&g, 0, 120.0)
        .is_none());
}

#[test]
fn insufficient_cash_income_and_actual_safety_keep_the_gold() {
    for case in 0..8 {
        let (mut g, mut ai, city, _) = board();
        match case {
            0 => {
                g.players[0].gold =
                    120.0 + g.unit_purchase_cost(0, city, "builder", "gold").unwrap() - 1.0
            }
            1 => g.players[0].gold_per_turn = 1.0,
            2 => ai.plan.as_mut().unwrap().threatened_city = Some(city),
            3 => ai.plan.as_mut().unwrap().strategy = GrandStrategy::Recovery,
            4 => {
                g.at_war.insert((0, 1));
            }
            5 => g.turn = g.standard_duration(160) + 1,
            6 => {
                ai.victory_target = None;
            }
            7 => {
                g.player_unit_ids(0)
                    .iter()
                    .filter(|uid| g.units[uid].kind == "warrior")
                    .copied()
                    .collect::<Vec<_>>()
                    .into_iter()
                    .for_each(|uid| g.remove_unit(uid));
            }
            _ => unreachable!(),
        }
        let before = serde_json::to_value(&g).unwrap();
        assert!(
            !ai.productive_builder_purchase(&mut g, 0, 120.0),
            "guard case {case}"
        );
        assert_eq!(
            serde_json::to_value(&g).unwrap(),
            before,
            "guard case {case}"
        );
    }
}

#[test]
fn purchased_builder_completes_a_real_worked_mine() {
    let (mut g, ai, city, _) = board();
    let pos = g.observed_city_worked_tiles[&city][0];
    let before = g.modeled_tile_yields(pos).production;
    let gain = g
        .improvement_yield_change(0, pos, crate::name!("mine"))
        .production;
    assert!(ai.productive_builder_purchase(&mut g, 0, 120.0));
    let builder = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "builder" && g.units[uid].pos == g.cities[&city].pos)
        .unwrap();
    g.apply(
        0,
        &Action::Move {
            unit: builder,
            to: pos,
        },
    )
    .unwrap();
    g.apply(0, &Action::EndTurn).unwrap();
    while g.current != 0 {
        let pid = g.current;
        g.apply(pid, &Action::EndTurn).unwrap();
    }
    g.apply(
        0,
        &Action::Improve {
            unit: builder,
            improvement: crate::name!("mine"),
        },
    )
    .unwrap();
    assert!((g.modeled_tile_yields(pos).production - before - gain).abs() < 1e-9);
}
