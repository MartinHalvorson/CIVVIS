use super::*;
use crate::game::defending_district_tests::fixture;

#[test]
fn domination_command_phase_fires_both_forts_independently() {
    let (mut g, city, enc, opp) = fixture();
    let first = g.spawn_test_unit("warrior", 1, (enc.0 - 1, enc.1));
    let second = g.spawn_test_unit("warrior", 1, (opp.0 + 1, opp.1));
    AdvancedAi::targeting(VictoryTarget::Domination).advanced_encampment_strikes(&mut g, 0);
    for uid in [first, second] {
        assert!(g.units.get(&uid).is_none_or(|unit| unit.hp < 100));
    }
    assert!(g.defending_district_state(city, enc).unwrap().struck);
    assert!(g.defending_district_state(city, opp).unwrap().struck);
}

#[test]
fn an_oppidum_is_an_enemy_forcing_reply_outside_city_center_range() {
    let (mut g, city, _, opp) = fixture();
    let target = (opp.0 + 1, opp.1);
    let victim = g.spawn_test_unit("warrior", 1, target);
    assert!(g.wdist(g.cities[&city].pos, target) > 2);
    assert!(AdvancedAi::enemy_can_force_a_reply_against_any(
        &g,
        0,
        &[victim]
    ));
    let replies = AdvancedAi::forcing_attacks_to(&g, 0, target, None);
    let action = replies.iter().find(|action| matches!(action,
        Action::DistrictStrike { city: c, source, target: t } if *c == city && *source == opp && *t == target
    )).expect("the observed Oppidum is a reply actor");
    g.apply(0, action).unwrap();
    assert!(g.units.get(&victim).is_none_or(|u| u.hp < 100));
    assert!(!AdvancedAi::forcing_attacks_to(&g, 0, target, None)
        .iter()
        .any(|action| matches!(action, Action::DistrictStrike { .. })));
}

#[test]
fn hostile_oppidum_fire_reaches_the_healing_and_damage_envelopes() {
    let (mut g, _, _, opp) = fixture();
    let target = (opp.0 + 1, opp.1);
    let victim = g.spawn_test_unit("warrior", 1, target);
    let income = BasicAi::incoming_damage(&g, 1, victim, target, &[]);
    assert!(income.total > 0.0 && income.worst > 0.0);
    assert!(BasicAi::anything_can_reach(&g, 1, target, &[]));
}

#[test]
fn threat_sum_counts_two_forts_in_one_city_as_two_sources() {
    let (mut g, city, enc, opp) = fixture();
    let moved = (opp.0 - 2, opp.1);
    g.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .remove(&crate::name!("encampment"));
    g.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("encampment"), moved);
    g.map.tiles.get_mut(&enc).unwrap().district = None;
    let tile = g.map.tiles.get_mut(&moved).unwrap();
    tile.owner_city = Some(city);
    tile.district = Some(crate::name!("encampment"));
    g.cities.get_mut(&city).unwrap().wall_hp = 0;
    let target = (opp.0 - 1, opp.1);
    let victim = g.spawn_test_unit("warrior", 1, target);
    let both = BasicAi::incoming_damage(&g, 1, victim, target, &[]);
    g.cities.get_mut(&city).unwrap().encampment_wall_hp = 0;
    let one = BasicAi::incoming_damage(&g, 1, victim, target, &[]);
    assert!(one.total > 0.0);
    assert!((both.total - 2.0 * one.total).abs() < 1e-9);
    assert_eq!(both.worst, one.worst);
}

#[test]
fn a_pillaged_oppidum_contributes_no_district_shot() {
    let (mut g, city, _, opp) = fixture();
    let target = (opp.0 + 1, opp.1);
    let victim = g.spawn_test_unit("warrior", 1, target);
    let mut state = g.defending_district_state(city, opp).unwrap();
    state.pillaged = true;
    g.set_defending_district_state(city, state);
    g.map.tiles.get_mut(&opp).unwrap().pillaged = true;
    assert_eq!(
        BasicAi::incoming_damage(&g, 1, victim, target, &[]).total,
        0.0
    );
    assert!(!AdvancedAi::forcing_attacks_to(&g, 0, target, None)
        .iter()
        .any(|action| matches!(action, Action::DistrictStrike { .. })));
}

#[test]
fn retreat_danger_field_includes_an_oppidum_as_its_own_actor() {
    let (mut g, city, _, opp) = fixture();
    let target = (opp.0 + 1, opp.1);
    let victim = g.spawn_test_unit("warrior", 1, target);
    let mut field = battle_planner::DangerField::with_reach(&g, 1, true);
    let blows = field.contributions(target, victim);
    assert_eq!(blows.len(), 1);
    assert!(blows[0].0.is_none() && blows[0].1 > 0.0);
    assert!(g.wdist(g.cities[&city].pos, target) > 2);
}
