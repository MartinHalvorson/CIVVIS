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

/// See `one_war_foothold_at_hand`: under the gene, an unwalled objective
/// with a taker beside it holds the victory-threat peace; walls, another
/// objective, or a taker four tiles out release it.
#[test]
fn an_open_foothold_holds_the_counter_peace_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    arm_the_front(&mut g);
    convert(&mut g, &[0, 1, 2]);
    g.at_war.remove(&(0, 2));
    let city = g.player_city_ids(1)[0];
    assert_eq!(g.cities[&city].wall_hp, 0, "the fixture's city is unwalled");
    let taker = g.spawn_test_unit("modern_armor", 0, (13, 12));
    let counter = Some(OneWarPeace::VictoryThreat);
    assert_eq!(ai.one_war_peace(&g, 0, 1), counter, "off");
    ai.enable_peace_waits_for_the_foothold();
    assert_eq!(ai.one_war_peace(&g, 0, 1), None, "held");
    g.cities.get_mut(&city).unwrap().wall_hp = 100;
    assert_eq!(ai.one_war_peace(&g, 0, 1), counter, "walls");
    g.cities.get_mut(&city).unwrap().wall_hp = 0;
    ai.plan.as_mut().unwrap().target_city = None;
    assert_eq!(ai.one_war_peace(&g, 0, 1), counter, "not the objective");
    ai.plan.as_mut().unwrap().target_city = Some(city);
    g.remove_unit(taker);
    g.spawn_test_unit("modern_armor", 0, (10, 12));
    assert_eq!(ai.one_war_peace(&g, 0, 1), counter, "too far");
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

    // A rival with no clock stays held by the one-war gate.
    let (mut g, mut ai) = two_fronts();
    g.at_war.remove(&(0, 2));
    ai.one_war_observe(&g, 0);
    assert!(!ai.domination_counter_target(&g, 0, 2));
    assert_eq!(ai.one_war_second_front(&g, 0), None);
    assert!(ai.one_war_holds_declaration(&g, 0, 2));
}

/// See `one_war_second_front`: beside a capital-prey front, a counter target
/// we outgun opens a second front before its clock turns urgent. Live King
/// civvis-20261003T155014Z (game 41) lost on Religion to a counter target it
/// outgunned twice over while the army stayed on the prey front.
#[test]
fn a_counter_target_opens_a_second_front_beside_a_prey_front() {
    let (mut g, mut ai) = two_fronts();
    g.at_war.remove(&(0, 2));
    // Our own cities follow the rival's faith: the faithless counter.
    convert(&mut g, &[0, 2]);
    ai.one_war_observe(&g, 0);
    assert!(ai.domination_capital_prey(&g, 0, 1));
    assert!(ai.domination_counter_target(&g, 0, 2));
    assert!(!ai.urgent_victory_threat(&g, 2), "fixture: not yet urgent");
    assert_eq!(ai.one_war_second_front(&g, 0), Some(2));
    assert!(!ai.one_war_holds_declaration(&g, 0, 2));
    assert_eq!(
        ai.assess(&g, 0).target_player,
        Some(2),
        "the plan aims at it"
    );

    // A front that can still fight back first gets its peace offer.
    let (mut g, mut ai) = two_fronts();
    arm_the_front(&mut g);
    g.at_war.remove(&(0, 2));
    convert(&mut g, &[0, 2]);
    ai.one_war_observe(&g, 0);
    assert!(!ai.domination_capital_prey(&g, 0, 1));
    assert_eq!(ai.one_war_second_front(&g, 0), None);
}

/// See `faith_counter_due`: the second front on a faith taking our cities is
/// declared without a staged siege; the war itself is the counter.
#[test]
fn a_faith_taking_our_cities_is_declared_on_without_a_staged_siege() {
    let (mut g, mut ai) = two_fronts();
    // A declaration needs a second city of ours.
    g.found_city_for(0, (6, 18), None);
    g.at_war.remove(&(0, 2));
    convert(&mut g, &[0, 2]);
    ai.one_war_observe(&g, 0);
    assert!(ai.faith_counter_due(&g, 0, 2));
    let plan = ai.assess(&g, 0);
    assert_eq!(plan.target_player, Some(2));
    // The coalition invitation holds a turn for answers; it is not the gate
    // under test.
    ai.coalition_before_war = false;
    ai.coalition_before_war_2 = false;
    ai.coalition_before_war_3 = false;
    // No spreader stands in our land, so the opening is the denouncement
    // and the Formal War follows its preparation period.
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 2));
    for _ in 0..12 {
        if g.is_at_war(0, 2) {
            break;
        }
        g.turn += 1;
        let plan = ai.assess(&g, 0);
        ai.advanced_diplomacy(&mut g, 0, &plan);
    }
    assert!(g.is_at_war(0, 2), "declared beside the prey front");

    // Without the power margin the war waits for its siege.
    let (mut g, mut ai) = two_fronts();
    g.at_war.remove(&(0, 2));
    convert(&mut g, &[0, 2]);
    for y in [4, 5, 6, 7, 8, 9] {
        g.spawn_test_unit("modern_armor", 2, (26, y));
    }
    ai.one_war_observe(&g, 0);
    assert!(!ai.faith_counter_due(&g, 0, 2));
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

/// See `second_front_kept_when_winning`: under the gene, a second war on a
/// rival we crush is kept, and one we are winning while we hold a city of
/// theirs; one we merely outgun is still closed.
#[test]
fn a_beaten_second_front_is_kept_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    arm_the_front(&mut g);
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(1));
    assert!(g.military_power(0) >= ONE_WAR_CRUSHED_RATIO * g.military_power(2));
    let second = Some(OneWarPeace::SecondFront);
    assert_eq!(ai.one_war_peace(&g, 0, 2), second, "off");
    ai.enable_second_front_kept_when_winning();
    assert_eq!(ai.one_war_peace(&g, 0, 2), None, "crushed");
    let mut row = 2;
    while g.military_power(0) >= ONE_WAR_CRUSHED_RATIO * g.military_power(2) {
        g.spawn_test_unit("modern_armor", 2, (30, row));
        row += 1;
    }
    assert!(g.military_power(0) >= ONE_WAR_WINNING_RATIO * g.military_power(2));
    assert_eq!(ai.one_war_peace(&g, 0, 2), second, "winning, nothing taken");
    let taken = g.found_city_for(2, (27, 16), None);
    g.cities.get_mut(&taken).unwrap().owner = 0;
    assert_eq!(ai.one_war_peace(&g, 0, 2), None, "winning, a city taken");
}

/// Live King civvis-20261004T025448Z (game 45): a counter the war cannot
/// answer without a siege (a culture or science clock) takes no second front
/// before it is urgent, so the army stays on the prey front's siege.
#[test]
fn a_slow_clock_takes_no_second_front_beside_a_prey_front() {
    let (mut g, mut ai) = two_fronts();
    g.at_war.remove(&(0, 2));
    let stats = std::sync::Arc::make_mut(&mut g.observed_public_empire_stats);
    for pid in 0..4 {
        stats.insert(
            pid,
            crate::game::ObservedPublicEmpireStats {
                domestic_tourists: Some(100),
                foreign_tourists: Some(if pid == 2 { 60 } else { 0 }),
                ..Default::default()
            },
        );
    }
    ai.one_war_observe(&g, 0);
    assert!(ai.domination_capital_prey(&g, 0, 1));
    assert!(ai.domination_counter_target(&g, 0, 2));
    assert!(!ai.urgent_victory_threat(&g, 2));
    assert!(!ai.faith_counter(&g, 0, 2));
    assert_eq!(ai.one_war_second_front(&g, 0), None);
}

/// See `ONE_WAR_SECOND_FRONT_HOLD_RATIO`: a second front once named holds
/// below the opening ratio, so the pick does not flicker on the line.
#[test]
fn a_named_second_front_holds_below_the_opening_ratio() {
    let (mut g, mut ai) = two_fronts();
    g.at_war.remove(&(0, 2));
    convert(&mut g, &[0, 2]);
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_second_front(&g, 0), Some(2));
    assert_eq!(ai.one_war_second, Some(2));
    let mut row = 2;
    while g.military_power(0) >= ONE_WAR_SECOND_FRONT_RATIO * g.military_power(2).max(1.0) {
        g.spawn_test_unit("warrior", 2, (30, row));
        row += 1;
    }
    assert!(
        g.military_power(0) >= ONE_WAR_SECOND_FRONT_HOLD_RATIO * g.military_power(2),
        "fixture: between the two ratios"
    );
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_second_front(&g, 0), Some(2), "held");
    assert!(ai.faith_counter_due(&g, 0, 2));
    // Not named before: the opening ratio applies.
    ai.one_war_second = None;
    assert_eq!(ai.one_war_second_front(&g, 0), None);
    assert!(!ai.faith_counter_due(&g, 0, 2));
}

/// See `second_front_waits_for_the_front`: beside a live front siege the
/// faith counter is declared on, but the plan stays on the front.
#[test]
fn a_faith_counter_waits_for_the_front_siege_but_is_declared_on() {
    let (mut g, mut ai) = two_fronts();
    g.found_city_for(0, (6, 18), None);
    g.at_war.remove(&(0, 2));
    convert(&mut g, &[0, 2]);
    let front_city = g.player_city_ids(1)[0];
    ai.sieges.insert(
        front_city,
        crate::ai::advanced::siege_train::Siege {
            stage: crate::ai::advanced::siege_train::SiegeStage::Invest,
            taker: None,
            entered: g.turn - 2,
            assessed: g.turn,
            posts: Default::default(),
            short_since: None,
        },
    );
    ai.coalition_before_war = false;
    ai.coalition_before_war_2 = false;
    ai.coalition_before_war_3 = false;
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_second_front(&g, 0), Some(2));
    assert!(ai.second_front_waits_for_the_front(&g, 0, 2));
    let plan = ai.assess(&g, 0);
    assert_eq!(plan.target_player, Some(1), "the army stays on the front");
    for _ in 0..12 {
        if g.is_at_war(0, 2) {
            break;
        }
        g.turn += 1;
        ai.sieges.get_mut(&front_city).unwrap().assessed = g.turn;
        let plan = ai.assess(&g, 0);
        assert_eq!(plan.target_player, Some(1));
        ai.advanced_diplomacy(&mut g, 0, &plan);
    }
    assert!(g.is_at_war(0, 2), "the faith counter is declared on");

    // Without a live front siege the plan hands over at once.
    let (mut g, mut ai) = two_fronts();
    g.at_war.remove(&(0, 2));
    convert(&mut g, &[0, 2]);
    ai.one_war_observe(&g, 0);
    assert!(!ai.second_front_waits_for_the_front(&g, 0, 2));
    assert_eq!(ai.assess(&g, 0).target_player, Some(2));
}

/// Whether rival 2 has grown past `COUNTER_WAR_POWER_FLOOR` of our power.
fn ai_probe_hopeless(g: &Game) -> bool {
    g.military_power(0) < COUNTER_WAR_POWER_FLOOR * g.military_power(2)
}

/// See `COUNTER_WAR_POWER_FLOOR`: an urgent faith too strong to fight is
/// neither declared on nor freed for. (The fixture's clock is religious.)
#[test]
fn a_counter_war_below_the_power_floor_is_neither_opened_nor_freed_for() {
    let strengthen = |g: &mut Game| {
        let mut row = 2;
        while !ai_probe_hopeless(g) {
            g.spawn_test_unit("modern_armor", 2, (30, row));
            row += 1;
        }
    };
    // The declaration: at peace, the urgent rival is declared on only while
    // the war is winnable.
    for strong in [false, true] {
        let (mut g, mut ai) = two_fronts();
        g.found_city_for(0, (6, 18), None);
        g.at_war.clear();
        convert(&mut g, &[0, 1, 2]);
        if strong {
            strengthen(&mut g);
        }
        ai.coalition_before_war = false;
        ai.coalition_before_war_2 = false;
        ai.coalition_before_war_3 = false;
        ai.one_war_observe(&g, 0);
        assert!(ai.urgent_victory_threat(&g, 2));
        let plan = ai.assess(&g, 0);
        assert_eq!(plan.target_player, Some(2));
        ai.advanced_diplomacy(&mut g, 0, &plan);
        assert_eq!(g.is_at_war(0, 2), !strong, "strong rival: {strong}");
    }
    // The front is not traded for it either.
    let (mut g, mut ai) = two_fronts();
    arm_the_front(&mut g);
    g.at_war.remove(&(0, 2));
    convert(&mut g, &[0, 1, 2]);
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_peace(&g, 0, 1), Some(OneWarPeace::VictoryThreat));
    strengthen(&mut g);
    ai.one_war_observe(&g, 0);
    assert_ne!(ai.one_war_peace(&g, 0, 1), Some(OneWarPeace::VictoryThreat));
}

/// See `FRONT_SIEGE_LIVE_TURNS`: a front siege whose city has shown no new
/// low of health for the window no longer holds the faith counter back
/// (Madrid, game 53, besieged turns 44-152).
#[test]
fn a_stalled_front_siege_stops_holding_the_faith_counter() {
    let (mut g, mut ai) = two_fronts();
    g.found_city_for(0, (6, 18), None);
    g.at_war.remove(&(0, 2));
    convert(&mut g, &[0, 2]);
    let front_city = g.player_city_ids(1)[0];
    ai.sieges.insert(
        front_city,
        crate::ai::advanced::siege_train::Siege {
            stage: crate::ai::advanced::siege_train::SiegeStage::Invest,
            taker: None,
            entered: g.turn - 2,
            assessed: g.turn,
            posts: Default::default(),
            short_since: None,
        },
    );
    ai.one_war_observe(&g, 0);
    assert!(
        ai.second_front_waits_for_the_front(&g, 0, 2),
        "a fresh siege"
    );
    // The city stands at the health it showed: no new low.
    g.turn += g.standard_duration(FRONT_SIEGE_LIVE_TURNS) + 1;
    ai.sieges.get_mut(&front_city).unwrap().assessed = g.turn;
    ai.one_war_observe(&g, 0);
    assert!(!ai.front_siege_live(&g), "a siege that only stands");
    assert!(!ai.second_front_waits_for_the_front(&g, 0, 2));
    // A blow that sets a new low makes it live again.
    g.cities.get_mut(&front_city).unwrap().hp -= 30;
    ai.one_war_observe(&g, 0);
    assert!(ai.second_front_waits_for_the_front(&g, 0, 2));
}

/// See `front_siege_to_finish`: under the gene, an urgent counter already at
/// war waits for the front's live siege of an unwalled city; walls, or a
/// siege still in Stage, release the army.
#[test]
fn the_front_finishes_its_siege_before_an_urgent_counter() {
    use crate::ai::advanced::siege_train::{Siege, SiegeStage};
    let front_after = |gene: bool, walls: i32, stage: SiegeStage| {
        let (mut g, mut ai) = two_fronts();
        convert(&mut g, &[0, 1, 2]);
        assert!(ai.urgent_victory_threat(&g, 2));
        if gene {
            ai.enable_front_finishes_its_siege();
        }
        let front_city = g.player_city_ids(1)[0];
        g.cities.get_mut(&front_city).unwrap().wall_hp = walls;
        let siege = Siege {
            stage,
            taker: None,
            entered: g.turn - 1,
            assessed: g.turn,
            posts: Default::default(),
            short_since: None,
        };
        ai.sieges.insert(front_city, siege);
        ai.one_war_observe(&g, 0);
        ai.one_war_front()
    };
    // Off, the counter moves the army; on, the siege is finished first.
    assert_eq!(front_after(false, 0, SiegeStage::Invest), Some(2));
    assert_eq!(front_after(true, 0, SiegeStage::Invest), Some(1));
    // A walled city does not hold it, nor does a siege still staging.
    assert_eq!(front_after(true, 100, SiegeStage::Invest), Some(2));
    assert_eq!(front_after(true, 0, SiegeStage::Stage), Some(2));
}

/// The live seat renumbers cities every turn; the front's health reading is
/// keyed by tile, so a renumbered city still reads as itself.
#[test]
fn the_front_reads_city_health_by_tile() {
    let (g, mut ai) = two_fronts();
    ai.one_war_observe(&g, 0);
    let front = ai.one_war.as_ref().expect("a front");
    let city = &g.cities[&g.player_city_ids(front.target)[0]];
    assert_eq!(
        front.city_health.get(&city.pos),
        Some(&(city.hp, city.wall_hp))
    );
}

/// See `stalled_front_swap`: a front whose cities have shown no new low of
/// health for the stall window yields, under the gene, to a weak enemy
/// holding a capital Domination needs (the Maori, game 65).
#[test]
fn a_stalled_front_yields_to_a_weak_enemy_holding_a_needed_capital() {
    for gene in [false, true] {
        let (mut g, mut ai) = two_fronts();
        if gene {
            ai.enable_one_war_swaps_a_stalled_front();
        }
        // Rival 1, the front, is strong enough to stall us; rival 2 is weak.
        arm_the_front(&mut g);
        g.turn += g.standard_duration(FRONT_STALL_TURNS) + 1;
        ai.one_war_observe(&g, 0);
        let expected = if gene { Some(2) } else { Some(1) };
        assert_eq!(ai.one_war_front(), expected, "gene {gene}");
    }
}

/// See `counter_war_hopeless`: a rival whose faith holds our majority is a
/// religious clock for the power floor even when another lane leads.
#[test]
fn a_faith_holding_our_majority_is_religious_for_the_power_floor() {
    let (mut g, ai) = two_fronts();
    convert(&mut g, &[0, 2]);
    assert!(g.civ_follows_religion(0, "islam"));
    // Make rival 2 strong enough that we stand under the floor.
    let mut row = 2;
    while g.military_power(0) >= COUNTER_WAR_POWER_FLOOR * g.military_power(2) {
        g.spawn_test_unit("modern_armor", 2, (30, row));
        row += 1;
    }
    assert!(ai.counter_war_hopeless(&g, 0, 2));
    // Without the faith in our cities, a non-religious lead is not hopeless.
    let mut free = g.clone();
    free.players[2].religion = None;
    if ai.rival_pressure(&free, 2).0 != GrandStrategy::Religion {
        assert!(!ai.counter_war_hopeless(&free, 0, 2));
    }
}
