use super::tests::{plan_against, ring_of, step_unit, walled_city};
use super::*;

fn landing_capture() -> (Game, u32, u32) {
    let (mut g, cid) = walled_city();
    let shore = ring_of(&g, cid)[0];
    g.map.tiles.get_mut(&shore).unwrap().terrain = crate::name!("coast");
    g.players[0].techs.clear();
    g.players[0].techs.insert(crate::name!("shipbuilding"));
    g.players[0].techs.insert(crate::name!("cartography"));
    g.players[0]
        .strategic_resources
        .insert(crate::name!("oil"), 400.0);
    let tank = g.spawn_unit("tank", 0, shore);
    let city_pos = g.cities[&cid].pos;
    g.spawn_unit("musketman", 1, city_pos);
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    g.cities.get_mut(&cid).unwrap().hp = 21;
    assert!(g.is_embarked(&g.units[&tank]));

    // The real action is the ground truth: the landing penalty applies to
    // the Tank's attack, not to the era's embarked defensive strength.
    let mut actual = g.clone();
    actual
        .apply(
            0,
            &Action::Attack {
                unit: tank,
                target: city_pos,
            },
        )
        .unwrap();
    assert_eq!(actual.cities[&cid].owner, 0);

    (g, cid, tank)
}

#[test]
fn an_embarked_finisher_takes_a_city_its_actual_attack_can_capture() {
    let (mut g, cid, tank) = landing_capture();
    let mut ai = AdvancedAi::new();
    let plan = plan_against(&g, cid);
    assert_eq!(ai.siege_doctrine_step(&mut g, 0, tank, &plan), None);
    ai.enable_siege_train();
    step_unit(&mut ai, &mut g, 0, tank, &plan);
    assert_eq!(
        g.cities[&cid].owner, 0,
        "a legal amphibious capture must not be discarded with ordinary embarked siege roles"
    );
    assert_eq!(ai.census.siege_captures, 1);
}

#[test]
fn the_landing_blow_uses_attack_strength_instead_of_embarked_defense() {
    let (mut g, cid, tank) = landing_capture();
    let city = CityView::of(&g, cid).unwrap();
    let blow = taker_blow(&g, 0, tank, cid);
    assert!(
        blow >= f64::from(city.hp),
        "a demonstrated capture is priced at only {blow}"
    );
    let mut ai = AdvancedAi::new();
    ai.taker_step(&mut g, 0, tank, &city);
    assert_eq!(g.cities[&cid].owner, 0);
}

#[test]
fn ordinary_tactics_can_select_the_legal_landing_capture() {
    let (mut g, cid, tank) = landing_capture();
    let target = g.cities[&cid].pos;
    assert!(g.melee_order_is_legal(0, tank, target));
    assert!(g.legal_actions(0).iter().any(|action| matches!(
        action, Action::Attack { unit, target: pos } if *unit == tank && *pos == target
    )));
    let mut ai = AdvancedAi::new();
    let plan = plan_against(&g, cid);
    assert!(ai.advanced_military_step(&mut g, 0, tank, &plan));
    assert_eq!(g.cities[&cid].owner, 0);
}

#[test]
fn landing_permission_does_not_allow_embarked_ranged_or_water_attacks() {
    let (mut g, cid, tank) = landing_capture();
    let target = g.cities[&cid].pos;
    let shore = g.units[&tank].pos;
    let archer = g.spawn_unit("archer", 0, shore);
    assert!(!g.legal_actions(0).iter().any(|action| matches!(
        action, Action::Ranged { unit, .. } if *unit == archer
    )));
    assert!(g
        .apply(
            0,
            &Action::Ranged {
                unit: archer,
                target
            }
        )
        .is_err());

    g.map.tiles.get_mut(&target).unwrap().terrain = crate::name!("coast");
    assert!(!g.melee_order_is_legal(0, tank, target));
    assert!(!g.legal_actions(0).iter().any(|action| matches!(
        action, Action::Attack { unit, target: pos } if *unit == tank && *pos == target
    )));
    assert!(g.apply(0, &Action::Attack { unit: tank, target }).is_err());
}
