use super::*;

fn capital_and_cheaper_city_state() -> (Game, AdvancedAi, usize, u32) {
    let mut g = Game::new_full(2, 48, 28, 91_002, 300, 1, false);
    let majors: Vec<_> = g
        .players
        .iter()
        .filter(|p| !p.is_minor && !p.is_barbarian)
        .map(|p| p.id)
        .collect();
    for pid in majors {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.found_city_for(pid, g.units[&settler].pos, None);
    }
    g.current = 0;
    g.turn = 200;
    let minor = g.players.iter().find(|p| p.is_minor).unwrap().id;
    g.record_contact(0, 1);
    g.record_contact(0, minor);
    let capital = g.player_city_ids(1)[0];
    let capital_pos = g.cities[&capital].pos;
    let city_state = g.player_city_ids(minor)[0];
    let city_state_pos = g.cities[&city_state].pos;
    g.cities.get_mut(&capital).unwrap().wall_hp = 400;
    g.cities.get_mut(&capital).unwrap().buildings.extend([
        crate::name!("walls"),
        crate::name!("medieval_walls"),
        crate::name!("renaissance_walls"),
    ]);
    for _ in 0..3 {
        g.spawn_test_unit("giant_death_robot", 1, capital_pos);
    }
    g.cities.get_mut(&city_state).unwrap().wall_hp = 0;
    g.cities.get_mut(&city_state).unwrap().hp = 25;
    g.spawn_test_unit("scout", 0, capital_pos);
    g.spawn_test_unit("scout", 0, city_state_pos);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.belief.observe(&g, 0);
    assert_eq!(ai.domination_capital_target(&g, 0), Some((1, capital)));
    let minor_value = ai.campaign_target_value_with_culture(&g, 0, minor, None);
    let major_value = ai.campaign_target_value_with_culture(&g, 0, 1, None);
    assert!(
        minor_value < major_value,
        "the generic scorer must prefer the detour: minor={minor_value}, major={major_value}"
    );
    (g, ai, minor, capital)
}

#[test]
fn domination_chooses_a_required_capital_before_a_cheaper_city_state() {
    let (g, ai, _, capital) = capital_and_cheaper_city_state();
    let plan = ai.assess(&g, 0);
    assert_eq!(plan.strategy, GrandStrategy::Conquest);
    assert_eq!(plan.target_player, Some(1));
    assert_eq!(plan.target_city, Some(capital));
}

#[test]
fn capital_priority_preserves_a_forced_target_and_an_existing_war() {
    let (mut g, mut ai, minor, _) = capital_and_cheaper_city_state();
    ai.forced_target_player = Some(minor);
    assert_eq!(ai.assess(&g, 0).target_player, Some(minor));
    ai.forced_target_player = None;
    g.at_war.insert((0, minor));
    assert_eq!(ai.assess(&g, 0).target_player, Some(minor));
}

#[test]
fn selecting_a_required_capital_still_waits_for_a_ready_army() {
    let (mut g, mut ai, _, capital) = capital_and_cheaper_city_state();
    ai.enable_war_policy_via_board();
    let plan = ai.assess(&g, 0);
    assert_eq!(plan.target_city, Some(capital));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        !g.is_at_war(0, 1),
        "a preparation target does not waive the war-readiness checks"
    );
}
