use super::super::ForcePosture;
use super::tests::{at_distance, plan_against, ring_of, walled_city};
use super::*;

fn group(g: &Game, uid: u32, cid: u32) -> ForceGroup {
    ForceGroup {
        id: uid,
        domain: ForceDomain::Land,
        units: vec![uid],
        anchor: g.units[&uid].pos,
        objective: g.cities[&cid].pos,
        focus_target: None,
        posture: ForcePosture::Advance,
        readiness: 1.0,
        local_strength_ratio: 2.0,
    }
}

#[test]
fn a_siege_does_not_reserve_another_citys_capture_unit() {
    let (mut g, first) = walled_city();
    let second_pos = at_distance(&g, first, 3)[0];
    g.found_city_for(1, second_pos, None);
    let second = g.city_at(second_pos).unwrap();
    let warrior_pos = ring_of(&g, second)
        .into_iter()
        .filter(|p| g.wdist(*p, g.cities[&first].pos) <= 3)
        .find(|p| g.city_at(*p).is_none() && !g.rules.is_water(g.map.get(*p).unwrap()))
        .unwrap();
    let warrior = g.spawn_unit("warrior", 0, warrior_pos);
    let gun_pos = at_distance(&g, first, 2)
        .into_iter()
        .find(|p| *p != warrior_pos && *p != second_pos)
        .unwrap();
    let gun = g.spawn_unit("catapult", 0, gun_pos);
    for cid in [first, second] {
        let city = g.cities.get_mut(&cid).unwrap();
        city.hp = 1;
        city.wall_hp = 0;
    }
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.force_groups = vec![group(&g, gun, first), group(&g, warrior, second)];
    let plan = plan_against(&g, first);
    let _ = ai.siege_doctrine_step(&mut g, 0, gun, &plan);
    assert_ne!(
        ai.sieges[&first].taker,
        Some(warrior),
        "a nearby unit assigned to the other city is not this siege's finisher"
    );
    assert!(!ai.sieges[&first].posts.contains_key(&warrior));
    let _ = ai.siege_doctrine_step(&mut g, 0, warrior, &plan);
    assert_eq!(
        g.cities[&second].owner, 0,
        "the warrior completes its assigned capture"
    );
}

#[test]
fn two_groups_assigned_to_one_city_share_its_finisher() {
    let (mut g, cid) = walled_city();
    let warrior = g.spawn_unit("warrior", 0, ring_of(&g, cid)[0]);
    let gun = g.spawn_unit("catapult", 0, at_distance(&g, cid, 2)[0]);
    g.cities.get_mut(&cid).unwrap().hp = 1;
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.force_groups = vec![group(&g, gun, cid), group(&g, warrior, cid)];
    let plan = plan_against(&g, cid);
    let _ = ai.siege_doctrine_step(&mut g, 0, gun, &plan);
    assert_eq!(ai.sieges[&cid].taker, Some(warrior));
    let _ = ai.siege_doctrine_step(&mut g, 0, warrior, &plan);
    assert_eq!(g.cities[&cid].owner, 0);
}
