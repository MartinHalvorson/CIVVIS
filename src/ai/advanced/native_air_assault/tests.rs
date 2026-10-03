use super::*;
use crate::ai::advanced::{GrandStrategy, StrategicPlan, VictoryTarget};
use std::collections::BTreeSet;

fn fixture() -> (Game, AdvancedAi, Pos, u32, Vec<u32>) {
    let mut g = Game::new_full(3, 40, 24, 387900, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (10, 10), None);
    let cid = g.found_city_for(1, (20, 10), None);
    let other = g.found_city_for(2, (30, 10), None);
    g.current = 0;
    g.at_war.extend([(0, 1), (0, 2)]);
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    g.players[0]
        .strategic_resources
        .insert(crate::name!("aluminum"), 40.0);
    let cavalry = g.spawn_test_unit("knight", 0, (18, 10));
    let aircraft = (0..2)
        .map(|_| g.spawn_test_unit("bomber", 0, (10, 10)))
        .collect();
    g.cities.get_mut(&cid).unwrap().hp = 60;
    // Match the observed Sheffield handoff: a healthy Knight cannot take a
    // 60-HP, strength-67 city without its still-unexecuted bomber sortie.
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(cid, 67.0);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.air_surge = true;
    // The general plan drifted to a different war after the spotting board.
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(2),
        target_city: Some(other),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    });
    ai.observe_air_assault_frame(BTreeSet::from([(20, 10)]), 1);
    (g, ai, (20, 10), cavalry, aircraft)
}

#[test]
fn fresh_observed_maneuver_survives_a_different_general_target() {
    let (mut g, mut ai, target, _, _) = fixture();
    let original_plan = ai.current_plan().unwrap().target_city;
    assert!(ai.resume_observed_air_city_assault(&mut g, 0, target));
    assert_eq!(ai.planned_air_city_assault().unwrap().target, target);
    assert_eq!(
        ai.current_plan().unwrap().target_city,
        original_plan,
        "local custody must not rewrite the strategic target"
    );
    assert!(g.log.iter().any(|(_, action)|
        matches!(action, crate::game::Action::AirStrike { target: p, .. } if *p == target)));
}

#[test]
fn capture_is_recomputed_from_observed_damage_not_the_prior_volley() {
    let (mut g, mut ai, target, cavalry, planes) = fixture();
    let cid = g.city_at(target).unwrap();
    g.cities.get_mut(&cid).unwrap().hp = 1;
    for uid in planes {
        g.units.get_mut(&uid).unwrap().moves_left = 0.0;
    }
    ai.observe_air_assault_frame(BTreeSet::from([target]), 0);
    assert!(ai.resume_observed_air_city_assault(&mut g, 0, target));
    assert_eq!(g.cities[&cid].owner, 0);
    assert_eq!(g.units[&cavalry].pos, target);
    assert!(ai.planned_air_city_assault().unwrap().aircraft.is_empty());
}

#[test]
fn an_observed_strong_city_is_not_claimed_captured_after_a_failed_volley() {
    let (mut g, mut ai, target, _, planes) = fixture();
    let cid = g.city_at(target).unwrap();
    let city = g.cities.get_mut(&cid).unwrap();
    city.hp = 200;
    city.wall_hp = 400;
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(cid, 400);
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(cid, 100.0);
    std::sync::Arc::make_mut(&mut g.observed_city_ranged_strength).insert(cid, 100.0);
    for uid in planes {
        g.units.get_mut(&uid).unwrap().moves_left = 0.0;
    }
    ai.observe_air_assault_frame(BTreeSet::from([target]), 0);
    ai.resume_observed_air_city_assault(&mut g, 0, target);
    assert_eq!(g.cities[&cid].owner, 1);
    assert!(!g.log.iter().any(|(_, action)|
        matches!(action, crate::game::Action::Attack { target: p, .. } if *p == target)));
}

#[test]
fn continuation_does_not_invent_visibility_or_a_remaining_frame() {
    for frames in [0, 1] {
        let (mut g, mut ai, target, cavalry, _) = fixture();
        g.relocate(cavalry, (16, 10));
        ai.observe_air_assault_frame(BTreeSet::new(), frames);
        let before = g.log.len();
        assert!(!ai.resume_observed_air_city_assault(&mut g, 0, target));
        assert_eq!(g.log.len(), before);
    }
}

#[test]
fn continuation_cannot_open_war_resume_recovery_or_attack_our_city() {
    for case in ["peace", "recovery", "own", "off", "turn"] {
        let (mut g, mut ai, target, _, _) = fixture();
        match case {
            "peace" => {
                g.at_war.clear();
            }
            "recovery" => ai.plan.as_mut().unwrap().strategy = GrandStrategy::Recovery,
            "own" => g.cities.get_mut(&g.city_at(target).unwrap()).unwrap().owner = 0,
            "off" => ai.air_surge = false,
            "turn" => g.current = 1,
            _ => unreachable!(),
        }
        let before = g.log.len();
        assert!(
            !ai.resume_observed_air_city_assault(&mut g, 0, target),
            "{case}"
        );
        assert_eq!(g.log.len(), before, "{case}");
    }
}

#[test]
fn refused_native_sorties_are_not_replayed_by_a_continuation() {
    let (mut g, mut ai, target, _, planes) = fixture();
    for uid in planes {
        std::sync::Arc::make_mut(&mut g.blocked_strikes).insert((uid, target));
    }
    let before = g.log.len();
    ai.resume_observed_air_city_assault(&mut g, 0, target);
    assert!(!g
        .log
        .iter()
        .skip(before)
        .any(|(_, action)| matches!(action, crate::game::Action::AirStrike { .. })));
    assert_eq!(g.cities[&g.city_at(target).unwrap()].owner, 1);
}

#[test]
fn failed_continuation_falls_back_to_the_exact_shared_player_policy() {
    let (mut baseline, mut before_ai, target, _, _) = fixture();
    before_ai.air_surge = false;
    let mut candidate = baseline.clone();
    let mut after_ai = before_ai.clone();
    let mapped = baseline
        .units
        .keys()
        .map(|uid| (*uid, i64::from(*uid)))
        .collect();
    let (normal, normal_begin) =
        crate::ai::player::plan_frame(&mut before_ai, &mut baseline, 0, &mapped);
    let (continued, continued_begin, resumed) = crate::ai::player::native_air_assault::plan_frame(
        &mut after_ai,
        &mut candidate,
        0,
        &mapped,
        target,
    );
    assert!(!resumed);
    assert_eq!(normal_begin, continued_begin);
    assert_eq!(
        serde_json::to_value(&normal.actions).unwrap(),
        serde_json::to_value(&continued.actions).unwrap()
    );
    assert_eq!(
        serde_json::to_value(baseline.log.iter().collect::<Vec<_>>()).unwrap(),
        serde_json::to_value(candidate.log.iter().collect::<Vec<_>>()).unwrap()
    );
}

#[test]
fn continuation_keeps_the_shared_immediate_kill_prepass() {
    let (mut g, mut ai, target, _, _) = fixture();
    let raider = g.spawn_test_unit("warrior", 1, (18, 11));
    g.units.get_mut(&raider).unwrap().hp = 1;
    let mapped = g.units.keys().map(|uid| (*uid, i64::from(*uid))).collect();
    let (finishing, _, _) =
        crate::ai::player::native_air_assault::plan_frame(&mut ai, &mut g, 0, &mapped, target);
    assert!(!finishing.actions.is_empty());
    assert!(
        !g.units.contains_key(&raider),
        "the continuation cannot bypass its tactical opening"
    );
    assert!(
        !g.log.iter().any(|(_, action)| matches!(action,
            crate::game::Action::Attack { unit, target: position }
            if finishing.actions.iter().any(|opening| matches!(opening,
                crate::game::Action::Attack { unit: used, .. } if used == unit))
                && *position == target)),
        "an attack spent by the opening cannot be reused for a city capture"
    );
}
