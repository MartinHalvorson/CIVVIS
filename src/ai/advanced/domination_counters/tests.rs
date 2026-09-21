use super::*;
use crate::game::ObservedPublicEmpireStats;

fn board(strategy: GrandStrategy) -> (Game, u32) {
    let mut g = Game::new_full(4, 48, 28, 91_004, 300, 0, false);
    super::tests::found_capitals(&mut g);
    g.turn = 100;
    for rival in 1..4 {
        g.record_contact(0, rival);
    }
    let capital = g.player_city_ids(1)[0];
    let site = super::tests::open_land_near(&g, g.cities[&capital].pos, 4);
    let objective = g.found_city_for(1, site, None);
    g.spawn_test_unit("scout", 0, site);
    match strategy {
        GrandStrategy::Culture => {
            g.cities
                .get_mut(&objective)
                .unwrap()
                .districts
                .insert(crate::name!("theater_square"), site);
            let stats = std::sync::Arc::make_mut(&mut g.observed_public_empire_stats);
            for pid in 0..4 {
                stats.insert(
                    pid,
                    ObservedPublicEmpireStats {
                        domestic_tourists: Some(if pid == 2 { 100 } else { 10 }),
                        foreign_tourists: Some(if pid == 1 { 78 } else { 0 }),
                        ..Default::default()
                    },
                );
            }
        }
        GrandStrategy::Religion => {
            // A founder still needs its army to stop the rival. Having our
            // own faith must not turn the entire empire into a Religion race.
            g.players[0].religion = Some("Buddhism".into());
            g.players[1].religion = Some("Orthodoxy".into());
            for city in g.cities.values_mut() {
                city.pressure.clear();
                city.pressure.insert(
                    if city.owner == 0 {
                        "Buddhism"
                    } else {
                        "Orthodoxy"
                    }
                    .into(),
                    100.0,
                );
            }
            g.cities
                .get_mut(&objective)
                .unwrap()
                .districts
                .insert(crate::name!("holy_site"), site);
        }
        _ => unreachable!(),
    }
    (g, objective)
}

#[test]
fn domination_counters_route_both_threats_without_optional_switches() {
    for strategy in [GrandStrategy::Culture, GrandStrategy::Religion] {
        let (g, objective) = board(strategy);
        let ai = AdvancedAi::targeting(VictoryTarget::Domination);
        assert!(
            !ai.deny_while_targeted
                && !ai.denial_outranks_expansion
                && !ai.counter_culture_by_conquest
        );
        assert_eq!(
            ai.rival_pressure(&g, 1),
            (
                strategy,
                if strategy == GrandStrategy::Religion {
                    75
                } else {
                    78
                }
            )
        );
        assert!(ai.denial_is_urgent(&g, 1));
        assert_eq!(ai.victory_denial(&g, 0), Some((1, GrandStrategy::Conquest)));
        assert_eq!(ai.denial_target(&g, 0), Some((1, GrandStrategy::Conquest)));
        let plan = ai.assess(&g, 0);
        assert_eq!(plan.strategy, GrandStrategy::Conquest, "{plan:?}");
        assert_eq!(plan.target_player, Some(1));
        assert_eq!(plan.target_city, Some(objective));
        assert_eq!(
            ai.active_victory_target(&g),
            Some(VictoryTarget::Domination)
        );
    }
}

#[test]
fn domination_counters_do_not_wait_for_a_slower_capital_race() {
    for strategy in [GrandStrategy::Culture, GrandStrategy::Religion] {
        let (g, _) = board(strategy);
        let ai = AdvancedAi::targeting(VictoryTarget::Domination);
        let pressure = ai.rival_victory_pressure(&g, 1);
        assert_eq!(
            ai.denial_response_for_pressure(&g, 0, 95, 1, pressure),
            Some(GrandStrategy::Conquest)
        );
    }
}

#[test]
fn domination_counters_respect_disabled_victories_and_denial_policy() {
    for strategy in [GrandStrategy::Culture, GrandStrategy::Religion] {
        let (mut g, _) = board(strategy);
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.deny_leaders = false;
        assert_eq!(ai.denial_target(&g, 0), None);
        ai.deny_leaders = true;
        if strategy == GrandStrategy::Culture {
            g.victory_conditions.culture = false;
        } else {
            g.victory_conditions.religious = false;
        }
        assert_eq!(ai.denial_target(&g, 0), None);
    }
}

#[test]
fn domination_counters_require_a_legal_known_enemy_and_ignore_teammates() {
    for strategy in [GrandStrategy::Culture, GrandStrategy::Religion] {
        let (g, _) = board(strategy);
        let ai = AdvancedAi::targeting(VictoryTarget::Domination);
        let mut allied = g.clone();
        allied.players[0].team = Some(1);
        allied.players[1].team = Some(1);
        assert_eq!(ai.denial_target(&allied, 0), None);
        let mut friends = g.clone();
        friends.players[0].friends_until.insert(1, g.turn + 10);
        assert_eq!(ai.denial_target(&friends, 0), None);
        let mut unknown = g.clone();
        for cid in unknown.player_city_ids(1) {
            unknown.cities.remove(&cid);
        }
        if strategy == GrandStrategy::Religion {
            std::sync::Arc::make_mut(&mut unknown.observed_majority_religion)
                .insert(1, "Orthodoxy".into());
        }
        assert!(ai.denial_is_urgent(&unknown, 1));
        assert_eq!(ai.denial_target(&unknown, 0), None);
    }
}

#[test]
fn domination_counters_leave_other_lanes_and_low_pressure_unchanged() {
    for strategy in [GrandStrategy::Culture, GrandStrategy::Religion] {
        let (g, _) = board(strategy);
        for target in [
            VictoryTarget::Science,
            VictoryTarget::Culture,
            VictoryTarget::Religion,
        ] {
            let ai = AdvancedAi::targeting(target);
            assert_eq!(ai.denial_target(&g, 0), None);
        }
        let ai = AdvancedAi::targeting(VictoryTarget::Domination);
        let below = VictoryFocus {
            strategy,
            progress: if strategy == GrandStrategy::Religion {
                74
            } else {
                49
            },
        };
        assert!(!ai.domination_counter_pressure(&g, below));
    }
}

#[test]
fn domination_religious_counter_also_works_without_a_founded_faith() {
    let (mut g, objective) = board(GrandStrategy::Religion);
    g.players[0].religion = None;
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let plan = ai.assess(&g, 0);
    assert_eq!(plan.strategy, GrandStrategy::Conquest);
    assert_eq!(plan.target_city, Some(objective));
}

#[test]
fn domination_religious_counter_tracks_the_living_major_match_point() {
    let (mut g, _) = board(GrandStrategy::Religion);
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    for (living, threshold) in [(4, 75), (3, 66), (2, 50)] {
        for p in g
            .players
            .iter_mut()
            .filter(|p| !p.is_minor && !p.is_barbarian)
        {
            p.alive = p.id < living;
        }
        assert!(ai.domination_counter_pressure(
            &g,
            VictoryFocus {
                strategy: GrandStrategy::Religion,
                progress: threshold
            }
        ));
        assert!(!ai.domination_counter_pressure(
            &g,
            VictoryFocus {
                strategy: GrandStrategy::Religion,
                progress: threshold - 1
            }
        ));
    }
}

#[test]
fn domination_culture_preparation_starts_early_without_bypassing_war_readiness() {
    let (mut g, objective) = board(GrandStrategy::Culture);
    std::sync::Arc::make_mut(&mut g.observed_public_empire_stats)
        .get_mut(&1)
        .unwrap()
        .foreign_tourists = Some(50);
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(!ai.denial_is_urgent(&g, 1));
    assert_eq!(ai.denial_target(&g, 0), Some((1, GrandStrategy::Conquest)));
    assert_eq!(ai.victory_denial(&g, 0), Some((1, GrandStrategy::Conquest)));
    let plan = ai.assess(&g, 0);
    assert_eq!(plan.strategy, GrandStrategy::Conquest);
    assert_eq!(plan.target_city, Some(objective));
    std::sync::Arc::make_mut(&mut g.observed_public_empire_stats)
        .get_mut(&1)
        .unwrap()
        .foreign_tourists = Some(49);
    assert_eq!(ai.denial_target(&g, 0), None);
}
