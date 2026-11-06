use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::game::HostResearchQuote;

fn quote(g: &mut Game, technology: &str, cost: f64, progress: f64) {
    Arc::make_mut(&mut g.host_research_quotes)
        .entry(0)
        .or_default()
        .insert(
            Name::new(technology),
            HostResearchQuote {
                cost: Some(cost),
                progress: Some(progress),
            },
        );
}

/// Known technologies, army counts and four native quotes come from the
/// actual final read at Indonesia115056Z68/0. Map/positions/other facts are
/// synthetic: this is a routing regression, not a reconstructed native game.
fn board() -> (Game, AdvancedAi) {
    let mut g = Game::new_full(2, 20, 14, 115_056, 300, 0, false);
    let home = g.units[&g.player_unit_ids(0)[0]].pos;
    for uid in g.player_unit_ids(0) {
        g.remove_unit(uid);
    }
    for _ in 0..3 {
        g.spawn_test_unit("archer", 0, home);
        g.spawn_test_unit("heavy_chariot", 0, home);
    }
    g.players[0].techs = [
        "pottery",
        "animal_husbandry",
        "mining",
        "sailing",
        "astrology",
        "irrigation",
        "archery",
        "writing",
        "masonry",
        "bronze_working",
        "wheel",
        "celestial_navigation",
        "currency",
        "horseback_riding",
        "iron_working",
    ]
    .into_iter()
    .map(Name::new)
    .collect();
    g.players[0].research = None;
    g.players[0].research_progress = 0.0;
    g.at_war.insert((0, 1));
    quote(&mut g, "engineering", 80.0, 47.0);
    quote(&mut g, "machinery", 150.0, 71.0);
    quote(&mut g, "apprenticeship", 150.0, 0.0);
    quote(&mut g, "stirrups", 195.0, 0.0);
    (g, AdvancedAi::targeting(VictoryTarget::Science))
}

#[test]
fn ships_off_and_only_one_registered_bit_changes() {
    opt_in_off_in_both_controllers("modernization-prices-the-path", |ai| {
        ai.modernization_prices_the_path
    });
}

#[test]
fn native_army_prefers_the_shorter_usable_research_path() {
    let (g, mut ai) = board();
    assert_eq!(
        ai.wartime_modernization_tech(&g, 0),
        Some(crate::name!("stirrups"))
    );
    assert_eq!(
        AdvancedAi::war_remaining_research_cost(&g, 0, crate::name!("machinery")),
        112.0
    );
    assert_eq!(
        AdvancedAi::war_remaining_research_cost(&g, 0, crate::name!("stirrups")),
        345.0
    );
    ai.enable_modernization_prices_the_path();
    assert_eq!(
        ai.wartime_modernization_tech(&g, 0),
        Some(crate::name!("machinery"))
    );
    ai.disable_modernization_prices_the_path();
    assert_eq!(
        ai.wartime_modernization_tech(&g, 0),
        Some(crate::name!("stirrups"))
    );
}

#[test]
fn native_banked_progress_can_make_the_larger_upgrade_win() {
    let (mut g, mut ai) = board();
    ai.enable_modernization_prices_the_path();
    g.players[0].techs.insert(crate::name!("apprenticeship"));
    quote(&mut g, "stirrups", 195.0, 185.0);
    // No stock gate: an unlock may reveal a resource, or trade/improvement
    // may supply it. This ranking prices research, not upgrade feasibility.
    assert_eq!(g.strategic_stockpile(0, crate::name!("iron")), 0.0);
    assert_eq!(
        ai.wartime_modernization_tech(&g, 0),
        Some(crate::name!("stirrups"))
    );
}

#[test]
fn shared_prerequisite_is_paid_once() {
    let (mut g, mut ai) = board();
    ai.enable_modernization_prices_the_path();
    g.players[0].techs = g.rules.techs.keys().copied().collect();
    for tech in ["apprenticeship", "stirrups", "gunpowder"] {
        g.players[0].techs.remove(&Name::new(tech));
    }
    quote(&mut g, "apprenticeship", 100.0, 0.0);
    quote(&mut g, "stirrups", 80.0, 0.0);
    quote(&mut g, "gunpowder", 30.0, 0.0);
    assert_eq!(
        ai.modernization_research_value(&g, 0, crate::name!("gunpowder"), 210.0),
        1.0
    );
}

#[test]
fn invalid_quotes_use_existing_fallback_and_fully_banked_paths_stay_finite() {
    let (mut g, mut ai) = board();
    ai.enable_modernization_prices_the_path();
    quote(&mut g, "machinery", f64::NAN, 0.0);
    let expected = 45.0 / (33.0 + g.tech_cost("machinery"));
    assert_eq!(
        ai.modernization_research_value(&g, 0, crate::name!("machinery"), 45.0),
        expected
    );
    quote(&mut g, "machinery", 150.0, 150.0);
    quote(&mut g, "engineering", 80.0, 80.0);
    assert_eq!(
        ai.modernization_research_value(&g, 0, crate::name!("machinery"), 45.0),
        45.0
    );
}

#[test]
fn war_and_two_unit_gates_and_known_upgrade_release_are_retained() {
    let (mut g, mut ai) = board();
    ai.enable_modernization_prices_the_path();
    g.at_war.clear();
    assert_eq!(ai.wartime_modernization_tech(&g, 0), None);
    g.at_war.insert((0, 1));
    g.players[1].is_minor = true;
    assert_eq!(ai.wartime_modernization_tech(&g, 0), None);
    g.players[1].is_minor = false;
    g.players[0].techs.insert(crate::name!("machinery"));
    g.players[0].techs.insert(crate::name!("stirrups"));
    assert_eq!(ai.wartime_modernization_tech(&g, 0), None);
    g.players[0].techs.remove(&crate::name!("machinery"));
    let archers: Vec<_> = g
        .player_unit_ids(0)
        .into_iter()
        .filter(|uid| g.units[uid].kind == "archer")
        .collect();
    for uid in archers.into_iter().take(2) {
        g.remove_unit(uid);
    }
    assert_eq!(ai.wartime_modernization_tech(&g, 0), None);
}

#[test]
fn complete_chooser_takes_engineering_and_keeps_an_inflight_project() {
    let (mut g, mut ai) = board();
    ai.enable_modernization_prices_the_path();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    };
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("engineering"));
    g.players[0].research = Some("apprenticeship".to_string());
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("apprenticeship"));
}

#[test]
fn active_progress_is_deducted_once_with_and_without_native_quotes() {
    let (mut g, mut ai) = board();
    ai.enable_modernization_prices_the_path();
    g.players[0].research = Some("engineering".to_string());
    g.players[0].research_progress = 47.0;
    assert_eq!(
        ai.modernization_research_value(&g, 0, crate::name!("machinery"), 45.0),
        45.0 / 112.0,
        "native remaining quotes already include the banked47"
    );
    Arc::make_mut(&mut g.host_research_quotes).clear();
    let remaining = g.tech_cost("engineering") + g.tech_cost("machinery") - 47.0;
    assert_eq!(
        ai.modernization_research_value(&g, 0, crate::name!("machinery"), 45.0),
        45.0 / remaining,
        "model-only research deducts the active bank exactly once"
    );
}
