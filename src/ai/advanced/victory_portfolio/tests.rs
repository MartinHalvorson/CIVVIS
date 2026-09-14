use super::*;
use crate::game::GameOptions;
use std::sync::Arc;

fn world() -> Game {
    let mut g = Game::new_with(GameOptions {
        speed: "online".into(),
        ..GameOptions::new(3, 40, 28, 914_356_900, 250, 0)
    });
    for pid in 0..3 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .unwrap();
        g.found_city_for(pid, g.units[&settler].pos, None);
        g.remove_unit(settler);
        for other in 0..3 {
            g.record_contact(pid, other);
        }
    }
    g.current = 0;
    g
}

fn plan(strategy: GrandStrategy) -> StrategicPlan {
    StrategicPlan {
        strategy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    }
}

fn driving(primary: VictoryTarget, secondary: Option<VictoryTarget>) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_victory_portfolio();
    ai.portfolio.reviewed = Some(100);
    ai.portfolio.report.primary = Some(primary);
    ai.portfolio.report.secondary = secondary;
    ai.portfolio.report.phase = DevelopmentPhase::Buildup;
    ai.portfolio.report.focus = 0.6;
    ai
}

fn expedition(g: &mut Game, pid: usize, distance: f64) {
    g.players[pid].science_projects.extend(
        [
            "launch_earth_satellite",
            "launch_moon_landing",
            "launch_mars_colony",
            "exoplanet_expedition",
        ]
        .into_iter()
        .map(str::to_string),
    );
    g.players[pid].exoplanet_distance = distance;
}

fn forecast(g: &Game, target: VictoryTarget) -> VictoryEstimate {
    AdvancedAi::victory_forecasts(g, 0)
        .into_iter()
        .find(|forecast| forecast.target == target)
        .unwrap()
}

#[test]
fn expedition_uses_the_observed_finish_line_and_speed() {
    let mut g = world();
    g.turn = 180;
    expedition(&mut g, 0, 20.0);
    let baseline = forecast(&g, VictoryTarget::Science);
    assert_eq!(baseline.finish_turn, Some(210.0));
    let observed = Arc::make_mut(&mut g.observed_public_empire_stats)
        .entry(0)
        .or_default();
    observed.science_victory_points = Some(60.0);
    observed.science_victory_points_needed = Some(100.0);
    observed.science_victory_points_per_turn = Some(4.0);
    assert_eq!(
        forecast(&g, VictoryTarget::Science).finish_turn,
        Some(190.0)
    );
}

#[test]
fn science_accounts_for_project_work_and_banked_production() {
    let mut g = world();
    g.turn = 140;
    g.players[0].techs = g.rules.techs.keys().cloned().collect();
    let cid = g.player_city_ids(0)[0];
    let pos = g.cities[&cid].pos;
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), pos);
    Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        0,
        Yields {
            science: 500.0,
            ..Yields::default()
        },
    );
    let before = forecast(&g, VictoryTarget::Science).finish_turn.unwrap();
    let item = Item::Project {
        project: crate::name!("launch_earth_satellite"),
    };
    let cost = g.item_cost_for_city(0, cid, &item);
    let city = g.cities.get_mut(&cid).unwrap();
    city.queue = vec![item];
    city.production = cost * 0.9;
    let after = forecast(&g, VictoryTarget::Science);
    assert!(after.finish_turn.unwrap() < before);
    assert!(after.committed);
    assert!(
        after.finish_turn.unwrap() > 140.0 + 50.0,
        "a pad is not a completed project chain"
    );
}

#[test]
fn science_research_prerequisites_are_counted_once_and_current_progress_is_used() {
    let mut g = world();
    let goal = crate::name!("rocketry");
    let work = AdvancedAi::tech_work(&g, 0, goal);
    g.players[0].research = Some("rocketry".into());
    g.players[0].research_progress = 50.0;
    assert!((AdvancedAi::tech_work(&g, 0, goal) - (work - 50.0)).abs() < 1e-6);
    g.players[0].techs = g.rules.techs.keys().cloned().collect();
    assert_eq!(AdvancedAi::tech_work(&g, 0, goal), 0.0);
}

#[test]
fn culture_must_overtake_a_growing_defender() {
    let mut g = world();
    g.turn = 150;
    Arc::make_mut(&mut g.observed_public_empire_stats)
        .entry(0)
        .or_default()
        .foreign_tourists = Some(50);
    Arc::make_mut(&mut g.observed_public_empire_stats)
        .entry(1)
        .or_default()
        .domestic_tourists = Some(100);
    Arc::make_mut(&mut g.observed_public_empire_stats)
        .entry(2)
        .or_default()
        .domestic_tourists = Some(90);
    Arc::make_mut(&mut g.observed_tourism_per_turn).insert(0, 600.0);
    let slow = forecast(&g, VictoryTarget::Culture);
    assert!(slow.finish_turn.is_some());
    Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        2,
        Yields {
            culture: 10_000.0,
            ..Yields::default()
        },
    );
    let fast = forecast(&g, VictoryTarget::Culture);
    assert_eq!(
        fast.finish_turn, None,
        "a currently smaller defender can still outgrow tourism"
    );
    assert_eq!(fast.bottleneck, "tourism_growth");
}

#[test]
fn unavailable_religion_and_unknown_capitals_are_not_fictitious_finishes() {
    let g = world();
    assert_eq!(forecast(&g, VictoryTarget::Religion).finish_turn, None);
    let mut hidden = g.clone();
    let cid = hidden.player_city_ids(2)[0];
    hidden.cities.remove(&cid);
    let estimate = forecast(&hidden, VictoryTarget::Domination);
    assert_eq!(estimate.finish_turn, None);
    assert_eq!(estimate.bottleneck, "capital_access");
}

#[test]
fn favor_without_earned_points_does_not_predict_a_diplomatic_win() {
    let mut g = world();
    g.turn = 140;
    g.players[0].diplomatic_favor = 20_000.0;
    assert_eq!(forecast(&g, VictoryTarget::Diplomacy).finish_turn, None);
    let before = Pace {
        turn: 120,
        yields: Yields::default(),
        visitors: 0,
        dvp: 10,
    };
    g.players[0].dvp = 18;
    let estimate = AdvancedAi::forecast_diplomacy(&g, 0, Some(&before));
    assert!(estimate.finish_turn.unwrap() > f64::from(g.turn));
    assert!(estimate.committed);
}

#[test]
fn score_requires_a_real_clock() {
    let mut g = world();
    g.max_turns = 0;
    assert_eq!(forecast(&g, VictoryTarget::Score).finish_turn, None);
}

#[test]
fn defense_changes_posture_without_erasing_primary_or_backup() {
    let g = world();
    let mut ai = driving(VictoryTarget::Science, Some(VictoryTarget::Culture));
    ai.record_portfolio_trace(&g, 0, &plan(GrandStrategy::Recovery));
    assert_eq!(
        ai.decision_objective(GrandStrategy::Recovery),
        GrandStrategy::Recovery
    );
    assert_eq!(
        ai.decision_objective(GrandStrategy::Science),
        GrandStrategy::Science
    );
    assert_eq!(ai.portfolio.report.primary, Some(VictoryTarget::Science));
    assert_eq!(ai.portfolio.report.secondary, Some(VictoryTarget::Culture));
    assert_eq!(ai.portfolio.report.trace[0].posture, "recovery");
}

#[test]
fn foundation_and_frozen_controller_keep_their_economic_objective() {
    let g = world();
    let mut ai = driving(VictoryTarget::Science, None);
    ai.portfolio.report.phase = DevelopmentPhase::Foundation;
    assert_eq!(
        ai.decision_objective(GrandStrategy::Expansion),
        GrandStrategy::Expansion
    );
    assert!(!ai.phase_specialization_active(&g));
    let mut frozen = AdvancedAi::legacy();
    frozen.enable_victory_portfolio();
    frozen.maintain_victory_portfolio(&g, 0);
    assert!(frozen.portfolio.report.forecasts.is_empty());
}

#[test]
fn assignment_is_preserved_even_when_a_different_race_is_nearly_finished() {
    let mut g = world();
    g.turn = 170;
    expedition(&mut g, 0, 49.0);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_victory_portfolio();
    ai.maintain_victory_portfolio(&g, 0);
    assert_eq!(ai.portfolio.report.primary, Some(VictoryTarget::Culture));
    assert_eq!(ai.victory_target(), Some(VictoryTarget::Culture));
}

#[test]
fn observed_near_finish_can_select_science_and_survives_small_changes() {
    let mut g = world();
    g.turn = 170;
    expedition(&mut g, 0, 40.0);
    let mut ai = AdvancedAi::new();
    ai.enable_victory_portfolio();
    ai.maintain_victory_portfolio(&g, 0);
    assert_eq!(ai.portfolio.report.primary, Some(VictoryTarget::Science));
    assert_eq!(ai.portfolio.report.phase, DevelopmentPhase::Finish);
    let committed = ai.portfolio.report.committed_turn;
    g.turn += 1;
    g.players[0].dvp += 1;
    ai.maintain_victory_portfolio(&g, 0);
    assert_eq!(ai.portfolio.report.primary, Some(VictoryTarget::Science));
    assert_eq!(ai.portfolio.report.committed_turn, committed);
    assert_eq!(ai.portfolio.report.primary_switches, 0);
}

#[test]
fn disabled_victories_are_removed_on_the_next_review() {
    let mut g = world();
    g.turn = 170;
    expedition(&mut g, 0, 40.0);
    let mut ai = AdvancedAi::new();
    ai.enable_victory_portfolio();
    ai.maintain_victory_portfolio(&g, 0);
    g.victory_conditions.science = false;
    // Configuration changes invalidate even a same-turn review.
    ai.maintain_victory_portfolio(&g, 0);
    assert_ne!(ai.portfolio.report.primary, Some(VictoryTarget::Science));
    assert!(ai
        .portfolio
        .report
        .forecasts
        .iter()
        .all(|forecast| forecast.target != VictoryTarget::Science));
}

#[test]
fn secondary_is_optional_and_compatibility_is_not_just_the_runner_up() {
    assert!(!AdvancedAi::compatible_backup(
        VictoryTarget::Science,
        VictoryTarget::Domination
    ));
    assert!(AdvancedAi::compatible_backup(
        VictoryTarget::Culture,
        VictoryTarget::Diplomacy
    ));
    let g = world();
    let mut ai = AdvancedAi::new();
    ai.enable_victory_portfolio();
    ai.maintain_victory_portfolio(&g, 0);
    assert_eq!(ai.portfolio.report.secondary, None);
}

#[test]
fn switching_requires_evidence_but_can_escape_a_losing_clock() {
    let mut incumbent = VictoryEstimate::new(VictoryTarget::Science, 0.7, "research");
    incumbent.finish(100, 100.0, 0.6);
    incumbent.committed = true;
    let mut challenger = VictoryEstimate::new(VictoryTarget::Culture, 0.7, "tourism");
    challenger.finish(100, 95.0, 0.6);
    assert!(incumbent.keeps_incumbent(&challenger, 100, 250.0, 100, 10));
    challenger.finish(100, 20.0, 0.6);
    assert!(!incumbent.keeps_incumbent(&challenger, 100, 250.0, 100, 10));
    assert!(incumbent.keeps_incumbent(&challenger, 100, 250.0, 2, 10));
    assert!(
        !incumbent.keeps_incumbent(&challenger, 100, 130.0, 2, 10),
        "hold time must not force a known loss when the alternative fits"
    );
}

#[test]
fn unmet_rivals_prevent_a_false_culture_finish() {
    let mut g = world();
    g.players[0].met.remove(&2);
    Arc::make_mut(&mut g.observed_tourism_per_turn).insert(0, 100_000.0);
    let culture = forecast(&g, VictoryTarget::Culture);
    assert_eq!(culture.finish_turn, None);
    assert_eq!(culture.bottleneck, "unmet_rivals");
}

#[test]
fn a_foundation_preference_is_not_reported_as_a_commitment() {
    let g = world();
    let mut ai = AdvancedAi::new();
    ai.enable_victory_portfolio();
    ai.maintain_victory_portfolio(&g, 0);
    assert!(ai.portfolio.report.primary.is_some());
    assert_eq!(ai.portfolio.report.phase, DevelopmentPhase::Foundation);
    assert_eq!(ai.portfolio.report.first_commitment_turn, None);
}

#[test]
fn extending_the_cap_does_not_delay_a_supported_buildup() {
    let mut g = world();
    g.turn = 100;
    g.world_era = 4;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_victory_portfolio();
    ai.maintain_victory_portfolio(&g, 0);
    let phase = ai.portfolio.report.phase;
    assert_ne!(phase, DevelopmentPhase::Foundation);
    g.max_turns = 650;
    let mut long = AdvancedAi::targeting(VictoryTarget::Science);
    long.enable_victory_portfolio();
    long.maintain_victory_portfolio(&g, 0);
    assert_eq!(long.portfolio.report.phase, phase);
}

#[test]
fn secondary_faith_budget_is_shared_across_replans() {
    let mut ai = driving(VictoryTarget::Science, Some(VictoryTarget::Culture));
    ai.portfolio.faith_opening = Some((100, 1000.0));
    assert_eq!(ai.portfolio_culture_budget(1000.0), 200.0);
    assert_eq!(ai.portfolio_culture_budget(850.0), 50.0);
    assert_eq!(ai.portfolio_culture_budget(800.0), 0.0);
    ai.portfolio.report.primary = Some(VictoryTarget::Culture);
    assert_eq!(ai.portfolio_culture_budget(800.0), 800.0);
}

#[test]
fn spending_supports_the_primary_without_buying_new_backup_districts() {
    use super::super::victory_conversion::ProductionQuote;
    let g = world();
    let ai = driving(VictoryTarget::Science, Some(VictoryTarget::Culture));
    let library = Item::Building {
        building: crate::name!("library"),
    };
    let theater = Item::District {
        district: crate::name!("theater_square"),
        pos: g.cities[&g.player_city_ids(0)[0]].pos,
    };
    let quote = || ProductionQuote {
        turns: 5.0,
        raw: 100.0,
    };
    assert!(
        ai.portfolio_production_adjustment(&g, 0, &library, &plan(GrandStrategy::Science), quote())
            > 0.0
    );
    assert_eq!(
        ai.portfolio_production_adjustment(&g, 0, &theater, &plan(GrandStrategy::Science), quote()),
        0.0
    );
    assert_eq!(
        ai.portfolio_production_adjustment(
            &g,
            0,
            &library,
            &plan(GrandStrategy::Recovery),
            quote()
        ),
        0.0
    );
    assert_eq!(
        ai.portfolio_production_adjustment(
            &g,
            0,
            &library,
            &plan(GrandStrategy::Science),
            ProductionQuote {
                raw: -1000.0,
                turns: 5.0
            }
        ),
        0.0
    );
}

#[test]
fn live_primary_reaches_research_and_late_culture_spending() {
    let mut g = world();
    g.turn = 150;
    let mut ai = driving(VictoryTarget::Science, Some(VictoryTarget::Culture));
    ai.plan = Some(plan(GrandStrategy::Science));
    assert!(ai.portfolio_tech_bonus(&g, 0, "rocketry") > 0.0);
    assert!(ai.culture_lane_spends(&g, 0, &plan(GrandStrategy::Science)));
    assert!(!ai.culture_lane_spends(&g, 0, &plan(GrandStrategy::Recovery)));
    ai.disable_victory_portfolio();
    assert_eq!(ai.portfolio_tech_bonus(&g, 0, "rocketry"), 0.0);
}

#[test]
fn rollout_value_sees_a_finish_instead_of_only_score() {
    let mut g = world();
    g.turn = 180;
    let economy = 0.33;
    assert_eq!(AdvancedAi::victory_finish_value(&g, 0, economy), economy);
    expedition(&mut g, 0, 49.0);
    assert!(AdvancedAi::victory_finish_value(&g, 0, economy) > 0.6);
    g.players[0].science_projects.clear();
    expedition(&mut g, 1, 49.0);
    assert!(AdvancedAi::victory_finish_value(&g, 0, economy) < economy);
}

#[test]
fn same_turn_observations_do_not_duplicate_samples_or_trace() {
    let mut g = world();
    g.turn = 100;
    let mut ai = AdvancedAi::new();
    ai.enable_victory_portfolio();
    for _ in 0..3 {
        ai.maintain_victory_portfolio(&g, 0);
        ai.record_portfolio_trace(&g, 0, &plan(GrandStrategy::Expansion));
    }
    assert_eq!(ai.portfolio.samples[&0].len(), 1);
    assert_eq!(ai.portfolio.report.trace.len(), 1);
    let json = serde_json::to_string(&ai.victory_portfolio_report()).unwrap();
    let restored: PortfolioReport = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, ai.victory_portfolio_report());
}

#[test]
fn treatment_changes_a_real_legal_research_choice() {
    let mut g = world();
    g.turn = 150;
    g.world_era = 4;
    // Two legal frontier technologies; the existing Science posture picks
    // Rocketry, while a durable Culture primary must pick Printing.
    g.players[0].techs = g
        .rules
        .techs
        .keys()
        .copied()
        .filter(|tech| !matches!(tech.as_str(), "printing" | "rocketry"))
        .collect();
    g.players[0].research = None;
    let posture = plan(GrandStrategy::Science);
    let mut control_game = g.clone();
    let mut control = AdvancedAi::new();
    control.plan = Some(posture.clone());
    control.advanced_research(&mut control_game, 0, &posture);
    assert_eq!(
        control_game.players[0].research.as_deref(),
        Some("rocketry")
    );

    let mut treatment = driving(VictoryTarget::Culture, None);
    treatment.plan = Some(posture.clone());
    treatment.advanced_research(&mut g, 0, &posture);
    assert_eq!(g.players[0].research.as_deref(), Some("printing"));
}

#[test]
fn an_installed_compatible_finish_can_be_kept_as_a_secondary() {
    let mut g = world();
    g.turn = 170;
    expedition(&mut g, 0, 30.0);
    Arc::make_mut(&mut g.observed_public_empire_stats)
        .entry(0)
        .or_default()
        .foreign_tourists = Some(90);
    for pid in 1..3 {
        Arc::make_mut(&mut g.observed_public_empire_stats)
            .entry(pid)
            .or_default()
            .domestic_tourists = Some(100);
    }
    Arc::make_mut(&mut g.observed_tourism_per_turn).insert(0, 3000.0);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_victory_portfolio();
    ai.maintain_victory_portfolio(&g, 0);
    assert_eq!(ai.portfolio.report.primary, Some(VictoryTarget::Science));
    assert_eq!(ai.portfolio.report.secondary, Some(VictoryTarget::Culture));
}

#[test]
fn telemetry_observes_queue_allocation_and_marks_late_growth() {
    let mut g = world();
    g.turn = 150;
    let cid = g.player_city_ids(0)[0];
    let mut ai = driving(VictoryTarget::Science, Some(VictoryTarget::Culture));
    g.cities.get_mut(&cid).unwrap().queue = vec![Item::Building {
        building: crate::name!("library"),
    }];
    ai.record_portfolio_trace(&g, 0, &plan(GrandStrategy::Science));
    let allocation = ai.portfolio.report.trace[0]
        .production_allocation
        .as_ref()
        .unwrap();
    assert!(allocation.primary > 0.0);
    assert_eq!(allocation.other, 0.0);
    g.turn += g.standard_duration(25);
    g.cities.get_mut(&cid).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("settler"),
    }];
    ai.record_portfolio_trace(&g, 0, &plan(GrandStrategy::Science));
    let allocation = ai
        .portfolio
        .report
        .trace
        .last()
        .unwrap()
        .production_allocation
        .as_ref()
        .unwrap();
    assert!(allocation.settlers_and_builders > 0.0);
    assert_eq!(allocation.settlers_and_builders, allocation.other);
    assert_eq!(allocation.primary, 0.0);
}
