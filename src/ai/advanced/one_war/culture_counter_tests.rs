use super::*;
use crate::ai::advanced::StrategicPlan;
use std::sync::Arc;

/// A Domination seat beside a walled culture rival at match point, with no
/// aircraft and no army staged on its city.
fn fixture(armies: usize) -> (Game, AdvancedAi, StrategicPlan) {
    let mut g = Game::new_full(2, 32, 20, 370_001, 400, 0, false);
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
    g.found_city_for(0, (2, 14), None);
    let city = g.found_city_for(1, (9, 8), None);
    g.cities.get_mut(&city).unwrap().wall_hp = 400;
    g.record_contact(0, 1);
    g.at_war.clear();
    g.current = 0;
    g.turn = 170;
    g.players[0].gold = 10000.0;
    for y in 0..armies {
        g.spawn_test_unit("musketman", 0, (1, 2 + y as i32));
    }
    g.spawn_test_unit("musketman", 1, (12, 8));
    let observed = Arc::make_mut(&mut g.observed_public_empire_stats);
    observed.entry(0).or_default().domestic_tourists = Some(100);
    observed.entry(1).or_default().foreign_tourists = Some(85);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.coalition_before_war = false;
    ai.coalition_before_war_2 = false;
    ai.coalition_before_war_3 = false;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(city),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    assert!(ai.urgent_victory_threat(&g, 1));
    assert_eq!(
        ai.rival_victory_pressure(&g, 1).strategy,
        GrandStrategy::Culture
    );
    assert!(!ai.campaign_staged_for_war(&g, 0, 1, g.cities[&city].pos, true));
    (g, ai, plan)
}

/// See `culture_counter_due`: with the gene, an urgent culture rival is
/// declared on without a staged siege once we hold the ratio over it.
#[test]
fn an_urgent_culture_rival_is_declared_on_without_a_staged_siege() {
    for gene in [false, true] {
        let (mut g, mut ai, plan) = fixture(4);
        assert!(g.military_power(0) >= CULTURE_COUNTER_RATIO * g.military_power(1));
        if gene {
            ai.enable_culture_counter_declares();
        }
        ai.advanced_diplomacy(&mut g, 0, &plan);
        assert_eq!(g.is_at_war(0, 1), gene, "gene {gene}");
    }
}

/// Under the ratio the declaration still waits for the army.
#[test]
fn the_culture_counter_needs_the_ratio() {
    let (mut g, mut ai, plan) = fixture(1);
    assert!(g.military_power(0) < CULTURE_COUNTER_RATIO * g.military_power(1));
    ai.enable_culture_counter_declares();
    assert!(!ai.culture_counter_due(&g, 0, 1));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1));
}

/// See `culture_lane_threat`: a culture race behind a rival's higher
/// Diplomacy lane still makes it a counter target under the gene, so the
/// Recovery plan does not offer it peace (Norway, game 61).
#[test]
fn a_culture_race_behind_another_lane_is_still_countered() {
    let (mut g, mut ai, _) = fixture(4);
    let observed = Arc::make_mut(&mut g.observed_public_empire_stats);
    observed.entry(1).or_default().foreign_tourists = Some(77);
    g.players[1].dvp = 16;
    let (lane, _) = ai.rival_pressure(&g, 1);
    assert_ne!(lane, GrandStrategy::Culture, "fixture: another lane leads");
    assert_eq!(ai.rival_culture_progress(&g, 1), 77);
    assert!(
        !ai.domination_counter_target(&g, 0, 1),
        "masked without the gene"
    );
    ai.enable_culture_counter_declares();
    assert!(ai.culture_lane_threat(&g, 1));
    assert!(ai.domination_counter_target(&g, 0, 1));
}

/// See `culture_embargo_target`: with the gene, a culture rival at match
/// point whose cities we have not found is declared on anyway; without
/// the gene it is not (Maya, game 64).
#[test]
fn an_unlocated_culture_rival_is_declared_on_under_the_gene() {
    for gene in [false, true] {
        let (mut g, mut ai, plan) = fixture(4);
        for city in g.player_city_ids(1) {
            g.cities.remove(&city);
        }
        assert!(g.player_city_ids(1).is_empty() && g.players[1].alive);
        if gene {
            ai.enable_culture_counter_declares();
        }
        assert_eq!(ai.culture_embargo_target(&g, 0).is_some(), gene);
        ai.advanced_diplomacy(&mut g, 0, &plan);
        assert_eq!(g.is_at_war(0, 1), gene, "gene {gene}");
    }
}

/// See `strike_when_staged`: a denouncement opening becomes the surprise war
/// for a staged Domination army at twice the target's power, only with the
/// gene, only when staged, and only over the ratio.
#[test]
fn a_staged_army_strikes_instead_of_denouncing_under_the_gene() {
    let (g, mut ai, _) = fixture(4);
    let denounce = crate::game::Action::Denounce { player: 1 };
    assert!(g.military_power(0) >= 2.0 * g.military_power(1));
    assert_eq!(
        ai.strike_when_staged(&g, 0, 1, &denounce, true),
        None,
        "off"
    );
    ai.enable_domination_strikes_when_staged();
    assert_eq!(
        ai.strike_when_staged(&g, 0, 1, &denounce, true),
        Some(crate::game::Action::DeclareWar { player: 1 })
    );
    assert_eq!(
        ai.strike_when_staged(&g, 0, 1, &denounce, false),
        None,
        "not staged"
    );
    let (weak, _, _) = fixture(1);
    assert_eq!(
        ai.strike_when_staged(&weak, 0, 1, &denounce, true),
        None,
        "under the ratio"
    );
}

/// See `counter_war_has_the_emperor_edge`: under the gene the culture counter
/// opens without a staged siege only at 2.5 times the rival's steady power.
/// Emperor G188 (the Mapuche) and G192 (Greece) declared at 1.6 times and
/// were routed; at 2.6 times the counter still opens.
#[test]
fn the_culture_counter_needs_the_emperor_edge_under_the_gene() {
    for (ratio, over_the_bar) in [(1.6, false), (2.6, true)] {
        for gene in [false, true] {
            let (mut g, mut ai, plan) = fixture(4);
            let power = Arc::make_mut(&mut g.observed_military_power);
            power.insert(1, 1000.0);
            power.insert(0, 1000.0 * ratio);
            ai.enable_culture_counter_declares();
            if gene {
                ai.enable_counter_war_needs_the_emperor_edge();
            }
            let opens = !gene || over_the_bar;
            assert_eq!(
                ai.counter_war_has_the_emperor_edge(&g, 0, 1),
                opens,
                "ratio {ratio} gene {gene}"
            );
            assert_eq!(
                ai.culture_counter_due(&g, 0, 1),
                opens,
                "ratio {ratio} gene {gene}"
            );
            ai.advanced_diplomacy(&mut g, 0, &plan);
            assert_eq!(g.is_at_war(0, 1), opens, "ratio {ratio} gene {gene}");
        }
    }
}

/// See `culture_embargo_target`: under `counter-war-needs-the-emperor-edge`
/// a culture rival with no located city is never declared on, even at four
/// times its power (Emperor G191, India at turn 160).
#[test]
fn an_unlocated_culture_rival_is_not_embargoed_under_the_emperor_edge() {
    let (mut g, mut ai, plan) = fixture(4);
    for city in g.player_city_ids(1) {
        g.cities.remove(&city);
    }
    ai.enable_culture_counter_declares();
    assert_eq!(
        ai.culture_embargo_target(&g, 0),
        Some(1),
        "fixture: the embargo"
    );
    ai.enable_counter_war_needs_the_emperor_edge();
    assert!(
        ai.counter_war_has_the_emperor_edge(&g, 0, 1),
        "over the bar"
    );
    assert_eq!(ai.culture_embargo_target(&g, 0), None);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1));
}

#[test]
fn counter_war_needs_the_emperor_edge_is_a_native_opt_in_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "counter-war-needs-the-emperor-edge",
        |ai| ai.counter_war_needs_the_emperor_edge,
    );
}

/// See `declaration_has_production_parity`: under
/// `declaration-needs-production-parity` the urgent culture counter holds
/// against a rival whose public Production is more than 1/0.8 of ours, and
/// opens as shipped at parity or when the board has no reading of the
/// rival's Production (public figures without a Production term, as the
/// mirror leaves them when the host does not report it).
#[test]
fn the_culture_counter_needs_production_parity_under_the_gene() {
    let correction = |g: &Game, factor: f64| {
        let ours = crate::ai::BasicAi::seat_production_per_turn(g, 0);
        let derived = crate::ai::BasicAi::seat_production_per_turn(g, 1);
        let wanted = factor * ours - derived;
        if wanted == 0.0 {
            0.25
        } else {
            wanted
        }
    };
    // (their Production over ours, or None for no reading; gene; declares)
    for (factor, gene, declares) in [
        (Some(2.0), false, true),
        (Some(2.0), true, false),
        (Some(1.0), true, true),
        (None, true, true),
    ] {
        let (mut g, mut ai, plan) = fixture(4);
        ai.enable_culture_counter_declares();
        if gene {
            ai.enable_declaration_needs_production_parity();
        }
        assert!(
            AdvancedAi::rival_production_reading(&g, 1).is_none(),
            "fixture: public figures, no Production term"
        );
        if let Some(factor) = factor {
            let production = correction(&g, factor);
            Arc::make_mut(&mut g.observed_yield_adjustments).insert(
                1,
                crate::rules::Yields {
                    production,
                    ..Default::default()
                },
            );
            let theirs = AdvancedAi::rival_production_reading(&g, 1).expect("a reading");
            let ours = crate::ai::BasicAi::seat_production_per_turn(&g, 0);
            assert!(
                (theirs - factor * ours).abs() < 0.5,
                "fixture: theirs {theirs} against ours {ours} at {factor}"
            );
        }
        assert!(
            ai.culture_counter_due(&g, 0, 1),
            "fixture: the counter is due"
        );
        assert_eq!(
            ai.declaration_has_production_parity(&g, 0, 1),
            declares,
            "factor {factor:?} gene {gene}"
        );
        ai.advanced_diplomacy(&mut g, 0, &plan);
        assert_eq!(g.is_at_war(0, 1), declares, "factor {factor:?} gene {gene}");
    }
}

/// See `rival_production_reading`: a native board's cities are the whole
/// reading; public figures without a Production term are none; a Production
/// term is the host's total.
#[test]
fn a_rival_production_reading_needs_the_host_figure_on_a_partial_board() {
    let (mut g, _, _) = fixture(4);
    Arc::make_mut(&mut g.observed_public_empire_stats).remove(&1);
    let native = crate::ai::BasicAi::seat_production_per_turn(&g, 1);
    assert_eq!(AdvancedAi::rival_production_reading(&g, 1), Some(native));
    Arc::make_mut(&mut g.observed_public_empire_stats)
        .entry(1)
        .or_default();
    assert_eq!(AdvancedAi::rival_production_reading(&g, 1), None);
    Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        1,
        crate::rules::Yields {
            science: 3.0,
            ..Default::default()
        },
    );
    assert_eq!(
        AdvancedAi::rival_production_reading(&g, 1),
        None,
        "a correction without a Production term"
    );
    Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        1,
        crate::rules::Yields {
            production: 40.0,
            ..Default::default()
        },
    );
    assert_eq!(
        AdvancedAi::rival_production_reading(&g, 1),
        Some(native + 40.0)
    );
}
