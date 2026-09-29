use super::*;
use std::sync::Arc;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new_full(4, 40, 24, 372100, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    g.found_city_for(0, (2, 8), None);
    g.found_city_for(2, (18, 8), None);
    g.found_city_for(3, (28, 8), None);
    g.players[1].religion = Some("Islam".into());
    for pid in [0, 1, 2] {
        Arc::make_mut(&mut g.observed_majority_religion).insert(pid, "Islam".into());
    }
    for rival in 1..4 {
        g.record_contact(0, rival);
    }
    g.at_war.clear();
    g.current = 0;
    g.turn = 83;
    g.players[0].gold = 10000.0;
    let defender = g.spawn_test_unit("spearman", 0, (4, 8));
    let missionary = g.spawn_test_unit("missionary", 1, (4, 8));
    g.units.get_mut(&missionary).unwrap().religion = Some("Islam".into());
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    assert!(ai.urgent_victory_threat(&g, 1));
    assert!(g.player_city_ids(1).is_empty());
    (g, ai, plan, defender, missionary)
}

#[test]
fn urgent_religious_interception_does_not_require_a_known_enemy_city() {
    let (mut g, mut ai, plan, _, missionary) = fixture();
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        g.is_at_war(0, 1),
        "the nearby spreader is an actionable religious counter"
    );
    assert!(
        !g.units.contains_key(&missionary),
        "the opening must execute its promised interception"
    );
}

#[test]
fn completed_interception_offers_peace_for_a_visible_conquest_campaign() {
    let (mut g, mut ai, mut plan, _, missionary) = fixture();
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.units.contains_key(&missionary));
    assert_eq!(ai.religious_interception_war, Some((1, 83)));

    // The one-war controller would otherwise keep this defensive front even
    // though its city is far away and another rival has a visible objective.
    let distant = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| {
            g.wdist(*pos, (2, 8)) >= 20
                && g.wdist(*pos, (18, 8)) >= 8
                && g.wdist(*pos, (28, 8)) >= 4
        })
        .unwrap();
    g.found_city_for(1, distant, None);
    ai.enable_one_war_at_a_time();
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(1));

    let objective = g.player_city_ids(2)[0];
    g.spawn_test_unit("scout", 0, (16, 8));
    plan.strategy = GrandStrategy::Conquest;
    plan.target_player = Some(2);
    plan.target_city = Some(objective);
    assert!(!ai.religious_interception_handoff_peace(&g, 0, 1, &plan));

    g.turn = g.peace_available_at(0, 1).unwrap();
    assert!(
        !ai.religious_interception_handoff_peace(&g, 0, 1, &plan),
        "the founder remains an urgent religious threat"
    );
    Arc::make_mut(&mut g.observed_majority_religion).remove(&2);
    assert!(ai.religious_interception_handoff_peace(&g, 0, 1, &plan));

    let home = g.player_city_ids(0)[0];
    g.host_observed = Arc::new(BTreeSet::from([
        g.cities[&home].pos,
        g.cities[&objective].pos,
    ]));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(ai.peace_offers.contains(&1));
    assert!(g.is_at_war(0, 1), "the host must confirm white peace");

    g.apply(0, &Action::MakePeace { player: 1 }).unwrap();
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert_eq!(ai.religious_interception_war, None);
}

#[test]
fn interception_peace_waits_without_an_alternative_city() {
    let (mut g, mut ai, plan, _, _) = fixture();
    ai.advanced_diplomacy(&mut g, 0, &plan);
    g.turn = g.peace_available_at(0, 1).unwrap();
    Arc::make_mut(&mut g.observed_majority_religion).remove(&2);
    assert!(!ai.religious_interception_handoff_peace(&g, 0, 1, &plan));
}

#[test]
fn religious_interception_preserves_nonurgent_and_executable_action_gates() {
    for control in 0..4 {
        let (mut g, mut ai, plan, defender, missionary) = fixture();
        match control {
            0 => {
                Arc::make_mut(&mut g.observed_majority_religion).remove(&2);
            }
            1 => {
                g.units.get_mut(&defender).unwrap().moves_left = 0.0;
            }
            2 => {
                ai = AdvancedAi::targeting(VictoryTarget::Science);
            }
            _ => {
                g.apply(0, &Action::DeclareWar { player: 2 }).unwrap();
            }
        }
        ai.advanced_diplomacy(&mut g, 0, &plan);
        assert!(!g.is_at_war(0, 1), "control {control}");
        assert!(g.units.contains_key(&missionary));
    }
}

#[test]
fn a_visible_spreader_keeps_the_religious_front_open_until_the_threat_recedes() {
    let (mut g, mut ai, _, _, missionary) = fixture();
    ai.enable_religious_veto_defence();
    let home = g.player_city_ids(0)[0];
    g.cities
        .get_mut(&home)
        .unwrap()
        .pressure
        .insert("Islam".into(), 1000.0);
    g.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
    assert!(ai.religious_interception_holds_war(&g, 0, 1));

    g.units.get_mut(&missionary).unwrap().pos = (18, 8);
    assert!(!ai.religious_interception_holds_war(&g, 0, 1));
    g.units.get_mut(&missionary).unwrap().pos = (4, 8);
    Arc::make_mut(&mut g.observed_majority_religion).remove(&2);
    assert!(!ai.religious_interception_holds_war(&g, 0, 1));

    Arc::make_mut(&mut g.observed_majority_religion).insert(2, "Islam".into());
    ai.disable_religious_veto_defence();
    assert!(!ai.religious_interception_holds_war(&g, 0, 1));
}
