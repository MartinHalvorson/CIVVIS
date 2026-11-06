use super::*;
use crate::ai::advanced::{
    test_support::opt_in_off_in_both_controllers, GrandStrategy, StrategicPlan,
};
use crate::ai::BasicAi;

/// Only the known-tech set and era come from native Indonesia215700Z129/0.
/// The rest is a model fixture, not a reconstructed native board or outcome.
fn board() -> (Game, AdvancedAi, StrategicPlan) {
    let mut g = Game::new_full(2, 20, 14, 76_129, 300, 0, false);
    g.turn = 129;
    g.world_era = 5;
    for uid in g.player_unit_ids(0) {
        g.remove_unit(uid);
    }
    g.players[0].techs = [
        "animal_husbandry",
        "apprenticeship",
        "archery",
        "astrology",
        "astronomy",
        "banking",
        "bronze_working",
        "buttress",
        "cartography",
        "celestial_navigation",
        "construction",
        "currency",
        "economics",
        "education",
        "engineering",
        "gunpowder",
        "horseback_riding",
        "industrialization",
        "iron_working",
        "irrigation",
        "masonry",
        "mass_production",
        "mathematics",
        "metal_casting",
        "military_engineering",
        "military_tactics",
        "mining",
        "pottery",
        "sailing",
        "sanitation",
        "scientific_theory",
        "shipbuilding",
        "square_rigging",
        "steam_power",
        "stirrups",
        "wheel",
        "writing",
    ]
    .into_iter()
    .map(Name::new)
    .collect();
    g.players[0].research = None;
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan)
}

#[test]
fn ships_off() {
    opt_in_off_in_both_controllers("science-chain-crosses-window", |ai| {
        ai.science_chain_crosses_window
    });
}

#[test]
fn chooser_crosses_the_window_only_for_the_enabled_science_chain() {
    let (mut g, mut ai, plan) = board();
    let initial = BasicAi::era_window_techs(&g, 0);
    assert!(!initial.is_empty());
    assert!(initial.iter().all(|t| !ai.tech_leads_to(&g, t, "rocketry")));
    for tag in ["flight", "replaceable_parts"] {
        let tech = Name::new(tag);
        assert!(g.available_techs(0).contains(&tech));
        assert!(!initial.contains(&tech));
        assert!(ai.tech_leads_to(&g, tag, "rocketry"));
    }
    ai.advanced_research(&mut g, 0, &plan);
    let off = g.players[0].research.as_deref().unwrap();
    assert!(initial.contains(&Name::new(off)));
    assert!(!ai.tech_leads_to(&g, off, "rocketry"));

    g.players[0].research = None;
    ai.enable_science_chain_crosses_window();
    ai.advanced_research(&mut g, 0, &plan);
    let on = g.players[0].research.as_deref().unwrap();
    assert!(g.available_techs(0).contains(&Name::new(on)));
    assert!(!initial.contains(&Name::new(on)));
    assert!(ai.tech_leads_to(&g, on, "rocketry"));
}

#[test]
fn admission_keeps_legal_steps_unique_and_excludes_unrelated_branches() {
    let (g, mut ai, _) = board();
    ai.enable_science_chain_crosses_window();
    let initial = BasicAi::era_window_techs(&g, 0);
    let mut candidates = initial.clone();
    ai.admit_science_chain_steps(&g, 0, Some("rocketry"), Some("rocketry"), &mut candidates);
    let added: Vec<_> = candidates
        .iter()
        .filter(|t| !initial.contains(t))
        .copied()
        .collect();
    assert_eq!(added.len(), 2);
    assert!(added.contains(&crate::name!("flight")));
    assert!(added.contains(&crate::name!("replaceable_parts")));
    let once = candidates.clone();
    ai.admit_science_chain_steps(&g, 0, Some("rocketry"), Some("rocketry"), &mut candidates);
    assert_eq!(candidates, once);
    assert!(candidates.iter().all(|t| g.available_techs(0).contains(t)));
}

#[test]
fn another_selected_goal_and_other_target_contracts_keep_the_window() {
    let (mut g, mut ai, _) = board();
    ai.enable_science_chain_crosses_window();
    let initial = BasicAi::era_window_techs(&g, 0);
    for selected in [None, Some("machinery")] {
        let mut candidates = initial.clone();
        ai.admit_science_chain_steps(&g, 0, Some("rocketry"), selected, &mut candidates);
        assert_eq!(candidates, initial);
    }
    for target in [
        None,
        Some(VictoryTarget::Culture),
        Some(VictoryTarget::Domination),
    ] {
        ai.victory_target = target;
        let mut candidates = initial.clone();
        ai.admit_science_chain_steps(&g, 0, Some("rocketry"), Some("rocketry"), &mut candidates);
        assert_eq!(candidates, initial);
    }
    ai.victory_target = Some(VictoryTarget::Science);
    g.victory_conditions.science = false;
    let mut candidates = initial.clone();
    ai.admit_science_chain_steps(&g, 0, Some("rocketry"), Some("rocketry"), &mut candidates);
    assert_eq!(candidates, initial);
}

#[test]
fn wartime_modernization_still_owns_the_research_slot() {
    let (mut g, mut ai, plan) = board();
    ai.enable_science_chain_crosses_window();
    let home = g.units[&g.player_unit_ids(1)[0]].pos;
    for _ in 0..2 {
        g.spawn_test_unit("archer", 0, home);
    }
    g.at_war.insert((0, 1));
    assert_eq!(
        ai.wartime_modernization_tech(&g, 0),
        Some(crate::name!("machinery"))
    );
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("machinery"));
}

#[test]
fn existing_research_is_not_cancelled() {
    let (mut g, mut ai, plan) = board();
    ai.enable_science_chain_crosses_window();
    g.players[0].research = Some("machinery".to_string());
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("machinery"));
}
