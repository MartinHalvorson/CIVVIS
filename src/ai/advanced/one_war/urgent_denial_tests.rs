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

/// Give the front an army near ours in strength, so it is not capital prey
/// (`domination_capital_prey`) and the VictoryThreat peace still applies.
fn arm_the_front(g: &mut Game) {
    for y in [20, 21, 22] {
        g.spawn_test_unit("modern_armor", 1, (16, y));
    }
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
    // A front we still outgun twice over is being won, whatever the window.
    assert_eq!(ai.one_war_peace(&g, 0, 2), None);
    for _ in 0..3 {
        g.spawn_test_unit("modern_armor", 2, (25, 12));
    }
    assert!(g.military_power(0) < ONE_WAR_WINNING_RATIO * g.military_power(2));
    assert_eq!(ai.one_war_peace(&g, 0, 2), Some(OneWarPeace::Rout));
    // A siege reducing one of its cities is being won too.
    let city = g.player_city_ids(2)[0];
    ai.sieges.insert(
        city,
        crate::ai::advanced::siege_train::Siege {
            stage: crate::ai::advanced::siege_train::SiegeStage::Reduce,
            taker: None,
            entered: g.turn,
            assessed: g.turn,
            posts: Default::default(),
            short_since: None,
        },
    );
    assert_eq!(ai.one_war_peace(&g, 0, 2), None);
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
        arm_the_front(&mut g);
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

#[test]
fn domination_finishes_a_breached_city_before_peace_for_another_rival() {
    let (mut g, ai) = two_fronts();
    arm_the_front(&mut g);
    convert(&mut g, &[0, 1, 2]);
    g.at_war.remove(&(0, 2));
    let city = g.player_city_ids(1)[0];
    g.cities.get_mut(&city).unwrap().hp = ONE_WAR_FINISH_HP;
    let finisher = g.spawn_test_unit("modern_armor", 0, (13, 12));
    assert_eq!(
        ai.actionable_victory_denial(&g, 0),
        Some((2, GrandStrategy::Conquest))
    );
    assert_eq!(ai.one_war_peace(&g, 0, 1), None);

    g.remove_unit(finisher);
    assert_eq!(
        ai.one_war_peace(&g, 0, 1),
        Some(OneWarPeace::VictoryThreat),
        "a distant army must counter the urgent rival"
    );
    g.spawn_test_unit("modern_armor", 0, (13, 12));
    g.cities.get_mut(&city).unwrap().hp = ONE_WAR_FINISH_HP + 1;
    assert_eq!(ai.one_war_peace(&g, 0, 1), Some(OneWarPeace::VictoryThreat));
    g.cities.get_mut(&city).unwrap().hp = ONE_WAR_FINISH_HP;
    g.cities.get_mut(&city).unwrap().wall_hp = 1;
    assert_eq!(ai.one_war_peace(&g, 0, 1), Some(OneWarPeace::VictoryThreat));
}

/// On the live Cree front, peace was offered for a stale war counter while
/// the capital had 41 HP, no walls, and a healthy infantry four tiles away.
/// The same bounded capture window must hold against fatigue as well as an
/// unrelated rival's victory clock.
#[test]
fn domination_finish_at_hand_outlasts_the_stalled_war_counter() {
    let (mut g, mut ai) = two_fronts();
    let city = g.player_city_ids(1)[0];
    g.cities.get_mut(&city).unwrap().hp = ONE_WAR_FINISH_HP;
    let finisher = g.spawn_test_unit("modern_armor", 0, (11, 12));
    ai.one_war.as_mut().unwrap().tide_against_since = Some(g.turn - 2);
    assert!(ai.one_war_presses(&g, 0, 1));

    g.cities.get_mut(&city).unwrap().hp = ONE_WAR_FINISH_HP + 1;
    assert!(!ai.one_war_presses(&g, 0, 1));
    g.cities.get_mut(&city).unwrap().hp = ONE_WAR_FINISH_HP;
    g.remove_unit(finisher);
    assert!(!ai.one_war_presses(&g, 0, 1));
}

#[test]
fn domination_keeps_a_crushed_front_until_the_counter_is_urgent() {
    let (mut g, mut ai) = two_fronts();
    g.at_war.remove(&(0, 2));
    let culture = |g: &mut Game, visiting: usize| {
        let stats = std::sync::Arc::make_mut(&mut g.observed_public_empire_stats);
        for pid in 0..4 {
            stats.insert(
                pid,
                crate::game::ObservedPublicEmpireStats {
                    domestic_tourists: Some(100),
                    foreign_tourists: Some(if pid == 2 { visiting } else { 0 }),
                    ..Default::default()
                },
            );
        }
    };
    // A 60% culture reading: a Domination counter target, not yet urgent.
    culture(&mut g, 60);
    assert_eq!(
        ai.actionable_victory_denial(&g, 0),
        Some((2, GrandStrategy::Conquest))
    );
    assert!(!ai.urgent_victory_threat(&g, 2));
    assert!(ai.one_war_front_crushed(&g, 0, 1));
    assert_eq!(
        ai.one_war_peace(&g, 0, 1),
        None,
        "a beaten front is not traded away for a slow clock"
    );
    // The same clock at match point is answered beside the crushed front,
    // which holds its capital at our mercy: it is kept, not traded
    // (`domination_capital_prey`), and the urgent rival opens a second front.
    culture(&mut g, 90);
    assert!(ai.urgent_victory_threat(&g, 2));
    assert!(ai.domination_capital_prey(&g, 0, 1));
    assert_eq!(ai.one_war_peace(&g, 0, 1), None);
    assert_eq!(ai.one_war_second_front(&g, 0), Some(2));
    // A front that can still fight back is traded as before.
    culture(&mut g, 60);
    for _ in 0..4 {
        g.spawn_test_unit("modern_armor", 1, (15, 12));
    }
    assert!(!ai.one_war_front_crushed(&g, 0, 1));
    // A front chosen this turn is kept against a clock that is not urgent
    // (`ONE_WAR_FRESH_FRONT_TURNS`); an older one is traded.
    assert_eq!(ai.one_war_peace(&g, 0, 1), None);
    let aged = g.turn - g.standard_duration(ONE_WAR_FRESH_FRONT_TURNS);
    ai.one_war.as_mut().unwrap().since = aged;
    assert_eq!(ai.one_war_peace(&g, 0, 1), Some(OneWarPeace::VictoryThreat));
}

/// See `one_war_second_front`: a rival at peace whose clock is urgent may be
/// declared on beside the burning war when we outgun it; an ordinary rival
/// stays held by the one-war gate.
#[test]
fn an_urgent_rival_at_peace_opens_a_second_front() {
    // A capital-prey front is kept, so the urgent rival opens beside it at
    // once and the front is never offered the peace.
    let (mut g, mut ai) = two_fronts();
    g.at_war.remove(&(0, 2));
    convert(&mut g, &[0, 1, 2]);
    ai.one_war_observe(&g, 0);
    assert!(ai.domination_capital_prey(&g, 0, 1));
    assert_eq!(ai.one_war_peace(&g, 0, 1), None);
    assert_eq!(ai.one_war_second_front(&g, 0), Some(2));

    let (mut g, mut ai) = two_fronts();
    arm_the_front(&mut g);
    g.at_war.remove(&(0, 2));
    convert(&mut g, &[0, 1, 2]);
    ai.one_war_observe(&g, 0);
    assert_eq!(
        ai.one_war_front(),
        Some(1),
        "the burning war stays the front"
    );
    assert!(ai.urgent_victory_threat(&g, 2));
    assert_eq!(
        ai.one_war_second_front(&g, 0),
        None,
        "the front first gets its chance to accept the peace"
    );
    for _ in 0..g.standard_duration(ONE_WAR_SECOND_FRONT_PATIENCE).max(1) {
        g.turn += 1;
        ai.one_war_observe(&g, 0);
    }
    assert_eq!(ai.one_war_front(), Some(1));
    assert_eq!(ai.one_war_second_front(&g, 0), Some(2));
    assert!(!ai.one_war_holds_declaration(&g, 0, 2));
    assert_eq!(
        ai.assess(&g, 0).target_player,
        Some(2),
        "the plan aims at it"
    );

    let (mut g, mut ai) = two_fronts();
    g.at_war.remove(&(0, 2));
    convert(&mut g, &[0, 2]);
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_second_front(&g, 0), None);
    assert!(ai.one_war_holds_declaration(&g, 0, 2));
}

/// The conquest opening's declared war is not traded for another rival's
/// victory clock; the opening's own peace rules decide it.
#[test]
fn the_conquest_opening_war_is_not_traded_for_a_victory_threat() {
    let (mut g, mut ai) = two_fronts();
    arm_the_front(&mut g);
    convert(&mut g, &[0, 1, 2]);
    g.at_war.remove(&(0, 2));
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_peace(&g, 0, 1), Some(OneWarPeace::VictoryThreat));
    let city = g.player_city_ids(1)[0];
    ai.early_conquest_opening = true;
    ai.conquest_opening = Some(crate::ai::advanced::early_conquest::ConquestOpening {
        target: 1,
        city,
        opened: g.turn.saturating_sub(20),
        preparing_since: None,
        grace_until: None,
        rally: (10, 12),
        force: Default::default(),
        assembled: Some(g.turn.saturating_sub(10)),
        declared: Some(g.turn.saturating_sub(8)),
        kills_at_war: 0,
        losses: 0,
        taken: 0,
    });
    assert_eq!(ai.one_war_peace(&g, 0, 1), None, "the opening keeps its war");
    for y in [2, 3, 4, 5] {
        g.spawn_test_unit("modern_armor", 0, (4, y));
    }
    assert!(g.military_power(0) >= ONE_WAR_SECOND_FRONT_RATIO * g.military_power(2).max(1.0));
    assert_eq!(
        ai.one_war_second_front(&g, 0),
        Some(2),
        "the urgent rival opens beside the opening's war"
    );
}

/// See `second_front_war_kept`: a war on an urgent rival beside the front is
/// not offered "one war at a time" peace; a war on a rival whose clock is
/// not a counter still is.
#[test]
fn a_second_front_on_an_urgent_rival_is_kept() {
    let (mut g, mut ai) = two_fronts();
    arm_the_front(&mut g);
    // Both wars run (the fixture is at war with 1 and 2); the front is 1.
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(1));
    assert_eq!(
        ai.one_war_peace(&g, 0, 2),
        Some(OneWarPeace::SecondFront),
        "the control: a plain second war is closed"
    );
    convert(&mut g, &[0, 1, 2]);
    assert!(ai.urgent_victory_threat(&g, 2));
    assert!(ai.second_front_war_kept(&g, 0, 2));
    assert_eq!(ai.one_war_peace(&g, 0, 2), None);
}
