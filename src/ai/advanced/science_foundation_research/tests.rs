use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

fn board() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut g = Game::new(2, 24, 16, 21, 250, 0);
    g.players[0].civ = "Rome".to_string();
    let home = g.units[&g.player_unit_ids(0)[0]].pos;
    let city = g.found_city_for(0, home, None);
    g.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), home);
    g.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .extend([crate::name!("library"), crate::name!("university")]);
    let missing = [
        "sanitation",
        "chemistry",
        "rocketry",
        "rifling",
        "refining",
        "steel",
        "combustion",
    ];
    g.players[0].techs = g
        .rules
        .techs
        .keys()
        .copied()
        .filter(|tech| !missing.contains(&tech.as_str()))
        .collect();
    g.players[0].research = None;
    for unit in g.player_unit_ids(0) {
        g.remove_unit(unit);
    }
    for _ in 0..2 {
        g.spawn_test_unit("cuirassier", 0, home);
    }
    g.world_era = 6;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_old_techs_before_the_beeline();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: 0,
        rush: false,
    };
    (g, ai, plan, city)
}

#[test]
fn ships_off() {
    opt_in_off_in_both_controllers("science-foundation-before-backlog", |ai| {
        ai.science_foundation_before_backlog
    });
}

#[test]
fn chooser_opens_lab_then_pad_before_tank_path_and_resumes_backlog() {
    let (mut g, mut ai, plan, _) = board();
    let old = ai.old_techs_before_the_beeline_goal(&g, 0, false).unwrap();
    assert_eq!(old.tech.as_str(), "combustion");
    ai.advanced_research(&mut g, 0, &plan);
    assert_ne!(g.players[0].research.as_deref(), Some("sanitation"));
    ai.enable_science_foundation_before_backlog();
    for tech in ["sanitation", "chemistry", "rocketry"] {
        g.players[0].research = None;
        ai.advanced_research(&mut g, 0, &plan);
        assert_eq!(g.players[0].research.as_deref(), Some(tech));
        g.players[0].techs.insert(Name::new(tech));
    }
    assert_eq!(ai.science_foundation_research_goal(&g, 0, &plan), None);
    g.players[0].research = None;
    ai.advanced_research(&mut g, 0, &plan);
    assert_ne!(g.players[0].research.as_deref(), Some("satellites"));
    assert!(ai.tech_leads_to(
        &g,
        &Name::new(g.players[0].research.as_deref().unwrap()),
        "combustion"
    ));
}

#[test]
fn minority_warrior_upgrade_keeps_priority() {
    let (mut g, mut ai, plan, _) = board();
    ai.enable_science_foundation_before_backlog();
    g.players[0].techs.remove(&crate::name!("iron_working"));
    let home = g.units[&g.player_unit_ids(0)[0]].pos;
    g.spawn_test_unit("warrior", 0, home);
    assert_eq!(ai.science_foundation_research_goal(&g, 0, &plan), None);
}

#[test]
fn recovery_threat_rush_and_other_lanes_keep_their_priorities() {
    let (g, mut ai, mut plan, city) = board();
    ai.enable_science_foundation_before_backlog();
    assert_eq!(
        ai.science_foundation_research_goal(&g, 0, &plan),
        Some("chemistry")
    );
    plan.strategy = GrandStrategy::Recovery;
    assert_eq!(ai.science_foundation_research_goal(&g, 0, &plan), None);
    plan.strategy = GrandStrategy::Science;
    plan.threatened_city = Some(city);
    assert_eq!(ai.science_foundation_research_goal(&g, 0, &plan), None);
    plan.threatened_city = None;
    plan.rush = true;
    assert_eq!(ai.science_foundation_research_goal(&g, 0, &plan), None);
    plan.rush = false;
    ai.retarget(VictoryTarget::Culture);
    assert_eq!(ai.science_foundation_research_goal(&g, 0, &plan), None);
}

#[test]
fn no_usable_lab_long_path_and_disabled_victory_do_not_open_window() {
    let (mut g, mut ai, plan, city) = board();
    ai.enable_science_foundation_before_backlog();
    g.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .retain(|building| *building != crate::name!("university"));
    assert_eq!(ai.science_foundation_research_goal(&g, 0, &plan), None);
    g.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("university"));
    g.players[0]
        .techs
        .remove(&crate::name!("scientific_theory"));
    assert_eq!(ai.science_foundation_research_goal(&g, 0, &plan), None);
    g.players[0].techs.insert(crate::name!("scientific_theory"));
    g.victory_conditions.science = false;
    assert_eq!(ai.science_foundation_research_goal(&g, 0, &plan), None);
}

#[test]
fn existing_research_is_not_interrupted() {
    let (mut g, mut ai, plan, _) = board();
    ai.enable_science_foundation_before_backlog();
    g.players[0].research = Some("rifling".to_string());
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("rifling"));
}

#[test]
fn appointed_breakthrough_stays_above_foundation() {
    let (mut g, mut ai, plan, _) = board();
    ai.enable_science_foundation_before_backlog();
    ai.war_plan = Some(WarPlan {
        target_player: 1,
        objective_city: 0,
        breakthrough_tech: crate::name!("rifling"),
        assault_unit: crate::name!("rifleman"),
        predecessor: None,
        breach_unit: None,
        estimated_research_turns: 1,
        estimated_production_turns: 1,
        estimated_upgrade_gold: 0.0,
        estimated_march_turns: 1,
        phase: WarPhase::Research,
        appointed_turn: 0,
        tech_turn: None,
        declared_turn: None,
        last_reviewed_turn: 0,
        recovery_assessments: 0,
    });
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("rifling"));
}

#[test]
fn chemistry_follows_the_university_ships_off() {
    opt_in_off_in_both_controllers("chemistry-follows-the-university", |ai| {
        ai.chemistry_follows_the_university
    });
}

/// `board()` with Chemistry two steps out: its parent Sanitation also missing
/// a parent of its own.
fn two_steps_out() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let (mut g, ai, plan, city) = board();
    let sanitation = g.rules.techs[&crate::name!("sanitation")].requires.clone();
    for parent in sanitation {
        g.players[0].techs.remove(&parent);
    }
    (g, ai, plan, city)
}

#[test]
fn the_university_opens_the_whole_path_where_the_last_step_gene_stays_shut() {
    let (g, mut ai, plan, _) = two_steps_out();
    ai.enable_science_foundation_before_backlog();
    assert_eq!(
        ai.science_foundation_research_goal(&g, 0, &plan),
        None,
        "the older gene needs every parent of Chemistry legal now"
    );
    let (g, mut ai, plan, _) = two_steps_out();
    ai.enable_chemistry_follows_the_university();
    assert_eq!(
        ai.science_foundation_research_goal(&g, 0, &plan),
        Some("chemistry")
    );
}

#[test]
fn the_new_gene_stops_at_chemistry_and_keeps_every_gate() {
    let (mut g, mut ai, plan, city) = two_steps_out();
    ai.enable_chemistry_follows_the_university();
    // Without a University there is no Lab to open.
    g.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .retain(|building| *building != crate::name!("university"));
    assert_eq!(ai.science_foundation_research_goal(&g, 0, &plan), None);

    // A threatened city and a rush keep their research.
    let (g, mut ai, mut plan, city) = two_steps_out();
    ai.enable_chemistry_follows_the_university();
    plan.threatened_city = Some(city);
    assert_eq!(ai.science_foundation_research_goal(&g, 0, &plan), None);

    // Chemistry known: the Rocketry step stays with the older gene.
    let (mut g, mut ai, plan, _) = board();
    ai.enable_chemistry_follows_the_university();
    g.players[0].techs.insert(crate::name!("chemistry"));
    assert_eq!(ai.science_foundation_research_goal(&g, 0, &plan), None);
}

#[test]
fn only_warriors_and_archers_hold_chemistry_back() {
    let unknown = |g: &mut Game, tech: &str| {
        g.players[0].techs.remove(&Name::new(tech));
    };
    // A Horseman waiting on Castles does not hold the new gene back, and
    // does hold the older one.
    let (mut g, mut ai, plan, city) = two_steps_out();
    let home = g.cities[&city].pos;
    g.spawn_test_unit("horseman", 0, home);
    unknown(&mut g, "castles");
    ai.enable_chemistry_follows_the_university();
    assert_eq!(
        ai.science_foundation_research_goal(&g, 0, &plan),
        Some("chemistry")
    );

    // An Archer waiting on Machinery does.
    let (mut g, mut ai, plan, city) = two_steps_out();
    let home = g.cities[&city].pos;
    g.spawn_test_unit("archer", 0, home);
    unknown(&mut g, "machinery");
    ai.enable_chemistry_follows_the_university();
    assert_eq!(ai.science_foundation_research_goal(&g, 0, &plan), None);
}

#[test]
fn a_coastal_seat_keeps_its_cartography_first() {
    let (mut g, mut ai, plan, _) = two_steps_out();
    ai.enable_chemistry_follows_the_university();
    let city = g.player_city_ids(0)[0];
    let home = g.cities[&city].pos;
    let shore = g.nbrs(home)[0];
    g.map.tiles.get_mut(&shore).unwrap().terrain = crate::name!("coast");
    assert!(BasicAi::empire_is_coastal(&g, 0), "the fixture is coastal");
    g.players[0].techs.remove(&crate::name!("cartography"));
    assert_eq!(ai.science_foundation_research_goal(&g, 0, &plan), None);
    g.players[0].techs.insert(crate::name!("cartography"));
    assert_eq!(
        ai.science_foundation_research_goal(&g, 0, &plan),
        Some("chemistry")
    );
}
