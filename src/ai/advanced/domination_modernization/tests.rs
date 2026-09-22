use super::*;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, Vec<u32>) {
    let mut g = Game::new_full(2, 24, 16, 366_400, 250, 0, false);
    g.units.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    g.found_city_for(0, (4, 4), None);
    g.found_city_for(1, (18, 10), None);
    g.current = 0;
    g.turn = 125;
    g.players[0].techs.insert(crate::name!("machinery"));
    g.players[0].gold = 1000.0;
    g.players[0].gold_per_turn = 18.0;
    let units = [(4, 4), (4, 5)]
        .into_iter()
        .map(|pos| g.spawn_test_unit("archer", 0, pos))
        .collect();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(g.player_city_ids(1)[0]),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (
        g,
        AdvancedAi::targeting(VictoryTarget::Domination),
        plan,
        units,
    )
}

#[test]
fn named_peacetime_offensive_upgrades_below_old_cash_floor() {
    let (mut g, ai, plan, units) = fixture();
    let price = g.unit_gold_upgrade_offer(0, units[0]).unwrap().1;
    g.players[0].gold = price + 31.0;
    let mut control = g.clone();
    BasicAi::modernize_army(&mut control, 0, 120.0, 0.0);
    assert_eq!(control.units[&units[0]].kind, "archer");
    ai.fund_domination_upgrades(&mut g, 0, &plan);
    assert_eq!(g.units[&units[0]].kind, "crossbowman");
    assert_eq!(g.units[&units[1]].kind, "archer");
    assert_eq!(g.players[0].gold, 31.0);
}

#[test]
fn deficit_and_spent_actions_keep_upgrade_money_in_reserve() {
    let (mut g, ai, plan, units) = fixture();
    let price = g.unit_gold_upgrade_offer(0, units[0]).unwrap().1;
    g.players[0].gold = price + 40.0;
    g.players[0].gold_per_turn = -11.0;
    ai.fund_domination_upgrades(&mut g, 0, &plan);
    assert!(units.iter().all(|uid| g.units[uid].kind == "archer"));
    g.players[0].gold_per_turn = 0.0;
    for uid in &units {
        g.units.get_mut(uid).unwrap().acted = true;
    }
    ai.fund_domination_upgrades(&mut g, 0, &plan);
    assert_eq!(g.players[0].gold, price + 40.0);
}

#[test]
fn healthier_body_gets_the_limited_upgrade_budget() {
    let (mut g, ai, plan, units) = fixture();
    g.units.get_mut(&units[0]).unwrap().hp = 9;
    let price = g.unit_gold_upgrade_offer(0, units[1]).unwrap().1;
    g.players[0].gold = price + 31.0;
    ai.fund_domination_upgrades(&mut g, 0, &plan);
    assert_eq!(g.units[&units[0]].kind, "archer");
    assert_eq!(g.units[&units[1]].kind, "crossbowman");
}

#[test]
fn other_lanes_and_no_named_major_offensive_do_not_spend() {
    let (g, ai, plan, units) = fixture();
    let mut science = g.clone();
    AdvancedAi::targeting(VictoryTarget::Science).fund_domination_upgrades(&mut science, 0, &plan);
    assert_eq!(science.players[0].gold, g.players[0].gold);
    for target in [None, Some(1)] {
        let mut control = g.clone();
        let mut peaceful = plan.clone();
        peaceful.target_player = target;
        peaceful.strategy = GrandStrategy::Expansion;
        ai.fund_domination_upgrades(&mut control, 0, &peaceful);
        assert!(units.iter().all(|uid| control.units[uid].kind == "archer"));
    }
}

fn prepare_civics(g: &mut Game) {
    g.players[0].civics.extend(
        [
            "code_of_laws",
            "craftsmanship",
            "foreign_trade",
            "early_empire",
            "state_workforce",
            "political_philosophy",
            "military_tradition",
            "games_recreation",
            "defensive_tactics",
            "feudalism",
        ]
        .into_iter()
        .map(Name::new),
    );
    g.players[0].government = Some("monarchy".into());
    g.players[0].civic = None;
}

#[test]
fn existing_cohort_brings_military_training_then_mercenaries_forward() {
    let (mut g, ai, plan, _) = fixture();
    prepare_civics(&mut g);
    assert!(g
        .available_civics(0)
        .contains(&crate::name!("military_training")));
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].civic.as_deref(), Some("military_training"));
    g.players[0]
        .civics
        .insert(crate::name!("military_training"));
    g.players[0].civic = None;
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].civic.as_deref(), Some("mercenaries"));
    g.players[0].civics.insert(crate::name!("mercenaries"));
    assert!(ai.domination_upgrade_civic_goal(&g, 0).is_none());
}

#[test]
fn discount_policy_displaces_ordinary_card_and_funds_actual_cohort() {
    let (mut g, ai, plan, units) = fixture();
    prepare_civics(&mut g);
    g.players[0].civics.extend(
        ["mercenaries", "military_training"]
            .into_iter()
            .map(Name::new),
    );
    g.players[0].policies = [
        "conscription",
        "retainers",
        "feudal_contract",
        "charismatic_leader",
        "urban_planning",
        "inspiration",
    ]
    .into_iter()
    .map(Name::new)
    .collect();
    let undiscounted = g.unit_gold_upgrade_offer(0, units[0]).unwrap().1;
    // Exactly enough for one discounted upgrade, but not a full-price one.
    g.players[0].gold = undiscounted / 2.0 + 31.0;
    ai.fund_domination_upgrades(&mut g, 0, &plan);
    assert!(g.players[0]
        .policies
        .contains(&crate::name!("professional_army")));
    assert!(g.players[0]
        .policies
        .contains(&crate::name!("conscription")));
    let discounted = g
        .unit_upgrade_price_in_formation(
            0,
            crate::name!("archer"),
            crate::name!("crossbowman"),
            g.units[&units[1]].formation,
        )
        .unwrap()
        .0;
    assert!(discounted < undiscounted);
    assert_eq!(g.units[&units[0]].kind, "crossbowman");
    assert_eq!(
        ai.domination_upgrade_policy(&g, 0),
        Some("professional_army")
    );
    g.players[0].gold = discounted + 31.0;
    ai.fund_domination_upgrades(&mut g, 0, &plan);
    assert_eq!(g.units[&units[1]].kind, "crossbowman");
    assert!(ai.domination_upgrade_policy(&g, 0).is_none());
}

#[test]
fn native_quote_and_denial_are_authoritative() {
    let (mut g, ai, plan, units) = fixture();
    for (index, uid) in units.iter().enumerate() {
        std::sync::Arc::make_mut(&mut g.host_unit_facts).insert(
            *uid,
            crate::game::HostUnitFacts {
                upgrade: Some(crate::game::HostUnitUpgrade {
                    to: Some(crate::name!("crossbowman")),
                    cost: Some(125.0),
                    blocked: (index == 0).then(|| "host refusal".into()),
                }),
                ..Default::default()
            },
        );
    }
    g.players[0].gold = 156.0;
    ai.fund_domination_upgrades(&mut g, 0, &plan);
    assert_eq!(g.units[&units[0]].kind, "archer");
    assert_eq!(g.units[&units[1]].kind, "crossbowman");
    assert_eq!(g.players[0].gold, 31.0);
}

#[test]
fn locked_successors_and_scouts_do_not_redirect_the_civic_path() {
    let (mut g, ai, _, units) = fixture();
    prepare_civics(&mut g);
    g.players[0].techs.clear();
    assert!(ai.domination_upgrade_civic_goal(&g, 0).is_none());
    for uid in units {
        g.remove_unit(uid);
    }
    g.players[0].techs.insert(crate::name!("machinery"));
    g.spawn_test_unit("scout", 0, (4, 4));
    g.spawn_test_unit("scout", 0, (4, 5));
    assert!(ai.domination_upgrade_civic_goal(&g, 0).is_none());
}

#[test]
fn appointed_war_package_keeps_its_own_budget() {
    let (mut g, mut ai, plan, units) = fixture();
    ai.war_plan = Some(WarPlan {
        target_player: 1,
        objective_city: plan.target_city.unwrap(),
        breakthrough_tech: crate::name!("machinery"),
        assault_unit: crate::name!("crossbowman"),
        predecessor: Some(crate::name!("archer")),
        breach_unit: None,
        estimated_research_turns: 1,
        estimated_production_turns: 1,
        estimated_upgrade_gold: 500.0,
        estimated_march_turns: 1,
        phase: WarPhase::Research,
        appointed_turn: g.turn,
        tech_turn: None,
        declared_turn: None,
        last_reviewed_turn: g.turn,
        recovery_assessments: 0,
    });
    ai.fund_domination_upgrades(&mut g, 0, &plan);
    assert_eq!(g.players[0].gold, 1000.0);
    assert!(units.iter().all(|uid| g.units[uid].kind == "archer"));
}

fn funding_fixture() -> (Game, AdvancedAi, StrategicPlan, Vec<u32>) {
    let (mut g, mut ai, plan, units) = fixture();
    prepare_civics(&mut g);
    g.players[0].civics.extend(
        ["mercenaries", "military_training"]
            .into_iter()
            .map(Name::new),
    );
    g.players[0].policies = [
        "professional_army",
        "feudal_contract",
        "veterancy",
        "colonization",
        "charismatic_leader",
        "inspiration",
    ]
    .into_iter()
    .map(Name::new)
    .collect();
    g.players[0].gold = 21.0;
    g.players[0].gold_per_turn = 5.0;
    ai.plan = Some(plan.clone());
    (g, ai, plan, units)
}

#[test]
fn funding_relief_slots_upkeep_savings_without_evicting_upgrade_discount() {
    for war in [false, true] {
        let (mut g, ai, _, _) = funding_fixture();
        if war {
            g.at_war.insert((0, 1));
        }
        let before = g.unit_gold_maintenance(0);
        ai.strategic_policies(&mut g, 0, GrandStrategy::Conquest);
        assert!(
            g.players[0]
                .policies
                .contains(&crate::name!("conscription")),
            "war={war}"
        );
        assert!(g.players[0]
            .policies
            .contains(&crate::name!("professional_army")));
        assert!(g.unit_gold_maintenance(0) < before);
    }
}

#[test]
fn funding_relief_does_not_take_slots_when_cash_or_income_covers_upgrades() {
    for income_only in [false, true] {
        let (mut g, ai, _, _) = funding_fixture();
        if income_only {
            g.players[0].gold_per_turn = 1000.0;
        } else {
            g.players[0].gold = 1000.0;
        }
        ai.strategic_policies(&mut g, 0, GrandStrategy::Conquest);
        assert!(!g.players[0]
            .policies
            .contains(&crate::name!("conscription")));
    }
}

#[test]
fn funding_relief_requires_a_major_offensive_and_an_upgrade_cohort() {
    for mode in ["peace", "science", "one_unit", "no_successor"] {
        let (mut g, mut ai, _, units) = funding_fixture();
        let strategy = match mode {
            "peace" => {
                ai.plan = None;
                GrandStrategy::Expansion
            }
            "science" => {
                ai = AdvancedAi::targeting(VictoryTarget::Science);
                GrandStrategy::Science
            }
            "one_unit" => {
                g.units.remove(&units[1]);
                GrandStrategy::Conquest
            }
            _ => {
                g.players[0].techs.remove(&crate::name!("machinery"));
                GrandStrategy::Conquest
            }
        };
        ai.strategic_policies(&mut g, 0, strategy);
        assert!(
            !g.players[0]
                .policies
                .contains(&crate::name!("conscription")),
            "{mode}"
        );
    }
}
