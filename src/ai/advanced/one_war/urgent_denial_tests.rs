use super::*;
use crate::ai::advanced::StrategicPlan;

fn two_fronts() -> (Game, AdvancedAi) {
    let mut g = Game::new_full(4, 40, 24, 936_034, 500, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    for (owner, x) in [6, 14, 23, 32].into_iter().enumerate() {
        g.found_city_for(owner, (x, 12), None);
        if owner != 0 {
            g.record_contact(0, owner);
        }
    }
    for _ in 0..4 {
        g.spawn_test_unit("modern_armor", 0, (8, 12));
    }
    g.spawn_test_unit("warrior", 2, (23, 12));
    g.at_war.insert((0, 1));
    g.at_war.insert((0, 2));
    g.current = 0;
    g.turn = 110;
    let mut ai = AdvancedAi::new();
    ai.retarget(VictoryTarget::Domination);
    ai.enable_one_war_at_a_time();
    ai.deny_leaders = true;
    ai.deny_while_targeted = true;
    ai.battlefront_observation = true;
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: g.player_city_ids(1).first().copied(),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    });
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(1));
    (g, ai)
}

fn convert(g: &mut Game, owners: &[usize]) {
    g.players[2].religion = Some("islam".to_string());
    for city in g.cities.values_mut() {
        city.pressure.clear();
        let religion = if owners.contains(&city.owner) {
            "islam"
        } else {
            "taoism"
        };
        city.pressure.insert(religion.to_string(), 10_000.0);
    }
}

#[test]
fn urgent_military_denial_replaces_the_sticky_front_and_peace_target() {
    let (mut g, mut ai) = two_fronts();
    convert(&mut g, &[0, 1, 2]);
    assert!(ai.urgent_victory_threat(&g, 2));
    assert_eq!(
        ai.actionable_victory_denial(&g, 0),
        Some((2, GrandStrategy::Conquest))
    );
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(2));
    assert_eq!(ai.assess(&g, 0).target_player, Some(2));
    assert_eq!(ai.one_war_peace(&g, 0, 1), Some(OneWarPeace::SecondFront));
    assert_eq!(ai.one_war_peace(&g, 0, 2), None);
    let plan = ai.assess(&g, 0);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g
        .pending_deals
        .iter()
        .any(|d| d.from == 0 && d.to == 1 && d.peace));
    assert!(!g
        .pending_deals
        .iter()
        .any(|d| d.from == 0 && d.to == 2 && d.peace));
}

#[test]
fn nonurgent_or_disabled_denial_does_not_switch_fronts() {
    for disabled in [false, true] {
        let (mut g, mut ai) = two_fronts();
        if disabled {
            convert(&mut g, &[0, 1, 2]);
            ai.deny_leaders = false;
        } else {
            convert(&mut g, &[0, 2]);
            assert!(!ai.urgent_victory_threat(&g, 2));
        }
        ai.one_war_observe(&g, 0);
        assert_eq!(ai.one_war_front(), Some(1));
    }
}

#[test]
fn a_religious_counter_does_not_reassign_the_army() {
    let (mut g, mut ai) = two_fronts();
    ai.retarget(VictoryTarget::Religion);
    convert(&mut g, &[0, 1, 2]);
    g.players[0].religion = Some("taoism".to_string());
    assert_eq!(
        ai.actionable_victory_denial(&g, 0),
        Some((2, GrandStrategy::Religion))
    );
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(1));
}

#[test]
fn an_urgent_rival_at_peace_does_not_replace_an_active_front() {
    let (mut g, mut ai) = two_fronts();
    convert(&mut g, &[0, 1, 2]);
    g.at_war.remove(&(0, 2));
    assert_eq!(
        ai.actionable_victory_denial(&g, 0),
        Some((2, GrandStrategy::Conquest))
    );
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(1));
    assert!(!g.is_at_war(0, 2));
}

#[test]
fn an_explicit_target_order_prevents_automatic_reassignment() {
    let (mut g, mut ai) = two_fronts();
    convert(&mut g, &[0, 1, 2]);
    ai.forced_target_player = Some(1);
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(1));
}

#[test]
fn fading_urgency_keeps_the_new_front_but_a_rout_still_allows_peace() {
    let (mut g, mut ai) = two_fronts();
    convert(&mut g, &[0, 1, 2]);
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(2));
    let since = ai.one_war.as_ref().unwrap().since;
    g.turn += 1;
    convert(&mut g, &[0, 2]);
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(2));
    assert_eq!(ai.one_war.as_ref().unwrap().since, since);
    convert(&mut g, &[0, 1, 2]);
    ai.one_war.as_mut().unwrap().window = VecDeque::from([(g.turn, ONE_WAR_ROUT_NET)]);
    assert_eq!(ai.one_war_peace(&g, 0, 2), Some(OneWarPeace::Rout));
}

#[test]
fn a_cityless_threat_cannot_take_over_the_military_front() {
    let (mut g, mut ai) = two_fronts();
    for id in g.player_city_ids(2) {
        g.cities.remove(&id);
    }
    convert(&mut g, &[0, 1, 3]);
    assert!(ai.urgent_victory_threat(&g, 2));
    assert_eq!(ai.actionable_victory_denial(&g, 0), None);
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(1));
}

#[test]
fn domination_founders_redirect_the_army_against_a_religious_match_point() {
    let (mut g, mut ai) = two_fronts();
    convert(&mut g, &[0, 1, 2]);
    g.players[0].religion = Some("taoism".to_string());
    assert_eq!(
        ai.actionable_victory_denial(&g, 0),
        Some((2, GrandStrategy::Conquest))
    );
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(2));
    assert_eq!(ai.assess(&g, 0).target_player, Some(2));
    assert_eq!(ai.one_war_peace(&g, 0, 1), Some(OneWarPeace::SecondFront));
    assert_eq!(ai.one_war_peace(&g, 0, 2), None);
}

#[test]
fn domination_seeks_peace_to_free_the_army_but_keeps_the_front_until_acceptance() {
    for culture in [false, true] {
        let (mut g, mut ai) = two_fronts();
        if culture {
            let stats = std::sync::Arc::make_mut(&mut g.observed_public_empire_stats);
            for pid in 0..4 {
                stats.insert(
                    pid,
                    crate::game::ObservedPublicEmpireStats {
                        domestic_tourists: Some(100),
                        foreign_tourists: Some(if pid == 2 { 90 } else { 0 }),
                        ..Default::default()
                    },
                );
            }
        } else {
            convert(&mut g, &[0, 1, 2]);
        }
        g.at_war.remove(&(0, 2));
        ai.one_war_observe(&g, 0);
        assert_eq!(ai.one_war_front(), Some(1));
        assert_eq!(ai.one_war_peace(&g, 0, 1), Some(OneWarPeace::VictoryThreat));
        let plan = ai.assess(&g, 0);
        assert_eq!(plan.target_player, Some(1));
        ai.advanced_diplomacy(&mut g, 0, &plan);
        assert!(g
            .pending_deals
            .iter()
            .any(|d| d.from == 0 && d.to == 1 && d.peace));
        // An offer is not acceptance: the current enemy remains the army front.
        assert!(g.is_at_war(0, 1));
        assert_eq!(ai.one_war_front(), Some(1));
        g.at_war.remove(&(0, 1));
        ai.one_war_observe(&g, 0);
        assert_eq!(ai.assess(&g, 0).target_player, Some(2));
    }
}

#[test]
fn domination_counter_peace_respects_explicit_targets_and_other_victory_lanes() {
    let (mut g, mut ai) = two_fronts();
    convert(&mut g, &[0, 1, 2]);
    g.at_war.remove(&(0, 2));
    ai.forced_target_player = Some(1);
    assert_eq!(ai.one_war_peace(&g, 0, 1), None);
    ai.forced_target_player = None;
    ai.retarget(VictoryTarget::Science);
    assert_eq!(ai.one_war_peace(&g, 0, 1), None);
}
