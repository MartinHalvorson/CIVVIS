use super::tests::{at_distance, plan_against, ring_of, walled_city};
use super::*;
use crate::ai::advanced::ForcePosture;

fn open_breach() -> (Game, u32, u32, ForceGroup) {
    let (mut g, city) = walled_city();
    g.cities.get_mut(&city).unwrap().wall_hp = 0;
    g.cities.get_mut(&city).unwrap().hp = 1;
    let taker = g.spawn_unit("warrior", 0, ring_of(&g, city)[0]);
    let far = at_distance(&g, city, STAGING_FAR + 2);
    let reserves: Vec<u32> = far
        .iter()
        .take(2)
        .map(|pos| g.spawn_unit("giant_death_robot", 0, *pos))
        .collect();
    assert_eq!(reserves.len(), 2);
    let city_pos = g.cities[&city].pos;
    let group = ForceGroup {
        id: taker,
        domain: ForceDomain::Land,
        units: [vec![taker], reserves].concat(),
        anchor: city_pos,
        objective: city_pos,
        focus_target: Some(city_pos),
        posture: ForcePosture::Muster,
        readiness: 0.0,
        local_strength_ratio: 1.0,
    };
    assert!(unit_power(&g, taker) < siege_bill(&g, 0, &CityView::of(&g, city).unwrap()));
    (g, city, taker, group)
}

#[test]
fn a_reachable_taker_finishes_a_breach_before_distant_reserves_stage() {
    let (mut g, city, taker, group) = open_breach();
    let plan = plan_against(&g, city);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    assert_eq!(
        ai.siege_train_step(&mut g, 0, taker, city, &plan, &group),
        Some(true)
    );
    assert_eq!(g.cities[&city].owner, 0);
    assert_eq!(ai.census.siege_captures, 1);
}

#[test]
fn a_taker_still_outside_one_turn_reach_marches_in_before_the_breach_heals() {
    let (mut g, city, old_taker, mut group) = open_breach();
    g.remove_unit(old_taker);
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    let start = at_distance(&g, city, STAGING_FAR)[0];
    let taker = g.spawn_unit("warrior", 0, start);
    group.id = taker;
    group.units[0] = taker;
    let target = g.cities[&city].pos;
    assert!(g.route_distance(taker, target, 1).is_some());
    assert!(ring_of(&g, city)
        .iter()
        .all(|pos| !g.reachable(taker).contains(pos)));
    let plan = plan_against(&g, city);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    assert_eq!(
        ai.siege_train_step(&mut g, 0, taker, city, &plan, &group),
        Some(true)
    );
    assert_eq!(ai.sieges[&city].stage, SiegeStage::Invest);
    assert_eq!(ai.sieges[&city].taker, Some(taker));
    assert!(g.wdist(g.units[&taker].pos, target) < STAGING_FAR);
    g.turn += 1;
    g.cities.get_mut(&city).unwrap().hp = 120;
    ai.assess_siege(&g, 0, city, &plan, &group);
    assert_eq!(ai.sieges[&city].stage, SiegeStage::Invest);
    assert_eq!(ai.sieges[&city].taker, Some(taker));
}

#[test]
fn a_wounded_adjacent_unit_does_not_hide_the_healthy_approaching_taker() {
    let (mut g, city, wounded, mut group) = open_breach();
    g.units.get_mut(&wounded).unwrap().hp = 50;
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    let start = at_distance(&g, city, STAGING_FAR)[0];
    let healthy = g.spawn_unit("warrior", 0, start);
    group.units.push(healthy);
    let plan = plan_against(&g, city);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.assess_siege(&g, 0, city, &plan, &group);
    assert_eq!(ai.sieges[&city].stage, SiegeStage::Invest);
    assert_eq!(ai.sieges[&city].taker, Some(healthy));
}

#[test]
fn an_unbreached_or_healthy_city_still_waits_for_the_train() {
    for (wall_hp, city_hp) in [(1, 1), (0, 200)] {
        let (mut g, city, _taker, group) = open_breach();
        g.cities.get_mut(&city).unwrap().wall_hp = wall_hp;
        g.cities.get_mut(&city).unwrap().hp = city_hp;
        let plan = plan_against(&g, city);
        let mut ai = AdvancedAi::new();
        ai.enable_siege_train();
        ai.assess_siege(&g, 0, city, &plan, &group);
        assert_eq!(ai.sieges[&city].stage, SiegeStage::Stage);
    }
}
