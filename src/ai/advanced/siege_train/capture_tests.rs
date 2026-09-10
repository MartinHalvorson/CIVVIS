use super::tests::{at_distance, plan_against, ring_of, step_unit, walled_city};
use super::*;

#[test]
fn a_hybrid_takes_the_city_instead_of_shooting_its_last_hit_point() {
    let (mut g, cid) = walled_city();
    let robot = g.spawn_unit("giant_death_robot", 0, ring_of(&g, cid)[0]);
    g.cities.get_mut(&cid).unwrap().hp = 1;
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    assert_eq!(arm_of(&g, robot), Arm::Shooter);
    let city = CityView::of(&g, cid).unwrap();
    assert_eq!(designate_taker(&g, &city, &[robot]), Some(robot));
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    let plan = plan_against(&g, cid);
    step_unit(&mut ai, &mut g, 0, robot, &plan);
    assert_eq!(g.cities[&cid].owner, 0);
    assert_eq!(ai.census.siege_captures, 1);
}

#[test]
fn a_hybrid_keeps_bombarding_until_it_can_finish() {
    let (mut g, cid) = walled_city();
    let robot = g.spawn_unit("giant_death_robot", 0, ring_of(&g, cid)[0]);
    let city = CityView::of(&g, cid).unwrap();
    assert_eq!(designate_taker(&g, &city, &[robot]), None);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    let plan = plan_against(&g, cid);
    step_unit(&mut ai, &mut g, 0, robot, &plan);
    assert!(g.cities[&cid].wall_hp < 100);
    assert!(!ai.unit_is_reserved(robot));
}

#[test]
fn a_hybrid_finishers_ring_post_is_not_overwritten_by_a_firing_post() {
    let (mut g, cid) = walled_city();
    let origin = at_distance(&g, cid, 2)[0];
    let robot = g.spawn_unit("giant_death_robot", 0, origin);
    g.cities.get_mut(&cid).unwrap().hp = 1;
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    let city = CityView::of(&g, cid).unwrap();
    assert_eq!(designate_taker(&g, &city, &[robot]), Some(robot));
    let posts = siege_posts(&g, 0, &city, &[robot], Some(robot));
    assert_eq!(g.wdist(posts[&robot], city.pos), 1);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    let plan = plan_against(&g, cid);
    for _ in 0..2 {
        step_unit(&mut ai, &mut g, 0, robot, &plan);
        if g.cities[&cid].owner == 0 {
            break;
        }
        g.apply(0, &Action::EndTurn).unwrap();
        g.apply(1, &Action::EndTurn).unwrap();
    }
    assert_eq!(
        g.cities[&cid].owner, 0,
        "the robot approaches and takes the city"
    );
}
