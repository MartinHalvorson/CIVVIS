use super::*;
use crate::setup::GameSpeed;
use std::sync::Arc;

fn board() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new_full(2, 32, 22, 610_038_860, 250, 0, false);
    g.game_speed = GameSpeed::Online;
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.hills = true;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.pillaged = false;
    }
    let first = g.found_city_for(0, (5, 5), None);
    let second = g.found_city_for(0, (19, 5), None);
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
        g.spawn_test_unit("warrior", 0, g.cities[&cid].pos);
    }
    let builder = g.spawn_test_unit("builder", 0, g.cities[&first].pos);
    g.players[0].techs.insert(crate::name!("mining"));
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 10.0;
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
        AdvancedAi::targeting(super::super::VictoryTarget::Domination),
        plan,
        second,
        builder,
    )
}

#[test]
fn researched_improvement_forecast_matches_the_completed_operation() {
    let (mut g, _, _, city, _) = board();
    let pos = g.observed_city_worked_tiles[&city][0];
    g.players[0].techs.insert(crate::name!("apprenticeship"));
    let before = g.modeled_tile_yields(pos);
    let gain = g.improvement_yield_change(0, pos, crate::name!("mine"));
    assert!(gain.production > g.rules.improvements["mine"].yields.production);
    assert!(g.map.tiles[&pos].improvement.is_none());
    let builder = g.spawn_test_unit("builder", 0, pos);
    g.apply(
        0,
        &Action::Improve {
            unit: builder,
            improvement: crate::name!("mine"),
        },
    )
    .unwrap();
    assert!(
        (g.modeled_tile_yields(pos).production - before.production - gain.production).abs() < 1e-9
    );
}

#[test]
fn production_research_unlocks_worked_forests_without_mutating_the_parent() {
    let (mut g, ai, plan, _, _) = board();
    for pos in g
        .observed_city_worked_tiles
        .values()
        .flatten()
        .copied()
        .collect::<Vec<_>>()
    {
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.hills = false;
        tile.feature = Some(crate::name!("forest"));
    }
    g.players[0]
        .techs
        .extend([crate::name!("masonry"), crate::name!("horseback_riding")]);
    assert!(
        ai.named_production_technology_goal(&g, 0, &plan).is_none(),
        "the prerequisite detour must fit the actual science rate"
    );
    for cid in g.player_city_ids(0) {
        crate::game::install_test_district(&mut g, cid, "campus");
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .push(crate::name!("library"));
    }
    let _memo = g.query_memo();
    assert_eq!(
        ai.named_production_technology_goal(&g, 0, &plan),
        Some(crate::name!("construction"))
    );
    assert!(!g.players[0].techs.contains(&crate::name!("construction")));
    for pos in g.observed_city_worked_tiles.values().flatten() {
        assert!(!g
            .valid_improvements(0, *pos)
            .contains(&crate::name!("lumber_mill")));
    }
    let mut threatened = plan.clone();
    threatened.threatened_city = Some(g.player_city_ids(0)[0]);
    assert!(ai
        .named_production_technology_goal(&g, 0, &threatened)
        .is_none());
    assert!(AdvancedAi::new()
        .named_production_technology_goal(&g, 0, &plan)
        .is_none());
}

#[test]
fn production_research_requires_worked_jobs_and_time_for_the_prerequisites() {
    let (mut g, ai, plan, _, _) = board();
    for pos in g
        .observed_city_worked_tiles
        .values()
        .flatten()
        .copied()
        .collect::<Vec<_>>()
    {
        g.map.tiles.get_mut(&pos).unwrap().feature = Some(crate::name!("forest"));
    }
    g.players[0]
        .techs
        .extend([crate::name!("masonry"), crate::name!("horseback_riding")]);
    g.turn = 248;
    assert!(ai.named_production_technology_goal(&g, 0, &plan).is_none());
    g.turn = 40;
    Arc::make_mut(&mut g.observed_city_worked_tiles).clear();
    // The native governor must also have no forest jobs to work.
    for tile in g.map.tiles.values_mut() {
        tile.feature = None;
        tile.hills = false;
    }
    assert!(ai.named_production_technology_goal(&g, 0, &plan).is_none());
}

#[test]
fn an_already_legal_production_unlock_reaches_the_real_research_selector() {
    let (mut g, ai, plan, _, _) = board();
    for pos in g
        .observed_city_worked_tiles
        .values()
        .flatten()
        .copied()
        .collect::<Vec<_>>()
    {
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.hills = false;
        tile.feature = Some(crate::name!("forest"));
    }
    g.players[0].techs.extend([
        crate::name!("masonry"),
        crate::name!("horseback_riding"),
        crate::name!("archery"),
        crate::name!("writing"),
        crate::name!("pottery"),
    ]);
    for cid in g.player_city_ids(0) {
        crate::game::install_test_district(&mut g, cid, "campus");
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .push(crate::name!("library"));
    }
    assert_eq!(
        ai.named_production_technology_step(&g, 0, crate::name!("construction")),
        Some(crate::name!("construction"))
    );
    g.players[0].research = None;
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("construction"));
}
