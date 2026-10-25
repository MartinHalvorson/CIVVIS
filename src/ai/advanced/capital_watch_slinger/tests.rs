use super::*;
use crate::game::{HostUnitFacts, HostUnitUpgrade, HostUnitUpgradeResource};

fn fixture() -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(2, 24, 16, 59006400, 250, 0, false);
    g.units.clear();
    g.barb_camps.clear();
    g.at_war.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.owner_city = None;
    }
    g.found_city_for(0, (4, 4), None);
    g.found_city_for(1, (18, 10), None);
    g.current = 0;
    g.turn = 20;
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    g.players[0].techs.extend([
        crate::name!("mining"),
        crate::name!("animal_husbandry"),
        crate::name!("archery"),
    ]);
    g.players[0].gold = 38.0;
    g.players[0].gold_per_turn = 6.0;
    Arc::make_mut(&mut g.observed_military_power).extend([(0, 100.0), (1, 250.0)]);
    g.host_observed = Arc::new(g.wdisk((4, 4), 9).into_iter().collect());
    g.spawn_test_unit("warrior", 1, (7, 4));
    let uid = slinger(&mut g, (4, 4));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_capital_watch_arms_the_slinger();
    (g, ai, uid)
}

fn slinger(g: &mut Game, at: Pos) -> u32 {
    let uid = g.spawn_test_unit("slinger", 0, at);
    Arc::make_mut(&mut g.host_unit_facts).insert(
        uid,
        HostUnitFacts {
            maintenance: Some(0.0),
            upgrade: Some(HostUnitUpgrade {
                to: Some(crate::name!("archer")),
                cost: Some(30.0),
                resources: Some(HostUnitUpgradeResource {
                    resource: None,
                    cost: 0.0,
                }),
                blocked: None,
            }),
            ..Default::default()
        },
    );
    uid
}

fn expansion(g: &Game) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    }
}

#[test]
fn a_healthy_slinger_upgrades_in_the_prewar_expansion_window() {
    let (mut g, mut ai, uid) = fixture();
    assert!(
        g.unit_gold_upgrade_offer(0, uid).is_some(),
        "legal native-priced offer"
    );
    let mut old = g.clone();
    ai.fund_domination_upgrades(&mut old, 0, &expansion(&g));
    assert_eq!(old.units[&uid].kind, "slinger", "no named offensive");
    BasicAi::modernize_army(&mut old, 0, 120.0, 0.0);
    assert_eq!(old.units[&uid].kind, "slinger", "old peaceful reserve");
    ai.observe_turn_start_hostiles(&g, 0);
    assert!(ai.capital_watch_slinger_upgrade(&mut g, 0));
    assert_eq!(g.units[&uid].kind, "archer");
    assert_eq!(g.players[0].gold, 8.0);
    assert_eq!(g.units[&uid].hp, 100);
    assert!(!ai.capital_walls_before_the_surprise);
    assert!(
        ai.capital_walls_research_threat(&g, 0).is_none(),
        "no implicit wall enable"
    );
}

#[test]
fn a_bounded_sighting_survives_the_army_leaving_current_sight() {
    let (mut g, mut ai, uid) = fixture();
    ai.observe_turn_start_hostiles(&g, 0);
    let enemy = g.player_unit_ids(1)[0];
    g.remove_unit(enemy);
    g.turn += 1;
    assert!(ai.capital_watch_slinger_upgrade(&mut g, 0));
    assert_eq!(g.units[&uid].kind, "archer");
}

#[test]
fn discounted_upkeep_keeps_a_cheap_healthy_upgrade_solvent() {
    let (mut g, mut ai, uid) = fixture();
    g.players[0].policies.insert(crate::name!("conscription"));
    g.players[0].gold = 30.5;
    g.players[0].gold_per_turn = 0.5;
    assert_eq!(
        AdvancedAi::upgrade_upkeep_delta(&g, uid, crate::name!("archer")),
        0.0
    );
    ai.observe_turn_start_hostiles(&g, 0);
    assert!(ai.capital_watch_slinger_upgrade(&mut g, 0));
    assert_eq!(g.players[0].gold, 0.5);
}

#[test]
fn only_one_proposal_is_made_per_turn_even_with_two_affordable_slingers() {
    let (mut g, mut ai, first) = fixture();
    let second = slinger(&mut g, (4, 5));
    g.players[0].gold = 100.0;
    ai.observe_turn_start_hostiles(&g, 0);
    assert!(ai.capital_watch_slinger_upgrade(&mut g, 0));
    assert!(!ai.capital_watch_slinger_upgrade(&mut g, 0));
    assert_eq!(
        [first, second]
            .iter()
            .filter(|u| g.units[u].kind == "archer")
            .count(),
        1
    );
    g.turn += 1;
    for uid in [first, second] {
        let moves = g.unit_max_moves(uid);
        let unit = g.units.get_mut(&uid).unwrap();
        unit.moves_left = moves;
        unit.acted = false;
        unit.moved = false;
    }
    assert!(ai.capital_watch_slinger_upgrade(&mut g, 0));
    assert!([first, second].iter().all(|u| g.units[u].kind == "archer"));
}

#[test]
fn stock_and_the_existing_wall_flag_do_not_enable_the_new_purchase() {
    let (g, _, uid) = fixture();
    for walls in [false, true] {
        let mut copy = g.clone();
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        if walls {
            ai.enable_capital_walls_before_the_surprise();
        }
        ai.observe_turn_start_hostiles(&copy, 0);
        assert!(!ai.capital_watch_slinger_upgrade(&mut copy, 0));
        assert_eq!(copy.units[&uid].kind, "slinger");
        assert_eq!(copy.players[0].gold, 38.0);
    }
}

#[test]
fn other_lanes_war_and_absent_or_expired_warning_keep_the_treasury() {
    for case in 0..8 {
        let (mut g, mut ai, uid) = fixture();
        match case {
            0 => {
                ai = AdvancedAi::targeting(VictoryTarget::Science);
                ai.enable_capital_watch_arms_the_slinger();
            }
            1 => {
                g.at_war.insert((0, 1));
            }
            2 => {
                Arc::make_mut(&mut g.observed_military_power).insert(1, 150.0);
            }
            3 => {
                g.host_observed = Arc::new(BTreeSet::new());
                g.remove_unit(g.player_unit_ids(1)[0]);
            }
            4 => {
                ai.forced_target_player = Some(1);
            }
            6 => {
                ai.plan = Some(StrategicPlan {
                    strategy: GrandStrategy::Recovery,
                    ..expansion(&g)
                });
            }
            7 => {
                ai.war_plan = Some(WarPlan {
                    target_player: 1,
                    objective_city: g.player_city_ids(1)[0],
                    breakthrough_tech: crate::name!("archery"),
                    assault_unit: crate::name!("archer"),
                    predecessor: Some(crate::name!("slinger")),
                    breach_unit: None,
                    estimated_research_turns: 1,
                    estimated_production_turns: 1,
                    estimated_upgrade_gold: 30.0,
                    estimated_march_turns: 1,
                    phase: WarPhase::Research,
                    appointed_turn: g.turn,
                    tech_turn: None,
                    declared_turn: None,
                    last_reviewed_turn: g.turn,
                    recovery_assessments: 0,
                });
            }
            _ => {}
        }
        ai.observe_turn_start_hostiles(&g, 0);
        if case == 5 {
            g.turn += g.standard_duration(15) + 1;
        }
        assert!(!ai.capital_watch_slinger_upgrade(&mut g, 0), "case {case}");
        assert_eq!(g.units[&uid].kind, "slinger");
        assert_eq!(g.players[0].gold, 38.0);
    }
}

#[test]
fn wounded_bodies_refused_commands_and_insolvent_bills_do_not_spend() {
    for case in 0..9 {
        let (mut g, mut ai, uid) = fixture();
        match case {
            0 => {
                g.units.get_mut(&uid).unwrap().hp = 74;
            }
            1 => {
                g.players[0].gold = 29.0;
            }
            2 => {
                g.players[0].gold_per_turn = 0.5;
            }
            3 => {
                g.players[0].gold_per_turn = 0.0;
            }
            4 => {
                g.players[0].gold = 31.0;
            }
            5 => {
                Arc::make_mut(&mut g.host_unit_facts)
                    .get_mut(&uid)
                    .unwrap()
                    .upgrade
                    .as_mut()
                    .unwrap()
                    .blocked = Some("Must be in friendly territory.".into());
            }
            6 => {
                g.units.get_mut(&uid).unwrap().acted = true;
            }
            7 => {
                Arc::make_mut(&mut g.host_unit_facts).remove(&uid);
                g.players[0].techs.remove(&crate::name!("archery"));
            }
            _ => {
                g.players[0].techs.insert(crate::name!("machinery"));
                Arc::make_mut(&mut g.host_unit_facts)
                    .get_mut(&uid)
                    .unwrap()
                    .upgrade
                    .as_mut()
                    .unwrap()
                    .to = Some(crate::name!("crossbowman"));
            }
        }
        let gold = g.players[0].gold;
        ai.observe_turn_start_hostiles(&g, 0);
        assert!(!ai.capital_watch_slinger_upgrade(&mut g, 0), "case {case}");
        assert_eq!(g.units[&uid].kind, "slinger");
        assert_eq!(g.players[0].gold, gold);
    }
}

#[test]
fn a_bound_civilian_guard_is_preserved() {
    let (mut g, mut ai, uid) = fixture();
    let civilian = g.spawn_test_unit("settler", 0, (4, 4));
    ai.bind_settler_guard(&g, civilian, uid);
    ai.observe_turn_start_hostiles(&g, 0);
    assert!(ai.guard_is_reserved_for_civilian(uid));
    assert!(!ai.capital_watch_slinger_upgrade(&mut g, 0));
    assert_eq!(g.units[&uid].kind, "slinger");
}

#[test]
fn real_observed_turn_dispatch_reaches_the_new_upgrade_lane() {
    let (mut g, mut ai, uid) = fixture();
    ai.plan = Some(expansion(&g));
    ai.plan_observed_turn(&mut g, 0);
    assert!(g
        .log
        .iter()
        .any(|(_, action)| matches!(action, Action::UpgradeUnit { unit } if *unit == uid)));
    assert_eq!(g.units[&uid].kind, "archer");
}
