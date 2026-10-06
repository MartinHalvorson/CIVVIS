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

/// A rival that out-guns us is not intercepted: the condemnation would open a
/// major war we are losing.
#[test]
fn an_outgunned_seat_does_not_open_a_religious_interception() {
    let (mut g, mut ai, plan, _, missionary) = fixture();
    for _ in 0..4 {
        g.spawn_test_unit("swordsman", 1, (36, 20));
    }
    assert!(g.military_power(1) > g.military_power(0));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "an out-gunned seat keeps the peace");
    assert!(g.units.contains_key(&missionary));
    assert_eq!(ai.religious_interception_war, None);
}

/// Live Emperor G186 (civvis-20261006T025742Z) at turn 73: we had founded a
/// faith of our own, our cities followed the rival's, its faith held every
/// major but one, none of its cities was located, its spreader stood beside
/// our garrison, and a war on a weaker major (Sweden) was already running.
/// Powers are that turn's readings: ours 341, the faith 195, Sweden 142.
fn g186_match_point_beside_a_war() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let (mut g, ai, plan, _, missionary) = fixture();
    g.players[0].religion = Some("Buddhism".into());
    g.apply(0, &Action::DeclareWar { player: 2 }).unwrap();
    Arc::make_mut(&mut g.observed_military_power).extend([(0, 341.0), (1, 195.0), (2, 142.0)]);
    assert!(g.is_at_war(0, 2));
    assert!(!g.is_at_war(0, 1));
    assert!(
        g.player_city_ids(1).is_empty(),
        "no city of the faith is located"
    );
    assert!(
        g.civ_follows_religion(0, "Islam"),
        "our majority follows the rival faith"
    );
    assert!(ai.urgent_victory_threat(&g, 1));
    (g, ai, plan, missionary)
}

#[test]
fn a_match_point_faith_is_intercepted_beside_a_running_war_under_the_gene() {
    let (mut g, mut ai, plan, missionary) = g186_match_point_beside_a_war();
    ai.enable_religious_match_point_defence();
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        g.is_at_war(0, 1),
        "the match point is intercepted though another war is running"
    );
    assert!(
        !g.units.contains_key(&missionary),
        "the opening condemns the spreader it promised"
    );
    assert_eq!(ai.religious_interception_war, Some((1, 83)));
    assert!(g.is_at_war(0, 2), "the running war is untouched");
}

#[test]
fn without_the_gene_a_running_war_still_refuses_the_interception() {
    let (mut g, mut ai, plan, missionary) = g186_match_point_beside_a_war();
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        !g.is_at_war(0, 1),
        "G186: no war on the faith while Sweden's ran"
    );
    assert!(g.units.contains_key(&missionary));
    assert_eq!(ai.religious_interception_war, None);
}

#[test]
fn the_match_point_defence_needs_the_edge_beside_a_running_war() {
    // Short of the faith and every enemy together: 341 against 195 + 160.
    let (mut g, mut ai, plan, missionary) = g186_match_point_beside_a_war();
    ai.enable_religious_match_point_defence();
    Arc::make_mut(&mut g.observed_military_power).insert(2, 160.0);
    assert!(!ai.match_point_defence_has_the_edge(&g, 0, 1, &[2]));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        !g.is_at_war(0, 1),
        "the faith and Sweden together outgun us"
    );
    assert!(g.units.contains_key(&missionary));

    // Short of 1.2 times the faith's power: 233 against 195 (1.19x).
    let (mut g, mut ai, plan, missionary) = g186_match_point_beside_a_war();
    ai.enable_religious_match_point_defence();
    Arc::make_mut(&mut g.observed_military_power).extend([(0, 233.0), (2, 30.0)]);
    assert!(!ai.match_point_defence_has_the_edge(&g, 0, 1, &[2]));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        !g.is_at_war(0, 1),
        "a second front under 1.2 times the faith"
    );
    assert!(g.units.contains_key(&missionary));

    // At the edge on both readings it opens: 234 against 195 (1.20x) and 225.
    let (mut g, mut ai, plan, missionary) = g186_match_point_beside_a_war();
    ai.enable_religious_match_point_defence();
    Arc::make_mut(&mut g.observed_military_power).extend([(0, 234.0), (2, 30.0)]);
    assert!(ai.match_point_defence_has_the_edge(&g, 0, 1, &[2]));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 1));
    assert!(!g.units.contains_key(&missionary));
}

#[test]
fn the_match_point_defence_never_opens_with_a_city_under_threat() {
    let (mut g, mut ai, plan, missionary) = g186_match_point_beside_a_war();
    ai.enable_religious_match_point_defence();
    // A Swedish army beside our only city: the second front waits.
    for _ in 0..3 {
        g.spawn_test_unit("swordsman", 2, (3, 9));
    }
    assert!(ai.threatened_city(&g, 0).is_some());
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1));
    assert!(g.units.contains_key(&missionary));
}

/// The rival's strongest lane can mask its faith: here its Diplomatic
/// Victory points read above the religion lane's 75, so the shipped
/// interception, which asks for a Religion-led rival, passes it by.
#[test]
fn a_masked_match_point_faith_is_read_on_the_religion_lane_under_the_gene() {
    let (mut g, mut ai, _, _, missionary) = fixture();
    // A founder, as in G186: the faithless counter cannot make the opening
    // urgent, so only `match_point_spreaders_at_home` can.
    g.players[0].religion = Some("Buddhism".into());
    g.players[1].dvp = 16;
    let (lane, progress) = ai.rival_pressure(&g, 1);
    assert_ne!(
        lane,
        GrandStrategy::Religion,
        "the faith is masked ({progress})"
    );
    assert!(ai.faith_at_match_point(&g, 1));
    assert!(!ai.match_point_faith(&g, 1), "off without the gene");
    assert!(!ai.religious_interception_opening(&mut g, 0));
    assert!(!g.is_at_war(0, 1));

    ai.enable_religious_match_point_defence();
    assert!(ai.match_point_faith(&g, 1));
    assert!(ai.match_point_spreaders_at_home(&g, 0, 1));
    assert!(ai.religious_interception_opening(&mut g, 0));
    assert!(g.is_at_war(0, 1));
    assert!(!g.units.contains_key(&missionary));
}

#[test]
fn religious_match_point_defence_is_a_native_opt_in_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "religious-match-point-defence",
        |ai| ai.religious_match_point_defence,
    );
}
