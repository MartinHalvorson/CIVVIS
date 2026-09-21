use super::tests::{ring_of, walled_city};
use super::*;
use crate::ai::VictoryTarget;

fn pressure_fixture() -> (Game, AdvancedAi, u32, u32) {
    let (mut g, cid) = walled_city();
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    let uid = g.spawn_unit("man_at_arms", 0, ring_of(&g, cid)[0]);
    let mut ai = AdvancedAi::new();
    ai.victory_target = Some(VictoryTarget::Domination);
    (g, ai, cid, uid)
}

#[test]
fn healthy_taker_reduces_an_unwalled_city_without_spending_its_capture_reserve() {
    let (mut g, mut ai, cid, uid) = pressure_fixture();
    let before = g.cities[&cid].hp;
    let city = CityView::of(&g, cid).unwrap();
    assert!(f64::from(before) > taker_blow(&g, 0, uid, cid));
    ai.taker_step(&mut g, 0, uid, &city);
    assert!(
        g.cities[&cid].hp < before,
        "the taker contributes before a one-hit capture"
    );
    assert!(g.units[&uid].hp >= 60, "capture body stays healthy");
    assert_eq!(g.cities[&cid].owner, 1);
}

#[test]
fn taker_pressure_preserves_walls_wounded_units_and_other_victory_lanes() {
    for case in ["wall", "wounded", "other_lane", "spent"] {
        let (mut g, mut ai, cid, uid) = pressure_fixture();
        match case {
            "wall" => g.cities.get_mut(&cid).unwrap().wall_hp = 100,
            "wounded" => g.units.get_mut(&uid).unwrap().hp = 79,
            "other_lane" => ai.victory_target = Some(VictoryTarget::Science),
            "spent" => g.units.get_mut(&uid).unwrap().attacks_left = 0,
            _ => unreachable!(),
        }
        let city = CityView::of(&g, cid).unwrap();
        let hp = g.units[&uid].hp;
        assert!(!ai.safe_taker_pressure(&mut g, 0, uid, &city), "{case}");
        assert_eq!(g.cities[&cid].hp, 200, "{case}");
        assert_eq!(g.units[&uid].hp, hp, "{case}");
    }
}

#[test]
fn a_strong_reply_keeps_the_capture_unit_in_reserve() {
    let (mut g, mut ai, cid, uid) = pressure_fixture();
    let pos = g.units[&uid].pos;
    let target = g
        .nbrs(pos)
        .into_iter()
        .find(|p| {
            *p != g.cities[&cid].pos
                && g.map
                    .get(*p)
                    .is_some_and(|t| g.rules.is_passable(t) && !g.rules.is_water(t))
        })
        .unwrap();
    g.spawn_unit("tank", 1, target);
    let city = CityView::of(&g, cid).unwrap();
    assert!(!ai.safe_taker_pressure(&mut g, 0, uid, &city));
    assert_eq!(g.cities[&cid].hp, 200);
    assert_eq!(g.units[&uid].hp, 100);
}
