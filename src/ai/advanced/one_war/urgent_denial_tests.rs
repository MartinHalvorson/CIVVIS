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

/// See `second_front_kept_when_winning_2`: under version two, a war we are
/// winning on a rival that still holds its own capital is kept without a
/// city taken; a rival without its capital is still offered peace.
#[test]
fn a_winning_second_front_on_a_standing_capital_is_kept_under_version_two() {
    let (mut g, mut ai) = two_fronts();
    arm_the_front(&mut g);
    let mut row = 2;
    while g.military_power(0) >= ONE_WAR_CRUSHED_RATIO * g.military_power(2) {
        g.spawn_test_unit("modern_armor", 2, (30, row));
        row += 1;
    }
    assert!(g.military_power(0) >= ONE_WAR_WINNING_RATIO * g.military_power(2));
    ai.one_war_observe(&g, 0);
    let second = Some(OneWarPeace::SecondFront);
    ai.enable_second_front_kept_when_winning();
    assert_eq!(ai.one_war_peace(&g, 0, 2), second, "version one");
    ai.enable_second_front_kept_when_winning_2();
    assert_eq!(ai.one_war_peace(&g, 0, 2), None, "its capital stands");
    let capital = g.player_city_ids(2)[0];
    assert!(g.cities[&capital].is_capital);
    g.cities.get_mut(&capital).unwrap().owner = 3;
    assert_eq!(ai.one_war_peace(&g, 0, 2), second, "its capital is gone");
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

/// See `rout_spares_a_stronger_army`: under the gene, a rout window against a
/// rival we outgun 1.5 times over offers no peace; below the margin it does.
#[test]
fn a_rout_spares_a_stronger_army_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    arm_the_front(&mut g);
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(1));
    while g.military_power(0) < ONE_WAR_SECOND_FRONT_RATIO * g.military_power(1) {
        g.spawn_test_unit("modern_armor", 0, (8, 13));
    }
    assert!(g.military_power(0) < ONE_WAR_WINNING_RATIO * g.military_power(1));
    ai.one_war.as_mut().unwrap().window = VecDeque::from([(g.turn, ONE_WAR_ROUT_NET)]);
    let mut row = 2;
    let rout = Some(OneWarPeace::Rout);
    assert_eq!(ai.one_war_peace(&g, 0, 1), rout, "off");
    ai.enable_rout_spares_a_stronger_army();
    assert_eq!(ai.one_war_peace(&g, 0, 1), None, "1.5 times over");
    while g.military_power(0) >= ONE_WAR_SECOND_FRONT_RATIO * g.military_power(1) {
        g.spawn_test_unit("modern_armor", 1, (16, row));
        row += 1;
    }
    assert_eq!(ai.one_war_peace(&g, 0, 1), rout, "under the margin");
}

/// See `last_capital_war_kept`: under the gene, the war on the one rival that
/// still holds an original capital, every other one ours, is kept while we
/// are its equal; another capital still out, or a stronger rival, frees it.
#[test]
fn the_war_for_the_last_capital_is_kept_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    let capital_of = |g: &Game, owner: usize| {
        g.cities
            .values()
            .find(|city| city.is_capital && city.original_owner == owner)
            .map(|city| city.id)
            .unwrap()
    };
    let first = capital_of(&g, 1);
    g.cities.get_mut(&first).unwrap().owner = 0;
    assert!(!ai.last_capital_war_kept(&g, 0, 2), "off");
    ai.enable_last_capital_war_kept();
    assert!(
        !ai.last_capital_war_kept(&g, 0, 2),
        "a third capital is still out"
    );
    let third = capital_of(&g, 3);
    g.cities.get_mut(&third).unwrap().owner = 0;
    assert!(ai.last_capital_war_kept(&g, 0, 2), "the last capital");
    assert!(!ai.last_capital_war_kept(&g, 0, 1), "not the holder");
    while g.military_power(2) <= g.military_power(0) {
        g.spawn_test_unit("modern_armor", 2, (30, 2));
    }
    assert!(!ai.last_capital_war_kept(&g, 0, 2), "a stronger holder");
}

/// See `diplomatic_contender`: under the gene, a crushed rival at fifteen
/// Diplomatic Victory points keeps its war, and at sixteen, the leader, it
/// opens the second front beside the war we are fighting.
#[test]
fn a_crushed_diplomatic_contender_is_kept_and_opened_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    arm_the_front(&mut g);
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(1));
    assert!(g.military_power(0) >= ONE_WAR_CRUSHED_RATIO * g.military_power(2));
    g.players[2].dvp = DIPLOMATIC_CONTENDER_DVP;
    let second = Some(OneWarPeace::SecondFront);
    assert_eq!(ai.one_war_peace(&g, 0, 2), second, "off");
    ai.enable_diplomatic_contender_kept();
    assert_eq!(ai.one_war_peace(&g, 0, 2), None, "kept");
    // At peace, the leader at sixteen opens the second front.
    g.at_war.remove(&(0, 2));
    assert_eq!(ai.one_war_second_front(&g, 0), None, "under the bar");
    g.players[2].dvp = DIPLOMATIC_CONTENDER_LEADER_DVP;
    assert_eq!(ai.one_war_second_front(&g, 0), Some(2), "the leader");
    g.players[3].dvp = DIPLOMATIC_CONTENDER_LEADER_DVP + 1;
    assert_eq!(ai.one_war_second_front(&g, 0), Some(3), "the new leader");
}

/// See `recovery_keeps_the_war`: under the gene, a war we outgun 1.5 times
/// over is kept when the Recovery plan's target is no war of ours; a target
/// we are fighting, or a closer war, still takes the peace.
#[test]
fn recovery_keeps_a_winning_war_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    let mut plan = ai.plan.clone().unwrap();
    plan.strategy = GrandStrategy::Recovery;
    plan.target_player = Some(3);
    assert!(!ai.recovery_keeps_the_war(&g, 0, 1, &plan), "off");
    ai.enable_recovery_keeps_a_winning_war();
    assert!(ai.recovery_keeps_the_war(&g, 0, 1, &plan), "kept");
    plan.target_player = Some(2);
    assert!(!ai.recovery_keeps_the_war(&g, 0, 1, &plan), "another war");
    plan.target_player = Some(3);
    while g.military_power(0) >= ONE_WAR_SECOND_FRONT_RATIO * g.military_power(1) {
        g.spawn_test_unit("modern_armor", 1, (16, 20));
    }
    assert!(!ai.recovery_keeps_the_war(&g, 0, 1, &plan), "not winning");
}

/// See `COUNTER_WAR_PARITY`: under the gene a non-religious counter needs our
/// equal power, and an urgent rival under its floor leaves the front alone.
#[test]
fn a_counter_war_needs_parity_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    while g.military_power(2) * COUNTER_WAR_PARITY <= g.military_power(0) {
        g.spawn_test_unit("modern_armor", 2, (30, 2));
    }
    assert!(g.military_power(0) >= COUNTER_WAR_POWER_FLOOR * g.military_power(2));
    assert!(!ai.counter_war_hopeless(&g, 0, 2), "off: no floor");
    ai.enable_counter_war_needs_parity();
    assert!(ai.counter_war_hopeless(&g, 0, 2), "under parity");
    // An urgent faith under its own floor keeps the army on the front.
    let (mut g, mut ai) = two_fronts();
    convert(&mut g, &[0, 1, 2]);
    assert!(ai.urgent_victory_threat(&g, 2));
    let mut row = 2;
    while !ai.counter_war_hopeless(&g, 0, 2) {
        g.spawn_test_unit("modern_armor", 2, (30, row));
        row += 1;
    }
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(2), "off: the counter");
    let (mut g, mut ai) = two_fronts();
    convert(&mut g, &[0, 1, 2]);
    let mut row = 2;
    while !ai.counter_war_hopeless(&g, 0, 2) {
        g.spawn_test_unit("modern_armor", 2, (30, row));
        row += 1;
    }
    ai.enable_counter_war_needs_parity();
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(1), "the front stays");
}

/// See `diplomatic_contender_front`: under version two, the crushed
/// contender we are already fighting takes the front, ahead of an urgent
/// clock, even beside a richer rival we are not fighting.
#[test]
fn a_crushed_contender_at_war_takes_the_front_under_version_two() {
    let (mut g, mut ai) = two_fronts();
    assert_eq!(ai.one_war_front(), Some(1));
    g.players[2].dvp = DIPLOMATIC_CONTENDER_DVP;
    g.players[3].dvp = DIPLOMATIC_CONTENDER_DVP + 1;
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(1), "off");
    ai.enable_diplomatic_contender_kept_2();
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(2), "the contender at war");
    convert(&mut g, &[0, 1, 2]);
    g.players[2].religion = None;
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(2), "ahead of the urgent clause");
    // Under the bar, the front is chosen as before.
    let (mut g, mut ai) = two_fronts();
    ai.enable_diplomatic_contender_kept_2();
    g.players[2].dvp = DIPLOMATIC_CONTENDER_DVP - 1;
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(1), "under the bar");
}

/// See `diplomatic_contender_to_eliminate`: under the gene a rival at war at
/// 14 points and at our mercy takes the front, below version two's bar of
/// 15, and its nearest city -- not its capital -- is the objective. Live King
/// civvis-20261005T134450Z (game 133) left Byzantium at 14 for Korea's
/// capital, and Byzantium won on Diplomacy.
#[test]
fn a_contender_at_fourteen_holds_the_front_under_the_elimination_gene() {
    let (mut g, mut ai) = two_fronts();
    g.players[2].dvp = ELIMINATION_CONTENDER_DVP;
    ai.enable_diplomatic_contender_kept_2();
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(1), "version two waits for 15");
    ai.enable_diplomatic_contender_eliminated();
    assert_eq!(ai.diplomatic_contender_to_eliminate(&g, 0), Some(2));
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(2), "the contender at fourteen");
    // Its nearest city is the objective, not its capital.
    let capital = g.player_city_ids(2)[0];
    let near = g.found_city_for(2, (12, 16), None);
    assert!(g.cities[&capital].is_capital);
    assert_eq!(ai.elimination_objective_city(&g, 0, 2), Some(near));
    // Under fourteen, or inside twice its power, it is passed by.
    g.players[2].dvp = ELIMINATION_CONTENDER_DVP - 1;
    assert_eq!(ai.diplomatic_contender_to_eliminate(&g, 0), None);
    g.players[2].dvp = ELIMINATION_CONTENDER_DVP;
    let mut row = 2;
    while g.military_power(0) >= ELIMINATION_POWER_RATIO * g.military_power(2) {
        g.spawn_test_unit("modern_armor", 2, (30, row));
        row += 1;
    }
    assert_eq!(ai.diplomatic_contender_to_eliminate(&g, 0), None);
}

/// See `diplomatic_contender_at_peace`: at peace, a rival on 14 points at
/// our mercy and in reach is the campaign's rival under the gene; at war it
/// is the elimination gene's, and under 14 or inside twice its power it is
/// passed by.
#[test]
fn a_contender_at_peace_is_the_campaign_target_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    g.at_war.clear();
    g.players[2].dvp = ELIMINATION_CONTENDER_DVP;
    assert_eq!(ai.diplomatic_contender_at_peace(&g, 0), None, "off");
    ai.enable_contender_at_peace_is_the_target();
    assert_eq!(ai.diplomatic_contender_at_peace(&g, 0), Some(2));
    assert_eq!(ai.assess(&g, 0).target_player, Some(2));
    g.players[2].dvp = ELIMINATION_CONTENDER_DVP - 1;
    assert_eq!(ai.diplomatic_contender_at_peace(&g, 0), None, "under 14");
    g.players[2].dvp = ELIMINATION_CONTENDER_DVP;
    g.at_war.insert((0, 1));
    assert_eq!(
        ai.diplomatic_contender_at_peace(&g, 0),
        None,
        "a war is running"
    );
    g.at_war.clear();
    let mut row = 2;
    while g.military_power(0) >= ELIMINATION_POWER_RATIO * g.military_power(2) {
        g.spawn_test_unit("modern_armor", 2, (30, row));
        row += 1;
    }
    assert_eq!(
        ai.diplomatic_contender_at_peace(&g, 0),
        None,
        "inside twice its power"
    );
}

/// See `declaration_has_the_edge`: under the gene a plain staged war needs
/// 1.5 times the target's steady power; with the gene off it always has it.
#[test]
fn a_staged_war_needs_the_edge_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    assert!(ai.declaration_has_the_edge(&g, 0, 2), "off");
    ai.enable_declaration_needs_the_edge();
    assert!(
        ai.declaration_has_the_edge(&g, 0, 2),
        "four armors against a warrior"
    );
    let mut row = 2;
    while g.military_power(0) >= DECLARATION_EDGE_RATIO * g.military_power(2) {
        g.spawn_test_unit("modern_armor", 2, (30, row));
        row += 1;
    }
    assert!(!ai.declaration_has_the_edge(&g, 0, 2), "short of 1.5 times");
    ai.disable_declaration_needs_the_edge();
    assert!(ai.declaration_has_the_edge(&g, 0, 2), "off again");
}

/// See `capital_prey_beside_the_front`: under the gene, the weakest rival
/// beside the front whose own capital stands open within reach is named; a
/// walled capital or a real army leaves it a near miss, and a war on such a
/// rival is kept (`capital_prey_kept`). Live King civvis-20261005T053701Z
/// (game 103) left the Inca, at 12 military against ~600, at peace.
#[test]
fn a_collapsed_rivals_open_capital_opens_a_front_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    assert_eq!(
        ai.capital_prey_beside_the_front(&g, 0, Some(1)).0,
        None,
        "off"
    );
    assert!(!ai.capital_prey_kept(&g, 0, 2), "off");
    ai.enable_capital_prey_opens_a_front();
    // Player 3, at peace with no army, is weaker than player 2's warrior.
    assert_eq!(ai.capital_prey_beside_the_front(&g, 0, Some(1)).0, Some(3));
    assert!(ai.capital_prey_kept(&g, 0, 2), "the war on a prey is kept");
    assert!(!ai.capital_prey_kept(&g, 0, 3), "no war, nothing to keep");
    let capital = g
        .cities
        .values()
        .find(|city| city.owner == 3 && city.is_capital)
        .map(|city| city.id)
        .unwrap();
    g.cities.get_mut(&capital).unwrap().wall_hp = CAPITAL_PREY_WALLS + 100;
    let (prey, near) = ai.capital_prey_beside_the_front(&g, 0, Some(1));
    assert_eq!(
        prey,
        Some(2),
        "a walled capital at peace waits; the war's prey stays"
    );
    assert!(near.contains(&(3, "walls")));
    g.cities.get_mut(&capital).unwrap().wall_hp = 0;
    for y in [2, 3, 4] {
        g.spawn_test_unit("modern_armor", 3, (32, y));
    }
    let (prey, near) = ai.capital_prey_beside_the_front(&g, 0, Some(1));
    assert_eq!(prey, Some(2));
    assert!(near.contains(&(3, "power")));
}

/// See `declarable_in_reach`: a rival is declarable while a city of theirs is
/// within the declaration range; moved out of it, it is not. Live King
/// civvis-20261005T060002Z (game 104) aimed its campaign at Greece, out of
/// range and at peace, from turn 147 to 182+.
#[test]
fn a_second_front_needs_a_city_within_declaration_range() {
    let (mut g, ai) = two_fronts();
    assert!(ai.declarable_in_reach(&g, 0, 3));
    for city in g.cities.values_mut().filter(|city| city.owner == 3) {
        city.pos = (26, 0);
    }
    assert!(!ai.declarable_in_reach(&g, 0, 3));
}

/// See `front_capital_to_finish`: under the gene, an urgent counter already
/// at war waits for a fresh siege of the front's original capital behind
/// light walls, even in Stage; heavy walls or a stale stage release it.
#[test]
fn the_front_finishes_its_capital_before_an_urgent_counter() {
    use crate::ai::advanced::siege_train::{Siege, SiegeStage};
    let front_after = |gene: bool, walls: i32, entered_ago: u32| {
        let (mut g, mut ai) = two_fronts();
        convert(&mut g, &[0, 1, 2]);
        assert!(ai.urgent_victory_threat(&g, 2));
        if gene {
            ai.enable_front_finishes_its_capital();
        }
        let capital = g.player_city_ids(1)[0];
        assert!(g.cities[&capital].is_capital);
        g.cities.get_mut(&capital).unwrap().wall_hp = walls;
        let siege = Siege {
            stage: SiegeStage::Stage,
            taker: None,
            entered: g.turn - entered_ago,
            assessed: g.turn,
            posts: Default::default(),
            short_since: None,
        };
        ai.sieges.insert(capital, siege);
        ai.one_war_observe(&g, 0);
        ai.one_war_front()
    };
    assert_eq!(front_after(false, 100, 2), Some(2), "off");
    assert_eq!(
        front_after(true, 100, 2),
        Some(1),
        "Canberra's walls, staged"
    );
    assert_eq!(front_after(true, 200, 2), Some(2), "heavier walls");
    assert_eq!(
        front_after(true, 100, FRONT_CAPITAL_FINISH_TURNS + 1),
        Some(2),
        "a stale stage"
    );
    // Any other city of the front holds it once breached past Stage.
    let other_after = |stage: SiegeStage| {
        let (mut g, mut ai) = two_fronts();
        let town = g.found_city_for(1, (14, 18), None);
        convert(&mut g, &[0, 1, 2]);
        assert!(ai.urgent_victory_threat(&g, 2));
        ai.enable_front_finishes_its_capital();
        assert!(!g.cities[&town].is_capital);
        g.cities.get_mut(&town).unwrap().wall_hp = 28;
        let siege = Siege {
            stage,
            taker: None,
            entered: g.turn - 1,
            assessed: g.turn,
            posts: Default::default(),
            short_since: None,
        };
        ai.sieges.insert(town, siege);
        ai.one_war_observe(&g, 0);
        ai.one_war_front()
    };
    assert_eq!(
        other_after(SiegeStage::Invest),
        Some(1),
        "Pharsalos at 28 walls, investing"
    );
    assert_eq!(
        other_after(SiegeStage::Stage),
        Some(2),
        "a town still staging"
    );
}

/// See `religious_threat_spares_the_front`: under the gene, a religious clock
/// does not take the front while our cities keep our own faith.
#[test]
fn a_religious_clock_spares_the_front_while_we_keep_our_faith() {
    let front_after = |gene: bool, ours_converted: bool| {
        let (mut g, mut ai) = two_fronts();
        g.players[0].religion = Some("taoism".to_string());
        if ours_converted {
            convert(&mut g, &[0, 1, 2, 3]);
        } else {
            convert(&mut g, &[1, 2, 3]);
        }
        assert!(ai.urgent_victory_threat(&g, 2));
        if gene {
            ai.enable_religious_threat_spares_the_front();
        }
        ai.one_war_observe(&g, 0);
        ai.one_war_front()
    };
    assert_eq!(front_after(false, false), Some(2), "off");
    assert_eq!(front_after(true, false), Some(1), "we keep taoism");
    assert_eq!(
        front_after(true, true),
        Some(2),
        "their faith holds our majority"
    );
}

/// See `denial_keeps_its_rival`: an incumbent that no longer counts holds
/// nothing, and the incumbent that leads stays.
#[test]
fn the_counter_keeps_its_rival_only_while_it_still_counts() {
    let (mut g, mut ai) = two_fronts();
    convert(&mut g, &[0, 1, 2]);
    let counter = Some((2, GrandStrategy::Conquest));
    assert_eq!(ai.actionable_victory_denial(&g, 0), counter);
    ai.enable_denial_keeps_its_rival();
    ai.denial_incumbent = Some(3);
    assert_eq!(
        ai.actionable_victory_denial(&g, 0),
        counter,
        "a stale incumbent"
    );
    ai.denial_incumbent = Some(2);
    assert_eq!(
        ai.actionable_victory_denial(&g, 0),
        counter,
        "the incumbent leads"
    );
    ai.disable_denial_keeps_its_rival();
    assert_eq!(ai.denial_incumbent, None);
}

/// See `peace_asks_city_from_strength`: under the gene, a white offer from
/// three times the rival's power asks for a town; a routed front, or a rival
/// near our strength, gets the white peace.
#[test]
fn a_peace_from_strength_asks_for_a_town_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    assert!(!ai.peace_asks_city_from_strength(&g, 0, 2), "off");
    ai.enable_peace_asks_a_city();
    assert!(
        ai.peace_asks_city_from_strength(&g, 0, 2),
        "a warrior against armor"
    );
    ai.peace_routed.insert(2);
    assert!(!ai.peace_asks_city_from_strength(&g, 0, 2), "routed");
    ai.peace_routed.clear();
    let mut row = 2;
    while g.military_power(0) >= super::super::PEACE_CITY_ASK_RATIO * g.military_power(2) {
        g.spawn_test_unit("modern_armor", 2, (24, row));
        row += 1;
    }
    assert!(
        !ai.peace_asks_city_from_strength(&g, 0, 2),
        "under three times"
    );
}

/// See `capital_prey_beside_the_front`: version two names a prey at peace,
/// with no war burning; version one only beside a front.
#[test]
fn version_two_names_a_capital_prey_at_peace() {
    let (mut g, mut ai) = two_fronts();
    g.at_war.clear();
    ai.enable_capital_prey_opens_a_front();
    assert_eq!(
        ai.capital_prey_beside_the_front(&g, 0, None).0,
        None,
        "version one"
    );
    ai.enable_capital_prey_opens_a_front_2();
    assert_eq!(
        ai.capital_prey_beside_the_front(&g, 0, None).0,
        Some(1),
        "the weakest met rival, its capital eight tiles out"
    );
}

/// See `capital_prey_reaches_far`: a prey at most a tenth of our military
/// reaches past the declaration range to its own original capital; a
/// stronger rival, or another tile, does not.
#[test]
fn a_deeply_collapsed_prey_reaches_past_the_declaration_range() {
    let (mut g, mut ai) = two_fronts();
    let capital = g
        .cities
        .values()
        .find(|city| city.owner == 3 && city.is_capital)
        .map(|city| city.pos)
        .unwrap();
    assert!(!ai.capital_prey_reaches_far(&g, 0, 3, capital), "off");
    ai.enable_capital_prey_opens_a_front_2();
    assert!(ai.capital_prey_reaches_far(&g, 0, 3, capital));
    assert!(
        !ai.capital_prey_reaches_far(&g, 0, 3, (capital.0, capital.1 + 2)),
        "not its capital"
    );
    let mut row = 2;
    while g.military_power(3) <= CAPITAL_PREY_DEEP_POWER * g.military_power(0) {
        g.spawn_test_unit("modern_armor", 3, (32, row));
        row += 1;
    }
    assert!(
        !ai.capital_prey_reaches_far(&g, 0, 3, capital),
        "more than a tenth"
    );
}

/// See `bleeding_capital`: a captured original capital under the runway is
/// bleeding; a healthy one is not.
#[test]
fn a_captured_capital_near_its_flip_is_bleeding() {
    let (mut g, ai) = two_fronts();
    let capital = g
        .cities
        .values()
        .find(|city| city.owner == 1 && city.is_capital)
        .map(|city| city.id)
        .unwrap();
    g.cities.get_mut(&capital).unwrap().owner = 0;
    g.cities.get_mut(&capital).unwrap().loyalty = 4.0;
    assert_eq!(ai.bleeding_capital(&g, 0), Some(capital));
    g.cities.get_mut(&capital).unwrap().loyalty = 50.0;
    assert_eq!(ai.bleeding_capital(&g, 0), None);
}

/// See `liberation_funds_the_congress`: a captured city-state city is
/// liberated for its Favor while a rival reaches the Diplomatic floor; a
/// major's city, or no Diplomatic threat, keeps the ordinary disposition.
#[test]
fn a_captured_city_state_city_funds_the_congress_under_a_diplomatic_threat() {
    let (mut g, mut ai) = two_fronts();
    // Player 3 stands in for a city-state whose city we took.
    let town = g.player_city_ids(3)[0];
    g.players[3].is_minor = true;
    g.cities.get_mut(&town).unwrap().owner = 0;
    let major_town = g.player_city_ids(1)[0];
    g.cities.get_mut(&major_town).unwrap().owner = 0;
    g.players[2].dvp = LIBERATION_DVP_FLOOR;
    assert!(!ai.liberation_funds_the_congress(&g, 0, town), "off");
    ai.enable_liberation_funds_the_congress();
    assert!(ai.liberation_funds_the_congress(&g, 0, town));
    assert!(
        !ai.liberation_funds_the_congress(&g, 0, major_town),
        "a major's city"
    );
    g.players[2].dvp = LIBERATION_DVP_FLOOR - 1;
    assert!(
        !ai.liberation_funds_the_congress(&g, 0, town),
        "no Diplomatic threat"
    );
    // A town of an eliminated major revives it; its capital never goes back.
    g.players[2].dvp = LIBERATION_DVP_FLOOR;
    let dead_town = g.found_city_for(1, (14, 18), None);
    g.cities.get_mut(&dead_town).unwrap().owner = 0;
    g.cities.get_mut(&dead_town).unwrap().is_capital = false;
    g.players[1].alive = false;
    assert!(
        ai.liberation_funds_the_congress(&g, 0, dead_town),
        "revives the eliminated"
    );
    assert!(
        !ai.liberation_funds_the_congress(&g, 0, major_town),
        "never its capital"
    );
}

/// See `engine_culture_clock`: under the gene, the host's own turns to a
/// Culture Victory replace the projected finish and the tourist-ratio
/// pressure; a reported "none" caps the pressure; no reading keeps both.
#[test]
fn the_engine_culture_clock_replaces_the_tourist_ratio_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    let set = |g: &mut Game, turns: Option<f64>| {
        let stats = std::sync::Arc::make_mut(&mut g.observed_public_empire_stats)
            .entry(2)
            .or_default();
        stats.culture_turns_to_victory = turns;
        stats.foreign_tourists = Some(40);
    };
    set(&mut g, Some(3.0));
    assert_eq!(ai.engine_culture_pressure(&g, 2, 40), 40, "off");
    assert_eq!(
        ai.projected_culture_finish(&g, 2),
        None,
        "off: no curve yet"
    );
    ai.enable_culture_reads_the_engine_clock();
    assert_eq!(ai.projected_culture_finish(&g, 2), Some(3.0));
    assert_eq!(ai.engine_culture_pressure(&g, 2, 40), 98);
    set(&mut g, Some(-1.0));
    assert_eq!(ai.projected_culture_finish(&g, 2), None, "no path");
    assert_eq!(ai.engine_culture_pressure(&g, 2, 70), 50);
    set(&mut g, None);
    assert_eq!(ai.engine_culture_pressure(&g, 2, 70), 70, "no reading");
    // Game 120's Cree at turn 70: 15 turns on 3 visitors is no path.
    set(&mut g, Some(15.0));
    assert_eq!(ai.projected_culture_finish(&g, 2), Some(15.0));
    std::sync::Arc::make_mut(&mut g.observed_public_empire_stats)
        .entry(2)
        .or_default()
        .foreign_tourists = Some(3);
    assert_eq!(ai.projected_culture_finish(&g, 2), None, "under the floor");
    assert_eq!(ai.engine_culture_pressure(&g, 2, 40), 40);
    ai.record_engine_culture_clock(&g, 0);
    assert!(ai.engine_culture_finish.get(&2).is_none_or(Vec::is_empty));
}

/// See `rout_spares_the_counter`: under the gene, a rout window against the
/// urgent rival we hold at parity or better offers no peace; a rival
/// stronger than us still gets it.
#[test]
fn a_rout_spares_the_rival_we_are_countering_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    convert(&mut g, &[0, 1, 2]);
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(2), "the urgent rival is the front");
    let mut row = 2;
    while g.military_power(0) >= ONE_WAR_SECOND_FRONT_RATIO * g.military_power(2) {
        g.spawn_test_unit("modern_armor", 2, (23, row));
        row += 1;
    }
    assert!(
        g.military_power(0) >= g.military_power(2),
        "fixture: still at parity"
    );
    ai.one_war.as_mut().unwrap().window = VecDeque::from([(g.turn, ONE_WAR_ROUT_NET)]);
    let rout = Some(OneWarPeace::Rout);
    assert_eq!(ai.one_war_peace(&g, 0, 2), rout, "off");
    ai.enable_rout_spares_the_counter();
    assert_eq!(ai.one_war_peace(&g, 0, 2), None, "the counter is spared");
    while g.military_power(0) >= g.military_power(2) {
        g.spawn_test_unit("modern_armor", 2, (23, row));
        row += 1;
    }
    assert_eq!(ai.one_war_peace(&g, 0, 2), rout, "a stronger rival");
}

/// See `engine_culture_clock`: the clock is the earliest projected finish of
/// the valid readings in the window, through the -1 readings between them,
/// and "none" once the window holds no valid reading.
#[test]
fn the_engine_culture_clock_takes_the_earliest_recent_finish() {
    let (mut g, mut ai) = two_fronts();
    ai.enable_culture_reads_the_engine_clock();
    let read = |g: &mut Game, ai: &mut AdvancedAi, turn: u32, turns: f64| {
        g.turn = turn;
        let stats = std::sync::Arc::make_mut(&mut g.observed_public_empire_stats)
            .entry(2)
            .or_default();
        stats.culture_turns_to_victory = Some(turns);
        stats.foreign_tourists = Some(40);
        ai.record_engine_culture_clock(g, 0);
    };
    // Game 116's Brazil, turns 160-166.
    read(&mut g, &mut ai, 160, 53.0);
    read(&mut g, &mut ai, 161, 51.0);
    read(&mut g, &mut ai, 162, 13.0);
    read(&mut g, &mut ai, 163, 7.0);
    // Finishes 213, 212, 175, 170: the earliest, 170, is seven turns out.
    assert_eq!(ai.engine_culture_clock(&g, 2), Some(Some(7.0)));
    read(&mut g, &mut ai, 164, -1.0);
    assert_eq!(
        ai.engine_culture_clock(&g, 2),
        Some(Some(6.0)),
        "through -1"
    );
    read(&mut g, &mut ai, 170, -1.0);
    assert_eq!(
        ai.engine_culture_clock(&g, 2),
        Some(None),
        "too long without a reading"
    );
}

/// See `capital_prey_walls`: under the gene a prey with no army may stand
/// behind Medieval walls, never more; one near our power bound keeps the
/// Ancient bar.
#[test]
fn a_collapsed_prey_capital_may_stand_behind_more_wall_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    ai.enable_capital_prey_opens_a_front();
    let capital = g
        .cities
        .values()
        .find(|city| city.owner == 3 && city.is_capital)
        .map(|city| city.id)
        .unwrap();
    g.cities.get_mut(&capital).unwrap().wall_hp = CAPITAL_PREY_WALLS + 100;
    let (_, near) = ai.capital_prey_beside_the_front(&g, 0, Some(1));
    assert!(near.contains(&(3, "walls")), "off");
    ai.enable_capital_prey_scales_the_walls();
    assert_eq!(ai.capital_prey_beside_the_front(&g, 0, Some(1)).0, Some(3));
    g.cities.get_mut(&capital).unwrap().wall_hp = CAPITAL_PREY_MAX_WALLS + 100;
    let (_, near) = ai.capital_prey_beside_the_front(&g, 0, Some(1));
    assert!(near.contains(&(3, "walls")), "past the Medieval bar");
    assert_eq!(ai.capital_prey_walls(0.0, 400.0), CAPITAL_PREY_MAX_WALLS);
    assert_eq!(ai.capital_prey_walls(40.0, 400.0), 150);
    assert_eq!(ai.capital_prey_walls(60.0, 400.0), CAPITAL_PREY_WALLS);
}

/// See `faith_at_match_point`: under the gene, a faith holding half the
/// majors (the founder and our cities) waits for a staged siege; one holding
/// all but one is declared on as before.
#[test]
fn a_faith_counter_waits_for_match_point_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    g.at_war.remove(&(0, 2));
    convert(&mut g, &[0, 2]);
    ai.one_war_observe(&g, 0);
    assert!(ai.faith_counter_due(&g, 0, 2), "off");
    ai.enable_faith_counter_waits_for_match_point();
    assert!(!ai.faith_at_match_point(&g, 2));
    assert!(!ai.faith_counter_due(&g, 0, 2), "two majors of four");
    convert(&mut g, &[0, 1, 2]);
    assert!(ai.faith_at_match_point(&g, 2));
    assert!(ai.faith_counter_due(&g, 0, 2), "three of four");
}

/// See `steady_rival_power`: under the gene, a rival whose army vanished from
/// the reading this turn is read at its strength of the turns before, so its
/// capital is no prey; the gene off reads the turn alone.
#[test]
fn a_one_turn_collapse_opens_no_prey_front_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    ai.enable_capital_prey_opens_a_front();
    let army: Vec<u32> = (2..8)
        .map(|y| g.spawn_test_unit("modern_armor", 3, (32, y)))
        .collect();
    let strong = g.military_power(3);
    ai.enable_prey_reads_a_steady_power();
    ai.record_rival_power(&g, 0);
    for uid in army {
        g.remove_unit(uid);
    }
    g.turn += 1;
    ai.record_rival_power(&g, 0);
    assert!(g.military_power(3) < strong);
    assert_eq!(ai.steady_rival_power(&g, 3), strong);
    // Player 2, at war with a lone warrior, is the prey that remains.
    let (prey, near) = ai.capital_prey_beside_the_front(&g, 0, Some(1));
    assert_ne!(prey, Some(3), "the reading of a turn ago stands");
    assert!(near.contains(&(3, "power")));
    // Past the memory the low reading is the rival's, the weakest prey.
    g.turn += PREY_POWER_MEMORY_TURNS;
    ai.record_rival_power(&g, 0);
    assert_eq!(ai.capital_prey_beside_the_front(&g, 0, Some(1)).0, Some(3));
    ai.disable_prey_reads_a_steady_power();
    assert_eq!(ai.steady_rival_power(&g, 3), g.military_power(3), "off");
}

/// See `second_front_recently_named`: a faith counter named the second front
/// and was declared on; when our converted city turns back the next turn,
/// the gene keeps the war for its memory, and only then offers the one-war
/// peace. The gene off offers it at once.
#[test]
fn a_named_second_front_keeps_its_war_under_the_gene() {
    let run = |gene: bool| -> (Option<OneWarPeace>, Option<OneWarPeace>) {
        let (mut g, mut ai) = two_fronts();
        if gene {
            ai.enable_second_front_keeps_its_war();
        }
        g.at_war.remove(&(0, 2));
        convert(&mut g, &[0, 2]);
        ai.one_war_observe(&g, 0);
        assert_eq!(ai.one_war_second_front(&g, 0), Some(2));
        // The declaration lands; our cities turn back from its faith.
        g.at_war.insert((0, 2));
        convert(&mut g, &[2]);
        g.turn += 1;
        ai.one_war_observe(&g, 0);
        assert_eq!(ai.one_war_front(), Some(1));
        let soon = ai.one_war_peace(&g, 0, 2);
        g.turn += g.standard_duration(SECOND_FRONT_MEMORY_TURNS) + 1;
        ai.one_war_observe(&g, 0);
        (soon, ai.one_war_peace(&g, 0, 2))
    };
    assert_eq!(run(false).0, Some(OneWarPeace::SecondFront), "off");
    let (soon, later) = run(true);
    assert_eq!(soon, None, "the named second front keeps its war");
    assert_eq!(later, Some(OneWarPeace::SecondFront), "past the memory");
}

/// See `recovery_peace_ready`: under the gene the Recovery clause's peace
/// waits for the Recovery plan to stand its patience; a return to Conquest
/// ends the spell.
#[test]
fn recovery_peace_waits_for_a_standing_recovery_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    assert!(ai.recovery_peace_ready(&g), "off");
    ai.enable_recovery_peace_waits();
    assert!(!ai.recovery_peace_ready(&g), "no Recovery yet");
    let mut plan = ai.plan.clone().unwrap();
    plan.strategy = GrandStrategy::Recovery;
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!ai.recovery_peace_ready(&g), "a fresh spell");
    g.turn += RECOVERY_PEACE_PATIENCE;
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(ai.recovery_peace_ready(&g), "a standing spell");
    plan.strategy = GrandStrategy::Conquest;
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!ai.recovery_peace_ready(&g), "the spell ended");
}

/// See `favor_spares_surprise`: with a rival on 18 Diplomatic Victory points
/// the urgent war on it opens by denouncement under the gene, a surprise war
/// with it off; no contender, or a faith at match point, keeps the surprise.
#[test]
fn favor_spares_the_surprise_war_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    g.at_war.remove(&(0, 2));
    g.players[2].dvp = 18;
    assert!(
        ai.urgent_victory_threat(&g, 2),
        "fixture: an urgent Diplomatic clock"
    );
    let surprise = |g: &Game, ai: &AdvancedAi| {
        matches!(
            ai.preferred_war_opening(g, 0, 2),
            Some(crate::game::Action::DeclareWar { .. })
        )
    };
    let denounce = |g: &Game, ai: &AdvancedAi| {
        matches!(
            ai.preferred_war_opening(g, 0, 2),
            Some(crate::game::Action::Denounce { .. })
        )
    };
    assert!(!ai.favor_spares_surprise(&g, 0, 2), "off");
    assert!(surprise(&g, &ai), "off: the surprise war");
    ai.enable_favor_spares_the_surprise_war();
    assert!(ai.favor_spares_surprise(&g, 0, 2));
    assert!(denounce(&g, &ai), "the denouncement first");
    // No rival at the bar: the surprise war stands.
    g.players[2].dvp = FAVOR_SURPRISE_DVP - 1;
    assert!(!ai.favor_spares_surprise(&g, 0, 2));
    // A faith at match point is a clock nearly out.
    g.players[3].dvp = FAVOR_SURPRISE_DVP;
    convert(&mut g, &[0, 1, 2]);
    assert!(ai.faith_at_match_point(&g, 2));
    assert!(!ai.favor_spares_surprise(&g, 0, 2));
}

/// See `second_front_waits_for_its_war`: without a live front siege, a second
/// front still at peace leaves the plan on the running war under the gene,
/// and is declared on from the diplomacy desk all the same. Live King
/// civvis-20261005T162932Z (game 143): the Washington row vanished for 51
/// turns while the plan aimed at Egypt, still at peace.
#[test]
fn a_second_front_at_peace_waits_for_its_war_under_the_gene() {
    // Gene off: the plan hands the running war's target to the second front.
    let (mut g, mut ai) = two_fronts();
    g.at_war.remove(&(0, 2));
    convert(&mut g, &[0, 2]);
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_second_front(&g, 0), Some(2));
    assert!(!ai.urgent_victory_threat(&g, 2), "fixture: not urgent");
    assert!(!ai.second_front_waits_for_the_front(&g, 0, 2));
    assert!(!ai.second_front_waits_for_its_war(&g, 0, 2));
    assert_eq!(ai.assess(&g, 0).target_player, Some(2));

    // Gene on: the plan and its objective stay on the running war, and the
    // second front is still declared on.
    let (mut g, mut ai) = two_fronts();
    g.found_city_for(0, (6, 18), None);
    g.at_war.remove(&(0, 2));
    convert(&mut g, &[0, 2]);
    ai.enable_second_front_waits_for_its_war();
    ai.coalition_before_war = false;
    ai.coalition_before_war_2 = false;
    ai.coalition_before_war_3 = false;
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_second_front(&g, 0), Some(2));
    assert!(ai.second_front_waits_for_its_war(&g, 0, 2));
    let plan = ai.assess(&g, 0);
    assert_eq!(plan.target_player, Some(1), "the army stays on its war");
    assert_eq!(
        plan.target_city.map(|city| g.cities[&city].owner),
        Some(1),
        "the running war keeps its objective, so the Board keeps its Siege row"
    );
    for _ in 0..12 {
        if g.is_at_war(0, 2) {
            break;
        }
        g.turn += 1;
        let plan = ai.assess(&g, 0);
        assert_eq!(plan.target_player, Some(1));
        ai.advanced_diplomacy(&mut g, 0, &plan);
    }
    assert!(g.is_at_war(0, 2), "the second front is declared on");
    assert!(
        !ai.second_front_waits_for_its_war(&g, 0, 2),
        "once at war it no longer waits"
    );

    // An urgent clock keeps its claim on the plan.
    let (mut g, mut ai) = two_fronts();
    g.at_war.remove(&(0, 2));
    convert(&mut g, &[0, 1, 2]);
    ai.enable_second_front_waits_for_its_war();
    ai.one_war_observe(&g, 0);
    assert!(ai.urgent_victory_threat(&g, 2));
    assert!(!ai.second_front_waits_for_its_war(&g, 0, 2));
}

/// Pin player `seat`'s military to `power` through the host reading.
fn set_power(g: &mut Game, seat: usize, power: f64) {
    std::sync::Arc::make_mut(&mut g.observed_military_power).insert(seat, power);
}

/// Pin player 2's Production a turn `margin` above ours (below when negative)
/// through the host's yield correction, as the live mirror folds a rival's
/// public total.
fn set_production_margin(g: &mut Game, margin: f64) {
    std::sync::Arc::make_mut(&mut g.observed_yield_adjustments).clear();
    let ours = crate::ai::BasicAi::seat_production_per_turn(g, 0);
    let theirs = crate::ai::BasicAi::seat_production_per_turn(g, 2);
    std::sync::Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        2,
        crate::rules::Yields {
            production: ours - theirs + margin,
            ..Default::default()
        },
    );
}

/// See `declaration_edge_2`: under version 2 a staged war at 1.6 times a
/// rival that out-produces us is held, at 2.1 times its peak it passes, and
/// at 1.6 times a rival we out-produce it passes on version 1's edge.
/// Version 1 alone passes all three.
#[test]
fn a_staged_war_needs_twice_the_peak_or_the_production_edge_under_version_two() {
    let (mut g, mut ai) = two_fronts();
    let ours = g.military_power(0);
    set_power(&mut g, 2, ours / 1.6);
    set_production_margin(&mut g, 10.0);
    assert!(ai.declaration_has_the_edge(&g, 0, 2), "off");
    ai.enable_declaration_needs_the_edge();
    assert!(ai.declaration_has_the_edge(&g, 0, 2), "version 1 at 1.6 times");
    ai.enable_declaration_needs_the_edge_2();
    assert!(!ai.declaration_needs_the_edge, "version 2 turns version 1 off");
    ai.record_rival_power(&g, 0);
    let reading = ai.declaration_edge_2(&g, 0, 2);
    assert!(reading.their_production > reading.our_production);
    assert!(
        !ai.declaration_has_the_edge(&g, 0, 2),
        "1.6 times a rival that out-produces us"
    );

    set_power(&mut g, 2, ours / 2.1);
    g.turn += DECLARATION_PEAK_TURNS;
    ai.record_rival_power(&g, 0);
    assert!(ai.declaration_has_the_edge(&g, 0, 2), "2.1 times its peak");

    set_power(&mut g, 2, ours / 1.6);
    g.turn += DECLARATION_PEAK_TURNS;
    ai.record_rival_power(&g, 0);
    set_production_margin(&mut g, -10.0);
    assert!(
        ai.declaration_has_the_edge(&g, 0, 2),
        "1.6 times a rival we out-produce"
    );
    ai.disable_declaration_needs_the_edge_2();
    ai.disable_declaration_needs_the_edge();
    assert!(ai.declaration_has_the_edge(&g, 0, 2), "off again");
}

/// See `peak_rival_power`: a rival that stood at its strength within the last
/// 30 turns is weighed at that peak however low it reads now, and the peak
/// leaves the reading once 30 turns have passed. The prey gates' three-turn
/// reading is untouched.
#[test]
fn version_two_weighs_the_thirty_turn_peak() {
    let (mut g, mut ai) = two_fronts();
    let ours = g.military_power(0);
    set_production_margin(&mut g, 10.0);
    ai.enable_declaration_needs_the_edge_2();
    set_power(&mut g, 2, ours / 1.6);
    ai.record_rival_power(&g, 0);
    let peak = g.military_power(2);
    set_power(&mut g, 2, ours / 2.1);
    g.turn += DECLARATION_PEAK_TURNS - 1;
    ai.record_rival_power(&g, 0);
    assert_eq!(ai.peak_rival_power(&g, 2), peak);
    assert_eq!(
        ai.steady_rival_power(&g, 2),
        g.military_power(2),
        "the prey reading is the turn's"
    );
    assert!(
        !ai.declaration_has_the_edge(&g, 0, 2),
        "2.1 times now but 1.6 times the peak of 29 turns ago"
    );
    g.turn += 1;
    ai.record_rival_power(&g, 0);
    assert_eq!(ai.peak_rival_power(&g, 2), g.military_power(2));
    assert!(ai.declaration_has_the_edge(&g, 0, 2), "the peak has aged out");
    ai.disable_declaration_needs_the_edge_2();
    ai.record_rival_power(&g, 0);
    assert!(
        ai.rival_power_peak_seen.is_empty(),
        "the gene off keeps no memory"
    );
}

/// See `faith_counter_has_the_edge`: under the gene the religion counter
/// needs 1.5 times the rival's steady power, so a faith whose army stood
/// at 1.4 times under ours a turn ago is not declared on without a siege,
/// though its army reads twice under ours this turn.
#[test]
fn a_faith_counter_needs_the_edge_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    g.found_city_for(0, (6, 18), None);
    g.at_war.remove(&(0, 2));
    convert(&mut g, &[0, 2]);
    ai.one_war_observe(&g, 0);
    let ours = g.military_power(0);
    ai.enable_prey_reads_a_steady_power();
    set_power(&mut g, 2, ours / 1.4);
    ai.record_rival_power(&g, 0);
    set_power(&mut g, 2, ours / 2.0);
    g.turn += 1;
    ai.record_rival_power(&g, 0);
    assert!(ai.faith_counter_due(&g, 0, 2), "off: twice its power this turn");
    assert!(ai.faith_counter_has_the_edge(&g, 0, 2), "off");
    ai.enable_faith_counter_needs_the_edge();
    assert!(
        !ai.faith_counter_has_the_edge(&g, 0, 2),
        "1.4 times its steady power"
    );
    assert!(!ai.faith_counter_due(&g, 0, 2));
    set_power(&mut g, 2, ours / 1.6);
    g.turn += PREY_POWER_MEMORY_TURNS;
    ai.record_rival_power(&g, 0);
    assert!(ai.faith_counter_has_the_edge(&g, 0, 2), "1.6 times");
    assert!(ai.faith_counter_due(&g, 0, 2));
    ai.disable_faith_counter_needs_the_edge();
    set_power(&mut g, 2, ours / 1.2);
    g.turn += PREY_POWER_MEMORY_TURNS;
    ai.record_rival_power(&g, 0);
    assert!(ai.faith_counter_has_the_edge(&g, 0, 2), "off again");
}

/// See `urgent_denial_has_the_edge`: under the gene an urgent victory
/// clock waives the staged war's ratio and edge only at 1.5 times the
/// rival's steady power. Live King game 175 declared on Nubia at 1.4 times
/// under the urgent waiver and was routed within seven turns.
#[test]
fn an_urgent_denial_needs_the_edge_under_the_gene() {
    let (mut g, mut ai) = two_fronts();
    g.found_city_for(0, (6, 18), None);
    g.at_war.remove(&(0, 2));
    ai.one_war_observe(&g, 0);
    let ours = g.military_power(0);
    ai.enable_prey_reads_a_steady_power();
    set_power(&mut g, 2, ours / 1.4);
    ai.record_rival_power(&g, 0);
    g.turn += 1;
    ai.record_rival_power(&g, 0);
    assert!(ai.urgent_denial_has_the_edge(&g, 0, 2), "off: the urgent clock waives the edge");
    ai.enable_urgent_denial_needs_the_edge();
    assert!(
        !ai.urgent_denial_has_the_edge(&g, 0, 2),
        "1.4 times its steady power is held under the gene"
    );
    set_power(&mut g, 2, ours / 1.6);
    g.turn += PREY_POWER_MEMORY_TURNS;
    ai.record_rival_power(&g, 0);
    assert!(ai.urgent_denial_has_the_edge(&g, 0, 2), "1.6 times declares");
    ai.disable_urgent_denial_needs_the_edge();
    set_power(&mut g, 2, ours / 1.2);
    g.turn += PREY_POWER_MEMORY_TURNS;
    ai.record_rival_power(&g, 0);
    assert!(ai.urgent_denial_has_the_edge(&g, 0, 2), "off again: 1.2 times declares");
}
