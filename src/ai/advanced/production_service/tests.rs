use super::*;
use crate::setup::GameSpeed;
use std::sync::Arc;

fn board() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new_full(2, 32, 22, 610_050_800, 150, 0, false);
    g.game_speed = GameSpeed::Online;
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
    }
    let fast = g.found_city_for(0, (5, 5), None);
    let weak = g.found_city_for(0, (11, 5), None);
    for (cid, production) in [(fast, 20.0), (weak, 2.0)] {
        let city = g.cities.get_mut(&cid).unwrap();
        city.queue.clear();
        city.pop = 3;
        Arc::make_mut(&mut g.observed_city_worked_tiles).insert(cid, vec![]);
        let actual = g.city_yields(cid).production;
        Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
            cid,
            Yields {
                production: production - actual,
                ..Default::default()
            },
        );
        g.spawn_test_unit("warrior", 0, g.cities[&cid].pos);
    }
    let jobs: Vec<_> = g.cities[&weak]
        .owned_tiles
        .iter()
        .copied()
        .filter(|pos| *pos != g.cities[&weak].pos && g.wdist(*pos, g.cities[&fast].pos) <= 6)
        .take(3)
        .collect();
    assert_eq!(jobs.len(), 3);
    for pos in &jobs {
        g.map.tiles.get_mut(pos).unwrap().feature = Some(crate::name!("forest"));
    }
    Arc::make_mut(&mut g.observed_city_worked_tiles).insert(weak, jobs);
    let current = g.city_yields(weak).production;
    Arc::make_mut(&mut g.observed_city_yield_adjustments)
        .get_mut(&weak)
        .unwrap()
        .production += 2.0 - current;
    assert!((g.city_yields(weak).production - 2.0).abs() < 1e-9);
    g.players[0].techs.insert(crate::name!("construction"));
    g.players[0].gold_per_turn = 10.0;
    g.players[0].gold = 500.0;
    g.turn = 40;
    g.current = 0;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: 40,
        rush: false,
    };
    (
        g,
        AdvancedAi::targeting(VictoryTarget::Domination),
        plan,
        fast,
        weak,
    )
}

#[test]
fn the_fast_city_actually_supplies_another_citys_worked_jobs() {
    let (mut g, mut ai, plan, fast, _) = board();
    assert_eq!(
        ai.productive_builder_dispatch_city(&g, 0, &plan, &ai.counts(&g, 0)),
        Some(fast)
    );
    ai.advanced_production(&mut g, 0, &plan, false);
    assert!(
        matches!(g.cities[&fast].queue.first(), Some(Item::Unit { unit }) if unit == "builder")
    );
    assert!(ai
        .productive_builder_dispatch_city(&g, 0, &plan, &ai.counts(&g, 0))
        .is_none());
}

#[test]
fn local_charges_and_unworked_land_do_not_create_another_reservation() {
    let (mut g, ai, plan, fast, weak) = board();
    g.spawn_test_unit("builder", 0, g.cities[&fast].pos);
    assert!(ai
        .productive_builder_dispatch_city(&g, 0, &plan, &ai.counts(&g, 0))
        .is_none());
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        if g.units[&uid].kind == "builder" {
            g.remove_unit(uid);
        }
    }
    Arc::make_mut(&mut g.observed_city_worked_tiles).insert(weak, vec![]);
    assert!(ai
        .productive_builder_dispatch_city(&g, 0, &plan, &ai.counts(&g, 0))
        .is_none());
}

#[test]
fn slow_or_committed_cities_and_recovery_keep_their_existing_work() {
    let (mut g, ai, mut plan, fast, _) = board();
    g.cities.get_mut(&fast).unwrap().queue = vec![Item::Building {
        building: crate::name!("monument"),
    }];
    assert!(ai
        .productive_builder_dispatch_city(&g, 0, &plan, &ai.counts(&g, 0))
        .is_none());
    g.cities.get_mut(&fast).unwrap().queue.clear();
    plan.threatened_city = Some(fast);
    assert!(ai
        .productive_builder_dispatch_city(&g, 0, &plan, &ai.counts(&g, 0))
        .is_none());
    plan.threatened_city = None;
    g.players[0].gold_per_turn = -1.0;
    assert!(ai
        .productive_builder_dispatch_city(&g, 0, &plan, &ai.counts(&g, 0))
        .is_none());
    g.players[0].gold_per_turn = 10.0;
    assert!(AdvancedAi::new()
        .productive_builder_dispatch_city(&g, 0, &plan, &ai.counts(&g, 0))
        .is_none());
    g.turn = g.standard_duration(160) + 1;
    assert!(ai
        .productive_builder_dispatch_city(&g, 0, &plan, &ai.counts(&g, 0))
        .is_none());
}
